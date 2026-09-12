use std::io::{self, Write};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::biblioteca::escaner::{self, ModoEscaneo};
use crate::config::{Config, Rutas};
use crate::eventos::{AppEvento, EventoEscaneo};

#[derive(Debug, Parser)]
#[command(
    name = "mmmusic",
    version,
    about = "Reproductor de música para terminal",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub comando: Option<Comando>,
}

#[derive(Debug, Subcommand)]
pub enum Comando {
    /// Reescanear la biblioteca sin abrir la TUI
    Reescanear {
        /// Ignora mtime y tamaño y relee todas las etiquetas
        #[arg(long)]
        completo: bool,
    },
}

pub fn ejecutar_reescanear(completo: bool) -> Result<u8> {
    let rutas = Rutas::detectar()?;
    rutas.crear_directorios()?;
    let carga = Config::cargar(&rutas.config)?;
    let raices = carga.config.carpetas_expandidas();
    let modo = if completo {
        ModoEscaneo::Completo
    } else {
        ModoEscaneo::Incremental
    };
    let (tx, rx) = mpsc::channel();
    let manejo = escaner::lanzar(
        rutas.base_datos.clone(),
        raices,
        rutas.cache_caratulas.clone(),
        modo,
        tx,
    )?;
    let mut codigo = 0u8;
    loop {
        match rx.recv() {
            Ok(AppEvento::Escaneo(EventoEscaneo::Iniciado { total })) => {
                println!("Escaneando {total} ficheros…");
            }
            Ok(AppEvento::Escaneo(EventoEscaneo::Progreso { procesadas, total })) => {
                print!("\r  {procesadas}/{total} ficheros");
                let _ = io::stdout().flush();
            }
            Ok(AppEvento::Escaneo(EventoEscaneo::Terminado { resumen })) => {
                println!("\r{}", resumen.mensaje());
                if resumen.omitidas > 0 {
                    println!("  {} omitidas", resumen.omitidas);
                }
                break;
            }
            Ok(AppEvento::Escaneo(EventoEscaneo::Cancelado { resumen })) => {
                println!("\r{}", resumen.mensaje());
                codigo = 1;
                break;
            }
            Ok(AppEvento::Escaneo(EventoEscaneo::Error { mensaje })) => {
                eprintln!("\rmmmusic: error de escaneo: {mensaje}");
                codigo = 1;
                break;
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = manejo.esperar(Duration::from_secs(10));
    Ok(codigo)
}
