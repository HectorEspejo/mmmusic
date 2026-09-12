# mmmusic - Fase 2: Informe de Implementación

**Documento vivo.** Última actualización: 12 de septiembre de 2026 (sesión 1: S1–S4 completados; fase cerrada).

Estado por sprint:

| Sprint | Contenido | Estado |
|--------|-----------|--------|
| S1 | Migración 002, recopilatorios y reescaneo completo | Completado y verificado |
| S2 | Credenciales, hilo de scrobbling y ListenBrainz | Completado y verificado |
| S3 | Last.fm y favoritas | Completado y verificado |
| S4 | Inicio con tarjetas, ayuda, README y cierre | Completado y verificado |

## Resumen de lo implementado

### S1 — Migración y recopilatorios

- Migración `002_recopilatorios_envios.sql`: columnas `varios_artistas` y `carpeta` en ALBUMES, `carpeta` y `artista_album_etiquetado` en PISTAS, índices únicos parciales por `varios_artistas`, tablas ENVIOS y FAVORITAS con cascadas, e inserción única de `AJUSTES.reescaneo_completo_pendiente = 1`.
- Helpers de fecha en `bd.rs`: `unix_desde_iso()` y `iso_en()` (inversa exacta del formato ISO 8601 propio), usados por scrobbling y por los descartes por antigüedad.
- Escáner: `ModoEscaneo::{Incremental, Completo}` (el completo ignora mtime/tamaño), relleno de `carpeta` y `artista_album_etiquetado`, y consolidación de recopilatorios antes de limpiar huérfanos.
- `recopilatorios.rs`: agrupa por `(carpeta, titulo_norm)` de álbum las pistas sin `albumartist` con artistas distintos bajo "Varios artistas" (artista único, álbum con `anio` mínimo y carátula reutilizada o generada después), revierte a álbumes normales cuando el grupo deja de cumplir la condición y es idempotente entre escaneos.
- Reescaneo completo forzado al arrancar cuando la bandera está a 1, con toast explicativo y puesta a 0 solo si el escaneo termina completado; `mmmusic reescanear [--completo]` sin TUI con progreso y resumen por stdout.
- Fixtures `tests/fixtures/recopilatorio/` (3 MP3 diminutos, artistas distintos, sin `albumartist`) y `tests/recopilatorios.rs` con agrupación, año mínimo, idempotencia, reversión etiquetando `albumartist` y reagrupación de una base fragmentada en modo completo.

### S2 — Credenciales, hilo de scrobbling y ListenBrainz

- `credenciales.rs`: carga y guardado de `~/.config/mmmusic/credenciales.toml` con permisos corregidos automáticamente a 600, aviso en UI/CLI y `redactar()` para que tokens, `api_secret` y `session_key` no aparezcan en logs, toasts ni `error_msg`.
- Sección `[scrobbling]` en la configuración (`listenbrainz`, `lastfm`, `now_playing`); un servicio activo sin credenciales se trata como inactivo con toast.
- Hilo de scrobbling con conexión SQLite propia, canal `ComandoScrobbling`, `watch<EstadoScrobbling>` y publicación a la UI; ciclo cada 30 s con lotes de hasta 50, reintentos `30 s·2^n` con tope de 6 h, descarte a los 20 intentos, errores de autenticación a +24 h con `AJUSTES.scrobbling_ultimo_error` y toast del servicio afectado.
- `regla.rs` con elegibilidad (>30 s) y umbral `min(50 %, 4 min)`; el historial usa el mismo umbral.
- `listenbrainz.rs`: `POST /1/submit-listens` (`playing_now` sin `listened_at` e `import` con segundos Unix, `track_metadata` y `additional_info` según especificación), `GET /1/validate-token` y reintento elemento a elemento cuando el lote devuelve 400.
- Now playing inmediato sin cola (suprimido al restaurar la cola en pausa), encolado de `scrobble` al marcar el historial como completado, `Ctrl+s` y subcomando `mmmusic probar-servicios` con códigos 0/1/2.
- Indicador de scrobbling en la barra inferior (`↑`, `…N`, `!`; `sc:ok`, `sc:N`, `sc:err` en ascii; solo `!` en modo compacto si hay error) y tests sin red de regla, planificador, clasificación, JSON, redacción y persistencia de ENVIOS.

