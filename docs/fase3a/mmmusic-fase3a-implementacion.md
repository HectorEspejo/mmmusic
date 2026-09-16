# mmmusic — Fase 3a: Visual «Ciudad» · Informe de implementación

**Fecha:** 16 de septiembre de 2026
**Rama:** `fase3a-visualizado_barras`
**Estado:** implementada y verificada con tests; pendiente de validación manual en Omarchy

---

## Resumen de lo implementado

La Fase 3a añade la **séptima visual, «Ciudad»**: el espectro se convierte en una
rejilla de bandas × tiempo de prismas dibujados en perspectiva, vista desde una
cámara que orbita como un helicóptero, con eliminación de líneas ocultas,
aristas punteadas y un cielo de estrellas parpadeantes.

El `Canvas` de `ratatui` es aditivo (solo añade puntos, no permite borrar), así
que la ocultación necesita el **raster propio** que ya anunciaba la regla T-38:
un raster de 2 × 4 puntos por celda con capa de color, borrado de polígonos por
scanline, líneas de Bresenham punteadas y volcado final al `Canvas` como
`Points` agrupados por color. El trait `Visual` no se ha tocado.

Tres módulos nuevos, sin E/S y probados con valores sintéticos: `proyeccion.rs`
(cámara, perspectiva, descarte de caras), `raster.rs` (raster y volcado) y
`ciudad.rs` (historia, prismas, orden del pintor, estrellas). La visual se
registra como séptima y se selecciona con `0` en el modo visual (la `7` sigue
saliendo), con la configuración `ciudad_vuelta_s`, `ciudad_filas` y
`ciudad_punteado`.

**Estado de las comprobaciones** (`cargo test`, `cargo clippy --all-targets -- -D
warnings`, `cargo fmt --check`): los tres limpios. 235 pruebas en verde (145 de
librería y 90 de integración), de las cuales 9 son de `tests/ciudad.rs` y 5
internas de `ciudad.rs`; la medición del presupuesto de frame queda fuera de la
ejecución normal (está marcada `#[ignore]`).

**Presupuesto de frame (riesgo R-27 medido):** en `--release`, el frame completo
(dibujo + volcado + `Canvas`) tarda **1,09 ms de mediana y 1,14 ms en el
percentil 95** a 200 × 50 (288 prismas) y **1,70 ms / 2,15 ms** en el peor caso
posible, 200 × 70 con `ciudad_filas = 16` (384 prismas), ambos dentro del
presupuesto de 10 ms de la Fase 3 y muy por debajo de los 33 ms del tick de
30 fps. En depuración el peor caso sube a 10,42 ms de mediana y 12,98 ms de p95,
todavía por debajo del umbral de degradación a 15 fps.

---

## Desviaciones respecto a la especificación (qué y por qué)

1. **`Raster::nuevo` recibe el modo ascii.** El informe §5 firma
   `Raster::nuevo(ancho_celdas, alto_celdas)`, pero §6 exige que el raster
   vuelque a caracteres en modo ascii. Se añade el tercer parámetro
   `ascii: bool`, coherente con las demás visuales (`Espectro::nuevo(ascii)`).
2. **`registro(ascii, &AjustesCiudad)`.** El trait `Visual::dibujar` no recibe
   configuración y no se puede modificar (T-38), así que los tres ajustes de
   Ciudad entran por construcción: `AjustesCiudad` (struct propio de
   `ciudad.rs`, con `Default`) se construye en `app.rs` desde `config.visuales`.
   La firma cambia en sus dos únicos llamadores (`src/app.rs:545` y
   `tests/visuales.rs:63`). No se acopla `ciudad.rs` a `config.rs`.
   **Seguimiento:** al integrar con la Fase 4a apareció un tercer llamador
   (`AppEstado::recargar_config`, la recarga de `config.toml` en caliente que
   reconstruye las visuales al cambiar de iconos), que seguía usando la firma
   antigua y dejaba `main` sin compilar. Se arregla pasando los ajustes por un
   único ayudante `ajustes_ciudad(&ConfigVisuales)` compartido por los dos sitios
   de `app.rs`, para que no vuelvan a divergir.
