# CLAUDE.md — mmmusic

## Contexto

mmmusic es un reproductor de música para terminal (TUI) para Omarchy con layout
tipo Spotify (sidebar, contenido, barra inferior de "sonando ahora") y atajos
vim. Cliente: proyecto personal de Hector (4d3).
Stack: Rust 2024, ratatui + crossterm, libmpv (`libmpv2`), rusqlite `bundled`,
lofty, ratatui-image, mpris-server (tokio solo en el hilo MPRIS), serde + toml,
notify, tracing, ureq (rustls) en el hilo de scrobbling, serde_json, md5, clap,
pipewire (libpipewire-0.3) en el hilo de captura, realfft, canvas braille de ratatui,
dns-lookup.

## Documentación del proyecto

La documentación funcional vive en `docs/faseN/` (una carpeta por fase; el
maestro en `docs/`):

- `docs/mmmusic-maestro.md` — visión global, mapa de fases y estado del proyecto
- `docs/faseN/mmmusic-faseN-{informe,checklist,prompt,implementacion}.md` — documentos de cada fase
- `mmmusic-fase1-informe.md` — especificación funcional completa de la fase
- `mmmusic-fase1-checklist.md` — alcance verificable de la fase
- `mmmusic-fase1-implementacion.md` — informe de implementación de la fase 1 (cerrada)
- `mmmusic-fase2-informe.md` — especificación funcional completa de la fase 2
- `mmmusic-fase2-checklist.md` — alcance verificable de la fase 2
- `mmmusic-fase2-implementacion.md` — informe de implementación de la fase 2 (cerrada)
- `mmmusic-fase3-informe.md` — especificación funcional completa de la fase 3
- `mmmusic-fase3-checklist.md` — alcance verificable de la fase 3
- `mmmusic-fase3-implementacion.md` — informe de implementación de la fase 3 (cerrada)
- `mmmusic-fase5-informe.md` — especificación funcional completa de la fase 5 (la fase 4 no está especificada; no existe)
- `mmmusic-fase5-checklist.md` — alcance verificable de la fase 5
- `mmmusic-fase5-implementacion.md` — informe de implementación de la fase 5 (cerrada)
- `mmmusic-fase4-informe.md` — especificación funcional completa de la fase 4
- `mmmusic-fase4-checklist.md` — alcance verificable de la fase 4
- `mmmusic-fase4-implementacion.md` — informe de implementación (lo escribes tú)

El informe de fase manda sobre tu criterio: si algo te parece incorrecto o
incompleto, no lo cambies por tu cuenta — impleméntalo como está o párate y
coméntalo con el desarrollador.

## Reglas de trabajo

- Escribe siempre en castellano: respuestas, comentarios de código, mensajes de
  commit, documentación y textos de interfaz. Solo se mantienen en inglés los
  términos técnicos estándar (API, CRUD, endpoint, commit...).
- Tu effort por defecto es `ultracode`: una vez aprobado el plan que presentes,
  utiliza los workflows y todo lo disponible en ese effort.
- No añadas funcionalidades que no estén en el checklist de la fase. Si detectas
  algo necesario que falta, propónlo antes de implementarlo y déjalo anotado
  como desviación.
- Nomenclatura: tablas SQL en `MAYUSCULAS_SNAKE_CASE`, campos en
  `minusculas_snake_case`. Identificadores de Rust (módulos, funciones,
  variables, variantes de enum) en castellano y `snake_case` / `CamelCase`
  según la convención de Rust; nombres de crates y términos técnicos en inglés.
- Sin `unwrap()` fuera de tests; errores con `anyhow` en binario y `thiserror`
  en módulos. Todo `Result` de E/S se registra con `tracing`.
- La UI nunca bloquea: escaneo, decodificación de imágenes y D-Bus van en hilos
  propios y comunican por canales (`AppEvento`, `ComandoReproductor`,
  `watch<EstadoReproduccion>`). Solo el hilo reproductor escribe el estado de
  reproducción.
