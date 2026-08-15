//! Les animations calculées par l'hôte, et la géométrie qu'elles traversent.
//!
//! Ce crate est **pur** : aucune entrée/sortie, aucun périphérique. Il est
//! importé par le démon *et* par la fenêtre, pour que l'aperçu du boîtier
//! affiche exactement les images écrites sur le bus — pas une
//! réimplémentation qui divergerait à la première animation ajoutée d'un seul
//! côté.
//!
//! Il est distinct de `reverb-proto`, dont la règle est de ne contenir que du
//! relevé matériel : ces animations sont de nous. Seule leur **nécessité** est
//! observée — le contrôleur de la RAM ne sait pas animer seul
//! (SPEC-CORSAIR-RAM §4.5), et rien ne permet de synchroniser une animation
//! firmware des ventilateurs avec quoi que ce soit d'autre.
//!
//! ## Comment un motif traverse le boîtier
//!
//! Chaque animation ramène une LED à un seul nombre entre 0 et 1, puis peint en
//! fonction de ce nombre et du temps. C'est ce qui remplace la file d'attente
//! numérotée de la première `vague`.
//!
//! Ce nombre se calcule de deux façons, et le catalogue se partage entre elles.
//! `vague` prend la **position réelle** le long de la direction : deux LED à la
//! même hauteur reçoivent donc la même couleur d'une onde qui monte, quels que
//! soient leur ventilateur et leur numéro d'ordre. Les quatre autres familles
//! dirigées suivent l'**écoulement**, qui traverse chaque ventilateur d'un bord
//! à l'autre même quand la direction l'aplatit — voir [`Geometrie::traversee`].
//!
//! Les cinq dernières ne suivent aucune direction, et n'en acceptent aucune :
//! `rotation` suit l'angle relevé de chaque anneau, `pouls` la distance à la
//! pompe, `thermique` une sonde, `scintillement` rien du tout, et `braise`
//! croise la distance à la pompe et l'angle (#119).

use std::fmt;

use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

pub mod geometrie;

pub use geometrie::{Geometrie, GeometrieInvalide, Orientation, OrientationInvalide, Point, Sens};

/// Une image complète : les dix ventilateurs, puis les quatre barrettes.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub ventilateurs: [(Position, [Rgb; LEDS_PER_FAN as usize]); 10],
    pub barrettes: [[Rgb; LEDS_PER_STICK]; SLOT_COUNT],
}

/// Direction d'un motif dans le boîtier.
///
/// `Horaire` et `Antihoraire` décrivent **le tour du boîtier vu de face** :
/// plancher, flanc de la carte mère, plafond. C'est le chemin que Nico décrit —
/// « on part du bas des ventilos d'en bas, on remonte vers la face du fond où
/// se situe la CM, on grimpe ce fond, puis du fond des ventilos du haut on
/// revient vers nous ».
///
/// ⚠️ **Il n'y a pas de direction gauche-droite.** Le seul axe qu'elle
/// traverserait est celui de la vitre au plateau de carte mère, et il ne porte
/// que trois ventilateurs — le radiateur, à une extrémité. Les sept autres s'y
/// superposeraient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    BasHaut,
    HautBas,
    AvantArriere,
    ArriereAvant,
    Horaire,
    Antihoraire,
    /// Des deux bords de **chaque objet** vers son milieu (#75).
    BordsCentre,
    /// L'inverse : du milieu de chaque objet vers ses deux bords.
    CentreBords,
}

impl Direction {
    /// Les huit directions, dans un ordre stable.
    pub const ALL: [Direction; 8] = [
        Direction::BasHaut,
        Direction::HautBas,
        Direction::AvantArriere,
        Direction::ArriereAvant,
        Direction::Horaire,
        Direction::Antihoraire,
        Direction::BordsCentre,
        Direction::CentreBords,
    ];

    /// Le mot qui écrit cette direction sur le socket.
    pub const fn slug(self) -> &'static str {
        match self {
            Direction::BasHaut => "bas-haut",
            Direction::HautBas => "haut-bas",
            Direction::AvantArriere => "avant-arriere",
            Direction::ArriereAvant => "arriere-avant",
            Direction::Horaire => "horaire",
            Direction::Antihoraire => "antihoraire",
            Direction::BordsCentre => "bords-centre",
            Direction::CentreBords => "centre-bords",
        }
    }

    /// Cette direction se mesure-t-elle **dans l'objet** plutôt que dans le
    /// boîtier ?
    ///
    /// ⚠️ C'est la seule chose qui les distingue, et elle ne produit **aucun
    /// message** : une direction locale traitée comme globale rend 124 couleurs
    /// parfaitement plausibles. Le motif se répète alors sur chacun des quatorze
    /// objets au lieu de traverser le volume — c'est ce que fait iCUE sur la RAM,
    /// « depuis chaque bord vers l'intérieur de chacune ».
    pub const fn est_locale(self) -> bool {
        matches!(self, Direction::BordsCentre | Direction::CentreBords)
    }

    /// L'inverse de [`Direction::slug`], strict.
    fn depuis_slug(brut: &str) -> Option<Direction> {
        Direction::ALL.into_iter().find(|d| d.slug() == brut)
    }
}

/// La place d'une LED dans son objet, sous une direction locale.
///
/// `parcours` va de 0 à 1 d'un bord de l'objet à l'autre. Le résultat est
/// **symétrique** autour du milieu — les deux bords y valent la même chose —,
/// ce qui est exactement le motif demandé : « les deux LED d'extrémité d'une
/// même barrette s'allument au même instant ».
///
/// Sous `bords-centre`, le bord vaut 0 et le milieu 1 : le motif progresse par
/// places croissantes, donc du bord vers le milieu, « et le milieu en dernier ».
fn place_locale(direction: Direction, parcours: f32) -> f32 {
    let depuis_le_milieu = (2.0 * parcours.clamp(0.0, 1.0) - 1.0).abs();
    ETENDUE_LOCALE
        * match direction {
            Direction::CentreBords => depuis_le_milieu,
            _ => 1.0 - depuis_le_milieu,
        }
}

/// Part d'un cycle qu'un motif local occupe, du bord d'un objet à son milieu.
///
/// ⚠️ **Strictement inférieure à 1, et c'est indispensable.** Les motifs du
/// catalogue vivent modulo 1 : `fraction(1.0)` vaut `0.0`, si bien qu'une place
/// locale atteignant 1 ferait retomber le **milieu** d'une barrette exactement
/// sur son **bord**. Mesuré : `comete` n'y distinguait plus jamais les deux, et
/// `respiration` y rendait un retard négatif — le motif semblait remonter du
/// milieu vers les bords sous « bords-centre ».
///
/// Laisser un sixième de cycle libre suffit à séparer les deux extrémités, et
/// garde au motif la lenteur qui le rend lisible.
const ETENDUE_LOCALE: f32 = 5.0 / 6.0;

