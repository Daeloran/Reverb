//! Tests d'intention du verbe `repare` du protocole (issue #136).
//!
//! Écrits **avant** l'implémentation, depuis les issues #136 et #98 seules. Aucune
//! ligne de `crates/*/src/` n'a été relue pour les écrire — seulement les
//! signatures publiques nécessaires à la compilation, relevées dans les fichiers de
//! tests d'intention de #17, #33 et #104. À l'écriture de ce fichier,
//! `Request::Repare` et le mot `repare` n'existent nulle part : la compilation doit
//! échouer, et c'est la phase rouge. Si l'un de ces tests échoue après
//! implémentation, c'est le code qu'on corrige, jamais le test.
//!
//! Ils prolongent `spec_ipc.rs` (#17), `spec_ipc_v2.rs` (#19), `spec_ipc_v3.rs`
//! (#23), `spec_ipc_zones.rs` (#29), `spec_ipc_ecran.rs` (#33), `spec_ipc_auto.rs`
//! (#50) et `spec_104_courbe.rs` (#104), auxquels **ce fichier ne touche pas** : ce
//! qui y est écrit doit continuer de passer tel quel.
//!
//! # Pourquoi le protocole doit porter ce verbe
//!
//! issue #136, contexte — le 2026-08-16, le Kraken a cessé de répondre à 12:53:37 ;
//! à 12:53:50, toutes ses cibles étant muettes, le démon a tenté le reset USB de
//! #98. Le noyau a ensuite perdu le périphérique pour de bon :
//!
//! ```text
//! 12:53:50  reverbd  réparation : reset USB de « kraken2023elite » (BB8C90820E900630)
//! 12:53:55  kernel   usb 1-9.1: device descriptor read/64, error -110
//! 12:54:53  kernel   usb 1-9.1: USB disconnect, device number 5
//! 12:55:35  kernel   usb 1-9-port1: attempt power cycle
//! 12:55:56  kernel   usb 1-9-port1: unable to enumerate USB device
//! ```
//!
//! « Sur les trois incidents connus, **aucun reset n'a jamais ramené le Kraken**. Le
//! geste ne guérit rien de mesuré, et il est le seul `ioctl` du projet qui fasse
//! disparaître un périphérique du bus. Il garde donc sa place — mais sous la main de
//! l'utilisateur, pas en automatique. »
//!
//! Le protocole n'a aujourd'hui aucun moyen de demander ce geste. Le verbe `repare`
//! est ce qui manque, et l'issue l'écrit en toutes lettres, des deux côtés :
//!
//! ```text
//! echo 'repare kraken2023elite' | socat - UNIX-CONNECT:/run/reverb/reverbd.sock
//! reverb repare kraken2023elite
//! ```
//!
//! # Ce que ce fichier fige
//!
//! 1. `Request::Repare { source }` traverse l'encodage et le décodage **dans les
//!    deux sens**, sans rien perdre.
//! 2. La **forme exacte** de la ligne, comparée à une chaîne littérale.
//! 3. Le nom de la source traverse **intact**, y compris quand il ne désigne rien :
//!    c'est ce qui permet au démon de refuser **en le nommant**.
//! 4. Une source omise ou vide, et un jeton de trop : refusés, en nommant le verbe
//!    et en disant quoi.
//! 5. Une ligne encodée tient sur **une seule ligne physique**, sans caractère de
//!    contrôle, quel que soit le nom qu'on lui donne.
//!
//! # Les choix de forme, et pourquoi
//!
//! L'issue donne la ligne — `repare kraken2023elite` — et rien de plus sur la
//! grammaire. Ce fichier tranche le reste, puisque c'est lui le contrat :
//!
//! - **Le verbe est `repare`, en français et sans accent.** C'est le mot que
//!   l'issue écrit des deux côtés, socket et ligne de commande. Le protocole mêle
//!   déjà les deux langues — `light`, `animate`, `screen`, `curve` d'un côté,
//!   `zone`, `profil`, `regule`, `geometry` de l'autre — et ce qui compte est que le
//!   socket et `reverb` disent **le même mot pour le même geste**, comme `curve`
//!   et `--enable` dans #104. Sans accent parce qu'aucun verbe du protocole n'en
//!   porte, et qu'un `é` sur une ligne tapée à la main dans un `socat` est une
//!   source de refus que personne ne saurait lire.
//! - **Un seul argument, la source, et c'est un jeton unique.** Un nom de source
//!   vient du fichier `name` d'un `hwmon` — « kraken2023elite », « nzxtsmart2 » —,
//!   jamais de ce qu'on tape : il ne porte pas d'espace, et le protocole le
//!   transporte déjà en préfixe de chaque `temp` et de chaque `chan` de `status`,
//!   où un blanc casserait la ligne depuis #17.
//! - **La règle du dernier champ ne s'applique donc PAS**, et c'est l'arbitrage
//!   inverse de `profil load <nom>` (#74) et de `screen image <chemin>` (#33). Là,
//!   le nom est **écrit par l'utilisateur** et peut porter espaces et accents —
//!   « soirée d'été » —, donc il va jusqu'au bout de la ligne. Ici, il est **recopié
//!   d'une réponse `status`**, et un jeton de trop est bien plus probablement un
//!   second argument mal compris — `repare kraken2023elite maintenant`, `repare all
//!   kraken2023elite` — qu'un nom à espaces. Avaler la fin de ligne ferait alors
//!   refuser une source « kraken2023elite maintenant » qui n'a jamais existé, là où
//!   le refus doit dire qu'il y a un jeton de trop.
//! - **Aucun mot d'action après la source.** `fan <canal> pwm 50`, `regule <canal>
//!   on`, `curve <canal> set …` en portent un parce qu'ils font chacun plusieurs
//!   choses au même canal. `repare` n'en fait qu'une, et elle est déjà dans le
//!   verbe. Un `repare <source> now` obligatoire serait un jeton qui ne distingue
//!   rien — le contraire exact du `set` de #104, écrit justement parce qu'il
//!   s'oppose à `enable`.
//!
//! # Ce que ce fichier laisse au démon, et pourquoi
//!
//! - **La source inconnue**, et la liste des sources connues qui doit accompagner
//!   son refus. Seul le démon a cette liste, découverte dans sysfs ; le protocole ne
//!   peut que transporter le nom intact pour que le refus le nomme.
//! - **La source partiellement vivante**, dont #136 exige le refus : c'est un fait
//!   relevé sur le matériel, pas une propriété de la ligne. Une ligne `repare` qui
//!   vise une source en pleine forme est parfaitement bien formée.
//! - **Le compte rendu de ce qui s'est passé.** Ce fichier n'invente aucune ligne de
//!   réponse : l'issue n'en décrit pas la forme, et le geste dure deux délais de
//!   trente secondes — soit bien plus qu'une requête. Le protocole transporte, le
//!   démon juge.
//!
//! Aucun accès matériel, aucune IO, aucun socket, aucun fichier : `reverb-proto` est
//! pur, ses tests aussi. **Aucun périphérique n'est réinitialisé ici** — ce fichier
//! ne parle que de texte.

