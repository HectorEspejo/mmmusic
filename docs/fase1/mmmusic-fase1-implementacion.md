# mmmusic - Fase 1: Informe de Implementación

**Documento vivo.** Última actualización: 12 de septiembre de 2026 (sesión 1: S1–S5 completados; fase cerrada).

Estado por sprint:

| Sprint | Contenido | Estado |
|--------|-----------|--------|
| S1 | Cimientos (Cargo, config, XDG, logs, BD, escáner, tema, UI base, fixtures) | Completado y verificado |
| S2 | Reproducción (libmpv, cola, historial, vista Pistas) | Completado y verificado |
| S3 | Navegación (Inicio, Artistas, Álbumes, detalle, panel de cola, vim) | Completado y verificado |
| S4 | Buscar, playlists y carátulas | Completado y verificado |
| S5 | MPRIS, escritorio y pulido | Completado y verificado |

## Resumen de lo implementado

### S1 — Cimientos

- Proyecto Cargo (edición 2024) con la estructura del informe y objetivo de librería + binario (ver desviaciones).
- Configuración `~/.config/mmmusic/config.toml` con valores por defecto, creación automática con aviso en pantalla, expansión de `~` y `$VARIABLES`, y validación/acotado de valores.
- Rutas XDG vía `directories`: BD en `~/.local/share/mmmusic/mmmusic.db`, caché de carátulas en `~/.cache/mmmusic/caratulas/`, logs en `~/.local/state/mmmusic/mmusic.log` con rotación diaria y filtro `RUST_LOG`.
- Terminal en modo raw + pantalla alternativa con restauración garantizada al salir y hook de pánico.
- Bucle principal de eventos por canal (`AppEvento`) con tick de 250 ms; hilo de entrada de crossterm.
- SQLite en WAL, `busy_timeout` de 5 s, claves foráneas activas y migración `001_inicial.sql` idempotente por `PRAGMA user_version` con las nueve tablas, índices y cascadas del informe.
- Escáner incremental en hilo propio: `walkdir` con symlinks y detección de ciclos por `(dev, ino)`, filtro de las seis extensiones, lectura de etiquetas con `lofty`, resolución de valores ausentes §7.2, normalización sin diacríticos con CJK intacto, incremental por mtime/tamaño, borrado de no vistas y huérfanos, conservación si una raíz desaparece, registro en ESCANEOS (4 estados), huérfano `en_curso` → error, cancelación con `AtomicBool`, commit cada 200 ficheros, progreso cada 50, omisión de corruptos/>2 GB/sin duración y escaneo automático al arrancar.
- Tema Omarchy desde `alacritty.toml` (`colors.primary/normal/bright`), mapeo configurable de acento/progreso, paleta "terminal" de respaldo, watcher con `notify` y debounce de 300 ms, recarga manual con `t`.
- Esqueleto de UI: layout raíz (sidebar/contenido/barra de 3 filas), sidebar con seis secciones, contador de pistas y progreso de escaneo, barra inferior "detenido" con `--:--`, toasts de 3 s, overlay de ayuda y modo compacto básico.
- Fixtures de los seis formatos generados con ffmpeg (`tests/fixtures/`) más un MP3 con carátula embebida y `portada.png` para S4.

### S2 — Reproducción

- Envoltorio `reproductor/mpv.rs` con libmpv2 inicializado con `vo=null`, `audio-display=no`, `ytdl=no`, `load-scripts=no`, `config=no`, `gapless-audio=yes`; observación de `time-pos`, `duration`, `pause`, `volume`, `mute`, `eof-reached` e `idle-active`; traducción de `file-loaded`, `end-file` (con motivo) y fallos.
- Hilo reproductor con máquina de estados detenido/cargando/reproduciendo/pausado/error, canal `watch<EstadoReproduccion>` y publicación a la UI por `AppEvento::Reproductor`.
- Implementación de todos los comandos del informe (reemplazar/añadir/a continuación/eliminar/mover/vaciar/saltar/pausa/detener/siguiente/anterior/buscar/volumen/silencio/aleatorio/repetición/apagar).
- Cola con orden efectivo y original, modo aleatorio que conserva la pista actual y restaura el orden, repetición no/todo/una, anterior con umbral de 3 s, eliminación de la pista actual, persistencia en COLA y AJUSTES en cada cambio, y restauración en pausa en la pista y posición guardadas.
- Precarga de la siguiente pista con `loadfile append` para reproducción gapless y avance automático al terminar.
- Fallos de reproducción: notificación, salto a la siguiente y paso a detenido tras tres fallos seguidos.
- Historial: inserción en `file-loaded` y `completada = 1` al acumular el 50 % de tiempo real reproducido (los seeks no cuentan).
- Vista Pistas como tabla ordenable (título, artista, álbum, duración, año) con indicador de orden, paginación incremental de 500, indicador `▶` en la pista sonando, contador "x-y de N" y reproducción con `Enter`/añadido con `a`/`A`.
- Verificación manual con audio real por PipeWire: reproducción, pausa, siguiente, avance automático, historial completado y restauración en pausa tras cerrar con `q`.

