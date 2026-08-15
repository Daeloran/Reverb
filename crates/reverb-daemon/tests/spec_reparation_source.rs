//! Tests d'intention de la **réparation d'une source muette** (issue #98).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #98 seule. Aucun corps de fonction de
//! `crates/*/src/` n'a été ouvert pour les produire ; seuls l'ont été les fichiers de tests
//! d'intention de #68 (`spec_quarantaine.rs`), de #88 (`spec_canaux_muets.rs`) et de #83
//! (`spec_fil_ecran.rs`), plus les signatures publiques de `reverb_daemon::quarantaine`,
//! `reverb_daemon::ecran` et `reverb_hw::hwmon`. À l'écriture de ce fichier, le module
//! `reparation` **n'existe pas** et `Quarantaine::oublie` non plus : la compilation doit échouer,
//! et c'est la phase rouge.
//!
//! Rien ici n'ouvre de périphérique, ne réinitialise quoi que ce soit, ni ne dort. Le temps est
//! **injecté**, comme dans #68 et #88 : chaque tour reçoit son instant. Un test qui appellerait
//! `Instant::now()` mesurerait l'ordonnanceur de la machine, et vérifier trois tentatives espacées
//! d'une minute en attendant trois minutes n'est pas une option. **Aucun `sleep`, nulle part.**
//!
//! ⚠️ **Ce fichier teste la décision, jamais le geste.** Un test qui exigerait un vrai
//! `USBDEVFS_RESET` serait un test d'intégration : il faudrait un Kraken, il réinitialiserait
//! vraiment un périphérique de la machine, et il ne dirait rien de ce qui compte ici — *quand* on
//! répare, *combien de fois*, et *quand on renonce*.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Relevé sur SHYNAEL le 2026-08-15 :
//!
//! ```text
//! 12:29:49  écran : pas de trame 37 02 en 2s — le contrôleur ne répond plus
//! 12:29:56  canal « kraken2023elite:fan-speed » écarté : Connection timed out
//! 12:29:56  canal « kraken2023elite:pump-speed » écarté : Connection timed out
//! 12:30:01  sonde « kraken2023elite:coolant-temp » écartée : Connection timed out
//! 12:30:41  3 échecs d'affilée sur la dalle → écran rendu au firmware
//! ```
//!
//! Le firmware du Kraken cesse de répondre **en gardant son lien USB** : le périphérique reste
//! énuméré, le journal noyau ne dit rien, et la lecture échoue hors de Reverb, dans un simple
//! shell. Tout ce que le contrôleur porte tombe d'un bloc — la dalle, les deux canaux de vitesse,
//! la sonde de liquide.
//!
//! La quarantaine de #68 et #88 fait exactement son travail : elle empêche une lecture muette de
//! geler le fil qui sert le socket. Mais elle est **purement défensive** — elle attend un
//! rétablissement qui, sur les deux incidents relevés, n'est jamais venu avant un redémarrage.
//!
//! ⚠️ **La cause reste inconnue, et cette issue traite la conséquence.** Ce fichier ne dit donc
//! rien de ce qui plante le Kraken ; il dit ce que le démon a le droit de tenter, et surtout ce
//! qu'il n'a pas le droit de tenter.
//!
//! ## La couture que ces tests exigent, et pourquoi celle-là
//!
//! L'issue pose la politique — toutes les cibles muettes, un nombre borné de tentatives, un
//! abandon dit une fois, aucun chevauchement — mais pas sa forme. Ce fichier la tranche, et voici
//! ce qu'il faut implémenter :
//!
//! ```ignore
//! // crates/reverb-daemon/src/reparation.rs
//! // et `pub mod reparation;` dans crates/reverb-daemon/src/lib.rs
//!
//! use std::io;
//! use std::time::Duration;
//!
//! /// Le nombre de tentatives pour un même effondrement.
//! pub const TENTATIVES_MAXIMALES: u32 = /* … */;
//!
//! /// Le délai qui sépare deux tentatives.
//! pub const DELAI_ENTRE_TENTATIVES: Duration = /* … */;
//!
//! /// Ce qu'une source `hwmon` porte, et ce qui se tait, à l'instant du tour.
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub struct EtatSource {
//!     /// Le nom de la source, tel que `hwmon` le donne — « kraken2023elite ».
//!     /// **Jamais un chemin** : les numéros `hwmonN` changent, les noms non.
//!     pub source: String,
//!     /// Toutes ses cibles connues, canaux et sondes confondus, par leur nom de protocole.
//!     pub cibles: Vec<String>,
//!     /// Celles qui sont en quarantaine à cet instant.
//!     pub muettes: Vec<String>,
//! }
//!
//! /// Ce qu'un tour de décision a produit pour une source.
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub enum Constat {
//!     /// La source répond, au moins en partie : rien à faire, et tout compteur est remis à zéro.
//!     Rien,
//!     /// La source est entièrement muette, mais la prochaine tentative n'est pas due.
//!     Patiente,
//!     /// Une tentative vient d'avoir lieu et le geste a réussi. `tentative` est son rang, à
//!     /// partir de 1. Les cibles de la source sont à **redécouvrir par leur nom**, et leurs
//!     /// quarantaines à libérer.
//!     ///
//!     /// ⚠️ « Réussie » qualifie l'`ioctl`, pas la guérison : un reset peut rendre `Ok` sans
//!     /// que le périphérique revienne.
//!     Reussie { tentative: u32 },
//!     /// Une tentative vient d'avoir lieu et le geste a échoué. **Rien n'a changé**,
//!     /// quarantaines comprises.
//!     Echouee { tentative: u32, erreur: String },
//!     /// Le plafond est atteint et la source est toujours muette : plus aucune tentative.
//!     /// Rendu **une seule fois** par épisode, pour que l'appelant journalise une ligne sans
//!     /// avoir à s'en souvenir.
//!     Abandon,
//!     /// L'abandon est déjà prononcé : rien n'a été tenté, rien n'est à dire.
//!     Repos,
//! }
//!
//! pub struct Reparations { /* un état par source */ }
//!
//! impl Reparations {
//!     pub fn nouvelles() -> Reparations;
//!
//!     /// Un tour de décision, pour **une** source.
//!     ///
//!     /// `reinitialiser` n'est appelée que si une tentative est due — au plus une fois par
//!     /// tour, jamais autrement.
//!     pub fn tour(
//!         &mut self,
//!         etat: &EtatSource,
//!         maintenant: Duration,
//!         reinitialiser: impl FnOnce() -> io::Result<()>,
//!     ) -> Constat;
//! }
//! ```
//!
//! ```ignore
//! // crates/reverb-daemon/src/quarantaine.rs — un seul ajout, la politique de #68 est intacte
//!
//! impl Quarantaine {
//!     /// Oublie tout ce qu'elle sait de cette cible : au prochain tour elle sera relevée sans
//!     /// délai, comme une cible jamais vue.
//!     pub fn oublie(&mut self, cible: &str);
//! }
//! ```
//!
//! Cinq choix, et ce qu'ils achètent :
//!
//! 1. **Le geste arrive en fermeture, il ne suit pas la décision.** C'est la leçon de #68 et de
//!    #88, et elle pèse bien plus lourd ici : un `faut_il_reparer(source) -> bool` laisserait « ne
//!    pas réinitialiser » à la politesse de l'appelant, et un `USBDEVFS_RESET` est un geste
//!    **visible sur la machine** — il fait disparaître puis réapparaître un périphérique. En
//!    prenant la fermeture, `tour` **est** l'endroit où le reset a lieu ou n'a pas lieu, et un
//!    test qui compte les appels le vérifie. Comparer des `Constat` ne le vérifierait pas : une
//!    implémentation qui réinitialiserait puis rendrait `Patiente` rendrait la bonne valeur en
//!    secouant le Kraken toutes les secondes.
//! 2. **Une source par tour, comme une sonde par tour dans #68.** L'issue met hors scope « réparer
//!    autre chose que le Kraken », mais demande un mécanisme qui ne lui soit pas propre : une clef
//!    par source rend l'indépendance structurelle plutôt que disciplinaire, et un test la vérifie.
//! 3. **`io::Result<()>` pour la fermeture**, comme `Vigie::tour` de #70. Le reset est un `ioctl`,
//!    et sa raison d'échec est le seul diagnostic que l'opérateur reçoit — elle traverse donc la
//!    couture et ressort dans `Constat::Echouee`.
//! 4. **`Abandon` est prononcé quand une tentative *serait* due et que le plafond est atteint**,
//!    et non au moment de la dernière tentative. C'est ce qui donne un sens uniforme au verdict :
//!    « j'ai essayé N fois, il s'est écoulé N délais, la source se tait toujours ». Le prononcer
//!    sur le dernier échec obligerait à inventer un verdict pour le cas — bien réel — où le
//!    dernier `ioctl` rend `Ok` sans que le périphérique revienne.
//! 5. **`Quarantaine::oublie` prend une cible, pas une source.** Il n'y a aucune raison de faire
//!    dépendre la libération d'un découpage du nom sur `:` — l'appelant a déjà la liste des cibles
//!    de la source dans `EtatSource`. Et c'est ce qui rend vérifiable « de **cette** source, et
//!    d'aucune autre » sans dicter comment un nom se lit.
//!
//! ## Ce que ce fichier fige
//!
//! 1. **Une source entièrement muette déclenche une tentative, sur-le-champ.** Attendre une minute
//!    de plus quand l'effondrement est déjà constaté n'apprend rien à personne.
//! 2. **Une source qui répond, ne serait-ce que par une cible, n'en déclenche aucune.** C'est la
//!    moitié qui protège la machine : un reset USB sur un contrôleur qui répond encore casse ce
//!    qui marchait.
//! 3. **Une source sans cible n'en déclenche aucune non plus.** « Toutes ses cibles se taisent »
//!    est vrai à vide, et cette vérité-là est un défaut, pas un critère.
//! 4. **Le compte de tentatives est borné, et l'abandon se dit une fois**, puis plus rien, jamais
//!    — vérifié sur une heure de tours.
//! 5. **Deux tentatives ne se suivent pas sans le délai**, bornes pincées au nanoseconde près.
//! 6. **Un reset qui échoue ne se dit jamais réussi**, et ne relance rien : c'est ce qui laisse le
//!    démon dans l'état d'avant, quarantaines comprises.
//! 7. **Un reset qui rend `Ok` sans ramener la source ne relance pas le compteur.** Sans cette
//!    règle, le démon secouerait le Kraken indéfiniment en se croyant à chaque fois au premier
//!    essai — la boucle exacte que l'issue interdit, atteinte par le chemin qu'elle ne nomme pas.
//! 8. **C'est la source qui répond à nouveau qui remet le compteur à zéro**, et elle rouvre alors
//!    un épisode entier, abandon compris. Un contrôleur qui clignote est justement celui dont on
//!    veut entendre parler (#68, #88).
//! 9. **La libération des quarantaines est par cible**, et n'atteint aucune autre source.
//! 10. **La décision ne connaît aucun chemin et ne touche aucun périphérique**, et elle voyage
//!     entre fils — c'est ce qui rend possible de la faire vivre hors du fil qui sert le socket,
//!     comme la dalle depuis #83.
//!
//! ## Ce que le contrat laisse ouvert, et que ces tests tranchent
//!
//! 1. **La première tentative a lieu au tour même de l'effondrement**, sans délai. Le délai sépare
//!    deux tentatives ; il n'est pas un droit d'entrée.
//! 2. **Le délai court depuis la dernière tentative**, pas depuis l'effondrement. Sur une série
//!    régulière les deux coïncident ; ils divergent dès qu'un tour est sauté, et c'est la première
//!    lecture qui a un sens.
//! 3. **La tentative est due à l'instant exact de l'échéance, pas après** : la comparaison est un
//!    `>=`, et les bornes sont pincées au nanoseconde près de part et d'autre. C'est la seule
//!    façon d'attraper les deux fautes symétriques — celle qui ne retente jamais, et celle qui
//!    retente tout de suite.
//! 4. **`TENTATIVES_MAXIMALES` et `DELAI_ENTRE_TENTATIVES` sont encadrés, pas pinnés.** L'issue
//!    écrit « au plus N essais, espacés » sans donner N : ce sont des réglages, et le test n° 0
//!    n'en exige que ce que l'arithmétique de l'incident impose.
//! 5. **Un `EtatSource` dont `muettes` nommerait une cible absente de `cibles` n'est pas
//!    spécifié.** C'est une entrée mal formée ; inventer un comportement figerait une règle que
//!    personne n'a choisie. Les fabriques de ce fichier ne produisent jamais un tel état, et le
//!    test n° 0 le vérifie.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Le `USBDEVFS_RESET` lui-même**, et la résolution du nœud usbfs par VID:PID + série. C'est
//!   le **geste**, il vit derrière la fermeture, dans `reverb-hw` — et l'éprouver demanderait de
//!   réinitialiser un vrai périphérique. Ce que ce fichier peut garantir, et garantit, c'est que
//!   **aucun chemin ne traverse la couture** : la décision ne nomme qu'une source.
//! - **La stabilité des noms sous renumérotation `hwmonN`.** Elle est déjà figée par
//!   `spec_sondes.rs` (#31, `le_slug_ne_depend_pas_du_numero_du_repertoire_hwmon`) et par
//!   `spec_fans.rs` (#7, `l_ordre_ne_depend_pas_du_numero_des_repertoires_hwmon`). Ce fichier
//!   n'en vérifie que le corollaire propre à #98, et que personne n'avait eu de raison d'écrire :
//!   les **chemins**, eux, changent, donc garder les anciennes poignées après un reset fait lire
//!   le mauvais `hwmon`.
//! - **Le fil dédié.** #83 l'a fait pour la dalle, avec sa propre couture (`fil_ecran`) et ses
//!   propres tests. Ce que ce fichier en retient est ce qu'une décision pure peut porter : elle ne
//!   bloque sur rien, et elle est `Send`.
//! - **La politique de la quarantaine elle-même** — délais, doublement, plafond. L'issue la met
//!   hors scope, et elle a ses deux fichiers.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reverb_daemon::quarantaine::{DELAI_INITIAL, DELAI_MAXIMAL, Quarantaine, Releve};
use reverb_daemon::reparation::{
    Constat, DELAI_ENTRE_TENTATIVES, EtatSource, Reparations, TENTATIVES_MAXIMALES,
};
use reverb_hw::hwmon;

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// L'origine du temps injecté.
///
/// Volontairement loin de zéro : le démon tourne depuis des heures quand le Kraken lâche — les
/// deux incidents relevés sont survenus à 23:22 et 12:29. Une décision qui prendrait
/// `Duration::ZERO` pour « cette source n'a jamais été réparée » se ferait attraper ici plutôt
/// qu'à l'usage.
const DEBUT: Duration = Duration::from_secs(3_600);

/// Le plus petit écart que le temps injecté sache représenter.
const INSTANT: Duration = Duration::from_nanos(1);

/// La cadence des tours du démon : la fenêtre demande `status` une fois par seconde (#88).
const CYCLE: Duration = Duration::from_secs(1);

/// Ce que coûte **une** lecture qui bute sur un contrôleur muet : cinq secondes en sommeil non
/// interruptible. Mesuré dans #68 — « 5,218 total » pour un `cat` sur `temp1_input` du Kraken —,
/// et c'est exactement le symptôme que #98 relève à nouveau, hors de Reverb, dans un shell.
const COUT_D_UNE_LECTURE_MUETTE: Duration = Duration::from_millis(5_218);

/// La durée de l'effondrement relevé le 2026-08-15 : de la première alerte de l'écran (12:29:49)
/// au renoncement de la dalle (12:30:41).
///
/// C'est le temps qu'il faut au démon pour **constater** la panne. Renoncer à réparer plus vite
/// que la panne ne met à se déclarer serait hâtif.
const DUREE_DE_L_EFFONDREMENT: Duration = Duration::from_secs(52);

/// Le budget d'un `status`, critère d'acceptation de #88 et promesse que #98 ne doit pas défaire.
const BUDGET_STATUS: Duration = Duration::from_millis(100);

/// La source qui lâche : le Kraken, nommé comme son pilote `hwmon`.
const KRAKEN: &str = "kraken2023elite";

/// Ses trois cibles, recopiées telles quelles des lignes du journal de l'issue.
const FAN: &str = "kraken2023elite:fan-speed";
const POMPE: &str = "kraken2023elite:pump-speed";
const LIQUIDE: &str = "kraken2023elite:coolant-temp";
const CIBLES_KRAKEN: [&str; 3] = [FAN, POMPE, LIQUIDE];

/// Une seconde source, qui elle répond : le contrôleur de ventilation NZXT.
const SMART2: &str = "nzxtsmart2";
const VENTILO_1: &str = "nzxtsmart2:fan-1";
const VENTILO_2: &str = "nzxtsmart2:fan-2";
const CIBLES_SMART2: [&str; 2] = [VENTILO_1, VENTILO_2];

/// La raison rendue par le contrôleur en rade, recopiée telle quelle de l'issue.
const RAISON_DE_LA_PANNE: &str = "Connection timed out (os error 110)";

/// Ce que rend un `USBDEVFS_RESET` refusé.
const RAISON_DU_RESET: &str = "USBDEVFS_RESET: No such device (os error 19)";

/// Une seconde raison de refus, distincte : deux échecs ne partagent pas la leur.
const AUTRE_RAISON_DU_RESET: &str = "USBDEVFS_RESET: Device or resource busy (os error 16)";

/// Ce qu'une sonde du Kraken rend quand elle va bien : 34,2 °C, en millidegrés comme le veut
/// `hwmon`.
const TEMPERATURE_LIQUIDE: i32 = 34_200;

/// Ce que rend la sonde du CPU, distincte de toutes les autres.
const TEMPERATURE_CPU: i32 = 52_875;

/// La valeur qu'une cible en quarantaine rendrait **si** on la relevait.
///
/// C'est un piège, et il attrape deux fautes d'un coup : une implémentation qui lit quand même se
/// voit au compte d'appels, **et** au verdict si elle rend ce qu'elle vient de lire.
const PIEGE: i32 = -999_999;

// ---------------------------------------------------------------------------
// Le banc : des sources, et un reset qu'on n'exécute jamais
// ---------------------------------------------------------------------------

/// L'état d'une source, avec les cibles muettes qu'on lui désigne.
///
/// ⚠️ `muettes` est toujours un sous-ensemble de `cibles` — le test n° 0 le vérifie. Un état mal
/// formé n'est pas spécifié par l'issue, et ce fichier n'en fabrique jamais.
fn etat(source: &str, cibles: &[&str], muettes: &[&str]) -> EtatSource {
    EtatSource {
        source: source.to_owned(),
        cibles: cibles.iter().map(|c| (*c).to_owned()).collect(),
        muettes: muettes.iter().map(|c| (*c).to_owned()).collect(),
    }
}

/// Une source dont **toutes** les cibles se taisent — l'effondrement de l'issue.
fn effondree(source: &str, cibles: &[&str]) -> EtatSource {
    etat(source, cibles, cibles)
}

/// Une source qui va bien : aucune cible muette.
fn intacte(source: &str, cibles: &[&str]) -> EtatSource {
    etat(source, cibles, &[])
}

/// Ce que la réinitialisation rendrait, et les sources qu'on a vraiment tenté de réinitialiser.
///
/// Compter les gestes et non seulement comparer les verdicts : une implémentation qui
/// réinitialiserait le Kraken à chaque tour puis rendrait `Patiente` rendrait exactement la bonne
/// valeur, et secouerait un périphérique en difficulté une fois par seconde. Un `USBDEVFS_RESET`
/// n'est pas une lecture — il fait disparaître puis réapparaître le périphérique sur le bus.
struct Banc {
    /// Par source : `None` si le reset réussit, `Some(raison)` s'il échoue.
    ///
    /// `io::Error` n'est pas `Clone` : la raison est donc gardée en texte et l'erreur fabriquée au
    /// moment de l'appel.
    verdicts: RefCell<HashMap<String, Option<String>>>,
    /// Les sources réellement réinitialisées, dans l'ordre.
    gestes: RefCell<Vec<String>>,
}

impl Banc {
    /// Un banc où tout reset réussit.
    fn neuf() -> Banc {
        Banc {
            verdicts: RefCell::new(HashMap::new()),
            gestes: RefCell::new(Vec::new()),
        }
    }

    fn reussit(&self, source: &str) {
        self.verdicts.borrow_mut().insert(source.to_owned(), None);
    }

    fn echoue(&self, source: &str, raison: &str) {
        self.verdicts
            .borrow_mut()
            .insert(source.to_owned(), Some(raison.to_owned()));
    }

    fn reinitialiser(&self, source: &str) -> io::Result<()> {
        self.gestes.borrow_mut().push(source.to_owned());
        match self.verdicts.borrow().get(source) {
            None | Some(None) => Ok(()),
            Some(Some(raison)) => Err(io::Error::other(raison.clone())),
        }
    }

    /// Rend les resets faits depuis la dernière vidange, dans l'ordre, et repart à vide.
    fn gestes(&self) -> Vec<String> {
        self.gestes.borrow_mut().drain(..).collect()
    }
}

/// Un tour de décision pour une source, à l'instant donné.
fn tour(
    reparations: &mut Reparations,
    banc: &Banc,
    etat: &EtatSource,
    maintenant: Duration,
) -> Constat {
    reparations.tour(etat, maintenant, || banc.reinitialiser(&etat.source))
}

// ---------------------------------------------------------------------------
// Aides d'assertion
// ---------------------------------------------------------------------------

/// Exige qu'aucun périphérique n'ait été réinitialisé pendant le tour qui vient.
///
/// Vide le compteur au passage — l'appelant l'a donc à vide au retour.
fn exige_aucun_geste(banc: &Banc, contexte: &str) {
    let gestes = banc.gestes();
    assert!(
        gestes.is_empty(),
        "{contexte} : aucun reset ne devait avoir lieu, {gestes:?} en a pourtant reçu un — un \
         USBDEVFS_RESET fait disparaître puis réapparaître le périphérique, ce n'est pas une \
         lecture qu'on refait au cas où"
    );
}

/// Exige qu'une source, et elle seule, ait été réinitialisée exactement une fois.
///
/// Vide le compteur au passage.
fn exige_un_geste(banc: &Banc, source: &str, contexte: &str) {
    let gestes = banc.gestes();
    assert_eq!(
        gestes,
        vec![source.to_owned()],
        "{contexte} : « {source} » devait être réinitialisée une fois et une seule — gestes \
         effectifs : {gestes:?}"
    );
}

/// La forme d'un `Constat`, en un mot.
///
/// ⚠️ **Ce filtrage est exhaustif et sans `..` ni `_` : c'est une assertion à part entière.**
/// Ajouter une variante, ou glisser un `chemin: PathBuf` dans l'une d'elles, casse la compilation
/// de ce fichier — ce qui est précisément le garde-fou du critère « jamais par un chemin codé en
/// dur » que la partie pure peut offrir.
fn forme(constat: &Constat) -> &'static str {
    match constat {
        Constat::Rien => "rien",
        Constat::Patiente => "patiente",
        Constat::Reussie { tentative: _ } => "réussie",
        Constat::Echouee {
            tentative: _,
            erreur: _,
        } => "échouée",
        Constat::Abandon => "abandon",
        Constat::Repos => "repos",
    }
}

/// Le rang de la tentative qu'un constat rapporte, s'il en rapporte une.
fn rang(constat: &Constat) -> Option<u32> {
    match constat {
        Constat::Reussie { tentative } | Constat::Echouee { tentative, .. } => Some(*tentative),
        _ => None,
    }
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

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tous les tests qui suivent supposent que les noms diffèrent, que les deux sources sont
    // vraiment distinctes, que les fabriques d'état sont bien formées, et que le délai entre deux
    // tentatives laisse la place d'observer « pas de reset au tour suivant ». Si l'un de ces
    // repères se dégradait, plusieurs tests deviendraient vrais sans rien vérifier — et personne
    // ne le verrait. Ce test est là pour que la panne soit ici.

    for (i, cible) in CIBLES_KRAKEN.iter().enumerate() {
        assert!(!cible.is_empty(), "une cible doit porter un nom");
        assert!(
            cible.starts_with(&format!("{KRAKEN}:")),
            "« {cible} » doit porter sa source en préfixe, comme les lignes du journal de l'issue"
        );
        for autre in CIBLES_KRAKEN.iter().skip(i + 1) {
            assert_ne!(
                cible, autre,
                "deux cibles du Kraken portent le même nom : les tests d'indépendance ne \
                 testeraient rien"
            );
        }
        for autre in CIBLES_SMART2 {
            assert_ne!(*cible, autre);
        }
    }
    assert_ne!(
        KRAKEN, SMART2,
        "les deux sources doivent différer, sinon « d'aucune autre » ne veut rien dire"
    );

    // ⚠️ Une source se nomme, elle ne se situe pas. Les numéros `hwmonN` changent au redémarrage —
    // et, depuis #98, à chaque reset USB. Un `EtatSource` qui porterait un chemin ferait rentrer
    // par la fenêtre ce que le critère d'acceptation chasse par la porte.
    for source in [KRAKEN, SMART2] {
        assert!(
            !source.contains('/'),
            "« {source} » ressemble à un chemin : une source se désigne par son nom de pilote"
        );
    }

    // Les fabriques d'état sont bien formées : `muettes` est toujours inclus dans `cibles`. Un
    // état mal formé n'est pas spécifié par l'issue, et ce fichier n'en produit jamais.
    for etat in [
        effondree(KRAKEN, &CIBLES_KRAKEN),
        intacte(KRAKEN, &CIBLES_KRAKEN),
        etat(KRAKEN, &CIBLES_KRAKEN, &[LIQUIDE]),
        effondree(SMART2, &CIBLES_SMART2),
    ] {
        for muette in &etat.muettes {
            assert!(
                etat.cibles.contains(muette),
                "« {muette} » est donnée muette mais n'est pas une cible de « {} » : cet état est \
                 mal formé",
                etat.source
            );
        }
    }
    assert_eq!(
        effondree(KRAKEN, &CIBLES_KRAKEN).muettes.len(),
        CIBLES_KRAKEN.len(),
        "une source effondrée a toutes ses cibles muettes, sinon le test n° 1 ne déclencherait rien"
    );
    assert!(
        intacte(KRAKEN, &CIBLES_KRAKEN).muettes.is_empty(),
        "une source intacte n'a aucune cible muette"
    );

    // Les deux raisons de refus diffèrent, et portent des espaces : ce sont des messages d'`ioctl`,
    // et c'est le seul diagnostic que l'opérateur reçoit.
    assert_ne!(RAISON_DU_RESET, AUTRE_RAISON_DU_RESET);
    assert_ne!(RAISON_DU_RESET, RAISON_DE_LA_PANNE);
    assert!(RAISON_DU_RESET.contains(' '));

    // Les valeurs de sonde diffèrent, et aucune ne se confond avec le piège : sans quoi une
    // quarantaine qui lit ce qu'elle ne devrait pas passerait inaperçue.
    assert_ne!(TEMPERATURE_LIQUIDE, TEMPERATURE_CPU);
    assert_ne!(TEMPERATURE_LIQUIDE, PIEGE);
    assert_ne!(TEMPERATURE_CPU, PIEGE);

    // ⚠️ Le nombre de tentatives et le délai sont des **réglages**, que l'issue n'a pas fixés :
    // elle écrit « au plus N essais, espacés ». On les encadre donc, et chaque borne a un motif.
    //
    // Les deux bornes du compte passent par une variable : `TENTATIVES_MAXIMALES` est un `const`,
    // et `clippy::assertions_on_constants` signale une comparaison qu'il sait replier. La borne
    // reste une assertion sur le réglage — ce n'est pas le compilateur qu'on encadre.
    let tentatives = TENTATIVES_MAXIMALES;
    assert!(
        tentatives >= 2,
        "avec {tentatives} tentative(s), « espacées » ne veut rien dire et le délai entre deux \
         n'aurait jamais l'occasion de s'appliquer"
    );
    assert!(
        tentatives <= 10,
        "{tentatives} tentatives, c'est de l'insistance : « insister sur un périphérique en \
         difficulté ne le réveille pas » est la leçon de #70, et un reset USB coûte bien plus cher \
         qu'une trame refusée"
    );
    assert!(
        DELAI_ENTRE_TENTATIVES >= COUT_D_UNE_LECTURE_MUETTE,
        "un délai de {DELAI_ENTRE_TENTATIVES:?} est plus court que les \
         {COUT_D_UNE_LECTURE_MUETTE:?} qu'il faut rien que pour apprendre qu'une cible se tait : ce \
         n'est plus espacer, c'est boucler"
    );
    assert!(
        DELAI_ENTRE_TENTATIVES <= DELAI_MAXIMAL,
        "un délai de {DELAI_ENTRE_TENTATIVES:?} dépasse le plafond de retente de la quarantaine \
         ({DELAI_MAXIMAL:?}) : la réparation serait plus lente que les retentes qu'elle existe pour \
         finir"
    );

    // La série entière doit durer au moins le temps que la panne met à se déclarer — 52 s le
    // 2026-08-15, de la première alerte de l'écran au renoncement de la dalle. Renoncer plus vite
    // que ça, ce serait renoncer avant d'avoir constaté.
    let serie = DELAI_ENTRE_TENTATIVES * TENTATIVES_MAXIMALES;
    assert!(
        serie >= DUREE_DE_L_EFFONDREMENT,
        "la série de {TENTATIVES_MAXIMALES} tentatives dure {serie:?}, moins que les \
         {DUREE_DE_L_EFFONDREMENT:?} que l'effondrement du 2026-08-15 a mis à se déclarer"
    );
    assert!(
        serie <= Duration::from_secs(30 * 60),
        "la série dure {serie:?} : au-delà d'une demi-heure, on n'attend plus, on s'acharne — et \
         sur les deux incidents relevés le Kraken n'est jamais revenu sans redémarrage"
    );

    // Et « pas de reset au tour suivant » doit avoir de quoi s'observer : le tour du démon est
    // d'une seconde.
    assert!(
        CYCLE * 10 <= DELAI_ENTRE_TENTATIVES,
        "un tour toutes les {CYCLE:?} contre un délai de {DELAI_ENTRE_TENTATIVES:?} : la source \
         serait retentée presque à chaque tour, et l'espacement ne servirait à rien"
    );
    assert!(INSTANT < CYCLE);
    assert!(
        BUDGET_STATUS < COUT_D_UNE_LECTURE_MUETTE,
        "le budget de #88 n'aurait aucun sens s'il tenait une lecture muette"
    );

    // Enfin, `forme` doit bien distinguer les six constats : c'est elle qui porte l'assertion
    // structurelle du test n° 11, et deux verdicts qui se ressembleraient la rendraient muette.
    let tous = [
        Constat::Rien,
        Constat::Patiente,
        Constat::Reussie { tentative: 1 },
        Constat::Echouee {
            tentative: 1,
            erreur: RAISON_DU_RESET.to_owned(),
        },
        Constat::Abandon,
        Constat::Repos,
    ];
    for (i, constat) in tous.iter().enumerate() {
        for autre in tous.iter().skip(i + 1) {
            assert_ne!(
                forme(constat),
                forme(autre),
                "{constat:?} et {autre:?} se rendent sous la même forme"
            );
            assert_ne!(constat, autre);
        }
    }
    assert_eq!(rang(&Constat::Reussie { tentative: 3 }), Some(3));
    assert_eq!(rang(&Constat::Patiente), None);
}

// ---------------------------------------------------------------------------
// 1 — une source entièrement muette déclenche une tentative, sur-le-champ
// ---------------------------------------------------------------------------

#[test]
fn une_source_dont_toutes_les_cibles_se_taisent_declenche_une_tentative_sur_le_champ() {
    // Critère d'acceptation : « Une source dont **toutes** les cibles sont en quarantaine
    // déclenche une tentative de réparation ».
    //
    // C'est le comportement que #98 ajoute, et il ne se vérifie pas par une égalité de verdict :
    // ce qui compte est que le **geste** ait lieu. On compte donc les resets.
    //
    // « Sur-le-champ » est l'arbitrage de ce fichier : le délai sépare deux tentatives, il n'est
    // pas un droit d'entrée. Quand les trois cibles du Kraken se taisent, l'effondrement est déjà
    // constaté depuis les 52 s qu'il a mis à se déclarer ; attendre une minute de plus n'apprend
    // rien à personne.
    let mut reparations = Reparations::nouvelles();
    let banc = Banc::neuf();
    banc.reussit(KRAKEN);

    let effondre = effondree(KRAKEN, &CIBLES_KRAKEN);
    let constat = tour(&mut reparations, &banc, &effondre, DEBUT);

    exige_un_geste(&banc, KRAKEN, "le tour de l'effondrement");
    assert_eq!(
        constat,
        Constat::Reussie { tentative: 1 },
        "la première tentative porte le rang 1, et le geste ayant réussi, les cibles sont à \
         redécouvrir et leurs quarantaines à libérer"
    );
}

// ---------------------------------------------------------------------------
// 2 — une seule cible muette ne déclenche rien
// ---------------------------------------------------------------------------

#[test]
fn une_source_dont_une_seule_cible_se_tait_n_en_declenche_aucune() {
    // Critère d'acceptation, seconde moitié : « une source dont une seule cible est muette n'en
    // déclenche pas ».
    //
    // C'est la moitié qui protège la machine, et c'est elle qu'un correctif pressé perdrait. Un
    // `USBDEVFS_RESET` fait disparaître puis réapparaître le périphérique : le déclencher parce
    // qu'un `pwmN_enable` a disparu, ce serait casser deux canaux qui marchaient pour en réparer
    // un qui n'était peut-être pas cassé.
    //
    // Toutes les silences partiels sont essayés, une cible après l'autre, puis deux sur trois —
    // parce qu'une implémentation qui compterait « au moins deux » au lieu de « toutes » passerait
    // le premier cas et échouerait au second.
    let mut reparations = Reparations::nouvelles();
    let banc = Banc::neuf();
    banc.reussit(KRAKEN);

    let partiels = [
        vec![FAN],
        vec![POMPE],
        vec![LIQUIDE],
        vec![FAN, POMPE],
        vec![FAN, LIQUIDE],
        vec![POMPE, LIQUIDE],
        vec![],
    ];

    // Une heure de tours, à raison d'une seconde par tour : bien au-delà de tout délai, pour qu'une
    // implémentation qui déclencherait au bout d'un moment n'ait nulle part où se cacher.
    for i in 0..3_600u32 {
        let muettes = &partiels[(i as usize) % partiels.len()];
        let etat_courant = etat(KRAKEN, &CIBLES_KRAKEN, muettes);
        let constat = tour(&mut reparations, &banc, &etat_courant, DEBUT + CYCLE * i);
        exige_aucun_geste(&banc, &format!("tour n° {i}, muettes : {muettes:?}"));
        assert_eq!(
            constat,
            Constat::Rien,
            "tour n° {i} : « {KRAKEN} » répond encore par {} cible(s) sur {} — il n'y a rien à \
             réparer, et rien à attendre non plus",
            CIBLES_KRAKEN.len() - muettes.len(),
            CIBLES_KRAKEN.len()
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — une source sans cible ne déclenche jamais rien
// ---------------------------------------------------------------------------

#[test]
fn une_source_sans_cible_ne_declenche_jamais_de_reparation() {
    // « Toutes ses cibles sont muettes » est **vrai à vide**, et c'est le piège classique de cette
    // formulation : une source dont on ne connaît aucune cible satisfait le critère sans qu'aucun
    // symptôme n'ait été observé.
    //
    // Le cas n'est pas théorique : une découverte qui échoue, un `hwmon` dépouillé, une source
    // repérée avant que ses cibles ne le soient — et le démon réinitialiserait un périphérique
    // dont il n'a jamais rien lu.
    let mut reparations = Reparations::nouvelles();
    let banc = Banc::neuf();
    banc.reussit(KRAKEN);

    let vide = effondree(KRAKEN, &[]);
    assert!(
        vide.cibles.is_empty() && vide.muettes.is_empty(),
        "le décor de ce test est bien une source sans cible"
    );

    for i in 0..600u32 {
        let constat = tour(&mut reparations, &banc, &vide, DEBUT + CYCLE * i);
        exige_aucun_geste(&banc, &format!("tour n° {i}, source sans cible"));
        assert_eq!(
            constat,
            Constat::Rien,
            "tour n° {i} : une source dont on ne connaît aucune cible n'a montré aucun symptôme — \
             « toutes ses cibles se taisent » y est vrai à vide, et cette vérité-là est un défaut"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — deux tours successifs ne lancent pas deux réparations
// ---------------------------------------------------------------------------

#[test]
fn deux_tours_successifs_ne_lancent_pas_deux_reparations() {
    // Critère d'acceptation : « Deux tentatives ne peuvent pas se chevaucher », et test d'intention
    // de l'issue : « Deux tours successifs ne lancent pas deux réparations concurrentes ».
    //
    // Le démon tourne une fois par seconde. Sans espacement, un effondrement ferait partir un reset
    // USB **par seconde** sur un périphérique déjà en difficulté — ce qui, en plus de ne pas le
    // réveiller (#70), l'empêcherait de finir de se réénumérer entre deux.
    //
    // L'échéance est pincée au nanoseconde près de part et d'autre : c'est la seule façon
    // d'attraper les deux fautes symétriques — celle qui ne retente jamais, et celle qui retente
    // tout de suite.
    let mut reparations = Reparations::nouvelles();
    let banc = Banc::neuf();
    banc.echoue(KRAKEN, RAISON_DU_RESET);

    let effondre = effondree(KRAKEN, &CIBLES_KRAKEN);

    // La première tentative, au tour de l'effondrement.
    let constat = tour(&mut reparations, &banc, &effondre, DEBUT);
    exige_un_geste(&banc, KRAKEN, "le tour de l'effondrement");
    assert_eq!(rang(&constat), Some(1));

    // Puis toute la fenêtre d'attente, seconde par seconde, jusqu'à la veille de l'échéance : pas
    // un geste. C'est ce que « ne se chevauchent pas » veut dire pour une décision qu'on
    // interroge chaque seconde.
    let tours_avant_l_echeance =
        u32::try_from(DELAI_ENTRE_TENTATIVES.as_millis() / CYCLE.as_millis()).unwrap_or(u32::MAX)
            - 1;
    for i in 1..=tours_avant_l_echeance {
        let contexte = format!("tour n° {i} après la première tentative");
        let constat = tour(&mut reparations, &banc, &effondre, DEBUT + CYCLE * i);
        exige_aucun_geste(&banc, &contexte);
        assert_eq!(
            constat,
            Constat::Patiente,
            "{contexte} : la source se tait toujours, mais la prochaine tentative n'est pas due"
        );
    }

    // Un nanoseconde avant l'échéance : toujours rien. Borne par le dessous, elle interdit
    // l'espacement trop court.
    let constat = tour(
        &mut reparations,
        &banc,
        &effondre,
        DEBUT + DELAI_ENTRE_TENTATIVES - INSTANT,
    );
    exige_aucun_geste(&banc, "un instant avant l'échéance");
    assert_eq!(constat, Constat::Patiente);

    // À l'échéance exacte : la seconde tentative part. Borne par le dessus, elle interdit
    // l'espacement qui ne relâche jamais.
    let constat = tour(
        &mut reparations,
        &banc,
        &effondre,
        DEBUT + DELAI_ENTRE_TENTATIVES,
    );
    exige_un_geste(&banc, KRAKEN, "l'échéance de la seconde tentative");
    assert_eq!(
        rang(&constat),
        Some(2),
        "la seconde tentative porte le rang 2, trouvé {constat:?}"
    );
}

// ---------------------------------------------------------------------------
// 5 — le nombre de tentatives est borné, et l'abandon ne se dit qu'une fois
// ---------------------------------------------------------------------------

#[test]
fn le_nombre_de_tentatives_est_borne_et_l_abandon_ne_se_dit_qu_une_fois() {
    // Critère d'acceptation : « Le nombre de tentatives est borné, et l'abandon est journalisé une
    // seule fois ».
    //
    // C'est la leçon de #70, transposée d'une dalle qui refuse à un contrôleur qui se tait :
    // « insister sur un périphérique en difficulté ne le réveille pas ». Ici l'insistance coûte
    // plus cher qu'ailleurs — chaque reset fait disparaître le périphérique du bus.
    //
    // « Une seule fois » est une propriété d'**état**, pas de sortie : si le constat ne disait pas
    // « c'est la première fois », l'appelant n'aurait d'autre choix que de journaliser à chaque
    // tour, soit une ligne par seconde pour toujours, dans un journal qu'on lit justement pour
    // trouver ce genre d'incident. D'où `Abandon` puis `Repos`.
    let mut reparations = Reparations::nouvelles();
    let banc = Banc::neuf();
    banc.echoue(KRAKEN, RAISON_DU_RESET);

    let effondre = effondree(KRAKEN, &CIBLES_KRAKEN);

    // Les N tentatives, à l'échéance exacte de chacune.
    for tentative in 1..=TENTATIVES_MAXIMALES {
        let instant = DEBUT + DELAI_ENTRE_TENTATIVES * (tentative - 1);
        let constat = tour(&mut reparations, &banc, &effondre, instant);
        exige_un_geste(&banc, KRAKEN, &format!("la tentative n° {tentative}"));
        let Constat::Echouee {
            tentative: rang_rendu,
            erreur,
        } = &constat
        else {
            panic!(
                "la tentative n° {tentative} a échoué : elle doit se dire échouée, trouvé « {} »",
                forme(&constat)
            );
        };
        assert_eq!(
            *rang_rendu, tentative,
            "le rang rapporté doit être celui de la tentative"
        );
        assert!(
            erreur.contains(RAISON_DU_RESET),
            "la raison rendue par l'`ioctl` est le seul diagnostic que l'opérateur reçoit ; elle \
             doit contenir « {RAISON_DU_RESET} », trouvé « {erreur} »"
        );
    }

    // L'échéance suivante : le plafond est atteint, la source se tait toujours. C'est là que
    // l'abandon se prononce, et c'est la ligne du journal.
    let abandon = DEBUT + DELAI_ENTRE_TENTATIVES * TENTATIVES_MAXIMALES;
    let constat = tour(&mut reparations, &banc, &effondre, abandon - INSTANT);
    exige_aucun_geste(&banc, "un instant avant l'abandon");
    assert_eq!(
        constat,
        Constat::Patiente,
        "avant l'échéance, on patiente encore — l'abandon n'est pas anticipé"
    );

    let constat = tour(&mut reparations, &banc, &effondre, abandon);
    exige_aucun_geste(&banc, "le tour de l'abandon");
    assert_eq!(
        constat,
        Constat::Abandon,
        "après {TENTATIVES_MAXIMALES} tentatives et autant de délais, le démon renonce — et c'est \
         ce constat-là qui se journalise"
    );

    // Puis une heure entière, tour par seconde : plus un geste, plus un mot.
    let mut abandons = 0u32;
    for i in 1..=3_600u32 {
        let constat = tour(&mut reparations, &banc, &effondre, abandon + CYCLE * i);
        exige_aucun_geste(&banc, &format!("tour n° {i} après l'abandon"));
        if constat == Constat::Abandon {
            abandons += 1;
        }
        assert_eq!(
            constat,
            Constat::Repos,
            "tour n° {i} après l'abandon : il est déjà prononcé, il n'y a rien à tenter et rien à \
             dire"
        );
    }
    assert_eq!(
        abandons, 0,
        "sur une heure de repos, l'abandon a été prononcé {abandons} fois de plus : le journal \
         recevrait une ligne par seconde"
    );
}

// ---------------------------------------------------------------------------
// 6 — un reset qui échoue ne se dit jamais réussi, et ne fait pas boucler
// ---------------------------------------------------------------------------

#[test]
fn un_reset_qui_echoue_ne_se_dit_jamais_reussi_et_ne_fait_pas_boucler() {
    // Critère d'acceptation : « Un reset qui échoue laisse le démon dans l'état d'avant —
    // quarantaines comprises — et ne fait pas boucler la tentative ».
    //
    // « L'état d'avant » se joue sur ce que le constat autorise : seul `Constat::Reussie` porte
    // l'instruction de redécouvrir et de libérer les quarantaines. Ce test exige donc que
    // `Reussie` soit rendu **si et seulement si** l'`ioctl` a rendu `Ok` — ce qui interdit d'un
    // même geste les deux fautes : le reset raté qui purgerait quand même les quarantaines, et le
    // reset réussi qui ne le dirait pas.
    //
    // Le scénario alterne à dessein — réussites, échecs, raisons différentes — parce qu'une
    // décision ne se trompe pas sur une suite régulière.
    let mut reparations = Reparations::nouvelles();
    let banc = Banc::neuf();
    let effondre = effondree(KRAKEN, &CIBLES_KRAKEN);

    // Les verdicts de l'`ioctl`, dans l'ordre des tentatives. Le premier échoue, le deuxième
    // aussi mais autrement, le troisième réussit sans rien ramener.
    let verdicts: [Option<&str>; 3] = [Some(RAISON_DU_RESET), Some(AUTRE_RAISON_DU_RESET), None];

    let mut gestes_comptes = 0u32;
    for (rang_du_verdict, verdict) in verdicts.into_iter().enumerate() {
        let tentative = u32::try_from(rang_du_verdict).unwrap() + 1;
        if tentative > TENTATIVES_MAXIMALES {
            break;
        }
        match verdict {
            Some(raison) => banc.echoue(KRAKEN, raison),
            None => banc.reussit(KRAKEN),
        }

        let instant = DEBUT + DELAI_ENTRE_TENTATIVES * (tentative - 1);
        let constat = tour(&mut reparations, &banc, &effondre, instant);
        exige_un_geste(&banc, KRAKEN, &format!("la tentative n° {tentative}"));
        gestes_comptes += 1;

        match (verdict, &constat) {
            (
                Some(raison),
                Constat::Echouee {
                    tentative: r,
                    erreur,
                },
            ) => {
                assert_eq!(
                    *r, tentative,
                    "le rang rapporté doit être celui de la tentative"
                );
                assert!(
                    erreur.contains(raison),
                    "la tentative n° {tentative} a échoué sur « {raison} » : la raison doit \
                     traverser la couture, c'est le seul diagnostic que l'opérateur reçoit ; \
                     trouvé « {erreur} »"
                );
            }
            (None, Constat::Reussie { tentative: r }) => {
                assert_eq!(*r, tentative);
            }
            (Some(_), autre) => panic!(
                "la tentative n° {tentative} a échoué : elle ne peut pas se dire « {} ». Un reset \
                 raté qui se dirait réussi ferait libérer les quarantaines et redécouvrir des \
                 cibles qui n'ont pas bougé",
                forme(autre)
            ),
            (None, autre) => panic!(
                "la tentative n° {tentative} a réussi : elle doit se dire réussie, trouvé « {} »",
                forme(autre)
            ),
        }

        // Et surtout : le tour suivant ne relance rien. Un reset raté ne se rejoue pas dans la
        // seconde — c'est le « ne fait pas boucler » du critère, et c'est ce qui distingue une
        // tentative bornée d'une insistance.
        let constat = tour(&mut reparations, &banc, &effondre, instant + CYCLE);
        exige_aucun_geste(
            &banc,
            &format!("le tour qui suit immédiatement la tentative n° {tentative}"),
        );
        assert_eq!(constat, Constat::Patiente);
    }

    assert!(
        gestes_comptes >= 2,
        "le scénario doit comporter au moins un échec et une réussite pour dire quelque chose ; \
         {gestes_comptes} tentative(s) jouée(s)"
    );
}

// ---------------------------------------------------------------------------
// 7 — un reset réussi qui ne ramène pas la source ne relance pas le compteur
// ---------------------------------------------------------------------------

#[test]
fn un_reset_reussi_qui_ne_ramene_pas_la_source_ne_relance_pas_le_compteur() {
    // ⚠️ **La boucle que l'issue interdit, atteinte par le chemin qu'elle ne nomme pas.**
    //
    // Le critère écrit parle du reset qui **échoue**. Mais `USBDEVFS_RESET` rend `Ok` dès que le
    // noyau a réinitialisé le port : il ne dit rien de ce que le firmware fait ensuite, et
    // l'incident du 2026-08-15 est précisément celui d'un périphérique **énuméré** qui ne répond
    // plus. Un reset peut donc parfaitement réussir sans rien ramener.
    //
    // Si la réussite du geste remettait le compteur à zéro, le démon repartirait indéfiniment au
    // premier essai : reset, cibles toujours muettes, reset, … une fois par délai, pour toujours.
    // Le plafond serait écrit et inatteignable.
    //
    // D'où la règle que ce fichier tranche : **c'est la source qui répond à nouveau qui remet le
    // compteur à zéro, jamais l'`ioctl` qui rend `Ok`.**
    let mut reparations = Reparations::nouvelles();
    let banc = Banc::neuf();
    banc.reussit(KRAKEN);

    // La source reste effondrée d'un bout à l'autre : chaque reset réussit, et ne ramène rien.
    let effondre = effondree(KRAKEN, &CIBLES_KRAKEN);

    let mut rangs = Vec::new();
    for tentative in 1..=TENTATIVES_MAXIMALES {
        let instant = DEBUT + DELAI_ENTRE_TENTATIVES * (tentative - 1);
        let constat = tour(&mut reparations, &banc, &effondre, instant);
        exige_un_geste(&banc, KRAKEN, &format!("la tentative n° {tentative}"));
        rangs.push(rang(&constat));
        assert_eq!(
            constat,
            Constat::Reussie { tentative },
            "le geste a réussi : il se dit réussi, avec son rang — mais « réussie » qualifie \
             l'`ioctl`, pas la guérison"
        );
    }
    assert_eq!(
        rangs,
        (1..=TENTATIVES_MAXIMALES).map(Some).collect::<Vec<_>>(),
        "les rangs doivent monter de 1 à {TENTATIVES_MAXIMALES} — un compteur remis à zéro par la \
         réussite du geste les laisserait tous à 1, et le plafond ne serait jamais atteint"
    );

    // Et le plafond est bien atteint, alors même qu'aucun `ioctl` n'a échoué.
    let abandon = DEBUT + DELAI_ENTRE_TENTATIVES * TENTATIVES_MAXIMALES;
    let constat = tour(&mut reparations, &banc, &effondre, abandon);
    exige_aucun_geste(&banc, "le tour de l'abandon");
    assert_eq!(
        constat,
        Constat::Abandon,
        "après {TENTATIVES_MAXIMALES} resets réussis qui n'ont rien ramené, le démon renonce — \
         sinon il secouerait le Kraken toutes les {DELAI_ENTRE_TENTATIVES:?} jusqu'au redémarrage"
    );

    // Puis plus rien, pendant une demi-heure de tours.
    for i in 1..=1_800u32 {
        let constat = tour(&mut reparations, &banc, &effondre, abandon + CYCLE * i);
        exige_aucun_geste(&banc, &format!("tour n° {i} après l'abandon"));
        assert_eq!(constat, Constat::Repos);
    }
}

// ---------------------------------------------------------------------------
// 8 — une source revenue puis ré-effondrée est un nouvel épisode
// ---------------------------------------------------------------------------

#[test]
fn une_source_revenue_puis_reeffondree_est_un_nouvel_episode() {
    // Le pendant du test précédent, et la faute symétrique : une décision qui, pour ne pas boucler,
    // condamnerait la source à jamais.
    //
    // Le Kraken de l'issue a lâché deux fois en deux jours, et il est **revenu** entre les deux —
    // par un redémarrage, mais il est revenu. Une source qui répond de nouveau a fait la preuve
    // qu'elle répond : son prochain effondrement est un incident neuf, qui mérite ses tentatives et
    // sa ligne de journal. C'est la même règle que « une sonde guérie qui retombe se journalise à
    // nouveau » (#68), et pour la même raison : un contrôleur qui clignote est justement celui dont
    // on veut entendre parler.
    let mut reparations = Reparations::nouvelles();
    let banc = Banc::neuf();
    banc.echoue(KRAKEN, RAISON_DU_RESET);

    let effondre = effondree(KRAKEN, &CIBLES_KRAKEN);
    let vivante = intacte(KRAKEN, &CIBLES_KRAKEN);

    // Premier épisode : les N tentatives, puis l'abandon.
    for tentative in 1..=TENTATIVES_MAXIMALES {
        tour(
            &mut reparations,
            &banc,
            &effondre,
            DEBUT + DELAI_ENTRE_TENTATIVES * (tentative - 1),
        );
        exige_un_geste(
            &banc,
            KRAKEN,
            &format!("tentative n° {tentative}, épisode 1"),
        );
    }
    let abandon = DEBUT + DELAI_ENTRE_TENTATIVES * TENTATIVES_MAXIMALES;
    assert_eq!(
        tour(&mut reparations, &banc, &effondre, abandon),
        Constat::Abandon
    );
    exige_aucun_geste(&banc, "l'abandon du premier épisode");

    // La source revient — une seule cible qui répond suffit à le prouver.
    let retour = abandon + CYCLE;
    let partielle = etat(KRAKEN, &CIBLES_KRAKEN, &[FAN, POMPE]);
    assert_eq!(
        tour(&mut reparations, &banc, &partielle, retour),
        Constat::Rien,
        "une seule cible qui répond suffit : la source n'est plus effondrée"
    );
    exige_aucun_geste(&banc, "le retour partiel");

    // Puis tout revient, et on laisse passer du temps.
    for i in 1..=600u32 {
        assert_eq!(
            tour(&mut reparations, &banc, &vivante, retour + CYCLE * i),
            Constat::Rien
        );
        exige_aucun_geste(&banc, &format!("tour n° {i}, source revenue"));
    }

    // Second effondrement : c'est un épisode neuf. La tentative porte le rang 1, pas le rang
    // N + 1, et surtout elle a lieu — une décision qui aurait condamné la source ne bougerait plus.
    let rechute = retour + CYCLE * 601;
    let constat = tour(&mut reparations, &banc, &effondre, rechute);
    exige_un_geste(&banc, KRAKEN, "la première tentative du second épisode");
    assert_eq!(
        rang(&constat),
        Some(1),
        "le second effondrement est un incident neuf : ses tentatives repartent de 1, trouvé \
         {constat:?}"
    );

    // Et le second épisode va jusqu'à son propre abandon, qui se dit à nouveau : c'est une nouvelle
    // ligne de journal, pour un nouvel incident.
    for tentative in 2..=TENTATIVES_MAXIMALES {
        tour(
            &mut reparations,
            &banc,
            &effondre,
            rechute + DELAI_ENTRE_TENTATIVES * (tentative - 1),
        );
        exige_un_geste(
            &banc,
            KRAKEN,
            &format!("tentative n° {tentative}, épisode 2"),
        );
    }
    assert_eq!(
        tour(
            &mut reparations,
            &banc,
            &effondre,
            rechute + DELAI_ENTRE_TENTATIVES * TENTATIVES_MAXIMALES
        ),
        Constat::Abandon,
        "le second épisode a son propre abandon : le taire ferait disparaître du journal le second \
         incident, c'est-à-dire celui qui prouve que le premier n'était pas un accident"
    );
}

// ---------------------------------------------------------------------------
// 9 — deux sources effondrées ont chacune leur compte de tentatives
// ---------------------------------------------------------------------------

#[test]
fn deux_sources_effondrees_ont_chacune_leur_compte_de_tentatives() {
    // L'issue met hors scope « réparer autre chose que le Kraken », mais demande explicitement que
    // « le mécanisme soit écrit pour ne pas lui être propre ». La forme la plus serrée de cette
    // exigence : non seulement l'état est séparé par source, mais le **compte de tentatives** et
    // l'**échéance** le sont aussi.
    //
    // Une décision qui garderait un compteur global passerait tous les tests précédents — un seul
    // Kraken y figure — et se ferait attraper ici : la seconde source, qui vient de s'effondrer,
    // hériterait du compte de la première et serait abandonnée sans avoir eu une seule tentative.
    //
    // Les deux effondrements démarrent à des instants différents pour qu'aucune coïncidence ne
    // puisse les faire passer pour indépendants.
    let mut reparations = Reparations::nouvelles();
    let banc = Banc::neuf();
    banc.echoue(KRAKEN, RAISON_DU_RESET);
    banc.echoue(SMART2, AUTRE_RAISON_DU_RESET);

    let kraken_effondre = effondree(KRAKEN, &CIBLES_KRAKEN);
    let smart2_effondre = effondree(SMART2, &CIBLES_SMART2);
    let smart2_vivant = intacte(SMART2, &CIBLES_SMART2);

    // Le Kraken s'effondre et brûle toutes ses tentatives, pendant que l'autre source va bien.
    for tentative in 1..=TENTATIVES_MAXIMALES {
        let instant = DEBUT + DELAI_ENTRE_TENTATIVES * (tentative - 1);
        tour(&mut reparations, &banc, &kraken_effondre, instant);
        exige_un_geste(
            &banc,
            KRAKEN,
            &format!("tentative n° {tentative} du Kraken"),
        );

        assert_eq!(
            tour(&mut reparations, &banc, &smart2_vivant, instant),
            Constat::Rien,
            "« {SMART2} » va bien : l'effondrement de « {KRAKEN} » ne la concerne pas"
        );
        exige_aucun_geste(&banc, "la source qui va bien");
    }
    let abandon_kraken = DEBUT + DELAI_ENTRE_TENTATIVES * TENTATIVES_MAXIMALES;
    assert_eq!(
        tour(&mut reparations, &banc, &kraken_effondre, abandon_kraken),
        Constat::Abandon
    );
    exige_aucun_geste(&banc, "l'abandon du Kraken");

    // Puis la seconde source s'effondre à son tour, bien plus tard. Elle a droit à ses propres
    // tentatives, à partir de 1.
    let effondrement_smart2 = abandon_kraken + CYCLE * 7;
    let constat = tour(
        &mut reparations,
        &banc,
        &smart2_effondre,
        effondrement_smart2,
    );
    exige_un_geste(&banc, SMART2, "la première tentative de la seconde source");
    assert_eq!(
        rang(&constat),
        Some(1),
        "« {SMART2} » vient de s'effondrer : sa première tentative porte le rang 1, quel que soit \
         ce que « {KRAKEN} » a consommé — trouvé {constat:?}"
    );

    // Et le Kraken, abandonné, reste au repos pendant tout ce temps : la seconde source ne le
    // réveille pas plus qu'il ne l'a condamnée.
    assert_eq!(
        tour(
            &mut reparations,
            &banc,
            &kraken_effondre,
            effondrement_smart2
        ),
        Constat::Repos
    );
    exige_aucun_geste(
        &banc,
        "le Kraken abandonné, pendant l'épisode de l'autre source",
    );

    // Les échéances sont indépendantes, elles aussi : la seconde source est due à son propre
    // rythme, décalé de sept secondes de celui du Kraken.
    let echeance_smart2 = effondrement_smart2 + DELAI_ENTRE_TENTATIVES;
    assert_eq!(
        tour(
            &mut reparations,
            &banc,
            &smart2_effondre,
            echeance_smart2 - INSTANT
        ),
        Constat::Patiente
    );
    exige_aucun_geste(&banc, "un instant avant l'échéance de la seconde source");

    let constat = tour(&mut reparations, &banc, &smart2_effondre, echeance_smart2);
    exige_un_geste(&banc, SMART2, "l'échéance de la seconde source");
    assert_eq!(rang(&constat), Some(2));
}

// ---------------------------------------------------------------------------
// 10 — une réparation réussie libère les quarantaines de cette source, et d'aucune autre
// ---------------------------------------------------------------------------

#[test]
fn une_reparation_reussie_libere_les_quarantaines_de_cette_source_et_d_aucune_autre() {
    // Critère d'acceptation : « Après une réparation réussie, les quarantaines de cette source sont
    // remises à zéro ».
    //
    // Sans cela, la réparation ne servirait à rien pendant cinq minutes : un Kraken qui vient d'être
    // réinitialisé porterait encore le délai accumulé par ses cibles avant la panne — jusqu'à
    // {DELAI_MAXIMAL:?} —, et le démon attendrait tout ce temps avant de découvrir qu'il est revenu.
    //
    // « Remises à zéro » veut dire **oubliées**, pas « retentées plus tôt » : la cible redevient une
    // cible ordinaire, lue au tour suivant, et sa prochaine mise en quarantaine est un événement
    // neuf qui se journalise. La distinction n'est pas cosmétique — c'est elle qui fait apparaître
    // dans le journal une panne qui revient tout de suite après une réparation.
    //
    // ⚠️ Ce test n'exerce que la libération. La politique de la quarantaine — délais, doublement,
    // plafond — est hors scope de #98 et a ses deux fichiers.
    let mut quarantaine = Quarantaine::nouvelle();
    let lectures = RefCell::new(Vec::new());

    // Toutes les cibles du Kraken se taisent, et une cible d'une autre source aussi — c'est le
    // décor exact de l'incident, où le CPU et le contrôleur de ventilation continuaient de
    // répondre pendant que le Kraken tombait.
    for cible in CIBLES_KRAKEN.iter().chain(std::iter::once(&VENTILO_1)) {
        assert_eq!(
            releve(&mut quarantaine, &lectures, cible, DEBUT, None),
            Releve::Muette { signaler: true },
            "« {cible} » vient d'échouer : elle entre en quarantaine, et ça se journalise"
        );
    }
    lectures.borrow_mut().clear();

    // Un tour plus tard, personne n'est relevé : c'est la quarantaine de #68 qui fait son travail.
    for cible in CIBLES_KRAKEN.iter().chain(std::iter::once(&VENTILO_1)) {
        assert_eq!(
            releve(
                &mut quarantaine,
                &lectures,
                cible,
                DEBUT + CYCLE,
                Some(PIEGE)
            ),
            Releve::Muette { signaler: false },
        );
    }
    assert!(
        lectures.borrow().is_empty(),
        "aucune cible en quarantaine ne doit être relevée : {:?}",
        lectures.borrow()
    );

    // Le reset réussit : le démon libère les cibles **de cette source**, une par une — il les a
    // toutes dans `EtatSource::cibles`.
    for cible in CIBLES_KRAKEN {
        quarantaine.oublie(cible);
    }

    // Au tour suivant, les trois cibles du Kraken sont relevées, sans attendre la moindre échéance.
    // La cible de l'autre source, elle, dort toujours.
    let apres = DEBUT + CYCLE * 2;
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
            "« {cible} » a été libérée : elle est relevée sans délai, comme une cible jamais vue — \
             attendre son échéance ferait patienter jusqu'à {DELAI_MAXIMAL:?} après une réparation \
             réussie"
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
    assert!(
        lectures.borrow().is_empty(),
        "libérer les cibles du Kraken ne doit pas relever celles d'une autre source : {:?}",
        lectures.borrow()
    );

    // Et l'oubli est bien un oubli : si une cible libérée retombe, c'est une **nouvelle** mise en
    // quarantaine, qui se journalise. Une implémentation qui se serait contentée d'avancer
    // l'échéance rendrait ici `signaler: false`, et la panne qui revient aussitôt après une
    // réparation ne laisserait aucune trace.
    assert_eq!(
        releve(&mut quarantaine, &lectures, LIQUIDE, apres + CYCLE, None),
        Releve::Muette { signaler: true },
        "une cible libérée puis retombée entre à nouveau en quarantaine : c'est un événement neuf"
    );

    // ⚠️ Le délai reparti de zéro, lui aussi : la retente est due après {DELAI_INITIAL:?}, et non
    // après ce que la cible avait accumulé avant la réparation.
    lectures.borrow_mut().clear();
    let rechute = apres + CYCLE;
    assert_eq!(
        releve(
            &mut quarantaine,
            &lectures,
            LIQUIDE,
            rechute + DELAI_INITIAL - INSTANT,
            Some(PIEGE)
        ),
        Releve::Muette { signaler: false }
    );
    assert!(lectures.borrow().is_empty());
    releve(
        &mut quarantaine,
        &lectures,
        LIQUIDE,
        rechute + DELAI_INITIAL,
        None,
    );
    assert_eq!(
        lectures.borrow().clone(),
        vec![LIQUIDE.to_owned()],
        "après une libération, la première retente est due après {DELAI_INITIAL:?}"
    );
}

// ---------------------------------------------------------------------------
// 11 — la décision ne touche aucun périphérique, ne connaît aucun chemin, et voyage entre fils
// ---------------------------------------------------------------------------

#[test]
fn la_decision_ne_touche_aucun_peripherique_ne_connait_aucun_chemin_et_voyage_entre_fils() {
    // Trois critères d'acceptation se rejoignent ici, et aucun ne se vérifie par une égalité de
    // valeur :
    //
    // — « Le périphérique à réinitialiser est résolu par **VID:PID + série**, jamais par un chemin
    //   codé en dur ». La résolution est le **geste**, elle vit derrière la fermeture ; ce que la
    //   partie pure peut garantir, c'est qu'aucun chemin ne traverse la couture. `EtatSource` ne
    //   nomme qu'une source, et [`forme`] filtre `Constat` de façon **exhaustive, sans `..` ni
    //   `_`** : y glisser un `chemin: PathBuf` casse la compilation de ce fichier.
    // — « Elle vit **hors du fil qui sert le socket**, comme la dalle depuis #83 ». Une décision qui
    //   ne serait pas `Send` ne pourrait pas y vivre. C'est une borne de compilation, pas une
    //   promesse.
    // — « La tentative ne bloque pas le socket ». Ce test ne chronomètre rien — une durée réelle
    //   mesurerait la charge de la machine. Il compte : sur une minute de tours, la décision ne
    //   déclenche qu'un nombre borné de gestes, et **rien du tout** le reste du temps. Un tour qui
    //   ne fait rien ne bloque personne.
    //
    // ⚠️ Ce que ce test **ne** dit **pas** : combien de temps un `USBDEVFS_RESET` prend. Personne
    // ne l'a mesuré, et inventer une durée ici la figerait dans un test qui prétendrait l'avoir
    // observée. Le seul modèle honnête est le **compte**.
    fn exige_send<T: Send>() {}
    exige_send::<Reparations>();
    exige_send::<EtatSource>();
    exige_send::<Constat>();

    let mut reparations = Reparations::nouvelles();
    let banc = Banc::neuf();
    banc.echoue(KRAKEN, RAISON_DU_RESET);
    let effondre = effondree(KRAKEN, &CIBLES_KRAKEN);

    let mut gestes = 0usize;
    let mut formes = Vec::new();
    for i in 0..60u32 {
        let constat = tour(&mut reparations, &banc, &effondre, DEBUT + CYCLE * i);
        gestes += banc.gestes().len();
        formes.push(forme(&constat));
    }

    // Une minute de tours, et le compte des resets tient dans le plafond : la décision ne peut pas,
    // même en régime dégradé, secouer le périphérique plus que ce qu'elle s'est autorisé.
    assert!(
        gestes <= usize::try_from(TENTATIVES_MAXIMALES).unwrap(),
        "sur soixante tours d'une seconde, {gestes} reset(s) ont eu lieu pour un plafond de \
         {TENTATIVES_MAXIMALES} : une décision qui dépasse son propre plafond ne le respecte nulle \
         part"
    );

    // Et l'écrasante majorité des tours ne fait rien du tout — c'est ce qui laisse le budget de
    // #88 intact : un tour sans geste ne consomme pas les 100 ms d'un `status`.
    let inertes = formes
        .iter()
        .filter(|f| **f == "patiente" || **f == "repos")
        .count();
    assert!(
        inertes >= 55,
        "seulement {inertes} tours sur 60 n'ont rien tenté : la décision doit être au repos \
         l'essentiel du temps, sinon les {BUDGET_STATUS:?} du budget de #88 partiraient en resets"
    );

    // Enfin, la couture ne transporte que des noms. Un chemin qui s'y serait glissé — sous
    // n'importe quel prétexte, « pour éviter de re-résoudre » en tête — se lirait ici.
    assert!(
        !effondre.source.contains('/') && !effondre.source.contains(char::is_whitespace),
        "« {} » n'est pas un nom de source",
        effondre.source
    );
    for cible in &effondre.cibles {
        assert!(
            !cible.contains('/'),
            "« {cible} » ressemble à un chemin : les cibles se nomment, elles ne se situent pas — \
             les numéros `hwmonN` changent au redémarrage, et depuis #98 à chaque reset"
        );
    }
}

// ---------------------------------------------------------------------------
// 12 — garder les anciennes poignées après un reset ferait lire le mauvais hwmon
// ---------------------------------------------------------------------------

#[test]
fn garder_les_anciennes_poignees_apres_un_reset_ferait_lire_le_mauvais_hwmon() {
    // Critère d'acceptation : « Après une réparation réussie, les canaux et sondes de la source
    // sont relus sous leurs noms, même si le numéro `hwmonN` a changé ».
    //
    // Que le **nom** survive à la renumérotation est déjà figé ailleurs : `spec_sondes.rs` (#31,
    // `le_slug_ne_depend_pas_du_numero_du_repertoire_hwmon`) et `spec_fans.rs` (#7,
    // `l_ordre_ne_depend_pas_du_numero_des_repertoires_hwmon`). Ce fichier n'a aucune raison de le
    // vérifier une troisième fois.
    //
    // Ce qui manque, et que #98 rend soudain nécessaire, c'est le **corollaire** : les chemins, eux,
    // changent. Une redécouverte n'est donc pas une politesse — garder les anciennes poignées après
    // un reset, c'est lire le fichier d'un **autre** périphérique et l'afficher sous le nom du
    // premier. Le mode de défaillance le plus coûteux du projet est celui qui rassure : une
    // température plausible sous le mauvais nom en est un.
    //
    // Rien n'est lu ni écrit sous `/sys` : l'arborescence est construite dans un dossier temporaire,
    // effacée à la fin du test, et lue par `hwmon::sondes_dans` — la fonction qui prend sa racine en
    // paramètre pour exactement cette raison.
    let dossier = DossierJetable::neuf("poignees");
    let racine = dossier.chemin().to_path_buf();

    // Avant le reset : le Kraken en `hwmon4`, le CPU en `hwmon7`.
    poser_sonde(&racine, "hwmon4", KRAKEN, "Coolant", TEMPERATURE_LIQUIDE);
    poser_sonde(&racine, "hwmon7", "k10temp", "Tctl", TEMPERATURE_CPU);

    let avant = hwmon::sondes_dans(&racine).expect("découverte avant le reset");
    let kraken_avant = sonde_de(&avant, KRAKEN);
    assert_eq!(
        kraken_avant.lire().expect("lecture avant le reset"),
        TEMPERATURE_LIQUIDE,
        "avant le reset, la sonde du Kraken lit bien la sienne"
    );

    // Le reset a lieu : le périphérique disparaît du bus puis y revient, et le noyau lui donne le
    // numéro libre. Les deux `hwmon` ont échangé leurs numéros — c'est exactement ce que le
    // CLAUDE.md relève des `hidraw`, « constaté, pas supposé », et un reset le provoque à volonté.
    fs::remove_dir_all(racine.join("hwmon4")).expect("retrait de hwmon4");
    fs::remove_dir_all(racine.join("hwmon7")).expect("retrait de hwmon7");
    poser_sonde(&racine, "hwmon7", KRAKEN, "Coolant", TEMPERATURE_LIQUIDE);
    poser_sonde(&racine, "hwmon4", "k10temp", "Tctl", TEMPERATURE_CPU);

    let apres = hwmon::sondes_dans(&racine).expect("découverte après le reset");
    let kraken_apres = sonde_de(&apres, KRAKEN);

    // Le nom, lui, n'a pas bougé : c'est ce qui rend la redécouverte « par le nom » possible.
    assert_eq!(
        kraken_apres.slug, kraken_avant.slug,
        "le nom d'une sonde ne dépend pas du numéro de son répertoire (#31) — c'est ce qui permet \
         de retrouver une cible après un reset"
    );

    // Le chemin, lui, a changé. Voilà pourquoi une redécouverte est obligatoire.
    assert_ne!(
        kraken_apres.chemin, kraken_avant.chemin,
        "le chemin doit avoir changé, sinon ce test ne dit rien du danger qu'il décrit"
    );

    // Et la faute que #98 doit rendre impossible, en toutes lettres : l'ancienne poignée lit
    // maintenant la température du CPU, et l'appellerait « liquide ».
    assert_eq!(
        kraken_avant.lire().expect("lecture de l'ancienne poignée"),
        TEMPERATURE_CPU,
        "l'ancienne poignée du Kraken pointe désormais sur le `hwmon` du CPU : un démon qui ne \
         redécouvre pas afficherait {TEMPERATURE_CPU} millidegrés sous le nom « {LIQUIDE} », et \
         rien ne le signalerait"
    );
    assert_eq!(
        kraken_apres.lire().expect("lecture de la nouvelle poignée"),
        TEMPERATURE_LIQUIDE,
        "la poignée redécouverte, elle, lit la bonne"
    );
}

// ---------------------------------------------------------------------------
// L'arborescence de test — aucun `/sys`, aucun périphérique
// ---------------------------------------------------------------------------

/// Crée `<racine>/<dossier>` avec son `name`, son `temp1_label` et son `temp1_input`.
fn poser_sonde(racine: &Path, dossier: &str, source: &str, libelle: &str, millidegres: i32) {
    let chemin = racine.join(dossier);
    fs::create_dir_all(&chemin)
        .unwrap_or_else(|erreur| panic!("création de {} : {erreur}", chemin.display()));
    ecrire(&chemin.join("name"), source);
    ecrire(&chemin.join("temp1_label"), libelle);
    ecrire(&chemin.join("temp1_input"), &millidegres.to_string());
}

fn ecrire(chemin: &Path, contenu: &str) {
    fs::write(chemin, format!("{contenu}\n"))
        .unwrap_or_else(|erreur| panic!("écriture de {} : {erreur}", chemin.display()));
}

/// La sonde d'une source donnée, ou un échec qui nomme ce qui manque.
fn sonde_de<'a>(sondes: &'a [hwmon::Sonde], source: &str) -> &'a hwmon::Sonde {
    sondes
        .iter()
        .find(|sonde| sonde.origine == source)
        .unwrap_or_else(|| {
            panic!(
                "aucune sonde pour « {source} » parmi {:?}",
                sondes.iter().map(|s| &s.slug).collect::<Vec<_>>()
            )
        })
}

/// Un dossier de travail sous `std::env::temp_dir()`, effacé à la fin du test.
///
/// L'effacement passe par `Drop` et non par une ligne en fin de test : un test qui échoue part en
/// `panic!`, et une fin de test jamais atteinte laisserait le dossier derrière elle.
struct DossierJetable {
    chemin: PathBuf,
}

impl DossierJetable {
    fn neuf(nom: &str) -> DossierJetable {
        let chemin = std::env::temp_dir().join(format!(
            "reverb-spec-reparation-{}-{nom}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&chemin);
        fs::create_dir_all(&chemin)
            .unwrap_or_else(|erreur| panic!("dossier de test {} : {erreur}", chemin.display()));
        DossierJetable { chemin }
    }

    fn chemin(&self) -> &Path {
        &self.chemin
    }
}

impl Drop for DossierJetable {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.chemin);
    }
}
