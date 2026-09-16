//! Geometría 3D de la visual «Ciudad»: cámara en órbita, proyección en
//! perspectiva y descarte de caras ocultas. Sin E/S: todo se prueba con
//! valores sintéticos.

use std::f64::consts::TAU;

/// Campo de visión vertical de la cámara, en grados.
pub const FOV_GRADOS: f64 = 55.0;
/// Radio de la órbita, en múltiplos del mayor de los dos lados de la ciudad.
pub const RADIO_ORBITA: f64 = 0.9;
/// Altura de la cámara sobre el suelo, en múltiplos del ancho de la ciudad.
pub const ALTURA_BASE: f64 = 0.55;
/// Balanceo vertical por RMS, en múltiplos del ancho de la ciudad.
pub const BALANCEO_RMS: f64 = 0.08;
/// Altura a la que mira la cámara, en múltiplos del ancho de la ciudad.
pub const OBJETIVO_Y: f64 = 0.15;
/// Distancia mínima de proyección, en unidades de mundo.
pub const PLANO_CERCANO: f64 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Punto3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Punto3 {
    pub const fn nuevo(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn resta(self, otro: Punto3) -> Punto3 {
        Punto3::nuevo(self.x - otro.x, self.y - otro.y, self.z - otro.z)
    }

    pub fn escalado(self, factor: f64) -> Punto3 {
        Punto3::nuevo(self.x * factor, self.y * factor, self.z * factor)
    }

    /// Producto escalar.
    pub fn punto(self, otro: Punto3) -> f64 {
        self.x * otro.x + self.y * otro.y + self.z * otro.z
    }

    /// Producto vectorial.
    pub fn cruz(self, otro: Punto3) -> Punto3 {
        Punto3::nuevo(
            self.y * otro.z - self.z * otro.y,
            self.z * otro.x - self.x * otro.z,
            self.x * otro.y - self.y * otro.x,
        )
    }

    pub fn longitud(self) -> f64 {
        self.punto(self).sqrt()
    }

    /// Versor; devuelve el propio vector si su longitud es despreciable.
    pub fn normalizado(self) -> Punto3 {
        let longitud = self.longitud();
        if longitud < 1e-9 {
            self
        } else {
            self.escalado(1.0 / longitud)
        }
    }

    pub fn distancia(self, otro: Punto3) -> f64 {
        self.resta(otro).longitud()
    }
}

/// Punto proyectado, en coordenadas normalizadas de cámara (el centro de la
/// pantalla es el origen y el campo de visión abarca ±1 en vertical).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Punto2 {
    pub x: f64,
    pub y: f64,
}

impl Punto2 {
    pub const fn nuevo(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Tamaño de la ciudad en unidades de mundo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dimensiones {
    pub ancho: f64,
    pub profundidad: f64,
}

/// Una cara de un prisma: sus vértices, la normal hacia fuera y los índices de
/// esos vértices dentro del prisma (para deduplicar las aristas compartidas).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cara {
    pub indices: [usize; 4],
    pub vertices: [Punto3; 4],
    pub normal: Punto3,
}

impl Cara {
    /// Construye la cara a partir de sus vértices en orden de devanado; la
    /// normal se deduce de los tres primeros.
    pub fn nueva(indices: [usize; 4], vertices: [Punto3; 4]) -> Self {
        let normal = vertices[1]
            .resta(vertices[0])
            .cruz(vertices[2].resta(vertices[0]))
            .normalizado();
        Self {
            indices,
            vertices,
            normal,
        }
    }

    /// Las cuatro aristas como pares de índices del prisma, en ciclo.
    pub fn aristas(&self) -> [(usize, usize); 4] {
        let [a, b, c, d] = self.indices;
        [(a, b), (b, c), (c, d), (d, a)]
    }

