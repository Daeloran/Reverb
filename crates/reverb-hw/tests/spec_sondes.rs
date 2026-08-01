//! Tests d'intention de la découverte des sondes de température (issue #31).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #31 et le contrat déposé en
//! fin de `crates/reverb-hw/src/hwmon.rs` — dont les corps sont `todo!("issue
//! #31")` à l'écriture de ce fichier. Rien n'est relu du corps des fonctions
//! déjà écrites.
//!
//! Comme `spec_fans.rs` (#7), dont ce fichier reprend la fausse arborescence :
//! ces tests encodent ce que le logiciel doit faire, pas ce que le code fait. Si
//! l'un d'eux échoue après implémentation, c'est le code qu'on corrige, jamais
//! le test.
//!
//! # Aucun accès matériel, et aucune écriture
//!
//! issue #31, critère d'acceptation — « La découverte est **strictement en
//! lecture** : aucun octet écrit sur un `hwmon` qui n'est pas un canal de
//! ventilateur connu », et approche technique — « ⚠️ **Pas de scan de bus.** Les
//! sondes se lisent dans `sysfs`, jamais en interrogeant un bus I2C : un scan en
//! lecture seule a déjà altéré l'éclairage par défaut de la RAM sur cette
//! machine. »
//!
//! Les seuls chemins touchés ici sont ceux d'une arborescence construite dans
//! `std::env::temp_dir()`, effacée à la fin de chaque test. **Rien n'est lu ni
//! écrit sous `/sys`**, et le module `lecture_seule` vérifie que la découverte
//! elle-même n'écrit nulle part.
//!
//! # Ce que ces tests ne vérifient pas
//!
//! - **`gpu::nvidia()`.** Il lance `nvidia-smi` et n'a de réponse que sur une
//!   machine portant une carte NVIDIA : ni le binaire ni la carte ne sont des
//!   entrées qu'un test en boîte noire peut poser. Le contrat n'expose aucune
//!   fonction de découpage de la sortie du programme, qui serait, elle,
//!   testable. Ce qui reste vérifiable — qu'une absence de `nvidia-smi` donne
//!   `None` et jamais une valeur inventée — l'est déjà par le type de retour.
//! - **Les températures relevées sur la machine.** Elles illustrent, elles ne
//!   spécifient pas : exiger « le CPU est à 45 °C » serait faux à la seconde
//!   suivante. Les valeurs n'apparaissent ici que comme **contenus de fichiers**
//!   de l'arborescence temporaire, pour vérifier qu'une sonde lit bien le sien.
//! - **L'affichage** : la colonne de cartes, la courbe, la mise à l'échelle. Le
//!   tracé et son historique sont spécifiés par
//!   `crates/reverb-gui/tests/spec_historique.rs`.
//! - **`reverb status`.** Le critère « liste les mêmes sondes que la fenêtre »
//!   porte sur la sortie du démon ; aucune signature de ce contrat ne la décrit.
//!
//! # Arbitrages de contrat
//!
//! **Le `slug` de deux `hwmon` homonymes.** Le contrat dit deux choses qui se
//! contredisent quand quatre barrettes de RAM exposent le même `spd5118` avec le
//! même `temp1` : le `slug` vaut « `<nom du hwmon>:<libellé>` », et la liste est
//! « sans doublon ». Arbitré ici : **« sans doublon » veut dire que deux sondes
//! ne portent jamais le même `slug`, pas qu'on en jette trois sur quatre.**
//! Perdre trois barrettes serait perdre de la donnée ; les départager coûte un
//! suffixe. Ces tests exigent donc que les quatre soient là, que leurs `slug`
//! soient distincts, qu'ils portent tous la source, et que la liste ne bouge pas
//! d'un appel à l'autre — mais ils ne dictent **pas** la forme du départage, que
//! le contrat ne donne pas.
//!
//! Ce qu'ils interdisent en revanche, c'est de payer ce départage par le numéro
//! du répertoire `hwmon` **quand il n'y a pas d'homonyme** : le CLAUDE.md du
//! projet rappelle que ces numéros changent au redémarrage, « constaté, pas
//! supposé », et le contrat veut un « identifiant **stable** ».

use std::fs;
use std::path::{Path, PathBuf};

use reverb_hw::hwmon::{Sonde, sondes_dans};

// ---------------------------------------------------------------------------
// Fausse arborescence sysfs
// ---------------------------------------------------------------------------

/// Une fausse `/sys/class/hwmon` dans un répertoire temporaire.
///
/// Même dispositif que `spec_fans.rs` : le répertoire porte le nom du test et le
/// PID — deux tests qui tournent en parallèle ne se marchent pas dessus — et il
/// est effacé à la destruction, y compris quand le test échoue.
struct FauxSysfs {
    racine: PathBuf,
}

