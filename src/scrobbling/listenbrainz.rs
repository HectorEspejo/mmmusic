use serde::Serialize;

use crate::biblioteca::modelos::PistaResumen;
use crate::red::cliente::{FalloHttp, agente_http, interpretar, url_permitida};

pub const HOST: &str = "https://api.listenbrainz.org";
pub const RUTA_ENVIO: &str = "/1/submit-listens";
pub const RUTA_VALIDACION: &str = "/1/validate-token";
pub const MEDIA_PLAYER: &str = "mmmusic";
pub const SUBMISSION_CLIENT: &str = "mmmusic";

#[derive(Debug, Clone, Serialize)]
pub struct Escucha {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listened_at: Option<i64>,
    pub track_metadata: MetadatosPista,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetadatosPista {
    pub artist_name: String,
    pub track_name: String,
    pub release_name: String,
    pub additional_info: InfoAdicional,
}

#[derive(Debug, Clone, Serialize)]
pub struct InfoAdicional {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    pub media_player: &'static str,
    pub submission_client: &'static str,
}

#[derive(Serialize)]
struct Peticion {
    listen_type: &'static str,
    payload: Vec<Escucha>,
}

pub fn escucha(
    artista: &str,
    titulo: &str,
    album: &str,
    duracion_ms: Option<i64>,
    listened_at: Option<i64>,
) -> Escucha {
    Escucha {
        listened_at,
        track_metadata: MetadatosPista {
            artist_name: artista.to_string(),
            track_name: titulo.to_string(),
            release_name: album.to_string(),
            additional_info: InfoAdicional {
                duration_ms: duracion_ms,
                media_player: MEDIA_PLAYER,
                submission_client: SUBMISSION_CLIENT,
            },
        },
    }
}

pub fn escucha_de(pista: &PistaResumen, listened_at: Option<i64>) -> Escucha {
    escucha(
        &pista.artista,
        &pista.titulo,
        &pista.album,
        Some(pista.duracion_ms),
        listened_at,
    )
}

pub fn cuerpo_playing_now(pista: &PistaResumen) -> Result<String, serde_json::Error> {
    cuerpo_playing_now_escucha(escucha_de(pista, None))
}

pub fn cuerpo_playing_now_escucha(escucha: Escucha) -> Result<String, serde_json::Error> {
    cuerpo("playing_now", vec![escucha])
}

pub fn cuerpo_import(escuchas: Vec<Escucha>) -> Result<String, serde_json::Error> {
    cuerpo("import", escuchas)
}

fn cuerpo(tipo: &'static str, payload: Vec<Escucha>) -> Result<String, serde_json::Error> {
    serde_json::to_string(&Peticion {
        listen_type: tipo,
        payload,
    })
}

pub struct ClienteListenBrainz {
    agente: ureq::Agent,
    token: Option<String>,
}

impl ClienteListenBrainz {
    pub fn nuevo(token: Option<String>) -> Self {
        Self {
            agente: agente_http(),
            token,
        }
    }

    pub fn activo(&self) -> bool {
        self.token.is_some()
    }

    pub fn actualizar_token(&mut self, token: Option<String>) {
        self.token = token;
    }

    pub fn enviar_ahora(&self, pista: &PistaResumen) -> Result<(), FalloHttp> {
        let Some(token) = self.token.as_deref() else {
            return Err(FalloHttp::red("ListenBrainz sin token"));
        };
        let cuerpo = cuerpo_playing_now(pista)
            .map_err(|error| FalloHttp::red(format!("JSON inválido: {error}")))?;
        self.post(RUTA_ENVIO, token, &cuerpo).map(|_| ())
    }

    pub fn enviar_ahora_escucha(&self, escucha: Escucha) -> Result<(), FalloHttp> {
        let Some(token) = self.token.as_deref() else {
            return Err(FalloHttp::red("ListenBrainz sin token"));
        };
        let cuerpo = cuerpo_playing_now_escucha(escucha)
            .map_err(|error| FalloHttp::red(format!("JSON inválido: {error}")))?;
        self.post(RUTA_ENVIO, token, &cuerpo).map(|_| ())
    }

