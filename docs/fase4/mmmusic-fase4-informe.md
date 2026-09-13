# mmmusic - Fase 4: Ecualizador y Letras

## Especificación Funcional

**Versión:** 1.0
**Fecha:** 13 de septiembre de 2026
**Cliente:** Proyecto personal (Hector) — sin cliente externo

---

## 1. Visión General

La Fase 4 completa la parte "de escucha" de mmmusic: un ecualizador gráfico de 10 bandas con preamplificador, limitador y presets (integrados y propios), normalización de volumen ReplayGain, y letras desde ficheros `.lrc` o etiquetas embebidas con vista sincronizada, offset por pista y modo combinado sobre las visuales. Todo se apoya en la cadena de filtros `af` de mpv (T-1) y en la lectura de etiquetas con `lofty` (F1). No hay red nueva; se cierra además la deuda R-20 de la Fase 5.

### Objetivos principales

1. Ecualizar en tiempo real sin cortes de audio: cadena de filtros construida una vez y ajustada con `af-command`.
2. Presets integrados y presets del usuario guardados en la base de datos, con preamp y limitador para evitar recortes.
3. ReplayGain nativo de mpv (pista/álbum) activable, leyendo las etiquetas existentes.
4. Letras locales: `.lrc` junto a la pista o en una carpeta propia, y etiquetas `USLT`/`LYRICS` embebidas, con detección automática de sincronización.
5. Vista `9 Letras` sincronizada con desplazamiento automático, salto a línea, offset por pista y modo combinado letras + visual.
6. Sustituir el resolutor DNS propio de la Fase 5 por `dns-lookup` (R-20) y adoptar `docs/faseN/` como ubicación real de la documentación.

### Contexto

Fases 1, 2, 3 y 5 completadas y validadas (290/290). Las letras dependen de lo que exista en disco (biblioteca de Soulseek y música propia); la descarga online (LRCLIB) queda como fase futura y el módulo se diseña con una fuente enchufable para ello.

---

## 2. Arquitectura Técnica

### Stack tecnológico (cambios)

| Capa | Tecnología | Motivo |
|------|-----------|--------|
| Ecualizador | mpv `af` con filtros lavfi `equalizer` ×10 (etiquetados `@eq1`…`@eq10`), `volume` (`@pre`), `alimiter` (`@lim`); ajustes en caliente con `af-command` | Sin reconstruir la cadena en cada cambio → sin cortes |
| ReplayGain | propiedades mpv `replaygain`, `replaygain-preamp`, `replaygain-clip`, `replaygain-fallback` | Nativo, lee etiquetas `REPLAYGAIN_*` |
| Letras | `lofty` (etiquetas `UnsynchronizedText` / `LYRICS`), parser LRC propio (sin E/S) | Ficheros locales |
| DNS (R-20) | crate `dns-lookup` (`lookup_host`, `lookup_addr` → libc `getnameinfo`) | Sustituye `radio/espejos.rs` |
| Resto | Sin cambios | |

### Estructura de carpetas (añadidos y cambios)

```
mmmusic/
├── docs/
│   └── fase4/
│       ├── mmmusic-fase4-informe.md
│       ├── mmmusic-fase4-checklist.md
│       ├── mmmusic-fase4-prompt.md
│       └── mmmusic-fase4-implementacion.md   (lo escribe el agente)
├── src/
│   ├── ecualizador/
│   │   ├── mod.rs               # EstadoEq, ComandoEq, integración con el hilo reproductor
│   │   ├── cadena.rs            # construcción de la cadena af y comandos af-command (sin E/S, testeable)
│   │   ├── presets.rs           # presets integrados y CRUD de PRESETS_EQ
│   │   └── replaygain.rs        # mapeo de modos a propiedades mpv
│   ├── letras/
│   │   ├── mod.rs               # trait FuenteLetras, resolución por prioridad, caché en memoria
│   │   ├── lrc.rs               # parser LRC (timestamps múltiples, offset, enhanced sin karaoke) sin E/S
│   │   ├── fichero.rs           # fuente: .lrc/.txt junto a la pista y en letras.carpeta
│   │   ├── etiqueta.rs          # fuente: USLT / LYRICS embebidas vía lofty
│   │   └── sincronia.rs         # línea actual por time-pos + offset (búsqueda binaria), sin E/S
│   ├── biblioteca/
│   │   ├── migraciones/005_ecualizador_letras.sql
│   │   └── consultas.rs         # + presets_eq::*, letras::{offset, fijar_offset}
│   ├── radio/
│   │   └── espejos.rs           # REESCRITO sobre dns-lookup (R-20)
│   ├── reproductor/
│   │   ├── mpv.rs               # + af, af-command, replaygain
│   │   └── mod.rs               # + ComandoEq, EstadoEq en EstadoReproduccion
│   └── ui/
│       ├── vistas/letras.rs     # sección 9
│       ├── vistas/visual.rs     # + superposición de letras
│       ├── componentes/ecualizador.rs  # overlay E
│       └── barra_inferior.rs    # + indicador EQ/RG
└── tests/
    ├── cadena_eq.rs             # cadena af, comandos, bypass, presets
    ├── lrc.rs                   # parser y sincronía
    └── letras.rs                # resolución de fuentes con fixtures (.lrc, .txt, etiqueta USLT)
```

