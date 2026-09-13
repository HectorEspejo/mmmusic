# Prompt mmmusic - Fase 5: Radio y Streams

Continúa desarrollando **mmmusic** (reproductor TUI para Omarchy; Fases 1-3 cerradas). Stack: el existente (Rust 2024, ratatui, libmpv2, rusqlite, ureq, clap, pipewire), sin crates nuevos. Tablas: (1) **EMISORAS** con nombre, nombre_norm, url única, pagina_web, pais, etiquetas, codec, bitrate_kbps, logo_url, logo_ruta, radiobrowser_uuid único, favorita, anadida_en, ultima_reproduccion, ultimo_error; (2) **EMISORA_TITULOS** con emisora_id, titulo, visto_en (50 por emisora); (3) **BUSQUEDAS_RADIO** con clave, respuesta_json, obtenido_en (24 h); (4) **COLA**, **HISTORIAL_REPRODUCCION** y **ENVIOS** recreadas con `pista_id` NULL admisible y `emisora_id`, más `tipo` en COLA y `titulo_icy` en HISTORIAL; (5) **AJUSTES** añade radiobrowser_servidor y radio_pestana. Migración `004_radio.sql` en transacción con copia previa `mmmusic.db.pre-004`. Funcionalidades: `ElementoCola { Pista | Emisora }` en toda la cola; streams con mpv (`cache=yes`, `demuxer-max-bytes=32MiB`, `stream-lavf-o=reconnect…`, `loadlist` para PLS/M3U remotos como espejos), estados conectando/almacenando/en_directo/reconectando(n)/rendido desde `paused-for-cache`, `cache-buffering-state` y `end-file`, reconexión 1-2-4-8-16-30 s en 6 intentos avanzando de espejo; títulos ICY desde `metadata` parseados "Artista - Título" con filtros de ruido, EMISORA_TITULOS podado a 50, historial solo con artista y scrobble (now playing + envío) tras 30 s en directo con `album` = emisora y sin duración; gestión de emisoras (`N`, `R`, `D`, `L` favorita, `i` importar PLS/M3U, `e` exportar favoritas M3U8); Radio Browser en un hilo de directorio con `red/cliente.rs` compartido (User-Agent identificativo, espejos por DNS de `all.api.radio-browser.info` guardados 24 h, búsqueda por nombre/país/etiqueta con `hidebroken`, caché local, click una vez por día) y logos descargados solo como `image/*` ≤ 512 KB, 5 s, 3 redirecciones, cacheados con el pipeline de carátulas. Interfaz: sección "8 Radio" con pestañas Favoritas, Todas, Buscar y Sonando (`[`/`]`), `f` busca el título ICY en la biblioteca, barra inferior en modo directo (estado, título ICY, "EN DIRECTO · tiempo · codec", logo), `◉` en la cola, MPRIS sin `length` y `CanSeek=false`, modo ascii. Config `[radio]`. Tests sin red: ICY, PLS/M3U, reconexión, migración con datos, cola mixta, historial/envíos ICY, caché.

---

## Instrucciones para el agente

1. Lee primero `CLAUDE.md` en la raíz del repositorio: contiene las reglas
   permanentes de trabajo (idioma, commits, effort, cierre de fase).
2. Sigue el checklist `mmmusic-fase5-checklist.md` como definición
   del alcance. No añadas funcionalidades fuera de él sin indicarlo.
3. Al finalizar el desarrollo o al pausar la sesión, marca el checklist y
   genera/actualiza el archivo **`mmmusic-fase5-implementacion.md`**
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
