# mmmusic - Fase 2: Scrobbling, Recopilatorios y Favoritas

## Especificación Funcional

**Versión:** 1.0
**Fecha:** 12 de septiembre de 2026
**Cliente:** Proyecto personal (Hector) — sin cliente externo

---

## 1. Visión General

La Fase 2 conecta mmmusic con los servicios de registro de escuchas (ListenBrainz y Last.fm), corrige la agrupación de recopilatorios sin `albumartist` heredada de la Fase 1, añade el concepto de pista favorita y lleva las carátulas a la vista Inicio. Es la primera fase con tráfico de red saliente, siempre opcional y desactivado por defecto.

### Objetivos principales

1. Enviar "now playing" y scrobbles a ListenBrainz y Last.fm de forma independiente, con cola persistente y reintentos cuando no hay red.
2. Autorizar Last.fm sin exponer credenciales: subcomando de CLI que guía el flujo de autorización y guarda la sesión en un fichero aparte con permisos 600.
3. Agrupar los recopilatorios bajo "Varios artistas" cuando falta `albumartist` y las pistas de una misma carpeta y álbum tienen artistas distintos, aplicándolo a la biblioteca existente con un reescaneo completo único.
4. Marcar pistas como favoritas con `L`, verlas como pseudo-playlist "♥ Favoritas" y sincronizar el "love" con Last.fm.
5. Mostrar carátulas en los tres bloques de Inicio con la misma rejilla de tarjetas que Álbumes.
6. Informar del estado del scrobbling en la barra inferior sin ruido: un icono para enviado / pendiente / error.

### Contexto

Fase 1 completada y validada en el Omarchy real del usuario (112/112). Esta fase se apoya en HISTORIAL_REPRODUCCION y en la regla de tiempo real acumulado (T-10), en el escáner incremental (T-8) y en el componente de rejilla con carátulas (T-9). Sigue sin haber cliente externo, por lo que "API Endpoints" se sustituye por comandos, atajos, API interna y las APIs externas consumidas.

---

## 2. Arquitectura Técnica

### Stack tecnológico (cambios respecto a la Fase 1)

| Capa | Tecnología | Motivo |
|------|-----------|--------|
| HTTP | `ureq` (rustls) en hilo dedicado | Síncrono, ligero, sin runtime async global (T-2); solo dos hosts permitidos |
| Firma Last.fm | `md5` | Firma de peticiones autenticadas (`api_sig`) |
| CLI | `clap` (derive) | Subcomandos `autorizar-lastfm`, `probar-servicios`, `reescanear --completo` |
| Credenciales | `serde` + `toml` | `~/.config/mmmusic/credenciales.toml`, permisos 600 comprobados al arrancar |
| Resto | Sin cambios | Rust 2024, ratatui, libmpv2, rusqlite, lofty, ratatui-image, mpris-server |

### Estructura de carpetas (añadidos y cambios)

```
mmmusic/
├── docs/
│   ├── mmmusic-fase2-informe.md
│   ├── mmmusic-fase2-checklist.md
│   ├── mmmusic-fase2-prompt.md
│   └── mmmusic-fase2-implementacion.md      (lo escribe el agente)
├── credenciales.ejemplo.toml
├── src/
│   ├── cli.rs                   # clap: subcomandos y arranque de la TUI por defecto
│   ├── credenciales.rs          # carga/guardado de credenciales.toml, comprobación de permisos
│   ├── biblioteca/
│   │   ├── migraciones/
│   │   │   └── 002_recopilatorios_envios.sql
│   │   ├── recopilatorios.rs    # pasada de consolidación "Varios artistas"
│   │   ├── consultas.rs         # + envios::*, favoritas::*, inicio() con carátulas
│   │   └── escaner.rs           # + carpeta, artista_album_etiquetado, reescaneo completo forzado
│   ├── scrobbling/
│   │   ├── mod.rs               # hilo de envío, planificador de reintentos, ComandoScrobbling
│   │   ├── regla.rs             # elegibilidad y umbral (≥50 % o ≥4 min, pista > 30 s)
│   │   ├── listenbrainz.rs      # submit-listens (playing_now / single / import), validate-token
│   │   ├── lastfm.rs            # auth.getToken/getSession, updateNowPlaying, scrobble, love/unlove, firma
│   │   └── estado.rs            # EstadoScrobbling (para barra inferior)
│   └── ui/
│       ├── barra_inferior.rs    # + indicador de scrobbling y ♥
│       └── vistas/
│           ├── inicio.rs        # rejillas de tarjetas por bloque
│           └── playlists.rs     # + pseudo-playlist "♥ Favoritas"
└── tests/
    ├── recopilatorios.rs
    ├── scrobbling.rs            # regla, planificador, firma Last.fm, cuerpo JSON de ListenBrainz
    └── fixtures/recopilatorio/  # carpeta con 3 pistas de artistas distintos sin albumartist
```

### Diagrama de arquitectura (nuevo hilo de envío)

