//! Encodage des trames des périphériques RGB pilotés par Reverb.
//!
//! Ce crate est **pur** : il ne fait aucune entrée/sortie et ne connaît aucun
//! périphérique. Il transforme une intention (« ce ventilateur en magenta »)
//! en octets. C'est ici que vivent les tests.
//!
//! Toutes les valeurs proviennent des specs de `docs/` — `SPEC-PROTOCOLE-NZXT.md`
//! pour les contrôleurs et l'éclairage, `SPEC-KRAKEN-LCD.md` pour l'écran,
//! `SPEC-CORSAIR-RAM.md` pour la RAM — chacune issue d'une rétro-ingénierie
//! validée sur le matériel. **Ne rien inventer** : une trame absente des specs
//! est inconnue.

pub mod color;
pub mod frame;
pub mod ipc;
pub mod led;
pub mod mode;
pub mod position;
pub mod profil;
pub mod ram;
pub mod screen;

pub use color::{Brightness, Rgb, Tsl, TslInvalide};
pub use frame::{FRAME_LEN, Frame};
pub use led::{Apply, Led, LedCountError, LedInconnue};
pub use mode::{ColorCountError, Mode, UnknownMode};
pub use position::{Placement, Position, UnknownPosition};
pub use profil::{NomInvalide, NomProfil};
pub use screen::{BrightnessError, ImageError, ScreenState, StateError};

// `ram` n'est volontairement pas ré-exporté ici : son `LedCountError` entrerait
// en collision avec celui de `led`, et un alias de ré-export coûterait plus
// cher en lisibilité qu'un chemin de module. On écrit `ram::payload(…)`.

/// Modèle de contrôleur, qui détermine la séquence d'initialisation (spec §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Model {
    /// `1e71:2019` — RGB & Fan Controller, 6 canaux ARGB et 3 canaux ventilateur.
    RgbAndFan,
    /// `1e71:2012` — 2023 RGB Controller, 3 canaux ARGB.
    Rgb,
}

impl Model {
    /// Tous les modèles supportés.
    ///
    /// Sert à vérifier que la règle udev livrée par le dépôt les couvre tous :
    /// ajouter une variante sans ajouter sa ligne dans
    /// `packaging/60-reverb.rules` échouerait sinon en silence, à l'exécution,
    /// sur la machine de quelqu'un d'autre.
    pub const ALL: [Model; 2] = [Model::RgbAndFan, Model::Rgb];

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
