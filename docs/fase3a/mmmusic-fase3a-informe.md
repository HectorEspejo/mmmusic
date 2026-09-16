# mmmusic - Fase 3a: Visual "Ciudad" (espectro 3D en órbita)

## Especificación Funcional

**Versión:** 1.0
**Fecha:** 16 de septiembre de 2026
**Cliente:** Proyecto personal (Hector) — sin cliente externo

---

## 1. Visión General

Subfase de la Fase 3 que añade una séptima visual, **Ciudad**: el espectro se convierte en una ciudad de rascacielos dibujados con aristas punteadas, en perspectiva, con eliminación de líneas ocultas, sobre un cielo de estrellas, y una cámara que orbita alrededor como un helicóptero. Cada banda del espectro es una columna de edificios y cada frame reciente una fila hacia el fondo, de modo que la ciudad muestra la historia del último segundo de audio. Referencia estética: gráficos vectoriales punteados de ordenador de 8 bits sobre fondo azul (imagen aportada por el usuario); reinterpretación propia, sin copiar recursos.

### Objetivos principales

1. Proyección 3D en perspectiva de una rejilla bandas × tiempo de prismas, con cámara en órbita a altura fija y una vuelta cada `ciudad_vuelta_s` segundos.
2. Dibujo de aristas punteadas con eliminación de líneas ocultas (algoritmo del pintor sobre un raster braille propio).
3. Color por altura, de tenue en la base a acento en las cimas; estrellas parpadeantes en tenue.
4. Selección con `0` en modo visual y con `v`/`V` en el ciclo; funciona en 40×12 y en modo ambiental.

### Contexto

Fases 1-5 completadas (la 4a especificada, sin informe). La infraestructura de la Fase 3 (captura, `Analisis`, trait `Visual`, `Paleta`, modo visual) se reutiliza tal cual; solo se añaden módulos de geometría y raster.

---

## 2. Arquitectura Técnica

Sin stack nuevo (`ratatui` canvas, `Analisis`, `Paleta`). Añadidos:

```
src/visuales/
├── ciudad.rs        # Visual "Ciudad": historia de bandas, cámara, orden de dibujo, color por altura, estrellas
├── proyeccion.rs    # cámara en órbita, proyección en perspectiva, backface culling (sin E/S, testeable)
└── raster.rs        # raster braille propio: puntos 2×4 por celda, borrado de polígonos, líneas punteadas,
                     # capa de color por celda; volcado al Canvas como Points por color (sin E/S, testeable)
tests/ciudad.rs      # proyección, culling, hidden-line y render en tres tamaños
docs/fase3a/mmmusic-fase3a-{informe,checklist,prompt,implementacion}.md
```

### Diagrama de flujo de un frame

```
  Analisis (F3) ──► historia.push(bandas)  (anillo de F filas; F = clamp(alto/4, 8, 16))
        │
        ▼
  columnas N = clamp(ancho/6, 8, 24) bandas (submuestreo de Analisis.bandas)
  edificio(i, j): base (x_i, z_j), huella 0,7 × 0,7 celdas de mundo, altura h = bandas_j[i] × alto_max
        │
        ▼
  cámara: θ = 2π · t / ciudad_vuelta_s; posición = centro + (R·cos θ, H + balanceo(rms), R·sin θ); mira al centro
        │
        ▼
  raster.limpiar(); estrellas.dibujar(t) en tenue
        │
        ▼
  para cada edificio ordenado por distancia a la cámara, de lejos a cerca:
      proyectar 8 vértices → caras visibles (normal · vista > 0)
      para cada cara visible: raster.borrar_poligono(cara)  ← tapa lo que hay detrás
      para cada arista de cara visible: raster.linea_punteada(a, b, color_por_altura)
        │
        ▼
  raster.volcar(ctx): Points agrupados por color → Canvas braille
```

---

## 3. Modelo de Datos

Sin cambios. `AJUSTES.visual_actual` admite el valor `ciudad`. Configuración nueva en `[visuales]`: `ciudad_vuelta_s` (5–120, por defecto 20), `ciudad_filas` (8–16, por defecto 12), `ciudad_punteado` (true).

Sin entidades con ciclo de vida.

---

## 4. Flujos de Trabajo

### 4.1 Historia del espectro

- Cada frame (30 fps) se añade la fila actual de bandas al anillo de F filas; la fila más reciente ocupa el borde z = 0 y las anteriores se alejan (z = 1..F−1).
- A 30 fps con F = 12 la ciudad muestra ~0,4 s; para que el "flujo" sea visible se añade una fila cada 2 frames (15 filas/s ≈ 0,8 s de historia con F = 12).
- En modo ambiental la historia se alimenta con `Analisis::ambiental`, que da una ciudad baja y ondulante.

### 4.2 Cámara

```python
def camara(t, rms, ancho_ciudad, profundidad, vuelta_s):
    R = 0.9 * max(ancho_ciudad, profundidad)          # radio de órbita
    H = 0.55 * ancho_ciudad + 0.08 * ancho_ciudad * rms  # altura fija + balanceo suave por RMS
    theta = 2 * pi * (t % vuelta_s) / vuelta_s
    ojo = (cx + R * cos(theta), H, cz + R * sin(theta))
    return mirar_a(ojo, objetivo=(cx, 0.15 * ancho_ciudad, cz), arriba=(0, 1, 0))
```

