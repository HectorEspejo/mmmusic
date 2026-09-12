pub mod estado;
pub mod lastfm;
pub mod listenbrainz;
pub mod planificador;
pub mod regla;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rusqlite::Connection;
use tokio::sync::watch;
use tracing::{info, warn};

use self::estado::{EstadoScrobbling, EstadoServicio};
use self::lastfm::{Cancion, ClienteLastfm};
use self::listenbrainz::{ClienteListenBrainz, escucha};
use self::planificador::Clasificacion;
use crate::biblioteca::modelos::PistaResumen;
use crate::biblioteca::{bd, consultas};
use crate::config::ConfigScrobbling;
use crate::credenciales::{self, Credenciales};
use crate::eventos::{AppEvento, NivelAviso};

pub const SERVICIO_LISTENBRAINZ: &str = "listenbrainz";
pub const SERVICIO_LASTFM: &str = "lastfm";
pub const HOSTS_PERMITIDOS: [&str; 2] = [
    "https://api.listenbrainz.org",
    "https://ws.audioscrobbler.com",
];
pub const LOTE_MAXIMO: usize = 50;
pub const MARCA_AUTH: &str = "auth:";
pub const ERROR_AUTH_LISTENBRAINZ: &str =
    "ListenBrainz: token no válido — revisa credenciales.toml";
pub const ERROR_AUTH_LASTFM: &str = "Last.fm: sesión no válida — ejecuta mmmusic autorizar-lastfm";

const CICLO: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct FalloHttp {
    pub status: u16,
    pub codigo_servicio: Option<i32>,
    pub mensaje: String,
}

