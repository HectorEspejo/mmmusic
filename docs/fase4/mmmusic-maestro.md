# mmmusic - Informe Maestro

**Última actualización:** 13 de septiembre de 2026 (Fase 4 especificada)

## 1. Visión Global

mmmusic es un reproductor de música para terminal (TUI) diseñado para Omarchy (Arch Linux + Hyprland). Reproduce la biblioteca local del usuario con la disposición de pantalla de Spotify / Apple Music (sidebar de navegación, contenido central, barra inferior de "sonando ahora") y atajos estilo vim. Es un binario único en Rust que usa mpv como motor de audio, SQLite como índice de biblioteca y se integra con el escritorio a través de MPRIS, el sistema de temas de Omarchy y carátulas renderizadas en terminal.

Es un proyecto personal de Hector, sin cliente externo, con vocación de sustituir a Cliamp como reproductor diario. No contempla servicios de streaming: la única fuente es la música local.

## 2. Mapa de Módulos

```
┌───────────────────────────────────────────────────────────────────────────┐
│ FASE 1 — Reproductor Base                                                 │
│                                                                           │
│  ┌──────────┐   ┌────────────┐   ┌──────────────┐   ┌──────────────────┐  │
│  │ Config   │──►│ Biblioteca │──►│ Reproductor  │──►│ MPRIS            │  │
│  │ Tema     │   │ escáner    │   │ libmpv, cola │   │ D-Bus sesión     │  │
│  └────┬─────┘   │ SQLite     │   └──────┬───────┘   └──────────────────┘  │
│       │         │ carátulas  │          │                                 │
│       │         └─────┬──────┘          │                                 │
│       ▼               ▼                 ▼                                 │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │ UI ratatui: Inicio · Buscar · Artistas · Álbumes · Pistas ·         │  │
│  │ Playlists · Cola · Ayuda · Barra inferior                           │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────┬────────────────────────────────────────────┘
                               │ depende de
       ┌───────────────────────┼───────────────────────┬──────────────────┬──────────────────┐
       ▼                       ▼                       ▼                  ▼                  ▼
┌──────────────┐   ┌──────────────────────┐   ┌──────────────────────┐   ┌──────────────────────┐   ┌─────────────────┐
│ FASE 2 ✔     │   │ FASE 3 ✔             │   │ FASE 4               │   │ FASE 5 ✔             │   │ FASE 6          │
│ Scrobbling,  │   │ Visuales reactivas   │   │ Ecualizador + letras │   │ Radio y streams      │   │ Nicotine+       │
│ recopilat.,  │   │ (PipeWire, FFT,      │   │ (mpv af, .lrc/USLT)  │   │ (cola mixta, ICY,    │   │ (escaneo notify)│
│ favoritas    │   │  canvas braille)     │   └──────────────────────┘   │  Radio Browser)      │   └─────────────────┘
└──────────────┘   └──────────────────────┘                              └──────────────────────┘
```

## 3. Tabla Resumen de Fases

| Fase | Módulo | Funcionalidades | Sprints | Semanas | Estado |
|------|--------|-----------------|---------|---------|--------|
| 1 | Reproductor Base | 112 | 5 | 5-6 | Completada (validada 12/09/2026) |
| 2 | Scrobbling, Recopilatorios y Favoritas | 59 | 4 | 3-4 | Completada (validada 12/09/2026) |
| 3 | Visuales Reactivas | 59 | 4 | 3-4 | Completada (validada 13/09/2026) |
| 4 | Ecualizador y Letras | 50 | 4 | 3-4 | Especificada |
| 5 | Radio y Streams | 60 | 4 | 3-4 | Completada (validada 13/09/2026) |
| **Total** | | **340** | **21** | **~5,5 meses** | |

## 4. Modelo de Datos Global

