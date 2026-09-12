use std::time::Instant;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EstadoServicio {
    pub activo: bool,
    pub pendientes: u32,
    pub ultimo_envio: Option<Instant>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EstadoScrobbling {
    pub listenbrainz: EstadoServicio,
    pub lastfm: EstadoServicio,
    pub ultimo_error: Option<String>,
}

impl EstadoScrobbling {
    pub fn activo(&self) -> bool {
        self.listenbrainz.activo || self.lastfm.activo
    }

    pub fn pendientes(&self) -> u32 {
        self.listenbrainz.pendientes + self.lastfm.pendientes
    }

    pub fn error(&self) -> Option<&str> {
        self.listenbrainz
            .error
            .as_deref()
            .or(self.lastfm.error.as_deref())
            .or(self.ultimo_error.as_deref())
    }

    pub fn ultimo_envio(&self) -> Option<Instant> {
        match (self.listenbrainz.ultimo_envio, self.lastfm.ultimo_envio) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }
}
