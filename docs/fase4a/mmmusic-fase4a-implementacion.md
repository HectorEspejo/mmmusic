# mmmusic - Fase 4a: Identidad Visual — Informe de Implementación

**Fecha:** 13 de septiembre de 2026
**Estado:** completada

## Resumen de lo implementado

La fase 4a dota a mmmusic de una identidad visual propia en texto, sin tocar el
modelo de datos ni el audio:

- Nuevo módulo `src/marca.rs` con todo el arte de la marca: `LOGO_COMPACTO`,
  `LOGO_MMM` y `LOGO_GRANDE` con sus variantes `_ASCII`, el eslogan, el patrón
  de onda, `version()`, `linea_reposo()`, `linea_reposo_sin_eslogan()` y `onda()`.
- Cabecera de la sidebar con `LOGO_MMM` en acento (dos filas); el título
  `" mmmusic "` del borde desaparece. Con la sidebar colapsada (< 70 columnas
  de terminal) o un interior menor de 17 columnas se muestra `♪`.
- Overlay de ayuda con `LOGO_COMPACTO` centrado y la versión a su derecha;
  por debajo de 37 columnas de overlay usa `LOGO_MMM`.
- Tarjeta de reposo en la barra inferior cuando no hay elemento cargado: fila 1
  con marca, versión y eslogan, fila 2 con la onda desplazándose un carácter
  cada 250 ms; los controles e indicadores siguen intactos a la derecha. En
  modo compacto solo se dibuja la fila 1 sin eslogan.
- `mmmusic --version` imprime `♪ mmmusic <versión>` y `mmmusic --version --logo`
  añade `LOGO_GRANDE` encima. `--logo` exige `--version`.
- README con `LOGO_GRANDE` en un bloque de código y nota de regeneración con la
  fuente `ansi_shadow` de figlet; `mmmusic.desktop` con el nuevo `Comment`.
- Variantes ascii en sidebar, ayuda y onda cuando `iconos = "ascii"`.
- Correcciones tras la revisión visual del desarrollador: la sidebar fuerza un
  mínimo de 20 columnas cuando no está colapsada (así se ve `LOGO_MMM` aunque la
  configuración guardada sea 18) y la vista de tarjetas de Inicio desplaza
  verticalmente el bloque enfocado cuando los tres no caben en pantalla.
- Petición posterior del desarrollador, fuera del checklist: `S` abre
  `config.toml` en `$EDITOR` suspendiendo la TUI y al volver recarga la
  configuración en caliente (con aviso de reinicio para radio, visuales y
  scrobbling).
- Suite nueva `tests/marca.rs`: anchuras con `unicode-width`, etapas de
  `linea_reposo`, onda, y tres pruebas de UI con `TestBackend` que cubren los
  criterios de aceptación de la barra y la sidebar.

Sin crates nuevos, sin tablas y sin migraciones.

## Desviaciones respecto a la especificación

1. **Anchuras reales de los logos.** El arte literal del informe mide 31
   columnas (`LOGO_COMPACTO`, que documenta 33) y 61 (`LOGO_GRANDE`, que
   documenta 62); `LOGO_MMM` sí mide las 17 anunciadas. Se consultó al
   desarrollador y se acordó mantener el arte literal: el test comprueba que
   todas las filas de un logo miden lo mismo y que las variantes ascii miden
   como su original, sin exigir 33/62. Pendiente de corrección en los
   documentos funcionales (informe y checklist).
2. **`linea_reposo_sin_eslogan(ancho, ascii)`** es una función añadida fuera de
   la tabla de §5 (aprobada antes de implementarla). Era necesaria para el modo
   compacto, donde debe mostrarse la fila 1 sin eslogan aunque el ancho diera
   para el texto completo; evita duplicar la cadena base en la UI. Su parámetro
   `ascii` queda sin uso porque la etapa sin eslogan no incluye el símbolo de
   reproducción.
3. **`ancho_sidebar` por defecto 18 → 20.** `LOGO_MMM` mide 17 columnas y la
   sidebar de 18 dejaba 16 útiles tras los bordes, así que no cabía. Se subió
   el valor por defecto a 20 (coherente con el mockup de la fase 1) y la
   validación de rango cae ahora a 20. Para que el logo aparezca también en
   configuraciones antiguas (como la del desarrollador, con 18), la UI aplica
   un mínimo de 20 columnas cuando la sidebar no está colapsada; si el terminal
   baja de 70 columnas, la sidebar se colapsa a `♪` como antes.
