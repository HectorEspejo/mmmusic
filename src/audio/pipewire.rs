use std::cell::RefCell;
use std::collections::HashMap;
use std::process;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pipewire as pw;
use pw::properties::properties;
use pw::spa;
use pw::stream::{StreamFlags, StreamListener, StreamRc, StreamState};
use spa::param::format::{MediaSubtype, MediaType};
use spa::param::format_utils;
use spa::pod::Pod;
use tokio::sync::watch;
use tracing::{info, warn};

use super::anillo::Anillo;
use crate::eventos::{AppEvento, NivelAviso};

pub const MAX_INTENTOS: u32 = 5;
const ESPERA_REINTENTO: Duration = Duration::from_secs(30);
const ESPERA_NODO: Duration = Duration::from_secs(5);
const ESPERA_ENLACE: Duration = Duration::from_secs(3);
const ESPERA_TRAS_FALLBACK: Duration = Duration::from_secs(5);
const INTERVALO_TIMER: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Default)]
pub enum EstadoCaptura {
    #[default]
    Desconectada,
    BuscandoNodo,
    Capturando {
        tasa_hz: u32,
        canales: u16,
        monitor: bool,
    },
    SinAudio,
    Error(String),
}

impl EstadoCaptura {
    pub fn etiqueta(&self) -> &'static str {
        match self {
            EstadoCaptura::Desconectada => "desconectada",
            EstadoCaptura::BuscandoNodo => "buscando nodo",
            EstadoCaptura::Capturando { .. } => "capturando",
            EstadoCaptura::SinAudio => "sin audio",
            EstadoCaptura::Error(_) => "PipeWire no disponible",
        }
    }

    pub fn capturando(&self) -> bool {
        matches!(self, EstadoCaptura::Capturando { .. })
    }

    pub fn por_monitor(&self) -> bool {
        matches!(self, EstadoCaptura::Capturando { monitor: true, .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComandoCaptura {
    Iniciar,
    Detener,
    Apagar,
}

pub struct ManejoCaptura {
    tx: Sender<ComandoCaptura>,
    hilo: Option<JoinHandle<()>>,
}

impl ManejoCaptura {
    pub fn enviar(&self, comando: ComandoCaptura) -> bool {
        self.tx.send(comando).is_ok()
    }

    pub fn apagar(mut self) {
        let _ = self.tx.send(ComandoCaptura::Apagar);
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

type Comandos = Rc<RefCell<Receiver<ComandoCaptura>>>;

pub fn lanzar(
    nodo: String,
    anillo: Arc<Anillo>,
    tx_app: Sender<AppEvento>,
) -> Result<(ManejoCaptura, watch::Receiver<EstadoCaptura>)> {
    let (tx_cmd, rx_cmd) = mpsc::channel();
    let (tx_estado, rx_estado) = watch::channel(EstadoCaptura::Desconectada);
    let hilo = thread::Builder::new()
        .name("captura".to_string())
        .spawn(move || {
            let comandos = Rc::new(RefCell::new(rx_cmd));
            ejecutar(nodo, anillo, tx_app, comandos, tx_estado);
        })
        .context("no se pudo lanzar el hilo de captura")?;
    Ok((
        ManejoCaptura {
            tx: tx_cmd,
            hilo: Some(hilo),
        },
        rx_estado,
    ))
}

enum Espera {
    Reintentar,
    Apagar,
}

fn ejecutar(
    nombre_nodo: String,
    anillo: Arc<Anillo>,
    tx_app: Sender<AppEvento>,
    comandos: Comandos,
    tx_estado: watch::Sender<EstadoCaptura>,
) {
    pw::init();
    let mut intentos = 0u32;
    loop {
        match intentar_sesion(&nombre_nodo, &anillo, &comandos, &tx_estado) {
            FinSesion::Apagada => break,
            FinSesion::Desconectada => {}
        }
        intentos += 1;
        if intentos >= MAX_INTENTOS {
            tx_estado.send_replace(EstadoCaptura::Error("PipeWire no disponible".to_string()));
            let _ = tx_app.send(AppEvento::Notificacion(
                NivelAviso::Aviso,
                "Visuales sin audio: PipeWire no disponible".to_string(),
            ));
            info!("captura: PipeWire no disponible tras {intentos} intentos");
            match esperar_comando(&comandos, None) {
                Espera::Apagar => break,
                Espera::Reintentar => {
                    intentos = 0;
                    continue;
                }
            }
        }
        match esperar_comando(&comandos, Some(ESPERA_REINTENTO)) {
            Espera::Apagar => break,
            Espera::Reintentar => {}
        }
    }
    info!("hilo de captura detenido");
}

fn esperar_comando(comandos: &Comandos, espera: Option<Duration>) -> Espera {
    let limite = espera.map(|espera| Instant::now() + espera);
    loop {
        let restante = limite.map(|limite| limite.saturating_duration_since(Instant::now()));
        if restante.is_some_and(|restante| restante.is_zero()) {
            return Espera::Reintentar;
        }
        let resultado = {
            let receptor = comandos.borrow();
            match restante {
                Some(restante) => receptor.recv_timeout(restante),
                None => receptor
                    .recv()
                    .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
            }
        };
        match resultado {
            Ok(ComandoCaptura::Apagar) => return Espera::Apagar,
            Ok(ComandoCaptura::Iniciar) => return Espera::Reintentar,
            Ok(ComandoCaptura::Detener) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => return Espera::Reintentar,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Espera::Apagar,
        }
    }
}

enum FinSesion {
    Apagada,
    Desconectada,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PasoEnlace {
    Directo,
    MonitorNodo,
    MonitorDefecto,
}

#[derive(Clone)]
struct Nodo {
    global_id: u32,
    serial: String,
}

struct DatosStream {
    anillo: Arc<Anillo>,
    sesion: Rc<RefCell<Sesion>>,
}

struct Sesion {
    pid: u32,
    nombre_nodo: String,
    anillo: Arc<Anillo>,
    tx_estado: watch::Sender<EstadoCaptura>,
    rx_cmd: Comandos,
    core: pw::core::CoreRc,
    nodo: Option<Nodo>,
    nodos_huerfanos: HashMap<u32, (Nodo, Option<u32>)>,
    clientes: HashMap<u32, (Option<String>, Option<u32>)>,
    stream: Option<StreamRc>,
    listener: Option<StreamListener<DatosStream>>,
    paso: PasoEnlace,
    monitor: bool,
    capturando: bool,
    conectado: bool,
    ultimo_formato: Option<(u32, u16)>,
    intento_enlace: Option<Instant>,
    proximo_intento: Option<Instant>,
    sin_nodo_desde: Option<Instant>,
    activo: bool,
    apagando: bool,
    caido: bool,
    estado: EstadoCaptura,
}

impl Sesion {
    fn publicar(&mut self, estado: EstadoCaptura) {
        if self.estado != estado {
            self.estado = estado.clone();
            self.tx_estado.send_replace(estado);
        }
    }

    fn destruir_stream(&mut self) {
        if let Some(stream) = self.stream.take() {
            let _ = stream.disconnect();
        }
        self.listener = None;
        self.capturando = false;
        self.conectado = false;
        self.ultimo_formato = None;
        self.intento_enlace = None;
    }

    fn detener(&mut self) {
        self.destruir_stream();
        self.activo = false;
        self.publicar(EstadoCaptura::Desconectada);
    }

    fn iniciar(&mut self) {
        self.activo = true;
        self.paso = PasoEnlace::Directo;
        self.monitor = false;
        self.proximo_intento = None;
        if self.nodo.is_none() && self.sin_nodo_desde.is_none() {
            self.sin_nodo_desde = Some(Instant::now());
        }
        if self.estado == EstadoCaptura::Desconectada {
            self.publicar(EstadoCaptura::BuscandoNodo);
        }
    }

    fn confirmar_formato(&mut self, tasa_hz: u32, canales: u16) {
        self.ultimo_formato = Some((tasa_hz, canales));
        let estado = EstadoCaptura::Capturando {
            tasa_hz,
            canales,
            monitor: self.monitor,
        };
        let cambio = !self.capturando || self.estado != estado;
        self.capturando = true;
        self.intento_enlace = None;
        if cambio {
            self.publicar(estado);
        }
    }

    fn colocar_nodo(&mut self, nodo: Nodo, es_propio: bool) {
        if self
            .nodo
            .as_ref()
            .is_some_and(|actual| actual.global_id == nodo.global_id)
        {
            return;
        }
        if self.nodo.is_some() && !es_propio {
            return;
        }
        info!(nodo = %self.nombre_nodo, serial = %nodo.serial, "nodo de audio encontrado");
        self.nodo = Some(nodo);
        self.sin_nodo_desde = None;
        self.paso = PasoEnlace::Directo;
        self.proximo_intento = None;
    }
}

fn intentar_sesion(
    nombre_nodo: &str,
    anillo: &Arc<Anillo>,
    comandos: &Comandos,
    tx_estado: &watch::Sender<EstadoCaptura>,
) -> FinSesion {
    let mainloop = match pw::main_loop::MainLoopRc::new(None) {
        Ok(mainloop) => mainloop,
        Err(error) => {
            warn!("no se pudo crear el bucle de PipeWire: {error}");
            return FinSesion::Desconectada;
        }
    };
    let context = match pw::context::ContextRc::new(&mainloop, None) {
        Ok(context) => context,
        Err(error) => {
            warn!("no se pudo crear el contexto de PipeWire: {error}");
            return FinSesion::Desconectada;
        }
    };
    let core = match context.connect_rc(None) {
        Ok(core) => core,
        Err(error) => {
            warn!("no se pudo conectar con PipeWire: {error}");
            return FinSesion::Desconectada;
        }
    };
    let registry = match core.get_registry_rc() {
        Ok(registry) => registry,
        Err(error) => {
            warn!("no se pudo obtener el registro de PipeWire: {error}");
            return FinSesion::Desconectada;
        }
    };
    let sesion = Rc::new(RefCell::new(Sesion {
        pid: process::id(),
        nombre_nodo: nombre_nodo.to_string(),
        anillo: anillo.clone(),
        tx_estado: tx_estado.clone(),
        rx_cmd: comandos.clone(),
        core: core.clone(),
        nodo: None,
        nodos_huerfanos: HashMap::new(),
        clientes: HashMap::new(),
        stream: None,
        listener: None,
        paso: PasoEnlace::Directo,
        monitor: false,
        capturando: false,
        conectado: false,
        ultimo_formato: None,
        intento_enlace: None,
        proximo_intento: None,
        sin_nodo_desde: Some(Instant::now()),
        activo: true,
        apagando: false,
        caido: false,
        estado: EstadoCaptura::Desconectada,
    }));

    let _listener_registro = {
        let sesion_global = sesion.clone();
        let sesion_cliente = sesion.clone();
        let sesion_remove = sesion.clone();
        registry
            .add_listener_local()
            .global(move |global| match global.type_ {
                pw::types::ObjectType::Node => descubrir_nodo(&sesion_global, global),
                pw::types::ObjectType::Client => descubrir_cliente(&sesion_cliente, global),
                _ => {}
            })
            .global_remove(move |id| quitar_global(&sesion_remove, id))
            .register()
    };

    let timer = {
        let sesion_timer = sesion.clone();
        let mainloop_timer = mainloop.clone();
        let timer = mainloop.loop_().add_timer(move |_| {
            if revisar(&sesion_timer) {
                mainloop_timer.quit();
            }
        });
        timer.update_timer(Some(INTERVALO_TIMER), Some(INTERVALO_TIMER));
        timer
    };

    sesion.borrow_mut().publicar(EstadoCaptura::BuscandoNodo);
    mainloop.run();
    drop(timer);
    if sesion.borrow().apagando {
        FinSesion::Apagada
    } else {
        FinSesion::Desconectada
    }
}

fn descubrir_nodo(
    sesion: &Rc<RefCell<Sesion>>,
    global: &pw::registry::GlobalObject<&spa::utils::dict::DictRef>,
) {
    let Some(props) = global.props else {
        return;
    };
    if props.get("media.class") == Some("Stream/Input/Audio") {
        return;
    }
    let pid = props
        .get("application.process.id")
        .and_then(|valor| valor.parse::<u32>().ok());
    let nombre = props
        .get("application.name")
        .or_else(|| props.get("node.name"));
    let cliente_id = props
        .get("client.id")
        .and_then(|valor| valor.parse::<u32>().ok());
    let serial = props
        .get("object.serial")
        .map(str::to_string)
        .unwrap_or_else(|| global.id.to_string());
    let nodo = Nodo {
        global_id: global.id,
        serial,
    };
    let mut s = sesion.borrow_mut();
    let por_props = pid == Some(s.pid) || nombre == Some(s.nombre_nodo.as_str());
    let cliente = cliente_id.and_then(|id| s.clientes.get(&id));
    let por_cliente = cliente.is_some_and(|(nombre_cliente, pid_cliente)| {
        *pid_cliente == Some(s.pid) || nombre_cliente.as_deref() == Some(s.nombre_nodo.as_str())
    });
    if !por_props && !por_cliente {
        if let Some(cliente_id) = cliente_id
            && !s.clientes.contains_key(&cliente_id)
        {
            s.nodos_huerfanos
                .insert(global.id, (nodo, Some(cliente_id)));
        }
        return;
    }
    let es_propio =
        pid == Some(s.pid) || cliente.is_some_and(|(_, pid_cliente)| *pid_cliente == Some(s.pid));
    s.colocar_nodo(nodo, es_propio);
}

fn descubrir_cliente(
    sesion: &Rc<RefCell<Sesion>>,
    global: &pw::registry::GlobalObject<&spa::utils::dict::DictRef>,
) {
    let Some(props) = global.props else {
        return;
    };
    let nombre = props.get("application.name").map(str::to_string);
    let pid = props
        .get("application.process.id")
        .and_then(|valor| valor.parse::<u32>().ok());
    let mut s = sesion.borrow_mut();
    s.clientes.insert(global.id, (nombre.clone(), pid));
    let candidatos: Vec<u32> = s
        .nodos_huerfanos
        .iter()
        .filter(|(_, (_, cliente_id))| *cliente_id == Some(global.id))
        .map(|(id, _)| *id)
        .collect();
    for id in candidatos {
        if let Some((nodo, _)) = s.nodos_huerfanos.remove(&id) {
            let es_propio = pid == Some(s.pid) || nombre.as_deref() == Some(s.nombre_nodo.as_str());
            s.colocar_nodo(nodo, es_propio);
        }
    }
}

fn quitar_global(sesion: &Rc<RefCell<Sesion>>, id: u32) {
    let mut s = sesion.borrow_mut();
    s.clientes.remove(&id);
    s.nodos_huerfanos.remove(&id);
    let Some(nodo) = s.nodo.clone() else {
        return;
    };
    if nodo.global_id != id {
        return;
    }
    info!("el nodo de audio desapareció");
    s.destruir_stream();
    s.nodo = None;
    s.sin_nodo_desde = Some(Instant::now());
    if s.activo {
        s.publicar(EstadoCaptura::SinAudio);
    }
}

fn revisar(sesion: &Rc<RefCell<Sesion>>) -> bool {
    let (apagar, recrear) = {
        let mut s = sesion.borrow_mut();
        loop {
            let comando = s.rx_cmd.borrow_mut().try_recv();
            match comando {
                Ok(ComandoCaptura::Apagar) => {
                    s.apagando = true;
                    return true;
                }
                Ok(ComandoCaptura::Detener) => s.detener(),
                Ok(ComandoCaptura::Iniciar) => s.iniciar(),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    s.apagando = true;
                    return true;
                }
            }
        }

        let mut recrear = false;
        if s.activo && !s.apagando {
            let ahora = Instant::now();
            if s.caido {
                s.caido = false;
                fallar_enlace_locked(&mut s);
            }
            if s.nodo.is_none() {
                if s.sin_nodo_desde.is_none() {
                    s.sin_nodo_desde = Some(ahora);
                }
                if !matches!(
                    s.estado,
                    EstadoCaptura::SinAudio | EstadoCaptura::BuscandoNodo
                ) && s
                    .sin_nodo_desde
                    .is_some_and(|desde| ahora.duration_since(desde) >= ESPERA_NODO)
                {
                    s.publicar(EstadoCaptura::SinAudio);
                }
            } else {
                let esperando_fallback = s.proximo_intento.is_some_and(|proximo| ahora < proximo);
                if !esperando_fallback {
                    s.proximo_intento = None;
                }
                if s.stream.is_none() && !esperando_fallback {
                    recrear = true;
                } else if s.stream.is_some() {
                    if !s.capturando
                        && !s.conectado
                        && s.intento_enlace
                            .is_some_and(|intento| ahora.duration_since(intento) >= ESPERA_ENLACE)
                    {
                        fallar_enlace_locked(&mut s);
                        recrear = s.proximo_intento.is_none();
                    } else if !s.capturando && s.conectado {
                        // El enlace está preparado pero la fuente aún no
                        // produce audio (reproductor en pausa o detenido).
                        s.publicar(EstadoCaptura::SinAudio);
                    }
                }
            }
        }
        (s.apagando, recrear)
    };
    if !apagar && recrear {
        crear_stream(sesion);
    }
    apagar
}

fn fallar_enlace(sesion: &Rc<RefCell<Sesion>>) {
    fallar_enlace_locked(&mut sesion.borrow_mut());
}

fn fallar_enlace_locked(sesion: &mut Sesion) {
    sesion.destruir_stream();
    sesion.paso = match sesion.paso {
        PasoEnlace::Directo => PasoEnlace::MonitorNodo,
        PasoEnlace::MonitorNodo => PasoEnlace::MonitorDefecto,
        PasoEnlace::MonitorDefecto => {
            sesion.proximo_intento = Some(Instant::now() + ESPERA_TRAS_FALLBACK);
            PasoEnlace::Directo
        }
    };
    sesion.monitor = sesion.paso != PasoEnlace::Directo;
    if sesion.paso == PasoEnlace::Directo {
        info!("captura: fallback agotado; se reintentará el enlace directo");
        sesion.publicar(EstadoCaptura::SinAudio);
    } else {
        info!(paso = ?sesion.paso, "captura: se prueba el fallback por monitor");
    }
}

fn crear_stream(sesion: &Rc<RefCell<Sesion>>) {
    let (core, nodo, paso, anillo) = {
        let mut s = sesion.borrow_mut();
        s.destruir_stream();
        let Some(nodo) = s.nodo.clone() else {
            return;
        };
        (s.core.clone(), nodo, s.paso, s.anillo.clone())
    };
    let mut props = properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        *pw::keys::MEDIA_ROLE => "Music",
        *pw::keys::NODE_NAME => "mmmusic-visuales",
        *pw::keys::NODE_DESCRIPTION => "mmmusic visuales",
        *pw::keys::STREAM_DONT_REMIX => "true",
    };
    match paso {
        PasoEnlace::Directo => {
            props.insert(*pw::keys::TARGET_OBJECT, nodo.serial.as_str());
        }
        PasoEnlace::MonitorNodo => {
            props.insert(*pw::keys::TARGET_OBJECT, nodo.serial.as_str());
            props.insert(*pw::keys::STREAM_CAPTURE_SINK, "true");
        }
        PasoEnlace::MonitorDefecto => {
            props.insert(*pw::keys::STREAM_CAPTURE_SINK, "true");
        }
    }
    let stream = match StreamRc::new(core, "mmmusic-visuales", props) {
        Ok(stream) => stream,
        Err(error) => {
            warn!("no se pudo crear el stream de captura: {error}");
            fallar_enlace(sesion);
            return;
        }
    };
    let datos = DatosStream {
        anillo,
        sesion: sesion.clone(),
    };
    let listener = match stream
        .add_local_listener_with_user_data(datos)
        .param_changed(|_, datos, id, param| {
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            let Some(param) = param else {
                return;
            };
            let Ok((tipo, subtipo)) = format_utils::parse_format(param) else {
                return;
            };
            if tipo != MediaType::Audio || subtipo != MediaSubtype::Raw {
                return;
            }
            let mut formato = spa::param::audio::AudioInfoRaw::default();
            if formato.parse(param).is_err() {
                return;
            }
            if let Ok(mut s) = datos.sesion.try_borrow_mut() {
                s.confirmar_formato(formato.rate(), formato.channels() as u16);
            }
        })
        .state_changed(|stream, datos, viejo, nuevo| {
            tracing::debug!(
                nodo = stream.name(),
                ?viejo,
                ?nuevo,
                "estado del stream de captura"
            );
            let Ok(mut s) = datos.sesion.try_borrow_mut() else {
                return;
            };
            match &nuevo {
                StreamState::Paused | StreamState::Streaming => {
                    s.conectado = true;
                    if matches!(nuevo, StreamState::Streaming) {
                        if let Some((tasa, canales)) = s.ultimo_formato {
                            s.confirmar_formato(tasa, canales);
                        }
                    } else if s.capturando {
                        s.capturando = false;
                        s.publicar(EstadoCaptura::SinAudio);
                    }
                }
                StreamState::Error(_) => {
                    s.conectado = false;
                    s.caido = true;
                }
                StreamState::Connecting | StreamState::Unconnected => {
                    s.conectado = false;
                }
            }
        })
        .process(|stream, datos| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let Some(data) = buffer.datas_mut().first_mut() else {
                return;
            };
            let tamano = data.chunk().size() as usize;
            if let Some(bytes) = data.data() {
                let fin = tamano.min(bytes.len());
                if fin > 0 {
                    datos.anillo.escribir_bytes_f32_le(&bytes[..fin]);
                }
            }
        })
        .register()
    {
        Ok(listener) => listener,
        Err(error) => {
            warn!("no se pudo registrar el listener del stream: {error}");
            fallar_enlace(sesion);
            return;
        }
    };
    let Some(valores) = parametros_captura() else {
        warn!("no se pudieron serializar los parámetros de captura");
        fallar_enlace(sesion);
        return;
    };
    let Some(pod) = Pod::from_bytes(&valores) else {
        warn!("los parámetros de captura no son un Pod válido");
        fallar_enlace(sesion);
        return;
    };
    let mut params = [pod];
    let resultado = stream.connect(
        spa::utils::Direction::Input,
        None,
        StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
        &mut params,
    );
    match resultado {
        Ok(()) => {
            let mut s = sesion.borrow_mut();
            s.stream = Some(stream);
            s.listener = Some(listener);
            s.monitor = paso != PasoEnlace::Directo;
            s.intento_enlace = Some(Instant::now());
        }
        Err(error) => {
            warn!("no se pudo conectar el stream de captura: {error}");
            fallar_enlace(sesion);
        }
    }
}

fn parametros_captura() -> Option<Vec<u8>> {
    let mut audio_info = spa::param::audio::AudioInfoRaw::new();
    audio_info.set_format(spa::param::audio::AudioFormat::F32LE);
    audio_info.set_channels(2);
    let objeto = spa::pod::Object {
        type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: spa::param::ParamType::EnumFormat.as_raw(),
        properties: audio_info.into(),
    };
    let (cursor, _) = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(objeto),
    )
    .ok()?;
    Some(cursor.into_inner())
}
