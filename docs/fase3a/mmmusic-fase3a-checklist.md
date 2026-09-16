# mmmusic - Checklist Fase 3a: Visual "Ciudad"

## Geometría y raster
- [x] `proyeccion.rs` sin E/S: `Camara::orbita(t, rms, dims, vuelta_s)` con radio 0,9 × máximo(ancho, profundidad), altura 0,55 × ancho + balanceo 0,08 × ancho × RMS, objetivo al centro; `proyectar` en perspectiva (FOV 55°) devolviendo `None` tras el plano cercano; `cara_visible` por normal · vista
- [x] `raster.rs` sin E/S: raster de puntos 2×4 por celda con `limpiar`, `punto`, `linea` (Bresenham, punteada un punto de cada dos), `borrar_poligono` (scanline) y `volcar` al `Canvas` como `Points` agrupados por color, con color de celda = último punto dibujado
- [x] El raster se reserva una vez por tamaño y se reutiliza sin asignaciones por frame
- [x] Tests: una cara que mira a la cámara es visible y su opuesta no; un polígono borrado elimina los puntos previos de su interior; una línea punteada tiene la mitad de puntos que la continua
  - AC: Dados dos edificios alineados con la cámara, cuando se dibuja el frame, entonces no queda ningún punto del edificio trasero dentro de la silueta del delantero

## Visual Ciudad
- [x] `ciudad.rs` implementa `Visual` con nombre "Ciudad": historia de F filas (`clamp(alto/4, 8, 16)`, config `ciudad_filas`) de N bandas (`clamp(ancho/6, 8, 24)`), una fila nueva cada 2 frames, fila más reciente en z = 0
- [x] Edificios como prismas de huella 0,7 × 0,7 y altura `banda × 0,8 × ancho_ciudad`, con altura mínima 0,05
- [x] Dibujo de lejos a cerca: borrado de las caras visibles y después aristas punteadas, con aristas compartidas dibujadas una vez y sin base
- [x] Color de cada arista por altura con `paleta.gradiente` (tenue en la base, acento en la cima)
- [x] Cielo de estrellas: una cada 60 celdas, posiciones estables por semilla del tamaño, parpadeo por ruido lento (2-5 s) en tenue, dibujadas antes que los edificios
- [x] Cámara en órbita continua con una vuelta cada `ciudad_vuelta_s` (5-120, por defecto 20)
- [x] Modo ambiental: historia alimentada por `Analisis::ambiental`, ciudad baja y ondulante con la órbita en marcha
- [x] Funciona en 40×12 (N = 8, F = 8) y con `reiniciar()` al redimensionar; entra en el presupuesto de frame de F3 (degradación a 15 fps intacta)
- [x] Modo `ascii`: volcado del raster a `.` y `:` por celda; `ciudad_punteado = false` dibuja aristas continuas

## Integración
- [x] Ciudad registrada como séptima visual: `v`/`V` la incluyen tras Túnel y `0` la selecciona en modo visual (`7` sigue siendo salir); cabecera "Visual 7/7 Ciudad"
- [x] `AJUSTES.visual_actual` admite `ciudad` y `visuales.predeterminada = "ciudad"` es válido
- [x] Claves `ciudad_vuelta_s`, `ciudad_filas` y `ciudad_punteado` en `[visuales]`, validadas y en `config.ejemplo.toml`
- [x] Ayuda del modo visual y README actualizados con la visual y la tecla `0`
- [x] Tests de render: Ciudad dibuja sin pánico en 40×12, 80×24 y 200×50 con análisis real, ambiental y vacío, en braille y ascii
- [x] `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios

---

**Progreso Fase 3a:** 19 / 19 funcionalidades

**Total mmmusic (Fases 1-5, incl. 3a y 4a):** 359 / 374 funcionalidades