    pub fn centro(&self) -> Punto3 {
        let suma = self
            .vertices
            .iter()
            .fold(Punto3::nuevo(0.0, 0.0, 0.0), |acumulado, vertice| {
                Punto3::nuevo(
                    acumulado.x + vertice.x,
                    acumulado.y + vertice.y,
                    acumulado.z + vertice.z,
                )
            });
        suma.escalado(0.25)
    }
}

/// Cámara en perspectiva con base ortonormal y campo de visión fijo.
#[derive(Debug, Clone, Copy)]
pub struct Camara {
    pub ojo: Punto3,
    pub derecha: Punto3,
    pub arriba: Punto3,
    pub adelante: Punto3,
    /// `1 / tan(fov/2)`, la distancia focal en unidades normalizadas.
    pub escala: f64,
}

impl Camara {
    /// Cámara mirando a `objetivo` con el arriba mundial `(0, 1, 0)`.
    pub fn mirando_a(ojo: Punto3, objetivo: Punto3) -> Self {
        let adelante = objetivo.resta(ojo).normalizado();
        let derecha = adelante.cruz(Punto3::nuevo(0.0, 1.0, 0.0)).normalizado();
        let arriba = derecha.cruz(adelante);
        Self {
            ojo,
            derecha,
            arriba,
            adelante,
            escala: 1.0 / (FOV_GRADOS.to_radians() / 2.0).tan(),
        }
    }

