use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread;

use image::DynamicImage;
use lru::LruCache;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use tracing::warn;

use crate::app::AppEstado;

pub const CAPACIDAD_CACHE: usize = 200;
const LADO_MAXIMO_DECODIFICADO: u32 = 768;

pub enum PeticionCaratula {
    Cargar { album_id: i64, ruta: PathBuf },
}

pub struct CacheCaratulas {
    picker: Option<Picker>,
    entradas: LruCache<i64, StatefulProtocol>,
    pendientes: HashSet<i64>,
    tx: Option<Sender<PeticionCaratula>>,
}

impl Default for CacheCaratulas {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl CacheCaratulas {
    pub fn nuevo() -> Self {
        Self {
            picker: None,
            entradas: LruCache::new(
                NonZeroUsize::new(CAPACIDAD_CACHE).unwrap_or(NonZeroUsize::MIN),
            ),
            pendientes: HashSet::new(),
            tx: None,
        }
    }

    pub fn activar(&mut self, picker: Picker, tx: Sender<PeticionCaratula>) {
        self.picker = Some(picker);
        self.tx = Some(tx);
    }

    pub fn tiene(&self, album_id: i64) -> bool {
        self.entradas.contains(&album_id)
    }

    pub fn insertar(&mut self, album_id: i64, imagen: DynamicImage) {
        self.pendientes.remove(&album_id);
        if let Some(picker) = self.picker.as_ref() {
            let protocolo = picker.new_resize_protocol(imagen);
            self.entradas.put(album_id, protocolo);
        }
    }

    pub fn solicitar(&mut self, album_id: i64, ruta: &str) {
        if self.entradas.contains(&album_id) || self.pendientes.contains(&album_id) {
            return;
        }
        let Some(tx) = self.tx.as_ref() else {
            return;
        };
        let ruta = PathBuf::from(ruta);
        if !ruta.exists() {
            return;
        }
        self.pendientes.insert(album_id);
        if tx
            .send(PeticionCaratula::Cargar { album_id, ruta })
            .is_err()
        {
            self.pendientes.remove(&album_id);
        }
    }
}

pub fn lanzar_worker(tx_resultados: Sender<(i64, DynamicImage)>) -> Sender<PeticionCaratula> {
    let (tx_peticiones, rx_peticiones) = mpsc::channel();
    thread::Builder::new()
        .name("caratulas".to_string())
        .spawn(move || {
            while let Ok(PeticionCaratula::Cargar { album_id, ruta }) = rx_peticiones.recv() {
                match image::open(&ruta) {
                    Ok(imagen) => {
                        let redimensionada =
                            imagen.thumbnail(LADO_MAXIMO_DECODIFICADO, LADO_MAXIMO_DECODIFICADO);
                        if tx_resultados.send((album_id, redimensionada)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        warn!(ruta = %ruta.display(), "carátula ilegible: {error}");
                    }
                }
            }
        })
        .expect("no se pudo lanzar el hilo de carátulas");
    tx_peticiones
}

pub fn dibujar(
    frame: &mut Frame,
    app: &mut AppEstado,
    area: Rect,
    album_id: i64,
    caratula_ruta: Option<&str>,
) {
    if !app.config.interfaz.caratulas || area.width < 2 || area.height < 1 {
        return;
    }
    if app.caratulas.tiene(album_id) {
        if let Some(protocolo) = app.caratulas.entradas.get_mut(&album_id) {
            let widget = StatefulImage::default().resize(Resize::Crop(None));
            frame.render_stateful_widget(widget, area, protocolo);
            return;
        }
    } else if let Some(ruta) = caratula_ruta {
        app.caratulas.solicitar(album_id, ruta);
    }
    frame.render_widget(
        Paragraph::new(Span::styled(
            app.iconos.nota,
            Style::new().fg(app.paleta.secundario),
        ))
        .alignment(Alignment::Center)
        .style(Style::new().bg(app.paleta.fondo)),
        area,
    );
}
