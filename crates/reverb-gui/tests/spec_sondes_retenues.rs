//! Tests d'intention du tri des sondes affichées (issue #51).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #51 seule. Rien n'a été
//! relu de `crates/reverb-gui/src/` sinon les signatures publiques déjà en
//! place — l'API testée ici n'existe pas encore, et ce fichier est ce qui la
//! décrit.
//!
//! Ces tests encodent ce que la fenêtre doit **montrer**, pas ce que le code
//! fait. Si l'un d'eux échoue après implémentation, c'est le code qu'on
//! corrige, jamais le test.
//!
//! # Le contrat proposé
//!
//! Dans `crates/reverb-gui/src/sondes.rs`, à côté de `Historique` et `Releve`,
//! qu'il ne touche pas :
//!
//! ```ignore
//! /// Une sonde que le panneau montre : son `slug` et son libellé lisible.
//! pub struct SondeRetenue { pub slug: String, pub libelle: String }
//!
//! /// Ce que la fenêtre a lu une fois dans `/sys/class/nvme/nvmeN/model`.
//! pub struct ModelesNvme { pub nvme0: Option<String>, pub nvme1: Option<String> }
//!
//! /// La table de correspondance : un `slug` entre, un libellé sort — ou rien.
//! pub fn libelle_retenu(slug: &str, modeles: &ModelesNvme) -> Option<String>;
//!
//! /// Les sondes à montrer, dans l'ordre du panneau, sans doublon.
//! pub fn sondes_retenues(slugs: &[String], modeles: &ModelesNvme) -> Vec<SondeRetenue>;
//! ```
//!
//! `SondeRetenue` porte `Debug`, `Clone`, `PartialEq` et `Eq` ; `ModelesNvme`
//! porte `Debug`, `Clone` et `Default` — `default()` étant le cas des deux
//! modèles illisibles, qui est celui d'une machine où `sysfs` n'a pas répondu.
//!
//! **Les deux fonctions sont pures.** Le modèle du disque est un *paramètre* :
//! s'il était lu dans la fonction, ce fichier ne testerait plus une table de
//! correspondance mais la machine sur laquelle il tourne. La lecture de `sysfs`
//! vit ailleurs, dans la fenêtre, au démarrage (issue #51, approche technique —
//! « lu une fois au démarrage […] c'est un fichier de `sysfs` en lecture seule,
//! pas un périphérique, donc l'ADR-002 n'est pas touché »).
//!
//! # Ce qu'un panneau de sondes ne doit pas faire dire
//!
//! Le point de départ de l'issue est une remarque, et elle est de lisibilité :
//! « je ne pensais pas qu'il y en avait autant et je ne sais même pas vraiment
//! où elles sont placées ». Seize cartes dont quatre hubs SPD et deux puces
//! réseau ne se lisent plus — mais trois façons de rater le tri produisent un
//! panneau qui *paraît* bon :
//!
//! 1. **Montrer la mauvaise puce sous le bon nom.** `amdgpu:edge` est le GPU
//!    intégré du processeur, pas la carte graphique ; affiché comme « GPU », il
//!    donne une température plausible et fausse — celle qui ne bouge pas quand
//!    le jeu chauffe.
//! 2. **Rendre deux disques indiscernables.** Deux cartes « NVMe » identiques
//!    ne disent pas laquelle chauffe, et l'issue exige de les distinguer « sans
//!    lire un numéro de `hwmon` ».
//! 3. **Laisser passer un `slug` voisin.** `nvme:sensor-2` est un capteur
//!    interne de SSD, `k10temp:tccd1` la température d'un die parmi d'autres :
//!    aucun des deux n'est faux en soi, mais tous deux repeuplent le panneau
//!    que l'issue vide.
//!
//! # Ce que ces tests ne vérifient pas
//!
//! - **La découverte côté démon**, qui ne change pas (issue #51, hors scope) :
//!   `status` rend toujours ses seize lignes `temp`, et le menu de sondes du
//!   cadran continue de les proposer toutes. Ce fichier ne parle que de ce que
//!   la **fenêtre** choisit de montrer.
//! - **La lecture de `/sys/class/nvme/nvmeN/model`.** Elle dépend de la
//!   machine ; ici le modèle est passé en paramètre, y compris absent.
//! - **Le dessin.** La taille des cartes, leur disposition, leur couleur se
//!   regardent. Ce qui se vérifie sans écran, c'est *quelles* sondes sortent,
//!   *dans quel ordre*, et *sous quel nom*.
//!
//! # Arbitrages de contrat
//!
//! - **Le libellé exact n'est pas figé, sa famille l'est.** Exiger la chaîne
//!   « CPU » au caractère près ferait échouer « CPU (Tctl) » ou « CPU ⌁ », qui
//!   remplissent la demande. Ce que ces tests exigent : non vide, ne contenant
//!   pas le `slug` brut ni « hwmon » (issue — « nommées en français et sans
//!   préfixe `hwmon` »), nommant sa famille (`cpu`, `liquide`, `gpu`, `nvme`,
//!   sans égard à la casse), et distinct des quatre autres.
//! - **Aucun rang n'est exposé.** L'issue parle d'« un rang d'affichage » dans
//!   son approche technique ; l'ordre observable est celui du `Vec` rendu, et
//!   un entier public en plus se testerait deux fois pour la même promesse. La
//!   forme du rang reste libre à l'implémentation.
//! - **Deux disques de même modèle restent distinguables.** L'issue n'envisage
//!   que deux modèles différents, mais deux SSD identiques dans une machine est
//!   le cas courant, et le critère « distinguables l'un de l'autre » ne
//!   souffre pas d'exception. Le repli en est le même que celui du modèle
//!   illisible : le numéro du disque, `nvme0` / `nvme1`, qui n'est pas un
//!   numéro de `hwmon`.
//! - **La table nomme deux disques, pas *n*.** `nvme:nvme2:composite` ne rend
//!   rien : `ModelesNvme` n'a que deux emplacements, et « rendre la liste
//!   configurable » est explicitement hors scope de l'issue.

