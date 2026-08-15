//! Tests d'intention du bouton « auto » que la fenêtre ne doit plus offrir (issue #97).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #97 seule. Rien n'a été relu
//! de `crates/reverb-daemon/src/` ni de `crates/reverb-hw/src/` hormis la liste
//! des signatures publiques — celles sans lesquelles un test ne compile pas. À
//! l'écriture de ce fichier, `telemetrie::lire_le_canal` est **privée** et ne
//! prend pas de carnet : la compilation doit échouer, et c'est la phase rouge.
//!
//! Ce fichier est le second volet de #97. Le premier,
//! `crates/reverb-hw/tests/spec_courbe_avant_auto.rs`, fige le **refus** ; celui-ci
//! fige ce que la fenêtre en apprend.
//!
//! # Le critère que ce fichier couvre, et où il se décide
//!
//! issue #97, comportement attendu — « La fenêtre n'offre plus "auto" sur un canal
//! sans courbe téléversée : le projet a déjà tranché que "montrer un bouton qui ne
//! peut qu'échouer vaut moins que ne pas le montrer" ».
//!
//! ⚠️ **La fenêtre ne décide pas de cela, et ne doit pas le décider.** Elle n'ouvre
//! aucun périphérique (ADR-002) : elle affiche le bouton quand la ligne `chan` que
//! le démon lui envoie porte `sait_faire_auto` (#50, #88). Le seul endroit où ce
//! booléen se calcule est le relevé d'un canal, dans le démon — donc le seul
//! endroit où ce critère peut être vérifié sans ouvrir de session graphique.
//!
//! # La couture que ces tests exigent
//!
//! ```ignore
//! // crates/reverb-daemon/src/telemetrie.rs
//!
//! use reverb_hw::hwmon::{CourbesPosees, FanChannel};
//!
//! /// Ce que la ligne `chan` d'un canal porte, ou pourquoi elle n'a pas pu être lue.
//! ///
//! /// `sait_faire_auto` répond « le bouton marcherait **maintenant** » : le pilote
//! /// sait exécuter une courbe (#50) **et** une courbe y a été posée depuis le
//! /// démarrage (#97). Les deux conditions, jamais une seule.
//! pub fn lire_le_canal(
//!     canal: &FanChannel,
//!     posees: &CourbesPosees,
//! ) -> Result<LectureCanal, String>;
//! ```
//!
//! Deux choix, et ce qu'ils achètent :
//!
//! 1. **La fonction devient publique plutôt que d'être recopiée dans un test.**
//!    C'est celle que le démon appelle à chaque `status`, une fois par seconde ; en
//!    tester une jumelle vérifierait la jumelle.
//! 2. **Le carnet arrive en paramètre, il n'est pas relu du matériel.** Il ne peut
//!    pas l'être : `tempN_auto_pointM_pwm` est en écriture seule (`0200`,
//!    docs/VENTILATEURS.md). C'est toute l'approche technique de l'issue.
//!
//! # Ce que ce fichier fige
//!
//! 1. Sans courbe posée, la ligne `chan` d'un canal du Kraken dit **non** — alors
//!    même que son pilote, lui, sait exécuter une courbe.
//! 2. Une courbe posée rouvre le bouton, **sur ce canal et lui seul**.
//! 3. Un pilote sans mode automatique ne le rouvre jamais.
//! 4. Ce que la fenêtre reçoit ne peut pas contredire ce que le socket accepte.
//! 5. Le reste de la ligne — régime, consigne, mode — continue d'être relevé : le
//!    panneau VENTILOS ne se débranche pas au passage.
//!
//! # Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Le chemin complet du socket.** issue #97, critère d'acceptation — « Le refus
//!   vaut aussi par le socket (`fan <canal> auto`), qui rend une ligne `err` et non
//!   un `end` ». `fan … auto` traverse `Peripheriques::consigner`, et
//!   `Peripheriques::ouvrir()` réclame de vrais `/dev/hidraw*`, un `/dev/i2c-8` et
//!   un nœud USB : **rien n'y est exerçable sans matériel**, et le dépôt interdit
//!   d'écrire sur du matériel dans un test automatisé. Ce qui est vérifiable l'est :
//!   l'`io::Error` que `set_mode` doit rendre (spec_courbe_avant_auto.rs) est ce
//!   que `consigner` remonte, et la traduction d'une erreur en ligne `err` est le
//!   comportement existant du démon, que #97 ne change pas. Le critère se vérifie à
//!   la main, sur la machine, avec `socat`.
//! - **Le rendu de la fenêtre.** Elle lit le booléen de la ligne `chan` (#88) : le
//!   vérifier une seconde fois dans `reverb-gui` ne vérifierait que le fil, pas la
//!   décision.
//!
//! # Aucun accès matériel
//!
//! Les seuls chemins touchés ici sont ceux d'une arborescence construite dans
//! `std::env::temp_dir()`, effacée à la fin de chaque test. **Rien n'est lu ni
//! écrit sous `/sys`.**

