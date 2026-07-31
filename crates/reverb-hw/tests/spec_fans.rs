//! Tests d'intention du contrôle de la vitesse des ventilateurs (issue #7).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #7, son commentaire de
//! clôture de la session matérielle et `docs/VENTILATEURS.md` seuls. Rien n'est
//! relu depuis `crates/reverb-cli/src/` : à l'écriture de ce fichier, le module
//! `reverb_hw::hwmon` n'existe pas.
//!
//! Comme pour `spec.rs` (#1), `spec_modes.rs` (#3) et `spec_led.rs` (#5) : ces
//! tests encodent ce que le logiciel doit faire, pas ce que le code fait. Si
//! l'un d'eux échoue après implémentation, c'est le code qu'on corrige, jamais
//! le test.
//!
//! # Aucun accès matériel
//!
//! issue #7, critère d'acceptation — « Aucun accès matériel dans les tests
//! automatisés », et « La découverte est testée contre une **fausse
//! arborescence sysfs**, comme `hidraw::discover_in` ».
//!
//! Les seuls chemins touchés ici sont ceux d'une arborescence construite dans
//! `std::env::temp_dir()`, effacée à la fin de chaque test. **Rien n'est lu ni
//! écrit sous `/sys`.** Aucune dépendance n'est ajoutée pour cela : `std::fs`
//! suffit.
//!
//! # Ce que ces tests ne vérifient pas
//!
//! - **Les valeurs relevées sur la machine illustrent, elles ne spécifient
//!   pas.** `docs/VENTILATEURS.md` donne « ~725 tr/min » pour FAN 1 ; exiger ce
//!   régime serait faux au prochain démarrage. Les régimes n'apparaissent ici
//!   que comme **contenus de fichiers** de l'arborescence temporaire, pour
//!   vérifier qu'un canal est associé au bon tachymètre — jamais comme
//!   attente sur le matériel.
//! - **La répartition des dix ventilateurs sur les quatre canaux de vitesse.**
//!   `docs/VENTILATEURS.md`, « Combien de ventilateurs par canal ? » : inconnue,
//!   deux mesures échouées, et « cette inconnue ne change aucune ligne de
//!   code ». Rien à tester.
//! - **La réaction d'un ventilateur à une consigne.** Le critère « appliquée,
//!   vérifiée par la variation du régime » se vérifie à la main, sur le
//!   matériel. Ce qui est vérifié ici, c'est que l'octet écrit dans `pwmN` est
//!   le bon, et qu'il n'est écrit que là.
//! - **Les critères qui portent sur la ligne de commande** — refus sans
//!   `--manual`, `--auto`, `--all`, canal inconnu listant les canaux valides,
//!   message d'absence de droits. Le contrat d'API de cette issue décrit
//!   `hwmon.rs` ; aucune signature ne correspond à la surface CLI. Elle reste à
//!   couvrir quand elle sera arrêtée (voir le compte rendu de session).
//!
//! # Arbitrages de contrat
//!
//! Le contrat d'API donne `Mode` mais aucune fonction pour l'obtenir. Arbitré
//! en session : le mode est lu par une méthode du canal,
//! `FanChannel::mode(&self) -> std::io::Result<Mode>` — un accès disque, donc
//! faillible, comme `discover_in`. `Mode::Unsupported` répond exactement à
//! « la source n'expose pas de `pwmN_enable` », soit `enable == None`.
//!
//! Le **plancher de 20 % est un arbitrage de la commande, pas du type** :
//! `Percent::new(5)` réussit, sinon `--force` n'aurait aucun moyen de
//! s'exprimer. `Percent::FLOOR` n'est ici qu'une constante.
//!
//! `set_pwm` et `set_mode` ne prennent le canal que **par référence** : les
//! chemins écrits viennent donc uniquement de la découverte. Le garde-fou
//! « l'écriture ne touche jamais un fichier que la découverte n'a pas listé »
//! (issue #7, approche technique) est une propriété des signatures ; les tests
//! du module `ecriture` vérifient qu'elle tient aussi à l'exécution.
//!
//! Enfin, `Percent::percent(self)` et `Percent::raw(self)` prennent `self` par
//! valeur : `Percent` est donc `Copy`. Et `Mode`, `Percent` et `PercentError`
//! sont attendus `Debug`, comme tous les types publics du dépôt, pour que les
//! échecs soient lisibles.

use std::fs;
use std::path::{Path, PathBuf};

use reverb_hw::hwmon::{self, FanChannel, Mode};

// ---------------------------------------------------------------------------
// Fausse arborescence sysfs
// ---------------------------------------------------------------------------

/// Une fausse `/sys/class/hwmon` dans un répertoire temporaire.
///
/// Le répertoire porte le nom du test et le PID : deux tests qui tournent en
/// parallèle ne se marchent pas dessus. Il est effacé à la destruction, y
/// compris quand le test échoue.
struct FauxSysfs {
    racine: PathBuf,
}

impl FauxSysfs {
    fn neuf(nom_du_test: &str) -> Self {
        let racine = std::env::temp_dir().join(format!(
            "reverb-spec-fans-{nom_du_test}-{}",
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

    fn noms(&self) -> Vec<String> {
        self.canaux().into_iter().map(|c| c.name).collect()
    }
}

impl Drop for FauxSysfs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.racine);
    }
}

/// Écrit un fichier sysfs.
///
/// Le saut de ligne final n'est pas décoratif : le noyau en ajoute un à chaque
/// attribut. Une lecture qui ne le retire pas produit une source
/// `"nzxtsmart2\n"` et un libellé `"FAN 1\n"`.
fn ecrire(chemin: &Path, valeur: &str) {
    fs::write(chemin, format!("{valeur}\n")).expect("écriture d'un fichier sysfs simulé");
}

/// Contenu d'un fichier de l'arborescence temporaire, sans son saut de ligne.
fn lire(chemin: &Path) -> String {
    fs::read_to_string(chemin)
        .expect("lecture d'un fichier sysfs simulé")
        .trim()
        .to_owned()
}

/// Photographie de toute l'arborescence : chemin relatif → contenu.
///
/// Sert à vérifier qu'une écriture n'a touché que le fichier visé. Comparer
/// deux photographies est plus sûr que de relire les quelques fichiers
/// auxquels on aurait pensé : un fichier oublié serait un fichier non
/// surveillé.
fn photographie(racine: &Path) -> Vec<(String, String)> {
    let mut vue = Vec::new();
    let mut a_visiter = vec![racine.to_owned()];
    while let Some(dossier) = a_visiter.pop() {
        for entree in fs::read_dir(&dossier).expect("parcours de l'arborescence temporaire") {
            let chemin = entree.expect("entrée de répertoire").path();
            if chemin.is_dir() {
                a_visiter.push(chemin);
            } else {
                let relatif = chemin
                    .strip_prefix(racine)
                    .expect("chemin sous la racine")
                    .to_string_lossy()
                    .into_owned();
                vue.push((relatif, lire(&chemin)));
            }
        }
    }
    vue.sort();
    vue
}

