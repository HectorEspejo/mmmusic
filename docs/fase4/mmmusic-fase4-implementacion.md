# mmmusic - Fase 4: Ecualizador y Letras — Informe de implementación

**Fecha:** 13 de septiembre de 2026
**Estado:** completa (50 / 50 funcionalidades del checklist)
**Autor del informe:** agente de desarrollo

## Resumen de lo implementado

La Fase 4 añade a mmmusic la parte de escucha y letras:

- **Migración `005_ecualizador_letras.sql`** (aditiva): tablas `PRESETS_EQ` y
  `LETRAS`, con `user_version` a 5. Verificada sobre una base real en XDG
  aislados (`user_version = 5`, tablas y columnas nuevas presentes).
- **Motor del ecualizador** en el hilo reproductor: cadena `af` etiquetada
  (`@pre` volume, `@eq1`…`@eq10` equalizer Q 1.41, `@lim` alimiter a −1 dBFS),
  instalada una vez y ajustada con `af-command` (bandas y preamp sin cortes),
  limitador con reconstrucción, bypass con `af = ""` cuando la curva es plana,
  detección de lavfi, errores de mpv con `disponible = false` y toast
  "Ecualizador no disponible", ganancias −12..12 en pasos de 0,5, preset
  personalizado al mover una banda, persistencia en AJUSTES con debounce de
  500 ms y restauración según `activo_al_arrancar`.
- **ReplayGain nativo** (`replaygain` no/pista/álbum, preamp, sin clip,
  fallback 0), aplicado con o sin EQ y persistido.
- **Nueve presets integrados** (valores §7.2) sembrados por código al arrancar
  y CRUD de presets propios.
- **Overlay `E`**: barras verticales con preamp como banda 0, valores, `▲`, cabecera
  con preset/EQ/limitador/ReplayGain, presets propios con `♦`, modos compacto
  (< 60 columnas) y ascii, atajos completos y captura de teclas sin ocultar la
  barra inferior. Indicadores `EQ`/`RG` (reducidos a `E`/`G` en compacto).
- **Letras locales**: parser LRC sin E/S, detección de sincronía, fuentes
  `.lrc`/`.txt` junto a la pista y en `letras.carpeta`, etiquetas USLT/LYRICS
  vía `lofty`, decodificación UTF-8 con respaldo Latin-1, worker de E/S propio y
  caché por `pista_id` invalidada al terminar un escaneo.
- **Vista `9 Letras`**: sincronizada con seguimiento centrado, manual de 5 s,
  salto con `Enter`, offset por pista `(`/`)` persistido en LETRAS, cambio de
  fuente con `s`, letra estática con `gg`/`G`, mensaje de rutas buscadas para
  pistas sin letra y "no aplicable" para emisoras.
- **Superposición en el modo visual** con `9` (persistida), banda inferior de
  `tamano_superposicion` líneas con fondo opaco, `(`/`)` y cabecera "· letras".
- **R-20 cerrada**: `radio/espejos.rs` reescrito sobre `dns-lookup`
  (`lookup_host` + `lookup_addr`), sin codificador/decodificador DNS propio.
- **Configuración**: secciones `[ecualizador]` y `[letras]` validadas y en
  `config.ejemplo.toml`; ayuda y README ampliados.

Estado de calidad: `cargo test` (128 pruebas de librería + integración, todas
verdes, incluida una prueba con libmpv real), `cargo clippy --all-targets --
-D warnings` y `cargo fmt --check` limpios.

## Desviaciones respecto a la especificación (qué y por qué)

1. **`af-command` necesita un cuarto argumento `target`.** El informe definía
   `comando_banda` como `(etiqueta, "g", valor)` y `comando_preamp` como
   `("@pre", "volume", valor)`. En mpv 0.41 (comprobado empíricamente por IPC y
   desde libmpv) los comandos a filtros lavfi fallan con `error running command`
   si no se indica el nombre del filtro FFmpeg interior. Se ampliaron ambas
   funciones con un cuarto campo (`"equalizer"` y `"volume"`) y
   `ReproductorMpv::af_command` recibe el target y pasa la etiqueta sin `@`.
   Sin esta desviación el ajuste en caliente era imposible. Fallback a
   reconstrucción intacto por si el comando falla.
