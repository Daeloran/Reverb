//! Tests d'intention du protocole du démon (issue #17).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #17 et son commentaire « Contrat d'API »
//! seuls. Aucune ligne n'est relue depuis `src/` : à l'écriture de ce fichier, le module `ipc`
//! n'existe pas, et `Position::slug` non plus. Ils encodent ce que le protocole doit faire, pas
//! ce que le code fait — si l'un d'eux échoue après implémentation, c'est le code qu'on corrige.
//!
//! ## Ce qui est vérifié, et pourquoi c'est ici
//!
//! Le socket, l'unité systemd, l'ouverture des périphériques sont des entrées/sorties : ils se
//! vérifient sur la machine, avec les critères d'acceptation de l'issue. Le protocole, lui, est
//! **pur** — et c'est la partie qui peut être silencieusement fausse. Une réponse mal cadrée ne
//! lève aucune erreur : le client lit trop peu de lignes, affiche un état partiel, et personne ne
//! voit rien.
//!
//! D'où le poids du test n° 5. La règle de cadrage tient en une phrase du contrat : « Une ligne de
//! données ne commence **jamais** par `end` ni par `err` — c'est ce qui rend la fin de réponse non
//! ambiguë sans compter les lignes. » Un nom de canal ou de capteur vient du matériel, pas de
//! nous ; le test le suppose donc hostile.
//!
//! Le module `ipc` **n'est pas ré-exporté à la racine** du crate (contrat d'API, même raison que
//! `ram`) — d'où les chemins `reverb_proto::ipc::…`. `Position` et `Rgb`, eux, vivent à la racine.
//!
//! Aucun accès matériel, aucune IO, aucun socket : `reverb-proto` est pur, ses tests aussi.
//!
//! ## Trois points que le contrat laisse ouverts, et que ces tests tranchent
//!
//! Un test d'intention n'a pas le droit d'aller regarder ce que le code a choisi. Quand le contrat
//! ne dit pas, c'est ici que la décision se prend — et elle est signalée à chaque fois :
//!
//! 1. **La couleur s'écrit en six chiffres hexadécimaux minuscules, sans `#`** — `ff2080`. Le
//!    contrat dit seulement `light <cible> <hex>`. Six chiffres alignent l'IPC sur ce que le
//!    README documente déjà côté ligne de commande (`reverb ram --all --color ff00ff`), et le `#`
//!    n'apporte rien dans un protocole à jetons séparés par des espaces.
//! 2. **`MAX_LINE_LEN` est une longueur acceptée, pas refusée** : 1024 octets passent, 1025 non.
//!    Le contrat écrit « longueur maximale d'une ligne acceptée » puis « ligne **au-delà** de
//!    `MAX_LINE_LEN` » — les deux disent la même borne, le test la fige.
//! 3. **Le protocole est sensible à la casse** : `STATUS` n'est pas `status`. C'est un dialogue
//!    entre deux programmes, pas une invite de commande ; accepter les variantes de casse
//!    n'ajoute qu'une surface de compatibilité à tenir.

use reverb_proto::ipc::{
    FanAction, LightTarget, MAX_LINE_LEN, Request, RequestError, ResponseError, ResponseLine,
    encode_request, encode_response_line, parse_request, parse_response_line,
};
use reverb_proto::{Position, Rgb};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Une couleur dont **les trois composantes diffèrent**, et son écriture sur le fil.
///
/// Le projet mélange trois ordres de composantes (CLAUDE.md : ventilateurs en GRB, écran en BGR,
/// RAM en RGB) et une erreur d'ordre ne produit aucun message. `ff2080` rendrait `20ff80` en GRB
/// et `8020ff` en BGR : trois textes distincts, donc une permutation ne peut pas passer.
const COULEUR_HEX: &str = "ff2080";

/// Un canal tel que le contrat en montre dans son dialogue d'exemple.
const CANAL: &str = "nzxtsmart2:fan-1";

/// La couleur de [`COULEUR_HEX`].
fn couleur() -> Rgb {
    Rgb::new(0xff, 0x20, 0x80)
}

/// Une requête d'éclairage sur la cible donnée, avec la couleur témoin.
fn lumiere(target: LightTarget) -> Request {
    Request::Light {
        target,
        color: couleur(),
    }
}

/// Une consigne de ventilateur sur le canal témoin.
fn ventilateur(action: FanAction) -> Request {
    Request::Fan {
        channel: CANAL.to_owned(),
        action,
    }
}

/// Une ligne `chan` complète, telle que le contrat l'écrit dans son dialogue d'exemple.
fn canal(channel: &str, mode: &str) -> ResponseLine {
    ResponseLine::Channel {
        channel: channel.to_owned(),
        position: Some(Position::RadiateurHaut),
        rpm: Some(1200),
        pwm: Some(60),
        mode: mode.to_owned(),
    }
}

/// Marqueur planté dans la ligne de 10 Ko du test n° 3, pour prouver qu'aucune erreur ne la
/// recopie. Une chaîne qu'aucun message d'erreur ne pourrait contenir par hasard.
const MARQUEUR: &str = "-marqueur-de-la-ligne-de-dix-kilo-octets-";

/// Une ligne de requête de 10 240 octets, très au-delà de [`MAX_LINE_LEN`].
fn ligne_de_dix_kilo_octets() -> String {
    let mut ligne = String::from("light all ");
    while ligne.len() < 10_240 {
        ligne.push_str(MARQUEUR);
    }
    ligne.truncate(10_240);
    ligne
}