/// La même, pour un objet dont les LED sont **numérotées** — une barrette.
///
/// ⚠️ **Calculée sur les indices, jamais sur un parcours flottant**, et c'est
/// tout l'intérêt : la symétrie exigée est une **égalité de couleurs**, donc de
/// nombres arrondis à l'octet. Or `2 × (9/10) − 1` ne vaut pas exactement
/// `|2 × (1/10) − 1|` en `f32` : la LED 1 et la LED 9 ressortaient à 153 et 152.
/// Un écart d'une unité sur 255, invisible à l'œil, et une symétrie fausse.
///
/// Prendre la distance au milieu en entiers rend les deux moitiés rigoureusement
/// identiques, par construction plutôt que par chance d'arrondi.
fn place_locale_discrete(direction: Direction, indice: usize, total: usize) -> f32 {
    let dernier = total.saturating_sub(1);
    let demi = (dernier / 2).max(1);
    let depuis_le_bord = indice.min(dernier.saturating_sub(indice)).min(demi);
    let vers_le_milieu = depuis_le_bord as f32 / demi as f32;
    ETENDUE_LOCALE
        * match direction {
            Direction::CentreBords => 1.0 - vers_le_milieu,
            _ => vers_le_milieu,
        }
}

/// Vitesse la plus lente acceptée.
const VITESSE_MIN: u8 = 1;
/// Vitesse la plus rapide acceptée.
const VITESSE_MAX: u8 = 10;

/// Part d'un cycle qu'un ventilateur occupe dans le motif.
///
/// Deux fois le rayon de l'anneau rapporté à l'étendue du boîtier : c'est la
/// place qu'un ventilateur occupe **réellement** le long d'une direction qui ne
/// l'aplatit pas. La reprendre pour ceux que la direction aplatit fait traverser
/// tous les ventilateurs à la même allure, quel que soit leur plan.
const EMPRISE: f32 = 0.26;

/// Durée d'un cycle complet, en pas, à la vitesse 1.
///
/// Quatre secondes à trente images par seconde. Rien de physique : c'est la
/// lenteur qui rend une onde lisible plutôt que clignotante.
const PERIODE: u64 = 120;

/// Période de l'horloge du scintillement, en pas.
///
/// **Premier**, donc étranger à toutes les vitesses de 1 à 10 : deux pas voisins
/// donnent toujours deux valeurs distinctes. Avec [`PERIODE`], à la vitesse 3,
/// l'horloge ne prendrait que quarante valeurs — quarante images pour une
/// animation censée n'avoir aucun cycle.
const PERIODE_DERIVE: u64 = 1021;

/// Les réglages d'une animation.
///
/// Un seul jeu de défauts pour tout le catalogue : une animation qui n'accepte
/// pas `couleur` laisse simplement le champ tranquille.
/// ⚠️ **Pas `Copy`**, depuis que `sonde` porte un `String` (#75). Le seul
/// substitut qui l'aurait gardé serait une chaîne de taille fixe embarquée, pour
/// un slug qui vient du réseau de sondes et dont personne ne connaît la longueur
/// maximale.
#[derive(Debug, Clone, PartialEq)]
pub struct Reglages {
    pub couleur: Rgb,
    pub vitesse: u8,
    pub direction: Direction,
    /// Le `slug` de la sonde à suivre. Seule `thermique` l'accepte, et elle
    /// l'exige.
    ///
    /// ⚠️ **Le nom, jamais la valeur.** Le nom est un réglage : il se tape, il
    /// s'écrit dans `eclairage.conf`, il se relit au redémarrage. La valeur
    /// change à chaque image et passe en argument d'[`Animation::image_avec_mesure`]
    /// — ce crate est pur et ne lit aucune sonde.
    pub sonde: Option<String>,
}

impl Default for Reglages {
    fn default() -> Reglages {
        Reglages {
            couleur: Rgb::new(0xff, 0x40, 0xff),
            vitesse: 3,
            sonde: None,
            // `horaire` et non `bas-haut` : c'est la seule direction où les dix
            // ventilateurs ont tous de l'épaisseur. Six d'entre eux sont
            // couchés — une onde verticale les traverse d'un bloc, et trois
            // autres sont plaqués dans le plan du radiateur, qu'une onde
            // avant-arrière aplatit de même. Le tour du boîtier n'aplatit
            // personne.
            direction: Direction::Horaire,
        }
    }
}

/// Erreur rendue lorsqu'un nom d'animation ne figure pas au [`CATALOGUE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationInconnue {
    pub saisi: String,
    pub valides: &'static [&'static str],
}

impl fmt::Display for AnimationInconnue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "animation « {} » inconnue. Animations disponibles : {}",
            self.saisi,
            self.valides.join(", ")
        )
    }
}

impl std::error::Error for AnimationInconnue {}

/// Erreur rendue lorsqu'un réglage est inconnu ou hors bornes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReglageInvalide {
    /// La clé fautive, toujours nommée — un refus muet ferait chercher.
    pub cle: String,
    pub raison: String,
    /// Les clés acceptées par l'animation visée.
    pub acceptees: &'static [&'static str],
}

impl fmt::Display for ReglageInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "réglage « {} » : {}. Réglages acceptés : {}",
            self.cle,
            self.raison,
            self.acceptees.join(", ")
        )
    }
}

impl std::error::Error for ReglageInvalide {}

/// Les familles du catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Famille {
    Vague,
    Comete,
    Respiration,
    ArcEnCiel,
    Balayage,
    Braise,
    /// Chaque anneau tourne **sur lui-même** (#75).
    Rotation,
    /// La couleur suit une sonde (#75).
    Thermique,
    /// Une onde **sphérique** née à la pompe (#75).
    Pouls,
    /// Des LED s'allument au hasard, sans période (#75).
    Scintillement,
}

/// Nom et famille, dans l'ordre du [`CATALOGUE`].
const FAMILLES: [(&str, Famille); 10] = [
    ("vague", Famille::Vague),
    ("comete", Famille::Comete),
    ("respiration", Famille::Respiration),
    ("arc-en-ciel", Famille::ArcEnCiel),
    ("balayage", Famille::Balayage),
    ("braise", Famille::Braise),
    ("rotation", Famille::Rotation),
    ("thermique", Famille::Thermique),
    ("pouls", Famille::Pouls),
    ("scintillement", Famille::Scintillement),
];

