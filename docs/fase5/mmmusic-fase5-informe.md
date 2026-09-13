# mmmusic - Fase 5: Radio y Streams

## Especificación Funcional

**Versión:** 1.0
**Fecha:** 13 de septiembre de 2026
**Cliente:** Proyecto personal (Hector) — sin cliente externo

---

## 1. Visión General

La Fase 5 añade radio por internet a mmmusic: emisoras por URL (HTTP/ICY/HLS), importación de PLS/M3U, búsqueda en el directorio abierto Radio Browser, favoritas, metadatos ICY con historial de títulos, reconexión automática, y scrobbling de las canciones que anuncia la emisora. La cola de reproducción pasa a admitir dos tipos de elemento (pista o emisora), cambio de modelo previsto desde la Fase 1.

La Fase 4 (ecualizador y letras) queda en el roadmap sin especificar; los documentos de esta fase llevan el número 5 para conservar la correspondencia con el maestro.

### Objetivos principales

1. Reproducir cualquier emisora por URL con mpv, con indicador de conexión/almacenamiento y reconexión automática con reintentos crecientes.
2. Gestionar emisoras propias: alta por URL, edición, borrado, favoritas con `L`, importación y exportación PLS/M3U.
3. Buscar emisoras en Radio Browser por nombre, país y etiqueta, con caché local de 24 h y logos reutilizando la caché de imágenes.
4. Mostrar el título ICY en la barra inferior, guardar los últimos 50 títulos por emisora y permitir buscar el título actual en la biblioteca.
5. Registrar en historial y scrobblear las canciones anunciadas por ICY (`Artista - Título`) con umbral de 30 s, y enviar now playing.
6. Integrar la radio en cola, MPRIS y visuales sin romper el comportamiento de las pistas locales.

### Contexto

Fases 1-3 completadas y validadas (230/230). Scrobbling (F2) y visuales (F3) deben funcionar igual con radio: la captura de PipeWire no distingue la fuente, y el hilo de scrobbling recibe eventos de "escucha" que ahora pueden venir de un título ICY. Podcasts y grabación de streams quedan fuera.

---

## 2. Arquitectura Técnica

### Stack tecnológico (cambios)

| Capa | Tecnología | Motivo |
|------|-----------|--------|
| Streams | mpv (F1) con `cache=yes`, `demuxer-max-bytes=32MiB`, `stream-lavf-o=reconnect=1,reconnect_streamed=1,reconnect_delay_max=5`, `ytdl=no` | Reproducción y reconexión a bajo nivel; observación de `metadata` (ICY), `paused-for-cache`, `cache-buffering-state`, `demuxer-cache-duration`, `audio-codec-name`, `audio-bitrate` |
| Directorio | Radio Browser (`all.api.radio-browser.info` → espejos resueltos por DNS) vía `ureq` en un hilo de red nuevo | Búsqueda de emisoras sin clave; obligatorio `User-Agent` identificativo |
| Cliente HTTP compartido | `src/red/cliente.rs` (extraído del hilo de scrobbling) | Un solo constructor `ureq` con timeout, User-Agent y `url_permitida()` |
| Logos | `image` (F1) sobre descargas limitadas (≤ 512 KB, 5 s, ≤ 3 redirecciones, `Content-Type: image/*`) | Reutiliza la caché de carátulas y el hilo auxiliar de imágenes |
| Resto | Sin cambios | |

### Estructura de carpetas (añadidos)

```
mmmusic/
├── docs/                                (o docs/fase5/ según lo decidido en CLAUDE.md)
│   ├── mmmusic-fase5-informe.md
│   ├── mmmusic-fase5-checklist.md
│   ├── mmmusic-fase5-prompt.md
│   └── mmmusic-fase5-implementacion.md  (lo escribe el agente)
├── src/
│   ├── red/
│   │   ├── mod.rs
│   │   └── cliente.rs           # constructor ureq compartido, hosts permitidos, descarga limitada de imágenes
│   ├── radio/
│   │   ├── mod.rs               # API pública: emisoras, títulos, importación/exportación
│   │   ├── icy.rs               # parseo "Artista - Título", filtros de ruido (sin E/S)
│   │   ├── listas.rs            # parseo/escritura PLS y M3U/M3U8 de emisoras (sin E/S)
│   │   ├── reconexion.rs        # política de reintentos de stream (sin E/S, testeable)
│   │   └── radiobrowser.rs      # hilo de directorio: espejos, búsqueda, click, caché
│   ├── biblioteca/
│   │   ├── migraciones/004_radio.sql
│   │   └── consultas.rs         # + emisoras::*, titulos_emisora::*, busquedas_radio::*, cola mixta
│   ├── reproductor/
│   │   ├── cola.rs              # ElementoCola { Pista | Emisora }
│   │   ├── mpv.rs               # opciones de stream, observación de metadata/caché
│   │   └── mod.rs               # estados de stream, reconexión, eventos ICY
│   ├── scrobbling/regla.rs      # + regla para títulos ICY (30 s)
│   ├── mpris.rs                 # metadatos de radio
│   └── ui/
│       ├── vistas/radio.rs      # sección 8: pestañas Favoritas · Todas · Buscar · Sonando
│       ├── barra_inferior.rs    # modo directo: título ICY, estado de conexión, tiempo escuchando
│       └── componentes/dialogo.rs # + formulario nombre + URL
└── tests/
    ├── icy.rs
    ├── listas_radio.rs
    ├── reconexion.rs
    └── radio.rs                 # cola mixta, historial/envíos de radio, caché de búsquedas
```

