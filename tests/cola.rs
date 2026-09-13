use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use mmmusic::biblioteca::escaner::{self, ModoEscaneo};
use mmmusic::biblioteca::modelos::{ElementoCola, PistaResumen};
use mmmusic::biblioteca::{bd, consultas};
use mmmusic::config::ConfigEcualizador;
use mmmusic::eventos::{AppEvento, EventoEscaneo, NivelAviso};
use mmmusic::reproductor::cola::Cola;
use mmmusic::reproductor::estado::{Estado, Repeticion};
use mmmusic::reproductor::{self, ComandoReproductor};

fn elemento(id: i64) -> ElementoCola {
    ElementoCola::Pista(PistaResumen {
        id,
        titulo: format!("Pista {id}"),
        ..PistaResumen::default()
    })
}

#[test]
fn aleatorio_y_repeticion_en_la_cola() {
    let mut cola = Cola::nueva();
    cola.reemplazar((1..=6).map(elemento).collect(), 2);
    let actual = cola.actual().and_then(|item| item.elemento.pista_id());
    cola.alternar_aleatorio();
    assert_eq!(
        cola.actual().and_then(|item| item.elemento.pista_id()),
        actual
    );
    cola.alternar_aleatorio();
    assert_eq!(cola.indice, Some(2));
    assert_eq!(cola.items[2].posicion_orig, 2);

    cola.fijar_repeticion(Repeticion::Una);
    assert_eq!(cola.siguiente(), Some(2));
    cola.fijar_repeticion(Repeticion::Todo);
    cola.indice = Some(5);
    assert_eq!(cola.siguiente(), Some(0));
    cola.fijar_repeticion(Repeticion::No);
    assert_eq!(cola.siguiente(), None);
}

fn preparar_bd_con_fixtures(temporal: &Path) -> PathBuf {
    let biblioteca = temporal.join("biblioteca");
    let album = biblioteca.join("Artista Prueba/Álbum Ñandú");
    fs::create_dir_all(&album).expect("crear álbum");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for nombre in ["prueba.mp3", "prueba.flac", "prueba.ogg"] {
        fs::copy(fixtures.join(nombre), album.join(nombre)).expect("copiar fixture");
    }
    let ruta_bd = temporal.join("mmmusic.db");
    let mut conn = bd::abrir(&ruta_bd).expect("bd");
    bd::migrar(&mut conn).expect("migrar");
    drop(conn);

    let (tx, rx) = mpsc::channel();
    let dir_caratulas = ruta_bd.parent().unwrap_or(Path::new(".")).join("caratulas");
    let manejo = escaner::lanzar(
        ruta_bd.clone(),
        vec![biblioteca],
        dir_caratulas,
        ModoEscaneo::Incremental,
        tx,
    )
    .expect("escanear");
    assert!(manejo.esperar(Duration::from_secs(30)));
    let limite = Instant::now() + Duration::from_secs(30);
    while Instant::now() < limite {
        if let Ok(AppEvento::Escaneo(EventoEscaneo::Terminado { .. })) =
            rx.recv_timeout(Duration::from_secs(5))
        {
            break;
        }
    }
    ruta_bd
}

fn ids_en_bd(ruta_bd: &Path) -> Vec<i64> {
    let conn = bd::abrir(ruta_bd).expect("bd");
    let mut sentencia = conn
        .prepare("SELECT id FROM PISTAS ORDER BY id")
        .expect("preparar");
    sentencia
        .query_map([], |fila| fila.get(0))
        .expect("consultar")
        .collect::<Result<Vec<i64>, _>>()
        .expect("leer")
}

fn esperar_estado(
    rx: &Receiver<AppEvento>,
    condicion: impl Fn(&Estado) -> bool,
    mensaje: &str,
) -> Estado {
    let limite = Instant::now() + Duration::from_secs(20);
    while Instant::now() < limite {
        if let Ok(AppEvento::Reproductor(estado)) = rx.recv_timeout(Duration::from_millis(500))
            && condicion(&estado.estado)
        {
            return estado.estado;
        }
    }
    panic!("no se alcanzó el estado esperado: {mensaje}");
}