impl FauxSysfs {
    fn neuf(nom_du_test: &str) -> Self {
        let racine = std::env::temp_dir().join(format!(
            "reverb-spec-sondes-{nom_du_test}-{}",
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

    fn sondes(&self) -> Vec<Sonde> {
        sondes_dans(self.racine()).expect("découverte sur l'arborescence temporaire")
    }

    fn slugs(&self) -> Vec<String> {
        self.sondes().into_iter().map(|s| s.slug).collect()
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
/// attribut. Une lecture qui ne le retire pas produit une origine `"k10temp\n"`,
/// et surtout une température qui ne se convertit plus en nombre.
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

/// Une sonde de température : `tempN_input`, et son `tempN_label` s'il existe.
fn temperature(hwmon: &Path, rang: u32, libelle: Option<&str>, millidegres: i32) {
    ecrire(
        &hwmon.join(format!("temp{rang}_input")),
        &millidegres.to_string(),
    );
    if let Some(libelle) = libelle {
        ecrire(&hwmon.join(format!("temp{rang}_label")), libelle);
    }
}

/// Photographie de toute l'arborescence : chemin relatif → contenu.
///
/// Sert à vérifier que la découverte n'a rien écrit. Comparer deux
/// photographies est plus sûr que de relire les quelques fichiers auxquels on
/// aurait pensé : un fichier oublié serait un fichier non surveillé.
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

// ---------------------------------------------------------------------------
// L'arborescence de référence
// ---------------------------------------------------------------------------

/// Les quatre barrettes de RAM, à quatre températures distinctes.
///
/// Distinctes exprès : c'est ce qui permet de voir qu'aucune ne se lit à la
/// place d'une autre. Les quatre `hwmon` portent le même `name` — `spd5118` — et
/// la même sonde `temp1` sans libellé, donc rien d'autre que leur répertoire ne
/// les distingue.
const RAM: [i32; 4] = [41_000, 42_000, 40_000, 43_000];

/// Le nombre de sondes de l'arborescence de référence.
///
/// C'est celui de la machine : commit du contrat — « SHYNAEL expose treize
/// hwmon et quinze sondes de temperature : CPU (k10temp, Tctl et Tccd1),
/// coolant du Kraken, cinq sondes NVMe, les QUATRE barrettes de RAM (spd5118),
/// le GPU integre au Ryzen (amdgpu), le wifi et la carte reseau. »
const SONDES_DE_REFERENCE: usize = 15;

/// Les sondes dont la source n'a pas d'homonyme : `slug`, origine, libellé.
///
/// Contrat — `slug`, « Identifiant stable : `<nom du hwmon>:<libellé>` » ;
/// `origine`, « le nom du `hwmon` » ; `libelle`, « le libellé du noyau, ou
/// `tempN` à défaut ». Le libellé passe en kebab-case ASCII dans le `slug`,
/// comme le nom d'un canal de ventilateur (`hwmon::slug`, « "Pump speed" ->
/// "pump-speed" ») ; l'origine, elle, est recopiée telle quelle — sinon
/// `origine` et `slug` ne parleraient plus de la même chose.
const SONDES_A_ORIGINE_UNIQUE: [(&str, &str, &str); 6] = [
    ("amdgpu:edge", "amdgpu", "edge"),
    ("k10temp:tccd1", "k10temp", "Tccd1"),
    ("k10temp:tctl", "k10temp", "Tctl"),
    (
        "kraken2023elite:coolant-temp",
        "kraken2023elite",
        "Coolant temp",
    ),
    ("mt7921_phy0:temp1", "mt7921_phy0", "temp1"),
    ("r8169_0_0d00:temp1", "r8169_0_0d00", "temp1"),
];

/// L'arborescence de référence, dans la forme du relevé de SHYNAEL.
///
/// Elle compte quatorze `hwmon` là où la machine en compte treize : le relevé
/// ne dit pas comment ses **cinq** sondes NVMe se répartissent entre
/// contrôleurs, et deux disques — trois sondes puis deux — est la forme
/// courante. Un homonyme de plus ne fait que durcir le test ; le nombre de
/// sondes, lui, est celui de la machine.
///
/// Trois `hwmon` n'ont **aucune** sonde de température : `nzxtsmart2`, qui ne
/// porte que des canaux de ventilateurs, `zenergy` et `hidpp_battery_0`. Ils
/// sont là pour qu'un `hwmon` muet reste muet.
///
/// Les numéros de répertoire ne suivent pas l'ordre alphabétique des sources :
/// l'ordre de lecture du répertoire ne peut donc pas être confondu avec l'ordre
/// attendu.
fn arborescence_de_reference(nom_du_test: &str) -> FauxSysfs {
    let sysfs = FauxSysfs::neuf(nom_du_test);

    // Le CPU : deux sondes libellées, et un trou dans les rangs — `temp2`
    // n'existe pas. Le rang appartient au fichier, pas à la position dans la
    // liste.
    let cpu = sysfs.hwmon("hwmon3", "k10temp");
    temperature(&cpu, 1, Some("Tctl"), 45_250);
    temperature(&cpu, 3, Some("Tccd1"), 44_000);

    // Le Kraken : la seule sonde que la fenêtre affichait jusqu'ici, parce
    // qu'elle est voisine d'un canal de ventilateur (issue #31, contexte).
    // Autour d'elle, des fichiers dont le nom commence par `temp1` sans être
    // des mesures — seuils, point de courbe firmware — et le tachymètre de la
    // pompe, qui est un régime, pas une température.
    let kraken = sysfs.hwmon("hwmon6", "kraken2023elite");
    temperature(&kraken, 1, Some("Coolant temp"), 31_000);
    ecrire(&kraken.join("temp1_crit"), "60000");
    ecrire(&kraken.join("temp1_max"), "59000");
    ecrire(&kraken.join("temp1_min"), "0");
    ecrire(&kraken.join("temp1_alarm"), "0");
    ecrire(&kraken.join("temp1_auto_point1_pwm"), "60");
    ecrire(&kraken.join("curr1_input"), "0");
    ecrire(&kraken.join("pwm1"), "171");
    ecrire(&kraken.join("fan1_input"), "2380");

    // Cinq sondes NVMe sur deux contrôleurs homonymes, avec les libellés que le
    // pilote donne.
    let nvme0 = sysfs.hwmon("hwmon0", "nvme");
    temperature(&nvme0, 1, Some("Composite"), 38_850);
    temperature(&nvme0, 2, Some("Sensor 1"), 38_850);
    temperature(&nvme0, 3, Some("Sensor 2"), 45_850);
    let nvme1 = sysfs.hwmon("hwmon1", "nvme");
    temperature(&nvme1, 1, Some("Composite"), 33_850);
    temperature(&nvme1, 2, Some("Sensor 1"), 33_850);

    // Les quatre barrettes. Aucun `temp1_label` : le libellé doit retomber sur
    // le rang, et le `slug` sur autre chose que le seul couple source+libellé,
    // qui est le même pour les quatre.
    // ⚠️ Elles se lisent par le pilote du noyau, qui détient le bus. Aucun octet
    // n'est émis sur l'I²C — contrat, « Lecture seule, et par `sysfs`
    // uniquement ».
    for (barrette, millidegres) in RAM.iter().enumerate() {
        let dimm = sysfs.hwmon(&format!("hwmon{}", 8 + barrette), "spd5118");
        temperature(&dimm, 1, None, *millidegres);
    }

    // Le GPU **intégré** au Ryzen. La RTX 5070 n'a pas de `hwmon` : le pilote
    // propriétaire n'en enregistre aucun (contrat de `gpu.rs`), elle ne peut
    // donc pas apparaître ici et ce n'est pas un manque de la découverte.
    let igpu = sysfs.hwmon("hwmon5", "amdgpu");
    temperature(&igpu, 1, Some("edge"), 40_000);
    ecrire(&igpu.join("pwm1"), "128");
    ecrire(&igpu.join("fan1_input"), "0");

    // Le wifi et la carte réseau : sans libellé, et avec des noms de source qui
    // portent chiffres et soulignés.
    temperature(&sysfs.hwmon("hwmon7", "mt7921_phy0"), 1, None, 52_000);
    temperature(&sysfs.hwmon("hwmon12", "r8169_0_0d00"), 1, None, 55_000);

    // Trois sources sans la moindre température. `nzxtsmart2` porte les canaux
    // de ventilateurs de Reverb — c'est justement autour d'eux que la fenêtre se
    // limitait —, `zenergy` compte des joules, et `hidpp_battery_0` des volts.
    // `temperature_input` est un voisin qui commence par `temp` et finit par
    // `_input` sans être une sonde ; il n'existe pas dans `sysfs`, il est là
    // pour interdire une reconnaissance par les bords du nom.
    let nzxt = sysfs.hwmon("hwmon4", "nzxtsmart2");
    for n in 1..=3 {
        ecrire(&nzxt.join(format!("pwm{n}")), "64");
        ecrire(&nzxt.join(format!("pwm{n}_enable")), "1");
        ecrire(&nzxt.join(format!("fan{n}_input")), "700");
        ecrire(&nzxt.join(format!("fan{n}_label")), &format!("FAN {n}"));
    }
    ecrire(
        &sysfs.hwmon("hwmon2", "zenergy").join("energy1_input"),
        "12345",
    );
    let souris = sysfs.hwmon("hwmon13", "hidpp_battery_0");
    ecrire(&souris.join("in0_input"), "3800");
    ecrire(&souris.join("temperature_input"), "42000");

    sysfs
}

/// La sonde portant ce `slug`, ou un échec explicite.
fn sonde<'a>(sondes: &'a [Sonde], slug: &str) -> &'a Sonde {
    sondes
        .iter()
        .find(|s| s.slug == slug)
        .unwrap_or_else(|| panic!("la sonde « {slug} » doit être découverte"))
}

/// Les sondes venant de cette source, dans l'ordre de la liste rendue.
fn famille<'a>(sondes: &'a [Sonde], origine: &str) -> Vec<&'a Sonde> {
    sondes.iter().filter(|s| s.origine == origine).collect()
}

/// Ce que `lire` rend, ou un échec explicite nommant la sonde.
fn valeur(sonde: &Sonde) -> i32 {
    sonde
        .lire()
        .unwrap_or_else(|e| panic!("la sonde « {} » doit se lire : {e}", sonde.slug))
}

// ---------------------------------------------------------------------------

mod decouverte {
    use super::{
        FauxSysfs, RAM, SONDES_A_ORIGINE_UNIQUE, SONDES_DE_REFERENCE, arborescence_de_reference,
        famille, sonde, temperature, valeur,
    };

