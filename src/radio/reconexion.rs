use std::time::Duration;

/// Esperas de reconexión en segundos para los intentos 1..6.
pub const ESPERAS: [u64; 6] = [1, 2, 4, 8, 16, 30];
pub const MAX_INTENTOS: u32 = 6;

/// Espera del intento indicado (1..=6); `None` significa rendido.
pub fn siguiente_espera(intento: u32) -> Option<Duration> {
    if intento == 0 {
        return None;
    }
    ESPERAS
        .get((intento - 1) as usize)
        .map(|segundos| Duration::from_secs(*segundos))
}

/// Plan de reconexión de un stream: cuenta intentos y avanza de espejo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanReconexion {
    intento: u32,
    espejo: usize,
    num_espejos: usize,
}

impl PlanReconexion {
    pub fn nuevo(num_espejos: usize) -> Self {
        Self {
            intento: 0,
            espejo: 0,
            num_espejos: num_espejos.max(1),
        }
    }

    pub fn intento(&self) -> u32 {
        self.intento
    }

    pub fn espejo(&self) -> usize {
        self.espejo
    }

    /// Se llama al alcanzar `en_directo`: el contador vuelve a cero.
    pub fn reiniciar(&mut self) {
        self.intento = 0;
    }

    /// Registra un fallo. Devuelve el número de intento (1..=6) y deja
    /// preparado el siguiente espejo; `None` si hay que darse por rendido.
    pub fn registrar_fallo(&mut self) -> Option<u32> {
        if self.intento >= MAX_INTENTOS {
            return None;
        }
        self.intento += 1;
        if self.num_espejos > 1 {
            self.espejo = (self.espejo + 1) % self.num_espejos;
        }
        Some(self.intento)
    }

    pub fn espera(&self) -> Option<Duration> {
        siguiente_espera(self.intento)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn esperas_crecientes_y_rendido() {
        assert_eq!(siguiente_espera(0), None);
        assert_eq!(siguiente_espera(1), Some(Duration::from_secs(1)));
        assert_eq!(siguiente_espera(2), Some(Duration::from_secs(2)));
        assert_eq!(siguiente_espera(3), Some(Duration::from_secs(4)));
        assert_eq!(siguiente_espera(4), Some(Duration::from_secs(8)));
        assert_eq!(siguiente_espera(5), Some(Duration::from_secs(16)));
        assert_eq!(siguiente_espera(6), Some(Duration::from_secs(30)));
        assert_eq!(siguiente_espera(7), None);
    }

    #[test]
    fn seis_fallos_agotan_el_plan() {
        let mut plan = PlanReconexion::nuevo(1);
        for intento in 1..=MAX_INTENTOS {
            assert_eq!(plan.registrar_fallo(), Some(intento));
            assert_eq!(plan.espera(), siguiente_espera(intento));
        }
        assert_eq!(plan.registrar_fallo(), None);
    }

    #[test]
    fn avanza_de_espejo_y_envuelve() {
        let mut plan = PlanReconexion::nuevo(3);
        assert_eq!(plan.espejo(), 0);
        plan.registrar_fallo();
        assert_eq!(plan.espejo(), 1);
        plan.registrar_fallo();
        assert_eq!(plan.espejo(), 2);
        plan.registrar_fallo();
        assert_eq!(plan.espejo(), 0);
    }

    #[test]
    fn reiniciar_vuelve_el_contador_a_cero() {
        let mut plan = PlanReconexion::nuevo(2);
        plan.registrar_fallo();
        plan.registrar_fallo();
        plan.reiniciar();
        assert_eq!(plan.intento(), 0);
        assert_eq!(plan.registrar_fallo(), Some(1));
    }
}
