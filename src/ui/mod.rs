pub mod barra_inferior;
pub mod componentes;
pub mod inactividad;
pub mod sidebar;
pub mod teclas;
pub mod vistas;

use std::io::{self, Stdout};
use std::panic;

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};

use crate::app::AppEstado;
use crate::config::ModoIconos;

#[derive(Debug, Clone, Copy)]
pub struct Iconos {
    pub reproducir: &'static str,
    pub pausa: &'static str,
    pub siguiente: &'static str,
    pub anterior: &'static str,
    pub aleatorio: &'static str,
    pub repeticion_no: &'static str,
    pub repeticion_todo: &'static str,
    pub repeticion_una: &'static str,
    pub volumen: &'static str,
    pub silencio: &'static str,
    pub nota: &'static str,
    pub reproducida: &'static str,
    pub scrobbling_enviado: &'static str,
    pub scrobbling_pendiente: &'static str,
    pub scrobbling_error: &'static str,
    pub corazon: &'static str,
    pub directo: &'static str,
    pub reconectando: &'static str,
    pub almacenando: &'static str,
    pub emisora: &'static str,
}

impl Iconos {
    pub fn desde(modo: ModoIconos) -> Self {
        match modo {
            ModoIconos::Nerd => Self {
                reproducir: "\u{f04b}",
                pausa: "\u{f04c}",
                siguiente: "\u{f051}",
                anterior: "\u{f048}",
                aleatorio: "\u{f074}",
                repeticion_no: "\u{f01e}",
                repeticion_todo: "\u{f01e}",
                repeticion_una: "\u{f01e}1",
                volumen: "\u{f028}",
                silencio: "\u{f026}",
                nota: "♪",
                reproducida: "▶",
                scrobbling_enviado: "↑",
                scrobbling_pendiente: "…",
                scrobbling_error: "!",
                corazon: "♥",
                directo: "●",
                reconectando: "⟳",
                almacenando: "◌",
                emisora: "◉",
            },
            ModoIconos::Ascii => Self {
                reproducir: ">",
                pausa: "||",
                siguiente: ">|",
                anterior: "|<",
                aleatorio: "shuf",
                repeticion_no: "rep",
                repeticion_todo: "rep todo",
                repeticion_una: "rep una",
                volumen: "vol",
                silencio: "sil",
                nota: "♪",
                reproducida: ">",
                scrobbling_enviado: "sc:ok",
                scrobbling_pendiente: "sc:",
                scrobbling_error: "sc:err",
                corazon: "♥",
                directo: "*",
                reconectando: "~",
                almacenando: "o",
                emisora: "(R)",
            },
        }
    }
}

pub fn iniciar() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    instalar_hook_panico();
    enable_raw_mode()?;
    let mut salida = io::stdout();
    execute!(salida, EnterAlternateScreen, EnableMouseCapture)?;
    let terminal = Terminal::new(CrosstermBackend::new(salida))?;
    Ok(terminal)
}

pub fn restaurar() {
    let _ = disable_raw_mode();
    let mut salida = io::stdout();
    let _ = execute!(salida, DisableMouseCapture, LeaveAlternateScreen);
}

fn instalar_hook_panico() {
    let anterior = panic::take_hook();
    panic::set_hook(Box::new(move |informacion| {
        restaurar();
        anterior(informacion);
    }));
}

pub fn dibujar(frame: &mut Frame, app: &mut AppEstado) {
    app.zonas = crate::app::ZonasRaton::default();
    let area = frame.area();
    app.tamano_terminal = (area.width, area.height);
    let compacto_ancho = area.width < 70;
    let compacto_alto = area.height < 20;
    let alto_barra = if compacto_alto { 2 } else { 3 };
    let [cuerpo, barra] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(alto_barra)]).areas(area);
    if app.modo_visual.is_some() {
        vistas::visual::dibujar(frame, app, cuerpo);
    } else {
        let ancho_sidebar = if compacto_ancho {
            4
        } else {
            app.config.interfaz.ancho_sidebar.min(area.width / 3)
        };
        let mostrar_cola = app.cola_visible && !compacto_ancho && cuerpo.width > 50;
        let (area_sidebar, area_contenido, area_cola) = if mostrar_cola {
            let [s, c, cola] = Layout::horizontal([
                Constraint::Length(ancho_sidebar),
                Constraint::Min(20),
                Constraint::Length(30),
            ])
            .areas(cuerpo);
            (s, c, Some(cola))
        } else {
            let [s, c] =
                Layout::horizontal([Constraint::Length(ancho_sidebar), Constraint::Min(1)])
                    .areas(cuerpo);
            (s, c, None)
        };
        sidebar::dibujar(frame, app, area_sidebar, compacto_ancho);
        vistas::dibujar(frame, app, area_contenido);
        if let Some(cola) = area_cola {
            vistas::cola::dibujar(frame, app, cola);
        }
    }
    if app.ecualizador_visible {
        componentes::ecualizador::dibujar(frame, app, cuerpo);
    }
    barra_inferior::dibujar(frame, app, barra, compacto_alto);
    componentes::notificacion::dibujar(frame, app, area);
    if app.ayuda_visible {
        vistas::ayuda::dibujar(frame, app, area);
    }
    componentes::dialogo::dibujar(frame, app, area);
}

pub fn miles(numero: i64) -> String {
    let negativo = numero < 0;
    let digitos = numero.unsigned_abs().to_string();
    let mut salida = String::with_capacity(digitos.len() + digitos.len() / 3 + 1);
    for (indice, caracter) in digitos.chars().enumerate() {
        if indice > 0 && (digitos.len() - indice).is_multiple_of(3) {
            salida.push('.');
        }
        salida.push(caracter);
    }
    if negativo {
        format!("-{salida}")
    } else {
        salida
    }
}

pub fn formatear_ms(ms: i64) -> String {
    if ms <= 0 {
        return "--:--".to_string();
    }
    let total = ms / 1000;
    let minutos = total / 60;
    let segundos = total % 60;
    if minutos >= 60 {
        let horas = minutos / 60;
        format!("{horas}:{:02}:{:02}", minutos % 60, segundos)
    } else {
        format!("{minutos}:{segundos:02}")
    }
}
