use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Cell, Paragraph, Row, Table};

use crate::app::AppEstado;
use crate::ui::{formatear_ms, miles};

pub fn dibujar(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    let bloque = super::panel(app, "Pistas");
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);
    if app.pistas.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                if app.total_pistas == 0 {
                    " No hay pistas. Pulsa Ctrl+r para escanear la biblioteca."
                } else {
                    " Cargando pistas…"
                },
                Style::new().fg(app.paleta.secundario),
            )),
            interior,
        );
        return;
    }

    let [cuerpo, estado] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(interior);
    let altura = cuerpo.height.saturating_sub(1) as usize;
    let inicio = app.seleccion.saturating_sub(altura / 2);
    let fin = (inicio + altura).min(app.pistas.len());

    let encabezado = Style::new()
        .fg(app.paleta.acento)
        .add_modifier(Modifier::BOLD);
    let cabecera = Row::new(vec![
        Cell::from(""),
        Cell::from(titulo_orden(
            app,
            "Título",
            crate::biblioteca::consultas::OrdenPistas::Titulo,
        )),
        Cell::from(titulo_orden(
            app,
            "Artista",
            crate::biblioteca::consultas::OrdenPistas::Artista,
        )),
        Cell::from(titulo_orden(
            app,
            "Álbum",
            crate::biblioteca::consultas::OrdenPistas::Album,
        )),
        Cell::from(titulo_orden(
            app,
            "Duración",
            crate::biblioteca::consultas::OrdenPistas::Duracion,
        )),
        Cell::from(titulo_orden(
            app,
            "Año",
            crate::biblioteca::consultas::OrdenPistas::Anio,
        )),
        Cell::from("Formato"),
    ])
    .style(encabezado);

    let id_sonando = app.estado_reproductor.pista_actual().map(|pista| pista.id);
    let filas = app.pistas[inicio..fin]
        .iter()
        .enumerate()
        .map(|(desplazamiento, pista)| {
            let indice = inicio + desplazamiento;
            let seleccionada = indice == app.seleccion;
            let sonando = id_sonando == Some(pista.id);
            let estilo = if seleccionada {
                Style::new().fg(app.paleta.fondo).bg(app.paleta.acento)
            } else if sonando {
                Style::new().fg(app.paleta.acento)
            } else {
                Style::new().fg(app.paleta.texto)
            };
            Row::new(vec![
                Cell::from(Span::styled(
                    if sonando { app.iconos.reproducida } else { " " },
                    if seleccionada {
                        Style::new().fg(app.paleta.fondo)
                    } else {
                        Style::new().fg(app.paleta.acento)
                    },
                )),
                Cell::from(pista.titulo.clone()),
                Cell::from(pista.artista.clone()),
                Cell::from(pista.album.clone()),
                Cell::from(formatear_ms(pista.duracion_ms)),
                Cell::from(pista.anio.map(|anio| anio.to_string()).unwrap_or_default()),
                Cell::from(pista.formato.clone()),
            ])
            .style(estilo)
        });

    let anchos = [
        Constraint::Length(1),
        Constraint::Min(16),
        Constraint::Length(18),
        Constraint::Length(18),
        Constraint::Length(8),
        Constraint::Length(5),
        Constraint::Length(6),
    ];
    app.zonas.lista = Some((cuerpo, inicio, app.pistas.len()));
    frame.render_widget(Table::new(filas, anchos).header(cabecera), cuerpo);
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(
                " {}-{} de {}   orden: {} {}",
                inicio + 1,
                fin,
                miles(app.total_pistas),
                app.orden_pistas.etiqueta(),
                if app.orden_descendente { "▼" } else { "▲" }
            ),
            Style::new().fg(app.paleta.secundario),
        )),
        estado,
    );
}

fn titulo_orden(
    app: &AppEstado,
    texto: &str,
    orden: crate::biblioteca::consultas::OrdenPistas,
) -> String {
    if app.orden_pistas == orden {
        format!("{texto} {}", if app.orden_descendente { "▼" } else { "▲" })
    } else {
        texto.to_string()
    }
}
