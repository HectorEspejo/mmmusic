use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use super::bd;
use super::etiquetas;
use super::modelos::{
    AlbumResumen, ArtistaResumen, DetalleArtista, ElementoCola, Emisora, EmisoraResumen, Escaneo,
    EstadoEscaneo, Inicio, Pista, PistaListado, PistaResumen, PlaylistResumen, PresetEq,
    TituloEmisora,
};

pub const PAGINA_PISTAS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrdenPistas {
    Titulo,
    Artista,
    Album,
    Duracion,
    Anio,
}

impl OrdenPistas {
    pub const TODAS: [OrdenPistas; 5] = [
        OrdenPistas::Titulo,
        OrdenPistas::Artista,
        OrdenPistas::Album,
        OrdenPistas::Duracion,
        OrdenPistas::Anio,
    ];

    pub fn etiqueta(self) -> &'static str {
        match self {
            OrdenPistas::Titulo => "título",
            OrdenPistas::Artista => "artista",
            OrdenPistas::Album => "álbum",
            OrdenPistas::Duracion => "duración",
            OrdenPistas::Anio => "año",
        }
    }

    fn como_sql(self, descendente: bool) -> &'static str {
        match (self, descendente) {
            (OrdenPistas::Titulo, false) => "p.titulo_norm ASC",
            (OrdenPistas::Titulo, true) => "p.titulo_norm DESC",
            (OrdenPistas::Artista, false) => {
                "ar.nombre_norm ASC, al.titulo_norm ASC, p.numero_disco ASC, p.numero_pista ASC"
            }
            (OrdenPistas::Artista, true) => {
                "ar.nombre_norm DESC, al.titulo_norm DESC, p.numero_disco DESC, p.numero_pista DESC"
            }
            (OrdenPistas::Album, false) => {
                "al.titulo_norm ASC, p.numero_disco ASC, p.numero_pista ASC"
            }
            (OrdenPistas::Album, true) => {
                "al.titulo_norm DESC, p.numero_disco DESC, p.numero_pista DESC"
            }
            (OrdenPistas::Duracion, false) => "p.duracion_ms ASC",
            (OrdenPistas::Duracion, true) => "p.duracion_ms DESC",
            (OrdenPistas::Anio, false) => "al.anio IS NULL ASC, al.anio ASC",
            (OrdenPistas::Anio, true) => "al.anio IS NULL ASC, al.anio DESC",
        }
    }
}

pub fn listar_pistas(
    conn: &Connection,
    orden: OrdenPistas,
    descendente: bool,
    pagina: usize,
) -> Result<Vec<PistaListado>> {
    let sql = format!(
        "SELECT p.id, p.titulo, ar.nombre, al.titulo, al.id, p.duracion_ms, al.anio,
                p.formato, al.caratula_ruta, p.ruta
           FROM PISTAS p
           JOIN ARTISTAS ar ON ar.id = p.artista_id
           JOIN ALBUMES al ON al.id = p.album_id
          ORDER BY {}
          LIMIT ?1 OFFSET ?2",
        orden.como_sql(descendente)
    );
    let mut sentencia = conn
        .prepare(&sql)
        .context("no se pudo preparar el listado de pistas")?;
    let filas = sentencia
        .query_map(
            params![PAGINA_PISTAS as i64, (pagina * PAGINA_PISTAS) as i64],
            |fila| {
                Ok(PistaListado {
                    id: fila.get(0)?,
                    titulo: fila.get(1)?,
                    artista: fila.get(2)?,
                    album: fila.get(3)?,
                    album_id: fila.get(4)?,
                    duracion_ms: fila.get(5)?,
                    anio: fila.get(6)?,
                    formato: fila.get(7)?,
                    caratula_ruta: fila.get(8)?,
                    ruta: fila.get(9)?,
                })
            },
        )
        .context("no se pudo listar las pistas")?;
    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer las pistas")
}

pub fn ids_pistas(conn: &Connection, orden: OrdenPistas, descendente: bool) -> Result<Vec<i64>> {
    let sql = format!(
        "SELECT p.id
           FROM PISTAS p
           JOIN ARTISTAS ar ON ar.id = p.artista_id
           JOIN ALBUMES al ON al.id = p.album_id
          ORDER BY {}",
        orden.como_sql(descendente)
    );
    let mut sentencia = conn
        .prepare(&sql)
        .context("no se pudo preparar el listado de identificadores")?;
    let filas = sentencia
        .query_map([], |fila| fila.get(0))
        .context("no se pudo listar los identificadores de pista")?;
    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer los identificadores de pista")
}

pub fn pistas_resumen_por_ids(conn: &Connection, ids: &[i64]) -> Result<Vec<PistaResumen>> {
    let mut resumenes = Vec::with_capacity(ids.len());
    for lote in ids.chunks(900) {
        let marcadores = (1..=lote.len())
            .map(|indice| format!("?{indice}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT p.id, p.titulo, ar.nombre, al.titulo, al.id, p.duracion_ms,
                    al.caratula_ruta, p.ruta
               FROM PISTAS p
               JOIN ARTISTAS ar ON ar.id = p.artista_id
               JOIN ALBUMES al ON al.id = p.album_id
              WHERE p.id IN ({marcadores})"
        );
        let parametros = rusqlite::params_from_iter(lote.iter());
        let mut sentencia = conn
            .prepare(&sql)
            .context("no se pudo preparar el resumen de pistas")?;
        let filas = sentencia
            .query_map(parametros, |fila| {
                Ok(PistaResumen {
                    id: fila.get(0)?,
                    titulo: fila.get(1)?,
                    artista: fila.get(2)?,
                    album: fila.get(3)?,
                    album_id: fila.get(4)?,
                    duracion_ms: fila.get(5)?,
                    caratula_ruta: fila.get(6)?,
                    ruta: fila.get(7)?,
                })
            })
            .context("no se pudo consultar el resumen de pistas")?;
        let encontrados: Vec<PistaResumen> = filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer los resúmenes de pista")?;
        let mut por_id: std::collections::HashMap<i64, PistaResumen> =
            encontrados.into_iter().map(|p| (p.id, p)).collect();
        for id in lote {
            if let Some(pista) = por_id.remove(id) {
                resumenes.push(pista);
            }
        }
    }
    Ok(resumenes)
}

pub fn emisoras_resumen_por_ids(conn: &Connection, ids: &[i64]) -> Result<Vec<EmisoraResumen>> {
    let mut resumenes = Vec::with_capacity(ids.len());
    for lote in ids.chunks(900) {
        let marcadores = (1..=lote.len())
            .map(|indice| format!("?{indice}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, nombre, url, pais, codec, bitrate_kbps, logo_url, logo_ruta,
                    favorita, ultima_reproduccion
               FROM EMISORAS
              WHERE id IN ({marcadores})"
        );
        let parametros = rusqlite::params_from_iter(lote.iter());
        let mut sentencia = conn
            .prepare(&sql)
            .context("no se pudo preparar el resumen de emisoras")?;
        let filas = sentencia
            .query_map(parametros, emisora_resumen_desde_fila)
            .context("no se pudo consultar el resumen de emisoras")?;
        let encontrados: Vec<EmisoraResumen> = filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer los resúmenes de emisora")?;
        let mut por_id: std::collections::HashMap<i64, EmisoraResumen> =
            encontrados.into_iter().map(|e| (e.id, e)).collect();
        for id in lote {
            if let Some(emisora) = por_id.remove(id) {
                resumenes.push(emisora);
            }
        }
    }
    Ok(resumenes)
}

pub fn emisora_resumen_desde_fila(fila: &rusqlite::Row<'_>) -> rusqlite::Result<EmisoraResumen> {
    Ok(EmisoraResumen {
        id: fila.get(0)?,
        nombre: fila.get(1)?,
        url: fila.get(2)?,
        pais: fila.get(3)?,
        codec: fila.get(4)?,
        bitrate_kbps: fila.get(5)?,
        logo_url: fila.get(6)?,
        logo_ruta: fila.get(7)?,
        favorita: fila.get::<_, i64>(8)? != 0,
        ultima_reproduccion: fila.get(9)?,
    })
}

pub fn existe_pista_por_artista_titulo(
    conn: &Connection,
    artista: &str,
    titulo: &str,
) -> Result<bool> {
    let artista_norm = etiquetas::normalizar(artista);
    let titulo_norm = etiquetas::normalizar(titulo);
    if artista_norm.is_empty() || titulo_norm.is_empty() {
        return Ok(false);
    }
    let existe: i64 = conn
        .query_row(
            "SELECT count(*)
               FROM PISTAS p
               JOIN ARTISTAS ar ON ar.id = p.artista_id
              WHERE ar.nombre_norm = ?1 AND p.titulo_norm = ?2",
            params![artista_norm, titulo_norm],
            |fila| fila.get(0),
        )
        .context("no se pudo comprobar el título en la biblioteca")?;
    Ok(existe > 0)
}

pub fn contar_pistas(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT count(*) FROM PISTAS", [], |f| f.get(0))
        .context("no se pudieron contar las pistas")
}

pub fn contar_albumes(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT count(*) FROM ALBUMES", [], |f| f.get(0))
        .context("no se pudieron contar los álbumes")
}

pub fn contar_artistas(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT count(*) FROM ARTISTAS", [], |f| f.get(0))
        .context("no se pudieron contar los artistas")
}

pub mod escaneos {
    use super::*;

    pub fn iniciar(conn: &Connection) -> Result<i64> {
        conn.query_row(
            "INSERT INTO ESCANEOS (iniciado_en, estado) VALUES (?1, 'en_curso') RETURNING id",
            [bd::ahora_iso()],
            |f| f.get(0),
        )
        .context("no se pudo registrar el inicio del escaneo")
    }

    pub fn finalizar(
        conn: &Connection,
        id: i64,
        estado: EstadoEscaneo,
        nuevas: u64,
        actualizadas: u64,
        eliminadas: u64,
        error_msg: Option<&str>,
    ) -> Result<()> {
        conn.execute(
            "UPDATE ESCANEOS
                SET finalizado_en = ?1, estado = ?2, nuevas = ?3, actualizadas = ?4,
                    eliminadas = ?5, error_msg = ?6
              WHERE id = ?7",
            params![
                bd::ahora_iso(),
                estado.como_str(),
                nuevas as i64,
                actualizadas as i64,
                eliminadas as i64,
                error_msg,
                id
            ],
        )
        .context("no se pudo finalizar el escaneo")?;
        Ok(())
    }

    pub fn marcar_huerfanos(conn: &Connection) -> Result<usize> {
        let filas = conn
            .execute(
                "UPDATE ESCANEOS
                    SET estado = 'error', finalizado_en = ?1, error_msg = 'proceso interrumpido'
                  WHERE estado = 'en_curso'",
                [bd::ahora_iso()],
            )
            .context("no se pudieron marcar los escaneos huérfanos")?;
        Ok(filas)
    }

    pub fn ultimo(conn: &Connection) -> Result<Option<Escaneo>> {
        conn.query_row(
            "SELECT id, iniciado_en, finalizado_en, estado, nuevas, actualizadas, eliminadas, error_msg
               FROM ESCANEOS ORDER BY id DESC LIMIT 1",
            [],
            desde_fila,
        )
        .optional()
        .context("no se pudo leer el último escaneo")
    }

    fn desde_fila(fila: &rusqlite::Row<'_>) -> rusqlite::Result<Escaneo> {
        let estado: String = fila.get(3)?;
        Ok(Escaneo {
            id: fila.get(0)?,
            iniciado_en: fila.get(1)?,
            finalizado_en: fila.get(2)?,
            estado: EstadoEscaneo::desde_str(&estado).unwrap_or(EstadoEscaneo::Error),
            nuevas: fila.get(4)?,
            actualizadas: fila.get(5)?,
            eliminadas: fila.get(6)?,
            error_msg: fila.get(7)?,
        })
    }
}

pub mod ajustes {
    use super::*;

    pub fn leer(conn: &Connection, clave: &str) -> Result<Option<String>> {
        conn.query_row("SELECT valor FROM AJUSTES WHERE clave = ?1", [clave], |f| {
            f.get(0)
        })
        .optional()
        .with_context(|| format!("no se pudo leer el ajuste {clave}"))
    }

    pub fn escribir(conn: &Connection, clave: &str, valor: &str) -> Result<()> {
        conn.execute(
            "INSERT INTO AJUSTES (clave, valor) VALUES (?1, ?2)
             ON CONFLICT(clave) DO UPDATE SET valor = excluded.valor",
            params![clave, valor],
        )
        .with_context(|| format!("no se pudo escribir el ajuste {clave}"))?;
        Ok(())
    }

    pub fn leer_i64(conn: &Connection, clave: &str) -> Result<Option<i64>> {
        Ok(leer(conn, clave)?.and_then(|v| v.parse().ok()))
    }

    pub fn leer_bool(conn: &Connection, clave: &str) -> Result<Option<bool>> {
        Ok(leer(conn, clave)?.map(|v| v == "1"))
    }

    pub fn reescaneo_completo_pendiente(conn: &Connection) -> Result<bool> {
        Ok(leer_bool(conn, "reescaneo_completo_pendiente")?.unwrap_or(false))
    }

    pub fn fijar_reescaneo_completo(conn: &Connection, pendiente: bool) -> Result<()> {
        escribir(
            conn,
            "reescaneo_completo_pendiente",
            if pendiente { "1" } else { "0" },
        )
    }

    /// Siembra claves que aún no existan; los valores dependientes de la
    /// configuración se fijan por código, nunca como constantes SQL.
    pub fn sembrar(conn: &Connection, pares: &[(&str, &str)]) -> Result<()> {
        for (clave, valor) in pares {
            conn.execute(
                "INSERT INTO AJUSTES (clave, valor) VALUES (?1, ?2)
                 ON CONFLICT(clave) DO NOTHING",
                params![clave, valor],
            )
            .with_context(|| format!("no se pudo sembrar el ajuste {clave}"))?;
        }
        Ok(())
    }
}

pub mod presets_eq {
    use super::*;
    use crate::ecualizador::{acotar_db, acotar_preamp, presets};

    fn desde_fila(fila: &rusqlite::Row<'_>) -> rusqlite::Result<PresetEq> {
        let id: i64 = fila.get(0)?;
        let nombre: String = fila.get(1)?;
        let ganancias_texto: String = fila.get(2)?;
        let preamp_db: f64 = fila.get(3)?;
        let integrado: i64 = fila.get(4)?;
        let ganancias = presets::parsear_ganancias(&ganancias_texto).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ganancias de preset corruptas",
                )),
            )
        })?;
        Ok(PresetEq {
            id,
            nombre,
            ganancias,
            preamp_db: preamp_db as f32,
            integrado: integrado != 0,
        })
    }

    pub fn listar(conn: &Connection) -> Result<Vec<PresetEq>> {
        let mut sentencia = conn
            .prepare(
                "SELECT id, nombre, ganancias, preamp_db, integrado FROM PRESETS_EQ
                  ORDER BY integrado DESC, nombre_norm ASC",
            )
            .context("no se pudieron preparar los presets")?;
        sentencia
            .query_map([], desde_fila)
            .context("no se pudieron listar los presets")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer los presets")
    }

    pub fn obtener(conn: &Connection, id: i64) -> Result<Option<PresetEq>> {
        conn.query_row(
            "SELECT id, nombre, ganancias, preamp_db, integrado FROM PRESETS_EQ WHERE id = ?1",
            [id],
            desde_fila,
        )
        .optional()
        .with_context(|| format!("no se pudo leer el preset {id}"))
    }

    /// Crea un preset propio. Devuelve `None` si el nombre ya existe
    /// (normalizado), incluidos los integrados. Valida y acota los valores.
    pub fn crear(
        conn: &Connection,
        nombre: &str,
        ganancias: &[f32; crate::ecualizador::BANDAS],
        preamp_db: f32,
    ) -> Result<Option<i64>> {
        let nombre = presets::validar_nombre(nombre).map_err(|mensaje| anyhow::anyhow!(mensaje))?;
        let nombre_norm = etiquetas::normalizar(&nombre);
        let existe: Option<i64> = conn
            .query_row(
                "SELECT id FROM PRESETS_EQ WHERE nombre_norm = ?1",
                [&nombre_norm],
                |fila| fila.get(0),
            )
            .optional()
            .with_context(|| format!("no se pudo comprobar el preset {nombre}"))?;
        if existe.is_some() {
            return Ok(None);
        }
        let ganancias: [f32; crate::ecualizador::BANDAS] =
            std::array::from_fn(|indice| acotar_db(ganancias[indice]));
        let preamp_db = acotar_preamp(preamp_db);
        conn.execute(
            "INSERT INTO PRESETS_EQ (nombre, nombre_norm, ganancias, preamp_db, integrado, creado_en)
             VALUES (?1, ?2, ?3, ?4, 0, ?5)",
            params![
                nombre,
                nombre_norm,
                presets::formatear_ganancias(&ganancias),
                f64::from(preamp_db),
                bd::ahora_iso(),
            ],
        )
        .with_context(|| format!("no se pudo crear el preset {nombre}"))?;
        Ok(Some(conn.last_insert_rowid()))
    }

    /// Elimina un preset propio. Los integrados no se pueden borrar.
    pub fn eliminar(conn: &Connection, id: i64) -> Result<bool> {
        let filas = conn
            .execute(
                "DELETE FROM PRESETS_EQ WHERE id = ?1 AND integrado = 0",
                [id],
            )
            .with_context(|| format!("no se pudo eliminar el preset {id}"))?;
        Ok(filas > 0)
    }

    /// Crea los presets de fábrica que falten. Devuelve cuántos sembró.
    pub fn sembrar_integrados(conn: &Connection) -> Result<usize> {
        let mut sembrados = 0;
        for preset in &presets::INTEGRADOS {
            let nombre_norm = etiquetas::normalizar(preset.nombre);
            let filas = conn
                .execute(
                    "INSERT INTO PRESETS_EQ
                        (nombre, nombre_norm, ganancias, preamp_db, integrado, creado_en)
                     VALUES (?1, ?2, ?3, ?4, 1, ?5)
                     ON CONFLICT(nombre_norm) DO NOTHING",
                    params![
                        preset.nombre,
                        nombre_norm,
                        presets::formatear_ganancias(&preset.ganancias),
                        f64::from(preset.preamp_db),
                        bd::ahora_iso(),
                    ],
                )
                .with_context(|| format!("no se pudo sembrar el preset {}", preset.nombre))?;
            sembrados += filas;
        }
        Ok(sembrados)
    }
}

