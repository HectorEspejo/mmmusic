pub mod ambiente;
pub mod barras_ondas;
pub mod caleidoscopio;
pub mod espectro;
pub mod mini_espectro;
pub mod paleta;
pub mod particulas;
pub mod tunel;

use std::cell::RefCell;
use std::rc::Rc;

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::canvas;

use crate::audio::Analisis;
use paleta::Paleta;

pub trait Visual {
    fn nombre(&self) -> &'static str;
    fn reiniciar(&mut self);
    fn dibujar(
        &mut self,
        a: &Analisis,
        area: Rect,
        p: &Paleta,
        dt: f32,
        ctx: &mut canvas::Context<'_>,
    );
}

/// Nombres en castellano y orden canónico de las seis visuales.
pub const NOMBRES: [&str; 6] = [
    "Espectro",
    "Barras y ondas",
    "Ambiente",
    "Partículas",
    "Caleidoscopio",
    "Túnel",
];

pub fn indice_por_nombre(nombre: &str) -> Option<usize> {
    NOMBRES
        .iter()
        .position(|candidato| candidato.eq_ignore_ascii_case(nombre.trim()))
}

pub type VisualCompartida = Rc<RefCell<Box<dyn Visual>>>;

pub fn registro(ascii: bool) -> Vec<VisualCompartida> {
    let mut lista: Vec<VisualCompartida> = Vec::with_capacity(NOMBRES.len());
    lista.push(Rc::new(RefCell::new(Box::new(espectro::Espectro::nuevo(
        ascii,
    )))));
    lista.push(Rc::new(RefCell::new(Box::new(
        barras_ondas::BarrasOndas::nuevo(ascii),
    ))));
    lista.push(Rc::new(RefCell::new(Box::new(ambiente::Ambiente::nuevo()))));
    lista.push(Rc::new(RefCell::new(Box::new(
        particulas::Particulas::nuevo(ascii),
    ))));
    lista.push(Rc::new(RefCell::new(Box::new(
        caleidoscopio::Caleidoscopio::nuevo(),
    ))));
    lista.push(Rc::new(RefCell::new(Box::new(tunel::Tunel::nuevo()))));
    lista
}

pub fn barra_ascii(ctx: &mut canvas::Context<'_>, x: f64, filas: usize, color: Color, glifo: char) {
    for fila in 0..filas {
        ctx.print(
            x,
            fila as f64,
            Span::styled(glifo.to_string(), Style::new().fg(color)),
        );
    }
}

pub fn glifo_onda(valor: f32) -> char {
    if valor > 0.33 {
        '~'
    } else if valor < -0.33 {
        '_'
    } else {
        '-'
    }
}

pub fn agudos(bandas: &[f32]) -> f32 {
    if bandas.is_empty() {
        return 0.0;
    }
    let inicio = bandas.len() * 2 / 3;
    let trozo = &bandas[inicio..];
    trozo.iter().sum::<f32>() / trozo.len() as f32
}
