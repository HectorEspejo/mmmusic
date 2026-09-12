use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{AppEstado, Foco};
use crate::biblioteca::modelos::{AlbumResumen, PistaListado};
use crate::ui::componentes::imagen;
use crate::ui::formatear_ms;

const ANCHO_TARJETA: u16 = 16;
const ALTO_MINIMO_TARJETAS: u16 = 20;

struct TarjetaInicio {
    album_id: i64,
    caratula_ruta: Option<String>,
    titulo: String,
    subtitulo: String,
    indice_flat: usize,
}

pub fn dibujar(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    let bloque = super::panel(app, "Inicio");
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);
    if app.config.interfaz.caratulas && area.height >= ALTO_MINIMO_TARJETAS {
        dibujar_tarjetas(frame, app, interior);
    } else {
        dibujar_texto(frame, app, interior);
    }
}

fn dibujar_tarjetas(frame: &mut Frame, app: &mut AppEstado, interior: Rect) {
    if interior.height < 6 || interior.width < 8 {
        dibujar_texto(frame, app, interior);
        return;
    }
    let alturas = repartir_alto(interior.height);
    let mut y = interior.y;
    for (indice_bloque, altura) in alturas.iter().enumerate() {
        let zona = Rect {
            y,
            height: *altura,
            ..interior
        };
        dibujar_bloque(frame, app, zona, indice_bloque);
        y += altura;
    }
}

fn repartir_alto(alto: u16) -> [u16; 3] {
    let base = alto / 3;
    let resto = alto % 3;
    let mut alturas = [base; 3];
    for altura in alturas.iter_mut().take(resto as usize) {
        *altura += 1;
    }
    alturas
}

fn dibujar_bloque(frame: &mut Frame, app: &mut AppEstado, zona: Rect, indice_bloque: usize) {
    let enfocado = app.bloque_inicio == indice_bloque && app.foco == Foco::Contenido;
    let estilo_titulo = if enfocado {
        Style::new()
            .fg(app.paleta.acento)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(app.paleta.secundario)
    };
    let cabecera = Rect { height: 1, ..zona };
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(" {}", titulo_bloque(indice_bloque)),
            estilo_titulo,
        )),
        cabecera,
    );
    let cuerpo = Rect {
        y: zona.y + 1,
        height: zona.height.saturating_sub(1),
        ..zona
    };
    if cuerpo.height < 3 {
        return;
    }
    let tarjetas = tarjetas_del_bloque(app, indice_bloque);
    if tarjetas.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                mensaje_vacio(app, indice_bloque),
                Style::new().fg(app.paleta.secundario),
            )),
            cuerpo,
        );
        return;
    }
    dibujar_fila_tarjetas(frame, app, cuerpo, &tarjetas);
}

fn titulo_bloque(indice: usize) -> &'static str {
    match indice {
        0 => "Reproducidas recientemente",
        1 => "Añadidos recientemente",
        _ => "Redescubre",
    }
}

fn mensaje_vacio(app: &AppEstado, indice: usize) -> &'static str {
    match indice {
        0 => "  (sin reproducciones todavía)",
        1 if app.total_pistas == 0 => "  (biblioteca vacía; pulsa Ctrl+r para escanear)",
        1 => "  (sin álbumes añadidos)",
        _ => "  (álbumes al azar de tu biblioteca)",
    }
}

fn tarjetas_del_bloque(app: &AppEstado, indice: usize) -> Vec<TarjetaInicio> {
    match indice {
        0 => app
            .inicio
            .recientes
            .iter()
            .take(10)
            .enumerate()
            .map(|(posicion, pista)| tarjeta_de_pista(pista, posicion))
            .collect(),
        1 => app
            .inicio
            .anadidos
            .iter()
            .take(10)
            .enumerate()
            .map(|(posicion, album)| tarjeta_de_album(album, app.inicio.recientes.len() + posicion))
            .collect(),
        _ => app
            .inicio
            .redescubre
            .iter()
            .take(10)
            .enumerate()
            .map(|(posicion, album)| {
                tarjeta_de_album(
                    album,
                    app.inicio.recientes.len() + app.inicio.anadidos.len() + posicion,
                )
            })
            .collect(),
    }
}

fn tarjeta_de_pista(pista: &PistaListado, indice_flat: usize) -> TarjetaInicio {
    TarjetaInicio {
        album_id: pista.album_id,
        caratula_ruta: pista.caratula_ruta.clone(),
        titulo: pista.titulo.clone(),
        subtitulo: format!("{} · {}", pista.artista, formatear_ms(pista.duracion_ms)),
        indice_flat,
    }
}

fn tarjeta_de_album(album: &AlbumResumen, indice_flat: usize) -> TarjetaInicio {
    TarjetaInicio {
        album_id: album.id,
        caratula_ruta: album.caratula_ruta.clone(),
        titulo: album.titulo.clone(),
        subtitulo: album.artista.clone(),
        indice_flat,
    }
}

fn dibujar_fila_tarjetas(
    frame: &mut Frame,
    app: &mut AppEstado,
    area: Rect,
    tarjetas: &[TarjetaInicio],
) {
    let visibles = (area.width / ANCHO_TARJETA).max(1) as usize;
    let columna = app.columna_inicio.min(tarjetas.len().saturating_sub(1));
    let inicio = columna
        .saturating_sub(visibles / 2)
        .min(tarjetas.len().saturating_sub(visibles));
    for (desplazamiento, tarjeta) in tarjetas[inicio..].iter().take(visibles).enumerate() {
        let x = area.x + desplazamiento as u16 * ANCHO_TARJETA;
        let ancho = ANCHO_TARJETA.min(area.right().saturating_sub(x));
        if ancho < 6 {
            break;
        }
        let destino = Rect {
            x,
            width: ancho,
            height: area.height,
            ..area
        };
        dibujar_tarjeta(
            frame,
            app,
            destino,
            tarjeta,
            tarjeta.indice_flat == app.seleccion,
        );
    }
}

fn dibujar_tarjeta(
    frame: &mut Frame,
    app: &mut AppEstado,
    area: Rect,
    tarjeta: &TarjetaInicio,
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
    imagen::dibujar(
        frame,
        app,
        caratula,
        tarjeta.album_id,
        tarjeta.caratula_ruta.as_deref(),
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
                super::truncar(&tarjeta.titulo, interior.width as usize),
                estilo_titulo,
            )),
            Line::from(Span::styled(
                super::truncar(&tarjeta.subtitulo, interior.width as usize),
                Style::new().fg(app.paleta.secundario),
            )),
        ])
        .style(Style::new().bg(app.paleta.fondo)),
        texto,
    );
}

fn dibujar_texto(frame: &mut Frame, app: &AppEstado, interior: Rect) {
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
        let estilo = if indice == app.seleccion && app.foco == Foco::Contenido {
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
    if indice == app.seleccion && app.foco == Foco::Contenido {
        Style::new()
            .fg(app.paleta.fondo)
            .bg(app.paleta.acento)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(app.paleta.texto)
    }
}

fn linea_album(album: &AlbumResumen) -> String {
    format!(
        "  {} {}  ·  {}  {}",
        "♪",
        super::truncar(&album.titulo, 30),
        super::truncar(&album.artista, 18),
        album.anio.map(|anio| anio.to_string()).unwrap_or_default()
    )
}