Proyección en perspectiva con campo de visión 55°; relación de aspecto de los puntos braille ≈ 1:1 (celda 2×4 sobre celdas de terminal ~1:2), por lo que no hace falta corrección adicional.

### 4.3 Diagrama de secuencia: cambio a Ciudad

```
  Usuario   Hilo UI                     ciudad.rs                raster.rs        Canvas
    │ `0`      │ visual_actual = ciudad    │                        │               │
    │─────────►│ reiniciar() ─────────────►│ historia vacía, θ=0    │               │
    │          │ tick 33 ms: dibujar(a) ──►│ push, cámara, orden    │               │
    │          │                           │──── borrar/linea ─────►│               │
    │          │                           │◄──── volcar(ctx) ──────│──── Points ──►│
```

---

## 5. Comandos, Atajos y API Interna

| Tecla | Contexto | Acción |
|-------|----------|--------|
| `0` | Modo visual | Seleccionar la visual Ciudad (séptima; `7` sigue siendo salir) |
| `v` / `V` | Modo visual | El ciclo incluye Ciudad tras Túnel |

`proyeccion.rs`: `Camara::orbita(t, rms, dims, vuelta_s)`, `proyectar(&Camara, punto3) -> Option<punto2>` (None si está detrás del plano cercano), `cara_visible(&Camara, cara) -> bool`.

`raster.rs`: `Raster::nuevo(ancho_celdas, alto_celdas)`, `limpiar()`, `punto(x, y, color)`, `linea(a, b, color, punteada: bool)` (Bresenham; punteada = un punto de cada dos), `borrar_poligono(&[punto2])` (scanline sobre el raster de puntos), `volcar(&mut canvas::Context)` (agrupa puntos por color y emite `Points`). Color por celda: gana el último punto dibujado (el más cercano, por el orden del pintor).

`ciudad.rs`: implementa `Visual` (`nombre() = "Ciudad"`, `reiniciar`, `dibujar`); `Historia` (anillo de F filas de N bandas), `Estrellas` (posiciones fijas por semilla del tamaño, parpadeo por ruido lento, ~1 estrella cada 60 celdas).

---

## 6. Interfaz de Usuario

### Mockup (80×24, aproximación en ASCII de lo que dibuja el raster braille)

```
┌ mmmusic · Visual 7/7 Ciudad · paleta: tema · sens 1.0 ────────────────────────┐
│   .          .                    .                      .            .       │
│                        .                     .                  .             │
│              . .                  ......                                      │
│         .          .            .:      :.        .                  .        │
│                  ......        .:  ......  :.                                 │
│                .:      :.     :  .:      :. :         ......                  │
│      ......   :  ......  :    : :          : :      .:      :.       .        │
│    .:      :. : :      : :    : :          : :     :  ......  :               │
│   :  ......  :: :      : :    : :          : :     : :      : :               │
│   : :      : :: :      : :    : :          : :     : :      : :    ......     │
│   : :      : :: :      : :    : :          : :     : :      : :  .:      :.   │
│   : :      : :: :      : :    : :          : :     : :      : : :  ......  :  │
│   : :      : :: :      : :    : :          : :     : :      : : : :      : :  │
│   : :      : : :.    .: :     : :          : :     : :      : : : :      : :  │
│   : :      : :   ::::   :      :.        .: :      : :      : : : :      : :  │
│    :.    .:  :.        .:        ::::::::   :      :.    .:  :  :.    .:  :   │
│      ::::      ::::::::                     :        ::::    :    ::::    :   │
│                                              :.              .:           :   │
│                                                ::::::::::::::             :   │
│ v/V visual · 0 ciudad · [ ] sensibilidad · b paleta · 7 salir                 │
├───────────────────────────────────────────────────────────────────────────────┤
│ ▀▀▀ Nocturne Drive           ▂▄▆█▅▃▂▁▂▃▂▁  ⏮  ⏸  ⏭   🔀 🔁 ♪ vol 80 %  EQ ↑   │
│ ▄▄▄ Midnight Premiere · Late Night Tapes  1:27 ━━━━━━━●───────────── 4:12     │
└───────────────────────────────────────────────────────────────────────────────┘
```

En braille real las aristas son líneas de puntos alternos; los edificios de delante ocultan a los de detrás; las cimas van en acento y las bases en tenue; las estrellas parpadean en tenue y desaparecen tras los edificios.

### Notas de UX

- Cabecera del modo visual: "Visual 7/7 Ciudad". Ayuda: `0 ciudad` en la línea de atajos.
- Modo `ascii`: mismas líneas con `.` y `:` en lugar de braille (el raster vuelca a caracteres en vez de a puntos), sin eliminación de líneas ocultas fina (se aproxima a celda).
- Con `ciudad_punteado = false` las aristas son continuas.
- Tamaño mínimo 40×12: N = 8, F = 8.
- Sin audio (ambiental): ciudad baja y ondulante, órbita continúa.

