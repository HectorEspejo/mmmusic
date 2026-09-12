# mmmusic - Checklist Fase 1: Reproductor Base

## Cimientos
- [x] Proyecto Cargo con estructura de carpetas del informe (`src/app.rs`, `biblioteca/`, `reproductor/`, `ui/`, `mpris.rs`, `tema.rs`, `tests/`)
- [x] Configuración en `~/.config/mmmusic/config.toml` con valores por defecto y creación automática si no existe
  - AC: Dado que no existe el fichero, cuando arranca mmmusic, entonces se crea con `carpetas = ["~/Music"]` y se muestra un aviso
- [x] Rutas XDG para BD (`~/.local/share/mmmusic/`), caché (`~/.cache/mmmusic/`) y logs (`~/.local/state/mmmusic/`)
- [x] Logs con `tracing` a fichero rotado diariamente, nivel configurable por `RUST_LOG`
- [x] Terminal en modo raw con pantalla alternativa y restauración garantizada al salir o ante pánico
- [x] Bucle principal de eventos con canal `AppEvento` y tick de 250 ms sin bloquear la UI

## Base de datos y migraciones
- [x] Apertura de SQLite en modo WAL con `busy_timeout` 5 s y claves foráneas activadas
- [x] Migración `001_inicial.sql` gestionada por `PRAGMA user_version` y aplicada de forma idempotente
- [x] Tablas ARTISTAS, ALBUMES, PISTAS, PLAYLISTS, PLAYLIST_PISTAS, COLA, HISTORIAL_REPRODUCCION, ESCANEOS y AJUSTES con los campos e índices del informe
- [x] Borrado en cascada de PLAYLIST_PISTAS, COLA e HISTORIAL_REPRODUCCION al eliminar una PISTA

## Escaneo de biblioteca
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

## Carátulas
- [x] Extracción de carátula embebida de la primera pista del álbum que la tenga
- [x] Búsqueda de `cover.*`, `folder.*` o `front.*` en la carpeta del álbum como alternativa
- [x] Caché de carátulas redimensionadas a 300×300 JPEG en `~/.cache/mmmusic/caratulas/{album_id}.jpg` y `caratula_ruta` actualizada
- [x] Renderizado con `ratatui-image` detectando protocolo (Kitty, Sixel, iTerm2) con fallback a half-blocks
  - AC: Dado Alacritty, cuando se muestra un álbum, entonces la carátula aparece en half-blocks sin errores en pantalla
- [x] Decodificación de imágenes en hilo auxiliar con caché LRU de 200 entradas y marcador `♪` mientras carga
- [x] Opción `interfaz.caratulas = false` que desactiva por completo el renderizado de imágenes

## Reproductor
- [x] Hilo reproductor con `libmpv2` inicializado con `vo=null`, `audio-display=no`, `ytdl=no`, `load-scripts=no`, `config=no`, `gapless-audio=yes`
- [x] Observación de propiedades `pause`, `time-pos`, `duration`, `volume`, `mute`, `eof-reached`, `idle-active` y eventos `file-loaded` / `end-file`
- [x] Máquina de estados detenido / cargando / reproduciendo / pausado / error según §3 con transiciones inválidas ignoradas
- [x] Publicación de `EstadoReproduccion` por canal `watch` consumido por UI y MPRIS
- [x] Comandos ReemplazarCola, AnadirAlFinal, ReproducirSiguiente, EliminarDeCola, MoverEnCola, VaciarCola, SaltarA, AlternarPausa, Reanudar, Pausar, Detener, Siguiente, Anterior, Buscar, Volumen, AlternarSilencio, AlternarAleatorio, CiclarRepeticion, FijarRepeticion y Apagar
- [x] Reproducción gapless entre pistas consecutivas de la cola
- [x] Al fallar una pista se notifica, se salta a la siguiente y tras tres fallos seguidos se pasa a detenido
- [x] Volumen acotado 0-100 y saltos acotados al rango de la pista

## Cola de reproducción
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

## Historial
- [x] Inserción en HISTORIAL_REPRODUCCION al recibir `file-loaded`
- [x] Marca `completada = 1` una sola vez al alcanzar el 50 % de tiempo reproducido real (sin contar seeks)

## Consultas de biblioteca
- [x] `listar_artistas` con nombre, número de álbumes y número de pistas
- [x] `detalle_artista` con álbumes por año descendente y pistas sueltas
- [x] `listar_albumes` con carátula, artista, año y número de pistas, con orden configurable
- [x] `detalle_album` con pistas ordenadas por disco y número
- [x] `listar_pistas` paginado de 500 con orden por título, artista, álbum, duración o año
- [x] `buscar` con términos normalizados en AND sobre `*_norm`, límite 20 por bloque e indicador "+N más"
- [x] `inicio` con 10 reproducidas recientemente, 10 álbumes añadidos recientemente y 10 álbumes al azar

