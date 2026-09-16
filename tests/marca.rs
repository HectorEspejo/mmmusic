use std::path::PathBuf;

use mmmusic::app::AppEstado;
use mmmusic::biblioteca::modelos::{ElementoCola, PistaResumen};
use mmmusic::config::{Config, ConfigTema};
use mmmusic::marca;
use mmmusic::{tema, ui};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use unicode_width::UnicodeWidthStr;

#[test]
fn las_filas_de_cada_logo_miden_lo_mismo() {
    let casos: [(&str, &[&str], usize); 5] = [
        ("LOGO_COMPACTO", &marca::LOGO_COMPACTO, 31),
        ("LOGO_COMPACTO_ASCII", &marca::LOGO_COMPACTO_ASCII, 31),
        ("LOGO_MMM", &marca::LOGO_MMM, 17),
        ("LOGO_MMM_ASCII", &marca::LOGO_MMM_ASCII, 17),
        ("LOGO_GRANDE", &marca::LOGO_GRANDE, 61),
    ];
    for (nombre, filas, esperada) in casos {
        for fila in filas {
            assert_eq!(
                fila.width(),
                esperada,
                "{nombre}: la fila {fila:?} no mide {esperada} columnas"
            );
        }
    }
    assert_eq!(marca::ONDA.width(), 12);
}

#[test]
fn las_variantes_ascii_miden_como_su_original() {
    assert_eq!(
        marca::LOGO_COMPACTO_ASCII[0].width(),
        marca::LOGO_COMPACTO[0].width()
    );
    assert_eq!(marca::LOGO_MMM_ASCII[0].width(), marca::LOGO_MMM[0].width());
    assert_eq!(marca::ONDA_ASCII.width(), 9);
}

#[test]
fn linea_reposo_recorta_por_etapas() {
    let base = format!("m m m u s i c · v{}", marca::version());

    let ancho_30 = marca::linea_reposo(30, false);
    assert_eq!(ancho_30.width(), 30);
    assert_eq!(ancho_30.trim(), base);
    assert!(!ancho_30.contains(marca::ESLOGAN));

    assert_eq!(marca::linea_reposo(6, false).trim(), "mmmusic");
}

#[test]
fn linea_reposo_completa_muestra_eslogan() {
    let nerd = marca::linea_reposo(80, false);
    assert_eq!(nerd.width(), 80);
    assert!(nerd.contains(marca::ESLOGAN));
    assert!(nerd.contains('▶'));

    let ascii = marca::linea_reposo(80, true);
    assert!(ascii.contains('>'));
    assert!(!ascii.contains('▶'));
}

#[test]
fn linea_reposo_sin_eslogan_omite_siempre_el_eslogan() {
    let linea = marca::linea_reposo_sin_eslogan(40, false);
    assert_eq!(
        linea.trim(),
        format!("m m m u s i c · v{}", marca::version())
    );
    assert!(!linea.contains(marca::ESLOGAN));
    assert_eq!(marca::linea_reposo_sin_eslogan(4, false).trim(), "mmmusic");
}

#[test]
fn onda_repite_el_patron_y_lo_desplaza() {
    let ancho = 24;
    let base = marca::onda(ancho, 0, false);
    assert_eq!(base.width(), ancho as usize);
    assert!(base.starts_with(marca::ONDA));

    let vuelta = marca::ONDA.chars().count();
    assert_eq!(marca::onda(ancho, vuelta, false), base);
    assert_eq!(marca::onda(ancho, 1, false).chars().next(), Some('▂'));

    assert_eq!(
        marca::onda(16, 0, true),
        marca::ONDA_ASCII.repeat(2)[..16].to_string()
    );
    assert!(marca::onda(0, 3, false).is_empty());
}

fn app_de_pruebas() -> AppEstado {
    AppEstado::nuevo(
        Config::default(),
        tema::terminal(&ConfigTema::default()),
        0,
        PathBuf::from("/tmp/tema"),
    )
}

fn dibujar(app: &mut AppEstado, ancho: u16, alto: u16) -> String {
    app.tamano_terminal = (ancho, alto);
    let backend = TestBackend::new(ancho, alto);
    let mut terminal = Terminal::new(backend).expect("terminal de pruebas");
    terminal
        .draw(|frame| ui::dibujar(frame, app))
        .expect("dibujar");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|celda| celda.symbol())
        .collect()
}

#[test]
fn la_barra_en_reposo_muestra_la_marca_y_no_el_estado_detenido() {
    let mut app = app_de_pruebas();
    let texto = dibujar(&mut app, 80, 24);
    assert!(texto.contains(marca::ESLOGAN));
    assert!(texto.contains(&format!("v{}", marca::version())));
    assert!(texto.contains('▁'));
    assert!(!texto.contains("(detenido)"));
}

#[test]
fn con_elemento_la_barra_vuelve_al_formato_normal() {
    let mut app = app_de_pruebas();
    app.estado_reproductor.elemento = Some(ElementoCola::Pista(PistaResumen {
        id: 1,
        titulo: "Nocturne Drive".to_string(),
        artista: "Midnight".to_string(),
        album: "Tapes".to_string(),
        album_id: 7,
        duracion_ms: 252_000,
        caratula_ruta: None,
        ruta: "/tmp/pista.flac".to_string(),
    }));
    let texto = dibujar(&mut app, 80, 24);
    assert!(texto.contains("Nocturne Drive"));
    assert!(!texto.contains(marca::ESLOGAN));
}

#[test]
fn la_sidebar_muestra_las_tres_emes_sin_el_titulo_del_borde() {
    let mut app = app_de_pruebas();
    // Una config antigua con 18 se ensancha al mínimo de la UI (20).
    app.config.interfaz.ancho_sidebar = 18;
    let texto = dibujar(&mut app, 80, 24);
    assert!(texto.contains(marca::LOGO_MMM[0]));
    assert!(texto.contains(marca::LOGO_MMM[1]));
    assert!(!texto.contains(" mmmusic "));
}
