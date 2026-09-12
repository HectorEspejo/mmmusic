# mmmusic - Fase 3: Informe de Implementación

**Documento vivo.** Última actualización: 12 de septiembre de 2026 (sesión 1: fase completa implementada; validación manual de captura/estética pendiente por el desarrollador).

Estado por sprint:

| Sprint | Contenido | Estado |
|--------|-----------|--------|
| S1 | Migración 003, R-10, captura PipeWire y análisis | Completado y verificado (captura real validada en este equipo) |
| S2 | Trait Visual, Espectro, modo pantalla completa, mini espectro y degradación | Completado y verificado |
| S3 | Resto de visuales, median cut y paleta de carátula | Completado y verificado |
| S4 | Protector, configuración, ASCII, tests y cierre | Completado; queda validación manual en Omarchy |

## Resumen de lo implementado

### Migración y corrección heredada

- Migración `003_colores_albumes.sql`: columna `colores TEXT` en ALBUMES (`user_version = 3`). Las claves `visual_actual`, `paleta_fuente` y `sensibilidad` se guardan en AJUSTES por código, sembradas desde la configuración la primera vez (ver Desviaciones).
- Corrección R-10: al revertir un grupo "Varios artistas" en un escaneo incremental, las pistas que no se han releído en ese escaneo (no están en el conjunto de rutas procesadas) se releen del fichero antes de reasignarlas, de modo que `artista`, `album` y `anio` salen de sus etiquetas. Test de regresión en `tests/recopilatorios.rs` etiquetando `albumartist` en una sola pista: quedan tres álbumes normales con los años 2020/2019/2021 de sus ficheros.
- Pasada en segundo plano al arrancar (`caratulas::lanzar_colores_pendientes`) que calcula `colores` para los álbumes con `caratula_ruta` y `colores IS NULL`, con conexión SQLite propia y cancelación al salir.

### Captura de audio (PipeWire)

- mpv arranca con `ao=pipewire,` (la coma final da la caída a la selección automática si PipeWire no está) y `audio-client-name=mmmusic`.
- `src/audio/`: hilo de captura con `MainLoopRc`/`ContextRc`/`CoreRc` y registro (`src/audio/pipewire.rs`), anillo SPSC con `AtomicU32` por muestra (`src/audio/anillo.rs`) y análisis (`src/audio/analisis.rs`).
- Localización del nodo por `application.name`/`node.name` = `visuales.nodo` y por el cliente PipeWire al que pertenece el nodo (el PID propio y el nombre del cliente viven en el global `Client`; ver Desviaciones). Se prefiere el candidato propio.
- Stream F32 estéreo (2 canales, tasa del nodo) con `target.object=<serial>`, `stream.dont-remix` y `RT_PROCESS`; callbacks `param_changed`/`state_changed`/`process`. El callback `process` solo escribe en el anillo (`escribir_bytes_f32_le`, sin bloqueos ni asignaciones).
- Fallback a `stream.capture.sink` (primero sobre el mismo nodo, después sin `target.object`) anunciado en la cabecera como "captura por monitor".
- Estados `Desconectada | BuscandoNodo | Capturando { tasa_hz, canales, monitor } | SinAudio | Error(String)` publicados por `watch<EstadoCaptura>`; reconexión con `global_remove`/nuevo global; reintentos de conexión cada 30 s hasta 5 y toast final "Visuales sin audio: PipeWire no disponible".
- `ComandoCaptura` (Iniciar, Detener, Apagar) atendido por un timer de 100 ms; `q` apaga el hilo limpiamente.
- Validación real en este equipo (con el demonio PipeWire y una fuente sonando): nodo encontrado, `Capturando { tasa_hz: 48000, canales: 2, monitor: false }` y ~49 000 frames/s entrando en el anillo. Con la fuente en pausa el enlace queda `Paused` y se publica `sin_audio` (modo ambiental) sin disparar el fallback.

### Análisis

