//! Tests de **logique** du dialogue avec le démon (issue #23).
//!
//! ⚠️ Ce ne sont **pas** des tests d'intention : ils ont été écrits après
//! l'implémentation. Les tests d'intention de #23 sont `spec_plan.rs` ici et
//! `spec_ipc_v3.rs` dans `reverb-proto`, tous deux écrits à l'aveugle.
//!
//! Ils tiennent un démon en carton au bout d'un vrai socket : c'est la seule
//! façon de vérifier sans matériel que la fenêtre lit ce qu'on lui envoie, et
//! qu'elle ne se décale pas d'une réponse.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use reverb_gui::client::{Abonnement, Client};
use reverb_proto::Rgb;
use reverb_proto::ipc::{LightTarget, Request, ResponseLine};

/// Un chemin de socket qui n'entre en collision avec personne.
fn socket_jetable(nom: &str) -> PathBuf {
    let chemin = std::env::temp_dir().join(format!("reverb-gui-{nom}-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&chemin);
    chemin
}

/// Un démon en carton : il répond ce qu'on lui a dit de répondre.
fn faux_demon(chemin: &Path, reponses: Vec<&'static str>) {
    let ecouteur = UnixListener::bind(chemin).expect("le faux démon ouvre son socket");
    thread::spawn(move || {
        for flux in ecouteur.incoming() {
            let Ok(flux) = flux else { continue };
            let reponses = reponses.clone();
            thread::spawn(move || servir(flux, &reponses));
        }
    });
}

fn servir(flux: UnixStream, reponses: &[&str]) {
    let mut sortie = flux.try_clone().expect("le flux se dédouble");
    let lecteur = BufReader::new(flux);
    for ligne in lecteur.lines() {
        let Ok(_) = ligne else { return };
        for reponse in reponses {
            if writeln!(sortie, "{reponse}").is_err() {
                return;
            }
        }
        let _ = sortie.flush();
    }
}

#[test]
fn une_reponse_se_lit_jusqu_a_sa_ligne_terminale_et_pas_au_dela() {
    // Le décalage d'une réponse est le pire mode de défaillance d'un protocole
    // texte : la fenêtre affiche l'état d'avant, puis celui d'avant encore, et
    // rien ne le signale jamais.
    let chemin = socket_jetable("reponse");
    faux_demon(
        &chemin,
        vec!["light fan:arriere ff2080", "light slot:0 000000", "end"],
    );

    let mut client = attendre(&chemin);
    let lignes = client
        .demander(&Request::Lighting)
        .expect("le faux démon répond");
    assert_eq!(
        lignes,
        vec![
            ResponseLine::Light {
                cible: "fan:arriere".to_owned(),
                couleur: Rgb::new(0xff, 0x20, 0x80),
            },
            ResponseLine::Light {
                cible: "slot:0".to_owned(),
                couleur: Rgb::BLACK,
            },
            ResponseLine::End,
        ],
    );

    // La deuxième requête doit rendre la même chose : si la première avait lu
    // une ligne de trop ou de trop peu, celle-ci serait décalée.
    let encore = client
        .demander(&Request::Light {
            target: LightTarget::All,
            color: Rgb::new(0x00, 0x00, 0xff),
        })
        .expect("le faux démon répond encore");
    assert_eq!(
        encore.len(),
        3,
        "aucune ligne ne déborde d'une réponse sur la suivante"
    );

    let _ = std::fs::remove_file(&chemin);
}

#[test]
fn une_ligne_inconnue_ne_fait_pas_perdre_la_reponse() {
    // Un démon plus récent que la fenêtre peut répondre des lignes qu'elle ne
    // connaît pas encore. Les ignorer coûte un affichage incomplet ; s'y
    // arrêter coûte la connexion, et l'utilisateur ne voit plus rien du tout.
    let chemin = socket_jetable("inconnue");
    faux_demon(
        &chemin,
        vec!["quelquechose de nouveau", "light fan:arriere 00ff00", "end"],
    );

    let mut client = attendre(&chemin);
    let lignes = client
        .demander(&Request::Lighting)
        .expect("le démon répond");
    assert_eq!(
        lignes,
        vec![
            ResponseLine::Light {
                cible: "fan:arriere".to_owned(),
                couleur: Rgb::new(0x00, 0xff, 0x00),
            },
            ResponseLine::End,
        ],
    );

    let _ = std::fs::remove_file(&chemin);
}

#[test]
fn un_abonnement_rend_une_image_a_la_fois() {
    // Deux images d'affilée sur le même flux : c'est le régime normal de
    // `watch`, et l'endroit où un décalage d'une ligne se paierait en couleurs
    // affichées sur la mauvaise cible.
    let chemin = socket_jetable("abonnement");
    faux_demon(
        &chemin,
        vec![
            "frame fan:arriere ff0000,ff0000,ff0000,ff0000,ff0000,ff0000,ff0000,ff0000",
            "end",
            "frame fan:arriere 00ff00,00ff00,00ff00,00ff00,00ff00,00ff00,00ff00,00ff00",
            "end",
        ],
    );

    let mut abonnement = attendre_abonnement(&chemin);
    for attendue in [Rgb::new(0xff, 0, 0), Rgb::new(0, 0xff, 0)] {
        let image = abonnement.image_suivante().expect("une image arrive");
        assert_eq!(
            image,
            vec![ResponseLine::Frame {
                cible: "fan:arriere".to_owned(),
                couleurs: vec![attendue; 8],
            }],
            "chaque image est rendue seule, sans sa voisine ni sa ligne de fin"
        );
    }

    let _ = std::fs::remove_file(&chemin);
}

#[test]
fn un_demon_absent_se_dit_au_lieu_de_faire_planter() {
    // Critère d'acceptation de #23 : « démon absent ou socket injoignable :
    // elle le dit ». Ce que ce test garantit est la moitié qui se teste — que
    // l'erreur remonte au lieu de terminer le processus.
    let chemin = socket_jetable("absent");
    let Err(erreur) = Client::connecter(&chemin) else {
        panic!("aucun démon n'écoute là : la connexion doit échouer");
    };
    assert!(
        !erreur.to_string().is_empty(),
        "l'erreur doit dire quelque chose : c'est ce que la fenêtre affichera"
    );
}

fn attendre(chemin: &Path) -> Client {
    for _ in 0..200 {
        if let Ok(client) = Client::connecter(chemin) {
            return client;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("le faux démon ne s'est pas ouvert");
}

fn attendre_abonnement(chemin: &Path) -> Abonnement {
    for _ in 0..200 {
        if let Ok(abonnement) = Abonnement::ouvrir(chemin) {
            return abonnement;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("le faux démon ne s'est pas ouvert");
}