### Diagrama de arquitectura

```
┌────────────────────────────────────────────────────────────────────────────────┐
│                                Proceso mmmusic                                 │
│                                                                                │
│  Hilo UI ──ComandoReproductor::ReemplazarCola(elementos)──► Hilo Reproductor   │
│    ▲                                                            │ loadfile url  │
│    │ watch<EstadoReproduccion{ elemento: Pista|Emisora,         ▼               │
│    │        stream: Conectando|Almacenando|EnDirecto|Reconectando(n)|Rendido,   │
│    │        titulo_icy, tiempo_escuchando, codec, bitrate }>   libmpv ──► HTTP/ICY/HLS
│    │                                                            │ metadata     (emisora,
│    │                                                            ▼              host libre,
│    │                                                  ComandoScrobbling::        lo abre mpv)
│    │                                                  TituloIcy(emisora, titulo)
│    │                                                            │
│  ┌─┴──────────────────┐    ComandoDirectorio     ┌──────────────▼──────────────┐
│  │ Hilo Directorio    │◄─────────────────────────│ Hilo Scrobbling (F2)        │
│  │ (radiobrowser.rs)  │  Buscar / Click / Logo   │  + escuchas ICY (30 s)      │
│  │  - espejos DNS     │                          └─────────────────────────────┘
│  │  - búsqueda + caché│──► ureq (hosts: api.listenbrainz.org, ws.audioscrobbler.com,
│  │  - logos limitados │         *.api.radio-browser.info; logos: cualquier https solo imagen)
│  └────────────────────┘
│  SQLite: EMISORAS, EMISORA_TITULOS, BUSQUEDAS_RADIO; COLA / HISTORIAL / ENVIOS con emisora_id
└────────────────────────────────────────────────────────────────────────────────┘
```

Principios: mpv es el único que abre streams de audio; el hilo de directorio es el único que habla con Radio Browser y descarga logos; la UI nunca espera a la red (búsquedas asíncronas con estado "buscando…").

---

## 3. Modelo de Datos

### Diagrama E-R (nuevo y modificado)

```
┌────────────────────────┐        ┌────────────────────────┐
│ EMISORAS (nueva)       │1      *│ EMISORA_TITULOS (nueva)│
│────────────────────────│────────│────────────────────────│
│ id                     │        │ id                     │
│ nombre                 │        │ emisora_id             │
│ nombre_norm            │        │ titulo                 │
│ url (única)            │        │ visto_en               │
│ pagina_web             │        └────────────────────────┘
│ pais, etiquetas        │
│ codec, bitrate_kbps    │        ┌────────────────────────┐
│ logo_url, logo_ruta    │        │ BUSQUEDAS_RADIO (nueva)│
│ radiobrowser_uuid      │        │────────────────────────│
│ favorita               │        │ clave (PK)             │
│ anadida_en             │        │ respuesta_json         │
│ ultima_reproduccion    │        │ obtenido_en            │
│ ultimo_error           │        └────────────────────────┘
└───────────┬────────────┘
            │1
            │*        (recreadas: pista_id pasa a NULL admisible; exactamente uno de pista_id/emisora_id)
┌───────────┴──────────┐  ┌────────────────────────────┐  ┌──────────────────────────┐
│ COLA (mod.)          │  │ HISTORIAL_REPRODUCCION(mod)│  │ ENVIOS (mod.)            │
│ + tipo pista|emisora │  │ + emisora_id               │  │ + emisora_id             │
│ + emisora_id         │  │ + titulo_icy               │  │ (scrobble de radio toma  │
│ pista_id NULL adm.   │  │ pista_id NULL adm.         │  │  artista/título del      │
└──────────────────────┘  └────────────────────────────┘  │  HISTORIAL enlazado)     │
                                                          └──────────────────────────┘
```

### Tablas nuevas (migración 004)

**EMISORAS**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| nombre | TEXT NOT NULL | |
| nombre_norm | TEXT NOT NULL | Para orden y búsqueda local |
| url | TEXT NOT NULL UNIQUE | URL del stream o de una lista PLS/M3U remota |
| pagina_web | TEXT NULL | |
| pais | TEXT NULL | Código ISO de dos letras si viene del directorio |
| etiquetas | TEXT NULL | Separadas por comas |
| codec | TEXT NULL | `MP3`, `AAC`, `OGG`, `FLAC`, `HLS`… (del directorio o detectado al sonar) |
| bitrate_kbps | INTEGER NULL | |
| logo_url | TEXT NULL | Favicon del directorio |
| logo_ruta | TEXT NULL | `~/.cache/mmmusic/logos/{id}.jpg` (300×300, mismo pipeline que carátulas) |
| radiobrowser_uuid | TEXT NULL UNIQUE | `stationuuid` si se añadió desde el directorio |
| favorita | INTEGER NOT NULL DEFAULT 0 | |
| anadida_en | TEXT NOT NULL | |
| ultima_reproduccion | TEXT NULL | |
| ultimo_error | TEXT NULL | Último fallo de conexión (texto seguro) |

