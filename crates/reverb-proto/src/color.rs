//! Couleurs et luminosité.
//!
//! ⚠️ Les contrôleurs de ventilateurs attendent les composantes dans l'ordre
//! **GRB** (spec §2). L'écran du Kraken utilise BGR et la RAM Corsair RGB —
//! d'où l'isolement strict de la conversion dans ce module.

/// Erreur d'analyse d'une couleur hexadécimale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseColorError {
    pub input: String,
}

/// Une couleur, en composantes rouge/vert/bleu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    /// Analyse une couleur hexadécimale à six chiffres, avec ou sans `#`.
    pub fn from_hex(input: &str) -> Result<Self, ParseColorError> {
        let _ = input;
        todo!()
    }

    /// Applique la luminosité en multipliant les composantes.
    ///
    /// Le protocole NZXT **n'a aucun octet de luminosité** : CAM l'applique
    /// côté hôte avant l'envoi (spec §4.3). Une luminosité nulle produit donc
    /// du noir, pas une LED éteinte par une commande dédiée.
    pub fn with_brightness(self, brightness: Brightness) -> Self {
        let _ = brightness;
        todo!()
    }

    /// Sérialise dans l'ordre attendu par les contrôleurs de ventilateurs (spec §2).
    pub fn to_grb(self) -> [u8; 3] {
        todo!()
    }
}

/// Luminosité en pourcentage, bornée à `0..=100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Brightness(u8);

impl Brightness {
    pub const FULL: Brightness = Brightness(100);

    /// Construit une luminosité, en écrêtant au-delà de 100.
    pub const fn new(percent: u8) -> Self {
        let _ = percent;
        todo!()
    }

    pub const fn percent(self) -> u8 {
        self.0
    }
}

impl Default for Brightness {
    fn default() -> Self {
        Brightness::FULL
    }
}