### S3 — Last.fm y favoritas

- `lastfm.rs`: firma `api_sig` (parámetros ordenados + secret, md5, `format=json`, con vector conocido en test), `auth.getToken`/`auth.getSession`, `track.updateNowPlaying`, `track.scrobble` por lotes indexados, `track.love`/`track.unlove` y `user.getInfo`.
- `mmmusic autorizar-lastfm`: exige `api_key` y `api_secret`, imprime y abre la URL de autorización con `xdg-open`, espera Enter, reintenta `getSession` hasta 3 veces ante el error 14 y guarda `session_key` y `usuario` con permisos 600.
- Descarte de scrobbles Last.fm con más de 14 días o timestamp futuro, descarte por firma inválida (error 13), respuesta parcial `ignored` (solo el envío afectado se descarta con el motivo) y reprogramación de errores de autenticación desde `probar-servicios`.
- Favoritas: `L` alterna la pista bajo el cursor (o la que suena), encolado `love`/`unlove` descartando el pendiente anterior, toasts, `♥` en la barra inferior, pseudo-playlist "♥ Favoritas" primera y no editable (bloquea `R`, `D`, `J`/`K` y `d`) con reproducción en contexto, `a`, `A`, `P` y exportación `Favoritas.m3u8`.
- Ayuda ampliada con la sección "Scrobbling y favoritas" y `credenciales.ejemplo.toml`.

### S4 — Inicio con tarjetas, ayuda y cierre

- Inicio con tres bloques apilados de hasta 10 tarjetas con carátula, `h`/`l` dentro del bloque y `j`/`k` entre bloques; el título del bloque enfocado se resalta y las tarjetas aparecen progresivamente con la caché de carátulas.
- Vuelta al formato de texto de la fase 1 con `caratulas = false` o alto menor de 20 filas.
- README actualizado (scrobbling, autorización de Last.fm, copia de seguridad de credenciales, reescaneo completo y nuevos atajos), `config.ejemplo.toml` y `credenciales.ejemplo.toml`.

## Desviaciones respecto a la especificación (qué y por qué)

1. **Interpretación de la rejilla de Inicio.** El checklist decía "rejilla de tarjetas 4×3", el mockup mostraba una fila de tarjetas pequeñas por bloque y Rendimiento decía "tres filas de hasta 10 tarjetas". Se confirmó con el desarrollador la interpretación de tres filas (una por bloque) de hasta 10 tarjetas con `h`/`l` dentro del bloque y `j`/`k` entre bloques.
2. **Dependencia `serde_json`.** El informe solo citaba `serde`, pero construir el cuerpo JSON de ListenBrainz y parsear las respuestas de Last.fm requiere `serde_json`; se añadió como dependencia técnica.
3. **`scrobbling/planificador.rs` como módulo propio.** El informe situaba el planificador dentro de `scrobbling/mod.rs`; se separó para poder testearlo desde `tests/scrobbling.rs` y mantener el hilo legible. No cambia la API interna.
4. **Marcador interno `auth:` en `error_msg`.** Para distinguir los errores de autenticación (que `probar-servicios` reprograma) se prefija su `error_msg` con `auth:`; nunca se muestra en UI (los toasts usan el mensaje limpio) y el resto de errores guardan su mensaje tal cual.
5. **`historial::registrar_inicio` devuelve `(id, reproducido_en)`.** El hilo de scrobbling necesita el instante exacto de inicio de la escucha para `timestamp`/`listened_at`; se devuelve en la misma inserción en lugar de volver a consultarlo.
6. **Now playing no se envía al restaurar.** Al arrancar, la cola se restaura en pausa; se suprime el "now playing" de esa carga para no registrar una escucha que no ha empezado.
7. **Reversión de recopilatorio y año.** Las pistas que no se releen en un escaneo incremental heredan el `anio` del álbum "Varios artistas" al volver a álbumes normales; el siguiente escaneo completo lo corrige con la etiqueta del fichero.
8. **`love`/`unlove` se envían de uno en uno.** La API de Last.fm no permite agruparlos; en ráfagas superan el objetivo de "≤ 2 peticiones por servicio y ciclo". Es puntual (solo cuando el usuario alterna favoritas) y el resto del ciclo respeta el objetivo.
9. **`mmmusic reescanear` sin `--completo`.** La especificación solo definía `--completo`; se admite también el incremental con el mismo formato de progreso y resumen.
10. **Códigos de salida de `probar-servicios`.** 0 si todos los servicios configurados responden OK (y hay al menos uno), 1 si no hay credenciales, falta autorizar o falla la autenticación, 2 si falla la red.
11. **Fallback del indicador compacto.** "Modo compacto (< 70 columnas)" se mide con el ancho real de la barra inferior; con el resto de la interfaz en modo ancho, el indicador sigue mostrando `↑`/`…N`.
12. **Ayuda con más líneas.** El overlay subió su altura máxima (de 32 a 46) porque la nueva sección no cabía en pantallas normales.
13. **Firma de Last.fm sin `format`.** Siguiendo la práctica estándar, `format` y `api_sig` quedan fuera del cálculo de la firma; el test con vector conocido fija el comportamiento.