    /// Cámara en órbita alrededor del centro de la ciudad (situado en el
    /// origen): una vuelta cada `vuelta_s` segundos, a `RADIO_ORBITA` del
    /// centro y a `ALTURA_BASE` del suelo más el balanceo por RMS.
    pub fn orbita(t: f64, rms: f64, dims: Dimensiones, vuelta_s: f64) -> Self {
        let vuelta = vuelta_s.max(1.0);
        let giro = TAU * t.max(0.0).rem_euclid(vuelta) / vuelta;
        let radio = RADIO_ORBITA * dims.ancho.max(dims.profundidad);
        let balanceo = BALANCEO_RMS * dims.ancho * rms.clamp(0.0, 1.0);
        let ojo = Punto3::nuevo(
            radio * giro.cos(),
            ALTURA_BASE * dims.ancho + balanceo,
            radio * giro.sin(),
        );
        let objetivo = Punto3::nuevo(0.0, OBJETIVO_Y * dims.ancho, 0.0);
        Camara::mirando_a(ojo, objetivo)
    }
}

/// Caras de un prisma recto, como índices de sus ocho vértices: base (0-3, en
/// sentido antihorario visto desde arriba) y tapa (4-7, encima de los
/// anteriores). El devanado deja la normal hacia fuera en todas.
pub const CARAS_PRISMA: [[usize; 4]; 6] = [
    [0, 1, 2, 3],
    [4, 7, 6, 5],
    [0, 4, 5, 1],
    [1, 5, 6, 2],
    [2, 6, 7, 3],
    [3, 7, 4, 0],
];

/// Las seis caras de un prisma recto a partir de sus vértices.
pub fn caras_de_prisma(vertices: &[Punto3; 8]) -> [Cara; 6] {
    CARAS_PRISMA.map(|indices| {
        Cara::nueva(
            indices,
            [
                vertices[indices[0]],
                vertices[indices[1]],
                vertices[indices[2]],
                vertices[indices[3]],
            ],
        )
    })
}

/// Proyección en perspectiva. Devuelve `None` si el punto queda detrás del
/// plano cercano o en él.
pub fn proyectar(camara: &Camara, punto: Punto3) -> Option<Punto2> {
    let relativo = punto.resta(camara.ojo);
    let profundidad = relativo.punto(camara.adelante);
    if profundidad <= PLANO_CERCANO {
        return None;
    }
    Some(Punto2::nuevo(
        relativo.punto(camara.derecha) / profundidad * camara.escala,
        relativo.punto(camara.arriba) / profundidad * camara.escala,
    ))
}

/// Descarte de caras ocultas: una cara se ve cuando su normal mira a la cámara.
pub fn cara_visible(camara: &Camara, cara: &Cara) -> bool {
    cara.normal.punto(camara.ojo.resta(cara.centro())) > 0.0
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const DIMS: Dimensiones = Dimensiones {
        ancho: 12.0,
        profundidad: 8.0,
    };

    #[test]
    fn la_orbita_repite_cada_vuelta_y_respeta_radio_y_altura() {
        let inicio = Camara::orbita(0.0, 0.0, DIMS, 20.0);
        let vuelta = Camara::orbita(20.0, 0.0, DIMS, 20.0);
        assert!((inicio.ojo.x - vuelta.ojo.x).abs() < 1e-9);
        assert!((inicio.ojo.z - vuelta.ojo.z).abs() < 1e-9);

        let radio = (inicio.ojo.x.powi(2) + inicio.ojo.z.powi(2)).sqrt();
        assert!((radio - RADIO_ORBITA * DIMS.ancho).abs() < 1e-9);
        assert!((inicio.ojo.y - ALTURA_BASE * DIMS.ancho).abs() < 1e-9);

        let con_rms = Camara::orbita(0.0, 1.0, DIMS, 20.0);
        let esperado = ALTURA_BASE * DIMS.ancho + BALANCEO_RMS * DIMS.ancho;
        assert!((con_rms.ojo.y - esperado).abs() < 1e-9);
    }

    #[test]
    fn la_ciudad_se_proyecta_al_centro_de_la_pantalla() {
        let camara = Camara::orbita(3.0, 0.2, DIMS, 20.0);
        let objetivo = Punto3::nuevo(0.0, OBJETIVO_Y * DIMS.ancho, 0.0);
        let centro = proyectar(&camara, objetivo).expect("el objetivo se proyecta");
        assert!(centro.x.abs() < 1e-9);
        assert!(centro.y.abs() < 1e-9);
    }

    #[test]
    fn lo_que_esta_detras_del_plano_cercano_no_se_proyecta() {
        let camara = Camara::orbita(0.0, 0.0, DIMS, 20.0);
        let detras = camara.ojo.resta(camara.adelante.escalado(1.0));
        assert!(proyectar(&camara, detras).is_none());
        let delante = camara.ojo;
        assert!(proyectar(&camara, delante).is_none());
    }

    #[test]
    fn de_dos_caras_paralelas_opuestas_solo_se_ve_una() {
        let prisma = prisma_de_prueba();
        for paso in 0..8 {
            let camara = Camara::orbita(paso as f64 * 2.5, 0.0, DIMS, 20.0);
            let visibles: Vec<usize> = prisma
                .iter()
                .enumerate()
                .filter(|(_, cara)| cara_visible(&camara, cara))
                .map(|(indice, _)| indice)
                .collect();
            // Cara 2 (−z) y cara 4 (+z) son opuestas: nunca se ven las dos.
            assert!(
                !(visibles.contains(&2) && visibles.contains(&4)),
                "las caras opuestas no pueden verse a la vez"
            );
            // La base (normal hacia abajo) nunca se ve desde la órbita.
            assert!(!visibles.contains(&0), "la base no debe verse");
        }
    }

    #[test]
    fn todas_las_normales_apuntan_hacia_fuera() {
        let prisma = prisma_de_prueba();
        let centro = prisma
            .iter()
            .flat_map(|cara| cara.vertices)
            .fold(Punto3::nuevo(0.0, 0.0, 0.0), |acumulado, vertice| {
                Punto3::nuevo(
                    acumulado.x + vertice.x,
                    acumulado.y + vertice.y,
                    acumulado.z + vertice.z,
                )
            })
            .escalado(1.0 / 24.0);
        for (indice, cara) in prisma.iter().enumerate() {
            let hacia_fuera = cara.normal.punto(cara.centro().resta(centro));
            assert!(
                hacia_fuera > 0.0,
                "la cara {indice} tiene la normal al revés"
            );
        }
    }

    #[test]
    fn la_tapa_solo_se_ve_desde_encima_de_su_altura() {
        let prisma = prisma_de_prueba();
        let tapa = prisma[1];
        let debajo = Camara::mirando_a(Punto3::nuevo(10.0, 1.0, 0.0), Punto3::nuevo(0.0, 1.0, 0.0));
        let encima =
            Camara::mirando_a(Punto3::nuevo(10.0, 30.0, 0.0), Punto3::nuevo(0.0, 1.0, 0.0));
        assert!(!cara_visible(&debajo, &tapa));
        assert!(cara_visible(&encima, &tapa));
    }

    /// Prisma de 4 × 6 × 4 (ancho, alto, fondo) centrado en el origen: base
    /// (0), tapa (1), −z (2), +x (3), +z (4) y −x (5).
    fn prisma_de_prueba() -> [Cara; 6] {
        let (x0, x1) = (-2.0, 2.0);
        let (z0, z1) = (-2.0, 2.0);
        let alto = 6.0;
        let vertices = [
            Punto3::nuevo(x0, 0.0, z0),
            Punto3::nuevo(x1, 0.0, z0),
            Punto3::nuevo(x1, 0.0, z1),
            Punto3::nuevo(x0, 0.0, z1),
            Punto3::nuevo(x0, alto, z0),
            Punto3::nuevo(x1, alto, z0),
            Punto3::nuevo(x1, alto, z1),
            Punto3::nuevo(x0, alto, z1),
        ];
        caras_de_prisma(&vertices)
    }
}