use reverb_proto::ipc::{MAX_LINE_LEN, Request, RequestError, encode_request, parse_request};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// La source de l'issue : le Kraken, nommé comme son pilote `hwmon`.
const KRAKEN: &str = "kraken2023elite";

/// L'autre source de la machine, celle qui a continué de répondre pendant les trois
/// incidents.
const SMART2: &str = "nzxtsmart2";

/// Une source qui existe, mais que rien ne saurait réparer : le pilote du CPU ne
/// tient aucun nœud USB. Le protocole doit la transporter quand même — c'est le
/// démon qui refusera, en la nommant.
const CPU: &str = "k10temp";

/// Un nom qui ne désigne rien. Il doit traverser intact pour que le refus le
/// répète : « aucune source ne s'appelle ainsi » n'aide personne si le nom a été
/// tronqué en chemin.
const INCONNUE: &str = "kraken2023";

fn repare(source: &str) -> Request {
    Request::Repare {
        source: source.to_owned(),
    }
}

/// Le nom de source d'une requête `repare`, ou un échec qui dit ce qu'on a reçu à la
/// place.
fn source_de(ligne: &str) -> String {
    match parse_request(ligne) {
        Ok(Request::Repare { source }) => source,
        Ok(autre) => {
            panic!("« {ligne} » devait être une commande `repare`, elle a rendu {autre:?}")
        }
        Err(erreur) => panic!("« {ligne} » devait être acceptée : {erreur}"),
    }
}

