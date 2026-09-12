use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use rusqlite::Connection;

use crate::biblioteca::consultas;
use crate::biblioteca::consultas::{LIMITE_BUSQUEDA, ResultadosBusqueda};
use crate::biblioteca::consultas::{OrdenAlbumes, OrdenPistas};
use crate::biblioteca::escaner::{self, ManejoEscaneo};
use crate::biblioteca::modelos::{
    AlbumResumen, ArtistaResumen, DetalleAlbum, DetalleArtista, Inicio, PistaListado,
    PlaylistResumen,
};
use crate::config::Config;
use crate::eventos::{AppEvento, EventoEscaneo, NivelAviso};
use crate::reproductor::estado::{Estado, EstadoReproduccion};
use crate::reproductor::{ComandoReproductor, ManejoReproductor};
use crate::scrobbling::estado::EstadoScrobbling;
use crate::scrobbling::{ComandoScrobbling, ManejoScrobbling};
use crate::tema::{self, Paleta};
use crate::ui::Iconos;
use crate::ui::componentes::imagen::CacheCaratulas;
use crate::ui::teclas::{Accion, traducir};

const DURACION_TOAST: Duration = Duration::from_secs(3);
const PAGINA_SALTOS: usize = 10;
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
}

impl Vista {
    pub const TODAS: [Vista; 6] = [
        Vista::Inicio,
        Vista::Buscar,
        Vista::Artistas,
        Vista::Albumes,
        Vista::Pistas,
        Vista::Playlists,
    ];

    pub fn numero(self) -> usize {
        match self {
            Vista::Inicio => 1,
            Vista::Buscar => 2,
            Vista::Artistas => 3,
            Vista::Albumes => 4,
            Vista::Pistas => 5,
            Vista::Playlists => 6,
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
        }
    }

