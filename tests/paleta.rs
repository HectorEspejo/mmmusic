use image::{DynamicImage, Rgb, RgbImage};
use mmmusic::visuales::paleta::{Paleta, a_rgb, colores_dominantes, contraste, interpolar_color};
use ratatui::style::Color;

fn imagen_cuatro_cuadrantes() -> DynamicImage {
    let mut imagen = RgbImage::new(128, 128);
    let colores = [
        Rgb([220, 30, 40]),
        Rgb([40, 200, 80]),
        Rgb([230, 60, 200]),
        Rgb([40, 200, 220]),
    ];
    for (x, y, pixel) in imagen.enumerate_pixels_mut() {
        let cuadrante = usize::from(x >= 64) + usize::from(y >= 64) * 2;
        *pixel = colores[cuadrante];
    }
    DynamicImage::ImageRgb8(imagen)
}

fn cercano(obtenido: Color, esperado: [u8; 3]) -> bool {
    let (r, g, b) = a_rgb(obtenido);
    let distancia = |a: u8, b: u8| (i32::from(a) - i32::from(b)).abs();
    distancia(r, esperado[0]) <= 24
        && distancia(g, esperado[1]) <= 24
        && distancia(b, esperado[2]) <= 24
}

#[test]
fn median_cut_encuentra_los_cuatro_colores_y_es_determinista() {
    let imagen = imagen_cuatro_cuadrantes();
    let primera = colores_dominantes(&imagen).expect("colores");
    let segunda = colores_dominantes(&imagen).expect("colores");
    assert_eq!(
        primera, segunda,
        "el resultado debe ser idéntico en dos ejecuciones"
    );

    let esperados = [[220, 30, 40], [40, 200, 80], [230, 60, 200], [40, 200, 220]];
    for esperado in esperados {
        assert!(
            primera.iter().any(|color| cercano(*color, esperado)),
            "falta el color {esperado:?} en {primera:?}"
        );
    }
}

#[test]
fn paleta_desde_texto_parsea_y_garantiza_contraste() {
    let texto = "#101010,#303030,#3a3a3a,#505050,#707070";
    let fondo = Color::Rgb(12, 12, 12);
    let paleta = Paleta::desde_texto(texto, fondo).expect("paleta");
    assert_eq!(paleta.primario, Color::Rgb(0x10, 0x10, 0x10));
    assert!(contraste(paleta.acento, fondo) >= 2.0);
    assert!(Paleta::desde_texto("no-es-hex", fondo).is_none());
    assert!(Paleta::desde_texto("#101010,#202020", fondo).is_none());
}

#[test]
fn interpolar_color_mezcla_y_conserva_extremos() {
    let a = Color::Rgb(0, 0, 0);
    let b = Color::Rgb(255, 255, 255);
    assert_eq!(interpolar_color(a, b, 0.0), a);
    assert_eq!(interpolar_color(a, b, 1.0), b);
    assert_eq!(interpolar_color(a, b, 0.5), Color::Rgb(128, 128, 128));
}
