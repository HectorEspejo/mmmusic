use std::net::IpAddr;

use anyhow::{Context, Result, bail};
use dns_lookup::{lookup_addr, lookup_host};
use tracing::{debug, warn};

pub const HOST: &str = "all.api.radio-browser.info";
const SUFIJO: &str = ".api.radio-browser.info";

/// Resuelve los espejos de Radio Browser: DNS directo de
/// `all.api.radio-browser.info` con `lookup_host` y DNS inverso (PTR) de cada
/// IP con `lookup_addr`. Si no queda ningún nombre válido, se usa el nombre
/// agregado como único espejo.
pub fn resolver_espejos() -> Result<Vec<String>> {
    let direcciones = lookup_host(HOST).with_context(|| format!("no se pudo resolver {HOST}"))?;
    let mut ips: Vec<IpAddr> = direcciones.collect();
    ips.sort();
    ips.dedup();
    if ips.is_empty() {
        bail!("{HOST} no devolvió direcciones");
    }
    let mut candidatos = Vec::new();
    for ip in ips {
        if let Ok(Some(nombre)) = consultar_ptr(ip) {
            candidatos.push(nombre);
        }
    }
    Ok(depurar_nombres(candidatos))
}

/// Nombre inverso (PTR) de una IP. Devuelve `None` cuando no existe.
pub fn consultar_ptr(ip: IpAddr) -> Result<Option<String>> {
    match lookup_addr(&ip) {
        Ok(nombre) => Ok(Some(nombre)),
        Err(error) => {
            debug!(%ip, "sin nombre inverso: {error:?}");
            Ok(None)
        }
    }
}

/// Filtra los nombres de espejo, elimina duplicados y recurre al nombre
/// agregado cuando no queda ninguno.
fn depurar_nombres(candidatos: Vec<String>) -> Vec<String> {
    let mut nombres = Vec::new();
    for nombre in candidatos {
        if es_nombre_espejo(&nombre) && !nombres.contains(&nombre) {
            nombres.push(nombre);
        }
    }
    if nombres.is_empty() {
        warn!("sin nombres de espejo por DNS inverso; se usa {HOST}");
        nombres.push(HOST.to_string());
    }
    nombres
}

/// Un espejo es un subdominio con etiqueta (no el propio dominio agregado).
fn es_nombre_espejo(nombre: &str) -> bool {
    nombre
        .strip_suffix(SUFIJO)
        .is_some_and(|etiqueta| !etiqueta.is_empty())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn filtra_y_deduplica_nombres() {
        let nombres = depurar_nombres(vec![
            "de1.api.radio-browser.info".to_string(),
            "de1.api.radio-browser.info".to_string(),
            "otro.example.com".to_string(),
            "api.radio-browser.info".to_string(),
        ]);
        assert_eq!(nombres, vec!["de1.api.radio-browser.info"]);
    }

    #[test]
    fn sin_candidatos_usa_el_agregado() {
        assert_eq!(depurar_nombres(Vec::new()), vec![HOST.to_string()]);
        assert_eq!(
            depurar_nombres(vec!["ejemplo.org".to_string()]),
            vec![HOST.to_string()]
        );
    }

    #[test]
    fn reconoce_el_sufijo() {
        assert!(es_nombre_espejo("fr1.api.radio-browser.info"));
        assert!(!es_nombre_espejo("api.radio-browser.info"));
        assert!(!es_nombre_espejo("radio-browser.info"));
        assert!(!es_nombre_espejo(""));
    }
}
