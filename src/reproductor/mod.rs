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
use self::estado::{Estado, EstadoReproduccion, Repeticion};
use self::mpv::{EventoMpv, RazonFin, ReproductorMpv, Valor};
use crate::biblioteca::modelos::PistaResumen;
use crate::biblioteca::{bd, consultas};
use crate::eventos::{AppEvento, NivelAviso};
use crate::scrobbling::ComandoScrobbling;
use crate::scrobbling::regla;

const TICK_REPRODUCTOR: Duration = Duration::from_millis(50);
const INTERVALO_PUBLICACION: Duration = Duration::from_millis(250);
const MAX_FALLOS_SEGUIDOS: u32 = 3;
const UMBRAL_SALTO_MS: i64 = 1500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComandoReproductor {
    ReemplazarCola { pistas: Vec<i64>, indice: usize },
    AnadirAlFinal { pistas: Vec<i64> },
    ReproducirSiguiente { pistas: Vec<i64> },
    EliminarDeCola { posicion: usize },
    MoverEnCola { de: usize, a: usize },
    VaciarCola,
    SaltarA { posicion: usize },
    AlternarPausa,
    Reanudar,
    Pausar,
    Detener,
    Siguiente,
    Anterior,
    Buscar { ms: i64, relativo: bool },
    Volumen { valor: u8 },
    AlternarSilencio,
    AlternarAleatorio,
    CiclarRepeticion,
    FijarRepeticion { modo: Repeticion },
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
    tx_app: Sender<AppEvento>,
    tx_scrobbling: Sender<ComandoScrobbling>,
) -> Result<(ManejoReproductor, watch::Receiver<EstadoReproduccion>)> {
    let mut conn = bd::abrir(&ruta_bd)?;
    bd::migrar(&mut conn)?;

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

    let mut estado = EstadoReproduccion {
        volumen,
        silencio,
        aleatorio,
        repeticion,
        cola: cola.pistas(),
        cola_indice: cola.indice,
        pista_actual: cola.actual().map(|item| item.pista.clone()),
        posicion_ms: cola_ms,
        duracion_ms: cola
            .actual()
            .map(|item| item.pista.duracion_ms)
            .unwrap_or(0),
        ..EstadoReproduccion::default()
    };
    estado.estado = Estado::Detenido;

    let (tx_estado, rx_estado) = watch::channel(estado.clone());
    let _ = tx_app.send(AppEvento::Reproductor(estado.clone()));

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
                cola_ms,
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
    ) -> Result<Self> {
        let mut conn = bd::abrir(&ruta_bd)?;
        bd::migrar(&mut conn)?;
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
            let ruta = item.pista.ruta.clone();
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
            ComandoReproductor::ReemplazarCola { pistas, indice } => {
                let resumenes = self.cargar_resumenes(&pistas);
                self.cola.reemplazar(resumenes, indice);
                self.persistir();
                if self.cola.vacia() {
                    self.detener();
                } else {
                    self.cargar_indice(self.cola.indice.unwrap_or(0));
                }
            }
            ComandoReproductor::AnadirAlFinal { pistas } => {
                let resumenes = self.cargar_resumenes(&pistas);
                let estaba_vacia = self.cola.anadir_al_final(resumenes);
                self.persistir();
                if estaba_vacia && self.estado.estado == Estado::Detenido {
                    self.cargar_indice(self.cola.indice.unwrap_or(0));
                } else {
                    self.publicar(true);
                }
            }
            ComandoReproductor::ReproducirSiguiente { pistas } => {
                let resumenes = self.cargar_resumenes(&pistas);
                let tenia_actual = self.cola.actual().is_some();
                self.cola.reproducir_a_continuacion(resumenes);
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
                if let Some(objetivo) = self.cola.anterior(self.estado.posicion_ms) {
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
                    self.manejar_fin(RazonFin::Error);
                }
                EventoMpv::Apagado => {}
            }
        }
    }

    fn propiedad(&mut self, nombre: &str, valor: Valor) {
        match (nombre, valor) {
            ("time-pos", Valor::Flotante(posicion)) => {
                let ms = (posicion * 1000.0).max(0.0) as i64;
                self.actualizar_posicion(ms);
            }
            ("duration", Valor::Flotante(duracion)) if duracion > 0.0 => {
                self.estado.duracion_ms = (duracion * 1000.0) as i64;
            }
            ("pause", Valor::Bandera(pausa)) => {
                if pausa && self.estado.estado == Estado::Reproduciendo {
                    self.estado.estado = Estado::Pausado;
                    self.publicar(true);
                } else if !pausa
                    && self.estado.estado == Estado::Pausado
                    && self.estado.pista_actual.is_some()
                {
                    self.estado.estado = Estado::Reproduciendo;
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
            _ => {}
        }
    }

    fn manejar_cargado(&mut self) {
        let Some(pista) = self.cola.actual().map(|item| item.pista.clone()) else {
            return;
        };
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
        self.estado.pista_actual = Some(pista);
        self.estado.duracion_ms = self
            .estado
            .pista_actual
            .as_ref()
            .map(|pista| pista.duracion_ms)
            .unwrap_or(0);
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
        self.publicar(true);
    }

    fn manejar_fin(&mut self, razon: RazonFin) {
        match razon {
            RazonFin::Detenida | RazonFin::Otra => {}
            RazonFin::Error => {
                let titulo = self
                    .estado
                    .pista_actual
                    .as_ref()
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
        self.ultimo_tick = Instant::now();
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
                        pista: self.estado.pista_actual.clone().unwrap_or_default(),
                        reproducido_en: self.historial_inicio.clone().unwrap_or_default(),
                    });
                }
            }
            self.completada = true;
        }
    }

    fn cargar_indice(&mut self, indice: usize) {
        let Some(item) = self.cola.items.get(indice) else {
            return;
        };
        let ruta = item.pista.ruta.clone();
        let pista = item.pista.clone();
        self.cola.indice = Some(indice);
        self.estado.estado = Estado::Cargando;
        self.estado.pista_actual = Some(pista);
        self.estado.posicion_ms = 0;
        self.estado.duracion_ms = self
            .estado
            .pista_actual
            .as_ref()
            .map(|pista| pista.duracion_ms)
            .unwrap_or(0);
        self.historial_id = None;
        self.historial_inicio = None;
        self.tiempo_reproducido_ms = 0;
        self.ultima_pos_ms = 0;
        self.completada = false;
        self.carga_en_pausa = false;
        self.precargada = None;
        let _ = self.mpv.pausar(false);
        if let Err(error) = self.mpv.cargar(&ruta) {
            warn!("no se pudo cargar {ruta}: {error:#}");
            self.fallos_seguidos += 1;
            let _ = self.tx_app.send(AppEvento::Notificacion(
                NivelAviso::Error,
                format!("No se pudo reproducir {ruta}"),
            ));
            if self.fallos_seguidos >= MAX_FALLOS_SEGUIDOS {
                self.detener();
            }
            return;
        }
        self.persistir();
        self.publicar(true);
    }

    fn precargar_siguiente(&mut self) {
        self.precargada = None;
        if self.cola.repeticion == Repeticion::Una {
            return;
        }
        if let Some(siguiente) = self.cola.siguiente()
            && let Some(item) = self.cola.items.get(siguiente)
        {
            let ruta = item.pista.ruta.clone();
            if self.mpv.anexar(&ruta).is_ok() {
                self.precargada = Some(siguiente);
            }
        }
    }

    fn pausar(&mut self) {
        if self.estado.estado != Estado::Reproduciendo {
            return;
        }
        let _ = self.mpv.pausar(true);
        self.estado.estado = Estado::Pausado;
        self.persistir();
        self.publicar(true);
    }

    fn reanudar(&mut self) {
        match self.estado.estado {
            Estado::Pausado => {
                let _ = self.mpv.pausar(false);
                self.estado.estado = Estado::Reproduciendo;
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
        self.estado.pista_actual = None;
        self.estado.posicion_ms = 0;
        self.estado.duracion_ms = 0;
        self.historial_id = None;
        self.historial_inicio = None;
        self.precargada = None;
        self.carga_en_pausa = false;
        self.persistir();
        self.publicar(true);
    }

    fn cargar_resumenes(&self, ids: &[i64]) -> Vec<PistaResumen> {
        match consultas::pistas_resumen_por_ids(&self.conn, ids) {
            Ok(pistas) => pistas,
            Err(error) => {
                warn!("no se pudieron cargar las pistas: {error:#}");
                Vec::new()
            }
        }
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
        self.estado.cola = self.cola.pistas();
        self.estado.cola_indice = self.cola.indice;
        self.tx_estado.send_replace(self.estado.clone());
        let _ = self
            .tx_app
            .send(AppEvento::Reproductor(self.estado.clone()));
        self.ultima_publicacion = Instant::now();
    }
}
