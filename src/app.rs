use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crate::audio::anillo::Anillo;
use crate::audio::{Analisis, Analizador, EstadoCaptura};
use crate::biblioteca::bd;
use crate::biblioteca::consultas;
use crate::biblioteca::consultas::emisoras::{NuevaEmisora, OrdenEmisoras};
use crate::biblioteca::consultas::{LIMITE_BUSQUEDA, ResultadosBusqueda};
use crate::biblioteca::consultas::{OrdenAlbumes, OrdenPistas};
use crate::biblioteca::escaner::{self, ManejoEscaneo};
use crate::biblioteca::modelos::{
    AlbumResumen, ArtistaResumen, DetalleAlbum, DetalleArtista, ElementoCola, EmisoraResumen,
    Inicio, PistaListado, PlaylistResumen, TituloEmisora,
};
use crate::config::{Config, FuentePaleta, ModoIconos};
use crate::eventos::{AppEvento, EventoEscaneo, NivelAviso};
use crate::radio::icy;
use crate::radio::radiobrowser::{self, ComandoDirectorio, EmisoraDirectorio, ManejoDirectorio};
use crate::reproductor::estado::{Estado, EstadoReproduccion};
use crate::reproductor::{ComandoReproductor, ManejoReproductor};
use crate::scrobbling::estado::EstadoScrobbling;
use crate::scrobbling::{ComandoScrobbling, ManejoScrobbling};
use crate::tema::{self, Paleta};
use crate::ui::Iconos;
use crate::ui::componentes::imagen::CacheCaratulas;
use crate::ui::inactividad::Inactividad;
use crate::ui::teclas::{Accion, traducir};
use crate::visuales;
use crate::visuales::paleta::Paleta as PaletaVisual;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use rusqlite::Connection;
use tokio::sync::watch;

const DURACION_TOAST: Duration = Duration::from_secs(3);
const PAGINA_SALTOS: usize = 10;
const TICK_BASE: Duration = Duration::from_millis(250);
const TICK_DEGRADADO: Duration = Duration::from_millis(66);
const DURACION_CABECERA: Duration = Duration::from_secs(3);
const DURACION_TRANSICION_PALETA: f32 = 0.4;
const UMBRAL_FRAME_LENTO: Duration = Duration::from_millis(33);
const MARGEN_RECUPERACION: Duration = Duration::from_secs(10);
pub const PLAYLIST_FAVORITAS: i64 = -1;
pub const NOMBRE_FAVORITAS: &str = "♥ Favoritas";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vista {
    Inicio,
    Buscar,
    Artistas,
    Albumes,
    Pistas,
    Playlists,
    Visual,
    Radio,
}

impl Vista {
    pub const TODAS: [Vista; 8] = [
        Vista::Inicio,
        Vista::Buscar,
        Vista::Artistas,
        Vista::Albumes,
        Vista::Pistas,
        Vista::Playlists,
        Vista::Visual,
        Vista::Radio,
    ];

    pub fn numero(self) -> usize {
        match self {
            Vista::Inicio => 1,
            Vista::Buscar => 2,
            Vista::Artistas => 3,
            Vista::Albumes => 4,
            Vista::Pistas => 5,
            Vista::Playlists => 6,
            Vista::Visual => 7,
            Vista::Radio => 8,
        }
    }

    pub fn titulo(self) -> &'static str {
        match self {
            Vista::Inicio => "Inicio",
            Vista::Buscar => "Buscar",
            Vista::Artistas => "Artistas",
            Vista::Albumes => "Álbumes",
            Vista::Pistas => "Pistas",
            Vista::Playlists => "Playlists",
            Vista::Visual => "Visual",
            Vista::Radio => "Radio",
        }
    }

    pub fn desde_numero(numero: usize) -> Option<Vista> {
        Vista::TODAS.get(numero.checked_sub(1)?).copied()
    }
}

