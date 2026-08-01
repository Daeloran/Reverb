//! Tests d'intention des deux curseurs de la fenêtre (issue #32).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #32 et le contrat déposé dans
//! `crates/reverb-gui/src/reglages.rs` — dont tous les corps sont `todo!("issue #32")` à l'écriture
//! de ce fichier. Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne dort : le temps est
//! **injecté**, chaque méthode qui en dépend reçoit son instant. Un test qui dormirait pour
//! attendre la fin de la grâce mesurerait l'ordonnanceur, pas la poignée.
//!
//! ## Les trois fautes que ce fichier existe pour interdire
//!
//! 1. **Le curseur qui ne dit rien.** Avant #32, bouger la vitesse n'écrivait qu'une propriété de
//!    l'interface : la poignée bougeait, l'animation gardait son ancienne allure, et rien ne
//!    signalait l'écart. C'est le défaut le plus silencieux du lot — l'interface a l'air de
//!    marcher. D'où des tests qui n'exigent pas seulement « une commande part », mais que cette
//!    commande **porte la nouvelle valeur et rien d'autre de changé**.
//! 2. **Le curseur qui parle trop.** Une commande par image pendant un glissement inonderait le
//!    démon d'ordres que le ventilateur n'a pas le temps de suivre. Le contrat le dit : `a_envoyer`
//!    rend `Some` « au premier appel qui suit un changement de consigne, `None` ensuite ».
//! 3. **La commande refusée en bloc.** `arc-en-ciel` génère ses propres teintes et **refuse**
//!    `couleur` ; la lui donner ne fait pas perdre la couleur, ça fait rejeter l'`animate` entier —
//!    donc bouger la vitesse sous un arc-en-ciel n'aurait aucun effet, pour une clé de trop. La
//!    faute vaut d'être vérifiée sur **tout** le catalogue, pas sur l'animation d'exemple.
//!
//! ## Ce que le contrat laisse ouvert, et que ces tests tranchent
//!
//! 1. **À l'instant précis où la grâce expire, la mesure a repris la main.** Le contrat dit « la
//!    consigne tient encore [`GRACE`] » : elle tient *pendant* `GRACE`, donc au relâchement plus
//!    `GRACE` pile, elle a fini de tenir. La borne est encadrée des deux côtés par le plus petit
//!    écart que le temps injecté sache représenter — un `<` mis pour un `<=` ne change rien à
//!    l'usage, mais une borne non pinnée laisserait passer les deux fautes qui comptent : une grâce
//!    qui ne s'applique jamais, et une grâce éternelle.
//! 2. **Un pas, c'est un changement de valeur, pas un appel.** Reposer la poignée là où elle est
//!    déjà n'est pas un pas : `saisir` avec la valeur courante ne réarme pas `a_envoyer`. Sans quoi
//!    « au plus une commande par pas de valeur » ne dirait rien — une interface qui appelle
//!    `saisir` à chaque image avec la même valeur produirait une commande par image tout en
//!    respectant la lettre du critère.
//! 3. **Rendre un canal au firmware annule la consigne en attente.** `liberer` envoie `fan <canal>
//!    auto` ; une consigne `pwm` partie juste après ressortirait le canal de l'automatique qu'on
//!    vient de lui demander. C'est la seule lecture qui rende les deux gestes composables.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Ce qu'affiche une poignée qui n'a encore rien vu** — ni mesure, ni consigne. Le contrat dit
//!   qu'elle « suit la mesure » sans dire ce qu'elle montre avant la première. En pratique la
//!   fenêtre reçoit la télémétrie avant de dessiner ; exiger un nombre ici inventerait une règle.
//!   Même trou juste après `liberer`, avant la mesure suivante : ces tests n'exigent pas laquelle
//!   des deux valeurs connues la poignée montre, seulement qu'elle n'en invente pas une troisième.
//! - **Un rang de direction hors des six**, et **un nom d'animation hors du catalogue**. Le contrat
//!   ne dit pas si `commande` doit alors rendre `None`, tomber sur un défaut, ou paniquer. Les deux
//!   viennent de la fenêtre, qui ne peut proposer que ce qu'elle liste — les inventer ici
//!   figerait un comportement que personne n'a choisi.
//! - **`animate off`** : l'arrêt d'une animation est `name: None` côté protocole, il ne passe pas
//!   par un curseur. Ce que ces tests exigent, c'est qu'aucun mouvement de curseur ne le produise.

use std::time::Duration;

use reverb_anim::{Animation, CATALOGUE, Direction, Reglages};
use reverb_gui::reglages::{GRACE, Poignee, Reglage};
use reverb_proto::Rgb;
use reverb_proto::ipc::Request;

// ---------------------------------------------------------------------------
// Repères et aides
// ---------------------------------------------------------------------------

