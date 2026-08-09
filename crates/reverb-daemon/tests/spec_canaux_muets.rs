//! Tests d'intention de la quarantaine des **canaux de ventilation** (issue #88).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #88 seule. Ni
//! `crates/reverb-daemon/src/telemetrie.rs` ni `crates/reverb-daemon/src/quarantaine.rs` — les deux
//! fichiers que la correction va toucher — n'ont été ouverts pour les produire ; seuls l'ont été le
//! fichier de tests d'intention de #68 (`spec_quarantaine.rs`), les signatures publiques de
//! `reverb_hw::hwmon` et la définition de `ResponseLine` dans `reverb_proto::ipc`. À l'écriture de ce
//! fichier, `telemetrie::releve_canaux` **n'existe pas** : la compilation doit échouer, et c'est la
//! phase rouge.
//!
//! Rien ici n'ouvre de fichier, ne lit un `hwmon`, ni ne dort. Le temps est **injecté**, comme dans
//! #68 : chaque tour reçoit son instant. Un test qui appellerait `Instant::now()` mesurerait
//! l'ordonnanceur de la machine, et vérifier un plafond de cinq minutes en attendant cinq minutes
//! n'est pas une option. **Aucun `sleep`, nulle part.**
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Mesuré sur SHYNAEL le 2026-08-09, Kraken en rade :
//!
//! ```text
//! $ status
//! 36.306 s · 811 octets
//! 30.708 s · 811 octets        ← reproductible, pas un hoquet
//! unreadable kraken2023elite:fan-speed:mode  Connection timed out (os error 110)
//! unreadable kraken2023elite:pump-speed:mode Connection timed out (os error 110)
//! ```
//!
//! `telemetrie::releve` fait passer les **sondes de température** par la quarantaine de #68, mais pas
//! les **canaux de ventilation**, lus juste au-dessus. Chaque attribut sysfs d'un canal muet coûte
//! ses cinq secondes en sommeil non interruptible (#68 : « 5,218 total » pour un `cat`).
//!
//! ⚠️ **Le défaut n'est pas que le canal échoue, c'est qu'il emporte tout le reste avec lui.** La
//! fenêtre interroge `status` une fois par seconde et le fil de rendu sert le socket : `geometry`,
//! qui ne touche aucun matériel, a mis **10,2 s** à répondre simplement parce qu'un `status` le
//! précédait.
//!
//! C'est la même maladie que #68 (les sondes) et #83 (la dalle), sur le troisième et dernier chemin
//! qui la porte encore.
//!
//! ## La couture que ces tests exigent, et pourquoi celle-là
//!
//! L'issue tranche le mécanisme — « étendre la `Quarantaine` de #68 aux canaux plutôt qu'en écrire
//! une seconde » — mais pas la forme. Ce fichier la tranche, et voici ce qu'il faut implémenter :
//!
//! ```ignore
//! // crates/reverb-daemon/src/telemetrie.rs
//!
//! use std::time::Duration;
//!
//! use reverb_hw::hwmon::FanChannel;
//! use reverb_proto::Position;
//! use reverb_proto::ipc::ResponseLine;
//!
//! use crate::quarantaine::Quarantaine;
//!
//! /// Ce qu'une lecture réussie d'un canal rend : tout ce que la ligne `chan` porte,
//! /// hormis le nom du canal, qui est déjà connu.
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub struct LectureCanal {
//!     pub position: Option<Position>,
//!     pub rpm: Option<u32>,
//!     pub pwm: Option<u8>,
//!     pub mode: String,
//!     pub sait_faire_auto: bool,
//! }
//!
//! /// Ce qu'un tour de relevé des canaux a produit.
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub struct TourCanaux {
//!     /// **Une ligne par canal reçu, dans le même ordre** : `Channel` s'il a répondu,
//!     /// `Unreadable` sinon. Jamais moins, jamais plus.
//!     pub lignes: Vec<ResponseLine>,
//!     /// Les canaux qui viennent d'être mis à l'écart — une fois par mise à l'écart,
//!     /// jamais à chaque tour. C'est ce que le journal dit.
//!     pub a_signaler: Vec<String>,
//! }
//!
//! pub fn releve_canaux(
//!     canaux: &[FanChannel],
//!     quarantaine: &mut Quarantaine,
//!     maintenant: Duration,
//!     lire: impl FnMut(&FanChannel) -> Result<LectureCanal, String>,
//! ) -> TourCanaux;
//! ```
//!
//! Quatre choix, et ce qu'ils achètent :
//!
//! 1. **La lecture arrive en fermeture, elle ne précède pas la décision.** C'est la leçon de #68 :
//!    un `faut_il_relever(canal, maintenant) -> bool` aurait laissé « n'est plus lu » à la politesse
//!    de l'appelant, et une `telemetrie` qui lirait sysfs puis consulterait la décision passerait
//!    tous les tests d'égalité en gelant le démon exactement comme avant. En prenant la fermeture,
//!    `releve_canaux` **est** l'endroit où la lecture a lieu ou n'a pas lieu, et un test qui compte
//!    les appels le vérifie.
//! 2. **Une seule fermeture par canal, et elle rend un `Result`.** L'issue le dit : « un canal porte
//!    plusieurs attributs (`rpm`, `pwm`, `mode`) là où une sonde n'en a qu'un. La clef d'écartement
//!    est le canal entier, pas l'attribut ». Une fermeture par canal rend cette clef **structurelle**
//!    plutôt que disciplinaire, et le `?` qui la parcourt fait qu'apprendre le silence coûte une
//!    lecture — 5,218 s — au lieu de trois.
//! 3. **`Result<_, String>` et non `Option`, contrairement à #68.** La décision de #68 ne fait rien
//!    de la cause, et c'est juste pour une sonde : sa ligne `temp` disparaît, point. Un canal, lui,
//!    est rendu par une ligne `unreadable <sujet> <raison>` dont la raison est **le seul diagnostic
//!    que l'opérateur reçoit** — « Connection timed out (os error 110) » est ce qui a permis
//!    d'écrire cette issue. La cause traverse donc la couture, et la quarantaine la retient : c'est
//!    précisément en quoi elle est **étendue** plutôt que dupliquée.
//! 4. **`TourCanaux.lignes` porte des `ResponseLine`, pas un type intermédiaire.** Le critère « un
//!    canal muet est **dit** `unreadable`, jamais omis » se vérifie alors sur ce qui sort vraiment du
//!    socket, et le piège que le parent nomme — un correctif rapide parce qu'il aurait débranché le
//!    panneau VENTILOS — devient impossible à faire passer.
//!
//! ## Ce que ce fichier fige
//!
//! 1. **Un canal qui se tait n'est plus lu**, et ça se mesure en **comptant les appels**, pas en
//!    comparant des sorties. Une implémentation qui lirait quand même puis jetterait le résultat
//!    rendrait exactement la même chose et gèlerait le socket tout pareil.
//! 2. **Apprendre le silence coûte une lecture, pas trois** — la clef est le canal, pas l'attribut.
//! 3. **Les canaux vivants sont toujours là, avec leurs valeurs**, et dans l'ordre reçu. C'est le
//!    contrepoids indispensable : sans lui, la série entière serait verte sur une `telemetrie` qui
//!    aurait cessé de rendre le moindre canal.
//! 4. **L'écart est par canal** : le voisin du même contrôleur continue d'être lu, et deux canaux
//!    muets ont chacun leur échéance.
//! 5. **Le canal écarté est rendu `unreadable` avec sa raison**, tour après tour, jamais omis et
//!    jamais maquillé en 0 tr/min — la ligne le dit déjà : « un canal illisible affiché à 0 tr/min
//!    est un mensonge, et un canal omis fait croire qu'il n'existe pas ».
//! 6. **Le délai double, plafonne, et ne condamne jamais** : un canal écarté est retenté jusqu'à ce
//!    qu'il revienne, et une retente réussie le remet en service **et** remet le délai à zéro.
//! 7. **Le journal parle une fois par mise à l'écart**, pas à chaque tour — propriété d'état, portée
//!    par `a_signaler`, et non discipline de l'appelant.
//!
//! ## Ce que le contrat laisse ouvert, et que ces tests tranchent
//!
//! 1. **Le `subject` d'une ligne `unreadable` de canal est le nom du canal, exactement.** Les lignes
//!    observées dans l'issue nomment un attribut (`…:fan-speed:mode`) parce qu'aujourd'hui chaque
//!    attribut est lu et échoue pour son compte. Une fois la clef d'écartement portée par le canal
//!    entier, un sujet qui nommerait `:mode` laisserait croire que `rpm` et `pwm`, eux, ont été lus
//!    et vont bien. C'est faux, et ça rend aussi indécidable la correspondance un canal ↔ une ligne.
//! 2. **La `reason` **contient** ce que la lecture a dit.** `contains` et non l'égalité : le démon a
//!    le droit d'y ajouter qu'il a écarté le canal et pour combien de temps. Ce qu'il n'a pas le
//!    droit de faire, c'est de perdre « Connection timed out (os error 110) » au deuxième tour — sans
//!    quoi le diagnostic ne survit pas à la seconde où il apparaît.
//! 3. **Un canal jamais vu est lu sans délai.** La quarantaine est une punition, pas un droit
//!    d'entrée. Sans cette règle, une implémentation qui écarterait tout le monde au démarrage
//!    passerait tout le reste du fichier.
//! 4. **La retente a lieu à l'instant exact de l'échéance, pas après** : la comparaison est un `>=`,
//!    et les bornes sont pincées au nanoseconde près de part et d'autre. C'est la seule façon
//!    d'attraper les deux fautes symétriques — la quarantaine qui ne relâche jamais et celle qui
//!    relâche tout de suite.
//! 5. **Un échec de retente ne se signale pas**, mais une rechute après guérison, si : c'est une
//!    **nouvelle** mise à l'écart. Le contraire ferait taire à jamais un canal qui clignote,
//!    c'est-à-dire justement celui dont on veut entendre parler.
//! 6. **Les 100 ms se vérifient sans horloge**, en comptant les lectures qui n'ont pas lieu. Une
//!    lecture qui n'a pas lieu ne bloque pas : c'est le seul modèle de coût qui ne dépende pas de la
//!    charge de la machine de test.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **La lecture sysfs elle-même.** Elle est de l'autre côté de la fermeture, et c'est tout
//!   l'intérêt de la couture : aucune écriture ni lecture matérielle dans un test automatisé.
//! - **Le relevé des sondes.** Il a son fichier, `spec_quarantaine.rs`, et sa quarantaine est la
//!   même — la vérifier deux fois ne prouverait rien de plus.
//! - **Réveiller un contrôleur muet** (`unbind`/`bind` USB) : hors scope, comme dans #68.
//! - **Le fil d'exécution séparé.** #83 l'a fait pour la dalle ; #88 ne le refait pas pour les
//!   canaux, parce qu'un canal qu'on ne lit pas ne bloque personne.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use reverb_daemon::quarantaine::{DELAI_INITIAL, DELAI_MAXIMAL, Quarantaine};
use reverb_daemon::telemetrie::{LectureCanal, TourCanaux, releve_canaux};
use reverb_hw::hwmon::{self, FanChannel, Mode};
use reverb_proto::Position;
use reverb_proto::ipc::{ResponseLine, encode_response_line, parse_response_line};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// L'origine du temps injecté.
///
/// Volontairement loin de zéro : le démon tourne depuis un moment quand un contrôleur lâche. Une
/// quarantaine qui prendrait `Duration::ZERO` pour « ce canal n'a jamais échoué » se ferait attraper
/// ici plutôt qu'à l'usage.
const DEBUT: Duration = Duration::from_secs(3_600);

/// Le plus petit écart que le temps injecté sache représenter.
const INSTANT: Duration = Duration::from_nanos(1);

/// La cadence des `status` : « la fenêtre interroge `status` une fois par seconde » (issue #88).
const CYCLE: Duration = Duration::from_secs(1);

/// Ce que coûte **une** lecture qui bute sur un contrôleur muet : cinq secondes en sommeil non
/// interruptible. Mesuré dans #68 (« 5,218 total » pour un `cat` sur `temp1_input`), et c'est la
/// même attente que #88 relève sur les attributs d'un canal.
const COUT_D_UNE_LECTURE_MUETTE: Duration = Duration::from_millis(5_218);

/// Ce que `status` doit tenir, critère d'acceptation de #88.
const BUDGET_STATUS: Duration = Duration::from_millis(100);

/// Ce que `status` coûtait, mesuré sur SHYNAEL le 2026-08-09.
const COUT_OBSERVE: Duration = Duration::from_millis(36_306);

/// Et sa reproduction, à l'appel suivant : « reproductible, pas un hoquet ».
const COUT_OBSERVE_REPRODUIT: Duration = Duration::from_millis(30_708);

/// Ce que `geometry` — qui ne touche aucun matériel — a mis à répondre, pour la seule raison qu'un
/// `status` le précédait dans le fil qui sert le socket.
const ATTENTE_DE_GEOMETRY: Duration = Duration::from_millis(10_200);

/// Le pilote du Kraken, tel que `hwmon` le nomme.
const KRAKEN: &str = "kraken2023elite";

/// Le pilote du contrôleur de ventilation NZXT.
const SMART2: &str = "nzxtsmart2";

/// Un pilote de carte mère, qui n'expose pas `pwmN_enable`.
const CARTE_MERE: &str = "nct6687";

/// Le canal qui se tait : celui de l'issue, nommé comme la ligne `unreadable` observée.
const MUET: &str = "kraken2023elite:fan-speed";

/// Son voisin, **sur le même contrôleur**.
///
/// Il n'existe que pour un critère : « l'écart est par canal ». Le nom d'un canal porte son pilote
/// en préfixe, donc une implémentation qui écarterait par contrôleur aurait de quoi le faire — et
/// c'est exactement ce qu'on interdit. Dans l'incident observé les deux se taisaient ensemble ;
/// l'issue exige qu'ils puissent ne pas le faire.
const VOISIN: &str = "kraken2023elite:pump-speed";

/// Trois canaux qui répondent, sur deux autres pilotes.
const VENTILO_1: &str = "nzxtsmart2:fan-1";
const VENTILO_2: &str = "nzxtsmart2:fan-2";
const BOITIER: &str = "nct6687:fan-3";

/// Le banc, dans l'ordre où le démon les découvre — c'est aussi l'ordre attendu des lignes.
const TOUS: [&str; 5] = [MUET, VOISIN, VENTILO_1, VENTILO_2, BOITIER];

/// La raison rendue par le contrôleur en rade, recopiée telle quelle de l'issue.
///
/// ⚠️ Elle **porte des espaces** : c'est le dernier champ de la ligne `unreadable`, et un
/// aller-retour par le protocole le vérifie.
const RAISON: &str = "Connection timed out (os error 110)";

/// Une seconde raison, distincte, pour que deux canaux muets ne puissent pas échanger la leur.
const AUTRE_RAISON: &str = "No such device (os error 19)";

/// Ce qu'une lecture rendrait si elle avait lieu alors que le canal est écarté.
///
/// C'est un piège, et il attrape deux fautes d'un coup : une implémentation qui lit quand même se
/// voit au compte d'appels, **et** au verdict si elle rend ce qu'elle vient de lire.
fn piege() -> LectureCanal {
    LectureCanal {
        position: Some(Position::Arriere),
        rpm: Some(66_666),
        pwm: Some(66),
        mode: "piège".to_owned(),
        sait_faire_auto: true,
    }
}

// ---------------------------------------------------------------------------
// Le banc : cinq canaux, aucun fichier
// ---------------------------------------------------------------------------

/// Fabrique un canal tel que la découverte le rendrait.
///
/// ⚠️ **Aucun de ces chemins n'est jamais ouvert.** Ils sont là parce que `FanChannel` les porte ;
/// la lecture, elle, passe par la fermeture que le test fournit. C'est toute la raison d'être de la
/// couture : « aucune écriture matérielle dans les tests automatisés » (CLAUDE.md).
fn canal(source: &str, libelle: &str, index: u32) -> FanChannel {
    let racine = format!("/sys/class/hwmon/hwmon-{source}");
    FanChannel {
        source: source.to_owned(),
        label: libelle.to_owned(),
        name: format!("{source}:{}", hwmon::slug(libelle)),
        index,
        pwm: PathBuf::from(format!("{racine}/pwm{index}")),
        tach: Some(PathBuf::from(format!("{racine}/fan{index}_input"))),
        enable: if source == CARTE_MERE {
            // « absent si la source n'expose pas de mode — c'est le cas de `nct6687` »
            // (`reverb_hw::hwmon::FanChannel`).
            None
        } else {
            Some(PathBuf::from(format!("{racine}/pwm{index}_enable")))
        },
        curve: Vec::new(),
    }
}

/// Les cinq canaux du banc, dans l'ordre de [`TOUS`].
fn banc() -> Vec<FanChannel> {
    vec![
        canal(KRAKEN, "Fan speed", 1),
        canal(KRAKEN, "Pump speed", 2),
        canal(SMART2, "Fan 1", 1),
        canal(SMART2, "Fan 2", 2),
        canal(CARTE_MERE, "Fan 3", 3),
    ]
}

/// Ce que chaque canal rend quand il va bien.
///
/// Des valeurs toutes distinctes : un verdict juste et un verdict recopié du voisin doivent se
/// distinguer.
fn en_bonne_sante(nom: &str) -> LectureCanal {
    match nom {
        MUET => LectureCanal {
            position: Some(Position::RadiateurHaut),
            rpm: Some(1_089),
            pwm: Some(40),
            mode: Mode::HostCurve.to_string(),
            sait_faire_auto: true,
        },
        VOISIN => LectureCanal {
            position: None,
            rpm: Some(2_412),
            pwm: Some(60),
            mode: Mode::HostCurve.to_string(),
            sait_faire_auto: true,
        },
        VENTILO_1 => LectureCanal {
            position: Some(Position::BasGauche),
            rpm: Some(716),
            pwm: Some(35),
            mode: Mode::Manual.to_string(),
            sait_faire_auto: false,
        },
        VENTILO_2 => LectureCanal {
            position: Some(Position::BasMilieu),
            rpm: Some(731),
            pwm: Some(36),
            mode: Mode::Manual.to_string(),
            sait_faire_auto: false,
        },
        BOITIER => LectureCanal {
            position: None,
            rpm: Some(918),
            pwm: Some(50),
            mode: Mode::Unsupported.to_string(),
            sait_faire_auto: false,
        },
        autre => panic!("« {autre} » n'est pas un canal du banc"),
    }
}

/// La ligne `chan` qu'un canal en bonne santé doit produire.
fn ligne_attendue(nom: &str, lecture: &LectureCanal) -> ResponseLine {
    ResponseLine::Channel {
        channel: nom.to_owned(),
        position: lecture.position,
        rpm: lecture.rpm,
        pwm: lecture.pwm,
        mode: lecture.mode.clone(),
        sait_faire_auto: lecture.sait_faire_auto,
    }
}

// ---------------------------------------------------------------------------
// Le bureau : ce que la lecture rendrait, et ce qu'elle a vraiment lu
// ---------------------------------------------------------------------------

/// Ce que la lecture rend pour chaque canal, et les canaux réellement lus.
///
/// Compter les appels et non seulement comparer les sorties : une implémentation qui lirait le canal
/// muet puis jetterait le résultat rendrait exactement le bon verdict, et gèlerait le socket cinq
/// secondes par `status` — c'est-à-dire précisément le défaut de #88, corrigé nulle part.
struct Bureau {
    reponses: RefCell<HashMap<String, Result<LectureCanal, String>>>,
    lectures: RefCell<Vec<String>>,
}

impl Bureau {
    /// Un bureau où les cinq canaux du banc répondent.
    fn neuf() -> Bureau {
        let bureau = Bureau {
            reponses: RefCell::new(HashMap::new()),
            lectures: RefCell::new(Vec::new()),
        };
        for nom in TOUS {
            bureau.repond(nom, en_bonne_sante(nom));
        }
        bureau
    }

    fn repond(&self, nom: &str, lecture: LectureCanal) {
        self.reponses
            .borrow_mut()
            .insert(nom.to_owned(), Ok(lecture));
    }

    /// Le canal se tait : sa lecture échoue, et c'est celle-là qui coûte 5,218 s au démon.
    fn se_tait(&self, nom: &str, raison: &str) {
        self.reponses
            .borrow_mut()
            .insert(nom.to_owned(), Err(raison.to_owned()));
    }

    /// Le canal est écarté : s'il est lu quand même, il rend le piège — et ça se voit.
    fn tend_le_piege(&self, nom: &str) {
        self.repond(nom, piege());
    }

    fn lire(&self, canal: &FanChannel) -> Result<LectureCanal, String> {
        self.lectures.borrow_mut().push(canal.name.clone());
        self.reponses
            .borrow()
            .get(&canal.name)
            .cloned()
            .unwrap_or_else(|| panic!("aucune réponse posée pour « {} »", canal.name))
    }

    /// Rend les canaux lus depuis la dernière vidange, dans l'ordre, et repart à vide.
    fn lectures(&self) -> Vec<String> {
        self.lectures.borrow_mut().drain(..).collect()
    }
}

/// Un tour de relevé, à l'instant donné.
fn tour(
    canaux: &[FanChannel],
    quarantaine: &mut Quarantaine,
    bureau: &Bureau,
    maintenant: Duration,
) -> TourCanaux {
    releve_canaux(canaux, quarantaine, maintenant, |c| bureau.lire(c))
}

// ---------------------------------------------------------------------------
// Aides d'assertion
// ---------------------------------------------------------------------------

/// Le canal qu'une ligne concerne, quelle que soit sa nature.
///
/// Le `panic!` final est une assertion à part entière : un tour de canaux ne produit que des
/// `chan` et des `unreadable`.
fn sujet(ligne: &ResponseLine) -> &str {
    match ligne {
        ResponseLine::Channel { channel, .. } => channel.as_str(),
        ResponseLine::Unreadable { subject, .. } => subject.as_str(),
        autre => {
            panic!("un tour de canaux ne produit que « chan » et « unreadable », trouvé {autre:?}")
        }
    }
}

/// Ce canal a-t-il été lu pendant le tour dont on vient de vider le compteur ?
fn a_ete_lu(lectures: &[String], nom: &str) -> bool {
    lectures.iter().any(|lu| lu.as_str() == nom)
}

/// **Une ligne par canal, dans l'ordre reçu, ni plus ni moins.**
///
/// C'est l'invariant qui interdit l'omission : « un canal absent et un canal muet sont deux choses
/// différentes » (issue #88), et un canal omis fait croire qu'il n'existe pas.
fn exige_une_ligne_par_canal(rendu: &TourCanaux, contexte: &str) {
    let sujets: Vec<&str> = rendu.lignes.iter().map(sujet).collect();
    assert_eq!(
        sujets,
        TOUS.to_vec(),
        "{contexte} : le tour doit rendre exactement une ligne par canal, dans l'ordre où les canaux \
         lui ont été donnés — un canal omis se lit comme un canal qui n'existe pas"
    );
}

/// La ligne d'un canal, ou un échec qui nomme ce qui manque.
fn ligne_de<'a>(rendu: &'a TourCanaux, nom: &str, contexte: &str) -> &'a ResponseLine {
    rendu
        .lignes
        .iter()
        .find(|ligne| sujet(ligne) == nom)
        .unwrap_or_else(|| {
            panic!(
                "{contexte} : aucune ligne pour « {nom} » dans {:?}",
                rendu.lignes
            )
        })
}

