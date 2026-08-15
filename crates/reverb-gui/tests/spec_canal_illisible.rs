//! Tests d'intention du routage d'un canal illisible dans la fenêtre (issue #100).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #100 seule. Rien de
//! `crates/reverb-gui/src/` ni de `crates/reverb-gui/ui/` n'a été lu pour les produire : ni corps
//! de fonction, ni `.slint`. Seules les signatures publiques déjà en place ont été relevées —
//! `Releve` et `Historique` (#40), `Poignee` (#32), `ResponseLine` (#17, #50) —, pour savoir à côté
//! de quoi la nouvelle API vient s'asseoir. À l'écriture de ce fichier, le module `telemetrie` de
//! `reverb-gui` n'existe pas : la compilation doit échouer sur lui, et **sur lui seul**. C'est la
//! phase rouge.
//!
//! Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne touche un bus.
//!
//! ## Le défaut, tel que l'issue le décrit
//!
//! Le démon fait correctement son travail depuis #88 : un canal en quarantaine est **dit**, jamais
//! omis, et il est dit à sa place dans le tour.
//!
//! ```text
//! chan nzxtsmart2:fan-1 radiateur-haut 1200 60 manual non
//! unreadable kraken2023elite:fan-speed Connection timed out (os error 110)
//! ```
//!
//! C'est la fenêtre qui perd la seconde ligne : elle ne fabrique une ligne de ventilateur que
//! depuis `ResponseLine::Channel`, et l'`Unreadable` tombe dans la branche des **sondes**, où il est
//! noté comme un relevé illisible. La ligne du ventilateur n'est jamais créée, et le Kraken
//! disparaît de la fenêtre au moment précis où l'on voudrait le régler à la main.
//!
//! ⚠️ **Une ligne absente et une ligne grisée ne disent pas la même chose** (issue #100, « pourquoi
//! ça compte »). La première laisse croire que le canal n'existe pas — donc qu'il n'y a rien à
//! régler ni rien à réparer. La seconde dit qu'il existe et qu'on ne l'atteint plus, ce qui est la
//! vérité. C'est la règle que le projet applique déjà aux sondes ; les canaux ne l'ont pas reçue.
//!
//! ## Ce qu'on peut tester d'une fenêtre, et ce qu'on ne peut pas
//!
//! ⚠️ **Un panneau ne s'écrit pas en assertions.** Ce qui se vérifie ici, c'est le **tri** : une
//! ligne entre, et l'on regarde de quel côté elle sort. C'est une fonction pure de la réponse du
//! démon et de ce que la fenêtre a déjà vu, et elle n'a besoin d'aucune session graphique.
//!
//! Le grisé, la police, la place du bandeau se regardent à l'œil, sur des images, hors de ce
//! fichier.
//!
//! ⚠️ **Le tri doit donc vivre dans la bibliothèque, pas dans `main.rs`.** `poser_telemetrie` est
//! aujourd'hui dans le binaire, et un binaire ne se teste pas depuis `tests/`. C'est la règle du
//! projet — « ce qui est testable sans matériel est séparé de ce qui touche au matériel »
//! (CLAUDE.md) — appliquée à la fenêtre : la fonction de tri est du calcul, le dessin est de l'E/S.
//! `main.rs` recopie ensuite chaque [`LigneCanal`] dans le `LigneVentilateur` du `.slint`, champ
//! pour champ, y compris son `lisible` — que l'issue relève comme **déjà présent et jamais
//! alimenté**.
//!
//! ## Le contrat que ces tests figent
//!
//! ```ignore
//! // dans crates/reverb-gui/src/telemetrie.rs, à côté de `sondes.rs` qu'il ne touche pas.
//!
//! use reverb_gui::sondes::Releve;
//! use reverb_proto::Position;
//! use reverb_proto::ipc::{FanAction, Request, ResponseLine};
//!
//! /// Une ligne de ventilateur, telle que la fenêtre la montre.
//! ///
//! /// Les champs de mesure sont ceux de `ResponseLine::Channel` ; `lisible` dit si le démon a
//! /// répondu pour ce canal à ce tour-ci.
//! pub struct LigneCanal {
//!     pub canal: String,
//!     pub position: Option<Position>,
//!     pub rpm: Option<u32>,
//!     pub pwm: Option<u8>,
//!     pub mode: String,
//!     pub sait_faire_auto: bool,
//!     pub lisible: bool,
//! }
//!
//! impl LigneCanal {
//!     /// La requête qu'une consigne produit pour ce canal — `None` s'il est illisible.
//!     pub fn commande(&self, action: FanAction) -> Option<Request>;
//! }
//!
//! /// Ce qu'un tour de `status` donne à montrer.
//! pub struct Telemetrie {
//!     pub canaux: Vec<LigneCanal>,
//!     pub sondes: Vec<(String, Releve)>,
//! }
//!
//! /// Le tri, qui se souvient des canaux déjà vus.
//! pub struct Tri { /* … */ }
//!
//! impl Tri {
//!     pub fn nouveau() -> Tri;
//!     /// Range un tour de réponses : les canaux d'un côté, les sondes de l'autre.
//!     pub fn poser(&mut self, lignes: &[ResponseLine]) -> Telemetrie;
//! }
//! ```
//!
//! `LigneCanal` et `Telemetrie` portent `Debug`, `Clone` et `PartialEq` ; les types des champs de
//! mesure sont **repris de `ResponseLine::Channel`**, tels que le démon les publie déjà — `rpm` en
//! `u32` parce qu'un régime dépasse les 65 535 tours que tiendrait un `u16`, `pwm` en `u8` parce
//! qu'il s'écrit en pourcentage. Rien ici n'invente une unité : ces lignes sont recopiées d'un tour
//! de télémétrie, pas converties.
//!
//! ### Pourquoi un objet qui se souvient, et non une fonction à deux arguments
//!
//! L'approche technique de l'issue laisse un point ouvert, et il est réel : **au tout premier tour,
//! un canal muet depuis le démarrage du démon n'a jamais été vu lisible**, et la fenêtre ne peut
//! pas savoir que c'est un canal. Deux issues y sont nommées — le démon distingue les deux natures
//! dans le protocole, ou la fenêtre reconnaît la forme du sujet —, « à trancher à
//! l'implémentation ».
//!
//! Un `Tri` qui garde sa mémoire **ne tranche pas ce point** : la mémoire est un détail interne, et
//! une implémentation qui routerait sur un jeton du protocole passerait ces tests sans jamais s'en
//! servir. Une fonction libre `trier(lignes, connus: &[String])` aurait, elle, imposé la première
//! branche dans sa signature — ce que ce fichier n'a pas à faire.
//!
//! ### Les lignes sont fabriquées depuis leur texte, pas depuis l'énumération
//!
//! Chaque ligne d'entrée passe par `parse_response_line`, avec le texte **qu'un démon
//! d'aujourd'hui écrit sur le socket**. Deux raisons, et la seconde est un critère :
//!
//! 1. le texte est vérifiable — c'est celui des specs et de `spec_ipc.rs`, pas une construction
//!    qui ressemblerait à ce que le démon envoie ;
//! 2. l'approche technique de l'issue exige qu'**un démon d'avant reste lisible**. Si
//!    l'implémentation ajoute un jeton de nature au protocole, ces tests continuent de lui envoyer
//!    la forme ancienne, et la fenêtre doit continuer d'y retrouver ses canaux — par sa mémoire, à
//!    défaut du jeton.
//!
//! ## Ce que ces tests ne vérifient pas
//!
//! - **La quarantaine elle-même** (issue #100, hors scope) : elle fait correctement son travail,
//!   et `spec_canaux_muets.rs` la couvre côté démon.
//! - **La réparation du périphérique muet**, qui est une issue séparée.
//! - **Le dessin de la ligne grisée.** Ce qui se vérifie sans écran, c'est *quelles* lignes
//!   sortent, *dans quel ordre*, et *ce qu'elles portent*.
//! - **La `position` d'un canal illisible.** C'est une donnée de montage, pas une mesure : la garder
//!   pour continuer de nommer la ligne physiquement est légitime, l'oublier l'est aussi. L'issue ne
//!   tranche pas, ces tests non plus.
//! - **Le `mode` d'un canal illisible**, pour la même raison — il n'est pas un régime mesuré.
//! - **L'ordre des sondes** dans `Telemetrie::sondes` : l'historique les range par nom, l'ordre
//!   d'arrivée n'a aucun effet visible.

