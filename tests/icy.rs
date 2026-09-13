use mmmusic::radio::icy::{limpiar, parsear};

#[test]
fn limpia_espacios_repetidos() {
    assert_eq!(limpiar("  Boards   of  Canada "), "Boards of Canada");
    assert_eq!(limpiar("\tTycho\n"), "Tycho");
}

#[test]
fn descarta_vacios_ruido_y_nombre_de_emisora() {
    assert!(parsear("", "Radio").is_none());
    assert!(parsear("   ", "Radio").is_none());
    assert!(parsear("Advertisement", "Radio").is_none());
    assert!(parsear("Jingle de la casa", "Radio").is_none());
    assert!(parsear("Station ID 24/7", "Radio").is_none());
    assert!(parsear("Visita https://ejemplo.org", "Radio").is_none());
    assert!(parsear("Radio Paradise", "Radio Paradise").is_none());
    assert!(parsear("RÀDIO PARADISE", "Ràdio Paradise").is_none());
}

#[test]
fn separa_por_el_primer_guion() {
    let parsed = parsear("Boards of Canada - Dayvan Cowboy", "Radio").expect("título");
    assert_eq!(parsed.artista.as_deref(), Some("Boards of Canada"));
    assert_eq!(parsed.titulo, "Dayvan Cowboy");
    assert!(parsed.tiene_artista());

    let parsed = parsear("Tycho - Awake - Remix", "Radio").expect("título");
    assert_eq!(parsed.artista.as_deref(), Some("Tycho"));
    assert_eq!(parsed.titulo, "Awake - Remix");
}

#[test]
fn conserva_titulos_sin_artista() {
    let parsed = parsear("Solo una canción", "Radio").expect("título");
    assert_eq!(parsed.artista, None);
    assert_eq!(parsed.titulo, "Solo una canción");
    assert!(!parsed.tiene_artista());
}
