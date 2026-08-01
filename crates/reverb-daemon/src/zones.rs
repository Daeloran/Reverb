//! Les zones : une zone, une couche.
//!
//! Jusqu'ici le démon n'avait qu'un état — une couleur par cible **ou** une
//! animation, jamais les deux. Toute peinture tuait l'animation, toute animation
//! écrasait les peintures. Une zone est ce qui réconcilie les deux.
//!
//! # Le modèle, et ce qu'il refuse
//!
//! - une **zone** est un ensemble de LED que l'utilisateur compose lui-même, à
//!   la souris. Ni prédéfinie, ni contiguë : « le ventilateur arrière plus
//!   bas-milieu plus haut-milieu » est une zone légitime ;
//! - une LED appartient à **au plus une** zone. Mettre une LED dans une nouvelle
//!   zone la retire de celle qui la tenait — c'est ce qui évite d'avoir à gérer
//!   un ordre d'empilement, et donc une notion de transparence qu'une LED ne
//!   sait pas porter ;
//! - ce qui n'est dans aucune zone suit la couche **« tout le boîtier »**,
//!   c'est-à-dire l'état d'avant les zones ;
//! - chaque zone porte soit une **couleur fixe**, soit une **animation** avec
//!   ses propres réglages, donc sa propre vitesse et sa propre phase.
//!
//! # Une animation de zone se calcule sur le boîtier entier
//!
//! Une vague donnée à la colonne du radiateur traverse le **boîtier**, et la
//! zone n'en montre que sa part. C'est ce qui garde deux zones voisines
//! cohérentes entre elles, et ce qui évite d'inventer une géométrie par zone.
//!
//! Conséquence assumée : une vague sur une zone d'une seule LED clignote au lieu
//! de défiler. C'est le prix d'une composition qui ne ment pas sur l'espace.
//!
//! # Un second fichier, et non un format élargi
//!
//! `eclairage.conf` porte la couche globale, `zones.conf` les couches nommées.
//! Deux fichiers pour deux natures, comme `geometrie.conf` et `eclairage.conf`.
//!
//! ⚠️ Le format d'`eclairage.conf` ne bouge **pas** : trente-six tests
//! d'intention de #21 le décrivent, et un design venu après eux ne les réécrit
//! pas.

use std::fmt;
use std::io;
use std::path::Path;

use reverb_anim::{Animation, Geometrie, Reglages};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Led, Rgb};

/// Où les zones sont conservées d'un démarrage à l'autre.
///
/// Un état de service, donc `/var/lib` et non `/etc` : le fichier est écrit par
/// le démon, pas par un administrateur.
pub const CHEMIN_ZONES: &str = "/var/lib/reverb/zones.conf";

/// Les couleurs des cent vingt-quatre LED, avant écriture sur les bus.
#[derive(Debug, Clone, PartialEq)]
pub struct Tampon {
    pub ventilateurs: [[Rgb; LEDS_PER_FAN as usize]; 10],
    pub barrettes: [[Rgb; LEDS_PER_STICK]; SLOT_COUNT],
}

impl Tampon {
    pub fn noir() -> Tampon {
        Tampon {
            ventilateurs: [[Rgb::BLACK; LEDS_PER_FAN as usize]; 10],
            barrettes: [[Rgb::BLACK; LEDS_PER_STICK]; SLOT_COUNT],
        }
    }

    pub fn couleur(&self, _led: Led) -> Rgb {
        todo!("issue #29")
    }

    pub fn poser(&mut self, _led: Led, _couleur: Rgb) {
        todo!("issue #29")
    }
}

/// Ce qu'une zone affiche.
#[derive(Debug, Clone, PartialEq)]
pub enum Rendu {
    /// Elle ne masque rien : ses LED suivent la couche globale.
    Transparente,
    Fixe(Rgb),
    Animee(Animation, Reglages),
}

/// Une zone : un nom, un ensemble de LED, et ce qu'elle affiche.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub nom: String,
    /// Triées et sans doublon : deux zones de même composition sont égales,
    /// quel que soit l'ordre dans lequel on a cliqué.
    pub cibles: Vec<Led>,
    pub rendu: Rendu,
}

