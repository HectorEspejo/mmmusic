use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{AppEstado, Foco, Pantalla};
use crate::ui::formatear_ms;

pub fn dibujar(frame: &mut Frame, app: &AppEstado, area: Rect) {
    match app.pantalla {
        Pantalla::DetalleArtista => dibujar_detalle(frame, app, area),
        _ => dibujar_lista(frame, app, area),
    }
}

fn dibujar_lista(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let bloque = super::panel(app, "Artistas");
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);

    if app.artistas.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                if app.total_pistas == 0 {
                    " No hay artistas. Pulsa Ctrl+r para escanear la biblioteca."
                } else {
                    " No hay artistas."
                },
                Style::new().fg(app.paleta.secundario),
            )),
            interior,
        );
        return;
    }

    let capacidad = interior.height as usize;
    let (inicio, fin) = super::ventana(app.artistas.len(), app.seleccion, capacidad);
    let lineas: Vec<Line> = app.artistas[inicio..fin]
        .iter()
        .enumerate()
        .map(|(desplazamiento, artista)| {
            let indice = inicio + desplazamiento;
            let seleccionada = indice == app.seleccion && app.foco == Foco::Contenido;
            let estilo = if seleccionada {
                Style::new()
                    .fg(app.paleta.fondo)
                    .bg(app.paleta.acento)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(app.paleta.texto)
            };
            let detalle = Style::new().fg(app.paleta.secundario);
            Line::from(vec![
                Span::styled(
                    format!(" {}  ", super::truncar(&artista.nombre, 32)),
                    estilo,
                ),
                Span::styled(
                    format!(
                        "{} · {}",
                        super::contar_albumes(artista.num_albumes),
                        super::contar_pistas(artista.num_pistas)
                    ),
                    if seleccionada { estilo } else { detalle },
                ),
            ])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lineas).style(Style::new().fg(app.paleta.texto).bg(app.paleta.fondo)),
        interior,
    );
}

fn dibujar_detalle(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let Some(detalle) = app.detalle_artista.as_ref() else {
        return;
    };
    let bloque = super::panel(app, &detalle.artista.nombre);
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);

    let encabezado = Style::new()
        .fg(app.paleta.acento)
        .add_modifier(Modifier::BOLD);
    let secundario = Style::new().fg(app.paleta.secundario);
    let mut filas: Vec<(Option<usize>, Line)> = Vec::new();
    let mut indice = 0usize;

    filas.push((
        None,
        Line::from(Span::styled(
            format!(
                " {} · {}",
                super::contar_albumes(detalle.artista.num_albumes),
                super::contar_pistas(detalle.artista.num_pistas)
            ),
            secundario,
        )),
    ));
    filas.push((None, Line::from("")));
    filas.push((None, Line::from(Span::styled(" Álbumes", encabezado))));
    for album in &detalle.albumes {
        let seleccionada = indice == app.seleccion && app.foco == Foco::Contenido;
        let estilo = if seleccionada {
            Style::new()
                .fg(app.paleta.fondo)
                .bg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.texto)
        };
        let sufijo = if seleccionada {
            estilo
        } else {
            Style::new().fg(app.paleta.secundario)
        };
        filas.push((
            Some(indice),
            Line::from(vec![
                Span::styled(format!("  {}  ", super::truncar(&album.titulo, 34)), estilo),
                Span::styled(
                    format!(
                        "{} · {} pistas",
                        album
                            .anio
                            .map(|anio| anio.to_string())
                            .unwrap_or_else(|| "—".to_string()),
                        album.num_pistas
                    ),
                    sufijo,
                ),
            ]),
        ));
        indice += 1;
    }

    if !detalle.pistas_sueltas.is_empty() {
        filas.push((None, Line::from("")));
        filas.push((
            None,
            Line::from(Span::styled(" Pistas sueltas", encabezado)),
        ));
        for pista in &detalle.pistas_sueltas {
            let seleccionada = indice == app.seleccion && app.foco == Foco::Contenido;
            let estilo = if seleccionada {
                Style::new()
                    .fg(app.paleta.fondo)
                    .bg(app.paleta.acento)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(app.paleta.texto)
            };
            filas.push((
                Some(indice),
                Line::from(Span::styled(
                    format!(
                        "  {}  ·  {}  {}",
                        super::truncar(&pista.titulo, 34),
                        super::truncar(&pista.album, 22),
                        formatear_ms(pista.duracion_ms)
                    ),
                    estilo,
                )),
            ));
            indice += 1;
        }
    }

    let linea_seleccion = filas
        .iter()
        .position(|(seleccion, _)| *seleccion == Some(app.seleccion))
        .unwrap_or(0);
    let (inicio, fin) = super::ventana(filas.len(), linea_seleccion, interior.height as usize);
    let visibles: Vec<Line> = filas[inicio..fin]
        .iter()
        .map(|(_, linea)| linea.clone())
        .collect();
    frame.render_widget(
        Paragraph::new(visibles).style(Style::new().fg(app.paleta.texto).bg(app.paleta.fondo)),
        interior,
    );
}