/// Exige qu'un canal soit rendu vivant, **avec ses valeurs**.
///
/// Sans cette exigence, toute la série passerait sur une `telemetrie` qui aurait simplement cessé de
/// rendre des canaux : `status` serait rapide, et le panneau VENTILOS vide.
fn exige_vivant(rendu: &TourCanaux, nom: &str, contexte: &str) {
    let attendue = ligne_attendue(nom, &en_bonne_sante(nom));
    assert_eq!(
        *ligne_de(rendu, nom, contexte),
        attendue,
        "{contexte} : « {nom} » répond, sa ligne doit porter ses valeurs telles que la lecture les a \
         rendues"
    );
}

/// Exige qu'un canal soit rendu `unreadable`, avec sa raison.
fn exige_illisible(rendu: &TourCanaux, nom: &str, raison_attendue: &str, contexte: &str) {
    let ligne = ligne_de(rendu, nom, contexte);
    let ResponseLine::Unreadable { subject, reason } = ligne else {
        panic!(
            "{contexte} : « {nom} » ne répond pas, il doit être rendu « unreadable » et non {ligne:?} \
             — un canal muet affiché à 0 tr/min est un mensonge"
        );
    };
    assert_eq!(
        subject.as_str(),
        nom,
        "{contexte} : le sujet d'une ligne « unreadable » de canal est le canal entier. Nommer un \
         attribut — « {nom}:mode », comme les lignes observées dans l'issue — laisserait croire que \
         les autres attributs, eux, ont été lus et vont bien"
    );
    assert!(
        reason.contains(raison_attendue),
        "{contexte} : la raison rendue pour « {nom} » doit contenir « {raison_attendue} », c'est le \
         seul diagnostic que l'opérateur reçoit ; trouvé « {reason} »"
    );
    assert!(
        !reason.trim().is_empty(),
        "{contexte} : une raison vide rendrait la ligne « unreadable » impossible à relire — le \
         protocole coupe le sujet de la raison au premier espace"
    );
}

