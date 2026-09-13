use mmmusic::biblioteca::bd;
use mmmusic::biblioteca::consultas::emisoras::NuevaEmisora;
use mmmusic::radio::listas::{EntradaLista, escribir_m3u8, parsear, parsear_m3u, parsear_pls};

#[test]
fn parsea_pls_y_m3u_ida_y_vuelta() {
    let pls = "[playlist]\nFile1=http://a.example:8000/stream\nTitle1=Radio A\nFile2=http://b.example/live\nTitle2=Radio B\n";
    let entradas = parsear_pls(pls);
    assert_eq!(entradas.len(), 2);
    assert_eq!(entradas[0].nombre.as_deref(), Some("Radio A"));
    assert_eq!(entradas[1].url, "http://b.example/live");

    let m3u8 = escribir_m3u8(&entradas);
    let releidas = parsear_m3u(&m3u8);
    assert_eq!(releidas, entradas);

    let detectado = parsear(m3u8.as_str(), None);
    assert_eq!(detectado, entradas);
}

#[test]
fn importa_contando_duplicadas_e_invalidas() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let ruta_bd = temporal.path().join("mmmusic.db");
    let mut conn = bd::abrir(&ruta_bd).expect("bd");
    bd::migrar(&mut conn).expect("migrar");

    for (nombre, url) in [
        ("Existente 1", "http://a.example/stream"),
        ("Existente 2", "http://b.example/stream"),
    ] {
        consultas_crear(&conn, nombre, url);
    }

    let mut lineas = String::from("[playlist]\n");
    for indice in 1..=15 {
        let url = match indice {
            1 => "http://a.example/stream".to_string(),
            2 => "http://b.example/stream".to_string(),
            3 => "file:///tmp/no-radio.pls".to_string(),
            _ => format!("http://emisora{indice}.example/stream"),
        };
        lineas.push_str(&format!(
            "File{indice}={url}\nTitle{indice}=Emisora {indice}\n"
        ));
    }
    let ruta = temporal.path().join("emisoras.pls");
    std::fs::write(&ruta, lineas).expect("escribir pls");

    let resumen = mmmusic::radio::importar_listas(&conn, &ruta).expect("importar");
    assert_eq!(resumen.anadidas, 12);
    assert_eq!(resumen.duplicadas, 2);
    assert_eq!(resumen.invalidas, 1);
    assert_eq!(
        resumen.mensaje(),
        "12 emisoras añadidas, 2 ya existían, 1 sin URL válida"
    );

    let total = mmmusic::biblioteca::consultas::emisoras::listar(
        &conn,
        false,
        mmmusic::biblioteca::consultas::emisoras::OrdenEmisoras::Nombre,
        false,
    )
    .expect("listar")
    .len();
    assert_eq!(total, 14);
}

#[test]
fn exporta_favoritas_a_m3u8() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let ruta_bd = temporal.path().join("mmmusic.db");
    let mut conn = bd::abrir(&ruta_bd).expect("bd");
    bd::migrar(&mut conn).expect("migrar");
    consultas_crear(&conn, "Favorita", "http://fav.example/stream");
    let favorita = mmmusic::biblioteca::consultas::emisoras::listar(
        &conn,
        false,
        mmmusic::biblioteca::consultas::emisoras::OrdenEmisoras::Nombre,
        false,
    )
    .expect("listar")[0]
        .id;
    mmmusic::biblioteca::consultas::emisoras::alternar_favorita(&conn, favorita).expect("favorita");
    consultas_crear(&conn, "Normal", "http://normal.example/stream");

    let dir = temporal.path().join("playlists");
    let ruta =
        mmmusic::radio::exportar_favoritas(&conn, &dir, "Radio favoritas").expect("exportar");
    assert_eq!(
        ruta.file_name().and_then(|n| n.to_str()),
        Some("Radio favoritas.m3u8")
    );
    let contenido = std::fs::read_to_string(&ruta).expect("leer");
    assert!(contenido.starts_with("#EXTM3U\n"));
    assert!(contenido.contains("#EXTINF:-1,Favorita\nhttp://fav.example/stream\n"));
    assert!(!contenido.contains("Normal"));

    let releidas = parsear_m3u(&contenido);
    assert_eq!(
        releidas,
        vec![EntradaLista {
            nombre: Some("Favorita".to_string()),
            url: "http://fav.example/stream".to_string(),
        }]
    );
}

fn consultas_crear(conn: &rusqlite::Connection, nombre: &str, url: &str) {
    let nueva = NuevaEmisora {
        nombre: nombre.to_string(),
        url: url.to_string(),
        ..NuevaEmisora::default()
    };
    mmmusic::biblioteca::consultas::emisoras::crear(conn, &nueva).expect("crear");
}