/// Une ligne de requête d'exactement `octets` octets, syntaxiquement plausible mais fausse.
///
/// Sert aux deux bornes de [`MAX_LINE_LEN`] : ce qui est refusé doit l'être pour la longueur, pas
/// pour autre chose, et ce qui passe la longueur doit être refusé pour son contenu.
fn ligne_de(octets: usize) -> String {
    let mut ligne = String::from("light all ");
    while ligne.len() < octets {
        ligne.push('a');
    }
    ligne.truncate(octets);
    ligne
}

/// Noms de canal, de capteur ou de sujet **hostiles** : ceux qui, mal encodés, produiraient une
/// ligne qu'un client prendrait pour la fin de la réponse.
///
/// Un nom de canal vient du matériel et de sa cartographie, pas de nous. Le protocole doit tenir
/// même si l'un d'eux s'appelle littéralement `err`.
///
/// Aucun de ces noms ne contient d'espace : la grammaire `chan <canal> <position> <rpm> <pwm>
/// <mode>` place le canal en champ **non final**, donc un canal à espaces n'est pas relisible (cf.
/// la note du test n° 8). Les cas à espaces sont traités à part, là où ils sont légitimes.
const NOMS_HOSTILES: &[&str] = &[
    "err",
    "end",
    "error",
    "ends",
    "errr",
    "endpoint",
    "err:1",
    "end-de-ligne",
    "ERR",
    "END",
    "e",
    "erreur",
];

/// Vérifie qu'une ligne de **données** ne peut, sous aucun prétexte, se faire prendre pour une fin
/// de réponse.
///
/// C'est la propriété centrale du protocole, et elle se vérifie sur le texte encodé tel qu'il part
/// sur le socket — pas sur la variante Rust dont il vient.
fn ne_termine_jamais(donnee: &ResponseLine) {
    assert!(
        !donnee.is_terminal(),
        "{donnee:?} n'est pas une ligne terminale"
    );

    let encodee = encode_response_line(donnee);

    // Une ligne de données tient sur **une** ligne. Un `\n` dans un nom de capteur scinderait
    // l'encodage en deux, et la seconde moitié pourrait, elle, commencer par `end`. Le contrat ne
    // dit pas comment s'en prémunir — refus, échappement ou nettoyage à la source sont trois
    // réponses acceptables — mais l'invariant de cadrage l'exige.
    assert_eq!(
        encodee.lines().count(),
        1,
        "une ligne de données doit tenir sur une seule ligne : « {encodee} »"
    );
    assert!(
        !encodee.contains('\n'),
        "aucun saut de ligne dans une ligne de données : « {encodee} »"
    );
    assert!(
        !encodee.contains('\r'),
        "aucun retour chariot dans une ligne de données : « {encodee} »"
    );

    // Le test porte sur le **préfixe brut**, pas sur le premier jeton : un client qui lit
    // `ligne.starts_with("end")` est un client naïf, mais c'est exactement celui que la grammaire
    // promet de ne pas piéger.
    for terminal in ["end", "err"] {
        assert!(
            !encodee.starts_with(terminal),
            "la ligne « {encodee} » commence par « {terminal} » — un client y verrait la fin de la \
             réponse et tronquerait sa lecture"
        );
    }

    // Et relue, elle reste une ligne de données : le cadrage ne tient que si le décodeur est
    // d'accord avec l'encodeur.
    let relue = parse_response_line(&encodee).expect("une ligne encodée se relit");
    assert!(
        !relue.is_terminal(),
        "« {encodee} » se relit en {relue:?}, qui terminerait la réponse"
    );
}

/// Lit une réponse comme le ferait un client : ligne par ligne, jusqu'à la première ligne
/// terminale, **sans compter les lignes** et sans savoir combien en attendre.
///
/// C'est la seule façon de lire prévue par le contrat, et donc la seule qui prouve le cadrage.
fn lit_une_reponse(flux: &str) -> (Vec<ResponseLine>, ResponseLine) {
    let mut donnees = Vec::new();
    for ligne in flux.lines() {
        let decodee = parse_response_line(ligne).expect("chaque ligne du flux se décode");
        if decodee.is_terminal() {
            return (donnees, decodee);
        }
        donnees.push(decodee);
    }
    panic!("le flux s'est terminé sans ligne terminale : « {flux} »");
}

/// Sérialise une réponse complète : les lignes de données, puis la ligne de fin.
fn flux(donnees: &[ResponseLine], fin: &ResponseLine) -> String {
    let mut lignes: Vec<String> = donnees.iter().map(encode_response_line).collect();
    lignes.push(encode_response_line(fin));
    lignes.join("\n")
}

// ---------------------------------------------------------------------------
// 1 — chaque verbe bien formé se décode
// ---------------------------------------------------------------------------

