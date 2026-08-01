//! L'écran du Kraken : ce qu'il affiche, et comment on le dessine.
//!
//! Le démon **détient** désormais la dalle. `peripheriques.rs` annonçait la
//! condition : « il rejoindra le démon quand la fenêtre en aura besoin ». La
//! fenêtre n'ouvre aucun périphérique (ADR-002), elle envoie donc un chemin de
//! fichier et le démon lit lui-même — jamais 1,2 Mo de pixels sur un protocole
//! texte.
//!
//! ⚠️ **Régression de capacité assumée** : `reverb screen --image` en direct ne
//! marche plus pendant que le démon tourne, le nœud USB ne se réclamant pas deux
//! fois. Elle est compensée — la ligne de commande passe par le socket comme la
//! fenêtre, et y gagne le PNG, le JPEG et le GIF qu'elle n'avait pas.
//!
//! # Le cadran ne dépend d'aucune pile de texte
//!
//! Des chiffres à sept segments dessinés à la main dans le tampon 640×640, plus
//! un anneau de proportion. Charger une pile de rendu de police pour afficher
//! « 40.5 » serait hors de proportion avec le besoin, et ajouterait une
//! bibliothèque système à un démon qui n'en veut pas.
//!
//! # Le firmware reprend la main au bout de trente secondes
//!
//! `screen::FIRMWARE_FALLBACK_SECS`. Ce qui est affiché doit donc être réémis
//! avant — d'où `screen::REFRESH_INTERVAL_SECS`, vingt-cinq. Un cadran s'en
//! sert pour se rafraîchir ; une image fixe aussi, sans quoi elle disparaîtrait
//! toute seule.

use std::fmt;
use std::io;
use std::path::Path;
use std::time::Duration;

/// Où l'état de l'écran est conservé d'un démarrage à l'autre.
pub const CHEMIN_ECRAN: &str = "/var/lib/reverb/ecran.conf";

/// Ce que la dalle montre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Affichage {
    /// Rendue au firmware.
    Rien,
    /// Une sonde, en gros, avec son unité.
    Cadran(String),
    /// Une image fixe, mise à l'échelle par le démon.
    Image(String),
    /// Une animation, jouée en boucle.
    Gif(String),
}

/// L'état de l'écran : sa luminosité et ce qu'il montre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Etat {
    /// De 0 à 100. Zéro éteint la dalle **sans** perdre ce qu'elle affichait.
    pub luminosite: u8,
    pub affichage: Affichage,
}

/// Un fichier d'écran n'a pas pu être lu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtatInvalide {
    pub ligne: usize,
    pub raison: String,
}

impl fmt::Display for EtatInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ligne == 0 {
            write!(f, "{}", self.raison)
        } else {
            write!(f, "ligne {} : {}", self.ligne, self.raison)
        }
    }
}

impl std::error::Error for EtatInvalide {}

impl Etat {
    /// Ce que la dalle montre au premier démarrage : rien, à pleine luminosité.
    pub fn accueil() -> Etat {
        Etat {
            luminosite: 100,
            affichage: Affichage::Rien,
        }
    }

    /// Le texte du fichier : une ligne `brightness`, une ligne `affiche`.
    pub fn encoder(&self) -> String {
        todo!("issue #33")
    }

    /// L'inverse, strict, en nommant la ligne fautive.
    pub fn decoder(_texte: &str) -> Result<Etat, EtatInvalide> {
        todo!("issue #33")
    }
}

/// Lit le fichier d'écran, en disant ce qui a cloché plutôt qu'en échouant.
pub fn charger(_chemin: &Path) -> (Etat, Option<String>) {
    todo!("issue #33")
}

/// Écrit le fichier d'écran, par fichier provisoire puis renommage.
pub fn enregistrer(_chemin: &Path, _etat: &Etat) -> io::Result<()> {
    todo!("issue #33")
}

/// Une image prête pour le bus : 640×640 en BGR, dans l'ordre du Kraken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dalle {
    _prive: (),
}

/// Une image n'a pas pu être lue ou n'a pas sa place sur la dalle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInvalide {
    pub raison: String,
}

impl fmt::Display for ImageInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raison)
    }
}

impl std::error::Error for ImageInvalide {}

impl Dalle {
    pub fn noire() -> Dalle {
        todo!("issue #33")
    }

    /// Les octets à pousser sur l'endpoint bulk.
    ///
    /// Toujours `screen::IMAGE_LEN`, quelle que soit l'image de départ.
    pub fn octets(&self) -> &[u8] {
        todo!("issue #33")
    }

    /// Lit un fichier et le met à l'échelle de la dalle.
    ///
    /// Rend **plusieurs** dalles pour un GIF, une seule pour une image fixe.
    ///
    /// L'image est mise à l'échelle **sans déformer ses proportions**, puis
    /// centrée sur du noir : une photo étirée en carré est laide et personne ne
    /// l'a demandée.
    ///
    /// ⚠️ Le chemin doit être **absolu**. Le démon ne partage pas le répertoire
    /// courant de son client, et un chemin relatif y désignerait autre chose —
    /// ou rien.
    pub fn depuis_fichier(_chemin: &Path) -> Result<Vec<Dalle>, ImageInvalide> {
        todo!("issue #33")
    }

    /// Le cadran d'une sonde.
    ///
    /// `valeur` absente quand la sonde ne répond plus : la dalle le **dit** au
    /// lieu de figer la dernière valeur lue.
    ///
    /// `proportion` sert l'anneau qui entoure le chiffre, de 0 à 1 ; hors de ces
    /// bornes, il est ramené dedans plutôt que de déborder.
    pub fn cadran(_libelle: &str, _valeur: Option<f32>, _unite: &str, _proportion: f32) -> Dalle {
        todo!("issue #33")
    }
}

/// Les délais d'un GIF, ramenés à ce que le bus tient.
///
/// Une image de 1,2 Mo met environ cent millisecondes à passer : un GIF à
/// trente images par seconde demanderait trois fois le débit disponible. On le
/// **ralentit** au lieu de sauter des images — un mouvement lent et complet se
/// regarde, un mouvement saccadé non.
pub fn cadence(_delais: &[Duration], _plancher: Duration) -> Vec<Duration> {
    todo!("issue #33")
}
