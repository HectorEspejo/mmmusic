//! Fuente de letras desde fichero: `<pista>.lrc`/`.txt` junto al audio y
//! `letras.carpeta/<artista> - <titulo>.lrc|.txt` con nombres normalizados.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use tracing::debug;

use super::{Fuente, FuenteLetras, TextoLetra, decodificar, limpiar_control, lrc};
use crate::biblioteca::etiquetas;
use crate::biblioteca::modelos::PistaResumen;

pub struct FuenteFichero {
    carpeta: PathBuf,
}

struct Candidato {
    ruta: PathBuf,
    aviso_truncado: bool,
}

impl FuenteFichero {
    pub fn nueva(carpeta: PathBuf) -> Self {
        Self { carpeta }
    }

    /// Rutas exactas que se buscan, en orden, para el mensaje de "sin letra".
    pub fn rutas_buscadas(&self, pista: &PistaResumen) -> Vec<String> {
        self.candidatos(pista)
            .iter()
            .map(|candidato| candidato.ruta.display().to_string())
            .collect()
    }

    fn candidatos(&self, pista: &PistaResumen) -> Vec<Candidato> {
        let mut candidatos = Vec::new();
        let ruta = Path::new(&pista.ruta);
        if let (Some(carpeta), Some(nombre)) = (ruta.parent(), ruta.file_stem()) {
            let base = carpeta.join(nombre);
            candidatos.push(Candidato {
                ruta: base.with_extension("lrc"),
                aviso_truncado: false,
            });
            candidatos.push(Candidato {
                ruta: base.with_extension("txt"),
                aviso_truncado: true,
            });
        }
        let nombre = format!(
            "{} - {}",
            etiquetas::normalizar(&pista.artista),
            etiquetas::normalizar(&pista.titulo)
        );
        candidatos.push(Candidato {
            ruta: self.carpeta.join(format!("{nombre}.lrc")),
            aviso_truncado: false,
        });
        candidatos.push(Candidato {
            ruta: self.carpeta.join(format!("{nombre}.txt")),
            aviso_truncado: true,
        });
        candidatos
    }
}

impl FuenteLetras for FuenteFichero {
    fn nombre(&self) -> Fuente {
        Fuente::Fichero
    }

    fn buscar(&self, pista: &PistaResumen) -> Option<TextoLetra> {
        for candidato in self.candidatos(pista) {
            if let Some(letra) = leer_candidato(&candidato) {
                return Some(letra);
            }
        }
        None
    }
}

fn leer_candidato(candidato: &Candidato) -> Option<TextoLetra> {
    let metadata = fs::metadata(&candidato.ruta).ok()?;
    if metadata.len() == 0 {
        return None;
    }
    let (bytes, aviso) = if candidato.aviso_truncado {
        if metadata.len() > lrc::MAX_TAMANO_TXT {
            let fichero = fs::File::open(&candidato.ruta).ok()?;
            let mut buffer = Vec::with_capacity(lrc::MAX_TAMANO_TXT as usize);
            fichero
                .take(lrc::MAX_TAMANO_TXT)
                .read_to_end(&mut buffer)
                .ok()?;
            (
                buffer,
                Some(format!(
                    "Letra .txt truncada a {} KB",
                    lrc::MAX_TAMANO_TXT / 1024
                )),
            )
        } else {
            (fs::read(&candidato.ruta).ok()?, None)
        }
    } else {
        if metadata.len() > lrc::MAX_TAMANO_LRC {
            debug!(
                ruta = %candidato.ruta.display(),
                "se ignora un .lrc mayor de {} KB",
                lrc::MAX_TAMANO_LRC / 1024
            );
            return None;
        }
        (fs::read(&candidato.ruta).ok()?, None)
    };
    let texto = limpiar_control(&decodificar(&bytes));
    if texto.trim().is_empty() {
        return None;
    }
    Some(TextoLetra {
        texto,
        fuente: Fuente::Fichero,
        aviso,
    })
}