### Diagrama de arquitectura

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ Hilo UI                                   Hilo Reproductor                    │
│  overlay EQ ──ComandoEq──────────────────► ecualizador::aplicar()              │
│  (E)                                        ├─ primera vez: mpv `af` = cadena   │
│                                             │   "@pre:volume … @eq1:lavfi=[equalizer…] … @lim:alimiter"
│                                             └─ cambios: mpv `af-command @eqN g <dB>` │
│                                                          (fallback: reconstruir cadena)│
│  vista Letras ◄──watch<EstadoReproduccion>── time-pos ──► libmpv ──► PipeWire   │
│   letras::resolver(pista) ──► FuenteLetras[fichero, etiqueta] → Letra{lineas,sync}
│   sincronia::linea_actual(time-pos + offset)                                   │
│                                                                                │
│  SQLite: PRESETS_EQ, LETRAS (offset por pista), AJUSTES (estado del EQ y RG)   │
└───────────────────────────────────────────────────────────────────────────────┘
```

Principios: el EQ vive en el hilo reproductor (es estado de mpv); la UI solo envía comandos y lee el estado. Las letras se resuelven en el hilo de UI (lectura de un fichero pequeño o de etiquetas ya en caché) con caché en memoria por `pista_id`; si la lectura de etiquetas supera 50 ms se pasa al hilo auxiliar de imágenes (mismo patrón que colores).

---

## 3. Modelo de Datos

### Diagrama E-R (nuevo)

```
┌──────────────────────┐          ┌──────────────────────┐        ┌──────────┐
│ PRESETS_EQ (nueva)   │          │ LETRAS (nueva)       │*      1│ PISTAS   │
│──────────────────────│          │──────────────────────│────────│ (F1)     │
│ id                   │          │ pista_id (PK, FK)    │        └──────────┘
│ nombre               │          │ offset_ms            │
│ nombre_norm (único)  │          │ fuente_preferida     │
│ ganancias            │          │ actualizado_en       │
│ preamp_db            │          └──────────────────────┘
│ integrado            │
│ creado_en            │
└──────────────────────┘
```

### Tablas nuevas (migración 005)

**PRESETS_EQ**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| nombre | TEXT NOT NULL | |
| nombre_norm | TEXT NOT NULL UNIQUE | |
| ganancias | TEXT NOT NULL | 10 valores dB separados por comas, orden 31 Hz → 16 kHz, rango −12..12 con un decimal |
| preamp_db | REAL NOT NULL DEFAULT 0 | −12..12 |
| integrado | INTEGER NOT NULL DEFAULT 0 | 1 para los presets de fábrica (no editables ni borrables; se resiembran si faltan) |
| creado_en | TEXT NOT NULL | |

Presets integrados (sembrados por código al arrancar si no existen, T-25): Plano, Rock, Pop, Electrónica, Hip-hop, Vocal, Bass boost, Treble boost, Loudness. Valores en §7.2.

**LETRAS**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| pista_id | INTEGER PK FK → PISTAS ON DELETE CASCADE | |
| offset_ms | INTEGER NOT NULL DEFAULT 0 | Ajuste manual, −30000..30000; se suma al `[offset:]` del LRC |
| fuente_preferida | TEXT NULL | `fichero` / `etiqueta` si el usuario eligió una manualmente |
| actualizado_en | TEXT NOT NULL | |

**AJUSTES** (claves nuevas, sembradas por código): `eq_activo` (0/1), `eq_preset_id` (NULL = personalizado), `eq_ganancias` (10 valores), `eq_preamp_db`, `eq_limitador` (0/1), `replaygain_modo` (`no`/`pista`/`album`), `replaygain_preamp_db`, `letras_superpuestas` (0/1).

### Diagrama de estados: Ecualizador

```
   arranque
      │  eq_activo=0 ──► desactivado (af vacío; mpv sin filtros; bypass total)
      │  eq_activo=1 ──► activo(preset | personalizado)
      ▼
   desactivado ── `e` en overlay / preset elegido ──► activo   (mpv `af` = cadena completa)
   activo ── `e` ──► desactivado                                (mpv `af` = "")
   activo(preset X) ── mover una banda o el preamp ──► activo(personalizado)
   activo(personalizado) ── `N` guardar como preset ──► activo(preset nuevo)
   activo ── error de mpv al aplicar (filtro no disponible) ──► desactivado + toast "Ecualizador no disponible"

   El limitador y ReplayGain son ortogonales: limitador solo existe dentro de la cadena (activo);
   ReplayGain se aplica siempre que replaygain_modo ≠ no, con o sin EQ.
   Transiciones inválidas: `af-command` en desactivado (se ignora; se aplicará al activar).
