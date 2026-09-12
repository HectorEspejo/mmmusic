use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{AppEstado, Foco, Pantalla};
use crate::ui::formatear_ms;

pub fn dibujar(frame: &mut Frame, app: &AppEstado, area: Rect) {
    match app.pantalla {
        Pantalla::DetallePlaylist => dibujar_detalle(frame, app, area),
        _ => dibujar_lista(frame, app, area),
    }
}

fn dibujar_lista(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let bloque = super::panel(app, "Playlists");
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);

    if app.playlists.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    " No hay playlists todavía.",
                    Style::new().fg(app.paleta.texto),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    " N crea una · i importa M3U/M3U8 · P añade la selección",
                    Style::new().fg(app.paleta.secundario),
                )),
            ]),
            interior,
        );
        return;
    }

    let (inicio, fin) =
        super::ventana(app.playlists.len(), app.seleccion, interior.height as usize);
    let lineas: Vec<Line> = app.playlists[inicio..fin]
        .iter()
        .enumerate()
        .map(|(desplazamiento, playlist)| {
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
            let sufijo = if seleccionada {
                estilo
            } else {
                Style::new().fg(app.paleta.secundario)
            };
            Line::from(vec![
                Span::styled(
                    format!(" {}  ", super::truncar(&playlist.nombre, 40)),
                    estilo,
                ),
                Span::styled(super::contar_pistas(playlist.num_pistas), sufijo),
            ])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lineas).style(Style::new().fg(app.paleta.texto).bg(app.paleta.fondo)),
        interior,
    );
}

fn dibujar_detalle(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let Some((playlist, pistas)) = app.detalle_playlist.as_ref() else {
        return;
    };
    let bloque = super::panel(app, &playlist.nombre);
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);

    if pistas.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    " Playlist vacía.",
                    Style::new().fg(app.paleta.texto),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    " P añade la selección desde cualquier vista.",
                    Style::new().fg(app.paleta.secundario),
                )),
            ]),
            interior,
        );
        return;
    }

    let id_sonando = app
        .estado_reproductor
        .pista_actual
        .as_ref()
        .map(|pista| pista.id);
    let (inicio, fin) = super::ventana(pistas.len(), app.seleccion, interior.height as usize);
    let lineas: Vec<Line> = pistas[inicio..fin]
        .iter()
        .enumerate()
        .map(|(desplazamiento, pista)| {
            let indice = inicio + desplazamiento;
            let seleccionada = indice == app.seleccion && app.foco == Foco::Contenido;
            let sonando = id_sonando == Some(pista.id);
            let estilo = if seleccionada {
                Style::new()
                    .fg(app.paleta.fondo)
                    .bg(app.paleta.acento)
                    .add_modifier(Modifier::BOLD)
            } else if sonando {
                Style::new().fg(app.paleta.acento)
            } else {
                Style::new().fg(app.paleta.texto)
            };
            Line::from(vec![
                Span::styled(
                    format!(
                        " {} {:>3}  ",
                        if sonando { app.iconos.reproducida } else { " " },
                        indice + 1
                    ),
                    estilo,
                ),
                Span::styled(super::truncar(&pista.titulo, 34), estilo),
                Span::styled(
                    format!(
                        "  ·  {}  {}",
                        super::truncar(&pista.artista, 18),
                        formatear_ms(pista.duracion_ms)
                    ),
                    if seleccionada {
                        estilo
                    } else {
                        Style::new().fg(app.paleta.secundario)
                    },
                ),
            ])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lineas).style(Style::new().fg(app.paleta.texto).bg(app.paleta.fondo)),
        interior,
    );
}
