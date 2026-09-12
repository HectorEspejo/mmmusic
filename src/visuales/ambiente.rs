use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::canvas::{self, Points};

use super::Visual;
use super::paleta::Paleta;
use crate::audio::Analisis;

const MANCHAS: usize = 4;

pub struct Ambiente {
    tiempo: f32,
}

impl Ambiente {
    pub fn nuevo() -> Self {
        Self { tiempo: 0.0 }
    }
}

impl Visual for Ambiente {
    fn nombre(&self) -> &'static str {
        "Ambiente"
    }

    fn reiniciar(&mut self) {
        self.tiempo = 0.0;
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
        let rms = ((a.rms[0] + a.rms[1]) / 2.0).clamp(0.0, 1.0);
        let radio_base = (ancho.min(alto * 2.0) * 0.12).clamp(2.0, 12.0);
        let latido = if a.pulso { 1.25 } else { 1.0 };

        for i in 0..MANCHAS {
            let fase = i as f32 * 1.7;
            let x = cx + ancho * 0.27 * f64::from((self.tiempo * 0.13 + fase).sin());
            let y = cy + alto * 0.27 * f64::from((self.tiempo * 0.17 + fase * 1.3).cos());
            let tamano = radio_base
                * (0.7 + 1.6 * f64::from(rms))
                * latido
                * (0.85 + 0.3 * f64::from((self.tiempo * 0.9 + fase).sin()));
            let t = i as f32 / MANCHAS as f32;
            let color = p.gradiente((t * 0.6 + rms * 0.4).clamp(0.0, 1.0));
            let color = if a.ambiental {
                super::paleta::interpolar_color(color, p.tenue, 0.35)
            } else {
                color
            };
            dibujar_mancha(ctx, x, y, tamano, color, rms);
        }
        self.tiempo += dt;
    }
}

fn dibujar_mancha(
    ctx: &mut canvas::Context<'_>,
    cx: f64,
    cy: f64,
    radio: f64,
    color: Color,
    densidad: f32,
) {
    let radio_y = radio * 2.0;
    let paso = if densidad > 0.45 { 0.7 } else { 1.0 };
    let mut coords: Vec<(f64, f64)> = Vec::new();
    let mut dy = -radio_y;
    while dy <= radio_y {
        let factor = 1.0 - (dy / radio_y).powi(2);
        if factor > 0.0 {
            let dx_max = radio * factor.sqrt();
            let mut dx = -dx_max;
            while dx <= dx_max {
                coords.push((cx + dx, cy + dy));
                dx += paso;
            }
        }
        dy += paso;
    }
    ctx.draw(&Points {
        coords: &coords,
        color,
    });
}
