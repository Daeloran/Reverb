//! Positions physiques des ventilateurs dans le boîtier.
//!
//! L'utilisateur désigne un ventilateur par l'endroit où il se trouve, jamais
//! par un masque de bits ni un chemin de périphérique. La correspondance vient
//! de la spec §3, établie pendant la rétro-ingénierie en isolant chaque canal.
//!
//! ⚠️ Les canaux **ne sont pas interchangeables entre contrôleurs** : le canal
//! `0x01` du `2019` et le canal `0x01` du `2012` sont deux ventilateurs
//! différents. C'est pourquoi un `Placement` porte toujours le numéro de série.

use std::fmt;

use crate::Model;

/// Numéros de série des trois contrôleurs de cette machine.
///
/// Les deux `1e71:2012` sont physiquement identiques : la série est le **seul**
/// moyen de les distinguer.
pub const SERIAL_FAN_CONTROLLER: &str = "1303F00AAAAD9529610494BE";
/// Le contrôleur du ventilateur arrière, seul canal utilisé.
pub const SERIAL_RGB_ARRIERE: &str = "0E014044AB7664C25F063BD5";
/// Le contrôleur des trois ventilateurs du dessus.
pub const SERIAL_RGB_HAUT: &str = "1101F021AA358489609AA5B2";

/// Emplacement physique d'un ventilateur.
///
/// Disposition du boîtier : trois en bas, trois sur l'avant (le radiateur du
/// Kraken, plaqué contre la face de la carte mère), trois sur le dessus, un à
/// l'arrière.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Position {
    BasGauche,
    BasMilieu,
    BasDroite,
    RadiateurHaut,
    RadiateurMilieu,
    RadiateurBas,
    Arriere,
    HautGauche,
    HautMilieu,
    HautDroite,
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

impl fmt::Display for UnknownPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "position « {} » inconnue. Positions valides : {}",
            self.input,
            self.valid.join(", ")
        )
    }
}

impl std::error::Error for UnknownPosition {}

/// Noms sans espace ni accent, dans le même ordre que [`Position::ALL`].
///
/// Écrits en clair plutôt que dérivés de [`NAMES`] par translittération : une
/// règle de conversion des accents serait du code à maintenir pour dix chaînes
/// connues, et le jour où une position s'appellerait « côté œil » elle serait
/// fausse sans prévenir.
const SLUGS: [&str; 10] = [
    "bas-gauche",
    "bas-milieu",
    "bas-droite",
    "radiateur-haut",
    "radiateur-milieu",
    "radiateur-bas",
    "arriere",
    "haut-gauche",
    "haut-milieu",
    "haut-droite",
];

/// Noms des positions, dans le même ordre que [`Position::ALL`] (spec §3).
const NAMES: [&str; 10] = [
    "bas gauche",
    "bas milieu",
    "bas droite",
    "radiateur haut",
    "radiateur milieu",
    "radiateur bas",
    "arrière",
    "haut gauche",
    "haut milieu",
    "haut droite",
];

/// Variantes de saisie tolérées en plus de [`NAMES`].
///
/// « arrière » est le seul nom du jeu à porter un accent. L'imposer au clavier
/// serait une friction gratuite ; on accepte donc la forme sans accent, sans
/// pour autant introduire une normalisation générale des chaînes.
const ALIASES: [(&str, Position); 1] = [("arriere", Position::Arriere)];

impl Position {
    /// Les dix positions, dans un ordre stable.
    pub const ALL: [Position; 10] = [
        Position::BasGauche,
        Position::BasMilieu,
        Position::BasDroite,
        Position::RadiateurHaut,
        Position::RadiateurMilieu,
        Position::RadiateurBas,
        Position::Arriere,
        Position::HautGauche,
        Position::HautMilieu,
        Position::HautDroite,
    ];

