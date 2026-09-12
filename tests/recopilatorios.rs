use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use mmmusic::biblioteca::bd;
use mmmusic::biblioteca::escaner::{self, ModoEscaneo};
use mmmusic::eventos::{AppEvento, EventoEscaneo};

const FICHEROS: [&str; 3] = ["01 Primera.mp3", "02 Segunda.mp3", "03 Tercera.mp3"];

fn preparar_biblioteca(dir: &Path) -> PathBuf {
    let album = dir.join("Recopilatorio");
    fs::create_dir_all(&album).expect("crear carpeta");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recopilatorio");
    for nombre in FICHEROS {
        fs::copy(fixtures.join(nombre), album.join(nombre)).expect("copiar fixture");
    }
    dir.to_path_buf()
}

fn abrir_bd(ruta: &Path) -> rusqlite::Connection {
    let mut conn = bd::abrir(ruta).expect("abrir bd");
    bd::migrar(&mut conn).expect("migrar");
    conn
}

fn escanear(
    ruta_bd: &Path,
    raices: Vec<PathBuf>,
    modo: ModoEscaneo,
) -> (Receiver<AppEvento>, escaner::ManejoEscaneo) {
    let (tx, rx) = mpsc::channel();
    let dir_caratulas = ruta_bd.parent().unwrap_or(Path::new(".")).join("caratulas");
    let manejo = escaner::lanzar(ruta_bd.to_path_buf(), raices, dir_caratulas, modo, tx)
        .expect("lanzar escaneo");
    (rx, manejo)
}

fn esperar_final(rx: &Receiver<AppEvento>) -> EventoEscaneo {
    let limite = Instant::now() + Duration::from_secs(60);
    while Instant::now() < limite {
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(AppEvento::Escaneo(evento)) => match evento {
                EventoEscaneo::Terminado { .. }
                | EventoEscaneo::Cancelado { .. }
                | EventoEscaneo::Error { .. } => return evento,
                _ => continue,
            },
            Ok(_) => continue,
            Err(error) => panic!("sin eventos de escaneo: {error}"),
        }
    }
    panic!("el escaneo no terminó a tiempo");
}

fn resumen(evento: &EventoEscaneo) -> (u64, u64, u64) {
    match evento {
        EventoEscaneo::Terminado { resumen } | EventoEscaneo::Cancelado { resumen } => {
            (resumen.nuevas, resumen.actualizadas, resumen.eliminadas)
        }
        EventoEscaneo::Error { mensaje } => panic!("escaneo con error: {mensaje}"),
        _ => panic!("evento inesperado"),
    }
}

fn etiquetar_albumartist(ruta: &Path, album_artista: &str) {
    use lofty::file::TaggedFileExt;
    use lofty::probe::Probe;
    use lofty::tag::{ItemKey, TagExt};

    let mut tagged = Probe::open(ruta)
        .expect("abrir pista")
        .read()
        .expect("leer etiquetas");
    {
        let tag = tagged.primary_tag_mut().expect("etiqueta primaria");
        tag.insert_text(ItemKey::AlbumArtist, album_artista.to_string());
    }
    tagged
        .primary_tag()
        .expect("etiqueta primaria")
        .save_to_path(ruta, lofty::config::WriteOptions::default())
        .expect("guardar etiqueta");
}

struct AlbumEnBd {
    artista: String,
    varios_artistas: bool,
    num_pistas: i64,
    anio: Option<i64>,
}

fn albumes(conn: &rusqlite::Connection) -> Vec<AlbumEnBd> {
    let mut sentencia = conn
        .prepare(
            "SELECT ar.nombre, al.varios_artistas, count(p.id), al.anio
               FROM ALBUMES al
               JOIN ARTISTAS ar ON ar.id = al.artista_id
               LEFT JOIN PISTAS p ON p.album_id = al.id
              GROUP BY al.id
              ORDER BY al.id",
        )
        .expect("preparar álbumes");
    sentencia
        .query_map([], |fila| {
            Ok(AlbumEnBd {
                artista: fila.get(0)?,
                varios_artistas: fila.get(1)?,
                num_pistas: fila.get(2)?,
                anio: fila.get(3)?,
            })
        })
        .expect("consultar álbumes")
        .collect::<Result<Vec<_>, _>>()
        .expect("leer álbumes")
}

#[test]
fn agrupa_recopilatorio_bajo_varios_artistas() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let biblioteca = preparar_biblioteca(&temporal.path().join("biblioteca"));
    let ruta_bd = temporal.path().join("mmmusic.db");

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca.clone()], ModoEscaneo::Incremental);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let (nuevas, _, _) = resumen(&esperar_final(&rx));
    assert_eq!(nuevas, 3);

    let conn = abrir_bd(&ruta_bd);
    let albumes = albumes(&conn);
    assert_eq!(albumes.len(), 1, "debe quedar un único álbum agrupado");
    assert_eq!(albumes[0].artista, "Varios artistas");
    assert!(albumes[0].varios_artistas);
    assert_eq!(albumes[0].num_pistas, 3);
    assert_eq!(
        albumes[0].anio,
        Some(2019),
        "el año es el mínimo de las pistas"
    );

    let (carpetas, etiquetados): (String, i64) = conn
        .query_row(
            "SELECT group_concat(DISTINCT carpeta), sum(artista_album_etiquetado) FROM PISTAS",
            [],
            |fila| Ok((fila.get::<_, String>(0)?, fila.get(1)?)),
        )
        .expect("metadatos de pistas");
    assert!(carpetas.contains("Recopilatorio"));
    assert_eq!(etiquetados, 0);
}

