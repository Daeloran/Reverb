//! Les animations calculées par l'hôte, et la géométrie qu'elles traversent.
//!
//! Ce crate est **pur** : aucune entrée/sortie, aucun périphérique. Il est
//! importé par le démon *et* par la fenêtre, pour que l'aperçu du boîtier
//! affiche exactement les images écrites sur le bus — pas une
//! réimplémentation qui divergerait à la première animation ajoutée d'un seul
//! côté.
//!
//! Il est distinct de `reverb-proto`, dont la règle est de ne contenir que du
//! relevé matériel : ces animations sont de nous. Seule leur **nécessité** est
//! observée — le contrôleur de la RAM ne sait pas animer seul
//! (SPEC-CORSAIR-RAM §4.5).
//!
//! ⚠️ **SQUELETTE — signatures seules, aucun corps.**

use std::fmt;

use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

pub mod geometrie;

pub use geometrie::{Geometrie, GeometrieInvalide, Orientation, OrientationInvalide, Point, Sens};

/// Une image complète : les dix ventilateurs, puis les quatre barrettes.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub ventilateurs: [(Position, [Rgb; LEDS_PER_FAN as usize]); 10],
    pub barrettes: [[Rgb; LEDS_PER_STICK]; SLOT_COUNT],
}

/// Direction d'un motif dans le boîtier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    BasHaut,
    HautBas,
    AvantArriere,
    ArriereAvant,
    Horaire,
    Antihoraire,
}

/// Les réglages d'une animation.
#[derive(Debug, Clone, PartialEq)]
pub struct Reglages {
    pub couleur: Rgb,
    pub vitesse: u8,
    pub direction: Direction,
}

impl Default for Reglages {
    fn default() -> Reglages {
        todo!()
    }
}

/// Erreur rendue lorsqu'un nom d'animation ne figure pas au [`CATALOGUE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationInconnue {
    pub saisi: String,
    pub valides: &'static [&'static str],
}

impl fmt::Display for AnimationInconnue {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl std::error::Error for AnimationInconnue {}

/// Erreur rendue lorsqu'un réglage est inconnu ou hors bornes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReglageInvalide {
    /// La clé fautive, toujours nommée — un refus muet ferait chercher.
    pub cle: String,
    pub raison: String,
    /// Les clés acceptées par l'animation visée.
    pub acceptees: &'static [&'static str],
}

impl fmt::Display for ReglageInvalide {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl std::error::Error for ReglageInvalide {}

/// Une animation du catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Animation {
    _prive: (),
}

impl Animation {
    pub fn par_nom(_nom: &str) -> Result<Animation, AnimationInconnue> {
        todo!()
    }

    pub fn nom(&self) -> &'static str {
        todo!()
    }

    /// Les clés que cette animation accepte — la seule source de vérité du
    /// refus, pour qu'ajouter un paramètre sans l'accepter soit impossible.
    pub fn parametres_acceptes(&self) -> &'static [&'static str] {
        todo!()
    }

    /// Valide des paires brutes venues du protocole.
    pub fn reglages(&self, _paires: &[(String, String)]) -> Result<Reglages, ReglageInvalide> {
        todo!()
    }

    /// L'image du pas donné. Fonction pure : mêmes entrées, même sortie.
    pub fn image(&self, _geometrie: &Geometrie, _reglages: &Reglages, _pas: u32) -> Image {
        todo!()
    }
}

/// Les animations que le démon sait jouer.
///
/// `vague` y figure : le protocole s'étend, il ne casse pas.
pub const CATALOGUE: &[&str] = &["vague"];