use reverb_gui::sondes::{ModelesNvme, SondeRetenue, libelle_retenu, sondes_retenues};

// ---------------------------------------------------------------------------
// Les sondes de la machine, relevées sur SHYNAEL (issue #51, contexte)
// ---------------------------------------------------------------------------

/// Le CPU — « celle que les ventilateurs suivent ».
const CPU: &str = "k10temp:tctl";
/// Le liquide du Kraken.
const LIQUIDE: &str = "kraken2023elite:coolant-temp";
/// La carte graphique — **pas** le GPU intégré `amdgpu:edge`.
const GPU: &str = "nvidia:NVIDIA_GeForce_RTX_5070";
/// Le premier disque NVMe.
const NVME0: &str = "nvme:nvme0:composite";
/// Le second disque NVMe.
const NVME1: &str = "nvme:nvme1:composite";

/// Les cinq `slug` retenus, **dans l'ordre où le panneau doit les montrer** :
/// CPU, liquide, GPU, NVMe (issue #51, tableau du comportement attendu).
const ATTENDUS: [&str; 5] = [CPU, LIQUIDE, GPU, NVME0, NVME1];

/// Le mot que chaque libellé doit porter, en minuscules.
///
/// C'est la colonne « affiché » du tableau de l'issue, ramenée à ce qui se
/// vérifie sans figer une typographie.
const FAMILLES: [(&str, &str); 5] = [
    (CPU, "cpu"),
    (LIQUIDE, "liquide"),
    (GPU, "gpu"),
    (NVME0, "nvme"),
    (NVME1, "nvme"),
];