use reverb_gui::sondes::Releve;
use reverb_gui::telemetrie::{LigneCanal, Telemetrie, Tri};
use reverb_proto::ipc::{FanAction, Request, ResponseLine, parse_response_line};

// ---------------------------------------------------------------------------
// Le banc : les canaux de SHYNAEL, et les sondes qui vivent à côté d'eux
// ---------------------------------------------------------------------------

/// Le canal qui se tait, nommé comme la ligne `unreadable` observée (issue #100, contexte).
const MUET: &str = "kraken2023elite:fan-speed";

/// Son voisin, **sur le même contrôleur**.
///
/// « Le Kraken cesse périodiquement de répondre à son pilote, et ses deux canaux partent en
/// quarantaine ensemble » : c'est l'incident tel qu'il a été signalé, et il a son test.
const VOISIN: &str = "kraken2023elite:pump-speed";

/// Trois canaux qui répondent, sur deux autres pilotes.
const VENTILO_1: &str = "nzxtsmart2:fan-1";
const VENTILO_2: &str = "nzxtsmart2:fan-2";
const BOITIER: &str = "nct6687:fan-3";

/// Le banc, dans l'ordre où le démon rend ses lignes — c'est aussi l'ordre attendu de la liste.
///
/// Le canal muet est **en tête**, et son voisin juste après : la faute qu'on cherche est un
/// `unreadable` renvoyé en fin de liste, et elle ne se voit que si la place perdue n'est pas la
/// dernière.
const TOUS: [&str; 5] = [MUET, VOISIN, VENTILO_1, VENTILO_2, BOITIER];