use std::fs;
use std::path::{Path, PathBuf};

use reverb_daemon::telemetrie::lire_le_canal;
use reverb_hw::hwmon::{
    self, CourbesPosees, Curve, FanChannel, Mode, Percent, set_curve, set_mode,
};

// ---------------------------------------------------------------------------
// Fausse arborescence sysfs
//
// Dupliquée depuis `spec_courbe_avant_auto.rs` plutôt que partagée : deux
// fichiers de tests d'intégration ne partagent rien, et celui-ci vit dans un
// autre crate.
// ---------------------------------------------------------------------------

/// Une fausse `/sys/class/hwmon` dans un répertoire temporaire.
struct FauxSysfs {
    racine: PathBuf,
}

impl FauxSysfs {
    fn neuf(nom_du_test: &str) -> Self {
        let racine = std::env::temp_dir().join(format!(
            "reverb-spec-auto-sans-courbe-{nom_du_test}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&racine);
        fs::create_dir_all(&racine).expect("création de la racine temporaire");
        Self { racine }
    }

    fn racine(&self) -> &Path {
        &self.racine
    }

    /// Crée `<racine>/<dossier>` et son fichier `name`.
    fn hwmon(&self, dossier: &str, source: &str) -> PathBuf {
        let chemin = self.racine.join(dossier);
        fs::create_dir_all(&chemin).expect("création d'un répertoire hwmon");
        ecrire(&chemin.join("name"), source);
        chemin
    }

    fn canaux(&self) -> Vec<FanChannel> {
        hwmon::discover_in(self.racine()).expect("découverte sur l'arborescence temporaire")
    }
}

impl Drop for FauxSysfs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.racine);
    }
}

/// Écrit un fichier sysfs, avec le saut de ligne que le noyau ajoute.
fn ecrire(chemin: &Path, valeur: &str) {
    fs::write(chemin, format!("{valeur}\n")).expect("écriture d'un fichier sysfs simulé");
}

/// Les 40 fichiers de courbe d'un indice de température, tous à « 0 ».
fn fichiers_de_courbe(hwmon: &Path, temp: u32) {
    for m in 1..=40 {
        ecrire(&hwmon.join(format!("temp{temp}_auto_point{m}_pwm")), "0");
    }
}

/// L'arborescence du relevé : le Kraken avec ses courbes, un `nzxtsmart2` sans
/// mode automatique, une carte mère sans `pwm*_enable`.
fn arborescence_de_reference(nom_du_test: &str) -> FauxSysfs {
    let sysfs = FauxSysfs::neuf(nom_du_test);

    let kraken = sysfs.hwmon("hwmon6", "kraken2023elite");
    for (n, libelle, pwm) in [(1u32, "Pump speed", "171"), (2, "Fan speed", "71")] {
        ecrire(&kraken.join(format!("pwm{n}")), pwm);
        ecrire(&kraken.join(format!("pwm{n}_enable")), "0");
        ecrire(&kraken.join(format!("fan{n}_input")), "2380");
        ecrire(&kraken.join(format!("fan{n}_label")), libelle);
        fichiers_de_courbe(&kraken, n);
    }

    let nzxt = sysfs.hwmon("hwmon4", "nzxtsmart2");
    for (n, libelle) in [(1u32, "FAN 1"), (2, "FAN 2"), (3, "FAN 3")] {
        ecrire(&nzxt.join(format!("pwm{n}")), "64");
        ecrire(&nzxt.join(format!("pwm{n}_enable")), "1");
        ecrire(&nzxt.join(format!("fan{n}_input")), "700");
        ecrire(&nzxt.join(format!("fan{n}_label")), libelle);
    }

    let nct = sysfs.hwmon("hwmon2", "nct6687");
    for (n, libelle) in [(1u32, "CPU FAN"), (2, "SYS FAN 1")] {
        ecrire(&nct.join(format!("pwm{n}")), "153");
        ecrire(&nct.join(format!("fan{n}_input")), "0");
        ecrire(&nct.join(format!("fan{n}_label")), libelle);
    }

    sysfs
}

