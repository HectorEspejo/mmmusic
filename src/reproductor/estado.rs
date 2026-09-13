use std::fmt;

use crate::biblioteca::modelos::{ElementoCola, EmisoraResumen, PistaResumen};
use crate::ecualizador::EstadoEq;

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

/// Estado de conexión de un stream de radio.
#[derive(Debug, Clone, PartialEq)]
pub enum EstadoStream {
    Conectando,
    Almacenando { segundos: f32 },
    EnDirecto,
    Reconectando(u32),
    Rendido,
}

impl EstadoStream {
    pub fn etiqueta(&self) -> &'static str {
        match self {
            EstadoStream::Conectando => "conectando",
            EstadoStream::Almacenando { .. } => "almacenando",
            EstadoStream::EnDirecto => "EN DIRECTO",
            EstadoStream::Reconectando(_) => "reconectando",
            EstadoStream::Rendido => "rendido",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EstadoReproduccion {
    pub estado: Estado,
    pub elemento: Option<ElementoCola>,
    pub posicion_ms: i64,
    pub duracion_ms: i64,
    pub volumen: u8,
    pub silencio: bool,
    pub aleatorio: bool,
    pub repeticion: Repeticion,
    pub cola: Vec<ElementoCola>,
    pub cola_indice: Option<usize>,
    pub stream: Option<EstadoStream>,
    pub titulo_icy: Option<String>,
    pub tiempo_escuchando_ms: i64,
    pub codec: Option<String>,
    pub bitrate_kbps: Option<i64>,
    pub cache_segundos: f32,
    pub reconexiones: u32,
    pub eq: EstadoEq,
}

impl Default for EstadoReproduccion {
    fn default() -> Self {
        Self {
            estado: Estado::Detenido,
            elemento: None,
            posicion_ms: 0,
            duracion_ms: 0,
            volumen: 80,
            silencio: false,
            aleatorio: false,
            repeticion: Repeticion::No,
            cola: Vec::new(),
            cola_indice: None,
            stream: None,
            titulo_icy: None,
            tiempo_escuchando_ms: 0,
            codec: None,
            bitrate_kbps: None,
            cache_segundos: 0.0,
            reconexiones: 0,
            eq: EstadoEq::default(),
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
        self.elemento.is_some()
    }

    pub fn pista_actual(&self) -> Option<&PistaResumen> {
        self.elemento.as_ref().and_then(ElementoCola::pista)
    }

    pub fn emisora_actual(&self) -> Option<&EmisoraResumen> {
        self.elemento.as_ref().and_then(ElementoCola::emisora)
    }

    pub fn es_emisora(&self) -> bool {
        self.elemento.as_ref().is_some_and(ElementoCola::es_emisora)
    }

    pub fn en_directo(&self) -> bool {
        matches!(self.stream, Some(EstadoStream::EnDirecto))
    }

    /// Título a mostrar en la interfaz: ICY si lo hay, si no el nombre del
    /// elemento (título de pista o nombre de emisora).
    pub fn titulo_mostrado(&self) -> Option<&str> {
        self.titulo_icy
            .as_deref()
            .or_else(|| self.elemento.as_ref().map(ElementoCola::titulo))
    }

    pub fn progreso(&self) -> f64 {
        if self.es_emisora() || self.duracion_ms <= 0 {
            0.0
        } else {
            (self.posicion_ms as f64 / self.duracion_ms as f64).clamp(0.0, 1.0)
        }
    }
}
