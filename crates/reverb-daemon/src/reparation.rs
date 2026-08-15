//! Ce que le démon a le droit de tenter quand une source entière se tait (issue #98).
//!
//! # Le défaut
//!
//! Relevé sur SHYNAEL le 2026-08-15 :
//!
//! ```text
//! 12:29:49  écran : pas de trame 37 02 en 2s — le contrôleur ne répond plus
//! 12:29:56  canal « kraken2023elite:fan-speed » écarté : Connection timed out
//! 12:29:56  canal « kraken2023elite:pump-speed » écarté : Connection timed out
//! 12:30:01  sonde « kraken2023elite:coolant-temp » écartée : Connection timed out
//! 12:30:41  3 échecs d'affilée sur la dalle → écran rendu au firmware
//! ```
//!
//! Le firmware du Kraken cesse de répondre **en gardant son lien USB** : le
//! périphérique reste énuméré, le journal noyau ne dit rien, et la lecture échoue
//! hors de Reverb, dans un simple shell. Tout ce que le contrôleur porte tombe
//! d'un bloc — la dalle, les deux canaux de vitesse, la sonde de liquide.
//!
//! La quarantaine de #68 et #88 fait exactement son travail : elle empêche une
//! lecture muette de geler le fil qui sert le socket. Mais elle est **purement
//! défensive** — elle attend un rétablissement qui, sur les deux incidents
//! relevés, n'est jamais venu avant un redémarrage.
//!
//! ⚠️ **La cause reste inconnue, et #98 traite la conséquence.** Ce module ne dit
//! donc rien de ce qui plante le Kraken ; il dit ce que le démon a le droit de
//! tenter, et surtout ce qu'il n'a pas le droit de tenter.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne réinitialise rien, n'ouvre rien, ne connaît aucun chemin et ne tient
//! aucune horloge : **le geste arrive en fermeture et l'instant en paramètre**.
//! C'est le parti de `Quarantaine` (#68) et de `Vigie` (#70), et il pèse plus
//! lourd ici — un `USBDEVFS_RESET` fait disparaître puis réapparaître un
//! périphérique sur le bus, et un test qui compte les appels est le seul moyen de
//! vérifier qu'il n'a **pas** lieu.
//!
//! C'est aussi ce qui permet à la décision de vivre **hors du fil qui sert le
//! socket**, comme la dalle depuis #83 : elle est pure, donc `Send`, donc elle
//! voyage. Voir [`crate::fil_reparation`].
//!
//! # Deux règles qui ne se devinent pas
//!
//! **C'est la source qui répond à nouveau qui remet le compteur à zéro, jamais
//! l'`ioctl` qui rend `Ok`.** `USBDEVFS_RESET` réussit dès que le noyau a
//! réinitialisé le port ; il ne dit rien de ce que le firmware fait ensuite, et
//! l'incident relevé est précisément celui d'un périphérique **énuméré qui ne
//! répond plus**. Si la réussite du geste relançait le compte, le démon
//! repartirait indéfiniment au premier essai et secouerait le Kraken jusqu'au
//! redémarrage : le plafond serait écrit et inatteignable.
//!
//! **Une source dont on ne connaît aucune cible n'est pas effondrée.** « Toutes
//! ses cibles se taisent » est vrai à vide, et cette vérité-là est un défaut : une
//! découverte qui a échoué, un `hwmon` dépouillé, une source repérée avant ses
//! cibles — et le démon réinitialiserait un périphérique dont il n'a jamais rien
//! lu.

use std::collections::HashMap;
use std::io;
use std::time::Duration;

/// Le nombre de tentatives pour un même effondrement.
///
/// Trois, comme le plafond de refus de la dalle (`ecran::ECHECS_AVANT_ABANDON`),
/// et pour la même raison : « insister sur un périphérique en difficulté ne le
/// réveille pas » est la leçon de #70. L'insistance coûte ici bien plus cher —
/// chaque essai fait quitter le bus au périphérique — et sur les deux incidents
/// relevés le Kraken n'est jamais revenu sans redémarrage.
pub const TENTATIVES_MAXIMALES: u32 = 3;

