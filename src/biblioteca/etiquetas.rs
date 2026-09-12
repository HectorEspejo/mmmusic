use std::path::Path;

use anyhow::{Context, Result};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

pub const ARTISTA_DESCONOCIDO: &str = "Artista desconocido";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EtiquetasCrudas {
    pub titulo: Option<String>,
    pub artista: Option<String>,
    pub album_artista: Option<String>,
    pub album: Option<String>,
    pub anio: Option<i64>,
    pub disco: Option<i64>,
    pub pista: Option<i64>,
    pub genero: Option<String>,
    pub duracion_ms: Option<i64>,
    pub bitrate_kbps: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtiquetasResueltas {
    pub titulo: String,
    pub artista: String,
    pub album_artista: String,
    pub album_artista_etiquetado: bool,
    pub album: String,
    pub anio: Option<i64>,
    pub disco: i64,
    pub pista: Option<i64>,
    pub genero: Option<String>,
    pub duracion_ms: i64,
    pub bitrate_kbps: Option<i64>,
}

pub fn normalizar(texto: &str) -> String {
    let sin_diacriticos: String = texto.nfkd().filter(|c| !is_combining_mark(*c)).collect();
    let mut salida = String::with_capacity(sin_diacriticos.len());
    let mut espacio_pendiente = false;
    for c in sin_diacriticos.chars() {
        if c.is_whitespace() {
            espacio_pendiente = !salida.is_empty();
        } else {
            if espacio_pendiente {
                salida.push(' ');
                espacio_pendiente = false;
            }
            for bajo in c.to_lowercase() {
                salida.push(bajo);
            }
        }
    }
    salida
}

pub fn formato_de_ruta(ruta: &Path) -> Option<&'static str> {
    let extension = ruta.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "mp3" => Some("mp3"),
        "flac" => Some("flac"),
        "ogg" => Some("ogg"),
        "opus" => Some("opus"),
        "m4a" => Some("m4a"),
        "wav" => Some("wav"),
        _ => None,
    }
}

pub fn primeros_4_digitos(texto: &str) -> Option<i64> {
    let bytes = texto.as_bytes();
    if bytes.len() < 4 {
        return None;
    }
    for i in 0..=bytes.len() - 4 {
        if bytes[i..i + 4].iter().all(u8::is_ascii_digit) {
            return texto.get(i..i + 4)?.parse().ok();
        }
    }
    None
}

pub fn parsear_numero(texto: &str) -> Option<i64> {
    let primero = texto.split('/').next()?.trim();
    primero.parse().ok()
}

pub fn leer(ruta: &Path) -> Result<EtiquetasCrudas> {
    let tagged = Probe::open(ruta)
        .with_context(|| format!("no se pudo abrir {}", ruta.display()))?
        .read()
        .with_context(|| format!("no se pudieron leer las etiquetas de {}", ruta.display()))?;

    let propiedades = tagged.properties();
    let duracion = propiedades.duration();
    let duracion_ms = if duracion.is_zero() {
        None
    } else {
        Some(duracion.as_millis() as i64)
    };
    let bitrate_kbps = propiedades
        .audio_bitrate()
        .map(i64::from)
        .or_else(|| propiedades.overall_bitrate().map(i64::from));

    let mut crudas = EtiquetasCrudas {
        duracion_ms,
        bitrate_kbps,
        ..EtiquetasCrudas::default()
    };

    let Some(etiqueta) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        return Ok(crudas);
    };

    crudas.titulo = etiqueta.title().map(|t| t.into_owned());
    crudas.artista = etiqueta.artist().map(|a| a.into_owned());
    crudas.album = etiqueta.album().map(|a| a.into_owned());
    crudas.genero = etiqueta.genre().map(|g| g.into_owned());
    crudas.album_artista = etiqueta
        .get_string(ItemKey::AlbumArtist)
        .map(str::to_string);
    crudas.anio = etiqueta
        .get_string(ItemKey::RecordingDate)
        .and_then(primeros_4_digitos)
        .or_else(|| {
            etiqueta
                .get_string(ItemKey::Year)
                .and_then(primeros_4_digitos)
        });
    crudas.disco = etiqueta.disk().map(i64::from).or_else(|| {
        etiqueta
            .get_string(ItemKey::DiscNumber)
            .and_then(parsear_numero)
    });
    crudas.pista = etiqueta.track().map(i64::from).or_else(|| {
        etiqueta
            .get_string(ItemKey::TrackNumber)
            .and_then(parsear_numero)
    });
    Ok(crudas)
}

