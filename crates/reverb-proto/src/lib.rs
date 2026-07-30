//! Encodage des trames des contrôleurs RGB NZXT.
//!
//! Ce crate est **pur** : il ne fait aucune entrée/sortie et ne connaît aucun
//! périphérique. Il transforme une intention (« ce ventilateur en magenta »)
//! en octets. C'est ici que vivent les tests.
//!
//! Toutes les valeurs proviennent de `docs/SPEC-PROTOCOLE-NZXT.md`, issue d'une
//! rétro-ingénierie validée sur le matériel. **Ne rien inventer** : une trame
//! absente de la spec est inconnue.

pub mod color;
pub mod frame;
pub mod position;

pub use color::{Brightness, Rgb};
pub use frame::{FRAME_LEN, Frame};
pub use position::{Placement, Position, UnknownPosition};

/// Modèle de contrôleur, qui détermine la séquence d'initialisation (spec §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Model {
    /// `1e71:2019` — RGB & Fan Controller, 6 canaux ARGB et 3 canaux ventilateur.
    RgbAndFan,
    /// `1e71:2012` — 2023 RGB Controller, 3 canaux ARGB.
    Rgb,
}

impl Model {
    /// Identifiant produit USB correspondant.
    pub const fn product_id(self) -> u16 {
        match self {
            Model::RgbAndFan => 0x2019,
            Model::Rgb => 0x2012,
        }
    }
}

/// Identifiant constructeur USB de NZXT.
pub const VENDOR_ID: u16 = 0x1E71;

/// Nombre de LED d'un ventilateur F140 RGB Core (spec §4, offset 58).
pub const LEDS_PER_FAN: u8 = 8;
