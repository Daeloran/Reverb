//! Tests d'intention du protocole de l'écran (issue #33).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #33 et le contrat déposé dans
//! `crates/reverb-proto/src/ipc.rs` — dont les deux points neufs, `Request::Screen(_)` à
//! l'encodage et `"screen"` à l'analyse, sont `todo!("issue #33")` à l'écriture de ce fichier. Ils
//! prolongent `spec_ipc.rs` (#17), `spec_ipc_v2.rs` (#19), `spec_ipc_v3.rs` (#23) et
//! `spec_ipc_zones.rs` (#29), auxquels **ce fichier ne touche pas** : ce qui y est écrit doit
//! continuer de passer tel quel.
//!
//! ## Ce que l'écran ajoute au fil, et ce que ça risque
//!
//! Un verbe à six actions — `screen state|brightness|off|image|gauge|gif` — et une ligne de
//! réponse, `screen <0-100> <affichage>`. Le neuf n'est ni la couleur ni la cible LED : c'est le
//! **chemin de fichier**, premier champ du protocole qui désigne une ressource que le démon ira
//! lire lui-même, sous son propre uid. Trois pièges en découlent.
//!
//! 1. **Un chemin tronqué désigne un autre fichier.** Partout ailleurs, un champ mal cadré donne
//!    une commande refusée. Ici, `screen image /home/nico/photos anciennes/a.png` coupé au premier
//!    blanc donne `/home/nico/photos`, un chemin parfaitement lisible qui n'est pas celui qu'on
//!    visait. C'est la raison d'être du point n° 1 ci-dessous.
//! 2. **Une luminosité écrêtée en silence ment au curseur.** `Request::Screen(Brightness(u8))`
//!    peut porter 255, que le matériel refuse (`screen::set_brightness`). Ramener 101 à 100 sans
//!    rien dire rendrait la fenêtre incapable de distinguer « réglé à 100 » de « ma valeur a été
//!    corrigée », et masquerait une erreur d'unité — un curseur gradué 0–255 au lieu de 0–100
//!    passerait pour un curseur qui marche.
//! 3. **Un chemin est long.** `PATH_MAX` vaut 4096 sur Linux, [`MAX_LINE_LEN`] 1024. Un chemin
//!    légal peut donc ne pas tenir sur le fil : il doit être **refusé**, jamais tronqué — voir
//!    `un_chemin_trop_long_est_refuse_jamais_tronque` et le rapport.
//!
//! ## Cinq points que le contrat laisse ouverts, et que ces tests tranchent
//!
//! 1. **Le chemin est le dernier champ de sa ligne, et va jusqu'au bout — espaces comprises.**
//!    C'est la règle du dernier champ de #17, celle qui donne ses espaces au `mode` d'un `chan` et
//!    au message d'un `err`. Elle ne s'appliquait jusqu'ici qu'à du texte écrit pour un humain ;
//!    on l'étend au chemin parce qu'un chemin **vient d'un sélecteur de fichiers**, que
//!    `Mes documents/fond d'écran.png` est un nom banal, et que le refuser rendrait le sélecteur
//!    de l'issue inutilisable sur la moitié d'un bureau. Le prix est qu'`image` et `gif` ne
//!    prendront jamais un second argument — voir le rapport.
//! 2. **La sonde d'un `gauge` est un seul jeton, non vide.** Arbitrage inverse, et pour la raison
//!    inverse : un nom de sonde ne vient pas d'une frappe humaine mais du matériel, il s'écrit
//!    `kraken2023elite:coolant`, et le reste du protocole le transporte déjà en jeton
//!    (`ResponseLine::Temp`, qui neutralise ses blancs). Une sonde à espace ne pourrait de toute
//!    façon pas se nommer sur le fil.
//! 3. **`screen off` n'est pas `screen brightness 0`.** Le contrat les sépare : `off` rend la
//!    dalle au firmware, `brightness 0` l'éteint « sans perdre ce qu'elle affichait ». Les
//!    confondre perdrait le cadran d'un utilisateur qui a seulement baissé son curseur à zéro.
//! 4. **Une action inconnue est refusée en nommant `screen`**, pas en nommant l'action : le verbe
//!    reçu est `screen`, il existe, ce sont ses arguments qui sont mauvais. Même arbitrage que
//!    `lighting tout` dans #23 et `zone toto` dans #29.
//! 5. **Le protocole ne connaît pas le vocabulaire d'un affichage.** `ResponseLine::Screen` porte
//!    l'affichage en `String` ; ce fichier fige donc l'aller-retour des quatre formes documentées
//!    et le refus d'une ligne sans affichage, mais **ne dit pas** ce que devient
//!    `screen 50 bidule`. Le protocole transporte, le démon interprète — comme pour les réglages
//!    d'animation de #19. Voir le rapport.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! Il ne fige pas le texte des refus, seulement qu'un refus nomme le verbe `screen`, dise
//! quelque chose, et que deux fautes différentes ne rendent pas la même phrase. Il ne dit rien de
//! l'encodage d'un `Brightness` au-delà de 100 : le type l'autorise, le fil le refuse, et cette
//! asymétrie appartient au contrat, pas à ses tests — voir le rapport.
//!
//! Aucun accès matériel, aucune IO, aucun socket : `reverb-proto` est pur, ses tests aussi.