- `analisis.rs` sin E/S: mezcla a mono, ventana Hann, `realfft` de 2048, magnitudes de un lado, bandas logarítmicas 20 Hz–16 kHz (`clamp(ancho/2, 16, 128)`, pico por intervalo), escala −60..0 dB → 0..1, AGC de ataque 50 ms / liberación 2 s (objetivo 0.85, ganancia 0.5–8) × sensibilidad, suavizado con subida inmediata y gravedad 0.9, onda de `ancho×2`, RMS/pico L/R, graves 20–150 Hz y pulso >1.4 × media móvil de 1 s con mínimo 250 ms.
- Modo ambiental sintético suave cuando no hay audio (sin nodo, error) o tras 2 s de silencio en pausa. Si llegan menos de 2048 muestras nuevas se reutiliza la ventana anterior.
- `tests/analisis.rs` (9 tests): seno de 1 kHz a −6 dB en su banda (AC cumplido), silencio a cero, pulso periódico, convergencia del AGC, sensibilidad, ambiental, pausa larga, acotado de bandas y etiquetas de estado.

### Modo visual, visuales, paleta, mini espectro y protector

- Trait `Visual` (`nombre`, `reiniciar`, `dibujar`) y registro de las seis visuales: Espectro, Barras y ondas, Ambiente, Partículas, Caleidoscopio y Túnel, con estado propio y modo ambiental automático.
- Vista a pantalla completa (`ui/vistas/visual.rs`) con Canvas braille, cabecera autoocultable a los 3 s (visual n/6, paleta, sensibilidad, estado de captura), línea de ayuda fija, tamaño mínimo 40×12 (por debajo solo Espectro) y `reiniciar()` al redimensionar.
- `7` entra y sale, `Esc` sale en modo manual y se vuelve a la vista y pantalla previas; `v`/`V`, `1`–`6`, `[`/`]` (×0.8/×1.25, 0.25–4.0), `b`; atajos de reproducción y `L` activos. Persistencia en AJUSTES. Degradación a 15 fps con toast único y vuelta tras 10 s de margen.
- Paleta de seis colores con `gradiente(t)` e `interpolar`; paleta del tema (recarga en caliente) o de los cinco colores dominantes de la carátula, con transición de 400 ms. Median cut determinista sobre 64×64, descartando casi negros/blancos si queda ≥20 %, orden por luminancia y contraste ≥2:1 del acento sobre el fondo. `tests/paleta.rs` cubre el AC de los cuatro cuadrantes y la determinación.
- Mini espectro de 12 barras en la barra inferior, oculto con <70 columnas o alto <20, con `mini_espectro=false` o `visuales.activo=false`.
- Protector de pantalla por `autoinicio_min` (solo sin diálogo y con el reproductor no detenido); la primera tecla o clic se consume; sale solo si el reproductor se detiene.
- Sección `[visuales]` validada y acotada, `config.ejemplo.toml`, ayuda y README actualizados (dependencias, fallback por monitor y aviso de fotosensibilidad).

## Desviaciones respecto a la especificación (qué y por qué)

