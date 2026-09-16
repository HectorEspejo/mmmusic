# Prompt mmmusic - Fase 3a: Visual "Ciudad"

Continúa desarrollando **mmmusic** (reproductor TUI para Omarchy) con una subfase de la Fase 3: una séptima visual llamada **Ciudad**, un espectro 3D visto desde un helicóptero en órbita. Stack: el existente (`ratatui` canvas braille, `Analisis`, `Paleta`, trait `Visual`), sin crates nuevos ni migraciones. Tablas: ninguna; `AJUSTES.visual_actual` admite `ciudad`. Funcionalidades: `proyeccion.rs` sin E/S (cámara en órbita con radio 0,9 × máximo(ancho, profundidad), altura 0,55 × ancho más balanceo 0,08 × ancho × RMS, una vuelta cada `ciudad_vuelta_s`, proyección en perspectiva con FOV 55° y backface culling); `raster.rs` sin E/S (raster de puntos 2×4 por celda con `limpiar`, `punto`, `linea` Bresenham punteada un punto de cada dos, `borrar_poligono` por scanline y `volcar` al `Canvas` como `Points` por color, color de celda = último punto, reservado una vez por tamaño); `ciudad.rs` (historia de F = clamp(alto/4, 8, 16) filas de N = clamp(ancho/6, 8, 24) bandas, una fila nueva cada 2 frames con la más reciente en z = 0; prismas de huella 0,7 y altura banda × 0,8 × ancho con mínimo 0,05; dibujo de lejos a cerca borrando caras visibles y trazando aristas punteadas sin base ni duplicados; color por altura tenue → acento; cielo de estrellas estables por semilla del tamaño con parpadeo lento en tenue, dibujadas antes; modo ambiental; 40×12 con N = 8 y F = 8; modo ascii con `.` y `:`; `ciudad_punteado`). Integración: séptima visual tras Túnel en `v`/`V`, tecla `0` en modo visual (`7` sigue siendo salir), cabecera "7/7 Ciudad", config `ciudad_vuelta_s` (5-120, 20), `ciudad_filas` (8-16, 12) y `ciudad_punteado`, ayuda y README. Tests: culling, borrado de polígonos, línea punteada, ocultación entre dos edificios alineados y render en tres tamaños con análisis real, ambiental y vacío.

---

## Instrucciones para el agente

1. Lee primero `CLAUDE.md` en la raíz del repositorio: contiene las reglas
   permanentes de trabajo (idioma, commits, effort, cierre de fase).
2. Sigue el checklist `mmmusic-fase3a-checklist.md` como definición
   del alcance. No añadas funcionalidades fuera de él sin indicarlo.
3. Al finalizar el desarrollo o al pausar la sesión, marca el checklist y
   genera/actualiza el archivo **`mmmusic-fase3a-implementacion.md`**
   (informe de implementación) con exactamente esta estructura:
   - Resumen de lo implementado
   - Desviaciones respecto a la especificación (qué y por qué)
   - Estructura de archivos creada/modificada
   - Decisiones técnicas tomadas durante el desarrollo
   - Funcionalidades del checklist completadas (copiando su texto exacto)
   - Pendientes y bloqueos
   - Ejecución y pruebas (cómo arrancar, migrar y testear)
4. Actualiza el informe de implementación en cada sesión, no lo
   regeneres desde cero.
5. Cuando el informe esté listo, avisa al desarrollador de que debe
   entregárselo al analista funcional: hasta entonces la documentación de
   proyecto (checklist maestro, informe maestro) no refleja la realidad.
