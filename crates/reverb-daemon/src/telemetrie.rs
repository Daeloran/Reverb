//! Lecture des vitesses et des températures, en lignes de réponse.
//!
//! ⚠️ **Une valeur illisible est rendue illisible**, jamais omise ni mise à
//! zéro. Un canal affiché à 0 tr/min est un mensonge ; un canal omis fait
//! croire qu'il n'existe pas. C'est [`ResponseLine::Unreadable`] qui porte la
//! différence, et c'est la seule forme honnête quand un fichier sysfs
//! disparaît sous nos pieds — ce qui arrive dès qu'on débranche.

use std::fs;
use std::path::Path;

use reverb_hw::hwmon::{FanChannel, Percent, Sonde};
use reverb_proto::ipc::ResponseLine;

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
) -> Vec<ResponseLine> {
    let mut lignes = Vec::new();

    for canal in canaux {
        let rpm = canal.tach.as_ref().and_then(|c| entier::<u32>(c));
        let pwm = lire(&canal.pwm)
            .and_then(|brut| brut.trim().parse::<u8>().ok())
            .map(|brut| Percent::from_raw(brut).percent());

        let mode = match canal.mode() {
            Ok(mode) => mode.to_string(),
            Err(erreur) => {
                lignes.push(ResponseLine::Unreadable {
                    subject: format!("{}:mode", canal.name),
                    reason: erreur.to_string(),
                });
                continue;
            }
        };

        lignes.push(ResponseLine::Channel {
            channel: canal.name.clone(),
            // ⚠️ Toujours absente, et ce n'est pas un oubli. Quels ventilateurs
            // physiques sont repiqués sur quel canal PWM est une **inconnue
            // documentée** (`docs/VENTILATEURS.md`) : deux tentatives de mesure
            // ont échoué, à l'oreille et par l'intensité, le firmware remontant
            // `0 mA`. Un canal alimente plusieurs ventilateurs et un seul
            // remonte son régime.
            //
            // Le champ existe dans le protocole pour le jour où la répartition
            // sera établie. Le remplir au jugé ferait afficher à la fenêtre une
            // correspondance que personne n'a mesurée.
            position: None,
            rpm,
            pwm,
            mode,
            // Lu sur le nom du pilote, jamais sur une tentative d'écriture : la
            // tentative qui réussit là où il ne fallait pas envoie le canal à
            // plein régime, en silence (issue #50).
            sait_faire_auto: canal.sait_faire_auto(),
        });
    }

    for sonde in sondes {
        match sonde.lire() {
            Ok(millidegres) => lignes.push(ResponseLine::Temp {
                sensor: sonde.slug.clone(),
                millidegrees: millidegres,
            }),
            // Le fichier a disparu — un périphérique débranché. Le dire, plutôt
            // que d'omettre la sonde et de laisser croire qu'elle n'a jamais
            // existé.
            Err(erreur) => lignes.push(ResponseLine::Unreadable {
                subject: sonde.slug.clone(),
                reason: erreur.to_string(),
            }),
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
