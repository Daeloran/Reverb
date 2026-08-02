//! Tests d'intention du limiteur de débit des trois jauges (issue #47).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #47 seule — aucun fichier de
//! `crates/reverb-gui/src/` n'a été lu pour les produire, hors les signatures publiques des types
//! du protocole. À l'écriture de ce fichier, `Limiteur`, `INTERVALLE` et `requetes_vers_la_cible`
//! **n'existent pas** : la compilation de ce test doit échouer, et c'est la phase rouge.
//!
//! Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne dort : le temps est **injecté**, chaque
//! méthode qui en dépend reçoit son instant. Un test qui appellerait `Instant::now()` mesurerait
//! l'ordonnanceur de la machine, pas le limiteur — et ne dirait plus la même chose deux fois.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Avant #47, bouger la teinte, la saturation ou la luminosité ne touchait que l'aperçu et le code
//! hexadécimal : rien ne partait sur le socket. Seule la pastille de couleur prédéfinie appelait
//! l'application. **C'est le défaut le plus silencieux du lot** — un aperçu qui suit le doigt en
//! direct promet que les LED suivent aussi, et la fenêtre a l'air de marcher.
//!
//! Le corriger naïvement — un ordre par image de curseur — remplacerait un défaut par un autre : un
//! curseur traîné émet des dizaines de valeurs par seconde, là où le démon en encaisse **31 au
//! mieux** et **20 quand toutes ses cibles changent** (README, mesuré sur SHYNAEL). Poser une
//! couleur unie fait justement changer les quatorze cibles : c'est le cas à 20 images par seconde,
//! pas celui à 100. D'où un limiteur, et d'où des tests qui exigent les **deux** moitiés du
//! contrat : que le flot soit borné, et que la dernière valeur parte quand même.
//!
//! ## Ce que ce fichier fige
//!
//! 1. **Au plus une couleur par intervalle.** Deux valeurs proposées à moins d'un [`INTERVALLE`]
//!    d'écart n'en font partir qu'une ; deux valeurs séparées d'au moins un intervalle en font
//!    partir deux.
//! 2. **La dernière valeur d'une rafale finit toujours par partir.** C'est le critère qui compte le
//!    plus : relâcher le curseur sur une couleur ne doit jamais laisser les LED sur la précédente.
//! 3. **Une couleur déjà envoyée n'est pas renvoyée.** Le matériel n'a pas de watchdog : réécrire
//!    une couleur identique ne fait que consommer du bus.
//! 4. **Le limiteur ne rend jamais une couleur qu'on ne lui a pas donnée** — ni une interpolation,
//!    ni une valeur intermédiaire de la rafale, ni la précédente.
//! 5. **La cible obéit à la règle du bouton « Appliquer ».** Une zone visée reçoit
//!    [`Request::ZoneLight`] à son nom ; sans zone, ce sont les requêtes du bouton, rendues telles
//!    quelles.
//!
//! ## Ce que le contrat laisse ouvert, et que ces tests tranchent
//!
//! 1. **La première couleur part sans attendre.** Un limiteur neuf n'a rien envoyé : il n'a donc
//!    aucune raison de retenir. Un limiteur qui commencerait par un intervalle de silence ferait
//!    démarrer chaque glissement par un temps mort, exactement l'impression que #47 corrige.
//! 2. **À l'instant précis où l'intervalle est écoulé, la couleur part.** L'intervalle est un délai
//!    minimal entre deux envois, pas un délai strictement dépassé : la comparaison est un `>=`. Un
//!    `>` ne changerait rien à l'usage, mais une borne non pinnée laisserait passer les deux fautes
//!    qui comptent — un limiteur qui n'envoie jamais rien, et un limiteur qui ne limite rien.
//! 3. **`a_envoyer` respecte l'intervalle comme `proposer`.** Sans quoi « au plus une requête par
//!    intervalle » ne serait plus une propriété du limiteur mais une politesse de son appelant :
//!    une fenêtre qui viderait l'attente à chaque image inonderait le démon tout en respectant la
//!    lettre du critère. La conséquence est sans danger : l'attente n'est pas perdue, elle reste
//!    en attente, et l'horloge d'une seconde déjà en place ([`HORLOGE`]) la trouvera toujours
//!    largement au-delà de l'intervalle.
//! 4. **Le dédoublonnage vaut aussi pour `proposer`, pas seulement pour la vidange.** L'issue ne le
//!    dit que de la rafale, mais la règle est la même des deux côtés : si la couleur qui partirait
//!    est celle qui est déjà posée, il n'y a rien de nouveau à envoyer. Sans cette règle, une
//!    fenêtre qui republierait sa couleur courante à chaque image produirait un ordre par
//!    intervalle sans que rien n'ait bougé.
//! 5. **Le repli n'est pas consulté quand une zone est visée.** C'est ce qui rend observable la
//!    faute décrite par l'issue — « ça passe tout en fixe et couleur unie » alors qu'une zone
//!    était visée. Une implémentation qui calculerait les deux et choisirait ensuite passerait un
//!    test d'égalité ; elle ne passe pas un test qui compte les appels.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Le sélecteur de couleur lui-même** — axes, aperçu, dégradés. L'issue le met hors scope : il
//!   fonctionne, c'est son branchement qui manque.
//! - **Le champ hexadécimal**, qui garde son comportement : il n'applique qu'à la validation. Rien
//!   ici ne passe par le limiteur, donc rien ici n'a de contrat à figer.
//! - **Ce que `Request::Light` fait au démon** — arrêter l'animation en cours. C'est voulu, et
//!   c'est de l'autre côté du socket.
//! - **Un nom de zone vide**, et **un temps qui reculerait**. La fenêtre ne propose que des zones
//!   qu'elle liste, et l'horloge d'où vient l'instant est monotone. Inventer un comportement pour
//!   ces deux cas figerait une règle que personne n'a choisie.
//! - **La valeur exacte de [`INTERVALLE`]**, dont seules les bornes utiles sont vérifiées. La
//!   valeur retenue est **50 ms**, soit vingt requêtes par seconde : c'est la cadence du démon
//!   quand ses quatorze cibles changent — 29,5 ms de trames HID plus 21,6 ms de blocs SMBus, un
//!   plancher physique et non logiciel (README). Une couleur unie posée sur tout le boîtier est
//!   exactement ce cas-là. Le test encadre plutôt qu'il ne pointe, pour qu'un ajustement mesuré
//!   sur le matériel n'ait pas à se négocier avec un test d'intention.

