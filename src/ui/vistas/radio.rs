use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{AppEstado, CampoBusquedaRadio, Foco, PestanaRadio};
use crate::biblioteca::modelos::{EmisoraResumen, TituloEmisora};
use crate::radio::radiobrowser::EmisoraDirectorio;
use crate::reproductor::estado::EstadoStream;
use crate::ui::componentes::imagen;
use crate::ui::vistas::{panel, truncar, ventana};

pub fn dibujar(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    let bloque = panel(app, "Radio");
    let interior = bloque.inner(area);
    frame.render_widget(bloque, area);
    if interior.height == 0 {
        return;
    }
    let [cabecera, cuerpo] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(interior);
    dibujar_pestanas(frame, app, cabecera);
    match app.pestana_radio {
        PestanaRadio::Favoritas | PestanaRadio::Todas => dibujar_lista_emisoras(frame, app, cuerpo),
        PestanaRadio::Buscar => dibujar_buscar(frame, app, cuerpo),
        PestanaRadio::Sonando => dibujar_sonando(frame, app, cuerpo),
    }
}

fn dibujar_pestanas(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let mut spans = vec![Span::styled(" Radio  ", Style::new().fg(app.paleta.texto))];
    for pestana in PestanaRadio::TODAS {
        let activa = pestana == app.pestana_radio;
        let estilo = if activa {
            Style::new()
                .fg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.secundario)
        };
        spans.push(Span::styled(
            if activa {
                format!("[{}] ", pestana.titulo())
            } else {
                format!("{} ", pestana.titulo())
            },
            estilo,
        ));
    }
    spans.push(Span::styled(
        "  [ ] pestaña",
        Style::new().fg(app.paleta.secundario),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn dibujar_lista_emisoras(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    let emisoras: Vec<EmisoraResumen> = app.emisoras.clone();
    if emisoras.is_empty() {
        let texto = if app.pestana_radio == PestanaRadio::Favoritas {
            vec![
                Line::from(Span::styled(
                    " No hay emisoras favoritas.",
                    Style::new().fg(app.paleta.secundario),
                )),
                Line::from(Span::styled(
                    " L marca favorita · N nueva · i importar",
                    Style::new().fg(app.paleta.secundario),
                )),
            ]
        } else {
            vec![
                Line::from(Span::styled(
                    " No hay emisoras guardadas.",
                    Style::new().fg(app.paleta.secundario),
                )),
                Line::from(Span::styled(
                    " N nueva · i importar · pestaña Buscar",
                    Style::new().fg(app.paleta.secundario),
                )),
            ]
        };
        frame.render_widget(Paragraph::new(texto), area);
        return;
    }
    let (inicio, fin) = ventana(emisoras.len(), app.seleccion, area.height as usize);
    app.zonas.lista = Some((area, inicio, emisoras.len()));
    for (desplazamiento, emisora) in emisoras[inicio..fin].iter().enumerate() {
        let indice = inicio + desplazamiento;
        let fila = Rect {
            y: area.y + desplazamiento as u16,
            height: 1,
            ..area
        };
        let seleccionada = indice == app.seleccion && app.foco == Foco::Contenido;
        let estilo = if seleccionada {
            Style::new()
                .fg(app.paleta.fondo)
                .bg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.texto)
        };
        let [zona_logo, zona_texto] =
            Layout::horizontal([Constraint::Length(3), Constraint::Min(1)]).areas(fila);
        if !seleccionada {
            imagen::dibujar_logo(
                frame,
                app,
                zona_logo,
                emisora.id,
                emisora.logo_ruta.as_deref(),
            );
        }
        let pais = emisora
            .pais
            .as_deref()
            .map(str::to_uppercase)
            .unwrap_or_else(|| "—".to_string());
        let calidad = calidad_emisora(emisora.codec.as_deref(), emisora.bitrate_kbps);
        let ultima = emisora
            .ultima_reproduccion
            .as_deref()
            .map(hace_cuando)
            .unwrap_or_else(|| "—".to_string());
        let corazon = if emisora.favorita {
            format!("{} ", app.iconos.corazon)
        } else {
            String::new()
        };
        let ancho_nombre = (zona_texto.width as usize).saturating_sub(
            pais.chars().count() + calidad.chars().count() + ultima.chars().count() + 8,
        );
        let texto = format!(
            "{corazon}{}  {pais}  {calidad}  {ultima}",
            truncar(&emisora.nombre, ancho_nombre.max(8))
        );
        frame.render_widget(Paragraph::new(Span::styled(texto, estilo)), zona_texto);
    }
}

fn dibujar_buscar(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    let [formulario, estado, resultados] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(area);
    dibujar_formulario(frame, app, formulario);
    let aviso = if app.busqueda_radio.buscando {
        " buscando…".to_string()
    } else if let Some(aviso) = app.busqueda_radio.aviso.clone() {
        format!(" {aviso}")
    } else if app.config.radio.directorio {
        String::new()
    } else {
        " directorio desactivado: solo emisoras locales".to_string()
    };
    frame.render_widget(
        Paragraph::new(Span::styled(aviso, Style::new().fg(app.paleta.secundario))),
        estado,
    );
    if app.config.radio.directorio {
        let resultados_radio: Vec<EmisoraDirectorio> = app.busqueda_radio.resultados.clone();
        dibujar_resultados(frame, app, resultados, &resultados_radio);
    } else {
        let emisoras: Vec<EmisoraResumen> = app.emisoras.clone();
        dibujar_lista_local(frame, app, resultados, &emisoras);
    }
}

fn dibujar_formulario(frame: &mut Frame, app: &AppEstado, area: Rect) {
    let mut spans = vec![Span::raw(" ")];
    for campo in [
        CampoBusquedaRadio::Nombre,
        CampoBusquedaRadio::Pais,
        CampoBusquedaRadio::Etiqueta,
    ] {
        let activo = app.busqueda_radio.campo == Some(campo);
        let estilo = if activo {
            Style::new().fg(app.paleta.acento)
        } else {
            Style::new().fg(app.paleta.secundario)
        };
        spans.push(Span::styled(format!("{}: ", campo.etiqueta()), estilo));
        let valor = app.busqueda_radio.valor(campo);
        spans.push(Span::styled(
            if valor.is_empty() {
                "…".to_string()
            } else {
                valor.to_string()
            },
            if valor.is_empty() {
                Style::new().fg(app.paleta.secundario)
            } else {
                Style::new().fg(app.paleta.texto)
            },
        ));
        if activo {
            spans.push(Span::styled("▌", Style::new().fg(app.paleta.acento)));
        }
        spans.push(Span::raw("   "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn dibujar_resultados(
    frame: &mut Frame,
    app: &mut AppEstado,
    area: Rect,
    resultados: &[EmisoraDirectorio],
) {
    if resultados.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Escribe un nombre, país o etiqueta y pulsa Enter",
                Style::new().fg(app.paleta.secundario),
            )),
            area,
        );
        return;
    }
    let (inicio, fin) = ventana(resultados.len(), app.seleccion, area.height as usize);
    app.zonas.lista = Some((area, inicio, resultados.len()));
    for (desplazamiento, resultado) in resultados[inicio..fin].iter().enumerate() {
        let indice = inicio + desplazamiento;
        let fila = Rect {
            y: area.y + desplazamiento as u16,
            height: 1,
            ..area
        };
        let seleccionada = indice == app.seleccion && app.foco == Foco::Contenido;
        let estilo = if seleccionada {
            Style::new()
                .fg(app.paleta.fondo)
                .bg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.texto)
        };
        let pais = resultado.pais_limpio().unwrap_or_else(|| "—".to_string());
        let calidad = calidad_emisora(
            Some(&resultado.codec),
            (resultado.bitrate > 0).then_some(resultado.bitrate),
        );
        let votos = format!("★ {}", crate::ui::miles(resultado.votos));
        let ancho_nombre = (fila.width as usize).saturating_sub(
            pais.chars().count() + calidad.chars().count() + votos.chars().count() + 8,
        );
        let texto = format!(
            "♪ {}  {pais}  {calidad}  {votos}",
            truncar(&resultado.nombre_limpio(), ancho_nombre.max(8))
        );
        frame.render_widget(Paragraph::new(Span::styled(texto, estilo)), fila);
    }
}

