//! Tests de **logique** du fil de réparation (issue #98).
//!
//! ⚠️ Ce ne sont **pas** des tests d'intention : ils ont été écrits avec le code,
//! en le voyant. Les tests d'intention de #98 portent sur la **décision**
//! (`spec_reparation_source.rs`), écrits à l'aveugle depuis l'issue seule, et ils
//! disent explicitement ce qu'ils laissent de côté : « Le fil dédié. #83 l'a fait
//! pour la dalle, avec sa propre couture et ses propres tests. »
//!
//! Ce fichier est ces tests-là. Ce qu'ils protègent est le critère d'acceptation
//! que la partie pure ne peut pas porter : « **La tentative ne bloque pas le
//! socket** : `status` et `geometry` répondent pendant qu'elle a lieu. »
//!
//! ⚠️ **Aucun périphérique n'est réinitialisé ici.** Le réparateur est factice, et
//! il ne dort pas : il prévient qu'il a commencé, puis **attend le feu du test**
//! pour rendre la main. C'est ce qui rend « pendant que le geste traîne »
//! reproductible au lieu de dépendre de l'ordonnanceur de la machine.

use std::io;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use reverb_daemon::fil_reparation::{FilReparation, Reparateur};
use reverb_daemon::reparation::{Constat, DELAI_ENTRE_TENTATIVES, EtatSource};

/// La source qui lâche, nommée comme son pilote `hwmon`.
const KRAKEN: &str = "kraken2023elite";

/// Ses trois cibles, recopiées des lignes du journal de l'issue.
const CIBLES: [&str; 3] = [
    "kraken2023elite:fan-speed",
    "kraken2023elite:pump-speed",
    "kraken2023elite:coolant-temp",
];

/// Le budget d'un `status`, critère d'acceptation de #88 que #98 ne doit pas
/// défaire.
const BUDGET_STATUS: Duration = Duration::from_millis(100);

/// De quoi laisser un fil démarrer sans rendre le test dépendant de la charge de
/// la machine.
const PATIENCE: Duration = Duration::from_secs(5);

/// Assez court pour ne pas allonger la suite, assez long pour qu'un second geste
/// parti par erreur ait le temps de se voir.
const SILENCE: Duration = Duration::from_millis(250);

/// Une source dont toutes les cibles se taisent.
fn effondree() -> EtatSource {
    EtatSource {
        source: KRAKEN.to_owned(),
        cibles: CIBLES.map(str::to_owned).to_vec(),
        muettes: CIBLES.map(str::to_owned).to_vec(),
    }
}

/// Une source qui répond.
fn vivante() -> EtatSource {
    EtatSource {
        source: KRAKEN.to_owned(),
        cibles: CIBLES.map(str::to_owned).to_vec(),
        muettes: Vec::new(),
    }
}

/// Un réparateur qui ne réinitialise rien, et que le test tient par la main.
///
/// Il annonce chaque geste sur `debuts`, puis **bloque** jusqu'à ce que le test
/// lui donne son verdict sur `feux`. Un `sleep` aurait fait la même chose en
/// pariant sur une durée : ici, « le geste est en cours » est un fait observable.
struct Factice {
    debuts: Sender<String>,
    feux: Receiver<Result<(), String>>,
    sources: Vec<String>,
}

impl Reparateur for Factice {
    fn sources(&self) -> Vec<String> {
        self.sources.clone()
    }

    fn reinitialiser(&mut self, source: &str) -> io::Result<()> {
        self.debuts
            .send(source.to_owned())
            .expect("le test doit écouter les débuts de geste");
        // ⚠️ **Borné, et pas un `recv` nu.** Un test qui oublierait de libérer son
        // geste ferait dormir ce fil pour toujours, et `arreter` l'attendrait —
        // une suite de tests qui se fige au lieu d'échouer, c'est-à-dire le pire
        // des deux mondes. La même leçon que `hidraw::ask` dans #83.
        match self.feux.recv_timeout(PATIENCE) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(raison)) => Err(io::Error::other(raison)),
            Err(erreur) => Err(io::Error::other(format!(
                "le test n'a donné aucun feu en {PATIENCE:?} : {erreur}"
            ))),
        }
    }
}

/// Le banc : le fil, plus les deux bouts que le test tient.
struct Banc {
    fil: FilReparation,
    debuts: Receiver<String>,
    feux: Sender<Result<(), String>>,
}

