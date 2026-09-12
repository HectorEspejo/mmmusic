# CLAUDE.md — mmmusic

## Contexto

mmmusic es un reproductor de música para terminal (TUI) para Omarchy con layout
tipo Spotify (sidebar, contenido, barra inferior de "sonando ahora") y atajos
vim. Cliente: proyecto personal de Hector (4d3).
Stack: Rust 2024, ratatui + crossterm, libmpv (`libmpv2`), rusqlite `bundled`,
lofty, ratatui-image, mpris-server (tokio solo en el hilo MPRIS), serde + toml,
notify, tracing.

## Documentación del proyecto

La documentación funcional vive en `docs/`:

- `mmmusic-maestro.md` — visión global, mapa de fases y estado del proyecto
- `mmmusic-fase1-informe.md` — especificación funcional completa de la fase
- `mmmusic-fase1-checklist.md` — alcance verificable de la fase
- `mmmusic-fase1-implementacion.md` — informe de implementación (lo escribes tú)

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
- Dependencias de sistema en Arch: `mpv`, `pkgconf`. Comandos: `cargo run`,
  `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`. Antes de
  cerrar una sesión los cuatro deben pasar.
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
