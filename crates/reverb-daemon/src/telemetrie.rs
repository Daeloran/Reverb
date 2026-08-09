//! Lecture des vitesses et des températures, en lignes de réponse.
//!
//! ⚠️ **Une valeur illisible est rendue illisible**, jamais omise ni mise à
//! zéro. Un canal affiché à 0 tr/min est un mensonge ; un canal omis fait
//! croire qu'il n'existe pas. C'est [`ResponseLine::Unreadable`] qui porte la
//! différence, et c'est la seule forme honnête quand un fichier sysfs
//! disparaît sous nos pieds — ce qui arrive dès qu'on débranche.

use std::fs;
use std::path::Path;
use std::time::Duration;

use reverb_hw::hwmon::{FanChannel, Percent, Sonde};
use reverb_proto::Position;
use reverb_proto::ipc::ResponseLine;

use crate::quarantaine::{Quarantaine, Releve, ReleveMotive};

/// Ce qu'une lecture réussie d'un canal a rapporté.
///
/// Sépare **ce qu'on lit** de **ce qu'on en fait**. C'est cette couture qui rend
/// #88 vérifiable sans toucher à sysfs : un test injecte un canal qui répond et
/// un canal qui se tait, et compte les lectures qui n'ont pas lieu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LectureCanal {
    pub position: Option<Position>,
    pub rpm: Option<u32>,
    pub pwm: Option<u8>,
    pub mode: String,
    pub sait_faire_auto: bool,
}

/// Ce qu'un tour de relevé des canaux a produit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TourCanaux {
    /// **Une ligne par canal reçu, dans le même ordre** : `Channel` s'il a
    /// répondu, `Unreadable` sinon. Jamais moins — un canal omis fait croire
    /// qu'il n'existe pas —, jamais plus.
    pub lignes: Vec<ResponseLine>,
    /// Les canaux qui **viennent** d'être mis à l'écart — une fois par
    /// épisode, jamais à chaque tour.
    ///
    /// Des noms, et non des phrases toutes faites : c'est l'appelant qui
    /// journalise, et lui seul sait dans quel contexte. Un tour qui rendrait des
    /// lignes de journal ne serait plus comparable dans un test qu'à la virgule
    /// près.
    pub a_signaler: Vec<String>,
}

/// Relève l'état des canaux, en écartant ceux qui ne répondent plus (issue #88).
///
/// # Le défaut que ceci corrige
///
/// Mesuré sur SHYNAEL le 2026-08-09, Kraken en rade :
///
/// ```text
/// $ status
/// 36.306 s · 811 octets
/// 30.708 s · 811 octets        ← reproductible, pas un hoquet
/// unreadable kraken2023elite:fan-speed:mode  Connection timed out (os error 110)
/// ```
///
/// Les **sondes** passaient par la quarantaine depuis #68, les **canaux** non.
/// Chaque attribut sysfs d'un canal muet coûte ses cinq secondes en sommeil non
/// interruptible, et la fenêtre demande `status` une fois par seconde — sur le
/// fil qui sert le socket. Mesuré : `geometry`, qui ne touche aucun matériel, a
/// mis **10,2 s** à répondre pour la seule raison qu'un `status` le précédait.
///
/// ⚠️ **La clef d'écartement est le canal entier, pas l'attribut.** Quand un
/// contrôleur ne répond plus, aucun de ses attributs ne répond : écarter attribut
/// par attribut ferait payer trois fois cinq secondes pour l'apprendre.
pub fn releve_canaux(
    canaux: &[FanChannel],
    quarantaine: &mut Quarantaine,
    maintenant: Duration,
    mut lire: impl FnMut(&FanChannel) -> Result<LectureCanal, String>,
) -> TourCanaux {
    let mut lignes = Vec::new();
    let mut a_signaler = Vec::new();

    for canal in canaux {
        match quarantaine.tour_motive(&canal.name, maintenant, || lire(canal)) {
            ReleveMotive::Valeur(lu) => lignes.push(ResponseLine::Channel {
                channel: canal.name.clone(),
                position: lu.position,
                rpm: lu.rpm,
                pwm: lu.pwm,
                mode: lu.mode,
                sait_faire_auto: lu.sait_faire_auto,
            }),
            ReleveMotive::Muette { signaler, raison } => {
                if signaler {
                    a_signaler.push(canal.name.clone());
                }
                // ⚠️ **Le sujet est le nom du canal, exactement.** Nommer
                // `…:mode` laisserait croire que `rpm` et `pwm` ont été lus et
                // vont bien : c'est faux, et ça rend indécidable la
                // correspondance d'un canal à sa ligne.
                lignes.push(ResponseLine::Unreadable {
                    subject: canal.name.clone(),
                    reason: raison,
                });
            }
        }
    }

    TourCanaux { lignes, a_signaler }
}

