use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{AppEstado, Foco};
use crate::ui::{formatear_ms, miles};

pub fn dibujar(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let bloque = super::panel(app, "Buscar");
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);
    if interior.height < 2 {
        return;
    }

    let [entrada, resto] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(interior);
    let estilo_entrada = if app.busqueda_enfocada {
        Style::new()
            .fg(app.paleta.acento)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(app.paleta.texto)
    };
    let cursor = if app.busqueda_enfocada { "▌" } else { "" };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" / ", Style::new().fg(app.paleta.acento)),
            Span::styled(app.busqueda.clone(), estilo_entrada),
            Span::styled(cursor, Style::new().fg(app.paleta.acento)),
        ])),
        entrada,
    );

    if app.busqueda.trim().chars().count() < 2 {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    " Escribe al menos 2 caracteres para buscar por artista, álbum o pista.",
                    Style::new().fg(app.paleta.secundario),
                )),
            ]),
            resto,
        );
        return;
    }

    let encabezado = Style::new()
        .fg(app.paleta.acento)
        .add_modifier(Modifier::BOLD);
    let secundario = Style::new().fg(app.paleta.secundario);
    let mut filas: Vec<(Option<usize>, Line)> = Vec::new();
    let mut indice = 0usize;

    filas.push((None, Line::from(Span::styled(" Artistas", encabezado))));
    if app.resultados.artistas.is_empty() {
        filas.push((
            None,
            Line::from(Span::styled("  (sin resultados)", secundario)),
        ));
    }
    for artista in &app.resultados.artistas {
        filas.push((
            Some(indice),
            Line::from(Span::styled(
                format!(
                    "  {}  ·  {}",
                    super::truncar(&artista.nombre, 40),
                    super::contar_albumes(artista.num_albumes)
                ),
                estilo_fila(app, indice),
            )),
        ));
        indice += 1;
    }
    if app.resultados.mas_artistas > 0 {
        filas.push((
            None,
            Line::from(Span::styled(
                format!("  +{} más", miles(app.resultados.mas_artistas as i64)),
                secundario,
            )),
        ));
    }

    filas.push((None, Line::from("")));
    filas.push((None, Line::from(Span::styled(" Álbumes", encabezado))));
    if app.resultados.albumes.is_empty() {
        filas.push((
            None,
            Line::from(Span::styled("  (sin resultados)", secundario)),
        ));
    }
    for album in &app.resultados.albumes {
        filas.push((
            Some(indice),
            Line::from(Span::styled(
                format!(
                    "  {}  ·  {}  {}",
                    super::truncar(&album.titulo, 34),
                    super::truncar(&album.artista, 22),
                    album.anio.map(|anio| anio.to_string()).unwrap_or_default()
                ),
                estilo_fila(app, indice),
            )),
        ));
        indice += 1;
    }
    if app.resultados.mas_albumes > 0 {
        filas.push((
            None,
            Line::from(Span::styled(
                format!("  +{} más", miles(app.resultados.mas_albumes as i64)),
                secundario,
            )),
        ));
    }

    filas.push((None, Line::from("")));
    filas.push((None, Line::from(Span::styled(" Pistas", encabezado))));
    if app.resultados.pistas.is_empty() {
        filas.push((
            None,
            Line::from(Span::styled("  (sin resultados)", secundario)),
        ));
    }
    for pista in &app.resultados.pistas {
        filas.push((
            Some(indice),
            Line::from(Span::styled(
                format!(
                    "  {}  ·  {}  {}",
                    super::truncar(&pista.titulo, 32),
                    super::truncar(&pista.artista, 22),
                    formatear_ms(pista.duracion_ms)
                ),
                estilo_fila(app, indice),
            )),
        ));
        indice += 1;
    }
    if app.resultados.mas_pistas > 0 {
        filas.push((
            None,
            Line::from(Span::styled(
                format!("  +{} más", miles(app.resultados.mas_pistas as i64)),
                secundario,
            )),
        ));
    }

    let linea_seleccion = filas
        .iter()
        .position(|(seleccion, _)| *seleccion == Some(app.seleccion))
        .unwrap_or(0);
    let (inicio, fin) = super::ventana(filas.len(), linea_seleccion, resto.height as usize);
    let visibles: Vec<Line> = filas[inicio..fin]
        .iter()
        .map(|(_, linea)| linea.clone())
        .collect();
    frame.render_widget(
        Paragraph::new(visibles).style(Style::new().fg(app.paleta.texto).bg(app.paleta.fondo)),
        resto,
    );
}

fn estilo_fila(app: &AppEstado, indice: usize) -> Style {
    if indice == app.seleccion && app.foco == Foco::Contenido && !app.busqueda_enfocada {
        Style::new()
            .fg(app.paleta.fondo)
            .bg(app.paleta.acento)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(app.paleta.texto)
    }
}
