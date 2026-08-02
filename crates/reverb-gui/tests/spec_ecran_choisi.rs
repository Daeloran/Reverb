//! Tests d'intention du menu d'affichage de l'écran (issue #48).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #48 seule. Rien n'a été lu dans
//! `crates/reverb-gui/src/` — ni corps de fonction, ni module : seules les **signatures**
//! publiques de `reglages.rs` ont été relevées, pour savoir à côté de quoi la nouvelle API vient
//! s'asseoir. Aucun test ici n'ouvre de socket, ne démarre de démon ni ne touche un bus : un relevé
//! du démon est une [`ResponseLine::Screen`], et c'est tout ce dont `EcranChoisi` a besoin.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Remarque de Nico, 2026-08-02 : « on peut sélectionner un truc dans le menu mais il revient sur
//! "rien" en quelques instants ». La fenêtre demande `screen state` à chaque tour de son horloge
//! d'une seconde et **réécrit** le rang du menu et le champ d'argument depuis la réponse. Tant que
//! « Appliquer » n'a pas été cliqué, le démon répond encore `rien` : la sélection en cours est
//! effacée sous les doigts, avec le chemin ou le nom de sonde qu'on était en train de taper.
//!
//! C'est le défaut déjà réparé sur les poignées de ventilateur, puis sur l'animation (#41), revenu
//! par une troisième porte. La règle est la même à chaque fois : **une valeur que l'utilisateur
//! compose ne se laisse pas écraser par un relevé.**
//!
//! ## Le contrat que ces tests figent
//!
//! ```ignore
//! pub struct EcranChoisi {
//!     pub affichage: usize,   // le rang dans le menu, toujours dans les bornes
//!     pub argument: String,   // le chemin de fichier ou le nom de sonde
//!     pub luminosite: u8,     // en pour cent — jamais retenue par la poignée
//!     // la poignée est privée : on la lève et on la baisse, on ne l'écrit pas
//! }
//!
//! impl EcranChoisi {
//!     pub fn saisir(&mut self);                                    // l'utilisateur compose
//!     pub fn relacher(&mut self);                                  // « Appliquer » ou « Annuler »
//!     pub fn compose(&self) -> bool;                               // la poignée est-elle levée
//!     pub fn adopter(&mut self, luminosite: u8, affichage: &str) -> bool;
//! }
//! ```
//!
//! Le menu porte **quatre** affichages, dans cet ordre : `rien`, `cadran`, `image`, `gif` — soit
//! les rangs 0 à 3. `adopter` rend `true` si l'un des trois champs a changé de valeur, ce qui est
//! aussi ce qui dit à la fenêtre s'il y a lieu de repeindre.
//!
//! ## Ce que le contrat laissait ouvert, et que ces tests tranchent
//!
//! 1. **`gauge` et `cadran` désignent le même affichage.** L'issue écrit son quatrième test
//!    d'intention avec `cadran:kraken2023elite:coolant`, et c'est aussi le mot du menu ; mais ce
//!    qui traverse vraiment le socket est `gauge:` — `crates/reverb-daemon/src/main.rs:346` écrit
//!    `format!("gauge:{sonde}")`, et la documentation de [`ResponseLine::Screen`] dit « `rien`,
//!    `gauge:<sonde>`, `image:<chemin>` ou `gif:<chemin>` ». Les deux orthographes sont donc
//!    exigées sur le rang du cadran : refuser `gauge` laisserait le défaut de #48 entier sur le
//!    seul affichage que la ligne de commande sait poser (`reverb screen --gauge …`), et refuser
//!    `cadran` contredirait le test d'intention n° 4 de l'issue. **C'est le seul endroit où ce
//!    fichier a dû arbitrer entre l'issue et le protocole.**
//! 2. **Un affichage inconnu retombe sur le rang 0, `rien`, et vide le champ.** L'issue exige
//!    seulement qu'il « ne fasse pas sortir le rang des bornes » et laisse le repli ouvert. `rien`
//!    est le seul des quatre rangs dont le sens reste vrai quand la fenêtre ne sait pas nommer ce
//!    que le démon affiche — « je ne pilote pas la dalle ». Et le champ se vide, parce qu'un
//!    argument orphelin sous un menu qui dit « rien » serait un chemin que rien n'explique.
//! 3. **`adopter` ne relâche jamais la poignée.** Sans cette règle, un relevé levé suffirait à
//!    rouvrir la porte au suivant, et le défaut reviendrait avec une seconde de retard. Seul
//!    [`EcranChoisi::relacher`] baisse la poignée — c'est-à-dire « Appliquer » ou « Annuler ».
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Le message d'erreur d'un refus.** L'issue demande qu'un refus du démon soit « signalé à
//!   l'utilisateur, pas seulement ravalé » : c'est une ligne `err` affichée par la fenêtre, pas une
//!   règle de `reglages.rs`. Ce que ce fichier fige, c'est que le refus **redevient visible dans le
//!   menu** — l'affichage choisi laisse la place à celui que le démon dit vraiment.
//! - **Ce que la fenêtre envoie** quand on clique « Appliquer ». `adopter` est le sens
//!   démon → fenêtre ; la commande est déjà couverte par le protocole (`spec_ipc_ecran.rs`).
//! - **Un affichage `rien` portant malgré tout un argument** (`rien:quelque-chose`). Le démon ne
//!   l'écrit jamais, et l'issue ne s'en préoccupe pas : trancher ici inventerait une règle que
//!   personne n'a choisie.
//! - **Les bornes de la luminosité.** Elles appartiennent au protocole, qui les valide déjà.

