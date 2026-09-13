# Prompt mmmusic - Fase 4: Ecualizador y Letras

Continúa desarrollando **mmmusic** (reproductor TUI para Omarchy; Fases 1, 2, 3 y 5 cerradas). Stack: el existente más el crate **`dns-lookup`** (sustituye al resolutor DNS propio de la Fase 5, misma interfaz en `radio/espejos.rs`); sin red nueva. Tablas: (1) **PRESETS_EQ** con nombre, nombre_norm único, ganancias (10 valores dB, 31 Hz-16 kHz), preamp_db, integrado, creado_en; (2) **LETRAS** con pista_id PK (cascada), offset_ms, fuente_preferida, actualizado_en; (3) **AJUSTES** añade eq_activo, eq_preset_id, eq_ganancias, eq_preamp_db, eq_limitador, replaygain_modo, replaygain_preamp_db y letras_superpuestas, sembradas por código. Migración `005_ecualizador_letras.sql`; nueve presets integrados sembrados al arrancar. Funcionalidades: ecualizador de 10 bandas ISO sobre la cadena `af` de mpv con etiquetas `@pre` (volume), `@eq1`…`@eq10` (`equalizer`, Q 1.41) y `@lim` (`alimiter` a −1 dBFS), instalada una vez y ajustada con `af-command` (fallback a reconstrucción), bypass con `af=""` cuando todo está plano, detección de lavfi, ganancias −12..12 en pasos de 0,5, preamp, limitador opcional, ReplayGain nativo (`replaygain` no/track/album, preamp, sin clip, fallback 0), persistencia en AJUSTES; overlay `E` con barras, valores, presets integrados y propios (`N` guardar, `D` borrar, `Enter` aplicar), atajos `h`/`l`/`j`/`k`/`J`/`K`/`0`/`R`/`Tab`/`e`/`x`/`g`, indicadores EQ/RG en la barra inferior; letras locales con trait `FuenteLetras` (fichero `.lrc`/`.txt` junto a la pista o en `letras.carpeta` con nombre `<artista> - <titulo>`, y etiquetas USLT/LYRICS vía lofty), parser LRC sin E/S (timestamps múltiples, metadatos, offset, enhanced sin karaoke, BOM/CRLF, Latin-1 de respaldo), sincronía por búsqueda binaria; vista `9 Letras` con seguimiento centrado, estado manual 5 s, salto con `Enter`, offset por pista con `(`/`)` guardado en LETRAS, cambio de fuente con `s`, letra estática, mensaje sin letra con rutas buscadas y no aplicable para emisoras; superposición de letras en el modo visual con `9`. Config `[ecualizador]` y `[letras]`; `CLAUDE.md` pasa a `docs/faseN/`. Tests: cadena y comandos, parser y sincronía, resolución de fuentes con fixtures.

---

## Instrucciones para el agente

1. Lee primero `CLAUDE.md` en la raíz del repositorio: contiene las reglas
   permanentes de trabajo (idioma, commits, effort, cierre de fase).
2. Sigue el checklist `mmmusic-fase4-checklist.md` como definición
   del alcance. No añadas funcionalidades fuera de él sin indicarlo.
3. Al finalizar el desarrollo o al pausar la sesión, marca el checklist y
   genera/actualiza el archivo **`mmmusic-fase4-implementacion.md`**
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