/// Les animations que le démon sait jouer.
///
/// `vague` y figure : le protocole s'étend, il ne casse pas. `off` n'y figure
/// pas — c'est l'absence d'animation, portée par `name: None`, et deux chemins
/// pour éteindre en rendraient un des deux faux.
pub const CATALOGUE: &[&str] = &[
    "vague",
    "comete",
    "respiration",
    "arc-en-ciel",
    "balayage",
    "braise",
    "rotation",
    "thermique",
    "pouls",
    "scintillement",
];

/// Les clés acceptées par une animation qui se colore.
const AVEC_COULEUR: &[&str] = &["couleur", "vitesse", "direction"];
/// Les clés acceptées par une animation qui produit ses propres teintes.
const SANS_COULEUR: &[&str] = &["vitesse", "direction"];
/// Les clés d'une animation dont le motif ne suit **aucune** direction du
/// boîtier : `rotation` suit le montage relevé de chaque anneau, `pouls` la
/// distance à la pompe, `scintillement` le hasard, et `braise` la distance à la
/// pompe croisée à l'angle de chaque anneau (#119).
const SANS_DIRECTION: &[&str] = &["couleur", "vitesse"];
/// Les clés de `thermique` : ses couleurs viennent du gradient, et sa sonde est
/// **exigée** — une animation qui ne pourrait jamais rien relever afficherait un
/// blanc pulsant permanent que rien n'expliquerait.
const SUIT_UNE_SONDE: &[&str] = &["vitesse", "sonde"];

/// Une animation du catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Animation {
    famille: Famille,
    nom: &'static str,
}

impl Animation {
    pub fn par_nom(nom: &str) -> Result<Animation, AnimationInconnue> {
        FAMILLES
            .into_iter()
            .find(|(connu, _)| *connu == nom)
            .map(|(nom, famille)| Animation { famille, nom })
            .ok_or_else(|| AnimationInconnue {
                saisi: nom.to_owned(),
                valides: CATALOGUE,
            })
    }