use std::cell::RefCell;
use std::time::Duration;

use reverb_gui::reglages::{INTERVALLE, Limiteur, requetes_vers_la_cible};
use reverb_proto::ipc::{LightTarget, Request};
use reverb_proto::{Position, Rgb};

// ---------------------------------------------------------------------------
// Repères et aides
// ---------------------------------------------------------------------------

/// L'origine du temps injecté.
///
/// Volontairement loin de zéro : la fenêtre est ouverte depuis un moment quand une main touche
/// enfin une jauge. Un limiteur qui prendrait `Duration::ZERO` pour « rien n'a encore été envoyé »
/// se ferait attraper ici plutôt qu'à l'usage.
const DEBUT: Duration = Duration::from_secs(3_600);

/// Le plus petit écart que le temps injecté sache représenter.
///
/// Sert à encadrer la borne de [`INTERVALLE`] sans laisser d'espace entre l'essai d'avant et celui
/// d'après, et à serrer les valeurs d'une rafale plus près que n'importe quelle main.
const INSTANT: Duration = Duration::from_nanos(1);

/// L'écart entre deux images de la fenêtre pendant un glissement : soixante par seconde.
///
/// C'est la cadence qui inonde le démon, et donc celle qu'il faut donner au limiteur pour que le
/// test décrive le geste réel plutôt qu'un cas de laboratoire.
const IMAGE: Duration = Duration::from_millis(16);

/// L'horloge qui vide ce qui reste en attente : « l'horloge d'une seconde déjà en place vide ce qui
/// reste en attente » (issue #47).
const HORLOGE: Duration = Duration::from_secs(1);

/// La cadence du démon dans son **meilleur** cas mesuré : 31 images par seconde, quand la plupart
/// des cibles n'ont pas bougé (README). Proposer plus vite que ça n'est jamais utile.
const MEILLEUR_CAS: Duration = Duration::from_millis(32);

/// La cadence en deçà de laquelle une jauge cesserait de « suivre le doigt » : dix par seconde. Le
/// README tient vingt pour continu à l'œil ; dix est la dernière valeur qu'on puisse défendre.
const PIRE_CAS_ACCEPTABLE: Duration = Duration::from_millis(100);

/// La première couleur d'un geste.
const PREMIERE: Rgb = Rgb::new(0x12, 0x9a, 0x40);

