//! Tests d'intention de l'issue #150 côté fenêtre : `course` est offerte, et sa `largeur` part au
//! démon.
//!
//! Écrits **depuis l'issue #150 seule**, avant l'implémentation. Rien de `crates/reverb-gui/src/`
//! ni de `crates/reverb-gui/ui/` n'a été lu pour les produire, hors les signatures publiques de
//! `reglages.rs` (`Reglage`, `Eclairage`, `eclairage_lu`, `familles_offertes`,
//! `requete_d_animation`) ; les idiomes viennent de `spec_fenetre.rs`, `spec_eclairage_lu.rs` et
//! `spec_149_vitesse.rs`. Si l'un de ces tests échoue après implémentation,
//! c'est le code qu'on corrige.
//!
//! # Le contrat de signature supposé
//!
//! ```ignore
//! pub struct Reglage { …, pub largeur: u8 } // de 1 à 10, comme `reverb_anim::Reglages::largeur`
//! pub struct Eclairage { …, pub largeur: Option<u8> } // lue sur la ligne `anim` de `lighting`
//! ```
//!
//! La lecture de `Eclairage::largeur` suit la règle de la vitesse (#43, #149) :
//!
//! - sous `course`, `largeur=n` avec `n` de 1 à 10 se lit `Some(n)` ;
//! - sous `course`, **sans** `largeur=`, elle se lit `Some(10)` : le démon n'écrit pas le défaut,
//!   et c'est donc 10 qui tourne (défaut 10 depuis l'essai boîtier du 2026-10-31, 4 avant) ;
//! - une valeur hors de 1 à 10 ou mal écrite est **ignorée** (`None`), sans emporter le reste ;
//! - sous **toute autre famille**, `None`, avec ou sans clé : ces familles n'ont pas de largeur,
//!   et le démon n'en écrit jamais. `Reglage::adopter` laisse alors la largeur de la fenêtre
//!   intacte, comme pour tout réglage sur lequel le démon ne s'est pas prononcé — repasser sous
//!   `course` retrouve la largeur qu'on avait choisie.
//!
//! `Reglage::adopter` reprend une largeur `Some(n)`. Sans cela, une largeur posée par le socket
//! (`animate course largeur=7`) serait écrasée par celle de la fenêtre au premier geste sur un
//! curseur.
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
//! | une largeur posée par le socket est lue et adoptée | [`une_largeur_lue_sous_course_est_adoptee_et_repart_telle_quelle`] |
//! | sans `largeur=`, `course` tourne à 10 | [`course_sans_largeur_ecrite_se_lit_a_dix`] |
//! | une largeur illisible est ignorée | [`une_largeur_que_le_demon_refuserait_n_est_pas_lue`] |
//! | hors de `course`, aucune largeur n'est lue ni perdue | [`hors_de_course_la_largeur_n_est_pas_lue_et_la_fenetre_garde_la_sienne`] |
//!
//! # Ce que ce fichier ne vérifie pas, et pourquoi
//!
//! ⚠️ **Le curseur `largeur` « visible seulement sous `course` ».** La visibilité vit dans
//! `fenetre.slint`, que ce dépôt n'instancie dans aucun test (`spec_fenetre.rs`, « ce qu'on peut
//! tester d'une fenêtre ») ; la vérifier demanderait d'ouvrir une session graphique ou de lire le
//! `.slint`, ce que les tests d'intention s'interdisent. Ce qui en est testable ici est le
//! **modèle** derrière le curseur : sous `course`, la valeur part ; ailleurs, elle ne part pas et ne
//! fait donc pas refuser la commande. Le reste se juge à l'œil, sur la fenêtre.