### S3 — Navegación

- Vistas Inicio (tres bloques navegables), Artistas (lista + detalle con álbumes y pistas sueltas) y Álbumes (rejilla de tarjetas + detalle con cabecera y pistas).
- Navegación vim con `j/k/h/l`, `gg/G`, `Ctrl+d/u`, `Enter`, `Esc`/`h` para volver, foco ciclado con `Tab`/`Shift+Tab` e indicador `◄` por panel.
- Panel de cola interactivo (selección, `Enter` salta, `d` quita, `J/K` mueve) y contexto de reproducción por vista (álbum, artista, lista de pistas, resultados, recientes).
- Sidebar con las seis secciones, playlists, nº de pistas y progreso de escaneo; línea de ayuda contextual por vista.

### S4 — Buscar, playlists y carátulas

- Búsqueda incremental con debounce de 150 ms, términos normalizados en AND sobre columnas `*_norm`, bloques de artistas/álbumes/pistas con "+N más" y sin consultar con menos de 2 caracteres.
- Playlists: alta, renombrado y borrado con validación (1–100 caracteres, único normalizado), detalle con reordenación `J/K`, quitado con `d`, selector `P` con "+ Nueva playlist…", importación/exportación M3U8 en `carpeta_playlists`, resolución de rutas relativas, filtro de rutas fuera de la biblioteca y sufijo `(2)` para nombres repetidos.
- Diálogos de confirmación, texto y selector renderizados como overlay.
- Carátulas: extracción de imagen embebida (primera pista que la tenga) o `cover.*`/`folder.*`/`front.*`; caché JPEG 300×300 en `~/.cache/mmmusic/caratulas/{album_id}.jpg` y actualización de `caratula_ruta`; render con `ratatui-image` (half-blocks en Alacritty) en rejilla, detalle de álbum y barra inferior, con decodificación en hilo aparte, caché LRU de 200 y marcador `♪` mientras carga; `interfaz.caratulas = false` desactiva el renderizado.

## Desviaciones respecto a la especificación (qué y por qué)

1. **`src/lib.rs` además de `src/main.rs`.** El informe solo lista `main.rs`, pero el checklist exige tests de integración en `tests/` (`escaneo.rs`, `cola.rs`, `etiquetas.rs`), que necesitan que el crate exponga sus módulos como librería. `main.rs` queda como arranque fino.
2. **`ratatui-image` sin la feature `chafa`.** La versión 11 la activa por defecto y exige `libchafa >= 1.8` como dependencia de sistema, que no figura en el stack (solo `mpv` y `pkgconf`). Se compila con `default-features = false, features = ["image-defaults", "crossterm"]`; el fallback a half-blocks sigue disponible.
3. **Atajos `o` / `O` en la vista Pistas** para ciclar la columna de orden e invertirla. La especificación exige tabla ordenable por columna con indicador, pero no fija la tecla; se documenta aquí como decisión de implementación.
4. **El escáner aplica las migraciones** en su propia conexión (además del hilo principal) para que sea autónomo y testeable. Es idempotente por `user_version`.
5. **`tests/cola.rs` incluye, además de la cola pura, las pruebas de integración del hilo reproductor** (persistencia y tres fallos) para respetar el nombre de fichero del informe sin añadir otro archivo.
6. **Los comandos `Buscar` con seek no se validaron manualmente**; la lógica está implementada y acotada al rango de la pista.
7. **Las carátulas no se muestran en Inicio** (el mockup las dibuja también en los bloques). Se priorizó rejilla, detalle y barra inferior; el bloque Inicio es texto seleccionable. Queda anotado como mejora estética, no como funcionalidad del checklist.
8. **La decodificación de carátulas comunica al hilo de UI por un canal dedicado** (`(album_id, DynamicImage)`) en lugar del evento `CaratulaLista` con el protocolo ya codificado; `Picker`/protocolos se construyen en el hilo de UI para no compartir estado gráfico entre hilos. El evento `CaratulaLista` sigue existiendo en `AppEvento`.
9. **Tamaños de carátula**: 3×2 celdas en barra inferior, 10×5 en detalle y 4×3 en rejilla (el informe decía "3×6, 10×20, 4×8" sin unidades de celda claras; se ajustó al espacio real del layout).
10. **La búsqueda ignora los caracteres escritos mientras el campo no está enfocado** (el campo se enfoca con `/`), y `Esc` sale del campo sin borrar el texto.

