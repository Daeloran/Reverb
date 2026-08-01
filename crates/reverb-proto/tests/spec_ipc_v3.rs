//! Tests d'intention du protocole de la fenêtre (issue #23).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #23 et le contrat déposé dans
//! `crates/reverb-proto/src/ipc.rs` — dont les corps neufs sont tous `todo!("#23")` à l'écriture
//! de ce fichier. Ils prolongent `spec_ipc.rs` (#17) et `spec_ipc_v2.rs` (#19), auxquels **ce
//! fichier ne touche pas** : ce qui y est écrit doit continuer de passer tel quel.
//!
//! ## Ce que la fenêtre ajoute au fil, et ce que ça risque
//!
//! Deux verbes en lecture — `lighting` et `watch` — et trois lignes de réponse : `light`, `anim`,
//! `frame`. Rien qui écrive : la fenêtre demande, elle ne décide pas.
//!
//! Le risque neuf tient en une phrase : **ces lignes portent des couleurs**, et une couleur
//! fautive ne produit aucun message. Un `chan` mal décodé affiche une vitesse absurde qu'on
//! remarque ; un rouge et un bleu permutés donnent un boîtier de la mauvaise couleur, que rien ne
//! signale et que seul l'œil rattrape — quand il regarde sous le bureau, ce que la fenêtre existe
//! précisément pour éviter. C'est la première source d'erreur du projet (les contrôleurs NZXT
//! attendent du GRB, la RAM du RGB), et c'est pourquoi ce fichier vérifie les couleurs
//! **composante par composante** et jamais seulement « la couleur est revenue ».
//!
//! Le second risque est l'ordre. Une image est une **suite** de couleurs : la troisième LED d'un
//! ventilateur n'est pas la sixième. Un encodeur qui trierait, ou un décodeur qui perdrait la
//! dernière, rendrait une image plausible et fausse. Les vecteurs témoins de ce fichier sont donc
//! délibérément **non triés** — une liste croissante laisserait passer exactement cette faute.
//!
//! ## Quatre points que le contrat laisse ouverts, et que ces tests tranchent
//!
//! 1. **Les couleurs s'écrivent en six chiffres hexadécimaux minuscules, sans `#`**, comme dans la
//!    requête `light` que #17 a figée. Une seule écriture par couleur, sinon l'aller-retour n'est
//!    plus exact.
//! 2. **Les lignes `light` et `frame` ont exactement trois champs.** Ni la cible ni la liste de
//!    couleurs ne peuvent porter d'espace ; la règle « dernier champ pris jusqu'au bout de la
//!    ligne » de #17 est réservée aux messages écrits pour un humain (`err`, `unreadable`, le mode
//!    d'un `chan`). L'appliquer ici ferait passer `frame fan:arriere ff2080 bidule` pour une image
//!    valide. Même arbitrage que la ligne `geom` de #19.
//! 3. **Une cible vide est refusée**, alors qu'un champ vide *encodé* devient `_` par la règle de
//!    neutralisation du module. Les deux ne se contredisent pas : on neutralise ce qu'on écrit, on
//!    refuse ce qu'on lit de travers — une ligne `light  ff2080` vient d'un émetteur cassé, pas
//!    d'un nom de cible bizarre.
//! 4. **L'absence d'animation est l'absence de ligne `anim`.** Il n'y a pas de `anim off` dans une
//!    réponse : `off` est un nom réservé de la *requête* `animate`, et le contrat dit que la ligne
//!    `anim` est « absente quand l'éclairage est fixe ». Une `anim` sans réglage reste donc une
//!    animation — celle qui n'accepte aucun paramètre, `arc-en-ciel` par exemple.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! Les tests d'intention 3 et 4 de l'issue — « `watch` diffuse une image par tick tant que le
//! client écoute », « un client qui se déconnecte brutalement n'emporte ni le démon ni
//! l'animation » — parlent d'un démon, d'un abonnement et d'un socket. Rien de tout cela ne vit
//! dans `reverb-proto`, qui est pur et n'ouvre rien. Ils appartiennent à la spécification de
//! `reverb-daemon`. Ce qui en revient ici est la moitié qui est bien du protocole : **deux images
//! qui se suivent se relisent sans décalage**, ce que vérifie le dernier test.
//!
//! Ce fichier ne fige pas non plus le **texte** des refus, seulement qu'un refus nomme la ligne
//! fautive, dise quelque chose, et que deux fautes différentes ne rendent pas la même phrase.
//!
//! Aucun accès matériel, aucune IO, aucun socket : `reverb-proto` est pur, ses tests aussi.

use reverb_proto::ipc::{
    MAX_LINE_LEN, Request, RequestError, ResponseLine, encode_request, encode_response_line,
    parse_request, parse_response_line,
};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// La couleur témoin, reprise de `spec_ipc.rs` : ses trois composantes sont distinctes, donc
/// aucune permutation ne peut passer inaperçue. `ff2080` rendrait `20ff80` en GRB et `8020ff` en
/// BGR — trois textes différents.
const COULEUR_HEX: &str = "ff2080";