use reverb_gui::reglages::EcranChoisi;
use reverb_proto::ipc::{ResponseLine, parse_response_line};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// Les quatre rangs du menu, dans l'ordre où il les montre.
const RANG_RIEN: usize = 0;
const RANG_CADRAN: usize = 1;
const RANG_IMAGE: usize = 2;
const RANG_GIF: usize = 3;
/// Le nombre d'entrées du menu : aucun rang adopté ne doit l'atteindre.
const RANGS: usize = 4;

/// La sonde du cadran — **et elle porte un deux-points**, c'est tout l'objet du test n° 4.
const SONDE: &str = "kraken2023elite:coolant";
/// Un chemin d'image qui en porte un aussi : rien n'interdit un deux-points dans un nom de
/// fichier sous Linux, et un découpage sur le **dernier** le couperait en deux.
const CHEMIN_IMAGE: &str = "/home/nico/photos/vue:sud.png";
/// Un chemin de GIF, sans deux-points cette fois : le cas ordinaire doit marcher aussi.
const CHEMIN_GIF: &str = "/home/nico/pluie.gif";
/// Un chemin qui porte des espaces, **dont un tout à la fin**.
///
/// Le protocole les conserve exprès — « l'affichage est le dernier champ, et porte un chemin : ses
/// espaces restent » (`crates/reverb-proto/src/ipc.rs`), et `sans_controle` ne rogne rien. Un nom de
/// fichier créé par mégarde avec un blanc final existe pour de bon sous Linux, et c'est justement le
/// cas où un `trim()` bien intentionné fait échouer « Appliquer » sans rien expliquer.
const CHEMIN_ESPACES: &str = "/home/nico/mes photos/vue du sud.png ";

/// La luminosité que le démon rapporte, celle que la fenêtre affichait, et une troisième.
///
/// Trois valeurs distinctes : une luminosité perdue en route se confondrait sinon avec une
/// luminosité bien adoptée.
const LUMINOSITE_DEMON: u8 = 40;
const LUMINOSITE_FENETRE: u8 = 75;
const LUMINOSITE_AUTRE: u8 = 12;

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

/// Un relevé tel que le démon l'écrit en réponse à `screen state`.
fn releve(luminosite: u8, affichage: &str) -> ResponseLine {
    ResponseLine::Screen {
        luminosite,
        affichage: affichage.to_owned(),
    }
}

/// Adopte un relevé, en passant par la ligne du protocole plutôt que par deux valeurs nues.
///
/// C'est ce qui rend ces tests sensibles au **type** que la fenêtre reçoit vraiment : le jour où
/// `ResponseLine::Screen` changerait de forme, ce fichier tomberait au lieu de continuer à
/// vérifier une API qui ne se branche plus sur rien.
fn adopte(choisi: &mut EcranChoisi, ligne: &ResponseLine) -> bool {
    let ResponseLine::Screen {
        luminosite,
        affichage,
    } = ligne
    else {
        panic!("`releve` rend une ligne `screen`, pas {ligne:?}");
    };
    choisi.adopter(*luminosite, affichage)
}

/// Les trois champs visibles, pour les comparer d'un coup.
fn etat(choisi: &EcranChoisi) -> (usize, String, u8) {
    (choisi.affichage, choisi.argument.clone(), choisi.luminosite)
}

/// Une fenêtre au repos : elle montre ceci, et l'utilisateur ne compose rien.
fn au_repos(affichage: usize, argument: &str, luminosite: u8) -> EcranChoisi {
    let mut choisi = EcranChoisi::default();
    choisi.affichage = affichage;
    choisi.argument = argument.to_owned();
    choisi.luminosite = luminosite;
    choisi
}

