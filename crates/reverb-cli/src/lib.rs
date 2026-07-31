//! Bibliothèque du binaire `reverb` : découverte du matériel et analyse des
//! arguments. Séparée de `main.rs` pour rester testable sans matériel.

pub mod cli;
pub mod hidraw;
pub mod hwmon;
pub mod usbfs;
