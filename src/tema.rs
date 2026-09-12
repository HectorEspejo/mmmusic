use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender as MpscSender};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use notify::{RecursiveMode, Watcher};
use ratatui::style::Color;
use tracing::{info, warn};

use crate::config::{ConfigTema, FuenteTema};
use crate::eventos::{AppEvento, NivelAviso};

const DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paleta {
    pub fondo: Color,
    pub texto: Color,
    pub acento: Color,
    pub progreso: Color,
    pub aviso: Color,
    pub error: Color,
    pub secundario: Color,
}

pub fn terminal(tema: &ConfigTema) -> Paleta {
    Paleta {
        fondo: Color::Reset,
        texto: Color::Reset,
        acento: color_ansi(&tema.acento).unwrap_or(Color::Blue),
        progreso: color_ansi(&tema.progreso).unwrap_or(Color::Green),
        aviso: Color::Yellow,
        error: Color::Red,
        secundario: Color::DarkGray,
    }
}

pub fn cargar(tema: &ConfigTema, fichero: &Path) -> (Paleta, Option<String>) {
    if tema.fuente == FuenteTema::Terminal {
        return (terminal(tema), None);
    }
    if !fichero.exists() {
        return (terminal(tema), None);
    }
    match fs::read_to_string(fichero) {
        Ok(texto) => match paleta_desde_toml(&texto, tema) {
            Ok(paleta) => (paleta, None),
            Err(motivo) => (terminal(tema), Some(format!("Tema no legible: {motivo}"))),
        },
        Err(error) => (terminal(tema), Some(format!("Tema no legible: {error}"))),
    }
}

pub fn paleta_desde_toml(texto: &str, tema: &ConfigTema) -> std::result::Result<Paleta, String> {
    let valor: toml::Value = toml::from_str(texto).map_err(|e| e.to_string())?;
    let colores = valor
        .get("colors")
        .ok_or_else(|| "falta la sección [colors]".to_string())?;
    let primario = colores
        .get("primary")
        .ok_or_else(|| "falta [colors.primary]".to_string())?;
    let fondo = primario
        .get("background")
        .and_then(color_de_valor)
        .ok_or_else(|| "falta colors.primary.background".to_string())?;
    let texto_color = primario
        .get("foreground")
        .and_then(color_de_valor)
        .ok_or_else(|| "falta colors.primary.foreground".to_string())?;
    let normal = colores.get("normal");
    let acento = normal
        .and_then(|n| n.get(tema.acento.as_str()))
        .and_then(color_de_valor)
        .or_else(|| color_ansi(&tema.acento))
        .unwrap_or(Color::Blue);
    let progreso = normal
        .and_then(|n| n.get(tema.progreso.as_str()))
        .and_then(color_de_valor)
        .or_else(|| color_ansi(&tema.progreso))
        .unwrap_or(Color::Green);
    let amarillo = normal
        .and_then(|n| n.get("yellow"))
        .and_then(color_de_valor)
        .unwrap_or(Color::Yellow);
    let rojo = normal
        .and_then(|n| n.get("red"))
        .and_then(color_de_valor)
        .unwrap_or(Color::Red);
    let secundario = colores
        .get("bright")
        .and_then(|b| b.get("black"))
        .and_then(color_de_valor)
        .or_else(|| normal.and_then(|n| n.get("black")).and_then(color_de_valor))
        .unwrap_or(Color::DarkGray);
    Ok(Paleta {
        fondo,
        texto: texto_color,
        acento,
        progreso,
        aviso: amarillo,
        error: rojo,
        secundario,
    })
}

fn color_de_valor(valor: &toml::Value) -> Option<Color> {
    let texto = valor.as_str()?.trim();
    let hex = texto.strip_prefix('#').unwrap_or(texto);
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let rojo = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let verde = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let azul = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(rojo, verde, azul))
}

fn color_ansi(nombre: &str) -> Option<Color> {
    match nombre.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::Gray),
        "bright_black" => Some(Color::DarkGray),
        "bright_red" => Some(Color::LightRed),
        "bright_green" => Some(Color::LightGreen),
        "bright_yellow" => Some(Color::LightYellow),
        "bright_blue" => Some(Color::LightBlue),
        "bright_magenta" => Some(Color::LightMagenta),
        "bright_cyan" => Some(Color::LightCyan),
        "bright_white" => Some(Color::White),
        _ => None,
    }
}

#[derive(Debug)]
pub struct VigilanteTema {
    cancelacion: Arc<AtomicBool>,
}

impl VigilanteTema {
    pub fn detener(&self) {
        self.cancelacion.store(true, Ordering::SeqCst);
    }
}