pub mod letras {
    use super::*;

    pub fn offset(conn: &Connection, pista_id: i64) -> Result<i64> {
        conn.query_row(
            "SELECT offset_ms FROM LETRAS WHERE pista_id = ?1",
            [pista_id],
            |fila| fila.get(0),
        )
        .optional()
        .with_context(|| format!("no se pudo leer el offset de la pista {pista_id}"))
        .map(|valor| valor.unwrap_or(0))
    }

    pub fn fijar_offset(conn: &Connection, pista_id: i64, offset_ms: i64) -> Result<()> {
        let offset_ms = offset_ms.clamp(-30_000, 30_000);
        conn.execute(
            "INSERT INTO LETRAS (pista_id, offset_ms, actualizado_en) VALUES (?1, ?2, ?3)
             ON CONFLICT(pista_id) DO UPDATE SET
                 offset_ms = excluded.offset_ms,
                 actualizado_en = excluded.actualizado_en",
            params![pista_id, offset_ms, bd::ahora_iso()],
        )
        .with_context(|| format!("no se pudo guardar el offset de la pista {pista_id}"))?;
        Ok(())
    }

    pub fn fuente_preferida(conn: &Connection, pista_id: i64) -> Result<Option<String>> {
        conn.query_row(
            "SELECT fuente_preferida FROM LETRAS WHERE pista_id = ?1",
            [pista_id],
            |fila| fila.get(0),
        )
        .optional()
        .with_context(|| format!("no se pudo leer la fuente de la pista {pista_id}"))
        .map(Option::flatten)
    }

    pub fn fijar_fuente(conn: &Connection, pista_id: i64, fuente: &str) -> Result<()> {
        conn.execute(
            "INSERT INTO LETRAS (pista_id, offset_ms, fuente_preferida, actualizado_en)
             VALUES (?1, 0, ?2, ?3)
             ON CONFLICT(pista_id) DO UPDATE SET
                 fuente_preferida = excluded.fuente_preferida,
                 actualizado_en = excluded.actualizado_en",
            params![pista_id, fuente, bd::ahora_iso()],
        )
        .with_context(|| format!("no se pudo guardar la fuente de la pista {pista_id}"))?;
        Ok(())
    }
}

pub mod albumes {
    use super::*;

    pub fn colores(conn: &Connection, album_id: i64) -> Result<Option<String>> {
        conn.query_row(
            "SELECT colores FROM ALBUMES WHERE id = ?1",
            [album_id],
            |fila| fila.get(0),
        )
        .optional()
        .with_context(|| format!("no se pudieron leer los colores del álbum {album_id}"))
        .map(Option::flatten)
    }

    pub fn fijar_colores(conn: &Connection, album_id: i64, colores: &str) -> Result<()> {
        conn.execute(
            "UPDATE ALBUMES SET colores = ?1 WHERE id = ?2",
            params![colores, album_id],
        )
        .with_context(|| format!("no se pudieron guardar los colores del álbum {album_id}"))?;
        Ok(())
    }

    pub fn sin_colores(conn: &Connection, limite: usize) -> Result<Vec<(i64, String)>> {
        let mut sentencia = conn
            .prepare(
                "SELECT id, caratula_ruta FROM ALBUMES
                  WHERE colores IS NULL AND caratula_ruta IS NOT NULL
                  ORDER BY id
                  LIMIT ?1",
            )
            .context("no se pudieron preparar los álbumes sin colores")?;
        sentencia
            .query_map([limite as i64], |fila| Ok((fila.get(0)?, fila.get(1)?)))
            .context("no se pudieron listar los álbumes sin colores")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer los álbumes sin colores")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrdenAlbumes {
    Titulo,
    Artista,
    Anio,
    Anadido,
}

impl OrdenAlbumes {
    pub const TODAS: [OrdenAlbumes; 4] = [
        OrdenAlbumes::Titulo,
        OrdenAlbumes::Artista,
        OrdenAlbumes::Anio,
        OrdenAlbumes::Anadido,
    ];

    pub fn etiqueta(self) -> &'static str {
        match self {
            OrdenAlbumes::Titulo => "título",
            OrdenAlbumes::Artista => "artista",
            OrdenAlbumes::Anio => "año",
            OrdenAlbumes::Anadido => "añadido",
        }
    }

    fn como_sql(self, descendente: bool) -> &'static str {
        match (self, descendente) {
            (OrdenAlbumes::Titulo, false) => "al.titulo_norm ASC",
            (OrdenAlbumes::Titulo, true) => "al.titulo_norm DESC",
            (OrdenAlbumes::Artista, false) => "ar.nombre_norm ASC, al.titulo_norm ASC",
            (OrdenAlbumes::Artista, true) => "ar.nombre_norm DESC, al.titulo_norm DESC",
            (OrdenAlbumes::Anio, false) => "al.anio IS NULL ASC, al.anio ASC, al.titulo_norm ASC",
            (OrdenAlbumes::Anio, true) => "al.anio IS NULL ASC, al.anio DESC, al.titulo_norm DESC",
            (OrdenAlbumes::Anadido, false) => "MIN(p.anadido_en) ASC",
            (OrdenAlbumes::Anadido, true) => "MIN(p.anadido_en) DESC",
        }
    }
}