/// Les fichiers apparus, disparus ou modifiés entre deux photographies.
fn ecarts(avant: &[(String, String)], apres: &[(String, String)]) -> Vec<String> {
    let mut noms: Vec<String> = apres
        .iter()
        .filter(|(chemin, contenu)| {
            !avant
                .iter()
                .any(|(ancien, valeur)| ancien == chemin && valeur == contenu)
        })
        .map(|(chemin, _)| chemin.clone())
        .collect();
    noms.extend(
        avant
            .iter()
            .filter(|(chemin, _)| !apres.iter().any(|(nouveau, _)| nouveau == chemin))
            .map(|(chemin, _)| chemin.clone()),
    );
    noms.sort();
    noms.dedup();
    noms
}

/// Un canal complet : `pwmN`, `pwmN_enable`, `fanN_input`, `fanN_label`.
fn canal_complet(hwmon: &Path, n: u32, libelle: &str) {
    ecrire(&hwmon.join(format!("pwm{n}")), "64");
    ecrire(&hwmon.join(format!("pwm{n}_enable")), "1");
    ecrire(&hwmon.join(format!("fan{n}_input")), "700");
    ecrire(&hwmon.join(format!("fan{n}_label")), libelle);
}

/// L'arborescence de référence, reprise de `docs/VENTILATEURS.md`.
///
/// Les deux premières sources sont celles du relevé, avec leurs cinq canaux
/// pilotables. Les trois autres couvrent les formes que la découverte doit
/// savoir traiter : une source sans aucun `pwm`, une source sans
/// `pwmN_enable`, un canal sans `fanN_label` ni `fanN_input`.
///
/// Les numéros de hwmon ne suivent **pas** l'ordre alphabétique des sources :
/// `hwmon0` est `nvme` et `hwmon9` la dernière source ajoutée. L'ordre de
/// lecture du répertoire ne peut donc pas être confondu avec l'ordre attendu.
fn arborescence_de_reference(nom_du_test: &str) -> FauxSysfs {
    let sysfs = FauxSysfs::neuf(nom_du_test);

    // docs/VENTILATEURS.md, table du résumé — `nzxtsmart2`, trois canaux
    // « FAN 1/2/3 » en mode manuel. issue #7, « État relevé sur la machine » :
    // `hwmon4  nzxtsmart2  pwm1/2/3 = 64  enable=1`.
    // Les régimes sont distincts d'un canal à l'autre pour que l'appariement
    // `pwmN` ↔ `fanN_input` soit vérifiable ; ce ne sont pas des attentes sur
    // le matériel.
    let nzxt = sysfs.hwmon("hwmon4", "nzxtsmart2");
    for (n, libelle, regime) in [
        (1, "FAN 1", "725"),
        (2, "FAN 2", "688"),
        (3, "FAN 3", "715"),
    ] {
        ecrire(&nzxt.join(format!("pwm{n}")), "64");
        ecrire(&nzxt.join(format!("pwm{n}_enable")), "1");
        ecrire(&nzxt.join(format!("fan{n}_input")), regime);
        ecrire(&nzxt.join(format!("fan{n}_label")), libelle);
    }

    // docs/VENTILATEURS.md — `kraken2023elite`, « Pump speed » et « Fan speed »,
    // tous deux en courbe firmware (`enable = 0`). issue #7, « État relevé » :
    // `pwm1 = 171`, `pwm2 = 71`.
    let kraken = sysfs.hwmon("hwmon6", "kraken2023elite");
    for (n, libelle, pwm, regime) in [
        (1, "Pump speed", "171", "2380"),
        (2, "Fan speed", "71", "714"),
    ] {
        ecrire(&kraken.join(format!("pwm{n}")), pwm);
        ecrire(&kraken.join(format!("pwm{n}_enable")), "0");
        ecrire(&kraken.join(format!("fan{n}_input")), regime);
        ecrire(&kraken.join(format!("fan{n}_label")), libelle);
    }
    // Bruit : la courbe firmware à 40 points, hors scope (issue #7,
    // « Hors scope » ; docs/VENTILATEURS.md, « Ce que le matériel sait faire
    // sans nous »), et les mesures d'intensité que le firmware remonte à zéro
    // (docs/VENTILATEURS.md, « par l'intensité »). Aucun de ces fichiers n'est
    // un canal.
    ecrire(&kraken.join("temp1_input"), "31000");
    ecrire(&kraken.join("temp1_auto_point1_pwm"), "60");
    ecrire(&kraken.join("curr1_input"), "0");

    // Une source sans aucune sortie PWM. Elle occupe une place dans
    // `/sys/class/hwmon` sans rien offrir à piloter.
    let nvme = sysfs.hwmon("hwmon0", "nvme");
    ecrire(&nvme.join("temp1_input"), "38850");
    ecrire(&nvme.join("temp2_input"), "38850");

    // docs/VENTILATEURS.md, « Les prises de la carte mère sont vides » —
    // `nct6687` « n'expose **aucun `pwm*_enable`** ». Deux canaux suffisent à le
    // montrer (le pilote en expose huit). `pwm1_mode` est un voisin qui
    // commence par `pwm` sans être une sortie.
    let nct = sysfs.hwmon("hwmon2", "nct6687");
    for (n, libelle) in [(1, "CPU FAN"), (2, "SYS FAN 1")] {
        ecrire(&nct.join(format!("pwm{n}")), "153");
        ecrire(&nct.join(format!("fan{n}_input")), "0");
        ecrire(&nct.join(format!("fan{n}_label")), libelle);
    }
    ecrire(&nct.join("pwm1_mode"), "1");

    // Un canal dépouillé : ni `fan1_label`, ni `fan1_input`. Le libellé doit
    // retomber sur « fan1 » (contrat d'API) et le tachymètre rester absent.
    // `pwm1_enable = 2` n'est employé par aucune des deux sources du relevé.
    let autre = sysfs.hwmon("hwmon9", "amdgpu");
    ecrire(&autre.join("pwm1"), "128");
    ecrire(&autre.join("pwm1_enable"), "2");

    sysfs
}

/// Les huit canaux de l'arborescence de référence, dans l'ordre attendu.
const NOMS_ATTENDUS: [&str; 8] = [
    "amdgpu:fan1",
    "kraken2023elite:fan-speed",
    "kraken2023elite:pump-speed",
    "nct6687:cpu-fan",
    "nct6687:sys-fan-1",
    "nzxtsmart2:fan-1",
    "nzxtsmart2:fan-2",
    "nzxtsmart2:fan-3",
];

/// Les cinq canaux réellement pilotables sur la machine.
///
/// docs/VENTILATEURS.md, table du résumé ; issue #7, sortie attendue de
/// `reverb fans`.
const CANAUX_DU_MATERIEL: [&str; 5] = [
    "nzxtsmart2:fan-1",
    "nzxtsmart2:fan-2",
    "nzxtsmart2:fan-3",
    "kraken2023elite:pump-speed",
    "kraken2023elite:fan-speed",
];

/// Le canal nommé `nom`, ou un échec explicite.
fn canal<'a>(canaux: &'a [FanChannel], nom: &str) -> &'a FanChannel {
    canaux
        .iter()
        .find(|c| c.name == nom)
        .unwrap_or_else(|| panic!("le canal « {nom} » doit être découvert"))
}

/// Le mode d'un canal, lu dans l'arborescence temporaire.
fn mode(canal: &FanChannel) -> Mode {
    canal
        .mode()
        .expect("lecture du mode sur l'arborescence temporaire")
}

// ---------------------------------------------------------------------------

mod libelles {
    use reverb_hw::hwmon::slug;