    pub fn enviar_lote(&self, escuchas: Vec<Escucha>) -> Result<(), FalloHttp> {
        let Some(token) = self.token.as_deref() else {
            return Err(FalloHttp::red("ListenBrainz sin token"));
        };
        let cuerpo = cuerpo_import(escuchas)
            .map_err(|error| FalloHttp::red(format!("JSON inválido: {error}")))?;
        self.post(RUTA_ENVIO, token, &cuerpo).map(|_| ())
    }

    pub fn validar_token(&self) -> Result<String, FalloHttp> {
        let Some(token) = self.token.as_deref() else {
            return Err(FalloHttp::red("ListenBrainz sin token"));
        };
        let url = format!("{HOST}{RUTA_VALIDACION}");
        if !url_permitida(&url) {
            return Err(FalloHttp::red("host no permitido"));
        }
        let respuesta = self
            .agente
            .get(&url)
            .header("Authorization", &format!("Token {token}"))
            .call();
        let cuerpo = interpretar(respuesta)?;
        let valor: serde_json::Value = serde_json::from_str(&cuerpo)
            .map_err(|error| FalloHttp::red(format!("respuesta ilegible: {error}")))?;
        if valor.get("valid").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err(FalloHttp {
                status: 401,
                codigo_servicio: None,
                mensaje: "token no válido".to_string(),
            });
        }
        Ok(valor
            .get("user_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("usuario desconocido")
            .to_string())
    }

    fn post(&self, ruta: &str, token: &str, cuerpo: &str) -> Result<String, FalloHttp> {
        let url = format!("{HOST}{ruta}");
        if !url_permitida(&url) {
            return Err(FalloHttp::red("host no permitido"));
        }
        let respuesta = self
            .agente
            .post(&url)
            .header("Authorization", &format!("Token {token}"))
            .header("Content-Type", "application/json")
            .send(cuerpo);
        interpretar(respuesta)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn pista() -> PistaResumen {
        PistaResumen {
            id: 7,
            titulo: "Neon Rain".to_string(),
            artista: "ESPRIT 空想".to_string(),
            album: "Midnight Premiere".to_string(),
            duracion_ms: 245_000,
            ..PistaResumen::default()
        }
    }

    #[test]
    fn cuerpo_playing_now_sin_listened_at() {
        let cuerpo = cuerpo_playing_now(&pista()).expect("json");
        let valor: serde_json::Value = serde_json::from_str(&cuerpo).expect("parseo");
        assert_eq!(valor["listen_type"], "playing_now");
        assert!(valor["payload"][0].get("listened_at").is_none());
        assert_eq!(
            valor["payload"][0]["track_metadata"]["artist_name"],
            "ESPRIT 空想"
        );
        assert_eq!(
            valor["payload"][0]["track_metadata"]["track_name"],
            "Neon Rain"
        );
        assert_eq!(
            valor["payload"][0]["track_metadata"]["release_name"],
            "Midnight Premiere"
        );
        assert_eq!(
            valor["payload"][0]["track_metadata"]["additional_info"]["duration_ms"],
            245_000
        );
        assert_eq!(
            valor["payload"][0]["track_metadata"]["additional_info"]["media_player"],
            "mmmusic"
        );
        assert_eq!(
            valor["payload"][0]["track_metadata"]["additional_info"]["submission_client"],
            "mmmusic"
        );
    }

    #[test]
    fn cuerpo_import_con_listened_at() {
        let escucha = escucha_de(&pista(), Some(1_700_000_000));
        let cuerpo = cuerpo_import(vec![escucha]).expect("json");
        let valor: serde_json::Value = serde_json::from_str(&cuerpo).expect("parseo");
        assert_eq!(valor["listen_type"], "import");
        assert_eq!(valor["payload"][0]["listened_at"], 1_700_000_000);
    }
}
