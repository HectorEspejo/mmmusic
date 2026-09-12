use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{AppEstado, Foco};

pub fn dibujar(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    let titulo = format!("Cola ({})", app.estado_reproductor.cola.len());
    let bloque = super::panel_cola(app, &titulo);
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);
    if interior.height == 0 {
        return;
    }

    if app.estado_reproductor.cola.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    " La cola está vacía.",
                    Style::new().fg(app.paleta.secundario),
                )),
                Line::from(Span::styled(
                    " a añade · A a continuación",
                    Style::new().fg(app.paleta.secundario),
                )),
            ]),
            interior,
        );
        return;
    }

    let altura_ayuda = u16::from(interior.height > 6);
    let [lista, ayuda] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Min(1),
        ratatui::layout::Constraint::Length(altura_ayuda),
    ])
    .areas(interior);

    let (inicio, fin) = super::ventana(
        app.estado_reproductor.cola.len(),
        app.seleccion_cola,
        lista.height as usize,
    );
    app.zonas.cola = Some((lista, inicio));
    let lineas: Vec<Line> = app.estado_reproductor.cola[inicio..fin]
        .iter()
        .enumerate()
        .map(|(desplazamiento, pista)| {
            let indice = inicio + desplazamiento;
            let actual = app.estado_reproductor.cola_indice == Some(indice);
            let seleccionada = indice == app.seleccion_cola && app.foco == Foco::Cola;
            let estilo = if seleccionada {
                Style::new()
                    .fg(app.paleta.fondo)
                    .bg(app.paleta.acento)
                    .add_modifier(Modifier::BOLD)
            } else if actual {
                Style::new().fg(app.paleta.acento)
            } else {
                Style::new().fg(app.paleta.texto)
            };
            Line::from(Span::styled(
                format!(
                    " {} {}",
                    if actual { app.iconos.reproducida } else { " " },
                    super::truncar(&pista.titulo, lista.width.saturating_sub(3) as usize)
                ),
                estilo,
            ))
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lineas).style(Style::new().fg(app.paleta.texto).bg(app.paleta.fondo)),
        lista,
    );
    if altura_ayuda == 1 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " d quitar · J/K mover · C vaciar",
                Style::new().fg(app.paleta.secundario),
            )),
            ayuda,
        );
    }
}
