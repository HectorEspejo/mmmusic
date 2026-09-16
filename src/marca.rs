//! Marca de mmmusic: logotipos, eslogan y onda.
//!
//! Todo el arte textual de la aplicación vive aquí, con una variante `ascii`
//! para terminales sin glifos Unicode. Ninguna de estas cadenas transmite
//! información funcional: si no caben se recortan sin afectar a los controles.

use unicode_width::UnicodeWidthStr;

/// Nombre de la marca tal y como se escribe en la tarjeta de reposo.
const NOMBRE: &str = "m m m u s i c";

/// Logo compacto de dos filas y 31 columnas (ayuda completa).
pub const LOGO_COMPACTO: [&str; 2] = [
    "█▀▄▀█ █▀▄▀█ █▀▄▀█ █ █ █▀▀ █ █▀▀",
    "█ ▀ █ █ ▀ █ █ ▀ █ █▄█ ▄▄█ █ █▄▄",
];

/// Variante ascii del logo compacto.
pub const LOGO_COMPACTO_ASCII: [&str; 2] = [
    "#=#=# #=#=# #=#=# # # #== # #==",
    "# = # # = # # = # #=# ==# # #==",
];

/// Solo las tres emes (17 columnas) para la cabecera de la sidebar.
pub const LOGO_MMM: [&str; 2] = ["█▀▄▀█ █▀▄▀█ █▀▄▀█", "█ ▀ █ █ ▀ █ █ ▀ █"];

/// Variante ascii de las tres emes.
pub const LOGO_MMM_ASCII: [&str; 2] = ["#=#=# #=#=# #=#=#", "# = # # = # # = #"];

/// Logo grande de seis filas y 61 columnas (README y `--version --logo`).
///
/// Generado con la fuente `ansi_shadow` de figlet. Las filas 3 y 4 terminan
/// en espacios que forman parte del arte.
pub const LOGO_GRANDE: [&str; 6] = [
    "███╗   ███╗███╗   ███╗███╗   ███╗██╗   ██╗███████╗██╗ ██████╗",
    "████╗ ████║████╗ ████║████╗ ████║██║   ██║██╔════╝██║██╔════╝",
    "██╔████╔██║██╔████╔██║██╔████╔██║██║   ██║███████╗██║██║     ",
    "██║╚██╔╝██║██║╚██╔╝██║██║╚██╔╝██║██║   ██║╚════██║██║██║     ",
    "██║ ╚═╝ ██║██║ ╚═╝ ██║██║ ╚═╝ ██║╚██████╔╝███████║██║╚██████╗",
    "╚═╝     ╚═╝╚═╝     ╚═╝╚═╝     ╚═╝ ╚═════╝ ╚══════╝╚═╝ ╚═════╝",
];

/// Eslogan de la marca.
pub const ESLOGAN: &str = "reproductor para tu tty";

/// Onda de la tarjeta de reposo.
pub const ONDA: &str = "▁▂▃▅▆▇█▇▆▅▃▂";

/// Variante ascii de la onda.
pub const ONDA_ASCII: &str = "._-~^~-_.";

/// Versión del paquete, tomada de `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Fila 1 de la tarjeta de reposo: marca, versión y eslogan.
///
/// Recorta por etapas (sin eslogan, luego solo el nombre) y centra el texto
/// en `ancho` columnas.
pub fn linea_reposo(ancho: u16, ascii: bool) -> String {
    let completa = format!("{NOMBRE} · v{} · {} {ESLOGAN}", version(), play(ascii));
    let sin_eslogan = nombre_version();
    let texto = if cabe(&completa, ancho) {
        completa
    } else if cabe(&sin_eslogan, ancho) {
        sin_eslogan
    } else {
        "mmmusic".to_string()
    };
    centrar(&texto, ancho)
}

/// Fila 1 de la tarjeta de reposo en modo compacto, sin eslogan.
pub fn linea_reposo_sin_eslogan(ancho: u16, _ascii: bool) -> String {
    let sin_eslogan = nombre_version();
    let texto = if cabe(&sin_eslogan, ancho) {
        sin_eslogan
    } else {
        "mmmusic".to_string()
    };
    centrar(&texto, ancho)
}

/// Fila 2 de la tarjeta de reposo: el patrón de onda repetido hasta `ancho`
/// columnas y desplazado `desplazamiento` caracteres.
pub fn onda(ancho: u16, desplazamiento: usize, ascii: bool) -> String {
    let patron = if ascii { ONDA_ASCII } else { ONDA };
    let inicio = desplazamiento % patron.chars().count();
    patron
        .chars()
        .cycle()
        .skip(inicio)
        .take(ancho as usize)
        .collect()
}

fn play(ascii: bool) -> &'static str {
    if ascii { ">" } else { "▶" }
}

fn nombre_version() -> String {
    format!("{NOMBRE} · v{}", version())
}

fn cabe(texto: &str, ancho: u16) -> bool {
    texto.width() <= ancho as usize
}

fn centrar(texto: &str, ancho: u16) -> String {
    format!("{texto:^ancho$}", ancho = ancho as usize)
}
