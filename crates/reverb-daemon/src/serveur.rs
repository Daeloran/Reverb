//! Le socket Unix et ses clients.
//!
//! Un fil par connexion, et **aucun accès au matériel depuis ces fils** : ils
//! transmettent des ordres au fil de rendu, qui détient seul les périphériques,
//! et attendent sa réponse. C'est ce qui rend impossible qu'un client déclenche
//! une écriture pendant qu'une image part — la collision SMBus contre laquelle
//! la spec Corsair met en garde.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::{Sender, channel};
use std::thread;

use reverb_proto::ipc::{self, MAX_LINE_LEN, Request, ResponseLine};

/// Ce qu'un client demande au fil de rendu.
pub struct Ordre {
    pub requete: Request,
    /// Par où renvoyer la réponse. Le fil de rendu y écrit une fois.
    pub reponse: Sender<Vec<ResponseLine>>,
}

/// Ouvre le socket et sert les clients jusqu'à l'arrêt du processus.
///
/// Le fichier de socket est retiré s'il traîne d'une exécution précédente :
/// `bind` échouerait sinon sur un fichier mort, et un démon qui refuse de
/// démarrer après un arrêt brutal est un démon qu'on finit par ne plus lancer.
pub fn servir(chemin: &Path, ordres: Sender<Ordre>) -> io::Result<()> {
    if chemin.exists() {
        let _ = std::fs::remove_file(chemin);
    }
    let ecouteur = UnixListener::bind(chemin)?;

    for flux in ecouteur.incoming() {
        let flux = match flux {
            Ok(flux) => flux,
            // Une connexion ratée ne doit pas emporter le service.
            Err(erreur) => {
                eprintln!("connexion refusée : {erreur}");
                continue;
            }
        };
        let ordres = ordres.clone();
        thread::spawn(move || {
            if let Err(erreur) = dialoguer(flux, &ordres) {
                eprintln!("client interrompu : {erreur}");
            }
        });
    }
    Ok(())
}

fn dialoguer(flux: UnixStream, ordres: &Sender<Ordre>) -> io::Result<()> {
    let mut sortie = flux.try_clone()?;
    let mut lecteur = BufReader::new(flux);

    loop {
        let mut ligne = String::new();
        // La lecture est bornée **avant** l'allocation : un client qui envoie
        // un mégaoctet sans `\n` ne doit pas faire enfler la mémoire du démon.
        let lus = lecteur
            .by_ref()
            .take((MAX_LINE_LEN + 1) as u64)
            .read_line(&mut ligne)?;
        if lus == 0 {
            return Ok(());
        }

        if !ligne.ends_with('\n') && lus > MAX_LINE_LEN {
            // On ne cherche pas le `\n` suivant pour se resynchroniser : ce
            // serait lire sans borne exactement ce qu'on vient de refuser.
            repondre(
                &mut sortie,
                &[ResponseLine::Error {
                    message: format!("ligne de plus de {MAX_LINE_LEN} octets — connexion fermée"),
                }],
            )?;
            return Ok(());
        }

        let lignes = match ipc::parse_request(ligne.trim_end()) {
            Ok(requete) => {
                let (repondeur, retour) = channel();
                if ordres
                    .send(Ordre {
                        requete,
                        reponse: repondeur,
                    })
                    .is_err()
                {
                    return Ok(());
                }
                match retour.recv() {
                    Ok(lignes) => lignes,
                    Err(_) => return Ok(()),
                }
            }
            Err(erreur) => vec![ResponseLine::Error {
                message: erreur.to_string(),
            }],
        };

        repondre(&mut sortie, &lignes)?;
    }
}

/// Écrit une réponse, en garantissant qu'elle se termine.
///
/// Si l'appelant a oublié la ligne terminale, on l'ajoute : une réponse sans
/// fin laisse le client bloqué en lecture pour toujours.
fn repondre(sortie: &mut UnixStream, lignes: &[ResponseLine]) -> io::Result<()> {
    for ligne in lignes {
        writeln!(sortie, "{}", ipc::encode_response_line(ligne))?;
    }
    if !lignes.last().is_some_and(ResponseLine::is_terminal) {
        writeln!(sortie, "{}", ipc::encode_response_line(&ResponseLine::End))?;
    }
    sortie.flush()
}
