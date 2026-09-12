use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::{ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};
use tracing::warn;

pub const NOMBRE_APP: &str = "mmmusic";
const CARPETA_TEMA_OMARCHY: &str = ".config/omarchy/current";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub biblioteca: ConfigBiblioteca,
    pub reproductor: ConfigReproductor,
    pub interfaz: ConfigInterfaz,
    pub tema: ConfigTema,
    pub scrobbling: ConfigScrobbling,
    pub visuales: ConfigVisuales,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigBiblioteca {
    pub carpetas: Vec<String>,
    pub escanear_al_arrancar: bool,
    pub carpeta_playlists: String,
}

impl Default for ConfigBiblioteca {
    fn default() -> Self {
        Self {
            carpetas: vec!["~/Music".to_string()],
            escanear_al_arrancar: true,
            carpeta_playlists: "~/Music/Playlists".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigReproductor {
    pub volumen_inicial: u8,
    pub salto_corto_s: i64,
    pub salto_largo_s: i64,
}

impl Default for ConfigReproductor {
    fn default() -> Self {
        Self {
            volumen_inicial: 80,
            salto_corto_s: 5,
            salto_largo_s: 30,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModoIconos {
    #[default]
    Nerd,
    Ascii,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigInterfaz {
    pub caratulas: bool,
    pub iconos: ModoIconos,
    pub ancho_sidebar: u16,
}

impl Default for ConfigInterfaz {
    fn default() -> Self {
        Self {
            caratulas: true,
            iconos: ModoIconos::Nerd,
            ancho_sidebar: 18,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FuenteTema {
    #[default]
    Omarchy,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigTema {
    pub fuente: FuenteTema,
    pub acento: String,
    pub progreso: String,
}

impl Default for ConfigTema {
    fn default() -> Self {
        Self {
            fuente: FuenteTema::Omarchy,
            acento: "blue".to_string(),
            progreso: "green".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigScrobbling {
    pub listenbrainz: bool,
    pub lastfm: bool,
    pub now_playing: bool,
}

impl Default for ConfigScrobbling {
    fn default() -> Self {
        Self {
            listenbrainz: false,
            lastfm: false,
            now_playing: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FuentePaleta {
    #[default]
    Tema,
    Caratula,
}

impl FuentePaleta {
    pub fn como_str(self) -> &'static str {
        match self {
            FuentePaleta::Tema => "tema",
            FuentePaleta::Caratula => "caratula",
        }
    }

    pub fn desde_str(texto: &str) -> Option<Self> {
        match texto {
            "tema" => Some(FuentePaleta::Tema),
            "caratula" => Some(FuentePaleta::Caratula),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigVisuales {
    pub activo: bool,
    pub fps: u16,
    pub predeterminada: String,
    pub paleta: FuentePaleta,
    pub mini_espectro: bool,
    pub autoinicio_min: u64,
    pub nodo: String,
}

impl Default for ConfigVisuales {
    fn default() -> Self {
        Self {
            activo: true,
            fps: 30,
            predeterminada: "espectro".to_string(),
            paleta: FuentePaleta::Tema,
            mini_espectro: true,
            autoinicio_min: 0,
            nodo: "mmmusic".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CargaConfig {
    pub config: Config,
    pub creada: bool,
    pub ruta: PathBuf,
}

impl Config {
    pub fn cargar(ruta: &Path) -> Result<CargaConfig> {
        if ruta.exists() {
            let texto = fs::read_to_string(ruta)
                .with_context(|| format!("no se pudo leer {}", ruta.display()))?;
            let mut config: Config = toml::from_str(&texto)
                .with_context(|| format!("configuración inválida en {}", ruta.display()))?;
            config.validar();
            Ok(CargaConfig {
                config,
                creada: false,
                ruta: ruta.to_path_buf(),
            })
        } else {
            let config = Config::default();
            if let Some(padre) = ruta.parent() {
                fs::create_dir_all(padre).with_context(|| {
                    format!("no se pudo crear el directorio {}", padre.display())
                })?;
            }
            let texto = toml::to_string_pretty(&config)
                .context("no se pudo serializar la configuración por defecto")?;
            fs::write(ruta, texto)
                .with_context(|| format!("no se pudo escribir {}", ruta.display()))?;
            Ok(CargaConfig {
                config,
                creada: true,
                ruta: ruta.to_path_buf(),
            })
        }
    }

    fn validar(&mut self) {
        if self.reproductor.volumen_inicial > 100 {
            warn!(
                valor = self.reproductor.volumen_inicial,
                "volumen_inicial fuera de rango; se acota a 100"
            );
            self.reproductor.volumen_inicial = 100;
        }
        if self.reproductor.salto_corto_s <= 0 {
            warn!("salto_corto_s debe ser positivo; se usa 5");
            self.reproductor.salto_corto_s = 5;
        }
        if self.reproductor.salto_largo_s <= 0 {
            warn!("salto_largo_s debe ser positivo; se usa 30");
            self.reproductor.salto_largo_s = 30;
        }
        let ancho = self.interfaz.ancho_sidebar;
        if !(14..=40).contains(&ancho) {
            warn!(valor = ancho, "ancho_sidebar fuera de rango; se usa 18");
            self.interfaz.ancho_sidebar = 18;
        }
        if !(15..=60).contains(&self.visuales.fps) {
            warn!(
                valor = self.visuales.fps,
                "visuales.fps fuera de rango; se usa 30"
            );
            self.visuales.fps = 30;
        }
        if self.visuales.nodo.trim().is_empty() {
            warn!("visuales.nodo vacío; se usa mmmusic");
            self.visuales.nodo = "mmmusic".to_string();
        }
        if self.visuales.predeterminada.trim().is_empty() {
            warn!("visuales.predeterminada vacía; se usa espectro");
            self.visuales.predeterminada = "espectro".to_string();
        }
        if self.visuales.autoinicio_min > 1_440 {
            warn!(
                valor = self.visuales.autoinicio_min,
                "visuales.autoinicio_min fuera de rango; se acota a 1440"
            );
            self.visuales.autoinicio_min = 1_440;
        }
        if self.biblioteca.carpetas.is_empty() {
            warn!("biblioteca.carpetas vacía; se usa ~/Music");
            self.biblioteca.carpetas = vec!["~/Music".to_string()];
        }
    }

    pub fn carpetas_expandidas(&self) -> Vec<PathBuf> {
        self.biblioteca
            .carpetas
            .iter()
            .map(|c| expandir_ruta(c))
            .collect()
    }

    pub fn carpeta_playlists(&self) -> PathBuf {
        expandir_ruta(&self.biblioteca.carpeta_playlists)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rutas {
    pub config: PathBuf,
    pub credenciales: PathBuf,
    pub base_datos: PathBuf,
    pub cache_caratulas: PathBuf,
    pub dir_logs: PathBuf,
    pub dir_tema: PathBuf,
    pub fichero_tema: PathBuf,
}

impl Rutas {
    pub fn detectar() -> Result<Self> {
        let proyecto = ProjectDirs::from("", "", NOMBRE_APP)
            .context("no se pudo determinar el directorio de datos del usuario")?;
        let usuario =
            UserDirs::new().context("no se pudo determinar el directorio personal del usuario")?;
        let dir_tema = usuario.home_dir().join(CARPETA_TEMA_OMARCHY);
        Ok(Self {
            config: proyecto.config_dir().join("config.toml"),
            credenciales: proyecto.config_dir().join("credenciales.toml"),
            base_datos: proyecto.data_dir().join("mmmusic.db"),
            cache_caratulas: proyecto.cache_dir().join("caratulas"),
            dir_logs: proyecto
                .state_dir()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| usuario.home_dir().join(".local/state").join(NOMBRE_APP)),
            fichero_tema: dir_tema.join("theme/alacritty.toml"),
            dir_tema,
        })
    }

    pub fn crear_directorios(&self) -> Result<()> {
        let mut directorios = vec![self.cache_caratulas.clone(), self.dir_logs.clone()];
        if let Some(padre) = self.base_datos.parent() {
            directorios.push(padre.to_path_buf());
        }
        if let Some(padre) = self.credenciales.parent() {
            directorios.push(padre.to_path_buf());
        }
        for dir in directorios {
            fs::create_dir_all(&dir)
                .with_context(|| format!("no se pudo crear el directorio {}", dir.display()))?;
        }
        Ok(())
    }
}

pub fn expandir_ruta(bruta: &str) -> PathBuf {
    let usuario = UserDirs::new();
    let hogar = usuario
        .as_ref()
        .map(|u| u.home_dir().to_path_buf())
        .or_else(|| env::var_os("HOME").map(PathBuf::from));
    expandir_con_hogar(bruta, hogar.as_deref())
}

fn expandir_con_hogar(bruta: &str, hogar: Option<&Path>) -> PathBuf {
    if bruta == "~" {
        hogar
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(bruta))
    } else if let Some(resto) = bruta.strip_prefix("~/") {
        match hogar {
            Some(h) => h.join(resto),
            None => PathBuf::from(bruta),
        }
    } else {
        PathBuf::from(expandir_variables(bruta))
    }
}

fn expandir_variables(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(pos) = resto.find('$') {
        salida.push_str(&resto[..pos]);
        resto = &resto[pos + 1..];
        let (nombre, siguiente) = if let Some(interior) = resto.strip_prefix('{') {
            match interior.find('}') {
                Some(fin) => (&interior[..fin], &interior[fin + 1..]),
                None => {
                    salida.push('$');
                    break;
                }
            }
        } else {
            let fin = resto
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(resto.len());
            (&resto[..fin], &resto[fin..])
        };
        match env::var(nombre) {
            Ok(valor) => salida.push_str(&valor),
            Err(_) => {
                salida.push('$');
                salida.push_str(nombre);
            }
        }
        resto = siguiente;
    }
    salida.push_str(resto);
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn config_por_defecto_se_crea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ruta = dir.path().join("mmm/config.toml");
        let carga = Config::cargar(&ruta).expect("carga inicial");
        assert!(carga.creada);
        assert_eq!(carga.config.biblioteca.carpetas, vec!["~/Music"]);
        assert!(ruta.exists());

        let recarga = Config::cargar(&ruta).expect("recarga");
        assert!(!recarga.creada);
        assert_eq!(recarga.config, carga.config);
    }

    #[test]
    fn parseo_de_config_completa() {
        let texto = r#"
[biblioteca]
carpetas = ["/mnt/musica", "~/Otros"]
escanear_al_arrancar = false
carpeta_playlists = "~/Listas"

[reproductor]
volumen_inicial = 55
salto_corto_s = 10
salto_largo_s = 60

[interfaz]
caratulas = false
iconos = "ascii"
ancho_sidebar = 20

[tema]
fuente = "terminal"
acento = "magenta"
progreso = "cyan"
"#;
        let config: Config = toml::from_str(texto).expect("parseo");
        assert_eq!(config.biblioteca.carpetas.len(), 2);
        assert!(!config.biblioteca.escanear_al_arrancar);
        assert_eq!(config.reproductor.volumen_inicial, 55);
        assert_eq!(config.interfaz.iconos, ModoIconos::Ascii);
        assert!(!config.interfaz.caratulas);
        assert_eq!(config.tema.fuente, FuenteTema::Terminal);
        assert_eq!(config.tema.acento, "magenta");
    }

    #[test]
    fn validacion_acota_valores() {
        let texto = r#"
[reproductor]
volumen_inicial = 250
salto_corto_s = 0

[interfaz]
ancho_sidebar = 5
"#;
        let mut config: Config = toml::from_str(texto).expect("parseo");
        config.validar();
        assert_eq!(config.reproductor.volumen_inicial, 100);
        assert_eq!(config.reproductor.salto_corto_s, 5);
        assert_eq!(config.interfaz.ancho_sidebar, 18);
    }

    #[test]
    fn visuales_se_parsean_y_acotan() {
        let texto = r#"
[visuales]
activo = false
fps = 200
predeterminada = ""
paleta = "caratula"
mini_espectro = false
autoinicio_min = 99999
nodo = ""
"#;
        let mut config: Config = toml::from_str(texto).expect("parseo");
        assert!(!config.visuales.activo);
        assert_eq!(config.visuales.paleta, FuentePaleta::Caratula);
        config.validar();
        assert_eq!(config.visuales.fps, 30);
        assert_eq!(config.visuales.nodo, "mmmusic");
        assert_eq!(config.visuales.predeterminada, "espectro");
        assert_eq!(config.visuales.autoinicio_min, 1_440);
    }

    #[test]
    fn expande_tilde_y_variables() {
        let hogar = Path::new("/hogar/prueba");
        assert_eq!(
            expandir_con_hogar("~/Musica", Some(hogar)),
            PathBuf::from("/hogar/prueba/Musica")
        );
        assert_eq!(
            expandir_con_hogar("~", Some(hogar)),
            PathBuf::from("/hogar/prueba")
        );
        assert_eq!(
            expandir_con_hogar("/absoluta", None),
            PathBuf::from("/absoluta")
        );

        // SAFETY: no hay otros hilos leyendo el entorno en este test.
        unsafe { env::set_var("MMMUSIC_PRUEBA_VAR", "/tmp/mmmusic-var") };
        assert_eq!(
            expandir_con_hogar("$MMMUSIC_PRUEBA_VAR/audio", None),
            PathBuf::from("/tmp/mmmusic-var/audio")
        );
        assert_eq!(
            expandir_con_hogar("${MMMUSIC_PRUEBA_VAR}/audio", None),
            PathBuf::from("/tmp/mmmusic-var/audio")
        );
        unsafe { env::remove_var("MMMUSIC_PRUEBA_VAR") };
    }
}
