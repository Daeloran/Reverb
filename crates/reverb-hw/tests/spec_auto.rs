//! Tests d'intention du mode automatique des ventilateurs (issue #50).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #50 seule. Rien n'est relu
//! depuis `crates/reverb-hw/src/` : à l'écriture de ce fichier, ni
//! `FanChannel::sait_faire_auto` ni `Mode::PleinRegime` n'existent, et
//! `set_mode` ne connaît pas encore le nom du pilote. Ils encodent ce que le
//! logiciel doit faire, pas ce que le code fait — si l'un d'eux échoue après
//! implémentation, c'est le code qu'on corrige, jamais le test.
//!
//! Ils prolongent `spec_fans.rs` (#7), auquel **ce fichier ne touche pas**.
//!
//! # Le fait mesuré, et les deux sources qui l'expliquent
//!
//! issue #50 — `fan nzxtsmart2:fan-1 auto` écrit aujourd'hui `0` dans
//! `pwmN_enable` et le démon répond `err Operation not supported (os error 95)`.
//! Les deux pilotes noyau disent pourquoi, et différemment.
//!
//! **`nzxt-smart2`** (canaux `nzxtsmart2:fan-1..3`) — `set_pwm_enable()` :
//!
//! ```c
//! expected_val = drvdata->fan_type[channel] != FAN_TYPE_NONE;
//! return (val == expected_val) ? 0 : -EOPNOTSUPP;
//! ```
//!
//! `pwm_enable` n'y est **pas un sélecteur de mode** : il ne réaccepte que la
//! valeur qu'il porte déjà. **Ce contrôleur n'a aucun mode automatique.**
//!
//! **`nzxt-kraken3`** (canaux `kraken2023elite:fan-speed`, `…:pump-speed`) —
//! `kraken3_write` accepte `0`, `1` et `2`, et `0` ne veut **pas** dire
//! « laissé au firmware » :
//!
//! ```c
//! case 0:
//!     /* Set channel to 100%, direct duty value */
//!     ret = kraken3_write_fixed_duty(priv, 255, channel);
//! ```
//!
//! `0` = **100 % et on lâche la barre**. Le vrai « rendre la main à la courbe »
//! est `2`, et seul le Kraken l'a.
//!
//! # Ce que ce fichier fige
//!
//! 1. Un canal sait dire s'il **peut** passer en automatique, et la réponse se
//!    déduit du **nom du pilote** — jamais d'une tentative d'écriture, qui
//!    enverrait les ventilateurs à fond dans le cas où elle réussit.
//! 2. Demander l'automatique à un canal qui ne sait pas est refusé **sans
//!    qu'aucun fichier ne soit écrit**, avec un message qui le dit.
//! 3. Demander l'automatique à un canal qui sait écrit **exactement `2`**.
//! 4. La valeur `0` n'est plus nommée « laissé au firmware » : son libellé dit
//!    ce qu'elle fait — 100 %, sans régulation.
//! 5. Un mode lu et inconnu continue de se lire sans se réécrire (issue #50,
//!    dernier critère d'acceptation : « comportement actuel, à ne pas casser »).
//!
//! # Aucun accès matériel
//!
//! Convention du dépôt, reprise de `spec_fans.rs` : les seuls chemins touchés
//! ici sont ceux d'une arborescence construite dans `std::env::temp_dir()`,
//! effacée à la fin de chaque test. **Rien n'est lu ni écrit sous `/sys`.**
//! `hwmon::discover_in` prend une racine précisément pour cela.
//!
//! # Les noms choisis, et pourquoi
//!
//! L'issue décrit le comportement et laisse les noms ouverts. Ce fichier les
//! tranche, puisque c'est lui le contrat :
//!
//! - **`FanChannel::sait_faire_auto(&self) -> bool`**. Pas de `io::Result` :
//!   la réponse vient du nom du pilote, déjà lu par la découverte. Une
//!   signature faillible laisserait croire qu'il faut aller voir le matériel,
//!   ce que l'issue interdit explicitement.
//! - **`Mode::PleinRegime`** pour la valeur `0`, d'après ce que le pilote en
//!   fait : `kraken3_write_fixed_duty(priv, 255, channel)`. Son libellé dit
//!   100 %, et ne dit plus « firmware ».
//! - **`Mode::HostCurve` reste `Mode::HostCurve`.** L'issue ne demande de
//!   renommer que `0`, et `2` est déjà nommé pour ce qu'il fait.
//!
//! # Trois points que l'issue ne tranche pas, et que ce fichier tranche
//!
//! 1. **Un pilote dont l'issue ne dit rien ne prétend pas savoir.** L'issue
//!    nomme deux pilotes ; pour les autres, la liste est une **liste
//!    d'autorisation**, pas une liste d'exclusion. Motif : le coût des deux
//!    erreurs n'est pas le même. Cacher un bouton légitime se répare d'une
//!    ligne ; le montrer à tort envoie un canal à 100 % en silence, ce qui est
//!    exactement la panne que l'issue décrit sur la pompe.
//! 2. **Un canal sans `pwmN_enable` ne sait pas faire auto**, même si son
//!    pilote le sait en général : il n'y a aucun fichier où écrire `2`, donc le
//!    bouton ne pourrait qu'échouer.
//! 3. **Le refus est produit avant toute écriture, y compris quand le canal est
//!    déjà à `2`.** Un no-op silencieux dans ce cas rendrait le refus
//!    dépendant de l'état courant, alors qu'il dépend du matériel.

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
            "reverb-spec-auto-{nom_du_test}-{}",
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
/// surveillé. C'est ce qui donne son sens à « refusé **sans qu'aucun fichier ne
/// soit écrit** ».
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

