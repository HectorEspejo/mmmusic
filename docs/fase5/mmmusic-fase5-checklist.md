# mmmusic - Checklist Fase 5: Radio y Streams

## Migración y modelo de datos
- [x] Migración `004_radio.sql`: tablas EMISORAS, EMISORA_TITULOS y BUSQUEDAS_RADIO con sus índices y cascadas
- [x] Recreación de COLA, HISTORIAL_REPRODUCCION y ENVIOS con `pista_id` NULL admisible, `emisora_id` (cascada) y, en COLA, `tipo`; en HISTORIAL, `titulo_icy`; preservando todas las filas dentro de una transacción
  - AC: Dada una base con cola, historial y envíos de F2, cuando se aplica la migración 004, entonces los recuentos de las tres tablas son idénticos y `user_version = 4`
- [x] Copia previa `mmmusic.db.pre-004` antes de la migración, borrada en el siguiente arranque correcto
- [x] Regla en código: en COLA, HISTORIAL_REPRODUCCION y ENVIOS exactamente uno de `pista_id` / `emisora_id` es no nulo
- [x] Claves `radiobrowser_servidor` y `radio_pestana` en AJUSTES sembradas por código

## Cola mixta y reproducción de streams
- [x] `ElementoCola` con variantes Pista y Emisora; `ReemplazarCola`, `AnadirAlFinal` y `ReproducirSiguiente` aceptan ambos tipos; persistencia y restauración de la cola con `tipo`
- [x] mpv con `cache=yes`, `demuxer-max-bytes=32MiB` y `stream-lavf-o=reconnect=1,reconnect_streamed=1,reconnect_delay_max=5` para elementos emisora
- [x] URLs de listas PLS/M3U/M3U8 remotas cargadas con `loadlist` y sus entradas tratadas como espejos; reintento con `loadfile` si no era una lista de emisoras (HLS)
- [x] Estados de stream conectando, almacenando, en_directo, reconectando(n) y rendido publicados en `EstadoReproduccion` a partir de `paused-for-cache`, `cache-buffering-state`, `file-loaded` y `end-file`
- [x] Timeout de conexión `espera_conexion_s` (15 s) y política de reconexión 1, 2, 4, 8, 16, 30 s con 6 intentos, avanzando de espejo en cada reintento y reiniciando el contador al llegar a en_directo
  - AC: Dada una emisora sonando, cuando se corta la red 5 s y vuelve, entonces la barra muestra "reconectando" y después "EN DIRECTO" sin intervención del usuario
- [x] Estado rendido: toast "No se pudo conectar con <emisora>", `EMISORAS.ultimo_error` y paso al siguiente elemento de la cola o detenido
- [x] Seeks ignorados con emisora; pausa ignorada en conectando/almacenando; `Reanudar` tras más de 60 s de pausa recarga el stream
- [x] Precarga gapless desactivada cuando el elemento actual o el siguiente es una emisora
- [x] Anterior sobre una emisora va siempre al elemento anterior; una emisora nunca avanza sola salvo por rendido
- [x] Al restaurar la cola al arrancar con una emisora como elemento actual, se queda detenida sin conectar hasta pulsar Espacio
- [x] El conteo de tres fallos seguidos de F1 se reinicia al cambiar de tipo de elemento y no aplica a emisoras
- [x] `EMISORAS.codec` y `bitrate_kbps` se rellenan desde `audio-codec-name` y `audio-bitrate` si estaban vacíos, y `ultima_reproduccion` al conectar

## Metadatos ICY, títulos e historial
- [x] Observación de `metadata` y extracción de `icy-title` con publicación de `titulo_icy` y `tiempo_escuchando_ms` en `EstadoReproduccion`
- [x] `icy::parsear` sin E/S: limpieza de espacios, descarte de vacío, del nombre de la emisora, de ruido (advert, jingle, station id, URLs) y separación "Artista - Título" por el primer " - "
- [x] Inserción en EMISORA_TITULOS con poda a los 50 más recientes por emisora, sin duplicar títulos consecutivos
- [x] Fila en HISTORIAL_REPRODUCCION con `emisora_id` y `titulo_icy` solo para títulos con artista; los títulos sin artista no generan historial
- [x] Marca `completada = 1` tras 30 s en estado en_directo con el mismo título; un cambio de título antes deja la fila sin completar
- [x] En la pestaña Sonando se marca ✓ el título cuyo artista y título existen en la biblioteca

## Scrobbling de radio
- [x] `ComandoScrobbling::TituloIcy` y `TituloIcyCompletado`; now playing con `album` = nombre de la emisora al reconocer un título con artista
- [x] ENVIOS de tipo scrobble con `emisora_id` e `historial_id`, tomando artista y título del historial y sin `duration`
  - AC: Dada una emisora en directo con scrobbling activo, cuando un título con artista lleva 30 s, entonces existe un ENVIO pendiente por servicio con ese artista y título y `album` = emisora
- [x] Los envíos de radio siguen el mismo planificador y reintentos de F2