pub fn listar_artistas(conn: &Connection) -> Result<Vec<ArtistaResumen>> {
    let mut sentencia = conn
        .prepare(
            "SELECT ar.id, ar.nombre,
                    (SELECT count(*) FROM ALBUMES al WHERE al.artista_id = ar.id),
                    (SELECT count(*) FROM PISTAS p WHERE p.artista_id = ar.id)
               FROM ARTISTAS ar
              ORDER BY ar.nombre_norm",
        )
        .context("no se pudo preparar el listado de artistas")?;
    let filas = sentencia
        .query_map([], |fila| {
            Ok(ArtistaResumen {
                id: fila.get(0)?,
                nombre: fila.get(1)?,
                num_albumes: fila.get(2)?,
                num_pistas: fila.get(3)?,
            })
        })
        .context("no se pudo listar los artistas")?;
    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer los artistas")
}

pub fn detalle_artista(conn: &Connection, artista_id: i64) -> Result<Option<DetalleArtista>> {
    let resumen = conn
        .query_row(
            "SELECT ar.id, ar.nombre,
                    (SELECT count(*) FROM ALBUMES al WHERE al.artista_id = ar.id),
                    (SELECT count(*) FROM PISTAS p WHERE p.artista_id = ar.id)
               FROM ARTISTAS ar WHERE ar.id = ?1",
            [artista_id],
            |fila| {
                Ok(ArtistaResumen {
                    id: fila.get(0)?,
                    nombre: fila.get(1)?,
                    num_albumes: fila.get(2)?,
                    num_pistas: fila.get(3)?,
                })
            },
        )
        .optional()
        .context("no se pudo leer el artista")?;
    let Some(artista) = resumen else {
        return Ok(None);
    };

    let mut sentencia = conn
        .prepare(
            "SELECT al.id, al.titulo, ar.nombre, al.anio, al.caratula_ruta,
                    count(p.id), coalesce(sum(p.duracion_ms), 0)
               FROM ALBUMES al
               JOIN ARTISTAS ar ON ar.id = al.artista_id
               LEFT JOIN PISTAS p ON p.album_id = al.id
              WHERE al.artista_id = ?1
              GROUP BY al.id
              ORDER BY al.anio IS NULL ASC, al.anio DESC, al.titulo_norm ASC",
        )
        .context("no se pudo preparar los álbumes del artista")?;
    let albumes = sentencia
        .query_map([artista_id], |fila| {
            Ok(AlbumResumen {
                id: fila.get(0)?,
                titulo: fila.get(1)?,
                artista: fila.get(2)?,
                anio: fila.get(3)?,
                caratula_ruta: fila.get(4)?,
                num_pistas: fila.get(5)?,
                duracion_ms: fila.get(6)?,
            })
        })
        .context("no se pudieron listar los álbumes del artista")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer los álbumes del artista")?;

    let mut sentencia = conn
        .prepare(
            "SELECT p.id, p.titulo, ar.nombre, al.titulo, al.id, p.duracion_ms, al.anio,
                    p.formato, al.caratula_ruta, p.ruta
               FROM PISTAS p
               JOIN ARTISTAS ar ON ar.id = p.artista_id
               JOIN ALBUMES al ON al.id = p.album_id
              WHERE p.artista_id = ?1 AND al.artista_id <> ?1
              ORDER BY al.titulo_norm, p.numero_disco, p.numero_pista, p.titulo_norm",
        )
        .context("no se pudo preparar las pistas sueltas")?;
    let pistas_sueltas = sentencia
        .query_map([artista_id], pista_listado_desde_fila)
        .context("no se pudieron listar las pistas sueltas")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer las pistas sueltas")?;

    Ok(Some(DetalleArtista {
        artista,
        albumes,
        pistas_sueltas,
    }))
}

pub fn listar_albumes(
    conn: &Connection,
    orden: OrdenAlbumes,
    descendente: bool,
    filtro_artista: Option<i64>,
) -> Result<Vec<AlbumResumen>> {
    let orden_sql = orden.como_sql(descendente);
    match filtro_artista {
        Some(artista_id) => {
            let sql = format!(
                "SELECT al.id, al.titulo, ar.nombre, al.anio, al.caratula_ruta,
                        count(p.id), coalesce(sum(p.duracion_ms), 0)
                   FROM ALBUMES al
                   JOIN ARTISTAS ar ON ar.id = al.artista_id
                   LEFT JOIN PISTAS p ON p.album_id = al.id
                  WHERE al.artista_id = ?1
                  GROUP BY al.id
                  ORDER BY {orden_sql}"
            );
            let mut sentencia = conn
                .prepare(&sql)
                .context("no se pudo preparar el listado de álbumes")?;
            sentencia
                .query_map([artista_id], album_resumen_desde_fila)
                .context("no se pudieron listar los álbumes")?
                .collect::<rusqlite::Result<Vec<_>>>()
                .context("no se pudieron leer los álbumes")
        }
        None => {
            let sql = format!(
                "SELECT al.id, al.titulo, ar.nombre, al.anio, al.caratula_ruta,
                        count(p.id), coalesce(sum(p.duracion_ms), 0)
                   FROM ALBUMES al
                   JOIN ARTISTAS ar ON ar.id = al.artista_id
                   JOIN PISTAS p ON p.album_id = al.id
                  GROUP BY al.id
                  ORDER BY {orden_sql}"
            );
            let mut sentencia = conn
                .prepare(&sql)
                .context("no se pudo preparar el listado de álbumes")?;
            sentencia
                .query_map([], album_resumen_desde_fila)
                .context("no se pudieron listar los álbumes")?
                .collect::<rusqlite::Result<Vec<_>>>()
                .context("no se pudieron leer los álbumes")
        }
    }
}

pub fn detalle_album(
    conn: &Connection,
    album_id: i64,
) -> Result<Option<(AlbumResumen, Vec<Pista>)>> {
    let resumen = conn
        .query_row(
            "SELECT al.id, al.titulo, ar.nombre, al.anio, al.caratula_ruta,
                    count(p.id), coalesce(sum(p.duracion_ms), 0)
               FROM ALBUMES al
               JOIN ARTISTAS ar ON ar.id = al.artista_id
               LEFT JOIN PISTAS p ON p.album_id = al.id
              WHERE al.id = ?1
              GROUP BY al.id",
            [album_id],
            album_resumen_desde_fila,
        )
        .optional()
        .context("no se pudo leer el álbum")?;
    let Some(album) = resumen else {
        return Ok(None);
    };
    let mut sentencia = conn
        .prepare(
            "SELECT id, album_id, artista_id, titulo, titulo_norm, numero_pista, numero_disco,
                    genero, carpeta, artista_album_etiquetado, duracion_ms, ruta, formato,
                    tamano_bytes, modificado_en, bitrate_kbps, anadido_en, escaneo_id
               FROM PISTAS
              WHERE album_id = ?1
              ORDER BY numero_disco IS NULL, numero_disco, numero_pista IS NULL,
                       numero_pista, titulo_norm",
        )
        .context("no se pudo preparar las pistas del álbum")?;
    let pistas = sentencia
        .query_map([album_id], |fila| {
            Ok(Pista {
                id: fila.get(0)?,
                album_id: fila.get(1)?,
                artista_id: fila.get(2)?,
                titulo: fila.get(3)?,
                titulo_norm: fila.get(4)?,
                numero_pista: fila.get(5)?,
                numero_disco: fila.get(6)?,
                genero: fila.get(7)?,
                carpeta: fila.get(8)?,
                artista_album_etiquetado: fila.get(9)?,
                duracion_ms: fila.get(10)?,
                ruta: fila.get(11)?,
                formato: fila.get(12)?,
                tamano_bytes: fila.get(13)?,
                modificado_en: fila.get(14)?,
                bitrate_kbps: fila.get(15)?,
                anadido_en: fila.get(16)?,
                escaneo_id: fila.get(17)?,
            })
        })
        .context("no se pudieron listar las pistas del álbum")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer las pistas del álbum")?;
    Ok(Some((album, pistas)))
}

pub fn ids_pistas_album(conn: &Connection, album_id: i64) -> Result<Vec<i64>> {
    let mut sentencia = conn
        .prepare(
            "SELECT id FROM PISTAS
              WHERE album_id = ?1
              ORDER BY numero_disco IS NULL, numero_disco, numero_pista IS NULL,
                       numero_pista, titulo_norm",
        )
        .context("no se pudo preparar las pistas del álbum")?;
    sentencia
        .query_map([album_id], |fila| fila.get(0))
        .context("no se pudieron listar las pistas del álbum")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer las pistas del álbum")
}

pub fn ids_pistas_artista(conn: &Connection, artista_id: i64) -> Result<Vec<i64>> {
    let mut sentencia = conn
        .prepare(
            "SELECT p.id
               FROM PISTAS p
               JOIN ALBUMES al ON al.id = p.album_id
              WHERE p.artista_id = ?1
              ORDER BY al.anio IS NULL, al.anio, al.titulo_norm,
                       p.numero_disco, p.numero_pista, p.titulo_norm",
        )
        .context("no se pudo preparar las pistas del artista")?;
    sentencia
        .query_map([artista_id], |fila| fila.get(0))
        .context("no se pudieron listar las pistas del artista")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer las pistas del artista")
}

pub fn historial_reciente(conn: &Connection, limite: usize) -> Result<Vec<PistaListado>> {
    let mut sentencia = conn
        .prepare(
            "SELECT p.id, p.titulo, ar.nombre, al.titulo, al.id, p.duracion_ms, al.anio,
                    p.formato, al.caratula_ruta, p.ruta
               FROM PISTAS p
               JOIN (SELECT pista_id, MAX(reproducido_en) AS ultima
                       FROM HISTORIAL_REPRODUCCION
                      GROUP BY pista_id
                      ORDER BY ultima DESC
                      LIMIT ?1) h ON h.pista_id = p.id
               JOIN ARTISTAS ar ON ar.id = p.artista_id
               JOIN ALBUMES al ON al.id = p.album_id
              ORDER BY h.ultima DESC",
        )
        .context("no se pudo preparar el historial reciente")?;
    sentencia
        .query_map([limite as i64], pista_listado_desde_fila)
        .context("no se pudo consultar el historial reciente")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudo leer el historial reciente")
}

pub fn inicio(conn: &Connection) -> Result<Inicio> {
    let recientes = historial_reciente(conn, 10)?;
    let anadidos = listar_albumes(conn, OrdenAlbumes::Anadido, true, None)?
        .into_iter()
        .take(10)
        .collect();
    let mut sentencia = conn
        .prepare(
            "SELECT al.id, al.titulo, ar.nombre, al.anio, al.caratula_ruta,
                    count(p.id), coalesce(sum(p.duracion_ms), 0)
               FROM ALBUMES al
               JOIN ARTISTAS ar ON ar.id = al.artista_id
               JOIN PISTAS p ON p.album_id = al.id
              GROUP BY al.id
              ORDER BY RANDOM()
              LIMIT 10",
        )
        .context("no se pudo preparar el descubrimiento aleatorio")?;
    let redescubre = sentencia
        .query_map([], album_resumen_desde_fila)
        .context("no se pudieron leer los álbumes al azar")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer los álbumes al azar")?;
    Ok(Inicio {
        recientes,
        anadidos,
        redescubre,
    })
}

fn pista_listado_desde_fila(fila: &rusqlite::Row<'_>) -> rusqlite::Result<PistaListado> {
    Ok(PistaListado {
        id: fila.get(0)?,
        titulo: fila.get(1)?,
        artista: fila.get(2)?,
        album: fila.get(3)?,
        album_id: fila.get(4)?,
        duracion_ms: fila.get(5)?,
        anio: fila.get(6)?,
        formato: fila.get(7)?,
        caratula_ruta: fila.get(8)?,
        ruta: fila.get(9)?,
    })
}

fn album_resumen_desde_fila(fila: &rusqlite::Row<'_>) -> rusqlite::Result<AlbumResumen> {
    Ok(AlbumResumen {
        id: fila.get(0)?,
        titulo: fila.get(1)?,
        artista: fila.get(2)?,
        anio: fila.get(3)?,
        caratula_ruta: fila.get(4)?,
        num_pistas: fila.get(5)?,
        duracion_ms: fila.get(6)?,
    })
}

pub mod playlist {
    use std::path::{Path, PathBuf};

    use super::*;

