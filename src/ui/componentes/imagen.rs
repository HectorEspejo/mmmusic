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
use ratatui_image::{FontSize, Resize, StatefulImage};
use tracing::warn;

use crate::app::AppEstado;
use crate::visuales::paleta::{a_hex, colores_dominantes};

pub const CAPACIDAD_CACHE: usize = 200;
pub const FUENTE_POR_DEFECTO: FontSize = FontSize {
    width: 10,
    height: 20,
};
const LADO_MAXIMO_DECODIFICADO: u32 = 768;

pub enum PeticionCaratula {
    Cargar { album_id: i64, ruta: PathBuf },
    Colores { album_id: i64, ruta: PathBuf },
}

pub enum RespuestaCaratula {
    Imagen(i64, DynamicImage),
    Colores(i64, String),
}

pub struct CacheCaratulas {
    picker: Option<Picker>,
    entradas: LruCache<i64, StatefulProtocol>,
    pendientes: HashSet<i64>,
    pendientes_colores: HashSet<i64>,
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
            pendientes_colores: HashSet::new(),
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

    pub fn fuente(&self) -> Option<FontSize> {
        self.picker.as_ref().map(Picker::font_size)
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

    /// Pide los colores dominantes de una carátula cacheada; el hilo auxiliar
    /// los devuelve por el canal de respuestas.
    pub fn solicitar_colores(&mut self, album_id: i64, ruta: &str) {
        if self.pendientes_colores.contains(&album_id) {
            return;
        }
        let Some(tx) = self.tx.as_ref() else {
            return;
        };
        let ruta = PathBuf::from(ruta);
        if !ruta.exists() {
            return;
        }
        self.pendientes_colores.insert(album_id);
        if tx
            .send(PeticionCaratula::Colores { album_id, ruta })
            .is_err()
        {
            self.pendientes_colores.remove(&album_id);
        }
    }
}

pub fn lanzar_worker(tx_resultados: Sender<RespuestaCaratula>) -> Sender<PeticionCaratula> {
    let (tx_peticiones, rx_peticiones) = mpsc::channel();
    thread::Builder::new()
        .name("caratulas".to_string())
        .spawn(move || {
            while let Ok(peticion) = rx_peticiones.recv() {
                match peticion {
                    PeticionCaratula::Cargar { album_id, ruta } => match image::open(&ruta) {
                        Ok(imagen) => {
                            let redimensionada = imagen
                                .thumbnail(LADO_MAXIMO_DECODIFICADO, LADO_MAXIMO_DECODIFICADO);
                            if tx_resultados
                                .send(RespuestaCaratula::Imagen(album_id, redimensionada))
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(error) => {
                            warn!(ruta = %ruta.display(), "carátula ilegible: {error}");
                        }
                    },
                    PeticionCaratula::Colores { album_id, ruta } => match image::open(&ruta) {
                        Ok(imagen) => {
                            if let Some(colores) = colores_dominantes(&imagen) {
                                let texto = colores
                                    .iter()
                                    .map(|color| a_hex(*color))
                                    .collect::<Vec<_>>()
                                    .join(",");
                                if tx_resultados
                                    .send(RespuestaCaratula::Colores(album_id, texto))
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        }
                        Err(error) => {
                            warn!(ruta = %ruta.display(), "carátula ilegible: {error}");
                        }
                    },
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
    if !app.config.interfaz.caratulas {
        return;
    }
    dibujar_clave(frame, app, area, album_id, caratula_ruta);
}

/// Clave de caché para el logo de una emisora: ids negativos para no
/// colisionar con los identificadores de álbum.
pub fn clave_logo(emisora_id: i64) -> i64 {
    -emisora_id
}

pub fn dibujar_logo(
    frame: &mut Frame,
    app: &mut AppEstado,
    area: Rect,
    emisora_id: i64,
    logo_ruta: Option<&str>,
) {
    if !app.config.radio.logos {
        return;
    }
    dibujar_clave(frame, app, area, clave_logo(emisora_id), logo_ruta);
}

fn dibujar_clave(
    frame: &mut Frame,
    app: &mut AppEstado,
    area: Rect,
    clave: i64,
    caratula_ruta: Option<&str>,
) {
    if area.width < 2 || area.height < 1 {
        return;
    }
    if app.caratulas.tiene(clave) {
        if let Some(protocolo) = app.caratulas.entradas.get_mut(&clave) {
            let destino = destino_ajustado(protocolo, area);
            let widget = StatefulImage::default().resize(Resize::Fit(None));
            frame.render_stateful_widget(widget, destino, protocolo);
            return;
        }
    } else if let Some(ruta) = caratula_ruta {
        app.caratulas.solicitar(clave, ruta);
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

/// Alto en celdas que ocupará una carátula cuadrada (la caché siempre lo es)
/// escalada al ancho indicado, redondeando al alza para no recortarla.
pub fn alto_caratula_ajustada(ancho_celdas: u16, fuente: FontSize) -> u16 {
    let ancho_px = u32::from(ancho_celdas.max(1)) * u32::from(fuente.width.max(1));
    let alto = ancho_px.div_ceil(u32::from(fuente.height.max(1)));
    alto.min(u32::from(u16::MAX)) as u16
}

/// Calcula el rectángulo donde cabe la carátula escalada y lo centra en el
/// área disponible, para que no quede recortada ni pegada a una esquina.
fn destino_ajustado(protocolo: &StatefulProtocol, area: Rect) -> Rect {
    let tamano = protocolo.size_for(Resize::Fit(None), area.as_size());
    let ancho = tamano.width.min(area.width);
    let alto = tamano.height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(ancho) / 2,
        y: area.y + area.height.saturating_sub(alto) / 2,
        width: ancho,
        height: alto,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use ratatui::layout::Size;

    #[test]
    fn la_caratula_se_escala_y_centra_en_el_area() {
        let picker = Picker::halfblocks();
        let imagen = DynamicImage::new_rgb8(300, 300);
        let protocolo = picker.new_resize_protocol(imagen);
        let area = Rect::new(2, 3, 16, 2);
        assert_eq!(
            protocolo.size_for(Resize::Fit(None), area.as_size()),
            Size::new(4, 2),
            "una carátula de 300×300 debe encogerse a 4×2 celdas"
        );
        assert_eq!(
            destino_ajustado(&protocolo, area),
            Rect::new(8, 3, 4, 2),
            "el recorte escalado debe quedar centrado en el área"
        );
    }

    #[test]
    fn el_area_grande_no_agranda_la_caratula() {
        let picker = Picker::halfblocks();
        let imagen = DynamicImage::new_rgb8(300, 300);
        let protocolo = picker.new_resize_protocol(imagen);
        let area = Rect::new(0, 0, 80, 24);
        let destino = destino_ajustado(&protocolo, area);
        assert_eq!(destino.width, 30, "no se amplía más allá de su tamaño");
        assert!(destino.right() <= area.right() && destino.bottom() <= area.bottom());
    }
}