/// Les onze sondes que le panneau ne doit plus montrer.
///
/// Les quatre hubs SPD des barrettes, la puce Wi-Fi, la puce Ethernet, trois
/// capteurs internes de SSD, le GPU intégré et le second die du CPU (issue #51,
/// critère — « Aucune sonde `spd5118`, `mt7921`, `r8169`, `amdgpu` ni
/// `nvme:sensor-*` n'y figure »).
const ECARTEES: [&str; 11] = [
    "amdgpu:edge",
    "k10temp:tccd1",
    "mt7921_phy0:temp1",
    "nvme:sensor-1",
    "nvme:sensor-2",
    "nvme:sensor-8",
    "r8169_0_e00:00:temp1",
    "spd5118:8-0050:temp1",
    "spd5118:8-0051:temp1",
    "spd5118:8-0052:temp1",
    "spd5118:8-0053:temp1",
];

/// Les seize sondes de SHYNAEL, dans l'ordre où la fenêtre les reçoit —
/// l'ordre alphabétique de `Historique::sondes()`.
///
/// ⚠️ Cet ordre place déjà les cinq retenues dans le bon ordre : une liste
/// bâtie là-dessus ne prouve donc **pas** que le tri a lieu. C'est le rôle des
/// permutations de `DESORDRES`.
const TOUTES: [&str; 16] = [
    "amdgpu:edge",
    "k10temp:tccd1",
    CPU,
    LIQUIDE,
    "mt7921_phy0:temp1",
    GPU,
    NVME0,
    NVME1,
    "nvme:sensor-1",
    "nvme:sensor-2",
    "nvme:sensor-8",
    "r8169_0_e00:00:temp1",
    "spd5118:8-0050:temp1",
    "spd5118:8-0051:temp1",
    "spd5118:8-0052:temp1",
    "spd5118:8-0053:temp1",
];

/// Des ordres d'arrivée qui ne sont pas celui du panneau.
///
/// Le premier est l'ordre exactement inverse ; les trois autres brassent les
/// deux NVMe entre les autres familles, pour qu'aucune ne puisse arriver par
/// accident à sa place.
const DESORDRES: [[&str; 5]; 4] = [
    [NVME1, NVME0, GPU, LIQUIDE, CPU],
    [GPU, CPU, NVME1, LIQUIDE, NVME0],
    [LIQUIDE, NVME0, CPU, NVME1, GPU],
    [NVME0, GPU, LIQUIDE, NVME1, CPU],
];

/// Le modèle du premier disque de SHYNAEL.
const MODELE_0: &str = "CT2000T705SSD5";
/// Le modèle du second — différent du premier, c'est tout ce qui compte.
const MODELE_1: &str = "CT4000P3SSD8";

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

/// Les modèles lisibles, tels que `sysfs` les rend sur cette machine.
fn modeles() -> ModelesNvme {
    ModelesNvme {
        nvme0: Some(MODELE_0.to_owned()),
        nvme1: Some(MODELE_1.to_owned()),
    }
}

/// Les modèles illisibles — `sysfs` muet, ou un disque qui n'expose rien.
fn sans_modele() -> ModelesNvme {
    ModelesNvme::default()
}

/// La liste de `slug` telle que la fenêtre la tient : des `String`.
fn liste(slugs: &[&str]) -> Vec<String> {
    slugs.iter().map(|s| (*s).to_owned()).collect()
}

/// Ce que le panneau montre pour cette entrée.
fn panneau(slugs: &[&str], modeles: &ModelesNvme) -> Vec<SondeRetenue> {
    sondes_retenues(&liste(slugs), modeles)
}

/// Les `slug` rendus, dans l'ordre où ils sortent.
fn slugs_rendus(sondes: &[SondeRetenue]) -> Vec<String> {
    sondes.iter().map(|s| s.slug.clone()).collect()
}

/// Le libellé d'une sonde qu'on sait retenue, ou un échec explicite.
fn libelle(slug: &str, modeles: &ModelesNvme) -> String {
    libelle_retenu(slug, modeles).unwrap_or_else(|| panic!("{slug} doit être retenue"))
}