/// Le délai qui sépare deux tentatives.
///
/// Trente secondes, et la valeur se raisonne : entre deux resets, il faut avoir
/// eu le temps de **juger** le précédent. Cela veut dire laisser le périphérique
/// se réénumérer, redécouvrir ce qu'il porte, puis relire chacune de ses cibles —
/// et une lecture qui bute sur un contrôleur muet coûte 5,218 s en sommeil non
/// interruptible (mesuré dans #68). Trois cibles, c'est déjà quinze secondes rien
/// que pour apprendre que rien n'a changé. Trente double cette marge.
///
/// En deçà, ce ne serait plus espacer, ce serait boucler. Au-delà, la série
/// entière durerait plus longtemps que la patience de quiconque regarde un
/// boîtier dont la pompe ne remonte plus sa température.
pub const DELAI_ENTRE_TENTATIVES: Duration = Duration::from_secs(30);

/// Ce qu'une source `hwmon` porte, et ce qui se tait, à l'instant du tour.
///
/// ⚠️ **Rien ici n'est un chemin.** Les numéros `hwmonN` changent au redémarrage —
/// et, depuis #98, à chaque reset. Une source et ses cibles se **nomment** ; c'est
/// ce qui rend la redécouverte possible, et c'est ce qui garde cette décision
/// transportable d'un fil à l'autre sans rien emporter d'ouvert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtatSource {
    /// Le nom de la source, tel que `hwmon` le donne — « kraken2023elite ».
    pub source: String,
    /// Toutes ses cibles connues, canaux et sondes confondus, par leur nom de
    /// protocole.
    pub cibles: Vec<String>,
    /// Celles dont la **dernière lecture** a échoué.
    ///
    /// ⚠️ Ce n'est pas tout à fait « celles qui dorment leur quarantaine » : une
    /// réparation réussie **oublie** la quarantaine d'une cible pour la relire
    /// sans délai, mais la cible n'a pas répondu pour autant. Confondre les deux
    /// ferait repartir le compte de tentatives à chaque reset, et le plafond
    /// serait inatteignable.
    pub muettes: Vec<String>,
}

/// Ce qu'un tour de décision a produit pour une source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constat {
    /// La source répond, au moins en partie : rien à faire, et tout compteur est
    /// remis à zéro.
    Rien,
    /// La source est entièrement muette, mais la prochaine tentative n'est pas
    /// due.
    Patiente,
    /// Une tentative vient d'avoir lieu et le geste a réussi. `tentative` est son
    /// rang, à partir de 1. Les cibles de la source sont à **redécouvrir par leur
    /// nom**, et leurs quarantaines à libérer.
    ///
    /// ⚠️ « Réussie » qualifie l'`ioctl`, pas la guérison : un reset peut rendre
    /// `Ok` sans que le périphérique revienne.
    Reussie { tentative: u32 },
    /// Une tentative vient d'avoir lieu et le geste a échoué. **Rien n'a
    /// changé**, quarantaines comprises.
    Echouee { tentative: u32, erreur: String },
    /// Le plafond est atteint et la source est toujours muette : plus aucune
    /// tentative. Rendu **une seule fois** par épisode, pour que l'appelant
    /// journalise une ligne sans avoir à s'en souvenir.
    Abandon,
    /// L'abandon est déjà prononcé : rien n'a été tenté, rien n'est à dire.
    Repos,
}

/// Où en est une source dans son effondrement courant.
#[derive(Debug, Clone, Default)]
struct Episode {
    /// Combien de resets ont déjà été tentés. Décide de l'abandon.
    tentatives: u32,
    /// Quand la dernière a eu lieu. `None` : aucune encore.
    ///
    /// ⚠️ **Le délai court depuis la dernière tentative, pas depuis
    /// l'effondrement.** Sur une série régulière les deux coïncident ; ils
    /// divergent dès qu'un tour est sauté, et c'est la première lecture qui a un
    /// sens — ce qu'on espace, ce sont les gestes.
    derniere: Option<Duration>,
    /// L'abandon a-t-il déjà été prononcé ? Une seule fois par épisode.
    abandonne: bool,
}

