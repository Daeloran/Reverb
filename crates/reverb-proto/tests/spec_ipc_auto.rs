//! Tests d'intention du champ « sait faire auto » du protocole (issue #50).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #50 seule. Aucune ligne
//! n'est relue depuis `crates/reverb-proto/src/` : à l'écriture de ce fichier,
//! `ResponseLine::Channel` ne porte pas ce champ. Ils encodent ce que le
//! protocole doit faire, pas ce que le code fait — si l'un d'eux échoue après
//! implémentation, c'est le code qu'on corrige, jamais le test.
//!
//! Ils prolongent `spec_ipc.rs` (#17), `spec_ipc_v2.rs` (#19), `spec_ipc_v3.rs`
//! (#23), `spec_ipc_ecran.rs` (#33) et `spec_ipc_zones.rs` (#29), auxquels
//! **ce fichier ne touche pas** : ce qui y est écrit doit continuer de passer
//! tel quel.
//!
//! # Pourquoi le protocole doit porter ce champ
//!
//! issue #50, approche technique — « Côté fenêtre, `ResponseLine::Channel` ne
//! porte pas encore "ce canal sait-il faire auto". Le protocole gagne le
//! champ ; la fenêtre s'en sert pour montrer ou cacher le bouton. »
//!
//! La fenêtre n'ouvre aucun périphérique (ADR-002) : elle ne peut donc pas lire
//! le nom du pilote elle-même. Sans ce champ, elle n'a que deux choix — montrer
//! un bouton qui ne peut qu'échouer, ou le cacher partout. Les deux sont faux.
//!
//! Les deux sources noyau qui fondent la distinction, citées par l'issue :
//!
//! **`nzxt-smart2`**, `set_pwm_enable()` — pas de mode automatique du tout :
//!
//! ```c
//! expected_val = drvdata->fan_type[channel] != FAN_TYPE_NONE;
//! return (val == expected_val) ? 0 : -EOPNOTSUPP;
//! ```
//!
//! **`nzxt-kraken3`**, `kraken3_write` — `0` n'est pas « laissé au firmware » :
//!
//! ```c
//! case 0:
//!     /* Set channel to 100%, direct duty value */
//!     ret = kraken3_write_fixed_duty(priv, 255, channel);
//! ```
//!
//! # Ce que ce fichier fige
//!
//! 1. `ResponseLine::Channel` porte `sait_faire_auto: bool`, et il traverse
//!    l'encodage et le décodage **dans les deux sens**.
//! 2. Une ligne `chan` produite par un démon plus ancien, **sans** ce champ, se
//!    relit encore sans erreur.
//!
//! # Les choix de forme, et pourquoi
//!
//! L'issue dit « le protocole gagne le champ » et rien de plus. Ce fichier
//! tranche, puisque c'est lui le contrat :
//!
//! - **Le champ s'appelle `sait_faire_auto`**, du même nom que la méthode de
//!   `reverb-hw` qui le produit. Un nom court comme `auto` se lirait « ce canal
//!   *est* en auto », qui est une autre question — celle que `mode` porte déjà.
//! - **Il s'écrit `oui` ou `non`**, et c'est le **dernier** champ de la ligne :
//!   `chan <canal> <position> <rpm> <pwm> <mode> <oui|non>`. Le dernier, parce
//!   que c'est la seule position qui laisse une ligne ancienne relisible. `oui`
//!   et `non` plutôt que `-` parce que `-` veut déjà dire « absent » dans cette
//!   grammaire (#17), et qu'un booléen n'est pas une absence ; plutôt que `0`
//!   et `1` parce qu'ils voisineraient avec `rpm` et `pwm`, où un chiffre a un
//!   tout autre sens.
//! - **L'absence du champ vaut « ne sait pas faire auto ».** C'est le repli
//!   conservateur : un démon d'avant #50 ne sait rien de la question, et la
//!   fenêtre doit alors cacher le bouton plutôt que d'en montrer un qui ne peut
//!   qu'échouer — c'est exactement la panne que l'issue corrige.
//! - **Un septième jeton qui n'est ni `oui` ni `non` est refusé**, plutôt
//!   qu'absorbé dans le mode. Absorber ferait passer une incompatibilité de
//!   protocole pour un mode exotique, et l'erreur ne se verrait jamais.
//! - **Le mode reste un jeton unique.** Il l'était déjà de fait — la grammaire
//!   de #17 ne le prend « jusqu'à la fin de la ligne » nulle part —, et c'est ce
//!   qui permet de distinguer six jetons de sept.
//!
//! Aucun accès matériel, aucune IO, aucun socket : `reverb-proto` est pur, ses
//! tests aussi.

