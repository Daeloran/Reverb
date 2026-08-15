//! Tests d'intention de la régulation pilotée depuis la fenêtre (issue #113).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #113 seule et les trois dont elle dépend —
//! #99 pour ce que « réguler » veut dire, #100 pour un canal illisible, #112 pour le verrou. Rien
//! de `crates/*/src/` ni de `crates/reverb-gui/ui/` n'a été lu pour les produire. Seules ont été
//! reprises les signatures publiques que les fichiers de tests d'intention voisins ont déjà figées :
//! `LigneCanal`, `Telemetrie` et `Tri` par `spec_canal_illisible.rs` (#100), `regule_seul` et
//! `commande_verrouillable` par `spec_verrou_canal.rs` (#112), `Courbe` par
//! `spec_regulation_hote.rs` (#99).
//!
//! À l'écriture de ce fichier, `LigneCanal::regulable`, `LigneCanal::sous_regulation`,
//! `ordre_de_regulation`, `commande_effective`, `Telemetrie::courbe` et le module
//! `reverb_proto::regulation` **n'existent pas** : la compilation doit échouer sur eux, et sur eux
//! seuls. C'est la phase rouge.
//!
//! Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne touche un bus.
//!
//! ## Le manque, tel que l'issue le décrit
//!
//! #99 a livré la régulation côté hôte des sept ventilateurs que rien ne pilotait sous Linux, et
//! elle ne se règle **que par le socket** :
//!
//! ```text
//! echo 'regule nzxtsmart2:fan-1 on' | socat - UNIX-CONNECT:/run/reverb/reverbd.sock
//! ```
//!
//! C'est pourtant le réglage qui décide du bruit et de la température du poste, et le seul qui
//! reste inaccessible à la souris.
//!
//! ## Ce qu'on peut tester d'une fenêtre, et ce qu'on ne peut pas
//!
//! ⚠️ **Un panneau ne s'écrit pas en assertions.** Ce qui se vérifie ici, c'est la **décision** :
//! une ligne entre, et l'on regarde quelle requête sort. C'est une fonction pure de ce que le démon
//! a répondu, et elle n'a besoin d'aucune session graphique. C'est la règle posée par #100 pour le
//! tri, par #102 pour la valeur écrite à côté d'une barre, et par #112 pour le verrou : ce qui est
//! du calcul se teste, ce qui est du dessin se regarde.
//!
//! Le grisé d'une barre régulée, la forme de la bascule, la place de l'éditeur de courbe et le fait
//! que la mise en page tienne dans la largeur du panneau se regardent sur une image, hors de ce
//! fichier : `REVERB_ONGLET=ventilos cargo run --release --example apercu -p reverb-gui`. Le
//! critère d'acceptation le dit lui-même — « la mise en page est vérifiée par l'exemple `apercu` ».
//!
//! ## Le contrat que ces tests figent
//!
//! ```ignore
//! // dans crates/reverb-gui/src/telemetrie.rs, à côté de `LigneCanal` (#100) et du verrou (#112).
//!
//! pub struct LigneCanal {
//!     // … les champs de #100, inchangés …
//!
//!     /// **Fait matériel** : le pilote de ce canal n'a **aucun** mode automatique, donc c'est à
//!     /// l'hôte de le réguler s'il doit l'être. Vient du dernier jeton de la ligne `chan`.
//!     pub regulable: bool,
//!
//!     /// **État rendu par le démon** : il régule ce canal, à ce tour-ci. Vient des lignes
//!     /// `regule <canal>` du même tour, et de nulle part ailleurs.
//!     pub sous_regulation: bool,
//! }
//!
//! impl LigneCanal {
//!     /// L'ordre qu'une bascule de régulation produit — `None` quand il n'y a rien à basculer :
//!     /// canal non régulable (le firmware s'en charge), ou canal illisible (#100).
//!     pub fn ordre_de_regulation(&self, activer: bool) -> Option<Request>;
//!
//!     /// La requête qu'une consigne produit, **tous les garde-fous appliqués** : la quarantaine
//!     /// de #100, le verrou de #112, et la régulation de #113.
//!     pub fn commande_effective(&self, action: FanAction, deverrouille: bool) -> Option<Request>;
//! }
//!
//! pub struct Telemetrie {
//!     pub canaux: Vec<LigneCanal>,
//!     pub sondes: Vec<(String, Releve)>,
//!
//!     /// La courbe que le démon vient de rendre, `None` si le tour n'en portait pas.
//!     pub courbe: Option<Courbe>,
//! }
//! ```
//!
//! et, côté protocole :
//!
//! ```ignore
//! // La ligne `chan` gagne un dernier jeton :
//! //   chan <canal> <position> <rpm> <pwm> <mode> <sait_faire_auto> <regulable>
//! // Un jeton absent — la forme d'avant #113 — se lit `regulable = non`.
//!
//! // crates/reverb-proto/src/regulation.rs — descendu de `reverb_daemon::regulation`, que les
//! // deux crates partagent désormais, et que le démon ré-exporte :
//! pub struct Courbe { /* … */ }
//! pub struct CourbeInvalide { pub raison: String }
//! ```
//!
//! ### Pourquoi un champ de plus sur la ligne `chan`
//!
//! L'issue exige que « un canal du Kraken ne propose pas la régulation » et que « un canal dont le
//! pilote n'a pas de mode automatique » puisse être mis sous régulation. **La fenêtre n'a aucun
//! moyen de le savoir aujourd'hui.** `sait_faire_auto` ne dit plus le fait matériel depuis #97 — il
//! vaut « le pilote sait **et** une courbe a été posée », donc toujours `non` (`docs/VENTILATEURS.md`,
//! et #112 l'écrit noir sur blanc : « Ne pas s'appuyer sur `sait_faire_auto` »).
//!
//! ### Pourquoi `regulable`, et pourquoi `sous_regulation`
//!
//! - **`regulable`** répond à la question que la fenêtre pose pour décider d'afficher la commande :
//!   « peut-on mettre ce canal sous régulation ? ». La formulation littérale du fait matériel —
//!   `sans_mode_auto` — dirait la même chose en négatif, et un booléen négatif lu dans une
//!   condition est exactement l'endroit où les erreurs de signe vivent.
//! - **`sous_regulation`** plutôt que `regule`, parce que `regule_seul()` existe déjà (#112) et
//!   désigne le **contraire** : le périphérique se régule tout seul. Deux noms à une lettre près
//!   pour deux décideurs opposés seraient une faute qui ne produirait aucun message.
//!
//! ### Pourquoi une troisième méthode de commande
//!
//! `commande` est figée par #100 (« un canal illisible n'émet rien »), `commande_verrouillable` par
//! #112 (« … ni un canal qui régule seul et dont le verrou est fermé »). Les élargir changerait le
//! sens de fonctions que des tests d'intention voisins tiennent déjà. `commande_effective` est
//! nommée d'après ce qu'elle répond — « ce qui part vraiment quand la barre bouge » — et non
//! d'après le garde-fou qu'elle ajoute : il y en a maintenant trois, et une issue de plus
//! épuiserait les adjectifs.
//!
//! ### Pourquoi la régulation n'est pas un état, mais une lecture
//!
//! Critère d'acceptation : « l'état de régulation affiché est celui que le démon rend, jamais une
//! mémoire de fenêtre ». C'est ce qui distingue cette issue de la pastille de profil, qui est
//! justement une mémoire de fenêtre — le README l'explique : « la pastille allumée dit *rappelé*,
//! jamais *actif* ».
//!
//! Conséquence que ces tests figent : **les deux réponses se posent ensemble**. Le tour de la
//! fenêtre porte la réponse à `status` *et* celle à `regule`, et `sous_regulation` est recalculé à
//! chaque tour depuis les seules lignes `regule` reçues. Une fenêtre qui poserait les deux
//! séparément verrait, à chaque tour de `status`, tous ses canaux sortir de régulation.
//!
//! ## Ce que ces tests ne figent pas, et pourquoi
//!
//! - **Le dessin** : le grisé, la bascule, l'éditeur de courbe, la place de tout cela. Cf. `apercu`.
//! - **Le geste qui bascule la régulation** — case à cocher, bouton, menu. L'issue n'en dit rien ;
//!   ce fichier tient l'effet du geste, pas sa forme.
//! - **`sous_regulation` sur une ligne illisible.** #100 laisse déjà ouverts la `position` et le
//!   `mode` d'une ligne en quarantaine, pour la même raison : ce ne sont pas des mesures. Ce
//!   fichier n'exige que l'inertie, qui est vraie des deux côtés du choix.
//! - **Un canal `regulable` dont le mode verrouille (#112).** Le cas n'existe sur aucune machine
//!   connue — les trois `nzxtsmart2` lisent `manuel` — et l'issue ne le nomme pas. Le trancher
//!   inventerait une règle : mettre sous régulation un canal qui régule déjà seul détruirait ce que
//!   le verrou protège, mais le refuser rendrait un canal inaccessible sans explication. À rouvrir
//!   le jour où un pilote sans mode automatique lit `non-piloté`.
//! - **L'édition et le tracé de la courbe**, qui ont leur fichier : `spec_courbe_fenetre.rs`.
//! - **Ce que le démon fait de `regule <canal> on`** : c'est #99, et `spec_regulation_hote.rs` le
//!   couvre. Ce fichier s'arrête à la requête qui part.

