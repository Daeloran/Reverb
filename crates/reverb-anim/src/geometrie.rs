//! Où se trouve physiquement chaque LED du boîtier.
//!
//! ⚠️ **SQUELETTE — signatures seules, aucun corps.** Les valeurs viendront de
//! la mesure décrite par `tools/mesure_orientation.sh`.

use std::fmt;

use reverb_proto::Position;

/// Sens de rotation de l'anneau de LED, vu depuis l'**extérieur** du boîtier.
///
/// Le protocole ne donne que l'ordre des indices (SPEC-PROTOCOLE-NZXT §5) : le
/// sens apparent dépend de la face par laquelle on regarde le ventilateur,
/// donc du montage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sens {
    Horaire,
    Antihoraire,
}

/// Où se trouve la LED 1 d'un ventilateur, et dans quel sens l'anneau tourne.
///
/// `angle` en degrés : 0 = midi, croissant dans le sens horaire vu de
/// l'extérieur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Orientation {
    pub angle: u16,
    pub sens: Sens,
}

/// Erreur rendue par [`Orientation::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrientationInvalide {
    pub champ: &'static str,
    pub raison: String,
}

impl fmt::Display for OrientationInvalide {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl std::error::Error for OrientationInvalide {}

impl Orientation {
    /// Refuse un angle hors `0..=359`.
    pub fn new(_angle: u16, _sens: Sens) -> Result<Orientation, OrientationInvalide> {
        todo!()
    }

    /// Angle absolu de la LED d'indice donné (`0..8`), en degrés.
    pub fn angle_led(&self, _led: usize) -> u16 {
        todo!()
    }
}

/// Un point du boîtier, en millimètres.
///
/// `x` du flanc gauche vers le flanc droit, `y` du plancher vers le plafond,
/// `z` de l'avant vers l'arrière. Trois axes et non deux : le boîtier a quatre
/// plans occupés, et une projection choisie ici serait un choix d'affichage
/// gelé dans la donnée.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Où se trouve chaque LED, et comment chaque ventilateur est monté.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometrie {
    _prive: (),
}

/// Erreur rendue par [`Geometrie::decoder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometrieInvalide {
    pub ligne: usize,
    pub champ: &'static str,
    pub raison: String,
}

impl fmt::Display for GeometrieInvalide {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl std::error::Error for GeometrieInvalide {}

impl Geometrie {
    /// La géométrie mesurée sur SHYNAEL.
    pub fn mesuree() -> Geometrie {
        todo!()
    }

    pub fn orientation(&self, _position: Position) -> Orientation {
        todo!()
    }

    pub fn definir(&mut self, _position: Position, _orientation: Orientation) {
        todo!()
    }

    /// Position d'une LED de ventilateur. `led` dans `0..8`.
    pub fn led_ventilateur(&self, _position: Position, _led: usize) -> Option<Point> {
        todo!()
    }

    /// Position d'une LED de barrette. `slot` dans `0..4`, `led` dans `0..11`.
    pub fn led_barrette(&self, _slot: usize, _led: usize) -> Option<Point> {
        todo!()
    }

    /// Coin bas-avant-gauche et coin haut-arrière-droit du volume occupé.
    ///
    /// Sert aux animations à normaliser sans coder de dimensions en dur.
    pub fn bornes(&self) -> (Point, Point) {
        todo!()
    }

    /// Une ligne par ventilateur : `<position-slug> <angle> <sens>`.
    pub fn encoder(&self) -> String {
        todo!()
    }

    /// Réciproque exacte d'[`Geometrie::encoder`].
    pub fn decoder(_texte: &str) -> Result<Geometrie, GeometrieInvalide> {
        todo!()
    }
}