## Estructura de archivos creada/modificada

```
Cargo.toml  Cargo.lock  README.md  config.ejemplo.toml  credenciales.ejemplo.toml
docs/fase2/mmmusic-fase2-checklist.md
docs/fase2/mmmusic-fase2-implementacion.md
src/
  cli.rs                                    (nuevo: clap, reescanear, probar-servicios, autorizar-lastfm)
  credenciales.rs                           (nuevo)
  config.rs                                 (+[scrobbling], +ruta de credenciales)
  eventos.rs                                (+AppEvento::Scrobbling)
  lib.rs                                    (+cli, +credenciales, +scrobbling)
  main.rs                                   (subcomandos, hilo de scrobbling, arranque forzado)
  app.rs                                    (modo de escaneo, favoritas, pseudo-playlist, Inicio por bloques)
  biblioteca/
    bd.rs                                   (+migración 002, unix_desde_iso, iso_en)
    consultas.rs                            (+envios, +favoritas, export M3U de listas, reescaneo)
    escaner.rs                              (ModoEscaneo, carpeta, etiquetado, consolidación)
    etiquetas.rs                            (+album_artista_etiquetado)
    modelos.rs                              (+campos nuevos de ALBUMES/PISTAS)
    recopilatorios.rs                       (nuevo)
    migraciones/002_recopilatorios_envios.sql (nuevo)
  scrobbling/
    mod.rs  estado.rs  regla.rs  planificador.rs  listenbrainz.rs  lastfm.rs
  reproductor/mod.rs                        (umbral, comandos de scrobbling, registrar_inicio)
  ui/
    mod.rs  teclas.rs  sidebar.rs  barra_inferior.rs
    vistas/inicio.rs  vistas/playlists.rs  vistas/ayuda.rs  vistas/mod.rs
tests/
  recopilatorios.rs  scrobbling.rs
  fixtures/recopilatorio/{01 Primera,02 Segunda,03 Tercera}.mp3
```

## Decisiones técnicas tomadas durante el desarrollo

