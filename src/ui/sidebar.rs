use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{AppEstado, Foco, Vista};
use crate::ui::miles;

pub fn dibujar(frame: &mut Frame, app: &AppEstado, area: Rect, compacto: bool) {
    if area.width < 4 || area.height < 3 {
        return;
    }
    let estilo = Style::new().fg(app.paleta.texto).bg(app.paleta.fondo);
    let bloque = Block::bordered()
        .title(" mmmusic ")
        .border_style(Style::new().fg(app.paleta.secundario).bg(app.paleta.fondo))
        .style(estilo);
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);

    let alto_pie = 1 + u16::from(app.escaneo_activo.is_some());
    let [cuerpo, pie] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(alto_pie)]).areas(interior);

    let mut lineas = Vec::new();
    for vista in Vista::TODAS {
        let activa = app.vista == vista;
        let marcador = if app.foco == Foco::Sidebar && activa {
            " ◄"
        } else {
            ""
        };
        let texto = if compacto {
            format!(" {} ", vista.numero())
        } else {
            format!(" {} {}{}", vista.numero(), vista.titulo(), marcador)
        };
        let estilo_linea = if activa {
            Style::new()
                .fg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.texto)
        };
        lineas.push(Line::from(Span::styled(texto, estilo_linea)));
    }
    if !compacto {
        lineas.push(Line::from(""));
        lineas.push(Line::from(Span::styled(
            " Playlists",
            Style::new().fg(app.paleta.secundario),
        )));
        lineas.push(Line::from(Span::styled(
            format!("  {}", crate::app::NOMBRE_FAVORITAS),
            Style::new().fg(app.paleta.texto),
        )));
        for playlist in &app.playlists {
            lineas.push(Line::from(Span::styled(
                format!("  {}", playlist.nombre),
                Style::new().fg(app.paleta.texto),
            )));
        }
    }
    frame.render_widget(Paragraph::new(lineas).style(estilo), cuerpo);

    let mut pie_lineas = vec![Line::from(Span::styled(
        format!(" {} pistas", miles(app.total_pistas)),
        Style::new().fg(app.paleta.secundario),
    ))];
    if let Some(progreso) = &app.escaneo_activo {
        pie_lineas.push(Line::from(Span::styled(
            format!(" escaneando… {} %", progreso.porcentaje()),
            Style::new().fg(app.paleta.aviso),
        )));
    }
    frame.render_widget(Paragraph::new(pie_lineas).style(estilo), pie);
}