- SQLite: modo WAL, `busy_timeout` 5 s, claves foráneas activadas, sin
  `CHECK` en enumeraciones (se validan en código). Migraciones en
  `src/biblioteca/migraciones/NNN_nombre.sql` aplicadas por `PRAGMA
  user_version`, nunca destructivas dentro de una fase.
- mpv se inicializa siempre con `vo=null`, `audio-display=no`, `ytdl=no`,
  `load-scripts=no`, `config=no`, `gapless-audio=yes`.
- Dependencias de sistema en Arch: `mpv`, `pkgconf`, `pipewire` y `clang`
  (bindgen del crate `pipewire`); nunca `libchafa` (`ratatui-image` va sin la
  feature `chafa`). Comandos: `cargo run`,
  `cargo test`, `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --check`. Antes de cerrar una sesión los cuatro deben pasar.
  Instalación: `instalar.sh` (cargo install + `mmmusic.desktop`).
- El crate es librería + binario: todo módulo nuevo se expone en `src/lib.rs`;
  `src/main.rs` solo arranca.
- Cada hilo abre su propia conexión SQLite y aplica migraciones al abrirla.
- `Picker` y protocolos de `ratatui-image` solo en el hilo de UI; los hilos
  auxiliares entregan `DynamicImage` por canal.
