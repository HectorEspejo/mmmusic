use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use tracing::warn;
use walkdir::WalkDir;

use super::bd;
use super::caratulas;
use super::consultas;
use super::etiquetas;
use super::modelos::EstadoEscaneo;
use crate::eventos::{AppEvento, EventoEscaneo, NivelAviso, ResumenEscaneo};

const MAX_TAMANO_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const COMMIT_CADA: u64 = 200;
const PROGRESO_CADA: u64 = 50;
const ESPERA_CANCELACION: Duration = Duration::from_secs(3);

pub struct ManejoEscaneo {
    cancelacion: Arc<AtomicBool>,
    receptor_fin: Receiver<()>,
}

impl ManejoEscaneo {
    pub fn cancelar(&self) {
        self.cancelacion.store(true, Ordering::SeqCst);
    }

    pub fn esperar(&self, timeout: Duration) -> bool {
        matches!(self.receptor_fin.recv_timeout(timeout), Ok(()))
    }
}

impl Drop for ManejoEscaneo {
    fn drop(&mut self) {
        self.cancelar();
    }
}

enum Resultado {
    Nueva,
    Actualizada,
    SinCambios,
    Omitida,
}

pub fn lanzar(
    ruta_bd: PathBuf,
    raices: Vec<PathBuf>,
    dir_caratulas: PathBuf,
    tx: Sender<AppEvento>,
) -> Result<ManejoEscaneo> {
    let cancelacion = Arc::new(AtomicBool::new(false));
    let bandera = cancelacion.clone();
    let (tx_fin, receptor_fin) = mpsc::channel();
    thread::Builder::new()
        .name("escaner".to_string())
        .spawn(move || {
            if let Err(error) = ejecutar(&ruta_bd, &raices, &dir_caratulas, &bandera, &tx) {
                tracing::error!("escaneo interrumpido: {error:#}");
                let _ = tx.send(AppEvento::Escaneo(EventoEscaneo::Error {
                    mensaje: format!("{error:#}"),
                }));
            }
            let _ = tx_fin.send(());
        })
        .context("no se pudo lanzar el hilo de escaneo")?;
    Ok(ManejoEscaneo {
        cancelacion,
        receptor_fin,
    })
}

pub fn cancelar_y_esperar(manejo: ManejoEscaneo) {
    manejo.cancelar();
    if !manejo.esperar(ESPERA_CANCELACION) {
        warn!("el escaneo anterior no terminó a tiempo; se continúa igualmente");
    }
}

fn ejecutar(
    ruta_bd: &Path,
    raices: &[PathBuf],
    dir_caratulas: &Path,
    cancelacion: &AtomicBool,
    tx: &Sender<AppEvento>,
) -> Result<()> {
    let mut conn = bd::abrir(ruta_bd)?;
    bd::migrar(&mut conn)?;
    if consultas::escaneos::marcar_huerfanos(&conn)? > 0 {
        warn!("se marcaron escaneos interrumpidos como error");
    }
    let escaneo_id = consultas::escaneos::iniciar(&conn)?;
    let mut resumen = ResumenEscaneo::default();
    match ejecutar_interno(
        &mut conn,
        escaneo_id,
        raices,
        dir_caratulas,
        cancelacion,
        tx,
        &mut resumen,
    ) {
        Ok(estado) => {
            consultas::escaneos::finalizar(
                &conn,
                escaneo_id,
                estado,
                resumen.nuevas,
                resumen.actualizadas,
                resumen.eliminadas,
                None,
            )?;
            Ok(())
        }
        Err(error) => {
            let mensaje = format!("{error:#}");
            let _ = consultas::escaneos::finalizar(
                &conn,
                escaneo_id,
                EstadoEscaneo::Error,
                resumen.nuevas,
                resumen.actualizadas,
                resumen.eliminadas,
                Some(&mensaje),
            );
            Err(error)
        }
    }
}

