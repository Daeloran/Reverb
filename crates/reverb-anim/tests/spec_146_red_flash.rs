//! Tests d'intention de la palette `red-flash` (issue #146).
//!
//! Écrits **depuis l'issue #146 seule**, avant l'implémentation. Aucun corps de fonction de
//! `crates/reverb-anim/src/` n'a été relu pour les écrire ; seules ont servi les **signatures
//! publiques** relevées au `grep`, et les idiomes de `spec_126_palettes.rs`, dont ce fichier est
//! le prolongement. Si l'un de ces tests échoue après implémentation, c'est le code qu'on corrige.
//!
//! # Ce que l'issue demande, et où c'est vérifié
//!
//! | test d'intention de l'issue | le test qui le porte |
//! |---|---|
//! | les arrêts de `red-flash` sont ceux du tableau, au bit près | [`red_flash_porte_exactement_les_cinq_arrets_releves_sur_wled`] |
//! | `PALETTES` vaut les douze de #126 suivis de `red-flash` | [`palettes_vaut_les_douze_de_126_puis_red_flash_en_dernier`] |
//! | chaque famille qui accepte `palette` accepte `red-flash`, et son rendu change | [`chaque_famille_qui_accepte_palette_accepte_red_flash_et_son_rendu_change`] |
//! | l'échantillon à 130 vaut `fde4a3`, et aux deux bouts `000000` | [`l_echantillon_vaut_fde4a3_a_130_et_noir_aux_deux_bouts`] |
//! | le refus d'un nom inconnu cite `red-flash` | [`un_nom_inconnu_est_refuse_en_citant_les_treize`] |
//! | fenêtre : le menu compte 14 entrées | `crates/reverb-gui/tests/spec_146_red_flash.rs` |
//! | profil : aller-retour avec `red-flash` | `crates/reverb-daemon/tests/spec_146_red_flash.rs` |
//!
//! # ⚠️ Le changement de spécification que l'issue assume
//!
//! `spec_126_palettes.rs` figeait `PALETTES` à **exactement** douze noms. L'issue le relâche en
//! « la liste commence par les douze, dans l'ordre » : ce que #126 protégeait — l'ordre du menu,
//! donc ce que l'utilisateur clique — reste figé là-bas, et c'est **ici** que la longueur exacte
//! de treize est désormais arrêtée. Les deux fichiers ensemble disent la même chose qu'avant, plus
//! une palette.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi.

use reverb_anim::{Animation, CATALOGUE, Geometrie, PALETTES, Palette, Reglages};
use reverb_proto::Rgb;

// ---------------------------------------------------------------------------
// Le domaine
// ---------------------------------------------------------------------------

/// Les douze palettes de #126, dans l'ordre de son tableau, écrites en dur — comme dans
/// `spec_126_palettes.rs`, et pour la même raison : lues chez le code, elles ne vérifieraient rien.
const DOUZE: [&str; 12] = [
    "light-pink",
    "lava",
    "ocean",
    "paysage",
    "couchant",
    "aurore",
    "atlantica",
    "sakura",
    "nuit-avril",
    "glace",
    "orange-teal",
    "sorbet",
];

/// Le nom de la palette que l'issue ajoute.
const RED_FLASH: &str = "red-flash";

/// Les arrêts de Red Flash, relevés le 2026-10-31 sur le WLED du PC (`/json/palx`, palette n° 66,
/// WLED 16.0.1) et recopiés du tableau de l'issue. Position, puis couleur.
const ARRETS: [(u8, Rgb); 5] = [
    (0, Rgb::new(0x00, 0x00, 0x00)),
    (99, Rgb::new(0xf2, 0x0c, 0x08)),
    (130, Rgb::new(0xfd, 0xe4, 0xa3)),
    (155, Rgb::new(0xf2, 0x0c, 0x08)),
    (255, Rgb::new(0x00, 0x00, 0x00)),
];

/// Durée d'un cycle, en pas, à la vitesse 1 — figée par `spec_sens.rs` (issue #49).
const PERIODE: u32 = 120;

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
}

fn red_flash() -> Palette {
    Palette::par_nom(RED_FLASH)
        .unwrap_or_else(|erreur| panic!("« {RED_FLASH} » est une palette : {erreur}"))
}

fn paire(cle: &str, valeur: &str) -> (String, String) {
    (cle.to_owned(), valeur.to_owned())
}

/// L'écriture d'une couleur sur le socket : six chiffres hexadécimaux minuscules.
fn hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