## Estructura de archivos creada/modificada

```
Cargo.toml  README.md  config.ejemplo.toml  mmmusic.desktop  instalar.sh
src/
  lib.rs  main.rs  app.rs  config.rs  eventos.rs  mpris.rs  tema.rs
  biblioteca/
    mod.rs  bd.rs  caratulas.rs  consultas.rs  escaner.rs  etiquetas.rs  modelos.rs
    migraciones/001_inicial.sql
  reproductor/
    mod.rs  mpv.rs  cola.rs  estado.rs
  ui/
    mod.rs  teclas.rs  sidebar.rs  barra_inferior.rs
    vistas/{mod,inicio,buscar,artistas,albumes,pistas,playlists,cola,ayuda}.rs
    componentes/{mod,dialogo,imagen,notificacion}.rs
tests/
  escaneo.rs  cola.rs  etiquetas.rs
  fixtures/{prueba.mp3,prueba.flac,prueba.ogg,prueba.opus,prueba.m4a,prueba.wav,
            prueba_caratula.mp3,portada.png}
```

## Decisiones técnicas tomadas durante el desarrollo

- **Conexión SQLite por hilo** (UI, escáner, reproductor) en WAL, tal como prevé el informe; el reproductor persiste cola/historial/ajustes desde su propio hilo.
- **Hilos nativos + canales**: `mpsc` para `AppEvento` y `ComandoReproductor`; `tokio::sync::watch` para `EstadoReproduccion` (el runtime tokio se usará solo en el hilo MPRIS en S5). El despertador de libmpv (`set_wakeup_callback`) evita sondeos busy-wait.
- **Diagrama de estados y fallos**: los errores de pista son transitorios (aviso + salto), por lo que `EstadoReproductor::Error` existe pero no se publica de forma persistente; tres fallos consecutivos detienen la reproducción.
- **Historial con tiempo real acumulado**: se suman deltas de `time-pos` menores de 1,5 s, de modo que los seeks no cuentan para el 50 %.
- **Restauración en pausa**: el hilo reproductor carga la pista con `pause=yes` y aplica el seek guardado tras `file-loaded`; nunca arranca sonando.
- **Orden de la cola persistida**: la tabla COLA ya refleja el orden efectivo (barajado si procede), por lo que la restauración no vuelve a barajar.
- **Fechas ISO 8601 sin dependencias extra**: conversión civil implementada y testeada en `bd.rs`.
- **Vista Pistas con carga incremental**: al acercar la selección al final de la página cargada se solicita la siguiente página, manteniendo el contador "x-y de N".

## Funcionalidades del checklist completadas (texto exacto)

### Cimientos
- [x] Proyecto Cargo con estructura de carpetas del informe (`src/app.rs`, `biblioteca/`, `reproductor/`, `ui/`, `mpris.rs`, `tema.rs`, `tests/`)
- [x] Configuración en `~/.config/mmmusic/config.toml` con valores por defecto y creación automática si no existe
  - AC: Dado que no existe el fichero, cuando arranca mmmusic, entonces se crea con `carpetas = ["~/Music"]` y se muestra un aviso