/// Une sonde **du même contrôleur que le canal muet**.
///
/// C'est le piège du routage par préfixe : `kraken2023elite:` nomme aussi bien un canal qu'une
/// sonde, et trier sur le pilote plutôt que sur le nom complet ferait de la température du liquide
/// une ligne de ventilateur.
const LIQUIDE: &str = "kraken2023elite:coolant-temp";

/// Une sonde d'un autre contrôleur — celle que les ventilateurs suivent.
const CPU: &str = "k10temp:tctl";

/// La raison rendue par le contrôleur en rade, recopiée telle quelle de l'issue.
///
/// ⚠️ Elle **porte des espaces** : c'est le dernier champ de la ligne `unreadable`, et l'y laisser
/// entière vérifie au passage que le tri ne coupe pas au premier blanc.
const RAISON: &str = "Connection timed out (os error 110)";

/// Une seconde raison, distincte, pour que deux sujets muets ne puissent pas échanger la leur.
const AUTRE_RAISON: &str = "No such device (os error 19)";

/// Le régime que les canaux rendent au premier tour.
const RPM: u32 = 1200;
/// La consigne que les canaux rendent au premier tour.
const PWM: u8 = 60;

/// Le régime du retour — **différent** du premier, sinon on ne saurait pas lequel est affiché.
const RPM_APRES: u32 = 1450;
/// La consigne du retour, elle aussi différente.
const PWM_APRES: u8 = 75;

// ---------------------------------------------------------------------------
// Les lignes du démon, écrites comme il les écrit
// ---------------------------------------------------------------------------

/// Décode une ligne de réponse, ou dit laquelle a été mal écrite.
fn ligne(texte: &str) -> ResponseLine {
    parse_response_line(texte).unwrap_or_else(|erreur| {
        panic!("« {texte} » doit être une ligne de réponse valide, refusée : {erreur:?}")
    })
}

/// La position et le drapeau « auto » de chaque canal du banc.
///
/// Le Kraken sait faire auto, les deux autres pilotes non (#50) ; les positions sont celles que le
/// démon publie, `-` quand le canal n'en porte pas.
fn habillage(canal: &str) -> (&'static str, &'static str) {
    match canal {
        MUET | VOISIN => ("-", "oui"),
        VENTILO_1 => ("radiateur-haut", "non"),
        VENTILO_2 => ("bas-milieu", "non"),
        BOITIER => ("arriere", "non"),
        autre => panic!("« {autre} » n'est pas un canal du banc"),
    }
}

/// La ligne `chan` d'un canal qui répond : `chan <canal> <position> <rpm> <pwm> <mode> <oui|non>`.
fn canal(nom: &str, rpm: u32, pwm: u8) -> ResponseLine {
    let (position, auto) = habillage(nom);
    ligne(&format!("chan {nom} {position} {rpm} {pwm} manual {auto}"))
}

/// La ligne `chan` d'un canal qui a répondu **sans régime** — le cas du bloc-pompe en firmware.
fn canal_sans_regime(nom: &str) -> ResponseLine {
    let (position, auto) = habillage(nom);
    ligne(&format!("chan {nom} {position} - - firmware {auto}"))
}

/// La ligne `unreadable` d'un sujet en quarantaine, telle que le démon l'écrit depuis #88.
fn illisible(sujet: &str, raison: &str) -> ResponseLine {
    ligne(&format!("unreadable {sujet} {raison}"))
}

/// La ligne `temp` d'une sonde qui répond.
fn temperature(sonde: &str, millidegres: i32) -> ResponseLine {
    ligne(&format!("temp {sonde} {millidegres}"))
}

/// Un tour de `status` complet : les cinq canaux dans l'ordre, puis les deux sondes.
///
/// `muets` nomme les sujets rendus `unreadable` à ce tour-ci — canaux comme sondes. Un canal muet
/// **garde sa place dans la réponse** : c'est ce que le démon fait (`spec_canaux_muets.rs`,
/// « une ligne par canal, dans l'ordre reçu »), et c'est donc ce que la fenêtre reçoit.
fn tour(muets: &[&str]) -> Vec<ResponseLine> {
    let mut lignes = Vec::new();
    for nom in TOUS {
        if muets.contains(&nom) {
            lignes.push(illisible(nom, RAISON));
        } else {
            lignes.push(canal(nom, RPM, PWM));
        }
    }
    for (sonde, millidegres) in [(LIQUIDE, 34_200), (CPU, 61_800)] {
        if muets.contains(&sonde) {
            lignes.push(illisible(sonde, AUTRE_RAISON));
        } else {
            lignes.push(temperature(sonde, millidegres));
        }
    }
    lignes.push(ligne("end"));
    lignes
}