    #[test]
    fn toutes_les_sondes_sont_rendues_pas_seulement_celles_d_un_hwmon_a_ventilateur() {
        // issue #31, critère d'acceptation — « Toutes les sondes de température
        // de la machine sont listées, pas seulement celles de Reverb : coolant,
        // CPU, GPU, chipset, NVMe si le noyau les expose », et contexte — « La
        // fenêtre n'affiche aujourd'hui que les sondes voisines des canaux de
        // ventilateurs [...] en pratique le coolant du Kraken ».
        // C'est le cœur de l'issue : une découverte qui repartirait des canaux
        // rendrait le coolant et le bord du GPU intégré, et perdrait le CPU, les
        // NVMe et les quatre barrettes — soit douze sondes sur quinze.
        let sysfs = arborescence_de_reference("toutes_les_sondes");
        let sondes = sysfs.sondes();

        assert_eq!(
            sondes.len(),
            SONDES_DE_REFERENCE,
            "sondes rendues : {:?}",
            sysfs.slugs()
        );

        // Les sources sans le moindre `pwm` sont là comme les autres.
        for (origine, attendu) in [
            ("k10temp", 2),
            ("nvme", 5),
            ("spd5118", RAM.len()),
            ("mt7921_phy0", 1),
            ("r8169_0_0d00", 1),
            ("kraken2023elite", 1),
            ("amdgpu", 1),
        ] {
            assert_eq!(
                famille(&sondes, origine).len(),
                attendu,
                "nombre de sondes de {origine}, sur {:?}",
                sysfs.slugs()
            );
        }
    }