```

### Diagrama de estados: Vista de letras

```
   sin_letra ── letra sincronizada encontrada ──► siguiendo  ── j/k/rueda ──► manual (5 s sin teclas o Enter → siguiendo)
   sin_letra ── letra sin sincronizar ──────────► estatica   (scroll libre, sin resaltado, sin offset)
   cualquiera ── cambio de pista ──► resolver de nuevo
   cualquiera ── elemento emisora ──► no_aplicable ("No disponible para emisoras")
```

---

## 4. Flujos de Trabajo

### 4.1 Aplicar el ecualizador

```
  Overlay E: tecla j/k sobre la banda 3 (+1 dB)
        │
        ▼
  ComandoEq::Banda { indice: 3, db: +4.0 }  (debounce 50 ms por banda en la UI)
        │
        ▼
  Hilo reproductor: estado_eq.ganancias[3] = 4.0; preset_id = None (personalizado)
        │
        ├─ ¿cadena instalada? ──no──► mpv set `af` = cadena::construir(estado_eq)  (§7.1)
        │                              ¿error? → eq_activo=0, toast "Ecualizador no disponible (mpv sin lavfi)"
        └─ sí ──► mpv `af-command @eq4 g 4.0`
                   ¿error? → reconstruir cadena completa (un solo corte breve)
        │
        ▼
  AJUSTES.eq_ganancias / eq_preset_id persistidos (debounce 500 ms); EstadoReproduccion.eq publicado
  → barra inferior "EQ" y overlay redibujados; las visuales (F3) reflejan el audio ecualizado.
```

### 4.2 Resolver la letra de una pista

```
  Cambio de pista (o entrar en 9 Letras)
        │
        ▼
  ¿caché en memoria para pista_id? ──sí──► usar
        │no
        ▼
  Fuentes en orden (o fuente_preferida si existe):
    1. fichero: <misma carpeta>/<mismo nombre>.lrc → .txt
                letras.carpeta/<artista> - <titulo>.lrc → .txt   (nombres normalizados, sin diacríticos)
    2. etiqueta: USLT (ID3), LYRICS (Vorbis/MP4), SYLT no soportado
        │
        ▼
  Texto → ¿contiene timestamps [mm:ss.xx]? ──sí──► lrc::parsear → Letra{sincronizada: true, lineas: [(ms, texto)], offset_lrc}
                                             └no──► Letra{sincronizada: false, lineas: [texto…]}
        │
        ▼
  Sin fuentes → Letra::Ninguna → vista "Sin letra para esta pista" + ayuda (ruta esperada del .lrc)
  Con más de una fuente → indicador "fichero ▸ etiqueta" y `s` alterna (guarda fuente_preferida)
```

### 4.3 Seguimiento sincronizado

```
  Tick 250 ms (o 100 ms con la vista Letras o la superposición visibles)
        │
        ▼
  t = time-pos_ms + LETRAS.offset_ms + offset_lrc
  i = sincronia::linea_actual(lineas, t)   (búsqueda binaria; -1 antes de la primera)
        │
        ├─ estado siguiendo → desplazar para centrar i; resaltar i; atenuar el resto
        └─ estado manual   → solo resaltar i sin desplazar
  Enter sobre una línea (sincronizada) → ComandoReproductor::Buscar(ms de la línea − offset, absoluto) y volver a siguiendo
  `(` / `)` → offset −100 / +100 ms (Shift: ±500) → LETRAS.fijar_offset; toast "offset −300 ms"
```

### 4.4 Diagrama de secuencia: seleccionar un preset

```
  Usuario     Hilo UI (overlay)         Hilo Reproductor            libmpv
    │ Enter "Rock" │                          │                        │
    │─────────────►│ ComandoEq::Preset(id)    │                        │
    │              │─────────────────────────►│ cargar PRESETS_EQ(id)  │
    │              │                          │ af-command @pre volume +2dB ─►│
    │              │                          │ af-command @eq1 g 5 ─────────►│  (×10)
    │              │                          │ AJUSTES ← preset_id, ganancias │
    │              │◄── watch: eq{preset:"Rock"} ─────────                    │
    │              │ redibuja barras          │                        │