- **Consolidación en memoria + reevaluación completa del grupo.** El escáner recoge los grupos `(carpeta, titulo_norm)` de las pistas nuevas/actualizadas (todos en modo completo) y `consolidar` reevalúa todo el grupo contra la base, de modo que la reversión funciona aunque solo se reetiquete una pista.
- **Índices únicos parciales y `ON CONFLICT` con `WHERE`.** Los upserts de álbumes normales y recopilatorios usan el destino de conflicto que corresponde a su índice parcial; los álbumes normales dejan `carpeta` a NULL.
- **Bandera de reescaneo idempotente y reanudable.** La pone a 1 la migración y a 0 el propio escáner solo al completar un escaneo completo; error o cancelación la conservan.
- **Un único planificador y una única cola ENVIOS** para scrobbles y love/unlove de ambos servicios: el hilo despacha por servicio y el indicador suma pendientes. La lógica de clasificación es común.
- **Redacción centralizada.** Todo mensaje que viaja a `error_msg`, logs o toasts pasa por `Credenciales::redactar()`; el hilo nunca escribe cuerpos de petición con secretos.
- **`ureq` síncrono con `http_status_as_error(false)`** y timeout global de 10 s, `User-Agent` `mmmusic/<versión>` y comprobación `url_permitida()` contra la lista cerrada de hosts.
- **Pseudo-playlist con id centinela `-1`** en lugar de una fila en PLAYLISTS: no ensucia la base, sobrevive a reescaneos y todas las acciones de edición comprueban el centinela.
- **Selección de Inicio como bloque + columna** sincronizada con el índice plano existente, para reutilizar `contexto()` y el resto de la lógica de reproducción sin duplicarla.
- **Tiempo real acumulado para el umbral.** El temporizador de escucha real (deltas < 1,5 s) alimenta ahora `min(50 %, 4 min)`, de modo que los seeks no cuentan ni para el historial ni para el scrobble.
- **Tests sin red.** Los clientes HTTP son envoltorios finos; los tests cubren firma, JSON, clasificación, planificador, regla, persistencia y favoritas con bases temporales.

## Funcionalidades del checklist completadas (texto exacto)

### Migración y modelo de datos
- [x] Migración `002_recopilatorios_envios.sql`: columnas `varios_artistas` y `carpeta` en ALBUMES, `carpeta` y `artista_album_etiquetado` en PISTAS, tablas ENVIOS y FAVORITAS con sus índices
- [x] Índices únicos parciales en ALBUMES: `(artista_id, titulo_norm) WHERE varios_artistas = 0` y `(titulo_norm, carpeta) WHERE varios_artistas = 1`, sustituyendo al índice único de F1
- [x] La migración 002 fija `AJUSTES.reescaneo_completo_pendiente = 1` una sola vez
- [x] Borrado en cascada de ENVIOS y FAVORITAS al eliminar una PISTA

### Recopilatorios
- [x] El escáner rellena `carpeta` (dirname de la ruta) y `artista_album_etiquetado` (1 si el fichero trae `albumartist`) en cada pista
- [x] Pasada de consolidación al final de cada escaneo sobre los grupos `(carpeta, titulo_norm de álbum)` con pistas nuevas o actualizadas
- [x] Artista "Varios artistas" creado una sola vez (`nombre_norm = "varios artistas"`), sin fusionar ficheros con `albumartist` explícito
- [x] Álbum recopilatorio con `anio` = mínimo de sus pistas y carátula extraída o reutilizada
- [x] Un grupo que deja de cumplir la condición devuelve sus pistas a álbumes normales
- [x] Limpieza de álbumes y artistas huérfanos tras la consolidación
- [x] Modo de escaneo completo que ignora mtime/tamaño y relee todas las etiquetas
- [x] Reescaneo completo forzado al arrancar cuando `reescaneo_completo_pendiente = 1`, con toast explicativo, y puesta a 0 solo si el escaneo se completa
- [x] Subcomando `mmmusic reescanear --completo` sin TUI con progreso y resumen por stdout

### CLI y credenciales
- [x] Argumentos con `clap`: sin subcomando arranca la TUI; `--version` y `--help`
- [x] Carga de `~/.config/mmmusic/credenciales.toml` con secciones `[listenbrainz]` (token) y `[lastfm]` (api_key, api_secret, session_key, usuario)
- [x] Permisos de `credenciales.toml` distintos de 600 se corrigen automáticamente con toast de aviso
- [x] Servicio activo en config sin credenciales se trata como inactivo con toast al arrancar
- [x] Subcomando `mmmusic autorizar-lastfm`: `auth.getToken`, URL de autorización (intento de `xdg-open`), espera de Enter, `auth.getSession` con hasta 3 reintentos y guardado de `session_key` y `usuario`
- [x] Subcomando `mmmusic probar-servicios`: `validate-token` en ListenBrainz y `user.getInfo` en Last.fm, una línea por servicio y reprogramación de los envíos en error de autenticación si la prueba pasa
- [x] Códigos de salida de CLI: 0 OK, 1 error de configuración, 2 error de red
- [x] `credenciales.ejemplo.toml` documentado y sección `[scrobbling]` en `config.ejemplo.toml`