```
                        [F1]                    [F1]                    [F1]
┌──────────────┐        ┌──────────────┐        ┌──────────────────┐
│  ARTISTAS    │1      *│  ALBUMES     │1      *│  PISTAS          │
│──────────────│────────│──────────────│────────│──────────────────│
│ id           │        │ id           │        │ id               │
│ nombre       │        │ artista_id   │        │ album_id         │
│ nombre_norm  │        │ titulo       │        │ artista_id       │◄── artista de pista
│ creado_en    │        │ titulo_norm  │        │ titulo           │
└──────────────┘        │ anio         │        │ titulo_norm      │
                        │ caratula_ruta│        │ numero_pista     │
                        │ creado_en    │        │ numero_disco     │
                        │ varios_art.  │[F2]    │ genero           │
                        │ colores      │[F3]    │                  │
                        │ carpeta      │[F2]    │ carpeta          │[F2]
                        └──────────────┘        │ art_alb_etiq.    │[F2]
                                                │ duracion_ms      │
                                                │ ruta             │
                                                │ formato          │
                                                │ tamano_bytes     │
                                                │ modificado_en    │
                                                │ bitrate_kbps     │
                                                │ anadido_en       │
                                                │ escaneo_id       │───► ESCANEOS [F1]
                                                └───┬────┬────┬────┘      id, iniciado_en,
                                                    │    │    │           finalizado_en, estado,
              ┌─────────────────────────────────────┘    │    │           nuevas, actualizadas,
              │*                                         │*   │*          eliminadas, error_msg
    ┌─────────┴─────────┐ [F1]     ┌──────────────────┐ [F1] ┌┴───────────────────────┐ [F1]
    │ PLAYLIST_PISTAS   │          │ COLA             │      │ HISTORIAL_REPRODUCCION │
    │ id, playlist_id,  │          │ id, pista_id,    │      │ id, pista_id,          │
    │ pista_id, posicion│          │ posicion,        │      │ reproducido_en,        │
    └─────────┬─────────┘          │ posicion_orig    │      │ completada             │
              │*                   └──────────────────┘      └────────────────────────┘
    ┌─────────┴─────────┐ [F1]     ┌──────────────────┐ [F1]
    │ PLAYLISTS         │          │ AJUSTES          │
    │ id, nombre,       │          │ clave, valor     │
    │ creado_en,        │          └──────────────────┘
    │ actualizado_en    │
    └───────────────────┘
    ┌───────────────────┐ [F2]     ┌────────────────────────────────┐ [F2]
    │ FAVORITAS         │          │ ENVIOS                         │
    │ id, pista_id (ú), │          │ id, servicio, tipo, pista_id,  │
    │ marcada_en        │          │ historial_id, reproducido_en,  │
    └───────────────────┘          │ estado, intentos,              │
        ▲ *→1 PISTAS               │ proximo_intento_en, error_msg, │
                                   │ creado_en, enviado_en,         │
                                   │ emisora_id [F5]                │
                                   └────────────────────────────────┘
                                       *→1 PISTAS (NULL adm. desde F5), *→1 HISTORIAL_REPRODUCCION
    [F5] COLA + tipo, emisora_id · HISTORIAL_REPRODUCCION + emisora_id, titulo_icy · pista_id NULL admisible

    ┌────────────────────────┐ [F5]   ┌──────────────────────┐ [F5]   ┌──────────────────────┐ [F5]
    │ EMISORAS               │1     *│ EMISORA_TITULOS      │        │ BUSQUEDAS_RADIO      │
    │ id, nombre, nombre_norm│───────│ id, emisora_id,      │        │ clave, respuesta_json│
    │ url (ú), pagina_web,   │        │ titulo, visto_en     │        │ obtenido_en          │
    │ pais, etiquetas, codec,│        └──────────────────────┘        └──────────────────────┘
    │ bitrate_kbps, logo_url,│
    │ logo_ruta, radiobrowser│
    │ _uuid (ú), favorita,   │
    │ anadida_en, ultima_    │
    │ reproduccion,          │
    │ ultimo_error           │
    └────────────────────────┘
        1→* COLA, HISTORIAL_REPRODUCCION, ENVIOS (emisora_id)

    ┌────────────────────────┐ [F4]   ┌──────────────────────┐ [F4]
    │ PRESETS_EQ             │        │ LETRAS               │
    │ id, nombre, nombre_norm│        │ pista_id (PK → PISTAS│
    │ (ú), ganancias,        │        │  cascada), offset_ms,│
    │ preamp_db, integrado,  │        │ fuente_preferida,    │
    │ creado_en              │        │ actualizado_en       │
    └────────────────────────┘        └──────────────────────┘
```

