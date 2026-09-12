use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::AppEstado;
use crate::ui::formatear_ms;

pub fn dibujar(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let bloque = super::panel(app, "Inicio");
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);

    let encabezado = Style::new()
        .fg(app.paleta.acento)
        .add_modifier(Modifier::BOLD);
    let secundario = Style::new().fg(app.paleta.secundario);
    let seleccionado = Style::new()
        .fg(app.paleta.fondo)
        .bg(app.paleta.acento)
        .add_modifier(Modifier::BOLD);
    let normal = Style::new().fg(app.paleta.texto);

    let mut filas: Vec<(Option<usize>, Line)> = Vec::new();
    let mut indice = 0usize;

    filas.push((
        None,
        Line::from(Span::styled(" Reproducidas recientemente", encabezado)),
    ));
    if app.inicio.recientes.is_empty() {
        filas.push((
            None,
            Line::from(Span::styled("  (sin reproducciones todavía)", secundario)),
        ));
    }
    for pista in &app.inicio.recientes {
        let estilo = if indice == app.seleccion && app.foco == crate::app::Foco::Contenido {
            seleccionado
        } else {
            normal
        };
        filas.push((
            Some(indice),
            Line::from(Span::styled(
                format!(
                    "  {} {}  ·  {}  {}",
                    app.iconos.reproducida,
                    super::truncar(&pista.titulo, 30),
                    super::truncar(&pista.artista, 18),
                    formatear_ms(pista.duracion_ms)
                ),
                estilo,
            )),
        ));
        indice += 1;
    }

    filas.push((None, Line::from("")));
    filas.push((
        None,
        Line::from(Span::styled(" Añadidos recientemente", encabezado)),
    ));
    if app.inicio.anadidos.is_empty() {
        filas.push((
            None,
            Line::from(Span::styled(
                if app.total_pistas == 0 {
                    "  (biblioteca vacía; pulsa Ctrl+r para escanear)"
                } else {
                    "  (sin álbumes añadidos)"
                },
                secundario,
            )),
        ));
    }
    for album in &app.inicio.anadidos {
        filas.push((
            Some(indice),
            Line::from(Span::styled(linea_album(album), estilo_album(app, indice))),
        ));
        indice += 1;
    }

    filas.push((None, Line::from("")));
    filas.push((None, Line::from(Span::styled(" Redescubre", encabezado))));
    if app.inicio.redescubre.is_empty() {
        filas.push((
            None,
            Line::from(Span::styled(
                "  (álbumes al azar de tu biblioteca)",
                secundario,
            )),
        ));
    }
    for album in &app.inicio.redescubre {
        filas.push((
            Some(indice),
            Line::from(Span::styled(linea_album(album), estilo_album(app, indice))),
        ));
        indice += 1;
    }

    let linea_seleccion = filas
        .iter()
        .position(|(seleccion, _)| *seleccion == Some(app.seleccion))
        .unwrap_or(0);
    let capacidad = interior.height as usize;
    let (inicio, fin) = super::ventana(filas.len(), linea_seleccion, capacidad);
    let visibles: Vec<Line> = filas[inicio..fin]
        .iter()
        .map(|(_, linea)| linea.clone())
        .collect();
    frame.render_widget(
        Paragraph::new(visibles).style(Style::new().fg(app.paleta.texto).bg(app.paleta.fondo)),
        interior,
    );
}

fn estilo_album(app: &AppEstado, indice: usize) -> Style {
    if indice == app.seleccion && app.foco == crate::app::Foco::Contenido {
        Style::new()
            .fg(app.paleta.fondo)
            .bg(app.paleta.acento)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(app.paleta.texto)
    }
}

fn linea_album(album: &crate::biblioteca::modelos::AlbumResumen) -> String {
    format!(
        "  {} {}  ·  {}  {}",
        "♪",
        super::truncar(&album.titulo, 30),
        super::truncar(&album.artista, 18),
        album.anio.map(|anio| anio.to_string()).unwrap_or_default()
    )
}
