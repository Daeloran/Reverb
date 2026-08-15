//! Tests d'intention de l'édition et du tracé de la courbe de régulation (issue #113).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #113 et #99, dont elle vient piloter la
//! régulation. Rien de `crates/*/src/` ni de `crates/reverb-gui/ui/` n'a été lu pour les produire.
//! Le contrat de `Courbe` n'est pas redécouvert ici : il est repris tel que
//! `crates/reverb-daemon/tests/spec_regulation_hote.rs` — le fichier de tests d'intention de #99 —
//! l'a figé, à un détail près et il est décisif, voir ci-dessous.
//!
//! À l'écriture de ce fichier, `reverb_proto::regulation`, `ordre_de_courbe` et `trace_de_courbe`
//! **n'existent pas** : la compilation doit échouer sur eux, et sur eux seuls. C'est la phase rouge.
//!
//! Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne touche un bus.
//!
//! ## Pourquoi `Courbe` descend dans `reverb-proto`
//!
//! L'issue l'exige : « ⚠️ Le tracé de la courbe doit venir de la **même** fonction que celle
//! qu'exécute le démon, comme la maquette partage `reverb-anim` avec lui — sinon l'aperçu montrerait
//! une courbe que le boîtier n'applique pas. »
//!
//! Or `Courbe` vit dans `reverb_daemon::regulation`, et **`reverb-gui` ne dépend pas de
//! `reverb-daemon`**. Le tracé serait donc forcément une réimplémentation — c'est-à-dire deux
//! interpolations à tenir d'accord, dont l'une n'aurait aucune raison d'être la bonne.
//!
//! `Courbe` et `CourbeInvalide` descendent donc dans `reverb-proto`, que les deux crates partagent :
//! c'est du calcul pur, sans E/S, `ipc.rs` y encode déjà les paliers, et c'est exactement le remède
//! que #112 vient d'appliquer aux jetons de mode.
//!
//! ```ignore
//! // crates/reverb-proto/src/regulation.rs — l'API de #99, inchangée, seulement déplacée.
//! pub struct Courbe { /* … */ }
//! impl Courbe {
//!     pub fn defaut() -> Courbe;
//!     pub fn depuis(paliers: &[(i32, u8)]) -> Result<Courbe, CourbeInvalide>;
//!     pub fn paliers(&self) -> &[(i32, u8)];
//!     pub fn consigne(&self, milli_degres: i32) -> u8;
//! }
//! pub struct CourbeInvalide { pub raison: String }
//!
//! // crates/reverb-daemon/src/regulation.rs
//! pub use reverb_proto::regulation::{Courbe, CourbeInvalide};
//! ```
//!
//! ⚠️ **La ré-exportation n'est pas une commodité, c'est la preuve.** `spec_regulation_hote.rs`
//! importe `Courbe` depuis `reverb_daemon::regulation` et doit continuer de compiler **sans être
//! touché** : c'est ce qui garantit que les deux crates parlent du même type, et non de deux copies
//! qui divergeraient au premier palier corrigé d'un seul côté.
//!
//! ## Le contrat que ces tests figent, côté fenêtre
//!
//! ```ignore
//! // dans crates/reverb-gui/src/telemetrie.rs, à côté de `LigneCanal`.
//!
//! /// La requête qu'une courbe éditée produit — refusée **en le disant**, avant tout envoi.
//! ///
//! /// Les paliers sont des `(millidegrés, pourcent)`, comme partout depuis #99.
//! pub fn ordre_de_courbe(paliers: &[(i32, u8)]) -> Result<Request, CourbeInvalide>;
//!
//! /// Le tracé d'une courbe : `points` couples `(millidegrés, consigne)` régulièrement répartis
//! /// de `froid` à `chaud`, **calculés par `Courbe::consigne` et par elle seule**.
//! pub fn trace_de_courbe(courbe: &Courbe, froid: i32, chaud: i32, points: usize)
//!     -> Vec<(i32, u8)>;
//! ```
//!
//! ### Pourquoi `ordre_de_courbe` prend des paliers et non une `Courbe`
//!
//! Critère d'acceptation : « une courbe invalide est refusée en le disant, **sans être posée** ».
//! L'éditeur tient des paliers bruts, tapés à la main ; s'il fallait d'abord construire une `Courbe`
//! pour obtenir une requête, le refus vivrait chez l'appelant — c'est-à-dire dans `main.rs`, où il
//! ne se testerait pas. C'est la règle que #100 a posée pour le tri et #102 pour la valeur affichée :
//! ce qui est du calcul vit dans la bibliothèque.
//!
//! ### Pourquoi une fonction de tracé, plutôt qu'une boucle dans le `.slint`
//!
//! Pour la même raison, et parce que l'exigence de l'issue ne se vérifie autrement qu'à l'œil.
//! Une boucle qui appellerait `consigne` dans `main.rs` serait juste — et le resterait par
//! surveillance, pas par test.
//!
//! ## Ce que ces tests ne figent pas, et pourquoi
//!
//! - **Ce que `Courbe::depuis` accepte ou refuse.** C'est #99, et `spec_regulation_hote.rs` le
//!   tient. Ce fichier n'exige qu'une chose : que la fenêtre refuse **exactement** ce que le démon
//!   refuse, et pour la même raison. Une fenêtre plus sévère refuserait une courbe que le socket
//!   accepte ; une fenêtre plus laxiste laisserait partir une ligne que le démon rejettera, et le
//!   refus arriverait après le geste plutôt qu'avant.
//! - ⚠️ **En particulier, un palier unique n'est pas refusé.** `spec_regulation_hote.rs` le fige
//!   accepté — `une_courbe_a_un_seul_palier_est_plate` — et c'est une courbe parfaitement sensée :
//!   une consigne fixe. Le refuser ici ferait diverger les deux bouts du même protocole.
//! - **Des consignes décroissantes.** #99 ne les refuse pas — son test des températures extrêmes
//!   encadre d'ailleurs le résultat par `premiere.min(derniere)..=premiere.max(derniere)`, ce qui
//!   n'a de sens que si une courbe peut descendre. Trancher ici, dans un sens ou dans l'autre,
//!   inventerait une règle que l'issue n'a pas voulue.
//! - **La plage de température du tracé** et le nombre de points que la fenêtre choisit : c'est une
//!   décision d'affichage, elle se regarde sur `apercu`. Ce fichier fige la fonction, pas ses
//!   arguments.
//! - **Le dessin lui-même** — axes, graduations, poignées de palier. Cf. `apercu`.
//! - **L'état de régulation des canaux**, qui a son fichier : `spec_regule_fenetre.rs`.

