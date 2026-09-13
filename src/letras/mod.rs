//! Letras locales: trait de fuentes, resolución por prioridad, caché (en la
//! UI) y worker de E/S. La vista no conoce las fuentes; solo `Letra`.

pub mod etiqueta;
pub mod fichero;
pub mod lrc;
pub mod sincronia;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result};

use crate::biblioteca::modelos::PistaResumen;
use crate::eventos::AppEvento;

pub use etiqueta::FuenteEtiqueta;
pub use fichero::FuenteFichero;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fuente {
    Fichero,
    Etiqueta,
}

impl Fuente {
    pub fn como_str(self) -> &'static str {
        match self {
            Fuente::Fichero => "fichero",
            Fuente::Etiqueta => "etiqueta",
        }
    }

    pub fn desde_str(texto: &str) -> Option<Self> {
        match texto {
            "fichero" => Some(Fuente::Fichero),
            "etiqueta" => Some(Fuente::Etiqueta),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextoLetra {
    pub texto: String,
    pub fuente: Fuente,
    /// Aviso a mostrar en la UI (por ejemplo, un `.txt` truncado).
    pub aviso: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Letra {
    Ninguna {
        rutas: Vec<String>,
    },
    Estatica {
        lineas: Vec<String>,
        fuente: Fuente,
    },
    Sincronizada {
        lineas: Vec<(u32, String)>,
        offset_lrc_ms: i64,
        fuente: Fuente,
    },
}

impl Letra {
    pub fn desde_texto(texto: &TextoLetra) -> Self {
        let limpio = limpiar_control(&texto.texto);
        if lrc::es_sincronizada(&limpio) {
            let lrc::Lrc { lineas, offset_ms } = lrc::parsear(&limpio);
            if !lineas.is_empty() {
                return Letra::Sincronizada {
                    lineas,
                    offset_lrc_ms: offset_ms,
                    fuente: texto.fuente,
                };
            }
        }
        let lineas: Vec<String> = limpio
            .lines()
            .map(str::trim_end)
            .map(str::to_string)
            .collect();
        Letra::Estatica {
            lineas,
            fuente: texto.fuente,
        }
    }

    pub fn lineas(&self) -> usize {
        match self {
            Letra::Ninguna { .. } => 0,
            Letra::Estatica { lineas, .. } => lineas.len(),
            Letra::Sincronizada { lineas, .. } => lineas.len(),
        }
    }

    pub fn fuente(&self) -> Option<Fuente> {
        match self {
            Letra::Ninguna { .. } => None,
            Letra::Estatica { fuente, .. } | Letra::Sincronizada { fuente, .. } => Some(*fuente),
        }
    }

    pub fn es_sincronizada(&self) -> bool {
        matches!(self, Letra::Sincronizada { .. })
    }
}

/// Resultado de resolver todas las fuentes disponibles para una pista.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resolucion {
    pub fichero: Option<TextoLetra>,
    pub etiqueta: Option<TextoLetra>,
    pub rutas: Vec<String>,
}

impl Resolucion {
    /// Fuentes con contenido, en orden de prioridad.
    pub fn fuentes(&self) -> Vec<Fuente> {
        let mut fuentes = Vec::new();
        if self.fichero.is_some() {
            fuentes.push(Fuente::Fichero);
        }
        if self.etiqueta.is_some() {
            fuentes.push(Fuente::Etiqueta);
        }
        fuentes
    }

    /// Elige la letra: la fuente preferida si existe; si no, fichero y luego
    /// etiqueta.
    pub fn elegir(&self, preferida: Option<Fuente>) -> Letra {
        let elegida = match preferida {
            Some(Fuente::Fichero) if self.fichero.is_some() => self.fichero.as_ref(),
            Some(Fuente::Etiqueta) if self.etiqueta.is_some() => self.etiqueta.as_ref(),
            _ => self.fichero.as_ref().or(self.etiqueta.as_ref()),
        };
        match elegida {
            Some(texto) => Letra::desde_texto(texto),
            None => Letra::Ninguna {
                rutas: self.rutas.clone(),
            },
        }
    }
}

pub trait FuenteLetras: Send {
    fn nombre(&self) -> Fuente;
    fn buscar(&self, pista: &PistaResumen) -> Option<TextoLetra>;
}

/// Resuelve todas las fuentes (fichero y etiqueta) para una pista. Sin caché;
/// el llamador decide qué hacer con el resultado.
pub fn resolver(pista: &PistaResumen, carpeta: &Path) -> Resolucion {
    let fuente_fichero = FuenteFichero::nueva(carpeta.to_path_buf());
    let rutas = fuente_fichero.rutas_buscadas(pista);
    let fichero = fuente_fichero.buscar(pista);
    let etiqueta = FuenteEtiqueta.buscar(pista);
    Resolucion {
        fichero,
        etiqueta,
        rutas,
    }
}

/// Decodifica UTF-8 con respaldo Latin-1 (byte a byte).
pub fn decodificar(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(texto) => texto.to_string(),
        Err(_) => bytes.iter().map(|byte| char::from(*byte)).collect(),
    }
}

/// Elimina caracteres de control salvo el salto de línea (separador de
/// estrofas).
pub fn limpiar_control(texto: &str) -> String {
    texto
        .chars()
        .filter(|caracter| !caracter.is_control() || *caracter == '\n')
        .collect()
}

#[derive(Debug, Clone)]
pub enum PeticionLetras {
    Resolver {
        pista: Box<PistaResumen>,
        carpeta: PathBuf,
    },
}

pub struct ManejoLetras {
    tx: Sender<PeticionLetras>,
    hilo: Option<JoinHandle<()>>,
}

impl ManejoLetras {
    pub fn enviar(&self, peticion: PeticionLetras) -> bool {
        self.tx.send(peticion).is_ok()
    }

    pub fn apagar(mut self) {
        drop(self.tx);
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

/// Worker de letras: resuelve ficheros y etiquetas fuera del hilo de UI y
/// responde con `AppEvento::LetrasListas`.
pub fn lanzar(tx_app: Sender<AppEvento>) -> Result<ManejoLetras> {
    let (tx, rx): (Sender<PeticionLetras>, Receiver<PeticionLetras>) = mpsc::channel();
    let hilo = thread::Builder::new()
        .name("letras".to_string())
        .spawn(move || {
            while let Ok(PeticionLetras::Resolver { pista, carpeta }) = rx.recv() {
                let pista_id = pista.id;
                let resolucion = resolver(&pista, &carpeta);
                let _ = tx_app.send(AppEvento::LetrasListas {
                    pista_id,
                    resolucion: Box::new(resolucion),
                });
            }
        })
        .context("no se pudo lanzar el hilo de letras")?;
    Ok(ManejoLetras {
        tx,
        hilo: Some(hilo),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn decodifica_latin1_de_respaldo() {
        assert_eq!(decodificar(b"caf\xc3\xa9"), "café");
        assert_eq!(decodificar(b"caf\xe9"), "café");
    }

    #[test]
    fn limpia_controles_sin_tocar_saltos() {
        assert_eq!(limpiar_control("a\u{0}b\u{7}c\nd"), "abc\nd");
    }
}
