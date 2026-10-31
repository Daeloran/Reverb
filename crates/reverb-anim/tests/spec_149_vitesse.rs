//! Tests d'intention de l'issue #149 : des vitesses d'animation **sous 1**, au dixième.
//!
//! Écrits **depuis l'issue #149 seule**, avant l'implémentation. Aucun corps de fonction de
//! `crates/reverb-anim/src/` n'a été relu pour les écrire — ni `temps`, ni `derive`, ni le
//! décodeur des réglages. N'ont servi que les **signatures publiques** relevées au `grep`, les
//! tests d'intention déjà présents dans ce répertoire pour les idiomes d'appel (dont l'empreinte
//! FNV-1a de `spec_148_schema_face.rs`), et l'exécution de l'API publique telle qu'elle existe sur
//! la branche de #148 (révision `d6bbc62`) pour figer les images qui ne doivent pas bouger.
//!
//! Si l'un de ces tests échoue après implémentation, c'est le code qu'on corrige.
//!
//! # Le contrat de signature que ce fichier fige
//!
//! ```ignore
//! pub struct Reglages {
//!     pub couleur: Rgb,
//!     /// De 0.1 à 10, au dixième : 0.1, 0.2, …, 9.9, 10. Validée à la lecture par
//!     /// `Animation::reglages` ; les entiers de 1 à 10 rendent les images d'avant #149.
//!     pub vitesse: f32,
//!     pub direction: Direction,
//!     pub sonde: Option<String>,
//!     pub palette: Option<Palette>,
//! }
//! ```
//!
//! ⚠️ **`f32`, et non un type « dixièmes » dédié.** L'issue laisse le choix (« un type ou un `f32`
//! validé à la lecture ») ; il faut en fixer un pour que les tests compilent, et le `f32` est celui
//! qui ne fait apparaître aucun nom nouveau dans l'API publique. Une valeur du domaine s'écrit
//! donc `0.7`, `3.0` dans un littéral, et elle est **exactement** le `f32` le plus proche du texte
//! écrit (`"0.7".parse::<f32>()`), ce qui rend les comparaisons par `==` légitimes ici : on ne
//! compare jamais un résultat de calcul, seulement une valeur lue à une valeur écrite.
//!
//! L'arithmétique entière en dixièmes que l'approche technique décrit pour `temps()` et `derive()`
//! reste une affaire d'implémentation : ces tests n'observent que les images.
//!
//! # Ce que ce fichier vérifie
//!
//! | critère de l'issue | test |
//! |---|---|
//! | n° 1 : pas `2n` à 0.5 = pas `n` à 1 | [`a_la_moitie_de_la_vitesse_le_pas_double_rend_l_image_du_pas_simple`] |
//! | n° 2 : vitesses entières, images d'avant à l'octet près | [`les_vitesses_entieres_rendent_les_images_d_avant_a_l_octet_pres`] |
//! | n° 3 : refus, avec la plage `0.1 à 10` | [`une_vitesse_hors_plage_ou_mal_ecrite_est_refusee_en_citant_la_plage`] |
//! | n° 4 : écriture `0.7` → `0.7`, `3` → `3` (côté catalogue) | [`une_vitesse_s_ecrit_sans_decimale_inutile_et_se_relit_a_l_identique`] |
//! | n° 5 : les familles sans cycle acceptent les décimales et ralentissent en proportion | [`toutes_les_familles_acceptent_les_vitesses_au_dixieme`] et le test n° 1, qui les couvre toutes |
//!
//! L'aller-retour par un profil et par `eclairage.conf` vit dans
//! `crates/reverb-daemon/tests/spec_149_vitesse.rs` ; la fenêtre, dans
//! `crates/reverb-gui/tests/spec_149_vitesse.rs`.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi.

use reverb_anim::{Animation, CATALOGUE, Direction, Geometrie, Image, Palette, Reglages};
use reverb_proto::{Position, Rgb};

// ---------------------------------------------------------------------------
// Le domaine
// ---------------------------------------------------------------------------

/// Les cinq familles sans cycle que l'issue nomme : elles doivent « accepter aussi les vitesses
/// décimales et ralentir en proportion ».
const SANS_CYCLE: [&str; 5] = ["scintillement", "bougie", "nuee", "artifice", "braise"];

