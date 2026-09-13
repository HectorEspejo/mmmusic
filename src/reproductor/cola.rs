use rand::seq::SliceRandom;

use super::estado::Repeticion;
use crate::biblioteca::modelos::ElementoCola;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemCola {
    pub elemento: ElementoCola,
    pub posicion_orig: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultadoEliminar {
    QuitadaNoActual,
    QuitadaActual { detener: bool },
}

#[derive(Debug, Clone, Default)]
pub struct Cola {
    pub items: Vec<ItemCola>,
    pub indice: Option<usize>,
    pub aleatorio: bool,
    pub repeticion: Repeticion,
    siguiente_orig: u32,
}

impl Cola {
    pub fn nueva() -> Self {
        Self::default()
    }

    pub fn vacia(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn actual(&self) -> Option<&ItemCola> {
        self.indice.and_then(|indice| self.items.get(indice))
    }

    pub fn elementos(&self) -> Vec<ElementoCola> {
        self.items
            .iter()
            .map(|item| item.elemento.clone())
            .collect()
    }

    pub fn reemplazar(&mut self, elementos: Vec<ElementoCola>, indice: usize) {
        self.siguiente_orig = 0;
        self.items = elementos
            .into_iter()
            .enumerate()
            .map(|(posicion, elemento)| {
                let posicion_orig = posicion as u32;
                self.siguiente_orig = posicion_orig + 1;
                ItemCola {
                    elemento,
                    posicion_orig,
                }
            })
            .collect();
        self.indice = if self.items.is_empty() {
            None
        } else {
            Some(indice.min(self.items.len() - 1))
        };
        if self.aleatorio {
            self.barajar_manteniendo_actual();
        }
    }

    pub fn restaurar(&mut self, items: Vec<(ElementoCola, u32)>, indice: Option<usize>) {
        self.siguiente_orig = items
            .iter()
            .map(|(_, posicion_orig)| *posicion_orig + 1)
            .max()
            .unwrap_or(0);
        self.items = items
            .into_iter()
            .map(|(elemento, posicion_orig)| ItemCola {
                elemento,
                posicion_orig,
            })
            .collect();
        self.indice = indice.filter(|valor| *valor < self.items.len());
    }

    pub fn anadir_al_final(&mut self, elementos: Vec<ElementoCola>) -> bool {
        let estaba_vacia = self.items.is_empty();
        for elemento in elementos {
            self.items.push(ItemCola {
                elemento,
                posicion_orig: self.siguiente_orig,
            });
            self.siguiente_orig += 1;
        }
        if self.indice.is_none() {
            self.indice = if self.items.is_empty() { None } else { Some(0) };
        }
        estaba_vacia
    }

    pub fn reproducir_a_continuacion(&mut self, elementos: Vec<ElementoCola>) {
        let base = self.indice.map(|indice| indice + 1).unwrap_or(0);
        let nuevos: Vec<ItemCola> = elementos
            .into_iter()
            .map(|elemento| {
                let item = ItemCola {
                    elemento,
                    posicion_orig: self.siguiente_orig,
                };
                self.siguiente_orig += 1;
                item
            })
            .collect();
        let nuevos_len = nuevos.len();
        self.items.splice(base..base, nuevos);
        if self.indice.is_none() && nuevos_len > 0 {
            self.indice = Some(base);
        }
        if self.aleatorio {
            self.reordenar_aleatorio();
        }
    }

    pub fn eliminar(&mut self, posicion: usize) -> ResultadoEliminar {
        if posicion >= self.items.len() {
            return ResultadoEliminar::QuitadaNoActual;
        }
        self.items.remove(posicion);
        match self.indice {
            Some(indice) if indice == posicion => {
                if self.items.is_empty() || posicion >= self.items.len() {
                    self.indice = None;
                    ResultadoEliminar::QuitadaActual { detener: true }
                } else {
                    ResultadoEliminar::QuitadaActual { detener: false }
                }
            }
            Some(indice) if indice > posicion => {
                self.indice = Some(indice - 1);
                ResultadoEliminar::QuitadaNoActual
            }
            _ => ResultadoEliminar::QuitadaNoActual,
        }
    }

    pub fn mover(&mut self, de: usize, a: usize) {
        if de >= self.items.len() || a >= self.items.len() || de == a {
            return;
        }
        let item = self.items.remove(de);
        self.items.insert(a, item);
        if let Some(indice) = self.indice {
            self.indice = Some(mover_indice(indice, de, a));
        }
    }

    pub fn vaciar(&mut self) {
        self.items.clear();
        self.indice = None;
    }

    pub fn saltar_a(&mut self, posicion: usize) -> bool {
        if posicion < self.items.len() {
            self.indice = Some(posicion);
            true
        } else {
            false
        }
    }

    pub fn alternar_aleatorio(&mut self) {
        self.aleatorio = !self.aleatorio;
        if self.aleatorio {
            self.barajar_manteniendo_actual();
        } else {
            self.restaurar_orden_original();
        }
    }

    pub fn fijar_repeticion(&mut self, modo: Repeticion) {
        self.repeticion = modo;
    }

    pub fn ciclar_repeticion(&mut self) -> Repeticion {
        self.repeticion = self.repeticion.ciclar();
        self.repeticion
    }

    pub fn siguiente(&self) -> Option<usize> {
        let indice = self.indice?;
        if self.repeticion == Repeticion::Una {
            return Some(indice);
        }
        if indice + 1 < self.items.len() {
            Some(indice + 1)
        } else if self.repeticion == Repeticion::Todo {
            Some(0)
        } else {
            None
        }
    }

    /// Elemento anterior. Sobre una emisora siempre va al anterior; sobre una
    /// pista reinicia si ya han pasado más de tres segundos.
    pub fn anterior(&self, es_emisora: bool, posicion_ms: i64) -> Option<usize> {
        let indice = self.indice?;
        if !es_emisora && posicion_ms > 3000 {
            Some(indice)
        } else {
            Some(indice.saturating_sub(1))
        }
    }

    pub fn persistible(&self) -> Vec<(ElementoCola, u32)> {
        self.items
            .iter()
            .map(|item| (item.elemento.clone(), item.posicion_orig))
            .collect()
    }

    fn barajar_manteniendo_actual(&mut self) {
        let Some(indice) = self.indice else {
            let mut generador = rand::rng();
            self.items.shuffle(&mut generador);
            return;
        };
        if indice >= self.items.len() {
            return;
        }
        let actual = self.items.remove(indice);
        let mut generador = rand::rng();
        self.items.shuffle(&mut generador);
        self.items.insert(0, actual);
        self.indice = Some(0);
    }

    fn restaurar_orden_original(&mut self) {
        if self.items.is_empty() {
            self.indice = None;
            return;
        }
        let actual = self.actual().map(|item| item.elemento.clone());
        self.items.sort_by_key(|item| item.posicion_orig);
        self.indice = actual
            .and_then(|elemento| self.items.iter().position(|item| item.elemento == elemento));
    }

    fn reordenar_aleatorio(&mut self) {
        let actual = self.indice.and_then(|indice| self.items.get(indice));
        let mut resto: Vec<ItemCola> = self
            .items
            .iter()
            .filter(|item| Some(&item.elemento) != actual.map(|a| &a.elemento))
            .cloned()
            .collect();
        let mut generador = rand::rng();
        resto.shuffle(&mut generador);
        if let Some(actual) = actual.cloned() {
            resto.insert(0, actual);
            self.indice = Some(0);
        }
        self.items = resto;
    }
}

fn mover_indice(indice: usize, de: usize, a: usize) -> usize {
    if indice == de {
        a
    } else if de < indice && a >= indice {
        indice - 1
    } else if de > indice && a <= indice {
        indice + 1
    } else {
        indice
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::biblioteca::modelos::{EmisoraResumen, PistaResumen};

    fn pista(id: i64, titulo: &str) -> ElementoCola {
        ElementoCola::Pista(PistaResumen {
            id,
            titulo: titulo.to_string(),
            ..PistaResumen::default()
        })
    }

    fn emisora(id: i64, nombre: &str) -> ElementoCola {
        ElementoCola::Emisora(EmisoraResumen {
            id,
            nombre: nombre.to_string(),
            url: format!("http://emisora{id}.example/stream"),
            ..EmisoraResumen::default()
        })
    }

    fn cola_con(ids: &[i64]) -> Cola {
        let mut cola = Cola::nueva();
        cola.reemplazar(
            ids.iter().map(|id| pista(*id, &format!("P{id}"))).collect(),
            0,
        );
        cola
    }

    #[test]
    fn siguiente_respeta_repeticion() {
        let mut cola = cola_con(&[1, 2, 3]);
        cola.indice = Some(2);
        assert_eq!(cola.siguiente(), None);
        cola.fijar_repeticion(Repeticion::Todo);
        assert_eq!(cola.siguiente(), Some(0));
        cola.fijar_repeticion(Repeticion::Una);
        assert_eq!(cola.siguiente(), Some(2));
        cola.indice = Some(0);
        cola.fijar_repeticion(Repeticion::No);
        assert_eq!(cola.siguiente(), Some(1));
    }

    #[test]
    fn anterior_reinicia_pistas_pero_no_emisoras() {
        let mut cola = cola_con(&[1, 2, 3]);
        cola.indice = Some(2);
        assert_eq!(cola.anterior(false, 4_000), Some(2));
        assert_eq!(cola.anterior(false, 2_000), Some(1));
        assert_eq!(cola.anterior(true, 4_000), Some(1));
        cola.indice = Some(0);
        assert_eq!(cola.anterior(false, 1_000), Some(0));
    }

    #[test]
    fn aleatorio_conserva_la_actual_y_restaura_el_orden() {
        let mut cola = cola_con(&[1, 2, 3, 4, 5, 6, 7, 8]);
        cola.indice = Some(3);
        let actual = cola.actual().map(|item| item.elemento.clone());
        cola.alternar_aleatorio();
        assert!(cola.aleatorio);
        assert_eq!(cola.indice, Some(0));
        assert_eq!(cola.actual().map(|item| item.elemento.clone()), actual);
        let mut ids: Vec<i64> = cola
            .items
            .iter()
            .map(|item| item.elemento.pista_id().expect("pista"))
            .collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2, 3, 4, 5, 6, 7, 8]);

        cola.alternar_aleatorio();
        assert!(!cola.aleatorio);
        assert_eq!(cola.actual().map(|item| item.elemento.clone()), actual);
        let origenes: Vec<u32> = cola.items.iter().map(|item| item.posicion_orig).collect();
        assert_eq!(origenes, (0..8).collect::<Vec<u32>>());
        assert_eq!(cola.indice, Some(3));
    }

    #[test]
    fn reproducir_a_continuacion_inserta_tras_la_actual() {
        let mut cola = cola_con(&[1, 2, 3]);
        cola.indice = Some(0);
        cola.reproducir_a_continuacion(vec![pista(9, "N9"), pista(10, "N10")]);
        let ids: Vec<i64> = cola
            .items
            .iter()
            .map(|item| item.elemento.pista_id().expect("pista"))
            .collect();
        assert_eq!(ids, vec![1, 9, 10, 2, 3]);
        assert_eq!(cola.indice, Some(0));
        assert_eq!(cola.siguiente(), Some(1));
        cola.reproducir_a_continuacion(vec![emisora(4, "Radio")]);
        assert!(cola.items[1].elemento.es_emisora());
    }

    #[test]
    fn eliminar_la_actual_pasa_a_la_siguiente_o_detiene() {
        let mut cola = cola_con(&[1, 2, 3]);
        cola.indice = Some(1);
        let resultado = cola.eliminar(1);
        assert_eq!(
            resultado,
            ResultadoEliminar::QuitadaActual { detener: false }
        );
        assert_eq!(cola.indice, Some(1));
        assert_eq!(
            cola.actual().map(|item| item.elemento.pista_id()),
            Some(Some(3))
        );

        let resultado = cola.eliminar(1);
        assert_eq!(
            resultado,
            ResultadoEliminar::QuitadaActual { detener: true }
        );
        assert_eq!(cola.indice, None);
        assert_eq!(cola.len(), 1);

        let mut cola = cola_con(&[1, 2, 3]);
        cola.indice = Some(2);
        assert_eq!(cola.eliminar(0), ResultadoEliminar::QuitadaNoActual);
        assert_eq!(cola.indice, Some(1));
    }

    #[test]
    fn anadir_al_final_sobre_vacia_lo_indica() {
        let mut cola = Cola::nueva();
        assert!(cola.anadir_al_final(vec![pista(1, "A")]));
        assert_eq!(cola.indice, Some(0));
        assert!(!cola.anadir_al_final(vec![pista(2, "B")]));
        assert_eq!(cola.len(), 2);
    }

    #[test]
    fn mover_reajusta_el_indice() {
        let mut cola = cola_con(&[1, 2, 3, 4]);
        cola.indice = Some(2);
        cola.mover(3, 0);
        let ids: Vec<i64> = cola
            .items
            .iter()
            .map(|item| item.elemento.pista_id().expect("pista"))
            .collect();
        assert_eq!(ids, vec![4, 1, 2, 3]);
        assert_eq!(
            cola.actual().map(|item| item.elemento.pista_id()),
            Some(Some(3))
        );
    }

    #[test]
    fn persistible_y_restaurar_conservan_orden_original() {
        let mut cola = Cola::nueva();
        cola.reemplazar(vec![pista(1, "A"), emisora(2, "Radio"), pista(3, "C")], 0);
        cola.alternar_aleatorio();
        let guardado = cola.persistible();
        let actual = cola.actual().map(|item| item.elemento.clone());
        let mut restaurada = Cola::nueva();
        restaurada.aleatorio = true;
        restaurada.restaurar(guardado, Some(0));
        assert_eq!(
            restaurada.actual().map(|item| item.elemento.clone()),
            actual
        );
        assert_eq!(restaurada.len(), 3);
    }
}