pub fn resolver(crudas: EtiquetasCrudas, ruta: &Path) -> EtiquetasResueltas {
    let titulo = crudas
        .titulo
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| nombre_sin_extension(ruta));
    let artista = crudas
        .artista
        .filter(|a| !a.trim().is_empty())
        .unwrap_or_else(|| ARTISTA_DESCONOCIDO.to_string());
    let album_artista_etiquetado = crudas
        .album_artista
        .as_deref()
        .is_some_and(|a| !a.trim().is_empty());
    let album_artista = crudas
        .album_artista
        .filter(|a| !a.trim().is_empty())
        .unwrap_or_else(|| artista.clone());
    let album = crudas
        .album
        .filter(|a| !a.trim().is_empty())
        .unwrap_or_else(|| nombre_carpeta(ruta));
    EtiquetasResueltas {
        titulo,
        artista,
        album_artista,
        album_artista_etiquetado,
        album,
        anio: crudas.anio,
        disco: crudas.disco.unwrap_or(1),
        pista: crudas.pista,
        genero: crudas.genero.filter(|g| !g.trim().is_empty()),
        duracion_ms: crudas.duracion_ms.unwrap_or(0),
        bitrate_kbps: crudas.bitrate_kbps,
    }
}

fn nombre_sin_extension(ruta: &Path) -> String {
    ruta.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Sin título")
        .to_string()
}

fn nombre_carpeta(ruta: &Path) -> String {
    ruta.parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or(ARTISTA_DESCONOCIDO)
        .to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn normaliza_diacriticos_y_espacios() {
        assert_eq!(normalizar("  Midnight  Première "), "midnight premiere");
        assert_eq!(normalizar("ESPRIT 空想"), "esprit 空想");
        assert_eq!(normalizar("ÁÉÍÓÚÜÑÇ"), "aeiouunc");
        assert_eq!(normalizar(""), "");
        assert_eq!(normalizar("   "), "");
    }

    #[test]
    fn extrae_anios() {
        assert_eq!(primeros_4_digitos("2024-03-01"), Some(2024));
        assert_eq!(primeros_4_digitos("12/03/1999"), Some(1999));
        assert_eq!(primeros_4_digitos("sin fecha"), None);
        assert_eq!(primeros_4_digitos("99"), None);
    }

    #[test]
    fn parsea_numeros_de_pista() {
        assert_eq!(parsear_numero("3/12"), Some(3));
        assert_eq!(parsear_numero(" 7 "), Some(7));
        assert_eq!(parsear_numero("A/B"), None);
    }

    #[test]
    fn reconoce_formatos() {
        assert_eq!(formato_de_ruta(Path::new("/x/a.MP3")), Some("mp3"));
        assert_eq!(formato_de_ruta(Path::new("/x/a.flac")), Some("flac"));
        assert_eq!(formato_de_ruta(Path::new("/x/a.opus")), Some("opus"));
        assert_eq!(formato_de_ruta(Path::new("/x/a.m4a")), Some("m4a"));
        assert_eq!(formato_de_ruta(Path::new("/x/a.ogg")), Some("ogg"));
        assert_eq!(formato_de_ruta(Path::new("/x/a.wav")), Some("wav"));
        assert_eq!(formato_de_ruta(Path::new("/x/a.txt")), None);
    }

    #[test]
    fn resuelve_valores_ausentes() {
        let ruta = Path::new("/Musica/Midnight Premiere/02 Neon Rain.flac");
        let resueltas = resolver(EtiquetasCrudas::default(), ruta);
        assert_eq!(resueltas.titulo, "02 Neon Rain");
        assert_eq!(resueltas.artista, ARTISTA_DESCONOCIDO);
        assert_eq!(resueltas.album_artista, ARTISTA_DESCONOCIDO);
        assert!(!resueltas.album_artista_etiquetado);
        assert_eq!(resueltas.album, "Midnight Premiere");
        assert_eq!(resueltas.disco, 1);
        assert_eq!(resueltas.pista, None);
        assert_eq!(resueltas.duracion_ms, 0);
    }

    #[test]
    fn resuelve_album_artist_y_album() {
        let crudas = EtiquetasCrudas {
            titulo: Some("Neon Rain".into()),
            artista: Some("ESPRIT 空想".into()),
            album_artista: Some("Varios".into()),
            album: Some("Recopilatorio".into()),
            disco: Some(2),
            pista: Some(3),
            duracion_ms: Some(12_345),
            ..EtiquetasCrudas::default()
        };
        let resueltas = resolver(crudas, Path::new("/x/y.mp3"));
        assert_eq!(resueltas.album_artista, "Varios");
        assert!(resueltas.album_artista_etiquetado);
        assert_eq!(resueltas.album, "Recopilatorio");
        assert_eq!(resueltas.disco, 2);
        assert_eq!(resueltas.pista, Some(3));
        assert_eq!(resueltas.duracion_ms, 12_345);
    }
}