/// La couleur affichée par la fenêtre dans ces tests.
///
/// Choisie **différente du défaut** de [`Reglages`], comme la vitesse et la direction qui suivent :
/// le décodeur du démon remplace une clé absente par son défaut, donc une implémentation qui
/// perdrait un réglage en route rendrait exactement la même chose qu'une implémentation correcte
/// si le test se contentait des valeurs par défaut. Un test dédié vérifie que les trois diffèrent.
const COULEUR: Rgb = Rgb::new(0x12, 0x9a, 0x40);

/// La vitesse avant le geste, et celle après. Deux valeurs dans les bornes acceptées (1 à 10).
const VITESSE_AVANT: u8 = 7;
const VITESSE_APRES: u8 = 9;

/// La direction avant le geste, et celle après.
///
/// `BasHaut` est le **rang zéro** de [`Direction::ALL`] : c'est la valeur qu'une implémentation
/// qui perdrait le rang en chemin rendrait par accident, d'où son emploi comme destination du
/// changement plutôt que comme point de départ.
const DIRECTION_AVANT: Direction = Direction::ArriereAvant;
const DIRECTION_APRES: Direction = Direction::BasHaut;

/// L'animation d'exemple : elle accepte les trois réglages.
const AVEC_COULEUR: &str = "comete";

/// Celle qui refuse `couleur` — elle génère ses teintes.
const SANS_COULEUR: &str = "arc-en-ciel";

/// L'origine du temps injecté.
///
/// Volontairement loin de zéro : la fenêtre est ouverte depuis un moment quand une main touche
/// enfin un curseur. Une poignée qui prendrait `Duration::ZERO` pour « aucune consigne encore » se
/// ferait attraper ici plutôt qu'à l'usage.
const DEBUT: Duration = Duration::from_secs(3_600);

/// La cadence de la télémétrie : « la télémétrie réécrit **chaque seconde** » (issue #32).
const CADENCE: Duration = Duration::from_secs(1);

/// Le plus petit écart que le temps injecté sache représenter.
///
/// Sert à encadrer la borne de [`GRACE`] sans laisser d'espace entre l'essai d'avant et celui
/// d'après, et à espacer les pas d'un glissement — une main qui traverse une barre le fait plus
/// vite que la télémétrie n'arrive.
const INSTANT: Duration = Duration::from_nanos(1);

/// Ce que le matériel rapporte tant qu'il n'a pas rejoint la consigne : le régime du firmware.
const MESURE_AU_REPOS: u8 = 30;

/// Là où l'utilisateur tire la poignée. Franchement au-dessus de la mesure : c'est l'écart que la
/// poignée refermait toute seule avant #32.
const CONSIGNE: u8 = 80;

/// Une seconde consigne, pour vérifier qu'un second geste réarme ce que le premier a désarmé.
const AUTRE_CONSIGNE: u8 = 55;

/// Le rang d'une direction dans [`Direction::ALL`], tel que [`Reglage::direction`] le porte.
fn rang(direction: Direction) -> usize {
    Direction::ALL
        .into_iter()
        .position(|d| d == direction)
        .unwrap_or_else(|| panic!("{direction:?} est une des six directions"))
}

/// Les réglages tels que la fenêtre les affiche.
fn reglage(animation: Option<&str>, vitesse: u8, direction: Direction) -> Reglage {
    Reglage {
        animation: animation.map(str::to_owned),
        couleur: COULEUR,
        vitesse,
        direction: rang(direction),
    }
}

/// Le nom et les paires portés par la commande d'un réglage, ou un échec qui dit ce qui est venu.
fn animate(reglage: &Reglage) -> (String, Vec<(String, String)>) {
    match reglage.commande() {
        Some(Request::Animate {
            name: Some(nom),
            reglages,
        }) => (nom, reglages),
        Some(Request::Animate {
            name: None,
            reglages,
        }) => panic!(
            "bouger un curseur pendant « {:?} » doit renvoyer cette animation, pas l'éteindre : \
             « animate off » reçu, avec {reglages:?}",
            reglage.animation
        ),
        autre => panic!(
            "les curseurs de #32 renvoient l'animation courante, pas un autre verbe : {autre:?} \
             pour {reglage:?}"
        ),
    }
}

/// Ce que le démon lira de ces paires.
///
/// C'est `reverb-anim` qui les valide de l'autre côté du socket, et son refus porte sur la
/// commande **entière** : passer par lui, c'est vérifier que la commande est jouable, pas
/// seulement qu'elle est jolie.
fn relu(nom: &str, paires: &[(String, String)]) -> Reglages {
    let animation = Animation::par_nom(nom)
        .unwrap_or_else(|e| panic!("« {nom} » doit être une animation du catalogue : {e}"));
    animation
        .reglages(paires)
        .unwrap_or_else(|e| panic!("le démon doit accepter {paires:?} pour « {nom} » : {e}"))
}