    pub fn listar(conn: &Connection) -> Result<Vec<PlaylistResumen>> {
        let mut sentencia = conn
            .prepare(
                "SELECT pl.id, pl.nombre, count(pp.id)
                   FROM PLAYLISTS pl
                   LEFT JOIN PLAYLIST_PISTAS pp ON pp.playlist_id = pl.id
                  GROUP BY pl.id
                  ORDER BY pl.nombre COLLATE NOCASE",
            )
            .context("no se pudo preparar el listado de playlists")?;
        sentencia
            .query_map([], |fila| {
                Ok(PlaylistResumen {
                    id: fila.get(0)?,
                    nombre: fila.get(1)?,
                    num_pistas: fila.get(2)?,
                })
            })
            .context("no se pudieron listar las playlists")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer las playlists")
    }

    pub fn pistas(conn: &Connection, playlist_id: i64) -> Result<Vec<PistaListado>> {
        let mut sentencia = conn
            .prepare(
                "SELECT p.id, p.titulo, ar.nombre, al.titulo, al.id, p.duracion_ms, al.anio,
                        p.formato, al.caratula_ruta, p.ruta
                   FROM PLAYLIST_PISTAS pp
                   JOIN PISTAS p ON p.id = pp.pista_id
                   JOIN ARTISTAS ar ON ar.id = p.artista_id
                   JOIN ALBUMES al ON al.id = p.album_id
                  WHERE pp.playlist_id = ?1
                  ORDER BY pp.posicion",
            )
            .context("no se pudieron preparar las pistas de la playlist")?;
        sentencia
            .query_map([playlist_id], pista_listado_desde_fila)
            .context("no se pudieron listar las pistas de la playlist")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer las pistas de la playlist")
    }

    pub fn validar_nombre(nombre: &str) -> std::result::Result<String, String> {
        let limpio = nombre.trim();
        if limpio.is_empty() {
            return Err("El nombre no puede estar vacío".to_string());
        }
        if limpio.chars().count() > 100 {
            return Err("El nombre no puede superar los 100 caracteres".to_string());
        }
        Ok(limpio.to_string())
    }

