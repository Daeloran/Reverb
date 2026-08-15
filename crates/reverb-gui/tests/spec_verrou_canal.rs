//! Tests d'intention du verrou de canal et de l'aide sur les modes (issue #112).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #112 seule et les deux dont elle dépend —
//! #100 pour un canal illisible, #101 pour le libellé `non-piloté`. Rien de
//! `crates/reverb-gui/src/` ni de `crates/reverb-gui/ui/` n'a été lu pour les produire. Seules ont
//! été relevées les signatures publiques déjà en place, telles que `spec_canal_illisible.rs` (#100)
//! les a figées : `LigneCanal`, `Telemetrie`, `Tri`, et la ligne `chan` du protocole.
//!
//! À l'écriture de ce fichier, `regule_seul`, `commande_verrouillable`, `aide_du_mode` et
//! `aide_des_modes` **n'existent pas** : la compilation doit échouer sur eux, et sur eux seuls.
//! C'est la phase rouge.
//!
//! Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne touche un bus.
//!
//! ## Le défaut, tel que l'issue le décrit
//!
//! Un canal qui régule seul — le Kraken sur sa courbe d'usine — perd cette régulation dès qu'on
//! touche sa barre de consigne, **sans que rien ne le demande** : la fenêtre bascule le canal en
//! manuel avant d'écrire, et le canal n'y reviendra pas avant un arrêt complet de la machine
//! (`docs/VENTILATEURS.md`, « `pwm_enable = 0` ne rend pas le Kraken à son profil d'usine »).
//!
//! Ce n'est pas théorique. Le 2026-08-15, un clic sur « auto » a mis la consigne de la pompe à 0 %
//! (#97) ; le même jour, la mesure a montré la courbe d'usine parfaitement vivante — 35 % à 37 °C,
//! 60 % à 51 °C. **Ce qui régule bien est exactement ce qu'un geste distrait détruit.**
//!
//! ## Ce qu'on peut tester d'une fenêtre, et ce qu'on ne peut pas
//!
//! ⚠️ **Un cadenas ne s'écrit pas en assertions.** Ce qui se vérifie ici, c'est la **décision** :
//! une ligne entre, et l'on regarde si elle laisse partir une commande. C'est une fonction pure de
//! ce que le démon a répondu et de l'état du verrou, et elle n'a besoin d'aucune session graphique.
//!
//! Le grisé, le dessin du cadenas, la place du point d'interrogation et le fait que la mise en page
//! tienne dans la largeur du panneau se regardent sur une image, hors de ce fichier :
//! `REVERB_ONGLET=ventilos cargo run --release --example apercu -p reverb-gui`. Le critère
//! d'acceptation le dit lui-même — « la mise en page est vérifiée par l'exemple `apercu`, pas à
//! l'œil ».
//!
//! ⚠️ **La décision doit donc vivre dans la bibliothèque, pas dans `main.rs`** ni dans le `.slint` —
//! c'est la règle que #100 a posée pour le tri, et que #102 a posée pour la valeur affichée à côté
//! d'une barre : ce qui est du calcul se teste, ce qui est du dessin se regarde.
//!
//! ## Le contrat que ces tests figent
//!
//! ```ignore
//! // dans crates/reverb-gui/src/telemetrie.rs, à côté de `LigneCanal` (#100).
//!
//! impl LigneCanal {
//!     /// Ce canal régule-t-il seul ? — vrai en `non-piloté` et en `courbe-de-l'hôte`, les deux
//!     /// modes où quelque chose d'autre que l'hôte décide de la vitesse.
//!     pub fn regule_seul(&self) -> bool;
//!
//!     /// La requête qu'une consigne produit — `None` si le canal est illisible (#100) **ou**
//!     /// s'il régule seul et que le verrou n'a pas été ouvert (#112).
//!     pub fn commande_verrouillable(&self, action: FanAction, deverrouille: bool) -> Option<Request>;
//! }
//!
//! /// Le texte d'aide d'un libellé de mode — `None` si le libellé n'est pas l'un des six.
//! pub fn aide_du_mode(mode: &str) -> Option<&'static str>;
//!
//! /// Les six modes et leur aide, de quoi garnir le panneau que le point d'interrogation ouvre.
//! pub fn aide_des_modes() -> Vec<(&'static str, &'static str)>;
//! ```
//!
//! ### Pourquoi une seconde méthode plutôt que d'armer `commande`
//!
//! `commande` est déjà figée par #100 : « les commandes d'un canal illisible n'émettent rien », et
//! rien d'autre. `spec_canal_illisible.rs` lui envoie des lignes en mode `manual` — un jeton qui
//! n'est **aucun** des six d'aujourd'hui — et exige qu'elle rende `Some`. Y glisser le verrou
//! changerait donc le sens d'une fonction que des tests d'intention voisins tiennent déjà, et le
//! verrou n'est pas une propriété de la ligne : c'est une propriété de la ligne **et** d'un geste
//! que l'utilisateur vient de faire ou non.
//!
//! ### Pourquoi un booléen passé en argument plutôt qu'un état dans `LigneCanal`
//!
//! `LigneCanal` est reconstruite à chaque tour de `status`, une fois par seconde. Un verrou rangé
//! dedans se refermerait tout seul à la seconde suivante, au milieu du geste. L'ouverture est une
//! mémoire de fenêtre — comme la pastille du profil rappelé —, pas une donnée de télémétrie.
//!
//! ## Les cas que l'issue laisse ouverts, et ce que ces tests en font
//!
//! 1. **Les quatre modes qui ne verrouillent pas.** L'issue est explicite : « Un canal est
//!    verrouillé quand il est en `non-piloté` **ou** en `courbe-de-l'hôte` — les deux modes où
//!    quelque chose d'autre que l'hôte décide de la vitesse. » Les quatre autres sont donc figés
//!    ouverts, chacun pour sa raison, écrite au-dessus de son test.
//! 2. **`inconnu-N` porte un nombre.** La table ne peut pas énumérer 256 entrées, et elle n'aurait
//!    rien de différent à dire de chacune : le nombre est justement ce que personne ne sait
//!    interpréter. Ces tests figent donc une **famille** — le libellé générique `inconnu-N` dans la
//!    table, et un `aide_du_mode` qui répond la même chose pour tout `inconnu-<n>`.
//! 3. **Le drapeau `sait_faire_auto` ne doit jouer aucun rôle.** L'approche technique de l'issue le
//!    dit : « depuis #97 ce drapeau vaut *le pilote sait faire auto ET une courbe a été posée*, donc
//!    toujours `non`. Il ne dit plus rien du matériel. » Chaque décision est donc essayée sous les
//!    deux valeurs du drapeau.
//! 4. **Le nom du contrôleur ne doit jouer aucun rôle non plus.** « Le déclencheur est le mode, pas
//!    le nom du contrôleur […] sans coder *Kraken* nulle part, et sans mentir sur un canal du Kraken
//!    déjà passé en manuel, où il n'y a plus rien à protéger. » Chaque décision est donc essayée sur
//!    un canal du Kraken **et** sur un canal `nzxtsmart2`.
//!
//! ## Ce que ces tests ne figent pas, et pourquoi
//!
//! - **Le dessin** : le grisé, le cadenas, l'icône d'aide, la place du panneau. Cf. `apercu`.
//! - **Le geste qui déverrouille** — clic long, bascule, double clic. L'issue demande qu'il soit
//!   « explicite » ; ce fichier tient l'effet du geste, pas sa forme.
//! - **Le sort d'un jeton de mode que la fenêtre ne connaît pas.** L'issue énumère six libellés et
//!   n'en nomme que deux comme verrouillants ; elle ne dit rien d'un septième qu'un démon futur
//!   écrirait. Le figer ici inventerait une règle que l'issue n'a pas voulue — et la trancher dans
//!   un sens comme dans l'autre a un coût réel : verrouiller l'inconnu rendrait un mode nouveau
//!   inaccessible sans explication, ne pas le verrouiller laisserait passer un geste distrait sur
//!   une régulation qu'on ne sait pas encore nommer. À rouvrir le jour où un septième libellé
//!   existe.
//! - **Le `mode` d'une ligne illisible.** `spec_canal_illisible.rs` laisse ce point ouvert, et ce
//!   fichier ne le referme pas : il exige seulement qu'une ligne illisible reste inerte, ce qui est
//!   vrai quel que soit le mode qu'on lui laisse.
//! - **Le verrou des autres barres de la fenêtre** — teinte, vitesse, luminosité. L'issue les met
//!   hors scope : elles ne détruisent rien d'irréversible.
//! - **Le garde-fou en ligne de commande**, qui a son fichier : `reverb-cli/tests/spec_refus_de_consigne.rs`.

