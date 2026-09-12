# mmmusic - Checklist Fase 3: Visuales Reactivas

## Migración y corrección heredada
- [x] Migración `003_colores_albumes.sql`: columna `colores` en ALBUMES y claves `visual_actual`, `paleta_fuente` y `sensibilidad` en AJUSTES
- [x] Corrección R-10: al revertir un grupo "Varios artistas" en escaneo incremental se releen las etiquetas de las pistas no releídas antes de reasignarlas
  - AC: Dado un recopilatorio agrupado, cuando se etiqueta `albumartist` en una sola pista y se reescanea incrementalmente, entonces todas las pistas del grupo vuelven a álbumes normales con el `anio` de su fichero
- [x] Pasada en segundo plano al arrancar que calcula `colores` para los álbumes con carátula y `colores IS NULL`

## Captura de audio (PipeWire)
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

## Análisis
- [x] `analisis.rs` sin E/S: mezcla a mono, ventana Hann, `realfft` de 2048 muestras y magnitudes
- [x] Bandas logarítmicas 20 Hz–16 kHz, número `clamp(ancho/2, 16, 128)` recalculado al redimensionar, pico por intervalo y escala −60..0 dB → 0..1
- [x] AGC lento (objetivo 0.85, ataque 50 ms, liberación 2 s, ganancia acotada 0.5–8) multiplicado por `sensibilidad`
- [x] Suavizado con subida inmediata y bajada con gravedad 0.9 por frame, y marca de pico que cae
- [x] Onda de `ancho×2` muestras submuestreadas, RMS y pico por canal, energía de graves 20–150 Hz
- [x] Detección de pulso: graves > 1.4 × media móvil de 1 s con mínimo 250 ms entre pulsos
- [x] Modo ambiental: valores sintéticos suaves dependientes del tiempo cuando no hay audio (sin_audio, error o 2 s de silencio en pausa)
- [x] Tests con señales sintéticas: seno de 1 kHz cae en la banda correcta, silencio da 0, pulso periódico se detecta, AGC converge
  - AC: Dado un seno de 1 kHz a −6 dB, cuando se analiza, entonces la banda con máximo contiene 1 kHz y ninguna otra supera el 30 % de ese valor

## Modo visual (pantalla completa)
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

## Visuales
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

## Paleta
- [x] `Paleta` (fondo, primario, secundario, acento, resalte, tenue) con `gradiente(t)` e `interpolar`
- [x] Paleta desde el tema de Omarchy, recargada en caliente con el tema (F1)
- [x] Extracción de 5 colores dominantes por median cut sobre 64×64, descartando casi negros y casi blancos si queda ≥ 20 % de píxeles, ordenados por luminancia, con contraste mínimo 2:1 del acento sobre el fondo
  - AC: Dada una imagen sintética con cuatro cuadrantes de colores puros, cuando se cuantiza, entonces los cuatro aparecen entre los colores devueltos y el resultado es idéntico en dos ejecuciones
- [x] `colores` calculado en el hilo auxiliar de imágenes al cachear la carátula y guardado en ALBUMES
- [x] `b` alterna la fuente de paleta tema ↔ carátula; sin carátula se usa la del tema con indicador ♪
- [x] Transición de 400 ms entre paletas al cambiar de pista o de fuente

## Mini espectro
- [x] 12 barras de una fila con `▁▂▃▄▅▆▇█` en la barra inferior, a la izquierda de los controles, con las mismas bandas suavizadas submuestreadas
- [x] Oculto en modo compacto (< 70 columnas), con `mini_espectro = false` o `visuales.activo = false`

## Protector de pantalla
- [x] Temporizador de inactividad que entra en modo visual tras `autoinicio_min` minutos sin teclado ni ratón, solo si no hay diálogo abierto y el reproductor no está detenido
- [x] Cualquier tecla o clic sale del protector y se consume sin ejecutar su acción
- [x] Salida automática del protector si el reproductor se detiene

## Configuración y entrega
- [x] Sección `[visuales]` en config: activo, fps (15–60), predeterminada, paleta, mini_espectro, autoinicio_min, nodo; validada y acotada
- [x] `visuales.activo = false` elimina el hilo de captura, el mini espectro y la sección 7
- [x] `config.ejemplo.toml` y overlay de ayuda actualizados con los atajos del modo visual
- [x] README: dependencias `pipewire` y `clang`, configuración de visuales, fallback por monitor y aviso de fotosensibilidad
- [x] `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios

---

**Progreso Fase 3:** 59 / 59 funcionalidades

**Total mmmusic (Fases 1-3):** 230 / 230 funcionalidades