fn dibujar_lista_local(
    frame: &mut Frame,
    app: &mut AppEstado,
    area: Rect,
    emisoras: &[EmisoraResumen],
) {
    if emisoras.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Sin coincidencias entre las emisoras guardadas",
                Style::new().fg(app.paleta.secundario),
            )),
            area,
        );
        return;
    }
    let (inicio, fin) = ventana(emisoras.len(), app.seleccion, area.height as usize);
    app.zonas.lista = Some((area, inicio, emisoras.len()));
    for (desplazamiento, emisora) in emisoras[inicio..fin].iter().enumerate() {
        let indice = inicio + desplazamiento;
        let fila = Rect {
            y: area.y + desplazamiento as u16,
            height: 1,
            ..area
        };
        let seleccionada = indice == app.seleccion && app.foco == Foco::Contenido;
        let estilo = if seleccionada {
            Style::new()
                .fg(app.paleta.fondo)
                .bg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.texto)
        };
        let [zona_logo, zona_texto] =
            Layout::horizontal([Constraint::Length(3), Constraint::Min(1)]).areas(fila);
        if !seleccionada {
            imagen::dibujar_logo(
                frame,
                app,
                zona_logo,
                emisora.id,
                emisora.logo_ruta.as_deref(),
            );
        }
        let calidad = calidad_emisora(emisora.codec.as_deref(), emisora.bitrate_kbps);
        let texto = format!(
            "{}  {calidad}",
            truncar(
                &emisora.nombre,
                zona_texto.width.saturating_sub(14) as usize
            )
        );
        frame.render_widget(Paragraph::new(Span::styled(texto, estilo)), zona_texto);
    }
}

