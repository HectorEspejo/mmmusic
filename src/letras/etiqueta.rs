//! Fuente de letras desde las etiquetas embebidas, leídas bajo demanda con
//! `lofty`: `UnsynchronizedText` (ID3 USLT) y `LYRICS` (Vorbis/MP4).

use std::path::Path;

use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use tracing::debug;

use super::{Fuente, FuenteLetras, TextoLetra, limpiar_control};
use crate::biblioteca::modelos::PistaResumen;

pub struct FuenteEtiqueta;

impl FuenteLetras for FuenteEtiqueta {
    fn nombre(&self) -> Fuente {
        Fuente::Etiqueta
    }

    fn buscar(&self, pista: &PistaResumen) -> Option<TextoLetra> {
        let tagged = match Probe::open(Path::new(&pista.ruta)).and_then(|probe| probe.read()) {
            Ok(tagged) => tagged,
            Err(error) => {
                debug!(ruta = %pista.ruta, "no se pudieron leer las etiquetas: {error}");
                return None;
            }
        };
        let etiqueta = tagged.primary_tag().or_else(|| tagged.first_tag())?;
        let texto = etiqueta
            .get_string(ItemKey::Lyrics)
            .or_else(|| etiqueta.get_string(ItemKey::UnsyncLyrics))?;
        if texto.trim().is_empty() {
            return None;
        }
        Some(TextoLetra {
            texto: limpiar_control(texto),
            fuente: Fuente::Etiqueta,
            aviso: None,
        })
    }
}