use reverb_gui::telemetrie::{ordre_de_courbe, trace_de_courbe};
use reverb_proto::ipc::{ReguleAction, Request, encode_request};
use reverb_proto::regulation::{Courbe, CourbeInvalide};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// Les millidegrés d'un nombre entier de degrés.
///
/// ⚠️ **Tout est en millidegrés entiers, jamais en degrés flottants.** C'est la règle posée en tête
/// de la régulation (#99) : « une conversion vers `f32` en chemin rendrait la courbe dépendante d'un
/// arrondi, et le projet a déjà payé ce prix une fois » — les LED 1 et 9 d'une barrette sorties à
/// 153 et 152 pour une symétrie qui devait être exacte.
const fn degres(entiers: i32) -> i32 {
    entiers * 1_000
}

/// La courbe par défaut de #99 : 30 % à 35 °C, 60 % à 45 °C, 100 % à 50 °C.
const PALIERS_PAR_DEFAUT: [(i32, u8); 3] = [(degres(35), 30), (degres(45), 60), (degres(50), 100)];

/// Une courbe éditée, distincte de celle par défaut sur ses trois paliers.
const PALIERS_EDITES: [(i32, u8); 3] = [(degres(30), 40), (degres(50), 60), (degres(60), 100)];

/// Une courbe dont un palier tombe sur un demi-degré : de quoi vérifier que la fenêtre envoie bien
/// des millidegrés, et non un nombre de degrés arrondi en chemin.
const PALIERS_AU_DEMI_DEGRE: [(i32, u8); 2] = [(45_500, 64), (47_500, 80)];