```
┌───────────────────────────────────────────────────────────────────────────┐
│                              Proceso mmmusic                              │
│  ┌──────────────┐ AppEvento ┌────────────────┐  watch<Estado>            │
│  │ Hilo UI      │◄──────────┤ Hilo Reproduct.│──────────┐                │
│  │              │           │ (F1)           │          │                │
│  └──────┬───────┘           └───────┬────────┘          │                │
│         │ ComandoScrobbling         │ file-loaded /      │                │
│         │ (Amar, Probar, Reintentar)│ completada=1       │                │
│         ▼                           ▼                    ▼                │
│  ┌────────────────────────────────────────────────┐ ┌──────────────┐      │
│  │ Hilo Scrobbling                                │ │ Hilo MPRIS   │      │
│  │  - now playing inmediato (en memoria)          │ │ (F1)         │      │
│  │  - encola ENVIOS al completar / amar           │ └──────────────┘      │
│  │  - cada 30 s o al despertar: lote de pendientes│                       │
│  │  - reintentos exponenciales, descarte          │                       │
│  │  - publica EstadoScrobbling → barra inferior   │                       │
│  └───────┬──────────────────────┬─────────────────┘                       │
│          │ ureq (TLS, 10 s)     │ ureq                                    │
└──────────┼──────────────────────┼─────────────────────────────────────────┘
           ▼                      ▼
   api.listenbrainz.org    ws.audioscrobbler.com          SQLite: ENVIOS, FAVORITAS
                                                          (conexión propia del hilo)
```

Principios: el hilo de scrobbling es el único que hace red; nunca bloquea al reproductor ni a la UI; cada petición tiene timeout de 10 s; los tokens no aparecen jamás en logs ni en toasts.

---

## 3. Modelo de Datos

### Diagrama E-R (nuevo y modificado)

```
┌──────────────────┐        ┌────────────────────────┐        ┌────────────────────────────┐
│ ALBUMES (mod.)   │        │ HISTORIAL_REPRODUCCION │1      *│ ENVIOS (nueva)             │
│──────────────────│        │ (F1, sin cambios)      │────────│────────────────────────────│
│ + varios_artistas│        └────────────────────────┘        │ id                         │
│ + carpeta        │                                          │ servicio                   │
└──────────────────┘        ┌────────────────────────┐        │ tipo                       │
                            │ PISTAS (mod.)          │1      *│ pista_id                   │
┌──────────────────┐        │────────────────────────│────────│ historial_id (NULL en love)│
│ FAVORITAS (nueva)│*      1│ + carpeta              │        │ reproducido_en (NULL love) │
│──────────────────│────────│ + artista_album_etiq.  │        │ estado                     │
│ id               │        └────────────────────────┘        │ intentos                   │
│ pista_id (único) │                                          │ proximo_intento_en         │
│ marcada_en       │                                          │ error_msg                  │
└──────────────────┘                                          │ creado_en / enviado_en     │
                                                              └────────────────────────────┘
```

### Cambios en tablas existentes (migración 002)

**ALBUMES**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| varios_artistas | INTEGER NOT NULL DEFAULT 0 | 1 si es un recopilatorio agrupado bajo "Varios artistas" |
| carpeta | TEXT NULL | Carpeta del recopilatorio; solo se rellena cuando `varios_artistas = 1` |

Índices únicos parciales (sustituyen al índice `(artista_id, titulo_norm)` de F1):
- `(artista_id, titulo_norm) WHERE varios_artistas = 0`
- `(titulo_norm, carpeta) WHERE varios_artistas = 1`

**PISTAS**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| carpeta | TEXT NOT NULL | Directorio contenedor (dirname de `ruta`); índice |
| artista_album_etiquetado | INTEGER NOT NULL DEFAULT 0 | 1 si el fichero traía etiqueta `albumartist` |

**AJUSTES** (claves nuevas): `reescaneo_completo_pendiente` (0/1, lo pone a 1 la migración 002), `scrobbling_ultimo_error` (texto, para mostrar en el indicador tras reiniciar).

### Tablas nuevas

**ENVIOS** (cola de envíos a servicios externos)

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| servicio | TEXT NOT NULL | `listenbrainz`, `lastfm` |
| tipo | TEXT NOT NULL | `scrobble`, `love`, `unlove` |
| pista_id | INTEGER FK → PISTAS ON DELETE CASCADE | |
| historial_id | INTEGER FK → HISTORIAL_REPRODUCCION NULL | Solo en `scrobble` |
| reproducido_en | TEXT NULL | Instante de inicio de la escucha (solo `scrobble`) |
| estado | TEXT NOT NULL | `pendiente`, `enviado`, `error`, `descartado` |
| intentos | INTEGER NOT NULL DEFAULT 0 | |
| proximo_intento_en | TEXT NOT NULL | ISO 8601; al crear = ahora |
| error_msg | TEXT NULL | Último error (sin credenciales) |
| creado_en | TEXT NOT NULL | |
| enviado_en | TEXT NULL | |

