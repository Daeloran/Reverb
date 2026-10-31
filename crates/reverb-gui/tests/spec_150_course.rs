//! Tests d'intention de l'issue #150 côté fenêtre : `course` est offerte, et sa `largeur` part au
//! démon.
//!
//! Écrits **depuis l'issue #150 seule**, avant l'implémentation. Rien de `crates/reverb-gui/src/`
//! ni de `crates/reverb-gui/ui/` n'a été lu pour les produire, hors les signatures publiques de
//! `reglages.rs` (`Reglage`, `familles_offertes`, `requete_d_animation`) ; les idiomes viennent de
//! `spec_fenetre.rs` et `spec_149_vitesse.rs`. Si l'un de ces tests échoue après implémentation,
//! c'est le code qu'on corrige.
//!
//! # Le contrat de signature supposé
//!
//! ```ignore
//! pub struct Reglage { …, pub largeur: u8 } // de 1 à 10, comme `reverb_anim::Reglages::largeur`
//! ```
//!
//! Même type que `reverb_anim::Reglages::largeur` (voir
//! `crates/reverb-anim/tests/spec_150_course.rs`) : la fenêtre ne porte pas une seconde
//! représentation de la largeur.
//!
//! # Ce que ce fichier vérifie
//!
//! | critère ou comportement de l'issue | test |
//! |---|---|
//! | « la fenêtre propose `course` parmi les familles », ajoutée en dernier | [`la_fenetre_offre_course_en_dernier`] |
//! | la largeur affichée part au démon, sur les deux chemins d'émission | [`la_largeur_affichee_part_au_demon_sous_course`] |
//! | une autre famille ne reçoit pas `largeur` | [`une_autre_famille_ne_recoit_pas_de_largeur`] |
//!
//! # Ce que ce fichier ne vérifie pas, et pourquoi
//!
//! ⚠️ **Le curseur `largeur` « visible seulement sous `course` ».** La visibilité vit dans
//! `fenetre.slint`, que ce dépôt n'instancie dans aucun test (`spec_fenetre.rs`, « ce qu'on peut
//! tester d'une fenêtre ») ; la vérifier demanderait d'ouvrir une session graphique ou de lire le
//! `.slint`, ce que les tests d'intention s'interdisent. Ce qui en est testable ici est le
//! **modèle** derrière le curseur : sous `course`, la valeur part ; ailleurs, elle ne part pas et ne
//! fait donc pas refuser la commande. Le reste se juge à l'œil, sur la fenêtre.
//!
//! La **lecture** de `largeur` dans une réponse `lighting` (`eclairage_lu`, `Reglage::adopter`)
//! n'est pas non plus vérifiée : l'issue ne la demande pas à la fenêtre.

use reverb_anim::{Animation, Direction, Reglages};
use reverb_gui::reglages::{Reglage, familles_offertes, requete_d_animation};
use reverb_proto::Rgb;
use reverb_proto::ipc::{Request, encode_request, parse_request};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// Le nom de la famille, tel que l'issue le fixe.
const COURSE: &str = "course";

/// La couleur affichée, différente du défaut de `Reglages` comme dans `spec_fenetre.rs`.
const COULEUR: Rgb = Rgb::new(0x12, 0x9a, 0x40);

/// La direction affichée, différente du défaut (`horaire`).
const DIRECTION: Direction = Direction::BasHaut;

/// La vitesse affichée.
const VITESSE: f32 = 3.0;

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

fn rang(direction: Direction) -> usize {
    Direction::ALL
        .into_iter()
        .position(|connue| connue == direction)
        .unwrap_or_else(|| panic!("{direction:?} est une des huit directions"))
}

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
}

/// Les réglages tels que la fenêtre les affiche, à la largeur donnée.
fn reglage(nom: &str, largeur: u8) -> Reglage {
    Reglage {
        animation: Some(nom.to_owned()),
        couleur: COULEUR,
        vitesse: VITESSE,
        direction: rang(DIRECTION),
        palette: None,
        largeur,
    }
}

/// Le nom et les paires portés par une requête d'animation, ou un échec qui dit ce qui est venu.
fn paires_animees(requete: Option<Request>, contexte: &str) -> (String, Vec<(String, String)>) {
    match requete {
        Some(Request::Animate {
            name: Some(nom),
            reglages,
        }) => (nom, reglages),
        autre => panic!("{contexte} : un « animate » nommé est attendu, reçu {autre:?}"),
    }
}

/// La valeur portée pour une clé, si elle est portée.
fn valeur<'a>(paires: &'a [(String, String)], cle: &str) -> Option<&'a str> {
    paires
        .iter()
        .find(|(portee, _)| portee == cle)
        .map(|(_, valeur)| valeur.as_str())
}