impl FalloHttp {
    pub fn red(mensaje: impl Into<String>) -> Self {
        Self {
            status: 0,
            codigo_servicio: None,
            mensaje: mensaje.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ComandoScrobbling {
    NowPlaying(PistaResumen),
    Completada {
        historial_id: i64,
        pista: PistaResumen,
        reproducido_en: String,
    },
    Amar(i64),
    Desamar(i64),
    EnviarAhora,
    RecargarCredenciales,
    Apagar,
}

pub struct ManejoScrobbling {
    tx: Sender<ComandoScrobbling>,
    hilo: Option<JoinHandle<()>>,
}

impl ManejoScrobbling {
    pub fn enviar(&self, comando: ComandoScrobbling) -> bool {
        self.tx.send(comando).is_ok()
    }

    pub fn emisor(&self) -> Sender<ComandoScrobbling> {
        self.tx.clone()
    }

    pub fn apagar(mut self) {
        let _ = self.tx.send(ComandoScrobbling::Apagar);
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

pub fn lanzar(
    ruta_bd: PathBuf,
    ruta_credenciales: PathBuf,
    config: ConfigScrobbling,
    credenciales: Credenciales,
    tx_app: Sender<AppEvento>,
) -> Result<(ManejoScrobbling, watch::Receiver<EstadoScrobbling>)> {
    let (tx_cmd, rx_cmd) = mpsc::channel();
    let (tx_estado, rx_estado) = watch::channel(EstadoScrobbling::default());
    let hilo = thread::Builder::new()
        .name("scrobbling".to_string())
        .spawn(move || {
            if let Err(error) = ejecutar(
                ruta_bd,
                ruta_credenciales,
                config,
                credenciales,
                tx_app,
                tx_estado,
                rx_cmd,
            ) {
                warn!("hilo de scrobbling detenido: {error:#}");
            }
        })
        .context("no se pudo lanzar el hilo de scrobbling")?;
    Ok((
        ManejoScrobbling {
            tx: tx_cmd,
            hilo: Some(hilo),
        },
        rx_estado,
    ))
}

fn ejecutar(
    ruta_bd: PathBuf,
    ruta_credenciales: PathBuf,
    config: ConfigScrobbling,
    credenciales: Credenciales,
    tx_app: Sender<AppEvento>,
    tx_estado: watch::Sender<EstadoScrobbling>,
    rx_cmd: Receiver<ComandoScrobbling>,
) -> Result<()> {
    let mut scrobbler = Scrobbler::nuevo(
        ruta_bd,
        ruta_credenciales,
        config,
        credenciales,
        tx_app,
        tx_estado,
        rx_cmd,
    )?;
    scrobbler.publicar();
    loop {
        let mut apagar = false;
        loop {
            match scrobbler.rx_cmd.try_recv() {
                Ok(comando) => {
                    if scrobbler.procesar_comando(comando)? {
                        apagar = true;
                        break;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    apagar = true;
                    break;
                }
            }
        }
        if apagar {
            break;
        }
        if let Err(error) = scrobbler.ciclo() {
            warn!("ciclo de scrobbling falló: {error:#}");
        }
        scrobbler.publicar();
        let recibido = if scrobbler.estado.pendientes() > 0 {
            scrobbler.rx_cmd.recv_timeout(CICLO)
        } else {
            scrobbler
                .rx_cmd
                .recv()
                .map_err(|_| RecvTimeoutError::Disconnected)
        };
        match recibido {
            Ok(comando) => {
                if scrobbler.procesar_comando(comando)? {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    info!("hilo de scrobbling detenido");
    Ok(())
}

struct Scrobbler {
    conn: Connection,
    ruta_credenciales: PathBuf,
    config: ConfigScrobbling,
    credenciales: Credenciales,
    lb: ClienteListenBrainz,
    lf: ClienteLastfm,
    tx_app: Sender<AppEvento>,
    tx_estado: watch::Sender<EstadoScrobbling>,
    rx_cmd: Receiver<ComandoScrobbling>,
    estado: EstadoScrobbling,
    ultimo_publicado: EstadoScrobbling,
}

impl Scrobbler {
    #[allow(clippy::too_many_arguments)]
    fn nuevo(
        ruta_bd: PathBuf,
        ruta_credenciales: PathBuf,
        config: ConfigScrobbling,
        credenciales: Credenciales,
        tx_app: Sender<AppEvento>,
        tx_estado: watch::Sender<EstadoScrobbling>,
        rx_cmd: Receiver<ComandoScrobbling>,
    ) -> Result<Self> {
        let mut conn = bd::abrir(&ruta_bd)?;
        bd::migrar(&mut conn)?;
        let mut estado = EstadoScrobbling::default();
        if let Some(texto) = consultas::ajustes::leer(&conn, "scrobbling_ultimo_error")?
            && !texto.trim().is_empty()
        {
            estado.ultimo_error = Some(texto);
        }
        let lb = ClienteListenBrainz::nuevo(credenciales.token_listenbrainz().map(str::to_string));
        let lf = ClienteLastfm::nuevo(&credenciales);
        let mut scrobbler = Self {
            conn,
            ruta_credenciales,
            config,
            credenciales,
            lb,
            lf,
            tx_app,
            tx_estado,
            rx_cmd,
            estado,
            ultimo_publicado: EstadoScrobbling::default(),
        };
        scrobbler.refrescar_estado_servicios(true)?;
        Ok(scrobbler)
    }

    fn procesar_comando(&mut self, comando: ComandoScrobbling) -> Result<bool> {
        match comando {
            ComandoScrobbling::Apagar => return Ok(true),
            ComandoScrobbling::EnviarAhora => {}
            ComandoScrobbling::RecargarCredenciales => self.recargar_credenciales()?,
            ComandoScrobbling::NowPlaying(pista) => self.now_playing(&pista),
            ComandoScrobbling::Completada {
                historial_id,
                pista,
                reproducido_en,
            } => self.completada(&pista, historial_id, &reproducido_en)?,
            ComandoScrobbling::Amar(pista_id) => self.encolar_favorita(pista_id, true)?,
            ComandoScrobbling::Desamar(pista_id) => self.encolar_favorita(pista_id, false)?,
        }
        Ok(false)
    }

    fn recargar_credenciales(&mut self) -> Result<()> {
        let carga = credenciales::cargar(&self.ruta_credenciales)?;
        self.credenciales = carga.credenciales;
        if carga.permisos_corregidos {
            self.notificar(
                NivelAviso::Aviso,
                "Las credenciales eran legibles por otros usuarios; permisos corregidos a 600",
            );
        }
        self.refrescar_estado_servicios(true)?;
        if self.credenciales.token_listenbrainz().is_some() {
            consultas::envios::reprogramar_errores_auth(&self.conn, SERVICIO_LISTENBRAINZ)?;
            self.estado.listenbrainz.error = None;
        }
        if self.credenciales.lastfm_autorizado() {
            consultas::envios::reprogramar_errores_auth(&self.conn, SERVICIO_LASTFM)?;
            self.estado.lastfm.error = None;
            consultas::envios::limpiar_errores_auth(&self.conn, SERVICIO_LASTFM)?;
        }
        if self.estado.listenbrainz.error.is_none() && self.estado.lastfm.error.is_none() {
            self.estado.ultimo_error = None;
            consultas::ajustes::escribir(&self.conn, "scrobbling_ultimo_error", "")?;
        }
        Ok(())
    }

    fn now_playing(&mut self, pista: &PistaResumen) {
        if !self.config.now_playing || !regla::elegible(pista.duracion_ms) {
            return;
        }
        if self.estado.listenbrainz.activo
            && let Err(fallo) = self.lb.enviar_ahora(pista)
        {
            let mensaje = self.credenciales.redactar(&fallo.mensaje);
            warn!(
                servicio = SERVICIO_LISTENBRAINZ,
                "now playing falló: {mensaje}"
            );
        }
        if self.estado.lastfm.activo {
            let cancion = cancion_de(pista, None);
            if let Err(fallo) = self.lf.enviar_ahora(&cancion) {
                let mensaje = self.credenciales.redactar(&fallo.mensaje);
                warn!(servicio = SERVICIO_LASTFM, "now playing falló: {mensaje}");
            }
        }
    }

    fn completada(
        &mut self,
        pista: &PistaResumen,
        historial_id: i64,
        reproducido_en: &str,
    ) -> Result<()> {
        if !regla::elegible(pista.duracion_ms) {
            return Ok(());
        }
        if self.estado.listenbrainz.activo {
            consultas::envios::encolar(
                &self.conn,
                SERVICIO_LISTENBRAINZ,
                "scrobble",
                pista.id,
                Some(historial_id),
                Some(reproducido_en),
            )?;
        }
        if self.estado.lastfm.activo {
            consultas::envios::encolar(
                &self.conn,
                SERVICIO_LASTFM,
                "scrobble",
                pista.id,
                Some(historial_id),
                Some(reproducido_en),
            )?;
        }
        self.actualizar_pendientes()?;
        Ok(())
    }

    fn encolar_favorita(&mut self, pista_id: i64, amar: bool) -> Result<()> {
        if !self.estado.lastfm.activo {
            return Ok(());
        }
        consultas::envios::encolar(
            &self.conn,
            SERVICIO_LASTFM,
            if amar { "love" } else { "unlove" },
            pista_id,
            None,
            None,
        )?;
        self.actualizar_pendientes()?;
        Ok(())
    }

    fn ciclo(&mut self) -> Result<()> {
        if self.estado.listenbrainz.activo {
            self.ciclo_listenbrainz()?;
        }
        if self.estado.lastfm.activo {
            self.ciclo_lastfm()?;
        }
        self.actualizar_pendientes()?;
        Ok(())
    }

    fn ciclo_lastfm(&mut self) -> Result<()> {
        let pendientes = consultas::envios::pendientes(&self.conn, SERVICIO_LASTFM, LOTE_MAXIMO)?;
        if pendientes.is_empty() {
            return Ok(());
        }
        let (favoritas, scrobbles): (Vec<_>, Vec<_>) = pendientes
            .iter()
            .partition(|envio| envio.tipo != "scrobble");
        for envio in favoritas {
            let cancion = cancion_de_envio(envio, None);
            let resultado = if envio.tipo == "love" {
                self.lf.amar(&cancion)
            } else {
                self.lf.desamar(&cancion)
            };
            match resultado {
                Ok(()) => {
                    consultas::envios::marcar_enviado(&self.conn, envio.id)?;
                    self.estado.lastfm.ultimo_envio = Some(Instant::now());
                    self.estado.lastfm.error = None;
                    self.limpiar_ultimo_error()?;
                }
                Err(fallo) => {
                    let clasificacion =
                        planificador::clasificar(fallo.status, fallo.codigo_servicio);
                    match clasificacion {
                        Clasificacion::Autenticacion => {
                            self.marcar_auth(
                                SERVICIO_LASTFM,
                                ERROR_AUTH_LASTFM,
                                std::slice::from_ref(envio),
                            )?;
                            return Ok(());
                        }
                        Clasificacion::Descartar => {
                            consultas::envios::descartar(
                                &self.conn,
                                envio.id,
                                &self.credenciales.redactar(&fallo.mensaje),
                            )?;
                        }
                        _ => self.reintentar_envio(envio, &fallo.mensaje)?,
                    }
                }
            }
        }

        let ahora = bd::ahora_unix();
        let mut canciones = Vec::with_capacity(scrobbles.len());
        let mut validos = Vec::with_capacity(scrobbles.len());
        for envio in &scrobbles {
            let Some(unix) = envio.reproducido_en.as_deref().and_then(bd::unix_desde_iso) else {
                consultas::envios::descartar(&self.conn, envio.id, "marca de tiempo inválida")?;
                continue;
            };
            if planificador::es_demasiado_antiguo(unix, ahora) {
                consultas::envios::descartar(
                    &self.conn,
                    envio.id,
                    "más de 14 días: Last.fm lo rechazaría",
                )?;
                continue;
            }
            if planificador::es_futuro(unix, ahora) {
                consultas::envios::descartar(&self.conn, envio.id, "timestamp futuro")?;
                continue;
            }
            canciones.push(cancion_de_envio(envio, Some(unix)));
            validos.push((*envio).clone());
        }
        if canciones.is_empty() {
            return Ok(());
        }
        match self.lf.enviar_scrobbles(&canciones) {
            Ok(resultado) => {
                let ignorados: std::collections::HashMap<usize, &String> = resultado
                    .ignorados
                    .iter()
                    .map(|(indice, motivo)| (*indice, motivo))
                    .collect();
                for (indice, envio) in validos.iter().enumerate() {
                    if let Some(motivo) = ignorados.get(&indice) {
                        consultas::envios::descartar(
                            &self.conn,
                            envio.id,
                            &self.credenciales.redactar(motivo),
                        )?;
                    } else {
                        consultas::envios::marcar_enviado(&self.conn, envio.id)?;
                    }
                }
                self.estado.lastfm.ultimo_envio = Some(Instant::now());
                self.estado.lastfm.error = None;
                self.limpiar_ultimo_error()?;
            }
            Err(fallo) => {
                let clasificacion = planificador::clasificar(fallo.status, fallo.codigo_servicio);
                match clasificacion {
                    Clasificacion::Autenticacion => {
                        self.marcar_auth(SERVICIO_LASTFM, ERROR_AUTH_LASTFM, &validos)?;
                    }
                    Clasificacion::Descartar => {
                        for envio in &validos {
                            consultas::envios::descartar(
                                &self.conn,
                                envio.id,
                                &self.credenciales.redactar(&fallo.mensaje),
                            )?;
                        }
                    }
                    _ => {
                        for envio in &validos {
                            self.reintentar_envio(envio, &fallo.mensaje)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn ciclo_listenbrainz(&mut self) -> Result<()> {
        let pendientes =
            consultas::envios::pendientes(&self.conn, SERVICIO_LISTENBRAINZ, LOTE_MAXIMO)?;
        if pendientes.is_empty() {
            return Ok(());
        }
        let mut escuchas = Vec::with_capacity(pendientes.len());
        let mut validos = Vec::with_capacity(pendientes.len());
        for envio in &pendientes {
            let Some(unix) = envio.reproducido_en.as_deref().and_then(bd::unix_desde_iso) else {
                consultas::envios::descartar(&self.conn, envio.id, "marca de tiempo inválida")?;
                continue;
            };
            escuchas.push(escucha(
                &envio.artista,
                &envio.titulo,
                &envio.album,
                envio.duracion_ms,
                Some(unix),
            ));
            validos.push(envio.clone());
        }
        if escuchas.is_empty() {
            return Ok(());
        }
        match self.lb.enviar_lote(escuchas) {
            Ok(()) => {
                let ids: Vec<i64> = validos.iter().map(|envio| envio.id).collect();
                consultas::envios::marcar_enviados(&self.conn, &ids)?;
                self.estado.listenbrainz.ultimo_envio = Some(Instant::now());
                self.estado.listenbrainz.error = None;
                self.limpiar_ultimo_error()?;
            }
            Err(fallo) if fallo.status == 400 => {
                self.enviar_elemento_a_elemento(&validos)?;
            }
            Err(fallo) => {
                let clasificacion = planificador::clasificar(fallo.status, fallo.codigo_servicio);
                match clasificacion {
                    Clasificacion::Autenticacion => {
                        self.marcar_auth(SERVICIO_LISTENBRAINZ, ERROR_AUTH_LISTENBRAINZ, &validos)?;
                    }
                    Clasificacion::Descartar => {
                        for envio in &validos {
                            consultas::envios::descartar(
                                &self.conn,
                                envio.id,
                                &self.credenciales.redactar(&fallo.mensaje),
                            )?;
                        }
                    }
                    _ => {
                        for envio in &validos {
                            self.reintentar_envio(envio, &fallo.mensaje)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn enviar_elemento_a_elemento(
        &mut self,
        pendientes: &[consultas::envios::EnvioPendiente],
    ) -> Result<()> {
        for envio in pendientes {
            let Some(unix) = envio.reproducido_en.as_deref().and_then(bd::unix_desde_iso) else {
                continue;
            };
            let escucha = escucha(
                &envio.artista,
                &envio.titulo,
                &envio.album,
                envio.duracion_ms,
                Some(unix),
            );
            match self.lb.enviar_lote(vec![escucha]) {
                Ok(()) => {
                    consultas::envios::marcar_enviado(&self.conn, envio.id)?;
                    self.estado.listenbrainz.ultimo_envio = Some(Instant::now());
                    self.limpiar_ultimo_error()?;
                }
                Err(fallo) => match planificador::clasificar(fallo.status, fallo.codigo_servicio) {
                    Clasificacion::Autenticacion => {
                        self.marcar_auth(
                            SERVICIO_LISTENBRAINZ,
                            ERROR_AUTH_LISTENBRAINZ,
                            std::slice::from_ref(envio),
                        )?;
                        break;
                    }
                    Clasificacion::Descartar => {
                        consultas::envios::descartar(
                            &self.conn,
                            envio.id,
                            &self.credenciales.redactar(&fallo.mensaje),
                        )?;
                    }
                    _ => self.reintentar_envio(envio, &fallo.mensaje)?,
                },
            }
        }
        Ok(())
    }

    fn reintentar_envio(
        &mut self,
        envio: &consultas::envios::EnvioPendiente,
        mensaje: &str,
    ) -> Result<()> {
        let mensaje = self.credenciales.redactar(mensaje);
        let intentos_previos = envio.intentos.max(0) as u32;
        let nuevos = intentos_previos + 1;
        if planificador::descartar_por_intentos(nuevos) {
            consultas::envios::descartar(&self.conn, envio.id, "agotados los reintentos")?;
        } else {
            consultas::envios::marcar_error(
                &self.conn,
                envio.id,
                &mensaje,
                &planificador::proximo_reintento(intentos_previos),
                i64::from(nuevos),
            )?;
        }
        Ok(())
    }

    fn marcar_auth(
        &mut self,
        servicio: &str,
        mensaje: &str,
        pendientes: &[consultas::envios::EnvioPendiente],
    ) -> Result<()> {
        let mensaje = self.credenciales.redactar(mensaje);
        for envio in pendientes {
            consultas::envios::marcar_error(
                &self.conn,
                envio.id,
                &format!("{MARCA_AUTH}{mensaje}"),
                &planificador::proximo_auth(),
                envio.intentos + 1,
            )?;
        }
        consultas::ajustes::escribir(&self.conn, "scrobbling_ultimo_error", &mensaje)?;
        self.estado.ultimo_error = Some(mensaje.clone());
        let destino = if servicio == SERVICIO_LISTENBRAINZ {
            &mut self.estado.listenbrainz
        } else {
            &mut self.estado.lastfm
        };
        let era_nuevo = destino.error.as_deref() != Some(mensaje.as_str());
        destino.error = Some(mensaje.clone());
        if era_nuevo {
            self.notificar(NivelAviso::Aviso, mensaje);
        }
        Ok(())
    }

    fn limpiar_ultimo_error(&mut self) -> Result<()> {
        if self.estado.ultimo_error.is_some() {
            self.estado.ultimo_error = None;
            consultas::ajustes::escribir(&self.conn, "scrobbling_ultimo_error", "")?;
        }
        Ok(())
    }

    fn actualizar_pendientes(&mut self) -> Result<()> {
        self.estado.listenbrainz.pendientes =
            consultas::envios::contar_pendientes(&self.conn, SERVICIO_LISTENBRAINZ)?;
        self.estado.lastfm.pendientes =
            consultas::envios::contar_pendientes(&self.conn, SERVICIO_LASTFM)?;
        Ok(())
    }

    fn refrescar_estado_servicios(&mut self, avisar: bool) -> Result<()> {
        let token = self.credenciales.token_listenbrainz().map(str::to_string);
        self.lb.actualizar_token(token.clone());
        self.estado.listenbrainz = EstadoServicio {
            activo: self.config.listenbrainz && token.is_some(),
            pendientes: consultas::envios::contar_pendientes(&self.conn, SERVICIO_LISTENBRAINZ)?,
            ultimo_envio: self.estado.listenbrainz.ultimo_envio,
            error: self.estado.listenbrainz.error.clone(),
        };
        if avisar && self.config.listenbrainz && token.is_none() {
            self.notificar(
                NivelAviso::Aviso,
                "ListenBrainz activo sin token en credenciales.toml",
            );
        }
        let lastfm_autorizado = self.credenciales.lastfm_autorizado();
        self.lf = ClienteLastfm::nuevo(&self.credenciales);
        self.estado.lastfm = EstadoServicio {
            activo: self.config.lastfm && lastfm_autorizado,
            pendientes: consultas::envios::contar_pendientes(&self.conn, SERVICIO_LASTFM)?,
            ultimo_envio: self.estado.lastfm.ultimo_envio,
            error: self.estado.lastfm.error.clone(),
        };
        if avisar && self.config.lastfm && !lastfm_autorizado {
            self.notificar(
                NivelAviso::Aviso,
                "Last.fm activo sin autorizar — ejecuta mmmusic autorizar-lastfm",
            );
        }
        Ok(())
    }

    fn publicar(&mut self) {
        if self.estado == self.ultimo_publicado {
            return;
        }
        self.tx_estado.send_replace(self.estado.clone());
        let _ = self.tx_app.send(AppEvento::Scrobbling(self.estado.clone()));
        self.ultimo_publicado = self.estado.clone();
    }

    fn notificar(&self, nivel: NivelAviso, mensaje: impl Into<String>) {
        let _ = self
            .tx_app
            .send(AppEvento::Notificacion(nivel, mensaje.into()));
    }
}

pub fn agente_http() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .user_agent(concat!("mmmusic/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

pub fn url_permitida(url: &str) -> bool {
    HOSTS_PERMITIDOS
        .iter()
        .any(|host| url.starts_with(&format!("{host}/")))
}

fn cancion_de(pista: &PistaResumen, unix: Option<i64>) -> Cancion<'_> {
    Cancion {
        artista: &pista.artista,
        titulo: &pista.titulo,
        album: &pista.album,
        duracion_ms: pista.duracion_ms,
        unix,
    }
}

fn cancion_de_envio(envio: &consultas::envios::EnvioPendiente, unix: Option<i64>) -> Cancion<'_> {
    Cancion {
        artista: &envio.artista,
        titulo: &envio.titulo,
        album: &envio.album,
        duracion_ms: envio.duracion_ms,
        unix,
    }
}

pub(crate) fn interpretar(
    respuesta: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<String, FalloHttp> {
    match respuesta {
        Ok(mut respuesta) => {
            let status = respuesta.status().as_u16();
            let cuerpo = respuesta.body_mut().read_to_string().unwrap_or_default();
            if (200..300).contains(&status) {
                return Ok(cuerpo);
            }
            Err(fallo_desde_cuerpo(status, &cuerpo))
        }
        Err(error) => Err(FalloHttp::red(error.to_string())),
    }
}

fn fallo_desde_cuerpo(status: u16, cuerpo: &str) -> FalloHttp {
    let valor: Option<serde_json::Value> = serde_json::from_str(cuerpo).ok();
    let codigo_servicio = valor.as_ref().and_then(|valor| {
        valor
            .get("error")
            .and_then(serde_json::Value::as_i64)
            .or_else(|| valor.get("code").and_then(serde_json::Value::as_i64))
            .map(|codigo| codigo as i32)
    });
    let mensaje = valor
        .as_ref()
        .and_then(|valor| {
            valor
                .get("message")
                .and_then(serde_json::Value::as_str)
                .or_else(|| valor.get("error").and_then(serde_json::Value::as_str))
        })
        .map(str::to_string)
        .unwrap_or_else(|| format!("HTTP {status}"));
    FalloHttp {
        status,
        codigo_servicio,
        mensaje,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn solo_permite_los_hosts_declarados() {
        assert!(url_permitida(
            "https://api.listenbrainz.org/1/submit-listens"
        ));
        assert!(url_permitida("https://ws.audioscrobbler.com/2.0/"));
        assert!(!url_permitida("https://example.com/robo"));
        assert!(!url_permitida(
            "http://api.listenbrainz.org/1/submit-listens"
        ));
        assert!(!url_permitida("https://api.listenbrainz.org.evil.com/x"));
    }

    #[test]
    fn extrae_error_de_lastfm_y_listenbrainz() {
        let lastfm = fallo_desde_cuerpo(400, r#"{"error":4,"message":"Invalid auth token"}"#);
        assert_eq!(lastfm.codigo_servicio, Some(4));
        assert_eq!(lastfm.mensaje, "Invalid auth token");

        let lb = fallo_desde_cuerpo(400, r#"{"code":400,"error":"Bad payload"}"#);
        assert_eq!(lb.codigo_servicio, Some(400));
        assert_eq!(lb.mensaje, "Bad payload");

        let sinesquema = fallo_desde_cuerpo(500, "vaya");
        assert_eq!(sinesquema.codigo_servicio, None);
        assert_eq!(sinesquema.mensaje, "HTTP 500");
    }
}
