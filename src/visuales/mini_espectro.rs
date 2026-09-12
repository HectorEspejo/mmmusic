use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::paleta::Paleta;
use crate::audio::Analisis;

pub const BARRAS: usize = 12;
const GLIFOS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Doce barras de una fila con las mismas bandas suavizadas, submuestreadas.
pub fn linea(a: &Analisis, p: &Paleta) -> Line<'static> {
    let n = a.bandas.len().max(1);
    let mut spans = Vec::with_capacity(BARRAS);
    for i in 0..BARRAS {
        let inicio = i * n / BARRAS;
        let fin = (((i + 1) * n) / BARRAS).max(inicio + 1).min(n);
        let valor = a.bandas[inicio.min(n - 1)..fin]
            .iter()
            .copied()
            .fold(0.0f32, f32::max);
        let indice = (valor * 8.0).round().clamp(0.0, 8.0) as usize;
        spans.push(Span::styled(
            GLIFOS[indice].to_string(),
            Style::new().fg(p.gradiente(valor)),
        ));
    }
    Line::from(spans)
}