/// Le canal nommé `nom`, ou un échec explicite.
fn canal<'a>(canaux: &'a [FanChannel], nom: &str) -> &'a FanChannel {
    canaux
        .iter()
        .find(|c| c.name == nom)
        .unwrap_or_else(|| panic!("le canal « {nom} » doit être découvert"))
}

/// Une courbe quelconque mais valable : une rampe de 30 % à 100 %.
fn courbe_de_reference() -> Curve {
    Curve::interpolate(&[
        (1, Percent::new(30).expect("30 % est un pourcentage")),
        (40, Percent::new(100).expect("100 % est un pourcentage")),
    ])
    .expect("une rampe croissante de 30 % à 100 % est une courbe valable")
}

/// Ce que la fenêtre lira sur la ligne `chan` de ce canal.
fn propose_auto(canal: &FanChannel, posees: &CourbesPosees) -> bool {
    lire_le_canal(canal, posees)
        .expect("le canal de l'arborescence temporaire répond")
        .sait_faire_auto
}

/// La pompe du Kraken — le canal de l'incident du 2026-08-15.
const POMPE: &str = "kraken2023elite:pump-speed";

/// Le ventilateur du Kraken, son voisin sur le même contrôleur.
const VENTILATEUR: &str = "kraken2023elite:fan-speed";

// ---------------------------------------------------------------------------
// 1 — sans courbe, la fenêtre n'offre rien
// ---------------------------------------------------------------------------

#[test]
fn un_canal_sans_courbe_posee_ne_propose_pas_auto_a_la_fenetre() {
    // issue #97, comportement attendu — « La fenêtre n'offre plus "auto" sur un
    // canal sans courbe téléversée ».
    //
    // Les deux canaux du Kraken *savent* passer en automatique — c'est ce que #50
    // a établi, et ça n'a pas changé : leur pilote exécute bien `pwm_enable = 2`.
    // Ce que #97 ajoute, c'est qu'il l'exécute sur **un tableau de zéros** tant
    // qu'aucune courbe n'a été posée, ce qui arrête la régulation de la pompe.
    let sysfs = arborescence_de_reference("sans_courbe");
    let canaux = sysfs.canaux();
    let posees = CourbesPosees::vide();

    for nom in [POMPE, VENTILATEUR] {
        let c = canal(&canaux, nom);
        assert!(
            c.sait_faire_auto(),
            "{nom} : son pilote sait exécuter une courbe (#50)"
        );
        assert!(
            !propose_auto(c, &posees),
            "{nom} : sans courbe posée, le bouton ne peut qu'arrêter la régulation"
        );
    }
}