// ---------------------------------------------------------------------------
// Lire un tri
// ---------------------------------------------------------------------------

/// Les noms des lignes de ventilateur, **dans l'ordre rendu**.
fn noms(vue: &Telemetrie) -> Vec<String> {
    vue.canaux.iter().map(|l| l.canal.clone()).collect()
}

/// La ligne d'un canal, ou l'échec qui dit ce qu'on a trouvé à la place.
///
/// C'est ici que le défaut de l'issue se voit : aujourd'hui, la ligne du canal muet n'existe pas.
fn ligne_de<'a>(vue: &'a Telemetrie, nom: &str) -> &'a LigneCanal {
    vue.canaux
        .iter()
        .find(|l| l.canal == nom)
        .unwrap_or_else(|| {
            panic!(
                "« {nom} » doit garder sa ligne de ventilateur ; la liste rendue est {:?}",
                noms(vue)
            )
        })
}

/// Le relevé d'une sonde, s'il y en a un.
fn releve_de<'a>(vue: &'a Telemetrie, nom: &str) -> Option<&'a Releve> {
    vue.sondes
        .iter()
        .find(|(sonde, _)| sonde == nom)
        .map(|(_, releve)| releve)
}

/// Les quatre consignes qu'une poignée de ventilateur peut produire.
///
/// Ce sont des **fabriques** et non des valeurs : `FanAction` n'a pas à être `Copy` pour que ces
/// tests compilent, et une consigne consommée par `commande` doit pouvoir être refabriquée à
/// l'identique pour écrire l'attendu.
const CONSIGNES: [fn() -> FanAction; 4] = [
    || FanAction::Auto,
    || FanAction::Pwm(0),
    || FanAction::Pwm(60),
    || FanAction::Pwm(100),
];

// ---------------------------------------------------------------------------
// 1 — un `unreadable` de canal connu donne une ligne de ventilateur illisible
// ---------------------------------------------------------------------------

#[test]
fn un_unreadable_de_canal_connu_donne_une_ligne_de_ventilateur_illisible() {
    // Critère d'acceptation n° 1 : « Un `unreadable` dont le sujet est un canal connu produit une
    // `LigneVentilateur` marquée illisible, et non un relevé de sonde ». Test d'intention n° 1 :
    // « un `unreadable` sur un sujet vu comme canal au tour précédent produit une ligne de
    // ventilateur illisible ».
    //
    // Les deux moitiés comptent, et la seconde est celle qui décrit le défaut : la ligne doit
    // apparaître **du bon côté**. Une implémentation qui la fabriquerait côté canaux tout en
    // continuant de la noter côté sondes ferait apparaître, dans le panneau des sondes, une carte
    // « kraken2023elite:fan-speed » grisée qui n'est pas une température.
    let mut tri = Tri::nouveau();

    let vu = tri.poser(&tour(&[]));
    assert!(
        ligne_de(&vu, MUET).lisible,
        "au premier tour le canal répond : sa ligne est lisible"
    );

    let apres = tri.poser(&tour(&[MUET]));
    let perdue = ligne_de(&apres, MUET);
    assert!(
        !perdue.lisible,
        "« {MUET} » ne répond plus : sa ligne reste, marquée illisible — une ligne absente \
         laisserait croire que le canal n'existe pas"
    );
    assert_eq!(
        releve_de(&apres, MUET),
        None,
        "« {MUET} » est un canal, pas une sonde : le noter comme un relevé illisible est \
         exactement le défaut de #100"
    );
}

// ---------------------------------------------------------------------------
// 2 — un `unreadable` de sonde continue d'aller aux sondes
// ---------------------------------------------------------------------------

#[test]
fn un_unreadable_de_sonde_reste_un_releve_de_sonde() {
    // Critère d'acceptation n° 2 : « Un `unreadable` dont le sujet est une sonde continue d'aller
    // aux sondes ». Test d'intention n° 2.
    //
    // La sonde choisie porte le **préfixe du canal muet** : `kraken2023elite:` nomme les deux. Un
    // tri qui reconnaîtrait un canal à son pilote — ce qui marche sur le banc et sur la machine —
    // ferait de la température du liquide une ligne de ventilateur, avec une poignée de régime
    // sous une valeur en millidegrés.
    let mut tri = Tri::nouveau();
    tri.poser(&tour(&[]));

    let apres = tri.poser(&tour(&[LIQUIDE]));
    assert_eq!(
        releve_de(&apres, LIQUIDE),
        Some(&Releve::Illisible),
        "une sonde muette reste un relevé illisible — c'est la règle que la fenêtre applique déjà"
    );
    assert_eq!(
        noms(&apres),
        TOUS.map(str::to_owned).to_vec(),
        "« {LIQUIDE} » n'est pas un canal : il n'a rien à faire dans la liste des ventilateurs"
    );

    // Et la sonde qui répond continue de rendre sa valeur, en millidegrés.
    assert_eq!(
        releve_de(&apres, CPU),
        Some(&Releve::Valeur(61_800)),
        "la sonde voisine n'a pas bougé : sa valeur passe, entière"
    );
}