/// Exige que ces canaux, et eux seuls, aient été lus pendant le tour qui vient.
///
/// Vide le compteur au passage.
fn exige_lectures(bureau: &Bureau, attendues: &[&str], contexte: &str) {
    let faites = bureau.lectures();
    assert_eq!(
        faites,
        attendues
            .iter()
            .map(|n| (*n).to_owned())
            .collect::<Vec<String>>(),
        "{contexte} : les lectures effectives ne sont pas celles attendues — chaque lecture de trop \
         sur un contrôleur en rade coûte {COUT_D_UNE_LECTURE_MUETTE:?} au fil qui sert le socket"
    );
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

/// Le nombre de `status` d'une seconde qui tiennent dans le premier délai **sans** qu'aucune retente
/// ne soit due.
fn cycles_avant_la_premiere_retente() -> u32 {
    u32::try_from(DELAI_INITIAL.as_millis() / CYCLE.as_millis()).unwrap_or(u32::MAX) - 1
}

/// Le coût modélisé d'un `status`, à partir des canaux qu'il a réellement lus.
///
/// **C'est ainsi qu'on vérifie les 100 ms sans horloge** : une lecture qui n'a pas lieu ne bloque
/// pas, et une lecture qui bute sur un contrôleur en rade coûte ses cinq secondes. Mesurer une durée
/// réelle mesurerait la charge de la machine de test, pas la correction.
fn cout_modelise(lectures: &[String], muets: &[&str]) -> Duration {
    let bloquantes = lectures
        .iter()
        .filter(|nom| muets.contains(&nom.as_str()))
        .count();
    COUT_D_UNE_LECTURE_MUETTE * u32::try_from(bloquantes).unwrap_or(u32::MAX)
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tous les tests qui suivent supposent que les noms de canaux diffèrent, que deux d'entre eux
    // partagent leur contrôleur, que les valeurs diffèrent, et que les délais sont assez espacés
    // pour que « n'est plus lu au tour suivant » veuille dire quelque chose. Si l'un de ces repères
    // se dégradait, plusieurs tests deviendraient vrais sans rien vérifier — et personne ne le
    // verrait. Ce test est là pour que la panne soit ici.

    // Les noms du banc sont ceux que la découverte fabrique. S'ils divergeaient, les constantes
    // recopiées de l'issue désigneraient des canaux qui n'existent pas.
    let banc = banc();
    let noms: Vec<&str> = banc.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        noms,
        TOUS.to_vec(),
        "les noms fabriqués par le banc doivent être ceux que l'issue a observés sur le socket"
    );

    for (i, nom) in TOUS.iter().enumerate() {
        assert!(!nom.is_empty(), "un canal doit porter un nom");
        for autre in TOUS.iter().skip(i + 1) {
            assert_ne!(
                nom, autre,
                "deux canaux du banc portent le même nom : les tests d'indépendance ne testeraient \
                 rien"
            );
        }
    }

    // Le voisin vient bien du même pilote que le canal muet : sans ça, le critère « l'écart est par
    // canal, jamais par contrôleur » serait vérifié par une coïncidence de nommage.
    let pilote = |nom: &str| nom.split(':').next().unwrap_or_default().to_owned();
    assert_eq!(
        pilote(MUET),
        pilote(VOISIN),
        "VOISIN doit venir du même pilote que MUET, sinon rien ne distingue un écart par canal d'un \
         écart par contrôleur"
    );
    assert_ne!(pilote(MUET), pilote(VENTILO_1));

    // Les valeurs de bonne santé sont toutes distinctes, et aucune ne ressemble au piège.
    let lectures = TOUS.map(en_bonne_sante);
    for (i, lecture) in lectures.iter().enumerate() {
        assert_ne!(
            *lecture,
            piege(),
            "la lecture de bonne santé de « {} » se confond avec le piège",
            TOUS[i]
        );
        for autre in lectures.iter().skip(i + 1) {
            assert_ne!(
                lecture, autre,
                "deux canaux du banc rendraient la même chose : une ligne recopiée du voisin passerait"
            );
        }
    }

    // ⚠️ Le mode est un jeton, sans espace : c'est l'arité de la ligne `chan` qui dit si le drapeau
    // « sait faire auto » la suit (#50). Un mode à espaces la rendrait indécidable.
    for lecture in &lectures {
        assert!(
            !lecture.mode.contains(' '),
            "un mode s'écrit en un seul jeton, trouvé « {} »",
            lecture.mode
        );
    }

    // Les deux raisons diffèrent, et portent des espaces — c'est le dernier champ de la ligne.
    assert_ne!(RAISON, AUTRE_RAISON);
    assert!(
        RAISON.contains(' '),
        "la raison observée porte des espaces : c'est ce qui rend l'aller-retour par le protocole \
         digne d'intérêt"
    );

    // Le plafond de retente est celui de #68, que #88 réemploie sans le redéfinir.
    assert_eq!(
        DELAI_MAXIMAL,
        Duration::from_secs(5 * 60),
        "le plafond de retente est de cinq minutes, trouvé {DELAI_MAXIMAL:?}"
    );
    assert!(
        DELAI_INITIAL >= COUT_D_UNE_LECTURE_MUETTE,
        "un premier délai de {DELAI_INITIAL:?} est plus court que les \
         {COUT_D_UNE_LECTURE_MUETTE:?} de gel qu'une retente coûte : ce n'est plus une quarantaine, \
         c'est une boucle"
    );
    assert!(
        DELAI_INITIAL * 8 <= DELAI_MAXIMAL,
        "un premier délai de {DELAI_INITIAL:?} sature le plafond de {DELAI_MAXIMAL:?} en moins de \
         quatre doublements : la croissance n'aurait plus la place de se vérifier"
    );
    assert!(
        CYCLE * 10 <= DELAI_INITIAL,
        "un `status` toutes les {CYCLE:?} contre un premier délai de {DELAI_INITIAL:?} : le canal \
         serait retenté presque à chaque tour, et l'écart ne servirait à rien"
    );
    assert!(INSTANT < CYCLE);
    assert!(
        cycles_avant_la_premiere_retente() >= 10,
        "seulement {} `status` tiennent dans le premier délai : les tests qui vérifient « n'est plus \
         lu » n'auraient presque rien à observer",
        cycles_avant_la_premiere_retente()
    );
    assert!(CYCLE * cycles_avant_la_premiere_retente() < DELAI_INITIAL);

    // La suite des délais est bien celle de #68 — « 30 s, 1 min, 2 min… — plafonné à cinq minutes ».
    assert_eq!(delai_attendu(0), DELAI_INITIAL);
    assert_eq!(delai_attendu(1), DELAI_INITIAL * 2);
    assert_eq!(delai_attendu(2), DELAI_INITIAL * 4);
    assert_eq!(delai_attendu(3), DELAI_INITIAL * 8);
    assert_eq!(delai_attendu(30), DELAI_MAXIMAL);
    for echecs in 0..40u32 {
        assert!(delai_attendu(echecs) <= DELAI_MAXIMAL);
    }

    // Le modèle de coût, enfin : il doit décrire ce qui a été mesuré. Sept lectures bloquantes,
    // c'est bien l'ordre de grandeur des 36,306 s observées — deux canaux muets, trois attributs
    // chacun, plus une sonde. C'est ce que #88 doit ramener à zéro.
    assert_eq!(cout_modelise(&[], &[MUET]), Duration::ZERO);
    assert_eq!(
        cout_modelise(&[MUET.to_owned()], &[MUET]),
        COUT_D_UNE_LECTURE_MUETTE
    );
    assert_eq!(
        cout_modelise(&[VENTILO_1.to_owned()], &[MUET]),
        Duration::ZERO,
        "un canal qui répond ne coûte rien de mesurable : « 0,001 total » pour un `cat` sur \
         `fan1_input` de nzxtsmart2 (#68)"
    );
    assert!(
        COUT_D_UNE_LECTURE_MUETTE * 7 >= COUT_OBSERVE_REPRODUIT,
        "le modèle doit expliquer les {COUT_OBSERVE_REPRODUIT:?} mesurées, sinon il ne mesure pas le \
         défaut de #88"
    );
    assert!(BUDGET_STATUS < COUT_D_UNE_LECTURE_MUETTE);
    assert!(ATTENTE_DE_GEOMETRY < COUT_OBSERVE);
}