## 5. Glosario de Dominio

| Término de negocio | Entidad/Tabla | Fase | Notas |
|--------------------|---------------|------|-------|
| Artista | ARTISTAS | 1 | Deduplicado por `nombre_norm` |
| Artista de álbum | ALBUMES.artista_id | 1 | `albumartist` o, en su defecto, `artist` |
| Álbum | ALBUMES | 1 | Único por (artista_id, titulo_norm) |
| Pista / canción | PISTAS | 1 | Un fichero de audio; única por `ruta` |
| Carátula | ALBUMES.caratula_ruta | 1 | Fichero JPEG 300×300 en caché |
| Biblioteca | Carpetas de `config.biblioteca.carpetas` | 1 | Raíces del escaneo |
| Escaneo | ESCANEOS | 1 | Con ciclo de vida en_curso → completado/error/cancelado |
| Playlist | PLAYLISTS + PLAYLIST_PISTAS | 1 | Propias; M3U8 solo como intercambio |
| Cola | COLA + AJUSTES.cola_posicion | 1 | Persistida; se restaura en pausa |
| Historial / reproducida | HISTORIAL_REPRODUCCION | 1 | `completada` al 50 % (base del scrobbling F2) |
| Ajuste | AJUSTES | 1 | Estado del reproductor persistido |
| Tema | (config `[tema]`, fichero de Omarchy) | 1 | No es tabla |
| Recopilatorio / Varios artistas | ALBUMES.varios_artistas + ARTISTAS "Varios artistas" | 2 | Grupo (carpeta, álbum) sin `albumartist` y con artistas distintos |
| Escucha / scrobble | HISTORIAL_REPRODUCCION + ENVIOS (tipo scrobble) | 2 | `completada` con umbral min(50 %, 4 min), pista > 30 s |
| Now playing | (no persistido) | 2 | Envío inmediato sin cola |
| Envío | ENVIOS | 2 | Ciclo pendiente → enviado / error / descartado |
| Favorita / love | FAVORITAS + ENVIOS (tipo love/unlove) | 2 | Sincronizada solo con Last.fm |
| Credenciales | `credenciales.toml` (600) | 2 | No es tabla; no se respalda con la BD |
| Visual | (código, `AJUSTES.visual_actual`) | 3 | Espectro, Barras y ondas, Ambiente, Partículas, Caleidoscopio, Túnel |
| Captura | (hilo, `EstadoCaptura`) | 3 | Stream de PipeWire sobre el nodo "mmmusic" |
| Paleta | (tema de Omarchy o `ALBUMES.colores`) | 3 | 5 colores dominantes de la carátula |
| Pulso | (`Analisis.pulso`) | 3 | Beat por energía de graves |
| Emisora | EMISORAS | 5 | Stream por URL, favorita, datos del directorio |
| Elemento de cola | COLA.tipo (pista / emisora) | 5 | La cola es mixta desde F5 |
| Título ICY | EMISORA_TITULOS + HISTORIAL_REPRODUCCION.titulo_icy | 5 | "Artista - Título"; historial solo con artista |
| Stream (estado) | (`EstadoStream`) | 5 | conectando → almacenando → en_directo → reconectando → rendido |
| Directorio | Radio Browser + BUSQUEDAS_RADIO | 5 | Búsqueda con caché 24 h |
| Logo | EMISORAS.logo_ruta | 5 | Mismo pipeline que las carátulas |
| Preset de EQ | PRESETS_EQ | 4 | Integrados (no editables) y propios (♦) |
| Banda | (`EstadoEq.ganancias`, 10 bandas ISO) | 4 | 31 Hz – 16 kHz, −12..12 dB |
| Letra | (`Letra`: sincronizada / estática / ninguna) | 4 | Fuentes locales: fichero `.lrc`/`.txt` o etiqueta embebida |
| Offset de letra | LETRAS.offset_ms | 4 | Por pista, se suma al `[offset:]` del LRC |
| ReplayGain | (`replaygain_modo`) | 4 | Nativo de mpv, por etiquetas |

