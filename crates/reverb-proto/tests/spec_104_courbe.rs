//! Tests d'intention du verbe `curve` du protocole (issue #104).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #104 seule. Aucune ligne de
//! `crates/reverb-proto/src/` n'a été relue pour les écrire — seulement les
//! signatures publiques nécessaires à la compilation. À l'écriture de ce fichier,
//! `Request::Curve` et le mot `curve` n'existent nulle part : ces tests encodent
//! ce que le protocole **doit** faire, pas ce que le code fait. Si l'un d'eux
//! échoue après implémentation, c'est le code qu'on corrige, jamais le test.
//!
//! Ils prolongent `spec_ipc.rs` (#17), `spec_ipc_v2.rs` (#19), `spec_ipc_v3.rs`
//! (#23), `spec_ipc_zones.rs` (#29), `spec_ipc_ecran.rs` (#33) et
//! `spec_ipc_auto.rs` (#50), auxquels **ce fichier ne touche pas** : ce qui y est
//! écrit doit continuer de passer tel quel.
//!
//! # Pourquoi le protocole doit porter ce verbe
//!
//! issue #104, contexte — « Le carnet des courbes posées ne peut vivre que
//! **dans le processus qui a écrit**, parce qu'on ne peut rien savoir de ce qu'un
//! autre outil a fait — les fichiers `tempN_auto_pointM_pwm` sont en écriture
//! seule et ne se relisent jamais. Conséquence : **poser une courbe et l'activer
//! doivent tenir dans un seul geste**, sinon le carnet est perdu entre les deux. »
//!
//! Le socket n'a que `FanAction::{Pwm, Auto}`. Aucune courbe n'entre donc jamais
//! dans le carnet du démon, et la fenêtre n'offre jamais « auto ». Le verbe
//! `curve` est ce qui manque : il pose la courbe **et** peut l'activer, dans la
//! même requête, donc dans le même processus.
//!
//! ⚠️ **Ce qu'il ne fera jamais, et ce que ce fichier ne teste donc pas** :
//! relire une courbe posée. L'issue le dit — « Le démon ne pourra jamais
//! **relire** la courbe posée, et ne doit pas prétendre le contraire. » Il n'y a
//! donc ni action de lecture (`curve <canal>` seul est **refusé**, pas rendu),
//! ni ligne de réponse `curve`.
//!
//! # Ce que ce fichier fige
//!
//! 1. `Request::Curve { channel, points, activer }`, avec `points` de type
//!    `[u8; CURVE_POINTS]`, traverse l'encodage et le décodage **dans les deux
//!    sens**, sans rien perdre.
//! 2. « Pose » et « pose et active » sont deux lignes différentes, et l'aller-
//!    retour préserve la distinction.
//! 3. La **forme exacte** de la ligne, comparée à une chaîne littérale : c'est
//!    elle que le critère « la ligne reste lisible par un démon d'avant, ou
//!    l'incompatibilité est explicite » oblige à ne pas laisser dériver.
//! 4. Un canal vide ou omis, une courbe incomplète ou hors bornes, une ligne
//!    malformée : refusés, en nommant le verbe et en disant quoi.
//! 5. Une ligne encodée tient sur **une seule ligne physique**, sans caractère
//!    de contrôle, quel que soit le nom de canal qu'on lui donne.
//!
//! # Les choix de forme, et pourquoi
//!
//! L'issue dit « Un verbe `curve` sur le socket, qui pose la courbe **et** peut
//! l'activer » et rien de plus sur la grammaire. Ce fichier tranche, puisque
//! c'est lui le contrat :
//!
//! - **La grammaire est `curve <canal> set|enable <p1>,…,<p40>`.** Le mot
//!   d'action se place juste après le canal, comme dans `fan <canal> pwm 50`,
//!   `fan <canal> auto` et `regule <canal> on` : dans tout le protocole, le
//!   jeton qui suit un canal dit ce qu'on lui fait. La charge variable finit la
//!   ligne, comme la règle du dernier champ de #17 l'impose partout ailleurs.
//! - **Les quarante consignes sont un seul jeton, séparées par des virgules**,
//!   exactement comme `paint <cible> <rrggbb>,<rrggbb>,…` (#19) — la seule autre
//!   commande du protocole qui porte une valeur par élément d'une série. Des
//!   espaces en feraient quarante champs, et la ligne cesserait d'avoir un
//!   nombre de jetons fixe.
//! - **Le mot d'activation est `enable`**, celui du drapeau `--enable` que
//!   l'issue demande en ligne de commande. Le socket et la ligne de commande
//!   disent alors le même mot pour le même geste, ce qui est tout l'intérêt d'un
//!   `reverb curve` qui passe par le socket quand le démon tourne.
//! - **`set` est écrit, jamais sous-entendu.** Un `enable` facultatif en fin de
//!   ligne se lirait aussi bien comme un oubli que comme un choix, et
//!   l'activation est justement le geste qui ne pardonne pas : `pwm_enable = 2`
//!   sur une courbe qui n'a pas été posée coupe la régulation de la pompe (#97).
//!   Deux mots obligatoires, deux intentions distinctes.
//! - **`activer` est un `bool`, pas une paire de variantes.** Les deux formes ne
//!   diffèrent par aucune donnée, seulement par un drapeau ; un `CurveAction` à
//!   deux variantes portant le même `[u8; 40]` serait une abstraction que
//!   personne n'a demandée. Le protocole a déjà un booléen sur le fil,
//!   `sait_faire_auto` (#50), écrit en toutes lettres plutôt qu'en `0`/`1`.
//! - **`points` est un tableau de taille fixe, `[u8; CURVE_POINTS]`.** Une
//!   courbe incomplète devient **irreprésentable** au lieu d'être vérifiée à
//!   l'exécution — c'est la règle que `SlotAddress` (#15) et `NomProfil` (#74)
//!   appliquent déjà là où une erreur coûterait cher. Le seul endroit où le
//!   compte peut clocher est donc la **lecture** d'une ligne, et c'est là que le
//!   refus est exigé.
//! - **Une consigne hors de `0..=100` est refusée sur le fil.** Même arbitrage
//!   que `screen brightness 101` (#33) : le type `u8` la porte, le fil la
//!   refuse. Le pourcentage est l'unité du protocole lui-même — `fan <canal>
//!   pwm <0-100>`, `regule courbe <millidegrés>:<0-100>` —, pas une politique
//!   du matériel.
//!
//! # Ce que ce fichier laisse au démon, et pourquoi
//!
//! - **Le canal inconnu.** Seul le démon a la liste des canaux ; le protocole ne
//!   peut que **transporter le nom intact**, pour que le refus puisse le nommer.
//!   C'est ce que `le_nom_du_canal_traverse_intact_pour_que_le_refus_le_nomme`
//!   vérifie, et rien de plus.
//! - **Une courbe qui descend.** `CurveError::Decreasing` est un fait de
//!   `reverb-hw`, qui écrit les fichiers ; ici, une suite décroissante est une
//!   ligne parfaitement formée. Le protocole transporte, le démon juge — comme
//!   pour les réglages d'`animate` (#19) et les paliers de `regule courbe`
//!   (#99).
//! - **Le carnet des courbes posées, et le déblocage d'« auto ».** Ce sont des
//!   critères du démon (#97), pas du fil.
//!
//! Aucun accès matériel, aucune IO, aucun socket, aucun fichier : `reverb-proto`
//! est pur, ses tests aussi.

