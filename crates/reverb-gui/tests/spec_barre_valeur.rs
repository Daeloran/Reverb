//! Tests d'intention de la valeur affichée à côté d'une barre de consigne (issue #102).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #102 et ses critères d'acceptation. Le
//! contrat de [`Poignee`] n'est pas relu ici : il est repris tel que `spec_reglages.rs` (#32) — le
//! fichier de tests d'intention voisin — l'a figé. Seule la forme publique de
//! `ResponseLine::Channel` a été relevée dans le protocole, pour savoir d'où vient la mesure d'un
//! canal et sous quelle forme son illisibilité arrive à la fenêtre.
//!
//! À l'écriture de ce fichier, `consigne_affichee` **n'existe pas** : la compilation doit échouer
//! sur elle, et sur elle seule. C'est la phase rouge.
//!
//! Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne dort : le temps est **injecté**. Un
//! test qui dormirait pour attendre la fin de la grâce mesurerait l'ordonnanceur ; un test qui
//! rendrait l'onglet mesurerait une session graphique.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Une barre sans repère chiffré rend une classe entière de défauts **invisible**. Le 2026-08-15,
//! le bouton « auto » du Kraken a été cru sans effet : il en avait un, très mauvais — il mettait la
//! consigne à **0 %** —, mais rien à l'écran ne montrait la valeur, donc rien ne distinguait « sans
//! effet » de « effet désastreux ». Il a fallu lire sysfs à la main pour le voir.
//!
//! Le corriger naïvement en produirait un pire : afficher un nombre **venu d'ailleurs** que de la
//! poignée. La fenêtre tient deux grandeurs pour un canal — la télémétrie du démon et ce que la
//! main vient de poser — et la poignée sait déjà laquelle montrer à chaque instant (#32). Un texte
//! branché sur la télémétrie brute afficherait 30 % pendant qu'on tire la poignée à 80 : la barre
//! et son chiffre se contrediraient, et on croirait la consigne perdue. C'est le sens de « pas une
//! seconde source qui divergerait » dans l'issue, et c'est ce que la moitié des tests d'ici
//! vérifient.
//!
//! ## Le contrat que ces tests figent
//!
//! ```ignore
//! // dans crates/reverb-gui/src/reglages.rs, à côté de `Poignee` (#32).
//!
//! /// Ce qu'une ligne de canal écrit à côté de sa barre de consigne.
//! ///
//! /// Le nombre vient de la poignée — `Poignee::affichee` — et de nulle part ailleurs : c'est ce
//! /// qui garde le texte et la barre d'accord pendant la saisie comme pendant la grâce.
//! ///
//! /// `mesure` est le `pwm` de la dernière ligne `chan` reçue du démon, `None` quand il l'a rendu
//! /// illisible. Son **absence** dit que le canal ne répond pas ; sa **valeur** n'écrit jamais le
//! /// nombre — `Poignee::mesurer` l'a déjà portée.
//! pub fn consigne_affichee(poignee: &Poignee, mesure: Option<u8>) -> String;
//! ```
//!
//! ## Ce que l'issue laisse ouvert, et que ces tests tranchent
//!
//! 1. **La valeur est une chaîne, pas une propriété de `.slint`.** L'issue note que
//!    `LigneVentilateur` porte déjà `pwm: i32` et qu'« elle n'est simplement pas rendue en texte ».
//!    Formater ce nombre dans le `.slint` marcherait — et ne se vérifierait que sur une image. La
//!    règle qui compte ici (« la valeur est celle que la poignée affiche déjà ») mérite d'être
//!    tenue par un test qui tourne à chaque `cargo test`, pas par une relecture d'aperçu. C'est le
//!    précédent de #76 : « la liste de ce que la fenêtre offre doit être une valeur, et non une
//!    suite d'entrées écrites à la main dans le `.slint` ».
//! 2. **La mesure entre par un `Option<u8>` et non par un booléen.** `None` et `Some(0)` sont
//!    exactement la paire que l'issue oppose — un canal qui ne répond pas, et un canal que le
//!    bouton « auto » vient de mettre à zéro. Un `bool` les distinguerait aussi, mais il obligerait
//!    l'appelant à faire la conversion lui-même, c'est-à-dire à décider **ailleurs** ce que
//!    `pwm: Option<u8>` veut dire — or c'est précisément la forme que le démon rend
//!    (`ResponseLine::Channel`), et la fenêtre l'a déjà en main.
//! 3. **Un canal illisible ne rend jamais le texte qu'un canal lisible rendrait**, quel que soit
//!    l'état de la poignée. C'est l'encodage littéral de « n'affiche pas une consigne comme si elle
//!    était mesurée » : rendre le même texte dans les deux cas *serait* la présenter comme mesurée.
//!    Ces tests n'exigent pas pour autant qu'un canal illisible cache la consigne qu'on lui tire —
//!    « -- » et « 80 % (consigne) » passent tous les deux. Ce qu'ils interdisent, c'est
//!    l'indistinction.
//! 4. **Un canal illisible qui ne porte aucune consigne n'écrit aucun chiffre.** Il ne reste alors
//!    qu'une mesure périmée, et l'afficher serait le défaut le plus coûteux du projet parce qu'il
//!    est rassurant : c'est déjà la règle du cadran (« une sonde muette affiche des tirets, jamais
//!    un zéro ni la dernière valeur connue ») et celle de `Releve::Illisible` dans le panneau des
//!    sondes. Le texte reste **non vide** pour la même raison : une case vide se lit comme une
//!    ligne sans consigne, pas comme un canal qui ne répond pas.
//! 5. **Le texte porte son unité.** « Le régime en tours par minute reste distinct de la consigne »
//!    ne se vérifie qu'en partie sans image ; ce qui s'en vérifie ici, c'est qu'un `%` accompagne le
//!    nombre et qu'aucune graphie du régime ne le suit. Un nombre nu à côté d'un autre nombre nu,
//!    c'est exactement la confusion que l'issue refuse — « 50 % donne 50 % du régime maximal ».
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Que chaque ligne de l'onglet rende effectivement ce texte**, et **que la mise en page tienne
//!   dans la largeur du panneau**. Les deux se voient sur une image et sur rien d'autre :
//!   `cargo run --release --example apercu -p reverb-gui` avec `REVERB_ONGLET=ventilos`. Le
//!   critère d'acceptation le dit lui-même — « vérifiée par l'exemple `apercu` et non à l'œil » —,
//!   et cette vérification-là est un rendu complet de fenêtre, pas une assertion.
//! - **La distinction *visuelle* entre le régime et la consigne** — place, taille, style. Ce
//!   fichier n'en tient que la moitié qui est une valeur : l'unité écrite.
//! - **Ce qu'affiche une poignée qui n'a encore rien vu**, ni mesure ni consigne. `spec_reglages.rs`
//!   laisse déjà ce trou ouvert, et le combler ici inventerait une règle que #32 n'a pas voulue.
//!   Même retenue juste après `liberer` : ces tests n'exigent pas laquelle des deux valeurs connues
//!   le texte montre, seulement qu'il n'en invente pas une troisième.
//! - **Les autres barres de la fenêtre** — vitesse d'animation, teinte, luminosité. L'issue les met
//!   hors scope : elles méritent peut-être le même traitement, elles n'ont masqué aucun défaut.
//! - **Les consignes au-delà de 100.** Le protocole écrit `fan <canal> pwm <0-100>` : au-delà, il
//!   n'y a pas de comportement à figer, il y a une valeur qui ne peut pas arriver.