**EMISORA_TITULOS**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| emisora_id | INTEGER FK → EMISORAS ON DELETE CASCADE | |
| titulo | TEXT NOT NULL | Título ICY tal cual |
| visto_en | TEXT NOT NULL | |

Índice `(emisora_id, visto_en)`; se conservan los 50 más recientes por emisora (poda al insertar).

**BUSQUEDAS_RADIO** (caché del directorio)

| Campo | Tipo | Descripción |
|-------|------|-------------|
| clave | TEXT PK | `tipo|nombre_norm|pais|etiqueta|orden` |
| respuesta_json | TEXT NOT NULL | Lista de emisoras del directorio (campos usados) |
| obtenido_en | TEXT NOT NULL | Caducidad 24 h; poda de entradas > 7 días |

### Tablas recreadas (migración 004, `CREATE … AS SELECT` + renombrado, sin perder datos)

**COLA**: `pista_id INTEGER NULL`, `tipo TEXT NOT NULL DEFAULT 'pista'`, `emisora_id INTEGER NULL FK → EMISORAS ON DELETE CASCADE`. Regla en código: exactamente uno de `pista_id`/`emisora_id` no nulo.

**HISTORIAL_REPRODUCCION**: `pista_id INTEGER NULL`, `emisora_id INTEGER NULL FK ON DELETE CASCADE`, `titulo_icy TEXT NULL`. Una fila por título ICY reconocido (no por conexión a la emisora).

**ENVIOS**: `pista_id INTEGER NULL`, `emisora_id INTEGER NULL FK ON DELETE CASCADE`. El scrobble de radio toma artista/título de `HISTORIAL_REPRODUCCION.titulo_icy` parseado y `album` = nombre de la emisora.

**AJUSTES** (claves nuevas): `radiobrowser_servidor` (espejo resuelto y fecha), `radio_pestana` (última pestaña).

### Diagrama de estados: Stream (elemento Emisora en reproducción)

```
   cargar(emisora)
        │
        ▼
   conectando ── datos llegan ──► almacenando ── paused-for-cache=false ──► en_directo
        │ error / timeout 15 s         │ error                                  │ end-file error /
        ▼                              ▼                                        │ eof inesperado
   reconectando(n=1) ◄─────────────────┴────────────────────────────────────────┘
        │ espera 1,2,4,8,16,30 s (n=1..6) → cargar de nuevo (siguiente espejo si la URL era PLS/M3U)
        │ n > 6
        ▼
   rendido ── toast "No se pudo conectar con <emisora>" → siguiente elemento de la cola (o detenido)
              EMISORAS.ultimo_error = motivo

   Transiciones inválidas: seek en cualquier estado (se ignora; no hay línea de tiempo);
   pausa en conectando/almacenando (se ignora hasta en_directo). `Espacio` en en_directo pausa
   (mpv mantiene la conexión; al reanudar se vuelve a almacenando y luego en_directo, sin
   reproducir el audio acumulado: `cache-pause` desactivado y `Reanudar` recarga si la pausa duró > 60 s).
```

---

## 4. Flujos de Trabajo

### 4.1 Reproducir una emisora

```
  Enter sobre una emisora (Favoritas/Todas/Buscar) ─► ReemplazarCola([Emisora(id)], 0)
        │  (a: añadir al final · A: a continuación, igual que con pistas)
        ▼
  Hilo reproductor: ¿la url termina en .pls/.m3u/.m3u8 o Content-Type de lista? ──sí──► mpv `loadlist url replace`
        │no                                                                              (entradas = espejos)
        ▼
  mpv `loadfile url replace` con `cache=yes`, `demuxer-max-bytes=32MiB`, `stream-lavf-o=reconnect…`
  Desactivar precarga gapless (F1) mientras el elemento actual es una emisora
        │
        ▼
  estado.stream = conectando ─► file-loaded ─► almacenando ─► paused-for-cache=false ─► en_directo
        │                                                          │
        │                                                          ▼
        │                                              observar `metadata` → icy-title
        │                                              observar `audio-codec-name`, `audio-bitrate`
        │                                              → EMISORAS.codec/bitrate si estaban vacíos
        ▼
  UPDATE EMISORAS.ultima_reproduccion; si vino del directorio → ComandoDirectorio::Click(uuid)
  (Radio Browser pide contar reproducciones; una vez por emisora y día)
        │
        ▼
  Barra inferior en modo directo: "● EN DIRECTO · 00:12:34" en vez de progreso; título ICY en
  la línea de título; nombre de la emisora donde iba "artista · álbum"; logo si existe.
```

### 4.2 Título ICY, historial y scrobbling