impl Banc {
    fn neuf(sources: &[&str]) -> Banc {
        let (envoi_debuts, debuts) = channel();
        let (feux, reception_feux) = channel();
        let fil = FilReparation::demarrer(Factice {
            debuts: envoi_debuts,
            feux: reception_feux,
            sources: sources.iter().map(|s| (*s).to_owned()).collect(),
        });
        Banc { fil, debuts, feux }
    }

    /// Attend qu'un geste commence, et dit sur quelle source.
    fn attendre_un_geste(&self, contexte: &str) -> String {
        self.debuts
            .recv_timeout(PATIENCE)
            .unwrap_or_else(|erreur| panic!("{contexte} : aucun geste en {PATIENCE:?} ({erreur})"))
    }

    /// Laisse le geste en cours rendre son verdict.
    fn liberer(&self, verdict: Result<(), String>) {
        self.feux.send(verdict).expect("le fil doit être en vie");
    }

    /// Exige qu'aucun geste ne commence dans les [`SILENCE`] qui viennent.
    fn exige_aucun_geste(&self, contexte: &str) {
        match self.debuts.recv_timeout(SILENCE) {
            Err(RecvTimeoutError::Timeout) => {}
            Ok(source) => panic!(
                "{contexte} : « {source} » a reçu un reset qu'elle ne devait pas recevoir — un \
                 USBDEVFS_RESET fait disparaître puis réapparaître le périphérique"
            ),
            Err(RecvTimeoutError::Disconnected) => panic!("{contexte} : le fil est mort"),
        }
    }
}

// ---------------------------------------------------------------------------
// 1 — déposer ne bloque pas, même pendant qu'un reset traîne
// ---------------------------------------------------------------------------

#[test]
fn deposer_ne_bloque_pas_pendant_qu_un_reset_traine() {
    // Le critère d'acceptation que la partie pure ne peut pas porter : « La
    // tentative ne bloque pas le socket ». Personne n'a mesuré ce que coûte un
    // `USBDEVFS_RESET` sur un contrôleur en difficulté — le périphérique quitte le
    // bus, le noyau réénumère, les pilotes se relient. Ce test ne suppose donc
    // aucune durée : il **tient le geste ouvert** et vérifie que le fil principal
    // continue de passer.
    let banc = Banc::neuf(&[KRAKEN]);

    banc.fil.soumettre(effondree(), Duration::from_secs(3_600));
    assert_eq!(
        banc.attendre_un_geste("le tour de l'effondrement"),
        KRAKEN,
        "l'effondrement doit déclencher un reset, sur cette source-là"
    );

    // Le geste est en cours et le restera tant que le test ne l'aura pas libéré.
    // Le fil principal, lui, continue de déposer — ce qu'il fait vraiment, tour
    // après tour, tant qu'une fenêtre est ouverte.
    //
    // ⚠️ Les instants injectés restent **sous** [`DELAI_ENTRE_TENTATIVES`] : ce
    // test mesure un dépôt, pas une échéance, et un état déjà dû ferait partir un
    // second geste qui n'a rien à voir avec ce qu'on vérifie.
    let debut = Instant::now();
    for i in 1..=60u32 {
        banc.fil.soumettre(
            effondree(),
            Duration::from_secs(3_600) + Duration::from_millis(u64::from(i)),
        );
    }
    let coute = debut.elapsed();

    assert!(
        coute < BUDGET_STATUS,
        "soixante dépôts ont coûté {coute:?} pendant qu'un reset était en cours : le fil qui sert \
         le socket a attendu le matériel, ce qui est exactement le défaut de #68, #83 et #88"
    );

    banc.liberer(Ok(()));
    let constats = banc.fil.arreter();
    assert!(
        constats.contains(&(KRAKEN.to_owned(), Constat::Reussie { tentative: 1 })),
        "le verdict du geste doit revenir au fil principal — trouvé {constats:?}"
    );
}

// ---------------------------------------------------------------------------
// 2 — les états qui s'accumulent pendant un geste ne se jugent pas l'un après
//     l'autre
// ---------------------------------------------------------------------------