/// Une couleur intermédiaire — celle qu'une implémentation qui garderait la **première** valeur
/// d'une rafale, ou qui rendrait la **précédente**, laisserait sur les LED.
const INTERMEDIAIRE: Rgb = Rgb::new(0x7f, 0x21, 0xc3);

/// Une seconde couleur intermédiaire, pour qu'une rafale ait vraiment un milieu.
const AUTRE_INTERMEDIAIRE: Rgb = Rgb::new(0x05, 0xee, 0x18);

/// La couleur sur laquelle la main relâche la jauge. C'est celle qui doit finir sur les LED.
const FINALE: Rgb = Rgb::new(0xd4, 0x60, 0x03);

/// Une couleur de plus, pour vérifier qu'un limiteur qui vient de servir n'est pas condamné.
const SUIVANTE: Rgb = Rgb::new(0x33, 0x33, 0xa8);

/// La zone visée par le pupitre.
const ZONE: &str = "cockpit";

/// Une seconde zone : un nom porté vaut mieux qu'un nom codé en dur, et seul un second nom le
/// prouve.
const AUTRE_ZONE: &str = "plancher";

/// Ce que le bouton « Appliquer » produit aujourd'hui sans zone visée, avec une sélection de LED.
///
/// Le contenu exact n'est pas le sujet : `commandes_de_couleur` en décide, et
/// `requetes_vers_la_cible` doit le rendre **tel quel**. Ce qui compte, c'est que cette liste soit
/// reconnaissable — plusieurs éléments, plusieurs verbes, un ordre — pour qu'une implémentation qui
/// la réordonnerait, la tronquerait ou la remplacerait par un `light all` se voie.
fn requetes_du_bouton(couleur: Rgb) -> Vec<Request> {
    vec![
        Request::Paint {
            target: LightTarget::Fan(Position::Arriere),
            couleurs: vec![couleur; 8],
        },
        Request::Paint {
            target: LightTarget::RamSlot(2),
            couleurs: vec![couleur; 11],
        },
        Request::Light {
            target: LightTarget::Fan(Position::HautMilieu),
            color: couleur,
        },
    ]
}

/// Ce que le bouton produit quand rien n'est sélectionné : une couleur pour tout le boîtier.
fn requete_du_boitier(couleur: Rgb) -> Vec<Request> {
    vec![Request::Light {
        target: LightTarget::All,
        color: couleur,
    }]
}

/// Les couleurs passées au repli, dans l'ordre où il les a reçues.
///
/// Compter les appels et non seulement comparer les sorties : une implémentation qui calcule les
/// requêtes du boîtier **puis** les jette au profit d'une `ZoneLight` rendrait le bon résultat tout
/// en faisant travailler la fenêtre pour rien — et surtout, elle rendrait indétectable la faute
/// symétrique, celle qui calcule les deux et garde la mauvaise.
#[derive(Default)]
struct Journal(RefCell<Vec<Rgb>>);

impl Journal {
    fn neuf() -> Journal {
        Journal::default()
    }

    fn noter(&self, couleur: Rgb) {
        self.0.borrow_mut().push(couleur);
    }

    fn appels(&self) -> Vec<Rgb> {
        self.0.borrow().clone()
    }
}

