# mmmusic - Fase 5: Radio y Streams — Informe de implementación

**Estado:** fases completadas según checklist (60/60). Sujeto a validación manual
del desarrollador en Omarchy (red real, streams y Radio Browser).
**Fecha:** 13 de septiembre de 2026.

## Resumen de lo implementado

Se implementa la fase 5 completa: migración `004_radio.sql` con las tablas
EMISORAS, EMISORA_TITULOS y BUSQUEDAS_RADIO y la recreación de COLA,
HISTORIAL_REPRODUCCION y ENVIOS para admitir emisoras; cola mixta
(`ElementoCola { Pista | Emisora }`) en reproductor, persistencia y UI;
reproducción de streams con `libmpv` (caché, reconexión automática con esperas
1-2-4-8-16-30 s y 6 intentos, avance de espejo para listas PLS/M3U); estados de
stream publicados (`conectando`, `almacenando`, `en_directo`, `reconectando(n)`,
`rendido`) con timeout configurable; metadatos ICY parseados y filtrados, con
historial por emisora (50 títulos), pestaña Sonando y búsqueda `f` en la
biblioteca; scrobbling de radio (now playing inmediato y envío a los 30 s en
directo, sin duración y con `album` = emisora) reutilizando el planificador de
la fase 2; gestión completa de emisoras (alta, edición, borrado, favoritas,
importación/exportación PLS/M3U); directorio Radio Browser en un hilo propio
(espejos por DNS, caché de 24 h, click diario, logos acotados); y la sección
`8 Radio` con cuatro pestañas, barra inferior en modo directo, cola con `◉`,
modo ascii, MPRIS sin `length`/`CanSeek` y ayuda.

El cliente HTTP compartido vive ahora en `red/cliente.rs`, con la lista de
hosts ampliada a los espejos de Radio Browser y la descarga acotada de logos
como única excepción. La migración gestiona una copia previa
`mmusic.db.pre-004` y recrea tablas con las claves foráneas desactivadas y
verificación posterior de integridad.

## Desviaciones respecto a la especificación (qué y por qué)

1. **Observación de `metadata`.** `libmpv2` 6 no soporta `MPV_FORMAT_NODE` en
   los eventos de propiedad (`PropertyData` no convierte nodos; intentarlo hace
   `unimplemented!()`). Se observa `metadata/by-key/icy-title` como texto, que
   es equivalente funcional para el único campo que se usa.
2. **Carga de listas remotas.** Para `.pls`/`.m3u` se intenta `loadlist` y se
   cae a `loadfile` si el comando falla; para `.m3u8` se intenta `loadfile`
   (HLS es el caso habitual) y se cae a `loadlist` si falla. La especificación
   describía `loadlist` primero en todos los casos con fallback a `loadfile`
   para HLS; el comportamiento final es equivalente y prueba ambas vías.
3. **Espejos de Radio Browser.** Se implementa un resolutor DNS propio en Rust
   estándar (`radio/espejos.rs`): consulta directa de
   `all.api.radio-browser.info` y PTR por UDP contra `/etc/resolv.conf`, con
   respaldo a `127.0.0.53` y al nombre agregado si no hay respuestas inversas.
   No se añade ningún crate. `url_permitida` acepta el host agregado y
   cualquier subdominio `.api.radio-browser.info` (los nombres se resuelven en
   ejecución; no existe una lista fija que validar).
4. **Logos en resultados de Buscar.** La API del informe
   (`ComandoDirectorio::Logo { emisora_id, url }`) exige un id de EMISORAS, así
   que los logos solo se descargan para emisoras guardadas; los resultados no
   guardados muestran el marcador `♪` hasta que se guardan con `Enter`/`a`/`L`.
5. **Importación de PLS/M3U.** Al no existir explorador de ficheros en la TUI,
   `i` abre un diálogo de ruta (admite `~` y variables) con la carpeta de
   playlists propuesta. Decisión acordada con el desarrollador antes de
   implementar.
6. **Click de Radio Browser.** El click (`/json/url/{uuid}`) se envía al
   reproducir una emisora desde Favoritas/Todas/Buscar; al restaurar la cola
   al arrancar no se envía porque el hilo reproductor no conoce el hilo de
   directorio ni el uuid de la emisora. Una vez por emisora y día natural.
