# Prompt mmmusic - Fase 3: Visuales Reactivas

Continúa desarrollando **mmmusic** (reproductor TUI para Omarchy; Fases 1-2 cerradas: Rust 2024, ratatui, libmpv2, rusqlite, lofty, ratatui-image, mpris-server, ureq, clap). Stack nuevo: **crate `pipewire` (libpipewire-0.3) en un hilo de captura, `realfft` para el análisis, `canvas` braille de ratatui para el render; mpv pasa a `ao=pipewire` y `audio-client-name=mmmusic`**. Tablas: (1) **ALBUMES** añade `colores` (5 colores hex dominantes de la carátula, median cut, regenerable); (2) **AJUSTES** añade visual_actual, paleta_fuente y sensibilidad. Migración `003_colores_albumes.sql`. Corrección R-10: al revertir un recopilatorio en escaneo incremental, releer las etiquetas de las pistas no releídas. Funcionalidades: hilo de captura que localiza en el registro de PipeWire el nodo con `application.name = mmmusic` (PID propio), abre un stream F32 estéreo con `target.object` y fallback a `stream.capture.sink` ("captura por monitor"), estados desconectada/buscando_nodo/capturando/sin_audio/error con reconexión y reintentos, anillo SPSC sin bloqueos de 8192 muestras por canal; `analisis.rs` sin E/S (Hann, FFT 2048, bandas logarítmicas 20 Hz-16 kHz con `clamp(ancho/2,16,128)`, −60..0 dB, AGC lento acotado 0.5-8 × sensibilidad, gravedad 0.9, onda, RMS/pico L/R, graves, pulso > 1.4 × media de 1 s, modo ambiental sintético); trait `Visual` con seis visuales (Espectro, Barras y ondas, Ambiente, Partículas ≤ 400, Caleidoscopio de 6 sectores, Túnel); `Paleta` desde tema u colores de carátula con gradiente e interpolación de 400 ms; mini espectro de 12 barras en la barra inferior; modo visual a pantalla completa (sección 7) con cabecera autoocultable, `7`/`Esc`, `v`/`V`, `1`-`6`, `[`/`]`, `b`, atajos de reproducción activos, tick 33 ms solo cuando hay visual, degradación a 15 fps; protector de pantalla por `autoinicio_min` cuya primera tecla se consume; config `[visuales]`; modo ascii; tamaño mínimo 40×12. Tests sin PipeWire: análisis con señales sintéticas, median cut determinista y cada visual en tres tamaños.

---

## Instrucciones para el agente

1. Lee primero `CLAUDE.md` en la raíz del repositorio: contiene las reglas
   permanentes de trabajo (idioma, commits, effort, cierre de fase).
2. Sigue el checklist `mmmusic-fase3-checklist.md` como definición
   del alcance. No añadas funcionalidades fuera de él sin indicarlo.
3. Al finalizar el desarrollo o al pausar la sesión, marca el checklist y
   genera/actualiza el archivo **`mmmusic-fase3-implementacion.md`**
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