/// La vraie lecture d'un canal, sur sysfs.
///
/// ⚠️ **Le mode d'abord, et c'est ce qui tient la promesse d'une seule lecture.**
/// C'est l'attribut qui rend `ETIMEDOUT` sur un contrôleur muet ; échouer dessus
/// avant de toucher au régime et au PWM fait qu'apprendre le silence coûte cinq
/// secondes une fois, et non trois.
fn lire_le_canal(canal: &FanChannel) -> Result<LectureCanal, String> {
    let mode = canal.mode().map_err(|erreur| erreur.to_string())?;

    Ok(LectureCanal {
        // ⚠️ Toujours absente, et ce n'est pas un oubli. Quels ventilateurs
        // physiques sont repiqués sur quel canal PWM est une **inconnue
        // documentée** (`docs/VENTILATEURS.md`) : deux tentatives de mesure ont
        // échoué, à l'oreille et par l'intensité, le firmware remontant `0 mA`.
        // Un canal alimente plusieurs ventilateurs et un seul remonte son régime.
        //
        // Le champ existe dans le protocole pour le jour où la répartition sera
        // établie. Le remplir au jugé ferait afficher à la fenêtre une
        // correspondance que personne n'a mesurée.
        position: None,
        rpm: canal.tach.as_ref().and_then(|c| entier::<u32>(c)),
        pwm: lire(&canal.pwm)
            .and_then(|brut| brut.trim().parse::<u8>().ok())
            .map(|brut| Percent::from_raw(brut).percent()),
        mode: mode.to_string(),
        // Lu sur le nom du pilote, jamais sur une tentative d'écriture : la
        // tentative qui réussit là où il ne fallait pas envoie le canal à plein
        // régime, en silence (issue #50).
        sait_faire_auto: canal.sait_faire_auto(),
    })
}

/// Relève l'état de tous les canaux et de toutes les sondes.
///
/// ⚠️ **Toutes les sondes de la machine**, et non seulement celles voisines d'un
/// canal de ventilateur. La fenêtre ne montrait jusqu'ici que le coolant du
/// Kraken, alors que SHYNAEL en expose quinze : le CPU, cinq NVMe, les quatre
/// barrettes de RAM, le GPU intégré, le wifi et la carte réseau.
pub fn releve(
    canaux: &[FanChannel],
    sondes: &[Sonde],
    gpu: Option<(String, i32)>,
    quarantaine: &mut Quarantaine,
    maintenant: Duration,
) -> Vec<ResponseLine> {
    let mut lignes = Vec::new();

    let tour = releve_canaux(canaux, quarantaine, maintenant, lire_le_canal);
    // La raison se relit sur la ligne qui vient d'être produite : c'est la même,
    // et la dupliquer dans `a_signaler` ferait deux endroits à tenir d'accord.
    for ecarte in &tour.a_signaler {
        let raison = tour
            .lignes
            .iter()
            .find_map(|ligne| match ligne {
                ResponseLine::Unreadable { subject, reason } if subject == ecarte => Some(reason),
                _ => None,
            })
            .map_or("relevé sans réponse", String::as_str);
        eprintln!(
            "attention : canal « {ecarte} » écarté : {raison} — retenté plus tard, \
             de plus en plus espacé"
        );
    }
    lignes.extend(tour.lignes);

    for sonde in sondes {
        // ⚠️ **La lecture passe par la quarantaine, elle ne la précède pas**
        // (#68). Une sonde qui ne répond plus bloque cinq secondes dans le noyau,
        // en sommeil non interruptible : lire d'abord et consulter ensuite
        // rendrait le bon verdict et gèlerait le démon exactement comme avant.
        let mut souci = None;
        let verdict = quarantaine.tour(&sonde.slug, maintenant, || match sonde.lire() {
            Ok(millidegres) => Some(millidegres),
            Err(erreur) => {
                souci = Some(erreur.to_string());
                None
            }
        });
        match verdict {
            Releve::Valeur(millidegres) => lignes.push(ResponseLine::Temp {
                sensor: sonde.slug.clone(),
                millidegrees: millidegres,
            }),
            // Le dire, plutôt que d'omettre la sonde et de laisser croire qu'elle
            // n'a jamais existé. Une sonde absente et une sonde muette sont deux
            // choses différentes.
            Releve::Muette { signaler } => {
                let raison = souci.unwrap_or_else(|| "écartée après un relevé sans réponse".into());
                if signaler {
                    eprintln!(
                        "attention : sonde « {} » écartée : {raison} — retentée plus tard, \
                         de plus en plus espacé",
                        sonde.slug
                    );
                }
                lignes.push(ResponseLine::Unreadable {
                    subject: sonde.slug.clone(),
                    reason: raison,
                });
            }
        }
    }

    // Le GPU discret arrive déjà lu : le pilote propriétaire n'enregistre aucun
    // `hwmon`, et `nvidia-smi` coûte 16 ms — un tiers d'image de rendu, donc
    // jamais dans cette boucle.
    if let Some((nom, millidegres)) = gpu {
        lignes.push(ResponseLine::Temp {
            sensor: format!("nvidia:{nom}"),
            millidegrees: millidegres,
        });
    }

    lignes
}

fn lire(chemin: &Path) -> Option<String> {
    fs::read_to_string(chemin).ok()
}

fn entier<T: std::str::FromStr>(chemin: &Path) -> Option<T> {
    lire(chemin)?.trim().parse().ok()
}
