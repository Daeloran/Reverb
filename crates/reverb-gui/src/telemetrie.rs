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
use reverb_proto::ipc::{
    FanAction, MODE_COURBE_DE_L_HOTE, MODE_INCONNU_PREFIXE, MODE_MANUEL, MODE_NON_PILOTE,
    MODE_NON_REGLABLE, MODE_PLEIN_REGIME, Request, ResponseLine,
};

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

    /// Ce canal régule-t-il seul ? (#112)
    ///
    /// Vrai en `non-piloté` et en `courbe-de-l'hôte` — les deux modes où
    /// **quelque chose d'autre que l'hôte** décide de la vitesse. Une consigne
    /// écrite là ne s'ajoute pas à la régulation, elle la remplace, et le canal
    /// n'y revient pas : `docs/VENTILATEURS.md` établit qu'aucune valeur de
    /// `pwmN_enable` ne rend le Kraken à son profil d'usine, seule une coupure
    /// d'alimentation complète le fait.
    ///
    /// ⚠️ **Le déclencheur est le mode, jamais le nom du contrôleur.** Verrouiller
    /// sur « kraken » donnerait aujourd'hui le bon résultat — seuls ses deux
    /// canaux lisent `non-piloté` — et mentirait sur un canal du Kraken déjà
    /// passé en manuel, où il n'y a plus rien à protéger.
    ///
    /// ⚠️ **Ni le drapeau `sait_faire_auto`.** Depuis #97 il vaut « le pilote sait
    /// faire auto **et** une courbe a été posée », donc toujours faux : il ne dit
    /// plus rien du matériel.
    pub fn regule_seul(&self) -> bool {
        self.mode == MODE_NON_PILOTE || self.mode == MODE_COURBE_DE_L_HOTE
    }

    /// La requête qu'une consigne produit, verrou compris (#112).
    ///
    /// `None` si le canal est illisible (#100) **ou** s'il régule seul et que le
    /// verrou n'a pas été ouvert. Les deux règles se composent : ouvrir le
    /// cadenas d'un canal en quarantaine ne rend pas sa barre manipulable, sans
    /// quoi on tirerait une poignée vers un périphérique dont on sait qu'il ne
    /// répond plus.
    ///
    /// ⚠️ **Une seconde méthode plutôt qu'un garde dans [`LigneCanal::commande`]** :
    /// celle-ci est figée par #100 — « les commandes d'un canal illisible
    /// n'émettent rien », et rien d'autre. Et le verrou n'est pas une propriété
    /// de la ligne : c'est une propriété de la ligne **et** d'un geste que
    /// l'utilisateur vient de faire ou non, d'où le booléen en argument. Rangé
    /// dans [`LigneCanal`], il se refermerait tout seul au tour de `status`
    /// suivant, une seconde plus tard, au milieu du geste.
    ///
    /// ⚠️ **La consigne part telle quelle** une fois le verrou ouvert. Le borner
    /// ou le remplacer au passage rendrait le déverrouillage plus dangereux que
    /// son absence.
    pub fn commande_verrouillable(&self, action: FanAction, deverrouille: bool) -> Option<Request> {
        if self.regule_seul() && !deverrouille {
            return None;
        }
        self.commande(action)
    }
}

/// Le libellé sous lequel la table d'aide nomme la **famille** `inconnu-<n>`.
///
/// ⚠️ **Ce n'est pas un jeton de fil** : le démon n'écrit jamais « N », il écrit
/// le nombre lu dans `pwmN_enable`. C'est un nom d'affichage, et il vit donc ici
/// et non à côté des six jetons de `reverb-proto`. Une table qui listerait
/// `inconnu-0`, `inconnu-1`, … dirait 256 fois la même chose ; une table qui
/// n'exposerait rien laisserait sans aide le seul mode qu'on ne peut pas
/// deviner.
const INCONNU_GENERIQUE: &str = "inconnu-N";