    pub fn nom(&self) -> &'static str {
        self.nom
    }

    /// Les clés que cette animation accepte — la seule source de vérité du
    /// refus, pour qu'ajouter un paramètre sans l'accepter soit impossible.
    pub fn parametres_acceptes(&self) -> &'static [&'static str] {
        match self.famille {
            // L'arc-en-ciel génère ses teintes : lui donner une couleur n'aurait
            // aucun sens, et l'accepter poliment sans l'employer serait pire.
            Famille::ArcEnCiel => SANS_COULEUR,
            // Le gradient produit ses couleurs, et la sonde remplace la
            // direction : un boîtier qui affiche une température n'a pas de
            // sens de lecture.
            Famille::Thermique => SUIT_UNE_SONDE,
            // Quatre motifs qui ne se projettent sur aucun axe du boîtier :
            // l'anneau suit son montage relevé, l'onde suit la distance à la
            // pompe, le scintillement ne suit rien, et la braise croise la
            // distance à la pompe et l'angle de chaque anneau. Accepter
            // `direction` serait ranger un réglage puis l'oublier.
            //
            // ⚠️ **`braise` les a rejoints (#119), et c'est une rupture de
            // compatibilité** : `animate braise direction=…` est désormais
            // refusé, la commande entière avec, et un profil enregistré qui
            // porte cette clé cesse de s'appliquer tant qu'il n'est pas
            // réenregistré. Le prix est assumé — accepter la clé pour ne rien en
            // faire serait le réglage qui ment que ce projet refuse partout.
            Famille::Rotation | Famille::Pouls | Famille::Scintillement | Famille::Braise => {
                SANS_DIRECTION
            }
            _ => AVEC_COULEUR,
        }
    }

    /// Les clés que cette animation **exige**, et non seulement accepte.
    ///
    /// ⚠️ **`thermique` est la première du catalogue à en avoir une**, et c'est
    /// ce qui l'a rendue nécessaire : `Reglages::default()` n'a aucune valeur
    /// sensée pour `sonde`. Il n'existe pas de sonde par défaut — la machine en
    /// expose seize, et le choix dépend d'elle.
    ///
    /// Jusqu'à #75, tout réglage avait un défaut, si bien que `reglages(&[])`
    /// rendait toujours `Reglages::default()` (#27). Cet invariant vaut encore
    /// pour toute animation dont cette liste est vide — c'est-à-dire les neuf
    /// autres.
    pub fn parametres_obligatoires(&self) -> &'static [&'static str] {
        match self.famille {
            Famille::Thermique => &["sonde"],
            _ => &[],
        }
    }

    /// L'inverse de [`Animation::reglages`] : les paires à réécrire.
    ///
    /// **Seulement les clés que cette animation accepte.** `arc-en-ciel` refuse
    /// `couleur` ; la lui redonner ferait rejeter en bloc le fichier d'état ou
    /// la commande qui la relance, et l'éclairage entier serait perdu pour une
    /// clé de trop.
    pub fn reglages_ecrits(&self, reglages: &Reglages) -> Vec<(String, String)> {
        // Déstructuration exhaustive, volontairement sans `..` : un réglage
        // ajouté un jour à `Reglages` ne compilera plus ici tant qu'on ne lui
        // aura pas donné sa place sur le fil. Sans ça, il serait simplement
        // perdu au redémarrage, sans un message.
        let Reglages {
            couleur,
            vitesse,
            direction,
            sonde,
        } = reglages;
        let tous = [
            ("couleur", couleur_hexa(*couleur)),
            ("vitesse", vitesse.to_string()),
            ("direction", direction.slug().to_owned()),
            ("sonde", sonde.clone().unwrap_or_default()),
        ];
        tous.into_iter()
            .filter(|(cle, _)| self.parametres_acceptes().contains(cle))
            .map(|(cle, valeur)| (cle.to_owned(), valeur))
            .collect()
    }

    /// Valide des paires brutes venues du protocole.
    ///
    /// Toutes les paires sont examinées : un décodeur qui s'arrêterait à la
    /// première paire acceptable appliquerait la moitié des réglages sans
    /// rien dire.
    pub fn reglages(&self, paires: &[(String, String)]) -> Result<Reglages, ReglageInvalide> {
        let acceptees = self.parametres_acceptes();
        let mut reglages = Reglages::default();

        for (cle, valeur) in paires {
            if !acceptees.contains(&cle.as_str()) {
                return Err(ReglageInvalide {
                    cle: cle.clone(),
                    raison: format!("« {cle} » n'est pas un réglage de « {} »", self.nom),
                    acceptees,
                });
            }
            let refus = |raison: String| ReglageInvalide {
                cle: cle.clone(),
                raison,
                acceptees,
            };
            match cle.as_str() {
                "couleur" => reglages.couleur = couleur(valeur).map_err(refus)?,
                "vitesse" => reglages.vitesse = vitesse(valeur).map_err(refus)?,
                "direction" => {
                    reglages.direction = Direction::depuis_slug(valeur).ok_or_else(|| {
                        refus(format!(
                            "« {valeur} » n'est pas une direction. Directions : {}",
                            Direction::ALL.map(Direction::slug).join(", ")
                        ))
                    })?;
                }
                "sonde" => {
                    // Une chaîne vide n'est le slug d'aucune sonde. L'accepter
                    // lancerait une animation qui ne pourra jamais rien
                    // relever, donc un blanc pulsant permanent que rien
                    // n'expliquerait.
                    if valeur.trim().is_empty() {
                        return Err(refus(
                            "une sonde vide ne se relève jamais — donne son slug, celui que \
                             « status » liste"
                                .to_owned(),
                        ));
                    }
                    reglages.sonde = Some(valeur.clone());
                }
                autre => unreachable!("« {autre} » est déclarée acceptée sans être décodée"),
            }
        }

        // ⚠️ **Exigée, pas seulement acceptée.** Sans sonde, `thermique`
        // pulserait en blanc indéfiniment — indiscernable d'une sonde tombée en
        // panne, donc le mode de défaillance rassurant que le projet refuse
        // partout ailleurs.
        for cle in self.parametres_obligatoires() {
            let manquante = match *cle {
                "sonde" => reglages.sonde.is_none(),
                autre => unreachable!("« {autre} » est déclarée obligatoire sans être vérifiée"),
            };
            if manquante {
                return Err(ReglageInvalide {
                    cle: (*cle).to_owned(),
                    raison: format!(
                        "« {} » suit une sonde : il lui en faut une, par exemple \
                         « sonde=kraken2023elite:coolant-temp ». « status » les liste toutes",
                        self.nom
                    ),
                    acceptees,
                });
            }
        }
        Ok(reglages)
    }

    /// L'image du pas donné. Fonction pure : mêmes entrées, même sortie.
    pub fn image(&self, geometrie: &Geometrie, reglages: &Reglages, pas: u32) -> Image {
        self.image_avec_mesure(geometrie, reglages, pas, None)
    }

    /// La même, en connaissant la valeur de la sonde que `thermique` suit.
    ///
    /// `mesure` est en degrés, ou `None` si la sonde ne répond pas — écartée par
    /// la quarantaine (#68), absente, ou jamais relevée.
    ///
    /// ⚠️ **`None` est le défaut sûr, et [`Animation::image`] l'emploie.** Un
    /// appelant qui oublierait de passer la mesure obtient le blanc pulsant,
    /// jamais une température plausible. Retomber sur une valeur par défaut
    /// serait le « 34 °C figé derrière une pompe arrêtée » que le projet refuse
    /// partout ailleurs.
    ///
    /// Reste **pure** : ce crate ne lit aucune sonde, et n'a pas à en connaître.
    pub fn image_avec_mesure(
        &self,
        geometrie: &Geometrie,
        reglages: &Reglages,
        pas: u32,
        mesure: Option<f32>,
    ) -> Image {
        let bornes = geometrie.bornes();
        let temps = temps(pas, reglages.vitesse);
        let derive = derive(pas, reglages.vitesse);
        let locale = reglages.direction.est_locale();
        let pompe = geometrie.pompe();
        let etendue = distance(bornes.0, bornes.1).max(1.0);

        let mut ventilateurs = [(Position::BasGauche, [Rgb::BLACK; LEDS_PER_FAN as usize]); 10];
        for (rang, (place, position)) in ventilateurs.iter_mut().zip(Position::ALL).enumerate() {
            // Le ventilateur entier occupe une place dans le motif ; chacune de
            // ses LED occupe une fraction de cette place, selon où elle se
            // trouve dans sa traversée.
            //
            // ⚠️ Calculée **seulement** hors direction locale : la place d'un
            // ventilateur dans le boîtier n'a pas de sens pour un motif qui se
            // répète à l'identique sur chaque objet, et `projection` refuse à
            // juste titre de lui en donner une.
            let du_ventilateur = if locale {
                0.0
            } else {
                projection(
                    reglages.direction,
                    geometrie.centre_ventilateur(position),
                    bornes,
                )
            };
            let mut couleurs = [Rgb::BLACK; LEDS_PER_FAN as usize];
            for (led, couleur) in couleurs.iter_mut().enumerate() {
                if let Some(point) = geometrie.led_ventilateur(position, led) {
                    let traversee = geometrie.traversee(position, led);
                    // ⚠️ Sous une direction locale, la place du ventilateur dans
                    // le boîtier **ne compte plus** : c'est ce qui fait que deux
                    // ventilateurs montés pareil portent la même image, où qu'ils
                    // soient. La traversée est la coordonnée le long de l'objet,
                    // celle-là même que l'écoulement suit.
                    let (spatiale, flux) = if locale {
                        let dans_l_objet = place_locale(reglages.direction, traversee);
                        (dans_l_objet, dans_l_objet)
                    } else {
                        (
                            projection(reglages.direction, point, bornes),
                            du_ventilateur + EMPRISE * (traversee - 0.5),
                        )
                    };
                    *couleur = self.peindre(
                        reglages,
                        &Place {
                            spatiale,
                            flux,
                            temps,
                            // L'angle relevé, pas le numéro de LED : c'est le
                            // montage qui décide où se trouve la LED sur son
                            // anneau, et `rotation` doit le suivre.
                            angle: f32::from(geometrie.orientation(position).angle_led(led))
                                / 360.0,
                            rayon: distance(point, pompe) / etendue,
                            graine: (rang * LEDS_PER_FAN as usize + led) as u32,
                            derive,
                            mesure,
                        },
                    );
                }
            }
            *place = (position, couleurs);
        }

        let mut barrettes = [[Rgb::BLACK; LEDS_PER_STICK]; SLOT_COUNT];
        for (slot, couleurs) in barrettes.iter_mut().enumerate() {
            for (led, couleur) in couleurs.iter_mut().enumerate() {
                if let Some(point) = geometrie.led_barrette(slot, led) {
                    // Une barrette est une ligne de onze LED : la direction ne
                    // l'aplatit jamais au point qu'il faille lui inventer une
                    // traversée. Sa position réelle suffit dans les deux voies.
                    let spatiale = if locale {
                        place_locale_discrete(reglages.direction, led, LEDS_PER_STICK)
                    } else {
                        projection(reglages.direction, point, bornes)
                    };
                    *couleur = self.peindre(
                        reglages,
                        &Place {
                            spatiale,
                            flux: spatiale,
                            temps,
                            // Une barrette n'a pas d'anneau : son équivalent
                            // radial est bords↔centre, l'analogue le plus proche
                            // pour un objet linéaire. Une RAM éteinte pendant
                            // qu'un motif tourne se lirait comme une panne.
                            angle: place_locale_discrete(
                                Direction::BordsCentre,
                                led,
                                LEDS_PER_STICK,
                            ),
                            rayon: distance(point, pompe) / etendue,
                            graine: (10 * LEDS_PER_FAN as usize + slot * LEDS_PER_STICK + led)
                                as u32,
                            derive,
                            mesure,
                        },
                    );
                }
            }
        }

        Image {
            ventilateurs,
            barrettes,
        }
    }

    /// La couleur d'une LED, connaissant sa place dans le boîtier et l'instant.
    ///
    /// Deux places, et le catalogue se partage entre elles :
    ///
    /// - `spatiale` — la position réelle le long de la direction demandée ;
    /// - `flux` — la place du ventilateur, plus la traversée de la LED depuis
    ///   le point d'entrée de l'écoulement.
    ///
    /// ## Pourquoi les deux, et pourquoi `vague` s'en tient à la première
    ///
    /// Une onde purement plane éclaire d'un seul bloc tout ce qui se trouve
    /// dans un même plan d'égale phase. Or **six ventilateurs sur dix sont
    /// couchés** : les vingt-quatre LED du plancher sont exactement à la même
    /// hauteur, et une onde `bas-haut` les allume ensemble, d'une seule
    /// couleur. Constaté à l'œil le 2026-07-31.
    ///
    /// Ce n'est pas un défaut de calcul, c'est la géométrie du boîtier : une
    /// onde plane ne peut pas dégrader ce qui n'a aucune épaisseur dans sa
    /// direction. Le `flux` prolonge la géométrie là où elle ne dit plus
    /// rien — voir [`Geometrie::traversee`], et noter qu'il **coïncide** avec
    /// la position réelle sur un ventilateur que la direction n'aplatit pas.
    ///
    /// **`vague` s'en tient au `spatiale`**, seule du catalogue : elle est
    /// l'onde plane, et la démonstration que le boîtier et la RAM sont
    /// synchronisés dans l'espace. Les quatre autres familles dirigées suivent
    /// l'écoulement.
    ///
    /// ⚠️ **Les cinq familles restantes ne lisent ni l'une ni l'autre**, et
    /// c'est ce qui les rend indifférentes à la direction. `braise` les a
    /// rejointes en #119 : elle lisait `flux`, donc elle défilait le long de
    /// l'axe demandé, et un lit de braises n'a pas d'axe.
    fn peindre(&self, reglages: &Reglages, ou: &Place) -> Rgb {
        let Place {
            spatiale,
            flux,
            temps,
            angle,
            rayon,
            graine,
            derive,
            mesure,
        } = *ou;
        let place = fraction(flux);

        match self.famille {
            // Une sinusoïde le long de la direction, et rien d'autre : le seul
            // motif du lot dont la couleur ne dépende que de la projection.
            Famille::Vague => teinter(reglages.couleur, (1.0 + cycle(spatiale - temps)) / 2.0),

            // Une tête vive suivie d'une traînée qui s'éteint. Le reste est
            // noir, ce que le cache de cibles inchangées du démon apprécie.
            Famille::Comete => {
                let recul = fraction(place - temps);
                let traineee = 0.25;
                if recul >= traineee {
                    Rgb::BLACK
                } else {
                    teinter(reglages.couleur, 1.0 - recul / traineee)
                }
            }

            // Le boîtier respire, et la respiration se propage : sans ce léger
            // retard, la direction n'aurait aucun effet et le réglage mentirait.
            Famille::Respiration => {
                let onde = (1.0 + cycle(temps - 0.2 * place)) / 2.0;
                teinter(reglages.couleur, 0.15 + 0.85 * onde)
            }

            // Le spectre déroulé le long de la direction. Seule famille à ne pas
            // accepter de couleur : elle les produit toutes.
            //
            // ⚠️ `place - temps`, et non `place + temps`. Avec le plus, une
            // teinte donnée se trouvait en `place = h - temps`, qui **décroît**
            // avec le temps : le spectre remontait la direction demandée pendant
            // que les cinq autres familles la descendaient. Le défaut se voyait
            // mal parce qu'un arc-en-ciel n'a ni tête ni traîne — c'est en
            // comparant les familles entre elles qu'il apparaît (issue #49).
            Famille::ArcEnCiel => teinte_vers_rgb(fraction(place - temps)),

            // Une bande nette, pour qui préfère voir la limite bouger plutôt
            // qu'un dégradé.
            Famille::Balayage => {
                let recul = fraction(place - temps);
                if recul < 0.15 {
                    reglages.couleur
                } else {
                    Rgb::BLACK
                }
            }

            // Un lit de braises : des zones chaudes et froides qui respirent et
            // se déplacent lentement, sans qu'aucun axe ne se lise.
            //
            // Deux ondes portées par les deux seules grandeurs spatiales que la
            // direction demandée n'atteint pas — le `rayon`, distance à la
            // pompe, et l'`angle` de la LED sur son organe. **Aucune des deux
            // n'est un axe du boîtier** : la première est une distance, qui
            // vaut autant dans les six sens ; la seconde tourne sur place et ne
            // sort jamais de son anneau. Il n'existe donc aucune direction où
            // ce motif défilerait, et c'est pourquoi `braise` n'en accepte plus
            // (#119) : il n'y aurait rien à en faire.
            //
            // L'onde **lente** respire depuis la pompe et porte les sept
            // dixièmes de l'amplitude ; l'onde **vive** tourne en sens inverse
            // sur chaque anneau et porte le reste. Leurs cadences sont
            // incommensurables, si bien qu'aucune des deux ne ramène jamais
            // l'autre à son point de départ.
            //
            // ⚠️ **Sans axe, mais pas LED par LED.** Les deux ondes sont lentes
            // dans l'espace, si bien que deux LED voisines restent liées : c'est
            // ce qui sépare un feu du grésillement, et le grésillement est déjà
            // `scintillement`. Le frémissement tiré de la `graine` ne rompt que
            // la régularité résiduelle des deux ondes — voir [`FREMISSEMENT`].
            //
            // ⚠️ **Ni le seul rayon**, sans quoi deux LED à égale distance de la
            // pompe s'allumeraient toujours ensemble : le boîtier porterait des
            // coquilles concentriques, et ce serait `pouls`. C'est l'`angle`
            // qui les sépare.
            //
            // ⚠️ **`derive` et non `temps`.** L'ancien commentaire promettait
            // « deux ondes de périodes incommensurables : l'œil n'y voit pas de
            // cycle ». C'était vrai **dans le temps d'un cycle** — 3 et 7 ne se
            // referment pas l'une sur l'autre —, muet sur l'**espace**, qui est
            // justement ce qu'on voit, et faux d'un cycle au suivant : `temps`
            // se replie sur [`PERIODE`] et ne prend que quarante valeurs à la
            // vitesse 3. [`derive`], elle, ne se replie pas sur le cycle.
            //
            // Aucun `rand`, aucune horloge, aucun état : le frémissement est un
            // hachage du numéro de LED, et l'aperçu de la fenêtre montre donc
            // exactement ce que le boîtier reçoit.
            Famille::Braise => {
                let fremissement = FREMISSEMENT * (melange(graine) % 4096) as f32 / 4096.0;
                let lente = cycle(3.0 * derive + 3.0 * rayon + fremissement);
                let vive = cycle(-7.0 * derive + 5.0 * rayon - angle + fremissement);
                let intensite = 0.5 + 0.35 * lente + 0.15 * vive;
                teinter(reglages.couleur, intensite.clamp(0.0, 1.0))
            }

            // Chaque anneau tourne **sur lui-même**, à sa place dans le boîtier.
            //
            // ⚠️ C'est l'`angle` relevé qui compte, jamais le numéro de LED :
            // deux ventilateurs montés d'un quart de tour l'un de l'autre
            // doivent tourner décalés d'autant, et les trois du plafond sont
            // montés antihoraire. Colorer par le numéro d'ordre ferait tourner
            // le motif à l'envers sur eux, sans un message.
            //
            // Le motif est le même que `comete` — tête vive et traînée —, ce qui
            // rend la rotation lisible : un dégradé plein tournerait sans qu'on
            // voie qu'il tourne.
            //
            // ⚠️ **La traînée fait le tour complet, sans jamais éteindre.** Une
            // comète classique laisserait la moitié de l'anneau noire, et huit
            // LED dont plusieurs valent le même noir ne portent pas huit valeurs
            // distinctes — le motif serait bien là, mais la rotation
            // illisible : on ne voit tourner que ce qui a huit nuances.
            Famille::Rotation => teinter(reglages.couleur, 1.0 - fraction(angle - temps)),

            // La couleur suit une sonde, et rien d'autre — ni le temps, ni la
            // place. À température constante le boîtier est constant : c'est
            // tout l'intérêt d'un thermomètre.
            //
            // ⚠️ **Une sonde muette pulse en blanc.** Jamais la dernière couleur
            // connue, jamais un zéro : c'est le mode de défaillance le plus
            // coûteux du projet parce qu'il est rassurant — « un 34 °C figé
            // derrière une pompe arrêtée, c'est un circuit qui chauffe sans que
            // rien ne le signale ». Le blanc est achromatique, ce qu'aucune
            // étape du gradient n'est ; et il pulse, ce qu'aucune température ne
            // fait.
            Famille::Thermique => match mesure {
                Some(degres) => gradient_thermique(degres),
                None => {
                    let onde = (1.0 + cycle(temps)) / 2.0;
                    let niveau = (BLANC_MIN + (255.0 - BLANC_MIN) * onde) as u8;
                    Rgb::new(niveau, niveau, niveau)
                }
            },

            // Une onde **sphérique** née à la pompe. Deux LED à égale distance
            // d'elle s'allument ensemble, quels que soient leur organe et leur
            // axe — c'est ce qui la distingue des six familles, qui projettent
            // toutes sur un axe du boîtier.
            Famille::Pouls => {
                let front = fraction(rayon * ETALEMENT - temps);
                let epaisseur = 0.3;
                if front >= epaisseur {
                    Rgb::BLACK
                } else {
                    teinter(reglages.couleur, 1.0 - front / epaisseur)
                }
            }

            // Des LED s'allument au hasard — un hasard **déterministe**, tiré du
            // numéro de pas et de celui de la LED.
            //
            // ⚠️ Pas de `rand`, pas d'horloge, pas d'état : `image` est une
            // fonction pure, et c'est ce qui rend vraie la promesse du README —
            // « l'aperçu montre ce que le boîtier reçoit ». Une source d'aléa
            // non reproductible la rendrait fausse sans le dire, la fenêtre et
            // le démon tirant chacun la leur.
            //
            // ⚠️ **Chaque LED pulse à sa propre cadence et sa propre phase**,
            // plutôt qu'un tirage renouvelé par paliers. Un tirage par paliers
            // rendait deux pas voisins identiques — le boîtier sautait d'un
            // motif à l'autre au lieu de scintiller —, et surtout il ne
            // changeait qu'une fois sur dix images. Ici tout bouge à chaque pas,
            // mais chaque LED lentement : c'est ce que l'œil lit comme un
            // scintillement.
            Famille::Scintillement => {
                let phase = (melange(graine) % 4096) as f32 / 4096.0;
                let cadence = 2.0 + (melange(graine ^ 0x9e37_79b9) % 5) as f32;
                let onde = (1.0 + cycle(cadence * derive + phase)) / 2.0;
                // Au-dessus du seuil seulement : un tiers du boîtier allumé à
                // un instant donné, ce qui laisse voir le motif sans le noyer.
                if onde < SEUIL_SCINTILLEMENT {
                    Rgb::BLACK
                } else {
                    teinter(
                        reglages.couleur,
                        (onde - SEUIL_SCINTILLEMENT) / (1.0 - SEUIL_SCINTILLEMENT),
                    )
                }
            }
        }
    }
}

