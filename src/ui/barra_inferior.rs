use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use crate::app::AppEstado;
use crate::biblioteca::modelos::ElementoCola;
use crate::reproductor::estado::{EstadoStream, Repeticion};
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
    let es_emisora = estado.es_emisora();
    let (titulo, subtitulo, album_id, ruta_imagen, id_logo) = match &estado.elemento {
        Some(ElementoCola::Pista(pista)) => {
            let titulo = if app.favoritas.contains(&pista.id) {
                format!("{} {}", app.iconos.corazon, pista.titulo)
            } else {
                pista.titulo.clone()
            };
            (
                titulo,
                format!("{} · {}", pista.artista, pista.album),
                Some(pista.album_id),
                pista.caratula_ruta.clone(),
                None,
            )
        }
        Some(ElementoCola::Emisora(emisora)) => (
            emisora.nombre.clone(),
            String::new(),
            None,
            emisora.logo_ruta.clone(),
            Some(emisora.id),
        ),
        None => ("(detenido)".to_string(), String::new(), None, None, None),
    };

    let linea_controles = controles(app, indicador_scrobbling(app, compacto_ancho));
    let ancho_controles = (linea_controles.width() as u16 + 3).min(interior.width / 2);

    if compacto {
        let [fila] = Layout::vertical([Constraint::Length(1)]).areas(interior);
        let [info, controles] =
            Layout::horizontal([Constraint::Min(5), Constraint::Length(ancho_controles)])
                .areas(fila);
        let icono = if es_emisora { icono_directo(app) } else { "" };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {icono} {titulo}"),
                estilo.add_modifier(Modifier::BOLD),
            ))),
            info,
        );
        frame.render_widget(Paragraph::new(linea_controles), controles);
        return;
    }

    let [zona_caratula, resto] =
        Layout::horizontal([Constraint::Length(4), Constraint::Min(1)]).areas(interior);
    if let Some(id_logo) = id_logo {
        let destino = Rect {
            height: zona_caratula.height.min(2),
            width: zona_caratula.width.saturating_sub(1).max(2),
            ..zona_caratula
        };
        imagen::dibujar_logo(frame, app, destino, id_logo, ruta_imagen.as_deref());
    } else if let Some(album_id) = album_id {
        let destino = Rect {
            height: zona_caratula.height.min(2),
            width: zona_caratula.width.saturating_sub(1).max(2),
            ..zona_caratula
        };
        imagen::dibujar(frame, app, destino, album_id, ruta_imagen.as_deref());
    }
    let [fila_info, fila_progreso] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(resto);
    let mostrar_mini = app.mini_espectro_visible();
    let ancho_mini = if mostrar_mini {
        crate::visuales::mini_espectro::BARRAS as u16 + 1
    } else {
        0
    };
    let [info, zona_mini, controles] = Layout::horizontal([
        Constraint::Min(10),
        Constraint::Length(ancho_mini),
        Constraint::Length(ancho_controles),
    ])
    .areas(fila_info);
    if mostrar_mini && zona_mini.width > 0 {
        frame.render_widget(
            Paragraph::new(crate::visuales::mini_espectro::linea(
                &app.analisis,
                &app.paleta_visual,
            ))
            .style(Style::new().bg(app.paleta.fondo)),
            zona_mini,
        );
    }
    if es_emisora {
        let icono = icono_directo(app);
        let estado_texto = estado_stream_texto(&estado);
        let titulo_icy = estado.titulo_icy.clone().unwrap_or_default();
        let cabecera = format!(" {icono} {titulo}");
        let detalle = if titulo_icy.is_empty() {
            format!(" {estado_texto}")
        } else {
            format!(" {titulo_icy}   {estado_texto}")
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(cabecera, estilo.add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(
                    detalle,
                    Style::new().fg(app.paleta.secundario).bg(app.paleta.fondo),
                )),
            ]),
            info,
        );
    } else {
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
    }
    frame.render_widget(Paragraph::new(linea_controles), controles);

    if es_emisora {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" {}", estado_stream_texto(&estado)),
                Style::new().fg(app.paleta.acento).bg(app.paleta.fondo),
            )),
            fila_progreso,
        );
        return;
    }

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

fn icono_directo(app: &AppEstado) -> &'static str {
    match &app.estado_reproductor.stream {
        Some(EstadoStream::EnDirecto) => app.iconos.directo,
        Some(EstadoStream::Conectando) | Some(EstadoStream::Reconectando(_)) => {
            app.iconos.reconectando
        }
        _ => app.iconos.almacenando,
    }
}

fn estado_stream_texto(estado: &crate::reproductor::estado::EstadoReproduccion) -> String {
    let codec = estado
        .codec
        .as_deref()
        .map(str::to_uppercase)
        .unwrap_or_default();
    match &estado.stream {
        Some(EstadoStream::Conectando) => "conectando…".to_string(),
        Some(EstadoStream::Almacenando { segundos }) => {
            format!("almacenando {segundos:.1} s")
        }
        Some(EstadoStream::EnDirecto) => {
            let tiempo = formatear_directo(estado.tiempo_escuchando_ms);
            if codec.is_empty() {
                format!("EN DIRECTO · {tiempo}")
            } else {
                format!("EN DIRECTO · {tiempo} · {codec}")
            }
        }
        Some(EstadoStream::Reconectando(intento)) => {
            format!("reconectando ({intento}/6)…")
        }
        Some(EstadoStream::Rendido) => "sin conexión".to_string(),
        None => "detenido".to_string(),
    }
}

fn formatear_directo(ms: i64) -> String {
    let total = (ms.max(0) / 1000) as u64;
    let horas = total / 3600;
    let minutos = (total % 3600) / 60;
    let segundos = total % 60;
    format!("{horas:02}:{minutos:02}:{segundos:02}")
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