    #[test]
    fn les_espaces_deviennent_des_tirets_et_les_majuscules_des_minuscules() {
        // Contrat d'API — « Met un libellé en kebab-case ASCII : "Pump speed"
        // -> "pump-speed" ».
        // issue #7, sortie attendue de `reverb fans` — la colonne LIBELLÉ porte
        // « FAN 1 », « Pump speed », « Fan speed », et la colonne CANAL les
        // rend en `fan-1`, `pump-speed`, `fan-speed`.
        assert_eq!(slug("Pump speed"), "pump-speed");
        assert_eq!(slug("Fan speed"), "fan-speed");
        assert_eq!(slug("FAN 1"), "fan-1");
        assert_eq!(slug("FAN 2"), "fan-2");
        assert_eq!(slug("FAN 3"), "fan-3");
    }

    #[test]
    fn un_libelle_sans_espace_ni_majuscule_est_inchange() {
        // Contrat d'API — le libellé de repli est « fanN », déjà en kebab-case
        // ASCII : le passer par `slug` ne doit rien lui faire.
        assert_eq!(slug("fan1"), "fan1");
        assert_eq!(slug("pump-speed"), "pump-speed");
    }

    #[test]
    fn pas_de_tiret_en_tete_ni_en_queue() {
        // Un nom de canal se tape à la main (issue #7, « avec le nom exact à
        // réutiliser ») : `nzxtsmart2:-fan-1-` serait impraticable.
        // Le saut de ligne est celui que le noyau ajoute à `fanN_label` — même
        // si la lecture l'a déjà retiré, `slug` ne doit pas en faire un tiret.
        assert_eq!(slug(" FAN 1 "), "fan-1");
        assert_eq!(slug("FAN 1\n"), "fan-1");
        assert_eq!(slug("  Pump speed  "), "pump-speed");
    }

    #[test]
    fn pas_de_tirets_doubles() {
        // Même raison : le nom doit être reproductible de tête. « fan--1 » ne
        // l'est pas.
        assert_eq!(slug("FAN  1"), "fan-1");
        assert_eq!(slug("Pump   speed"), "pump-speed");
        assert_eq!(slug("Pump - speed"), "pump-speed");
    }

    #[test]
    fn deux_libelles_qui_ne_different_que_par_la_casse_donnent_le_meme_slug() {
        // Contrat d'API — « minuscules ». Les sources ne s'accordent pas sur la
        // casse : `nzxtsmart2` écrit « FAN 1 », `kraken2023elite` « Fan speed ».
        // L'utilisateur ne doit pas avoir à s'en souvenir.
        assert_eq!(slug("FAN 1"), slug("fan 1"));
        assert_eq!(slug("FAN 1"), slug("Fan 1"));
        assert_eq!(slug("PUMP SPEED"), slug("Pump speed"));
    }

    #[test]
    fn le_slug_est_stable_par_reapplication() {
        // Conséquence des règles ci-dessus : minuscules, pas de tiret en bord,
        // pas de tiret double. Un slug est déjà un slug.
        for libelle in ["FAN 1", "Pump speed", "Fan speed", "fan1", " SYS FAN 1 "] {
            let une_fois = slug(libelle);
            assert_eq!(
                slug(&une_fois),
                une_fois,
                "« {libelle} » : le slug doit être son propre point fixe"
            );
        }
    }

    #[test]
    fn un_slug_ne_contient_que_des_minuscules_des_chiffres_et_des_tirets() {
        // Contrat d'API — « kebab-case ASCII ». Le slug est la moitié droite
        // d'un nom de canal, tapé au clavier : il ne doit rien contenir qui
        // demande une touche morte ou un guillemet.
        for libelle in [
            "FAN 1",
            "FAN 2",
            "FAN 3",
            "Pump speed",
            "Fan speed",
            "CPU FAN",
            "SYS FAN 1",
            "fan1",
        ] {
            let s = slug(libelle);
            assert!(
                s.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "« {libelle} » donne « {s} », qui sort de [a-z0-9-]"
            );
            assert!(
                !s.is_empty(),
                "« {libelle} » ne doit pas donner un slug vide"
            );
        }
    }
}

// ---------------------------------------------------------------------------

mod noms_de_canaux {
    use super::{
        CANAUX_DU_MATERIEL, FauxSysfs, NOMS_ATTENDUS, arborescence_de_reference, canal,
        canal_complet,
    };
    use reverb_hw::hwmon::slug;

    #[test]
    fn le_nom_est_la_source_puis_le_libelle_en_kebab_case() {
        // issue #7 — « Le nom est `<source>:<libellé en kebab-case>` ».
        // Contrat d'API — `name`, « le nom que l'utilisateur tape ».
        let sysfs = arborescence_de_reference("nom_source_libelle");
        for c in sysfs.canaux() {
            assert_eq!(
                c.name,
                format!("{}:{}", c.source, slug(&c.label)),
                "le nom d'un canal se recompose depuis sa source et son libellé"
            );
        }
    }

    #[test]
    fn les_cinq_canaux_du_materiel_portent_les_noms_de_l_issue() {
        // issue #7, sortie attendue de `reverb fans` — la colonne CANAL donne
        // les noms exacts, « avec le nom exact à réutiliser ».
        // docs/VENTILATEURS.md — « Cinq canaux sont pilotables sous Linux ».
        let sysfs = arborescence_de_reference("noms_du_materiel");
        let noms = sysfs.noms();
        for attendu in CANAUX_DU_MATERIEL {
            assert!(
                noms.iter().any(|n| n == attendu),
                "« {attendu} » doit figurer dans {noms:?}"
            );
        }
    }

    #[test]
    fn le_nom_est_toujours_complet_meme_sans_ambiguite() {
        // issue #7 — « Le nom est `<source>:<libellé en kebab-case>`, **toujours
        // complet**. […] un nom ambigu qui désigne le mauvais ventilateur est
        // pire que verbeux. » Aucun raccourci n'est rendu, même quand une seule
        // source porte ce libellé.
        let sysfs = arborescence_de_reference("nom_complet");
        for c in sysfs.canaux() {
            assert!(
                c.name.starts_with(&format!("{}:", c.source)),
                "« {} » doit commencer par sa source « {} »",
                c.name,
                c.source
            );
            assert_eq!(
                c.name.matches(':').count(),
                1,
                "« {}, » un seul séparateur",
                c.name
            );
        }
        assert_eq!(NOMS_ATTENDUS.len(), sysfs.canaux().len());
    }

    #[test]
    fn deux_sources_qui_nomment_un_canal_fan_1_produisent_deux_noms_distincts() {
        // issue #7 — « Deux sources peuvent nommer un canal "FAN 1" ». C'est la
        // justification du nom toujours complet ; ce test la met en situation.
        let sysfs = FauxSysfs::neuf("homonymes");
        canal_complet(&sysfs.hwmon("hwmon0", "nzxtsmart2"), 1, "FAN 1");
        canal_complet(&sysfs.hwmon("hwmon1", "nct6687"), 1, "FAN 1");

        let noms = sysfs.noms();
        assert_eq!(noms, vec!["nct6687:fan-1", "nzxtsmart2:fan-1"]);
        assert_ne!(noms[0], noms[1], "deux canaux homonymes restent distincts");
    }