/// Ce que le démon lira de ces paires — `reverb-anim`, le juge de l'autre côté du socket.
fn relu(nom: &str, paires: &[(String, String)]) -> Reglages {
    animation(nom).reglages(paires).unwrap_or_else(|erreur| {
        panic!("le démon doit accepter {paires:?} pour « {nom} » : {erreur}")
    })
}

/// Les deux chemins par lesquels la fenêtre émet une animation : le geste sur un curseur
/// (`Reglage::commande`, #32) et le panneau d'animation (`requete_d_animation`, #76).
fn emissions(affiche: &Reglage) -> Vec<(&'static str, Option<Request>)> {
    vec![
        ("Reglage::commande", affiche.commande()),
        (
            "requete_d_animation",
            Some(requete_d_animation(affiche, None).unwrap_or_else(|erreur| {
                panic!("{affiche:?} doit produire une commande : {erreur}")
            })),
        ),
    ]
}

// ---------------------------------------------------------------------------
// 1 — course est offerte
// ---------------------------------------------------------------------------

#[test]
fn la_fenetre_offre_course_en_dernier() {
    // Critère n° 6 : « la fenêtre propose `course` parmi les familles ». Approche technique :
    // `course` est ajoutée « en dernier au catalogue pour ne pas déplacer les boutons de la
    // fenêtre » — la fenêtre la montre donc après les autres.
    let offertes = familles_offertes();
    assert!(
        offertes.contains(&COURSE),
        "« {COURSE} » est à portée de clic — la fenêtre n'offre que {offertes:?}"
    );
    assert_eq!(
        offertes.last(),
        Some(&COURSE),
        "« {COURSE} » vient après les familles d'avant, qui gardent leur place : {offertes:?}"
    );
}

// ---------------------------------------------------------------------------
// 2 — la largeur part au démon
// ---------------------------------------------------------------------------

#[test]
fn la_largeur_affichee_part_au_demon_sous_course() {
    // Comportement attendu : « la fenêtre ajoute un bouton et un curseur ». Ce qui est affiché est
    // ce qui part : une largeur laissée de côté ferait retomber le démon sur 4, sans un mot — le
    // défaut de #32 par une autre porte.
    //
    // Les dix largeurs ; à 4, la clé peut être portée ou non (le démon lit 4 dans les deux cas),
    // mais ailleurs elle doit l'être, écrite comme un entier.
    for largeur in 1..=10u8 {
        let affiche = reglage(COURSE, largeur);
        for (chemin, requete) in emissions(&affiche) {
            let contexte = format!("« {COURSE} » à la largeur {largeur} par {chemin}");
            let gardee = requete.clone();
            let (envoye, paires) = paires_animees(requete, &contexte);
            assert_eq!(
                envoye, COURSE,
                "{contexte} : la famille affichée part au démon"
            );
            if largeur != 4 {
                assert_eq!(
                    valeur(&paires, "largeur"),
                    Some(largeur.to_string().as_str()),
                    "{contexte} : « largeur={largeur} » part — {paires:?}"
                );
            }
            assert_eq!(
                relu(COURSE, &paires).largeur,
                largeur,
                "{contexte} : le démon lit la largeur affichée — {paires:?}"
            );

            // Et la ligne passe le socket telle quelle.
            let requete = gardee.expect("vérifiée ci-dessus");
            let ligne = encode_request(&requete);
            let relue = parse_request(&ligne)
                .unwrap_or_else(|erreur| panic!("le démon doit comprendre « {ligne} » : {erreur}"));
            assert_eq!(relue, requete, "« {ligne} » rend la requête qui l'a écrite");
        }
    }
}

#[test]
fn une_autre_famille_ne_recoit_pas_de_largeur() {
    // Critère n° 1 vu de la fenêtre : « `largeur` est refusé par toute autre famille ». Le curseur
    // peut garder sa valeur quand on passe de `course` à `vague` ; la porter ferait refuser
    // l'`animate` entier.
    for nom in familles_offertes()
        .into_iter()
        .filter(|nom| *nom != COURSE && animation(nom).parametres_obligatoires().is_empty())
    {
        let affiche = reglage(nom, 7);
        for (chemin, requete) in emissions(&affiche) {
            let contexte = format!("« {nom} » avec une largeur 7 affichée, par {chemin}");
            let (_, paires) = paires_animees(requete, &contexte);
            assert_eq!(
                valeur(&paires, "largeur"),
                None,
                "{contexte} : seule « {COURSE} » reçoit `largeur` — {paires:?}"
            );
            relu(nom, &paires);
        }
    }
}