7. **Recreación de tablas y claves foráneas.** `bd::migrar` desactiva
   `foreign_keys` durante cada migración y ejecuta `PRAGMA foreign_key_check`
   al terminar. Sin ello, el `DROP` implícito de HISTORIAL_REPRODUCCION
   dispararía `ON DELETE SET NULL` sobre ENVIOS y se perderían enlaces. Afecta
   también a migraciones aplicadas desde cero (comportamiento verificado por
   la suite).
8. **`ElementoCola` se resuelve en la UI.** Los comandos
   `ReemplazarCola`/`AnadirAlFinal`/`ReproducirSiguiente` llevan
   `Vec<ElementoCola>` ya completos (las pistas se resuelven con
   `consultas::pistas_resumen_por_ids`), en vez de ids resueltos en el hilo
   reproductor.
9. **`EstadoReproduccion` amplía dos campos no listados en el informe:**
   `cache_segundos` y `reconexiones`, necesarios para cumplir el requisito de
   la pestaña Sonando (estado, reconexiones y caché).
10. **Nuevo `Dialogo::Formulario`** en la UI para alta/edición de emisoras
    (Nombre, URL y, al editar, Página web), con validación y error en línea.
11. **`url_permitida` se comprueba también en Last.fm** (`post` y `get`), como
    pedía la regla de red del proyecto; antes solo lo hacía ListenBrainz.
12. **`User-Agent`**: ahora `mmmusic/<versión> (+https://codeberg.org/4d3/mmmusic)`
    (antes `mmmusic/<versión>`), como exige la etiqueta de Radio Browser.

## Estructura de archivos creada/modificada

**Nuevos**

- `src/red/mod.rs`, `src/red/cliente.rs` — cliente HTTP compartido, allowlist y
  descarga acotada de logos.
- `src/radio/mod.rs` — API de emisoras y listas (validación, importación,
  exportación).
- `src/radio/icy.rs` — parseo de títulos ICY y filtros de ruido.
- `src/radio/listas.rs` — parseo/escritura PLS y M3U/M3U8 (sin E/S).
- `src/radio/reconexion.rs` — política de reintentos y espejos (sin E/S).
- `src/radio/espejos.rs` — resolutor DNS directo/PTR en Rust estándar.
- `src/radio/radiobrowser.rs` — hilo de directorio (búsqueda, caché, click,
  logos).
- `src/ui/vistas/radio.rs` — sección 8 con las cuatro pestañas.
- `src/biblioteca/migraciones/004_radio.sql`.
- `tests/icy.rs`, `tests/listas_radio.rs`, `tests/reconexion.rs`,
  `tests/radio.rs`.

**Modificados**

- `src/biblioteca/bd.rs` — migración 004, copia previa, FK off/check.
- `src/biblioteca/modelos.rs` — `Emisora`, `EmisoraResumen`, `TituloEmisora`,
  `ElementoCola`.
- `src/biblioteca/consultas.rs` — módulos `emisoras`, `titulos_emisora`,
  `busquedas_radio`, cola mixta, historial ICY y envíos con origen.
- `src/biblioteca/caratulas.rs` — reutilización del pipeline de logos.
- `src/config.rs` — sección `[radio]`, `Rutas.cache_logos`.
- `config.ejemplo.toml` — sección `[radio]`.
- `src/eventos.rs` — `ResultadosRadio` y `LogoListo`; estado de reproductor
  en caja para no engordar el enum.
- `src/reproductor/cola.rs`, `estado.rs`, `mpv.rs`, `mod.rs`.
- `src/scrobbling/mod.rs`, `lastfm.rs`, `listenbrainz.rs` — comandos ICY,
  duración opcional y cliente compartido.
- `src/mpris.rs` — metadatos de radio y `CanSeek`.
- `src/app.rs` — estado, atajos, diálogos y lógica de la sección Radio.
- `src/ui/teclas.rs`, `sidebar.rs` (numeración), `barra_inferior.rs`,
  `ui/mod.rs`, `ui/vistas/mod.rs`, `ui/vistas/cola.rs`,
  `ui/vistas/ayuda.rs`, `ui/componentes/dialogo.rs`,
  `ui/componentes/imagen.rs`.
- `src/cli.rs`, `src/main.rs`, `src/lib.rs`.
- `README.md`.
- `docs/fase5/mmmusic-fase5-checklist.md` (marcado).

