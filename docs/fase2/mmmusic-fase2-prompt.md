# Prompt mmmusic - Fase 2: Scrobbling, Recopilatorios y Favoritas

Continúa desarrollando **mmmusic**, el reproductor TUI para Omarchy de la Fase 1 (Rust 2024, ratatui, libmpv2, rusqlite, lofty, ratatui-image, mpris-server). Stack nuevo: **`ureq` con rustls en un hilo dedicado de scrobbling (sin runtime async global), `md5` para la firma de Last.fm, `clap` para subcomandos**. Tablas: (1) **ENVIOS** con servicio (listenbrainz/lastfm), tipo (scrobble/love/unlove), pista_id, historial_id (→ HISTORIAL_REPRODUCCION, sin cambios), reproducido_en, estado (pendiente/enviado/error/descartado), intentos, proximo_intento_en, error_msg, creado_en, enviado_en; (2) **FAVORITAS** con pista_id único y marcada_en; (3) **ALBUMES** añade varios_artistas y carpeta con índices únicos parciales por `varios_artistas`; (4) **PISTAS** añade carpeta y artista_album_etiquetado; (5) **AJUSTES** añade reescaneo_completo_pendiente y scrobbling_ultimo_error. Todo en la migración `002_recopilatorios_envios.sql`. Funcionalidades: consolidación de recopilatorios al final del escaneo (grupo carpeta+álbum sin albumartist y con artistas distintos → álbum bajo "Varios artistas", reversible), reescaneo completo forzado al arrancar tras la migración y subcomando `reescanear --completo`; credenciales en `~/.config/mmmusic/credenciales.toml` con permisos 600; subcomandos `autorizar-lastfm` (getToken, URL, getSession) y `probar-servicios`; regla de scrobble pista > 30 s y umbral min(50 %, 4 min) de tiempo real (también para `completada`); now playing sin cola; cola de envíos persistente con lotes de 50, reintentos 30 s·2^n hasta 6 h, descarte a los 20 intentos o > 14 días en Last.fm, errores de autenticación a +24 h con toast; ListenBrainz `submit-listens` (playing_now/import) y `validate-token`; Last.fm firmado (updateNowPlaying, scrobble, love, unlove, user.getInfo); favoritas con `L`, sincronizadas solo con Last.fm, pseudo-playlist "♥ Favoritas" no editable; `Ctrl+s` envía pendientes. Interfaz: indicador de scrobbling en la barra inferior (`↑`, `…N`, `!`, con modo ascii), ♥ junto al título, Inicio con tres bloques en rejilla de tarjetas 4×3 con carátula y fallback a texto, ayuda ampliada. Sin secretos en logs ni toasts; red solo a `api.listenbrainz.org` y `ws.audioscrobbler.com` con timeout 10 s. Tests sin red: regla, planificador, firma, JSON, recopilatorios con fixture.

---

## Instrucciones para el agente

1. Lee primero `CLAUDE.md` en la raíz del repositorio: contiene las reglas
   permanentes de trabajo (idioma, commits, effort, cierre de fase).
2. Sigue el checklist `mmmusic-fase2-checklist.md` como definición
   del alcance. No añadas funcionalidades fuera de él sin indicarlo.
3. Al finalizar el desarrollo o al pausar la sesión, marca el checklist y
   genera/actualiza el archivo **`mmmusic-fase2-implementacion.md`**
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
