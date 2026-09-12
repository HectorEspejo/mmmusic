pub mod analisis;
pub mod anillo;
pub mod pipewire;

pub use analisis::{Analisis, Analizador};
pub use anillo::Anillo;
pub use pipewire::{ComandoCaptura, EstadoCaptura, ManejoCaptura};