Índices: `(estado, proximo_intento_en)`, `(servicio, tipo, pista_id)`. Un `love`/`unlove` nuevo para la misma pista y servicio reemplaza al pendiente anterior (se descarta el viejo).

**FAVORITAS**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| id | INTEGER PK | |
| pista_id | INTEGER FK → PISTAS ON DELETE CASCADE UNIQUE | |
| marcada_en | TEXT NOT NULL | |

### Fichero de credenciales (`~/.config/mmmusic/credenciales.toml`, no es tabla)

```toml
[listenbrainz]
token = "…"                 # token de usuario (Settings → User token)

[lastfm]
api_key = "…"               # de https://www.last.fm/api/account/create
api_secret = "…"
session_key = "…"           # lo escribe `mmmusic autorizar-lastfm`
usuario = "…"               # lo escribe `mmmusic autorizar-lastfm`
```

### Diagrama de estados: ENVIO

```
   encolar()
  ──────────► pendiente ──── envío OK ───────────────► enviado
                 ▲ │
                 │ │ fallo de red / 5xx / timeout
                 │ ▼
                 │ error ── intentos ≤ 20 ─► pendiente (proximo_intento_en = ahora + min(30 s·2^intentos, 6 h))
                 │   │
                 │   └── intentos > 20 ───────────────► descartado ("agotados los reintentos")
                 │
                 └── 4xx de autenticación → error con proximo_intento_en = +24 h y aviso en UI
                     (se reintenta antes si cambian las credenciales o el usuario ejecuta `probar-servicios`)

   Descarte directo desde pendiente:
   - lastfm y reproducido_en con más de 14 días (Last.fm lo rechazaría) → descartado
   - la pista se borra de PISTAS → fila eliminada en cascada
   - llega un love/unlove posterior para la misma pista y servicio → el anterior pasa a descartado

Transiciones inválidas: enviado y descartado son finales; error nunca pasa a enviado sin volver por pendiente.
```

---

## 4. Flujos de Trabajo

### 4.1 Ciclo de vida de una escucha

```
  file-loaded (F1) ─► HISTORIAL_REPRODUCCION.insert ─► hilo scrobbling: ¿elegible?
                                                          │ (duracion_ms > 30.000 y servicio activo)
                                                          ├─ no → nada
                                                          └─ sí → now playing a cada servicio activo (sin cola,
                                                                  1 intento, error solo a log)
  tick (F1, tiempo real acumulado, T-10)
        │
        ▼
  tiempo_real ≥ min(duracion/2, 240 s)  →  completada = 1 (una sola vez; sustituye a la regla "50 %" de F1)
        │
        ▼
  INSERT ENVIOS(tipo=scrobble, servicio=cada uno activo, reproducido_en=inicio, estado=pendiente)
        │
        ▼
  Hilo scrobbling se despierta (canal) o cada 30 s:
    SELECT pendientes WHERE proximo_intento_en <= ahora ORDER BY reproducido_en LIMIT 50 por servicio
        │
        ├─ listenbrainz: POST /1/submit-listens { listen_type: "import", payload: [...] }
        ├─ lastfm:       POST track.scrobble con timestamp[i], artist[i], track[i], album[i], duration[i], api_sig
        │
        ├─ 200 → UPDATE estado=enviado, enviado_en; EstadoScrobbling ← "enviado"
        ├─ red / 5xx / timeout → estado=error, intentos++, proximo_intento_en; EstadoScrobbling ← "pendiente N"
        └─ 401/403 (token inválido, sesión revocada) → estado=error, +24 h, AJUSTES.scrobbling_ultimo_error,
                                                        toast "Last.fm: sesión no válida — ejecuta mmmusic autorizar-lastfm"
```

### 4.2 Autorización de Last.fm (CLI)

```
  $ mmmusic autorizar-lastfm
        │
        ▼
  Lee credenciales.toml → ¿api_key y api_secret? ──no──► "Rellena api_key y api_secret en <ruta>" → exit 1
        │sí
        ▼
  GET auth.getToken (firmado) → token
        │
        ▼
  Imprime: "Abre esta URL y autoriza mmmusic: https://www.last.fm/api/auth/?api_key=…&token=…"
  Intenta abrir con xdg-open (si falla, solo imprime)
  "Pulsa Enter cuando hayas autorizado…"
        │
        ▼
  GET auth.getSession(token, firmado) ──error 14 (no autorizado)──► "Aún no autorizado, vuelve a pulsar Enter" (hasta 3 veces)
        │OK
        ▼
  Escribe session_key y usuario en credenciales.toml (chmod 600) → "Last.fm autorizado como <usuario>" → exit 0
```

### 4.3 Consolidación de recopilatorios (pasada posterior al escaneo)

