use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::canvas::{self, Line};

use super::Visual;
use super::paleta::Paleta;
use crate::audio::Analisis;

const BASE: f64 = 1.4;

pub struct Espectro {
    picos: Vec<f32>,
    ascii: bool,
}

impl Espectro {
    pub fn nuevo(ascii: bool) -> Self {
        Self {
            picos: Vec::new(),
            ascii,
        }
    }
}

impl Visual for Espectro {
    fn nombre(&self) -> &'static str {
        "Espectro"
    }

    fn reiniciar(&mut self) {
        self.picos.clear();
    }

    fn dibujar(
        &mut self,
        a: &Analisis,
        area: Rect,
        p: &Paleta,
        dt: f32,
        ctx: &mut canvas::Context<'_>,
    ) {
        let n = a.bandas.len().max(1);
        if self.picos.len() != n {
            self.picos = vec![0.0; n];
        }
        let alto = (f64::from(area.height) - BASE).max(1.0);
        let ancho = f64::from(area.width) / n as f64;
        let segmentos = 4usize;

        for (i, valor) in a.bandas.iter().enumerate() {
            let x0 = i as f64 * ancho + 0.15;
            let x1 = (((i + 1) as f64 * ancho) - 0.15).max(x0 + 0.1);
            let altura = f64::from(*valor) * alto;
            if self.ascii {
                let filas = altura.round() as usize;
                for fila in 0..filas {
                    let t = fila as f32 / filas.max(1) as f32;
                    ctx.print(
                        x0,
                        fila as f64,
                        Span::styled("#", Style::new().fg(p.gradiente(t))),
                    );
                }
            } else {
                for segmento in 0..segmentos {
                    let t0 = segmento as f64 / segmentos as f64;
                    let t1 = (segmento + 1) as f64 / segmentos as f64;
                    let y0 = BASE + altura * t0;
                    let y1 = BASE + altura * t1;
                    if y1 - y0 < 0.01 {
                        continue;
                    }
                    ctx.draw(&Line {
                        x1: x0,
                        y1: y0,
                        x2: x1,
                        y2: y1,
                        color: p.gradiente(((t0 + t1) / 2.0) as f32),
                    });
                }
            }
            self.picos[i] = (self.picos[i] - 0.55 * dt).max(*valor);
            let y_pico = BASE + f64::from(self.picos[i]) * alto;
            ctx.draw(&Line {
                x1: x0,
                y1: y_pico,
                x2: x1,
                y2: y_pico,
                color: p.resalte,
            });
        }

        let estilo_eje = Style::new().fg(p.tenue);
        ctx.print(1.0, 0.2, Span::styled("20 Hz", estilo_eje));
        let etiqueta = "16 kHz";
        ctx.print(
            f64::from(area.width).max(7.0) - 6.5,
            0.2,
            Span::styled(etiqueta, estilo_eje),
        );
    }
}