/// La couleur de [`COULEUR_HEX`].
fn couleur() -> Rgb {
    Rgb::new(0xff, 0x20, 0x80)
}

/// Onze couleurs témoins, **délibérément non triées**, sans doublon et sans composante commune.
///
/// Onze parce que c'est le plus long vecteur du boîtier — une barrette. Non triées parce qu'un
/// encodeur qui rendrait les couleurs dans l'ordre croissant passerait tous les tests d'une liste
/// déjà croissante, et rendrait pourtant une image fausse.
const TEMOINS: [(u8, u8, u8); LEDS_PER_STICK] = [
    (0xff, 0x20, 0x80),
    (0x12, 0xab, 0x03),
    (0x80, 0x20, 0xff),
    (0x00, 0x00, 0x00),
    (0x7f, 0xfe, 0x01),
    (0xab, 0x03, 0xcd),
    (0x01, 0x02, 0x03),
    (0xff, 0xff, 0xff),
    (0x40, 0x91, 0x22),
    (0x09, 0xd1, 0x5c),
    (0xc3, 0x11, 0x9a),
];

/// La `i`-ième couleur témoin. Distincte pour tout `i` inférieur à quatorze.
fn temoin(i: usize) -> Rgb {
    let (r, g, b) = TEMOINS[i % TEMOINS.len()];
    Rgb::new(r ^ (i / TEMOINS.len()) as u8, g, b)
}

/// Les `n` premières couleurs témoins.
fn couleurs(n: usize) -> Vec<Rgb> {
    (0..n).map(temoin).collect()
}

/// L'écriture d'une couleur sur le fil, telle que ces tests la figent.
fn hex(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

fn lumiere(cible: &str, couleur: Rgb) -> ResponseLine {
    ResponseLine::Light {
        cible: cible.to_owned(),
        couleur,
    }
}

fn animation(nom: &str, reglages: &[(&str, &str)]) -> ResponseLine {
    ResponseLine::Anim {
        nom: nom.to_owned(),
        reglages: reglages
            .iter()
            .map(|(cle, valeur)| ((*cle).to_owned(), (*valeur).to_owned()))
            .collect(),
    }
}

fn image(cible: &str, couleurs: &[Rgb]) -> ResponseLine {
    ResponseLine::Frame {
        cible: cible.to_owned(),
        couleurs: couleurs.to_vec(),
    }
}

/// Les quatorze cibles de l'état d'éclairage : dix ventilateurs, quatre barrettes.
///
/// Recalculées depuis le matériel, pas recopiées — si un ventilateur s'ajoute un jour, c'est le
/// test qui suit le boîtier. Les cibles collectives (`all`, `fans`, `ram`) n'y sont pas : l'état
/// est ce que chaque cible **porte**, pas la commande qui l'y a mise.
fn cibles_du_boitier() -> Vec<String> {
    let mut cibles: Vec<String> = Position::ALL
        .into_iter()
        .map(|position| format!("fan:{}", position.slug()))
        .collect();
    for slot in 0..SLOT_COUNT {
        cibles.push(format!("slot:{slot}"));
    }
    cibles
}

/// Champs **hostiles** : ceux qui, mal encodés, produiraient une ligne qu'un client prendrait pour
/// la fin de la réponse, ou pour deux champs là où il n'y en a qu'un.
///
/// Même esprit que la liste de `spec_ipc_v2.rs`. Ici les noms d'animation viennent d'un catalogue
/// qui est de notre côté, mais les réglages viennent d'une frappe humaine — et une valeur portant
/// une espace scinderait la ligne si rien ne la neutralisait.
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
    "va leur",
    "\t",
    "  ",
    "",
    "a\u{0}b",
];

/// Vérifie qu'une ligne de données ne peut pas se faire prendre pour une fin de réponse.
///
/// Reprise de la règle n° 5 de `spec_ipc.rs`, appliquée aux trois lignes neuves : « une ligne de
/// données ne commence **jamais** par `end` ni par `err` », et elle tient sur une seule ligne
/// physique.
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

