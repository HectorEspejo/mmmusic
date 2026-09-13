//! Parser LRC sin E/S. Soporta timestamps múltiples por línea, metadatos
//! (`ar`, `ti`, `al`, `by`, `offset`, `length`, `re`, `ve`), marcas enhanced
//! `<mm:ss.xx>` (eliminadas) y `[offset:…]`. BOM y CRLF se normalizan.

pub const MAX_TAMANO_LRC: u64 = 256 * 1024;
pub const MAX_TAMANO_TXT: u64 = 64 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lrc {
    pub lineas: Vec<(u32, String)>,
    pub offset_ms: i64,
}

impl Lrc {
    pub fn vacio(&self) -> bool {
        self.lineas.is_empty()
    }
}

struct AnalisisLinea {
    marcas: Vec<u32>,
    texto: String,
}

/// Parsea un texto LRC completo. Nunca falla: lo que no encaja se ignora.
pub fn parsear(texto: &str) -> Lrc {
    let texto = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    let mut lineas: Vec<(u32, String)> = Vec::new();
    let mut offset_ms = 0;
    for cruda in texto.lines() {
        let cruda = cruda.trim();
        if cruda.is_empty() {
            continue;
        }
        if let Some((clave, valor)) = metadato(cruda) {
            if clave.eq_ignore_ascii_case("offset") {
                offset_ms = valor.trim().parse::<i64>().unwrap_or(0);
            }
            continue;
        }
        let analisis = analizar(cruda);
        if analisis.marcas.is_empty() {
            continue;
        }
        let contenido = analisis.texto.trim().to_string();
        for ms in analisis.marcas {
            lineas.push((ms, contenido.clone()));
        }
    }
    lineas.sort_by_key(|(ms, _)| *ms);
    Lrc { lineas, offset_ms }
}

/// ¿Contiene timestamps `[mm:ss(.xx)]` en cualquier línea? Una línea con
/// marcas enhanced sueltas no basta.
pub fn es_sincronizada(texto: &str) -> bool {
    let texto = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    texto
        .lines()
        .any(|linea| !analizar(linea.trim()).marcas.is_empty())
}

/// Metadato `[clave:valor]` de la lista del informe.
fn metadato(linea: &str) -> Option<(&str, &str)> {
    let interior = linea.strip_prefix('[')?.strip_suffix(']')?;
    let (clave, valor) = interior.split_once(':')?;
    let clave = clave.trim();
    let normalizada = clave.to_ascii_lowercase();
    if matches!(
        normalizada.as_str(),
        "ar" | "ti" | "al" | "by" | "offset" | "length" | "re" | "ve"
    ) {
        Some((clave, valor))
    } else {
        None
    }
}

/// Extrae los timestamps `[…]` de la línea y devuelve el texto sin ellos ni
/// sin las marcas enhanced `<…>`.
fn analizar(linea: &str) -> AnalisisLinea {
    let bytes = linea.as_bytes();
    let mut marcas = Vec::new();
    let mut texto = String::with_capacity(linea.len());
    let mut ultimo = 0;
    let mut indice = 0;
    while indice < bytes.len() {
        let encontrado = match bytes[indice] {
            b'[' => parsear_timestamp(bytes, indice).map(|(ms, fin)| {
                marcas.push(ms);
                fin
            }),
            b'<' => parsear_marca_enhanced(bytes, indice),
            _ => None,
        };
        if let Some(fin) = encontrado {
            texto.push_str(&linea[ultimo..indice]);
            indice = fin;
            ultimo = fin;
            continue;
        }
        indice += 1;
    }
    texto.push_str(&linea[ultimo..]);
    AnalisisLinea { marcas, texto }
}

