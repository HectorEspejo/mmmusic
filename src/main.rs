use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{self, Event as EventoCrossterm};
use mmmusic::app::{AppEstado, ContextoApp, Vista};
use mmmusic::audio::{self, Anillo};
use mmmusic::biblioteca::escaner::ModoEscaneo;
use mmmusic::biblioteca::{bd, caratulas, consultas};
use mmmusic::cli::{self, Cli, Comando};
use mmmusic::config::{ActivoAlArrancar, Config, Rutas};
use mmmusic::credenciales;
use mmmusic::eventos::{AppEvento, NivelAviso};
use mmmusic::letras;
use mmmusic::mpris;
use mmmusic::reproductor::{self, ManejoReproductor};
use mmmusic::scrobbling;
use mmmusic::tema;
use mmmusic::tema::VigilanteTema;
use mmmusic::ui;
use mmmusic::ui::componentes::imagen::{self, RespuestaCaratula};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use rusqlite::Connection;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let resultado: Result<u8> = match cli.comando {
        Some(Comando::Reescanear { completo }) => cli::ejecutar_reescanear(completo),
        Some(Comando::ProbarServicios) => cli::ejecutar_probar_servicios(),
        Some(Comando::AutorizarLastfm) => cli::ejecutar_autorizar_lastfm(),
        None => ejecutar_tui().map(|()| 0),
    };
    match resultado {
        Ok(codigo) => ExitCode::from(codigo),
        Err(error) => {
            eprintln!("mmmusic: {error:#}");
            ExitCode::from(1)
        }
    }
}