// ---------------------------------------------------------------------------
// 1 — un canal qui se tait est écarté au premier échec, et n'est plus lu
// ---------------------------------------------------------------------------

#[test]
fn un_canal_qui_se_tait_est_ecarte_au_premier_echec_et_n_est_plus_lu() {
    // Comportement n° 1 de l'issue : « un canal muet est écarté au premier échec, et n'est plus lu
    // au tour suivant ».
    //
    // C'est le critère central de #88, et le seul qui débloque vraiment le socket. Il ne se vérifie
    // pas par une égalité de sortie : une implémentation qui lirait quand même puis jetterait le
    // résultat rendrait le même verdict et gèlerait le démon exactement pareil — 36,306 s par
    // `status`, et 10,2 s d'attente pour le `geometry` qui suit. On compte donc les **lectures**, et
    // le canal écarté tend un piège qu'il n'a pas le droit de voir.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();
    bureau.se_tait(MUET, RAISON);

    // Premier tour : le canal est inconnu de la quarantaine, il est lu. Il échoue.
    let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT);
    exige_lectures(&bureau, &TOUS, "premier `status`");
    exige_une_ligne_par_canal(&rendu, "premier `status`");
    exige_illisible(&rendu, MUET, RAISON, "premier `status`");
    assert_eq!(
        rendu.a_signaler,
        vec![MUET.to_owned()],
        "la mise à l'écart est l'événement : c'est elle que le journal dit"
    );

    // Puis toute la fenêtre d'écart, `status` par `status`, jusqu'à la veille de l'échéance : le
    // canal muet n'est plus lu du tout, et les quatre autres le sont à chaque fois.
    bureau.tend_le_piege(MUET);
    for i in 1..=cycles_avant_la_premiere_retente() {
        let contexte = format!("`status` n° {i} après l'échec");
        let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT + CYCLE * i);
        exige_lectures(&bureau, &[VOISIN, VENTILO_1, VENTILO_2, BOITIER], &contexte);
        exige_une_ligne_par_canal(&rendu, &contexte);
        exige_illisible(&rendu, MUET, RAISON, &contexte);
        assert!(
            rendu.a_signaler.is_empty(),
            "{contexte} : rien de neuf ne s'est produit, le journal n'a rien à dire"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — apprendre qu'un canal se tait ne coûte qu'une seule lecture
// ---------------------------------------------------------------------------

#[test]
fn apprendre_qu_un_canal_se_tait_ne_coute_qu_une_seule_lecture() {
    // Approche technique de l'issue : « un canal porte plusieurs attributs (`rpm`, `pwm`, `mode`) là
    // où une sonde n'en a qu'un. La clef d'écartement est le canal entier, pas l'attribut : quand un
    // contrôleur ne répond plus, aucun de ses attributs ne répond, et écarter attribut par attribut
    // ferait payer trois fois cinq secondes pour l'apprendre. »
    //
    // Il faut échouer une fois pour l'apprendre — c'est irréductible, et #68 le paie déjà. Ce qui ne
    // l'est pas, c'est de le payer trois fois : la couture ne donne qu'**une** fermeture par canal
    // et par tour, et ce test la tient au mot. Deux canaux en rade coûtent alors 10,4 s au lieu des
    // 30,708 s mesurées, et zéro à tous les tours suivants.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();
    bureau.se_tait(MUET, RAISON);
    bureau.se_tait(VOISIN, AUTRE_RAISON);

    let muets = [MUET, VOISIN];
    let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT);
    let lectures = bureau.lectures();

    let bloquantes = lectures
        .iter()
        .filter(|n| muets.contains(&n.as_str()))
        .count();
    assert_eq!(
        bloquantes, 2,
        "deux canaux en rade, deux lectures bloquantes : une par canal, et non une par attribut — \
         sinon apprendre le silence coûte trois fois {COUT_D_UNE_LECTURE_MUETTE:?} par canal"
    );

    let cout = cout_modelise(&lectures, &muets);
    assert!(
        cout * 2 < COUT_OBSERVE_REPRODUIT,
        "le tour qui apprend le silence coûte {cout:?} : c'est le prix de l'apprentissage, mais il \
         doit rester très en deçà des {COUT_OBSERVE_REPRODUIT:?} mesurées"
    );

    // Et les deux sont rendus, chacun avec **sa** raison : deux canaux muets ne partagent pas la
    // leur, même sur le même contrôleur.
    exige_une_ligne_par_canal(&rendu, "le tour qui apprend");
    exige_illisible(&rendu, MUET, RAISON, "le tour qui apprend");
    exige_illisible(&rendu, VOISIN, AUTRE_RAISON, "le tour qui apprend");
    assert_eq!(
        rendu.a_signaler,
        vec![MUET.to_owned(), VOISIN.to_owned()],
        "deux mises à l'écart, deux lignes de journal, dans l'ordre des canaux"
    );
}