/// La plage sur laquelle ce fichier trace, en millidegrés, et le nombre de points.
///
/// Elle **déborde des deux côtés** des paliers de la courbe par défaut — 20 °C est sous le premier
/// palier, 60 °C au-dessus du dernier —, ce qui est tout l'intérêt : c'est là que l'écrêtage se voit.
/// Quarante et un points sur quarante degrés font un point par degré, et donc des températures
/// entières où l'arithmétique tombe juste.
const FROID: i32 = degres(20);
const CHAUD: i32 = degres(60);
const POINTS: usize = 41;

/// La température que le README nomme pour motiver l'écrêtage : « prolonger la droite du premier
/// segment donnerait 0 % à 25 °C, c'est-à-dire des ventilateurs à l'arrêt sur un circuit qui
/// démarre ».
const CIRCUIT_QUI_DEMARRE: i32 = degres(25);

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

/// Une courbe qu'on exige valide, avec un refus qui recopie les paliers fautifs.
fn courbe(paliers: &[(i32, u8)]) -> Courbe {
    Courbe::depuis(paliers).unwrap_or_else(|erreur| {
        panic!(
            "Ces paliers devaient être acceptés : {paliers:?}\n  Refus : {}",
            erreur.raison
        )
    })
}

/// L'ordre qu'on exige produit, avec un refus qui dit pourquoi il ne l'a pas été.
fn ordre(paliers: &[(i32, u8)]) -> Request {
    ordre_de_courbe(paliers).unwrap_or_else(|erreur| {
        panic!(
            "Ces paliers devaient produire un ordre : {paliers:?}\n  Refus : {}",
            erreur.raison
        )
    })
}

/// Le refus qu'on exige, et le refus lui-même.
fn refus(paliers: &[(i32, u8)]) -> CourbeInvalide {
    match ordre_de_courbe(paliers) {
        Ok(requete) => panic!(
            "Ces paliers devaient être refusés **avant** tout envoi, la fenêtre en a fait « {} » :\
             \n  {paliers:?}",
            encode_request(&requete)
        ),
        Err(erreur) => erreur,
    }
}