3. **Filas de historia: `ciudad_filas` actúa como tope.** El informe pide a la
   vez `F = clamp(alto/4, 8, 16)` y una clave `ciudad_filas` (8-16, por defecto
   12), sin decir cómo se combinan. Acordado con el desarrollador:
   `F = min(clamp(alto/4, 8, 16), ciudad_filas)` — con el valor por defecto, 24
   filas de terminal dan F = 8, 48 dan 12 y 66 siguen en 12.
4. **Regla de los glifos ascii.** El informe pide volcar `.` y `:` por celda sin
   decir qué regla los asigna. Acordado con el desarrollador: celda con 1-2
   puntos → `.`, con 3-4 → `:` (las paredes verticales salen `:` y las aristas
   horizontales, diagonales y las estrellas salen `.`, como el mockup).
5. **Alcance del color por altura.** El pseudocódigo de §7.1 divide por
   `e.alto_max` (por edificio) y §7.2 define `alto_max = 0,8 × ancho_ciudad`
   (global). Se usa la **global**: `paleta.gradiente(altura / alto_max)`, tal
   como pide literalmente el checklist (`paleta.gradiente`), de modo que el color
   codifica la altura absoluta en toda la ciudad. Con la paleta del tema,
   `gradiente` interpola del color de acento del tema al de texto, y
   `interpolar_color` trata `Color::Reset` con un escalón en t = 0,5: el
   degradado puede verse escalonado según el tema. Si se prefiere el gradiente
   por edificio (cada uno de tenue en su base a acento en su cima) es un cambio
   de una línea en `dibujar_prisma`.
6. **Estrellas repartidas por toda el área.** El informe dice «una cada 60
   celdas» y las dibuja antes que los edificios, pero no reserva el cielo a la
   franja superior; se reparten por toda la rejilla y son los propios edificios
   (y el borrado de polígonos) los que las tapan.
7. **Constante de encuadre.** El informe solo dice que la relación de aspecto de
   los puntos braille ≈ 1:1 no necesita corrección adicional. La proyección
   normalizada se lleva a puntos con escala uniforme (`min(ancho, alto)` de la
   rejilla) y una constante `ENCUADRE` (0,7) que fija cuánto ocupa la ciudad:
   con los valores del informe (altura de cámara 0,55 × ancho y edificios de
   hasta 0,8 × ancho) las torres más cercanas se salen por arriba del encuadre;
   con 0,7 se ve el cielo con estrellas arriba y la ciudad llena el resto.
8. **Origen de la ciudad.** El informe sitúa la fila más reciente en «z = 0» y
   las anteriores alejándose (z = 1..F−1), pero también hace orbitar la cámara
   alrededor del centro de la ciudad. La ciudad se centra en el origen
   (`x = i − (N−1)/2`, `z = j − (F−1)/2`, con j = 0 la fila más reciente), de
   modo que la cámara orbita ese centro sin parámetro extra y se conserva lo
   esencial: la fila más reciente es el borde delantero y las anteriores se
   alejan.
9. **«Una fila cada 2 frames» depende de los fps.** Se implementa literal (a 30
   fps son 15 filas/s ≈ 0,8 s de historia con F = 12); a 60 fps la historia dura
   la mitad y a 15 fps el doble. Queda anotado por si se prefiere un acumulador
   temporal independiente de `fps`.
10. **Copia de `CLAUDE.md` dentro de `docs/` (hallazgo para el desarrollador).**
   `docs/fase3a/CLAUDE.md` es idéntico byte a byte al de la raíz (mismo md5
   `92fb2089a3460dd3600c021aaaf83d70`), lo que incumple la regla T-37; el
   maestro ya tiene R-26 abierto por la copia de `docs/fase4/`. También hay
   copias versionadas en `docs/fase3/` y `docs/fase5/`, y `AGENTS.md` (raíz) es
   otra copia exacta. **No se ha tocado ninguna** (decisión del desarrollador:
   solo avisar).

---

## Estructura de archivos creada/modificada

**Nuevos**

```
src/visuales/proyeccion.rs   Punto3/Punto2/Dimensiones/Cara/Camara: órbita, perspectiva (FOV 55°),
                             descarte de caras por normal · vista, caras_de_prisma (devanado).
src/visuales/raster.rs       Raster de 2×4 puntos por celda con color por celda, limpiar, punto,
                             linea (Bresenham punteada), borrar_poligono (scanline par-impar),
                             volcar (Points por color en braille; glifos por celda en ascii).
src/visuales/ciudad.rs       Visual «Ciudad»: AjustesCiudad, Prisma, Historia (anillo de filas),
                             Estrellas, Ciudad (Visual), dibujar_prisma, a_puntos.
tests/ciudad.rs              9 pruebas: séptima visual, culling, borrado, punteada, ocultación (AC),
                             render en tres tamaños, redimensionado, ambiental y presupuesto de frame.
docs/fase3a/mmmusic-fase3a-implementacion.md   este informe.
```