// ---------------------------------------------------------------------------
// 3 — un canal écarté est dit « unreadable », avec sa raison, jamais omis
// ---------------------------------------------------------------------------

#[test]
fn un_canal_ecarte_est_dit_unreadable_avec_sa_raison_jamais_omis() {
    // Comportement n° 2 de l'issue : « il est rendu `unreadable` avec sa raison, jamais omis ».
    //
    // Deux fautes sont visées, et la seconde est la plus coûteuse parce qu'elle est **rassurante** :
    //
    // — l'**omission**. Un canal absent et un canal muet sont deux choses différentes ; omettre le
    //   second fait croire qu'il n'existe pas, et le panneau VENTILOS perd une ligne sans le dire ;
    // — le **maquillage**. Rendre la dernière valeur connue, ou 0 tr/min. `ResponseLine::Unreadable`
    //   le dit déjà en toutes lettres : « un canal illisible affiché à 0 tr/min est un mensonge ».
    //
    // Et la raison doit **survivre à l'écart** : elle n'est relue nulle part pendant les cinq
    // minutes qui suivent, donc si la quarantaine ne la retient pas, le diagnostic disparaît au
    // deuxième `status` — c'est-à-dire une seconde après être apparu.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();

    // Le canal répond d'abord : c'est le cas où une valeur périmée traîne quelque part et peut être
    // resservie.
    let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT);
    exige_vivant(&rendu, MUET, "avant l'incident");
    bureau.lectures();

    // Un second tour, toujours en bonne santé : la valeur est bien installée quelque part, et c'est
    // elle qu'une implémentation « astucieuse » resservirait.
    let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT + CYCLE);
    bureau.lectures();
    exige_vivant(&rendu, MUET, "avant l'incident, second tour");

    // Puis le contrôleur lâche.
    bureau.se_tait(MUET, RAISON);
    let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT + CYCLE * 2);
    bureau.lectures();
    exige_une_ligne_par_canal(&rendu, "l'échec");
    exige_illisible(&rendu, MUET, RAISON, "l'échec");
    assert_ne!(
        *ligne_de(&rendu, MUET, "l'échec"),
        ligne_attendue(MUET, &en_bonne_sante(MUET)),
        "resservir la dernière valeur connue laisserait croire que le ventilateur tourne à \
         1 089 tr/min alors que plus rien ne le mesure"
    );

    // Puis, tout l'écart durant, la ligne reste la même : `unreadable`, avec la raison d'origine.
    // Pas une valeur périmée, pas un zéro, pas un silence.
    bureau.tend_le_piege(MUET);
    for i in 3..=cycles_avant_la_premiere_retente() {
        let contexte = format!("`status` n° {i}, canal écarté");
        let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT + CYCLE * i);
        bureau.lectures();
        exige_une_ligne_par_canal(&rendu, &contexte);
        exige_illisible(&rendu, MUET, RAISON, &contexte);
    }
}

// ---------------------------------------------------------------------------
// 4 — les canaux vivants restent là, avec leurs valeurs
// ---------------------------------------------------------------------------

#[test]
fn les_canaux_vivants_restent_la_avec_leurs_valeurs() {
    // Le contrepoids de tout ce fichier, et il est indispensable : **un test qui vérifie « `status`
    // est rapide » passerait sur une implémentation qui n'affiche plus aucun canal.** Un correctif
    // qui aurait simplement débranché le panneau VENTILOS rendrait le démon très rapide et le
    // rendrait aussi aveugle.
    //
    // Ce test exige donc l'inverse : pendant tout l'écart d'un canal, les quatre autres sont lus à
    // chaque tour et rendent **exactement** ce que la lecture leur a donné — position, régime,
    // consigne, mode et drapeau compris.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();

    // Un tour de référence, tout le monde en bonne santé : c'est le « inchangé » du critère.
    let reference = tour(&banc, &mut quarantaine, &bureau, DEBUT);
    exige_lectures(&bureau, &TOUS, "tour de référence");
    exige_une_ligne_par_canal(&reference, "tour de référence");
    for nom in TOUS {
        exige_vivant(&reference, nom, "tour de référence");
    }

    // Le Kraken lâche sur l'un de ses deux canaux.
    bureau.se_tait(MUET, RAISON);
    let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT + CYCLE);
    bureau.lectures();
    exige_illisible(&rendu, MUET, RAISON, "l'échec");

    // Et pendant tout l'écart, les quatre autres rendent toujours exactement les mêmes lignes.
    bureau.tend_le_piege(MUET);
    let vivants = [VOISIN, VENTILO_1, VENTILO_2, BOITIER];
    for i in 2..=cycles_avant_la_premiere_retente() {
        let contexte = format!("`status` n° {i}");
        let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT + CYCLE * i);
        exige_lectures(&bureau, &vivants, &contexte);
        exige_une_ligne_par_canal(&rendu, &contexte);
        for nom in vivants {
            exige_vivant(&rendu, nom, &contexte);
            assert_eq!(
                ligne_de(&rendu, nom, &contexte),
                ligne_de(&reference, nom, &contexte),
                "{contexte} : un canal muet a changé ce que « {nom} » rapporte"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — le voisin du même contrôleur continue d'être lu
// ---------------------------------------------------------------------------

#[test]
fn le_voisin_du_meme_controleur_continue_d_etre_lu() {
    // Critère d'acceptation : « l'écart est **par canal** : celui qui répond continue d'être lu
    // quand son voisin du même contrôleur se tait ».
    //
    // La faute visée est tentante, et elle est même défendable à l'oreille : le nom d'un canal porte
    // son pilote en préfixe, et « si le Kraken ne répond plus, aucun de ses canaux ne répondra » est
    // vrai dans l'incident observé — les deux lignes `unreadable` de l'issue viennent du même
    // contrôleur. C'est faux comme règle : un `pwmN_enable` peut disparaître sans que `fanN_input`
    // s'en aille, et écarter un canal qui répond, c'est perdre une mesure qu'on avait.
    //
    // Seul un second canal du **même** pilote met cette faute en évidence.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();
    bureau.se_tait(MUET, RAISON);

    let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT);
    bureau.lectures();
    exige_illisible(&rendu, MUET, RAISON, "l'échec");
    exige_vivant(&rendu, VOISIN, "l'échec");

    bureau.tend_le_piege(MUET);
    for i in 1..=cycles_avant_la_premiere_retente() {
        let contexte = format!("`status` n° {i}");
        let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT + CYCLE * i);
        let lectures = bureau.lectures();
        assert!(
            a_ete_lu(&lectures, VOISIN),
            "{contexte} : « {VOISIN} » partage son contrôleur avec « {MUET} », pas son écart — il \
             doit continuer d'être lu"
        );
        assert!(
            !a_ete_lu(&lectures, MUET),
            "{contexte} : « {MUET} » est écarté, il ne doit pas être lu"
        );
        exige_vivant(&rendu, VOISIN, &contexte);
        exige_illisible(&rendu, MUET, RAISON, &contexte);
    }
}

// ---------------------------------------------------------------------------
// 6 — deux canaux muets ont chacun leur délai
// ---------------------------------------------------------------------------

