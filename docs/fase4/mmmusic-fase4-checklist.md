# mmmusic - Checklist Fase 4: Ecualizador y Letras

## Migración, deuda y documentación
- [x] Migración `005_ecualizador_letras.sql`: tablas PRESETS_EQ (nombre_norm único) y LETRAS (pista_id PK con cascada)
- [x] Claves `eq_activo`, `eq_preset_id`, `eq_ganancias`, `eq_preamp_db`, `eq_limitador`, `replaygain_modo`, `replaygain_preamp_db` y `letras_superpuestas` en AJUSTES sembradas por código desde `[ecualizador]` y `[letras]`
- [x] Presets integrados (Plano, Rock, Pop, Electrónica, Hip-hop, Vocal, Bass boost, Treble boost, Loudness) con los valores de §7.2, sembrados al arrancar si faltan, con `integrado = 1`
- [x] R-20: `radio/espejos.rs` reescrito sobre `dns-lookup` (`lookup_host` + `lookup_addr`) con la misma interfaz pública, eliminando el codificador/decodificador DNS propio y sus tests
- [x] `CLAUDE.md` actualizado a `docs/faseN/` como ubicación de la documentación

## Motor del ecualizador
- [x] `cadena.rs` sin E/S: `construir` (etiquetas `@pre`, `@eq1`…`@eq10`, `@lim`; `equalizer` con `width_type=q`, `width=1.41`; `volume` en dB; `alimiter` a 0.891), `comando_banda`, `comando_preamp` y `es_bypass`
- [x] Detección de lavfi al arrancar; sin lavfi, `EstadoEq.disponible = false` y el overlay explica el motivo
- [x] Instalación de la cadena en mpv (`af`) al activar o al primer cambio; `af = ""` cuando `es_bypass` es verdadero
- [x] Cambios de banda y preamp aplicados con `af-command` sin reconstruir la cadena; fallback a reconstrucción completa si el comando falla
  - AC: Dada una pista sonando con EQ activo, cuando se sube una banda 3 dB, entonces el cambio se oye en menos de 50 ms sin corte de audio
- [x] Activar o desactivar el limitador reconstruye la cadena (único cambio con corte)
- [x] Error de mpv al aplicar la cadena deja el EQ desactivado con toast "Ecualizador no disponible"
- [x] `ComandoEq` (Activar, Banda, Preamp, Limitador, Preset, Restablecer, ReplayGain, ReplayGainPreamp) atendido por el hilo reproductor y `EstadoEq` publicado dentro de `EstadoReproduccion`
- [x] Ganancias acotadas −12..12 dB en pasos de 0,5; preamp −12..12; mover una banda con preset aplicado pasa a personalizado
- [x] Persistencia de ganancias, preamp, preset, activo y limitador en AJUSTES con debounce de 500 ms y restauración al arrancar según `activo_al_arrancar`
- [x] ReplayGain: propiedades `replaygain` (no/track/album), `replaygain-preamp`, `replaygain-clip = no`, `replaygain-fallback = 0`, aplicadas con o sin EQ y persistidas

## Overlay del ecualizador
- [x] `E` abre y cierra el overlay sobre cualquier vista salvo el protector; captura las teclas mientras está abierto y no oculta la barra inferior
- [x] Barras verticales por banda (preamp como banda 0) con valores numéricos, marca `▲` de banda seleccionada y cabecera con preset, EQ, limitador y ReplayGain
- [x] `h`/`l` seleccionan banda, `j`/`k` ±1 dB, `J`/`K` ±0,5 dB, `0` pone la banda a 0 y `R` restablece todo a Plano
- [x] `Tab` alterna entre barras y lista de presets; `Enter` aplica el preset seleccionado; los presets propios llevan `♦`
- [x] `N` guarda la curva actual como preset propio con nombre validado (1-40 caracteres, único normalizado, distinto de los integrados)
- [x] `D` elimina un preset propio con confirmación; los integrados no se pueden eliminar ni editar
- [x] `e` activa/desactiva el EQ, `x` el limitador y `g` cicla ReplayGain no → pista → álbum
- [x] Indicadores "EQ" y "RG" en la barra inferior (reducidos a `E`/`G` en modo compacto); `RG` en tenue si la pista no tiene etiquetas ReplayGain
- [x] Modo compacto del overlay (< 60 columnas: solo valores y presets) y modo `ascii` (`#`, `^`, `*`)