/// L'arborescence du relevé, telle que `spec_fans.rs` la décrit.
///
/// `nzxtsmart2` en `1` — c'est la seule valeur que son pilote réaccepte —,
/// `kraken2023elite` en `0`, et trois sources de bruit : une sans `pwm*`, une
/// sans aucun `pwm*_enable`, une en `2`.
fn arborescence_de_reference(nom_du_test: &str) -> FauxSysfs {
    let sysfs = FauxSysfs::neuf(nom_du_test);

    let nzxt = sysfs.hwmon("hwmon4", "nzxtsmart2");
    for (n, libelle) in [(1, "FAN 1"), (2, "FAN 2"), (3, "FAN 3")] {
        canal_complet(&nzxt, n, libelle, "1");
    }

    let kraken = sysfs.hwmon("hwmon6", "kraken2023elite");
    for (n, libelle, pwm) in [(1, "Pump speed", "171"), (2, "Fan speed", "71")] {
        ecrire(&kraken.join(format!("pwm{n}")), pwm);
        ecrire(&kraken.join(format!("pwm{n}_enable")), "0");
        ecrire(&kraken.join(format!("fan{n}_input")), "2380");
        ecrire(&kraken.join(format!("fan{n}_label")), libelle);
    }

    // Une source qui n'expose aucun `pwm*_enable` (spec_fans.rs, #7).
    let nct = sysfs.hwmon("hwmon2", "nct6687");
    for (n, libelle) in [(1, "CPU FAN"), (2, "SYS FAN 1")] {
        ecrire(&nct.join(format!("pwm{n}")), "153");
        ecrire(&nct.join(format!("fan{n}_input")), "0");
        ecrire(&nct.join(format!("fan{n}_label")), libelle);
    }

    // Une source sans le moindre `pwm*`, et une source déjà en `2`.
    let nvme = sysfs.hwmon("hwmon0", "nvme");
    ecrire(&nvme.join("temp1_input"), "38850");

    let autre = sysfs.hwmon("hwmon9", "amdgpu");
    ecrire(&autre.join("pwm1"), "128");
    ecrire(&autre.join("pwm1_enable"), "2");

    sysfs
}

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

/// Les deux canaux du Kraken, seuls du relevé à savoir passer en automatique.
const CANAUX_DU_KRAKEN: [&str; 2] = ["kraken2023elite:pump-speed", "kraken2023elite:fan-speed"];

/// Les trois canaux `nzxtsmart2`, dont le pilote n'a aucun mode automatique.
const CANAUX_SANS_AUTO: [&str; 3] = ["nzxtsmart2:fan-1", "nzxtsmart2:fan-2", "nzxtsmart2:fan-3"];