## Decisiones técnicas tomadas durante el desarrollo

- **Copia previa centralizada.** `bd::abrir_y_migrar` solo crea
  `mmmusic.db.pre-004` (con `wal_checkpoint(TRUNCATE)` antes de copiar) y
  `bd::limpiar_copia_previa_si_migrada` la borra en el siguiente arranque; así
  los hilos que abren su propia conexión no la eliminan en la misma sesión.
- **Migraciones con `foreign_keys = OFF` y verificación** (ver desviación 7).
- **Resolutor DNS propio** en vez de añadir un crate, con el codificador y el
  decodificador de mensajes DNS cubiertos por tests de bytes sintéticos.
- **`ElementoCola` en `biblioteca::modelos`** para que consultas, reproductor,
  UI y scrobbling compartan el tipo sin dependencias cruzadas.
- **Máquina de estados de stream en el hilo reproductor** con
  `PlanReconexion` puro y testeable; el timeout usa
  `config.radio.espera_conexion_s`.
- **Logos reutilizan `CacheCaratulas`** con claves negativas (`-emisora_id`)
  para no colisionar con ids de álbum; el hilo de directorio genera el JPEG
  300×300 con `caratulas::cachear` y actualiza `logo_ruta`.
- **El historial ICY se registra en el hilo reproductor** y el scrobbler
  reparsea el título desde `HISTORIAL_REPRODUCCION` al recibir
  `TituloIcyCompletado`, de modo que ENVIOS no duplica campos derivados.
- **Duración y álbum opcionales** en los clientes de scrobbling (`Cancion`
  con `duracion_ms: Option`, `InfoAdicional.duration_ms` omitido en JSON)
  para los envíos de radio.
- **Búsqueda con caché en la UI**: la clave de `BUSQUEDAS_RADIO` se calcula
  con `radiobrowser::clave_busqueda` y la UI pinta "caché, hace N h" a partir
  de `obtenido_en`.
- **Modo ascii** ampliado con `*`, `~`, `o` y `(R)` y textos de estado
  siempre visibles.

## Funcionalidades del checklist completadas (copiando su texto exacto)

### Migración y modelo de datos

- Migración `004_radio.sql`: tablas EMISORAS, EMISORA_TITULOS y BUSQUEDAS_RADIO con sus índices y cascadas
- Recreación de COLA, HISTORIAL_REPRODUCCION y ENVIOS con `pista_id` NULL admisible, `emisora_id` (cascada) y, en COLA, `tipo`; en HISTORIAL, `titulo_icy`; preservando todas las filas dentro de una transacción
- Copia previa `mmmusic.db.pre-004` antes de la migración, borrada en el siguiente arranque correcto
- Regla en código: en COLA, HISTORIAL_REPRODUCCION y ENVIOS exactamente uno de `pista_id` / `emisora_id` es no nulo
- Claves `radiobrowser_servidor` y `radio_pestana` en AJUSTES sembradas por código

### Cola mixta y reproducción de streams

- `ElementoCola` con variantes Pista y Emisora; `ReemplazarCola`, `AnadirAlFinal` y `ReproducirSiguiente` aceptan ambos tipos; persistencia y restauración de la cola con `tipo`
- mpv con `cache=yes`, `demuxer-max-bytes=32MiB` y `stream-lavf-o=reconnect=1,reconnect_streamed=1,reconnect_delay_max=5` para elementos emisora
- URLs de listas PLS/M3U/M3U8 remotas cargadas con `loadlist` y sus entradas tratadas como espejos; reintento con `loadfile` si no era una lista de emisoras (HLS)
- Estados de stream conectando, almacenando, en_directo, reconectando(n) y rendido publicados en `EstadoReproduccion` a partir de `paused-for-cache`, `cache-buffering-state`, `file-loaded` y `end-file`
- Timeout de conexión `espera_conexion_s` (15 s) y política de reconexión 1, 2, 4, 8, 16, 30 s con 6 intentos, avanzando de espejo en cada reintento y reiniciando el contador al llegar a en_directo
- Estado rendido: toast "No se pudo conectar con <emisora>", `EMISORAS.ultimo_error` y paso al siguiente elemento de la cola o detenido
- Seeks ignorados con emisora; pausa ignorada en conectando/almacenando; `Reanudar` tras más de 60 s de pausa recarga el stream
- Precarga gapless desactivada cuando el elemento actual o el siguiente es una emisora
- Anterior sobre una emisora va siempre al elemento anterior; una emisora nunca avanza sola salvo por rendido
- Al restaurar la cola al arrancar con una emisora como elemento actual, se queda detenida sin conectar hasta pulsar Espacio
- El conteo de tres fallos seguidos de F1 se reinicia al cambiar de tipo de elemento y no aplica a emisoras
- `EMISORAS.codec` y `bitrate_kbps` se rellenan desde `audio-codec-name` y `audio-bitrate` si estaban vacíos, y `ultima_reproduccion` al conectar

