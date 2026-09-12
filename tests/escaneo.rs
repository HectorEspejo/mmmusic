use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use mmmusic::biblioteca::bd;
use mmmusic::biblioteca::escaner::{self, ModoEscaneo};
use mmmusic::eventos::{AppEvento, EventoEscaneo};

const FIXTURES: [&str; 6] = [
    "prueba.mp3",
    "prueba.flac",
    "prueba.ogg",
    "prueba.opus",
    "prueba.m4a",
    "prueba.wav",
];

fn preparar_biblioteca(dir: &Path) -> PathBuf {
    let album = dir.join("Artista Prueba/Álbum Ñandú");
    fs::create_dir_all(&album).expect("crear carpeta de álbum");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for nombre in FIXTURES {
        fs::copy(fixtures.join(nombre), album.join(nombre)).expect("copiar fixture");
    }
    dir.to_path_buf()
}

fn abrir_bd(ruta: &Path) -> rusqlite::Connection {
    let mut conn = bd::abrir(ruta).expect("abrir bd");
    bd::migrar(&mut conn).expect("migrar");
    conn
}

fn escanear(ruta_bd: &Path, raices: Vec<PathBuf>) -> (Receiver<AppEvento>, escaner::ManejoEscaneo) {
    let (tx, rx) = mpsc::channel();
    let dir_caratulas = ruta_bd.parent().unwrap_or(Path::new(".")).join("caratulas");
    let manejo = escaner::lanzar(
        ruta_bd.to_path_buf(),
        raices,
        dir_caratulas,
        ModoEscaneo::Incremental,
        tx,
    )
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

#[test]
fn indexa_los_seis_formatos_y_normaliza() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let biblioteca = preparar_biblioteca(&temporal.path().join("biblioteca"));
    let ruta_bd = temporal.path().join("mmmusic.db");

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca.clone()]);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let evento = esperar_final(&rx);
    let (nuevas, actualizadas, eliminadas) = resumen(&evento);
    assert_eq!(nuevas, 6);
    assert_eq!(actualizadas, 0);
    assert_eq!(eliminadas, 0);

    let conn = abrir_bd(&ruta_bd);
    let total: i64 = conn
        .query_row("SELECT count(*) FROM PISTAS", [], |f| f.get(0))
        .expect("contar pistas");
    assert_eq!(total, 6);

    let formatos: Vec<String> = conn
        .prepare("SELECT DISTINCT formato FROM PISTAS ORDER BY formato")
        .expect("preparar")
        .query_map([], |f| f.get(0))
        .expect("consultar")
        .collect::<Result<_, _>>()
        .expect("leer formatos");
    assert_eq!(formatos, vec!["flac", "m4a", "mp3", "ogg", "opus", "wav"]);

    let (album_norm, artista_norm, anio): (String, String, Option<i64>) = conn
        .query_row(
            "SELECT ALBUMES.titulo_norm, ARTISTAS.nombre_norm, ALBUMES.anio
               FROM ALBUMES JOIN ARTISTAS ON ARTISTAS.id = ALBUMES.artista_id",
            [],
            |f| Ok((f.get(0)?, f.get(1)?, f.get(2)?)),
        )
        .expect("leer álbum");
    assert_eq!(album_norm, "album nandu");
    assert_eq!(artista_norm, "artista prueba");
    assert_eq!(anio, Some(2024));

    let duraciones: Vec<i64> = conn
        .prepare("SELECT duracion_ms FROM PISTAS")
        .expect("preparar")
        .query_map([], |f| f.get(0))
        .expect("consultar")
        .collect::<Result<_, _>>()
        .expect("leer duraciones");
    for duracion in duraciones {
        assert!(
            (800..=1200).contains(&duracion),
            "duración inesperada: {duracion} ms"
        );
    }
}

#[test]
fn reescaneo_sin_cambios_no_relee_ni_borra() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let biblioteca = preparar_biblioteca(&temporal.path().join("biblioteca"));
    let ruta_bd = temporal.path().join("mmmusic.db");

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca.clone()]);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let _ = esperar_final(&rx);

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca]);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let (nuevas, actualizadas, eliminadas) = resumen(&esperar_final(&rx));
    assert_eq!((nuevas, actualizadas, eliminadas), (0, 0, 0));

    let conn = abrir_bd(&ruta_bd);
    let total: i64 = conn
        .query_row("SELECT count(*) FROM PISTAS", [], |f| f.get(0))
        .expect("contar");
    assert_eq!(total, 6);
}

#[test]
fn elimina_pistas_desaparecidas_y_huerfanos() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let biblioteca = preparar_biblioteca(&temporal.path().join("biblioteca"));
    let ruta_bd = temporal.path().join("mmmusic.db");

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca.clone()]);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let _ = esperar_final(&rx);

    fs::remove_file(biblioteca.join("Artista Prueba/Álbum Ñandú/prueba.wav")).expect("borrar wav");
    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca]);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let (nuevas, actualizadas, eliminadas) = resumen(&esperar_final(&rx));
    assert_eq!((nuevas, actualizadas, eliminadas), (0, 0, 1));

    let conn = abrir_bd(&ruta_bd);
    let total: i64 = conn
        .query_row("SELECT count(*) FROM PISTAS", [], |f| f.get(0))
        .expect("contar");
    assert_eq!(total, 5);
    let huerfanos: i64 = conn
        .query_row("SELECT count(*) FROM ARTISTAS", [], |f| f.get(0))
        .expect("contar artistas");
    assert_eq!(huerfanos, 1, "el artista sigue teniendo pistas");
}

