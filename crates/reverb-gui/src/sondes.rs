//! L'historique des sondes, pour les tracer.
//!
//! Il vit **dans la fenêtre** et ne survit pas à sa fermeture. Persister deux
//! minutes de sparkline coûterait un format de fichier et un rythme d'écriture
//! sur disque pour une donnée dont la valeur tombe à zéro dès qu'on ferme.
//!
//! # Ce qu'une courbe ne doit pas faire dire
//!
//! Deux pièges, et les deux mentent de la même façon — en montrant une mesure
//! qui n'a pas eu lieu :
//!
//! - une sonde **qui vient d'apparaître** ne doit pas traîner derrière elle une
//!   ligne à zéro qui ferait croire à une chute ;
//! - une sonde **qui disparaît** — un périphérique débranché — ne doit pas figer
//!   sa dernière valeur et laisser croire qu'elle est encore lue.
//!
//! # Ce que la fenêtre montre, et ce qu'elle tait
//!
//! Le démon découvre **tout** ce que `hwmon` expose — seize sondes sur SHYNAEL,
//! dont quatre hubs SPD de barrettes, une puce Wi-Fi et une puce Ethernet — et
//! c'est le bon comportement pour un relevé : mieux vaut tout découvrir que
//! rater celle qui compte. Mais un panneau de seize cartes ne se lit plus.
//!
//! Le tri est donc un choix **d'affichage**, et il vit ici (issue #51).

use std::collections::{BTreeMap, VecDeque};

/// Combien de relevés une courbe garde.
///
/// Deux minutes au pas d'une seconde, qui est celui auquel la fenêtre interroge
/// le démon. Plus long ne se lirait plus dans la largeur d'une carte.
pub const MEMOIRE: usize = 120;

/// Le rapport largeur/hauteur dans lequel le tracé d'une sonde est émis.
///
/// ⚠️ **C'est le défaut corrigé en #113 sur la courbe de régulation, sur un
/// second tracé** : un `Path` dessiné dans le **carré unité** ne couvrait qu'une
/// partie de sa tuile, le reste restant vide. Mesuré au pixel sur l'image
/// d'aperçu : **60 px de tracé pour 81 px de cadre, soit 74 %** ; 67 % sur la
/// tuile qu'un libellé de NVMe élargit. À 2,0, on passe à **98 %**.
///
/// Comme [`crate::reglages::TRACE_ASPECT`], cette constante vit **ici** et non
/// dans le `.slint`, pour n'exister qu'une fois : la fenêtre la lit dans
/// `courbe-aspect` et en tire son `viewbox`. Deux chiffres à tenir d'accord
/// finiraient par diverger, et le symptôme serait celui qu'on corrige — un tracé
/// faux que rien ne signale.
///
/// ⚠️ **La valeur a été trouvée en balayant, pas en la déduisant.** Le calcul
/// « largeur du cadre / hauteur du cadre » donnait 1,4 et laissait 16 % de vide :
/// le rendu de Slint ne s'y ramène pas exactement, et une dérivation géométrique
/// aurait été plus convaincante que juste. Le balayage relevé le 2026-08-15 :
///
/// | rapport | largeur du tracé | hauteur |
/// |---|---|---|
/// | 1,0 *(avant)* | 60 px — 74 % | 55 px |
/// | 1,4 | 68 px — 84 % | 55 px |
/// | 1,7 | 73 px — 90 % | 55 px |
/// | **2,0** | **79 px — 98 %** | **55 px** |
/// | 2,1 | 80 px — 99 % | 54 px |
/// | 2,4 | 80 px — 99 % | 52 px |
///
/// **2,0 est le genou** : au-delà, le tracé gagne un pixel de large et en perd un
/// de haut. C'est donc le rapport qui dessine la plus grande courbe, et non le
/// plus grand nombre.
///
/// ⚠️ **Elle est distincte de `TRACE_ASPECT` (4,0), et doit le rester.** Une
/// tuile de sonde et le cadre de la courbe de régulation n'ont pas la même
/// forme ; les confondre remettrait le défaut sur l'un des deux.
///
/// ⚠️ **Les tuiles n'ont pas toutes la même largeur** — 93 px pour trois d'entre
/// elles, 124 et 113 pour les deux qu'un libellé de NVMe élargit —, si bien que
/// le tracé ne remplit au mieux que les premières. Les autres y gagnent quand
/// même. Égaliser les tuiles serait un changement de mise en page et non de
/// tracé : hors du périmètre de #118.
pub const COURBE_ASPECT: f32 = 2.0;