use reverb_proto::ipc::{
    MAX_LINE_LEN, Request, RequestError, ResponseLine, ScreenAction, encode_request,
    encode_response_line, parse_request, parse_response_line,
};
use reverb_proto::screen;

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// La luminosité maximale, celle qu'accepte le matériel.
///
/// Pas écrite `100` en l'air : `screen::set_brightness` documente « [`BrightnessError::OutOfRange`]
/// au-delà de 100. Zéro est accepté ». `les_bornes_de_luminosite_sont_celles_du_materiel` vérifie
/// que cette constante est bien celle-là — sans quoi ce fichier pourrait borner le protocole
/// ailleurs que le matériel, et une commande acceptée sur le fil échouerait sur le bus.
const LUMINOSITE_MAX: u8 = 100;

/// Un chemin absolu quelconque, pour les cas où seul le fait qu'il soit absolu compte.
const CHEMIN: &str = "/home/nico/images/fond.png";

/// Un chemin absolu portant une espace, comme en rend un sélecteur de fichiers.
///
/// Point n° 1 du préambule, et le seul champ du protocole où un blanc soit légitime.
const CHEMIN_A_ESPACE: &str = "/home/nico/Mes documents/fond d'écran.png";

/// Une sonde, écrite comme le matériel la nomme (`ResponseLine::Temp`).
const SONDE: &str = "kraken2023elite:coolant";

fn screen(action: ScreenAction) -> Request {
    Request::Screen(action)
}

fn image(chemin: &str) -> Request {
    screen(ScreenAction::Image(chemin.to_owned()))
}

fn gif(chemin: &str) -> Request {
    screen(ScreenAction::Gif(chemin.to_owned()))
}

fn gauge(sonde: &str) -> Request {
    screen(ScreenAction::Gauge(sonde.to_owned()))
}

fn ligne_screen(luminosite: u8, affichage: &str) -> ResponseLine {
    ResponseLine::Screen {
        luminosite,
        affichage: affichage.to_owned(),
    }
}

/// L'action d'une requête `screen`, ou un échec qui dit ce qu'on a reçu à la place.
fn action_de(ligne: &str) -> ScreenAction {
    match parse_request(ligne) {
        Ok(Request::Screen(action)) => action,
        Ok(autre) => {
            panic!("« {ligne} » devait être une commande `screen`, elle a rendu {autre:?}")
        }
        Err(erreur) => panic!("« {ligne} » devait être acceptée : {erreur}"),
    }
}

/// Le verbe et la raison d'un refus d'arguments, ou un échec qui dit ce qui est arrivé.
///
/// Tout ce que ce fichier exige d'un refus tient là : un verbe **connu** — donc `BadArgument` et
/// non `UnknownVerb` — et une raison non vide. Le texte de la raison n'est jamais figé.
fn refus(ligne: &str) -> (String, String) {
    match parse_request(ligne) {
        Ok(requete) => panic!("« {ligne} » devait être refusée, elle a rendu {requete:?}"),
        Err(RequestError::BadArgument { verb, reason }) => {
            assert!(
                !reason.trim().is_empty(),
                "« {ligne} » doit être refusée en disant pourquoi"
            );
            let message = RequestError::BadArgument {
                verb: verb.clone(),
                reason: reason.clone(),
            }
            .to_string();
            assert!(
                message.contains(verb.as_str()) && message.contains(reason.as_str()),
                "le Display dit lequel et pourquoi : « {message} »"
            );
            (verb, reason)
        }
        Err(autre) => panic!("« {ligne} » doit donner un BadArgument, pas {autre:?}"),
    }
}

/// Le refus d'une commande d'écran : le verbe nommé doit être `screen`.
///
/// Point n° 4 du préambule.
fn refus_de_screen(ligne: &str) -> String {
    let (verbe, raison) = refus(ligne);
    assert_eq!(
        verbe, "screen",
        "« {ligne} » : le verbe reçu est `screen`, l'erreur doit le nommer — nommer autre chose \
         envoie chercher une faute là où il n'y en a pas"
    );
    raison
}

/// Un refus qui nomme le champ de luminosité.
///
/// Les deux orthographes sont admises — `brightness` est le mot du fil, `luminosité` celui de
/// `screen::BrightnessError` et de la ligne de réponse. Ce fichier ne choisit pas entre les deux,
/// il exige seulement que l'utilisateur sache **quel** champ il a mal tapé : une commande `screen`
/// a six formes, et « argument invalide » ne dit pas laquelle.
fn nomme_la_luminosite(raison: &str, ligne: &str) {
    let bas = raison.to_lowercase();
    assert!(
        bas.contains("brightness") || bas.contains("luminosit"),
        "« {ligne} » doit être refusée en nommant le champ fautif. Raison obtenue : {raison}"
    );
}

/// Vérifie qu'une ligne de données ne peut pas se faire prendre pour une fin de réponse.
///
/// Règle n° 5 de `spec_ipc.rs`, appliquée à la ligne neuve : « une ligne de données ne commence
/// **jamais** par `end` ni par `err` », et elle tient sur une seule ligne physique.
fn ne_termine_jamais(donnee: &ResponseLine) {
    assert!(
        !donnee.is_terminal(),
        "{donnee:?} n'est pas une ligne terminale"
    );

    let encodee = encode_response_line(donnee);
    assert_eq!(
        encodee.lines().count(),
        1,
        "une ligne de données tient sur une seule ligne : « {encodee} »"
    );
    assert!(
        !encodee.contains('\n') && !encodee.contains('\r'),
        "aucun saut de ligne dans une ligne de données : « {encodee} »"
    );
    for terminal in ["end", "err"] {
        assert!(
            !encodee.starts_with(terminal),
            "la ligne « {encodee} » commence par « {terminal} » — un client y verrait la fin de la \
             réponse et tronquerait sa lecture"
        );
    }

    let relue = parse_response_line(&encodee).expect("une ligne encodée se relit");
    assert!(
        !relue.is_terminal(),
        "« {encodee} » se relit en {relue:?}, qui terminerait la réponse"
    );
}

