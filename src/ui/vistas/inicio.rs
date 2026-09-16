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
const EXTRA_ALTO_TARJETA: u16 = 4; // bordes superior e inferior + dos líneas de texto

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
    let bloques: Vec<Vec<TarjetaInicio>> = (0..3)
        .map(|indice_bloque| tarjetas_del_bloque(app, indice_bloque))
        .collect();
    let alturas: Vec<u16> = bloques
        .iter()
        .map(|tarjetas| {
            if tarjetas.is_empty() {
                2 // título + mensaje
            } else {
                1 + alto_tarjeta(app, ANCHO_TARJETA, interior.height)
            }
        })
        .collect();
    let primero = primer_bloque_visible(app, interior.height, &alturas);
    let mut y = interior.y;
    for (indice_bloque, tarjetas) in bloques.iter().enumerate().skip(primero) {
        let disponible = interior.bottom().saturating_sub(y);
        if disponible < 2 {
            break;
        }
        let zona = Rect {
            y,
            height: alturas[indice_bloque].min(disponible),
            ..interior
        };
        dibujar_bloque(frame, app, zona, indice_bloque, tarjetas);
        y = zona.bottom() + 1; // una fila en blanco separa los bloques
    }
}

/// Índice del primer bloque que se dibuja para que el bloque enfocado quede a
/// la vista: normalmente 0, y solo sube cuando los anteriores no caben junto a
/// él.
fn primer_bloque_visible(app: &AppEstado, alto: u16, alturas: &[u16]) -> usize {
    let foco = app.bloque_inicio.min(alturas.len().saturating_sub(1));
    let mut primero = foco;
    while primero > 0 {
        let candidato = primero - 1;
        let total: u16 = alturas[candidato..=foco].iter().sum::<u16>() + (foco - candidato) as u16;
        if total > alto {
            break;
        }
        primero = candidato;
    }
    primero
}