/// Le verbe et la raison d'un refus d'arguments, ou un échec qui dit ce qui est
/// arrivé.
///
/// Même idiome que `spec_ipc_ecran.rs` (#33) et `spec_104_courbe.rs` (#104) : un
/// verbe **connu** — donc `BadArgument` et non `UnknownVerb` — et une raison non
/// vide. Le texte de la raison n'est jamais figé, seulement ce qu'il doit nommer.
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

/// Le refus d'une commande de réparation : le verbe nommé doit être `repare`.
///
/// Le verbe reçu est `repare`, il existe : ce sont ses arguments qui sont mauvais.
/// Nommer autre chose enverrait chercher une faute de frappe là où il n'y en a pas.
fn refus_de_repare(ligne: &str) -> String {
    let (verbe, raison) = refus(ligne);
    assert_eq!(
        verbe, "repare",
        "« {ligne} » : le verbe reçu est `repare`, l'erreur doit le nommer"
    );
    raison
}

/// Noms de source **hostiles** : ceux qui, mal encodés, scinderaient la commande en
/// deux ou la feraient passer pour la fin d'une réponse.
///
/// La liste reprend celle de `spec_ipc_ecran.rs` et de `spec_104_courbe.rs`, dont la
/// raison vaut ici encore — et elle pèse plus lourd sur ce verbe-là que sur les
/// autres : la seconde moitié d'une commande scindée serait lue comme une requête à
/// part entière, et ce qu'on demande ici fait **disparaître un périphérique du bus**.
///
/// Le nom **vide** n'y figure pas : ce qu'il devient sur le fil est traité à part,
/// par `une_source_vide_ne_fait_pas_dire_autre_chose_a_la_ligne`.
const SOURCES_HOSTILES: &[&str] = &[
    "err",
    "end",
    "error",
    "endpoint",
    "ERR",
    "boom\nend",
    "\nend",
    "end\n",
    "a\r\nend",
    "ma source",
    "\t",
    "  ",
    "a\u{0}b",
    "repare",
    "status",
    "kraken2023elite:coolant-temp",
    "\u{feff}a",
];

// ---------------------------------------------------------------------------
// 1 — un nom de source fait l'aller-retour sans rien perdre
// ---------------------------------------------------------------------------

#[test]
fn une_demande_de_reparation_fait_l_aller_retour_sans_rien_perdre() {
    // issue #136, critère d'acceptation — « `repare <source>` sur le socket lance
    // les trois tentatives bornées et rend un compte rendu de ce qui s'est passé. »
    //
    // Rien de tout cela n'est possible si la ligne ne fait pas l'aller-retour. Les
    // deux sens comptent autant l'un que l'autre : sans le réencodage exact, un
    // encodeur qui écrirait sa source autrement passerait un test de décodage
    // indulgent sans qu'on le voie, et une fenêtre d'une version ultérieure ne
    // saurait plus le lire. C'est aussi ce qui rend « `reverb repare` passe par le
    // démon quand il tourne, comme `screen` et `curve` » possible sans deux
    // vocabulaires à tenir.
    for source in [KRAKEN, SMART2, CPU, INCONNUE] {
        let requete = repare(source);
        let encodee = encode_request(&requete);

        assert_eq!(
            encodee.lines().count(),
            1,
            "une requête tient sur une seule ligne : « {encodee} »"
        );
        assert!(
            encodee.len() <= MAX_LINE_LEN,
            "« {encodee} » fait {} octets, au-delà des {MAX_LINE_LEN} du protocole",
            encodee.len()
        );
        assert_eq!(
            parse_request(&encodee),
            Ok(requete.clone()),
            "aller-retour exact de {requete:?} par « {encodee} »"
        );
    }
}