// ---------------------------------------------------------------------------
// 1 — un canal sait dire s'il peut passer en automatique
// ---------------------------------------------------------------------------

mod sait_faire_auto {
    use super::{
        CANAUX_DU_KRAKEN, CANAUX_SANS_AUTO, FauxSysfs, arborescence_de_reference, canal,
        canal_complet, ecarts, ecrire, photographie,
    };

    #[test]
    fn les_deux_canaux_du_kraken_savent_passer_en_automatique() {
        // issue #50, comportement attendu — « Sur les deux canaux du Kraken,
        // "auto" rend la main à la courbe du périphérique (`pwm_enable = 2`) ».
        // Le pilote `nzxt-kraken3` accepte `0`, `1` et `2` ; `2` est le seul qui
        // rende vraiment la main.
        let sysfs = arborescence_de_reference("kraken_sait");
        let canaux = sysfs.canaux();

        for nom in CANAUX_DU_KRAKEN {
            assert!(
                canal(&canaux, nom).sait_faire_auto(),
                "{nom} : son pilote exécute la courbe du périphérique en `pwm_enable = 2`"
            );
        }
    }

    #[test]
    fn les_trois_canaux_nzxtsmart2_ne_savent_pas() {
        // issue #50, comportement attendu — « Sur les trois canaux
        // `nzxtsmart2`, "auto" **n'est pas proposé** : le matériel n'en a pas. »
        //
        // `nzxt-smart2`, `set_pwm_enable()` :
        //     expected_val = drvdata->fan_type[channel] != FAN_TYPE_NONE;
        //     return (val == expected_val) ? 0 : -EOPNOTSUPP;
        //
        // `pwm_enable` n'y est pas un sélecteur de mode : il ne réaccepte que la
        // valeur qu'il porte déjà.
        let sysfs = arborescence_de_reference("nzxtsmart2_ne_sait_pas");
        let canaux = sysfs.canaux();

        for nom in CANAUX_SANS_AUTO {
            assert!(
                !canal(&canaux, nom).sait_faire_auto(),
                "{nom} : ce contrôleur n'a aucun mode automatique"
            );
        }
    }

    #[test]
    fn la_reponse_vient_du_pilote_et_non_du_mode_courant() {
        // issue #50, approche technique — « La réponse se déduit du nom du
        // pilote […] et non d'une tentative d'écriture ». Elle ne se déduit pas
        // davantage de la valeur *déjà* écrite : un Kraken laissé en manuel sait
        // toujours revenir à sa courbe, et un `nzxtsmart2` qu'on trouverait à
        // `2` ne saurait toujours pas.
        //
        // Trois valeurs de `pwmN_enable` pour chaque pilote, mêmes réponses
        // attendues : c'est ce qui distingue « lit le nom du pilote » de « lit
        // le fichier ».
        let sysfs = FauxSysfs::neuf("pilote_pas_mode");
        let kraken = sysfs.hwmon("hwmon0", "kraken2023elite");
        let nzxt = sysfs.hwmon("hwmon1", "nzxtsmart2");
        for (n, enable) in [(1, "0"), (2, "1"), (3, "2")] {
            canal_complet(&kraken, n, &format!("Fan {n}"), enable);
            canal_complet(&nzxt, n, &format!("FAN {n}"), enable);
        }

        let canaux = sysfs.canaux();
        for n in 1..=3 {
            assert!(
                canal(&canaux, &format!("kraken2023elite:fan-{n}")).sait_faire_auto(),
                "le Kraken sait faire auto quel que soit son mode courant"
            );
            assert!(
                !canal(&canaux, &format!("nzxtsmart2:fan-{n}")).sait_faire_auto(),
                "`nzxtsmart2` ne sait pas faire auto, même trouvé en « 2 »"
            );
        }
    }

