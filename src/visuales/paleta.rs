use ratatui::style::Color;

use crate::tema;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paleta {
    pub fondo: Color,
    pub primario: Color,
    pub secundario: Color,
    pub acento: Color,
    pub resalte: Color,
    pub tenue: Color,
}

impl Paleta {
    pub fn desde_tema(paleta: &tema::Paleta) -> Self {
        Self {
            fondo: paleta.fondo,
            primario: paleta.acento,
            secundario: paleta.progreso,
            acento: paleta.texto,
            resalte: paleta.aviso,
            tenue: paleta.secundario,
        }
    }

    pub fn gradiente(&self, t: f32) -> Color {
        interpolar_color(self.primario, self.acento, t)
    }

    pub fn interpolar(a: &Paleta, b: &Paleta, t: f32) -> Paleta {
        Paleta {
            fondo: interpolar_color(a.fondo, b.fondo, t),
            primario: interpolar_color(a.primario, b.primario, t),
            secundario: interpolar_color(a.secundario, b.secundario, t),
            acento: interpolar_color(a.acento, b.acento, t),
            resalte: interpolar_color(a.resalte, b.resalte, t),
            tenue: interpolar_color(a.tenue, b.tenue, t),
        }
    }

    /// Paleta a partir de los cinco colores dominantes de la carátula,
    /// garantizando contraste del acento sobre el fondo.
    pub fn desde_colores(colores: &[Color; 5], fondo: Color) -> Paleta {
        Paleta {
            fondo,
            primario: colores[0],
            secundario: colores[1],
            acento: asegurar_contraste(colores[2], fondo, 2.0),
            resalte: colores[3],
            tenue: colores[4],
        }
    }

    /// Lee `"#rrggbb,#rrggbb,…"` (cinco colores) y construye la paleta.
    pub fn desde_texto(texto: &str, fondo: Color) -> Option<Paleta> {
        let mut colores = Vec::with_capacity(5);
        for bruto in texto.split(',') {
            let hex = bruto.trim().trim_start_matches('#');
            if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            let componente = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
            colores.push(Color::Rgb(componente(0)?, componente(2)?, componente(4)?));
            if colores.len() == 5 {
                break;
            }
        }
        if colores.len() < 5 {
            return None;
        }
        Some(Paleta::desde_colores(
            &[colores[0], colores[1], colores[2], colores[3], colores[4]],
            fondo,
        ))
    }
}

pub fn interpolar_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    if a == b {
        return a;
    }
    if a == Color::Reset || b == Color::Reset {
        return if t < 0.5 { a } else { b };
    }
    let (r1, g1, b1) = a_rgb(a);
    let (r2, g2, b2) = a_rgb(b);
    let mezcla = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color::Rgb(mezcla(r1, r2), mezcla(g1, g2), mezcla(b1, b2))
}

pub fn a_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (0xcc, 0x24, 0x1d),
        Color::Green => (0x98, 0x97, 0x1a),
        Color::Yellow => (0xd7, 0x99, 0x21),
        Color::Blue => (0x45, 0x85, 0x88),
        Color::Magenta => (0xb1, 0x62, 0x86),
        Color::Cyan => (0x68, 0x9d, 0x6a),
        Color::Gray => (0xa8, 0x99, 0x84),
        Color::DarkGray => (0x92, 0x83, 0x74),
        Color::LightRed => (0xfb, 0x49, 0x34),
        Color::LightGreen => (0xb8, 0xbb, 0x26),
        Color::LightYellow => (0xfa, 0xbd, 0x2f),
        Color::LightBlue => (0x83, 0xa5, 0x98),
        Color::LightMagenta => (0xd3, 0x86, 0x9b),
        Color::LightCyan => (0x8e, 0xc0, 0x7c),
        Color::White => (0xeb, 0xdb, 0xb2),
        Color::Reset => (0, 0, 0),
        Color::Indexed(_) => (0x80, 0x80, 0x80),
    }
}

