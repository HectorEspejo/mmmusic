use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::AppEstado;
use crate::letras::{Fuente, Letra, Resolucion};
use crate::ui::vistas::panel;

pub fn dibujar(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    let bloque = panel(app, "Letras");
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);
    if interior.height == 0 || interior.width == 0 {
        return;
    }
    let [cabecera, cuerpo, pie] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(interior);
    dibujar_cabecera(frame, app, cabecera);
    dibujar_cuerpo(frame, app, cuerpo);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            ayuda(app),
            Style::new().fg(app.paleta.secundario),
        ))),
        pie,
    );
}

fn dibujar_cabecera(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let titulo = if app.estado_reproductor.es_emisora() {
        app.estado_reproductor
            .emisora_actual()
            .map(|emisora| format!(" {}", emisora.nombre))
            .unwrap_or_else(|| " (emisora)".to_string())
    } else {
        app.estado_reproductor
            .pista_actual()
            .map(|pista| format!(" {} · {}", pista.titulo, pista.artista))
            .unwrap_or_else(|| " (sin pista)".to_string())
    };
    let mut spans = vec![Span::styled(
        titulo,
        Style::new()
            .fg(app.paleta.texto)
            .add_modifier(Modifier::BOLD),
    )];
    if let Some((disponibles, activa)) = fuentes_disponibles(app) {
        let mut indicador = vec![Span::raw("  ")];
        for (indice, fuente) in disponibles.iter().enumerate() {
            if indice > 0 {
                indicador.push(Span::styled(" ▸ ", Style::new().fg(app.paleta.secundario)));
            }
            let elegida = Some(*fuente) == activa;
            indicador.push(Span::styled(
                fuente.como_str(),
                if elegida {
                    Style::new()
                        .fg(app.paleta.acento)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(app.paleta.secundario)
                },
            ));
        }
        let ancho_titulo = spans.iter().map(Span::width).sum::<usize>();
        let ancho_indicador = indicador.iter().map(Span::width).sum::<usize>();
        if ancho_titulo + ancho_indicador + 2 < area.width as usize {
            spans.extend(indicador);
        }
    }
    let estado = estado_texto(app);
    let mut lineas = vec![Line::from(spans)];
    if !estado.is_empty() {
        lineas.push(Line::from(Span::styled(
            estado,
            Style::new().fg(app.paleta.secundario),
        )));
    }
    frame.render_widget(Paragraph::new(lineas), area);
}

fn fuentes_disponibles(app: &AppEstado) -> Option<(Vec<Fuente>, Option<Fuente>)> {
    let pista_id = app.letras.pista_id?;
    let resolucion: &Resolucion = app.letras.cache.get(&pista_id)?;
    let fuentes = resolucion.fuentes();
    if fuentes.is_empty() {
        return None;
    }
    Some((fuentes, app.letras.letra.fuente()))
}

fn estado_texto(app: &AppEstado) -> String {
    match &app.letras.letra {
        Letra::Sincronizada { .. } => {
            let manual = app.letras.manual_hasta.is_some();
            let sufijo = if manual {
                " · j/k desplaza; 5 s sin teclas vuelven al seguimiento"
            } else {
                " · línea actual centrada"
            };
            format!(
                " {} · offset {:+} ms{sufijo}",
                if manual { "manual" } else { "siguiendo" },
                app.letras.offset_usuario_ms
            )
        }
        Letra::Estatica { .. } => " letra estática (sin sincronía)".to_string(),
        Letra::Ninguna { .. } => String::new(),
    }
}

fn ayuda(app: &AppEstado) -> &'static str {
    match app.letras.letra {
        Letra::Sincronizada { .. } => {
            " j/k desplazar · Enter saltar · ( ) offset · s fuente · gg/G inicio/fin · ? ayuda"
        }
        Letra::Estatica { .. } => " j/k desplazar · gg/G inicio/fin · s fuente · ? ayuda",
        Letra::Ninguna { .. } => " Coloca un .lrc/.txt junto a la pista · ? ayuda",
    }
}