/// Où se trouve une LED, et ce que l'instant lui apporte.
///
/// Une structure et non sept arguments : le catalogue s'étend, et une liste
/// d'arguments positionnels de cette longueur s'échange sans qu'aucun
/// compilateur ne s'en aperçoive.
struct Place {
    /// La position réelle le long de la direction demandée, entre 0 et 1.
    spatiale: f32,
    /// La place du ventilateur, plus la traversée de la LED depuis le point
    /// d'entrée de l'écoulement.
    flux: f32,
    /// L'instant du cycle, entre 0 et 1.
    temps: f32,
    /// L'angle de la LED sur son anneau, entre 0 et 1, **d'après le montage
    /// relevé**. Pour une barrette, sa place bords↔centre.
    angle: f32,
    /// La distance à la pompe, rapportée à l'étendue du boîtier.
    rayon: f32,
    /// Un numéro stable et unique de LED, de 0 à 123. Sert d'aléa reproductible.
    graine: u32,
    /// Une horloge qui **ne se replie pas sur le cycle**, entre 0 et 1.
    ///
    /// ⚠️ Le [`Place::temps`] des neuf autres familles boucle sur [`PERIODE`],
    /// et à la vitesse 3 il ne prend que quarante valeurs — donc au plus quarante
    /// images. C'est ce qu'on veut d'un motif périodique, et l'inverse de ce
    /// qu'on veut d'un scintillement, qui est justement la famille sans cycle.
    derive: f32,
    /// La valeur de la sonde suivie, en degrés, ou `None` si elle est muette.
    mesure: Option<f32>,
}