    #[test]
    fn le_libelle_absent_retombe_sur_fan_n() {
        // Contrat d'API — `label`, « lu dans `fanN_label` ; à défaut "fanN" ».
        // Le canal reste nommable : sans repli, il serait découvert et
        // inatteignable.
        let sysfs = arborescence_de_reference("libelle_de_repli");
        let canaux = sysfs.canaux();
        let sans_libelle = canal(&canaux, "amdgpu:fan1");

        assert_eq!(sans_libelle.label, "fan1");
        assert_eq!(sans_libelle.source, "amdgpu");
    }
}

// ---------------------------------------------------------------------------

mod pourcentage {
    use reverb_hw::hwmon::{Percent, PercentError};

    /// La consigne `p`, qui doit être acceptée.
    fn consigne(p: u8) -> Percent {
        Percent::new(p).expect("une consigne de 0 à 100 % est valide")
    }

    #[test]
    fn zero_pour_cent_vaut_zero_et_cent_pour_cent_vaut_255() {
        // issue #7, critère d'acceptation — « La consigne est exprimée en
        // **pourcent** ; la conversion vers l'échelle `0-255` du noyau est
        // testée **aux bornes** et à mi-course ».
        // Contrat d'API — `raw`, « Échelle du noyau, 0 à 255 ».
        assert_eq!(consigne(0).raw(), 0);
        assert_eq!(consigne(100).raw(), 255);
        assert_eq!(Percent::from_raw(0).percent(), 0);
        assert_eq!(Percent::from_raw(255).percent(), 100);
    }

    #[test]
    fn la_conversion_est_lineaire_a_une_unite_pres() {
        // issue #7, même critère — « et à mi-course ». 255 ne se divise pas par
        // 100 : la valeur exacte de 50 % est 127,5. Le test borne l'écart au
        // lieu de trancher entre 127 et 128, arbitrage que l'issue ne fait pas.
        for p in 0u8..=100 {
            let brut = i32::from(consigne(p).raw());
            let exact = i32::from(p) * 255;
            assert!(
                (brut * 100 - exact).abs() <= 100,
                "{p} % devrait valoir {:.2} sur 255, pas {brut}",
                f64::from(exact) / 100.0
            );
        }
        assert!(
            [127, 128].contains(&consigne(50).raw()),
            "à mi-course, 127 ou 128 : {}",
            consigne(50).raw()
        );
    }

    #[test]
    fn la_conversion_est_monotone() {
        // Une consigne plus forte ne doit jamais donner une valeur brute plus
        // faible — sinon `reverb fan --pwm 61` souffle moins que `--pwm 60`.
        let mut precedent = 0;
        for p in 0u8..=100 {
            let brut = consigne(p).raw();
            assert!(
                brut >= precedent,
                "{p} % donne {brut}, en recul sur {precedent}"
            );
            precedent = brut;
        }
    }

    #[test]
    fn la_relecture_du_noyau_reproduit_les_pourcentages_de_l_issue() {
        // issue #7, « État relevé sur la machine » — le noyau rend des valeurs
        // brutes que l'issue traduit elle-même :
        //     pwm1/2/3 = 64  (25 %)     nzxtsmart2
        //     pwm1     = 171 (67 %)     kraken2023elite, pompe
        //     pwm2     = 71  (28 %)     kraken2023elite, ventilateur
        // et la sortie attendue de `reverb fans` affiche bien « 25% », « 67% »,
        // « 28% » dans la colonne PWM. C'est ce que `from_raw` doit rendre.
        assert_eq!(Percent::from_raw(64).percent(), 25);
        assert_eq!(Percent::from_raw(171).percent(), 67);
        assert_eq!(Percent::from_raw(71).percent(), 28);
    }

    #[test]
    fn l_aller_retour_pourcent_brut_pourcent_reste_a_un_point_pres() {
        // Contrat d'API — `from_raw`, « Reconstruit un pourcentage depuis la
        // valeur brute du noyau ». `reverb fans` relit ce que `reverb fan` a
        // écrit : afficher « 58% » après avoir demandé 60 serait un défaut.
        // L'écart d'arrondi est inévitable — 255 ne divise pas 100 — mais il
        // reste borné à un point. Au-delà, la conversion est fausse.
        for p in 0u8..=100 {
            let retour = Percent::from_raw(consigne(p).raw()).percent();
            assert!(
                i32::from(retour).abs_diff(i32::from(p)) <= 1,
                "{p} % relu à {retour} %, écart supérieur à un point"
            );
        }
        // Aux bornes, l'aller-retour est exact.
        assert_eq!(Percent::from_raw(consigne(0).raw()).percent(), 0);
        assert_eq!(Percent::from_raw(consigne(100).raw()).percent(), 100);
    }

    #[test]
    fn toute_consigne_de_zero_a_cent_est_acceptee_et_se_relit_telle_quelle() {
        // Contrat d'API — `percent`, l'accesseur du pourcentage. La consigne
        // rendue est celle qui a été donnée, sans repli ni écrêtage silencieux.
        for p in 0u8..=100 {
            assert_eq!(consigne(p).percent(), p);
        }
    }

    #[test]
    fn au_dela_de_cent_pour_cent_est_refuse() {
        // Contrat d'API — `new`, « Refuse au-dessus de 100 ».
        // issue #7 — « `reverb fan --channel <NOM> --pwm <0-100>` ». 150 %
        // n'existe pas ; écrire 255 quand on demande 150 serait obéir de
        // travers.
        for p in [101u8, 110, 150, 200, 255] {
            assert!(Percent::new(p).is_err(), "{p} % doit être refusé");
        }
        assert!(Percent::new(100).is_ok(), "100 % est la borne haute valide");
    }

    #[test]
    fn le_refus_cite_la_valeur_donnee() {
        // issue #7, critère d'acceptation du plancher — « refus explicite
        // **citant la valeur demandée** ». Même exigence pour la borne haute :
        // un message qui ne rappelle pas ce qui a été tapé oblige à relire sa
        // commande. Contrat d'API — `pub struct PercentError { pub given: u8 }`.
        for p in [101u8, 150, 255] {
            let erreur: PercentError =
                Percent::new(p).expect_err("au-dessus de 100 %, la consigne est refusée");
            assert_eq!(erreur.given, p);
            let message = erreur.to_string();
            assert!(
                message.contains(&p.to_string()),
                "le message doit citer {p} : « {message} »"
            );
        }
    }

    #[test]
    fn le_plancher_vaut_vingt_pour_cent() {
        // issue #7, critère d'acceptation — « **Plancher de 20 %** ».
        // Contrat d'API — `pub const FLOOR: u8 = 20`.
        assert_eq!(Percent::FLOOR, 20);
    }

    #[test]
    fn une_consigne_sous_le_plancher_reste_constructible() {
        // issue #7, critère d'acceptation — « en dessous, refus explicite citant
        // la valeur demandée, **sauf `--force`** ». Le plancher est donc un
        // arbitrage de la commande, pas une propriété du type : `--force` doit
        // pouvoir fabriquer un `Percent` de 5 %. Si `new` le refusait, l'option
        // n'aurait aucun moyen de s'exprimer.
        for p in 0u8..Percent::FLOOR {
            let sous_plancher = Percent::new(p)
                .unwrap_or_else(|_| panic!("{p} % reste une valeur représentable (--force)"));
            assert_eq!(sous_plancher.percent(), p);
        }
    }
}

// ---------------------------------------------------------------------------

mod decouverte {
    use super::{NOMS_ATTENDUS, arborescence_de_reference, canal, lire};