fn ejecutar_tui() -> Result<()> {
    let rutas = Rutas::detectar()?;
    rutas.crear_directorios()?;
    let _guardia_logs = iniciar_logs(&rutas)?;
    let carga = Config::cargar(&rutas.config)?;
    bd::limpiar_copia_previa_si_migrada(&rutas.base_datos)?;
    let conn = bd::abrir_y_migrar(&rutas.base_datos)?;
    let replaygain_preamp = format!("{:.1}", carga.config.ecualizador.replaygain_preamp_db);
    consultas::ajustes::sembrar(
        &conn,
        &[
            ("radio_pestana", "favoritas"),
            ("radiobrowser_servidor", ""),
            (
                "eq_activo",
                match carga.config.ecualizador.activo_al_arrancar {
                    ActivoAlArrancar::Si => "1",
                    ActivoAlArrancar::Recordar | ActivoAlArrancar::No => "0",
                },
            ),
            ("eq_preset_id", ""),
            ("eq_ganancias", "0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0"),
            ("eq_preamp_db", "0.0"),
            (
                "eq_limitador",
                if carga.config.ecualizador.limitador {
                    "1"
                } else {
                    "0"
                },
            ),
            (
                "replaygain_modo",
                carga.config.ecualizador.replaygain.como_str(),
            ),
            ("replaygain_preamp_db", &replaygain_preamp),
            (
                "letras_superpuestas",
                if carga.config.letras.superpuestas {
                    "1"
                } else {
                    "0"
                },
            ),
        ],
    )?;
    consultas::presets_eq::sembrar_integrados(&conn)?;
    if consultas::escaneos::marcar_huerfanos(&conn)? > 0 {
        tracing::warn!("se marcaron escaneos interrumpidos como error");
    }
    let reescaneo_pendiente = consultas::ajustes::reescaneo_completo_pendiente(&conn)?;
    let (paleta, aviso_tema) = tema::cargar(&carga.config.tema, &rutas.fichero_tema);
    let total_pistas = consultas::contar_pistas(&conn)?;
    let (tx_app, rx_app) = mpsc::channel();
    let mut app = AppEstado::nuevo(
        carga.config.clone(),
        paleta,
        total_pistas,
        rutas.fichero_tema.clone(),
    );
    if let Err(error) = app.aplicar_ajustes_visuales(&conn) {
        app.notificar(
            NivelAviso::Aviso,
            format!("No se pudieron leer los ajustes de visuales: {error:#}"),
        );
    }
    if let Err(error) = app.aplicar_ajustes_radio(&conn) {
        app.notificar(
            NivelAviso::Aviso,
            format!("No se pudieron leer los ajustes de radio: {error:#}"),
        );
    }
    if let Err(error) = app.aplicar_ajustes_letras(&conn) {
        app.notificar(
            NivelAviso::Aviso,
            format!("No se pudieron leer los ajustes de letras: {error:#}"),
        );
    }
    if let Some(aviso) = aviso_tema {
        app.notificar(NivelAviso::Aviso, aviso);
    }
    if carga.creada {
        app.notificar(
            NivelAviso::Info,
            format!("Configuración creada en {}", carga.ruta.display()),
        );
    }

    let _vigilante: Option<VigilanteTema> = if rutas.dir_tema.exists() {
        match tema::vigilar(
            rutas.dir_tema.clone(),
            carga.config.tema.clone(),
            tx_app.clone(),
        ) {
            Ok(vigilante) => Some(vigilante),
            Err(error) => {
                tracing::warn!("no se pudo vigilar el tema: {error:#}");
                None
            }
        }
    } else {
        None
    };
    let manejo_colores =
        match caratulas::lanzar_colores_pendientes(rutas.base_datos.clone(), tx_app.clone()) {
            Ok(manejo) => Some(manejo),
            Err(error) => {
                app.notificar(
                    NivelAviso::Aviso,
                    format!("No se pudieron calcular los colores de carátula: {error:#}"),
                );
                None
            }
        };
    let manejo_directorio = if carga.config.radio.directorio {
        match mmmusic::radio::radiobrowser::lanzar(
            rutas.base_datos.clone(),
            rutas.cache_logos.clone(),
            carga.config.radio.logos,
            tx_app.clone(),
        ) {
            Ok(manejo) => Some(manejo),
            Err(error) => {
                app.notificar(
                    NivelAviso::Aviso,
                    format!("Directorio de radio desactivado: {error:#}"),
                );
                None
            }
        }
    } else {
        None
    };
    app.directorio = manejo_directorio;
    let carga_credenciales = credenciales::cargar(&rutas.credenciales)?;
    if carga_credenciales.permisos_corregidos {
        app.notificar(
            NivelAviso::Aviso,
            "Las credenciales eran legibles por otros usuarios; permisos corregidos a 600",
        );
    }
    let anillo = Arc::new(Anillo::nuevo());
    let manejo_captura = if carga.config.visuales.activo {
        match audio::pipewire::lanzar(
            carga.config.visuales.nodo.clone(),
            anillo.clone(),
            tx_app.clone(),
        ) {
            Ok((manejo, rx_captura)) => {
                app.conectar_captura(rx_captura, anillo);
                Some(manejo)
            }
            Err(error) => {
                app.notificar(NivelAviso::Aviso, format!("Visuales sin audio: {error:#}"));
                None
            }
        }
    } else {
        None
    };
    let (manejo_scrobbling, _rx_scrobbling) = scrobbling::lanzar(
        rutas.base_datos.clone(),
        rutas.credenciales.clone(),
        carga.config.scrobbling.clone(),
        carga_credenciales.credenciales,
        tx_app.clone(),
    )?;
    let (manejo_reproductor, rx_estado) = reproductor::lanzar(
        rutas.base_datos.clone(),
        carga.config.reproductor.volumen_inicial,
        carga.config.radio.espera_conexion_s,
        carga.config.ecualizador.clone(),
        tx_app.clone(),
        manejo_scrobbling.emisor(),
    )?;
    mpris::lanzar(rx_estado, manejo_reproductor.emisor(), tx_app.clone())?;
    app.conectar_letras(letras::lanzar(tx_app.clone())?);
    let mut terminal = ui::iniciar()?;
    let picker = ratatui_image::picker::Picker::from_query_stdio()
        .unwrap_or_else(|_| ratatui_image::picker::Picker::halfblocks());
    let (tx_caratulas, rx_caratulas) = mpsc::channel();
    app.activar_caratulas(picker, imagen::lanzar_worker(tx_caratulas));
    lanzar_hilo_entrada(tx_app.clone())?;
    {
        let ctx = ContextoApp {
            conn: &conn,
            tx_app: &tx_app,
            ruta_bd: &rutas.base_datos,
            dir_caratulas: &rutas.cache_caratulas,
            reproductor: &manejo_reproductor,
            scrobbling: &manejo_scrobbling,
        };
        app.refrescar_contadores(&ctx);
        app.refrescar_favoritas(&ctx);
        app.refrescar_vista(Vista::Inicio, &ctx);
        if reescaneo_pendiente {
            app.notificar(
                NivelAviso::Aviso,
                "Actualizando la biblioteca para agrupar recopilatorios…",
            );
            app.iniciar_escaneo_modo(&ctx, ModoEscaneo::Completo);
        } else if carga.config.biblioteca.escanear_al_arrancar {
            app.iniciar_escaneo(&ctx);
        }
    }

    let recursos = RecursosBucle {
        conn: &conn,
        tx_app: &tx_app,
        rx_app: &rx_app,
        rx_caratulas: &rx_caratulas,
        ruta_bd: &rutas.base_datos,
        dir_caratulas: &rutas.cache_caratulas,
        reproductor: &manejo_reproductor,
        scrobbling: &manejo_scrobbling,
    };
    let resultado = bucle(&mut terminal, &mut app, &recursos);
    app.cancelar_escaneo();
    manejo_reproductor.apagar();
    app.apagar_letras();
    if let Some(manejo) = manejo_captura {
        manejo.apagar();
    }
    if let Some(manejo) = manejo_colores {
        manejo.apagar();
    }
    if let Some(manejo) = app.directorio.take() {
        manejo.apagar();
    }
    manejo_scrobbling.apagar();
    ui::restaurar();
    resultado
}