/// La marge laissée en haut et en bas du cadre, en fraction de sa hauteur.
///
/// Un trait collé au bord se fait rogner par sa propre épaisseur.
const MARGE_VERTICALE: f64 = 0.05;

/// Un relevé : une valeur, ou l'aveu qu'on n'a pas pu lire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Releve {
    /// En millidegrés, ou en tours par minute selon la sonde.
    Valeur(i32),
    Illisible,
}

/// Ce qu'on garde de chaque sonde.
///
/// Une file par sonde, jamais partagée : deux sondes qui se partageraient une
/// mémoire se voleraient leurs relevés dès que l'une bat plus vite que l'autre.
#[derive(Debug, Clone, Default)]
pub struct Historique {
    courbes: BTreeMap<String, VecDeque<Releve>>,
}

impl Historique {
    pub fn nouvel() -> Historique {
        Historique::default()
    }

    /// Note un relevé pour cette sonde.
    ///
    /// Au-delà de [`MEMOIRE`], le plus ancien tombe.
    pub fn noter(&mut self, sonde: &str, releve: Releve) {
        let courbe = self.courbes.entry(sonde.to_owned()).or_default();
        courbe.push_back(releve);
        // ⚠️ `Illisible` occupe une place comme les autres. Sans cela, une sonde
        // débranchée garderait indéfiniment ses vieilles valeurs à l'écran, et
        // la courbe dirait qu'on la lit encore.
        while courbe.len() > MEMOIRE {
            courbe.pop_front();
        }
    }

