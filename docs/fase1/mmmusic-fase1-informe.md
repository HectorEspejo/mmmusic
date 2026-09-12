# mmmusic - Fase 1: Reproductor Base

## Especificación Funcional

**Versión:** 1.0
**Fecha:** 12 de septiembre de 2026
**Cliente:** Proyecto personal (Hector) — sin cliente externo

---

## 1. Visión General

**mmmusic** es un reproductor de música para terminal (TUI) pensado para Omarchy (Arch Linux + Hyprland). Reproduce la biblioteca local del usuario con una disposición de pantalla inspirada en Spotify / Apple Music: barra lateral de navegación a la izquierda, panel central de contenido y barra inferior fija de "sonando ahora" con controles y progreso. Sustituye a Cliamp como reproductor habitual del usuario.

La Fase 1 entrega un reproductor completo y usable a diario. No hay servidor, ni cuentas, ni red: todo ocurre en la máquina del usuario.

### Objetivos principales

1. Indexar una o varias carpetas de música en SQLite con escaneo incremental (solo se releen los ficheros nuevos o modificados).
2. Reproducir MP3, FLAC, OGG/Opus, M4A/AAC y WAV sin cortes usando mpv como motor (libmpv).
3. Ofrecer la navegación clásica de un reproductor de escritorio: Inicio, Buscar, Artistas, Álbumes, Pistas, Playlists y Cola, con atajos estilo vim.
4. Integrarse en el escritorio: colores del tema activo de Omarchy, carátulas en terminal y MPRIS (teclas multimedia de Hyprland, Waybar, `playerctl`).
5. Gestionar playlists propias con importación/exportación M3U y una cola de reproducción que sobrevive al cierre de la aplicación.
6. Ser un binario único en Rust, rápido de arrancar y con dependencias de sistema mínimas (`mpv`).

### Contexto

El usuario usa Omarchy con Alacritty como terminal, temas de Omarchy (`omarchy-theme-set`) y fuentes Nerd Font. Su biblioteca es local (descargas de Soulseek vía Nicotine+ y música propia). No se contempla ningún servicio de streaming: no existe una vía legal y estable para reproducir Spotify o Apple Music desde un cliente propio.

Adaptación de la plantilla: al no haber cliente ni backend web, la sección "API Endpoints" se sustituye por "Comandos, atajos y API interna", y los requisitos de RGPD se reducen a los datos locales del propio usuario.

---

## 2. Arquitectura Técnica

### Stack tecnológico

| Capa | Tecnología | Motivo |
|------|-----------|--------|
| Lenguaje | Rust (edición 2024, stable) | Binario único, rendimiento, ecosistema TUI maduro, encaja con Omarchy |
| TUI | `ratatui` + `crossterm` | Framework de facto para TUIs en Rust; layouts anidados |
| Audio | `mpv` (libmpv) vía crate `libmpv2` | Gapless, todos los formatos, decodificación probada; se enlaza con la `libmpv.so` del sistema |
| Base de datos | SQLite vía `rusqlite` (feature `bundled`) | Índice de biblioteca, playlists, cola, historial; fichero único |
| Etiquetas | `lofty` | Lee ID3v2, Vorbis Comments, MP4 atoms, RIFF INFO y carátulas embebidas |
| Carátulas | `ratatui-image` + `image` | Detecta protocolo (Kitty/Sixel/iTerm2) y cae a half-blocks en Alacritty |
| MPRIS | `mpris-server` (zbus) sobre `tokio` | Implementa `org.mpris.MediaPlayer2` en el bus de sesión |
| Configuración | `serde` + `toml` + `directories` | `~/.config/mmmusic/config.toml`, rutas XDG |
| Tema | Parser TOML del `alacritty.toml` del tema de Omarchy + `notify` | Colores del sistema, recarga en caliente al cambiar de tema |
| Escaneo | `walkdir` + hilo dedicado | Recorrido incremental por mtime/tamaño |
| Errores y logs | `anyhow`, `thiserror`, `tracing` + `tracing-appender` | Log en `~/.local/state/mmmusic/mmmusic.log` |
| Dependencias de sistema (Arch) | `mpv`, `pkgconf` | `libmpv.so` y cabeceras para la compilación |

### Estructura de carpetas del proyecto

```
mmmusic/
├── CLAUDE.md
├── Cargo.toml
├── Cargo.lock
├── README.md
├── docs/
│   ├── mmmusic-maestro.md
│   ├── mmmusic-fase1-informe.md
│   ├── mmmusic-fase1-checklist.md
│   ├── mmmusic-fase1-prompt.md
│   └── mmmusic-fase1-implementacion.md      (lo escribe el agente)
├── config.ejemplo.toml
├── src/
│   ├── main.rs                  # arranque, terminal raw mode, bucle principal
│   ├── app.rs                   # estado global de la app (AppEstado) y reductor de eventos
│   ├── eventos.rs               # enum AppEvento / ComandoReproductor y canales
│   ├── config.rs                # carga/validación de config.toml, rutas XDG
│   ├── tema.rs                  # lectura del tema Omarchy, paleta, watcher
│   ├── biblioteca/
│   │   ├── mod.rs               # API pública: consultas
│   │   ├── bd.rs                # conexión rusqlite, migraciones (PRAGMA user_version)
│   │   ├── migraciones/
│   │   │   └── 001_inicial.sql
│   │   ├── escaner.rs           # hilo de escaneo incremental
│   │   ├── etiquetas.rs         # lofty → PistaEtiquetas, normalización
│   │   ├── caratulas.rs         # extracción y caché de carátulas
│   │   ├── modelos.rs           # structs Artista, Album, Pista, Playlist...
│   │   └── consultas.rs         # SQL: listados, búsqueda, playlists, cola, historial
│   ├── reproductor/
│   │   ├── mod.rs               # hilo del reproductor, ComandoReproductor
│   │   ├── mpv.rs               # envoltorio libmpv2 (propiedades observadas, eventos)
│   │   ├── cola.rs              # Cola: orden, aleatorio, repetición, persistencia
│   │   └── estado.rs            # EstadoReproduccion (compartido con UI y MPRIS)
│   ├── mpris.rs                 # servidor MPRIS (tokio + mpris-server)
│   └── ui/
│       ├── mod.rs               # layout raíz (sidebar | contenido / barra inferior)
│       ├── teclas.rs            # mapa de atajos → Accion
│       ├── sidebar.rs
│       ├── barra_inferior.rs    # sonando ahora, progreso, controles
│       ├── vistas/
│       │   ├── inicio.rs
│       │   ├── buscar.rs
│       │   ├── artistas.rs      # lista + detalle de artista
│       │   ├── albumes.rs       # rejilla + detalle de álbum
│       │   ├── pistas.rs        # tabla de todas las pistas
│       │   ├── playlists.rs     # lista + detalle + selector
│       │   ├── cola.rs          # panel lateral derecho
│       │   └── ayuda.rs
│       └── componentes/
│           ├── lista.rs         # lista con scroll y selección
│           ├── tabla.rs
│           ├── rejilla.rs       # rejilla de tarjetas con carátula
│           ├── imagen.rs        # envoltorio ratatui-image con caché LRU
│           ├── dialogo.rs       # confirmación, input de texto
│           └── notificacion.rs  # toasts temporales
└── tests/
    ├── fixtures/                # ficheros de audio diminutos (mp3, flac, ogg, opus, m4a, wav)
    ├── escaneo.rs
    ├── cola.rs
    └── etiquetas.rs
```