    #[test]
    fn la_liste_est_celle_attendue_et_triee_par_nom() {
        // issue #7, critère d'acceptation — « `reverb fans` liste **tous** les
        // canaux PWM découverts ».
        // Contrat d'API — `discover_in`, « Découvre les canaux sous une racine
        // sysfs donnée, **triés par nom** ». L'ordre de lecture d'un répertoire
        // n'est pas déterministe ; celui de la liste doit l'être.
        let sysfs = arborescence_de_reference("liste_complete");
        let noms = sysfs.noms();
        let noms: Vec<&str> = noms.iter().map(String::as_str).collect();

        assert_eq!(noms, NOMS_ATTENDUS.to_vec());

        let mut tries = noms.clone();
        tries.sort_unstable();
        assert_eq!(noms, tries, "la liste doit être triée par nom");
    }

    #[test]
    fn l_ordre_ne_depend_pas_du_numero_des_repertoires_hwmon() {
        // Les numéros de hwmon suivent l'ordre de chargement des pilotes, pas
        // un ordre stable — même piège que les `hidraw` du CLAUDE.md, « les
        // numéros changent au redémarrage, constaté, pas supposé ». Deux
        // arborescences identiques au numéro près rendent la même liste.
        use super::{FauxSysfs, canal_complet};

        let a = FauxSysfs::neuf("ordre_a");
        canal_complet(&a.hwmon("hwmon0", "nzxtsmart2"), 1, "FAN 1");
        canal_complet(&a.hwmon("hwmon1", "kraken2023elite"), 1, "Pump speed");

        let b = FauxSysfs::neuf("ordre_b");
        canal_complet(&b.hwmon("hwmon7", "kraken2023elite"), 1, "Pump speed");
        canal_complet(&b.hwmon("hwmon3", "nzxtsmart2"), 1, "FAN 1");

        assert_eq!(a.noms(), b.noms());
        assert_eq!(
            a.noms(),
            vec!["kraken2023elite:pump-speed", "nzxtsmart2:fan-1"]
        );
    }

    #[test]
    fn chaque_canal_porte_sa_source_et_son_libelle() {
        // Contrat d'API — `source`, « lu dans le fichier `name` » ; `label`,
        // « lu dans `fanN_label` ».
        // issue #7, critère d'acceptation — la liste donne « source, libellé,
        // régime, consigne et mode ».
        let sysfs = arborescence_de_reference("source_et_libelle");
        let canaux = sysfs.canaux();

        for (nom, source, libelle) in [
            ("nzxtsmart2:fan-1", "nzxtsmart2", "FAN 1"),
            ("nzxtsmart2:fan-2", "nzxtsmart2", "FAN 2"),
            ("nzxtsmart2:fan-3", "nzxtsmart2", "FAN 3"),
            (
                "kraken2023elite:pump-speed",
                "kraken2023elite",
                "Pump speed",
            ),
            ("kraken2023elite:fan-speed", "kraken2023elite", "Fan speed"),
        ] {
            let c = canal(&canaux, nom);
            assert_eq!(c.source, source, "source de {nom}");
            assert_eq!(c.label, libelle, "libellé de {nom}");
        }
    }

    #[test]
    fn les_valeurs_sysfs_sont_lues_sans_leur_saut_de_ligne() {
        // Le noyau termine chaque attribut par `\n`. Une source
        // « nzxtsmart2\n » casserait la comparaison de nom que fait
        // `--channel`, et sans message compréhensible.
        let sysfs = arborescence_de_reference("sans_saut_de_ligne");
        for c in sysfs.canaux() {
            assert!(
                !c.source.contains(['\n', '\r']),
                "source brute : {:?}",
                c.source
            );
            assert!(
                !c.label.contains(['\n', '\r']),
                "libellé brut : {:?}",
                c.label
            );
            assert!(!c.name.contains(['\n', '\r']), "nom brut : {:?}", c.name);
            assert_eq!(c.source.trim(), c.source);
            assert_eq!(c.label.trim(), c.label);
        }
    }

    #[test]
    fn un_hwmon_sans_pwm_ne_produit_aucun_canal() {
        // issue #7, approche technique — la découverte part des `pwmN`
        // trouvés. Une source qui ne rend que des températures n'a rien à
        // piloter ; `reverb fans` ne doit pas l'afficher.
        let sysfs = arborescence_de_reference("sans_pwm");
        assert!(
            sysfs.canaux().iter().all(|c| c.source != "nvme"),
            "une source sans `pwmN` ne donne aucun canal : {:?}",
            sysfs.noms()
        );
    }

    #[test]
    fn les_fichiers_voisins_de_pwm_ne_sont_pas_des_canaux() {
        // issue #7, approche technique — « pour chaque `pwmN` trouvé associe
        // `fanN_input`, `fanN_label` et `pwmN_enable` ». `pwm1_enable` et
        // `pwm1_mode` commencent par `pwm` sans être des sorties : les compter
        // doublerait la liste et ferait écrire une consigne dans un fichier de
        // mode.
        // docs/VENTILATEURS.md — trois canaux pour `nzxtsmart2`, deux pour
        // `kraken2023elite`.
        let sysfs = arborescence_de_reference("voisins_de_pwm");
        let canaux = sysfs.canaux();

        for (source, attendu) in [
            ("nzxtsmart2", 3),
            ("kraken2023elite", 2),
            ("nct6687", 2),
            ("amdgpu", 1),
            ("nvme", 0),
        ] {
            let compte = canaux.iter().filter(|c| c.source == source).count();
            assert_eq!(compte, attendu, "nombre de canaux de {source}");
        }
    }

    #[test]
    fn le_chemin_pwm_designe_le_fichier_de_consigne_du_canal() {
        // Contrat d'API — `pwm: PathBuf`, « .../pwmN ».
        // issue #7, approche technique — « l'écriture ne touche jamais un
        // fichier que la découverte n'a pas listé ». Ce chemin est le seul par
        // lequel une consigne partira : il doit désigner le bon fichier.
        let sysfs = arborescence_de_reference("chemin_pwm");
        let canaux = sysfs.canaux();

        for (nom, fichier, valeur) in [
            ("nzxtsmart2:fan-1", "pwm1", "64"),
            ("nzxtsmart2:fan-3", "pwm3", "64"),
            ("kraken2023elite:pump-speed", "pwm1", "171"),
            ("kraken2023elite:fan-speed", "pwm2", "71"),
        ] {
            let c = canal(&canaux, nom);
            assert_eq!(
                c.pwm.file_name().and_then(|f| f.to_str()),
                Some(fichier),
                "chemin de consigne de {nom}"
            );
            assert!(c.pwm.exists(), "{nom} : {} doit exister", c.pwm.display());
            assert_eq!(lire(&c.pwm), valeur, "{nom} pointe sur le bon fichier");
        }
    }

    #[test]
    fn l_indice_du_tachymetre_suit_celui_du_pwm() {
        // issue #7, approche technique — « pour chaque `pwmN` trouvé associe
        // `fanN_input` ». Croiser les indices afficherait le régime de la
        // pompe en face du ventilateur : les valeurs relues ci-dessous ne sont
        // là que pour distinguer les deux fichiers, pas pour spécifier un
        // régime.
        let sysfs = arborescence_de_reference("indice_du_tachymetre");
        let canaux = sysfs.canaux();

        for (nom, fichier, valeur) in [
            ("kraken2023elite:pump-speed", "fan1_input", "2380"),
            ("kraken2023elite:fan-speed", "fan2_input", "714"),
            ("nzxtsmart2:fan-2", "fan2_input", "688"),
        ] {
            let tach = canal(&canaux, nom)
                .tach
                .as_ref()
                .unwrap_or_else(|| panic!("{nom} a un tachymètre"));
            assert_eq!(
                tach.file_name().and_then(|f| f.to_str()),
                Some(fichier),
                "tachymètre de {nom}"
            );
            assert_eq!(lire(tach), valeur, "{nom} lit son propre tachymètre");
        }
    }

