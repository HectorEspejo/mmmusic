//! Raster propio de puntos 2 × 4 por celda con borrado de polígonos y líneas
//! punteadas. El `Canvas` de `ratatui` solo añade puntos (DT-62); este raster
//! permite eliminar lo que tapa un polígono y, al final del frame, vuelca al
//! `Canvas` únicamente los puntos supervivientes.
//!
//! Sin E/S: todo se prueba con valores sintéticos.

use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::canvas::{self, Points};

use super::proyeccion::Punto2;

/// Puntos por celda en horizontal.
pub const PUNTOS_X: usize = 2;
/// Puntos por celda en vertical.
pub const PUNTOS_Y: usize = 4;
/// Cruces por fila que admite el scanline de `borrar_poligono`; los polígonos
/// que dibuja la ciudad son cuadriláteros.
const MAX_CRUCES: usize = 8;
/// Umbral de densidad del modo ascii: a partir de aquí la celda es `:`, si no `.`.
const PUNTOS_DENSOS: u32 = 3;

/// Grupo de puntos de un mismo color, reutilizado entre frames.
struct Grupo {
    color: Color,
    coords: Vec<(f64, f64)>,
}

pub struct Raster {
    ancho_celdas: usize,
    alto_celdas: usize,
    ascii: bool,
    /// Máscara de 8 bits por celda: bit `(x % 2) + 2 · (y % 4)`, como el
    /// `PatternGrid` braille de `ratatui` (fila superior primero).
    mascaras: Vec<u8>,
    /// Color de cada celda: gana el último punto dibujado en ella.
    colores: Vec<Option<Color>>,
    /// Buffers del volcado, reservados una vez y reutilizados.
    grupos: Vec<Grupo>,
    usados: usize,
    ultimo: Option<(Color, usize)>,
}

impl Raster {
    pub fn nuevo(ancho_celdas: usize, alto_celdas: usize, ascii: bool) -> Self {
        let celdas = ancho_celdas * alto_celdas;
        Self {
            ancho_celdas,
            alto_celdas,
            ascii,
            mascaras: vec![0; celdas],
            colores: vec![None; celdas],
            grupos: Vec::new(),
            usados: 0,
            ultimo: None,
        }
    }

    /// Ancho en puntos (el doble de celdas).
    pub fn ancho_puntos(&self) -> usize {
        self.ancho_celdas * PUNTOS_X
    }

    /// Alto en puntos (el cuádruple de celdas).
    pub fn alto_puntos(&self) -> usize {
        self.alto_celdas * PUNTOS_Y
    }

    pub fn limpiar(&mut self) {
        self.mascaras.fill(0);
        self.colores.fill(None);
    }

    /// Enciende un punto; fuera de la rejilla no hace nada.
    pub fn punto(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.ancho_puntos() || y >= self.alto_puntos() {
            return;
        }
        let celda = self.celda_de(x, y);
        self.mascaras[celda] |= 1 << self.bit_de(x, y);
        self.colores[celda] = Some(color);
    }

    /// Línea de Bresenham en puntos; `punteada` dibuja uno de cada dos.
    pub fn linea(&mut self, a: Punto2, b: Punto2, color: Color, punteada: bool) {
        let (mut x, mut y) = (a.x.round() as i32, a.y.round() as i32);
        let (x_fin, y_fin) = (b.x.round() as i32, b.y.round() as i32);
        let dx = (x_fin - x).abs();
        let dy = -(y_fin - y).abs();
        let paso_x = if x < x_fin { 1 } else { -1 };
        let paso_y = if y < y_fin { 1 } else { -1 };
        let mut error = dx + dy;
        let mut orden = 0u32;

        loop {
            if !punteada || orden.is_multiple_of(2) {
                self.punto(x, y, color);
            }
            if x == x_fin && y == y_fin {
                break;
            }
            let doble = 2 * error;
            if doble >= dy {
                error += dy;
                x += paso_x;
            }
            if doble <= dx {
                error += dx;
                y += paso_y;
            }
            orden += 1;
        }
    }