    pub fn nombre_disponible(
        conn: &Connection,
        nombre: &str,
        excepto_id: Option<i64>,
    ) -> Result<bool> {
        let normalizado = super::super::etiquetas::normalizar(nombre);
        let mut sentencia = conn
            .prepare("SELECT id, nombre FROM PLAYLISTS")
            .context("no se pudo preparar la comprobación de nombre")?;
        let existentes = sentencia
            .query_map([], |fila| {
                Ok((fila.get::<_, i64>(0)?, fila.get::<_, String>(1)?))
            })
            .context("no se pudo comprobar el nombre")?;
        for fila in existentes {
            let (id, existente) = fila.context("no se pudo leer la playlist")?;
            if Some(id) == excepto_id {
                continue;
            }
            if super::super::etiquetas::normalizar(&existente) == normalizado {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn crear(conn: &Connection, nombre: &str) -> Result<Option<i64>> {
        let limpio = match validar_nombre(nombre) {
            Ok(limpio) => limpio,
            Err(_) => return Ok(None),
        };
        if !nombre_disponible(conn, &limpio, None)? {
            return Ok(None);
        }
        let ahora = bd::ahora_iso();
        let id = conn
            .query_row(
                "INSERT INTO PLAYLISTS (nombre, creado_en, actualizado_en)
                 VALUES (?1, ?2, ?2) RETURNING id",
                params![limpio, ahora],
                |fila| fila.get(0),
            )
            .context("no se pudo crear la playlist")?;
        Ok(Some(id))
    }

    pub fn renombrar(conn: &Connection, playlist_id: i64, nombre: &str) -> Result<Option<bool>> {
        let limpio = match validar_nombre(nombre) {
            Ok(limpio) => limpio,
            Err(_) => return Ok(None),
        };
        if !nombre_disponible(conn, &limpio, Some(playlist_id))? {
            return Ok(None);
        }
        let filas = conn
            .execute(
                "UPDATE PLAYLISTS SET nombre = ?1, actualizado_en = ?2 WHERE id = ?3",
                params![limpio, bd::ahora_iso(), playlist_id],
            )
            .context("no se pudo renombrar la playlist")?;
        Ok(Some(filas > 0))
    }

    pub fn eliminar(conn: &Connection, playlist_id: i64) -> Result<()> {
        conn.execute("DELETE FROM PLAYLISTS WHERE id = ?1", [playlist_id])
            .context("no se pudo eliminar la playlist")?;
        Ok(())
    }

    pub fn anadir_pistas(conn: &Connection, playlist_id: i64, pista_ids: &[i64]) -> Result<usize> {
        let siguiente: i64 = conn
            .query_row(
                "SELECT coalesce(max(posicion) + 1, 0) FROM PLAYLIST_PISTAS WHERE playlist_id = ?1",
                [playlist_id],
                |fila| fila.get(0),
            )
            .context("no se pudo calcular la posición")?;
        let tx = conn
            .unchecked_transaction()
            .context("no se pudo iniciar la transacción")?;
        let mut anadidas = 0usize;
        for (indice, pista_id) in pista_ids.iter().enumerate() {
            let filas = tx
                .execute(
                    "INSERT OR IGNORE INTO PLAYLIST_PISTAS (playlist_id, pista_id, posicion)
                     VALUES (?1, ?2, ?3)",
                    params![playlist_id, pista_id, siguiente + indice as i64],
                )
                .context("no se pudo añadir la pista a la playlist")?;
            anadidas += filas;
        }
        tx.execute(
            "UPDATE PLAYLISTS SET actualizado_en = ?1 WHERE id = ?2",
            params![bd::ahora_iso(), playlist_id],
        )
        .context("no se pudo actualizar la fecha de la playlist")?;
        tx.commit().context("no se pudo confirmar la playlist")?;
        Ok(anadidas)
    }

    pub fn quitar_pista(conn: &Connection, playlist_id: i64, posicion: usize) -> Result<()> {
        let tx = conn
            .unchecked_transaction()
            .context("no se pudo iniciar la transacción")?;
        tx.execute(
            "DELETE FROM PLAYLIST_PISTAS WHERE playlist_id = ?1 AND posicion = ?2",
            params![playlist_id, posicion as i64],
        )
        .context("no se pudo quitar la pista")?;
        tx.execute(
            "UPDATE PLAYLIST_PISTAS SET posicion = posicion - 1
              WHERE playlist_id = ?1 AND posicion > ?2",
            params![playlist_id, posicion as i64],
        )
        .context("no se pudo reordenar la playlist")?;
        tx.commit().context("no se pudo confirmar el cambio")?;
        Ok(())
    }

    pub fn mover(conn: &Connection, playlist_id: i64, de: usize, a: usize) -> Result<()> {
        let mut ids: Vec<i64> = pistas(conn, playlist_id)?
            .into_iter()
            .map(|pista| pista.id)
            .collect();
        if de >= ids.len() || a >= ids.len() || de == a {
            return Ok(());
        }
        let item = ids.remove(de);
        ids.insert(a, item);
        let tx = conn
            .unchecked_transaction()
            .context("no se pudo iniciar la transacción")?;
        tx.execute(
            "DELETE FROM PLAYLIST_PISTAS WHERE playlist_id = ?1",
            [playlist_id],
        )
        .context("no se pudo reescribir la playlist")?;
        for (posicion, pista_id) in ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO PLAYLIST_PISTAS (playlist_id, pista_id, posicion)
                 VALUES (?1, ?2, ?3)",
                params![playlist_id, pista_id, posicion as i64],
            )
            .context("no se pudo reordenar la playlist")?;
        }
        tx.commit().context("no se pudo confirmar el orden")?;
        Ok(())
    }

    pub fn exportar_m3u(
        conn: &Connection,
        playlist_id: i64,
        carpeta: &Path,
        nombre_fichero: &str,
    ) -> Result<PathBuf> {
        let pistas = pistas(conn, playlist_id)?;
        exportar_m3u_pistas(carpeta, nombre_fichero, &pistas)
    }

    pub fn exportar_m3u_pistas(
        carpeta: &Path,
        nombre_fichero: &str,
        pistas: &[PistaListado],
    ) -> Result<PathBuf> {
        std::fs::create_dir_all(carpeta)
            .with_context(|| format!("no se pudo crear {}", carpeta.display()))?;
        let mut fichero = nombre_fichero.trim().to_string();
        if !fichero.to_ascii_lowercase().ends_with(".m3u8")
            && !fichero.to_ascii_lowercase().ends_with(".m3u")
        {
            fichero.push_str(".m3u8");
        }
        let ruta = carpeta.join(fichero);
        let mut contenido = String::from("#EXTM3U\n");
        for pista in pistas {
            let segundos = pista.duracion_ms.max(0) / 1000;
            contenido.push_str(&format!(
                "#EXTINF:{segundos},{} - {}\n{}\n",
                pista.artista, pista.titulo, pista.ruta
            ));
        }
        std::fs::write(&ruta, contenido)
            .with_context(|| format!("no se pudo escribir {}", ruta.display()))?;
        Ok(ruta)
    }

    pub fn importar_m3u(
        conn: &Connection,
        ruta_fichero: &Path,
        raices: &[PathBuf],
    ) -> Result<(i64, u32, u32)> {
        let contenido = std::fs::read_to_string(ruta_fichero)
            .with_context(|| format!("no se pudo leer {}", ruta_fichero.display()))?;
        let base = ruta_fichero.parent().unwrap_or(Path::new("."));
        let rutas = parsear_m3u(&contenido, base);
        let mut encontradas = Vec::new();
        let mut no_encontradas = 0u32;
        for ruta in rutas {
            if !dentro_de_raices(&ruta, raices) {
                no_encontradas += 1;
                continue;
            }
            match buscar_pista_por_ruta(conn, &ruta)? {
                Some(id) => encontradas.push(id),
                None => no_encontradas += 1,
            }
        }
        let base_nombre = ruta_fichero
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Importada")
            .to_string();
        let nombre = nombre_libre(conn, &base_nombre)?;
        let playlist_id =
            crear(conn, &nombre)?.context("no se pudo crear la playlist importada")?;
        if !encontradas.is_empty() {
            anadir_pistas(conn, playlist_id, &encontradas)?;
        }
        Ok((playlist_id, encontradas.len() as u32, no_encontradas))
    }

    pub fn parsear_m3u(contenido: &str, base: &Path) -> Vec<PathBuf> {
        let mut rutas = Vec::new();
        for linea in contenido.lines() {
            let linea = linea.trim();
            if linea.is_empty() || linea.starts_with('#') {
                continue;
            }
            let ruta = PathBuf::from(linea);
            let absoluta = if ruta.is_absolute() {
                ruta
            } else {
                base.join(ruta)
            };
            rutas.push(normalizar_lexica(&absoluta));
        }
        rutas
    }

    fn nombre_libre(conn: &Connection, base: &str) -> Result<String> {
        for sufijo in 0..100 {
            let candidato = if sufijo == 0 {
                base.to_string()
            } else {
                format!("{base} ({})", sufijo + 1)
            };
            if nombre_disponible(conn, &candidato, None)? {
                return Ok(candidato);
            }
        }
        Ok(format!("{base} ({})", bd::ahora_iso()))
    }

    fn buscar_pista_por_ruta(conn: &Connection, ruta: &Path) -> Result<Option<i64>> {
        let texto = ruta.to_string_lossy().to_string();
        let directa = conn
            .query_row("SELECT id FROM PISTAS WHERE ruta = ?1", [&texto], |fila| {
                fila.get(0)
            })
            .optional()
            .context("no se pudo buscar la pista")?;
        if directa.is_some() {
            return Ok(directa);
        }
        if let Ok(canonica) = std::fs::canonicalize(ruta) {
            let canonica = canonica.to_string_lossy().to_string();
            return conn
                .query_row(
                    "SELECT id FROM PISTAS WHERE ruta = ?1",
                    [&canonica],
                    |fila| fila.get(0),
                )
                .optional()
                .context("no se pudo buscar la pista");
        }
        Ok(None)
    }

    fn dentro_de_raices(ruta: &Path, raices: &[PathBuf]) -> bool {
        if raices.is_empty() {
            return true;
        }
        raices.iter().any(|raiz| {
            let raiz = normalizar_lexica(raiz);
            ruta.starts_with(&raiz) || normalizar_lexica(ruta).starts_with(&raiz)
        })
    }

    fn normalizar_lexica(ruta: &Path) -> PathBuf {
        use std::path::Component;
        let mut salida = PathBuf::new();
        for componente in ruta.components() {
            match componente {
                Component::CurDir => {}
                Component::ParentDir => {
                    salida.pop();
                }
                otro => salida.push(otro.as_os_str()),
            }
        }
        salida
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResultadosBusqueda {
    pub artistas: Vec<ArtistaResumen>,
    pub albumes: Vec<AlbumResumen>,
    pub pistas: Vec<PistaListado>,
    pub mas_artistas: usize,
    pub mas_albumes: usize,
    pub mas_pistas: usize,
}

pub const LIMITE_BUSQUEDA: usize = 20;

pub fn buscar(conn: &Connection, texto: &str, limite: usize) -> Result<ResultadosBusqueda> {
    let normalizado = super::etiquetas::normalizar(texto);
    let terminos: Vec<String> = normalizado.split_whitespace().map(str::to_string).collect();
    if terminos.is_empty() || normalizado.chars().count() < 2 {
        return Ok(ResultadosBusqueda::default());
    }

    let (condicion_artistas, parametros_artistas) =
        condicion_busqueda(&terminos, &["ar.nombre_norm"]);
    let (condicion_albumes, parametros_albumes) =
        condicion_busqueda(&terminos, &["al.titulo_norm", "ar.nombre_norm"]);
    let (condicion_pistas, parametros_pistas) = condicion_busqueda(
        &terminos,
        &["p.titulo_norm", "ar.nombre_norm", "al.titulo_norm"],
    );

    let total_artistas: i64 = conn
        .query_row(
            &format!("SELECT count(*) FROM ARTISTAS ar WHERE {condicion_artistas}"),
            rusqlite::params_from_iter(parametros_artistas.iter()),
            |fila| fila.get(0),
        )
        .context("no se pudo contar la búsqueda de artistas")?;
    let mut sentencia = conn
        .prepare(&format!(
            "SELECT ar.id, ar.nombre,
                    (SELECT count(*) FROM ALBUMES al WHERE al.artista_id = ar.id),
                    (SELECT count(*) FROM PISTAS p WHERE p.artista_id = ar.id)
               FROM ARTISTAS ar
              WHERE {condicion_artistas}
              ORDER BY ar.nombre_norm
              LIMIT {limite}"
        ))
        .context("no se pudo preparar la búsqueda de artistas")?;
    let artistas = sentencia
        .query_map(
            rusqlite::params_from_iter(parametros_artistas.iter()),
            |fila| {
                Ok(ArtistaResumen {
                    id: fila.get(0)?,
                    nombre: fila.get(1)?,
                    num_albumes: fila.get(2)?,
                    num_pistas: fila.get(3)?,
                })
            },
        )
        .context("no se pudo buscar artistas")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer los artistas encontrados")?;

    let total_albumes: i64 = conn
        .query_row(
            &format!(
                "SELECT count(*)
                   FROM ALBUMES al JOIN ARTISTAS ar ON ar.id = al.artista_id
                  WHERE {condicion_albumes}"
            ),
            rusqlite::params_from_iter(parametros_albumes.iter()),
            |fila| fila.get(0),
        )
        .context("no se pudo contar la búsqueda de álbumes")?;
    let mut sentencia = conn
        .prepare(&format!(
            "SELECT al.id, al.titulo, ar.nombre, al.anio, al.caratula_ruta,
                    count(p.id), coalesce(sum(p.duracion_ms), 0)
               FROM ALBUMES al
               JOIN ARTISTAS ar ON ar.id = al.artista_id
               LEFT JOIN PISTAS p ON p.album_id = al.id
              WHERE {condicion_albumes}
              GROUP BY al.id
              ORDER BY al.titulo_norm
              LIMIT {limite}"
        ))
        .context("no se pudo preparar la búsqueda de álbumes")?;
    let albumes = sentencia
        .query_map(
            rusqlite::params_from_iter(parametros_albumes.iter()),
            album_resumen_desde_fila,
        )
        .context("no se pudo buscar álbumes")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer los álbumes encontrados")?;

    let total_pistas: i64 = conn
        .query_row(
            &format!(
                "SELECT count(*)
                   FROM PISTAS p
                   JOIN ARTISTAS ar ON ar.id = p.artista_id
                   JOIN ALBUMES al ON al.id = p.album_id
                  WHERE {condicion_pistas}"
            ),
            rusqlite::params_from_iter(parametros_pistas.iter()),
            |fila| fila.get(0),
        )
        .context("no se pudo contar la búsqueda de pistas")?;
    let mut sentencia = conn
        .prepare(&format!(
            "SELECT p.id, p.titulo, ar.nombre, al.titulo, al.id, p.duracion_ms, al.anio,
                    p.formato, al.caratula_ruta, p.ruta
               FROM PISTAS p
               JOIN ARTISTAS ar ON ar.id = p.artista_id
               JOIN ALBUMES al ON al.id = p.album_id
              WHERE {condicion_pistas}
              ORDER BY p.titulo_norm
              LIMIT {limite}"
        ))
        .context("no se pudo preparar la búsqueda de pistas")?;
    let pistas = sentencia
        .query_map(
            rusqlite::params_from_iter(parametros_pistas.iter()),
            pista_listado_desde_fila,
        )
        .context("no se pudo buscar pistas")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("no se pudieron leer las pistas encontradas")?;

    Ok(ResultadosBusqueda {
        mas_artistas: (total_artistas as usize).saturating_sub(artistas.len()),
        mas_albumes: (total_albumes as usize).saturating_sub(albumes.len()),
        mas_pistas: (total_pistas as usize).saturating_sub(pistas.len()),
        artistas,
        albumes,
        pistas,
    })
}

fn condicion_busqueda(terminos: &[String], columnas: &[&str]) -> (String, Vec<String>) {
    let mut condiciones = Vec::new();
    let mut parametros = Vec::new();
    for termino in terminos {
        let patron = format!(
            "%{}%",
            termino
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut alternativas = Vec::new();
        for columna in columnas {
            alternativas.push(format!("{columna} LIKE ? ESCAPE '\\'"));
            parametros.push(patron.clone());
        }
        condiciones.push(format!("({})", alternativas.join(" OR ")));
    }
    (condiciones.join(" AND "), parametros)
}

pub mod historial {
    use super::*;

    pub fn registrar_inicio(conn: &Connection, pista_id: i64) -> Result<(i64, String)> {
        let iniciado_en = bd::ahora_iso();
        conn.query_row(
            "INSERT INTO HISTORIAL_REPRODUCCION (pista_id, reproducido_en, completada)
             VALUES (?1, ?2, 0) RETURNING id",
            params![pista_id, iniciado_en],
            |fila| Ok((fila.get(0)?, iniciado_en.clone())),
        )
        .context("no se pudo registrar el inicio de reproducción")
    }

    /// Registra el título ICY de una emisora como escucha potencial (sin
    /// completar); devuelve el id y la marca de tiempo.
    pub fn registrar_icy(
        conn: &Connection,
        emisora_id: i64,
        titulo_icy: &str,
    ) -> Result<(i64, String)> {
        let visto_en = bd::ahora_iso();
        conn.query_row(
            "INSERT INTO HISTORIAL_REPRODUCCION (emisora_id, titulo_icy, reproducido_en, completada)
             VALUES (?1, ?2, ?3, 0) RETURNING id",
            params![emisora_id, titulo_icy, visto_en],
            |fila| Ok((fila.get(0)?, visto_en.clone())),
        )
        .context("no se pudo registrar el título de radio")
    }

    pub fn marcar_completada(conn: &Connection, historial_id: i64) -> Result<()> {
        conn.execute(
            "UPDATE HISTORIAL_REPRODUCCION SET completada = 1 WHERE id = ?1",
            [historial_id],
        )
        .context("no se pudo marcar el historial como completado")?;
        Ok(())
    }

    /// Devuelve `(emisora_id, titulo_icy, reproducido_en)` de una fila de radio.
    pub fn leer_icy(conn: &Connection, historial_id: i64) -> Result<Option<(i64, String, String)>> {
        conn.query_row(
            "SELECT emisora_id, titulo_icy, reproducido_en FROM HISTORIAL_REPRODUCCION
              WHERE id = ?1 AND emisora_id IS NOT NULL AND titulo_icy IS NOT NULL",
            [historial_id],
            |fila| Ok((fila.get(0)?, fila.get(1)?, fila.get(2)?)),
        )
        .optional()
        .context("no se pudo leer el título de radio del historial")
    }
}

pub mod favoritas {
    use super::*;

    pub fn es_favorita(conn: &Connection, pista_id: i64) -> Result<bool> {
        let existe: i64 = conn
            .query_row(
                "SELECT count(*) FROM FAVORITAS WHERE pista_id = ?1",
                [pista_id],
                |fila| fila.get(0),
            )
            .context("no se pudo comprobar la favorita")?;
        Ok(existe > 0)
    }

    pub fn alternar(conn: &Connection, pista_id: i64) -> Result<bool> {
        if es_favorita(conn, pista_id)? {
            conn.execute("DELETE FROM FAVORITAS WHERE pista_id = ?1", [pista_id])
                .context("no se pudo quitar la favorita")?;
            Ok(false)
        } else {
            conn.execute(
                "INSERT INTO FAVORITAS (pista_id, marcada_en) VALUES (?1, ?2)
                 ON CONFLICT(pista_id) DO NOTHING",
                params![pista_id, bd::ahora_iso()],
            )
            .context("no se pudo marcar la favorita")?;
            Ok(true)
        }
    }

    pub fn ids(conn: &Connection) -> Result<Vec<i64>> {
        let mut sentencia = conn
            .prepare("SELECT pista_id FROM FAVORITAS")
            .context("no se pudieron preparar las favoritas")?;
        sentencia
            .query_map([], |fila| fila.get(0))
            .context("no se pudieron listar las favoritas")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer las favoritas")
    }

    pub fn listar(conn: &Connection) -> Result<Vec<PistaListado>> {
        let mut sentencia = conn
            .prepare(
                "SELECT p.id, p.titulo, ar.nombre, al.titulo, al.id, p.duracion_ms, al.anio,
                        p.formato, al.caratula_ruta, p.ruta
                   FROM FAVORITAS f
                   JOIN PISTAS p ON p.id = f.pista_id
                   JOIN ARTISTAS ar ON ar.id = p.artista_id
                   JOIN ALBUMES al ON al.id = p.album_id
                  ORDER BY f.marcada_en DESC, p.titulo_norm",
            )
            .context("no se pudieron preparar las pistas favoritas")?;
        sentencia
            .query_map([], pista_listado_desde_fila)
            .context("no se pudieron listar las pistas favoritas")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer las pistas favoritas")
    }

    pub fn contar(conn: &Connection) -> Result<i64> {
        conn.query_row("SELECT count(*) FROM FAVORITAS", [], |fila| fila.get(0))
            .context("no se pudieron contar las favoritas")
    }
}

pub mod envios {
    use super::*;

    /// Origen de un envío: exactamente uno de pista o emisora.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum OrigenEnvio {
        Pista(i64),
        Emisora(i64),
    }

    impl OrigenEnvio {
        pub fn pista_id(self) -> Option<i64> {
            match self {
                OrigenEnvio::Pista(id) => Some(id),
                OrigenEnvio::Emisora(_) => None,
            }
        }

        pub fn emisora_id(self) -> Option<i64> {
            match self {
                OrigenEnvio::Pista(_) => None,
                OrigenEnvio::Emisora(id) => Some(id),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct EnvioPendiente {
        pub id: i64,
        pub tipo: String,
        pub origen: OrigenEnvio,
        pub historial_id: Option<i64>,
        pub reproducido_en: Option<String>,
        pub intentos: i64,
        /// Título a scrobblear: el de la pista o el `titulo_icy` en crudo.
        pub titulo: String,
        /// Artista de la pista; vacío en los envíos de radio (se parsea del ICY).
        pub artista: String,
        /// Álbum de la pista o nombre de la emisora.
        pub album: String,
        pub duracion_ms: Option<i64>,
    }

    impl EnvioPendiente {
        pub fn es_radio(&self) -> bool {
            matches!(self.origen, OrigenEnvio::Emisora(_))
        }
    }

    pub fn encolar(
        conn: &Connection,
        servicio: &str,
        tipo: &str,
        origen: OrigenEnvio,
        historial_id: Option<i64>,
        reproducido_en: Option<&str>,
    ) -> Result<i64> {
        let tx = conn
            .unchecked_transaction()
            .context("no se pudo iniciar la transacción del envío")?;
        if tipo != "scrobble"
            && let Some(pista_id) = origen.pista_id()
        {
            tx.execute(
                "UPDATE ENVIOS
                    SET estado = 'descartado', error_msg = 'reemplazado por un cambio posterior'
                  WHERE servicio = ?1 AND pista_id = ?2
                    AND tipo IN ('love', 'unlove')
                    AND estado IN ('pendiente', 'error')",
                params![servicio, pista_id],
            )
            .context("no se pudo descartar el love anterior")?;
        }
        let ahora = bd::ahora_iso();
        let id = tx
            .query_row(
                "INSERT INTO ENVIOS
                    (servicio, tipo, pista_id, emisora_id, historial_id, reproducido_en, estado,
                     intentos, proximo_intento_en, creado_en)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pendiente', 0, ?7, ?7)
                 RETURNING id",
                params![
                    servicio,
                    tipo,
                    origen.pista_id(),
                    origen.emisora_id(),
                    historial_id,
                    reproducido_en,
                    ahora
                ],
                |fila| fila.get(0),
            )
            .with_context(|| format!("no se pudo encolar el envío {tipo} para {servicio}"))?;
        tx.commit().context("no se pudo confirmar el envío")?;
        Ok(id)
    }

    pub fn pendientes(
        conn: &Connection,
        servicio: &str,
        limite: usize,
    ) -> Result<Vec<EnvioPendiente>> {
        let mut sentencia = conn
            .prepare(
                "SELECT e.id, e.tipo, e.pista_id, e.emisora_id, e.historial_id,
                        e.reproducido_en, e.intentos,
                        p.titulo, ar.nombre, al.titulo, p.duracion_ms,
                        h.titulo_icy, em.nombre
                   FROM ENVIOS e
                   LEFT JOIN PISTAS p ON p.id = e.pista_id
                   LEFT JOIN ARTISTAS ar ON ar.id = p.artista_id
                   LEFT JOIN ALBUMES al ON al.id = p.album_id
                   LEFT JOIN HISTORIAL_REPRODUCCION h ON h.id = e.historial_id
                   LEFT JOIN EMISORAS em ON em.id = e.emisora_id
                  WHERE e.servicio = ?1
                    AND e.estado IN ('pendiente', 'error')
                    AND e.proximo_intento_en <= ?2
                    AND ((e.pista_id IS NOT NULL AND e.emisora_id IS NULL)
                         OR (e.emisora_id IS NOT NULL AND e.pista_id IS NULL))
                  ORDER BY e.reproducido_en IS NULL, e.reproducido_en, e.id
                  LIMIT ?3",
            )
            .context("no se pudieron preparar los envíos pendientes")?;
        let filas = sentencia
            .query_map(params![servicio, bd::ahora_iso(), limite as i64], |fila| {
                let pista_id: Option<i64> = fila.get(2)?;
                let emisora_id: Option<i64> = fila.get(3)?;
                let (origen, titulo, artista, album, duracion_ms) = match (pista_id, emisora_id) {
                    (Some(pista_id), None) => (
                        OrigenEnvio::Pista(pista_id),
                        fila.get::<_, Option<String>>(7)?.unwrap_or_default(),
                        fila.get::<_, Option<String>>(8)?.unwrap_or_default(),
                        fila.get::<_, Option<String>>(9)?.unwrap_or_default(),
                        fila.get(10)?,
                    ),
                    (None, Some(emisora_id)) => (
                        OrigenEnvio::Emisora(emisora_id),
                        fila.get::<_, Option<String>>(11)?.unwrap_or_default(),
                        String::new(),
                        fila.get::<_, Option<String>>(12)?.unwrap_or_default(),
                        None,
                    ),
                    _ => {
                        return Err(rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Null,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "envío sin origen único",
                            )),
                        ));
                    }
                };
                Ok(EnvioPendiente {
                    id: fila.get(0)?,
                    tipo: fila.get(1)?,
                    origen,
                    historial_id: fila.get(4)?,
                    reproducido_en: fila.get(5)?,
                    intentos: fila.get(6)?,
                    titulo,
                    artista,
                    album,
                    duracion_ms,
                })
            })
            .context("no se pudieron listar los envíos pendientes")?;
        filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer los envíos pendientes")
    }

    pub fn marcar_enviado(conn: &Connection, id: i64) -> Result<()> {
        conn.execute(
            "UPDATE ENVIOS
                SET estado = 'enviado', enviado_en = ?1, error_msg = NULL
              WHERE id = ?2",
            params![bd::ahora_iso(), id],
        )
        .context("no se pudo marcar el envío como enviado")?;
        Ok(())
    }

    pub fn marcar_enviados(conn: &Connection, ids: &[i64]) -> Result<()> {
        let tx = conn
            .unchecked_transaction()
            .context("no se pudo iniciar la transacción de envíos")?;
        for id in ids {
            tx.execute(
                "UPDATE ENVIOS
                    SET estado = 'enviado', enviado_en = ?1, error_msg = NULL
                  WHERE id = ?2",
                params![bd::ahora_iso(), id],
            )
            .context("no se pudo marcar el envío como enviado")?;
        }
        tx.commit().context("no se pudo confirmar los envíos")?;
        Ok(())
    }

    pub fn marcar_error(
        conn: &Connection,
        id: i64,
        mensaje: &str,
        proximo_intento_en: &str,
        intentos: i64,
    ) -> Result<()> {
        conn.execute(
            "UPDATE ENVIOS
                SET estado = 'error', error_msg = ?1, proximo_intento_en = ?2, intentos = ?3
              WHERE id = ?4",
            params![mensaje, proximo_intento_en, intentos, id],
        )
        .context("no se pudo marcar el error del envío")?;
        Ok(())
    }

    pub fn descartar(conn: &Connection, id: i64, motivo: &str) -> Result<()> {
        conn.execute(
            "UPDATE ENVIOS SET estado = 'descartado', error_msg = ?1 WHERE id = ?2",
            params![motivo, id],
        )
        .context("no se pudo descartar el envío")?;
        Ok(())
    }

    pub fn contar_pendientes(conn: &Connection, servicio: &str) -> Result<u32> {
        conn.query_row(
            "SELECT count(*) FROM ENVIOS WHERE servicio = ?1 AND estado IN ('pendiente', 'error')",
            [servicio],
            |fila| fila.get::<_, i64>(0),
        )
        .map(|total| total.max(0) as u32)
        .context("no se pudieron contar los envíos pendientes")
    }

    pub fn reprogramar_errores_auth(conn: &Connection, servicio: &str) -> Result<usize> {
        let filas = conn
            .execute(
                "UPDATE ENVIOS
                    SET estado = 'pendiente', proximo_intento_en = ?1
                  WHERE servicio = ?2 AND estado = 'error' AND error_msg LIKE 'auth:%'",
                params![bd::ahora_iso(), servicio],
            )
            .context("no se pudieron reprogramar los errores de autenticación")?;
        Ok(filas)
    }

    pub fn limpiar_errores_auth(conn: &Connection, servicio: &str) -> Result<()> {
        conn.execute(
            "UPDATE ENVIOS SET error_msg = NULL
              WHERE servicio = ?1 AND error_msg LIKE 'auth:%'",
            [servicio],
        )
        .context("no se pudieron limpiar los errores de autenticación")?;
        Ok(())
    }
}

pub mod cola {
    use super::*;

    pub type ColaPersistida = (Vec<(ElementoCola, u32)>, Option<usize>);

    pub fn cargar(conn: &Connection) -> Result<ColaPersistida> {
        let mut sentencia = conn
            .prepare(
                "SELECT c.tipo, c.pista_id, c.emisora_id, c.posicion_orig,
                        p.titulo, ar.nombre, al.titulo, al.id, p.duracion_ms, al.caratula_ruta,
                        p.ruta,
                        e.nombre, e.url, e.pais, e.codec, e.bitrate_kbps, e.logo_url,
                        e.logo_ruta, e.favorita, e.ultima_reproduccion
                   FROM COLA c
                   LEFT JOIN PISTAS p ON p.id = c.pista_id
                   LEFT JOIN ARTISTAS ar ON ar.id = p.artista_id
                   LEFT JOIN ALBUMES al ON al.id = p.album_id
                   LEFT JOIN EMISORAS e ON e.id = c.emisora_id
                  ORDER BY c.posicion",
            )
            .context("no se pudo preparar la lectura de la cola")?;
        let filas = sentencia
            .query_map([], |fila| {
                let posicion_orig: i64 = fila.get(3)?;
                let pista_id: Option<i64> = fila.get(1)?;
                let emisora_id: Option<i64> = fila.get(2)?;
                let elemento = match (pista_id, emisora_id) {
                    (Some(_), None) => ElementoCola::Pista(PistaResumen {
                        id: fila.get(1)?,
                        titulo: fila.get(4)?,
                        artista: fila.get(5)?,
                        album: fila.get(6)?,
                        album_id: fila.get(7)?,
                        duracion_ms: fila.get(8)?,
                        caratula_ruta: fila.get(9)?,
                        ruta: fila.get(10)?,
                    }),
                    (None, Some(_)) => ElementoCola::Emisora(EmisoraResumen {
                        id: fila.get(2)?,
                        nombre: fila.get(11)?,
                        url: fila.get(12)?,
                        pais: fila.get(13)?,
                        codec: fila.get(14)?,
                        bitrate_kbps: fila.get(15)?,
                        logo_url: fila.get(16)?,
                        logo_ruta: fila.get(17)?,
                        favorita: fila.get::<_, i64>(18)? != 0,
                        ultima_reproduccion: fila.get(19)?,
                    }),
                    _ => {
                        return Err(rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Null,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "elemento de cola sin origen único",
                            )),
                        ));
                    }
                };
                Ok((elemento, posicion_orig.max(0) as u32))
            })
            .context("no se pudo leer la cola")?;
        let items: Vec<(ElementoCola, u32)> = filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer los elementos de la cola")?;
        if items.is_empty() {
            return Ok((items, None));
        }
        let indice = ajustes::leer_i64(conn, "cola_posicion")
            .unwrap_or(None)
            .filter(|valor| *valor >= 0 && (*valor as usize) < items.len())
            .map(|valor| valor as usize);
        Ok((items, indice))
    }

