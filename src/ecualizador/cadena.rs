//! Construcción de la cadena `af` de mpv y de los comandos en caliente.
//! Sin E/S: todo es puro para poder probarlo sin audio.

use super::EstadoEq;

pub const BANDAS_HZ: [u32; 10] = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
pub const Q: f32 = 1.41;
/// Techo del limitador: −1 dBFS en escala lineal.
pub const TECHO_LIMITADOR: f32 = 0.891;

/// Cadena etiquetada completa: `@pre` (volume), `@eq1`…`@eq10` (equalizer) y
/// `@lim` (alimiter) cuando el limitador está activo.
pub fn construir(eq: &EstadoEq) -> String {
    let mut partes = vec![format!("@pre:lavfi=[volume=volume={:.1}dB]", eq.preamp_db)];
    for (indice, (hz, ganancia)) in BANDAS_HZ.iter().zip(eq.ganancias.iter()).enumerate() {
        partes.push(format!(
            "@eq{}:lavfi=[equalizer=f={hz}:width_type=q:width={Q:.2}:gain={ganancia:.1}]",
            indice + 1
        ));
    }
    if eq.limitador {
        partes.push(format!(
            "@lim:lavfi=[alimiter=limit={TECHO_LIMITADOR:.3}:attack=5:release=50:level=false]"
        ));
    }
    partes.join(",")
}

/// Comando en caliente para una banda (índice 0..9 → `@eq1`…`@eq10`). El
/// cuarto valor es el `target` que exige mpv: el nombre del filtro FFmpeg.
pub fn comando_banda(indice: usize, db: f32) -> (String, &'static str, String, &'static str) {
    (
        format!("@eq{}", indice + 1),
        "g",
        format!("{db:.1}"),
        "equalizer",
    )
}

/// Comando en caliente del preamp.
pub fn comando_preamp(db: f32) -> (&'static str, &'static str, String, &'static str) {
    ("@pre", "volume", format!("{db:.1}dB"), "volume")
}

/// Bypass total: sin EQ activo, o curva plana sin limitador. En ese caso mpv
/// queda con `af = ""` y no gasta CPU.
pub fn es_bypass(eq: &EstadoEq) -> bool {
    if !eq.activo {
        return true;
    }
    let plana = eq.ganancias.iter().all(|ganancia| ganancia.abs() < 0.05);
    plana && eq.preamp_db.abs() < 0.05 && !eq.limitador
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn eq_plano() -> EstadoEq {
        EstadoEq {
            activo: true,
            ..EstadoEq::default()
        }
    }

    #[test]
    fn cadena_con_todas_las_etiquetas() {
        let mut eq = eq_plano();
        eq.ganancias[0] = 5.0;
        eq.ganancias[9] = -3.5;
        eq.preamp_db = -2.0;
        let cadena = construir(&eq);
        assert!(cadena.starts_with("@pre:lavfi=[volume=volume=-2.0dB]"));
        assert!(cadena.contains("@eq1:lavfi=[equalizer=f=31:width_type=q:width=1.41:gain=5.0]"));
        assert!(
            cadena.contains("@eq10:lavfi=[equalizer=f=16000:width_type=q:width=1.41:gain=-3.5]")
        );
        assert!(!cadena.contains("@lim"));
    }

    #[test]
    fn limitador_se_anade_al_final() {
        let mut eq = eq_plano();
        eq.limitador = true;
        let cadena = construir(&eq);
        assert!(
            cadena.ends_with("@lim:lavfi=[alimiter=limit=0.891:attack=5:release=50:level=false]")
        );
    }

    #[test]
    fn comandos_en_caliente() {
        assert_eq!(
            comando_banda(0, 3.0),
            (
                "@eq1".to_string(),
                "g",
                "3.0".to_string(),
                "equalizer" as &'static str
            )
        );
        assert_eq!(
            comando_banda(9, -1.5),
            (
                "@eq10".to_string(),
                "g",
                "-1.5".to_string(),
                "equalizer" as &'static str
            )
        );
        assert_eq!(
            comando_preamp(2.0),
            ("@pre", "volume", "2.0dB".to_string(), "volume")
        );
    }

    #[test]
    fn bypass_solo_con_curva_plana() {
        assert!(es_bypass(&eq_plano()));
        let mut eq = eq_plano();
        eq.activo = false;
        assert!(es_bypass(&eq));
        eq.activo = true;
        eq.limitador = true;
        assert!(!es_bypass(&eq));
        eq.limitador = false;
        eq.ganancias[4] = 0.5;
        assert!(!es_bypass(&eq));
        eq.ganancias[4] = 0.0;
        eq.preamp_db = -1.0;
        assert!(!es_bypass(&eq));
    }
}