#[test]
fn deux_canaux_muets_ont_chacun_leur_delai() {
    // La forme la plus serrée de « l'écart est par canal » : non seulement l'état est séparé, mais
    // le **délai** l'est aussi. Une implémentation qui garderait un compteur d'échecs global — ou un
    // compteur par contrôleur — passerait le test précédent et se ferait attraper ici : le second
    // canal, qui n'a échoué qu'une fois, hériterait du délai accumulé par le premier et serait
    // retenté cinq minutes trop tard.
    //
    // Les deux écarts démarrent à des instants différents et n'encaissent pas le même nombre
    // d'échecs, pour qu'aucune coïncidence ne puisse les faire passer pour indépendants.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();

    // Le premier lâche à DEBUT et rate deux retentes : son délai est monté à 4 × DELAI_INITIAL.
    bureau.se_tait(MUET, RAISON);
    tour(&banc, &mut quarantaine, &bureau, DEBUT);
    bureau.lectures();
    let mut echec_a = DEBUT;
    for echecs_precedents in 0..2u32 {
        echec_a += delai_attendu(echecs_precedents);
        tour(&banc, &mut quarantaine, &bureau, echec_a);
        let lectures = bureau.lectures();
        assert!(
            a_ete_lu(&lectures, MUET),
            "la retente n° {} du premier canal est due à {echec_a:?}",
            echecs_precedents + 1
        );
    }
    let echeance_a = echec_a + delai_attendu(2);

    // Le second lâche bien plus tard, une seule fois : il est dû après DELAI_INITIAL, pas après ce
    // que le premier a accumulé.
    let echec_b = echec_a + CYCLE * 7;
    bureau.se_tait(VOISIN, AUTRE_RAISON);
    let rendu = tour(&banc, &mut quarantaine, &bureau, echec_b);
    bureau.lectures();
    exige_illisible(&rendu, VOISIN, AUTRE_RAISON, "l'échec du second canal");
    assert_eq!(
        rendu.a_signaler,
        vec![VOISIN.to_owned()],
        "seul le second canal vient d'être écarté ; le premier l'était déjà"
    );
    let echeance_b = echec_b + DELAI_INITIAL;
    assert!(
        echeance_b < echeance_a,
        "les deux échéances doivent différer pour que ce test dise quelque chose : {echeance_b:?} \
         contre {echeance_a:?}"
    );

    bureau.tend_le_piege(MUET);
    bureau.tend_le_piege(VOISIN);

    // À l'échéance du second, lui seul est retenté. Le premier dort encore.
    tour(&banc, &mut quarantaine, &bureau, echeance_b - INSTANT);
    let lectures = bureau.lectures();
    assert!(
        !a_ete_lu(&lectures, VOISIN) && !a_ete_lu(&lectures, MUET),
        "un instant avant son échéance, aucun des deux n'est dû ; lectures : {lectures:?}"
    );

    bureau.repond(VOISIN, en_bonne_sante(VOISIN));
    let rendu = tour(&banc, &mut quarantaine, &bureau, echeance_b);
    let lectures = bureau.lectures();
    assert!(
        a_ete_lu(&lectures, VOISIN),
        "« {VOISIN} » n'a échoué qu'une fois : il est dû après {DELAI_INITIAL:?}, quel que soit le \
         nombre d'échecs de « {MUET} »"
    );
    assert!(
        !a_ete_lu(&lectures, MUET),
        "« {MUET} » n'est dû qu'à {echeance_a:?} : la guérison du voisin ne l'avance pas"
    );
    exige_vivant(&rendu, VOISIN, "l'échéance du second canal");
    exige_illisible(&rendu, MUET, RAISON, "l'échéance du second canal");

    // Puis à l'échéance du premier, c'est lui qui est retenté — et l'autre, guéri, est lu à chaque
    // tour depuis un moment.
    tour(&banc, &mut quarantaine, &bureau, echeance_a - INSTANT);
    let lectures = bureau.lectures();
    assert!(
        !a_ete_lu(&lectures, MUET),
        "un instant avant son échéance, « {MUET} » n'est pas dû"
    );
    assert!(
        a_ete_lu(&lectures, VOISIN),
        "le voisin est guéri, il est lu"
    );

    tour(&banc, &mut quarantaine, &bureau, echeance_a);
    let lectures = bureau.lectures();
    assert!(
        a_ete_lu(&lectures, MUET),
        "« {MUET} » était dû à {echeance_a:?}, indépendamment de la guérison de l'autre"
    );
}

// ---------------------------------------------------------------------------
// 7 — le délai de retente double à chaque échec
// ---------------------------------------------------------------------------

#[test]
fn le_delai_de_retente_double_a_chaque_echec() {
    // Comportement n° 4 de l'issue : « le délai de retente double ».
    //
    // Le délai n'est pas un ornement : chaque retente coûte 5,218 s de socket muet, et le socket est
    // celui qui sert aussi `geometry` — d'où les 10,2 s d'attente mesurées sur une commande qui ne
    // touche aucun matériel. Un délai constant d'une minute laisserait le démon gelé 8 % du temps.
    //
    // Le délai se mesure par ses **bornes**, pincées au nanoseconde près de part et d'autre : pas de
    // lecture un instant avant l'échéance, une lecture à l'échéance exacte. C'est la seule façon
    // d'attraper les deux fautes symétriques — l'écart qui ne relâche jamais, et celui qui relâche
    // tout de suite — sans lire l'implémentation.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();
    bureau.se_tait(MUET, RAISON);

    let mut dernier_echec = DEBUT;
    let rendu = tour(&banc, &mut quarantaine, &bureau, dernier_echec);
    bureau.lectures();
    assert_eq!(rendu.a_signaler, vec![MUET.to_owned()]);

    // Quatre échecs consécutifs, donc quatre délais : 1×, 2×, 4× et 8× le délai initial. Le test
    // n° 0 garantit que le plafond ne s'en mêle pas sur cette plage.
    for echecs_precedents in 0..4u32 {
        let attendu = delai_attendu(echecs_precedents);
        assert_eq!(
            attendu,
            DELAI_INITIAL * 2u32.pow(echecs_precedents),
            "ce test vérifie la croissance, pas le plafond : sur ces quatre échecs le doublement ne \
             doit pas être rattrapé par le plafond — c'est ce que le test n° 0 garantit"
        );

        // Au tiers du délai, rien. À la moitié, rien non plus : un écart dont le délai ne croîtrait
        // pas se ferait attraper dès le deuxième tour, son échéance étant déjà passée.
        for fraction in [3u32, 2] {
            let contexte = format!(
                "après {} échec(s), au 1/{fraction} du délai",
                echecs_precedents + 1
            );
            bureau.tend_le_piege(MUET);
            let rendu = tour(
                &banc,
                &mut quarantaine,
                &bureau,
                dernier_echec + attendu / fraction,
            );
            let lectures = bureau.lectures();
            assert!(
                !a_ete_lu(&lectures, MUET),
                "{contexte} : le canal est écarté, il ne doit pas être lu"
            );
            exige_illisible(&rendu, MUET, RAISON, &contexte);
        }

        // Un nanoseconde avant l'échéance : toujours rien. Borne par le dessous, elle interdit
        // l'écart trop court.
        tour(
            &banc,
            &mut quarantaine,
            &bureau,
            dernier_echec + attendu - INSTANT,
        );
        let lectures = bureau.lectures();
        assert!(
            !a_ete_lu(&lectures, MUET),
            "après {} échec(s), un instant avant l'échéance de {attendu:?}, le canal n'est pas encore \
             dû",
            echecs_precedents + 1
        );

        // À l'échéance exacte : le canal est retenté. Borne par le dessus, elle interdit l'écart qui
        // ne relâche jamais. La retente échoue à son tour, ce qui arme le délai suivant — et le fait
        // courir depuis **cet** échec-ci.
        bureau.se_tait(MUET, RAISON);
        dernier_echec += attendu;
        let rendu = tour(&banc, &mut quarantaine, &bureau, dernier_echec);
        let lectures = bureau.lectures();
        assert!(
            a_ete_lu(&lectures, MUET),
            "la retente due après {attendu:?} n'a pas eu lieu, soit {} échec(s) encaissés",
            echecs_precedents + 1
        );
        assert!(
            rendu.a_signaler.is_empty(),
            "une retente qui échoue prolonge l'écart sans le rouvrir : rien de neuf à journaliser"
        );
    }
}

// ---------------------------------------------------------------------------
// 8 — un canal écarté n'est jamais condamné : le délai plafonne
// ---------------------------------------------------------------------------

