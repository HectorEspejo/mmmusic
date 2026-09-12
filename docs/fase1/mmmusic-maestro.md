# mmmusic - Informe Maestro

**Última actualización:** 12 de septiembre de 2026

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
       ┌───────────────────────┼───────────────────────┬──────────────────┐
       ▼                       ▼                       ▼                  ▼
┌──────────────┐   ┌──────────────────────┐   ┌────────────────┐   ┌─────────────────┐
│ FASE 2       │   │ FASE 3               │   │ FASE 4         │   │ FASE 5          │
│ Scrobbling   │   │ Letras + ecualizador │   │ Radio / URL    │   │ Nicotine+       │
│ (HISTORIAL)  │   │ (mpv af, PISTAS)     │   │ (cola no-PISTA)│   │ (escaneo notify)│
└──────────────┘   └──────────────────────┘   └────────────────┘   └─────────────────┘
```

## 3. Tabla Resumen de Fases

| Fase | Módulo | Funcionalidades | Sprints | Semanas | Estado |
|------|--------|-----------------|---------|---------|--------|
| 1 | Reproductor Base | 112 | 5 | 5-6 | Especificada |
| **Total** | | **112** | **5** | **~1,5 meses** | |

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
                        └──────────────┘        │ genero           │
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

## 6. Estado del Proyecto

| Fase | Progreso según checklist | Último informe de implementación procesado |
|------|--------------------------|--------------------------------------------|
| 1 | 0 / 112 | ninguno — sin confirmar |

**Progreso total:** 0 / 112 funcionalidades.

## 7. Roadmap

| Fase | Contenido | Depende de |
|------|-----------|-----------|
| 2 | Scrobbling Last.fm / ListenBrainz (cola de scrobbles offline, credenciales en config); corrección de recopilatorios sin `albumartist` | F1 (HISTORIAL_REPRODUCCION, regla del 50 %) |
| 3 | Letras (ficheros `.lrc` junto a la pista y etiqueta `lyrics`) y ecualizador (filtros `af` de mpv con presets) | F1 (envoltorio mpv) |
| 4 | Radio y streams por URL (cola con elementos no-PISTA, favoritos de emisoras) | F1 (modelo de cola) |
| 5 | Integración con Nicotine+ (carpeta de descargas como raíz, reescaneo automático con `notify`) | F1 (escáner incremental) |
| — | Ideas sin fase: estadísticas de escucha, edición de etiquetas, modo servidor MPD-compatible | |

Orden recomendado: 2 → 3 → 4 → 5. La corrección de recopilatorios se ha colocado en F2 por ser pequeña y afectar a la calidad del índice desde el principio.

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

### Riesgos abiertos

| Id | Riesgo | Impacto | Mitigación |
|----|--------|---------|-----------|
| R-1 | El crate `libmpv2` no compila contra la versión de mpv de Arch (cambios en la ABI de libmpv) | Bloquea S2 | Alternativa: mpv como proceso hijo con socket IPC (`--input-ipc-server`), +3 días |
| R-2 | Carátulas en half-blocks en Alacritty resultan poco atractivas para el usuario | Estético | Probar Ghostty/Kitty; opción `caratulas = false`; reducir tamaño de tarjetas |
| R-3 | Formato del tema de Omarchy cambia en versiones futuras (deja de haber `alacritty.toml` o cambia su estructura) | Pierde colores del sistema | Fallback a paleta terminal ya previsto; añadir otras fuentes (`colors.toml`) cuando existan |
| R-4 | Recopilatorios sin `albumartist` se fragmentan en varios álbumes | Calidad del índice | Documentado; corrección en F2 |
| R-5 | `ratatui-image` con protocolo Kitty y muchas tarjetas simultáneas puede ser lento | Rendimiento en rejillas | Limitar a tarjetas visibles + LRU; medir en S4 |
