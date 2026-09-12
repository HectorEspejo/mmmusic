use std::time::Duration;

use crate::biblioteca::bd;

pub const MAX_INTENTOS: u32 = 20;
pub const ESPERA_BASE_S: u64 = 30;
pub const TOPE_ESPERA_S: u64 = 6 * 3600;
pub const ESPERA_AUTH_S: i64 = 24 * 3600;
pub const LIMITE_ANTIGUEDAD_S: i64 = 14 * 86_400;
pub const MARGEN_FUTURO_S: i64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clasificacion {
    Enviado,
    Reintentar,
    Autenticacion,
    Descartar,
}

pub fn espera_reintento(intentos_previos: u32) -> Duration {
    let segundos = ESPERA_BASE_S.saturating_mul(1u64 << intentos_previos.min(20));
    Duration::from_secs(segundos.min(TOPE_ESPERA_S))
}

pub fn descartar_por_intentos(intentos_nuevos: u32) -> bool {
    intentos_nuevos > MAX_INTENTOS
}

pub fn es_demasiado_antiguo(unix: i64, ahora: i64) -> bool {
    ahora.saturating_sub(unix) > LIMITE_ANTIGUEDAD_S
}

pub fn es_futuro(unix: i64, ahora: i64) -> bool {
    unix > ahora.saturating_add(MARGEN_FUTURO_S)
}

pub fn proximo_reintento(intentos_previos: u32) -> String {
    bd::iso_en(espera_reintento(intentos_previos).as_secs() as i64)
}

pub fn proximo_auth() -> String {
    bd::iso_en(ESPERA_AUTH_S)
}

pub fn clasificar(status: u16, codigo_servicio: Option<i32>) -> Clasificacion {
    if (200..300).contains(&status) {
        return Clasificacion::Enviado;
    }
    if status == 401 || status == 403 {
        return Clasificacion::Autenticacion;
    }
    match codigo_servicio {
        Some(4 | 9 | 14) => Clasificacion::Autenticacion,
        Some(13) => Clasificacion::Descartar,
        _ => Clasificacion::Reintentar,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn reintentos_exponenciales_con_tope() {
        assert_eq!(espera_reintento(0), Duration::from_secs(30));
        assert_eq!(espera_reintento(1), Duration::from_secs(60));
        assert_eq!(espera_reintento(2), Duration::from_secs(120));
        assert_eq!(espera_reintento(5), Duration::from_secs(960));
        assert_eq!(espera_reintento(10), Duration::from_secs(6 * 3600));
        assert_eq!(espera_reintento(30), Duration::from_secs(6 * 3600));
    }

    #[test]
    fn descarte_pasados_los_veinte_intentos() {
        assert!(!descartar_por_intentos(1));
        assert!(!descartar_por_intentos(20));
        assert!(descartar_por_intentos(21));
    }

    #[test]
    fn detecta_scrobbles_antiguos_y_futuros() {
        assert!(es_demasiado_antiguo(0, LIMITE_ANTIGUEDAD_S + 1));
        assert!(!es_demasiado_antiguo(0, LIMITE_ANTIGUEDAD_S));
        assert!(es_futuro(1_000, 200));
        assert!(!es_futuro(1_000, 1_000));
    }

    #[test]
    fn clasifica_respuestas() {
        assert_eq!(clasificar(200, None), Clasificacion::Enviado);
        assert_eq!(clasificar(204, None), Clasificacion::Enviado);
        assert_eq!(clasificar(401, None), Clasificacion::Autenticacion);
        assert_eq!(clasificar(403, None), Clasificacion::Autenticacion);
        assert_eq!(clasificar(400, Some(4)), Clasificacion::Autenticacion);
        assert_eq!(clasificar(400, Some(9)), Clasificacion::Autenticacion);
        assert_eq!(clasificar(400, Some(14)), Clasificacion::Autenticacion);
        assert_eq!(clasificar(400, Some(13)), Clasificacion::Descartar);
        assert_eq!(clasificar(400, Some(6)), Clasificacion::Reintentar);
        assert_eq!(clasificar(500, None), Clasificacion::Reintentar);
        assert_eq!(clasificar(429, None), Clasificacion::Reintentar);
        assert_eq!(clasificar(0, None), Clasificacion::Reintentar);
    }
}