// ---------------------------------------------------------------------------

mod retenues {
    use super::{
        ATTENDUS, FAMILLES, ModelesNvme, libelle, libelle_retenu, modeles, panneau, sans_modele,
        slugs_rendus,
    };

    #[test]
    fn chacune_des_cinq_sondes_retenues_rend_un_libelle_lisible() {
        // issue #51, test d'intention n° 1 — « Chacun des quatre `slug`
        // attendus rend un libellé non vide » ; comportement attendu — « nommées
        // en français et sans préfixe `hwmon` ».
        // Quatre familles font cinq `slug`, les deux disques ayant chacun leur
        // carte. Un libellé vide ou égal au `slug` laisserait le panneau aussi
        // illisible qu'avant le tri, sans qu'aucune erreur ne le signale.
        for &slug in &ATTENDUS {
            for modeles in [modeles(), sans_modele()] {
                let libelle = libelle(slug, &modeles);
                assert!(
                    !libelle.trim().is_empty(),
                    "{slug} rend un libellé vide (modèles : {modeles:?})"
                );
                assert!(
                    !libelle.contains(slug),
                    "{slug} se montre brut dans son libellé « {libelle} »"
                );
                assert!(
                    !libelle.to_lowercase().contains("hwmon"),
                    "{slug} garde un préfixe hwmon dans « {libelle} »"
                );
            }
        }
    }

    #[test]
    fn chaque_libelle_nomme_sa_famille_en_clair() {
        // issue #51, tableau du comportement attendu — colonne « affiché » :
        // CPU, Liquide, GPU, NVMe. La typographie reste libre (voir les
        // arbitrages en tête de fichier), le mot ne l'est pas.
        // C'est ce qui interdit d'attribuer le libellé « GPU » à la sonde du
        // liquide : les deux rendraient un texte non vide et distinct, et le
        // panneau afficherait une température plausible sous le mauvais nom.
        for (slug, mot) in FAMILLES {
            let libelle = libelle(slug, &modeles());
            assert!(
                libelle.to_lowercase().contains(mot),
                "{slug} devrait se nommer « {mot} », rend « {libelle} »"
            );
        }
    }

    #[test]
    fn les_cinq_libelles_sont_deux_a_deux_distincts() {
        // Corollaire du critère « Les deux NVMe sont distinguables l'un de
        // l'autre », étendu à tout le panneau : deux cartes du même nom ne
        // disent pas laquelle chauffe. Vrai avec ou sans modèle de disque.
        for modeles in [modeles(), sans_modele()] {
            let libelles: Vec<String> = ATTENDUS.iter().map(|s| libelle(s, &modeles)).collect();
            for (i, gauche) in libelles.iter().enumerate() {
                for (j, droite) in libelles.iter().enumerate().skip(i + 1) {
                    assert_ne!(
                        gauche, droite,
                        "{} et {} portent le même libellé « {gauche} » (modèles : {modeles:?})",
                        ATTENDUS[i], ATTENDUS[j]
                    );
                }
            }
        }
    }

    #[test]
    fn la_table_et_le_panneau_ne_peuvent_pas_diverger() {
        // Les deux fonctions disent la même chose de la même sonde : la table
        // décide, le panneau applique. Un filtre appliqué d'un seul côté
        // donnerait un menu et un panneau en désaccord, et l'écart ne se verrait
        // qu'à l'usage.
        for slug in super::TOUTES {
            let attendu = libelle_retenu(slug, &modeles());
            let rendu = panneau(&[slug], &modeles());
            match attendu {
                Some(libelle) => {
                    assert_eq!(slugs_rendus(&rendu), vec![slug.to_owned()], "{slug}");
                    assert_eq!(rendu[0].libelle, libelle, "{slug}");
                }
                None => assert!(rendu.is_empty(), "{slug} est écartée, obtenu {rendu:?}"),
            }
        }
    }

