//! Presets de fábrica y utilidades puras de PRESETS_EQ (formato y validación).
//! El CRUD vive en `biblioteca::consultas::presets_eq`.

use super::BANDAS;

pub struct PresetIntegrado {
    pub nombre: &'static str,
    pub ganancias: [f32; BANDAS],
    pub preamp_db: f32,
}

/// Valores de §7.2 del informe (dB por banda, 31 Hz → 16 kHz, y preamp).
pub const INTEGRADOS: [PresetIntegrado; 9] = [
    PresetIntegrado {
        nombre: "Plano",
        ganancias: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        preamp_db: 0.0,
    },
    PresetIntegrado {
        nombre: "Rock",
        ganancias: [5.0, 4.0, 2.0, -1.0, -2.0, 0.0, 1.0, 3.0, 4.0, 4.0],
        preamp_db: -2.0,
    },
    PresetIntegrado {
        nombre: "Pop",
        ganancias: [-1.0, 1.0, 3.0, 4.0, 3.0, 0.0, -1.0, -1.0, 1.0, 2.0],
        preamp_db: -1.0,
    },
    PresetIntegrado {
        nombre: "Electrónica",
        ganancias: [4.0, 3.0, 1.0, 0.0, -2.0, 1.0, 0.0, 1.0, 3.0, 4.0],
        preamp_db: -2.0,
    },
    PresetIntegrado {
        nombre: "Hip-hop",
        ganancias: [5.0, 4.0, 1.0, 2.0, -1.0, -1.0, 1.0, 0.0, 1.0, 2.0],
        preamp_db: -2.0,
    },
    PresetIntegrado {
        nombre: "Vocal",
        ganancias: [-2.0, -3.0, -2.0, 1.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0],
        preamp_db: 0.0,
    },
    PresetIntegrado {
        nombre: "Bass boost",
        ganancias: [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        preamp_db: -3.0,
    },
    PresetIntegrado {
        nombre: "Treble boost",
        ganancias: [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0],
        preamp_db: -3.0,
    },
    PresetIntegrado {
        nombre: "Loudness",
        ganancias: [5.0, 3.0, 0.0, -1.0, -2.0, -2.0, -1.0, 1.0, 3.0, 5.0],
        preamp_db: -3.0,
    },
];

/// Serializa las diez ganancias como valores separados por comas con un
/// decimal («5.0,4.0,…»).
pub fn formatear_ganancias(ganancias: &[f32; BANDAS]) -> String {
    ganancias
        .iter()
        .map(|ganancia| format!("{ganancia:.1}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Parsea «5.0,4.0,…»; exige exactamente diez valores.
pub fn parsear_ganancias(texto: &str) -> Option<[f32; BANDAS]> {
    let valores: Vec<f32> = texto
        .split(',')
        .map(str::trim)
        .map(str::parse::<f32>)
        .collect::<std::result::Result<_, _>>()
        .ok()?;
    let valores: [f32; BANDAS] = valores.try_into().ok()?;
    if valores.iter().all(|valor| valor.is_finite()) {
        Some(valores)
    } else {
        None
    }
}

/// Nombre de preset propio: 1–40 caracteres tras recortar.
pub fn validar_nombre(nombre: &str) -> std::result::Result<String, String> {
    let nombre = nombre.trim();
    if nombre.is_empty() {
        return Err("El nombre no puede estar vacío".to_string());
    }
    if nombre.chars().count() > 40 {
        return Err("El nombre no puede superar los 40 caracteres".to_string());
    }
    Ok(nombre.to_string())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_integrados_cubren_el_informe() {
        assert_eq!(INTEGRADOS.len(), 9);
        let rock = INTEGRADOS
            .iter()
            .find(|p| p.nombre == "Rock")
            .expect("Rock");
        assert_eq!(
            rock.ganancias,
            [5.0, 4.0, 2.0, -1.0, -2.0, 0.0, 1.0, 3.0, 4.0, 4.0]
        );
        assert_eq!(rock.preamp_db, -2.0);
        for preset in &INTEGRADOS {
            assert!(
                preset.ganancias.iter().all(|g| (-12.0..=12.0).contains(g)),
                "{} fuera de rango",
                preset.nombre
            );
            assert!((-12.0..=12.0).contains(&preset.preamp_db));
        }
    }

    #[test]
    fn formato_y_parseo_son_inversos() {
        let ganancias = [1.0, -2.5, 0.0, 12.0, -12.0, 0.5, 3.0, 4.0, 5.0, 6.0];
        let texto = formatear_ganancias(&ganancias);
        assert_eq!(texto, "1.0,-2.5,0.0,12.0,-12.0,0.5,3.0,4.0,5.0,6.0");
        assert_eq!(parsear_ganancias(&texto), Some(ganancias));
        assert_eq!(parsear_ganancias("1,2,3"), None);
        assert_eq!(parsear_ganancias(""), None);
    }

    #[test]
    fn valida_nombres_de_preset() {
        assert_eq!(
            validar_nombre("  Coche noche "),
            Ok("Coche noche".to_string())
        );
        assert!(validar_nombre("").is_err());
        assert!(validar_nombre("   ").is_err());
        assert!(validar_nombre(&"x".repeat(41)).is_err());
        assert!(validar_nombre(&"x".repeat(40)).is_ok());
    }
}
