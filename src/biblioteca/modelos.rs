use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artista {
    pub id: i64,
    pub nombre: String,
    pub nombre_norm: String,
    pub creado_en: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArtistaResumen {
    pub id: i64,
    pub nombre: String,
    pub num_albumes: i64,
    pub num_pistas: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Album {
    pub id: i64,
    pub artista_id: i64,
    pub titulo: String,
    pub titulo_norm: String,
    pub anio: Option<i64>,
    pub caratula_ruta: Option<String>,
    pub varios_artistas: bool,
    pub carpeta: Option<String>,
    pub creado_en: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AlbumResumen {
    pub id: i64,
    pub titulo: String,
    pub artista: String,
    pub anio: Option<i64>,
    pub caratula_ruta: Option<String>,
    pub num_pistas: i64,
    pub duracion_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DetalleAlbum {
    pub album: AlbumResumen,
    pub pistas: Vec<Pista>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DetalleArtista {
    pub artista: ArtistaResumen,
    pub albumes: Vec<AlbumResumen>,
    pub pistas_sueltas: Vec<PistaListado>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Inicio {
    pub recientes: Vec<PistaListado>,
    pub anadidos: Vec<AlbumResumen>,
    pub redescubre: Vec<AlbumResumen>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pista {
    pub id: i64,
    pub album_id: i64,
    pub artista_id: i64,
    pub titulo: String,
    pub titulo_norm: String,
    pub numero_pista: Option<i64>,
    pub numero_disco: Option<i64>,
    pub genero: Option<String>,
    pub carpeta: String,
    pub artista_album_etiquetado: bool,
    pub duracion_ms: i64,
    pub ruta: String,
    pub formato: String,
    pub tamano_bytes: i64,
    pub modificado_en: i64,
    pub bitrate_kbps: Option<i64>,
    pub anadido_en: String,
    pub escaneo_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PistaResumen {
    pub id: i64,
    pub titulo: String,
    pub artista: String,
    pub album: String,
    pub album_id: i64,
    pub duracion_ms: i64,
    pub caratula_ruta: Option<String>,
    pub ruta: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PistaListado {
    pub id: i64,
    pub titulo: String,
    pub artista: String,
    pub album: String,
    pub album_id: i64,
    pub duracion_ms: i64,
    pub anio: Option<i64>,
    pub formato: String,
    pub caratula_ruta: Option<String>,
    pub ruta: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Playlist {
    pub id: i64,
    pub nombre: String,
    pub creado_en: String,
    pub actualizado_en: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlaylistResumen {
    pub id: i64,
    pub nombre: String,
    pub num_pistas: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoEscaneo {
    EnCurso,
    Completado,
    Error,
    Cancelado,
}

impl EstadoEscaneo {
    pub fn como_str(self) -> &'static str {
        match self {
            EstadoEscaneo::EnCurso => "en_curso",
            EstadoEscaneo::Completado => "completado",
            EstadoEscaneo::Error => "error",
            EstadoEscaneo::Cancelado => "cancelado",
        }
    }

    pub fn desde_str(texto: &str) -> Option<Self> {
        match texto {
            "en_curso" => Some(EstadoEscaneo::EnCurso),
            "completado" => Some(EstadoEscaneo::Completado),
            "error" => Some(EstadoEscaneo::Error),
            "cancelado" => Some(EstadoEscaneo::Cancelado),
            _ => None,
        }
    }
}

impl fmt::Display for EstadoEscaneo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.como_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Escaneo {
    pub id: i64,
    pub iniciado_en: String,
    pub finalizado_en: Option<String>,
    pub estado: EstadoEscaneo,
    pub nuevas: i64,
    pub actualizadas: i64,
    pub eliminadas: i64,
    pub error_msg: Option<String>,
}