    #[test]
    fn la_reponse_ne_vient_ni_du_libelle_ni_de_l_indice() {
        // Même critère, pris par l'autre bout : c'est la **source** qui décide,
        // pas le libellé du canal. Un `nzxtsmart2` dont les canaux
        // s'appelleraient « Pump speed » ne devient pas un Kraken, et un Kraken
        // dont les canaux s'appelleraient « FAN 1 » reste un Kraken.
        //
        // Sans ce test, une implémentation qui reconnaîtrait « pump » ou
        // « speed » dans le libellé passerait les deux tests précédents.
        let sysfs = FauxSysfs::neuf("pilote_pas_libelle");
        let menteur = sysfs.hwmon("hwmon0", "nzxtsmart2");
        canal_complet(&menteur, 1, "Pump speed", "1");
        canal_complet(&menteur, 2, "Fan speed", "1");
        let discret = sysfs.hwmon("hwmon1", "kraken2023elite");
        canal_complet(&discret, 1, "FAN 1", "0");

        let canaux = sysfs.canaux();
        for nom in ["nzxtsmart2:pump-speed", "nzxtsmart2:fan-speed"] {
            assert!(
                !canal(&canaux, nom).sait_faire_auto(),
                "{nom} : le libellé ne fait pas le pilote"
            );
        }
        assert!(
            canal(&canaux, "kraken2023elite:fan-1").sait_faire_auto(),
            "un canal du Kraken sait faire auto quel que soit son libellé"
        );
    }

    #[test]
    fn un_pilote_dont_l_issue_ne_dit_rien_ne_pretend_pas_savoir() {
        // Arbitrage de ce fichier (voir l'en-tête, point 1) : liste
        // d'autorisation, pas liste d'exclusion. L'issue nomme deux pilotes ;
        // pour tous les autres, on ne sait pas, et « on ne sait pas » se répond
        // « non » — parce que montrer le bouton à tort envoie un canal à 100 %
        // en silence, ce qui est la panne même que l'issue décrit.
        //
        // CLAUDE.md — « Ne jamais implémenter depuis un ❓ ».
        let sysfs = arborescence_de_reference("pilote_inconnu");
        let canaux = sysfs.canaux();

        for nom in ["nct6687:cpu-fan", "nct6687:sys-fan-1", "amdgpu:fan1"] {
            assert!(
                !canal(&canaux, nom).sait_faire_auto(),
                "{nom} : l'issue ne dit rien de ce pilote, on ne promet rien"
            );
        }
    }

    #[test]
    fn un_canal_sans_fichier_enable_ne_sait_pas_faire_auto() {
        // Arbitrage de ce fichier (en-tête, point 2). Le pilote a beau savoir
        // en général, il n'y a ici aucun `pwm1_enable` où écrire `2` : le
        // bouton ne pourrait qu'échouer, et l'issue demande justement que la
        // fenêtre « n'affiche pas un bouton qui ne peut qu'échouer ».
        let sysfs = FauxSysfs::neuf("kraken_sans_enable");
        let kraken = sysfs.hwmon("hwmon0", "kraken2023elite");
        ecrire(&kraken.join("pwm1"), "171");
        ecrire(&kraken.join("fan1_label"), "Pump speed");

        let canaux = sysfs.canaux();
        let c = canal(&canaux, "kraken2023elite:pump-speed");
        assert!(
            c.enable.is_none(),
            "le canal témoin n'a pas de `pwm1_enable`"
        );
        assert!(
            !c.sait_faire_auto(),
            "sans fichier de mode, il n'y a nulle part où écrire « 2 »"
        );
    }

    #[test]
    fn demander_si_un_canal_sait_faire_auto_n_ecrit_rien_et_ne_change_pas_d_avis() {
        // issue #50, approche technique — « et non d'une tentative d'écriture,
        // parce qu'une tentative allume les ventilateurs à fond dans le cas où
        // elle réussit ». C'est le cœur de l'issue : la question doit être
        // gratuite et sans effet.
        //
        // Toute l'arborescence est photographiée, pas seulement les fichiers
        // auxquels on aurait pensé.
        let sysfs = arborescence_de_reference("question_sans_effet");
        let canaux = sysfs.canaux();

        let avant = photographie(sysfs.racine());
        let premieres: Vec<bool> = canaux.iter().map(|c| c.sait_faire_auto()).collect();
        let secondes: Vec<bool> = canaux.iter().map(|c| c.sait_faire_auto()).collect();
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
}

// ---------------------------------------------------------------------------
// 2 — l'automatique refusé, sans rien écrire
// ---------------------------------------------------------------------------

mod refus {
    use super::{
        CANAUX_SANS_AUTO, FauxSysfs, arborescence_de_reference, canal, canal_complet, ecarts, lire,
        photographie,
    };
    use reverb_hw::hwmon::{Mode, set_mode};

