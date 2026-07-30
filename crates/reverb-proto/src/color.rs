//! Couleurs et luminosité.
//!
//! ⚠️ Les contrôleurs de ventilateurs attendent les composantes dans l'ordre
//! **GRB** (spec §2). L'écran du Kraken utilise BGR et la RAM Corsair RGB —
//! d'où l'isolement strict de la conversion dans ce module.

use std::fmt;

/// Erreur d'analyse d'une couleur hexadécimale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseColorError {
    pub input: String,
}

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "couleur « {} » invalide : attendu six chiffres hexadécimaux, par exemple ff00ff ou #ff00ff",
            self.input
        )
    }
}

impl std::error::Error for ParseColorError {}

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
        let invalide = || ParseColorError {
            input: input.to_owned(),
        };

        let chiffres = input.strip_prefix('#').unwrap_or(input);
        if chiffres.len() != 6 || !chiffres.bytes().all(|o| o.is_ascii_hexdigit()) {
            return Err(invalide());
        }

        // Le découpage est sûr : six caractères, tous hexadécimaux ASCII.
        let composante = |debut: usize| {
            u8::from_str_radix(&chiffres[debut..debut + 2], 16).map_err(|_| invalide())
        };

        Ok(Rgb {
            r: composante(0)?,
            g: composante(2)?,
            b: composante(4)?,
        })
    }

    /// Applique la luminosité en multipliant les composantes.
    ///
    /// Le protocole NZXT **n'a aucun octet de luminosité** : CAM l'applique
    /// côté hôte avant l'envoi (spec §4.3). Une luminosité nulle produit donc
    /// du noir, pas une LED éteinte par une commande dédiée.
    ///
    /// L'arrondi est une **troncature**, comme l'implémentation de référence de
    /// la spec §11 (`int(c * luminosite)`). La spec ne documente pas la règle
    /// d'arrondi de CAM : ce choix est le nôtre, pas une observation du matériel.
    pub fn with_brightness(self, brightness: Brightness) -> Self {
        let echelle = |composante: u8| {
            ((u16::from(composante) * u16::from(brightness.percent())) / 100) as u8
        };
        Rgb {
            r: echelle(self.r),
            g: echelle(self.g),
            b: echelle(self.b),
        }
    }

    /// Sérialise dans l'ordre attendu par les contrôleurs de ventilateurs (spec §2).
    pub fn to_grb(self) -> [u8; 3] {
        [self.g, self.r, self.b]
    }
}

/// Luminosité en pourcentage, bornée à `0..=100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Brightness(u8);

impl Brightness {
    pub const FULL: Brightness = Brightness(100);

    /// Construit une luminosité, en écrêtant au-delà de 100.
    pub const fn new(percent: u8) -> Self {
        Brightness(if percent > 100 { 100 } else { percent })
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