```
  `metadata/by-key/icy-title` cambia ─► icy::parsear(titulo, nombre_emisora)
        │
        ├─ vacío / igual al anterior / coincide con el nombre de la emisora / URL / "advert", "jingle"
        │  → se ignora (no se guarda)
        ├─ sin separador " - " → EMISORA_TITULOS.insert (poda a 50); titulo_icy en barra; SIN historial
        └─ "Artista - Título" → EMISORA_TITULOS.insert; HISTORIAL_REPRODUCCION.insert(emisora_id, titulo_icy,
                                completada=0); ComandoScrobbling::TituloIcy{ artista, titulo, emisora }
                                → now playing (album = emisora) en cada servicio activo
        │
        ▼
  Tick: si el mismo título lleva ≥ 30 s en directo (tiempo real, sin pausa) y no está marcado →
        completada=1 → ENVIOS(scrobble, historial_id, emisora_id, reproducido_en=instante del cambio)
        (Last.fm: sin `duration`; ListenBrainz: sin `duration_ms`; ambos con `album` = emisora)
        │
        ▼
  Cambio de título antes de 30 s → la fila de historial queda completada=0 y no se envía.
```

### 4.3 Búsqueda en Radio Browser

```
  Pestaña Buscar: campos Nombre · País · Etiqueta (Tab entre campos) · Enter
        │
        ▼
  clave = "search|"+norm(nombre)+"|"+pais+"|"+norm(etiqueta)+"|votes"
  ¿BUSQUEDAS_RADIO con clave y obtenido_en < 24 h? ──sí──► pintar resultados (marcar "caché")
        │no
        ▼
  ComandoDirectorio::Buscar(clave, params) → hilo directorio:
    ¿espejo en AJUSTES con < 24 h? ──no──► resolver DNS de all.api.radio-browser.info, elegir uno al azar,
    │                                      guardar en AJUSTES
    ▼
    GET https://<espejo>/json/stations/search?name=&country=&tag=&order=votes&reverse=true&limit=100&hidebroken=true
      User-Agent: mmmusic/<versión> (+https://codeberg.org/4d3/mmmusic)
        │
        ├─ 200 → guardar BUSQUEDAS_RADIO; AppEvento::ResultadosRadio(clave)
        ├─ error de red / 5xx → probar otro espejo (hasta 3) → toast "Radio Browser no disponible"
        └─ 429 → toast y no reintentar 5 min
        │
        ▼
  Resultados: nombre, país, etiquetas, codec/bitrate, votos, logo (descarga perezosa al hacerse visible)
  Enter → añade a EMISORAS (si no existe por uuid o url) y reproduce; `a` añade sin reproducir;
  `L` añade como favorita.
```

### 4.4 Alta manual, importación y exportación

```
  N (en Radio) → diálogo con Nombre y URL (validación: http/https, no vacía, única) → INSERT EMISORAS
  R → editar nombre/URL/página web
  D → eliminar con confirmación (cascada: títulos, cola, historial de esa emisora conserva la fila con
      emisora_id NULL? NO: se borra en cascada; se avisa en el diálogo)
  i → importar PLS o M3U/M3U8 local: cada entrada con URL http(s) → EMISORAS (nombre = Title/EXTINF o host);
      duplicadas por url se saltan; toast "12 emisoras añadidas, 2 ya existían, 1 sin URL válida"
  e → exportar favoritas a `Radio favoritas.m3u8` en carpeta_playlists (`#EXTINF:-1,<nombre>` + URL)
```

### 4.5 Diagrama de secuencia: caída del stream

```
  libmpv           Hilo Reproductor            Hilo UI               Usuario
     │ end-file (error) │                          │                     │
     │─────────────────►│ stream=reconectando(1)   │                     │
     │                  │──────────────────────────►│ barra: "⟳ reconectando (1/6)"
     │                  │ espera 1 s               │                     │
     │◄─ loadfile url ──│                          │                     │
     │ file-loaded      │                          │                     │
     │─────────────────►│ stream=almacenando       │                     │
     │ paused-for-cache=false                      │                     │
     │─────────────────►│ stream=en_directo, n=0   │                     │
     │                  │──────────────────────────►│ barra: "● EN DIRECTO"
