//! Lecture des vitesses et des températures, en lignes de réponse.
//!
//! ⚠️ **Une valeur illisible est rendue illisible**, jamais omise ni mise à
//! zéro. Un canal affiché à 0 tr/min est un mensonge ; un canal omis fait
//! croire qu'il n'existe pas. C'est [`ResponseLine::Unreadable`] qui porte la
//! différence, et c'est la seule forme honnête quand un fichier sysfs
//! disparaît sous nos pieds — ce qui arrive dès qu'on débranche.

use std::fs;
use std::path::Path;

use reverb_hw::hwmon::{FanChannel, Percent};
use reverb_proto::ipc::ResponseLine;

/// Relève l'état de tous les canaux.
pub fn releve(canaux: &[FanChannel]) -> Vec<ResponseLine> {
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
        });
    }

    for (nom, chemin) in capteurs(canaux) {
        match entier::<i32>(&chemin) {
            Some(millidegres) => lignes.push(ResponseLine::Temp {
                sensor: nom,
                millidegrees: millidegres,
            }),
            None => lignes.push(ResponseLine::Unreadable {
                subject: nom,
                reason: format!("{} illisible", chemin.display()),
            }),
        }
    }

    lignes
}

/// Les capteurs de température, voisins des canaux dans le même `hwmon`.
///
/// Le nom porte la source, comme les canaux : deux pilotes nomment volontiers
/// leur capteur `temp1`, et un nom ambigu vaut moins que pas de nom du tout.
fn capteurs(canaux: &[FanChannel]) -> Vec<(String, std::path::PathBuf)> {
    let mut trouves: Vec<(String, std::path::PathBuf)> = Vec::new();

    for canal in canaux {
        let Some(dossier) = canal.pwm.parent() else {
            continue;
        };
        let Ok(entrees) = fs::read_dir(dossier) else {
            continue;
        };
        for entree in entrees.flatten() {
            let fichier = entree.file_name();
            let Some(fichier) = fichier.to_str() else {
                continue;
            };
            let Some(numero) = fichier
                .strip_prefix("temp")
                .and_then(|reste| reste.strip_suffix("_input"))
            else {
                continue;
            };

            let libelle = lire(&dossier.join(format!("temp{numero}_label")))
                .map(|brut| reverb_hw::hwmon::slug(brut.trim()))
                .unwrap_or_else(|| format!("temp{numero}"));
            let nom = format!("{}:{libelle}", canal.source);

            if !trouves.iter().any(|(deja, _)| deja == &nom) {
                trouves.push((nom, entree.path()));
            }
        }
    }

    trouves.sort_by(|a, b| a.0.cmp(&b.0));
    trouves
}

fn lire(chemin: &Path) -> Option<String> {
    fs::read_to_string(chemin).ok()
}

fn entier<T: std::str::FromStr>(chemin: &Path) -> Option<T> {
    lire(chemin)?.trim().parse().ok()
}
