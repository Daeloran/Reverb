//! Tests d'intention de l'issue #149 côté fenêtre : le cran de 0.5 part au démon, et une vitesse
//! au dixième posée par le socket est lue telle quelle.
//!
//! Écrits **depuis l'issue #149 seule**, avant l'implémentation. Rien de `crates/reverb-gui/src/`
//! ni de `crates/reverb-gui/ui/` n'a été lu pour les produire, hors les signatures publiques de
//! `reglages.rs` (`Reglage`, `Eclairage`, `eclairage_lu`, `requete_d_animation`) ; les idiomes
//! viennent de `spec_reglages.rs`, `spec_eclairage_lu.rs` et `spec_fenetre.rs`. Si l'un de ces
//! tests échoue après implémentation, c'est le code qu'on corrige.
//!
//! # Le contrat de signature supposé
//!
//! ```ignore
//! pub struct Reglage  { …, pub vitesse: f32, … }          // était u8
//! pub struct Eclairage { …, pub vitesse: Option<f32>, … } // était Option<u8>
//! ```
//!
//! Même type que `reverb_anim::Reglages::vitesse` (voir `crates/reverb-anim/tests/spec_149_vitesse.rs`) :
//! la fenêtre ne porte pas une seconde représentation de la vitesse.
//!
//! # Ce que ce fichier vérifie
//!
//! | critère ou comportement de l'issue | test |
//! |---|---|
//! | « choisir 0.5 envoie `vitesse=0.5` » | [`choisir_la_vitesse_un_demi_envoie_vitesse_0_5_que_le_demon_accepte`] |
//! | les crans de 0.5 à 10 par pas de 0.5 partent tels quels | [`chaque_cran_de_0_5_a_10_part_au_demon_tel_qu_il_est_affiche`] |
//! | une vitesse hors crans (`0.7`) posée par le socket est lue telle quelle | [`une_vitesse_hors_des_crans_venue_du_socket_est_lue_et_adoptee_telle_quelle`] |
//! | la fenêtre ignore ce que le démon refuserait | [`une_vitesse_que_le_demon_refuserait_n_est_pas_lue`] |
//!
//! # Ce que ce fichier ne vérifie pas, et pourquoi
//!
//! ⚠️ **Le curseur lui-même** — son minimum à 0.5, son pas de 0.5, son libellé `Vitesse 0.5` /
//! `Vitesse 3`, et sa place au cran le plus proche d'une vitesse `0.7` **sans la renvoyer au
//! démon** tant qu'on n'y touche pas. Tout cela vit dans `fenetre.slint`, et ce dépôt ne
//! l'instancie dans aucun test (`spec_fenetre.rs`, « ce qu'on peut tester d'une fenêtre »). Le
//! vérifier demanderait soit d'ouvrir une session graphique, soit de lire le `.slint`, ce que les
//! tests d'intention s'interdisent. Ce qui en est testable ici est le **modèle** derrière le
//! curseur : qu'un cran de 0.5 produise la commande attendue, et qu'une vitesse lue hors des
//! crans ne soit pas arrondie par la lecture ni par l'adoption — c'est la condition pour que le
//! curseur puisse se placer sans rien renvoyer. Le reste se juge à l'œil, sur la fenêtre.

use reverb_anim::{Animation, Direction, Reglages};
use reverb_gui::reglages::{Reglage, eclairage_lu, requete_d_animation};
use reverb_proto::Rgb;
use reverb_proto::ipc::{Request, ResponseLine, encode_request, parse_request};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// L'animation de l'issue : `animate vague vitesse=0.5`.
const ANIMATION: &str = "vague";

/// Une famille sans cycle : elle accepte aussi les décimales (critère n° 5).
const SANS_CYCLE: &str = "scintillement";

/// La couleur affichée, différente du défaut de `Reglages` comme dans `spec_fenetre.rs`.
const COULEUR: Rgb = Rgb::new(0x12, 0x9a, 0x40);

/// La direction affichée, différente du défaut (`horaire`).
const DIRECTION: Direction = Direction::AvantArriere;

/// Le cran le plus bas du curseur, selon l'issue.
const MINIMUM: f32 = 0.5;

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