2. **Worker dedicado de letras** (acordado con el desarrollador): la resolución
   (ficheros y etiquetas) se hace en un hilo propio y la UI nunca bloquea; la
   caché en memoria por `pista_id` vive en `AppEstado` y se invalida al terminar
   un escaneo. El informe describía resolución en el hilo de UI con paso al hilo
   auxiliar si la etiqueta tardaba más de 50 ms.
3. **La resolución guarda ambas fuentes a la vez** (fichero y etiqueta) para que
   `s` alterne sin volver a leer; la elección respeta la fuente preferida y, si
   no hay, el orden fichero → etiqueta. El informe describía un resolutor
   puramente por prioridad, que no permitía saber si existía la otra fuente.
4. **Fixtures con etiqueta** (acordado): los `.lrc`/`.txt` están versionados y
   las pistas con USLT/LYRICS se generan en los tests copiando un fixture de
   audio y escribiendo la etiqueta con `lofty` sobre el temporal.
5. **`ComandoReproductor` pierde `Eq`** en su derive: `ComandoEq` contiene `f32`
   y no puede derivar `Eq`. Solo se usa `PartialEq`, también en tests.
6. **Detección de ReplayGain en la pista**: `EstadoEq.tiene_replaygain` se fija
   al cargar con `metadata/by-key/REPLAYGAIN_TRACK_GAIN` y
   `…ALBUM_GAIN` de mpv (nativo, sin releer etiquetas).
7. **Al borrar el preset activo**, el reproductor conserva el nombre hasta el
   siguiente cambio de banda (no existe un comando "personalizar" en la API del
   informe). Es cosmético: la curva sigue sonando.
8. **Selección manual en la vista Letras**: la línea seleccionada se resalta
   con `REVERSED` (el mockup solo especificaba el `▶` de la línea sonando) para
   que `Enter` tenga un destino visible; el `▶` sigue siempre en la línea actual.
9. **`CLAUDE.md`**: la raíz y `docs/fase4/CLAUDE.md` ya estaban actualizados a
   `docs/faseN/` en el árbol de trabajo al comenzar la sesión; no requirió
   cambios.
10. **Test extra con libmpv real** (`mpv_acepta_la_cadena_etiquetada_y_los_comandos_en_caliente`):
    valida la cadena y los `af-command` reales (el AC de cambio en caliente).
    No estaba en la lista de tests del checklist; no añade red ni dependencias.
11. **El redibujo de letras va a 100 ms**, pero `time-pos` se publica cada
    250 ms (intervalo del reproductor, sin cambios): el seguimiento actualiza la
    línea como máximo cada 250 ms aunque la vista se repinte más a menudo.

## Estructura de archivos creada/modificada

Creados:

- `src/biblioteca/migraciones/005_ecualizador_letras.sql`
- `src/ecualizador/mod.rs`, `cadena.rs`, `presets.rs`, `replaygain.rs`
- `src/letras/mod.rs`, `lrc.rs`, `sincronia.rs`, `fichero.rs`, `etiqueta.rs`
- `src/ui/componentes/ecualizador.rs`
- `src/ui/vistas/letras.rs`
- `tests/cadena_eq.rs`, `tests/lrc.rs`, `tests/letras.rs`

Modificados:

- `Cargo.toml` / `Cargo.lock` (`dns-lookup = "4"`, añadido con `cargo add`)
- `src/lib.rs` (módulos `ecualizador` y `letras`)
- `src/biblioteca/bd.rs` (migración 5, `VERSION_ACTUAL`, tests)
- `src/biblioteca/modelos.rs` (`PresetEq`)
- `src/biblioteca/consultas.rs` (`presets_eq`, `letras`, tests)
- `src/config.rs` (`[ecualizador]`, `[letras]`, validación, `carpeta_letras()`)
- `src/reproductor/mpv.rs` (`fijar_af`, `af_command`, `fijar_texto/número`,
  `lavfi_disponible`)
- `src/reproductor/estado.rs` (`eq: EstadoEq`)
- `src/reproductor/mod.rs` (`ComandoEq`, persistencia y restauración,
  ReplayGain, cadena)
