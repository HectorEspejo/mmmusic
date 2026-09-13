use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::biblioteca::{bd, caratulas, consultas, etiquetas};
use crate::eventos::{AppEvento, NivelAviso};
use crate::radio::espejos;
use crate::red::cliente;

const CICLO: Duration = Duration::from_secs(30);
const VIGENCIA_ESPEJO: Duration = Duration::from_secs(24 * 3600);
const VIGENCIA_CACHE: i64 = 24 * 3600;
const PODA_CACHE_DIAS: i64 = 7;
const PAUSA_429: Duration = Duration::from_secs(5 * 60);
const MAX_ESPEJOS_POR_BUSQUEDA: usize = 3;
const LIMITE_RESULTADOS: usize = 100;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmisoraDirectorio {
    #[serde(rename = "stationuuid", default)]
    pub uuid: String,
    #[serde(rename = "name", default)]
    pub nombre: String,
    #[serde(rename = "url_resolved", default)]
    pub url_resolved: String,
    #[serde(rename = "url", default)]
    pub url: String,
    #[serde(rename = "homepage", default)]
    pub pagina_web: String,
    #[serde(rename = "countrycode", default)]
    pub pais: String,
    #[serde(rename = "tags", default)]
    pub etiquetas: String,
    #[serde(rename = "codec", default)]
    pub codec: String,
    #[serde(rename = "bitrate", default)]
    pub bitrate: i64,
    #[serde(rename = "favicon", default)]
    pub favicon: String,
    #[serde(rename = "votes", default)]
    pub votos: i64,
    #[serde(rename = "lastcheckok", default)]
    pub lastcheckok: i64,
}

impl EmisoraDirectorio {
    pub fn url(&self) -> &str {
        if self.url_resolved.trim().is_empty() {
            &self.url
        } else {
            &self.url_resolved
        }
    }

    pub fn nombre_limpio(&self) -> String {
        crate::radio::limpiar_texto(&self.nombre)
    }

    pub fn pais_limpio(&self) -> Option<String> {
        let pais = crate::radio::limpiar_texto(&self.pais).to_uppercase();
        (!pais.is_empty()).then_some(pais)
    }

    pub fn url_logo(&self) -> Option<String> {
        let url = self.favicon.trim();
        (!url.is_empty()).then(|| url.to_string())
    }
}

/// Clave normalizada de una búsqueda: `search|nombre|pais|etiqueta|votes`.
pub fn clave_busqueda(nombre: &str, pais: &str, etiqueta: &str) -> String {
    format!(
        "search|{}|{}|{}|votes",
        etiquetas::normalizar(nombre),
        etiquetas::normalizar(pais),
        etiquetas::normalizar(etiqueta)
    )
}

#[derive(Debug, Clone)]
pub enum ComandoDirectorio {
    Buscar {
        clave: String,
        nombre: String,
        pais: String,
        etiqueta: String,
    },
    Click(String),
    Logo {
        emisora_id: i64,
        url: String,
    },
    Apagar,
}

pub struct ManejoDirectorio {
    tx: Sender<ComandoDirectorio>,
    hilo: Option<JoinHandle<()>>,
}

impl ManejoDirectorio {
    pub fn enviar(&self, comando: ComandoDirectorio) -> bool {
        self.tx.send(comando).is_ok()
    }

