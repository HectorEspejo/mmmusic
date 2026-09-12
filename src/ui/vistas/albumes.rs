use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{AppEstado, Foco, Pantalla};
use crate::ui::{formatear_ms, miles};

const ANCHO_TARJETA: u16 = 18;
const ALTO_TARJETA: u16 = 6;
const ANCHO_CARATULA_DETALLE: u16 = 12;
const ALTO_CARATULA_DETALLE: u16 = 6;

pub fn dibujar(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    match app.pantalla {
        Pantalla::DetalleAlbum => dibujar_detalle(frame, app, area),
        _ => dibujar_rejilla(frame, app, area),
    }
}

fn dibujar_rejilla(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    let bloque = super::panel(app, "Álbumes");
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);
    if app.albumes.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                if app.total_pistas == 0 {
                    " No hay álbumes. Pulsa Ctrl+r para escanear la biblioteca."
                } else {
                    " No hay álbumes."
                },
                Style::new().fg(app.paleta.secundario),
            )),
            interior,
        );
        return;
    }

    let columnas = (interior.width / ANCHO_TARJETA).max(1) as usize;
    app.columnas_rejilla = columnas;
    let filas_visibles = (interior.height / ALTO_TARJETA).max(1) as usize;
    let total_filas = app.albumes.len().div_ceil(columnas);
    let fila_seleccion = app.seleccion / columnas;
    let (inicio, fin) = super::ventana(total_filas, fila_seleccion, filas_visibles);

    for (desplazamiento, fila) in (inicio..fin).enumerate() {
        let y = interior.y + (desplazamiento as u16) * ALTO_TARJETA;
        for columna in 0..columnas {
            let indice = fila * columnas + columna;
            let Some(album) = app.albumes.get(indice).cloned() else {
                break;
            };
            let x = interior.x + (columna as u16) * ANCHO_TARJETA;
            let ancho = ANCHO_TARJETA.min(interior.right().saturating_sub(x));
            if ancho < 6 {
                break;
            }
            let destino = Rect {
                x,
                y,
                width: ancho,
                height: ALTO_TARJETA.min(interior.bottom().saturating_sub(y)),
            };
            if destino.height < 3 {
                break;
            }
            dibujar_tarjeta(frame, app, destino, &album, indice == app.seleccion);
        }
    }
}

fn dibujar_tarjeta(
    frame: &mut Frame,
    app: &mut AppEstado,
    area: Rect,
    album: &crate::biblioteca::modelos::AlbumResumen,
    seleccionada: bool,
) {
    let estilo_borde = if seleccionada {
        Style::new()
            .fg(app.paleta.acento)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(app.paleta.secundario)
    };
    let bloque = Block::bordered()
        .border_style(estilo_borde)
        .style(Style::new().bg(app.paleta.fondo));
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);
    if interior.height == 0 || interior.width == 0 {
        return;
    }

    let alto_caratula = interior.height.saturating_sub(2);
    let caratula = Rect {
        height: alto_caratula,
        ..interior
    };
    let album_id = album.id;
    let caratula_ruta = album.caratula_ruta.clone();
    crate::ui::componentes::imagen::dibujar(
        frame,
        app,
        caratula,
        album_id,
        caratula_ruta.as_deref(),
    );
    let texto = Rect {
        y: caratula.bottom(),
        height: interior.height.saturating_sub(alto_caratula),
        ..interior
    };
    let estilo_titulo = if seleccionada {
        Style::new()
            .fg(app.paleta.acento)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(app.paleta.texto)
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                super::truncar(&album.titulo, interior.width as usize),
                estilo_titulo,
            )),
            Line::from(Span::styled(
                super::truncar(&album.artista, interior.width as usize),
                Style::new().fg(app.paleta.secundario),
            )),
        ])
        .style(Style::new().bg(app.paleta.fondo)),
        texto,
    );
}

fn dibujar_detalle(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    let Some(detalle) = app.detalle_album.clone() else {
        return;
    };
    let bloque = super::panel(app, &detalle.album.titulo);
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);
    if interior.height < 4 || interior.width < 20 {
        return;
    }

    let alto_cabecera = (ALTO_CARATULA_DETALLE + 2).min(interior.height.saturating_sub(2));
    let cabecera = Rect {
        height: alto_cabecera,
        ..interior
    };
    let caratula = Rect {
        width: ANCHO_CARATULA_DETALLE.min(interior.width / 3),
        ..cabecera
    };
    let bloque_caratula = Block::bordered()
        .border_style(Style::new().fg(app.paleta.secundario))
        .style(Style::new().bg(app.paleta.fondo));
    let interior_caratula = bloque_caratula.inner(caratula);
    frame.render_widget(bloque_caratula, caratula);
    let album_id = detalle.album.id;
    let caratula_ruta = detalle.album.caratula_ruta.clone();
    let zona_imagen = Rect {
        height: interior_caratula.height,
        ..interior_caratula
    };
    crate::ui::componentes::imagen::dibujar(
        frame,
        app,
        zona_imagen,
        album_id,
        caratula_ruta.as_deref(),
    );

    let info = Rect {
        x: caratula.right() + 1,
        width: interior.right().saturating_sub(caratula.right() + 1),
        ..cabecera
    };
    let formato = detalle
        .pistas
        .first()
        .map(|pista| pista.formato.to_uppercase())
        .unwrap_or_else(|| "—".to_string());
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                detalle.album.titulo.clone(),
                Style::new()
                    .fg(app.paleta.acento)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!(
                    "{} · {} · {} pistas · {} · {}",
                    detalle.album.artista,
                    detalle
                        .album
                        .anio
                        .map(|anio| anio.to_string())
                        .unwrap_or_else(|| "—".to_string()),
                    miles(detalle.album.num_pistas),
                    formatear_ms(detalle.album.duracion_ms),
                    formato
                ),
                Style::new().fg(app.paleta.secundario),
            )),
        ])
        .style(Style::new().bg(app.paleta.fondo)),
        info,
    );

    let cuerpo = Rect {
        y: cabecera.bottom(),
        height: interior.bottom().saturating_sub(cabecera.bottom()),
        ..interior
    };
    if detalle.pistas.is_empty() || cuerpo.height == 0 {
        return;
    }
    let id_sonando = app
        .estado_reproductor
        .pista_actual
        .as_ref()
        .map(|pista| pista.id);
    let (inicio, fin) = super::ventana(detalle.pistas.len(), app.seleccion, cuerpo.height as usize);
    let lineas: Vec<Line> = detalle.pistas[inicio..fin]
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
            let numero = pista
                .numero_pista
                .map(|numero| numero.to_string())
                .unwrap_or_else(|| "·".to_string());
            Line::from(vec![
                Span::styled(
                    format!(
                        " {} {:>3}  ",
                        if sonando { app.iconos.reproducida } else { " " },
                        numero
                    ),
                    estilo,
                ),
                Span::styled(super::truncar(&pista.titulo, 44), estilo),
                Span::styled(
                    format!("   {}", formatear_ms(pista.duracion_ms)),
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
        cuerpo,
    );
}
