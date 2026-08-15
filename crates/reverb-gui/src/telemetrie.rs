//! Le tri d'un tour de télémétrie : les canaux d'un côté, les sondes de l'autre.
//!
//! Il vit **dans la bibliothèque** et non dans le binaire, pour la raison qui
//! vaut partout ailleurs dans le projet : ce qui se vérifie sans matériel est
//! séparé de ce qui touche au matériel (CLAUDE.md). Ranger une réponse du démon
//! est du calcul, la dessiner est de l'E/S — et un binaire ne se teste pas
//! depuis `tests/`. `main.rs` recopie ensuite chaque [`LigneCanal`] dans le
//! `LigneVentilateur` du `.slint`, champ pour champ.
//!
//! # Le défaut que ce module corrige (issue #100)
//!
//! Un canal qui cesse de répondre à son pilote part en quarantaine, et le démon
//! le **dit** : une ligne `unreadable` à la place de sa ligne `chan`, à sa place
//! dans le tour (#88). La fenêtre, elle, la perdait. Elle ne fabriquait une
//! ligne de ventilateur que depuis `chan`, et l'`unreadable` tombait dans la
//! branche des sondes — où le sujet n'est pas une sonde retenue (#51) et ne
//! s'affiche donc nulle part. Les deux canaux du Kraken disparaissaient du
//! panneau au moment précis où l'on voulait les régler à la main.
//!
//! ⚠️ **Une ligne absente et une ligne grisée ne disent pas la même chose.** La
//! première laisse croire que le canal n'existe pas — donc qu'il n'y a rien à
//! régler, ni rien à réparer. La seconde dit qu'il existe et qu'on ne l'atteint
//! plus, ce qui est la vérité. C'est la règle que le projet applique déjà aux
//! sondes ; les canaux ne l'avaient pas reçue.
//!
//! # Comment un sujet muet est reconnu, et ce que ça laisse ouvert
//!
//! Le protocole ne dit pas la **nature** d'un sujet illisible : `unreadable`
//! porte un sujet et une raison, rien de plus. Le tri se souvient donc des
//! canaux qu'il a vus répondre — un canal se nomme lui-même en `chan`, et un
//! sujet déjà vu là est un canal pour toujours.
//!
//! ⚠️ **Le préfixe ne suffirait pas.** `kraken2023elite:` nomme aussi bien un
//! canal (`…:fan-speed`) qu'une sonde (`…:coolant-temp`) : trier sur le pilote
//! ferait de la température du liquide une ligne de ventilateur, avec une
//! poignée de régime sous une valeur en millidegrés. C'est le nom complet qui
//! décide, et rien d'autre.
//!
//! ⚠️ **Le tout premier tour reste sans réponse**, et c'est la limite connue de
//! ce choix : un canal muet depuis le démarrage du démon n'a jamais paru en
//! `chan`, et rien dans sa ligne ne le distingue d'une sonde muette. Il part
//! alors du côté des sondes, comme avant. Fenêtre ouverte pendant que le
//! contrôleur répondait, le cas ne se produit jamais ; fenêtre ouverte sur un
//! Kraken déjà en rade, il dure jusqu'au premier tour où le canal répond.
//!
//! L'issue nommait le remède — que le démon dise la nature dans le protocole —
//! et il ne tient pas dans une correction de la fenêtre : `ResponseLine` est
//! partagé par les trois binaires, et son variant `Unreadable` est déstructuré
//! champ pour champ par les tests d'intention de #88, qui ne se réécrivent pas
//! pour arranger un design venu après eux. **À reprendre** le jour où le
//! protocole gagne ce jeton : la mémoire d'ici devient alors un repli pour les
//! démons d'avant, et non la seule source.
//!
//! # Ce qu'une ligne illisible ne montre pas
//!
//! Aucune mesure : ni le régime, ni la consigne du tour d'avant. « Une sonde
//! muette écrit des tirets, jamais un zéro ni la dernière valeur connue »
//! (README, le cadran), et un 1200 tr/min figé derrière un ventilateur arrêté
//! est exactement le mode de défaillance rassurant que le projet traite partout
//! ailleurs de la même façon.
//!
//! La **position** et le drapeau « auto » sont gardés, eux : ce ne sont pas des
//! mesures, mais une donnée de montage et une capacité du pilote. Les oublier
//! ferait retomber la ligne sur son nom de hwmon au moment précis où l'on
//! cherche à la reconnaître, et ferait disparaître le bouton « auto » — donc
//! bouger la ligne sous les doigts à chaque hoquet du Kraken.

use std::collections::HashMap;

use reverb_proto::Position;
use reverb_proto::ipc::{FanAction, Request, ResponseLine};

use crate::sondes::Releve;

/// Une ligne de ventilateur, telle que la fenêtre la montre.
///
/// Les champs de mesure sont ceux de [`ResponseLine::Channel`], recopiés sans
/// conversion ; `lisible` dit si le démon a répondu pour ce canal à ce tour-ci.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigneCanal {
    pub canal: String,
    pub position: Option<Position>,
    pub rpm: Option<u32>,
    pub pwm: Option<u8>,
    pub mode: String,
    pub sait_faire_auto: bool,
    pub lisible: bool,
}

