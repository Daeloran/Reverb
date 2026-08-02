//! Ce qui empêche une sonde muette d'emporter le démon avec elle (issue #68).
//!
//! # Le défaut
//!
//! Le 2026-08-02, sur SHYNAEL : plus rien ne répondait depuis la fenêtre. Le
//! démon tournait, animait le boîtier, et **ne servait plus aucun ordre** — pas
//! même ceux qui ne touchent aucun matériel.
//!
//! ```text
//! zone list   → aucune réponse en 12 s
//! geometry    → aucune réponse en 12 s
//! ```
//!
//! Le fil principal était en état `D` — sommeil non interruptible, donc bloqué
//! dans le noyau — à **0 % de processeur**. Il n'attendait pas un verrou, il
//! attendait le matériel. La cause est **hors de Reverb**, et se mesure sans
//! lui :
//!
//! ```text
//! $ time cat /sys/class/hwmon/hwmon5/temp1_input      # kraken2023elite
//! cat: … : Connexion terminée par expiration du délai d'attente
//!         5,218 total
//! $ time cat /sys/class/hwmon/hwmon6/fan1_input       # nzxtsmart2
//! 716
//!         0,001 total
//! ```
//!
//! Le Kraken avait cessé de répondre à son pilote noyau, et **une simple lecture
//! sysfs y bloque cinq secondes**. Le démon relève ses sondes chaque seconde,
//! dans le fil qui sert aussi le socket et tient les bus (ADR-002) : une sonde
//! muette suffisait donc à geler le service entier.
//!
//! ⚠️ **Le défaut n'était pas que la sonde échoue, c'était qu'elle emporte tout
//! le reste avec elle.** Un ventilateur, une zone, une couleur n'ont rien à voir
//! avec la température du liquide, et ne devraient pas attendre après elle.
//!
//! # Pourquoi le délai croît
//!
//! Chaque retente **coûte les cinq secondes** — c'est le noyau qui expire, et on
//! ne peut pas l'interrompre. Une retente par minute rendrait le socket muet 8 %
//! du temps. D'où un délai qui double, plafonné : le coût tombe sous 2 % en
//! quelques minutes, et à rien du tout ensuite.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne lit rien, n'ouvre rien, ne tient aucune horloge : **l'instant est un
//! paramètre**. C'est ce qui le rend vérifiable sans attendre et reproductible à
//! l'identique — le même parti que `Poignee` et `Limiteur` dans la fenêtre.

use std::collections::HashMap;
use std::time::Duration;

/// Le délai avant la première retente d'une sonde qui vient de se taire.
///
/// Trente secondes : six fois le blocage mesuré, assez pour qu'un contrôleur qui
/// bafouille s'en remette, assez peu pour qu'une sonde revenue ne se fasse pas
/// attendre une éternité.
pub const DELAI_INITIAL: Duration = Duration::from_secs(30);

/// Le plafond du délai de retente.
///
/// Cinq minutes : au-delà, une sonde rebranchée resterait muette si longtemps
/// qu'on la croirait morte. En deçà, une sonde définitivement partie coûterait
/// ses cinq secondes de gel trop souvent.
pub const DELAI_MAXIMAL: Duration = Duration::from_secs(300);

/// Ce qu'un tour de relevé a donné pour une sonde.
///
/// ⚠️ **Deux variantes, et pas trois.** Il n'y a pas de « rien à dire » : une
/// sonde muette est **toujours** rapportée comme telle, jamais omise. Une sonde
/// absente et une sonde muette sont deux choses différentes, et les confondre
/// ferait disparaître du relevé une sonde qui existe — ce qui se lit comme un
/// matériel débranché.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Releve<T> {
    /// La sonde a répondu.
    Valeur(T),
    /// La sonde s'est tue, ou dort sa quarantaine.
    ///
    /// `signaler` n'est vrai qu'à l'**entrée** en quarantaine : c'est ce qui
    /// permet d'écrire une ligne de journal et une seule. Sans lui, l'appelant
    /// n'aurait d'autre choix que de journaliser à chaque tour — une ligne par
    /// seconde, pour toujours, dans un journal qu'on lit justement pour trouver
    /// ce genre d'incident.
    Muette { signaler: bool },
}

/// L'état d'une sonde qui s'est tue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Ecartee {
    /// Combien de fois elle a échoué d'affilée. Décide du délai.
    echecs: u32,
    /// Quand la retenter. Avant cet instant, elle n'est pas lue du tout.
    echeance: Duration,
    /// A-t-elle déjà été signalée ? Une seule fois par épisode.
    signalee: bool,
}

/// Les sondes écartées, et depuis quand.
///
/// ⚠️ **Un état par sonde, jamais par contrôleur ni global.** Deux sondes du même
/// `hwmon` sont deux sondes : celle qui répond doit continuer d'être lue quand sa
/// voisine se tait.
#[derive(Debug, Clone, Default)]
pub struct Quarantaine {
    ecartees: HashMap<String, Ecartee>,
}

impl Quarantaine {
    /// Une quarantaine vide : toutes les sondes sont lues.
    pub fn nouvelle() -> Quarantaine {
        Quarantaine::default()
    }

    /// Un tour de relevé pour cette sonde.
    ///
    /// ⚠️ **`relever` n'est appelée que si la sonde n'est pas en quarantaine.**
    /// C'est une propriété de ce type et non une politesse de son appelant : une
    /// télémétrie qui lirait le fichier *puis* consulterait la décision rendrait
    /// le bon verdict et gèlerait le démon exactement comme avant. C'est ici que
    /// la lecture a lieu, ou n'a pas lieu.
    pub fn tour<T>(
        &mut self,
        slug: &str,
        maintenant: Duration,
        relever: impl FnOnce() -> Option<T>,
    ) -> Releve<T> {
        if let Some(ecartee) = self.ecartees.get(slug)
            && maintenant < ecartee.echeance
        {
            return Releve::Muette { signaler: false };
        }

        match relever() {
            Some(valeur) => {
                // Une retente réussie efface tout : le compte, l'échéance, et le
                // fait d'avoir été signalée. Une sonde qui retomberait en panne
                // plus tard est un nouvel épisode, et se journalise à nouveau.
                self.ecartees.remove(slug);
                Releve::Valeur(valeur)
            }
            None => {
                let ecartee = self.ecartees.entry(slug.to_owned()).or_default();
                // Le délai vient du nombre d'échecs **déjà** essuyés : le premier
                // échec vaut `DELAI_INITIAL`, le deuxième le double, et ainsi de
                // suite jusqu'au plafond.
                let delai = DELAI_INITIAL
                    .checked_mul(2u32.saturating_pow(ecartee.echecs.min(31)))
                    .unwrap_or(DELAI_MAXIMAL)
                    .min(DELAI_MAXIMAL);
                ecartee.echecs = ecartee.echecs.saturating_add(1);
                ecartee.echeance = maintenant.saturating_add(delai);
                let signaler = !ecartee.signalee;
                ecartee.signalee = true;
                Releve::Muette { signaler }
            }
        }
    }
}