#[test]
fn la_forme_de_la_ligne_repare_est_figee() {
    // La ligne que l'issue écrit, recopiée telle quelle :
    //
    //     echo 'repare kraken2023elite' | socat - UNIX-CONNECT:/run/reverb/reverbd.sock
    //
    // Une chaîne littérale plutôt qu'une reconstruction : une comparaison qui
    // recalcule le format ne peut pas détecter un changement de format. Ce que ce
    // test protège est la dérive silencieuse — deux versions de Reverb qui
    // n'écrivent pas la même ligne pour le même geste. `tools/installe.sh` met le
    // démon et la fenêtre à jour ensemble, mais rien ne garantit qu'un `reverb` neuf
    // ne parlera jamais à un `reverbd` de la veille.
    assert_eq!(
        encode_request(&repare(KRAKEN)),
        "repare kraken2023elite",
        "la forme du fil ne doit pas dériver de la ligne que l'issue publie"
    );

    // Et elle se relit en la requête d'où elle vient : figer une forme que l'analyse
    // ne saurait pas relire ne prouverait rien.
    assert_eq!(parse_request("repare kraken2023elite"), Ok(repare(KRAKEN)));
}

#[test]
fn la_ligne_repare_compte_exactement_deux_jetons() {
    // Corollaire de la grammaire choisie (en-tête) : le verbe, puis la source, et
    // rien d'autre. Un mot d'action glissé entre les deux — par symétrie avec `fan
    // <canal> pwm`, `regule <canal> on` ou `curve <canal> set` — ne distinguerait
    // rien : `repare` ne fait qu'une chose.
    let encodee = encode_request(&repare(KRAKEN));
    let jetons: Vec<&str> = encodee.split(' ').collect();

    assert_eq!(jetons.len(), 2, "deux jetons attendus : « {encodee} »");
    assert_eq!(jetons[0], "repare");
    assert_eq!(jetons[1], KRAKEN, "la source est le second jeton");
}

// ---------------------------------------------------------------------------
// 2 — le nom traverse intact, pour que le refus le nomme
// ---------------------------------------------------------------------------

#[test]
fn le_nom_de_la_source_traverse_intact_pour_que_le_refus_le_nomme() {
    // issue #136, critère d'acceptation — « `repare` sur une source inconnue est
    // refusé **en listant les sources connues**. »
    //
    // Le protocole ne peut pas savoir quelles sources existent : seul le démon a la
    // liste, découverte dans sysfs. Ce qu'il doit garantir, c'est que le nom arrive
    // **intact** jusqu'à lui — un nom tronqué au premier tiret, mis en minuscules ou
    // coupé sur son `:` ferait refuser une source qui existe, ou nommer la mauvaise
    // dans le refus. Or le refus se lit pour taper la bonne commande derrière.
    for nom in [
        KRAKEN,
        SMART2,
        CPU,
        INCONNUE,
        // Une cible plutôt qu'une source : la faute de frappe la plus probable, et
        // celle que le démon doit pouvoir répéter mot pour mot pour être utile.
        "kraken2023elite:coolant-temp",
        "nvme-Samsung_SSD_990_PRO",
        "UNE-SOURCE-EN-CAPITALES",
        "source_avec_underscores",
        "source.avec.points",
        "2019",
    ] {
        let relu = source_de(&encode_request(&repare(nom)));
        assert_eq!(relu, nom, "le nom de la source traverse sans être retouché");
    }
}