4. **`AppEstado::desplazamiento_onda()`.** Pequeño método de apoyo en `app.rs`
   que deriva el desplazamiento del tiempo transcurrido (un salto por cada tick
   base de 250 ms) en vez de contar eventos `Tick`. Así la onda no acelera
   cuando el tick está en modo visual. No forma parte de `marca.rs` pero era
   necesario para la animación.
5. **Versión en la ayuda en overlays mínimos.** Si el bloque
   logo + versión no cabe en el interior del overlay, se omite la versión en
   esa fila en lugar de recortarla; con el ancho mínimo del overlay (20) el
   logo de tres emes siempre cabe.
6. **Pruebas de UI no previstas en el plan de tests.** `tests/marca.rs` incluye
   tres pruebas con `TestBackend` (barra en reposo, barra con elemento,
   sidebar) para cubrir los criterios de aceptación del checklist, además de
   las pruebas de anchura y funciones prometidas.
7. **Desplazamiento del bloque enfocado en Inicio.** Corrección solicitada por
   el desarrollador tras la revisión visual y fuera del alcance de la fase 4a:
   la vista de tarjetas dibujaba los bloques de arriba abajo sin scroll, así
   que en terminales bajas "Redescubre" podía quedar solo con el título o
   directamente fuera. Ahora `primer_bloque_visible()` sube el primer bloque
   dibujado cuando el bloque enfocado no cabe entero; con el foco en el primer
   bloque el comportamiento no cambia.
8. **Editar `config.toml` desde la TUI (`S`).** Funcionalidad solicitada por el
   desarrollador y ajena al checklist de la 4a. `S` suspende la TUI, abre el
   fichero en `$VISUAL`/`$EDITOR` (o `vi` si no hay ninguno definido) y al
   volver relee la configuración: interfaz, iconos y tema se aplican al
   momento; si cambian radio, visuales o scrobbling se avisa de que hace falta
   reiniciar. Un TOML inválido deja la configuración anterior intacta y muestra
   un toast de error.

## Estructura de archivos creada/modificada

Creados:

- `src/marca.rs` — constantes y funciones de la marca.
- `tests/marca.rs` — 9 pruebas: 6 de marca y 3 de UI con `TestBackend`.
- `docs/fase4a/mmmusic-fase4a-implementacion.md` — este informe.

Modificados:

- `src/lib.rs` — expone `pub mod marca;`.
- `src/ui/mod.rs` — mínimo de 20 columnas para la sidebar no colapsada y
  `reanudar()` tras la suspensión de la TUI.
- `src/ui/sidebar.rs` — cabecera con `LOGO_MMM` (o `♪`) y sin título de borde.
- `src/ui/teclas.rs` — atajo `S` (`Accion::EditarConfig`).
- `src/ui/vistas/inicio.rs` — desplazamiento vertical del bloque enfocado.
- `src/ui/vistas/ayuda.rs` — logo compacto centrado con versión; `LOGO_MMM`
  bajo 37 columnas; variante ascii y fila del atajo `S`.
- `src/ui/barra_inferior.rs` — tarjeta de reposo cuando no hay elemento.
- `src/app.rs` — `desplazamiento_onda()` y `recargar_config()`.
- `src/cli.rs` — `--version` con marca y `--logo` (clap con
  `disable_version_flag`), `imprimir_version()`.
- `src/main.rs` — atiende `--version` antes de cualquier subcomando; pausa
  cooperativa del hilo de entrada, suspensión de la TUI y lanzamiento de
  `$EDITOR` sobre `config.toml` con recarga al volver.
- `src/config.rs` — `ancho_sidebar` por defecto 20 (y su validación).
- `config.ejemplo.toml` — `ancho_sidebar = 20`.
- `README.md` — `LOGO_GRANDE` en la cabecera, nota de figlet y atajo `S`.
- `mmmusic.desktop` — `Comment=Reproductor de música para tu terminal`.
- `docs/fase4a/mmmusic-fase4a-checklist.md` — marcado (15/15).

## Decisiones técnicas tomadas durante el desarrollo