/// Intensité la plus basse du blanc pulsant d'une sonde muette.
///
/// Jamais zéro : un boîtier qui s'éteint périodiquement se lit comme une panne
/// d'alimentation, pas comme une sonde qui ne répond plus.
const BLANC_MIN: f32 = 90.0;

/// Nombre de fronts sphériques simultanés dans le boîtier, pour `pouls`.
///
/// Deux : un qui part, un qui achève sa course. Un seul rendrait le boîtier noir
/// la plupart du temps, quatre en feraient des anneaux concentriques illisibles.
const ETALEMENT: f32 = 2.0;

/// Part de cycle dont la `graine` décale une LED de la braise.
///
/// Un quinzième. Assez pour rompre la régularité des deux ondes — sans lui, un
/// anneau entier passerait par la même phase à un dixième près et le motif se
/// lirait —, assez peu pour que deux LED voisines restent liées : c'est cette
/// borne qui sépare un feu du grésillement de `scintillement`, dont chaque LED
/// tire une phase sur un cycle entier.
const FREMISSEMENT: f32 = 1.0 / 15.0;

/// Au-dessus de ce seuil, une LED du scintillement s'allume.
///
/// Deux tiers : sur une sinusoïde, cela laisse environ un tiers des LED
/// allumées à un instant donné. Assez pour voir un motif, assez peu pour que ce
/// soit encore un scintillement.
const SEUIL_SCINTILLEMENT: f32 = 0.66;