impl Drop for VigilanteTema {
    fn drop(&mut self) {
        self.detener();
    }
}

pub fn vigilar(
    base: PathBuf,
    tema: ConfigTema,
    tx: MpscSender<AppEvento>,
) -> Result<VigilanteTema> {
    let (tx_notify, rx_notify) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |resultado| {
        let _ = tx_notify.send(resultado);
    })
    .context("no se pudo crear el vigilante de ficheros")?;
    watcher
        .watch(&base, RecursiveMode::Recursive)
        .with_context(|| format!("no se pudo vigilar {}", base.display()))?;
    let cancelacion = Arc::new(AtomicBool::new(false));
    let cancelacion_hilo = cancelacion.clone();
    thread::Builder::new()
        .name("tema".to_string())
        .spawn(move || {
            let _watcher = watcher;
            let fichero = base.join("theme/alacritty.toml");
            loop {
                if cancelacion_hilo.load(Ordering::SeqCst) {
                    break;
                }
                match rx_notify.recv_timeout(DEBOUNCE) {
                    Ok(Ok(_)) => continue,
                    Ok(Err(error)) => warn!("evento de tema con error: {error}"),
                    Err(RecvTimeoutError::Timeout) => {
                        if !fichero.exists() {
                            continue;
                        }
                        match fs::read_to_string(&fichero)
                            .map_err(|e| e.to_string())
                            .and_then(|texto| paleta_desde_toml(&texto, &tema))
                        {
                            Ok(paleta) => {
                                info!("paleta recargada desde {}", fichero.display());
                                if tx.send(AppEvento::TemaActualizado(paleta)).is_err() {
                                    break;
                                }
                            }
                            Err(motivo) => {
                                warn!("tema no legible: {motivo}");
                                let _ = tx.send(AppEvento::Notificacion(
                                    NivelAviso::Aviso,
                                    format!("Tema no legible: {motivo}"),
                                ));
                            }
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .context("no se pudo lanzar el hilo del tema")?;
    Ok(VigilanteTema { cancelacion })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const ALACRITTY: &str = r##"
[colors.primary]
background = "#1d2021"
foreground = "#ebdbb2"

[colors.normal]
black = "#282828"
red = "#cc241d"
green = "#98971a"
yellow = "#d79921"
blue = "#458588"
magenta = "#b16286"
cyan = "#689d6a"
white = "#a89984"

[colors.bright]
black = "#928374"
blue = "#83a598"
"##;

    fn tema_omarchy() -> ConfigTema {
        ConfigTema::default()
    }

    #[test]
    fn parsea_paleta_de_alacritty() {
        let paleta = paleta_desde_toml(ALACRITTY, &tema_omarchy()).expect("paleta");
        assert_eq!(paleta.fondo, Color::Rgb(0x1d, 0x20, 0x21));
        assert_eq!(paleta.texto, Color::Rgb(0xeb, 0xdb, 0xb2));
        assert_eq!(paleta.acento, Color::Rgb(0x45, 0x85, 0x88));
        assert_eq!(paleta.progreso, Color::Rgb(0x98, 0x97, 0x1a));
        assert_eq!(paleta.aviso, Color::Rgb(0xd7, 0x99, 0x21));
        assert_eq!(paleta.error, Color::Rgb(0xcc, 0x24, 0x1d));
        assert_eq!(paleta.secundario, Color::Rgb(0x92, 0x83, 0x74));
    }

    #[test]
    fn mapea_acento_configurable() {
        let mut tema = tema_omarchy();
        tema.acento = "magenta".to_string();
        tema.progreso = "cyan".to_string();
        let paleta = paleta_desde_toml(ALACRITTY, &tema).expect("paleta");
        assert_eq!(paleta.acento, Color::Rgb(0xb1, 0x62, 0x86));
        assert_eq!(paleta.progreso, Color::Rgb(0x68, 0x9d, 0x6a));
    }

    #[test]
    fn tema_ilegible_cae_a_terminal() {
        let mut tema = tema_omarchy();
        tema.acento = "magenta".to_string();
        let error = paleta_desde_toml("esto no es toml = =", &tema);
        assert!(error.is_err());
        let paleta = terminal(&tema);
        assert_eq!(paleta.acento, Color::Magenta);
    }

    #[test]
    fn fuente_terminal_ignora_fichero() {
        let mut tema = tema_omarchy();
        tema.fuente = FuenteTema::Terminal;
        let (paleta, aviso) = cargar(&tema, Path::new("/no/existe/alacritty.toml"));
        assert_eq!(paleta, terminal(&tema));
        assert!(aviso.is_none());
    }
}