/// Estado del modo visual a pantalla completa. Cuando `protector` es true,
/// cualquier tecla o clic sale y se consume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModoVisual {
    pub protector: bool,
    vista_previa: Vista,
    pantalla_previa: Pantalla,
    seleccion_previa: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pantalla {
    Lista,
    DetalleArtista,
    DetalleAlbum,
    DetallePlaylist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PestanaRadio {
    #[default]
    Favoritas,
    Todas,
    Buscar,
    Sonando,
}

impl PestanaRadio {
    pub const TODAS: [PestanaRadio; 4] = [
        PestanaRadio::Favoritas,
        PestanaRadio::Todas,
        PestanaRadio::Buscar,
        PestanaRadio::Sonando,
    ];

    pub fn indice(self) -> usize {
        match self {
            PestanaRadio::Favoritas => 0,
            PestanaRadio::Todas => 1,
            PestanaRadio::Buscar => 2,
            PestanaRadio::Sonando => 3,
        }
    }

    pub fn titulo(self) -> &'static str {
        match self {
            PestanaRadio::Favoritas => "Favoritas",
            PestanaRadio::Todas => "Todas",
            PestanaRadio::Buscar => "Buscar",
            PestanaRadio::Sonando => "Sonando",
        }
    }

    pub fn como_str(self) -> &'static str {
        match self {
            PestanaRadio::Favoritas => "favoritas",
            PestanaRadio::Todas => "todas",
            PestanaRadio::Buscar => "buscar",
            PestanaRadio::Sonando => "sonando",
        }
    }

    pub fn desde_str(texto: &str) -> Option<Self> {
        match texto {
            "favoritas" => Some(PestanaRadio::Favoritas),
            "todas" => Some(PestanaRadio::Todas),
            "buscar" => Some(PestanaRadio::Buscar),
            "sonando" => Some(PestanaRadio::Sonando),
            _ => None,
        }
    }

    pub fn anterior(self) -> Self {
        let indice = (self.indice() + 3) % 4;
        PestanaRadio::TODAS[indice]
    }

    pub fn siguiente(self) -> Self {
        let indice = (self.indice() + 1) % 4;
        PestanaRadio::TODAS[indice]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampoBusquedaRadio {
    Nombre,
    Pais,
    Etiqueta,
}

impl CampoBusquedaRadio {
    pub fn etiqueta(self) -> &'static str {
        match self {
            CampoBusquedaRadio::Nombre => "Nombre",
            CampoBusquedaRadio::Pais => "País",
            CampoBusquedaRadio::Etiqueta => "Etiqueta",
        }
    }

    pub fn siguiente(self) -> Self {
        match self {
            CampoBusquedaRadio::Nombre => CampoBusquedaRadio::Pais,
            CampoBusquedaRadio::Pais => CampoBusquedaRadio::Etiqueta,
            CampoBusquedaRadio::Etiqueta => CampoBusquedaRadio::Nombre,
        }
    }

    pub fn anterior(self) -> Self {
        match self {
            CampoBusquedaRadio::Nombre => CampoBusquedaRadio::Etiqueta,
            CampoBusquedaRadio::Pais => CampoBusquedaRadio::Nombre,
            CampoBusquedaRadio::Etiqueta => CampoBusquedaRadio::Pais,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BusquedaRadio {
    pub nombre: String,
    pub pais: String,
    pub etiqueta: String,
    pub campo: Option<CampoBusquedaRadio>,
    pub resultados: Vec<EmisoraDirectorio>,
    pub buscando: bool,
    pub aviso: Option<String>,
}

impl BusquedaRadio {
    pub fn clave(&self) -> String {
        radiobrowser::clave_busqueda(&self.nombre, &self.pais, &self.etiqueta)
    }

    pub fn valor_mut(&mut self, campo: CampoBusquedaRadio) -> &mut String {
        match campo {
            CampoBusquedaRadio::Nombre => &mut self.nombre,
            CampoBusquedaRadio::Pais => &mut self.pais,
            CampoBusquedaRadio::Etiqueta => &mut self.etiqueta,
        }
    }

    pub fn valor(&self, campo: CampoBusquedaRadio) -> &str {
        match campo {
            CampoBusquedaRadio::Nombre => &self.nombre,
            CampoBusquedaRadio::Pais => &self.pais,
            CampoBusquedaRadio::Etiqueta => &self.etiqueta,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampoDialogo {
    pub etiqueta: String,
    pub valor: String,
    pub pista: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialogo {
    Confirmacion {
        titulo: String,
        mensaje: String,
        accion: AccionDialogo,
    },
    Texto {
        titulo: String,
        valor: String,
        accion: AccionDialogo,
    },
    Formulario {
        titulo: String,
        campos: Vec<CampoDialogo>,
        enfocado: usize,
        error: Option<String>,
        accion: AccionDialogo,
    },
    Selector {
        titulo: String,
        pistas: Vec<i64>,
        seleccion: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccionDialogo {
    CrearPlaylist {
        anadir: Vec<i64>,
    },
    RenombrarPlaylist(i64),
    EliminarPlaylist(i64),
    ExportarPlaylist(i64),
    ExportarFavoritas,
    ExportarFavoritasConfirmado {
        nombre_fichero: String,
    },
    ImportarPlaylist,
    AnadirAPlaylist {
        pistas: Vec<i64>,
        playlist_id: i64,
    },
    ExportarConfirmado {
        playlist_id: i64,
        nombre_fichero: String,
    },
    VaciarCola,
    NuevaEmisora,
    EditarEmisora {
        id: i64,
    },
    EliminarEmisora {
        id: i64,
    },
    ImportarRadio,
    ExportarRadioConfirmado,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Foco {
    Sidebar,
    Contenido,
    Cola,
}

impl Foco {
    pub fn siguiente(self) -> Self {
        match self {
            Foco::Sidebar => Foco::Contenido,
            Foco::Contenido => Foco::Cola,
            Foco::Cola => Foco::Sidebar,
        }
    }

    pub fn anterior(self) -> Self {
        match self {
            Foco::Sidebar => Foco::Cola,
            Foco::Contenido => Foco::Sidebar,
            Foco::Cola => Foco::Contenido,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Aviso {
    pub nivel: NivelAviso,
    pub texto: String,
    pub creado: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgresoEscaneo {
    pub procesadas: usize,
    pub total: usize,
}

impl ProgresoEscaneo {
    pub fn fraccion(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.procesadas as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }

    pub fn porcentaje(&self) -> u16 {
        (self.fraccion() * 100.0).round() as u16
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ZonasRaton {
    pub barra_progreso: Rect,
    pub lista: Option<(Rect, usize, usize)>,
    pub cola: Option<(Rect, usize)>,
}

pub struct ContextoApp<'a> {
    pub conn: &'a Connection,
    pub tx_app: &'a Sender<AppEvento>,
    pub ruta_bd: &'a Path,
    pub dir_caratulas: &'a Path,
    pub reproductor: &'a ManejoReproductor,
    pub scrobbling: &'a ManejoScrobbling,
}

enum Contexto {
    Ninguno,
    Pistas { ids: Vec<i64>, indice: usize },
    Album { album_id: i64 },
    Artista { artista_id: i64 },
    Emisora(EmisoraResumen),
    ResultadoRadio(EmisoraDirectorio),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Eje {
    /// En rejillas avanza una fila completa; en listas, `delta` posiciones.
    Vertical,
    /// Avanza `delta` posiciones: columna a columna en rejillas.
    Horizontal,
}

pub struct AppEstado {
    pub config: Config,
    pub paleta: Paleta,
    pub iconos: Iconos,
    pub vista: Vista,
    pub pantalla: Pantalla,
    pub foco: Foco,
    pub pila: Vec<(Vista, Pantalla)>,
    pub estado_reproductor: EstadoReproduccion,
    pub estado_scrobbling: EstadoScrobbling,
    pub favoritas: HashSet<i64>,
    pub total_pistas: i64,
    pub playlists: Vec<PlaylistResumen>,
    pub pistas: Vec<PistaListado>,
    pub pagina_pistas: usize,
    pub orden_pistas: OrdenPistas,
    pub orden_descendente: bool,
    pub artistas: Vec<ArtistaResumen>,
    pub detalle_artista: Option<DetalleArtista>,
    pub albumes: Vec<AlbumResumen>,
    pub orden_albumes: OrdenAlbumes,
    pub orden_albumes_descendente: bool,
    pub detalle_album: Option<DetalleAlbum>,
    pub detalle_playlist: Option<(PlaylistResumen, Vec<PistaListado>)>,
    pub dialogo: Option<Dialogo>,
    pub inicio: Inicio,
    pub busqueda: String,
    pub busqueda_enfocada: bool,
    pub resultados: ResultadosBusqueda,
    pub pendiente_busqueda: Option<Instant>,
    pub ultima_busqueda: String,
    pub seleccion: usize,
    pub seleccion_cola: usize,
    pub bloque_inicio: usize,
    pub columna_inicio: usize,
    pub columnas_rejilla: usize,
    pub escaneo_activo: Option<ProgresoEscaneo>,
    pub manejo_escaneo: Option<ManejoEscaneo>,
    pub avisos: Vec<Aviso>,
    pub ayuda_visible: bool,
    pub cola_visible: bool,
    pub debe_salir: bool,
    pub salida_inmediata: bool,
    pub ruta_tema: PathBuf,
    pub caratulas: CacheCaratulas,
    pub zonas: ZonasRaton,
    pub estado_captura: EstadoCaptura,
    pub anillo: Option<Arc<Anillo>>,
    rx_captura: Option<watch::Receiver<EstadoCaptura>>,
    pub modo_visual: Option<ModoVisual>,
    pub visuales: Vec<visuales::VisualCompartida>,
    pub indice_visual: usize,
    pub sensibilidad: f32,
    pub fuente_paleta: FuentePaleta,
    pub paleta_visual: PaletaVisual,
    paleta_visual_objetivo: PaletaVisual,
    transicion_paleta: Option<(Instant, PaletaVisual)>,
    pub colores_actuales: Option<String>,
    ultimo_album_colores: Option<i64>,
    pub cabecera_hasta: Option<Instant>,
    pub analisis: Analisis,
    analizador: Analizador,
    pub fps_degradado: bool,
    frames_lentos: u8,
    degradado_desde: Option<Instant>,
    aviso_degradado: bool,
    pub tamano_terminal: (u16, u16),
    pub reinicio_visual: bool,
    pub inactividad: Inactividad,
    arranque: Instant,
    ultimo_frame: Instant,
    pub dt_frame: f32,
    ultimo_clic: Option<(Instant, u16, u16)>,
    pendiente_g: bool,
    pub pestana_radio: PestanaRadio,
    pub seleccion_radio: [usize; 4],
    pub emisoras: Vec<EmisoraResumen>,
    pub orden_emisoras: OrdenEmisoras,
    pub orden_emisoras_descendente: bool,
    pub busqueda_radio: BusquedaRadio,
    pub radio_titulos: Vec<TituloEmisora>,
    pub radio_titulos_ok: HashSet<i64>,
    pub directorio: Option<ManejoDirectorio>,
}

impl AppEstado {
    pub fn nuevo(config: Config, paleta: Paleta, total_pistas: i64, ruta_tema: PathBuf) -> Self {
        let iconos = Iconos::desde(config.interfaz.iconos);
        let visuales = visuales::registro(config.interfaz.iconos == ModoIconos::Ascii);
        let paleta_visual = PaletaVisual::desde_tema(&paleta);
        let fuente_paleta = config.visuales.paleta;
        let estado_reproductor = EstadoReproduccion {
            volumen: config.reproductor.volumen_inicial,
            ..EstadoReproduccion::default()
        };
        Self {
            config,
            paleta,
            iconos,
            vista: Vista::Inicio,
            pantalla: Pantalla::Lista,
            foco: Foco::Contenido,
            pila: Vec::new(),
            estado_reproductor,
            estado_scrobbling: EstadoScrobbling::default(),
            favoritas: HashSet::new(),
            total_pistas,
            playlists: Vec::new(),
            pistas: Vec::new(),
            pagina_pistas: 0,
            orden_pistas: OrdenPistas::Titulo,
            orden_descendente: false,
            artistas: Vec::new(),
            detalle_artista: None,
            albumes: Vec::new(),
            orden_albumes: OrdenAlbumes::Titulo,
            orden_albumes_descendente: false,
            detalle_album: None,
            detalle_playlist: None,
            dialogo: None,
            inicio: Inicio::default(),
            busqueda: String::new(),
            busqueda_enfocada: false,
            resultados: ResultadosBusqueda::default(),
            pendiente_busqueda: None,
            ultima_busqueda: String::new(),
            seleccion: 0,
            seleccion_cola: 0,
            bloque_inicio: 0,
            columna_inicio: 0,
            columnas_rejilla: 4,
            escaneo_activo: None,
            manejo_escaneo: None,
            avisos: Vec::new(),
            ayuda_visible: false,
            cola_visible: false,
            debe_salir: false,
            salida_inmediata: false,
            ruta_tema,
            caratulas: CacheCaratulas::nuevo(),
            zonas: ZonasRaton::default(),
            estado_captura: EstadoCaptura::Desconectada,
            anillo: None,
            rx_captura: None,
            modo_visual: None,
            visuales,
            indice_visual: 0,
            sensibilidad: 1.0,
            fuente_paleta,
            paleta_visual,
            paleta_visual_objetivo: paleta_visual,
            transicion_paleta: None,
            colores_actuales: None,
            ultimo_album_colores: None,
            cabecera_hasta: None,
            analisis: Analisis::default(),
            analizador: Analizador::nuevo(),
            fps_degradado: false,
            frames_lentos: 0,
            degradado_desde: None,
            aviso_degradado: false,
            tamano_terminal: (0, 0),
            reinicio_visual: false,
            inactividad: Inactividad::nuevo(),
            arranque: Instant::now(),
            ultimo_frame: Instant::now(),
            dt_frame: 0.033,
            ultimo_clic: None,
            pendiente_g: false,
            pestana_radio: PestanaRadio::Favoritas,
            seleccion_radio: [0; 4],
            emisoras: Vec::new(),
            orden_emisoras: OrdenEmisoras::Nombre,
            orden_emisoras_descendente: false,
            busqueda_radio: BusquedaRadio::default(),
            radio_titulos: Vec::new(),
            radio_titulos_ok: HashSet::new(),
            directorio: None,
        }
    }

    pub fn conectar_captura(&mut self, rx: watch::Receiver<EstadoCaptura>, anillo: Arc<Anillo>) {
        self.rx_captura = Some(rx);
        self.anillo = Some(anillo);
    }

    pub fn actualizar_captura(&mut self) {
        let Some(rx) = self.rx_captura.as_mut() else {
            return;
        };
        if !rx.has_changed().unwrap_or(false) {
            return;
        }
        self.estado_captura = rx.borrow_and_update().clone();
    }

    pub fn aplicar_ajustes_visuales(&mut self, conn: &Connection) -> anyhow::Result<()> {
        let nombre = consultas::ajustes::leer(conn, "visual_actual")?
            .unwrap_or_else(|| self.config.visuales.predeterminada.clone());
        if let Some(indice) = visuales::indice_por_nombre(&nombre) {
            self.indice_visual = indice;
        }
        self.fuente_paleta = consultas::ajustes::leer(conn, "paleta_fuente")?
            .and_then(|valor| FuentePaleta::desde_str(&valor))
            .unwrap_or(self.config.visuales.paleta);
        self.sensibilidad = consultas::ajustes::leer(conn, "sensibilidad")?
            .and_then(|valor| valor.parse::<f32>().ok())
            .unwrap_or(1.0)
            .clamp(0.25, 4.0);
        self.actualizar_paleta_visual();
        Ok(())
    }

    pub fn aplicar_ajustes_radio(&mut self, conn: &Connection) -> anyhow::Result<()> {
        if let Some(pestana) = consultas::ajustes::leer(conn, "radio_pestana")?
            .and_then(|valor| PestanaRadio::desde_str(&valor))
        {
            self.pestana_radio = pestana;
        }
        Ok(())
    }

    pub fn entrar_visual(&mut self, protector: bool) {
        if self.modo_visual.is_some() || !self.config.visuales.activo {
            return;
        }
        self.modo_visual = Some(ModoVisual {
            protector,
            vista_previa: self.vista,
            pantalla_previa: self.pantalla,
            seleccion_previa: self.seleccion,
        });
        self.busqueda_enfocada = false;
        self.cabecera_hasta = Some(Instant::now() + DURACION_CABECERA);
        self.inactividad.registrar();
    }

    pub fn salir_visual(&mut self) {
        if let Some(modo) = self.modo_visual.take() {
            self.vista = modo.vista_previa;
            self.pantalla = modo.pantalla_previa;
            self.seleccion = modo.seleccion_previa;
        }
    }

    /// Cierra la ayuda si está visible. Devuelve `true` si la cerró.
    fn cerrar_ayuda(&mut self) -> bool {
        if self.ayuda_visible {
            self.ayuda_visible = false;
            true
        } else {
            false
        }
    }

    pub fn cabecera_visible(&self) -> bool {
        self.modo_visual.is_some()
            && self
                .cabecera_hasta
                .is_some_and(|hasta| Instant::now() < hasta)
    }

    fn refrescar_cabecera(&mut self) {
        if self.modo_visual.is_some() {
            self.cabecera_hasta = Some(Instant::now() + DURACION_CABECERA);
        }
    }

    pub fn visual_actual_nombre(&self) -> &'static str {
        self.visuales
            .get(self.indice_visual)
            .map(|visual| visual.borrow().nombre())
            .unwrap_or("Espectro")
    }

    fn tecla_en_visual(&mut self, tecla: &KeyEvent, ctx: &ContextoApp<'_>) -> bool {
        let Some(modo) = self.modo_visual else {
            return false;
        };
        if modo.protector {
            self.salir_visual();
            return true;
        }
        if tecla.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        match tecla.code {
            KeyCode::Esc | KeyCode::Char('7') => {
                self.salir_visual();
                true
            }
            KeyCode::Char('v') => {
                self.ciclar_visual(1, ctx);
                true
            }
            KeyCode::Char('V') => {
                self.ciclar_visual(-1, ctx);
                true
            }
            KeyCode::Char(caracter @ '1'..='6') => {
                self.saltar_visual(caracter as usize - '1' as usize, ctx);
                true
            }
            KeyCode::Char('[') => {
                self.ajustar_sensibilidad(0.8, ctx);
                true
            }
            KeyCode::Char(']') => {
                self.ajustar_sensibilidad(1.25, ctx);
                true
            }
            KeyCode::Char('b') => {
                self.alternar_paleta(ctx);
                true
            }
            _ => false,
        }
    }

    fn ciclar_visual(&mut self, delta: i64, ctx: &ContextoApp<'_>) {
        let total = self.visuales.len().max(1) as i64;
        self.indice_visual = ((self.indice_visual as i64 + delta).rem_euclid(total)) as usize;
        self.reinicio_visual = true;
        self.guardar_ajuste(ctx, "visual_actual", self.visual_actual_nombre());
        self.refrescar_cabecera();
    }

    fn saltar_visual(&mut self, indice: usize, ctx: &ContextoApp<'_>) {
        if indice < self.visuales.len() {
            self.indice_visual = indice;
            self.reinicio_visual = true;
            self.guardar_ajuste(ctx, "visual_actual", self.visual_actual_nombre());
            self.refrescar_cabecera();
        }
    }

    fn ajustar_sensibilidad(&mut self, factor: f32, ctx: &ContextoApp<'_>) {
        self.sensibilidad = (self.sensibilidad * factor).clamp(0.25, 4.0);
        self.guardar_ajuste(ctx, "sensibilidad", &format!("{:.3}", self.sensibilidad));
        self.refrescar_cabecera();
    }

    fn alternar_paleta(&mut self, ctx: &ContextoApp<'_>) {
        self.fuente_paleta = match self.fuente_paleta {
            FuentePaleta::Tema => FuentePaleta::Caratula,
            FuentePaleta::Caratula => FuentePaleta::Tema,
        };
        self.guardar_ajuste(ctx, "paleta_fuente", self.fuente_paleta.como_str());
        self.refrescar_cabecera();
    }

    fn guardar_ajuste(&mut self, ctx: &ContextoApp<'_>, clave: &str, valor: &str) {
        if let Err(error) = consultas::ajustes::escribir(ctx.conn, clave, valor) {
            self.notificar(
                NivelAviso::Error,
                format!("No se pudo guardar el ajuste {clave}: {error:#}"),
            );
        }
    }

    fn paleta_objetivo(&self) -> PaletaVisual {
        if self.fuente_paleta == FuentePaleta::Caratula
            && let Some(texto) = self.colores_actuales.as_deref()
            && let Some(paleta) = PaletaVisual::desde_texto(texto, self.paleta.fondo)
        {
            return paleta;
        }
        PaletaVisual::desde_tema(&self.paleta)
    }

    pub fn actualizar_paleta_visual(&mut self) {
        let objetivo = self.paleta_objetivo();
        if objetivo != self.paleta_visual_objetivo {
            self.transicion_paleta = Some((Instant::now(), self.paleta_visual));
            self.paleta_visual_objetivo = objetivo;
        }
        if let Some((inicio, desde)) = self.transicion_paleta {
            let t = (inicio.elapsed().as_secs_f32() / DURACION_TRANSICION_PALETA).clamp(0.0, 1.0);
            self.paleta_visual = PaletaVisual::interpolar(&desde, &self.paleta_visual_objetivo, t);
            if t >= 1.0 {
                self.transicion_paleta = None;
            }
        }
    }

    pub fn necesita_analisis(&self) -> bool {
        if !self.config.visuales.activo {
            return false;
        }
        self.modo_visual.is_some() || self.mini_espectro_visible()
    }

    pub fn mini_espectro_visible(&self) -> bool {
        self.config.visuales.activo
            && self.config.visuales.mini_espectro
            && self.tamano_terminal.0 >= 70
            && self.tamano_terminal.1 >= 20
    }

    fn tick_activo(&self) -> bool {
        self.modo_visual.is_some() || self.mini_espectro_visible()
    }

    pub fn intervalo_tick(&self) -> Duration {
        if !self.tick_activo() {
            return TICK_BASE;
        }
        if self.fps_degradado {
            return TICK_DEGRADADO;
        }
        let fps = u64::from(self.config.visuales.fps.clamp(15, 60));
        Duration::from_millis((1000 / fps).max(1))
    }

    /// Analiza el anillo y prepara la paleta antes de dibujar el frame.
    pub fn preparar_frame(&mut self) {
        let ahora = Instant::now();
        self.dt_frame = ahora
            .duration_since(self.ultimo_frame)
            .as_secs_f32()
            .clamp(0.001, 0.5);
        self.ultimo_frame = ahora;
        if self.reinicio_visual {
            if let Some(visual) = self.visuales.get(self.indice_visual) {
                visual.borrow_mut().reiniciar();
            }
            self.reinicio_visual = false;
        }
        self.actualizar_paleta_visual();
        if !self.necesita_analisis() {
            return;
        }
        let ancho = self.tamano_terminal.0;
        let (tasa_hz, sin_audio) = match &self.estado_captura {
            EstadoCaptura::Capturando { tasa_hz, .. } => (*tasa_hz, false),
            _ => (48_000, true),
        };
        let t_ms = self.arranque.elapsed().as_millis() as u64;
        let en_pausa = self.estado_reproductor.pausado();
        if let Some(anillo) = self.anillo.clone() {
            self.analisis = self.analizador.procesar(
                &anillo,
                ancho,
                self.sensibilidad,
                tasa_hz,
                t_ms,
                sin_audio,
                en_pausa,
            );
        } else {
            let n_bandas = (ancho as usize / 2).clamp(16, 128);
            let n_onda = (ancho as usize * 2).max(2);
            self.analisis = Analisis::ambiental(t_ms, n_bandas, n_onda);
            self.analisis.tasa_hz = tasa_hz;
        }
    }

    pub fn registrar_frame(&mut self, duracion: Duration) {
        if !self.tick_activo() {
            return;
        }
        if duracion > UMBRAL_FRAME_LENTO {
            self.frames_lentos = self.frames_lentos.saturating_add(1);
        } else {
            self.frames_lentos = 0;
            if self.fps_degradado
                && self
                    .degradado_desde
                    .is_some_and(|desde| desde.elapsed() >= MARGEN_RECUPERACION)
            {
                self.fps_degradado = false;
                self.degradado_desde = None;
                self.notificar(NivelAviso::Info, "Visuales de nuevo a 30 fps");
            }
        }
        if self.frames_lentos >= 3 && !self.fps_degradado {
            self.fps_degradado = true;
            self.degradado_desde = Some(Instant::now());
            if !self.aviso_degradado {
                self.aviso_degradado = true;
                self.notificar(NivelAviso::Aviso, "Visuales a 15 fps por rendimiento");
            }
        }
    }

    fn actualizar_protector(&mut self) {
        if self.modo_visual.is_none() {
            let autoinicio = self.config.visuales.autoinicio_min;
            if self.config.visuales.activo
                && autoinicio > 0
                && self.dialogo.is_none()
                && !self.esta_detenido()
                && self.inactividad.segundos() >= autoinicio * 60
            {
                self.entrar_visual(true);
            }
            return;
        }
        if self.modo_visual.is_some_and(|modo| modo.protector) && self.esta_detenido() {
            self.salir_visual();
        }
    }

    pub fn activar_caratulas(
        &mut self,
        picker: ratatui_image::picker::Picker,
        tx: std::sync::mpsc::Sender<crate::ui::componentes::imagen::PeticionCaratula>,
    ) {
        self.caratulas.activar(picker, tx);
    }

    pub fn recibir_caratula(&mut self, album_id: i64, imagen: image::DynamicImage) {
        self.caratulas.insertar(album_id, imagen);
    }

    pub fn recibir_colores(&mut self, album_id: i64, colores: String) {
        if self
            .estado_reproductor
            .pista_actual()
            .is_some_and(|pista| pista.album_id == album_id)
        {
            self.colores_actuales = Some(colores);
        }
    }

    fn actualizar_colores_album(&mut self, ctx: &ContextoApp<'_>) {
        let album_id = self
            .estado_reproductor
            .pista_actual()
            .map(|pista| pista.album_id);
        if album_id == self.ultimo_album_colores {
            return;
        }
        self.ultimo_album_colores = album_id;
        let Some(album_id) = album_id else {
            self.colores_actuales = None;
            return;
        };
        match consultas::albumes::colores(ctx.conn, album_id) {
            Ok(Some(colores)) => self.colores_actuales = Some(colores),
            Ok(None) => {
                self.colores_actuales = None;
                if let Some(ruta) = self
                    .estado_reproductor
                    .pista_actual()
                    .and_then(|pista| pista.caratula_ruta.clone())
                {
                    self.caratulas.solicitar_colores(album_id, &ruta);
                }
            }
            Err(error) => {
                tracing::warn!("no se pudieron leer los colores del álbum: {error:#}");
                self.colores_actuales = None;
            }
        }
    }

    pub fn notificar(&mut self, nivel: NivelAviso, texto: impl Into<String>) {
        self.avisos.push(Aviso {
            nivel,
            texto: texto.into(),
            creado: Instant::now(),
        });
        if self.avisos.len() > 6 {
            self.avisos.remove(0);
        }
    }

    pub fn manejar(&mut self, evento: AppEvento, ctx: &ContextoApp<'_>) {
        match evento {
            AppEvento::Tecla(tecla) => {
                self.inactividad.registrar();
                if self.dialogo.is_some() && self.tecla_en_dialogo(&tecla, ctx) {
                    return;
                }
                if tecla.code == KeyCode::Esc && self.cerrar_ayuda() {
                    return;
                }
                if self.tecla_en_visual(&tecla, ctx) {
                    return;
                }
                if self.tecla_en_radio(&tecla, ctx) {
                    return;
                }
                if self.busqueda_enfocada && self.tecla_en_busqueda(&tecla) {
                    return;
                }
                if let Some(accion) = traducir(&tecla) {
                    let es_g = matches!(accion, Accion::TeclaG);
                    if !es_g {
                        self.pendiente_g = false;
                    }
                    self.ejecutar(accion, ctx);
                }
            }
            AppEvento::Tick => {
                self.avisos
                    .retain(|aviso| aviso.creado.elapsed() < DURACION_TOAST);
                self.actualizar_busqueda(ctx);
                self.actualizar_paleta_visual();
                self.actualizar_protector();
                if self
                    .cabecera_hasta
                    .is_some_and(|hasta| Instant::now() >= hasta)
                {
                    self.cabecera_hasta = None;
                }
            }
            AppEvento::Reproductor(estado) => {
                let estado = *estado;
                let titulo_anterior = self.estado_reproductor.titulo_icy.clone();
                let emisora_anterior = self
                    .estado_reproductor
                    .emisora_actual()
                    .map(|emisora| emisora.id);
                self.estado_reproductor = estado;
                if self.seleccion_cola >= self.estado_reproductor.cola.len() {
                    self.seleccion_cola = self.estado_reproductor.cola.len().saturating_sub(1);
                }
                self.actualizar_colores_album(ctx);
                if self.vista == Vista::Radio
                    && self.pestana_radio == PestanaRadio::Sonando
                    && (self.estado_reproductor.titulo_icy != titulo_anterior
                        || self
                            .estado_reproductor
                            .emisora_actual()
                            .map(|emisora| emisora.id)
                            != emisora_anterior)
                {
                    self.refrescar_titulos_radio(ctx);
                }
            }
            AppEvento::Escaneo(evento) => self.manejar_escaneo(evento, ctx),
            AppEvento::TemaActualizado(paleta) => {
                self.paleta = paleta;
            }
            AppEvento::Notificacion(nivel, texto) => self.notificar(nivel, texto),
            AppEvento::CaratulaLista(_album_id) => {}
            AppEvento::Scrobbling(estado) => self.estado_scrobbling = estado,
            AppEvento::ResultadosRadio(clave) => self.manejar_resultados_radio(&clave, ctx),
            AppEvento::LogoListo(emisora_id) => self.manejar_logo_listo(emisora_id, ctx),
            AppEvento::Salir => {
                self.debe_salir = true;
            }
            AppEvento::Raton(evento) => {
                self.inactividad.registrar();
                if self.modo_visual.is_some_and(|modo| modo.protector) {
                    self.salir_visual();
                    return;
                }
                self.manejar_raton(evento, ctx);
            }
            AppEvento::Redimension(ancho, alto) => {
                self.tamano_terminal = (ancho, alto);
                self.reinicio_visual = true;
            }
        }
    }

    fn ejecutar(&mut self, accion: Accion, ctx: &ContextoApp<'_>) {
        match accion {
            Accion::Salir => {
                self.debe_salir = true;
            }
            Accion::SalirInmediato => {
                self.salida_inmediata = true;
                self.debe_salir = true;
            }
            Accion::Cerrar => {
                if !self.cerrar_ayuda() {
                    self.volver();
                }
            }
            Accion::Ayuda => {
                self.ayuda_visible = !self.ayuda_visible;
            }
            Accion::IrA(vista) => {
                if vista == Vista::Visual {
                    if self.modo_visual.is_some() {
                        self.salir_visual();
                    } else {
                        self.entrar_visual(false);
                    }
                    return;
                }
                self.pila.clear();
                self.pantalla = Pantalla::Lista;
                self.seleccion = 0;
                if vista == Vista::Inicio {
                    self.bloque_inicio = 0;
                    self.columna_inicio = 0;
                }
                self.vista = vista;
                self.refrescar_vista(vista, ctx);
            }
            Accion::AlternarCola => {
                self.cola_visible = !self.cola_visible;
                if !self.cola_visible && self.foco == Foco::Cola {
                    self.foco = Foco::Contenido;
                }
            }
            Accion::FocoSiguiente => {
                self.foco = self.foco.siguiente();
                if self.foco == Foco::Cola && !self.cola_visible {
                    self.foco = Foco::Sidebar;
                }
            }
            Accion::FocoAnterior => {
                self.foco = self.foco.anterior();
                if self.foco == Foco::Cola && !self.cola_visible {
                    self.foco = Foco::Contenido;
                }
            }
            Accion::Reescanear => self.iniciar_escaneo(ctx),
            Accion::EnviarScrobbles => {
                ctx.scrobbling.enviar(ComandoScrobbling::EnviarAhora);
                if self.estado_scrobbling.pendientes() > 0 {
                    self.notificar(NivelAviso::Info, "Enviando pendientes de scrobbling…");
                }
            }
            Accion::RecargarTema => self.recargar_tema(),
            Accion::TeclaG => {
                if self.pendiente_g {
                    self.pendiente_g = false;
                    self.mover_seleccion(ctx, Eje::Vertical, isize::MIN / 2);
                } else {
                    self.pendiente_g = true;
                }
            }
            Accion::EnfocarBuscar => {
                if self.vista == Vista::Radio {
                    self.foco = Foco::Contenido;
                    self.enfocar_busqueda_radio(ctx);
                } else {
                    self.vista = Vista::Buscar;
                    self.pantalla = Pantalla::Lista;
                    self.busqueda_enfocada = true;
                    self.foco = Foco::Contenido;
                }
            }
            Accion::PestanaAnterior => {
                if self.vista == Vista::Radio {
                    self.cambiar_pestana_radio(ctx, self.pestana_radio.anterior());
                }
            }
            Accion::PestanaSiguiente => {
                if self.vista == Vista::Radio {
                    self.cambiar_pestana_radio(ctx, self.pestana_radio.siguiente());
                }
            }
            Accion::BuscarTituloIcy => self.buscar_titulo_icy(ctx),
            Accion::Abajo if self.vista == Vista::Inicio => self.mover_inicio(ctx, 1, 0),
            Accion::Abajo => self.mover_seleccion(ctx, Eje::Vertical, 1),
            Accion::Arriba if self.vista == Vista::Inicio => self.mover_inicio(ctx, -1, 0),
            Accion::Arriba => self.mover_seleccion(ctx, Eje::Vertical, -1),
            Accion::Derecha if self.vista == Vista::Inicio => self.mover_inicio(ctx, 0, 1),
            Accion::Derecha => match self.vista_pantalla_rejilla() {
                true => self.mover_seleccion(ctx, Eje::Horizontal, 1),
                false => self.abrir_seleccion(ctx),
            },
            Accion::Izquierda if self.vista == Vista::Inicio => self.mover_inicio(ctx, 0, -1),
            Accion::Izquierda => match self.vista_pantalla_rejilla() {
                true => self.mover_seleccion(ctx, Eje::Horizontal, -1),
                false => self.volver(),
            },
            Accion::MediaAbajo => self.mover_seleccion(ctx, Eje::Vertical, PAGINA_SALTOS as isize),
            Accion::MediaArriba => {
                self.mover_seleccion(ctx, Eje::Vertical, -(PAGINA_SALTOS as isize))
            }
            Accion::Primero => self.mover_seleccion(ctx, Eje::Vertical, isize::MIN / 2),
            Accion::Ultimo => self.mover_seleccion(ctx, Eje::Vertical, isize::MAX / 2),
            Accion::CiclarOrden => match self.vista {
                Vista::Pistas => {
                    let posicion = OrdenPistas::TODAS
                        .iter()
                        .position(|orden| *orden == self.orden_pistas)
                        .unwrap_or(0);
                    self.orden_pistas =
                        OrdenPistas::TODAS[(posicion + 1) % OrdenPistas::TODAS.len()];
                    self.pagina_nueva(ctx);
                }
                Vista::Albumes if self.pantalla == Pantalla::Lista => {
                    let posicion = OrdenAlbumes::TODAS
                        .iter()
                        .position(|orden| *orden == self.orden_albumes)
                        .unwrap_or(0);
                    self.orden_albumes =
                        OrdenAlbumes::TODAS[(posicion + 1) % OrdenAlbumes::TODAS.len()];
                    self.refrescar_albumes(ctx);
                }
                Vista::Radio => {
                    let posicion = OrdenEmisoras::TODAS
                        .iter()
                        .position(|orden| *orden == self.orden_emisoras)
                        .unwrap_or(0);
                    self.orden_emisoras =
                        OrdenEmisoras::TODAS[(posicion + 1) % OrdenEmisoras::TODAS.len()];
                    self.refrescar_radio(ctx);
                }
                _ => {}
            },
            Accion::InvertirOrden => match self.vista {
                Vista::Pistas => {
                    self.orden_descendente = !self.orden_descendente;
                    self.pagina_nueva(ctx);
                }
                Vista::Albumes if self.pantalla == Pantalla::Lista => {
                    self.orden_albumes_descendente = !self.orden_albumes_descendente;
                    self.refrescar_albumes(ctx);
                }
                Vista::Radio => {
                    self.orden_emisoras_descendente = !self.orden_emisoras_descendente;
                    self.refrescar_radio(ctx);
                }
                _ => {}
            },
            Accion::Abrir => {
                if self.vista == Vista::Radio
                    && self.pestana_radio == PestanaRadio::Buscar
                    && self.busqueda_radio.campo.is_some()
                {
                    self.lanzar_busqueda_radio(ctx);
                } else if self.foco == Foco::Cola {
                    if self.seleccion_cola < self.estado_reproductor.cola.len() {
                        ctx.reproductor.enviar(ComandoReproductor::SaltarA {
                            posicion: self.seleccion_cola,
                        });
                    }
                } else {
                    self.abrir_seleccion(ctx);
                }
            }
            Accion::AnadirCola => self.anadir_seleccion(ctx, false),
            Accion::ReproducirSiguiente => self.anadir_seleccion(ctx, true),
            Accion::Quitar => {
                if self.foco == Foco::Cola
                    && self.seleccion_cola < self.estado_reproductor.cola.len()
                {
                    ctx.reproductor.enviar(ComandoReproductor::EliminarDeCola {
                        posicion: self.seleccion_cola,
                    });
                } else if self.vista == Vista::Playlists
                    && self.pantalla == Pantalla::DetallePlaylist
                    && self.foco == Foco::Contenido
                    && !self.es_playlist_favoritas()
                {
                    self.quitar_pista_playlist(ctx);
                }
            }
            Accion::MoverAbajo => self.mover(ctx, 1),
            Accion::MoverArriba => self.mover(ctx, -1),
            Accion::VaciarCola => {
                self.dialogo = Some(Dialogo::Confirmacion {
                    titulo: "Vaciar la cola".to_string(),
                    mensaje: "¿Seguro que quieres vaciar la cola? La reproducción se detendrá."
                        .to_string(),
                    accion: AccionDialogo::VaciarCola,
                });
            }
            Accion::NuevaPlaylist => {
                if self.vista == Vista::Radio {
                    self.nueva_emisora_dialogo();
                } else {
                    self.dialogo = Some(Dialogo::Texto {
                        titulo: "Nueva playlist".to_string(),
                        valor: String::new(),
                        accion: AccionDialogo::CrearPlaylist { anadir: Vec::new() },
                    });
                }
            }
            Accion::RenombrarPlaylist => {
                if self.vista == Vista::Radio {
                    if let Some(emisora) = self
                        .emisora_bajo_cursor()
                        .or_else(|| self.estado_reproductor.emisora_actual().cloned())
                    {
                        self.editar_emisora_dialogo(ctx, emisora.id);
                    }
                } else if let Some(playlist) = self.playlist_seleccionada() {
                    self.dialogo = Some(Dialogo::Texto {
                        titulo: "Renombrar playlist".to_string(),
                        valor: playlist.nombre.clone(),
                        accion: AccionDialogo::RenombrarPlaylist(playlist.id),
                    });
                }
            }
            Accion::EliminarPlaylist => {
                if self.vista == Vista::Radio {
                    if let Some(emisora) = self.emisora_bajo_cursor() {
                        self.eliminar_emisora_dialogo(ctx, emisora.id);
                    }
                } else if let Some(playlist) = self.playlist_seleccionada() {
                    self.dialogo = Some(Dialogo::Confirmacion {
                        titulo: "Eliminar playlist".to_string(),
                        mensaje: format!(
                            "¿Eliminar «{}» y sus {} pistas? La biblioteca no se modifica.",
                            playlist.nombre, playlist.num_pistas
                        ),
                        accion: AccionDialogo::EliminarPlaylist(playlist.id),
                    });
                }
            }
            Accion::ExportarPlaylist => {
                if self.vista == Vista::Radio {
                    self.exportar_radio(ctx, false);
                } else if let Some(playlist) = self.playlist_seleccionada() {
                    self.dialogo = Some(Dialogo::Texto {
                        titulo: "Exportar a M3U8 (nombre de fichero)".to_string(),
                        valor: format!("{}.m3u8", playlist.nombre),
                        accion: AccionDialogo::ExportarPlaylist(playlist.id),
                    });
                } else if self.es_playlist_favoritas() {
                    self.dialogo = Some(Dialogo::Texto {
                        titulo: "Exportar a M3U8 (nombre de fichero)".to_string(),
                        valor: "Favoritas.m3u8".to_string(),
                        accion: AccionDialogo::ExportarFavoritas,
                    });
                }
            }
            Accion::ImportarPlaylist => {
                if self.vista == Vista::Radio {
                    self.importar_radio_dialogo();
                } else {
                    self.dialogo = Some(Dialogo::Texto {
                        titulo: "Importar M3U/M3U8 (ruta del fichero)".to_string(),
                        valor: String::new(),
                        accion: AccionDialogo::ImportarPlaylist,
                    });
                }
            }
            Accion::AnadirPlaylist => self.abrir_selector_playlist(ctx),
            Accion::AlternarPausa => {
                ctx.reproductor.enviar(ComandoReproductor::AlternarPausa);
            }
            Accion::AlternarFavorita => self.alternar_favorita(ctx),
            Accion::Siguiente => {
                ctx.reproductor.enviar(ComandoReproductor::Siguiente);
            }
            Accion::Anterior => {
                ctx.reproductor.enviar(ComandoReproductor::Anterior);
            }
            Accion::Detener => {
                ctx.reproductor.enviar(ComandoReproductor::Detener);
            }
            Accion::RetrocederCorto => self.buscar(ctx, -self.config.reproductor.salto_corto_s),
            Accion::AvanzarCorto => self.buscar(ctx, self.config.reproductor.salto_corto_s),
            Accion::RetrocederLargo => self.buscar(ctx, -self.config.reproductor.salto_largo_s),
            Accion::AvanzarLargo => self.buscar(ctx, self.config.reproductor.salto_largo_s),
            Accion::SubirVolumen => self.cambiar_volumen(ctx, 5),
            Accion::BajarVolumen => self.cambiar_volumen(ctx, -5),
            Accion::Silencio => {
                ctx.reproductor.enviar(ComandoReproductor::AlternarSilencio);
            }
            Accion::AlternarAleatorio => {
                ctx.reproductor
                    .enviar(ComandoReproductor::AlternarAleatorio);
            }
            Accion::CiclarRepeticion => {
                ctx.reproductor.enviar(ComandoReproductor::CiclarRepeticion);
            }
        }
    }

    fn manejar_raton(&mut self, evento: MouseEvent, ctx: &ContextoApp<'_>) {
        match evento.kind {
            MouseEventKind::ScrollDown => self.mover_seleccion(ctx, Eje::Vertical, 3),
            MouseEventKind::ScrollUp => self.mover_seleccion(ctx, Eje::Vertical, -3),
            MouseEventKind::Down(MouseButton::Left) => {
                let (x, y) = (evento.column, evento.row);
                let doble = self.ultimo_clic.is_some_and(|(cuando, cx, cy)| {
                    cuando.elapsed() < Duration::from_millis(400) && cx == x && cy == y
                });
                self.ultimo_clic = Some((Instant::now(), x, y));

                if en_rect(self.zonas.barra_progreso, x, y)
                    && self.estado_reproductor.pista_cargada()
                    && self.estado_reproductor.duracion_ms > 0
                {
                    let ancho = f64::from(self.zonas.barra_progreso.width.max(1));
                    let relativo =
                        (f64::from(x - self.zonas.barra_progreso.x) / ancho).clamp(0.0, 1.0);
                    let ms = (relativo * self.estado_reproductor.duracion_ms as f64) as i64;
                    ctx.reproductor.enviar(ComandoReproductor::Buscar {
                        ms,
                        relativo: false,
                    });
                    return;
                }

                if let Some((rect, inicio)) = self.zonas.cola
                    && en_rect(rect, x, y)
                {
                    let fila = (y - rect.y) as usize + inicio;
                    if fila < self.estado_reproductor.cola.len() {
                        self.foco = Foco::Cola;
                        self.seleccion_cola = fila;
                        if doble {
                            ctx.reproductor
                                .enviar(ComandoReproductor::SaltarA { posicion: fila });
                        }
                    }
                    return;
                }

                if let Some((rect, inicio, total)) = self.zonas.lista
                    && en_rect(rect, x, y)
                {
                    let fila = (y - rect.y) as usize + inicio;
                    if fila < total {
                        self.seleccion = fila;
                        if doble {
                            self.abrir_seleccion(ctx);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn tecla_en_busqueda(&mut self, tecla: &KeyEvent) -> bool {
        if tecla.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        match tecla.code {
            KeyCode::Esc => {
                self.busqueda_enfocada = false;
                true
            }
            KeyCode::Enter => {
                self.busqueda_enfocada = false;
                self.foco = Foco::Contenido;
                true
            }
            KeyCode::Backspace => {
                self.busqueda.pop();
                self.pendiente_busqueda = Some(Instant::now());
                true
            }
            KeyCode::Char(caracter) => {
                if self.busqueda.chars().count() < 120 {
                    self.busqueda.push(caracter);
                    self.pendiente_busqueda = Some(Instant::now());
                }
                true
            }
            _ => true,
        }
    }

    fn actualizar_busqueda(&mut self, ctx: &ContextoApp<'_>) {
        let Some(inicio) = self.pendiente_busqueda else {
            return;
        };
        if inicio.elapsed() < Duration::from_millis(150) {
            return;
        }
        self.pendiente_busqueda = None;
        self.ejecutar_busqueda(ctx);
    }

    pub fn ejecutar_busqueda(&mut self, ctx: &ContextoApp<'_>) {
        if self.busqueda == self.ultima_busqueda {
            return;
        }
        match consultas::buscar(ctx.conn, &self.busqueda, LIMITE_BUSQUEDA) {
            Ok(resultados) => {
                self.resultados = resultados;
                self.seleccion = 0;
                self.ultima_busqueda = self.busqueda.clone();
            }
            Err(error) => {
                self.notificar(NivelAviso::Error, format!("No se pudo buscar: {error:#}"))
            }
        }
    }

    fn vista_pantalla_rejilla(&self) -> bool {
        self.vista == Vista::Albumes && self.pantalla == Pantalla::Lista
    }

    fn total_seleccionable(&self) -> usize {
        match (self.vista, self.pantalla) {
            (Vista::Inicio, _) => {
                self.inicio.recientes.len()
                    + self.inicio.anadidos.len()
                    + self.inicio.redescubre.len()
            }
            (Vista::Artistas, Pantalla::Lista) => self.artistas.len(),
            (Vista::Artistas, Pantalla::DetalleArtista) => self
                .detalle_artista
                .as_ref()
                .map(|detalle| detalle.albumes.len() + detalle.pistas_sueltas.len())
                .unwrap_or(0),
            (Vista::Albumes, Pantalla::Lista) => self.albumes.len(),
            (Vista::Albumes, Pantalla::DetalleAlbum) => self
                .detalle_album
                .as_ref()
                .map(|detalle| detalle.pistas.len())
                .unwrap_or(0),
            (Vista::Pistas, _) => self.pistas.len(),
            (Vista::Playlists, Pantalla::DetallePlaylist) => self
                .detalle_playlist
                .as_ref()
                .map(|(_, pistas)| pistas.len())
                .unwrap_or(0),
            (Vista::Playlists, _) => self.playlists.len() + 1,
            (Vista::Buscar, _) => {
                self.resultados.artistas.len()
                    + self.resultados.albumes.len()
                    + self.resultados.pistas.len()
            }
            (Vista::Radio, _) => match self.pestana_radio {
                PestanaRadio::Favoritas | PestanaRadio::Todas => self.emisoras.len(),
                PestanaRadio::Buscar => {
                    if self.busqueda_radio.campo.is_some() {
                        0
                    } else if self.config.radio.directorio {
                        self.busqueda_radio.resultados.len()
                    } else {
                        self.emisoras.len()
                    }
                }
                PestanaRadio::Sonando => self.radio_titulos.len(),
            },
            _ => 0,
        }
    }

    fn mover_seleccion(&mut self, ctx: &ContextoApp<'_>, eje: Eje, delta: isize) {
        self.desplazar_seleccion(eje, delta);
        if self.vista == Vista::Pistas {
            self.asegurar_pagina(ctx);
        }
    }

    fn desplazar_seleccion(&mut self, eje: Eje, delta: isize) {
        if self.foco == Foco::Cola {
            let total = self.estado_reproductor.cola.len();
            if total == 0 {
                return;
            }
            let ultimo = total - 1;
            self.seleccion_cola = mover_indice(self.seleccion_cola, ultimo, delta);
            return;
        }
        if self.foco != Foco::Contenido {
            return;
        }
        let total = self.total_seleccionable();
        if total == 0 {
            return;
        }
        self.seleccion = if delta.abs() == 1 && self.vista_pantalla_rejilla() {
            let columnas = self.columnas_rejilla.max(1);
            match eje {
                Eje::Vertical => mover_en_rejilla(self.seleccion, total - 1, columnas, delta),
                Eje::Horizontal => mover_indice(self.seleccion, total - 1, delta),
            }
        } else {
            mover_indice(self.seleccion, total - 1, delta)
        };
        if self.vista == Vista::Inicio {
            self.sincronizar_inicio_desde_seleccion();
        }
    }

    fn inicio_bloque_len(&self, bloque: usize) -> usize {
        match bloque {
            0 => self.inicio.recientes.len(),
            1 => self.inicio.anadidos.len(),
            _ => self.inicio.redescubre.len(),
        }
    }

    fn mover_inicio(&mut self, _ctx: &ContextoApp<'_>, delta_bloque: isize, delta_columna: isize) {
        if self.foco != Foco::Contenido {
            return;
        }
        if delta_bloque != 0 {
            self.bloque_inicio = (self.bloque_inicio as isize + delta_bloque).clamp(0, 2) as usize;
            let len = self.inicio_bloque_len(self.bloque_inicio);
            self.columna_inicio = self.columna_inicio.min(len.saturating_sub(1));
            if len == 0 {
                self.columna_inicio = 0;
            }
        }
        if delta_columna != 0 {
            let len = self.inicio_bloque_len(self.bloque_inicio);
            if len > 0 {
                self.columna_inicio = (self.columna_inicio as isize + delta_columna)
                    .clamp(0, len as isize - 1) as usize;
            }
        }
        self.actualizar_seleccion_inicio();
    }

    fn actualizar_seleccion_inicio(&mut self) {
        self.seleccion = match self.bloque_inicio {
            0 => self.columna_inicio,
            1 => self.inicio.recientes.len() + self.columna_inicio,
            _ => self.inicio.recientes.len() + self.inicio.anadidos.len() + self.columna_inicio,
        };
    }

    fn sincronizar_inicio_desde_seleccion(&mut self) {
        let n_recientes = self.inicio.recientes.len();
        let n_anadidos = self.inicio.anadidos.len();
        if self.seleccion < n_recientes {
            self.bloque_inicio = 0;
            self.columna_inicio = self.seleccion;
        } else if self.seleccion < n_recientes + n_anadidos {
            self.bloque_inicio = 1;
            self.columna_inicio = self.seleccion - n_recientes;
        } else {
            self.bloque_inicio = 2;
            self.columna_inicio = self.seleccion - n_recientes - n_anadidos;
        }
        let len = self.inicio_bloque_len(self.bloque_inicio);
        if len == 0 {
            self.columna_inicio = 0;
        } else {
            self.columna_inicio = self.columna_inicio.min(len - 1);
        }
    }

    fn abrir_seleccion(&mut self, ctx: &ContextoApp<'_>) {
        if self.vista == Vista::Playlists && self.pantalla == Pantalla::Lista {
            if self.seleccion == 0 {
                self.abrir_favoritas(ctx);
                return;
            }
            let Some(playlist) = self.playlists.get(self.seleccion - 1) else {
                return;
            };
            let playlist = playlist.clone();
            match consultas::playlist::pistas(ctx.conn, playlist.id) {
                Ok(pistas) => {
                    self.pila.push((self.vista, self.pantalla));
                    self.detalle_playlist = Some((playlist, pistas));
                    self.pantalla = Pantalla::DetallePlaylist;
                    self.seleccion = 0;
                }
                Err(error) => self.notificar(
                    NivelAviso::Error,
                    format!("No se pudo abrir la playlist: {error:#}"),
                ),
            }
            return;
        }
        match self.contexto() {
            Contexto::Pistas { ids, indice } => {
                let elementos = self.elementos_pistas(ctx, &ids);
                if elementos.is_empty() {
                    return;
                }
                let indice = indice.min(elementos.len() - 1);
                ctx.reproductor
                    .enviar(ComandoReproductor::ReemplazarCola { elementos, indice });
            }
            Contexto::Album { album_id } => self.abrir_album(ctx, album_id),
            Contexto::Artista { artista_id } => self.abrir_artista(ctx, artista_id),
            Contexto::Emisora(emisora) => {
                let id = emisora.id;
                ctx.reproductor.enviar(ComandoReproductor::ReemplazarCola {
                    elementos: vec![ElementoCola::Emisora(emisora)],
                    indice: 0,
                });
                self.enviar_click_emisora(ctx, id);
            }
            Contexto::ResultadoRadio(resultado) => {
                if let Some(emisora) = self.guardar_resultado_radio(ctx, &resultado) {
                    let id = emisora.id;
                    ctx.reproductor.enviar(ComandoReproductor::ReemplazarCola {
                        elementos: vec![ElementoCola::Emisora(emisora)],
                        indice: 0,
                    });
                    self.enviar_click_emisora(ctx, id);
                }
            }
            Contexto::Ninguno => {}
        }
    }

    fn elementos_pistas(&mut self, ctx: &ContextoApp<'_>, ids: &[i64]) -> Vec<ElementoCola> {
        match consultas::pistas_resumen_por_ids(ctx.conn, ids) {
            Ok(pistas) => pistas.into_iter().map(ElementoCola::Pista).collect(),
            Err(error) => {
                self.notificar(
                    NivelAviso::Error,
                    format!("No se pudieron leer las pistas: {error:#}"),
                );
                Vec::new()
            }
        }
    }

    fn contexto(&self) -> Contexto {
        match (self.vista, self.pantalla) {
            (Vista::Pistas, _) => {
                if self.pistas.is_empty() {
                    return Contexto::Ninguno;
                }
                let ids: Vec<i64> = self.pistas.iter().map(|pista| pista.id).collect();
                let indice = self.seleccion.min(self.pistas.len() - 1);
                Contexto::Pistas { ids, indice }
            }
            (Vista::Albumes, Pantalla::DetalleAlbum) => {
                let Some(detalle) = self.detalle_album.as_ref() else {
                    return Contexto::Ninguno;
                };
                if detalle.pistas.is_empty() {
                    return Contexto::Ninguno;
                }
                let ids: Vec<i64> = detalle.pistas.iter().map(|pista| pista.id).collect();
                let indice = self.seleccion.min(detalle.pistas.len() - 1);
                Contexto::Pistas { ids, indice }
            }
            (Vista::Artistas, Pantalla::DetalleArtista) => {
                let Some(detalle) = self.detalle_artista.as_ref() else {
                    return Contexto::Ninguno;
                };
                let n_albumes = detalle.albumes.len();
                if self.seleccion < n_albumes {
                    return Contexto::Album {
                        album_id: detalle.albumes[self.seleccion].id,
                    };
                }
                let indice = self.seleccion - n_albumes;
                if indice >= detalle.pistas_sueltas.len() {
                    return Contexto::Ninguno;
                }
                let ids: Vec<i64> = detalle
                    .pistas_sueltas
                    .iter()
                    .map(|pista| pista.id)
                    .collect();
                Contexto::Pistas { ids, indice }
            }
            (Vista::Inicio, _) => {
                let n_recientes = self.inicio.recientes.len();
                let n_anadidos = self.inicio.anadidos.len();
                if self.seleccion < n_recientes {
                    let ids: Vec<i64> =
                        self.inicio.recientes.iter().map(|pista| pista.id).collect();
                    Contexto::Pistas {
                        ids,
                        indice: self.seleccion,
                    }
                } else if self.seleccion < n_recientes + n_anadidos {
                    Contexto::Album {
                        album_id: self.inicio.anadidos[self.seleccion - n_recientes].id,
                    }
                } else if let Some(album) = self
                    .inicio
                    .redescubre
                    .get(self.seleccion - n_recientes - n_anadidos)
                {
                    Contexto::Album { album_id: album.id }
                } else {
                    Contexto::Ninguno
                }
            }
            (Vista::Artistas, Pantalla::Lista) => self
                .artistas
                .get(self.seleccion)
                .map(|artista| Contexto::Artista {
                    artista_id: artista.id,
                })
                .unwrap_or(Contexto::Ninguno),
            (Vista::Albumes, Pantalla::Lista) => self
                .albumes
                .get(self.seleccion)
                .map(|album| Contexto::Album { album_id: album.id })
                .unwrap_or(Contexto::Ninguno),
            (Vista::Buscar, _) => {
                let n_artistas = self.resultados.artistas.len();
                let n_albumes = self.resultados.albumes.len();
                if self.seleccion < n_artistas {
                    Contexto::Artista {
                        artista_id: self.resultados.artistas[self.seleccion].id,
                    }
                } else if self.seleccion < n_artistas + n_albumes {
                    Contexto::Album {
                        album_id: self.resultados.albumes[self.seleccion - n_artistas].id,
                    }
                } else if let Some(_pista) = self
                    .resultados
                    .pistas
                    .get(self.seleccion - n_artistas - n_albumes)
                {
                    let ids: Vec<i64> = self
                        .resultados
                        .pistas
                        .iter()
                        .map(|pista| pista.id)
                        .collect();
                    Contexto::Pistas {
                        ids,
                        indice: self.seleccion - n_artistas - n_albumes,
                    }
                } else {
                    Contexto::Ninguno
                }
            }
            (Vista::Playlists, Pantalla::DetallePlaylist) => {
                let Some((_, pistas)) = self.detalle_playlist.as_ref() else {
                    return Contexto::Ninguno;
                };
                if pistas.is_empty() {
                    return Contexto::Ninguno;
                }
                let ids: Vec<i64> = pistas.iter().map(|pista| pista.id).collect();
                let indice = self.seleccion.min(pistas.len() - 1);
                Contexto::Pistas { ids, indice }
            }
            (Vista::Radio, _) => match self.pestana_radio {
                PestanaRadio::Favoritas | PestanaRadio::Todas => self
                    .emisoras
                    .get(self.seleccion)
                    .cloned()
                    .map(Contexto::Emisora)
                    .unwrap_or(Contexto::Ninguno),
                PestanaRadio::Buscar => {
                    if self.busqueda_radio.campo.is_some() {
                        return Contexto::Ninguno;
                    }
                    if self.config.radio.directorio {
                        self.busqueda_radio
                            .resultados
                            .get(self.seleccion)
                            .cloned()
                            .map(Contexto::ResultadoRadio)
                            .unwrap_or(Contexto::Ninguno)
                    } else {
                        self.emisoras
                            .get(self.seleccion)
                            .cloned()
                            .map(Contexto::Emisora)
                            .unwrap_or(Contexto::Ninguno)
                    }
                }
                PestanaRadio::Sonando => Contexto::Ninguno,
            },
            _ => Contexto::Ninguno,
        }
    }

    fn anadir_seleccion(&mut self, ctx: &ContextoApp<'_>, a_continuacion: bool) {
        let elementos = match self.contexto() {
            Contexto::Pistas { ids, indice } => ids
                .get(indice)
                .copied()
                .map(|id| self.elementos_pistas(ctx, &[id]))
                .unwrap_or_default(),
            Contexto::Album { album_id } => {
                let ids = self.cargar_ids(&consultas::ids_pistas_album(ctx.conn, album_id));
                self.elementos_pistas(ctx, &ids)
            }
            Contexto::Artista { artista_id } => {
                let ids = self.cargar_ids(&consultas::ids_pistas_artista(ctx.conn, artista_id));
                self.elementos_pistas(ctx, &ids)
            }
            Contexto::Emisora(emisora) => vec![ElementoCola::Emisora(emisora)],
            Contexto::ResultadoRadio(resultado) => self
                .guardar_resultado_radio(ctx, &resultado)
                .map(ElementoCola::Emisora)
                .into_iter()
                .collect(),
            Contexto::Ninguno => Vec::new(),
        };
        if elementos.is_empty() {
            return;
        }
        let comando = if a_continuacion {
            ComandoReproductor::ReproducirSiguiente { elementos }
        } else {
            ComandoReproductor::AnadirAlFinal { elementos }
        };
        ctx.reproductor.enviar(comando);
    }

    fn cargar_ids(&mut self, resultado: &anyhow::Result<Vec<i64>>) -> Vec<i64> {
        match resultado {
            Ok(ids) => ids.clone(),
            Err(error) => {
                self.notificar(
                    NivelAviso::Error,
                    format!("No se pudieron leer las pistas: {error:#}"),
                );
                Vec::new()
            }
        }
    }

    fn abrir_album(&mut self, ctx: &ContextoApp<'_>, album_id: i64) {
        match consultas::detalle_album(ctx.conn, album_id) {
            Ok(Some((album, pistas))) => {
                self.pila.push((self.vista, self.pantalla));
                self.detalle_album = Some(DetalleAlbum { album, pistas });
                self.vista = Vista::Albumes;
                self.pantalla = Pantalla::DetalleAlbum;
                self.seleccion = 0;
            }
            Ok(None) => self.notificar(NivelAviso::Aviso, "El álbum ya no existe"),
            Err(error) => self.notificar(
                NivelAviso::Error,
                format!("No se pudo abrir el álbum: {error:#}"),
            ),
        }
    }

    fn abrir_artista(&mut self, ctx: &ContextoApp<'_>, artista_id: i64) {
        match consultas::detalle_artista(ctx.conn, artista_id) {
            Ok(Some(detalle)) => {
                self.pila.push((self.vista, self.pantalla));
                self.detalle_artista = Some(detalle);
                self.vista = Vista::Artistas;
                self.pantalla = Pantalla::DetalleArtista;
                self.seleccion = 0;
            }
            Ok(None) => self.notificar(NivelAviso::Aviso, "El artista ya no existe"),
            Err(error) => self.notificar(
                NivelAviso::Error,
                format!("No se pudo abrir el artista: {error:#}"),
            ),
        }
    }

    fn volver(&mut self) {
        if let Some((vista, pantalla)) = self.pila.pop() {
            self.vista = vista;
            self.pantalla = pantalla;
            self.seleccion = 0;
        }
    }

    fn mover(&mut self, ctx: &ContextoApp<'_>, delta: isize) {
        if self.foco == Foco::Cola {
            if self.estado_reproductor.cola.len() < 2 {
                return;
            }
            let destino = if delta < 0 {
                self.seleccion_cola.checked_sub(1)
            } else if self.seleccion_cola + 1 < self.estado_reproductor.cola.len() {
                Some(self.seleccion_cola + 1)
            } else {
                None
            };
            if let Some(destino) = destino {
                ctx.reproductor.enviar(ComandoReproductor::MoverEnCola {
                    de: self.seleccion_cola,
                    a: destino,
                });
                self.seleccion_cola = destino;
            }
            return;
        }
        if self.vista == Vista::Playlists
            && self.pantalla == Pantalla::DetallePlaylist
            && self.foco == Foco::Contenido
            && !self.es_playlist_favoritas()
        {
            let Some((playlist, pistas)) = self.detalle_playlist.as_ref() else {
                return;
            };
            if pistas.len() < 2 {
                return;
            }
            let destino = if delta < 0 {
                self.seleccion.checked_sub(1)
            } else if self.seleccion + 1 < pistas.len() {
                Some(self.seleccion + 1)
            } else {
                None
            };
            if let Some(destino) = destino {
                let playlist_id = playlist.id;
                if let Err(error) =
                    consultas::playlist::mover(ctx.conn, playlist_id, self.seleccion, destino)
                {
                    self.notificar(
                        NivelAviso::Error,
                        format!("No se pudo reordenar: {error:#}"),
                    );
                    return;
                }
                self.seleccion = destino;
                self.recargar_detalle_playlist(ctx, playlist_id);
            }
        }
    }

    fn playlist_seleccionada(&self) -> Option<PlaylistResumen> {
        match self.pantalla {
            Pantalla::DetallePlaylist => self
                .detalle_playlist
                .as_ref()
                .filter(|(playlist, _)| playlist.id != PLAYLIST_FAVORITAS)
                .map(|(playlist, _)| playlist.clone()),
            _ => self
                .seleccion
                .checked_sub(1)
                .and_then(|indice| self.playlists.get(indice))
                .cloned(),
        }
    }

    fn es_playlist_favoritas(&self) -> bool {
        if self.pantalla == Pantalla::DetallePlaylist {
            return self
                .detalle_playlist
                .as_ref()
                .is_some_and(|(playlist, _)| playlist.id == PLAYLIST_FAVORITAS);
        }
        self.vista == Vista::Playlists && self.pantalla == Pantalla::Lista && self.seleccion == 0
    }

    fn abrir_favoritas(&mut self, ctx: &ContextoApp<'_>) {
        match consultas::favoritas::listar(ctx.conn) {
            Ok(pistas) => {
                self.pila.push((self.vista, self.pantalla));
                self.detalle_playlist = Some((
                    PlaylistResumen {
                        id: PLAYLIST_FAVORITAS,
                        nombre: NOMBRE_FAVORITAS.to_string(),
                        num_pistas: pistas.len() as i64,
                    },
                    pistas,
                ));
                self.pantalla = Pantalla::DetallePlaylist;
                self.seleccion = 0;
            }
            Err(error) => self.notificar(
                NivelAviso::Error,
                format!("No se pudieron abrir las favoritas: {error:#}"),
            ),
        }
    }

    fn quitar_pista_playlist(&mut self, ctx: &ContextoApp<'_>) {
        let Some((playlist, pistas)) = self.detalle_playlist.as_ref() else {
            return;
        };
        if self.seleccion >= pistas.len() {
            return;
        }
        let playlist_id = playlist.id;
        if let Err(error) = consultas::playlist::quitar_pista(ctx.conn, playlist_id, self.seleccion)
        {
            self.notificar(
                NivelAviso::Error,
                format!("No se pudo quitar la pista: {error:#}"),
            );
            return;
        }
        self.recargar_detalle_playlist(ctx, playlist_id);
        self.notificar(NivelAviso::Info, "Pista quitada de la playlist");
    }

    fn recargar_detalle_playlist(&mut self, ctx: &ContextoApp<'_>, playlist_id: i64) {
        let nombre = self
            .detalle_playlist
            .as_ref()
            .map(|(playlist, _)| playlist.nombre.clone())
            .unwrap_or_default();
        let resultado = if playlist_id == PLAYLIST_FAVORITAS {
            consultas::favoritas::listar(ctx.conn)
        } else {
            consultas::playlist::pistas(ctx.conn, playlist_id)
        };
        match resultado {
            Ok(pistas) => {
                let num_pistas = pistas.len() as i64;
                self.detalle_playlist = Some((
                    PlaylistResumen {
                        id: playlist_id,
                        nombre,
                        num_pistas,
                    },
                    pistas,
                ));
                if self.seleccion >= num_pistas as usize {
                    self.seleccion = (num_pistas as usize).saturating_sub(1);
                }
                if playlist_id != PLAYLIST_FAVORITAS {
                    self.refrescar_playlists(ctx);
                }
            }
            Err(error) => self.notificar(
                NivelAviso::Error,
                format!("No se pudo recargar la playlist: {error:#}"),
            ),
        }
    }

    fn abrir_selector_playlist(&mut self, ctx: &ContextoApp<'_>) {
        let pistas = self.ids_contexto_ampliado(ctx);
        if pistas.is_empty() {
            return;
        }
        self.refrescar_playlists(ctx);
        self.dialogo = Some(Dialogo::Selector {
            titulo: "Añadir a…".to_string(),
            pistas,
            seleccion: 0,
        });
    }

    fn ids_contexto_ampliado(&mut self, ctx: &ContextoApp<'_>) -> Vec<i64> {
        match self.contexto() {
            Contexto::Pistas { ids, indice } => ids
                .get(indice)
                .copied()
                .map(|id| vec![id])
                .unwrap_or_default(),
            Contexto::Album { album_id } => {
                self.cargar_ids(&consultas::ids_pistas_album(ctx.conn, album_id))
            }
            Contexto::Artista { artista_id } => {
                self.cargar_ids(&consultas::ids_pistas_artista(ctx.conn, artista_id))
            }
            Contexto::Emisora(_) | Contexto::ResultadoRadio(_) => Vec::new(),
            Contexto::Ninguno => {
                if self.vista == Vista::Playlists && self.pantalla == Pantalla::Lista {
                    if let Some(playlist) = self.playlists.get(self.seleccion) {
                        match consultas::playlist::pistas(ctx.conn, playlist.id) {
                            Ok(pistas) => pistas.into_iter().map(|pista| pista.id).collect(),
                            Err(error) => {
                                self.notificar(
                                    NivelAviso::Error,
                                    format!("No se pudo leer la playlist: {error:#}"),
                                );
                                Vec::new()
                            }
                        }
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn validar_formulario(
        &self,
        accion: &AccionDialogo,
        campos: &[CampoDialogo],
        ctx: &ContextoApp<'_>,
    ) -> Result<(), String> {
        match accion {
            AccionDialogo::NuevaEmisora => {
                self.validar_emisora(ctx, None, &campos[0].valor, &campos[1].valor)
            }
            AccionDialogo::EditarEmisora { id } => {
                self.validar_emisora(ctx, Some(*id), &campos[0].valor, &campos[1].valor)
            }
            _ => Ok(()),
        }
    }

    fn validar_emisora(
        &self,
        ctx: &ContextoApp<'_>,
        id: Option<i64>,
        nombre: &str,
        url: &str,
    ) -> Result<(), String> {
        if !crate::radio::validar_nombre(nombre) {
            return Err("El nombre debe tener entre 1 y 100 caracteres".to_string());
        }
        if !crate::radio::validar_url(url) {
            return Err("La URL debe empezar por http:// o https:// y tener host".to_string());
        }
        let normalizada = consultas::emisoras::normalizar_url(url);
        if let Ok(Some(otra)) = consultas::emisoras::por_url(ctx.conn, &normalizada)
            && Some(otra.id) != id
        {
            return Err("Ya existe una emisora con esa URL".to_string());
        }
        Ok(())
    }

    fn ejecutar_formulario(
        &mut self,
        accion: &AccionDialogo,
        campos: Vec<CampoDialogo>,
        ctx: &ContextoApp<'_>,
    ) {
        match accion {
            AccionDialogo::NuevaEmisora => {
                let nombre = campos[0].valor.trim().to_string();
                let url = consultas::emisoras::normalizar_url(&campos[1].valor);
                let nueva = NuevaEmisora {
                    nombre: nombre.clone(),
                    url,
                    ..NuevaEmisora::default()
                };
                match consultas::emisoras::crear(ctx.conn, &nueva) {
                    Ok(_) => {
                        self.refrescar_radio(ctx);
                        self.notificar(NivelAviso::Info, format!("Emisora «{nombre}» añadida"));
                    }
                    Err(error) => self.notificar(
                        NivelAviso::Error,
                        format!("No se pudo crear la emisora: {error:#}"),
                    ),
                }
            }
            AccionDialogo::EditarEmisora { id } => {
                let nombre = campos[0].valor.trim().to_string();
                let url = campos[1].valor.trim().to_string();
                let pagina_web = campos[2].valor.trim();
                let pagina_web = (!pagina_web.is_empty()).then_some(pagina_web);
                match consultas::emisoras::editar(ctx.conn, *id, &nombre, &url, pagina_web) {
                    Ok(()) => {
                        self.refrescar_radio(ctx);
                        self.notificar(NivelAviso::Info, "Emisora actualizada");
                    }
                    Err(error) => self.notificar(
                        NivelAviso::Error,
                        format!("No se pudo editar la emisora: {error:#}"),
                    ),
                }
            }
            _ => {}
        }
    }

    fn tecla_en_dialogo(&mut self, tecla: &KeyEvent, ctx: &ContextoApp<'_>) -> bool {
        let Some(dialogo) = self.dialogo.take() else {
            return false;
        };
        match dialogo {
            Dialogo::Confirmacion {
                titulo,
                mensaje,
                accion,
            } => match tecla.code {
                KeyCode::Enter | KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.ejecutar_accion_dialogo(accion, String::new(), ctx);
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {}
                _ => {
                    self.dialogo = Some(Dialogo::Confirmacion {
                        titulo,
                        mensaje,
                        accion,
                    });
                }
            },
            Dialogo::Texto {
                titulo,
                mut valor,
                accion,
            } => match tecla.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    self.ejecutar_accion_dialogo(accion, valor, ctx);
                }
                KeyCode::Backspace => {
                    valor.pop();
                    self.dialogo = Some(Dialogo::Texto {
                        titulo,
                        valor,
                        accion,
                    });
                }
                KeyCode::Char(caracter) if !tecla.modifiers.contains(KeyModifiers::CONTROL) => {
                    if valor.chars().count() < 200 {
                        valor.push(caracter);
                    }
                    self.dialogo = Some(Dialogo::Texto {
                        titulo,
                        valor,
                        accion,
                    });
                }
                _ => {
                    self.dialogo = Some(Dialogo::Texto {
                        titulo,
                        valor,
                        accion,
                    });
                }
            },
            Dialogo::Formulario {
                titulo,
                mut campos,
                mut enfocado,
                mut error,
                accion,
            } => match tecla.code {
                KeyCode::Esc => {}
                KeyCode::Tab | KeyCode::Down => {
                    let total = campos.len().max(1);
                    enfocado = (enfocado + 1) % total;
                    error = None;
                    self.dialogo = Some(Dialogo::Formulario {
                        titulo,
                        campos,
                        enfocado,
                        error,
                        accion,
                    });
                }
                KeyCode::BackTab | KeyCode::Up => {
                    let total = campos.len().max(1);
                    enfocado = (enfocado + total - 1) % total;
                    error = None;
                    self.dialogo = Some(Dialogo::Formulario {
                        titulo,
                        campos,
                        enfocado,
                        error,
                        accion,
                    });
                }
                KeyCode::Enter => match self.validar_formulario(&accion, &campos, ctx) {
                    Ok(()) => self.ejecutar_formulario(&accion, campos, ctx),
                    Err(mensaje) => {
                        error = Some(mensaje);
                        self.dialogo = Some(Dialogo::Formulario {
                            titulo,
                            campos,
                            enfocado,
                            error,
                            accion,
                        });
                    }
                },
                KeyCode::Backspace => {
                    if let Some(campo) = campos.get_mut(enfocado) {
                        campo.valor.pop();
                    }
                    error = None;
                    self.dialogo = Some(Dialogo::Formulario {
                        titulo,
                        campos,
                        enfocado,
                        error,
                        accion,
                    });
                }
                KeyCode::Char(caracter) if !tecla.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(campo) = campos.get_mut(enfocado)
                        && campo.valor.chars().count() < 300
                    {
                        campo.valor.push(caracter);
                    }
                    error = None;
                    self.dialogo = Some(Dialogo::Formulario {
                        titulo,
                        campos,
                        enfocado,
                        error,
                        accion,
                    });
                }
                _ => {
                    self.dialogo = Some(Dialogo::Formulario {
                        titulo,
                        campos,
                        enfocado,
                        error,
                        accion,
                    });
                }
            },
            Dialogo::Selector {
                titulo,
                pistas,
                mut seleccion,
            } => match tecla.code {
                KeyCode::Esc => {}
                KeyCode::Char('j') | KeyCode::Down => {
                    let total = self.playlists.len() + 1;
                    if total > 0 {
                        seleccion = (seleccion + 1).min(total - 1);
                    }
                    self.dialogo = Some(Dialogo::Selector {
                        titulo,
                        pistas,
                        seleccion,
                    });
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    seleccion = seleccion.saturating_sub(1);
                    self.dialogo = Some(Dialogo::Selector {
                        titulo,
                        pistas,
                        seleccion,
                    });
                }
                KeyCode::Enter => {
                    if seleccion < self.playlists.len() {
                        let playlist_id = self.playlists[seleccion].id;
                        self.ejecutar_accion_dialogo(
                            AccionDialogo::AnadirAPlaylist {
                                pistas,
                                playlist_id,
                            },
                            String::new(),
                            ctx,
                        );
                    } else {
                        self.dialogo = Some(Dialogo::Texto {
                            titulo: "Nueva playlist".to_string(),
                            valor: String::new(),
                            accion: AccionDialogo::CrearPlaylist { anadir: pistas },
                        });
                    }
                }
                _ => {
                    self.dialogo = Some(Dialogo::Selector {
                        titulo,
                        pistas,
                        seleccion,
                    });
                }
            },
        }
        true
    }

    fn ejecutar_accion_dialogo(
        &mut self,
        accion: AccionDialogo,
        valor: String,
        ctx: &ContextoApp<'_>,
    ) {
        match accion {
            AccionDialogo::CrearPlaylist { anadir } => {
                match consultas::playlist::crear(ctx.conn, &valor) {
                    Ok(Some(playlist_id)) => {
                        if !anadir.is_empty() {
                            let _ =
                                consultas::playlist::anadir_pistas(ctx.conn, playlist_id, &anadir);
                        }
                        self.refrescar_playlists(ctx);
                        self.notificar(
                            NivelAviso::Info,
                            format!("Playlist «{}» creada", valor.trim()),
                        );
                    }
                    Ok(None) => self.notificar(
                        NivelAviso::Error,
                        "Nombre de playlist vacío, demasiado largo o ya existente",
                    ),
                    Err(error) => self.notificar(
                        NivelAviso::Error,
                        format!("No se pudo crear la playlist: {error:#}"),
                    ),
                }
            }
            AccionDialogo::RenombrarPlaylist(playlist_id) => {
                match consultas::playlist::renombrar(ctx.conn, playlist_id, &valor) {
                    Ok(Some(true)) => {
                        self.refrescar_playlists(ctx);
                        if self.pantalla == Pantalla::DetallePlaylist {
                            self.recargar_detalle_playlist(ctx, playlist_id);
                        }
                        self.notificar(
                            NivelAviso::Info,
                            format!("Renombrada a «{}»", valor.trim()),
                        );
                    }
                    Ok(_) => self.notificar(
                        NivelAviso::Error,
                        "Nombre de playlist vacío, demasiado largo o ya existente",
                    ),
                    Err(error) => self.notificar(
                        NivelAviso::Error,
                        format!("No se pudo renombrar: {error:#}"),
                    ),
                }
            }
            AccionDialogo::EliminarPlaylist(playlist_id) => {
                if let Err(error) = consultas::playlist::eliminar(ctx.conn, playlist_id) {
                    self.notificar(NivelAviso::Error, format!("No se pudo eliminar: {error:#}"));
                } else {
                    if self
                        .detalle_playlist
                        .as_ref()
                        .is_some_and(|(playlist, _)| playlist.id == playlist_id)
                    {
                        self.detalle_playlist = None;
                        self.pantalla = Pantalla::Lista;
                    }
                    self.refrescar_playlists(ctx);
                    self.notificar(NivelAviso::Info, "Playlist eliminada");
                }
            }
            AccionDialogo::ExportarPlaylist(playlist_id) => {
                let carpeta = self.config.carpeta_playlists();
                let ruta = carpeta.join(valor.trim());
                if ruta.exists() {
                    self.dialogo = Some(Dialogo::Confirmacion {
                        titulo: "Sobrescribir fichero".to_string(),
                        mensaje: format!("{} ya existe. ¿Sobrescribirlo?", ruta.display()),
                        accion: AccionDialogo::ExportarConfirmado {
                            playlist_id,
                            nombre_fichero: valor,
                        },
                    });
                } else {
                    self.exportar_playlist(ctx, playlist_id, &valor);
                }
            }
            AccionDialogo::ExportarConfirmado {
                playlist_id,
                nombre_fichero,
            } => {
                self.exportar_playlist(ctx, playlist_id, &nombre_fichero);
            }
            AccionDialogo::ExportarFavoritas => {
                let carpeta = self.config.carpeta_playlists();
                let ruta = carpeta.join(valor.trim());
                if ruta.exists() {
                    self.dialogo = Some(Dialogo::Confirmacion {
                        titulo: "Sobrescribir fichero".to_string(),
                        mensaje: format!("{} ya existe. ¿Sobrescribirlo?", ruta.display()),
                        accion: AccionDialogo::ExportarFavoritasConfirmado {
                            nombre_fichero: valor,
                        },
                    });
                } else {
                    self.exportar_favoritas(ctx, &valor);
                }
            }
            AccionDialogo::ExportarFavoritasConfirmado { nombre_fichero } => {
                self.exportar_favoritas(ctx, &nombre_fichero);
            }
            AccionDialogo::ImportarPlaylist => {
                let bruto = valor.trim();
                if bruto.is_empty() {
                    return;
                }
                let candidata = PathBuf::from(bruto);
                let ruta = if candidata.is_absolute() {
                    candidata
                } else {
                    self.config.carpeta_playlists().join(candidata)
                };
                let raices = self.config.carpetas_expandidas();
                match consultas::playlist::importar_m3u(ctx.conn, &ruta, &raices) {
                    Ok((_, anadidas, no_encontradas)) => {
                        self.refrescar_playlists(ctx);
                        let texto_anadidas = if anadidas == 1 {
                            "1 pista añadida".to_string()
                        } else {
                            format!("{anadidas} pistas añadidas")
                        };
                        let texto_no = if no_encontradas == 1 {
                            "1 no encontrada".to_string()
                        } else {
                            format!("{no_encontradas} no encontradas")
                        };
                        self.notificar(NivelAviso::Info, format!("{texto_anadidas}, {texto_no}"));
                    }
                    Err(error) => {
                        self.notificar(NivelAviso::Error, format!("No se pudo importar: {error:#}"))
                    }
                }
            }
            AccionDialogo::AnadirAPlaylist {
                pistas,
                playlist_id,
            } => match consultas::playlist::anadir_pistas(ctx.conn, playlist_id, &pistas) {
                Ok(anadidas) => {
                    self.refrescar_playlists(ctx);
                    if self.pantalla == Pantalla::DetallePlaylist
                        && self
                            .detalle_playlist
                            .as_ref()
                            .is_some_and(|(playlist, _)| playlist.id == playlist_id)
                    {
                        self.recargar_detalle_playlist(ctx, playlist_id);
                    }
                    self.notificar(
                        NivelAviso::Info,
                        format!("{anadidas} pistas añadidas a la playlist"),
                    );
                }
                Err(error) => self.notificar(
                    NivelAviso::Error,
                    format!("No se pudo añadir a la playlist: {error:#}"),
                ),
            },
            AccionDialogo::VaciarCola => {
                ctx.reproductor.enviar(ComandoReproductor::VaciarCola);
            }
            AccionDialogo::NuevaEmisora | AccionDialogo::EditarEmisora { .. } => {}
            AccionDialogo::EliminarEmisora { id } => {
                self.quitar_emisora_de_cola(ctx, id);
                match consultas::emisoras::eliminar(ctx.conn, id) {
                    Ok(()) => {
                        self.refrescar_radio(ctx);
                        self.notificar(NivelAviso::Info, "Emisora eliminada");
                    }
                    Err(error) => self.notificar(
                        NivelAviso::Error,
                        format!("No se pudo eliminar la emisora: {error:#}"),
                    ),
                }
            }
            AccionDialogo::ImportarRadio => {
                let bruto = valor.trim();
                if bruto.is_empty() {
                    return;
                }
                let ruta = crate::config::expandir_ruta(bruto);
                match crate::radio::importar_listas(ctx.conn, &ruta) {
                    Ok(resumen) => {
                        self.refrescar_radio(ctx);
                        self.notificar(NivelAviso::Info, resumen.mensaje());
                    }
                    Err(error) => {
                        self.notificar(NivelAviso::Error, format!("No se pudo importar: {error:#}"))
                    }
                }
            }
            AccionDialogo::ExportarRadioConfirmado => self.exportar_radio(ctx, true),
        }
    }

    fn exportar_playlist(&mut self, ctx: &ContextoApp<'_>, playlist_id: i64, nombre_fichero: &str) {
        let carpeta = self.config.carpeta_playlists();
        match consultas::playlist::exportar_m3u(ctx.conn, playlist_id, &carpeta, nombre_fichero) {
            Ok(ruta) => self.notificar(NivelAviso::Info, format!("Exportada a {}", ruta.display())),
            Err(error) => {
                self.notificar(NivelAviso::Error, format!("No se pudo exportar: {error:#}"))
            }
        }
    }

    fn exportar_favoritas(&mut self, ctx: &ContextoApp<'_>, nombre_fichero: &str) {
        let carpeta = self.config.carpeta_playlists();
        let resultado = consultas::favoritas::listar(ctx.conn).and_then(|pistas| {
            consultas::playlist::exportar_m3u_pistas(&carpeta, nombre_fichero, &pistas)
        });
        match resultado {
            Ok(ruta) => self.notificar(NivelAviso::Info, format!("Exportada a {}", ruta.display())),
            Err(error) => {
                self.notificar(NivelAviso::Error, format!("No se pudo exportar: {error:#}"))
            }
        }
    }

    fn pista_actual_id(&self) -> Option<i64> {
        self.estado_reproductor.pista_actual().map(|pista| pista.id)
    }

    fn pista_bajo_cursor(&self) -> Option<i64> {
        if self.foco == Foco::Cola {
            return self
                .estado_reproductor
                .cola
                .get(self.seleccion_cola)
                .and_then(|elemento| elemento.pista_id())
                .or_else(|| self.pista_actual_id());
        }
        let seleccionada = match self.contexto() {
            Contexto::Pistas { ids, indice } => ids.get(indice).copied(),
            _ => None,
        };
        seleccionada.or_else(|| self.pista_actual_id())
    }

    fn alternar_favorita(&mut self, ctx: &ContextoApp<'_>) {
        if self.vista == Vista::Radio {
            if let Some(resultado) = self.resultado_bajo_cursor() {
                if let Some(emisora) = self.guardar_resultado_radio(ctx, &resultado) {
                    self.alternar_favorita_emisora(ctx, emisora.id);
                }
                return;
            }
            if let Some(emisora) = self
                .emisora_bajo_cursor()
                .or_else(|| self.estado_reproductor.emisora_actual().cloned())
            {
                self.alternar_favorita_emisora(ctx, emisora.id);
                return;
            }
        } else if self.estado_reproductor.es_emisora()
            && self.pista_bajo_cursor().is_none()
            && let Some(emisora) = self.estado_reproductor.emisora_actual().cloned()
        {
            self.alternar_favorita_emisora(ctx, emisora.id);
            return;
        }
        let Some(pista_id) = self.pista_bajo_cursor() else {
            return;
        };
        match consultas::favoritas::alternar(ctx.conn, pista_id) {
            Ok(true) => {
                self.favoritas.insert(pista_id);
                let titulo = consultas::pistas_resumen_por_ids(ctx.conn, &[pista_id])
                    .ok()
                    .and_then(|pistas| pistas.into_iter().next())
                    .map(|pista| pista.titulo)
                    .unwrap_or_else(|| format!("#{pista_id}"));
                self.notificar(NivelAviso::Info, format!("♥ {titulo}"));
                ctx.scrobbling.enviar(ComandoScrobbling::Amar(pista_id));
            }
            Ok(false) => {
                self.favoritas.remove(&pista_id);
                self.notificar(NivelAviso::Info, "Quitada de favoritas".to_string());
                ctx.scrobbling.enviar(ComandoScrobbling::Desamar(pista_id));
            }
            Err(error) => self.notificar(
                NivelAviso::Error,
                format!("No se pudo actualizar la favorita: {error:#}"),
            ),
        }
        if self.es_playlist_favoritas() {
            self.recargar_detalle_playlist(ctx, PLAYLIST_FAVORITAS);
        }
    }

    pub fn refrescar_favoritas(&mut self, ctx: &ContextoApp<'_>) {
        match consultas::favoritas::ids(ctx.conn) {
            Ok(ids) => self.favoritas = ids.into_iter().collect(),
            Err(error) => self.notificar(
                NivelAviso::Error,
                format!("No se pudieron leer las favoritas: {error:#}"),
            ),
        }
    }

    fn asegurar_pagina(&mut self, ctx: &ContextoApp<'_>) {
        let cargados = self.pistas.len();
        if cargados == 0 || self.total_pistas as usize <= cargados {
            return;
        }
        if self.seleccion + 1 >= cargados {
            self.pagina_pistas += 1;
            if let Ok(siguientes) = consultas::listar_pistas(
                ctx.conn,
                self.orden_pistas,
                self.orden_descendente,
                self.pagina_pistas,
            ) {
                self.pistas.extend(siguientes);
            }
        }
    }

    fn pagina_nueva(&mut self, ctx: &ContextoApp<'_>) {
        self.seleccion = 0;
        self.pagina_pistas = 0;
        self.refrescar_pistas(ctx);
    }

    fn buscar(&mut self, ctx: &ContextoApp<'_>, ms: i64) {
        ctx.reproductor
            .enviar(ComandoReproductor::Buscar { ms, relativo: true });
    }

    fn cambiar_volumen(&mut self, ctx: &ContextoApp<'_>, delta: i32) {
        let nuevo = (i32::from(self.estado_reproductor.volumen) + delta).clamp(0, 100) as u8;
        ctx.reproductor
            .enviar(ComandoReproductor::Volumen { valor: nuevo });
    }

    pub fn refrescar_vista(&mut self, vista: Vista, ctx: &ContextoApp<'_>) {
        match vista {
            Vista::Inicio => {
                match consultas::inicio(ctx.conn) {
                    Ok(inicio) => self.inicio = inicio,
                    Err(error) => self.notificar(
                        NivelAviso::Error,
                        format!("No se pudo cargar Inicio: {error:#}"),
                    ),
                }
                let total = self.inicio.recientes.len()
                    + self.inicio.anadidos.len()
                    + self.inicio.redescubre.len();
                if total == 0 {
                    self.seleccion = 0;
                } else {
                    self.seleccion = self.seleccion.min(total - 1);
                }
                self.sincronizar_inicio_desde_seleccion();
                self.refrescar_playlists(ctx);
            }
            Vista::Artistas => match consultas::listar_artistas(ctx.conn) {
                Ok(artistas) => self.artistas = artistas,
                Err(error) => self.notificar(
                    NivelAviso::Error,
                    format!("No se pudieron listar los artistas: {error:#}"),
                ),
            },
            Vista::Albumes => self.refrescar_albumes(ctx),
            Vista::Pistas => self.pagina_nueva(ctx),
            Vista::Playlists => self.refrescar_playlists(ctx),
            Vista::Radio => self.refrescar_radio(ctx),
            Vista::Buscar | Vista::Visual => {}
        }
    }

    pub fn refrescar_albumes(&mut self, ctx: &ContextoApp<'_>) {
        match consultas::listar_albumes(
            ctx.conn,
            self.orden_albumes,
            self.orden_albumes_descendente,
            None,
        ) {
            Ok(albumes) => {
                self.albumes = albumes;
                if self.seleccion >= self.albumes.len() {
                    self.seleccion = self.albumes.len().saturating_sub(1);
                }
            }
            Err(error) => self.notificar(
                NivelAviso::Error,
                format!("No se pudieron listar los álbumes: {error:#}"),
            ),
        }
    }

    pub fn refrescar_playlists(&mut self, ctx: &ContextoApp<'_>) {
        match consultas::playlist::listar(ctx.conn) {
            Ok(playlists) => self.playlists = playlists,
            Err(error) => self.notificar(
                NivelAviso::Error,
                format!("No se pudieron listar las playlists: {error:#}"),
            ),
        }
    }

    pub fn refrescar_pistas(&mut self, ctx: &ContextoApp<'_>) {
        match consultas::listar_pistas(
            ctx.conn,
            self.orden_pistas,
            self.orden_descendente,
            self.pagina_pistas,
        ) {
            Ok(pistas) => {
                self.pistas = pistas;
                if self.seleccion >= self.pistas.len() {
                    self.seleccion = self.pistas.len().saturating_sub(1);
                }
            }
            Err(error) => self.notificar(
                NivelAviso::Error,
                format!("No se pudieron listar las pistas: {error:#}"),
            ),
        }
    }

    pub fn refrescar_contadores(&mut self, ctx: &ContextoApp<'_>) {
        if let Ok(total) = consultas::contar_pistas(ctx.conn) {
            self.total_pistas = total;
        }
    }

    // ---------- Radio ----------

    pub fn refrescar_radio(&mut self, ctx: &ContextoApp<'_>) {
        self.solicitar_logos_pendientes();
        if self.pestana_radio == PestanaRadio::Sonando {
            self.refrescar_titulos_radio(ctx);
            self.seleccion = self
                .seleccion
                .min(self.radio_titulos.len().saturating_sub(1));
            return;
        }
        if self.pestana_radio == PestanaRadio::Buscar {
            if !self.config.radio.directorio {
                let texto = self.busqueda_radio.nombre.clone();
                match consultas::emisoras::buscar_local(ctx.conn, &texto, false) {
                    Ok(emisoras) => self.emisoras = emisoras,
                    Err(error) => self.notificar(
                        NivelAviso::Error,
                        format!("No se pudo buscar en las emisoras: {error:#}"),
                    ),
                }
            }
            self.seleccion = self
                .seleccion
                .min(self.total_seleccionable().saturating_sub(1));
            return;
        }
        let solo_favoritas = self.pestana_radio == PestanaRadio::Favoritas;
        match consultas::emisoras::listar(
            ctx.conn,
            solo_favoritas,
            self.orden_emisoras,
            self.orden_emisoras_descendente,
        ) {
            Ok(emisoras) => self.emisoras = emisoras,
            Err(error) => self.notificar(
                NivelAviso::Error,
                format!("No se pudieron listar las emisoras: {error:#}"),
            ),
        }
        self.seleccion = self.seleccion.min(self.emisoras.len().saturating_sub(1));
    }

    fn refrescar_titulos_radio(&mut self, ctx: &ContextoApp<'_>) {
        self.radio_titulos.clear();
        self.radio_titulos_ok.clear();
        let Some(emisora) = self.estado_reproductor.emisora_actual().cloned() else {
            return;
        };
        match consultas::titulos_emisora::listar(ctx.conn, emisora.id, 50) {
            Ok(titulos) => self.radio_titulos = titulos,
            Err(error) => {
                tracing::warn!("no se pudieron leer los títulos de la emisora: {error:#}");
                return;
            }
        }
        for titulo in &self.radio_titulos {
            let Some(parsed) = icy::parsear(&titulo.titulo, &emisora.nombre) else {
                continue;
            };
            let Some(artista) = parsed.artista else {
                continue;
            };
            if consultas::existe_pista_por_artista_titulo(ctx.conn, &artista, &parsed.titulo)
                .unwrap_or(false)
            {
                self.radio_titulos_ok.insert(titulo.id);
            }
        }
    }

    fn solicitar_logos_pendientes(&mut self) {
        if !self.config.radio.logos {
            return;
        }
        let Some(directorio) = self.directorio.as_ref() else {
            return;
        };
        let pendientes: Vec<(i64, String)> = self
            .emisoras
            .iter()
            .filter_map(|emisora| match (&emisora.logo_ruta, &emisora.logo_url) {
                (None, Some(url)) if !url.trim().is_empty() => Some((emisora.id, url.clone())),
                _ => None,
            })
            .collect();
        for (emisora_id, url) in pendientes {
            directorio.enviar(ComandoDirectorio::Logo { emisora_id, url });
        }
    }

    fn emisora_bajo_cursor(&self) -> Option<EmisoraResumen> {
        if self.vista != Vista::Radio {
            return None;
        }
        match self.pestana_radio {
            PestanaRadio::Favoritas | PestanaRadio::Todas => {
                self.emisoras.get(self.seleccion).cloned()
            }
            _ => None,
        }
    }

    fn resultado_bajo_cursor(&self) -> Option<EmisoraDirectorio> {
        if self.vista != Vista::Radio
            || self.pestana_radio != PestanaRadio::Buscar
            || self.busqueda_radio.campo.is_some()
        {
            return None;
        }
        if !self.config.radio.directorio {
            return None;
        }
        self.busqueda_radio.resultados.get(self.seleccion).cloned()
    }

    fn cambiar_pestana_radio(&mut self, ctx: &ContextoApp<'_>, nueva: PestanaRadio) {
        self.seleccion_radio[self.pestana_radio.indice()] = self.seleccion;
        self.pestana_radio = nueva;
        self.seleccion = self.seleccion_radio[nueva.indice()];
        if nueva == PestanaRadio::Buscar && self.busqueda_radio.campo.is_none() {
            self.busqueda_radio.campo = Some(CampoBusquedaRadio::Nombre);
        } else if nueva != PestanaRadio::Buscar {
            self.busqueda_radio.campo = None;
        }
        self.guardar_ajuste(ctx, "radio_pestana", nueva.como_str());
        self.refrescar_radio(ctx);
    }

    fn enfocar_busqueda_radio(&mut self, ctx: &ContextoApp<'_>) {
        self.pestana_radio = PestanaRadio::Buscar;
        self.busqueda_radio.campo = Some(CampoBusquedaRadio::Nombre);
        self.guardar_ajuste(ctx, "radio_pestana", PestanaRadio::Buscar.como_str());
        self.refrescar_radio(ctx);
    }

    fn guardar_resultado_radio(
        &mut self,
        ctx: &ContextoApp<'_>,
        resultado: &EmisoraDirectorio,
    ) -> Option<EmisoraResumen> {
        let nombre = resultado.nombre_limpio();
        let url = crate::radio::limpiar_texto(resultado.url());
        if nombre.is_empty() || !crate::radio::validar_url(&url) {
            self.notificar(
                NivelAviso::Error,
                "La emisora no tiene nombre o URL válidos",
            );
            return None;
        }
        let url = consultas::emisoras::normalizar_url(&url);
        let uuid = resultado.uuid.trim();
        if !uuid.is_empty()
            && let Ok(Some(emisora)) = consultas::emisoras::por_uuid(ctx.conn, uuid)
        {
            return Some(emisora.a_resumen());
        }
        if let Ok(Some(emisora)) = consultas::emisoras::por_url(ctx.conn, &url) {
            return Some(emisora.a_resumen());
        }
        let nueva = NuevaEmisora {
            nombre: nombre.clone(),
            url,
            pagina_web: Some(resultado.pagina_web.clone()).filter(|v| !v.trim().is_empty()),
            pais: resultado.pais_limpio(),
            etiquetas: Some(crate::radio::limpiar_texto(&resultado.etiquetas))
                .filter(|v| !v.is_empty()),
            codec: Some(resultado.codec.clone()).filter(|v| !v.trim().is_empty()),
            bitrate_kbps: (resultado.bitrate > 0).then_some(resultado.bitrate),
            logo_url: resultado.url_logo(),
            radiobrowser_uuid: (!uuid.is_empty()).then(|| uuid.to_string()),
            favorita: false,
        };
        match consultas::emisoras::crear(ctx.conn, &nueva) {
            Ok(id) => {
                self.notificar(NivelAviso::Info, format!("Emisora «{nombre}» añadida"));
                consultas::emisoras::por_id(ctx.conn, id)
                    .ok()
                    .flatten()
                    .map(|emisora| emisora.a_resumen())
            }
            Err(error) => {
                self.notificar(
                    NivelAviso::Error,
                    format!("No se pudo añadir la emisora: {error:#}"),
                );
                None
            }
        }
    }

    fn lanzar_busqueda_radio(&mut self, ctx: &ContextoApp<'_>) {
        self.busqueda_radio.campo = None;
        if !self.config.radio.directorio {
            let texto = self.busqueda_radio.nombre.clone();
            match consultas::emisoras::buscar_local(ctx.conn, &texto, false) {
                Ok(emisoras) => {
                    self.emisoras = emisoras;
                    self.seleccion = 0;
                }
                Err(error) => self.notificar(
                    NivelAviso::Error,
                    format!("No se pudo buscar en las emisoras: {error:#}"),
                ),
            }
            return;
        }
        let clave = self.busqueda_radio.clave();
        if let Ok(Some((json, obtenido_en))) = consultas::busquedas_radio::leer(ctx.conn, &clave)
            && cache_vigente(&obtenido_en)
        {
            self.cargar_resultados_radio(&clave, &json, aviso_cache(&obtenido_en));
            return;
        }
        let Some(directorio) = self.directorio.as_ref() else {
            self.notificar(NivelAviso::Aviso, "El directorio de radio está desactivado");
            return;
        };
        self.busqueda_radio.resultados.clear();
        self.busqueda_radio.buscando = true;
        self.busqueda_radio.aviso = None;
        self.seleccion = 0;
        let enviado = directorio.enviar(ComandoDirectorio::Buscar {
            clave,
            nombre: self.busqueda_radio.nombre.clone(),
            pais: self.busqueda_radio.pais.clone(),
            etiqueta: self.busqueda_radio.etiqueta.clone(),
        });
        if !enviado {
            self.busqueda_radio.buscando = false;
            self.notificar(
                NivelAviso::Error,
                "No se pudo consultar el directorio de radio",
            );
        }
    }

    fn cargar_resultados_radio(&mut self, clave: &str, json: &str, aviso: Option<String>) {
        if clave != self.busqueda_radio.clave() {
            return;
        }
        match serde_json::from_str::<Vec<EmisoraDirectorio>>(json) {
            Ok(resultados) => {
                self.busqueda_radio.resultados = resultados;
                self.busqueda_radio.buscando = false;
                self.busqueda_radio.aviso = aviso;
                self.seleccion_radio[PestanaRadio::Buscar.indice()] = 0;
                self.seleccion = 0;
            }
            Err(error) => {
                tracing::warn!("resultados de radio ilegibles: {error}");
                self.busqueda_radio.buscando = false;
                self.busqueda_radio.aviso = Some("resultados ilegibles".to_string());
            }
        }
    }

    fn manejar_resultados_radio(&mut self, clave: &str, ctx: &ContextoApp<'_>) {
        if clave != self.busqueda_radio.clave() {
            return;
        }
        match consultas::busquedas_radio::leer(ctx.conn, clave) {
            Ok(Some((json, obtenido_en))) => {
                let aviso = aviso_cache(&obtenido_en);
                self.cargar_resultados_radio(clave, &json, aviso);
            }
            Ok(None) => {
                self.busqueda_radio.buscando = false;
            }
            Err(error) => {
                tracing::warn!("no se pudieron leer los resultados de radio: {error:#}");
                self.busqueda_radio.buscando = false;
            }
        }
    }

    fn manejar_logo_listo(&mut self, _emisora_id: i64, ctx: &ContextoApp<'_>) {
        self.refrescar_radio(ctx);
    }

    fn buscar_titulo_icy(&mut self, ctx: &ContextoApp<'_>) {
        let texto = if self.vista == Vista::Radio && self.pestana_radio == PestanaRadio::Sonando {
            self.radio_titulos
                .get(self.seleccion)
                .map(|titulo| titulo.titulo.clone())
        } else {
            self.estado_reproductor.titulo_icy.clone()
        };
        let Some(texto) = texto.filter(|texto| !texto.trim().is_empty()) else {
            self.notificar(NivelAviso::Info, "No hay ningún título ICY que buscar");
            return;
        };
        let consulta = icy::parsear(&texto, "")
            .map(|parsed| match parsed.artista {
                Some(artista) => format!("{artista} {}", parsed.titulo),
                None => parsed.titulo,
            })
            .unwrap_or(texto);
        self.vista = Vista::Buscar;
        self.pantalla = Pantalla::Lista;
        self.foco = Foco::Contenido;
        self.busqueda_enfocada = true;
        self.busqueda = consulta;
        self.ejecutar_busqueda(ctx);
    }

    fn tecla_en_radio(&mut self, tecla: &KeyEvent, ctx: &ContextoApp<'_>) -> bool {
        if self.vista != Vista::Radio
            || self.pestana_radio != PestanaRadio::Buscar
            || self.busqueda_radio.campo.is_none()
        {
            return false;
        }
        let campo = self
            .busqueda_radio
            .campo
            .unwrap_or(CampoBusquedaRadio::Nombre);
        match tecla.code {
            KeyCode::Char(caracter)
                if !caracter.is_control() && !tecla.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                let valor = self.busqueda_radio.valor_mut(campo);
                if valor.chars().count() < 80 {
                    valor.push(caracter);
                }
                true
            }
            KeyCode::Backspace => {
                self.busqueda_radio.valor_mut(campo).pop();
                true
            }
            KeyCode::Tab => {
                self.busqueda_radio.campo = Some(campo.siguiente());
                true
            }
            KeyCode::BackTab => {
                self.busqueda_radio.campo = Some(campo.anterior());
                true
            }
            KeyCode::Enter => {
                self.lanzar_busqueda_radio(ctx);
                true
            }
            KeyCode::Esc => {
                self.busqueda_radio.campo = None;
                true
            }
            _ => false,
        }
    }

    fn alternar_favorita_emisora(&mut self, ctx: &ContextoApp<'_>, emisora_id: i64) {
        match consultas::emisoras::alternar_favorita(ctx.conn, emisora_id) {
            Ok(favorita) => {
                self.refrescar_radio(ctx);
                self.notificar(
                    NivelAviso::Info,
                    if favorita {
                        "Emisora marcada como favorita"
                    } else {
                        "Emisora quitada de favoritas"
                    },
                );
            }
            Err(error) => self.notificar(
                NivelAviso::Error,
                format!("No se pudo cambiar la favorita: {error:#}"),
            ),
        }
    }

    fn nueva_emisora_dialogo(&mut self) {
        self.dialogo = Some(Dialogo::Formulario {
            titulo: "Nueva emisora".to_string(),
            campos: vec![
                CampoDialogo {
                    etiqueta: "Nombre".to_string(),
                    valor: String::new(),
                    pista: None,
                },
                CampoDialogo {
                    etiqueta: "URL".to_string(),
                    valor: String::new(),
                    pista: Some("https://…".to_string()),
                },
            ],
            enfocado: 0,
            error: None,
            accion: AccionDialogo::NuevaEmisora,
        });
    }

    fn editar_emisora_dialogo(&mut self, ctx: &ContextoApp<'_>, id: i64) {
        let Ok(Some(emisora)) = consultas::emisoras::por_id(ctx.conn, id) else {
            self.notificar(NivelAviso::Aviso, "La emisora ya no existe");
            return;
        };
        self.dialogo = Some(Dialogo::Formulario {
            titulo: "Editar emisora".to_string(),
            campos: vec![
                CampoDialogo {
                    etiqueta: "Nombre".to_string(),
                    valor: emisora.nombre,
                    pista: None,
                },
                CampoDialogo {
                    etiqueta: "URL".to_string(),
                    valor: emisora.url,
                    pista: None,
                },
                CampoDialogo {
                    etiqueta: "Página web".to_string(),
                    valor: emisora.pagina_web.unwrap_or_default(),
                    pista: Some("opcional".to_string()),
                },
            ],
            enfocado: 0,
            error: None,
            accion: AccionDialogo::EditarEmisora { id },
        });
    }

    fn eliminar_emisora_dialogo(&mut self, ctx: &ContextoApp<'_>, id: i64) {
        let nombre = consultas::emisoras::por_id(ctx.conn, id)
            .ok()
            .flatten()
            .map(|emisora| emisora.nombre)
            .unwrap_or_else(|| "la emisora".to_string());
        self.dialogo = Some(Dialogo::Confirmacion {
            titulo: "Eliminar emisora".to_string(),
            mensaje: format!(
                "¿Eliminar «{nombre}»? Se borrarán también sus títulos, su historial y su presencia en la cola."
            ),
            accion: AccionDialogo::EliminarEmisora { id },
        });
    }

    fn importar_radio_dialogo(&mut self) {
        let valor = self
            .config
            .carpeta_playlists()
            .to_string_lossy()
            .to_string();
        self.dialogo = Some(Dialogo::Texto {
            titulo: "Importar PLS/M3U (ruta del fichero)".to_string(),
            valor,
            accion: AccionDialogo::ImportarRadio,
        });
    }

    fn exportar_radio(&mut self, ctx: &ContextoApp<'_>, confirmado: bool) {
        let carpeta = self.config.carpeta_playlists();
        let ruta = carpeta.join(format!("{}.m3u8", crate::radio::NOMBRE_EXPORTACION));
        if ruta.exists() && !confirmado {
            self.dialogo = Some(Dialogo::Confirmacion {
                titulo: "Sobrescribir exportación".to_string(),
                mensaje: format!("{} ya existe. ¿Sobrescribirlo?", ruta.display()),
                accion: AccionDialogo::ExportarRadioConfirmado,
            });
            return;
        }
        match crate::radio::exportar_favoritas(ctx.conn, &carpeta, crate::radio::NOMBRE_EXPORTACION)
        {
            Ok(ruta) => self.notificar(
                NivelAviso::Info,
                format!("Emisoras favoritas exportadas a {}", ruta.display()),
            ),
            Err(error) => self.notificar(
                NivelAviso::Error,
                format!("No se pudieron exportar las emisoras: {error:#}"),
            ),
        }
    }

    fn quitar_emisora_de_cola(&self, ctx: &ContextoApp<'_>, emisora_id: i64) {
        let posiciones: Vec<usize> = self
            .estado_reproductor
            .cola
            .iter()
            .enumerate()
            .filter(|(_, elemento)| elemento.emisora_id() == Some(emisora_id))
            .map(|(posicion, _)| posicion)
            .collect();
        for posicion in posiciones.into_iter().rev() {
            ctx.reproductor
                .enviar(ComandoReproductor::EliminarDeCola { posicion });
        }
    }

    fn enviar_click_emisora(&self, ctx: &ContextoApp<'_>, emisora_id: i64) {
        let Some(directorio) = self.directorio.as_ref() else {
            return;
        };
        if let Ok(Some(emisora)) = consultas::emisoras::por_id(ctx.conn, emisora_id)
            && let Some(uuid) = emisora.radiobrowser_uuid
            && !uuid.trim().is_empty()
        {
            directorio.enviar(ComandoDirectorio::Click(uuid));
        }
    }

    fn manejar_escaneo(&mut self, evento: EventoEscaneo, ctx: &ContextoApp<'_>) {
        match evento {
            EventoEscaneo::Iniciado { total } => {
                self.escaneo_activo = Some(ProgresoEscaneo {
                    procesadas: 0,
                    total,
                });
            }
            EventoEscaneo::Progreso { procesadas, total } => {
                self.escaneo_activo = Some(ProgresoEscaneo { procesadas, total });
            }
            EventoEscaneo::Terminado { resumen } => {
                self.escaneo_activo = None;
                self.manejo_escaneo = None;
                self.refrescar_contadores(ctx);
                self.refrescar_favoritas(ctx);
                if self.pantalla == Pantalla::Lista {
                    self.refrescar_vista(self.vista, ctx);
                }
                self.notificar(NivelAviso::Info, resumen.mensaje());
            }
            EventoEscaneo::Cancelado { resumen } => {
                self.escaneo_activo = None;
                self.manejo_escaneo = None;
                self.notificar(
                    NivelAviso::Aviso,
                    format!(
                        "Escaneo cancelado: {} nuevas, {} actualizadas",
                        resumen.nuevas, resumen.actualizadas
                    ),
                );
            }
            EventoEscaneo::Error { mensaje } => {
                self.escaneo_activo = None;
                self.manejo_escaneo = None;
                self.notificar(NivelAviso::Error, format!("Escaneo: {mensaje}"));
            }
        }
    }

    pub fn iniciar_escaneo(&mut self, ctx: &ContextoApp<'_>) {
        self.iniciar_escaneo_modo(ctx, escaner::ModoEscaneo::Incremental);
    }

    pub fn iniciar_escaneo_modo(&mut self, ctx: &ContextoApp<'_>, modo: escaner::ModoEscaneo) {
        if let Some(anterior) = self.manejo_escaneo.take() {
            escaner::cancelar_y_esperar(anterior);
        }
        let raices = self.config.carpetas_expandidas();
        match escaner::lanzar(
            ctx.ruta_bd.to_path_buf(),
            raices,
            ctx.dir_caratulas.to_path_buf(),
            modo,
            ctx.tx_app.clone(),
        ) {
            Ok(manejo) => {
                self.manejo_escaneo = Some(manejo);
                let mensaje = if modo.es_completo() {
                    "Reescaneando la biblioteca al completo…"
                } else {
                    "Escaneando biblioteca…"
                };
                self.notificar(NivelAviso::Info, mensaje);
            }
            Err(error) => {
                self.notificar(NivelAviso::Error, format!("No se pudo escanear: {error:#}"));
            }
        }
    }

    pub fn recargar_tema(&mut self) {
        let (paleta, aviso) = tema::cargar(&self.config.tema, &self.ruta_tema);
        self.paleta = paleta;
        if let Some(aviso) = aviso {
            self.notificar(NivelAviso::Aviso, aviso);
        } else {
            self.notificar(NivelAviso::Info, "Tema recargado");
        }
    }

    pub fn cancelar_escaneo(&mut self) {
        if let Some(manejo) = self.manejo_escaneo.take() {
            manejo.cancelar();
        }
    }

    pub fn esta_detenido(&self) -> bool {
        self.estado_reproductor.estado == Estado::Detenido
            && self.estado_reproductor.pista_actual().is_none()
    }
}

fn en_rect(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom()
}

fn cache_vigente(obtenido_en: &str) -> bool {
    bd::unix_desde_iso(obtenido_en)
        .map(|unix| bd::ahora_unix() - unix < 24 * 3600)
        .unwrap_or(false)
}

fn aviso_cache(obtenido_en: &str) -> Option<String> {
    let unix = bd::unix_desde_iso(obtenido_en)?;
    let horas = (bd::ahora_unix() - unix).max(0) / 3600;
    if horas <= 0 {
        None
    } else if horas < 24 {
        Some(format!("caché, hace {horas} h"))
    } else {
        Some(format!("caché, hace {} d", horas / 24))
    }
}

fn mover_indice(actual: usize, ultimo: usize, delta: isize) -> usize {
    if delta == isize::MIN / 2 {
        return 0;
    }
    if delta == isize::MAX / 2 {
        return ultimo;
    }
    (actual as isize + delta).clamp(0, ultimo as isize) as usize
}

/// Movimiento vertical en una rejilla: conserva la columna y no salta de
/// fila en los extremos.
fn mover_en_rejilla(actual: usize, ultimo: usize, columnas: usize, filas: isize) -> usize {
    let columnas = columnas.max(1);
    let fila = actual / columnas;
    let columna = actual % columnas;
    let ultima_fila = ultimo / columnas;
    let destino = (fila as isize + filas).clamp(0, ultima_fila as isize) as usize;
    (destino * columnas + columna).min(ultimo)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::biblioteca::modelos::{AlbumResumen, PistaListado};
    use crate::config::ConfigTema;

    fn app_con_albumes(cantidad: usize) -> AppEstado {
        let mut app = AppEstado::nuevo(
            Config::default(),
            crate::tema::terminal(&ConfigTema::default()),
            0,
            std::path::PathBuf::from("/tmp/tema"),
        );
        app.vista = Vista::Albumes;
        app.pantalla = Pantalla::Lista;
        app.columnas_rejilla = 5;
        app.albumes = (1..=cantidad as i64)
            .map(|id| AlbumResumen {
                id,
                titulo: format!("Álbum {id}"),
                artista: "Artista".to_string(),
                anio: Some(2024),
                caratula_ruta: None,
                num_pistas: 1,
                duracion_ms: 1_000,
            })
            .collect();
        app
    }

    #[test]
    fn el_movimiento_vertical_de_rejilla_conserva_la_columna() {
        assert_eq!(mover_en_rejilla(6, 11, 5, -1), 1, "sube una fila");
        assert_eq!(mover_en_rejilla(1, 11, 5, 1), 6, "baja una fila");
        assert_eq!(mover_en_rejilla(7, 11, 5, 1), 11, "fila final incompleta");
        assert_eq!(mover_en_rejilla(11, 11, 5, 1), 11, "no salta en el fondo");
        assert_eq!(mover_en_rejilla(3, 11, 5, -1), 3, "no salta en el tope");
        assert_eq!(mover_en_rejilla(11, 11, 5, -1), 6, "recupera su columna");
    }

    #[test]
    fn las_flechas_mueven_en_todas_las_direcciones_en_la_rejilla() {
        let mut app = app_con_albumes(12);
        app.seleccion = 6;
        app.desplazar_seleccion(Eje::Vertical, -1);
        assert_eq!(app.seleccion, 1, "arriba");
        app.desplazar_seleccion(Eje::Vertical, 1);
        assert_eq!(app.seleccion, 6, "abajo");
        app.desplazar_seleccion(Eje::Horizontal, 1);
        assert_eq!(app.seleccion, 7, "derecha");
        app.desplazar_seleccion(Eje::Horizontal, -1);
        assert_eq!(app.seleccion, 6, "izquierda");
    }

    #[test]
    fn el_movimiento_vertical_en_listas_es_una_fila() {
        let mut app = app_con_albumes(12);
        app.vista = Vista::Pistas;
        app.pistas = (1..=12)
            .map(|id| PistaListado {
                id,
                ..PistaListado::default()
            })
            .collect();
        app.seleccion = 6;
        app.desplazar_seleccion(Eje::Vertical, -1);
        assert_eq!(app.seleccion, 5, "en listas cada flecha mueve un elemento");
    }

    #[test]
    fn cerrar_la_ayuda_no_sale_del_modo_visual() {
        let mut app = app_con_albumes(1);
        app.entrar_visual(false);
        app.ayuda_visible = true;
        assert!(app.cerrar_ayuda(), "el primer Esc cierra la ayuda");
        assert!(!app.ayuda_visible);
        assert!(
            app.modo_visual.is_some(),
            "cerrar la ayuda no debe salir del modo visual"
        );
        assert!(
            !app.cerrar_ayuda(),
            "el siguiente Esc ya no tiene ayuda que cerrar"
        );
    }
}