- `src/eventos.rs` (`AppEvento::LetrasListas`)
- `src/app.rs` (overlay, presets, estado de letras, atajos, superposición,
  tick de 100 ms, caché)
- `src/ui/mod.rs`, `componentes/mod.rs`, `teclas.rs`, `barra_inferior.rs`,
  `sidebar.rs` (indirecto vía `Vista::TODAS`), `vistas/mod.rs`,
  `vistas/visual.rs`, `vistas/ayuda.rs`
- `src/main.rs` (siembra de AJUSTES y presets, worker de letras, config del EQ)
- `src/radio/espejos.rs` (R-20)
- `config.ejemplo.toml`, `README.md`
- `tests/cola.rs` (firma nueva de `reproductor::lanzar`)
- `tests/radio.rs` (`VERSION_ACTUAL`)

## Decisiones técnicas tomadas durante el desarrollo

- **Comandos en caliente con target**: bandas con `af-command eq1 g <dB>
  equalizer` y preamp con `af-command pre volume <dB> volume`. El limitador no
  admite comando y reconstruye la cadena (previsto en el informe).
- **Bypass** con `af = ""` cuando no hay EQ activo o la curva es plana sin
  limitador, para no gastar CPU.
- **Detección de lavfi** con una cadena de prueba al inicializar el reproductor;
  tras la prueba `af` queda vacío y se aplica el estado restaurado.
- **Persistencia del EQ** con debounce de 500 ms en el `tick` del reproductor y
  volcado final al apagar; `activo_al_arrancar` ("si"/"no") pisa el valor
  guardado y se persiste el estado efectivo.
- **Nombre de preset** validado en la UI y en la capa de datos; unicidad por
  `nombre_norm` con `etiquetas::normalizar`, compartida con los integrados.
- **Parser LRC manual** (sin dependencias nuevas): timestamps múltiples,
  metadatos, offset, marcas enhanced, BOM/CRLF y Latin-1 de respaldo.
- **Offset de letras** acotado a ±30 s en la capa de datos y en la UI; se
  conserva al cambiar de fuente (upsert que no pisa el otro campo).
- **Superposición** solo si la letra es sincronizada; la bandera se persiste.
- **R-20**: `resolver_espejos` con `lookup_host` + `lookup_addr`; helper puro
  `depurar_nombres` (filtro, deduplicación y fallback al nombre agregado) con
  tests que no tocan la red.
- **Índice de la pista en la vista Letras** calculado por
  `sincronia::linea_actual` (búsqueda binaria) con
  `t = time-pos + offset_lrc + offset_usuario`, también para la superposición.

## Funcionalidades del checklist completadas