#[test]
fn consolidacion_es_idempotente_entre_escaneos() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let biblioteca = preparar_biblioteca(&temporal.path().join("biblioteca"));
    let ruta_bd = temporal.path().join("mmmusic.db");

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca.clone()], ModoEscaneo::Incremental);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let _ = esperar_final(&rx);

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca], ModoEscaneo::Incremental);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let _ = esperar_final(&rx);

    let conn = abrir_bd(&ruta_bd);
    let albumes = albumes(&conn);
    assert_eq!(albumes.len(), 1);
    assert_eq!(albumes[0].num_pistas, 3);
    let artistas_varios: i64 = conn
        .query_row(
            "SELECT count(*) FROM ARTISTAS WHERE nombre_norm = 'varios artistas'",
            [],
            |fila| fila.get(0),
        )
        .expect("contar artistas");
    assert_eq!(artistas_varios, 1, "el artista se crea una sola vez");
}

#[test]
fn revierte_al_etiquetar_albumartist() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let biblioteca = preparar_biblioteca(&temporal.path().join("biblioteca"));
    let ruta_bd = temporal.path().join("mmmusic.db");

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca.clone()], ModoEscaneo::Incremental);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let _ = esperar_final(&rx);

    std::thread::sleep(Duration::from_millis(1_100));
    for nombre in FICHEROS {
        etiquetar_albumartist(
            &biblioteca.join("Recopilatorio").join(nombre),
            "Colectivo Prueba",
        );
    }

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca], ModoEscaneo::Incremental);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let (_, actualizadas, _) = resumen(&esperar_final(&rx));
    assert_eq!(actualizadas, 3);

    let conn = abrir_bd(&ruta_bd);
    let albumes = albumes(&conn);
    assert_eq!(albumes.len(), 1, "debe quedar un álbum normal");
    assert_eq!(albumes[0].artista, "Colectivo Prueba");
    assert!(!albumes[0].varios_artistas);
    assert_eq!(albumes[0].num_pistas, 3);
    let artistas_varios: i64 = conn
        .query_row(
            "SELECT count(*) FROM ARTISTAS WHERE nombre_norm = 'varios artistas'",
            [],
            |fila| fila.get(0),
        )
        .expect("contar artistas");
    assert_eq!(artistas_varios, 0, "el artista huérfano se limpia");
}

#[test]
fn reescaneo_completo_reagrupa_una_base_fragmentada() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let biblioteca = preparar_biblioteca(&temporal.path().join("biblioteca"));
    let ruta_bd = temporal.path().join("mmmusic.db");

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca.clone()], ModoEscaneo::Incremental);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let _ = esperar_final(&rx);

    {
        let conn = abrir_bd(&ruta_bd);
        conn.execute_batch(
            "INSERT INTO ALBUMES (artista_id, titulo, titulo_norm, anio, caratula_ruta, varios_artistas, carpeta, creado_en)
             SELECT p.artista_id, 'Recopilatorio Prueba', 'recopilatorio prueba', al.anio, NULL, 0, NULL, 'x'
               FROM PISTAS p JOIN ALBUMES al ON al.id = p.album_id
              GROUP BY p.artista_id;
             UPDATE PISTAS SET album_id = (
                 SELECT al2.id FROM ALBUMES al2
                  WHERE al2.artista_id = PISTAS.artista_id
                    AND al2.titulo_norm = 'recopilatorio prueba'
                    AND al2.varios_artistas = 0
             );
             DELETE FROM ALBUMES WHERE varios_artistas = 1;
             UPDATE AJUSTES SET valor = '1' WHERE clave = 'reescaneo_completo_pendiente';",
        )
        .expect("fragmentar la base");
        assert_eq!(albumes(&conn).len(), 3, "base fragmentada para la prueba");
    }

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca], ModoEscaneo::Completo);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let (_, actualizadas, _) = resumen(&esperar_final(&rx));
    assert_eq!(
        actualizadas, 3,
        "el modo completo relee aunque no cambie el mtime"
    );

    let conn = abrir_bd(&ruta_bd);
    let albumes = albumes(&conn);
    assert_eq!(albumes.len(), 1);
    assert_eq!(albumes[0].artista, "Varios artistas");
    assert_eq!(albumes[0].num_pistas, 3);
    let pendiente: String = conn
        .query_row(
            "SELECT valor FROM AJUSTES WHERE clave = 'reescaneo_completo_pendiente'",
            [],
            |fila| fila.get(0),
        )
        .expect("bandera");
    assert_eq!(pendiente, "0", "solo se limpia si el escaneo se completa");
}