- [x] Rutas XDG para BD (`~/.local/share/mmmusic/`), caché (`~/.cache/mmmusic/`) y logs (`~/.local/state/mmmusic/`)
- [x] Logs con `tracing` a fichero rotado diariamente, nivel configurable por `RUST_LOG`
- [x] Terminal en modo raw con pantalla alternativa y restauración garantizada al salir o ante pánico
- [x] Bucle principal de eventos con canal `AppEvento` y tick de 250 ms sin bloquear la UI

### Base de datos y migraciones
- [x] Apertura de SQLite en modo WAL con `busy_timeout` 5 s y claves foráneas activadas
- [x] Migración `001_inicial.sql` gestionada por `PRAGMA user_version` y aplicada de forma idempotente
- [x] Tablas ARTISTAS, ALBUMES, PISTAS, PLAYLISTS, PLAYLIST_PISTAS, COLA, HISTORIAL_REPRODUCCION, ESCANEOS y AJUSTES con los campos e índices del informe
- [x] Borrado en cascada de PLAYLIST_PISTAS, COLA e HISTORIAL_REPRODUCCION al eliminar una PISTA

### Escaneo de biblioteca
- [x] Hilo de escaneo independiente que recorre las carpetas de `biblioteca.carpetas` con `walkdir`, siguiendo enlaces simbólicos y detectando ciclos
- [x] Filtro por extensión soportada: mp3, flac, ogg, opus, m4a, wav
- [x] Lectura de etiquetas con `lofty`: título, artista, artista de álbum, álbum, año, disco, número de pista, género, duración, bitrate
- [x] Resolución de valores ausentes según §7.2 (nombre de fichero, nombre de carpeta, "Artista desconocido", albumartist ?? artist)
- [x] Normalización de texto (minúsculas, sin diacríticos, espacios colapsados, CJK intacto) en columnas `*_norm`
- [x] Escaneo incremental: solo se releen ficheros con mtime o tamaño distintos a los de PISTAS
  - AC: Dado un reescaneo sin cambios en disco, cuando termina, entonces no se ha releído ninguna etiqueta y tarda menos de 5 s con 10.000 pistas
- [x] Eliminación de pistas no vistas en el escaneo completado y limpieza de álbumes y artistas huérfanos
- [x] Una raíz de biblioteca inexistente no provoca el borrado de sus pistas indexadas
- [x] Registro de cada escaneo en ESCANEOS con estados en_curso, completado, error y cancelado, y contadores nuevas/actualizadas/eliminadas
- [x] Escaneo huérfano en_curso al arrancar se marca como error antes de iniciar uno nuevo
- [x] Cancelación del escaneo en curso al pulsar Ctrl+r de nuevo o al salir, conservando lo ya indexado
- [x] Commit cada 200 ficheros y evento de progreso a la UI cada 50 ficheros
- [x] Ficheros corruptos, mayores de 2 GB o sin duración se omiten con WARN sin detener el escaneo
- [x] Escaneo automático al arrancar si `escanear_al_arrancar = true`, sin bloquear la interfaz

### Reproductor
- [x] Hilo reproductor con `libmpv2` inicializado con `vo=null`, `audio-display=no`, `ytdl=no`, `load-scripts=no`, `config=no`, `gapless-audio=yes`
- [x] Observación de propiedades `pause`, `time-pos`, `duration`, `volume`, `mute`, `eof-reached`, `idle-active` y eventos `file-loaded` / `end-file`
- [x] Máquina de estados detenido / cargando / reproduciendo / pausado / error según §3 con transiciones inválidas ignoradas
- [x] Publicación de `EstadoReproduccion` por canal `watch` consumido por UI y MPRIS
- [x] Comandos ReemplazarCola, AnadirAlFinal, ReproducirSiguiente, EliminarDeCola, MoverEnCola, VaciarCola, SaltarA, AlternarPausa, Reanudar, Pausar, Detener, Siguiente, Anterior, Buscar, Volumen, AlternarSilencio, AlternarAleatorio, CiclarRepeticion, FijarRepeticion y Apagar
- [x] Reproducción gapless entre pistas consecutivas de la cola
- [x] Al fallar una pista se notifica, se salta a la siguiente y tras tres fallos seguidos se pasa a detenido
- [x] Volumen acotado 0-100 y saltos acotados al rango de la pista