use reverb_proto::Position;
use reverb_proto::ipc::{ResponseError, ResponseLine, encode_response_line, parse_response_line};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Le canal `nzxtsmart2` du dialogue d'exemple de #17 — celui qui **ne sait
/// pas** faire auto, d'après `nzxt-smart2`.
const SANS_AUTO: &str = "nzxtsmart2:fan-1";

/// Un canal du Kraken — celui qui **sait**, d'après `nzxt-kraken3`.
const AVEC_AUTO: &str = "kraken2023elite:fan-speed";

/// Une ligne `chan` complète, telle que le dialogue d'exemple de #17 l'écrit,
/// augmentée du champ de #50.
fn canal(channel: &str, mode: &str, sait_faire_auto: bool) -> ResponseLine {
    ResponseLine::Channel {
        channel: channel.to_owned(),
        position: Some(Position::RadiateurHaut),
        rpm: Some(1200),
        pwm: Some(60),
        mode: mode.to_owned(),
        sait_faire_auto,
        // #113 : la ligne gagne un dernier jeton. `false` ici parce que ce
        // fichier fige la place du drapeau de #50, pas la capacité d'un
        // contrôleur donné.
        regulable: false,
    }
}

/// Une ligne `chan` dont tous les champs facultatifs sont absents.
fn canal_muet(channel: &str, mode: &str, sait_faire_auto: bool) -> ResponseLine {
    ResponseLine::Channel {
        channel: channel.to_owned(),
        position: None,
        rpm: None,
        pwm: None,
        mode: mode.to_owned(),
        sait_faire_auto,
        regulable: false,
    }
}

/// Noms hostiles de #17 : ceux qui, mal encodés, produiraient une ligne qu'un
/// client prendrait pour la fin de la réponse.
///
/// Un nom de canal vient du matériel, pas de nous. Le champ ajouté ici ne doit
/// pas rouvrir la brèche que #17 a fermée.
const NOMS_HOSTILES: &[&str] = &["err", "end", "error", "ends", "ERR", "END", "e", "erreur"];

/// Vérifie qu'une ligne de données ne peut pas se faire prendre pour une fin de
/// réponse, et qu'elle survit à l'aller-retour.
///
/// Reprise de la propriété centrale de #17 — « une ligne de données ne commence
/// **jamais** par `end` ni par `err` » — appliquée aux lignes qui portent
/// désormais un champ de plus.
fn ne_termine_jamais_et_revient_intacte(donnee: &ResponseLine) {
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
    for terminal in ["end", "err"] {
        assert!(
            !encodee.starts_with(terminal),
            "« {encodee} » commence par « {terminal} » — un client y verrait la fin de la réponse"
        );
    }

    let relue = parse_response_line(&encodee).expect("une ligne encodée se relit");
    assert!(
        !relue.is_terminal(),
        "« {encodee} » se relit en {relue:?}, qui terminerait la réponse"
    );
    assert_eq!(&relue, donnee, "aller-retour exact : « {encodee} »");
}

/// Lit une réponse comme le ferait un client : ligne par ligne, jusqu'à la
/// première ligne terminale, **sans compter les lignes**.
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

// ---------------------------------------------------------------------------
// 1 — le champ traverse l'encodage et le décodage dans les deux sens
// ---------------------------------------------------------------------------

