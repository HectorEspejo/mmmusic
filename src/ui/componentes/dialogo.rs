use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::app::{AppEstado, Dialogo};

pub fn dibujar(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let Some(dialogo) = app.dialogo.as_ref() else {
        return;
    };
    match dialogo {
        Dialogo::Confirmacion {
            titulo, mensaje, ..
        } => {
            dibujar_confirmacion(frame, app, area, titulo, mensaje);
        }
        Dialogo::Texto { titulo, valor, .. } => {
            dibujar_texto(frame, app, area, titulo, valor);
        }
        Dialogo::Selector {
            titulo, seleccion, ..
        } => dibujar_selector(frame, app, area, titulo, *seleccion),
    }
}

fn marco(frame: &mut Frame, app: &AppEstado, destino: Rect, titulo: &str) -> Rect {
    frame.render_widget(Clear, destino);
    let bloque = Block::bordered()
        .title(format!(" {titulo} "))
        .border_style(Style::new().fg(app.paleta.acento).bg(app.paleta.fondo))
        .style(Style::new().fg(app.paleta.texto).bg(app.paleta.fondo));
    let interior = bloque.inner(destino);
    frame.render_widget(bloque, destino);
    interior
}

fn centrar(area: Rect, ancho: u16, alto: u16) -> Rect {
    let ancho = ancho.min(area.width.saturating_sub(2)).max(10);
    let alto = alto.min(area.height.saturating_sub(2)).max(3);
    Rect {
        x: area.x + area.width.saturating_sub(ancho) / 2,
        y: area.y + area.height.saturating_sub(alto) / 2,
        width: ancho,
        height: alto,
    }
}

fn dibujar_confirmacion(
    frame: &mut Frame,
    app: &AppEstado,
    area: Rect,
    titulo: &str,
    mensaje: &str,
) {
    let destino = centrar(area, 56, 7);
    let interior = marco(frame, app, destino, titulo);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {mensaje}"),
                Style::new().fg(app.paleta.texto),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Enter/S confirmar · Esc/N cancelar",
                Style::new().fg(app.paleta.secundario),
            )),
        ]),
        interior,
    );
}

fn dibujar_texto(frame: &mut Frame, app: &AppEstado, area: Rect, titulo: &str, valor: &str) {
    let destino = centrar(area, 64, 6);
    let interior = marco(frame, app, destino, titulo);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  > ", Style::new().fg(app.paleta.acento)),
                Span::styled(
                    valor.to_string(),
                    Style::new()
                        .fg(app.paleta.texto)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("▌", Style::new().fg(app.paleta.acento)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Enter aceptar · Esc cancelar",
                Style::new().fg(app.paleta.secundario),
            )),
        ]),
        interior,
    );
}

fn dibujar_selector(
    frame: &mut Frame,
    app: &AppEstado,
    area: Rect,
    titulo: &str,
    seleccion: usize,
) {
    let alto = (app.playlists.len() as u16 + 5).min(area.height.saturating_sub(2));
    let destino = centrar(area, 52, alto);
    let interior = marco(frame, app, destino, titulo);
    let mut lineas = Vec::new();
    for (indice, playlist) in app.playlists.iter().enumerate() {
        let seleccionada = indice == seleccion;
        let estilo = if seleccionada {
            Style::new()
                .fg(app.paleta.fondo)
                .bg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.texto)
        };
        lineas.push(Line::from(Span::styled(
            format!(
                "  {}  ·  {} pistas",
                crate::ui::vistas::truncar(&playlist.nombre, 30),
                playlist.num_pistas
            ),
            estilo,
        )));
    }
    let nueva_seleccionada = seleccion >= app.playlists.len();
    lineas.push(Line::from(Span::styled(
        "  + Nueva playlist…",
        if nueva_seleccionada {
            Style::new()
                .fg(app.paleta.fondo)
                .bg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.acento)
        },
    )));
    lineas.push(Line::from(""));
    lineas.push(Line::from(Span::styled(
        "  Enter añadir · Esc cancelar",
        Style::new().fg(app.paleta.secundario),
    )));
    frame.render_widget(Paragraph::new(lineas).alignment(Alignment::Left), interior);
}