    pub fn guardar(
        conn: &Connection,
        items: &[(ElementoCola, u32)],
        indice: Option<usize>,
    ) -> Result<()> {
        let tx = conn
            .unchecked_transaction()
            .context("no se pudo iniciar la transacción de la cola")?;
        tx.execute("DELETE FROM COLA", [])
            .context("no se pudo vaciar la cola persistida")?;
        for (posicion, (elemento, posicion_orig)) in items.iter().enumerate() {
            tx.execute(
                "INSERT INTO COLA (tipo, pista_id, emisora_id, posicion, posicion_orig)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    elemento.tipo(),
                    elemento.pista_id(),
                    elemento.emisora_id(),
                    posicion as i64,
                    *posicion_orig as i64
                ],
            )
            .context("no se pudo persistir un elemento de la cola")?;
        }
        if let Some(indice) = indice {
            tx.execute(
                "INSERT INTO AJUSTES (clave, valor) VALUES ('cola_posicion', ?1)
                 ON CONFLICT(clave) DO UPDATE SET valor = excluded.valor",
                [indice.to_string()],
            )
            .context("no se pudo persistir la posición de la cola")?;
        } else {
            tx.execute("DELETE FROM AJUSTES WHERE clave = 'cola_posicion'", [])
                .context("no se pudo limpiar la posición de la cola")?;
        }
        tx.commit()
            .context("no se pudo confirmar la cola persistida")?;
        Ok(())
    }
}

