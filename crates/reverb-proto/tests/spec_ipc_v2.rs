//! Tests d'intention de l'extension du protocole (issue #19).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #19 et son commentaire « Contrat d'API »
//! seuls, sans relire un corps de fonction. Ils prolongent `spec_ipc.rs`, qui reste la
//! spécification du protocole tel que l'issue #17 l'a figé — **ce fichier n'y touche pas** : ce
//! qui y est écrit doit continuer de passer tel quel, c'est la moitié de ce que « le protocole
//! s'étend, il ne casse pas » veut dire.
//!
//! ## Ce que l'extension ajoute, et ce qu'elle risque
//!
//! Deux choses : des paires `clé=valeur` derrière `animate`, et un verbe `geometry` en lecture
//! comme en écriture, avec sa ligne de réponse `geom`.
//!
//! Le protocole les **transporte sans les interpréter** : `reverb-proto` ne peut pas dépendre de
//! `reverb-anim` (ce serait le relevé matériel qui dépendrait de ce qu'on invente), et n'a donc
//! aucun moyen de savoir qu'une animation accepte `couleur`. Ce fichier ne vérifie donc jamais
//! qu'une clé est valide — seulement qu'une paire est une paire, qu'elle traverse intacte et
//! dans l'ordre, et qu'aucune valeur ne peut casser le cadrage des lignes.
//!
//! Ce dernier point est celui qui compte. Les valeurs viennent maintenant d'une frappe humaine
//! (`animate comete couleur=ff00ff`) et non plus seulement de nous ; un `\n` qui traverserait
//! l'encodage scinderait une requête en deux commandes, ou une réponse en deux lignes dont la
//! seconde pourrait se faire prendre pour la fin. Le client s'y décalerait pour toujours.
//!
//! ## Trois points que le contrat laisse ouverts, et que ces tests tranchent
//!
//! 1. **Une paire s'écrit `clé=valeur`, sans espace autour du `=`**, et les paires suivent le nom
//!    de l'animation ou la cible, séparées par des espaces — c'est la forme que l'issue écrit
//!    dans ses exemples (`animate comete couleur=ff00ff vitesse=2`,
//!    `geometry radiateur-haut angle=90 sens=horaire`).
//! 2. **L'ordre des paires est conservé** de bout en bout. Le contrat exige
//!    `parse_request(encode_request(r)) == r` « réglages compris », or `Vec` est ordonné : un
//!    encodeur qui trierait les paires casserait cette égalité. Conserver l'ordre est aussi ce
//!    qui permet de rendre le message d'erreur d'`reverb-anim` sur la **première** clé fautive
//!    dans l'ordre où l'utilisateur les a tapées.
//! 3. **La ligne `geom` a exactement quatre jetons** — son sens est un mot d'un vocabulaire fermé
//!    (`horaire`, `antihoraire`), pas du texte libre. #17 réservait la règle « dernier champ pris
//!    jusqu'à la fin de la ligne » aux **messages** et aux **raisons**, qui sont écrits pour être
//!    lus par un humain ; l'appliquer ici ferait passer `geom radiateur-haut 90 horaire bidule`
//!    pour une orientation valide. La contrepartie est que les blancs d'un sens se neutralisent
//!    comme ceux d'un champ non final — ce que le test exige indirectement, en réclamant qu'une
//!    ligne encodée se relise toujours.
//!
//! Aucun accès matériel, aucune IO, aucun socket : `reverb-proto` est pur, ses tests aussi.

use reverb_proto::Position;
use reverb_proto::ipc::{
    MAX_LINE_LEN, Request, RequestError, ResponseLine, encode_request, encode_response_line,
    parse_request, parse_response_line,
};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Une paire brute, telle que le protocole la transporte.
fn paire(cle: &str, valeur: &str) -> (String, String) {
    (cle.to_owned(), valeur.to_owned())
}

