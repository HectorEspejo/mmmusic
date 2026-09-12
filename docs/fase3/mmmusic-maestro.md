# mmmusic - Informe Maestro

**Última actualización:** 12 de septiembre de 2026 (Fase 3 especificada; Fase 2 completada y validada)

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
┌──────────────┐   ┌──────────────────────┐   ┌──────────────────────┐   ┌────────────────┐   ┌─────────────────┐
│ FASE 2 ✔     │   │ FASE 3               │   │ FASE 4               │   │ FASE 5         │   │ FASE 6          │
│ Scrobbling,  │   │ Visuales reactivas   │   │ Letras + ecualizador │   │ Radio / URL    │   │ Nicotine+       │
│ recopilat.,  │   │ (PipeWire, FFT,      │   │ (mpv af, PISTAS)     │   │ (cola no-PISTA)│   │ (escaneo notify)│
│ favoritas    │   │  canvas braille)     │   └──────────────────────┘   └────────────────┘   └─────────────────┘
└──────────────┘   └──────────────────────┘
```

## 3. Tabla Resumen de Fases

| Fase | Módulo | Funcionalidades | Sprints | Semanas | Estado |
|------|--------|-----------------|---------|---------|--------|
| 1 | Reproductor Base | 112 | 5 | 5-6 | Completada (validada 12/09/2026) |
| 2 | Scrobbling, Recopilatorios y Favoritas | 59 | 4 | 3-4 | Completada (validada 12/09/2026) |
| 3 | Visuales Reactivas | 59 | 4 | 3-4 | Especificada |
| **Total** | | **230** | **13** | **~3,5 meses** | |

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
                                   │ creado_en, enviado_en          │
                                   └────────────────────────────────┘
                                       *→1 PISTAS, *→1 HISTORIAL_REPRODUCCION
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

## 6. Estado del Proyecto

| Fase | Progreso según checklist | Último informe de implementación procesado |
|------|--------------------------|--------------------------------------------|
| 1 | 112 / 112 | 12/09/2026 (sesión 1, S1–S5); validaciones manuales confirmadas por Hector el 12/09/2026 |
| 2 | 59 / 59 | 12/09/2026 (sesión 1, S1–S4); validación con servicios reales confirmada por Hector el 12/09/2026 |
| 3 | 0 / 59 | ninguno — sin confirmar |

**Progreso total:** 171 / 230 funcionalidades.

## 7. Roadmap

| Fase | Contenido | Depende de |
|------|-----------|-----------|
| 4 | Letras (ficheros `.lrc` junto a la pista y etiqueta `lyrics`) y ecualizador (filtros `af` de mpv con presets); posible vista letras + visual | F1 (envoltorio mpv), F3 |
| 5 | Radio y streams por URL (cola con elementos no-PISTA, favoritos de emisoras) | F1 (modelo de cola) |
| 6 | Integración con Nicotine+ (carpeta de descargas como raíz, reescaneo automático con `notify`) | F1 (escáner incremental), F2 (consolidación) |
| — | Ideas sin fase: estadísticas de escucha, edición de etiquetas, modo servidor MPD-compatible, espectro espejado, "sunset grid" vaporwave, carátula pulsante, exportar frame de visual, mini visual en Waybar | |

Orden recomendado: 4 → 5 → 6 (la 3 está especificada). La corrección R-10 va dentro de la Fase 3. Idea añadida en F2: sincronizar favoritas con ListenBrainz si algún día la biblioteca lleva MBIDs (Picard).

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
| T-12 | Toda red saliente pasa por el hilo de scrobbling con `ureq` (rustls), timeout 10 s y lista cerrada de hosts; un módulo nuevo que necesite red añade su host a esa lista, no abre otro cliente | F2 DT-15 | F3+ |
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
| R-11 | El enlace directo del stream de captura al nodo de mpv (`target.object`) puede no funcionar según la versión de PipeWire/WirePlumber | Visuales sin aislamiento (mostrarían todo el audio del sistema) | Fallback por monitor del sink anunciado en la cabecera; comprobar con `pw-link` en Omarchy real |
| R-12 | El crate `pipewire` requiere `clang`/bindgen y una versión concreta de libpipewire | Bloquea S1 de F3 | Fijar versión en `Cargo.toml`; alternativa: `pw-cat`/`pw-record` como proceso hijo leyendo stdout (+2 días) |
| R-13 | Render braille a 30 fps en Alacritty puede consumir más CPU de lo previsto en terminales grandes | Ventilador, batería | Degradación automática a 15 fps; `fps` configurable; presupuesto medido en S2 |
| R-14 | Visuales con pulsos pueden molestar a personas fotosensibles | Salud del usuario | Aviso en README; "Ambiente" como visual suave; `fps = 15`; `activo = false` |