use reverb_gui::telemetrie::{LigneCanal, Tri, aide_des_modes, aide_du_mode};
use reverb_proto::ipc::{FanAction, Request, ResponseLine, parse_response_line};

// ---------------------------------------------------------------------------
// Les six libellés de la colonne MODE
// ---------------------------------------------------------------------------
//
// Recopiés de l'issue #112 : « La colonne MODE écrit `non-piloté`, `manuel`, `courbe-de-l'hôte`,
// `plein-régime-100%`, `inconnu-N`, `non-réglable` ». Ce sont les `Display` de `reverb_hw::hwmon::
// Mode`, qui traversent le socket dans le champ `mode` d'une ligne `chan`.
//
// ⚠️ Ils sont **recopiés** et non calculés : `reverb-gui` ne dépend pas de `reverb-hw`, et ne doit
// pas se mettre à en dépendre pour une colonne de texte. C'est la règle du projet appliquée à un
// libellé plutôt qu'à une trame — « les trames de référence des tests viennent des specs, recopiées
// telles quelles avec leur source ». Le jour où l'un de ces libellés change, ce fichier est
// l'endroit qui doit suivre.

/// `pwmN_enable` lu à `0` : le pilote **n'a jamais touché** ce canal, le périphérique exécute son
/// propre profil (#101). C'est le mode des deux canaux du Kraken sur SHYNAEL.
const NON_PILOTE: &str = "non-piloté";

/// `pwmN_enable` à `1` : la vitesse est celle que l'hôte a écrite.
const MANUEL: &str = "manuel";

/// `pwmN_enable` à `2` : le firmware exécute la courbe que **l'hôte** lui a posée.
const COURBE_DE_L_HOTE: &str = "courbe-de-l'hôte";

/// Ce qu'une **écriture** de `0` provoque : 100 % et la barre lâchée. Une intention, jamais un état
/// observable (#101) — le distinguer de `non-piloté` est tout l'objet de cette issue-là.
const PLEIN_REGIME: &str = "plein-régime-100%";

/// Une valeur de `pwmN_enable` que l'hôte ne sait pas interpréter. Le libellé porte le nombre lu.
const INCONNU: &str = "inconnu-7";

/// Le libellé sous lequel la table d'aide nomme la **famille** `inconnu-<n>`.
///
/// C'est la graphie que l'issue emploie elle-même quand elle énumère les six libellés. Une table
/// qui listerait `inconnu-0`, `inconnu-1`, … dirait 256 fois la même chose ; une table qui
/// n'exposerait aucune entrée laisserait le sixième mode sans aide, alors que c'est justement celui
/// qu'on ne peut pas deviner.
const INCONNU_GENERIQUE: &str = "inconnu-N";

/// La source ne porte aucun `pwmN_enable` — le cas de `nct6687`, qui « n'expose **aucun**
/// `pwm*_enable` » (`docs/VENTILATEURS.md`).
const NON_REGLABLE: &str = "non-réglable";