use std::time::Duration;

use reverb_gui::reglages::{GRACE, Poignee, consigne_affichee};
use reverb_proto::Position;
use reverb_proto::ipc::ResponseLine;

// ---------------------------------------------------------------------------
// Repères et aides
// ---------------------------------------------------------------------------

/// L'origine du temps injecté.
///
/// Volontairement loin de zéro : la fenêtre est ouverte depuis un moment quand une main touche
/// enfin une barre. Une implémentation qui prendrait `Duration::ZERO` pour « aucune consigne
/// encore » se ferait attraper ici plutôt qu'à l'usage.
const DEBUT: Duration = Duration::from_secs(3_600);

/// La cadence de la télémétrie : la fenêtre demande `status` **une fois par seconde** (README).
const CADENCE: Duration = Duration::from_secs(1);

/// Le plus petit écart que le temps injecté sache représenter.
///
/// Sert à encadrer la borne de [`GRACE`] sans laisser d'espace entre l'essai d'avant et celui
/// d'après.
const INSTANT: Duration = Duration::from_nanos(1);

/// Ce que le matériel rapporte tant qu'il n'a pas rejoint la consigne : le régime du firmware.
const MESURE_AU_REPOS: u8 = 30;

/// Là où l'utilisateur tire la poignée. Franchement au-dessus de la mesure : c'est l'écart que le
/// texte doit montrer pendant qu'on tire, et que la télémétrie refermerait s'il en venait.
const CONSIGNE: u8 = 80;