/// Les formes de courbe sur lesquelles la fenêtre et le démon doivent tomber d'accord.
///
/// Bonnes et mauvaises mêlées : c'est l'accord qui est testé, pas le verdict. Chaque entrée porte un
/// nom, pour que l'échec dise laquelle a divergé.
fn formes() -> Vec<(&'static str, Vec<(i32, u8)>)> {
    vec![
        ("la courbe par défaut", PALIERS_PAR_DEFAUT.to_vec()),
        ("une courbe éditée", PALIERS_EDITES.to_vec()),
        ("des paliers au demi-degré", PALIERS_AU_DEMI_DEGRE.to_vec()),
        ("une courbe vide", vec![]),
        ("un seul palier", vec![(degres(45), 80)]),
        ("deux paliers", vec![(degres(35), 30), (degres(50), 100)]),
        ("les deux bornes", vec![(degres(20), 0), (degres(70), 100)]),
        (
            "des températures décroissantes",
            vec![(degres(50), 100), (degres(35), 30)],
        ),
        (
            "une température au milieu du désordre",
            vec![(degres(35), 30), (degres(50), 100), (degres(45), 60)],
        ),
        (
            "une température répétée",
            vec![(degres(35), 30), (degres(45), 60), (degres(45), 90)],
        ),
        (
            "un palier tout entier répété",
            vec![(degres(45), 60), (degres(45), 60)],
        ),
        (
            "une consigne à 101",
            vec![(degres(35), 30), (degres(45), 101)],
        ),
        (
            "une consigne à 255",
            vec![(degres(35), 30), (degres(45), 255)],
        ),
        (
            "des consignes décroissantes",
            vec![(degres(35), 80), (degres(45), 20)],
        ),
        (
            "des températures négatives",
            vec![(degres(-40), 30), (degres(45), 100)],
        ),
        (
            "les extrêmes de l'entier",
            vec![(i32::MIN / 2, 10), (i32::MAX / 2, 90)],
        ),
    ]
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tous les tests qui suivent supposent que la courbe par défaut est bien celle de #99, que la
    // courbe éditée en diffère, et que la plage de tracé déborde des deux côtés des paliers. Si l'un
    // de ces repères se dégradait, plusieurs tests deviendraient vrais sans rien vérifier — et
    // personne ne le verrait. Ce test est là pour que la panne soit ici.
    assert_eq!(
        Courbe::defaut().paliers(),
        PALIERS_PAR_DEFAUT,
        "la courbe par défaut est le tableau de #99, en millidegrés — et `reverb-proto` doit rendre \
         la même que `reverb-daemon` rendait, puisque c'est le même type"
    );

    let editee = courbe(&PALIERS_EDITES);
    assert_ne!(
        editee.consigne(degres(45)),
        Courbe::defaut().consigne(degres(45)),
        "la courbe éditée doit différer de celle par défaut, sinon les tests qui les opposent ne \
         vérifient rien"
    );

    const {
        assert!(
            FROID < PALIERS_PAR_DEFAUT[0].0,
            "la plage de tracé doit commencer **sous** le premier palier, sinon l'écrêtage bas ne \
             se voit pas"
        );
        assert!(
            CHAUD > PALIERS_PAR_DEFAUT[2].0,
            "et finir **au-dessus** du dernier, sinon l'écrêtage haut ne se voit pas"
        );
        assert!(
            FROID < CIRCUIT_QUI_DEMARRE && CIRCUIT_QUI_DEMARRE < PALIERS_PAR_DEFAUT[0].0,
            "les 25 °C du README doivent tomber dans la plage tracée, et sous le premier palier"
        );
        assert!(POINTS >= 2, "un tracé demande au moins ses deux bornes");
        assert!(
            (CHAUD - FROID) % (POINTS as i32 - 1) == 0,
            "la plage doit se diviser en pas entiers, sinon les températures tracées dépendraient \
             d'un arrondi et les assertions exactes ne voudraient plus rien dire"
        );
    }

    // Et le banc de formes doit porter des bonnes **et** des mauvaises, sinon l'accord entre la
    // fenêtre et le démon se vérifierait sur un seul verdict.
    let verdicts: Vec<bool> = formes()
        .iter()
        .map(|(_, paliers)| Courbe::depuis(paliers).is_ok())
        .collect();
    assert!(
        verdicts.contains(&true) && verdicts.contains(&false),
        "le banc de formes doit contenir des courbes acceptées et des courbes refusées"
    );
}

// ---------------------------------------------------------------------------
// 1 — une courbe éditée produit la ligne que le socket attend
// ---------------------------------------------------------------------------