fn dibujar_cuerpo(frame: &mut Frame, app: &AppEstado, area: Rect) {
    if app.estado_reproductor.es_emisora() {
        mensaje(
            frame,
            app,
            area,
            " No disponible para emisoras",
            &["Las letras solo aplican a pistas locales."],
        );
        return;
    }
    if app.letras.cargando {
        mensaje(frame, app, area, " Cargando letra…", &[]);
        return;
    }
    match &app.letras.letra {
        Letra::Ninguna { .. } => dibujar_sin_letra(frame, app, area),
        Letra::Estatica { lineas, .. } => dibujar_estatica(frame, app, area, lineas),
        Letra::Sincronizada { lineas, .. } => {
            dibujar_sincronizada(frame, app, area, lineas, app.linea_actual_letras())
        }
    }
}

fn mensaje(frame: &mut Frame, app: &AppEstado, area: Rect, titulo: &str, extra: &[&str]) {
    let mut lineas = vec![
        Line::from(""),
        Line::from(Span::styled(
            titulo.to_string(),
            Style::new()
                .fg(app.paleta.texto)
                .add_modifier(Modifier::BOLD),
        )),
    ];
    for texto in extra {
        lineas.push(Line::from(Span::styled(
            texto.to_string(),
            Style::new().fg(app.paleta.secundario),
        )));
    }
    frame.render_widget(Paragraph::new(lineas).alignment(Alignment::Center), area);
}

fn dibujar_sin_letra(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let mut lineas = vec![
        Line::from(""),
        Line::from(Span::styled(
            " Sin letra para esta pista",
            Style::new()
                .fg(app.paleta.texto)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " mmmusic busca, por este orden:",
            Style::new().fg(app.paleta.secundario),
        )),
    ];
    for (indice, ruta) in app.letras.rutas.iter().enumerate() {
        lineas.push(Line::from(Span::styled(
            format!(" {}. {ruta}", indice + 1),
            Style::new().fg(app.paleta.secundario),
        )));
    }
    lineas.push(Line::from(Span::styled(
        format!(
            " {}. Etiqueta de letra embebida (USLT/LYRICS)",
            app.letras.rutas.len() + 1
        ),
        Style::new().fg(app.paleta.secundario),
    )));
    frame.render_widget(Paragraph::new(lineas), area);
}

fn dibujar_estatica(frame: &mut Frame, app: &AppEstado, area: Rect, lineas: &[String]) {
    let alto = area.height as usize;
    if lineas.is_empty() {
        return;
    }
    let inicio = app
        .letras
        .seleccion
        .saturating_sub(alto / 2)
        .min(lineas.len().saturating_sub(alto));
    let visibles: Vec<Line> = lineas[inicio..(inicio + alto).min(lineas.len())]
        .iter()
        .map(|texto| {
            Line::from(Span::styled(
                format!(" {texto}"),
                Style::new().fg(app.paleta.texto),
            ))
        })
        .collect();
    frame.render_widget(Paragraph::new(visibles), area);
}

fn dibujar_sincronizada(
    frame: &mut Frame,
    app: &AppEstado,
    area: Rect,
    lineas: &[(u32, String)],
    actual: i64,
) {
    let alto = area.height as usize;
    if lineas.is_empty() || alto == 0 {
        return;
    }
    let manual = app.letras.manual_hasta.is_some();
    let centro = if manual {
        app.letras.seleccion
    } else {
        actual.max(0) as usize
    };
    let inicio = centro
        .saturating_sub(alto / 2)
        .min(lineas.len().saturating_sub(alto));
    let mut visibles = Vec::new();
    for (indice, (_, texto)) in lineas.iter().enumerate().skip(inicio).take(alto) {
        let es_actual = indice as i64 == actual;
        let es_seleccion = manual && indice == app.letras.seleccion;
        let prefijo = if es_actual { "▶ " } else { "  " };
        let mut estilo = if es_actual {
            Style::new()
                .fg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.secundario)
        };
        if es_seleccion {
            estilo = estilo.add_modifier(Modifier::REVERSED);
        }
        visibles.push(Line::from(Span::styled(
            format!("{prefijo}{texto}"),
            estilo,
        )));
    }
    frame.render_widget(Paragraph::new(visibles).alignment(Alignment::Center), area);
}
