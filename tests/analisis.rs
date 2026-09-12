use mmmusic::audio::{self, Analisis, Analizador, Anillo, EstadoCaptura};

const TASA: u32 = 48_000;

fn seno(frecuencia: f64, amplitud: f32, fase: f64) -> f32 {
    (amplitud as f64 * (2.0 * std::f64::consts::PI * frecuencia * fase / TASA as f64).sin()) as f32
}

fn llenar_anillo(anillo: &Anillo, frecuencia: f64, amplitud: f32, desde: u64, frames: usize) {
    let mut datos = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let fase = (desde + i as u64) as f64;
        let valor = seno(frecuencia, amplitud, fase);
        datos.push(valor);
        datos.push(valor);
    }
    anillo.escribir(&datos);
}

fn banda_con_maximo(analisis: &Analisis) -> usize {
    analisis
        .bandas
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(indice, _)| indice)
        .unwrap_or(0)
}

#[test]
fn seno_de_1khz_cae_en_la_banda_correcta() {
    let anillo = Anillo::nuevo();
    let mut analizador = Analizador::nuevo();
    let frames = 2048 * 4;
    llenar_anillo(&anillo, 1000.0, 0.5, 0, frames);
    let analisis = analizador.procesar(&anillo, 128, 1.0, TASA, 0, false, false);
    assert!(!analisis.ambiental);
    assert_eq!(analisis.bandas.len(), 64);

    let limites = audio::analisis::limites_bandas(analisis.bandas.len());
    let indice = banda_con_maximo(&analisis);
    let inferior = limites[indice];
    let superior = limites[indice + 1];
    assert!(
        inferior <= 1000.0 && 1000.0 < superior,
        "el máximo está en {inferior:.0}-{superior:.0} Hz"
    );

    let maximo = analisis.bandas[indice];
    assert!(maximo > 0.5, "el seno debe dar una banda viva: {maximo}");
    for (i, valor) in analisis.bandas.iter().enumerate() {
        if i != indice {
            assert!(
                *valor <= 0.3 * maximo,
                "la banda {i} ({valor}) supera el 30 % del máximo {maximo}"
            );
        }
    }
}

#[test]
fn silencio_da_bandas_a_cero() {
    let anillo = Anillo::nuevo();
    let mut analizador = Analizador::nuevo();
    llenar_anillo(&anillo, 440.0, 0.0, 0, 4096);
    let analisis = analizador.procesar(&anillo, 128, 1.0, TASA, 0, false, false);
    assert!(analisis.bandas.iter().all(|valor| *valor == 0.0));
    assert_eq!(analisis.rms, [0.0, 0.0]);
}

#[test]
fn pulso_periodico_se_detecta() {
    let anillo = Anillo::nuevo();
    let mut analizador = Analizador::nuevo();
    let mut fase = 0u64;
    let mut pulsos = 0;
    let mut ultimo_pulso: Option<u64> = None;
    for frame in 0..90u64 {
        let t_ms = frame * 33;
        let fuerte = (frame % 15) < 3;
        let (frecuencia, amplitud) = if fuerte { (60.0, 0.9) } else { (4000.0, 0.02) };
        llenar_anillo(&anillo, frecuencia, amplitud, fase, 2048);
        fase += 2048;
        let analisis = analizador.procesar(&anillo, 32, 1.0, TASA, t_ms, false, false);
        if analisis.pulso {
            if let Some(anterior) = ultimo_pulso {
                assert!(
                    t_ms - anterior >= 250,
                    "dos pulsos demasiado juntos: {anterior} y {t_ms}"
                );
            }
            ultimo_pulso = Some(t_ms);
            pulsos += 1;
        }
    }
    assert!(pulsos >= 3, "se esperaban varios pulsos, hubo {pulsos}");
}

#[test]
fn agc_converge_sin_dispararse() {
    let anillo = Anillo::nuevo();
    let mut analizador = Analizador::nuevo();
    let mut fase = 0u64;
    let mut anterior = analizador.ganancia();
    let mut delta_final = 0.0;
    for frame in 0..120u64 {
        llenar_anillo(&anillo, 200.0, 0.02, fase, 2048);
        fase += 2048;
        let analisis = analizador.procesar(&anillo, 128, 1.0, TASA, frame * 33, false, false);
        assert!(
            analisis
                .bandas
                .iter()
                .all(|valor| (0.0..=1.0).contains(valor))
        );
        let actual = analizador.ganancia();
        assert!((0.5..=8.0).contains(&actual));
        delta_final = (actual - anterior).abs();
        anterior = actual;
    }
    assert!(
        delta_final < 0.05,
        "el AGC no se estabilizó (último salto {delta_final})"
    );
    assert!(anterior > 1.0, "debe amplificar una señal débil");
}

#[test]
fn sensibilidad_multiplica_la_ganancia() {
    let anillo = Anillo::nuevo();
    let mut suave = Analizador::nuevo();
    let mut fuerte = Analizador::nuevo();
    llenar_anillo(&anillo, 1000.0, 0.01, 0, 4096);
    let bajo = suave.procesar(&anillo, 128, 0.5, TASA, 0, false, false);
    let alto = fuerte.procesar(&anillo, 128, 4.0, TASA, 0, false, false);
    let max_bajo = bajo.bandas.iter().copied().fold(0.0f32, f32::max);
    let max_alto = alto.bandas.iter().copied().fold(0.0f32, f32::max);
    assert!(max_alto >= max_bajo);
}

#[test]
fn modo_ambiental_sustituye_la_senal() {
    let anillo = Anillo::nuevo();
    let mut analizador = Analizador::nuevo();
    llenar_anillo(&anillo, 1000.0, 0.5, 0, 2048);
    let analisis = analizador.procesar(&anillo, 128, 1.0, TASA, 0, true, false);
    assert!(analisis.ambiental);
    assert_eq!(analisis.tasa_hz, TASA);
    assert!(analisis.bandas.iter().any(|valor| *valor > 0.0));
    assert!(!analisis.pulso);
}

#[test]
fn pausa_larga_entra_en_ambiental() {
    let anillo = Anillo::nuevo();
    let mut analizador = Analizador::nuevo();
    llenar_anillo(&anillo, 1000.0, 0.5, 0, 4096);
    let inicial = analizador.procesar(&anillo, 128, 1.0, TASA, 0, false, true);
    assert!(!inicial.ambiental);
    let despues = analizador.procesar(&anillo, 128, 1.0, TASA, 2500, false, true);
    assert!(despues.ambiental);
}

#[test]
fn ancho_acota_las_bandas() {
    let anillo = Anillo::nuevo();
    let mut analizador = Analizador::nuevo();
    llenar_anillo(&anillo, 1000.0, 0.5, 0, 4096);
    assert_eq!(
        analizador
            .procesar(&anillo, 10, 1.0, TASA, 0, false, false)
            .bandas
            .len(),
        16
    );
    assert_eq!(
        analizador
            .procesar(&anillo, 400, 1.0, TASA, 33, false, false)
            .bandas
            .len(),
        128
    );
}

#[test]
fn estados_de_captura_tienen_etiqueta() {
    assert_eq!(EstadoCaptura::SinAudio.etiqueta(), "sin audio");
    assert!(
        EstadoCaptura::Capturando {
            tasa_hz: 48000,
            canales: 2,
            monitor: true
        }
        .por_monitor()
    );
}