1. **`Capturando` incluye `monitor: bool`.** La API del informe definía `Capturando { tasa_hz, canales }`, pero la cabecera debe anunciar "captura por monitor"; se añadió el campo al estado en lugar de un canal aparte.
2. **Descubrimiento del nodo a través del cliente PipeWire.** Con libmpv real, el nodo del stream expone `application.name = "mpv"` y `node.name = "mpv"` y **no** trae `application.process.id`; el PID propio y el nombre configurado viven en el global `Client` (los del cliente). Se escuchan también los globales `Client` y se enlaza cada nodo por `client.id`, prefiriendo el cliente con PID propio y conservando la coincidencia directa por `application.name`/`node.name`.
3. **Claves de AJUSTES sembradas por código.** La migración 003 solo añade `colores`; `visual_actual`, `paleta_fuente` y `sensibilidad` se leen de AJUSTES y, si faltan, se siembran desde `[visuales]` (si no, `visuales.predeterminada` quedaría ignorada por un valor fijo en SQL).
4. **AGC con constantes de tiempo.** Se usa `1 − exp(−dt/τ)` con τ de 50 ms (ataque) y 2 s (liberación) para cumplir el checklist con independencia del fps; a 30 fps equivale al pseudocódigo del informe (0.3 / 0.01 por frame).
5. **Escala de magnitudes `pico / N`.** Para que el primer lóbulo lateral de Hann quede por debajo del 30 % del pico en la escala −60..0 dB (AC del seno de 1 kHz), las magnitudes se normalizan por `N` (incluye la ganancia coherente de Hann). Un seno a plena escala ronda −12 dB.
6. **Marcas de pico en las visuales.** `Analisis` no expone las marcas; las gestionan Espectro y Barras y ondas con su propia gravedad, que es donde se dibujan.
7. **Fallback por monitor.** Se prueba `stream.capture.sink` con el mismo `target.object` y, si el enlace sigue fallando, sin target (sink por defecto). El informe no detallaba cómo localizar el sink del nodo.
8. **Enlace `Paused` no es fallo.** Un stream que llega a `Paused` está enlazado aunque la fuente no produzca formato todavía (reproductor en pausa/detenido): se publica `sin_audio` (ambiental) y no se lanza el fallback. El fallback solo se dispara si sigue en `Connecting`/`Unconnected` tras 3 s o si llega `Error`.
9. **`7` como atajo principal.** El diagrama mencionaba "Enter sobre 7 Visual", pero la sidebar no es navegable hoy; se añadió `Vista::Visual` y el atajo `7` (la sección se resalta cuando el modo está activo y se oculta con `visuales.activo=false`).
10. **Línea de ayuda fija en modo visual.** Solo la cabecera se autooculta; la línea de teclas queda visible, como en el mockup.
11. **Pasada de colores incondicional.** Se ejecuta al arrancar aunque `visuales.activo=false`, porque el checklist no la condiciona; solo omite el trabajo si no hay carátulas pendientes.
12. **Colores calculados en tres puntos con la misma función pura**: el escáner al cachear la carátula, la pasada de arranque y el worker de imágenes a demanda (al cambiar de pista sin colores). El guardado lo hace quien tiene conexión (escáner/pasada) o el hilo de UI (respuesta del worker).
13. **`ao=pipewire,` con coma final** para que mpv caiga a su selección automática si PipeWire no está disponible; el informe pedía la caída pero no el mecanismo.

## Estructura de archivos creada/modificada

Nuevos:

```
src/audio/mod.rs
src/audio/anillo.rs
src/audio/analisis.rs
src/audio/pipewire.rs
src/visuales/mod.rs
src/visuales/paleta.rs
src/visuales/espectro.rs
src/visuales/barras_ondas.rs
src/visuales/ambiente.rs
src/visuales/particulas.rs
src/visuales/caleidoscopio.rs
src/visuales/tunel.rs
src/visuales/mini_espectro.rs
src/ui/inactividad.rs
src/ui/vistas/visual.rs
src/biblioteca/migraciones/003_colores_albumes.sql
tests/analisis.rs
tests/paleta.rs
tests/visuales.rs
docs/fase3/mmmusic-fase3-implementacion.md
```

Modificados:

```
Cargo.toml / Cargo.lock            (+pipewire con feature v0_3_44, +realfft)
src/lib.rs                         (+audio, +visuales)
src/config.rs                      (+[visuales], +FuentePaleta, validación) + tests
src/reproductor/mpv.rs             (+ao=pipewire, +audio-client-name)
src/biblioteca/bd.rs               (migración 3) + test
src/biblioteca/consultas.rs        (+albumes::{colores, fijar_colores, sin_colores})
src/biblioteca/caratulas.rs        (colores al cachear, pasada y hilo de colores)
src/biblioteca/escaner.rs          (rutas releídas, asegurar_* pub(crate))
src/biblioteca/recopilatorios.rs   (R-10)
src/ui/componentes/imagen.rs       (PeticionCaratula/RespuestaCaratula y colores)
src/ui/barra_inferior.rs           (mini espectro)
src/ui/mod.rs                      (dispatch del modo visual y tamaño)
src/ui/sidebar.rs                  (7 Visual y ocultado con activo=false)
src/ui/teclas.rs                   (7 → IrA(Visual)) + tests
src/ui/vistas/mod.rs               (+visual, arm)
src/ui/vistas/ayuda.rs             (+sección Visual)
src/app.rs                         (estado visual, teclas, paleta, análisis, fps, protector)
src/main.rs                        (captura, colores, tick dinámico, respuesta de carátulas)
config.ejemplo.toml                (+[visuales])
README.md                          (visuales, dependencias, aviso de fotosensibilidad)
tests/recopilatorios.rs            (+regresión R-10)
```

## Decisiones técnicas tomadas durante el desarrollo