/// La plage du gradient thermique, en degrés.
///
/// De la température d'un liquide au repos à celle d'une charge soutenue. Au-delà
/// des bornes, le gradient sature — un boîtier ne peut pas être « plus rouge que
/// rouge », et écrêter est ici la lecture juste.
const THERMIQUE_MIN: f32 = 25.0;
const THERMIQUE_MAX: f32 = 60.0;

/// Bleu → vert → orange → rouge, sur [`THERMIQUE_MIN`]..[`THERMIQUE_MAX`].
///
/// ⚠️ **Aucune étape n'est achromatique**, et c'est ce qui rend le blanc pulsant
/// d'une sonde muette impossible à confondre avec une température.
fn gradient_thermique(degres: f32) -> Rgb {
    const ETAPES: [(u8, u8, u8); 4] = [
        (0x00, 0x40, 0xff), // bleu
        (0x00, 0xd0, 0x40), // vert
        (0xff, 0x90, 0x00), // orange
        (0xff, 0x10, 0x00), // rouge
    ];
    let place = ((degres - THERMIQUE_MIN) / (THERMIQUE_MAX - THERMIQUE_MIN)).clamp(0.0, 1.0);
    let derniere = ETAPES.len() - 1;
    let echelle = place * derniere as f32;
    let bas = (echelle.floor() as usize).min(derniere);
    let haut = (bas + 1).min(derniere);
    let part = echelle - bas as f32;
    let melanger =
        |de: u8, vers: u8| (f32::from(de) + (f32::from(vers) - f32::from(de)) * part) as u8;
    Rgb::new(
        melanger(ETAPES[bas].0, ETAPES[haut].0),
        melanger(ETAPES[bas].1, ETAPES[haut].1),
        melanger(ETAPES[bas].2, ETAPES[haut].2),
    )
}

/// Un brassage d'entier, pour un aléa reproductible sans dépendance.
///
/// Le finaliseur de `MurmurHash3` : quatre opérations, aucune allocation, et une
/// dispersion suffisante pour que deux LED voisines ne s'allument pas ensemble.
const fn melange(graine: u32) -> u32 {
    let mut valeur = graine ^ (graine >> 16);
    valeur = valeur.wrapping_mul(0x85eb_ca6b);
    valeur ^= valeur >> 13;
    valeur = valeur.wrapping_mul(0xc2b2_ae35);
    valeur ^ (valeur >> 16)
}

