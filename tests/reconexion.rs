use std::time::Duration;

use mmmusic::radio::reconexion::{ESPERAS, MAX_INTENTOS, PlanReconexion, siguiente_espera};

#[test]
fn esperas_crecientes_hasta_rendirse() {
    assert_eq!(siguiente_espera(0), None);
    for (indice, segundos) in ESPERAS.iter().enumerate() {
        assert_eq!(
            siguiente_espera(indice as u32 + 1),
            Some(Duration::from_secs(*segundos))
        );
    }
    assert_eq!(siguiente_espera(MAX_INTENTOS + 1), None);
}

#[test]
fn seis_fallos_agotan_el_plan() {
    let mut plan = PlanReconexion::nuevo(1);
    for intento in 1..=MAX_INTENTOS {
        assert_eq!(plan.registrar_fallo(), Some(intento));
    }
    assert_eq!(plan.registrar_fallo(), None, "el séptimo fallo se rinde");
    assert_eq!(plan.intento(), MAX_INTENTOS);
}

#[test]
fn avanza_de_espejo_y_vuelve_al_primero() {
    let mut plan = PlanReconexion::nuevo(3);
    assert_eq!(plan.espejo(), 0);
    plan.registrar_fallo();
    assert_eq!(plan.espejo(), 1);
    plan.registrar_fallo();
    assert_eq!(plan.espejo(), 2);
    plan.registrar_fallo();
    assert_eq!(plan.espejo(), 0);
}

#[test]
fn reiniciar_al_llegar_a_directo() {
    let mut plan = PlanReconexion::nuevo(2);
    plan.registrar_fallo();
    plan.registrar_fallo();
    plan.reiniciar();
    assert_eq!(plan.intento(), 0);
    assert_eq!(plan.registrar_fallo(), Some(1));
}