pub mod emisoras {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum OrdenEmisoras {
        Nombre,
        UltimaReproduccion,
    }

    impl OrdenEmisoras {
        pub const TODAS: [OrdenEmisoras; 2] =
            [OrdenEmisoras::Nombre, OrdenEmisoras::UltimaReproduccion];

        pub fn etiqueta(self) -> &'static str {
            match self {
                OrdenEmisoras::Nombre => "nombre",
                OrdenEmisoras::UltimaReproduccion => "última reproducción",
            }
        }
    }

    /// Datos para dar de alta una emisora (alta manual, importación o directorio).
    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct NuevaEmisora {
        pub nombre: String,
        pub url: String,
        pub pagina_web: Option<String>,
        pub pais: Option<String>,
        pub etiquetas: Option<String>,
        pub codec: Option<String>,
        pub bitrate_kbps: Option<i64>,
        pub logo_url: Option<String>,
        pub radiobrowser_uuid: Option<String>,
        pub favorita: bool,
    }

    const CAMPOS: &str = "id, nombre, nombre_norm, url, pagina_web, pais, etiquetas, codec,
                          bitrate_kbps, logo_url, logo_ruta, radiobrowser_uuid, favorita,
                          anadida_en, ultima_reproduccion, ultimo_error";

    fn emisora_desde_fila(fila: &rusqlite::Row<'_>) -> rusqlite::Result<Emisora> {
        Ok(Emisora {
            id: fila.get(0)?,
            nombre: fila.get(1)?,
            nombre_norm: fila.get(2)?,
            url: fila.get(3)?,
            pagina_web: fila.get(4)?,
            pais: fila.get(5)?,
            etiquetas: fila.get(6)?,
            codec: fila.get(7)?,
            bitrate_kbps: fila.get(8)?,
            logo_url: fila.get(9)?,
            logo_ruta: fila.get(10)?,
            radiobrowser_uuid: fila.get(11)?,
            favorita: fila.get::<_, i64>(12)? != 0,
            anadida_en: fila.get(13)?,
            ultima_reproduccion: fila.get(14)?,
            ultimo_error: fila.get(15)?,
        })
    }

    pub fn listar(
        conn: &Connection,
        solo_favoritas: bool,
        orden: OrdenEmisoras,
        descendente: bool,
    ) -> Result<Vec<EmisoraResumen>> {
        let orden_sql = match (orden, descendente) {
            (OrdenEmisoras::Nombre, false) => "nombre_norm ASC",
            (OrdenEmisoras::Nombre, true) => "nombre_norm DESC",
            (OrdenEmisoras::UltimaReproduccion, false) => {
                "ultima_reproduccion IS NULL, ultima_reproduccion ASC, nombre_norm ASC"
            }
            (OrdenEmisoras::UltimaReproduccion, true) => {
                "ultima_reproduccion IS NULL, ultima_reproduccion DESC, nombre_norm ASC"
            }
        };
        let filtro = if solo_favoritas {
            "WHERE favorita = 1"
        } else {
            ""
        };
        let sql = format!(
            "SELECT id, nombre, url, pais, codec, bitrate_kbps, logo_url, logo_ruta,
                    favorita, ultima_reproduccion
               FROM EMISORAS {filtro} ORDER BY {orden_sql}"
        );
        let mut sentencia = conn
            .prepare(&sql)
            .context("no se pudo preparar el listado de emisoras")?;
        sentencia
            .query_map([], emisora_resumen_desde_fila)
            .context("no se pudieron listar las emisoras")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer las emisoras")
    }

    pub fn buscar_local(
        conn: &Connection,
        texto: &str,
        solo_favoritas: bool,
    ) -> Result<Vec<EmisoraResumen>> {
        let patron = format!(
            "%{}%",
            etiquetas::normalizar(texto)
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let filtro = if solo_favoritas {
            "AND favorita = 1"
        } else {
            ""
        };
        let sql = format!(
            "SELECT id, nombre, url, pais, codec, bitrate_kbps, logo_url, logo_ruta,
                    favorita, ultima_reproduccion
               FROM EMISORAS
              WHERE nombre_norm LIKE ?1 ESCAPE '\\' {filtro}
              ORDER BY nombre_norm"
        );
        let mut sentencia = conn
            .prepare(&sql)
            .context("no se pudo preparar la búsqueda local de emisoras")?;
        sentencia
            .query_map([patron], emisora_resumen_desde_fila)
            .context("no se pudo buscar emisoras locales")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer las emisoras locales")
    }

    pub fn por_id(conn: &Connection, id: i64) -> Result<Option<Emisora>> {
        conn.query_row(
            &format!("SELECT {CAMPOS} FROM EMISORAS WHERE id = ?1"),
            [id],
            emisora_desde_fila,
        )
        .optional()
        .context("no se pudo leer la emisora")
    }

    pub fn por_uuid(conn: &Connection, uuid: &str) -> Result<Option<Emisora>> {
        conn.query_row(
            &format!("SELECT {CAMPOS} FROM EMISORAS WHERE radiobrowser_uuid = ?1"),
            [uuid],
            emisora_desde_fila,
        )
        .optional()
        .context("no se pudo leer la emisora por uuid")
    }

    pub fn por_url(conn: &Connection, url: &str) -> Result<Option<Emisora>> {
        conn.query_row(
            &format!("SELECT {CAMPOS} FROM EMISORAS WHERE url = ?1"),
            [url],
            emisora_desde_fila,
        )
        .optional()
        .context("no se pudo leer la emisora por url")
    }

    pub fn crear(conn: &Connection, nueva: &NuevaEmisora) -> Result<i64> {
        let nombre = nueva.nombre.trim();
        let url = normalizar_url(&nueva.url);
        conn.query_row(
            "INSERT INTO EMISORAS
                (nombre, nombre_norm, url, pagina_web, pais, etiquetas, codec, bitrate_kbps,
                 logo_url, radiobrowser_uuid, favorita, anadida_en)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             RETURNING id",
            params![
                nombre,
                etiquetas::normalizar(nombre),
                url,
                nueva.pagina_web,
                nueva.pais,
                nueva.etiquetas,
                nueva.codec,
                nueva.bitrate_kbps,
                nueva.logo_url,
                nueva.radiobrowser_uuid,
                i64::from(nueva.favorita),
                bd::ahora_iso()
            ],
            |fila| fila.get(0),
        )
        .context("no se pudo crear la emisora")
    }

    pub fn editar(
        conn: &Connection,
        id: i64,
        nombre: &str,
        url: &str,
        pagina_web: Option<&str>,
    ) -> Result<()> {
        let nombre = nombre.trim();
        conn.execute(
            "UPDATE EMISORAS
                SET nombre = ?1, nombre_norm = ?2, url = ?3, pagina_web = ?4
              WHERE id = ?5",
            params![
                nombre,
                etiquetas::normalizar(nombre),
                normalizar_url(url),
                pagina_web,
                id
            ],
        )
        .context("no se pudo editar la emisora")?;
        Ok(())
    }

    pub fn eliminar(conn: &Connection, id: i64) -> Result<()> {
        conn.execute("DELETE FROM EMISORAS WHERE id = ?1", [id])
            .context("no se pudo eliminar la emisora")?;
        Ok(())
    }

    pub fn alternar_favorita(conn: &Connection, id: i64) -> Result<bool> {
        let favorita: Option<i64> = conn
            .query_row(
                "SELECT favorita FROM EMISORAS WHERE id = ?1",
                [id],
                |fila| fila.get(0),
            )
            .optional()
            .context("no se pudo leer la favorita de la emisora")?;
        let nuevo = match favorita {
            Some(1) => 0,
            Some(_) => 1,
            None => return Ok(false),
        };
        conn.execute(
            "UPDATE EMISORAS SET favorita = ?1 WHERE id = ?2",
            params![nuevo, id],
        )
        .context("no se pudo cambiar la favorita de la emisora")?;
        Ok(nuevo == 1)
    }

    pub fn marcar_reproducida(conn: &Connection, id: i64) -> Result<()> {
        conn.execute(
            "UPDATE EMISORAS SET ultima_reproduccion = ?1, ultimo_error = NULL WHERE id = ?2",
            params![bd::ahora_iso(), id],
        )
        .context("no se pudo marcar la emisora como reproducida")?;
        Ok(())
    }

    pub fn fijar_codec(
        conn: &Connection,
        id: i64,
        codec: Option<&str>,
        bitrate_kbps: Option<i64>,
    ) -> Result<()> {
        conn.execute(
            "UPDATE EMISORAS
                SET codec = COALESCE(codec, ?1), bitrate_kbps = COALESCE(bitrate_kbps, ?2)
              WHERE id = ?3",
            params![codec, bitrate_kbps, id],
        )
        .context("no se pudo fijar el codec de la emisora")?;
        Ok(())
    }

    pub fn fijar_logo(conn: &Connection, id: i64, logo_ruta: &str) -> Result<()> {
        conn.execute(
            "UPDATE EMISORAS SET logo_ruta = ?1 WHERE id = ?2",
            params![logo_ruta, id],
        )
        .context("no se pudo fijar el logo de la emisora")?;
        Ok(())
    }

    pub fn marcar_error(conn: &Connection, id: i64, mensaje: &str) -> Result<()> {
        conn.execute(
            "UPDATE EMISORAS SET ultimo_error = ?1 WHERE id = ?2",
            params![mensaje, id],
        )
        .context("no se pudo guardar el error de la emisora")?;
        Ok(())
    }

    pub fn normalizar_url(url: &str) -> String {
        url.trim().trim_end_matches('/').to_string()
    }
}

pub mod titulos_emisora {
    use super::*;

    const MAX_TITULOS: usize = 50;

    /// Inserta el título si no repite el último visto y poda la emisora a los
    /// 50 más recientes.
    pub fn insertar_y_podar(conn: &Connection, emisora_id: i64, titulo: &str) -> Result<()> {
        let ultimo: Option<String> = conn
            .query_row(
                "SELECT titulo FROM EMISORA_TITULOS
                  WHERE emisora_id = ?1 ORDER BY visto_en DESC, id DESC LIMIT 1",
                [emisora_id],
                |fila| fila.get(0),
            )
            .optional()
            .context("no se pudo leer el último título de la emisora")?;
        if ultimo.as_deref() == Some(titulo) {
            return Ok(());
        }
        let tx = conn
            .unchecked_transaction()
            .context("no se pudo iniciar la transacción de títulos")?;
        tx.execute(
            "INSERT INTO EMISORA_TITULOS (emisora_id, titulo, visto_en) VALUES (?1, ?2, ?3)",
            params![emisora_id, titulo, bd::ahora_iso()],
        )
        .context("no se pudo insertar el título de la emisora")?;
        tx.execute(
            "DELETE FROM EMISORA_TITULOS
              WHERE emisora_id = ?1
                AND id NOT IN (
                    SELECT id FROM EMISORA_TITULOS
                     WHERE emisora_id = ?1
                     ORDER BY visto_en DESC, id DESC LIMIT ?2)",
            params![emisora_id, MAX_TITULOS as i64],
        )
        .context("no se pudieron podar los títulos de la emisora")?;
        tx.commit()
            .context("no se pudo confirmar los títulos de la emisora")?;
        Ok(())
    }

    pub fn listar(conn: &Connection, emisora_id: i64, limite: usize) -> Result<Vec<TituloEmisora>> {
        let mut sentencia = conn
            .prepare(
                "SELECT id, emisora_id, titulo, visto_en
                   FROM EMISORA_TITULOS
                  WHERE emisora_id = ?1
                  ORDER BY visto_en DESC, id DESC
                  LIMIT ?2",
            )
            .context("no se pudo preparar el listado de títulos")?;
        sentencia
            .query_map(params![emisora_id, limite as i64], |fila| {
                Ok(TituloEmisora {
                    id: fila.get(0)?,
                    emisora_id: fila.get(1)?,
                    titulo: fila.get(2)?,
                    visto_en: fila.get(3)?,
                })
            })
            .context("no se pudieron listar los títulos de la emisora")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("no se pudieron leer los títulos de la emisora")
    }
}

