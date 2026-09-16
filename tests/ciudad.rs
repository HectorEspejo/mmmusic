//! Pruebas de la visual «Ciudad»: geometría, raster, ocultación entre edificios
//! y render en los tres tamaños con análisis real, ambiental y vacío.

use mmmusic::audio::Analisis;
use mmmusic::config::ConfigTema;
use mmmusic::tema;
use mmmusic::visuales::ciudad::{AjustesCiudad, Prisma, a_puntos, dibujar_prisma};
use mmmusic::visuales::paleta::Paleta;
use mmmusic::visuales::proyeccion::{
    Camara, Dimensiones, Punto2, Punto3, cara_visible, caras_de_prisma, proyectar,
};
use mmmusic::visuales::raster::Raster;
use mmmusic::visuales::{self, Visual, VisualCompartida};
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

fn dibujar(visual: &VisualCompartida, analisis: &Analisis, ancho: u16, alto: u16) {
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

/// Cuenta los puntos encendidos del raster (por si el modo cambia).
fn puntos(raster: &Raster) -> usize {
    raster.total_puntos()
}

#[test]
fn ciudad_es_la_septima_visual() {
    assert_eq!(visuales::indice_por_nombre("Ciudad"), Some(6));
    assert_eq!(visuales::indice_por_nombre("ciudad"), Some(6));
    let lista = visuales::registro(false, &AjustesCiudad::default());
    assert_eq!(lista.len(), 7);
    assert_eq!(lista[6].borrow().nombre(), "Ciudad");
    assert_eq!(visuales::NOMBRES[6], "Ciudad");
}

#[test]
fn una_cara_que_mira_a_la_camara_es_visible_y_su_opuesta_no() {
    let vertices = [
        Punto3::nuevo(-1.0, 0.0, -1.0),
        Punto3::nuevo(1.0, 0.0, -1.0),
        Punto3::nuevo(1.0, 0.0, 1.0),
        Punto3::nuevo(-1.0, 0.0, 1.0),
        Punto3::nuevo(-1.0, 6.0, -1.0),
        Punto3::nuevo(1.0, 6.0, -1.0),
        Punto3::nuevo(1.0, 6.0, 1.0),
        Punto3::nuevo(-1.0, 6.0, 1.0),
    ];
    let caras = caras_de_prisma(&vertices);
    // Caras opuestas: −z (2) y +z (4); −x (5) y +x (3).
    let desde_delante =
        Camara::mirando_a(Punto3::nuevo(0.0, 3.0, 20.0), Punto3::nuevo(0.0, 3.0, 0.0));
    assert!(cara_visible(&desde_delante, &caras[4]));
    assert!(!cara_visible(&desde_delante, &caras[2]));
    assert!(!cara_visible(&desde_delante, &caras[0]), "la base no se ve");

    let desde_un_lado =
        Camara::mirando_a(Punto3::nuevo(20.0, 3.0, 0.0), Punto3::nuevo(0.0, 3.0, 0.0));
    assert!(cara_visible(&desde_un_lado, &caras[3]));
    assert!(!cara_visible(&desde_un_lado, &caras[5]));

    let desde_arriba = Camara::mirando_a(
        Punto3::nuevo(20.0, 40.0, 20.0),
        Punto3::nuevo(0.0, 3.0, 0.0),
    );
    assert!(
        cara_visible(&desde_arriba, &caras[1]),
        "la tapa se ve desde arriba"
    );
}

#[test]
fn el_borrado_de_poligono_elimina_sus_puntos_anteriores() {
    let mut raster = Raster::nuevo(40, 12, false);
    for x in 0..80 {
        raster.punto(x, 24, Color::Rgb(1, 2, 3));
    }
    assert_eq!(puntos(&raster), 80);

    raster.borrar_poligono(&[
        Punto2::nuevo(20.0, 20.0),
        Punto2::nuevo(50.0, 20.0),
        Punto2::nuevo(50.0, 28.0),
        Punto2::nuevo(20.0, 28.0),
    ]);

    for x in 20..50 {
        assert!(!raster.marcado(x, 24), "el punto {x} debía borrarse");
    }
    assert!(raster.marcado(19, 24));
    assert!(raster.marcado(50, 24));
    assert_eq!(puntos(&raster), 50);
}

#[test]
fn una_linea_punteada_tiene_la_mitad_de_puntos_que_la_continua() {
    let mut continua = Raster::nuevo(20, 12, false);
    let mut punteada = Raster::nuevo(20, 12, false);
    let (desde, hasta) = (Punto2::nuevo(2.0, 2.0), Punto2::nuevo(37.0, 45.0));
    let color = Color::Rgb(9, 9, 9);
    continua.linea(desde, hasta, color, false);
    punteada.linea(desde, hasta, color, true);

    assert_eq!(puntos(&punteada) * 2, puntos(&continua));
}

#[test]
fn el_edificio_trasero_no_asoma_por_detras_del_delantero() {
    let dims = Dimensiones {
        ancho: 12.0,
        profundidad: 12.0,
    };
    let camara = Camara::orbita(0.0, 0.0, dims, 20.0);
    let objetivo = Punto3::nuevo(0.0, 0.15 * dims.ancho, 0.0);
    let en_el_rayo = |paso: f64| {
        Punto3::nuevo(
            camara.ojo.x + (objetivo.x - camara.ojo.x) * paso,
            camara.ojo.y + (objetivo.y - camara.ojo.y) * paso,
            camara.ojo.z + (objetivo.z - camara.ojo.z) * paso,
        )
    };
    // Dos edificios iguales, uno detrás del otro sobre el rayo de la cámara.
    let cerca = en_el_rayo(0.3);
    let lejos = en_el_rayo(0.55);
    let delantero = Prisma::nuevo(cerca.x, cerca.z, 0.7, 6.0);
    let trasero = Prisma::nuevo(lejos.x, lejos.z, 0.7, 6.0);

    let mut solo_trasero = Raster::nuevo(80, 24, false);
    let mut solo_delantero = Raster::nuevo(80, 24, false);
    let mut los_dos = Raster::nuevo(80, 24, false);
    let p = paleta();
    dibujar_prisma(&mut solo_trasero, &camara, &trasero, &p, 10.0, true);
    dibujar_prisma(&mut solo_delantero, &camara, &delantero, &p, 10.0, true);
    dibujar_prisma(&mut los_dos, &camara, &trasero, &p, 10.0, true);
    dibujar_prisma(&mut los_dos, &camara, &delantero, &p, 10.0, true);

    // Silueta del delantero: la unión de sus caras visibles, en puntos del raster.
    let mut silueta: Vec<[Punto2; 4]> = Vec::new();
    for cara in delantero
        .caras
        .iter()
        .filter(|cara| cara_visible(&camara, cara))
    {
        let mut puntos = [Punto2::nuevo(0.0, 0.0); 4];
        let mut completa = true;
        for (posicion, indice) in cara.indices.iter().enumerate() {
            match proyectar(&camara, delantero.vertices[*indice]) {
                Some(proyectado) => puntos[posicion] = a_puntos(&solo_delantero, proyectado),
                None => completa = false,
            }
        }
        if completa {
            silueta.push(puntos);
        }
    }
    assert!(!silueta.is_empty(), "el delantero debe mostrar alguna cara");

    let mut tapados = 0;
    let mut fuera = 0;
    for y in 0..solo_trasero.alto_puntos() {
        for x in 0..solo_trasero.ancho_puntos() {
            if !solo_trasero.marcado(x, y) {
                continue;
            }
            let (centro_x, centro_y) = (x as f64 + 0.5, y as f64 + 0.5);
            if silueta.iter().any(|cara| dentro(cara, centro_x, centro_y)) {
                // Los puntos que también son del delantero no dicen nada.
                if solo_delantero.marcado(x, y) {
                    continue;
                }
                tapados += 1;
                assert!(
                    !los_dos.marcado(x, y),
                    "el edificio trasero asoma en ({x}, {y})"
                );
            } else {
                fuera += 1;
                assert!(
                    los_dos.marcado(x, y),
                    "el borrado no debe tocar lo que no tapa ({x}, {y})"
                );
            }
        }
    }
    assert!(tapados > 0, "la silueta del delantero debe tapar algo");
    assert!(fuera > 0, "el trasero debe asomar por fuera de la silueta");
}

/// Polígono convexo o cóncavo por lanzamiento de rayo.
fn dentro(poligono: &[Punto2; 4], x: f64, y: f64) -> bool {
    let mut dentro = false;
    let mut anterior = poligono.len() - 1;
    for actual in 0..poligono.len() {
        let (uno, dos) = (poligono[actual], poligono[anterior]);
        if (uno.y > y) != (dos.y > y) {
            let cruce = uno.x + (y - uno.y) * (dos.x - uno.x) / (dos.y - uno.y);
            if x < cruce {
                dentro = !dentro;
            }
        }
        anterior = actual;
    }
    dentro
}

#[test]
fn ciudad_dibuja_en_tres_tamanos_y_tres_estados() {
    let analisis = [
        analisis_real(),
        Analisis::ambiental(1200, 64, 160),
        Analisis::vacio(64, 160),
    ];
    for ascii in [false, true] {
        for punteado in [true, false] {
            let ajustes = AjustesCiudad {
                punteado,
                ..AjustesCiudad::default()
            };
            let lista = visuales::registro(ascii, &ajustes);
            let visual = &lista[6];
            for (ancho, alto) in TAMANOS {
                for estado in &analisis {
                    // Cinco frames: la historia entra una fila cada dos.
                    for _ in 0..5 {
                        dibujar(visual, estado, ancho, alto);
                    }
                }
                visual.borrow_mut().reiniciar();
                dibujar(visual, &analisis[0], ancho, alto);
            }
        }
    }
}

#[test]
fn ciudad_aguanta_un_redimensionado() {
    let ajustes = AjustesCiudad::default();
    let lista = visuales::registro(false, &ajustes);
    let visual = &lista[6];
    let analisis = analisis_real();
    for (ancho, alto) in [(80, 24), (40, 12), (200, 50), (80, 24)] {
        visual.borrow_mut().reiniciar();
        for _ in 0..3 {
            dibujar(visual, &analisis, ancho, alto);
        }
    }
}

#[test]
fn la_ciudad_dibuja_algo_con_audio_y_se_apaga_sin_el() {
    let ajustes = AjustesCiudad::default();
    let area = Rect::new(0, 0, 80, 24);
    let p = paleta();
    let pintadas = |analisis: &Analisis| {
        let ciudad =
            std::cell::RefCell::new(mmmusic::visuales::ciudad::Ciudad::nuevo(false, ajustes));
        let mut buffer = Buffer::empty(area);
        for _ in 0..20 {
            let canvas = Canvas::default()
                .x_bounds([0.0, 80.0])
                .y_bounds([0.0, 24.0])
                .background_color(Color::Reset)
                .paint(|ctx| ciudad.borrow_mut().dibujar(analisis, area, &p, 0.033, ctx));
            canvas.render(area, &mut buffer);
        }
        buffer
            .content()
            .iter()
            .filter(|celda| celda.symbol() != " ")
            .count()
    };
    let con_audio = pintadas(&analisis_real());
    let sin_audio = pintadas(&Analisis::ambiental(1200, 64, 160));
    assert!(
        con_audio > 100,
        "con espectro la ciudad debe llenar la pantalla"
    );
    assert!(sin_audio > 0, "en ambiental sigue habiendo ciudad baja");
    assert!(
        sin_audio < con_audio,
        "la ciudad ambiental es más baja que la real"
    );
}

#[test]
fn la_reserva_del_raster_no_crece_con_los_frames() {
    let mut raster = Raster::nuevo(80, 24, false);
    let color = Color::Rgb(4, 5, 6);
    let antes = puntos(&raster);
    for frame in 0..10 {
        raster.limpiar();
        for x in 0..160 {
            raster.punto(x, frame, color);
        }
        assert!(puntos(&raster) > antes);
    }
    raster.limpiar();
    assert_eq!(puntos(&raster), 0);
}

/// Medición del presupuesto de frame (riesgo R-27). El peor caso posible es
/// 200 × 70 con `ciudad_filas = 16`: 24 bandas × 16 filas = 384 prismas. Se
/// ejecuta a mano:
/// `cargo test --release --test ciudad -- --ignored --nocapture`
#[test]
#[ignore = "medición manual del presupuesto de frame"]
fn el_frame_cabe_en_el_presupuesto_de_la_fase_3() {
    use std::time::Instant;

    let p = paleta();
    let analisis = analisis_real();
    let ajustes = AjustesCiudad {
        filas: 16,
        ..AjustesCiudad::default()
    };

    let mut peor = 0.0f64;
    for (ancho, alto) in [(200u16, 50u16), (200, 70)] {
        let area = Rect::new(0, 0, ancho, alto);
        let ciudad =
            std::cell::RefCell::new(mmmusic::visuales::ciudad::Ciudad::nuevo(false, ajustes));
        let mut duraciones = Vec::new();
        for frame in 0..40 {
            let inicio = Instant::now();
            let mut buffer = Buffer::empty(area);
            let canvas = Canvas::default()
                .x_bounds([0.0, f64::from(ancho)])
                .y_bounds([0.0, f64::from(alto)])
                .background_color(Color::Reset)
                .paint(|ctx| ciudad.borrow_mut().dibujar(&analisis, area, &p, 0.033, ctx));
            canvas.render(area, &mut buffer);
            if frame >= 10 {
                duraciones.push(inicio.elapsed().as_secs_f64() * 1000.0);
            }
        }
        duraciones.sort_by(f64::total_cmp);
        let mediana = duraciones[duraciones.len() / 2];
        let p95 = duraciones[duraciones.len() * 95 / 100];
        let prismas = (ancho as usize / 6).clamp(8, 24) * (alto as usize / 4).clamp(8, 16).min(16);
        println!("{ancho}×{alto} con {prismas} prismas: mediana {mediana:.2} ms, p95 {p95:.2} ms");
        peor = peor.max(p95);
    }
    assert!(
        peor < 33.0,
        "el frame debe caber en el presupuesto de F3 (33 ms a 30 fps): {peor:.2} ms"
    );
}