/// Les familles qui acceptent `palette`.
///
/// ⚠️ Refuse de rendre une liste vide, comme dans `spec_126_palettes.rs` : un test qui boucle sur
/// une liste vide est vert sans rien vérifier.
fn familles_avec_palette() -> Vec<Animation> {
    let familles: Vec<Animation> = CATALOGUE
        .iter()
        .map(|nom| animation(nom))
        .filter(|animation| animation.parametres_acceptes().contains(&"palette"))
        .collect();
    assert!(
        !familles.is_empty(),
        "aucune famille du catalogue n'accepte « palette » : le test qui boucle sur cette liste ne \
         vérifie rien — {CATALOGUE:?}"
    );
    familles
}

// ---------------------------------------------------------------------------
// 1 — la palette et ses arrêts
// ---------------------------------------------------------------------------

#[test]
fn red_flash_porte_exactement_les_cinq_arrets_releves_sur_wled() {
    // Test d'intention n° 1 de l'issue — « les arrêts de `red-flash` sont ceux du tableau, au bit
    // près » —, et critère d'acceptation : « `Palette::par_nom("red-flash")` réussit, et ses arrêts
    // sont exactement les cinq ci-dessus ».
    //
    // « Exactement » vaut pour le nombre comme pour les valeurs : un sixième arrêt de raccord, ou
    // une couleur arrondie d'une unité à la recopie, ferait un dégradé qui n'est plus celui du
    // ruban WLED d'à côté — et c'est la seule raison d'être de cette palette.
    //
    // Le nom suit la règle de slug des douze : ASCII, minuscules, sans espace, puisqu'il traverse
    // le socket comme un jeton (`animate vague palette=red-flash`).
    assert!(RED_FLASH.is_ascii() && !RED_FLASH.contains(' '));
    assert_eq!(RED_FLASH, RED_FLASH.to_lowercase());

    let palette = red_flash();
    assert_eq!(
        palette.nom(),
        RED_FLASH,
        "la palette ouverte par « {RED_FLASH} » doit se nommer « {RED_FLASH} »"
    );

    let rendus: Vec<String> = palette
        .arrets()
        .iter()
        .map(|(position, couleur)| format!("{position}:{}", hexa(*couleur)))
        .collect();
    let attendus: Vec<String> = ARRETS
        .iter()
        .map(|(position, couleur)| format!("{position}:{}", hexa(*couleur)))
        .collect();
    assert_eq!(
        rendus, attendus,
        "« {RED_FLASH} » doit porter les cinq arrêts relevés sur le WLED du PC, au bit près"
    );
    assert_eq!(palette.arrets(), ARRETS.as_slice());
}