/// Une couleur du glissement, à son pas `i`.
///
/// Toutes distinctes deux à deux sur les 128 premiers pas — la composante rouge suffit à le
/// garantir. C'est nécessaire : deux couleurs égales feraient jouer le dédoublonnage et brouilleraient
/// le comptage des envois.
fn couleur_du_pas(i: u8) -> Rgb {
    Rgb::new(i, 0x40u8.wrapping_add(i / 2), 0xff - i)
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tous les tests qui suivent supposent que leurs couleurs diffèrent, que les écarts injectés
    // sont bien de part et d'autre de l'intervalle, et que les deux zones portent des noms
    // distincts. Si l'un de ces repères se dégradait, plusieurs tests deviendraient vrais sans
    // rien vérifier — et personne ne le verrait. Ce test est là pour que la panne soit ici.
    let couleurs = [
        ("PREMIERE", PREMIERE),
        ("INTERMEDIAIRE", INTERMEDIAIRE),
        ("AUTRE_INTERMEDIAIRE", AUTRE_INTERMEDIAIRE),
        ("FINALE", FINALE),
        ("SUIVANTE", SUIVANTE),
    ];
    for (i, (nom, couleur)) in couleurs.iter().enumerate() {
        assert_ne!(
            *couleur,
            Rgb::BLACK,
            "{nom} doit être visible : le noir est aussi ce qu'une couleur non initialisée vaut"
        );
        for (autre_nom, autre) in couleurs.iter().skip(i + 1) {
            assert_ne!(
                couleur, autre,
                "{nom} et {autre_nom} doivent différer, sinon le dédoublonnage rendrait vrais des \
                 tests qui ne vérifient rien"
            );
        }
    }

    assert_ne!(ZONE, AUTRE_ZONE, "deux zones, deux noms");
    assert!(
        !ZONE.is_empty() && !AUTRE_ZONE.is_empty(),
        "une zone porte un nom"
    );

    // Les cent vingt-huit couleurs du glissement, toutes distinctes : sans quoi le dédoublonnage
    // ferait chuter le compte des envois et le test de volume passerait pour de mauvaises raisons.
    let mut vues: Vec<Rgb> = Vec::new();
    for i in 0..=127u8 {
        let couleur = couleur_du_pas(i);
        assert!(
            !vues.contains(&couleur),
            "le pas {i} répète une couleur déjà vue : {couleur:?}"
        );
        vues.push(couleur);
    }

    // L'intervalle, encadré par les deux cadences mesurées du README. En deçà de 32 ms, le limiteur
    // proposerait plus vite que le démon n'a jamais su encaisser, même dans son meilleur cas —
    // il ne limiterait rien d'utile. Au-delà de 100 ms, la jauge cesserait de suivre le doigt, et
    // #47 se serait déplacée au lieu de se résoudre.
    assert!(
        INTERVALLE >= MEILLEUR_CAS,
        "un intervalle de {INTERVALLE:?} laisse passer plus que les 31 images par seconde du \
         meilleur cas mesuré : il ne limite rien"
    );
    assert!(
        INTERVALLE <= PIRE_CAS_ACCEPTABLE,
        "un intervalle de {INTERVALLE:?} met la jauge en dessous de dix envois par seconde : \
         l'aperçu promettrait encore ce que les LED ne tiendraient pas"
    );
    assert!(
        INTERVALLE < HORLOGE,
        "l'horloge d'une seconde vide l'attente : un intervalle plus long qu'elle ferait sauter \
         des vidanges"
    );
    assert!(
        IMAGE < INTERVALLE && INSTANT < INTERVALLE,
        "les écarts d'une rafale doivent tomber en deçà de l'intervalle, sinon les tests de \
         fusion n'en testent aucune"
    );
}

// ---------------------------------------------------------------------------
// 1 — deux couleurs dans le même intervalle n'en envoient qu'une
// ---------------------------------------------------------------------------

#[test]
fn deux_couleurs_dans_le_meme_intervalle_n_en_envoient_qu_une() {
    // Critère d'acceptation : « une suite de valeurs de curseur émises plus vite que la cadence
    // retenue produit **au plus une** requête par intervalle ».
    //
    // La faute visée est celle qu'on écrit en corrigeant #47 sans y penser : brancher les trois
    // rappels d'axe directement sur le socket. La fenêtre marcherait, et le démon recevrait
    // soixante ordres par seconde là où il en tient vingt.
    let mut limiteur = Limiteur::nouveau();

    assert_eq!(
        limiteur.proposer(PREMIERE, DEBUT),
        Some(PREMIERE),
        "un limiteur neuf n'a rien envoyé : il n'a aucune raison de retenir la première couleur"
    );
    assert_eq!(
        limiteur.proposer(INTERMEDIAIRE, DEBUT + Duration::from_millis(1)),
        None,
        "une milliseconde après le premier envoi, la seconde couleur est retenue"
    );
    assert_eq!(
        limiteur.proposer(AUTRE_INTERMEDIAIRE, DEBUT + IMAGE),
        None,
        "une image plus tard non plus : l'intervalle n'est pas écoulé"
    );

    // La borne, serrée par le dessous au plus petit écart représentable : à un instant de
    // l'intervalle, on retient encore.
    assert_eq!(
        limiteur.proposer(FINALE, DEBUT + INTERVALLE - INSTANT),
        None,
        "l'intervalle n'est pas encore écoulé : rien ne part"
    );

    // Et la vidange n'est pas une porte dérobée : elle obéit au même intervalle, sans quoi
    // « au plus une requête par intervalle » ne dépendrait plus du limiteur mais de son appelant.
    assert_eq!(
        limiteur.a_envoyer(DEBUT + INTERVALLE - INSTANT),
        None,
        "vider l'attente avant l'intervalle contournerait le limiteur"
    );
}