/// Les deux modes que l'issue nomme verrouillants.
const MODES_QUI_VERROUILLENT: [&str; 2] = [NON_PILOTE, COURBE_DE_L_HOTE];

/// Les quatre autres, figés ouverts.
const MODES_QUI_NE_VERROUILLENT_PAS: [&str; 4] = [MANUEL, PLEIN_REGIME, INCONNU, NON_REGLABLE];

/// Les six libellés que la table d'aide doit couvrir, la famille `inconnu-<n>` sous son nom
/// générique.
const LES_SIX: [&str; 6] = [
    NON_PILOTE,
    MANUEL,
    COURBE_DE_L_HOTE,
    PLEIN_REGIME,
    INCONNU_GENERIQUE,
    NON_REGLABLE,
];

/// Le préfixe commun à toute la famille `inconnu-<n>`.
const PREFIXE_INCONNU: &str = "inconnu-";

/// Des jetons que la colonne MODE n'écrit pas, et pour lesquels l'aide n'a rien à dire.
///
/// ⚠️ La liste porte exprès `non-pilote` sans accent, `Manuel` capitalisé et `manual` en anglais :
/// une recherche tolérante les accepterait, et masquerait donc le jour où le libellé du démon
/// dérive. Le libellé arrive **verbatim** du socket ; le reconnaître approximativement, c'est
/// s'interdire de voir qu'on ne le reconnaît plus.
///
/// La chaîne vide en fait partie : c'est ce que porte une ligne illisible, et une aide affichée
/// sous un mode qu'on n'a pas pu lire serait une aide sur rien.
const LIBELLES_ETRANGERS: [&str; 8] = [
    "",
    " ",
    "manual",
    "Manuel",
    "non-pilote",
    "pwm",
    "firmware",
    "courbe",
];

/// La longueur au-dessous de laquelle un texte d'aide ne peut pas tenir sa promesse.
///
/// L'issue exige que chaque texte dise **qui décide de la vitesse** et **ce qu'on perd en y
/// touchant**. Deux informations ne tiennent pas en un mot. Le seuil est volontairement bas — bien
/// en dessous de n'importe quelle phrase honnête, bien au-dessus d'un libellé recopié.
const LONGUEUR_MINIMALE_D_UNE_AIDE: usize = 40;

// ---------------------------------------------------------------------------
// Le banc : deux contrôleurs, pour que le nom n'entre jamais dans la décision
// ---------------------------------------------------------------------------

/// Le canal le plus critique de la machine, et celui qui a motivé l'issue.
const POMPE: &str = "kraken2023elite:pump-speed";

/// Son voisin sur le même contrôleur.
const VENTILO_KRAKEN: &str = "kraken2023elite:fan-speed";

/// Un canal d'un tout autre pilote. Sur SHYNAEL il lit `manuel` — mais rien ne l'y oblige, et c'est
/// exactement ce que « le déclencheur est le mode, pas le nom du contrôleur » veut dire.
const SMART: &str = "nzxtsmart2:fan-1";

/// Les trois canaux du banc, avec la position que le démon publie pour chacun.
const BANC: [(&str, &str); 3] = [
    (POMPE, "-"),
    (VENTILO_KRAKEN, "-"),
    (SMART, "radiateur-haut"),
];

/// Les deux valeurs du drapeau `sait_faire_auto`, tel que la ligne `chan` l'écrit.
///
/// L'issue interdit de s'appuyer dessus : chaque décision est donc essayée sous les deux.
const DRAPEAUX_AUTO: [&str; 2] = ["oui", "non"];

/// Le régime et la consigne que les lignes du banc rendent — ceux du relevé du 2026-08-15, Kraken
/// jamais touché depuis le démarrage : `pwm1_enable = 0`, `pwm1 = 77 (30 %)`, pompe 1357 tr/min.
const RPM: u32 = 1_357;
const PWM: u8 = 30;

/// La raison d'une quarantaine, telle que le démon l'écrit (#88, #100).
const RAISON: &str = "Connection timed out (os error 110)";

/// Les quatre consignes qu'une poignée de ventilateur peut produire.
///
/// Ce sont des **fabriques** et non des valeurs, comme dans `spec_canal_illisible.rs` : `FanAction`
/// n'a pas à être `Copy` pour que ces tests compilent, et une consigne consommée par la commande
/// doit pouvoir être refabriquée à l'identique pour écrire l'attendu.
const CONSIGNES: [fn() -> FanAction; 4] = [
    || FanAction::Auto,
    || FanAction::Pwm(0),
    || FanAction::Pwm(50),
    || FanAction::Pwm(100),
];

// ---------------------------------------------------------------------------
// Les lignes du démon, écrites comme il les écrit
// ---------------------------------------------------------------------------

/// Décode une ligne de réponse, ou dit laquelle a été mal écrite.
fn ligne(texte: &str) -> ResponseLine {
    parse_response_line(texte).unwrap_or_else(|erreur| {
        panic!("« {texte} » doit être une ligne de réponse valide, refusée : {erreur:?}")
    })
}

/// La position que le démon publie pour un canal du banc.
fn position(canal: &str) -> &'static str {
    BANC.iter()
        .find(|(nom, _)| *nom == canal)
        .map(|(_, position)| *position)
        .unwrap_or_else(|| panic!("« {canal} » n'est pas un canal du banc"))
}

/// La ligne `chan` d'un canal qui répond : `chan <canal> <position> <rpm> <pwm> <mode> <oui|non>`.
fn chan(canal: &str, mode: &str, auto: &str) -> ResponseLine {
    ligne(&format!(
        "chan {canal} {} {RPM} {PWM} {mode} {auto}",
        position(canal)
    ))
}