fn ejecutar_interno(
    conn: &mut Connection,
    escaneo_id: i64,
    raices: &[PathBuf],
    dir_caratulas: &Path,
    cancelacion: &AtomicBool,
    tx: &Sender<AppEvento>,
    resumen: &mut ResumenEscaneo,
) -> Result<EstadoEscaneo> {
    let mut raices_validas = Vec::new();
    for raiz in raices {
        if raiz.is_dir() {
            raices_validas.push(raiz.clone());
        } else {
            warn!("carpeta de biblioteca no disponible: {}", raiz.display());
            let _ = tx.send(AppEvento::Notificacion(
                NivelAviso::Aviso,
                format!("Carpeta no disponible: {}", raiz.display()),
            ));
        }
    }

    let ficheros = recoger_ficheros(&raices_validas, cancelacion);
    let total = ficheros.len();
    let _ = tx.send(AppEvento::Escaneo(EventoEscaneo::Iniciado { total }));

    let mut tx_bd = conn
        .transaction()
        .context("no se pudo iniciar la transacción de escaneo")?;
    let mut procesadas = 0u64;
    let mut cancelado = false;
    for ruta in &ficheros {
        if cancelacion.load(Ordering::SeqCst) {
            cancelado = true;
            break;
        }
        match procesar_fichero(&tx_bd, ruta, escaneo_id) {
            Ok(Resultado::Nueva) => resumen.nuevas += 1,
            Ok(Resultado::Actualizada) => resumen.actualizadas += 1,
            Ok(Resultado::SinCambios) => {}
            Ok(Resultado::Omitida) => resumen.omitidas += 1,
            Err(error) => return Err(error),
        }
        procesadas += 1;
        if procesadas.is_multiple_of(COMMIT_CADA) {
            tx_bd.commit().context("no se pudo confirmar el lote")?;
            tx_bd = conn
                .transaction()
                .context("no se pudo iniciar el siguiente lote")?;
        }
        if procesadas.is_multiple_of(PROGRESO_CADA) {
            let _ = tx.send(AppEvento::Escaneo(EventoEscaneo::Progreso {
                procesadas: procesadas as usize,
                total,
            }));
        }
    }
    tx_bd.commit().context("no se pudo confirmar el escaneo")?;

    if cancelado {
        let _ = tx.send(AppEvento::Escaneo(EventoEscaneo::Cancelado {
            resumen: resumen.clone(),
        }));
        return Ok(EstadoEscaneo::Cancelado);
    }

    eliminar_no_vistas(conn, escaneo_id, &raices_validas, resumen)?;
    limpiar_huerfanos(conn)?;

    match caratulas::procesar_pendientes(conn, dir_caratulas, Some(cancelacion)) {
        Ok(generadas) if generadas > 0 => {
            let _ = tx.send(AppEvento::Notificacion(
                NivelAviso::Info,
                format!("{generadas} carátulas generadas"),
            ));
        }
        Ok(_) => {}
        Err(error) => warn!("no se pudieron procesar las carátulas: {error:#}"),
    }

    let _ = tx.send(AppEvento::Escaneo(EventoEscaneo::Terminado {
        resumen: resumen.clone(),
    }));
    Ok(EstadoEscaneo::Completado)
}

fn recoger_ficheros(raices: &[PathBuf], cancelacion: &AtomicBool) -> Vec<PathBuf> {
    let mut vistos: HashSet<(u64, u64)> = HashSet::new();
    let mut ficheros = Vec::new();
    for raiz in raices {
        let mut recorrido = WalkDir::new(raiz).follow_links(true).into_iter();
        while let Some(entrada) = recorrido.next() {
            if cancelacion.load(Ordering::SeqCst) {
                return ficheros;
            }
            let entrada = match entrada {
                Ok(entrada) => entrada,
                Err(error) => {
                    warn!("aviso al recorrer la biblioteca: {error}");
                    continue;
                }
            };
            if entrada.file_type().is_dir() {
                if let Ok(metadatos) = entrada.metadata() {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;
                        if !vistos.insert((metadatos.dev(), metadatos.ino())) {
                            recorrido.skip_current_dir();
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        let _ = metadatos;
                    }
                }
            } else if entrada.file_type().is_file()
                && etiquetas::formato_de_ruta(entrada.path()).is_some()
            {
                ficheros.push(entrada.path().to_path_buf());
            }
        }
    }
    ficheros
}

