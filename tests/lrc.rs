use mmmusic::letras::{lrc, sincronia};

/// AC del checklist: dos timestamps en una línea y `[offset:-200]`.
#[test]
fn timestamps_multiples_y_offset() {
    let texto = "[offset:-200]\n[00:12.50]hola\n[00:20.00][00:40.00]estribillo\n";
    let lrc = lrc::parsear(texto);
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
fn metadatos_enhanced_y_lineas_vacias() {
    let texto = "\u{feff}[ar:ESPRIT 空想]\r\n[ti:Neon]\r\n[00:05.00]\r\n[00:10.00]<00:10.00>Hola <00:10.50>mundo\r\n";
    let lrc = lrc::parsear(texto);
    assert_eq!(lrc.offset_ms, 0);
    assert_eq!(lrc.lineas[0], (5_000, String::new()));
    assert_eq!(lrc.lineas[1], (10_000, "Hola mundo".to_string()));
    assert_eq!(lrc.lineas.len(), 2);
}

#[test]
fn ordena_por_tiempo() {
    let lrc = lrc::parsear("[00:30.00]tres\n[00:10.00]uno\n[00:20.00]dos\n");
    let tiempos: Vec<u32> = lrc.lineas.iter().map(|(ms, _)| *ms).collect();
    assert_eq!(tiempos, vec![10_000, 20_000, 30_000]);
}

#[test]
fn detecta_sincronizacion_en_cualquier_texto() {
    assert!(lrc::es_sincronizada("[00:01.00]hola"));
    assert!(lrc::es_sincronizada("[ti:x]\nletra\n[00:01]hola"));
    assert!(!lrc::es_sincronizada("letra plana\nsin marcas"));
    assert!(!lrc::es_sincronizada("<00:01.00>enhanced suelto"));
}

#[test]
fn sincronia_por_busqueda_binaria() {
    let lrc = lrc::parsear("[00:00.00]a\n[00:01.00]b\n[00:02.50]c\n[00:02.50]c bis\n[00:05.00]d\n");
    let lineas = &lrc.lineas;
    assert_eq!(sincronia::linea_actual(lineas, -1), -1);
    assert_eq!(sincronia::linea_actual(lineas, 0), 0);
    assert_eq!(sincronia::linea_actual(lineas, 999), 0);
    assert_eq!(sincronia::linea_actual(lineas, 1_000), 1);
    assert_eq!(sincronia::linea_actual(lineas, 2_999), 3);
    assert_eq!(sincronia::linea_actual(lineas, 100_000), 4);
}