/// La ligne de canal que la fenêtre montre pour un `chan` donné.
fn ligne_de(canal: &str, mode: &str, auto: &str) -> LigneCanal {
    let mut tri = Tri::nouveau();
    let vue = tri.poser(&[chan(canal, mode, auto), ligne("end")]);
    vue.canaux.into_iter().next().unwrap_or_else(|| {
        panic!("« {canal} » a répondu : la fenêtre doit lui donner une ligne (#100)")
    })
}

/// La ligne d'un canal que le démon ne sait plus lire, après l'avoir vu répondre dans ce mode-là.
///
/// Deux tours, parce que #100 laisse ouvert le sort d'un canal muet **dès le premier** : le faire
/// voir une fois lisible met ce fichier du bon côté de ce que l'issue voisine garantit.
fn ligne_illisible_de(canal: &str, mode_d_avant: &str) -> LigneCanal {
    let mut tri = Tri::nouveau();
    tri.poser(&[chan(canal, mode_d_avant, "oui"), ligne("end")]);
    let vue = tri.poser(&[ligne(&format!("unreadable {canal} {RAISON}")), ligne("end")]);
    let en_quarantaine = vue
        .canaux
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("« {canal} » garde sa ligne en quarantaine (#100)"));
    assert!(
        !en_quarantaine.lisible,
        "« {canal} » ne répond plus : sa ligne doit être marquée illisible (#100), sinon rien de \
         ce qui suit ne vérifie la composition des deux règles"
    );
    en_quarantaine
}

/// La requête qu'une consigne doit produire quand rien ne l'arrête.
fn requete(canal: &str, action: FanAction) -> Request {
    Request::Fan {
        channel: canal.to_owned(),
        action,
    }
}

/// L'aide d'un libellé, ou l'échec qui dit lequel n'en a pas.
fn aide(mode: &str) -> &'static str {
    aide_du_mode(mode).unwrap_or_else(|| {
        panic!(
            "« {mode} » est l'un des six libellés de la colonne MODE : il doit avoir une aide. \
             L'issue #112 demande une aide qui décrive **les six** — trois d'entre eux demandent \
             un contexte que seul `docs/VENTILATEURS.md` porte aujourd'hui."
        )
    })
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tout ce qui suit suppose que les six libellés diffèrent, qu'aucun ne porte d'espace, et que
    // les deux graphies de la famille `inconnu` partagent bien leur préfixe. Si l'un de ces repères
    // se dégradait, plusieurs tests deviendraient vrais sans rien vérifier — et personne ne le
    // verrait. Ce test est là pour que la panne soit ici.
    let tous = [
        NON_PILOTE,
        MANUEL,
        COURBE_DE_L_HOTE,
        PLEIN_REGIME,
        INCONNU,
        INCONNU_GENERIQUE,
        NON_REGLABLE,
    ];
    for (i, libelle) in tous.iter().enumerate() {
        assert!(
            !libelle.chars().any(char::is_whitespace),
            "« {libelle} » porte une espace : la ligne `chan` gagnerait un jeton et son arité \
             deviendrait indécidable (#101)"
        );
        for autre in tous.iter().skip(i + 1) {
            assert_ne!(
                libelle, autre,
                "deux libellés identiques : les tests qui les opposent ne vérifieraient rien"
            );
        }
    }

    assert!(
        INCONNU.starts_with(PREFIXE_INCONNU) && INCONNU_GENERIQUE.starts_with(PREFIXE_INCONNU),
        "« {INCONNU} » et « {INCONNU_GENERIQUE} » doivent partager le préfixe « {PREFIXE_INCONNU} », \
         sinon la famille testée n'est pas celle que la table nomme"
    );

    assert_eq!(
        MODES_QUI_VERROUILLENT.len() + MODES_QUI_NE_VERROUILLENT_PAS.len(),
        LES_SIX.len(),
        "les deux listes doivent partitionner les six libellés : un mode oublié ne serait testé \
         d'aucun côté"
    );

    // Et le banc doit bien porter deux contrôleurs, sans quoi « le déclencheur est le mode, pas le
    // nom du contrôleur » ne se vérifierait pas.
    assert!(
        BANC.iter().any(|(nom, _)| nom.starts_with("kraken"))
            && BANC.iter().any(|(nom, _)| nom.starts_with("nzxtsmart2")),
        "le banc doit porter un canal de chaque contrôleur"
    );

    // Les lignes fabriquées portent bien le mode qu'on croit y mettre.
    for mode in LES_SIX {
        assert_eq!(
            ligne_de(POMPE, mode, "oui").mode,
            mode,
            "la ligne `chan` doit rendre le mode tel quel : c'est de lui que tout dépend"
        );
    }
}

// ---------------------------------------------------------------------------
// 1 — quels modes verrouillent
// ---------------------------------------------------------------------------

#[test]
fn un_canal_non_pilote_est_verrouille() {
    // Test d'intention n° 1 de l'issue : « Une ligne dont le mode est `non-piloté` est
    // verrouillée ». Critère d'acceptation n° 1.
    //
    // C'est le cas de la machine : les deux canaux du Kraken lisent `non-piloté`, et la mesure du
    // 2026-08-15 a montré derrière ce libellé une régulation d'usine bien vivante — un duty qui a
    // suivi le liquide par paliers, 89, 102, 115, 128, 153, pour une pompe montée de 1500 à
    // 2380 tr/min. C'est cette régulation-là qu'un geste distrait détruit sans retour.
    for (canal, _) in BANC {
        for auto in DRAPEAUX_AUTO {
            let ligne = ligne_de(canal, NON_PILOTE, auto);
            assert!(
                ligne.regule_seul(),
                "« {canal} » lit « {NON_PILOTE} » (sait_faire_auto={auto}) : le pilote ne pilote \
                 pas, donc le périphérique décide — il y a quelque chose à protéger"
            );
        }
    }
}

