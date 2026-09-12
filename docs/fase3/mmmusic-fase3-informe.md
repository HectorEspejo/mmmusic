# mmmusic - Fase 3: Visuales Reactivas

## Especificación Funcional

**Versión:** 1.0
**Fecha:** 12 de septiembre de 2026
**Cliente:** Proyecto personal (Hector) — sin cliente externo

---

## 1. Visión General

La Fase 3 añade a mmmusic un modo visual: visualizaciones que reaccionan al audio en tiempo real, al estilo de las clásicas de Windows Media Player, renderizadas en la terminal con caracteres braille y bloques. El audio se captura de PipeWire, apuntando únicamente al nodo de salida de mmmusic, de modo que las visuales responden a lo que reproduce mmmusic y no a otras aplicaciones. Incluye un mini espectro permanente en la barra inferior, un modo a pantalla completa (sección `7`), paletas de color del tema de Omarchy o extraídas de la carátula, y un protector de pantalla opcional. Cierra además la corrección R-10 heredada de la Fase 2.

### Objetivos principales

1. Capturar el audio de mmmusic desde PipeWire sin decodificar dos veces y sin afectar a la reproducción.
2. Analizar el audio a 30 fps: espectro en bandas logarítmicas con suavizado y gravedad, forma de onda, niveles RMS/pico por canal y detección de graves/pulso.
3. Ofrecer seis visuales: Espectro, Barras y ondas, Ambiente, Partículas, Caleidoscopio y Túnel, más un mini espectro en la barra inferior.
4. Dos paletas conmutables: colores del tema activo de Omarchy o cinco colores dominantes de la carátula del álbum en reproducción.
5. Modo pantalla completa con la barra inferior visible, controles de visual y sensibilidad, y protector de pantalla por inactividad.
6. Degradar con elegancia: sin PipeWire las visuales entran en modo ambiental sin audio y avisan; en terminales lentas se baja el frame rate.

### Contexto

Fases 1 y 2 completadas y validadas (171/171). El usuario usa Alacritty (sin protocolo gráfico), fuentes Nerd Font y PipeWire (Omarchy). Referencia estética: visuales de Windows Media Player 9-11 (barras y ondas, ambiente, partículas, "batería", plenóptico) y vaporwave. No se copian diseños ni nombres comerciales: son reinterpretaciones propias.

Adaptación de la plantilla: sin cliente ni backend; "API Endpoints" pasa a ser atajos y API interna.

---

## 2. Arquitectura Técnica

### Stack tecnológico (cambios respecto a las fases anteriores)

| Capa | Tecnología | Motivo |
|------|-----------|--------|
| Captura de audio | crate `pipewire` (bindings a `libpipewire-0.3`) en hilo dedicado | Captura el PCM del nodo de mmmusic; sin decodificación paralela |
| Análisis | `realfft` (+ `rustfft`) | FFT real de 2048 muestras con ventana Hann |
| Render | `ratatui::widgets::canvas` con marcadores braille y bloques | Ya disponible; resolución 2×4 por celda con braille |
| Colores de carátula | `image` + cuantización propia (median cut) | Sin dependencia nueva; se calcula al cachear la carátula |
| mpv | opciones `ao=pipewire`, `audio-client-name=mmmusic` | El nodo de PipeWire se identifica por `application.name` / `node.name` |
| Dependencias de sistema (Arch) | `pipewire` (ya presente en Omarchy), `clang` para bindgen | `libpipewire-0.3.so` y cabeceras |

### Estructura de carpetas (añadidos)