    #[test]
    fn chaque_sonde_porte_son_slug_son_origine_et_son_libelle() {
        // Contrat — `slug` « `<nom du hwmon>:<libellé>` », `origine` « le nom du
        // `hwmon` », `libelle` « le libellé du noyau, ou `tempN` à défaut ».
        // issue #31, critère — « Chaque sonde porte un nom lisible ». Un `slug`
        // qui perdrait sa source afficherait quatre cartes « temp1 » sans dire
        // laquelle est la RAM et laquelle est le wifi.
        let sysfs = arborescence_de_reference("slug_origine_libelle");
        let sondes = sysfs.sondes();

        for (slug, origine, libelle) in SONDES_A_ORIGINE_UNIQUE {
            let s = sonde(&sondes, slug);
            assert_eq!(s.origine, origine, "origine de {slug}");
            assert_eq!(s.libelle, libelle, "libellé de {slug}");
            assert!(
                s.slug.starts_with(&format!("{origine}:")),
                "le slug « {} » doit commencer par sa source « {origine} »",
                s.slug
            );
        }
    }

    #[test]
    fn les_valeurs_sysfs_sont_lues_sans_leur_saut_de_ligne() {
        // Le noyau termine chaque attribut par `\n`. Une origine « k10temp\n »
        // casserait le `slug`, et un libellé « Tctl\n » couperait la carte en
        // deux dans la fenêtre.
        let sysfs = arborescence_de_reference("sans_saut_de_ligne");
        for s in sysfs.sondes() {
            for (quoi, texte) in [
                ("slug", &s.slug),
                ("libellé", &s.libelle),
                ("origine", &s.origine),
            ] {
                assert!(
                    !texte.contains(['\n', '\r']),
                    "{quoi} brut : {texte:?} (sonde {})",
                    s.slug
                );
                assert_eq!(texte.trim(), texte, "{quoi} non ébarbé : {texte:?}");
            }
        }
    }

    #[test]
    fn un_hwmon_sans_sonde_de_temperature_ne_produit_rien_et_n_echoue_pas() {
        // issue #31, test d'intention n° 4 — « Un `hwmon` sans aucune sonde de
        // température ne produit aucune ligne, et n'échoue pas ».
        // Les trois cas de la machine : le contrôleur de ventilateurs, le
        // compteur d'énergie, la batterie de la souris. Une ligne inventée pour
        // eux ferait trois cartes vides dans la fenêtre ; une erreur ferait
        // disparaître les quinze autres sondes.
        let sysfs = arborescence_de_reference("hwmon_muet");
        let sondes = sysfs.sondes();

        for muet in ["nzxtsmart2", "zenergy", "hidpp_battery_0"] {
            assert!(
                famille(&sondes, muet).is_empty(),
                "{muet} n'expose aucune température : {:?}",
                sysfs.slugs()
            );
        }
        assert_eq!(sondes.len(), SONDES_DE_REFERENCE);
    }

    #[test]
    fn un_fichier_au_nom_voisin_n_est_pas_pris_pour_une_sonde() {
        // issue #31, approche technique — « parcours de
        // `/sys/class/hwmon/*/temp*_input` ». Le motif se lit strictement :
        // `temp`, un rang, `_input`. Autour de la seule sonde du Kraken il y a
        // `temp1_crit`, `temp1_max`, `temp1_min`, `temp1_alarm` et
        // `temp1_auto_point1_pwm` ; ailleurs `curr1_input`, `fan1_input`,
        // `in0_input`, `energy1_input` et un `temperature_input` qui commence
        // et finit comme une sonde sans en être une. Les prendre afficherait
        // des seuils, des volts et des tours par minute comme des degrés.
        let sysfs = arborescence_de_reference("noms_voisins");
        let sondes = sysfs.sondes();

        assert_eq!(famille(&sondes, "kraken2023elite").len(), 1);
        assert_eq!(sondes.len(), SONDES_DE_REFERENCE, "{:?}", sysfs.slugs());

        for s in &sondes {
            let fichier = s
                .chemin
                .file_name()
                .and_then(|f| f.to_str())
                .expect("nom du fichier de la sonde");
            let rang = fichier
                .strip_prefix("temp")
                .and_then(|reste| reste.strip_suffix("_input"))
                .unwrap_or_else(|| panic!("{} lit « {fichier} », pas un tempN_input", s.slug));
            assert!(
                !rang.is_empty() && rang.bytes().all(|b| b.is_ascii_digit()),
                "{} lit « {fichier} » : « {rang} » n'est pas un rang",
                s.slug
            );
        }
    }