/// Une fenêtre dont l'utilisateur compose l'affichage : la poignée est levée.
fn en_cours(affichage: usize, argument: &str, luminosite: u8) -> EcranChoisi {
    let mut choisi = au_repos(affichage, argument, luminosite);
    choisi.saisir();
    choisi
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Garde-fou, pas critère. Trois confusions rendraient tous les tests qui suivent complaisants :
    // une sonde sans deux-points — et le test n° 4 ne testerait plus rien —, deux luminosités
    // égales — et une luminosité perdue passerait pour adoptée —, et surtout une fenêtre neuve
    // dont la poignée serait **déjà levée**, qui ferait passer le test n° 1 par accident.
    assert!(
        SONDE.matches(':').count() >= 1,
        "la sonde de ce fichier doit porter un deux-points, sinon le test n° 4 ne teste rien"
    );
    assert!(
        CHEMIN_IMAGE.contains(':'),
        "le chemin d'image aussi : c'est lui qui attrape un découpage sur le dernier deux-points"
    );
    assert!(
        !CHEMIN_GIF.contains(':'),
        "le chemin de GIF, lui, n'en porte pas : le cas ordinaire doit marcher également"
    );
    assert_ne!(
        CHEMIN_ESPACES,
        CHEMIN_ESPACES.trim(),
        "le chemin à espaces doit en porter aux extrémités de son dernier segment, sinon il \
         n'attrape aucun rognage"
    );

    let luminosites = [LUMINOSITE_DEMON, LUMINOSITE_FENETRE, LUMINOSITE_AUTRE];
    for (i, luminosite) in luminosites.iter().enumerate() {
        assert!(
            !luminosites[i + 1..].contains(luminosite),
            "les trois luminosités de ce fichier doivent être distinctes : {luminosite} revient"
        );
    }

    let rangs = [RANG_RIEN, RANG_CADRAN, RANG_IMAGE, RANG_GIF];
    for (attendu, rang) in rangs.iter().enumerate() {
        assert_eq!(
            *rang, attendu,
            "le menu montre « rien », « cadran », « image », « gif » dans cet ordre"
        );
    }
    assert_eq!(RANGS, rangs.len(), "et il en montre quatre, pas davantage");

    let neuve = EcranChoisi::default();
    assert!(
        !neuve.compose(),
        "une fenêtre qui vient de s'ouvrir ne compose rien : la poignée est baissée — {neuve:?}"
    );
    assert_eq!(
        etat(&neuve),
        (RANG_RIEN, String::new(), 0),
        "et elle ne montre rien, sans argument — {neuve:?}"
    );
}

// ---------------------------------------------------------------------------
// 1 — poignée baissée : le relevé est adopté
// ---------------------------------------------------------------------------