// ---------------------------------------------------------------------------
// 2 — deux couleurs séparées par l'intervalle partent toutes les deux
// ---------------------------------------------------------------------------

#[test]
fn deux_couleurs_separees_par_l_intervalle_partent_toutes_les_deux() {
    // Le pendant du test précédent, et la faute symétrique : un limiteur qui retiendrait tout
    // laisserait les jauges aussi décoratives qu'avant #47, mais avec du code en plus.
    //
    // La borne est pinnée à l'instant exact où l'intervalle est écoulé — voir le point n° 2 de
    // l'en-tête : l'intervalle est un délai minimal entre deux envois, la comparaison est un `>=`.
    let mut limiteur = Limiteur::nouveau();

    assert_eq!(
        limiteur.proposer(PREMIERE, DEBUT),
        Some(PREMIERE),
        "la première couleur part sans attendre"
    );
    assert_eq!(
        limiteur.proposer(INTERMEDIAIRE, DEBUT + INTERVALLE),
        Some(INTERMEDIAIRE),
        "un intervalle plein s'est écoulé : la couleur part"
    );
    assert_eq!(
        limiteur.proposer(FINALE, DEBUT + INTERVALLE * 2 + INSTANT),
        Some(FINALE),
        "et l'intervalle court depuis le dernier envoi, pas depuis le premier"
    );

    // Trois couleurs proposées à un intervalle d'écart valent trois envois : un limiteur qui
    // compterait les appels au lieu de mesurer le temps s'arrêterait ici.
    assert_eq!(
        limiteur.proposer(SUIVANTE, DEBUT + INTERVALLE * 3 + INSTANT),
        Some(SUIVANTE),
        "un glissement lent n'est pas une rafale : chaque valeur part"
    );
}

// ---------------------------------------------------------------------------
// 3 — la dernière couleur d'une rafale finit toujours par partir
// ---------------------------------------------------------------------------

#[test]
fn la_derniere_couleur_d_une_rafale_finit_toujours_par_partir() {
    // Critère d'acceptation : « la **dernière** valeur d'une rafale est toujours envoyée, même si
    // elle tombe dans un intervalle déjà servi — relâcher le curseur sur une couleur ne doit jamais
    // laisser les LED sur la précédente ».
    //
    // C'est le critère qui compte le plus, et la faute qu'il interdit est la plus vicieuse du lot :
    // un limiteur qui **jette** ce qu'il retient au lieu de le garder laisse le boîtier sur une
    // couleur que la fenêtre n'affiche plus. L'écart est permanent, silencieux, et il ne se
    // reproduit qu'un geste sur deux.
    let mut limiteur = Limiteur::nouveau();

    assert_eq!(
        limiteur.proposer(PREMIERE, DEBUT),
        Some(PREMIERE),
        "le début du geste part tout de suite"
    );
    for (i, couleur) in [INTERMEDIAIRE, AUTRE_INTERMEDIAIRE, SUIVANTE, FINALE]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            limiteur.proposer(couleur, DEBUT + INSTANT * (i as u32 + 1)),
            None,
            "la rafale tient dans un intervalle : rien ne part en cours de route"
        );
    }

    // L'attente n'est ni perdue ni servie trop tôt.
    assert_eq!(
        limiteur.a_envoyer(DEBUT + INSTANT * 5),
        None,
        "l'intervalle n'est pas écoulé : l'attente reste en attente"
    );
    assert_eq!(
        limiteur.a_envoyer(DEBUT + INTERVALLE),
        Some(FINALE),
        "c'est la DERNIÈRE couleur de la rafale qui part, pas la première ni une du milieu"
    );

    // Une fois servie, l'attente est vide : un limiteur qui la garderait renverrait la même couleur
    // à chaque tour d'horloge, pour toujours.
    assert_eq!(
        limiteur.a_envoyer(DEBUT + INTERVALLE + HORLOGE),
        None,
        "l'attente a été servie : il n'y a plus rien à envoyer"
    );
    assert_eq!(
        limiteur.a_envoyer(DEBUT + INTERVALLE + HORLOGE * 2),
        None,
        "et elle ne repousse pas toute seule au tour d'horloge suivant"
    );

    // La vidange compte comme un envoi : sinon l'intervalle repartirait du dernier `proposer`, et
    // deux ordres se suivraient à un instant d'écart.
    assert_eq!(
        limiteur.proposer(PREMIERE, DEBUT + INTERVALLE + INSTANT),
        None,
        "la vidange vient d'envoyer : l'intervalle court depuis elle"
    );
    assert_eq!(
        limiteur.proposer(INTERMEDIAIRE, DEBUT + INTERVALLE * 2),
        Some(INTERMEDIAIRE),
        "et le limiteur n'est pas condamné : le geste suivant repart"
    );
}