```

### 4.5 Sustitución del resolutor DNS (R-20)

```
  radio/espejos.rs: `lookup_host("all.api.radio-browser.info")` → IPs
                    para cada IP: `lookup_addr(ip)` → nombre del espejo (o nombre agregado si falla)
  Misma interfaz pública que en F5 (lista de espejos, elección aleatoria, caché 24 h); se elimina el
  codificador/decodificador DNS propio y sus tests se sustituyen por tests de la lógica de selección.
```

---

## 5. Comandos, Atajos y API Interna

### 5.1 Atajos nuevos

| Tecla | Contexto | Acción |
|-------|----------|--------|
| `E` | Global (no en protector) | Abrir / cerrar el overlay del ecualizador |
| `h` / `l` | Overlay EQ | Banda anterior / siguiente (banda 0 = preamp) |
| `j` / `k` | Overlay EQ | −1 / +1 dB en la banda seleccionada (`J`/`K`: ±0,5 dB) |
| `0` | Overlay EQ | Poner la banda seleccionada a 0 dB |
| `R` | Overlay EQ | Restablecer todas las bandas y el preamp a 0 (preset Plano) |
| `Tab` | Overlay EQ | Alternar foco entre barras y lista de presets |
| `Enter` | Lista de presets | Aplicar preset |
| `N` | Overlay EQ | Guardar la curva actual como preset nuevo (diálogo de nombre) |
| `D` | Lista de presets | Eliminar preset propio (confirmación); los integrados no se borran |
| `e` | Overlay EQ | Activar / desactivar el ecualizador |
| `x` | Overlay EQ | Activar / desactivar el limitador |
| `g` | Overlay EQ | Ciclar ReplayGain: no → pista → álbum |
| `Esc` | Overlay EQ | Cerrar |
| `9` | Global | Ir a Letras; en modo visual, alternar letras superpuestas |
| `j` / `k` / rueda | Letras | Desplazamiento manual (pasa a estado manual 5 s) |
| `Enter` | Letras (sincronizada) | Saltar a la línea seleccionada y volver a seguimiento |
| `(` / `)` | Letras y modo visual con letras | Offset −100 / +100 ms (`Shift`: ±500 ms), guardado por pista |
| `s` | Letras | Alternar fuente (fichero / etiqueta) cuando hay más de una |
| `gg` / `G` | Letras | Inicio / fin |

`s` era "aleatorio" en el resto de la app; dentro de la vista Letras cambia de fuente y el aleatorio sigue disponible desde cualquier otra vista. `R`, `N`, `D` dentro del overlay no afectan a playlists (el overlay captura las teclas).

### 5.2 API interna

`ComandoEq` (UI → reproductor): `Activar(bool)`, `Banda { indice: usize, db: f32 }`, `Preamp(db)`, `Limitador(bool)`, `Preset(id)`, `Restablecer`, `ReplayGain(modo)`, `ReplayGainPreamp(db)`.

`EstadoEq` (dentro de `EstadoReproduccion`): `activo`, `ganancias: [f32; 10]`, `preamp_db`, `limitador`, `preset: Option<(id, nombre)>`, `replaygain: Modo`, `replaygain_preamp_db`, `disponible: bool` (false si mpv no tiene lavfi).

`cadena.rs` (sin E/S): `construir(&EstadoEq) -> String` (cadena `af`), `comando_banda(i, db) -> (etiqueta, "g", valor)`, `comando_preamp(db)`, `es_bypass(&EstadoEq) -> bool`.

`FuenteLetras` (trait): `fn buscar(&self, pista: &PistaResumen) -> Option<TextoLetra>`; implementaciones `FuenteFichero`, `FuenteEtiqueta`; el resolutor recorre `Vec<Box<dyn FuenteLetras>>` en orden. (Una futura `FuenteLrclib` encajaría aquí sin tocar la vista.)

`Letra`: `Ninguna | Estatica { lineas: Vec<String>, fuente } | Sincronizada { lineas: Vec<(u32 ms, String)>, offset_lrc_ms, fuente }`.

Consultas nuevas: `presets_eq::{listar, obtener, crear, eliminar, sembrar_integrados}`, `letras::{offset, fijar_offset, fuente_preferida, fijar_fuente}`, `ajustes` para las claves nuevas.

### 5.3 Configuración

```toml
[ecualizador]
activo_al_arrancar = "recordar"   # "recordar" | "si" | "no"
limitador = true
replaygain = "no"                 # "no" | "pista" | "album"  (valor inicial; después se recuerda)
replaygain_preamp_db = 0.0

