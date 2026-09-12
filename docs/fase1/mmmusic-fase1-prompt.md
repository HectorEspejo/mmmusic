# Prompt mmmusic - Fase 1: Reproductor Base

Desarrolla **mmmusic**, un reproductor de música para terminal (TUI) para Omarchy (Arch + Hyprland) con layout tipo Spotify: sidebar izquierda de navegación, contenido central y barra inferior fija de "sonando ahora". Stack: **Rust (edición 2024), ratatui + crossterm, libmpv vía crate `libmpv2` (opciones `vo=null`, `audio-display=no`, `ytdl=no`, `load-scripts=no`, `config=no`, `gapless-audio=yes`), rusqlite `bundled` en WAL, lofty para etiquetas, ratatui-image + image para carátulas (Kitty/Sixel con fallback half-blocks), mpris-server sobre tokio en hilo propio, serde + toml + directories para `~/.config/mmmusic/config.toml`, notify para recargar el tema, walkdir, tracing**. Tablas: (1) **ARTISTAS** con id, nombre, nombre_norm único, creado_en; (2) **ALBUMES** con artista_id, titulo, titulo_norm, anio, caratula_ruta (único artista_id+titulo_norm); (3) **PISTAS** con album_id, artista_id, titulo, titulo_norm, numero_pista, numero_disco, genero, duracion_ms, ruta única, formato, tamano_bytes, modificado_en, bitrate_kbps, anadido_en, escaneo_id; (4) **PLAYLISTS** con nombre único; (5) **PLAYLIST_PISTAS** con playlist_id, pista_id, posicion; (6) **COLA** con pista_id, posicion, posicion_orig; (7) **HISTORIAL_REPRODUCCION** con pista_id, reproducido_en, completada; (8) **ESCANEOS** con estado en_curso/completado/error/cancelado y contadores; (9) **AJUSTES** clave-valor. Funcionalidades: escaneo incremental por mtime/tamaño en hilo propio con cancelación, normalización sin diacríticos, resolución de etiquetas ausentes, carátulas embebidas o `cover.*` cacheadas a 300×300 en `~/.cache/mmmusic/caratulas/`, hilo reproductor con máquina de estados y canal watch de `EstadoReproduccion`, cola con aleatorio (conservando orden original), repetición no/todo/una, persistencia y restauración en pausa, historial con marca de completada al 50 %, búsqueda LIKE sobre columnas normalizadas, playlists CRUD con reorden e import/export M3U8, tema leído de `~/.config/omarchy/current/theme/alacritty.toml` con recarga en caliente, MPRIS completo (`org.mpris.MediaPlayer2.mmmusic`) y atajos vim. Interfaz: vistas Inicio, Buscar, Artistas (lista+detalle), Álbumes (rejilla con carátulas+detalle), Pistas (tabla ordenable paginada), Playlists, panel de cola a la derecha, overlay de ayuda, diálogos de confirmación/texto/selector, toasts, modo compacto bajo 70 columnas o 20 filas, iconos Nerd Font con modo ascii, ratón opcional. Tests unitarios de cola/normalización/config e integración de escaneo con fixtures de los seis formatos.

---

## Instrucciones para el agente

1. Lee primero `CLAUDE.md` en la raíz del repositorio: contiene las reglas
   permanentes de trabajo (idioma, commits, effort, cierre de fase).
2. Sigue el checklist `mmmusic-fase1-checklist.md` como definición
   del alcance. No añadas funcionalidades fuera de él sin indicarlo.
3. Al finalizar el desarrollo o al pausar la sesión, marca el checklist y
   genera/actualiza el archivo **`mmmusic-fase1-implementacion.md`**
   (informe de implementación) con exactamente esta estructura:
   - Resumen de lo implementado
   - Desviaciones respecto a la especificación (qué y por qué)
   - Estructura de archivos creada/modificada
   - Decisiones técnicas tomadas durante el desarrollo
   - Funcionalidades del checklist completadas (copiando su texto exacto)
   - Pendientes y bloqueos
   - Ejecución y pruebas (cómo arrancar, migrar y testear)
4. Actualiza el informe de implementación en cada sesión, no lo
   regeneres desde cero.
5. Cuando el informe esté listo, avisa al desarrollador de que debe
   entregárselo al analista funcional: hasta entonces la documentación de
   proyecto (checklist maestro, informe maestro) no refleja la realidad.
