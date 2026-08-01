//! Tests d'intention du protocole des zones (issue #29).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #29 et le contrat déposé dans
//! `crates/reverb-proto/src/ipc.rs` et `crates/reverb-proto/src/led.rs` — dont les corps neufs
//! sont tous `todo!("issue #29")` à l'écriture de ce fichier. Ils prolongent `spec_ipc.rs` (#17),
//! `spec_ipc_v2.rs` (#19) et `spec_ipc_v3.rs` (#23), auxquels **ce fichier ne touche pas** : ce qui
//! y est écrit doit continuer de passer tel quel.
//!
//! ## Ce que les zones ajoutent au fil, et ce que ça risque
//!
//! Un verbe à cinq actions — `zone list|set|drop|light|anim` — et trois lignes de réponse —
//! `zone`, `zone-light`, `zone-anim`. Le neuf n'est pas la couleur, que #23 a déjà couverte
//! composante par composante : c'est la **cible LED**, et le fait qu'une même cible s'écrive de
//! deux façons.
//!
//! `fan:arriere` désigne huit LED, `fan:arriere:3` en désigne une. Les deux formes traversent le
//! même champ, et une erreur d'expansion ne produit **aucun** message : une zone qui devait couvrir
//! un ventilateur entier n'en couvre qu'une LED, les sept autres restent sur la couche globale, et
//! rien dans le journal ne le dit. C'est le mode de défaillance central de ce fichier, d'où les
//! comptes exacts partout où une cible s'expanse.
//!
//! Le second risque est le **rang hors bornes**. Un ventilateur a huit LED (`0`–`7`), une barrette
//! onze (`0`–`10`), et il y a quatre barrettes. Un rang de trop rogné en silence sur la dernière
//! LED donnerait une zone plausible et fausse ; un rang de trop qui paniquerait tuerait le démon
//! sur une frappe. Il est donc refusé, en nommant la cible fautive — c'est la seule façon pour
//! l'utilisateur de savoir laquelle de ses vingt cibles il a mal tapée.
//!
//! ## Six points que le contrat laisse ouverts, et que ces tests tranchent
//!
//! 1. **Le protocole transporte la liste de cibles dans l'ordre écrit** — il ne la trie pas, il ne
//!    la regroupe pas. C'est ce qui rend l'aller-retour inconditionnel, comme pour les réglages de
//!    `animate` (#19) : « l'encodeur ne réordonne pas les réglages ». Le tri et la déduplication
//!    appartiennent à `Zones::poser` côté démon, dont le contrat documente des cibles « triées et
//!    sans doublon ».
//! 2. **Une cible collective — `all`, `fans`, `ram` — n'est pas une cible de zone.** Le contrat de
//!    `Led::depuis_slug` énumère ce qu'il accepte : une LED précise, ou un organe entier. Une zone
//!    qui couvre tout le boîtier s'écrit avec les quatorze organes, ce que ce fichier vérifie.
//!    L'alias serait une commodité, mais il ferait de `zone set` la seule commande où `all` veut
//!    dire « cent vingt-quatre LED » plutôt que « les quatorze cibles d'éclairage ».
//! 3. **Un nom de zone est un seul jeton, non vide.** Le contrat écrit `zone set <nom> <cibles>` :
//!    trois champs après `zone`, pas plus. Un nom portant une espace décalerait tout ce qui suit —
//!    `zone drop ma zone` supprimerait une zone nommée `ma` sans un mot. Il est donc refusé plutôt
//!    que tronqué, et la règle « dernier champ pris jusqu'au bout de la ligne » de #17 ne
//!    s'applique pas ici, exactement comme #23 l'a tranché pour `light` et `frame`.
//! 4. **`zone anim <nom> off` n'emporte aucun réglage.** `off` est l'absence d'animation, portée
//!    par `animation: None` ; lui écrire une vitesse n'aurait rien à régler. Ce fichier fige donc
//!    la forme sans réglage, et ne dit rien de `zone anim z off vitesse=3` — voir le rapport.
//! 5. **Une sous-commande inconnue est refusée en nommant `zone`**, pas en nommant la
//!    sous-commande : le verbe reçu est `zone`, il existe, ce sont ses arguments qui sont mauvais.
//!    Même arbitrage que `lighting tout` dans #23.
//! 6. **Une ligne `zone` sans aucune cible est refusée.** Une zone vide ne s'affiche pas et ne se
//!    désigne plus — le contrat du démon la supprime. En laisser paraître une sur le fil ferait
//!    croire à un abonné qu'une zone existe alors qu'elle n'existe plus.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! ⚠️ **Une zone qui couvre les cent vingt-quatre LED ne tient pas dans [`MAX_LINE_LEN`] une fois
//! écrite LED par LED** : son unique ligne `zone` pèse près de 1,9 Kio, contre 1024 octets de
//! limite. Ce fichier vérifie donc que la **requête** qui crée une telle zone tient, elle, dans la
//! limite — parce qu'elle s'écrit avec quatorze organes — et laisse la question de la **réponse**
//! au rapport de spécification : ce n'est pas au test d'intention d'inventer une pagination que
//! l'issue n'a pas demandée.
//!
//! Ce fichier ne fige pas non plus le texte des refus, seulement qu'un refus nomme le verbe ou la
//! cible fautive, dise quelque chose, et que deux fautes différentes ne rendent pas la même phrase.
//!
//! Aucun accès matériel, aucune IO, aucun socket : `reverb-proto` est pur, ses tests aussi.

use reverb_proto::ipc::{
    MAX_LINE_LEN, Request, RequestError, ResponseLine, encode_request, encode_response_line,
    parse_request, parse_response_line,
};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Led, Position, Rgb};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// La couleur témoin, reprise de `spec_ipc.rs` : ses trois composantes sont distinctes, donc aucune
/// permutation ne peut passer inaperçue. `ff2080` rendrait `20ff80` en GRB et `8020ff` en BGR.
const COULEUR_HEX: &str = "ff2080";

/// La couleur de [`COULEUR_HEX`].
fn couleur() -> Rgb {
    Rgb::new(0xff, 0x20, 0x80)
}

/// Le nombre de LED du boîtier : dix anneaux de huit, quatre barrettes de onze.
///
/// Recalculé depuis le matériel plutôt qu'écrit `124` : si une barrette s'ajoute un jour, c'est le
/// test qui suit le boîtier.
const LEDS_DU_BOITIER: usize = 10 * LEDS_PER_FAN as usize + SLOT_COUNT * LEDS_PER_STICK;

