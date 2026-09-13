use mmmusic::ecualizador::cadena::{self, BANDAS_HZ, Q, TECHO_LIMITADOR};
use mmmusic::ecualizador::presets;
use mmmusic::ecualizador::{EstadoEq, acotar_db, acotar_preamp};

fn eq_con_valores() -> EstadoEq {
    let mut eq = EstadoEq {
        activo: true,
        preamp_db: -2.0,
        ..EstadoEq::default()
    };
    eq.ganancias = [5.0, 4.0, 2.0, -1.0, -2.0, 0.0, 1.0, 3.0, 4.0, 4.0];
    eq
}

#[test]
fn cadena_etiquetada_completa() {
    let mut eq = eq_con_valores();
    eq.limitador = true;
    let cadena = cadena::construir(&eq);
    let partes: Vec<&str> = cadena.split(',').collect();
    assert_eq!(partes.len(), 12);
    assert_eq!(partes[0], "@pre:lavfi=[volume=volume=-2.0dB]");
    for (indice, hz) in BANDAS_HZ.iter().enumerate() {
        assert!(
            partes[indice + 1].starts_with(&format!("@eq{}:lavfi=[equalizer=f={hz}", indice + 1)),
            "etiqueta de la banda {hz} Hz"
        );
    }
    assert_eq!(
        partes[11],
        format!("@lim:lavfi=[alimiter=limit={TECHO_LIMITADOR:.3}:attack=5:release=50:level=false]")
    );
    assert!(cadena.contains(&format!("width_type=q:width={Q:.2}")));
    assert!(cadena.contains("gain=5.0"));
    assert!(cadena.contains("gain=4.0"));
}

#[test]
fn sin_limitador_la_cadena_termina_en_eq10() {
    let cadena = cadena::construir(&eq_con_valores());
    assert!(!cadena.contains("@lim"));
    assert!(cadena.ends_with("@eq10:lavfi=[equalizer=f=16000:width_type=q:width=1.41:gain=4.0]"));
}

#[test]
fn comandos_de_banda_y_preamp() {
    assert_eq!(
        cadena::comando_banda(0, 3.0),
        ("@eq1".to_string(), "g", "3.0".to_string(), "equalizer")
    );
    assert_eq!(
        cadena::comando_banda(9, -0.5),
        ("@eq10".to_string(), "g", "-0.5".to_string(), "equalizer")
    );
    assert_eq!(
        cadena::comando_preamp(-1.5),
        ("@pre", "volume", "-1.5dB".to_string(), "volume")
    );
}

#[test]
fn bypass_con_eq_apagado_o_curva_plana() {
    let mut eq = EstadoEq {
        activo: true,
        ..EstadoEq::default()
    };
    assert!(cadena::es_bypass(&eq));
    eq.limitador = true;
    assert!(!cadena::es_bypass(&eq));
    eq.limitador = false;
    eq.preamp_db = -0.5;
    assert!(!cadena::es_bypass(&eq));
    eq.preamp_db = 0.0;
    eq.ganancias[2] = 0.5;
    assert!(!cadena::es_bypass(&eq));
    eq.activo = false;
    assert!(cadena::es_bypass(&eq));
}

#[test]
fn presets_integrados_del_informe() {
    let esperados: [(&str, [f32; 10], f32); 9] = [
        ("Plano", [0.0; 10], 0.0),
        (
            "Rock",
            [5.0, 4.0, 2.0, -1.0, -2.0, 0.0, 1.0, 3.0, 4.0, 4.0],
            -2.0,
        ),
        (
            "Pop",
            [-1.0, 1.0, 3.0, 4.0, 3.0, 0.0, -1.0, -1.0, 1.0, 2.0],
            -1.0,
        ),
        (
            "Electrónica",
            [4.0, 3.0, 1.0, 0.0, -2.0, 1.0, 0.0, 1.0, 3.0, 4.0],
            -2.0,
        ),
        (
            "Hip-hop",
            [5.0, 4.0, 1.0, 2.0, -1.0, -1.0, 1.0, 0.0, 1.0, 2.0],
            -2.0,
        ),
        (
            "Vocal",
            [-2.0, -3.0, -2.0, 1.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0],
            0.0,
        ),
        (
            "Bass boost",
            [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            -3.0,
        ),
        (
            "Treble boost",
            [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0],
            -3.0,
        ),
        (
            "Loudness",
            [5.0, 3.0, 0.0, -1.0, -2.0, -2.0, -1.0, 1.0, 3.0, 5.0],
            -3.0,
        ),
    ];
    assert_eq!(presets::INTEGRADOS.len(), esperados.len());
    for (nombre, ganancias, preamp) in esperados {
        let preset = presets::INTEGRADOS
            .iter()
            .find(|preset| preset.nombre == nombre)
            .unwrap_or_else(|| panic!("falta el preset {nombre}"));
        assert_eq!(preset.ganancias, ganancias, "bandas de {nombre}");
        assert_eq!(preset.preamp_db, preamp, "preamp de {nombre}");
    }
}

#[test]
fn ganancias_se_formatean_y_recuperan() {
    let ganancias = [1.0, -2.5, 0.0, 12.0, -12.0, 0.5, 3.0, 4.0, 5.0, 6.0];
    let texto = presets::formatear_ganancias(&ganancias);
    assert_eq!(texto, "1.0,-2.5,0.0,12.0,-12.0,0.5,3.0,4.0,5.0,6.0");
    assert_eq!(presets::parsear_ganancias(&texto), Some(ganancias));
    assert_eq!(presets::parsear_ganancias("1.0,2.0"), None);
    assert_eq!(presets::parsear_ganancias("a,b,c,d,e,f,g,h,i,j"), None);
}

#[test]
fn acotado_y_paso_de_medio_db() {
    assert_eq!(acotar_db(50.0), 12.0);
    assert_eq!(acotar_db(-50.0), -12.0);
    assert_eq!(acotar_db(2.24), 2.0);
    assert_eq!(acotar_db(2.26), 2.5);
    assert_eq!(acotar_preamp(7.26), 7.5);
    assert_eq!(acotar_preamp(-99.0), -12.0);
}

#[test]
fn mpv_acepta_la_cadena_etiquetada_y_los_comandos_en_caliente() {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use mmmusic::reproductor::mpv::{EventoMpv, ReproductorMpv};
    let Ok(mpv) = ReproductorMpv::nuevo(50) else {
        return;
    };
    if !mpv.lavfi_disponible() {
        return;
    }
    let mut eq = eq_con_valores();
    eq.limitador = true;
    mpv.fijar_af(&cadena::construir(&eq))
        .expect("mpv debe aceptar la cadena");
    let ruta = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/prueba.flac");
    mpv.cargar(&ruta.display().to_string()).expect("cargar");
    let limite = Instant::now() + Duration::from_secs(10);
    while Instant::now() < limite {
        if matches!(mpv.esperar_evento(0.1), Some(EventoMpv::Cargado)) {
            break;
        }
    }
    // El grafo de filtros se inicializa al arrancar el audio.
    std::thread::sleep(Duration::from_millis(500));
    let (etiqueta, comando, valor, filtro) = cadena::comando_banda(0, 4.0);
    mpv.af_command(&etiqueta, comando, &valor, filtro)
        .expect("af-command de banda sin reconstruir");
    let (etiqueta, comando, valor, filtro) = cadena::comando_preamp(-2.0);
    mpv.af_command(etiqueta, comando, &valor, filtro)
        .expect("af-command de preamp sin reconstruir");
    mpv.fijar_af("").expect("bypass");
}
