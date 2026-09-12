use std::collections::VecDeque;
use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};

use super::anillo::Anillo;

pub const TAMANO_FFT: usize = 2048;
pub const F_MIN: f64 = 20.0;
pub const F_MAX: f64 = 16000.0;

const DB_MINIMO: f32 = -60.0;
const OBJETIVO_AGC: f32 = 0.85;
const ATAQUE_SEG: f32 = 0.05;
const LIBERACION_SEG: f32 = 2.0;
const GANANCIA_MIN: f32 = 0.5;
const GANANCIA_MAX: f32 = 8.0;
const SENSIBILIDAD_MIN: f32 = 0.25;
const SENSIBILIDAD_MAX: f32 = 4.0;
const GRAVEDAD: f32 = 0.9;
const PULSO_FACTOR: f32 = 1.4;
const MIN_MS_ENTRE_PULSOS: u64 = 250;
const VENTANA_PULSO_MS: u64 = 1000;
const SILENCIO_PAUSA_MS: u64 = 2000;

#[derive(Debug, Clone, PartialEq)]
pub struct Analisis {
    pub bandas: Vec<f32>,
    pub onda: Vec<f32>,
    pub rms: [f32; 2],
    pub pico: [f32; 2],
    pub graves: f32,
    pub pulso: bool,
    pub tasa_hz: u32,
    pub ambiental: bool,
}

impl Default for Analisis {
    fn default() -> Self {
        Self::vacio(64, 160)
    }
}

impl Analisis {
    pub fn vacio(n_bandas: usize, n_onda: usize) -> Self {
        Self {
            bandas: vec![0.0; n_bandas],
            onda: vec![0.0; n_onda],
            rms: [0.0, 0.0],
            pico: [0.0, 0.0],
            graves: 0.0,
            pulso: false,
            tasa_hz: 48_000,
            ambiental: false,
        }
    }

    /// Valores sintéticos suaves para cuando no hay audio (sin nodo, error o
    /// dos segundos de silencio en pausa).
    pub fn ambiental(t_ms: u64, n_bandas: usize, n_onda: usize) -> Self {
        let t = t_ms as f32 / 1000.0;
        let bandas = (0..n_bandas)
            .map(|i| {
                let x = i as f32 / n_bandas.max(1) as f32;
                let base = 0.16 + 0.30 * (1.0 - x);
                let vaiven = 0.10 * (t * 0.55 + i as f32 * 0.31).sin()
                    + 0.06 * (t * 0.23 + i as f32 * 0.13).sin();
                (base + vaiven).clamp(0.0, 1.0)
            })
            .collect();
        let onda = (0..n_onda)
            .map(|j| {
                0.22 * (t * 1.1 + j as f32 * 0.16).sin() + 0.08 * (t * 0.37 + j as f32 * 0.05).sin()
            })
            .collect();
        Self {
            bandas,
            onda,
            rms: [0.16, 0.15],
            pico: [0.24, 0.23],
            graves: 0.2,
            pulso: false,
            tasa_hz: 48_000,
            ambiental: true,
        }
    }
}

/// Análisis por frame: FFT, bandas logarítmicas, AGC, suavizado, onda, niveles
/// y detección de pulso. Sin E/S: todo se alimenta del `Anillo`.
pub struct Analizador {
    plan: Arc<dyn RealToComplex<f64>>,
    hann: Vec<f64>,
    entrada: Vec<f64>,
    salida: Vec<Complex<f64>>,
    scratch: Vec<Complex<f64>>,
    limites: Vec<f64>,
    bandas_previas: Vec<f32>,
    estereo: [Vec<f32>; 2],
    frames_leidos: u64,
    silencio_desde_ms: Option<u64>,
    t_ms_anterior: u64,
    ganancia: f32,
    historial_graves: VecDeque<(u64, f32)>,
    ultimo_pulso_ms: Option<u64>,
}

impl Default for Analizador {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Analizador {
    pub fn nuevo() -> Self {
        let mut planificador = RealFftPlanner::<f64>::new();
        let plan = planificador.plan_fft_forward(TAMANO_FFT);
        let scratch = vec![Complex::default(); plan.get_scratch_len()];
        let hann = (0..TAMANO_FFT)
            .map(|i| {
                let x = 2.0 * std::f64::consts::PI * i as f64 / TAMANO_FFT as f64;
                0.5 * (1.0 - x.cos())
            })
            .collect();
        Self {
            plan,
            hann,
            entrada: vec![0.0; TAMANO_FFT],
            salida: vec![Complex::default(); TAMANO_FFT / 2 + 1],
            scratch,
            limites: limites_bandas(64),
            bandas_previas: vec![0.0; 64],
            estereo: [vec![0.0; TAMANO_FFT], vec![0.0; TAMANO_FFT]],
            frames_leidos: 0,
            silencio_desde_ms: None,
            t_ms_anterior: 0,
            ganancia: 1.0,
            historial_graves: VecDeque::new(),
            ultimo_pulso_ms: None,
        }
    }