    #[test]
    fn un_canal_sans_tachymetre_n_en_invente_pas() {
        // Contrat d'API — `tach: Option<PathBuf>`, « absent si le canal n'en a
        // pas ». Un canal muet reste pilotable : c'est exactement le cas des
        // ventilateurs repiqués dont un seul fil tachymétrique remonte
        // (docs/VENTILATEURS.md, « les répartiteurs ne transmettant qu'un fil
        // tachymétrique »).
        let sysfs = arborescence_de_reference("sans_tachymetre");
        let canaux = sysfs.canaux();

        assert!(
            canal(&canaux, "amdgpu:fan1").tach.is_none(),
            "sans `fan1_input`, pas de tachymètre"
        );
        assert!(
            canal(&canaux, "nzxtsmart2:fan-1").tach.is_some(),
            "avec `fan1_input`, un tachymètre"
        );
    }

    #[test]
    fn le_chemin_enable_est_celui_du_canal_quand_il_existe() {
        // Contrat d'API — `enable: Option<PathBuf>`, « .../pwmN_enable, absent
        // si la source n'en expose pas ».
        // docs/VENTILATEURS.md — `nct6687` « n'expose aucun `pwm*_enable` ».
        let sysfs = arborescence_de_reference("chemin_enable");
        let canaux = sysfs.canaux();

        for (nom, fichier) in [
            ("nzxtsmart2:fan-2", "pwm2_enable"),
            ("kraken2023elite:pump-speed", "pwm1_enable"),
        ] {
            let enable = canal(&canaux, nom)
                .enable
                .as_ref()
                .unwrap_or_else(|| panic!("{nom} expose un fichier de mode"));
            assert_eq!(
                enable.file_name().and_then(|f| f.to_str()),
                Some(fichier),
                "fichier de mode de {nom}"
            );
        }

        for nom in ["nct6687:cpu-fan", "nct6687:sys-fan-1"] {
            assert!(
                canal(&canaux, nom).enable.is_none(),
                "{nom} : la source n'expose aucun `pwm*_enable`"
            );
        }
    }