fn dibujar_sonando(frame: &mut Frame, app: &mut AppEstado, area: Rect) {
    let Some(emisora) = app.estado_reproductor.emisora_actual().cloned() else {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    " No hay ninguna emisora sonando.",
                    Style::new().fg(app.paleta.secundario),
                )),
                Line::from(Span::styled(
                    " Reproduce una emisora desde Favoritas, Todas o Buscar",
                    Style::new().fg(app.paleta.secundario),
                )),
            ]),
            area,
        );
        return;
    };
    let titulos: Vec<TituloEmisora> = app.radio_titulos.clone();
    let [cabecera, estado, titulos_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(area);
    let pais = emisora
        .pais
        .as_deref()
        .map(str::to_uppercase)
        .unwrap_or_else(|| "—".to_string());
    let calidad = calidad_emisora(emisora.codec.as_deref(), emisora.bitrate_kbps);
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(" {} · {pais} · {calidad}", emisora.nombre),
            Style::new()
                .fg(app.paleta.texto)
                .add_modifier(Modifier::BOLD),
        )),
        cabecera,
    );
    let estado_stream = app
        .estado_reproductor
        .stream
        .as_ref()
        .map(|stream| match stream {
            EstadoStream::Conectando => "conectando…".to_string(),
            EstadoStream::Almacenando { segundos } => {
                format!("almacenando {segundos:.1} s")
            }
            EstadoStream::EnDirecto => {
                let mut texto = format!(
                    "EN DIRECTO {}",
                    crate::ui::formatear_ms(app.estado_reproductor.tiempo_escuchando_ms)
                );
                texto.push_str(&format!(
                    " · reconexiones: {}",
                    app.estado_reproductor.reconexiones
                ));
                if app.estado_reproductor.cache_segundos > 0.0 {
                    texto.push_str(&format!(
                        " · caché {:.1} s",
                        app.estado_reproductor.cache_segundos
                    ));
                }
                texto
            }
            EstadoStream::Reconectando(intento) => format!("reconectando ({intento}/6)…"),
            EstadoStream::Rendido => "sin conexión".to_string(),
        });
    let icono = match &app.estado_reproductor.stream {
        Some(EstadoStream::EnDirecto) => app.iconos.directo,
        Some(EstadoStream::Reconectando(_)) => app.iconos.reconectando,
        Some(EstadoStream::Conectando) => app.iconos.reconectando,
        Some(EstadoStream::Almacenando { .. }) => app.iconos.almacenando,
        _ => app.iconos.almacenando,
    };
    let titulo_icy = app
        .estado_reproductor
        .titulo_icy
        .clone()
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(
                " {icono} {}  ·  {titulo_icy}",
                estado_stream.unwrap_or_else(|| "detenido".to_string())
            ),
            Style::new().fg(app.paleta.acento),
        )),
        estado,
    );
    if titulos.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Todavía no hay títulos registrados para esta emisora",
                Style::new().fg(app.paleta.secundario),
            ))
            .wrap(Wrap { trim: true }),
            titulos_area,
        );
        return;
    }
    let [cabecera_titulos, lista] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(titulos_area);
    frame.render_widget(
        Paragraph::new(Span::styled(
            " Últimos títulos",
            Style::new().fg(app.paleta.secundario),
        )),
        cabecera_titulos,
    );
    let (inicio, fin) = ventana(titulos.len(), app.seleccion, lista.height as usize);
    app.zonas.lista = Some((lista, inicio, titulos.len()));
    for (desplazamiento, titulo) in titulos[inicio..fin].iter().enumerate() {
        let indice = inicio + desplazamiento;
        let fila = Rect {
            y: lista.y + desplazamiento as u16,
            height: 1,
            ..lista
        };
        let seleccionada = indice == app.seleccion && app.foco == Foco::Contenido;
        let estilo = if seleccionada {
            Style::new()
                .fg(app.paleta.fondo)
                .bg(app.paleta.acento)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(app.paleta.texto)
        };
        let hora = titulo.visto_en.get(11..16).unwrap_or("--:--").to_string();
        let en_biblioteca = if app.radio_titulos_ok.contains(&titulo.id) {
            "  ✓"
        } else {
            ""
        };
        let texto = format!(
            " {hora}  {}{en_biblioteca}",
            truncar(&titulo.titulo, fila.width.saturating_sub(11) as usize)
        );
        frame.render_widget(Paragraph::new(Span::styled(texto, estilo)), fila);
    }
}

