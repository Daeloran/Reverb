//! Tests d'intention du **désarmement** de la réparation automatique (issue #136).
//!
//! Écrits **avant** l'implémentation, depuis les issues #136 et #98 seules. Aucun corps de fonction
//! de `crates/*/src/` n'a été ouvert pour les produire ; seuls l'ont été les fichiers de tests
//! d'intention de #68 (`spec_quarantaine.rs`), de #88 (`spec_canaux_muets.rs`) et de #98
//! (`spec_reparation_source.rs`), plus les signatures publiques de `reverb_daemon::quarantaine` et
//! `reverb_daemon::reparation`. À l'écriture de ce fichier, `Veille`, `Alerte`,
//! `RefusDeReparation` et `demande_de_reparation` **n'existent pas** : la compilation doit échouer,
//! et c'est la phase rouge.
//!
//! Rien ici n'ouvre de périphérique, ne réinitialise quoi que ce soit, ni ne dort. Le temps est
//! **injecté**, comme dans #68, #88 et #98. **Aucun `sleep`, nulle part.**
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Relevé sur SHYNAEL le 2026-08-16. Le Kraken cesse de répondre à 12:53:37 ; à 12:53:50, toutes
//! ses cibles étant muettes, le démon tente le reset USB de #98 :
//!
//! ```text
//! 12:53:50  reverbd  réparation : reset USB de « kraken2023elite » (BB8C90820E900630)
//! 12:53:55  kernel   usb 1-9.1: device descriptor read/64, error -110
//! 12:54:53  kernel   usb 1-9.1: USB disconnect, device number 5
//! 12:55:35  kernel   usb 1-9-port1: attempt power cycle
//! 12:55:56  kernel   usb 1-9-port1: unable to enumerate USB device
//! ```
//!
//! `lsusb` ne voit plus `1e71:300c`, `hwmon5` a disparu, et **seule une coupure d'alimentation
//! complète l'a ramené** — cycle d'alimentation du port par le noyau compris.
//!
//! ⚠️ **Ce que la chronologie établit, et ce qu'elle n'établit pas.** Le blocage précède le reset
//! de treize secondes : le reset n'a pas causé la panne. Mais le noyau ne se plaint qu'**après**
//! lui. Sur un seul incident, « le firmware s'est enfoncé seul » et « notre reset a transformé un
//! blocage récupérable en un périphérique inénumérable » sont indiscernables.
//!
//! Ce qui, lui, est établi sur les **trois** incidents connus : aucun reset n'a jamais ramené le
//! Kraken. Le geste ne guérit rien de mesuré, et il est le seul `ioctl` du projet qui fasse
//! disparaître un périphérique du bus. Il garde sa place — **sous la main de l'utilisateur, pas en
//! automatique**.
//!
//! ## Ce que ce fichier NE remet PAS en cause
//!
//! - **La quarantaine (#68, #88) ne bouge pas.** Elle reste automatique : c'est elle qui empêche
//!   une lecture muette de geler le fil qui sert le socket, et ses retentes doublent comme avant.
//!   L'issue la met explicitement hors scope.
//! - **La mécanique de #98 ne bouge pas** : trois tentatives bornées, espacées de trente secondes,
//!   sur le fil de réparation, puis redécouverte par nom et oubli des quarantaines.
//!   `spec_reparation_source.rs` et `unit_fil_reparation.rs` doivent continuer de passer **tels
//!   quels**, et ce fichier n'y touche pas.
//!
//! ⚠️ **`Reparations::tour` change donc de statut sans changer de code** : elle était la boucle
//! automatique, elle devient le **geste demandé**. C'est pour cela que les tests de #98 survivent
//! intacts — ils décrivent ce qui se passe *une fois qu'on a demandé*, ce qui reste vrai mot pour
//! mot. Ce que #136 retire est la ligne, ailleurs, qui appelait cette mécanique toute seule.
//!
//! ## La couture que ces tests exigent, et pourquoi celle-là
//!
//! L'issue pose la politique — plus de déclenchement automatique, un signalement unique par
//! épisode, un verbe qui dépose, deux refus — mais pas sa forme. Ce fichier la tranche, et voici ce
//! qu'il faut implémenter :
//!
//! ```ignore
//! // crates/reverb-daemon/src/reparation.rs — deux ajouts ; `Reparations`, `Constat` et
//! // `EtatSource` sont inchangés.
//!
//! /// Ce qu'un tour de **constat** apprend d'une source.
//! #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//! pub enum Alerte {
//!     /// La source répond, au moins par une cible. L'épisode en cours, s'il y en avait un, est
//!     /// clos.
//!     Rien,
//!     /// La source vient de se taire entièrement. À journaliser **une fois**, en nommant la
//!     /// commande à taper.
//!     Signaler,
//!     /// Elle se tait toujours, et c'est déjà dit. Rien à faire, rien à écrire.
//!     DejaDite,
//! }
//!
//! pub struct Veille { /* un épisode par source */ }
//!
//! impl Veille {
//!     pub fn nouvelle() -> Veille;
//!
//!     /// Un tour de constat, pour **une** source.
//!     ///
//!     /// ⚠️ **Aucune fermeture, aucun descripteur, aucun chemin.** « Ne provoque plus aucun
//!     /// `USBDEVFS_RESET` de lui-même » devient une propriété de cette signature, pas une
//!     /// promesse de son corps.
//!     pub fn tour(&mut self, etat: &EtatSource) -> Alerte;
//! }
//!
//! /// Pourquoi une demande `repare` est refusée.
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub enum RefusDeReparation {
//!     /// Aucune source relevée ne porte ce nom. `connues` les liste **toutes**.
//!     SourceInconnue { demandee: String, connues: Vec<String> },
//!     /// La source existe, mais au moins une de ses cibles répond encore.
//!     /// `vivantes` nomme celles qui répondent.
//!     SourceRepond { source: String, vivantes: Vec<String> },
//! }
//!
//! impl std::fmt::Display for RefusDeReparation { /* … */ }
//!
//! /// La demande `repare <source>`, jugée sur ce qui a été relevé.
//! ///
//! /// Rend l'état à déposer sur le fil de réparation, ou le refus qui dit pourquoi.
//! ///
//! /// ⚠️ Ni descripteur, ni chemin, ni périphérique : le refus est un **calcul**, exactement
//! /// comme `refus_de_consigne` (#101). « Rien n'est écrit » se lit dans la signature.
//! pub fn demande_de_reparation(
//!     source: &str,
//!     sources: &[EtatSource],
//! ) -> Result<EtatSource, RefusDeReparation>;
//! ```
//!
//! Quatre choix, et ce qu'ils achètent :
//!
//! 1. **`Veille::tour` ne prend aucune fermeture, et c'est tout l'objet de #136.** #98 avait pris
//!    la fermeture pour que `tour` **soit** l'endroit où le reset a lieu ; ici on veut l'inverse,
//!    et la façon la plus forte de l'obtenir est de ne pas lui donner de quoi le faire. Un test qui
//!    compterait les gestes vérifierait qu'on n'en fait pas ; une signature sans fermeture rend
//!    l'idée même irreprésentable — la règle de `SlotAddress` (#15) et de `NomProfil` (#74),
//!    appliquée à un geste plutôt qu'à une adresse.
//! 2. **`Veille` est un type à part, pas une méthode de plus sur `Reparations`.** Les deux états
//!    n'ont ni la même durée de vie ni le même propriétaire : la veille tourne à chaque tour du
//!    démon, la réparation ne vit qu'après une demande. Les mêler ferait porter à un seul type le
//!    compteur qu'on veut manuel et l'épisode qu'on veut automatique, c'est-à-dire exactement la
//!    confusion que l'issue défait.
//! 3. **Le refus est un calcul, pas une relecture** — la règle posée par #101 pour
//!    `refus_de_consigne`, et elle vaut plus encore ici : ce qu'on refuse est un geste qui fait
//!    quitter le bus à un périphérique. `demande_de_reparation` ne reçoit que des noms et des
//!    listes ; « rien n'est écrit » n'est pas une promesse, c'est sa signature.
//! 4. **La demande accepte en rendant l'`EtatSource` à déposer**, et non un `bool`. C'est ce qui
//!    branche le verbe sur la mécanique de #98 sans qu'aucun appelant ait à reconstruire l'état :
//!    `FilReparation::soumettre` prend exactement ça.
//!
//! ## Ce que ce fichier fige
//!
//! 1. **Une source entièrement muette ne provoque aucun geste**, sur une heure de tours, et par
//!    construction.
//! 2. **Elle est signalée une fois par épisode**, puis plus rien — jamais.
//! 3. **Une source revenue puis re-effondrée est un épisode neuf**, et se signale à nouveau.
//! 4. **Une source qui répond, ne serait-ce que par une cible, ne se signale jamais** ; une source
//!    sans cible non plus — « toutes ses cibles se taisent » est vrai à vide.
//! 5. **Chaque source a son épisode**, indépendamment des autres.
//! 6. **`repare` sur une source inconnue est refusé en listant les sources connues** — toutes, pas
//!    seulement les muettes.
//! 7. **`repare` sur une source dont une cible répond encore est refusé en nommant ce qui répond**.
//! 8. **`repare` sur une source entièrement muette rend l'état à déposer**, tel quel.
//! 9. **La quarantaine et ses délais ne changent pas**, et l'oubli après une réparation réussie
//!    reste celui de #98.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Le `USBDEVFS_RESET` lui-même.** C'est le geste, il vit dans `reverb-hw` derrière la
//!   fermeture de `Reparations::tour`, et l'éprouver demanderait de réinitialiser un vrai
//!   périphérique — celui-là même dont l'issue relève qu'il n'en revient pas.
//! - **Les trois tentatives bornées et leur espacement.** Ils sont figés par
//!   `spec_reparation_source.rs` (#98), que #136 laisse intact. Ce fichier ne vérifie que la
//!   **jonction** : l'état rendu par une demande acceptée alimente bien cette mécanique-là.
//! - **Le compte rendu rendu au client sur le socket.** L'issue en demande un — « rend un compte
//!   rendu de ce qui s'est passé » — sans en décrire la forme, et le geste dure deux délais de
//!   trente secondes, soit bien plus qu'une requête. Inventer ici une ligne de réponse figerait un
//!   contrat que personne n'a choisi.
//! - **Le texte de la ligne de journal**, qui doit nommer « repare <source> ». Elle vit dans le
//!   journal du démon, pas dans une valeur ; ce que ce fichier garantit est que le nom de la source
//!   est disponible au moment où elle s'écrit, et qu'elle ne s'écrit qu'une fois.
//! - **Que `reverb repare` passe par le démon quand il tourne.** C'est un critère de la ligne de
//!   commande, qui demande un démon vivant — comme `screen` (#33) et `curve` (#104), qui n'ont pas
//!   de test d'intention non plus sur ce point.

use std::cell::RefCell;
use std::time::Duration;

use reverb_daemon::quarantaine::{DELAI_INITIAL, Quarantaine, Releve};
use reverb_daemon::reparation::{
    Alerte, Constat, DELAI_ENTRE_TENTATIVES, EtatSource, RefusDeReparation, Reparations,
    TENTATIVES_MAXIMALES, Veille, demande_de_reparation,
};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// L'origine du temps injecté, volontairement loin de zéro : le démon tourne depuis des heures
/// quand le Kraken lâche — les trois incidents relevés sont survenus à 23:22, 12:29 et 12:53.
const DEBUT: Duration = Duration::from_secs(3_600);

/// La cadence des tours du démon : la fenêtre demande `status` une fois par seconde (#88).
const CYCLE: Duration = Duration::from_secs(1);

/// La source qui lâche : le Kraken, nommé comme son pilote `hwmon`.
const KRAKEN: &str = "kraken2023elite";

/// Ses trois cibles, recopiées telles quelles des lignes de journal des issues.
const FAN: &str = "kraken2023elite:fan-speed";
const POMPE: &str = "kraken2023elite:pump-speed";
const LIQUIDE: &str = "kraken2023elite:coolant-temp";
const CIBLES_KRAKEN: [&str; 3] = [FAN, POMPE, LIQUIDE];

/// Une seconde source, qui elle répond : le contrôleur de ventilation NZXT.
const SMART2: &str = "nzxtsmart2";
const VENTILO_1: &str = "nzxtsmart2:fan-1";
const VENTILO_2: &str = "nzxtsmart2:fan-2";
const CIBLES_SMART2: [&str; 2] = [VENTILO_1, VENTILO_2];

/// Une troisième source, qui n'a qu'une cible : le pilote du CPU.
const CPU: &str = "k10temp";
const TCTL: &str = "k10temp:tctl";
const CIBLES_CPU: [&str; 1] = [TCTL];

/// Un nom qui ne désigne aucune source relevée — la faute de frappe la plus probable, puisque
/// « kraken2023elite » se recopie à la main depuis une réponse `status`.
const INCONNUE: &str = "kraken2023";

/// Ce que rend un `USBDEVFS_RESET` refusé.
const RAISON_DU_RESET: &str = "USBDEVFS_RESET: No such device (os error 19)";

/// Ce qu'une sonde du Kraken rend quand elle va bien : 34,2 °C, en millidegrés.
const TEMPERATURE_LIQUIDE: i32 = 34_200;

/// La valeur qu'une cible en quarantaine rendrait **si** on la relevait.
const PIEGE: i32 = -999_999;

// ---------------------------------------------------------------------------
// Le banc
// ---------------------------------------------------------------------------

/// L'état d'une source, avec les cibles muettes qu'on lui désigne.
///
/// ⚠️ `muettes` est toujours un sous-ensemble de `cibles` — le test n° 0 le vérifie.
fn etat(source: &str, cibles: &[&str], muettes: &[&str]) -> EtatSource {
    EtatSource {
        source: source.to_owned(),
        cibles: cibles.iter().map(|c| (*c).to_owned()).collect(),
        muettes: muettes.iter().map(|c| (*c).to_owned()).collect(),
    }
}

/// Une source dont **toutes** les cibles se taisent — l'effondrement des issues.
fn effondree(source: &str, cibles: &[&str]) -> EtatSource {
    etat(source, cibles, cibles)
}

/// Une source qui va bien : aucune cible muette.
fn intacte(source: &str, cibles: &[&str]) -> EtatSource {
    etat(source, cibles, &[])
}

/// Les trois sources de la machine, dans l'état où l'incident du 2026-08-16 les a laissées : le
/// Kraken entièrement muet, les deux autres en pleine forme.
fn machine_en_panne() -> Vec<EtatSource> {
    vec![
        effondree(KRAKEN, &CIBLES_KRAKEN),
        intacte(SMART2, &CIBLES_SMART2),
        intacte(CPU, &CIBLES_CPU),
    ]
}

/// Un tour de quarantaine pour une cible, avec ce que la lecture rendrait.
fn releve(
    quarantaine: &mut Quarantaine,
    lectures: &RefCell<Vec<String>>,
    cible: &str,
    maintenant: Duration,
    reponse: Option<i32>,
) -> Releve<i32> {
    quarantaine.tour(cible, maintenant, || {
        lectures.borrow_mut().push(cible.to_owned());
        reponse
    })
}

/// Le refus d'une demande, ou un échec qui dit ce qu'on a reçu à la place.
fn refus(source: &str, sources: &[EtatSource]) -> RefusDeReparation {
    match demande_de_reparation(source, sources) {
        Err(refus) => {
            // Un refus qui ne se lit pas n'aide personne : il finit dans une réponse `err` du
            // socket, que l'utilisateur lit pour taper la bonne commande derrière.
            let message = refus.to_string();
            assert!(
                !message.trim().is_empty(),
                "un refus de réparation doit dire pourquoi"
            );
            refus
        }
        Ok(etat) => panic!(
            "« repare {source} » devait être refusée, elle a rendu l'état de « {} » à déposer",
            etat.source
        ),
    }
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tous les tests qui suivent supposent que les trois sources sont distinctes, que les fabriques
    // d'état sont bien formées, et que le nom inconnu ne désigne vraiment rien. Si l'un de ces
    // repères se dégradait, plusieurs tests deviendraient vrais sans rien vérifier — et personne ne
    // le verrait.
    let sources = [KRAKEN, SMART2, CPU];
    for (i, source) in sources.iter().enumerate() {
        assert!(!source.is_empty());
        assert!(
            !source.contains('/') && !source.contains(':'),
            "« {source} » ressemble à un chemin ou à une cible : une source se nomme, elle ne se \
             situe pas — les numéros `hwmonN` changent au redémarrage, et depuis #98 à chaque reset"
        );
        for autre in sources.iter().skip(i + 1) {
            assert_ne!(source, autre, "deux sources portent le même nom");
        }
    }

    for etat in machine_en_panne() {
        assert!(
            !etat.cibles.is_empty(),
            "« {} » doit avoir au moins une cible, sinon les tests de silence sont vrais à vide",
            etat.source
        );
        for muette in &etat.muettes {
            assert!(
                etat.cibles.contains(muette),
                "« {muette} » est donnée muette mais n'est pas une cible de « {} » : cet état est \
                 mal formé",
                etat.source
            );
        }
    }

    // Le Kraken est bien le seul effondré de la machine en panne : c'est ce qui donne son sens au
    // refus « cette source répond encore » sur les deux autres.
    let en_panne = machine_en_panne();
    let muettes: Vec<&str> = en_panne
        .iter()
        .filter(|e| !e.cibles.is_empty() && e.muettes.len() == e.cibles.len())
        .map(|e| e.source.as_str())
        .collect();
    assert_eq!(
        muettes,
        vec![KRAKEN],
        "une seule source doit être entièrement muette dans le décor"
    );

    // Et le nom inconnu ne désigne aucune d'elles — ni par égalité, ni par préfixe : une
    // implémentation qui reconnaîtrait un nom tronqué passerait le test du refus sans qu'on le
    // voie.
    for source in sources {
        assert_ne!(INCONNUE, source);
        assert!(
            !source.eq_ignore_ascii_case(INCONNUE),
            "« {INCONNUE} » ne doit désigner « {source} » sous aucune lecture"
        );
    }

    // Les valeurs de sonde diffèrent du piège : sans quoi une quarantaine qui lit ce qu'elle ne
    // devrait pas passerait inaperçue.
    assert_ne!(TEMPERATURE_LIQUIDE, PIEGE);
}

// ---------------------------------------------------------------------------
// 1 — une source entièrement muette n'écrit plus rien sur le bus
// ---------------------------------------------------------------------------

#[test]
fn une_source_entierement_muette_n_ecrit_rien_sur_le_bus() {
    // issue #136, premier critère d'acceptation — « Une source dont **toutes** les cibles sont
    // muettes ne provoque plus aucun `USBDEVFS_RESET` de lui-même. »
    //
    // C'est le renversement exact de #98, et il ne se démontre pas en comptant des gestes : compter
    // supposerait qu'il y ait de quoi en faire un. `Veille::tour` ne reçoit **aucune fermeture** —
    // ni descripteur, ni chemin, ni périphérique —, donc « ne réinitialise rien » n'est pas une
    // promesse de son corps, c'est une propriété de sa signature. Ce test est l'endroit où cette
    // signature est appelée ; le jour où quelqu'un lui ajouterait de quoi agir, il cesserait de
    // compiler.
    //
    // Reste à vérifier ce que la signature ne dit pas : que le constat tienne une heure durant sans
    // jamais rien réclamer d'autre que du silence.
    let mut veille = Veille::nouvelle();
    let effondre = effondree(KRAKEN, &CIBLES_KRAKEN);

    // ⚠️ Aucun instant n'est injecté ici, et c'en est une conséquence : un constat qui ne fait rien
    // n'a pas d'échéance à respecter. Le temps de #98 mesurait l'espacement de deux gestes ; sans
    // geste, il n'a plus rien à mesurer.
    let mut signalements = 0u32;
    for i in 0..3_600u32 {
        let alerte = veille.tour(&effondre);
        if alerte == Alerte::Signaler {
            signalements += 1;
        }
        assert_ne!(
            alerte,
            Alerte::Rien,
            "tour n° {i} : « {KRAKEN} » se tait entièrement, ce n'est pas « rien »"
        );
    }

    assert_eq!(
        signalements, 1,
        "une heure de silence complet doit produire exactement un signalement, pas {signalements}"
    );
}

// ---------------------------------------------------------------------------
// 2 — elle est signalée une fois, pas à chaque tour
// ---------------------------------------------------------------------------

#[test]
fn une_source_entierement_muette_est_signalee_une_fois_et_pas_a_chaque_tour() {
    // issue #136, deuxième critère d'acceptation — « Le démon le signale **une seule fois** par
    // épisode, en nommant la commande. »
    //
    // « Une seule fois » est une propriété d'**état**, pas de sortie : si le constat ne disait pas
    // « c'est la première fois », l'appelant n'aurait d'autre choix que de journaliser à chaque
    // tour, soit une ligne par seconde pour toujours, dans un journal qu'on lit justement pour
    // trouver ce genre d'incident. C'est la règle de l'abandon de #98 et du `Repos` de la dalle
    // (#83), reprise sur le constat lui-même.
    //
    // Le chiffre n'est pas théorique : la fenêtre demande `status` une fois par seconde, donc
    // 86 400 lignes par jour — le même chiffre qui justifiait le cache de consignes de #110,
    // retourné contre le journal.
    let mut veille = Veille::nouvelle();
    let effondre = effondree(KRAKEN, &CIBLES_KRAKEN);

    assert_eq!(
        veille.tour(&effondre),
        Alerte::Signaler,
        "le tour de l'effondrement est celui qui se journalise : c'est là qu'on nomme « repare \
         {KRAKEN} »"
    );

    for i in 1..=86_400u32 {
        assert_eq!(
            veille.tour(&effondre),
            Alerte::DejaDite,
            "tour n° {i} : le silence est déjà dit, il n'y a rien à ajouter — une journée de tours \
             écrirait 86 400 fois la même ligne"
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — une source qui répond encore ne se signale jamais
// ---------------------------------------------------------------------------

#[test]
fn une_source_qui_repond_encore_ne_se_signale_jamais() {
    // La garde de #98 — « une seule cible muette ne déclenche rien » — devient ici une garde sur le
    // **signalement** : annoncer « kraken2023elite ne répond plus sur aucune de ses cibles » alors
    // qu'une répond serait une ligne fausse, et elle inviterait à taper une commande que #136 fait
    // par ailleurs refuser. Les deux moitiés doivent dire la même chose.
    //
    // Tous les silences partiels sont essayés, une cible après l'autre, puis deux sur trois — parce
    // qu'une implémentation qui compterait « au moins deux » au lieu de « toutes » passerait le
    // premier cas et échouerait au second.
    let mut veille = Veille::nouvelle();

    let partiels = [
        vec![FAN],
        vec![POMPE],
        vec![LIQUIDE],
        vec![FAN, POMPE],
        vec![FAN, LIQUIDE],
        vec![POMPE, LIQUIDE],
        vec![],
    ];

    for i in 0..3_600u32 {
        let muettes = &partiels[(i as usize) % partiels.len()];
        assert_eq!(
            veille.tour(&etat(KRAKEN, &CIBLES_KRAKEN, muettes)),
            Alerte::Rien,
            "tour n° {i} : « {KRAKEN} » répond encore par {} cible(s) sur {} — il n'y a rien à \
             signaler, et rien à réparer",
            CIBLES_KRAKEN.len() - muettes.len(),
            CIBLES_KRAKEN.len()
        );
    }
}

#[test]
fn une_source_sans_cible_ne_se_signale_jamais() {
    // « Toutes ses cibles sont muettes » est **vrai à vide**, et c'est le piège classique de cette
    // formulation — #98 l'avait déjà désamorcé pour le geste, il faut le désamorcer pour la parole.
    //
    // Le cas n'est pas théorique : une découverte qui échoue, un `hwmon` dépouillé, une source
    // repérée avant que ses cibles ne le soient — et le journal annoncerait la mort d'un
    // périphérique dont il n'a jamais rien lu, en invitant à le réinitialiser.
    let mut veille = Veille::nouvelle();
    let vide = effondree(KRAKEN, &[]);
    assert!(vide.cibles.is_empty() && vide.muettes.is_empty());

    for i in 0..600u32 {
        assert_eq!(
            veille.tour(&vide),
            Alerte::Rien,
            "tour n° {i} : une source dont on ne connaît aucune cible n'a montré aucun symptôme"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — un épisode se referme, et un second se rouvre
// ---------------------------------------------------------------------------

#[test]
fn une_source_revenue_puis_re_effondree_se_signale_a_nouveau() {
    // « Une seule fois **par épisode** » : la faute symétrique de celle du test n° 2 est une veille
    // qui, pour ne pas répéter, se tairait à jamais.
    //
    // Le Kraken a lâché trois fois en huit jours, et il est revenu entre chaque — par un
    // redémarrage, mais il est revenu. Une source qui répond de nouveau a fait la preuve qu'elle
    // répond : son prochain effondrement est un incident neuf, qui mérite sa ligne de journal.
    // C'est la même règle que « une sonde guérie qui retombe se journalise à nouveau » (#68), et
    // pour la même raison : un contrôleur qui clignote est justement celui dont on veut entendre
    // parler.
    let mut veille = Veille::nouvelle();
    let effondre = effondree(KRAKEN, &CIBLES_KRAKEN);
    let vivante = intacte(KRAKEN, &CIBLES_KRAKEN);
    let partielle = etat(KRAKEN, &CIBLES_KRAKEN, &[FAN, POMPE]);

    for episode in 1..=3u32 {
        assert_eq!(
            veille.tour(&effondre),
            Alerte::Signaler,
            "épisode n° {episode} : un effondrement neuf se signale, quel qu'ait été le précédent"
        );
        for i in 1..=600u32 {
            assert_eq!(
                veille.tour(&effondre),
                Alerte::DejaDite,
                "épisode n° {episode}, tour n° {i}"
            );
        }

        // Le retour : **une seule** cible qui répond suffit à clore l'épisode. C'est la même règle
        // que pour le déclenchement, prise par l'autre bout — sans quoi une source à moitié revenue
        // resterait comptée pour morte, et son effondrement complet suivant ne se dirait pas.
        assert_eq!(
            veille.tour(&partielle),
            Alerte::Rien,
            "épisode n° {episode} : une seule cible qui répond suffit, la source n'est plus muette"
        );
        for i in 0..600u32 {
            assert_eq!(
                veille.tour(&vivante),
                Alerte::Rien,
                "épisode n° {episode}, tour n° {i} après le retour"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — chaque source a son épisode
// ---------------------------------------------------------------------------

#[test]
fn deux_sources_ont_chacune_leur_episode() {
    // #98 demandait déjà « un mécanisme qui ne soit pas propre au Kraken ». La forme la plus serrée
    // de cette exigence pour la veille : une source qui s'effondre ne consomme pas le signalement
    // d'une autre.
    //
    // Une veille qui garderait un drapeau global passerait tous les tests précédents — un seul
    // Kraken y figure — et se ferait attraper ici : la seconde source, qui vient de se taire,
    // hériterait du « déjà dit » de la première et son effondrement n'apparaîtrait nulle part.
    let mut veille = Veille::nouvelle();
    let kraken_muet = effondree(KRAKEN, &CIBLES_KRAKEN);
    let smart2_muet = effondree(SMART2, &CIBLES_SMART2);
    let smart2_vivant = intacte(SMART2, &CIBLES_SMART2);
    let cpu_vivant = intacte(CPU, &CIBLES_CPU);

    // Le Kraken tombe pendant que les deux autres vont bien.
    assert_eq!(veille.tour(&kraken_muet), Alerte::Signaler);
    for i in 0..60u32 {
        assert_eq!(veille.tour(&kraken_muet), Alerte::DejaDite, "tour n° {i}");
        assert_eq!(
            veille.tour(&smart2_vivant),
            Alerte::Rien,
            "« {SMART2} » va bien : l'effondrement de « {KRAKEN} » ne la concerne pas"
        );
        assert_eq!(veille.tour(&cpu_vivant), Alerte::Rien);
    }

    // Puis la seconde source tombe à son tour, bien plus tard : elle a droit à son propre
    // signalement, alors que la première est encore muette et déjà dite.
    assert_eq!(
        veille.tour(&smart2_muet),
        Alerte::Signaler,
        "« {SMART2} » vient de se taire : c'est un incident neuf, quel que soit l'état de \
         « {KRAKEN} »"
    );
    assert_eq!(
        veille.tour(&kraken_muet),
        Alerte::DejaDite,
        "et « {KRAKEN} » n'est pas re-signalée par l'incident de sa voisine"
    );
    assert_eq!(veille.tour(&smart2_muet), Alerte::DejaDite);
    assert_eq!(veille.tour(&cpu_vivant), Alerte::Rien);
}

// ---------------------------------------------------------------------------
// 6 — `repare` sur une source inconnue est refusé en listant les sources
// ---------------------------------------------------------------------------

#[test]
fn repare_sur_une_source_inconnue_est_refuse_en_listant_les_sources() {
    // issue #136, critère d'acceptation — « `repare` sur une source inconnue est refusé **en
    // listant les sources connues**. »
    //
    // Lister n'est pas une politesse : « kraken2023elite » se recopie à la main depuis une réponse
    // `status`, et le seul autre moyen d'en retrouver l'orthographe est de relancer `status` et de
    // relire seize lignes. Un refus qui dirait « source inconnue » et rien d'autre obligerait à ce
    // détour à chaque faute de frappe — devant un boîtier dont on vient de constater qu'il ne
    // répond plus.
    let sources = machine_en_panne();

    for demandee in [
        INCONNUE,
        "",
        "KRAKEN2023ELITE",
        "kraken",
        "kraken2023elite ",
        LIQUIDE,
        "all",
        "list",
    ] {
        let RefusDeReparation::SourceInconnue {
            demandee: nommee,
            connues,
        } = refus(demandee, &sources)
        else {
            panic!(
                "« repare {demandee} » ne désigne aucune source relevée : le refus doit le dire, \
                 pas prétendre que la source répond encore"
            );
        };

        assert_eq!(
            nommee, demandee,
            "le refus répète le nom reçu, mot pour mot : c'est ce qu'on relit pour voir sa faute \
             de frappe"
        );

        // ⚠️ **Toutes** les sources, pas seulement celles qui se taisent. L'utilisateur qui se
        // trompe de nom ne sait pas encore laquelle est en cause ; lui montrer la seule muette
        // supposerait qu'il ait déjà lu le journal, et si c'était le cas il n'aurait pas fait de
        // faute de frappe.
        for attendue in [KRAKEN, SMART2, CPU] {
            assert!(
                connues.iter().any(|c| c == attendue),
                "« {attendue} » manque à la liste rendue par le refus de « repare {demandee} » : \
                 {connues:?}"
            );
        }
        assert_eq!(
            connues.len(),
            sources.len(),
            "la liste doit porter chaque source une fois et une seule : {connues:?}"
        );

        // Et le message rendu à l'utilisateur porte vraiment cette liste : la ranger dans une
        // variante sans jamais l'écrire ne servirait personne.
        let message = refus(demandee, &sources).to_string();
        for attendue in [KRAKEN, SMART2, CPU] {
            assert!(
                message.contains(attendue),
                "le message du refus doit lister « {attendue} » : « {message} »"
            );
        }
    }
}

#[test]
fn repare_sans_aucune_source_relevee_est_refuse_sans_paniquer() {
    // Le cas limite du précédent, et il arrive : le démon peut servir le socket avant d'avoir
    // relevé quoi que ce soit, ou tourner sur une machine dont aucun `hwmon` connu n'est présent.
    // Une liste vide est un refus parfaitement légitime — ce qui ne l'est pas, c'est de paniquer,
    // ou de tomber dans la branche « cette source répond encore » faute d'avoir trouvé la première.
    let RefusDeReparation::SourceInconnue { demandee, connues } = refus(KRAKEN, &[]) else {
        panic!("sans aucune source relevée, aucun nom n'en désigne une");
    };
    assert_eq!(demandee, KRAKEN);
    assert!(
        connues.is_empty(),
        "aucune source relevée, donc aucune à lister : {connues:?}"
    );
}

// ---------------------------------------------------------------------------
// 7 — `repare` sur une source partiellement vivante est refusé
// ---------------------------------------------------------------------------

#[test]
fn repare_sur_une_source_partiellement_vivante_est_refuse() {
    // issue #136, critère d'acceptation — « `repare` sur une source dont au moins une cible répond
    // encore est **refusé** — la garde de #98 (« une seule cible muette ne déclenche rien ») vaut
    // aussi pour le geste manuel. »
    //
    // C'est la moitié qui protège la machine, et c'est elle qu'on perdrait en croyant que « manuel »
    // veut dire « sous la responsabilité de celui qui tape ». Un `USBDEVFS_RESET` fait disparaître
    // puis réapparaître le périphérique — et le 2026-08-16, il n'est pas réapparu du tout. Le
    // déclencher sur un contrôleur qui répond encore, c'est casser ce qui marchait, à la main.
    //
    // Tous les silences partiels sont essayés, y compris le silence total... de rien du tout.
    for muettes in [
        vec![],
        vec![FAN],
        vec![POMPE],
        vec![LIQUIDE],
        vec![FAN, POMPE],
        vec![FAN, LIQUIDE],
        vec![POMPE, LIQUIDE],
    ] {
        let sources = vec![
            etat(KRAKEN, &CIBLES_KRAKEN, &muettes),
            intacte(SMART2, &CIBLES_SMART2),
        ];

        let RefusDeReparation::SourceRepond { source, vivantes } = refus(KRAKEN, &sources) else {
            panic!(
                "« {KRAKEN} » répond encore par {} cible(s) sur {} : le refus doit le dire, pas \
                 prétendre que la source est inconnue",
                CIBLES_KRAKEN.len() - muettes.len(),
                CIBLES_KRAKEN.len()
            );
        };
        assert_eq!(source, KRAKEN, "le refus nomme la source visée");

        // Il nomme aussi **ce qui répond**, et c'est le seul renseignement qui vaille : « la source
        // répond encore » invite à réessayer plus tard, « la pompe répond encore » dit ce qu'on
        // aurait cassé.
        let attendues: Vec<&str> = CIBLES_KRAKEN
            .iter()
            .copied()
            .filter(|c| !muettes.contains(c))
            .collect();
        for cible in &attendues {
            assert!(
                vivantes.iter().any(|v| v == cible),
                "« {cible} » répond encore et doit figurer parmi les vivantes : {vivantes:?}"
            );
        }
        assert_eq!(
            vivantes.len(),
            attendues.len(),
            "seules les cibles qui répondent sont vivantes : {vivantes:?} contre {attendues:?}"
        );
    }
}

#[test]
fn repare_sur_une_source_sans_cible_est_refuse() {
    // « Toutes ses cibles se taisent » est vrai à vide — le même piège que pour le signalement,
    // pris par la porte du geste. Une source dont on n'a jamais rien lu n'a montré aucun symptôme,
    // et un `repare` dessus réinitialiserait un périphérique sur la foi d'une découverte ratée.
    //
    // C'est bien un refus et non une acceptation silencieuse : la question posée est « cette source
    // est-elle entièrement muette », et une source sans cible n'y répond pas oui.
    //
    // ⚠️ **Laquelle des deux variantes le porte n'est pas figée ici**, et c'est délibéré : la
    // source figure bien dans la liste (donc « inconnue » ment), mais aucune de ses cibles ne
    // répond (donc « répond encore » ment aussi). Aucune des deux formulations que l'issue nomme ne
    // décrit ce cas, et en imposer une figerait un message que personne n'a choisi. Ce qui est
    // exigé, c'est le refus — et qu'il dise quelque chose, ce dont [`refus`] se charge.
    let sources = vec![effondree(KRAKEN, &[]), intacte(SMART2, &CIBLES_SMART2)];
    let refuse = refus(KRAKEN, &sources);
    let message = refuse.to_string();
    assert!(
        message.contains(KRAKEN),
        "le refus doit nommer la source visée : « {message} » ({refuse:?})"
    );
}

// ---------------------------------------------------------------------------
// 8 — une demande acceptée rend l'état à déposer, et rien d'autre
// ---------------------------------------------------------------------------

#[test]
fn repare_sur_une_source_entierement_muette_rend_l_etat_a_deposer() {
    // issue #136 — « Un verbe `Repare { source }` rejoint `reverb-proto/src/ipc.rs`, et c'est lui
    // qui dépose. »
    //
    // La demande acceptée rend l'`EtatSource` **tel qu'il a été relevé** : c'est exactement ce que
    // `FilReparation::soumettre` prend, et le reconstruire côté appelant serait le rebâtir depuis
    // des noms — soit la seule façon de déposer un état qui ne corresponde plus à ce qu'on a
    // constaté.
    let sources = machine_en_panne();
    let depose = demande_de_reparation(KRAKEN, &sources)
        .expect("« kraken2023elite » est entièrement muette : la demande est recevable");

    assert_eq!(
        depose,
        effondree(KRAKEN, &CIBLES_KRAKEN),
        "l'état déposé est celui qui a été relevé, sans retouche"
    );

    // Les deux autres sources, elles, ne sont pas déposables : elles répondent.
    for vivante in [SMART2, CPU] {
        assert!(
            demande_de_reparation(vivante, &sources).is_err(),
            "« {vivante} » répond : sa réparation doit être refusée"
        );
    }
}

#[test]
fn l_etat_rendu_par_une_demande_acceptee_alimente_la_mecanique_bornee_de_98() {
    // issue #136, critère d'acceptation — « `repare <source>` sur le socket lance les trois
    // tentatives bornées ». Et : « Le geste lui-même ne change pas : trois tentatives bornées,
    // espacées de trente secondes, sur le fil de réparation. »
    //
    // Les tentatives, leur espacement et l'abandon sont figés par `spec_reparation_source.rs`
    // (#98), et ce fichier n'a aucune raison de les vérifier une seconde fois. Ce qu'il vérifie est
    // la **jonction** — la seule chose que #136 ajoute : l'état rendu par une demande acceptée est
    // celui que cette mécanique-là sait juger, et il déclenche bien un geste.
    //
    // Sans ce test, les deux moitiés pourraient être justes séparément et ne se rencontrer nulle
    // part : `repare` accepterait, déposerait un état que `Reparations::tour` jugerait vivant, et
    // le geste n'aurait jamais lieu — un verbe qui répond « d'accord » et ne fait rien.
    let sources = machine_en_panne();
    let depose = demande_de_reparation(KRAKEN, &sources).expect("la demande est recevable");

    let mut reparations = Reparations::nouvelles();
    let gestes = RefCell::new(Vec::new());

    let constat = reparations.tour(&depose, DEBUT, || {
        gestes.borrow_mut().push(depose.source.clone());
        Err(std::io::Error::other(RAISON_DU_RESET))
    });

    assert_eq!(
        gestes.borrow().clone(),
        vec![KRAKEN.to_owned()],
        "l'état déposé par une demande acceptée doit déclencher le geste, sur cette source-là"
    );
    assert!(
        matches!(constat, Constat::Echouee { tentative: 1, .. }),
        "c'est la première tentative de l'épisode, et l'`ioctl` a échoué : {constat:?}"
    );

    // Et la série reste bornée : le plafond de #98 s'applique au geste demandé comme il
    // s'appliquait au geste automatique. « Trois tentatives » est la promesse de l'issue, pas une
    // conséquence du déclencheur.
    let plafond = TENTATIVES_MAXIMALES;
    for tentative in 2..=plafond {
        reparations.tour(
            &depose,
            DEBUT + DELAI_ENTRE_TENTATIVES * (tentative - 1),
            || {
                gestes.borrow_mut().push(depose.source.clone());
                Err(std::io::Error::other(RAISON_DU_RESET))
            },
        );
    }
    assert_eq!(
        gestes.borrow().len(),
        usize::try_from(plafond).unwrap(),
        "la série demandée reste bornée par {plafond}"
    );

    let apres = reparations.tour(&depose, DEBUT + DELAI_ENTRE_TENTATIVES * plafond, || {
        gestes.borrow_mut().push(depose.source.clone());
        Ok(())
    });
    assert_eq!(
        apres,
        Constat::Abandon,
        "au-delà du plafond, le démon renonce et le dit une fois — même demandé à la main"
    );
    assert_eq!(
        gestes.borrow().len(),
        usize::try_from(plafond).unwrap(),
        "l'abandon ne réinitialise rien de plus"
    );
}

// ---------------------------------------------------------------------------
// 9 — la quarantaine ne change pas
// ---------------------------------------------------------------------------

#[test]
fn la_quarantaine_ses_retentes_et_leur_doublement_ne_changent_pas() {
    // issue #136, critère d'acceptation — « La quarantaine, ses retentes et leur doublement ne
    // changent pas », et hors scope — « Toute modification de la quarantaine ».
    //
    // C'est la moitié **défensive** du dispositif, celle qui empêche une lecture muette de geler le
    // fil qui sert le socket (#68, #88). Elle n'a jamais réinitialisé quoi que ce soit, et rien de
    // ce que #136 retire ne la concerne. Ce test la reprend au minimum — entrée, silence, retente
    // due après le délai initial — pour que le désarmement du reset ne l'emporte pas par
    // inadvertance.
    //
    // Sa politique complète — doublement, plafond de cinq minutes — a ses propres fichiers, et
    // ce fichier ne la duplique pas.
    let mut quarantaine = Quarantaine::nouvelle();
    let lectures = RefCell::new(Vec::new());

    assert_eq!(
        releve(&mut quarantaine, &lectures, LIQUIDE, DEBUT, None),
        Releve::Muette { signaler: true },
        "une cible qui échoue entre en quarantaine, et ça se journalise — comme avant #136"
    );

    lectures.borrow_mut().clear();
    assert_eq!(
        releve(
            &mut quarantaine,
            &lectures,
            LIQUIDE,
            DEBUT + DELAI_INITIAL - Duration::from_nanos(1),
            Some(PIEGE)
        ),
        Releve::Muette { signaler: false },
        "avant l'échéance, elle n'est pas relevée"
    );
    assert!(
        lectures.borrow().is_empty(),
        "une cible en quarantaine ne doit pas être lue : {:?}",
        lectures.borrow()
    );

    assert_eq!(
        releve(
            &mut quarantaine,
            &lectures,
            LIQUIDE,
            DEBUT + DELAI_INITIAL,
            Some(TEMPERATURE_LIQUIDE)
        ),
        Releve::Valeur(TEMPERATURE_LIQUIDE),
        "à l'échéance, la retente a lieu, et une cible revenue est rendue à son service"
    );
}

#[test]
fn une_source_reparee_a_la_main_oublie_ses_quarantaines() {
    // issue #136, test d'intention — « une source réparée oublie ses quarantaines ».
    //
    // C'est le critère de #98 (« Après une réparation réussie, les quarantaines de cette source
    // sont remises à zéro »), et il ne bouge pas : ce que #136 change est **qui** déclenche, jamais
    // ce qui se passe après. Ce test le reprend sur le chemin manuel, parce que c'est désormais le
    // seul — un oubli qui ne serait câblé que sur le déclencheur automatique disparaîtrait avec
    // lui, sans une erreur.
    //
    // Sans cet oubli, une réparation demandée à la main ne servirait à rien pendant cinq minutes :
    // les cibles porteraient encore le délai accumulé avant la panne, et le démon attendrait tout
    // ce temps avant de découvrir que la source est revenue.
    let mut quarantaine = Quarantaine::nouvelle();
    let lectures = RefCell::new(Vec::new());

    for cible in CIBLES_KRAKEN.iter().chain(std::iter::once(&VENTILO_1)) {
        assert_eq!(
            releve(&mut quarantaine, &lectures, cible, DEBUT, None),
            Releve::Muette { signaler: true }
        );
    }

    // La demande est acceptée, le geste réussit : le démon libère les cibles **de cette source**,
    // qu'il a toutes dans l'`EtatSource` que la demande lui a rendu.
    let sources = machine_en_panne();
    let depose = demande_de_reparation(KRAKEN, &sources).expect("la demande est recevable");
    for cible in &depose.cibles {
        quarantaine.oublie(cible);
    }

    lectures.borrow_mut().clear();
    let apres = DEBUT + CYCLE;
    for cible in CIBLES_KRAKEN {
        assert_eq!(
            releve(
                &mut quarantaine,
                &lectures,
                cible,
                apres,
                Some(TEMPERATURE_LIQUIDE)
            ),
            Releve::Valeur(TEMPERATURE_LIQUIDE),
            "« {cible} » a été libérée : elle est relevée sans délai, comme une cible jamais vue"
        );
    }
    assert_eq!(
        lectures.borrow().clone(),
        CIBLES_KRAKEN.map(|c| c.to_owned()).to_vec(),
        "les trois cibles du Kraken, et elles seules, devaient être relevées"
    );

    lectures.borrow_mut().clear();
    assert_eq!(
        releve(&mut quarantaine, &lectures, VENTILO_1, apres, Some(PIEGE)),
        Releve::Muette { signaler: false },
        "« {VENTILO_1} » n'appartient pas à la source réparée : sa quarantaine est intacte"
    );
    assert!(lectures.borrow().is_empty());
}

// ---------------------------------------------------------------------------
// 10 — le refus est un calcul, pas une relecture
// ---------------------------------------------------------------------------

#[test]
fn le_refus_est_un_calcul_et_la_veille_ne_connait_aucun_chemin() {
    // Deux propriétés structurelles, qu'aucune égalité de valeur ne peut porter.
    //
    // — **`demande_de_reparation` ne reçoit ni descripteur, ni canal ouvert, ni chemin.** « Rien
    //   n'est écrit » devient une propriété de sa signature, exactement comme `refus_de_consigne`
    //   (#101). C'est la règle du projet — ce qui est testable sans matériel est séparé de ce qui y
    //   touche — appliquée à un garde-fou dont le franchissement fait quitter le bus à un
    //   périphérique.
    // — **`Veille` et `Alerte` voyagent entre fils.** La réparation vit hors du fil qui sert le
    //   socket depuis #98 ; la veille, elle, tourne dans le fil principal et son verdict doit
    //   pouvoir en sortir. Une décision qui ne serait pas `Send` ne le pourrait pas. C'est une
    //   borne de compilation, pas une promesse.
    fn exige_send<T: Send>() {}
    exige_send::<Veille>();
    exige_send::<Alerte>();
    exige_send::<RefusDeReparation>();

    // Et la couture ne transporte que des noms. Un chemin qui s'y serait glissé — sous n'importe
    // quel prétexte, « pour éviter de re-résoudre le nœud usbfs » en tête — se lirait ici. Les
    // numéros `hwmonN` changent au redémarrage, et depuis #98 à chaque reset : c'est justement ce
    // qui rend un chemin conservé faux.
    let sources = machine_en_panne();
    let depose = demande_de_reparation(KRAKEN, &sources).expect("la demande est recevable");
    assert!(
        !depose.source.contains('/') && !depose.source.contains(char::is_whitespace),
        "« {} » n'est pas un nom de source",
        depose.source
    );
    for cible in &depose.cibles {
        assert!(
            !cible.contains('/'),
            "« {cible} » ressemble à un chemin : les cibles se nomment, elles ne se situent pas"
        );
    }

    let RefusDeReparation::SourceInconnue { connues, .. } = refus(INCONNUE, &sources) else {
        panic!("« {INCONNUE} » ne désigne aucune source");
    };
    for connue in &connues {
        assert!(
            !connue.contains('/'),
            "la liste des sources connues porte un chemin : « {connue} »"
        );
    }
}