#[test]
fn le_champ_s_ecrit_oui_ou_non_en_dernier_et_se_relit() {
    // issue #50, tests d'intention n° 5 — « Le champ "sait faire auto" traverse
    // le protocole IPC dans les deux sens. »
    //
    // Les deux sens comptent autant l'un que l'autre : sans le réencodage
    // exact, un encodeur qui écrirait `chan … manual 1` passerait un test de
    // décodage indulgent sans qu'on le voie, et la fenêtre d'une version
    // ultérieure ne saurait plus le lire.
    //
    // ⚠️ **Les quatre lignes attendues ont gagné un jeton en #113**, qui ajoute
    // `regulable` à la fin de la grammaire — exactement comme #50 avait ajouté
    // le sien à la fin de celle de #17. Le drapeau de ce fichier reste à sa
    // place, sixième champ, et c'est bien ce que la ligne vérifie ; ce qui a
    // changé, c'est ce qui vient **après** lui.
    let cas: [(ResponseLine, &str); 4] = [
        (
            canal(AVEC_AUTO, "manual", true),
            "chan kraken2023elite:fan-speed radiateur-haut 1200 60 manual oui non",
        ),
        (
            canal(SANS_AUTO, "manual", false),
            "chan nzxtsmart2:fan-1 radiateur-haut 1200 60 manual non non",
        ),
        (
            canal_muet("kraken2023elite:pump-speed", "firmware", true),
            "chan kraken2023elite:pump-speed - - - firmware oui non",
        ),
        (
            canal_muet(SANS_AUTO, "firmware", false),
            "chan nzxtsmart2:fan-1 - - - firmware non non",
        ),
    ];

    for (ligne, texte) in cas {
        assert_eq!(
            encode_response_line(&ligne),
            texte,
            "encodage de {ligne:?} — la forme du fil ne doit pas dériver"
        );
        assert_eq!(
            parse_response_line(texte),
            Ok(ligne),
            "décodage de « {texte} »"
        );
    }
}

#[test]
fn le_champ_est_le_dernier_jeton_de_la_ligne() {
    // C'est la condition qui rend une ligne ancienne relisible : un champ
    // inséré au milieu décalerait tout ce qui suit, et une ligne de démon d'hier
    // se décoderait en un canal plausible et faux — un `pwm` lu comme un `rpm`.
    //
    // La ligne `chan` comptait donc sept jetons, et le nouveau était le
    // septième.
    //
    // ⚠️ **#113 en a ajouté un huitième, `regulable`, et pour la même raison.**
    // Ce que ce test protège est intact : le drapeau de #50 reste le **sixième
    // champ**, donc rien de ce qui le précède n'a bougé, donc une ligne de démon
    // d'hier se décode toujours en elle-même — c'est cette propriété-là qui
    // rendait « le dernier » souhaitable, et non la place elle-même. Un champ
    // inséré **au milieu** décalerait tout ce qui suit et ferait lire un `pwm`
    // comme un `rpm` ; c'est ce que les deux dernières assertions interdisent
    // toujours.
    for (ligne, attendu) in [
        (canal(AVEC_AUTO, "manual", true), "oui"),
        (canal(SANS_AUTO, "manual", false), "non"),
    ] {
        let encodee = encode_response_line(&ligne);
        let jetons: Vec<&str> = encodee.split(' ').collect();

        assert_eq!(jetons.len(), 8, "huit jetons attendus : « {encodee} »");
        assert_eq!(jetons[0], "chan");
        assert_eq!(
            jetons[6], attendu,
            "le champ reste le sixième champ, à la fin de la grammaire de #50 : « {encodee} »"
        );
        assert!(
            encodee.ends_with(&format!(" {attendu} non")),
            "« {encodee} » doit porter « {attendu} » juste avant le jeton ajouté par #113"
        );
    }
}

