# mmmusic

Reproductor de música para terminal (TUI) pensado para Omarchy (Arch Linux +
Hyprland), con disposición tipo Spotify: sidebar de navegación, contenido
central y barra inferior de "sonando ahora". Atajos estilo vim, carátulas en la
terminal y MPRIS para Waybar/`playerctl` y las teclas multimedia de Hyprland.

## Características

- Biblioteca local en SQLite con escaneo incremental (solo relee ficheros
  nuevos o modificados) de MP3, FLAC, OGG/Opus, M4A/AAC y WAV.
- Reproducción con `libmpv` (gapless, cola persistente con aleatorio y
  repetición, historial con marca al 50 %).
- Vistas Inicio, Buscar, Artistas, Álbumes, Pistas y Playlists, con detalle de
  artista y de álbum, y panel de cola lateral.
- Búsqueda incremental sobre columnas normalizadas (sin diacríticos, CJK
  intacto).
- Playlists propias con importación/exportación M3U8.
- Tema del sistema de Omarchy con recarga en caliente al cambiar de tema.
- Carátulas embebidas o `cover.*` en caché 300×300, renderizadas con
  `ratatui-image` (Kitty/Sixel en terminales compatibles, half-blocks en
  Alacritty).
- MPRIS (`org.mpris.MediaPlayer2.mmmusic`) para `playerctl`, Waybar y teclas
  multimedia.

## Instalación en Arch

Dependencias de sistema:

```bash
sudo pacman -S mpv pkgconf
```

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
- Caché de carátulas: `~/.cache/mmmusic/caratulas/`
- Logs: `~/.local/state/mmmusic/mmmusic.log` (nivel configurable con `RUST_LOG`)

## Atajos

Globales:

| Tecla | Acción |
|-------|--------|
| `q` / `Ctrl+c` | Salir guardando / salir inmediato |
| `Esc` | Volver o cerrar |
| `?` | Ayuda |
| `1`–`6` | Inicio, Buscar, Artistas, Álbumes, Pistas, Playlists |
| `c` | Panel de cola |
| `Tab` / `Shift+Tab` | Ciclar foco |
| `Ctrl+r` | Reescanear |
| `t` | Recargar tema |

Navegación: `j/k/h/l`, flechas, `gg`/`G`, `Ctrl+d`/`Ctrl+u`, `Enter`.
En la tabla de Pistas, `o` cambia la columna de orden y `O` la invierte.

Reproducción: `Espacio`, `n`/`p`, `x`, `,`/`.` (5 s), `<`/`>` (30 s),
`+`/`-` (volumen), `m` (silencio), `s` (aleatorio), `r` (repetición).

Cola y playlists: `a`/`A` (añadir / a continuación), `P` (añadir a playlist),
`d` (quitar), `J`/`K` (mover), `N`/`R`/`D` (nueva/renombrar/eliminar),
`e`/`i` (exportar/importar M3U8), `C` (vaciar cola).

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