fn procesar_fichero(conn: &Connection, ruta: &Path, escaneo_id: i64) -> Result<Resultado> {
    let metadatos = match fs::metadata(ruta) {
        Ok(metadatos) => metadatos,
        Err(error) => {
            warn!(ruta = %ruta.display(), "no se pudo leer el fichero: {error}");
            return Ok(Resultado::Omitida);
        }
    };
    let tamano = metadatos.len();
    if tamano > MAX_TAMANO_BYTES {
        warn!(ruta = %ruta.display(), "fichero mayor de 2 GB omitido");
        return Ok(Resultado::Omitida);
    }
    let modificado = metadatos
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let ruta_texto = ruta.to_string_lossy().to_string();

    let existente: Option<(i64, i64, i64)> = conn
        .query_row(
            "SELECT id, modificado_en, tamano_bytes FROM PISTAS WHERE ruta = ?1",
            [&ruta_texto],
            |fila| Ok((fila.get(0)?, fila.get(1)?, fila.get(2)?)),
        )
        .optional()
        .with_context(|| format!("no se pudo consultar la pista {}", ruta.display()))?;

    if let Some((id, modificado_previo, tamano_previo)) = existente
        && modificado_previo == modificado
        && tamano_previo == tamano as i64
    {
        conn.execute(
            "UPDATE PISTAS SET escaneo_id = ?1 WHERE id = ?2",
            params![escaneo_id, id],
        )
        .with_context(|| format!("no se pudo actualizar el escaneo de {}", ruta.display()))?;
        return Ok(Resultado::SinCambios);
    }

    let crudas = match etiquetas::leer(ruta) {
        Ok(crudas) => crudas,
        Err(error) => {
            warn!(ruta = %ruta.display(), "etiquetas ilegibles: {error:#}");
            return Ok(Resultado::Omitida);
        }
    };
    if crudas.duracion_ms.unwrap_or(0) <= 0 {
        warn!(ruta = %ruta.display(), "sin duración detectable; omitida");
        return Ok(Resultado::Omitida);
    }
    let etiquetas = etiquetas::resolver(crudas, ruta);
    let artista_id = asegurar_artista(conn, &etiquetas.artista)?;
    let album_artista_id = asegurar_artista(conn, &etiquetas.album_artista)?;
    let album_id = asegurar_album(conn, album_artista_id, &etiquetas)?;
    let formato = etiquetas::formato_de_ruta(ruta).unwrap_or("desconocido");
    let titulo_norm = etiquetas::normalizar(&etiquetas.titulo);

    if let Some((id, _, _)) = existente {
        conn.execute(
            "UPDATE PISTAS
                SET album_id = ?1, artista_id = ?2, titulo = ?3, titulo_norm = ?4,
                    numero_pista = ?5, numero_disco = ?6, genero = ?7, duracion_ms = ?8,
                    formato = ?9, tamano_bytes = ?10, modificado_en = ?11,
                    bitrate_kbps = ?12, escaneo_id = ?13
              WHERE id = ?14",
            params![
                album_id,
                artista_id,
                etiquetas.titulo,
                titulo_norm,
                etiquetas.pista,
                etiquetas.disco,
                etiquetas.genero,
                etiquetas.duracion_ms,
                formato,
                tamano as i64,
                modificado,
                etiquetas.bitrate_kbps,
                escaneo_id,
                id
            ],
        )
        .with_context(|| format!("no se pudo actualizar la pista {}", ruta.display()))?;
        Ok(Resultado::Actualizada)
    } else {
        conn.execute(
            "INSERT INTO PISTAS
                (album_id, artista_id, titulo, titulo_norm, numero_pista, numero_disco,
                 genero, duracion_ms, ruta, formato, tamano_bytes, modificado_en,
                 bitrate_kbps, anadido_en, escaneo_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                album_id,
                artista_id,
                etiquetas.titulo,
                titulo_norm,
                etiquetas.pista,
                etiquetas.disco,
                etiquetas.genero,
                etiquetas.duracion_ms,
                ruta_texto,
                formato,
                tamano as i64,
                modificado,
                etiquetas.bitrate_kbps,
                bd::ahora_iso(),
                escaneo_id
            ],
        )
        .with_context(|| format!("no se pudo insertar la pista {}", ruta.display()))?;
        Ok(Resultado::Nueva)
    }
}