- **Un único módulo de arte.** Ninguna cadena de logo, eslogan u onda vive
  fuera de `marca.rs`; la UI solo compone. `NOMBRE` ("m m m u s i c") es una
  constante privada del módulo para no repetirla entre las dos variantes de
  `linea_reposo`.
- **Anchura en columnas con `unicode-width`.** Las comprobaciones de "cabe" y
  los tests usan el ancho de terminal, no el número de bytes; el centrado se
  apoya en `format!("{:^}")` porque todos los glifos usados miden una columna.
- **Onda exacta.** `onda()` construye el patrón rotado con
  `chars().cycle().skip(desplazamiento % longitud).take(ancho)`, de forma que
  la fila mide siempre las columnas pedidas (la variante ascii mide 9 y la
  Unicode 12, ambas documentadas así).
- **`linea_reposo` por etapas.** Texto completo → sin eslogan → `"mmmusic"`;
  el último escalón siempre cabe en el mínimo de la barra (10 columnas) por lo
  que no se añade recorte con `"…"` (la tabla de §5 lo mencionaba, pero choca
  con el pseudocódigo de §7 y con el AC del checklist).
- **Acento y tenue.** Logo en `paleta.acento`; la onda de reposo en
  `paleta.secundario` (tenue); la versión de la ayuda también en secundario.
  La fila 1 de la tarjeta de reposo va en color de texto normal.
- **`--version --logo` sin "v".** La salida del CLI sigue el mockup 4
  (`♪ mmmusic 0.1.0 · reproductor para tu tty`); la tarjeta de reposo usa
  `v0.1.0` como en el pseudocódigo de §7.
- **Mínimo de sidebar en la UI.** El ancho configurado (14-40) se respeta por
  encima de 20; por debajo, la UI dibuja 20 columnas mientras la sidebar no
  esté colapsada, para que la cabecera nunca quede a medias.
- **Scroll por bloque enfocado en Inicio.** El desplazamiento se calcula por
  frame a partir de `bloque_inicio` y de las alturas naturales de los bloques,
  sin estado nuevo: no hay offset persistente que se quede obsoleto al
  redimensionar.
- **Pausa cooperativa del hilo de entrada.** El editor y crossterm comparten el
  mismo tty, así que antes de suspender la TUI el hilo de lectura deja de
  `poll()`ear y confirma la pausa con un segundo atómico; el bucle espera la
  confirmación (máx. 1 s) y solo entonces cede el terminal. Al volver se
  reanuda y se limpia la pantalla para forzar un frame completo.
- **Editor sin shell.** Se usa `$VISUAL`, luego `$EDITOR` y, si no hay ninguno,
  `vi`; la cadena se parte por espacios para admitir banderas (`nvim -f`) sin
  invocar un intérprete de por medio.
- **Recarga parcial y segura.** `recargar_config()` reemplaza `app.config`
  entero, recalcula iconos/visuales y relee el tema; un TOML inválido devuelve
  error y no toca la configuración en uso. Radio, visuales y scrobbling se
  comparan con la versión anterior para avisar de que sus hilos necesitan
  reinicio.
- **Sin migración ni cambios de esquema.** No se toca `PRAGMA user_version`
  ni ninguna tabla.

## Funcionalidades del checklist completadas

### Módulo de marca

- [x] `src/marca.rs` con `LOGO_COMPACTO`, `LOGO_MMM`, `LOGO_GRANDE`, sus variantes `_ASCII`, `ESLOGAN`, `ONDA`, `ONDA_ASCII`, `version()`, `linea_reposo()` y `onda()`
- [x] Test de anchura: todas las filas de cada logo miden lo mismo en columnas de terminal (33, 17 y 62) y cada variante ascii mide igual que su original
- [x] `linea_reposo` recorta por etapas (sin eslogan, luego solo "mmmusic") y centra; `onda` repite el patrón y lo desplaza módulo su longitud
  - AC: Dado un ancho de 30, cuando se pide la línea de reposo con versión 0.4.0, entonces devuelve "m m m u s i c · v0.4.0" centrada sin eslogan

Nota: el test cubre uniformidad y equivalencia ascii; las anchuras reales son
31, 17 y 61 (ver desviación 1). El AC se prueba con la versión real del
paquete (`marca::version()`), que es la que la función usa.

### Sidebar y ayuda

