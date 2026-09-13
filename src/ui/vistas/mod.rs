pub mod albumes;
pub mod artistas;
pub mod ayuda;
pub mod buscar;
pub mod cola;
pub mod inicio;
pub mod letras;
pub mod pistas;
pub mod playlists;
pub mod radio;
pub mod visual;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Block, Paragraph};

use crate::app::{AppEstado, Foco, Pantalla, Vista};

pub fn dibujar(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    let [contenido, ayuda] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    match app.vista {
        Vista::Inicio => inicio::dibujar(frame, app, contenido),
        Vista::Buscar => buscar::dibujar(frame, app, contenido),
        Vista::Artistas => artistas::dibujar(frame, app, contenido),
        Vista::Albumes => albumes::dibujar(frame, app, contenido),
        Vista::Pistas => pistas::dibujar(frame, app, contenido),
        Vista::Playlists => playlists::dibujar(frame, app, contenido),
        Vista::Radio => radio::dibujar(frame, app, contenido),
        Vista::Letras => letras::dibujar(frame, app, contenido),
        Vista::Visual => {}
    }
    frame.render_widget(
        Paragraph::new(Span::styled(
            ayuda_contextual(app),
            Style::new().fg(app.paleta.secundario).bg(app.paleta.fondo),
        )),
        ayuda,
    );
}

pub fn panel(app: &AppEstado, titulo: &str) -> Block<'static> {
    Block::bordered()
        .title(titulo_panel(app, titulo))
        .border_style(Style::new().fg(app.paleta.secundario).bg(app.paleta.fondo))
        .style(Style::new().fg(app.paleta.texto).bg(app.paleta.fondo))
}

pub fn titulo_panel(app: &AppEstado, texto: &str) -> String {
    if app.foco == Foco::Contenido {
        format!(" ◄ {texto} ")
    } else {
        format!(" {texto} ")
    }
}

pub fn panel_cola(app: &AppEstado, titulo: &str) -> Block<'static> {
    let texto = if app.foco == Foco::Cola {
        format!(" ◄ {titulo} ")
    } else {
        format!(" {titulo} ")
    };
    Block::bordered()
        .title(texto)
        .border_style(Style::new().fg(app.paleta.secundario).bg(app.paleta.fondo))
        .style(Style::new().fg(app.paleta.texto).bg(app.paleta.fondo))
}

pub fn contar_albumes(numero: i64) -> String {
    if numero == 1 {
        "1 álbum".to_string()
    } else {
        format!("{numero} álbumes")
    }
}

pub fn contar_pistas(numero: i64) -> String {
    if numero == 1 {
        "1 pista".to_string()
    } else {
        format!("{numero} pistas")
    }
}

pub fn ventana(total: usize, seleccion: usize, capacidad: usize) -> (usize, usize) {
    if total == 0 || capacidad == 0 {
        return (0, 0);
    }
    let capacidad = capacidad.min(total);
    let seleccion = seleccion.min(total - 1);
    let inicio = seleccion
        .saturating_sub(capacidad / 2)
        .min(total - capacidad);
    (inicio, (inicio + capacidad).min(total))
}

pub fn truncar(texto: &str, ancho: usize) -> String {
    if ancho == 0 {
        return String::new();
    }
    let caracteres: Vec<char> = texto.chars().collect();
    if caracteres.len() <= ancho {
        return texto.to_string();
    }
    if ancho == 1 {
        return "…".to_string();
    }
    let mut salida: String = caracteres[..ancho - 1].iter().collect();
    salida.push('…');
    salida
}

fn ayuda_contextual(app: &AppEstado) -> String {
    match (app.vista, app.pantalla) {
        (Vista::Inicio, _) => {
            " h/l tarjeta · j/k bloque · Enter reproducir/abrir · a cola · A a continuación · ? ayuda"
        }
        (Vista::Buscar, _) => {
            " Escribe para buscar · Enter resultados · Esc salir del campo · ? ayuda"
        }
        (Vista::Artistas, Pantalla::Lista) => {
            " Enter detalle · a cola · A a continuación · j/k navegar · ? ayuda"
        }
        (Vista::Artistas, Pantalla::DetalleArtista) => {
            " Enter abrir/reproducir · h volver · a cola · A a continuación · ? ayuda"
        }
        (Vista::Albumes, Pantalla::Lista) => {
            " Enter detalle · h/l/j/k rejilla · o orden · a cola · ? ayuda"
        }
        (Vista::Albumes, Pantalla::DetalleAlbum) => {
            " Enter reproducir · h volver · a cola · A a continuación · ? ayuda"
        }
        (Vista::Pistas, _) => {
            " Enter reproducir · a cola · A a continuación · o orden · Ctrl+d/u página · ? ayuda"
        }
        (Vista::Playlists, _) => {
            " Enter abrir · N nueva · R renombrar · D eliminar · i importar · ? ayuda"
        }
        (Vista::Radio, _) => match app.pestana_radio {
            crate::app::PestanaRadio::Favoritas | crate::app::PestanaRadio::Todas => {
                " Enter escuchar · a cola · L favorita · N nueva · i importar · [ ] pestaña"
            }
            crate::app::PestanaRadio::Buscar => {
                " Tab campo · Enter buscar/escuchar · a guardar · L favorita · f título ICY"
            }
            crate::app::PestanaRadio::Sonando => {
                " f buscar en la biblioteca · [ ] pestaña · Espacio pausar"
            }
        },
        (Vista::Letras, _) => {
            " j/k desplazar · Enter saltar · ( ) offset · s fuente · gg/G · ? ayuda"
        }
        _ => " Enter reproducir · h volver · a cola · A a continuación · ? ayuda",
    }
    .to_string()
}