/// Vérifie qu'aucun champ ne peut se faire passer pour deux.
///
/// C'est l'invariant de cadrage vu de l'intérieur d'une ligne : le nombre de champs qui sort du
/// décodeur est celui qui est entré dans l'encodeur. Un champ à espace qui se scinderait
/// décalerait tous les suivants, et une image de neuf couleurs se lirait comme une image de huit
/// suivie d'un rebut.
fn ne_scinde_pas_les_champs(donnee: &ResponseLine) {
    ne_termine_jamais(donnee);

    let encodee = encode_response_line(donnee);
    let relue = parse_response_line(&encodee).expect("une ligne encodée se relit");

    let sans_blanc = |champ: &str, quoi: &str| {
        assert!(
            !champ.is_empty(),
            "{quoi} vide disparaîtrait entre deux espaces : « {encodee} »"
        );
        assert!(
            !champ.chars().any(|c| c.is_whitespace() || c.is_control()),
            "{quoi} « {champ} » porte un blanc ou un caractère de contrôle après relecture de \
             « {encodee} » — il se ferait prendre pour deux champs"
        );
    };

    match (donnee, &relue) {
        (ResponseLine::Light { .. }, ResponseLine::Light { cible, couleur }) => {
            sans_blanc(cible, "la cible");
            let ResponseLine::Light { couleur: pose, .. } = donnee else {
                unreachable!()
            };
            assert_eq!(
                couleur, pose,
                "la couleur traverse le champ hostile intacte"
            );
        }
        (
            ResponseLine::Frame {
                couleurs: posees, ..
            },
            ResponseLine::Frame { cible, couleurs },
        ) => {
            sans_blanc(cible, "la cible");
            assert_eq!(
                couleurs, posees,
                "les couleurs d'une image ne dépendent pas de ce que porte sa cible : « {encodee} »"
            );
        }
        (
            ResponseLine::Anim {
                reglages: poses, ..
            },
            ResponseLine::Anim { nom, reglages },
        ) => {
            sans_blanc(nom, "le nom d'animation");
            assert_eq!(
                reglages.len(),
                poses.len(),
                "autant de réglages qu'on en a posés : « {encodee} » rend {reglages:?}"
            );
            for (cle, valeur) in reglages {
                sans_blanc(cle, "une clé de réglage");
                sans_blanc(valeur, "une valeur de réglage");
            }
        }
        _ => panic!("{donnee:?} s'est relue en {relue:?} : le type de ligne a changé"),
    }
}

/// Lit une réponse comme le ferait un client : ligne par ligne, jusqu'à la première ligne
/// terminale, **sans compter les lignes** et sans savoir combien en attendre.
///
/// Rend aussi ce qui reste du flux — c'est ce qui permet de lire deux réponses à la suite sur un
/// même socket, comme un abonné à `watch` en reçoit une par image.
fn lit_une_reponse(flux: &str) -> (Vec<ResponseLine>, ResponseLine, &str) {
    let mut donnees = Vec::new();
    let mut reste = flux;
    while let Some((ligne, suite)) = decoupe(reste) {
        reste = suite;
        let decodee = parse_response_line(ligne).unwrap_or_else(|erreur| {
            panic!("chaque ligne du flux se décode : {erreur}");
        });
        if decodee.is_terminal() {
            return (donnees, decodee, reste);
        }
        donnees.push(decodee);
    }
    panic!("le flux s'est terminé sans ligne terminale : « {flux} »");
}

/// Découpe la première ligne d'un flux, séparateur compris.
fn decoupe(flux: &str) -> Option<(&str, &str)> {
    if flux.is_empty() {
        return None;
    }
    Some(match flux.split_once('\n') {
        Some((ligne, suite)) => (ligne, suite),
        None => (flux, ""),
    })
}

/// Sérialise une réponse complète : les lignes de données, puis la ligne de fin, puis un `\n`.
fn flux(donnees: &[ResponseLine], fin: &ResponseLine) -> String {
    let mut sortie = String::new();
    for ligne in donnees.iter().chain(std::iter::once(fin)) {
        sortie.push_str(&encode_response_line(ligne));
        sortie.push('\n');
    }
    sortie
}

// ---------------------------------------------------------------------------
// 1 — `lighting` et `watch` sont deux verbes qui n'attendent rien
// ---------------------------------------------------------------------------

