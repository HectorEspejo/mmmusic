# mmmusic

Reproductor de música para terminal (TUI) pensado para Omarchy (Arch Linux +
Hyprland), con disposición tipo Spotify: sidebar de navegación, contenido
central y barra inferior de "sonando ahora". Atajos estilo vim, carátulas en la
terminal y MPRIS para Waybar/`playerctl` y las teclas multimedia de Hyprland.

## Características

- Biblioteca local en SQLite con escaneo incremental (solo relee ficheros
  nuevos o modificados) de MP3, FLAC, OGG/Opus, M4A/AAC y WAV.
- Reproducción con `libmpv` (gapless, cola persistente con aleatorio y
  repetición, historial con umbral del 50 % o 4 minutos de tiempo real).
- Vistas Inicio, Buscar, Artistas, Álbumes, Pistas, Playlists y Radio (sección
  8, con pestañas Favoritas, Todas, Buscar y Sonando), con detalle de artista y
  de álbum, y panel de cola lateral mixta (pistas y emisoras).
- Búsqueda incremental sobre columnas normalizadas (sin diacríticos, CJK
  intacto).
- Playlists propias con importación/exportación M3U8.
- Scrobbling opcional a ListenBrainz y Last.fm con cola persistente y
  reintentos, favoritas sincronizadas con Last.fm (♥ Favoritas) e indicador de
  estado en la barra inferior.
- Recopilatorios sin `albumartist` agrupados bajo "Varios artistas" por carpeta
  y álbum.
- Visuales reactivas en terminal (espectro, barras y ondas, ambiente,
  partículas, caleidoscopio y túnel) que capturan el audio propio desde
  PipeWire, con mini espectro y paleta del tema o de la carátula.
- Tema del sistema de Omarchy con recarga en caliente al cambiar de tema.
- Carátulas embebidas o `cover.*` en caché 300×300, renderizadas con
  `ratatui-image` (Kitty/Sixel en terminales compatibles, half-blocks en
  Alacritty).
- Radio por internet: emisoras por URL (HTTP/ICY/HLS), importación y
  exportación PLS/M3U, favoritas, títulos ICY con historial por emisora,
  reconexión automática con esperas crecientes y scrobbling de las canciones
  anunciadas.
- Directorio abierto Radio Browser (búsqueda por nombre, país y etiqueta) con
  caché local de 24 h y descarga acotada de logos.
- MPRIS (`org.mpris.MediaPlayer2.mmmusic`) para `playerctl`, Waybar y teclas
  multimedia.

## Instalación en Arch

Dependencias de sistema:

```bash
sudo pacman -S mpv pkgconf pipewire clang
```

`pipewire` (ya presente en Omarchy) y `clang` (bindgen de las bindings de
PipeWire) son necesarias para las visuales.

Compilar y ejecutar:

```bash
cargo run --release
```

Instalar en el perfil (binario + lanzador de escritorio):

```bash
./instalar.sh
```

## Configuración

mmmusic crea `~/.config/mmmusic/config.toml` en el primer arranque con valores
razonables. Consulta `config.ejemplo.toml` para ver todas las opciones
(carpetas de la biblioteca, saltos de tiempo, iconos, carátulas, tema).

Rutas utilizadas:

- Base de datos: `~/.local/share/mmmusic/mmmusic.db`
- Credenciales de scrobbling: `~/.config/mmmusic/credenciales.toml` (permisos 600)
- Caché de carátulas: `~/.cache/mmmusic/caratulas/`
- Caché de logos de emisora: `~/.cache/mmmusic/logos/`
- Logs: `~/.local/state/mmmusic/mmmusic.log` (nivel configurable con `RUST_LOG`)

## Scrobbling

El envío de escuchas está desactivado por defecto. Actívalo en la sección
`[scrobbling]` de `config.toml` (`listenbrainz`, `lastfm`, `now_playing`) y
guarda las credenciales en `~/.config/mmmusic/credenciales.toml` (copia
`credenciales.ejemplo.toml`). mmmusic corrige los permisos a 600 si son más
abiertos.