use reverb_proto::ipc::{
    CURVE_POINTS, MAX_LINE_LEN, Request, RequestError, encode_request, parse_request,
};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Le canal de l'exemple de l'issue — celui dont la régulation d'usine est
/// vivante, et que #97 protège d'un « auto » posé sans courbe.
const CANAL: &str = "kraken2023elite:pump-speed";

/// L'autre canal du Kraken, celui qui a lui aussi une courbe matérielle.
const AUTRE_CANAL: &str = "kraken2023elite:fan-speed";

/// Un canal qui existe mais **n'a pas** de courbe matérielle : le pilote
/// `nzxt-smart2` n'a aucun mode automatique (#50). Le protocole doit le
/// transporter quand même — c'est le démon qui refusera, en le nommant.
const SANS_COURBE: &str = "nzxtsmart2:fan-1";

/// La courbe témoin de ce fichier : dix points à 30 %, quinze à 60 %, quinze à
/// 100 %.
///
/// Trois paliers plutôt qu'une interpolation, et c'est délibéré : la ligne figée
/// par `la_forme_de_la_ligne_curve_est_figee` doit pouvoir se relire à l'œil.
/// Une rampe interpolée y ferait dépendre le contrat d'une règle d'arrondi.
fn temoin() -> [u8; CURVE_POINTS] {
    let mut points = [0u8; CURVE_POINTS];
    for (i, point) in points.iter_mut().enumerate() {
        *point = if i < 10 {
            30
        } else if i < 25 {
            60
        } else {
            100
        };
    }
    points
}

/// Une courbe plate à `consigne` — dont les deux bornes du protocole, `0` et
/// `100`.
fn plate(consigne: u8) -> [u8; CURVE_POINTS] {
    [consigne; CURVE_POINTS]
}

/// Une rampe croissante qui balaie une bonne part de l'intervalle sans en
/// sortir : de 20 % à 98 %, deux points de plus à chaque cran.
///
/// Ses quarante valeurs sont toutes différentes — c'est ce qui permet de voir un
/// ordre inversé, qu'une courbe plate cacherait.
fn rampe() -> [u8; CURVE_POINTS] {
    let mut points = [0u8; CURVE_POINTS];
    for (i, point) in points.iter_mut().enumerate() {
        *point = 20 + u8::try_from(i).expect("CURVE_POINTS tient dans un u8") * 2;
    }
    points
}

/// La rampe à l'envers : une courbe qui **descend** quand la température monte.
///
/// Elle est bien formée sur le fil, et c'est tout ce que ce fichier en dit — son
/// refus appartient à `reverb-hw` (`CurveError::Decreasing`), qui écrit les
/// fichiers et connaît le matériel. Le protocole transporte, le démon juge.
fn descendante() -> [u8; CURVE_POINTS] {
    let mut points = rampe();
    points.reverse();
    points
}

fn courbe(channel: &str, points: [u8; CURVE_POINTS], activer: bool) -> Request {
    Request::Curve {
        channel: channel.to_owned(),
        points,
        activer,
    }
}

/// La liste de consignes telle qu'elle s'écrit sur le fil : des nombres décimaux
/// séparés par des virgules, sans espace.
fn liste(points: &[u8]) -> String {
    points
        .iter()
        .map(u8::to_string)
        .collect::<Vec<String>>()
        .join(",")
}