    #[test]
    fn aucun_chemin_ne_sort_de_la_racine_donnee() {
        // issue #7, critère d'acceptation — « Aucun accès matériel dans les
        // tests automatisés », et approche technique — « l'écriture ne touche
        // jamais un fichier que la découverte n'a pas listé ». Si `discover_in`
        // rendait un chemin absolu vers `/sys`, un test finirait par écrire sur
        // le vrai matériel.
        let sysfs = arborescence_de_reference("chemins_confines");
        let racine = sysfs.racine();

        for c in sysfs.canaux() {
            let chemins = [Some(c.pwm), c.tach, c.enable];
            for chemin in chemins.into_iter().flatten() {
                assert!(
                    chemin.starts_with(racine),
                    "{} sort de la racine {}",
                    chemin.display(),
                    racine.display()
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------

mod modes {
    use super::{arborescence_de_reference, canal, mode};
    use reverb_hw::hwmon::Mode;

    #[test]
    fn enable_a_zero_est_la_courbe_firmware() {
        // issue #7, critère d'acceptation — « Un canal en courbe firmware
        // (`pwm_enable = 0`) n'est jamais basculé en manuel implicitement ».
        // docs/VENTILATEURS.md — « la pompe et le ventilateur du Kraken sont en
        // `pwm_enable = 0`, donc **sur leur courbe firmware** ». C'est la
        // lecture du mode qui rend ce refus possible : mal la faire, c'est
        // sortir la pompe de sa courbe sans le dire.
        let sysfs = arborescence_de_reference("mode_courbe_firmware");
        let canaux = sysfs.canaux();

        for nom in ["kraken2023elite:pump-speed", "kraken2023elite:fan-speed"] {
            let m = mode(canal(&canaux, nom));
            assert!(
                matches!(m, Mode::FirmwareCurve),
                "{nom} suit sa courbe firmware, lu : {m:?}"
            );
        }
    }

    #[test]
    fn enable_a_un_est_le_mode_manuel() {
        // issue #7, « État relevé sur la machine » — `nzxtsmart2 […] enable=1`,
        // et la sortie attendue de `reverb fans` affiche « manuel » pour ces
        // trois canaux.
        let sysfs = arborescence_de_reference("mode_manuel");
        let canaux = sysfs.canaux();

        for nom in ["nzxtsmart2:fan-1", "nzxtsmart2:fan-2", "nzxtsmart2:fan-3"] {
            let m = mode(canal(&canaux, nom));
            assert!(matches!(m, Mode::Manual), "{nom} est manuel, lu : {m:?}");
        }
    }

    #[test]
    fn la_valeur_deux_designe_la_courbe_de_l_hote() {
        // docs/VENTILATEURS.md, « ce que la sonde a établi » — `pwm_enable`
        // accepte `0`, `1` et `2`, refuse `3`, et `2` est le mode dans lequel
        // le firmware exécute la courbe téléversée par l'hôte. Mesuré le
        // 2026-07-30 : la pompe suit le pic de la courbe écrite.
        //
        // Ce test attendait `Unknown(2)` avant cette mesure, et c'était juste :
        // on ne devine pas ce qu'on n'a pas observé. La spec a bougé, le test
        // la suit. L'ordre est tenu — spec, puis test, puis code.
        let sysfs = arborescence_de_reference("mode_courbe_hote");
        let canaux = sysfs.canaux();

        let m = mode(canal(&canaux, "amdgpu:fan1"));
        assert!(
            matches!(m, Mode::HostCurve),
            "`pwm1_enable = 2` est le mode courbe de l'hôte, lu : {m:?}"
        );
    }

    #[test]
    fn l_absence_de_fichier_enable_n_est_ni_manuel_ni_courbe() {
        // docs/VENTILATEURS.md — `nct6687` « n'expose **aucun `pwm*_enable`**.
        // Même avec des ventilateurs branchés, il n'y aurait aucun moyen de
        // passer un canal en mode manuel proprement. »
        // Contrat d'API — `Mode::Unsupported`. Le confondre avec `Manual`
        // laisserait croire qu'un `--auto` sur ce canal ferait quelque chose.
        let sysfs = arborescence_de_reference("mode_absent");
        let canaux = sysfs.canaux();

        for nom in ["nct6687:cpu-fan", "nct6687:sys-fan-1"] {
            let c = canal(&canaux, nom);
            let m = mode(c);
            assert!(
                matches!(m, Mode::Unsupported),
                "{nom} n'a pas de fichier de mode, lu : {m:?}"
            );
            assert!(c.enable.is_none(), "{nom} : `enable` reste absent");
        }
    }

    #[test]
    fn le_mode_ne_depend_que_du_fichier_enable_du_canal() {
        // issue #7, approche technique — chaque `pwmN` est associé à **son**
        // `pwmN_enable`. Dans l'arborescence de référence, `nzxtsmart2` est en
        // `1` et `kraken2023elite` en `0` : un mode lu sur le mauvais fichier
        // se verrait immédiatement.
        let sysfs = arborescence_de_reference("mode_par_canal");
        let canaux = sysfs.canaux();

        let manuels = canaux
            .iter()
            .filter(|c| matches!(mode(c), Mode::Manual))
            .count();
        let courbes = canaux
            .iter()
            .filter(|c| matches!(mode(c), Mode::FirmwareCurve))
            .count();
        assert_eq!(manuels, 3, "les trois canaux de `nzxtsmart2`");
        assert_eq!(courbes, 2, "les deux canaux de `kraken2023elite`");
    }
}

// ---------------------------------------------------------------------------

mod ecriture {
    use super::{arborescence_de_reference, canal, ecarts, lire, mode, photographie};
    use reverb_hw::hwmon::{Mode, Percent, set_mode, set_pwm};

    /// La consigne `p`, qui doit être acceptée.
    fn consigne(p: u8) -> Percent {
        Percent::new(p).expect("une consigne de 0 à 100 % est valide")
    }

    #[test]
    fn set_pwm_ecrit_la_valeur_brute_du_noyau_et_non_le_pourcentage() {
        // Contrat d'API — `set_pwm`, « Écrit la consigne dans `pwmN` », et
        // `raw`, « Échelle du noyau, 0 à 255 ».
        // issue #7, critère d'acceptation — « La consigne est exprimée en
        // **pourcent** ; la conversion vers l'échelle `0-255` du noyau ».
        // Écrire « 60 » là où le noyau attend 153 donnerait un ventilateur à
        // 24 % pour une commande à 60 %, sans le moindre message.
        //
        // Le canal visé est en mode manuel (`enable = 1`) : `set_pwm` sur un
        // canal en courbe firmware est un arbitrage que l'issue ne tranche pas
        // (voir le compte rendu de session).
        let sysfs = arborescence_de_reference("set_pwm_valeur");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "nzxtsmart2:fan-2");

        // 0 et 100 sont les bornes de l'issue ; 20 % vaut exactement 51 sur 255
        // (20 × 2,55), quel que soit l'arrondi retenu.
        for (pourcent, brut) in [(0u8, "0"), (20, "51"), (100, "255")] {
            set_pwm(c, consigne(pourcent)).expect("écriture de la consigne");
            assert_eq!(lire(&c.pwm), brut, "{pourcent} % sur l'échelle du noyau");
        }

        // À mi-course, la valeur exacte est 127,5 : le test suit `raw()` plutôt
        // que de trancher entre 127 et 128, arbitrage que l'issue ne fait pas.
        set_pwm(c, consigne(50)).expect("écriture de la consigne");
        assert_eq!(lire(&c.pwm), consigne(50).raw().to_string());
        assert_ne!(lire(&c.pwm), "50", "c'est la valeur brute qui est écrite");
    }

    #[test]
    fn set_pwm_n_ecrit_que_dans_le_fichier_pwm_du_canal() {
        // issue #7, approche technique — « l'écriture ne touche jamais un
        // fichier que la découverte n'a pas listé ». `set_pwm` ne reçoit que le
        // canal : le seul chemin qu'elle peut légitimement ouvrir est son
        // `pwm`. Ni `pwm2_enable` — qui basculerait le mode dans le dos de
        // l'utilisateur, ce que le critère « jamais basculé en manuel
        // implicitement » interdit —, ni `fan2_input`, ni les autres `pwmM`.
        let sysfs = arborescence_de_reference("set_pwm_confine");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "nzxtsmart2:fan-2");

        let avant = photographie(sysfs.racine());
        set_pwm(c, consigne(40)).expect("écriture de la consigne");
        let apres = photographie(sysfs.racine());

        // `hwmon4` est le répertoire de `nzxtsmart2` dans l'arborescence de
        // référence ; sysfs n'existe que sous Linux, le séparateur est `/`.
        assert_eq!(ecarts(&avant, &apres), vec!["hwmon4/pwm2".to_owned()]);
    }

    #[test]
    fn set_pwm_sous_le_plancher_ecrit_quand_meme() {
        // issue #7, critère d'acceptation — « Plancher de 20 % : en dessous,
        // refus explicite […] **sauf `--force`** ». Le plancher est un
        // arbitrage de la commande : `set_pwm` est la primitive qu'emploie
        // `--force`, elle ne peut pas refuser 5 % elle-même.
        let sysfs = arborescence_de_reference("set_pwm_sous_plancher");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "nzxtsmart2:fan-1");

        let sous_plancher = consigne(Percent::FLOOR - 15);
        set_pwm(c, sous_plancher).expect("`--force` doit pouvoir descendre");
        assert_eq!(lire(&c.pwm), sous_plancher.raw().to_string());
    }

    #[test]
    fn set_mode_manuel_ecrit_un() {
        // issue #7, critère d'acceptation — « Sans `--manual`, la commande
        // échoue en expliquant que le canal suit sa courbe et **ce que
        // `--manual` ferait** ». Ce que `--manual` fait, c'est ceci : écrire 1
        // dans `pwmN_enable`.
        // docs/VENTILATEURS.md — « la pompe et le ventilateur du Kraken sont en
        // `pwm_enable = 0` […] Leur imposer une consigne fixe les en sort ».
        let sysfs = arborescence_de_reference("set_mode_manuel");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "kraken2023elite:pump-speed");
        let enable = c.enable.as_ref().expect("le Kraken expose `pwm1_enable`");

        assert_eq!(lire(enable), "0", "le canal part de sa courbe firmware");
        set_mode(c, Mode::Manual).expect("bascule explicite en manuel");
        assert_eq!(lire(enable), "1");
        assert!(
            matches!(mode(c), Mode::Manual),
            "le mode relu doit suivre le fichier écrit"
        );
    }

    #[test]
    fn set_mode_courbe_firmware_ecrit_zero() {
        // issue #7, critère d'acceptation — « `--auto` rend un canal à sa
        // courbe firmware (`pwm_enable = 0`) ».
        // docs/VENTILATEURS.md — « une courbe qui survit à l'extinction de
        // Reverb vaut mieux qu'un démon qui la surveille » : rendre la main au
        // firmware doit être aussi simple que la prendre.
        let sysfs = arborescence_de_reference("set_mode_auto");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "nzxtsmart2:fan-1");
        let enable = c
            .enable
            .as_ref()
            .expect("`nzxtsmart2` expose `pwm1_enable`");

        assert_eq!(lire(enable), "1", "le canal part du mode manuel");
        set_mode(c, Mode::FirmwareCurve).expect("retour à la courbe firmware");
        assert_eq!(lire(enable), "0");
    }

    #[test]
    fn set_mode_n_ecrit_que_dans_le_fichier_enable_du_canal() {
        // issue #7, approche technique — « l'écriture ne touche jamais un
        // fichier que la découverte n'a pas listé ». Un `--auto` ne doit pas
        // effacer la consigne au passage : `pwm1` reste ce qu'il était.
        let sysfs = arborescence_de_reference("set_mode_confine");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "nzxtsmart2:fan-1");

        let avant = photographie(sysfs.racine());
        set_mode(c, Mode::FirmwareCurve).expect("retour à la courbe firmware");
        let apres = photographie(sysfs.racine());

