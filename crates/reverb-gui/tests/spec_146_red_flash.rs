//! Tests d'intention de la palette `red-flash` côté fenêtre (issue #146).
//!
//! Écrits **depuis l'issue #146 seule**, avant l'implémentation, avec les idiomes de
//! `spec_eclairage_lu.rs`. Rien de `crates/reverb-gui/src/` n'a été lu hors les signatures
//! publiques et la documentation du champ [`Reglage::palette`] : « rang dans
//! `reverb_anim::PALETTES`, ou `None` pour « aucune » ».
//!
//! # Ce que ce fichier vérifie, et ce qu'il ne peut pas vérifier
//!
//! Critère d'acceptation : « le menu de la fenêtre propose « Aucune » puis les treize ». Test
//! d'intention : « le menu compte 14 entrées ».
//!
//! ⚠️ **La liste affichée par le menu n'est pas atteignable depuis un test.** Elle est construite
//! dans `main.rs`, le binaire, par une fonction privée, et poussée dans le `.slint` ; la
//! bibliothèque n'expose pas d'équivalent de `familles_offertes()` pour les palettes. Compter ses
//! quatorze lignes demanderait de lire `src/` ou d'y ajouter une fonction publique — ce que des
//! tests d'intention ne font pas. Ce fichier vérifie donc ce que la bibliothèque **expose** du
//! menu, c'est-à-dire le contrat de rang que le menu et le démon partagent :
//!
//! - le menu, c'est « Aucune » puis `PALETTES` : quatorze entrées, `red-flash` en dernière ;
//! - une réponse `lighting` portant `palette=red-flash` se lit au rang de `red-flash` ;
//! - ce rang, posé dans les réglages de la fenêtre, part sur le socket comme `palette=red-flash`,
//!   que le démon accepte.
//!
//! Ce qui reste à vérifier à l'œil, dans la fenêtre lancée : que le menu affiche bien ces
//! quatorze lignes.

use reverb_anim::{Animation, Direction, PALETTES, Reglages};
use reverb_gui::reglages::{Reglage, eclairage_lu};
use reverb_proto::ipc::{Request, ResponseLine};
use reverb_proto::{Position, Rgb};

/// Le nom de la palette que l'issue ajoute.
const RED_FLASH: &str = "red-flash";

/// Une famille qui accepte `palette` depuis #126, et la plus simple du catalogue.
const FAMILLE: &str = "vague";

/// Le libellé de la première entrée du menu : pas de palette, la couleur décide.
const AUCUNE: &str = "Aucune";

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|e| panic!("« {nom} » est au catalogue : {e}"))
}

/// Une réponse `lighting` complète : les quatorze couleurs fixes, une ligne `anim`, `end` — le
/// cadre de `spec_eclairage_lu.rs`.
fn reponse(ligne_anim: ResponseLine) -> Vec<ResponseLine> {
    let fixe = Rgb::new(0xde, 0xad, 0xbe);
    let mut lignes: Vec<ResponseLine> = Position::ALL
        .into_iter()
        .map(|position| ResponseLine::Light {
            cible: format!("fan:{}", position.slug()),
            couleur: fixe,
        })
        .chain((0..4).map(|slot| ResponseLine::Light {
            cible: format!("slot:{slot}"),
            couleur: fixe,
        }))
        .collect();
    lignes.push(ligne_anim);
    lignes.push(ResponseLine::End);
    lignes
}

#[test]
fn le_menu_propose_aucune_puis_les_treize_soit_quatorze_entrees() {
    // Test d'intention de l'issue — « Fenêtre : le menu compte 14 entrées » —, par le seul contrat
    // public : le menu est « Aucune » suivi de `PALETTES`, et `Reglage::palette` est un rang dans
    // `PALETTES`. Voir l'en-tête pour ce qui n'est pas atteignable d'ici.
    let menu: Vec<&str> = std::iter::once(AUCUNE)
        .chain(PALETTES.iter().copied())
        .collect();
    assert_eq!(
        menu.len(),
        14,
        "« Aucune » puis les treize palettes : {menu:?}"
    );
    assert_eq!(
        menu.last().copied(),
        Some(RED_FLASH),
        "« {RED_FLASH} » ferme le menu, après les douze de #126 : {menu:?}"
    );
}

#[test]
fn une_animation_lue_sous_red_flash_se_place_au_rang_de_red_flash() {
    // Critère « le menu propose « Aucune » puis les treize » vu depuis le démon : quand la fenêtre
    // s'ouvre sur un boîtier qui tourne déjà sous `red-flash` (#41), le menu doit s'y placer. Un
    // nom que la fenêtre ne connaît pas se lit `None` — le menu resterait sur « Aucune » alors que
    // le boîtier est rouge.
    //
    // La ligne `anim` est fabriquée par `reglages_ecrits`, l'émetteur réel du démon, comme dans
    // `spec_eclairage_lu.rs` : le format ne se recopie pas à la main.
    let famille = animation(FAMILLE);
    let reglages = famille
        .reglages(&[("palette".to_owned(), RED_FLASH.to_owned())])
        .unwrap_or_else(|e| panic!("« {FAMILLE} » accepte « palette={RED_FLASH} » : {e}"));
    let ligne = ResponseLine::Anim {
        nom: FAMILLE.to_owned(),
        reglages: famille.reglages_ecrits(&reglages),
    };

    let lu = eclairage_lu(&reponse(ligne));
    let rang = lu
        .palette
        .unwrap_or_else(|| panic!("la fenêtre doit reconnaître « palette={RED_FLASH} » : {lu:?}"));
    assert_eq!(
        PALETTES.get(rang).copied(),
        Some(RED_FLASH),
        "le rang lu ({rang}) doit désigner « {RED_FLASH} » dans PALETTES"
    );
    assert_eq!(
        rang, 12,
        "« {RED_FLASH} » est la treizième palette, au rang 12"
    );
}

#[test]
fn choisir_red_flash_dans_le_menu_envoie_palette_red_flash_que_le_demon_accepte() {
    // L'autre sens : la dernière entrée du menu, posée dans les réglages de la fenêtre, doit
    // partir sur le socket comme `palette=red-flash`, et le démon — `Animation::reglages`, même
    // juge des deux côtés — doit l'accepter.
    let reglage = Reglage {
        animation: Some(FAMILLE.to_owned()),
        couleur: Reglages::default().couleur,
        vitesse: 5.0, // #149 : vitesse décimale
        largeur: 10, // #150 : largeur des bandes, au défaut
        direction: Direction::ALL
            .into_iter()
            .position(|d| d == Direction::BasHaut)
            .expect("bas-haut est une direction"),
        palette: Some(12),
    };

    let (nom, paires) = match reglage.commande() {
        Some(Request::Animate {
            name: Some(nom),
            reglages,
        }) => (nom, reglages),
        autre => panic!("la fenêtre doit lancer « {FAMILLE} » sous « {RED_FLASH} » : {autre:?}"),
    };
    assert_eq!(nom, FAMILLE);
    assert!(
        paires
            .iter()
            .any(|(cle, valeur)| cle == "palette" && valeur == RED_FLASH),
        "la commande doit porter « palette={RED_FLASH} » : {paires:?}"
    );

    let relus = animation(&nom)
        .reglages(&paires)
        .unwrap_or_else(|e| panic!("le démon doit accepter {paires:?} : {e}"));
    assert_eq!(
        relus.palette.map(|palette| palette.nom()),
        Some(RED_FLASH),
        "le démon doit relire « {RED_FLASH} » : {paires:?}"
    );
}