#[test]
fn une_courbe_editee_produit_la_ligne_regule_courbe_attendue() {
    // Test d'intention n° 4 de l'issue. Critère d'acceptation n° 4 : « la courbe s'édite et se pose
    // par le socket ».
    //
    // Le verbe existe déjà — l'issue le dit : « les verbes existent déjà : `regule`,
    // `regule <canal> on|off`, `regule courbe …`. Il manque leur lecture dans le client de la fenêtre
    // et le rendu. » La fenêtre n'a donc aucune ligne à inventer, seulement celle-là à écrire.
    assert_eq!(
        ordre(&PALIERS_PAR_DEFAUT),
        Request::Regule(ReguleAction::Courbe(PALIERS_PAR_DEFAUT.to_vec())),
        "la courbe éditée part telle quelle, palier pour palier"
    );
    assert_eq!(
        encode_request(&ordre(&PALIERS_PAR_DEFAUT)),
        "regule courbe 35000:30 45000:60 50000:100",
        "c'est la ligne relevée sur le socket de SHYNAEL"
    );
    assert_eq!(
        encode_request(&ordre(&PALIERS_EDITES)),
        "regule courbe 30000:40 50000:60 60000:100",
        "une courbe réglée s'écrit de la même façon — sinon l'éditeur ne servirait qu'à réémettre \
         le défaut"
    );
}

#[test]
fn les_paliers_partent_en_millidegres_entiers() {
    // ⚠️ **Les températures sont en millidegrés entiers, jamais en degrés flottants** (#99). Une
    // fenêtre qui manipulerait des degrés — ce que l'éditeur affiche, forcément — et convertirait au
    // moment d'envoyer ferait disparaître le demi-degré sans un message : c'est exactement la faute
    // des trois ordres de composantes, qui ne produit rien d'autre qu'un résultat faux.
    assert_eq!(
        encode_request(&ordre(&PALIERS_AU_DEMI_DEGRE)),
        "regule courbe 45500:64 47500:80",
        "45,5 °C s'écrit 45500 : le demi-degré doit traverser le socket intact"
    );

    // Et une température négative aussi — un `i32` signé, pas un compteur.
    assert_eq!(
        encode_request(&ordre(&[(degres(-40), 30), (degres(45), 100)])),
        "regule courbe -40000:30 45000:100",
    );
}

// ---------------------------------------------------------------------------
// 2 — une courbe invalide est refusée en le disant, et rien ne part
// ---------------------------------------------------------------------------

#[test]
fn une_courbe_aux_paliers_decroissants_est_refusee_avant_d_etre_envoyee() {
    // Test d'intention n° 5 de l'issue. Critère d'acceptation n° 5 : « une courbe invalide est
    // refusée en le disant, sans être posée ».
    //
    // Deux formes du même désordre : la courbe entièrement à l'envers, et celle dont un seul palier
    // s'est glissé au mauvais endroit — la faute qu'on fait en éditant, pas en tapant. #99 refuse
    // les deux plutôt que de trier en silence : « réordonner ce qu'un humain a écrit, c'est
    // *compléter au jugé*, ce que le projet refuse pour `eclairage.conf` ».
    for paliers in [
        &[(degres(50), 100), (degres(35), 30)][..],
        &[(degres(35), 30), (degres(50), 100), (degres(45), 60)][..],
    ] {
        let erreur = refus(paliers);
        assert!(
            !erreur.raison.trim().is_empty(),
            "une courbe dont les paliers ne montent pas doit être refusée **avec une raison**, pas \
             avec un silence. Paliers : {paliers:?}"
        );
    }
}

#[test]
fn une_temperature_repetee_est_refusee_en_la_nommant() {
    // Deux consignes à 45 °C se contredisent — laquelle appliquer ? — et n'en garder qu'une ferait
    // dépendre la régulation de l'ordre des lignes. #99 exige que le refus **nomme** le fautif ;
    // ce fichier reprend la même exigence, dans la même forme : la raison contient le jeton en
    // cause, sans que sa mise en forme soit imposée — « 45 », « 45000 » et « 45 °C » passent tous.
    for paliers in [
        &[(degres(35), 30), (degres(45), 60), (degres(45), 90)][..],
        &[(degres(45), 60), (degres(45), 60)][..],
    ] {
        let erreur = refus(paliers);
        assert!(
            erreur.raison.contains("45"),
            "le refus doit nommer la température répétée (45 °C), pour qu'on sache quelle ligne de \
             l'éditeur corriger. Raison obtenue : {}",
            erreur.raison
        );
    }
}

