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
use std::sync::mpsc::{Sender, SyncSender, TrySendError::Disconnected, channel, sync_channel};
use std::thread;

use reverb_proto::ipc::{self, MAX_LINE_LEN, Request, ResponseLine};

/// Images qu'un abonné peut avoir en retard avant qu'on lui en saute.
///
/// Deux, et pas davantage : un abonné qui ne lit pas à la cadence du boîtier ne
/// doit ni ralentir le rendu, ni faire enfler la mémoire du démon. À 21 images
/// par seconde, deux images de retard font 95 ms — au-delà, ce qu'il n'a pas lu
/// est de toute façon périmé.
const RETARD_TOLERE: usize = 2;

/// Ce qu'un client demande au fil de rendu.
pub struct Ordre {
    pub requete: Request,
    /// Par où renvoyer la réponse. Le fil de rendu y écrit une fois.
    pub reponse: Sender<Vec<ResponseLine>>,
    /// Par où pousser les images, quand la requête est un abonnement `watch`.
    ///
    /// Le fil de rendu la garde tant que le client écoute, et l'oublie dès que
    /// l'envoi échoue — c'est ainsi qu'un abonné parti se retire tout seul,
    /// sans que personne ait à le remarquer.
    pub abonnement: Option<SyncSender<Vec<ResponseLine>>>,
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
            // `watch` ne rend pas la main : la connexion devient un flux, et ce
            // fil n'y lit plus de commandes. Une fenêtre ouvre donc deux
            // connexions — une pour regarder, une pour agir — au lieu d'un
            // protocole où les réponses et les images se mêleraient sur le même
            // fil sans qu'on sache laquelle on lit.
            Ok(Request::Watch) => return abonner(&mut sortie, ordres),
            Ok(requete) => {
                let (repondeur, retour) = channel();
                if ordres
                    .send(Ordre {
                        requete,
                        reponse: repondeur,
                        abonnement: None,
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

/// Pousse une image à tous les abonnés, **sans jamais en attendre un**.
///
/// Un abonné en retard perd l'image au lieu de retenir la boucle de rendu : le
/// boîtier de tout le monde ne doit pas saccader parce qu'une fenêtre est
/// occupée ailleurs. Un abonné parti se retire de lui-même — son canal est
/// fermé, et c'est le seul signal dont on ait besoin.
pub fn diffuser(abonnes: &mut Vec<SyncSender<Vec<ResponseLine>>>, lignes: &[ResponseLine]) {
    abonnes.retain(|canal| !matches!(canal.try_send(lignes.to_vec()), Err(Disconnected(_))));
}

/// Transforme la connexion en flux d'images, jusqu'au départ du client.
///
/// La file est **bornée** : le fil de rendu y dépose sans jamais attendre, et
/// saute l'image quand elle est pleine. Un client lent ne peut donc pas faire
/// saccader le boîtier de tout le monde — il perd des images, ce qui ne se voit
/// que chez lui.
fn abonner(sortie: &mut UnixStream, ordres: &Sender<Ordre>) -> io::Result<()> {
    let (repondeur, _retour) = channel();
    let (images, arrivees) = sync_channel(RETARD_TOLERE);
    if ordres
        .send(Ordre {
            requete: Request::Watch,
            reponse: repondeur,
            abonnement: Some(images),
        })
        .is_err()
    {
        return Ok(());
    }

    // La boucle s'arrête d'elle-même : le client parti, l'écriture échoue, et
    // le fil de rendu s'aperçoit à son tour que plus personne n'écoute.
    while let Ok(lignes) = arrivees.recv() {
        repondre(sortie, &lignes)?;
    }
    Ok(())
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
