use std::time::Duration;

use tracing::debug;

pub const USER_AGENT: &str = concat!(
    "mmmusic/",
    env!("CARGO_PKG_VERSION"),
    " (+https://codeberg.org/4d3/mmmusic)"
);

pub const HOSTS_PERMITIDOS: [&str; 2] = [
    "https://api.listenbrainz.org",
    "https://ws.audioscrobbler.com",
];

const SUFIJO_RADIOBROWSER: &str = ".api.radio-browser.info";
const HOST_RADIOBROWSER: &str = "all.api.radio-browser.info";
const LIMITE_LOGO_BYTES: usize = 512 * 1024;
const TIMEOUT_LOGO: Duration = Duration::from_secs(5);
const REDIRECCIONES_LOGO: u32 = 3;

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

pub fn agente_http() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .build()
        .into()
}

pub fn url_permitida(url: &str) -> bool {
    if HOSTS_PERMITIDOS
        .iter()
        .any(|host| url.starts_with(&format!("{host}/")))
    {
        return true;
    }
    let Some(resto) = url.strip_prefix("https://") else {
        return false;
    };
    let host = resto.split(['/', '?', '#']).next().unwrap_or("");
    host == HOST_RADIOBROWSER || host.ends_with(SUFIJO_RADIOBROWSER)
}

/// Descarga un logo de emisora fuera de la lista cerrada de hosts. Solo admite
/// https, cuerpos de tipo `image/*`, un máximo de 512 KB y 3 redirecciones.
/// El contenido es no confiable: se decodifica en el hilo de directorio.
pub fn descargar_logo(url: &str) -> Result<Vec<u8>, FalloHttp> {
    if !url.starts_with("https://") {
        return Err(FalloHttp::red("el logo no usa https"));
    }
    let agente = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT_LOGO))
        .http_status_as_error(false)
        .https_only(true)
        .max_redirects(REDIRECCIONES_LOGO)
        .user_agent(USER_AGENT)
        .build();
    let agente: ureq::Agent = agente.into();
    let mut respuesta = match agente.get(url).call() {
        Ok(respuesta) => respuesta,
        Err(error) => return Err(FalloHttp::red(error.to_string())),
    };
    let status = respuesta.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(FalloHttp {
            status,
            codigo_servicio: None,
            mensaje: format!("HTTP {status}"),
        });
    }
    let tipo = respuesta
        .headers()
        .get("content-type")
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !tipo.starts_with("image/") {
        return Err(FalloHttp::red("la respuesta no es una imagen"));
    }
    match respuesta
        .body_mut()
        .with_config()
        .limit(LIMITE_LOGO_BYTES as u64 + 1)
        .read_to_vec()
    {
        Ok(datos) if datos.len() <= LIMITE_LOGO_BYTES => Ok(datos),
        Ok(_) => Err(FalloHttp::red("el logo supera 512 KB")),
        Err(error) => {
            debug!("descarga de logo interrumpida: {error}");
            Err(FalloHttp::red(error.to_string()))
        }
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
        assert!(url_permitida(
            "https://de1.api.radio-browser.info/json/stations/search"
        ));
        assert!(url_permitida(
            "https://all.api.radio-browser.info/json/stations/search"
        ));
        assert!(!url_permitida("https://example.com/robo"));
        assert!(!url_permitida(
            "http://api.listenbrainz.org/1/submit-listens"
        ));
        assert!(!url_permitida("https://api.listenbrainz.org.evil.com/x"));
        assert!(!url_permitida("https://radio-browser.info.evil.com/x"));
        assert!(!url_permitida("https://malapi.radio-browser.info/x"));
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

    #[test]
    fn el_logo_exige_https() {
        let fallo = descargar_logo("http://example.com/logo.png").expect_err("http no permitido");
        assert_eq!(fallo.status, 0);
    }
}