struct RecursosBucle<'a> {
    conn: &'a Connection,
    tx_app: &'a Sender<AppEvento>,
    rx_app: &'a Receiver<AppEvento>,
    rx_caratulas: &'a Receiver<RespuestaCaratula>,
    ruta_bd: &'a Path,
    dir_caratulas: &'a Path,
    reproductor: &'a ManejoReproductor,
    scrobbling: &'a scrobbling::ManejoScrobbling,
}

fn bucle(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut AppEstado,
    recursos: &RecursosBucle<'_>,
) -> Result<()> {
    loop {
        for respuesta in recursos.rx_caratulas.try_iter() {
            match respuesta {
                RespuestaCaratula::Imagen(album_id, imagen) => {
                    app.recibir_caratula(album_id, imagen)
                }
                RespuestaCaratula::Colores(album_id, colores) => {
                    if let Err(error) =
                        consultas::albumes::fijar_colores(recursos.conn, album_id, &colores)
                    {
                        tracing::warn!(
                            "no se pudieron guardar los colores del álbum {album_id}: {error:#}"
                        );
                    }
                    app.recibir_colores(album_id, colores);
                }
            }
        }
        app.actualizar_captura();
        let inicio = Instant::now();
        app.preparar_frame();
        terminal.draw(|marco| ui::dibujar(marco, app))?;
        app.registrar_frame(inicio.elapsed());
        let intervalo = app.intervalo_tick();
        let evento = match recursos.rx_app.recv_timeout(intervalo) {
            Ok(evento) => evento,
            Err(RecvTimeoutError::Timeout) => AppEvento::Tick,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let ctx = ContextoApp {
            conn: recursos.conn,
            tx_app: recursos.tx_app,
            ruta_bd: recursos.ruta_bd,
            dir_caratulas: recursos.dir_caratulas,
            reproductor: recursos.reproductor,
            scrobbling: recursos.scrobbling,
        };
        app.manejar(evento, &ctx);
        if app.debe_salir {
            break;
        }
    }
    Ok(())
}

fn lanzar_hilo_entrada(tx: Sender<AppEvento>) -> Result<()> {
    thread::Builder::new()
        .name("entrada".to_string())
        .spawn(move || {
            loop {
                match event::read() {
                    Ok(EventoCrossterm::Key(tecla)) => {
                        if tx.send(AppEvento::Tecla(tecla)).is_err() {
                            break;
                        }
                    }
                    Ok(EventoCrossterm::Resize(ancho, alto)) => {
                        if tx.send(AppEvento::Redimension(ancho, alto)).is_err() {
                            break;
                        }
                    }
                    Ok(EventoCrossterm::Mouse(raton)) => {
                        if tx.send(AppEvento::Raton(raton)).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::error!("error leyendo eventos de terminal: {error}");
                        break;
                    }
                }
            }
        })
        .context("no se pudo lanzar el hilo de entrada")?;
    Ok(())
}

fn iniciar_logs(rutas: &Rutas) -> Result<WorkerGuard> {
    let appender = tracing_appender::rolling::daily(&rutas.dir_logs, "mmmusic.log");
    let (escritor, guardia) = tracing_appender::non_blocking(appender);
    let filtro =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("mmmusic=info,warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filtro)
        .with_writer(escritor)
        .with_ansi(false)
        .try_init();
    Ok(guardia)
}