- **Anillo sin `unsafe`:** cada muestra es un `AtomicU32` con los bits del `f32`; el productor guarda con `Relaxed` y publica el contador con `Release`, el consumidor lee con `Acquire`. El callback de tiempo real no bloquea ni asigna.
- **Watch para la captura y polling en el bucle:** `watch<EstadoCaptura>` es la fuente de verdad; el hilo de UI lo refresca antes de cada frame. El canal `AppEvento` se reserva para toasts y eventos de un solo uso.
- **Timer de 100 ms en el hilo de captura** para atender `ComandoCaptura` y timeouts (nodo, enlace, fallback) sin bloquear el bucle de PipeWire. El temporizador debe armarse con `value > 0` (un `Duration::ZERO` lo desactiva).
- **`try_borrow_mut` en los callbacks de stream:** `stream.connect()` y `disconnect()` disparan `state_changed` de forma síncrona; los callbacks ignoran la reentrada en lugar de entrar en pánico por `RefCell`.
- **`Visual` compartida con `Rc<RefCell<Box<dyn Visual>>>`:** el closure de `Canvas::paint` es `Fn`, así que el estado mutable de la visual se toma por préstamo interior.
- **Degradación de fps midiendo el frame completo** (análisis + paleta + dibujo) en el bucle principal; el tick base de 250 ms se mantiene cuando no hay visual ni mini espectro.
- **Tamaño mínimo en modo visual** forzando el índice 0 solo al dibujar, sin cambiar la visual persistida.
- **Colores de carátula como texto `#rrggbb,#...`** en una columna, reutilizando la paleta también para el indicador de fuente.

## Funcionalidades del checklist completadas (copiando su texto exacto)

### Migración y corrección heredada
- [x] Migración `003_colores_albumes.sql`: columna `colores` en ALBUMES y claves `visual_actual`, `paleta_fuente` y `sensibilidad` en AJUSTES
- [x] Corrección R-10: al revertir un grupo "Varios artistas" en escaneo incremental se releen las etiquetas de las pistas no releídas antes de reasignarlas
  - AC: Dado un recopilatorio agrupado, cuando se etiqueta `albumartist` en una sola pista y se reescanea incrementalmente, entonces todas las pistas del grupo vuelven a álbumes normales con el `anio` de su fichero
- [x] Pasada en segundo plano al arrancar que calcula `colores` para los álbumes con carátula y `colores IS NULL`

### Captura de audio (PipeWire)
- [x] mpv inicializado además con `ao=pipewire` y `audio-client-name=mmmusic`, con caída a la selección automática de mpv si PipeWire no está disponible
- [x] Hilo de captura con `pipewire` (bucle principal, contexto, core y registro) arrancado solo si `visuales.activo = true`
- [x] Localización del nodo por `application.name` (o `node.name`) igual a `visuales.nodo`, prefiriendo el que tenga `application.process.id` igual al PID propio
- [x] Stream de captura F32 estéreo a la tasa del nodo, enlazado con `target.object` al serial del nodo y `stream.dont-remix`
- [x] Fallback a `stream.capture.sink` sobre el sink del nodo cuando el enlace directo falla, anunciado en la cabecera como "captura por monitor"
- [x] Máquina de estados de captura: desconectada, buscando_nodo, capturando, sin_audio, error, publicada por `watch<EstadoCaptura>`
- [x] Reconexión automática al desaparecer y reaparecer el nodo (`global_remove` / nuevo `global`)
- [x] Reintentos de conexión a PipeWire cada 30 s hasta 5 veces; después estado error con toast "Visuales sin audio: PipeWire no disponible"
- [x] Anillo SPSC sin bloqueos de 8192 muestras por canal; el callback `process` nunca bloquea ni asigna memoria
  - AC: Dado el reproductor sonando, cuando se leen 2048 muestras desde la UI a 30 fps durante 60 s, entonces no hay xruns en el log de PipeWire ni pánicos
- [x] `ComandoCaptura` (Iniciar, Detener, Apagar) y parada limpia al salir con `q`
- [x] Con dos instancias de mmmusic cada una captura su propio nodo