#[test]
fn poignee_baissee_le_releve_du_demon_est_adopte() {
    // Test d'intention n° 1 de l'issue : « poignée baissée + relevé du démon → le relevé est
    // adopté ». C'est le comportement d'aujourd'hui, celui qu'il ne faut surtout pas casser en
    // réparant le reste : sans lui, `reverb screen --gauge …` tapé dans un terminal ne remonterait
    // plus jamais dans la fenêtre.
    //
    // Les quatre affichages, chacun depuis une fenêtre qui montrait autre chose : une
    // implémentation qui écrirait le rang sans écrire l'argument — ou l'inverse — passe n'importe
    // quel test qui n'en vérifie qu'un.
    for (mot, rang, argument) in [
        ("rien", RANG_RIEN, ""),
        (&format!("gauge:{SONDE}"), RANG_CADRAN, SONDE),
        (&format!("image:{CHEMIN_IMAGE}"), RANG_IMAGE, CHEMIN_IMAGE),
        (&format!("gif:{CHEMIN_GIF}"), RANG_GIF, CHEMIN_GIF),
    ] {
        let mut fenetre = au_repos(RANG_IMAGE, "/un/autre/chemin.png", LUMINOSITE_FENETRE);
        let bouge = adopte(&mut fenetre, &releve(LUMINOSITE_DEMON, mot));
        assert!(
            bouge,
            "« {mot} » n'est pas ce que la fenêtre montrait : c'est un changement"
        );
        assert_eq!(
            etat(&fenetre),
            (rang, argument.to_owned(), LUMINOSITE_DEMON),
            "« {mot} » se lit au rang {rang} avec « {argument} » — {fenetre:?}"
        );
        assert!(
            fenetre.affichage < RANGS,
            "et le rang reste dans les bornes du menu — {fenetre:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — poignée levée : rien n'est adopté, et le choix tient
// ---------------------------------------------------------------------------

#[test]
fn poignee_levee_le_releve_n_est_pas_adopte_et_le_choix_en_cours_tient() {
    // Test d'intention n° 2 de l'issue, et critère d'acceptation n° 1 : « sélectionner un affichage
    // puis attendre plusieurs tours d'horloge : la sélection et le champ d'argument sont
    // inchangés ».
    //
    // C'est le défaut de #48 en une phrase. Le relevé qui l'a causé est reproduit tel quel — le
    // démon dit encore `rien`, parce que « Appliquer » n'a pas été cliqué — et il est répété cinq
    // fois, comme l'horloge le ferait pendant qu'on tape un chemin.
    //
    // La seconde faute visée est plus discrète : `adopter` qui **relâcherait** la poignée. Le
    // premier relevé serait alors bien refusé, et le second passerait — le défaut reviendrait avec
    // une seconde de retard, ce qui est exactement ce que Nico décrit par « en quelques instants ».
    let compose = "/home/nico/pluie";
    let mut fenetre = en_cours(RANG_GIF, compose, LUMINOSITE_DEMON);

    for tour in 1..=5 {
        let bouge = adopte(&mut fenetre, &releve(LUMINOSITE_DEMON, "rien"));
        assert!(
            !bouge,
            "tour {tour} : le démon ne dit rien de neuf et l'utilisateur compose — rien ne bouge"
        );
        assert_eq!(
            etat(&fenetre),
            (RANG_GIF, compose.to_owned(), LUMINOSITE_DEMON),
            "tour {tour} : la sélection et le chemin à demi tapé sont intacts — {fenetre:?}"
        );
        assert!(
            fenetre.compose(),
            "tour {tour} : adopter ne relâche jamais la poignée — {fenetre:?}"
        );
    }

    // Et un relevé qui contredit franchement le choix en cours ne passe pas davantage.
    adopte(
        &mut fenetre,
        &releve(LUMINOSITE_DEMON, &format!("gauge:{SONDE}")),
    );
    assert_eq!(
        etat(&fenetre),
        (RANG_GIF, compose.to_owned(), LUMINOSITE_DEMON),
        "un cadran venu d'ailleurs n'écrase pas non plus un GIF qu'on est en train de composer — \
         {fenetre:?}"
    );

    // Y compris `rien`, dont l'adoption viderait le champ : c'est la forme la plus coûteuse du
    // défaut, celle qui efface le chemin qu'on tapait.
    adopte(&mut fenetre, &releve(LUMINOSITE_DEMON, "rien"));
    assert_eq!(
        fenetre.argument, compose,
        "poignée levée, « rien » ne vide pas le champ : c'est le chemin qu'on tape — {fenetre:?}"
    );
}

// ---------------------------------------------------------------------------
// 3 — la poignée relâchée, le relevé suivant est adopté
// ---------------------------------------------------------------------------

#[test]
fn poignee_levee_puis_relachee_le_releve_suivant_est_adopte() {
    // Test d'intention n° 3 de l'issue, et critère d'acceptation n° 3 : « après "Appliquer", le
    // prochain relevé est adopté — y compris quand il contredit ce qui était choisi, ce qui est le
    // cas d'un refus ».
    //
    // La faute visée est symétrique de celle du test n° 2 : une poignée qu'on ne baisserait jamais
    // ferait une fenêtre qui ne suit plus rien du tout après la première sélection. Le défaut serait
    // moins voyant — et pire, parce qu'il durerait jusqu'à la fermeture de la fenêtre.
    let mut fenetre = en_cours(RANG_CADRAN, SONDE, LUMINOSITE_FENETRE);

    assert!(
        !adopte(&mut fenetre, &releve(LUMINOSITE_FENETRE, "rien")),
        "tant qu'on compose, ce relevé ne dit rien à la fenêtre"
    );
    assert_eq!(
        etat(&fenetre),
        (RANG_CADRAN, SONDE.to_owned(), LUMINOSITE_FENETRE),
        "le choix tient — {fenetre:?}"
    );

    fenetre.relacher();
    assert!(
        !fenetre.compose(),
        "« Appliquer » baisse la poignée — {fenetre:?}"
    );

    assert!(
        adopte(&mut fenetre, &releve(LUMINOSITE_FENETRE, "rien")),
        "le même relevé, poignée baissée, est un changement"
    );
    assert_eq!(
        etat(&fenetre),
        (RANG_RIEN, String::new(), LUMINOSITE_FENETRE),
        "et il s'adopte tel quel, même quand il contredit ce qui venait d'être choisi — {fenetre:?}"
    );

    // Lever la poignée de nouveau suspend de nouveau : la porte n'est pas restée ouverte.
    fenetre.affichage = RANG_IMAGE;
    fenetre.argument = CHEMIN_IMAGE.to_owned();
    fenetre.saisir();
    adopte(&mut fenetre, &releve(LUMINOSITE_FENETRE, "rien"));
    assert_eq!(
        etat(&fenetre),
        (RANG_IMAGE, CHEMIN_IMAGE.to_owned(), LUMINOSITE_FENETRE),
        "une seconde composition est protégée comme la première — {fenetre:?}"
    );

    // Et les deux gestes sont idempotents : deux clics sur « Appliquer », deux frappes dans le
    // champ, ne comptent pas double.
    fenetre.saisir();
    assert!(
        fenetre.compose(),
        "saisir deux fois lève toujours — {fenetre:?}"
    );
    fenetre.relacher();
    fenetre.relacher();
    assert!(
        !fenetre.compose(),
        "relâcher deux fois baisse toujours — {fenetre:?}"
    );
}

// ---------------------------------------------------------------------------
// 4 — le relevé se coupe au PREMIER deux-points
// ---------------------------------------------------------------------------

#[test]
fn le_releve_se_coupe_au_premier_deux_points_et_l_argument_garde_les_siens() {
    // Test d'intention n° 4 de l'issue : « le relevé adopté sépare bien
    // `cadran:kraken2023elite:coolant` en affichage et argument, y compris quand l'argument
    // contient lui-même des deux-points ».
    //
    // C'est la faute la plus silencieuse du lot. Un découpage sur le **dernier** deux-points rend
    // « coolant » au lieu de « kraken2023elite:coolant » : le champ se remplit d'un nom de sonde
    // presque juste, que le démon refusera au prochain « Appliquer » — et le refus paraîtra venir
    // de la sonde, pas de la fenêtre. Un `rsplit` est aussi court à écrire qu'un `split`.
    //
    // Les noms de sondes de ce projet portent tous un deux-points (`reverb fans`), et un chemin de
    // fichier a le droit d'en porter sous Linux : les deux cas sont couverts.
    for (mot, rang, argument) in [
        (format!("gauge:{SONDE}"), RANG_CADRAN, SONDE),
        // L'orthographe de l'issue. Voir le point n° 1 de l'en-tête : les deux mots désignent le
        // même affichage, parce que l'issue écrit l'un et que le démon écrit l'autre.
        (format!("cadran:{SONDE}"), RANG_CADRAN, SONDE),
        (
            String::from("gauge:k10temp:tctl"),
            RANG_CADRAN,
            "k10temp:tctl",
        ),
        (format!("image:{CHEMIN_IMAGE}"), RANG_IMAGE, CHEMIN_IMAGE),
        (String::from("gif:a:b:c:d"), RANG_GIF, "a:b:c:d"),
        // Les espaces d'un nom de fichier sont portés par le protocole **exprès** — « l'affichage
        // est le dernier champ, et porte un chemin : ses espaces restent »
        // (`encode_response_line`, `crates/reverb-proto/src/ipc.rs`). Un `trim()` bien intentionné
        // rendrait ici un chemin qui n'est pas celui de la dalle, et « Appliquer » irait chercher
        // un fichier qui n'existe pas.
        (
            format!("image:{CHEMIN_ESPACES}"),
            RANG_IMAGE,
            CHEMIN_ESPACES,
        ),
        // Sans deux-points du tout : l'argument est vide, et ce n'est pas une erreur.
        (String::from("image"), RANG_IMAGE, ""),
        (String::from("gif"), RANG_GIF, ""),
        (String::from("rien"), RANG_RIEN, ""),
    ] {
        let mut fenetre = au_repos(RANG_RIEN, "residu-a-effacer", LUMINOSITE_FENETRE);
        adopte(&mut fenetre, &releve(LUMINOSITE_DEMON, &mot));
        assert_eq!(
            etat(&fenetre),
            (rang, argument.to_owned(), LUMINOSITE_DEMON),
            "« {mot} » se coupe au premier deux-points — {fenetre:?}"
        );
        assert!(
            !fenetre.argument.starts_with(':'),
            "le deux-points de coupure n'appartient pas à l'argument — {fenetre:?}"
        );
    }

    // La ligne telle qu'elle arrive vraiment du socket, décodée par le protocole lui-même : ce test
    // est le seul du fichier sensible au **format** de la réponse, et donc au mot que le démon
    // emploie pour le cadran.
    let ligne = parse_response_line(&format!("screen {LUMINOSITE_DEMON} gauge:{SONDE}"))
        .expect("« screen <0-100> gauge:<sonde> » est une réponse valide du protocole");
    let mut fenetre = au_repos(RANG_GIF, CHEMIN_GIF, LUMINOSITE_FENETRE);
    adopte(&mut fenetre, &ligne);
    assert_eq!(
        etat(&fenetre),
        (RANG_CADRAN, SONDE.to_owned(), LUMINOSITE_DEMON),
        "ce que le démon écrit vraiment sur le socket se relit entier — {fenetre:?}"
    );

    // Et le chemin à espaces, lui aussi passé par le socket : c'est ce qui prouve que le protocole
    // le porte, et donc que la fenêtre n'a pas le droit de le rogner.
    let ligne = parse_response_line(&format!("screen {LUMINOSITE_DEMON} image:{CHEMIN_ESPACES}"))
        .expect("un chemin à espaces est une réponse valide : c'est le dernier champ de la ligne");
    let mut fenetre = au_repos(RANG_RIEN, "", LUMINOSITE_FENETRE);
    adopte(&mut fenetre, &ligne);
    assert_eq!(
        etat(&fenetre),
        (RANG_IMAGE, CHEMIN_ESPACES.to_owned(), LUMINOSITE_DEMON),
        "les espaces du nom de fichier arrivent intacts dans le champ — {fenetre:?}"
    );
}

// ---------------------------------------------------------------------------
// 5 — « rien » vide le champ d'argument
// ---------------------------------------------------------------------------

#[test]
fn l_affichage_rien_remet_le_champ_d_argument_a_vide() {
    // Test d'intention n° 5 de l'issue : « un affichage `rien` remet le champ d'argument à vide ».
    //
    // Le verbe est « remet » : le champ portait quelque chose, et il ne doit plus rien porter. La
    // faute visée est un `if let Some(argument) = …` bien intentionné qui n'écrirait le champ que
    // lorsqu'il y a un argument à écrire — et laisserait donc, sous un menu qui dit « rien », le
    // chemin de la dernière image affichée. L'utilisateur croirait qu'elle est encore là.
    for (venu_de, rang, argument) in [
        ("une image", RANG_IMAGE, CHEMIN_IMAGE),
        ("un cadran", RANG_CADRAN, SONDE),
        ("un GIF", RANG_GIF, CHEMIN_GIF),
    ] {
        let mut fenetre = au_repos(rang, argument, LUMINOSITE_FENETRE);
        adopte(&mut fenetre, &releve(LUMINOSITE_DEMON, "rien"));
        assert_eq!(
            etat(&fenetre),
            (RANG_RIEN, String::new(), LUMINOSITE_DEMON),
            "en venant d'{venu_de}, « rien » vide le champ — {fenetre:?}"
        );
        assert!(
            fenetre.argument.is_empty(),
            "et il est vraiment vide, pas rempli d'un blanc — {fenetre:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — un affichage inconnu ne fait pas sortir le rang des bornes
// ---------------------------------------------------------------------------

#[test]
fn un_affichage_inconnu_du_menu_retombe_sur_rien_sans_sortir_des_bornes() {
    // Test d'intention n° 6 de l'issue : « un affichage inconnu du menu ne fait pas sortir le rang
    // des bornes ». Le repli n'y est pas nommé ; ce test le nomme : **rang 0, `rien`, champ vide**
    // (voir le point n° 2 de l'en-tête).
    //
    // Le cas se produit dès qu'un démon plus récent que la fenêtre affiche quelque chose qu'elle ne
    // connaît pas. Un rang hors bornes envoyé à un menu Slint ne fait pas planter : il n'affiche
    // simplement plus rien, et le menu devient muet sans qu'aucun journal ne le dise.
    //
    // La seconde faute visée est un `starts_with` au lieu d'une égalité : `gauge2:x` tomberait alors
    // sur le cadran, et la fenêtre montrerait un affichage qui n'est pas celui de la dalle.
    for inconnu in [
        "video:/home/nico/film.mp4", // un affichage qu'un démon plus récent saurait faire
        "",                          // une réponse tronquée
        ":",                         // rien avant, rien après
        ":kraken2023elite:coolant",  // l'argument sans son affichage
        "gauge2:k10temp:tctl",       // un mot qui commence comme « gauge »
        "gaug:k10temp:tctl",         // un mot dont « gauge » commence comme lui
        "cadranx:k10temp:tctl",      // idem du côté de l'orthographe de l'issue
        "GAUGE:k10temp:tctl",        // la casse : le protocole écrit en minuscules
        "Image:/x.png",              // idem
        "IMAGE",                     // idem, sans argument
        "gif :/x.gif",               // un blanc avant le deux-points
        " gif:/x.gif",               // un blanc avant le mot
        "screen:/x.png",             // le verbe du protocole pris pour un affichage
    ] {
        let mut fenetre = au_repos(RANG_IMAGE, CHEMIN_IMAGE, LUMINOSITE_FENETRE);
        adopte(&mut fenetre, &releve(LUMINOSITE_DEMON, inconnu));
        assert!(
            fenetre.affichage < RANGS,
            "« {inconnu} » ne doit pas envoyer le menu hors de ses quatre entrées — {fenetre:?}"
        );
        assert_eq!(
            etat(&fenetre),
            (RANG_RIEN, String::new(), LUMINOSITE_DEMON),
            "« {inconnu} » n'est aucun des quatre affichages : le menu retombe sur « rien », le \
             champ se vide, et la luminosité est adoptée quand même — {fenetre:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 7 — la luminosité n'est jamais retenue par la poignée
// ---------------------------------------------------------------------------

#[test]
fn la_luminosite_est_adoptee_meme_quand_la_poignee_est_levee() {
    // Critère d'acceptation n° 5 de l'issue : « la luminosité n'est jamais retenue par ce
    // mécanisme : elle part à chaque cran ». Et son contexte : « la luminosité garde son
    // comportement actuel, qui est bon ».
    //
    // La faute visée est la plus naturelle de toutes : un `if choisi.compose() { return false; }` en
    // tête d'`adopter`, qui suspend le relevé **entier**. Le curseur de luminosité se figerait alors
    // dès qu'on touche au menu, et ne repartirait qu'après « Appliquer » — un second défaut
    // introduit en réparant le premier, et dans le même écran.
    let compose = "/home/nico/photos/en-cours";
    let mut fenetre = en_cours(RANG_IMAGE, compose, LUMINOSITE_FENETRE);

    assert!(
        adopte(&mut fenetre, &releve(LUMINOSITE_DEMON, "rien")),
        "la luminosité a changé : quelque chose a bougé, même si le menu n'a pas bronché"
    );
    assert_eq!(
        etat(&fenetre),
        (RANG_IMAGE, compose.to_owned(), LUMINOSITE_DEMON),
        "la luminosité passe, le choix en cours reste — {fenetre:?}"
    );

    adopte(
        &mut fenetre,
        &releve(LUMINOSITE_AUTRE, &format!("gauge:{SONDE}")),
    );
    assert_eq!(
        etat(&fenetre),
        (RANG_IMAGE, compose.to_owned(), LUMINOSITE_AUTRE),
        "et elle passe encore au relevé suivant — {fenetre:?}"
    );

    // Tous les crans, poignée levée : la luminosité suit le démon d'un bout à l'autre.
    for cran in 0..=100u8 {
        adopte(&mut fenetre, &releve(cran, "rien"));
        assert_eq!(
            fenetre.luminosite, cran,
            "le cran {cran} arrive dans la fenêtre malgré la poignée — {fenetre:?}"
        );
    }
    assert_eq!(
        (fenetre.affichage, fenetre.argument.as_str()),
        (RANG_IMAGE, compose),
        "et cent un relevés n'ont pas grignoté le choix en cours — {fenetre:?}"
    );
    assert!(
        fenetre.compose(),
        "ni relâché la poignée en chemin — {fenetre:?}"
    );
}

// ---------------------------------------------------------------------------
// 8 — ce que « adopter » rend
// ---------------------------------------------------------------------------

#[test]
fn adopter_ne_rend_vrai_que_lorsque_l_un_des_trois_champs_a_bouge() {
    // Contrat : `adopter` rend `true` si l'un des trois champs a changé de valeur. C'est ce qui dit
    // à la fenêtre s'il y a lieu de repeindre — et c'est aussi ce qui rend « rien n'est adopté »
    // observable autrement qu'en comparant trois champs à la main.
    //
    // Deux fautes symétriques, toutes deux invisibles à l'œil : rendre toujours `true` repeint la
    // fenêtre chaque seconde pour rien, rendre toujours `false` la laisse afficher un état périmé.
    let mut fenetre = au_repos(RANG_RIEN, "", LUMINOSITE_FENETRE);

    assert!(
        adopte(
            &mut fenetre,
            &releve(LUMINOSITE_DEMON, &format!("image:{CHEMIN_IMAGE}"))
        ),
        "le menu, le champ et la luminosité changent tous les trois"
    );
    assert!(
        !adopte(
            &mut fenetre,
            &releve(LUMINOSITE_DEMON, &format!("image:{CHEMIN_IMAGE}"))
        ),
        "le même relevé une seconde fois n'apprend plus rien"
    );
    assert!(
        adopte(
            &mut fenetre,
            &releve(LUMINOSITE_AUTRE, &format!("image:{CHEMIN_IMAGE}"))
        ),
        "la seule luminosité suffit"
    );
    assert!(
        adopte(
            &mut fenetre,
            &releve(LUMINOSITE_AUTRE, &format!("gif:{CHEMIN_GIF}"))
        ),
        "le seul affichage aussi"
    );

    // Poignée levée : seule la luminosité peut encore faire bouger quelque chose.
    let mut compose = en_cours(RANG_GIF, CHEMIN_GIF, LUMINOSITE_DEMON);
    assert!(
        !adopte(&mut compose, &releve(LUMINOSITE_DEMON, "rien")),
        "l'affichage est retenu et la luminosité est la même : rien n'a bougé"
    );
    assert!(
        !adopte(
            &mut compose,
            &releve(LUMINOSITE_DEMON, &format!("image:{CHEMIN_IMAGE}"))
        ),
        "un affichage tout à fait différent ne fait toujours rien bouger tant qu'on compose"
    );
    assert!(
        adopte(&mut compose, &releve(LUMINOSITE_AUTRE, "rien")),
        "la luminosité, elle, passe : c'est un changement"
    );
}

// ---------------------------------------------------------------------------
// 9 — sans choix en cours, un changement venu d'ailleurs remonte
// ---------------------------------------------------------------------------

#[test]
fn sans_choix_en_cours_un_changement_venu_d_ailleurs_remonte_dans_la_fenetre() {
    // Critère d'acceptation n° 2 de l'issue : « sans choix en cours, un `screen` reçu du démon met
    // bien à jour menu et champ ». Son contexte le nomme : `reverb screen --gauge …` tapé dans un
    // terminal, ou un redémarrage du démon qui relit `ecran.conf`.
    //
    // C'est la contrepartie du test n° 2, et la faute qu'une poignée trop zélée introduirait : une
    // fenêtre qui ne suivrait plus rien. Le parcours enchaîne les quatre affichages, pour qu'une
    // implémentation qui n'en adopterait qu'un se fasse attraper.
    let mut fenetre = au_repos(RANG_RIEN, "", LUMINOSITE_FENETRE);

    for (mot, rang, argument) in [
        (format!("image:{CHEMIN_IMAGE}"), RANG_IMAGE, CHEMIN_IMAGE),
        (format!("gif:{CHEMIN_GIF}"), RANG_GIF, CHEMIN_GIF),
        (format!("gauge:{SONDE}"), RANG_CADRAN, SONDE),
        (String::from("rien"), RANG_RIEN, ""),
        (format!("gauge:{SONDE}"), RANG_CADRAN, SONDE),
    ] {
        assert!(
            !fenetre.compose(),
            "l'utilisateur ne compose rien : la fenêtre suit le démon — {fenetre:?}"
        );
        adopte(&mut fenetre, &releve(LUMINOSITE_DEMON, &mot));
        assert_eq!(
            etat(&fenetre),
            (rang, argument.to_owned(), LUMINOSITE_DEMON),
            "« {mot} » posé depuis un terminal remonte dans la fenêtre — {fenetre:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 10 — le bout par lequel tout ça se voit
// ---------------------------------------------------------------------------

#[test]
fn choisir_un_cadran_et_taper_sa_sonde_survit_a_vingt_trois_tours_d_horloge() {
    // Le défaut de #48 tel que Nico le rencontre, joué en entier : ouvrir le menu, choisir
    // « cadran », taper le nom de la sonde caractère par caractère pendant que l'horloge d'une
    // seconde continue de demander `screen state`, puis appliquer.
    //
    // Aucun test plus court ne l'attrape vraiment. La sélection peut tenir un tour et céder au
    // deuxième ; le champ peut tenir tant qu'il est vide et se vider dès qu'on y tape ; et le nom de
    // sonde passe par « kraken2023elite: », un état intermédiaire qui **finit par un deux-points** —
    // ce qu'une implémentation qui recomposerait le champ depuis l'affichage couperait en silence.
    let mut fenetre = au_repos(RANG_RIEN, "", LUMINOSITE_DEMON);

    // On ouvre le menu et on choisit « cadran ». La fenêtre écrit le rang, puis lève la poignée.
    fenetre.affichage = RANG_CADRAN;
    fenetre.saisir();

    let mut frappe = String::new();
    for caractere in SONDE.chars() {
        frappe.push(caractere);
        fenetre.argument = frappe.clone();

        // Un tour d'horloge. « Appliquer » n'a pas été cliqué : le démon dit encore « rien ».
        let bouge = adopte(&mut fenetre, &releve(LUMINOSITE_DEMON, "rien"));
        assert!(!bouge, "rien n'a bougé pendant qu'on tape « {frappe} »");
        assert_eq!(
            etat(&fenetre),
            (RANG_CADRAN, frappe.clone(), LUMINOSITE_DEMON),
            "après « {frappe} », le menu dit toujours « cadran » et le champ tient — {fenetre:?}"
        );
    }
    assert_eq!(
        fenetre.argument, SONDE,
        "la sonde est entrée en entier, deux-points compris — {fenetre:?}"
    );

    // « Appliquer ». La fenêtre envoie, le démon accepte, et le relevé suivant le confirme.
    fenetre.relacher();
    adopte(
        &mut fenetre,
        &releve(LUMINOSITE_DEMON, &format!("gauge:{SONDE}")),
    );
    assert_eq!(
        etat(&fenetre),
        (RANG_CADRAN, SONDE.to_owned(), LUMINOSITE_DEMON),
        "le démon affiche le cadran demandé : la fenêtre montre la même chose — {fenetre:?}"
    );

    // Le refus, maintenant : une sonde que le démon ne connaît pas. Après « Appliquer », c'est
    // l'état réel qui revient — la fenêtre cesse de prétendre afficher un cadran qui n'existe pas.
    fenetre.affichage = RANG_CADRAN;
    fenetre.argument = String::from("sonde-inventee:valeur");
    fenetre.saisir();
    adopte(&mut fenetre, &releve(LUMINOSITE_DEMON, "rien"));
    assert_eq!(
        fenetre.argument, "sonde-inventee:valeur",
        "tant qu'on compose, la sonde inventée reste lisible et corrigeable — {fenetre:?}"
    );

    fenetre.relacher();
    adopte(&mut fenetre, &releve(LUMINOSITE_DEMON, "rien"));
    assert_eq!(
        etat(&fenetre),
        (RANG_RIEN, String::new(), LUMINOSITE_DEMON),
        "le démon a refusé : c'est l'état réel qui s'affiche, et le refus se voit — {fenetre:?}"
    );
}