/// Une requête d'animation nommée, avec ses réglages.
fn animer(nom: &str, reglages: &[(&str, &str)]) -> Request {
    Request::Animate {
        name: Some(nom.to_owned()),
        reglages: reglages.iter().map(|(c, v)| paire(c, v)).collect(),
    }
}

/// Une requête de géométrie sur une cible, avec ses réglages.
fn geometrie(cible: Option<&str>, reglages: &[(&str, &str)]) -> Request {
    Request::Geometry {
        cible: cible.map(str::to_owned),
        reglages: reglages.iter().map(|(c, v)| paire(c, v)).collect(),
    }
}

/// Une ligne de réponse `geom`.
fn geom(position: &str, angle: u16, sens: &str) -> ResponseLine {
    ResponseLine::Geom {
        position: position.to_owned(),
        angle,
        sens: sens.to_owned(),
    }
}

/// Noms **hostiles** : ceux qui, mal encodés, produiraient une ligne qu'un client prendrait pour
/// la fin de la réponse, ou une requête qu'un démon prendrait pour deux.
///
/// Même liste d'esprit que celle de `spec_ipc.rs` : le protocole doit tenir même si un champ
/// s'appelle littéralement `err`. Ici la raison est plus forte encore — ces champs viennent d'une
/// frappe humaine, pas seulement de la cartographie du matériel.
const HOSTILES: &[&str] = &[
    "err",
    "end",
    "error",
    "endpoint",
    "err:1",
    "ERR",
    "boom\nend",
    "\nend",
    "end\n",
    "a\r\nend",
    "\n\n\nend",
    "va leur",
    "\t",
    "  ",
    "a\u{0}b",
    "\u{feff}a",
];

/// Vérifie qu'une requête encodée tient sur **une** ligne physique, quoi qu'on ait mis dedans.
///
/// C'est l'invariant du cadrage vu du côté des requêtes : le démon lit une ligne, une commande.
/// Un `\n` qui survivrait à l'encodage ferait exécuter une seconde commande que personne n'a
/// demandée — `animate vague couleur=$'ff0000\nlight all ffffff'` depuis un script suffirait.
fn tient_sur_une_ligne(requete: &Request) -> String {
    let encodee = encode_request(requete);
    assert!(
        !encodee.contains('\n'),
        "aucun saut de ligne dans une requête encodée : « {encodee} »"
    );
    assert!(
        !encodee.contains('\r'),
        "aucun retour chariot dans une requête encodée : « {encodee} »"
    );
    assert_eq!(
        encodee.lines().count(),
        1,
        "une requête tient sur une seule ligne : « {encodee} »"
    );
    encodee
}