    #[test]
    fn un_rang_a_deux_chiffres_est_reconnu_et_ne_se_confond_pas_avec_le_premier() {
        // Le chipset des cartes mères modernes expose une dizaine de sondes :
        // `temp10_input` existe, et il n'est pas `temp1`. Une reconnaissance qui
        // ne lirait qu'un chiffre après « temp » afficherait deux cartes
        // « temp1 » dont l'une montrerait la mauvaise valeur — sans le moindre
        // message.
        let sysfs = FauxSysfs::neuf("rang_a_deux_chiffres");
        let carte = sysfs.hwmon("hwmon0", "nct6687");
        let (premier, dixieme) = (30_000, 70_000);
        temperature(&carte, 1, None, premier);
        temperature(&carte, 10, None, dixieme);

        let sondes = sysfs.sondes();
        assert_eq!(sondes.len(), 2, "{:?}", sysfs.slugs());
        assert_eq!(sonde(&sondes, "nct6687:temp1").libelle, "temp1");
        assert_eq!(sonde(&sondes, "nct6687:temp10").libelle, "temp10");
        assert_eq!(valeur(sonde(&sondes, "nct6687:temp1")), premier);
        assert_eq!(valeur(sonde(&sondes, "nct6687:temp10")), dixieme);
    }

    #[test]
    fn chaque_sonde_lit_le_fichier_de_son_propre_hwmon() {
        // Contrat — `chemin`, « Le fichier à relire à chaque tour ». Croiser
        // deux chemins entre `hwmon` homonymes afficherait la température de la
        // barrette 3 sous le nom de la barrette 1 : c'est faux, c'est plausible,
        // et rien ne le signalerait. Les quatre valeurs de `RAM` sont distinctes
        // exprès ; on les retrouve toutes les quatre, une fois chacune.
        let sysfs = arborescence_de_reference("chemins_propres");
        let sondes = sysfs.sondes();

        let mut lues: Vec<i32> = famille(&sondes, "spd5118")
            .into_iter()
            .map(valeur)
            .collect();
        lues.sort_unstable();
        let mut ecrites = RAM.to_vec();
        ecrites.sort_unstable();
        assert_eq!(lues, ecrites, "les quatre barrettes, une fois chacune");

        // Et le rang du fichier est bien celui de la sonde : `Tccd1` est
        // `temp3_input`, pas le premier fichier venu du répertoire.
        for (slug, fichier) in [
            ("k10temp:tctl", "temp1_input"),
            ("k10temp:tccd1", "temp3_input"),
        ] {
            assert_eq!(
                sonde(&sondes, slug)
                    .chemin
                    .file_name()
                    .and_then(|f| f.to_str()),
                Some(fichier),
                "fichier de {slug}"
            );
        }
    }

    #[test]
    fn aucun_chemin_ne_sort_de_la_racine_donnee() {
        // Le même garde-fou que `spec_fans.rs` : si la découverte rendait un
        // chemin absolu vers `/sys`, un test finirait par lire — ou pire — sur
        // le vrai matériel.
        let sysfs = arborescence_de_reference("chemins_confines");
        let racine = sysfs.racine();
        for s in sysfs.sondes() {
            assert!(
                s.chemin.starts_with(racine),
                "{} sort de la racine {}",
                s.chemin.display(),
                racine.display()
            );
        }
    }

    #[test]
    fn un_hwmon_atteint_par_un_lien_symbolique_est_parcouru() {
        // `/sys/class/hwmon/hwmonN` **est un lien** vers
        // `../../devices/.../hwmon/hwmonN` : c'est la forme réelle de la racine
        // que `sondes()` parcourt. Une découverte qui déciderait « est-ce un
        // répertoire ? » sur le type d'entrée renvoyé par la lecture du
        // répertoire, sans suivre le lien, ne trouverait **rien du tout** sur la
        // machine tout en passant tous les autres tests de ce fichier.
        let sysfs = FauxSysfs::neuf("hwmon_par_lien");
        let reel = sysfs.racine().join("devices");
        std::fs::create_dir_all(&reel).expect("création du répertoire des périphériques");
        let cpu = reel.join("hwmon0");
        std::fs::create_dir_all(&cpu).expect("création du hwmon réel");
        super::ecrire(&cpu.join("name"), "k10temp");
        temperature(&cpu, 1, Some("Tctl"), 45_250);
        std::os::unix::fs::symlink(&cpu, sysfs.racine().join("hwmon0")).expect("lien symbolique");

        let sondes = sysfs.sondes();
        assert!(
            sondes.iter().any(|s| s.slug == "k10temp:tctl"),
            "la sonde derrière le lien doit être découverte : {:?}",
            sysfs.slugs()
        );
    }
}

// ---------------------------------------------------------------------------

mod noms {
    use super::{
        FauxSysfs, RAM, SONDES_DE_REFERENCE, arborescence_de_reference, famille, sonde, temperature,
    };

    #[test]
    fn une_sonde_sans_label_prend_le_nom_du_hwmon_et_son_rang() {
        // issue #31, critère d'acceptation — « Chaque sonde porte un nom
        // lisible : le libellé du noyau quand il existe, sinon le nom du
        // `hwmon` et le rang de la sonde », et test d'intention n° 2.
        // Le wifi et la carte réseau n'ont pas de `tempN_label` ; les quatre
        // barrettes non plus.
        let sysfs = arborescence_de_reference("sans_label");
        let sondes = sysfs.sondes();

        for slug in ["mt7921_phy0:temp1", "r8169_0_0d00:temp1"] {
            let s = sonde(&sondes, slug);
            assert_eq!(s.libelle, "temp1", "libellé de repli de {slug}");
            assert_eq!(s.slug, format!("{}:{}", s.origine, s.libelle));
        }
        for s in famille(&sondes, "spd5118") {
            assert_eq!(s.libelle, "temp1", "libellé de repli d'une barrette");
        }
    }