pub mod busquedas_radio {
    use super::*;

    pub fn leer(conn: &Connection, clave: &str) -> Result<Option<(String, String)>> {
        conn.query_row(
            "SELECT respuesta_json, obtenido_en FROM BUSQUEDAS_RADIO WHERE clave = ?1",
            [clave],
            |fila| Ok((fila.get(0)?, fila.get(1)?)),
        )
        .optional()
        .context("no se pudo leer la búsqueda de radio cacheada")
    }

    pub fn guardar(conn: &Connection, clave: &str, respuesta_json: &str) -> Result<()> {
        conn.execute(
            "INSERT INTO BUSQUEDAS_RADIO (clave, respuesta_json, obtenido_en)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(clave) DO UPDATE
                SET respuesta_json = excluded.respuesta_json,
                    obtenido_en = excluded.obtenido_en",
            params![clave, respuesta_json, bd::ahora_iso()],
        )
        .context("no se pudo guardar la búsqueda de radio")?;
        Ok(())
    }

    pub fn podar(conn: &Connection, dias: i64) -> Result<usize> {
        let limite = bd::iso_en(-dias * 86_400);
        conn.execute(
            "DELETE FROM BUSQUEDAS_RADIO WHERE obtenido_en < ?1",
            [limite],
        )
        .context("no se pudo podar la caché de búsquedas de radio")
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn bd_con_pistas(numero: i64) -> Connection {
        let mut conn = Connection::open_in_memory().expect("memoria");
        conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
        bd::migrar(&mut conn).expect("esquema");
        conn.execute(
            "INSERT INTO ARTISTAS (id, nombre, nombre_norm, creado_en) VALUES (1, 'ESPRIT 空想', 'esprit 空想', 'x')",
            [],
        )
        .expect("artista");
        conn.execute(
            "INSERT INTO ALBUMES (id, artista_id, titulo, titulo_norm, anio, creado_en) VALUES (1, 1, 'Midtown Mall', 'midtown mall', 2019, 'x')",
            [],
        )
        .expect("álbum");
        for indice in 1..=numero {
            conn.execute(
                "INSERT INTO PISTAS (id, album_id, artista_id, titulo, titulo_norm, duracion_ms, ruta, formato, tamano_bytes, modificado_en, anadido_en)
                 VALUES (?1, 1, 1, ?2, ?3, 1000, ?4, 'mp3', 10, 1, 'x')",
                rusqlite::params![
                    indice,
                    format!("Midnight {indice}"),
                    format!("midnight {indice}"),
                    format!("/musica/pista{indice}.mp3")
                ],
            )
            .expect("pista");
        }
        conn
    }

    #[test]
    fn busca_por_terminos_normalizados() {
        let conn = bd_con_pistas(5);
        let resultados = buscar(&conn, "MIDNIGHT", LIMITE_BUSQUEDA).expect("buscar");
        assert_eq!(resultados.pistas.len(), 5);
        assert_eq!(resultados.artistas.len(), 0);

        let resultados = buscar(&conn, "midtown 空想", LIMITE_BUSQUEDA).expect("buscar");
        assert_eq!(resultados.albumes.len(), 1);
        assert_eq!(resultados.pistas.len(), 5);

        let vacio = buscar(&conn, "x", LIMITE_BUSQUEDA).expect("buscar");
        assert_eq!(vacio, ResultadosBusqueda::default());
    }

    #[test]
    fn limites_y_mas_resultados() {
        let conn = bd_con_pistas(25);
        let resultados = buscar(&conn, "midnight", 20).expect("buscar");
        assert_eq!(resultados.pistas.len(), 20);
        assert_eq!(resultados.mas_pistas, 5);
    }

    #[test]
    fn valida_nombres_de_playlist() {
        let conn = bd_con_pistas(1);
        assert!(playlist::crear(&conn, "  ").expect("crear").is_none());
        assert!(
            playlist::crear(&conn, &"a".repeat(101))
                .expect("crear")
                .is_none()
        );
        let id = playlist::crear(&conn, "Coche").expect("crear").expect("id");
        assert!(
            playlist::crear(&conn, " coche ")
                .expect("crear duplicada")
                .is_none()
        );
        assert!(
            playlist::renombrar(&conn, id, "Trabajo")
                .expect("renombrar")
                .is_some()
        );
        assert!(
            playlist::renombrar(&conn, id, "coche")
                .expect("renombrar")
                .is_some()
        );
    }

    #[test]
    fn anade_quita_y_reordena_pistas() {
        let conn = bd_con_pistas(3);
        let id = playlist::crear(&conn, "Mixtape")
            .expect("crear")
            .expect("id");
        playlist::anadir_pistas(&conn, id, &[1, 2, 3]).expect("añadir");
        let ids: Vec<i64> = playlist::pistas(&conn, id)
            .expect("pistas")
            .iter()
            .map(|pista| pista.id)
            .collect();
        assert_eq!(ids, vec![1, 2, 3]);

        playlist::quitar_pista(&conn, id, 0).expect("quitar");
        let ids: Vec<i64> = playlist::pistas(&conn, id)
            .expect("pistas")
            .iter()
            .map(|pista| pista.id)
            .collect();
        assert_eq!(ids, vec![2, 3]);

        playlist::mover(&conn, id, 0, 1).expect("mover");
        let ids: Vec<i64> = playlist::pistas(&conn, id)
            .expect("pistas")
            .iter()
            .map(|pista| pista.id)
            .collect();
        assert_eq!(ids, vec![3, 2]);
    }

    #[test]
    fn exporta_e_importa_m3u() {
        let conn = bd_con_pistas(3);
        let id = playlist::crear(&conn, "Mixtape")
            .expect("crear")
            .expect("id");
        playlist::anadir_pistas(&conn, id, &[1, 2, 3]).expect("añadir");
        let temporal = tempfile::tempdir().expect("tempdir");

        let ruta = playlist::exportar_m3u(&conn, id, temporal.path(), "mixtape").expect("exportar");
        assert!(ruta.ends_with("mixtape.m3u8"));
        let contenido = std::fs::read_to_string(&ruta).expect("leer");
        assert!(contenido.starts_with("#EXTM3U\n"));
        assert!(contenido.contains("#EXTINF:1,ESPRIT 空想 - Midnight 1"));
        assert!(contenido.contains("/musica/pista1.mp3"));

        // La importación ignora rutas fuera de la biblioteca y cuenta las no encontradas.
        let m3u = temporal.path().join("importada.m3u");
        std::fs::write(
            &m3u,
            "#EXTM3U\n/musica/pista1.mp3\n/musica/pista2.mp3\n/fuera/pista3.mp3\n",
        )
        .expect("escribir m3u");
        let vacia: Vec<std::path::PathBuf> = Vec::new();
        let (nueva, anadidas, no_encontradas) =
            playlist::importar_m3u(&conn, &m3u, &vacia).expect("importar");
        assert_eq!(anadidas, 2);
        assert_eq!(no_encontradas, 1);
        assert_eq!(playlist::pistas(&conn, nueva).expect("pistas").len(), 2);

        // El nombre importado choca: se usa sufijo (2).
        let m3u2 = temporal.path().join("importada.m3u");
        let (otra, _, _) = playlist::importar_m3u(&conn, &m3u2, &vacia).expect("importar 2");
        let nombre: String = conn
            .query_row(
                "SELECT nombre FROM PLAYLISTS WHERE id = ?1",
                [otra],
                |fila| fila.get(0),
            )
            .expect("nombre");
        assert_eq!(nombre, "importada (2)");

        // Filtro de raíces: sin raíces se acepta todo; con raíz que no contiene, se ignora.
        let raiz = temporal.path().join("otra");
        let (_, anadidas, no_encontradas) =
            playlist::importar_m3u(&conn, &m3u, &[raiz]).expect("importar 3");
        assert_eq!(anadidas, 0);
        assert_eq!(no_encontradas, 3);
    }

    #[test]
    fn parsea_m3u_con_rutas_relativas() {
        let base = std::path::Path::new("/musica/listas");
        let rutas = playlist::parsear_m3u(
            "#EXTM3U\n../pista.mp3\n\n#comentario\n/abs/pista2.flac\n",
            base,
        );
        assert_eq!(
            rutas,
            vec![
                std::path::PathBuf::from("/musica/pista.mp3"),
                std::path::PathBuf::from("/abs/pista2.flac"),
            ]
        );
    }

    #[test]
    fn siembra_presets_integrados_una_sola_vez() {
        let conn = bd_con_pistas(1);
        assert_eq!(presets_eq::sembrar_integrados(&conn).expect("sembrar"), 9);
        assert_eq!(presets_eq::sembrar_integrados(&conn).expect("resembrar"), 0);
        let presets = presets_eq::listar(&conn).expect("listar");
        assert_eq!(presets.len(), 9);
        assert!(presets.iter().all(|preset| preset.integrado));
        let rock = presets
            .iter()
            .find(|preset| preset.nombre == "Rock")
            .expect("Rock");
        assert_eq!(rock.ganancias[0], 5.0);
        assert_eq!(rock.preamp_db, -2.0);
    }

    #[test]
    fn crea_y_elimina_presets_propios() {
        let conn = bd_con_pistas(1);
        presets_eq::sembrar_integrados(&conn).expect("sembrar");
        let ganancias = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let id = presets_eq::crear(&conn, "  Coche noche ", &ganancias, 1.5)
            .expect("crear")
            .expect("id");
        assert!(
            presets_eq::crear(&conn, "coche NOCHE", &ganancias, 0.0)
                .expect("duplicado")
                .is_none()
        );
        assert!(
            presets_eq::crear(&conn, "Rock", &ganancias, 0.0)
                .expect("integrado")
                .is_none()
        );
        assert!(presets_eq::crear(&conn, "   ", &ganancias, 0.0).is_err());
        let preset = presets_eq::obtener(&conn, id)
            .expect("obtener")
            .expect("existe");
        assert_eq!(preset.nombre, "Coche noche");
        assert!(!preset.integrado);
        assert_eq!(preset.ganancias, ganancias);
        assert_eq!(preset.preamp_db, 1.5);
        let integrado = presets_eq::listar(&conn)
            .expect("listar")
            .into_iter()
            .find(|preset| preset.integrado)
            .expect("integrado");
        assert!(!presets_eq::eliminar(&conn, integrado.id).expect("integr"));
        assert!(presets_eq::eliminar(&conn, id).expect("eliminar"));
        assert!(presets_eq::obtener(&conn, id).expect("obtener").is_none());
    }

    #[test]
    fn guarda_offset_y_fuente_de_letras() {
        let conn = bd_con_pistas(1);
        assert_eq!(letras::offset(&conn, 1).expect("offset"), 0);
        assert_eq!(letras::fuente_preferida(&conn, 1).expect("fuente"), None);
        letras::fijar_offset(&conn, 1, -350).expect("fijar offset");
        letras::fijar_fuente(&conn, 1, "etiqueta").expect("fijar fuente");
        assert_eq!(letras::offset(&conn, 1).expect("offset"), -350);
        assert_eq!(
            letras::fuente_preferida(&conn, 1).expect("fuente"),
            Some("etiqueta".to_string())
        );
        // Conserva el offset al fijar de nuevo la fuente y viceversa.
        letras::fijar_fuente(&conn, 1, "fichero").expect("fijar fuente 2");
        assert_eq!(letras::offset(&conn, 1).expect("offset"), -350);
        letras::fijar_offset(&conn, 1, 40_000).expect("acotar");
        assert_eq!(letras::offset(&conn, 1).expect("offset"), 30_000);
        assert_eq!(
            letras::fuente_preferida(&conn, 1).expect("fuente"),
            Some("fichero".to_string())
        );
    }
}