#[test]
fn une_consigne_au_dessus_de_cent_est_refusee_en_la_nommant() {
    // Au-delà de cent pour cent il n'y a plus de ventilateur. #99 le refuse à la construction
    // plutôt que de raboter en silence une valeur que l'utilisateur a écrite — et la fenêtre est
    // justement l'endroit où il l'écrit.
    for consigne in [101u8, 150, 200, 255] {
        let erreur = refus(&[(degres(35), 30), (degres(45), consigne)]);
        assert!(
            erreur.raison.contains(&consigne.to_string()),
            "le refus doit nommer la consigne fautive « {consigne} ». Raison obtenue : {}",
            erreur.raison
        );
    }
}

#[test]
fn une_courbe_vide_est_refusee_avec_une_raison() {
    // Une table sans palier n'a aucune consigne à rendre, et rendre 0 % serait arrêter les
    // ventilateurs sur une table que personne n'a écrite. C'est aussi l'état d'un éditeur dont on
    // vient de retirer la dernière ligne : le refus doit être lisible, pas un silence.
    let erreur = refus(&[]);
    assert!(
        !erreur.raison.trim().is_empty(),
        "une courbe sans palier doit être refusée avec une raison, pas avec un silence"
    );
}

#[test]
fn une_courbe_a_un_seul_palier_reste_acceptee() {
    // ⚠️ **Ce n'est pas un oubli, c'est un refus de contredire.** `spec_regulation_hote.rs` fige un
    // palier unique comme une courbe **valide** et plate — `une_courbe_a_un_seul_palier_est_plate`
    // —, et c'est un réglage sensé : une consigne fixe, la même à toute température.
    //
    // Une fenêtre plus sévère que le socket refuserait ici ce que `regule courbe 45000:80` accepte
    // là-bas, et l'utilisateur n'aurait aucun moyen de comprendre pourquoi la même courbe passe par
    // `socat` et pas par la souris.
    assert_eq!(
        encode_request(&ordre(&[(degres(45), 80)])),
        "regule courbe 45000:80",
        "un palier unique est une courbe plate, et le démon l'accepte (#99)"
    );
}