## Interfaz: layout general
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

## Interfaz: vistas
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

## Atajos de teclado
- [x] Globales: `q` salir guardando, `Ctrl+c` salir inmediato, `Esc` volver/cerrar, `?` ayuda, `1`–`6` secciones, `c` panel de cola, `Tab`/`Shift+Tab` foco, `Ctrl+r` reescanear, `t` recargar tema
- [x] Navegación vim: `j`/`k`/`↓`/`↑`, `h`/`l`/`←`/`→`, `gg`/`G`, `Ctrl+d`/`Ctrl+u`, `Enter`
- [x] Reproducción: `Espacio`, `n`/`p`, `x`, `,`/`.`, `<`/`>`, `+`/`-`, `m`, `s`, `r`
- [x] Cola y playlists: `a`, `A`, `P`, `d`, `J`/`K`, `N`, `R`, `D`, `e`, `i`, `C`
- [x] Búsqueda: `/` enfoca el campo, `Enter` pasa a resultados, `Esc` sale del campo
- [x] Los atajos sin sentido para el foco o el elemento seleccionado se ignoran sin error
- [x] Ratón: clic selecciona, doble clic abre/reproduce, rueda hace scroll y clic en la barra de progreso hace seek

## Playlists
- [x] Crear playlist con nombre validado (1-100 caracteres, no vacío, único normalizado)
- [x] Renombrar playlist con la misma validación
- [x] Eliminar playlist con confirmación y borrado en cascada de sus pistas
- [x] Añadir pista, álbum, artista o playlist completa a una playlist desde el selector
- [x] Quitar pista de una playlist y mover pistas dentro de ella con `J`/`K` manteniendo `posicion` consistente
- [x] Exportar playlist a M3U8 en `carpeta_playlists` con `#EXTM3U` y `#EXTINF`, con confirmación de sobreescritura
- [x] Importar M3U/M3U8 como playlist nueva resolviendo rutas relativas, ignorando rutas fuera de la biblioteca e informando de pistas no encontradas
  - AC: Dado un M3U con 26 líneas de las que 2 no existen en PISTAS, cuando se importa, entonces se crea la playlist con 24 pistas y la notificación indica "24 pistas añadidas, 2 no encontradas"
- [x] Nombre de playlist importada igual al del fichero, con sufijo `(2)` si ya existe

## Tema
- [x] Lectura de `~/.config/omarchy/current/theme/alacritty.toml` (`colors.primary`, `colors.normal`, `colors.bright`) y construcción de la paleta
- [x] Paleta "terminal" con colores ANSI por defecto cuando `tema.fuente = "terminal"` o el fichero no existe
- [x] Mapeo configurable de acento y progreso a nombres de color ANSI del tema
- [x] Watcher con `notify` sobre `~/.config/omarchy/current/` con debounce de 300 ms y recarga en caliente de la paleta
  - AC: Dado mmmusic abierto, cuando se ejecuta `omarchy-theme-set`, entonces la interfaz cambia de colores en menos de 1 s sin reiniciar
- [x] Tema ilegible conserva la paleta anterior y muestra notificación de aviso

## MPRIS
- [x] Servidor `org.mpris.MediaPlayer2.mmmusic` en el bus de sesión con Identity, CanQuit, CanRaise=false y DesktopEntry
- [x] Interfaz Player con PlaybackStatus, Metadata (trackid, length, artUrl, title, artist, album, url), Position, Volume, Shuffle y LoopStatus
- [x] Métodos PlayPause, Play, Pause, Stop, Next, Previous, Seek, SetPosition y Quit enrutados a `ComandoReproductor`
- [x] Propiedades Can* coherentes con el estado y la cola
- [x] Emisión de PropertiesChanged y señal Seeked ante cambios de estado
  - AC: Dado mmmusic reproduciendo, cuando se ejecuta `playerctl -p mmmusic metadata`, entonces devuelve título, artista, álbum y artUrl correctos
- [x] Fichero `mmmusic.desktop` con `Terminal=true` y script de instalación junto a `cargo install`

## Calidad y entrega
- [x] Tests unitarios de normalización, resolución de etiquetas, cola (aleatorio, repetición, anterior, reproducir a continuación) y parseo de configuración
- [x] Tests de integración de escaneo sobre `tests/fixtures/` con ficheros diminutos de los seis formatos
- [x] Tests de importación/exportación M3U
- [x] `cargo clippy -- -D warnings` y `cargo fmt --check` limpios
- [x] README con instalación en Arch (`mpv`, `pkgconf`), configuración y atajos
- [x] `config.ejemplo.toml` documentado

---

**Progreso Fase 1:** 112 / 112 funcionalidades

**Total mmmusic (Fase 1):** 112 / 112 funcionalidades
