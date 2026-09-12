use serde_json::Value;

use super::{FalloHttp, interpretar};
use crate::credenciales::Credenciales;

pub const HOST: &str = "https://ws.audioscrobbler.com/2.0/";

#[derive(Debug, Clone)]
pub struct Cancion<'a> {
    pub artista: &'a str,
    pub titulo: &'a str,
    pub album: &'a str,
    pub duracion_ms: i64,
    pub unix: Option<i64>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ResultadoScrobble {
    pub aceptados: usize,
    pub ignorados: Vec<(usize, String)>,
}

#[derive(Debug, Clone)]
pub struct Sesion {
    pub session_key: String,
    pub usuario: String,
}

pub fn firmar_parametros(parametros: &[(&str, &str)], api_secret: &str) -> String {
    let mut ordenados = parametros.to_vec();
    ordenados.sort_by(|a, b| a.0.cmp(b.0));
    let mut base = String::new();
    for (clave, valor) in ordenados {
        base.push_str(clave);
        base.push_str(valor);
    }
    base.push_str(api_secret);
    format!("{:x}", md5::compute(base.as_bytes()))
}

fn pares_firmados(
    api_key: &str,
    api_secret: &str,
    session_key: Option<&str>,
    metodo: &str,
    extra: Vec<(String, String)>,
) -> Vec<(String, String)> {
    let mut pares = vec![
        ("method".to_string(), metodo.to_string()),
        ("api_key".to_string(), api_key.to_string()),
    ];
    if let Some(session_key) = session_key {
        pares.push(("sk".to_string(), session_key.to_string()));
    }
    pares.extend(extra);
    let referencias: Vec<(&str, &str)> = pares
        .iter()
        .map(|(clave, valor)| (clave.as_str(), valor.as_str()))
        .collect();
    let firma = firmar_parametros(&referencias, api_secret);
    pares.push(("api_sig".to_string(), firma));
    pares.push(("format".to_string(), "json".to_string()));
    pares
}

pub struct ClienteLastfm {
    agente: ureq::Agent,
    api_key: Option<String>,
    api_secret: Option<String>,
    session_key: Option<String>,
}

impl ClienteLastfm {
    pub fn nuevo(credenciales: &Credenciales) -> Self {
        Self {
            agente: super::agente_http(),
            api_key: credenciales.api_key_lastfm().map(str::to_string),
            api_secret: credenciales.api_secret_lastfm().map(str::to_string),
            session_key: credenciales.session_key_lastfm().map(str::to_string),
        }
    }

    pub fn tiene_api(&self) -> bool {
        self.api_key.is_some() && self.api_secret.is_some()
    }

    pub fn activo(&self) -> bool {
        self.tiene_api() && self.session_key.is_some()
    }

    pub fn enviar_ahora(&self, cancion: &Cancion<'_>) -> Result<(), FalloHttp> {
        let extra = vec![
            ("artist".to_string(), cancion.artista.to_string()),
            ("track".to_string(), cancion.titulo.to_string()),
            ("album".to_string(), cancion.album.to_string()),
            (
                "duration".to_string(),
                (cancion.duracion_ms.max(0) / 1000).to_string(),
            ),
        ];
        self.post("track.updateNowPlaying", extra).map(|_| ())
    }

