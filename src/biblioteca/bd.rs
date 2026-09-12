use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::Connection;
use tracing::info;

const MIGRACIONES: &[(i32, &str)] = &[(1, include_str!("migraciones/001_inicial.sql"))];

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

pub fn migrar(conn: &mut Connection) -> Result<i32> {
    let mut version: i32 = conn
        .query_row("PRAGMA user_version", [], |fila| fila.get(0))
        .context("no se pudo leer user_version")?;
    for (destino, sql) in MIGRACIONES {
        if *destino > version {
            let tx = conn
                .transaction()
                .context("no se pudo iniciar la transacción de migración")?;
            tx.execute_batch(sql)
                .with_context(|| format!("fallo la migración {destino}"))?;
            tx.pragma_update(None, "user_version", destino)
                .context("no se pudo actualizar user_version")?;
            tx.commit().context("no se pudo confirmar la migración")?;
            info!(version = destino, "migración aplicada");
            version = *destino;
        }
    }
    Ok(version)
}

pub fn ahora_iso() -> String {
    let segundos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    iso_desde_unix(segundos)
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
    fn migracion_crea_tablas_y_es_idempotente() {
        let mut conn = Connection::open_in_memory().expect("conexión en memoria");
        conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
        let version = migrar(&mut conn).expect("migración");
        assert_eq!(version, 1);
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
        let version2 = migrar(&mut conn).expect("segunda migración");
        assert_eq!(version2, 1);
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
             INSERT INTO HISTORIAL_REPRODUCCION (pista_id, reproducido_en, completada) VALUES (1, 'x', 0);",
        )
        .expect("datos");
        conn.execute("DELETE FROM PISTAS WHERE id = 1", [])
            .expect("borrado");
        for tabla in ["PLAYLIST_PISTAS", "COLA", "HISTORIAL_REPRODUCCION"] {
            let filas: i64 = conn
                .query_row(&format!("SELECT count(*) FROM {tabla}"), [], |f| f.get(0))
                .expect("consulta");
            assert_eq!(filas, 0, "la tabla {tabla} debería quedar vacía");
        }
    }
}