/// L'état de réparation de chaque source.
///
/// ⚠️ **Un épisode par source, jamais un compteur global.** #98 met hors scope
/// « réparer autre chose que le Kraken » mais demande un mécanisme qui ne lui soit
/// pas propre : une clef par source rend l'indépendance structurelle plutôt que
/// disciplinaire. Un compteur partagé ferait abandonner une source qui vient de
/// s'effondrer parce qu'une autre a épuisé ses essais la veille.
#[derive(Debug, Clone, Default)]
pub struct Reparations {
    episodes: HashMap<String, Episode>,
}

impl Reparations {
    /// Aucune source n'a encore rien montré.
    pub fn nouvelles() -> Reparations {
        Reparations::default()
    }

    /// Un tour de décision, pour **une** source.
    ///
    /// `reinitialiser` n'est appelée que si une tentative est due — au plus une
    /// fois par tour, jamais autrement. C'est une propriété de ce type et non une
    /// politesse de son appelant : un `faut_il_reparer() -> bool` laisserait « ne
    /// pas réinitialiser » à la discipline du code d'en face, et le geste est trop
    /// visible sur la machine pour se confier à une discipline.
    pub fn tour(
        &mut self,
        etat: &EtatSource,
        maintenant: Duration,
        reinitialiser: impl FnOnce() -> io::Result<()>,
    ) -> Constat {
        if !effondree(etat) {
            // La source répond, ne serait-ce que par une cible : l'épisode est
            // clos, et son prochain effondrement sera un incident neuf, qui
            // mérite ses tentatives et sa ligne de journal. Un contrôleur qui
            // clignote est justement celui dont on veut entendre parler (#68).
            self.episodes.remove(&etat.source);
            return Constat::Rien;
        }

        let episode = self.episodes.entry(etat.source.clone()).or_default();
        if episode.abandonne {
            return Constat::Repos;
        }

        // La première tentative a lieu au tour même de l'effondrement : le délai
        // sépare deux gestes, il n'est pas un droit d'entrée. Quand toutes les
        // cibles se taisent, la panne a déjà mis près d'une minute à se déclarer.
        let due = episode
            .derniere
            .is_none_or(|derniere| maintenant.saturating_sub(derniere) >= DELAI_ENTRE_TENTATIVES);
        if !due {
            return Constat::Patiente;
        }

        // ⚠️ **L'abandon se prononce quand une tentative *serait* due**, et non au
        // moment de la dernière tentative. C'est ce qui donne au verdict un sens
        // uniforme — « j'ai essayé N fois, il s'est écoulé N délais, la source se
        // tait toujours » — et ce qui couvre le cas bien réel où le dernier
        // `ioctl` rend `Ok` sans que le périphérique revienne.
        if episode.tentatives >= TENTATIVES_MAXIMALES {
            episode.abandonne = true;
            return Constat::Abandon;
        }

        episode.tentatives += 1;
        episode.derniere = Some(maintenant);
        let tentative = episode.tentatives;

        match reinitialiser() {
            Ok(()) => Constat::Reussie { tentative },
            // La raison rendue par l'`ioctl` traverse la couture : c'est le seul
            // diagnostic que l'opérateur reçoive sur un geste qu'il n'a pas vu.
            Err(erreur) => Constat::Echouee {
                tentative,
                erreur: erreur.to_string(),
            },
        }
    }
}

/// Toutes les cibles connues de cette source se taisent-elles ?
///
/// ⚠️ **Toutes, et il en faut au moins une.** Une source qui répond encore par une
/// seule cible ne se réinitialise pas : ce serait casser ce qui marche pour
/// réparer ce qui n'est peut-être pas cassé. Et une source sans aucune cible
/// n'a montré aucun symptôme, quoi qu'en dise la logique du « pour tout ».
fn effondree(etat: &EtatSource) -> bool {
    !etat.cibles.is_empty() && etat.cibles.iter().all(|cible| etat.muettes.contains(cible))
}
