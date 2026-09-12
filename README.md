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
- Vistas Inicio, Buscar, Artistas, Álbumes, Pistas y Playlists, con detalle de
  artista y de álbum, y panel de cola lateral.
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
| `c` | Panel de cola |
| `Tab` / `Shift+Tab` | Ciclar foco |
| `Ctrl+r` | Reescanear |
| `t` | Recargar tema |

En el modo visual: `v`/`V` ciclan las visuales, `1`–`6` saltan a una concreta,
`[`/`]` ajustan la sensibilidad (×0.8 / ×1.25), `b` alterna la paleta
tema ↔ carátula y `7`/`Esc` salen; los atajos de reproducción y `L` siguen
activos.

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
