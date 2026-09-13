use anyhow::{Context, Result};
use libmpv2::events::{Event, PropertyData};
use libmpv2::{Format, Mpv, mpv_end_file_reason};
use tracing::warn;

pub const OPCIONES_INICIALES: &[(&str, &str)] = &[
    ("vo", "null"),
    ("audio-display", "no"),
    ("ytdl", "no"),
    ("load-scripts", "no"),
    ("config", "no"),
    ("gapless-audio", "yes"),
    ("ao", "pipewire,"),
    ("audio-client-name", "mmmusic"),
    ("cache", "yes"),
    ("demuxer-max-bytes", "32MiB"),
    (
        "stream-lavf-o",
        "reconnect=1,reconnect_streamed=1,reconnect_delay_max=5",
    ),
    ("cache-pause", "no"),
];

#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Texto(String),
    Entero(i64),
    Flotante(f64),
    Bandera(bool),
}

impl Valor {
    pub fn numero(&self) -> Option<f64> {
        match self {
            Valor::Entero(valor) => Some(*valor as f64),
            Valor::Flotante(valor) => Some(*valor),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RazonFin {
    Eof,
    Detenida,
    Error,
    Otra,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventoMpv {
    Propiedad { nombre: String, valor: Valor },
    Cargado,
    FinDePista { razon: RazonFin },
    Fallo(String),
    Apagado,
}

pub struct ReproductorMpv {
    mpv: Mpv,
}

impl ReproductorMpv {
    pub fn nuevo(volumen_inicial: u8) -> Result<Self> {
        let mpv = Mpv::with_initializer(|inicializador| {
            for (clave, valor) in OPCIONES_INICIALES {
                inicializador.set_property(clave, *valor)?;
            }
            inicializador.set_property("volume", f64::from(volumen_inicial))?;
            inicializador.set_property("pause", true)?;
            Ok(())
        })
        .context("no se pudo inicializar libmpv")?;
        Ok(Self { mpv })
    }

    pub fn observar_propiedades(&self) -> Result<()> {
        let propiedades: &[(&str, Format, u64)] = &[
            ("time-pos", Format::Double, 1),
            ("duration", Format::Double, 2),
            ("pause", Format::Flag, 3),
            ("volume", Format::Double, 4),
            ("mute", Format::Flag, 5),
            ("eof-reached", Format::Flag, 6),
            ("idle-active", Format::Flag, 7),
            ("paused-for-cache", Format::Flag, 8),
            ("cache-buffering-state", Format::Int64, 9),
            ("demuxer-cache-duration", Format::Double, 10),
            ("metadata/by-key/icy-title", Format::String, 11),
            ("audio-codec-name", Format::String, 12),
            ("audio-bitrate", Format::Double, 13),
            ("playlist-count", Format::Int64, 14),
        ];
        for (nombre, formato, id) in propiedades {
            self.mpv
                .observe_property(nombre, *formato, *id)
                .with_context(|| format!("no se pudo observar la propiedad {nombre}"))?;
        }
        Ok(())
    }

    pub fn fijar_despertador<F: Fn() + Send + 'static>(&mut self, despertador: F) {
        self.mpv.set_wakeup_callback(despertador);
    }

    pub fn cargar(&self, ruta: &str) -> Result<()> {
        self.mpv
            .command("loadfile", &[ruta, "replace"])
            .with_context(|| format!("no se pudo cargar {ruta}"))
    }

    /// Carga un stream o lista remota con las opciones de caché ya activas.
    pub fn cargar_stream(&self, url: &str) -> Result<()> {
        self.mpv
            .command("loadfile", &[url, "replace"])
            .with_context(|| format!("no se pudo cargar el stream {url}"))
    }

    /// Carga una lista PLS/M3U remota; sus entradas quedan como espejos.
    pub fn cargar_lista(&self, url: &str) -> Result<()> {
        self.mpv
            .command("loadlist", &[url, "replace"])
            .with_context(|| format!("no se pudo cargar la lista {url}"))
    }

    pub fn siguiente_lista(&self) -> Result<()> {
        self.mpv
            .command("playlist-next", &[])
            .context("no se pudo avanzar de espejo")
    }

    pub fn anexar(&self, ruta: &str) -> Result<()> {
        self.mpv
            .command("loadfile", &[ruta, "append"])
            .with_context(|| format!("no se pudo precargar {ruta}"))
    }

    pub fn pausar(&self, pausa: bool) -> Result<()> {
        self.mpv
            .set_property("pause", pausa)
            .context("no se pudo cambiar el estado de pausa")
    }

    pub fn fijar_volumen(&self, volumen: f64) -> Result<()> {
        self.mpv
            .set_property("volume", volumen)
            .context("no se pudo fijar el volumen")
    }

    pub fn fijar_silencio(&self, silencio: bool) -> Result<()> {
        self.mpv
            .set_property("mute", silencio)
            .context("no se pudo cambiar el silencio")
    }

    /// Instala (o vacía) la cadena de filtros de audio etiquetada.
    pub fn fijar_af(&self, cadena: &str) -> Result<()> {
        self.mpv
            .set_property("af", cadena)
            .context("no se pudo fijar la cadena de filtros")
    }

    /// Cambia un parámetro de un filtro etiquetado sin reconstruir la cadena.
    /// `filtro` es el `target` que exige mpv (el nombre del filtro FFmpeg,
    /// p. ej. `equalizer` o `volume`); la etiqueta se pasa sin `@`.
    pub fn af_command(
        &self,
        etiqueta: &str,
        comando: &str,
        valor: &str,
        filtro: &str,
    ) -> Result<()> {
        let etiqueta = etiqueta.trim_start_matches('@');
        self.mpv
            .command("af-command", &[etiqueta, comando, valor, filtro])
            .with_context(|| format!("no se pudo aplicar af-command {etiqueta} {comando}"))
    }

    pub fn fijar_texto(&self, propiedad: &str, valor: &str) -> Result<()> {
        self.mpv
            .set_property(propiedad, valor)
            .with_context(|| format!("no se pudo fijar {propiedad}"))
    }

    pub fn fijar_numero(&self, propiedad: &str, valor: f64) -> Result<()> {
        self.mpv
            .set_property(propiedad, valor)
            .with_context(|| format!("no se pudo fijar {propiedad}"))
    }

    /// Comprueba si la build de mpv trae filtros lavfi. Deja `af` vacío.
    pub fn lavfi_disponible(&self) -> bool {
        let prueba = "@eqtest:lavfi=[equalizer=f=1000:width_type=q:width=1.41:gain=0]";
        let disponible = self.mpv.set_property("af", prueba).is_ok();
        let _ = self.mpv.set_property("af", "");
        disponible
    }

    pub fn buscar_relativo(&self, segundos: f64) -> Result<()> {
        self.mpv
            .command("seek", &[&format!("{segundos:.3}"), "relative"])
            .context("no se pudo hacer el salto relativo")
    }

    pub fn buscar_absoluto(&self, milisegundos: i64) -> Result<()> {
        let segundos = milisegundos.max(0) as f64 / 1000.0;
        self.mpv
            .command("seek", &[&format!("{segundos:.3}"), "absolute"])
            .context("no se pudo hacer el salto absoluto")
    }

    pub fn detener(&self) -> Result<()> {
        self.mpv.command("stop", &[]).context("no se pudo detener")
    }

    pub fn numero(&self, propiedad: &str) -> Option<f64> {
        self.mpv.get_property::<f64>(propiedad).ok()
    }

    pub fn entero(&self, propiedad: &str) -> Option<i64> {
        self.mpv.get_property::<i64>(propiedad).ok()
    }

    pub fn cadena(&self, propiedad: &str) -> Option<String> {
        self.mpv.get_property::<String>(propiedad).ok()
    }

    pub fn bandera(&self, propiedad: &str) -> Option<bool> {
        self.mpv.get_property::<bool>(propiedad).ok()
    }

    pub fn esperar_evento(&self, timeout: f64) -> Option<EventoMpv> {
        match self.mpv.wait_event(timeout) {
            None => None,
            Some(Err(error)) => {
                warn!("evento de mpv con error: {error}");
                Some(EventoMpv::Fallo(error.to_string()))
            }
            Some(Ok(Event::PropertyChange { name, change, .. })) => {
                let valor = match change {
                    PropertyData::Str(texto) | PropertyData::OsdStr(texto) => {
                        Valor::Texto(texto.to_string())
                    }
                    PropertyData::Flag(bandera) => Valor::Bandera(bandera),
                    PropertyData::Int64(entero) => Valor::Entero(entero),
                    PropertyData::Double(decimal) => Valor::Flotante(decimal),
                };
                Some(EventoMpv::Propiedad {
                    nombre: name.to_string(),
                    valor,
                })
            }
            Some(Ok(Event::FileLoaded)) => Some(EventoMpv::Cargado),
            Some(Ok(Event::EndFile(razon))) => {
                let razon = if razon == mpv_end_file_reason::Eof {
                    RazonFin::Eof
                } else if razon == mpv_end_file_reason::Stop || razon == mpv_end_file_reason::Quit {
                    RazonFin::Detenida
                } else if razon == mpv_end_file_reason::Error {
                    RazonFin::Error
                } else {
                    RazonFin::Otra
                };
                Some(EventoMpv::FinDePista { razon })
            }
            Some(Ok(Event::Shutdown)) => Some(EventoMpv::Apagado),
            Some(Ok(_)) => None,
        }
    }
}