### Análisis
- [x] `analisis.rs` sin E/S: mezcla a mono, ventana Hann, `realfft` de 2048 muestras y magnitudes
- [x] Bandas logarítmicas 20 Hz–16 kHz, número `clamp(ancho/2, 16, 128)` recalculado al redimensionar, pico por intervalo y escala −60..0 dB → 0..1
- [x] AGC lento (objetivo 0.85, ataque 50 ms, liberación 2 s, ganancia acotada 0.5–8) multiplicado por `sensibilidad`
- [x] Suavizado con subida inmediata y bajada con gravedad 0.9 por frame, y marca de pico que cae
- [x] Onda de `ancho×2` muestras submuestreadas, RMS y pico por canal, energía de graves 20–150 Hz
- [x] Detección de pulso: graves > 1.4 × media móvil de 1 s con mínimo 250 ms entre pulsos
- [x] Modo ambiental: valores sintéticos suaves dependientes del tiempo cuando no hay audio (sin_audio, error o 2 s de silencio en pausa)
- [x] Tests con señales sintéticas: seno de 1 kHz cae en la banda correcta, silencio da 0, pulso periódico se detecta, AGC converge
  - AC: Dado un seno de 1 kHz a −6 dB, cuando se analiza, entonces la banda con máximo contiene 1 kHz y ninguna otra supera el 30 % de ese valor

### Modo visual (pantalla completa)
- [x] Sección "7 Visual" en la sidebar y vista a pantalla completa que oculta sidebar y contenido y conserva la barra inferior
- [x] `7` entra y sale; `Esc` sale en modo manual; se vuelve a la vista anterior
- [x] Tick de 33 ms solo en modo visual o con mini espectro visible; 250 ms en el resto
- [x] Cabecera con nombre y número de visual, fuente de paleta, sensibilidad y estado de captura cuando no es capturando, autoocultable a los 3 s
- [x] `v` / `V` ciclan las visuales y `1`–`6` saltan a la n; el orden es Espectro, Barras y ondas, Ambiente, Partículas, Caleidoscopio, Túnel
- [x] `[` / `]` ajustan la sensibilidad ×0.8 / ×1.25 acotada 0.25–4.0 con indicador temporal
- [x] Los atajos de reproducción y `L` funcionan en modo visual
- [x] Persistencia de `visual_actual`, `paleta_fuente` y `sensibilidad` en AJUSTES
- [x] Degradación de fps a 15 si tres frames seguidos superan 33 ms, con toast una sola vez, y vuelta a 30 tras 10 s de margen
- [x] Tamaño mínimo 40×12; por debajo solo se muestra el Espectro
- [x] Redibujado y `reiniciar()` de la visual al redimensionar la terminal

### Visuales
- [x] Trait `Visual` (nombre, reiniciar, dibujar sobre `canvas::Context`) y registro de las seis visuales
- [x] Espectro: barras con gradiente primario→acento, marca de pico con gravedad y eje 20 Hz–16 kHz
- [x] Barras y ondas: osciloscopio braille arriba, 8–16 barras anchas con reflejo tenue abajo y medidores L/R con pico
- [x] Ambiente: 3–5 manchas braille que derivan lentamente, tamaño por RMS, color por gradiente y latido en el pulso
- [x] Partículas: emisor central, 20–60 partículas por pulso con velocidad proporcional a los graves, brillo por agudos, vida 1–2 s, estela y máximo 400 vivas
- [x] Caleidoscopio: 6 sectores simétricos con polilíneas braille cuyos radios son las bandas, rotación lenta y giro extra en cada pulso
- [x] Túnel: anillos concéntricos que avanzan hacia el espectador modulados por las bandas, velocidad por RMS
- [x] Todas las visuales muestran el modo ambiental con la misma geometría cuando `ambiental = true`
- [x] Modo `iconos = "ascii"`: barras con `#`, onda con `-`/`_`/`~`, braille conservado
- [x] Tests: cada visual dibuja sin pánico en 40×12, 80×24 y 200×50 con análisis real, ambiental y vacío

### Paleta
- [x] `Paleta` (fondo, primario, secundario, acento, resalte, tenue) con `gradiente(t)` e `interpolar`
- [x] Paleta desde el tema de Omarchy, recargada en caliente con el tema (F1)
- [x] Extracción de 5 colores dominantes por median cut sobre 64×64, descartando casi negros y casi blancos si queda ≥ 20 % de píxeles, ordenados por luminancia, con contraste mínimo 2:1 del acento sobre el fondo
  - AC: Dada una imagen sintética con cuatro cuadrantes de colores puros, cuando se cuantiza, entonces los cuatro aparecen entre los colores devueltos y el resultado es idéntico en dos ejecuciones