**Modificados**

```
src/visuales/mod.rs          +ciudad, +proyeccion, +raster; NOMBRES a 7 con «Ciudad» tras «Túnel»;
                             registro(ascii, &AjustesCiudad).
src/app.rs                   AjustesCiudad desde config.visuales; tecla '0' → saltar_visual(6).
src/config.rs                ciudad_vuelta_s (5-120, 20), ciudad_filas (8-16, 12),
                             ciudad_punteado (true) en ConfigVisuales, su Default y su validación.
config.ejemplo.toml          las tres claves comentadas en [visuales] y «ciudad» en la lista de
                             valores de predeterminada.
src/ui/vistas/visual.rs      línea de atajos: «0 ciudad».
src/ui/vistas/ayuda.rs       fila («0», «Ciudad (espectro 3D en órbita)»).
README.md                    séptima visual, cómo funciona, sus tres claves y la tecla 0.
tests/visuales.rs            registro con ajustes; 6 → 7 visuales; «Visual 1/6» → «Visual 1/7».
docs/fase3a/mmmusic-fase3a-checklist.md  casillas y contadores.
```

---

## Decisiones técnicas tomadas durante el desarrollo

1. **Raster con máscara por celda y color por celda.** `mascaras: Vec<u8>` (ocho
   bits por celda, con el mismo orden de bits que el `PatternGrid` braille de
   `ratatui`) y `colores: Vec<Option<Color>>`, donde gana el último punto
   dibujado — exactamente el «color de celda = último punto» del informe. Como
   cada celda pertenece a un único grupo de color, volcar un `Points` por color
   es exacto: ninguna celda se pinta dos veces ni con el color equivocado.
2. **Sin asignaciones por frame.** La máscara, los colores y los buffers del
   volcado se reservan una vez por tamaño. El agrupado por color reutiliza un
   pool de `Vec<(f64, f64)>` y una caché del último color pedido (las aristas de
   un edificio comparten color, así que casi todas las celdas consecutivas
   aciertan sin recorrer el pool). `pintar_edificios` no reserva: usa arrays
   fijos en pila para las aristas ya trazadas.
3. **Dos ocultaciones distintas según el modo.** En braille el borrado apaga
   puntos sueltos (resolución 2 × 4). En ascii la celda es atómica, así que el
   borrado limpia la celda entera en cuanto algún punto suyo cae dentro: la
   ocultación «se aproxima a celda», como pide el informe, y tapa de más (nunca
   de menos).
4. **Los dos mapeos de coordenadas son inversos de fórmulas distintas.** Se
   verificó en la fuente de `ratatui-widgets 0.3.2`: `Painter::get_point`
   redondea con `(res-1)/(right-left)` y las etiquetas de `Context::print`
   truncan con `(ancho-1)/ancho`. Volcar con la fórmula del otro desplaza el
   dibujo una celda. Los mapeos quedan documentados en `raster.rs` y cubiertos
   por pruebas que pintan un punto y comprueban el glifo y el color en la celda
   exacta del `Buffer`.
5. **Deduplicación de aristas por par de índices.** Cada arista compartida se
   traza una sola vez normalizando el par `(i, j)` a `(min, max)` y buscándolo
   en un array fijo. Es lo que pide el informe («sin duplicados») y no depende
   del orden de recorrido de las caras.
6. **Devanado de las caras y prueba que lo vigila.** La normal se deduce del
   devanado de los cuatro vértices (`Cara::nueva`). La primera versión tenía las
   caras laterales y la tapa orientadas hacia dentro y lo detectó la prueba
   `todas_las_normales_apuntan_hacia_fuera`, que comprueba que cada normal apunta
   hacia fuera del centro del prisma. Se conserva como red de seguridad.
7. **La base no se dibuja por descarte, no por caso especial.** La cara inferior
   tiene la normal hacia abajo y la cámara siempre está por encima del suelo
   (0,55 × ancho), así que el culling la elimina sola: «sin base» se cumple sin
   código extra y las aristas inferiores de las caras laterales sí se trazan, que
   es lo que dibuja el mockup.