use reverb_gui::telemetrie::{LigneCanal, Telemetrie, Tri};
use reverb_proto::ipc::{
    FanAction, ReguleAction, Request, ResponseLine, encode_request, parse_response_line,
};
use reverb_proto::regulation::Courbe;

// ---------------------------------------------------------------------------
// Le banc : les cinq canaux de SHYNAEL, et les deux natures qui les séparent
// ---------------------------------------------------------------------------

/// Les trois canaux `nzxtsmart2`, qui portent sept ventilateurs sur dix et que **rien ne régulait**
/// avant #99 : « le pilote `nzxt-smart2` n'a aucun mode automatique, sa vitesse est celle que
/// l'hôte écrit, et personne ne l'écrivait ».
const SMART_1: &str = "nzxtsmart2:fan-1";
const SMART_2: &str = "nzxtsmart2:fan-2";
const SMART_3: &str = "nzxtsmart2:fan-3";

/// Les deux canaux du Kraken, **hors scope de #99** : « leur firmware régule déjà correctement,
/// mesuré ». Ce sont eux que le critère d'acceptation n° 2 vise — « un canal du Kraken ne propose
/// pas la régulation ».
const POMPE: &str = "kraken2023elite:pump-speed";
const VENTILO_KRAKEN: &str = "kraken2023elite:fan-speed";

/// Le banc : le canal, la position que le démon publie, et le **fait matériel** que #113 fait
/// traverser le socket — ce pilote n'a aucun mode automatique, donc l'hôte doit réguler lui-même.
const BANC: [(&str, &str, bool); 5] = [
    (SMART_1, "radiateur-haut", true),
    (SMART_2, "radiateur-milieu", true),
    (SMART_3, "radiateur-bas", true),
    (POMPE, "-", false),
    (VENTILO_KRAKEN, "-", false),
];

/// Les trois canaux que le démon peut prendre en charge.
const REGULABLES: [&str; 3] = [SMART_1, SMART_2, SMART_3];

/// Les deux qu'il doit laisser à leur firmware.
const NON_REGULABLES: [&str; 2] = [POMPE, VENTILO_KRAKEN];

// ---------------------------------------------------------------------------
// Les libellés de mode, recopiés de #112
// ---------------------------------------------------------------------------
//
// ⚠️ Ils sont **recopiés** et non calculés : `reverb-gui` ne dépend pas de `reverb-hw`, et ne doit
// pas se mettre à en dépendre pour une colonne de texte. C'est la règle du projet appliquée à un
// libellé plutôt qu'à une trame. `spec_verrou_canal.rs` porte la liste complète et sa source.

/// Ce que lisent les trois `nzxtsmart2` sur SHYNAEL — l'hôte décide déjà de la vitesse, il n'y a
/// aucune régulation à perdre, donc aucun verrou (#112).
const MANUEL: &str = "manuel";

/// Ce que lisent les deux canaux du Kraken — le périphérique décide, donc verrou (#112).
const NON_PILOTE: &str = "non-piloté";

/// Un second mode que #112 laisse ouvert, pour que « la régulation ne dépend pas du mode » ne se
/// vérifie pas sur un seul libellé.
const PLEIN_REGIME: &str = "plein-régime-100%";

/// Les deux modes sur lesquels ce fichier exerce la bascule de régulation.
///
/// Ni l'un ni l'autre ne verrouille (#112) : la composition « régulable **et** verrouillé » n'existe
/// sur aucune machine connue, et l'en-tête dit pourquoi ce fichier ne la tranche pas.
const MODES_OUVERTS: [&str; 2] = [MANUEL, PLEIN_REGIME];

/// Les deux valeurs du drapeau `sait_faire_auto`, tel que la ligne `chan` l'écrit.
///
/// #112 interdit de s'appuyer dessus, et #113 ajoute un fait qui ne s'en déduit pas : chaque
/// décision est donc essayée sous les deux.
const DRAPEAUX_AUTO: [bool; 2] = [true, false];

// ---------------------------------------------------------------------------
// Les mesures du banc
// ---------------------------------------------------------------------------

/// Le régime que les lignes du banc rendent.
const RPM: u32 = 1_357;

