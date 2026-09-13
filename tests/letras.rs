use std::fs;
use std::path::Path;

use lofty::config::WriteOptions;
use lofty::tag::{ItemKey, Tag, TagExt, TagType};

use mmmusic::biblioteca::modelos::PistaResumen;
use mmmusic::biblioteca::{bd, consultas};
use mmmusic::letras::{self, Fuente, Letra};

const LRC: &str =
    "[ar:Artista Prueba]\n[offset:-200]\n[00:01.00]primera\n[00:02.50]segunda\n[00:04.00]tercera\n";
const TXT: &str = "primera\nsegunda\ntercera\n";

fn pista(ruta: &Path) -> PistaResumen {
    PistaResumen {
        id: 1,
        titulo: "Prueba MP3".to_string(),
        artista: "Artista Prueba".to_string(),
        ruta: ruta.display().to_string(),
        ..PistaResumen::default()
    }
}

fn copiar_fixture(destino: &Path) {
    let origen = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/prueba.mp3");
    fs::copy(origen, destino).expect("copiar fixture");
}

fn escribir_id3(audio: &Path, clave: ItemKey, texto: &str) {
    let mut tag = Tag::new(TagType::Id3v2);
    tag.insert_text(clave, texto.to_string());
    tag.save_to_path(audio, WriteOptions::default())
        .expect("escribir etiqueta ID3v2");
}

#[test]
fn resuelve_lrc_junto_a_la_pista() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let audio = temporal.path().join("pista.mp3");
    copiar_fixture(&audio);
    fs::write(temporal.path().join("pista.lrc"), LRC).expect("escribir lrc");
    let resolucion = letras::resolver(&pista(&audio), &temporal.path().join("letras"));
    assert!(resolucion.fichero.is_some());
    let letra = resolucion.elegir(None);
    match letra {
        Letra::Sincronizada {
            lineas,
            offset_lrc_ms,
            fuente,
        } => {
            assert_eq!(fuente, Fuente::Fichero);
            assert_eq!(offset_lrc_ms, -200);
            assert_eq!(lineas.len(), 3);
            assert_eq!(lineas[0], (1_000, "primera".to_string()));
        }
        otra => panic!("se esperaba letra sincronizada, no {otra:?}"),
    }
    assert_eq!(resolucion.rutas.len(), 4);
}

#[test]
fn resuelve_txt_estatico_junto_a_la_pista() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let audio = temporal.path().join("pista.mp3");
    copiar_fixture(&audio);
    fs::write(temporal.path().join("pista.txt"), TXT).expect("escribir txt");
    let resolucion = letras::resolver(&pista(&audio), &temporal.path().join("letras"));
    let letra = resolucion.elegir(None);
    match letra {
        Letra::Estatica { lineas, fuente } => {
            assert_eq!(fuente, Fuente::Fichero);
            assert_eq!(lineas, vec!["primera", "segunda", "tercera"]);
        }
        otra => panic!("se esperaba letra estática, no {otra:?}"),
    }
}

#[test]
fn resuelve_desde_la_carpeta_de_letras_normalizada() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let audio = temporal.path().join("pista.mp3");
    copiar_fixture(&audio);
    let carpeta = temporal.path().join("letras");
    fs::create_dir_all(&carpeta).expect("carpeta");
    fs::write(
        carpeta.join("artista prueba - prueba mp3.lrc"),
        "[00:01.00]desde la carpeta\n",
    )
    .expect("escribir");
    let resolucion = letras::resolver(&pista(&audio), &carpeta);
    assert!(resolucion.fichero.is_some());
    assert!(resolucion.elegir(None).es_sincronizada());
}

#[test]
fn lee_uslt_embebida_sincronizada() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let audio = temporal.path().join("pista.mp3");
    copiar_fixture(&audio);
    escribir_id3(&audio, ItemKey::UnsyncLyrics, LRC);
    let resolucion = letras::resolver(&pista(&audio), &temporal.path().join("letras"));
    assert!(resolucion.fichero.is_none());
    let letra = resolucion.elegir(None);
    match letra {
        Letra::Sincronizada {
            fuente,
            offset_lrc_ms,
            ..
        } => {
            assert_eq!(fuente, Fuente::Etiqueta);
            assert_eq!(offset_lrc_ms, -200);
        }
        otra => panic!("se esperaba USLT sincronizada, no {otra:?}"),
    }
}

#[test]
fn lee_lrc_dentro_de_la_etiqueta_lyrics() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let audio = temporal.path().join("pista.flac");
    let origen = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/prueba.flac");
    fs::copy(origen, &audio).expect("copiar fixture");
    let mut tag = Tag::new(TagType::VorbisComments);
    tag.insert_text(ItemKey::Lyrics, LRC.to_string());
    tag.save_to_path(&audio, WriteOptions::default())
        .expect("escribir LYRICS");
    let resolucion = letras::resolver(&pista(&audio), &temporal.path().join("letras"));
    assert!(resolucion.elegir(None).es_sincronizada());
}

