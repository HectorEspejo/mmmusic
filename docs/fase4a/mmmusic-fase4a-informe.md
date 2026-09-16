# mmmusic - Fase 4a: Identidad Visual

## Especificación Funcional

**Versión:** 1.0
**Fecha:** 13 de septiembre de 2026
**Cliente:** Proyecto personal (Hector) — sin cliente externo

---

## 1. Visión General

Subfase corta que da a mmmusic una marca propia en texto: tres logotipos ASCII/Unicode con un uso fijo cada uno (ayuda y sidebar, README, barra de reproducción en reposo), una salida de `--version` con marca y un módulo único de constantes para que el resto de la aplicación no duplique arte. No toca el modelo de datos ni el audio; es independiente de la Fase 4 y puede implementarse antes o después.

### Objetivos principales

1. Logo compacto de dos filas en el overlay de ayuda (completo) y en la cabecera de la sidebar (solo las tres emes).
2. Logo grande de bloques en el README.
3. Tarjeta de reposo en la barra inferior cuando no hay nada cargado, con eslogan, versión y onda animada.
4. Un solo módulo `marca.rs` con todo el arte, versiones para modo `ascii` y tests de anchura.

### Contexto

Fases 1, 2, 3 y 5 completadas; Fase 4 especificada. Terminal Alacritty con Nerd Font; los logos usan solo bloques Unicode (`▀▄█▁▂▃▅▆▇`) y caja simple, sin glifos de Nerd Font, para que funcionen en cualquier terminal Unicode; el modo `ascii` tiene sustitutos.

---

## 2. Arquitectura Técnica

Sin stack nuevo. Añadidos:

```
src/marca.rs                 # constantes de logos, eslogan, versión; funciones de ajuste a ancho
src/ui/sidebar.rs            # cabecera con las tres emes
src/ui/vistas/ayuda.rs       # logo completo + versión en la cabecera del overlay
src/ui/barra_inferior.rs     # tarjeta de reposo (estado detenido sin elemento)
src/cli.rs                   # --version con marca
README.md                    # logo grande
tests/marca.rs               # anchuras y variantes ascii
docs/fase4a/mmmusic-fase4a-{informe,checklist,prompt,implementacion}.md
```

Diagrama: no aplica (sin componentes ni hilos nuevos). El módulo `marca` es una hoja sin dependencias que consumen `ui` y `cli`.

---

## 3. Modelo de Datos

Sin cambios. No hay migración.

---

## 4. Flujos de Trabajo

### 4.1 Tarjeta de reposo en la barra inferior

```
  EstadoReproduccion.elemento == None (cola vacía o detenido sin elemento seleccionado)
        │
        ▼
  Barra inferior dibuja la tarjeta de reposo (§6) en lugar de "(detenido)":
    fila 1: "m m m u s i c · v<versión> · ▶ reproductor para tu tty"   (centrada)
    fila 2: onda ▁▂▃▅▆▇█▇▆▅▃▂ repetida hasta el ancho, desplazada 1 carácter por tick (250 ms)
  Controles (⏮ ⏯ ⏭, volumen, indicadores) se mantienen a la derecha como siempre.
        │
        ▼
  Se carga un elemento (o se restaura la cola en pausa) → barra normal de F1/F5.
  Cola restaurada en pausa con elemento → NO se muestra la tarjeta (hay elemento).
```

### 4.2 Ajuste de logos al ancho

```
  Sidebar (≥ 18 columnas y no colapsada) → cabecera de 2 filas con LOGO_MMM (17 columnas)
  Sidebar colapsada (< 70 columnas de terminal) → una fila "♪"
  Ayuda → LOGO_COMPACTO (33 columnas) centrado; si el overlay tiene < 37 columnas, LOGO_MMM
  Modo ascii → LOGO_MMM_ASCII / LOGO_COMPACTO_ASCII; onda de reposo con "._-~-_." repetido
```

---

## 5. Comandos, Atajos y API Interna

Sin atajos nuevos.

`marca.rs`:

| Constante / función | Contenido |
|---------------------|-----------|
| `LOGO_COMPACTO: [&str; 2]` | Logo 5 (33 columnas) |
| `LOGO_MMM: [&str; 2]` | Las tres emes del logo 5 (17 columnas) |
| `LOGO_COMPACTO_ASCII`, `LOGO_MMM_ASCII` | Variantes con `#` y `=` |
| `LOGO_GRANDE: [&str; 6]` | Logo 1 (62 columnas), solo para README y `--version --logo` |
| `ESLOGAN: &str` | "reproductor para tu tty" |
| `ONDA: &str`, `ONDA_ASCII: &str` | `▁▂▃▅▆▇█▇▆▅▃▂` / `._-~^~-_.` |
| `version() -> &str` | `CARGO_PKG_VERSION` |
| `linea_reposo(ancho, ascii) -> String` | fila 1 de la tarjeta, centrada y truncada con "…" si no cabe |
| `onda(ancho, desplazamiento, ascii) -> String` | fila 2 de la tarjeta |

CLI: `mmmusic --version` imprime `♪ mmmusic <versión>`; `mmmusic --version --logo` imprime LOGO_GRANDE y debajo la versión y el eslogan.

---

## 6. Interfaz de Usuario

### Logos

```
LOGO_COMPACTO (33 col):
█▀▄▀█ █▀▄▀█ █▀▄▀█ █ █ █▀▀ █ █▀▀
█ ▀ █ █ ▀ █ █ ▀ █ █▄█ ▄▄█ █ █▄▄

LOGO_MMM (17 col):
█▀▄▀█ █▀▄▀█ █▀▄▀█
█ ▀ █ █ ▀ █ █ ▀ █

LOGO_COMPACTO_ASCII (33 col):
#=#=# #=#=# #=#=# # # #== # #==
# = # # = # # = # #=# ==# # #==

LOGO_GRANDE (62 col, README):
███╗   ███╗███╗   ███╗███╗   ███╗██╗   ██╗███████╗██╗ ██████╗
████╗ ████║████╗ ████║████╗ ████║██║   ██║██╔════╝██║██╔════╝
██╔████╔██║██╔████╔██║██╔████╔██║██║   ██║███████╗██║██║     
██║╚██╔╝██║██║╚██╔╝██║██║╚██╔╝██║██║   ██║╚════██║██║██║     
██║ ╚═╝ ██║██║ ╚═╝ ██║██║ ╚═╝ ██║╚██████╔╝███████║██║╚██████╗
╚═╝     ╚═╝╚═╝     ╚═╝╚═╝     ╚═╝ ╚═════╝ ╚══════╝╚═╝ ╚═════╝
```

### Mockup 1: sidebar con cabecera

```
┌──────────────────┬───────
│ █▀▄▀█ █▀▄▀█ █▀▄▀█│
│ █ ▀ █ █ ▀ █ █ ▀ █│
│                  │
│  1 Inicio      ◄ │
│  2 Buscar        │
│  …               │
```

El título del borde "mmmusic" de F1 desaparece (lo sustituye la cabecera). La cabecera usa el color de acento.

### Mockup 2: overlay de ayuda

```
              ┌ Ayuda ─────────────────────────────────────────────┐
              │        █▀▄▀█ █▀▄▀█ █▀▄▀█ █ █ █▀▀ █ █▀▀             │
              │        █ ▀ █ █ ▀ █ █ ▀ █ █▄█ ▄▄█ █ █▄▄   v0.4.0    │
              │                                                    │
              │ Globales                                           │
              │  q salir · ? ayuda · 1-9 secciones · …             │
```

### Mockup 3: barra inferior en reposo

```
├──────────────────┴──────────────────────────────────────────────────────────┤
│      m m m u s i c · v0.4.0 · ▶ reproductor para tu tty      ⏮  ▶  ⏭  ♪ vol 80 % │
│ ▁▂▃▅▆▇█▇▆▅▃▂▁▂▃▅▆▇█▇▆▅▃▂▁▂▃▅▆▇█▇▆▅▃▂▁▂▃▅▆▇█▇▆▅▃▂▁▂▃▅▆▇█▇▆▅▃▂▁▂▃▅▆▇█▇▆▅▃▂▁▂  │
└─────────────────────────────────────────────────────────────────────────────┘
```