/// Les six modes de la colonne MODE, et ce que chacun veut dire.
///
/// Chaque texte dit **qui décide de la vitesse** et **ce qu'on perd en y
/// touchant**. C'est l'information qui manquait le 2026-08-15 : rien, nulle
/// part, ne disait qu'un canal en `non-piloté` exécutait une régulation d'usine,
/// ni qu'un geste la remplacerait sans retour. La matière vient de
/// `docs/VENTILATEURS.md` et de la documentation de `reverb_hw::hwmon::Mode`.
const AIDES: [(&str, &str); 6] = [
    (
        MODE_NON_PILOTE,
        "Le pilote n'a jamais touché ce canal : c'est le périphérique qui décide, \
         sur son propre profil. Lui écrire une consigne l'en sort, et aucune commande \
         ne l'y rend — seule une coupure d'alimentation complète.",
    ),
    (
        MODE_MANUEL,
        "C'est l'hôte qui décide : la consigne écrite est appliquée telle quelle, \
         et elle ne bouge plus. Aucune régulation n'est en cours, donc rien ne se \
         perd à la changer.",
    ),
    (
        MODE_COURBE_DE_L_HOTE,
        "Le firmware suit la courbe que l'hôte lui a posée : elle décide de la \
         vitesse d'après la température. Une consigne fixe la remplace, et c'est \
         la seule courbe que vous ayez posée vous-même.",
    ),
    (
        MODE_PLEIN_REGIME,
        "Personne ne régule : le pilote a lâché la barre et le canal tourne à fond. \
         C'est ce qu'une écriture de zéro provoque, jamais ce qu'une lecture établit \
         — rien à perdre, et tout à gagner à en sortir.",
    ),
    (
        INCONNU_GENERIQUE,
        "Le fichier de mode porte une valeur que l'hôte ne sait pas interpréter, et \
         le nombre affiché est celle qui a été lue. On ne sait donc ni qui décide de \
         la vitesse, ni ce qu'une consigne remplacerait.",
    ),
    (
        MODE_NON_REGLABLE,
        "La source n'expose aucun fichier de mode — le cas des canaux de la carte \
         mère. La vitesse se lit, elle ne s'écrit pas : aucun mode ne s'y pose, \
         donc aucun ne s'y perd.",
    ),
];

/// Le texte d'aide d'un libellé de mode — `None` s'il n'est aucun des six.
///
/// La clé est le libellé **verbatim** de la ligne `chan`, celui que la colonne
/// affiche : le point d'interrogation est à côté du mode, et c'est donc ce
/// texte-là qui sert de clé.
///
/// ⚠️ **Reconnu exactement, jamais approximativement.** Une recherche tolérante
/// accepterait `non-pilote` sans accent ou `manual` en anglais, et masquerait
/// donc le jour où le libellé du démon dérive — au moment précis où il faudrait
/// le voir, puisque la fenêtre décide dessus (#112).
///
/// Seule exception, et elle est dans le protocole : la famille `inconnu-<n>`
/// porte un nombre, et se reconnaît donc à son préfixe. Les 256 valeurs
/// partagent une seule aide — le nombre est justement ce que personne ne sait
/// interpréter, et 256 textes seraient 256 façons d'écrire « on ne sait pas ».
pub fn aide_du_mode(mode: &str) -> Option<&'static str> {
    if mode.starts_with(MODE_INCONNU_PREFIXE) {
        return aide_generique_des_inconnus();
    }
    AIDES
        .iter()
        .find(|(libelle, _)| *libelle == mode)
        .map(|(_, texte)| *texte)
}

/// Les six modes et leur aide, de quoi garnir le panneau que le `?` ouvre.
///
/// La famille `inconnu-<n>` y figure sous son nom générique, les cinq autres
/// tels que la colonne les écrit. C'est la **même** donnée que celle
/// qu'[`aide_du_mode`] retrouve : deux sources séparées divergeraient au premier
/// texte corrigé d'un seul côté, et c'est le genre d'écart qu'on ne voit jamais
/// parce qu'on ne regarde jamais les deux en même temps.
pub fn aide_des_modes() -> Vec<(&'static str, &'static str)> {
    AIDES.to_vec()
}

/// L'aide de la famille `inconnu-<n>`, prise dans la table plutôt que recopiée.
fn aide_generique_des_inconnus() -> Option<&'static str> {
    AIDES
        .iter()
        .find(|(libelle, _)| *libelle == INCONNU_GENERIQUE)
        .map(|(_, texte)| *texte)
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

#[cfg(test)]
mod tests {
    //! Tests de logique de l'aide sur les modes (#112).
    //!
    //! Ce que l'aide **dit** est figé par les tests d'intention
    //! (`tests/spec_verrou_canal.rs`). Ce qui reste ici est ce que ceux-là ne
    //! peuvent pas voir : que le seul libellé écrit à la main dans ce fichier
    //! appartient bien à la famille que le démon écrit.

    use super::*;

    #[test]
    fn le_libelle_generique_appartient_a_la_famille_des_inconnus() {
        // C'est le seul endroit de `reverb-gui` où une graphie de mode est
        // écrite plutôt que lue dans `reverb-proto` — le `N` est un nom
        // d'affichage, le démon écrit un nombre. Si le préfixe du protocole
        // changeait, la table nommerait une famille qui n'existe plus et le
        // point d'interrogation d'un `inconnu-7` ouvrirait sur du vide.
        assert!(
            INCONNU_GENERIQUE.starts_with(MODE_INCONNU_PREFIXE),
            "« {INCONNU_GENERIQUE} » doit commencer par « {MODE_INCONNU_PREFIXE} »"
        );
        assert_eq!(
            aide_du_mode(INCONNU_GENERIQUE),
            aide_du_mode(&format!("{MODE_INCONNU_PREFIXE}7")),
            "le nom générique et un libellé réel doivent ouvrir la même aide"
        );
    }
}