#[test]
fn une_sonde_jamais_vue_lisible_va_quand_meme_aux_sondes() {
    // Le cas du premier tour, côté sondes : une sonde muette depuis le démarrage du démon n'a
    // jamais paru en `temp`. Elle doit malgré tout être notée comme un relevé illisible — c'est ce
    // que la fenêtre fait aujourd'hui, et #100 ne change que le sort des canaux.
    let mut tri = Tri::nouveau();

    let premier = tri.poser(&tour(&[LIQUIDE]));
    assert_eq!(
        releve_de(&premier, LIQUIDE),
        Some(&Releve::Illisible),
        "dès le premier tour, une sonde muette est un relevé illisible"
    );
    assert!(
        !noms(&premier).contains(&LIQUIDE.to_owned()),
        "et elle n'entre pas dans la liste des ventilateurs"
    );
}

// ---------------------------------------------------------------------------
// 3 — la ligne illisible garde la place du canal dans la liste
// ---------------------------------------------------------------------------

#[test]
fn la_ligne_illisible_garde_la_place_du_canal_dans_la_liste() {
    // Critère d'acceptation n° 3 : « L'ordre des canaux dans la liste ne change pas quand l'un
    // devient illisible ». Test d'intention n° 3.
    //
    // La faute visée a une forme précise et elle est naturelle à écrire : traiter les `chan`
    // d'abord, puis rattraper les `unreadable` à la fin. La liste reste complète, le compte est
    // bon, et les lignes se déplacent sous les doigts à chaque quarantaine — une poignée tirée
    // n'est plus celle qu'on croyait. C'est pourquoi chaque canal du banc est mis muet à son tour,
    // y compris le premier et le dernier : mettre le seul dernier en quarantaine ne montrerait
    // rien.
    let attendu = TOUS.map(str::to_owned).to_vec();

    for muet in TOUS {
        let mut tri = Tri::nouveau();
        tri.poser(&tour(&[]));
        let apres = tri.poser(&tour(&[muet]));
        assert_eq!(
            noms(&apres),
            attendu,
            "« {muet} » est illisible : sa ligne reste à sa place, elle ne va pas à la fin"
        );
        assert!(!ligne_de(&apres, muet).lisible);
    }
}