- **ListenBrainz**: pega tu token de usuario en `[listenbrainz]`.
- **Last.fm**: crea una API key en <https://www.last.fm/api/account/create>,
  escribe `api_key` y `api_secret` en `[lastfm]` y ejecuta
  `mmmusic autorizar-lastfm`, que abre el navegador y guarda `session_key` y
  `usuario`.

Comprueba ambos servicios con `mmmusic probar-servicios` (código 0 si todo
está bien, 1 si falta configuración y 2 si hay error de red). Los envíos
pendientes se reintentan solos; `Ctrl+s` fuerza el envío inmediato y la barra
inferior muestra `↑`, `…N` o `!` según el estado.

`credenciales.toml` es el único fichero no regenerable: haz una copia de
seguridad aparte (no la subas a ningún repositorio).

## Radio

La sección `8 Radio` permite escuchar emisoras por URL y gestionarlas:

- **Favoritas / Todas**: `Enter` reproduce (reemplaza la cola), `a` añade al
  final y `A` a continuación; `L` marca o quita la favorita, `N` crea una
  emisora (nombre y URL), `R` la edita, `D` la elimina y `o`/`O` cambian el
  orden por nombre o por última reproducción.
- **Buscar**: consulta Radio Browser por nombre, país y etiqueta (`Tab` cambia
  de campo, `/` enfoca Nombre). `Enter` guarda y reproduce, `a` guarda en la
  cola y `L` guarda como favorita. Los resultados se cachean 24 h y se podan a
  7 días; con la red caída se muestran los resultados en caché.
- **Sonando**: datos de la emisora, estado de conexión, reconexiones, caché y
  los últimos 50 títulos con `✓` si existen en la biblioteca. `f` busca el
  título ICY (el seleccionado o el que suena) en la biblioteca.

La cola admite emisoras junto a las pistas (marcadas con `◉` y sin duración) y
la barra inferior pasa a modo directo con el estado, el título ICY y el tiempo
escuchando. MPRIS anuncia las emisoras sin `length` y con `CanSeek = false`.

### Importar y exportar

`i` importa un fichero PLS o M3U/M3U8 local (las duplicadas por URL se omiten
y las entradas sin URL válida se cuentan como inválidas). `e` exporta las
favoritas a `Radio favoritas.m3u8` en la carpeta de playlists, pidiendo
confirmación si ya existe.

### Radio Browser y red

El directorio se consulta en un hilo propio compartiendo el cliente HTTP de
mmmusic (`red/cliente.rs`, `ureq` + rustls). El espejo se resuelve por DNS de
`all.api.radio-browser.info` (consulta directa e inversa) y se guarda 24 h en
la base; ante fallos se prueban hasta 3 espejos por búsqueda y ante un 429 se
espera 5 minutos. Se envía un `User-Agent` identificativo y un click por
emisora y día (`/json/url/{uuid}`).

Los streams de audio los abre siempre `mpv`, nunca `ureq`. Los logos son la
única descarga fuera de la lista cerrada de hosts: solo https, solo
`image/*`, ≤ 512 KB, 5 s, 3 redirecciones, tratados como no confiables y
cacheados a 300×300 en `~/.cache/mmmusic/logos/{id}.jpg`. Con
`radio.directorio = false` no se lanza el hilo de directorio y la pestaña
Buscar se limita a las emisoras locales; `radio.logos = false` desactiva los
logos.

## Visuales

La sección `7 Visual` muestra a pantalla completa visualizaciones que reaccionan
al audio: Espectro, Barras y ondas, Ambiente, Partículas, Caleidoscopio y
Túnel. La barra inferior incorpora además un mini espectro de 12 barras.

