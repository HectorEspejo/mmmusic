use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::canvas::{self, Line};

use super::Visual;
use super::paleta::Paleta;
use crate::audio::Analisis;

pub struct BarrasOndas {
    ascii: bool,
    pico_izquierdo: f32,
    pico_derecho: f32,
}

impl BarrasOndas {
    pub fn nuevo(ascii: bool) -> Self {
        Self {
            ascii,
            pico_izquierdo: 0.0,
            pico_derecho: 0.0,
        }
    }
}

impl Visual for BarrasOndas {
    fn nombre(&self) -> &'static str {
        "Barras y ondas"
    }

    fn reiniciar(&mut self) {
        self.pico_izquierdo = 0.0;
        self.pico_derecho = 0.0;
    }

    fn dibujar(
        &mut self,
        a: &Analisis,
        area: Rect,
        p: &Paleta,
        dt: f32,
        ctx: &mut canvas::Context<'_>,
    ) {
        let ancho = f64::from(area.width).max(1.0);
        let alto = f64::from(area.height).max(1.0);
        let mitad = (alto / 2.0).floor().max(2.0);

        // Osciloscopio en la mitad superior.
        let centro = alto - mitad / 2.0;
        let amplitud = mitad * 0.42;
        let n = a.onda.len();
        if self.ascii {
            for (j, valor) in a.onda.iter().enumerate() {
                let x = if n > 1 {
                    j as f64 * (ancho - 1.0) / (n - 1) as f64
                } else {
                    0.0
                };
                let color = p.gradiente(valor.abs().min(1.0));
                ctx.print(
                    x.round(),
                    centro.round(),
                    Span::styled(
                        super::glifo_onda(*valor).to_string(),
                        Style::default().fg(color),
                    ),
                );
            }
        } else if n > 1 {
            for (j, pareja) in a.onda.windows(2).enumerate() {
                let x0 = j as f64 * (ancho - 1.0) / (n - 1) as f64;
                let x1 = (j + 1) as f64 * (ancho - 1.0) / (n - 1) as f64;
                let y0 = centro + f64::from(pareja[0]) * amplitud;
                let y1 = centro + f64::from(pareja[1]) * amplitud;
                let brillo = ((pareja[0].abs() + pareja[1].abs()) / 2.0).min(1.0);
                ctx.draw(&Line {
                    x1: x0,
                    y1: y0,
                    x2: x1,
                    y2: y1,
                    color: p.gradiente(brillo),
                });
            }
        }

        // Barras anchas con reflejo tenue.
        let cuantas = (area.width as usize / 10).clamp(8, 16);
        let base = 2.2;
        let alto_barra = (mitad - base).max(1.0);
        let paso = ancho / cuantas as f64;
        for i in 0..cuantas {
            let inicio = i * a.bandas.len() / cuantas;
            let fin = ((i + 1) * a.bandas.len() / cuantas)
                .max(inicio + 1)
                .min(a.bandas.len());
            let valor = a.bandas[inicio..fin].iter().copied().fold(0.0f32, f32::max);
            let x0 = i as f64 * paso + paso * 0.15;
            let x1 = ((i + 1) as f64 * paso - paso * 0.15).max(x0 + 0.1);
            let y1 = base + f64::from(valor) * alto_barra;
            ctx.draw(&Line {
                x1: x0,
                y1: base,
                x2: x1,
                y2: y1,
                color: p.gradiente(valor),
            });
            let reflejo = (y1 - base) * 0.25;
            ctx.draw(&Line {
                x1: x0,
                y1: base - reflejo,
                x2: x1,
                y2: base,
                color: p.tenue,
            });
        }

        // Medidores L/R con pico que cae.
        self.pico_izquierdo = (self.pico_izquierdo - 0.4 * dt).max(a.pico[0]);
        self.pico_derecho = (self.pico_derecho - 0.4 * dt).max(a.pico[1]);
        let ancho_medidor = (area.width as usize).saturating_sub(10) / 2;
        dibujar_medidor(
            ctx,
            1.0,
            0.6,
            'L',
            a.rms[0],
            self.pico_izquierdo,
            ancho_medidor,
            p,
        );
        dibujar_medidor(
            ctx,
            ancho / 2.0 + 1.0,
            0.6,
            'R',
            a.rms[1],
            self.pico_derecho,
            ancho_medidor,
            p,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn dibujar_medidor(
    ctx: &mut canvas::Context<'_>,
    x: f64,
    y: f64,
    etiqueta: char,
    nivel: f32,
    pico: f32,
    ancho: usize,
    p: &Paleta,
) {
    ctx.print(
        x,
        y,
        Span::styled(format!("{etiqueta} "), Style::default().fg(p.tenue).bold()),
    );
    let llenas = ((nivel.clamp(0.0, 1.0)) * ancho as f32).round() as usize;
    let marca = ((pico.clamp(0.0, 1.0)) * ancho as f32).round() as usize;
    let relleno: String = "█".repeat(llenas);
    ctx.print(
        x + 2.0,
        y,
        Span::styled(relleno, Style::default().fg(p.secundario)),
    );
    if marca > llenas {
        ctx.print(
            x + 2.0 + llenas as f64,
            y,
            Span::styled("▏", Style::default().fg(p.resalte)),
        );
    }
    let vacias: String = "·".repeat(ancho.saturating_sub(marca));
    ctx.print(
        x + 2.0 + (marca.max(llenas)) as f64,
        y,
        Span::styled(vacias, Style::default().fg(p.tenue)),
    );
}