    #[test]
    fn l_automatique_sur_un_canal_qui_ne_sait_pas_est_refuse_sans_aucune_ecriture() {
        // issue #50, critère d'acceptation — « `fan nzxtsmart2:fan-1 auto` est
        // refusé **avant** toute écriture ». Le mot « avant » est tout le
        // critère : aujourd'hui l'écriture part, le noyau la refuse, et
        // l'utilisateur reçoit un errno nu.
        let sysfs = arborescence_de_reference("refus_sans_ecriture");
        let canaux = sysfs.canaux();

        let avant = photographie(sysfs.racine());
        for nom in CANAUX_SANS_AUTO {
            let c = canal(&canaux, nom);
            assert!(
                set_mode(c, Mode::HostCurve).is_err(),
                "{nom} : « auto » doit être refusé, son pilote n'en a pas"
            );
        }
        let apres = photographie(sysfs.racine());

        assert!(
            ecarts(&avant, &apres).is_empty(),
            "un refus n'écrit nulle part : {:?}",
            ecarts(&avant, &apres)
        );

        // Dit autrement, sur le fichier qui compte : il porte toujours la valeur
        // que `nzxt-smart2` réaccepte, et pas le `2` qu'on lui aurait demandé.
        for nom in CANAUX_SANS_AUTO {
            let enable = canal(&canaux, nom)
                .enable
                .as_ref()
                .expect("`nzxtsmart2` expose son `pwmN_enable`");
            assert_eq!(lire(enable), "1", "{nom} n'a pas bougé");
        }
    }

    #[test]
    fn le_refus_dit_que_ce_controleur_n_a_pas_de_mode_automatique() {
        // issue #50, critère d'acceptation — « avec un message qui dit que ce
        // contrôleur n'a pas de mode automatique », et comportement attendu —
        // « Un canal qui refuse un mode le dit avec le message du noyau, pas
        // avec un code errno nu ».
        //
        // Le message part tel quel sur le socket (issue #50, approche
        // technique : « Le refus est produit dans `reverb-hw` […] et remonte tel
        // quel par le socket »). C'est donc ce que l'utilisateur lira.
        //
        // Convention du dépôt — ce qui est refusé est refusé **en le nommant**
        // (README, `eclairage.conf`) : le contrôleur fautif doit apparaître,
        // sinon l'utilisateur ne sait pas lequel de ses cinq canaux il a visé.
        let sysfs = arborescence_de_reference("refus_explicite");
        let canaux = sysfs.canaux();

        let message = set_mode(canal(&canaux, "nzxtsmart2:fan-1"), Mode::HostCurve)
            .expect_err("« auto » est refusé sur `nzxtsmart2`")
            .to_string();

        let minuscules = message.to_lowercase();
        assert!(
            minuscules.contains("automatique"),
            "le message doit dire qu'il n'y a pas de mode automatique : « {message} »"
        );
        assert!(
            message.contains("nzxtsmart2"),
            "le message doit nommer le contrôleur fautif : « {message} »"
        );
        assert!(
            !minuscules.contains("os error"),
            "un errno nu n'explique rien à l'utilisateur : « {message} »"
        );
    }

