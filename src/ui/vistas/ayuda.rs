use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::app::AppEstado;
use crate::config::ModoIconos;
use crate::marca;

pub fn dibujar(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let ancho = area.width.saturating_sub(4).clamp(20, 76);
    let alto = area.height.saturating_sub(2).clamp(6, 60);
    let destino = Rect {
        x: area.x + area.width.saturating_sub(ancho) / 2,
        y: area.y + area.height.saturating_sub(alto) / 2,
        width: ancho,
        height: alto,
    };
    frame.render_widget(Clear, destino);
    let bloque = Block::bordered()
        .title(" Ayuda (Esc para cerrar) ")
        .border_style(Style::new().fg(app.paleta.acento).bg(app.paleta.fondo))
        .style(Style::new().fg(app.paleta.texto).bg(app.paleta.fondo));
    let interior = bloque.inner(destino);
    frame.render_widget(bloque, destino);

    let seccion = Style::new()
        .fg(app.paleta.acento)
        .add_modifier(Modifier::BOLD);
    let tecla = Style::new().fg(app.paleta.texto);
    let descripcion = Style::new().fg(app.paleta.secundario);
    let mut lineas = Vec::new();

    let ascii = app.config.interfaz.iconos == ModoIconos::Ascii;
    let filas: &[&str; 2] = if ancho < 37 {
        if ascii {
            &marca::LOGO_MMM_ASCII
        } else {
            &marca::LOGO_MMM
        }
    } else if ascii {
        &marca::LOGO_COMPACTO_ASCII
    } else {
        &marca::LOGO_COMPACTO
    };
    let ancho_interior = interior.width as usize;
    let version = format!("v{}", marca::version());
    let ancho_bloque = filas[0].width() + 3 + version.width();
    let con_version = ancho_interior >= ancho_bloque;
    let margen = if con_version {
        (ancho_interior - ancho_bloque) / 2
    } else {
        ancho_interior.saturating_sub(filas[0].width()) / 2
    };
    let sangria = " ".repeat(margen);
    let estilo_logo = Style::new().fg(app.paleta.acento);
    lineas.push(Line::from(Span::styled(
        format!("{sangria}{}", filas[0]),
        estilo_logo,
    )));
    if con_version {
        lineas.push(Line::from(vec![
            Span::styled(format!("{sangria}{}", filas[1]), estilo_logo),
            Span::raw("   "),
            Span::styled(version, Style::new().fg(app.paleta.secundario)),
        ]));
    } else {
        lineas.push(Line::from(Span::styled(
            format!("{sangria}{}", filas[1]),
            estilo_logo,
        )));
    }
    lineas.push(Line::from(""));

    let añadir_seccion = |titulo: &str, lineas: &mut Vec<Line>| {
        lineas.push(Line::from(Span::styled(format!(" {titulo}"), seccion)));
    };
    añadir_seccion("Globales", &mut lineas);
    for (k, d) in [
        ("q", "salir guardando"),
        ("Ctrl+c", "salir inmediato"),
        ("Esc", "volver / cerrar"),
        ("?", "mostrar u ocultar esta ayuda"),
        (
            "1-9",
            "Inicio, Buscar, Artistas, Álbumes, Pistas, Playlists, Visual, Radio, Letras",
        ),
        ("c", "mostrar u ocultar la cola"),
        ("Tab / Shift+Tab", "ciclar foco sidebar → contenido → cola"),
        ("Ctrl+r", "reescanear la biblioteca"),
        ("t", "recargar el tema"),
        ("S", "editar la configuración en $EDITOR"),
    ] {
        lineas.push(Line::from(vec![
            Span::styled(format!("   {k:<17} "), tecla),
            Span::styled(d, descripcion),
        ]));
    }
    añadir_seccion("Navegación", &mut lineas);
    for (k, d) in [
        ("j / k / ↓ / ↑", "bajar / subir"),
        ("h / l / ← / →", "izquierda / derecha"),
        ("gg / G", "primero / último"),
        ("Ctrl+d / Ctrl+u", "media página abajo / arriba"),
        ("o / O", "cambiar columna de orden / invertirla (Pistas)"),
        ("Enter", "abrir o reproducir"),
    ] {
        lineas.push(Line::from(vec![
            Span::styled(format!("   {k:<17} "), tecla),
            Span::styled(d, descripcion),
        ]));
    }
    añadir_seccion("Reproducción", &mut lineas);
    for (k, d) in [
        ("Espacio", "reproducir / pausar"),
        ("n / p", "siguiente / anterior"),
        ("x", "detener"),
        (", / .", "retroceder / avanzar 5 s"),
        ("< / >", "retroceder / avanzar 30 s"),
        ("+ / -", "volumen ±5"),
        ("m", "silenciar"),
        ("s / r", "aleatorio / repetición"),
    ] {
        lineas.push(Line::from(vec![
            Span::styled(format!("   {k:<17} "), tecla),
            Span::styled(d, descripcion),
        ]));
    }
    añadir_seccion("Cola y playlists", &mut lineas);
    for (k, d) in [
        ("a / A", "añadir a la cola / a continuación"),
        ("P", "añadir a una playlist"),
        ("d", "quitar de la cola o playlist"),
        ("J / K", "mover abajo / arriba"),
        ("N / R / D", "nueva / renombrar / eliminar playlist"),
        ("e / i", "exportar / importar M3U8"),
        ("C", "vaciar la cola"),
    ] {
        lineas.push(Line::from(vec![
            Span::styled(format!("   {k:<17} "), tecla),
            Span::styled(d, descripcion),
        ]));
    }
    añadir_seccion("Scrobbling y favoritas", &mut lineas);
    for (k, d) in [
        ("L", "marcar o quitar la favorita"),
        ("Ctrl+s", "enviar pendientes de scrobbling ahora"),
        (
            "CLI",
            "autorizar-lastfm · probar-servicios · reescanear --completo",
        ),
    ] {
        lineas.push(Line::from(vec![
            Span::styled(format!("   {k:<17} "), tecla),
            Span::styled(d, descripcion),
        ]));
    }
    añadir_seccion("Radio", &mut lineas);
    for (k, d) in [
        ("[ / ]", "pestaña anterior / siguiente"),
        ("Enter", "escuchar; en Buscar guarda y reproduce"),
        ("L", "favorita de la emisora seleccionada o sonando"),
        ("N / R / D", "nueva / editar / eliminar emisora"),
        ("i / e", "importar PLS/M3U · exportar favoritas M3U8"),
        ("/", "pestaña Buscar y foco en Nombre"),
        ("f", "buscar el título ICY en la biblioteca"),
        ("Tab", "ciclar campos en Buscar"),
    ] {
        lineas.push(Line::from(vec![
            Span::styled(format!("   {k:<17} "), tecla),
            Span::styled(d, descripcion),
        ]));
    }
    añadir_seccion("Ecualizador (E)", &mut lineas);
    for (k, d) in [
        ("E", "abrir o cerrar el overlay del ecualizador"),
        ("h / l", "banda anterior / siguiente (preamp incluido)"),
        ("j / k / J / K", "±1 dB / ±0,5 dB en la banda"),
        ("0 / R", "banda a 0 / todo a Plano"),
        ("Tab / Enter", "barras ↔ lista de presets / aplicar preset"),
        (
            "N / D",
            "guardar la curva como preset / borrar un preset propio",
        ),
        (
            "e / x / g",
            "EQ on-off / limitador / ReplayGain no-pista-álbum",
        ),
    ] {
        lineas.push(Line::from(vec![
            Span::styled(format!("   {k:<17} "), tecla),
            Span::styled(d, descripcion),
        ]));
    }
    añadir_seccion("Letras (9)", &mut lineas);
    for (k, d) in [
        ("9", "vista Letras; en modo visual, letras superpuestas"),
        ("j / k / rueda", "desplazamiento manual durante 5 s"),
        ("Enter", "saltar a la línea y volver a seguimiento"),
        ("( / )", "offset −100 / +100 ms (Shift: ±500 ms)"),
        ("s", "alternar fuente fichero / etiqueta"),
        ("gg / G", "inicio / fin"),
    ] {
        lineas.push(Line::from(vec![
            Span::styled(format!("   {k:<17} "), tecla),
            Span::styled(d, descripcion),
        ]));
    }
    añadir_seccion("Visual", &mut lineas);
    for (k, d) in [
        ("7", "entrar o salir del modo visual"),
        ("v / V", "siguiente / anterior visual"),
        (
            "1-6",
            "Espectro, Barras y ondas, Ambiente, Partículas, Caleidoscopio, Túnel",
        ),
        ("[ / ]", "sensibilidad ×0.8 / ×1.25"),
        ("b", "paleta: tema ↔ carátula"),
        ("Esc", "salir del modo visual"),
    ] {
        lineas.push(Line::from(vec![
            Span::styled(format!("   {k:<17} "), tecla),
            Span::styled(d, descripcion),
        ]));
    }
    frame.render_widget(Paragraph::new(lineas), interior);
}
