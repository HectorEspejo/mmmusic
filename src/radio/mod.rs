pub mod espejos;
pub mod icy;
pub mod listas;
pub mod radiobrowser;
pub mod reconexion;

pub use icy::TituloIcy;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::biblioteca::consultas;
use crate::biblioteca::consultas::emisoras::NuevaEmisora;

pub const NOMBRE_EXPORTACION: &str = "Radio favoritas";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResumenImportacion {
    pub anadidas: usize,
    pub duplicadas: usize,
    pub invalidas: usize,
}

impl ResumenImportacion {
    pub fn mensaje(&self) -> String {
        format!(
            "{} emisoras añadidas, {} ya existían, {} sin URL válida",
            self.anadidas, self.duplicadas, self.invalidas
        )
    }
}

pub fn validar_nombre(nombre: &str) -> bool {
    let nombre = nombre.trim();
    (1..=100).contains(&nombre.chars().count())
}

pub fn validar_url(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() || url.len() > 2048 {
        return false;
    }
    let resto = if let Some(resto) = url.strip_prefix("https://") {
        resto
    } else if let Some(resto) = url.strip_prefix("http://") {
        resto
    } else {
        return false;
    };
    let host = resto
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("");
    let host = host.split(':').next().unwrap_or("");
    !host.is_empty()
}

pub fn nombre_desde_url(url: &str) -> String {
    let host = url
        .split("://")
        .nth(1)
        .unwrap_or(url)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim_start_matches("www.");
    if host.is_empty() {
        "Emisora".to_string()
    } else {
        host.to_string()
    }
}

/// Importa un fichero PLS o M3U/M3U8 local. Las entradas sin URL http(s)
/// cuentan como inválidas y las repetidas (en el fichero o en la base) como
/// duplicadas.
pub fn importar_listas(conn: &Connection, ruta: &Path) -> Result<ResumenImportacion> {
    let contenido = std::fs::read_to_string(ruta)
        .with_context(|| format!("no se pudo leer {}", ruta.display()))?;
    let extension = ruta
        .extension()
        .and_then(|valor| valor.to_str())
        .map(str::to_ascii_lowercase);
    let entradas = listas::parsear(&contenido, extension.as_deref());
    let mut resumen = ResumenImportacion::default();
    let mut vistas: HashSet<String> = HashSet::new();
    for entrada in entradas {
        let url = consultas::emisoras::normalizar_url(&entrada.url);
        if !validar_url(&url) {
            resumen.invalidas += 1;
            continue;
        }
        if !vistas.insert(url.clone()) || consultas::emisoras::por_url(conn, &url)?.is_some() {
            resumen.duplicadas += 1;
            continue;
        }
        let nombre = entrada
            .nombre
            .as_deref()
            .map(str::trim)
            .filter(|valor| !valor.is_empty() && validar_nombre(valor))
            .map(str::to_string)
            .unwrap_or_else(|| nombre_desde_url(&url));
        let nueva = NuevaEmisora {
            nombre,
            url,
            ..NuevaEmisora::default()
        };
        consultas::emisoras::crear(conn, &nueva)?;
        resumen.anadidas += 1;
    }
    Ok(resumen)
}

/// Escribe las emisoras favoritas en `dir/<nombre>.m3u8` (sobrescribe).
pub fn exportar_favoritas(conn: &Connection, dir: &Path, nombre: &str) -> Result<PathBuf> {
    let favoritas = consultas::emisoras::listar(
        conn,
        true,
        consultas::emisoras::OrdenEmisoras::Nombre,
        false,
    )?;
    let entradas: Vec<listas::EntradaLista> = favoritas
        .into_iter()
        .map(|emisora| listas::EntradaLista {
            nombre: Some(emisora.nombre),
            url: emisora.url,
        })
        .collect();
    std::fs::create_dir_all(dir).with_context(|| format!("no se pudo crear {}", dir.display()))?;
    let ruta = dir.join(format!("{nombre}.m3u8"));
    std::fs::write(&ruta, listas::escribir_m3u8(&entradas))
        .with_context(|| format!("no se pudo escribir {}", ruta.display()))?;
    Ok(ruta)
}

/// Limpia caracteres de control de nombres y URLs que vienen del directorio.
pub fn limpiar_texto(texto: &str) -> String {
    texto
        .chars()
        .filter(|caracter| !caracter.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}
