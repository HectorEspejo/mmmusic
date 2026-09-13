use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::app::AppEstado;
use crate::config::ModoIconos;
use crate::ecualizador::cadena::BANDAS_HZ;
use crate::ecualizador::replaygain::ModoReplayGain;
use crate::ecualizador::{MAX_DB, MIN_DB};

pub const ANCHO_COMPACTO: u16 = 60;
/// Filas reservadas para valores, etiquetas, marca, presets y leyenda.
const RESERVA_VERTICAL: u16 = 7;

pub fn dibujar(frame: &mut Frame, app: &AppEstado, area: Rect) {
    if area.width < 20 || area.height < 4 {
        return;
    }
    frame.render_widget(Clear, area);
    let bloque = Block::bordered()
        .title(format!(" {} ", cabecera(app)))
        .border_style(Style::new().fg(app.paleta.acento).bg(app.paleta.fondo))
        .style(Style::new().fg(app.paleta.texto).bg(app.paleta.fondo));
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);
    if interior.height == 0 || interior.width == 0 {
        return;
    }
    let ascii = app.config.interfaz.iconos == ModoIconos::Ascii;
    let compacto = area.width < ANCHO_COMPACTO;
    let mut lineas = Vec::new();
    if !app.estado_reproductor.eq.disponible {
        lineas.push(Line::from(Span::styled(
            " Ecualizador no disponible: esta build de mpv no trae lavfi.",
            Style::new().fg(app.paleta.aviso),
        )));
        lineas.push(Line::from(Span::styled(
            " ReplayGain sigue funcionando.",
            Style::new().fg(app.paleta.secundario),
        )));
    } else if compacto {
        lineas.extend(lineas_compacto(app));
    } else {
        lineas.extend(lineas_barras(app, interior, ascii));
    }
    lineas.push(Line::from(""));
    lineas.extend(lineas_presets(app, interior.width as usize, ascii));
    lineas.push(Line::from(Span::styled(
        if compacto {
            " h/l banda · j/k ±1 · J/K ±0.5 · 0 reset · R plano · Tab presets"
        } else {
            " h/l banda · j/k ±1 dB · J/K ±0,5 dB · 0 reset · R plano · Tab presets"
        },
        Style::new().fg(app.paleta.secundario),
    )));
    lineas.push(Line::from(Span::styled(
        " N guardar · D borrar · e EQ on/off · x limitador · g ReplayGain · Esc cerrar",
        Style::new().fg(app.paleta.secundario),
    )));
    frame.render_widget(Paragraph::new(lineas), interior);
}

fn cabecera(app: &AppEstado) -> String {
    let eq = &app.estado_reproductor.eq;
    let preset = eq
        .preset
        .as_ref()
        .map(|(_, nombre)| nombre.as_str())
        .unwrap_or("personalizado");
    let rg = match eq.replaygain {
        ModoReplayGain::No => "no",
        ModoReplayGain::Pista => "pista",
        ModoReplayGain::Album => "álbum",
    };
    let disponibilidad = if eq.disponible { "" } else { " · sin lavfi" };
    format!(
        "Ecualizador · {preset} · EQ {} · lim {} · RG: {rg}{disponibilidad}",
        if eq.activo { "on" } else { "off" },
        if eq.limitador { "on" } else { "off" },
    )
}

fn valores(app: &AppEstado) -> [f32; 11] {
    let eq = &app.estado_reproductor.eq;
    let mut valores = [0.0; 11];
    valores[0] = eq.preamp_db;
    valores[1..].copy_from_slice(&eq.ganancias);
    valores
}

fn etiquetas() -> [String; 11] {
    let mut etiquetas = std::array::from_fn(|indice| {
        let hz = BANDAS_HZ[indice];
        if hz >= 1000 {
            format!("{}k", hz / 1000)
        } else {
            hz.to_string()
        }
    });
    etiquetas[0] = "pre".to_string();
    etiquetas
}