/// La consigne qu'elles rendent — 33 %, soit ce que la courbe par défaut calcule à 36 °C de liquide
/// (`spec_regulation_hote.rs`). C'est la « consigne calculée » du critère d'acceptation n° 6 : sur
/// un canal régulé, c'est le démon qui l'a écrite.
const PWM: u8 = 33;

/// La raison d'une quarantaine, telle que le démon l'écrit (#88, #100).
///
/// ⚠️ Elle porte des espaces : c'est le dernier champ de la ligne `unreadable`.
const RAISON: &str = "Connection timed out (os error 110)";

/// Les quatre consignes qu'une poignée de ventilateur peut produire.
///
/// Ce sont des **fabriques** et non des valeurs, comme dans `spec_canal_illisible.rs` et
/// `spec_verrou_canal.rs` : `FanAction` n'a pas à être `Copy` pour que ces tests compilent.
const CONSIGNES: [fn() -> FanAction; 4] = [
    || FanAction::Auto,
    || FanAction::Pwm(0),
    || FanAction::Pwm(50),
    || FanAction::Pwm(100),
];

/// Les paliers de la courbe par défaut, en **millidegrés entiers** — jamais en degrés flottants.
///
/// C'est la règle du projet, et `spec_regulation_hote.rs` la motive : « une conversion vers `f32` en
/// chemin rendrait la courbe dépendante d'un arrondi ».
const PALIERS_PAR_DEFAUT: [(i32, u8); 3] = [(35_000, 30), (45_000, 60), (50_000, 100)];

/// Une seconde courbe, distincte de celle par défaut : de quoi vérifier que la fenêtre montre celle
/// que le démon rend, et non celle qu'elle sait écrire.
const PALIERS_AUTRES: [(i32, u8); 3] = [(30_000, 40), (50_000, 60), (60_000, 100)];

// ---------------------------------------------------------------------------
// Les lignes du démon, écrites comme il les écrit
// ---------------------------------------------------------------------------

/// Décode une ligne de réponse, ou dit laquelle a été mal écrite.
fn ligne(texte: &str) -> ResponseLine {
    parse_response_line(texte).unwrap_or_else(|erreur| {
        panic!("« {texte} » doit être une ligne de réponse valide, refusée : {erreur:?}")
    })
}

/// Le jeton booléen du protocole.
fn oui_non(vrai: bool) -> &'static str {
    if vrai { "oui" } else { "non" }
}

/// La position et la nature d'un canal du banc.
fn habillage(canal: &str) -> (&'static str, bool) {
    BANC.iter()
        .find(|(nom, _, _)| *nom == canal)
        .map(|(_, position, regulable)| (*position, *regulable))
        .unwrap_or_else(|| panic!("« {canal} » n'est pas un canal du banc"))
}

/// La ligne `chan` telle que #113 la porte :
/// `chan <canal> <position> <rpm> <pwm> <mode> <sait_faire_auto> <regulable>`.
fn chan(canal: &str, mode: &str, auto: bool, regulable: bool) -> ResponseLine {
    let (position, _) = habillage(canal);
    ligne(&format!(
        "chan {canal} {position} {RPM} {PWM} {mode} {} {}",
        oui_non(auto),
        oui_non(regulable)
    ))
}

/// La ligne `chan` d'un canal du banc, avec sa nature vraie.
fn chan_du_banc(canal: &str, mode: &str, auto: bool) -> ResponseLine {
    let (_, regulable) = habillage(canal);
    chan(canal, mode, auto, regulable)
}

/// La ligne `chan` **d'avant #113** : six jetons, sans le drapeau de régulation.
///
/// C'est la forme qu'écrit un démon d'avant — et c'est aussi celle que `spec_canal_illisible.rs`,
/// `spec_verrou_canal.rs` et `spec_barre_valeur.rs` envoient. Elle doit rester lisible.
fn chan_d_avant(canal: &str, mode: &str, auto: bool) -> ResponseLine {
    let (position, _) = habillage(canal);
    ligne(&format!(
        "chan {canal} {position} {RPM} {PWM} {mode} {}",
        oui_non(auto)
    ))
}

/// La ligne `unreadable` d'un canal en quarantaine, telle que le démon l'écrit depuis #88.
fn illisible(canal: &str) -> ResponseLine {
    ligne(&format!("unreadable {canal} {RAISON}"))
}

/// La ligne `regule <canal>` que le démon rend pour chacun des canaux qu'il régule.
fn regule(canal: &str) -> ResponseLine {
    ligne(&format!("regule {canal}"))
}

/// La ligne `courbe <milli>:<pourcent> …` qui ouvre la réponse à `regule`.
fn courbe_rendue(paliers: &[(i32, u8)]) -> ResponseLine {
    let jetons: Vec<String> = paliers
        .iter()
        .map(|(milli, pourcent)| format!("{milli}:{pourcent}"))
        .collect();
    ligne(&format!("courbe {}", jetons.join(" ")))
}

/// Un tour de fenêtre complet : la réponse à `status`, puis celle à `regule`.
///
/// ⚠️ **Les deux réponses sont posées ensemble.** C'est le point que ce fichier fige : l'état de
/// régulation d'un canal est celui que porte le tour où sa ligne `chan` est arrivée. Les deux `end`
/// sont laissés en place — c'est ce que la fenêtre lit vraiment sur le socket, et #100 a déjà figé
/// qu'une ligne qui n'est pas de la télémétrie ne fait ni canal ni sonde.
fn tour(mode: &str, auto: bool, regules: &[&str], muets: &[&str]) -> Vec<ResponseLine> {
    let mut lignes = Vec::new();
    for (canal, _, _) in BANC {
        if muets.contains(&canal) {
            lignes.push(illisible(canal));
        } else {
            lignes.push(chan_du_banc(canal, mode, auto));
        }
    }
    lignes.push(ligne("end"));

    lignes.push(courbe_rendue(&PALIERS_PAR_DEFAUT));
    for canal in regules {
        lignes.push(regule(canal));
    }
    lignes.push(ligne("end"));
    lignes
}

// ---------------------------------------------------------------------------
// Lire un tri
// ---------------------------------------------------------------------------

/// Les noms des lignes de ventilateur, dans l'ordre rendu.
fn noms(vue: &Telemetrie) -> Vec<String> {
    vue.canaux.iter().map(|l| l.canal.clone()).collect()
}

/// La ligne d'un canal, ou l'échec qui dit ce qu'on a trouvé à la place.
fn ligne_de<'a>(vue: &'a Telemetrie, canal: &str) -> &'a LigneCanal {
    vue.canaux
        .iter()
        .find(|l| l.canal == canal)
        .unwrap_or_else(|| {
            panic!(
                "« {canal} » doit garder sa ligne de ventilateur (#100) ; la liste rendue est {:?}",
                noms(vue)
            )
        })
}