```
mmmusic/
├── docs/
│   ├── mmmusic-fase3-informe.md
│   ├── mmmusic-fase3-checklist.md
│   ├── mmmusic-fase3-prompt.md
│   └── mmmusic-fase3-implementacion.md      (lo escribe el agente)
├── src/
│   ├── audio/
│   │   ├── mod.rs               # hilo de captura, EstadoCaptura, anillo de muestras
│   │   ├── pipewire.rs          # registro, localización del nodo, stream de captura
│   │   ├── anillo.rs            # buffer circular sin bloqueo (productor único / consumidor único)
│   │   └── analisis.rs          # FFT, bandas, onda, niveles, pulso, AGC (sin E/S, testeable)
│   ├── biblioteca/
│   │   ├── migraciones/
│   │   │   └── 003_colores_albumes.sql
│   │   ├── caratulas.rs         # + extracción de colores dominantes
│   │   └── recopilatorios.rs    # corrección R-10
│   ├── visuales/
│   │   ├── mod.rs               # trait Visual, registro, Paleta, ciclo de visuales
│   │   ├── paleta.rs            # paleta desde tema u colores de carátula, gradientes
│   │   ├── espectro.rs
│   │   ├── barras_ondas.rs
│   │   ├── ambiente.rs
│   │   ├── particulas.rs
│   │   ├── caleidoscopio.rs
│   │   ├── tunel.rs
│   │   └── mini_espectro.rs     # barra inferior
│   └── ui/
│       ├── vistas/visual.rs     # vista a pantalla completa (sección 7)
│       ├── barra_inferior.rs    # + mini espectro
│       └── inactividad.rs       # temporizador del protector de pantalla
└── tests/
    ├── analisis.rs              # bandas, onda, AGC, pulso con señales sintéticas
    ├── paleta.rs                # cuantización de colores con imágenes sintéticas
    └── visuales.rs              # cada visual dibuja sin pánico en varios tamaños
```

### Diagrama de arquitectura

```
┌────────────────────────────────────────────────────────────────────────────────┐
│                                Proceso mmmusic                                 │
│                                                                                │
│  Hilo Reproductor ──► libmpv (ao=pipewire, audio-client-name=mmmusic) ──┐      │
│                                                                         │ PCM  │
│  ┌──────────────────────────┐   anillo (lock-free)   ┌──────────────┐   ▼      │
│  │ Hilo Captura (pipewire)  │───────────────────────►│ Hilo UI      │  PipeWire│
│  │  - registro: busca nodo  │                        │  tick 33 ms  │  nodo    │
│  │    application.name=     │◄── watch<EstadoCaptura>│  en modo     │  "mmmusic"│
│  │    mmmusic               │                        │  visual      │    │     │
│  │  - stream captura F32 2ch│◄───────────────────────┼──────────────┼────┘     │
│  │  - reconexión al cambiar │      (target.object)   │  analisis()  │  (puertos│
│  │    el nodo (cada pista   │                        │  visual.dibujar()│  de    │
│  │    no; cada reinicio de  │                        │  Canvas braille│  salida)│
│  │    mpv sí)               │                        └──────────────┘          │
│  └──────────────────────────┘                                                   │
│                                                                                │
│  Sin PipeWire / sin nodo: EstadoCaptura::SinAudio → visuales en modo ambiental │
└────────────────────────────────────────────────────────────────────────────────┘
```

Principios:

- La captura nunca toca la reproducción: es un stream de captura independiente; si falla, mpv sigue sonando.
- El análisis (FFT) se ejecuta en el hilo de UI solo cuando hay algo que dibujar (modo visual o mini espectro visible), con presupuesto de 33 ms por frame.
- El buffer entre captura y UI es un anillo sin bloqueos de 8192 muestras por canal; el productor sobrescribe, el consumidor lee la ventana más reciente.

---

## 3. Modelo de Datos

### Cambios (migración 003)

**ALBUMES**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| colores | TEXT NULL | Cinco colores dominantes de la carátula en hex (`"#1a2b3c,#…"`), calculados al cachear la carátula; NULL si no hay carátula |

**AJUSTES** (claves nuevas): `visual_actual` (nombre de la visual), `paleta_fuente` (`tema` / `caratula`), `sensibilidad` (0.25–4.0, por defecto 1.0).

Los álbumes ya cacheados sin `colores` se completan en segundo plano al arrancar (una pasada por los que tengan `caratula_ruta` y `colores IS NULL`), sin reescaneo.

### Diagrama E-R (cambio)

```
┌──────────────────┐
│ ALBUMES (mod.)   │
│──────────────────│
│ + colores [F3]   │
└──────────────────┘
```

### Corrección R-10 (recopilatorios)

Al revertir un grupo "Varios artistas" a álbumes normales en un escaneo incremental, las pistas del grupo que no se han releído en ese escaneo se releen en ese momento (etiquetas completas) antes de reasignarlas, de modo que `anio`, artista y álbum salen del fichero y no del recopilatorio.

### Diagrama de estados: Captura