## 6. Estado del Proyecto

| Fase | Progreso según checklist | Último informe de implementación procesado |
|------|--------------------------|--------------------------------------------|
| 1 | 112 / 112 | 12/09/2026 (sesión 1, S1–S5); validaciones manuales confirmadas por Hector el 12/09/2026 |
| 2 | 59 / 59 | 12/09/2026 (sesión 1, S1–S4); validación con servicios reales confirmada por Hector el 12/09/2026 |
| 3 | 59 / 59 | 12/09/2026 (sesión 1, S1–S4); validación en Omarchy confirmada por Hector el 13/09/2026 |
| 4 | 0 / 50 | ninguno — sin confirmar |
| 5 | 60 / 60 | 13/09/2026 (sesión 1, S1–S4); validación con red real confirmada por Hector el 13/09/2026 |

**Progreso total:** 290 / 340 funcionalidades.

Pendiente de decisión (tres fases seguidas): el agente crea `docs/faseN/`; `CLAUDE.md` dice `docs/` plano. Si no se indica lo contrario, en la siguiente fase se adoptará `docs/faseN/` en `CLAUDE.md` para que la documentación siga a la realidad.

## 7. Roadmap

| Fase | Contenido | Depende de |
|------|-----------|-----------|
| 6 | Integración con Nicotine+ (carpeta de descargas como raíz, reescaneo automático con `notify`) | F1 (escáner incremental), F2 (consolidación) |
| 7 | Estadísticas y resumen anual sobre HISTORIAL_REPRODUCCION | F2, F5 |
| 8 | Modo remoto (MPD-compatible o API HTTP + cliente web) | F1 (MPRIS), red |
| 9 | Empaquetado y distribución (AUR, CI en Codeberg) | Modelo estable (post-F5) |
| — | Ideas sin fase: letras online (LRCLIB como `FuenteLetras`), ReplayGain calculado en el escaneo (EBU R128), presets de EQ por álbum, karaoke por palabra, edición de etiquetas, podcasts (RSS), grabación de streams, emisoras en Inicio, espectro espejado, "sunset grid" vaporwave, carátula pulsante, exportar frame de visual, mini visual en Waybar, arranque perezoso de la captura (R-15) | |

Orden recomendado tras la 4: 6 → 7 → 8 → 9. Idea añadida en F2: sincronizar favoritas con ListenBrainz si algún día la biblioteca lleva MBIDs (Picard).

## 8. Decisiones Transversales y Riesgos Abiertos

### Decisiones transversales

