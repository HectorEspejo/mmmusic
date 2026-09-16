//! Visual «Ciudad»: el espectro como una rejilla de bandas × tiempo de prismas
//! dibujados en perspectiva desde una cámara en órbita, con eliminación de
//! líneas ocultas sobre el raster propio y un cielo de estrellas parpadeantes.

use std::collections::VecDeque;
use std::f64::consts::TAU;

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::canvas;

use super::Visual;
use super::paleta::Paleta;
use super::proyeccion::{
    Camara, Cara, Dimensiones, Punto2, Punto3, cara_visible, caras_de_prisma, proyectar,
};
use super::raster::Raster;
use crate::audio::Analisis;

/// Huella de cada edificio, en celdas de mundo (la rejilla va a paso 1).
pub const HUELLA: f64 = 0.7;
/// Altura máxima de un edificio, en múltiplos del ancho de la ciudad.
pub const ALTO_MAX: f64 = 0.8;
/// Altura mínima de un edificio, en múltiplos de la altura máxima.
pub const ALTO_MIN: f64 = 0.05;
/// Frames entre dos filas de la historia: a 30 fps son 15 filas por segundo.
pub const FILA_CADA: u32 = 2;
/// Celdas por estrella del cielo.
pub const CELDAS_POR_ESTRELLA: usize = 60;
/// Bandas (columnas) mínimas y máximas de la ciudad.
pub const BANDAS_MIN: usize = 8;
pub const BANDAS_MAX: usize = 24;
/// Filas (profundidad) mínimas y máximas de la historia.
pub const FILAS_MIN: usize = 8;
pub const FILAS_MAX: usize = 16;
/// Proporción del raster que ocupa la ciudad.
const ENCUADRE: f64 = 0.7;
/// Aristas de un prisma que puede llegar a trazar el orden del pintor.
const ARISTAS_MAX: usize = 12;

/// Ajustes de la visual que vienen de la configuración (`[visuales]`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AjustesCiudad {
    /// Segundos que tarda la cámara en dar una vuelta.
    pub vuelta_s: f32,
    /// Tope de filas de historia; la altura de la terminal puede pedir menos.
    pub filas: usize,
    /// Aristas punteadas; si no, continuas.
    pub punteado: bool,
}

impl Default for AjustesCiudad {
    fn default() -> Self {
        Self {
            vuelta_s: 20.0,
            filas: 12,
            punteado: true,
        }
    }
}

/// Bandas (columnas) para un ancho de terminal dado.
fn bandas_para(ancho_celdas: usize) -> usize {
    (ancho_celdas / 6).clamp(BANDAS_MIN, BANDAS_MAX)
}

/// Prisma recto apoyado en el suelo: la huella es cuadrada y la base está en
/// `y = 0`.
#[derive(Debug, Clone, Copy)]
pub struct Prisma {
    pub vertices: [Punto3; 8],
    pub caras: [Cara; 6],
}

impl Prisma {
    pub fn nuevo(x: f64, z: f64, huella: f64, alto: f64) -> Self {
        let (x0, x1) = (x - huella / 2.0, x + huella / 2.0);
        let (z0, z1) = (z - huella / 2.0, z + huella / 2.0);
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
        Self {
            vertices,
            caras: caras_de_prisma(&vertices),
        }
    }

    /// Centro del prisma, para ordenar por distancia a la cámara.
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
        suma.escalado(0.125)
    }
}

/// Anillo de filas de bandas: la fila 0 es la más reciente y ocupa el borde
/// delantero de la ciudad; las anteriores se alejan hacia el fondo.
pub struct Historia {
    filas: VecDeque<Vec<f32>>,
    n_bandas: usize,
    n_filas: usize,
}

impl Historia {
    pub fn nueva(n_bandas: usize, n_filas: usize) -> Self {
        Self {
            filas: VecDeque::new(),
            n_bandas,
            n_filas,
        }
    }