```
  Fin de la pasada principal del escaneo (F1) ─► recopilatorios::consolidar(escaneo_id)
        │
        ▼
  Grupos candidatos = (carpeta, titulo_norm del álbum) de las pistas nuevas/actualizadas en este escaneo
  (o de TODAS las pistas si el escaneo es completo forzado)
        │
        ▼
  Para cada grupo:
    todas las pistas con artista_album_etiquetado = 0
    y COUNT(DISTINCT artista_id) > 1
        │
        ├─ sí → obtener/crear ARTISTA "Varios artistas"
        │       obtener/crear ALBUM(varios_artistas=1, titulo, carpeta, artista_id=VA, anio=min(anio))
        │       UPDATE PISTAS SET album_id=VA_album WHERE carpeta=? AND album titulo_norm=?
        │       carátula: reutilizar la del álbum anterior si existe o extraer de nuevo
        │
        └─ no → si alguna pista del grupo cuelga de un álbum varios_artistas=1 (ya no aplica):
                reasignar cada pista a (artista_id, titulo_norm) normal (crear álbum si falta)
        │
        ▼
  Limpieza de álbumes sin pistas y artistas sin álbumes ni pistas (F1)
```

### 4.4 Reescaneo completo forzado

```
  Arranque ─► migración 002 pone AJUSTES.reescaneo_completo_pendiente = 1 (solo la primera vez)
        │
        ▼
  ¿reescaneo_completo_pendiente = 1? ──sí──► toast "Actualizando la biblioteca para agrupar recopilatorios…"
        │                                    escaneo con modo=completo: ignora mtime/tamaño, relee todas las etiquetas
        │                                    (rellena carpeta y artista_album_etiquetado), consolida, y al
        │                                    completarse pone el ajuste a 0
        │                                    Si el escaneo termina en error o cancelado, el ajuste sigue a 1 y
        │                                    se reintenta en el siguiente arranque
        └──no──► escaneo incremental normal (F1)

  También: `mmmusic reescanear --completo` (CLI, sin TUI, imprime progreso y resumen) y
  Ctrl+r sigue siendo incremental.

  Las playlists, la cola, el historial y las favoritas sobreviven: PISTAS se actualiza por `ruta`,
  nunca se borra y reinserta.
```

### 4.5 Marcar favorita

```
  `L` sobre una pista seleccionada (o sin selección: la que suena) ─► favoritas::alternar(pista_id)
        │
        ├─ no era favorita → INSERT FAVORITAS; si lastfm activo → ENVIOS(tipo=love)
        └─ era favorita    → DELETE FAVORITAS;  si lastfm activo → ENVIOS(tipo=unlove)
        │
        ▼
  Toast "♥ <título>" / "Quitada de favoritas"; barra inferior muestra ♥ junto al título si la actual es favorita
  ListenBrainz: sin sincronización (DT-16)
```

### 4.6 Diagrama de secuencia: scrobble con caída de red

```
  Hilo Reproductor    Hilo Scrobbling        SQLite (ENVIOS)     api.listenbrainz.org     Hilo UI
       │ completada=1      │                       │                     │                   │
       │──────────────────►│ INSERT pendiente ×2   │                     │                   │
       │                   │──────────────────────►│                     │                   │
       │                   │ SELECT pendientes     │                     │                   │
       │                   │◄──────────────────────│                     │                   │
       │                   │ POST submit-listens ─────────────────────► X (sin red, timeout 10 s)
       │                   │ UPDATE error, intentos=1, +60 s            │                   │
       │                   │──────────────────────►│                     │                   │
       │                   │ EstadoScrobbling{pendientes:2} ───────────────────────────────►│ icono "…2"
       │        (60 s después, o al recuperar red y despertar por reintento)                │
       │                   │ POST submit-listens ─────────────────────► 200                │
       │                   │ UPDATE enviado ×2     │                     │                   │
       │                   │ EstadoScrobbling{pendientes:0} ───────────────────────────────►│ icono "↑"
```

---

## 5. Comandos, Atajos y API Interna

### 5.1 Atajos nuevos

| Tecla | Acción |
|-------|--------|
| `L` | Marcar / desmarcar favorita la pista seleccionada; sin selección de pista, la que suena |
| `Ctrl+s` | Forzar ahora el envío de pendientes (despierta el hilo de scrobbling) |

La pseudo-playlist "♥ Favoritas" aparece la primera en la sección Playlists; no admite `R`, `D`, `J`/`K` ni `d` (se quita con `L`); sí admite `Enter`, `a`, `A`, `P` y `e` (exportar M3U8).

### 5.2 Subcomandos de CLI

| Comando | Descripción | Salida |
|---------|-------------|--------|
| `mmmusic` | Arranca la TUI (comportamiento de F1) | |
| `mmmusic autorizar-lastfm` | Flujo §4.2 | Código 0 si guarda la sesión |
| `mmmusic probar-servicios` | `validate-token` en ListenBrainz y `user.getInfo` en Last.fm con la sesión guardada; reprograma a "ahora" los envíos en error de autenticación si la prueba pasa | Una línea por servicio: OK / no configurado / error |
| `mmmusic reescanear --completo` | Reescaneo completo forzado sin TUI | Progreso y resumen |
| `mmmusic --version`, `--help` | clap | |

### 5.3 API interna