/// La distance entre deux points du boîtier, en millimètres.
fn distance(un: Point, autre: Point) -> f32 {
    let dx = un.x - autre.x;
    let dy = un.y - autre.y;
    let dz = un.z - autre.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// L'instant du cycle, entre 0 et 1.
///
/// En `u64` : au pas `u32::MAX` et à la vitesse 10, le produit déborderait un
/// `u32` — et paniquerait en debug, donc dans les tests, donc jamais chez
/// l'utilisateur, ce qui est la pire façon de découvrir un défaut.
fn temps(pas: u32, vitesse: u8) -> f32 {
    let avance = (u64::from(pas) * u64::from(vitesse)) % PERIODE;
    avance as f32 / PERIODE as f32
}

/// L'horloge du scintillement, entre 0 et 1 — **sans repli sur le cycle**.
///
/// [`PERIODE_DERIVE`] est premier, donc étranger à toutes les vitesses de 1 à
/// 10 : deux pas voisins donnent toujours deux valeurs différentes, quelle que
/// soit la vitesse. C'est ce qui distingue un scintillement d'un clignotement.
///
/// Bornée plutôt que croissante indéfiniment : un `f32` n'a que vingt-quatre
/// bits de mantisse, et une horloge qui grandit sans fin finirait par se figer
/// faute de précision — au bout de quelques jours, sans que rien ne le dise.
fn derive(pas: u32, vitesse: u8) -> f32 {
    let avance = (u64::from(pas) * u64::from(vitesse)) % PERIODE_DERIVE;
    avance as f32 / PERIODE_DERIVE as f32
}

/// La partie fractionnaire, toujours positive.
fn fraction(valeur: f32) -> f32 {
    valeur.rem_euclid(1.0)
}

/// Un cosinus de période 1, entre -1 et 1.
fn cycle(valeur: f32) -> f32 {
    (valeur * std::f32::consts::TAU).cos()
}

/// Où se trouve un point le long de la direction demandée, entre 0 et 1.
///
/// La convention est **zéro au départ** : un motif avance des projections
/// faibles vers les fortes, donc `bas-haut` rend zéro au plancher.
///
/// ⚠️ **La paire de profondeur y déroge, et c'est voulu.** `avant-arriere` rend
/// zéro au **grand** `z`. Nico a jugé la paire inversée à l'œil le 2026-08-02
/// (issue #49), sur une comète — et le repère du boîtier, lui, n'est pas en
/// cause : `docs/GEOMETRIE.md` et `spec_disposition.rs` le tiennent depuis le
/// schéma du 2026-08-01, où le radiateur est bien devant les deux rangées
/// couchées, et la RAM entre les deux comme sur une carte mère réelle. C'est
/// donc l'attache des deux noms à leurs extrémités qui était fautive, pas la
/// donnée : la corriger ici laisse le relevé physique et la maquette intacts.
fn projection(direction: Direction, point: Point, bornes: (Point, Point)) -> f32 {
    let (bas, haut) = bornes;
    let rapport = |valeur: f32, min: f32, max: f32| {
        if max <= min {
            0.5
        } else {
            ((valeur - min) / (max - min)).clamp(0.0, 1.0)
        }
    };
    match direction {
        Direction::BasHaut => rapport(point.y, bas.y, haut.y),
        Direction::HautBas => 1.0 - rapport(point.y, bas.y, haut.y),
        Direction::AvantArriere => 1.0 - rapport(point.z, bas.z, haut.z),
        Direction::ArriereAvant => rapport(point.z, bas.z, haut.z),
        Direction::Horaire => angle_du_tour(point, bornes),
        Direction::Antihoraire => 1.0 - angle_du_tour(point, bornes),
        // ⚠️ **Une direction locale n'a pas de projection dans le boîtier**, et
        // lui en donner une serait exactement le défaut que l'issue #75 nomme :
        // « une direction locale silencieusement traitée comme globale donnerait
        // une image plausible et fausse ». L'appelant doit passer par
        // [`place_locale`] ; ce bras existe pour que le compilateur reste
        // exhaustif, et rend le milieu, qui n'avantage aucun objet.
        Direction::BordsCentre | Direction::CentreBords => 0.5,
    }
}

/// L'angle d'un point sur le tour du boîtier, entre 0 et 1.
///
/// La rotation se fait dans le plan **vitre / hauteur**, donc autour de l'axe
/// avant-arrière : c'est le seul cercle que ce boîtier contienne réellement,
/// puisque ses parois occupées sont le plancher, le flanc de la carte mère et
/// le plafond.
fn angle_du_tour(point: Point, (bas, haut): (Point, Point)) -> f32 {
    let centre_x = (bas.x + haut.x) / 2.0;
    let centre_y = (bas.y + haut.y) / 2.0;
    let angle = (point.x - centre_x).atan2(point.y - centre_y);
    fraction(angle / std::f32::consts::TAU + 0.5)
}

/// Une couleur assombrie proportionnellement.
///
/// Proportionnellement, et non par une courbe : l'**ordre** des composantes
/// survit à l'assombrissement, donc la teinte demandée reste reconnaissable
/// jusqu'au bas du dégradé.
fn teinter(couleur: Rgb, intensite: f32) -> Rgb {
    let facteur = intensite.clamp(0.0, 1.0);
    let composante = |valeur: u8| (f32::from(valeur) * facteur) as u8;
    Rgb::new(
        composante(couleur.r),
        composante(couleur.g),
        composante(couleur.b),
    )
}

/// Une teinte du cercle chromatique, à saturation et valeur pleines.
fn teinte_vers_rgb(teinte: f32) -> Rgb {
    let secteur = fraction(teinte) * 6.0;
    let rang = secteur as u32 % 6;
    let montee = secteur - secteur.floor();
    let plein = 255.0;
    let croissant = (montee * plein) as u8;
    let decroissant = ((1.0 - montee) * plein) as u8;
    match rang {
        0 => Rgb::new(255, croissant, 0),
        1 => Rgb::new(decroissant, 255, 0),
        2 => Rgb::new(0, 255, croissant),
        3 => Rgb::new(0, decroissant, 255),
        4 => Rgb::new(croissant, 0, 255),
        _ => Rgb::new(255, 0, decroissant),
    }
}

/// L'inverse de [`couleur`], six chiffres hexadécimaux.
fn couleur_hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

/// Décode six chiffres hexadécimaux.
///
/// Exactement six : `ff00ff00` tronqué à `ff00ff` donnerait une couleur juste
/// par accident, et `ff00` complété par du noir une couleur fausse en silence.
fn couleur(brut: &str) -> Result<Rgb, String> {
    if brut.len() != 6 || !brut.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "« {brut} » n'est pas une couleur : il en faut six chiffres hexadécimaux, par exemple \
             ff00ff"
        ));
    }
    let composante = |debut: usize| u8::from_str_radix(&brut[debut..debut + 2], 16);
    match (composante(0), composante(2), composante(4)) {
        (Ok(r), Ok(g), Ok(b)) => Ok(Rgb::new(r, g, b)),
        _ => Err(format!("« {brut} » n'est pas une couleur")),
    }
}

/// Décode une vitesse, bornée.
///
/// La borne haute n'est pas une politesse : `u8::from_str` accepte 255, et une
/// animation qui avance de 255 pas par image ne se distingue plus du bruit.
fn vitesse(brut: &str) -> Result<u8, String> {
    let valeur: u8 = brut.parse().map_err(|_| {
        format!("« {brut} » n'est pas une vitesse : un entier de {VITESSE_MIN} à {VITESSE_MAX}")
    })?;
    if !(VITESSE_MIN..=VITESSE_MAX).contains(&valeur) {
        return Err(format!(
            "« {brut} » est hors bornes : la vitesse va de {VITESSE_MIN} à {VITESSE_MAX}"
        ));
    }
    Ok(valeur)
}
