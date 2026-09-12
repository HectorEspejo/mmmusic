use std::f64::consts::TAU;

use ratatui::layout::Rect;
use ratatui::widgets::canvas::{self, Line};

use super::Visual;
use super::paleta::Paleta;
use crate::audio::Analisis;

const ANILLOS: usize = 6;
const PASOS: usize = 48;

pub struct Tunel {
    fase: f32,
}

impl Tunel {
    pub fn nuevo() -> Self {
        Self { fase: 0.0 }
    }
}

impl Visual for Tunel {
    fn nombre(&self) -> &'static str {
        "Túnel"
    }

    fn reiniciar(&mut self) {
        self.fase = 0.0;
    }

    fn dibujar(
        &mut self,
        a: &Analisis,
        area: Rect,
        p: &Paleta,
        dt: f32,
        ctx: &mut canvas::Context<'_>,
    ) {
        let ancho = f64::from(area.width).max(4.0);
        let alto = f64::from(area.height).max(4.0);
        let (cx, cy) = (ancho / 2.0, alto / 2.0);
        let max_radio = (ancho.min(alto * 2.0) / 2.0) * 0.95;
        let rms = ((a.rms[0] + a.rms[1]) / 2.0).clamp(0.0, 1.0);

        self.fase = (self.fase + dt * (0.25 + 1.8 * rms)) % 1.0;
        let n = a.bandas.len().max(1);

        for anillo in 0..ANILLOS {
            let profundidad = (anillo as f32 + self.fase) / ANILLOS as f32;
            let radio_base = max_radio * (0.12 + 0.88 * f64::from(profundidad));
            let color = p.gradiente((1.0 - profundidad).clamp(0.0, 1.0));
            let mut anterior = None;
            for paso in 0..=PASOS {
                let angulo = paso as f64 / PASOS as f64 * TAU;
                let indice = paso * n / PASOS;
                let banda = f64::from(a.bandas[indice.min(n - 1)]);
                let radio = radio_base * (0.72 + 0.55 * banda);
                let punto = (cx + radio * angulo.cos(), cy + radio * angulo.sin() * 2.0);
                if let Some((x0, y0)) = anterior {
                    ctx.draw(&Line {
                        x1: x0,
                        y1: y0,
                        x2: punto.0,
                        y2: punto.1,
                        color,
                    });
                }
                anterior = Some(punto);
            }
        }
    }
}