    pub fn enviar_scrobbles(
        &self,
        canciones: &[Cancion<'_>],
    ) -> Result<ResultadoScrobble, FalloHttp> {
        let mut extra = Vec::with_capacity(canciones.len() * 5);
        for (indice, cancion) in canciones.iter().enumerate() {
            extra.push((format!("artist[{indice}]"), cancion.artista.to_string()));
            extra.push((format!("track[{indice}]"), cancion.titulo.to_string()));
            extra.push((format!("album[{indice}]"), cancion.album.to_string()));
            extra.push((
                format!("duration[{indice}]"),
                (cancion.duracion_ms.max(0) / 1000).to_string(),
            ));
            extra.push((
                format!("timestamp[{indice}]"),
                cancion.unix.unwrap_or(0).to_string(),
            ));
        }
        let cuerpo = self.post("track.scrobble", extra)?;
        interpretar_scrobbles(&cuerpo)
    }

    pub fn amar(&self, cancion: &Cancion<'_>) -> Result<(), FalloHttp> {
        self.post("track.love", pares_cancion(cancion)).map(|_| ())
    }

    pub fn desamar(&self, cancion: &Cancion<'_>) -> Result<(), FalloHttp> {
        self.post("track.unlove", pares_cancion(cancion))
            .map(|_| ())
    }

    pub fn validar_sesion(&self) -> Result<String, FalloHttp> {
        let cuerpo = self.post("user.getInfo", Vec::new())?;
        let valor: Value = serde_json::from_str(&cuerpo)
            .map_err(|error| FalloHttp::red(format!("respuesta ilegible: {error}")))?;
        valor
            .pointer("/user/name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| FalloHttp::red("respuesta sin usuario"))
    }

    fn post(&self, metodo: &str, extra: Vec<(String, String)>) -> Result<String, FalloHttp> {
        let (Some(api_key), Some(api_secret)) =
            (self.api_key.as_deref(), self.api_secret.as_deref())
        else {
            return Err(FalloHttp::red("Last.fm sin api_key o api_secret"));
        };
        if self.session_key.is_none() {
            return Err(FalloHttp::red("Last.fm sin sesión autorizada"));
        }
        let pares = pares_firmados(
            api_key,
            api_secret,
            self.session_key.as_deref(),
            metodo,
            extra,
        );
        let respuesta = self.agente.post(HOST).send_form(
            pares
                .iter()
                .map(|(clave, valor)| (clave.as_str(), valor.as_str())),
        );
        interpretar(respuesta)
    }
}

fn pares_cancion(cancion: &Cancion<'_>) -> Vec<(String, String)> {
    vec![
        ("artist".to_string(), cancion.artista.to_string()),
        ("track".to_string(), cancion.titulo.to_string()),
    ]
}

fn interpretar_scrobbles(cuerpo: &str) -> Result<ResultadoScrobble, FalloHttp> {
    let valor: Value = serde_json::from_str(cuerpo)
        .map_err(|error| FalloHttp::red(format!("respuesta ilegible: {error}")))?;
    let Some(scrobbles) = valor.get("scrobbles") else {
        return Ok(ResultadoScrobble::default());
    };
    let lista: Vec<&Value> = match scrobbles.get("scrobble") {
        Some(Value::Array(elementos)) => elementos.iter().collect(),
        Some(elemento @ Value::Object(_)) => vec![elemento],
        _ => Vec::new(),
    };
    let mut ignorados = Vec::new();
    for (indice, elemento) in lista.iter().enumerate() {
        if let Some(motivo) = elemento.get("ignored_message").and_then(Value::as_str) {
            ignorados.push((indice, motivo.to_string()));
        }
    }
    let aceptados = lista.len().saturating_sub(ignorados.len());
    Ok(ResultadoScrobble {
        aceptados,
        ignorados,
    })
}

pub fn obtener_token(
    agente: &ureq::Agent,
    api_key: &str,
    api_secret: &str,
) -> Result<String, FalloHttp> {
    let pares = pares_firmados(api_key, api_secret, None, "auth.getToken", Vec::new());
    let cuerpo = get(agente, &pares)?;
    let valor: Value = serde_json::from_str(&cuerpo)
        .map_err(|error| FalloHttp::red(format!("respuesta ilegible: {error}")))?;
    valor
        .get("token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| FalloHttp::red("respuesta sin token"))
}

pub fn obtener_sesion(
    agente: &ureq::Agent,
    api_key: &str,
    api_secret: &str,
    token: &str,
) -> Result<Sesion, FalloHttp> {
    let extra = vec![("token".to_string(), token.to_string())];
    let pares = pares_firmados(api_key, api_secret, None, "auth.getSession", extra);
    let cuerpo = get(agente, &pares)?;
    let valor: Value = serde_json::from_str(&cuerpo)
        .map_err(|error| FalloHttp::red(format!("respuesta ilegible: {error}")))?;
    let Some(sesion) = valor.get("session") else {
        return Err(FalloHttp::red("respuesta sin sesión"));
    };
    let session_key = sesion
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| FalloHttp::red("sesión sin clave"))?
        .to_string();
    let usuario = sesion
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("usuario desconocido")
        .to_string();
    Ok(Sesion {
        session_key,
        usuario,
    })
}

fn get(agente: &ureq::Agent, pares: &[(String, String)]) -> Result<String, FalloHttp> {
    let mut peticion = agente.get(HOST);
    for (clave, valor) in pares {
        peticion = peticion.query(clave, valor);
    }
    interpretar(peticion.call())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn firma_coincide_con_el_vector_conocido() {
        let firma = firmar_parametros(
            &[("method", "auth.getToken"), ("api_key", "abc123")],
            "secreto",
        );
        assert_eq!(firma, "63fe35f258cfd5df6da0dde606daaafd");
    }

    #[test]
    fn firma_ordena_los_parametros() {
        let desordenada = firmar_parametros(
            &[
                ("method", "track.love"),
                ("artist", "Zeta"),
                ("api_key", "k"),
            ],
            "s",
        );
        let ordenada = firmar_parametros(
            &[
                ("api_key", "k"),
                ("artist", "Zeta"),
                ("method", "track.love"),
            ],
            "s",
        );
        assert_eq!(desordenada, ordenada);
    }

    #[test]
    fn interpreta_scrobble_simple_y_parcial() {
        let simple = r#"{"scrobbles":{"@attr":{"accepted":"1","ignored":"0"},"scrobble":[{"timestamp":"1"}]}}"#;
        let resultado = interpretar_scrobbles(simple).expect("simple");
        assert_eq!(resultado.aceptados, 1);
        assert!(resultado.ignorados.is_empty());

        let parcial = r#"{"scrobbles":{"@attr":{"accepted":"1","ignored":"1"},"scrobble":[{"timestamp":"1"},{"ignored_message":"Invalid timestamp","timestamp":"2"}]}}"#;
        let resultado = interpretar_scrobbles(parcial).expect("parcial");
        assert_eq!(resultado.aceptados, 1);
        assert_eq!(
            resultado.ignorados,
            vec![(1, "Invalid timestamp".to_string())]
        );

        let unico =
            r#"{"scrobbles":{"@attr":{"ignored":"1"},"scrobble":{"ignored_message":"Too old"}}}"#;
        let resultado = interpretar_scrobbles(unico).expect("único");
        assert_eq!(resultado.ignorados, vec![(0, "Too old".to_string())]);
    }
}