- Pruebas manuales aisladas: exporta `XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
  `XDG_CACHE_HOME` y `XDG_STATE_HOME` a un directorio temporal para no tocar
  el perfil real.
- Red: todas las peticiones del código de mmmusic pasan por `red/cliente.rs`
  (`ureq` rustls, timeout, User-Agent, `url_permitida()`), desde el hilo de
  scrobbling o el hilo de directorio. Hosts permitidos: `api.listenbrainz.org`,
  `ws.audioscrobbler.com` y los espejos de `all.api.radio-browser.info`. Única
  excepción: logos de emisora (cualquier https, solo `image/*`, ≤ 512 KB, 5 s,
  3 redirecciones, no confiables). Los streams de audio los abre mpv, nunca
  `ureq`. Los tests nunca tocan la red.
- Cola, historial y envíos son mixtos (`ElementoCola`): exactamente uno de
  `pista_id` / `emisora_id` por fila, validado en código. Una "escucha" es una
  canción reconocida, nunca una conexión.
- Recrear tablas en migraciones (SQLite no permite quitar `NOT NULL`): en
  transacción, `INSERT … SELECT`, y copia previa `mmmusic.db.pre-NNN` (tras
  `wal_checkpoint(TRUNCATE)`) borrada en el siguiente arranque correcto.
  Toda migración corre con `foreign_keys = OFF` y termina con
  `PRAGMA foreign_key_check` antes de confirmar.
- Ningún módulo construye su propio cliente `ureq`: todas las peticiones pasan
  por `red/cliente.rs`, que aplica la allowlist. No implementes protocolos de
  red a mano (DNS, HTTP…) cuando exista un crate pequeño y auditado.
- Filtros de audio de mpv: cadena etiquetada (`@pre`, `@eqN`, `@lim`) instalada
  una vez y ajustada con `af-command`; reconstruir solo cuando no hay comando
  en caliente. `af = ""` cuando todo está plano.
- mmmusic nunca escribe en la biblioteca del usuario (audio, `.lrc`); offsets,
  colores y presets van a la base de datos o a la carpeta de datos.
- Fuentes de datos externas (letras y similares) se implementan como traits
  enchufables con resolución por orden y caché; la vista no conoce la fuente.
- Nombre del proyecto y de sus ficheros: `mmmusic` con tres emes
  (`mmmusic.db`, `mmmusic.log`, `mmmusic.db.pre-NNN`). Revisa que no se cuele
  `mmusic`.
- Secretos: viven solo en `~/.config/mmmusic/credenciales.toml` (permisos 600).
  Nunca los escribas en logs, toasts, `error_msg`, tests, fixtures ni en el
  informe de implementación.
- Playlists, cola, historial, favoritas y envíos referencian siempre PISTAS
  (por id), nunca ALBUMES ni ARTISTAS: los álbumes pueden reasignarse en un
  reescaneo.
- Releer ficheros tras una migración se hace con la bandera
  `AJUSTES.reescaneo_completo_pendiente`, nunca dentro del SQL de la migración.
- Todo texto hacia logs, toasts o `error_msg` pasa por `Credenciales::redactar()`.
  Los clientes HTTP comprueban `url_permitida()` antes de cada petición.
- Entidades virtuales de la UI (como "♥ Favoritas") usan ids centinela
  negativos; nunca se insertan en la base.
- Reintentos y clasificación de respuestas van en `scrobbling/planificador.rs`
  (sin E/S); reutilízalo para cualquier integración nueva.
- Subcomandos actuales: `reescanear [--completo]`, `probar-servicios`,
  `autorizar-lastfm`. Códigos de salida: 0 OK, 1 configuración, 2 red.
- Audio para visuales: solo capturando el nodo propio de mmmusic en PipeWire
  (`audio-client-name=mmmusic`); nunca decodificar en paralelo. Los callbacks
  de tiempo real (PipeWire `process`, wakeup de libmpv) no bloquean ni asignan.
- Trabajo por frame (FFT, dibujo) solo cuando hay algo visible; el tick de 33 ms
  se activa bajo demanda y el tick base sigue en 250 ms.
- Las visuales son reinterpretaciones propias con nombres genéricos en
  castellano; no copies diseños, nombres ni recursos de otros reproductores.
- Tests sin PipeWire: el análisis y las visuales se prueban con señales
  sintéticas; la captura real la valida el desarrollador en Omarchy.
- PipeWire: el nodo de libmpv se llama "mpv" y no lleva PID; el nombre de
  `audio-client-name` y el PID están en el global `Client`. Localiza nodos
  propios por `client.id`. Un enlace en `Paused` NO es un fallo.
- Estructuras compartidas con hilos de tiempo real: atómicos por elemento sin
  `unsafe`; callbacks con `try_borrow_mut` tolerantes a reentrada; comandos y
  plazos atendidos por un timer del propio bucle (valor > 0).
- Valores por defecto que dependen de la configuración se siembran por código
  en AJUSTES, nunca como constantes en el SQL de una migración.
- Fixtures de audio en `tests/fixtures/` (ficheros diminutos de los seis
  formatos, generados una vez con ffmpeg y versionados).

## Al terminar una fase (o al pausar la sesión)

1. Busca el archivo markdown que tenga `checklist` en el nombre correspondiente
   a esa fase y marca `[x]` todo lo que se haya realizado. Actualiza los
   contadores de progreso del final del archivo. No reescribas el texto de las
   funcionalidades: es la clave de trazabilidad con el resto de documentos.
2. Genera o actualiza `mmmusic-fase[N]-implementacion.md` con exactamente
   esta estructura:
   - Resumen de lo implementado
   - Desviaciones respecto a la especificación (qué y por qué)
   - Estructura de archivos creada/modificada
   - Decisiones técnicas tomadas durante el desarrollo
   - Funcionalidades del checklist completadas (copiando su texto exacto)
   - Pendientes y bloqueos
   - Ejecución y pruebas (cómo arrancar, migrar y testear)

   Es un documento vivo: actualízalo de forma incremental en cada sesión, nunca
   lo regeneres desde cero. Sé honesto en las desviaciones y en los pendientes;
   ese informe es lo único que el analista funcional verá de lo que realmente
   pasó, y con él se actualiza la especificación.
3. Recuérdale al desarrollador que debe entregar el informe de implementación al
   analista funcional antes de especificar la siguiente fase.
4. Invita al desarrollador a hacer PR. Si te dice que lo hagas, usa el comando
   `gh`.

## Commits

- NUNCA firmes ni te atribuyas los commits: la autoría es del usuario.
- Sin `Co-Authored-By`, sin líneas de "Generated with", sin menciones a Claude
  en el mensaje.
- Mensajes en castellano, en imperativo y concisos.