#[test]
fn la_fenetre_refuse_exactement_ce_que_le_demon_refuse_et_pour_la_meme_raison() {
    // C'est la règle qui compte, et elle vaut mieux que la liste des cas ci-dessus : la fenêtre ne
    // décide de rien, elle **réutilise** le juge du démon.
    //
    // Une fenêtre plus sévère refuserait une courbe que le socket accepte — l'utilisateur ne
    // saurait pas laquelle des deux a raison. Une fenêtre plus laxiste laisserait partir une ligne
    // que le démon rejettera, et le refus arriverait **après** le geste, dans un journal, au lieu
    // d'arriver devant le champ qu'on vient de taper.
    //
    // La raison est comparée elle aussi : deux textes différents pour le même défaut, c'est deux
    // messages à maintenir, dont un seul serait relu le jour où le format change.
    for (nom, paliers) in formes() {
        match (ordre_de_courbe(&paliers), Courbe::depuis(&paliers)) {
            (Ok(requete), Ok(_)) => assert_eq!(
                requete,
                Request::Regule(ReguleAction::Courbe(paliers.clone())),
                "{nom} : acceptée des deux côtés, elle doit partir telle quelle — {paliers:?}"
            ),
            (Err(fenetre), Err(demon)) => assert_eq!(
                fenetre.raison, demon.raison,
                "{nom} : refusée des deux côtés, mais avec deux raisons différentes — {paliers:?}"
            ),
            (Ok(requete), Err(demon)) => panic!(
                "{nom} : la fenêtre laisse partir « {} » que le démon refusera — « {} ». Le refus \
                 arriverait après le geste au lieu d'arriver devant le champ. Paliers : {paliers:?}",
                encode_request(&requete),
                demon.raison
            ),
            (Err(fenetre), Ok(_)) => panic!(
                "{nom} : la fenêtre refuse « {} » une courbe que le socket accepte. La même courbe \
                 passerait par `socat` et pas par la souris. Paliers : {paliers:?}",
                fenetre.raison
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — le tracé vient de `Courbe::consigne`, et de nulle part ailleurs
// ---------------------------------------------------------------------------

#[test]
fn chaque_point_du_trace_est_celui_que_consigne_rend() {
    // ⚠️ L'exigence de l'issue, mot pour mot : « le tracé de la courbe doit venir de la **même**
    // fonction que celle qu'exécute le démon […] sinon l'aperçu montrerait une courbe que le boîtier
    // n'applique pas. »
    //
    // C'est la même promesse que la maquette du boîtier tient déjà — « l'aperçu montre ce que le
    // boîtier reçoit », vrai par construction et non par la coïncidence de deux implémentations
    // qu'il faudrait tenir d'accord (README). Ici, la vérification est directe : chaque point tracé
    // doit valoir ce que `consigne` rend à **sa** température.
    for c in [
        Courbe::defaut(),
        courbe(&PALIERS_EDITES),
        courbe(&[(degres(45), 80)]),
    ] {
        for (temperature, consigne) in trace_de_courbe(&c, FROID, CHAUD, POINTS) {
            assert_eq!(
                consigne,
                c.consigne(temperature),
                "à {temperature} m°C, la courbe {:?} rend {} % et le tracé en dessine {consigne} % \
                 : ce serait une seconde interpolation, et elle n'aurait aucune raison d'être la \
                 bonne",
                c.paliers(),
                c.consigne(temperature)
            );
        }
    }
}

#[test]
fn le_trace_couvre_la_plage_demandee_de_bout_en_bout() {
    // Un tracé qui s'arrêterait avant le dernier palier montrerait une courbe qui monte encore là
    // où elle est en fait déjà à fond — c'est-à-dire l'inverse de ce qu'on cherche à lire.
    let trace = trace_de_courbe(&Courbe::defaut(), FROID, CHAUD, POINTS);

    assert_eq!(
        trace.len(),
        POINTS,
        "un tracé de {POINTS} points doit en rendre {POINTS}"
    );
    assert_eq!(
        trace.first().map(|(t, _)| *t),
        Some(FROID),
        "le premier point est à la borne froide demandée"
    );
    assert_eq!(
        trace.last().map(|(t, _)| *t),
        Some(CHAUD),
        "le dernier point est à la borne chaude demandée"
    );

    let mut precedente = i32::MIN;
    for (temperature, _) in &trace {
        assert!(
            *temperature > precedente,
            "les points doivent aller du froid au chaud sans revenir en arrière : {temperature} \
             après {precedente}"
        );
        precedente = *temperature;
    }
}

#[test]
fn le_trace_ecrete_aux_bornes_et_n_extrapole_jamais() {
    // Critère de #99 que le tracé hérite : « une température hors des paliers connus est ramenée aux
    // bornes, pas extrapolée ». Le README dit pourquoi, et c'est la raison la plus concrète du
    // projet : « prolonger la droite du premier segment donnerait 0 % à 25 °C, c'est-à-dire des
    // ventilateurs à l'arrêt sur un circuit qui démarre ».
    //
    // Un tracé qui extrapolerait dessinerait donc une courbe plongeant vers zéro à gauche et
    // dépassant cent à droite — et l'utilisateur réglerait sa machine sur ce dessin.
    let defaut = Courbe::defaut();
    let trace = trace_de_courbe(&defaut, FROID, CHAUD, POINTS);
    let (premier_palier, premiere_consigne) = PALIERS_PAR_DEFAUT[0];
    let (dernier_palier, derniere_consigne) = PALIERS_PAR_DEFAUT[2];

    let mut vu_sous = 0;
    let mut vu_au_dessus = 0;
    for (temperature, consigne) in &trace {
        assert!(
            *consigne <= 100,
            "à {temperature} m°C le tracé dessine {consigne} % : au-delà de cent pour cent il n'y a \
             plus de ventilateur"
        );
        if *temperature <= premier_palier {
            assert_eq!(
                *consigne, premiere_consigne,
                "à {temperature} m°C, sous le premier palier, le tracé doit être plat à \
                 {premiere_consigne} % — pas prolonger la droite vers zéro"
            );
            vu_sous += 1;
        }
        if *temperature >= dernier_palier {
            assert_eq!(
                *consigne, derniere_consigne,
                "à {temperature} m°C, au-dessus du dernier palier, le tracé doit être plat à \
                 {derniere_consigne} %"
            );
            vu_au_dessus += 1;
        }
    }

    assert!(
        vu_sous >= 2 && vu_au_dessus >= 2,
        "la plage doit déborder des deux côtés pour que l'écrêtage se voie : {vu_sous} points sous \
         le premier palier, {vu_au_dessus} au-dessus du dernier"
    );

    // Et le point que le README nomme, pointé pour lui-même.
    assert_eq!(
        defaut.consigne(CIRCUIT_QUI_DEMARRE),
        premiere_consigne,
        "à 25 °C la courbe rend {premiere_consigne} %, jamais 0 % : c'est un circuit qui démarre, \
         pas un boîtier à l'arrêt"
    );
}

#[test]
fn le_trace_ne_redescend_jamais_sur_une_courbe_qui_monte() {
    // Ce n'est pas un critère écrit, c'est ce qui rend les autres vrais : une erreur de signe dans
    // le parcours produirait un tracé parfaitement borné, parfaitement continu, et qui montrerait
    // les ventilateurs ralentir quand le liquide chauffe. Aucun autre test ne la verrait, et le
    // dessin ne dirait rien — c'est le même garde-fou que #99 s'est donné sur `consigne`.
    for c in [Courbe::defaut(), courbe(&PALIERS_EDITES)] {
        let mut precedente = 0u8;
        for (temperature, consigne) in trace_de_courbe(&c, FROID, CHAUD, POINTS) {
            assert!(
                consigne >= precedente,
                "à {temperature} m°C le tracé de {:?} retombe de {precedente} % à {consigne} %",
                c.paliers()
            );
            precedente = consigne;
        }
        assert_eq!(
            precedente,
            100,
            "au bout de la plage, ces deux courbes sont à fond : {:?}",
            c.paliers()
        );
    }
}

#[test]
fn le_trace_ne_panique_pas_sur_les_formes_biscornues() {
    // Le tracé reçoit ses bornes d'une fenêtre, donc d'un code qui peut se tromper : un panneau qui
    // n'a pas encore sa taille, une plage vide, une plage inversée. Aucun de ces cas ne doit arrêter
    // la fenêtre — et les extrêmes de l'`i32` sont le piège réel, parce que `chaud - froid` déborde
    // avant même qu'on ait tracé quoi que ce soit. En `debug`, ce serait une panique.
    //
    // Ce test n'exige aucun résultat en particulier : seulement que la fonction en rende un.
    let defaut = Courbe::defaut();
    for (froid, chaud) in [
        (FROID, CHAUD),
        (FROID, FROID),
        (CHAUD, FROID),
        (i32::MIN, i32::MAX),
        (i32::MAX, i32::MIN),
        (i32::MIN, i32::MIN),
        (0, 1),
    ] {
        for points in [0usize, 1, 2, 3, POINTS, 1_000] {
            let trace = trace_de_courbe(&defaut, froid, chaud, points);
            for (temperature, consigne) in trace {
                assert!(
                    consigne <= 100,
                    "plage {froid}..{chaud} en {points} points : à {temperature} m°C le tracé rend \
                     {consigne} %"
                );
            }
        }
    }
}
