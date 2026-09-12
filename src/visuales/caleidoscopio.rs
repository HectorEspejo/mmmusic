use std::f64::consts::TAU;

use ratatui::layout::Rect;
use ratatui::widgets::canvas::{self, Line};

use super::Visual;
use super::paleta::Paleta;
use crate::audio::Analisis;

const SECTORES: usize = 6;

pub struct Caleidoscopio {
    rotacion: f64,
    giro: f64,
}

impl Caleidoscopio {
    pub fn nuevo() -> Self {
        Self {
            rotacion: 0.0,
            giro: 0.0,
        }
    }
}

impl Visual for Caleidoscopio {
    fn nombre(&self) -> &'static str {
        "Caleidoscopio"
    }

    fn reiniciar(&mut self) {
        self.rotacion = 0.0;
        self.giro = 0.0;
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
        let max_radio = (ancho.min(alto * 2.0) / 2.0) * 0.92;

        self.rotacion += f64::from(dt) * 0.12;
        if a.pulso {
            self.giro += f64::from(dt) * 3.0;
        }
        self.giro *= (1.0 - f64::from(dt) * 1.5).max(0.0);

        let n = a.bandas.len().max(1);
        let usadas = n.min(48);
        let sector = TAU / SECTORES as f64;
        for k in 0..SECTORES {
            let base = self.rotacion + self.giro + k as f64 * sector;
            let mut anterior = None;
            for j in 0..usadas {
                let indice = j * n / usadas;
                let valor = f64::from(a.bandas[indice.min(n - 1)]);
                let radio = (0.12 + 0.88 * valor) * max_radio;
                let angulo = base + (j as f64 / usadas as f64) * sector;
                let punto = (cx + radio * angulo.cos(), cy + radio * angulo.sin() * 2.0);
                if let Some((x0, y0)) = anterior {
                    ctx.draw(&Line {
                        x1: x0,
                        y1: y0,
                        x2: punto.0,
                        y2: punto.1,
                        color: p.gradiente((j as f32 / usadas as f32) * 0.7 + a.graves * 0.3),
                    });
                }
                anterior = Some(punto);
            }
        }
    }
}