#[test]
fn un_canal_sur_la_courbe_de_l_hote_est_verrouille() {
    // Test d'intention n° 2 : « Une ligne dont le mode est `courbe-de-l'hôte` est verrouillée ».
    //
    // L'issue le motive : « `courbe-de-l'hôte` est inatteignable tant que #104 n'est pas faite,
    // mais il régule lui aussi, et c'est même la seule courbe que l'utilisateur ait posée
    // lui-même ». La détruire d'un geste coûte donc plus cher que les autres, pas moins.
    for (canal, _) in BANC {
        for auto in DRAPEAUX_AUTO {
            let ligne = ligne_de(canal, COURBE_DE_L_HOTE, auto);
            assert!(
                ligne.regule_seul(),
                "« {canal} » lit « {COURBE_DE_L_HOTE} » (sait_faire_auto={auto}) : le firmware \
                 exécute une courbe, et écrire une consigne la remplace"
            );
        }
    }
}

#[test]
fn un_canal_manuel_n_est_pas_verrouille() {
    // Test d'intention n° 3 : « Une ligne en `manuel` ne l'est pas ». Critère d'acceptation n° 2 :
    // « Un canal en `manuel` n'a ni cadenas ni barre grisée ».
    //
    // Le canal du Kraken est essayé comme les autres, et c'est le point de l'issue : « sans mentir
    // sur un canal du Kraken déjà passé en manuel, où il n'y a plus rien à protéger ». Un verrou
    // posé sur le nom du contrôleur mettrait un cadenas devant une barre qui ne détruit plus rien —
    // et il le mettrait pour toujours, puisqu'on ne revient pas de `manuel` sans arrêt complet.
    for (canal, _) in BANC {
        for auto in DRAPEAUX_AUTO {
            let ligne = ligne_de(canal, MANUEL, auto);
            assert!(
                !ligne.regule_seul(),
                "« {canal} » lit « {MANUEL} » (sait_faire_auto={auto}) : c'est l'hôte qui décide \
                 déjà de la vitesse, il n'y a aucune régulation à perdre"
            );
        }
    }
}

#[test]
fn les_trois_autres_modes_ne_verrouillent_pas() {
    // L'issue nomme deux modes verrouillants et six libellés : les quatre autres sont donc ouverts.
    // Ce test fige les trois qui restent après `manuel`, et chacun a sa raison :
    //
    // - `plein-régime-100%` est une **intention**, jamais un état observable (#101) : le pilote a
    //   lâché la barre et le canal tourne à fond. Il n'y a pas de régulation à protéger, et c'est
    //   au contraire l'état dont on veut le plus pouvoir sortir — un cadenas y ajouterait un geste
    //   devant un ventilateur qui hurle.
    // - `inconnu-N` dit exactement que l'hôte ne sait pas ce que fait ce canal. Verrouiller
    //   là-dessus, ce serait affirmer qu'il régule ; le projet refuse d'implémenter depuis un
    //   inconnu (CLAUDE.md), et l'issue n'a pas nommé ce mode.
    // - `non-réglable` désigne une source sans `pwmN_enable` — `nct6687` sur cette machine. Aucun
    //   mode ne s'y écrit, donc aucun ne s'y perd.
    //
    // Aucune de ces trois raisons ne tient d'elle-même à jamais : elles se rouvrent le jour où l'un
    // de ces modes se met à réguler.
    for mode in [PLEIN_REGIME, INCONNU, NON_REGLABLE] {
        for (canal, _) in BANC {
            for auto in DRAPEAUX_AUTO {
                assert!(
                    !ligne_de(canal, mode, auto).regule_seul(),
                    "« {canal} » lit « {mode} » (sait_faire_auto={auto}) : l'issue ne nomme que \
                     « {NON_PILOTE} » et « {COURBE_DE_L_HOTE} » comme verrouillants"
                );
            }
        }
    }
}

#[test]
fn toute_la_famille_inconnu_n_reste_ouverte() {
    // `inconnu-N` porte un nombre variable : le figer sur la seule valeur 7 laisserait passer une
    // implémentation qui verrouillerait, disons, `inconnu-0` — la valeur qu'un `kzalloc` produit,
    // et donc la plus tentante à traiter à part.
    for n in [0u8, 1, 3, 7, 42, 255] {
        let mode = format!("{PREFIXE_INCONNU}{n}");
        assert!(
            !ligne_de(POMPE, &mode, "oui").regule_seul(),
            "« {mode} » : une valeur que l'hôte ne sait pas interpréter n'établit aucune régulation"
        );
    }
}

