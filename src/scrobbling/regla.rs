pub const DURACION_MINIMA_MS: i64 = 30_000;
pub const UMBRAL_MAXIMO_MS: i64 = 240_000;

pub fn elegible(duracion_ms: i64) -> bool {
    duracion_ms > DURACION_MINIMA_MS
}

pub fn umbral_ms(duracion_ms: i64) -> i64 {
    (duracion_ms / 2).clamp(0, UMBRAL_MAXIMO_MS)
}

pub fn debe_scrobblear(duracion_ms: i64, tiempo_real_ms: i64) -> bool {
    elegible(duracion_ms) && tiempo_real_ms >= umbral_ms(duracion_ms)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn pistas_cortas_no_son_elegibles() {
        assert!(!elegible(0));
        assert!(!elegible(30_000));
        assert!(elegible(30_001));
        assert!(elegible(300_000));
    }

    #[test]
    fn umbral_es_la_mitad_o_cuatro_minutos() {
        assert_eq!(umbral_ms(30_001), 15_000);
        assert_eq!(umbral_ms(120_000), 60_000);
        assert_eq!(umbral_ms(480_000), 240_000);
        assert_eq!(umbral_ms(1_200_000), 240_000);
        assert_eq!(umbral_ms(-5), 0);
    }

    #[test]
    fn debe_scrobblear_exige_elegibilidad_y_tiempo() {
        assert!(!debe_scrobblear(20_000, 20_000));
        assert!(!debe_scrobblear(100_000, 49_999));
        assert!(debe_scrobblear(100_000, 50_000));
        assert!(!debe_scrobblear(600_000, 239_999));
        assert!(debe_scrobblear(600_000, 240_000));
        assert!(debe_scrobblear(600_000, 500_000));
    }
}
