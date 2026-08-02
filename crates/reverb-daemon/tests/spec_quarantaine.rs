//! Tests d'intention de la quarantaine des sondes (issue #68).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #68 seule — aucun fichier de
//! `crates/reverb-daemon/src/` n'a été lu pour les produire, hors la liste des `pub mod` de
//! `lib.rs` et la signature de `telemetrie::releve`. À l'écriture de ce fichier, le module
//! `quarantaine` **n'existe pas** : la compilation de ce test doit échouer, et c'est la phase
//! rouge.
//!
//! Rien ici n'ouvre de fichier, ne lit un `hwmon`, ni ne dort : le temps est **injecté**, chaque
//! méthode qui en dépend reçoit son instant. Un test qui appellerait `Instant::now()` mesurerait
//! l'ordonnanceur de la machine, pas la quarantaine — et surtout, vérifier un plafond de cinq
//! minutes en attendant cinq minutes n'est pas une option.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Constaté sur SHYNAEL le 2026-08-02 : le Kraken avait cessé de répondre à son pilote noyau, et
//! une simple lecture sysfs y bloquait **5,218 s** avant d'échouer — quand l'autre contrôleur
//! répondait en **1 ms**. Le démon relève ses sondes chaque seconde, dans le thread qui sert aussi
//! le socket (ADR-002) : `zone list` et `geometry`, qui ne touchent aucun matériel, ne répondaient
//! plus en douze secondes, thread principal en état `D` à 0 % de processeur.
//!
//! ⚠️ **Le défaut n'est pas que la sonde échoue, c'est qu'elle emporte tout le reste avec elle.**
//!
//! ## La signature que ces tests exigent, et pourquoi celle-là
//!
//! L'issue pose que la décision est **pure** : « un état par sonde, un temps passé en paramètre,
//! aucune E/S », sur le modèle de `Poignee` et `Limiteur` dans `reverb-gui/src/reglages.rs`. Elle
//! ne dit pas quelle forme lui donner. Ce fichier tranche, et voici ce qu'il faut implémenter :
//!
//! ```ignore
//! // crates/reverb-daemon/src/quarantaine.rs
//! // et `pub mod quarantaine;` dans crates/reverb-daemon/src/lib.rs
//!
//! use std::time::Duration;
//!
//! /// Délai avant la première retente.
//! pub const DELAI_INITIAL: Duration = Duration::from_secs(30);
//!
//! /// Plafond du délai de retente.
//! pub const DELAI_MAXIMAL: Duration = Duration::from_secs(300);
//!
//! /// Ce qu'un tour de relevé a donné pour une sonde.
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub enum Releve<T> {
//!     /// La sonde a répondu, et voici ce qu'elle a rendu.
//!     Valeur(T),
//!     /// La sonde est muette : soit elle vient d'échouer, soit elle est en quarantaine
//!     /// et n'a pas été relevée du tout.
//!     Muette {
//!         /// Vrai une seule fois par mise en quarantaine — c'est le tour qui se journalise.
//!         signaler: bool,
//!     },
//! }
//!
//! pub struct Quarantaine { /* un état par sonde */ }
//!
//! impl Quarantaine {
//!     pub fn nouvelle() -> Quarantaine;
//!
//!     pub fn tour<T>(
//!         &mut self,
//!         slug: &str,
//!         maintenant: Duration,
//!         relever: impl FnOnce() -> Option<T>,
//!     ) -> Releve<T>;
//! }
//! ```
//!
//! Trois choix, et ce qu'ils achètent :
//!
//! 1. **La quarantaine reçoit le relevé en fermeture, elle ne le précède pas.** Un simple
//!    `faut_il_relever(slug, maintenant) -> bool` aurait laissé « n'est plus relevée » à la
//!    politesse de l'appelant : une `telemetrie` qui lirait le fichier puis consulterait la
//!    décision passerait tous les tests d'égalité, et gèlerait le démon exactement comme avant.
//!    En prenant la fermeture, `tour` **est** l'endroit où la lecture a lieu ou n'a pas lieu, et
//!    un test qui compte les appels le vérifie. C'est la leçon de `requetes_vers_la_cible` dans
//!    `spec_limiteur.rs`.
//! 2. **`Option<T>` plutôt que `Result<T, E>` pour la fermeture.** La décision ne fait rien de la
//!    cause : une lecture qui échoue, une valeur illisible et un délai dépassé mènent au même
//!    endroit. Le module reste pur et sans paramètre de type d'erreur, et `telemetrie` y verse un
//!    `fs::read_to_string(..).ok().and_then(..)` qui est déjà sa forme actuelle.
//! 3. **`Releve<T>` n'a pas de troisième variante.** Une sonde muette est **structurellement**
//!    rapportée : il n'existe aucune façon pour `tour` de ne rien rendre. Le critère « une sonde
//!    absente et une sonde muette sont deux choses différentes » devient une propriété du type et
//!    non une discipline d'appelant — une sonde absente est une sonde dont on n'appelle jamais
//!    `tour`, et il n'y a pas de troisième cas à confondre.
//!
//! ## Ce que ce fichier fige
//!
//! 1. **Une sonde qui échoue n'est plus relevée**, et ça se mesure en **comptant les appels**, pas
//!    en comparant des sorties. Une implémentation qui lirait quand même puis jetterait le
//!    résultat rendrait exactement la même chose et gèlerait le démon tout pareil.
//! 2. **Le relevé des autres sondes est inchangé**, et la quarantaine est **par sonde** : une
//!    muette n'en écarte aucune autre, ni de son contrôleur ni d'ailleurs.
//! 3. **La muette est rapportée comme telle**, jamais omise et jamais remplacée par une valeur
//!    plausible. C'est le mode de défaillance le plus coûteux du projet parce qu'il est rassurant :
//!    un 34 °C figé derrière une pompe arrêtée, c'est un circuit qui chauffe sans que rien ne le
//!    signale (README, à propos du cadran).
//! 4. **Le délai double à chaque échec, et ne dépasse jamais son plafond** — vérifié sur quatre
//!    échecs consécutifs pour la croissance, sur vingt pour le plafond.
//! 5. **Une retente réussie sort de quarantaine et remet le délai à zéro** : une sonde guérie
//!    redevient une sonde ordinaire, pas une sonde en sursis.
//! 6. **Le journal parle une fois par mise en quarantaine**, pas à chaque cycle — et c'est une
//!    propriété d'état, portée par `signaler`, pas une discipline de l'appelant.
//!
//! ## Ce que le contrat laisse ouvert, et que ces tests tranchent
//!
//! 1. **Une sonde jamais vue est relevée sans délai.** La quarantaine est une punition, pas un
//!    droit d'entrée : au premier tour, chaque sonde est lue. Sans cette règle, une implémentation
//!    qui mettrait tout le monde en quarantaine au démarrage passerait tout le reste du fichier.
//! 2. **La retente a lieu à l'instant exact de l'échéance, pas après.** Le délai est une durée
//!    minimale de mise à l'écart, la comparaison est un `>=`. La borne est pinnée au nanoseconde
//!    près, de part et d'autre, pour attraper les deux fautes qui comptent — la quarantaine qui ne
//!    relâche jamais, et celle qui relâche tout de suite.
//! 3. **Un échec de retente ne se journalise pas.** L'issue dit « le journal dit la mise en
//!    quarantaine une fois » : c'est l'**entrée** en quarantaine qui est l'événement. Une sonde
//!    morte depuis une heure qui rate sa douzième retente n'apprend rien à personne.
//! 4. **Une sonde guérie puis retombée se journalise à nouveau.** C'est une nouvelle mise en
//!    quarantaine, donc un nouvel événement. Le contraire ferait taire à jamais une sonde qui
//!    clignote — exactement le cas où l'on veut savoir.
//! 5. **Le délai court depuis le dernier échec**, pas depuis l'entrée en quarantaine. Sans quoi
//!    une sonde échouant depuis longtemps verrait ses retentes se rapprocher au lieu de s'espacer.
//! 6. **`DELAI_INITIAL` est encadré, `DELAI_MAXIMAL` est pinné.** Le plafond de cinq minutes est un
//!    critère d'acceptation écrit ; le premier délai est un réglage, et le test en exige seulement
//!    ce que l'arithmétique de l'issue impose (voir le test n° 0).
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **La lecture sysfs elle-même**, et le rendu sur le socket. `telemetrie::releve` consulte cette
//!   décision et lui rend le verdict ; ce qu'il en fait est de l'autre côté de l'E/S, et l'issue
//!   met le fil d'exécution séparé hors scope.
//! - **Réveiller le périphérique** — `unbind`/`bind` USB : hors scope de l'issue.
//! - **Un temps qui reculerait.** L'horloge d'où vient l'instant est monotone. Inventer un
//!   comportement pour ce cas figerait une règle que personne n'a choisie.

use std::cell::RefCell;
use std::time::Duration;

use reverb_daemon::quarantaine::{DELAI_INITIAL, DELAI_MAXIMAL, Quarantaine, Releve};

// ---------------------------------------------------------------------------
// Repères et aides
// ---------------------------------------------------------------------------

/// L'origine du temps injecté.
///
/// Volontairement loin de zéro : le démon tourne depuis un moment quand une sonde lâche. Une
/// quarantaine qui prendrait `Duration::ZERO` pour « cette sonde n'a jamais échoué » se ferait
/// attraper ici plutôt qu'à l'usage.
const DEBUT: Duration = Duration::from_secs(3_600);

/// Le plus petit écart que le temps injecté sache représenter.
///
/// Sert à serrer les échéances par le dessous sans laisser d'espace entre l'essai d'avant et celui
/// d'après.
const INSTANT: Duration = Duration::from_nanos(1);

/// La cadence de relevé du démon : « le démon relève ses sondes chaque seconde » (issue #68).
const CYCLE: Duration = Duration::from_secs(1);

/// Ce que coûte une lecture d'une sonde muette, mesuré sur SHYNAEL : « 5,218 total » pour un
/// `cat` sur `temp1_input` du Kraken (issue #68).
///
/// C'est la constante qui donne son sens au délai : chaque retente gèle le socket de ce temps-là.
const COUT_DU_GEL: Duration = Duration::from_millis(5_218);

/// La sonde qui lâche : celle de l'issue, nommée par son `slug` de protocole.
const MUETTE: &str = "kraken2023elite:coolant-temp";

/// Une sonde du même contrôleur que [`MUETTE`].
///
/// Elle n'existe que pour un critère : « la quarantaine est par sonde, jamais par contrôleur ». Un
/// `slug` porte son origine en préfixe (`<nom du hwmon>:<libellé>`), donc une implémentation qui
/// écarterait par contrôleur aurait de quoi le faire — et c'est exactement ce qu'on interdit.
const MEME_CONTROLEUR: &str = "kraken2023elite:pump-speed";

/// La sonde qui répond en une milliseconde, sur l'autre contrôleur (issue #68).
const VIVANTE: &str = "nzxtsmart2:fan1";

/// Deux sondes de plus, pour qu'un relevé ait vraiment une foule.
const CPU: &str = "k10temp:Tctl";
const DISQUE: &str = "nvme:Composite";

/// Une seconde sonde qui lâche, pour vérifier que deux quarantaines s'ignorent.
const AUTRE_MUETTE: &str = DISQUE;

/// Ce que rend le Kraken quand il va bien : 34,2 °C, en millidegrés comme le veut `hwmon`.
const LIQUIDE: i32 = 34_200;

/// La valeur d'une sonde vivante, distincte de toutes les autres.
const TOURS: i32 = 716;

/// Deux valeurs de plus, distinctes.
const TEMPERATURE_CPU: i32 = 52_875;
const TEMPERATURE_DISQUE: i32 = 41_850;

/// La valeur que rendrait une sonde **si** on la relevait alors qu'elle est en quarantaine.
///
/// C'est un piège, et il attrape deux fautes d'un coup : une implémentation qui lit quand même se
/// voit au compte d'appels, **et** au verdict si elle rend ce qu'elle vient de lire.
const PIEGE: i32 = -999_999;

/// La valeur qu'une sonde muette ne doit jamais valoir. Zéro est plausible pour une température,
/// et c'est bien le problème : il ne se distingue pas d'une mesure.
const ZERO_RASSURANT: i32 = 0;

/// Les sondes qui ont vraiment été lues, dans l'ordre.
///
/// Compter les appels et non seulement comparer les sorties : une implémentation qui relèverait la
/// sonde muette puis jetterait le résultat rendrait exactement le bon verdict, et gèlerait le démon
/// cinq secondes par cycle — c'est-à-dire précisément le défaut de #68, corrigé nulle part.
#[derive(Default)]
struct Journal(RefCell<Vec<String>>);

impl Journal {
    fn neuf() -> Journal {
        Journal::default()
    }

    fn noter(&self, slug: &str) {
        self.0.borrow_mut().push(slug.to_owned());
    }

    /// Rend les appels depuis la dernière vidange, et repart à vide.
    fn vider(&self) -> Vec<String> {
        self.0.borrow_mut().drain(..).collect()
    }
}

/// Un tour de relevé pour une sonde, avec `reponse` pour ce que la lecture rendrait.
///
/// `reponse` à `None` est une sonde qui échoue — lecture impossible, valeur illisible, délai
/// dépassé : la décision n'en distingue pas les causes.
fn tour(
    quarantaine: &mut Quarantaine,
    journal: &Journal,
    slug: &str,
    maintenant: Duration,
    reponse: Option<i32>,
) -> Releve<i32> {
    quarantaine.tour(slug, maintenant, || {
        journal.noter(slug);
        reponse
    })
}

/// Exige qu'une sonde en quarantaine soit rapportée muette **sans avoir été lue**.
///
/// Le relevé lui donnerait [`PIEGE`] s'il était appelé : le verdict et le compte d'appels
/// dénoncent donc tous deux une implémentation qui lirait quand même.
///
/// Vide le journal au passage — l'appelant l'a donc à vide au retour.
fn exige_muette_sans_lecture(
    quarantaine: &mut Quarantaine,
    journal: &Journal,
    slug: &str,
    maintenant: Duration,
    contexte: &str,
) {
    let verdict = tour(quarantaine, journal, slug, maintenant, Some(PIEGE));
    assert_eq!(
        verdict,
        Releve::Muette { signaler: false },
        "{contexte} : « {slug} » est en quarantaine à {maintenant:?}, elle doit être rapportée \
         muette et sans nouvelle mise en quarantaine à journaliser"
    );
    let appels = journal.vider();
    assert!(
        appels.is_empty(),
        "{contexte} : « {slug} » est en quarantaine à {maintenant:?} et ne doit plus être relevée \
         — le relevé a pourtant été appelé pour {appels:?}, ce qui gèle le démon cinq secondes"
    );
}

/// Exige qu'une sonde soit relevée à cet instant : le relevé est appelé, une fois et une seule.
///
/// Rend le verdict, pour que l'appelant décide de ce qu'il en attend.
///
/// Vide le journal au passage.
fn exige_lue(
    quarantaine: &mut Quarantaine,
    journal: &Journal,
    slug: &str,
    maintenant: Duration,
    reponse: Option<i32>,
    contexte: &str,
) -> Releve<i32> {
    let verdict = tour(quarantaine, journal, slug, maintenant, reponse);
    let appels = journal.vider();
    assert_eq!(
        appels,
        vec![slug.to_owned()],
        "{contexte} : « {slug} » doit être relevée à {maintenant:?}, exactement une fois — les \
         appels effectifs sont {appels:?}"
    );
    verdict
}

/// Le nombre de cycles d'une seconde qui tiennent dans le premier délai **sans** qu'aucune retente
/// ne soit due.
///
/// C'est la fenêtre exacte où « la sonde n'est plus relevée » doit tenir, et elle se déduit des
/// constantes plutôt que de se coder en dur : un jour où [`DELAI_INITIAL`] serait révisé sur le
/// matériel, les tests suivraient au lieu de mentir dans un sens ou dans l'autre.
fn cycles_avant_la_premiere_retente() -> u32 {
    u32::try_from(DELAI_INITIAL.as_millis() / CYCLE.as_millis()).unwrap_or(u32::MAX) - 1
}

/// Le délai attendu après `echecs_precedents` échecs déjà encaissés : [`DELAI_INITIAL`] doublé à
/// chaque échec, plafonné à [`DELAI_MAXIMAL`].
///
/// `delai_attendu(0)` est donc le délai qui suit le tout premier échec.
fn delai_attendu(echecs_precedents: u32) -> Duration {
    DELAI_INITIAL
        .checked_mul(2u32.saturating_pow(echecs_precedents.min(31)))
        .unwrap_or(DELAI_MAXIMAL)
        .min(DELAI_MAXIMAL)
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tous les tests qui suivent supposent que les `slug` diffèrent, que les valeurs diffèrent, et
    // que les délais sont assez espacés pour que « n'est plus relevée au cycle suivant » veuille
    // dire quelque chose. Si l'un de ces repères se dégradait, plusieurs tests deviendraient vrais
    // sans rien vérifier — et personne ne le verrait. Ce test est là pour que la panne soit ici.
    let slugs = [
        ("MUETTE", MUETTE),
        ("MEME_CONTROLEUR", MEME_CONTROLEUR),
        ("VIVANTE", VIVANTE),
        ("CPU", CPU),
        ("DISQUE", DISQUE),
    ];
    for (i, (nom, slug)) in slugs.iter().enumerate() {
        assert!(!slug.is_empty(), "{nom} doit porter un nom");
        for (autre_nom, autre) in slugs.iter().skip(i + 1) {
            assert_ne!(
                slug, autre,
                "{nom} et {autre_nom} doivent différer, sinon deux sondes n'en feraient qu'une et \
                 les tests d'indépendance ne testeraient rien"
            );
        }
    }

    // La sonde du même contrôleur partage bien son préfixe avec la muette : sans ça, le critère
    // « jamais par contrôleur » serait vérifié par une coïncidence de nommage.
    let origine = |slug: &str| slug.split(':').next().unwrap_or_default().to_owned();
    assert_eq!(
        origine(MUETTE),
        origine(MEME_CONTROLEUR),
        "MEME_CONTROLEUR doit venir du même `hwmon` que MUETTE, sinon rien ne distingue une \
         quarantaine par sonde d'une quarantaine par contrôleur"
    );
    assert_ne!(
        origine(MUETTE),
        origine(VIVANTE),
        "VIVANTE doit venir d'un autre contrôleur"
    );

    let valeurs = [
        ("LIQUIDE", LIQUIDE),
        ("TOURS", TOURS),
        ("TEMPERATURE_CPU", TEMPERATURE_CPU),
        ("TEMPERATURE_DISQUE", TEMPERATURE_DISQUE),
        ("PIEGE", PIEGE),
        ("ZERO_RASSURANT", ZERO_RASSURANT),
    ];
    for (i, (nom, valeur)) in valeurs.iter().enumerate() {
        for (autre_nom, autre) in valeurs.iter().skip(i + 1) {
            assert_ne!(
                valeur, autre,
                "{nom} et {autre_nom} doivent différer, sinon un verdict juste et un verdict faux \
                 se ressembleraient"
            );
        }
    }

    // Le plafond est un critère d'acceptation écrit : « plafonné à cinq minutes ».
    assert_eq!(
        DELAI_MAXIMAL,
        Duration::from_secs(5 * 60),
        "le plafond de retente est de cinq minutes (issue #68), trouvé {DELAI_MAXIMAL:?}"
    );

    // Et l'arithmétique de l'issue le justifie : « ce qui ramène le coût à moins de 2 % ». À
    // 5,218 s de gel par retente, il faut au moins 261 s entre deux pour tenir cette promesse.
    assert!(
        COUT_DU_GEL.as_millis() * 50 <= DELAI_MAXIMAL.as_millis(),
        "une retente coûte {COUT_DU_GEL:?} de socket muet : un plafond de {DELAI_MAXIMAL:?} \
         dépasse les 2 % que l'issue promet"
    );

    // Le premier délai, lui, est encadré et non pointé — c'est un réglage, pas un critère. Deux
    // bornes seulement, et toutes deux ont un sens :
    //
    // — plus court que le gel qu'il évite, il ne serait plus un délai mais une boucle d'attente ;
    // — plus long que le huitième du plafond, et le doublement n'aurait plus la place de doubler
    //   quatre fois avant de saturer : « le délai croît » deviendrait vide de sens, et le test qui
    //   le vérifie sur quatre échecs consécutifs se ferait rattraper par le plafond sans le dire.
    assert!(
        DELAI_INITIAL >= COUT_DU_GEL,
        "un premier délai de {DELAI_INITIAL:?} est plus court que les {COUT_DU_GEL:?} de gel qu'une \
         retente coûte : ce n'est plus une quarantaine, c'est une boucle"
    );
    assert!(
        DELAI_INITIAL * 8 <= DELAI_MAXIMAL,
        "un premier délai de {DELAI_INITIAL:?} sature le plafond de {DELAI_MAXIMAL:?} en moins de \
         quatre doublements : la croissance n'aurait plus la place de se vérifier"
    );

    // Et « n'est plus relevée **au cycle suivant** » suppose que le cycle soit court devant le
    // délai. Un délai plus court qu'un cycle rendrait le critère inobservable.
    assert!(
        CYCLE * 10 <= DELAI_INITIAL,
        "un cycle de {CYCLE:?} contre un premier délai de {DELAI_INITIAL:?} : la sonde serait \
         retentée presque à chaque tour, et la quarantaine ne servirait à rien"
    );
    assert!(
        INSTANT < CYCLE,
        "le plus petit écart représentable doit tenir dans un cycle"
    );
    assert!(
        cycles_avant_la_premiere_retente() >= 10,
        "seulement {} cycle(s) tiennent dans le premier délai : les tests qui vérifient « n'est \
         plus relevée » n'auraient presque rien à observer",
        cycles_avant_la_premiere_retente()
    );
    assert!(
        CYCLE * cycles_avant_la_premiere_retente() < DELAI_INITIAL,
        "l'aide doit s'arrêter avant l'échéance, sinon les boucles qui s'en servent tomberaient \
         sur une retente due et exigeraient l'inverse du critère"
    );

    // Enfin, le calcul du délai attendu doit bien décrire la suite annoncée : « 30 s, 1 min,
    // 2 min… — plafonné à cinq minutes ». Si cette aide se trompait, les tests de croissance
    // vérifieraient la mauvaise suite.
    assert_eq!(delai_attendu(0), DELAI_INITIAL);
    assert_eq!(delai_attendu(1), DELAI_INITIAL * 2);
    assert_eq!(delai_attendu(2), DELAI_INITIAL * 4);
    assert_eq!(delai_attendu(3), DELAI_INITIAL * 8);
    assert_eq!(
        delai_attendu(30),
        DELAI_MAXIMAL,
        "l'aide doit plafonner, sinon le test des vingt échecs exigerait l'inverse du critère"
    );
    for echecs in 0..40u32 {
        assert!(
            delai_attendu(echecs) <= DELAI_MAXIMAL,
            "l'aide dépasse le plafond après {echecs} échecs"
        );
    }
}

// ---------------------------------------------------------------------------
// 1 — une sonde qui échoue n'est plus relevée au cycle suivant
// ---------------------------------------------------------------------------

#[test]
fn une_sonde_qui_echoue_n_est_plus_relevee_au_cycle_suivant() {
    // Critère d'acceptation : « une sonde qui échoue est mise en quarantaine, et **n'est plus
    // relevée** au cycle suivant ».
    //
    // C'est le critère central de #68, et le seul qui débloque vraiment le socket. Il ne se vérifie
    // pas par une égalité de sortie : une implémentation qui lirait quand même puis jetterait le
    // résultat rendrait le même verdict et gèlerait le démon exactement pareil. On compte donc les
    // **appels**, et le relevé qu'on lui donne rend un piège qu'il n'a pas le droit de voir.
    let mut quarantaine = Quarantaine::nouvelle();
    let journal = Journal::neuf();

    // Premier tour : la sonde est inconnue, elle est lue. Elle échoue.
    let verdict = exige_lue(
        &mut quarantaine,
        &journal,
        MUETTE,
        DEBUT,
        None,
        "premier tour",
    );
    assert_eq!(
        verdict,
        Releve::Muette { signaler: true },
        "la sonde vient d'échouer : elle est muette, et cette mise en quarantaine se journalise"
    );

    // Puis toute la fenêtre de quarantaine, cycle d'une seconde par cycle d'une seconde, jusqu'à
    // la veille de l'échéance : aucune retente n'y est due, donc pas une lecture.
    for i in 1..=cycles_avant_la_premiere_retente() {
        exige_muette_sans_lecture(
            &mut quarantaine,
            &journal,
            MUETTE,
            DEBUT + CYCLE * i,
            &format!("cycle n° {i} après l'échec"),
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — une sonde muette n'en invalide aucune autre
// ---------------------------------------------------------------------------

#[test]
fn une_sonde_muette_n_invalide_aucune_autre() {
    // Critère d'acceptation : « le relevé des autres sondes est **inchangé** — une sonde muette
    // n'en invalide aucune autre ».
    //
    // C'est l'autre moitié du contrat, et la faute symétrique de la précédente : une quarantaine
    // qui, pour se protéger, écarterait le tour entier. Le démon répondrait vite, et ne saurait
    // plus rien de sa machine.
    let mut quarantaine = Quarantaine::nouvelle();
    let journal = Journal::neuf();

    let vivantes = [
        (VIVANTE, TOURS),
        (CPU, TEMPERATURE_CPU),
        (DISQUE, TEMPERATURE_DISQUE),
    ];

    // Un tour de référence, tout le monde en bonne santé : c'est le « inchangé » du critère.
    let mut reference = Vec::new();
    for (slug, valeur) in vivantes {
        reference.push(exige_lue(
            &mut quarantaine,
            &journal,
            slug,
            DEBUT,
            Some(valeur),
            "tour de référence",
        ));
    }
    assert_eq!(
        reference,
        vec![
            Releve::Valeur(TOURS),
            Releve::Valeur(TEMPERATURE_CPU),
            Releve::Valeur(TEMPERATURE_DISQUE)
        ],
        "avant tout incident, chaque sonde rend sa valeur telle quelle"
    );

    // Le Kraken lâche.
    let verdict = exige_lue(
        &mut quarantaine,
        &journal,
        MUETTE,
        DEBUT,
        None,
        "l'échec du Kraken",
    );
    assert_eq!(verdict, Releve::Muette { signaler: true });

    // Toute la quarantaine durant, les trois autres rendent toujours exactement la même chose, et
    // sont toujours lues à chaque tour. C'est le mot « inchangé » du critère, au pied de la lettre.
    for i in 1..=cycles_avant_la_premiere_retente() {
        let maintenant = DEBUT + CYCLE * i;
        exige_muette_sans_lecture(
            &mut quarantaine,
            &journal,
            MUETTE,
            maintenant,
            &format!("cycle n° {i}"),
        );

        let mut tour_courant = Vec::new();
        for (slug, valeur) in vivantes {
            tour_courant.push(exige_lue(
                &mut quarantaine,
                &journal,
                slug,
                maintenant,
                Some(valeur),
                &format!("cycle n° {i}"),
            ));
        }
        assert_eq!(
            tour_courant, reference,
            "au cycle n° {i}, une sonde muette a changé ce que les autres rapportent"
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — la quarantaine est par sonde, jamais par contrôleur ni globale
// ---------------------------------------------------------------------------

#[test]
fn la_quarantaine_est_par_sonde_jamais_par_controleur_ni_globale() {
    // Critère d'acceptation : « la quarantaine est **par sonde**, jamais par contrôleur ni
    // globale ».
    //
    // Deux fautes distinctes sont visées, et il faut deux dispositifs pour les séparer :
    //
    // — la quarantaine **globale**, qui écarte tout le monde dès qu'une sonde lâche. Attrapée par
    //   les sondes des autres contrôleurs ;
    // — la quarantaine **par contrôleur**, plus tentante parce que le `slug` porte son origine en
    //   préfixe (`kraken2023elite:…`), et défendable à l'oreille : « si le Kraken ne répond plus,
    //   aucune de ses sondes ne répondra ». Elle est fausse quand même — l'issue le tranche — et
    //   seule une seconde sonde du **même** `hwmon` la met en évidence.
    let mut quarantaine = Quarantaine::nouvelle();
    let journal = Journal::neuf();

    let verdict = exige_lue(
        &mut quarantaine,
        &journal,
        MUETTE,
        DEBUT,
        None,
        "l'échec du Kraken",
    );
    assert_eq!(verdict, Releve::Muette { signaler: true });

    for i in 1..=cycles_avant_la_premiere_retente() {
        let maintenant = DEBUT + CYCLE * i;

        // La voisine, sur le même contrôleur : elle n'a rien fait, elle est lue.
        assert_eq!(
            exige_lue(
                &mut quarantaine,
                &journal,
                MEME_CONTROLEUR,
                maintenant,
                Some(TOURS),
                &format!("cycle n° {i}, sonde voisine sur le même contrôleur"),
            ),
            Releve::Valeur(TOURS),
            "« {MEME_CONTROLEUR} » partage son contrôleur avec « {MUETTE} », pas sa quarantaine"
        );

        // Et les trois autres, ailleurs.
        for (slug, valeur) in [
            (VIVANTE, TOURS),
            (CPU, TEMPERATURE_CPU),
            (DISQUE, TEMPERATURE_DISQUE),
        ] {
            assert_eq!(
                exige_lue(
                    &mut quarantaine,
                    &journal,
                    slug,
                    maintenant,
                    Some(valeur),
                    &format!("cycle n° {i}"),
                ),
                Releve::Valeur(valeur),
                "une quarantaine globale aurait aussi écarté « {slug} »"
            );
        }

        exige_muette_sans_lecture(
            &mut quarantaine,
            &journal,
            MUETTE,
            maintenant,
            &format!("cycle n° {i}"),
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — une sonde muette est rapportée muette, jamais omise ni maquillée
// ---------------------------------------------------------------------------

#[test]
fn une_sonde_muette_est_rapportee_muette_jamais_omise_ni_maquillee() {
    // Critère d'acceptation : « la sonde muette est **rapportée comme telle** sur le socket, pas
    // omise en silence : une sonde absente et une sonde muette sont deux choses différentes ».
    //
    // La faute visée est la plus coûteuse du projet parce qu'elle est **rassurante** : rendre la
    // dernière valeur connue, ou un zéro, plutôt que « je ne sais pas ». Le README le dit du
    // cadran, et c'est le même piège ici — « un 34 °C figé derrière une pompe arrêtée, c'est un
    // circuit qui chauffe sans que rien ne le signale ».
    //
    // L'omission littérale, elle, est **irreprésentable** : `tour` rend un `Releve`, qui n'a que
    // deux variantes. Une sonde absente est une sonde dont on n'appelle jamais `tour`. Ce test
    // vérifie donc ce qui reste possible — le maquillage.
    let mut quarantaine = Quarantaine::nouvelle();
    let journal = Journal::neuf();

    // Une sonde qui a d'abord bien répondu : c'est le cas où une valeur périmée traîne quelque
    // part et peut être resservie.
    assert_eq!(
        exige_lue(
            &mut quarantaine,
            &journal,
            MUETTE,
            DEBUT,
            Some(LIQUIDE),
            "la sonde répond encore",
        ),
        Releve::Valeur(LIQUIDE)
    );

    // Puis elle lâche.
    let verdict = exige_lue(
        &mut quarantaine,
        &journal,
        MUETTE,
        DEBUT + CYCLE,
        None,
        "l'échec",
    );
    assert_eq!(
        verdict,
        Releve::Muette { signaler: true },
        "une sonde qui vient d'échouer est muette, et non porteuse de sa dernière valeur"
    );
    assert_ne!(
        verdict,
        Releve::Valeur(LIQUIDE),
        "rendre {LIQUIDE} — la dernière valeur connue — laisserait croire que le liquide est \
         mesuré alors que plus rien ne le mesure"
    );
    assert_ne!(
        verdict,
        Releve::Valeur(ZERO_RASSURANT),
        "un zéro se lit comme une mesure, pas comme une absence de mesure"
    );

    // Et sur toute la durée de la quarantaine, le verdict reste le même : muette. Pas une valeur
    // périmée, pas un zéro, pas un silence.
    for i in 2..=cycles_avant_la_premiere_retente() {
        let verdict = tour(
            &mut quarantaine,
            &journal,
            MUETTE,
            DEBUT + CYCLE * i,
            Some(PIEGE),
        );
        assert!(
            matches!(verdict, Releve::Muette { .. }),
            "au cycle n° {i}, la sonde en quarantaine doit rester rapportée muette, trouvé \
             {verdict:?}"
        );
        journal.vider();
    }
}

// ---------------------------------------------------------------------------
// 5 — le délai de retente double à chaque échec
// ---------------------------------------------------------------------------

#[test]
fn le_delai_de_retente_double_a_chaque_echec() {
    // Critère d'acceptation : « elle est retentée après un délai qui **croît** à chaque échec ».
    // Approche technique de l'issue : « un délai qui double à chaque échec — 30 s, 1 min, 2 min… ».
    //
    // Le délai n'est pas un ornement : chaque retente coûte 5,218 s de socket muet. Un délai
    // constant d'une minute laisserait le démon gelé 8 % du temps — l'issue fait le calcul.
    //
    // Le délai se mesure ici par ses **bornes**, pinçées au nanoseconde près de part et d'autre :
    // muette à un instant près de l'échéance, lue à l'échéance exacte. C'est la seule façon
    // d'attraper les deux fautes symétriques — la quarantaine qui ne relâche jamais, et celle qui
    // relâche trop tôt — sans lire l'implémentation.
    let mut quarantaine = Quarantaine::nouvelle();
    let journal = Journal::neuf();

    let mut dernier_echec = DEBUT;
    let verdict = exige_lue(
        &mut quarantaine,
        &journal,
        MUETTE,
        dernier_echec,
        None,
        "le premier échec",
    );
    assert_eq!(verdict, Releve::Muette { signaler: true });

    // Quatre échecs consécutifs, donc quatre délais : 1×, 2×, 4× et 8× le délai initial. Le test
    // n° 0 garantit que le plafond ne s'en mêle pas sur cette plage.
    for echecs_precedents in 0..4u32 {
        let attendu = delai_attendu(echecs_precedents);
        assert_eq!(
            attendu,
            DELAI_INITIAL * 2u32.pow(echecs_precedents),
            "ce test vérifie la croissance, pas le plafond : sur ces quatre échecs le doublement \
             ne doit pas être rattrapé par le plafond — c'est ce que le test n° 0 garantit"
        );

        // Au tiers du délai, rien. À la moitié, rien non plus : une quarantaine dont le délai ne
        // croîtrait pas se ferait attraper dès le deuxième tour, son échéance étant déjà passée.
        for fraction in [3u32, 2] {
            exige_muette_sans_lecture(
                &mut quarantaine,
                &journal,
                MUETTE,
                dernier_echec + attendu / fraction,
                &format!(
                    "après {} échec(s), au 1/{fraction} du délai",
                    echecs_precedents + 1
                ),
            );
        }

        // Un nanoseconde avant l'échéance : toujours rien. C'est la borne par le dessous, et elle
        // interdit la quarantaine trop courte.
        exige_muette_sans_lecture(
            &mut quarantaine,
            &journal,
            MUETTE,
            dernier_echec + attendu - INSTANT,
            &format!(
                "après {} échec(s), un instant avant l'échéance de {attendu:?}",
                echecs_precedents + 1
            ),
        );

        // À l'échéance exacte : la sonde est retentée. C'est la borne par le dessus, et elle
        // interdit la quarantaine qui ne relâche jamais. La retente échoue à son tour, ce qui
        // arme le délai suivant — et le fait courir depuis **cet** échec-ci.
        dernier_echec += attendu;
        let verdict = exige_lue(
            &mut quarantaine,
            &journal,
            MUETTE,
            dernier_echec,
            None,
            &format!(
                "la retente due après {attendu:?}, soit {} échec(s) encaissés",
                echecs_precedents + 1
            ),
        );
        assert_eq!(
            verdict,
            Releve::Muette { signaler: false },
            "une retente qui échoue prolonge la quarantaine sans la rouvrir : rien de neuf à \
             journaliser"
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — le délai ne dépasse jamais son plafond, et l'atteint
// ---------------------------------------------------------------------------

#[test]
fn le_delai_de_retente_ne_depasse_jamais_son_plafond_et_l_atteint() {
    // Critère d'acceptation : « … jusqu'à un plafond », fixé à cinq minutes par l'approche
    // technique.
    //
    // Deux fautes opposées, et il faut les deux bornes pour les séparer :
    //
    // — **sans plafond**, le doublement emporte tout : après vingt échecs, une sonde ne serait
    //   retentée que dans huit siècles. Une pompe remise en marche ne serait jamais revue ;
    // — **plafond jamais atteint**, c'est-à-dire une croissance qui s'arrête trop tôt : le coût du
    //   gel resterait au-dessus des 2 % que l'issue promet.
    //
    // Vingt échecs consécutifs, largement de quoi saturer : le délai initial ne pouvant excéder le
    // huitième du plafond (test n° 0), la saturation est acquise bien avant.
    let mut quarantaine = Quarantaine::nouvelle();
    let journal = Journal::neuf();

    let mut dernier_echec = DEBUT;
    exige_lue(
        &mut quarantaine,
        &journal,
        MUETTE,
        dernier_echec,
        None,
        "le premier échec",
    );

    let mut satures = 0u32;
    for echecs_precedents in 0..20u32 {
        let attendu = delai_attendu(echecs_precedents);
        assert!(
            attendu <= DELAI_MAXIMAL,
            "le délai attendu après {echecs_precedents} échecs dépasse déjà le plafond"
        );
        if attendu == DELAI_MAXIMAL {
            satures += 1;
        }

        // Borne par le dessous : rien avant l'échéance. C'est elle qui attrape le plafond qui
        // s'appliquerait trop tôt, ou une croissance qui s'arrêterait avant lui.
        exige_muette_sans_lecture(
            &mut quarantaine,
            &journal,
            MUETTE,
            dernier_echec + attendu - INSTANT,
            &format!(
                "après {} échec(s), juste avant l'échéance",
                echecs_precedents + 1
            ),
        );

        // Borne par le dessus : à l'échéance, elle est retentée. Sans plafond, l'échéance serait
        // bien plus loin et cette lecture n'aurait pas lieu.
        dernier_echec += attendu;
        exige_lue(
            &mut quarantaine,
            &journal,
            MUETTE,
            dernier_echec,
            None,
            &format!(
                "la retente due à {attendu:?} après {} échec(s)",
                echecs_precedents + 1
            ),
        );
    }

    assert!(
        satures >= 10,
        "sur vingt échecs, le délai n'a saturé qu'à {satures} reprises : le plafond de \
         {DELAI_MAXIMAL:?} n'est pas atteint assez tôt pour tenir la promesse des 2 %"
    );

    // Et le plafond tient dans la durée : après vingt échecs, la sonde est encore retentée toutes
    // les cinq minutes, pas toutes les huit siècles.
    for retente in 0..5u32 {
        exige_muette_sans_lecture(
            &mut quarantaine,
            &journal,
            MUETTE,
            dernier_echec + DELAI_MAXIMAL - INSTANT,
            &format!("retente n° {retente} après saturation"),
        );
        dernier_echec += DELAI_MAXIMAL;
        exige_lue(
            &mut quarantaine,
            &journal,
            MUETTE,
            dernier_echec,
            None,
            &format!("retente n° {retente} après saturation"),
        );
    }
}

// ---------------------------------------------------------------------------
// 7 — une retente réussie sort de quarantaine et remet le délai à zéro
// ---------------------------------------------------------------------------

#[test]
fn une_retente_reussie_sort_de_quarantaine_et_remet_le_delai_a_zero() {
    // Critère d'acceptation : « une retente réussie sort la sonde de quarantaine et remet son
    // délai à zéro ».
    //
    // Les deux moitiés comptent, et la seconde est celle qu'on oublie : une implémentation qui
    // sortirait de quarantaine sans remettre le compteur à zéro laisserait une sonde qui clignote
    // — et un Kraken qui a lâché trois fois dans la journée en est une — écartée cinq minutes dès
    // son prochain hoquet, alors qu'elle vient de prouver qu'elle répond.
    let mut quarantaine = Quarantaine::nouvelle();
    let journal = Journal::neuf();

    // Trois échecs consécutifs : le délai est monté à 4 × DELAI_INITIAL.
    let mut dernier_echec = DEBUT;
    exige_lue(
        &mut quarantaine,
        &journal,
        MUETTE,
        dernier_echec,
        None,
        "le premier échec",
    );
    for echecs_precedents in 0..2u32 {
        dernier_echec += delai_attendu(echecs_precedents);
        exige_lue(
            &mut quarantaine,
            &journal,
            MUETTE,
            dernier_echec,
            None,
            &format!("la retente n° {}, qui échoue encore", echecs_precedents + 1),
        );
    }

    // La quatrième retente, elle, réussit.
    let guerison = dernier_echec + delai_attendu(2);
    exige_muette_sans_lecture(
        &mut quarantaine,
        &journal,
        MUETTE,
        guerison - INSTANT,
        "juste avant la retente qui guérit",
    );
    assert_eq!(
        exige_lue(
            &mut quarantaine,
            &journal,
            MUETTE,
            guerison,
            Some(LIQUIDE),
            "la retente qui réussit",
        ),
        Releve::Valeur(LIQUIDE),
        "une retente réussie rend la valeur relevée, et non un verdict de muette"
    );

    // Sortie de quarantaine : elle est relevée à chaque cycle, comme n'importe quelle sonde.
    for i in 1..=15u32 {
        assert_eq!(
            exige_lue(
                &mut quarantaine,
                &journal,
                MUETTE,
                guerison + CYCLE * i,
                Some(LIQUIDE),
                &format!("cycle n° {i} après guérison"),
            ),
            Releve::Valeur(LIQUIDE),
            "une sonde guérie est une sonde ordinaire, pas une sonde en sursis"
        );
    }

    // Et le délai est bien reparti de zéro : le prochain échec vaut DELAI_INITIAL, pas les
    // 8 × DELAI_INITIAL qu'aurait valus la suite si le compteur n'avait pas été remis.
    let rechute = guerison + CYCLE * 16;
    let verdict = exige_lue(
        &mut quarantaine,
        &journal,
        MUETTE,
        rechute,
        None,
        "la rechute",
    );
    assert_eq!(
        verdict,
        Releve::Muette { signaler: true },
        "la sonde était sortie de quarantaine : y retomber est un nouvel événement, qui se \
         journalise"
    );

    exige_muette_sans_lecture(
        &mut quarantaine,
        &journal,
        MUETTE,
        rechute + DELAI_INITIAL - INSTANT,
        "après la rechute, avant le délai initial",
    );
    exige_lue(
        &mut quarantaine,
        &journal,
        MUETTE,
        rechute + DELAI_INITIAL,
        None,
        "la retente qui suit la rechute, due après le délai INITIAL et non après le délai d'avant",
    );
}

// ---------------------------------------------------------------------------
// 8 — le journal dit la mise en quarantaine une fois, pas à chaque cycle
// ---------------------------------------------------------------------------

#[test]
fn le_journal_dit_la_mise_en_quarantaine_une_fois_pas_a_chaque_cycle() {
    // Critère d'acceptation : « le journal dit la mise en quarantaine **une fois**, pas à chaque
    // cycle ».
    //
    // « Une fois » est une propriété d'**état**, pas de sortie : c'est la décision qui doit dire
    // « c'est la première fois », sinon l'appelant n'a d'autre choix que de journaliser à chaque
    // tour — soit une ligne par seconde, pour toujours, dans un journal qu'on lit justement pour
    // trouver ce genre d'incident. D'où `signaler`.
    //
    // Le compte porte sur toute la vie de la quarantaine : l'entrée, les cycles muets, et les
    // retentes qui échouent. Une seule doit signaler.
    let mut quarantaine = Quarantaine::nouvelle();
    let journal = Journal::neuf();

    let mut signalements = 0u32;

    // L'entrée en quarantaine.
    let verdict = tour(&mut quarantaine, &journal, MUETTE, DEBUT, None);
    journal.vider();
    if matches!(verdict, Releve::Muette { signaler: true }) {
        signalements += 1;
    }
    assert_eq!(
        verdict,
        Releve::Muette { signaler: true },
        "l'entrée en quarantaine est l'événement : c'est elle qui se journalise"
    );

    // Puis on laisse tourner longtemps, cycle par seconde, avec les retentes qui tombent dedans et
    // qui échouent toutes. Une heure de démon.
    let mut dernier_echec = DEBUT;
    let mut echecs_encaisses = 1u32;
    for i in 1..=3_600u32 {
        let maintenant = DEBUT + CYCLE * i;
        let verdict = tour(&mut quarantaine, &journal, MUETTE, maintenant, None);
        journal.vider();
        if matches!(verdict, Releve::Muette { signaler: true }) {
            signalements += 1;
        }
        assert!(
            matches!(verdict, Releve::Muette { .. }),
            "au cycle n° {i}, la sonde échoue toujours : elle reste muette, trouvé {verdict:?}"
        );
        if maintenant >= dernier_echec + delai_attendu(echecs_encaisses - 1) {
            dernier_echec = maintenant;
            echecs_encaisses += 1;
        }
    }

    assert!(
        echecs_encaisses > 4,
        "l'heure simulée doit contenir plusieurs retentes, sinon le test ne vérifie que les cycles \
         muets ; {echecs_encaisses} échec(s) encaissé(s)"
    );
    assert_eq!(
        signalements, 1,
        "sur une heure de quarantaine — 3 600 cycles et {echecs_encaisses} échecs —, la mise en \
         quarantaine doit être signalée une seule fois, trouvé {signalements}"
    );

    // La guérison, puis une rechute : c'est une **nouvelle** mise en quarantaine, et elle se
    // signale. Le contraire ferait taire à jamais une sonde qui clignote, c'est-à-dire justement
    // celle dont on veut entendre parler.
    //
    // L'instant est pris un plafond entier après le dernier cycle : le délai ne dépassant jamais
    // [`DELAI_MAXIMAL`], une retente y est due à coup sûr, quel que soit le moment du dernier
    // échec. C'est plus robuste que de rejouer le calendrier des retentes ici.
    let apres = DEBUT + CYCLE * 3_600 + DELAI_MAXIMAL;
    assert_eq!(
        tour(&mut quarantaine, &journal, MUETTE, apres, Some(LIQUIDE)),
        Releve::Valeur(LIQUIDE),
        "au bout d'une heure, la retente due réussit"
    );
    journal.vider();

    let verdict = tour(&mut quarantaine, &journal, MUETTE, apres + CYCLE, None);
    journal.vider();
    assert_eq!(
        verdict,
        Releve::Muette { signaler: true },
        "une sonde guérie qui retombe entre à nouveau en quarantaine : c'est un nouvel événement"
    );

    // Et la nouvelle quarantaine se tait ensuite, comme la première.
    for i in 2..=cycles_avant_la_premiere_retente() {
        let verdict = tour(
            &mut quarantaine,
            &journal,
            MUETTE,
            apres + CYCLE * i,
            Some(PIEGE),
        );
        journal.vider();
        assert_eq!(
            verdict,
            Releve::Muette { signaler: false },
            "au cycle n° {i} de la seconde quarantaine, il n'y a rien de neuf à journaliser"
        );
    }
}

// ---------------------------------------------------------------------------
// 9 — deux sondes en quarantaine ont des délais indépendants
// ---------------------------------------------------------------------------

#[test]
fn deux_sondes_en_quarantaine_ont_des_delais_independants() {
    // Test d'intention n° 3 de l'issue : « deux sondes qui échouent sont mises de côté
    // indépendamment ».
    //
    // C'est la forme la plus serrée du critère « par sonde » : non seulement l'état est séparé,
    // mais le **délai** l'est aussi. Une implémentation qui garderait un compteur d'échecs global
    // passerait les tests d'appartenance — chaque sonde a bien son état — et se ferait attraper
    // ici : la seconde sonde, qui n'a échoué qu'une fois, hériterait du délai de la première.
    //
    // Les deux quarantaines démarrent à des instants différents et n'encaissent pas le même
    // nombre d'échecs, pour qu'aucune coïncidence ne puisse les faire passer pour indépendantes.
    let mut quarantaine = Quarantaine::nouvelle();
    let journal = Journal::neuf();

    // La première lâche à DEBUT, et rate deux retentes : son délai est monté à 4 × DELAI_INITIAL.
    let mut echec_a = DEBUT;
    exige_lue(
        &mut quarantaine,
        &journal,
        MUETTE,
        echec_a,
        None,
        "premier échec de la première sonde",
    );
    for echecs_precedents in 0..2u32 {
        echec_a += delai_attendu(echecs_precedents);
        exige_lue(
            &mut quarantaine,
            &journal,
            MUETTE,
            echec_a,
            None,
            &format!("retente n° {} de la première sonde", echecs_precedents + 1),
        );
    }
    let echeance_a = echec_a + delai_attendu(2);

    // La seconde lâche bien plus tard, une seule fois : son délai doit valoir DELAI_INITIAL, et
    // pas celui que la première a accumulé.
    let echec_b = echec_a + CYCLE * 7;
    exige_lue(
        &mut quarantaine,
        &journal,
        AUTRE_MUETTE,
        echec_b,
        None,
        "premier échec de la seconde sonde",
    );
    let echeance_b = echec_b + DELAI_INITIAL;

    assert!(
        echeance_b < echeance_a,
        "les deux échéances doivent différer pour que ce test dise quelque chose : {echeance_b:?} \
         contre {echeance_a:?}"
    );

    // À l'échéance de la seconde, elle seule est retentée. La première dort encore.
    exige_muette_sans_lecture(
        &mut quarantaine,
        &journal,
        AUTRE_MUETTE,
        echeance_b - INSTANT,
        "juste avant l'échéance de la seconde sonde",
    );
    exige_muette_sans_lecture(
        &mut quarantaine,
        &journal,
        MUETTE,
        echeance_b,
        "à l'échéance de la SECONDE sonde, la première n'est pas due",
    );
    assert_eq!(
        exige_lue(
            &mut quarantaine,
            &journal,
            AUTRE_MUETTE,
            echeance_b,
            Some(TEMPERATURE_DISQUE),
            "l'échéance de la seconde sonde",
        ),
        Releve::Valeur(TEMPERATURE_DISQUE),
        "la seconde sonde n'a échoué qu'une fois : elle est due après {DELAI_INITIAL:?}, quel que \
         soit le nombre d'échecs de la première"
    );

    // Puis à l'échéance de la première, c'est elle qui est retentée — et l'autre, guérie, est lue
    // à chaque cycle depuis un moment.
    exige_muette_sans_lecture(
        &mut quarantaine,
        &journal,
        MUETTE,
        echeance_a - INSTANT,
        "juste avant l'échéance de la première sonde",
    );
    assert_eq!(
        exige_lue(
            &mut quarantaine,
            &journal,
            AUTRE_MUETTE,
            echeance_a - INSTANT,
            Some(TEMPERATURE_DISQUE),
            "la seconde sonde, guérie",
        ),
        Releve::Valeur(TEMPERATURE_DISQUE)
    );
    assert_eq!(
        exige_lue(
            &mut quarantaine,
            &journal,
            MUETTE,
            echeance_a,
            None,
            "l'échéance de la première sonde",
        ),
        Releve::Muette { signaler: false },
        "la première sonde était due à {echeance_a:?}, indépendamment de la guérison de l'autre"
    );
}

// ---------------------------------------------------------------------------
// 10 — une sonde qui n'a jamais échoué est relevée à chaque cycle, sans délai
// ---------------------------------------------------------------------------

#[test]
fn une_sonde_qui_n_a_jamais_echoue_est_relevee_a_chaque_cycle_sans_delai() {
    // Test d'intention n° 7 de l'issue.
    //
    // C'est le test qui empêche la correction paresseuse : une quarantaine qui écarterait tout le
    // monde, ou qui n'admettrait une sonde qu'après un délai d'observation, ferait taire #68 en
    // supprimant la télémétrie. Le démon répondrait vite, et ne saurait plus rien.
    //
    // Il tient aussi le cas du **tout premier tour** : une sonde inconnue est lue tout de suite,
    // pas au bout de trente secondes.
    let mut quarantaine = Quarantaine::nouvelle();
    let journal = Journal::neuf();

    let sondes = [
        (VIVANTE, TOURS),
        (CPU, TEMPERATURE_CPU),
        (DISQUE, TEMPERATURE_DISQUE),
        (MUETTE, LIQUIDE),
        (MEME_CONTROLEUR, TOURS),
    ];

    // Deux mille cycles — largement plus que le plafond de cinq minutes, pour qu'une quarantaine
    // sournoise qui n'écarterait qu'au bout d'un moment n'ait nulle part où se cacher.
    for i in 0..2_000u32 {
        let maintenant = DEBUT + CYCLE * i;
        for (slug, valeur) in sondes {
            assert_eq!(
                exige_lue(
                    &mut quarantaine,
                    &journal,
                    slug,
                    maintenant,
                    Some(valeur),
                    &format!("cycle n° {i}"),
                ),
                Releve::Valeur(valeur),
                "« {slug} » n'a jamais échoué : au cycle n° {i}, elle doit être relevée et rendre \
                 sa valeur telle quelle"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 11 — la quarantaine ne rend jamais une valeur qu'on ne lui a pas donnée
// ---------------------------------------------------------------------------

#[test]
fn la_quarantaine_ne_rend_jamais_une_valeur_qu_on_ne_lui_a_pas_donnee() {
    // Propriété transversale, sur le modèle du test n° 7 de `spec_limiteur.rs`. Elle interdit toute
    // la famille des quarantaines « astucieuses » : celle qui lisse, celle qui garde la valeur
    // d'avant « le temps que ça revienne », celle qui rend zéro par défaut.
    //
    // Une valeur périmée servie sans étiquette est indistinguable d'une mesure, et c'est ce qui
    // rend la faute coûteuse : un cadran figé à 34 °C derrière une pompe arrêtée.
    //
    // Le scénario est irrégulier à dessein — succès, échecs, guérisons, valeurs qui changent —
    // parce qu'une quarantaine ne se trompe pas sur une suite régulière.
    let mut quarantaine = Quarantaine::nouvelle();
    let journal = Journal::neuf();

    // Chaque étape : un instant, et ce que la lecture rendrait si elle avait lieu.
    let scenario: [(Duration, Option<i32>); 10] = [
        (DEBUT, Some(LIQUIDE)),
        (DEBUT + CYCLE, Some(LIQUIDE + 1)),
        (DEBUT + CYCLE * 2, None),
        (DEBUT + CYCLE * 3, Some(PIEGE)),
        (DEBUT + CYCLE * 3 + DELAI_INITIAL, Some(LIQUIDE + 2)),
        (DEBUT + CYCLE * 4 + DELAI_INITIAL, Some(LIQUIDE + 3)),
        (DEBUT + CYCLE * 5 + DELAI_INITIAL, None),
        (DEBUT + CYCLE * 6 + DELAI_INITIAL, Some(PIEGE)),
        (DEBUT + CYCLE * 6 + DELAI_INITIAL * 2, Some(LIQUIDE + 4)),
        (DEBUT + CYCLE * 7 + DELAI_INITIAL * 2, Some(LIQUIDE + 5)),
    ];

    for (i, (maintenant, reponse)) in scenario.into_iter().enumerate() {
        let verdict = tour(&mut quarantaine, &journal, MUETTE, maintenant, reponse);
        let lue = !journal.vider().is_empty();

        match verdict {
            Releve::Valeur(valeur) => {
                assert!(
                    lue,
                    "étape {i} : une valeur ne peut venir que d'une lecture qui a eu lieu, et \
                     aucune n'a eu lieu"
                );
                assert_eq!(
                    Some(valeur),
                    reponse,
                    "étape {i} : la quarantaine rend ce que la lecture vient de donner, jamais une \
                     valeur d'avant ni une valeur inventée"
                );
            }
            Releve::Muette { .. } => {
                // Une muette qui a été lue est une lecture qui vient d'échouer : la réponse était
                // `None`. Une muette qui n'a pas été lue est une sonde en quarantaine.
                if lue {
                    assert_eq!(
                        reponse, None,
                        "étape {i} : la lecture a rendu {reponse:?} et la sonde est déclarée \
                         muette — une valeur relevée ne se jette pas"
                    );
                }
            }
        }
    }
}