- [x] `colores` calculado en el hilo auxiliar de imágenes al cachear la carátula y guardado en ALBUMES
- [x] `b` alterna la fuente de paleta tema ↔ carátula; sin carátula se usa la del tema con indicador ♪
- [x] Transición de 400 ms entre paletas al cambiar de pista o de fuente

### Mini espectro
- [x] 12 barras de una fila con `▁▂▃▄▅▆▇█` en la barra inferior, a la izquierda de los controles, con las mismas bandas suavizadas submuestreadas
- [x] Oculto en modo compacto (< 70 columnas), con `mini_espectro = false` o `visuales.activo = false`

### Protector de pantalla
- [x] Temporizador de inactividad que entra en modo visual tras `autoinicio_min` minutos sin teclado ni ratón, solo si no hay diálogo abierto y el reproductor no está detenido
- [x] Cualquier tecla o clic sale del protector y se consume sin ejecutar su acción
- [x] Salida automática del protector si el reproductor se detiene

### Configuración y entrega
- [x] Sección `[visuales]` en config: activo, fps (15–60), predeterminada, paleta, mini_espectro, autoinicio_min, nodo; validada y acotada
- [x] `visuales.activo = false` elimina el hilo de captura, el mini espectro y la sección 7
- [x] `config.ejemplo.toml` y overlay de ayuda actualizados con los atajos del modo visual
- [x] README: dependencias `pipewire` y `clang`, configuración de visuales, fallback por monitor y aviso de fotosensibilidad
- [x] `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios

## Pendientes y bloqueos

- **Validación manual en Omarchy (desarrollador):** dos instancias de mmmusic capturando cada una su nodo; fallback real por monitor forzando el fallo del enlace directo; ausencia de xruns durante 60 s a 30 fps (AC del anillo); estética fina de las seis visuales, del fundido de paleta y del protector.
- **`visuales.activo = true` por defecto:** al arrancar se crea el hilo de captura aunque no se entre al modo visual; si se prefiere un arranque sin PipeWire, basta poner `activo = false` en la configuración.
- **La captura usa el hilo de PipeWire del propio proceso:** probado con fuentes en reproducción y en pausa, pero la prueba con dos instancias queda para el entorno real.

## Ejecución y pruebas

Arranque normal (la migración 003 se aplica sola al abrir la base de datos en cualquier hilo):

```bash
cargo run
cargo run --release
```

Pruebas manuales aisladas sin tocar el perfil real:

```bash
mkdir -p /tmp/mmmusic-pruebas/{config,data,cache,state}
XDG_CONFIG_HOME=/tmp/mmmusic-pruebas/config \
XDG_DATA_HOME=/tmp/mmmusic-pruebas/data \
XDG_CACHE_HOME=/tmp/mmmusic-pruebas/cache \
XDG_STATE_HOME=/tmp/mmmusic-pruebas/state \
cargo run
```

Suite y calidad:

```bash
cargo test                        # 70 unitarios + 40 de integración (analisis, paleta, visuales, escaneo, recopilatorios, cola, scrobbling)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Qué cubre cada suite nueva:

- `tests/analisis.rs`: bandas y AC del seno de 1 kHz, silencio, pulso, AGC, sensibilidad, ambiental, pausa y acotado.
- `tests/paleta.rs`: median cut determinista con cuatro cuadrantes (AC), parseo de colores y contraste.
- `tests/visuales.rs`: las seis visuales en tres tamaños × análisis real/ambiental/vacío (ASCII y braille), mini espectro de 12 barras y render completo del modo visual con `TestBackend` y vuelta a la vista anterior.
- `tests/recopilatorios.rs`: regresión R-10 con años distintos por fichero.

Comprobación de captura real (con mmmusic reproduciendo, o con una fuente PipeWire equivalente):

```bash
mpv --no-video --loop-file=inf --ao=pipewire --audio-client-name=mmusic-prueba tests/fixtures/prueba.flac
# en la aplicación, RUST_LOG=mmmusic=info muestra "nodo de audio encontrado" y el estado Capturando
```

Durante el desarrollo se validó en este equipo el enlace directo (`target.object` + `dont-remix`) contra una fuente real: `Capturando { tasa_hz: 48000, canales: 2 }` con el anillo recibiendo muestras, además del caso de fuente en pausa (`sin_audio` sin fallback) y la parada limpia del hilo con `q`.
