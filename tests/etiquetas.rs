use std::path::Path;

use mmmusic::biblioteca::etiquetas;

#[test]
fn lee_titulos_y_duracion_de_los_seis_formatos() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let esperados = [
        ("prueba.mp3", "Prueba MP3"),
        ("prueba.flac", "Prueba FLAC"),
        ("prueba.ogg", "Prueba OGG"),
        ("prueba.opus", "Prueba OPUS"),
        ("prueba.m4a", "Prueba M4A"),
        ("prueba.wav", "Prueba WAV"),
    ];
    for (archivo, titulo) in esperados {
        let crudas = etiquetas::leer(&fixtures.join(archivo))
            .unwrap_or_else(|error| panic!("{archivo} ilegible: {error:#}"));
        assert_eq!(
            crudas.titulo.as_deref(),
            Some(titulo),
            "título de {archivo}"
        );
        assert_eq!(
            crudas.artista.as_deref(),
            Some("Artista Prueba"),
            "artista de {archivo}"
        );
        assert!(crudas.duracion_ms.unwrap_or(0) > 0, "duración de {archivo}");
    }
}

#[test]
fn resuelve_album_desde_etiqueta_o_carpeta() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let crudas = etiquetas::leer(&fixtures.join("prueba.flac")).expect("flac");
    let resueltas = etiquetas::resolver(crudas, &fixtures.join("prueba.flac"));
    assert_eq!(resueltas.album, "Álbum Ñandú");
    assert_eq!(resueltas.album_artista, "Artista Prueba");
    assert_eq!(resueltas.anio, Some(2024));
    assert_eq!(resueltas.pista, Some(2));
}

#[test]
fn fichero_corrupto_no_se_puede_leer() {
    let temporal = tempfile::tempdir().expect("tempdir");
    let ruta = temporal.path().join("corrupto.mp3");
    std::fs::write(&ruta, b"esto no es un mp3").expect("escribir");
    assert!(etiquetas::leer(&ruta).is_err());
}