### Diagrama de arquitectura

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                               Proceso mmmusic                               │
│                                                                             │
│  ┌──────────────────────┐   AppEvento (mpsc)   ┌─────────────────────────┐  │
│  │  Hilo principal (UI) │◄─────────────────────┤  Hilo Reproductor       │  │
│  │  ratatui + crossterm │  ComandoReproductor  │  libmpv2 (wait_event)   │──┼──► libmpv.so ──► PipeWire
│  │  AppEstado, vistas   │─────────────────────►│  Cola, EstadoReproduc.  │  │
│  └──────┬───────┬───────┘                      └───────────┬─────────────┘  │
│         │       │ EventoEscaneo                            │ watch<Estado>   │
│         │       │                                          ▼                 │
│         │  ┌────┴─────────────┐                 ┌─────────────────────────┐  │
│         │  │  Hilo Escáner    │                 │  Hilo MPRIS (tokio)     │──┼──► D-Bus sesión
│         │  │  walkdir + lofty │                 │  mpris-server (zbus)    │  │    (Waybar, playerctl,
│         │  └────┬─────────────┘                 └─────────────────────────┘  │     teclas Hyprland)
│         │       │                                                            │
│         ▼       ▼                                                            │
│  ┌──────────────────────┐        ┌──────────────────────┐                    │
│  │ SQLite (rusqlite)    │        │ Hilo Tema (notify)   │──► ~/.config/omarchy/current/theme
│  │ ~/.local/share/      │        │ recarga paleta       │                    │
│  │   mmmusic/mmmusic.db │        └──────────────────────┘                    │
│  └──────────────────────┘                                                    │
│  Caché carátulas: ~/.cache/mmmusic/caratulas/{album_id}.jpg                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

Principios:

- El hilo de UI nunca bloquea: toda E/S lenta (escaneo, decodificación de imágenes grandes, D-Bus) va en hilos propios y comunica por canales.
- La UI y MPRIS reciben el mismo `EstadoReproduccion` a través de un canal `watch`; solo el hilo reproductor lo escribe.
- La base de datos la abre el hilo de UI (consultas) y el hilo escáner (escrituras) con conexiones separadas en modo WAL y `busy_timeout` de 5 s.

---

## 3. Modelo de Datos

### Diagrama E-R

```
┌──────────────┐        ┌──────────────┐        ┌──────────────────┐
│  ARTISTAS    │1      *│  ALBUMES     │1      *│  PISTAS          │
│──────────────│────────│──────────────│────────│──────────────────│
│ id           │        │ id           │        │ id               │
│ nombre       │        │ artista_id   │        │ album_id         │
│ nombre_norm  │        │ titulo       │        │ artista_id       │──┐ (artista de la pista,
│ creado_en    │        │ titulo_norm  │        │ titulo           │  │  puede diferir del
└──────────────┘        │ anio         │        │ titulo_norm      │  │  artista del álbum)
       ▲                │ caratula_ruta│        │ numero_pista     │  │
       └────────────────│ creado_en    │        │ numero_disco     │  │
                        └──────────────┘        │ genero           │  │
                                                │ duracion_ms      │  │
                                                │ ruta (única)     │  │
                                                │ formato          │  │
                                                │ tamano_bytes     │  │
                                                │ modificado_en    │  │
                                                │ bitrate_kbps     │  │
                                                │ anadido_en       │  │
                                                │ escaneo_id       │──┼──► ESCANEOS
                                                └──────┬───────────┘  │
                     ┌──────────────────────────┬──────┴───────┬──────┴─────────┐
                     │*                         │*             │*               │
           ┌─────────┴─────────┐     ┌──────────┴──────┐  ┌────┴────────────────┴──┐
           │ PLAYLIST_PISTAS   │     │ COLA            │  │ HISTORIAL_REPRODUCCION │
           │───────────────────│     │─────────────────│  │────────────────────────│
           │ id                │     │ id              │  │ id                     │
           │ playlist_id       │     │ pista_id        │  │ pista_id               │
           │ pista_id          │     │ posicion        │  │ reproducido_en         │
           │ posicion          │     │ posicion_orig   │  │ completada             │
           └─────────┬─────────┘     └─────────────────┘  └────────────────────────┘
                     │*
           ┌─────────┴─────────┐     ┌─────────────────┐  ┌────────────────────────┐
           │ PLAYLISTS         │     │ ESCANEOS        │  │ AJUSTES                │
           │───────────────────│     │─────────────────│  │────────────────────────│
           │ id                │     │ id              │  │ clave (PK)             │
           │ nombre            │     │ iniciado_en     │  │ valor                  │
           │ creado_en         │     │ finalizado_en   │  └────────────────────────┘
           │ actualizado_en    │     │ estado          │
           └───────────────────┘     │ nuevas          │
                                     │ actualizadas    │
                                     │ eliminadas      │
                                     │ error_msg       │
                                     └─────────────────┘
```

### Definición de tablas

**ARTISTAS**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| nombre | TEXT NOT NULL | Nombre tal como aparece en las etiquetas |
| nombre_norm | TEXT NOT NULL UNIQUE | Normalizado (minúsculas, sin acentos, espacios colapsados); clave de deduplicación y búsqueda |
| creado_en | TEXT NOT NULL | ISO 8601 |

**ALBUMES**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| artista_id | INTEGER FK → ARTISTAS | Artista del álbum (`albumartist` o, en su defecto, `artist`) |
| titulo | TEXT NOT NULL | |
| titulo_norm | TEXT NOT NULL | |
| anio | INTEGER NULL | Año de la etiqueta `date`/`year` |
| caratula_ruta | TEXT NULL | Ruta en caché de la carátula (`~/.cache/mmmusic/caratulas/{id}.jpg`) |
| creado_en | TEXT NOT NULL | |

Índice único: `(artista_id, titulo_norm)`.

**PISTAS**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| album_id | INTEGER FK → ALBUMES | |
| artista_id | INTEGER FK → ARTISTAS | Artista de la pista |
| titulo | TEXT NOT NULL | Etiqueta `title` o nombre de fichero sin extensión |
| titulo_norm | TEXT NOT NULL | |
| numero_pista | INTEGER NULL | |
| numero_disco | INTEGER NULL | Por defecto 1 |
| genero | TEXT NULL | |
| duracion_ms | INTEGER NOT NULL | |
| ruta | TEXT NOT NULL UNIQUE | Ruta absoluta |
| formato | TEXT NOT NULL | `mp3`, `flac`, `ogg`, `opus`, `m4a`, `wav` |
| tamano_bytes | INTEGER NOT NULL | |
| modificado_en | INTEGER NOT NULL | mtime Unix del fichero; junto con `tamano_bytes` decide si se relee |
| bitrate_kbps | INTEGER NULL | |
| anadido_en | TEXT NOT NULL | Primera vez que se indexó |
| escaneo_id | INTEGER FK → ESCANEOS | Último escaneo que la vio; las pistas no vistas en el escaneo completado se borran |

Índices: `(album_id, numero_disco, numero_pista)`, `(artista_id)`, `(titulo_norm)`, `(anadido_en)`.

**PLAYLISTS**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| nombre | TEXT NOT NULL UNIQUE | |
| creado_en | TEXT NOT NULL | |
| actualizado_en | TEXT NOT NULL | |

**PLAYLIST_PISTAS**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| playlist_id | INTEGER FK → PLAYLISTS ON DELETE CASCADE | |
| pista_id | INTEGER FK → PISTAS ON DELETE CASCADE | |
| posicion | INTEGER NOT NULL | Orden 0..n; único por `(playlist_id, posicion)` |