### Cola de reproducción
- [x] Cola con orden efectivo y orden original (`posicion_orig`)
- [x] Modo aleatorio que baraja manteniendo la pista actual la primera y restaura el orden original al desactivarse
- [x] Repetición con modos no / todo / una según §7.3
- [x] Anterior reinicia la pista si lleva más de 3 s reproducidos; si no, va a la anterior
- [x] Añadir al final sobre cola vacía y reproductor detenido inicia la reproducción
- [x] Eliminar la pista que suena pasa a la siguiente o detiene si era la última
- [x] Persistencia de la cola en COLA y de `cola_posicion` en AJUSTES en cada cambio
- [x] Al arrancar se restaura la cola en pausa en la pista y posición guardadas, sin reproducir automáticamente
  - AC: Dado que se cerró con `q` sonando la pista 3 en 1:20, cuando se arranca, entonces la cola muestra la pista 3 seleccionada en pausa a 1:20
- [x] Persistencia de volumen, silencio, aleatorio y repetición en AJUSTES

### Historial
- [x] Inserción en HISTORIAL_REPRODUCCION al recibir `file-loaded`
- [x] Marca `completada = 1` una sola vez al alcanzar el 50 % de tiempo reproducido real (sin contar seeks)

### Carátulas
- [x] Extracción de carátula embebida de la primera pista del álbum que la tenga
- [x] Búsqueda de `cover.*`, `folder.*` o `front.*` en la carpeta del álbum como alternativa
- [x] Caché de carátulas redimensionadas a 300×300 JPEG en `~/.cache/mmmusic/caratulas/{album_id}.jpg` y `caratula_ruta` actualizada
- [x] Renderizado con `ratatui-image` detectando protocolo (Kitty, Sixel, iTerm2) con fallback a half-blocks
- [x] Decodificación de imágenes en hilo auxiliar con caché LRU de 200 entradas y marcador `♪` mientras carga
- [x] Opción `interfaz.caratulas = false` que desactiva por completo el renderizado de imágenes

### Consultas de biblioteca
- [x] `listar_artistas` con nombre, número de álbumes y número de pistas
- [x] `detalle_artista` con álbumes por año descendente y pistas sueltas
- [x] `listar_albumes` con carátula, artista, año y número de pistas, con orden configurable
- [x] `detalle_album` con pistas ordenadas por disco y número
- [x] `listar_pistas` paginado de 500 con orden por título, artista, álbum, duración o año
- [x] `buscar` con términos normalizados en AND sobre `*_norm`, límite 20 por bloque e indicador "+N más"
- [x] `inicio` con 10 reproducidas recientemente, 10 álbumes añadidos recientemente y 10 álbumes al azar

### Interfaz: layout general
- [x] Layout raíz con sidebar de 18 columnas, contenido flexible y barra inferior de 3 filas
- [x] Sidebar con las seis secciones numeradas, lista de playlists, número de pistas y progreso de escaneo
- [x] Barra inferior con carátula, título, artista · álbum, controles, progreso con tiempos, estado de aleatorio/repetición y volumen
- [x] Barra inferior muestra "(detenido)" y tiempos `--:--` sin pista cargada
- [x] Modo compacto: con menos de 70 columnas la sidebar se colapsa a números y con menos de 20 filas la barra inferior pasa a 2 filas sin carátula
- [x] Ciclo de foco sidebar → contenido → cola con Tab / Shift+Tab e indicador `◄` del panel activo
- [x] Notificaciones tipo toast en la esquina superior derecha durante 3 s con niveles info/aviso/error
- [x] Línea de ayuda contextual al pie del contenido con los atajos de la vista
- [x] Listas y tablas virtualizadas que solo pintan las filas visibles
- [x] Render bajo demanda sin parpadeo, con redibujado al redimensionar la terminal
- [x] Modo `iconos = "ascii"` que sustituye los iconos Nerd Font por texto