## Gestión de emisoras
- [x] `emisoras::{listar, buscar_local, crear, editar, eliminar, alternar_favorita, marcar_reproducida, fijar_codec, fijar_logo, por_uuid, por_url}`
- [x] Alta manual `N` con diálogo Nombre + URL y validación (http/https, host, ≤ 2048, única normalizada, nombre 1-100)
- [x] Edición `R` de nombre, URL y página web; eliminación `D` con confirmación y aviso de cascada
- [x] `L` marca o desmarca favorita la emisora seleccionada, o la que suena si no hay pista seleccionada
- [x] Importación `i` de PLS y M3U/M3U8 locales: nombre desde Title/EXTINF o host, duplicadas por URL omitidas, toast con recuento
  - AC: Dado un PLS con 15 entradas de las que 2 ya existen y 1 no tiene URL válida, cuando se importa, entonces hay 12 emisoras nuevas y el toast lo indica
- [x] Exportación `e` de favoritas a `Radio favoritas.m3u8` con `#EXTINF:-1,<nombre>` y URL, con confirmación de sobreescritura
- [x] `listas.rs` sin E/S con parseo y escritura de PLS y M3U/M3U8 testeados

## Radio Browser
- [x] `red/cliente.rs` compartido por scrobbling y directorio: constructor `ureq`, timeout, User-Agent `mmmusic/<versión> (+https://codeberg.org/4d3/mmmusic)` y `url_permitida()` con los espejos de Radio Browser añadidos
- [x] Hilo de directorio con `ComandoDirectorio` (Buscar, Click, Logo, Apagar) y respuestas por `AppEvento` sin bloquear la UI
- [x] Resolución DNS de `all.api.radio-browser.info`, elección aleatoria de espejo guardada 24 h en AJUSTES y cambio de espejo ante fallo (máximo 3 por búsqueda)
- [x] Búsqueda `GET /json/stations/search` por nombre, país y etiqueta con `order=votes`, `reverse=true`, `limit=100` y `hidebroken=true`, descartando `lastcheckok = 0`
- [x] Caché en BUSQUEDAS_RADIO por clave normalizada con validez 24 h, poda a 7 días e indicación "caché, hace N h" en la interfaz
- [x] Con red caída y caché disponible se muestran los resultados con aviso; sin caché, toast; ante 429, pausa de 5 minutos
- [x] Añadir desde resultados: `url_resolved` o `url`, nombre, país, etiquetas, codec, bitrate, favicon, homepage y uuid; sin duplicar por uuid ni URL
- [x] `GET /json/url/{uuid}` al reproducir una emisora del directorio, una vez por emisora y día
- [x] `radio.directorio = false` desactiva el hilo de directorio y la pestaña Buscar solo busca en emisoras locales

## Logos
- [x] Descarga de logos en el hilo de directorio limitada a `image/*`, 512 KB, 5 s y 3 redirecciones, sin reintento en la misma sesión
- [x] Logos cacheados a 300×300 en `~/.cache/mmmusic/logos/{id}.jpg` con el pipeline de carátulas y `logo_ruta` actualizada; marcador ♪ mientras carga; `radio.logos = false` los desactiva
- [x] Nombres y URLs del directorio limpiados de caracteres de control antes de mostrarse

## Interfaz
- [x] Sección "8 Radio" en la sidebar, accesible con `8`, con pestañas Favoritas, Todas, Buscar y Sonando; `[` / `]` cambian de pestaña y la última se recuerda en AJUSTES
- [x] Listas Favoritas y Todas con logo, nombre, país, codec/bitrate y última reproducción; orden por nombre o por última reproducción
- [x] Pestaña Buscar con campos Nombre, País y Etiqueta (`Tab` entre campos, `/` enfoca Nombre), resultados con votos, estado "buscando…" y "caché"
- [x] Pestaña Sonando con datos de la emisora, estado de conexión, reconexiones, caché y últimos 50 títulos con hora
- [x] `f` busca en la biblioteca el título ICY actual o el seleccionado en Sonando, abriendo Buscar con el texto
- [x] `Enter`, `a` y `A` sobre emisoras en Favoritas, Todas y Buscar; en Buscar, `Enter` y `a` guardan la emisora
- [x] Barra inferior en modo directo: icono de estado (●, ⟳, ◌), nombre de la emisora, título ICY, "EN DIRECTO · hh:mm:ss · codec" en lugar de la barra de progreso, y logo
- [x] Elementos emisora en el panel de cola con `◉` y sin duración
- [x] Modo `ascii` para los iconos nuevos (`*`, `~`, `o`, `(R)`) y estado de conexión siempre con texto
- [x] MPRIS con emisora: título ICY o nombre, artista parseado, álbum = emisora, sin `mpris:length`, `CanSeek = false`, `artUrl` = logo
- [x] Overlay de ayuda con la sección Radio

## Calidad y entrega
- [x] Tests sin red: `icy.rs` (parseo y ruido), `listas_radio.rs` (PLS/M3U ida y vuelta), `reconexion.rs` (esperas, espejos, rendido), `radio.rs` (migración 004 con datos, cola mixta, historial/envíos ICY, caché de búsquedas)
- [x] Sección `[radio]` en config (directorio, logos, espera_conexion_s) validada y en `config.ejemplo.toml`
- [x] README: radio, Radio Browser, importación PLS/M3U, límites de logos y política de red
- [x] `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios

---

**Progreso Fase 5:** 60 / 60 funcionalidades

**Total mmmusic (Fases 1-3 y 5):** 290 / 290 funcionalidades