// ---------------------------------------------------------------------------
// 4 — un glissement continu ne dépasse pas une requête par intervalle
// ---------------------------------------------------------------------------

#[test]
fn un_glissement_continu_reste_borne_et_finit_sur_la_derniere_couleur() {
    // Les deux moitiés du contrat, mesurées sur le geste réel plutôt que sur trois appels : deux
    // secondes de jauge traînée, une valeur toutes les seize millisecondes. C'est ce que la fenêtre
    // produit, et c'est ce qui noyait le démon.
    //
    // Trois fautes sont visées d'un coup : le limiteur muet — qui n'envoie que la première ; le
    // limiteur passant — qui laisse tout filer ; et le limiteur oublieux — qui borne le flot mais
    // perd la valeur sur laquelle la main s'arrête.
    let mut limiteur = Limiteur::nouveau();
    const PAS: u8 = 125;

    let mut envoyees: Vec<Rgb> = Vec::new();
    for i in 0..PAS {
        let couleur = couleur_du_pas(i);
        if let Some(partie) = limiteur.proposer(couleur, DEBUT + IMAGE * u32::from(i)) {
            assert_eq!(
                partie, couleur,
                "ce que `proposer` rend est la couleur qu'on vient de lui donner, jamais une autre"
            );
            envoyees.push(partie);
        }
    }
    let fin = DEBUT + IMAGE * u32::from(PAS - 1);
    let duree = fin - DEBUT;

    // Borne haute : le premier envoi, plus au plus un par intervalle écoulé.
    let plafond = duree.as_millis() / INTERVALLE.as_millis() + 1;
    assert!(
        envoyees.len() as u128 <= plafond,
        "{} envois pour {duree:?} de glissement : le limiteur en autorise {plafond} au plus, un \
         par intervalle de {INTERVALLE:?}",
        envoyees.len()
    );

    // Borne basse : au moins un envoi tous les deux intervalles. Un limiteur qui n'enverrait que la
    // première couleur respecterait la borne haute sans rien corriger de #47.
    assert!(
        (envoyees.len() as u128) * INTERVALLE.as_millis() * 2 >= duree.as_millis(),
        "{} envois pour {duree:?} : une jauge traînée doit suivre, pas s'arrêter au premier ordre",
        envoyees.len()
    );

    // Aucune couleur inventée en chemin, et l'ordre du geste est conservé.
    let mut rang = 0usize;
    for envoyee in &envoyees {
        let position = (rang..usize::from(PAS))
            .find(|i| couleur_du_pas(*i as u8) == *envoyee)
            .unwrap_or_else(|| {
                panic!(
                    "{envoyee:?} n'a jamais été proposée après le pas {rang}, ou l'a été dans le \
                     désordre"
                )
            });
        rang = position + 1;
    }

    // Et la main relâche : la dernière couleur du geste est la dernière à quitter le limiteur —
    // qu'elle soit partie pendant le glissement ou à la vidange qui suit.
    if let Some(reste) = limiteur.a_envoyer(fin + HORLOGE) {
        envoyees.push(reste);
    }
    assert_eq!(
        envoyees.last().copied(),
        Some(couleur_du_pas(PAS - 1)),
        "la main s'est arrêtée sur {:?} : c'est ce que le boîtier doit porter, pas la valeur d'avant",
        couleur_du_pas(PAS - 1)
    );
}

// ---------------------------------------------------------------------------
// 5 — une couleur déjà posée n'a rien de nouveau à envoyer
// ---------------------------------------------------------------------------