### Hilo de scrobbling y cola de envíos
- [x] Hilo de scrobbling con conexión SQLite propia, canal `ComandoScrobbling` (NowPlaying, Completada, Amar, Desamar, EnviarAhora, RecargarCredenciales, Apagar) y `watch<EstadoScrobbling>`
- [x] Regla de elegibilidad: pista > 30 s; umbral min(50 %, 4 min) de tiempo real acumulado
- [x] `HISTORIAL_REPRODUCCION.completada` pasa a usar el umbral min(50 %, 4 min) en lugar del 50 % fijo
- [x] Now playing enviado a cada servicio activo al cargar una pista elegible, sin cola ni reintentos
- [x] Encolado de un ENVIO `scrobble` por servicio activo al marcarse `completada`, con `reproducido_en` = inicio de la escucha
- [x] Ciclo de envío cada 30 s con lotes de hasta 50 pendientes por servicio ordenados por `reproducido_en`, y reposo del hilo sin pendientes
- [x] Planificador de reintentos exponencial 30 s · 2^intentos con tope 6 h y descarte tras 20 intentos
- [x] Errores de autenticación (401/403, Last.fm 4/9/14) reprograman a +24 h, guardan `scrobbling_ultimo_error` y muestran toast con el servicio afectado
- [x] Descarte de scrobbles Last.fm con `reproducido_en` de más de 14 días o timestamp futuro, y de envíos con firma inválida (Last.fm 13)
- [x] Respuesta parcial de Last.fm (`ignored`) marca solo ese envío como descartado con el motivo
- [x] Lote ListenBrainz rechazado (400) se reintenta elemento a elemento para aislar y descartar el defectuoso
- [x] Toda petición HTTP con `ureq` (rustls), timeout 10 s, `User-Agent: mmmusic/<versión>`, solo a `api.listenbrainz.org` y `ws.audioscrobbler.com`
- [x] Tokens, `api_secret` y `session_key` nunca aparecen en logs, toasts ni `error_msg`
- [x] Los envíos pendientes sobreviven al cierre y se envían en el siguiente arranque

### ListenBrainz
- [x] `POST /1/submit-listens` con `listen_type = playing_now` (sin `listened_at`) y `import` (con `listened_at` en segundos Unix), cabecera `Authorization: Token`
- [x] `track_metadata` con artist_name, track_name, release_name y `additional_info` (duration_ms, media_player "mmmusic", submission_client "mmmusic")
- [x] `GET /1/validate-token` para `probar-servicios`

### Last.fm
- [x] Firma `api_sig` = md5 de parámetros ordenados + secret, `format=json`, con test unitario contra un vector conocido
- [x] `track.updateNowPlaying`, `track.scrobble` por lotes con parámetros indexados `[i]`, `track.love`, `track.unlove` y `user.getInfo`
- [x] Envío de título, artista y álbum originales (no normalizados) y `duration` en segundos

### Favoritas
- [x] `L` alterna la favorita de la pista seleccionada o, sin selección de pista, de la que suena; se ignora sobre artistas, álbumes y playlists
- [x] Encolado de `love`/`unlove` para Last.fm al alternar, descartando el pendiente anterior de la misma pista
- [x] Pseudo-playlist "♥ Favoritas" primera en la sección Playlists, no editable con `R`, `D`, `J`/`K` ni `d`
- [x] Detalle de ♥ Favoritas con reproducción en contexto, `a`, `A`, `P` y exportación M3U8 como `Favoritas.m3u8`
- [x] Barra inferior muestra ♥ junto al título cuando la pista actual es favorita
- [x] Toast "♥ <título>" / "Quitada de favoritas" al alternar