[letras]
carpeta = "~/.local/share/mmmusic/letras"   # además de junto a la pista
superpuestas = false                        # letras sobre la visual al entrar en modo visual
tamano_superposicion = 3                    # líneas visibles (actual ± 1)
```

---

## 6. Interfaz de Usuario

### Mapa de navegación (cambios)

```
mmmusic
├── [9] Letras   (vista sincronizada / estática / sin letra / no aplicable)
├── [E] Overlay Ecualizador (sobre cualquier vista; barras + presets + RG)
├── [7] Visual + [9] letras superpuestas
└── Barra inferior: + "EQ" (activo) y "RG" (ReplayGain ≠ no) junto al volumen
```

### Mockup 1: overlay del ecualizador

```
              ┌ Ecualizador · Rock · EQ on · lim on · RG: álbum ────────────────────┐
              │  +12                                                                │
              │   +6      ▄▄   ▄▄                                   ▄▄   ▄▄         │
              │    0 ──── ██ ─ ██ ─ ▄▄ ─ ── ─ ── ─ ── ─ ── ─ ▄▄ ─ ██ ─ ██ ───────  │
              │   -6                     ▀▀   ▀▀                                    │
              │  -12                                                                │
              │       pre  31   62  125  250  500   1k   2k   4k   8k  16k          │
              │       +2   +5   +4   +2   -1   -2    0   +1   +3   +4   +4  dB      │
              │                          ▲                                          │
              │ Presets ────────────────────────────────────────────────────────── │
              │  Plano   ▶Rock   Pop   Electrónica   Hip-hop   Vocal   Bass boost   │
              │  Treble boost   Loudness   ♦ Coche noche   ♦ Vaporwave suave        │
              │                                                                     │
              │ h/l banda · j/k ±1 dB · 0 reset · R plano · Tab presets · N guardar │
              │ e EQ on/off · x limitador · g ReplayGain · Esc cerrar               │
              └─────────────────────────────────────────────────────────────────────┘
```

Los presets propios llevan `♦`. La barra seleccionada se marca con `▲`. Con menos de 60 columnas el overlay muestra solo los valores numéricos y los presets.

### Mockup 2: vista Letras (sincronizada, siguiendo)

```
┌ mmmusic ─────────┬──────────────────────────────────────────────────────────┐
│  9 Letras      ◄ │ Nocturne Drive · Midnight Premiere        fichero ▸ etiqueta│
│                  │ sincronizada · offset −200 ms                            │
│                  │──────────────────────────────────────────────────────────│
│                  │                                                          │
│                  │              neon signs reflect on wet asphalt           │
│                  │              the city breathes in slow motion            │
│                  │                                                          │
│                  │          ▶ we drive until the tape runs out              │
│                  │                                                          │
│                  │              static hums beneath the chorus              │
│                  │              headlights carve the night in two           │
│                  │                                                          │
│                  │                                                          │
│ 4.812 pistas     │ j/k desplazar · Enter saltar · ( ) offset · s fuente     │
├──────────────────┴──────────────────────────────────────────────────────────┤
│ ▀▀▀ Nocturne Drive           ▂▄▆█▅▃▂▁▂▃▂▁  ⏮  ⏸  ⏭   🔀 🔁 ♪ vol 80 % EQ RG ↑│
│ ▄▄▄ Midnight Premiere · Late Night Tapes  1:27 ━━━━━━━●───────────── 4:12  │
└─────────────────────────────────────────────────────────────────────────────┘
```

Línea actual en acento y con `▶`; las demás en color tenue; centrado vertical. En estado manual el `▶` sigue moviéndose pero la vista no se desplaza.

### Mockup 3: modo visual con letras superpuestas

```
┌ mmmusic · Visual 3/6 Ambiente · letras ─────────────────────────────────────┐
│           ⣠⣴⣶⣦⣄        ⢀⣠⣤⣤⣄⡀                                                │
│        ⣰⣿⣿⣿⣿⣿⣿⣆     ⣠⣿⣿⣿⣿⣿⣿⣿⣦                ⢀⣤⣶⣶⣤⡀                        │
│       ⣿⣿⣿⣿⣿⣿⣿⣿⣿   ⣸⣿⣿⣿⣿⣿⣿⣿⣿⣿⡇             ⣰⣿⣿⣿⣿⣿⣿⣆                       │
│        ⠹⣿⣿⣿⣿⣿⡿⠃    ⠹⣿⣿⣿⣿⣿⣿⣿⣿⡿⠃             ⢿⣿⣿⣿⣿⣿⣿⡿                       │
│                                                                              │
│                        the city breathes in slow motion                      │
│                     ▶ we drive until the tape runs out                       │
│                        static hums beneath the chorus                        │
├──────────────────────────────────────────────────────────────────────────────┤
│ ▀▀▀ Nocturne Drive           ▂▄▆█▅▃▂▁▂▃▂▁  ⏮  ⏸  ⏭   🔀 🔁 ♪ vol 80 % EQ RG ↑│
```

Banda inferior del canvas (`tamano_superposicion` líneas) con fondo del tema al 100 % para legibilidad; solo con letra sincronizada; con letra estática o sin letra no se superpone nada.

### Mockup 4: sin letra

```
│ Nocturne Drive · Midnight Premiere                                          │
│─────────────────────────────────────────────────────────────────────────────│
│                                                                             │
│                    Sin letra para esta pista                                │
│                                                                             │
│   mmmusic busca, por este orden:                                            │
│   1. /home/hector/Music/Midnight Premiere/Late Night Tapes/02 Nocturne Drive.lrc (o .txt)│
│   2. ~/.local/share/mmmusic/letras/midnight premiere - nocturne drive.lrc     │
│   3. Etiqueta de letra embebida en el fichero                               │
```

### Notas de UX

- El overlay EQ se dibuja sobre la vista actual sin ocultar la barra inferior; los cambios suenan al instante.
- El indicador "EQ" aparece solo con el ecualizador activo y "RG" solo con ReplayGain ≠ no; en modo compacto se reducen a `E`/`G`.
- Modo `ascii`: barras con `#`, `▶` → `>`, `♦` → `*`, `▲` → `^`.
- La vista Letras con letra estática muestra el texto completo con scroll libre y sin `▶`.
- Cambiar de pista mientras se está en Letras resuelve la nueva letra sin salir de la vista; si no hay letra, se muestra el mensaje con las rutas esperadas (útil para saber dónde dejar un `.lrc`).
- Todas las letras se muestran tal cual (sin normalizar); se limpian caracteres de control y se respetan líneas vacías como separadores de estrofa.