#[test]
fn deux_canaux_qui_ne_different_que_par_ce_champ_ne_se_confondent_pas() {
    // Sans ce test, un encodeur qui laisserait tomber le champ passerait tous
    // les décodages — puisque l'absence se relit en `false` — et la fenêtre
    // cacherait le bouton du Kraken pour toujours. C'est le mode de défaillance
    // le plus discret du lot : aucune erreur, juste un bouton qui ne revient
    // jamais.
    let sait = canal(AVEC_AUTO, "manual", true);
    let ne_sait_pas = canal(AVEC_AUTO, "manual", false);

    assert_ne!(sait, ne_sait_pas, "les deux valeurs sont distinctes");
    assert_ne!(
        encode_response_line(&sait),
        encode_response_line(&ne_sait_pas),
        "et leurs deux lignes le sont aussi"
    );
    assert_eq!(parse_response_line(&encode_response_line(&sait)), Ok(sait));
    assert_eq!(
        parse_response_line(&encode_response_line(&ne_sait_pas)),
        Ok(ne_sait_pas)
    );
}

#[test]
fn les_champs_absents_restent_des_tirets_a_cote_du_nouveau_champ() {
    // #17, test n° 8 — « un champ absent s'écrit `-` […], jamais `0` », et « `-`
    // se relit en `None`, jamais en `Some(0)` ». Le champ ajouté ne doit pas
    // faire réécrire l'encodeur d'une façon qui perde cette distinction :
    // `unwrap_or_default()` reste plus court à écrire que le cas absent.
    let muet = canal_muet(AVEC_AUTO, "firmware", true);
    let encodee = encode_response_line(&muet);
    assert_eq!(
        encodee,
        "chan kraken2023elite:fan-speed - - - firmware oui non"
    );

    let relue = parse_response_line(&encodee).expect("ligne bien formée");
    let ResponseLine::Channel {
        position,
        rpm,
        pwm,
        sait_faire_auto,
        ..
    } = &relue
    else {
        panic!("« chan … » doit se décoder en Channel, pas en {relue:?}");
    };
    assert_eq!(*position, None, "`-` n'est pas une position");
    assert_eq!(*rpm, None, "`-` n'est pas 0 tr/min");
    assert_eq!(*pwm, None, "`-` n'est pas 0 %");
    assert!(*sait_faire_auto);

    // Et zéro reste zéro, à côté du nouveau champ : les deux se distinguent
    // toujours dans les deux sens.
    let arrete = ResponseLine::Channel {
        channel: "nzxtsmart2:fan-2".to_owned(),
        position: Some(Position::BasMilieu),
        rpm: Some(0),
        pwm: Some(0),
        mode: "manual".to_owned(),
        sait_faire_auto: false,
        regulable: false,
    };
    let encodee = encode_response_line(&arrete);
    assert_eq!(
        encodee,
        "chan nzxtsmart2:fan-2 bas-milieu 0 0 manual non non"
    );
    assert_eq!(parse_response_line(&encodee), Ok(arrete));
}

#[test]
fn le_nouveau_champ_ne_rouvre_pas_la_breche_du_cadrage() {
    // #17, test n° 5 — « Une ligne de données ne commence **jamais** par `end`
    // ni par `err` — c'est ce qui rend la fin de réponse non ambiguë sans
    // compter les lignes. » C'est le seul garde-fou du cadre : un champ ajouté
    // ne doit pas le desserrer, et un nom de canal vient du matériel.
    for &nom in NOMS_HOSTILES {
        for sait in [false, true] {
            ne_termine_jamais_et_revient_intacte(&canal(nom, "manual", sait));
            ne_termine_jamais_et_revient_intacte(&canal(SANS_AUTO, nom, sait));
            ne_termine_jamais_et_revient_intacte(&canal_muet(nom, nom, sait));
        }
    }
}

#[test]
fn une_reponse_entiere_se_lit_encore_sans_compter_les_lignes() {
    // La démonstration de bout en bout, comme #17 la joue : un client lit
    // jusqu'à la première ligne terminale et retrouve **exactement** ses lignes
    // de données. Les cinq canaux du relevé y figurent avec leurs vraies
    // réponses — deux Kraken qui savent, trois `nzxtsmart2` qui ne savent pas.
    let donnees = [
        canal("nzxtsmart2:fan-1", "manual", false),
        canal("nzxtsmart2:fan-2", "manual", false),
        canal("nzxtsmart2:fan-3", "manual", false),
        canal_muet("kraken2023elite:pump-speed", "plein-regime", true),
        canal("kraken2023elite:fan-speed", "plein-regime", true),
    ];

    let mut lignes: Vec<String> = donnees.iter().map(encode_response_line).collect();
    lignes.push(encode_response_line(&ResponseLine::End));
    let flux = lignes.join("\n");

    let (lues, terminale) = lit_une_reponse(&flux);
    assert_eq!(
        lues[..],
        donnees[..],
        "les lignes de données doivent revenir intactes, dans l'ordre"
    );
    assert_eq!(terminale, ResponseLine::End);
}