```
   arranque (visuales.activo=true)
        │
        ▼
   desconectada ── conectar PipeWire ──► buscando_nodo ── nodo "mmmusic" encontrado ──► capturando
        ▲                │ sin libpipewire /                │ 5 s sin nodo               │ nodo desaparece
        │                │ sin demonio                      ▼ (mpv sin audio activo)     │ (mpv reinicia)
        │                ▼                              sin_audio ◄──────────────────────┘
        │              error                                │ nodo aparece
        │                │ reintento cada 30 s              └────────────────► capturando
        └────────────────┘ (hasta 5 veces; después queda en error con aviso)

   sin_audio es normal cuando el reproductor está detenido: las visuales muestran el modo ambiental.
   Transiciones inválidas: capturando → buscando_nodo sin pasar por sin_audio; error → capturando directo.
```

### Diagrama de estados: Modo visual

```
   normal ──── `7` o `Enter` sobre "7 Visual" ────► pantalla_completa (manual)
   normal ──── N min sin entrada (autoinicio>0) ──► pantalla_completa (protector)
   pantalla_completa (manual) ──── `7` / `Esc` ───► normal (vuelve a la vista anterior)
   pantalla_completa (protector) ── cualquier tecla o clic (consumido) ──► normal
   En pantalla_completa siguen funcionando los atajos de reproducción y los de visual.
```

---

## 4. Flujos de Trabajo

### 4.1 Captura del nodo de mmmusic en PipeWire

```
  Hilo captura: pw_main_loop + contexto + core
        │
        ▼
  Registro: escucha eventos "global" de tipo Node
    ¿props application.name == "mmmusic" (o node.name == "mmmusic")? ──no──► seguir escuchando
        │sí (guarda object.serial)
        ▼
  Crear Stream de captura: formato F32LE, 2 canales, tasa del nodo (44.1k/48k)
    props: media.type=Audio, media.category=Capture, media.role=Music,
           target.object=<serial>, node.name="mmmusic-visuales", stream.dont-remix=true
        │
        ├─ conexión OK → estado=capturando; callback process(): copiar frames al anillo
        └─ fallo → intentar stream.capture.sink=true sobre el sink al que está enlazado el nodo
                   (monitor del dispositivo, filtrado no garantizado) → aviso "captura por monitor"
        │
        ▼
  Evento "global_remove" del nodo → estado=sin_audio; se destruye el stream y se vuelve a buscar.
  El nodo de mpv es estable durante la sesión (mpv no lo recrea por pista con gapless), así que la
  reconexión es rara.

  Camino de error: libpipewire no carga (dlopen) o el demonio no responde → estado=error,
  toast "Visuales sin audio: PipeWire no disponible", modo ambiental.
```

### 4.2 Frame de visual (30 fps)

```
  Tick 33 ms (solo en modo visual o con mini espectro visible)
        │
        ▼
  leer las últimas 2048 muestras por canal del anillo (si hay < 2048 nuevas se reutiliza la ventana anterior)
        │
        ▼
  analisis::procesar(): mezcla a mono → Hann → realfft 2048 → magnitudes →
    bandas logarítmicas (N = según ancho: 16..128) → dB → AGC lento (objetivo pico 0.85, ataque 50 ms,
    liberación 2 s) × sensibilidad → suavizado exponencial (subida inmediata, bajada con gravedad 0.9/frame)
    + onda (últimas `ancho×2` muestras, submuestreo) + rms/pico L/R + energía 20–150 Hz → pulso si
    energía > 1.4 × media móvil de 1 s (mínimo 250 ms entre pulsos)
        │
        ▼
  visual_actual.dibujar(Analisis, area, Paleta, dt) sobre un Canvas braille
        │
        ▼
  ¿el frame tardó > 33 ms tres veces seguidas? → fps=15 (toast una vez); vuelve a 30 tras 10 s de margen
  Sin audio (estado sin_audio/error) → Analisis::ambiental(t): ruido suave dependiente del tiempo
```

### 4.3 Cambio de pista y paleta de carátula

```
  EstadoReproduccion.pista_actual cambia (F1)
        │
        ▼
  ¿paleta_fuente == caratula? ──no──► paleta = tema (F1)
        │sí
        ▼
  ALBUMES.colores del album_id ──NULL──► calcular en hilo auxiliar desde caratula_ruta (median cut, 5 colores,
        │                                  ordenados por luminancia) → UPDATE ALBUMES.colores → evento
        ▼
  Paleta::desde_colores(): fondo = tema; primario/secundario/acento/resalte/tenue = los 5 colores;
  transición de 400 ms interpolando entre la paleta anterior y la nueva (evita el corte brusco)
  Sin carátula → paleta del tema y ♪ en el indicador de paleta
```

