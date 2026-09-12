pub mod estado;
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
        let mut scrobbler = Self {
            conn,
            ruta_credenciales,
            config,
            credenciales,
            lb,
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
        self.actualizar_pendientes()?;
        Ok(())
    }

    fn ciclo(&mut self) -> Result<()> {
        if self.estado.listenbrainz.activo {
            self.ciclo_listenbrainz()?;
        }
        self.actualizar_pendientes()?;
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
        let lastfm = self.estado.lastfm.clone();
        self.estado.lastfm = EstadoServicio {
            activo: false,
            ..lastfm
        };
        self.estado.lastfm.pendientes =
            consultas::envios::contar_pendientes(&self.conn, SERVICIO_LASTFM)?;
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

pub(crate) fn interpretar_lb(
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
