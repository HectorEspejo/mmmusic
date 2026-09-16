# Prompt mmmusic - Fase 4a: Identidad Visual

Continúa desarrollando **mmmusic** (reproductor TUI para Omarchy) con una subfase corta de identidad visual, independiente de la Fase 4. Stack: el existente, sin crates nuevos ni migraciones. Tablas: ninguna. Funcionalidades: módulo `src/marca.rs` con los logos como constantes (`LOGO_COMPACTO` de 2 filas y 33 columnas, `LOGO_MMM` con solo las tres emes de 17 columnas, `LOGO_GRANDE` de 6 filas y 62 columnas generado con la fuente figlet `ansi_shadow`, variantes `_ASCII` con `#` y `=`, `ESLOGAN` "reproductor para tu tty", `ONDA` `▁▂▃▅▆▇█▇▆▅▃▂` y `ONDA_ASCII` `._-~^~-_.`), funciones `version()`, `linea_reposo(ancho, ascii)` (recorta por etapas y centra) y `onda(ancho, desplazamiento, ascii)`; test de anchura uniforme por fila con `unicode-width`; cabecera de la sidebar con `LOGO_MMM` en acento (o "♪" colapsada) sustituyendo al título del borde; overlay de ayuda con `LOGO_COMPACTO` centrado y versión (o `LOGO_MMM` bajo 37 columnas); tarjeta de reposo en la barra inferior cuando no hay elemento cargado (fila 1 marca · versión · ▶ eslogan, fila 2 onda desplazándose un carácter por tick de 250 ms, controles intactos, solo fila 1 en modo compacto), volviendo al formato normal en cuanto hay elemento; `--version` con `♪ mmmusic <versión>` y `--version --logo` con el logo grande; README con `LOGO_GRANDE` en un bloque de código; `.desktop` con `Comment=Reproductor de música para tu terminal`. Los logos usan solo bloques Unicode y tienen variante ascii; la marca nunca sustituye información de estado.

---

## Instrucciones para el agente

1. Lee primero `CLAUDE.md` en la raíz del repositorio: contiene las reglas
   permanentes de trabajo (idioma, commits, effort, cierre de fase).
2. Sigue el checklist `mmmusic-fase4a-checklist.md` como definición
   del alcance. No añadas funcionalidades fuera de él sin indicarlo.
3. Al finalizar el desarrollo o al pausar la sesión, marca el checklist y
   genera/actualiza el archivo **`mmmusic-fase4a-implementacion.md`**
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
