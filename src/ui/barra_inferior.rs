use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use crate::app::AppEstado;
use crate::reproductor::estado::Repeticion;
use crate::ui::componentes::imagen;
use crate::ui::formatear_ms;

const ANCHO_COMPACTO: u16 = 70;

pub fn dibujar(frame: &mut Frame, app: &mut AppEstado, area: Rect, compacto: bool) {
    let estilo = Style::new().fg(app.paleta.texto).bg(app.paleta.fondo);
    let bloque = Block::new()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(app.paleta.secundario).bg(app.paleta.fondo))
        .style(estilo);
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);
    if interior.height == 0 || interior.width < 10 {
        return;
    }
    let compacto_ancho = area.width < ANCHO_COMPACTO;

    let estado = app.estado_reproductor.clone();
    let pista = estado.pista_actual.as_ref();
    let titulo = pista
        .map(|p| p.titulo.clone())
        .unwrap_or_else(|| "(detenido)".to_string());
    let es_favorita = pista.is_some_and(|p| app.favoritas.contains(&p.id));
    let titulo = if es_favorita {
        format!("{} {titulo}", app.iconos.corazon)
    } else {
        titulo
    };
    let subtitulo = pista
        .map(|p| format!("{} · {}", p.artista, p.album))
        .unwrap_or_default();
    let album_id = pista.map(|p| p.album_id);

    let linea_controles = controles(app, indicador_scrobbling(app, compacto_ancho));
    let ancho_controles = (linea_controles.width() as u16 + 3).min(interior.width / 2);

    if compacto {
        let [fila] = Layout::vertical([Constraint::Length(1)]).areas(interior);
        let [info, controles] =
            Layout::horizontal([Constraint::Min(5), Constraint::Length(ancho_controles)])
                .areas(fila);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {titulo}"),
                estilo.add_modifier(Modifier::BOLD),
            ))),
            info,
        );
        frame.render_widget(Paragraph::new(linea_controles), controles);
        return;
    }

    let [zona_caratula, resto] =
        Layout::horizontal([Constraint::Length(4), Constraint::Min(1)]).areas(interior);
    if let Some(album_id) = album_id {
        let ruta = estado
            .pista_actual
            .as_ref()
            .and_then(|p| p.caratula_ruta.clone());
        let destino = Rect {
            height: zona_caratula.height.min(2),
            width: zona_caratula.width.saturating_sub(1).max(2),
            ..zona_caratula
        };
        imagen::dibujar(frame, app, destino, album_id, ruta.as_deref());
    }
    let [fila_info, fila_progreso] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(resto);
    let [info, controles] =
        Layout::horizontal([Constraint::Min(10), Constraint::Length(ancho_controles)])
            .areas(fila_info);
    let parrafo_info = Paragraph::new(vec![
        Line::from(Span::styled(
            format!(" {titulo}"),
            estilo.add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            if subtitulo.is_empty() {
                String::new()
            } else {
                format!(" {subtitulo}")
            },
            Style::new().fg(app.paleta.secundario).bg(app.paleta.fondo),
        )),
    ])
    .style(estilo);
    frame.render_widget(parrafo_info, info);
    frame.render_widget(Paragraph::new(linea_controles), controles);

    let [posicion, barra, duracion] = Layout::horizontal([
        Constraint::Length(6),
        Constraint::Min(4),
        Constraint::Length(6),
    ])
    .areas(fila_progreso);
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(" {:>5}", formatear_ms(estado.posicion_ms)),
            Style::new().fg(app.paleta.secundario).bg(app.paleta.fondo),
        )),
        posicion,
    );
    app.zonas.barra_progreso = barra;
    let gauge = Gauge::default()
        .ratio(estado.progreso())
        .gauge_style(
            Style::new()
                .fg(app.paleta.progreso)
                .bg(app.paleta.secundario),
        )
        .style(Style::new().bg(app.paleta.fondo));
    frame.render_widget(gauge, barra);
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("{:<5} ", formatear_ms(estado.duracion_ms)),
            Style::new().fg(app.paleta.secundario).bg(app.paleta.fondo),
        )),
        duracion,
    );
}

fn controles(app: &AppEstado, indicador: Option<Span<'static>>) -> Line<'static> {
    let estado = &app.estado_reproductor;
    let icono_estado = if estado.sonando() {
        app.iconos.pausa
    } else {
        app.iconos.reproducir
    };
    let icono_repeticion = match estado.repeticion {
        Repeticion::No => app.iconos.repeticion_no,
        Repeticion::Todo => app.iconos.repeticion_todo,
        Repeticion::Una => app.iconos.repeticion_una,
    };
    let color_activo = app.paleta.acento;
    let color_inactivo = app.paleta.secundario;
    let estilo_aleatorio = if estado.aleatorio {
        Style::new().fg(color_activo)
    } else {
        Style::new().fg(color_inactivo)
    };
    let estilo_repeticion = if estado.repeticion == Repeticion::No {
        Style::new().fg(color_inactivo)
    } else {
        Style::new().fg(color_activo)
    };
    let icono_volumen = if estado.silencio {
        app.iconos.silencio
    } else {
        app.iconos.volumen
    };
    let mut spans = vec![
        Span::styled(
            format!("{} {}", app.iconos.anterior, icono_estado),
            Style::new().fg(app.paleta.texto),
        ),
        Span::styled(
            format!(" {}", app.iconos.siguiente),
            Style::new().fg(app.paleta.texto),
        ),
        Span::styled("  ", Style::new()),
        Span::styled(app.iconos.aleatorio, estilo_aleatorio),
        Span::styled(" ", Style::new()),
        Span::styled(icono_repeticion, estilo_repeticion),
        Span::styled(
            format!("  {} {}%", icono_volumen, estado.volumen),
            Style::new().fg(app.paleta.texto),
        ),
    ];
    if let Some(indicador) = indicador {
        spans.push(Span::styled("  ", Style::new()));
        spans.push(indicador);
    }
    Line::from(spans)
}

fn indicador_scrobbling(app: &AppEstado, compacto_ancho: bool) -> Option<Span<'static>> {
    let estado = &app.estado_scrobbling;
    if !estado.activo() {
        return None;
    }
    if estado.error().is_some() {
        return Some(Span::styled(
            app.iconos.scrobbling_error,
            Style::new()
                .fg(app.paleta.error)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if compacto_ancho {
        return None;
    }
    if estado.pendientes() > 0 {
        return Some(Span::styled(
            format!("{}{}", app.iconos.scrobbling_pendiente, estado.pendientes()),
            Style::new().fg(app.paleta.aviso),
        ));
    }
    estado.ultimo_envio().map(|_| {
        Span::styled(
            app.iconos.scrobbling_enviado,
            Style::new().fg(app.paleta.progreso),
        )
    })
}