    #[test]
    fn les_seize_sondes_de_la_machine_donnent_cinq_cartes() {
        // issue #51, critère — « Le panneau montre au plus une carte par entrée
        // du tableau ci-dessus ». Le cas réel, entier : seize entrent, cinq
        // sortent.
        let rendues = panneau(&super::TOUTES, &modeles());
        assert_eq!(
            slugs_rendus(&rendues),
            super::liste(&ATTENDUS),
            "seize sondes, cinq cartes"
        );
    }

    #[test]
    fn un_slug_inconnu_ou_tronque_ne_rend_rien() {
        // La table reconnaît un `slug` entier, pas un préfixe. Un rapprochement
        // approximatif rattraperait `k10temp:tccd1` par « k10temp » et
        // `nvme:sensor-2` par « nvme ». Et la table nomme deux disques, pas *n*
        // (voir les arbitrages) : un troisième n'a pas d'emplacement dans
        // `ModelesNvme`.
        for slug in [
            "",
            "k10temp",
            "tctl",
            "nvme",
            "nvme:nvme0",
            "nvme:nvme2:composite",
            "kraken2023elite",
            "nvidia",
        ] {
            assert_eq!(
                libelle_retenu(slug, &modeles()),
                None,
                "« {slug} » ne doit rien rendre"
            );
            assert!(panneau(&[slug], &modeles()).is_empty(), "« {slug} »");
        }
    }

    #[test]
    fn le_modele_du_disque_ne_change_rien_aux_trois_autres_familles() {
        // Le paramètre `ModelesNvme` ne concerne que les disques. S'il déplaçait
        // le libellé du CPU, du liquide ou du GPU, le panneau changerait de nom
        // selon qu'un fichier de `sysfs` a répondu — un mode de défaillance
        // silencieux, et sans rapport avec la donnée affichée.
        let etranges = ModelesNvme {
            nvme0: Some("".to_owned()),
            nvme1: Some("CPU".to_owned()),
        };
        for slug in [super::CPU, super::LIQUIDE, super::GPU] {
            assert_eq!(
                libelle_retenu(slug, &modeles()),
                libelle_retenu(slug, &sans_modele()),
                "{slug}"
            );
            assert_eq!(
                libelle_retenu(slug, &modeles()),
                libelle_retenu(slug, &etranges),
                "{slug}"
            );
        }
    }
}

// ---------------------------------------------------------------------------

mod ecartees {
    use super::{ECARTEES, libelle_retenu, modeles, panneau, sans_modele};

    #[test]
    fn aucune_sonde_ecartee_ne_rend_de_libelle() {
        // issue #51, test d'intention n° 2 et critère — « Aucune sonde
        // `spd5118`, `mt7921`, `r8169`, `amdgpu` ni `nvme:sensor-*` n'y
        // figure ».
        // `amdgpu:edge` est le piège du lot : c'est bien un GPU, celui qui est
        // intégré au processeur. Affiché comme « GPU » il ne produit aucune
        // erreur — juste une température qui ne monte jamais quand la carte
        // travaille.
        for slug in ECARTEES {
            for modeles in [modeles(), sans_modele()] {
                assert_eq!(
                    libelle_retenu(slug, &modeles),
                    None,
                    "{slug} ne doit pas être retenue"
                );
                assert!(
                    panneau(&[slug], &modeles).is_empty(),
                    "{slug} ne doit pas avoir de carte"
                );
            }
        }
    }

    #[test]
    fn une_machine_sans_aucune_sonde_retenue_montre_un_panneau_vide() {
        // issue #51, critère — « Sur une machine qui n'a aucune de ces sondes,
        // le panneau est vide et la fenêtre s'ouvre quand même ». Onze sondes
        // entrent, aucune carte ne sort, et rien ne panique.
        assert_eq!(panneau(&ECARTEES, &modeles()), Vec::new());
        assert_eq!(panneau(&ECARTEES, &sans_modele()), Vec::new());
    }
}