    #[test]
    fn le_refus_ne_depend_pas_du_mode_deja_ecrit() {
        // Arbitrage de ce fichier (en-tête, point 3). Un canal `nzxtsmart2`
        // qu'on trouverait déjà à `2` ne doit pas faire répondre « d'accord » :
        // le refus dépend du matériel, pas de l'état courant. Sans ce test, une
        // implémentation qui court-circuiterait « la valeur est déjà la bonne »
        // rendrait le refus intermittent — le pire des deux mondes.
        let sysfs = FauxSysfs::neuf("refus_meme_deja_a_deux");
        let nzxt = sysfs.hwmon("hwmon0", "nzxtsmart2");
        canal_complet(&nzxt, 1, "FAN 1", "2");

        let canaux = sysfs.canaux();
        let c = canal(&canaux, "nzxtsmart2:fan-1");

        let avant = photographie(sysfs.racine());
        assert!(
            set_mode(c, Mode::HostCurve).is_err(),
            "« auto » reste refusé, même si le fichier porte déjà « 2 »"
        );
        let apres = photographie(sysfs.racine());
        assert!(ecarts(&avant, &apres).is_empty());
    }
}

// ---------------------------------------------------------------------------
// 3 — l'automatique écrit exactement 2
// ---------------------------------------------------------------------------

mod ecriture {
    use super::{
        CANAUX_DU_KRAKEN, arborescence_de_reference, canal, ecarts, lire, mode, photographie,
    };
    use reverb_hw::hwmon::{Mode, set_mode};

    #[test]
    fn l_automatique_ecrit_exactement_deux_sur_un_canal_qui_sait() {
        // issue #50, critère d'acceptation — « `fan kraken2023elite:fan-speed
        // auto` écrit `2` et répond sans erreur », et approche technique —
        // « `Consigne::Auto` vise le mode `2` ».
        //
        // `nzxt-kraken3`, `kraken3_write` :
        //     case 0:
        //         /* Set channel to 100%, direct duty value */
        //         ret = kraken3_write_fixed_duty(priv, 255, channel);
        //
        // Écrire `0` au lieu de `2`, c'est envoyer la pompe à fond en silence.
        // Écrire `1`, c'est la figer sur la dernière consigne. Ni l'un ni
        // l'autre ne se voit dans un message : d'où la comparaison au texte
        // exact du fichier.
        let sysfs = arborescence_de_reference("auto_ecrit_deux");
        let canaux = sysfs.canaux();

        for nom in CANAUX_DU_KRAKEN {
            let c = canal(&canaux, nom);
            let enable = c
                .enable
                .as_ref()
                .expect("le Kraken expose son `pwmN_enable`");
            assert_eq!(lire(enable), "0", "{nom} part de « 0 »");

            set_mode(c, Mode::HostCurve).expect("« auto » réussit sur un canal qui sait");
            assert_eq!(lire(enable), "2", "{nom} : « auto » écrit exactement « 2 »");
        }
    }

    #[test]
    fn l_automatique_se_relit_comme_la_courbe_du_peripherique() {
        // Le pendant du test précédent, vu par la lecture : ce que `reverb fans`
        // affichera après un « auto » réussi. Une écriture correcte relue en
        // autre chose serait une colonne MODE fausse.
        let sysfs = arborescence_de_reference("auto_relu");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "kraken2023elite:fan-speed");

        set_mode(c, Mode::HostCurve).expect("« auto » réussit sur un canal qui sait");
        let relu = mode(c);
        assert!(
            matches!(relu, Mode::HostCurve),
            "après « auto », le canal suit la courbe du périphérique, lu : {relu:?}"
        );
    }

    #[test]
    fn l_automatique_n_ecrit_que_dans_le_fichier_enable_du_canal() {
        // Convention du dépôt (#7, approche technique) — « l'écriture ne touche
        // jamais un fichier que la découverte n'a pas listé ». Un « auto » ne
        // doit pas effacer la consigne au passage, ni toucher l'autre canal du
        // même Kraken : la pompe et le ventilateur se règlent séparément.
        let sysfs = arborescence_de_reference("auto_confine");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "kraken2023elite:pump-speed");

        let avant = photographie(sysfs.racine());
        set_mode(c, Mode::HostCurve).expect("« auto » réussit sur un canal qui sait");
        let apres = photographie(sysfs.racine());