## Letras: fuentes y parser
- [x] `lrc.rs` sin E/S: timestamps `[mm:ss.xx]` múltiples por línea, metadatos (`ar`, `ti`, `al`, `offset`…), eliminación de marcas enhanced `<mm:ss.xx>`, líneas vacías con timestamp conservadas, orden por tiempo, BOM y CRLF normalizados
  - AC: Dado un LRC con dos timestamps en una línea y `[offset:-200]`, cuando se parsea, entonces hay dos entradas con ese texto y el offset es −200
- [x] Detección de sincronización (`es_sincronizada`) en cualquier texto, incluidas letras embebidas con formato LRC
- [x] Trait `FuenteLetras` y resolutor por orden con caché en memoria por `pista_id`, invalidada al reescanear la pista
- [x] `FuenteFichero`: `.lrc` y `.txt` con el mismo nombre junto a la pista, y `letras.carpeta/<artista> - <titulo>.lrc|.txt` con nombres normalizados; `.lrc` > 256 KB ignorados y `.txt` > 64 KB truncados con aviso
- [x] `FuenteEtiqueta`: `UnsynchronizedText` (ID3 USLT) y `LYRICS` (Vorbis/MP4) vía `lofty`, leídas bajo demanda; si tarda más de 50 ms, en el hilo auxiliar
- [x] Decodificación UTF-8 con respaldo Latin-1 y limpieza de caracteres de control
- [x] `fuente_preferida` por pista en LETRAS cuando el usuario cambia de fuente con `s`
- [x] Fixtures: pista con `.lrc` sincronizado, pista con `.txt`, pista con USLT embebida y pista con LRC dentro de la etiqueta

## Vista Letras
- [x] Sección `9 Letras` accesible con `9`, con estados sincronizada (siguiendo / manual), estática, sin letra y no aplicable (emisoras)
- [x] Seguimiento: `sincronia::linea_actual` por búsqueda binaria con `t = time-pos + offset_lrc + offset_usuario`, línea actual centrada, en acento y con `▶`, resto en tenue
- [x] `j`/`k`/rueda pasan a estado manual durante 5 s (el `▶` sigue moviéndose sin desplazar); `Enter` vuelve a seguimiento
- [x] `Enter` sobre una línea sincronizada salta a su tiempo (compensando offsets) y vuelve a seguimiento
- [x] `(` / `)` ajustan el offset ±100 ms (`Shift` ±500 ms), acotado ±30 s, guardado en LETRAS.offset_ms con toast del valor
- [x] `s` alterna fichero/etiqueta cuando hay más de una fuente, con indicador "fichero ▸ etiqueta" en la cabecera
- [x] Letra estática con scroll libre, `gg`/`G`, sin resaltado ni offset
- [x] Sin letra: mensaje con las rutas exactas donde se buscó (`.lrc`/`.txt` junto a la pista y en `letras.carpeta`) y la etiqueta
- [x] Cambio de pista con la vista abierta resuelve la nueva letra sin salir; tick de 100 ms solo mientras la vista o la superposición son visibles

## Letras sobre la visual
- [x] `9` en modo visual alterna la superposición (persistida en `letras_superpuestas`); solo con letra sincronizada
- [x] Banda inferior del canvas de `tamano_superposicion` líneas con fondo del tema opaco, línea actual con `▶` y vecinas en tenue
- [x] `(` / `)` funcionan también en modo visual con la superposición activa
- [x] La cabecera del modo visual indica "letras" cuando la superposición está activa

## Configuración y entrega
- [x] Secciones `[ecualizador]` (activo_al_arrancar, limitador, replaygain, replaygain_preamp_db) y `[letras]` (carpeta, superpuestas, tamano_superposicion) validadas y en `config.ejemplo.toml`
- [x] Overlay de ayuda con las secciones Ecualizador y Letras
- [x] README: ecualizador, presets, ReplayGain, dónde colocar `.lrc`, offsets, superposición y aviso de que `mmmusic.db` contiene presets y offsets
- [x] Tests: `cadena_eq.rs` (cadena, comandos, bypass, presets integrados), `lrc.rs` (parser, sincronía, enhanced, offset), `letras.rs` (resolución por fuentes con fixtures y `fuente_preferida`)
- [x] `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios

---

**Progreso Fase 4:** 50 / 50 funcionalidades

**Total mmmusic (Fases 1-5):** 340 / 340 funcionalidades
