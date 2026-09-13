use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use tracing::{info, warn};

pub const VERSION_RADIO: i32 = 4;

const MIGRACIONES: &[(i32, &str)] = &[
    (1, include_str!("migraciones/001_inicial.sql")),
    (2, include_str!("migraciones/002_recopilatorios_envios.sql")),
    (3, include_str!("migraciones/003_colores_albumes.sql")),
    (4, include_str!("migraciones/004_radio.sql")),
];

pub fn abrir(ruta: &Path) -> Result<Connection> {
    if let Some(padre) = ruta.parent() {
        std::fs::create_dir_all(padre)
            .with_context(|| format!("no se pudo crear el directorio {}", padre.display()))?;
    }
    let conn = Connection::open(ruta)
        .with_context(|| format!("no se pudo abrir la base de datos {}", ruta.display()))?;
    conn.busy_timeout(Duration::from_secs(5))
        .context("no se pudo fijar busy_timeout")?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .context("no se pudo activar el modo WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .context("no se pudo fijar synchronous")?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .context("no se pudieron activar las claves foráneas")?;
    Ok(conn)
}

/// Abre la base de datos y aplica las migraciones pendientes. Si la base está
/// en una versión anterior a la 4, antes de migrar deja una copia
/// `mmusic.db.pre-004` que solo se borra en el siguiente arranque mediante
/// `limpiar_copia_previa_si_migrada`.
pub fn abrir_y_migrar(ruta: &Path) -> Result<Connection> {
    crear_copia_previa_si_procede(ruta)?;
    let mut conn = abrir(ruta)?;
    migrar(&mut conn)?;
    Ok(conn)
}

fn ruta_copia_previa(ruta: &Path) -> PathBuf {
    let mut nombre = ruta.file_name().map(ToOwned::to_owned).unwrap_or_default();
    nombre.push(".pre-004");
    ruta.with_file_name(nombre)
}

fn version_si_existe(ruta: &Path) -> Result<Option<i32>> {
    if !ruta.exists() {
        return Ok(None);
    }
    let conn = Connection::open(ruta)
        .with_context(|| format!("no se pudo abrir la base de datos {}", ruta.display()))?;
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |fila| fila.get(0))
        .context("no se pudo leer user_version")?;
    Ok(Some(version))
}

/// Crea `mmusic.db.pre-004` (con la WAL volcada) solo si la base está en una
/// versión anterior a la 4 y aún no hay copia.
fn crear_copia_previa_si_procede(ruta: &Path) -> Result<()> {
    let Some(version) = version_si_existe(ruta)? else {
        return Ok(());
    };
    if !(1..VERSION_RADIO).contains(&version) {
        return Ok(());
    }
    let copia = ruta_copia_previa(ruta);
    if copia.exists() {
        return Ok(());
    }
    let conn = Connection::open(ruta)
        .with_context(|| format!("no se pudo abrir la base de datos {}", ruta.display()))?;
    conn.pragma_update(None, "wal_checkpoint", "TRUNCATE")
        .context("no se pudo volcar la WAL antes de copiar")?;
    drop(conn);
    std::fs::copy(ruta, &copia)
        .with_context(|| format!("no se pudo copiar {} antes de migrar", ruta.display()))?;
    info!(
        destino = %copia.display(),
        "copia previa a la migración 004 creada"
    );
    Ok(())
}

/// Borra `mmusic.db.pre-004` cuando la base ya está migrada; se llama una sola
/// vez en el arranque de la aplicación o de un subcomando.
pub fn limpiar_copia_previa_si_migrada(ruta: &Path) -> Result<()> {
    let Some(version) = version_si_existe(ruta)? else {
        return Ok(());
    };
    if version < VERSION_RADIO {
        return Ok(());
    }
    let copia = ruta_copia_previa(ruta);
    if copia.exists() {
        std::fs::remove_file(&copia)
            .with_context(|| format!("no se pudo borrar {}", copia.display()))?;
        info!("copia previa de la migración 004 eliminada");
    }
    Ok(())
}