#[test]
fn la_fuente_preferida_decide_entre_fichero_y_etiqueta() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let audio = temporal.path().join("pista.mp3");
    copiar_fixture(&audio);
    fs::write(temporal.path().join("pista.lrc"), "[00:01.00]fichero\n").expect("lrc");
    escribir_id3(&audio, ItemKey::UnsyncLyrics, "[00:01.00]etiqueta\n");
    let resolucion = letras::resolver(&pista(&audio), &temporal.path().join("letras"));
    assert_eq!(
        resolucion.fuentes(),
        vec![Fuente::Fichero, Fuente::Etiqueta]
    );
    match resolucion.elegir(Some(Fuente::Etiqueta)) {
        Letra::Sincronizada { lineas, fuente, .. } => {
            assert_eq!(fuente, Fuente::Etiqueta);
            assert_eq!(lineas[0].1, "etiqueta");
        }
        otra => panic!("se esperaba la etiqueta, no {otra:?}"),
    }
    assert_eq!(resolucion.elegir(None).fuente(), Some(Fuente::Fichero));
}

#[test]
fn guarda_y_lee_la_fuente_preferida_en_la_base() {
    let mut conn = rusqlite::Connection::open_in_memory().expect("memoria");
    bd::migrar(&mut conn).expect("esquema");
    conn.execute(
        "INSERT INTO ARTISTAS (id, nombre, nombre_norm, creado_en) VALUES (1, 'A', 'a', 'x')",
        [],
    )
    .expect("artista");
    conn.execute(
        "INSERT INTO ALBUMES (id, artista_id, titulo, titulo_norm, creado_en) VALUES (1, 1, 'B', 'b', 'x')",
        [],
    )
    .expect("álbum");
    conn.execute(
        "INSERT INTO PISTAS (id, album_id, artista_id, titulo, titulo_norm, duracion_ms, ruta, formato, tamano_bytes, modificado_en, anadido_en)
         VALUES (7, 1, 1, 'C', 'c', 1000, '/x.mp3', 'mp3', 10, 1, 'x')",
        [],
    )
    .expect("pista");
    assert_eq!(
        consultas::letras::fuente_preferida(&conn, 7).expect("leer"),
        None
    );
    consultas::letras::fijar_fuente(&conn, 7, "etiqueta").expect("fijar");
    assert_eq!(
        consultas::letras::fuente_preferida(&conn, 7).expect("leer"),
        Some("etiqueta".to_string())
    );
}

#[test]
fn un_lrc_gigante_se_ignora() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let audio = temporal.path().join("pista.mp3");
    copiar_fixture(&audio);
    let grande = format!("[00:01.00]{}\n", "x".repeat(300 * 1024));
    fs::write(temporal.path().join("pista.lrc"), grande).expect("lrc");
    let resolucion = letras::resolver(&pista(&audio), &temporal.path().join("letras"));
    assert!(resolucion.fichero.is_none());
    let letra = resolucion.elegir(None);
    assert!(matches!(letra, Letra::Ninguna { .. }));
    assert_eq!(resolucion.rutas.len(), 4);
}

#[test]
fn un_txt_gigante_se_trunca_con_aviso() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let audio = temporal.path().join("pista.mp3");
    copiar_fixture(&audio);
    let grande = "x".repeat(100 * 1024);
    fs::write(temporal.path().join("pista.txt"), grande).expect("txt");
    let resolucion = letras::resolver(&pista(&audio), &temporal.path().join("letras"));
    let fichero = resolucion.fichero.expect("txt");
    assert!(fichero.aviso.is_some());
    assert!(fichero.texto.len() <= 64 * 1024);
}

#[test]
fn decodifica_latin1_de_respaldo() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let audio = temporal.path().join("pista.mp3");
    copiar_fixture(&audio);
    fs::write(
        temporal.path().join("pista.lrc"),
        b"[00:01.00]caf\xe9 con melod\xeda\n",
    )
    .expect("lrc latin1");
    let resolucion = letras::resolver(&pista(&audio), &temporal.path().join("letras"));
    match resolucion.elegir(None) {
        Letra::Sincronizada { lineas, .. } => {
            assert_eq!(lineas[0].1, "café con melodía");
        }
        otra => panic!("se esperaba letra sincronizada, no {otra:?}"),
    }
}

#[test]
fn sin_fuentes_la_letra_es_ninguna_con_rutas() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let audio = temporal.path().join("pista.mp3");
    copiar_fixture(&audio);
    let resolucion = letras::resolver(&pista(&audio), &temporal.path().join("letras"));
    let letra = resolucion.elegir(None);
    match letra {
        Letra::Ninguna { rutas } => {
            assert_eq!(rutas.len(), 4);
            assert!(rutas[0].ends_with("pista.lrc"));
            assert!(rutas[1].ends_with("pista.txt"));
        }
        otra => panic!("se esperaba Letra::Ninguna, no {otra:?}"),
    }
}
