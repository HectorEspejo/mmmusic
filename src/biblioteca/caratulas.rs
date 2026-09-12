use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use image::imageops::FilterType;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use rusqlite::{Connection, params};
use tracing::{info, warn};

const LADO_CACHE: u32 = 300;
const CALIDAD_JPEG: u8 = 85;
const LIMITE_BYTES: usize = 40 * 1024 * 1024;

pub fn extraer_embebida(ruta: &Path) -> Result<Option<Vec<u8>>> {
    let tagged = Probe::open(ruta)
        .with_context(|| format!("no se pudo abrir {}", ruta.display()))?
        .read()
        .with_context(|| format!("no se pudieron leer las etiquetas de {}", ruta.display()))?;
    let Some(etiqueta) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        return Ok(None);
    };
    let Some(picture) = etiqueta.pictures().first() else {
        return Ok(None);
    };
    let datos = picture.data();
    if datos.is_empty() || datos.len() > LIMITE_BYTES {
        return Ok(None);
    }
    Ok(Some(datos.to_vec()))
}

pub fn buscar_fichero(carpeta: &Path) -> Option<PathBuf> {
    let entradas = fs::read_dir(carpeta).ok()?;
    let mut candidatos: Vec<PathBuf> = Vec::new();
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        let Some(nombre) = ruta.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let nombre_min = nombre.to_ascii_lowercase();
        let es_imagen = matches!(
            ruta.extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif")
        );
        if es_imagen
            && (nombre_min.starts_with("cover")
                || nombre_min.starts_with("folder")
                || nombre_min.starts_with("front"))
        {
            candidatos.push(ruta);
        }
    }
    candidatos.sort();
    candidatos.into_iter().next()
}

pub fn cachear(datos: &[u8], album_id: i64, dir_cache: &Path) -> Result<PathBuf> {
    fs::create_dir_all(dir_cache)
        .with_context(|| format!("no se pudo crear {}", dir_cache.display()))?;
    let imagen = image::load_from_memory(datos).context("imagen de carátula ilegible")?;
    let redimensionada = imagen.resize_to_fill(LADO_CACHE, LADO_CACHE, FilterType::Lanczos3);
    let ruta = dir_cache.join(format!("{album_id}.jpg"));
    let fichero =
        fs::File::create(&ruta).with_context(|| format!("no se pudo crear {}", ruta.display()))?;
    let rgb = redimensionada.to_rgb8();
    let mut codificador = image::codecs::jpeg::JpegEncoder::new_with_quality(fichero, CALIDAD_JPEG);
    codificador
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .context("no se pudo escribir la carátula")?;
    Ok(ruta)
}

pub fn procesar_pendientes(
    conn: &Connection,
    dir_cache: &Path,
    cancelacion: Option<&AtomicBool>,
) -> Result<usize> {
    let pendientes: Vec<i64> = {
        let mut sentencia = conn
            .prepare("SELECT id FROM ALBUMES WHERE caratula_ruta IS NULL ORDER BY id")
            .context("no se pudieron listar los álbumes sin carátula")?;
        sentencia
            .query_map([], |fila| fila.get(0))
            .context("no se pudieron listar los álbumes sin carátula")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer los álbumes sin carátula")?
    };
    let mut generadas = 0usize;
    for album_id in pendientes {
        if cancelacion.is_some_and(|bandera| bandera.load(Ordering::SeqCst)) {
            break;
        }
        match generar_para_album(conn, album_id, dir_cache) {
            Ok(true) => generadas += 1,
            Ok(false) => {}
            Err(error) => warn!(album_id, "no se pudo generar la carátula: {error:#}"),
        }
    }
    if generadas > 0 {
        info!(generadas, "carátulas generadas");
    }
    Ok(generadas)
}

fn generar_para_album(conn: &Connection, album_id: i64, dir_cache: &Path) -> Result<bool> {
    let rutas: Vec<String> = {
        let mut sentencia = conn
            .prepare(
                "SELECT ruta FROM PISTAS WHERE album_id = ?1 ORDER BY numero_disco, numero_pista",
            )
            .context("no se pudieron listar las pistas del álbum")?;
        sentencia
            .query_map([album_id], |fila| fila.get(0))
            .context("no se pudieron listar las pistas del álbum")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer las pistas del álbum")?
    };
    let mut datos = None;
    for ruta in &rutas {
        let ruta = Path::new(ruta);
        if !ruta.exists() {
            continue;
        }
        match extraer_embebida(ruta) {
            Ok(Some(imagen)) => {
                datos = Some(imagen);
                break;
            }
            Ok(None) => {}
            Err(error) => {
                warn!(ruta = %ruta.display(), "no se pudo leer la carátula embebida: {error:#}");
            }
        }
    }
    if datos.is_none()
        && let Some(primera) = rutas.first().map(Path::new)
        && let Some(carpeta) = primera.parent()
        && let Some(fichero) = buscar_fichero(carpeta)
    {
        datos = fs::read(&fichero).ok();
    }
    let Some(datos) = datos else {
        return Ok(false);
    };
    let ruta_cache = cachear(&datos, album_id, dir_cache)?;
    conn.execute(
        "UPDATE ALBUMES SET caratula_ruta = ?1 WHERE id = ?2",
        params![ruta_cache.to_string_lossy().to_string(), album_id],
    )
    .context("no se pudo actualizar la carátula del álbum")?;
    Ok(true)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn extrae_caratula_embebida_del_fixture() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let datos = extraer_embebida(&fixtures.join("prueba_caratula.mp3"))
            .expect("leer")
            .expect("carátula");
        assert!(!datos.is_empty());
        let sin_caratula = extraer_embebida(&fixtures.join("prueba.flac")).expect("leer");
        assert!(sin_caratula.is_none());
    }

    #[test]
    fn cachea_a_300x300_jpeg() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let datos = extraer_embebida(&fixtures.join("prueba_caratula.mp3"))
            .expect("leer")
            .expect("carátula");
        let temporal = tempfile::tempdir().expect("tempdir");
        let ruta = cachear(&datos, 7, temporal.path()).expect("cachear");
        assert_eq!(ruta.file_name().and_then(|s| s.to_str()), Some("7.jpg"));
        let imagen = image::open(&ruta).expect("abrir caché");
        assert_eq!(imagen.width(), LADO_CACHE);
        assert_eq!(imagen.height(), LADO_CACHE);
    }

    #[test]
    fn busca_cover_en_carpeta() {
        let temporal = tempfile::tempdir().expect("tempdir");
        std::fs::write(temporal.path().join("cancion.mp3"), b"x").expect("audio");
        std::fs::write(temporal.path().join("COVER.jpg"), b"x").expect("cover");
        let encontrado = buscar_fichero(temporal.path()).expect("cover");
        assert!(encontrado.ends_with("COVER.jpg"));

        let vacio = tempfile::tempdir().expect("tempdir");
        assert!(buscar_fichero(vacio.path()).is_none());
    }
}