    /// Les relevés d'une sonde, du plus ancien au plus récent.
    ///
    /// Vide si la sonde n'a jamais été vue. **Jamais complétée** : une sonde
    /// apparue il y a trois secondes rend trois relevés, pas cent vingt.
    pub fn courbe(&self, sonde: &str) -> Vec<Releve> {
        self.courbes
            .get(sonde)
            .map(|courbe| courbe.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Les sondes connues, par ordre alphabétique de leur nom.
    pub fn sondes(&self) -> Vec<String> {
        // `BTreeMap` : le tri est celui de la structure, pas un tri de plus à
        // penser à refaire quand une sonde apparaît.
        self.courbes.keys().cloned().collect()
    }

    /// Le dernier relevé d'une sonde, s'il y en a un.
    pub fn dernier(&self, sonde: &str) -> Option<Releve> {
        self.courbes.get(sonde)?.back().copied()
    }

    /// Les bornes d'une courbe : la plus basse et la plus haute valeur lisible.
    ///
    /// `None` quand aucun relevé n'est lisible — il n'y a alors rien à mettre à
    /// l'échelle, et forcer un intervalle inventerait une courbe.
    pub fn bornes(&self, sonde: &str) -> Option<(i32, i32)> {
        let mut lisibles = self
            .courbes
            .get(sonde)?
            .iter()
            .filter_map(|releve| match releve {
                Releve::Valeur(valeur) => Some(*valeur),
                Releve::Illisible => None,
            });
        let premier = lisibles.next()?;
        Some(lisibles.fold((premier, premier), |(bas, haut), valeur| {
            (bas.min(valeur), haut.max(valeur))
        }))
    }
}

/// Le tracé d'une sonde, en commandes SVG sur [`COURBE_ASPECT`] × 1.
///
/// Une polyligne absolue — `M` puis des `L` —, **un point par relevé lisible**.
/// Rend la chaîne vide quand il n'y a rien à tracer : moins de deux relevés, ou
/// une sonde qu'on n'a jamais su lire.
///
/// ⚠️ **`x` court sur `0..=COURBE_ASPECT`, et non sur le carré unité** (#118),
/// pour la raison écrite sur [`COURBE_ASPECT`] : Slint ne sait pas étirer un
/// `Path`, il ne sait que le mettre à l'échelle uniformément.
///
/// ⚠️ **Un trou dans les relevés coupe le trait**, il ne l'enjambe pas. Relier
/// les deux bords d'une absence dessinerait une pente que personne n'a mesurée
/// — c'est le second piège écrit en tête de ce module.
///
/// ⚠️ **Une série plate se dessine au milieu du cadre**, jamais en haut ni en
/// bas : un trait collé à un bord se lirait comme un extrême, alors qu'il ne dit
/// que « rien n'a bougé ». C'est aussi le seul cas où l'étendue est nulle, donc
/// celui où la division rendrait `NaN`.
pub fn commandes_de_courbe(historique: &Historique, sonde: &str) -> String {
    let releves = historique.courbe(sonde);
    if releves.len() < 2 {
        return String::new();
    }
    let Some((bas, haut)) = historique.bornes(sonde) else {
        return String::new();
    };
    let etendue = f64::from(haut - bas);
    let dernier = (releves.len() - 1) as f64;
    let utile = 1.0 - 2.0 * MARGE_VERTICALE;
    let mut commandes = String::new();
    let mut pose = false;
    for (rang, releve) in releves.iter().enumerate() {
        let Releve::Valeur(valeur) = releve else {
            pose = false;
            continue;
        };
        let x = rang as f64 / dernier * f64::from(COURBE_ASPECT);
        // ⚠️ `y` descend quand la mesure monte : le repère de Slint a son axe
        // vertical vers le bas, comme celui de SVG.
        let y = if etendue > 0.0 {
            1.0 - f64::from(valeur - bas) / etendue
        } else {
            0.5
        };
        let y = MARGE_VERTICALE + y * utile;
        commandes.push_str(&format!("{} {x:.4} {y:.4} ", if pose { "L" } else { "M" }));
        pose = true;
    }
    commandes
}

// ---------------------------------------------------------------------------
// Les sondes retenues (issue #51)
// ---------------------------------------------------------------------------

/// Une sonde que le panneau montre : son `slug` et son libellé lisible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SondeRetenue {
    pub slug: String,
    pub libelle: String,
}

/// Ce que la fenêtre a lu une fois dans `/sys/class/nvme/nvmeN/model`.
///
/// `default()` est le cas des deux modèles illisibles — une machine où `sysfs`
/// n'a pas répondu. Il ne rend pas les deux disques indiscernables pour autant :
/// voir [`libelle_du_disque`].
#[derive(Debug, Clone, Default)]
pub struct ModelesNvme {
    pub nvme0: Option<String>,
    pub nvme1: Option<String>,
}

/// Le CPU — celle que les ventilateurs suivent, et non un die parmi d'autres.
const CPU: &str = "k10temp:tctl";
/// Le liquide du Kraken.
const LIQUIDE: &str = "kraken2023elite:coolant-temp";
/// La carte graphique.
///
/// ⚠️ **Pas `amdgpu:edge`**, qui est le GPU intégré du 9800X3D : affiché sous le
/// nom « GPU », il donnerait une température plausible et fausse — celle qui ne
/// bouge pas quand le jeu chauffe.
const GPU: &str = "nvidia:NVIDIA_GeForce_RTX_5070";
/// Le premier disque NVMe.
const NVME0: &str = "nvme:nvme0:composite";
/// Le second disque NVMe.
const NVME1: &str = "nvme:nvme1:composite";

/// Les sondes retenues, **dans l'ordre du panneau**.
///
/// C'est cette table qui donne l'ordre, et non un tri des libellés : le panneau
/// doit se lire pareil quel que soit l'ordre dans lequel le démon rend ses
/// lignes `temp`.
const RETENUES: [&str; 5] = [CPU, LIQUIDE, GPU, NVME0, NVME1];

/// Le libellé d'une sonde retenue, ou `None` si elle ne l'est pas.
///
/// Fonction **pure** : le modèle du disque est un paramètre. Le lire ici ferait
/// dépendre le tri de la machine sur laquelle il tourne.
pub fn libelle_retenu(slug: &str, modeles: &ModelesNvme) -> Option<String> {
    match slug {
        CPU => Some("CPU".to_owned()),
        LIQUIDE => Some("Liquide".to_owned()),
        GPU => Some("GPU".to_owned()),
        NVME0 => Some(libelle_du_disque(0, modeles)),
        NVME1 => Some(libelle_du_disque(1, modeles)),
        _ => None,
    }
}

/// Les sondes à montrer, dans l'ordre du panneau, sans doublon.
///
/// Parcourt [`RETENUES`] plutôt que l'entrée : l'ordre est garanti, un `slug`
/// répété ne produit qu'une carte, et une sonde absente de la machine ne laisse
/// pas de trou — elle manque, simplement.
pub fn sondes_retenues(slugs: &[String], modeles: &ModelesNvme) -> Vec<SondeRetenue> {
    RETENUES
        .iter()
        .filter(|retenue| slugs.iter().any(|slug| slug == *retenue))
        .filter_map(|retenue| {
            libelle_retenu(retenue, modeles).map(|libelle| SondeRetenue {
                slug: (*retenue).to_owned(),
                libelle,
            })
        })
        .collect()
}

/// Le libellé d'un des deux disques.
///
/// ⚠️ **Le modèle ne suffit pas à les distinguer.** Deux SSD identiques dans une
/// machine est le cas courant, et l'issue exige de savoir « laquelle chauffe ».
///
/// Trois cas, et le numéro du disque n'apparaît que quand il sert vraiment — un
/// `nvme0` collé à un modèle déjà unique n'apprendrait rien et allongerait une
/// carte étroite :
///
/// - modèle lisible et **différent** de celui du voisin : le modèle suffit ;
/// - modèle lisible mais **partagé** : le modèle, plus le numéro pour trancher ;
/// - modèle illisible : le numéro seul.
///
/// Ce numéro n'est pas un numéro de `hwmon` — il ne change pas au redémarrage,
/// et l'issue demandait précisément de ne pas en faire lire un.
fn libelle_du_disque(rang: usize, modeles: &ModelesNvme) -> String {
    let (mien, autre) = if rang == 0 {
        (&modeles.nvme0, &modeles.nvme1)
    } else {
        (&modeles.nvme1, &modeles.nvme0)
    };
    match mien {
        Some(modele) if Some(modele) != autre.as_ref() => format!("NVMe {modele}"),
        Some(modele) => format!("NVMe {modele} (nvme{rang})"),
        None => format!("NVMe nvme{rang}"),
    }
}

/// Les modèles des deux premiers disques, lus dans `sysfs`.
///
/// Lecture d'un fichier en lecture seule, faite **une fois au démarrage** : ce
/// n'est pas ouvrir un périphérique, et l'ADR-002 n'est donc pas touché. Un
/// fichier absent ou vide donne `None`, ce que [`libelle_du_disque`] sait
/// traiter.
pub fn modeles_nvme() -> ModelesNvme {
    ModelesNvme {
        nvme0: modele_de("/sys/class/nvme/nvme0/model"),
        nvme1: modele_de("/sys/class/nvme/nvme1/model"),
    }
}

/// Le modèle écrit dans un fichier de `sysfs`, débarrassé de son bourrage.
///
/// `sysfs` complète le champ à sa largeur fixe avec des espaces : sans le
/// `trim`, le libellé traînerait vingt blancs derrière lui.
fn modele_de(chemin: &str) -> Option<String> {
    let brut = std::fs::read_to_string(chemin).ok()?;
    let propre = brut.trim();
    (!propre.is_empty()).then(|| propre.to_owned())
}