1. Migración `005_ecualizador_letras.sql`: tablas PRESETS_EQ (nombre_norm único) y LETRAS (pista_id PK con cascada)
2. Claves `eq_activo`, `eq_preset_id`, `eq_ganancias`, `eq_preamp_db`, `eq_limitador`, `replaygain_modo`, `replaygain_preamp_db` y `letras_superpuestas` en AJUSTES sembradas por código desde `[ecualizador]` y `[letras]`
3. Presets integrados (Plano, Rock, Pop, Electrónica, Hip-hop, Vocal, Bass boost, Treble boost, Loudness) con los valores de §7.2, sembrados al arrancar si faltan, con `integrado = 1`
4. R-20: `radio/espejos.rs` reescrito sobre `dns-lookup` (`lookup_host` + `lookup_addr`) con la misma interfaz pública, eliminando el codificador/decodificador DNS propio y sus tests
5. `CLAUDE.md` actualizado a `docs/faseN/` como ubicación de la documentación
6. `cadena.rs` sin E/S: `construir` (etiquetas `@pre`, `@eq1`…`@eq10`, `@lim`; `equalizer` con `width_type=q`, `width=1.41`; `volume` en dB; `alimiter` a 0.891), `comando_banda`, `comando_preamp` y `es_bypass`
7. Detección de lavfi al arrancar; sin lavfi, `EstadoEq.disponible = false` y el overlay explica el motivo
8. Instalación de la cadena en mpv (`af`) al activar o al primer cambio; `af = ""` cuando `es_bypass` es verdadero
9. Cambios de banda y preamp aplicados con `af-command` sin reconstruir la cadena; fallback a reconstrucción completa si el comando falla
10. Activar o desactivar el limitador reconstruye la cadena (único cambio con corte)
11. Error de mpv al aplicar la cadena deja el EQ desactivado con toast "Ecualizador no disponible"
12. `ComandoEq` (Activar, Banda, Preamp, Limitador, Preset, Restablecer, ReplayGain, ReplayGainPreamp) atendido por el hilo reproductor y `EstadoEq` publicado dentro de `EstadoReproduccion`
13. Ganancias acotadas −12..12 dB en pasos de 0,5; preamp −12..12; mover una banda con preset aplicado pasa a personalizado
14. Persistencia de ganancias, preamp, preset, activo y limitador en AJUSTES con debounce de 500 ms y restauración al arrancar según `activo_al_arrancar`
15. ReplayGain: propiedades `replaygain` (no/track/album), `replaygain-preamp`, `replaygain-clip = no`, `replaygain-fallback = 0`, aplicadas con o sin EQ y persistidas
16. `E` abre y cierra el overlay sobre cualquier vista salvo el protector; captura las teclas mientras está abierto y no oculta la barra inferior
17. Barras verticales por banda (preamp como banda 0) con valores numéricos, marca `▲` de banda seleccionada y cabecera con preset, EQ, limitador y ReplayGain
18. `h`/`l` seleccionan banda, `j`/`k` ±1 dB, `J`/`K` ±0,5 dB, `0` pone la banda a 0 y `R` restablece todo a Plano
19. `Tab` alterna entre barras y lista de presets; `Enter` aplica el preset seleccionado; los presets propios llevan `♦`
20. `N` guarda la curva actual como preset propio con nombre validado (1-40 caracteres, único normalizado, distinto de los integrados)
21. `D` elimina un preset propio con confirmación; los integrados no se pueden eliminar ni editar
22. `e` activa/desactiva el EQ, `x` el limitador y `g` cicla ReplayGain no → pista → álbum
23. Indicadores "EQ" y "RG" en la barra inferior (reducidos a `E`/`G` en modo compacto); `RG` en tenue si la pista no tiene etiquetas ReplayGain
24. Modo compacto del overlay (< 60 columnas: solo valores y presets) y modo `ascii` (`#`, `^`, `*`)
25. `lrc.rs` sin E/S: timestamps `[mm:ss.xx]` múltiples por línea, metadatos (`ar`, `ti`, `al`, `offset`…), eliminación de marcas enhanced `<mm:ss.xx>`, líneas vacías con timestamp conservadas, orden por tiempo, BOM y CRLF normalizados
26. Detección de sincronización (`es_sincronizada`) en cualquier texto, incluidas letras embebidas con formato LRC
27. Trait `FuenteLetras` y resolutor por orden con caché en memoria por `pista_id`, invalidada al reescanear la pista
28. `FuenteFichero`: `.lrc` y `.txt` con el mismo nombre junto a la pista, y `letras.carpeta/<artista> - <titulo>.lrc|.txt` con nombres normalizados; `.lrc` > 256 KB ignorados y `.txt` > 64 KB truncados con aviso
29. `FuenteEtiqueta`: `UnsynchronizedText` (ID3 USLT) y `LYRICS` (Vorbis/MP4) vía `lofty`, leídas bajo demanda; si tarda más de 50 ms, en el hilo auxiliar
30. Decodificación UTF-8 con respaldo Latin-1 y limpieza de caracteres de control
31. `fuente_preferida` por pista en LETRAS cuando el usuario cambia de fuente con `s`
32. Fixtures: pista con `.lrc` sincronizado, pista con `.txt`, pista con USLT embebida y pista con LRC dentro de la etiqueta
33. Sección `9 Letras` accesible con `9`, con estados sincronizada (siguiendo / manual), estática, sin letra y no aplicable (emisoras)
34. Seguimiento: `sincronia::linea_actual` por búsqueda binaria con `t = time-pos + offset_lrc + offset_usuario`, línea actual centrada, en acento y con `▶`, resto en tenue
35. `j`/`k`/rueda pasan a estado manual durante 5 s (el `▶` sigue moviéndose sin desplazar); `Enter` vuelve a seguimiento
36. `Enter` sobre una línea sincronizada salta a su tiempo (compensando offsets) y vuelve a seguimiento
37. `(` / `)` ajustan el offset ±100 ms (`Shift` ±500 ms), acotado ±30 s, guardado en LETRAS.offset_ms con toast del valor
38. `s` alterna fichero/etiqueta cuando hay más de una fuente, con indicador "fichero ▸ etiqueta" en la cabecera
39. Letra estática con scroll libre, `gg`/`G`, sin resaltado ni offset
40. Sin letra: mensaje con las rutas exactas donde se buscó (`.lrc`/`.txt` junto a la pista y en `letras.carpeta`) y la etiqueta
41. Cambio de pista con la vista abierta resuelve la nueva letra sin salir; tick de 100 ms solo mientras la vista o la superposición son visibles
42. `9` en modo visual alterna la superposición (persistida en `letras_superpuestas`); solo con letra sincronizada
43. Banda inferior del canvas de `tamano_superposicion` líneas con fondo del tema opaco, línea actual con `▶` y vecinas en tenue
44. `(` / `)` funcionan también en modo visual con la superposición activa
45. La cabecera del modo visual indica "letras" cuando la superposición está activa
46. Secciones `[ecualizador]` (activo_al_arrancar, limitador, replaygain, replaygain_preamp_db) y `[letras]` (carpeta, superpuestas, tamano_superposicion) validadas y en `config.ejemplo.toml`
47. Overlay de ayuda con las secciones Ecualizador y Letras
48. README: ecualizador, presets, ReplayGain, dónde colocar `.lrc`, offsets, superposición y aviso de que `mmmusic.db` contiene presets y offsets
49. Tests: `cadena_eq.rs` (cadena, comandos, bypass, presets integrados), `lrc.rs` (parser, sincronía, enhanced, offset), `letras.rs` (resolución por fuentes con fixtures y `fuente_preferida`)
50. `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios

## Pendientes y bloqueos

- **Prueba manual en Omarchy pendiente del desarrollador**: arrancar con audio
  real y comprobar el ajuste en caliente del EQ (sin cortes), el click al
  alternar el limitador, la captura de PipeWire reflejando el audio ecualizado
  y el seguimiento de letras con una pista real. La parte automatizable está
  cubierta (incluida una prueba con libmpv real).
- **Sin bloqueos.** La prueba de migración 004 → 005 está cubierta por los tests
  de `bd.rs` (que aplican todas las migraciones en orden y comprueban la
  preservación de datos).

## Ejecución y pruebas

Arrancar:

```bash
cargo run
```

Prueba manual aislada (sin tocar el perfil real):

```bash
export XDG_CONFIG_HOME=$(mktemp -d)/config XDG_DATA_HOME=$(mktemp -d)/data \
       XDG_CACHE_HOME=$(mktemp -d)/cache XDG_STATE_HOME=$(mktemp -d)/state
mkdir -p "$XDG_CONFIG_HOME/mmmusic"
printf '[biblioteca]\ncarpetas = ["/ruta/a/tu/musica"]\n' > "$XDG_CONFIG_HOME/mmmusic/config.toml"
cargo run
```

Migración: se aplica sola al abrir la base (`005_ecualizador_letras.sql`,
`user_version` 4 → 5, aditiva). No hay copia `pre-005` porque no recrea tablas;
la copia `mmmusic.db.pre-004` sigue su ciclo habitual. Se puede forzar el
migrado con `cargo run -- reescanear` sobre una base existente.

Pruebas:

```bash
cargo test                        # incluida la de libmpv real
cargo test --test cadena_eq       # cadena, comandos, bypass y presets
cargo test --test lrc             # parser y sincronía
cargo test --test letras          # fuentes, offsets y fuente preferida
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Comprobación de la migración hecha en esta sesión: con XDG aislados,
`cargo run -- reescanear --completo` sobre dos fixtures dejó
`user_version = 5` y las tablas `PRESETS_EQ` (7 columnas) y `LETRAS`
(4 columnas) creadas.