/// Une ligne `curve` construite à la main, pour les cas de lecture.
fn ligne(canal: &str, mot: &str, points: &[u8]) -> String {
    format!("curve {canal} {mot} {}", liste(points))
}

/// Les trois champs d'une requête `curve`, ou un échec qui dit ce qu'on a reçu à
/// la place.
fn curve_de(ligne: &str) -> (String, [u8; CURVE_POINTS], bool) {
    match parse_request(ligne) {
        Ok(Request::Curve {
            channel,
            points,
            activer,
        }) => (channel, points, activer),
        Ok(autre) => {
            panic!("« {ligne} » devait être une commande `curve`, elle a rendu {autre:?}")
        }
        Err(erreur) => panic!("« {ligne} » devait être acceptée : {erreur}"),
    }
}

/// Le verbe et la raison d'un refus d'arguments, ou un échec qui dit ce qui est
/// arrivé.
///
/// Même idiome que `spec_ipc_ecran.rs` : un verbe **connu** — donc `BadArgument`
/// et non `UnknownVerb` — et une raison non vide. Le texte de la raison n'est
/// jamais figé, seulement ce qu'il doit nommer.
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

/// Le refus d'une commande de courbe : le verbe nommé doit être `curve`.
///
/// Le verbe reçu est `curve`, il existe : ce sont ses arguments qui sont
/// mauvais. Nommer autre chose enverrait chercher une faute là où il n'y en a
/// pas — même arbitrage que `screen` dans #33 et `zone` dans #29.
fn refus_de_curve(ligne: &str) -> String {
    let (verbe, raison) = refus(ligne);
    assert_eq!(
        verbe, "curve",
        "« {ligne} » : le verbe reçu est `curve`, l'erreur doit le nommer"
    );
    raison
}

/// Noms de canal **hostiles** : ceux qui, mal encodés, scinderaient la commande
/// en deux ou la feraient passer pour la fin d'une réponse.
///
/// Un nom de canal vient du matériel, pas de nous : il est construit depuis le
/// nom d'un pilote lu dans sysfs. La liste reprend celle de `spec_ipc_ecran.rs`,
/// dont la raison vaut ici encore — une ligne `curve` porte quarante consignes,
/// et une commande scindée en deux poserait une demi-courbe.
///
/// Le nom **vide** n'y figure pas : ce qu'il devient sur le fil est traité à
/// part, par `un_canal_vide_ne_fait_pas_dire_autre_chose_a_la_ligne`.
const CANAUX_HOSTILES: &[&str] = &[
    "err",
    "end",
    "error",
    "endpoint",
    "ERR",
    "boom\nend",
    "\nend",
    "end\n",
    "a\r\nend",
    "mon canal",
    "\t",
    "  ",
    "a\u{0}b",
    "curve",
    "set",
    "enable",
    "\u{feff}a",
];

// ---------------------------------------------------------------------------
// 1 — une courbe complète fait l'aller-retour sans rien perdre
// ---------------------------------------------------------------------------

#[test]
fn une_courbe_complete_fait_l_aller_retour_sans_rien_perdre() {
    // issue #104, critère d'acceptation — « Le socket accepte une courbe
    // complète et l'écrit sur le canal nommé. »
    //
    // Les deux sens comptent autant l'un que l'autre : sans le réencodage
    // exact, un encodeur qui écrirait ses consignes autrement passerait un test
    // de décodage indulgent sans qu'on le voie, et une fenêtre d'une version
    // ultérieure ne saurait plus le lire. C'est aussi ce qui rend
    // « `reverb curve` passe par le socket quand le démon tourne, et écrit en
    // direct sinon » possible sans deux vocabulaires à tenir.
    let temoins = [
        courbe(CANAL, temoin(), false),
        courbe(CANAL, temoin(), true),
        courbe(AUTRE_CANAL, rampe(), false),
        courbe(AUTRE_CANAL, rampe(), true),
        // Les deux bornes du protocole, qui doivent passer telles quelles :
        // « à l'arrêt » et « à fond » sont des courbes légitimes sur le fil.
        courbe(CANAL, plate(0), false),
        courbe(CANAL, plate(100), true),
        // Une courbe qui **descend** est une ligne bien formée : c'est
        // `reverb-hw` qui la refusera, pas le protocole (en-tête).
        courbe(CANAL, descendante(), false),
        // Le canal sans courbe matérielle traverse aussi : le refus appartient
        // au démon, qui seul connaît la liste.
        courbe(SANS_COURBE, temoin(), false),
    ];

    for requete in temoins {
        let encodee = encode_request(&requete);
        assert_eq!(
            encodee.lines().count(),
            1,
            "une requête tient sur une seule ligne : « {encodee} »"
        );
        assert_eq!(
            parse_request(&encodee),
            Ok(requete.clone()),
            "aller-retour exact de {requete:?} par « {encodee} »"
        );
    }
}

#[test]
fn les_quarante_consignes_reviennent_dans_l_ordre() {
    // Une courbe est une suite **ordonnée** : le point n° 1 est le plus froid.
    // Un encodage qui inverserait ou trierait la liste passerait tous les tests
    // d'aller-retour écrits sur des courbes plates, et poserait une courbe à
    // l'envers sur la pompe sans un message. La rampe est là pour ça — ses
    // quarante valeurs sont toutes différentes.
    let attendus = rampe();
    let (_, relus, _) = curve_de(&encode_request(&courbe(CANAL, attendus, false)));

    assert_eq!(
        relus.len(),
        CURVE_POINTS,
        "une courbe porte exactement {CURVE_POINTS} consignes"
    );
    assert_eq!(
        relus, attendus,
        "les consignes reviennent dans l'ordre où elles ont été posées"
    );
}