// ---------------------------------------------------------------------------
// 2 — une ligne d'un démon plus ancien se relit encore
// ---------------------------------------------------------------------------

#[test]
fn une_ligne_chan_sans_le_champ_se_relit_encore() {
    // issue #50, tests d'intention n° 6 — « Une ligne `chan` d'un démon plus
    // ancien, sans ce champ, se relit encore. »
    //
    // Le cas se produit pour de bon : le démon est un service système et la
    // fenêtre un binaire installé à côté ; `tools/installe.sh` les met à jour
    // ensemble, mais rien ne garantit qu'un `reverb-gui` neuf ne parlera jamais
    // à un `reverbd` d'hier. Refuser la ligne ferait disparaître **tous** les
    // canaux de la fenêtre, pour un champ qui ne sert qu'à cacher un bouton.
    //
    // Les deux formes du dialogue d'exemple de #17, mot pour mot.
    for (ancienne, attendue) in [
        (
            "chan nzxtsmart2:fan-1 radiateur-haut 1200 60 manual",
            canal(SANS_AUTO, "manual", false),
        ),
        (
            "chan kraken2023elite:pump-speed - - - firmware",
            canal_muet("kraken2023elite:pump-speed", "firmware", false),
        ),
    ] {
        let relue = parse_response_line(ancienne)
            .unwrap_or_else(|e| panic!("« {ancienne} » doit encore se relire : {e}"));
        assert_eq!(
            relue, attendue,
            "« {ancienne} » : les champs connus sont intacts, et le champ absent vaut « non »"
        );
    }
}

#[test]
fn l_absence_du_champ_vaut_ne_sait_pas_faire_auto() {
    // Arbitrage de ce fichier (en-tête). Le repli est **conservateur** : un
    // démon d'avant #50 ne sait rien de la question, et la fenêtre doit alors
    // cacher le bouton. Répondre « oui » par défaut ressusciterait exactement la
    // panne de l'issue — un bouton qui envoie `0` et récolte un errno nu, ou
    // pire, un canal à 100 %.
    //
    // Dit autrement : la ligne ancienne et la ligne neuve qui finit par « non »
    // décrivent le même canal.
    let ancienne = "chan nzxtsmart2:fan-1 radiateur-haut 1200 60 manual";
    let neuve = "chan nzxtsmart2:fan-1 radiateur-haut 1200 60 manual non";

    assert_eq!(
        parse_response_line(ancienne),
        parse_response_line(neuve),
        "l'absence du champ se lit comme « non »"
    );
    assert_eq!(
        parse_response_line(ancienne),
        Ok(canal(SANS_AUTO, "manual", false))
    );

    // Et le sens interdit : la ligne ancienne ne doit surtout pas se lire
    // « oui ».
    assert_ne!(
        parse_response_line(ancienne),
        Ok(canal(SANS_AUTO, "manual", true)),
        "un démon muet ne promet pas un mode automatique"
    );
}