impl LigneCanal {
    /// La requête qu'une consigne produit pour ce canal — `None` s'il est
    /// illisible.
    ///
    /// ⚠️ **C'est le seul endroit qui décide** que la poignée d'un canal en
    /// quarantaine est inerte, `auto` compris — et c'est justement le bouton
    /// qu'on irait chercher quand la ligne ne montre plus rien. Consigner un tel
    /// canal n'a aucun effet visible, et coûte au démon une écriture sysfs vers
    /// le périphérique même dont on sait qu'il ne répond plus, dans le fil qui
    /// sert le socket (#88).
    pub fn commande(&self, action: FanAction) -> Option<Request> {
        self.lisible.then(|| Request::Fan {
            channel: self.canal.clone(),
            action,
        })
    }
}

/// Ce qu'un tour de `status` donne à montrer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Telemetrie {
    /// Une ligne par canal reçu, **dans l'ordre du tour** — lisible ou non.
    pub canaux: Vec<LigneCanal>,
    /// Les relevés à noter dans l'historique des sondes.
    pub sondes: Vec<(String, Releve)>,
}

/// Ce qu'on garde d'un canal entre deux tours.
///
/// Ni le régime ni la consigne : ce sont des mesures, et une mesure qui n'a pas
/// eu lieu ne se réaffiche pas. Voir l'en-tête du module.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Habillage {
    position: Option<Position>,
    sait_faire_auto: bool,
}

/// Le tri, qui se souvient des canaux déjà vus.
#[derive(Debug, Clone, Default)]
pub struct Tri {
    /// Les canaux qui ont répondu au moins une fois, avec leur habillage.
    ///
    /// La mémoire ne se vide jamais : un canal qui a répondu une fois est un
    /// canal, même s'il se tait ensuite pendant toute la session.
    connus: HashMap<String, Habillage>,
}

impl Tri {
    pub fn nouveau() -> Tri {
        Tri::default()
    }

    /// Range un tour de réponses : les canaux d'un côté, les sondes de l'autre.
    ///
    /// L'ordre des canaux est celui du tour, quarantaine ou non : le démon rend
    /// une ligne par canal à sa place (#88), et rattraper les `unreadable` en
    /// fin de liste ferait glisser les poignées sous les doigts à chaque hoquet.
    ///
    /// Les lignes qui ne sont ni un canal ni un relevé — l'éclairage, l'écran,
    /// la géométrie, `end` — ne vont dans aucune des deux listes.
    pub fn poser(&mut self, lignes: &[ResponseLine]) -> Telemetrie {
        let mut vue = Telemetrie::default();
        for ligne in lignes {
            match ligne {
                ResponseLine::Channel {
                    channel,
                    position,
                    rpm,
                    pwm,
                    mode,
                    sait_faire_auto,
                } => {
                    self.connus.insert(
                        channel.clone(),
                        Habillage {
                            position: *position,
                            sait_faire_auto: *sait_faire_auto,
                        },
                    );
                    vue.canaux.push(LigneCanal {
                        canal: channel.clone(),
                        position: *position,
                        rpm: *rpm,
                        pwm: *pwm,
                        mode: mode.clone(),
                        sait_faire_auto: *sait_faire_auto,
                        // ⚠️ **Un canal qui a répondu sans tachymètre est
                        // illisible lui aussi** — c'est le bloc-pompe rendu au
                        // firmware. Cette règle était déjà celle de la fenêtre,
                        // et #100 lui ajoute un cas au lieu de la remplacer.
                        lisible: rpm.is_some(),
                    });
                }
                ResponseLine::Temp {
                    sensor,
                    millidegrees,
                } => vue
                    .sondes
                    .push((sensor.clone(), Releve::Valeur(*millidegrees))),
                // La raison n'est pas relue ici : elle porte des espaces, et
                // c'est le protocole qui l'a déjà séparée du sujet. Le journal
                // du démon la dit une fois par mise à l'écart (#88).
                ResponseLine::Unreadable { subject, .. } => match self.connus.get(subject) {
                    Some(habillage) => vue.canaux.push(LigneCanal {
                        canal: subject.clone(),
                        position: habillage.position,
                        rpm: None,
                        pwm: None,
                        // Le mode est un relevé, pas une donnée de montage : il
                        // reste vide plutôt que de figer celui d'avant.
                        mode: String::new(),
                        sait_faire_auto: habillage.sait_faire_auto,
                        lisible: false,
                    }),
                    // Une sonde débranchée le dit, et sa courbe garde la trace
                    // du trou : figer sa dernière valeur ferait croire qu'on la
                    // lit encore.
                    None => vue.sondes.push((subject.clone(), Releve::Illisible)),
                },
                _ => {}
            }
        }
        vue
    }
}
