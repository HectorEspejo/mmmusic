use crate::biblioteca::etiquetas;

const RUIDO: [&str; 7] = [
    "advert",
    "advertisement",
    "jingle",
    "station id",
    "commercial",
    "http://",
    "https://",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TituloIcy {
    /// Canción sin artista cuando el ICY trae "Artista - Título".
    pub titulo: String,
    pub artista: Option<String>,
}

impl TituloIcy {
    pub fn tiene_artista(&self) -> bool {
        self.artista.is_some()
    }
}

/// Limpia espacios repetidos y recorta el título.
pub fn limpiar(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parsea un `icy-title`. Devuelve `None` para vacíos, ruido, URLs y títulos
/// que solo repiten el nombre de la emisora. Separa "Artista - Título" por el
/// primer " - "; si no hay artista válido, el título se conserva sin artista.
pub fn parsear(titulo: &str, nombre_emisora: &str) -> Option<TituloIcy> {
    let limpio = limpiar(titulo);
    if limpio.is_empty() {
        return None;
    }
    let minusculas = limpio.to_lowercase();
    if RUIDO.iter().any(|patron| minusculas.contains(patron)) {
        return None;
    }
    let emisora_norm = etiquetas::normalizar(nombre_emisora);
    if !emisora_norm.is_empty() && etiquetas::normalizar(&limpio) == emisora_norm {
        return None;
    }
    if let Some((artista, cancion)) = limpio.split_once(" - ") {
        let artista = artista.trim();
        let cancion = cancion.trim();
        if !artista.is_empty() && !cancion.is_empty() {
            return Some(TituloIcy {
                titulo: cancion.to_string(),
                artista: Some(artista.to_string()),
            });
        }
    }
    Some(TituloIcy {
        titulo: limpio,
        artista: None,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn limpia_espacios_y_descarta_vacios() {
        assert_eq!(limpiar("  Boards   of  Canada  "), "Boards of Canada");
        assert_eq!(parsear("   ", "Radio"), None);
        assert_eq!(parsear("", "Radio"), None);
    }

    #[test]
    fn descarta_ruido_y_nombre_de_la_emisora() {
        assert_eq!(parsear("Advertisement break", "Radio"), None);
        assert_eq!(parsear("Station ID", "Radio"), None);
        assert_eq!(parsear("Listen at https://radio.example", "Radio"), None);
        assert_eq!(parsear("NIGHTWAVE PLAZA", "Nightwave Plaza"), None);
    }

    #[test]
    fn separa_artista_y_titulo_por_el_primer_guion() {
        let titulo = parsear("Boards of Canada - Dayvan Cowboy", "Radio").expect("título");
        assert_eq!(titulo.artista.as_deref(), Some("Boards of Canada"));
        assert_eq!(titulo.titulo, "Dayvan Cowboy");

        let titulo = parsear("Tycho - Awake - Remix", "Radio").expect("título");
        assert_eq!(titulo.artista.as_deref(), Some("Tycho"));
        assert_eq!(titulo.titulo, "Awake - Remix");
    }

    #[test]
    fn titulos_sin_artista_se_conservan() {
        let titulo = parsear("Solo una canción", "Radio").expect("título");
        assert_eq!(titulo.artista, None);
        assert_eq!(titulo.titulo, "Solo una canción");

        let titulo = parsear(" - Solo título", "Radio").expect("título");
        assert_eq!(titulo.artista, None);
        assert_eq!(titulo.titulo, "- Solo título");
    }
}