pub fn contraste(color: Color, fondo: Color) -> f32 {
    let (l1, l2) = (luminancia(color), luminancia(fondo));
    let (claro, oscuro) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (claro + 0.05) / (oscuro + 0.05)
}

pub fn a_hex(color: Color) -> String {
    let (r, g, b) = a_rgb(color);
    format!("#{r:02x}{g:02x}{b:02x}")
}

const LADO_CUANTIZACION: u32 = 64;
const CAJAS_MEDIAN_CUT: usize = 8;
const COLORES: usize = 5;
const LUMINANCIA_CASI_NEGRO: u8 = 20;
const LUMINANCIA_CASI_BLANCO: u8 = 242;

struct Caja {
    pixeles: Vec<[u8; 3]>,
}

impl Caja {
    fn color(&self) -> [u8; 3] {
        if self.pixeles.is_empty() {
            return [0, 0, 0];
        }
        let suma = self.pixeles.iter().fold([0u32; 3], |acumulado, pixel| {
            [
                acumulado[0] + u32::from(pixel[0]),
                acumulado[1] + u32::from(pixel[1]),
                acumulado[2] + u32::from(pixel[2]),
            ]
        });
        let n = self.pixeles.len() as u32;
        [
            (suma[0] / n) as u8,
            (suma[1] / n) as u8,
            (suma[2] / n) as u8,
        ]
    }

    fn rango(&self) -> u8 {
        let mut minimo = [255u8; 3];
        let mut maximo = [0u8; 3];
        for pixel in &self.pixeles {
            for canal in 0..3 {
                minimo[canal] = minimo[canal].min(pixel[canal]);
                maximo[canal] = maximo[canal].max(pixel[canal]);
            }
        }
        (0..3)
            .map(|canal| maximo[canal] - minimo[canal])
            .max()
            .unwrap_or(0)
    }

    fn canal_dominante(&self) -> usize {
        let mut minimo = [255u8; 3];
        let mut maximo = [0u8; 3];
        for pixel in &self.pixeles {
            for canal in 0..3 {
                minimo[canal] = minimo[canal].min(pixel[canal]);
                maximo[canal] = maximo[canal].max(pixel[canal]);
            }
        }
        (0..3)
            .max_by_key(|canal| maximo[*canal] - minimo[*canal])
            .unwrap_or(0)
    }
}

/// Cinco colores dominantes de una carátula por median cut sobre 64×64,
/// descartando casi negros y casi blancos si queda al menos un 20 % de píxeles.
/// Determinista para el mismo fichero.
pub fn colores_dominantes(imagen: &image::DynamicImage) -> Option<[Color; 5]> {
    let reducida = imagen.thumbnail_exact(LADO_CUANTIZACION, LADO_CUANTIZACION);
    let rgb = reducida.to_rgb8();
    let pixeles: Vec<[u8; 3]> = rgb.pixels().map(|pixel| pixel.0).collect();
    if pixeles.is_empty() {
        return None;
    }
    let filtrados: Vec<[u8; 3]> = pixeles
        .iter()
        .copied()
        .filter(|pixel| {
            let l = luminancia_u8(*pixel);
            !(LUMINANCIA_CASI_NEGRO..=LUMINANCIA_CASI_BLANCO).contains(&l)
        })
        .collect();
    let seleccion = if filtrados.len() * 5 >= pixeles.len() {
        filtrados
    } else {
        pixeles
    };

    let cajas = median_cut(seleccion, CAJAS_MEDIAN_CUT);
    let mut cajas: Vec<Caja> = cajas
        .into_iter()
        .filter(|caja| !caja.pixeles.is_empty())
        .collect();
    cajas.sort_by(|a, b| {
        b.pixeles
            .len()
            .cmp(&a.pixeles.len())
            .then_with(|| a.color().cmp(&b.color()))
    });
    let mut colores: Vec<[u8; 3]> = cajas.iter().map(Caja::color).collect();
    colores.truncate(COLORES);
    if colores.is_empty() {
        return None;
    }
    while colores.len() < COLORES {
        let ultimo = colores[colores.len() - 1];
        colores.push(ultimo);
    }
    colores.sort_by_key(|color| luminancia_u8(*color));
    let colores: [Color; 5] = [
        Color::Rgb(colores[0][0], colores[0][1], colores[0][2]),
        Color::Rgb(colores[1][0], colores[1][1], colores[1][2]),
        Color::Rgb(colores[2][0], colores[2][1], colores[2][2]),
        Color::Rgb(colores[3][0], colores[3][1], colores[3][2]),
        Color::Rgb(colores[4][0], colores[4][1], colores[4][2]),
    ];
    Some(colores)
}

