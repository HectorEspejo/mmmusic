# mmmusic - Checklist Fase 2: Scrobbling, Recopilatorios y Favoritas

## Migración y modelo de datos
- [x] Migración `002_recopilatorios_envios.sql`: columnas `varios_artistas` y `carpeta` en ALBUMES, `carpeta` y `artista_album_etiquetado` en PISTAS, tablas ENVIOS y FAVORITAS con sus índices
- [x] Índices únicos parciales en ALBUMES: `(artista_id, titulo_norm) WHERE varios_artistas = 0` y `(titulo_norm, carpeta) WHERE varios_artistas = 1`, sustituyendo al índice único de F1
- [x] La migración 002 fija `AJUSTES.reescaneo_completo_pendiente = 1` una sola vez
- [x] Borrado en cascada de ENVIOS y FAVORITAS al eliminar una PISTA

## Recopilatorios
- [x] El escáner rellena `carpeta` (dirname de la ruta) y `artista_album_etiquetado` (1 si el fichero trae `albumartist`) en cada pista
- [x] Pasada de consolidación al final de cada escaneo sobre los grupos `(carpeta, titulo_norm de álbum)` con pistas nuevas o actualizadas
  - AC: Dada una carpeta con 3 pistas del mismo `album`, artistas distintos y sin `albumartist`, cuando termina el escaneo, entonces las 3 cuelgan de un único álbum con `varios_artistas = 1` y artista "Varios artistas"
- [x] Artista "Varios artistas" creado una sola vez (`nombre_norm = "varios artistas"`), sin fusionar ficheros con `albumartist` explícito
- [x] Álbum recopilatorio con `anio` = mínimo de sus pistas y carátula extraída o reutilizada
- [x] Un grupo que deja de cumplir la condición devuelve sus pistas a álbumes normales
- [x] Limpieza de álbumes y artistas huérfanos tras la consolidación
- [x] Modo de escaneo completo que ignora mtime/tamaño y relee todas las etiquetas
- [x] Reescaneo completo forzado al arrancar cuando `reescaneo_completo_pendiente = 1`, con toast explicativo, y puesta a 0 solo si el escaneo se completa
  - AC: Dada una base de F1 con recopilatorios partidos, cuando se arranca por primera vez tras la migración, entonces al terminar el escaneo aparecen agrupados y las playlists, la cola, el historial y las favoritas conservan sus pistas
- [x] Subcomando `mmmusic reescanear --completo` sin TUI con progreso y resumen por stdout

## CLI y credenciales
- [x] Argumentos con `clap`: sin subcomando arranca la TUI; `--version` y `--help`
- [x] Carga de `~/.config/mmmusic/credenciales.toml` con secciones `[listenbrainz]` (token) y `[lastfm]` (api_key, api_secret, session_key, usuario)
- [x] Permisos de `credenciales.toml` distintos de 600 se corrigen automáticamente con toast de aviso
- [x] Servicio activo en config sin credenciales se trata como inactivo con toast al arrancar
- [ ] Subcomando `mmmusic autorizar-lastfm`: `auth.getToken`, URL de autorización (intento de `xdg-open`), espera de Enter, `auth.getSession` con hasta 3 reintentos y guardado de `session_key` y `usuario`
  - AC: Dado api_key y api_secret válidos, cuando el usuario autoriza en el navegador y pulsa Enter, entonces el fichero contiene `session_key` y el comando termina con código 0
- [ ] Subcomando `mmmusic probar-servicios`: `validate-token` en ListenBrainz y `user.getInfo` en Last.fm, una línea por servicio y reprogramación de los envíos en error de autenticación si la prueba pasa
- [x] Códigos de salida de CLI: 0 OK, 1 error de configuración, 2 error de red
- [ ] `credenciales.ejemplo.toml` documentado y sección `[scrobbling]` en `config.ejemplo.toml`

