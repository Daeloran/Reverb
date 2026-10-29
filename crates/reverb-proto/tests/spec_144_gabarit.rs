//! Tests d'intention du mode gabarit SVG de l'écran — issue #144, côté **protocole**.
//!
//! Écrits avant l'implémentation depuis l'issue #144 seule ; si un test échoue après
//! implémentation, c'est le code qu'on corrige. Rien de `crates/*/src/` n'a été lu pour les
//! produire, hors les signatures publiques existantes (`Request`, `RequestError`, `ScreenAction`,
//! `parse_request`, `encode_request`) et la variante neuve que l'issue nomme,
//! `ScreenAction::Gabarit(String)`.
//!
//! Le pendant côté démon — état d'écran, rendu, refus d'un SVG invalide — vit dans
//! `crates/reverb-daemon/tests/spec_144_gabarit.rs`.
//!
//! ## Deux points que l'issue laisse ouverts, et que ces tests tranchent
//!
//! 1. **Le chemin va jusqu'au bout de la ligne, espaces comprises**, comme celui de
//!    `screen image` (`spec_ipc_ecran.rs`). L'issue dit que la CLI complète le chemin « comme
//!    `--image` » : un gabarit rangé dans `Mes documents/` doit donc passer de la même façon.
//! 2. **Le tilde n'est pas une racine.** `~/ecran.svg` est refusé comme un chemin relatif : c'est
//!    le shell qui l'étend, et le démon ne partage pas le foyer de son client.

use reverb_proto::ipc::{Request, RequestError, ScreenAction, encode_request, parse_request};

/// L'action `screen` d'une ligne acceptée, ou un échec qui dit ce qui est arrivé.
fn action_de(ligne: &str) -> ScreenAction {
    match parse_request(ligne) {
        Ok(Request::Screen(action)) => action,
        Ok(autre) => {
            panic!("« {ligne} » devait être une commande `screen`, elle a rendu {autre:?}")
        }
        Err(erreur) => panic!("« {ligne} » devait être acceptée : {erreur}"),
    }
}

/// Exige un refus d'arguments du verbe `screen`, avec une raison non vide.
fn refus_de_screen(ligne: &str) {
    match parse_request(ligne) {
        Ok(requete) => panic!("« {ligne} » devait être refusée, elle a rendu {requete:?}"),
        Err(RequestError::BadArgument { verb, reason }) => {
            assert_eq!(
                verb, "screen",
                "« {ligne} » : c'est `screen` qui refuse ses arguments"
            );
            assert!(
                !reason.trim().is_empty(),
                "« {ligne} » doit être refusée en disant pourquoi"
            );
        }
        Err(autre) => panic!("« {ligne} » devait être un refus d'arguments, pas {autre:?}"),
    }
}

#[test]
fn screen_template_avec_un_chemin_absolu_est_decode_en_gabarit() {
    assert_eq!(
        action_de("screen template /home/nico/gabarits/ecran.svg"),
        ScreenAction::Gabarit("/home/nico/gabarits/ecran.svg".to_owned())
    );
}

#[test]
fn le_chemin_d_un_gabarit_va_jusqu_au_bout_de_la_ligne() {
    let chemin = "/home/nico/Mes documents/écran sobre.svg";
    assert_eq!(
        action_de(&format!("screen template {chemin}")),
        ScreenAction::Gabarit(chemin.to_owned()),
        "un chemin à espaces ne doit pas être coupé au premier blanc : il désignerait un autre \
         fichier"
    );
}

#[test]
fn un_chemin_relatif_est_refuse_au_decodage() {
    for brut in [
        "ecran.svg",
        "./ecran.svg",
        "../gabarits/ecran.svg",
        "gabarits/ecran.svg",
        "~/ecran.svg",
    ] {
        refus_de_screen(&format!("screen template {brut}"));
    }
}

#[test]
fn un_chemin_absent_ou_vide_est_refuse_au_decodage() {
    for ligne in ["screen template", "screen template ", "screen template    "] {
        refus_de_screen(ligne);
    }
}

#[test]
fn un_gabarit_encode_se_relit_a_l_identique() {
    for chemin in [
        "/home/nico/gabarits/ecran.svg",
        "/home/nico/Mes documents/écran sobre.svg",
    ] {
        let requete = Request::Screen(ScreenAction::Gabarit(chemin.to_owned()));
        let ligne = encode_request(&requete);
        assert_eq!(
            parse_request(&ligne),
            Ok(requete.clone()),
            "« {ligne} » doit se relire en la requête qui l'a produite"
        );
    }
}
