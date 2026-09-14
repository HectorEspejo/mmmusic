# mmmusic - Checklist Fase 4a: Identidad Visual

## Módulo de marca
- [x] `src/marca.rs` con `LOGO_COMPACTO`, `LOGO_MMM`, `LOGO_GRANDE`, sus variantes `_ASCII`, `ESLOGAN`, `ONDA`, `ONDA_ASCII`, `version()`, `linea_reposo()` y `onda()`
- [x] Test de anchura: todas las filas de cada logo miden lo mismo en columnas de terminal (33, 17 y 62) y cada variante ascii mide igual que su original
- [x] `linea_reposo` recorta por etapas (sin eslogan, luego solo "mmmusic") y centra; `onda` repite el patrón y lo desplaza módulo su longitud
  - AC: Dado un ancho de 30, cuando se pide la línea de reposo con versión 0.4.0, entonces devuelve "m m m u s i c · v0.4.0" centrada sin eslogan

## Sidebar y ayuda
- [x] Cabecera de la sidebar con `LOGO_MMM` en dos filas en color de acento cuando la sidebar no está colapsada; "♪" en una fila cuando lo está
- [x] El título "mmmusic" del borde de la sidebar de F1 se elimina
- [x] Overlay de ayuda con `LOGO_COMPACTO` centrado y la versión a su derecha; con menos de 37 columnas usa `LOGO_MMM`
- [x] Variantes ascii en sidebar y ayuda cuando `iconos = "ascii"`

## Barra de reproducción en reposo
- [x] Sin elemento cargado, la barra inferior muestra la tarjeta de reposo: fila 1 con `linea_reposo` y fila 2 con la onda, manteniendo controles e indicadores a la derecha
  - AC: Dada la cola vacía, cuando se arranca, entonces la barra muestra la marca con versión y eslogan y no el texto "(detenido)"
- [x] La onda se desplaza un carácter por tick de 250 ms; variante ascii `._-~^~-_.`
- [x] Con elemento cargado (aunque esté en pausa o detenido) la barra vuelve al formato normal de F1/F5
- [x] En modo compacto (barra de 2 filas) solo se muestra la fila 1 sin eslogan

## CLI y ficheros
- [x] `mmmusic --version` imprime `♪ mmmusic <versión>` y `mmmusic --version --logo` imprime `LOGO_GRANDE`, la versión y el eslogan
- [x] README con `LOGO_GRANDE` en la cabecera dentro de un bloque de código y nota de regeneración con figlet `ansi_shadow`
- [x] `mmmusic.desktop` con `Comment=Reproductor de música para tu terminal`
- [x] `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios

---

**Progreso Fase 4a:** 15 / 15 funcionalidades

**Total mmmusic (Fases 1-5, incl. 4a):** 305 / 355 funcionalidades