#[test]
fn une_rafale_qui_revient_sur_la_couleur_deja_envoyee_n_envoie_rien() {
    // Critère : « une rafale dont la dernière valeur est égale à la précédente envoyée → rien de
    // nouveau à envoyer ».
    //
    // C'est le geste de la main qui hésite : elle part de la couleur posée, promène la jauge, et
    // revient exactement d'où elle vient. Renvoyer la couleur ne changerait rien à l'œil et
    // consommerait du bus — et le matériel n'a pas de watchdog qui justifierait de réécrire.
    let mut limiteur = Limiteur::nouveau();
    assert_eq!(limiteur.proposer(PREMIERE, DEBUT), Some(PREMIERE));

    for (i, couleur) in [INTERMEDIAIRE, AUTRE_INTERMEDIAIRE, PREMIERE]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            limiteur.proposer(couleur, DEBUT + INSTANT * (i as u32 + 1)),
            None,
            "la rafale tient dans un intervalle"
        );
    }
    assert_eq!(
        limiteur.a_envoyer(DEBUT + INTERVALLE),
        None,
        "la main est revenue sur la couleur déjà posée : il n'y a rien de nouveau à envoyer"
    );
    assert_eq!(
        limiteur.a_envoyer(DEBUT + INTERVALLE + HORLOGE),
        None,
        "et l'attente ne ressurgit pas au tour d'horloge suivant"
    );

    // La même règle vaut pour `proposer`, bien après l'intervalle : republier la couleur courante
    // n'est pas un geste. Voir le point n° 4 de l'en-tête.
    assert_eq!(
        limiteur.proposer(PREMIERE, DEBUT + INTERVALLE * 10),
        None,
        "reproposer la couleur déjà posée ne demande rien de nouveau au démon"
    );

    // Mais le limiteur n'est pas fermé pour autant : une vraie couleur repart.
    assert_eq!(
        limiteur.proposer(FINALE, DEBUT + INTERVALLE * 11),
        Some(FINALE),
        "une couleur qui change, elle, part"
    );
}

// ---------------------------------------------------------------------------
// 6 — un limiteur neuf, et un limiteur à vide
// ---------------------------------------------------------------------------

#[test]
fn un_limiteur_a_vide_n_a_rien_a_envoyer_et_ne_panique_pas() {
    // Critère : « rien en attente et rien proposé → rien à envoyer, et pas de panique ».
    //
    // C'est le cas le plus fréquent de tous : l'horloge d'une seconde tourne en permanence, et
    // personne ne touche aux jauges la plupart du temps. Une implémentation qui dépilerait sans
    // vérifier — ou qui prendrait `Duration::ZERO` pour une couleur — planterait la fenêtre au
    // repos, pas sous les doigts.
    let mut limiteur = Limiteur::nouveau();
    for tour in 0..5u32 {
        assert_eq!(
            limiteur.a_envoyer(DEBUT + HORLOGE * tour),
            None,
            "personne n'a touché les jauges : il n'y a rien à envoyer au tour {tour}"
        );
    }

    // Et la première couleur qui vient après ce silence part sans délai : les tours d'horloge à
    // vide n'ont pas armé d'intervalle.
    assert_eq!(
        limiteur.proposer(PREMIERE, DEBUT + HORLOGE * 5),
        Some(PREMIERE),
        "le premier geste part tout de suite, quel que soit le temps passé au repos"
    );
    assert_eq!(
        limiteur.a_envoyer(DEBUT + HORLOGE * 6),
        None,
        "la couleur vient de partir par `proposer` : la vidange n'a rien à reprendre"
    );
}

// ---------------------------------------------------------------------------
// 7 — le limiteur ne rend jamais une couleur qu'on ne lui a pas donnée
// ---------------------------------------------------------------------------

#[test]
fn le_limiteur_ne_rend_jamais_une_couleur_qu_on_ne_lui_a_pas_donnee() {
    // Test d'intention n° 7 de l'issue. C'est la propriété qui interdit toute la famille des
    // limiteurs « astucieux » : celui qui moyenne deux valeurs pour lisser, celui qui rend la
    // couleur précédente au lieu de la courante, celui qui repart du noir entre deux envois.
    //
    // Le geste est irrégulier à dessein — des pauses, des rafales serrées, des valeurs qui
    // reviennent — parce qu'un limiteur ne se trompe pas sur une suite régulière.
    let mut limiteur = Limiteur::nouveau();
    let ecarts = [
        INSTANT,
        IMAGE,
        IMAGE,
        INTERVALLE,
        INSTANT,
        INSTANT,
        HORLOGE,
        IMAGE,
        INTERVALLE - INSTANT,
        IMAGE,
        INTERVALLE * 3,
    ];

    let mut instant = DEBUT;
    let mut derniere_proposee = None;
    for (i, ecart) in ecarts.iter().enumerate() {
        let couleur = couleur_du_pas(i as u8 * 7);
        derniere_proposee = Some(couleur);
        if let Some(partie) = limiteur.proposer(couleur, instant) {
            assert_eq!(
                partie, couleur,
                "`proposer` rend la couleur qu'on lui donne, jamais la précédente ni une inventée"
            );
            derniere_proposee = None;
        }
        // La vidange, si elle rend quelque chose, ne peut rendre que la dernière proposition non
        // encore partie. Pas la précédente, pas une moyenne, pas le noir.
        if let Some(vidangee) = limiteur.a_envoyer(instant) {
            assert_eq!(
                Some(vidangee),
                derniere_proposee,
                "la vidange rend la dernière couleur proposée et non encore partie, à l'appel {i}"
            );
            derniere_proposee = None;
        }
        instant += *ecart;
    }

    if let Some(vidangee) = limiteur.a_envoyer(instant + HORLOGE) {
        assert_eq!(
            Some(vidangee),
            derniere_proposee,
            "la vidange finale rend ce qui restait, et rien d'autre"
        );
    }
}