/// Les quatorze organes adressables, écrits en forme courte.
///
/// Ce sont les mêmes quatorze cibles que l'état d'éclairage de #23, à ceci près qu'ici chacune
/// **s'expanse** en toutes ses LED.
fn organes() -> Vec<String> {
    let mut cibles: Vec<String> = Position::ALL
        .into_iter()
        .map(|position| format!("fan:{}", position.slug()))
        .collect();
    for slot in 0..SLOT_COUNT {
        cibles.push(format!("slot:{slot}"));
    }
    cibles
}

/// Les huit LED d'un ventilateur, dans l'ordre du matériel.
fn led_du_ventilateur(position: Position) -> Vec<Led> {
    (0..LEDS_PER_FAN as usize)
        .map(|led| Led::Ventilateur { position, led })
        .collect()
}

/// Les onze LED d'une barrette, dans l'ordre du matériel.
fn led_de_la_barrette(slot: usize) -> Vec<Led> {
    (0..LEDS_PER_STICK)
        .map(|led| Led::Barrette { slot, led })
        .collect()
}

/// Une liste de cibles **délibérément en désordre**, sans doublon, prise des deux bus.
///
/// En désordre parce qu'un encodeur ou un décodeur qui trierait passerait tous les tests d'une
/// liste déjà croissante et rendrait pourtant une autre zone — celle où la première LED cliquée
/// n'est plus la première écrite. Des deux bus parce qu'un décodeur qui confondrait `fan:` et
/// `slot:` sur un seul bus n'aurait rien à confondre.
fn cibles_en_desordre() -> Vec<Led> {
    vec![
        Led::Barrette { slot: 2, led: 10 },
        Led::Ventilateur {
            position: Position::Arriere,
            led: 3,
        },
        Led::Barrette { slot: 0, led: 0 },
        Led::Ventilateur {
            position: Position::BasGauche,
            led: 7,
        },
        Led::Ventilateur {
            position: Position::RadiateurMilieu,
            led: 0,
        },
    ]
}

fn zone_set(nom: &str, cibles: &[Led]) -> Request {
    Request::ZoneSet {
        nom: nom.to_owned(),
        cibles: cibles.to_vec(),
    }
}

fn zone_anim(nom: &str, animation: Option<&str>, reglages: &[(&str, &str)]) -> Request {
    Request::ZoneAnim {
        nom: nom.to_owned(),
        animation: animation.map(str::to_owned),
        reglages: paires(reglages),
    }
}

fn ligne_zone(nom: &str, cibles: &[Led]) -> ResponseLine {
    ResponseLine::Zone {
        nom: nom.to_owned(),
        cibles: cibles.to_vec(),
    }
}

fn ligne_zone_light(nom: &str, couleur: Rgb) -> ResponseLine {
    ResponseLine::ZoneLight {
        nom: nom.to_owned(),
        couleur,
    }
}

fn ligne_zone_anim(nom: &str, animation: &str, reglages: &[(&str, &str)]) -> ResponseLine {
    ResponseLine::ZoneAnim {
        nom: nom.to_owned(),
        animation: animation.to_owned(),
        reglages: paires(reglages),
    }
}

fn paires(reglages: &[(&str, &str)]) -> Vec<(String, String)> {
    reglages
        .iter()
        .map(|(cle, valeur)| ((*cle).to_owned(), (*valeur).to_owned()))
        .collect()
}