// ---------------------------------------------------------------------------

mod ordre {
    use super::{ATTENDUS, DESORDRES, ECARTEES, liste, modeles, panneau, slugs_rendus};

    #[test]
    fn l_ordre_rendu_est_cpu_liquide_gpu_nvme_quel_que_soit_l_ordre_d_entree() {
        // issue #51, test d'intention n° 3 et critère — « Le panneau montre au
        // plus une carte par entrée du tableau ci-dessus, **dans cet ordre** ».
        // L'ordre d'arrivée est celui de la découverte, alphabétique, et il
        // place par chance les cinq retenues dans le bon ordre : le tri doit
        // donc être vérifié sur des permutations, sinon un code qui ne trie
        // rien passerait.
        for desordre in DESORDRES {
            let rendues = panneau(&desordre, &modeles());
            assert_eq!(
                slugs_rendus(&rendues),
                liste(&ATTENDUS),
                "entrée : {desordre:?}"
            );
        }
    }

    #[test]
    fn l_ordre_tient_quand_les_sondes_ecartees_sont_intercalees() {
        // Le cas réel désordonné : les seize sondes arrivent mêlées. Ce qui est
        // écarté ne doit ni décaler, ni réordonner ce qui reste.
        let mut entree: Vec<&str> = Vec::new();
        for (i, retenue) in DESORDRES[0].iter().enumerate() {
            entree.push(ECARTEES[i * 2]);
            entree.push(retenue);
            entree.push(ECARTEES[i * 2 + 1]);
        }

        let rendues = panneau(&entree, &modeles());
        assert_eq!(slugs_rendus(&rendues), liste(&ATTENDUS), "{entree:?}");
    }

    #[test]
    fn les_deux_disques_se_suivent_et_arrivent_apres_le_gpu() {
        // Le tableau de l'issue lit « CPU, Liquide, GPU, NVMe » : les deux
        // disques forment la dernière famille, et `nvme0` précède `nvme1`.
        // Testé à part de l'égalité de liste ci-dessus, parce que c'est cette
        // lecture-là qui compte quand on regarde le panneau.
        let rendues = panneau(&DESORDRES[0], &modeles());
        let rendus = slugs_rendus(&rendues);
        let rang = |slug: &str| {
            rendus
                .iter()
                .position(|s| s == slug)
                .unwrap_or_else(|| panic!("{slug} absente de {rendus:?}"))
        };

        assert!(rang(super::CPU) < rang(super::LIQUIDE));
        assert!(rang(super::LIQUIDE) < rang(super::GPU));
        assert!(rang(super::GPU) < rang(super::NVME0));
        assert!(rang(super::NVME0) < rang(super::NVME1));
    }
}

// ---------------------------------------------------------------------------

mod nvme {
    use super::{
        MODELE_0, MODELE_1, ModelesNvme, NVME0, NVME1, libelle, modeles, sans_modele, slugs_rendus,
    };

    #[test]
    fn chaque_disque_porte_son_modele() {
        // issue #51, test d'intention n° 4 et critère — « Les deux NVMe sont
        // distinguables l'un de l'autre sans lire un numéro de `hwmon` » ;
        // comportement attendu — « distingués par le modèle du disque lu dans
        // `/sys/class/nvme/nvmeN/model` ».
        // Sans le modèle, les deux cartes disent « NVMe » et l'utilisateur
        // regarde une température sans savoir de quel disque elle vient.
        let zero = libelle(NVME0, &modeles());
        let un = libelle(NVME1, &modeles());

        assert!(
            zero.contains(MODELE_0),
            "{NVME0} devrait porter « {MODELE_0} », rend « {zero} »"
        );
        assert!(
            un.contains(MODELE_1),
            "{NVME1} devrait porter « {MODELE_1} », rend « {un} »"
        );
        assert_ne!(zero, un);
    }

