//! ReplayGain nativo de mpv: modos y mapeo a propiedades.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModoReplayGain {
    #[default]
    No,
    Pista,
    Album,
}

impl ModoReplayGain {
    pub fn como_str(self) -> &'static str {
        match self {
            ModoReplayGain::No => "no",
            ModoReplayGain::Pista => "pista",
            ModoReplayGain::Album => "album",
        }
    }

    pub fn desde_str(texto: &str) -> Option<Self> {
        match texto {
            "no" => Some(ModoReplayGain::No),
            "pista" => Some(ModoReplayGain::Pista),
            "album" => Some(ModoReplayGain::Album),
            _ => None,
        }
    }

    pub fn ciclar(self) -> Self {
        match self {
            ModoReplayGain::No => ModoReplayGain::Pista,
            ModoReplayGain::Pista => ModoReplayGain::Album,
            ModoReplayGain::Album => ModoReplayGain::No,
        }
    }

    /// Valor de la propiedad `replaygain` de mpv.
    pub fn modo_mpv(self) -> &'static str {
        match self {
            ModoReplayGain::No => "no",
            ModoReplayGain::Pista => "track",
            ModoReplayGain::Album => "album",
        }
    }
}

/// Propiedad de mpv con su tipo, para aplicarla sin ambigüedad.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Propiedad {
    Texto(&'static str),
    Numero(f64),
}

/// Propiedades que se fijan siempre que cambia el modo o el preamp. El clip
/// se desactiva (el limitador ya protege) y el fallback queda en 0.
pub fn propiedades(modo: ModoReplayGain, preamp_db: f32) -> [(&'static str, Propiedad); 4] {
    [
        ("replaygain", Propiedad::Texto(modo.modo_mpv())),
        ("replaygain-preamp", Propiedad::Numero(f64::from(preamp_db))),
        ("replaygain-clip", Propiedad::Texto("no")),
        ("replaygain-fallback", Propiedad::Numero(0.0)),
    ]
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn modos_a_propiedades() {
        assert_eq!(ModoReplayGain::No.modo_mpv(), "no");
        assert_eq!(ModoReplayGain::Pista.modo_mpv(), "track");
        assert_eq!(ModoReplayGain::Album.modo_mpv(), "album");
        let props = propiedades(ModoReplayGain::Album, -3.5);
        assert_eq!(props[0], ("replaygain", Propiedad::Texto("album")));
        assert_eq!(props[1], ("replaygain-preamp", Propiedad::Numero(-3.5)));
        assert_eq!(props[2], ("replaygain-clip", Propiedad::Texto("no")));
        assert_eq!(props[3], ("replaygain-fallback", Propiedad::Numero(0.0)));
    }

    #[test]
    fn cicla_y_convierte() {
        assert_eq!(ModoReplayGain::No.ciclar(), ModoReplayGain::Pista);
        assert_eq!(ModoReplayGain::Pista.ciclar(), ModoReplayGain::Album);
        assert_eq!(ModoReplayGain::Album.ciclar(), ModoReplayGain::No);
        for modo in [
            ModoReplayGain::No,
            ModoReplayGain::Pista,
            ModoReplayGain::Album,
        ] {
            assert_eq!(ModoReplayGain::desde_str(modo.como_str()), Some(modo));
        }
        assert_eq!(ModoReplayGain::desde_str("otro"), None);
    }
}