### 4.4 Protector de pantalla

```
  Cada evento de teclado/ratón reinicia `ultimo_input`.
  Tick: si autoinicio_min > 0, no hay diálogo abierto, el reproductor no está detenido
        y ahora - ultimo_input ≥ autoinicio_min → entrar en pantalla_completa(protector)
        con la visual `visual_actual`.
  Primera tecla/clic → salir y CONSUMIR el evento (no se ejecuta como atajo).
  Si el reproductor se detiene estando en modo protector → salir automáticamente.
```

### 4.5 Diagrama de secuencia: pulsar `7` con captura activa

```
  Usuario     Hilo UI                 Anillo/Captura        Visual (espectro)      Barra inferior
    │ `7`        │                          │                     │                     │
    │───────────►│ modo=pantalla_completa   │                     │                     │
    │            │ tick → 33 ms             │                     │                     │
    │            │──── leer ventana ───────►│                     │                     │
    │            │◄─── 2048×2 muestras ─────│                     │                     │
    │            │ analisis::procesar()     │                     │                     │
    │            │──── dibujar(Analisis, area, paleta) ──────────►│                     │
    │            │◄─── Canvas ────────────────────────────────────│                     │
    │            │──── mini_espectro (mismas bandas) ─────────────────────────────────►│
    │            │ frame.render()           │                     │                     │
```

---

## 5. Comandos, Atajos y API Interna

### 5.1 Atajos nuevos

| Tecla | Contexto | Acción |
|-------|----------|--------|
| `7` | Global | Entrar en el modo visual a pantalla completa; en él, salir |
| `Esc` | Modo visual (manual) | Salir al modo normal |
| `v` / `V` | Modo visual | Siguiente / anterior visual (Espectro → Barras y ondas → Ambiente → Partículas → Caleidoscopio → Túnel) |
| `[` / `]` | Modo visual | Sensibilidad ×0.8 / ×1.25 (acotada 0.25–4.0), con indicador temporal |
| `b` | Modo visual | Alternar paleta: tema ↔ carátula |
| `1`–`6` | Modo visual | Saltar a la visual n |
| Reproducción (`Espacio`, `n`, `p`, `,`, `.`, `<`, `>`, `+`, `-`, `m`, `s`, `r`, `L`) | Modo visual | Igual que en el resto de la app |
| Cualquier tecla / clic | Protector | Salir (evento consumido) |

La sidebar incorpora "7 Visual" como séptima sección. En modo visual manual, las teclas `1`–`6` cambian de visual, no de sección; `7` o `Esc` devuelven a la vista anterior.

### 5.2 API interna

`EstadoCaptura` (`watch`, hilo captura → UI): `Desconectada | BuscandoNodo | Capturando { tasa_hz, canales } | SinAudio | Error(String)`.

`Anillo` (`audio/anillo.rs`): `escribir(&[f32])` (productor), `ventana(n) -> [f32; n]` por canal (consumidor); capacidad 8192 por canal.

`Analisis` (`audio/analisis.rs`, sin E/S):

```
bandas: Vec<f32>      // 0..1, N según ancho
onda: Vec<f32>        // -1..1, longitud = ancho×2
rms: [f32; 2], pico: [f32; 2]
graves: f32           // 0..1 energía 20–150 Hz
pulso: bool           // beat detectado en este frame
tasa_hz: u32
ambiental: bool       // true si no hay audio y los valores son sintéticos
```

`Visual` (trait, `visuales/mod.rs`):

```
fn nombre(&self) -> &'static str
fn reiniciar(&mut self)                          // al cambiar de visual o de tamaño
fn dibujar(&mut self, a: &Analisis, area: Rect, p: &Paleta, dt: f32, ctx: &mut canvas::Context)
```

`Paleta` (`visuales/paleta.rs`): `fondo, primario, secundario, acento, resalte, tenue: Color`; `gradiente(t) -> Color`; `desde_tema(&PaletaTema)`; `desde_colores(&[Rgb; 5], fondo)`; `interpolar(&Paleta, &Paleta, t)`.

`ComandoCaptura` (UI → hilo captura): `Iniciar`, `Detener`, `Apagar`.

Consultas nuevas: `albumes::colores(album_id)`, `albumes::fijar_colores(album_id, &str)`, `albumes::sin_colores(limite)`, `ajustes` para `visual_actual`, `paleta_fuente`, `sensibilidad`.