#[test]
fn l_echantillon_vaut_fde4a3_a_130_et_noir_aux_deux_bouts() {
    // Test d'intention n° 4 de l'issue — « l'échantillon à la position 130 vaut `fde4a3`, et aux
    // deux bouts `000000` ».
    //
    // Ce n'est pas un doublon du précédent : celui-là lit les **données**, celui-ci ce que les
    // familles **reçoivent**. Une palette aux bons arrêts mais mal câblée à `echantillon` — l'arrêt
    // 130 confondu avec un autre, un bout extrapolé — peindrait le boîtier sans que la table soit
    // fausse.
    //
    // ⚠️ L'échelle de l'argument est celle des arrêts, `0.0..=255.0` : c'est l'arbitrage n° 1 de
    // `spec_126_palettes.rs`, qui fait de « au bit près » une propriété et non un hasard d'arrondi.
    let palette = red_flash();
    for (position, attendue) in [
        (130.0f32, Rgb::new(0xfd, 0xe4, 0xa3)),
        (0.0, Rgb::BLACK),
        (255.0, Rgb::BLACK),
    ] {
        let rendue = palette.echantillon(position);
        assert_eq!(
            rendue,
            attendue,
            "« {RED_FLASH} » à la position {position} rend {} au lieu de {}",
            hexa(rendue),
            hexa(attendue)
        );
    }

    // Et chacun des cinq arrêts, puisque spec_126 ne boucle que sur les douze.
    for (position, attendue) in ARRETS {
        let rendue = palette.echantillon(f32::from(position));
        assert_eq!(
            rendue,
            attendue,
            "« {RED_FLASH} » à l'arrêt {position} rend {} au lieu de {}",
            hexa(rendue),
            hexa(attendue)
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — la liste, et l'ordre du menu
// ---------------------------------------------------------------------------

#[test]
fn palettes_vaut_les_douze_de_126_puis_red_flash_en_dernier() {
    // Test d'intention n° 2 de l'issue — « `PALETTES` vaut les douze de #126 suivis de
    // `red-flash` » —, et critère d'acceptation : « `PALETTES` compte **treize** noms : les douze
    // de #126, dans leur ordre, puis `red-flash` en **dernier**. Un ajout ne déplace pas ce que
    // l'utilisateur clique dans le menu. »
    //
    // ⚠️ L'ordre n'est pas cosmétique : la fenêtre manipule un **rang** dans `PALETTES`. Insérer
    // `red-flash` au milieu ferait glisser d'un cran toutes les palettes qui la suivent, et le
    // clic que l'utilisateur connaît par cœur choisirait la voisine, sans message.
    let mut attendue: Vec<&str> = DOUZE.to_vec();
    attendue.push(RED_FLASH);
    assert_eq!(
        PALETTES,
        attendue.as_slice(),
        "les douze de #126 dans leur ordre, puis « {RED_FLASH} » en dernier"
    );
    assert_eq!(PALETTES.len(), 13, "treize palettes : {PALETTES:?}");
}

// ---------------------------------------------------------------------------
// 3 — les familles
// ---------------------------------------------------------------------------

#[test]
fn chaque_famille_qui_accepte_palette_accepte_red_flash_et_son_rendu_change() {
    // Test d'intention n° 3 de l'issue — « chaque famille qui accepte `palette` accepte
    // `red-flash`, et son rendu change » —, et critère d'acceptation : « toute famille qui accepte
    // `palette` accepte `palette=red-flash`, et le rendu diffère de celui sans palette ».
    //
    // On passe par la porte du décodeur, `reglages`, parce que c'est celle qu'emprunte
    // `animate <famille> palette=red-flash` sur le socket. Et on vérifie le rendu, parce qu'une
    // palette acceptée puis ignorée est « le pire des trois défauts » (`spec_animations.rs`, n° 4).
    let geometrie = Geometrie::mesuree();
    let sans_palette = Reglages::default();

    for animation in familles_avec_palette() {
        let nom = animation.nom();
        let avec = animation
            .reglages(&[paire("palette", RED_FLASH)])
            .unwrap_or_else(|erreur| {
                panic!("« {nom} » doit accepter « palette={RED_FLASH} » : {erreur}")
            });
        assert_eq!(
            avec.palette.map(|palette| palette.nom()),
            Some(RED_FLASH),
            "« {nom} » : « palette={RED_FLASH} » doit atterrir dans le champ `palette`"
        );

        let differe = (0..PERIODE).any(|pas| {
            animation.image(&geometrie, &avec, pas)
                != animation.image(&geometrie, &sans_palette, pas)
        });
        assert!(
            differe,
            "« {nom} » peint la même image avec et sans « palette={RED_FLASH} » : la palette est \
             rangée dans les réglages, puis ignorée au rendu"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — le refus
// ---------------------------------------------------------------------------

#[test]
fn un_nom_inconnu_est_refuse_en_citant_les_treize() {
    // Test d'intention n° 5 de l'issue — « le refus d'un nom inconnu cite `red-flash` » —, et
    // critère d'acceptation : « un nom inconnu est refusé avec la liste des treize ».
    //
    // Les quasi-homonymes d'abord : ce sont eux qu'on tape en recopiant le nom affiché par WLED,
    // « Red Flash ». Un décodeur tolérant les accepterait, et le nom écrit dans `eclairage.conf`
    // ne serait plus celui que le démon relit.
    let saisies = [
        "Red Flash",
        "red flash",
        "red_flash",
        "Red-Flash",
        "RED-FLASH",
        "redflash",
        "bidule",
    ];
    // ⚠️ Aucune saisie ne contient « red-flash » telle quelle : le message cite le nom reçu, et
    // chercher « red-flash » dans le refus de « red-flash-x » serait vert sans rien prouver.

    for saisi in saisies {
        // Par la porte du catalogue de palettes…
        let erreur = Palette::par_nom(saisi).expect_err("un nom hors des treize est refusé");
        let message = erreur.to_string();
        for connue in DOUZE.iter().chain(&[RED_FLASH]) {
            assert!(
                message.contains(connue),
                "le refus de « {saisi} » doit citer « {connue} » : « {message} »"
            );
        }
        assert!(
            erreur.valides.contains(&RED_FLASH),
            "le refus de « {saisi} » doit porter « {RED_FLASH} » parmi les noms valides : {:?}",
            erreur.valides
        );

        // … et par celle du réglage, que l'utilisateur emprunte vraiment.
        for animation in familles_avec_palette() {
            let erreur = animation
                .reglages(&[paire("palette", saisi)])
                .expect_err("un nom hors des treize est refusé comme réglage");
            assert_eq!(
                erreur.cle,
                "palette",
                "« {} » : le refus doit nommer la clé fautive — {erreur}",
                animation.nom()
            );
            let message = erreur.to_string();
            assert!(
                message.contains(RED_FLASH),
                "« {} » : le refus de « palette={saisi} » doit citer « {RED_FLASH} » — « {message} »",
                animation.nom()
            );
        }
    }
}