/// Parsea `[mm:ss]`, `[mm:ss.xx]` o `[mm:ss:xx]` desde `inicio`. Devuelve los
/// milisegundos y la posición siguiente al `]`.
fn parsear_timestamp(bytes: &[u8], inicio: usize) -> Option<(u32, usize)> {
    let mut indice = inicio + 1;
    let (minutos, siguiente) = leer_digitos(bytes, indice, 1, 2)?;
    indice = siguiente;
    if *bytes.get(indice)? != b':' {
        return None;
    }
    indice += 1;
    let (segundos, siguiente) = leer_digitos(bytes, indice, 2, 2)?;
    indice = siguiente;
    let mut fraccion = String::new();
    if matches!(bytes.get(indice), Some(b'.') | Some(b':')) {
        indice += 1;
        let (digitos, siguiente) = leer_digitos(bytes, indice, 1, 3)?;
        fraccion = format!("{digitos:0<3}");
        indice = siguiente;
    }
    if *bytes.get(indice)? != b']' {
        return None;
    }
    let ms = minutos * 60_000 + segundos * 1_000 + fraccion.parse::<u32>().unwrap_or(0);
    Some((ms, indice + 1))
}

/// Parsea `<mm:ss>` o `<mm:ss.xx>`; devuelve la posición siguiente al `>`.
fn parsear_marca_enhanced(bytes: &[u8], inicio: usize) -> Option<usize> {
    let mut indice = inicio + 1;
    let (_, siguiente) = leer_digitos(bytes, indice, 1, 2)?;
    indice = siguiente;
    if *bytes.get(indice)? != b':' {
        return None;
    }
    indice += 1;
    let (_, siguiente) = leer_digitos(bytes, indice, 2, 2)?;
    indice = siguiente;
    if matches!(bytes.get(indice), Some(b'.') | Some(b':')) {
        indice += 1;
        let (_, siguiente) = leer_digitos(bytes, indice, 1, 3)?;
        indice = siguiente;
    }
    if *bytes.get(indice)? != b'>' {
        return None;
    }
    Some(indice + 1)
}

/// Lee entre `minimo` y `maximo` dígitos ASCII. Devuelve el valor y la
/// posición siguiente.
fn leer_digitos(bytes: &[u8], inicio: usize, minimo: usize, maximo: usize) -> Option<(u32, usize)> {
    let mut valor = 0u32;
    let mut leidos = 0usize;
    let mut indice = inicio;
    while leidos < maximo {
        match bytes.get(indice) {
            Some(digito) if digito.is_ascii_digit() => {
                valor = valor * 10 + u32::from(digito - b'0');
                leidos += 1;
                indice += 1;
            }
            _ => break,
        }
    }
    if leidos < minimo {
        None
    } else {
        Some((valor, indice))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn parsea_timestamps_multiples_y_metadatos() {
        let texto =
            "[ar:ESPRIT]\n[ti:Neon]\n[offset:-200]\n[00:12.50]hola\n[00:20][00:40]estribillo\n";
        let lrc = parsear(texto);
        assert_eq!(lrc.offset_ms, -200);
        assert_eq!(
            lrc.lineas,
            vec![
                (12_500, "hola".to_string()),
                (20_000, "estribillo".to_string()),
                (40_000, "estribillo".to_string()),
            ]
        );
    }

    #[test]
    fn elimina_marcas_enhanced() {
        let lrc = parsear("[00:10.00]<00:10.00>Hola <00:10.50>mundo\n");
        assert_eq!(lrc.lineas, vec![(10_000, "Hola mundo".to_string())]);
    }

    #[test]
    fn conserva_lineas_vacias_con_timestamp() {
        let lrc = parsear("[00:05.00]\n[00:10.00]texto\n");
        assert_eq!(
            lrc.lineas,
            vec![(5_000, String::new()), (10_000, "texto".to_string())]
        );
    }

    #[test]
    fn normaliza_bom_y_crlf() {
        let lrc = parsear("\u{feff}[00:01.00]uno\r\n[00:02.00]dos\r\n");
        assert_eq!(lrc.lineas.len(), 2);
        assert_eq!(lrc.lineas[1], (2_000, "dos".to_string()));
    }

    #[test]
    fn detecta_sincronizacion() {
        assert!(es_sincronizada("[00:01.00]hola"));
        assert!(es_sincronizada("cabecera\n[00:01]hola"));
        assert!(!es_sincronizada("solo texto\nsin marcas"));
        assert!(!es_sincronizada("<00:01.00>enhanced suelto"));
    }
}