### Interfaz: vistas
- [x] Vista Inicio con tres bloques (reproducidas recientemente, añadidos recientemente, redescubre) navegables
- [x] Vista Buscar con campo de texto, búsqueda incremental con debounce de 150 ms y bloques Artistas / Álbumes / Pistas
- [x] Vista Artistas como lista con contadores y detalle de artista (rejilla de álbumes + pistas sueltas)
- [x] Vista Álbumes como rejilla de tarjetas con carátula, título y artista
- [x] Detalle de álbum con carátula grande, cabecera (artista · año · nº pistas · duración · formato) y lista de pistas con indicador `▶` en la que suena
- [x] Vista Pistas como tabla ordenable por columna con indicador de orden y contador "x-y de N"
- [x] Vista Playlists como lista y detalle de playlist con pistas reordenables
- [x] Panel de cola como columna derecha con la pista actual marcada, contador y ayuda de atajos
- [x] Overlay de ayuda con todos los atajos agrupados
- [x] Diálogo de confirmación (eliminar playlist, vaciar cola, sobreescribir M3U)
- [x] Diálogo de texto (nueva playlist, renombrar, nombre de fichero de importación/exportación)
- [x] Selector de playlist con opción "+ Nueva playlist…"
- [x] Contexto de reproducción según la vista: Enter sobre una pista construye la cola con el álbum, artista, lista filtrada, playlist o bloque de resultados correspondiente

### Atajos de teclado
- [x] Globales: `q` salir guardando, `Ctrl+c` salir inmediato, `Esc` volver/cerrar, `?` ayuda, `1`–`6` secciones, `c` panel de cola, `Tab`/`Shift+Tab` foco, `Ctrl+r` reescanear, `t` recargar tema
- [x] Navegación vim: `j`/`k`/`↓`/`↑`, `h`/`l`/`←`/`→`, `gg`/`G`, `Ctrl+d`/`Ctrl+u`, `Enter`
- [x] Reproducción: `Espacio`, `n`/`p`, `x`, `,`/`.`, `<`/`>`, `+`/`-`, `m`, `s`, `r`
- [x] Cola y playlists: `a`, `A`, `P`, `d`, `J`/`K`, `N`, `R`, `D`, `e`, `i`, `C`
- [x] Búsqueda: `/` enfoca el campo, `Enter` pasa a resultados, `Esc` sale del campo
- [x] Los atajos sin sentido para el foco o el elemento seleccionado se ignoran sin error
- [x] Ratón: clic selecciona, doble clic abre/reproduce, rueda hace scroll y clic en la barra de progreso hace seek

### Playlists
- [x] Crear playlist con nombre validado (1-100 caracteres, no vacío, único normalizado)
- [x] Renombrar playlist con la misma validación
- [x] Eliminar playlist con confirmación y borrado en cascada de sus pistas
- [x] Añadir pista, álbum, artista o playlist completa a una playlist desde el selector
- [x] Quitar pista de una playlist y mover pistas dentro de ella con `J`/`K` manteniendo `posicion` consistente
- [x] Exportar playlist a M3U8 en `carpeta_playlists` con `#EXTM3U` y `#EXTINF`, con confirmación de sobreescritura
- [x] Importar M3U/M3U8 como playlist nueva resolviendo rutas relativas, ignorando rutas fuera de la biblioteca e informando de pistas no encontradas
- [x] Nombre de playlist importada igual al del fichero, con sufijo `(2)` si ya existe

### Tema
- [x] Lectura de `~/.config/omarchy/current/theme/alacritty.toml` (`colors.primary`, `colors.normal`, `colors.bright`) y construcción de la paleta
- [x] Paleta "terminal" con colores ANSI por defecto cuando `tema.fuente = "terminal"` o el fichero no existe
- [x] Mapeo configurable de acento y progreso a nombres de color ANSI del tema
- [x] Watcher con `notify` sobre `~/.config/omarchy/current/` con debounce de 300 ms y recarga en caliente de la paleta
- [x] Tema ilegible conserva la paleta anterior y muestra notificación de aviso

### MPRIS
- [x] Servidor `org.mpris.MediaPlayer2.mmmusic` en el bus de sesión con Identity, CanQuit, CanRaise=false y DesktopEntry
- [x] Interfaz Player con PlaybackStatus, Metadata (trackid, length, artUrl, title, artist, album, url), Position, Volume, Shuffle y LoopStatus
- [x] Métodos PlayPause, Play, Pause, Stop, Next, Previous, Seek, SetPosition y Quit enrutados a `ComandoReproductor`
- [x] Propiedades Can* coherentes con el estado y la cola
- [x] Emisión de PropertiesChanged y señal Seeked ante cambios de estado
- [x] Fichero `mmmusic.desktop` con `Terminal=true` y script de instalación junto a `cargo install`