8. **Historia en anillo con buffers reutilizados.** `VecDeque<Vec<f32>>`: al
   entrar una fila se reaprovecha el `Vec` de la fila que sale, sin asignar. Solo
   se dibujan las filas ya llenas, de modo que tras `reiniciar()` (cambio de
   visual o redimensionado) la ciudad vuelve a crecer desde el borde delantero
   en vez de aparecer con F × N prismas de golpe.
9. **Estrellas deterministas.** `StdRng::seed_from_u64` con semilla derivada de
   las celdas de la terminal: las posiciones y los periodos son estables entre
   frames y cambian al redimensionar. El parpadeo es senoidal con periodo de 2 a
   5 s por estrella y se enciende por encima de 0,5, siempre en `paleta.tenue`.
10. **Tipos enteros en la configuración.** `ConfigVisuales` deriva `Eq`, así que
    un `f32` en `ciudad_vuelta_s` rompería el derive; se usa `u32`/`u16` y la
    conversión a `f32`/`f64` se hace al construir `AjustesCiudad`.
11. **Medición del presupuesto de frame.** Se añade una prueba `#[ignore]`
    (`el_frame_cabe_en_el_presupuesto_de_la_fase_3`) que mide mediana y p95 del
    frame completo a 200 × 50 (288 prismas) y en el peor caso posible, 200 × 70
    con 16 filas (384 prismas), y falla solo si el p95 supera el presupuesto de
    33 ms del tick de 30 fps. Se ejecuta a mano: `cargo test --release --test
    ciudad -- --ignored --nocapture`. Los números van al resumen de este informe
    (cierre de R-27). A 200 × 50 la altura de la terminal solo pide 12 filas
    (`clamp(alto/4, 8, 16)`), así que el máximo real de prismas exige medir
    aparte a 200 × 70.

---

## Funcionalidades del checklist completadas

### Geometría y raster

- [x] `proyeccion.rs` sin E/S: `Camara::orbita(t, rms, dims, vuelta_s)` con radio 0,9 × máximo(ancho, profundidad), altura 0,55 × ancho + balanceo 0,08 × ancho × RMS, objetivo al centro; `proyectar` en perspectiva (FOV 55°) devolviendo `None` tras el plano cercano; `cara_visible` por normal · vista
- [x] `raster.rs` sin E/S: raster de puntos 2×4 por celda con `limpiar`, `punto`, `linea` (Bresenham, punteada un punto de cada dos), `borrar_poligono` (scanline) y `volcar` al `Canvas` como `Points` agrupados por color, con color de celda = último punto dibujado
- [x] El raster se reserva una vez por tamaño y se reutiliza sin asignaciones por frame
- [x] Tests: una cara que mira a la cámara es visible y su opuesta no; un polígono borrado elimina los puntos previos de su interior; una línea punteada tiene la mitad de puntos que la continua
  - AC: Dados dos edificios alineados con la cámara, cuando se dibuja el frame, entonces no queda ningún punto del edificio trasero dentro de la silueta del delantero

### Visual Ciudad

- [x] `ciudad.rs` implementa `Visual` con nombre "Ciudad": historia de F filas (`clamp(alto/4, 8, 16)`, config `ciudad_filas`) de N bandas (`clamp(ancho/6, 8, 24)`), una fila nueva cada 2 frames, fila más reciente en z = 0
- [x] Edificios como prismas de huella 0,7 × 0,7 y altura `banda × 0,8 × ancho_ciudad`, con altura mínima 0,05
- [x] Dibujo de lejos a cerca: borrado de las caras visibles y después aristas punteadas, con aristas compartidas dibujadas una vez y sin base
- [x] Color de cada arista por altura con `paleta.gradiente` (tenue en la base, acento en la cima)
- [x] Cielo de estrellas: una cada 60 celdas, posiciones estables por semilla del tamaño, parpadeo por ruido lento (2-5 s) en tenue, dibujadas antes que los edificios
- [x] Cámara en órbita continua con una vuelta cada `ciudad_vuelta_s` (5-120, por defecto 20)
- [x] Modo ambiental: historia alimentada por `Analisis::ambiental`, ciudad baja y ondulante con la órbita en marcha
- [x] Funciona en 40×12 (N = 8, F = 8) y con `reiniciar()` al redimensionar; entra en el presupuesto de frame de F3 (degradación a 15 fps intacta)
- [x] Modo `ascii`: volcado del raster a `.` y `:` por celda; `ciudad_punteado = false` dibuja aristas continuas