**COLA** (persistencia de la cola de reproducción)

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| pista_id | INTEGER FK → PISTAS ON DELETE CASCADE | |
| posicion | INTEGER NOT NULL UNIQUE | Orden de reproducción efectivo (ya barajado si `aleatorio` está activo) |
| posicion_orig | INTEGER NOT NULL | Orden original, para desactivar el modo aleatorio sin perder el orden |

**HISTORIAL_REPRODUCCION**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| pista_id | INTEGER FK → PISTAS ON DELETE CASCADE | |
| reproducido_en | TEXT NOT NULL | Instante de inicio |
| completada | INTEGER NOT NULL | 0/1; 1 si se reprodujo ≥ 50 % de la duración (regla reutilizable por el scrobbling de Fase 2) |

**ESCANEOS**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| iniciado_en | TEXT NOT NULL | |
| finalizado_en | TEXT NULL | |
| estado | TEXT NOT NULL | `en_curso`, `completado`, `error`, `cancelado` (sin CHECK, validado en código) |
| nuevas | INTEGER NOT NULL DEFAULT 0 | |
| actualizadas | INTEGER NOT NULL DEFAULT 0 | |
| eliminadas | INTEGER NOT NULL DEFAULT 0 | |
| error_msg | TEXT NULL | |

**AJUSTES** (clave-valor)

| Clave | Valor | Descripción |
|-------|-------|-------------|
| volumen | 0-100 | Último volumen |
| silencio | 0/1 | |
| aleatorio | 0/1 | |
| repeticion | `no`, `todo`, `una` | |
| cola_posicion | entero | Índice de la pista actual en COLA |
| cola_ms | entero | Posición dentro de la pista al cerrar (se restaura en pausa) |
| ancho_cola | entero | Ancho del panel de cola |

### Relaciones

- Un ARTISTA tiene muchos ÁLBUMES (como artista de álbum) y muchas PISTAS (como artista de pista).
- Un ÁLBUM tiene muchas PISTAS; una pista pertenece a exactamente un álbum (si no hay etiqueta `album`, se usa el nombre de la carpeta contenedora).
- PLAYLISTS ↔ PISTAS: N:M a través de PLAYLIST_PISTAS con posición.
- COLA y HISTORIAL_REPRODUCCION referencian PISTAS; al borrar una pista desaparecen de ambas.

### Diagrama de estados: ESCANEO

```
              iniciar()
   [inexistente] ──────────► en_curso
                                │
              ┌─────────────────┼─────────────────┐
              │ fin sin error   │ excepción       │ usuario cancela (Ctrl+r
              ▼                 ▼                 ▼  de nuevo / salir)
          completado          error            cancelado

Transiciones inválidas: cualquier salida de completado/error/cancelado
(un escaneo terminado no se reabre; se crea uno nuevo). Solo puede haber
un escaneo en_curso a la vez; si al arrancar existe uno en_curso huérfano,
se marca como error ("proceso interrumpido") antes de iniciar otro.
```

### Diagrama de estados: Reproductor

```
                        cargar(pista)
   ┌──────────┐ ────────────────────────► ┌──────────┐
   │ detenido │                           │ cargando │
   └──────────┘ ◄──────┐                  └────┬─────┘
        ▲              │ detener()             │ file-loaded        │ error mpv
        │              │                       ▼                    ▼
        │       ┌──────┴────────┐   pausar()  ┌─────────┐      ┌───────┐
        │       │ reproduciendo │ ──────────► │ pausado │      │ error │
        │       │               │ ◄────────── │         │      └───┬───┘
        │       └──────┬────────┘  reanudar() └────┬────┘          │ siguiente automático
        │              │ fin de pista (eof)        │ detener()     │ (si hay más en cola)
        │              ▼                           ▼               ▼
        │   ¿siguiente en cola? ──sí──► cargando(siguiente)   cargando(siguiente)
        │              │no (y repeticion=no)
        └──────────────┘

Transiciones inválidas: pausar() en detenido/cargando/error (se ignora);
reanudar() en reproduciendo (se ignora); seek() en detenido (se ignora).
El estado error es transitorio: se registra en log y notificación, se salta
la pista y se continúa; si tres pistas seguidas fallan, se pasa a detenido.
```

---

## 4. Flujos de Trabajo

### 4.1 Escaneo incremental de la biblioteca

```
  Ctrl+r / arranque con escanear_al_arrancar=true
        │
        ▼
  ¿Hay escaneo en_curso? ──sí──► cancelar el anterior, esperar a que termine
        │no
        ▼
  INSERT ESCANEOS(estado=en_curso) ──► notificar UI "Escaneando…"
        │
        ▼
  Para cada carpeta de config.biblioteca.carpetas:
    ¿existe y es legible? ──no──► registrar aviso, continuar con la siguiente
        │sí
        ▼
    walkdir (siguiendo enlaces simbólicos, ignorando ocultos)
      para cada fichero con extensión soportada:
        │
        ▼
      SELECT modificado_en, tamano_bytes FROM PISTAS WHERE ruta=?
        │
        ├─ no existe ─────────────────────────► leer etiquetas ──► upsert ARTISTA/ALBUM ──► INSERT PISTA (nuevas++)
        ├─ existe y mtime/tamaño iguales ─────► UPDATE escaneo_id (sin releer)
        └─ existe y difiere ──────────────────► leer etiquetas ──► UPDATE PISTA (actualizadas++)
        │
        │ (error al leer etiquetas → log WARN, fichero omitido, sigue)
        │ (commit cada 200 ficheros; evento de progreso a la UI cada 50)
        ▼
  DELETE FROM PISTAS WHERE escaneo_id <> este (eliminadas++)
  DELETE ALBUMES sin pistas; DELETE ARTISTAS sin álbumes ni pistas
        │
        ▼
  Carátulas: para cada ALBUM nuevo o sin caratula_ruta → extraer y cachear
        │
        ▼
  UPDATE ESCANEOS(estado=completado, finalizado_en, contadores)
        │
        ▼
  Notificar UI: "Escaneo: 120 nuevas, 3 actualizadas, 1 eliminada" y refrescar vista actual

  Camino de error: cualquier fallo de BD o de E/S no recuperable →
  UPDATE ESCANEOS(estado=error, error_msg) → notificación roja → log ERROR.
  Cancelación: el hilo comprueba un AtomicBool entre ficheros → estado=cancelado;
  lo indexado hasta entonces se conserva (commits parciales), no se ejecuta el DELETE.
```

### 4.2 Reproducir una pista desde una vista

```
  Enter sobre una pista (en álbum, artista, lista de pistas, búsqueda o playlist)
        │
        ▼
  Construir nueva cola con el CONTEXTO de la vista:
    - detalle de álbum       → todas las pistas del álbum, en orden
    - detalle de artista     → todas las pistas del artista (álbumes por año, luego disco/pista)
    - lista "Pistas"         → la lista tal como está filtrada/ordenada
    - playlist               → la playlist en orden
    - resultados de búsqueda → solo las pistas del bloque de resultados
        │
        ▼
  ComandoReproductor::ReemplazarCola { pistas, indice_inicial }
        │
        ▼
  Hilo reproductor: cola.reemplazar(); si aleatorio → barajar manteniendo la pista elegida
  la primera; persistir COLA y AJUSTES.cola_posicion
        │
        ▼
  mpv loadfile(ruta) ──► estado=cargando ──► evento file-loaded ──► estado=reproduciendo
        │                                        │
        │                                        └─► INSERT HISTORIAL_REPRODUCCION(completada=0)
        ▼
  watch<EstadoReproduccion> → UI actualiza barra inferior; MPRIS emite PropertiesChanged
        │
        ▼
  Cada 250 ms: tick → position/duration observadas → progreso
  Al superar el 50 % → UPDATE HISTORIAL completada=1 (una sola vez)
        │
        ▼
  eof-reached ──► cola.siguiente() según repeticion:
                  `una`  → misma pista
                  `todo` → siguiente, y al final vuelve a la 0
                  `no`   → siguiente, y al final estado=detenido

  Camino de error: fichero borrado/ilegible → mpv emite error → notificación
  "No se pudo reproducir <título>", se marca contador de fallos, se salta.
```