/// Vérifie qu'une ligne de données ne peut pas se faire prendre pour une fin de réponse.
///
/// Reprise de la règle de `spec_ipc.rs` n° 5, appliquée au champ neuf : « Une ligne de données ne
/// commence **jamais** par `end` ni par `err` », et elle tient sur une seule ligne physique.
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
        !encodee.contains('\n'),
        "aucun saut de ligne dans une ligne de données : « {encodee} »"
    );
    assert!(
        !encodee.contains('\r'),
        "aucun retour chariot dans une ligne de données : « {encodee} »"
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

// ---------------------------------------------------------------------------
// 7 — `animate vague` reste accepté, et les réglages le prolongent
// ---------------------------------------------------------------------------

#[test]
fn animate_vague_reste_accepte_et_les_reglages_ne_font_que_le_prolonger() {
    // Test d'intention n° 7 de l'issue — « `animate vague` reste accepté par le protocole
    // (compatibilité) », critère d'acceptation — « `animate vague` (sans paramètre) continue de
    // marcher : le protocole s'étend, il ne casse pas », et contrat d'API — « `animate vague`
    // sans paire reste accepté et rend `reglages` vide ».
    //
    // La forme courte est celle que le README publie (`echo 'animate vague' | socat …`) et celle
    // que la ligne de commande envoie déjà. Un champ ajouté qui rendrait obligatoire une paire
    // vide casserait les deux, et le seul symptôme serait un `err` là où ça marchait la veille.
    assert_eq!(
        parse_request("animate vague"),
        Ok(Request::Animate {
            name: Some("vague".to_owned()),
            reglages: Vec::new(),
        }),
        "la forme sans paramètre reste la forme sans paramètre"
    );
    assert_eq!(
        encode_request(&animer("vague", &[])),
        "animate vague",
        "et se réencode sans paire vide ni séparateur en trop"
    );
    assert_eq!(
        parse_request("animate off"),
        Ok(Request::Animate {
            name: None,
            reglages: Vec::new(),
        }),
        "l'arrêt reste l'arrêt"
    );
    assert_eq!(
        encode_request(&Request::Animate {
            name: None,
            reglages: Vec::new(),
        }),
        "animate off"
    );

    // La forme longue, telle que l'issue l'écrit : `animate comete couleur=ff00ff vitesse=2`.
    // Vérifiée dans les deux sens — c'est le réencodage qui fige la forme du fil, sans lui un
    // encodeur pourrait écrire `animate comete --couleur ff00ff` et le test resterait vert.
    let cas: [(&str, Request); 5] = [
        (
            "animate comete couleur=ff00ff vitesse=2",
            animer("comete", &[("couleur", "ff00ff"), ("vitesse", "2")]),
        ),
        (
            "animate onde sens=bas-haut",
            animer("onde", &[("sens", "bas-haut")]),
        ),
        (
            "animate vague couleur=ff2080",
            animer("vague", &[("couleur", "ff2080")]),
        ),
        (
            "geometry radiateur-haut angle=90 sens=horaire",
            geometrie(
                Some("radiateur-haut"),
                &[("angle", "90"), ("sens", "horaire")],
            ),
        ),
        ("geometry", geometrie(None, &[])),
    ];
    for (ligne, attendue) in &cas {
        assert_eq!(
            parse_request(ligne).as_ref(),
            Ok(attendue),
            "décodage de « {ligne} »"
        );
        assert_eq!(
            &encode_request(attendue),
            ligne,
            "réencodage de {attendue:?} — la forme du fil ne doit pas dériver"
        );
    }

    // L'ordre des paires est celui de la frappe, et il survit à l'aller-retour. Deux paires
    // permutées ne sont pas la même requête : c'est ce qui décide quelle clé un message d'erreur
    // nommera en premier.
    let dans_l_ordre = animer("comete", &[("couleur", "ff00ff"), ("vitesse", "2")]);
    let permutee = animer("comete", &[("vitesse", "2"), ("couleur", "ff00ff")]);
    assert_ne!(dans_l_ordre, permutee, "deux ordres, deux requêtes");
    assert_eq!(
        encode_request(&permutee),
        "animate comete vitesse=2 couleur=ff00ff",
        "l'encodeur ne réordonne pas les paires"
    );

    // Une clé répétée traverse deux fois : le protocole transporte, il n'arbitre pas. C'est à
    // `reverb-anim` de dire ce qu'il en fait — le protocole ne peut pas trancher sans connaître
    // les animations, et perdre une paire en route lui ferait prendre la décision en silence.
    let repetee = animer("comete", &[("couleur", "ff0000"), ("couleur", "00ff00")]);
    let encodee = encode_request(&repetee);
    assert_eq!(encodee, "animate comete couleur=ff0000 couleur=00ff00");
    assert_eq!(parse_request(&encodee), Ok(repetee));
}

// ---------------------------------------------------------------------------
// `geometry` seul est la forme de lecture
// ---------------------------------------------------------------------------

#[test]
fn geometry_seul_est_la_forme_de_lecture_et_ne_change_rien() {
    // Contrat d'API — « `geometry` seul rend `cible: None` — c'est la forme de lecture », et
    // comportement attendu de l'issue : `geometry` rend la géométrie courante, `geometry
    // radiateur-haut angle=90 sens=horaire` la change.
    //
    // La distinction porte tout le critère « La fenêtre n'écrit aucun fichier : le socket reste
    // l'unique franchissement de privilège ». Une lecture qui se déciderait autrement qu'à
    // l'absence de cible — par exemple à l'absence de paires — rendrait `geometry radiateur-haut`
    // ambigu, et c'est le démon, qui est root, qui trancherait.
    assert_eq!(parse_request("geometry"), Ok(geometrie(None, &[])));

    // Les dix positions comme cible, pas un échantillon : ce sont elles qui traversent le verbe,
    // et le slug est le seul nom de position qu'un protocole à jetons séparés par des espaces
    // peut porter (spec_ipc n° 4).
    for position in Position::ALL {
        let slug = position.slug();
        let slug = slug.as_str();
        let requete = geometrie(Some(slug), &[("angle", "90"), ("sens", "antihoraire")]);
        let ligne = format!("geometry {slug} angle=90 sens=antihoraire");
        assert_eq!(
            parse_request(&ligne).as_ref(),
            Ok(&requete),
            "décodage de « {ligne} »"
        );
        assert_eq!(encode_request(&requete), ligne, "réencodage de « {ligne} »");

        // La lecture d'un seul ventilateur : une cible, aucun réglage.
        let lecture = geometrie(Some(slug), &[]);
        assert_eq!(encode_request(&lecture), format!("geometry {slug}"));
        assert_eq!(
            parse_request(&format!("geometry {slug}")),
            Ok(lecture),
            "une cible sans réglage reste une cible sans réglage"
        );
    }

    // ⚠️ Le contrat ne dit pas ce que veut dire `geometry angle=90` — un réglage sans cible. Ce
    // test n'en tranche donc rien : il exige seulement qu'une **paire ne soit jamais prise pour
    // une cible**. Confondre les deux ferait chercher un ventilateur nommé « angle=90 », et le
    // message d'erreur enverrait sur une fausse piste.
    if let Ok(requete) = parse_request("geometry angle=90") {
        let Request::Geometry { cible, reglages } = requete else {
            panic!("« geometry angle=90 » ne peut pas être autre chose qu'une requête `geometry`");
        };
        assert_ne!(
            cible.as_deref(),
            Some("angle=90"),
            "une paire n'est pas une cible"
        );
        if cible.is_none() {
            assert_eq!(
                reglages,
                vec![paire("angle", "90")],
                "si la paire n'a pas de cible, elle reste une paire"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Une paire malformée est refusée
// ---------------------------------------------------------------------------

#[test]
fn une_paire_sans_egal_a_cle_vide_ou_a_valeur_vide_est_refusee() {
    // Contrat d'API — « une paire sans `=`, à clé vide ou à valeur vide est refusée par le
    // protocole ».
    //
    // C'est le seul contrôle que le protocole peut faire sans connaître les animations, et il
    // vaut la peine : une paire sans `=` est une faute de frappe (`animate comete couleur ff00ff`
    // — l'espace au lieu du signe), et une valeur vide vient d'une variable de shell non
    // substituée. Les deux, avalées en silence, donneraient une animation qui tourne avec des
    // réglages que personne n'a écrits.
    let malformees = [
        // Pas de signe égal.
        "animate comete couleur",
        "animate comete couleur ff00ff",
        "animate comete vitesse=2 couleur",
        "geometry radiateur-haut angle",
        "geometry radiateur-haut angle 90",
        // Clé vide.
        "animate comete =ff00ff",
        "geometry radiateur-haut =90",
        // Valeur vide.
        "animate comete couleur=",
        "geometry radiateur-haut angle=",
        // Ni l'un ni l'autre.
        "animate comete =",
        "geometry radiateur-haut =",
    ];

    for ligne in malformees {
        let erreur = parse_request(ligne).expect_err("une paire malformée est refusée");
        let RequestError::BadArgument { verb, reason } = &erreur else {
            panic!("« {ligne} » doit donner un BadArgument, pas {erreur:?}");
        };
        assert!(
            verb == "animate" || verb == "geometry",
            "« {ligne} » : le verbe est connu, l'erreur doit le nommer — reçu « {verb} »"
        );
        assert!(!reason.is_empty(), "l'erreur doit dire pourquoi");

        let message = erreur.to_string();
        assert!(
            message.contains(verb.as_str()) && message.contains(reason.as_str()),
            "le Display dit lequel et pourquoi : « {message} »"
        );
        let _: &dyn std::error::Error = &erreur;
    }

    // Et rien ne panique, quelle que soit l'entrée. Le risque n'est pas la clé exotique : c'est
    // le découpage d'une chaîne UTF-8 à un décalage fixe, qui panique dès qu'un caractère
    // multioctet chevauche la coupe — un `=` cherché à l'octet près dans « couleur=é » en est
    // l'occasion la plus courte.
    let longue = format!("animate comete couleur={}", "é".repeat(500));
    let hostiles = [
        "animate comete couleur=é",
        "animate comete é=ff00ff",
        "animate comete 🌈=🌈",
        "animate comete couleur==ff00ff",
        "animate comete ==",
        "animate 🌈 couleur=ff00ff",
        "geometry 🌈 angle=90",
        "geometry =",
        "geometry ==",
        "geometry \u{0}",
        longue.as_str(),
    ];
    for entree in hostiles {
        // La valeur n'est pas ce qu'on vérifie : c'est que l'appel **revient**.
        let _ = parse_request(entree);
    }

    // La limite de longueur continue de s'appliquer aux nouvelles formes : un client qui envoie
    // mille paires ne doit pas faire enfler la mémoire du démon davantage qu'un client qui envoie
    // mille fois `light`.
    let mut trop_longue = String::from("animate comete");
    while trop_longue.len() <= MAX_LINE_LEN {
        trop_longue.push_str(" couleur=ff00ff");
    }
    let longueur = trop_longue.len();
    assert_eq!(
        parse_request(&trop_longue),
        Err(RequestError::TooLong { given: longueur }),
        "une ligne au-delà de {MAX_LINE_LEN} octets est refusée, verbe neuf ou pas"
    );
}

// ---------------------------------------------------------------------------
// L'aller-retour, réglages compris
// ---------------------------------------------------------------------------

#[test]
fn toute_requete_a_reglages_se_relit_a_l_identique() {
    // Contrat d'API — « `parse_request(encode_request(r)) == r` pour toute requête valide,
    // réglages compris ».
    //
    // C'est ce qui autorise la fenêtre et le démon à parler sans se mettre d'accord sur autre
    // chose que ce module. La fenêtre construira des `Request` et le démon les relira ; tout ce
    // que l'une peut vouloir dire, l'autre doit le relire exactement — y compris une clé qu'aucune
    // animation d'aujourd'hui n'accepte, puisque le protocole ne les connaît pas.
    let mut temoins = vec![
        animer("vague", &[]),
        animer("vague", &[("couleur", "ff2080")]),
        animer("comete", &[("couleur", "ff00ff"), ("vitesse", "2")]),
        animer(
            "onde",
            &[
                ("sens", "bas-haut"),
                ("vitesse", "255"),
                ("couleur", "000000"),
            ],
        ),
        animer("arc-en-ciel", &[("cle-inconnue-du-protocole", "valeur")]),
        Request::Animate {
            name: None,
            reglages: Vec::new(),
        },
        geometrie(None, &[]),
        geometrie(Some("radiateur-haut"), &[]),
        geometrie(Some("radiateur-haut"), &[("angle", "0")]),
        geometrie(
            Some("arriere"),
            &[("angle", "359"), ("sens", "antihoraire")],
        ),
    ];
    for position in Position::ALL {
        temoins.push(geometrie(Some(&position.slug()), &[("angle", "180")]));
    }

    for temoin in &temoins {
        let encodee = tient_sur_une_ligne(temoin);
        assert!(!encodee.is_empty(), "{temoin:?} s'encode en rien");
        assert!(
            encodee.len() <= MAX_LINE_LEN,
            "une requête que le démon refuserait à la lecture : {} octets",
            encodee.len()
        );
        assert_eq!(
            parse_request(&encodee).as_ref(),
            Ok(temoin),
            "aller-retour de {temoin:?} par « {encodee} »"
        );
    }
}

// ---------------------------------------------------------------------------
// Aucun encodage ne produit deux lignes physiques
// ---------------------------------------------------------------------------

#[test]
fn aucune_valeur_hostile_ne_scinde_une_requete_en_deux_commandes() {
    // Contrat d'API — « la neutralisation des blancs et caractères de contrôle (#17) s'applique
    // aussi aux nouveaux champs : aucun encodage ne peut produire deux lignes physiques ».
    //
    // Le champ neuf est le plus exposé de tous : une valeur de réglage vient d'une frappe, et
    // souvent d'un script. `reverb animate vague --couleur "$COULEUR"` avec une variable mal
    // remplie suffit. Si le `\n` traverse l'encodage, le démon lit deux lignes et exécute une
    // commande que personne n'a écrite — et comme le socket est en root, cette commande a les
    // droits du démon.
    //
    // Le contrat ne prescrit pas le remède — refus, échappement ou nettoyage à la source sont
    // trois réponses valables — donc le test ne vérifie que l'invariant : une ligne, une seule,
    // et elle se relit comme la même sorte de requête.
    for poison in HOSTILES {
        for requete in [
            animer("vague", &[("couleur", poison)]),
            animer("vague", &[(poison, "ff00ff")]),
            animer(poison, &[]),
            animer("vague", &[("couleur", "ff0000"), ("vitesse", poison)]),
        ] {
            let encodee = tient_sur_une_ligne(&requete);
            assert!(
                matches!(parse_request(&encodee), Ok(Request::Animate { .. })),
                "« {encodee} » ne se relit plus comme une animation — le poison « {poison:?} » a \
                 changé la nature de la requête"
            );
        }

        for requete in [
            geometrie(Some(poison), &[]),
            geometrie(Some("radiateur-haut"), &[("angle", poison)]),
            geometrie(Some("radiateur-haut"), &[(poison, "90")]),
        ] {
            let encodee = tient_sur_une_ligne(&requete);
            assert!(
                matches!(parse_request(&encodee), Ok(Request::Geometry { .. })),
                "« {encodee} » ne se relit plus comme une requête de géométrie — le poison \
                 « {poison:?} » a changé la nature de la requête"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// La ligne de réponse `geom`
// ---------------------------------------------------------------------------

#[test]
fn une_ligne_geom_se_relit_exactement_et_ne_termine_jamais_une_reponse() {
    // Contrat d'API — `Geom { position, angle, sens }` : « `geom <position-slug> <angle> <sens>` »,
    // et la règle de cadrage de #17, que le champ neuf ne doit pas rompre.
    //
    // C'est la réponse à `geometry` : dix lignes puis `end`. Un client qui lirait une onzième
    // ligne prise pour une fin, ou qui s'arrêterait à la troisième, afficherait une géométrie
    // partielle — et la fenêtre proposerait de « corriger » des ventilateurs déjà corrects.
    assert_eq!(
        encode_response_line(&geom("radiateur-haut", 90, "horaire")),
        "geom radiateur-haut 90 horaire",
        "la ligne porte son type en tête, puis les trois champs du contrat"
    );
    assert_eq!(
        parse_response_line("geom radiateur-haut 90 horaire"),
        Ok(geom("radiateur-haut", 90, "horaire"))
    );

    // Les dix positions et tout le tour, exhaustivement : le domaine est petit, et c'est le
    // contenu même de la réponse au verbe `geometry`. L'angle s'écrit en base dix, sans zéro de
    // tête — un `090` ne se relirait pas partout de la même façon.
    for position in Position::ALL {
        for sens in ["horaire", "antihoraire"] {
            for angle in 0u16..=359 {
                let ligne = geom(&position.slug(), angle, sens);
                let encodee = encode_response_line(&ligne);
                assert_eq!(
                    encodee,
                    format!("geom {} {angle} {sens}", position.slug()),
                    "l'écriture d'une ligne geom"
                );
                assert_eq!(
                    parse_response_line(&encodee),
                    Ok(ligne),
                    "aller-retour par « {encodee} »"
                );
            }
        }
    }

    // Une ligne de données, jamais une fin de réponse — y compris quand ses champs portent les
    // mots qui terminent une réponse, ou un saut de ligne.
    ne_termine_jamais(&geom("radiateur-haut", 0, "horaire"));
    for &hostile in HOSTILES {
        ne_termine_jamais(&geom(hostile, 90, "horaire"));

        // Le sens est le **dernier** champ de sa ligne. La règle de #17 y traite les blancs
        // autrement que dans les champs non finaux (« pris jusqu'à la fin ») : un champ final
        // fait uniquement de blancs pose une question que le contrat ne tranche pas, et qui n'a
        // rien à voir avec le cadrage. Ce qui est vérifié ici, c'est le cadrage.
        if !hostile.trim().is_empty() {
            ne_termine_jamais(&geom("radiateur-haut", 90, hostile));
        }
    }

    // Ce qui n'est pas une ligne `geom` bien formée est refusé explicitement, pas complété. Un
    // champ manquant deviné donnerait une orientation inventée, et l'utilisateur corrigerait un
    // ventilateur qui ne l'était pas.
    for ligne in [
        "geom",
        "geom radiateur-haut",
        "geom radiateur-haut 90",
        "geom radiateur-haut horaire 90",
        "geom radiateur-haut 90 horaire bidule",
        "geom radiateur-haut -90 horaire",
        "geom radiateur-haut 70000 horaire",
        "geom radiateur-haut 9.5 horaire",
    ] {
        let erreur =
            parse_response_line(ligne).expect_err("une ligne geom mal formée n'est pas une ligne");
        assert_eq!(erreur.line, ligne, "l'erreur porte la ligne fautive");
        assert!(!erreur.reason.is_empty(), "l'erreur doit dire pourquoi");
        let _: &dyn std::error::Error = &erreur;
    }
}

// ---------------------------------------------------------------------------
// Les deux vocabulaires ne se confondent pas
// ---------------------------------------------------------------------------

#[test]
fn geom_n_est_pas_un_verbe_de_requete_et_geometry_n_est_pas_une_ligne_de_reponse() {
    // `spec_ipc.rs` n° 2 pose la règle : les mots-clés du protocole de **réponse** ne sont pas des
    // verbes de requête, « si l'un d'eux était aussi un verbe de requête, un flux de réponse
    // renvoyé par erreur dans un socket de commande exécuterait quelque chose ». L'extension
    // ajoute un mot de chaque côté — `geometry` en requête, `geom` en réponse — et ils se
    // ressemblent assez pour qu'une confusion soit plausible.
    for verbe in ["geom", "geometr", "geometrie", "GEOMETRY", "Geometry"] {
        for ligne in [verbe.to_owned(), format!("{verbe} radiateur-haut angle=90")] {
            assert_eq!(
                parse_request(&ligne),
                Err(RequestError::UnknownVerb {
                    verb: verbe.to_owned()
                }),
                "« {ligne} » n'est pas une requête"
            );
        }
    }

    // Et le mot-clé du verbe de requête n'est pas un type de ligne de réponse : un client qui
    // recevrait `geometry …` doit s'en plaindre, pas le décoder en géométrie.
    for ligne in ["geometry", "geometry radiateur-haut 90 horaire"] {
        assert!(
            parse_response_line(ligne).is_err(),
            "« {ligne} » n'est pas une ligne de réponse"
        );
    }

    // La ligne `geom` d'une réponse, relue comme une requête, ne doit rien exécuter non plus.
    assert!(
        parse_request("geom radiateur-haut 90 horaire").is_err(),
        "une ligne de réponse renvoyée dans un socket de commande n'exécute rien"
    );
}