#[test]
fn le_nom_du_canal_traverse_intact_pour_que_le_refus_le_nomme() {
    // issue #104, critère d'acceptation — « Un canal inconnu […] est refusé en
    // le nommant. »
    //
    // Le protocole ne peut pas savoir quels canaux existent : seul le démon a la
    // liste, lue dans sysfs. Ce qu'il doit garantir, c'est que le nom arrive
    // **intact** jusqu'à lui — un nom tronqué au premier tiret ou mis en
    // minuscules ferait refuser un canal qui existe, ou nommer le mauvais dans
    // le refus.
    for nom in [CANAL, AUTRE_CANAL, SANS_COURBE, "un-canal-qui-n-existe-pas"] {
        let (relu, _, _) = curve_de(&encode_request(&courbe(nom, temoin(), false)));
        assert_eq!(relu, nom, "le nom du canal traverse sans être retouché");
    }
}

// ---------------------------------------------------------------------------
// 2 — poser et poser-puis-activer ne se confondent jamais
// ---------------------------------------------------------------------------

#[test]
fn poser_et_poser_puis_activer_sont_deux_lignes_differentes() {
    // issue #104, critères d'acceptation — « `reverb curve --enable` pose les 40
    // points puis bascule le canal, dans le même processus, et réussit » et
    // « Sans `--enable`, `reverb curve` se comporte comme aujourd'hui ».
    //
    // C'est le cœur de l'issue : les deux gestes doivent être distincts **sur le
    // fil**, sinon le socket ne peut pas porter la différence, et le carnet des
    // courbes posées se perd de nouveau entre deux commandes. Le mode de
    // défaillance est silencieux dans les deux sens — une activation perdue
    // laisse un canal sur sa courbe d'avant, une activation surnuméraire pousse
    // `pwm_enable = 2` sur un canal qu'on ne voulait pas basculer.
    let pose = courbe(CANAL, temoin(), false);
    let pose_et_active = courbe(CANAL, temoin(), true);

    assert_ne!(pose, pose_et_active, "les deux requêtes sont distinctes");
    assert_ne!(
        encode_request(&pose),
        encode_request(&pose_et_active),
        "et leurs deux lignes le sont aussi"
    );

    assert_eq!(parse_request(&encode_request(&pose)), Ok(pose.clone()));
    assert_eq!(
        parse_request(&encode_request(&pose_et_active)),
        Ok(pose_et_active.clone())
    );

    // Et la distinction se lit dans le drapeau, pas seulement dans l'égalité des
    // requêtes : une implémentation qui rangerait l'activation ailleurs que dans
    // `activer` passerait les assertions ci-dessus.
    let (_, _, sans) = curve_de(&encode_request(&pose));
    let (_, _, avec) = curve_de(&encode_request(&pose_et_active));
    assert!(!sans, "« set » ne bascule pas le canal");
    assert!(avec, "« enable » le bascule");
}

#[test]
fn les_deux_lignes_ne_different_que_par_leur_mot_d_action() {
    // Corollaire de la grammaire choisie (en-tête) : le canal et les quarante
    // consignes sont écrits pareil dans les deux cas. Si l'activation
    // déplaçait, réordonnait ou reformatait quoi que ce soit d'autre, une des
    // deux formes finirait par dériver de l'autre sans qu'on s'en aperçoive.
    let sans = encode_request(&courbe(CANAL, temoin(), false));
    let avec = encode_request(&courbe(CANAL, temoin(), true));

    let jetons_sans: Vec<&str> = sans.split(' ').collect();
    let jetons_avec: Vec<&str> = avec.split(' ').collect();

    assert_eq!(jetons_sans.len(), 4, "quatre jetons attendus : « {sans} »");
    assert_eq!(jetons_avec.len(), 4, "quatre jetons attendus : « {avec} »");

    assert_eq!(jetons_sans[0], "curve");
    assert_eq!(jetons_avec[0], "curve");
    assert_eq!(jetons_sans[1], CANAL, "le canal est le deuxième jeton");
    assert_eq!(jetons_avec[1], CANAL, "le canal est le deuxième jeton");
    assert_eq!(jetons_sans[2], "set");
    assert_eq!(jetons_avec[2], "enable");
    assert_eq!(
        jetons_sans[3], jetons_avec[3],
        "les consignes s'écrivent pareil, activation ou non"
    );
}

// ---------------------------------------------------------------------------
// 3 — la forme de la ligne est figée
// ---------------------------------------------------------------------------