### Metadatos ICY, títulos e historial

- Observación de `metadata` y extracción de `icy-title` con publicación de `titulo_icy` y `tiempo_escuchando_ms` en `EstadoReproduccion`
- `icy::parsear` sin E/S: limpieza de espacios, descarte de vacío, del nombre de la emisora, de ruido (advert, jingle, station id, URLs) y separación "Artista - Título" por el primer " - "
- Inserción en EMISORA_TITULOS con poda a los 50 más recientes por emisora, sin duplicar títulos consecutivos
- Fila en HISTORIAL_REPRODUCCION con `emisora_id` y `titulo_icy` solo para títulos con artista; los títulos sin artista no generan historial
- Marca `completada = 1` tras 30 s en estado en_directo con el mismo título; un cambio de título antes deja la fila sin completar
- En la pestaña Sonando se marca ✓ el título cuyo artista y título existen en la biblioteca

### Scrobbling de radio

- `ComandoScrobbling::TituloIcy` y `TituloIcyCompletado`; now playing con `album` = nombre de la emisora al reconocer un título con artista
- ENVIOS de tipo scrobble con `emisora_id` e `historial_id`, tomando artista y título del historial y sin `duration`
- Los envíos de radio siguen el mismo planificador y reintentos de F2

### Gestión de emisoras

- `emisoras::{listar, buscar_local, crear, editar, eliminar, alternar_favorita, marcar_reproducida, fijar_codec, fijar_logo, por_uuid, por_url}`
- Alta manual `N` con diálogo Nombre + URL y validación (http/https, host, ≤ 2048, única normalizada, nombre 1-100)
- Edición `R` de nombre, URL y página web; eliminación `D` con confirmación y aviso de cascada
- `L` marca o desmarca favorita la emisora seleccionada, o la que suena si no hay pista seleccionada
- Importación `i` de PLS y M3U/M3U8 locales: nombre desde Title/EXTINF o host, duplicadas por URL omitidas, toast con recuento
- Exportación `e` de favoritas a `Radio favoritas.m3u8` con `#EXTINF:-1,<nombre>` y URL, con confirmación de sobreescritura
- `listas.rs` sin E/S con parseo y escritura de PLS y M3U/M3U8 testeados

### Radio Browser

- `red/cliente.rs` compartido por scrobbling y directorio: constructor `ureq`, timeout, User-Agent `mmmusic/<versión> (+https://codeberg.org/4d3/mmmusic)` y `url_permitida()` con los espejos de Radio Browser añadidos
- Hilo de directorio con `ComandoDirectorio` (Buscar, Click, Logo, Apagar) y respuestas por `AppEvento` sin bloquear la UI
- Resolución DNS de `all.api.radio-browser.info`, elección aleatoria de espejo guardada 24 h en AJUSTES y cambio de espejo ante fallo (máximo 3 por búsqueda)
- Búsqueda `GET /json/stations/search` por nombre, país y etiqueta con `order=votes`, `reverse=true`, `limit=100` y `hidebroken=true`, descartando `lastcheckok = 0`
- Caché en BUSQUEDAS_RADIO por clave normalizada con validez 24 h, poda a 7 días e indicación "caché, hace N h" en la interfaz
- Con red caída y caché disponible se muestran los resultados con aviso; sin caché, toast; ante 429, pausa de 5 minutos
- Añadir desde resultados: `url_resolved` o `url`, nombre, país, etiquetas, codec, bitrate, favicon, homepage y uuid; sin duplicar por uuid ni URL
- `GET /json/url/{uuid}` al reproducir una emisora del directorio, una vez por emisora y día
- `radio.directorio = false` desactiva el hilo de directorio y la pestaña Buscar solo busca en emisoras locales