/// Les réglages tels que la fenêtre les affiche.
fn reglage(nom: &str, vitesse: f32) -> Reglage {
    Reglage {
        animation: Some(nom.to_owned()),
        couleur: COULEUR,
        vitesse,
        direction: rang(DIRECTION),
        palette: None,
        // #150 : la fenêtre porte la `largeur` de `course` ; 10 est son défaut (essai boîtier du 2026-10-31), sans effet ailleurs.
        largeur: 10,
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

/// La ligne qu'une requête écrit sur le socket, après s'être assuré que le démon la relit.
fn sur_le_socket(requete: &Request) -> String {
    let ligne = encode_request(requete);
    let relue = parse_request(&ligne)
        .unwrap_or_else(|erreur| panic!("le démon doit comprendre « {ligne} » : {erreur}"));
    assert_eq!(
        relue, *requete,
        "« {ligne} » ne rend pas la requête qui l'a écrite"
    );
    ligne
}

/// Une ligne `anim` de réponse `lighting` portant cette vitesse, avec couleur et direction.
fn ligne_anim(nom: &str, vitesse: &str) -> ResponseLine {
    ResponseLine::Anim {
        nom: nom.to_owned(),
        reglages: vec![
            ("couleur".to_owned(), "129a40".to_owned()),
            ("vitesse".to_owned(), vitesse.to_owned()),
            ("direction".to_owned(), DIRECTION.slug().to_owned()),
        ],
    }
}

// ---------------------------------------------------------------------------
// 1 — le cran de 0.5
// ---------------------------------------------------------------------------

#[test]
fn choisir_la_vitesse_un_demi_envoie_vitesse_0_5_que_le_demon_accepte() {
    // Critère d'acceptation n° 6 : « le curseur propose 0.5 comme minimum, et choisir 0.5 envoie
    // `vitesse=0.5` ». Les deux chemins par lesquels la fenêtre émet une animation y passent : le
    // geste sur un curseur (`Reglage::commande`, #32) et le panneau d'animation
    // (`requete_d_animation`, #76). Et la famille de l'issue comme une famille sans cycle.
    for nom in [ANIMATION, SANS_CYCLE] {
        let affiche = reglage(nom, MINIMUM);
        for (chemin, requete) in [
            ("Reglage::commande", affiche.commande()),
            (
                "requete_d_animation",
                Some(
                    requete_d_animation(&affiche, None).unwrap_or_else(|erreur| {
                        panic!("« {nom} » à la vitesse 0.5 doit être acceptée : {erreur}")
                    }),
                ),
            ),
        ] {
            let contexte = format!("« {nom} » à 0.5 par {chemin}");
            let requete_gardee = requete.clone();
            let (envoye, paires) = paires_animees(requete, &contexte);
            assert_eq!(
                envoye, nom,
                "{contexte} : la famille affichée part au démon"
            );
            assert_eq!(
                valeur(&paires, "vitesse"),
                Some("0.5"),
                "{contexte} : c'est « vitesse=0.5 » qui part, ni « 0 », ni « 1 », ni « 0.50 » — \
                 {paires:?}"
            );
            assert_eq!(
                relu(nom, &paires).vitesse,
                MINIMUM,
                "{contexte} : le démon lit 0.5 — {paires:?}"
            );
            let ligne = sur_le_socket(&requete_gardee.expect("vérifiée ci-dessus"));
            assert!(
                ligne.split_whitespace().any(|jeton| jeton == "vitesse=0.5"),
                "{contexte} : la ligne du socket porte « vitesse=0.5 » — « {ligne} »"
            );
        }
    }
}

#[test]
fn chaque_cran_de_0_5_a_10_part_au_demon_tel_qu_il_est_affiche() {
    // Comportement attendu : « le curseur de vitesse va de 0.5 à 10 par pas de 0.5 ». Chacun des
    // vingt crans doit produire une commande que le démon accepte, à la vitesse affichée, écrite
    // sans décimale inutile (`3`, et non `3.0`) ou avec une seule (`1.5`).
    for cran in 1..=20u8 {
        let vitesse = f32::from(cran) / 2.0;
        let attendue = if cran % 2 == 0 {
            format!("{}", cran / 2)
        } else {
            format!("{}.5", cran / 2)
        };
        let (_, paires) = paires_animees(
            reglage(ANIMATION, vitesse).commande(),
            &format!("cran {vitesse}"),
        );
        assert_eq!(
            valeur(&paires, "vitesse"),
            Some(attendue.as_str()),
            "le cran {vitesse} part en « vitesse={attendue} » — {paires:?}"
        );
        assert_eq!(
            relu(ANIMATION, &paires).vitesse,
            vitesse,
            "le démon lit le cran {vitesse} — {paires:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — une vitesse hors des crans, venue du socket
// ---------------------------------------------------------------------------

#[test]
fn une_vitesse_hors_des_crans_venue_du_socket_est_lue_et_adoptee_telle_quelle() {
    // Comportement attendu : « une vitesse posée par le socket hors de ces crans (`0.7`) est
    // affichée telle quelle par le libellé, et le curseur se place au cran le plus proche **sans la
    // renvoyer au démon** tant qu'on n'y touche pas ».
    //
    // Le libellé et le curseur sont dans le `.slint` (voir l'en-tête). Ce qui en est vérifiable ici
    // est leur condition : la lecture de `lighting` et l'adoption gardent `0.7`, et non le cran
    // voisin. Une fenêtre qui arrondirait à la lecture afficherait `Vitesse 0.5` et, au premier
    // geste sur la couleur ou la direction, renverrait au démon une vitesse que personne n'a
    // choisie.
    for (texte, attendue) in [("0.7", 0.7f32), ("0.1", 0.1), ("9.9", 9.9), ("3", 3.0)] {
        let lu = eclairage_lu(&[ligne_anim(ANIMATION, texte)]);
        assert_eq!(
            lu.vitesse,
            Some(attendue),
            "« vitesse={texte} » se lit {attendue}, sans arrondi au cran — {lu:?}"
        );

        let mut fenetre = reglage(ANIMATION, 4.0);
        fenetre.animation = None;
        assert!(
            fenetre.adopter(&lu),
            "une animation tourne alors que la fenêtre n'en pilotait aucune : c'est un changement"
        );
        assert_eq!(
            fenetre.vitesse, attendue,
            "l'adoption garde la vitesse {attendue} telle que le démon l'a écrite — {fenetre:?}"
        );

        // Changer la direction relance l'animation : la vitesse qui repart est celle du démon,
        // inchangée, puisque personne n'a touché au curseur.
        fenetre.direction = rang(Direction::BasHaut);
        let (_, paires) = paires_animees(fenetre.commande(), &format!("après adoption de {texte}"));
        assert_eq!(
            valeur(&paires, "vitesse"),
            Some(texte),
            "toucher à la direction ne change pas la vitesse {texte} — {paires:?}"
        );
    }
}

#[test]
fn une_vitesse_que_le_demon_refuserait_n_est_pas_lue() {
    // Le pendant fenêtre du critère n° 3 : ce que `reverb-anim` refuse, la fenêtre ne le lit pas
    // non plus — elle l'ignore, comme `spec_eclairage_lu.rs` (#43) le fait déjà pour `0` ou `11`.
    // Les valeurs sont celles de l'issue. Une fenêtre qui lirait `0,7` comme `0` ou `7`
    // afficherait une vitesse que le démon n'a pas.
    for refusee in ["0", "-1", "10.5", "0.75", "0,7", "abc"] {
        assert!(
            animation(ANIMATION)
                .reglages(&[("vitesse".to_owned(), refusee.to_owned())])
                .is_err(),
            "appareil : le démon refuse « vitesse={refusee} »"
        );
        let lu = eclairage_lu(&[ligne_anim(ANIMATION, refusee)]);
        assert_eq!(
            lu.vitesse, None,
            "« vitesse={refusee} » n'est pas une vitesse du catalogue : elle est ignorée — {lu:?}"
        );
        assert_eq!(
            (lu.animation.as_deref(), lu.couleur, lu.direction),
            (Some(ANIMATION), Some(COULEUR), Some(rang(DIRECTION))),
            "une vitesse illisible n'emporte ni le nom ni les deux autres réglages — {lu:?}"
        );
    }
}