/// Une couleur dont les trois composantes diffèrent deux à deux, comme dans `spec_148_schema_face.rs`.
const TEMOIN: Rgb = Rgb::new(0xff, 0x20, 0x80);

/// Le nom de sonde donné à `thermique`, qui en exige un. Sa valeur n'entre pas dans l'image.
const SONDE: &str = "pompe";

/// Les pas échantillonnés.
///
/// - Les petits nombres et les bords de la période de 120 pas (`spec_sens.rs`, #49) : c'est là
///   qu'un arrondi décalé d'un pas se voit d'abord.
/// - Des pas **au-delà de 1021**, où une graine de bruit ou un modulo peut changer de régime.
/// - Des pas **très grands**, qu'un démon atteint en tournant : 25 920 000 est dix jours à 30
///   images/s, 100 000 007 un peu plus d'un mois. 429 496 729 et 429 496 730 encadrent
///   `u32::MAX / 10` — le seuil où un pas compté en **dixièmes** sur 32 bits déborde —, et
///   `u32::MAX` est le dernier pas du compteur. Le code d'avant les rend sans paniquer (vérifié à
///   `d6bbc62`) ; le critère n° 2 dit « identiques », sans borne sur le pas.
const PAS: [u32; 32] = [
    0,
    1,
    2,
    3,
    5,
    7,
    11,
    17,
    29,
    41,
    59,
    60,
    61,
    89,
    119,
    120,
    121,
    240,
    901,
    1021,
    1022,
    1023,
    2042,
    4093,
    10_007,
    65_537,
    1_000_003,
    25_920_000,
    100_000_007,
    429_496_729,
    429_496_730,
    u32::MAX,
];

// ---------------------------------------------------------------------------
// Aides communes
// ---------------------------------------------------------------------------

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
}

fn paire(cle: &str, valeur: &str) -> (String, String) {
    (cle.to_owned(), valeur.to_owned())
}

/// Une seconde géométrie, sans orientation particulière, construite par le décodeur — celle de
/// `spec_148_schema_face.rs`.
fn geometrie_plate() -> Geometrie {
    let lignes: Vec<String> = Position::ALL
        .iter()
        .map(|position| format!("{} 0 horaire", position.slug()))
        .collect();
    Geometrie::decoder(&lignes.join("\n")).expect("la géométrie de test est valide")
}

/// Les familles du catalogue qui acceptent `vitesse`, avec un garde-fou : les cinq familles sans
/// cycle de l'issue et `vague` en font partie. Une famille qui perdrait la clé sortirait sinon
/// des tests en silence.
fn familles_a_vitesse() -> Vec<&'static str> {
    let familles: Vec<&'static str> = CATALOGUE
        .iter()
        .copied()
        .filter(|nom| animation(nom).parametres_acceptes().contains(&"vitesse"))
        .collect();
    for attendue in SANS_CYCLE.iter().chain(&["vague"]) {
        assert!(
            familles.contains(attendue),
            "appareil : « {attendue} » doit accepter `vitesse` — l'issue la nomme. Familles à \
             vitesse : {familles:?}"
        );
    }
    familles
}

/// La vitesse au dixième `dixiemes / 10`, comme une valeur écrite : `7` donne `0.7`.
///
/// La division de deux entiers exacts par l'IEEE 754 est correctement arrondie : le résultat est
/// le `f32` le plus proche de `0.7`, c'est-à-dire celui que rend `"0.7".parse::<f32>()`.
fn au_dixieme(dixiemes: u8) -> f32 {
    f32::from(dixiemes) / 10.0
}

/// Ce que le catalogue doit écrire pour cette vitesse : sans décimale si elle est entière, avec
/// **un seul** chiffre après le point sinon — `3`, `10`, `0.7`, `9.9`.
fn ecrite(dixiemes: u8) -> String {
    if dixiemes.is_multiple_of(10) {
        format!("{}", dixiemes / 10)
    } else {
        format!("{}.{}", dixiemes / 10, dixiemes % 10)
    }
}