### Logos

- Descarga de logos en el hilo de directorio limitada a `image/*`, 512 KB, 5 s y 3 redirecciones, sin reintento en la misma sesión
- Logos cacheados a 300×300 en `~/.cache/mmmusic/logos/{id}.jpg` con el pipeline de carátulas y `logo_ruta` actualizada; marcador ♪ mientras carga; `radio.logos = false` los desactiva
- Nombres y URLs del directorio limpiados de caracteres de control antes de mostrarse

### Interfaz

- Sección "8 Radio" en la sidebar, accesible con `8`, con pestañas Favoritas, Todas, Buscar y Sonando; `[` / `]` cambian de pestaña y la última se recuerda en AJUSTES
- Listas Favoritas y Todas con logo, nombre, país, codec/bitrate y última reproducción; orden por nombre o por última reproducción
- Pestaña Buscar con campos Nombre, País y Etiqueta (`Tab` entre campos, `/` enfoca Nombre), resultados con votos, estado "buscando…" y "caché"
- Pestaña Sonando con datos de la emisora, estado de conexión, reconexiones, caché y últimos 50 títulos con hora
- `f` busca en la biblioteca el título ICY actual o el seleccionado en Sonando, abriendo Buscar con el texto
- `Enter`, `a` y `A` sobre emisoras en Favoritas, Todas y Buscar; en Buscar, `Enter` y `a` guardan la emisora
- Barra inferior en modo directo: icono de estado (●, ⟳, ◌), nombre de la emisora, título ICY, "EN DIRECTO · hh:mm:ss · codec" en lugar de la barra de progreso, y logo
- Elementos emisora en el panel de cola con `◉` y sin duración
- Modo `ascii` para los iconos nuevos (`*`, `~`, `o`, `(R)`) y estado de conexión siempre con texto
- MPRIS con emisora: título ICY o nombre, artista parseado, álbum = emisora, sin `mpris:length`, `CanSeek = false`, `artUrl` = logo
- Overlay de ayuda con la sección Radio

### Calidad y entrega

- Tests sin red: `icy.rs` (parseo y ruido), `listas_radio.rs` (PLS/M3U ida y vuelta), `reconexion.rs` (esperas, espejos, rendido), `radio.rs` (migración 004 con datos, cola mixta, historial/envíos ICY, caché de búsquedas)
- Sección `[radio]` en config (directorio, logos, espera_conexion_s) validada y en `config.ejemplo.toml`
- README: radio, Radio Browser, importación PLS/M3U, límites de logos y política de red
- `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios
## Pendientes y bloqueos

- **Validación manual en Omarchy**: probar una URL real con corte de red,
  HLS, una lista PLS/M3U remota con espejos, Radio Browser con red y los
  logos de emisoras guardadas. La suite no toca la red por diseño.
- **Click al restaurar la cola**: al arrancar con una emisora del directorio
  como elemento actual no se registra el click (ver desviación 6). Si se
  considera necesario, requeriría llevar el uuid en `EmisoraResumen` y un
  canal del reproductor al hilo de directorio.
- **DNS inverso**: el resolutor usa `/etc/resolv.conf` (o `127.0.0.53`); en
  redes con resolutores que no respondan a PTR cae al nombre agregado, que
  sigue siendo funcional.
- **Logos de resultados sin guardar**: placeholder `♪` (ver desviación 4).

## Ejecución y pruebas

```bash
cargo run            # arranca la TUI; migra a user_version = 4 con copia previa
cargo run -- reescanear --completo
cargo test           # 60+ tests, ninguno toca la red
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Para pruebas manuales aisladas, exporta `XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
`XDG_CACHE_HOME` y `XDG_STATE_HOME` a un directorio temporal. Tras el primer
arranque correcto con la base ya migrada, `mmusic.db.pre-004` desaparece del
directorio de datos. Los tests de radio cubren el parseo ICY, el ida y vuelta
PLS/M3U (incluida la importación con duplicadas e inválidas), la política de
reconexión, la migración 004 con datos reales y copia previa, la cola mixta,
el historial/envíos de radio sin duración y la caché de búsquedas.