    #[test]
    fn les_modeles_ne_se_croisent_pas() {
        // Le même critère, sur l'inversion : deux libellés bien distincts mais
        // échangés désignent le mauvais disque, et rien ne le dit. C'est
        // exactement la faute que le projet connaît déjà de sa cartographie de
        // canaux — « la table issue de la session Windows était fausse sur deux
        // groupes sur quatre » (CLAUDE.md).
        let zero = libelle(NVME0, &modeles());
        let un = libelle(NVME1, &modeles());

        assert!(
            !zero.contains(MODELE_1),
            "{NVME0} porte le modèle de l'autre"
        );
        assert!(!un.contains(MODELE_0), "{NVME1} porte le modèle de l'autre");
    }

    #[test]
    fn un_modele_illisible_laisse_deux_libelles_differents() {
        // issue #51, test d'intention n° 5 et approche technique — « S'il est
        // illisible, on retombe sur `nvme0` / `nvme1`, qui reste
        // distinguable ». Un `sysfs` muet ne doit pas faire fusionner les deux
        // cartes : le repli perd le modèle, jamais la distinction.
        let zero = libelle(NVME0, &sans_modele());
        let un = libelle(NVME1, &sans_modele());

        assert!(!zero.trim().is_empty(), "libellé vide pour {NVME0}");
        assert!(!un.trim().is_empty(), "libellé vide pour {NVME1}");
        assert_ne!(zero, un, "les deux disques doivent rester distinguables");
    }

    #[test]
    fn un_seul_modele_illisible_laisse_aussi_deux_libelles_differents() {
        // Le cas mixte, que l'issue n'énonce pas et qui arrive dès qu'un des
        // deux disques n'expose pas son modèle : le repli s'applique à celui-là
        // seul, et la distinction tient toujours.
        for modeles in [
            ModelesNvme {
                nvme0: Some(MODELE_0.to_owned()),
                nvme1: None,
            },
            ModelesNvme {
                nvme0: None,
                nvme1: Some(MODELE_1.to_owned()),
            },
        ] {
            let zero = libelle(NVME0, &modeles);
            let un = libelle(NVME1, &modeles);
            assert!(!zero.trim().is_empty(), "{modeles:?}");
            assert!(!un.trim().is_empty(), "{modeles:?}");
            assert_ne!(zero, un, "{modeles:?}");
        }
    }

    #[test]
    fn deux_disques_de_meme_modele_restent_distinguables() {
        // Arbitrage de contrat (voir en tête de fichier) : deux SSD identiques
        // est le cas courant, et « distinguables l'un de l'autre » ne souffre
        // pas d'exception. Le repli est le même que celui du modèle illisible,
        // le numéro du disque — qui n'est pas un numéro de `hwmon`.
        let jumeaux = ModelesNvme {
            nvme0: Some(MODELE_0.to_owned()),
            nvme1: Some(MODELE_0.to_owned()),
        };
        let zero = libelle(NVME0, &jumeaux);
        let un = libelle(NVME1, &jumeaux);

        assert!(zero.contains(MODELE_0), "« {zero} »");
        assert!(un.contains(MODELE_0), "« {un} »");
        assert_ne!(zero, un, "deux disques du même modèle, deux libellés");
    }

    #[test]
    fn les_deux_disques_ont_chacun_leur_carte() {
        // Une famille du tableau, deux `slug`, donc deux cartes. Fondre les deux
        // en une seule ferait disparaître un disque du panneau sans rien dire.
        let rendues = super::panneau(&[NVME1, NVME0], &modeles());
        assert_eq!(
            slugs_rendus(&rendues),
            vec![NVME0.to_owned(), NVME1.to_owned()]
        );
        assert_ne!(rendues[0].libelle, rendues[1].libelle);
    }
}

// ---------------------------------------------------------------------------

mod robustesse {
    use super::{ATTENDUS, liste, modeles, panneau, sans_modele, slugs_rendus};

