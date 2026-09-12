use mmmusic::app::{AppEstado, Vista};
use mmmusic::audio::Analisis;
use mmmusic::config::{Config, ConfigTema};
use mmmusic::tema;
use mmmusic::ui;
use mmmusic::visuales::{self, mini_espectro, paleta::Paleta};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;
use ratatui::widgets::canvas::Canvas;

const TAMANOS: [(u16, u16); 3] = [(40, 12), (80, 24), (200, 50)];

fn paleta() -> Paleta {
    Paleta::desde_tema(&tema::terminal(&ConfigTema::default()))
}

fn analisis_real() -> Analisis {
    let bandas = (0..64)
        .map(|i| {
            let x = i as f32 / 64.0;
            (0.15 + 0.8 * (x * 3.1).sin().abs()) * (1.0 - x * 0.5)
        })
        .collect();
    Analisis {
        bandas,
        onda: (0..160).map(|i| ((i as f32) * 0.13).sin() * 0.7).collect(),
        rms: [0.42, 0.38],
        pico: [0.8, 0.75],
        graves: 0.6,
        pulso: true,
        tasa_hz: 48_000,
        ambiental: false,
    }
}

fn dibujar(visual: &visuales::VisualCompartida, analisis: &Analisis, ancho: u16, alto: u16) {
    let area = Rect::new(0, 0, ancho, alto);
    let mut buffer = Buffer::empty(area);
    let canvas = Canvas::default()
        .x_bounds([0.0, f64::from(ancho)])
        .y_bounds([0.0, f64::from(alto)])
        .background_color(Color::Reset)
        .paint(|ctx| {
            visual
                .borrow_mut()
                .dibujar(analisis, area, &paleta(), 0.033, ctx);
        });
    canvas.render(area, &mut buffer);
}

#[test]
fn cada_visual_dibuja_en_tres_tamanos_y_tres_estados() {
    let analisis = [
        analisis_real(),
        Analisis::ambiental(1200, 64, 160),
        Analisis::vacio(64, 160),
    ];
    for ascii in [false, true] {
        let visuales = visuales::registro(ascii);
        assert_eq!(visuales.len(), 6);
        for (indice, visual) in visuales.iter().enumerate() {
            for (ancho, alto) in TAMANOS {
                for estado in &analisis {
                    dibujar(visual, estado, ancho, alto);
                }
            }
            visual.borrow_mut().reiniciar();
            assert!(!visual.borrow().nombre().is_empty());
            assert!(
                visuales::indice_por_nombre(visual.borrow().nombre()) == Some(indice),
                "el nombre {} no está registrado en su posición",
                visual.borrow().nombre()
            );
        }
    }
}

#[test]
fn mini_espectro_dibuja_doce_barras() {
    let analisis = analisis_real();
    let linea = mini_espectro::linea(&analisis, &paleta());
    assert_eq!(linea.spans.len(), mini_espectro::BARRAS);

    let vacio = Analisis::vacio(64, 160);
    let linea_vacia = mini_espectro::linea(&vacio, &paleta());
    assert_eq!(linea_vacia.spans.len(), mini_espectro::BARRAS);
}

#[test]
fn la_vista_visual_se_dibuja_y_vuelve_a_la_vista_anterior() {
    let mut app = AppEstado::nuevo(
        Config::default(),
        tema::terminal(&ConfigTema::default()),
        0,
        std::path::PathBuf::from("/tmp/tema"),
    );
    app.tamano_terminal = (80, 24);
    app.analisis = analisis_real();

    app.entrar_visual(false);
    assert!(app.modo_visual.is_some());
    assert!(app.necesita_analisis());

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal de pruebas");
    terminal
        .draw(|frame| ui::dibujar(frame, &mut app))
        .expect("dibujar modo visual");
    let texto: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|celda| celda.symbol())
        .collect();
    assert!(
        texto.contains("Visual 1/6"),
        "la cabecera debe mostrar la visual activa"
    );

    let ayuda: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|celda| celda.symbol())
        .collect();
    assert!(ayuda.contains("sensibilidad"));

    app.salir_visual();
    assert!(app.modo_visual.is_none());
    assert_eq!(app.vista, Vista::Inicio);
    terminal
        .draw(|frame| ui::dibujar(frame, &mut app))
        .expect("dibujar modo normal");
}