---

## 7. Lógica de Negocio

### 7.1 Cadena de filtros

```python
BANDAS_HZ = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
Q = 1.41   # ≈ una octava

def construir_cadena(eq):
    if not eq.activo: return ""                      # bypass: sin filtros
    partes = [f"@pre:lavfi=[volume=volume={eq.preamp_db:.1f}dB]"]
    for i, (f, g) in enumerate(zip(BANDAS_HZ, eq.ganancias), 1):
        partes.append(f"@eq{i}:lavfi=[equalizer=f={f}:width_type=q:width={Q}:gain={g:.1f}]")
    if eq.limitador:
        partes.append("@lim:lavfi=[alimiter=limit=0.891:attack=5:release=50:level=false]")  # techo −1 dBFS
    return ",".join(partes)

def comando_banda(i, db):   return (f"@eq{i+1}", "g", f"{db:.1f}")
def comando_preamp(db):     return ("@pre", "volume", f"{db:.1f}dB")

def es_bypass(eq):
    return (not eq.activo) or (all(abs(g) < 0.05 for g in eq.ganancias) and abs(eq.preamp_db) < 0.05 and not eq.limitador)
```

- Con `es_bypass` verdadero se fija `af=""` aunque `activo` sea true (cero coste de CPU); al mover una banda se instala la cadena.
- Activar o desactivar el limitador reconstruye la cadena (no tiene comando en caliente); es el único cambio con corte perceptible y se avisa en la ayuda.
- Ganancias acotadas a −12..12 con paso de 0,5; se redondean al almacenar.

### 7.2 Presets integrados (dB, 31 → 16k; preamp)

| Preset | 31 | 62 | 125 | 250 | 500 | 1k | 2k | 4k | 8k | 16k | Pre |
|--------|----|----|-----|-----|-----|----|----|----|----|-----|-----|
| Plano | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| Rock | 5 | 4 | 2 | -1 | -2 | 0 | 1 | 3 | 4 | 4 | -2 |
| Pop | -1 | 1 | 3 | 4 | 3 | 0 | -1 | -1 | 1 | 2 | -1 |
| Electrónica | 4 | 3 | 1 | 0 | -2 | 1 | 0 | 1 | 3 | 4 | -2 |
| Hip-hop | 5 | 4 | 1 | 2 | -1 | -1 | 1 | 0 | 1 | 2 | -2 |
| Vocal | -2 | -3 | -2 | 1 | 3 | 4 | 3 | 1 | 0 | -1 | 0 |
| Bass boost | 6 | 5 | 4 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | -3 |
| Treble boost | 0 | 0 | 0 | 0 | 0 | 1 | 2 | 4 | 5 | 6 | -3 |
| Loudness | 5 | 3 | 0 | -1 | -2 | -2 | -1 | 1 | 3 | 5 | -3 |