#[test]
fn un_mode_nomme_oui_ou_non_ne_se_confond_pas_avec_le_champ() {
    // Le mode est une chaîne libre venue du démon : rien n'interdit qu'il
    // s'écrive un jour `oui`. C'est le **nombre de jetons** qui tranche, pas
    // leur contenu — six jetons, pas de champ ; sept, un champ.
    //
    // Même prudence que les noms hostiles de #17 : le protocole doit tenir même
    // quand une valeur ressemble à un mot-clé.
    for (ligne, attendue) in [
        (
            "chan nzxtsmart2:fan-1 - - - oui",
            canal_muet(SANS_AUTO, "oui", false),
        ),
        (
            "chan nzxtsmart2:fan-1 - - - non",
            canal_muet(SANS_AUTO, "non", false),
        ),
        (
            "chan nzxtsmart2:fan-1 - - - oui non",
            canal_muet(SANS_AUTO, "oui", false),
        ),
        (
            "chan nzxtsmart2:fan-1 - - - non oui",
            canal_muet(SANS_AUTO, "non", true),
        ),
    ] {
        assert_eq!(
            parse_response_line(ligne),
            Ok(attendue.clone()),
            "décodage de « {ligne} »"
        );
        // Et le réencodage remet toujours tous les jetons — sept jusqu'à #113,
        // huit depuis : une ligne courte est une entrée tolérée, jamais une
        // sortie.
        let reencodee = encode_response_line(&attendue);
        assert_eq!(reencodee.split(' ').count(), 8, "« {reencodee} »");
        assert_eq!(parse_response_line(&reencodee), Ok(attendue));
    }
}

#[test]
fn un_septieme_jeton_qui_n_est_ni_oui_ni_non_est_refuse() {
    // Arbitrage de ce fichier (en-tête). Absorber le jeton dans le mode ferait
    // passer une incompatibilité de protocole pour un mode exotique, affiché
    // tel quel dans la colonne MODE, et personne ne saurait que le champ n'a pas
    // été compris.
    //
    // `1` et `true` figurent exprès : ce sont les deux écritures qu'un autre
    // encodeur choisirait spontanément. Les accepter reviendrait à avoir deux
    // formes sur le fil, donc deux à tenir.
    for ligne in [
        "chan nzxtsmart2:fan-1 - - - manual peut-etre",
        "chan nzxtsmart2:fan-1 - - - manual 1",
        "chan nzxtsmart2:fan-1 - - - manual 0",
        "chan nzxtsmart2:fan-1 - - - manual true",
        "chan nzxtsmart2:fan-1 - - - manual false",
        "chan nzxtsmart2:fan-1 - - - manual OUI",
        "chan nzxtsmart2:fan-1 - - - manual Non",
        "chan nzxtsmart2:fan-1 - - - manual -",
        // ⚠️ **Cette ligne-ci était « chan … manual oui non » avant #113**, où
        // elle portait un jeton de trop. Elle en est désormais une valide :
        // sept champs, `regulable` en dernier. Ce qu'elle vérifiait — un jeton
        // au-delà de la grammaire est refusé, jamais absorbé — se vérifie donc
        // un cran plus loin, sur le huitième champ.
        "chan nzxtsmart2:fan-1 - - - manual oui non oui",
    ] {
        let erreur: ResponseError = parse_response_line(ligne)
            .expect_err("un champ « sait faire auto » incompréhensible est refusé");
        assert_eq!(erreur.line, ligne, "l'erreur porte la ligne fautive");
        assert!(!erreur.reason.is_empty(), "l'erreur doit dire pourquoi");
        let _: &dyn std::error::Error = &erreur;
    }

    // Et surtout : pas d'absorption silencieuse dans le mode.
    assert_ne!(
        parse_response_line("chan nzxtsmart2:fan-1 - - - manual peut-etre"),
        Ok(canal_muet(SANS_AUTO, "manual peut-etre", false)),
        "le jeton de trop ne se range pas dans le mode"
    );
}

#[test]
fn une_ligne_chan_tronquee_reste_refusee() {
    // Le pendant du test précédent, par l'autre bout : rendre le septième champ
    // facultatif ne doit pas rendre les six premiers facultatifs. Une ligne
    // amputée est une ligne cassée, pas un canal partiel — #17, « ni omise, ni
    // remplacée par zéro ».
    for ligne in [
        "chan",
        "chan nzxtsmart2:fan-1",
        "chan nzxtsmart2:fan-1 -",
        "chan nzxtsmart2:fan-1 - -",
        "chan nzxtsmart2:fan-1 - - -",
    ] {
        assert!(
            parse_response_line(ligne).is_err(),
            "« {ligne} » est amputée, elle doit être refusée"
        );
    }
}