/// La valeur portée pour une clé, si elle est portée.
fn valeur<'a>(paires: &'a [(String, String)], cle: &str) -> Option<&'a str> {
    paires
        .iter()
        .find(|(k, _)| k == cle)
        .map(|(_, v)| v.as_str())
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Garde-fou, pas critère : le décodeur du démon comble une clé absente avec le défaut de
    // `Reglages`. Si les valeurs choisies plus haut étaient ces défauts, tous les tests qui suivent
    // passeraient aussi bien sur une implémentation qui n'envoie **rien** — le round-trip rendrait
    // les mêmes réglages par accident. Ce test est la condition de validité de tous les autres.
    let defaut = Reglages::default();
    assert_ne!(
        COULEUR, defaut.couleur,
        "la couleur des tests doit différer du défaut, sinon une couleur perdue passe inaperçue"
    );
    for vitesse in [VITESSE_AVANT, VITESSE_APRES] {
        assert_ne!(
            vitesse, defaut.vitesse,
            "la vitesse {vitesse} des tests doit différer du défaut {}",
            defaut.vitesse
        );
    }
    for direction in [DIRECTION_AVANT, DIRECTION_APRES] {
        assert_ne!(
            direction, defaut.direction,
            "la direction {direction:?} des tests doit différer du défaut {:?}",
            defaut.direction
        );
    }
    assert_ne!(
        VITESSE_AVANT, VITESSE_APRES,
        "le geste doit changer quelque chose"
    );
    assert_ne!(
        DIRECTION_AVANT, DIRECTION_APRES,
        "le geste doit changer quelque chose"
    );
    assert_ne!(
        MESURE_AU_REPOS, CONSIGNE,
        "la mesure doit contredire la consigne, c'est tout le sujet de la poignée"
    );
}

// ---------------------------------------------------------------------------
// 1 — la vitesse s'applique tout de suite, et ne change qu'elle
// ---------------------------------------------------------------------------

#[test]
fn bouger_la_vitesse_pendant_une_animation_renvoie_l_animation_avec_le_reste_intact() {
    // Test d'intention n° 1 de l'issue : « un changement de vitesse pendant une animation produit
    // un `animate` portant le même nom et les mêmes autres réglages ». Critère d'acceptation :
    // « sans changer ni son nom ni sa couleur ni sa direction ».
    //
    // Le défaut d'avant #32 est muet : le curseur bouge, la propriété est écrite, et personne
    // n'envoie rien. La moitié « il part quelque chose » ne suffit donc pas — un `animate` qui
    // repartirait avec l'ancienne vitesse aurait exactement le même symptôme. Les deux moitiés du
    // critère sont vérifiées ensemble : la nouvelle vitesse arrive, et rien d'autre ne bouge.
    let avant = reglage(Some(AVEC_COULEUR), VITESSE_AVANT, DIRECTION_AVANT);
    let (nom_avant, paires_avant) = animate(&avant);
    let lu_avant = relu(&nom_avant, &paires_avant);

    let apres = reglage(Some(AVEC_COULEUR), VITESSE_APRES, DIRECTION_AVANT);
    let (nom_apres, paires_apres) = animate(&apres);
    let lu_apres = relu(&nom_apres, &paires_apres);

    assert_eq!(
        nom_apres, AVEC_COULEUR,
        "bouger la vitesse relance la même animation, pas une autre — reçu « {nom_apres} »"
    );
    assert_eq!(
        nom_avant, nom_apres,
        "le nom ne dépend pas de la position du curseur de vitesse"
    );
    assert_eq!(
        lu_apres.vitesse, VITESSE_APRES,
        "c'est la vitesse affichée qui part au démon, pas celle d'avant : {lu_apres:?}"
    );
    assert_eq!(
        lu_apres.couleur, COULEUR,
        "la couleur ne bouge pas quand on tire la vitesse : {lu_apres:?}"
    );
    assert_eq!(
        lu_apres.direction, DIRECTION_AVANT,
        "la direction ne bouge pas quand on tire la vitesse : {lu_apres:?}"
    );
    assert_eq!(
        (lu_avant.couleur, lu_avant.direction),
        (lu_apres.couleur, lu_apres.direction),
        "seule la vitesse a changé entre les deux commandes : {lu_avant:?} puis {lu_apres:?}"
    );
    assert_ne!(
        lu_avant.vitesse, lu_apres.vitesse,
        "deux positions du curseur donnent deux commandes différentes, sinon il ne sert à rien"
    );
}

// ---------------------------------------------------------------------------
// 2 — la direction s'applique tout de suite, et ne change qu'elle
// ---------------------------------------------------------------------------

