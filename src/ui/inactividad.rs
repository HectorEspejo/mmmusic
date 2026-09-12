use std::time::Instant;

/// Temporizador de inactividad del protector de pantalla.
pub struct Inactividad {
    ultimo: Instant,
}

impl Default for Inactividad {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Inactividad {
    pub fn nuevo() -> Self {
        Self {
            ultimo: Instant::now(),
        }
    }

    pub fn registrar(&mut self) {
        self.ultimo = Instant::now();
    }

    pub fn segundos(&self) -> u64 {
        self.ultimo.elapsed().as_secs()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn registra_actividad() {
        let mut inactividad = Inactividad::nuevo();
        assert_eq!(inactividad.segundos(), 0);
        inactividad.registrar();
        assert_eq!(inactividad.segundos(), 0);
    }
}
