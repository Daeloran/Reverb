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
//! soient leur ventilateur et leur numéro d'ordre. Les cinq autres suivent
//! l'**écoulement**, qui traverse chaque ventilateur d'un bord à l'autre même
//! quand la direction l'aplatit — voir [`Geometrie::traversee`].

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
}

impl Direction {
    /// Les six directions, dans un ordre stable.
    pub const ALL: [Direction; 6] = [
        Direction::BasHaut,
        Direction::HautBas,
        Direction::AvantArriere,
        Direction::ArriereAvant,
        Direction::Horaire,
        Direction::Antihoraire,
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
        }
    }

    /// L'inverse de [`Direction::slug`], strict.
    fn depuis_slug(brut: &str) -> Option<Direction> {
        Direction::ALL.into_iter().find(|d| d.slug() == brut)
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

/// Les réglages d'une animation.
///
/// Un seul jeu de défauts pour tout le catalogue : une animation qui n'accepte
/// pas `couleur` laisse simplement le champ tranquille.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reglages {
    pub couleur: Rgb,
    pub vitesse: u8,
    pub direction: Direction,
}

impl Default for Reglages {
    fn default() -> Reglages {
        Reglages {
            couleur: Rgb::new(0xff, 0x40, 0xff),
            vitesse: 3,
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
}

/// Nom et famille, dans l'ordre du [`CATALOGUE`].
const FAMILLES: [(&str, Famille); 6] = [
    ("vague", Famille::Vague),
    ("comete", Famille::Comete),
    ("respiration", Famille::Respiration),
    ("arc-en-ciel", Famille::ArcEnCiel),
    ("balayage", Famille::Balayage),
    ("braise", Famille::Braise),
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
];

/// Les clés acceptées par une animation qui se colore.
const AVEC_COULEUR: &[&str] = &["couleur", "vitesse", "direction"];
/// Les clés acceptées par une animation qui produit ses propres teintes.
const SANS_COULEUR: &[&str] = &["vitesse", "direction"];

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
            _ => AVEC_COULEUR,
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
        } = *reglages;
        let tous = [
            ("couleur", couleur_hexa(couleur)),
            ("vitesse", vitesse.to_string()),
            ("direction", direction.slug().to_owned()),
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
                autre => unreachable!("« {autre} » est déclarée acceptée sans être décodée"),
            }
        }
        Ok(reglages)
    }

    /// L'image du pas donné. Fonction pure : mêmes entrées, même sortie.
    pub fn image(&self, geometrie: &Geometrie, reglages: &Reglages, pas: u32) -> Image {
        let bornes = geometrie.bornes();
        let temps = temps(pas, reglages.vitesse);

        let mut ventilateurs = [(Position::BasGauche, [Rgb::BLACK; LEDS_PER_FAN as usize]); 10];
        for (place, position) in ventilateurs.iter_mut().zip(Position::ALL) {
            // Le ventilateur entier occupe une place dans le motif ; chacune de
            // ses LED occupe une fraction de cette place, selon où elle se
            // trouve dans sa traversée.
            let du_ventilateur = projection(
                reglages.direction,
                geometrie.centre_ventilateur(position),
                bornes,
            );
            let mut couleurs = [Rgb::BLACK; LEDS_PER_FAN as usize];
            for (led, couleur) in couleurs.iter_mut().enumerate() {
                if let Some(point) = geometrie.led_ventilateur(position, led) {
                    let spatiale = projection(reglages.direction, point, bornes);
                    let flux =
                        du_ventilateur + EMPRISE * (geometrie.traversee(position, led) - 0.5);
                    *couleur = self.peindre(reglages, spatiale, flux, temps);
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
                    let spatiale = projection(reglages.direction, point, bornes);
                    *couleur = self.peindre(reglages, spatiale, spatiale, temps);
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
    /// synchronisés dans l'espace. Les cinq autres suivent l'écoulement.
    fn peindre(&self, reglages: &Reglages, spatiale: f32, flux: f32, temps: f32) -> Rgb {
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
            Famille::ArcEnCiel => teinte_vers_rgb(fraction(place + temps)),

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

            // Deux ondes de périodes incommensurables : l'œil n'y voit pas de
            // cycle, sans qu'aucun hasard n'entre dans un rendu qui doit rester
            // reproductible à l'identique dans la fenêtre et dans le démon.
            Famille::Braise => {
                let lente = cycle(3.0 * temps - place);
                let vive = cycle(7.0 * temps + 3.0 * place);
                let intensite = 0.5 + 0.3 * lente + 0.2 * vive;
                teinter(reglages.couleur, intensite.clamp(0.0, 1.0))
            }
        }
    }
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

/// La partie fractionnaire, toujours positive.
fn fraction(valeur: f32) -> f32 {
    valeur.rem_euclid(1.0)
}

/// Un cosinus de période 1, entre -1 et 1.
fn cycle(valeur: f32) -> f32 {
    (valeur * std::f32::consts::TAU).cos()
}

/// Où se trouve un point le long de la direction demandée, entre 0 et 1.
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
        Direction::AvantArriere => rapport(point.z, bas.z, haut.z),
        Direction::ArriereAvant => 1.0 - rapport(point.z, bas.z, haut.z),
        Direction::Horaire => angle_du_tour(point, bornes),
        Direction::Antihoraire => 1.0 - angle_du_tour(point, bornes),
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