#[test]
fn un_etat_perime_est_remplace_et_non_empile() {
    // Le fil principal dépose une fois par seconde ; un reset dure bien plus
    // longtemps que ça. Une file qui accumulerait ferait juger, après coup, des
    // effondrements que la réalité a déjà démentis — et, sur une source revenue
    // puis repartie, des tours dans le désordre.
    //
    // Ce test tient un geste ouvert, dépose dix états pendant ce temps, puis
    // libère : il ne doit rester **qu'un seul** état à juger, le dernier.
    let banc = Banc::neuf(&[KRAKEN]);

    banc.fil.soumettre(effondree(), Duration::from_secs(3_600));
    banc.attendre_un_geste("le tour de l'effondrement");

    for i in 1..=10u32 {
        banc.fil
            .soumettre(effondree(), Duration::from_secs(3_600 + u64::from(i)));
    }
    banc.liberer(Ok(()));

    // Les dix états déposés ont été réduits à un, et cet un-là tombe dans la
    // fenêtre d'attente : aucun second geste.
    banc.exige_aucun_geste("les états déposés pendant le geste");

    let constats = banc.fil.arreter();
    assert_eq!(
        constats,
        vec![(KRAKEN.to_owned(), Constat::Reussie { tentative: 1 })],
        "un seul geste, donc un seul constat : dix états empilés en auraient rendu dix, dont neuf \
         périmés — trouvé {constats:?}"
    );
}

// ---------------------------------------------------------------------------
// 3 — deux gestes ne se chevauchent jamais
// ---------------------------------------------------------------------------

#[test]
fn deux_gestes_ne_se_chevauchent_jamais() {
    // « Deux tentatives ne peuvent pas se chevaucher » est vrai **par
    // construction** ici : un seul fil traite un état à la fois, et il détient
    // seul le réparateur. Ce test le rend observable — le second geste ne part
    // qu'après que le premier a rendu la main, et pas avant son échéance.
    let banc = Banc::neuf(&[KRAKEN]);
    let debut = Duration::from_secs(3_600);

    banc.fil.soumettre(effondree(), debut);
    banc.attendre_un_geste("la première tentative");

    // Un état déjà dû est déposé pendant que le premier geste est ouvert. S'il
    // partait quand même, deux resets se croiseraient sur le même périphérique.
    banc.fil
        .soumettre(effondree(), debut + DELAI_ENTRE_TENTATIVES);
    banc.exige_aucun_geste("pendant que la première tentative est en cours");

    banc.liberer(Err(
        "USBDEVFS_RESET: No such device (os error 19)".to_owned()
    ));

    // Le premier geste rendu, celui qui attendait part — et c'est bien le second.
    assert_eq!(banc.attendre_un_geste("la seconde tentative"), KRAKEN);
    banc.liberer(Ok(()));

    let constats = banc.fil.arreter();
    let rangs: Vec<Option<u32>> = constats
        .iter()
        .map(|(_, constat)| match constat {
            Constat::Reussie { tentative } | Constat::Echouee { tentative, .. } => Some(*tentative),
            _ => None,
        })
        .collect();
    assert_eq!(
        rangs,
        vec![Some(1), Some(2)],
        "les deux gestes se suivent, dans l'ordre — trouvé {constats:?}"
    );
}

// ---------------------------------------------------------------------------
// 4 — un tour sans geste ne dit rien
// ---------------------------------------------------------------------------

#[test]
fn un_tour_sans_geste_ne_remonte_aucun_constat() {
    // Le fil principal dépose l'état de chaque source une fois par seconde, qu'il
    // se passe quelque chose ou non. Remonter `Rien`, `Patiente` et `Repos`
    // noierait le journal d'une ligne par seconde et par source — dans un journal
    // qu'on lit justement pour trouver ce genre d'incident. C'est la règle du
    // `Repos` de la dalle (#83).
    let banc = Banc::neuf(&[KRAKEN]);

    for i in 0..60u32 {
        banc.fil
            .soumettre(vivante(), Duration::from_secs(3_600 + u64::from(i)));
    }
    banc.exige_aucun_geste("une source qui répond");

    let constats = banc.fil.arreter();
    assert!(
        constats.is_empty(),
        "soixante tours sur une source qui va bien ont produit {} constat(s) : {constats:?}",
        constats.len()
    );
}

// ---------------------------------------------------------------------------
// 5 — le fil dit de quelles sources il vaut la peine de parler
// ---------------------------------------------------------------------------

#[test]
fn le_fil_declare_les_sources_que_le_reparateur_sait_traiter() {
    // Déposer l'état d'une source que personne ne sait réparer ferait brûler trois
    // tentatives et une ligne d'abandon pour apprendre ce qu'on savait déjà. La
    // liste est relevée **avant** que le réparateur ne parte dans son fil : après,
    // plus personne ne peut le lui demander, et c'est exactement ce qui garantit
    // que deux gestes ne se chevauchent pas.
    let banc = Banc::neuf(&[KRAKEN]);
    assert_eq!(banc.fil.sources(), [KRAKEN.to_owned()]);

    let vide = Banc::neuf(&[]);
    assert!(
        vide.fil.sources().is_empty(),
        "un réparateur qui ne sait rien faire ne doit faire déposer aucun état"
    );
}
