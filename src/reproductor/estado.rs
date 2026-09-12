use std::fmt;

use crate::biblioteca::modelos::PistaResumen;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Estado {
    #[default]
    Detenido,
    Cargando,
    Reproduciendo,
    Pausado,
    Error,
}

impl Estado {
    pub fn etiqueta(self) -> &'static str {
        match self {
            Estado::Detenido => "detenido",
            Estado::Cargando => "cargando",
            Estado::Reproduciendo => "reproduciendo",
            Estado::Pausado => "pausado",
            Estado::Error => "error",
        }
    }

    pub fn esta_activo(self) -> bool {
        matches!(
            self,
            Estado::Cargando | Estado::Reproduciendo | Estado::Pausado
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Repeticion {
    #[default]
    No,
    Todo,
    Una,
}

impl Repeticion {
    pub fn ciclar(self) -> Self {
        match self {
            Repeticion::No => Repeticion::Todo,
            Repeticion::Todo => Repeticion::Una,
            Repeticion::Una => Repeticion::No,
        }
    }

    pub fn como_str(self) -> &'static str {
        match self {
            Repeticion::No => "no",
            Repeticion::Todo => "todo",
            Repeticion::Una => "una",
        }
    }

    pub fn desde_str(texto: &str) -> Option<Self> {
        match texto {
            "no" => Some(Repeticion::No),
            "todo" => Some(Repeticion::Todo),
            "una" => Some(Repeticion::Una),
            _ => None,
        }
    }
}

impl fmt::Display for Repeticion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.como_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstadoReproduccion {
    pub estado: Estado,
    pub pista_actual: Option<PistaResumen>,
    pub posicion_ms: i64,
    pub duracion_ms: i64,
    pub volumen: u8,
    pub silencio: bool,
    pub aleatorio: bool,
    pub repeticion: Repeticion,
    pub cola: Vec<PistaResumen>,
    pub cola_indice: Option<usize>,
}

impl Default for EstadoReproduccion {
    fn default() -> Self {
        Self {
            estado: Estado::Detenido,
            pista_actual: None,
            posicion_ms: 0,
            duracion_ms: 0,
            volumen: 80,
            silencio: false,
            aleatorio: false,
            repeticion: Repeticion::No,
            cola: Vec::new(),
            cola_indice: None,
        }
    }
}

impl EstadoReproduccion {
    pub fn pausado(&self) -> bool {
        matches!(self.estado, Estado::Pausado)
    }

    pub fn sonando(&self) -> bool {
        matches!(self.estado, Estado::Reproduciendo)
    }

    pub fn pista_cargada(&self) -> bool {
        self.pista_actual.is_some()
    }

    pub fn progreso(&self) -> f64 {
        if self.duracion_ms <= 0 {
            0.0
        } else {
            (self.posicion_ms as f64 / self.duracion_ms as f64).clamp(0.0, 1.0)
        }
    }
}