// ---------------------------------------------------------------------------
// 3 — une ligne encodée reste une seule ligne
// ---------------------------------------------------------------------------

#[test]
fn aucune_ligne_repare_ne_porte_de_saut_de_ligne_ni_de_caractere_de_controle() {
    // En-tête du module `ipc` de #17 : « le préfixe de type assure l'invariant pour
    // le début de ligne. Il n'assure rien contre un saut de ligne à l'intérieur d'un
    // champ. »
    //
    // Le protocole est en texte, **une ligne par requête**. Un nom de source portant
    // un saut de ligne scinderait la commande en deux, et la seconde moitié serait
    // lue comme une requête à part entière. Sur ce verbe-là, la conséquence n'est
    // pas une couleur fausse : c'est un `USBDEVFS_RESET` sur un périphérique qu'on
    // n'a pas nommé — le geste que #136 existe pour retirer des mains du démon.
    //
    // Un nom de source vient du matériel : il est lu dans le fichier `name` d'un
    // `hwmon`, donc de rien qu'on écrive nous-même.
    for &hostile in SOURCES_HOSTILES {
        let encodee = encode_request(&repare(hostile));

        assert_eq!(
            encodee.lines().count(),
            1,
            "une requête tient sur une seule ligne, quelle que soit sa source : « {encodee} »"
        );
        assert!(
            !encodee.chars().any(char::is_control),
            "aucun caractère de contrôle dans une requête encodée : {encodee:?}"
        );

        let source = source_de(&encodee);
        assert!(
            !source.is_empty(),
            "une source vide disparaîtrait après le verbe : « {encodee} »"
        );
        assert!(
            !source.chars().any(|c| c.is_whitespace() || c.is_control()),
            "la source {source:?} porte un blanc ou un caractère de contrôle après relecture de \
             « {encodee} » — elle se ferait prendre pour deux champs"
        );
    }
}

#[test]
fn une_ligne_repare_ne_se_fait_jamais_prendre_pour_une_fin_de_reponse() {
    // Règle n° 5 de `spec_ipc.rs` (#17), reprise du côté des requêtes : rien de ce
    // qu'on écrit sur le socket ne doit commencer par `end` ni par `err`, les deux
    // mots qui terminent une réponse. Une source peut porter ces noms — le matériel
    // choisit ses noms, pas nous.
    for &hostile in SOURCES_HOSTILES {
        let encodee = encode_request(&repare(hostile));
        for terminal in ["end", "err"] {
            assert!(
                !encodee.starts_with(terminal),
                "« {encodee} » commence par « {terminal} »"
            );
        }
    }
}