La onda ocupa la fila 2 completa (donde iría la barra de progreso) y se desplaza un carácter a la izquierda cada tick; con `iconos = "ascii"` usa `._-~^~-_.`; en modo compacto (2 filas de barra) solo se muestra la fila 1 sin eslogan.

### Mockup 4: `--version --logo`

```
$ mmmusic --version --logo
███╗   ███╗███╗   ███╗███╗   ███╗██╗   ██╗███████╗██╗ ██████╗
…
♪ mmmusic 0.4.0 · reproductor para tu tty
```

### Notas de UX

- Nada de la marca transmite información funcional: si no cabe, se recorta sin afectar a controles ni datos.
- La onda de reposo es decorativa y lenta (4 columnas por segundo); se detiene cuando `visuales.activo = false` y `fps` no aplica.
- Colores: logo en acento; eslogan en texto normal; onda en tenue.

---

## 7. Lógica de Negocio

```python
def linea_reposo(ancho, version, ascii):
    play = ">" if ascii else "▶"
    texto = f"m m m u s i c · v{version} · {play} reproductor para tu tty"
    if len(texto) > ancho:
        texto = f"m m m u s i c · v{version}"
    if len(texto) > ancho:
        texto = "mmmusic"
    return texto.center(ancho)

def onda(ancho, desplazamiento, ascii):
    patron = "._-~^~-_." if ascii else "▁▂▃▅▆▇█▇▆▅▃▂"
    d = desplazamiento % len(patron)
    return (patron[d:] + patron) * (ancho // len(patron) + 2)[:ancho]
```

Regla: todas las filas de un mismo logo tienen la misma anchura en columnas de terminal (comprobado por test con `unicode-width`, ya dependencia transitiva de ratatui).

---

## 8. Requisitos No Funcionales

- Sin red, sin datos, sin permisos. Rendimiento: la onda se recalcula en el tick existente (250 ms) sin coste apreciable.
- Accesibilidad: todo tiene variante ascii; la marca nunca sustituye a un estado (la tarjeta de reposo solo aparece cuando de verdad no hay elemento).
- Licencias: LOGO_GRANDE se genera con la fuente `ANSI Shadow` de figlet (dominio público/libre); el resto es diseño propio.

---

## 9. Integraciones

Ninguna nueva. El `.desktop` (F1) actualiza `Comment=Reproductor de música para tu terminal`.

---

## 10. Decisiones Técnicas (ADR-lite)

**DT-55 — Un módulo `marca.rs` con todo el arte.** Evita cadenas de logo repartidas por la UI y permite testear anchuras y variantes ascii en un solo sitio.

**DT-56 — La sidebar lleva solo las tres emes.** El logo completo mide 33 columnas y la sidebar 18; cortar el logo sería peor que mostrar su núcleo. La ayuda muestra el completo.

**DT-57 — La tarjeta de reposo sustituye a "(detenido)", no a la barra con elemento.** Con un elemento cargado (aunque esté detenido o en pausa) el usuario necesita ver qué es; la marca solo ocupa el espacio vacío.

**DT-58 — Subfase independiente de la Fase 4.** No comparte código ni modelo con el ecualizador y las letras; así puede implementarse en una sesión corta sin bloquear ni esperar a la 4.

---

## 11. Plan de Desarrollo

| Sprint | Contenido | Entregable verificable |
|--------|-----------|------------------------|
| S1 (único) | `marca.rs` + tests, sidebar, ayuda, barra de reposo, `--version`, README, `.desktop` | Los tres logos en su sitio y suite verde |

**Estimación:** 1 sprint, 1 sesión.

---

## 12. Conexiones con Otras Fases

- **Desde F1:** sidebar, ayuda, barra inferior, `.desktop`, `--version` (clap desde F2).
- **Con F4:** independiente; si la 4 se implementa antes, la ayuda ya incluye sus secciones y la cabecera se añade encima.
- **Hacia F9 (empaquetado):** el logo grande y el eslogan se reutilizan en la web y en el AUR.
