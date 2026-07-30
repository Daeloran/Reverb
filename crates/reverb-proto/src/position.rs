//! Positions physiques des ventilateurs dans le boîtier.
//!
//! L'utilisateur désigne un ventilateur par l'endroit où il se trouve, jamais
//! par un masque de bits ni un chemin de périphérique. La correspondance vient
//! de la spec §3, établie pendant la rétro-ingénierie en isolant chaque canal.
//!
//! ⚠️ Les canaux **ne sont pas interchangeables entre contrôleurs** : le canal
//! `0x01` du `2019` et le canal `0x01` du `2012` sont deux ventilateurs
//! différents. C'est pourquoi un `Placement` porte toujours le numéro de série.

use crate::Model;

/// Numéros de série des trois contrôleurs de cette machine.
///
/// Les deux `1e71:2012` sont physiquement identiques : la série est le **seul**
/// moyen de les distinguer.
pub const SERIAL_FAN_CONTROLLER: &str = "1303F00AAAAD9529610494BE";
pub const SERIAL_RGB_SINGLE: &str = "0E014044AB7664C25F063BD5";
pub const SERIAL_RGB_TRIPLE: &str = "1101F021AA358489609AA5B2";

/// Emplacement physique d'un ventilateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Position {
    BasGauche,
    BasMilieu,
    BasDroite,
    DroitBas,
    DroitMilieu,
    DroitHaut,
    Gauche,
    HautDroite,
    HautMilieu,
    HautGauche,
}

/// Où se trouve réellement un ventilateur : quel contrôleur, quel canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub serial: &'static str,
    pub model: Model,
    /// Masque de canal — un bit par canal (spec §3).
    pub mask: u8,
}

/// Erreur rendue lorsqu'un nom de position ne correspond à rien.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownPosition {
    pub input: String,
    /// Les noms acceptés, pour que le message d'erreur soit exploitable.
    pub valid: &'static [&'static str],
}

impl Position {
    /// Les dix positions, dans un ordre stable.
    pub const ALL: [Position; 10] = [
        Position::BasGauche,
        Position::BasMilieu,
        Position::BasDroite,
        Position::DroitBas,
        Position::DroitMilieu,
        Position::DroitHaut,
        Position::Gauche,
        Position::HautDroite,
        Position::HautMilieu,
        Position::HautGauche,
    ];

    /// Nom lisible, tel que saisi par l'utilisateur.
    pub const fn name(self) -> &'static str {
        todo!()
    }

    /// Tous les noms acceptés, dans le même ordre que [`Position::ALL`].
    pub fn names() -> &'static [&'static str] {
        todo!()
    }

    /// Résout un nom saisi par l'utilisateur.
    pub fn from_name(input: &str) -> Result<Self, UnknownPosition> {
        let _ = input;
        todo!()
    }

    /// Contrôleur et canal correspondants (spec §3).
    pub const fn placement(self) -> Placement {
        todo!()
    }
}