    pub fn apagar(mut self) {
        let _ = self.tx.send(ComandoDirectorio::Apagar);
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

pub fn lanzar(
    ruta_bd: PathBuf,
    dir_logos: PathBuf,
    descargar_logos: bool,
    tx_app: Sender<AppEvento>,
) -> Result<ManejoDirectorio> {
    let (tx, rx) = mpsc::channel();
    let hilo = thread::Builder::new()
        .name("directorio".to_string())
        .spawn(move || {
            if let Err(error) = ejecutar(ruta_bd, dir_logos, descargar_logos, tx_app, rx) {
                warn!("hilo de directorio detenido: {error:#}");
            }
        })
        .context("no se pudo lanzar el hilo de directorio")?;
    Ok(ManejoDirectorio {
        tx,
        hilo: Some(hilo),
    })
}

fn ejecutar(
    ruta_bd: PathBuf,
    dir_logos: PathBuf,
    descargar_logos: bool,
    tx_app: Sender<AppEvento>,
    rx: Receiver<ComandoDirectorio>,
) -> Result<()> {
    let conn = bd::abrir_y_migrar(&ruta_bd)?;
    let agente = cliente::agente_http();
    let mut estado = EstadoDirectorio {
        espejo: None,
        espejo_hasta: Instant::now(),
        pausa_hasta: None,
        clicks: HashSet::new(),
        logos_cargados: HashSet::new(),
    };
    loop {
        match rx.recv_timeout(CICLO) {
            Ok(ComandoDirectorio::Apagar) => break,
            Ok(ComandoDirectorio::Buscar {
                clave,
                nombre,
                pais,
                etiqueta,
            }) => buscar(
                &conn,
                &agente,
                &tx_app,
                &mut estado,
                &clave,
                &nombre,
                &pais,
                &etiqueta,
            ),
            Ok(ComandoDirectorio::Click(uuid)) => click(&agente, &mut estado, &uuid),
            Ok(ComandoDirectorio::Logo { emisora_id, url }) => {
                if descargar_logos
                    && let Err(error) =
                        logo(&conn, &tx_app, &mut estado, &dir_logos, emisora_id, &url)
                {
                    debug!(emisora_id, "logo descartado: {error:#}");
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    info!("hilo de directorio detenido");
    Ok(())
}

struct EstadoDirectorio {
    espejo: Option<String>,
    espejo_hasta: Instant,
    pausa_hasta: Option<Instant>,
    clicks: HashSet<(String, String)>,
    logos_cargados: HashSet<i64>,
}

#[allow(clippy::too_many_arguments)]
fn buscar(
    conn: &Connection,
    agente: &ureq::Agent,
    tx_app: &Sender<AppEvento>,
    estado: &mut EstadoDirectorio,
    clave: &str,
    nombre: &str,
    pais: &str,
    etiqueta: &str,
) {
    if let Ok(Some((_respuesta, obtenido_en))) = consultas::busquedas_radio::leer(conn, clave)
        && vigente(&obtenido_en, VIGENCIA_CACHE)
    {
        let _ = tx_app.send(AppEvento::ResultadosRadio(clave.to_string()));
        return;
    }
    if estado
        .pausa_hasta
        .is_some_and(|hasta| Instant::now() < hasta)
    {
        notificar(
            tx_app,
            NivelAviso::Aviso,
            "Radio Browser pidió esperar; inténtalo en unos minutos",
        );
        return;
    }
    let mut candidatos = espejos_para_busqueda(conn, estado).unwrap_or_else(|error| {
        warn!("no se pudieron resolver espejos: {error:#}");
        vec![espejos::HOST.to_string()]
    });
    let mut intentados: Vec<String> = Vec::new();
    let mut ultimo_error: Option<String> = None;
    while intentados.len() < MAX_ESPEJOS_POR_BUSQUEDA {
        let Some(espejo) = candidatos
            .iter()
            .find(|candidato| !intentados.contains(candidato))
            .cloned()
        else {
            break;
        };
        intentados.push(espejo.clone());
        match consultar(agente, &espejo, nombre, pais, etiqueta) {
            Ok(respuesta) => {
                guardar_resultados(conn, clave, respuesta);
                let _ = tx_app.send(AppEvento::ResultadosRadio(clave.to_string()));
                return;
            }
            Err(FalloDirectorio::DemasiadasPeticiones) => {
                estado.pausa_hasta = Some(Instant::now() + PAUSA_429);
                notificar(
                    tx_app,
                    NivelAviso::Aviso,
                    "Radio Browser limitó las peticiones; reintento en 5 minutos",
                );
                return;
            }
            Err(error) => {
                debug!(espejo, "fallo de búsqueda: {error}");
                ultimo_error = Some(error.to_string());
                if intentados.len() == 1
                    && let Ok(extra) = espejos::resolver_espejos()
                {
                    for candidato in extra {
                        if !candidatos.contains(&candidato) {
                            candidatos.push(candidato);
                        }
                    }
                }
            }
        }
    }
    if let Some(espejo) = candidatos
        .iter()
        .find(|candidato| !intentados.contains(candidato))
    {
        guardar_espejo(conn, estado, espejo);
    }
    warn!("Radio Browser no disponible: {ultimo_error:?}");
    mostrar_cache_o_aviso(conn, tx_app, clave);
}

fn guardar_resultados(conn: &Connection, clave: &str, respuesta: Vec<EmisoraDirectorio>) {
    let filtradas: Vec<EmisoraDirectorio> = respuesta
        .into_iter()
        .filter(|emisora| emisora.lastcheckok != 0 && !emisora.url().trim().is_empty())
        .collect();
    match serde_json::to_string(&filtradas) {
        Ok(json) => {
            if let Err(error) = consultas::busquedas_radio::guardar(conn, clave, &json) {
                warn!("no se pudo cachear la búsqueda: {error:#}");
            }
            let _ = consultas::busquedas_radio::podar(conn, PODA_CACHE_DIAS);
        }
        Err(error) => warn!("no se pudo serializar la búsqueda: {error:#}"),
    }
}

fn guardar_espejo(conn: &Connection, estado: &mut EstadoDirectorio, espejo: &str) {
    let valor = format!("{espejo}|{}", bd::iso_en(VIGENCIA_ESPEJO.as_secs() as i64));
    if let Err(error) = consultas::ajustes::escribir(conn, "radiobrowser_servidor", &valor) {
        warn!("no se pudo guardar el espejo de Radio Browser: {error:#}");
    }
    estado.espejo = Some(espejo.to_string());
    estado.espejo_hasta = Instant::now() + VIGENCIA_ESPEJO;
}

fn espejos_para_busqueda(conn: &Connection, estado: &mut EstadoDirectorio) -> Result<Vec<String>> {
    if let Some(espejo) = estado.espejo.clone()
        && Instant::now() < estado.espejo_hasta
    {
        return Ok(vec![espejo]);
    }
    if let Ok(Some(valor)) = consultas::ajustes::leer(conn, "radiobrowser_servidor")
        && let Some((espejo, fecha)) = valor.split_once('|')
        && !espejo.trim().is_empty()
        && vigente(fecha, VIGENCIA_ESPEJO.as_secs() as i64)
    {
        let espejo = espejo.trim().to_string();
        estado.espejo = Some(espejo.clone());
        estado.espejo_hasta = Instant::now() + VIGENCIA_ESPEJO;
        return Ok(vec![espejo]);
    }
    let mut candidatos = espejos::resolver_espejos()?;
    if candidatos.is_empty() {
        candidatos.push(espejos::HOST.to_string());
    }
    let indice = rand::random_range(0..candidatos.len());
    candidatos.rotate_left(indice);
    let elegido = candidatos[0].clone();
    guardar_espejo(conn, estado, &elegido);
    Ok(candidatos)
}

fn consultar(
    agente: &ureq::Agent,
    espejo: &str,
    nombre: &str,
    pais: &str,
    etiqueta: &str,
) -> std::result::Result<Vec<EmisoraDirectorio>, FalloDirectorio> {
    let mut url = format!(
        "https://{espejo}/json/stations/search?order=votes&reverse=true&limit={LIMITE_RESULTADOS}&hidebroken=true"
    );
    if !nombre.trim().is_empty() {
        url.push_str(&format!("&name={}", codificar_query(nombre)));
    }
    if !pais.trim().is_empty() {
        url.push_str(&format!("&country={}", codificar_query(pais)));
    }
    if !etiqueta.trim().is_empty() {
        url.push_str(&format!("&tag={}", codificar_query(etiqueta)));
    }
    if !cliente::url_permitida(&url) {
        return Err(FalloDirectorio::HostNoPermitido);
    }
    let mut respuesta = agente
        .get(&url)
        .call()
        .map_err(|error| FalloDirectorio::Red(error.to_string()))?;
    let status = respuesta.status().as_u16();
    if status == 429 {
        return Err(FalloDirectorio::DemasiadasPeticiones);
    }
    if !(200..300).contains(&status) {
        return Err(FalloDirectorio::Red(format!("HTTP {status}")));
    }
    let cuerpo = respuesta
        .body_mut()
        .with_config()
        .limit(8 * 1024 * 1024)
        .read_to_string()
        .map_err(|error| FalloDirectorio::Red(error.to_string()))?;
    serde_json::from_str(&cuerpo).map_err(|error| FalloDirectorio::Formato(error.to_string()))
}

fn click(agente: &ureq::Agent, estado: &mut EstadoDirectorio, uuid: &str) {
    if uuid.trim().is_empty() {
        return;
    }
    let hoy = bd::ahora_iso()[..10].to_string();
    if !estado.clicks.insert((uuid.to_string(), hoy)) {
        return;
    }
    let Some(espejo) = estado.espejo.clone() else {
        return;
    };
    let url = format!("https://{espejo}/json/url/{uuid}");
    if !cliente::url_permitida(&url) {
        return;
    }
    if let Err(error) = agente.get(&url).call() {
        debug!(uuid, "click no registrado: {error}");
    }
}

fn logo(
    conn: &Connection,
    tx_app: &Sender<AppEvento>,
    estado: &mut EstadoDirectorio,
    dir_logos: &std::path::Path,
    emisora_id: i64,
    url: &str,
) -> Result<()> {
    if !estado.logos_cargados.insert(emisora_id) {
        return Ok(());
    }
    let datos = cliente::descargar_logo(url).map_err(|fallo| anyhow::anyhow!(fallo.mensaje))?;
    let (ruta, _) = caratulas::cachear(&datos, emisora_id, dir_logos)?;
    let ruta_texto = ruta.to_string_lossy().to_string();
    consultas::emisoras::fijar_logo(conn, emisora_id, &ruta_texto)?;
    info!(emisora_id, "logo de emisora cacheado");
    let _ = tx_app.send(AppEvento::LogoListo(emisora_id));
    Ok(())
}

fn mostrar_cache_o_aviso(conn: &Connection, tx_app: &Sender<AppEvento>, clave: &str) {
    if let Ok(Some(_)) = consultas::busquedas_radio::leer(conn, clave) {
        let _ = tx_app.send(AppEvento::ResultadosRadio(clave.to_string()));
        notificar(
            tx_app,
            NivelAviso::Aviso,
            "Radio Browser no disponible; resultados en caché",
        );
    } else {
        notificar(
            tx_app,
            NivelAviso::Error,
            "Radio Browser no disponible y sin resultados en caché",
        );
    }
}

fn notificar(tx_app: &Sender<AppEvento>, nivel: NivelAviso, mensaje: &str) {
    let _ = tx_app.send(AppEvento::Notificacion(nivel, mensaje.to_string()));
}

fn vigente(fecha_iso: &str, segundos: i64) -> bool {
    bd::unix_desde_iso(fecha_iso)
        .map(|unix| bd::ahora_unix() - unix < segundos)
        .unwrap_or(false)
}

fn codificar_query(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len());
    for byte in texto.trim().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                salida.push(*byte as char);
            }
            b' ' => salida.push('+'),
            _ => salida.push_str(&format!("%{byte:02X}")),
        }
    }
    salida
}

enum FalloDirectorio {
    Red(String),
    Formato(String),
    DemasiadasPeticiones,
    HostNoPermitido,
}

impl std::fmt::Display for FalloDirectorio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FalloDirectorio::Red(mensaje) => write!(f, "red: {mensaje}"),
            FalloDirectorio::Formato(mensaje) => write!(f, "formato: {mensaje}"),
            FalloDirectorio::DemasiadasPeticiones => write!(f, "demasiadas peticiones"),
            FalloDirectorio::HostNoPermitido => write!(f, "host no permitido"),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_clave_normaliza_los_campos() {
        assert_eq!(
            clave_busqueda("Vapor Wave", "US", "Chill"),
            "search|vapor wave|us|chill|votes"
        );
    }

    #[test]
    fn codifica_los_parametros() {
        assert_eq!(codificar_query("lofi girl"), "lofi+girl");
        assert_eq!(codificar_query("d&b"), "d%26b");
        assert_eq!(codificar_query("españa"), "espa%C3%B1a");
    }

    #[test]
    fn elige_url_resuelta_si_existe() {
        let emisora = EmisoraDirectorio {
            url: "http://a.example/stream".to_string(),
            url_resolved: "http://b.example/stream".to_string(),
            ..EmisoraDirectorio::default()
        };
        assert_eq!(emisora.url(), "http://b.example/stream");
    }
}