---

## 7. Lógica de Negocio

### 7.1 Orden del pintor y ocultación

```python
def dibujar_ciudad(raster, camara, edificios, paleta):
    raster.limpiar()
    dibujar_estrellas(raster, paleta.tenue)
    for e in sorted(edificios, key=lambda e: -distancia(camara.ojo, e.centro)):   # lejos → cerca
        caras = [c for c in e.caras() if cara_visible(camara, c)]                   # normal · vista > 0
        for c in caras:
            poly = [proyectar(camara, v) for v in c.vertices]
            if None in poly: continue
            raster.borrar_poligono(poly)                     # tapa lo que quedó detrás
        for c in caras:
            for a, b in c.aristas():
                pa, pb = proyectar(camara, a), proyectar(camara, b)
                if pa and pb:
                    color = paleta.gradiente(max(a.y, b.y) / e.alto_max)   # tenue → acento por altura
                    raster.linea(pa, pb, color, punteada=True)
```

- Un prisma tiene 6 caras; con la cámara siempre por encima del suelo se ven como mucho 3 (tapa y dos laterales).
- Las aristas compartidas por dos caras visibles se dibujan una vez (deduplicación por par de vértices).
- La base de los edificios no se dibuja (queda en el suelo, sin suelo dibujado).

### 7.2 Altura y suavizado

- `h = bandas_fila[i] × alto_max`, con `alto_max = 0,8 × ancho_ciudad`; las bandas ya llegan suavizadas y con gravedad desde `Analisis`, así que las filas antiguas conservan la forma que tenían.
- Altura mínima 0,05 × alto_max para que la ciudad no desaparezca en silencios.

### 7.3 Estrellas

- Número = celdas / 60; posiciones pseudoaleatorias con semilla derivada del tamaño de la terminal (estables entre frames, cambian al redimensionar).
- Brillo = ruido lento por estrella (periodo 2–5 s); se dibuja un punto cuando el brillo supera 0,5 (parpadeo); siempre en tenue.
- Las estrellas se dibujan antes que los edificios: quedan tapadas por ellos.

### 7.4 Rendimiento

- N × F ≤ 24 × 16 = 384 prismas, ≤ 3 caras visibles, ≤ 9 aristas cada uno → < 3.500 líneas y < 1.200 polígonos por frame; en un raster de 400 × 200 puntos entra holgadamente en el presupuesto de 10 ms de F3.
- El raster se reserva una vez por tamaño y se reutiliza (sin asignaciones por frame).

---

## 8. Requisitos No Funcionales

- Sin red, sin datos nuevos. Rendimiento dentro del presupuesto de F3; se aplica la misma degradación a 15 fps.
- Accesibilidad: la altura se codifica por geometría y por color; en monocromo sigue siendo legible; visual sin destellos (el parpadeo de estrellas es lento y tenue), apta como visual "suave".
- La visual es diseño propio: geometría y estilo punteado son técnicas genéricas; no se reproducen recursos de la imagen de referencia.

---

## 9. Integraciones

Ninguna nueva. Reutiliza `Analisis`, `Paleta`, el modo visual y la degradación de fps de F3.

---

## 10. Decisiones Técnicas (ADR-lite)

**DT-62 — Raster braille propio con algoritmo del pintor frente al `Canvas` aditivo de ratatui.** El `Canvas` solo añade puntos y no permite borrar, imprescindible para ocultar líneas; el raster propio borra polígonos y al final vuelca los puntos supervivientes al `Canvas` como `Points` por color. Se mantiene el trait `Visual` sin cambios.

**DT-63 — Rejilla bandas × tiempo frente a una sola fila.** Una sola fila deja media pantalla vacía y la órbita aburre; la historia convierte la visual en una ciudad que fluye y aprovecha la perspectiva.

**DT-64 — Órbita a altura fija con balanceo por RMS.** Es la "vista de helicóptero" pedida; el balanceo aporta vida sin marear; zoom por pulso queda como opción futura.

**DT-65 — Tecla `0` para la séptima visual.** `7` es la salida del modo visual; `0` queda libre y es coherente con el teclado numérico.

---

## 11. Plan de Desarrollo

| Sprint | Contenido | Entregable verificable |
|--------|-----------|------------------------|
| S1 (único) | `proyeccion.rs` y `raster.rs` con tests; `ciudad.rs` (historia, cámara, orden, color, estrellas); registro como séptima visual, `0`, ayuda, config, modo ascii; tests de render en tres tamaños | Ciudad orbitando con audio real |

**Estimación:** 1 sprint, 1 sesión.

---

## 12. Conexiones con Otras Fases

- **Desde F3:** todo; `raster.rs` puede servir a futuras visuales con ocultación (por ejemplo, un "sunset grid" con montañas).
- **Con F4 (letras superpuestas):** compatible; la banda de letras se dibuja encima como en cualquier visual.
- **Ideas:** zoom por pulso, suelo en perspectiva, exportar un frame de la ciudad como PNG (idea ya en el roadmap).