#[test]
fn un_canal_ecarte_n_est_jamais_condamne_le_delai_plafonne() {
    // Le second piège que le parent nomme : **« le canal muet est écarté » passerait sur une
    // implémentation qui l'écarte pour toujours.** Un contrôleur qu'on rebranche ne reviendrait
    // jamais, et le seul remède serait de redémarrer le démon — ce qui, pour une correction censée
    // rendre le service robuste, serait une régression déguisée en corrigé.
    //
    // Deux fautes opposées, et il faut les deux bornes pour les séparer :
    //
    // — **sans plafond**, le doublement emporte tout : après vingt échecs, le canal ne serait
    //   retenté que dans huit siècles ;
    // — **plafond jamais atteint**, c'est-à-dire une croissance qui s'arrête trop tôt : le coût du
    //   gel resterait au-dessus de ce que l'issue promet.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();
    bureau.se_tait(MUET, RAISON);

    let mut dernier_echec = DEBUT;
    tour(&banc, &mut quarantaine, &bureau, dernier_echec);
    bureau.lectures();

    let mut satures = 0u32;
    for echecs_precedents in 0..20u32 {
        let attendu = delai_attendu(echecs_precedents);
        if attendu == DELAI_MAXIMAL {
            satures += 1;
        }

        // Borne par le dessous : rien avant l'échéance. Elle attrape le plafond qui s'appliquerait
        // trop tôt, ou une croissance qui s'arrêterait avant lui.
        tour(
            &banc,
            &mut quarantaine,
            &bureau,
            dernier_echec + attendu - INSTANT,
        );
        let lectures = bureau.lectures();
        assert!(
            !a_ete_lu(&lectures, MUET),
            "après {} échec(s), la retente n'est pas encore due",
            echecs_precedents + 1
        );

        // Borne par le dessus : à l'échéance, il est retenté. Sans plafond, l'échéance serait bien
        // plus loin et cette lecture n'aurait pas lieu.
        dernier_echec += attendu;
        tour(&banc, &mut quarantaine, &bureau, dernier_echec);
        let lectures = bureau.lectures();
        assert!(
            a_ete_lu(&lectures, MUET),
            "la retente due à {attendu:?} après {} échec(s) n'a pas eu lieu : un canal écarté n'est \
             jamais condamné",
            echecs_precedents + 1
        );
    }

    assert!(
        satures >= 10,
        "sur vingt échecs, le délai n'a saturé qu'à {satures} reprises : le plafond de \
         {DELAI_MAXIMAL:?} n'est pas atteint assez tôt"
    );

    // Et après vingt échecs, un contrôleur qu'on rebranche revient. C'est le point du test.
    bureau.repond(MUET, en_bonne_sante(MUET));
    tour(
        &banc,
        &mut quarantaine,
        &bureau,
        dernier_echec + DELAI_MAXIMAL - INSTANT,
    );
    let lectures = bureau.lectures();
    assert!(
        !a_ete_lu(&lectures, MUET),
        "le plafond vaut {DELAI_MAXIMAL:?}, pas moins"
    );

    let rendu = tour(
        &banc,
        &mut quarantaine,
        &bureau,
        dernier_echec + DELAI_MAXIMAL,
    );
    exige_vivant(
        &rendu,
        MUET,
        "après vingt échecs, le contrôleur rebranché doit revenir",
    );
}

// ---------------------------------------------------------------------------
// 9 — une retente réussie remet le canal en service, et le délai à zéro
// ---------------------------------------------------------------------------

#[test]
fn une_retente_reussie_remet_le_canal_en_service_et_le_delai_a_zero() {
    // Comportement n° 4 de l'issue, seconde moitié : « … et se remet à zéro quand le canal répond ».
    //
    // Les deux moitiés comptent, et c'est la seconde qu'on oublie : une implémentation qui sortirait
    // le canal de l'écart sans remettre le compteur laisserait un contrôleur qui clignote — et un
    // Kraken qui a lâché trois fois dans la journée en est un — écarté cinq minutes dès son prochain
    // hoquet, alors qu'il vient de prouver qu'il répond.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();
    bureau.se_tait(MUET, RAISON);

    // Trois échecs consécutifs : le délai est monté à 4 × DELAI_INITIAL.
    let mut dernier_echec = DEBUT;
    tour(&banc, &mut quarantaine, &bureau, dernier_echec);
    bureau.lectures();
    for echecs_precedents in 0..2u32 {
        dernier_echec += delai_attendu(echecs_precedents);
        tour(&banc, &mut quarantaine, &bureau, dernier_echec);
        bureau.lectures();
    }

    // La retente suivante réussit.
    let guerison = dernier_echec + delai_attendu(2);
    bureau.repond(MUET, en_bonne_sante(MUET));
    tour(&banc, &mut quarantaine, &bureau, guerison - INSTANT);
    let lectures = bureau.lectures();
    assert!(
        !a_ete_lu(&lectures, MUET),
        "juste avant l'échéance, le canal n'est pas encore retenté"
    );

    let rendu = tour(&banc, &mut quarantaine, &bureau, guerison);
    bureau.lectures();
    exige_une_ligne_par_canal(&rendu, "la retente qui guérit");
    exige_vivant(&rendu, MUET, "la retente qui guérit");
    assert!(
        rendu.a_signaler.is_empty(),
        "une guérison n'est pas une mise à l'écart : elle n'a rien à signaler à ce titre"
    );

    // Sorti de l'écart, le canal est lu à chaque `status`, comme n'importe quel autre.
    for i in 1..=15u32 {
        let contexte = format!("`status` n° {i} après guérison");
        let rendu = tour(&banc, &mut quarantaine, &bureau, guerison + CYCLE * i);
        exige_lectures(&bureau, &TOUS, &contexte);
        exige_vivant(&rendu, MUET, &contexte);
    }

    // Et le délai est bien reparti de zéro : le prochain échec vaut DELAI_INITIAL, pas les
    // 8 × DELAI_INITIAL qu'aurait valus la suite si le compteur n'avait pas été remis.
    let rechute = guerison + CYCLE * 16;
    bureau.se_tait(MUET, AUTRE_RAISON);
    let rendu = tour(&banc, &mut quarantaine, &bureau, rechute);
    bureau.lectures();
    assert_eq!(
        rendu.a_signaler,
        vec![MUET.to_owned()],
        "le canal était sorti de l'écart : y retomber est un nouvel événement, qui se journalise"
    );
    exige_illisible(&rendu, MUET, AUTRE_RAISON, "la rechute");

    bureau.tend_le_piege(MUET);
    tour(
        &banc,
        &mut quarantaine,
        &bureau,
        rechute + DELAI_INITIAL - INSTANT,
    );
    let lectures = bureau.lectures();
    assert!(
        !a_ete_lu(&lectures, MUET),
        "après la rechute, le canal reste écarté jusqu'à {DELAI_INITIAL:?}"
    );

    tour(&banc, &mut quarantaine, &bureau, rechute + DELAI_INITIAL);
    let lectures = bureau.lectures();
    assert!(
        a_ete_lu(&lectures, MUET),
        "la retente qui suit la rechute est due après {DELAI_INITIAL:?}, et non après le délai que \
         le canal avait accumulé avant de guérir"
    );
}

// ---------------------------------------------------------------------------
// 10 — la mise à l'écart ne se signale qu'une fois
// ---------------------------------------------------------------------------

#[test]
fn la_mise_a_l_ecart_ne_se_signale_qu_une_fois() {
    // Critère d'acceptation : « le journal le signale **une fois** par mise à l'écart, pas à chaque
    // tour ».
    //
    // « Une fois » est une propriété d'**état**, pas de sortie : c'est le relevé qui doit dire
    // « c'est la première fois », sinon l'appelant n'a d'autre choix que de journaliser à chaque
    // tour — soit une ligne par seconde, pour toujours, dans un journal qu'on lit justement pour
    // trouver ce genre d'incident. D'où `a_signaler`.
    //
    // Le compte porte sur toute la vie de l'écart : l'entrée, les tours muets, et les retentes qui
    // échouent. Une seule doit signaler.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();
    bureau.se_tait(MUET, RAISON);

    let mut signalements = 0usize;

    let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT);
    bureau.lectures();
    signalements += rendu
        .a_signaler
        .iter()
        .filter(|n| n.as_str() == MUET)
        .count();
    assert_eq!(
        rendu.a_signaler,
        vec![MUET.to_owned()],
        "l'entrée dans l'écart est l'événement : c'est elle qui se journalise"
    );

    // Puis on laisse tourner une heure de démon, `status` par seconde, avec les retentes qui tombent
    // dedans et qui échouent toutes.
    let mut dernier_echec = DEBUT;
    let mut echecs_encaisses = 1u32;
    for i in 1..=3_600u32 {
        let maintenant = DEBUT + CYCLE * i;
        let rendu = tour(&banc, &mut quarantaine, &bureau, maintenant);
        bureau.lectures();
        signalements += rendu
            .a_signaler
            .iter()
            .filter(|n| n.as_str() == MUET)
            .count();
        exige_illisible(&rendu, MUET, RAISON, &format!("`status` n° {i}"));
        if maintenant >= dernier_echec + delai_attendu(echecs_encaisses - 1) {
            dernier_echec = maintenant;
            echecs_encaisses += 1;
        }
    }

    assert!(
        echecs_encaisses > 4,
        "l'heure simulée doit contenir plusieurs retentes, sinon le test ne vérifie que les tours \
         muets ; {echecs_encaisses} échec(s) encaissé(s)"
    );
    assert_eq!(
        signalements, 1,
        "sur une heure d'écart — 3 600 `status` et {echecs_encaisses} échecs —, la mise à l'écart \
         doit être signalée une seule fois, trouvé {signalements}"
    );

    // La guérison, puis une rechute : c'est une **nouvelle** mise à l'écart, et elle se signale. Le
    // contraire ferait taire à jamais un contrôleur qui clignote, c'est-à-dire justement celui dont
    // on veut entendre parler.
    //
    // L'instant est pris un plafond entier après le dernier tour : le délai ne dépassant jamais
    // DELAI_MAXIMAL, une retente y est due à coup sûr, quel que soit le moment du dernier échec.
    let apres = DEBUT + CYCLE * 3_600 + DELAI_MAXIMAL;
    bureau.repond(MUET, en_bonne_sante(MUET));
    let rendu = tour(&banc, &mut quarantaine, &bureau, apres);
    bureau.lectures();
    exige_vivant(&rendu, MUET, "au bout d'une heure, la retente due réussit");

    bureau.se_tait(MUET, RAISON);
    let rendu = tour(&banc, &mut quarantaine, &bureau, apres + CYCLE);
    bureau.lectures();
    assert_eq!(
        rendu.a_signaler,
        vec![MUET.to_owned()],
        "un canal guéri qui retombe est écarté à nouveau : c'est un nouvel événement"
    );

    // Et le nouvel écart se tait ensuite, comme le premier.
    bureau.tend_le_piege(MUET);
    for i in 2..=cycles_avant_la_premiere_retente() {
        let rendu = tour(&banc, &mut quarantaine, &bureau, apres + CYCLE * i);
        bureau.lectures();
        assert!(
            rendu.a_signaler.is_empty(),
            "au `status` n° {i} du second écart, il n'y a rien de neuf à journaliser"
        );
    }
}