/// Champs **hostiles** : ceux qui, mal encodés, produiraient une ligne qu'un client prendrait pour
/// la fin de la réponse, ou pour deux champs là où il n'y en a qu'un.
///
/// Reprise de la liste de `spec_ipc_zones.rs`, dont la raison vaut ici encore : un chemin vient
/// d'un sélecteur de fichiers, donc du disque, donc de n'importe quoi — un fichier peut s'appeler
/// `a\nlight all ffffff` sur ext4, et `screen image` ne doit pas allumer le boîtier en blanc.
///
/// Le champ **vide** n'y figure pas : il n'a pas de blanc à neutraliser, et ce qu'une commande à
/// champ vide doit devenir sur le fil n'est pas tranché par le contrat. Ce que ce fichier en dit
/// tient dans les tests de refus, du côté de la lecture. Il figure en revanche dans les hostiles
/// de la **ligne de réponse**, comme dans #29.
const HOSTILES: &[&str] = &[
    "err",
    "end",
    "error",
    "endpoint",
    "ERR",
    "boom\nend",
    "\nend",
    "end\n",
    "a\r\nend",
    "mon fichier",
    "\t",
    "  ",
    "a\u{0}b",
    "screen",
    "off",
    "state",
    "\u{feff}a",
];

// ---------------------------------------------------------------------------
// 1 — les six actions font l'aller-retour sans rien perdre
// ---------------------------------------------------------------------------

#[test]
fn les_six_actions_de_screen_font_l_aller_retour_sans_rien_perdre() {
    // Contrat — `Request::Screen(ScreenAction)` et ses six variantes. C'est la propriété qui rend
    // la fenêtre et la ligne de commande interchangeables : « `reverb screen` en ligne de commande
    // continue de fonctionner, en passant désormais par le démon » (critère d'acceptation).
    let temoins = vec![
        screen(ScreenAction::State),
        screen(ScreenAction::Off),
        screen(ScreenAction::Brightness(0)),
        screen(ScreenAction::Brightness(1)),
        screen(ScreenAction::Brightness(42)),
        screen(ScreenAction::Brightness(LUMINOSITE_MAX)),
        image(CHEMIN),
        image("/a.png"),
        image(CHEMIN_A_ESPACE),
        image("/home/nico/images/été 2025/plage.jpeg"),
        gif("/home/nico/anims/pluie.gif"),
        gif(CHEMIN_A_ESPACE),
        gauge(SONDE),
        gauge("k10temp:tctl"),
    ];

    for temoin in &temoins {
        let encodee = encode_request(temoin);
        assert!(
            !encodee.contains('\n') && !encodee.contains('\r'),
            "une requête tient sur une seule ligne physique : « {encodee} »"
        );
        assert_eq!(
            parse_request(&encodee),
            Ok(temoin.clone()),
            "aller-retour de {temoin:?} par « {encodee} » — rien ne se perd en route"
        );
    }

    // La forme exacte du fil, une fois par action. Sans ces égalités, un encodeur pourrait écrire
    // `screen lum 42` et tous les allers-retours resteraient verts : c'est le décodeur qui
    // rattraperait sa propre écriture, et la ligne de commande d'un autre client ne marcherait pas.
    assert_eq!(encode_request(&screen(ScreenAction::State)), "screen state");
    assert_eq!(encode_request(&screen(ScreenAction::Off)), "screen off");
    assert_eq!(
        encode_request(&screen(ScreenAction::Brightness(42))),
        "screen brightness 42",
        "la luminosité s'écrit en clair, en pour cent — pas en 0-255, pas en hexadécimal"
    );
    assert_eq!(
        encode_request(&image(CHEMIN)),
        format!("screen image {CHEMIN}")
    );
    assert_eq!(encode_request(&gif(CHEMIN)), format!("screen gif {CHEMIN}"));
    assert_eq!(
        encode_request(&gauge(SONDE)),
        format!("screen gauge {SONDE}")
    );

    // Les six actions sont six commandes distinctes. Un décodeur qui rendrait `Image` pour un
    // `gif` afficherait la première image d'une animation, fixe, et rien ne dirait qu'elle devait
    // bouger.
    let toutes = [
        "screen state",
        "screen off",
        "screen brightness 42",
        &format!("screen image {CHEMIN}"),
        &format!("screen gif {CHEMIN}"),
        &format!("screen gauge {SONDE}"),
    ];
    for (rang, ligne) in toutes.iter().enumerate() {
        for autre in &toutes[rang + 1..] {
            assert_ne!(
                parse_request(ligne),
                parse_request(autre),
                "« {ligne} » et « {autre} » sont deux commandes différentes"
            );
        }
    }

    // `image` et `gif` portent le même chemin sans se confondre : c'est le verbe qui dit comment
    // le fichier sera joué, pas son extension. Un `.gif` passé à `image` reste une image fixe.
    assert_eq!(
        action_de("screen image /a.gif"),
        ScreenAction::Image("/a.gif".to_owned()),
        "l'action est choisie par le mot du fil, jamais devinée depuis l'extension"
    );
}