```

---

## 5. Comandos, Atajos y API Interna

### 5.1 Atajos nuevos (sección Radio salvo indicación)

| Tecla | Contexto | Acción |
|-------|----------|--------|
| `8` | Global | Ir a Radio |
| `[` / `]` | Radio | Pestaña anterior / siguiente: Favoritas · Todas · Buscar · Sonando |
| `Enter` | Favoritas/Todas/Buscar | Reproducir la emisora (reemplaza la cola); en Buscar, además la añade a EMISORAS |
| `a` / `A` | Favoritas/Todas/Buscar | Añadir al final / a continuación de la cola |
| `L` | Radio, o global con una emisora sonando y sin pista seleccionada | Marcar / desmarcar favorita la emisora |
| `N` | Radio | Nueva emisora (nombre + URL) |
| `R` | Favoritas/Todas | Editar emisora |
| `D` | Favoritas/Todas | Eliminar emisora (confirmación) |
| `i` / `e` | Radio | Importar PLS/M3U · exportar favoritas a M3U8 |
| `/` | Radio | Ir a la pestaña Buscar y enfocar Nombre; `Tab` cambia de campo |
| `f` | Global con emisora sonando; Sonando | Buscar el título ICY actual (o el seleccionado) en la biblioteca: abre Buscar con el texto |
| `Espacio`, `n`, `p`, `x`, `+`/`-`, `m` | Global | Igual; `,` `.` `<` `>` se ignoran con emisora |

### 5.2 API interna

`ElementoCola`: `Pista(PistaResumen) | Emisora(EmisoraResumen)`. `ReemplazarCola`, `AnadirAlFinal`, `ReproducirSiguiente` aceptan `Vec<ElementoCola>`; el resto de comandos no cambian. Aleatorio y repetición tratan las emisoras como un elemento más (repetición `una` sobre una emisora = seguir en directo).

`EstadoReproduccion` (ampliado): `elemento: Option<ElementoCola>`, `stream: Option<EstadoStream>` (`Conectando | Almacenando { segundos: f32 } | EnDirecto | Reconectando(n) | Rendido`), `titulo_icy: Option<String>`, `tiempo_escuchando_ms`, `codec`, `bitrate_kbps`.

`ComandoScrobbling` (+): `TituloIcy { emisora_id, emisora_nombre, artista, titulo, historial_id, instante }`, `TituloIcyCompletado { historial_id }`.

`ComandoDirectorio` (UI → hilo directorio): `Buscar { clave, nombre, pais, etiqueta }`, `Click(uuid)`, `Logo { emisora_id, url }`, `Apagar`. Respuestas por `AppEvento`: `ResultadosRadio(clave)`, `LogoListo(emisora_id)`, `Notificacion`.

Consultas nuevas: `emisoras::{listar(favoritas_solo), buscar_local, crear, editar, eliminar, alternar_favorita, marcar_reproducida, fijar_codec, fijar_logo, por_uuid, por_url}`, `titulos_emisora::{insertar_y_podar, listar(emisora_id, 50)}`, `busquedas_radio::{leer(clave), guardar, podar}`, `cola::{cargar, guardar}` con tipo, `historial::registrar_icy`, `envios::encolar` con `emisora_id`.

### 5.3 APIs externas

| Servicio | Petición | Uso |
|----------|----------|-----|
| Radio Browser | DNS `all.api.radio-browser.info` → espejos | Elección de servidor (24 h) |
| Radio Browser | `GET /json/stations/search?name&country&tag&order=votes&reverse=true&limit=100&hidebroken=true` | Búsqueda |
| Radio Browser | `GET /json/url/{stationuuid}` | Contador de reproducciones (una vez por emisora y día) |
| Logos | `GET <logo_url>` (cualquier host https, solo `image/*`, ≤ 512 KB, 5 s, ≤ 3 redirecciones) | Logo de emisora |
| Streams | Los abre mpv | Fuera del cliente HTTP de mmmusic |

---

## 6. Interfaz de Usuario

### Mapa de navegación (cambios)

```
mmmusic
├── [8] Radio
│     ├── Favoritas   (lista: logo, nombre, país, codec/bitrate, última reproducción)
│     ├── Todas       (misma lista, todas las EMISORAS, orden por nombre o última reproducción)
│     ├── Buscar      (campos Nombre/País/Etiqueta + resultados del directorio, con "caché"/"buscando…")
│     └── Sonando     (emisora actual: datos + últimos 50 títulos; f busca en biblioteca)
├── Cola: elementos de tipo emisora con icono ◉ y sin duración
└── Barra inferior en modo directo
```

### Mockup 1: Radio · Favoritas

```
┌ mmmusic ─────────┬──────────────────────────────────────────────────────────┐
│  1 Inicio        │ Radio   [Favoritas]  Todas  Buscar  Sonando      [ ] pestaña│
│  2 Buscar        │──────────────────────────────────────────────────────────│
│  3 Artistas      │  ▀▀ ♥ Nightwave Plaza           UA  MP3 128  hace 2 h    │
│  4 Álbumes       │  ▀▀ ♥ SomaFM Vaporwaves         US  AAC 128  ayer        │
│  5 Pistas        │▶ ▀▀ ♥ Radio Paradise Mellow     US  FLAC     ahora       │
│  6 Playlists     │  ▀▀ ♥ Lofi Girl                 FR  MP3 192  —           │
│  7 Visual        │  ▀▀ ♥ Ràdio 4                   ES  AAC  96  hace 3 d    │
│  8 Radio       ◄ │                                                          │
│                  │                                                          │
│  Playlists       │                                                          │
│   ♥ Favoritas    │                                                          │
│   Vaporwave      │                                                          │
│                  │                                                          │
│ 4.812 pistas     │ Enter escuchar · a cola · L favorita · N nueva · i importar│
├──────────────────┴──────────────────────────────────────────────────────────┤
│ ▀▀ ● Radio Paradise Mellow   ▂▄▆█▅▃▂▁▂▃▂▁  ⏮  ⏸  ⏭      🔀 🔁 ♪ vol 80 %  ↑ │
│ ▄▄ Boards of Canada - Dayvan Cowboy     EN DIRECTO · 00:12:34 · FLAC        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Mockup 2: Radio · Buscar

```
┌ mmmusic ─────────┬──────────────────────────────────────────────────────────┐
│  8 Radio       ◄ │ Radio    Favoritas  Todas  [Buscar]  Sonando             │
│                  │ Nombre: vaporwave_        País: __    Etiqueta: ________ │
│                  │──────────────────────────────────────────────────────────│
│                  │  ▀▀ Nightwave Plaza              UA  MP3 128   ★ 4.213   │
│                  │  ▀▀ SomaFM Vaporwaves            US  AAC 128   ★ 2.877   │
│                  │  ▀▀ Vaporwave Radio 24/7         DE  MP3 128   ★   931   │
│                  │  ♪  Mallsoft FM                  —   OGG  96   ★   412   │
│                  │  …                                  (caché, hace 3 h)    │
│                  │                                                          │
│                  │ Enter escuchar y guardar · a guardar · L favorita · Tab campo│
```

### Mockup 3: Radio · Sonando

```
│ Radio    Favoritas  Todas  Buscar  [Sonando]                                │
│ ▀▀▀▀  Radio Paradise Mellow · US · FLAC · radioparadise.com                │
│ ▄▄▄▄  ● EN DIRECTO 00:12:34 · reconexiones: 0 · caché 8,2 s               │
│                                                                             │
│ Últimos títulos                                                             │
│ ▶ 13:04  Boards of Canada - Dayvan Cowboy                       f buscar     │
│   13:00  Tycho - Awake                                          (en biblioteca ✓)│
│   12:55  Bonobo - Kerala                                                     │
│   12:51  Radio Paradise station ID                              (ignorado)   │
```

### Mockup 4: barra inferior en estados de conexión

```
│ ▀▀ ⟳ Nightwave Plaza          ▂▁▁▁▁▁▁▁▁▁▁▁  ⏮  ⏸  ⏭      🔀 🔁 ♪ vol 80 %  ↑ │
│ ▄▄ conectando…                          ────────────────────────────────     │

│ ▀▀ ⟳ Nightwave Plaza                                                          │
│ ▄▄ reconectando (3/6)…                  ────────────────────────────────     │

│ ▀▀ ◌ Nightwave Plaza                                                          │
│ ▄▄ almacenando 2,4 s                    ━━━━━━━━────────────────────────     │
```

### Notas de UX

- En la cola, las emisoras muestran `◉` en lugar de duración; la línea de progreso se sustituye por el estado de conexión y el tiempo escuchando.
- Ninguna acción de radio bloquea: buscar muestra "buscando…" y los logos aparecen cuando llegan (marcador `♪` mientras tanto, como las carátulas).
- Modo `ascii`: `●`→`*`, `⟳`→`~`, `◌`→`o`, `◉`→`(R)`.
- Los títulos ICY se muestran tal cual llegan (sin normalizar); en Sonando se marca `✓` si el título parseado existe en la biblioteca (búsqueda por `*_norm` exacta de artista y título).
- MPRIS con emisora: `xesam:title` = título ICY (o nombre de la emisora), `xesam:artist` = artista parseado, `xesam:album` = emisora, sin `mpris:length`, `CanSeek=false`, `mpris:artUrl` = logo si existe.

---

## 7. Lógica de Negocio

### 7.1 Parseo de títulos ICY

```python
import re

RUIDO = ("advert", "advertisement", "jingle", "station id", "commercial", "http://", "https://")

def parsear_icy(titulo: str, nombre_emisora: str):
    t = " ".join(titulo.split()).strip()
    if not t or normalizar(t) == normalizar(nombre_emisora): return None
    if any(r in t.lower() for r in RUIDO): return None
    if " - " not in t: return {"titulo": t, "artista": None}          # se guarda, no se scrobblea
    artista, cancion = t.split(" - ", 1)
    artista, cancion = artista.strip(), cancion.strip()
    if not artista or not cancion: return {"titulo": t, "artista": None}
    return {"titulo": cancion, "artista": artista}
```

- Se conserva el título original en `EMISORA_TITULOS.titulo` e `HISTORIAL_REPRODUCCION.titulo_icy`.
- Un título repetido consecutivo (la emisora reenvía el mismo) no crea filas nuevas.

### 7.2 Umbral de scrobble en radio

```python
def debe_scrobblear_icy(segundos_en_directo_con_este_titulo: float) -> bool:
    return segundos_en_directo_con_este_titulo >= 30.0
```

Solo cuenta el tiempo en estado `en_directo` (no conectando, almacenando ni pausa). `completada` se marca una sola vez por fila. Sin duración conocida, los servicios aceptan el scrobble sin `duration`.

### 7.3 Política de reconexión

```python
ESPERAS = [1, 2, 4, 8, 16, 30]   # segundos, intentos 1..6

def siguiente_espera(intento: int):
    return ESPERAS[intento - 1] if intento <= len(ESPERAS) else None   # None → rendido
```

- Cada vez que se alcanza `en_directo` el contador vuelve a 0.
- Si la URL era una lista (PLS/M3U) con varias entradas, cada reintento avanza al siguiente espejo antes de volver al primero.
- `rendido` guarda `EMISORAS.ultimo_error` (código HTTP o texto de mpv, sin URL con credenciales si las hubiera) y pasa al siguiente elemento de la cola (o `detenido`).
- Los tres fallos seguidos de F1 (pistas) no aplican a emisoras: el conteo de F1 se reinicia al cambiar de tipo de elemento.

### 7.4 Cola mixta

- `Anterior` sobre una emisora va siempre al elemento anterior (no hay "reiniciar pista").
- `Siguiente automático` desde una emisora solo ocurre por `rendido`; una emisora no "termina".
- Al restaurar la cola en pausa al arrancar (DT-9), si el elemento actual es una emisora se restaura **sin conectar** (estado detenido sobre la emisora seleccionada); `Espacio` conecta.
- Precarga gapless desactivada mientras el elemento actual o el siguiente es una emisora.

### 7.5 Radio Browser

- Espejo: resolución DNS de `all.api.radio-browser.info`, elección aleatoria, guardado 24 h; ante fallo, siguiente espejo (máximo 3 por búsqueda).
- Caché por clave 24 h; resultados con `lastcheckok = 0` se descartan (además de `hidebroken=true`).
- `Click` se envía una vez por emisora y día natural (registro en memoria + `ultima_reproduccion`).
- Al añadir desde el directorio: `url_resolved` si existe, si no `url`; nombre, país (`countrycode`), etiquetas (`tags`), codec, bitrate, favicon, homepage, uuid.
- Búsqueda local (pestaña Todas, `/` con texto) por `nombre_norm LIKE`.

### 7.6 Validaciones

- URL de emisora: esquema `http` o `https`, host no vacío, longitud ≤ 2048, única (normalizada sin barra final).
- Nombre: 1–100 caracteres, no vacío; si viene vacío al importar se usa el host.
- Logos: se descartan si no son `image/*`, superan 512 KB, o `image` no los decodifica; nunca se reintenta más de una vez por sesión.
- Los códigos de país se muestran en mayúsculas; desconocido → `—`.

### 7.7 Casos especiales

- HLS (`.m3u8` de segmentos, no lista de emisoras): mpv lo distingue; si `loadlist` falla por no ser lista de emisoras, se reintenta con `loadfile`.
- Emisora que emite títulos solo con el nombre de la canción (sin artista): aparece en Sonando y en la barra, no scrobblea.
- Emisora eliminada mientras suena: se detiene y se quita de la cola.
- Sin red al pulsar Buscar: si hay caché se muestra con aviso "resultados en caché (sin conexión)"; si no, toast.
- Visuales (F3) y mini espectro funcionan igual: la captura no distingue la fuente.

---

## 8. Requisitos No Funcionales

### Seguridad

- **T-12 modificada:** el cliente HTTP de mmmusic (`red/cliente.rs`) admite `api.listenbrainz.org`, `ws.audioscrobbler.com` y los espejos resueltos de `all.api.radio-browser.info`; además, exclusivamente para logos, descargas de cualquier host https limitadas a `image/*`, 512 KB, 5 s y 3 redirecciones, tratadas como no confiables (decodificación en el hilo auxiliar, error ignorado).
- Los streams de audio los abre mpv con las opciones de F1 más las de caché/reconexión; `ytdl=no` y `load-scripts=no` siguen activos. mpv no ejecuta nada del contenido.
- URLs y nombres de emisora procedentes del directorio se tratan como texto: se limpian caracteres de control antes de mostrarlos.
- `User-Agent` identificativo obligatorio en Radio Browser (política del servicio).

### Protección de datos

- Con scrobbling activo, los títulos ICY reconocidos se envían a los servicios configurados igual que las pistas locales (mismo opt-in de F2).
- Radio Browser recibe las búsquedas y el contador de reproducciones (uuid de emisora); no recibe identificadores del usuario más allá del User-Agent y la IP.

### Backups y recuperación

- Migración 004 recrea COLA, HISTORIAL_REPRODUCCION y ENVIOS preservando filas; se ejecuta dentro de una transacción y con copia previa `mmmusic.db.pre-004` que se borra en el siguiente arranque correcto.
- EMISORAS es la única tabla nueva no regenerable: exportación de favoritas a M3U8 como copia manual. Logos y caché de búsquedas son regenerables.

### Rendimiento

- Conexión a una emisora: audio en < 3 s con red normal; indicador de estado en < 100 ms.
- Búsqueda en el directorio: respuesta pintada en < 2 s; desde caché instantánea.
- Historial de títulos acotado a 50 por emisora; caché de búsquedas podada a 7 días.

### Accesibilidad

- Estado de conexión siempre con texto, no solo icono.
- Todas las acciones por teclado; formularios con `Tab` entre campos.
- Modo `ascii` completo para los iconos nuevos.

---

## 9. Integraciones

| Integración | Detalle |
|-------------|---------|
| mpv | Opciones de stream; propiedades `metadata` (icy-title), `paused-for-cache`, `cache-buffering-state`, `demuxer-cache-duration`, `audio-codec-name`, `audio-bitrate`; `loadlist` para PLS/M3U remotos |
| Radio Browser | Búsqueda, click; User-Agent `mmmusic/<versión> (+https://codeberg.org/4d3/mmmusic)` |
| PLS / M3U / M3U8 | Importación local y exportación de favoritas (§4.4) |
| Scrobbling (F2) | Escuchas ICY con álbum = emisora, sin duración |
| MPRIS (F1) | Metadatos de radio, `CanSeek=false` |
| Carátulas (F1) | Pipeline reutilizado para logos (`~/.cache/mmmusic/logos/`) |

`config.toml`:

```toml
[radio]
directorio = true            # false: sin Radio Browser (pestaña Buscar solo busca en emisoras locales)
logos = true
espera_conexion_s = 15
```

---

## 10. Decisiones Técnicas (ADR-lite)

**DT-36 — Cola mixta `ElementoCola` frente a "modo radio" separado.** Una única cola mantiene un solo modelo mental, MPRIS y persistencia; el coste es recrear COLA/HISTORIAL/ENVIOS con `pista_id` nulo, previsto desde F1.

**DT-37 — mpv abre los streams; el cliente HTTP propio no.** Mantiene T-12 (lista cerrada) para el código de mmmusic y delega a mpv/ffmpeg la reconexión a bajo nivel; la política de reintentos de alto nivel vive en `reconexion.rs`, testeable sin red.

**DT-38 — Radio Browser como único directorio.** Es abierto, sin clave y con espejos; alternativas (Shoutcast, TuneIn) requieren claves o scraping. Se respeta su etiqueta: User-Agent, `hidebroken`, contador de clicks.

**DT-39 — Logos como excepción acotada a la lista cerrada de hosts.** Sin ella no hay logos (los favicons viven en el host de cada emisora). Los límites (solo imagen, tamaño, tiempo, redirecciones, decodificación aislada) reducen la superficie a algo comparable a abrir una carátula de un fichero descargado.

**DT-40 — Historial por título ICY, no por conexión.** Lo que se escucha son canciones; conectar con una emisora no es una "escucha". Los títulos sin artista se guardan en EMISORA_TITULOS pero no en historial.

**DT-41 — Recreación de tablas en la migración 004 con copia previa.** SQLite no permite quitar `NOT NULL`; se recrea con `CREATE TABLE nueva … INSERT … SELECT … DROP … RENAME` en transacción, con copia de la BD antes por si la migración falla a medias.

**DT-42 — Hilo de directorio propio.** Las búsquedas son interactivas y no deben competir con la cola de envíos del hilo de scrobbling; ambos comparten `red/cliente.rs` (constructor, User-Agent, hosts).

---

## 11. Plan de Desarrollo

| Sprint | Contenido | Entregable verificable |
|--------|-----------|------------------------|
| S1 — Modelo y reproducción | Migración 004 con copia previa; `ElementoCola`; EMISORAS/EMISORA_TITULOS; opciones de stream en mpv; estados de stream; reconexión; barra inferior en modo directo; alta manual `N` y sección 8 con pestañas Favoritas/Todas | Escuchar una URL añadida a mano con reconexión al cortar la red |
| S2 — ICY, títulos y gestión | Parseo ICY, EMISORA_TITULOS, pestaña Sonando, `f`, `L`/`R`/`D`, importación/exportación PLS/M3U, `[`/`]`, MPRIS de radio | Títulos en barra y Sonando; favoritas y M3U funcionando |
| S3 — Radio Browser | `red/cliente.rs` compartido, hilo de directorio, espejos, búsqueda con caché, click, logos limitados, pestaña Buscar | Buscar "vaporwave" y escuchar desde el resultado con logo |
| S4 — Scrobbling y cierre | Historial/ENVIOS de radio, now playing y scrobble ICY (30 s), regla de F1 de tres fallos aislada por tipo, tests (icy, listas, reconexión, cola mixta, caché), README, config, ayuda, clippy/fmt | Escucha de radio visible en ListenBrainz/Last.fm y suite verde |

**Estimación:** 4 sprints de ~1 semana → 3–4 semanas nominales (1–2 sesiones al ritmo real). Supuestos: los anteriores; el agente puede probar streams públicos y Radio Browser desde su entorno si tiene red (si no, tests sin red y validación de Hector).

---

## 12. Conexiones con Otras Fases

- **Desde F1:** cola, MPRIS, barra inferior, carátulas (logos), diálogos; regla de tres fallos ahora aislada por tipo de elemento.
- **Desde F2:** hilo de scrobbling y ENVIOS (escuchas ICY); `red/cliente.rs` se extrae del código de F2 sin cambiar su comportamiento.
- **Desde F3:** las visuales funcionan con radio sin cambios.
- **Hacia F4 (ecualizador y letras, pendiente):** el ecualizador aplica a streams; las letras no aplican a radio.
- **Hacia F6 (Nicotine+):** `f` (buscar título ICY en biblioteca) podría ampliarse a "buscar en Soulseek".
- **Ideas:** podcasts (RSS), grabación de streams, emisoras en Inicio, importar favoritas desde Radio Browser por cuenta.
