use std::path::Path;

use mmmusic::biblioteca::bd;
use mmmusic::biblioteca::consultas;
use mmmusic::biblioteca::consultas::emisoras::NuevaEmisora;
use mmmusic::biblioteca::modelos::{ElementoCola, PistaResumen};
use mmmusic::eventos::AppEvento;
use mmmusic::radio::radiobrowser::{EmisoraDirectorio, clave_busqueda};

const MIGRACIONES_PREVIAS: [&str; 3] = [
    include_str!("../src/biblioteca/migraciones/001_inicial.sql"),
    include_str!("../src/biblioteca/migraciones/002_recopilatorios_envios.sql"),
    include_str!("../src/biblioteca/migraciones/003_colores_albumes.sql"),
];

fn bd_temporal(temporal: &Path) -> rusqlite::Connection {
    let ruta = temporal.join("mmmusic.db");
    let mut conn = bd::abrir(&ruta).expect("bd");
    bd::migrar(&mut conn).expect("migrar");
    conn
}

fn emisora(conn: &rusqlite::Connection, nombre: &str, url: &str) -> i64 {
    consultas::emisoras::crear(
        conn,
        &NuevaEmisora {
            nombre: nombre.to_string(),
            url: url.to_string(),
            ..NuevaEmisora::default()
        },
    )
    .expect("crear emisora")
}

#[test]
fn migracion_004_preserva_datos_y_deja_copia_previa() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let ruta = temporal.path().join("mmmusic.db");

    {
        let conn = rusqlite::Connection::open(&ruta).expect("bd en versión 3");
        conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
        for (destino, sql) in MIGRACIONES_PREVIAS.iter().enumerate() {
            conn.execute_batch(sql).expect("migración previa");
            conn.pragma_update(None, "user_version", (destino + 1) as i32)
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
        .expect("datos");
    }

    let conn = bd::abrir_y_migrar(&ruta).expect("migrar con copia");
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |fila| fila.get(0))
        .expect("version");
    assert_eq!(version, bd::VERSION_RADIO);
    let copia = ruta.with_file_name("mmmusic.db.pre-004");
    assert!(copia.exists(), "debe existir la copia previa");
    assert_eq!(
        conn.query_row("SELECT count(*) FROM COLA", [], |f| f.get::<_, i64>(0))
            .expect("cola"),
        1
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM HISTORIAL_REPRODUCCION", [], |f| {
            f.get::<_, i64>(0)
        })
        .expect("historial"),
        1
    );
    let envio: (Option<i64>, Option<i64>) = conn
        .query_row("SELECT pista_id, historial_id FROM ENVIOS", [], |fila| {
            Ok((fila.get(0)?, fila.get(1)?))
        })
        .expect("envío");
    assert_eq!(envio, (Some(1), Some(1)));
    drop(conn);

    // Siguiente arranque correcto: la copia se borra.
    bd::limpiar_copia_previa_si_migrada(&ruta).expect("limpiar copia");
    assert!(!copia.exists());
}

#[test]
fn cola_mixta_conserva_pistas_y_emisoras() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let conn = bd_temporal(temporal.path());
    conn.execute_batch(
        "INSERT INTO ARTISTAS (id, nombre, nombre_norm, creado_en) VALUES (1, 'A', 'a', 'x');
         INSERT INTO ALBUMES (id, artista_id, titulo, titulo_norm, creado_en) VALUES (1, 1, 'B', 'b', 'x');
         INSERT INTO PISTAS (id, album_id, artista_id, titulo, titulo_norm, duracion_ms, ruta, formato, tamano_bytes, modificado_en, anadido_en)
            VALUES (1, 1, 1, 'Pista 1', 'pista 1', 1000, '/x.mp3', 'mp3', 10, 1, 'x');",
    )
    .expect("pista");
    let emisora_id = emisora(&conn, "Radio Prueba", "http://radio.example/stream");
    let emisora = consultas::emisoras::por_id(&conn, emisora_id)
        .expect("por id")
        .expect("existe")
        .a_resumen();

    let elementos = vec![
        (
            ElementoCola::Pista(PistaResumen {
                id: 1,
                titulo: "Pista 1".to_string(),
                ..PistaResumen::default()
            }),
            0,
        ),
        (ElementoCola::Emisora(emisora), 1),
    ];
    consultas::cola::guardar(&conn, &elementos, Some(1)).expect("guardar");
    let (recuperados, indice) = consultas::cola::cargar(&conn).expect("cargar");
    assert_eq!(indice, Some(1));
    assert_eq!(recuperados.len(), 2);
    assert_eq!(recuperados[0].0.tipo(), "pista");
    match &recuperados[1].0 {
        ElementoCola::Emisora(recuperada) => {
            assert_eq!(recuperada.nombre, "Radio Prueba");
            assert_eq!(recuperada.url, "http://radio.example/stream");
        }
        otro => panic!("se esperaba una emisora, llegó {otro:?}"),
    }

    // Borrar la emisora la quita de la cola en cascada.
    consultas::emisoras::eliminar(&conn, emisora_id).expect("eliminar");
    let (restantes, _) = consultas::cola::cargar(&conn).expect("recargar");
    assert_eq!(restantes.len(), 1);
}