| Id | Decisión | Origen | Afecta a |
|----|----------|--------|----------|
| T-1 | mpv (libmpv2) es el único motor de audio; toda funcionalidad de audio futura (EQ, streams) se implementa sobre `reproductor/mpv.rs` | F1 DT-2 | F3, F4 |
| T-2 | Hilos nativos + canales; async solo en el hilo MPRIS (tokio). Ningún módulo nuevo introduce un runtime async global | F1 DT-5 | Todas |
| T-3 | SQLite `bundled`, WAL, migraciones por `PRAGMA user_version`, sin `CHECK` en enumeraciones (convención 4d3) | F1 DT-3 | Todas |
| T-4 | Texto normalizado (`*_norm`) es la clave de deduplicación, orden y búsqueda | F1 §7.1 | Todas |
| T-5 | El reproductor nunca arranca sonando por sí solo (cola restaurada en pausa) | F1 DT-9 | F4, F5 |
| T-6 | Identificadores de código en castellano snake_case; tablas en `MAYUSCULAS_SNAKE_CASE`; crates y términos técnicos en inglés | F1 DT-10 | Todas |
| T-7 | Crate como librería + binario fino; todo módulo nuevo se expone en `lib.rs` para ser testeable desde `tests/` | F1 impl. DT-11 | Todas |
| T-8 | Cada hilo abre su conexión SQLite y aplica migraciones al abrirla (idempotente) | F1 impl. | Todas |
| T-9 | Estado gráfico (`Picker`, protocolos) solo en el hilo de UI; los hilos auxiliares entregan `DynamicImage` | F1 impl. DT-12 | Todas las que rendericen imágenes |
| T-10 | Tiempo de escucha real por acumulación de deltas de `time-pos` < 1,5 s; los seeks no cuentan | F1 impl. DT-13 | F2 (scrobbling) |
| T-11 | `ratatui-image` sin feature `chafa`; las únicas dependencias de sistema son `mpv` y `pkgconf` | F1 impl. DT-7 | Todas |
| T-12 | **(modificada en F5)** Toda red saliente del código de mmmusic pasa por `red/cliente.rs` (`ureq` rustls, timeout, User-Agent) con lista cerrada de hosts: ListenBrainz, Last.fm y los espejos de Radio Browser; única excepción, la descarga de logos de emisora (cualquier https, solo `image/*`, ≤ 512 KB, 5 s, 3 redirecciones, tratados como no confiables). Los streams de audio los abre mpv, no el cliente propio | F2 DT-15, F5 DT-37/39 | F5+ |
| T-13 | Secretos solo en `credenciales.toml` (600), nunca en `config.toml`, logs, toasts ni informes | F2 DT-18 | F3+ |
| T-14 | Cola genérica ENVIOS (`servicio`, `tipo`) para cualquier envío diferido a servicios externos | F2 DT-17 | F3+ |
| T-15 | Releer ficheros tras una migración se hace por bandera en AJUSTES + escaneo completo, nunca dentro del SQL de migración | F2 DT-20 | Todas |
| T-16 | Todo texto que sale a logs, toasts o `error_msg` pasa por `Credenciales::redactar()`; los clientes HTTP comprueban `url_permitida()` y usan `http_status_as_error(false)` para clasificar por código | F2 impl. DT-23 | F3+ |
| T-17 | Entidades virtuales de la UI (pseudo-playlists y similares) se representan con ids centinela negativos, nunca con filas en la base | F2 impl. DT-22 | F3+ |
| T-18 | El planificador de reintentos y la clasificación de respuestas viven en un módulo propio sin E/S (`scrobbling/planificador.rs`), testeable sin red; cualquier integración nueva lo reutiliza | F2 impl. | F3+ |
| T-19 | El audio se obtiene de PipeWire capturando el nodo propio de mmmusic (`audio-client-name`), nunca decodificando en paralelo; toda fuente de audio alternativa alimenta el mismo `Anillo` | F3 DT-25 | F4+ |
| T-20 | Los callbacks de tiempo real (PipeWire `process`, libmpv wakeup) nunca bloquean ni asignan; comunican por estructuras sin bloqueo | F3 DT-27 | Todas |
| T-21 | Trabajo por frame solo cuando hay consumidor visible; ticks rápidos (33 ms) se activan bajo demanda y el tick base sigue en 250 ms | F3 DT-26 | F4+ |
| T-22 | Las visuales y efectos son reinterpretaciones propias con nombres genéricos en castellano; nunca se copian diseños, nombres ni recursos de productos existentes | F3 DT-30 | Todas |
| T-23 | En PipeWire, el nombre de aplicación y el PID de un stream de libmpv están en el global `Client`, no en el `Node` (que se llama "mpv"): cualquier localización de nodos propios se hace por `client.id` | F3 impl. desviación 2 | F4+ |
| T-24 | Estructuras compartidas con hilos de tiempo real: atómicos por elemento sin `unsafe`; callbacks con `try_borrow_mut` tolerantes a reentrada; comandos y plazos atendidos por timer del propio bucle | F3 impl. DT-32/33/34 | Todas |
| T-25 | Los valores por defecto que dependen de la configuración se siembran por código en AJUSTES, nunca con valores fijos en el SQL de una migración | F3 impl. desviación 3 | Todas |
| T-26 | La cola, el historial y los envíos admiten elementos que no son pistas (`ElementoCola`); exactamente una referencia (`pista_id` o `emisora_id`) por fila, validada en código | F5 DT-36 | F6+ |
| T-27 | Recrear una tabla en una migración (para quitar `NOT NULL` u otros cambios que SQLite no permite con ALTER) se hace en transacción, con `INSERT … SELECT` y copia previa de la BD | F5 DT-41 | Todas |
| T-28 | Una "escucha" es una canción reconocida (pista local o título ICY con artista), nunca una conexión ni un fichero sin identificar | F5 DT-40 | F6+ |
| T-29 | Toda migración corre con `foreign_keys = OFF` y termina con `PRAGMA foreign_key_check` antes de confirmar; recrear una tabla referenciada con claves activas dispara acciones `ON DELETE` en las hijas | F5 impl. DT-43 | Todas |
| T-30 | La allowlist de red se comprueba en un único punto (`red/cliente.rs`) para todas las peticiones; ningún cliente construye su propio `ureq` | F5 impl. DT-46 | Todas |
| T-31 | Los filtros de audio de mpv se instalan una vez como cadena etiquetada y se ajustan con `af-command`; reconstruir la cadena solo cuando no hay comando en caliente | F4 DT-47 | F6+ |
| T-32 | mmmusic nunca escribe en la biblioteca del usuario (ficheros de audio, `.lrc`); los ajustes derivados (offsets, colores, presets) van a la base de datos o a la carpeta de datos | F4 DT-52 (y F2/F3) | Todas |
| T-33 | Fuentes de datos externas a la biblioteca (letras, y en el futuro otras) se modelan como traits enchufables con resolución por orden y caché | F4 DT-51 | F6+ |
| T-34 | La documentación de fase vive en `docs/faseN/` (adoptado por la realidad del repositorio) | F4 DT-54 | Todas |

