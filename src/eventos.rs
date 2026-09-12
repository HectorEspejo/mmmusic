use crossterm::event::{KeyEvent, MouseEvent};

use crate::reproductor::estado::EstadoReproduccion;
use crate::tema::Paleta;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NivelAviso {
    Info,
    Aviso,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResumenEscaneo {
    pub nuevas: u64,
    pub actualizadas: u64,
    pub eliminadas: u64,
    pub omitidas: u64,
}

impl ResumenEscaneo {
    pub fn mensaje(&self) -> String {
        format!(
            "Escaneo: {} nuevas, {} actualizadas, {} eliminadas",
            self.nuevas, self.actualizadas, self.eliminadas
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventoEscaneo {
    Iniciado { total: usize },
    Progreso { procesadas: usize, total: usize },
    Terminado { resumen: ResumenEscaneo },
    Cancelado { resumen: ResumenEscaneo },
    Error { mensaje: String },
}

#[derive(Debug, Clone)]
pub enum AppEvento {
    Tecla(KeyEvent),
    Raton(MouseEvent),
    Redimension(u16, u16),
    Tick,
    Reproductor(EstadoReproduccion),
    Escaneo(EventoEscaneo),
    TemaActualizado(Paleta),
    Notificacion(NivelAviso, String),
    CaratulaLista(i64),
    Salir,
}