        // `hwmon6` est le répertoire de `kraken2023elite` ; sysfs n'existe que
        // sous Linux, le séparateur est `/`.
        assert_eq!(
            ecarts(&avant, &apres),
            vec!["hwmon6/pwm1_enable".to_owned()]
        );
        assert_eq!(lire(&c.pwm), "171", "la consigne n'est pas touchée");
        assert_eq!(
            lire(
                canal(&canaux, "kraken2023elite:fan-speed")
                    .enable
                    .as_ref()
                    .expect("l'autre canal expose son `pwm2_enable`")
            ),
            "0",
            "l'autre canal du même Kraken n'a pas bougé"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — « 0 » n'est plus « laissé au firmware »
// ---------------------------------------------------------------------------

mod libelle_de_zero {
    use super::{arborescence_de_reference, canal, mode};
    use reverb_hw::hwmon::Mode;

    #[test]
    fn la_valeur_zero_se_lit_en_non_pilote() {
        // ⚠️ **Ce test portait la conclusion inverse jusqu'au 2026-08-15**, et
        // il est modifié parce que sa PRÉMISSE était fausse — jamais pour le
        // faire passer. Le précédent de procédure est celui de #50 lui-même,
        // qui avait déjà retourné un test de #9 pour la même raison.
        //
        // issue #50 établissait « `0` = 100 % et on lâche la barre », ce qui est
        // exact **de l'écriture** : `kraken3_write_pwm_enable(0)` appelle
        // `kraken3_write_fixed_duty(priv, 255, channel)`. Ce fichier en avait
        // conclu que la LECTURE de `0` disait la même chose. Elle ne le dit pas.
        //
        // `nzxt-kraken3` n'écrit rien au probe — son initialisation n'envoie que
        // `set_interval` et `finish_init`. Le champ `mode` sort du `kzalloc` à
        // `0`, et un `0` lu sur un canal jamais touché signifie seulement que
        // personne côté hôte ne le pilote.
        //
        // Constaté sur SHYNAEL le 2026-08-15 : `pwm1_enable = 0`, `pwm1 = 77`,
        // pompe à 1357 tr/min, duty qui suit le liquide par paliers — 89, 102,
        // 115, 128, 153. La colonne MODE annonçait « plein-régime-100% » (#101).
        let sysfs = arborescence_de_reference("zero_plein_regime");
        let canaux = sysfs.canaux();

        for nom in ["kraken2023elite:pump-speed", "kraken2023elite:fan-speed"] {
            let m = mode(canal(&canaux, nom));
            assert!(
                matches!(m, Mode::NonPilote),
                "{nom} est à « 0 » : le pilote ne le pilote pas, lu : {m:?}"
            );
        }
    }

    #[test]
    fn le_libelle_de_zero_dit_cent_pour_cent_et_ne_dit_plus_firmware() {
        // issue #50, critère d'acceptation — « La colonne MODE de `reverb fans`
        // ne contient plus "laissé au firmware" pour `0` », et comportement
        // attendu — « Il est nommé pour ce qu'il fait sur le Kraken — 100 %
        // sans régulation ».
        //
        // Le libellé actuel ment deux fois : il promet une courbe, et il promet
        // le firmware. Un utilisateur qui lit « laissé au firmware » devant une
        // pompe à 100 % n'a aucune raison de s'inquiéter.
        let libelle = Mode::PleinRegime.to_string();
        let minuscules = libelle.to_lowercase();

        assert!(
            libelle.contains("100"),
            "le libellé doit dire ce que « 0 » fait — 100 % : « {libelle} »"
        );
        assert!(
            !minuscules.contains("firmware"),
            "« 0 » n'est pas laissé au firmware : « {libelle} »"
        );
        assert!(
            !minuscules.contains("auto"),
            "« 0 » n'est pas un mode automatique : « {libelle} »"
        );
        assert!(
            !minuscules.contains("courbe"),
            "« 0 » n'exécute aucune courbe : « {libelle} »"
        );
    }

    #[test]
    fn les_six_libelles_de_mode_restent_distincts_et_non_vides() {
        // La colonne MODE se lit d'un coup d'œil : deux modes qui s'écrivent
        // pareil ne se distinguent plus. C'est ce qui rend le renommage utile
        // plutôt que cosmétique — `PleinRegime` et `HostCurve` doivent
        // désormais se lire différemment, alors que « laissé au firmware » et
        // « courbe de l'hôte » se confondaient à l'usage.
        //
        // ⚠️ **Six depuis #101**, et les deux nouvelles voisines sont justement
        // celles qu'il ne faut pas confondre : `NonPilote` est ce qu'un `0` lu
        // établit, `PleinRegime` ce qu'un `0` écrit provoque. Le fichier sysfs
        // ne les distingue pas ; cette colonne, si.
        let libelles: Vec<String> = [
            Mode::Manual,
            Mode::NonPilote,
            Mode::PleinRegime,
            Mode::HostCurve,
            Mode::Unknown(3),
            Mode::Unsupported,
        ]
        .iter()
        .map(|m| m.to_string())
        .collect();

        for libelle in &libelles {
            assert!(!libelle.trim().is_empty(), "un mode sans libellé");
        }
        for (i, un) in libelles.iter().enumerate() {
            for (j, autre) in libelles.iter().enumerate() {
                if i != j {
                    assert_ne!(
                        un, autre,
                        "deux modes ne peuvent pas porter le même libellé"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — un mode inconnu se lit, et ne se réécrit pas
// ---------------------------------------------------------------------------

mod inconnu {
    use super::{FauxSysfs, canal, canal_complet, ecarts, mode, photographie};
    use reverb_hw::hwmon::{Mode, set_mode};

    #[test]
    fn une_valeur_de_mode_inconnue_se_lit_telle_quelle() {
        // issue #50, dernier critère d'acceptation — « Un mode lu et inconnu
        // continue de se lire sans se réécrire (comportement actuel, à ne pas
        // casser) ». Le renommage de `0` et l'arrivée de « sait faire auto » ne
        // doivent pas transformer une valeur inconnue en `PleinRegime` par
        // défaut : `unwrap_or` est plus court à écrire que le cas inconnu.
        let sysfs = FauxSysfs::neuf("lecture_inconnue");
        let kraken = sysfs.hwmon("hwmon0", "kraken2023elite");
        let nzxt = sysfs.hwmon("hwmon1", "nzxtsmart2");
        for (n, valeur) in [(1u32, "3"), (2, "7"), (3, "255")] {
            canal_complet(&kraken, n, &format!("Fan {n}"), valeur);
            canal_complet(&nzxt, n, &format!("FAN {n}"), valeur);
        }

        let canaux = sysfs.canaux();
        for (n, valeur) in [(1u32, 3u8), (2, 7), (3, 255)] {
            for prefixe in ["kraken2023elite:fan", "nzxtsmart2:fan"] {
                let nom = format!("{prefixe}-{n}");
                let m = mode(canal(&canaux, &nom));
                assert!(
                    matches!(m, Mode::Unknown(v) if v == valeur),
                    "{nom} porte « {valeur} », qui se lit `Unknown({valeur})`, lu : {m:?}"
                );
            }
        }
    }

    #[test]
    fn une_valeur_de_mode_inconnue_ne_se_reecrit_pas_meme_sur_un_canal_qui_sait_faire_auto() {
        // Même critère, côté écriture. « Sait faire auto » autorise `2`, pas
        // n'importe quoi : un canal du Kraken doit refuser `Unknown` comme les
        // autres. CLAUDE.md — « Ne jamais inventer une trame absente des specs.
        // Si c'est inconnu, le dire. »
        let sysfs = FauxSysfs::neuf("ecriture_inconnue");
        let kraken = sysfs.hwmon("hwmon0", "kraken2023elite");
        canal_complet(&kraken, 1, "Pump speed", "0");
        let nzxt = sysfs.hwmon("hwmon1", "nzxtsmart2");
        canal_complet(&nzxt, 1, "FAN 1", "1");

        let canaux = sysfs.canaux();
        let sait = canal(&canaux, "kraken2023elite:pump-speed");
        let ne_sait_pas = canal(&canaux, "nzxtsmart2:fan-1");
        assert!(sait.sait_faire_auto());
        assert!(!ne_sait_pas.sait_faire_auto());

        let avant = photographie(sysfs.racine());
        for valeur in [3u8, 4, 7, 255] {
            for c in [sait, ne_sait_pas] {
                assert!(
                    set_mode(c, Mode::Unknown(valeur)).is_err(),
                    "{} : `Mode::Unknown({valeur})` n'est pas réémis",
                    c.name
                );
            }
        }
        let apres = photographie(sysfs.racine());

        assert!(
            ecarts(&avant, &apres).is_empty(),
            "un refus n'écrit nulle part : {:?}",
            ecarts(&avant, &apres)
        );
    }
}