#[test]
fn une_source_vide_ne_fait_pas_dire_autre_chose_a_la_ligne() {
    // Le champ vide est le seul hostile qui n'ait aucun caractère à neutraliser. Ce
    // que le contrat exige n'est donc pas qu'il traverse, mais qu'il ne **décale
    // rien** : une ligne dont la source se serait évaporée ne doit jamais se relire
    // en une commande plausible visant autre chose.
    //
    // Refuser à la relecture est un résultat parfaitement acceptable ; se relire en
    // une réparation d'une source qu'on n'a pas nommée ne l'est pas — c'est un reset
    // USB au hasard.
    let encodee = encode_request(&repare(""));
    match parse_request(&encodee) {
        Err(_) => {}
        Ok(Request::Repare { source }) => {
            assert!(
                !source.is_empty(),
                "« {encodee} » se relit sur une source vide"
            );
        }
        Ok(autre) => {
            panic!("« {encodee} » se relit en {autre:?} — la source vide a changé de verbe")
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — les refus, en nommant ce qui cloche
// ---------------------------------------------------------------------------

#[test]
fn une_source_omise_est_refusee_en_nommant_repare() {
    // La ligne compte deux jetons, et aucun n'est facultatif. Une ligne plus courte
    // n'est pas une commande partielle : c'est une commande cassée. La laisser
    // passer en devinant la source manquante — « il n'y en a qu'une qui se taise » —
    // ferait réinitialiser un périphérique que personne n'a nommé, et ce serait
    // exactement le déclenchement automatique que #136 retire.
    for ligne in ["repare", "repare ", "repare  ", "repare\t", "repare \t "] {
        refus_de_repare(ligne);
    }
}

#[test]
fn un_jeton_de_trop_est_refuse_plutot_qu_avale() {
    // L'arbitrage de l'en-tête, et il est l'inverse de celui des profils (#74) et
    // des chemins d'image (#33) : un nom de source **n'est pas écrit par
    // l'utilisateur**, il est recopié d'une réponse `status`, et il ne porte pas
    // d'espace — le protocole le transporte déjà en préfixe de chaque ligne `temp`
    // et `chan`, où un blanc casserait la ligne depuis #17.
    //
    // Un second jeton est donc un argument mal compris, jamais la suite d'un nom.
    // L'avaler ferait refuser une source « kraken2023elite maintenant » qui n'a
    // jamais existé, avec un message qui enverrait chercher une faute de frappe dans
    // le nom au lieu de dire qu'il y a un mot de trop.
    for ligne in [
        "repare kraken2023elite maintenant",
        "repare kraken2023elite now",
        "repare kraken2023elite on",
        "repare kraken2023elite reset",
        "repare all kraken2023elite",
        "repare kraken2023elite nzxtsmart2",
        "repare kraken2023elite BB8C90820E900630",
        "repare kraken2023elite 1e71:300c",
    ] {
        refus_de_repare(ligne);
    }
}

#[test]
fn deux_fautes_differentes_ne_rendent_pas_la_meme_phrase() {
    // Un refus qui dit toujours la même chose n'aide personne à corriger sa ligne.
    // Les deux fautes que la grammaire rend possibles — la source manquante et le
    // jeton de trop — doivent se distinguer dans le message, sans quoi la seconde
    // enverrait chercher une source qu'on a pourtant écrite.
    let manquante = refus_de_repare("repare");
    let en_trop = refus_de_repare("repare kraken2023elite maintenant");

    assert_ne!(
        manquante, en_trop,
        "« une source manque » et « un jeton de trop » sont deux fautes opposées : les dire de la \
         même façon enverrait corriger l'inverse de ce qui cloche"
    );
}

#[test]
fn une_ligne_repare_trop_longue_est_refusee_pour_sa_longueur() {
    // #17 — la longueur est vérifiée **avant** tout découpage, et l'erreur ne porte
    // que la longueur. Un nom de source légitime tient très largement dans les
    // 1024 octets du protocole ; ce qui peut déborder, c'est un nom aberrant, et le
    // refus doit alors parler de longueur plutôt que de source.
    let source = "s".repeat(MAX_LINE_LEN);
    let trop_longue = format!("repare {source}");
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

// ---------------------------------------------------------------------------
// 5 — la cohabitation avec le protocole d'avant
// ---------------------------------------------------------------------------

#[test]
fn un_verbe_voisin_reste_refuse_en_le_nommant() {
    // `repare` est un **premier mot neuf** : un démon d'avant #136 ne peut que
    // répondre « commande inconnue » en la nommant, jamais lire la ligne de travers.
    //
    // Le cas se produit pour de bon : `reverb` et `reverbd` sont deux binaires
    // installés séparément, et `reverb repare` passe par le socket dès que le démon
    // tourne. Les orthographes essayées ici sont celles qu'on tape vraiment — la
    // version accentuée en tête, puisque le verbe est en français.
    for verbe in [
        "réparer", "reparer", "repair", "repares", "Repare", "REPARE", "rep", "reset",
    ] {
        match parse_request(&format!("{verbe} {KRAKEN}")) {
            Err(RequestError::UnknownVerb { verb }) => assert_eq!(
                verb, verbe,
                "l'erreur nomme le verbe reçu, pour qu'on voie lequel n'est pas compris"
            ),
            autre => panic!("« {verbe} » doit donner un UnknownVerb, pas {autre:?}"),
        }
    }
}

#[test]
fn les_verbes_d_avant_la_reparation_traversent_toujours() {
    // Non-régression, comme #33, #50 et #104 en ont écrit une : le verbe ajouté ne
    // doit rien casser de ce que les clients installés envoient déjà. La fenêtre
    // demande `status` une fois par seconde, et `reverb` a dix autres verbes.
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
            "« {ligne} » marchait avant #136 et doit continuer"
        );
        assert!(
            !matches!(parse_request(ligne), Ok(Request::Repare { .. })),
            "« {ligne} » ne doit pas se relire en une demande de réparation"
        );
    }
}

#[test]
fn repare_ne_se_confond_avec_aucun_autre_verbe() {
    // Le protocole a maintenant trois verbes qui prennent un nom de matériel en
    // second jeton — `fan <canal> …`, `regule <canal> …`, `curve <canal> …` — et un
    // quatrième qui prend un nom de **source**. Les confondre ne produirait aucun
    // message : `repare` sur un canal viserait une source qui n'existe pas, et le
    // démon la refuserait en listant des noms qui ne ressemblent pas à ce qu'on a
    // tapé.
    //
    // Ce que ce test fige, c'est que la ligne `repare` ne prend la grammaire de
    // personne d'autre, et que personne ne prend la sienne.
    let demande = parse_request(&format!("repare {KRAKEN}")).expect("la commande de #136 passe");
    assert!(
        matches!(demande, Request::Repare { .. }),
        "« repare {KRAKEN} » est une demande de réparation, elle a rendu {demande:?}"
    );

    // La grammaire des autres ne s'y invite pas.
    for ligne in [
        format!("repare {KRAKEN} on"),
        format!("repare {KRAKEN} off"),
        format!("repare {KRAKEN} auto"),
        format!("repare {KRAKEN} pwm 50"),
        format!("repare {KRAKEN} set 30,60,100"),
        "repare courbe 35000:30".to_owned(),
    ] {
        assert!(
            parse_request(&ligne).is_err(),
            "« {ligne} » n'est pas une grammaire du protocole"
        );
    }

    // ⚠️ En revanche `repare list` et `repare all` sont des lignes **bien formées** :
    // ce sont deux jetons, donc deux demandes de réparation visant des sources qui
    // s'appelleraient « list » et « all ». Leur donner un sens ici — lister,
    // réparer tout — serait inventer une grammaire que l'issue ne demande pas, et
    // « réparer tout » est précisément le geste en masse que #136 retire. C'est le
    // démon qui les refusera, en listant les sources connues.
    for mot in ["list", "all", "off", "status"] {
        assert_eq!(
            parse_request(&format!("repare {mot}")),
            Ok(repare(mot)),
            "« repare {mot} » vise une source nommée « {mot} » — c'est au démon de dire qu'elle \
             n'existe pas, pas au protocole d'inventer un sens"
        );
    }

    // Et la sienne ne déteint sur personne : `repare` est un verbe, pas un mot
    // d'action qu'on pourrait glisser après un canal.
    for ligne in [
        "fan nzxtsmart2:fan-1 repare",
        "regule nzxtsmart2:fan-1 repare",
        "screen repare",
        "zone repare",
    ] {
        assert!(
            parse_request(ligne).is_err(),
            "« {ligne} » n'est pas une grammaire du protocole"
        );
    }
}
