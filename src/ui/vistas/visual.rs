use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::Canvas;
use ratatui::widgets::{Clear, Paragraph};

use crate::app::AppEstado;
use crate::audio::EstadoCaptura;
use crate::config::FuentePaleta;
use crate::letras::Letra;

pub const ANCHO_MINIMO: u16 = 40;
pub const ALTO_MINIMO: u16 = 12;

pub fn dibujar(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (cabecera, resto) = if app.cabecera_visible() && area.height > 3 {
        let [cabecera, resto] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
        (Some(cabecera), resto)
    } else {
        (None, area)
    };
    let (cuerpo, ayuda) = if resto.height > 2 {
        let [cuerpo, ayuda] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(resto);
        (cuerpo, ayuda)
    } else {
        (resto, Rect::default())
    };
    if let Some(cabecera) = cabecera {
        dibujar_cabecera(frame, app, cabecera);
    }

    let indice = if cuerpo.width < ANCHO_MINIMO || cuerpo.height < ALTO_MINIMO {
        0
    } else {
        app.indice_visual.min(app.visuales.len().saturating_sub(1))
    };
    let paleta = app.paleta_visual;
    let analisis = app.analisis.clone();
    let dt = app.dt_frame;
    let visual = app.visuales[indice].clone();
    let canvas = Canvas::default()
        .x_bounds([0.0, f64::from(cuerpo.width)])
        .y_bounds([0.0, f64::from(cuerpo.height)])
        .background_color(paleta.fondo)
        .paint(|ctx| {
            visual
                .borrow_mut()
                .dibujar(&analisis, cuerpo, &paleta, dt, ctx);
        });
    frame.render_widget(canvas, cuerpo);

    dibujar_superposicion(frame, app, cuerpo);

    if ayuda.height > 0 {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " v/V visual · 1-6 elegir · [ ] sensibilidad · b paleta · 9 letras · 7/Esc salir · ? ayuda",
                Style::new().fg(app.paleta.secundario),
            ))),
            ayuda,
        );
    }
}

fn dibujar_superposicion(frame: &mut Frame, app: &AppEstado, area: Rect) {
    if !app.letras_superpuestas || area.height == 0 {
        return;
    }
    let Letra::Sincronizada { lineas, .. } = &app.letras.letra else {
        return;
    };
    if lineas.is_empty() {
        return;
    }
    let visible = (app.config.letras.tamano_superposicion as usize).clamp(1, area.height as usize);
    let banda = Rect {
        y: area.bottom() - visible as u16,
        height: visible as u16,
        ..area
    };
    let actual = app.linea_actual_letras();
    let centro = actual.max(0) as usize;
    let inicio = centro
        .saturating_sub(visible / 2)
        .min(lineas.len().saturating_sub(visible));
    let mut visibles = Vec::new();
    for (indice, (_, texto)) in lineas.iter().enumerate().skip(inicio).take(visible) {
        let es_actual = indice as i64 == actual;
        let prefijo = if es_actual { "▶ " } else { "  " };
        let estilo = if es_actual {
            Style::new()
                .fg(app.paleta.acento)
                .bg(app.paleta.fondo)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.secundario).bg(app.paleta.fondo)
        };
        visibles.push(Line::from(Span::styled(
            format!("{prefijo}{texto}"),
            estilo,
        )));
    }
    frame.render_widget(Clear, banda);
    frame.render_widget(
        Paragraph::new(visibles)
            .style(Style::new().bg(app.paleta.fondo))
            .alignment(Alignment::Center),
        banda,
    );
}

fn dibujar_cabecera(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let numero = app.indice_visual + 1;
    let fuente = match app.fuente_paleta {
        FuentePaleta::Tema => "tema",
        FuentePaleta::Caratula => "carátula",
    };
    let nota = if app.fuente_paleta == FuentePaleta::Caratula && app.colores_actuales.is_none() {
        " ♪"
    } else {
        ""
    };
    let captura = match &app.estado_captura {
        EstadoCaptura::Capturando { monitor: true, .. } => " · captura por monitor".to_string(),
        EstadoCaptura::Capturando { .. } => String::new(),
        otro => format!(" · {}", otro.etiqueta()),
    };
    let letras = if app.letras_superpuestas && app.letras.letra.es_sincronizada() {
        " · letras"
    } else {
        ""
    };
    let texto = format!(
        " mmmusic · Visual {numero}/{} {} · paleta: {fuente}{nota} · sens {:.2}{captura}{letras}",
        app.visuales.len(),
        app.visual_actual_nombre(),
        app.sensibilidad
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            texto,
            Style::new()
                .fg(app.paleta.texto)
                .bg(app.paleta_visual.fondo)
                .add_modifier(Modifier::BOLD),
        ))),
        area,
    );
}
