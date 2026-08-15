//! Tests d'intention du refus d'« auto » sans courbe téléversée (issue #97).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #97 seule. Rien n'a été relu
//! de `crates/reverb-hw/src/` hormis la liste des signatures publiques de
//! `hwmon.rs` — celles sans lesquelles un test ne compile pas. À l'écriture de ce
//! fichier, ni `CourbesPosees` ni le troisième paramètre de `set_mode`
//! n'existent : **la compilation doit échouer**, et c'est la phase rouge.
//!
//! Ces tests encodent ce que le logiciel doit faire, pas ce que le code fait. Si
//! l'un d'eux échoue après implémentation, c'est le code qu'on corrige, jamais
//! le test.
//!
//! Ils prolongent `spec_fans.rs` (#7), `spec_courbes.rs` (#9) et `spec_auto.rs`
//! (#50), dont ce fichier reprend la fausse arborescence et les utilitaires —
//! dupliqués plutôt que partagés, deux fichiers de tests d'intégration ne
//! partageant rien.
//!
//! # Le fait mesuré
//!
//! issue #97, constaté sur SHYNAEL le 2026-08-15, à l'œil puis dans sysfs :
//!
//! ```text
//! pwm1 = 0        pwm1_enable = 2        pompe 1910 tr/min (son plancher matériel)
//! ```
//!
//! La pompe est passée à **0 % de consigne** et a cessé de suivre la
//! température, alors que le même jour la régulation d'usine avait été mesurée
//! bonne — 35 % à 37 °C de liquide, 60 % à 51 °C.
//!
//! La cause est dans `nzxt-kraken3`, que l'issue cite :
//!
//! ```c
//! ret = kraken3_write_curve(priv, priv->channel_info[channel].pwm_points, channel);
//! ```
//!
//! `2` pousse **le tableau de courbe détenu par le pilote**. Si aucune courbe n'a
//! jamais été téléversée, ce tableau est celui du `kzalloc` : **zéro partout**.
//! Autrement dit `2` rend la main à la courbe **de l'hôte**, jamais au profil
//! d'usine — et il n'existe aucune valeur de `pwm_enable` qui l'y rende.
//!
//! ⚠️ **C'est le mode de défaillance le plus coûteux du projet, parce qu'il est
//! rassurant** : aucune erreur n'est rendue, le mode relu devient exactement
//! celui qu'on a demandé, la pompe continue de tourner à son plancher — donc
//! rien ne s'entend — et le refroidissement est dégradé jusqu'au prochain arrêt
//! complet. Même famille que le 34 °C figé derrière une pompe arrêtée (#68).
//!
//! # La couture que ces tests exigent, et pourquoi celle-là
//!
//! L'issue tranche le comportement et laisse la forme ouverte ; ce fichier la
//! tranche, puisque c'est lui le contrat. Voici ce qu'il faut implémenter :
//!
//! ```ignore
//! // crates/reverb-hw/src/hwmon.rs
//!
//! /// Les canaux sur lesquels une courbe a été téléversée depuis le démarrage.
//! ///
//! /// Les fichiers `tempN_auto_pointM_pwm` sont en **écriture seule** (`0200`,
//! /// docs/VENTILATEURS.md) : l'état ne se relit pas sur le matériel, il ne peut
//! /// donc que vivre côté hôte. L'instance est détenue par le démon
//! /// (`peripheriques.rs`) et **repart vide à chaque démarrage**.
//! pub struct CourbesPosees { /* … */ }
//!
//! impl CourbesPosees {
//!     /// Un carnet vide : un démon qui démarre ne sait rien de ce qu'un autre
//!     /// outil a écrit avant lui.
//!     pub fn vide() -> Self;
//!
//!     /// Vrai si ce canal peut passer en automatique **maintenant** : son
//!     /// pilote sait le faire, et une courbe y a été posée.
//!     pub fn autorise_auto(&self, canal: &FanChannel) -> bool;
//! }
//!
//! pub fn set_curve(
//!     channel: &FanChannel,
//!     curve: &Curve,
//!     posees: &mut CourbesPosees,
//! ) -> io::Result<()>;
//!
//! pub fn set_mode(
//!     channel: &FanChannel,
//!     mode: Mode,
//!     posees: &CourbesPosees,
//! ) -> io::Result<()>;
//! ```
//!
//! Quatre choix, et ce qu'ils achètent :
//!
//! 1. **Le carnet traverse `set_curve`, il ne se remplit pas à côté.** C'est ce
//!    qui rend « une courbe a été posée » **structurel** plutôt que
//!    disciplinaire : on ne peut pas écrire une courbe sans que le carnet
//!    l'apprenne, ni faire croire au carnet qu'une courbe est partie alors
//!    qu'elle a échoué. Un `carnet.retenir(canal)` laissé à la politesse de
//!    l'appelant se réoublierait au premier chemin de code ajouté — et le défaut
//!    qu'on corrige est justement un défaut silencieux.
//! 2. **Le carnet est une valeur, pas un `static`.** « Remis à zéro au
//!    démarrage » devient alors une propriété qu'un test peut exercer — un
//!    carnet neuf est un démon qui redémarre — au lieu d'une promesse
//!    invérifiable dans un processus qui ne redémarre pas. Un état global
//!    rendrait de surcroît deux tests parallèles dépendants l'un de l'autre.
//! 3. **Le carnet est déclaré ici, à côté du refus, et détenu là-bas, par le
//!    démon.** L'issue veut la mémoire « côté hôte, dans le démon » : c'est
//!    l'**instance** qui y vit. Le type doit être visible d'où part le refus,
//!    sinon `reverb-hw` dépendrait de `reverb-daemon`.
//! 4. **`autorise_auto` est la même question que celle du refus, posée sans
//!    écrire.** C'est elle que la fenêtre lira pour décider d'afficher le bouton
//!    (README — « montrer un bouton qui ne peut qu'échouer vaut moins que ne pas
//!    le montrer »), et un test vérifie qu'elle ne peut jamais contredire
//!    `set_mode`.
//!
//! # Ce que ce fichier fige
//!
//! 1. `HostCurve` sans courbe posée est **refusé, et rien n'est écrit** — vérifié
//!    en photographiant toute l'arborescence, pas les seuls fichiers auxquels on
//!    aurait pensé.
//! 2. Le refus **nomme le canal** et **cite `reverb curve`** : il dit ce qui
//!    manque et comment y remédier.
//! 3. `set_curve` puis `HostCurve` sur le même canal réussit, et `pwmN_enable`
//!    vaut alors exactement `2`.
//! 4. La mémoire est **par canal** : une courbe posée sur la pompe n'autorise
//!    rien sur le ventilateur.
//! 5. La mémoire **ne survit pas** à un redémarrage du démon, y compris quand le
//!    matériel, lui, est resté en `2`.
//! 6. `Manual` et `PleinRegime` restent écrivables sans courbe.
//! 7. Les deux refus déjà en place — canal sans `pwmN_enable`, pilote sans mode
//!    automatique — sont **inchangés**, message compris.
//!
//! # Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Le socket.** `fan <canal> auto` traverse `Peripheriques::consigner`, qui
//!   exige des périphériques ouverts : rien n'y est exerçable sans matériel. Ce
//!   que le socket rend — une ligne `err` plutôt qu'un `end` — découle de
//!   l'`io::Error` que ces tests exigent, et la ligne elle-même est déjà spécifiée
//!   par #50.
//! - **La fenêtre.** Elle n'appelle jamais `set_mode` : elle lit le booléen que
//!   le démon lui envoie. Le chaînon qui manque est vérifié dans
//!   `crates/reverb-daemon/tests/spec_auto_sans_courbe.rs`.
//! - **`docs/VENTILATEURS.md`.** Une section de documentation ne se teste pas.
//!
//! # Aucun accès matériel
//!
//! Convention du dépôt, reprise de `spec_fans.rs` : les seuls chemins touchés ici
//! sont ceux d'une arborescence construite dans `std::env::temp_dir()`, effacée à
//! la fin de chaque test. **Rien n'est lu ni écrit sous `/sys`.**
//!
//! # Une attente de #50 que #97 remplace
//!
//! `spec_auto.rs::ecriture::l_automatique_ecrit_exactement_deux_sur_un_canal_qui_sait`
//! pose « auto » sur un Kraken **sans courbe** et l'attend réussi. C'est
//! exactement ce que cette issue interdit : l'attente est **remplacée**, pas
//! contournée, et le canal doit désormais y recevoir sa courbe d'abord.