### Integración

- [x] Ciudad registrada como séptima visual: `v`/`V` la incluyen tras Túnel y `0` la selecciona en modo visual (`7` sigue siendo salir); cabecera "Visual 7/7 Ciudad"
- [x] `AJUSTES.visual_actual` admite `ciudad` y `visuales.predeterminada = "ciudad"` es válido
- [x] Claves `ciudad_vuelta_s`, `ciudad_filas` y `ciudad_punteado` en `[visuales]`, validadas y en `config.ejemplo.toml`
- [x] Ayuda del modo visual y README actualizados con la visual y la tecla `0`
- [x] Tests de render: Ciudad dibuja sin pánico en 40×12, 80×24 y 200×50 con análisis real, ambiental y vacío, en braille y ascii
- [x] `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios

---

## Pendientes y bloqueos

- **Validación manual en Omarchy (desarrollador).** No se puede hacer desde el
  entorno de desarrollo: estética de la ciudad y del parpadeo de estrellas con
  audio real, comportamiento a 40×12 y a 200×50, que la degradación a 15 fps no
  se dispare y que el redimensionado recompile bien la vista.
- **Teclas del modo visual.** No hay pruebas automáticas de las teclas porque
  construir un `ContextoApp` exige el hilo de mpv y una conexión a la base; el
  mapeo (`0` → Ciudad, `7`/`Esc` → salir, `v`/`V` → ciclo) se ha revisado por
  lectura y conviene confirmarlo a mano.
- **Copias de `CLAUDE.md` dentro de `docs/`** (`fase3a` idéntica a la raíz, y
  también en `fase3`, `fase4` —R-26—, `fase5`, más `AGENTS.md`): decisión del
  desarrollador aplazada; no se ha tocado ninguna.
- **Recorte de las cimas.** Con las constantes del informe (cámara a 0,55 ×
  ancho, edificios de hasta 0,8 × ancho) las torres más cercanas y altas se
  salen por arriba del encuadre. No se ha cambiado nada del informe; si molesta
  visualmente, basta bajar `ENCUADRE` en `ciudad.rs`.
- **Degradado por color.** Con la paleta del tema, `Paleta::gradiente` pasa por
  `Color::Reset` y el escalón de `interpolar_color` en t = 0,5 puede hacer que el
  degradado por altura se vea a saltos. Anotado en las desviaciones.
- **R-25 (ajeno a esta fase):** `time-pos` se publica cada 250 ms aunque la vista
  Letras se repinte cada 100 ms.

---

## Ejecución y pruebas

**Arrancar** (perfil aislado, sin tocar el del usuario):

```bash
XDG_CONFIG_HOME=$D/cfg XDG_DATA_HOME=$D/data XDG_CACHE_HOME=$D/cache \
XDG_STATE_HOME=$D/state cargo run
```

En el modo visual: `7` entra y sale, `0` va a Ciudad, `v`/`V` la ciclan, `[`/`]`
sensibilidad, `b` paleta y `?` ayuda. La cabecera muestra «Visual 7/7 Ciudad».

**Migraciones:** ninguna. La fase no toca la base de datos: `AJUSTES.visual_actual`
acepta `ciudad` porque el nombre se resuelve por `visuales::indice_por_nombre`, y
las tres claves nuevas de `[visuales]` viven solo en `config.toml`.

**Comandos de cierre:**

```bash
cargo run
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

**Pruebas de la fase:**

```bash
cargo test --test ciudad             # 9 pruebas, incluidas la ocultación (AC) y el render
cargo test --lib visuales::          # pruebas internas de proyeccion y raster
cargo test --release --test ciudad -- --ignored --nocapture   # presupuesto de frame (R-27)
```

Medición obtenida en `--release`: **1,09 ms de mediana y 1,14 ms de p95** a
200 × 50 (288 prismas) y **1,70 ms / 2,15 ms** a 200 × 70 con 16 filas (384
prismas, el peor caso); en depuración, 6,75 ms y 10,42 ms de mediana
respectivamente.