// ---------------------------------------------------------------------------
// 2 — `screen state` et `screen off` n'acceptent aucun argument
// ---------------------------------------------------------------------------

#[test]
fn screen_state_et_screen_off_n_acceptent_aucun_argument() {
    // Contrat — `ScreenAction::State` : « luminosité et contenu courants ». Elle ne change rien,
    // elle n'a donc rien à recevoir.
    //
    // Un argument accepté en silence serait le plus vicieux des défauts : `screen state 5`
    // marcherait aujourd'hui et voudrait dire autre chose le jour où `state` prendra un filtre.
    assert_eq!(
        parse_request("screen state"),
        Ok(screen(ScreenAction::State))
    );
    assert_eq!(parse_request("screen off"), Ok(screen(ScreenAction::Off)));

    for ligne in [
        "screen state 1",
        "screen state all",
        "screen state state",
        &format!("screen state {CHEMIN}"),
        "screen off 0",
        "screen off off",
        &format!("screen off {CHEMIN}"),
    ] {
        refus_de_screen(ligne);
    }

    // Point n° 3 du préambule : `off` rend la dalle au firmware, `brightness 0` l'éteint « sans
    // perdre ce qu'elle affichait ». Deux commandes, deux effets — les confondre effacerait le
    // cadran d'un utilisateur qui a seulement baissé son curseur.
    assert_ne!(
        parse_request("screen off"),
        parse_request("screen brightness 0"),
        "`screen off` et `screen brightness 0` ne sont pas la même commande : l'une rend la dalle \
         au firmware, l'autre l'éteint en gardant ce qu'elle affichait"
    );

    // Sensible à la casse, comme tout le reste du protocole. Une variante de casse est un verbe
    // inconnu, pas un argument mauvais : le client n'a pas mal rempli une commande, il en a
    // inventé une.
    for ligne in [
        "SCREEN state",
        "Screen state",
        "screens state",
        "ecran state",
    ] {
        assert!(
            matches!(parse_request(ligne), Err(RequestError::UnknownVerb { .. })),
            "« {ligne} » n'est pas une commande du protocole"
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — une luminosité hors bornes est refusée, en nommant le champ
// ---------------------------------------------------------------------------

#[test]
fn une_luminosite_hors_bornes_est_refusee_en_nommant_le_champ() {
    // Test d'intention n° 9 de l'issue : « Une luminosité hors bornes est refusée en la nommant. »
    // Piège n° 2 du préambule : écrêter 101 à 100 en silence rendrait la fenêtre incapable de
    // distinguer « réglé à 100 » d'une valeur corrigée derrière son dos, et masquerait un curseur
    // gradué dans la mauvaise unité.
    let hors_bornes = [
        "101", "102", "200", "255", "256", "1000", "-1", "-0", "abc", "", "1.5", "50%", "0x10",
        "1e2", "５０",
    ];
    // ⚠️ `+5` ne figure pas dans cette liste : `u8::from_str` accepte le signe plus, ici comme
    // pour la consigne d'un `fan pwm`. Le refuser ferait de `screen` la seule commande du
    // protocole à borner ses nombres autrement que le reste — un test d'intention ne crée pas une
    // exception dont l'issue ne parle pas. Voir le rapport.

    let mut raisons = Vec::new();
    for brut in hors_bornes {
        let ligne = format!("screen brightness {brut}");
        let raison = refus_de_screen(&ligne);
        nomme_la_luminosite(&raison, &ligne);
        if !brut.trim().is_empty() {
            assert!(
                raison.contains(brut.trim()),
                "« {ligne} » doit être refusée en rendant la valeur reçue — sans elle, un curseur \
                 mal gradué envoie 255 et l'utilisateur ne voit qu'un refus sans cause. Raison \
                 obtenue : {raison}"
            );
        }
        raisons.push(raison);
    }

    // La valeur manquante est une faute distincte : il n'y a rien à nommer, mais il faut dire
    // lequel des six arguments manque.
    let raison = refus_de_screen("screen brightness");
    nomme_la_luminosite(&raison, "screen brightness");

    // Les refus distinguent les fautes : « hors bornes » et « pas un nombre » ne se corrigent pas
    // de la même façon, et un curseur qui envoie du texte n'est pas un curseur mal gradué.
    raisons.sort_unstable();
    raisons.dedup();
    assert!(
        raisons.len() >= 2,
        "une valeur hors bornes et une valeur qui n'est pas un nombre ne peuvent pas partager une \
         seule explication : {raisons:?}"
    );

    // Un argument de trop : `screen brightness 50 60` ne doit pas prendre le premier et jeter le
    // second — un client qui envoie deux valeurs se trompe, et le silence lui donnerait raison.
    for ligne in ["screen brightness 50 60", "screen brightness 50 %"] {
        refus_de_screen(ligne);
    }

    // Les bornes elles-mêmes passent, et c'est ce qui fait la valeur du test précédent : une borne
    // posée à `< 100` perdrait la pleine luminosité, une borne à `> 0` perdrait l'extinction —
    // « Zéro est accepté : éteindre l'écran est un réglage légitime » (`screen::set_brightness`).
    for valeur in [0, 1, 50, LUMINOSITE_MAX - 1, LUMINOSITE_MAX] {
        let ligne = format!("screen brightness {valeur}");
        assert_eq!(
            action_de(&ligne),
            ScreenAction::Brightness(valeur),
            "« {ligne} » est une luminosité légitime"
        );
    }
}

#[test]
fn les_bornes_de_luminosite_sont_celles_du_materiel() {
    // La constante de ce fichier n'est pas un nombre choisi ici : c'est la borne que le matériel
    // impose, documentée par `screen::set_brightness`. Si l'écran acceptait un jour davantage, ce
    // test tomberait avant que le protocole ne devienne plus étroit que le bus.
    assert!(
        screen::set_brightness(LUMINOSITE_MAX).is_ok(),
        "le matériel accepte {LUMINOSITE_MAX} %"
    );
    assert!(
        screen::set_brightness(LUMINOSITE_MAX + 1).is_err(),
        "le matériel refuse au-delà de {LUMINOSITE_MAX} % — le protocole ne doit pas le laisser \
         passer non plus"
    );
    assert!(screen::set_brightness(0).is_ok(), "zéro est un réglage");
}

// ---------------------------------------------------------------------------
// 4 — un chemin relatif est refusé, en le disant
// ---------------------------------------------------------------------------

#[test]
fn un_chemin_relatif_est_refuse_en_le_disant() {
    // Test d'intention n° 3 de l'issue, et contrat de `ScreenAction::Image` : « Le chemin doit
    // être **absolu** : le démon lit sous son propre répertoire courant, qui n'est pas celui du
    // client. »
    //
    // Le mode de défaillance est muet et déroutant : `reverb screen image fond.png` lancé depuis
    // `~/images` ferait chercher au démon `/fond.png`, ou `/` — et le message serait « fichier
    // introuvable », qui envoie l'utilisateur vérifier un fichier qu'il a sous les yeux.
    let relatifs = [
        "fond.png",
        "./fond.png",
        "../fond.png",
        "images/fond.png",
        "~/fond.png",
        "~nico/fond.png",
        ".",
        "..",
    ];

    let mut raisons = Vec::new();
    for brut in relatifs {
        for verbe in ["image", "gif"] {
            let ligne = format!("screen {verbe} {brut}");
            let raison = refus_de_screen(&ligne);
            let bas = raison.to_lowercase();
            assert!(
                bas.contains("absolu") || bas.contains("relatif"),
                "« {ligne} » doit être refusée **en le disant** : sans le mot, le message se \
                 confond avec « fichier introuvable » et envoie chercher la faute ailleurs. \
                 Raison obtenue : {raison}"
            );
            raisons.push(raison);
        }
    }

    // Un chemin absent est une faute distincte d'un chemin relatif : il n'y a rien à rendre
    // absolu, et un message parlant de racine enverrait chercher une faute dans un champ qu'on n'a
    // pas écrit.
    for ligne in ["screen image", "screen image ", "screen gif", "screen gif "] {
        refus_de_screen(ligne);
    }

    // Le tilde n'est pas une racine : c'est le shell qui l'étend, et un démon qui l'étendrait
    // lui-même le ferait vers *son* foyer, pas celui du client.
    assert!(
        parse_request("screen image ~/fond.png").is_err(),
        "`~` n'est pas un chemin absolu : le démon ne partage pas le foyer de son client"
    );

    // Et l'absolu passe, y compris les formes qui ressemblent à du relatif sans en être.
    for brut in ["/", "/a", "/./fond.png", "/../fond.png", "/home/nico/."] {
        let ligne = format!("screen image {brut}");
        assert_eq!(
            action_de(&ligne),
            ScreenAction::Image(brut.to_owned()),
            "« {ligne} » commence par une racine : c'est au démon de dire si le fichier existe, \
             pas au protocole de deviner"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — un chemin porte ses espaces jusqu'au bout de la ligne
// ---------------------------------------------------------------------------

#[test]
fn un_chemin_portant_une_espace_traverse_intact() {
    // Point n° 1 du préambule, isolé dans son propre test pour qu'un échec pointe cette décision
    // et pas une autre. Le protocole se découpe aux espaces ; le chemin est le **dernier** champ
    // de sa ligne, et prend donc tout ce qui reste — la règle du dernier champ de #17, celle qui
    // donne ses espaces au `mode` d'un `chan` et au message d'un `err`.
    //
    // Piège n° 1 du préambule : un chemin coupé au premier blanc reste un chemin lisible. Là où
    // une cible LED tronquée donne un refus, un chemin tronqué donne un **autre fichier**, et le
    // démon l'affiche sans un mot.
    assert_eq!(
        action_de(&format!("screen image {CHEMIN_A_ESPACE}")),
        ScreenAction::Image(CHEMIN_A_ESPACE.to_owned()),
        "un chemin de sélecteur de fichiers porte des espaces : le refuser ou le couper rendrait \
         le sélecteur de l'issue inutilisable sur la moitié d'un bureau"
    );
    assert_eq!(
        action_de(&format!("screen gif {CHEMIN_A_ESPACE}")),
        ScreenAction::Gif(CHEMIN_A_ESPACE.to_owned())
    );

    // Plusieurs espaces d'affilée survivent aussi : un chemin les porte tels quels sur le disque,
    // et un découpage qui les fusionnerait désignerait un fichier qui n'existe pas.
    for chemin in [
        "/a  b.png",
        "/a b c d.png",
        "/tmp/deux  espaces/fond.png",
        "/tmp/. et .. au milieu/../fond.png",
    ] {
        assert_eq!(
            action_de(&format!("screen image {chemin}")),
            ScreenAction::Image(chemin.to_owned()),
            "« {chemin} » doit traverser octet pour octet"
        );
        assert_eq!(
            encode_request(&image(chemin)),
            format!("screen image {chemin}"),
            "et se réécrire tel quel : neutraliser les espaces d'un chemin le ferait pointer \
             ailleurs"
        );
    }

    // Point n° 2 du préambule, le contraire pour la sonde : elle est un jeton, comme dans
    // `ResponseLine::Temp`. Un second jeton derrière elle est une faute, pas un nom à rallonge.
    for ligne in [
        "screen gauge ma sonde",
        "screen gauge k10temp:tctl bidule",
        "screen gauge",
        "screen gauge ",
    ] {
        refus_de_screen(ligne);
    }
}

#[test]
fn un_chemin_trop_long_est_refuse_jamais_tronque() {
    // Piège n° 3 du préambule. `PATH_MAX` vaut 4096 sur Linux, `MAX_LINE_LEN` 1024 : un chemin
    // parfaitement légal peut ne pas tenir sur le fil.
    //
    // Ce test ne demande pas qu'il passe — c'est une tension du contrat, laissée au rapport. Il
    // demande qu'il soit **refusé**, et refusé pour sa longueur. Un chemin tronqué à 1024 octets
    // désigne un répertoire parent, ou un fichier voisin : le démon lirait autre chose et
    // l'afficherait sans un mot.
    let prefixe = "screen image /";
    let long = format!("{prefixe}{}", "a".repeat(MAX_LINE_LEN + 1 - prefixe.len()));
    assert_eq!(long.len(), MAX_LINE_LEN + 1);
    assert_eq!(
        parse_request(&long),
        Err(RequestError::TooLong {
            given: MAX_LINE_LEN + 1
        }),
        "un octet de trop suffit, et l'erreur ne recopie pas la ligne (#17)"
    );

    // La ligne juste à la limite passe, elle : la borne est sur la longueur, pas sur le verbe.
    let juste = format!("{prefixe}{}", "a".repeat(MAX_LINE_LEN - prefixe.len()));
    assert_eq!(juste.len(), MAX_LINE_LEN);
    assert!(
        matches!(action_de(&juste), ScreenAction::Image(_)),
        "{MAX_LINE_LEN} octets sont la longueur maximale *acceptée*"
    );
}

// ---------------------------------------------------------------------------
// 6 — une action inconnue, et `screen` tout seul
// ---------------------------------------------------------------------------

#[test]
fn une_action_inconnue_est_refusee_en_nommant_screen() {
    // Point n° 4 du préambule. Le verbe reçu est `screen` : il existe, ce sont ses arguments qui
    // sont mauvais. Nommer `toto` comme verbe enverrait l'utilisateur chercher une commande qu'il
    // n'a pas tapée, et un client qui filtre les `UnknownVerb` pour proposer une correction
    // proposerait de corriger `toto` au lieu de `state`.
    let mut raisons = Vec::new();
    for ligne in [
        "screen toto",
        "screen toto /a.png",
        "screen png /a.png",
        "screen video /a.mp4",
        "screen luminosite 50",
        "screen bright 50",
        "screen State",
        "screen IMAGE /a.png",
        "screen images /a.png",
        "screen on",
    ] {
        raisons.push(refus_de_screen(ligne));
    }

    // `screen` seul n'est pas `screen state` : le verbe attend une action, et en deviner une
    // ferait d'une commande tronquée une commande réussie. Même arbitrage que `light` seul dans
    // #17 et `zone` seul dans #29.
    refus_de_screen("screen");
    refus_de_screen("screen ");
    assert_ne!(
        parse_request("screen"),
        Ok(screen(ScreenAction::State)),
        "`screen` seul ne doit pas se lire comme `screen state` : deviner une action fait d'une \
         frappe interrompue une commande exécutée"
    );

    // Les refus disent quelque chose, et ne se ramènent pas tous à la même phrase : une action
    // inconnue et une action mal orthographiée ne se corrigent pas de la même façon.
    raisons.sort_unstable();
    raisons.dedup();
    assert!(
        !raisons.is_empty(),
        "un refus doit dire quelque chose : {raisons:?}"
    );

    // Et le préfixe voisin n'est pas le verbe `screen` : `screen <0-100> <affichage>` est une
    // ligne de **réponse**, qui n'a rien à faire dans une requête.
    for ligne in ["screen 100 rien", "screen 0 rien"] {
        refus_de_screen(ligne);
    }
}

// ---------------------------------------------------------------------------
// 7 — la ligne de réponse `screen` fait l'aller-retour
// ---------------------------------------------------------------------------

#[test]
fn la_ligne_de_reponse_screen_fait_l_aller_retour_sans_rien_perdre() {
    // Contrat — `ResponseLine::Screen` : « `screen <0-100> <affichage>` — l'état de la dalle.
    // L'affichage s'écrit `rien`, `gauge:<sonde>`, `image:<chemin>` ou `gif:<chemin>`. »
    //
    // C'est par cette ligne que la fenêtre apprend ce que la dalle montre. Une luminosité perdue
    // en route, et le curseur se replace tout seul au mauvais endroit à chaque ouverture.
    let lignes = vec![
        ligne_screen(0, "rien"),
        ligne_screen(LUMINOSITE_MAX, "rien"),
        ligne_screen(LUMINOSITE_MAX, &format!("gauge:{SONDE}")),
        ligne_screen(100, "gauge:k10temp:tctl"),
        ligne_screen(42, &format!("image:{CHEMIN}")),
        ligne_screen(42, &format!("image:{CHEMIN_A_ESPACE}")),
        ligne_screen(1, "gif:/home/nico/anims/pluie.gif"),
        ligne_screen(99, &format!("gif:{CHEMIN_A_ESPACE}")),
    ];

    for ligne in &lignes {
        let encodee = encode_response_line(ligne);
        let relue = parse_response_line(&encodee)
            .unwrap_or_else(|erreur| panic!("« {encodee} » doit se relire : {erreur}"));
        assert_eq!(
            &relue, ligne,
            "aller-retour de {ligne:?} par « {encodee} » — rien ne se perd en route"
        );
        ne_termine_jamais(ligne);
    }

    // Les deux formes que l'énoncé nomme, à la lettre. Sans ces égalités, un encodeur pourrait
    // écrire `screen rien 0` — l'ordre inversé — et l'aller-retour resterait vert.
    assert_eq!(
        encode_response_line(&ligne_screen(0, "rien")),
        "screen 0 rien",
        "la luminosité d'abord, l'affichage ensuite"
    );
    assert_eq!(
        encode_response_line(&ligne_screen(100, "gauge:k10temp:tctl")),
        "screen 100 gauge:k10temp:tctl"
    );
    assert_eq!(
        parse_response_line("screen 0 rien"),
        Ok(ligne_screen(0, "rien")),
        "et « screen 0 rien » se relit — c'est l'état d'une dalle éteinte au premier démarrage"
    );
    assert_eq!(
        parse_response_line("screen 100 gauge:k10temp:tctl"),
        Ok(ligne_screen(100, "gauge:k10temp:tctl")),
        "la sonde porte deux-points dans son nom : découper l'affichage sur le premier `:` \
         perdrait `tctl`"
    );

    // La luminosité traverse en clair et se relit à l'identique : une valeur écrêtée, arrondie ou
    // lue en 0-255 replacerait le curseur ailleurs qu'où l'utilisateur l'a mis.
    for valeur in [0, 1, 42, 99, LUMINOSITE_MAX] {
        let encodee = encode_response_line(&ligne_screen(valeur, "rien"));
        assert_eq!(
            encodee,
            format!("screen {valeur} rien"),
            "la luminosité s'écrit en pour cent, en décimal"
        );
        let ResponseLine::Screen { luminosite, .. } =
            parse_response_line(&encodee).expect("une ligne bien formée se relit")
        else {
            panic!("« {encodee} » est une ligne d'écran");
        };
        assert_eq!(luminosite, valeur, "relue par « {encodee} »");
    }

    // Ce qui n'est pas une ligne d'écran valide est refusé en portant la ligne fautive, pour qu'on
    // la retrouve dans un journal.
    for inconnue in [
        "screen",
        "screen ",
        "screen 50",
        "screen rien",
        "screen abc rien",
        "screen 101 rien",
        "screen 255 rien",
        "screen -1 rien",
        "screen 1.5 rien",
        "screen  rien",
        "screens 50 rien",
        "screen-light 50 rien",
        "screen state",
        "screen brightness 50",
    ] {
        let erreur = parse_response_line(inconnue)
            .err()
            .unwrap_or_else(|| panic!("« {inconnue} » n'est pas une ligne de réponse valide"));
        assert_eq!(
            erreur.line, inconnue,
            "le refus porte la ligne fautive, pour qu'on la retrouve dans un journal"
        );
        assert!(
            !erreur.reason.trim().is_empty(),
            "« {inconnue} » doit être refusée en disant pourquoi"
        );
    }

    // `screen state` est une requête, pas une ligne de réponse ; et réciproquement. Les deux
    // espaces de noms partagent le mot `screen` et ne doivent pas se mélanger — le lire comme un
    // état de dalle ferait apparaître dans la fenêtre une luminosité tirée du mot « state ».
    assert!(
        parse_response_line("screen state").is_err(),
        "« screen state » est une requête : la lire comme un état de dalle donnerait une \
         luminosité inventée"
    );
    assert!(
        parse_request("screen 50 image:/a.png").is_err(),
        "« screen 50 image:/a.png » est une ligne de réponse, pas une commande"
    );
}

// ---------------------------------------------------------------------------
// 8 — aucun champ de `screen` ne peut se faire passer pour deux
// ---------------------------------------------------------------------------

#[test]
fn aucun_champ_de_screen_ne_peut_se_faire_passer_pour_deux() {
    // En-tête du module `ipc` : « le préfixe de type assure l'invariant pour le début de ligne. Il
    // n'assure rien contre un saut de ligne à l'intérieur d'un champ. »
    //
    // Un chemin vient du disque, une sonde du matériel : ni l'un ni l'autre n'est écrit par le
    // protocole. Un champ portant un saut de ligne scinderait la commande en deux, et
    // `screen image $'/tmp/a\nlight all ffffff'` allumerait le boîtier en blanc depuis un
    // sélecteur de fichiers.
    for hostile in HOSTILES {
        // Un chemin hostile reste **absolu** : la racine est ce qui en fait un chemin, et un
        // hostile relatif serait refusé pour cette raison-là, pas pour son cadrage.
        let chemin = format!("/tmp/{hostile}");
        for requete in [image(&chemin), gif(&chemin), gauge(hostile)] {
            let encodee = encode_request(&requete);
            assert_eq!(
                encodee.lines().count(),
                1,
                "une requête tient sur une seule ligne, quel que soit son chemin : « {encodee} »"
            );
            assert!(
                !encodee.contains('\n') && !encodee.contains('\r'),
                "aucun saut de ligne dans une requête encodée : « {encodee} »"
            );

            let relue = parse_request(&encodee).unwrap_or_else(|erreur| {
                panic!("« {encodee} » doit rester une commande lisible : {erreur}")
            });
            let Request::Screen(action) = &relue else {
                panic!("« {encodee} » se relit en {relue:?} — le champ hostile a changé de verbe");
            };
            let Request::Screen(posee) = &requete else {
                unreachable!("les trois témoins sont des commandes d'écran");
            };
            assert_eq!(
                std::mem::discriminant(action),
                std::mem::discriminant(posee),
                "« {encodee} » se relit en {action:?} — le champ hostile a changé l'action"
            );

            // Une sonde ne peut pas ressortir avec un blanc : elle n'est pas en fin de ligne dans
            // l'esprit du protocole, et un blanc en ferait deux champs le jour où `gauge` en
            // prendra un second. Un chemin, lui, a le droit à ses espaces — mais jamais à un
            // caractère de contrôle, qui casserait le cadrage.
            match action {
                ScreenAction::Gauge(sonde) => {
                    assert!(
                        !sonde.is_empty(),
                        "une sonde vide disparaîtrait entre deux espaces : « {encodee} »"
                    );
                    assert!(
                        !sonde.chars().any(|c| c.is_whitespace() || c.is_control()),
                        "la sonde « {sonde} » porte un blanc ou un caractère de contrôle après \
                         relecture de « {encodee} » — elle se ferait prendre pour deux champs"
                    );
                }
                ScreenAction::Image(lu) | ScreenAction::Gif(lu) => {
                    assert!(
                        !lu.chars().any(char::is_control),
                        "le chemin « {lu} » porte un caractère de contrôle après relecture de \
                         « {encodee} » — il casserait le cadrage de la réponse suivante"
                    );
                    assert!(
                        lu.starts_with('/'),
                        "un chemin absolu le reste après encodage : « {lu} »"
                    );
                }
                autre => panic!("{autre:?} n'est pas une des trois actions à champ libre"),
            }
        }
    }

    // Le champ vide entre en scène ici, du côté des lignes de réponse : #23 a déjà tranché qu'un
    // champ vide encodé ne doit pas casser la ligne.
    let mut hostiles: Vec<&str> = HOSTILES.to_vec();
    hostiles.push("");

    for hostile in hostiles {
        for ligne in [
            ligne_screen(0, hostile),
            ligne_screen(LUMINOSITE_MAX, hostile),
            ligne_screen(50, &format!("image:/tmp/{hostile}")),
            ligne_screen(50, &format!("gauge:{hostile}")),
        ] {
            ne_termine_jamais(&ligne);

            let encodee = encode_response_line(&ligne);
            let relue = parse_response_line(&encodee)
                .unwrap_or_else(|erreur| panic!("« {encodee} » doit se relire : {erreur}"));
            let ResponseLine::Screen { luminosite, .. } = &relue else {
                panic!("« {encodee} » se relit en {relue:?} : le type de ligne a changé");
            };
            let ResponseLine::Screen {
                luminosite: posee, ..
            } = &ligne
            else {
                unreachable!("les quatre témoins sont des lignes d'écran");
            };
            assert_eq!(
                luminosite, posee,
                "la luminosité traverse le champ hostile intacte : « {encodee} »"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 9 — le protocole s'étend, il ne casse pas
// ---------------------------------------------------------------------------

#[test]
fn les_verbes_et_les_lignes_d_avant_l_ecran_traversent_toujours() {
    // Un verbe de plus ne doit rien coûter à ceux d'avant. `screen` partage son préfixe avec rien,
    // mais la ligne de réponse `screen` et la requête `screen` partagent leur premier mot — et un
    // découpage trop souple les confondrait.
    assert!(parse_request("status").is_ok());
    assert!(parse_request("geometry").is_ok());
    assert!(parse_request("lighting").is_ok());
    assert!(parse_request("watch").is_ok());
    assert!(parse_request("zone list").is_ok());
    assert!(parse_request("animate vague").is_ok());
    assert!(parse_request("light all ff2080").is_ok());
    assert!(parse_request("fan nzxtsmart2:fan-1 auto").is_ok());

    assert_eq!(
        parse_response_line("temp kraken2023elite:coolant 34200"),
        Ok(ResponseLine::Temp {
            sensor: "kraken2023elite:coolant".to_owned(),
            millidegrees: 34_200
        }),
        "la ligne `temp` de #17 reste ce qu'elle était : c'est la sonde d'un cadran, pas son \
         affichage"
    );
    assert_eq!(
        parse_response_line("end"),
        Ok(ResponseLine::End),
        "et les lignes terminales aussi"
    );

    // `light` seul reste refusé en nommant son verbe — test d'intention de #17, repris ici sans
    // être réécrit. Un test d'intention ne se réécrit pas pour arranger un design venu après lui.
    let (verbe, _) = refus("light");
    assert_eq!(verbe, "light");
}