    /// Borra los puntos cuyo centro cae dentro del polígono (scanline con regla
    /// par-impar). En modo ascii la celda es atómica, así que se borra entera:
    /// la ocultación se aproxima a celda, como pide la especificación.
    pub fn borrar_poligono(&mut self, poligono: &[Punto2]) {
        if poligono.len() < 3 {
            return;
        }
        let (ancho_puntos, alto_puntos) = (self.ancho_puntos(), self.alto_puntos());
        if ancho_puntos == 0 || alto_puntos == 0 {
            return;
        }
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        for punto in poligono {
            y_min = y_min.min(punto.y);
            y_max = y_max.max(punto.y);
        }
        let fila_ini = ((y_min - 0.5).ceil().max(0.0) as usize).min(alto_puntos);
        let fila_fin = ((y_max - 0.5).ceil().max(0.0) as usize).min(alto_puntos - 1);
        if fila_ini > fila_fin {
            return;
        }

        let mut cruces = [0.0f64; MAX_CRUCES];
        for fila in fila_ini..=fila_fin {
            let centro_y = fila as f64 + 0.5;
            let mut cuantos = 0;
            let mut desbordado = false;
            for indice in 0..poligono.len() {
                let actual = poligono[indice];
                let siguiente = poligono[(indice + 1) % poligono.len()];
                // Regla semiabierta: cada vértice cuenta una sola vez.
                if (actual.y <= centro_y) == (siguiente.y <= centro_y) {
                    continue;
                }
                let x_cruce = actual.x
                    + (centro_y - actual.y) * (siguiente.x - actual.x) / (siguiente.y - actual.y);
                if cuantos == MAX_CRUCES {
                    desbordado = true;
                    break;
                }
                let posicion = cruces[..cuantos]
                    .iter()
                    .position(|valor| *valor > x_cruce)
                    .unwrap_or(cuantos);
                cruces.copy_within(posicion..cuantos, posicion + 1);
                cruces[posicion] = x_cruce;
                cuantos += 1;
            }
            if desbordado {
                continue;
            }
            let mut par = 0;
            while par + 1 < cuantos {
                let centro_x_ini = cruces[par];
                let centro_x_fin = cruces[par + 1];
                let desde = ((centro_x_ini - 0.5).ceil().max(0.0) as usize).min(ancho_puntos);
                let hasta = ((centro_x_fin - 0.5).ceil().max(0.0) as usize).min(ancho_puntos);
                for x in desde..hasta {
                    self.borrar_punto(x, fila);
                }
                par += 2;
            }
        }
    }

    /// Vuelca los puntos supervivientes al `Canvas`: en braille, un `Points` por
    /// color; en ascii, un glifo por celda.
    ///
    /// Da por supuesto el contrato de `ui/vistas/visual.rs`: los límites del
    /// canvas son `[0, ancho]` × `[0, alto]` en celdas y el origen está abajo a
    /// la izquierda.
    pub fn volcar(&mut self, ctx: &mut canvas::Context<'_>) {
        if self.ascii {
            self.volcar_ascii(ctx);
        } else {
            self.volcar_braille(ctx);
        }
    }

    /// Punto encendido (coordenadas de punto).
    pub fn marcado(&self, x: usize, y: usize) -> bool {
        if x >= self.ancho_puntos() || y >= self.alto_puntos() {
            return false;
        }
        let celda = self.celda_de(x, y);
        self.mascaras[celda] & (1 << self.bit_de(x, y)) != 0
    }

    /// Color de la celda que contiene el punto (el último dibujado).
    pub fn color_de(&self, x: usize, y: usize) -> Option<Color> {
        if x >= self.ancho_puntos() || y >= self.alto_puntos() {
            return None;
        }
        self.colores[self.celda_de(x, y)]
    }

    /// Número de puntos encendidos.
    pub fn total_puntos(&self) -> usize {
        self.mascaras
            .iter()
            .map(|mascara| mascara.count_ones() as usize)
            .sum()
    }

    fn celda_de(&self, x: usize, y: usize) -> usize {
        (y / PUNTOS_Y) * self.ancho_celdas + (x / PUNTOS_X)
    }

    fn bit_de(&self, x: usize, y: usize) -> u32 {
        ((x % PUNTOS_X) + PUNTOS_X * (y % PUNTOS_Y)) as u32
    }

    fn borrar_punto(&mut self, x: usize, y: usize) {
        let celda = self.celda_de(x, y);
        if self.ascii {
            self.mascaras[celda] = 0;
            self.colores[celda] = None;
            return;
        }
        self.mascaras[celda] &= !(1 << self.bit_de(x, y));
        if self.mascaras[celda] == 0 {
            self.colores[celda] = None;
        }
    }

    /// Índice del grupo de `color`, reutilizando los buffers de frames previos.
    fn grupo_de(&mut self, color: Color) -> usize {
        if let Some((ultimo, indice)) = self.ultimo
            && ultimo == color
        {
            return indice;
        }
        let indice = match self.grupos[..self.usados]
            .iter()
            .position(|grupo| grupo.color == color)
        {
            Some(indice) => indice,
            None => {
                let indice = self.usados;
                if indice == self.grupos.len() {
                    self.grupos.push(Grupo {
                        color,
                        coords: Vec::new(),
                    });
                } else {
                    self.grupos[indice].color = color;
                }
                self.usados += 1;
                indice
            }
        };
        self.ultimo = Some((color, indice));
        indice
    }

