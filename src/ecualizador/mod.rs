pub mod cadena;
pub mod presets;
pub mod replaygain;

use replaygain::ModoReplayGain;

pub const BANDAS: usize = 10;
pub const MIN_DB: f32 = -12.0;
pub const MAX_DB: f32 = 12.0;
pub const PASO_DB: f32 = 0.5;
pub const MIN_PREAMP_DB: f32 = -12.0;
pub const MAX_PREAMP_DB: f32 = 12.0;

/// Estado del ecualizador que viaja dentro de `EstadoReproduccion`. La UI lo
/// dibuja y el hilo reproductor lo modifica al atender `ComandoEq`.
#[derive(Debug, Clone, PartialEq)]
pub struct EstadoEq {
    pub activo: bool,
    pub ganancias: [f32; BANDAS],
    pub preamp_db: f32,
    pub limitador: bool,
    pub preset: Option<(i64, String)>,
    pub replaygain: ModoReplayGain,
    pub replaygain_preamp_db: f32,
    /// false si mpv no trae lavfi: el overlay lo explica y ReplayGain sigue.
    pub disponible: bool,
    /// false si la pista en curso no tiene etiquetas ReplayGain.
    pub tiene_replaygain: bool,
}

impl Default for EstadoEq {
    fn default() -> Self {
        Self {
            activo: false,
            ganancias: [0.0; BANDAS],
            preamp_db: 0.0,
            limitador: false,
            preset: None,
            replaygain: ModoReplayGain::No,
            replaygain_preamp_db: 0.0,
            disponible: true,
            tiene_replaygain: false,
        }
    }
}

/// Comandos de la UI al hilo reproductor.
#[derive(Debug, Clone, PartialEq)]
pub enum ComandoEq {
    Activar(bool),
    Banda { indice: usize, db: f32 },
    Preamp(f32),
    Limitador(bool),
    Preset(i64),
    Restablecer,
    ReplayGain(ModoReplayGain),
    ReplayGainPreamp(f32),
}

/// Acota a −12..12 dB y redondea al paso de 0,5.
pub fn acotar_db(db: f32) -> f32 {
    if !db.is_finite() {
        return 0.0;
    }
    (db.clamp(MIN_DB, MAX_DB) / PASO_DB).round() * PASO_DB
}

/// Acota el preamp a −12..12 dB (paso de 0,5).
pub fn acotar_preamp(db: f32) -> f32 {
    if !db.is_finite() {
        return 0.0;
    }
    (db.clamp(MIN_PREAMP_DB, MAX_PREAMP_DB) / PASO_DB).round() * PASO_DB
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn acota_y_redondea_al_medio_db() {
        assert_eq!(acotar_db(100.0), 12.0);
        assert_eq!(acotar_db(-100.0), -12.0);
        assert_eq!(acotar_db(0.24), 0.0);
        assert_eq!(acotar_db(0.26), 0.5);
        assert_eq!(acotar_db(f32::NAN), 0.0);
        assert_eq!(acotar_preamp(3.3), 3.5);
        assert_eq!(acotar_preamp(-99.0), -12.0);
    }
}
