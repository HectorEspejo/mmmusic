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
use crate::scrobbling::lastfm::{self, ClienteLastfm};
use crate::scrobbling::listenbrainz::ClienteListenBrainz;
use crate::scrobbling::{SERVICIO_LASTFM, SERVICIO_LISTENBRAINZ};

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
    /// Autorizar Last.fm y guardar la sesión
    AutorizarLastfm,
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

    if cred.api_key_lastfm().is_some() && !cred.lastfm_autorizado() {
        println!("Last.fm: error — sesión sin autorizar (ejecuta mmmusic autorizar-lastfm)");
        error_configuracion = true;
    } else if cred.lastfm_autorizado() {
        configurados += 1;
        let cliente = ClienteLastfm::nuevo(&cred);
        match cliente.validar_sesion() {
            Ok(usuario) => {
                println!("Last.fm: OK ({usuario})");
                consultas::envios::reprogramar_errores_auth(&conn, SERVICIO_LASTFM)?;
                consultas::envios::limpiar_errores_auth(&conn, SERVICIO_LASTFM)?;
            }
            Err(fallo) => {
                println!("Last.fm: error — {}", cred.redactar(&fallo.mensaje));
                if fallo.status == 0 {
                    error_red = true;
                } else {
                    error_configuracion = true;
                }
            }
        }
    } else {
        println!("Last.fm: no configurado");
    }

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

pub fn ejecutar_autorizar_lastfm() -> Result<u8> {
    let rutas = Rutas::detectar()?;
    rutas.crear_directorios()?;
    let mut carga = credenciales::cargar(&rutas.credenciales)?;
    let Some(api_key) = carga.credenciales.api_key_lastfm().map(str::to_string) else {
        eprintln!(
            "Rellena api_key y api_secret en {}",
            rutas.credenciales.display()
        );
        return Ok(1);
    };
    let Some(api_secret) = carga.credenciales.api_secret_lastfm().map(str::to_string) else {
        eprintln!(
            "Rellena api_key y api_secret en {}",
            rutas.credenciales.display()
        );
        return Ok(1);
    };
    let agente = crate::scrobbling::agente_http();
    let token = match lastfm::obtener_token(&agente, &api_key, &api_secret) {
        Ok(token) => token,
        Err(fallo) => {
            eprintln!(
                "No se pudo obtener el token de Last.fm: {}",
                carga.credenciales.redactar(&fallo.mensaje)
            );
            return Ok(if fallo.status == 0 { 2 } else { 1 });
        }
    };
    let url = format!("https://www.last.fm/api/auth/?api_key={api_key}&token={token}");
    println!("Abre esta URL y autoriza mmmusic en Last.fm:\n  {url}");
    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    for intento in 1..=3 {
        println!("Pulsa Enter cuando hayas autorizado…");
        let mut linea = String::new();
        io::stdin().read_line(&mut linea)?;
        match lastfm::obtener_sesion(&agente, &api_key, &api_secret, &token) {
            Ok(sesion) => {
                carga.credenciales.lastfm.session_key = Some(sesion.session_key);
                carga.credenciales.lastfm.usuario = Some(sesion.usuario.clone());
                credenciales::guardar(&rutas.credenciales, &carga.credenciales)?;
                println!(
                    "Last.fm autorizado como {}. Sesión guardada en {}",
                    sesion.usuario,
                    rutas.credenciales.display()
                );
                return Ok(0);
            }
            Err(fallo) if fallo.codigo_servicio == Some(14) => {
                println!("Aún no autorizado (intento {intento}/3); vuelve a pulsar Enter.");
            }
            Err(fallo) => {
                eprintln!(
                    "No se pudo obtener la sesión: {}",
                    carga.credenciales.redactar(&fallo.mensaje)
                );
                return Ok(if fallo.status == 0 { 2 } else { 1 });
            }
        }
    }
    eprintln!("No se pudo autorizar Last.fm tras 3 intentos");
    Ok(1)
}