    /// Cambia el tamaño de la rejilla; si cambia, la historia empieza de cero.
    pub fn ajustar(&mut self, n_bandas: usize, n_filas: usize) {
        if n_bandas == self.n_bandas && n_filas == self.n_filas {
            return;
        }
        self.n_bandas = n_bandas;
        self.n_filas = n_filas;
        self.filas.clear();
    }

    pub fn reiniciar(&mut self) {
        self.filas.clear();
    }

    /// Submuestrea las bandas (el máximo de cada tramo, como el mini espectro)
    /// y deja una fila nueva al frente, reutilizando el buffer de la más vieja.
    pub fn empujar(&mut self, bandas: &[f32]) {
        if bandas.is_empty() || self.n_bandas == 0 || self.n_filas == 0 {
            return;
        }
        let mut fila = if self.filas.len() >= self.n_filas {
            self.filas.pop_back().unwrap_or_default()
        } else {
            Vec::with_capacity(self.n_bandas)
        };
        fila.clear();
        for banda in 0..self.n_bandas {
            let inicio = banda * bandas.len() / self.n_bandas;
            let fin = (((banda + 1) * bandas.len()) / self.n_bandas)
                .max(inicio + 1)
                .min(bandas.len());
            let valor = bandas[inicio.min(bandas.len() - 1)..fin]
                .iter()
                .copied()
                .fold(0.0f32, f32::max);
            fila.push(valor);
        }
        self.filas.push_front(fila);
    }

    /// Una fila de bandas, 0 = la más reciente.
    pub fn fila(&self, indice: usize) -> Option<&[f32]> {
        self.filas.get(indice).map(|fila| fila.as_slice())
    }

    /// Las filas de la más reciente a la más antigua.
    pub fn filas(&self) -> impl Iterator<Item = &[f32]> {
        self.filas.iter().map(|fila| fila.as_slice())
    }

    pub fn len(&self) -> usize {
        self.filas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.filas.is_empty()
    }
}

/// Estrella del cielo: posición fija y parpadeo lento propios.
struct Estrella {
    x: i32,
    y: i32,
    fase: f64,
    periodo: f64,
}

/// Cielo de estrellas estables para un tamaño dado: la semilla sale del tamaño,
/// así que no cambian entre frames y sí al redimensionar la terminal.
pub struct Estrellas {
    estrellas: Vec<Estrella>,
}

impl Estrellas {
    pub fn para_celdas(ancho_celdas: usize, alto_celdas: usize) -> Self {
        let cuantas = (ancho_celdas * alto_celdas) / CELDAS_POR_ESTRELLA;
        if cuantas == 0 || ancho_celdas == 0 || alto_celdas == 0 {
            return Self {
                estrellas: Vec::new(),
            };
        }
        let semilla = ((ancho_celdas as u64) << 32) | alto_celdas as u64;
        let mut generador = StdRng::seed_from_u64(semilla);
        let mut estrellas = Vec::with_capacity(cuantas);
        for _ in 0..cuantas {
            estrellas.push(Estrella {
                x: generador.random_range(0..(ancho_celdas * 2) as i32),
                y: generador.random_range(0..(alto_celdas * 4) as i32),
                fase: generador.random_range(0.0..TAU),
                periodo: generador.random_range(2.0..5.0),
            });
        }
        Self { estrellas }
    }