fn dibujar_bloque(
    frame: &mut Frame,
    app: &mut AppEstado,
    zona: Rect,
    indice_bloque: usize,
    tarjetas: &[TarjetaInicio],
) {
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
    if cuerpo.height == 0 {
        return;
    }
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
    if cuerpo.height < 3 {
        return;
    }
    dibujar_fila_tarjetas(frame, app, cuerpo, tarjetas);
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

/// Alto que necesita una tarjeta para envolver su carátula, el texto y el
/// borde, sin estirarse por todo el alto del bloque.
fn alto_tarjeta(app: &AppEstado, ancho: u16, alto_disponible: u16) -> u16 {
    let fuente = app.caratulas.fuente().unwrap_or(imagen::FUENTE_POR_DEFECTO);
    let alto_caratula = imagen::alto_caratula_ajustada(ancho.saturating_sub(2), fuente);
    (alto_caratula + EXTRA_ALTO_TARJETA).min(alto_disponible)
}

fn dibujar_fila_tarjetas(
    frame: &mut Frame,
    app: &mut AppEstado,
    area: Rect,
    tarjetas: &[TarjetaInicio],
) {
    let alto = alto_tarjeta(app, ANCHO_TARJETA, area.height);
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
            height: alto,
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

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::config::{Config, ConfigTema};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui_image::picker::Picker;

    fn app_con_caratula() -> AppEstado {
        let mut app = AppEstado::nuevo(
            Config::default(),
            crate::tema::terminal(&ConfigTema::default()),
            0,
            std::path::PathBuf::from("/tmp/tema"),
        );
        let (tx, _rx) = std::sync::mpsc::channel();
        app.activar_caratulas(Picker::halfblocks(), tx);
        app.caratulas
            .insertar(1, image::DynamicImage::new_rgb8(300, 300));
        app.inicio.recientes.push(PistaListado {
            id: 1,
            titulo: "Pista".to_string(),
            artista: "Artista".to_string(),
            album: "Álbum".to_string(),
            album_id: 1,
            duracion_ms: 180_000,
            anio: Some(2024),
            formato: "flac".to_string(),
            caratula_ruta: None,
            ruta: "pista.flac".to_string(),
        });
        app
    }

    #[test]
    fn la_tarjeta_se_ajusta_a_la_caratula_el_texto_y_el_borde() {
        let app = app_con_caratula();
        // Fuente 10×20 y ancho interior 14: carátula de 7 celdas + 2 de texto
        // + 2 de borde.
        assert_eq!(alto_tarjeta(&app, ANCHO_TARJETA, 40), 11);
        assert_eq!(
            alto_tarjeta(&app, ANCHO_TARJETA, 8),
            8,
            "nunca debe superar el alto disponible"
        );
    }

    #[test]
    fn los_bloques_se_apilan_pegados_a_su_contenido() {
        let mut app = app_con_caratula();
        app.inicio.anadidos.push(AlbumResumen {
            id: 2,
            titulo: "Otro álbum".to_string(),
            artista: "Artista".to_string(),
            anio: Some(2024),
            caratula_ruta: None,
            num_pistas: 1,
            duracion_ms: 180_000,
        });
        app.tamano_terminal = (120, 40);
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal de pruebas");
        terminal
            .draw(|frame| crate::ui::dibujar(frame, &mut app))
            .expect("dibujar Inicio");

        let buffer = terminal.backend().buffer();
        let fila = |texto: &str| {
            (0..buffer.area.height).find(|y| {
                let linea: String = (0..buffer.area.width)
                    .filter_map(|x| buffer.cell((x, *y)))
                    .map(|celda| celda.symbol())
                    .collect();
                linea.contains(texto)
            })
        };
        let primer_titulo = fila("Reproducidas recientemente").expect("primer bloque");
        let segundo_titulo = fila("Añadidos recientemente").expect("segundo bloque");
        // Título + tarjeta de 11 filas + fila en blanco de separación.
        assert_eq!(segundo_titulo, primer_titulo + 13);
    }

    #[test]
    fn el_bloque_enfocado_se_desplaza_a_la_vista() {
        let mut app = app_con_caratula();
        for indice in 1..=3 {
            app.inicio.anadidos.push(AlbumResumen {
                id: indice + 10,
                titulo: format!("Álbum {indice}"),
                artista: "Artista".to_string(),
                anio: Some(2024),
                caratula_ruta: None,
                num_pistas: 1,
                duracion_ms: 180_000,
            });
        }
        app.inicio.redescubre.push(AlbumResumen {
            id: 99,
            titulo: "Reencontrado".to_string(),
            artista: "Artista".to_string(),
            anio: Some(2023),
            caratula_ruta: None,
            num_pistas: 1,
            duracion_ms: 180_000,
        });
        // Alto insuficiente para los tres bloques completos.
        app.tamano_terminal = (120, 26);
        app.bloque_inicio = 2;
        let backend = TestBackend::new(120, 26);
        let mut terminal = Terminal::new(backend).expect("terminal de pruebas");
        terminal
            .draw(|frame| crate::ui::dibujar(frame, &mut app))
            .expect("dibujar Inicio");

        let buffer = terminal.backend().buffer();
        let fila = |texto: &str| {
            (0..buffer.area.height).find(|y| {
                let linea: String = (0..buffer.area.width)
                    .filter_map(|x| buffer.cell((x, *y)))
                    .map(|celda| celda.symbol())
                    .collect();
                linea.contains(texto)
            })
        };
        let titulo = fila("Redescubre").expect("bloque enfocado visible");
        assert!(
            titulo < 6,
            "el bloque enfocado debe subir a la parte superior"
        );
        assert!(fila("Reencontrado").is_some(), "las tarjetas deben verse");
    }

    #[test]
    fn el_borde_de_la_tarjeta_envuelve_el_contenido() {
        let mut app = app_con_caratula();
        app.tamano_terminal = (120, 40);
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal de pruebas");
        terminal
            .draw(|frame| crate::ui::dibujar(frame, &mut app))
            .expect("dibujar Inicio");

        let buffer = terminal.backend().buffer();
        let ancho = buffer.area.width as usize;
        let esquina = buffer
            .content()
            .iter()
            .enumerate()
            .filter(|(indice, celda)| {
                let x = indice % ancho;
                let y = indice / ancho;
                y >= 2 && x > 18 && celda.symbol() == "┌"
            })
            .map(|(indice, _)| ((indice % ancho) as u16, (indice / ancho) as u16))
            .min_by_key(|(x, y)| (*y, *x))
            .expect("esquina superior de la tarjeta");
        let fondo = (esquina.1..buffer.area.height)
            .find(|y| {
                buffer
                    .cell((esquina.0, *y))
                    .is_some_and(|celda| celda.symbol() == "└")
            })
            .expect("esquina inferior de la tarjeta");
        assert_eq!(
            fondo - esquina.1 + 1,
            alto_tarjeta(&app, ANCHO_TARJETA, 11),
            "el borde debe medir lo que ocupan carátula, texto y bordes"
        );
    }
}
