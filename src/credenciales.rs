use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::warn;

pub const PERMISOS_FICHERO: u32 = 0o600;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Credenciales {
    pub listenbrainz: CredencialListenBrainz,
    pub lastfm: CredencialLastfm,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CredencialListenBrainz {
    pub token: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CredencialLastfm {
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub session_key: Option<String>,
    pub usuario: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CargaCredenciales {
    pub credenciales: Credenciales,
    pub existe: bool,
    pub permisos_corregidos: bool,
}

impl Credenciales {
    pub fn token_listenbrainz(&self) -> Option<&str> {
        self.listenbrainz
            .token
            .as_deref()
            .filter(|token| !token.trim().is_empty())
    }

    pub fn api_key_lastfm(&self) -> Option<&str> {
        self.lastfm
            .api_key
            .as_deref()
            .filter(|valor| !valor.trim().is_empty())
    }

    pub fn api_secret_lastfm(&self) -> Option<&str> {
        self.lastfm
            .api_secret
            .as_deref()
            .filter(|valor| !valor.trim().is_empty())
    }

    pub fn session_key_lastfm(&self) -> Option<&str> {
        self.lastfm
            .session_key
            .as_deref()
            .filter(|valor| !valor.trim().is_empty())
    }

    pub fn usuario_lastfm(&self) -> Option<&str> {
        self.lastfm
            .usuario
            .as_deref()
            .filter(|valor| !valor.trim().is_empty())
    }

    pub fn lastfm_autorizado(&self) -> bool {
        self.api_key_lastfm().is_some()
            && self.api_secret_lastfm().is_some()
            && self.session_key_lastfm().is_some()
    }

    pub fn redactar(&self, texto: &str) -> String {
        let mut salida = texto.to_string();
        for secreto in [
            self.token_listenbrainz(),
            self.api_secret_lastfm(),
            self.session_key_lastfm(),
        ]
        .into_iter()
        .flatten()
        {
            if !secreto.is_empty() {
                salida = salida.replace(secreto, "***");
            }
        }
        salida
    }
}

pub fn cargar(ruta: &Path) -> Result<CargaCredenciales> {
    if !ruta.exists() {
        return Ok(CargaCredenciales {
            credenciales: Credenciales::default(),
            existe: false,
            permisos_corregidos: false,
        });
    }
    let texto =
        fs::read_to_string(ruta).with_context(|| format!("no se pudo leer {}", ruta.display()))?;
    let credenciales: Credenciales = toml::from_str(&texto)
        .with_context(|| format!("credenciales inválidas en {}", ruta.display()))?;
    let permisos_corregidos = corregir_permisos(ruta)?;
    Ok(CargaCredenciales {
        credenciales,
        existe: true,
        permisos_corregidos,
    })
}

pub fn guardar(ruta: &Path, credenciales: &Credenciales) -> Result<()> {
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre)
            .with_context(|| format!("no se pudo crear {}", padre.display()))?;
    }
    let texto = toml::to_string_pretty(credenciales)
        .context("no se pudieron serializar las credenciales")?;
    escribir_privado(ruta, &texto)
}

#[cfg(unix)]
fn escribir_privado(ruta: &Path, texto: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut fichero = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(PERMISOS_FICHERO)
        .open(ruta)
        .with_context(|| format!("no se pudo crear {}", ruta.display()))?;
    fichero
        .write_all(texto.as_bytes())
        .with_context(|| format!("no se pudo escribir {}", ruta.display()))?;
    corregir_permisos(ruta)?;
    Ok(())
}

#[cfg(not(unix))]
fn escribir_privado(ruta: &Path, texto: &str) -> Result<()> {
    fs::write(ruta, texto).with_context(|| format!("no se pudo escribir {}", ruta.display()))
}

#[cfg(unix)]
fn corregir_permisos(ruta: &Path) -> Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    let metadatos = fs::metadata(ruta)
        .with_context(|| format!("no se pudieron leer los permisos de {}", ruta.display()))?;
    let modo = metadatos.permissions().mode() & 0o777;
    if modo == PERMISOS_FICHERO {
        return Ok(false);
    }
    warn!(ruta = %ruta.display(), modo = format!("{modo:o}"), "corrigiendo permisos de credenciales");
    let mut permisos = metadatos.permissions();
    permisos.set_mode(PERMISOS_FICHERO);
    fs::set_permissions(ruta, permisos)
        .with_context(|| format!("no se pudieron corregir los permisos de {}", ruta.display()))?;
    Ok(true)
}

#[cfg(not(unix))]
fn corregir_permisos(_ruta: &Path) -> Result<bool> {
    Ok(false)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn parsea_y_encuentra_secretos() {
        let texto = r#"
[listenbrainz]
token = "lb-token"

[lastfm]
api_key = "clave"
api_secret = "secreto"
session_key = "sesion"
usuario = "hector"
"#;
        let credenciales: Credenciales = toml::from_str(texto).expect("parseo");
        assert_eq!(credenciales.token_listenbrainz(), Some("lb-token"));
        assert!(credenciales.lastfm_autorizado());
        assert_eq!(credenciales.usuario_lastfm(), Some("hector"));

        let redactado = credenciales.redactar("fallo con lb-token y secreto y sesion");
        assert_eq!(redactado, "fallo con *** y *** y ***");
        assert!(!redactado.contains("lb-token"));
    }

    #[test]
    fn credenciales_parciales_no_autorizan() {
        let credenciales = Credenciales::default();
        assert!(!credenciales.lastfm_autorizado());
        assert_eq!(credenciales.token_listenbrainz(), None);
    }

    #[test]
    fn guarda_con_permisos_600() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ruta = dir.path().join("credenciales.toml");
        let credenciales = Credenciales {
            listenbrainz: CredencialListenBrainz {
                token: Some("token".to_string()),
            },
            ..Credenciales::default()
        };
        guardar(&ruta, &credenciales).expect("guardar");
        let carga = cargar(&ruta).expect("cargar");
        assert!(carga.existe);
        assert!(!carga.permisos_corregidos);
        assert_eq!(carga.credenciales.token_listenbrainz(), Some("token"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = fs::metadata(&ruta).expect("metadatos").permissions().mode() & 0o777;
            assert_eq!(modo, PERMISOS_FICHERO);
        }
    }

    #[cfg(unix)]
    #[test]
    fn corrige_permisos_demasiado_abiertos() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let ruta = dir.path().join("credenciales.toml");
        guardar(&ruta, &Credenciales::default()).expect("guardar");
        let mut permisos = fs::metadata(&ruta).expect("metadatos").permissions();
        permisos.set_mode(0o644);
        fs::set_permissions(&ruta, permisos).expect("abrir permisos");

        let carga = cargar(&ruta).expect("cargar");
        assert!(carga.permisos_corregidos);
        let modo = fs::metadata(&ruta).expect("metadatos").permissions().mode() & 0o777;
        assert_eq!(modo, PERMISOS_FICHERO);
    }

    #[test]
    fn fichero_inexistente_devuelve_vacio() {
        let dir = tempfile::tempdir().expect("tempdir");
        let carga = cargar(&dir.path().join("no-existe.toml")).expect("cargar");
        assert!(!carga.existe);
        assert!(carga.credenciales.token_listenbrainz().is_none());
    }
}