    #[test]
    fn le_rang_de_repli_est_celui_du_fichier_pas_le_numero_d_ordre() {
        // Suite du même critère : « le rang **de la sonde** ». Les rangs ont des
        // trous — `k10temp` expose `temp1` et `temp3`, jamais `temp2`. Numéroter
        // les sondes au fil de la découverte donnerait « temp1 » et « temp2 » :
        // deux noms qui ne désignent aucun fichier, et qui changeraient de
        // sonde le jour où un rang apparaît.
        let sysfs = FauxSysfs::neuf("rang_du_fichier");
        let carte = sysfs.hwmon("hwmon0", "nct6687");
        temperature(&carte, 2, None, 30_000);
        temperature(&carte, 5, None, 35_000);

        assert_eq!(sysfs.slugs(), vec!["nct6687:temp2", "nct6687:temp5"]);
    }

    #[test]
    fn un_label_vide_retombe_sur_un_nom_lisible() {
        // Le critère dit « le libellé du noyau **quand il existe** ». Un
        // `tempN_label` présent mais vide — un attribut que le pilote expose
        // sans le remplir — n'est pas un libellé : il donnerait une carte sans
        // titre et un `slug` qui s'arrête sur son deux-points. Ce test
        // n'impose pas le repli choisi, seulement qu'il en existe un.
        let sysfs = FauxSysfs::neuf("label_vide");
        let carte = sysfs.hwmon("hwmon0", "nct6687");
        temperature(&carte, 1, Some(""), 30_000);

        let sondes = sysfs.sondes();
        assert_eq!(sondes.len(), 1, "{:?}", sysfs.slugs());
        assert!(
            !sondes[0].libelle.trim().is_empty(),
            "libellé de repli attendu, obtenu {:?}",
            sondes[0].libelle
        );
        assert!(
            !sondes[0].slug.ends_with(':'),
            "slug tronqué : {:?}",
            sondes[0].slug
        );
    }

    #[test]
    fn les_slugs_sont_uniques_tries_et_portent_leur_source() {
        // Contrat — « Triées par `slug`, sans doublon », et « Le nom porte la
        // source parce que deux pilotes nomment volontiers leur capteur
        // `temp1`, et qu'un nom ambigu vaut moins que pas de nom du tout ».
        // Voir l'arbitrage en tête de fichier : « sans doublon » veut dire que
        // deux sondes ne portent jamais le même `slug`, pas qu'on jette trois
        // barrettes sur quatre. La forme du départage n'est pas dictée ici.
        let sysfs = arborescence_de_reference("slugs_uniques");
        let sondes = sysfs.sondes();
        let slugs = sysfs.slugs();

        let mut uniques = slugs.clone();
        uniques.sort();
        uniques.dedup();
        assert_eq!(
            uniques.len(),
            SONDES_DE_REFERENCE,
            "deux sondes partagent un slug : {slugs:?}"
        );
        assert_eq!(slugs, uniques, "la liste doit être triée par slug");

        for s in &sondes {
            assert!(
                s.slug.contains(&s.origine),
                "le slug « {} » ne dit pas d'où il vient ({})",
                s.slug,
                s.origine
            );
        }

        // Les deux familles homonymes : toutes leurs sondes sont là, toutes
        // distinctes, et chacune sur son propre fichier.
        for (origine, attendu) in [("spd5118", RAM.len()), ("nvme", 5)] {
            let homonymes = famille(&sondes, origine);
            assert_eq!(homonymes.len(), attendu, "sondes de {origine}");
            let mut chemins: Vec<_> = homonymes.iter().map(|s| &s.chemin).collect();
            chemins.sort();
            chemins.dedup();
            assert_eq!(
                chemins.len(),
                attendu,
                "{origine} : deux sondes, un fichier"
            );
        }
    }

    #[test]
    fn la_liste_ne_bouge_pas_d_un_appel_a_l_autre() {
        // Contrat — « Identifiant **stable** ». L'ordre de lecture d'un
        // répertoire n'est pas garanti ; celui de la liste doit l'être, sinon la
        // fenêtre change ses cartes de nom entre deux secondes et l'historique
        // de chaque sonde se vide.
        let sysfs = arborescence_de_reference("liste_stable");
        assert_eq!(sysfs.sondes(), sysfs.sondes());
    }

    #[test]
    fn le_slug_ne_depend_pas_du_numero_du_repertoire_hwmon() {
        // CLAUDE.md du projet — « Les numéros de `hidraw` changent au
        // redémarrage — constaté, pas supposé » ; ceux des `hwmon` suivent de la
        // même façon l'ordre de chargement des pilotes. Un `slug` qui les
        // porterait ne serait pas l'« identifiant stable » que le contrat
        // annonce : l'historique et la sélection de la fenêtre changeraient de
        // sonde au démarrage suivant.
        // Deux arborescences identiques au numéro près, sans homonyme — le
        // départage des homonymes, lui, reste libre (voir l'arbitrage en tête de
        // fichier).
        let a = FauxSysfs::neuf("numeros_a");
        temperature(&a.hwmon("hwmon0", "k10temp"), 1, Some("Tctl"), 45_250);
        temperature(&a.hwmon("hwmon1", "amdgpu"), 1, Some("edge"), 40_000);

        let b = FauxSysfs::neuf("numeros_b");
        temperature(&b.hwmon("hwmon7", "amdgpu"), 1, Some("edge"), 40_000);
        temperature(&b.hwmon("hwmon3", "k10temp"), 1, Some("Tctl"), 45_250);

        assert_eq!(a.slugs(), b.slugs());
        assert_eq!(a.slugs(), vec!["amdgpu:edge", "k10temp:tctl"]);
    }
}

// ---------------------------------------------------------------------------

mod lecture {
    use super::{FauxSysfs, arborescence_de_reference, ecrire, sonde, temperature, valeur};