        assert_eq!(
            ecarts(&avant, &apres),
            vec!["hwmon4/pwm1_enable".to_owned()]
        );
        assert_eq!(lire(&c.pwm), "64", "la consigne n'est pas touchée");
    }

    #[test]
    fn set_mode_echoue_sur_un_canal_sans_fichier_enable_et_n_ecrit_nulle_part() {
        // Contrat d'API — `set_mode` « Échoue si le canal n'expose pas ce
        // fichier (`Mode::Unsupported`) ».
        // docs/VENTILATEURS.md — `nct6687` « n'expose **aucun `pwm*_enable`**.
        // Même avec des ventilateurs branchés, il n'y aurait aucun moyen de
        // passer un canal en mode manuel proprement. » Un échec silencieux
        // laisserait croire au contraire.
        let sysfs = arborescence_de_reference("set_mode_sans_enable");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "nct6687:cpu-fan");
        assert!(c.enable.is_none());

        let avant = photographie(sysfs.racine());
        for demande in [Mode::Manual, Mode::FirmwareCurve] {
            assert!(
                set_mode(c, demande).is_err(),
                "{demande:?} sur un canal sans `pwmN_enable`"
            );
        }
        let apres = photographie(sysfs.racine());

        assert!(
            ecarts(&avant, &apres).is_empty(),
            "un échec n'écrit nulle part : {:?}",
            ecarts(&avant, &apres)
        );
    }

    #[test]
    fn set_mode_refuse_une_valeur_inconnue_sans_rien_ecrire() {
        // Contrat d'API — `set_mode` « refuse d'écrire `Mode::Unknown` — on ne
        // réémet pas une valeur qu'on n'a pas comprise ».
        // CLAUDE.md — « Ne jamais implémenter depuis un ❓ […] dire qu'elle est
        // inconnue ». Le canal visé **expose** son `pwm1_enable` : le refus
        // vient donc du mode demandé, pas d'un fichier absent.
        let sysfs = arborescence_de_reference("set_mode_inconnu");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "nzxtsmart2:fan-1");
        assert!(c.enable.is_some());

        let avant = photographie(sysfs.racine());
        for valeur in [0u8, 2, 5, 255] {
            assert!(
                set_mode(c, Mode::Unknown(valeur)).is_err(),
                "`Mode::Unknown({valeur})` n'est pas réémis"
            );
        }
        let apres = photographie(sysfs.racine());

        assert!(
            ecarts(&avant, &apres).is_empty(),
            "un refus n'écrit nulle part : {:?}",
            ecarts(&avant, &apres)
        );
    }

    #[test]
    fn set_mode_refuse_un_mode_non_supporte_sans_rien_ecrire() {
        // Lecture **par extension** du contrat d'API : `Mode::Unsupported` ne
        // porte aucune valeur, il n'y a donc rien à écrire dans `pwmN_enable`.
        // Le contrat le nomme comme cause d'échec (« Échoue si le canal
        // n'expose pas ce fichier ») mais ne dit pas ce qu'il advient quand on
        // le passe en **argument**. À confirmer — voir le compte rendu de
        // session.
        let sysfs = arborescence_de_reference("set_mode_unsupported");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "nzxtsmart2:fan-1");

        let avant = photographie(sysfs.racine());
        assert!(
            set_mode(c, Mode::Unsupported).is_err(),
            "« non supporté » n'est pas une consigne de mode"
        );
        let apres = photographie(sysfs.racine());

        assert!(
            ecarts(&avant, &apres).is_empty(),
            "un refus n'écrit nulle part : {:?}",
            ecarts(&avant, &apres)
        );
    }

    #[test]
    fn ecrire_sur_un_canal_ne_modifie_aucun_autre_canal() {
        // issue #7, critère d'acceptation — « `--all` applique la consigne à
        // **tous** les canaux découverts ». Si écrire sur un canal en touchait
        // d'autres, `--all` et `--channel` seraient indissociables, et les cinq
        // canaux du relevé cesseraient d'être pilotables séparément
        // (docs/VENTILATEURS.md, table du résumé).
        //
        // Deux écritures sur deux sources différentes ; toute l'arborescence
        // est comparée, pas seulement les fichiers auxquels on aurait pensé.
        let sysfs = arborescence_de_reference("ecritures_isolees");
        let canaux = sysfs.canaux();
        let vise = canal(&canaux, "nzxtsmart2:fan-2");
        let bascule = canal(&canaux, "kraken2023elite:pump-speed");

        let avant = photographie(sysfs.racine());
        set_pwm(vise, consigne(80)).expect("écriture de la consigne");
        set_mode(bascule, Mode::Manual).expect("bascule explicite en manuel");
        let apres = photographie(sysfs.racine());

        assert_eq!(
            ecarts(&avant, &apres),
            vec!["hwmon4/pwm2".to_owned(), "hwmon6/pwm1_enable".to_owned()]
        );

        // Dit autrement, sur les canaux voisins : leur consigne est intacte.
        for nom in [
            "nzxtsmart2:fan-1",
            "nzxtsmart2:fan-3",
            "kraken2023elite:fan-speed",
        ] {
            let voisin = canal(&canaux, nom);
            assert_ne!(
                voisin.pwm, vise.pwm,
                "{nom} ne partage pas le fichier de consigne visé"
            );
        }
        assert_eq!(lire(&canal(&canaux, "nzxtsmart2:fan-1").pwm), "64");
        assert_eq!(lire(&canal(&canaux, "nzxtsmart2:fan-3").pwm), "64");
        assert_eq!(lire(&canal(&canaux, "kraken2023elite:fan-speed").pwm), "71");
    }
}

// ---------------------------------------------------------------------------

mod robustesse {
    use super::FauxSysfs;

    #[test]
    fn une_racine_vide_ne_produit_aucun_canal() {
        // Le cas d'une machine sans aucun pilote hwmon chargé. Il n'y a rien à
        // piloter, ce n'est pas une erreur.
        let sysfs = FauxSysfs::neuf("racine_vide");
        assert!(sysfs.canaux().is_empty());
    }

    #[test]
    fn une_racine_inexistante_ne_panique_pas() {
        // Contrat d'API — `discover_in` rend un `std::io::Result`. Une racine
        // absente est une condition d'erreur ordinaire, pas un défaut du
        // programme : `reverb fans` doit pouvoir en parler, pas s'interrompre
        // sur une trace de panique.
        let absente = std::env::temp_dir().join(format!(
            "reverb-spec-fans-racine-absente-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&absente);

        match reverb_hw::hwmon::discover_in(&absente) {
            Ok(canaux) => assert!(
                canaux.is_empty(),
                "une racine absente ne peut rien avoir découvert : {:?}",
                canaux.iter().map(|c| &c.name).collect::<Vec<_>>()
            ),
            Err(_) => { /* une erreur d'entrée/sortie est une réponse valable */ }
        }
    }

    #[test]
    fn un_hwmon_sans_le_moindre_attribut_ne_fait_pas_echouer_la_decouverte() {
        // `/sys/class/hwmon` est peuplé par le noyau, pas par nous : une source
        // dépouillée ne doit pas empêcher de découvrir les autres.
        let sysfs = FauxSysfs::neuf("hwmon_depouille");
        sysfs.hwmon("hwmon0", "nvme");
        super::canal_complet(&sysfs.hwmon("hwmon1", "nzxtsmart2"), 1, "FAN 1");

        assert_eq!(sysfs.noms(), vec!["nzxtsmart2:fan-1"]);
    }
}