### 4.3 Cambio de tema de Omarchy en caliente

```
  omarchy-theme-set <tema>  (relinka ~/.config/omarchy/current/theme)
        │
        ▼
  Hilo Tema (notify sobre ~/.config/omarchy/current/) recibe evento
        │
        ▼
  Espera 300 ms (debounce) → lee <theme>/alacritty.toml
        │
        ├─ parse OK → construye Paleta → AppEvento::TemaActualizado(paleta)
        └─ error   → log WARN, conserva paleta anterior, notificación "Tema no legible"
        │
        ▼
  UI redibuja con la nueva paleta en el siguiente frame.
  Si el fichero no existe al arrancar (no es Omarchy) → paleta "terminal" (colores ANSI por defecto).
  `t` fuerza la recarga manual.
```

### 4.4 Diagrama de secuencia: play/pause desde una tecla multimedia (MPRIS)

```
  Hyprland        playerctl / Waybar      Hilo MPRIS (zbus)     Hilo Reproductor      libmpv         Hilo UI
     │ XF86AudioPlay      │                     │                     │                  │               │
     │───────────────────►│ PlayPause           │                     │                  │               │
     │                    │────────────────────►│ ComandoReproductor::│                  │               │
     │                    │                     │ AlternarPausa ─────►│ set pause=!pause │               │
     │                    │                     │                     │─────────────────►│               │
     │                    │                     │                     │◄─ evento pause ──│               │
     │                    │                     │                     │ estado.pausado=… │               │
     │                    │                     │  watch<Estado> ◄────┤────────────────────────────────►│ redibuja ⏸/▶
     │                    │◄── PropertiesChanged│                     │                  │               │
     │                    │   (PlaybackStatus)  │                     │                  │               │
```

### 4.5 Diagrama de secuencia: cambio de pista con carátula

```
  Hilo Reproductor        Hilo UI                       Caché imágenes (LRU)     Disco
     │ estado.pista=nueva     │                                │                    │
     │───────────────────────►│ ¿caratula_ruta en LRU?         │                    │
     │                        │───────────────────────────────►│                    │
     │                        │◄── no ─────────────────────────│                    │
     │                        │ hilo auxiliar: image::open + resize ───────────────►│ leer jpg
     │                        │◄─────────────────────────────────── DynamicImage ───│
     │                        │ LRU.insert(album_id, protocolo.encode(img))          │
     │                        │ redibuja barra inferior y detalle de álbum           │
  (mientras carga se muestra un marcador "♪" en el hueco de la carátula)
```

### 4.6 Descripción paso a paso: primera ejecución

1. `mmmusic` arranca; si no existe `~/.config/mmmusic/config.toml` lo crea con `carpetas = ["~/Music"]` y avisa en pantalla.
2. Abre/crea `~/.local/share/mmmusic/mmmusic.db`, aplica migraciones pendientes (`PRAGMA user_version`).
3. Carga el tema de Omarchy; si no existe, paleta de terminal.
4. Restaura COLA y AJUSTES (volumen, aleatorio, repetición, pista actual en pausa).
5. Lanza el hilo MPRIS y registra `org.mpris.MediaPlayer2.mmmusic`.
6. Si `escanear_al_arrancar = true` inicia el escaneo en segundo plano; la UI es usable desde el primer frame.
7. Muestra la vista Inicio.

---

## 5. Comandos, Atajos y API Interna

### 5.1 Atajos de teclado

**Globales**

| Tecla | Acción |
|-------|--------|
| `q` | Salir (guarda cola y ajustes) |
| `Ctrl+c` | Salir inmediato |
| `Esc` | Volver / cerrar panel o diálogo / salir del campo de búsqueda |
| `?` | Mostrar/ocultar ayuda |
| `1`–`6` | Ir a Inicio, Buscar, Artistas, Álbumes, Pistas, Playlists |
| `c` | Mostrar/ocultar panel de cola |
| `Tab` / `Shift+Tab` | Ciclar foco: sidebar → contenido → cola |
| `Ctrl+r` | Reescanear biblioteca (si ya hay uno en curso, lo cancela) |
| `t` | Recargar tema |

**Navegación (estilo vim)**

| Tecla | Acción |
|-------|--------|
| `j` / `k` / `↓` / `↑` | Bajar / subir |
| `h` / `l` / `←` / `→` | Izquierda / derecha en rejillas; en listas `h` vuelve y `l` entra |
| `gg` / `G` | Primer / último elemento |
| `Ctrl+d` / `Ctrl+u` | Media página abajo / arriba |
| `Enter` | Abrir (artista, álbum, playlist) o reproducir (pista) |

**Reproducción**

| Tecla | Acción |
|-------|--------|
| `Espacio` | Reproducir / pausar |
| `n` / `p` | Siguiente / anterior (anterior reinicia la pista si van > 3 s) |
| `x` | Detener |
| `,` / `.` | Retroceder / avanzar `salto_corto_s` (5 s) |
| `<` / `>` | Retroceder / avanzar `salto_largo_s` (30 s) |
| `+` / `-` | Volumen ±5 |
| `m` | Silenciar / restaurar |
| `s` | Aleatorio on/off |
| `r` | Repetición: no → todo → una → no |

**Cola y playlists**

| Tecla | Acción |
|-------|--------|
| `a` | Añadir elemento seleccionado (pista, álbum, artista, playlist) al final de la cola |
| `A` | Reproducir a continuación (insertar tras la pista actual) |
| `P` | Añadir elemento seleccionado a una playlist (abre selector) |
| `d` | Eliminar elemento seleccionado de la cola o de la playlist abierta |
| `J` / `K` | Mover elemento abajo / arriba en cola o playlist |
| `N` | Nueva playlist |
| `R` | Renombrar playlist seleccionada |
| `D` | Eliminar playlist seleccionada (con confirmación) |
| `e` | Exportar playlist seleccionada a M3U8 |
| `i` | Importar M3U/M3U8 como playlist nueva |
| `C` | Vaciar la cola (con confirmación) |

**Búsqueda**

| Tecla | Acción |
|-------|--------|
| `/` | Ir a Buscar y enfocar el campo |
| `Enter` (en campo) | Pasar el foco a los resultados |
| Escribir | Búsqueda incremental con debounce de 150 ms |

Los atajos se aplican solo cuando tienen sentido para el foco y el elemento seleccionado; si no, se ignoran sin error.

### 5.2 API interna entre hilos

`ComandoReproductor` (UI y MPRIS → reproductor):