/// Toutes les zones, dans l'ordre de leur création.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Zones {
    _prive: (),
}

/// Un fichier de zones n'a pas pu être lu.
#[derive(Debug, Clone, PartialEq)]
pub struct ZonesInvalides {
    /// Numéro de ligne, **à partir de 1**, comme un éditeur. Zéro quand la
    /// faute ne tient à aucune ligne en particulier.
    pub ligne: usize,
    pub raison: String,
}

impl fmt::Display for ZonesInvalides {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ligne == 0 {
            write!(f, "{}", self.raison)
        } else {
            write!(f, "ligne {} : {}", self.ligne, self.raison)
        }
    }
}

impl std::error::Error for ZonesInvalides {}

impl Zones {
    pub fn vide() -> Zones {
        todo!("issue #29")
    }

    /// Crée ou redéfinit une zone.
    ///
    /// ⚠️ Les cibles données sont **retirées de toutes les autres zones**. Une
    /// zone qui se retrouve sans aucune LED est supprimée : une zone vide
    /// n'affiche rien et ne se désigne plus.
    ///
    /// Une zone qui existait garde son rendu ; une zone nouvelle naît
    /// transparente.
    pub fn poser(&mut self, _nom: &str, _cibles: &[Led]) {
        todo!("issue #29")
    }

    /// Supprime une zone. Ses LED reviennent à la couche globale sans
    /// s'éteindre. Faux si elle n'existait pas.
    pub fn retirer(&mut self, _nom: &str) -> bool {
        todo!("issue #29")
    }

    /// Donne une couleur fixe à une zone. Faux si elle n'existe pas.
    pub fn eclairer(&mut self, _nom: &str, _couleur: Rgb) -> bool {
        todo!("issue #29")
    }

    /// Donne une animation à une zone, ou la rend transparente avec `None`.
    /// Faux si elle n'existe pas.
    pub fn animer(&mut self, _nom: &str, _rendu: Option<(Animation, Reglages)>) -> bool {
        todo!("issue #29")
    }

    pub fn liste(&self) -> &[Zone] {
        todo!("issue #29")
    }

    pub fn zone(&self, _nom: &str) -> Option<&Zone> {
        todo!("issue #29")
    }

    /// Écrase, sur le tampon de la couche globale, ce que les zones affichent.
    ///
    /// Une zone transparente ne touche à rien. Une zone fixe pose sa couleur.
    /// Une zone animée calcule son image **sur la géométrie entière** et n'en
    /// prend que ses propres LED.
    pub fn composer(&self, _geometrie: &Geometrie, _pas: u32, _fond: &mut Tampon) {
        todo!("issue #29")
    }

    /// Le texte du fichier de zones.
    ///
    /// Une ligne `zone <nom> <cible>,<cible>,…` par zone, suivie de son rendu :
    /// `light <nom> <rrggbb>` ou `anim <nom> <animation> [clé=valeur…]`. Une
    /// zone transparente n'a pas de ligne de rendu.
    pub fn encoder(&self) -> String {
        todo!("issue #29")
    }

    /// L'inverse d'[`Zones::encoder`], strict.
    ///
    /// Refuse en nommant la ligne : un premier mot inconnu, un nom de zone
    /// répété, un rendu pour une zone qu'aucune ligne `zone` n'a déclarée, une
    /// LED illisible, une LED présente dans deux zones.
    pub fn decoder(_texte: &str) -> Result<Zones, ZonesInvalides> {
        todo!("issue #29")
    }
}

/// Lit le fichier de zones, en disant ce qui a cloché plutôt qu'en échouant.
///
/// Un fichier absent donne des zones vides : c'est le cas d'un premier
/// démarrage, et ce n'est pas une anomalie. Un fichier **abîmé** donne des zones
/// vides **et** un message : le démon doit démarrer sur la couche globale seule
/// plutôt que de refuser de s'allumer.
pub fn charger(_chemin: &Path) -> (Zones, Option<String>) {
    todo!("issue #29")
}

/// Écrit le fichier de zones, par fichier temporaire puis renommage.
pub fn enregistrer(_chemin: &Path, _zones: &Zones) -> io::Result<()> {
    todo!("issue #29")
}