#[test]
fn raiz_inexistente_no_borra_sus_pistas() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let biblioteca = preparar_biblioteca(&temporal.path().join("biblioteca"));
    let ruta_bd = temporal.path().join("mmmusic.db");
    let unidad_desmontada = temporal.path().join("unidad-desmontada");

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca.clone()]);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let _ = esperar_final(&rx);

    let (rx, manejo) = escanear(&ruta_bd, vec![unidad_desmontada]);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let (nuevas, actualizadas, eliminadas) = resumen(&esperar_final(&rx));
    assert_eq!((nuevas, actualizadas, eliminadas), (0, 0, 0));

    let conn = abrir_bd(&ruta_bd);
    let total: i64 = conn
        .query_row("SELECT count(*) FROM PISTAS", [], |f| f.get(0))
        .expect("contar");
    assert_eq!(total, 6, "las pistas de la raíz ausente se conservan");
}

#[test]
fn omite_ficheros_corruptos_sin_detener_el_escaneo() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let biblioteca = preparar_biblioteca(&temporal.path().join("biblioteca"));
    let corrupto = biblioteca.join("Artista Prueba/Álbum Ñandú/corrupto.mp3");
    fs::write(&corrupto, b"no soy un mp3").expect("escribir corrupto");
    let ruta_bd = temporal.path().join("mmmusic.db");

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca]);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let evento = esperar_final(&rx);
    let (nuevas, omitidas) = match evento {
        EventoEscaneo::Terminado { resumen } => (resumen.nuevas, resumen.omitidas),
        _ => panic!("se esperaba Terminado"),
    };
    assert_eq!(nuevas, 6);
    assert_eq!(omitidas, 1);
}

#[test]
fn cancelar_conserva_lo_indexado() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let biblioteca = preparar_biblioteca(&temporal.path().join("biblioteca"));
    let album = biblioteca.join("Artista Prueba/Álbum Ñandú");
    let origen = album.join("prueba.mp3");
    for indice in 0..1000 {
        fs::copy(&origen, album.join(format!("copia-{indice:04}.mp3"))).expect("copiar");
    }
    let ruta_bd = temporal.path().join("mmmusic.db");

    let (tx, rx) = mpsc::channel();
    let dir_caratulas = ruta_bd.parent().unwrap_or(Path::new(".")).join("caratulas");
    let manejo = escaner::lanzar(
        ruta_bd.clone(),
        vec![biblioteca.clone()],
        dir_caratulas,
        ModoEscaneo::Incremental,
        tx,
    )
    .expect("lanzar");
    let limite = Instant::now() + Duration::from_secs(30);
    loop {
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(AppEvento::Escaneo(EventoEscaneo::Progreso { .. })) => break,
            Ok(_) if Instant::now() < limite => continue,
            _ => panic!("no llegó el primer progreso"),
        }
    }
    manejo.cancelar();
    assert!(manejo.esperar(Duration::from_secs(30)));
    let evento = esperar_final(&rx);
    assert!(
        matches!(evento, EventoEscaneo::Cancelado { .. }),
        "se esperaba Cancelado: {evento:?}"
    );

    let conn = abrir_bd(&ruta_bd);
    let parciales: i64 = conn
        .query_row("SELECT count(*) FROM PISTAS", [], |f| f.get(0))
        .expect("contar");
    assert!(parciales > 0, "debe conservarse lo indexado");
    assert!(parciales < 1006, "no debió terminar el escaneo");
    let estado: String = conn
        .query_row(
            "SELECT estado FROM ESCANEOS ORDER BY id DESC LIMIT 1",
            [],
            |f| f.get(0),
        )
        .expect("estado");
    assert_eq!(estado, "cancelado");
}

#[test]
fn marca_escaneo_huerfano_como_error() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let biblioteca = preparar_biblioteca(&temporal.path().join("biblioteca"));
    let ruta_bd = temporal.path().join("mmmusic.db");
    let conn = abrir_bd(&ruta_bd);
    conn.execute(
        "INSERT INTO ESCANEOS (iniciado_en, estado) VALUES ('2026-01-01T00:00:00Z', 'en_curso')",
        [],
    )
    .expect("insertar huérfano");
    drop(conn);

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca]);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let _ = esperar_final(&rx);

    let conn = abrir_bd(&ruta_bd);
    let (estado, mensaje): (String, Option<String>) = conn
        .query_row(
            "SELECT estado, error_msg FROM ESCANEOS ORDER BY id ASC LIMIT 1",
            [],
            |f| Ok((f.get(0)?, f.get(1)?)),
        )
        .expect("leer huérfano");
    assert_eq!(estado, "error");
    assert_eq!(mensaje.as_deref(), Some("proceso interrumpido"));
}

#[test]
fn registra_el_escaneo_en_la_tabla() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let biblioteca = preparar_biblioteca(&temporal.path().join("biblioteca"));
    let ruta_bd = temporal.path().join("mmmusic.db");

    let (rx, manejo) = escanear(&ruta_bd, vec![biblioteca]);
    assert!(manejo.esperar(Duration::from_secs(60)));
    let _ = esperar_final(&rx);

    let conn = abrir_bd(&ruta_bd);
    let (estado, nuevas, finalizado): (String, i64, Option<String>) = conn
        .query_row(
            "SELECT estado, nuevas, finalizado_en FROM ESCANEOS ORDER BY id DESC LIMIT 1",
            [],
            |f| Ok((f.get(0)?, f.get(1)?, f.get(2)?)),
        )
        .expect("leer escaneo");
    assert_eq!(estado, "completado");
    assert_eq!(nuevas, 6);
    assert!(finalizado.is_some());
}