    fn volcar_braille(&mut self, ctx: &mut canvas::Context<'_>) {
        for grupo in &mut self.grupos {
            grupo.coords.clear();
        }
        self.usados = 0;
        self.ultimo = None;
        let (ancho_puntos, alto_puntos) = (self.ancho_puntos(), self.alto_puntos());
        let factor_x = if ancho_puntos <= 1 {
            0.0
        } else {
            self.ancho_celdas as f64 / (ancho_puntos - 1) as f64
        };
        let factor_y = if alto_puntos <= 1 {
            0.0
        } else {
            self.alto_celdas as f64 / (alto_puntos - 1) as f64
        };
        let alto_celdas = self.alto_celdas as f64;

        for celda in 0..self.mascaras.len() {
            let mascara = self.mascaras[celda];
            if mascara == 0 {
                continue;
            }
            let Some(color) = self.colores[celda] else {
                continue;
            };
            let indice = self.grupo_de(color);
            let columna = celda % self.ancho_celdas;
            let fila = celda / self.ancho_celdas;
            let destino = &mut self.grupos[indice].coords;
            for bit in 0..(PUNTOS_X * PUNTOS_Y) {
                if mascara & (1 << bit) == 0 {
                    continue;
                }
                let x = columna * PUNTOS_X + bit % PUNTOS_X;
                let y = fila * PUNTOS_Y + bit / PUNTOS_X;
                // El redondeo en coma flotante deja la última fila y la última
                // columna unas ulps fuera de los límites del canvas, y
                // `Painter::get_point` descarta lo que se sale: se acotan para
                // que el punto vuelva a caer en su celda.
                destino.push((
                    (x as f64 * factor_x).clamp(0.0, self.ancho_celdas as f64),
                    (alto_celdas - y as f64 * factor_y).clamp(0.0, alto_celdas),
                ));
            }
        }

        for grupo in &self.grupos[..self.usados] {
            if grupo.coords.is_empty() {
                continue;
            }
            ctx.draw(&Points::new(&grupo.coords, grupo.color));
        }
    }

    fn volcar_ascii(&mut self, ctx: &mut canvas::Context<'_>) {
        for celda in 0..self.mascaras.len() {
            let mascara = self.mascaras[celda];
            if mascara == 0 {
                continue;
            }
            let Some(color) = self.colores[celda] else {
                continue;
            };
            let glifo = if mascara.count_ones() >= PUNTOS_DENSOS {
                ":"
            } else {
                "."
            };
            let columna = celda % self.ancho_celdas;
            let fila = celda / self.ancho_celdas;
            ctx.print(
                self.x_de_celda(columna),
                self.y_de_celda(fila),
                Span::styled(glifo, Style::new().fg(color)),
            );
        }
    }

    /// Coordenada de canvas de una columna. `Context::print` trunca en vez de
    /// redondear, así que hay que apuntar al centro de la celda; en la última
    /// columna el centro cae fuera de los límites y se usa el borde.
    fn x_de_celda(&self, columna: usize) -> f64 {
        let ancho = self.ancho_celdas as f64;
        if ancho <= 1.0 {
            return 0.0;
        }
        ((columna as f64 + 0.5) * ancho / (ancho - 1.0)).min(ancho)
    }