/// Une seconde mesure, pour vérifier qu'un texte ne se branche pas sur le relevé qu'on lui passe.
///
/// Distincte des deux autres, et distincte du complément à 100 de chacune : une implémentation qui
/// afficherait `100 - mesure` — l'inversion la plus banale — ne doit pas tomber juste par accident.
const AUTRE_MESURE: u8 = 62;

/// La borne haute de la consigne, telle que le protocole l'écrit : `fan <canal> pwm <0-100>`.
const PLEIN_REGIME: u8 = 100;

/// Le canal d'exemple. Son identité n'a aucune part dans ce qui est testé — elle sert à bâtir une
/// ligne `chan` qui ressemble à ce que le démon rend vraiment.
const CANAL: &str = "nzxtsmart2:fan-speed:1";

/// Les graphies sous lesquelles un régime pourrait s'écrire.
///
/// Aucune ne doit accompagner une consigne : « ce sont deux grandeurs, et les confondre ferait
/// croire qu'une consigne à 50 % donne 50 % du régime maximal » (issue #102).
const UNITES_DE_REGIME: [&str; 5] = ["tr/min", "tr / min", "rpm", "RPM", "tours"];

/// Le nombre écrit dans un texte, ou `None` s'il n'en porte aucun.
///
/// Les chiffres sont **rassemblés** plutôt que cherchés en sous-chaîne : `texte.contains("1")`
/// serait vrai de « 100 % » comme de « 1 % », et un texte qui afficherait toujours la même valeur
/// passerait la moitié des tests d'ici.
fn nombre(texte: &str) -> Option<u32> {
    let chiffres: String = texte.chars().filter(char::is_ascii_digit).collect();
    if chiffres.is_empty() {
        None
    } else {
        chiffres.parse().ok()
    }
}

/// Une poignée qui n'a vu qu'une mesure : le ventilateur tourne, aucune main dessus.
fn au_repos() -> Poignee {
    let mut poignee = Poignee::nouvelle();
    poignee.mesurer(MESURE_AU_REPOS, DEBUT);
    poignee
}

/// Une poignée **tenue** : la main vient de la tirer à [`CONSIGNE`].
///
/// Aucune télémétrie n'arrive après le geste — c'est le critère « sans attendre le tour de
/// télémétrie suivant ».
fn en_saisie() -> Poignee {
    let mut poignee = au_repos();
    poignee.saisir(CONSIGNE, DEBUT + INSTANT);
    poignee
}

/// Une poignée relâchée depuis moins que [`GRACE`], et à qui la télémétrie a redit la mesure.
fn en_grace() -> Poignee {
    let mut poignee = en_saisie();
    poignee.relacher(DEBUT + CADENCE);
    poignee.mesurer(MESURE_AU_REPOS, DEBUT + CADENCE + GRACE - INSTANT);
    poignee
}

/// Une poignée dont la grâce a expiré : la mesure a repris la main.
fn apres_la_grace() -> Poignee {
    let mut poignee = en_grace();
    poignee.mesurer(MESURE_AU_REPOS, DEBUT + CADENCE + GRACE);
    poignee
}

/// Une poignée rendue au firmware par le bouton « auto ».
fn liberee() -> Poignee {
    let mut poignee = en_saisie();
    poignee.liberer();
    poignee
}

/// Les cinq états d'une poignée, sous leur nom — de quoi passer une règle sur tous plutôt que sur
/// celui qui arrange.
fn tous_les_etats() -> Vec<(&'static str, Poignee)> {
    vec![
        ("neuve", Poignee::nouvelle()),
        ("au repos", au_repos()),
        ("en saisie", en_saisie()),
        ("en grâce", en_grace()),
        ("après la grâce", apres_la_grace()),
        ("libérée", liberee()),
    ]
}

