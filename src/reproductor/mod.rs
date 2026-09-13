pub mod cola;
pub mod estado;
pub mod mpv;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rusqlite::Connection;
use tokio::sync::watch;
use tracing::{info, warn};

use self::cola::{Cola, ResultadoEliminar};
use self::estado::{Estado, EstadoReproduccion, EstadoStream, Repeticion};
use self::mpv::{EventoMpv, RazonFin, ReproductorMpv, Valor};
use crate::biblioteca::modelos::{ElementoCola, EmisoraResumen};
use crate::biblioteca::{bd, consultas};
use crate::eventos::{AppEvento, NivelAviso};
use crate::radio::icy;
use crate::radio::reconexion::PlanReconexion;
use crate::scrobbling::ComandoScrobbling;
use crate::scrobbling::regla;

const TICK_REPRODUCTOR: Duration = Duration::from_millis(50);
const INTERVALO_PUBLICACION: Duration = Duration::from_millis(250);
const MAX_FALLOS_SEGUIDOS: u32 = 3;
const UMBRAL_SALTO_MS: i64 = 1500;
const UMBRAL_SCROBBLE_ICY_MS: i64 = 30_000;
const PAUSA_MAXIMA_STREAM: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComandoReproductor {
    ReemplazarCola {
        elementos: Vec<ElementoCola>,
        indice: usize,
    },
    AnadirAlFinal {
        elementos: Vec<ElementoCola>,
    },
    ReproducirSiguiente {
        elementos: Vec<ElementoCola>,
    },
    EliminarDeCola {
        posicion: usize,
    },
    MoverEnCola {
        de: usize,
        a: usize,
    },
    VaciarCola,
    SaltarA {
        posicion: usize,
    },
    AlternarPausa,
    Reanudar,
    Pausar,
    Detener,
    Siguiente,
    Anterior,
    Buscar {
        ms: i64,
        relativo: bool,
    },
    Volumen {
        valor: u8,
    },
    AlternarSilencio,
    AlternarAleatorio,
    CiclarRepeticion,
    FijarRepeticion {
        modo: Repeticion,
    },
    Apagar,
}

pub struct ManejoReproductor {
    tx: Sender<ComandoReproductor>,
    hilo: Option<JoinHandle<()>>,
}

impl ManejoReproductor {
    pub fn enviar(&self, comando: ComandoReproductor) -> bool {
        self.tx.send(comando).is_ok()
    }

    pub fn emisor(&self) -> Sender<ComandoReproductor> {
        self.tx.clone()
    }