    #[test]
    fn une_sonde_rend_la_valeur_de_son_fichier_en_millidegres() {
        // Contrat — `lire`, « La température du moment, en millidegrés ». La
        // conversion en degrés appartient à l'affichage ; ce que le noyau donne
        // est un entier de millidegrés, et il est rendu tel quel.
        let sysfs = arborescence_de_reference("lecture_simple");
        let sondes = sysfs.sondes();
        for slug in ["k10temp:tctl", "kraken2023elite:coolant-temp"] {
            let s = sonde(&sondes, slug);
            let attendu: i32 = super::lire(&s.chemin)
                .parse()
                .expect("le fichier simulé contient un entier");
            assert_eq!(valeur(s), attendu, "valeur de {slug}");
        }
    }

    #[test]
    fn une_temperature_negative_se_lit_comme_telle() {
        // `lire` rend un `i32` signé, et une sonde sous zéro est ordinaire :
        // une carte réseau au démarrage à froid, une sonde débranchée que le
        // pilote rend à -40 °C. Lire un entier non signé rendrait une erreur ou,
        // pire, une température astronomique.
        let sysfs = FauxSysfs::neuf("temperature_negative");
        let carte = sysfs.hwmon("hwmon0", "r8169_0_0d00");
        let froid = -12_000;
        temperature(&carte, 1, None, froid);

        assert_eq!(valeur(&sysfs.sondes()[0]), froid);
    }

    #[test]
    fn une_sonde_dont_le_fichier_disparait_le_dit_au_lieu_de_figer_sa_valeur() {
        // issue #31, critère d'acceptation — « Une sonde qui disparaît en cours
        // de route — périphérique débranché — le dit au lieu de figer sa
        // dernière valeur », et test d'intention n° 3. Contrat — `lire`, « `Err`
        // quand le fichier a disparu ».
        // La valeur est d'abord lue avec succès : ce qu'on vérifie n'est pas
        // qu'une sonde absente échoue, mais qu'une sonde **qui a marché** cesse
        // de rendre sa dernière valeur quand son fichier s'en va.
        let sysfs = FauxSysfs::neuf("sonde_disparue");
        let cle = sysfs.hwmon("hwmon0", "nvme");
        let derniere = 38_850;
        temperature(&cle, 1, Some("Composite"), derniere);

        let sondes = sysfs.sondes();
        let s = sonde(&sondes, "nvme:composite");
        assert_eq!(valeur(s), derniere, "la sonde se lit tant qu'elle est là");

        std::fs::remove_file(&s.chemin).expect("le périphérique est débranché");
        let apres = s.lire();
        assert!(
            apres.is_err(),
            "une sonde débranchée doit le dire, obtenu {apres:?} — la dernière \
             valeur lue était {derniere}"
        );
    }

    #[test]
    fn un_contenu_qui_n_est_pas_un_nombre_ne_devient_pas_zero() {
        // Même critère, autre visage : un pilote qui rend un attribut vide ou
        // un texte pendant une remise à zéro. Zéro est une **température
        // plausible** — la carte réseau y est au démarrage — donc un repli
        // silencieux à zéro ne se voit pas sur la courbe : il se lit comme une
        // chute brutale.
        let sysfs = FauxSysfs::neuf("contenu_non_numerique");
        let carte = sysfs.hwmon("hwmon0", "nct6687");
        temperature(&carte, 1, None, 30_000);
        temperature(&carte, 2, None, 31_000);
        ecrire(&carte.join("temp1_input"), "N/A");
        ecrire(&carte.join("temp2_input"), "");

        for s in sysfs.sondes() {
            let lu = s.lire();
            assert!(
                lu.is_err(),
                "« {} » n'est pas un nombre, obtenu {lu:?}",
                super::lire(&s.chemin)
            );
        }
    }
}

// ---------------------------------------------------------------------------

mod robustesse {
    use super::{FauxSysfs, arborescence_de_reference, ecrire, sonde, temperature, valeur};

    #[test]
    fn un_temp_input_illisible_n_empeche_pas_les_autres_d_etre_rendues() {
        // issue #31, test d'intention n° 5 — « Un `temp*_input` illisible
        // n'empêche pas les autres d'être rendues ».
        // Le fichier illisible est ici un **répertoire** portant le nom d'une
        // sonde : aucune permission à changer, donc le test dit la même chose
        // que l'utilisateur soit root ou non. Qu'il figure ou non dans la liste
        // reste ouvert ; ce qui ne l'est pas, c'est que les trois autres
        // sondes, dont deux du même `hwmon`, soient rendues et que l'appel
        // n'échoue pas.
        let sysfs = FauxSysfs::neuf("temp_illisible");
        let carte = sysfs.hwmon("hwmon0", "nct6687");
        temperature(&carte, 1, None, 30_000);
        temperature(&carte, 3, None, 33_000);
        std::fs::create_dir(carte.join("temp2_input")).expect("un temp2_input qui ne se lit pas");
        temperature(&sysfs.hwmon("hwmon1", "k10temp"), 1, Some("Tctl"), 45_250);

        let sondes = sysfs.sondes();
        for slug in ["nct6687:temp1", "nct6687:temp3", "k10temp:tctl"] {
            assert!(
                sondes.iter().any(|s| s.slug == slug),
                "« {slug} » doit être rendue malgré le voisin illisible : {:?}",
                sysfs.slugs()
            );
        }
        assert_eq!(valeur(sonde(&sondes, "nct6687:temp1")), 30_000);
        if let Some(cassee) = sondes.iter().find(|s| s.slug == "nct6687:temp2") {
            assert!(
                cassee.lire().is_err(),
                "une sonde illisible ne rend pas une valeur"
            );
        }
    }