/// Une ligne `chan` telle que le démon la rend, avec la mesure qu'on lui donne.
///
/// `None` est ce qu'il écrit d'un canal qu'il n'a pas pu lire : « ni omise, ni remplacée par
/// zéro » (`ResponseLine::Channel`). C'est de là que vient l'illisibilité, et bâtir la ligne plutôt
/// que d'écrire `None` à la main garde le test attaché à ce que la fenêtre reçoit vraiment.
fn ligne_chan(pwm: Option<u8>) -> ResponseLine {
    ResponseLine::Channel {
        channel: CANAL.to_string(),
        position: Some(Position::Arriere),
        rpm: Some(1_180),
        pwm,
        mode: "pwm".to_string(),
        sait_faire_auto: false,
        // #113 : la ligne `chan` gagne un dernier jeton. Ce fichier ne parle que
        // de la valeur écrite à côté d'une barre, et rien ici n'en dépend.
        regulable: false,
    }
}

/// La mesure portée par une ligne `chan`.
fn mesure_de(ligne: &ResponseLine) -> Option<u8> {
    match ligne {
        ResponseLine::Channel { pwm, .. } => *pwm,
        autre => panic!("ce test bâtit des lignes « chan », pas {autre:?}"),
    }
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tous les tests qui suivent supposent que la mesure, la consigne et la seconde mesure
    // diffèrent, et que les instants injectés tombent bien de part et d'autre de la grâce. Si l'un
    // de ces repères se dégradait, plusieurs tests deviendraient vrais sans rien vérifier — et
    // personne ne le verrait. Ce test est là pour que la panne soit ici.
    let valeurs = [
        ("MESURE_AU_REPOS", MESURE_AU_REPOS),
        ("CONSIGNE", CONSIGNE),
        ("AUTRE_MESURE", AUTRE_MESURE),
        ("PLEIN_REGIME", PLEIN_REGIME),
    ];
    for (i, (nom, valeur)) in valeurs.iter().enumerate() {
        assert!(
            *valeur <= PLEIN_REGIME,
            "{nom} vaut {valeur}, hors du « pwm <0-100> » que le protocole écrit"
        );
        for (autre_nom, autre) in valeurs.iter().skip(i + 1) {
            assert_ne!(
                valeur, autre,
                "{nom} et {autre_nom} doivent différer, sinon les tests qui les opposent ne \
                 vérifient rien"
            );
        }
    }

    // L'inversion la plus banale — afficher le complément — ne doit tomber juste sur aucune paire.
    for (nom, valeur) in valeurs {
        for (autre_nom, autre) in valeurs {
            if nom != autre_nom {
                assert_ne!(
                    PLEIN_REGIME - valeur,
                    autre,
                    "{nom} inversé vaut {autre_nom} : une implémentation qui afficherait \
                     « 100 − v » passerait un test sur deux"
                );
            }
        }
    }

    assert!(
        GRACE > INSTANT,
        "une grâce de {GRACE:?} ne se laisse pas encadrer par un écart d'une nanoseconde"
    );
    assert!(
        CADENCE < GRACE,
        "la télémétrie doit tomber au moins une fois pendant la grâce, sinon « la consigne tient \
         malgré la mesure qui arrive » ne se vérifie pas"
    );

    // Les six états d'une poignée doivent bien être six états, et non six fois le même.
    let etats = tous_les_etats();
    assert_eq!(etats.len(), 6, "six états nommés");
    assert_eq!(
        en_saisie().affichee(),
        CONSIGNE,
        "une poignée tenue affiche la consigne — sans quoi #32 serait cassée, et rien d'ici ne \
         voudrait dire ce qu'il prétend"
    );
    assert_eq!(
        en_grace().affichee(),
        CONSIGNE,
        "la consigne tient pendant la grâce, même quand la télémétrie redit la mesure"
    );
    assert_eq!(
        apres_la_grace().affichee(),
        MESURE_AU_REPOS,
        "la grâce expirée, la mesure reprend la main"
    );

    // Et la ligne `chan` d'où vient la mesure doit porter ce qu'on croit y mettre.
    assert_eq!(
        mesure_de(&ligne_chan(Some(MESURE_AU_REPOS))),
        Some(MESURE_AU_REPOS)
    );
    assert_eq!(mesure_de(&ligne_chan(None)), None);
}