El audio se captura del nodo de PipeWire propio de mmmusic
(`audio-client-name=mmmusic`) sin decodificar dos veces; si el enlace directo
con el nodo no está disponible se usa la captura por monitor del dispositivo y
la cabecera lo anuncia como "captura por monitor" (puede incluir audio de otras
aplicaciones). Sin PipeWire o sin nodo, las visuales pasan a modo ambiental.

Ajustes en `[visuales]` de `config.toml`: `activo`, `fps` (15-60),
`predeterminada`, `paleta` (`tema` o `caratula`), `mini_espectro`,
`autoinicio_min` (protector de pantalla) y `nodo`. Con `activo = false`
desaparecen el hilo de captura, el mini espectro y la sección 7.

La paleta `caratula` usa los cinco colores dominantes de la carátula del álbum
en reproducción, calculados en segundo plano y guardados en la base de datos.
Si una carátula aún no tiene colores se usa la del tema y la cabecera muestra
`♪`.

> Aviso de fotosensibilidad: las visuales con pulsos rápidos pueden resultar
> molestas. Para un uso suave, baja `fps` a 15 o usa la visual «Ambiente».

## Reescaneo completo

Tras actualizar mmmusic a una versión que cambie la agrupación de la
biblioteca, el primer arranque relanza solo un reescaneo completo para agrupar
los recopilatorios. También puedes forzarlo desde la terminal:

```bash
mmmusic reescanear --completo
mmmusic reescanear              # incremental, con progreso por stdout
```

## Atajos

Globales:

| Tecla | Acción |
|-------|--------|
| `q` / `Ctrl+c` | Salir guardando / salir inmediato |
| `Esc` | Volver o cerrar |
| `?` | Ayuda |
| `1`–`6` | Inicio, Buscar, Artistas, Álbumes, Pistas, Playlists |
| `7` | Modo visual (entrar o salir) |
| `8` | Radio |
| `c` | Panel de cola |
| `Tab` / `Shift+Tab` | Ciclar foco |
| `Ctrl+r` | Reescanear |
| `t` | Recargar tema |

En el modo visual: `v`/`V` ciclan las visuales, `1`–`6` saltan a una concreta,
`[`/`]` ajustan la sensibilidad (×0.8 / ×1.25), `b` alterna la paleta
tema ↔ carátula y `7`/`Esc` salen; los atajos de reproducción y `L` siguen
activos.

Radio: `[`/`]` cambian de pestaña, `f` busca el título ICY en la biblioteca,
`Enter` escucha (en Buscar guarda) y `Tab` cicla los campos de búsqueda. Con
una emisora sonando, `Espacio` pausa y reanuda (si la pausa supera un minuto se
recarga el stream), `n`/`p` cambian de elemento y `x` detiene; los seeks se
ignoran porque un stream no tiene línea de tiempo.

Navegación: `j/k/h/l`, flechas, `gg`/`G`, `Ctrl+d`/`Ctrl+u`, `Enter`.
En la tabla de Pistas, `o` cambia la columna de orden y `O` la invierte.

Reproducción: `Espacio`, `n`/`p`, `x`, `,`/`.` (5 s), `<`/`>` (30 s),
`+`/`-` (volumen), `m` (silencio), `s` (aleatorio), `r` (repetición).

Scrobbling y favoritas: `L` marca o quita la favorita de la pista seleccionada
(o de la que suena) y `Ctrl+s` envía los pendientes de scrobbling.

Cola y playlists: `a`/`A` (añadir / a continuación), `P` (añadir a playlist),
`d` (quitar), `J`/`K` (mover), `N`/`R`/`D` (nueva/renombrar/eliminar),
`e`/`i` (exportar/importar M3U8), `C` (vaciar cola). «♥ Favoritas» es una
pseudo-playlist no editable que se exporta como `Favoritas.m3u8`.

Ratón: rueda para desplazarse, clic para seleccionar y doble clic para
reproducir en la tabla de pistas y la cola; clic en la barra de progreso para
hacer seek.

## Desarrollo

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Los tests de integración usan fixtures diminutos de los seis formatos en
`tests/fixtures/`, generados con ffmpeg.