use std::fs;
use std::path::{Path, PathBuf};

use reverb_hw::hwmon::{self, Curve, FanChannel, Percent};

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
            "reverb-spec-courbe-avant-auto-{nom_du_test}-{}",
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

/// Contenu d'un fichier de l'arborescence temporaire, sans son saut de ligne.
fn lire(chemin: &Path) -> String {
    fs::read_to_string(chemin)
        .expect("lecture d'un fichier sysfs simulé")
        .trim()
        .to_owned()
}

/// Photographie de toute l'arborescence : chemin relatif → contenu.
///
/// Comparer deux photographies est plus sûr que de relire les quelques fichiers
/// auxquels on aurait pensé : un fichier oublié serait un fichier non
/// surveillé. C'est ce qui donne son sens à « rien n'est écrit ».
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
fn canal_complet(hwmon: &Path, n: u32, libelle: &str, enable: &str) {
    ecrire(&hwmon.join(format!("pwm{n}")), "64");
    ecrire(&hwmon.join(format!("pwm{n}_enable")), enable);
    ecrire(&hwmon.join(format!("fan{n}_input")), "700");
    ecrire(&hwmon.join(format!("fan{n}_label")), libelle);
}

/// Les 40 fichiers de courbe d'un indice de température, tous à « 0 ».
///
/// docs/VENTILATEURS.md — `kraken2023elite` expose `temp[1-2]_auto_point[1-40]_pwm`.
/// Sur le matériel ils sont en `0200`, donc **illisibles** ; ici ils se relisent,
/// et c'est justement ce qui permet de vérifier ce qui a été écrit.
fn fichiers_de_courbe(hwmon: &Path, temp: u32) {
    for m in 1..=40 {
        ecrire(&hwmon.join(format!("temp{temp}_auto_point{m}_pwm")), "0");
    }
}