#[test]
fn changer_la_direction_pendant_une_animation_renvoie_l_animation_avec_le_reste_intact() {
    // Test d'intention n° 2 de l'issue : « un changement de direction fait de même ». L'issue note
    // que « le menu déroulant de direction a exactement le même défaut » que le curseur de vitesse.
    //
    // Le piège propre à ce réglage : il voyage en **rang** dans `Direction::ALL` côté fenêtre et en
    // mot côté socket. Une conversion oubliée rend le rang zéro — `bas-haut` — pour toutes les
    // directions, ce qui ressemble à une animation qui marche. D'où une destination qui *est* le
    // rang zéro et un départ qui ne l'est pas : la commande d'après doit valoir `bas-haut`, celle
    // d'avant doit valoir autre chose.
    let avant = reglage(Some(AVEC_COULEUR), VITESSE_AVANT, DIRECTION_AVANT);
    let (nom_avant, paires_avant) = animate(&avant);
    let lu_avant = relu(&nom_avant, &paires_avant);

    let apres = reglage(Some(AVEC_COULEUR), VITESSE_AVANT, DIRECTION_APRES);
    let (nom_apres, paires_apres) = animate(&apres);
    let lu_apres = relu(&nom_apres, &paires_apres);

    assert_eq!(
        nom_apres, AVEC_COULEUR,
        "changer la direction relance la même animation — reçu « {nom_apres} »"
    );
    assert_eq!(
        lu_avant.direction,
        DIRECTION_AVANT,
        "le rang {} vaut {DIRECTION_AVANT:?} : {lu_avant:?}",
        rang(DIRECTION_AVANT)
    );
    assert_eq!(
        lu_apres.direction,
        DIRECTION_APRES,
        "le rang {} vaut {DIRECTION_APRES:?} : {lu_apres:?}",
        rang(DIRECTION_APRES)
    );
    assert_eq!(
        (lu_apres.vitesse, lu_apres.couleur),
        (VITESSE_AVANT, COULEUR),
        "ni la vitesse ni la couleur ne bougent quand on change la direction : {lu_apres:?}"
    );

    // Et les six rangs voyagent, pas seulement deux : une table de conversion tronquée passerait
    // les deux premiers et écraserait les suivants.
    for direction in Direction::ALL {
        let (nom, paires) = animate(&reglage(Some(AVEC_COULEUR), VITESSE_AVANT, direction));
        assert_eq!(
            relu(&nom, &paires).direction,
            direction,
            "le rang {} du menu déroulant vaut {direction:?} : {paires:?}",
            rang(direction)
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — à vide, les curseurs ne disent rien
// ---------------------------------------------------------------------------

#[test]
fn bouger_les_curseurs_sans_animation_en_cours_n_envoie_rien() {
    // Test d'intention n° 3 de l'issue : « un changement de vitesse sans animation en cours ne
    // produit aucune commande ». Contrat — `commande` : « `None` quand aucune animation ne tourne :
    // bouger la vitesse à vide ne doit rien envoyer, sinon la fenêtre démarrerait une animation que
    // personne n'a demandée ».
    //
    // C'est le symétrique exact du défaut réparé : une correction trop enthousiaste renverrait un
    // `animate` à chaque mouvement, et l'éclairage fixe que l'utilisateur a choisi se ferait
    // remplacer par une animation au premier frôlement du curseur — ou, pire, par un `animate off`
    // qui éteindrait tout.
    for vitesse in [VITESSE_AVANT, VITESSE_APRES] {
        for direction in Direction::ALL {
            let a_vide = reglage(None, vitesse, direction);
            assert_eq!(
                a_vide.commande(),
                None,
                "aucune animation ne tourne : rien ne part au démon — {a_vide:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — chaque animation ne reçoit que les clés qu'elle accepte
// ---------------------------------------------------------------------------

#[test]
fn chaque_animation_du_catalogue_ne_recoit_que_les_cles_qu_elle_accepte() {
    // Contrat — `commande` : « les clés portées sont **celles que l'animation accepte**, et elles
    // seules : `arc-en-ciel` refuse `couleur`, et la lui donner ferait refuser la commande
    // entière ».
    //
    // La faute ne se voit pas sur l'animation d'exemple, qui accepte tout. Elle se voit sur
    // `arc-en-ciel`, et elle ne dégrade pas la commande : elle l'annule. Le symptôme à l'usage est
    // celui-là même que #32 répare — un curseur qui ne fait rien — mais pour une autre cause, sur
    // une seule animation du catalogue. D'où un balayage complet plutôt qu'un cas.
    for nom in CATALOGUE {
        let regle = reglage(Some(*nom), VITESSE_AVANT, DIRECTION_AVANT);
        let (porte, paires) = animate(&regle);
        assert_eq!(porte, *nom, "« {nom} » se relance sous son propre nom");

        let acceptees = Animation::par_nom(nom)
            .unwrap_or_else(|e| panic!("« {nom} » est au catalogue : {e}"))
            .parametres_acceptes();
        for (cle, _) in &paires {
            assert!(
                acceptees.contains(&cle.as_str()),
                "« {nom} » n'accepte que {acceptees:?} : la clé « {cle} » ferait refuser la \
                 commande entière — {paires:?}"
            );
        }

        // Pas deux fois la même clé : le protocole les transporte telles quelles, et une paire en
        // double laisse au démon le soin de choisir laquelle compte.
        for (i, (cle, _)) in paires.iter().enumerate() {
            assert!(
                !paires.iter().skip(i + 1).any(|(autre, _)| autre == cle),
                "« {cle} » est portée deux fois par « {nom} » : {paires:?}"
            );
        }

        // Et ce qui est porté est bien ce qui est affiché : une clé acceptée mais laissée de côté
        // ferait retomber le démon sur son défaut, sans un mot.
        let lu = relu(nom, &paires);
        assert_eq!(
            (lu.vitesse, lu.direction),
            (VITESSE_AVANT, DIRECTION_AVANT),
            "« {nom} » reçoit la vitesse et la direction affichées : {paires:?}"
        );
        if acceptees.contains(&"couleur") {
            assert_eq!(
                lu.couleur, COULEUR,
                "« {nom} » accepte la couleur, donc il la reçoit : {paires:?}"
            );
        }
    }

    // Les deux bornes du balayage, nommées, pour qu'un échec dise laquelle est tombée.
    let (_, avec) = animate(&reglage(Some(AVEC_COULEUR), VITESSE_AVANT, DIRECTION_AVANT));
    assert!(
        valeur(&avec, "couleur").is_some(),
        "« {AVEC_COULEUR} » se colore : sa commande porte la couleur — {avec:?}"
    );
    let (_, sans) = animate(&reglage(Some(SANS_COULEUR), VITESSE_AVANT, DIRECTION_AVANT));
    assert_eq!(
        valeur(&sans, "couleur"),
        None,
        "« {SANS_COULEUR} » génère ses teintes et refuse « couleur » : la porter ferait rejeter \
         l'animate entier, et le curseur de vitesse n'aurait toujours aucun effet — {sans:?}"
    );
    assert!(
        valeur(&sans, "vitesse").is_some() && valeur(&sans, "direction").is_some(),
        "« {SANS_COULEUR} » accepte la vitesse et la direction : les omettre serait le défaut de \
         #32 sous un autre nom — {sans:?}"
    );
}

// ---------------------------------------------------------------------------
// 5 — une poignée neuve suit la mesure, et ne commande rien
// ---------------------------------------------------------------------------

#[test]
fn une_poignee_neuve_suit_la_mesure_et_ne_commande_rien() {
    // Contrat — `nouvelle` : « une poignée qui n'a encore rien vu : elle suit la mesure », et
    // `a_envoyer` : « rend `Some` au premier appel qui suit un **changement de consigne** ».
    //
    // Deux fautes symétriques sont visées. Une poignée qui ne suivrait plus la mesure figerait
    // l'affichage d'un ventilateur que personne ne touche — la télémétrie n'aurait plus d'effet
    // visible. Et une poignée qui prendrait une mesure pour une consigne renverrait au démon le
    // régime qu'il vient d'annoncer : un canal en automatique se retrouverait épinglé à la valeur
    // qu'il avait au démarrage de la fenêtre, sans que rien ne l'ait demandé.
    let mut poignee = Poignee::nouvelle();
    assert_eq!(
        poignee.a_envoyer(),
        None,
        "personne n'a touché cette poignée : elle n'a rien à demander"
    );

    for (i, mesure) in [MESURE_AU_REPOS, AUTRE_CONSIGNE, CONSIGNE, MESURE_AU_REPOS]
        .into_iter()
        .enumerate()
    {
        poignee.mesurer(mesure, DEBUT + CADENCE * i as u32);
        assert_eq!(
            poignee.affichee(),
            mesure,
            "aucune main sur la poignée : elle montre la mesure {mesure}"
        );
        assert_eq!(
            poignee.a_envoyer(),
            None,
            "la télémétrie n'est pas une consigne : la renvoyer au démon bouclerait sur elle-même"
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — une poignée tenue ne saute pas, quoi qu'annonce la mesure
// ---------------------------------------------------------------------------

#[test]
fn une_poignee_tenue_ne_saute_pas_quelle_que_soit_la_mesure() {
    // Critère d'acceptation : « une poignée de ventilateur tirée ne saute plus pendant qu'on la
    // tient ». Test d'intention n° 4 : « une consigne de ventilateur récente l'emporte sur la
    // mesure venue après elle ».
    //
    // L'issue décrit précisément la scène : « pendant qu'on tire la poignée, le modèle se rafraîchit
    // sous les doigts et la poignée saute à la valeur que le matériel rapporte ». Le point important
    // est que **tenir n'a pas de durée** : la grâce protège l'après-relâchement, pas le geste. Une
    // implémentation qui n'aurait qu'un délai — et pas d'état « tenue » — laisserait la poignée
    // sauter sous les doigts de quiconque prend plus de trois secondes à choisir son régime. D'où
    // une main qui tient dix fois la grâce.
    let mut poignee = Poignee::nouvelle();
    poignee.mesurer(MESURE_AU_REPOS, DEBUT);
    poignee.saisir(CONSIGNE, DEBUT);
    assert_eq!(
        poignee.affichee(),
        CONSIGNE,
        "la poignée est là où la main la pose, avant même la première mesure qui suit"
    );

    let tenue = GRACE * 10;
    let mut instant = DEBUT;
    while instant <= DEBUT + tenue {
        poignee.mesurer(MESURE_AU_REPOS, instant);
        assert_eq!(
            poignee.affichee(),
            CONSIGNE,
            "la main tient encore la poignée à {CONSIGNE} depuis {:?} : la mesure \
             {MESURE_AU_REPOS} ne la reprend pas",
            instant - DEBUT
        );
        instant += CADENCE;
    }

    // La grâce court depuis le **relâchement**, pas depuis la saisie : comptée depuis la saisie,
    // elle serait déjà expirée ici, et la poignée sauterait à la seconde suivante.
    poignee.relacher(DEBUT + tenue);
    poignee.mesurer(MESURE_AU_REPOS, DEBUT + tenue + CADENCE);
    assert_eq!(
        poignee.affichee(),
        CONSIGNE,
        "la grâce commence quand la main lâche, pas quand elle saisit"
    );
}

// ---------------------------------------------------------------------------
// 7 — après le relâchement, la consigne tient toute la grâce, puis rend la main
// ---------------------------------------------------------------------------

#[test]
fn apres_le_relachement_la_consigne_tient_toute_la_grace_puis_la_mesure_reprend() {
    // Critère d'acceptation : « elle ne saute pas non plus dans la seconde qui suit le relâchement,
    // le temps que le ventilateur rejoigne sa consigne ». Contrat — `relacher` : « la consigne
    // tient encore `GRACE` », et `GRACE` : « un ventilateur de 140 mm met environ deux secondes à
    // passer d'un régime à un autre, et la télémétrie arrive chaque seconde ».
    //
    // Les deux moitiés comptent autant, et ce sont deux fautes opposées. Une grâce qui ne
    // s'appliquerait jamais rendrait #32 à moitié réparée — la poignée sauterait à l'instant du
    // lâcher, quand le ventilateur n'a pas encore bougé. Une grâce éternelle est pire et
    // silencieuse : la poignée cesserait définitivement de rapporter l'état du matériel, et
    // afficherait la dernière consigne pour toujours, y compris quand le ventilateur en est loin.
    //
    // Voir le point n° 1 de l'en-tête pour la borne : à `relâchement + GRACE` pile, la grâce a fini
    // de tenir.
    let mut poignee = Poignee::nouvelle();
    poignee.mesurer(MESURE_AU_REPOS, DEBUT);
    poignee.saisir(CONSIGNE, DEBUT);
    poignee.relacher(DEBUT);

    for ecart in [Duration::ZERO, CADENCE, GRACE / 2, GRACE - INSTANT] {
        poignee.mesurer(MESURE_AU_REPOS, DEBUT + ecart);
        assert_eq!(
            poignee.affichee(),
            CONSIGNE,
            "{ecart:?} après le lâcher, la grâce ({GRACE:?}) court encore : le ventilateur \
             n'a pas fini d'accélérer, et la poignée reste sur la consigne {CONSIGNE}"
        );
    }

    poignee.mesurer(MESURE_AU_REPOS, DEBUT + GRACE);
    assert_eq!(
        poignee.affichee(),
        MESURE_AU_REPOS,
        "la consigne a tenu {GRACE:?} : à cet instant elle a fini de tenir, et la poignée rapporte \
         de nouveau ce que le matériel fait ({MESURE_AU_REPOS})"
    );

    // Et elle continue de le rapporter : la grâce ne se rallume pas toute seule.
    poignee.mesurer(AUTRE_CONSIGNE, DEBUT + GRACE + CADENCE);
    assert_eq!(
        poignee.affichee(),
        AUTRE_CONSIGNE,
        "la grâce est finie : chaque mesure suivante s'affiche"
    );

    // Un second geste réarme une grâce entière — sans quoi la deuxième correction d'un régime
    // sauterait là où la première tenait.
    let second = DEBUT + GRACE * 2;
    poignee.saisir(CONSIGNE, second);
    poignee.relacher(second);
    poignee.mesurer(MESURE_AU_REPOS, second + GRACE - INSTANT);
    assert_eq!(
        poignee.affichee(),
        CONSIGNE,
        "la grâce du second geste vaut celle du premier"
    );
    poignee.mesurer(MESURE_AU_REPOS, second + GRACE);
    assert_eq!(poignee.affichee(), MESURE_AU_REPOS, "et elle expire pareil");
}

// ---------------------------------------------------------------------------
// 8 — rendre le canal au firmware délie la poignée tout de suite
// ---------------------------------------------------------------------------

#[test]
fn liberer_un_canal_rend_la_main_a_la_mesure_sans_attendre_la_grace() {
    // Critère d'acceptation : « rendre un canal au firmware délie la poignée : elle suit de nouveau
    // la mesure ». Test d'intention n° 5 : « passer un canal en automatique rend la main à la
    // mesure ». Contrat — `liberer` : « la mesure reprend la main **tout de suite**, sans attendre
    // la fin de la grâce ».
    //
    // Sans ce chemin, cliquer « auto » laisserait la poignée figée sur l'ancienne consigne pendant
    // toute la grâce, alors que le ventilateur est déjà reparti sur la courbe du firmware : le seul
    // retour visible du passage en automatique mentirait, précisément pendant les secondes où on le
    // regarde. Et voir le point n° 3 de l'en-tête pour la consigne en attente : partie après le
    // `fan auto`, elle ressortirait le canal de l'automatique qu'on vient de demander.
    let mut poignee = Poignee::nouvelle();
    poignee.mesurer(MESURE_AU_REPOS, DEBUT);
    poignee.saisir(CONSIGNE, DEBUT);
    poignee.relacher(DEBUT);
    poignee.liberer();

    // Entre le clic sur « auto » et la télémétrie qui suit, il peut s'écouler une seconde entière.
    // Laquelle des deux valeurs connues la poignée montre pendant ce temps est un détail que le
    // contrat ne tranche pas — mais elle n'a pas le droit d'en inventer une troisième. Une poignée
    // remise à zéro « en attendant » plongerait à fond de barre sous les yeux de celui qui vient de
    // rendre le canal au firmware, pour se relever à la mesure suivante.
    let entre_deux = poignee.affichee();
    assert!(
        entre_deux == CONSIGNE || entre_deux == MESURE_AU_REPOS,
        "juste après « auto », la poignée montre ce qu'elle sait — la consigne {CONSIGNE} ou la \
         dernière mesure {MESURE_AU_REPOS} — et pas {entre_deux}"
    );

    poignee.mesurer(MESURE_AU_REPOS, DEBUT + GRACE / 2);
    assert_eq!(
        poignee.affichee(),
        MESURE_AU_REPOS,
        "le canal est repassé au firmware : la poignée suit la mesure sans attendre la fin de la \
         grâce"
    );
    assert_eq!(
        poignee.a_envoyer(),
        None,
        "une consigne partie après le « fan auto » ressortirait le canal de l'automatique"
    );

    // Même chose la main encore posée : « auto » délie, quel que soit l'état du geste. Le canal
    // suit désormais la courbe du firmware, donc la poignée n'a plus rien à défendre.
    let mut tenue = Poignee::nouvelle();
    tenue.mesurer(MESURE_AU_REPOS, DEBUT);
    tenue.saisir(CONSIGNE, DEBUT);
    tenue.liberer();
    tenue.mesurer(AUTRE_CONSIGNE, DEBUT + CADENCE);
    assert_eq!(
        tenue.affichee(),
        AUTRE_CONSIGNE,
        "libéré, le canal n'obéit plus à la poignée : elle montre ce que le firmware fait"
    );

    // Et reprendre la poignée reprend le canal : `liberer` délie, il ne condamne pas.
    tenue.saisir(CONSIGNE, DEBUT + CADENCE * 2);
    assert_eq!(
        tenue.affichee(),
        CONSIGNE,
        "on peut re-saisir une poignée qu'on avait rendue au firmware"
    );
    assert_eq!(
        tenue.a_envoyer(),
        Some(CONSIGNE),
        "et ce nouveau geste est une consigne à envoyer"
    );
}

// ---------------------------------------------------------------------------
// 9 — un glissement continu ne produit qu'une commande par pas
// ---------------------------------------------------------------------------

#[test]
fn un_glissement_continu_ne_produit_qu_une_commande_par_pas_de_valeur() {
    // Critère d'acceptation : « un déplacement continu n'inonde pas le démon : au plus une commande
    // par pas de valeur ». Contrat — `a_envoyer` : « rend `Some` au premier appel qui suit un
    // changement de consigne, `None` ensuite : un glissement continu produit une commande par pas
    // franchi, pas une par image ».
    //
    // Trois fautes distinctes sont visées, et aucune ne se voit à l'œil sur la fenêtre :
    // `a_envoyer` qui rendrait toujours `Some` — une commande par image, soit des dizaines par
    // seconde pour un ventilateur qui met deux secondes à réagir ; `a_envoyer` qui rendrait
    // toujours `None` — le curseur redeviendrait décoratif ; et la coalescence qui garderait la
    // **première** valeur d'une rafale au lieu de la dernière, ce qui laisserait le ventilateur à
    // un régime que la poignée n'affiche plus. Voir aussi le point n° 2 de l'en-tête : reposer la
    // poignée où elle est déjà n'est pas un pas.
    let mut poignee = Poignee::nouvelle();
    poignee.mesurer(MESURE_AU_REPOS, DEBUT);

    // Le glissement : de la mesure au repos jusqu'à la consigne, un pas à la fois. La fenêtre
    // repasse plusieurs fois par pas — c'est exactement ce qui inondait le démon.
    const IMAGES_PAR_PAS: usize = 3;
    let mut envoyees = Vec::new();
    for (i, valeur) in (MESURE_AU_REPOS..=CONSIGNE).enumerate() {
        // Une main traverse une barre plus vite que la télémétrie n'arrive : les pas se suivent à
        // un instant d'intervalle, tous très en deçà de la cadence.
        poignee.saisir(valeur, DEBUT + INSTANT * i as u32);
        assert_eq!(
            poignee.affichee(),
            valeur,
            "la poignée suit la main pas à pas"
        );
        for _ in 0..IMAGES_PAR_PAS {
            if let Some(consigne) = poignee.a_envoyer() {
                envoyees.push(consigne);
            }
        }
    }
    let attendues: Vec<u8> = (MESURE_AU_REPOS..=CONSIGNE).collect();
    assert_eq!(
        envoyees,
        attendues,
        "un glissement de {MESURE_AU_REPOS} à {CONSIGNE} vaut une commande par pas, dans l'ordre, \
         et pas une par image : {} commandes pour {} pas",
        envoyees.len(),
        attendues.len()
    );

    // La fenêtre peut aussi être en retard sur la souris : plusieurs pas franchis entre deux
    // images. Une seule commande en sort, et c'est la **dernière** valeur — une valeur
    // intermédiaire laisserait le ventilateur là où la poignée n'est plus.
    let rafale = DEBUT + CADENCE;
    for (i, valeur) in [CONSIGNE - 3, CONSIGNE - 2, AUTRE_CONSIGNE]
        .into_iter()
        .enumerate()
    {
        poignee.saisir(valeur, rafale + INSTANT * i as u32);
    }
    assert_eq!(
        poignee.a_envoyer(),
        Some(AUTRE_CONSIGNE),
        "trois pas franchis entre deux images valent une commande, celle de la dernière valeur"
    );
    assert_eq!(
        poignee.a_envoyer(),
        None,
        "et la commande ne se répète pas à l'image suivante"
    );

    // Reposer la poignée là où elle est déjà n'est pas un pas : sans cette règle, une interface qui
    // appelle `saisir` à chaque image respecterait la lettre du critère en inondant le démon.
    poignee.saisir(AUTRE_CONSIGNE, rafale + CADENCE);
    assert_eq!(
        poignee.a_envoyer(),
        None,
        "la consigne n'a pas changé : il n'y a rien de nouveau à demander"
    );

    // Lâcher n'est pas une consigne de plus — le démon a déjà reçu la valeur au dernier pas.
    poignee.relacher(rafale + CADENCE);
    assert_eq!(
        poignee.a_envoyer(),
        None,
        "relâcher ne renvoie pas une consigne déjà envoyée"
    );

    // Mais un nouveau geste réarme : `a_envoyer` désarme la consigne, il ne condamne pas la
    // poignée.
    poignee.saisir(CONSIGNE, rafale + CADENCE * 2);
    assert_eq!(
        poignee.a_envoyer(),
        Some(CONSIGNE),
        "un pas franchi après coup se commande comme les autres"
    );
}

// ---------------------------------------------------------------------------
// 10 — deux ventilateurs se règlent indépendamment
// ---------------------------------------------------------------------------

#[test]
fn deux_poignees_ne_partagent_ni_leur_consigne_ni_leur_grace() {
    // Contrat — module : « c'est un état par canal, pas un verrou global — deux ventilateurs se
    // règlent indépendamment ».
    //
    // Un état global marcherait parfaitement tant qu'on ne touche qu'un curseur, et le boîtier en a
    // plusieurs : régler un ventilateur figerait l'affichage de tous les autres pendant la grâce,
    // et la télémétrie des canaux qu'on ne touche pas cesserait d'apparaître le temps de chaque
    // geste. C'est visible à l'usage et invisible dans un test qui n'instancie qu'une poignée.
    let mut tiree = Poignee::nouvelle();
    let mut libre = Poignee::nouvelle();
    tiree.mesurer(MESURE_AU_REPOS, DEBUT);
    libre.mesurer(MESURE_AU_REPOS, DEBUT);

    tiree.saisir(CONSIGNE, DEBUT);
    tiree.relacher(DEBUT);

    tiree.mesurer(MESURE_AU_REPOS, DEBUT + CADENCE);
    libre.mesurer(AUTRE_CONSIGNE, DEBUT + CADENCE);

    assert_eq!(
        tiree.affichee(),
        CONSIGNE,
        "la poignée qu'on vient de lâcher tient sa consigne"
    );
    assert_eq!(
        libre.affichee(),
        AUTRE_CONSIGNE,
        "celle du ventilateur d'à côté suit sa propre mesure : régler un canal n'en fige pas un \
         autre"
    );
    assert_eq!(
        libre.a_envoyer(),
        None,
        "et un ventilateur que personne n'a touché ne commande rien"
    );
    assert_eq!(
        tiree.a_envoyer(),
        Some(CONSIGNE),
        "tandis que celui qu'on a tiré a bien une consigne à envoyer"
    );
}