/// Les réglages d'une famille à une vitesse donnée : couleur témoin, et pour `thermique` le nom de
/// la sonde qu'elle exige.
fn reglages(nom: &str, vitesse: f32, palette: Option<Palette>) -> Reglages {
    Reglages {
        couleur: TEMOIN,
        vitesse,
        sonde: (nom == "thermique").then(|| SONDE.to_owned()),
        palette,
        ..Reglages::default()
    }
}

// ---------------------------------------------------------------------------
// Critère n° 2 — les vitesses entières, à l'octet près
// ---------------------------------------------------------------------------

/// La vitesse entière `v` dans le type de #149.
fn vitesse_entiere(v: u8) -> f32 {
    f32::from(v)
}

/// L'empreinte d'une image : FNV-1a 64 bits sur les 372 octets, dans un ordre canonique — celle de
/// `spec_148_schema_face.rs`. Les ventilateurs sont parcourus dans l'ordre de `Position::ALL` et
/// cherchés **par position**.
fn avale_image(image: &Image, valeur: &mut u64) {
    let mut avale = |octet: u8| {
        *valeur ^= u64::from(octet);
        *valeur = valeur.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for position in Position::ALL {
        let (_, couleurs) = image
            .ventilateurs
            .iter()
            .find(|(portee, _)| *portee == position)
            .unwrap_or_else(|| panic!("l'image porte le ventilateur {position:?}"));
        for couleur in couleurs {
            avale(couleur.r);
            avale(couleur.g);
            avale(couleur.b);
        }
    }
    for barrette in &image.barrettes {
        for couleur in barrette {
            avale(couleur.r);
            avale(couleur.g);
            avale(couleur.b);
        }
    }
}

/// L'empreinte d'une famille aux dix vitesses entières : deux géométries, avec et sans palette,
/// trois directions si la famille en suit une (la direction par défaut, une globale, une locale),
/// les [`PAS`] — et, pour `thermique`, trois mesures de sonde.
fn empreinte(nom: &str) -> u64 {
    let famille = animation(nom);
    let directions: Vec<Direction> = if famille.parametres_acceptes().contains(&"direction") {
        vec![
            Reglages::default().direction,
            Direction::BasHaut,
            Direction::BordsCentre,
        ]
    } else {
        vec![Reglages::default().direction]
    };
    let mesures: &[Option<f32>] = if nom == "thermique" {
        &[None, Some(25.0), Some(55.0)]
    } else {
        &[None]
    };
    let sonde = (nom == "thermique").then(|| SONDE.to_owned());

    let mut valeur: u64 = 0xcbf2_9ce4_8422_2325;
    for geometrie in [Geometrie::mesuree(), geometrie_plate()] {
        for palette in [
            None,
            Some(Palette::par_nom("lava").expect("« lava » existe")),
        ] {
            for direction in &directions {
                for v in 1..=10u8 {
                    let reglages = Reglages {
                        couleur: TEMOIN,
                        vitesse: vitesse_entiere(v),
                        direction: *direction,
                        sonde: sonde.clone(),
                        palette,
                        // #150 : `largeur` rejoint `Reglages` ; 10 est son défaut (essai boîtier du 2026-10-31), sans effet hors `course`.
                        largeur: 10,
                    };
                    for mesure in mesures {
                        for pas in PAS {
                            let image =
                                famille.image_avec_mesure(&geometrie, &reglages, pas, *mesure);
                            avale_image(&image, &mut valeur);
                        }
                    }
                }
            }
        }
    }
    valeur
}

/// Les empreintes d'avant, relevées à la révision `d6bbc62` (branche de #148, dont #149 part),
/// **avant toute implémentation**, en appelant l'API publique (`Animation::image_avec_mesure`)
/// avec la vitesse encore en `u8` — la seule différence avec [`empreinte`] était le type.
///
/// L'issue dit « référence prise sur `main` » ; #149 part de la branche de #148, qui n'est pas
/// encore fusionnée et change les directions globales. La référence est donc prise sur le code de
/// départ de cette branche : c'est lui que #149 ne doit pas faire bouger.
///
/// ⚠️ **Une empreinte qui bouge n'est jamais une empreinte à corriger.** #149 ajoute des vitesses
/// sous 1 et entre les entiers ; si l'une de ces valeurs diffère après implémentation, c'est le
/// code qu'on corrige.
const EMPREINTES: [(&str, u64); 13] = [
    ("vague", 0x275c_38b1_2eb0_1b85),
    ("comete", 0x807f_d14c_66a8_2d61),
    ("respiration", 0x2f62_f53a_0774_c137),
    ("arc-en-ciel", 0x32e4_4640_6070_ddf5),
    ("balayage", 0xb2e4_0aef_1215_4296),
    ("braise", 0x6fcb_cd77_4817_3215),
    ("rotation", 0x9b38_1f2c_0ca3_6496),
    ("thermique", 0x00ed_4dca_5ed0_c085),
    ("pouls", 0x28d3_9fe3_aea7_ad77),
    ("scintillement", 0x7a49_7c72_fc73_ba85),
    ("bougie", 0xd1c2_d515_feea_5df5),
    ("nuee", 0x61a2_24b8_6756_8083),
    ("artifice", 0xc85b_bf8b_89c0_d352),
];

#[test]
fn les_vitesses_entieres_rendent_les_images_d_avant_a_l_octet_pres() {
    // Critère d'acceptation n° 2 : « pour les vitesses entières de 1 à 10, les images de toutes les
    // familles sont identiques à l'octet près à celles d'avant ». C'est ce qui garde lisibles, et
    // affichés à l'identique, les profils, `eclairage.conf` et les zones écrits avant.
    //
    // Toutes les empreintes fautives sont listées d'un coup : savoir si une seule famille a bougé
    // ou toutes à la fois dit déjà où chercher (une famille seule : son calcul ; toutes : `temps`).
    assert_eq!(
        EMPREINTES.iter().map(|(nom, _)| *nom).collect::<Vec<_>>(),
        // #150 : le catalogue commence par ces treize, dans l'ordre ; `course`, ajoutée après,
        // n'a pas d'image d'avant à garder.
        CATALOGUE[..EMPREINTES.len()].to_vec(),
        "appareil : la table couvre les treize premières familles du catalogue, dans son ordre"
    );

    let mut fautes = Vec::new();
    for (nom, attendue) in EMPREINTES {
        let obtenue = empreinte(nom);
        if obtenue != attendue {
            fautes.push(format!(
                "(\"{nom}\") : 0x{obtenue:016x} au lieu de 0x{attendue:016x}"
            ));
        }
    }
    assert!(
        fautes.is_empty(),
        "{} famille(s) ne rendent plus les images de d6bbc62 aux vitesses entières — #149 ne doit \
         rien changer de 1 à 10 : {}",
        fautes.len(),
        fautes.join(" ; ")
    );
}

// ---------------------------------------------------------------------------
// Critères n° 1 et 5 — la durée d'un cycle est inversement proportionnelle à la vitesse
// ---------------------------------------------------------------------------

/// Les couples (vitesse lente, vitesse double) comparés.
///
/// `0.5` et `1` sont ceux de l'issue. `1.5` et `3` posent le même critère entre deux vitesses non
/// entières et entières, à la vitesse par défaut : une implémentation qui ne traiterait que le cas
/// `0.5` à part le passerait seule. Les deux couples ne demandent que des produits exacts (`2n ×
/// 0.5 = n`, `2n × 1.5 = 3n`), justes quelle que soit l'arithmétique retenue.
const COUPLES: [(f32, f32); 2] = [(0.5, 1.0), (1.5, 3.0)];

/// Les `n` comparés : le pas `2n` à la vitesse lente contre le pas `n` à la vitesse double. Des
/// petits, des bords de période et des grands — le plus grand reste sous `u32::MAX / 2`.
const N: [u32; 16] = [
    0, 1, 2, 3, 7, 29, 59, 60, 61, 119, 120, 511, 1021, 1500, 65_537, 50_000_017,
];

#[test]
fn a_la_moitie_de_la_vitesse_le_pas_double_rend_l_image_du_pas_simple() {
    // Critère d'acceptation n° 1 : « Sur un même nombre de pas, la LED parcourt la moitié du chemin
    // qu'elle parcourt à `vitesse=1`. Formulé en images : l'image au pas `2n` à `0.5` est
    // identique à l'image au pas `n` à `1`. » Et n° 5 : les familles sans cycle « ralentissent en
    // proportion » — elles n'ont pas de chemin à parcourir, mais leur suite d'images en a un, et
    // c'est la même égalité qui le dit.
    //
    // Toutes les familles qui acceptent la vitesse y passent, sous deux géométries et avec ou sans
    // palette : une famille oubliée par le passage aux dixièmes tournerait à la mauvaise allure
    // sans un message.
    let palettes = [
        None,
        Some(Palette::par_nom("lava").expect("« lava » existe")),
    ];
    let mut fautes = Vec::new();
    for nom in familles_a_vitesse() {
        let famille = animation(nom);

        // Appareil : la famille bouge d'un pas à l'autre à vitesse 1. Une famille figée dans le
        // temps passerait l'égalité sans rien prouver.
        let un = reglages(nom, 1.0, None);
        let geometrie = Geometrie::mesuree();
        assert!(
            N.iter().any(
                |&n| famille.image(&geometrie, &un, 2 * n) != famille.image(&geometrie, &un, n)
            ),
            "appareil : « {nom} » rend la même image aux pas n et 2n à la vitesse 1 — l'égalité \
             ne prouverait rien"
        );

        for geometrie in [Geometrie::mesuree(), geometrie_plate()] {
            for palette in palettes {
                for (lente, double) in COUPLES {
                    let reglages_lents = reglages(nom, lente, palette);
                    let reglages_doubles = reglages(nom, double, palette);
                    for n in N {
                        let a_la_lente = famille.image(&geometrie, &reglages_lents, 2 * n);
                        let a_la_double = famille.image(&geometrie, &reglages_doubles, n);
                        if a_la_lente != a_la_double {
                            fautes.push(format!(
                                "« {nom} » (palette {:?}) : pas {} à {lente} ≠ pas {n} à {double}",
                                palette.map(|p| p.nom()),
                                2 * n
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        fautes.is_empty(),
        "{} image(s) ne ralentissent pas en proportion — à la moitié de la vitesse, un cycle dure \
         deux fois plus longtemps : {}",
        fautes.len(),
        fautes.join(" ; ")
    );
}

// ---------------------------------------------------------------------------
// Critère n° 5 — toutes les familles acceptent les dixièmes
// ---------------------------------------------------------------------------

#[test]
fn toutes_les_familles_acceptent_les_vitesses_au_dixieme() {
    // Comportement attendu : « `vitesse` accepte les valeurs décimales avec un chiffre après la
    // virgule, de 0.1 à 10, écrites avec un point ». Critère n° 1 : « `animate vague vitesse=0.5`
    // est accepté ». Critère n° 5 : les familles sans cycle « acceptent aussi les vitesses
    // décimales ».
    //
    // Les cent valeurs du domaine, sur chaque famille qui accepte la vitesse : les deux bornes
    // (0.1 et 10), les entiers d'avant, et tous les dixièmes entre eux.
    for nom in familles_a_vitesse() {
        let famille = animation(nom);
        for dixiemes in 1..=100u8 {
            let texte = ecrite(dixiemes);
            let mut paires = vec![paire("vitesse", &texte)];
            if nom == "thermique" {
                paires.push(paire("sonde", SONDE));
            }
            let lus = famille.reglages(&paires).unwrap_or_else(|erreur| {
                panic!("« {nom} » doit accepter « vitesse={texte} » : {erreur}")
            });
            assert_eq!(
                lus.vitesse,
                au_dixieme(dixiemes),
                "« {nom} » : « vitesse={texte} » doit atterrir dans le champ `vitesse` tel qu'écrit"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Critère n° 3 — les refus, avec la plage
// ---------------------------------------------------------------------------

/// Les vitesses que l'issue refuse, chacune pour sa raison.
const REFUSEES: [(&str, &str); 9] = [
    ("0", "zéro"),
    ("0.0", "zéro, écrit en décimal"),
    ("-1", "un négatif"),
    ("-0.5", "un négatif au dixième"),
    ("10.5", "plus de 10"),
    ("10.1", "plus de 10, au dixième près"),
    ("0.75", "plus d'un chiffre après la virgule"),
    ("0,7", "la virgule française"),
    ("abc", "pas un nombre"),
];

/// La plage que le message doit citer, telle que l'issue l'écrit.
const PLAGE: &str = "0.1 à 10";

#[test]
fn une_vitesse_hors_plage_ou_mal_ecrite_est_refusee_en_citant_la_plage() {
    // Critère d'acceptation n° 3 : « les refus listés plus haut, chacun avec un message qui cite la
    // plage `0.1 à 10` ». La liste est celle de l'issue — `0`, les négatifs, plus de 10, plus d'un
    // chiffre après la virgule (`0.75`), la virgule française (`0,7`), tout ce qui n'est pas un
    // nombre —, sur chaque famille qui accepte la vitesse.
    //
    // La virgule est le piège : `0,7` est ce que tape un francophone, et un décodeur qui le
    // lirait `0` ou `7` lancerait une animation à une vitesse que personne n'a demandée. D'où le
    // message, qui doit dire la plage **et** l'écriture attendue par l'exemple de sa borne basse.
    for nom in familles_a_vitesse() {
        let famille = animation(nom);
        for (valeur, pourquoi) in REFUSEES {
            let mut paires = vec![paire("vitesse", valeur)];
            if nom == "thermique" {
                paires.push(paire("sonde", SONDE));
            }
            let Err(erreur) = famille.reglages(&paires) else {
                panic!(
                    "« {nom} » a accepté « vitesse={valeur} » ({pourquoi}) : elle doit la refuser"
                );
            };
            assert_eq!(
                erreur.cle, "vitesse",
                "« {nom} » : « {valeur} » est fautive au titre de « vitesse » — {erreur}"
            );
            let message = erreur.to_string();
            assert!(
                message.contains(PLAGE),
                "« {nom} » : le refus de « vitesse={valeur} » ({pourquoi}) doit citer la plage \
                 « {PLAGE} » — reçu « {message} »"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Critère n° 4 — l'écriture, et l'aller-retour
// ---------------------------------------------------------------------------

#[test]
fn une_vitesse_s_ecrit_sans_decimale_inutile_et_se_relit_a_l_identique() {
    // Comportement attendu : « une vitesse entière s'écrit sans décimale (`3`, et non `3.0`), une
    // vitesse décimale avec un seul chiffre après le point (`0.7`) ». Critère n° 4 : « aller-retour
    // d'écriture : `0.7` → `0.7`, `3` → `3` ». `reglages_ecrits` est l'émetteur de `lighting`, des
    // profils et d'`eclairage.conf` : c'est ici que l'écriture se juge une fois pour les trois.
    //
    // Les cent valeurs du domaine : `3.0` écrit `3.0` relirait pareil, mais un profil d'avant
    // relu puis réécrit changerait d'octets sans que rien ait changé.
    for nom in familles_a_vitesse() {
        let famille = animation(nom);
        for dixiemes in 1..=100u8 {
            let posee = reglages(nom, au_dixieme(dixiemes), None);
            let ecrits = famille.reglages_ecrits(&posee);
            let valeurs: Vec<&str> = ecrits
                .iter()
                .filter(|(cle, _)| cle == "vitesse")
                .map(|(_, valeur)| valeur.as_str())
                .collect();
            assert_eq!(
                valeurs,
                vec![ecrite(dixiemes).as_str()],
                "« {nom} » : la vitesse {} s'écrit « {} », une seule fois — {ecrits:?}",
                au_dixieme(dixiemes),
                ecrite(dixiemes)
            );

            let relus = famille.reglages(&ecrits).unwrap_or_else(|erreur| {
                panic!("« {nom} » doit relire ce qu'il écrit, {ecrits:?} : {erreur}")
            });
            assert_eq!(
                relus.vitesse, posee.vitesse,
                "« {nom} » : la vitesse {} ne survit pas à l'aller-retour — {ecrits:?}",
                posee.vitesse
            );
        }
    }
}
