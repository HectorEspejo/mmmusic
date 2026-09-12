use std::io::{self, Write};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::biblioteca::escaner::{self, ModoEscaneo};
use crate::biblioteca::{bd, consultas};
use crate::config::{Config, Rutas};
use crate::credenciales;
use crate::eventos::{AppEvento, EventoEscaneo};
use crate::scrobbling::SERVICIO_LISTENBRAINZ;
use crate::scrobbling::listenbrainz::ClienteListenBrainz;

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
    /// Comprobar las credenciales de los servicios de scrobbling
    ProbarServicios,
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

pub fn ejecutar_probar_servicios() -> Result<u8> {
    let rutas = Rutas::detectar()?;
    rutas.crear_directorios()?;
    let carga = credenciales::cargar(&rutas.credenciales)?;
    if carga.permisos_corregidos {
        println!(
            "Aviso: las credenciales eran legibles por otros usuarios; permisos corregidos a 600"
        );
    }
    let cred = carga.credenciales;
    let mut conn = bd::abrir(&rutas.base_datos)?;
    bd::migrar(&mut conn)?;

    let mut configurados = 0usize;
    let mut error_red = false;
    let mut error_configuracion = false;

    if let Some(token) = cred.token_listenbrainz() {
        configurados += 1;
        let cliente = ClienteListenBrainz::nuevo(Some(token.to_string()));
        match cliente.validar_token() {
            Ok(usuario) => {
                println!("ListenBrainz: OK ({usuario})");
                consultas::envios::reprogramar_errores_auth(&conn, SERVICIO_LISTENBRAINZ)?;
                consultas::envios::limpiar_errores_auth(&conn, SERVICIO_LISTENBRAINZ)?;
            }
            Err(fallo) => {
                println!("ListenBrainz: error — {}", cred.redactar(&fallo.mensaje));
                if fallo.status == 0 {
                    error_red = true;
                } else {
                    error_configuracion = true;
                }
            }
        }
    } else {
        println!("ListenBrainz: no configurado");
    }

    println!("Last.fm: no configurado");

    if configurados == 0 {
        return Ok(1);
    }
    if error_red {
        return Ok(2);
    }
    if error_configuracion {
        return Ok(1);
    }
    Ok(0)
}