use reverb_anim::{Animation, Direction, Reglages};
use reverb_gui::reglages::{Reglage, eclairage_lu, familles_offertes, requete_d_animation};
use reverb_proto::Rgb;
use reverb_proto::ipc::{Request, ResponseLine, encode_request, parse_request};

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
    // ce qui part : une largeur laissée de côté ferait retomber le démon sur 10, sans un mot — le
    // défaut de #32 par une autre porte.
    //
    // Les dix largeurs ; à 10, la clé peut être portée ou non (le démon lit 10 dans les deux cas),
    // mais ailleurs elle doit l'être, écrite comme un entier. #150, essai boîtier du 2026-10-31 :
    // le défaut passe de 4 à 10.
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
            if largeur != 10 {
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

// ---------------------------------------------------------------------------
// 3 — la largeur lue dans `lighting`
// ---------------------------------------------------------------------------

/// Une ligne `anim` de réponse `lighting`, couleur, vitesse et direction comprises, plus les paires
/// données — celle de `spec_149_vitesse.rs`.
fn ligne_anim(nom: &str, en_plus: &[(&str, &str)]) -> ResponseLine {
    let mut reglages = vec![
        ("couleur".to_owned(), "129a40".to_owned()),
        ("vitesse".to_owned(), "3".to_owned()),
        ("direction".to_owned(), DIRECTION.slug().to_owned()),
    ];
    reglages.extend(
        en_plus
            .iter()
            .map(|(cle, valeur)| ((*cle).to_owned(), (*valeur).to_owned())),
    );
    ResponseLine::Anim {
        nom: nom.to_owned(),
        reglages,
    }
}

#[test]
fn une_largeur_lue_sous_course_est_adoptee_et_repart_telle_quelle() {
    // `animate course largeur=7` posé par le socket : la fenêtre doit le lire, l'adopter, et le
    // renvoyer tel quel au premier geste sur un autre curseur — sans quoi elle écraserait 7 par sa
    // propre valeur, sans un mot.
    let lu = eclairage_lu(&[ligne_anim(COURSE, &[("largeur", "7")])]);
    assert_eq!(lu.largeur, Some(7), "« largeur=7 » se lit 7 — {lu:?}");

    let mut fenetre = reglage(COURSE, 4);
    fenetre.animation = None;
    assert!(
        fenetre.adopter(&lu),
        "une animation tourne alors que la fenêtre n'en pilotait aucune : c'est un changement"
    );
    assert_eq!(
        fenetre.largeur, 7,
        "l'adoption reprend la largeur du démon — {fenetre:?}"
    );

    // Toucher à la direction relance l'animation : la largeur qui repart est celle du démon.
    fenetre.direction = rang(Direction::HautBas);
    let (_, paires) = paires_animees(fenetre.commande(), "après adoption de largeur=7");
    assert_eq!(
        valeur(&paires, "largeur"),
        Some("7"),
        "toucher à la direction ne change pas la largeur 7 — {paires:?}"
    );
}

#[test]
fn course_sans_largeur_ecrite_se_lit_a_dix() {
    // #150, essai boîtier du 2026-10-31 : le défaut passe de 4 à 10.
    // Le démon n'écrit pas la largeur par défaut (critère n° 5) : une ligne `anim course` sans
    // `largeur=` dit donc que 10 tourne. Une fenêtre réglée sur 9 qui ne le lirait pas renverrait 9
    // au premier geste.
    let lu = eclairage_lu(&[ligne_anim(COURSE, &[])]);
    assert_eq!(
        lu.largeur,
        Some(10),
        "« anim course » sans largeur tourne à 10 — {lu:?}"
    );

    let mut fenetre = reglage(COURSE, 9);
    fenetre.animation = None;
    fenetre.adopter(&lu);
    assert_eq!(
        fenetre.largeur, 10,
        "l'adoption ramène la fenêtre à 10 — {fenetre:?}"
    );
}

#[test]
fn une_largeur_que_le_demon_refuserait_n_est_pas_lue() {
    // La règle de la vitesse (#43, #149) : ce que `reverb-anim` refuse, la fenêtre ne le lit pas.
    // Elle l'ignore, sans emporter le nom ni les autres réglages.
    for refusee in ["0", "11", "2.5", "abc"] {
        assert!(
            animation(COURSE)
                .reglages(&[("largeur".to_owned(), refusee.to_owned())])
                .is_err(),
            "appareil : le démon refuse « largeur={refusee} »"
        );
        let lu = eclairage_lu(&[ligne_anim(COURSE, &[("largeur", refusee)])]);
        assert_eq!(
            lu.largeur, None,
            "« largeur={refusee} » n'est pas une largeur : elle est ignorée — {lu:?}"
        );
        assert_eq!(
            (
                lu.animation.as_deref(),
                lu.couleur,
                lu.vitesse,
                lu.direction
            ),
            (
                Some(COURSE),
                Some(COULEUR),
                Some(VITESSE),
                Some(rang(DIRECTION))
            ),
            "une largeur illisible n'emporte ni le nom ni les autres réglages — {lu:?}"
        );

        let mut fenetre = reglage(COURSE, 6);
        fenetre.adopter(&lu);
        assert_eq!(
            fenetre.largeur, 6,
            "une largeur ignorée ne déplace pas celle de la fenêtre — {fenetre:?}"
        );
    }
}

#[test]
fn hors_de_course_la_largeur_n_est_pas_lue_et_la_fenetre_garde_la_sienne() {
    // Contrat de ce fichier (voir l'en-tête) : hors de `course`, `None`, et la fenêtre garde sa
    // largeur. Repasser sous `course` retrouve la largeur qu'on avait choisie, et non un 10 que
    // personne n'a demandé.
    for nom in ["vague", "arc-en-ciel"] {
        for en_plus in [&[][..], &[("largeur", "7")][..]] {
            let lu = eclairage_lu(&[ligne_anim(nom, en_plus)]);
            assert_eq!(
                lu.largeur, None,
                "« {nom} » n'a pas de largeur : rien n'est lu, avec {en_plus:?} — {lu:?}"
            );
            let mut fenetre = reglage(COURSE, 9);
            fenetre.adopter(&lu);
            assert_eq!(
                fenetre.largeur, 9,
                "sous « {nom} », la fenêtre garde sa largeur — {fenetre:?}"
            );
        }
    }
}