/// Les cibles d'une requête `zone set`, ou un échec qui dit ce qu'on a reçu à la place.
fn cibles_de(ligne: &str) -> Vec<Led> {
    match parse_request(ligne) {
        Ok(Request::ZoneSet { cibles, .. }) => cibles,
        Ok(autre) => panic!("« {ligne} » devait être un `zone set`, elle a rendu {autre:?}"),
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
                !reason.is_empty(),
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

/// Le refus d'une commande de zone : le verbe nommé doit être `zone`.
fn refus_de_zone(ligne: &str) -> String {
    let (verbe, raison) = refus(ligne);
    assert_eq!(
        verbe, "zone",
        "« {ligne} » : le verbe reçu est `zone`, l'erreur doit le nommer — nommer autre chose \
         envoie chercher une faute là où il n'y en a pas"
    );
    raison
}

/// Vérifie qu'une ligne de données ne peut pas se faire prendre pour une fin de réponse.
///
/// Règle n° 5 de `spec_ipc.rs`, appliquée aux trois lignes neuves : « une ligne de données ne
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
/// Même esprit que les listes de `spec_ipc_v2.rs` et `spec_ipc_v3.rs`. Ici la raison est plus forte
/// encore : un nom de zone vient **entièrement** d'une frappe humaine, et `zone set $'a\nlight all
/// ffffff' fan:arriere` depuis un script suffirait à faire exécuter une commande que personne n'a
/// demandée.
///
/// Le champ **vide** n'y figure pas : il n'a pas de blanc à neutraliser, et ce qu'une requête à nom
/// vide doit devenir sur le fil n'est pas tranché par le contrat. Ce que ce fichier en dit tient
/// dans `un_nom_de_zone_vide_ou_portant_une_espace_est_refuse`, du côté de la lecture — voir le
/// rapport. Il figure en revanche dans les hostiles des **lignes de réponse**, où #23 a déjà
/// tranché qu'un champ vide encodé ne reste pas vide.
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
    "ma zone",
    "\t",
    "  ",
    "a\u{0}b",
    "zone",
    "off",
    "\u{feff}a",
];

// ---------------------------------------------------------------------------
// 1 — `zone list` est une action sans argument
// ---------------------------------------------------------------------------

#[test]
fn zone_list_est_une_action_sans_argument() {
    // Contrat — `Request::ZoneList` : « `zone list` — rend la composition et le rendu de chaque
    // zone ». Elle ne change rien, elle n'a donc rien à recevoir.
    //
    // Un argument accepté en silence serait le plus vicieux des défauts : `zone list mazone`
    // marcherait aujourd'hui et voudrait dire autre chose le jour où `zone list` prendra un filtre.
    assert_eq!(parse_request("zone list"), Ok(Request::ZoneList));
    assert_eq!(
        encode_request(&Request::ZoneList),
        "zone list",
        "la forme du fil de `zone list` ne doit pas dériver — sans cette égalité, un encodeur \
         pourrait écrire `zones` et le décodage seul resterait vert"
    );
    assert_eq!(
        parse_request(&encode_request(&Request::ZoneList)),
        Ok(Request::ZoneList)
    );

    for ligne in ["zone list mazone", "zone list all", "zone list 1"] {
        refus_de_zone(ligne);
    }

    // Sensible à la casse, comme tout le reste du protocole. Une variante de casse est un verbe
    // inconnu, pas un argument mauvais : le client n'a pas mal rempli une commande, il en a
    // inventé une.
    for ligne in ["ZONE list", "Zone list", "zones list"] {
        assert!(
            matches!(parse_request(ligne), Err(RequestError::UnknownVerb { .. })),
            "« {ligne} » n'est pas une commande du protocole"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — les cinq commandes font l'aller-retour sans rien perdre
// ---------------------------------------------------------------------------

#[test]
fn les_cinq_commandes_de_zone_font_l_aller_retour_sans_rien_perdre() {
    // Test d'intention n° 5 de l'issue, côté fil : « l'aller-retour écriture/lecture est exact ».
    // C'est la propriété qui rend la fenêtre et la ligne de commande interchangeables — les deux
    // parlent au démon par ces cinq lignes et rien d'autre.
    let temoins = vec![
        Request::ZoneList,
        zone_set("colonne", &cibles_en_desordre()),
        zone_set("arriere-seul", &led_du_ventilateur(Position::Arriere)),
        zone_set(
            "une-seule-led",
            &[Led::Ventilateur {
                position: Position::HautDroite,
                led: 5,
            }],
        ),
        zone_set("une-barrette", &led_de_la_barrette(3)),
        Request::ZoneDrop {
            nom: "colonne".to_owned(),
        },
        Request::ZoneLight {
            nom: "colonne".to_owned(),
            couleur: couleur(),
        },
        Request::ZoneLight {
            nom: "colonne".to_owned(),
            couleur: Rgb::BLACK,
        },
        zone_anim("colonne", Some("braise"), &[]),
        zone_anim("colonne", Some("comete"), &[("couleur", "ff00ff")]),
        zone_anim(
            "colonne",
            Some("vague"),
            &[("vitesse", "7"), ("direction", "bas-haut")],
        ),
        zone_anim("colonne", None, &[]),
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
    // `zone delete` et tous les allers-retours resteraient verts : c'est le décodeur qui
    // rattraperait sa propre écriture, et la ligne de commande d'un autre client ne marcherait pas.
    assert_eq!(
        encode_request(&Request::ZoneDrop {
            nom: "colonne".to_owned()
        }),
        "zone drop colonne"
    );
    assert_eq!(
        encode_request(&Request::ZoneLight {
            nom: "colonne".to_owned(),
            couleur: couleur()
        }),
        format!("zone light colonne {COULEUR_HEX}"),
        "rouge d'abord, vert ensuite, bleu enfin — et en minuscules, comme partout ailleurs"
    );
    assert_eq!(
        encode_request(&zone_anim("colonne", Some("comete"), &[("vitesse", "2")])),
        "zone anim colonne comete vitesse=2"
    );
    assert_eq!(
        encode_request(&zone_anim("colonne", None, &[])),
        "zone anim colonne off",
        "`off` est le nom réservé qui éteint une couche, ici comme pour `animate`"
    );
    assert_eq!(
        encode_request(&zone_set(
            "colonne",
            &[
                Led::Ventilateur {
                    position: Position::Arriere,
                    led: 3
                },
                Led::Barrette { slot: 1, led: 10 },
            ]
        )),
        "zone set colonne fan:arriere:3,slot:1:10",
        "les cibles sont séparées par des virgules, sans espace — une liste à espaces ferait de \
         chaque cible un champ, et le nom de zone ne serait plus repérable"
    );

    // L'ordre des réglages est celui qu'on a posé, comme pour `animate` (#19). Deux ordres, deux
    // commandes : c'est lui qui décide quelle clé un message d'erreur nommera en premier.
    let dans_l_ordre = zone_anim(
        "z",
        Some("comete"),
        &[("couleur", "ff00ff"), ("vitesse", "2")],
    );
    let permutee = zone_anim(
        "z",
        Some("comete"),
        &[("vitesse", "2"), ("couleur", "ff00ff")],
    );
    assert_ne!(dans_l_ordre, permutee, "deux ordres, deux commandes");
    assert_ne!(
        encode_request(&dans_l_ordre),
        encode_request(&permutee),
        "l'encodeur ne réordonne pas les réglages"
    );
}

// ---------------------------------------------------------------------------
// 3 — un organe entier s'expanse, une LED précise n'en donne qu'une
// ---------------------------------------------------------------------------

#[test]
fn zone_set_expanse_un_organe_entier_et_ne_donne_qu_une_led_pour_un_rang_precis() {
    // Contrat — `Request::ZoneSet` : « une cible est une LED précise (`fan:arriere:3`, `slot:0:7`)
    // ou un organe entier (`fan:arriere`, `slot:0`), qui vaut pour toutes ses LED », et
    // `Led::depuis_slug` : « accepte aussi un organe entier […] auquel cas toutes ses LED sont
    // rendues ».
    //
    // Critère d'acceptation n° 1 de l'issue : « créer une zone à partir de cibles quelconques — LED
    // isolées, ventilateurs entiers, barrettes ». Le mode de défaillance est muet : une zone qui
    // devait couvrir un ventilateur n'en couvre qu'une LED, les sept autres suivent la couche
    // globale, et rien ne le signale.
    assert_eq!(
        cibles_de("zone set z fan:arriere"),
        led_du_ventilateur(Position::Arriere),
        "`fan:arriere` vaut pour les huit LED de l'anneau, dans l'ordre du matériel"
    );
    assert_eq!(
        cibles_de("zone set z fan:arriere:3"),
        vec![Led::Ventilateur {
            position: Position::Arriere,
            led: 3
        }],
        "`fan:arriere:3` n'en désigne qu'une, la quatrième"
    );
    assert_eq!(
        cibles_de("zone set z slot:0"),
        led_de_la_barrette(0),
        "`slot:0` vaut pour les onze LED de la barrette"
    );
    assert_eq!(
        cibles_de("zone set z slot:0:7"),
        vec![Led::Barrette { slot: 0, led: 7 }],
        "`slot:0:7` n'en désigne qu'une"
    );

    // Chaque organe du boîtier s'expanse au bon compte : huit pour un anneau, onze pour une
    // barrette. Une expansion qui rendrait toujours huit LED serait juste sur dix cibles et fausse
    // sur quatre — trois LED de RAM perdues par barrette, sans un message.
    for organe in organes() {
        let attendues = if let Some(slot) = organe.strip_prefix("slot:") {
            led_de_la_barrette(slot.parse().expect("le slot du test est un nombre"))
        } else {
            let slug = organe
                .strip_prefix("fan:")
                .expect("un organe est un `fan:` ou un `slot:`");
            led_du_ventilateur(Position::from_slug(slug).expect("le slug du test est une position"))
        };
        let ligne = format!("zone set z {organe}");
        assert_eq!(
            cibles_de(&ligne),
            attendues,
            "« {ligne} » doit rendre les {} LED de « {organe} », dans l'ordre du matériel",
            attendues.len()
        );
    }

    // Une cible isolée et un organe se mélangent dans la même liste : c'est l'exemple même de
    // l'issue, « ventilateur arrière + bas-milieu + haut-milieu », auquel on ajoute une LED seule.
    let melange = cibles_de("zone set z fan:arriere,fan:bas-milieu:2,slot:3");
    let mut attendues = led_du_ventilateur(Position::Arriere);
    attendues.push(Led::Ventilateur {
        position: Position::BasMilieu,
        led: 2,
    });
    attendues.extend(led_de_la_barrette(3));
    assert_eq!(
        melange,
        attendues,
        "les trois formes se mélangent dans une seule liste, et chacune s'expanse à sa place : \
         {} LED attendues",
        attendues.len()
    );

    // Et un aller-retour par la forme longue rend la même zone : peu importe comment
    // l'utilisateur l'a écrite, ce qui compte est l'ensemble de LED qui en sort.
    let requete = zone_set("z", &melange);
    assert_eq!(
        parse_request(&encode_request(&requete)),
        Ok(requete.clone()),
        "aller-retour d'une zone née d'un mélange de formes"
    );
}

// ---------------------------------------------------------------------------
// 4 — un rang hors bornes est refusé, en le nommant
// ---------------------------------------------------------------------------

#[test]
fn un_rang_hors_bornes_est_refuse_en_nommant_la_cible() {
    // Contrat — `Led::depuis_slug` : « l'inverse de `Led::slug`, **strict** », et `LedInconnue
    // { donne, raison }`. Le boîtier a huit LED par ventilateur (`0`–`7`), onze par barrette
    // (`0`–`10`), quatre barrettes (`0`–`3`).
    //
    // Deux façons de mal faire, aussi coûteuses l'une que l'autre : rogner `fan:arriere:8` sur la
    // huitième LED donne une zone fausse dont rien ne dit qu'elle l'est ; indexer sans borner fait
    // paniquer le démon sur une faute de frappe, et emporte l'éclairage de toute la machine.
    //
    // Nommer la cible fautive n'est pas du confort : une commande porte jusqu'à quatorze cibles, et
    // « cible invalide » sans plus laisse l'utilisateur les relire une par une.
    let hors_bornes = [
        "fan:arriere:8",
        "fan:arriere:9",
        "fan:arriere:255",
        "slot:0:11",
        "slot:0:12",
        "slot:4:0",
        "slot:4",
        "slot:255:0",
        "fan:pas-un-ventilateur",
        "fan:pas-un-ventilateur:0",
        "fan:arriere:",
        "fan:arriere:-1",
        "fan:arriere:x",
        "fan:",
        "slot:",
        "slot:a",
        "fan:arriere:0:0",
        "arriere:0",
        "led:0",
        "all",
        "fans",
        "ram",
    ];

    let mut raisons = Vec::new();
    for brut in hors_bornes {
        // 1 — la lecture d'une cible seule, là où la faute se constate.
        let erreur = Led::depuis_slug(brut)
            .err()
            .unwrap_or_else(|| panic!("« {brut} » n'est pas une cible LED valide"));
        assert_eq!(
            erreur.donne, brut,
            "le refus porte la cible fautive telle qu'elle a été écrite, pour qu'on la retrouve \
             dans la commande"
        );
        assert!(
            !erreur.raison.trim().is_empty(),
            "« {brut} » doit être refusée en disant pourquoi"
        );
        let message = erreur.to_string();
        assert!(
            message.contains(brut),
            "le Display nomme la cible : « {message} »"
        );
        let _: &dyn std::error::Error = &erreur;
        raisons.push(erreur.raison);

        // 2 — la même faute vue du protocole, où l'utilisateur la commet vraiment.
        let ligne = format!("zone set z {brut}");
        let raison = refus_de_zone(&ligne);
        assert!(
            raison.contains(brut),
            "« {ligne} » doit être refusée en nommant « {brut} » : une commande porte jusqu'à \
             quatorze cibles, et il faut savoir laquelle est fautive. Raison obtenue : {raison}"
        );

        // 3 — et noyée au milieu de cibles valides, ce qui est le cas réel.
        let ligne = format!("zone set z fan:arriere:0,{brut},slot:1");
        let raison = refus_de_zone(&ligne);
        assert!(
            raison.contains(brut),
            "une cible fautive au milieu de cibles valides doit être nommée, pas avalée. Raison \
             obtenue : {raison}"
        );
    }

    // Les refus doivent **distinguer** les fautes : un rang trop grand, une position qui n'existe
    // pas et un préfixe inconnu ne se corrigent pas de la même façon.
    raisons.sort_unstable();
    raisons.dedup();
    assert!(
        raisons.len() >= 3,
        "vingt-deux cibles fautives de plusieurs familles ne peuvent pas partager une seule \
         explication : {raisons:?}"
    );

    // Les bornes elles-mêmes passent : la dernière LED de chaque organe existe bel et bien, et une
    // borne posée à `< n - 1` la perdrait sans un message.
    for brut in [
        "fan:arriere:0",
        "fan:arriere:7",
        "slot:0:0",
        "slot:0:10",
        "slot:3:10",
    ] {
        assert_eq!(
            Led::depuis_slug(brut)
                .unwrap_or_else(|erreur| panic!("« {brut} » est une cible valide : {erreur}"))
                .len(),
            1,
            "« {brut} » désigne exactement une LED"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — une cible vide, un nom vide, un nom à espace
// ---------------------------------------------------------------------------

#[test]
fn un_nom_de_zone_vide_ou_portant_une_espace_est_refuse() {
    // Point n° 3 du préambule. Un nom est un seul jeton, non vide : le contrat écrit `zone set
    // <nom> <cibles>`, et la règle « dernier champ pris jusqu'au bout de la ligne » de #17 est
    // réservée aux messages écrits pour un humain.
    //
    // Le mode de défaillance est silencieux et destructeur : `zone drop ma zone` qui tronquerait
    // supprimerait une zone nommée `ma` — pas celle qu'on visait, et sans un mot.
    for ligne in [
        // Nom manquant : le champ n'est pas là du tout.
        "zone set",
        "zone set fan:arriere",
        "zone drop",
        "zone light",
        "zone light ff2080",
        "zone anim",
        "zone anim braise",
        // Un jeton de trop, c'est-à-dire un nom qui portait une espace.
        "zone drop ma zone",
        "zone light ma zone ff2080",
        "zone set ma zone fan:arriere",
        // Et un argument de trop derrière une commande complète.
        "zone drop colonne encore",
        "zone light colonne ff2080 encore",
    ] {
        refus_de_zone(ligne);
    }

    // Une liste de cibles vide est refusée : une zone sans LED ne s'affiche pas et ne se désigne
    // plus. La créer ferait paraître dans `zone list` une zone que le démon supprime aussitôt.
    for ligne in [
        "zone set colonne",
        "zone set colonne ,",
        "zone set colonne fan:arriere,",
        "zone set colonne ,fan:arriere",
        "zone set colonne fan:arriere,,slot:0",
    ] {
        refus_de_zone(ligne);
    }

    // Un nom hostile ne peut ni scinder la commande en deux, ni la changer de nature. Le
    // protocole neutralise les blancs et les caractères de contrôle (#17) : `zone set $'a\nlight
    // all ffffff' fan:arriere` depuis un script ne doit pas allumer le boîtier en blanc.
    for hostile in HOSTILES {
        for requete in [
            zone_set(hostile, &cibles_en_desordre()),
            Request::ZoneDrop {
                nom: (*hostile).to_owned(),
            },
            Request::ZoneLight {
                nom: (*hostile).to_owned(),
                couleur: couleur(),
            },
            zone_anim(hostile, Some("braise"), &[]),
            zone_anim(hostile, None, &[]),
            zone_anim("colonne", Some(hostile), &[]),
            zone_anim("colonne", Some("comete"), &[(hostile, "2")]),
            zone_anim("colonne", Some("comete"), &[("vitesse", hostile)]),
        ] {
            let encodee = encode_request(&requete);
            assert_eq!(
                encodee.lines().count(),
                1,
                "une requête tient sur une seule ligne, quel que soit son nom de zone : \
                 « {encodee} »"
            );
            assert!(
                !encodee.contains('\n') && !encodee.contains('\r'),
                "aucun saut de ligne dans une requête encodée : « {encodee} »"
            );

            let relue = parse_request(&encodee).unwrap_or_else(|erreur| {
                panic!("« {encodee} » doit rester une commande lisible : {erreur}")
            });
            assert_eq!(
                std::mem::discriminant(&relue),
                std::mem::discriminant(&requete),
                "« {encodee} » ne se relit plus comme {requete:?} — le nom hostile a changé la \
                 nature de la commande"
            );

            // Et ce qui n'était pas hostile traverse intact : neutraliser un nom ne doit pas
            // emporter les cibles ni les réglages avec lui.
            match (&requete, &relue) {
                (
                    Request::ZoneSet { cibles: posees, .. },
                    Request::ZoneSet { cibles: relues, .. },
                ) => assert_eq!(
                    relues, posees,
                    "les cibles traversent le nom hostile intactes"
                ),
                (
                    Request::ZoneLight { couleur: posee, .. },
                    Request::ZoneLight { couleur: relue, .. },
                ) => assert_eq!(relue, posee, "la couleur traverse le nom hostile intacte"),
                (
                    Request::ZoneAnim {
                        reglages: poses, ..
                    },
                    Request::ZoneAnim {
                        reglages: relus, ..
                    },
                ) => assert_eq!(
                    relus.len(),
                    poses.len(),
                    "autant de réglages qu'on en a posés : « {encodee} » rend {relus:?}"
                ),
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 6 — `zone anim <nom> off` est l'absence d'animation
// ---------------------------------------------------------------------------

#[test]
fn zone_anim_off_est_l_absence_d_animation_et_non_une_animation_nommee_off() {
    // Contrat — `Request::ZoneAnim` : « `off` la rend transparente : ses LED reprennent la couche
    // globale », et le module : « `off` est un nom réservé qui arrête l'animation en cours ».
    //
    // Confondre les deux se paie en clair : une zone qui porterait une animation appelée `off`
    // resterait à masquer la couche globale pendant que l'utilisateur croit l'avoir rendue
    // transparente. Et le catalogue ne contient pas `off`, donc la commande échouerait plus loin,
    // au moment de résoudre le nom — un refus qui ne dirait rien de la vraie cause.
    assert_eq!(
        parse_request("zone anim colonne off"),
        Ok(zone_anim("colonne", None, &[])),
        "`off` ne se lit que d'une façon : l'absence d'animation, portée par `animation: None`"
    );
    assert_eq!(
        encode_request(&zone_anim("colonne", None, &[])),
        "zone anim colonne off",
        "et se réécrit `off`, sans champ vide ni jeton en trop"
    );

    // Une animation nommée reste une animation, même sans réglage : `arc-en-ciel` génère ses
    // teintes et n'accepte pas de couleur, sa ligne n'a donc rien derrière le nom.
    assert_eq!(
        parse_request("zone anim colonne arc-en-ciel"),
        Ok(zone_anim("colonne", Some("arc-en-ciel"), &[])),
        "un nom sans réglage reste une animation"
    );
    assert_ne!(
        parse_request("zone anim colonne arc-en-ciel"),
        parse_request("zone anim colonne off"),
        "une animation sans réglage n'est pas une absence d'animation"
    );

    // Le protocole **transporte** les réglages sans les interpréter : il ne connaît pas le
    // catalogue et ne peut pas savoir qu'une clé est refusée. C'est l'arbitrage de #19, et il vaut
    // ici aussi — sinon `reverb-proto` devrait dépendre de `reverb-anim`.
    assert_eq!(
        parse_request("zone anim colonne pas-au-catalogue cle-inconnue=valeur"),
        Ok(zone_anim(
            "colonne",
            Some("pas-au-catalogue"),
            &[("cle-inconnue", "valeur")]
        )),
        "le protocole transporte, le moteur d'animation interprète"
    );

    // Mais une paire malformée reste refusée, comme pour `animate` (#19) : c'est le seul contrôle
    // possible sans connaître les animations, et il attrape la faute de frappe la plus courante —
    // l'espace au lieu du signe égal.
    for ligne in [
        "zone anim colonne comete couleur",
        "zone anim colonne comete couleur ff00ff",
        "zone anim colonne comete =ff00ff",
        "zone anim colonne comete couleur=",
        "zone anim colonne comete =",
    ] {
        refus_de_zone(ligne);
    }
}

// ---------------------------------------------------------------------------
// 7 — une sous-commande inconnue, et `zone` tout seul
// ---------------------------------------------------------------------------

#[test]
fn une_sous_commande_inconnue_est_refusee_en_nommant_zone() {
    // Point n° 5 du préambule. Le verbe reçu est `zone` : il existe, ce sont ses arguments qui sont
    // mauvais. Nommer `toto` comme verbe enverrait l'utilisateur chercher une commande qu'il n'a
    // pas tapée, et un client qui filtre les `UnknownVerb` pour proposer une correction
    // proposerait de corriger `toto` au lieu de `set`.
    let mut raisons = Vec::new();
    for ligne in [
        "zone toto",
        "zone toto colonne",
        "zone delete colonne",
        "zone add colonne fan:arriere",
        "zone LIST",
        "zone Set colonne fan:arriere",
        "zone anima colonne braise",
    ] {
        raisons.push(refus_de_zone(ligne));
    }

    // `zone` seul n'est pas `zone list` : le verbe attend une action, et en deviner une ferait
    // d'une commande tronquée une commande réussie. Même arbitrage que `light` seul dans #17.
    refus_de_zone("zone");
    assert_ne!(
        parse_request("zone"),
        Ok(Request::ZoneList),
        "`zone` seul ne doit pas se lire comme `zone list` : deviner une action fait d'une frappe \
         interrompue une commande exécutée"
    );

    // Les refus distinguent l'action inconnue du reste : « arguments mauvais » pour tout laisse le
    // diagnostic à faire à la main alors que la ligne est sous les yeux.
    raisons.sort_unstable();
    raisons.dedup();
    assert!(
        !raisons.is_empty(),
        "un refus doit dire quelque chose : {raisons:?}"
    );

    // Et les préfixes voisins ne sont pas le verbe `zone` : ce sont des lignes de **réponse**, qui
    // n'ont rien à faire dans une requête.
    for ligne in [
        "zone-light colonne ff2080",
        "zone-anim colonne braise",
        "zones list",
    ] {
        assert!(
            matches!(parse_request(ligne), Err(RequestError::UnknownVerb { .. })),
            "« {ligne} » n'est pas un verbe de requête"
        );
    }
}

// ---------------------------------------------------------------------------
// 8 — les trois lignes de réponse font l'aller-retour
// ---------------------------------------------------------------------------

#[test]
fn les_trois_lignes_de_zone_font_l_aller_retour_sans_rien_perdre() {
    // Contrat — `ResponseLine::Zone` : « `zone <nom> <cible>,<cible>,…` », `ResponseLine::ZoneLight`
    // : « `zone-light <nom> <rrggbb>` », `ResponseLine::ZoneAnim` : « `zone-anim <nom> <animation>
    // [clé=valeur…]` ».
    //
    // C'est par ces trois lignes que la fenêtre apprend qu'une zone existe. Une composition perdue
    // en route, et l'utilisateur voit une zone qu'il ne peut plus désigner.
    let lignes = vec![
        ligne_zone("colonne", &cibles_en_desordre()),
        ligne_zone("arriere-seul", &led_du_ventilateur(Position::Arriere)),
        ligne_zone("une-barrette", &led_de_la_barrette(2)),
        ligne_zone("une-seule-led", &[Led::Barrette { slot: 3, led: 10 }]),
        ligne_zone_light("colonne", couleur()),
        ligne_zone_light("colonne", Rgb::BLACK),
        ligne_zone_light("colonne", Rgb::new(0xff, 0xff, 0xff)),
        ligne_zone_anim("colonne", "arc-en-ciel", &[]),
        ligne_zone_anim(
            "colonne",
            "comete",
            &[("couleur", "ff00ff"), ("vitesse", "2")],
        ),
        ligne_zone_anim("colonne", "vague", &[("direction", "bas-haut")]),
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

    // La forme exacte du fil, une fois par type de ligne.
    assert_eq!(
        encode_response_line(&ligne_zone_light("colonne", couleur())),
        format!("zone-light colonne {COULEUR_HEX}")
    );
    assert_eq!(
        encode_response_line(&ligne_zone_anim("colonne", "comete", &[("vitesse", "2")])),
        "zone-anim colonne comete vitesse=2"
    );
    assert_eq!(
        encode_response_line(&ligne_zone(
            "colonne",
            &[
                Led::Ventilateur {
                    position: Position::Arriere,
                    led: 3
                },
                Led::Barrette { slot: 1, led: 10 },
            ]
        )),
        "zone colonne fan:arriere:3,slot:1:10"
    );

    // Une couleur de zone traverse le fil composante par composante. La faute est muette — le
    // projet mêle trois ordres de composantes — et une zone est précisément ce qu'on crée pour
    // tenir une couleur choisie à la main.
    let ResponseLine::ZoneLight { couleur: relue, .. } =
        parse_response_line(&format!("zone-light colonne {COULEUR_HEX}"))
            .expect("une couleur bien formée se relit")
    else {
        panic!("« zone-light colonne {COULEUR_HEX} » est une ligne de couleur de zone");
    };
    assert_eq!(relue.r, 0xff, "les deux premiers chiffres sont le rouge");
    assert_eq!(relue.g, 0x20, "les deux du milieu sont le vert");
    assert_eq!(relue.b, 0x80, "les deux derniers sont le bleu");

    // Les préfixes restent ce qui décide du type de ligne, et `zone` n'est pas `zone-light`.
    for inconnue in [
        "zones colonne fan:arriere:0",
        "zone-lights colonne ff2080",
        "zone-anims colonne braise",
        "zone list",
        "zone colonne",
        "zone colonne ",
        "zone  fan:arriere:0",
        "zone colonne fan:arriere:0 slot:0:0",
        "zone colonne fan:arriere:8",
        "zone colonne fan:arriere:0,",
        "zone-light colonne",
        "zone-light colonne zzzzzz",
        "zone-light colonne #ff2080",
        "zone-light colonne ff2080 encore",
        "zone-anim",
        "zone-anim colonne comete couleur",
        "zone-anim colonne comete couleur=",
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

    // `zone list` est une requête, pas une ligne de réponse ; `zone-light` une ligne de réponse,
    // pas une requête. Les deux espaces de noms partagent le mot `zone` et ne doivent pas se
    // mélanger.
    assert!(
        parse_response_line("zone list").is_err(),
        "« zone list » est une requête : la lire comme une zone nommée « list » sans cible ferait \
         apparaître une zone fantôme dans la fenêtre"
    );
}

// ---------------------------------------------------------------------------
// 9 — une ligne `zone` est écrite LED par LED
// ---------------------------------------------------------------------------

#[test]
fn les_cibles_d_une_ligne_zone_sont_rendues_led_par_led_jamais_regroupees() {
    // Contrat — `ResponseLine::Zone` : « les cibles sont toujours rendues **LED par LED**, jamais
    // regroupées en organes : une zone peut n'en tenir que la moitié, et rendre `fan:arriere` pour
    // six LED sur huit serait faux ».
    //
    // La faute est asymétrique et c'est ce qui la rend dangereuse : regrouper huit LED en
    // `fan:arriere` est exact, donc un regroupement passe l'aller-retour sans broncher. Il ne
    // devient faux que le jour où la zone n'en tient que sept — et ce jour-là la fenêtre affiche
    // une LED de plus que le boîtier, sans un message.
    let entier = ligne_zone("anneau", &led_du_ventilateur(Position::Arriere));
    let attendue: Vec<String> = (0..LEDS_PER_FAN as usize)
        .map(|led| format!("fan:arriere:{led}"))
        .collect();
    assert_eq!(
        encode_response_line(&entier),
        format!("zone anneau {}", attendue.join(",")),
        "un ventilateur entier s'écrit quand même en huit cibles"
    );

    let barrette = ligne_zone("barrette", &led_de_la_barrette(1));
    let attendue: Vec<String> = (0..LEDS_PER_STICK)
        .map(|led| format!("slot:1:{led}"))
        .collect();
    assert_eq!(
        encode_response_line(&barrette),
        format!("zone barrette {}", attendue.join(",")),
        "une barrette entière s'écrit quand même en onze cibles"
    );

    // Et le compte se lit sur le texte : autant de virgules que de cibles moins une.
    for (ligne, combien) in [
        (&entier, LEDS_PER_FAN as usize),
        (&barrette, LEDS_PER_STICK),
    ] {
        let encodee = encode_response_line(ligne);
        assert_eq!(
            encodee.matches(',').count(),
            combien - 1,
            "« {encodee} » doit porter {combien} cibles"
        );
    }

    // L'ordre survit : deux ordres ne sont pas la même zone tant qu'elle voyage sur le fil. C'est
    // le démon qui trie, pas le protocole — et un protocole qui trierait masquerait un démon qui
    // ne trie pas.
    let posees = cibles_en_desordre();
    let mut triees = posees.clone();
    triees.sort_unstable();
    assert_ne!(
        triees, posees,
        "les cibles témoins doivent être en désordre, sinon ce test ne prouve rien"
    );
    assert_ne!(
        encode_response_line(&ligne_zone("z", &triees)),
        encode_response_line(&ligne_zone("z", &posees)),
        "un encodeur qui trierait rendrait une autre ligne"
    );
    assert_eq!(
        parse_response_line(&encode_response_line(&ligne_zone("z", &posees))),
        Ok(ligne_zone("z", &posees)),
        "et le désordre traverse tel quel, il n'est pas remis dans l'ordre"
    );

    // Une cible perdue en route se voit : la longueur est portée par la ligne, pas devinée.
    let amputees = &posees[..posees.len() - 1];
    assert_ne!(
        encode_response_line(&ligne_zone("z", amputees)),
        encode_response_line(&ligne_zone("z", &posees)),
        "quatre cibles ne s'écrivent pas comme cinq"
    );
}

// ---------------------------------------------------------------------------
// 10 — le protocole s'étend, il ne casse pas
// ---------------------------------------------------------------------------

#[test]
fn les_verbes_et_les_lignes_d_avant_les_zones_traversent_toujours() {
    // Un verbe de plus ne doit rien coûter à ceux d'avant. `zone` partage son préfixe avec rien,
    // mais `zone-light` et `light` partagent le mot `light`, et un découpage trop souple les
    // confondrait — la fenêtre afficherait la couleur d'une zone comme celle du boîtier entier.
    assert!(parse_request("status").is_ok());
    assert!(parse_request("geometry").is_ok());
    assert!(parse_request("lighting").is_ok());
    assert!(parse_request("watch").is_ok());
    assert!(parse_request("animate vague").is_ok());
    assert!(parse_request(&format!("light all {COULEUR_HEX}")).is_ok());
    assert!(parse_request("fan nzxtsmart2:fan-1 auto").is_ok());

    assert_eq!(
        parse_response_line(&format!("light fan:arriere {COULEUR_HEX}")),
        Ok(ResponseLine::Light {
            cible: "fan:arriere".to_owned(),
            couleur: couleur()
        }),
        "la ligne `light` de #23 reste ce qu'elle était : c'est la couche globale, pas une zone"
    );
    assert_eq!(
        parse_response_line("anim arc-en-ciel"),
        Ok(ResponseLine::Anim {
            nom: "arc-en-ciel".to_owned(),
            reglages: Vec::new()
        }),
        "et la ligne `anim` aussi"
    );

    // `light` seul reste refusé en nommant son verbe — test d'intention de #17, repris ici sans
    // être réécrit. Un test d'intention ne se réécrit pas pour arranger un design venu après lui.
    let (verbe, _) = refus("light");
    assert_eq!(verbe, "light");
}

// ---------------------------------------------------------------------------
// 11 — une zone qui couvre tout le boîtier tient dans une requête
// ---------------------------------------------------------------------------

#[test]
fn une_zone_qui_couvre_tout_le_boitier_s_ecrit_avec_les_quatorze_organes() {
    // Test d'intention n° 8 de l'issue : « une zone qui couvre tout le boîtier masque entièrement
    // la couche globale ». Encore faut-il pouvoir la créer — et c'est là que la forme courte gagne
    // sa place : quatorze organes tiennent dans la ligne, cent vingt-quatre LED n'y tiendraient
    // pas.
    //
    // ⚠️ Le corollaire est une **tension du contrat**, laissée au rapport de spécification : la
    // ligne de réponse `zone` de cette même zone, écrite LED par LED comme son contrat l'exige,
    // dépasse largement `MAX_LINE_LEN`. Ce test ne la fige pas — il vérifie seulement que la
    // requête, elle, passe.
    let ligne = format!("zone set tout {}", organes().join(","));
    assert!(
        ligne.len() <= MAX_LINE_LEN,
        "« {ligne} » fait {} octets, au-delà des {MAX_LINE_LEN} du protocole — une zone couvrant \
         le boîtier deviendrait impossible à créer en une commande",
        ligne.len()
    );

    let cibles = cibles_de(&ligne);
    assert_eq!(
        cibles.len(),
        LEDS_DU_BOITIER,
        "les quatorze organes s'expansent en les {LEDS_DU_BOITIER} LED du boîtier"
    );

    let mut uniques = cibles.clone();
    uniques.sort_unstable();
    uniques.dedup();
    assert_eq!(
        uniques.len(),
        LEDS_DU_BOITIER,
        "et chacune une seule fois : une LED comptée deux fois masquerait un organe oublié"
    );

    // Chaque LED du boîtier y est, nommément. Un décodeur qui oublierait la dernière barrette
    // rendrait 113 cibles — plausible, et faux de onze LED.
    for position in Position::ALL {
        for led in led_du_ventilateur(position) {
            assert!(
                cibles.contains(&led),
                "{led:?} manque à la zone qui couvre tout le boîtier"
            );
        }
    }
    for slot in 0..SLOT_COUNT {
        for led in led_de_la_barrette(slot) {
            assert!(
                cibles.contains(&led),
                "{led:?} manque à la zone qui couvre tout le boîtier"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 12 — l'ordre écrit est l'ordre transporté
// ---------------------------------------------------------------------------

#[test]
fn le_protocole_transporte_les_cibles_dans_l_ordre_ecrit() {
    // Point n° 1 du préambule, isolé dans son propre test pour qu'un échec pointe cette décision
    // et pas une autre.
    //
    // Le tri appartient à `Zones::poser`, dont le contrat documente des cibles « triées et sans
    // doublon ». Trier ici aussi ferait deux endroits où la même règle vit, et le jour où l'un des
    // deux change, `encode(parse(x)) != x` sans que rien ne le dise.
    let ecrites = "zone set z slot:2:10,fan:arriere:3,slot:0:0";
    assert_eq!(
        cibles_de(ecrites),
        vec![
            Led::Barrette { slot: 2, led: 10 },
            Led::Ventilateur {
                position: Position::Arriere,
                led: 3
            },
            Led::Barrette { slot: 0, led: 0 },
        ],
        "« {ecrites} » se lit dans l'ordre écrit — ni trié, ni regroupé par bus"
    );

    // Et l'aller-retour d'une requête en désordre reste exact, ce qui est la raison d'être de
    // cette décision.
    let requete = zone_set("z", &cibles_en_desordre());
    let encodee = encode_request(&requete);
    assert_eq!(
        parse_request(&encodee),
        Ok(requete.clone()),
        "aller-retour d'une liste en désordre par « {encodee} »"
    );
}

// ---------------------------------------------------------------------------
// 13 — aucun champ des trois lignes neuves ne peut se faire passer pour deux
// ---------------------------------------------------------------------------

#[test]
fn aucun_champ_des_lignes_de_zone_ne_peut_se_faire_passer_pour_deux() {
    // En-tête du module `ipc` : « le préfixe de type assure l'invariant pour le début de ligne. Il
    // n'assure rien contre un saut de ligne à l'intérieur d'un champ. »
    //
    // Un nom de zone vient d'une frappe humaine, et une valeur de réglage aussi. Un champ portant
    // une espace scinderait la ligne, et un abonné à `watch` prendrait la moitié de la réponse
    // suivante pour la fin de celle-ci — un décalage qui ne se rattrape jamais.
    let mut hostiles: Vec<&str> = HOSTILES.to_vec();
    hostiles.push("");

    for hostile in hostiles {
        for ligne in [
            ligne_zone(hostile, &cibles_en_desordre()),
            ligne_zone_light(hostile, couleur()),
            ligne_zone_anim(hostile, "braise", &[]),
            ligne_zone_anim("colonne", hostile, &[]),
            ligne_zone_anim("colonne", "comete", &[(hostile, "2")]),
            ligne_zone_anim("colonne", "comete", &[("vitesse", hostile)]),
        ] {
            ne_termine_jamais(&ligne);

            let encodee = encode_response_line(&ligne);
            let relue = parse_response_line(&encodee)
                .unwrap_or_else(|erreur| panic!("« {encodee} » doit se relire : {erreur}"));
            assert_eq!(
                std::mem::discriminant(&relue),
                std::mem::discriminant(&ligne),
                "« {encodee} » se relit en {relue:?} : le type de ligne a changé"
            );

            // Les champs qui ne sont pas en fin de ligne ressortent sans blanc ni caractère de
            // contrôle : c'est l'invariant de cadrage vu de l'intérieur d'une ligne.
            let sans_blanc = |champ: &str, quoi: &str| {
                assert!(
                    !champ.is_empty(),
                    "{quoi} vide disparaîtrait entre deux espaces : « {encodee} »"
                );
                assert!(
                    !champ.chars().any(|c| c.is_whitespace() || c.is_control()),
                    "{quoi} « {champ} » porte un blanc ou un caractère de contrôle après relecture \
                     de « {encodee} » — il se ferait prendre pour deux champs"
                );
            };

            match (&ligne, &relue) {
                (ResponseLine::Zone { cibles: posees, .. }, ResponseLine::Zone { nom, cibles }) => {
                    sans_blanc(nom, "le nom de zone");
                    assert_eq!(
                        cibles, posees,
                        "les cibles d'une zone ne dépendent pas de ce que porte son nom : \
                         « {encodee} »"
                    );
                }
                (
                    ResponseLine::ZoneLight { couleur: posee, .. },
                    ResponseLine::ZoneLight { nom, couleur },
                ) => {
                    sans_blanc(nom, "le nom de zone");
                    assert_eq!(
                        couleur, posee,
                        "la couleur traverse le champ hostile intacte"
                    );
                }
                (
                    ResponseLine::ZoneAnim {
                        reglages: poses, ..
                    },
                    ResponseLine::ZoneAnim {
                        nom,
                        animation,
                        reglages,
                    },
                ) => {
                    sans_blanc(nom, "le nom de zone");
                    sans_blanc(animation, "le nom d'animation");
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
                _ => panic!("{ligne:?} s'est relue en {relue:?}"),
            }
        }
    }
}