#[test]
fn la_forme_de_la_ligne_curve_est_figee() {
    // issue #104, critère d'acceptation — « La ligne reste lisible par un démon
    // d'avant, ou l'incompatibilité est explicite. »
    //
    // Un verbe neuf ne peut pas être lu par un démon d'avant : celui-ci répondra
    // `err commande « curve » inconnue`, et c'est bien la seconde branche du
    // critère — l'incompatibilité est explicite, jamais une demi-courbe posée en
    // silence. Ce que ce test protège, c'est l'**autre** dérive, celle qui n'a
    // pas de message : deux versions de Reverb qui n'écrivent pas la même ligne
    // pour la même courbe. `tools/installe.sh` met le démon et la fenêtre à jour
    // ensemble, mais rien ne garantit qu'un `reverb` neuf ne parlera jamais à un
    // `reverbd` de la veille.
    //
    // D'où une chaîne littérale plutôt qu'une reconstruction : une comparaison
    // qui recalcule le format ne peut pas détecter un changement de format.
    let attendue_set = "curve kraken2023elite:pump-speed set \
                        30,30,30,30,30,30,30,30,30,30,\
                        60,60,60,60,60,60,60,60,60,60,60,60,60,60,60,\
                        100,100,100,100,100,100,100,100,100,100,100,100,100,100,100";
    let attendue_enable = "curve kraken2023elite:pump-speed enable \
                           30,30,30,30,30,30,30,30,30,30,\
                           60,60,60,60,60,60,60,60,60,60,60,60,60,60,60,\
                           100,100,100,100,100,100,100,100,100,100,100,100,100,100,100";

    assert_eq!(
        encode_request(&courbe(CANAL, temoin(), false)),
        attendue_set,
        "la forme du fil ne doit pas dériver"
    );
    assert_eq!(
        encode_request(&courbe(CANAL, temoin(), true)),
        attendue_enable,
        "la forme du fil ne doit pas dériver"
    );

    // Et les deux se relisent en la requête d'où elles viennent : figer une
    // forme que l'analyse ne saurait pas relire ne prouverait rien.
    assert_eq!(
        parse_request(attendue_set),
        Ok(courbe(CANAL, temoin(), false))
    );
    assert_eq!(
        parse_request(attendue_enable),
        Ok(courbe(CANAL, temoin(), true))
    );
}

#[test]
fn le_protocole_compte_autant_de_points_que_le_materiel() {
    // issue #104 — « Une courbe fait 40 points et tient sur une ligne de texte ».
    //
    // Le nombre est un fait matériel : le Kraken expose quarante fichiers
    // `tempN_auto_pointM_pwm`. `reverb-proto` est pur et ne lit pas sysfs, mais
    // il porte déjà les constantes du matériel — `ram::LEDS_PER_STICK`,
    // `screen::WIDTH`. Celle-ci y a sa place, et c'est elle qui doit servir des
    // deux côtés : deux `40` écrits séparément dériveraient sans un message.
    assert_eq!(
        CURVE_POINTS, 40,
        "quarante points, comme les fichiers du pilote"
    );
}