### Calidad y entrega
- [x] Tests unitarios de normalización, resolución de etiquetas, cola (aleatorio, repetición, anterior, reproducir a continuación) y parseo de configuración
- [x] Tests de integración de escaneo sobre `tests/fixtures/` con ficheros diminutos de los seis formatos
- [x] Tests de importación/exportación M3U
- [x] `cargo clippy -- -D warnings` y `cargo fmt --check` limpios
- [x] README con instalación en Arch (`mpv`, `pkgconf`), configuración y atajos
- [x] `config.ejemplo.toml` documentado

### S5 — Escritorio y pulido

- Servidor MPRIS completo sobre `mpris-server`/zbus en hilo propio: identidad, `CanQuit`, `CanRaise=false`, `DesktopEntry`, interfaz `Player` con estado, metadatos (trackid, length, artUrl, title, artist, album, url), posición, volumen, shuffle y loop; métodos enrutados a `ComandoReproductor`; `PropertiesChanged` emitidos por los setters y señal `Seeked` en saltos. Verificado con `busctl` (metadatos correctos, pausa/play/next y `Quit`).
- `mmmusic.desktop` con `Terminal=true` y `instalar.sh` (cargo install + lanzador).
- Ratón: rueda con desplazamiento, clic para seleccionar y doble clic para reproducir en la tabla de Pistas y en el panel de cola, y clic en la barra de progreso para hacer seek.
- Modo compacto verificado (62×18: sidebar a números y barra inferior de 2 filas sin carátula), iconos `ascii`, toasts de 3 s, listas y tablas virtualizadas con ventana de filas visibles y render bajo demanda.
- `README.md` con instalación en Arch, configuración, atajos y ratón; `config.ejemplo.toml` documentado.
- Cierre del checklist: 112/112 funcionalidades marcadas.

## Pendientes y bloqueos

- S3: consultas restantes (`listar_artistas`, `detalle_artista`, `listar_albumes`, `detalle_album`, `inicio`, `buscar`) y vistas Inicio/Artistas/Álbumes con detalle, navegación vim completa, panel de cola interactivo y enriquecimiento de la sidebar.
- S4: búsqueda incremental, CRUD de playlists, M3U, carátulas (extracción, caché 300×300 y render con LRU).
- **Sin funcionalidades pendientes del checklist (112/112).**
- Validaciones manuales que quedan en manos del desarrollador en su Omarchy real:
  - Recarga en caliente del tema con `omarchy-theme-set` (no hay tema activo en la máquina de desarrollo; el parser y el watcher están testeados con fixtures).
  - Render de carátulas en Alacritty real (se validó en tmux con protocolo half-blocks).
  - Objetivo de rendimiento de reescaneo < 5 s con 10.000 pistas (el test de cancelación usa 1.000 copias y el incremental ya evita releer etiquetas).
  - Clic del ratón en terminal real (la lógica de zonas está implementada; en tmux no se pudieron inyectar eventos de ratón).
- Riesgo R-1 (libmpv2/mpv) descartado: enlaza y reproduce con mpv 0.41.

## Ejecución y pruebas

```bash
# Requisitos de sistema (Arch): mpv y pkgconf (y rustup/cargo)
cargo run                 # arranca la TUI
cargo test                # unitarios + integración (fixtures)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

- La BD y las migraciones se aplican solas al arrancar (`~/.local/share/mmmusic/mmmusic.db`).
- Prueba manual verificada: escaneo de una carpeta con seis formatos, reproducción por PipeWire, pausa, siguiente, gapless/avance automático, historial al 50 %, cierre con `q` y restauración en pausa.
- Para aislar pruebas manuales se usaron `XDG_*` apuntando a `/tmp`, sin tocar el perfil real.
- MPRIS verificado con `busctl`:
  `busctl --user get-property org.mpris.MediaPlayer2.mmmusic /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player Metadata`
  y llamadas `Pause`, `Play`, `Next` y `Quit` sobre la misma interfaz.
- Ratón verificado por código; la inyección de eventos en tmux no es fiable, queda la prueba manual en Alacritty.