#[test]
fn lighting_et_watch_sont_des_verbes_sans_argument() {
    // Contrat — `Request::Lighting` : « `lighting` — rend l'état d'éclairage courant, sans rien
    // changer », `Request::Watch` : « `watch` — le démon pousse l'image courante, puis chaque
    // nouvelle ». Aucun des deux n'écrit quoi que ce soit, donc aucun des deux n'a d'argument.
    //
    // Un argument accepté en silence serait le plus vicieux des défauts : `watch tout` marcherait
    // aujourd'hui et voudrait dire autre chose le jour où `watch` prendra un filtre. Refuser
    // maintenant est gratuit ; le rattraper plus tard casse un client.
    assert_eq!(parse_request("lighting"), Ok(Request::Lighting));
    assert_eq!(parse_request("watch"), Ok(Request::Watch));

    // La forme du fil est figée dans les deux sens : sans le réencodage, un encodeur pourrait
    // écrire « light-state » et le décodage seul resterait vert.
    for (ligne, requete) in [("lighting", Request::Lighting), ("watch", Request::Watch)] {
        assert_eq!(
            encode_request(&requete),
            ligne,
            "la forme du fil de {requete:?} ne doit pas dériver"
        );
        assert_eq!(
            parse_request(&encode_request(&requete)),
            Ok(requete.clone()),
            "aller-retour de {requete:?}"
        );
    }

    // Un argument est refusé **en nommant le verbe reçu**, comme `status` l'est déjà. Nommer un
    // autre verbe enverrait chercher une faute là où il n'y en a pas — et c'est exactement ce
    // qu'un copier-coller d'une branche à l'autre produit.
    let refusees = [
        ("lighting tout", "lighting"),
        ("lighting fan:arriere", "lighting"),
        ("lighting ff2080", "lighting"),
        ("watch tout", "watch"),
        ("watch fan:arriere", "watch"),
        ("watch 1", "watch"),
    ];
    for (ligne, verbe) in refusees {
        let erreur = parse_request(ligne).expect_err("un argument de trop est refusé");
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

    // Sensible à la casse, comme tout le reste du protocole (en-tête du module). Une variante de
    // casse est un verbe inconnu, pas un argument mauvais : le client n'a pas mal rempli une
    // commande, il en a inventé une.
    for ligne in ["LIGHTING", "Lighting", "WATCH", "Watch", "lightning"] {
        assert!(
            matches!(parse_request(ligne), Err(RequestError::UnknownVerb { .. })),
            "« {ligne} » n'est pas une commande du protocole"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — le protocole s'étend, il ne casse pas
// ---------------------------------------------------------------------------

#[test]
fn light_seul_reste_refuse_en_nommant_son_verbe() {
    // Test d'intention de #17, repris ici **sans être réécrit** : c'est lui qui interdit de faire
    // de `lighting` un `light` sans argument, et c'est la raison pour laquelle le contrat en fait
    // un verbe à part. Un test d'intention ne se réécrit pas pour arranger un design venu après
    // lui — celui-ci se vérifie encore, et rien de plus.
    let erreur = parse_request("light").expect_err("« light » seul reste une requête tronquée");
    let RequestError::BadArgument { verb, .. } = &erreur else {
        panic!("« light » doit donner un BadArgument, pas {erreur:?}");
    };
    assert_eq!(verb.as_str(), "light");

    // Et les verbes de #17 et #19 traversent toujours, maintenant qu'il y en a deux de plus.
    assert!(parse_request("status").is_ok());
    assert!(parse_request("geometry").is_ok());
    assert!(parse_request("animate vague").is_ok());
    assert!(parse_request(&format!("light all {COULEUR_HEX}")).is_ok());
    assert!(parse_request("fan nzxtsmart2:fan-1 auto").is_ok());
}

// ---------------------------------------------------------------------------
// 3 — les trois lignes neuves font l'aller-retour sans rien perdre
// ---------------------------------------------------------------------------

#[test]
fn les_trois_lignes_neuves_font_l_aller_retour_sans_rien_perdre() {
    // Contrat — `ResponseLine::Light` : « `light <cible> <rrggbb>` », `ResponseLine::Anim` :
    // « `anim <nom> [clé=valeur…]` », `ResponseLine::Frame` : « `frame <cible> <rrggbb>,… ».
    //
    // Test d'intention n° 2 de l'issue : « cette réponse se relit : ce qui en sort remonte le même
    // état qu'on avait posé ». C'est la propriété qui rend l'aperçu vrai — la fenêtre ne voit du
    // boîtier que ce qui traverse ces trois lignes.
    let mut lignes = Vec::new();
    for (i, cible) in cibles_du_boitier().iter().enumerate() {
        lignes.push(lumiere(cible, temoin(i)));
        let combien = if cible.starts_with("slot:") {
            LEDS_PER_STICK
        } else {
            LEDS_PER_FAN as usize
        };
        lignes.push(image(cible, &couleurs(combien)));
    }
    lignes.push(animation("arc-en-ciel", &[]));
    lignes.push(animation(
        "comete",
        &[("couleur", "ff00ff"), ("vitesse", "2")],
    ));
    lignes.push(animation("onde", &[("sens", "bas-haut")]));

    for ligne in &lignes {
        let encodee = encode_response_line(ligne);
        let relue = parse_response_line(&encodee)
            .unwrap_or_else(|erreur| panic!("« {encodee} » doit se relire : {erreur}"));
        assert_eq!(
            &relue, ligne,
            "aller-retour de {ligne:?} par « {encodee} » — rien ne se perd en route"
        );
    }

    // La forme exacte du fil, une fois pour chaque type de ligne. Sans ces trois égalités, un
    // encodeur pourrait écrire `light fan:arriere #FF2080` et tous les allers-retours resteraient
    // verts : c'est le décodeur qui rattraperait sa propre écriture.
    assert_eq!(
        encode_response_line(&lumiere("fan:arriere", couleur())),
        format!("light fan:arriere {COULEUR_HEX}")
    );
    assert_eq!(
        encode_response_line(&animation(
            "comete",
            &[("couleur", "ff00ff"), ("vitesse", "2")]
        )),
        "anim comete couleur=ff00ff vitesse=2"
    );
    assert_eq!(
        encode_response_line(&image("slot:0", &[couleur(), Rgb::new(0x00, 0xff, 0x00)])),
        format!("frame slot:0 {COULEUR_HEX},00ff00"),
        "les couleurs d'une image sont séparées par des virgules, sans espace"
    );

    // L'ordre des réglages est celui qu'on a posé, comme pour `animate` (#19). Deux ordres, deux
    // animations : c'est lui qui décide quelle clé un message d'erreur nommera en premier.
    let dans_l_ordre = animation("comete", &[("couleur", "ff00ff"), ("vitesse", "2")]);
    let permutee = animation("comete", &[("vitesse", "2"), ("couleur", "ff00ff")]);
    assert_ne!(dans_l_ordre, permutee, "deux ordres, deux lignes");
    assert_ne!(
        encode_response_line(&dans_l_ordre),
        encode_response_line(&permutee),
        "l'encodeur ne réordonne pas les réglages"
    );
    assert_eq!(
        parse_response_line(&encode_response_line(&permutee)),
        Ok(permutee)
    );

    // Aucune des lignes neuves ne termine une réponse, quelle que soit sa charge.
    for ligne in &lignes {
        ne_termine_jamais(ligne);
    }
}

// ---------------------------------------------------------------------------
// 4 — une couleur traverse le fil composante par composante
// ---------------------------------------------------------------------------

#[test]
fn une_couleur_traverse_le_fil_composante_par_composante() {
    // Critère d'acceptation de l'issue : « la couleur affichée est celle que le démon envoie
    // réellement au matériel ». Une permutation de composantes ne produit **aucun** message : ni
    // erreur, ni ligne illisible, ni valeur hors bornes. Juste un boîtier de la mauvaise couleur.
    //
    // C'est la première source d'erreur du projet — les contrôleurs NZXT attendent du GRB et la
    // RAM du RGB — et le seul endroit où elle se rattrape à froid est ici. D'où des assertions sur
    // chaque composante prise séparément : `assert_eq!(relue, posee)` passerait encore si
    // l'encodeur **et** le décodeur permutaient tous les deux, ce qui est précisément ce qu'un
    // copier-coller de `to_grb` produirait.
    assert_eq!(
        encode_response_line(&lumiere("fan:arriere", couleur())),
        "light fan:arriere ff2080",
        "rouge d'abord, vert ensuite, bleu enfin — et en minuscules"
    );

    let ResponseLine::Light { couleur: relue, .. } =
        parse_response_line("light fan:arriere ff2080").expect("une couleur bien formée se relit")
    else {
        panic!("« light fan:arriere ff2080 » est une ligne d'éclairage");
    };
    assert_eq!(relue.r, 0xff, "les deux premiers chiffres sont le rouge");
    assert_eq!(relue.g, 0x20, "les deux du milieu sont le vert");
    assert_eq!(relue.b, 0x80, "les deux derniers sont le bleu");

    // Les trois permutations qui ne changent pas la longueur du texte donnent trois couleurs
    // distinctes. Si l'une d'elles se relisait comme `ff2080`, une inversion serait invisible.
    for (texte, attendue) in [
        ("ff2080", Rgb::new(0xff, 0x20, 0x80)),
        ("20ff80", Rgb::new(0x20, 0xff, 0x80)),
        ("8020ff", Rgb::new(0x80, 0x20, 0xff)),
        ("80ff20", Rgb::new(0x80, 0xff, 0x20)),
    ] {
        let ligne = format!("light fan:arriere {texte}");
        assert_eq!(
            parse_response_line(&ligne),
            Ok(lumiere("fan:arriere", attendue)),
            "« {ligne} » ne se lit que d'une façon"
        );
        assert_eq!(
            encode_response_line(&lumiere("fan:arriere", attendue)),
            ligne
        );
    }

    // Et dans une image, où la faute se répéterait cent vingt-quatre fois.
    let posees = couleurs(LEDS_PER_FAN as usize);
    let encodee = encode_response_line(&image("fan:radiateur-haut", &posees));
    let attendue: Vec<String> = posees.iter().map(|c| hex(*c)).collect();
    assert_eq!(
        encodee,
        format!("frame fan:radiateur-haut {}", attendue.join(","))
    );

    let ResponseLine::Frame {
        couleurs: relues, ..
    } = parse_response_line(&encodee).expect("une image bien formée se relit")
    else {
        panic!("« {encodee} » est une ligne d'image");
    };
    for (rang, (relue, posee)) in relues.iter().zip(&posees).enumerate() {
        assert_eq!(relue.r, posee.r, "rouge de la LED {rang}");
        assert_eq!(relue.g, posee.g, "vert de la LED {rang}");
        assert_eq!(relue.b, posee.b, "bleu de la LED {rang}");
    }

    // Les bornes : le noir n'est pas l'absence de couleur, et le blanc ne déborde pas.
    for extreme in [Rgb::new(0, 0, 0), Rgb::new(0xff, 0xff, 0xff)] {
        let ligne = lumiere("slot:3", extreme);
        assert_eq!(
            parse_response_line(&encode_response_line(&ligne)),
            Ok(ligne.clone()),
            "aller-retour de {ligne:?}"
        );
    }
    assert_eq!(
        encode_response_line(&lumiere("slot:3", Rgb::new(0, 0, 0))),
        "light slot:3 000000",
        "une LED éteinte s'écrit en six zéros, pas en champ vide"
    );
}

// ---------------------------------------------------------------------------
// 5 — huit couleurs par ventilateur, onze par barrette, et dans l'ordre
// ---------------------------------------------------------------------------

#[test]
fn une_image_porte_huit_couleurs_par_ventilateur_onze_par_barrette_et_dans_l_ordre() {
    // Critère d'acceptation de l'issue : « chaque ventilateur montre ses **huit** LED, chaque
    // barrette ses **onze** », et contrat — `ResponseLine::Frame` : « huit couleurs pour un
    // ventilateur, onze pour une barrette, dans l'ordre du matériel ».
    //
    // L'ordre est la moitié de l'information : la troisième LED d'un anneau n'est pas la sixième.
    // Une image triée, ou amputée de sa dernière couleur, reste une image plausible — c'est
    // pourquoi les témoins ne sont pas croissants et pourquoi la longueur est vérifiée.
    for cible in cibles_du_boitier() {
        let combien = if cible.starts_with("slot:") {
            LEDS_PER_STICK
        } else {
            LEDS_PER_FAN as usize
        };
        let posees = couleurs(combien);
        let ligne = image(&cible, &posees);
        let encodee = encode_response_line(&ligne);

        // Le compte exact, lu sur le texte : autant de virgules que de couleurs moins une.
        assert_eq!(
            encodee.matches(',').count(),
            combien - 1,
            "« {encodee} » doit porter {combien} couleurs"
        );

        let ResponseLine::Frame {
            cible: relue,
            couleurs: relues,
        } = parse_response_line(&encodee).expect("une image se relit")
        else {
            panic!("« {encodee} » est une ligne d'image");
        };
        assert_eq!(relue, cible, "l'image reste sur sa cible");
        assert_eq!(
            relues, posees,
            "les {combien} couleurs reviennent dans l'ordre où on les a posées"
        );

        // Et la ligne la plus longue du protocole tient dans la limite que le module impose aux
        // requêtes : un client qui partage son tampon ne doit pas s'étrangler sur une barrette.
        assert!(
            encodee.len() <= MAX_LINE_LEN,
            "« {encodee} » fait {} octets, au-delà des {MAX_LINE_LEN} du protocole",
            encodee.len()
        );
    }

    // L'ordre survit, et deux ordres ne sont pas la même image. Sans ces trois refus, un encodeur
    // qui trierait ses couleurs — ou un décodeur qui les rangerait — passerait tout ce qui
    // précède.
    let posees = couleurs(LEDS_PER_FAN as usize);
    let dans_l_ordre = image("fan:arriere", &posees);

    let mut permutees = posees.clone();
    permutees.swap(0, 3);
    let permutee = image("fan:arriere", &permutees);
    assert_ne!(dans_l_ordre, permutee, "deux ordres, deux images");
    assert_ne!(
        encode_response_line(&dans_l_ordre),
        encode_response_line(&permutee),
        "une permutation doit se voir sur le fil"
    );
    assert_eq!(
        parse_response_line(&encode_response_line(&permutee)),
        Ok(permutee),
        "et la permutation traverse telle quelle, elle n'est pas remise dans l'ordre"
    );

    let mut renversees = posees.clone();
    renversees.reverse();
    assert_ne!(
        encode_response_line(&image("fan:arriere", &renversees)),
        encode_response_line(&dans_l_ordre),
        "une image renversée n'est pas la même image"
    );

    let mut triees = posees.clone();
    triees.sort_by_key(|c| (c.r, c.g, c.b));
    assert_ne!(
        triees, posees,
        "les couleurs témoins doivent être en désordre, sinon ce test ne prouve rien"
    );
    assert_ne!(
        encode_response_line(&image("fan:arriere", &triees)),
        encode_response_line(&dans_l_ordre),
        "un encodeur qui trierait rendrait une autre image"
    );

    // Une couleur perdue en route se voit : la longueur est portée par la ligne, pas devinée.
    let amputees = &posees[..posees.len() - 1];
    let tronquee = encode_response_line(&image("fan:arriere", amputees));
    assert_ne!(
        tronquee,
        encode_response_line(&dans_l_ordre),
        "sept couleurs ne s'écrivent pas comme huit"
    );
    let ResponseLine::Frame {
        couleurs: relues, ..
    } = parse_response_line(&tronquee).expect("sept couleurs se relisent aussi")
    else {
        panic!("« {tronquee} » est une ligne d'image");
    };
    assert_eq!(
        relues.len(),
        LEDS_PER_FAN as usize - 1,
        "le décodeur ne complète pas ce qui manque"
    );
}

// ---------------------------------------------------------------------------
// 6 — une ligne malformée est refusée, en disant ce qui cloche
// ---------------------------------------------------------------------------

#[test]
fn une_ligne_de_reponse_malformee_est_refusee_en_disant_ce_qui_cloche() {
    // Test d'intention n° 5 de l'issue : « les refus nomment la faute et ce qui est accepté, comme
    // partout ailleurs ». Contrat — `ResponseError { line, reason }`.
    //
    // Ce qui compte n'est pas le mot employé, mais qu'une ligne fausse soit **refusée** plutôt que
    // devinée. Une image à qui il manque une couleur, complétée en silence par du noir, donnerait
    // une LED éteinte que personne ne saurait expliquer.
    let malformees = [
        "frame fan:arriere",
        "frame fan:arriere ",
        "frame fan:arriere ff2080,zzzzzz",
        "frame fan:arriere ff2080,",
        "frame fan:arriere ,ff2080",
        "frame fan:arriere ff2080,,00ff00",
        "frame fan:arriere ff2080 00ff00",
        "frame  ff2080",
        "frame",
        "light fan:arriere",
        "light fan:arriere zzzzzz",
        "light fan:arriere #ff2080",
        "light fan:arriere ff20",
        "light fan:arriere ff208000",
        "light fan:arriere ff2080 encore",
        "light  ff2080",
        "light",
        "anim",
        "anim vague couleur",
        "anim vague =ff2080",
        "anim vague couleur=",
    ];
    let mut raisons = Vec::new();
    for ligne in malformees {
        let erreur = parse_response_line(ligne)
            .err()
            .unwrap_or_else(|| panic!("« {ligne} » n'est pas une ligne de réponse valide"));
        assert_eq!(
            erreur.line, ligne,
            "le refus porte la ligne fautive, pour qu'on la retrouve dans un journal"
        );
        assert!(
            !erreur.reason.is_empty(),
            "« {ligne} » doit être refusée en disant pourquoi"
        );
        let message = erreur.to_string();
        assert!(
            message.contains(ligne) && message.contains(erreur.reason.as_str()),
            "le Display dit la ligne et la raison : « {message} »"
        );
        let _: &dyn std::error::Error = &erreur;
        raisons.push(erreur.reason);
    }

    // Les refus doivent **distinguer** les fautes. Un décodeur qui rendrait « ligne illisible »
    // pour tout laisserait le diagnostic à faire à la main, alors que la ligne est sous les yeux.
    raisons.sort_unstable();
    raisons.dedup();
    assert!(
        raisons.len() >= 3,
        "vingt et une lignes fautives de quatre familles ne peuvent pas partager une seule \
         explication : {raisons:?}"
    );

    // Et le préfixe reste ce qui décide du type de ligne : rien de neuf ne l'a rendu ambigu.
    for inconnu in [
        "lights fan:arriere ff2080",
        "frames slot:0 ff2080",
        "anima vague",
    ] {
        assert!(
            parse_response_line(inconnu).is_err(),
            "« {inconnu} » n'est pas un préfixe du protocole"
        );
    }
}

// ---------------------------------------------------------------------------
// 7 — une animation sans réglage n'est pas une absence d'animation
// ---------------------------------------------------------------------------

#[test]
fn une_animation_sans_reglage_n_est_pas_une_absence_d_animation() {
    // Contrat — `ResponseLine::Anim` : « absente quand l'éclairage est fixe ». Et critère
    // d'acceptation de l'issue : « `arc-en-ciel` ne propose pas de couleur, parce qu'elle la
    // refuse » — il existe donc des animations qui n'acceptent **aucun** réglage, et leur ligne
    // n'a rien derrière le nom.
    //
    // Confondre les deux se paie en clair : la fenêtre proposerait « démarrer » une animation qui
    // tourne déjà, et le bouton « arrêter » resterait grisé pendant qu'un arc-en-ciel défile.
    assert_eq!(
        parse_response_line("anim arc-en-ciel"),
        Ok(animation("arc-en-ciel", &[])),
        "un nom sans réglage reste une animation"
    );
    assert_eq!(
        encode_response_line(&animation("arc-en-ciel", &[])),
        "anim arc-en-ciel",
        "et se réencode sans espace ni séparateur en trop"
    );

    // La différence se lit sur la réponse entière : une ligne de plus, pas un champ vide.
    let fixe: Vec<ResponseLine> = cibles_du_boitier()
        .iter()
        .enumerate()
        .map(|(i, cible)| lumiere(cible, temoin(i)))
        .collect();
    let mut anime = fixe.clone();
    anime.push(animation("arc-en-ciel", &[]));

    let (sans, _, _) = lit_une_reponse(&flux(&fixe, &ResponseLine::End));
    let (avec, _, _) = lit_une_reponse(&flux(&anime, &ResponseLine::End));

    assert!(
        !sans.iter().any(|l| matches!(l, ResponseLine::Anim { .. })),
        "sans ligne « anim », l'éclairage est fixe : {sans:?}"
    );
    assert_eq!(
        avec.iter()
            .filter(|l| matches!(l, ResponseLine::Anim { .. }))
            .count(),
        1,
        "avec ligne « anim », il y a une animation et une seule"
    );
    assert_eq!(
        avec.len(),
        sans.len() + 1,
        "l'animation est une ligne de plus, pas une modification des quatorze autres"
    );
    assert_eq!(
        &avec[..sans.len()],
        &sans[..],
        "et les quatorze couleurs fixes sont les mêmes des deux côtés — ce sont celles que \
         « animate off » rendrait"
    );
}

// ---------------------------------------------------------------------------
// 8 — aucun champ ne peut se faire passer pour deux
// ---------------------------------------------------------------------------

#[test]
fn aucun_champ_des_lignes_neuves_ne_peut_se_faire_passer_pour_deux() {
    // En-tête du module : « le préfixe de type assure l'invariant pour le début de ligne. Il
    // n'assure rien contre un saut de ligne à l'intérieur d'un champ », d'où `jeton` et `reste`.
    //
    // Les cibles viennent de nous, mais les noms d'animation et surtout leurs **réglages**
    // viennent d'une frappe humaine : `animate comete couleur=$'ff\nend'` depuis un script suffit.
    // Une valeur portant une espace scinderait la ligne, et le client prendrait la moitié de
    // l'image suivante pour la fin de celle-ci — un décalage qui ne se rattrape jamais.
    for hostile in HOSTILES {
        ne_scinde_pas_les_champs(&lumiere(hostile, couleur()));
        ne_scinde_pas_les_champs(&image(hostile, &couleurs(LEDS_PER_FAN as usize)));
        ne_scinde_pas_les_champs(&animation(hostile, &[]));
        ne_scinde_pas_les_champs(&animation("comete", &[(hostile, "ff00ff")]));
        ne_scinde_pas_les_champs(&animation("comete", &[("couleur", hostile)]));
        ne_scinde_pas_les_champs(&animation(
            hostile,
            &[(hostile, hostile), ("vitesse", "2"), (hostile, "3")],
        ));
    }

    // Et le cadrage d'une réponse entière tient même si chaque champ est hostile : un client lit
    // jusqu'à la première ligne terminale, et il ne doit pas la trouver avant la fin.
    let mut lignes = Vec::new();
    for (i, hostile) in HOSTILES.iter().enumerate() {
        lignes.push(lumiere(hostile, temoin(i)));
        lignes.push(image(hostile, &couleurs(LEDS_PER_STICK)));
        lignes.push(animation(hostile, &[(hostile, hostile)]));
    }
    let fil = flux(&lignes, &ResponseLine::End);
    let (donnees, fin, reste) = lit_une_reponse(&fil);
    assert_eq!(fin, ResponseLine::End);
    assert_eq!(
        donnees.len(),
        lignes.len(),
        "aucune ligne perdue ni inventée, quels que soient les champs"
    );
    assert!(reste.is_empty(), "et rien ne traîne après la fin");
}

// ---------------------------------------------------------------------------
// 9 — deux images qui se suivent se relisent sans décalage
// ---------------------------------------------------------------------------

#[test]
fn deux_images_qui_se_suivent_se_relisent_sans_decalage() {
    // Contrat — `Request::Watch` : « quatorze lignes `frame` puis `end`, et ainsi de suite tant
    // que le client écoute ». Le rythme et l'abonnement appartiennent au démon ; ce qui est du
    // protocole, et qui se vérifie ici, est qu'une réponse **se répète** sans que le lecteur
    // dérive.
    //
    // C'est la faute que le cadrage existe pour interdire, et la seule qui empire avec le temps :
    // à vingt et une images par seconde, un décalage d'une ligne devient un aperçu entièrement
    // faux en moins d'une seconde, et rien ne le signale.
    let cibles = cibles_du_boitier();
    assert_eq!(cibles.len(), 14, "le boîtier a quatorze cibles adressables");

    let composer = |decalage: usize| -> Vec<ResponseLine> {
        cibles
            .iter()
            .map(|cible| {
                let combien = if cible.starts_with("slot:") {
                    LEDS_PER_STICK
                } else {
                    LEDS_PER_FAN as usize
                };
                let couleurs: Vec<Rgb> = (0..combien).map(|i| temoin(i + decalage)).collect();
                image(cible, &couleurs)
            })
            .collect()
    };

    let premiere = composer(0);
    let seconde = composer(3);
    assert_ne!(
        premiere, seconde,
        "deux images de suite ne sont pas la même"
    );

    let mut fil = flux(&premiere, &ResponseLine::End);
    fil.push_str(&flux(&seconde, &ResponseLine::End));

    let (lues, fin, reste) = lit_une_reponse(&fil);
    assert_eq!(fin, ResponseLine::End);
    assert_eq!(lues, premiere, "la première image arrive entière");

    let (lues, fin, reste) = lit_une_reponse(reste);
    assert_eq!(fin, ResponseLine::End);
    assert_eq!(
        lues, seconde,
        "et la seconde aussi, sans avoir hérité d'une ligne de la première"
    );
    assert!(reste.is_empty(), "le fil est consommé jusqu'au bout");
}