fn calidad_emisora(codec: Option<&str>, bitrate: Option<i64>) -> String {
    let codec = codec
        .map(str::trim)
        .filter(|codec| !codec.is_empty())
        .map(str::to_uppercase);
    match (codec, bitrate) {
        (Some(codec), Some(bitrate)) if bitrate > 0 => format!("{codec} {bitrate}"),
        (Some(codec), _) => codec,
        (None, Some(bitrate)) if bitrate > 0 => format!("{bitrate} kbps"),
        _ => "—".to_string(),
    }
}

fn hace_cuando(iso: &str) -> String {
    let Some(unix) = crate::biblioteca::bd::unix_desde_iso(iso) else {
        return "—".to_string();
    };
    let segundos = (crate::biblioteca::bd::ahora_unix() - unix).max(0);
    if segundos < 60 {
        "ahora".to_string()
    } else if segundos < 3600 {
        format!("hace {} min", segundos / 60)
    } else if segundos < 86_400 {
        format!("hace {} h", segundos / 3600)
    } else if segundos < 172_800 {
        "ayer".to_string()
    } else {
        format!("hace {} d", segundos / 86_400)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::app::{AppEstado, PestanaRadio};
    use crate::biblioteca::modelos::TituloEmisora;
    use crate::config::Config;
    use crate::eventos::AppEvento;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn app_de_prueba() -> AppEstado {
        AppEstado::nuevo(
            Config::default(),
            crate::tema::terminal(&Config::default().tema),
            0,
            std::path::PathBuf::from("/tmp/tema"),
        )
    }

    fn emisora_resumen() -> EmisoraResumen {
        EmisoraResumen {
            id: 1,
            nombre: "Nightwave Plaza".to_string(),
            url: "http://nightwave.example/stream".to_string(),
            pais: Some("us".to_string()),
            codec: Some("MP3".to_string()),
            bitrate_kbps: Some(128),
            favorita: true,
            ..EmisoraResumen::default()
        }
    }

    fn dibujar_radio(app: &mut AppEstado) {
        dibujar_radio_en(app, 120, 40);
    }

    fn dibujar_radio_en(app: &mut AppEstado, ancho: u16, alto: u16) {
        let backend = TestBackend::new(ancho, alto);
        let mut terminal = Terminal::new(backend).expect("terminal de pruebas");
        terminal
            .draw(|frame| crate::ui::dibujar(frame, app))
            .expect("dibujar Radio");
    }

    #[test]
    fn dibuja_las_cuatro_pestanas() {
        let mut app = app_de_prueba();
        app.vista = crate::app::Vista::Radio;
        app.emisoras = vec![emisora_resumen()];
        dibujar_radio(&mut app);

        app.pestana_radio = PestanaRadio::Todas;
        dibujar_radio(&mut app);

        app.pestana_radio = PestanaRadio::Buscar;
        app.busqueda_radio.resultados = vec![EmisoraDirectorio {
            uuid: "abc".to_string(),
            nombre: "Nightwave Plaza".to_string(),
            url: "http://nightwave.example/stream".to_string(),
            pais: "US".to_string(),
            codec: "MP3".to_string(),
            bitrate: 128,
            lastcheckok: 1,
            ..EmisoraDirectorio::default()
        }];
        dibujar_radio(&mut app);

        app.pestana_radio = PestanaRadio::Sonando;
        app.estado_reproductor.elemento = Some(crate::biblioteca::modelos::ElementoCola::Emisora(
            emisora_resumen(),
        ));
        app.radio_titulos = vec![TituloEmisora {
            id: 1,
            emisora_id: 1,
            titulo: "Boards of Canada - Dayvan Cowboy".to_string(),
            visto_en: "2026-09-13T13:04:00Z".to_string(),
        }];
        app.radio_titulos_ok.insert(1);
        dibujar_radio(&mut app);

        // Terminales pequeñas: no debe haber pánicos de layout.
        dibujar_radio_en(&mut app, 40, 8);
        dibujar_radio_en(&mut app, 20, 4);
        app.pestana_radio = PestanaRadio::Buscar;
        dibujar_radio_en(&mut app, 30, 5);
    }

    #[test]
    fn las_emisiones_antiguas_se_muestran_relativas() {
        assert_eq!(hace_cuando(&crate::biblioteca::bd::ahora_iso()), "ahora");
        assert!(hace_cuando("2000-01-01T00:00:00Z").starts_with("hace"));
        assert_eq!(hace_cuando("no es fecha"), "—");
    }

    #[test]
    fn los_eventos_de_radio_tienen_variantes() {
        let _ = AppEvento::LogoListo(1);
        let _ = AppEvento::ResultadosRadio("clave".to_string());
    }
}
