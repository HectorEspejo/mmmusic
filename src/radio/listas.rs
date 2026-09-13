#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntradaLista {
    pub nombre: Option<String>,
    pub url: String,
}

/// Detecta el formato por extensión y, si no es concluyente, por contenido.
pub fn parsear(contenido: &str, extension: Option<&str>) -> Vec<EntradaLista> {
    match extension {
        Some("pls") => parsear_pls(contenido),
        Some("m3u") | Some("m3u8") => parsear_m3u(contenido),
        _ => {
            if contenido.trim_start().starts_with("[playlist]") {
                parsear_pls(contenido)
            } else {
                parsear_m3u(contenido)
            }
        }
    }
}

pub fn parsear_pls(contenido: &str) -> Vec<EntradaLista> {
    let mut ficheros: Vec<(usize, String)> = Vec::new();
    let mut titulos: Vec<(usize, String)> = Vec::new();
    for linea in contenido.lines() {
        let linea = linea.trim();
        let Some((clave, valor)) = linea.split_once('=') else {
            continue;
        };
        let valor = valor.trim().to_string();
        if valor.is_empty() {
            continue;
        }
        if let Some(indice) = clave.strip_prefix("File") {
            if let Ok(indice) = indice.trim().parse::<usize>() {
                ficheros.push((indice, valor));
            }
        } else if let Some(indice) = clave.strip_prefix("Title")
            && let Ok(indice) = indice.trim().parse::<usize>()
        {
            titulos.push((indice, valor));
        }
    }
    ficheros
        .into_iter()
        .map(|(indice, url)| EntradaLista {
            nombre: titulos
                .iter()
                .find(|(titulo_indice, _)| *titulo_indice == indice)
                .map(|(_, titulo)| titulo.clone()),
            url,
        })
        .collect()
}

pub fn parsear_m3u(contenido: &str) -> Vec<EntradaLista> {
    let mut entradas = Vec::new();
    let mut nombre: Option<String> = None;
    for linea in contenido.lines() {
        let linea = linea.trim();
        if linea.is_empty() {
            continue;
        }
        if let Some(resto) = linea.strip_prefix("#EXTINF:") {
            nombre = resto
                .split_once(',')
                .map(|(_, titulo)| titulo.trim().to_string())
                .filter(|titulo| !titulo.is_empty());
            continue;
        }
        if linea.starts_with('#') {
            continue;
        }
        entradas.push(EntradaLista {
            nombre: nombre.take(),
            url: linea.to_string(),
        });
    }
    entradas
}

pub fn escribir_m3u8(entradas: &[EntradaLista]) -> String {
    let mut salida = String::from("#EXTM3U\n");
    for entrada in entradas {
        let nombre = entrada
            .nombre
            .as_deref()
            .map(|nombre| nombre.replace(['\n', '\r'], " "))
            .unwrap_or_default();
        salida.push_str(&format!("#EXTINF:-1,{nombre}\n{}\n", entrada.url));
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn parsea_pls_con_titulos() {
        let pls = "[playlist]\nNumberOfEntries=2\nFile1=http://a.example:8000/stream\nTitle1=Radio A\nLength1=-1\nFile2=http://b.example/live\nTitle2=Radio B\n";
        let entradas = parsear_pls(pls);
        assert_eq!(entradas.len(), 2);
        assert_eq!(entradas[0].url, "http://a.example:8000/stream");
        assert_eq!(entradas[0].nombre.as_deref(), Some("Radio A"));
        assert_eq!(entradas[1].nombre.as_deref(), Some("Radio B"));
    }

    #[test]
    fn parsea_m3u_con_extinf() {
        let m3u = "#EXTM3U\n#EXTINF:-1,Radio A\nhttp://a.example/stream\n#EXTINF:-1,\nhttp://b.example/stream\n";
        let entradas = parsear_m3u(m3u);
        assert_eq!(entradas.len(), 2);
        assert_eq!(entradas[0].nombre.as_deref(), Some("Radio A"));
        assert_eq!(entradas[1].nombre, None);
    }

    #[test]
    fn ida_y_vuelta_m3u8() {
        let entradas = vec![
            EntradaLista {
                nombre: Some("Radio A".to_string()),
                url: "http://a.example/stream".to_string(),
            },
            EntradaLista {
                nombre: None,
                url: "http://b.example/stream".to_string(),
            },
        ];
        let escrito = escribir_m3u8(&entradas);
        assert!(escrito.starts_with("#EXTM3U\n"));
        assert!(escrito.contains("#EXTINF:-1,Radio A\nhttp://a.example/stream\n"));
        let releidas = parsear_m3u(&escrito);
        assert_eq!(releidas, entradas);
    }

    #[test]
    fn detecta_por_contenido_sin_extension() {
        let pls = "[playlist]\nFile1=http://a.example/stream\n";
        assert_eq!(parsear(pls, None).len(), 1);
        let m3u = "#EXTM3U\nhttp://a.example/stream\n";
        assert_eq!(parsear(m3u, None).len(), 1);
    }
}
