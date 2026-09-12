use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use crate::app::AppEstado;
use crate::eventos::NivelAviso;

pub fn dibujar(frame: &mut Frame, app: &AppEstado, area: Rect) {
    if app.avisos.is_empty() || area.width < 12 {
        return;
    }
    let ancho = 40.min(area.width.saturating_sub(4));
    for (indice, aviso) in app.avisos.iter().enumerate() {
        let y = area.y + 1 + indice as u16;
        if y >= area.bottom() {
            break;
        }
        let color = match aviso.nivel {
            NivelAviso::Info => app.paleta.acento,
            NivelAviso::Aviso => app.paleta.aviso,
            NivelAviso::Error => app.paleta.error,
        };
        let destino = Rect {
            x: area.right().saturating_sub(ancho + 2),
            y,
            width: ancho,
            height: 1,
        };
        frame.render_widget(Clear, destino);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {} ", aviso.texto),
                Style::new().fg(color).bg(app.paleta.fondo),
            ))),
            destino,
        );
    }
}
