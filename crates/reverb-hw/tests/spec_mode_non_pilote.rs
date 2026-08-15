//! Tests d'intention du mode « non piloté » (issue #101).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #101 seule. Rien n'est relu
//! depuis `crates/reverb-hw/src/` : ni le corps de `FanChannel::mode`, ni le
//! contenu de l'énumération `Mode`. Ils encodent ce que le logiciel doit faire,
//! pas ce que le code fait — si l'un d'eux échoue après implémentation, c'est le
//! code qu'on corrige, jamais le test.
//!
//! Ils prolongent `spec_fans.rs` (#7) et `spec_auto.rs` (#50). Voir plus bas ce
//! que #101 leur retire.
//!
//! # Le fait mesuré
//!
//! issue #101, contexte — au démarrage, `status` annonce les deux canaux du
//! Kraken en `plein-régime-100%` alors qu'ils sont à 30 % et que la pompe tourne
//! à 1357 tr/min :
//!
//! ```text
//! chan kraken2023elite:pump-speed - 1357 30 plein-régime-100% oui
//! chan kraken2023elite:fan-speed  -  714 30 plein-régime-100% oui
//! ```
//!
//! La cause est nommée par l'issue : `nzxt-kraken3` n'écrit **rien** au probe —
//! son initialisation n'envoie que `set_interval` et `finish_init` —, le champ
//! `mode` sort donc du `kzalloc` à `0`, et **lire** `0` ne veut pas dire ce
//! qu'**écrire** `0` fait. L'écriture envoie à 100 % et lâche la barre (#50,
//! `kraken3_write_fixed_duty(priv, 255, channel)`) ; la lecture dit seulement
//! « le pilote n'a jamais touché ce canal ».
//!
//! C'est le contraire de la vérité sur l'organe le plus critique de la machine :
//! qui vérifie « est-ce que ma pompe tourne assez ? » lit « plein-régime-100% »
//! devant une pompe au minimum.
//!
//! # Ce que ce fichier fige, et ce qu'il laisse ouvert
//!
//! L'issue laisse explicitement le choix de la forme — « se lit dans un variant
//! distinct de celui qui s'écrit `0`… ou, si le même variant est conservé, son
//! libellé ne prétend pas 100 % ». **Ces tests ne tranchent donc pas la forme
//! interne** : ils ne nomment aucun variant nouveau, ne comparent jamais le mode
//! lu à `Mode::PleinRegime`, et portent tous sur du **comportement observable** —
//! le libellé rendu, l'absence d'espace dedans, l'octet écrit dans
//! `pwmN_enable`. Ils restent vrais quelle que soit l'option retenue.
//!
//! Ce qu'ils exigent :
//!
//! 1. Un `pwmN_enable` valant `0`, **lu**, ne prétend pas le plein régime.
//! 2. Son libellé tient en **un seul jeton** — l'arité de la ligne `chan` en
//!    dépend, le mode n'y étant plus le dernier champ depuis #50.
//! 3. Ce mode reste **écrivable**, et son écriture produit toujours `0`.
//! 4. `1`, `2`, une valeur inconnue et l'absence de fichier sont **inchangées**.
//!
//! # Ce que #101 retire à #50 et #7, et qui n'est pas de ce fichier
//!
//! Trois tests déjà écrits figent la lecture de `0` telle que l'issue la
//! corrige, et l'un d'eux fige aussi le libellé :
//!
//! - `spec_fans.rs::enable_a_zero_est_la_courbe_firmware`
//! - `spec_fans.rs::le_mode_ne_depend_que_du_fichier_enable_du_canal`
//! - `spec_auto.rs::la_valeur_zero_se_lit_en_plein_regime`
//! - `spec_auto.rs::le_libelle_de_zero_dit_cent_pour_cent_et_ne_dit_plus_firmware`
//!
//! Ce fichier **n'y touche pas** : les arbitrer est un travail de la phase
//! d'implémentation, pas de celle-ci. Le précédent existe et il est nommé dans
//! `spec_fans.rs` — « Ce test attendait `Unknown(2)` avant cette mesure, et
//! c'était juste : on ne devine pas ce qu'on n'a pas observé. La spec a bougé,
//! le test la suit. L'ordre est tenu — spec, puis test, puis code. »
//!
//! # Aucun accès matériel
//!
//! Convention du dépôt, reprise de `spec_fans.rs` et `spec_sondes.rs` : les
//! seuls chemins touchés ici sont ceux d'une arborescence construite dans
//! `std::env::temp_dir()`, effacée à la fin de chaque test. **Rien n'est lu ni
//! écrit sous `/sys`.** `hwmon::discover_in` prend une racine précisément pour
//! cela.

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
            "reverb-spec-mode-non-pilote-{nom_du_test}-{}",
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