/// La vue d'un tour, posée sur un tri neuf **après** un tour où tout répondait.
///
/// Le tour préalable met ce fichier du bon côté de ce que #100 garantit : un canal muet dès le
/// premier tour n'a jamais été vu lisible, et #100 laisse son sort ouvert.
fn telemetrie(mode: &str, auto: bool, regules: &[&str], muets: &[&str]) -> Telemetrie {
    let mut tri = Tri::nouveau();
    tri.poser(&tour(mode, auto, &[], &[]));
    tri.poser(&tour(mode, auto, regules, muets))
}

/// La requête `regule <canal> on|off` attendue.
fn ordre_attendu(canal: &str, activer: bool) -> Request {
    let canal = canal.to_owned();
    Request::Regule(if activer {
        ReguleAction::Activer(canal)
    } else {
        ReguleAction::Couper(canal)
    })
}

/// La requête qu'une consigne doit produire quand rien ne l'arrête.
fn consigne_attendue(canal: &str, action: FanAction) -> Request {
    Request::Fan {
        channel: canal.to_owned(),
        action,
    }
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tout ce qui suit suppose que le banc porte les deux natures de canal, que les modes employés
    // se comportent comme #112 les a figés, et que les lignes fabriquées portent bien ce qu'on croit
    // y mettre. Si l'un de ces repères se dégradait, plusieurs tests deviendraient vrais sans rien
    // vérifier — et personne ne le verrait. Ce test est là pour que la panne soit ici.
    assert_eq!(
        REGULABLES.len() + NON_REGULABLES.len(),
        BANC.len(),
        "les deux listes doivent partitionner le banc : un canal oublié ne serait testé d'aucun côté"
    );
    for (i, (canal, _, _)) in BANC.iter().enumerate() {
        for (autre, _, _) in BANC.iter().skip(i + 1) {
            assert_ne!(
                canal, autre,
                "deux canaux identiques dans le banc : les tests qui les opposent ne vérifient rien"
            );
        }
    }

    // Les deux natures, telles que la ligne `chan` les porte.
    let vue = telemetrie(MANUEL, false, &[], &[]);
    for canal in REGULABLES {
        assert!(
            ligne_de(&vue, canal).regulable,
            "« {canal} » est sur `nzxt-smart2`, dont le pilote n'a aucun mode automatique : c'est \
             le fait matériel que #113 fait traverser le socket"
        );
    }
    for canal in NON_REGULABLES {
        assert!(
            !ligne_de(&vue, canal).regulable,
            "« {canal} » est sur le Kraken, dont le firmware régule déjà (hors scope de #99)"
        );
    }

    // Les modes employés se comportent comme #112 les a figés — sans quoi les tests de composition
    // ne composeraient rien.
    for mode in MODES_OUVERTS {
        assert!(
            !ligne_de(&telemetrie(mode, false, &[], &[]), SMART_1).regule_seul(),
            "« {mode} » ne verrouille pas (#112) : c'est ce qui rend la bascule de régulation \
             exerçable dessus"
        );
    }
    assert!(
        ligne_de(&telemetrie(NON_PILOTE, false, &[], &[]), POMPE).regule_seul(),
        "« {NON_PILOTE} » verrouille (#112) : c'est l'autre moitié de la composition testée ici"
    );

    // Et une ligne `regule` marque bien le canal qu'elle nomme, sinon rien de ce qui suit ne
    // vérifie ce qu'il prétend.
    let regulee = telemetrie(MANUEL, false, &[SMART_1], &[]);
    assert!(ligne_de(&regulee, SMART_1).sous_regulation);
    assert!(!ligne_de(&regulee, SMART_2).sous_regulation);
}

// ---------------------------------------------------------------------------
// 1 — l'état de régulation rendu par le socket arrive dans les lignes de canal
// ---------------------------------------------------------------------------

#[test]
fn l_etat_de_regulation_rendu_par_le_socket_se_retrouve_dans_les_lignes_de_canal() {
    // Test d'intention n° 1 de l'issue. Critère d'acceptation n° 3, première moitié : « l'état de
    // régulation affiché est celui que le démon rend ».
    //
    // Le tour porte la réponse relevée sur SHYNAEL — `courbe …`, puis une ligne `regule` par canal
    // régulé — et chacun des cinq canaux doit s'y retrouver du bon côté. Deux canaux régulés sur
    // trois régulables : un test à « tous » ou « aucun » passerait sur une implémentation qui
    // recopierait `regulable` dans `sous_regulation`.
    let vue = telemetrie(MANUEL, false, &[SMART_1, SMART_3], &[]);

    for canal in [SMART_1, SMART_3] {
        assert!(
            ligne_de(&vue, canal).sous_regulation,
            "le démon rend « regule {canal} » : sa ligne doit le montrer sous régulation"
        );
    }
    for canal in [SMART_2, POMPE, VENTILO_KRAKEN] {
        assert!(
            !ligne_de(&vue, canal).sous_regulation,
            "le démon ne rend aucune ligne « regule {canal} » : sa ligne ne doit pas le montrer \
             sous régulation"
        );
    }

    // Et la liste des canaux n'a pas bougé : les lignes `regule` s'ajoutent aux `chan`, elles ne
    // les remplacent pas.
    assert_eq!(
        noms(&vue),
        BANC.map(|(canal, _, _)| canal.to_owned()).to_vec(),
        "la réponse à `regule` ne doit ni retirer ni réordonner les lignes de canal (#100)"
    );
}

#[test]
fn toutes_les_combinaisons_de_canaux_regules_se_lisent() {
    // La même règle, sur les trente-deux sous-ensembles du banc plutôt que sur celui qui arrange.
    // Un tri qui se tromperait d'un cran — le canal suivant, le précédent — passerait le test
    // ci-dessus et pas celui-ci.
    let noms_du_banc: Vec<&str> = BANC.iter().map(|(canal, _, _)| *canal).collect();

    for combinaison in 0..(1u32 << BANC.len()) {
        let regules: Vec<&str> = noms_du_banc
            .iter()
            .enumerate()
            .filter(|(i, _)| combinaison & (1 << *i) != 0)
            .map(|(_, canal)| *canal)
            .collect();

        let vue = telemetrie(MANUEL, false, &regules, &[]);
        for canal in &noms_du_banc {
            assert_eq!(
                ligne_de(&vue, canal).sous_regulation,
                regules.contains(canal),
                "avec {regules:?} régulés, « {canal} » se lit du mauvais côté"
            );
        }
    }
}