    pub fn reiniciar(&mut self) {
        self.bandas_previas.fill(0.0);
        self.estereo = [vec![0.0; TAMANO_FFT], vec![0.0; TAMANO_FFT]];
        self.frames_leidos = 0;
        self.silencio_desde_ms = None;
        self.t_ms_anterior = 0;
        self.ganancia = 1.0;
        self.historial_graves.clear();
        self.ultimo_pulso_ms = None;
    }

    pub fn ganancia(&self) -> f32 {
        self.ganancia
    }

    #[allow(clippy::too_many_arguments)]
    pub fn procesar(
        &mut self,
        anillo: &Anillo,
        ancho_celdas: u16,
        sensibilidad: f32,
        tasa_hz: u32,
        t_ms: u64,
        sin_audio: bool,
        en_pausa: bool,
    ) -> Analisis {
        let n_bandas = (ancho_celdas as usize / 2).clamp(16, 128);
        let n_onda = (ancho_celdas as usize * 2).max(2);
        if self.bandas_previas.len() != n_bandas {
            self.bandas_previas = vec![0.0; n_bandas];
            self.limites = limites_bandas(n_bandas);
        }

        let t_previo = self.t_ms_anterior;
        let total = anillo.frames_escritos();
        let nuevas = total.saturating_sub(self.frames_leidos);
        if nuevas >= TAMANO_FFT as u64 {
            self.estereo = anillo.ventana_estereo(TAMANO_FFT);
            self.frames_leidos = total;
            self.silencio_desde_ms = None;
        } else if nuevas == 0 {
            self.silencio_desde_ms.get_or_insert(t_previo);
        }

        let silencio_en_pausa = en_pausa
            && self
                .silencio_desde_ms
                .is_some_and(|desde| t_ms.saturating_sub(desde) >= SILENCIO_PAUSA_MS);
        if sin_audio || silencio_en_pausa {
            let mut ambiental = Analisis::ambiental(t_ms, n_bandas, n_onda);
            ambiental.tasa_hz = tasa_hz;
            return ambiental;
        }

        for ((destino, muestra), hann) in self
            .entrada
            .iter_mut()
            .zip(self.estereo[0].iter().zip(self.estereo[1].iter()))
            .zip(self.hann.iter())
        {
            *destino = f64::from((muestra.0 + muestra.1) * 0.5) * hann;
        }
        if let Err(error) =
            self.plan
                .process_with_scratch(&mut self.entrada, &mut self.salida, &mut self.scratch)
        {
            tracing::warn!("FFT con error: {error}");
            return Analisis::vacio(n_bandas, n_onda);
        }
        let magnitudes: Vec<f64> = self.salida.iter().map(|valor| valor.norm()).collect();

        let pares = self.limites.windows(2);
        let preliminares: Vec<f32> = pares
            .map(|limite| banda_desde_magnitudes(&magnitudes, tasa_hz as f64, limite[0], limite[1]))
            .collect();
        let graves = banda_desde_magnitudes(&magnitudes, tasa_hz as f64, F_MIN, 150.0);

        let pico_actual = preliminares.iter().copied().fold(0.0f32, f32::max);
        let dt = (t_ms.saturating_sub(self.t_ms_anterior) as f32 / 1000.0).clamp(0.001, 0.5);
        self.t_ms_anterior = t_ms;
        if pico_actual > 1e-6 {
            let objetivo = (OBJETIVO_AGC / pico_actual).clamp(GANANCIA_MIN, GANANCIA_MAX);
            let coeficiente = if pico_actual * self.ganancia > OBJETIVO_AGC {
                1.0 - (-dt / ATAQUE_SEG).exp()
            } else {
                1.0 - (-dt / LIBERACION_SEG).exp()
            };
            self.ganancia += (objetivo - self.ganancia) * coeficiente;
            self.ganancia = self.ganancia.clamp(GANANCIA_MIN, GANANCIA_MAX);
        }
        let ganancia = self.ganancia * sensibilidad.clamp(SENSIBILIDAD_MIN, SENSIBILIDAD_MAX);

        let bandas: Vec<f32> = preliminares
            .iter()
            .zip(self.bandas_previas.iter_mut())
            .map(|(nueva, previa)| {
                let valor = (nueva * ganancia).min(1.0);
                *previa = valor.max(*previa * GRAVEDAD);
                *previa
            })
            .collect();

        let pulso = self.detectar_pulso(graves, t_ms);
        Analisis {
            bandas,
            onda: muestrear_onda(&self.estereo, n_onda),
            rms: [rms(&self.estereo[0], n_onda), rms(&self.estereo[1], n_onda)],
            pico: [
                pico(&self.estereo[0], n_onda),
                pico(&self.estereo[1], n_onda),
            ],
            graves,
            pulso,
            tasa_hz,
            ambiental: false,
        }
    }

