use std::net::{IpAddr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use tracing::{debug, warn};

pub const HOST: &str = "all.api.radio-browser.info";
const PUERTO_DNS: u16 = 53;
const TIMEOUT: Duration = Duration::from_secs(3);
const MAX_SALTOS: usize = 16;
const TIPO_PTR: u16 = 12;

/// Resuelve los espejos de Radio Browser: DNS directo de
/// `all.api.radio-browser.info` y DNS inverso (PTR) de cada IP. Si el DNS
/// inverso falla, se usa el nombre agregado como único espejo.
pub fn resolver_espejos() -> Result<Vec<String>> {
    let direcciones = (HOST, 443u16)
        .to_socket_addrs()
        .with_context(|| format!("no se pudo resolver {HOST}"))?;
    let mut ips: Vec<IpAddr> = direcciones.map(|direccion| direccion.ip()).collect();
    ips.sort();
    ips.dedup();
    if ips.is_empty() {
        bail!("{HOST} no devolvió direcciones");
    }
    let mut nombres: Vec<String> = Vec::new();
    for ip in ips {
        match consultar_ptr(ip) {
            Ok(Some(nombre))
                if nombre.ends_with(".api.radio-browser.info") && !nombres.contains(&nombre) =>
            {
                nombres.push(nombre);
            }
            Ok(_) => {}
            Err(error) => {
                debug!(%ip, "DNS inverso falló: {error:#}");
            }
        }
    }
    if nombres.is_empty() {
        warn!("sin nombres de espejo por DNS inverso; se usa {HOST}");
        nombres.push(HOST.to_string());
    }
    Ok(nombres)
}

/// Consulta PTR de una IP contra el primer servidor DNS disponible.
pub fn consultar_ptr(ip: IpAddr) -> Result<Option<String>> {
    let nombre = nombre_ptr(&ip);
    let id = rand::random::<u16>();
    let consulta = construir_consulta_ptr(&nombre, id);
    let mut ultimo_error = None;
    for servidor in servidores_dns() {
        match enviar_consulta(servidor, &consulta) {
            Ok(respuesta) => return parsear_respuesta_ptr(&respuesta, id),
            Err(error) => ultimo_error = Some(error),
        }
    }
    Err(ultimo_error.unwrap_or_else(|| anyhow::anyhow!("no hay servidores DNS configurados")))
}

fn enviar_consulta(servidor: SocketAddr, consulta: &[u8]) -> Result<Vec<u8>> {
    let socket = UdpSocket::bind("0.0.0.0:0").context("no se pudo abrir el socket DNS")?;
    socket
        .set_read_timeout(Some(TIMEOUT))
        .context("no se pudo fijar el timeout DNS")?;
    socket
        .connect(servidor)
        .with_context(|| format!("no se pudo conectar con el DNS {servidor}"))?;
    socket
        .send(consulta)
        .context("no se pudo enviar la consulta DNS")?;
    let mut buffer = [0u8; 4096];
    let leidos = socket
        .recv(&mut buffer)
        .with_context(|| format!("sin respuesta del DNS {servidor}"))?;
    Ok(buffer[..leidos].to_vec())
}

fn servidores_dns() -> Vec<SocketAddr> {
    let mut servidores = Vec::new();
    if let Ok(contenido) = std::fs::read_to_string("/etc/resolv.conf") {
        for linea in contenido.lines() {
            let linea = linea.trim();
            let Some(resto) = linea.strip_prefix("nameserver") else {
                continue;
            };
            let Some(ip) = resto.split_whitespace().next() else {
                continue;
            };
            let Ok(ip) = ip.parse::<IpAddr>() else {
                continue;
            };
            servidores.push(SocketAddr::new(ip, PUERTO_DNS));
        }
    }
    if servidores.is_empty() {
        servidores.push(SocketAddr::new(
            "127.0.0.53".parse().expect("ip fija"),
            PUERTO_DNS,
        ));
    }
    servidores
}

/// Nombre PTR de una IP: octetos o nibbles invertidos + `.arpa`.
pub fn nombre_ptr(ip: &IpAddr) -> String {
    match ip {
        IpAddr::V4(ip) => {
            let octetos = ip.octets();
            format!(
                "{}.{}.{}.{}.in-addr.arpa",
                octetos[3], octetos[2], octetos[1], octetos[0]
            )
        }
        IpAddr::V6(ip) => {
            let mut nibbles = String::with_capacity(72);
            for byte in ip.octets().iter().rev() {
                nibbles.push_str(&format!("{:x}.{:x}.", byte & 0x0f, byte >> 4));
            }
            format!("{nibbles}ip6.arpa")
        }
    }
}

pub fn construir_consulta_ptr(nombre: &str, id: u16) -> Vec<u8> {
    let mut paquete = Vec::with_capacity(64);
    paquete.extend_from_slice(&id.to_be_bytes());
    paquete.extend_from_slice(&0x0100u16.to_be_bytes()); // RD
    paquete.extend_from_slice(&1u16.to_be_bytes()); // qdcount
    paquete.extend_from_slice(&0u16.to_be_bytes()); // ancount
    paquete.extend_from_slice(&0u16.to_be_bytes()); // nscount
    paquete.extend_from_slice(&0u16.to_be_bytes()); // arcount
    escribir_nombre(&mut paquete, nombre);
    paquete.extend_from_slice(&TIPO_PTR.to_be_bytes());
    paquete.extend_from_slice(&1u16.to_be_bytes()); // clase IN
    paquete
}

fn escribir_nombre(paquete: &mut Vec<u8>, nombre: &str) {
    for etiqueta in nombre.split('.').filter(|etiqueta| !etiqueta.is_empty()) {
        let bytes = etiqueta.as_bytes();
        paquete.push(bytes.len().min(63) as u8);
        paquete.extend_from_slice(&bytes[..bytes.len().min(63)]);
    }
    paquete.push(0);
}

/// Parsea una respuesta DNS y devuelve el primer PTR de la sección de
/// respuestas. Solo se usa con respuestas de nuestro propio servidor.
pub fn parsear_respuesta_ptr(datos: &[u8], id: u16) -> Result<Option<String>> {
    if datos.len() < 12 {
        bail!("respuesta DNS truncada");
    }
    let respuesta_id = u16::from_be_bytes([datos[0], datos[1]]);
    if respuesta_id != id {
        bail!("respuesta DNS con identificador inesperado");
    }
    let flags = u16::from_be_bytes([datos[2], datos[3]]);
    if flags & 0x000f != 0 {
        bail!("el DNS devolvió un error {}", flags & 0x000f);
    }
    let qdcount = u16::from_be_bytes([datos[4], datos[5]]) as usize;
    let ancount = u16::from_be_bytes([datos[6], datos[7]]) as usize;
    let mut offset = 12;
    for _ in 0..qdcount {
        let (_, siguiente) = leer_nombre(datos, offset)?;
        offset = siguiente + 4;
    }
    for _ in 0..ancount {
        let (_, siguiente) = leer_nombre(datos, offset)?;
        offset = siguiente;
        if offset + 10 > datos.len() {
            bail!("respuesta DNS truncada en el registro");
        }
        let tipo = u16::from_be_bytes([datos[offset], datos[offset + 1]]);
        let longitud = u16::from_be_bytes([datos[offset + 8], datos[offset + 9]]) as usize;
        let inicio_datos = offset + 10;
        let fin_datos = inicio_datos
            .checked_add(longitud)
            .context("longitud DNS desbordada")?;
        if fin_datos > datos.len() {
            bail!("respuesta DNS truncada en los datos");
        }
        if tipo == TIPO_PTR {
            let (nombre, _) = leer_nombre(datos, inicio_datos)?;
            return Ok(Some(nombre));
        }
        offset = fin_datos;
    }
    Ok(None)
}

/// Lee un nombre DNS (con punteros de compresión) desde `offset` y devuelve el
/// nombre y la posición siguiente a su primera aparición.
fn leer_nombre(datos: &[u8], offset: usize) -> Result<(String, usize)> {
    let mut nombre = String::new();
    let mut actual = offset;
    let mut siguiente = None;
    let mut saltos = 0;
    loop {
        if actual >= datos.len() {
            bail!("nombre DNS fuera de rango");
        }
        let longitud = datos[actual] as usize;
        if longitud == 0 {
            if siguiente.is_none() {
                siguiente = Some(actual + 1);
            }
            break;
        }
        if longitud & 0xc0 == 0xc0 {
            if actual + 1 >= datos.len() {
                bail!("puntero DNS truncado");
            }
            let destino = ((longitud & 0x3f) << 8) | datos[actual + 1] as usize;
            if siguiente.is_none() {
                siguiente = Some(actual + 2);
            }
            saltos += 1;
            if saltos > MAX_SALTOS {
                bail!("bucle de punteros DNS");
            }
            actual = destino;
            continue;
        }
        let inicio = actual + 1;
        let fin = inicio + longitud;
        if fin > datos.len() {
            bail!("etiqueta DNS truncada");
        }
        if !nombre.is_empty() {
            nombre.push('.');
        }
        nombre.push_str(&String::from_utf8_lossy(&datos[inicio..fin]));
        actual = fin;
    }
    Ok((nombre, siguiente.unwrap_or(actual)))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn respuesta_prueba(id: u16, nombre: &str) -> Vec<u8> {
        let mut paquete = Vec::new();
        paquete.extend_from_slice(&id.to_be_bytes());
        paquete.extend_from_slice(&0x8180u16.to_be_bytes());
        paquete.extend_from_slice(&1u16.to_be_bytes());
        paquete.extend_from_slice(&1u16.to_be_bytes());
        paquete.extend_from_slice(&0u16.to_be_bytes());
        paquete.extend_from_slice(&0u16.to_be_bytes());
        escribir_nombre(&mut paquete, "78.4.98.91.in-addr.arpa");
        paquete.extend_from_slice(&TIPO_PTR.to_be_bytes());
        paquete.extend_from_slice(&1u16.to_be_bytes());
        paquete.push(0xc0);
        paquete.push(0x0c);
        paquete.extend_from_slice(&TIPO_PTR.to_be_bytes());
        paquete.extend_from_slice(&1u16.to_be_bytes());
        paquete.extend_from_slice(&60u32.to_be_bytes());
        let mut datos = Vec::new();
        escribir_nombre(&mut datos, nombre);
        paquete.extend_from_slice(&(datos.len() as u16).to_be_bytes());
        paquete.extend_from_slice(&datos);
        paquete
    }

    #[test]
    fn nombre_ptr_invierte_octetos() {
        let ip: IpAddr = "91.98.4.78".parse().expect("ip");
        assert_eq!(nombre_ptr(&ip), "78.4.98.91.in-addr.arpa");
        let ip: IpAddr = "2a01:4f8:1c1d:699::1".parse().expect("ip6");
        assert!(nombre_ptr(&ip).ends_with(".ip6.arpa"));
        assert!(nombre_ptr(&ip).starts_with("1.0.0.0."));
    }

    #[test]
    fn construye_consulta_ptr() {
        let consulta = construir_consulta_ptr("78.4.98.91.in-addr.arpa", 0x1234);
        assert_eq!(&consulta[0..2], &[0x12, 0x34]);
        assert_eq!(u16::from_be_bytes([consulta[4], consulta[5]]), 1);
        assert_eq!(&consulta[12..14], &[2, b'7']);
        assert_eq!(
            &consulta[consulta.len() - 4..],
            &[0, 12, 0, 1],
            "la consulta debe terminar en QTYPE=PTR y QCLASS=IN"
        );
    }

    #[test]
    fn parsea_respuesta_con_compresion() {
        let paquete = respuesta_prueba(0xbeef, "de1.api.radio-browser.info");
        let nombre = parsear_respuesta_ptr(&paquete, 0xbeef).expect("parseo");
        assert_eq!(nombre.as_deref(), Some("de1.api.radio-browser.info"));
    }

    #[test]
    fn rechaza_respuestas_invalidas() {
        let paquete = respuesta_prueba(1, "de1.api.radio-browser.info");
        assert!(parsear_respuesta_ptr(&paquete, 2).is_err());
        assert!(parsear_respuesta_ptr(&[0, 1, 2], 0).is_err());
        let mut error = respuesta_prueba(7, "x");
        error[3] = 3; // NXDOMAIN
        assert!(parsear_respuesta_ptr(&error, 7).is_err());
    }
}