Los preamps negativos compensan la suma de bandas para evitar recorte incluso con el limitador apagado.

### 7.3 ReplayGain

- `replaygain = track|album|no` según `replaygain_modo`; `replaygain-preamp` = `replaygain_preamp_db`; `replaygain-clip = no` (el limitador ya protege); `replaygain-fallback = 0`.
- Pistas sin etiquetas ReplayGain suenan sin cambio (fallback 0) y la barra muestra `RG` en tenue.
- Emisoras: ReplayGain no aplica (mpv lo ignora sin etiquetas).

### 7.4 Parser LRC

```python
import re
TS = re.compile(r"\[(\d{1,2}):(\d{2})(?:[.:](\d{1,3}))?\]")
META = re.compile(r"^\[(ar|ti|al|by|offset|length|re|ve):(.*)\]$")
PALABRA = re.compile(r"<\d{1,2}:\d{2}(?:[.:]\d{1,3})?>")   # enhanced LRC: se elimina

def parsear_lrc(texto):
    lineas, offset = [], 0
    for cruda in texto.splitlines():
        cruda = cruda.strip()
        m = META.match(cruda)
        if m:
            if m.group(1) == "offset": offset = int(m.group(2).strip() or 0)   # ms; positivo = adelantar
            continue
        marcas = TS.findall(cruda)
        if not marcas: continue
        contenido = PALABRA.sub("", TS.sub("", cruda)).strip()
        for mm, ss, frac in marcas:
            ms = int(mm) * 60000 + int(ss) * 1000 + (int(frac.ljust(3, "0")) if frac else 0)
            lineas.append((ms, contenido))
    lineas.sort(key=lambda x: x[0])
    return lineas, offset

def es_sincronizada(texto): return TS.search(texto) is not None
```

- Líneas con timestamp y texto vacío se conservan como separadores (permiten "silencios" en el seguimiento).
- Timestamps duplicados: se conservan ambos (coros repetidos).
- Ficheros con BOM o CRLF se normalizan; codificación UTF-8 con detección de Latin-1 como respaldo.

### 7.5 Sincronía

```python
def linea_actual(lineas, t_ms):
    """índice de la última línea con marca <= t, o -1"""
    lo, hi, r = 0, len(lineas) - 1, -1
    while lo <= hi:
        m = (lo + hi) // 2
        if lineas[m][0] <= t_ms: r, lo = m, m + 1
        else: hi = m - 1
    return r
```

`t_ms = time_pos_ms + offset_lrc + LETRAS.offset_ms`. Al saltar con `Enter` se busca `lineas[i].ms − offset_lrc − offset_usuario` para que la línea quede alineada.

### 7.6 Validaciones y casos especiales

- Nombre de preset: 1–40 caracteres, único normalizado, distinto de los integrados.
- `.lrc` mayores de 256 KB se ignoran (no son letras); `.txt` mayores de 64 KB se truncan con aviso.
- Letra embebida con timestamps (algunos etiquetadores guardan LRC en `LYRICS`) se trata como sincronizada.
- Pista sin duración conocida o emisora: vista "No disponible".
- Si mpv no tiene lavfi (build mínima), el EQ se marca `disponible=false`, el overlay explica el motivo y ReplayGain sigue funcionando.
- Offset por pista se conserva al reescanear (LETRAS referencia `pista_id`, que se mantiene por `ruta`).

---

## 8. Requisitos No Funcionales

### Seguridad

- Sin red nueva. `dns-lookup` sustituye el parser DNS propio (menos superficie).
- Letras y `.txt` se leen como texto: se eliminan caracteres de control y secuencias de escape antes de dibujar.
- Los presets y offsets son datos del usuario; sin entradas externas.

### Protección de datos

- Nada sale de la máquina. Las letras no se envían a los servicios de scrobbling.

### Backups y recuperación

- Migración 005 aditiva (dos tablas). Los presets propios y los offsets de letras no son regenerables: se documenta en README que `mmmusic.db` los contiene (mismo aviso que playlists y favoritas).

### Rendimiento

- Cambio de banda audible en < 50 ms; sin cortes salvo al alternar el limitador.
- Resolución de letra < 20 ms desde fichero; la letra embebida no se guarda en el escaneo, se lee bajo demanda (< 50 ms); por encima, hilo auxiliar.
- Vista Letras a 10 fps (tick 100 ms) solo mientras es visible.

### Accesibilidad

- Overlay EQ íntegramente por teclado; valores numéricos siempre visibles junto a las barras.
- Letras con contraste del tema; línea actual marcada con `▶`, no solo por color.
- Modo `ascii` completo.