    /// Brillo lento (periodo de 2 a 5 s): la estrella se enciende mientras pasa
    /// de 0,5. Siempre en color tenue.
    pub fn dibujar(&self, raster: &mut Raster, t: f64, color: Color) {
        for estrella in &self.estrellas {
            let brillo = 0.5 + 0.5 * (TAU * t / estrella.periodo + estrella.fase).sin();
            if brillo > 0.5 {
                raster.punto(estrella.x, estrella.y, color);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.estrellas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.estrellas.is_empty()
    }
}

pub struct Ciudad {
    ascii: bool,
    ajustes: AjustesCiudad,
    historia: Historia,
    estrellas: Estrellas,
    raster: Raster,
    tamano: (usize, usize),
    tiempo: f64,
    fotogramas: u32,
    orden: Vec<(usize, f64)>,
}

impl Ciudad {
    pub fn nuevo(ascii: bool, ajustes: AjustesCiudad) -> Self {
        let filas = ajustes.filas.max(FILAS_MIN);
        Self {
            ascii,
            ajustes,
            historia: Historia::nueva(BANDAS_MIN, filas),
            estrellas: Estrellas::para_celdas(0, 0),
            raster: Raster::nuevo(0, 0, ascii),
            tamano: (0, 0),
            tiempo: 0.0,
            fotogramas: 0,
            orden: Vec::new(),
        }
    }

    /// Filas de historia para una altura dada, acotadas por la configuración.
    fn filas_para(&self, alto_celdas: usize) -> usize {
        (alto_celdas / 4)
            .clamp(FILAS_MIN, FILAS_MAX)
            .min(self.ajustes.filas.max(FILAS_MIN))
    }

    /// Construye el prisma de la banda `banda` de la fila `fila`.
    fn edificio(
        &self,
        fila: usize,
        banda: usize,
        n_bandas: usize,
        n_filas: usize,
        alto_max: f64,
    ) -> Prisma {
        let valor = self
            .historia
            .fila(fila)
            .and_then(|bandas| bandas.get(banda))
            .copied()
            .unwrap_or(0.0);
        let alto = (f64::from(valor) * alto_max).max(ALTO_MIN * alto_max);
        let x = banda as f64 - (n_bandas as f64 - 1.0) / 2.0;
        let z = fila as f64 - (n_filas as f64 - 1.0) / 2.0;
        Prisma::nuevo(x, z, HUELLA, alto)
    }
}

impl Visual for Ciudad {
    fn nombre(&self) -> &'static str {
        "Ciudad"
    }

    fn reiniciar(&mut self) {
        self.historia.reiniciar();
        self.tiempo = 0.0;
        self.fotogramas = 0;
        self.orden.clear();
        self.raster.limpiar();
    }

    fn dibujar(
        &mut self,
        a: &Analisis,
        area: Rect,
        p: &Paleta,
        dt: f32,
        ctx: &mut canvas::Context<'_>,
    ) {
        let (ancho_celdas, alto_celdas) = (area.width as usize, area.height as usize);
        if ancho_celdas == 0 || alto_celdas == 0 {
            return;
        }
        let n_bandas = bandas_para(ancho_celdas);
        let n_filas = self.filas_para(alto_celdas);

        // El raster y las estrellas se reservan una vez por tamaño.
        if (ancho_celdas, alto_celdas) != self.tamano {
            self.tamano = (ancho_celdas, alto_celdas);
            self.raster = Raster::nuevo(ancho_celdas, alto_celdas, self.ascii);
            self.estrellas = Estrellas::para_celdas(ancho_celdas, alto_celdas);
        }
        self.historia.ajustar(n_bandas, n_filas);

        // El espectro entra una fila cada dos frames.
        self.fotogramas = self.fotogramas.wrapping_add(1);
        if self.fotogramas % FILA_CADA == 1 {
            self.historia.empujar(&a.bandas);
        }
        self.tiempo += f64::from(dt);

        let rms = f64::from(((a.rms[0] + a.rms[1]) / 2.0).clamp(0.0, 1.0));
        let dims = Dimensiones {
            ancho: n_bandas as f64,
            profundidad: n_filas as f64,
        };
        let camara = Camara::orbita(self.tiempo, rms, dims, f64::from(self.ajustes.vuelta_s));
        let alto_max = ALTO_MAX * dims.ancho;

        // Orden del pintor: de lejos a cerca.
        self.orden.clear();
        for (fila, bandas) in self.historia.filas().enumerate() {
            let z = fila as f64 - (n_filas as f64 - 1.0) / 2.0;
            for (banda, valor) in bandas.iter().enumerate() {
                let x = banda as f64 - (n_bandas as f64 - 1.0) / 2.0;
                let alto = (f64::from(*valor) * alto_max).max(ALTO_MIN * alto_max);
                let centro = Punto3::nuevo(x, alto / 2.0, z);
                self.orden
                    .push((fila * n_bandas + banda, camara.ojo.distancia(centro)));
            }
        }
        self.orden
            .sort_unstable_by(|uno, otro| otro.1.total_cmp(&uno.1));

        self.raster.limpiar();
        self.estrellas
            .dibujar(&mut self.raster, self.tiempo, p.tenue);

        for (indice, _) in &self.orden {
            let (fila, banda) = (indice / n_bandas, indice % n_bandas);
            let prisma = self.edificio(fila, banda, n_bandas, n_filas, alto_max);
            dibujar_prisma(
                &mut self.raster,
                &camara,
                &prisma,
                p,
                alto_max,
                self.ajustes.punteado,
            );
        }

        self.raster.volcar(ctx);
    }
}

/// Traslada un punto proyectado (normalizado) a coordenadas de punto del raster,
/// con escala uniforme y el eje vertical volteado.
pub fn a_puntos(raster: &Raster, punto: Punto2) -> Punto2 {
    let (ancho_puntos, alto_puntos) = (raster.ancho_puntos(), raster.alto_puntos());
    let escala = ancho_puntos.min(alto_puntos) as f64 / 2.0 * ENCUADRE;
    Punto2::nuevo(
        ancho_puntos as f64 / 2.0 + punto.x * escala,
        alto_puntos as f64 / 2.0 - punto.y * escala,
    )
}

/// Proyecta los cuatro vértices de una cara, o `None` si alguno queda detrás de
/// la cámara.
fn proyectar_cara(cara: &Cara, proyectados: &[Option<Punto2>; 8]) -> Option<[Punto2; 4]> {
    let mut puntos = [Punto2::nuevo(0.0, 0.0); 4];
    for (posicion, indice) in cara.indices.iter().enumerate() {
        puntos[posicion] = proyectados[*indice]?;
    }
    Some(puntos)
}

/// Dibuja un prisma con el orden del pintor: primero borra las caras visibles
/// (que tapan lo que hay detrás) y después traza sus aristas punteadas, sin
/// repetir las compartidas. La base no se dibuja: su normal mira hacia abajo y
/// la cámara siempre está por encima del suelo, así que el descarte la elimina.
pub fn dibujar_prisma(
    raster: &mut Raster,
    camara: &Camara,
    prisma: &Prisma,
    p: &Paleta,
    alto_max: f64,
    punteado: bool,
) {
    let proyectados: [Option<Punto2>; 8] =
        prisma.vertices.map(|vertice| proyectar(camara, vertice));

    for cara in prisma
        .caras
        .iter()
        .filter(|cara| cara_visible(camara, cara))
    {
        let Some(poligono) = proyectar_cara(cara, &proyectados) else {
            continue;
        };
        let puntos = poligono.map(|punto| a_puntos(raster, punto));
        raster.borrar_poligono(&puntos);
    }

    let mut trazadas = [(usize::MAX, usize::MAX); ARISTAS_MAX];
    let mut cuantas = 0;
    for cara in prisma
        .caras
        .iter()
        .filter(|cara| cara_visible(camara, cara))
    {
        for (desde, hasta) in cara.aristas() {
            let clave = if desde < hasta {
                (desde, hasta)
            } else {
                (hasta, desde)
            };
            if trazadas[..cuantas].contains(&clave) {
                continue;
            }
            if cuantas < trazadas.len() {
                trazadas[cuantas] = clave;
                cuantas += 1;
            }
            let (Some(uno), Some(otro)) = (proyectados[desde], proyectados[hasta]) else {
                continue;
            };
            let altura = prisma.vertices[desde].y.max(prisma.vertices[hasta].y);
            let color = p.gradiente((altura / alto_max).clamp(0.0, 1.0) as f32);
            raster.linea(
                a_puntos(raster, uno),
                a_puntos(raster, otro),
                color,
                punteado,
            );
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_historia_guarda_las_ultimas_filas() {
        let mut historia = Historia::nueva(3, 4);
        for fila in 0..6 {
            historia.empujar(&[fila as f32, 1.0, 2.0]);
        }
        assert_eq!(historia.len(), 4);
        assert_eq!(historia.fila(0), Some([5.0, 1.0, 2.0].as_slice()));
        assert_eq!(historia.fila(3), Some([2.0, 1.0, 2.0].as_slice()));

        historia.reiniciar();
        assert!(historia.is_empty());

        let mut historia = Historia::nueva(2, 2);
        historia.empujar(&[0.1, 0.2, 0.3, 0.4]);
        // Dos columnas: el máximo de cada mitad.
        assert_eq!(historia.fila(0), Some([0.2, 0.4].as_slice()));
    }

    #[test]
    fn las_estrellas_son_estables_por_tamano() {
        let unas = Estrellas::para_celdas(80, 24);
        let otras = Estrellas::para_celdas(80, 24);
        assert_eq!(unas.len(), otras.len());
        assert_eq!(unas.len(), 80 * 24 / CELDAS_POR_ESTRELLA);
        for (una, otra) in unas.estrellas.iter().zip(&otras.estrellas) {
            assert_eq!((una.x, una.y), (otra.x, otra.y));
            assert_eq!(una.periodo, otra.periodo);
        }

        let distintas = Estrellas::para_celdas(40, 12);
        assert_eq!(distintas.len(), 40 * 12 / CELDAS_POR_ESTRELLA);
        assert!(Estrellas::para_celdas(0, 0).is_empty());
    }

    #[test]
    fn las_estrellas_parpadean() {
        let estrellas = Estrellas::para_celdas(80, 24);
        let mut encendidas_ahora = 0;
        let mut encendidas_luego = 0;
        let color = Color::Rgb(10, 10, 10);
        let mut raster = Raster::nuevo(80, 24, false);
        estrellas.dibujar(&mut raster, 0.0, color);
        encendidas_ahora += raster.total_puntos();
        raster.limpiar();
        estrellas.dibujar(&mut raster, 1.3, color);
        encendidas_luego += raster.total_puntos();
        assert!(encendidas_ahora > 0, "algunas estrellas deben verse");
        assert!(
            encendidas_ahora < estrellas.len(),
            "no todas pueden estar encendidas a la vez"
        );
        assert_ne!(encendidas_ahora, encendidas_luego);
    }

    #[test]
    fn las_filas_dependen_de_la_altura_y_de_la_configuracion() {
        let ciudad = Ciudad::nuevo(false, AjustesCiudad::default());
        assert_eq!(ciudad.filas_para(12), 8);
        assert_eq!(ciudad.filas_para(24), 8);
        assert_eq!(ciudad.filas_para(48), 12);
        assert_eq!(ciudad.filas_para(80), 12, "la configuración acota a 12");

        let pocas = AjustesCiudad {
            filas: 8,
            ..AjustesCiudad::default()
        };
        let ciudad = Ciudad::nuevo(false, pocas);
        assert_eq!(ciudad.filas_para(80), 8);
    }

    #[test]
    fn las_bandas_dependen_del_ancho() {
        assert_eq!(bandas_para(40), 8, "el tamaño mínimo usa 8 bandas");
        assert_eq!(bandas_para(80), 13);
        assert_eq!(bandas_para(200), 24, "el máximo son 24 bandas");
        assert_eq!(bandas_para(0), 8);
    }
}