    /// Coordenada de canvas de una fila (contando desde arriba).
    fn y_de_celda(&self, fila: usize) -> f64 {
        let alto = self.alto_celdas as f64;
        if alto <= 1.0 {
            return 0.0;
        }
        (alto - (fila as f64 + 0.5) * alto / (alto - 1.0)).max(0.0)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const AZUL: Color = Color::Rgb(0, 0, 255);
    const ROJO: Color = Color::Rgb(255, 0, 0);

    #[test]
    fn los_puntos_fuera_de_la_rejilla_se_ignoran() {
        let mut raster = Raster::nuevo(10, 5, false);
        raster.punto(-1, 0, AZUL);
        raster.punto(0, -1, AZUL);
        raster.punto(20, 0, AZUL);
        raster.punto(0, 20, AZUL);
        assert_eq!(raster.total_puntos(), 0);

        raster.punto(19, 19, AZUL);
        assert_eq!(raster.total_puntos(), 1);
        assert!(raster.marcado(19, 19));
        assert_eq!(raster.color_de(19, 19), Some(AZUL));
    }

    #[test]
    fn el_color_de_la_celda_es_el_del_ultimo_punto() {
        let mut raster = Raster::nuevo(10, 5, false);
        raster.punto(4, 8, AZUL);
        raster.punto(5, 9, ROJO);
        assert_eq!(raster.color_de(4, 8), Some(ROJO));
        assert_eq!(raster.color_de(5, 9), Some(ROJO));

        raster.limpiar();
        assert_eq!(raster.total_puntos(), 0);
        assert_eq!(raster.color_de(4, 8), None);
    }

    #[test]
    fn el_borrado_de_poligono_elimina_su_interior() {
        let mut raster = Raster::nuevo(20, 10, false);
        for x in 0..40 {
            raster.punto(x, 20, AZUL);
        }
        for y in 0..40 {
            raster.punto(20, y, AZUL);
        }
        let antes = raster.total_puntos();

        // Cuadrado de puntos (10, 10) – (29, 29): deja fuera los bordes.
        raster.borrar_poligono(&[
            Punto2::nuevo(10.0, 10.0),
            Punto2::nuevo(30.0, 10.0),
            Punto2::nuevo(30.0, 30.0),
            Punto2::nuevo(10.0, 30.0),
        ]);

        for y in 10..30 {
            for x in 10..30 {
                assert!(!raster.marcado(x, y), "el punto ({x}, {y}) debía borrarse");
            }
        }
        assert!(raster.marcado(9, 20) || raster.marcado(8, 20));
        assert!(raster.marcado(20, 9));
        assert!(raster.total_puntos() < antes);
    }

    #[test]
    fn la_linea_punteada_tiene_la_mitad_de_puntos() {
        let mut continua = Raster::nuevo(40, 10, false);
        let mut punteada = Raster::nuevo(40, 10, false);
        let (a, b) = (Punto2::nuevo(0.0, 20.0), Punto2::nuevo(39.0, 20.0));
        continua.linea(a, b, AZUL, false);
        punteada.linea(a, b, AZUL, true);

        let (total, mitad) = (continua.total_puntos(), punteada.total_puntos());
        assert_eq!(total, 40);
        assert_eq!(mitad * 2, total);
        for x in 0..40 {
            if punteada.marcado(x, 20) {
                assert!(continua.marcado(x, 20), "la punteada no añade puntos");
            }
        }
        assert!(punteada.marcado(0, 20));
        assert!(punteada.marcado(38, 20));
    }

    #[test]
    fn en_ascii_el_borrado_se_lleva_la_celda_entera() {
        let mut raster = Raster::nuevo(20, 10, true);
        for x in 0..40 {
            for y in 0..40 {
                raster.punto(x, y, AZUL);
            }
        }
        // Un cuadrado pequeño dentro de una sola celda (celda 2,2 = puntos 4-5 × 8-11).
        raster.borrar_poligono(&[
            Punto2::nuevo(4.0, 8.0),
            Punto2::nuevo(5.5, 8.0),
            Punto2::nuevo(5.5, 9.5),
            Punto2::nuevo(4.0, 9.5),
        ]);
        assert!(!raster.marcado(4, 8));
        assert!(!raster.marcado(5, 11), "la celda entera queda limpia");
        assert!(raster.marcado(6, 8), "la celda vecina no se toca");
    }

    #[test]
    fn el_volcado_en_braille_no_pierde_puntos_y_respeta_el_color() {
        use std::cell::RefCell;

        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use ratatui::widgets::Widget;
        use ratatui::widgets::canvas::Canvas;

        let (ancho, alto) = (20u16, 10u16);
        let raster = RefCell::new(Raster::nuevo(ancho as usize, alto as usize, false));
        raster.borrow_mut().punto(0, 0, AZUL);
        raster
            .borrow_mut()
            .punto(i32::from(ancho) * 2 - 1, i32::from(alto) * 4 - 1, ROJO);
        raster.borrow_mut().punto(3, 5, AZUL);
        let area = Rect::new(0, 0, ancho, alto);
        let mut buffer = Buffer::empty(area);
        let canvas = Canvas::default()
            .x_bounds([0.0, f64::from(ancho)])
            .y_bounds([0.0, f64::from(alto)])
            .background_color(Color::Reset)
            .paint(|ctx| raster.borrow_mut().volcar(ctx));
        canvas.render(area, &mut buffer);

        let pintadas = buffer
            .content()
            .iter()
            .filter(|celda| celda.symbol() != " ")
            .count();
        assert_eq!(pintadas, 3, "cada punto cae en una celda distinta");
        assert_eq!(buffer[(0, 0)].fg, AZUL);
        assert_eq!(buffer[(ancho - 1, alto - 1)].fg, ROJO);
    }

    #[test]
    fn los_puntos_del_borde_caen_en_su_celda() {
        use std::cell::RefCell;

        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use ratatui::widgets::Widget;
        use ratatui::widgets::canvas::Canvas;

        // Tamaños en los que el redondeo dejaba fuera la última fila o la
        // última columna de puntos.
        for (ancho, alto) in [(20u16, 22u16), (15, 24), (44, 22), (80, 24)] {
            let raster = RefCell::new(Raster::nuevo(ancho as usize, alto as usize, false));
            let (ultimo_x, ultimo_y) = (i32::from(ancho) * 2 - 1, i32::from(alto) * 4 - 1);
            for (x, y) in [(0, 0), (ultimo_x, 0), (0, ultimo_y), (ultimo_x, ultimo_y)] {
                raster.borrow_mut().punto(x, y, AZUL);
            }
            let area = Rect::new(0, 0, ancho, alto);
            let mut buffer = Buffer::empty(area);
            let canvas = Canvas::default()
                .x_bounds([0.0, f64::from(ancho)])
                .y_bounds([0.0, f64::from(alto)])
                .background_color(Color::Reset)
                .paint(|ctx| raster.borrow_mut().volcar(ctx));
            canvas.render(area, &mut buffer);

            for (columna, fila) in [(0, 0), (ancho - 1, 0), (0, alto - 1), (ancho - 1, alto - 1)] {
                assert_ne!(
                    buffer[(columna, fila)].symbol(),
                    " ",
                    "el punto de la esquina ({columna}, {fila}) no llegó a pintarse en {ancho}×{alto}"
                );
            }
        }
    }

    #[test]
    fn el_volcado_reutiliza_el_pool_de_grupos() {
        use ratatui::symbols::Marker;
        use ratatui::widgets::canvas::Context;

        let mut raster = Raster::nuevo(40, 12, false);
        let mut ctx = Context::new(40, 12, [0.0, 40.0], [0.0, 12.0], Marker::Braille);
        let colores = [AZUL, ROJO, Color::Rgb(1, 1, 1), Color::Rgb(2, 2, 2)];
        let mut medidas = Vec::new();
        for frame in 0..6 {
            raster.limpiar();
            for (indice, color) in colores.iter().enumerate() {
                for x in 0..80 {
                    raster.punto(x, indice as i32 * 4 + frame % 4, *color);
                }
            }
            raster.volcar(&mut ctx);
            medidas.push((
                raster.grupos.len(),
                raster
                    .grupos
                    .iter()
                    .map(|grupo| grupo.coords.capacity())
                    .sum::<usize>(),
            ));
        }

        // El pool de grupos y sus buffers son los mismos en todos los frames:
        // ni se acumulan grupos nuevos ni crecen los buffers ya reservados.
        let (grupos, capacidad) = *medidas.last().expect("hubo frames");
        assert_eq!(grupos, colores.len(), "un grupo por color del frame");
        for (frame, medida) in medidas.iter().enumerate().skip(1) {
            assert_eq!(
                *medida,
                (grupos, capacidad),
                "el volcado reservó memoria nueva en el frame {frame}"
            );
        }
    }

    #[test]
    fn el_volcado_en_ascii_escribe_glifos_por_celda() {
        use std::cell::RefCell;

        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use ratatui::widgets::Widget;
        use ratatui::widgets::canvas::Canvas;

        let (ancho, alto) = (12u16, 6u16);
        let raster = RefCell::new(Raster::nuevo(ancho as usize, alto as usize, true));
        // Columna densa: cuatro puntos en una celda.
        for y in 0..4 {
            raster.borrow_mut().punto(4, y, AZUL);
        }
        // Punto suelto.
        raster.borrow_mut().punto(11, 9, ROJO);
        let area = Rect::new(0, 0, ancho, alto);
        let mut buffer = Buffer::empty(area);
        let canvas = Canvas::default()
            .x_bounds([0.0, f64::from(ancho)])
            .y_bounds([0.0, f64::from(alto)])
            .background_color(Color::Reset)
            .paint(|ctx| raster.borrow_mut().volcar(ctx));
        canvas.render(area, &mut buffer);

        assert_eq!(buffer[(2, 0)].symbol(), ":");
        assert_eq!(buffer[(2, 0)].fg, AZUL);
        assert_eq!(buffer[(5, 2)].symbol(), ".");
        assert_eq!(buffer[(5, 2)].fg, ROJO);
    }
}