#[test]
fn un_pilote_sans_mode_automatique_ne_propose_toujours_rien() {
    // #50, critère d'acceptation — « Sur les trois canaux `nzxtsmart2`, "auto"
    // n'est pas proposé : le matériel n'en a pas ». #97 ne rouvre rien : il
    // **ferme** en plus. Sans ce test, une implémentation qui remplacerait la
    // question du pilote par celle de la courbe passerait tout le reste du
    // fichier.
    let sysfs = arborescence_de_reference("sans_pilote");
    let canaux = sysfs.canaux();
    let posees = CourbesPosees::vide();

    for nom in ["nzxtsmart2:fan-1", "nzxtsmart2:fan-2", "nzxtsmart2:fan-3"] {
        assert!(
            !propose_auto(canal(&canaux, nom), &posees),
            "{nom} : ce contrôleur n'a aucun mode automatique"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — une courbe posée rouvre le bouton, sur ce canal seul
// ---------------------------------------------------------------------------

#[test]
fn une_courbe_posee_rouvre_le_bouton_sur_ce_canal_et_lui_seul() {
    // issue #97, tests d'intention — « La mémoire est **par canal** : une courbe
    // posée sur `pump-speed` n'autorise pas `HostCurve` sur `fan-speed` ».
    //
    // Vu depuis la fenêtre : deux boutons voisins, sur le même contrôleur, dont un
    // seul s'allume. Les proposer ensemble enverrait le ventilateur sur le tableau
    // de zéros que la pompe vient d'éviter.
    let sysfs = arborescence_de_reference("courbe_posee");
    let canaux = sysfs.canaux();
    let pompe = canal(&canaux, POMPE);
    let ventilateur = canal(&canaux, VENTILATEUR);
    let mut posees = CourbesPosees::vide();

    set_curve(pompe, &courbe_de_reference(), &mut posees).expect("la courbe de la pompe part");

    assert!(
        propose_auto(pompe, &posees),
        "la pompe a reçu sa courbe : le bouton marcherait"
    );
    assert!(
        !propose_auto(ventilateur, &posees),
        "le ventilateur n'a pas reçu la sienne, quoique sur le même contrôleur"
    );
}

#[test]
fn le_bouton_repart_ferme_au_redemarrage_du_demon() {
    // issue #97, approche technique — la mémoire « est **remis à zéro au
    // démarrage** — c'est correct, puisqu'on ne peut rien savoir de ce qu'un autre
    // outil a écrit ».
    //
    // Un carnet vide **est** un démon qui redémarre. Le matériel, lui, est resté
    // où il était : c'est justement l'état qu'il ne faut pas prendre pour une
    // preuve.
    let sysfs = arborescence_de_reference("redemarrage");
    let canaux = sysfs.canaux();
    let pompe = canal(&canaux, POMPE);

    let mut premiere_vie = CourbesPosees::vide();
    set_curve(pompe, &courbe_de_reference(), &mut premiere_vie).expect("la courbe part");
    assert!(propose_auto(pompe, &premiere_vie));

    let seconde_vie = CourbesPosees::vide();
    assert!(
        !propose_auto(pompe, &seconde_vie),
        "un démon qui redémarre ne sait plus quelle courbe le pilote détient"
    );
}

// ---------------------------------------------------------------------------
// 3 — la fenêtre et le socket ne se contredisent jamais
// ---------------------------------------------------------------------------

#[test]
fn ce_que_la_fenetre_recoit_ne_ment_pas_sur_ce_que_le_socket_accepte() {
    // README, à propos de `nzxt-smart2` — « montrer un bouton qui ne peut
    // qu'échouer vaut moins que ne pas le montrer ». La réciproque compte autant :
    // cacher un bouton qui marcherait est une capacité perdue sans le dire.
    //
    // Le relevé et l'écriture sont deux chemins distincts — l'un sert `status`,
    // l'autre `fan … auto` — et rien ne les tient d'accord à part ce test. Le
    // carnet ne porte qu'une seule courbe : les quatre cas sont donc couverts d'un
    // coup — sait et a sa courbe, sait sans courbe, ne sait pas, n'a pas de fichier
    // de mode.
    let sysfs = arborescence_de_reference("pas_de_contradiction");
    let canaux = sysfs.canaux();
    let mut posees = CourbesPosees::vide();
    set_curve(canal(&canaux, POMPE), &courbe_de_reference(), &mut posees)
        .expect("la courbe de la pompe part");

    for c in &canaux {
        let promis = propose_auto(c, &posees);
        let tenu = set_mode(c, Mode::HostCurve, &posees).is_ok();
        assert_eq!(
            promis, tenu,
            "{} : la fenêtre reçoit {promis}, le socket rend {tenu}",
            c.name
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — le reste de la ligne continue d'être relevé
// ---------------------------------------------------------------------------

#[test]
fn le_reste_de_la_ligne_chan_ne_bouge_pas() {
    // Le contrepoids indispensable : sans lui, une implémentation qui cesserait de
    // relever les canaux — donc qui débrancherait le panneau VENTILOS — passerait
    // tous les tests ci-dessus, puisque « pas de bouton » y est la bonne réponse.
    //
    // #88 le disait déjà pour la quarantaine : « un canal illisible affiché à
    // 0 tr/min est un mensonge, et un canal omis fait croire qu'il n'existe pas ».
    let sysfs = arborescence_de_reference("ligne_complete");
    let canaux = sysfs.canaux();
    let posees = CourbesPosees::vide();

    let lecture = lire_le_canal(canal(&canaux, POMPE), &posees)
        .expect("le canal de l'arborescence temporaire répond");

    assert_eq!(
        lecture.rpm,
        Some(2380),
        "le régime relevé reste celui de `fan1_input`"
    );
    assert!(
        lecture.pwm.is_some(),
        "la consigne reste relevée depuis `pwm1`"
    );
    assert!(
        !lecture.mode.trim().is_empty(),
        "le mode reste rendu, et il est lisible"
    );
    assert!(
        !lecture.sait_faire_auto,
        "seul le booléen du bouton change, et il dit non"
    );
}
