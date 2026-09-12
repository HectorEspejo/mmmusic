use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use tracing::{info, warn};

use super::escaner;
use super::{bd, etiquetas};

pub const ARTISTA_VARIOS: &str = "Varios artistas";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GrupoCandidato {
    pub carpeta: String,
    pub titulo_norm: String,
}

struct PistaGrupo {
    id: i64,
    artista_id: i64,
    album_id: i64,
    etiquetado: bool,
    titulo_album: String,
    anio: Option<i64>,
    caratula_ruta: Option<String>,
    varios_artistas: bool,
    ruta: String,
}

pub fn grupos_candidatos(conn: &Connection) -> Result<Vec<GrupoCandidato>> {
    let mut sentencia = conn
        .prepare(
            "SELECT DISTINCT p.carpeta, al.titulo_norm
               FROM PISTAS p
               JOIN ALBUMES al ON al.id = p.album_id",
        )
        .context("no se pudieron preparar los grupos de recopilatorios")?;
    sentencia
        .query_map([], |fila| {
            Ok(GrupoCandidato {
                carpeta: fila.get(0)?,
                titulo_norm: fila.get(1)?,
            })
        })
        .context("no se pudieron listar los grupos de recopilatorios")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer los grupos de recopilatorios")
}

pub fn consolidar(
    conn: &Connection,
    grupos: &[GrupoCandidato],
    releidas: &HashSet<String>,
) -> Result<()> {
    let mut convertidos = 0usize;
    let mut revertidos = 0usize;
    for grupo in grupos {
        let pistas = pistas_del_grupo(conn, grupo)?;
        if pistas.is_empty() {
            continue;
        }
        let artistas: HashSet<i64> = pistas.iter().map(|pista| pista.artista_id).collect();
        let es_recopilatorio = artistas.len() > 1 && pistas.iter().all(|pista| !pista.etiquetado);
        if es_recopilatorio {
            if mover_a_varios_artistas(conn, grupo, &pistas)? {
                convertidos += 1;
            }
        } else if pistas.iter().any(|pista| pista.varios_artistas)
            && revertir_a_albumes_normales(conn, &pistas, releidas)?
        {
            revertidos += 1;
        }
    }
    if convertidos > 0 || revertidos > 0 {
        info!(convertidos, revertidos, "consolidación de recopilatorios");
    }
    Ok(())
}

fn pistas_del_grupo(conn: &Connection, grupo: &GrupoCandidato) -> Result<Vec<PistaGrupo>> {
    let mut sentencia = conn
        .prepare(
            "SELECT p.id, p.artista_id, p.album_id, p.artista_album_etiquetado,
                    al.titulo, al.anio, al.caratula_ruta, al.varios_artistas,
                    p.ruta
               FROM PISTAS p
               JOIN ALBUMES al ON al.id = p.album_id
              WHERE p.carpeta = ?1 AND al.titulo_norm = ?2",
        )
        .context("no se pudieron preparar las pistas del grupo")?;
    sentencia
        .query_map(params![grupo.carpeta, grupo.titulo_norm], |fila| {
            Ok(PistaGrupo {
                id: fila.get(0)?,
                artista_id: fila.get(1)?,
                album_id: fila.get(2)?,
                etiquetado: fila.get(3)?,
                titulo_album: fila.get(4)?,
                anio: fila.get(5)?,
                caratula_ruta: fila.get(6)?,
                varios_artistas: fila.get(7)?,
                ruta: fila.get(8)?,
            })
        })
        .context("no se pudieron listar las pistas del grupo")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer las pistas del grupo")
}

fn mover_a_varios_artistas(
    conn: &Connection,
    grupo: &GrupoCandidato,
    pistas: &[PistaGrupo],
) -> Result<bool> {
    let artista_id = asegurar_artista_varios(conn)?;
    let titulo = pistas
        .iter()
        .find(|pista| !pista.varios_artistas)
        .or_else(|| pistas.first())
        .map(|pista| pista.titulo_album.clone())
        .unwrap_or_default();
    let anio = pistas.iter().filter_map(|pista| pista.anio).min();
    let caratula = pistas.iter().find_map(|pista| pista.caratula_ruta.clone());
    let album_id: i64 = conn
        .query_row(
            "INSERT INTO ALBUMES
                (artista_id, titulo, titulo_norm, anio, caratula_ruta, varios_artistas, carpeta, creado_en)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)
             ON CONFLICT(titulo_norm, carpeta) WHERE varios_artistas = 1
             DO UPDATE SET
                 anio = COALESCE(excluded.anio, ALBUMES.anio),
                 caratula_ruta = COALESCE(ALBUMES.caratula_ruta, excluded.caratula_ruta),
                 titulo = excluded.titulo
             RETURNING id",
            params![
                artista_id,
                titulo,
                grupo.titulo_norm,
                anio,
                caratula,
                grupo.carpeta,
                bd::ahora_iso()
            ],
            |fila| fila.get(0),
        )
        .with_context(|| format!("no se pudo registrar el recopilatorio {}", grupo.titulo_norm))?;
    let mut movidas = false;
    for pista in pistas {
        if pista.album_id != album_id {
            conn.execute(
                "UPDATE PISTAS SET album_id = ?1 WHERE id = ?2",
                params![album_id, pista.id],
            )
            .with_context(|| format!("no se pudo mover la pista {}", pista.id))?;
            movidas = true;
        }
    }
    Ok(movidas)
}

