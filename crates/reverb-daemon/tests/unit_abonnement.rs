//! Tests de **logique** de l'abonnement `watch` (issue #23).
//!
//! ⚠️ Ce ne sont **pas** des tests d'intention : ils ont été écrits après
//! l'implémentation, en la voyant. Les tests d'intention de #23 portent sur le
//! protocole (`spec_ipc_v3.rs`) et sur la projection (`spec_plan.rs`), tous deux
//! écrits à l'aveugle. Ceux-ci couvrent le mécanisme d'abonnement, que la
//! session de spec avait explicitement laissé de côté — « ils parlent d'un
//! abonnement et d'un socket, rien de tout ça ne vit dans `reverb-proto` ».
//!
//! Ce qu'ils protègent est la seule chose de ce chantier qui puisse se payer en
//! fluidité visible : une fenêtre lente ne doit pas faire saccader le boîtier.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc::{channel, sync_channel};
use std::thread;
use std::time::Duration;

use reverb_daemon::serveur::{self, Ordre};
use reverb_proto::Rgb;
use reverb_proto::ipc::ResponseLine;

/// Une image quelconque, sous la forme qu'un abonné reçoit.
fn image(teinte: u8) -> Vec<ResponseLine> {
    vec![
        ResponseLine::Frame {
            cible: "fan:arriere".to_owned(),
            couleurs: vec![Rgb::new(teinte, 0, 0); 8],
        },
        ResponseLine::End,
    ]
}

/// Un chemin de socket qui n'entre en collision avec personne.
///
/// Dans `/tmp` et non dans un sous-dossier : `sockaddr_un` borne le chemin à
/// une centaine d'octets, et un chemin de test profond y suffit à faire échouer
/// `bind` pour une raison qui n'a rien à voir avec ce qu'on teste.
fn socket_jetable(nom: &str) -> PathBuf {
    let chemin = std::env::temp_dir().join(format!("reverb-{nom}-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&chemin);
    chemin
}

#[test]
fn un_abonne_recoit_les_images_poussees_par_la_boucle() {
    let chemin = socket_jetable("abonne");
    let (envoi, ordres) = channel();

    let ecoute = chemin.clone();
    thread::spawn(move || {
        let _ = serveur::servir(&ecoute, envoi);
    });

    // Une fausse boucle de rendu : elle ne tient aucun matériel, elle pousse.
    thread::spawn(move || {
        let mut abonnes = Vec::new();
        while let Ok(ordre) = ordres.recv() {
            let Ordre { abonnement, .. } = ordre;
            if let Some(canal) = abonnement {
                abonnes.push(canal);
            }
            for teinte in 1..=3 {
                serveur::diffuser(&mut abonnes, &image(teinte));
            }
        }
    });

    let mut flux = attendre_le_socket(&chemin);
    writeln!(flux, "watch").expect("l'abonnement part");
    flux.flush().expect("l'abonnement part");

    let mut lecteur = BufReader::new(flux);
    let mut recues = Vec::new();
    for _ in 0..6 {
        let mut ligne = String::new();
        lecteur.read_line(&mut ligne).expect("une image arrive");
        recues.push(ligne.trim_end().to_owned());
    }

    assert_eq!(
        recues,
        vec![
            "frame fan:arriere 010000,010000,010000,010000,010000,010000,010000,010000",
            "end",
            "frame fan:arriere 020000,020000,020000,020000,020000,020000,020000,020000",
            "end",
            "frame fan:arriere 030000,030000,030000,030000,030000,030000,030000,030000",
            "end",
        ],
        "l'abonné reçoit les images dans l'ordre, chacune terminée par « end » — c'est ce qui lui \
         dit qu'elle est complète",
    );

    let _ = std::fs::remove_file(&chemin);
}

#[test]
fn un_abonne_parti_se_retire_de_lui_meme() {
    let (canal, arrivees) = sync_channel(2);
    let mut abonnes = vec![canal];

    serveur::diffuser(&mut abonnes, &image(1));
    assert_eq!(abonnes.len(), 1, "un abonné qui écoute reste abonné");

    drop(arrivees);
    serveur::diffuser(&mut abonnes, &image(2));
    assert!(
        abonnes.is_empty(),
        "un abonné dont le canal est fermé doit être oublié : sans ça, la liste enflerait à \
         chaque fenêtre fermée, et le démon pousserait des images à personne pour toujours",
    );
}

#[test]
fn un_abonne_lent_perd_ses_images_mais_ne_retient_personne() {
    // La file vaut deux images. On en pousse cent sans jamais lire : si la
    // diffusion attendait, ce test ne finirait pas — c'est le boîtier entier
    // qui se figerait derrière une fenêtre occupée ailleurs.
    let (canal, arrivees) = sync_channel(2);
    let mut abonnes = vec![canal];

    let (fini, verdict) = channel();
    thread::spawn(move || {
        for teinte in 0..100 {
            serveur::diffuser(&mut abonnes, &image(teinte));
        }
        let _ = fini.send(abonnes.len());
    });

    let restants = verdict
        .recv_timeout(Duration::from_secs(5))
        .expect("cent images poussées à un abonné qui ne lit pas ne doivent bloquer personne");
    assert_eq!(
        restants, 1,
        "un abonné en retard reste abonné : il rattrapera"
    );

    let mut lues = 0;
    while arrivees.try_recv().is_ok() {
        lues += 1;
    }
    assert!(
        lues <= 2,
        "la file est bornée à deux images : {lues} en attente veut dire qu'un abonné muet peut \
         faire enfler la mémoire du démon",
    );
}

/// Attend que le socket existe, sans dormir un temps fixe.
fn attendre_le_socket(chemin: &PathBuf) -> UnixStream {
    for _ in 0..200 {
        if let Ok(flux) = UnixStream::connect(chemin) {
            return flux;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("le socket {} ne s'est pas ouvert", chemin.display());
}