pub fn migrar(conn: &mut Connection) -> Result<i32> {
    let mut version: i32 = conn
        .query_row("PRAGMA user_version", [], |fila| fila.get(0))
        .context("no se pudo leer user_version")?;
    let objetivo = MIGRACIONES.last().map(|(v, _)| *v).unwrap_or(0);
    if version >= objetivo {
        return Ok(version);
    }

    // La 004 recrea tablas: con las claves foráneas activas, el DROP implícito
    // dispararía SET NULL en ENVIOS y se perderían enlaces.
    conn.pragma_update(None, "foreign_keys", "OFF")
        .context("no se pudieron desactivar las claves foráneas para migrar")?;
    let resultado = aplicar_migraciones(conn, &mut version);
    if let Err(error) = resultado {
        if let Err(reactivar) = conn.pragma_update(None, "foreign_keys", "ON") {
            warn!("no se pudieron reactivar las claves foráneas: {reactivar:#}");
        }
        return Err(error);
    }
    if let Err(error) = comprobar_claves_foraneas(conn) {
        if let Err(reactivar) = conn.pragma_update(None, "foreign_keys", "ON") {
            warn!("no se pudieron reactivar las claves foráneas: {reactivar:#}");
        }
        return Err(error);
    }
    conn.pragma_update(None, "foreign_keys", "ON")
        .context("no se pudieron reactivar las claves foráneas")?;
    Ok(version)
}

fn aplicar_migraciones(conn: &mut Connection, version: &mut i32) -> Result<()> {
    for (destino, sql) in MIGRACIONES {
        if *destino > *version {
            let tx = conn
                .transaction()
                .context("no se pudo iniciar la transacción de migración")?;
            tx.execute_batch(sql)
                .with_context(|| format!("fallo la migración {destino}"))?;
            tx.pragma_update(None, "user_version", destino)
                .context("no se pudo actualizar user_version")?;
            tx.commit().context("no se pudo confirmar la migración")?;
            info!(version = destino, "migración aplicada");
            *version = *destino;
        }
    }
    Ok(())
}

fn comprobar_claves_foraneas(conn: &Connection) -> Result<()> {
    let mut stmt = conn
        .prepare("PRAGMA foreign_key_check")
        .context("no se pudo comprobar la integridad referencial")?;
    let mut filas = stmt
        .query([])
        .context("no se pudo comprobar la integridad referencial")?;
    if filas.next().context("consulta de integridad")?.is_some() {
        bail!("la migración dejó claves foráneas huérfanas");
    }
    Ok(())
}

pub fn ahora_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn ahora_iso() -> String {
    iso_desde_unix(ahora_unix())
}

pub fn iso_en(segundos: i64) -> String {
    iso_desde_unix(ahora_unix().saturating_add(segundos))
}

pub fn unix_desde_iso(texto: &str) -> Option<i64> {
    if texto.len() != 20
        || !texto.ends_with('Z')
        || texto.as_bytes().get(4) != Some(&b'-')
        || texto.as_bytes().get(7) != Some(&b'-')
        || texto.as_bytes().get(10) != Some(&b'T')
        || texto.as_bytes().get(13) != Some(&b':')
        || texto.as_bytes().get(16) != Some(&b':')
    {
        return None;
    }
    let anio: i64 = texto.get(0..4)?.parse().ok()?;
    let mes: u32 = texto.get(5..7)?.parse().ok()?;
    let dia: u32 = texto.get(8..10)?.parse().ok()?;
    let hora: i64 = texto.get(11..13)?.parse().ok()?;
    let minuto: i64 = texto.get(14..16)?.parse().ok()?;
    let segundo: i64 = texto.get(17..19)?.parse().ok()?;
    if !(1..=12).contains(&mes)
        || !(1..=31).contains(&dia)
        || hora > 23
        || minuto > 59
        || segundo > 59
    {
        return None;
    }
    Some(dias_desde_civil(anio, mes, dia) * 86_400 + hora * 3_600 + minuto * 60 + segundo)
}