### 5.3 Configuración

```toml
[visuales]
activo = true              # false: sin hilo de captura ni mini espectro
fps = 30                   # 15..60
predeterminada = "espectro"
paleta = "tema"            # "tema" | "caratula"
mini_espectro = true
autoinicio_min = 0         # 0 = sin protector de pantalla
nodo = "mmmusic"           # application.name del nodo a capturar
```

---

## 6. Interfaz de Usuario

### Mapa de navegación (cambios)

```
mmmusic
├── [7] Visual (pantalla completa)
│     ├── 1 Espectro · 2 Barras y ondas · 3 Ambiente · 4 Partículas · 5 Caleidoscopio · 6 Túnel
│     ├── v/V ciclar · [/] sensibilidad · b paleta · 7/Esc salir
│     └── Barra inferior visible (título, controles, progreso, mini espectro)
└── Barra inferior (todas las vistas): + mini espectro de 12 barras a la izquierda de los controles
```

### Mockup 1: Espectro (pantalla completa, 80×24)

```
┌ mmmusic · Visual 1/6 Espectro · paleta: carátula · sens 1.0 ──────────────────┐
│                                                                               │
│                 ▂                                                             │
│                ▂█▂       ▂                                                    │
│               ▃███▃     ▃█▃          ▂                                        │
│              ▅█████▅   ▅███▅        ▃█▃                                       │
│             ▆███████▆ ▆█████▆      ▅███▅      ▂                               │
│            ████████████████████   ▆█████▆    ▃█▃     ▂                        │
│           ██████████████████████ ████████▆  ▅███▅   ▃█▃    ▂                  │
│          ████████████████████████████████████████████████▃▅█▃▂  ▂    ▂        │
│         ██████████████████████████████████████████████████████▃▅█▃▂▃█▃▂      │
│        ██████████████████████████████████████████████████████████████████▃   │
│  ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔▔ ▔▔▔ │
│  20 Hz                                                                 16 kHz │
│                                                                               │
│ v/V visual · [ ] sensibilidad · b paleta · 7 salir                            │
├───────────────────────────────────────────────────────────────────────────────┤
│ ▀▀▀ ♥ Nocturne Drive           ▂▄▆█▅▃▂▁▂▃▂▁  ⏮  ⏸  ⏭   🔀 🔁 ♪ vol 80 %  ↑    │
│ ▄▄▄ Midnight Premiere · Late Night Tapes  1:27 ━━━━━━━●───────────── 4:12    │
└───────────────────────────────────────────────────────────────────────────────┘
```

Las barras usan el gradiente de la paleta de abajo (primario) a arriba (acento) y una marca de pico (`▔`) que cae con gravedad. Con anchura par se ofrece variante espejada (config `espejo = true` dentro de la visual, no en Fase 3: solo la normal).

### Mockup 2: Barras y ondas

```
┌ mmmusic · Visual 2/6 Barras y ondas ──────────────────────────────────────────┐
│      ⢀⡠⠤⠤⢄⡀              ⢀⡠⠔⠒⠒⠢⢄                ⡠⠔⠒⠢⢄⡀                        │
│ ⠤⠤⠤⠤⠊    ⠈⠑⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠊      ⠈⠒⠤⠤⠤⠤⠤⠤⠤⠤⠔⠊      ⠈⠒⠤⠤⠤⠤⠤⠤⠤⠤⠤⠤⠤⠤⠤⠤⠤⠤⠤⠤    │
│                                                                               │
│ ▁▂▃▅▇█▇▅▃▂▁ ▁▂▃▅▇█▇▅▃▂▁ ▁▂▃▅▇█▇▅▃▂▁ ▁▂▃▅▇█▇▅▃▂▁ ▁▂▃▅▇█▇▅▃▂▁ ▁▂▃▅▇█▇▅▃▂▁ ▁▂▃ │
│ ███████████ ███████████ ███████████ ███████████ ███████████ ███████████ ███ │
│ ███████████ ███████████ ███████████ ███████████ ███████████ ███████████ ███ │
│ L ████████████████▏        R ██████████████▏                                 │
```

Mitad superior: osciloscopio en braille (onda). Mitad inferior: 8–16 barras anchas con reflejo tenue debajo y medidores L/R con pico.

### Mockup 3: Ambiente / Partículas / Caleidoscopio / Túnel (descripción)