fn revertir_a_albumes_normales(
    conn: &Connection,
    pistas: &[PistaGrupo],
    releidas: &HashSet<String>,
) -> Result<bool> {
    let mut movidas = false;
    for pista in pistas.iter().filter(|pista| pista.varios_artistas) {
        let destino = album_normal_para_pista(conn, pista, releidas)?;
        if pista.album_id != destino.album_id
            || pista.artista_id != destino.artista_id
            || pista.etiquetado != destino.etiquetado
        {
            conn.execute(
                "UPDATE PISTAS
                    SET album_id = ?1, artista_id = ?2, artista_album_etiquetado = ?3
                  WHERE id = ?4",
                params![
                    destino.album_id,
                    destino.artista_id,
                    i64::from(destino.etiquetado),
                    pista.id
                ],
            )
            .with_context(|| format!("no se pudo devolver la pista {}", pista.id))?;
            movidas = true;
        }
    }
    Ok(movidas)
}

struct AlbumDestino {
    artista_id: i64,
    album_id: i64,
    etiquetado: bool,
}

fn album_normal_para_pista(
    conn: &Connection,
    pista: &PistaGrupo,
    releidas: &HashSet<String>,
) -> Result<AlbumDestino> {
    if !releidas.contains(&pista.ruta) {
        match releer_etiquetas(conn, pista) {
            Ok(Some(destino)) => return Ok(destino),
            Ok(None) => {}
            Err(error) => {
                warn!(
                    ruta = %pista.ruta,
                    "no se pudieron releer las etiquetas al revertir el recopilatorio: {error:#}"
                );
            }
        }
    }
    let album_id = asegurar_album_normal(conn, pista)?;
    Ok(AlbumDestino {
        artista_id: pista.artista_id,
        album_id,
        etiquetado: pista.etiquetado,
    })
}

fn releer_etiquetas(conn: &Connection, pista: &PistaGrupo) -> Result<Option<AlbumDestino>> {
    let ruta = Path::new(&pista.ruta);
    if !ruta.exists() {
        return Ok(None);
    }
    let crudas = etiquetas::leer(ruta)?;
    let etiquetas = etiquetas::resolver(crudas, ruta);
    let artista_id = escaner::asegurar_artista(conn, &etiquetas.artista)?;
    let album_artista_id = escaner::asegurar_artista(conn, &etiquetas.album_artista)?;
    let album_id = escaner::asegurar_album(conn, album_artista_id, &etiquetas)?;
    Ok(Some(AlbumDestino {
        artista_id,
        album_id,
        etiquetado: etiquetas.album_artista_etiquetado,
    }))
}

fn asegurar_album_normal(conn: &Connection, pista: &PistaGrupo) -> Result<i64> {
    conn.query_row(
        "INSERT INTO ALBUMES
            (artista_id, titulo, titulo_norm, anio, caratula_ruta, varios_artistas, carpeta, creado_en)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, NULL, ?6)
         ON CONFLICT(artista_id, titulo_norm) WHERE varios_artistas = 0
         DO UPDATE SET
             anio = COALESCE(ALBUMES.anio, excluded.anio),
             caratula_ruta = COALESCE(ALBUMES.caratula_ruta, excluded.caratula_ruta)
         RETURNING id",
        params![
            pista.artista_id,
            pista.titulo_album,
            etiquetas::normalizar(&pista.titulo_album),
            pista.anio,
            pista.caratula_ruta,
            bd::ahora_iso()
        ],
        |fila| fila.get(0),
    )
    .with_context(|| format!("no se pudo registrar el álbum de la pista {}", pista.id))
}

fn asegurar_artista_varios(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "INSERT INTO ARTISTAS (nombre, nombre_norm, creado_en) VALUES (?1, ?2, ?3)
         ON CONFLICT(nombre_norm) DO UPDATE SET nombre = excluded.nombre
         RETURNING id",
        params![
            ARTISTA_VARIOS,
            etiquetas::normalizar(ARTISTA_VARIOS),
            bd::ahora_iso()
        ],
        |fila| fila.get(0),
    )
    .context("no se pudo registrar el artista Varios artistas")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn nombre_normalizado_de_varios_artistas() {
        assert_eq!(etiquetas::normalizar(ARTISTA_VARIOS), "varios artistas");
    }
}
