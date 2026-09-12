use std::time::Duration;

use mmmusic::biblioteca::bd;
use mmmusic::biblioteca::consultas;
use mmmusic::scrobbling::planificador::{self, Clasificacion};
use mmmusic::scrobbling::regla;
use mmmusic::scrobbling::{LOTE_MAXIMO, SERVICIO_LASTFM, SERVICIO_LISTENBRAINZ};

fn bd_con_pista() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ruta = dir.path().join("mmmusic.db");
    let mut conn = bd::abrir(&ruta).expect("abrir bd");
    bd::migrar(&mut conn).expect("migrar");
    conn.execute_batch(
        "INSERT INTO ARTISTAS (id, nombre, nombre_norm, creado_en) VALUES (1, 'A', 'a', 'x');
         INSERT INTO ALBUMES (id, artista_id, titulo, titulo_norm, creado_en) VALUES (1, 1, 'B', 'b', 'x');
         INSERT INTO PISTAS (id, album_id, artista_id, titulo, titulo_norm, duracion_ms, ruta, formato, tamano_bytes, modificado_en, anadido_en)
             VALUES (1, 1, 1, 'Pista', 'pista', 200000, '/musica/uno.mp3', 'mp3', 10, 1, 'x');
         INSERT INTO HISTORIAL_REPRODUCCION (id, pista_id, reproducido_en, completada) VALUES (1, 1, '2026-09-12T10:00:00Z', 1);",
    )
    .expect("datos");
    (dir, conn)
}

#[test]
fn encola_y_persiste_envios_al_reabrir() {
    let (dir, conn) = bd_con_pista();
    let ruta = dir.path().join("mmmusic.db");
    consultas::envios::encolar(
        &conn,
        SERVICIO_LISTENBRAINZ,
        "scrobble",
        1,
        Some(1),
        Some("2026-09-12T10:00:00Z"),
    )
    .expect("encolar");
    assert_eq!(
        consultas::envios::contar_pendientes(&conn, SERVICIO_LISTENBRAINZ).expect("contar"),
        1
    );
    drop(conn);

    let conn = bd::abrir(&ruta).expect("reabrir");
    let pendientes =
        consultas::envios::pendientes(&conn, SERVICIO_LISTENBRAINZ, LOTE_MAXIMO).expect("leer");
    assert_eq!(pendientes.len(), 1, "el envío sobrevive al cierre");
    assert_eq!(pendientes[0].historial_id, Some(1));
    assert_eq!(
        pendientes[0].reproducido_en.as_deref(),
        Some("2026-09-12T10:00:00Z")
    );
    assert_eq!(pendientes[0].titulo, "Pista");
    assert_eq!(pendientes[0].album, "B");
}

#[test]
fn love_reemplaza_al_pendiente_anterior() {
    let (_dir, conn) = bd_con_pista();
    let primero =
        consultas::envios::encolar(&conn, SERVICIO_LASTFM, "love", 1, None, None).expect("love");
    let segundo = consultas::envios::encolar(&conn, SERVICIO_LASTFM, "unlove", 1, None, None)
        .expect("unlove");

    let estado_primero: String = conn
        .query_row(
            "SELECT estado FROM ENVIOS WHERE id = ?1",
            [primero],
            |fila| fila.get(0),
        )
        .expect("estado del primero");
    assert_eq!(estado_primero, "descartado");
    let motivo: Option<String> = conn
        .query_row(
            "SELECT error_msg FROM ENVIOS WHERE id = ?1",
            [primero],
            |fila| fila.get(0),
        )
        .expect("motivo");
    assert!(motivo.is_some());
    assert_eq!(
        consultas::envios::contar_pendientes(&conn, SERVICIO_LASTFM).expect("contar"),
        1
    );
    let tipo: String = conn
        .query_row("SELECT tipo FROM ENVIOS WHERE id = ?1", [segundo], |fila| {
            fila.get(0)
        })
        .expect("tipo");
    assert_eq!(tipo, "unlove");
}

