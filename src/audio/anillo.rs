use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub const CAPACIDAD: usize = 8192;
pub const CANALES: usize = 2;

/// Anillo SPSC sin bloqueos: el callback de tiempo real de PipeWire escribe y
/// el hilo de UI lee la ventana más reciente. El productor nunca asigna memoria
/// ni se bloquea; el consumidor tolera leer mientras el productor sobrescribe.
pub struct Anillo {
    canales: [Vec<AtomicU32>; CANALES],
    escritas: AtomicU64,
}

impl Default for Anillo {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Anillo {
    pub fn nuevo() -> Self {
        let canal = || (0..CAPACIDAD).map(|_| AtomicU32::new(0)).collect();
        Self {
            canales: [canal(), canal()],
            escritas: AtomicU64::new(0),
        }
    }

    /// Escribe muestras intercaladas (L, R, L, R…).
    pub fn escribir(&self, intercalado: &[f32]) {
        let base = self.escritas.load(Ordering::Relaxed);
        let mut indice = 0u64;
        for valor in intercalado {
            self.guardar(base, indice, *valor);
            indice += 1;
        }
        self.escritas
            .store(base + indice / CANALES as u64, Ordering::Release);
    }

    /// Escribe muestras F32LE intercaladas directamente desde el buffer de
    /// PipeWire, sin decodificar ni asignar.
    pub fn escribir_bytes_f32_le(&self, datos: &[u8]) {
        let base = self.escritas.load(Ordering::Relaxed);
        let mut indice = 0u64;
        for trozo in datos.as_chunks::<4>().0 {
            let valor = f32::from_le_bytes(*trozo);
            self.guardar(base, indice, valor);
            indice += 1;
        }
        self.escritas
            .store(base + indice / CANALES as u64, Ordering::Release);
    }

    fn guardar(&self, base: u64, indice: u64, valor: f32) {
        let canal = (indice % CANALES as u64) as usize;
        let frame = (base + indice / CANALES as u64) % CAPACIDAD as u64;
        self.canales[canal][frame as usize].store(valor.to_bits(), Ordering::Relaxed);
    }

    pub fn frames_escritos(&self) -> u64 {
        self.escritas.load(Ordering::Acquire)
    }

    /// Últimas `n` muestras de un canal; rellena con ceros si aún no hay
    /// suficientes.
    pub fn ventana(&self, canal: usize, n: usize) -> Vec<f32> {
        let canal = canal.min(CANALES - 1);
        let total = self.frames_escritos();
        let disponibles = (total as usize).min(n);
        let base = total.saturating_sub(disponibles as u64);
        let mut salida = vec![0.0; n - disponibles];
        salida.reserve(disponibles);
        for i in 0..disponibles {
            let frame = ((base + i as u64) % CAPACIDAD as u64) as usize;
            let bits = self.canales[canal][frame].load(Ordering::Relaxed);
            salida.push(f32::from_bits(bits));
        }
        salida
    }

    pub fn ventana_estereo(&self, n: usize) -> [Vec<f32>; CANALES] {
        [self.ventana(0, n), self.ventana(1, n)]
    }

    /// Media de los dos canales: la mezcla a mono del análisis.
    pub fn ventana_mono(&self, n: usize) -> Vec<f32> {
        let [izquierda, derecha] = self.ventana_estereo(n);
        izquierda
            .iter()
            .zip(derecha.iter())
            .map(|(i, d)| (i + d) * 0.5)
            .collect()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn escribir_y_leer_ventana() {
        let anillo = Anillo::nuevo();
        anillo.escribir(&[0.1, -0.1, 0.2, -0.2, 0.3, -0.3]);
        assert_eq!(anillo.frames_escritos(), 3);
        assert_eq!(anillo.ventana(0, 3), vec![0.1, 0.2, 0.3]);
        assert_eq!(anillo.ventana(1, 3), vec![-0.1, -0.2, -0.3]);
        assert_eq!(anillo.ventana(0, 2), vec![0.2, 0.3]);
        assert_eq!(anillo.ventana(0, 5), vec![0.0, 0.0, 0.1, 0.2, 0.3]);
    }

    #[test]
    fn da_la_vuelta_al_anillo() {
        let anillo = Anillo::nuevo();
        for i in 0..CAPACIDAD + 10 {
            let valor = i as f32;
            anillo.escribir(&[valor, -valor]);
        }
        assert_eq!(anillo.frames_escritos(), (CAPACIDAD + 10) as u64);
        let ventana = anillo.ventana(0, 4);
        let inicio = (CAPACIDAD + 6) as f32;
        assert_eq!(
            ventana,
            vec![inicio, inicio + 1.0, inicio + 2.0, inicio + 3.0]
        );
    }

    #[test]
    fn decodifica_bytes_f32_le() {
        let anillo = Anillo::nuevo();
        let mut bytes = Vec::new();
        for valor in [0.5f32, -0.5, 1.0, -1.0] {
            bytes.extend_from_slice(&valor.to_le_bytes());
        }
        anillo.escribir_bytes_f32_le(&bytes);
        assert_eq!(anillo.ventana(0, 2), vec![0.5, 1.0]);
        assert_eq!(anillo.ventana(1, 2), vec![-0.5, -1.0]);
    }
}