| Comando | Parámetros | Efecto |
|---------|-----------|--------|
| ReemplazarCola | pistas: Vec<PistaId>, indice: usize | Nueva cola y reproduce `indice` |
| AnadirAlFinal | pistas | Anexa; si estaba detenido, reproduce la primera |
| ReproducirSiguiente | pistas | Inserta tras la actual |
| EliminarDeCola | posicion | |
| MoverEnCola | de, a | |
| VaciarCola | — | Detiene y vacía |
| SaltarA | posicion | Reproduce el elemento `posicion` de la cola |
| AlternarPausa / Reanudar / Pausar / Detener | — | |
| Siguiente / Anterior | — | |
| Buscar | ms: i64, relativo: bool | Seek |
| Volumen | valor 0-100 | |
| AlternarSilencio | — | |
| AlternarAleatorio | — | Rebaraja / restaura `posicion_orig` |
| CiclarRepeticion / FijarRepeticion | modo | |
| Apagar | — | Persiste y termina el hilo |

`EstadoReproduccion` (reproductor → UI y MPRIS, vía `watch`):

```
estado: Detenido | Cargando | Reproduciendo | Pausado
pista_actual: Option<PistaResumen>   (id, titulo, artista, album, album_id, duracion_ms, caratula_ruta, ruta)
posicion_ms, duracion_ms, volumen, silencio, aleatorio, repeticion
cola: Vec<PistaResumen>, cola_indice: Option<usize>
```

`AppEvento` (hacia UI): `Reproductor(EstadoReproduccion)`, `Escaneo(Progreso | Terminado | Error)`, `TemaActualizado(Paleta)`, `Notificacion(Nivel, String)`, `CaratulaLista(album_id)`, `Tecla(KeyEvent)`, `Redimension`, `Tick`.

### 5.3 Consultas de biblioteca (módulo `consultas`)

| Función | Descripción |
|---------|-------------|
| `listar_artistas(orden)` | Nombre + nº álbumes + nº pistas |
| `detalle_artista(id)` | Álbumes (por año desc) y pistas sueltas |
| `listar_albumes(orden, filtro_artista)` | Con carátula, artista, año, nº pistas |
| `detalle_album(id)` | Pistas ordenadas por disco/pista |
| `listar_pistas(orden, pagina)` | Paginado de 500 |
| `buscar(texto, limite)` | Artistas, álbumes y pistas por `*_norm LIKE %termino%` (cada término por separado, AND) |
| `inicio()` | Reproducidas recientemente (10), añadidas recientemente (10 álbumes), 10 álbumes al azar |
| `playlists::{listar, crear, renombrar, eliminar, pistas, anadir, quitar, mover, exportar_m3u, importar_m3u}` | |
| `cola::{cargar, guardar}` | |
| `historial::{registrar_inicio, marcar_completada}` | |
| `ajustes::{leer, escribir}` | |
| `escaneos::{iniciar, finalizar, ultimo}` | |

### 5.4 Interfaz MPRIS (`org.mpris.MediaPlayer2.mmmusic`)

| Interfaz | Miembro | Mapeo |
|----------|---------|-------|
| MediaPlayer2 | Identity = "mmmusic", CanQuit = true, CanRaise = false, DesktopEntry = "mmmusic" | |
| MediaPlayer2 | Quit() | ComandoReproductor::Apagar + salir |
| Player | PlaybackStatus | Playing / Paused / Stopped |
| Player | Metadata | mpris:trackid, mpris:length (µs), mpris:artUrl (`file://` caché), xesam:title, xesam:artist, xesam:album, xesam:url |
| Player | Position, Seeked (señal) | posicion_ms × 1000 |
| Player | Volume (0.0–1.0), Shuffle, LoopStatus (None/Playlist/Track) | Ajustes |
| Player | PlayPause, Play, Pause, Stop, Next, Previous, Seek(offset), SetPosition(trackid, pos) | Comandos equivalentes |
| Player | CanPlay/CanPause/CanSeek/CanGoNext/CanGoPrevious/CanControl | Según cola y estado |

---

## 6. Interfaz de Usuario

### Mapa de navegación

```
mmmusic
├── [1] Inicio
│     ├── Reproducidas recientemente ──► Enter: reproduce la pista (cola = las 10 recientes)
│     ├── Añadidos recientemente ───────► Enter: detalle de álbum
│     └── Redescubre (álbumes al azar) ─► Enter: detalle de álbum
├── [2] Buscar
│     └── Resultados: Artistas / Álbumes / Pistas ──► Enter: abrir o reproducir
├── [3] Artistas (lista)
│     └── Detalle de artista (álbumes en rejilla + pistas sueltas)
│           └── Detalle de álbum
├── [4] Álbumes (rejilla con carátulas)
│     └── Detalle de álbum (carátula grande + lista de pistas)
├── [5] Pistas (tabla ordenable: título, artista, álbum, duración, año)
├── [6] Playlists (lista)
│     └── Detalle de playlist (pistas ordenables)
├── [c] Panel de cola (columna derecha, superpuesto a cualquier vista)
├── [?] Ayuda (overlay)
└── Diálogos: confirmación (D, C), texto (N, R, e, i), selector de playlist (P)

Barra inferior: siempre visible en todas las vistas.
```

### Mockup 1: Inicio (80×24, Alacritty, carátulas en half-blocks)

```
┌ mmmusic ─────────┬──────────────────────────────────────────────────────────┐
│  1 Inicio      ◄ │ Reproducidas recientemente                               │
│  2 Buscar        │  ▶ Nocturne Drive         Midnight Premiere     4:12     │
│  3 Artistas      │    Neon Rain              ESPRIT 空想            3:48     │
│  4 Álbumes       │    Terminal Velocity      Monodrone             5:01     │
│  5 Pistas        │                                                          │
│  6 Playlists     │ Añadidos recientemente                                   │
│                  │  ▀▀▀▀  ▀▀▀▀  ▀▀▀▀  ▀▀▀▀  ▀▀▀▀                            │
│  Playlists       │  ▄▄▄▄  ▄▄▄▄  ▄▄▄▄  ▄▄▄▄  ▄▄▄▄                            │
│   Vaporwave      │  Virtua Dream  Mallsoft   Lost  Late  Signal              │
│   Trabajo        │  ESPRIT 空想   Monodrone  Mid.  Mid.  Mono.               │
│   Coche          │                                                          │
│                  │ Redescubre                                               │
│                  │  ▀▀▀▀  ▀▀▀▀  ▀▀▀▀  ▀▀▀▀  ▀▀▀▀                            │
│                  │  ▄▄▄▄  ▄▄▄▄  ▄▄▄▄  ▄▄▄▄  ▄▄▄▄                            │
│                  │  Album A       Album B    Alb.  Alb.  Alb.                │
│ 4.812 pistas     │                                                          │
│ escaneando… 62 % │                                                          │
├──────────────────┴──────────────────────────────────────────────────────────┤
│ ▀▀▀ Nocturne Drive                       ⏮  ⏸  ⏭      🔀 🔁 ♪ vol 80 %      │
│ ▄▄▄ Midnight Premiere · Late Night Tapes  1:27 ━━━━━━━●───────────── 4:12  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Mockup 2: Detalle de álbum

```
┌ mmmusic ─────────┬──────────────────────────────────────────────────────────┐
│  1 Inicio        │ ◄ Álbumes                                                │
│  2 Buscar        │  ▀▀▀▀▀▀▀▀▀▀                                              │
│  3 Artistas      │  ▄▄▄▄▄▄▄▄▄▄   Late Night Tapes                           │
│  4 Álbumes     ◄ │  ▀▀▀▀▀▀▀▀▀▀   Midnight Premiere · 2024 · 9 pistas · 38:04│
│  5 Pistas        │  ▄▄▄▄▄▄▄▄▄▄   FLAC                                       │
│  6 Playlists     │                                                          │
│                  │  #  Título                        Artista        Durac.  │
│  Playlists       │  1  Opening Credits               Midnight Prem.  2:10   │
│   Vaporwave      │▶ 2  Nocturne Drive                Midnight Prem.  4:12   │
│   Trabajo        │  3  Parking Lot Lights            Midnight Prem.  3:55   │
│   Coche          │  4  Interlude                     Midnight Prem.  1:02   │
│                  │  5  Aisle Seven                   Midnight Prem.  4:40   │
│                  │  6  Closing Time                  Midnight Prem.  5:12   │
│                  │                                                          │
│                  │                                                          │
│ 4.812 pistas     │ Enter reproducir · a a la cola · A a continuación · P playlist│
├──────────────────┴──────────────────────────────────────────────────────────┤
│ ▀▀▀ Nocturne Drive                       ⏮  ⏸  ⏭      🔀 🔁 ♪ vol 80 %      │
│ ▄▄▄ Midnight Premiere · Late Night Tapes  1:27 ━━━━━━━●───────────── 4:12  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Mockup 3: Buscar