fn asegurar_artista(conn: &Connection, nombre: &str) -> Result<i64> {
    let norm = etiquetas::normalizar(nombre);
    conn.query_row(
        "INSERT INTO ARTISTAS (nombre, nombre_norm, creado_en) VALUES (?1, ?2, ?3)
         ON CONFLICT(nombre_norm) DO UPDATE SET nombre = excluded.nombre
         RETURNING id",
        params![nombre, norm, bd::ahora_iso()],
        |fila| fila.get(0),
    )
    .with_context(|| format!("no se pudo registrar el artista {nombre}"))
}

fn asegurar_album(
    conn: &Connection,
    artista_id: i64,
    etiquetas: &etiquetas::EtiquetasResueltas,
) -> Result<i64> {
    let norm = etiquetas::normalizar(&etiquetas.album);
    conn.query_row(
        "INSERT INTO ALBUMES (artista_id, titulo, titulo_norm, anio, caratula_ruta, creado_en)
         VALUES (?1, ?2, ?3, ?4, NULL, ?5)
         ON CONFLICT(artista_id, titulo_norm) DO UPDATE SET
             anio = COALESCE(excluded.anio, ALBUMES.anio),
             titulo = excluded.titulo
         RETURNING id",
        params![
            artista_id,
            etiquetas.album,
            norm,
            etiquetas.anio,
            bd::ahora_iso()
        ],
        |fila| fila.get(0),
    )
    .with_context(|| format!("no se pudo registrar el álbum {}", etiquetas.album))
}

fn eliminar_no_vistas(
    conn: &Connection,
    escaneo_id: i64,
    raices: &[PathBuf],
    resumen: &mut ResumenEscaneo,
) -> Result<()> {
    for raiz in raices {
        let raiz_texto = raiz.to_string_lossy().to_string();
        let patron = format!("{}/%", escapar_like(&raiz_texto));
        let eliminadas = conn
            .execute(
                "DELETE FROM PISTAS
                  WHERE escaneo_id IS NOT ?1
                    AND (ruta = ?2 OR ruta LIKE ?3 ESCAPE '\\')",
                params![escaneo_id, raiz_texto, patron],
            )
            .with_context(|| format!("no se pudieron eliminar pistas de {}", raiz.display()))?;
        resumen.eliminadas += eliminadas as u64;
    }
    Ok(())
}

fn limpiar_huerfanos(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM ALBUMES WHERE id NOT IN (SELECT album_id FROM PISTAS)",
        [],
    )
    .context("no se pudieron limpiar álbumes huérfanos")?;
    conn.execute(
        "DELETE FROM ARTISTAS
          WHERE id NOT IN (SELECT artista_id FROM PISTAS)
            AND id NOT IN (SELECT artista_id FROM ALBUMES)",
        [],
    )
    .context("no se pudieron limpiar artistas huérfanos")?;
    Ok(())
}

fn escapar_like(texto: &str) -> String {
    texto
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn escapa_comodines_like() {
        assert_eq!(escapar_like("/música/100%"), "/música/100\\%");
        assert_eq!(escapar_like("/a_b/c"), "/a\\_b/c");
        assert_eq!(escapar_like("/a\\b"), "/a\\\\b");
    }

    #[test]
    fn escaneo_sin_raices_no_toca_la_bd() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ruta_bd = dir.path().join("mmmusic.db");
        let mut conn = bd::abrir(&ruta_bd).expect("bd");
        bd::migrar(&mut conn).expect("migración");
        drop(conn);

        let (tx, rx) = channel();
        let manejo = lanzar(
            ruta_bd.clone(),
            vec![],
            ruta_bd.parent().unwrap_or(Path::new(".")).join("caratulas"),
            tx,
        )
        .expect("lanzar");
        assert!(manejo.esperar(Duration::from_secs(5)));
        let mut recibidos = Vec::new();
        while let Ok(evento) = rx.try_recv() {
            recibidos.push(evento);
        }
        assert!(
            recibidos
                .iter()
                .any(|e| matches!(e, AppEvento::Escaneo(EventoEscaneo::Terminado { .. })))
        );

        let conn = bd::abrir(&ruta_bd).expect("bd");
        let total: i64 = conn
            .query_row("SELECT count(*) FROM PISTAS", [], |f| f.get(0))
            .expect("contar");
        assert_eq!(total, 0);
    }
}