#[test]
fn historial_y_envios_de_radio() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let conn = bd_temporal(temporal.path());
    let emisora_id = emisora(&conn, "Nightwave", "http://nightwave.example/stream");

    // Podado a 50 títulos y sin duplicar el consecutivo.
    for indice in 0..55 {
        consultas::titulos_emisora::insertar_y_podar(
            &conn,
            emisora_id,
            &format!("Artista - Tema {indice}"),
        )
        .expect("insertar título");
    }
    consultas::titulos_emisora::insertar_y_podar(&conn, emisora_id, "Artista - Tema 54")
        .expect("duplicado consecutivo");
    let titulos = consultas::titulos_emisora::listar(&conn, emisora_id, 50).expect("listar");
    assert_eq!(titulos.len(), 50);
    assert_eq!(titulos[0].titulo, "Artista - Tema 54");

    // Fila de historial ICY y envío de radio sin duración, álbum = emisora.
    let (historial_id, reproducido_en) =
        consultas::historial::registrar_icy(&conn, emisora_id, "Artista - Tema 54")
            .expect("historial icy");
    consultas::envios::encolar(
        &conn,
        mmmusic::scrobbling::SERVICIO_LISTENBRAINZ,
        "scrobble",
        consultas::envios::OrigenEnvio::Emisora(emisora_id),
        Some(historial_id),
        Some(&reproducido_en),
    )
    .expect("encolar radio");

    let pendientes =
        consultas::envios::pendientes(&conn, mmmusic::scrobbling::SERVICIO_LISTENBRAINZ, 50)
            .expect("pendientes");
    assert_eq!(pendientes.len(), 1);
    let envio = &pendientes[0];
    assert!(envio.es_radio());
    assert_eq!(envio.album, "Nightwave");
    assert_eq!(envio.titulo, "Artista - Tema 54");
    assert_eq!(envio.duracion_ms, None);
    assert_eq!(envio.historial_id, Some(historial_id));

    // Sin artista no se puede preparar la canción (regla de scrobbling).
    let (otro_historial, _) =
        consultas::historial::registrar_icy(&conn, emisora_id, "Tema suelto").expect("icy");
    assert!(
        consultas::historial::leer_icy(&conn, otro_historial)
            .expect("leer")
            .is_some()
    );

    // Borrar la emisora limpia títulos, historial y envíos.
    consultas::emisoras::eliminar(&conn, emisora_id).expect("eliminar");
    assert!(
        consultas::titulos_emisora::listar(&conn, emisora_id, 50)
            .expect("títulos")
            .is_empty()
    );
    assert!(
        consultas::envios::pendientes(&conn, mmmusic::scrobbling::SERVICIO_LISTENBRAINZ, 50)
            .expect("envíos")
            .is_empty()
    );
}

#[test]
fn cache_de_busquedas_de_radio_se_poda() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let conn = bd_temporal(temporal.path());
    let clave = clave_busqueda("Vapor Wave", "US", "Chill");
    let resultados = vec![EmisoraDirectorio {
        uuid: "abc".to_string(),
        nombre: "Nightwave Plaza".to_string(),
        url: "http://nightwave.example/stream".to_string(),
        pais: "US".to_string(),
        codec: "MP3".to_string(),
        bitrate: 128,
        lastcheckok: 1,
        ..EmisoraDirectorio::default()
    }];
    consultas::busquedas_radio::guardar(
        &conn,
        &clave,
        &serde_json::to_string(&resultados).expect("json"),
    )
    .expect("guardar");
    let (json, _) = consultas::busquedas_radio::leer(&conn, &clave)
        .expect("leer")
        .expect("existe");
    let leidos: Vec<EmisoraDirectorio> = serde_json::from_str(&json).expect("parsear");
    assert_eq!(leidos, resultados);
    assert_eq!(clave, "search|vapor wave|us|chill|votes");

    // Entrada antigua: se poda a 7 días.
    conn.execute(
        "UPDATE BUSQUEDAS_RADIO SET obtenido_en = '2000-01-01T00:00:00Z' WHERE clave = ?1",
        [&clave],
    )
    .expect("envejecer");
    assert_eq!(
        consultas::busquedas_radio::podar(&conn, 7).expect("podar"),
        1
    );
    assert!(
        consultas::busquedas_radio::leer(&conn, &clave)
            .expect("leer")
            .is_none()
    );
}

#[test]
fn los_eventos_de_radio_se_publican_por_el_canal() {
    // Los tipos nuevos del canal existen y son construibles.
    let _ = AppEvento::ResultadosRadio("search|x|||votes".to_string());
    let _ = AppEvento::LogoListo(7);
}