```
Ambiente:      3–5 "manchas" de color (elipses braille rellenas con densidad variable) que derivan
               lentamente; su tamaño sigue el RMS y su color el gradiente; el pulso las hace latir.
Partículas:    emisor central; cada pulso lanza 20–60 partículas con velocidad ∝ graves; las agudas
               añaden brillo (carácter más denso); vida 1–2 s; estela con caracteres decrecientes.
Caleidoscopio: 6 sectores simétricos; en cada uno, polilíneas braille cuyos radios son las bandas;
               rotación lenta constante + giro extra en cada pulso; colores por banda.
Túnel:         anillos concéntricos que avanzan hacia el espectador; el radio de cada anillo se
               modula por las bandas (anillo = tiempo, ángulo = frecuencia); la velocidad sigue el RMS.
Todas:         modo ambiental sin audio = misma geometría con valores sintéticos suaves.
```

### Mockup 4: Mini espectro en la barra inferior (modo normal)

```
├──────────────────┴──────────────────────────────────────────────────────────┤
│ ▀▀▀ Nocturne Drive           ▂▄▆█▅▃▂▁▂▃▂▁  ⏮  ⏸  ⏭      🔀 🔁 ♪ vol 80 %  ↑ │
│ ▄▄▄ Midnight Premiere · Late Night Tapes  1:27 ━━━━━━━●───────────── 4:12  │
```

12 barras de una fila con caracteres `▁▂▃▄▅▆▇█`; se oculta en modo compacto (< 70 columnas) y con `mini_espectro = false`.

### Notas de UX

- Cabecera del modo visual: nombre y número de la visual, fuente de paleta y sensibilidad; desaparece tras 3 s sin pulsar teclas y reaparece al pulsar `v`, `V`, `[`, `]` o `b`.
- Estado de captura en la cabecera cuando no es `capturando`: "sin audio", "PipeWire no disponible", "captura por monitor".
- Todas las visuales funcionan desde 40×12; por debajo se muestra solo el espectro.
- Modo `iconos = "ascii"`: las barras usan `#` con altura por filas y la onda usa `-`/`_`/`~`; braille se mantiene (es Unicode básico, no Nerd Font).
- Contraste: las visuales dibujan sobre el fondo del tema; ningún elemento informativo depende solo del color.
- El mini espectro no debe parpadear: se dibuja con las mismas bandas suavizadas que la visual, submuestreadas a 12.

---

## 7. Lógica de Negocio

### 7.1 Bandas logarítmicas y suavizado

```python
import math

def limites_bandas(n_bandas, f_min=20.0, f_max=16000.0):
    """Fronteras de frecuencia espaciadas logarítmicamente."""
    r = (f_max / f_min) ** (1.0 / n_bandas)
    return [f_min * r ** i for i in range(n_bandas + 1)]

def bandas_desde_magnitudes(mag, tasa_hz, n_fft, n_bandas):
    lim = limites_bandas(n_bandas)
    salida = []
    for i in range(n_bandas):
        k0 = max(1, int(lim[i] * n_fft / tasa_hz))
        k1 = max(k0 + 1, int(lim[i + 1] * n_fft / tasa_hz))
        v = max(mag[k0:k1])                       # pico del intervalo (más vivo que la media)
        db = 20 * math.log10(v + 1e-9)
        salida.append(min(1.0, max(0.0, (db + 60) / 60)))   # -60 dB..0 dB → 0..1
    return salida

def suavizar(prev, nuevo, gravedad=0.9):
    """Subida inmediata, bajada con gravedad."""
    return [max(n, p * gravedad) for p, n in zip(prev, nuevo)]

def agc(ganancia, pico_actual, objetivo=0.85, ataque=0.3, liberacion=0.01):
    if pico_actual * ganancia > objetivo:
        return ganancia * (1 - ataque) + (objetivo / max(pico_actual, 1e-6)) * ataque
    return ganancia * (1 - liberacion) + (objetivo / max(pico_actual, 1e-6)) * liberacion
```

La ganancia del AGC se acota a [0.5, 8.0] y se multiplica por `sensibilidad`.

### 7.2 Detección de pulso

```python
def pulso(energia_graves, historial, umbral=1.4, minimo_ms=250, desde_ultimo_ms=0):
    media = sum(historial) / max(len(historial), 1)      # ventana de 1 s (30 valores)
    return energia_graves > umbral * media and desde_ultimo_ms >= minimo_ms
```