    pub fn desde_numero(numero: usize) -> Option<Vista> {
        Vista::TODAS.get(numero.checked_sub(1)?).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pantalla {
    Lista,
    DetalleArtista,
    DetalleAlbum,
    DetallePlaylist,
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
    ultimo_clic: Option<(Instant, u16, u16)>,
    pendiente_g: bool,
}

impl AppEstado {
    pub fn nuevo(config: Config, paleta: Paleta, total_pistas: i64, ruta_tema: PathBuf) -> Self {
        let iconos = Iconos::desde(config.interfaz.iconos);
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
            ultimo_clic: None,
            pendiente_g: false,
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
                if self.dialogo.is_some() && self.tecla_en_dialogo(&tecla, ctx) {
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
            }
            AppEvento::Reproductor(estado) => {
                self.estado_reproductor = estado;
                if self.seleccion_cola >= self.estado_reproductor.cola.len() {
                    self.seleccion_cola = self.estado_reproductor.cola.len().saturating_sub(1);
                }
            }
            AppEvento::Escaneo(evento) => self.manejar_escaneo(evento, ctx),
            AppEvento::TemaActualizado(paleta) => {
                self.paleta = paleta;
            }
            AppEvento::Notificacion(nivel, texto) => self.notificar(nivel, texto),
            AppEvento::CaratulaLista(_album_id) => {}
            AppEvento::Scrobbling(estado) => self.estado_scrobbling = estado,
            AppEvento::Salir => {
                self.debe_salir = true;
            }
            AppEvento::Raton(evento) => self.manejar_raton(evento, ctx),
            AppEvento::Redimension(_, _) => {}
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
                if self.ayuda_visible {
                    self.ayuda_visible = false;
                } else {
                    self.volver();
                }
            }
            Accion::Ayuda => {
                self.ayuda_visible = !self.ayuda_visible;
            }
            Accion::IrA(vista) => {
                self.pila.clear();
                self.pantalla = Pantalla::Lista;
                self.seleccion = 0;
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
                    self.mover_seleccion(ctx, isize::MIN / 2);
                } else {
                    self.pendiente_g = true;
                }
            }
            Accion::EnfocarBuscar => {
                self.vista = Vista::Buscar;
                self.pantalla = Pantalla::Lista;
                self.busqueda_enfocada = true;
                self.foco = Foco::Contenido;
            }
            Accion::Abajo => self.mover_seleccion(ctx, 1),
            Accion::Arriba => self.mover_seleccion(ctx, -1),
            Accion::Derecha => match self.vista_pantalla_rejilla() {
                true => self.mover_seleccion(ctx, 1),
                false => self.abrir_seleccion(ctx),
            },
            Accion::Izquierda => match self.vista_pantalla_rejilla() {
                true => self.mover_seleccion(ctx, -1),
                false => self.volver(),
            },
            Accion::MediaAbajo => self.mover_seleccion(ctx, PAGINA_SALTOS as isize),
            Accion::MediaArriba => self.mover_seleccion(ctx, -(PAGINA_SALTOS as isize)),
            Accion::Primero => self.mover_seleccion(ctx, isize::MIN / 2),
            Accion::Ultimo => self.mover_seleccion(ctx, isize::MAX / 2),
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
                _ => {}
            },
            Accion::Abrir => {
                if self.foco == Foco::Cola {
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
                self.dialogo = Some(Dialogo::Texto {
                    titulo: "Nueva playlist".to_string(),
                    valor: String::new(),
                    accion: AccionDialogo::CrearPlaylist { anadir: Vec::new() },
                });
            }
            Accion::RenombrarPlaylist => {
                if let Some(playlist) = self.playlist_seleccionada() {
                    self.dialogo = Some(Dialogo::Texto {
                        titulo: "Renombrar playlist".to_string(),
                        valor: playlist.nombre.clone(),
                        accion: AccionDialogo::RenombrarPlaylist(playlist.id),
                    });
                }
            }
            Accion::EliminarPlaylist => {
                if let Some(playlist) = self.playlist_seleccionada() {
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
                if let Some(playlist) = self.playlist_seleccionada() {
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
                self.dialogo = Some(Dialogo::Texto {
                    titulo: "Importar M3U/M3U8 (ruta del fichero)".to_string(),
                    valor: String::new(),
                    accion: AccionDialogo::ImportarPlaylist,
                });
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
            MouseEventKind::ScrollDown => self.mover_seleccion(ctx, 3),
            MouseEventKind::ScrollUp => self.mover_seleccion(ctx, -3),
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
            _ => 0,
        }
    }

    fn mover_seleccion(&mut self, ctx: &ContextoApp<'_>, delta: isize) {
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
        let paso = if delta.abs() == 1 && self.vista_pantalla_rejilla() {
            self.columnas_rejilla.max(1) as isize
        } else {
            delta
        };
        self.seleccion = mover_indice(self.seleccion, total - 1, paso);
        if self.vista == Vista::Pistas {
            self.asegurar_pagina(ctx);
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
                ctx.reproductor.enviar(ComandoReproductor::ReemplazarCola {
                    pistas: ids,
                    indice,
                });
            }
            Contexto::Album { album_id } => self.abrir_album(ctx, album_id),
            Contexto::Artista { artista_id } => self.abrir_artista(ctx, artista_id),
            Contexto::Ninguno => {}
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
            _ => Contexto::Ninguno,
        }
    }

    fn anadir_seleccion(&mut self, ctx: &ContextoApp<'_>, a_continuacion: bool) {
        let pistas = match self.contexto() {
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
            Contexto::Ninguno => Vec::new(),
        };
        if pistas.is_empty() {
            return;
        }
        let comando = if a_continuacion {
            ComandoReproductor::ReproducirSiguiente { pistas }
        } else {
            ComandoReproductor::AnadirAlFinal { pistas }
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
        self.estado_reproductor
            .pista_actual
            .as_ref()
            .map(|pista| pista.id)
    }

    fn pista_bajo_cursor(&self) -> Option<i64> {
        if self.foco == Foco::Cola {
            return self
                .estado_reproductor
                .cola
                .get(self.seleccion_cola)
                .map(|pista| pista.id)
                .or_else(|| self.pista_actual_id());
        }
        let seleccionada = match self.contexto() {
            Contexto::Pistas { ids, indice } => ids.get(indice).copied(),
            _ => None,
        };
        seleccionada.or_else(|| self.pista_actual_id())
    }

    fn alternar_favorita(&mut self, ctx: &ContextoApp<'_>) {
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
            Vista::Buscar => {}
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
            && self.estado_reproductor.pista_actual.is_none()
    }
}

fn en_rect(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom()
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