fn dias_desde_civil(anio: i64, mes: u32, dia: u32) -> i64 {
    let y = if mes <= 2 { anio - 1 } else { anio };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if mes > 2 { mes - 3 } else { mes + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + dia as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn iso_desde_unix(segundos: i64) -> String {
    let dias = segundos.div_euclid(86_400);
    let resto = segundos.rem_euclid(86_400);
    let (anio, mes, dia) = civil_desde_dias(dias);
    let hora = resto / 3_600;
    let minuto = (resto % 3_600) / 60;
    let segundo = resto % 60;
    format!("{anio:04}-{mes:02}-{dia:02}T{hora:02}:{minuto:02}:{segundo:02}Z")
}

fn civil_desde_dias(dias: i64) -> (i64, u32, u32) {
    let z = dias + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut anio = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    if mes <= 2 {
        anio += 1;
    }
    (anio, mes, dia)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn iso8601_conocidos() {
        assert_eq!(iso_desde_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_desde_unix(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(iso_desde_unix(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(iso_desde_unix(1_735_689_599), "2024-12-31T23:59:59Z");
        assert_eq!(iso_desde_unix(1_735_689_600), "2025-01-01T00:00:00Z");
    }

    #[test]
    fn unix_desde_iso_es_inversa() {
        for segundos in [0, 1_000_000_000, 1_700_000_000, 1_735_689_599] {
            assert_eq!(unix_desde_iso(&iso_desde_unix(segundos)), Some(segundos));
        }
        assert_eq!(unix_desde_iso("2026-09-12T10:30:00Z"), Some(1_789_209_000));
        assert_eq!(unix_desde_iso("no es una fecha"), None);
        assert_eq!(unix_desde_iso("2026-13-01T00:00:00Z"), None);
        assert_eq!(unix_desde_iso("2026-09-12T25:00:00Z"), None);
    }

    #[test]
    fn migracion_crea_tablas_y_es_idempotente() {
        let mut conn = Connection::open_in_memory().expect("conexión en memoria");
        conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
        let version = migrar(&mut conn).expect("migración");
        assert_eq!(version, VERSION_RADIO);
        for tabla in [
            "ARTISTAS",
            "ALBUMES",
            "PISTAS",
            "PLAYLISTS",
            "PLAYLIST_PISTAS",
            "COLA",
            "HISTORIAL_REPRODUCCION",
            "ESCANEOS",
            "AJUSTES",
            "ENVIOS",
            "FAVORITAS",
            "EMISORAS",
            "EMISORA_TITULOS",
            "BUSQUEDAS_RADIO",
        ] {
            let existe: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [tabla],
                    |f| f.get(0),
                )
                .expect("consulta");
            assert_eq!(existe, 1, "falta la tabla {tabla}");
        }
        let pendiente: String = conn
            .query_row(
                "SELECT valor FROM AJUSTES WHERE clave = 'reescaneo_completo_pendiente'",
                [],
                |f| f.get(0),
            )
            .expect("bandera de reescaneo");
        assert_eq!(pendiente, "1");
        let version2 = migrar(&mut conn).expect("segunda migración");
        assert_eq!(version2, VERSION_RADIO);
        let colores: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('ALBUMES') WHERE name = 'colores'",
                [],
                |f| f.get(0),
            )
            .expect("columna colores");
        assert_eq!(colores, 1);
        let tipo: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('COLA') WHERE name = 'tipo'",
                [],
                |f| f.get(0),
            )
            .expect("columna tipo");
        assert_eq!(tipo, 1);
        let titulo_icy: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('HISTORIAL_REPRODUCCION')
                    WHERE name = 'titulo_icy'",
                [],
                |f| f.get(0),
            )
            .expect("columna titulo_icy");
        assert_eq!(titulo_icy, 1);
    }

    #[test]
    fn migracion_004_preserva_cola_historial_y_envios() {
        let mut conn = Connection::open_in_memory().expect("conexión");
        conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
        for (destino, sql) in MIGRACIONES.iter().take(3) {
            conn.execute_batch(sql).expect("migración previa");
            conn.pragma_update(None, "user_version", destino)
                .expect("version");
        }
        conn.execute_batch(
            "INSERT INTO ARTISTAS (id, nombre, nombre_norm, creado_en) VALUES (1, 'A', 'a', 'x');
             INSERT INTO ALBUMES (id, artista_id, titulo, titulo_norm, creado_en) VALUES (1, 1, 'B', 'b', 'x');
             INSERT INTO PISTAS (id, album_id, artista_id, titulo, titulo_norm, duracion_ms, ruta, formato, tamano_bytes, modificado_en, anadido_en)
                VALUES (1, 1, 1, 'C', 'c', 1000, '/x.mp3', 'mp3', 10, 1, 'x');
             INSERT INTO COLA (pista_id, posicion, posicion_orig) VALUES (1, 0, 0);
             INSERT INTO HISTORIAL_REPRODUCCION (pista_id, reproducido_en, completada) VALUES (1, '2026-01-01T00:00:00Z', 1);
             INSERT INTO ENVIOS (servicio, tipo, pista_id, historial_id, reproducido_en, proximo_intento_en, creado_en)
                VALUES ('lastfm', 'scrobble', 1, 1, '2026-01-01T00:00:00Z', 'x', 'x');",
        )
        .expect("datos previos");
        let version = migrar(&mut conn).expect("migración 004");
        assert_eq!(version, VERSION_RADIO);
        let cola: (i64, String) = conn
            .query_row("SELECT pista_id, tipo FROM COLA", [], |f| {
                Ok((f.get(0)?, f.get(1)?))
            })
            .expect("cola");
        assert_eq!(cola, (1, "pista".to_string()));
        let historial: i64 = conn
            .query_row(
                "SELECT count(*) FROM HISTORIAL_REPRODUCCION WHERE pista_id = 1 AND completada = 1",
                [],
                |f| f.get(0),
            )
            .expect("historial");
        assert_eq!(historial, 1);
        let envio: (i64, Option<i64>) = conn
            .query_row("SELECT pista_id, historial_id FROM ENVIOS", [], |f| {
                Ok((f.get(0)?, f.get(1)?))
            })
            .expect("envío");
        assert_eq!(envio, (1, Some(1)));
        let columnas_pista: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('ENVIOS') WHERE name = 'pista_id' AND \"notnull\" = 0",
                [],
                |f| f.get(0),
            )
            .expect("pista_id admite NULL");
        assert_eq!(columnas_pista, 1);
        conn.execute("DELETE FROM PISTAS WHERE id = 1", [])
            .expect("borrado");
        let envios: i64 = conn
            .query_row("SELECT count(*) FROM ENVIOS", [], |f| f.get(0))
            .expect("cascada");
        assert_eq!(envios, 0);
    }

    #[test]
    fn cascadas_borran_dependientes() {
        let mut conn = Connection::open_in_memory().expect("conexión");
        conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
        migrar(&mut conn).expect("migración");
        conn.execute_batch(
            "INSERT INTO ARTISTAS (id, nombre, nombre_norm, creado_en) VALUES (1, 'A', 'a', 'x');
             INSERT INTO ALBUMES (id, artista_id, titulo, titulo_norm, creado_en) VALUES (1, 1, 'B', 'b', 'x');
             INSERT INTO PISTAS (id, album_id, artista_id, titulo, titulo_norm, duracion_ms, ruta, formato, tamano_bytes, modificado_en, anadido_en)
                VALUES (1, 1, 1, 'C', 'c', 1000, '/x.mp3', 'mp3', 10, 1, 'x');
             INSERT INTO PLAYLISTS (id, nombre, creado_en, actualizado_en) VALUES (1, 'P', 'x', 'x');
             INSERT INTO PLAYLIST_PISTAS (playlist_id, pista_id, posicion) VALUES (1, 1, 0);
             INSERT INTO COLA (pista_id, posicion, posicion_orig) VALUES (1, 0, 0);
             INSERT INTO HISTORIAL_REPRODUCCION (pista_id, reproducido_en, completada) VALUES (1, 'x', 0);
             INSERT INTO ENVIOS (servicio, tipo, pista_id, historial_id, reproducido_en, proximo_intento_en, creado_en)
                VALUES ('lastfm', 'scrobble', 1, 1, 'x', 'x', 'x');
             INSERT INTO FAVORITAS (pista_id, marcada_en) VALUES (1, 'x');",
        )
        .expect("datos");
        conn.execute("DELETE FROM PISTAS WHERE id = 1", [])
            .expect("borrado");
        for tabla in [
            "PLAYLIST_PISTAS",
            "COLA",
            "HISTORIAL_REPRODUCCION",
            "ENVIOS",
            "FAVORITAS",
        ] {
            let filas: i64 = conn
                .query_row(&format!("SELECT count(*) FROM {tabla}"), [], |f| f.get(0))
                .expect("consulta");
            assert_eq!(filas, 0, "la tabla {tabla} debería quedar vacía");
        }
    }
}