---

## 9. Integraciones

| Integración | Detalle |
|-------------|---------|
| mpv | Propiedad `af` (cadena etiquetada), comando `af-command`, propiedades `replaygain*`; detección de lavfi al arrancar (`af` de prueba) |
| lofty | Lectura de `UnsynchronizedText` (ID3v2 USLT) y `LYRICS` (Vorbis Comment / MP4 `©lyr`) |
| Sistema de ficheros | `.lrc`/`.txt` junto a la pista y en `letras.carpeta` (solo lectura) |
| dns-lookup | `lookup_host` + `lookup_addr` para los espejos de Radio Browser (R-20) |

---

## 10. Decisiones Técnicas (ADR-lite)

**DT-47 — Cadena etiquetada + `af-command` frente a reconstruir `af` en cada cambio.** Reconstruir reinicia el grafo de filtros y produce un corte audible; `af-command` cambia la ganancia en caliente. Solo el limitador exige reconstrucción.

**DT-48 — 10 bandas ISO con `equalizer` frente a `superequalizer` de 18 bandas.** Diez bandas son el estándar de los reproductores de escritorio y caben en el overlay; `superequalizer` no admite comandos en caliente.

**DT-49 — Limitador `alimiter` a −1 dBFS opcional, y preamps negativos en los presets.** Doble protección: los presets no recortan por diseño y el limitador cubre curvas personalizadas agresivas.

**DT-50 — ReplayGain nativo de mpv frente a análisis propio.** mpv aplica las etiquetas existentes sin coste; calcular ReplayGain para ficheros sin etiquetas (análisis EBU R128 en el escaneo) queda como idea futura.

**DT-51 — Letras solo locales con `FuenteLetras` enchufable.** Decisión del usuario para esta fase; el trait permite añadir LRCLIB u otras fuentes sin tocar la vista ni la sincronía.

**DT-52 — Offset por pista en tabla propia (LETRAS) frente a modificar el `.lrc`.** No se escribe nunca en la biblioteca; el offset sobrevive a reescaneos y a cambios de fuente.

**DT-53 — `dns-lookup` para los espejos (cierra R-20).** `getnameinfo` del sistema en lugar de un cliente DNS a mano; misma interfaz pública.

**DT-54 — `docs/faseN/` como ubicación de la documentación.** Cuatro fases de implementación la han creado así; se actualiza `CLAUDE.md` para que la documentación siga a la realidad.

---

## 11. Plan de Desarrollo

| Sprint | Contenido | Entregable verificable |
|--------|-----------|------------------------|
| S1 — Motor de EQ y deuda | Migración 005; `cadena.rs` con tests; integración en mpv (`af`, `af-command`, detección de lavfi, fallback); preamp, limitador, ReplayGain; persistencia en AJUSTES; presets integrados sembrados; R-20 con `dns-lookup`; `docs/faseN/` en `CLAUDE.md` | Cambiar una banda por comando interno se oye al instante |
| S2 — Overlay del ecualizador | Overlay `E` con barras y presets, atajos, CRUD de presets propios, indicadores EQ/RG, modo compacto y ascii | Overlay completo y presets propios |
| S3 — Letras | Parser LRC, fuentes fichero y etiqueta, resolución con caché, vista `9 Letras` (sincronizada, estática, sin letra, no aplicable), offset por pista, cambio de fuente, fixtures | Letra sincronizada siguiendo la pista de prueba |
| S4 — Combinado y cierre | Letras superpuestas en modo visual, `(`/`)` en modo visual, tick dinámico, ayuda, README, config, tests, clippy/fmt | Suite verde y documentación |

**Estimación:** 4 sprints de ~1 semana → 3–4 semanas nominales (1–2 sesiones al ritmo real). Supuestos: los anteriores; el mpv de Arch incluye lavfi (lo hace por defecto); el agente puede probar audio en su entorno como en F1.

---

## 12. Conexiones con Otras Fases

- **Desde F1:** envoltorio mpv, etiquetas `lofty`, barra inferior, diálogos; `s` (aleatorio) se redefine solo dentro de la vista Letras.
- **Desde F3:** el modo visual gana la superposición de letras; las visuales reflejan el audio ecualizado (captura post-filtros).
- **Desde F5:** R-20 cerrado; ReplayGain y letras no aplican a emisoras.
- **Hacia una fase de letras online:** `FuenteLrclib` como tercera fuente, con su host en la allowlist y caché de negativos.
- **Ideas:** cálculo de ReplayGain en el escaneo (EBU R128), presets por álbum, exportación/importación de presets, karaoke por palabra (enhanced LRC).