### Interfaz
- [x] Indicador de scrobbling en la barra inferior: `↑` enviado, `…N` pendientes, `!` error, nada si está desactivado, con equivalentes `sc:ok` / `sc:N` / `sc:err` en modo ascii
- [x] En modo compacto (< 70 columnas) el indicador solo muestra `!` si hay error
- [x] `Ctrl+s` fuerza el envío inmediato de pendientes
- [x] Vista Inicio con tres bloques en rejilla de tarjetas 4×3 con carátula, `h`/`l` dentro del bloque y `j`/`k` entre bloques
- [x] Inicio vuelve al formato de texto de F1 con `caratulas = false` o alto < 20 filas
- [x] Overlay de ayuda con la sección "Scrobbling y favoritas" (`L`, `Ctrl+s`, subcomandos de CLI)
- [x] Cabecera de álbum y vista Artistas muestran "Varios artistas" en los recopilatorios, manteniendo el artista real por pista en la lista

### Calidad y entrega
- [x] Tests unitarios de regla de scrobble, planificador de reintentos, clasificación de respuestas, firma Last.fm y cuerpo JSON de ListenBrainz
- [x] Test de integración de recopilatorios con `tests/fixtures/recopilatorio/` (3 pistas, artistas distintos, sin albumartist) y de su reversión al etiquetar `albumartist`
- [x] Test de persistencia de ENVIOS y de descarte de love/unlove anteriores
- [x] `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios
- [x] README actualizado: scrobbling, autorización de Last.fm, copia de seguridad de `credenciales.toml`, reescaneo completo
## Pendientes y bloqueos

- **Sin funcionalidades pendientes del checklist (59/59).**
- Validaciones manuales que quedan en manos del desarrollador en su Omarchy real:
  - Scrobbling real contra ListenBrainz y Last.fm (R-7): falta comprobar con cuentas reales `playing_now`, `import`, `track.scrobble`, love/unlove y el flujo de `autorizar-lastfm` con navegador. Las pruebas del agente son sin red.
  - `Ctrl+s` y la recuperación de la cola tras cortes de red con los servicios activos.
  - Inicio con tarjetas en Ghostty/Kitty (protocolo Kitty) y con `caratulas = false` / terminales bajos.
  - Objetivo de rendimiento del reescaneo completo (< 90 s con 10.000 pistas) y de la consolidación (< 2 s con 50.000 pistas), no medidos con una biblioteca real.
- Riesgos abiertos: R-7 (integraciones reales pendientes de validar) y R-9 (Last.fm requiere API key propia del usuario).

## Ejecución y pruebas

```bash
# Requisitos de sistema (Arch): mpv, pkgconf y rustup/cargo
cargo run                          # arranca la TUI
cargo test                         # unitarios + integración (fixtures, sin red)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

- La base de datos, la migración 002 y la bandera de reescaneo se aplican solas al arrancar (`~/.local/share/mmmusic/mmmusic.db`). El primer arranque tras esta versión lanza un reescaneo completo para agrupar recopilatorios.
- Subcomandos: `mmmusic reescanear [--completo]`, `mmmusic probar-servicios`, `mmmusic autorizar-lastfm`; `--version` y `--help` de clap.
- Credenciales: copia `credenciales.ejemplo.toml` a `~/.config/mmmusic/credenciales.toml` y complétala. mmmusic corrige los permisos a 600 y avisa si eran más abiertos.
- Para aislar pruebas manuales usa `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` y `XDG_STATE_HOME` apuntando a un directorio temporal.
- Verificación hecha durante el desarrollo: suite completa (60 unitarios + 24 de integración), `clippy` y `fmt` limpios; migración y consolidación end-to-end desde la CLI con los 3 fixtures (`Varios artistas`, 2019, bandera a 0); arranque de la TUI en tmux con navegación de Pistas, favoritas (`L`, pseudo-playlist, corazón, toasts) e Inicio por tarjetas; salida limpia con `q` (parada de reproductor, scrobbling y MPRIS registrada en el log).