#[test]
fn reprograma_solo_los_errores_de_autenticacion() {
    let (_dir, conn) = bd_con_pista();
    let auth = consultas::envios::encolar(&conn, SERVICIO_LISTENBRAINZ, "scrobble", 1, None, None)
        .expect("auth");
    let red = consultas::envios::encolar(&conn, SERVICIO_LISTENBRAINZ, "scrobble", 1, None, None)
        .expect("red");
    consultas::envios::marcar_error(
        &conn,
        auth,
        "auth:token no válido",
        &planificador::proximo_auth(),
        1,
    )
    .expect("marcar auth");
    consultas::envios::marcar_error(
        &conn,
        red,
        "timeout",
        &planificador::proximo_reintento(0),
        1,
    )
    .expect("marcar red");

    let reprogramados =
        consultas::envios::reprogramar_errores_auth(&conn, SERVICIO_LISTENBRAINZ).expect("repro");
    assert_eq!(reprogramados, 1, "solo se reprograma el error de auth");
    let proximo: String = conn
        .query_row(
            "SELECT proximo_intento_en FROM ENVIOS WHERE id = ?1",
            [auth],
            |fila| fila.get(0),
        )
        .expect("proximo auth");
    assert!(proximo <= bd::ahora_iso());
    let proximo_red: String = conn
        .query_row(
            "SELECT proximo_intento_en FROM ENVIOS WHERE id = ?1",
            [red],
            |fila| fila.get(0),
        )
        .expect("proximo red");
    assert!(proximo_red > bd::ahora_iso());

    consultas::envios::limpiar_errores_auth(&conn, SERVICIO_LISTENBRAINZ).expect("limpiar");
    let mensaje: Option<String> = conn
        .query_row(
            "SELECT error_msg FROM ENVIOS WHERE id = ?1",
            [auth],
            |fila| fila.get(0),
        )
        .expect("mensaje");
    assert!(mensaje.is_none());
}

#[test]
fn descarta_al_agotar_los_reintentos() {
    let (_dir, conn) = bd_con_pista();
    let envio = consultas::envios::encolar(&conn, SERVICIO_LISTENBRAINZ, "scrobble", 1, None, None)
        .expect("encolar");
    consultas::envios::marcar_error(
        &conn,
        envio,
        "timeout",
        &planificador::proximo_reintento(20),
        20,
    )
    .expect("marcar error");
    let pendiente = consultas::envios::pendientes(&conn, SERVICIO_LISTENBRAINZ, LOTE_MAXIMO)
        .expect("pendientes");
    assert!(pendiente.is_empty(), "aún no toca reintentar");

    let intentos_previos = 20u32;
    let nuevos = intentos_previos + 1;
    assert!(planificador::descartar_por_intentos(nuevos));
    consultas::envios::descartar(&conn, envio, "agotados los reintentos").expect("descartar");
    let estado: String = conn
        .query_row("SELECT estado FROM ENVIOS WHERE id = ?1", [envio], |fila| {
            fila.get(0)
        })
        .expect("estado");
    assert_eq!(estado, "descartado");
}

#[test]
fn favoritas_alternan_de_forma_idempotente() {
    let (_dir, conn) = bd_con_pista();
    assert!(!consultas::favoritas::es_favorita(&conn, 1).expect("consultar"));
    assert!(consultas::favoritas::alternar(&conn, 1).expect("marcar"));
    assert!(consultas::favoritas::es_favorita(&conn, 1).expect("consultar"));
    assert_eq!(consultas::favoritas::contar(&conn).expect("contar"), 1);
    assert_eq!(consultas::favoritas::ids(&conn).expect("ids"), vec![1]);
    let listado = consultas::favoritas::listar(&conn).expect("listar");
    assert_eq!(listado.len(), 1);
    assert_eq!(listado[0].titulo, "Pista");
    assert!(!consultas::favoritas::alternar(&conn, 1).expect("quitar"));
    assert_eq!(consultas::favoritas::contar(&conn).expect("contar"), 0);
}

#[test]
fn regla_y_planificador_expuestos() {
    assert!(!regla::elegible(30_000));
    assert!(regla::elegible(30_001));
    assert_eq!(regla::umbral_ms(600_000), 240_000);
    assert_eq!(planificador::espera_reintento(1), Duration::from_secs(60));
    assert_eq!(
        planificador::clasificar(401, None),
        Clasificacion::Autenticacion
    );
    assert_eq!(
        planificador::clasificar(400, Some(13)),
        Clasificacion::Descartar
    );
}
