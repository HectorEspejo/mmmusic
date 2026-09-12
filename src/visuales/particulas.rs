use std::f64::consts::TAU;

use rand::RngExt;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::canvas::{self, Line};

use super::Visual;
use super::paleta::{Paleta, interpolar_color};
use crate::audio::Analisis;

const MAX_VIVAS: usize = 400;

struct Particula {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    vida: f32,
    vida_max: f32,
    brillo: f32,
}

pub struct Particulas {
    vivas: Vec<Particula>,
    ascii: bool,
}

impl Particulas {
    pub fn nuevo(ascii: bool) -> Self {
        Self {
            vivas: Vec::new(),
            ascii,
        }
    }
}

impl Visual for Particulas {
    fn nombre(&self) -> &'static str {
        "Partículas"
    }

    fn reiniciar(&mut self) {
        self.vivas.clear();
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
        let brillo_agudos = super::agudos(&a.bandas);

        if a.pulso {
            let cantidad = (20.0 + 40.0 * a.graves).round() as usize;
            let mut rng = rand::rng();
            for _ in 0..cantidad {
                let angulo: f64 = rng.random_range(0.0..TAU);
                let velocidad = 1.5 + 16.0 * f64::from(a.graves) * rng.random_range(0.35..1.0);
                let vida = rng.random_range(1.0f32..2.0);
                self.vivas.push(Particula {
                    x: cx,
                    y: cy,
                    vx: angulo.cos() * velocidad,
                    vy: angulo.sin() * velocidad * 2.0,
                    vida,
                    vida_max: vida,
                    brillo: brillo_agudos,
                });
            }
            if self.vivas.len() > MAX_VIVAS {
                let sobrante = self.vivas.len() - MAX_VIVAS;
                self.vivas.drain(0..sobrante);
            }
        }

        for particula in &mut self.vivas {
            particula.x += particula.vx * f64::from(dt);
            particula.y += particula.vy * f64::from(dt);
            particula.vx *= 0.985;
            particula.vy *= 0.985;
            particula.vida -= dt;
        }
        self.vivas.retain(|particula| particula.vida > 0.0);

        for particula in &self.vivas {
            let vida = (particula.vida / particula.vida_max).clamp(0.0, 1.0);
            let intensidad = (particula.brillo * 0.6 + vida * 0.4).clamp(0.0, 1.0);
            let color = interpolar_color(p.tenue, p.acento, intensidad);
            if self.ascii {
                let glifo = if intensidad > 0.75 {
                    '#'
                } else if intensidad > 0.45 {
                    '+'
                } else {
                    '.'
                };
                ctx.print(
                    particula.x.round(),
                    particula.y.round(),
                    Span::styled(glifo.to_string(), Style::default().fg(color)),
                );
            } else {
                let estela_x = particula.x - particula.vx * 0.06;
                let estela_y = particula.y - particula.vy * 0.06;
                ctx.draw(&Line {
                    x1: estela_x,
                    y1: estela_y,
                    x2: particula.x,
                    y2: particula.y,
                    color,
                });
            }
        }
    }
}