/// L'arborescence du relevé : le Kraken avec ses courbes, un `nzxtsmart2` sans
/// mode automatique, une carte mère sans `pwm*_enable`.
fn arborescence_de_reference(nom_du_test: &str) -> FauxSysfs {
    let sysfs = FauxSysfs::neuf(nom_du_test);

    // Les deux canaux du Kraken, chacun avec ses quarante points. Ils partent en
    // « 0 », comme dans le relevé de #7.
    let kraken = sysfs.hwmon("hwmon6", "kraken2023elite");
    for (n, libelle, pwm) in [(1u32, "Pump speed", "171"), (2, "Fan speed", "71")] {
        ecrire(&kraken.join(format!("pwm{n}")), pwm);
        ecrire(&kraken.join(format!("pwm{n}_enable")), "0");
        ecrire(&kraken.join(format!("fan{n}_input")), "2380");
        ecrire(&kraken.join(format!("fan{n}_label")), libelle);
        fichiers_de_courbe(&kraken, n);
    }

    // Le contrôleur de ventilation : son pilote n'a aucun mode automatique (#50)
    // et il n'expose aucune courbe.
    let nzxt = sysfs.hwmon("hwmon4", "nzxtsmart2");
    for (n, libelle) in [(1u32, "FAN 1"), (2, "FAN 2"), (3, "FAN 3")] {
        canal_complet(&nzxt, n, libelle, "1");
    }

    // Une carte mère, qui n'expose aucun `pwm*_enable` (docs/VENTILATEURS.md).
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

/// Un pourcentage valable.
fn pct(pourcent: u8) -> Percent {
    Percent::new(pourcent).expect("un pourcentage entre 0 et 100")
}

/// Une courbe quelconque mais valable : une rampe de 30 % à 100 %.
///
/// Son contenu n'a aucune importance ici — seul compte le fait qu'une courbe ait
/// été posée. C'est précisément la connaissance que le pilote ne rend pas.
fn courbe_de_reference() -> Curve {
    Curve::interpolate(&[(1, pct(30)), (40, pct(100))])
        .expect("une rampe croissante de 30 % à 100 % est une courbe valable")
}

/// Le chemin `pwmN_enable` d'un canal, ou un échec explicite.
fn fichier_de_mode(canal: &FanChannel) -> &Path {
    canal
        .enable
        .as_deref()
        .unwrap_or_else(|| panic!("le canal « {} » expose son `pwmN_enable`", canal.name))
}

/// La pompe du Kraken — le canal de l'incident.
const POMPE: &str = "kraken2023elite:pump-speed";

/// Le ventilateur du Kraken, son voisin sur le même contrôleur.
const VENTILATEUR: &str = "kraken2023elite:fan-speed";

/// Les trois canaux `nzxtsmart2`, dont le pilote n'a aucun mode automatique (#50).
const CANAUX_SANS_AUTO: [&str; 3] = ["nzxtsmart2:fan-1", "nzxtsmart2:fan-2", "nzxtsmart2:fan-3"];

/// Les deux canaux de la carte mère, sans `pwmN_enable`.
const CANAUX_SANS_MODE: [&str; 2] = ["nct6687:cpu-fan", "nct6687:sys-fan-1"];

// ---------------------------------------------------------------------------
// 1 — « auto » sans courbe est refusé, et n'écrit rien
// ---------------------------------------------------------------------------

mod refus_sans_courbe {
    use super::{
        FauxSysfs, POMPE, VENTILATEUR, arborescence_de_reference, canal, canal_complet, ecarts,
        fichier_de_mode, fichiers_de_courbe, lire, photographie,
    };
    use reverb_hw::hwmon::{CourbesPosees, Mode, set_mode};

    #[test]
    fn l_auto_sans_courbe_est_refuse_et_n_ecrit_rien() {
        // issue #97, critère d'acceptation — « `set_mode(canal, HostCurve)` sur
        // un canal dont aucune courbe n'a été téléversée rend une erreur, et
        // **n'écrit rien** dans `pwmN_enable` ».
        //
        // Le « n'écrit rien » est tout le critère : écrire puis échouer laisserait
        // la pompe à 0 % de consigne, ce qui est exactement l'incident du
        // 2026-08-15 — `pwm1 = 0`, `pwm1_enable = 2`, 1910 tr/min.
        let sysfs = arborescence_de_reference("refus_sans_courbe");
        let canaux = sysfs.canaux();
        let posees = CourbesPosees::vide();

        let avant = photographie(sysfs.racine());
        for nom in [POMPE, VENTILATEUR] {
            let c = canal(&canaux, nom);
            assert!(
                set_mode(c, Mode::HostCurve, &posees).is_err(),
                "{nom} : « auto » sans courbe pousse un tableau de zéros"
            );
        }
        let apres = photographie(sysfs.racine());

        assert!(
            ecarts(&avant, &apres).is_empty(),
            "un refus n'écrit nulle part : {:?}",
            ecarts(&avant, &apres)
        );

        // Dit autrement, sur le fichier qui compte.
        for nom in [POMPE, VENTILATEUR] {
            assert_eq!(
                lire(fichier_de_mode(canal(&canaux, nom))),
                "0",
                "{nom} n'a pas bougé"
            );
        }
    }

    #[test]
    fn le_refus_nomme_le_canal_et_cite_reverb_curve() {
        // issue #97, critère d'acceptation — « Le message d'erreur nomme le canal
        // et cite `reverb curve` », et comportement attendu — « le refus **nomme**
        // ce qui manque et comment y remédier ».
        //
        // Convention du dépôt : ce qui est refusé est refusé **en le nommant**
        // (README, `eclairage.conf`, les profils, les sondes). Un utilisateur qui
        // lit « refusé » sans savoir sur quel canal ni quoi faire ensuite ira
        // chercher la panne ailleurs — et c'est un boîtier à quatorze canaux.
        let sysfs = arborescence_de_reference("refus_explicite");
        let canaux = sysfs.canaux();
        let posees = CourbesPosees::vide();

        for nom in [POMPE, VENTILATEUR] {
            let message = set_mode(canal(&canaux, nom), Mode::HostCurve, &posees)
                .expect_err("« auto » est refusé sans courbe")
                .to_string();
            let minuscules = message.to_lowercase();

            assert!(
                message.contains(nom),
                "le message doit nommer le canal visé : « {message} »"
            );
            assert!(
                minuscules.contains("curve"),
                "le message doit citer `reverb curve`, le moyen d'en sortir : « {message} »"
            );
            assert!(
                !minuscules.contains("os error"),
                "un errno nu n'explique rien à l'utilisateur : « {message} »"
            );
        }
    }

    #[test]
    fn le_refus_ne_depend_pas_de_la_valeur_deja_ecrite() {
        // Un canal qu'on trouverait **déjà** en « 2 » ne fait pas répondre
        // « d'accord » : c'est le cas de l'incident, où la machine est restée en
        // `pwm1_enable = 2` avec un tableau de zéros. Un court-circuit « la valeur
        // est déjà la bonne » rendrait le refus intermittent — le pire des deux
        // mondes — et rendrait surtout le seul état dangereux irréparable en
        // silence.
        let sysfs = FauxSysfs::neuf("refus_meme_deja_a_deux");
        let kraken = sysfs.hwmon("hwmon0", "kraken2023elite");
        canal_complet(&kraken, 1, "Pump speed", "2");
        fichiers_de_courbe(&kraken, 1);

        let canaux = sysfs.canaux();
        let c = canal(&canaux, POMPE);
        let posees = CourbesPosees::vide();

        let avant = photographie(sysfs.racine());
        assert!(
            set_mode(c, Mode::HostCurve, &posees).is_err(),
            "« auto » reste refusé, même si le fichier porte déjà « 2 »"
        );
        let apres = photographie(sysfs.racine());
        assert!(
            ecarts(&avant, &apres).is_empty(),
            "un refus n'écrit nulle part : {:?}",
            ecarts(&avant, &apres)
        );
    }

    #[test]
    fn la_presence_des_fichiers_de_courbe_ne_prouve_pas_qu_une_courbe_a_ete_posee() {
        // issue #97, approche technique — « les fichiers
        // `tempN_auto_pointM_pwm` sont en **écriture seule** (`0200`,
        // docs/VENTILATEURS.md), donc l'état ne se relit pas sur le matériel ».
        //
        // docs/VENTILATEURS.md — « Une courbe se pose, elle ne se relit jamais.
        // […] aucun état conservé côté hôte — il mentirait dès qu'un autre outil
        // écrirait ».
        //
        // Les deux canaux du Kraken portent leurs quarante fichiers dans
        // l'arborescence de référence : une implémentation qui déduirait « il y a
        // une courbe » de leur simple existence passerait pour prudente et
        // relancerait exactement l'incident.
        let sysfs = arborescence_de_reference("fichiers_ne_prouvent_rien");
        let canaux = sysfs.canaux();
        let posees = CourbesPosees::vide();

        for nom in [POMPE, VENTILATEUR] {
            let c = canal(&canaux, nom);
            assert_eq!(
                c.curve.len(),
                40,
                "{nom} porte bien ses quarante fichiers de courbe"
            );
            assert!(
                set_mode(c, Mode::HostCurve, &posees).is_err(),
                "{nom} : des fichiers en écriture seule ne disent rien de leur contenu"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 2 — une courbe posée ouvre « auto », sur ce canal et lui seul
// ---------------------------------------------------------------------------

mod apres_une_courbe {
    use super::{
        FauxSysfs, POMPE, VENTILATEUR, arborescence_de_reference, canal, canal_complet,
        courbe_de_reference, fichier_de_mode, lire,
    };
    use reverb_hw::hwmon::{CourbesPosees, Mode, set_curve, set_mode};

    #[test]
    fn poser_une_courbe_puis_auto_ecrit_exactement_deux() {
        // issue #97, critère d'acceptation — « `set_curve()` puis
        // `set_mode(HostCurve)` sur le même canal réussit », et tests d'intention
        // — « `pwmN_enable` vaut `2` ».
        //
        // La comparaison porte sur le texte exact du fichier : écrire `0` au lieu
        // de `2` envoie le canal à 100 % (#50), écrire `1` le fige sur sa dernière
        // consigne, et ni l'un ni l'autre ne se voit dans un message.
        let sysfs = arborescence_de_reference("courbe_puis_auto");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, POMPE);
        let mut posees = CourbesPosees::vide();

        set_curve(c, &courbe_de_reference(), &mut posees).expect("la courbe part sur le matériel");
        assert_eq!(
            lire(fichier_de_mode(c)),
            "0",
            "poser une courbe ne bascule pas le mode toute seule"
        );

        set_mode(c, Mode::HostCurve, &posees).expect("« auto » réussit après une courbe");
        assert_eq!(
            lire(fichier_de_mode(c)),
            "2",
            "« auto » écrit exactement « 2 »"
        );
    }

    #[test]
    fn la_memoire_est_par_canal_et_non_par_controleur() {
        // issue #97, tests d'intention — « La mémoire est **par canal** : une
        // courbe posée sur `pump-speed` n'autorise pas `HostCurve` sur
        // `fan-speed` ».
        //
        // Les deux canaux partagent leur contrôleur, leur pilote et le préfixe de
        // leur nom, mais pas leur tableau de points : `temp1_*` pilote la pompe,
        // `temp2_*` le ventilateur (docs/VENTILATEURS.md). Autoriser le voisin
        // enverrait le ventilateur sur un tableau de zéros.
        let sysfs = arborescence_de_reference("memoire_par_canal");
        let canaux = sysfs.canaux();
        let pompe = canal(&canaux, POMPE);
        let ventilateur = canal(&canaux, VENTILATEUR);
        let mut posees = CourbesPosees::vide();

        set_curve(pompe, &courbe_de_reference(), &mut posees).expect("la courbe de la pompe part");

        assert!(
            set_mode(pompe, Mode::HostCurve, &posees).is_ok(),
            "la pompe a sa courbe"
        );
        assert!(
            set_mode(ventilateur, Mode::HostCurve, &posees).is_err(),
            "le ventilateur n'a pas la sienne, quoique sur le même contrôleur"
        );
        assert_eq!(
            lire(fichier_de_mode(ventilateur)),
            "0",
            "le voisin refusé n'a pas bougé"
        );

        // Et il s'ouvre dès qu'il reçoit la sienne.
        set_curve(ventilateur, &courbe_de_reference(), &mut posees)
            .expect("la courbe du ventilateur part");
        set_mode(ventilateur, Mode::HostCurve, &posees).expect("« auto » réussit à son tour");
        assert_eq!(lire(fichier_de_mode(ventilateur)), "2");
    }

    #[test]
    fn une_courbe_qui_a_echoue_n_autorise_rien() {
        // Corollaire du choix de couture (en-tête, point 1) : le carnet retient
        // ce qui est **parti**, jamais ce qui a été tenté. Un canal sans fichier
        // de courbe fait échouer `set_curve` sans rien écrire (#9) ; « auto » doit
        // rester refusé après, sans quoi une tentative ratée ouvrirait la porte
        // que l'issue ferme.
        let sysfs = FauxSysfs::neuf("courbe_echouee");
        let kraken = sysfs.hwmon("hwmon0", "kraken2023elite");
        canal_complet(&kraken, 1, "Pump speed", "0");

        let canaux = sysfs.canaux();
        let c = canal(&canaux, POMPE);
        let mut posees = CourbesPosees::vide();

        assert!(
            c.curve.is_empty(),
            "ce canal n'expose aucun point de courbe"
        );
        assert!(
            set_curve(c, &courbe_de_reference(), &mut posees).is_err(),
            "sans fichier de points, la courbe ne part pas"
        );
        assert!(
            set_mode(c, Mode::HostCurve, &posees).is_err(),
            "une courbe qui n'est pas partie n'autorise pas « auto »"
        );
        assert_eq!(lire(fichier_de_mode(c)), "0", "le canal n'a pas bougé");
    }
}

// ---------------------------------------------------------------------------
// 3 — la mémoire ne survit pas au redémarrage du démon
// ---------------------------------------------------------------------------

mod redemarrage {
    use super::{
        POMPE, arborescence_de_reference, canal, courbe_de_reference, ecarts, fichier_de_mode,
        lire, photographie,
    };
    use reverb_hw::hwmon::{CourbesPosees, Mode, set_curve, set_mode};

    #[test]
    fn un_carnet_neuf_ne_sait_rien_meme_quand_le_materiel_est_reste_en_courbe_de_l_hote() {
        // issue #97, tests d'intention — « La mémoire ne survit pas à un
        // redémarrage du démon », et approche technique — « il est **remis à zéro
        // au démarrage** — c'est correct, puisqu'on ne peut rien savoir de ce
        // qu'un autre outil a écrit ».
        //
        // Un carnet vide **est** un démon qui redémarre : c'est ce qui rend le
        // critère exerçable dans un processus qui, lui, ne redémarre pas. Le
        // matériel, de son côté, est resté en « 2 » — rien ne se réinitialise à
        // l'arrêt du démon —, et c'est précisément l'état qui ne doit pas être pris
        // pour une preuve.
        let sysfs = arborescence_de_reference("carnet_neuf");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, POMPE);

        let mut premiere_vie = CourbesPosees::vide();
        set_curve(c, &courbe_de_reference(), &mut premiere_vie).expect("la courbe part");
        set_mode(c, Mode::HostCurve, &premiere_vie).expect("« auto » réussit dans cette vie-là");
        assert_eq!(lire(fichier_de_mode(c)), "2");

        // Le démon redémarre. Le carnet repart vide, le matériel non.
        let seconde_vie = CourbesPosees::vide();
        let avant = photographie(sysfs.racine());
        assert!(
            set_mode(c, Mode::HostCurve, &seconde_vie).is_err(),
            "un démon qui redémarre ne sait plus quelle courbe le pilote détient"
        );
        let apres = photographie(sysfs.racine());
        assert!(
            ecarts(&avant, &apres).is_empty(),
            "et ce refus-là n'écrit rien non plus : {:?}",
            ecarts(&avant, &apres)
        );
    }

    #[test]
    fn deux_carnets_ne_se_partagent_rien() {
        // Même critère, pris par l'autre bout : la mémoire est portée par le
        // carnet, jamais par un état global du processus. Sans cette propriété,
        // « remis à zéro au démarrage » ne serait vrai que par la grâce du fait
        // qu'un démon est un processus, et deux tests parallèles se
        // contamineraient l'un l'autre.
        let sysfs = arborescence_de_reference("deux_carnets");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, POMPE);

        let mut le_mien = CourbesPosees::vide();
        let le_tien = CourbesPosees::vide();
        set_curve(c, &courbe_de_reference(), &mut le_mien).expect("la courbe part");

        assert!(set_mode(c, Mode::HostCurve, &le_mien).is_ok());
        assert!(
            set_mode(c, Mode::HostCurve, &le_tien).is_err(),
            "un carnet ne sait que ce qu'il a lui-même vu partir"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — les autres modes ne sont pas concernés
// ---------------------------------------------------------------------------

mod les_autres_modes {
    use super::{
        POMPE, VENTILATEUR, arborescence_de_reference, canal, courbe_de_reference, ecarts,
        fichier_de_mode, lire, photographie,
    };
    use reverb_hw::hwmon::{CourbesPosees, Mode, set_curve, set_mode};

    #[test]
    fn manuel_et_plein_regime_restent_ecrivables_sans_courbe() {
        // issue #97, tests d'intention — « `Manual` et `PleinRegime` restent
        // écrivables sans courbe ».
        //
        // Ce sont les deux sorties de secours : `Manual` (`1`) impose une consigne
        // fixe, `PleinRegime` (`0`) envoie le canal à 100 % — bruyant, mais sûr
        // (#50). Les fermer aussi laisserait un canal coincé en « 2 » sans aucun
        // moyen d'en sortir sans coupure d'alimentation.
        let sysfs = arborescence_de_reference("autres_modes");
        let canaux = sysfs.canaux();
        let posees = CourbesPosees::vide();

        let pompe = canal(&canaux, POMPE);
        set_mode(pompe, Mode::Manual, &posees).expect("le manuel n'a jamais eu besoin de courbe");
        assert_eq!(lire(fichier_de_mode(pompe)), "1");

        let ventilateur = canal(&canaux, VENTILATEUR);
        set_mode(ventilateur, Mode::PleinRegime, &posees)
            .expect("le plein régime n'a jamais eu besoin de courbe");
        assert_eq!(lire(fichier_de_mode(ventilateur)), "0");
    }

    #[test]
    fn un_mode_inconnu_reste_refuse_avec_ou_sans_courbe() {
        // #50, dernier critère d'acceptation — « Un mode lu et inconnu continue de
        // se lire sans se réécrire ». Une courbe posée ouvre `HostCurve`, et rien
        // d'autre : CLAUDE.md — « Ne jamais inventer une trame absente des specs. »
        let sysfs = arborescence_de_reference("inconnu_avec_courbe");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, POMPE);
        let mut posees = CourbesPosees::vide();
        set_curve(c, &courbe_de_reference(), &mut posees).expect("la courbe part");

        let avant = photographie(sysfs.racine());
        for valeur in [3u8, 7, 255] {
            assert!(
                set_mode(c, Mode::Unknown(valeur), &posees).is_err(),
                "`Mode::Unknown({valeur})` n'est pas réémis, courbe ou pas"
            );
        }
        assert!(
            set_mode(c, Mode::Unsupported, &posees).is_err(),
            "`Mode::Unsupported` ne s'écrit pas davantage"
        );
        let apres = photographie(sysfs.racine());
        assert!(
            ecarts(&avant, &apres).is_empty(),
            "un refus n'écrit nulle part : {:?}",
            ecarts(&avant, &apres)
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — les deux refus déjà en place ne changent pas
// ---------------------------------------------------------------------------

mod refus_inchanges {
    use super::{
        CANAUX_SANS_AUTO, CANAUX_SANS_MODE, FauxSysfs, arborescence_de_reference, canal,
        canal_complet, courbe_de_reference, ecarts, fichiers_de_courbe, photographie,
    };
    use reverb_hw::hwmon::{CourbesPosees, Mode, set_curve, set_mode};

    #[test]
    fn un_canal_sans_fichier_enable_reste_refuse_sans_rien_ecrire() {
        // issue #97, critère d'acceptation — « Un canal sans `pwmN_enable`
        // continue d'être refusé comme avant », et #7 — `set_mode` « échoue si le
        // canal n'expose pas ce fichier ».
        //
        // Le message ne doit pas devenir celui de la courbe : sur un canal qui n'a
        // aucun fichier de mode, `reverb curve` ne changerait rien, et envoyer
        // l'utilisateur poser une courbe pour un canal qui n'en exécutera jamais
        // est un faux diagnostic. C'est le sens de « comme avant ».
        let sysfs = arborescence_de_reference("sans_enable");
        let canaux = sysfs.canaux();
        let posees = CourbesPosees::vide();

        let avant = photographie(sysfs.racine());
        for nom in CANAUX_SANS_MODE {
            let c = canal(&canaux, nom);
            assert!(c.enable.is_none(), "{nom} : la source n'expose aucun mode");
            let message = set_mode(c, Mode::HostCurve, &posees)
                .expect_err("sans fichier de mode, il n'y a nulle part où écrire")
                .to_string();
            assert!(
                !message.to_lowercase().contains("curve"),
                "{nom} : une courbe n'y changerait rien, le message ne doit pas la citer : « {message} »"
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
    fn un_pilote_sans_mode_automatique_garde_son_message() {
        // issue #97, critère d'acceptation — « Un canal dont le pilote n'a pas de
        // mode auto continue d'être refusé comme avant, avec son message actuel ».
        //
        // #50 : `nzxt-smart2` ne réaccepte que la valeur qu'il porte déjà, donc ce
        // contrôleur n'a **aucun** mode automatique, et son refus le dit en nommant
        // le contrôleur.
        let sysfs = arborescence_de_reference("sans_mode_auto");
        let canaux = sysfs.canaux();
        let posees = CourbesPosees::vide();

        for nom in CANAUX_SANS_AUTO {
            let message = set_mode(canal(&canaux, nom), Mode::HostCurve, &posees)
                .expect_err("« auto » est refusé sur `nzxtsmart2`")
                .to_string();
            let minuscules = message.to_lowercase();
            assert!(
                minuscules.contains("automatique"),
                "{nom} : le message doit dire qu'il n'y a pas de mode automatique : « {message} »"
            );
            assert!(
                message.contains("nzxtsmart2"),
                "{nom} : le message doit nommer le contrôleur fautif : « {message} »"
            );
        }
    }

    #[test]
    fn une_courbe_posee_ne_donne_pas_un_mode_automatique_a_un_pilote_qui_n_en_a_pas() {
        // Les deux conditions sont un **et**, pas un ou. Un contrôleur qui
        // exposerait des fichiers de points sans savoir exécuter `pwm_enable = 2`
        // reste refusé, et avec son message à lui : c'est le pilote qui décide, la
        // courbe ne fait qu'ouvrir une porte que le pilote doit avoir.
        //
        // #50 — la liste des pilotes qui savent est une **liste d'autorisation**,
        // pas une liste d'exclusion.
        let sysfs = FauxSysfs::neuf("courbe_sans_pilote");
        let nzxt = sysfs.hwmon("hwmon0", "nzxtsmart2");
        canal_complet(&nzxt, 1, "FAN 1", "1");
        fichiers_de_courbe(&nzxt, 1);

        let canaux = sysfs.canaux();
        let c = canal(&canaux, "nzxtsmart2:fan-1");
        let mut posees = CourbesPosees::vide();
        set_curve(c, &courbe_de_reference(), &mut posees)
            .expect("les fichiers existent, la courbe part");

        let message = set_mode(c, Mode::HostCurve, &posees)
            .expect_err("« auto » reste refusé : ce pilote n'a pas de mode automatique")
            .to_string();
        assert!(
            message.to_lowercase().contains("automatique"),
            "le refus qui l'emporte est celui du pilote : « {message} »"
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — le bouton, et ce qu'il promet
// ---------------------------------------------------------------------------

mod le_bouton {
    use super::{
        CANAUX_SANS_AUTO, CANAUX_SANS_MODE, POMPE, VENTILATEUR, arborescence_de_reference, canal,
        courbe_de_reference, ecarts, photographie,
    };
    use reverb_hw::hwmon::{CourbesPosees, Mode, set_curve, set_mode};

    #[test]
    fn un_canal_sans_courbe_n_autorise_pas_l_auto_meme_quand_son_pilote_sait() {
        // issue #97, comportement attendu — « La fenêtre n'offre plus "auto" sur
        // un canal sans courbe téléversée ».
        //
        // La question du pilote (#50) et celle de la courbe (#97) sont deux
        // questions distinctes : `sait_faire_auto` continue de dire ce dont le
        // matériel est capable, `autorise_auto` dit ce qui marcherait **maintenant**.
        // C'est la seconde que la fenêtre doit regarder.
        let sysfs = arborescence_de_reference("bouton_sans_courbe");
        let canaux = sysfs.canaux();
        let posees = CourbesPosees::vide();

        for nom in [POMPE, VENTILATEUR] {
            let c = canal(&canaux, nom);
            assert!(
                c.sait_faire_auto(),
                "{nom} : son pilote sait exécuter une courbe (#50)"
            );
            assert!(
                !posees.autorise_auto(c),
                "{nom} : mais aucune courbe n'y a été posée, le bouton ne peut qu'échouer"
            );
        }
    }

    #[test]
    fn le_bouton_suit_le_canal_qui_a_recu_la_courbe() {
        // Même règle que le refus (§2) : la mémoire est par canal. Un bouton
        // proposé sur le voisin enverrait le ventilateur sur un tableau de zéros.
        let sysfs = arborescence_de_reference("bouton_par_canal");
        let canaux = sysfs.canaux();
        let pompe = canal(&canaux, POMPE);
        let ventilateur = canal(&canaux, VENTILATEUR);
        let mut posees = CourbesPosees::vide();

        set_curve(pompe, &courbe_de_reference(), &mut posees).expect("la courbe de la pompe part");

        assert!(posees.autorise_auto(pompe), "la pompe a reçu sa courbe");
        assert!(
            !posees.autorise_auto(ventilateur),
            "le ventilateur n'a pas reçu la sienne"
        );
    }

    #[test]
    fn le_bouton_reste_ferme_sans_mode_automatique_materiel() {
        // Les canaux que #50 avait déjà écartés le restent : un pilote sans mode
        // automatique, un canal sans `pwmN_enable`. Aucune courbe ne les rouvre —
        // ils n'ont rien où écrire « 2 », ou personne pour l'exécuter.
        let sysfs = arborescence_de_reference("bouton_ferme");
        let canaux = sysfs.canaux();
        let posees = CourbesPosees::vide();

        for nom in CANAUX_SANS_AUTO
            .iter()
            .chain(CANAUX_SANS_MODE.iter())
            .copied()
        {
            assert!(
                !posees.autorise_auto(canal(&canaux, nom)),
                "{nom} : ce canal n'a jamais su passer en automatique"
            );
        }
    }

    #[test]
    fn poser_la_question_n_ecrit_rien_et_ne_change_pas_d_avis() {
        // #50, approche technique — la question doit être gratuite et sans effet :
        // « une tentative allume les ventilateurs à fond dans le cas où elle
        // réussit ». La fenêtre la pose à chaque `status`, une fois par seconde.
        let sysfs = arborescence_de_reference("question_sans_effet");
        let canaux = sysfs.canaux();
        let mut posees = CourbesPosees::vide();
        set_curve(canal(&canaux, POMPE), &courbe_de_reference(), &mut posees)
            .expect("la courbe part");

        let avant = photographie(sysfs.racine());
        let premieres: Vec<bool> = canaux.iter().map(|c| posees.autorise_auto(c)).collect();
        let secondes: Vec<bool> = canaux.iter().map(|c| posees.autorise_auto(c)).collect();
        let apres = photographie(sysfs.racine());

        assert!(
            ecarts(&avant, &apres).is_empty(),
            "poser la question n'écrit nulle part : {:?}",
            ecarts(&avant, &apres)
        );
        assert_eq!(
            premieres, secondes,
            "la réponse ne dépend pas du nombre de fois qu'on la demande"
        );
    }

    #[test]
    fn le_bouton_et_le_refus_ne_se_contredisent_jamais() {
        // README, à propos de `nzxt-smart2` — « montrer un bouton qui ne peut
        // qu'échouer vaut moins que ne pas le montrer ». La réciproque compte
        // autant : cacher un bouton qui marcherait, c'est une capacité perdue sans
        // le dire.
        //
        // Ce test est le lien entre les deux moitiés de l'issue. Il tient sur tous
        // les canaux du relevé, avec un carnet où une seule courbe a été posée —
        // donc sur les quatre cas à la fois : sait et a sa courbe, sait sans
        // courbe, ne sait pas, n'a pas de fichier de mode.
        let sysfs = arborescence_de_reference("pas_de_contradiction");
        let canaux = sysfs.canaux();
        let mut posees = CourbesPosees::vide();
        set_curve(canal(&canaux, POMPE), &courbe_de_reference(), &mut posees)
            .expect("la courbe part");

        for c in &canaux {
            let promis = posees.autorise_auto(c);
            let tenu = set_mode(c, Mode::HostCurve, &posees).is_ok();
            assert_eq!(
                promis, tenu,
                "{} : le bouton promet {promis}, l'écriture rend {tenu}",
                c.name
            );
        }
    }
}