// ---------------------------------------------------------------------------
// 11 — en régime établi, `status` ne lit plus rien de muet et tient sous 100 ms
// ---------------------------------------------------------------------------

#[test]
fn un_status_en_regime_etabli_ne_lit_plus_rien_de_muet_et_tient_sous_cent_millisecondes() {
    // Critère d'acceptation : « `status` sous 100 ms avec au moins un canal muet ».
    //
    // ⚠️ **Ce test ne chronomètre rien** — il modélise. Une durée réelle mesurerait la charge de la
    // machine de test ; ce qu'on veut mesurer, c'est le nombre de lectures qui butent sur un
    // contrôleur en rade, à 5,218 s pièce. Une lecture qui n'a pas lieu ne bloque pas : c'est tout
    // le théorème.
    //
    // Le décor est celui de l'incident : les **deux** canaux du Kraken en rade, comme les deux
    // lignes `unreadable` de l'issue. Avant #88, chaque `status` coûtait 36,306 s puis 30,708 s, et
    // le `geometry` qui suivait attendait 10,2 s pour ne toucher aucun matériel.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();
    let muets = [MUET, VOISIN];
    for nom in muets {
        bureau.se_tait(nom, RAISON);
    }

    // Le premier `status` paie l'apprentissage : c'est irréductible, il faut échouer une fois pour
    // savoir. Il coûte déjà trois fois moins que ce qui a été mesuré (voir le test n° 2).
    let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT);
    let premier = cout_modelise(&bureau.lectures(), &muets);
    exige_une_ligne_par_canal(&rendu, "le `status` qui apprend");
    assert!(
        premier < COUT_OBSERVE_REPRODUIT,
        "même le tour qui apprend doit coûter moins que les {COUT_OBSERVE_REPRODUIT:?} mesurées, \
         trouvé {premier:?}"
    );

    // Puis une minute entière de fenêtre ouverte, un `status` par seconde.
    //
    // ⚠️ **Le budget vaut en régime établi, la retente exceptée** — arbitrage rendu le 2026-08-09,
    // et il corrige le critère d'acceptation de l'issue plutôt que ce fichier. Écrit d'abord « sous
    // 100 ms, toujours », il était **impossible à satisfaire** : relire est le seul moyen de savoir
    // si un canal est revenu, et cette relecture coûte ses 5,218 s. L'exiger à zéro aurait demandé
    // soit de ne jamais retenter — ce que les tests n° 8 et 9 interdisent, et à raison —, soit un
    // fil dédié que l'issue met hors scope.
    //
    // Ce que le test exige donc : **une seule** retente sur la minute, et rien du tout les
    // cinquante-neuf autres secondes.
    let mut total = Duration::ZERO;
    let mut retentes = 0u32;
    for i in 1..=60u32 {
        let contexte = format!("`status` n° {i}");
        let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT + CYCLE * i);
        let cout = cout_modelise(&bureau.lectures(), &muets);
        total += cout;
        if cout.is_zero() {
            // Le cas courant, et il doit être écrasant.
        } else {
            retentes += 1;
            assert_eq!(
                i,
                cycles_avant_la_premiere_retente() + 1,
                "{contexte} : la seule relecture permise est celle de l'échéance, au cycle {}",
                cycles_avant_la_premiere_retente() + 1
            );
        }

        // Et il tient sous ce budget en **disant** ce qu'il sait, pas en se taisant : les deux
        // canaux en rade sont rendus illisibles, les trois autres avec leurs valeurs.
        exige_une_ligne_par_canal(&rendu, &contexte);
        for nom in muets {
            exige_illisible(&rendu, nom, RAISON, &contexte);
        }
        for nom in [VENTILO_1, VENTILO_2, BOITIER] {
            exige_vivant(&rendu, nom, &contexte);
        }
    }

    assert_eq!(
        retentes, 1,
        "sur une minute, l'échéance de {DELAI_INITIAL:?} n'échoit qu'une fois : {retentes} \
         relectures trahissent une quarantaine qui ne retient pas"
    );
    // ⚠️ **Une minute entière de `status` doit coûter moins qu'un seul `status` d'avant.** C'est la
    // formulation qui dit quelque chose une fois la retente admise — comparer ce total aux
    // {ATTENTE_DE_GEOMETRY:?} qu'un `geometry` attendait, comme ce test le faisait, n'avait de sens
    // que tant qu'on l'exigeait nul.
    assert!(
        total < COUT_OBSERVE_REPRODUIT,
        "soixante `status` doivent coûter moins que le seul {COUT_OBSERVE_REPRODUIT:?} mesuré sur \
         SHYNAEL avant #88, trouvé {total:?}"
    );

    // Et le coût moyen, qui est ce que l'utilisateur ressent : la fenêtre demande `status` chaque
    // seconde, et ce qu'elle attend en moyenne doit avoir changé d'ordre de grandeur.
    let moyen = total / 60;
    assert!(
        moyen * 100 < COUT_OBSERVE_REPRODUIT,
        "en moyenne sur la minute, un `status` doit coûter au moins cent fois moins que les \
         {COUT_OBSERVE_REPRODUIT:?} mesurées, trouvé {moyen:?}"
    );
}

// ---------------------------------------------------------------------------
// 12 — chaque canal donne exactement une ligne, dans l'ordre reçu
// ---------------------------------------------------------------------------

#[test]
fn chaque_canal_donne_exactement_une_ligne_dans_l_ordre_recu() {
    // L'invariant qui rend « jamais omis » vérifiable d'un coup d'œil, et qui interdit du même geste
    // la faute inverse — un canal rendu deux fois, une ligne `chan` **et** une ligne `unreadable`,
    // ce qui laisserait la fenêtre choisir laquelle croire.
    //
    // Le scénario est irrégulier à dessein — pannes, guérisons, rechutes décalées — parce qu'un
    // relevé ne se trompe pas sur une suite régulière.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();

    for i in 0..400u32 {
        match i {
            10 => bureau.se_tait(MUET, RAISON),
            37 => bureau.se_tait(VENTILO_2, AUTRE_RAISON),
            120 => bureau.repond(MUET, en_bonne_sante(MUET)),
            180 => bureau.se_tait(VOISIN, RAISON),
            250 => bureau.repond(VENTILO_2, en_bonne_sante(VENTILO_2)),
            300 => bureau.se_tait(MUET, AUTRE_RAISON),
            360 => bureau.repond(VOISIN, en_bonne_sante(VOISIN)),
            _ => {}
        }
        let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT + CYCLE * i);
        bureau.lectures();
        exige_une_ligne_par_canal(&rendu, &format!("`status` n° {i}"));

        // Et rien ne se signale sans être écarté : `a_signaler` ne nomme que des canaux qui
        // paraissent en `unreadable` dans le même tour.
        for nom in &rendu.a_signaler {
            let ligne = ligne_de(&rendu, nom, &format!("`status` n° {i}"));
            assert!(
                matches!(ligne, ResponseLine::Unreadable { .. }),
                "`status` n° {i} : « {nom} » est signalé au journal comme mis à l'écart, mais sa \
                 ligne est {ligne:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 13 — les lignes produites traversent le protocole sans perte
// ---------------------------------------------------------------------------

#[test]
fn les_lignes_produites_traversent_le_protocole_sans_perte() {
    // Ce que ce test empêche est concret : une raison vide, ou un sujet qui porterait un espace.
    //
    // `unreadable <sujet> <raison>` se relit en coupant au **premier** espace. Une raison vide rend
    // la ligne impossible à relire ; un sujet à espaces déplacerait la frontière, et le sujet lu par
    // la fenêtre ne serait plus le canal. `encode_response_line` neutralise les blancs des champs
    // qui ne sont pas en fin de ligne — donc un sujet à espaces ne casse pas la relecture, il rend
    // silencieusement un nom de canal que rien ne reconnaîtra.
    //
    // L'aller-retour attrape les trois d'un coup, et il coûte une boucle.
    let banc = banc();
    let mut quarantaine = Quarantaine::nouvelle();
    let bureau = Bureau::neuf();
    bureau.se_tait(MUET, RAISON);
    bureau.se_tait(VOISIN, AUTRE_RAISON);

    for i in 0..40u32 {
        let rendu = tour(&banc, &mut quarantaine, &bureau, DEBUT + CYCLE * i);
        bureau.lectures();
        for ligne in &rendu.lignes {
            let encodee = encode_response_line(ligne);
            assert!(
                !encodee.contains('\n'),
                "`status` n° {i} : une ligne de réponse tient sur une ligne, trouvé « {encodee} »"
            );
            let relue = parse_response_line(&encodee).unwrap_or_else(|e| {
                panic!("`status` n° {i} : « {encodee} » ne se relit pas — {e}")
            });
            assert_eq!(
                relue, *ligne,
                "`status` n° {i} : « {encodee} » ne rend pas ce qui a été produit — la fenêtre \
                 afficherait autre chose que ce que le démon a mesuré"
            );
        }
    }
}