#[test]
fn le_verrou_ne_regarde_ni_le_controleur_ni_le_drapeau_auto() {
    // Approche technique de l'issue : « Ne pas s'appuyer sur `sait_faire_auto` : depuis #97 ce
    // drapeau vaut *le pilote sait faire auto ET une courbe a été posée*, donc toujours `non`. Il
    // ne dit plus rien du matériel. » Et : « Le déclencheur est le mode, pas le nom du contrôleur ».
    //
    // Les deux fautes visées passeraient aujourd'hui sur SHYNAEL sans qu'on le voie : verrouiller
    // sur `kraken` donne le bon résultat parce que seuls ses canaux lisent `non-piloté`, et
    // verrouiller sur `sait_faire_auto` donne le bon résultat parce qu'il est toujours faux. Les
    // deux cassent au premier changement de matériel ou de pilote — et cassent en silence.
    for mode in LES_SIX {
        let attendu = MODES_QUI_VERROUILLENT.contains(&mode);
        for (canal, _) in BANC {
            for auto in DRAPEAUX_AUTO {
                assert_eq!(
                    ligne_de(canal, mode, auto).regule_seul(),
                    attendu,
                    "« {canal} » en « {mode} » (sait_faire_auto={auto}) doit se décider sur le \
                     mode seul : ni le nom du contrôleur ni le drapeau n'entrent dans la règle"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 2 — ce qu'une ligne verrouillée laisse partir, et ce qu'elle retient
// ---------------------------------------------------------------------------

#[test]
fn une_ligne_verrouillee_n_emet_aucune_commande() {
    // Test d'intention n° 4 : « Une ligne verrouillée n'émet aucune commande tant qu'elle l'est ».
    // Critère d'acceptation n° 1 : « a sa barre inerte ».
    //
    // Les quatre consignes sont essayées, `Auto` comprise. Ce n'est pas du zèle : c'est
    // exactement le geste du 2026-08-15 — un clic sur « auto » a mis la consigne de la pompe à 0 %
    // (#97). Un verrou qui ne retiendrait que les `Pwm` laisserait passer celui des deux gestes qui
    // a réellement coûté la régulation.
    for mode in MODES_QUI_VERROUILLENT {
        for (canal, _) in BANC {
            let ligne = ligne_de(canal, mode, "non");
            for consigne in CONSIGNES {
                assert_eq!(
                    ligne.commande_verrouillable(consigne(), false),
                    None,
                    "« {canal} » en « {mode} », verrou fermé : {:?} ne doit pas partir — c'est \
                     une régulation qu'on ne récupère pas sans arrêt complet de la machine",
                    consigne()
                );
            }
        }
    }
}

#[test]
fn une_ligne_deverrouillee_emet_normalement() {
    // Test d'intention n° 5 : « Une ligne déverrouillée émet normalement ». Critère d'acceptation
    // n° 3 : « Déverrouiller rend la barre manipulable ».
    //
    // Le verrou est une confirmation, pas une interdiction : une fois le geste fait, la commande
    // part **telle quelle**, vers ce canal-là, avec cette consigne-là. Une implémentation qui
    // transformerait la consigne au passage — en la bornant, en la remplaçant par un `Auto` —
    // rendrait le déverrouillage plus dangereux que son absence.
    for mode in MODES_QUI_VERROUILLENT {
        for (canal, _) in BANC {
            let ligne = ligne_de(canal, mode, "non");
            for consigne in CONSIGNES {
                assert_eq!(
                    ligne.commande_verrouillable(consigne(), true),
                    Some(requete(canal, consigne())),
                    "« {canal} » en « {mode} », verrou ouvert : {:?} doit partir intacte",
                    consigne()
                );
            }
        }
    }
}

#[test]
fn une_ligne_manuelle_passe_sans_deverrouillage() {
    // Critère d'acceptation n° 2, versant commande : un canal en `manuel` n'a pas de cadenas, donc
    // sa barre commande sans qu'on ait rien à ouvrir. C'est le cas des trois canaux `nzxtsmart2`,
    // c'est-à-dire de sept ventilateurs sur dix : leur imposer un geste de plus serait payer le
    // prix du verrou sans en tirer la protection.
    for (canal, _) in BANC {
        let ligne = ligne_de(canal, MANUEL, "non");
        for consigne in CONSIGNES {
            assert_eq!(
                ligne.commande_verrouillable(consigne(), false),
                Some(requete(canal, consigne())),
                "« {canal} » en « {MANUEL} » : {:?} part sans déverrouillage",
                consigne()
            );
        }
    }
}

#[test]
fn les_modes_qui_ne_verrouillent_pas_passent_sans_deverrouillage() {
    // Le pendant du test qui fige les quatre modes ouverts : « ouvert » doit vouloir dire que la
    // commande part, et non seulement que `regule_seul` rend faux. Les deux moitiés d'une même
    // règle, et c'est celle-ci qu'un utilisateur constate.
    for mode in MODES_QUI_NE_VERROUILLENT_PAS {
        for (canal, _) in BANC {
            let ligne = ligne_de(canal, mode, "oui");
            for consigne in CONSIGNES {
                assert_eq!(
                    ligne.commande_verrouillable(consigne(), false),
                    Some(requete(canal, consigne())),
                    "« {canal} » en « {mode} » n'est pas verrouillé : {:?} doit partir sans geste \
                     préalable",
                    consigne()
                );
            }
        }
    }
}

#[test]
fn le_verrou_ne_change_rien_a_ce_que_commande_faisait() {
    // Garde-fou de non-régression vis-à-vis de #100 : `commande` est figée par
    // `spec_canal_illisible.rs`, et ce fichier ne doit pas la déplacer. Sur une ligne que le verrou
    // n'arrête pas, les deux méthodes doivent rendre exactement la même chose — sinon la fenêtre
    // aurait deux chemins d'émission qui divergeraient, et le second n'aurait aucune raison d'être
    // le bon.
    for mode in MODES_QUI_NE_VERROUILLENT_PAS {
        for (canal, _) in BANC {
            let ligne = ligne_de(canal, mode, "non");
            for consigne in CONSIGNES {
                for deverrouille in [false, true] {
                    assert_eq!(
                        ligne.commande_verrouillable(consigne(), deverrouille),
                        ligne.commande(consigne()),
                        "« {canal} » en « {mode} » : le verrou ne concerne pas ce canal, la \
                         commande doit être celle que #100 a figée"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — la composition avec #100 : illisible d'abord, verrou ensuite
// ---------------------------------------------------------------------------

#[test]
fn une_ligne_illisible_reste_inerte_verrou_ouvert_ou_ferme() {
    // Critère d'acceptation n° 4 : « Un canal illisible reste inerte, verrou ou pas (#100) ».
    //
    // C'est la composition des deux règles, et elle doit être testée explicitement parce qu'elle
    // n'est impliquée par aucune des deux : #100 dit « illisible ⇒ inerte », #112 dit
    // « verrouillé ⇒ inerte », et une implémentation qui écrirait « inerte ⇔ verrouillé et non
    // déverrouillé » satisferait la seconde en cassant la première. Le canal du Kraken part
    // périodiquement en quarantaine (#100) : ouvrir son cadenas pendant ce temps rendrait sa barre
    // manipulable vers un périphérique qui ne répond plus, et le démon paierait la lecture qui l'a
    // fait écarter.
    for mode_d_avant in LES_SIX {
        for (canal, _) in BANC {
            let ligne = ligne_illisible_de(canal, mode_d_avant);
            for consigne in CONSIGNES {
                for deverrouille in [false, true] {
                    assert_eq!(
                        ligne.commande_verrouillable(consigne(), deverrouille),
                        None,
                        "« {canal} » est en quarantaine (il lisait « {mode_d_avant} ») : \
                         {:?} ne part pas, verrou {}",
                        consigne(),
                        if deverrouille { "ouvert" } else { "fermé" }
                    );
                }
            }
        }
    }
}

#[test]
fn un_canal_verrouille_qui_redevient_lisible_se_deverrouille_de_nouveau() {
    // L'inertie d'une quarantaine est un état, pas une punition (#100), et le verrou ne doit pas la
    // rendre définitive. Le Kraken « cesse périodiquement de répondre » : une implémentation qui
    // laisserait la ligne inerte après le retour rendrait la pompe irréglable au premier hoquet,
    // exactement au moment où l'on veut la régler à la main.
    let mut tri = Tri::nouveau();
    tri.poser(&[chan(POMPE, NON_PILOTE, "non"), ligne("end")]);
    tri.poser(&[ligne(&format!("unreadable {POMPE} {RAISON}")), ligne("end")]);
    let vue = tri.poser(&[chan(POMPE, NON_PILOTE, "non"), ligne("end")]);
    let revenue = vue
        .canaux
        .into_iter()
        .next()
        .expect("le canal revenu garde sa ligne");

    assert!(
        revenue.lisible,
        "« {POMPE} » répond de nouveau : sa ligne redevient une ligne normale (#100)"
    );
    assert!(
        revenue.regule_seul(),
        "et elle relit « {NON_PILOTE} » : le cadenas revient avec la mesure"
    );
    assert_eq!(
        revenue.commande_verrouillable(FanAction::Pwm(50), false),
        None,
        "verrou fermé, elle reste inerte"
    );
    assert_eq!(
        revenue.commande_verrouillable(FanAction::Pwm(50), true),
        Some(requete(POMPE, FanAction::Pwm(50))),
        "verrou ouvert, elle commande de nouveau"
    );
}

// ---------------------------------------------------------------------------
// 4 — l'aide sur les six modes
// ---------------------------------------------------------------------------

#[test]
fn chacun_des_six_modes_a_une_aide_non_vide() {
    // Test d'intention n° 8 : « Chacun des six modes a un texte d'aide non vide et distinct ».
    // Critère d'acceptation n° 8 : « Une aide accessible depuis la fenêtre décrit les six modes ».
    //
    // L'issue dit pourquoi : « six libellés dont trois demandent un contexte que seul
    // `docs/VENTILATEURS.md` porte aujourd'hui. Le texte doit dire, pour chacun, **qui décide de la
    // vitesse** et **ce qu'on perd en y touchant**. C'est l'information qui manquait le
    // 2026-08-15. »
    for mode in LES_SIX {
        let texte = aide(mode);
        assert!(
            !texte.trim().is_empty(),
            "« {mode} » a une aide vide : autant ne pas afficher le point d'interrogation"
        );
        assert!(
            texte.chars().count() >= LONGUEUR_MINIMALE_D_UNE_AIDE,
            "l'aide de « {mode} » fait {} caractères : « qui décide de la vitesse » et « ce qu'on \
             perd en y touchant » sont deux informations, elles ne tiennent pas en si peu. Texte \
             rendu : {texte:?}",
            texte.chars().count()
        );
        assert_ne!(
            texte.trim(),
            mode,
            "l'aide de « {mode} » n'est que son libellé recopié : elle n'explique rien"
        );
    }
}

#[test]
fn les_six_aides_sont_deux_a_deux_distinctes() {
    // Suite du même test d'intention : « non vide **et distinct** ».
    //
    // Un texte partagé entre deux modes est pire qu'une absence de texte : il affirme que les deux
    // se valent. C'est exactement l'erreur que #101 a coûtée — deux sens sous une même valeur, et
    // celui de l'écriture qui s'affichait.
    for (i, mode) in LES_SIX.iter().enumerate() {
        for autre in LES_SIX.iter().skip(i + 1) {
            assert_ne!(
                aide(mode),
                aide(autre),
                "« {mode} » et « {autre} » partagent leur aide : le panneau dirait que ces deux \
                 modes se valent, alors qu'ils ne désignent pas le même décideur"
            );
        }
    }
}

#[test]
fn le_non_pilote_et_le_plein_regime_ne_se_lisent_pas_pareil() {
    // Le cas particulier qui a coûté un diagnostic, et qui justifie que cette aide existe.
    //
    // `non-piloté` et `plein-régime-100%` partagent la **même valeur** dans `pwmN_enable` : `0`.
    // Lue, elle dit « le pilote n'a jamais touché ce canal » ; écrite, elle envoie à 100 % et lâche
    // la barre (`docs/VENTILATEURS.md`, #101). Deux textes qui se ressembleraient ici referaient
    // exactement la confusion que #101 a corrigée dans la colonne — au moment précis où
    // l'utilisateur ouvre l'aide pour la lever.
    let lu = aide(NON_PILOTE);
    let ecrit = aide(PLEIN_REGIME);
    assert_ne!(
        lu, ecrit,
        "« {NON_PILOTE} » et « {PLEIN_REGIME} » disent la même chose : ce sont pourtant les deux \
         sens opposés du même `0`, et les confondre a fait annoncer une pompe à fond au moment où \
         elle était au minimum"
    );
}

#[test]
fn la_table_expose_exactement_les_six_modes() {
    // L'aide doit décrire « les six modes » : ni cinq, ni un septième inventé. Une table incomplète
    // laisserait sans explication un libellé de la colonne — et ce serait forcément celui qu'on n'a
    // pas pensé à expliquer, donc celui qu'on ne comprend pas.
    //
    // L'ordre n'est pas figé : c'est un choix d'affichage, et l'issue n'en dit rien.
    let table = aide_des_modes();
    assert_eq!(
        table.len(),
        LES_SIX.len(),
        "la table porte {} entrées pour six libellés : {:?}",
        table.len(),
        table.iter().map(|(mode, _)| *mode).collect::<Vec<_>>()
    );

    let mut rendus: Vec<&str> = table.iter().map(|(mode, _)| *mode).collect();
    let mut attendus: Vec<&str> = LES_SIX.to_vec();
    rendus.sort_unstable();
    attendus.sort_unstable();
    assert_eq!(
        rendus, attendus,
        "la table ne nomme pas les six libellés de la colonne MODE — la famille `inconnu-<n>` y \
         figure sous son nom générique « {INCONNU_GENERIQUE} », les cinq autres tels quels"
    );
}

#[test]
fn chaque_entree_de_la_table_se_retrouve_par_son_libelle() {
    // Les deux fonctions doivent être deux vues d'une même donnée : le panneau liste, le point
    // d'interrogation d'une ligne cherche. Deux sources séparées divergeraient au premier texte
    // corrigé d'un seul côté, et c'est le genre d'écart qu'on ne voit jamais parce qu'on ne regarde
    // jamais les deux en même temps.
    for (mode, texte) in aide_des_modes() {
        assert_eq!(
            aide_du_mode(mode),
            Some(texte),
            "« {mode} » figure dans la table mais ne se retrouve pas par son libellé"
        );
    }
}

#[test]
fn l_aide_se_trouve_depuis_le_mode_d_une_ligne() {
    // La promesse telle qu'un utilisateur la constate : le point d'interrogation est **à côté du
    // mode affiché**, et c'est donc le libellé de la ligne — pris tel quel dans la réponse du
    // démon — qui sert de clé. Une table qui n'accepterait qu'une graphie normalisée en amont
    // rendrait le chemin dépendant d'une conversion invisible.
    for mode in LES_SIX {
        let ligne = ligne_de(POMPE, mode, "oui");
        assert!(
            aide_du_mode(&ligne.mode).is_some(),
            "la ligne affiche « {} » et l'aide ne trouve rien : le point d'interrogation ouvrirait \
             sur du vide",
            ligne.mode
        );
    }

    // Y compris pour un `inconnu-<n>` réel, celui que la colonne écrit vraiment.
    let ligne = ligne_de(POMPE, INCONNU, "oui");
    assert!(
        aide_du_mode(&ligne.mode).is_some(),
        "la colonne écrit « {INCONNU} », pas « {INCONNU_GENERIQUE} » : c'est ce libellé-là qu'il \
         faut savoir retrouver"
    );
}

#[test]
fn toute_la_famille_inconnu_partage_une_seule_aide() {
    // ⚠️ `inconnu-N` porte un nombre variable, et il faut décider comment la table le couvre. Ce
    // fichier tranche pour une **famille** : un même texte pour tout `inconnu-<n>`.
    //
    // La raison n'est pas l'économie de lignes, c'est qu'il n'y a rien d'autre à dire. Le nombre
    // est la valeur brute de `pwmN_enable` que l'hôte n'a pas su interpréter ; 256 textes seraient
    // 256 façons d'écrire « on ne sait pas ». Et le texte générique doit être le même que celui de
    // l'entrée de la table, sinon le panneau et le point d'interrogation d'une ligne diraient deux
    // choses différentes du même mode.
    let generique = aide(INCONNU_GENERIQUE);
    for n in [0u8, 1, 3, 7, 42, 255] {
        let mode = format!("{PREFIXE_INCONNU}{n}");
        assert_eq!(
            aide_du_mode(&mode),
            Some(generique),
            "« {mode} » doit rendre l'aide de la famille : le nombre est justement ce que \
             personne ne sait interpréter"
        );
    }
}

#[test]
fn un_libelle_inconnu_ne_fait_pas_paniquer() {
    // L'aide reçoit le mode **verbatim** du socket : un démon d'une autre version, une ligne
    // illisible dont le mode est vide, un jeton qu'on n'a pas prévu. Aucun de ces cas ne doit
    // arrêter la fenêtre — elle n'affiche alors pas d'aide, ce qui est la seule réponse honnête.
    for etranger in LIBELLES_ETRANGERS {
        assert_eq!(
            aide_du_mode(etranger),
            None,
            "« {etranger} » n'est aucun des six libellés : rendre une aide reviendrait à \
             reconnaître approximativement un texte qui arrive tel quel du démon, et donc à ne \
             jamais voir le jour où il dérive"
        );
    }
}

#[test]
fn les_libelles_biscornus_ne_font_pas_paniquer_non_plus() {
    // Même exigence, sur les formes qui cassent une découpe naïve : un préfixe sans son nombre, un
    // nombre hors de l'octet que `pwmN_enable` porte, un texte non ASCII, un texte très long. Ce
    // test n'exige aucune réponse en particulier — seulement que la fonction en rende une.
    let long = "x".repeat(4_096);
    for biscornu in [
        PREFIXE_INCONNU,
        "inconnu-999",
        "inconnu-abc",
        "inconnu--1",
        "🙂",
        long.as_str(),
    ] {
        let _ = aide_du_mode(biscornu);
    }
}