```
┌ mmmusic ─────────┬──────────────────────────────────────────────────────────┐
│  1 Inicio        │ / mid_                                                   │
│  2 Buscar      ◄ │──────────────────────────────────────────────────────────│
│  3 Artistas      │ Artistas                                                 │
│  4 Álbumes       │  Midnight Premiere                          3 álbumes    │
│  5 Pistas        │  Midori                                     1 álbum      │
│  6 Playlists     │                                                          │
│                  │ Álbumes                                                  │
│  Playlists       │  Late Night Tapes          Midnight Premiere   2024      │
│   Vaporwave      │  Midtown Mall              ESPRIT 空想          2019      │
│   Trabajo        │                                                          │
│   Coche          │ Pistas                                                   │
│                  │  Midnight Cruise           Monodrone           3:33      │
│                  │  Mid-Morning Lounge        ESPRIT 空想          4:01      │
│                  │  Midway                    Midnight Premiere   2:58      │
│                  │                                            +12 más       │
│ 4.812 pistas     │                                                          │
├──────────────────┴──────────────────────────────────────────────────────────┤
│     (detenido)                           ⏮  ▶  ⏭      🔀 🔁 ♪ vol 80 %      │
│                                           --:-- ─────────────────── --:--   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Mockup 4: Vista Pistas con panel de cola abierto

```
┌ mmmusic ─────────┬─────────────────────────────────────┬────────────────────┐
│  1 Inicio        │ Título ▲       Artista      Durac.   │ Cola (7)           │
│  2 Buscar        │ Aisle Seven    Midnight P.   4:40    │ ▶ Nocturne Drive   │
│  3 Artistas      │ Closing Time   Midnight P.   5:12    │   Parking Lot L.   │
│  4 Álbumes       │ Interlude      Midnight P.   1:02    │   Interlude        │
│  5 Pistas      ◄ │ Lost Signal    Monodrone     6:20    │   Aisle Seven      │
│  6 Playlists     │ Mallsoft       ESPRIT 空想    3:12    │   Closing Time     │
│                  │ Midnight Cru.  Monodrone     3:33    │   Lost Signal      │
│  Playlists       │ Neon Rain      ESPRIT 空想    3:48    │   Mallsoft         │
│   Vaporwave      │ Nocturne Dr.   Midnight P.   4:12    │                    │
│   Trabajo        │ …                                    │                    │
│   Coche          │                                      │                    │
│                  │                                      │                    │
│                  │                                      │ d quitar · J/K     │
│ 4.812 pistas     │ 1-8 de 4.812        orden: título    │ mover · C vaciar   │
├──────────────────┴─────────────────────────────────────┴────────────────────┤
│ ▀▀▀ Nocturne Drive                       ⏮  ⏸  ⏭      🔀 🔁 ♪ vol 80 %      │
│ ▄▄▄ Midnight Premiere · Late Night Tapes  1:27 ━━━━━━━●───────────── 4:12  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Mockup 5: Diálogo de selección de playlist (`P`)

```
                 ┌ Añadir "Nocturne Drive" a… ─────────────┐
                 │  Vaporwave                    142 pistas │
                 │▶ Trabajo                       38 pistas │
                 │  Coche                         57 pistas │
                 │  + Nueva playlist…                       │
                 │                                          │
                 │  Enter añadir · Esc cancelar             │
                 └──────────────────────────────────────────┘
```

### Notas de UX / diseño

- **Layout**: sidebar de 18 columnas (mínimo 14), contenido flexible, barra inferior de 3 filas. Con menos de 70 columnas la sidebar se colapsa a iconos/números (`1`–`6`); con menos de 20 filas la barra inferior pasa a 2 filas sin carátula.
- **Paleta**: `primary.background/foreground` como fondo/texto; `normal.blue` como acento (selección, pista sonando); `normal.green` para progreso; `normal.yellow` para avisos; `normal.red` para errores; `bright.black` para texto secundario. Mapeo configurable en `[tema]`.
- **Iconos**: Nerd Font por defecto (`iconos = "nerd"`); `iconos = "ascii"` sustituye ⏮⏸⏭🔀🔁 por `|< || >| shuf rep`. Ningún estado se transmite solo por color: la pista actual lleva `▶`, el foco lleva `◄`.
- **Carátulas**: en Alacritty se renderizan en half-blocks (dos píxeles por celda); en Ghostty/Kitty a resolución real. Tamaños: 3×6 en barra inferior, 10×20 en detalle, 4×8 en rejillas. Decodificación en hilo auxiliar con caché LRU de 200 imágenes; mientras carga se muestra `♪`.
- **Rendimiento visible**: render bajo demanda (evento o tick de 250 ms), sin parpadeo; listas virtualizadas (solo se pintan las filas visibles).
- **Feedback**: notificaciones (toast) en la esquina superior derecha durante 3 s; barra de estado en la sidebar con nº de pistas y progreso de escaneo; ayuda contextual en la última línea del contenido.
- **Ratón**: clic para seleccionar y doble clic para abrir/reproducir; rueda para scroll; clic en la barra de progreso hace seek. Ratón es complemento, nunca requisito.

---

## 7. Lógica de Negocio

### 7.1 Normalización de texto

Se usa para deduplicar artistas/álbumes, ordenar y buscar:

```python
import unicodedata, re

def normalizar(texto: str) -> str:
    """minúsculas, sin diacríticos, espacios colapsados, sin espacios extremos."""
    t = unicodedata.normalize("NFKD", texto)
    t = "".join(c for c in t if not unicodedata.combining(c))
    t = t.lower()
    t = re.sub(r"\s+", " ", t).strip()
    return t

# normalizar("  Midnight  Première ") == "midnight premiere"
```

En Rust: `unicode-normalization` + filtro de marcas combinantes. Los caracteres CJK se conservan tal cual (la búsqueda "空想" debe encontrar "ESPRIT 空想").

### 7.2 Resolución de artista y álbum al indexar

```python
def resolver(etiquetas, ruta):
    titulo   = etiquetas.get("title") or nombre_sin_extension(ruta)
    artista  = etiquetas.get("artist") or "Artista desconocido"
    album_ar = etiquetas.get("albumartist") or artista
    album    = etiquetas.get("album") or nombre_carpeta(ruta)
    anio     = primeros_4_digitos(etiquetas.get("date") or etiquetas.get("year"))
    disco    = int_o_none(etiquetas.get("discnumber")) or 1
    pista    = int_o_none(etiquetas.get("tracknumber"))   # "3/12" → 3
    return titulo, artista, album_ar, album, anio, disco, pista
```