/// Écrit un fichier sysfs.
///
/// Le saut de ligne final n'est pas décoratif : le noyau en ajoute un à chaque
/// attribut. Une lecture qui ne le retire pas ne convertit plus « 0\n » en
/// nombre, et le mode se lirait « inconnu » partout.
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
/// Sert à vérifier qu'une écriture n'a touché que le fichier visé. Comparer deux
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

/// Pose un canal complet : `pwmN`, `fanN_input`, `fanN_label`, et `pwmN_enable`
/// quand la source en expose un.
fn pose_canal(hwmon: &Path, n: u32, libelle: &str, pwm: &str, rpm: &str, enable: Option<&str>) {
    ecrire(&hwmon.join(format!("pwm{n}")), pwm);
    ecrire(&hwmon.join(format!("fan{n}_input")), rpm);
    ecrire(&hwmon.join(format!("fan{n}_label")), libelle);
    if let Some(valeur) = enable {
        ecrire(&hwmon.join(format!("pwm{n}_enable")), valeur);
    }
}

/// L'arborescence de référence : les cinq valeurs que `pwmN_enable` peut
/// prendre, plus son absence.
///
/// Le Kraken y est **exactement dans l'état de l'issue** : `pwm*_enable = 0`
/// alors que la pompe tourne à 1357 tr/min pour une consigne de 30 %. `77` sur
/// l'échelle du noyau vaut 30 % (77 × 100 / 255 = 30,19), soit la colonne PWM de
/// la ligne `chan` citée par l'issue. Ces régimes ne spécifient rien du
/// matériel : ils rendent la contradiction lisible dans le message d'échec.
///
/// Les quatre autres formes sont celles que #7 et #50 ont déjà figées, et que
/// #101 doit laisser intactes : `1` manuel, `2` courbe de l'hôte, une valeur
/// inconnue, et une source qui n'expose aucun `pwm*_enable`.
///
/// Les numéros de hwmon ne suivent pas l'ordre alphabétique des sources :
/// l'ordre de lecture du répertoire ne peut pas être confondu avec l'ordre
/// attendu.
fn arborescence_de_reference(nom_du_test: &str) -> FauxSysfs {
    let sysfs = FauxSysfs::neuf(nom_du_test);

    // issue #101, contexte — les deux canaux du Kraken au démarrage, jamais
    // touchés par le pilote.
    let kraken = sysfs.hwmon("hwmon6", "kraken2023elite");
    pose_canal(&kraken, 1, "Pump speed", "77", "1357", Some("0"));
    pose_canal(&kraken, 2, "Fan speed", "77", "714", Some("0"));

    // docs/VENTILATEURS.md, table du résumé — `nzxtsmart2`, trois canaux en
    // mode manuel. Ce sont eux qui vérifient que `1` ne bouge pas.
    let nzxt = sysfs.hwmon("hwmon4", "nzxtsmart2");
    for (n, libelle, rpm) in [
        (1, "FAN 1", "725"),
        (2, "FAN 2", "688"),
        (3, "FAN 3", "715"),
    ] {
        pose_canal(&nzxt, n, libelle, "64", rpm, Some("1"));
    }

    // `2`, la courbe exécutée par le firmware sur consigne de l'hôte (#50), et
    // une valeur que rien ne documente. Les deux se lisent, aucune des deux ne
    // doit devenir le mode de `0`.
    let autre = sysfs.hwmon("hwmon9", "amdgpu");
    pose_canal(&autre, 1, "Courbe hote", "128", "1100", Some("2"));
    pose_canal(&autre, 2, "Valeur inconnue", "100", "900", Some("7"));

    // docs/VENTILATEURS.md — `nct6687` « n'expose **aucun `pwm*_enable`** ».
    let nct = sysfs.hwmon("hwmon2", "nct6687");
    pose_canal(&nct, 1, "CPU FAN", "153", "0", None);

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

/// Le libellé du mode d'un canal, tel que la ligne `chan` le porte.
fn libelle(canal: &FanChannel) -> String {
    mode(canal).to_string()
}

/// Les deux canaux du Kraken de l'arborescence de référence, tous deux à `0`.
const CANAUX_A_ZERO: [&str; 2] = ["kraken2023elite:pump-speed", "kraken2023elite:fan-speed"];

/// Ce que le libellé d'un `0` **lu** ne doit pas promettre.
///
/// issue #101, comportement attendu — « `0` s'affiche comme un état
/// **indéterminé du point de vue de l'hôte** », et contexte — « La fenêtre dit
/// donc que la pompe est à fond au moment précis où elle est au minimum ».
///
/// La liste vise la seule chose que l'issue interdit : **prétendre un régime**.
/// Elle ne dit rien de la formulation retenue — `non-piloté` est une suggestion
/// de l'issue, pas une exigence.
const CE_QUE_ZERO_LU_NE_PROMET_PAS: [&str; 4] = ["100", "plein", "max", "fond"];

/// Vérifie qu'un libellé de mode tient en un seul jeton.
///
/// issue #101, critère d'acceptation — « Son libellé tient en **un seul jeton**,
/// sans espace — l'arité de la ligne `chan` en dépend », et approche technique —
/// « Le libellé traverse le socket dans le champ `mode` d'une ligne `chan`, où
/// il n'est plus le dernier depuis #50. Un jeton à espaces rendrait l'arité de
/// la ligne indécidable. »
fn un_seul_jeton(libelle: &str, quoi: &str) {
    assert!(
        !libelle.trim().is_empty(),
        "{quoi} : un mode sans libellé ne laisse rien à afficher"
    );
    assert!(
        !libelle.chars().any(char::is_whitespace),
        "{quoi} : « {libelle} » porte une espace — la ligne `chan` gagnerait un \
         jeton, et son arité deviendrait indécidable"
    );
    assert!(
        !libelle.chars().any(char::is_control),
        "{quoi} : « {libelle} » porte un caractère de contrôle — un saut de \
         ligne couperait la réponse en deux"
    );
}

// ---------------------------------------------------------------------------
// 1 — un « 0 » lu ne prétend pas le plein régime
// ---------------------------------------------------------------------------

mod lecture_de_zero {
    use super::{
        CANAUX_A_ZERO, CE_QUE_ZERO_LU_NE_PROMET_PAS, arborescence_de_reference, canal, libelle,
        lire, mode, un_seul_jeton,
    };
    use reverb_hw::hwmon::Mode;

    #[test]
    fn un_enable_a_zero_lu_ne_pretend_pas_le_plein_regime() {
        // issue #101, comportement attendu — « Lu sur un canal, `0` s'affiche
        // comme un état **indéterminé du point de vue de l'hôte** : le pilote ne
        // pilote pas, la consigne visible est celle que le périphérique
        // s'applique. »
        //
        // C'est le cœur de l'issue, et le seul test de ce fichier qui la
        // reproduise telle quelle : les deux canaux sont à `0` avec une consigne
        // de 30 % et une pompe à 1357 tr/min. Un libellé qui annonce 100 % ment
        // sur l'organe le plus critique de la machine, et il ment dans le sens
        // rassurant.
        //
        // Le test ne dit **pas** quel libellé porter, ni si le variant change :
        // il interdit seulement de promettre un régime que rien ne mesure.
        let sysfs = arborescence_de_reference("zero_ne_pretend_pas_cent");
        let canaux = sysfs.canaux();

        for nom in CANAUX_A_ZERO {
            let c = canal(&canaux, nom);
            let enable = c
                .enable
                .as_ref()
                .expect("le Kraken expose son `pwmN_enable`");
            assert_eq!(lire(enable), "0", "{nom} part bien de « 0 »");

            let libelle = libelle(c);
            let minuscules = libelle.to_lowercase();
            for promesse in CE_QUE_ZERO_LU_NE_PROMET_PAS {
                assert!(
                    !minuscules.contains(promesse),
                    "{nom} : « {libelle} » promet « {promesse} » pour un canal que \
                     le pilote n'a jamais touché — la pompe y tourne à {} tr/min \
                     pour une consigne de 30 %",
                    lire(c.tach.as_ref().expect("le Kraken expose son tachymètre"))
                );
            }
        }
    }

    #[test]
    fn le_libelle_du_zero_lu_tient_en_un_seul_jeton() {
        // issue #101, critère d'acceptation — « Son libellé tient en **un seul
        // jeton**, sans espace — l'arité de la ligne `chan` en dépend ».
        //
        // La suggestion de l'issue, `non-piloté`, en est un ; « non piloté » n'en
        // serait pas un. C'est la seule contrainte de forme que l'issue pose.
        let sysfs = arborescence_de_reference("zero_un_seul_jeton");
        let canaux = sysfs.canaux();

        for nom in CANAUX_A_ZERO {
            un_seul_jeton(&libelle(canal(&canaux, nom)), nom);
        }
    }

    #[test]
    fn le_zero_lu_ne_se_confond_avec_aucun_des_autres_modes() {
        // issue #101, comportement attendu — « Le `0` lu se distingue du `0`
        // écrit », et critère — « Un `pwmN_enable` valant `0` se lit comme un
        // mode **distinct** ».
        //
        // Le test compare aux quatre autres modes que #7 et #50 ont figés, et
        // **pas** à celui que `0` écrit produit : c'est précisément ce que
        // l'issue laisse ouvert. Ce qu'elle ne laisse pas ouvert, c'est qu'un
        // canal jamais touché se lise « manuel », « courbe de l'hôte », « valeur
        // inconnue » ou « non supporté » — chacun de ces quatre-là dit quelque
        // chose de faux, et deux modes qui s'écrivent pareil ne se distinguent
        // plus dans la colonne MODE.
        let sysfs = arborescence_de_reference("zero_distinct");
        let canaux = sysfs.canaux();
        let zero = libelle(canal(&canaux, CANAUX_A_ZERO[0]));

        for autre in [
            Mode::Manual,
            Mode::HostCurve,
            Mode::Unknown(7),
            Mode::Unsupported,
        ] {
            assert_ne!(
                zero,
                autre.to_string(),
                "un canal jamais touché ne se lit pas « {autre:?} »"
            );
        }
    }

    #[test]
    fn le_zero_lu_n_est_ni_une_valeur_incomprise_ni_une_absence_de_fichier() {
        // Suite du même critère, au niveau du variant cette fois — c'est le seul
        // endroit où ce fichier en parle, et il ne nomme que des variants qui
        // existent déjà.
        //
        // `Unknown(0)` et `Unsupported` sont les deux replis qu'une correction
        // pressée produirait, et tous deux cassent un autre critère de l'issue :
        // « Écrire ce mode reste possible et produit toujours `0` ». `Unknown`
        // n'est jamais réémis (#7, #50), et `Unsupported` ne porte aucune valeur
        // à écrire. `Unsupported` dirait de surcroît « la source n'expose pas de
        // `pwmN_enable' », alors que le Kraken en expose un et le remplit.
        let sysfs = arborescence_de_reference("zero_ni_inconnu_ni_absent");
        let canaux = sysfs.canaux();

        for nom in CANAUX_A_ZERO {
            let m = mode(canal(&canaux, nom));
            assert!(
                !matches!(m, Mode::Unknown(_)),
                "{nom} : « 0 » est une valeur documentée, pas une valeur \
                 incomprise, lu : {m:?}"
            );
            assert!(
                !matches!(m, Mode::Unsupported),
                "{nom} expose bien un `pwmN_enable`, lu : {m:?}"
            );
        }
    }

    #[test]
    fn la_lecture_du_mode_n_ecrit_rien() {
        // Garde-fou du dépôt, repris de `spec_fans.rs` et `spec_sondes.rs` :
        // lire un mode reste une lecture. #101 ne change que ce que `0`
        // **signifie** ; une correction qui « normaliserait » le fichier en
        // écrivant une autre valeur sortirait la pompe de son état sans que
        // personne ne l'ait demandé — exactement la panne que
        // `docs/VENTILATEURS.md` documente comme « irréversible sans coupure
        // d'alimentation complète ».
        let sysfs = arborescence_de_reference("lecture_seule");
        let avant = super::photographie(sysfs.racine());

        for c in &sysfs.canaux() {
            let _ = c.mode();
        }

        let apres = super::photographie(sysfs.racine());
        assert_eq!(
            super::ecarts(&avant, &apres),
            Vec::<String>::new(),
            "lire un mode ne doit toucher aucun fichier"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — ce mode reste écrivable, et son écriture produit « 0 »
// ---------------------------------------------------------------------------

mod ecriture_de_zero {
    use super::{FauxSysfs, canal, ecarts, libelle, lire, mode, photographie, pose_canal};
    use reverb_hw::hwmon::{CourbesPosees, set_mode};

    /// Deux canaux d'un même Kraken : le premier à `0`, le second à `1`.
    ///
    /// Le premier **donne** le mode — c'est la lecture que #101 corrige. Le
    /// second le **reçoit** : partir de `1` est ce qui rend l'écriture
    /// observable, un canal déjà à `0` ne prouverait rien.
    fn deux_canaux(nom_du_test: &str) -> FauxSysfs {
        let sysfs = FauxSysfs::neuf(nom_du_test);
        let kraken = sysfs.hwmon("hwmon6", "kraken2023elite");
        pose_canal(&kraken, 1, "Pump speed", "77", "1357", Some("0"));
        pose_canal(&kraken, 2, "Fan speed", "171", "714", Some("1"));
        sysfs
    }

    #[test]
    fn ecrire_le_mode_lu_depuis_zero_produit_zero() {
        // issue #101, critère d'acceptation — « Écrire ce mode reste possible et
        // produit toujours `0` », et comportement attendu — « L'écriture de `0`
        // garde son sens et son avertissement : elle envoie à 100 % et lâche la
        // barre. »
        //
        // Le mode écrit n'est pas nommé : il est **lu** sur le canal à `0`, puis
        // réécrit sur son voisin. Le test vaut donc que #101 ajoute un variant ou
        // qu'il se contente d'en relibeller un — dans les deux cas, ce que la
        // lecture rend doit pouvoir repartir vers le noyau, et y valoir `0`.
        //
        // Le canal visé est celui d'un Kraken : `nzxt-kraken3` est le seul pilote
        // dont l'issue documente ce que `0` fait à l'écriture.
        let sysfs = deux_canaux("ecriture_produit_zero");
        let canaux = sysfs.canaux();
        let source = canal(&canaux, "kraken2023elite:pump-speed");
        let cible = canal(&canaux, "kraken2023elite:fan-speed");
        let enable = cible
            .enable
            .as_ref()
            .expect("le Kraken expose `pwm2_enable`");

        assert_eq!(lire(enable), "1", "la cible part du mode manuel");
        set_mode(cible, mode(source), &CourbesPosees::vide())
            .expect("le mode d'un canal non piloté reste écrivable");
        assert_eq!(
            lire(enable),
            "0",
            "l'écriture de ce mode vaut toujours « 0 » dans `pwmN_enable`"
        );
    }

    #[test]
    fn le_mode_ecrit_se_relit_a_l_identique() {
        // Conséquence des deux critères pris ensemble — « se lit comme un mode
        // distinct » et « l'écriture produit toujours `0` » : l'aller-retour doit
        // fermer. Sans lui, la fenêtre montrerait un mode après l'avoir demandé
        // et un autre à la seconde suivante, sans qu'aucun des deux tests
        // précédents ne le voie.
        //
        // La comparaison porte sur le libellé rendu, pas sur le variant : c'est
        // ce que la ligne `chan` transporte, et c'est ce que la fenêtre affiche.
        let sysfs = deux_canaux("aller_retour");
        let canaux = sysfs.canaux();
        let source = canal(&canaux, "kraken2023elite:pump-speed");
        let cible = canal(&canaux, "kraken2023elite:fan-speed");

        let attendu = libelle(source);
        set_mode(cible, mode(source), &CourbesPosees::vide())
            .expect("le mode d'un canal non piloté reste écrivable");
        assert_eq!(
            libelle(cible),
            attendu,
            "un mode écrit puis relu doit être le même mode"
        );
    }

    #[test]
    fn cette_ecriture_ne_touche_que_le_fichier_enable_du_canal() {
        // Garde-fou de #7, approche technique — « l'écriture ne touche jamais un
        // fichier que la découverte n'a pas listé ». La consigne ne doit pas être
        // effacée au passage, et le canal voisin — celui d'où le mode a été lu —
        // ne doit pas bouger : c'est la pompe.
        let sysfs = deux_canaux("ecriture_confinee");
        let canaux = sysfs.canaux();
        let source = canal(&canaux, "kraken2023elite:pump-speed");
        let cible = canal(&canaux, "kraken2023elite:fan-speed");

        let avant = photographie(sysfs.racine());
        set_mode(cible, mode(source), &CourbesPosees::vide())
            .expect("le mode d'un canal non piloté reste écrivable");
        let apres = photographie(sysfs.racine());

        // `hwmon6` est le répertoire du Kraken ; sysfs n'existe que sous Linux,
        // le séparateur est `/`.
        assert_eq!(
            ecarts(&avant, &apres),
            vec!["hwmon6/pwm2_enable".to_owned()]
        );
        assert_eq!(lire(&cible.pwm), "171", "la consigne n'est pas touchée");
        assert_eq!(
            lire(
                source
                    .enable
                    .as_ref()
                    .expect("la pompe expose `pwm1_enable`")
            ),
            "0",
            "la pompe n'a pas bougé"
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — les autres valeurs sont inchangées
// ---------------------------------------------------------------------------

mod autres_valeurs {
    use super::{
        CANAUX_A_ZERO, arborescence_de_reference, canal, ecarts, libelle, mode, photographie,
        un_seul_jeton,
    };
    use reverb_hw::hwmon::{CourbesPosees, Mode, set_mode};

    #[test]
    fn un_enable_a_un_reste_le_mode_manuel() {
        // issue #101, critère d'acceptation — « Les autres valeurs — `1`, `2`,
        // inconnues, absentes — sont inchangées ».
        // #7, « État relevé sur la machine » — `nzxtsmart2 […] enable=1`, affiché
        // « manuel ».
        let sysfs = arborescence_de_reference("un_reste_manuel");
        let canaux = sysfs.canaux();

        for nom in ["nzxtsmart2:fan-1", "nzxtsmart2:fan-2", "nzxtsmart2:fan-3"] {
            let m = mode(canal(&canaux, nom));
            assert!(matches!(m, Mode::Manual), "{nom} est manuel, lu : {m:?}");
        }
    }

    #[test]
    fn un_enable_a_deux_reste_la_courbe_de_l_hote() {
        // Même critère. #50 — « Le vrai "rendre la main à la courbe" est `2`, et
        // seul le Kraken l'a. » Confondre `2` et le nouveau sens de `0` ferait
        // dire « le pilote ne pilote pas » d'un canal que le firmware régule
        // précisément sur la courbe qu'on lui a téléversée.
        let sysfs = arborescence_de_reference("deux_reste_courbe_hote");
        let canaux = sysfs.canaux();

        let m = mode(canal(&canaux, "amdgpu:courbe-hote"));
        assert!(
            matches!(m, Mode::HostCurve),
            "« 2 » reste la courbe de l'hôte, lu : {m:?}"
        );
    }

    #[test]
    fn une_valeur_inconnue_reste_inconnue_et_ne_se_reecrit_pas() {
        // Même critère, et #50, dernier critère d'acceptation — « Un mode lu et
        // inconnu continue de se lire sans se réécrire (comportement actuel, à ne
        // pas casser) ».
        // CLAUDE.md — « Ne jamais inventer une trame absente des specs. Si c'est
        // inconnu, le dire. »
        let sysfs = arborescence_de_reference("inconnu_reste_inconnu");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "amdgpu:valeur-inconnue");

        let m = mode(c);
        assert!(
            matches!(m, Mode::Unknown(7)),
            "« 7 » n'est documenté nulle part et se lit tel quel, lu : {m:?}"
        );

        let avant = photographie(sysfs.racine());
        assert!(
            set_mode(c, Mode::Unknown(7), &CourbesPosees::vide()).is_err(),
            "une valeur incomprise n'est pas réémise"
        );
        let apres = photographie(sysfs.racine());
        assert!(
            ecarts(&avant, &apres).is_empty(),
            "un refus n'écrit nulle part : {:?}",
            ecarts(&avant, &apres)
        );
    }

    #[test]
    fn un_canal_sans_fichier_enable_reste_non_supporte() {
        // Même critère, cas « absentes ». docs/VENTILATEURS.md — `nct6687`
        // « n'expose **aucun `pwm*_enable`** ». Le nouveau sens de `0` est « le
        // pilote n'a jamais touché ce canal » ; l'absence de fichier est « il n'y
        // a nulle part où l'écrire ». Les confondre laisserait croire qu'un mode
        // peut être posé sur un canal qui n'en a pas.
        let sysfs = arborescence_de_reference("absent_reste_non_supporte");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "nct6687:cpu-fan");

        assert!(c.enable.is_none(), "la source n'expose aucun `pwm*_enable`");
        let m = mode(c);
        assert!(
            matches!(m, Mode::Unsupported),
            "sans fichier de mode, le canal n'est pas piloté par un mode, lu : {m:?}"
        );
    }

    #[test]
    fn tous_les_libelles_de_mode_restent_distincts_et_tiennent_en_un_jeton() {
        // #50 — « La colonne MODE se lit d'un coup d'œil : deux modes qui
        // s'écrivent pareil ne se distinguent plus. » #101 ajoute un sens à la
        // liste ; il ne doit ni la doubler ni desserrer l'arité de la ligne
        // `chan`, qui vaut pour **tous** les modes et pas seulement pour le
        // nouveau.
        //
        // Les libellés sont pris là où ils comptent — sur des canaux découverts —
        // et non sur des variants nommés à la main : c'est la seule façon
        // d'inclure celui de `0` sans présumer de sa forme.
        let sysfs = arborescence_de_reference("libelles_distincts");
        let canaux = sysfs.canaux();

        let mut libelles: Vec<(&str, String)> = Vec::new();
        for nom in [
            CANAUX_A_ZERO[0],
            "nzxtsmart2:fan-1",
            "amdgpu:courbe-hote",
            "amdgpu:valeur-inconnue",
            "nct6687:cpu-fan",
        ] {
            libelles.push((nom, libelle(canal(&canaux, nom))));
        }

        for (nom, texte) in &libelles {
            un_seul_jeton(texte, nom);
        }
        for (i, (un_nom, un)) in libelles.iter().enumerate() {
            for (autre_nom, autre) in libelles.iter().skip(i + 1) {
                assert_ne!(
                    un, autre,
                    "{un_nom} et {autre_nom} portent le même libellé et ne se \
                     distinguent plus dans la colonne MODE"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — le libellé traverse la ligne `chan` sans en changer l'arité
// ---------------------------------------------------------------------------

mod ligne_chan {
    use super::{CANAUX_A_ZERO, arborescence_de_reference, canal, libelle};
    use reverb_proto::ipc::{ResponseLine, encode_response_line, parse_response_line};

    #[test]
    fn une_ligne_chan_portant_ce_mode_se_relit_intacte() {
        // issue #101, approche technique — « ⚠️ Le libellé traverse le socket
        // dans le champ `mode` d'une ligne `chan`, où il n'est plus le dernier
        // depuis #50. Un jeton à espaces rendrait l'arité de la ligne
        // indécidable. »
        //
        // C'est la conséquence que `un_seul_jeton` prévient, vérifiée là où elle
        // se produirait vraiment : la ligne est celle de l'issue — pompe muette
        // côté position, 1357 tr/min, 30 %, et un canal qui sait faire auto.
        //
        // #50 a figé la grammaire : `chan <canal> <position> <rpm> <pwm> <mode>
        // <oui|non>`, sept jetons, « le mode reste un jeton unique […] c'est ce
        // qui permet de distinguer six jetons de sept ». Un mode à espaces ferait
        // relire le dernier morceau du libellé comme le champ « sait faire
        // auto », ou ferait refuser la ligne entière.
        let sysfs = arborescence_de_reference("ligne_chan");
        let canaux = sysfs.canaux();

        for nom in CANAUX_A_ZERO {
            let ligne = ResponseLine::Channel {
                channel: nom.to_owned(),
                position: None,
                rpm: Some(1357),
                pwm: Some(30),
                mode: libelle(canal(&canaux, nom)),
                sait_faire_auto: true,
            };

            let encodee = encode_response_line(&ligne);
            assert_eq!(
                encodee.split_whitespace().count(),
                7,
                "la ligne « {encodee} » doit garder ses sept jetons"
            );
            assert_eq!(
                parse_response_line(&encodee),
                Ok(ligne),
                "la ligne « {encodee} » doit revenir intacte"
            );
        }
    }
}