    fn detectar_pulso(&mut self, graves: f32, t_ms: u64) -> bool {
        self.historial_graves.push_back((t_ms, graves));
        while self
            .historial_graves
            .front()
            .is_some_and(|(cuando, _)| t_ms.saturating_sub(*cuando) > VENTANA_PULSO_MS)
        {
            self.historial_graves.pop_front();
        }
        let media = if self.historial_graves.is_empty() {
            0.0
        } else {
            self.historial_graves
                .iter()
                .map(|(_, valor)| valor)
                .sum::<f32>()
                / self.historial_graves.len() as f32
        };
        let hubo_recientemente = self
            .ultimo_pulso_ms
            .is_some_and(|ultimo| t_ms.saturating_sub(ultimo) < MIN_MS_ENTRE_PULSOS);
        if graves > PULSO_FACTOR * media.max(1e-3) && !hubo_recientemente {
            self.ultimo_pulso_ms = Some(t_ms);
            true
        } else {
            false
        }
    }
}

pub fn limites_bandas(n_bandas: usize) -> Vec<f64> {
    let razon = (F_MAX / F_MIN).powf(1.0 / n_bandas.max(1) as f64);
    (0..=n_bandas)
        .map(|i| F_MIN * razon.powi(i as i32))
        .collect()
}

fn banda_desde_magnitudes(magnitudes: &[f64], tasa_hz: f64, f0: f64, f1: f64) -> f32 {
    let maximo_bin = magnitudes.len().saturating_sub(1);
    let k0 = ((f0 * TAMANO_FFT as f64 / tasa_hz) as usize)
        .max(1)
        .min(maximo_bin);
    let k1 = ((f1 * TAMANO_FFT as f64 / tasa_hz) as usize)
        .max(k0 + 1)
        .min(maximo_bin + 1);
    let pico = magnitudes[k0..k1].iter().copied().fold(0.0f64, f64::max);
    // Escala de un solo lado con la ganancia coherente de Hann incluida: un
    // seno a plena escala ronda −12 dB y el primer lóbulo lateral de Hann
    // (−31,5 dB) queda por debajo del 30 % del pico en la escala −60..0.
    let normalizado = pico / TAMANO_FFT as f64;
    let db = 20.0 * (normalizado + 1e-9).log10();
    (((db as f32) - DB_MINIMO) / -DB_MINIMO).clamp(0.0, 1.0)
}

fn muestrear_onda(estereo: &[Vec<f32>; 2], n_onda: usize) -> Vec<f32> {
    let fuente = &estereo[0];
    let total = fuente.len();
    (0..n_onda)
        .map(|j| {
            let indice = if n_onda <= total {
                total - n_onda + j
            } else {
                j * total / n_onda
            };
            let izquierda = fuente.get(indice).copied().unwrap_or(0.0);
            let derecha = estereo[1].get(indice).copied().unwrap_or(0.0);
            (izquierda + derecha) * 0.5
        })
        .collect()
}

fn rms(muestras: &[f32], n: usize) -> f32 {
    if muestras.is_empty() {
        return 0.0;
    }
    let inicio = muestras.len().saturating_sub(n);
    let trozo = &muestras[inicio..];
    (trozo.iter().map(|valor| valor * valor).sum::<f32>() / trozo.len() as f32).sqrt()
}

fn pico(muestras: &[f32], n: usize) -> f32 {
    if muestras.is_empty() {
        return 0.0;
    }
    let inicio = muestras.len().saturating_sub(n);
    muestras[inicio..]
        .iter()
        .fold(0.0f32, |maximo, valor| maximo.max(valor.abs()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn limites_bandas_cubren_el_rango() {
        let limites = limites_bandas(64);
        assert_eq!(limites.len(), 65);
        assert!((limites[0] - F_MIN).abs() < 1e-9);
        assert!((limites[64] - F_MAX).abs() < 1.0);
    }

    #[test]
    fn banda_del_seno_es_la_correcta() {
        let limites = limites_bandas(64);
        let indice = limites
            .windows(2)
            .position(|limite| limite[0] <= 1000.0 && 1000.0 < limite[1])
            .expect("banda de 1 kHz");
        let magnitudes: Vec<f64> = (0..TAMANO_FFT / 2 + 1)
            .map(|bin| {
                let frecuencia = bin as f64 * 48_000.0 / TAMANO_FFT as f64;
                if (frecuencia - 1000.0).abs() < 25.0 {
                    1024.0
                } else {
                    1e-9
                }
            })
            .collect();
        let valor =
            banda_desde_magnitudes(&magnitudes, 48_000.0, limites[indice], limites[indice + 1]);
        assert!(
            valor > 0.85,
            "la banda del seno debe estar cerca del máximo"
        );
        let vecina = banda_desde_magnitudes(&magnitudes, 48_000.0, limites[0], limites[1]);
        assert_eq!(vecina, 0.0);
    }

    #[test]
    fn onda_rms_y_pico_basicos() {
        let estereo = [vec![0.5, -0.5, 0.25, -0.25], vec![0.0, 0.0, 0.0, 0.0]];
        let onda = muestrear_onda(&estereo, 4);
        assert_eq!(onda, vec![0.25, -0.25, 0.125, -0.125]);
        assert!(rms(&estereo[0], 4) > 0.0);
        assert_eq!(pico(&estereo[0], 4), 0.5);
    }
}