    #[test]
    fn une_liste_vide_rend_une_liste_vide() {
        // issue #51, test d'intention n° 6 — « Une liste d'entrée vide rend une
        // liste vide, sans panique ». C'est ce que la fenêtre reçoit avant son
        // premier `status`, et au démarrage d'un démon qui n'a encore rien
        // découvert : elle doit s'ouvrir quand même.
        assert_eq!(panneau(&[], &modeles()), Vec::new());
        assert_eq!(panneau(&[], &sans_modele()), Vec::new());
    }

    #[test]
    fn une_sonde_retenue_absente_ne_laisse_pas_de_trou() {
        // issue #51, test d'intention n° 7 et comportement attendu — « Une sonde
        // retenue mais absente de la machine ne laisse pas un trou : elle
        // n'apparaît pas, et son absence ne fait pas disparaître les autres ».
        // Deux façons de rater : une carte vide à sa place, ou les suivantes qui
        // décalent et se retrouvent sous le nom de la précédente. L'entrée est
        // fournie à l'envers pour que le tri soit exercé à chaque tour.
        for manquante in ATTENDUS {
            let presentes: Vec<&str> = ATTENDUS
                .iter()
                .copied()
                .filter(|s| *s != manquante)
                .collect();
            let mut entree = presentes.clone();
            entree.reverse();

            let rendues = panneau(&entree, &modeles());
            assert_eq!(
                slugs_rendus(&rendues),
                liste(&presentes),
                "sans {manquante}"
            );
            for sonde in &rendues {
                assert!(
                    !sonde.libelle.trim().is_empty(),
                    "carte vide pour {} quand {manquante} manque",
                    sonde.slug
                );
            }
        }
    }

    #[test]
    fn aucune_sonde_retenue_presente_seule_ne_depend_des_autres() {
        // Le même critère poussé à bout : chaque famille doit savoir s'afficher
        // seule. Une table qui suppose ses voisines présentes — un index calculé
        // sur la liste d'entrée, par exemple — se casserait ici et nulle part
        // ailleurs.
        for slug in ATTENDUS {
            let rendues = panneau(&[slug], &modeles());
            assert_eq!(slugs_rendus(&rendues), vec![slug.to_owned()]);
            assert!(!rendues[0].libelle.trim().is_empty(), "{slug}");
        }
    }

    #[test]
    fn un_slug_repete_ne_fait_pas_deux_cartes() {
        // issue #51, critère — « Le panneau montre **au plus une** carte par
        // entrée du tableau ». Le démon rend ses sondes sans doublon, mais la
        // fenêtre concatène plusieurs sources — l'historique et le dernier
        // `status` — et la même sonde peut arriver deux fois. Deux cartes
        // identiques côte à côte se lisent comme deux disques.
        assert_eq!(
            slugs_rendus(&panneau(&[super::CPU, super::CPU], &modeles())).len(),
            1
        );

        let mut doublons: Vec<&str> = ATTENDUS.to_vec();
        doublons.extend_from_slice(&ATTENDUS);
        assert_eq!(
            slugs_rendus(&panneau(&doublons, &modeles())),
            liste(&ATTENDUS),
            "chaque sonde deux fois, une carte chacune"
        );
    }

    #[test]
    fn un_slug_repete_non_adjacent_ne_fait_pas_deux_cartes_non_plus() {
        // Le piège du dédoublonnage : ne comparer qu'aux voisins immédiats
        // suffit pour `[CPU, CPU]` et laisse passer `[CPU, GPU, CPU]`. La
        // fenêtre concaténant deux sources, c'est justement la forme qu'y prend
        // un doublon.
        let entree = [
            super::CPU,
            super::GPU,
            super::CPU,
            super::LIQUIDE,
            super::GPU,
        ];
        assert_eq!(
            slugs_rendus(&panneau(&entree, &modeles())),
            liste(&[super::CPU, super::LIQUIDE, super::GPU])
        );
    }
}