## Hilo de scrobbling y cola de envíos
- [ ] Hilo de scrobbling con conexión SQLite propia, canal `ComandoScrobbling` (NowPlaying, Completada, Amar, Desamar, EnviarAhora, RecargarCredenciales, Apagar) y `watch<EstadoScrobbling>`
- [x] Regla de elegibilidad: pista > 30 s; umbral min(50 %, 4 min) de tiempo real acumulado
- [x] `HISTORIAL_REPRODUCCION.completada` pasa a usar el umbral min(50 %, 4 min) en lugar del 50 % fijo
- [x] Now playing enviado a cada servicio activo al cargar una pista elegible, sin cola ni reintentos
- [x] Encolado de un ENVIO `scrobble` por servicio activo al marcarse `completada`, con `reproducido_en` = inicio de la escucha
- [x] Ciclo de envío cada 30 s con lotes de hasta 50 pendientes por servicio ordenados por `reproducido_en`, y reposo del hilo sin pendientes
- [x] Planificador de reintentos exponencial 30 s · 2^intentos con tope 6 h y descarte tras 20 intentos
- [x] Errores de autenticación (401/403, Last.fm 4/9/14) reprograman a +24 h, guardan `scrobbling_ultimo_error` y muestran toast con el servicio afectado
- [ ] Descarte de scrobbles Last.fm con `reproducido_en` de más de 14 días o timestamp futuro, y de envíos con firma inválida (Last.fm 13)
- [ ] Respuesta parcial de Last.fm (`ignored`) marca solo ese envío como descartado con el motivo
- [x] Lote ListenBrainz rechazado (400) se reintenta elemento a elemento para aislar y descartar el defectuoso
- [x] Toda petición HTTP con `ureq` (rustls), timeout 10 s, `User-Agent: mmmusic/<versión>`, solo a `api.listenbrainz.org` y `ws.audioscrobbler.com`
- [x] Tokens, `api_secret` y `session_key` nunca aparecen en logs, toasts ni `error_msg`
- [x] Los envíos pendientes sobreviven al cierre y se envían en el siguiente arranque
  - AC: Dada una escucha completada sin red y mmmusic cerrado, cuando se arranca con red, entonces el envío pasa a `enviado` en menos de 60 s

## ListenBrainz
- [x] `POST /1/submit-listens` con `listen_type = playing_now` (sin `listened_at`) y `import` (con `listened_at` en segundos Unix), cabecera `Authorization: Token`
- [x] `track_metadata` con artist_name, track_name, release_name y `additional_info` (duration_ms, media_player "mmmusic", submission_client "mmmusic")
- [x] `GET /1/validate-token` para `probar-servicios`

## Last.fm
- [ ] Firma `api_sig` = md5 de parámetros ordenados + secret, `format=json`, con test unitario contra un vector conocido
- [ ] `track.updateNowPlaying`, `track.scrobble` por lotes con parámetros indexados `[i]`, `track.love`, `track.unlove` y `user.getInfo`
- [ ] Envío de título, artista y álbum originales (no normalizados) y `duration` en segundos

## Favoritas
- [ ] `L` alterna la favorita de la pista seleccionada o, sin selección de pista, de la que suena; se ignora sobre artistas, álbumes y playlists
- [ ] Encolado de `love`/`unlove` para Last.fm al alternar, descartando el pendiente anterior de la misma pista
- [ ] Pseudo-playlist "♥ Favoritas" primera en la sección Playlists, no editable con `R`, `D`, `J`/`K` ni `d`
- [ ] Detalle de ♥ Favoritas con reproducción en contexto, `a`, `A`, `P` y exportación M3U8 como `Favoritas.m3u8`
- [ ] Barra inferior muestra ♥ junto al título cuando la pista actual es favorita
- [ ] Toast "♥ <título>" / "Quitada de favoritas" al alternar

## Interfaz
- [x] Indicador de scrobbling en la barra inferior: `↑` enviado, `…N` pendientes, `!` error, nada si está desactivado, con equivalentes `sc:ok` / `sc:N` / `sc:err` en modo ascii
- [x] En modo compacto (< 70 columnas) el indicador solo muestra `!` si hay error
- [x] `Ctrl+s` fuerza el envío inmediato de pendientes
- [ ] Vista Inicio con tres bloques en rejilla de tarjetas 4×3 con carátula, `h`/`l` dentro del bloque y `j`/`k` entre bloques
  - AC: Dado Inicio con carátulas activas, cuando se abre, entonces las tarjetas aparecen progresivamente y el bloque enfocado resalta su título
- [ ] Inicio vuelve al formato de texto de F1 con `caratulas = false` o alto < 20 filas
- [ ] Overlay de ayuda con la sección "Scrobbling y favoritas" (`L`, `Ctrl+s`, subcomandos de CLI)
- [x] Cabecera de álbum y vista Artistas muestran "Varios artistas" en los recopilatorios, manteniendo el artista real por pista en la lista

## Calidad y entrega
- [ ] Tests unitarios de regla de scrobble, planificador de reintentos, clasificación de respuestas, firma Last.fm y cuerpo JSON de ListenBrainz
- [x] Test de integración de recopilatorios con `tests/fixtures/recopilatorio/` (3 pistas, artistas distintos, sin albumartist) y de su reversión al etiquetar `albumartist`
- [x] Test de persistencia de ENVIOS y de descarte de love/unlove anteriores
- [x] `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios
- [ ] README actualizado: scrobbling, autorización de Last.fm, copia de seguridad de `credenciales.toml`, reescaneo completo

---

**Progreso Fase 2:** 39 / 59 funcionalidades

**Total mmmusic (Fases 1-2):** 151 / 171 funcionalidades