`ComandoScrobbling` (UI / reproductor → hilo scrobbling): `NowPlaying(PistaResumen)`, `Completada(historial_id, PistaResumen, reproducido_en)`, `Amar(pista_id)`, `Desamar(pista_id)`, `EnviarAhora`, `RecargarCredenciales`, `Apagar`.

`EstadoScrobbling` (hilo scrobbling → UI, `watch`): `activo: bool`, `pendientes: u32`, `ultimo_envio_en: Option<Instant>`, `error: Option<String>` (texto seguro sin credenciales), por servicio.

Consultas nuevas (`consultas.rs`):

| Función | Descripción |
|---------|-------------|
| `envios::encolar(servicio, tipo, pista_id, historial_id, reproducido_en)` | Descarta love/unlove pendiente anterior para la misma pista y servicio |
| `envios::pendientes(servicio, limite=50)` | Con `proximo_intento_en <= ahora`, orden por `reproducido_en` |
| `envios::marcar_enviados(ids)` / `marcar_error(ids, msg, proximo)` / `descartar(ids, motivo)` | |
| `envios::contar_pendientes()` | Para el indicador |
| `envios::reprogramar_errores_auth(servicio)` | Tras `probar-servicios` OK |
| `favoritas::alternar(pista_id) -> bool` / `es_favorita` / `listar()` | |
| `inicio()` | Ahora devuelve `album_id` y `caratula_ruta` en los tres bloques |
| `recopilatorios::grupos_candidatos(escaneo_id, completo)` / `consolidar(...)` | |
| `ajustes::reescaneo_completo_pendiente()` / `fijar(...)` | |

### 5.4 APIs externas consumidas

| Servicio | Petición | Uso |
|----------|----------|-----|
| ListenBrainz | `GET https://api.listenbrainz.org/1/validate-token` (Authorization: Token …) | `probar-servicios` |
| ListenBrainz | `POST https://api.listenbrainz.org/1/submit-listens` con `listen_type` `playing_now` / `import` | now playing / lote de scrobbles (`track_metadata`: artist_name, track_name, release_name, additional_info.duration_ms, media_player "mmmusic", submission_client "mmmusic") |
| Last.fm | `POST https://ws.audioscrobbler.com/2.0/` `auth.getToken`, `auth.getSession` | Autorización |
| Last.fm | `track.updateNowPlaying`, `track.scrobble` (hasta 50), `track.love`, `track.unlove`, `user.getInfo` | Scrobbling y favoritas |

Firma Last.fm: `api_sig = md5(concat(claves ordenadas alfabéticamente + valores) + api_secret)`, `format=json`. Todas las peticiones con `User-Agent: mmmusic/<versión>` y timeout de 10 s.

---

## 6. Interfaz de Usuario

### Mapa de navegación (cambios)

```
mmmusic
├── [1] Inicio  ── tres bloques en rejilla de tarjetas con carátula (h/l dentro del bloque, j/k entre bloques)
├── [6] Playlists
│     ├── ♥ Favoritas (pseudo-playlist, primera, no editable)
│     └── … playlists del usuario (F1)
├── Barra inferior: + ♥ si la pista actual es favorita; + indicador de scrobbling
└── CLI: autorizar-lastfm · probar-servicios · reescanear --completo
```

### Mockup 1: Inicio con carátulas

```
┌ mmmusic ─────────┬──────────────────────────────────────────────────────────┐
│  1 Inicio      ◄ │ Reproducidas recientemente                               │
│  2 Buscar        │ ▄▄▄▄ ▄▄▄▄ ▄▄▄▄ ▄▄▄▄ ▄▄▄▄ ▄▄▄▄                            │
│  3 Artistas      │ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀                            │
│  4 Álbumes       │ ▶Noct Neon Term Mall Lost Open                           │
│  5 Pistas        │ Midn ESPR Mono ESPR Midn Midn                            │
│  6 Playlists     │                                                          │
│                  │ Añadidos recientemente                                   │
│  Playlists       │ ▄▄▄▄ ▄▄▄▄ ▄▄▄▄ ▄▄▄▄ ▄▄▄▄ ▄▄▄▄                            │
│   ♥ Favoritas    │ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀                            │
│   Vaporwave      │ Virt Mall Lost Late Sign Vari                            │
│   Trabajo        │ ESPR Mono Midn Midn Mono Vari                            │
│   Coche          │                                                          │
│                  │ Redescubre                                               │
│                  │ ▄▄▄▄ ▄▄▄▄ ▄▄▄▄ ▄▄▄▄ ▄▄▄▄ ▄▄▄▄                            │
│ 4.812 pistas     │ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀                            │
├──────────────────┴──────────────────────────────────────────────────────────┤
│ ▀▀▀ ♥ Nocturne Drive                     ⏮  ⏸  ⏭      🔀 🔁 ♪ vol 80 %  ↑  │
│ ▄▄▄ Midnight Premiere · Late Night Tapes  1:27 ━━━━━━━●───────────── 4:12  │
└─────────────────────────────────────────────────────────────────────────────┘
```