### Riesgos abiertos

| Id | Riesgo | Impacto | Mitigación |
|----|--------|---------|-----------|
| ~~R-1~~ | Cerrado 12/09/2026: `libmpv2` enlaza y reproduce con mpv 0.41 | — | — |
| R-2 | Carátulas en half-blocks en Alacritty resultan poco atractivas para el usuario | Estético | Probar Ghostty/Kitty; opción `caratulas = false`; reducir tamaño de tarjetas |
| R-3 | Formato del tema de Omarchy cambia en versiones futuras (deja de haber `alacritty.toml` o cambia su estructura) | Pierde colores del sistema | Fallback a paleta terminal ya previsto; añadir otras fuentes (`colors.toml`) cuando existan |
| R-4 | Recopilatorios sin `albumartist` se fragmentan en varios álbumes | Calidad del índice | Documentado; corrección en F2 |
| R-5 | `ratatui-image` con protocolo Kitty y muchas tarjetas simultáneas puede ser lento | Rendimiento en rejillas | Limitado a tarjetas visibles + LRU; pendiente de medir en Ghostty/Kitty (el desarrollo se validó en half-blocks) |
| ~~R-6~~ | Cerrado 12/09/2026: validaciones manuales de F1 confirmadas | — | — |
| R-7 | El agente no puede probar contra ListenBrainz/Last.fm reales; los formatos de petición se validan solo con tests sin red | Errores de integración descubiertos solo por Hector | `probar-servicios` y tests con vectores conocidos de firma; ajuste como desviación si falla |
| R-8 | El reescaneo completo forzado reasigna `album_id` en recopilatorios; cualquier referencia futura a álbumes (no a pistas) se rompería | Bajo en F2 (solo PISTAS se referencian) | Regla: playlists, cola, historial y favoritas referencian siempre PISTAS |
| R-9 | Last.fm exige que el usuario cree una API key propia; sin ella, solo ListenBrainz | Funcionalidad parcial | Documentado en README; ListenBrainz funciona con solo el token |
| R-10 | `anio` heredado del recopilatorio al revertir en escaneo incremental (F2 desviación 7) | Dato de año incorrecto hasta un reescaneo completo | Corrección incluida en el checklist de F3 |
| ~~R-11~~ | Cerrado 12/09/2026: el enlace directo por `target.object` funciona (validado por el agente con PipeWire real) | — | Fallback en dos pasos disponible por si falla en otra versión |
| ~~R-12~~ | Cerrado 12/09/2026: `pipewire` compila con la feature `v0_3_44` | — | — |
| R-13 | Render braille a 30 fps en Alacritty puede consumir más CPU de lo previsto en terminales grandes | Ventilador, batería | Degradación automática a 15 fps; `fps` configurable; presupuesto medido en S2 |
| R-14 | Visuales con pulsos pueden molestar a personas fotosensibles | Salud del usuario | Aviso en README; "Ambiente" como visual suave; `fps = 15`; `activo = false` |
| R-15 | Con `visuales.activo = true` (por defecto) el hilo de captura arranca siempre, aunque no se use el modo visual | Conexión PipeWire permanente, consumo mínimo | Documentado; `activo = false` lo evita. Candidato a mejora: arrancar la captura de forma perezosa al entrar en modo visual o con mini espectro visible |
| ~~R-16~~ | Cerrado 13/09/2026: migración 004 testeada con datos, copia previa y `foreign_key_check` | — | — |
| R-17 | Radio Browser es un servicio comunitario sin SLA; espejos y esquema pueden cambiar | La pestaña Buscar deja de funcionar | Caché local, cambio de espejo, `radio.directorio = false`; nunca bloquea la reproducción |
| R-18 | Descarga de logos desde hosts arbitrarios | Superficie de red mayor | Límites estrictos (T-12), decodificación aislada, `radio.logos = false` |
| R-19 | Títulos ICY mal formados producen scrobbles erróneos (nombre de programa, publicidad) | Historial y perfiles de scrobbling con ruido | Filtros de ruido, exigir artista, umbral de 30 s; el usuario puede desactivar scrobbling por emisora en una fase futura |
| R-20 | Resolutor DNS propio (`radio/espejos.rs`) parsea respuestas UDP de la red a mano | Superficie de seguridad y mantenimiento innecesarias | **En el checklist de F4**: sustitución por `dns-lookup` |
| R-21 | Omisión detectada en F2: el cliente de Last.fm no comprobaba la allowlist hasta F5 | Ninguno en producción (hosts fijos); lección de revisión | Corregido en F5 (T-30); añadir al cross-check de fases: toda petición pasa por `red/cliente.rs` |
| R-22 | `af-command` sobre filtros lavfi puede no estar soportado en alguna versión de mpv/ffmpeg | Cortes al ajustar bandas | Fallback a reconstrucción de la cadena; verificar en Omarchy real |
| R-23 | La biblioteca del usuario tiene pocas letras locales | La vista Letras se usa poco hasta añadir una fuente online | `FuenteLetras` enchufable; LRCLIB como fase corta futura |
| R-24 | `s` cambia de significado dentro de la vista Letras (fuente en vez de aleatorio) | Posible confusión | Documentado en ayuda y línea de atajos; el aleatorio sigue en la barra y en cualquier otra vista |