### 7.3 Colores dominantes de la carátula (median cut)

- Entrada: miniatura 300×300 (caché de F1) reducida a 64×64.
- Se descartan píxeles casi negros (L < 8 %) y casi blancos (L > 95 %) si quedan al menos 20 % de píxeles; si no, se conservan.
- Median cut hasta 8 cajas; se toman los 5 colores más poblados; se ordenan por luminancia y se garantiza contraste mínimo 2:1 entre `acento` y el fondo del tema (si no, se aclara/oscurece el acento).
- Resultado en hex, guardado en `ALBUMES.colores`. Determinista (mismo fichero → mismo resultado), con test.

### 7.4 Reglas de rendimiento

- FFT solo si hay consumidor (modo visual o mini espectro visible); si no, el hilo de captura sigue escribiendo el anillo pero no se analiza.
- Número de bandas = `clamp(ancho_celdas / 2, 16, 128)`; se recalcula al redimensionar.
- Partículas: máximo 400 vivas; se recortan las más antiguas.
- Presupuesto: un frame completo (análisis + dibujo) < 10 ms en un i7 moderno a 200×50; si tres frames seguidos superan 33 ms, fps → 15.

### 7.5 Casos especiales

- Reproductor en pausa: la captura sigue conectada pero no llegan muestras; el análisis decae con la gravedad hasta 0 y luego se activa el modo ambiental tras 2 s.
- Volumen de mpv al 0 / silencio: mpv sigue emitiendo el PCM atenuado; el AGC lo compensará hasta el tope de ganancia (×8); por encima, las visuales se ven bajas, lo cual es correcto.
- Dos instancias de mmmusic: cada una captura su propio nodo (`application.name` es igual; se elige el nodo cuyo `application.process.id` coincide con el PID propio; si no se puede, el más reciente).
- Cambio de dispositivo de salida (auriculares Bluetooth): el nodo de mpv se mueve; el stream de captura enlazado por `target.object` sigue al nodo. Si se recibe `global_remove`, se reconecta.

---

## 8. Requisitos No Funcionales

### Seguridad

- Sin red nueva. El hilo de captura solo lee audio; no expone nada por D-Bus ni fichero.
- Solo se captura el nodo de mmmusic; el fallback por monitor del sink se anuncia explícitamente en la cabecera para que el usuario sepa que puede estar viendo audio de otras apps.

### Protección de datos

- No se almacena audio. `ALBUMES.colores` deriva de ficheros locales del usuario.

### Backups y recuperación

- Migración 003 aditiva. `colores` es regenerable (se recalcula si es NULL).

### Rendimiento

- CPU en modo visual < 8 % de un núcleo a 30 fps en 200×50; en modo normal con mini espectro < 2 %.
- Latencia audio→pantalla < 80 ms (anillo + frame).
- Memoria adicional < 5 MB.

### Accesibilidad

- El modo visual es opcional (`activo = false` lo elimina por completo, incluido el mini espectro).
- Sin dependencia de Nerd Font: braille y bloques son Unicode estándar; modo `ascii` disponible.
- El protector de pantalla se sale con cualquier tecla, consumida, para no ejecutar acciones accidentales.
- Advertencia en README: las visuales con pulsos rápidos pueden ser molestas para personas con fotosensibilidad; `fps = 15` y "Ambiente" como visual suave.

---

## 9. Integraciones

| Integración | Detalle |
|-------------|---------|
| PipeWire | `libpipewire-0.3` vía crate `pipewire`; stream de captura F32 estéreo enlazado por `target.object` al nodo con `application.name = "mmmusic"`; fallback `stream.capture.sink` |
| mpv | Opciones nuevas: `ao=pipewire`, `audio-client-name=mmmusic` (además de las de F1). Si `ao=pipewire` falla (sistema sin PipeWire), mpv cae a su selección automática y las visuales quedan en `error` con aviso |
| Tema de Omarchy (F1) | La paleta "tema" se deriva de la paleta existente; se recarga en caliente igual que el resto |
| Carátulas (F1) | `colores` se calcula en el mismo hilo auxiliar de decodificación |

---

## 10. Decisiones Técnicas (ADR-lite)