fn median_cut(pixeles: Vec<[u8; 3]>, objetivo: usize) -> Vec<Caja> {
    let mut cajas = vec![Caja { pixeles }];
    while cajas.len() < objetivo {
        let elegida = cajas
            .iter()
            .enumerate()
            .filter(|(_, caja)| caja.pixeles.len() > 1 && caja.rango() > 0)
            .max_by_key(|(_, caja)| (caja.pixeles.len(), caja.rango()))
            .map(|(indice, _)| indice);
        let Some(indice) = elegida else {
            break;
        };
        let mut caja = cajas.swap_remove(indice);
        let canal = caja.canal_dominante();
        caja.pixeles.sort_by_key(|pixel| pixel[canal]);
        let mitad = caja.pixeles.len() / 2;
        let derecha = caja.pixeles.split_off(mitad);
        cajas.push(caja);
        cajas.push(Caja { pixeles: derecha });
    }
    cajas
}

fn luminancia_u8(pixel: [u8; 3]) -> u8 {
    (0.2126 * f32::from(pixel[0]) + 0.7152 * f32::from(pixel[1]) + 0.0722 * f32::from(pixel[2]))
        .round()
        .clamp(0.0, 255.0) as u8
}

fn luminancia(color: Color) -> f32 {
    let (r, g, b) = a_rgb(color);
    let f = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
}

/// Aclara o oscurece `color` hasta que alcance `minimo` de contraste con el
/// fondo. Devuelve el color original si ya cumple.
pub fn asegurar_contraste(color: Color, fondo: Color, minimo: f32) -> Color {
    if contraste(color, fondo) >= minimo {
        return color;
    }
    let (r, g, b) = a_rgb(color);
    let fondo_es_claro = luminancia(fondo) > 0.5;
    for paso in 1..=20 {
        let factor = paso as f32 / 20.0;
        let candidato = if fondo_es_claro {
            Color::Rgb(
                (f32::from(r) * (1.0 - factor)).round() as u8,
                (f32::from(g) * (1.0 - factor)).round() as u8,
                (f32::from(b) * (1.0 - factor)).round() as u8,
            )
        } else {
            Color::Rgb(
                (f32::from(r) + (255.0 - f32::from(r)) * factor).round() as u8,
                (f32::from(g) + (255.0 - f32::from(g)) * factor).round() as u8,
                (f32::from(b) + (255.0 - f32::from(b)) * factor).round() as u8,
            )
        };
        if contraste(candidato, fondo) >= minimo {
            return candidato;
        }
    }
    if fondo_es_claro {
        Color::Black
    } else {
        Color::White
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn interpola_y_respeta_extremos() {
        let a = Color::Rgb(0, 0, 0);
        let b = Color::Rgb(100, 200, 50);
        assert_eq!(interpolar_color(a, b, 0.0), a);
        assert_eq!(interpolar_color(a, b, 1.0), b);
        assert_eq!(interpolar_color(a, b, 0.5), Color::Rgb(50, 100, 25));
    }

    #[test]
    fn garantiza_contraste_minimo() {
        let fondo = Color::Rgb(20, 20, 20);
        let gris = Color::Rgb(45, 45, 45);
        assert!(contraste(gris, fondo) < 2.0);
        let ajustado = asegurar_contraste(gris, fondo, 2.0);
        assert!(contraste(ajustado, fondo) >= 2.0);
    }
}