#[test]
fn chaque_verbe_bien_forme_est_decode_en_sa_requete() {
    // Contrat d'API, « Requêtes » — quatre verbes : `status`, `light <cible> <hex>`,
    // `animate <nom>` / `animate off`, `fan <canal> pwm <0-100>` / `fan <canal> auto`.
    // Cinq cibles d'éclairage : `all`, `fans`, `fan:<slug>`, `ram`, `slot:<0-3>`.
    //
    // Chaque cas est vérifié dans les deux sens : la ligne se décode en la requête, et la requête
    // se réencode en **exactement** cette ligne. C'est ce second sens qui fige la forme du fil —
    // sans lui, un encodeur pourrait écrire `light all #FF2080` et le test resterait vert.
    let cas: [(String, Request); 10] = [
        ("status".to_owned(), Request::Status),
        (
            format!("light all {COULEUR_HEX}"),
            lumiere(LightTarget::All),
        ),
        (
            format!("light fans {COULEUR_HEX}"),
            lumiere(LightTarget::Fans),
        ),
        (
            format!("light fan:radiateur-haut {COULEUR_HEX}"),
            lumiere(LightTarget::Fan(Position::RadiateurHaut)),
        ),
        (
            format!("light ram {COULEUR_HEX}"),
            lumiere(LightTarget::Ram),
        ),
        (
            format!("light slot:2 {COULEUR_HEX}"),
            lumiere(LightTarget::RamSlot(2)),
        ),
        (
            "animate vague".to_owned(),
            Request::Animate {
                name: Some("vague".to_owned()),
                reglages: Vec::new(),
            },
        ),
        (
            "animate off".to_owned(),
            Request::Animate {
                name: None,
                reglages: Vec::new(),
            },
        ),
        (
            format!("fan {CANAL} pwm 60"),
            ventilateur(FanAction::Pwm(60)),
        ),
        (format!("fan {CANAL} auto"), ventilateur(FanAction::Auto)),
    ];

    for (ligne, attendue) in cas {
        assert_eq!(
            parse_request(&ligne),
            Ok(attendue.clone()),
            "décodage de « {ligne} »"
        );
        assert_eq!(
            encode_request(&attendue),
            ligne,
            "réencodage de {attendue:?} — la forme du fil ne doit pas dériver"
        );
    }

    // La couleur est écrite sur **six** chiffres, zéros de tête compris. `010203` et non `123` :
    // un décodeur qui lirait la longueur du champ pour deviner la couleur se tromperait, et
    // `reverb ram --all --color ff00ff` (README) écrit déjà six chiffres.
    let sombre = Request::Light {
        target: LightTarget::All,
        color: Rgb::new(0x01, 0x02, 0x03),
    };
    assert_eq!(encode_request(&sombre), "light all 010203");
    assert_eq!(parse_request("light all 010203"), Ok(sombre));

    // Les deux extrêmes, pour que le zéro ne soit pas un cas particulier.
    for (texte, rvb) in [("000000", [0, 0, 0]), ("ffffff", [0xff, 0xff, 0xff])] {
        let requete = Request::Light {
            target: LightTarget::Ram,
            color: Rgb::new(rvb[0], rvb[1], rvb[2]),
        };
        assert_eq!(encode_request(&requete), format!("light ram {texte}"));
        assert_eq!(parse_request(&format!("light ram {texte}")), Ok(requete));
    }

    // Les quatre bornes de la consigne PWM. Le contrat dit `0-100`, pas `0-255` : la valeur est un
    // pourcentage, et c'est la même échelle que celle exposée par `reverb fan`.
    for pourcentage in [0u8, 1, 50, 99, 100] {
        let ligne = format!("fan {CANAL} pwm {pourcentage}");
        assert_eq!(
            parse_request(&ligne),
            Ok(ventilateur(FanAction::Pwm(pourcentage))),
            "consigne de {pourcentage} %"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — un verbe inconnu est nommé, et ne fait pas paniquer
// ---------------------------------------------------------------------------

#[test]
fn un_verbe_inconnu_donne_une_erreur_qui_le_cite() {
    // Contrat d'API — `UnknownVerb { verb: String }`, « Premier mot inconnu. Le `Display` le
    // nomme », et le dialogue d'exemple : `< bidule` → `> err commande « bidule » inconnue`.
    //
    // Nommer le verbe reçu est ce qui rend le message utile : côté client, la faute est presque
    // toujours une faute de frappe ou un décalage de version, et les deux se voient au verbe.
    //
    // La liste contient les cinq mots-clés du **protocole de réponse** (`chan`, `temp`,
    // `unreadable`, `end`, `err`). Ce sont les seuls mots que le démon écrit lui-même : si l'un
    // d'eux était aussi un verbe de requête, un flux de réponse renvoyé par erreur dans un socket
    // de commande exécuterait quelque chose. Ils doivent être inconnus.
    let inconnus = [
        "bidule",
        "statuss",
        "ligth",
        "sta",
        "chan",
        "temp",
        "unreadable",
        "end",
        "err",
        "STATUS",
        "Light",
        "FAN",
        "--help",
        "0",
        "🌈",
    ];

    for verbe in inconnus {
        for ligne in [verbe.to_owned(), format!("{verbe} un deux trois")] {
            let erreur = parse_request(&ligne).expect_err("un verbe inconnu est refusé");
            assert_eq!(
                erreur,
                RequestError::UnknownVerb {
                    verb: verbe.to_owned()
                },
                "« {ligne} » — l'erreur doit porter le premier mot, et rien d'autre"
            );

            let message = erreur.to_string();
            assert!(
                message.contains(verbe),
                "le message doit citer « {verbe} » : « {message} »"
            );

            let _: &dyn std::error::Error = &erreur;
        }
    }

    // Et rien ne panique, quelle que soit l'entrée. Le risque réel n'est pas le verbe exotique :
    // c'est le découpage d'une chaîne UTF-8 à un décalage fixe, qui panique dès qu'un caractère
    // multioctet chevauche la coupe. D'où les accents, les emoji et les combinaisons.
    let accents = "é".repeat(1000);
    let lettres = "a".repeat(1024);
    let emoji = "🌈".repeat(400);
    let hostiles = [
        "é",
        "lightéé",
        "statusé",
        "🌈status",
        "light 🌈 ff2080",
        "fan é pwm 60",
        "l\u{0}ight",
        "\u{feff}status",
        "e\u{301}tat",
        "light\tall\tff2080",
        "        ",
        "-",
        ":",
        "fan:",
        "light fan:",
        "light slot:",
        accents.as_str(),
        lettres.as_str(),
        emoji.as_str(),
    ];
    for entree in hostiles {
        // La valeur n'est pas ce qu'on vérifie : c'est que l'appel **revient**.
        let _ = parse_request(entree);
    }
}

// ---------------------------------------------------------------------------
// 3 — les lignes vides, tronquées, hors bornes et démesurées
// ---------------------------------------------------------------------------

#[test]
fn une_ligne_vide_tronquee_ou_de_dix_kilo_octets_est_refusee() {
    // Contrat d'API — `Empty` : « Ligne vide ou uniquement des espaces ».
    for vide in ["", " ", "   ", "\t", " \t \t "] {
        assert_eq!(
            parse_request(vide),
            Err(RequestError::Empty),
            "« {vide:?} » ne porte aucune commande"
        );
    }

    // Contrat d'API — `BadArgument { verb, reason }` : « Verbe connu, arguments mauvais. Le
    // `Display` dit lequel et pourquoi. » Le verbe est reconnu, donc l'erreur doit le dire : un
    // `UnknownVerb` ici enverrait le client chercher une faute de frappe qui n'existe pas.
    let tronquees = [
        ("light", "light"),
        ("light all", "light"),
        ("light fan:radiateur-haut", "light"),
        ("fan", "fan"),
        ("fan nzxtsmart2:fan-1", "fan"),
        ("fan nzxtsmart2:fan-1 pwm", "fan"),
    ];
    for (ligne, verbe) in tronquees {
        let erreur = parse_request(ligne).expect_err("une requête tronquée est refusée");
        let RequestError::BadArgument { verb, reason } = &erreur else {
            panic!("« {ligne} » doit donner un BadArgument, pas {erreur:?}");
        };
        assert_eq!(verb.as_str(), verbe, "l'erreur nomme le verbe reçu");
        assert!(!reason.is_empty(), "l'erreur doit dire pourquoi");

        let message = erreur.to_string();
        assert!(
            message.contains(verbe) && message.contains(reason.as_str()),
            "le Display dit lequel et pourquoi : « {message} »"
        );
    }

    // Verbe connu, argument hors bornes — l'exemple même du contrat :
    // `< fan nzxtsmart2:fan-1 pwm 250` → `> err consigne 250 hors bornes : attendu 0 à 100`.
    // 250 tient dans un `u8` : c'est exactement le cas qu'un décodeur laisserait passer s'il se
    // contentait du type.
    let hors_bornes = [
        format!("fan {CANAL} pwm 250"),
        format!("fan {CANAL} pwm 101"),
        format!("fan {CANAL} pwm -1"),
        format!("fan {CANAL} pwm beaucoup"),
        format!("fan {CANAL} rapide"),
        "light all zzzzzz".to_owned(),
        "light all ff20".to_owned(),
        "light all #ff2080".to_owned(),
        "light slot:4 ff2080".to_owned(),
        "light slot:-1 ff2080".to_owned(),
        "light fan:milieu-du-plafond ff2080".to_owned(),
    ];
    for ligne in hors_bornes {
        let erreur = parse_request(&ligne).expect_err("un argument mauvais est refusé");
        assert!(
            matches!(erreur, RequestError::BadArgument { .. }),
            "« {ligne} » : verbe connu et argument mauvais, donc BadArgument — reçu {erreur:?}"
        );
    }

    // Contrat d'API — `MAX_LINE_LEN: usize = 1024`, « Au-delà, la ligne est refusée sans être
    // accumulée : un client qui envoie un mégaoctet sans `\n` ne doit pas faire enfler la mémoire
    // du démon. »
    assert_eq!(MAX_LINE_LEN, 1024);

    // Les deux bornes. 1024 octets sont acceptés *en longueur* — refusés pour leur contenu, ce qui
    // n'est pas la même erreur ; 1025 sont refusés pour leur longueur. Sans ce couple, un
    // décodeur qui compte `>=` au lieu de `>` passerait inaperçu.
    let juste = ligne_de(MAX_LINE_LEN);
    assert_eq!(juste.len(), MAX_LINE_LEN);
    let erreur = parse_request(&juste).expect_err("le contenu reste faux");
    assert!(
        !matches!(erreur, RequestError::TooLong { .. }),
        "{MAX_LINE_LEN} octets sont la longueur maximale *acceptée* — reçu {erreur:?}"
    );

    let trop = ligne_de(MAX_LINE_LEN + 1);
    assert_eq!(
        parse_request(&trop),
        Err(RequestError::TooLong {
            given: MAX_LINE_LEN + 1
        }),
        "un octet de trop suffit"
    );

    // La ligne de 10 Ko de l'énoncé.
    let enorme = ligne_de_dix_kilo_octets();
    assert_eq!(enorme.len(), 10_240);
    let erreur = parse_request(&enorme).expect_err("10 Ko sont refusés");
    assert_eq!(
        erreur,
        RequestError::TooLong { given: 10_240 },
        "l'erreur porte la longueur reçue — et, la variante n'ayant que ce champ, rien d'autre"
    );

    // Le point du test : **l'erreur ne recopie pas la ligne.** Contrat d'API — « recopier une
    // ligne de 10 Ko dans une erreur pour l'afficher ensuite, c'est refaire au moment du
    // diagnostic exactement l'allocation qu'on refusait à l'analyse ». Vérifié sur le `Display`
    // *et* sur le `Debug` : un `#[derive(Debug)]` qui porterait la ligne la ferait ressortir dans
    // le premier journal venu.
    let affichee = erreur.to_string();
    let debogage = format!("{erreur:?}");
    for texte in [&affichee, &debogage] {
        assert!(
            !texte.contains(MARQUEUR),
            "l'erreur recopie la ligne refusée : « {texte} »"
        );
        assert!(
            !texte.contains(enorme.as_str()),
            "l'erreur recopie la ligne refusée en entier"
        );
        assert!(
            texte.len() < MAX_LINE_LEN,
            "un message d'erreur ne doit pas dépasser la limite qu'il fait respecter : {} octets",
            texte.len()
        );
    }

    // Ce qui reste au diagnostic, c'est la longueur — donc elle doit être dite.
    assert!(
        affichee.contains("10240"),
        "le message doit dire la longueur reçue : « {affichee} »"
    );
    let _: &dyn std::error::Error = &erreur;
}

// ---------------------------------------------------------------------------
// 4 — l'aller-retour des requêtes
// ---------------------------------------------------------------------------

#[test]
fn toute_requete_encodee_se_relit_a_l_identique() {
    // Contrat d'API — « `parse_request(&encode_request(&r)) == Ok(r)` pour toute requête ».
    //
    // C'est la propriété qui autorise l'interface graphique et le démon à parler sans se mettre
    // d'accord sur autre chose que ce module : tout ce que l'un peut vouloir dire, l'autre le
    // relit exactement.
    let mut temoins = vec![
        Request::Status,
        lumiere(LightTarget::All),
        lumiere(LightTarget::Fans),
        lumiere(LightTarget::Ram),
        Request::Animate {
            name: Some("vague".to_owned()),
            reglages: Vec::new(),
        },
        Request::Animate {
            name: Some("arc-en-ciel".to_owned()),
            reglages: Vec::new(),
        },
        Request::Animate {
            name: None,
            reglages: Vec::new(),
        },
        ventilateur(FanAction::Auto),
        ventilateur(FanAction::Pwm(0)),
        ventilateur(FanAction::Pwm(100)),
        Request::Fan {
            channel: "kraken2023elite:pump-speed".to_owned(),
            action: FanAction::Pwm(37),
        },
        Request::Light {
            target: LightTarget::All,
            color: Rgb::new(0, 0, 0),
        },
        Request::Light {
            target: LightTarget::Fans,
            color: Rgb::new(0xff, 0xff, 0xff),
        },
    ];

    // Les **dix** positions, pas un échantillon : ce sont elles qui traversent le protocole, et
    // deux d'entre elles portent le piège — « radiateur haut » a une espace, « arrière » un
    // accent. Ni l'une ni l'autre n'a sa place dans un protocole à jetons séparés par des espaces,
    // d'où le passage par le slug.
    for position in Position::ALL {
        temoins.push(lumiere(LightTarget::Fan(position)));
    }
    for slot in 0..4usize {
        temoins.push(lumiere(LightTarget::RamSlot(slot)));
    }

    for temoin in &temoins {
        let encodee = encode_request(temoin);
        assert!(
            !encodee.contains('\n'),
            "une requête tient sur une ligne : « {encodee} »"
        );
        assert!(!encodee.is_empty(), "{temoin:?} s'encode en rien");
        assert!(
            encodee.len() <= MAX_LINE_LEN,
            "une requête que le démon refuserait à la lecture : {} octets",
            encodee.len()
        );
        assert_eq!(
            parse_request(&encodee),
            Ok(temoin.clone()),
            "aller-retour de {temoin:?} par « {encodee} »"
        );
    }

    // Contrat d'API, `position.rs` — `slug()` : « Nom sans espace ni accent, pour le protocole
    // IPC », et `from_slug` sa réciproque. Les exemples cités sont `radiateur-haut`, `bas-gauche`
    // et `arriere`.
    assert_eq!(Position::RadiateurHaut.slug(), "radiateur-haut");
    assert_eq!(Position::BasGauche.slug(), "bas-gauche");
    assert_eq!(Position::Arriere.slug(), "arriere");

    let mut slugs: Vec<String> = Vec::new();
    for position in Position::ALL {
        let slug = position.slug();
        assert!(
            slug.is_ascii(),
            "« {slug} » n'est pas en ASCII — un accent ne traverse pas le protocole"
        );
        assert!(
            !slug.contains(' '),
            "« {slug} » contient une espace, or les champs sont séparés par des espaces"
        );
        assert_eq!(slug, slug.to_lowercase(), "le slug est en kebab-case");
        assert_eq!(
            Position::from_slug(&slug),
            Ok(position),
            "aller-retour du slug de {position:?}"
        );
        slugs.push(slug);
    }
    slugs.sort_unstable();
    slugs.dedup();
    assert_eq!(
        slugs.len(),
        Position::ALL.len(),
        "deux positions ne peuvent pas partager un slug — la cible deviendrait ambiguë"
    );

    // Le nom d'affichage n'est **pas** un slug : c'est lui qui porte l'espace et l'accent, et il
    // ne doit pas se glisser dans le protocole par une porte de derrière.
    for nom in ["radiateur haut", "arrière", "bas gauche", "Radiateur-Haut"] {
        assert!(
            Position::from_slug(nom).is_err(),
            "« {nom} » n'est pas un slug"
        );
    }
    assert!(
        parse_request(&format!("light fan:radiateur haut {COULEUR_HEX}")).is_err(),
        "une position à espace casse le découpage en jetons : elle doit être refusée, pas devinée"
    );
}

// ---------------------------------------------------------------------------
// 5 — le cadrage des réponses
// ---------------------------------------------------------------------------

#[test]
fn aucune_ligne_de_donnees_ne_peut_se_faire_passer_pour_une_fin_de_reponse() {
    // Contrat d'API, « Forme du dialogue » — « une réponse = zéro ou plusieurs lignes de données,
    // puis **exactement une** ligne de fin : `end` en cas de succès, `err <message>` en cas
    // d'échec. Une ligne de données ne commence **jamais** par `end` ni par `err` — c'est ce qui
    // rend la fin de réponse non ambiguë sans compter les lignes. »
    //
    // Contrat d'API, note sur ce test — « la parade attendue est de préfixer toute ligne de
    // données par son type […] mais le test doit le vérifier plutôt que de le supposer ».
    //
    // C'est le seul garde-fou du cadre. S'il cède, le client ne voit pas d'erreur : il lit une
    // réponse tronquée, affiche un état partiel, et prend les lignes restantes pour la réponse
    // suivante — un décalage qui ne se rattrape jamais.

    // `is_terminal` : vraie pour les deux lignes de fin, fausse pour les trois autres.
    assert!(ResponseLine::End.is_terminal());
    for message in ["oups", "commande « bidule » inconnue", "end", "err", ""] {
        assert!(
            ResponseLine::Error {
                message: message.to_owned()
            }
            .is_terminal(),
            "`err {message}` termine la réponse"
        );
    }

    // Les trois lignes de données, d'abord sur les valeurs paisibles du dialogue d'exemple.
    for donnee in [
        canal(CANAL, "manual"),
        ResponseLine::Channel {
            channel: "kraken2023elite:pump-speed".to_owned(),
            position: None,
            rpm: Some(2400),
            pwm: Some(80),
            mode: "firmware".to_owned(),
        },
        ResponseLine::Temp {
            sensor: "kraken2023elite:coolant".to_owned(),
            millidegrees: 34_200,
        },
        ResponseLine::Unreadable {
            subject: "nzxtsmart2:fan-4".to_owned(),
            reason: "descripteur ferme par le noyau".to_owned(),
        },
    ] {
        ne_termine_jamais(&donnee);
    }

    // Puis sur les noms hostiles, dans les **quatre** champs qui viennent d'ailleurs que de nous :
    // le canal, son mode, le capteur, le sujet illisible. Un nom de canal est construit depuis la
    // cartographie du matériel ; rien ne garantit qu'il ne s'appellera jamais `err`.
    for &nom in NOMS_HOSTILES {
        ne_termine_jamais(&canal(nom, "manual"));
        ne_termine_jamais(&canal(CANAL, nom));
        ne_termine_jamais(&ResponseLine::Temp {
            sensor: nom.to_owned(),
            millidegrees: -1,
        });
        ne_termine_jamais(&ResponseLine::Unreadable {
            subject: nom.to_owned(),
            reason: "raison quelconque".to_owned(),
        });
        ne_termine_jamais(&ResponseLine::Unreadable {
            subject: "nzxtsmart2:fan-4".to_owned(),
            reason: nom.to_owned(),
        });
    }

    // Les cas à espaces, là où le contrat les autorise : la raison d'un `unreadable` est le
    // dernier champ de sa ligne, « pris jusqu'à la fin ». Une raison qui *commence* par `err` ou
    // `end` est le cas que l'énoncé nomme — « err quelque chose ».
    for raison in [
        "err quelque chose",
        "end de la course",
        "err: ENODEV, le peripherique a disparu",
        "end",
        "err",
    ] {
        ne_termine_jamais(&ResponseLine::Unreadable {
            subject: "nzxtsmart2:fan-4".to_owned(),
            reason: raison.to_owned(),
        });
    }

    // L'injection par saut de ligne : le cas qui casse le cadrage sans qu'aucun champ ne commence
    // par `end`. Si l'encodeur se contente d'interpoler, `unreadable capteur boom\nend` part en
    // **deux** lignes, dont la seconde est une fin de réponse parfaitement formée. Le client
    // s'arrête là et prend la suite pour la réponse d'après.
    //
    // Le contrat ne prescrit pas le remède — refuser, échapper, ou nettoyer à la source sont trois
    // réponses valables — donc le test ne vérifie que l'invariant : une ligne, une seule.
    //
    // Ces poisons-ci ne portent pas d'espace : ils vont dans **tous** les champs, y compris ceux
    // qui, n'étant pas le dernier de leur ligne, ne peuvent pas en accueillir.
    for poison in ["boom\nend", "\nend", "end\n", "a\r\nend", "\n\n\nend"] {
        ne_termine_jamais(&canal(poison, "manual"));
        ne_termine_jamais(&canal(CANAL, poison));
        ne_termine_jamais(&ResponseLine::Temp {
            sensor: poison.to_owned(),
            millidegrees: 0,
        });
        ne_termine_jamais(&ResponseLine::Unreadable {
            subject: poison.to_owned(),
            reason: "raison quelconque".to_owned(),
        });
    }

    // Et les mêmes avec une espace, dans le seul champ que le contrat autorise à en porter : la
    // raison, dernier champ de sa ligne, « prise jusqu'à la fin ».
    for poison in [
        "boom\nerr injecte",
        "fin\nerr quelque chose",
        "\nend de la course",
        "a\r\nend",
    ] {
        ne_termine_jamais(&ResponseLine::Unreadable {
            subject: "nzxtsmart2:fan-4".to_owned(),
            reason: poison.to_owned(),
        });
    }

    // La démonstration de bout en bout : un client qui lit jusqu'à la première ligne terminale
    // retrouve **exactement** les lignes de données, sans en connaître le nombre. C'est la
    // promesse du contrat, jouée telle quelle.
    let donnees = [
        canal("err", "end"),
        canal("end", "err"),
        ResponseLine::Temp {
            sensor: "err".to_owned(),
            millidegrees: -12_345,
        },
        ResponseLine::Unreadable {
            subject: "end".to_owned(),
            reason: "err quelque chose".to_owned(),
        },
    ];

    for fin in [
        ResponseLine::End,
        ResponseLine::Error {
            message: "consigne 250 hors bornes : attendu 0 à 100".to_owned(),
        },
    ] {
        let (lues, terminale) = lit_une_reponse(&flux(&donnees, &fin));
        assert_eq!(
            lues[..],
            donnees[..],
            "les lignes de données doivent revenir intactes, dans l'ordre"
        );
        assert_eq!(terminale, fin, "et la réponse doit finir sur {fin:?}");
    }

    // Une réponse vide : rien que la fin. Le lecteur ne doit pas attendre une ligne de données.
    let (lues, terminale) = lit_une_reponse(&flux(&[], &ResponseLine::End));
    assert!(lues.is_empty());
    assert_eq!(terminale, ResponseLine::End);

    // Et le sens inverse : `err` gagne toujours, quel que soit ce qui suit. Un message d'erreur qui
    // ressemble à une ligne de données reste un message d'erreur — c'est sa position en tête de
    // ligne qui tranche, pas son contenu.
    assert_eq!(
        parse_response_line("err chan nzxtsmart2:fan-1 - - - manual"),
        Ok(ResponseLine::Error {
            message: "chan nzxtsmart2:fan-1 - - - manual".to_owned()
        })
    );
    assert_eq!(parse_response_line("end"), Ok(ResponseLine::End));
}

// ---------------------------------------------------------------------------
// 8 — l'illisible se dit, et ne se déguise pas en zéro
// ---------------------------------------------------------------------------

#[test]
fn un_canal_illisible_ne_se_decode_ni_en_zero_ni_en_rien() {
    // Contrat d'API — `Unreadable { subject, reason }` : « ⚠️ Ni omise, ni remplacée par zéro : un
    // canal illisible affiché à 0 tr/min est un mensonge, et un canal omis fait croire qu'il
    // n'existe pas. »
    //
    // Les deux erreurs sont silencieuses par construction : un `0` s'affiche comme une valeur
    // plausible, une omission comme un matériel absent. Aucune des deux ne remonte au client.
    let illisible = ResponseLine::Unreadable {
        subject: "nzxtsmart2:fan-4".to_owned(),
        reason: "descripteur ferme par le noyau  (ENODEV), arrêté".to_owned(),
    };

    let encodee = encode_response_line(&illisible);
    assert!(
        encodee.starts_with("unreadable "),
        "la ligne porte son type en tête : « {encodee} »"
    );

    // Contrat d'API — « Les messages d'erreur et les raisons peuvent contenir des espaces : ce
    // sont toujours **le dernier champ** de leur ligne, et ils sont pris jusqu'à la fin. » La
    // raison témoin porte donc une double espace et un accent : un découpage naïf sur *toutes* les
    // espaces les perdrait.
    let relue = parse_response_line(&encodee).expect("une ligne encodée se relit");
    assert_eq!(relue, illisible, "aller-retour exact, raison comprise");

    // « ni en `Channel` à zéro » — le décodage ne doit pas fabriquer un canal.
    assert!(
        matches!(relue, ResponseLine::Unreadable { .. }),
        "{relue:?} n'est pas un signalement d'illisibilité"
    );
    assert!(
        !matches!(relue, ResponseLine::Channel { .. }),
        "un canal illisible n'est pas un canal à zéro"
    );

    // « ni en rien » — la ligne se décode, elle n'est pas avalée. Et ce qui ne se décode pas donne
    // une erreur **explicite**, qui porte la ligne fautive : contrat d'API,
    // `ResponseError { line, reason }`. Ici la ligne est courte et vient du démon : la recopier a
    // du sens, contrairement à la ligne de 10 Ko du test n° 3.
    let erreur: ResponseError =
        parse_response_line("bidule truc machin").expect_err("un type de ligne inconnu est refusé");
    assert_eq!(erreur.line, "bidule truc machin");
    assert!(!erreur.reason.is_empty(), "l'erreur doit dire pourquoi");
    let _: &dyn std::error::Error = &erreur;

    // Le pendant : un champ absent s'écrit `-` (contrat d'API), et `-` se relit en `None`, jamais
    // en `Some(0)`. C'est la même distinction que ci-dessus, vue depuis la ligne `chan`.
    let muet = parse_response_line("chan kraken2023elite:pump-speed - - - firmware")
        .expect("ligne bien formée");
    let ResponseLine::Channel {
        channel,
        position,
        rpm,
        pwm,
        mode,
    } = &muet
    else {
        panic!("« chan … » doit se décoder en Channel, pas en {muet:?}");
    };
    assert_eq!(channel.as_str(), "kraken2023elite:pump-speed");
    assert_eq!(*position, None, "`-` n'est pas une position");
    assert_eq!(*rpm, None, "`-` n'est pas 0 tr/min");
    assert_eq!(*pwm, None, "`-` n'est pas 0 %");
    assert_ne!(*rpm, Some(0), "0 tr/min serait un mensonge");
    assert_ne!(*pwm, Some(0), "0 % serait un mensonge");
    assert_eq!(mode.as_str(), "firmware");

    // Et dans l'autre sens : `None` s'**écrit** `-`, il ne s'écrit pas `0`. Sans cette
    // vérification, un encodeur qui rendrait `chan … 0 0 manual` pour un canal muet passerait le
    // décodage ci-dessus sans être vu — c'est le mensonge le plus facile à commettre, parce que
    // `unwrap_or_default()` est plus court à écrire que le cas absent.
    let muet = ResponseLine::Channel {
        channel: "kraken2023elite:pump-speed".to_owned(),
        position: None,
        rpm: None,
        pwm: None,
        mode: "firmware".to_owned(),
    };
    let encodee = encode_response_line(&muet);
    assert_eq!(
        encodee, "chan kraken2023elite:pump-speed - - - firmware",
        "un champ absent s'écrit `-` (contrat d'API), jamais `0`"
    );
    assert_eq!(
        parse_response_line(&encodee),
        Ok(muet),
        "et l'absence survit à l'aller-retour"
    );

    // Et zéro reste zéro : les deux se distinguent dans les deux sens, sinon la distinction ne
    // sert à rien.
    let arrete = ResponseLine::Channel {
        channel: "nzxtsmart2:fan-2".to_owned(),
        position: Some(Position::BasMilieu),
        rpm: Some(0),
        pwm: Some(0),
        mode: "manual".to_owned(),
    };
    let encodee = encode_response_line(&arrete);
    assert_eq!(
        parse_response_line(&encodee),
        Ok(arrete),
        "un ventilateur réellement à l'arrêt reste à l'arrêt : « {encodee} »"
    );

    // Le dialogue d'exemple du contrat, décodé ligne à ligne — la seule séquence de réponse dont
    // le texte exact soit publié.
    //
    //     > chan nzxtsmart2:fan-1 radiateur-haut 1200 60 manual
    //     > chan kraken2023elite:pump-speed - 2400 80 firmware
    //     > temp kraken2023elite:coolant 34200
    //     > end
    assert_eq!(
        parse_response_line("chan nzxtsmart2:fan-1 radiateur-haut 1200 60 manual"),
        Ok(canal(CANAL, "manual"))
    );
    assert_eq!(
        parse_response_line("temp kraken2023elite:coolant 34200"),
        Ok(ResponseLine::Temp {
            sensor: "kraken2023elite:coolant".to_owned(),
            millidegrees: 34_200,
        })
    );
    assert_eq!(
        encode_response_line(&canal(CANAL, "manual")),
        "chan nzxtsmart2:fan-1 radiateur-haut 1200 60 manual",
        "et le réencodage rend le texte du contrat, mot pour mot"
    );
}

// ---------------------------------------------------------------------------
// 9 — les millidegrés entiers
// ---------------------------------------------------------------------------

#[test]
fn une_temperature_fait_un_aller_retour_exact_y_compris_negative_et_nulle() {
    // Contrat d'API — `Temp { sensor, millidegrees: i32 }` : « En **millidegrés entiers**, comme
    // hwmon les publie. Pas de flottant : un aller-retour texte sur un `f32` ne rend pas toujours
    // le même nombre, et un protocole doit être exact. »
    //
    // Le zéro et les négatifs ne sont pas des curiosités : hwmon publie des offsets et des deltas,
    // et un capteur débranché rend couramment une valeur négative. Un décodeur qui lirait un `u32`
    // compilerait, passerait tous les cas positifs, et se tromperait de signe le jour où ça compte.
    let valeurs = [
        0i32,
        1,
        -1,
        1_000,
        -1_000,
        34_200,   // la température du liquide, dialogue d'exemple du contrat
        -273_150, // le zéro absolu, en millidegrés Celsius
        i32::MAX, // 2 147 483 647 — non représentable en `f32`, qui l'arrondirait à 2 147 483 648
        i32::MIN,
        i32::MAX - 1,
        i32::MIN + 1,
    ];

    for millidegrees in valeurs {
        let temperature = ResponseLine::Temp {
            sensor: "kraken2023elite:coolant".to_owned(),
            millidegrees,
        };
        let encodee = encode_response_line(&temperature);

        assert_eq!(
            parse_response_line(&encodee),
            Ok(temperature),
            "aller-retour de {millidegrees} par « {encodee} »"
        );

        // Écrit en base dix, tel quel : c'est ce qui rend l'aller-retour exact et la ligne
        // lisible dans un journal. Un `34.2` ou un `3.42e1` seraient tous deux des flottants
        // déguisés, et le second ne se relirait même pas comme un entier.
        assert!(
            encodee.ends_with(&millidegrees.to_string()),
            "la ligne doit finir sur la valeur en base dix : « {encodee} »"
        );
        for interdit in ['.', ',', 'e', 'E'] {
            assert!(
                !encodee
                    .rsplit(' ')
                    .next()
                    .unwrap_or_default()
                    .contains(interdit),
                "« {interdit} » dans le champ de température : « {encodee} » n'est pas un entier"
            );
        }
    }

    // Le signe négatif se relit tel qu'il s'écrit — et le texte est celui qu'on attend, pas un
    // encodage complémenté ni une valeur décalée.
    assert_eq!(
        encode_response_line(&ResponseLine::Temp {
            sensor: "carte-mere:vrm".to_owned(),
            millidegrees: -40_000,
        }),
        "temp carte-mere:vrm -40000"
    );
    assert_eq!(
        parse_response_line("temp carte-mere:vrm -40000"),
        Ok(ResponseLine::Temp {
            sensor: "carte-mere:vrm".to_owned(),
            millidegrees: -40_000,
        })
    );
    assert_eq!(
        parse_response_line("temp carte-mere:vrm 0"),
        Ok(ResponseLine::Temp {
            sensor: "carte-mere:vrm".to_owned(),
            millidegrees: 0,
        }),
        "zéro est une température, pas une absence — un capteur absent se dit `unreadable`"
    );
}