// ---------------------------------------------------------------------------
// 1 — chaque consigne s'écrit, et elle s'écrit en pourcentage
// ---------------------------------------------------------------------------

#[test]
fn chaque_consigne_s_ecrit_en_pourcentage() {
    // Critère d'acceptation : « chaque ligne de canal affiche sa consigne en pourcentage ».
    //
    // Les cent une valeurs que le protocole sait écrire, une par une : un texte qui n'en rendrait
    // qu'une partie — les rondes, les non nulles — laisserait sans repère justement les canaux
    // qu'on regarde quand quelque chose cloche.
    for valeur in 0..=PLEIN_REGIME {
        let mut poignee = Poignee::nouvelle();
        poignee.mesurer(valeur, DEBUT);
        let texte = consigne_affichee(&poignee, Some(valeur));

        assert_eq!(
            nombre(&texte),
            Some(u32::from(valeur)),
            "une consigne à {valeur} % doit s'écrire {valeur}, pas autre chose — obtenu {texte:?}"
        );
        assert!(
            texte.contains('%'),
            "une consigne se lit en pourcentage : {texte:?} n'en porte pas le signe, et un nombre \
             nu à côté d'un régime en tours par minute se confond avec lui"
        );
    }
}

#[test]
fn la_consigne_ne_porte_jamais_l_unite_d_un_regime() {
    // Critère d'acceptation : « le régime en tr/min et la consigne en % restent visuellement
    // distincts ». Ce qui s'en vérifie sans image, c'est l'unité écrite : une consigne qui
    // porterait la graphie d'un régime serait la confusion elle-même, pas un risque de confusion.
    for (nom, poignee) in tous_les_etats() {
        let texte = consigne_affichee(&poignee, Some(MESURE_AU_REPOS));
        for unite in UNITES_DE_REGIME {
            assert!(
                !texte.to_lowercase().contains(&unite.to_lowercase()),
                "la consigne d'une poignée {nom} s'écrit {texte:?} : elle porte « {unite} », qui \
                 est l'unité du régime — les deux grandeurs se confondraient"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 2 — le texte suit la poignée pendant qu'on la tire
// ---------------------------------------------------------------------------

#[test]
fn le_texte_suit_la_poignee_pendant_la_saisie() {
    // Critère d'acceptation : « la valeur suit la poignée pendant qu'on la tire, sans attendre le
    // tour de télémétrie suivant ».
    //
    // La poignée est tirée à CONSIGNE et **rien** n'arrive du démon après le geste : la dernière
    // mesure connue vaut toujours MESURE_AU_REPOS. Un texte qui attendrait le tour suivant
    // afficherait 30 pendant qu'on tire à 80.
    let poignee = en_saisie();
    let texte = consigne_affichee(&poignee, Some(MESURE_AU_REPOS));

    assert_eq!(
        nombre(&texte),
        Some(u32::from(CONSIGNE)),
        "la poignée est tirée à {CONSIGNE} et le texte dit {texte:?} : la barre et son chiffre se \
         contredisent, et on croirait la consigne perdue"
    );
    assert_ne!(
        nombre(&texte),
        Some(u32::from(MESURE_AU_REPOS)),
        "le texte affiche la mesure pendant qu'on tire la poignée : c'est la seconde source que \
         l'issue interdit"
    );
}

#[test]
fn la_telemetrie_qui_arrive_pendant_le_geste_ne_referme_pas_le_texte_sur_la_mesure() {
    // La fenêtre demande `status` une fois par seconde, y compris pendant qu'une main tient la
    // poignée. Un texte qui suivrait la dernière ligne `chan` reçue reviendrait à la mesure à
    // chaque tour — et le chiffre sauterait de 80 à 30 une fois par seconde sous le doigt.
    let mut poignee = en_saisie();
    for tour in 1..=5u32 {
        poignee.mesurer(MESURE_AU_REPOS, DEBUT + CADENCE * tour);
        let texte = consigne_affichee(&poignee, Some(MESURE_AU_REPOS));
        assert_eq!(
            nombre(&texte),
            Some(u32::from(CONSIGNE)),
            "au tour {tour} de télémétrie, la poignée est toujours tenue à {CONSIGNE} et le texte \
             dit {texte:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — le texte reprend la mesure quand la grâce expire
// ---------------------------------------------------------------------------

#[test]
fn le_texte_reprend_la_mesure_quand_la_grace_expire() {
    // Critère d'acceptation : « elle reprend la mesure du démon quand la grâce de la poignée
    // expire ».
    //
    // La borne est encadrée des deux côtés par le plus petit écart représentable, comme
    // `spec_reglages.rs` (#32) le fait de la poignée elle-même : un `<` mis pour un `<=` ne change
    // rien à l'usage, mais une borne non pinnée laisserait passer les deux fautes qui comptent —
    // un texte qui ne montre jamais la consigne, et un texte qui ne montre plus jamais la mesure.
    let relachement = DEBUT + CADENCE;

    let mut juste_avant = en_saisie();
    juste_avant.relacher(relachement);
    juste_avant.mesurer(MESURE_AU_REPOS, relachement + GRACE - INSTANT);
    assert_eq!(
        nombre(&consigne_affichee(&juste_avant, Some(MESURE_AU_REPOS))),
        Some(u32::from(CONSIGNE)),
        "une nanoseconde avant la fin de la grâce, la consigne tient encore : le texte doit la \
         montrer, sinon on verrait le chiffre retomber avant que le ventilateur ait eu le temps de \
         monter"
    );

    let mut juste_apres = en_saisie();
    juste_apres.relacher(relachement);
    juste_apres.mesurer(MESURE_AU_REPOS, relachement + GRACE);
    assert_eq!(
        nombre(&consigne_affichee(&juste_apres, Some(MESURE_AU_REPOS))),
        Some(u32::from(MESURE_AU_REPOS)),
        "à relâchement + GRACE pile, la consigne a fini de tenir : le texte doit rendre la mesure, \
         sinon il figerait à jamais ce qu'on a demandé au lieu de ce qui se passe"
    );
}

// ---------------------------------------------------------------------------
// 4 — le texte n'a qu'une source : la poignée
// ---------------------------------------------------------------------------

#[test]
fn le_texte_est_toujours_celui_que_la_poignee_affiche() {
    // Comportement attendu : « la valeur affichée est celle que la poignée affiche déjà […]. Pas
    // une seconde source qui divergerait. »
    //
    // La règle est vérifiée à **chaque pas** d'une vie complète de canal, plutôt qu'aux deux ou
    // trois instants qui arrangent : c'est ce qui la rend vraie par construction et non par
    // coïncidence.
    let mut poignee = Poignee::nouvelle();
    let mut instant = DEBUT;
    let verifier = |poignee: &Poignee, etape: &str| {
        let texte = consigne_affichee(poignee, Some(MESURE_AU_REPOS));
        assert_eq!(
            nombre(&texte),
            Some(u32::from(poignee.affichee())),
            "à l'étape « {etape} », la poignée affiche {} et le texte dit {texte:?} : deux sources \
             qui divergent",
            poignee.affichee()
        );
    };

    poignee.mesurer(MESURE_AU_REPOS, instant);
    verifier(&poignee, "première mesure");

    instant += INSTANT;
    poignee.saisir(CONSIGNE, instant);
    verifier(&poignee, "la main tire la poignée");

    instant += CADENCE;
    poignee.mesurer(MESURE_AU_REPOS, instant);
    verifier(&poignee, "télémétrie pendant le geste");

    poignee.relacher(instant);
    verifier(&poignee, "la main relâche");

    let relachement = instant;
    instant = relachement + GRACE - INSTANT;
    poignee.mesurer(MESURE_AU_REPOS, instant);
    verifier(&poignee, "dernier tour de la grâce");

    instant = relachement + GRACE;
    poignee.mesurer(MESURE_AU_REPOS, instant);
    verifier(&poignee, "la grâce a expiré");

    instant += CADENCE;
    poignee.mesurer(AUTRE_MESURE, instant);
    verifier(&poignee, "le ventilateur a changé de régime");

    instant += INSTANT;
    poignee.saisir(PLEIN_REGIME, instant);
    verifier(&poignee, "second geste, jusqu'au plein régime");

    poignee.liberer();
    verifier(&poignee, "le canal est rendu au firmware");
}

#[test]
fn la_mesure_passee_en_argument_n_ecrit_jamais_le_nombre() {
    // C'est la forme directe de « pas une seconde source ». Le nombre vient de la poignée, à qui
    // la mesure a déjà été donnée par `mesurer` ; le relevé passé à côté ne sert qu'à dire si le
    // canal répond. Une implémentation qui écrirait ce relevé plutôt que la poignée rendrait deux
    // textes différents ici — et afficherait la télémétrie par-dessus la consigne qu'on tire.
    for (nom, poignee) in tous_les_etats() {
        let attendu = consigne_affichee(&poignee, Some(MESURE_AU_REPOS));
        for autre in [0, AUTRE_MESURE, PLEIN_REGIME] {
            assert_eq!(
                consigne_affichee(&poignee, Some(autre)),
                attendu,
                "une poignée {nom} rend {attendu:?} avec un relevé à {MESURE_AU_REPOS} et autre \
                 chose avec un relevé à {autre} : le nombre affiché vient du relevé, pas de la \
                 poignée"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — un canal illisible ne se lit pas comme un canal mesuré
// ---------------------------------------------------------------------------

#[test]
fn un_canal_illisible_ne_rend_jamais_le_texte_d_un_canal_lisible() {
    // Critère d'acceptation : « un canal illisible n'affiche pas une consigne comme si elle était
    // mesurée ». Rendre le même texte dans les deux cas *serait* la présenter comme mesurée.
    //
    // Ce test n'exige pas qu'un canal illisible cache le chiffre : « -- » et « 80 % (consigne) »
    // passent tous les deux. Ce qu'il interdit, c'est l'indistinction.
    for (nom, poignee) in tous_les_etats() {
        let illisible = consigne_affichee(&poignee, mesure_de(&ligne_chan(None)));
        let lisible = consigne_affichee(&poignee, mesure_de(&ligne_chan(Some(MESURE_AU_REPOS))));
        assert_ne!(
            illisible, lisible,
            "une poignée {nom} rend {illisible:?} que le canal réponde ou non : rien ne distingue \
             une valeur mesurée d'une valeur qui ne l'est pas"
        );
    }
}

#[test]
fn un_canal_illisible_sans_consigne_n_ecrit_aucun_chiffre() {
    // Comportement attendu : « un canal illisible n'affiche pas de valeur trompeuse ».
    //
    // Sans consigne en cours, il ne reste qu'une mesure périmée. L'afficher est le mode de
    // défaillance le plus coûteux du projet parce qu'il est rassurant — c'est déjà la règle du
    // cadran : « une sonde muette affiche des tirets, jamais un zéro ni la dernière valeur
    // connue ».
    for (nom, poignee) in [
        ("neuve", Poignee::nouvelle()),
        ("au repos", au_repos()),
        ("après la grâce", apres_la_grace()),
    ] {
        let texte = consigne_affichee(&poignee, mesure_de(&ligne_chan(None)));
        assert_eq!(
            nombre(&texte),
            None,
            "une poignée {nom} sur un canal qui ne répond pas écrit {texte:?} : ce chiffre est une \
             mesure périmée présentée comme courante"
        );
        assert!(
            !texte.trim().is_empty(),
            "un canal qui ne répond pas doit se voir : un texte vide se lit comme une ligne sans \
             consigne, pas comme un canal muet"
        );
    }
}

#[test]
fn zero_pour_cent_et_illisible_ne_se_confondent_pas() {
    // C'est l'incident du 2026-08-15, exactement : « le bouton auto mettait la consigne à 0 %, et
    // rien à l'écran ne montrait la valeur, donc rien ne distinguait sans effet d'effet
    // désastreux ». Un canal à zéro doit se lire zéro ; un canal muet ne doit pas se lire zéro.
    let mut a_zero = Poignee::nouvelle();
    a_zero.mesurer(0, DEBUT);

    let arrete = consigne_affichee(&a_zero, mesure_de(&ligne_chan(Some(0))));
    assert_eq!(
        nombre(&arrete),
        Some(0),
        "un canal mesuré à 0 % doit écrire zéro, et non se taire : {arrete:?} — c'est ce silence \
         qui a fait croire le bouton « auto » sans effet"
    );

    let muet = consigne_affichee(&a_zero, mesure_de(&ligne_chan(None)));
    assert_ne!(
        muet, arrete,
        "un canal qui ne répond pas et un canal arrêté rendent le même texte {arrete:?} : un \
         ventilateur à l'arrêt et un ventilateur dont on ne sait rien ne sont pas la même panne"
    );
}