fn lineas_barras(app: &AppEstado, interior: Rect, ascii: bool) -> Vec<Line<'static>> {
    let valores = valores(app);
    let glifo = if ascii { '#' } else { '█' };
    let filas = interior.height.saturating_sub(RESERVA_VERTICAL).max(3) as usize;
    let cero = filas / 2;
    let ancho = (interior.width as usize / 11).clamp(3, 7);
    let ancho_bloque = ancho.saturating_sub(1).max(1);
    let mut lineas = Vec::with_capacity(filas + 3);
    for fila in 0..filas {
        let mut spans = vec![Span::styled(" ", Style::new())];
        for (indice, valor) in valores.iter().enumerate() {
            let pintar = pintar_en_fila(*valor, fila, cero);
            let columna = if pintar {
                let mut columna = String::with_capacity(ancho);
                for _ in 0..ancho_bloque {
                    columna.push(glifo);
                }
                columna.push(' ');
                columna
            } else {
                " ".repeat(ancho)
            };
            let color = if indice == app.eq_banda {
                app.paleta.acento
            } else {
                app.paleta.texto
            };
            spans.push(Span::styled(columna, Style::new().fg(color)));
        }
        lineas.push(Line::from(spans));
    }
    let etiquetas = etiquetas();
    let mut valores_linea = vec![Span::styled(" ", Style::new())];
    let mut etiquetas_linea = vec![Span::styled(" ", Style::new())];
    let mut marca_linea = vec![Span::styled(" ", Style::new())];
    for (indice, etiqueta) in etiquetas.iter().enumerate() {
        let seleccionada = indice == app.eq_banda;
        let estilo = if seleccionada {
            Style::new()
                .fg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.texto)
        };
        valores_linea.push(Span::styled(
            centrar(&format!("{:+.1}", valores[indice]), ancho),
            estilo,
        ));
        etiquetas_linea.push(Span::styled(
            centrar(etiqueta, ancho),
            Style::new().fg(app.paleta.secundario),
        ));
        marca_linea.push(Span::styled(
            centrar(
                if seleccionada {
                    if ascii { "^" } else { "▲" }
                } else {
                    ""
                },
                ancho,
            ),
            Style::new().fg(app.paleta.acento),
        ));
    }
    lineas.push(Line::from(""));
    lineas.push(Line::from(valores_linea));
    lineas.push(Line::from(etiquetas_linea));
    lineas.push(Line::from(marca_linea));
    lineas
}

fn pintar_en_fila(valor: f32, fila: usize, cero: usize) -> bool {
    let valor = valor.clamp(MIN_DB, MAX_DB);
    if valor > 0.0 {
        let k = ((valor / MAX_DB) * cero as f32).round() as usize;
        fila + k >= cero && fila < cero
    } else if valor < 0.0 {
        let k = ((-valor / -MIN_DB) * cero as f32).round() as usize;
        fila >= cero && fila < cero + k
    } else {
        false
    }
}

fn lineas_compacto(app: &AppEstado) -> Vec<Line<'static>> {
    let valores = valores(app);
    let etiquetas = etiquetas();
    let estilo_valor = Style::new().fg(app.paleta.texto);
    let estilo_acento = Style::new()
        .fg(app.paleta.acento)
        .add_modifier(Modifier::BOLD);
    let mut primera = vec![Span::styled(" ", Style::new())];
    let mut segunda = vec![Span::styled(" ", Style::new())];
    for indice in 0..11 {
        let destino = if indice < 6 {
            &mut primera
        } else {
            &mut segunda
        };
        let valor = format!("{} {:+.1}  ", etiquetas[indice], valores[indice]);
        if indice == app.eq_banda {
            destino.push(Span::styled(valor, estilo_acento));
        } else {
            destino.push(Span::styled(valor, estilo_valor));
        }
    }
    vec![Line::from(primera), Line::from(segunda)]
}

fn lineas_presets(app: &AppEstado, ancho: usize, ascii: bool) -> Vec<Line<'static>> {
    let eq = &app.estado_reproductor.eq;
    let mut lineas = vec![Line::from(Span::styled(
        " Presets",
        Style::new().fg(app.paleta.secundario),
    ))];
    let diamante = if ascii { '*' } else { '♦' };
    let mut actual = vec![Span::styled(" ", Style::new())];
    let mut longitud = 1;
    for (indice, preset) in app.presets_eq.iter().enumerate() {
        let seleccionado = app.eq_foco_presets && indice == app.eq_seleccion_preset;
        let activo = eq.preset.as_ref().is_some_and(|(id, _)| *id == preset.id);
        let prefijo = if preset.integrado {
            String::new()
        } else {
            format!("{diamante} ")
        };
        let texto = format!("{prefijo}{}", preset.nombre);
        let estilo = if seleccionado {
            Style::new()
                .fg(app.paleta.fondo)
                .bg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else if activo {
            Style::new()
                .fg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.texto)
        };
        let ancho_entrada = texto.chars().count() + 2;
        if longitud + ancho_entrada > ancho.saturating_sub(1) && longitud > 1 {
            lineas.push(Line::from(std::mem::take(&mut actual)));
            actual.push(Span::styled(" ", Style::new()));
            longitud = 1;
        }
        actual.push(Span::styled(format!(" {texto} "), estilo));
        longitud += ancho_entrada;
    }
    lineas.push(Line::from(actual));
    lineas
}

fn centrar(texto: &str, ancho: usize) -> String {
    let longitud = texto.chars().count();
    if longitud > ancho {
        return texto.chars().take(ancho).collect();
    }
    if longitud == ancho {
        return texto.to_string();
    }
    let relleno = ancho - longitud;
    let izquierda = relleno / 2;
    let derecha = relleno - izquierda;
    format!("{}{}{}", " ".repeat(izquierda), texto, " ".repeat(derecha))
}