    #[test]
    fn une_racine_vide_ne_produit_aucune_sonde() {
        // Une machine sans aucun pilote hwmon chargé : il n'y a rien à
        // afficher, ce n'est pas une erreur.
        let sysfs = FauxSysfs::neuf("racine_vide");
        assert!(sysfs.sondes().is_empty());
    }

    #[test]
    fn une_racine_inexistante_ne_panique_pas() {
        // Contrat — `sondes_dans` rend un `std::io::Result`. Une racine absente
        // est une condition d'erreur ordinaire, pas un défaut du programme :
        // `reverb status` doit pouvoir en parler, pas s'interrompre sur une
        // trace de panique.
        let absente = std::env::temp_dir().join(format!(
            "reverb-spec-sondes-racine-absente-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&absente);

        match reverb_hw::hwmon::sondes_dans(&absente) {
            Ok(sondes) => assert!(
                sondes.is_empty(),
                "une racine absente ne peut rien avoir découvert : {:?}",
                sondes.iter().map(|s| &s.slug).collect::<Vec<_>>()
            ),
            Err(_) => { /* une erreur d'entrée/sortie est une réponse valable */ }
        }
    }

    #[test]
    fn un_hwmon_depouille_ou_un_intrus_ne_font_pas_echouer_la_decouverte() {
        // `/sys/class/hwmon` est peuplé par le noyau, pas par nous : un
        // répertoire sans `name`, un `hwmon` sans le moindre attribut, ou une
        // entrée qui n'est pas un répertoire ne doivent pas emporter les sondes
        // des autres sources.
        let sysfs = FauxSysfs::neuf("hwmon_depouille");
        sysfs.hwmon("hwmon0", "zenergy");
        std::fs::create_dir(sysfs.racine().join("hwmon1")).expect("un hwmon sans name");
        ecrire(&sysfs.racine().join("intrus"), "ceci n'est pas un hwmon");
        temperature(&sysfs.hwmon("hwmon2", "k10temp"), 1, Some("Tctl"), 45_250);

        assert!(
            sysfs.slugs().contains(&"k10temp:tctl".to_owned()),
            "les sondes des autres sources restent découvertes : {:?}",
            sysfs.slugs()
        );
    }

    #[test]
    fn un_hwmon_sans_name_ne_prend_pas_le_nom_d_un_autre() {
        // Suite du précédent : si un `hwmon` sans `name` produit tout de même
        // une sonde, son origine ne doit pas être celle du voisin — deux sondes
        // sous le même nom valent moins qu'une sonde anonyme. Le répertoire
        // anonyme vient **après** celui du CPU : c'est dans cet ordre-là qu'une
        // origine reprise du `hwmon` précédent passerait inaperçue.
        let sysfs = FauxSysfs::neuf("hwmon_anonyme");
        temperature(&sysfs.hwmon("hwmon1", "k10temp"), 1, Some("Tctl"), 45_250);
        let anonyme = sysfs.racine().join("hwmon9");
        std::fs::create_dir(&anonyme).expect("un hwmon sans name");
        temperature(&anonyme, 1, None, 30_000);

        let sondes = sysfs.sondes();
        assert_eq!(
            sondes.iter().filter(|s| s.origine == "k10temp").count(),
            1,
            "une seule sonde vient de k10temp : {:?}",
            sysfs.slugs()
        );
        assert_eq!(valeur(sonde(&sondes, "k10temp:tctl")), 45_250);
    }

    #[test]
    fn la_meme_arborescence_lue_deux_fois_rend_la_meme_chose() {
        // La fenêtre redécouvre à chaque tour de boucle. Une liste qui varierait
        // sans que le matériel bouge ferait clignoter les cartes.
        let sysfs = arborescence_de_reference("deux_lectures");
        let premiere = sysfs.sondes();
        let seconde = sysfs.sondes();
        assert_eq!(premiere, seconde);
    }
}

// ---------------------------------------------------------------------------

mod lecture_seule {
    use super::{arborescence_de_reference, ecarts, photographie};

    #[test]
    fn la_decouverte_et_la_lecture_n_ecrivent_pas_un_octet() {
        // issue #31, critère d'acceptation — « La découverte est **strictement
        // en lecture** : aucun octet écrit sur un `hwmon` qui n'est pas un canal
        // de ventilateur connu », et approche technique — « Aucune écriture —
        // c'est la contrainte, pas une intention ».
        // Le rappel qui la motive est dans le CLAUDE.md du projet : « un simple
        // scan en lecture a déjà modifié l'éclairage par défaut de cette RAM ».
        // Quatre des `hwmon` parcourus ici sont justement les barrettes.
        // Comparer deux photographies de toute l'arborescence est plus sûr que
        // de relire les fichiers auxquels on aurait pensé : un fichier créé,
        // effacé ou modifié se voit, y compris là où on ne l'attendait pas.
        let sysfs = arborescence_de_reference("lecture_seule");
        let avant = photographie(sysfs.racine());

        let sondes = sysfs.sondes();
        for s in &sondes {
            let _ = s.lire();
        }

        let apres = photographie(sysfs.racine());
        assert_eq!(
            ecarts(&avant, &apres),
            Vec::<String>::new(),
            "la découverte ne doit toucher aucun fichier"
        );
    }
}