    /// Nom lisible, tel que saisi par l'utilisateur.
    pub const fn name(self) -> &'static str {
        NAMES[self.index()]
    }

    /// Rang de la position dans [`Position::ALL`], [`Position::names`] et
    /// [`Position::slug`].
    ///
    /// Public depuis l'issue #17 : le démon garde l'état des dix ventilateurs
    /// dans un tableau, et ce rang est la seule clé qui ne dépende ni du nom
    /// d'affichage ni du numéro de canal.
    pub const fn index(self) -> usize {
        match self {
            Position::BasGauche => 0,
            Position::BasMilieu => 1,
            Position::BasDroite => 2,
            Position::RadiateurHaut => 3,
            Position::RadiateurMilieu => 4,
            Position::RadiateurBas => 5,
            Position::Arriere => 6,
            Position::HautGauche => 7,
            Position::HautMilieu => 8,
            Position::HautDroite => 9,
        }
    }

    /// Tous les noms acceptés, dans le même ordre que [`Position::ALL`].
    pub fn names() -> &'static [&'static str] {
        &NAMES
    }

    /// Résout un nom saisi par l'utilisateur.
    ///
    /// Accepte aussi les variantes de [`ALIASES`].
    pub fn from_name(input: &str) -> Result<Self, UnknownPosition> {
        Position::ALL
            .into_iter()
            .find(|position| position.name() == input)
            .or_else(|| {
                ALIASES
                    .iter()
                    .find(|(alias, _)| *alias == input)
                    .map(|(_, position)| *position)
            })
            .ok_or_else(|| UnknownPosition {
                input: input.to_owned(),
                valid: &NAMES,
            })
    }

    /// Nom sans espace ni accent, pour le protocole IPC.
    ///
    /// Les noms d'affichage portent une espace (« radiateur haut ») et un
    /// accent (« arrière ») : une position à espace casserait le découpage
    /// d'une ligne en jetons.
    pub fn slug(self) -> String {
        SLUGS[self.index()].to_owned()
    }

    /// Réciproque de [`Position::slug`].
    ///
    /// Stricte : ni les noms d'affichage, ni les variantes de casse, ni les
    /// alias. C'est un dialogue entre deux programmes, pas une saisie humaine —
    /// [`Position::from_name`] reste la porte tolérante, celle de la ligne de
    /// commande.
    pub fn from_slug(input: &str) -> Result<Self, UnknownPosition> {
        Position::ALL
            .into_iter()
            .find(|position| SLUGS[position.index()] == input)
            .ok_or_else(|| UnknownPosition {
                input: input.to_owned(),
                valid: &SLUGS,
            })
    }

    /// Contrôleur et canal correspondants (spec §3).
    pub const fn placement(self) -> Placement {
        // Un bit par canal. Les masques se répètent d'un contrôleur à l'autre :
        // seule la série lève l'ambiguïté.
        let (serial, model, mask) = match self {
            Position::BasGauche => (SERIAL_FAN_CONTROLLER, Model::RgbAndFan, 0x01),
            Position::BasMilieu => (SERIAL_FAN_CONTROLLER, Model::RgbAndFan, 0x02),
            Position::BasDroite => (SERIAL_FAN_CONTROLLER, Model::RgbAndFan, 0x04),
            Position::RadiateurHaut => (SERIAL_FAN_CONTROLLER, Model::RgbAndFan, 0x08),
            Position::RadiateurMilieu => (SERIAL_FAN_CONTROLLER, Model::RgbAndFan, 0x10),
            Position::RadiateurBas => (SERIAL_FAN_CONTROLLER, Model::RgbAndFan, 0x20),
            Position::Arriere => (SERIAL_RGB_ARRIERE, Model::Rgb, 0x01),
            Position::HautGauche => (SERIAL_RGB_HAUT, Model::Rgb, 0x01),
            Position::HautMilieu => (SERIAL_RGB_HAUT, Model::Rgb, 0x02),
            Position::HautDroite => (SERIAL_RGB_HAUT, Model::Rgb, 0x04),
        };
        Placement {
            serial,
            model,
            mask,
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
