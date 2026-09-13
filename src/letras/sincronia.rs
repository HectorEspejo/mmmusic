//! Sincronía de letras: índice de la última línea cuyo tiempo no supera `t`.
//! Búsqueda binaria, sin E/S.

/// Devuelve el índice de la última línea con marca `<= t_ms`, o −1 si `t`
/// queda antes de la primera.
pub fn linea_actual(lineas: &[(u32, String)], t_ms: i64) -> i64 {
    let mut bajo = 0isize;
    let mut alto = lineas.len() as isize - 1;
    let mut resultado = -1i64;
    while bajo <= alto {
        let medio = (bajo + alto) / 2;
        if i64::from(lineas[medio as usize].0) <= t_ms {
            resultado = medio as i64;
            bajo = medio + 1;
        } else {
            alto = medio - 1;
        }
    }
    resultado
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn lineas() -> Vec<(u32, String)> {
        vec![
            (0, "cero".to_string()),
            (1_000, "uno".to_string()),
            (2_500, "dos".to_string()),
            (2_500, "dos bis".to_string()),
            (5_000, "tres".to_string()),
        ]
    }

    #[test]
    fn encuentra_la_ultima_linea_que_no_supera_el_tiempo() {
        let lineas = lineas();
        assert_eq!(linea_actual(&lineas, -500), -1);
        assert_eq!(linea_actual(&lineas, 0), 0);
        assert_eq!(linea_actual(&lineas, 999), 0);
        assert_eq!(linea_actual(&lineas, 1_000), 1);
        assert_eq!(linea_actual(&lineas, 2_999), 3);
        assert_eq!(linea_actual(&lineas, 5_000), 4);
        assert_eq!(linea_actual(&lineas, 999_999), 4);
    }

    #[test]
    fn lista_vacia_o_sin_marcas_anteriores() {
        assert_eq!(linea_actual(&[], 100), -1);
        let lineas = vec![(10_000, "diez".to_string())];
        assert_eq!(linea_actual(&lineas, 1_000), -1);
        assert_eq!(linea_actual(&lineas, 10_000), 0);
    }
}