Clave de álbum: `(normalizar(album_ar), normalizar(album))`. Consecuencia conocida: un recopilatorio sin `albumartist` y con artistas distintos por pista se parte en varios álbumes; se documenta como limitación y se aborda en fases futuras (agrupación por carpeta).

### 7.3 Cola de reproducción

```python
class Cola:
    def __init__(self):
        self.items = []        # [(pista_id, posicion_orig)]
        self.indice = None
        self.aleatorio = False
        self.repeticion = "no" # "no" | "todo" | "una"

    def reemplazar(self, pistas, indice):
        self.items = [(p, i) for i, p in enumerate(pistas)]
        self.indice = indice
        if self.aleatorio:
            self._barajar_manteniendo_actual()

    def _barajar_manteniendo_actual(self):
        actual = self.items[self.indice]
        resto = [x for i, x in enumerate(self.items) if i != self.indice]
        random.shuffle(resto)
        self.items = [actual] + resto
        self.indice = 0

    def alternar_aleatorio(self):
        self.aleatorio = not self.aleatorio
        if self.aleatorio:
            self._barajar_manteniendo_actual()
        else:                      # restaurar orden original
            actual = self.items[self.indice]
            self.items.sort(key=lambda x: x[1])
            self.indice = self.items.index(actual)

    def siguiente(self):
        if self.indice is None: return None
        if self.repeticion == "una": return self.indice
        if self.indice + 1 < len(self.items): return self.indice + 1
        return 0 if self.repeticion == "todo" else None   # None → detenido

    def anterior(self, posicion_ms):
        if posicion_ms > 3000: return self.indice          # reiniciar la pista
        return max(self.indice - 1, 0)

    def reproducir_a_continuacion(self, pistas):
        base = (self.indice or -1) + 1
        nuevos = [(p, max(o for _, o in self.items) + 1 + i) for i, p in enumerate(pistas)]
        self.items[base:base] = nuevos
```

Reglas:

- `Anterior` con más de 3 s reproducidos reinicia la pista; con menos, va a la anterior (comportamiento Spotify).
- `Añadir al final` sobre una cola vacía y reproductor detenido inicia la reproducción.
- Al eliminar la pista que suena, se pasa a la siguiente; si era la última, se detiene.
- La cola se persiste en cada cambio (COLA + AJUSTES.cola_posicion); la posición dentro de la pista solo al salir con `q`.
- Al arrancar, la cola se restaura **en pausa** en la pista y posición guardadas; nunca empieza a sonar sola.

### 7.4 Historial y "completada"

- Se inserta una fila al recibir `file-loaded`.
- `completada = 1` cuando `posicion_ms >= duracion_ms * 0.5` (comprobado en cada tick, una sola vez por fila).
- Seeks no cuentan como reproducción: se usa el tiempo de reproducción acumulado, no la posición (`time-pos` menos saltos).

### 7.5 Búsqueda

- El texto se normaliza y se divide por espacios; cada término se aplica como `LIKE '%termino%'` sobre `*_norm` con AND entre términos.
- Límite de 20 resultados por bloque (artistas, álbumes, pistas) con indicador "+N más".
- Con menos de 2 caracteres no se consulta.

### 7.6 Importación / exportación M3U

- Exportación: M3U8 (UTF-8) con `#EXTM3U`, `#EXTINF:<segundos>,<artista> - <título>` y ruta absoluta; archivo en `~/Music/Playlists/<nombre>.m3u8` (configurable), sobreescribe con confirmación.
- Importación: acepta M3U y M3U8; rutas relativas se resuelven respecto al fichero; las líneas cuya ruta no esté en PISTAS se cuentan como "no encontradas" y se informan en la notificación ("24 pistas añadidas, 2 no encontradas"). Nombre de la playlist = nombre del fichero; si existe, se añade sufijo `(2)`.

### 7.7 Validaciones

- Nombre de playlist: 1–100 caracteres, no vacío tras `trim`, único (comparación normalizada).
- Carpetas de biblioteca: se expanden `~` y variables; carpetas inexistentes generan aviso, no error fatal.
- Volumen acotado 0–100; saltos acotados a `[0, duracion]`.
- Ficheros > 2 GB o sin duración detectable se omiten con WARN.

### 7.8 Casos especiales

- Ficheros con extensión válida pero corruptos: se omiten en escaneo (WARN) y, si están ya indexados y fallan al reproducir, se saltan y se marcan en la notificación.
- Enlaces simbólicos: se siguen; se detectan ciclos con el conjunto de inodos visitados.
- Carpetas en unidades desmontadas: al escanear, si la raíz no existe **no** se borran sus pistas (se detecta que la raíz entera desapareció y se salta el DELETE de esa raíz para evitar perder playlists).
- Cambio de tema mientras hay un diálogo abierto: el diálogo se redibuja con la nueva paleta.

---

## 8. Requisitos No Funcionales

### Seguridad

- Sin red saliente en Fase 1. Sin ejecución de comandos externos salvo `libmpv` (enlazada) y D-Bus.
- Rutas de la configuración canonicalizadas; la importación M3U no sigue rutas fuera de las carpetas de biblioteca (se ignoran con aviso) para no indexar ficheros arbitrarios.
- mpv se inicializa con `vo=null`, `audio-display=no`, `ytdl=no`, `load-scripts=no`, `config=no` para no heredar la configuración ni scripts Lua del usuario.
- MPRIS solo en el bus de sesión; sin métodos que reciban rutas del exterior más allá de `SetPosition(trackid)`, validado contra la cola.

### Protección de datos

- Todos los datos son locales del propio usuario (biblioteca, historial de reproducción). No se envía nada a terceros. El historial se puede vaciar borrando `mmmusic.db`. Sin base legal externa que documentar.

### Backups y recuperación

- `mmmusic.db` en modo WAL con `synchronous=NORMAL`; la caché de carátulas es regenerable.
- La biblioteca es siempre regenerable con un reescaneo; lo único no regenerable son PLAYLISTS e HISTORIAL. Exportación M3U como copia de seguridad manual de playlists.
- Migraciones idempotentes con `PRAGMA user_version`; nunca destructivas dentro de la fase.

### Rendimiento

- Objetivo: biblioteca de hasta 50.000 pistas. Escaneo inicial de 10.000 ficheros < 90 s en SSD; reescaneo sin cambios < 5 s.
- Arranque a UI usable < 300 ms (sin esperar al escaneo). Inicio de reproducción < 200 ms tras `Enter`.
- Uso de memoria < 150 MB con LRU de carátulas llena. Un solo usuario, sin concurrencia.
- Consultas de listados paginadas (500) e índices definidos en §3.

### Accesibilidad

- Toda la funcionalidad accesible por teclado; ratón opcional.
- Ningún estado transmitido solo por color (símbolos `▶`, `◄`, texto de estado).
- Contraste heredado del tema de Omarchy; con `tema.fuente = "terminal"` se respeta el esquema del emulador.
- Modo `iconos = "ascii"` para terminales sin Nerd Font.
- Todas las pantallas funcionan desde 60×18.

---

## 9. Integraciones