- [x] Cabecera de la sidebar con `LOGO_MMM` en dos filas en color de acento cuando la sidebar no está colapsada; "♪" en una fila cuando lo está
- [x] El título "mmmusic" del borde de la sidebar de F1 se elimina
- [x] Overlay de ayuda con `LOGO_COMPACTO` centrado y la versión a su derecha; con menos de 37 columnas usa `LOGO_MMM`
- [x] Variantes ascii en sidebar y ayuda cuando `iconos = "ascii"`

### Barra de reproducción en reposo

- [x] Sin elemento cargado, la barra inferior muestra la tarjeta de reposo: fila 1 con `linea_reposo` y fila 2 con la onda, manteniendo controles e indicadores a la derecha
  - AC: Dada la cola vacía, cuando se arranca, entonces la barra muestra la marca con versión y eslogan y no el texto "(detenido)"
- [x] La onda se desplaza un carácter por tick de 250 ms; variante ascii `._-~^~-_.`
- [x] Con elemento cargado (aunque esté en pausa o detenido) la barra vuelve al formato normal de F1/F5
- [x] En modo compacto (barra de 2 filas) solo se muestra la fila 1 sin eslogan

### CLI y ficheros

- [x] `mmmusic --version` imprime `♪ mmmusic <versión>` y `mmmusic --version --logo` imprime `LOGO_GRANDE`, la versión y el eslogan
- [x] README con `LOGO_GRANDE` en la cabecera dentro de un bloque de código y nota de regeneración con figlet `ansi_shadow`
- [x] `mmmusic.desktop` con `Comment=Reproductor de música para tu terminal`
- [x] `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios

## Pendientes y bloqueos

- Sin bloqueos. No hay funcionalidad pendiente de la fase.
- Los documentos funcionales siguen indicando 33/62 columnas para
  `LOGO_COMPACTO` y `LOGO_GRANDE`; conviene corregirlos a 31/61 en la próxima
  revisión del analista.
- Los mockups de la barra usaban la versión 0.4.0; la implementación muestra la
  versión real del paquete (0.1.0).

## Ejecución y pruebas

Arrancar la TUI:

```bash
cargo run
```

Prueba manual aislada, sin tocar el perfil real:

```bash
tmp=$(mktemp -d)
XDG_CONFIG_HOME=$tmp/config XDG_DATA_HOME=$tmp/data \
XDG_CACHE_HOME=$tmp/cache XDG_STATE_HOME=$tmp/state cargo run
```

No hay migración nueva: la base de datos se abre igual que en las fases
anteriores.

CLI:

```bash
cargo run -- --version          # ♪ mmmusic 0.1.0
cargo run -- --version --logo   # LOGO_GRANDE + ♪ mmmusic 0.1.0 · eslogan
```

Editar la configuración desde la TUI: `S` (usa `$VISUAL`/`$EDITOR`; al volver
muestra «Configuración recargada» o el aviso de reinicio). Para una prueba
aislada basta apuntar `$EDITOR` a un script que reescriba el fichero.

Suite completa y comprobaciones de estilo:

```bash
cargo test
cargo test --test marca
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

`tests/marca.rs` cubre: anchura uniforme de los tres logos y sus variantes
ascii con `unicode-width`, etapas de `linea_reposo`, `linea_reposo_sin_eslogan`,
onda (repetición y desplazamiento módulo), y tres pruebas de UI con
`TestBackend`: barra en reposo con marca/eslogan y sin "(detenido)", vuelta al
formato normal con elemento cargado, y sidebar con las tres emes sin el título
del borde incluso con una config antigua de 18 columnas. La suite unitaria
(`cargo test --lib`) añade
`ui::vistas::inicio::pruebas::el_bloque_enfocado_se_desplaza_a_la_vista`, que
comprueba que "Redescubre" sube a la parte superior al enfocarlo en una terminal
baja, y `app::pruebas::la_configuracion_se_recarga_en_caliente`, que verifica la
recarga de interfaz e iconos y que un TOML inválido no cambia nada. El flujo del
editor se comprobó de forma manual en una sesión tmux aislada: `S` lanzó el
editor, un script reescribió `config.toml` y la UI aplicó el nuevo ancho de
sidebar con el toast correspondiente. La comprobación manual en TUI (cola vacía,
ayuda con `?`, `iconos = "ascii"`, redimensionado) queda a cargo del
desarrollador en Omarchy.