Cada bloque es una fila de tarjetas 4×3 (carátula + título + artista truncados) con desplazamiento horizontal; el bloque enfocado resalta su título. Con `caratulas = false` o alto < 20 filas, Inicio vuelve al formato de texto de F1.

### Mockup 2: ♥ Favoritas

```
┌ mmmusic ─────────┬──────────────────────────────────────────────────────────┐
│  6 Playlists   ◄ │ ♥ Favoritas · 23 pistas · 1:32:10                        │
│                  │  Título                        Artista        Durac.     │
│  Playlists       │▶ Nocturne Drive                Midnight Prem.  4:12      │
│   ♥ Favoritas    │  Neon Rain                     ESPRIT 空想     3:48      │
│   Vaporwave      │  Lost Signal                   Monodrone       6:20      │
│   Trabajo        │  …                                                       │
│   Coche          │                                                          │
│ 4.812 pistas     │ Enter reproducir · L quitar · a cola · e exportar M3U    │
├──────────────────┴──────────────────────────────────────────────────────────┤
│ ▀▀▀ ♥ Nocturne Drive                     ⏮  ⏸  ⏭      🔀 🔁 ♪ vol 80 %  …3 │
│ ▄▄▄ Midnight Premiere · Late Night Tapes  1:27 ━━━━━━━●───────────── 4:12  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Mockup 3: indicador de scrobbling y ayuda

```
  Indicador (esquina derecha de la barra inferior):
    ↑     último lote enviado, nada pendiente          (ascii: sc:ok)
    …3    3 envíos pendientes / reintentando            (ascii: sc:3)
    !     error de autenticación en algún servicio      (ascii: sc:err) — el toast dice cuál
    (nada) scrobbling desactivado

  Overlay de ayuda (?) — sección nueva "Scrobbling y favoritas":
    L        favorita          Ctrl+s   enviar pendientes ahora
    CLI:     mmmusic autorizar-lastfm · probar-servicios · reescanear --completo
```

### Mockup 4: `mmmusic autorizar-lastfm`

```
$ mmmusic autorizar-lastfm
Abre esta URL y autoriza mmmusic en Last.fm:
  https://www.last.fm/api/auth/?api_key=0123…&token=abcd…
Pulsa Enter cuando hayas autorizado…
Last.fm autorizado como hector_4d3. Sesión guardada en ~/.config/mmmusic/credenciales.toml
```

### Notas de UX

- El indicador no distrae: sin scrobbling activo no ocupa espacio; en modo compacto (< 70 columnas) solo se muestra `!` si hay error.
- Nunca se bloquea la reproducción por el estado de la red.
- El "Varios artistas" se muestra como artista en Artistas y en la cabecera del álbum; en la lista de pistas del recopilatorio la columna Artista muestra el artista real de cada pista (ya era así en F1).
- Al pulsar `L` en un elemento que no es pista (artista, álbum, playlist) se ignora sin error (regla de F1).

---

## 7. Lógica de Negocio

### 7.1 Elegibilidad y umbral de scrobble

```python
def elegible(duracion_ms: int) -> bool:
    return duracion_ms > 30_000

