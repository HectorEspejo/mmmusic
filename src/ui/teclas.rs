use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::Vista;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accion {
    Salir,
    SalirInmediato,
    Cerrar,
    Ayuda,
    IrA(Vista),
    AlternarCola,
    FocoSiguiente,
    FocoAnterior,
    Reescanear,
    RecargarTema,
    EnfocarBuscar,
    Abajo,
    Arriba,
    Izquierda,
    Derecha,
    Primero,
    Ultimo,
    MediaAbajo,
    MediaArriba,
    Abrir,
    AlternarPausa,
    Siguiente,
    Anterior,
    Detener,
    RetrocederCorto,
    AvanzarCorto,
    RetrocederLargo,
    AvanzarLargo,
    SubirVolumen,
    BajarVolumen,
    Silencio,
    AlternarAleatorio,
    CiclarRepeticion,
    AnadirCola,
    ReproducirSiguiente,
    AnadirPlaylist,
    Quitar,
    MoverAbajo,
    MoverArriba,
    NuevaPlaylist,
    RenombrarPlaylist,
    EliminarPlaylist,
    ExportarPlaylist,
    ImportarPlaylist,
    VaciarCola,
    CiclarOrden,
    InvertirOrden,
    TeclaG,
}

pub fn traducir(tecla: &KeyEvent) -> Option<Accion> {
    let control = tecla.modifiers.contains(KeyModifiers::CONTROL);
    if control {
        return match tecla.code {
            KeyCode::Char('c') => Some(Accion::SalirInmediato),
            KeyCode::Char('r') => Some(Accion::Reescanear),
            KeyCode::Char('d') => Some(Accion::MediaAbajo),
            KeyCode::Char('u') => Some(Accion::MediaArriba),
            _ => None,
        };
    }
    match tecla.code {
        KeyCode::Char(caracter) => match caracter {
            'q' => Some(Accion::Salir),
            '1'..='6' => Vista::desde_numero(caracter as usize - '0' as usize).map(Accion::IrA),
            '?' => Some(Accion::Ayuda),
            'c' => Some(Accion::AlternarCola),
            't' => Some(Accion::RecargarTema),
            'j' => Some(Accion::Abajo),
            'k' => Some(Accion::Arriba),
            'h' => Some(Accion::Izquierda),
            'l' => Some(Accion::Derecha),
            'g' => Some(Accion::TeclaG),
            'G' => Some(Accion::Ultimo),
            ' ' => Some(Accion::AlternarPausa),
            'n' => Some(Accion::Siguiente),
            'p' => Some(Accion::Anterior),
            'x' => Some(Accion::Detener),
            ',' => Some(Accion::RetrocederCorto),
            '.' => Some(Accion::AvanzarCorto),
            '<' => Some(Accion::RetrocederLargo),
            '>' => Some(Accion::AvanzarLargo),
            '+' | '=' => Some(Accion::SubirVolumen),
            '-' | '_' => Some(Accion::BajarVolumen),
            'm' => Some(Accion::Silencio),
            's' => Some(Accion::AlternarAleatorio),
            'r' => Some(Accion::CiclarRepeticion),
            'a' => Some(Accion::AnadirCola),
            'A' => Some(Accion::ReproducirSiguiente),
            'P' => Some(Accion::AnadirPlaylist),
            'd' => Some(Accion::Quitar),
            'J' => Some(Accion::MoverAbajo),
            'K' => Some(Accion::MoverArriba),
            'N' => Some(Accion::NuevaPlaylist),
            'R' => Some(Accion::RenombrarPlaylist),
            'D' => Some(Accion::EliminarPlaylist),
            'e' => Some(Accion::ExportarPlaylist),
            'i' => Some(Accion::ImportarPlaylist),
            'C' => Some(Accion::VaciarCola),
            'o' => Some(Accion::CiclarOrden),
            'O' => Some(Accion::InvertirOrden),
            '/' => Some(Accion::EnfocarBuscar),
            _ => None,
        },
        KeyCode::Down => Some(Accion::Abajo),
        KeyCode::Up => Some(Accion::Arriba),
        KeyCode::Left => Some(Accion::Izquierda),
        KeyCode::Right => Some(Accion::Derecha),
        KeyCode::Home => Some(Accion::Primero),
        KeyCode::End => Some(Accion::Ultimo),
        KeyCode::Enter => Some(Accion::Abrir),
        KeyCode::Tab => Some(Accion::FocoSiguiente),
        KeyCode::BackTab => Some(Accion::FocoAnterior),
        KeyCode::Esc => Some(Accion::Cerrar),
        _ => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn tecla(codigo: KeyCode) -> KeyEvent {
        KeyEvent {
            code: codigo,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    fn tecla_control(caracter: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(caracter),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn traduce_globales() {
        assert_eq!(traducir(&tecla(KeyCode::Char('q'))), Some(Accion::Salir));
        assert_eq!(traducir(&tecla_control('c')), Some(Accion::SalirInmediato));
        assert_eq!(traducir(&tecla_control('r')), Some(Accion::Reescanear));
        assert_eq!(
            traducir(&tecla(KeyCode::Char('4'))),
            Some(Accion::IrA(Vista::Albumes))
        );
        assert_eq!(traducir(&tecla(KeyCode::Char('7'))), None);
        assert_eq!(traducir(&tecla(KeyCode::Char('?'))), Some(Accion::Ayuda));
    }

    #[test]
    fn traduce_reproduccion() {
        assert_eq!(
            traducir(&tecla(KeyCode::Char(' '))),
            Some(Accion::AlternarPausa)
        );
        assert_eq!(
            traducir(&tecla(KeyCode::Char('+'))),
            Some(Accion::SubirVolumen)
        );
        assert_eq!(
            traducir(&tecla(KeyCode::Char('-'))),
            Some(Accion::BajarVolumen)
        );
        assert_eq!(traducir(&tecla(KeyCode::Char('m'))), Some(Accion::Silencio));
        assert_eq!(
            traducir(&tecla(KeyCode::Char('s'))),
            Some(Accion::AlternarAleatorio)
        );
    }

    #[test]
    fn ignora_teclas_desconocidas() {
        assert_eq!(traducir(&tecla(KeyCode::Char('z'))), None);
        assert_eq!(traducir(&tecla(KeyCode::F(5))), None);
    }
}