#[test]
fn reproduce_persiste_y_restaura_la_cola() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let ruta_bd = preparar_bd_con_fixtures(temporal.path());
    let ids = ids_en_bd(&ruta_bd);
    assert_eq!(ids.len(), 3);

    let (tx, rx) = mpsc::channel();
    let (tx_scrobbling, _rx_scrobbling) = mpsc::channel();
    let conn = bd::abrir(&ruta_bd).expect("bd");
    let elementos: Vec<ElementoCola> = consultas::pistas_resumen_por_ids(&conn, &ids)
        .expect("resúmenes")
        .into_iter()
        .map(ElementoCola::Pista)
        .collect();
    drop(conn);
    let (manejo, _watch) = reproductor::lanzar(
        ruta_bd.clone(),
        70,
        15,
        ConfigEcualizador::default(),
        tx,
        tx_scrobbling,
    )
    .expect("lanzar reproductor");
    manejo.enviar(ComandoReproductor::ReemplazarCola {
        elementos,
        indice: 1,
    });
    esperar_estado(
        &rx,
        |estado| *estado == Estado::Reproduciendo,
        "reproduciendo",
    );
    manejo.enviar(ComandoReproductor::AlternarPausa);
    esperar_estado(&rx, |estado| *estado == Estado::Pausado, "pausado");
    manejo.enviar(ComandoReproductor::Volumen { valor: 42 });
    std::thread::sleep(Duration::from_millis(300));
    manejo.apagar();

    let conn = bd::abrir(&ruta_bd).expect("bd");
    let en_cola: i64 = conn
        .query_row("SELECT count(*) FROM COLA", [], |fila| fila.get(0))
        .expect("contar cola");
    assert_eq!(en_cola, 3);
    let posicion: String = conn
        .query_row(
            "SELECT valor FROM AJUSTES WHERE clave = 'cola_posicion'",
            [],
            |fila| fila.get(0),
        )
        .expect("cola_posicion");
    assert_eq!(posicion, "1");
    let volumen: String = conn
        .query_row(
            "SELECT valor FROM AJUSTES WHERE clave = 'volumen'",
            [],
            |fila| fila.get(0),
        )
        .expect("volumen");
    assert_eq!(volumen, "42");
    let historial: i64 = conn
        .query_row("SELECT count(*) FROM HISTORIAL_REPRODUCCION", [], |fila| {
            fila.get(0)
        })
        .expect("historial");
    assert!(historial >= 1, "debe haberse registrado la reproducción");
}

#[test]
fn tres_fallos_seguidos_pasan_a_detenido() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let ruta_bd = temporal.path().join("mmmusic.db");
    let mut conn = bd::abrir(&ruta_bd).expect("bd");
    bd::migrar(&mut conn).expect("migrar");
    conn.execute_batch(
        "INSERT INTO ARTISTAS (id, nombre, nombre_norm, creado_en) VALUES (1, 'A', 'a', 'x');
         INSERT INTO ALBUMES (id, artista_id, titulo, titulo_norm, creado_en) VALUES (1, 1, 'B', 'b', 'x');
         INSERT INTO PISTAS (id, album_id, artista_id, titulo, titulo_norm, duracion_ms, ruta, formato, tamano_bytes, modificado_en, anadido_en)
             VALUES (1, 1, 1, 'F1', 'f1', 1000, '/no/existe/1.mp3', 'mp3', 1, 1, 'x'),
                    (2, 1, 1, 'F2', 'f2', 1000, '/no/existe/2.mp3', 'mp3', 1, 1, 'x'),
                    (3, 1, 1, 'F3', 'f3', 1000, '/no/existe/3.mp3', 'mp3', 1, 1, 'x');",
    )
    .expect("datos");
    drop(conn);

    let (tx, rx) = mpsc::channel();
    let (tx_scrobbling, _rx_scrobbling) = mpsc::channel();
    let (manejo, _watch) = reproductor::lanzar(
        ruta_bd,
        50,
        15,
        ConfigEcualizador::default(),
        tx,
        tx_scrobbling,
    )
    .expect("lanzar reproductor");
    manejo.enviar(ComandoReproductor::ReemplazarCola {
        elementos: vec![elemento(1), elemento(2), elemento(3)],
        indice: 0,
    });

    let mut errores = 0;
    let limite = Instant::now() + Duration::from_secs(20);
    let mut detenido = false;
    while Instant::now() < limite && !detenido {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(AppEvento::Notificacion(NivelAviso::Error, _)) => errores += 1,
            Ok(AppEvento::Reproductor(estado))
                if estado.estado == Estado::Detenido && errores >= 3 =>
            {
                detenido = true;
            }
            _ => {}
        }
    }
    assert!(
        errores >= 3,
        "se esperaban al menos 3 errores, hubo {errores}"
    );
    assert!(
        detenido,
        "tras tres fallos el reproductor debe quedar detenido"
    );
    manejo.apagar();
}