**DT-25 — Captura desde PipeWire del nodo propio frente a decodificación paralela.** Da el audio real, sincronizado y sin duplicar CPU; el coste es una dependencia de sistema (`libpipewire`) que Omarchy ya tiene. La decodificación paralela con symphonia se descartó por deriva de sincronía en seeks y por doblar el trabajo. Si algún día se necesita portar a sistemas sin PipeWire, se añadirá como fuente alternativa detrás del mismo `Anillo`.

**DT-26 — Análisis en el hilo de UI bajo demanda frente a hilo de análisis propio.** El análisis cuesta < 1 ms por frame; un hilo más añadiría sincronización sin beneficio. Solo se ejecuta cuando hay algo visible.

**DT-27 — Anillo sin bloqueos productor/consumidor frente a `Mutex<VecDeque>`.** El callback `process` de PipeWire corre en un hilo de tiempo real y no debe bloquearse nunca; un anillo SPSC con índices atómicos garantiza latencia y evita xruns.

**DT-28 — Canvas braille de ratatui frente a protocolo gráfico.** Funciona en Alacritty y en cualquier terminal Unicode; 2×4 puntos por celda dan resolución suficiente para ondas y partículas. Con Kitty se podría pintar en píxeles, pero el usuario usa Alacritty y el estilo "terminal" es parte del encanto.

**DT-29 — Colores de carátula persistidos en ALBUMES frente a calcular en cada cambio de pista.** La cuantización cuesta ~20 ms y ocurre en cambios de pista; guardarla evita repetirla y permite usar la paleta desde el primer frame. Es regenerable.

**DT-30 — Las visuales son homenajes, no copias.** Se reinterpretan los conceptos clásicos (barras, ondas, ambiente, partículas, caleidoscopio, túnel) con algoritmos propios y nombres genéricos en castellano; no se reproducen diseños, nombres ni recursos de productos existentes.

**DT-31 — `1`–`6` en modo visual cambian de visual.** Reutilizar los números para la visual activa es más útil que cambiar de sección desde una pantalla que oculta la sidebar; `7`/`Esc` es la salida única.

---

## 11. Plan de Desarrollo

| Sprint | Contenido | Entregable verificable |
|--------|-----------|------------------------|
| S1 — Captura y análisis | Migración 003 y corrección R-10; opciones mpv `ao=pipewire` / `audio-client-name`; hilo de captura con localización del nodo, stream, anillo, estados y fallback; `analisis.rs` con tests sintéticos (seno de 1 kHz → banda correcta, silencio → 0, pulso) | `EstadoCaptura::Capturando` en el log y un test de análisis verde |
| S2 — Espectro, mini espectro y modo visual | Trait `Visual`, `Paleta` desde tema, vista a pantalla completa con cabecera, atajos `7`/`Esc`/`v`/`V`/`[`/`]`, mini espectro en la barra, degradación de fps, modo ambiental | Espectro funcionando a 30 fps con audio real |
| S3 — Resto de visuales y paleta de carátula | Barras y ondas, Ambiente, Partículas, Caleidoscopio, Túnel; median cut y `colores`; `b`; transición de paleta; pasada de relleno de `colores` | Las seis visuales con ambas paletas |
| S4 — Protector, pulido y cierre | Protector de pantalla, cabecera autoocultable, modo ascii, tamaños mínimos, tests de visuales, README con aviso de fotosensibilidad, clippy/fmt | Suite verde y documentación |

**Estimación:** 4 sprints de ~1 semana → 3–4 semanas nominales. Supuestos: los de F1/F2; el crate `pipewire` compila con la versión de PipeWire de Arch (riesgo R-11: si el enlace directo al nodo de mpv no funciona, el fallback por monitor cubre la funcionalidad con menos aislamiento); el agente puede probar PipeWire en su entorno solo si existe un demonio (si no, las pruebas de captura las hace Hector).

---

## 12. Conexiones con Otras Fases

- **Desde F1/F2:** paleta del tema, caché de carátulas e hilo auxiliar de imágenes, barra inferior, `EstadoReproduccion`, recopilatorios (R-10).
- **Hacia F4 (letras y ecualizador):** el ecualizador (filtros `af` de mpv) se refleja directamente en la captura, así que las visuales mostrarán el resultado ecualizado; posible vista combinada letras + visual.
- **Hacia F5 (radio / URL):** las visuales funcionan igual (capturan el nodo, no el fichero).
- **Ideas para más adelante:** variante espejada del espectro, "sunset grid" vaporwave, carátula pulsante, exportar un frame a PNG/SVG, mini visual en Waybar vía fichero.