| Integración | Dirección | Detalle |
|-------------|-----------|---------|
| libmpv | salida de audio | Propiedades observadas: `pause`, `time-pos`, `duration`, `volume`, `mute`, `eof-reached`, `idle-active`; eventos `file-loaded`, `end-file`; `gapless-audio=yes` |
| MPRIS / D-Bus | bidireccional | Ver §5.4. Compatible con Waybar (`custom/mediaplayer`), `playerctl` y los binds `XF86Audio*` de Hyprland |
| Tema Omarchy | entrada | `~/.config/omarchy/current/theme/alacritty.toml` (secciones `[colors.primary]`, `[colors.normal]`, `[colors.bright]`); watcher con `notify` |
| M3U / M3U8 | entrada / salida | Ver §7.6 |
| Carátulas | entrada | Imagen embebida (primera pista del álbum que la tenga) o `cover.*`, `folder.*`, `front.*` en la carpeta del álbum; redimensionada a 300×300 JPEG en caché |
| Fichero de escritorio | salida | `mmmusic.desktop` (Terminal=true) para que `DesktopEntry` de MPRIS y los lanzadores lo encuentren; se instala con `cargo install` + script |

Configuración de referencia (`config.ejemplo.toml`):

```toml
[biblioteca]
carpetas = ["~/Music"]
escanear_al_arrancar = true
carpeta_playlists = "~/Music/Playlists"

[reproductor]
volumen_inicial = 80
salto_corto_s = 5
salto_largo_s = 30

[interfaz]
caratulas = true
iconos = "nerd"          # "nerd" | "ascii"
ancho_sidebar = 18

[tema]
fuente = "omarchy"       # "omarchy" | "terminal"
acento = "blue"          # nombre de color ANSI del tema
progreso = "green"
```

---

## 10. Decisiones Técnicas (ADR-lite)

**DT-1 — Rust + ratatui frente a Python + Textual.** Binario único sin runtime, arranque instantáneo y coherencia con el resto de herramientas de Omarchy. Textual habría acelerado el layout, pero añade Python + venv a una máquina de escritorio y peor rendimiento en listas grandes.

**DT-2 — libmpv (crate `libmpv2`) frente a decodificación integrada (symphonia + rodio).** mpv resuelve gapless, todos los contenedores, resample y salida PipeWire sin esfuerzo; symphonia obligaría a implementar gapless y cola de salida a mano. Coste: dependencia de sistema `mpv` (ya presente en Omarchy) y enlazado dinámico. Descartado el socket IPC de mpv por latencia y por tener que gestionar un proceso hijo.

**DT-3 — SQLite como índice persistente frente a reescaneo en memoria.** Con decenas de miles de pistas el reescaneo completo tarda decenas de segundos; el índice permite arranque instantáneo y guarda playlists, cola e historial en el mismo fichero. `rusqlite` con `bundled` para no depender de la versión de SQLite del sistema.

**DT-4 — Búsqueda con `LIKE` sobre columnas normalizadas frente a FTS5.** Para bibliotecas personales (< 100k filas) `LIKE` sobre índices es suficiente y evita tokenizadores y sincronización de tablas virtuales. FTS5 queda como opción si el rendimiento no cumple.

**DT-5 — Hilos nativos + canales frente a runtime async global.** La UI y el reproductor son bucles síncronos; solo MPRIS (zbus) necesita async y vive en su propio runtime tokio. Evita contaminar toda la app con `async` y el problema de `libmpv` no siendo `Send`-friendly en contextos async.

**DT-6 — Tema desde `alacritty.toml` del tema activo frente a variables de entorno o `colors.toml`.** Es el fichero que todos los temas de Omarchy garantizan y su formato es TOML estable; el usuario usa Alacritty, por lo que la coincidencia con su terminal es exacta. Si en el futuro Omarchy expone una fuente canónica de colores se añadirá como primera opción.

**DT-7 — Carátulas vía `ratatui-image` con detección automática de protocolo.** Un solo código para Kitty/Sixel/iTerm2/half-blocks; en Alacritty el resultado es de baja resolución pero funcional. Descartado renderizar solo texto: la rejilla de álbumes es parte esencial del layout Spotify.

**DT-8 — Playlists propias en SQLite con M3U como formato de intercambio.** Los ficheros M3U como única fuente de verdad complican el reordenado y la referencia estable a pistas; la BD permite posiciones y borrado en cascada. M3U cubre portabilidad.

**DT-9 — Cola restaurada en pausa al arrancar.** Evita que el reproductor arranque sonando sin intervención del usuario (molesto en un TUI que se abre desde scripts o al inicio de sesión).

**DT-10 — Identificadores de código en castellano, snake_case, y tablas en `MAYUSCULAS_SNAKE_CASE`.** Convención de casa de 4d3; los nombres de crates y términos técnicos permanecen en inglés.

---

## 11. Plan de Desarrollo

| Sprint | Contenido | Entregable verificable |
|--------|-----------|------------------------|
| S1 — Cimientos | Cargo, estructura, config XDG, logs, BD + migraciones, escáner incremental con lofty, tema Omarchy (lectura + watcher), esqueleto de UI con sidebar y barra inferior vacía | `mmmusic` arranca, escanea `~/Music`, muestra nº de pistas y usa los colores del tema |
| S2 — Reproducción | Hilo reproductor con libmpv2, cola con aleatorio/repetición, persistencia de cola y ajustes, barra inferior completa, atajos de reproducción, vista Pistas | Se puede reproducir toda la biblioteca desde la tabla de pistas con controles y progreso |
| S3 — Navegación | Vistas Artistas (lista + detalle), Álbumes (rejilla + detalle), Inicio, navegación vim, panel de cola, historial | Layout Spotify completo navegable |
| S4 — Buscar, playlists y carátulas | Búsqueda incremental, playlists CRUD + selector + M3U, carátulas (extracción, caché, render) | Flujo completo de organización de música |
| S5 — Escritorio y pulido | MPRIS, `.desktop`, ratón, ayuda, responsive de terminal pequeña, diálogos, tests de integración, README | Integración con Waybar/playerctl y suite de tests verde |

**Estimación:** 5 sprints de ~1 semana → 5–6 semanas.

Supuestos: desarrollo con Claude Code en sesiones de 2–3 h, 3–4 sesiones por semana; Hector valida en su Omarchy real al final de cada sprint; los crates indicados están disponibles en crates.io en versiones estables; `ratatui-image` y `libmpv2` no requieren parches. Riesgo de desviación: compatibilidad de `libmpv2` con la versión de mpv de Arch (si falla, alternativa IPC en S2, +3 días).

---

## 12. Conexiones con Otras Fases

- **Fase 2 (Scrobbling Last.fm / ListenBrainz):** reutiliza HISTORIAL_REPRODUCCION y la regla `completada` (≥ 50 %); añadirá tabla de scrobbles pendientes y configuración de credenciales.
- **Fase 3 (Letras y ecualizador):** el ecualizador usa filtros `af` de mpv sobre el mismo envoltorio `reproductor/mpv.rs`; letras se asocian a PISTAS.
- **Fase 4 (Radio / streams por URL):** el modelo de cola deberá admitir elementos que no sean PISTAS (URL); DT-2 (mpv) lo hace trivial en el lado de audio.
- **Fase 5 (Integración Nicotine+ / Soulseek):** la carpeta de descargas de Nicotine+ se añade como raíz y el escaneo incremental se dispara con `notify` al terminar una descarga.
- **Limitación heredada:** recopilatorios sin `albumartist` (§7.2) — candidata a corrección en Fase 2 o 3.