#[test]
fn une_ligne_regule_sur_un_canal_jamais_vu_ne_fabrique_pas_de_ligne() {
    // Une ligne `regule` dit l'état d'un canal, elle n'en déclare pas l'existence. Une
    // implémentation qui fabriquerait une ligne à la volée ferait apparaître dans l'onglet une
    // rangée sans mesure, sans mode et sans poignée — et rien ne dirait d'où elle sort.
    //
    // C'est la règle que #100 a posée pour les lignes qui ne sont pas de la télémétrie : elles ne
    // font ni canal ni sonde.
    const INCONNU: &str = "nct6687:fan-7";

    let mut tri = Tri::nouveau();
    tri.poser(&tour(MANUEL, false, &[], &[]));
    let mut lignes = tour(MANUEL, false, &[SMART_1], &[]);
    lignes.push(regule(INCONNU));
    let vue = tri.poser(&lignes);

    assert_eq!(
        noms(&vue),
        BANC.map(|(canal, _, _)| canal.to_owned()).to_vec(),
        "« {INCONNU} » n'a jamais eu de ligne `chan` : il n'entre pas dans la liste des canaux"
    );
}

// ---------------------------------------------------------------------------
// 2 — l'état affiché est celui du démon, jamais une mémoire de fenêtre
// ---------------------------------------------------------------------------

#[test]
fn un_canal_que_le_demon_cesse_de_lister_n_est_plus_montre_comme_regule() {
    // Critère d'acceptation n° 3 : « l'état de régulation affiché est celui que le démon rend,
    // **jamais une mémoire de fenêtre** ».
    //
    // C'est le critère qui distingue cette issue de la pastille de profil, qui est justement une
    // mémoire de fenêtre — le README l'explique. Et la faute visée est naturelle à écrire : cocher
    // la case, retenir qu'on l'a cochée, et l'afficher cochée. Un `regule off` posé par le socket
    // depuis un autre client, ou refusé par le démon, laisserait alors la fenêtre en dire le
    // contraire indéfiniment.
    let mut tri = Tri::nouveau();
    tri.poser(&tour(MANUEL, false, &[], &[]));

    let pris = tri.poser(&tour(MANUEL, false, &[SMART_1], &[]));
    assert!(
        ligne_de(&pris, SMART_1).sous_regulation,
        "le démon liste « {SMART_1} » : la fenêtre le montre régulé"
    );

    // La fenêtre envoie ce qu'elle veut : ça ne change rien à ce qu'elle affiche.
    assert_eq!(
        ligne_de(&pris, SMART_1).ordre_de_regulation(true),
        Some(ordre_attendu(SMART_1, true)),
        "la bascule produit bien un ordre"
    );

    let rendu = tri.poser(&tour(MANUEL, false, &[], &[]));
    assert!(
        !ligne_de(&rendu, SMART_1).sous_regulation,
        "le démon ne liste plus « {SMART_1} » : la fenêtre cesse de le montrer régulé, quoi qu'elle \
         vienne d'envoyer"
    );
}

#[test]
fn un_canal_que_la_fenetre_n_a_jamais_active_se_montre_regule_si_le_demon_le_dit() {
    // L'autre sens de la même règle, et il compte autant : la régulation survit à un redémarrage
    // (#99, `/var/lib/reverb/regulation.conf`). Une fenêtre qui n'afficherait comme régulés que les
    // canaux qu'elle a elle-même activés montrerait, à chaque ouverture, trois canaux libres qui
    // sont en réalité régulés depuis le démarrage du démon.
    let mut tri = Tri::nouveau();
    let premier = tri.poser(&tour(MANUEL, false, &[SMART_1, SMART_2, SMART_3], &[]));

    for canal in REGULABLES {
        assert!(
            ligne_de(&premier, canal).sous_regulation,
            "« {canal} » est régulé depuis le démarrage du démon : la fenêtre le montre dès son \
             premier tour, sans qu'aucun geste ait eu lieu"
        );
    }
}

#[test]
fn l_ordre_de_bascule_ne_regarde_pas_l_etat_courant() {
    // Test d'intention n° 3 : « activer puis couper la régulation d'un canal émet exactement deux
    // ordres ».
    //
    // ⚠️ C'est le corollaire direct de « jamais une mémoire de fenêtre » : entre le clic qui active
    // et celui qui coupe, la ligne dit **toujours** `sous_regulation = false`, puisqu'elle vient du
    // démon et que le tour suivant n'est pas encore arrivé. Une bascule qui filtrerait sur l'état
    // courant — « il est déjà coupé, rien à envoyer » — avalerait donc le second ordre, et le canal
    // resterait régulé sans que rien ne l'explique.
    for canal in REGULABLES {
        for mode in MODES_OUVERTS {
            for etat_rendu in [&[][..], &[canal][..]] {
                let vue = telemetrie(mode, false, etat_rendu, &[]);
                let ligne = ligne_de(&vue, canal);

                let activation = ligne.ordre_de_regulation(true);
                let coupure = ligne.ordre_de_regulation(false);

                assert_eq!(
                    activation,
                    Some(ordre_attendu(canal, true)),
                    "« {canal} » en « {mode} » : la bascule vers « on » produit son ordre, que le \
                     démon l'ait déjà listé ou non"
                );
                assert_eq!(
                    coupure,
                    Some(ordre_attendu(canal, false)),
                    "« {canal} » en « {mode} » : la bascule vers « off » produit son ordre, que le \
                     démon l'ait déjà listé ou non"
                );
                assert_ne!(
                    activation, coupure,
                    "les deux gestes doivent produire deux ordres distincts"
                );
            }
        }
    }
}

#[test]
fn les_deux_ordres_s_ecrivent_comme_le_socket_les_attend() {
    // La forme des deux lignes, telle que #99 l'a livrée et telle qu'on la tape à la main :
    // `regule <canal> on` et `regule <canal> off`. La fenêtre n'a aucun verbe à inventer.
    let vue = telemetrie(MANUEL, false, &[], &[]);
    let ligne = ligne_de(&vue, SMART_1);

    assert_eq!(
        ligne
            .ordre_de_regulation(true)
            .as_ref()
            .map(encode_request)
            .as_deref(),
        Some("regule nzxtsmart2:fan-1 on"),
    );
    assert_eq!(
        ligne
            .ordre_de_regulation(false)
            .as_ref()
            .map(encode_request)
            .as_deref(),
        Some("regule nzxtsmart2:fan-1 off"),
    );
}