def umbral_ms(duracion_ms: int) -> int:
    return min(duracion_ms // 2, 240_000)

def debe_scrobblear(duracion_ms, tiempo_real_ms) -> bool:
    return elegible(duracion_ms) and tiempo_real_ms >= umbral_ms(duracion_ms)
```

`HISTORIAL_REPRODUCCION.completada` pasa a usar este umbral (antes, 50 % fijo). Las filas antiguas no se recalculan. Pistas ≤ 30 s se registran en historial pero nunca generan ENVIOS. Con repetición `una`, cada vuelta genera una fila de historial nueva y, por tanto, un scrobble nuevo (comportamiento estándar).

### 7.2 Planificador de reintentos

```python
def proximo_intento(intentos: int, ahora) -> datetime:
    espera = min(30 * 2 ** intentos, 6 * 3600)   # 30 s, 60 s, 2 min, … tope 6 h
    return ahora + timedelta(seconds=espera)

def clasificar(respuesta):
    if respuesta.ok: return "enviado"
    if respuesta.status in (401, 403) or respuesta.lastfm_error in (4, 9, 14): return "auth"    # +24 h y aviso
    if respuesta.status == 400 and respuesta.lastfm_error == 13: return "descartar"           # firma inválida: bug, no reintentar
    return "reintentar"                                                                        # red, timeout, 5xx, 429
```

- Lote máximo 50 por servicio y ciclo; ciclo cada 30 s si hay pendientes, si no el hilo duerme hasta recibir un comando.
- Respuesta parcial de Last.fm (`ignored` en un scrobble del lote): ese envío pasa a `descartado` con el motivo de Last.fm; el resto, `enviado`.
- ListenBrainz `import` acepta el lote completo o lo rechaza entero (400): en 400 se reintenta una vez elemento a elemento para aislar el defectuoso y descartarlo.

### 7.3 Recopilatorios

```python
def es_recopilatorio(pistas_del_grupo) -> bool:
    return (all(p.artista_album_etiquetado == 0 for p in pistas_del_grupo)
            and len({p.artista_id for p in pistas_del_grupo}) > 1)
```

- El grupo es `(carpeta, titulo_norm del álbum)`; dos carpetas distintas con un recopilatorio del mismo nombre son dos álbumes.
- El artista "Varios artistas" (`nombre_norm = "varios artistas"`) se crea una sola vez; si el usuario tiene ficheros con `albumartist = "Various Artists"` se conservan tal cual (etiquetado explícito manda) y no se fusionan.
- `anio` del recopilatorio = mínimo de las pistas.
- Si tras un escaneo incremental un grupo deja de cumplir la condición (por ejemplo, el usuario etiqueta `albumartist`), las pistas vuelven a álbumes normales.

### 7.4 Favoritas y "love"

- `alternar` es idempotente por pista; `love`/`unlove` pendientes anteriores para la misma pista y servicio se descartan al encolar el nuevo, así solo llega el último estado.
- Al exportar "♥ Favoritas" a M3U8 se usa el nombre `Favoritas.m3u8`.
- Borrar una pista de la biblioteca borra su favorita (cascada); no se envía `unlove`.

### 7.5 Validaciones

- `credenciales.toml` con permisos distintos de 600 → toast de aviso al arrancar ("credenciales legibles por otros usuarios") y se corrigen a 600 automáticamente.
- Servicio activo en config sin credenciales → toast al arrancar "ListenBrainz activo sin token" y el servicio se trata como inactivo hasta `RecargarCredenciales`.
- `now playing` nunca se encola ni se reintenta.
- Textos enviados: se envían `titulo`, `nombre` de artista de pista y `titulo` de álbum originales (no normalizados); si el artista es "Artista desconocido" se envía igualmente (es lo que sabe la biblioteca).

### 7.6 Casos especiales

- Escucha completada mientras no hay red y cierre de mmmusic: los ENVIOS persisten y se envían en el siguiente arranque.
- Reloj del sistema atrasado: Last.fm rechaza timestamps futuros → `descartado` con motivo; ListenBrainz los acepta.
- Recopilatorio con una sola pista indexada al principio de un escaneo y más después: la consolidación ocurre al final de la pasada, así que el resultado es correcto al completarse.

---

## 8. Requisitos No Funcionales

### Seguridad

- Red saliente exclusivamente a `api.listenbrainz.org` y `ws.audioscrobbler.com` por HTTPS (rustls), y solo si el servicio está activo en config. Ningún otro host; sin proxies de configuración de mpv.
- Tokens, `api_secret` y `session_key` nunca se escriben en logs, toasts, `error_msg` ni informes; el hilo de scrobbling redacta cualquier cadena que los contenga antes de registrarla.
- `credenciales.toml` con permisos 600; `config.toml` sigue siendo compartible.
- `xdg-open` en `autorizar-lastfm` recibe únicamente la URL construida por mmmusic.

### Protección de datos

- Con scrobbling activo, el historial de escuchas (título, artista, álbum, duración, instante) sale de la máquina hacia los servicios elegidos por el usuario. Es una decisión explícita del usuario (opt-in en config), revocable desactivando el servicio; los envíos pendientes se pueden vaciar borrando las filas de ENVIOS. Las favoritas se sincronizan solo con Last.fm.

### Backups y recuperación

- ENVIOS y FAVORITAS viven en `mmmusic.db` (mismo WAL y misma política de F1). `credenciales.toml` es el único fichero nuevo no regenerable: se documenta en el README que debe respaldarse aparte.
- Migración 002 aditiva (columnas e índices nuevos); el reescaneo completo forzado es idempotente y reanudable.

### Rendimiento

- El reescaneo completo forzado de 10.000 pistas relee todas las etiquetas: objetivo < 90 s (mismo que el escaneo inicial de F1); la consolidación de recopilatorios < 2 s para 50.000 pistas (consulta agrupada, un `UPDATE` por grupo).
- Inicio con carátulas: tres filas de hasta 10 tarjetas; decodificación diferida y LRU de F1; primer render < 100 ms, tarjetas aparecen progresivamente.
- Hilo de scrobbling: ≤ 2 peticiones por servicio y ciclo; consumo despreciable.

### Accesibilidad

- El indicador de scrobbling tiene equivalente `ascii` y su significado se explica en el overlay de ayuda.
- Los subcomandos de CLI imprimen texto plano y códigos de salida estándar (0 OK, 1 error de configuración, 2 error de red).

---

## 9. Integraciones

| Integración | Detalle |
|-------------|---------|
| ListenBrainz | Token de usuario. `submit-listens` con `listen_type` `playing_now` (sin `listened_at`) e `import` (con `listened_at` en segundos Unix). Límite de lote 50. Sin `love` (DT-16) |
| Last.fm | API key + secret del usuario, sesión obtenida por `autorizar-lastfm`. Métodos §5.4. Rechaza scrobbles > 14 días, timestamps futuros y pistas < 30 s |
| Escáner (F1) | Nueva pasada de consolidación y modo completo forzado |
| Rejilla de tarjetas (F1) | Reutilizada en Inicio |

`config.toml` (sección nueva):

```toml
[scrobbling]
listenbrainz = false
lastfm = false
now_playing = true
```

---

## 10. Decisiones Técnicas (ADR-lite)

**DT-15 — `ureq` en hilo dedicado frente a `reqwest` sobre tokio.** Coherente con T-2: el único código async sigue siendo MPRIS. `ureq` con rustls no añade OpenSSL ni runtime; el hilo duerme cuando no hay pendientes.

**DT-16 — Favoritas sincronizadas solo con Last.fm.** ListenBrainz solo acepta "feedback" por `recording_mbid`/`recording_msid`; sin MusicBrainz en la biblioteca no los tenemos y el lookup por texto no es fiable. Se documenta como limitación; si en el futuro se añaden MBIDs (etiquetas Picard), se ampliará.

**DT-17 — Cola de envíos genérica (ENVIOS con `tipo`) frente a tablas separadas por servicio.** Un solo planificador de reintentos y un solo indicador para scrobbles y love/unlove; añadir un servicio o tipo nuevo es una fila más, no una tabla más.

**DT-18 — Credenciales en fichero aparte con permisos 600 frente a keyring.** El keyring (secret-service) exige un demonio corriendo y complica el uso desde scripts y CLI; el fichero 600 es el estándar de las herramientas de terminal (`~/.netrc`, `rclone.conf`). `config.toml` queda libre de secretos.

**DT-19 — Recopilatorios por `(carpeta, álbum)` con índices únicos parciales frente a un flag `compilation`.** La carpeta es la señal más fiable en bibliotecas descargadas sin etiquetado cuidado; los índices parciales mantienen la unicidad de F1 para los álbumes normales sin romper claves existentes. La etiqueta `compilation` (opción 9b) queda fuera: cuando existe, suele venir con `albumartist` y ya se respeta.

**DT-20 — Reescaneo completo forzado por bandera en AJUSTES frente a recalcular en la migración SQL.** La migración no puede leer etiquetas (necesita `artista_album_etiquetado`, que solo el escáner conoce). La bandera hace el proceso reanudable si se cancela y reutilizable para futuras migraciones que requieran releer ficheros.

**DT-21 — Umbral estándar (≥ 50 % o ≥ 4 min, pista > 30 s) también para `completada`.** Una única regla para historial y scrobbling evita divergencias entre lo que Inicio muestra como "reproducida" y lo que se envía.

---

## 11. Plan de Desarrollo

| Sprint | Contenido | Entregable verificable |
|--------|-----------|------------------------|
| S1 — Recopilatorios | Migración 002, columnas nuevas del escáner, pasada de consolidación, reescaneo completo forzado (arranque + CLI `reescanear --completo`), tests con fixture de recopilatorio | Un recopilatorio de prueba aparece bajo "Varios artistas" tras el reescaneo |
| S2 — Scrobbling ListenBrainz | `clap`, `credenciales.rs`, hilo de scrobbling, regla y umbral, ENVIOS, planificador, ListenBrainz now playing + import, indicador en barra inferior, `probar-servicios`, `Ctrl+s` | Escuchas visibles en el perfil de ListenBrainz; con red cortada se acumulan y luego se envían |
| S3 — Last.fm y favoritas | Firma, `autorizar-lastfm`, now playing, scrobble por lotes, love/unlove, FAVORITAS, `L`, pseudo-playlist "♥ Favoritas", ♥ en barra inferior | Scrobbles y loves en el perfil de Last.fm |
| S4 — Inicio y cierre | Inicio con rejillas de tarjetas, fallback texto, ayuda actualizada, README y `credenciales.ejemplo.toml`, tests, clippy/fmt | Inicio con carátulas y suite verde |

**Estimación:** 4 sprints de ~1 semana → 3–4 semanas nominales. Supuestos: los mismos de F1; el usuario dispone de cuentas en ListenBrainz y Last.fm y crea la API key de Last.fm; las pruebas contra los servicios reales las hace el usuario (el agente prueba firma, cuerpos JSON y planificador con tests sin red). En F1 los cinco sprints se resolvieron en una sesión; es razonable esperar 1–2 sesiones.

---

## 12. Conexiones con Otras Fases

- **Desde F1:** HISTORIAL_REPRODUCCION (`completada` cambia de regla, DT-21), T-10 (tiempo real acumulado), escáner (T-8), rejilla de tarjetas (T-9), `AJUSTES`.
- **Hacia F3 (letras y ecualizador):** ninguna dependencia; `clap` y `credenciales.rs` quedan disponibles si algún proveedor de letras requiere clave.
- **Hacia F4 (radio / URL):** los elementos de cola que no sean PISTAS no generan historial ni ENVIOS (la regla `elegible` exige `pista_id`).
- **Hacia F5 (Nicotine+):** el reescaneo por `notify` reutiliza la pasada de consolidación de recopilatorios.
- **Limitación registrada:** favoritas no sincronizadas con ListenBrainz (DT-16).