// ---------------------------------------------------------------------------
// 8 — une zone visée reçoit la couleur à sa place
// ---------------------------------------------------------------------------

#[test]
fn une_zone_visee_recoit_la_couleur_a_sa_place() {
    // Critère d'acceptation : « zone visée : c'est `zone light <nom> <rrggbb>` qui part, jamais
    // `light all` ».
    //
    // C'est le second point de la remarque de Nico : « si je sélectionne une couleur prédéfinie
    // alors qu'une animation est en cours, ça passe tout en fixe et couleur unie ». Voulu quand
    // rien n'est visé — `Request::Light` arrête l'animation — et à interdire quand une zone l'est :
    // un `light all` échappé effacerait d'un coup l'animation du boîtier entier pour colorer une
    // poignée de LED.
    for zone in [ZONE, AUTRE_ZONE] {
        let journal = Journal::neuf();
        let rendues = requetes_vers_la_cible(Some(zone), FINALE, |couleur| {
            journal.noter(couleur);
            requetes_du_bouton(couleur)
        });

        assert_eq!(
            rendues,
            vec![Request::ZoneLight {
                nom: zone.to_owned(),
                couleur: FINALE,
            }],
            "une couleur posée sur « {zone} » vaut un `zone light {zone}`, et rien d'autre"
        );
        assert_eq!(
            journal.appels(),
            Vec::new(),
            "le repli du boîtier n'a rien à faire ici : la zone est visée"
        );
    }
}

// ---------------------------------------------------------------------------
// 9 — sans zone, ce sont les requêtes du bouton « Appliquer »
// ---------------------------------------------------------------------------

#[test]
fn sans_zone_ce_sont_les_memes_requetes_que_le_bouton_appliquer() {
    // Critère d'acceptation : « aucune zone visée : ce sont les mêmes requêtes que le bouton
    // “Appliquer” produit aujourd'hui, sélection de LED comprise ».
    //
    // Le mot qui compte est **mêmes** : la jauge et le bouton mènent au même endroit. Une jauge qui
    // enverrait un `light all` là où le bouton peint une sélection ferait d'un mouvement de curseur
    // un effacement de la peinture en cours — sans un message.
    let journal = Journal::neuf();
    let rendues = requetes_vers_la_cible(None, FINALE, |couleur| {
        journal.noter(couleur);
        requetes_du_bouton(couleur)
    });

    assert_eq!(
        rendues,
        requetes_du_bouton(FINALE),
        "sans zone, les requêtes du bouton passent telles quelles — même contenu, même ordre"
    );
    assert_eq!(
        journal.appels(),
        vec![FINALE],
        "le repli est consulté une fois, avec la couleur proposée et pas une autre"
    );

    // La sélection vide mène au boîtier entier : c'est encore le bouton qui en décide, et la
    // fonction ne fait toujours que transmettre.
    let journal = Journal::neuf();
    let rendues = requetes_vers_la_cible(None, PREMIERE, |couleur| {
        journal.noter(couleur);
        requete_du_boitier(couleur)
    });
    assert_eq!(
        rendues,
        requete_du_boitier(PREMIERE),
        "rien de sélectionné : la couleur va au boîtier entier, comme le bouton le fait"
    );
    assert_eq!(journal.appels(), vec![PREMIERE]);

    // Et si le bouton n'a rien à envoyer, la jauge n'a rien à envoyer non plus. Inventer un
    // `light all` par défaut ferait de la jauge un ordre que le bouton ne donne pas.
    let rendues = requetes_vers_la_cible(None, PREMIERE, |_| Vec::new());
    assert_eq!(
        rendues,
        Vec::new(),
        "le repli n'a rien produit : la jauge n'invente pas d'ordre à sa place"
    );
}