#[test]
fn un_canal_regulable_peut_etre_mis_sous_regulation_et_en_etre_sorti() {
    // Critère d'acceptation n° 1 : « un canal dont le pilote n'a pas de mode automatique peut être
    // mis sous régulation depuis la fenêtre, et en être sorti ».
    //
    // Les trois canaux, sous les deux valeurs de `sait_faire_auto` : le drapeau ne doit entrer dans
    // aucune décision (#112, approche technique).
    for canal in REGULABLES {
        for auto in DRAPEAUX_AUTO {
            let vue = telemetrie(MANUEL, auto, &[], &[]);
            let ligne = ligne_de(&vue, canal);
            for activer in [true, false] {
                assert_eq!(
                    ligne.ordre_de_regulation(activer),
                    Some(ordre_attendu(canal, activer)),
                    "« {canal} » (sait_faire_auto={auto}) doit pouvoir être {} : c'est le réglage \
                     qui décide du bruit et de la température du poste",
                    if activer { "pris" } else { "rendu" }
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — le Kraken ne propose pas la régulation
// ---------------------------------------------------------------------------

#[test]
fn un_canal_du_kraken_ne_produit_aucun_ordre_regule() {
    // Test d'intention n° 6 et critère d'acceptation n° 2 : « un canal du Kraken ne propose pas la
    // régulation — son firmware s'en charge ».
    //
    // ⚠️ Et la décision se prend sur le **fait matériel** porté par la ligne, jamais sur le nom du
    // contrôleur : c'est la règle que #112 a posée pour le verrou, et pour la même raison. Coder
    // « kraken » donnerait aujourd'hui le bon résultat et casserait en silence au premier pilote qui
    // change. Les trois modes et les deux drapeaux sont donc essayés.
    for canal in NON_REGULABLES {
        for mode in [MANUEL, NON_PILOTE, PLEIN_REGIME] {
            for auto in DRAPEAUX_AUTO {
                let vue = telemetrie(mode, auto, &[], &[]);
                let ligne = ligne_de(&vue, canal);
                for activer in [true, false] {
                    assert_eq!(
                        ligne.ordre_de_regulation(activer),
                        None,
                        "« {canal} » en « {mode} » (sait_faire_auto={auto}) : son firmware régule \
                         déjà, et #99 le met hors scope — la fenêtre n'a rien à proposer"
                    );
                }
            }
        }
    }
}

#[test]
fn la_decision_suit_le_drapeau_de_la_ligne_et_non_le_nom_du_controleur() {
    // La forme directe de l'avertissement ci-dessus : la même ligne, le même canal, et le drapeau
    // retourné. Un canal du Kraken déclaré régulable doit proposer la régulation ; un `nzxtsmart2`
    // déclaré non régulable ne doit rien proposer.
    //
    // Aucune des deux lignes ne correspond à SHYNAEL — c'est le but. Ce sont elles qui attrapent une
    // implémentation qui aurait lu le nom du contrôleur au lieu du drapeau.
    for (canal, regulable) in [(POMPE, true), (SMART_1, false)] {
        let mut tri = Tri::nouveau();
        let vue = tri.poser(&[chan(canal, MANUEL, false, regulable), ligne("end")]);
        let ligne = ligne_de(&vue, canal);

        assert_eq!(
            ligne.regulable, regulable,
            "« {canal} » : le drapeau de la ligne est ce qui compte, pas le pilote qu'on croit \
             reconnaître dans son nom"
        );
        assert_eq!(
            ligne.ordre_de_regulation(true).is_some(),
            regulable,
            "« {canal} » déclaré regulable={regulable} : la bascule suit le drapeau"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — `regulable` et `sait_faire_auto` sont deux faits, et aucun ne déduit l'autre
// ---------------------------------------------------------------------------

#[test]
fn regulable_et_sait_faire_auto_sont_independants() {
    // Les deux drapeaux disent deux choses différentes, et il faut les deux :
    //
    // — `sait_faire_auto` dit « le bouton auto marcherait **maintenant** » — depuis #97, c'est
    //   « le pilote sait faire auto **et** une courbe a été posée », donc toujours `non` sur cette
    //   machine (`docs/VENTILATEURS.md`) ;
    // — `regulable` dit « ce matériel **ne sait pas** se réguler seul », donc l'hôte doit le faire.
    //
    // Un canal peut être les deux faux — c'est le cas des deux canaux du Kraken aujourd'hui : leur
    // pilote a un mode automatique, mais aucune courbe n'a été posée. Les quatre combinaisons sont
    // donc essayées, et chaque champ doit rendre ce que la ligne a dit, indépendamment de l'autre.
    for regulable in [true, false] {
        for auto in DRAPEAUX_AUTO {
            let mut tri = Tri::nouveau();
            let vue = tri.poser(&[chan(SMART_1, MANUEL, auto, regulable), ligne("end")]);
            let ligne = ligne_de(&vue, SMART_1);

            assert_eq!(
                ligne.regulable, regulable,
                "regulable={regulable}, sait_faire_auto={auto} : le premier ne doit pas se déduire \
                 du second"
            );
            assert_eq!(
                ligne.sait_faire_auto, auto,
                "regulable={regulable}, sait_faire_auto={auto} : le second ne doit pas se déduire \
                 du premier"
            );
            assert_eq!(
                ligne.ordre_de_regulation(true).is_some(),
                regulable,
                "regulable={regulable}, sait_faire_auto={auto} : la bascule suit `regulable`, et \
                 lui seul"
            );
        }
    }
}

#[test]
fn regulable_n_entre_pas_dans_le_verrou_de_112() {
    // Non-régression vis-à-vis de #112 : « le déclencheur est le mode, pas le nom du contrôleur »,
    // et pas davantage un drapeau. `regule_seul` doit rendre exactement la même chose sous les
    // quatre combinaisons — sinon #113 aurait déplacé une décision que #112 tient.
    for mode in [MANUEL, NON_PILOTE, PLEIN_REGIME] {
        let attendu = {
            let mut tri = Tri::nouveau();
            let vue = tri.poser(&[chan(SMART_1, mode, false, false), ligne("end")]);
            ligne_de(&vue, SMART_1).regule_seul()
        };
        for regulable in [true, false] {
            for auto in DRAPEAUX_AUTO {
                let mut tri = Tri::nouveau();
                let vue = tri.poser(&[chan(SMART_1, mode, auto, regulable), ligne("end")]);
                assert_eq!(
                    ligne_de(&vue, SMART_1).regule_seul(),
                    attendu,
                    "en « {mode} », le verrou doit se décider sur le mode seul (#112) — \
                     regulable={regulable}, sait_faire_auto={auto}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — une barre régulée ne commande pas
// ---------------------------------------------------------------------------

#[test]
fn un_canal_regule_n_emet_pas_de_consigne_manuelle_quand_sa_barre_bouge() {
    // Test d'intention n° 2 de l'issue, et comportement attendu : « quand elle est active, la barre
    // montre la consigne calculée et n'est pas manipulable à la main : la tirer reprendrait le canal
    // (#99), ce qui doit rester un geste voulu et non un glissement ».
    //
    // ⚠️ Les quatre consignes sont essayées, `Auto` comprise. Ce n'est pas du zèle : c'est le geste
    // du 2026-08-15 — un clic sur « auto » a mis la consigne de la pompe à 0 % (#97).
    for canal in REGULABLES {
        for mode in MODES_OUVERTS {
            let vue = telemetrie(mode, false, &[canal], &[]);
            let ligne = ligne_de(&vue, canal);
            assert!(ligne.sous_regulation, "le banc doit bien le montrer régulé");

            for consigne in CONSIGNES {
                for deverrouille in [false, true] {
                    assert_eq!(
                        ligne.commande_effective(consigne(), deverrouille),
                        None,
                        "« {canal} » est régulé par le démon : {:?} ne doit pas partir. La tirer \
                         reprendrait le canal, et la régulation disparaîtrait sans explication (#99)",
                        consigne()
                    );
                }
            }
        }
    }
}

#[test]
fn un_canal_regule_ne_se_deverrouille_pas() {
    // L'issue le pose comme la différence de fond avec #112 : « le canal régulé et le canal
    // verrouillé ne sont pas le même état, et les deux grisent une barre pour des raisons
    // opposées ». Le verrou est une **confirmation** — un geste l'ouvre. La régulation est une
    // **prise en charge** — seul `regule off` la rend.
    //
    // Une implémentation qui ferait passer la régulation par le même déverrouillage rendrait le
    // geste « ouvrir le cadenas » suffisant pour reprendre un canal à la boucle du démon, sans que
    // rien ne dise que c'est ce qui vient d'arriver.
    let vue = telemetrie(MANUEL, false, &[SMART_1], &[]);
    let ligne = ligne_de(&vue, SMART_1);

    assert!(
        !ligne.regule_seul(),
        "un canal régulé par l'hôte ne régule pas seul : il n'a pas de cadenas (#112)"
    );
    assert_eq!(
        ligne.commande_effective(FanAction::Pwm(80), true),
        None,
        "verrou ouvert ou non, un canal régulé reste inerte : ce n'est pas un verrou qu'on ouvre, \
         c'est une prise en charge qu'on rend"
    );

    // Et le canal verrouillé, lui, s'ouvre bien : les deux états se distinguent par leur réponse au
    // geste, et non seulement par un booléen.
    let verrouille = telemetrie(NON_PILOTE, false, &[], &[]);
    let pompe = ligne_de(&verrouille, POMPE);
    assert!(pompe.regule_seul(), "la pompe lit « {NON_PILOTE} » (#112)");
    assert!(
        !pompe.sous_regulation,
        "et personne ne la régule depuis l'hôte : #99 la met hors scope"
    );
    assert_eq!(
        pompe.commande_effective(FanAction::Pwm(80), false),
        None,
        "verrou fermé, elle est inerte (#112)"
    );
    assert_eq!(
        pompe.commande_effective(FanAction::Pwm(80), true),
        Some(consigne_attendue(POMPE, FanAction::Pwm(80))),
        "verrou ouvert, elle commande — c'est là que les deux états divergent"
    );
}

#[test]
fn un_canal_regule_montre_toujours_la_consigne_que_le_demon_a_ecrite() {
    // Critère d'acceptation n° 6, première moitié : « un canal régulé montre la consigne calculée ».
    //
    // Il n'y a rien de neuf à calculer : la consigne d'un canal régulé, c'est le `pwm` que le démon
    // vient de lui écrire, et qu'il republie dans sa ligne `chan`. Ce qui se vérifie ici, c'est que
    // la prise en charge ne l'efface pas — une ligne régulée dont le `pwm` serait `None` s'afficherait
    // « -- % » (#102) alors que la valeur est parfaitement connue.
    let vue = telemetrie(MANUEL, false, &[SMART_1], &[]);
    let ligne = ligne_de(&vue, SMART_1);

    assert!(ligne.sous_regulation);
    assert_eq!(
        ligne.pwm,
        Some(PWM),
        "la consigne calculée par la boucle du démon est ce que la ligne `chan` republie : la \
         régulation ne doit pas la faire disparaître"
    );
    assert_eq!(ligne.rpm, Some(RPM), "ni le régime mesuré");
    assert!(ligne.lisible, "ni marquer la ligne illisible (#100)");
}

#[test]
fn hors_regulation_commande_effective_est_exactement_celle_de_112() {
    // Garde-fou de non-régression : `commande_verrouillable` est figée par `spec_verrou_canal.rs`,
    // et #113 ne doit pas la déplacer. Sur une ligne que la régulation ne concerne pas, les deux
    // méthodes doivent rendre exactement la même chose — sinon la fenêtre aurait deux chemins
    // d'émission qui divergeraient, et le second n'aurait aucune raison d'être le bon.
    //
    // C'est la forme que #112 a lui-même employée vis-à-vis de #100.
    for mode in [MANUEL, NON_PILOTE, PLEIN_REGIME] {
        let vue = telemetrie(mode, false, &[], &[]);
        for (canal, _, _) in BANC {
            let ligne = ligne_de(&vue, canal);
            assert!(!ligne.sous_regulation);

            for consigne in CONSIGNES {
                for deverrouille in [false, true] {
                    assert_eq!(
                        ligne.commande_effective(consigne(), deverrouille),
                        ligne.commande_verrouillable(consigne(), deverrouille),
                        "« {canal} » en « {mode} » n'est pas régulé : la commande doit être celle \
                         que #112 a figée, verrou {}",
                        if deverrouille { "ouvert" } else { "fermé" }
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 6 — la composition avec la quarantaine de #100
// ---------------------------------------------------------------------------

#[test]
fn un_canal_illisible_n_emet_aucun_ordre_regule() {
    // Composition avec #100, que l'issue appelle par son critère n° 4 côté verrou et qui vaut ici
    // pour la même raison : **on ne prend pas en charge un canal qui ne répond pas**.
    //
    // Elle doit être testée explicitement parce qu'elle n'est impliquée par aucune des deux règles :
    // #100 dit « illisible ⇒ inerte » pour les consignes, #113 dit « non régulable ⇒ pas de
    // bascule », et une implémentation qui n'écrirait que la seconde laisserait la bascule vivante
    // devant un canal muet. Le Kraken part périodiquement en quarantaine, et les `nzxtsmart2` avec
    // lui quand le contrôleur se tait.
    for canal in REGULABLES {
        for regules in [&[][..], &[canal][..]] {
            let vue = telemetrie(MANUEL, false, regules, &[canal]);
            let ligne = ligne_de(&vue, canal);
            assert!(
                !ligne.lisible,
                "le banc doit bien mettre « {canal} » en quarantaine (#100), sinon rien de ce qui \
                 suit ne vérifie la composition"
            );

            for activer in [true, false] {
                assert_eq!(
                    ligne.ordre_de_regulation(activer),
                    None,
                    "« {canal} » ne répond pas : ni le prendre ni le rendre n'a de sens tant qu'on \
                     ne sait pas ce qu'il fait"
                );
            }
        }
    }
}

#[test]
fn un_canal_illisible_reste_inerte_regule_ou_non() {
    // L'autre moitié de la même composition, du côté de la barre. #100 : « les commandes d'un canal
    // illisible n'émettent rien vers le démon » — et cela reste vrai qu'il soit régulé, verrouillé,
    // ou les deux.
    //
    // La faute visée est une implémentation qui écrirait « inerte ⇔ régulé ou verrouillé fermé » :
    // elle satisferait #113 en cassant #100, et la barre d'un canal muet redeviendrait manipulable
    // au moment précis où le démon paierait cinq secondes pour la lecture qui l'a fait écarter.
    for canal in REGULABLES {
        for regules in [&[][..], &[canal][..]] {
            let vue = telemetrie(MANUEL, false, regules, &[canal]);
            let ligne = ligne_de(&vue, canal);
            for consigne in CONSIGNES {
                for deverrouille in [false, true] {
                    assert_eq!(
                        ligne.commande_effective(consigne(), deverrouille),
                        None,
                        "« {canal} » est en quarantaine : {:?} ne part pas, verrou {}, régulé {}",
                        consigne(),
                        if deverrouille { "ouvert" } else { "fermé" },
                        !regules.is_empty()
                    );
                }
            }
        }
    }
}

#[test]
fn un_canal_revenu_se_regule_de_nouveau() {
    // L'inertie d'une quarantaine est un état, pas une punition (#100), et la régulation ne doit pas
    // la rendre définitive. Une implémentation qui laisserait la bascule morte après le retour
    // rendrait les sept ventilateurs du boîtier irrégulables au premier hoquet du contrôleur — et
    // c'est exactement au retour d'un hoquet qu'on veut vérifier qu'ils sont bien repris.
    let mut tri = Tri::nouveau();
    tri.poser(&tour(MANUEL, false, &[SMART_1], &[]));
    tri.poser(&tour(MANUEL, false, &[SMART_1], &[SMART_1]));
    let vue = tri.poser(&tour(MANUEL, false, &[SMART_1], &[]));
    let ligne = ligne_de(&vue, SMART_1);

    assert!(ligne.lisible, "« {SMART_1} » répond de nouveau (#100)");
    assert!(
        ligne.sous_regulation,
        "et le démon le liste toujours comme régulé"
    );
    assert_eq!(
        ligne.ordre_de_regulation(false),
        Some(ordre_attendu(SMART_1, false)),
        "la bascule revient avec le canal"
    );
}

// ---------------------------------------------------------------------------
// 7 — la ligne `chan` d'un démon d'avant reste lisible
// ---------------------------------------------------------------------------

#[test]
fn une_ligne_chan_sans_le_drapeau_reste_lisible_et_ne_propose_rien() {
    // #100 a posé l'exigence, et elle vaut pour tout ajout au protocole : « un démon d'avant reste
    // lisible ». Elle a ici un second bénéficiaire, immédiat et vérifiable : les lignes à six jetons
    // qu'envoient `spec_canal_illisible.rs`, `spec_verrou_canal.rs` et `spec_barre_valeur.rs`.
    //
    // ⚠️ Un drapeau absent se lit **non**, et non « on ne sait pas ». Un démon qui ne dit rien ne
    // propose donc pas la régulation : montrer une bascule qui ne peut qu'échouer vaut moins que ne
    // pas la montrer — c'est déjà la règle du bouton « auto » (README, « ce que la fenêtre ne fait
    // pas »).
    for (canal, _, _) in BANC {
        let mut tri = Tri::nouveau();
        let vue = tri.poser(&[chan_d_avant(canal, MANUEL, false), ligne("end")]);
        let ligne = ligne_de(&vue, canal);

        assert!(
            ligne.lisible,
            "« {canal} » : une ligne `chan` à six jetons reste une ligne de canal valide"
        );
        assert!(
            !ligne.regulable,
            "« {canal} » : un démon qui ne dit rien du mode automatique de son pilote ne fait rien \
             proposer à la fenêtre"
        );
        assert_eq!(
            ligne.ordre_de_regulation(true),
            None,
            "« {canal} » : et la bascule ne produit donc aucun ordre"
        );
        assert_eq!(
            ligne.pwm,
            Some(PWM),
            "« {canal} » : les six champs d'avant continuent d'arriver entiers"
        );
    }
}

// ---------------------------------------------------------------------------
// 8 — la courbe que le démon rend arrive dans la télémétrie
// ---------------------------------------------------------------------------

#[test]
fn la_courbe_rendue_par_le_socket_arrive_dans_la_telemetrie() {
    // La fenêtre doit garnir son éditeur avec la courbe **du démon**, pas avec celle qu'elle sait
    // écrire : c'est le même critère que pour les canaux — « l'état affiché est celui que le démon
    // rend ». Une fenêtre qui afficherait `Courbe::defaut()` en dur montrerait la bonne courbe sur
    // une machine neuve et une courbe fausse sur la seule où le réglage a servi.
    let mut tri = Tri::nouveau();
    let mut lignes = tour(MANUEL, false, &[SMART_1], &[]);
    let vue = tri.poser(&lignes);
    assert_eq!(
        vue.courbe,
        Some(Courbe::defaut()),
        "le tour porte « courbe 35000:30 45000:60 50000:100 » : c'est la courbe par défaut de #99"
    );

    // Une courbe réglée doit arriver telle quelle, sans quoi le test ci-dessus ne prouverait rien
    // d'autre que la présence d'une constante.
    lignes = tour(MANUEL, false, &[SMART_1], &[]);
    lignes.retain(|l| !matches!(l, ResponseLine::Courbe { .. }));
    lignes.push(courbe_rendue(&PALIERS_AUTRES));
    let reglee = tri.poser(&lignes);
    assert_eq!(
        reglee.courbe,
        Courbe::depuis(&PALIERS_AUTRES).ok(),
        "une courbe réglée arrive telle quelle : {PALIERS_AUTRES:?}"
    );
    assert_ne!(
        reglee.courbe,
        Some(Courbe::defaut()),
        "et elle diffère de celle par défaut, sinon ce test ne distinguerait rien"
    );
}

#[test]
fn un_tour_sans_ligne_de_courbe_n_en_invente_aucune() {
    // Le pendant du test précédent. Un tour où la réponse à `regule` n'est pas arrivée — le démon
    // vient de tomber, la ligne a été perdue — ne doit pas faire afficher une courbe plausible et
    // fausse. C'est la règle que le projet applique à toutes ses valeurs manquantes : « une sonde
    // muette affiche des tirets, jamais un zéro ni la dernière valeur connue » (README).
    let mut tri = Tri::nouveau();
    let mut lignes = tour(MANUEL, false, &[], &[]);
    lignes.retain(|l| !matches!(l, ResponseLine::Courbe { .. }));
    let vue = tri.poser(&lignes);

    assert_eq!(
        vue.courbe, None,
        "aucune ligne `courbe` dans ce tour : la fenêtre n'a pas de courbe à montrer, et le dire \
         vaut mieux que d'en dessiner une"
    );

    // Et les lignes de canal, elles, sont bien là : l'absence de courbe n'emporte pas le reste.
    assert_eq!(
        noms(&vue),
        BANC.map(|(canal, _, _)| canal.to_owned()).to_vec()
    );
}