#[test]
fn une_courbe_complete_tient_dans_une_ligne_du_protocole() {
    // issue #104 — « Une courbe fait 40 points et tient sur une ligne de texte,
    // contrairement au mégaoctet d'une image : le protocole texte n'a pas à être
    // contourné. »
    //
    // C'est l'argument qui justifie de faire passer la courbe par le socket
    // plutôt que par un chemin de fichier comme l'écran (#33). Il ne tient que
    // si la ligne la plus longue possible passe : quarante consignes à trois
    // chiffres, sur le canal au nom le plus long. La vérifier ici évite de
    // découvrir sur le matériel qu'une courbe à fond est refusée pour longueur.
    let long_canal = "un-controleur-au-nom-deraisonnablement-long:fan-speed";
    for canal in [CANAL, AUTRE_CANAL, SANS_COURBE, long_canal] {
        for activer in [false, true] {
            let encodee = encode_request(&courbe(canal, plate(100), activer));
            assert!(
                encodee.len() <= MAX_LINE_LEN,
                "« {encodee} » fait {} octets, au-delà des {MAX_LINE_LEN} du protocole",
                encodee.len()
            );
            assert_eq!(
                parse_request(&encodee),
                Ok(courbe(canal, plate(100), activer)),
                "et la ligne la plus longue se relit"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — une ligne encodée reste une seule ligne
// ---------------------------------------------------------------------------

#[test]
fn aucune_ligne_curve_ne_porte_de_saut_de_ligne_ni_de_caractere_de_controle() {
    // En-tête du module `ipc` de #17 : « le préfixe de type assure l'invariant
    // pour le début de ligne. Il n'assure rien contre un saut de ligne à
    // l'intérieur d'un champ. »
    //
    // Le protocole est en texte, **une ligne par requête**. Un nom de canal
    // portant un saut de ligne scinderait la commande en deux, et la seconde
    // moitié — quarante consignes nues — serait lue comme une requête à part
    // entière. Un nom de canal vient du matériel : il est construit depuis le
    // nom d'un pilote lu dans sysfs, donc de rien qu'on écrive nous-même.
    for &hostile in CANAUX_HOSTILES {
        for activer in [false, true] {
            let requete = courbe(hostile, temoin(), activer);
            let encodee = encode_request(&requete);

            assert_eq!(
                encodee.lines().count(),
                1,
                "une requête tient sur une seule ligne, quel que soit son canal : « {encodee} »"
            );
            assert!(
                !encodee.chars().any(char::is_control),
                "aucun caractère de contrôle dans une requête encodée : {encodee:?}"
            );

            let (canal, points, relu_activer) = curve_de(&encodee);
            assert!(
                !canal.is_empty(),
                "un canal vide disparaîtrait entre deux espaces : « {encodee} »"
            );
            assert!(
                !canal.chars().any(|c| c.is_whitespace() || c.is_control()),
                "le canal {canal:?} porte un blanc ou un caractère de contrôle après relecture de \
                 « {encodee} » — il se ferait prendre pour deux champs"
            );
            assert_eq!(
                points,
                temoin(),
                "le canal hostile n'a pas déplacé les consignes : « {encodee} »"
            );
            assert_eq!(
                relu_activer, activer,
                "le canal hostile n'a pas changé l'activation : « {encodee} »"
            );
        }
    }
}

#[test]
fn une_ligne_curve_ne_se_fait_jamais_prendre_pour_une_fin_de_reponse() {
    // Règle n° 5 de `spec_ipc.rs` (#17), reprise du côté des requêtes : rien de
    // ce qu'on écrit sur le socket ne doit commencer par `end` ni par `err`, les
    // deux mots qui terminent une réponse. Un canal peut porter ces noms — le
    // matériel choisit ses noms, pas nous.
    for &hostile in CANAUX_HOSTILES {
        let encodee = encode_request(&courbe(hostile, temoin(), true));
        for terminal in ["end", "err"] {
            assert!(
                !encodee.starts_with(terminal),
                "« {encodee} » commence par « {terminal} »"
            );
        }
    }
}

#[test]
fn un_canal_vide_ne_fait_pas_dire_autre_chose_a_la_ligne() {
    // Le champ vide est le seul hostile qui n'ait aucun caractère à neutraliser.
    // Ce que le contrat exige n'est donc pas qu'il traverse, mais qu'il ne
    // **décale rien** : une ligne dont le canal se serait évaporé entre deux
    // espaces ne doit jamais se relire en une commande `curve` plausible, où le
    // mot d'action aurait glissé à la place du canal et la liste de consignes à
    // la place du mot d'action.
    //
    // Refuser à la relecture est un résultat parfaitement acceptable ; se relire
    // en une autre courbe ne l'est pas.
    for activer in [false, true] {
        let encodee = encode_request(&courbe("", temoin(), activer));
        match parse_request(&encodee) {
            Err(_) => {}
            Ok(Request::Curve {
                channel,
                points,
                activer: relu,
            }) => {
                assert!(
                    !channel.is_empty(),
                    "« {encodee} » se relit sur un canal vide"
                );
                assert_eq!(points, temoin(), "« {encodee} » a décalé les consignes");
                assert_eq!(relu, activer, "« {encodee} » a changé l'activation");
            }
            Ok(autre) => {
                panic!("« {encodee} » se relit en {autre:?} — le canal vide a changé de verbe")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — les refus, en nommant ce qui cloche
// ---------------------------------------------------------------------------

#[test]
fn un_canal_omis_est_refuse_en_nommant_curve() {
    // issue #104, critère d'acceptation — « Un canal inconnu, une courbe
    // incomplète ou hors bornes sont refusés en le nommant. »
    //
    // La ligne compte quatre jetons, et aucun n'est facultatif. Une ligne plus
    // courte n'est pas une commande partielle : c'est une commande cassée. La
    // laisser passer en devinant le canal manquant poserait une courbe sur un
    // canal qu'on n'a pas nommé.
    let points = liste(&temoin());
    for ligne in [
        "curve".to_owned(),
        format!("curve {CANAL}"),
        format!("curve {CANAL} set"),
        format!("curve {CANAL} enable"),
        format!("curve set {points}"),
        format!("curve enable {points}"),
        format!("curve {points}"),
    ] {
        refus_de_curve(&ligne);
    }
}

#[test]
fn une_courbe_incomplete_est_refusee_en_nommant_le_compte() {
    // issue #104, critère d'acceptation — « […] une courbe incomplète […] sont
    // refusés en le nommant. »
    //
    // Une courbe trop courte complétée par des zéros donnerait des ventilateurs
    // à l'arrêt sur les points chauds, sans une erreur : exactement le mode de
    // défaillance rassurant que le projet refuse partout. Le compte doit donc
    // être **exact**, ni trop ni trop peu, et le refus doit dire lequel des deux
    // — « argument invalide » n'apprend rien sur une ligne de cent soixante-dix
    // caractères.
    let trop_court = temoin()[..CURVE_POINTS - 1].to_vec();
    let beaucoup_trop_court = vec![50u8; 2];
    let un_seul = vec![50u8];
    let mut trop_long = temoin().to_vec();
    trop_long.push(100);

    for points in [trop_court, beaucoup_trop_court, un_seul, trop_long] {
        for mot in ["set", "enable"] {
            let ligne = ligne(CANAL, mot, &points);
            let raison = refus_de_curve(&ligne);
            let bas = raison.to_lowercase();
            assert!(
                bas.contains(&CURVE_POINTS.to_string())
                    || bas.contains(&points.len().to_string())
                    || bas.contains("point")
                    || bas.contains("consigne"),
                "une courbe de {} consignes doit être refusée en nommant le compte. \
                 Raison obtenue : {raison}",
                points.len()
            );
        }
    }
}

#[test]
fn une_courbe_vide_est_refusee() {
    // Le cas limite de l'incomplétude : le mot d'action est là, la liste non.
    // Rien à interpoler, rien à poser — et surtout, aucune raison de le
    // confondre avec « remets ce canal à zéro », qui n'est pas ce que la
    // commande dit.
    for mot in ["set", "enable"] {
        refus_de_curve(&format!("curve {CANAL} {mot} "));
        refus_de_curve(&format!("curve {CANAL} {mot} ,"));
        refus_de_curve(&format!("curve {CANAL} {mot} ,,,"));
    }
}

#[test]
fn une_consigne_hors_bornes_est_refusee_en_nommant_la_valeur() {
    // issue #104, critère d'acceptation — « […] ou hors bornes sont refusés en
    // le nommant. »
    //
    // Le pourcentage est l'unité du protocole lui-même : `fan <canal> pwm
    // <0-100>`, `regule courbe <millidegrés>:<0-100>`, `screen brightness
    // <0-100>`. Une consigne à 200 est soit une faute de frappe, soit une erreur
    // d'unité — un tableau gradué 0–255 pris pour un pourcentage —, et l'écrêter
    // en silence ferait passer la seconde pour un réglage qui marche.
    // ⚠️ Que des écritures décimales positives : `-1` n'est pas « hors bornes »
    // mais « pas un nombre », et son refus est vérifié par
    // `une_ligne_curve_malformee_est_refusee_sans_paniquer`. Les mélanger ferait
    // exiger d'un même message qu'il nomme deux fautes différentes.
    for fautive in ["101", "150", "200", "255", "256", "1000"] {
        for position in [0usize, 1, CURVE_POINTS / 2, CURVE_POINTS - 1] {
            let mut valeurs: Vec<String> = temoin().iter().map(u8::to_string).collect();
            valeurs[position] = fautive.to_owned();
            let ligne = format!("curve {CANAL} set {}", valeurs.join(","));

            let raison = refus_de_curve(&ligne);
            assert!(
                raison.contains(fautive)
                    || raison.contains("100")
                    || raison.to_lowercase().contains("born"),
                "« {fautive} » en position {position} doit être refusée en nommant ce qui cloche. \
                 Raison obtenue : {raison}"
            );
        }
    }

    // Et la borne haute passe, elle : c'est « à fond », pas une faute.
    assert_eq!(
        parse_request(&ligne(CANAL, "set", &plate(100))),
        Ok(courbe(CANAL, plate(100), false))
    );
    assert_eq!(
        parse_request(&ligne(CANAL, "set", &plate(0))),
        Ok(courbe(CANAL, plate(0), false))
    );
}

#[test]
fn une_ligne_curve_malformee_est_refusee_sans_paniquer() {
    // Le protocole lit ce qu'un client lui envoie, et un client peut être un
    // `socat` tapé à la main. Aucune de ces lignes ne doit faire tomber le
    // démon : `parse_request` rend une erreur, il ne panique pas.
    //
    // Les deux familles que le sujet nomme y figurent — champ manquant et nombre
    // non numérique —, plus les écritures qu'un autre encodeur choisirait
    // spontanément (hexadécimal, notation scientifique, décimales, espaces au
    // lieu de virgules).
    let bons = temoin();
    let mut lignes = vec![
        format!("curve {CANAL} poser {}", liste(&bons)),
        format!("curve {CANAL} SET {}", liste(&bons)),
        format!("curve {CANAL} on {}", liste(&bons)),
        format!("curve {CANAL} auto {}", liste(&bons)),
        format!("curve {CANAL} pwm {}", liste(&bons)),
        // Des espaces au lieu de virgules : quarante champs au lieu d'un.
        format!("curve {CANAL} set {}", liste(&bons).replace(',', " ")),
        // Un jeton de trop après une ligne par ailleurs valide.
        format!("curve {CANAL} set {} enable", liste(&bons)),
        format!("curve {CANAL} set {} {}", liste(&bons), liste(&bons)),
    ];

    // Un nombre qui n'en est pas, à chaque place où il pourrait se glisser.
    for fautif in [
        "x",
        "",
        " ",
        "-",
        "+",
        "1e2",
        "0x40",
        "5.5",
        "50%",
        "cinquante",
        "nan",
        "inf",
        "٣",
        "١٠٠",
    ] {
        for position in [0usize, CURVE_POINTS / 2, CURVE_POINTS - 1] {
            let mut valeurs: Vec<String> = bons.iter().map(u8::to_string).collect();
            valeurs[position] = fautif.to_owned();
            lignes.push(format!("curve {CANAL} set {}", valeurs.join(",")));
        }
    }

    for ligne in &lignes {
        assert!(
            parse_request(ligne).is_err(),
            "« {ligne} » est malformée, elle doit être refusée"
        );
        // Et le refus doit être exploitable : verbe connu, raison non vide.
        refus_de_curve(ligne);
    }
}

#[test]
fn une_ligne_curve_trop_longue_est_refusee_pour_sa_longueur() {
    // #17 — la longueur est vérifiée **avant** tout découpage, et l'erreur ne
    // porte que la longueur. Une courbe légitime tient très largement dans les
    // 1024 octets du protocole ; ce qui peut déborder, c'est un nom de canal
    // aberrant, et le refus doit alors parler de longueur plutôt que de courbe.
    let canal = "c".repeat(MAX_LINE_LEN);
    let trop_longue = ligne(&canal, "set", &temoin());
    assert!(trop_longue.len() > MAX_LINE_LEN);

    match parse_request(&trop_longue) {
        Err(RequestError::TooLong { given }) => {
            assert_eq!(given, trop_longue.len(), "l'erreur porte la longueur reçue");
        }
        autre => panic!(
            "une ligne de {} octets doit donner un TooLong, pas {autre:?}",
            trop_longue.len()
        ),
    }
}

#[test]
fn deux_fautes_differentes_ne_rendent_pas_la_meme_phrase() {
    // Un refus qui dit toujours la même chose n'aide personne à corriger une
    // ligne de cent soixante-dix caractères. Les trois fautes que l'issue nomme
    // — canal, compte, bornes — doivent se distinguer dans le message.
    let compte = refus_de_curve(&ligne(CANAL, "set", &temoin()[..10]));

    let mut hors = temoin();
    hors[3] = 200;
    let bornes = refus_de_curve(&ligne(CANAL, "set", &hors));

    let action = refus_de_curve(&ligne(CANAL, "poser", &temoin()));

    let manquant = refus_de_curve("curve");

    let raisons = [&compte, &bornes, &action, &manquant];
    for (i, une) in raisons.iter().enumerate() {
        for autre in raisons.iter().skip(i + 1) {
            assert_ne!(
                une, autre,
                "deux fautes différentes doivent se dire différemment"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 6 — la cohabitation avec le protocole d'avant
// ---------------------------------------------------------------------------

#[test]
fn un_verbe_inconnu_reste_refuse_en_le_nommant() {
    // issue #104, critère d'acceptation — « La ligne reste lisible par un démon
    // d'avant, ou l'incompatibilité est explicite. »
    //
    // `curve` est un **premier mot neuf** : un démon d'avant #104 ne peut que
    // répondre « commande inconnue » en la nommant, jamais lire la ligne de
    // travers. C'est cette branche-là du critère que le protocole tient, et ce
    // test vérifie que le mécanisme qui la porte — `UnknownVerb`, qui nomme le
    // verbe reçu — marche encore une fois `curve` ajouté.
    //
    // Le cas se produit pour de bon : `reverb` et `reverbd` sont deux binaires
    // installés séparément, et `reverb curve` passe par le socket dès que le
    // démon tourne.
    for verbe in ["courbe", "curves", "curve2", "cur", "Curve", "CURVE"] {
        match parse_request(&format!("{verbe} {CANAL} set {}", liste(&temoin()))) {
            Err(RequestError::UnknownVerb { verb }) => assert_eq!(
                verb, verbe,
                "l'erreur nomme le verbe reçu, pour qu'on voie lequel n'est pas compris"
            ),
            autre => panic!("« {verbe} » doit donner un UnknownVerb, pas {autre:?}"),
        }
    }
}

#[test]
fn curve_ne_se_confond_pas_avec_regule_courbe() {
    // Le protocole a désormais deux courbes, et elles n'ont ni la même unité, ni
    // le même exécutant :
    //
    // - `regule courbe <millidegrés>:<0-100> …` règle la courbe que **le démon**
    //   applique aux trois `nzxtsmart2` (#99) — elle s'arrête avec lui ;
    // - `curve <canal> set <p1>,…,<p40>` pose la courbe que **le firmware** du
    //   Kraken exécute — elle tient sans hôte.
    //
    // Les confondre poserait des millidegrés là où on attend des consignes, ou
    // l'inverse : aucun message, juste un résultat faux. C'est la faute des
    // trois ordres de composantes, reprise sur les courbes.
    let regule = parse_request("regule courbe 35000:30 45000:60 50000:100")
        .expect("la commande de #99 continue de passer telle quelle");
    assert!(
        matches!(regule, Request::Regule(_)),
        "« regule courbe … » reste une commande de régulation, elle a rendu {regule:?}"
    );

    let ligne_curve = ligne(CANAL, "set", &temoin());
    let posee = parse_request(&ligne_curve).expect("la commande de #104 passe");
    assert!(
        matches!(posee, Request::Curve { .. }),
        "« {ligne_curve} » est une pose de courbe matérielle, elle a rendu {posee:?}"
    );

    // Et le verbe `curve` ne prend pas la grammaire de l'autre.
    assert!(
        parse_request("curve 35000:30 45000:60 50000:100").is_err(),
        "des paliers en millidegrés ne sont pas une courbe matérielle"
    );
    assert!(
        parse_request(&format!("curve courbe {}", liste(&temoin()))).is_err(),
        "« curve courbe … » n'est pas une grammaire du protocole"
    );
}

#[test]
fn les_verbes_d_avant_la_courbe_traversent_toujours() {
    // Non-régression, comme #33 et #50 en ont écrit une : le verbe ajouté ne
    // doit rien casser de ce que les clients installés envoient déjà. La fenêtre
    // demande `status` une fois par seconde, et `reverb` a huit autres verbes.
    for ligne in [
        "status",
        "geometry",
        "lighting",
        "watch",
        "zone list",
        "animate vague",
        "light all ff2080",
        "fan nzxtsmart2:fan-1 auto",
        "fan nzxtsmart2:fan-1 pwm 50",
        "screen state",
        "screen off",
        "profil list",
        "regule",
        "regule nzxtsmart2:fan-1 on",
        "regule courbe 35000:30 45000:60 50000:100",
    ] {
        assert!(
            parse_request(ligne).is_ok(),
            "« {ligne} » marchait avant #104 et doit continuer"
        );
        assert!(
            !matches!(parse_request(ligne), Ok(Request::Curve { .. })),
            "« {ligne} » ne doit pas se relire en une pose de courbe"
        );
    }
}