    pub fn apagar(mut self) {
        let _ = self.tx.send(ComandoReproductor::Apagar);
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

pub fn lanzar(
    ruta_bd: PathBuf,
    volumen_inicial: u8,
    espera_conexion_s: u64,
    tx_app: Sender<AppEvento>,
    tx_scrobbling: Sender<ComandoScrobbling>,
) -> Result<(ManejoReproductor, watch::Receiver<EstadoReproduccion>)> {
    let conn = bd::abrir_y_migrar(&ruta_bd)?;

    let (items, indice) = consultas::cola::cargar(&conn).unwrap_or_default();
    let cola_ms = consultas::ajustes::leer_i64(&conn, "cola_ms")?.unwrap_or(0);
    let volumen = consultas::ajustes::leer_i64(&conn, "volumen")?
        .map(|valor| valor.clamp(0, 100) as u8)
        .unwrap_or(volumen_inicial);
    let silencio = consultas::ajustes::leer_bool(&conn, "silencio")?.unwrap_or(false);
    let aleatorio = consultas::ajustes::leer_bool(&conn, "aleatorio")?.unwrap_or(false);
    let repeticion = consultas::ajustes::leer(&conn, "repeticion")?
        .and_then(|valor| Repeticion::desde_str(&valor))
        .unwrap_or(Repeticion::No);

    let mut cola = Cola::nueva();
    cola.aleatorio = aleatorio;
    cola.repeticion = repeticion;
    cola.restaurar(items, indice);

    let elemento = cola.actual().map(|item| item.elemento.clone());
    let es_emisora = elemento.as_ref().is_some_and(ElementoCola::es_emisora);
    let duracion_ms = elemento
        .as_ref()
        .and_then(ElementoCola::pista)
        .map(|pista| pista.duracion_ms)
        .unwrap_or(0);
    let posicion_ms = if es_emisora { 0 } else { cola_ms };
    let estado = EstadoReproduccion {
        volumen,
        silencio,
        aleatorio,
        repeticion,
        cola: cola.elementos(),
        cola_indice: cola.indice,
        elemento,
        posicion_ms,
        duracion_ms,
        ..EstadoReproduccion::default()
    };

    let (tx_estado, rx_estado) = watch::channel(estado.clone());
    let _ = tx_app.send(AppEvento::Reproductor(Box::new(estado.clone())));

    let (tx_cmd, rx_cmd) = mpsc::channel();
    let (tx_despertar, rx_despertar) = mpsc::channel();
    let hilo = thread::Builder::new()
        .name("reproductor".to_string())
        .spawn(move || {
            let mut reproductor = match ReproductorInterno::nuevo(
                ruta_bd,
                estado,
                cola,
                tx_app,
                tx_estado,
                rx_cmd,
                rx_despertar,
                tx_despertar,
                tx_scrobbling,
                posicion_ms,
                Duration::from_secs(espera_conexion_s.max(1)),
            ) {
                Ok(reproductor) => reproductor,
                Err(error) => {
                    warn!("no se pudo iniciar el reproductor: {error:#}");
                    return;
                }
            };
            reproductor.ejecutar();
        })
        .context("no se pudo lanzar el hilo reproductor")?;

    Ok((
        ManejoReproductor {
            tx: tx_cmd,
            hilo: Some(hilo),
        },
        rx_estado,
    ))
}

struct ReproductorInterno {
    mpv: ReproductorMpv,
    cola: Cola,
    estado: EstadoReproduccion,
    conn: Connection,
    tx_app: Sender<AppEvento>,
    tx_estado: watch::Sender<EstadoReproduccion>,
    tx_scrobbling: Sender<ComandoScrobbling>,
    rx_cmd: Receiver<ComandoReproductor>,
    rx_despertar: Receiver<()>,
    historial_id: Option<i64>,
    historial_inicio: Option<String>,
    tiempo_reproducido_ms: i64,
    ultima_pos_ms: i64,
    completada: bool,
    fallos_seguidos: u32,
    posicion_restauracion_ms: i64,
    carga_en_pausa: bool,
    precargada: Option<usize>,
    ultima_publicacion: Instant,
    ultimo_tick: Instant,
    plan_reconexion: PlanReconexion,
    espera_conexion: Duration,
    conexion_inicio: Option<Instant>,
    reintento_en: Option<Instant>,
    url_stream: Option<String>,
    modo_lista: bool,
    pausado_desde: Option<Instant>,
    ultimo_titulo_icy: Option<String>,
    tiempo_icy_ms: i64,
    completada_icy: bool,
    cache_segundos: f32,
    buffering_actual: Option<i64>,
    ultimo_error_stream: Option<String>,
}

impl ReproductorInterno {
    #[allow(clippy::too_many_arguments)]
    fn nuevo(
        ruta_bd: PathBuf,
        estado: EstadoReproduccion,
        cola: Cola,
        tx_app: Sender<AppEvento>,
        tx_estado: watch::Sender<EstadoReproduccion>,
        rx_cmd: Receiver<ComandoReproductor>,
        rx_despertar: Receiver<()>,
        tx_despertar: Sender<()>,
        tx_scrobbling: Sender<ComandoScrobbling>,
        posicion_restauracion_ms: i64,
        espera_conexion: Duration,
    ) -> Result<Self> {
        let conn = bd::abrir_y_migrar(&ruta_bd)?;
        let mut mpv = ReproductorMpv::nuevo(estado.volumen)?;
        mpv.observar_propiedades()?;
        mpv.fijar_despertador(move || {
            let _ = tx_despertar.send(());
        });
        Ok(Self {
            mpv,
            cola,
            estado,
            conn,
            tx_app,
            tx_estado,
            tx_scrobbling,
            rx_cmd,
            rx_despertar,
            historial_id: None,
            historial_inicio: None,
            tiempo_reproducido_ms: 0,
            ultima_pos_ms: 0,
            completada: false,
            fallos_seguidos: 0,
            posicion_restauracion_ms,
            carga_en_pausa: false,
            precargada: None,
            ultima_publicacion: Instant::now() - INTERVALO_PUBLICACION,
            ultimo_tick: Instant::now() - INTERVALO_PUBLICACION,
            plan_reconexion: PlanReconexion::nuevo(1),
            espera_conexion,
            conexion_inicio: None,
            reintento_en: None,
            url_stream: None,
            modo_lista: false,
            pausado_desde: None,
            ultimo_titulo_icy: None,
            tiempo_icy_ms: 0,
            completada_icy: true,
            cache_segundos: 0.0,
            buffering_actual: None,
            ultimo_error_stream: None,
        })
    }

    fn ejecutar(&mut self) {
        self.restaurar_sesion();
        loop {
            match self.procesar_comandos() {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => warn!("error procesando comandos: {error:#}"),
            }
            let _ = self.rx_despertar.recv_timeout(TICK_REPRODUCTOR);
            self.procesar_eventos();
            self.tick();
        }
        self.guardar_estado_final();
        info!("hilo reproductor detenido");
    }

    fn restaurar_sesion(&mut self) {
        if let Some(item) = self.cola.actual() {
            match item.elemento.clone() {
                ElementoCola::Pista(pista) => {
                    let ruta = pista.ruta.clone();
                    self.estado.estado = Estado::Cargando;
                    self.carga_en_pausa = true;
                    self.fallos_seguidos = 0;
                    if let Err(error) = self.mpv.pausar(true) {
                        warn!("no se pudo dejar en pausa: {error:#}");
                    }
                    if let Err(error) = self.mpv.cargar(&ruta) {
                        warn!("no se pudo restaurar la pista {ruta}: {error:#}");
                        self.estado.estado = Estado::Detenido;
                    }
                }
                ElementoCola::Emisora(emisora) => {
                    self.estado.estado = Estado::Detenido;
                    self.estado.posicion_ms = 0;
                    self.estado.duracion_ms = 0;
                    self.estado.stream = None;
                    self.estado.titulo_icy = None;
                    self.estado.codec = emisora.codec.clone();
                    self.estado.bitrate_kbps = emisora.bitrate_kbps;
                }
            }
        }
        self.publicar(true);
    }

    fn procesar_comandos(&mut self) -> Result<bool> {
        loop {
            match self.rx_cmd.try_recv() {
                Ok(ComandoReproductor::Apagar) => return Ok(false),
                Ok(comando) => self.ejecutar_comando(comando)?,
                Err(TryRecvError::Empty) => return Ok(true),
                Err(TryRecvError::Disconnected) => return Ok(false),
            }
        }
    }

    fn ejecutar_comando(&mut self, comando: ComandoReproductor) -> Result<()> {
        match comando {
            ComandoReproductor::ReemplazarCola { elementos, indice } => {
                self.cola.reemplazar(elementos, indice);
                self.persistir();
                if self.cola.vacia() {
                    self.detener();
                } else {
                    self.cargar_indice(self.cola.indice.unwrap_or(0));
                }
            }
            ComandoReproductor::AnadirAlFinal { elementos } => {
                let estaba_vacia = self.cola.anadir_al_final(elementos);
                self.persistir();
                if estaba_vacia && self.estado.estado == Estado::Detenido {
                    self.cargar_indice(self.cola.indice.unwrap_or(0));
                } else {
                    self.publicar(true);
                }
            }
            ComandoReproductor::ReproducirSiguiente { elementos } => {
                let tenia_actual = self.cola.actual().is_some();
                self.cola.reproducir_a_continuacion(elementos);
                self.persistir();
                if !tenia_actual && self.estado.estado == Estado::Detenido {
                    self.cargar_indice(self.cola.indice.unwrap_or(0));
                } else {
                    self.publicar(true);
                }
            }
            ComandoReproductor::EliminarDeCola { posicion } => match self.cola.eliminar(posicion) {
                ResultadoEliminar::QuitadaActual { detener } => {
                    self.persistir();
                    if detener {
                        self.detener();
                    } else {
                        let indice = self.cola.indice.unwrap_or(0);
                        self.cargar_indice(indice);
                    }
                }
                ResultadoEliminar::QuitadaNoActual => {
                    self.persistir();
                    self.publicar(true);
                }
            },
            ComandoReproductor::MoverEnCola { de, a } => {
                self.cola.mover(de, a);
                self.persistir();
                self.publicar(true);
            }
            ComandoReproductor::VaciarCola => {
                self.cola.vaciar();
                self.persistir();
                self.detener();
            }
            ComandoReproductor::SaltarA { posicion } => {
                if self.cola.saltar_a(posicion) {
                    self.cargar_indice(posicion);
                }
            }
            ComandoReproductor::AlternarPausa => match self.estado.estado {
                Estado::Reproduciendo => self.pausar(),
                Estado::Pausado => self.reanudar(),
                Estado::Detenido => {
                    if self.cola.actual().is_some() {
                        let indice = self.cola.indice.unwrap_or(0);
                        self.cargar_indice(indice);
                    }
                }
                Estado::Cargando | Estado::Error => {}
            },
            ComandoReproductor::Reanudar => self.reanudar(),
            ComandoReproductor::Pausar => self.pausar(),
            ComandoReproductor::Detener => self.detener(),
            ComandoReproductor::Siguiente => {
                if self.cola.vacia() {
                    return Ok(());
                }
                if let Some(siguiente) = self.cola.siguiente() {
                    self.cargar_indice(siguiente);
                } else {
                    self.detener();
                }
            }
            ComandoReproductor::Anterior => {
                if self.cola.vacia() {
                    return Ok(());
                }
                let actual = self.cola.indice;
                let es_emisora = self.estado.es_emisora();
                if let Some(objetivo) = self.cola.anterior(es_emisora, self.estado.posicion_ms) {
                    if Some(objetivo) == actual && self.estado.estado != Estado::Detenido {
                        self.estado.posicion_ms = 0;
                        self.ultima_pos_ms = 0;
                        let _ = self.mpv.buscar_absoluto(0);
                        self.publicar(true);
                    } else {
                        self.cargar_indice(objetivo);
                    }
                }
            }
            ComandoReproductor::Buscar { ms, relativo } => {
                if self.estado.es_emisora() {
                    return Ok(());
                }
                if !matches!(self.estado.estado, Estado::Reproduciendo | Estado::Pausado) {
                    return Ok(());
                }
                let duracion = self.estado.duracion_ms.max(0);
                let objetivo = if relativo {
                    (self.estado.posicion_ms + ms).clamp(0, duracion)
                } else {
                    ms.clamp(0, duracion)
                };
                let _ = self.mpv.buscar_absoluto(objetivo);
                self.estado.posicion_ms = objetivo;
                self.ultima_pos_ms = objetivo;
                self.publicar(true);
            }
            ComandoReproductor::Volumen { valor } => {
                self.estado.volumen = valor.min(100);
                let _ = self.mpv.fijar_volumen(f64::from(self.estado.volumen));
                self.persistir();
                self.publicar(true);
            }
            ComandoReproductor::AlternarSilencio => {
                self.estado.silencio = !self.estado.silencio;
                let _ = self.mpv.fijar_silencio(self.estado.silencio);
                self.persistir();
                self.publicar(true);
            }
            ComandoReproductor::AlternarAleatorio => {
                self.cola.alternar_aleatorio();
                self.estado.aleatorio = self.cola.aleatorio;
                self.persistir();
                self.publicar(true);
            }
            ComandoReproductor::CiclarRepeticion => {
                self.cola.ciclar_repeticion();
                self.estado.repeticion = self.cola.repeticion;
                self.persistir();
                self.publicar(true);
            }
            ComandoReproductor::FijarRepeticion { modo } => {
                self.cola.fijar_repeticion(modo);
                self.estado.repeticion = modo;
                self.persistir();
                self.publicar(true);
            }
            ComandoReproductor::Apagar => {}
        }
        Ok(())
    }

    fn procesar_eventos(&mut self) {
        while let Some(evento) = self.mpv.esperar_evento(0.0) {
            match evento {
                EventoMpv::Propiedad { nombre, valor } => self.propiedad(&nombre, valor),
                EventoMpv::Cargado => self.manejar_cargado(),
                EventoMpv::FinDePista { razon } => self.manejar_fin(razon),
                EventoMpv::Fallo(mensaje) => {
                    warn!("fallo de reproducción: {mensaje}");
                    self.ultimo_error_stream = Some(mensaje);
                    self.manejar_fin(RazonFin::Error);
                }
                EventoMpv::Apagado => {}
            }
        }
    }

    fn propiedad(&mut self, nombre: &str, valor: Valor) {
        match (nombre, valor) {
            ("time-pos", Valor::Flotante(posicion)) => {
                if self.estado.es_emisora() {
                    return;
                }
                let ms = (posicion * 1000.0).max(0.0) as i64;
                self.actualizar_posicion(ms);
            }
            ("duration", Valor::Flotante(duracion)) if duracion > 0.0 => {
                if self.estado.es_emisora() {
                    return;
                }
                self.estado.duracion_ms = (duracion * 1000.0) as i64;
            }
            ("pause", Valor::Bandera(pausa)) => {
                if pausa && self.estado.estado == Estado::Reproduciendo {
                    self.estado.estado = Estado::Pausado;
                    if self.estado.es_emisora() && self.pausado_desde.is_none() {
                        self.pausado_desde = Some(Instant::now());
                    }
                    self.publicar(true);
                } else if !pausa
                    && self.estado.estado == Estado::Pausado
                    && self.estado.elemento.is_some()
                {
                    self.estado.estado = Estado::Reproduciendo;
                    self.pausado_desde = None;
                    self.publicar(true);
                }
            }
            ("volume", valor) => {
                let entero = match valor {
                    Valor::Flotante(valor) => valor.round() as i64,
                    Valor::Entero(valor) => valor,
                    _ => return,
                };
                self.estado.volumen = entero.clamp(0, 100) as u8;
            }
            ("mute", Valor::Bandera(silencio)) => {
                self.estado.silencio = silencio;
            }
            ("paused-for-cache", Valor::Bandera(ocupado)) => {
                self.pausa_de_cache(ocupado);
            }
            ("cache-buffering-state", valor) => {
                self.buffering_actual = valor.numero().map(|valor| valor.round() as i64);
                self.actualizar_buffering();
            }
            ("demuxer-cache-duration", valor) => {
                if let Some(segundos) = valor.numero()
                    && self.estado.es_emisora()
                {
                    self.cache_segundos = segundos as f32;
                    self.estado.cache_segundos = self.cache_segundos;
                }
            }
            ("metadata/by-key/icy-title", Valor::Texto(titulo)) => {
                self.procesar_titulo_icy(&titulo);
            }
            ("audio-codec-name", Valor::Texto(codec)) => {
                self.fijar_codec_stream(Some(codec), None);
            }
            ("audio-bitrate", valor) => {
                if let Some(bitrate) = valor.numero() {
                    self.fijar_codec_stream(None, Some(bitrate));
                }
            }
            _ => {}
        }
    }

    fn manejar_cargado(&mut self) {
        let Some(elemento) = self.cola.actual().map(|item| item.elemento.clone()) else {
            return;
        };
        match elemento {
            ElementoCola::Pista(pista) => {
                let restauracion = self.carga_en_pausa;
                match consultas::historial::registrar_inicio(&self.conn, pista.id) {
                    Ok((id, iniciado_en)) => {
                        self.historial_id = Some(id);
                        self.historial_inicio = Some(iniciado_en);
                    }
                    Err(error) => {
                        warn!("no se pudo registrar el historial: {error:#}");
                        self.historial_id = None;
                        self.historial_inicio = None;
                    }
                }
                if !restauracion && regla::elegible(pista.duracion_ms) {
                    let _ = self
                        .tx_scrobbling
                        .send(ComandoScrobbling::NowPlaying(pista.clone()));
                }
                self.tiempo_reproducido_ms = 0;
                self.ultima_pos_ms = 0;
                self.completada = false;
                self.fallos_seguidos = 0;
                self.estado.elemento = Some(ElementoCola::Pista(pista.clone()));
                self.estado.stream = None;
                self.estado.titulo_icy = None;
                self.estado.duracion_ms = pista.duracion_ms;
                if self.posicion_restauracion_ms > 0 {
                    let objetivo = self
                        .posicion_restauracion_ms
                        .clamp(0, self.estado.duracion_ms.max(0));
                    let _ = self.mpv.buscar_absoluto(objetivo);
                    self.estado.posicion_ms = objetivo;
                    self.ultima_pos_ms = objetivo;
                    self.posicion_restauracion_ms = 0;
                }
                let pausado = self.mpv.bandera("pause").unwrap_or(self.carga_en_pausa);
                self.estado.estado = if pausado {
                    Estado::Pausado
                } else {
                    Estado::Reproduciendo
                };
                self.precargar_siguiente();
            }
            ElementoCola::Emisora(emisora) => {
                self.estado.elemento = Some(ElementoCola::Emisora(emisora));
                self.estado.estado = if self.mpv.bandera("pause").unwrap_or(false) {
                    Estado::Pausado
                } else {
                    Estado::Reproduciendo
                };
                self.estado.stream = Some(EstadoStream::Almacenando { segundos: 0.0 });
                self.conexion_inicio = Some(Instant::now());
            }
        }
        self.publicar(true);
    }

    fn manejar_fin(&mut self, razon: RazonFin) {
        if self.estado.es_emisora() {
            match razon {
                RazonFin::Detenida | RazonFin::Otra => {}
                RazonFin::Error | RazonFin::Eof => self.reconectar(),
            }
            return;
        }
        match razon {
            RazonFin::Detenida | RazonFin::Otra => {}
            RazonFin::Error => {
                let titulo = self
                    .estado
                    .pista_actual()
                    .map(|pista| pista.titulo.clone())
                    .unwrap_or_default();
                self.fallos_seguidos += 1;
                let _ = self.tx_app.send(AppEvento::Notificacion(
                    NivelAviso::Error,
                    format!("No se pudo reproducir {titulo}"),
                ));
                if self.fallos_seguidos >= MAX_FALLOS_SEGUIDOS {
                    self.detener();
                } else if let Some(siguiente) = self.cola.siguiente() {
                    self.cargar_indice(siguiente);
                } else {
                    self.detener();
                }
            }
            RazonFin::Eof => {
                if let Some(precargada) = self.precargada.take() {
                    self.cola.indice = Some(precargada);
                    self.estado.estado = Estado::Cargando;
                    self.persistir();
                    self.publicar(true);
                } else if self.cola.repeticion == Repeticion::Una {
                    let indice = self.cola.indice.unwrap_or(0);
                    self.cargar_indice(indice);
                } else if let Some(siguiente) = self.cola.siguiente() {
                    self.cargar_indice(siguiente);
                } else {
                    self.detener();
                }
            }
        }
    }

    fn tick(&mut self) {
        if self.ultimo_tick.elapsed() < INTERVALO_PUBLICACION {
            return;
        }
        let ahora = Instant::now();
        let delta = ahora.saturating_duration_since(self.ultimo_tick);
        self.ultimo_tick = ahora;
        if self.estado.es_emisora() {
            self.tick_radio(ahora, delta);
            return;
        }
        if !matches!(self.estado.estado, Estado::Reproduciendo | Estado::Pausado) {
            return;
        }
        if let Some(posicion) = self.mpv.numero("time-pos") {
            self.actualizar_posicion((posicion * 1000.0).max(0.0) as i64);
        }
        if let Some(duracion) = self.mpv.numero("duration")
            && duracion > 0.0
        {
            self.estado.duracion_ms = (duracion * 1000.0) as i64;
        }
        self.publicar(false);
    }

    fn tick_radio(&mut self, ahora: Instant, delta: Duration) {
        if matches!(
            self.estado.stream,
            Some(EstadoStream::Conectando) | Some(EstadoStream::Almacenando { .. })
        ) && let Some(inicio) = self.conexion_inicio
            && ahora.saturating_duration_since(inicio) > self.espera_conexion
        {
            warn!("timeout de conexión del stream");
            self.ultimo_error_stream = Some("tiempo de conexión agotado".to_string());
            self.reconectar();
            return;
        }
        if matches!(self.estado.stream, Some(EstadoStream::Reconectando(_)))
            && self.reintento_en.is_some_and(|instante| ahora >= instante)
        {
            self.reintentar_stream();
            return;
        }
        if self.estado.en_directo() && self.estado.estado == Estado::Reproduciendo {
            let delta_ms = delta.as_millis().min(5_000) as i64;
            self.tiempo_icy_ms += delta_ms;
            self.estado.tiempo_escuchando_ms = self.tiempo_icy_ms;
            if !self.completada_icy
                && self.tiempo_icy_ms >= UMBRAL_SCROBBLE_ICY_MS
                && let Some(historial_id) = self.historial_id
            {
                if let Err(error) =
                    consultas::historial::marcar_completada(&self.conn, historial_id)
                {
                    warn!("no se pudo marcar el historial de radio: {error:#}");
                }
                let _ = self
                    .tx_scrobbling
                    .send(ComandoScrobbling::TituloIcyCompletado { historial_id });
                self.completada_icy = true;
            }
        }
        self.publicar(false);
    }

    fn pausa_de_cache(&mut self, ocupado: bool) {
        if !self.estado.es_emisora() {
            return;
        }
        if ocupado {
            if !matches!(self.estado.stream, Some(EstadoStream::Almacenando { .. })) {
                self.estado.stream = Some(EstadoStream::Almacenando {
                    segundos: self.cache_segundos,
                });
                self.publicar(true);
            }
        } else {
            self.actualizar_buffering();
        }
    }

    fn actualizar_buffering(&mut self) {
        if !self.estado.es_emisora() {
            return;
        }
        let listo = self
            .buffering_actual
            .map(|valor| valor >= 100)
            .unwrap_or(true);
        if listo {
            self.marcar_en_directo();
        } else if !matches!(self.estado.stream, Some(EstadoStream::Almacenando { .. })) {
            self.estado.stream = Some(EstadoStream::Almacenando {
                segundos: self.cache_segundos,
            });
            self.publicar(true);
        }
    }

    fn marcar_en_directo(&mut self) {
        if matches!(self.estado.stream, Some(EstadoStream::EnDirecto)) {
            return;
        }
        self.estado.stream = Some(EstadoStream::EnDirecto);
        self.plan_reconexion.reiniciar();
        self.conexion_inicio = None;
        self.reintento_en = None;
        self.pausado_desde = None;
        self.ultimo_error_stream = None;
        self.publicar(true);
    }

    fn reconectar(&mut self) {
        let Some(emisora) = self.estado.emisora_actual().cloned() else {
            return;
        };
        match self.plan_reconexion.registrar_fallo() {
            Some(intento) => {
                self.estado.stream = Some(EstadoStream::Reconectando(intento));
                self.estado.estado = Estado::Cargando;
                self.estado.reconexiones = self.estado.reconexiones.saturating_add(1);
                let espera = self
                    .plan_reconexion
                    .espera()
                    .unwrap_or(Duration::from_secs(30));
                self.reintento_en = Some(Instant::now() + espera);
                self.conexion_inicio = None;
                self.publicar(true);
            }
            None => {
                let mensaje = self
                    .ultimo_error_stream
                    .clone()
                    .unwrap_or_else(|| "sin conexión".to_string());
                if let Err(error) =
                    consultas::emisoras::marcar_error(&self.conn, emisora.id, &mensaje)
                {
                    warn!("no se pudo guardar el error de la emisora: {error:#}");
                }
                let _ = self.tx_app.send(AppEvento::Notificacion(
                    NivelAviso::Error,
                    format!("No se pudo conectar con {}", emisora.nombre),
                ));
                self.estado.stream = Some(EstadoStream::Rendido);
                self.publicar(true);
                if let Some(siguiente) = self.cola.siguiente()
                    && Some(siguiente) != self.cola.indice
                {
                    self.cargar_indice(siguiente);
                    return;
                }
                self.detener();
            }
        }
    }

    fn reintentar_stream(&mut self) {
        let Some(emisora) = self.estado.emisora_actual().cloned() else {
            return;
        };
        self.estado.estado = Estado::Cargando;
        self.estado.stream = Some(EstadoStream::Conectando);
        self.conexion_inicio = Some(Instant::now());
        self.reintento_en = None;
        let url = emisora.url.clone();
        let resultado = if self.modo_lista && self.mpv.entero("playlist-count").unwrap_or(0) > 1 {
            self.mpv.siguiente_lista()
        } else if self.modo_lista {
            self.mpv
                .cargar_lista(&url)
                .or_else(|_| self.mpv.cargar_stream(&url))
        } else {
            self.mpv.cargar_stream(&url)
        };
        if let Err(error) = resultado {
            warn!("reintento de stream fallido: {error:#}");
            self.ultimo_error_stream = Some(error.to_string());
            self.reconectar();
            return;
        }
        self.publicar(true);
    }

    fn abrir_stream(&mut self, url: &str) -> Result<()> {
        let extension = url
            .split(['?', '#'])
            .next()
            .unwrap_or(url)
            .rsplit('.')
            .next()
            .map(str::to_ascii_lowercase);
        self.url_stream = Some(url.to_string());
        match extension.as_deref() {
            Some("pls") | Some("m3u") => {
                self.modo_lista = true;
                if self.mpv.cargar_lista(url).is_err() {
                    self.modo_lista = false;
                    self.mpv.cargar_stream(url)?;
                }
                Ok(())
            }
            Some("m3u8") => {
                self.modo_lista = false;
                if self.mpv.cargar_stream(url).is_err() {
                    self.modo_lista = true;
                    self.mpv.cargar_lista(url)?;
                }
                Ok(())
            }
            _ => {
                self.modo_lista = false;
                self.mpv.cargar_stream(url)
            }
        }
    }

    fn conectar_emisora(&mut self, emisora: &EmisoraResumen) {
        self.estado.estado = Estado::Cargando;
        self.estado.elemento = Some(ElementoCola::Emisora(emisora.clone()));
        self.estado.posicion_ms = 0;
        self.estado.duracion_ms = 0;
        self.estado.titulo_icy = None;
        self.estado.tiempo_escuchando_ms = 0;
        self.estado.codec = emisora.codec.clone();
        self.estado.bitrate_kbps = emisora.bitrate_kbps;
        self.estado.stream = Some(EstadoStream::Conectando);
        self.historial_id = None;
        self.historial_inicio = None;
        self.tiempo_icy_ms = 0;
        self.completada_icy = true;
        self.ultimo_titulo_icy = None;
        self.plan_reconexion = PlanReconexion::nuevo(1);
        self.conexion_inicio = Some(Instant::now());
        self.reintento_en = None;
        self.pausado_desde = None;
        self.precargada = None;
        self.fallos_seguidos = 0;
        self.ultimo_error_stream = None;
        self.cache_segundos = 0.0;
        self.estado.cache_segundos = 0.0;
        self.estado.reconexiones = 0;
        self.buffering_actual = None;
        let _ = self.mpv.pausar(false);
        let resultado = self.abrir_stream(&emisora.url);
        match resultado {
            Ok(()) => {
                if let Err(error) = consultas::emisoras::marcar_reproducida(&self.conn, emisora.id)
                {
                    warn!("no se pudo marcar la emisora como reproducida: {error:#}");
                }
                self.persistir();
                self.publicar(true);
            }
            Err(error) => {
                warn!("no se pudo abrir el stream: {error:#}");
                self.ultimo_error_stream = Some(error.to_string());
                self.reconectar();
            }
        }
    }

    fn procesar_titulo_icy(&mut self, bruto: &str) {
        let Some(emisora) = self.estado.emisora_actual().cloned() else {
            return;
        };
        let Some(parsed) = icy::parsear(bruto, &emisora.nombre) else {
            return;
        };
        let completo = match &parsed.artista {
            Some(artista) => format!("{artista} - {}", parsed.titulo),
            None => parsed.titulo.clone(),
        };
        if self.ultimo_titulo_icy.as_deref() == Some(completo.as_str()) {
            return;
        }
        self.ultimo_titulo_icy = Some(completo.clone());
        self.estado.titulo_icy = Some(completo.clone());
        if let Err(error) =
            consultas::titulos_emisora::insertar_y_podar(&self.conn, emisora.id, &completo)
        {
            warn!("no se pudo guardar el título ICY: {error:#}");
        }
        self.tiempo_icy_ms = 0;
        self.estado.tiempo_escuchando_ms = 0;
        match parsed.artista {
            Some(artista) => {
                match consultas::historial::registrar_icy(&self.conn, emisora.id, &completo) {
                    Ok((id, instante)) => {
                        self.historial_id = Some(id);
                        self.historial_inicio = Some(instante.clone());
                        self.completada_icy = false;
                        let _ = self.tx_scrobbling.send(ComandoScrobbling::TituloIcy {
                            emisora_id: emisora.id,
                            emisora_nombre: emisora.nombre.clone(),
                            artista,
                            titulo: parsed.titulo,
                            historial_id: id,
                            instante,
                        });
                    }
                    Err(error) => {
                        warn!("no se pudo registrar el título de radio: {error:#}");
                        self.historial_id = None;
                        self.historial_inicio = None;
                        self.completada_icy = true;
                    }
                }
            }
            None => {
                self.historial_id = None;
                self.historial_inicio = None;
                self.completada_icy = true;
            }
        }
        self.publicar(true);
    }

    fn fijar_codec_stream(&mut self, codec: Option<String>, bitrate: Option<f64>) {
        if !self.estado.es_emisora() {
            return;
        }
        let codec = codec.filter(|valor| !valor.trim().is_empty());
        let bitrate = bitrate
            .filter(|valor| *valor > 0.0)
            .map(|valor| valor.round() as i64);
        if self.estado.codec.is_none() {
            self.estado.codec = codec.clone();
        }
        if self.estado.bitrate_kbps.is_none() {
            self.estado.bitrate_kbps = bitrate;
        }
        if let Some(emisora) = self.estado.emisora_actual()
            && let Err(error) =
                consultas::emisoras::fijar_codec(&self.conn, emisora.id, codec.as_deref(), bitrate)
        {
            warn!("no se pudo fijar el codec de la emisora: {error:#}");
        }
    }

    fn actualizar_posicion(&mut self, ms: i64) {
        let delta = ms - self.ultima_pos_ms;
        if self.estado.estado == Estado::Reproduciendo && delta > 0 && delta < UMBRAL_SALTO_MS {
            self.tiempo_reproducido_ms += delta;
        }
        self.ultima_pos_ms = ms;
        self.estado.posicion_ms = ms;
        let duracion = self.estado.duracion_ms;
        if !self.completada
            && duracion > 0
            && self.tiempo_reproducido_ms >= regla::umbral_ms(duracion)
        {
            if let Some(historial_id) = self.historial_id {
                if let Err(error) =
                    consultas::historial::marcar_completada(&self.conn, historial_id)
                {
                    warn!("no se pudo marcar el historial como completado: {error:#}");
                }
                if regla::elegible(duracion) {
                    let _ = self.tx_scrobbling.send(ComandoScrobbling::Completada {
                        historial_id,
                        pista: self.estado.pista_actual().cloned().unwrap_or_default(),
                        reproducido_en: self.historial_inicio.clone().unwrap_or_default(),
                    });
                }
            }
            self.completada = true;
        }
    }

    fn cargar_indice(&mut self, indice: usize) {
        let Some(item) = self.cola.items.get(indice).cloned() else {
            return;
        };
        self.cola.indice = Some(indice);
        self.estado.estado = Estado::Cargando;
        self.estado.elemento = Some(item.elemento.clone());
        self.estado.posicion_ms = 0;
        self.historial_id = None;
        self.historial_inicio = None;
        self.tiempo_reproducido_ms = 0;
        self.ultima_pos_ms = 0;
        self.completada = false;
        self.carga_en_pausa = false;
        self.precargada = None;
        self.fallos_seguidos = 0;
        match item.elemento {
            ElementoCola::Pista(pista) => {
                self.estado.duracion_ms = pista.duracion_ms;
                self.estado.stream = None;
                self.estado.titulo_icy = None;
                self.estado.tiempo_escuchando_ms = 0;
                self.estado.codec = None;
                self.estado.bitrate_kbps = None;
                let _ = self.mpv.pausar(false);
                if let Err(error) = self.mpv.cargar(&pista.ruta) {
                    warn!("no se pudo cargar {}: {error:#}", pista.ruta);
                    self.fallos_seguidos += 1;
                    let _ = self.tx_app.send(AppEvento::Notificacion(
                        NivelAviso::Error,
                        format!("No se pudo reproducir {}", pista.ruta),
                    ));
                    if self.fallos_seguidos >= MAX_FALLOS_SEGUIDOS {
                        self.detener();
                    }
                    return;
                }
                self.persistir();
                self.publicar(true);
            }
            ElementoCola::Emisora(emisora) => {
                self.conectar_emisora(&emisora);
            }
        }
    }

    fn precargar_siguiente(&mut self) {
        self.precargada = None;
        if self.cola.repeticion == Repeticion::Una {
            return;
        }
        let actual_es_emisora = self
            .cola
            .actual()
            .map(|item| item.elemento.es_emisora())
            .unwrap_or(false);
        if actual_es_emisora {
            return;
        }
        if let Some(siguiente) = self.cola.siguiente()
            && let Some(item) = self.cola.items.get(siguiente)
        {
            let ElementoCola::Pista(pista) = &item.elemento else {
                return;
            };
            if self.mpv.anexar(&pista.ruta).is_ok() {
                self.precargada = Some(siguiente);
            }
        }
    }

    fn pausar(&mut self) {
        if self.estado.estado != Estado::Reproduciendo {
            return;
        }
        if self.estado.es_emisora() && !self.estado.en_directo() {
            return;
        }
        let _ = self.mpv.pausar(true);
        self.estado.estado = Estado::Pausado;
        if self.estado.es_emisora() {
            self.pausado_desde = Some(Instant::now());
        }
        self.persistir();
        self.publicar(true);
    }

    fn reanudar(&mut self) {
        match self.estado.estado {
            Estado::Pausado => {
                if self.estado.es_emisora() {
                    let recargar = self
                        .pausado_desde
                        .is_some_and(|instante| instante.elapsed() > PAUSA_MAXIMA_STREAM);
                    if recargar {
                        if let Some(emisora) = self.estado.emisora_actual().cloned() {
                            self.conectar_emisora(&emisora);
                        }
                        return;
                    }
                }
                let _ = self.mpv.pausar(false);
                self.estado.estado = Estado::Reproduciendo;
                self.pausado_desde = None;
                self.publicar(true);
            }
            Estado::Detenido if !self.cola.vacia() => {
                let indice = self.cola.indice.unwrap_or(0);
                self.cargar_indice(indice);
            }
            _ => {}
        }
    }

    fn detener(&mut self) {
        let _ = self.mpv.detener();
        self.estado.estado = Estado::Detenido;
        self.estado.elemento = None;
        self.estado.posicion_ms = 0;
        self.estado.duracion_ms = 0;
        self.estado.stream = None;
        self.estado.titulo_icy = None;
        self.estado.tiempo_escuchando_ms = 0;
        self.estado.codec = None;
        self.estado.bitrate_kbps = None;
        self.historial_id = None;
        self.historial_inicio = None;
        self.precargada = None;
        self.carga_en_pausa = false;
        self.conexion_inicio = None;
        self.reintento_en = None;
        self.url_stream = None;
        self.modo_lista = false;
        self.pausado_desde = None;
        self.ultimo_titulo_icy = None;
        self.tiempo_icy_ms = 0;
        self.completada_icy = true;
        self.persistir();
        self.publicar(true);
    }

    fn persistir(&self) {
        if let Err(error) =
            consultas::cola::guardar(&self.conn, &self.cola.persistible(), self.cola.indice)
        {
            warn!("no se pudo persistir la cola: {error:#}");
        }
        self.persistir_ajustes();
    }

    fn persistir_ajustes(&self) {
        let ajustes = [
            ("volumen", self.estado.volumen.to_string()),
            ("silencio", u8::from(self.estado.silencio).to_string()),
            ("aleatorio", u8::from(self.estado.aleatorio).to_string()),
            ("repeticion", self.estado.repeticion.como_str().to_string()),
        ];
        for (clave, valor) in ajustes {
            if let Err(error) = consultas::ajustes::escribir(&self.conn, clave, &valor) {
                warn!("no se pudo persistir el ajuste {clave}: {error:#}");
            }
        }
    }

    fn guardar_estado_final(&self) {
        if let Err(error) = consultas::ajustes::escribir(
            &self.conn,
            "cola_ms",
            &self.estado.posicion_ms.to_string(),
        ) {
            warn!("no se pudo persistir la posición: {error:#}");
        }
        self.persistir();
    }

    fn publicar(&mut self, forzar: bool) {
        if !forzar && self.ultima_publicacion.elapsed() < INTERVALO_PUBLICACION {
            return;
        }
        self.estado.cola = self.cola.elementos();
        self.estado.cola_indice = self.cola.indice;
        self.tx_estado.send_replace(self.estado.clone());
        let _ = self
            .tx_app
            .send(AppEvento::Reproductor(Box::new(self.estado.clone())));
        self.ultima_publicacion = Instant::now();
    }
}