#[test]
fn l_ordre_tient_quand_les_canaux_reviennent_un_par_un() {
    // Le retour est le second moment où l'ordre se perd : un canal réapparu peut être poussé à la
    // fin de la liste par une implémentation qui le retire puis le rajoute. Ici les cinq canaux
    // partent ensemble, puis reviennent un par un, et la liste ne bouge jamais.
    let attendu = TOUS.map(str::to_owned).to_vec();
    let mut tri = Tri::nouveau();
    tri.poser(&tour(&[]));
    assert_eq!(noms(&tri.poser(&tour(&TOUS))), attendu);

    let mut muets: Vec<&str> = TOUS.to_vec();
    while !muets.is_empty() {
        let revenu = muets.remove(0);
        let vue = tri.poser(&tour(&muets));
        assert_eq!(
            noms(&vue),
            attendu,
            "« {revenu} » vient de revenir : il reprend sa place, pas la dernière"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — le retour du canal restaure une ligne lisible avec son régime
// ---------------------------------------------------------------------------

#[test]
fn le_retour_du_canal_restaure_une_ligne_lisible_avec_son_regime() {
    // Critère d'acceptation n° 4 : « Un canal qui redevient lisible reprend sa ligne normale, avec
    // sa mesure ». Test d'intention n° 4.
    //
    // Le régime du retour est **différent** de celui d'avant la quarantaine : sans ça, une
    // implémentation qui réafficherait la dernière valeur connue passerait le test sans avoir rien
    // relu.
    let mut tri = Tri::nouveau();
    tri.poser(&tour(&[]));
    tri.poser(&tour(&[MUET]));

    let mut revenu = Vec::new();
    for nom in TOUS {
        if nom == MUET {
            revenu.push(canal(nom, RPM_APRES, PWM_APRES));
        } else {
            revenu.push(canal(nom, RPM, PWM));
        }
    }
    let apres = tri.poser(&revenu);

    let ligne = ligne_de(&apres, MUET);
    assert!(
        ligne.lisible,
        "« {MUET} » répond de nouveau : sa ligne redevient une ligne normale"
    );
    assert_eq!(
        ligne.rpm,
        Some(RPM_APRES),
        "et elle montre la mesure du moment, pas celle d'avant la quarantaine"
    );
    assert_eq!(ligne.pwm, Some(PWM_APRES));
}

#[test]
fn un_canal_peut_partir_et_revenir_sans_fin() {
    // Le Kraken « cesse **périodiquement** de répondre » (issue #100, contexte) : l'aller-retour
    // n'arrive pas une fois. Une mémoire qui ne se remettrait à jour qu'au premier passage
    // laisserait la ligne collée dans l'un des deux états — et ce serait pire dans le sens
    // « lisible », où elle afficherait un régime qui ne bouge plus.
    let mut tri = Tri::nouveau();
    for _ in 0..5 {
        let vivant = tri.poser(&tour(&[]));
        assert!(ligne_de(&vivant, MUET).lisible);
        assert_eq!(ligne_de(&vivant, MUET).rpm, Some(RPM));

        let mort = tri.poser(&tour(&[MUET]));
        assert!(!ligne_de(&mort, MUET).lisible);
        assert_eq!(releve_de(&mort, MUET), None);
    }
}

// ---------------------------------------------------------------------------
// 5 — une ligne illisible ne maquille aucune mesure
// ---------------------------------------------------------------------------

#[test]
fn une_ligne_illisible_ne_montre_pas_la_derniere_mesure_connue() {
    // Comportement attendu n° 3 : « Sa dernière consigne connue n'est **pas** affichée comme si
    // elle était mesurée — même raison qu'une courbe de sonde qui figerait sa dernière valeur ».
    //
    // C'est le mode de défaillance rassurant, celui que le projet traite partout ailleurs de la
    // même façon : une sonde muette écrit des tirets, jamais un zéro ni la dernière valeur connue
    // (README, « le cadran »). Un 1200 tr/min figé derrière un ventilateur arrêté, c'est un boîtier
    // qui chauffe sans que rien ne le signale.
    let mut tri = Tri::nouveau();
    tri.poser(&tour(&[]));
    let apres = tri.poser(&tour(&[MUET]));

    let ligne = ligne_de(&apres, MUET);
    assert_eq!(
        ligne.rpm, None,
        "aucun régime n'a été mesuré à ce tour : en montrer un serait un maquillage"
    );
    assert_eq!(
        ligne.pwm, None,
        "la dernière consigne connue n'est pas une mesure — elle ne s'affiche pas comme telle"
    );
}

#[test]
fn un_canal_qui_repond_sans_regime_reste_illisible() {
    // L'issue relève le mécanisme existant : « `LigneVentilateur` porte pourtant déjà un champ
    // `lisible`, alimenté par `rpm.is_some()` ». La correction lui donne le cas qui lui manquait ;
    // elle ne doit pas lui retirer celui qu'il avait.
    //
    // `chan kraken2023elite:pump-speed - - firmware oui` est une ligne qu'un démon en bonne santé
    // écrit : le canal a répondu, mais sans tachymètre. La marquer lisible afficherait une ligne
    // normale avec un régime vide.
    let mut tri = Tri::nouveau();
    let mut lignes = Vec::new();
    for nom in TOUS {
        if nom == VOISIN {
            lignes.push(canal_sans_regime(nom));
        } else {
            lignes.push(canal(nom, RPM, PWM));
        }
    }
    let vue = tri.poser(&lignes);

    assert_eq!(
        noms(&vue),
        TOUS.map(str::to_owned).to_vec(),
        "un canal sans régime garde sa ligne et sa place"
    );
    assert!(
        !ligne_de(&vue, VOISIN).lisible,
        "sans régime mesuré, la ligne est illisible — c'est déjà ce que fait la fenêtre"
    );
}

// ---------------------------------------------------------------------------
// 6 — les commandes d'un canal illisible n'émettent rien
// ---------------------------------------------------------------------------

#[test]
fn les_commandes_d_un_canal_illisible_n_emettent_rien() {
    // Critère d'acceptation n° 5 : « Les commandes d'un canal illisible n'émettent rien vers le
    // démon ». Comportement attendu n° 2 : « Ses commandes sont inertes tant qu'il l'est : on ne
    // tire pas une poignée vers un canal qui ne répond pas ».
    //
    // Les quatre consignes sont essayées, `auto` comprise : un canal du Kraken sait faire auto
    // (#50), et c'est justement le bouton qu'on serait tenté d'aller chercher quand la ligne ne
    // montre plus rien.
    let mut tri = Tri::nouveau();
    let vivant = tri.poser(&tour(&[]));
    let muette = tri.poser(&tour(&[MUET]));

    for consigne in CONSIGNES {
        assert_eq!(
            ligne_de(&vivant, MUET).commande(consigne()),
            Some(Request::Fan {
                channel: MUET.to_owned(),
                action: consigne(),
            }),
            "tant que le canal répond, sa poignée commande ce canal-là"
        );
        assert_eq!(
            ligne_de(&muette, MUET).commande(consigne()),
            None,
            "« {MUET} » ne répond pas : {:?} ne part pas — écrire vers un canal en quarantaine \
             n'a aucun effet, et le démon paierait la lecture qui l'a fait écarter",
            consigne()
        );
    }
}

#[test]
fn les_commandes_reviennent_avec_le_canal() {
    // L'inertie est un état, pas une punition : le canal revenu se règle de nouveau. Une
    // implémentation qui marquerait la ligne inerte une fois pour toutes rendrait le Kraken
    // définitivement irréglable au premier hoquet de son pilote — et il en a périodiquement.
    let mut tri = Tri::nouveau();
    tri.poser(&tour(&[]));
    tri.poser(&tour(&[MUET]));
    let revenu = tri.poser(&tour(&[]));

    assert_eq!(
        ligne_de(&revenu, MUET).commande(FanAction::Pwm(80)),
        Some(Request::Fan {
            channel: MUET.to_owned(),
            action: FanAction::Pwm(80),
        }),
    );
}

#[test]
fn les_commandes_des_canaux_voisins_ne_sont_pas_emportees() {
    // Un canal muet ne fait pas taire ses voisins. La faute visée est celle que #88 nomme côté
    // démon — écarter par contrôleur plutôt que par canal —, transposée à la fenêtre : le Kraken
    // en rade rendrait alors les trois autres ventilateurs inertes eux aussi.
    let mut tri = Tri::nouveau();
    tri.poser(&tour(&[]));
    let apres = tri.poser(&tour(&[MUET, VOISIN]));

    for vivant in [VENTILO_1, VENTILO_2, BOITIER] {
        assert_eq!(
            ligne_de(&apres, vivant).commande(FanAction::Pwm(40)),
            Some(Request::Fan {
                channel: vivant.to_owned(),
                action: FanAction::Pwm(40),
            }),
            "« {vivant} » répond : sa poignée commande, quoi que fasse le Kraken"
        );
    }
}

// ---------------------------------------------------------------------------
// 7 — un `status` complet rend une ligne par canal, lisible ou non
// ---------------------------------------------------------------------------

#[test]
fn un_status_complet_rend_une_ligne_par_canal_lisible_ou_non() {
    // Test d'intention n° 5 : « Les quatorze cibles d'un `status` complet produisent le même nombre
    // de lignes, qu'elles soient lisibles ou non ».
    //
    // Les trente-deux combinaisons du banc sont essayées, de « tout répond » à « plus rien ne
    // répond ». Le compte et l'ordre sont les mêmes dans les trente-deux, et le nombre de lignes
    // marquées illisibles est exactement celui des canaux muets — un tri qui perdrait une ligne et
    // en dupliquerait une autre garderait le compte, pas la liste.
    let attendu = TOUS.map(str::to_owned).to_vec();

    for combinaison in 0..(1u32 << TOUS.len()) {
        let muets: Vec<&str> = TOUS
            .iter()
            .enumerate()
            .filter(|(i, _)| combinaison & (1 << *i) != 0)
            .map(|(_, nom)| *nom)
            .collect();

        let mut tri = Tri::nouveau();
        tri.poser(&tour(&[]));
        let vue = tri.poser(&tour(&muets));

        assert_eq!(
            noms(&vue),
            attendu,
            "avec {muets:?} en quarantaine, la liste garde ses cinq canaux dans l'ordre"
        );
        assert_eq!(
            vue.canaux.iter().filter(|l| !l.lisible).count(),
            muets.len(),
            "avec {muets:?} en quarantaine, il y a exactement autant de lignes illisibles"
        );
        for muet in &muets {
            assert!(!ligne_de(&vue, muet).lisible);
        }
    }
}

#[test]
fn l_incident_observe_les_deux_canaux_du_kraken_ensemble() {
    // L'incident tel qu'il a été signalé : « il arrive que dans Reverb je ne vois pas les lignes du
    // Kraken pour les régler manuellement », et les deux canaux partent ensemble. C'est le scénario
    // complet, de bout en bout, et il vaut la peine d'être écrit tel quel — c'est celui qu'on
    // rejouera à la main devant la fenêtre.
    let mut tri = Tri::nouveau();
    tri.poser(&tour(&[]));
    let apres = tri.poser(&tour(&[MUET, VOISIN]));

    assert_eq!(noms(&apres), TOUS.map(str::to_owned).to_vec());
    for muet in [MUET, VOISIN] {
        let ligne = ligne_de(&apres, muet);
        assert!(
            !ligne.lisible,
            "« {muet} » : la ligne est là, et elle est grise"
        );
        assert_eq!(ligne.rpm, None);
        assert_eq!(ligne.pwm, None);
        assert_eq!(ligne.commande(FanAction::Pwm(50)), None);
        assert_eq!(
            releve_de(&apres, muet),
            None,
            "« {muet} » n'apparaît pas non plus en sonde illisible"
        );
    }
    // Et les sondes du même contrôleur continuent leur vie : le liquide est lu par une autre
    // ligne, qui n'est pas concernée par l'état des deux canaux.
    assert_eq!(releve_de(&apres, LIQUIDE), Some(&Releve::Valeur(34_200)));
}

// ---------------------------------------------------------------------------
// 8 — le premier tour : ce qui est ouvert, et ce qui ne l'est pas
// ---------------------------------------------------------------------------

#[test]
fn un_canal_muet_des_le_premier_tour_ne_disparait_jamais() {
    // ⚠️ L'approche technique de l'issue laisse ce cas ouvert : « au **tout premier** tour, un canal
    // muet depuis le démarrage du démon n'a jamais été vu lisible : la fenêtre ne peut pas savoir
    // que c'est un canal. Deux issues […] à trancher à l'implémentation ».
    //
    // Ce test ne tranche donc pas — il exige seulement ce qui est vrai des deux côtés du choix :
    // **le sujet ne disparaît pas**. Qu'il paraisse en ligne de ventilateur grisée (le démon dit sa
    // nature) ou en relevé de sonde illisible (la fenêtre l'ignore encore), il est quelque part.
    // Le silence, lui, est le défaut de #100 et n'est acceptable dans aucune des deux branches.
    let mut tri = Tri::nouveau();
    let premier = tri.poser(&tour(&[MUET]));

    let en_canal = noms(&premier).contains(&MUET.to_owned());
    let en_sonde = releve_de(&premier, MUET).is_some();
    assert!(
        en_canal || en_sonde,
        "au premier tour, « {MUET} » n'a jamais été vu lisible : la fenêtre peut le prendre pour \
         une sonde, mais elle ne peut pas le perdre. Canaux rendus : {:?}",
        noms(&premier)
    );

    // Et dès qu'il a été vu répondre une fois, le doute n'existe plus : c'est un canal.
    tri.poser(&tour(&[]));
    let apres = tri.poser(&tour(&[MUET]));
    assert!(!ligne_de(&apres, MUET).lisible);
    assert_eq!(releve_de(&apres, MUET), None);
}

// ---------------------------------------------------------------------------
// 9 — le tri ne se laisse pas distraire par le reste de la réponse
// ---------------------------------------------------------------------------

#[test]
fn les_lignes_qui_ne_sont_pas_de_la_telemetrie_ne_font_ni_canal_ni_sonde() {
    // Un `status` ne porte pas que des canaux et des sondes : il dit aussi l'éclairage, la
    // géométrie, l'écran, et il se termine par `end`. Aucune de ces lignes n'est un relevé, et
    // aucune ne doit se retrouver dans l'une des deux listes — un tri qui rattraperait « tout ce
    // qui n'est pas `chan` » côté sondes y ferait entrer l'état de l'écran.
    let mut tri = Tri::nouveau();
    let attendue = tri.poser(&tour(&[MUET]));

    let mut bruyant = tour(&[MUET]);
    for texte in [
        "light fan:arriere ff2080",
        "anim arc-en-ciel",
        "geom radiateur-haut 90 horaire",
        "screen 50 layout",
        "end",
    ] {
        bruyant.push(ligne(texte));
    }
    let mut tri = Tri::nouveau();
    let obtenue = tri.poser(&bruyant);

    assert_eq!(
        noms(&obtenue),
        noms(&attendue),
        "les lignes d'éclairage et d'écran ne sont pas des canaux"
    );
    assert_eq!(
        obtenue.sondes.len(),
        attendue.sondes.len(),
        "elles ne sont pas non plus des relevés de sonde"
    );
}

#[test]
fn la_raison_a_espaces_ne_coupe_pas_le_sujet() {
    // La raison de l'issue porte des espaces : « Connection timed out (os error 110) ». Un tri qui
    // relirait la ligne en coupant au blanc — plutôt que de lire le `subject` que le protocole a
    // déjà séparé — chercherait un canal nommé « Connection », ne le trouverait pas, et
    // reproduirait le défaut avec un tout autre air.
    let mut tri = Tri::nouveau();
    tri.poser(&tour(&[]));

    for raison in [RAISON, AUTRE_RAISON, "Remote I/O error (os error 121)"] {
        let mut lignes = Vec::new();
        for nom in TOUS {
            if nom == MUET {
                lignes.push(illisible(nom, raison));
            } else {
                lignes.push(canal(nom, RPM, PWM));
            }
        }
        let vue = tri.poser(&lignes);
        assert!(
            !ligne_de(&vue, MUET).lisible,
            "raison « {raison} » : le sujet reste « {MUET} », entier"
        );
    }
}
