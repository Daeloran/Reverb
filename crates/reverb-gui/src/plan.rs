//! Le boîtier vu depuis le panneau latéral gauche, face à la carte mère.
//!
//! C'est le point de vue de l'utilisateur : celui depuis lequel il « lit » ses
//! ventilateurs, et celui dans lequel la géométrie a été relevée (`docs/
//! GEOMETRIE.md`). L'**arrière du boîtier est donc à gauche** de l'écran,
//! l'avant à droite, le haut en haut.
//!
//! # C'est un schéma, pas une photographie
//!
//! Vus depuis ce panneau, seuls les trois ventilateurs du radiateur présentent
//! leur disque : les six couchés et celui de l'arrière sont vus **par la
//! tranche**. Un dessin fidèle en ferait sept traits, où aucune LED ne serait
//! cliquable. Ils sont donc dessinés **en cercles quand même**, à leur place
//! dans ce repère. Une vue dont le but est de ne plus regarder sous le bureau
//! doit montrer les cent vingt-quatre LED, pas sept traits.
//!
//! Deuxième écart assumé, pour la même raison : les barrettes et la colonne du
//! radiateur sont à des **profondeurs différentes** qu'une vue de face
//! confondrait — la RAM se dresse devant la carte mère, le radiateur est plaqué
//! contre elle. Physiquement, la RAM masque une partie du radiateur ; sur le
//! plan, elle est décalée pour que les deux restent cliquables.
//!
//! # Deux écarts que la mesure impose, et non le goût
//!
//! Ils ne sont pas négociables, ils se calculent :
//!
//! - **Les anneaux sont dessinés plus petits que nature.** `bas-milieu` et
//!   `radiateur-bas` n'ont que 70 mm entre leurs centres, pour un rayon
//!   physique de 55 : à l'échelle, deux ventilateurs voisins se chevauchent de
//!   40 mm. Le rayon dessiné doit rester sous `70 / 2,25 ≈ 31` pour que deux
//!   anneaux et le diamètre d'une LED tiennent entre deux centres.
//! - **La RAM est décalée vers l'arrière.** Sa profondeur réelle la met dans la
//!   colonne du radiateur ; le décalage la ramène dans le vide qui sépare cette
//!   colonne du ventilateur arrière — là où l'utilisateur la décrit lui-même,
//!   « entre les ventilateurs de devant et celui de l'arrière ».
//!
//! Les barrettes sont aussi dessinées **plus grandes que nature** : onze LED
//! espacées de 4 mm ne se cliquent pas. Une réglette lisible est le seul moyen
//! d'atteindre les quarante-quatre LED de la RAM.

use reverb_anim::Geometrie;
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position};

/// Nombre de LED d'un ventilateur, comme indice.
const LEDS: usize = LEDS_PER_FAN as usize;

/// Rayon d'un anneau **dessiné**, en millimètres du boîtier.
///
/// Sous les 31 mm que la mesure impose (voir l'en-tête), avec de quoi respirer.
const RAYON_DESSINE: f32 = 28.0;

/// Diamètre d'une LED dessinée.
///
/// Le quart du rayon : huit LED réparties sur un anneau sont à `0,765 · R`
/// l'une de l'autre, donc un disque de `R/4` laisse entre deux voisines deux
/// fois son propre diamètre — visiblement séparées, et visables à la souris.
const DIAMETRE_LED: f32 = RAYON_DESSINE / 4.0;

/// Où le bloc de RAM est posé, en profondeur.
///
/// Entre la colonne du radiateur (210) et le ventilateur arrière (420), dans le
/// vide que rien n'occupe.
const RAM_PROFONDEUR: f32 = 300.0;

/// Hauteur du milieu du bloc de RAM.
///
/// Entre le plancher et le plafond, un peu plus près du haut — comme relevé.
const RAM_HAUTEUR: f32 = 240.0;

/// Écart entre deux LED d'une même barrette, sur le plan.
const RAM_PAS_LED: f32 = 1.5 * DIAMETRE_LED;

/// Écart entre deux barrettes, sur le plan.
const RAM_PAS_SLOT: f32 = 2.5 * DIAMETRE_LED;

/// Marge laissée entre la maquette et le bord du cadre.
const MARGE: f32 = 0.01;

/// Un point de la maquette, en coordonnées d'écran.
///
/// Normalisées de 0 à 1, `y` vers le bas — la convention de l'écran, pas celle
/// du boîtier. C'est la fenêtre qui les multiplie par sa taille du moment : un
/// plan en pixels serait faux dès qu'on redimensionne.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub x: f32,
    pub y: f32,
}

/// Ce qu'on désigne en cliquant sur la maquette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cible {
    Led { position: Position, led: usize },
    Barrette { slot: usize, led: usize },
}

/// Où dessiner chaque LED du boîtier.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    ventilateurs: [(Place, [Place; LEDS]); 10],
    barrettes: [[Place; LEDS_PER_STICK]; SLOT_COUNT],
    rayon: f32,
}

impl Plan {
    /// Projette une géométrie dans le repère « panneau gauche ».
    ///
    /// La géométrie donne des points en trois dimensions ; le plan n'en garde
    /// que deux. La profondeur — l'axe qui va du panneau vers la carte mère —
    /// n'est pas dessinée : elle ne distingue rien que l'utilisateur cherche.
    pub fn nouveau(geometrie: &Geometrie) -> Plan {
        // Les centres et les barrettes, encore en millimètres du boîtier :
        // (profondeur, hauteur). La profondeur croît vers l'arrière, donc vers
        // la gauche de l'écran ; la hauteur croît vers le haut.
        let centres: Vec<(f32, f32)> = Position::ALL
            .into_iter()
            .map(|position| {
                let point = geometrie.centre_ventilateur(position);
                (point.z, point.y)
            })
            .collect();
        let barrettes: Vec<Vec<(f32, f32)>> = (0..SLOT_COUNT)
            .map(|slot| {
                (0..LEDS_PER_STICK)
                    .map(|led| (profondeur_barrette(slot), hauteur_barrette(led)))
                    .collect()
            })
            .collect();

        // Le cadre englobe les anneaux, pas seulement les centres : un
        // ventilateur dont l'anneau déborde se dessine tronqué, et les LED qui
        // manquent sont celles qu'on ne pourra pas cliquer.
        let mut bornes = Bornes::vide();
        for (z, y) in &centres {
            bornes.ajouter(z - RAYON_DESSINE, y - RAYON_DESSINE);
            bornes.ajouter(z + RAYON_DESSINE, y + RAYON_DESSINE);
        }
        for barrette in &barrettes {
            for (z, y) in barrette {
                bornes.ajouter(*z, *y);
            }
        }

        // Échelle **uniforme** : un ventilateur rond doit rester rond. Une
        // échelle libre étirerait les anneaux en ellipses, et la maquette
        // mentirait sur ce qu'on regarde.
        let utile = 1.0 - 2.0 * MARGE;
        let echelle = utile / bornes.largeur().max(bornes.hauteur());
        let decalage_x = (utile - bornes.largeur() * echelle) / 2.0;
        let decalage_y = (utile - bornes.hauteur() * echelle) / 2.0;
        let place = |z: f32, y: f32| Place {
            x: MARGE + decalage_x + (bornes.z_max - z) * echelle,
            y: MARGE + decalage_y + (bornes.y_max - y) * echelle,
        };

        let rayon = RAYON_DESSINE * echelle;
        let mut dessin = [(Place { x: 0.0, y: 0.0 }, [Place { x: 0.0, y: 0.0 }; LEDS]); 10];
        for (position, (z, y)) in Position::ALL.into_iter().zip(&centres) {
            let centre = place(*z, *y);
            let orientation = geometrie.orientation(position);
            let mut anneau = [centre; LEDS];
            for (led, point) in anneau.iter_mut().enumerate() {
                // L'angle est celui du relevé : zéro à midi, croissant dans le
                // sens où l'anneau tourne **vu de l'extérieur du boîtier** —
                // c'est-à-dire vu comme on le regarde quand on le mesure.
                let angle = f32::from(orientation.angle_led(led)).to_radians();
                *point = Place {
                    x: centre.x + rayon * angle.sin(),
                    y: centre.y - rayon * angle.cos(),
                };
            }
            dessin[position.index()] = (centre, anneau);
        }

        let mut reglettes = [[Place { x: 0.0, y: 0.0 }; LEDS_PER_STICK]; SLOT_COUNT];
        for (slot, barrette) in barrettes.iter().enumerate() {
            for (led, (z, y)) in barrette.iter().enumerate() {
                reglettes[slot][led] = place(*z, *y);
            }
        }

        Plan {
            ventilateurs: dessin,
            barrettes: reglettes,
            rayon,
        }
    }

    /// Le centre d'un ventilateur.
    pub fn centre_ventilateur(&self, position: Position) -> Place {
        self.ventilateurs[position.index()].0
    }

    /// Une des huit LED d'un ventilateur, `None` au-delà.
    pub fn led_ventilateur(&self, position: Position, led: usize) -> Option<Place> {
        self.ventilateurs[position.index()].1.get(led).copied()
    }

    /// Une des onze LED d'une barrette, `None` au-delà.
    pub fn led_barrette(&self, slot: usize, led: usize) -> Option<Place> {
        self.barrettes.get(slot)?.get(led).copied()
    }

    /// Le rayon d'un anneau de ventilateur, dans la même unité que [`Place`].
    ///
    /// Deux LED voisines doivent rester distinctes à l'écran : c'est ce rayon
    /// qui décide si on peut en cliquer une.
    pub fn rayon_anneau(&self) -> f32 {
        self.rayon
    }

    /// Ce qui se trouve sous un point, s'il y a quelque chose.
    ///
    /// C'est la fonction du clic. Elle rend la LED **la plus proche** dans son
    /// rayon de prise, et `None` au-delà : cliquer dans le vide ne doit pas
    /// changer une couleur au hasard.
    ///
    /// La prise vaut un rayon d'anneau. Plus courte, l'aperçu deviendrait
    /// pénible — ce qui se voit tout de suite ; plus longue, un clic manqué
    /// changerait la couleur d'une LED qu'on ne visait pas — ce qu'on ne relie
    /// jamais au clic qui l'a causée.
    pub fn sous(&self, place: Place) -> Option<Cible> {
        // La fenêtre est plus grande que la maquette : un clic dans sa marge ne
        // doit pas se rabattre sur la LED la plus proche.
        if !(0.0..=1.0).contains(&place.x) || !(0.0..=1.0).contains(&place.y) {
            return None;
        }

        let mut meilleure: Option<(f32, Cible)> = None;
        let mut retenir = |ecart: f32, cible: Cible| {
            if ecart <= self.rayon && meilleure.is_none_or(|(pire, _)| ecart < pire) {
                meilleure = Some((ecart, cible));
            }
        };

        for position in Position::ALL {
            for (led, point) in self.ventilateurs[position.index()].1.iter().enumerate() {
                retenir(distance(place, *point), Cible::Led { position, led });
            }
        }
        for (slot, barrette) in self.barrettes.iter().enumerate() {
            for (led, point) in barrette.iter().enumerate() {
                retenir(distance(place, *point), Cible::Barrette { slot, led });
            }
        }
        meilleure.map(|(_, cible)| cible)
    }
}

/// La profondeur d'une barrette sur le plan.
///
/// Même sens que la géométrie — la profondeur décroît quand le rang croît — de
/// sorte que les quatre barrettes se suivent dans le même ordre ici et là-bas.
fn profondeur_barrette(slot: usize) -> f32 {
    let milieu = (SLOT_COUNT - 1) as f32 / 2.0;
    RAM_PROFONDEUR - (slot as f32 - milieu) * RAM_PAS_SLOT
}

/// La hauteur d'une LED de barrette : la LED 0 est **en bas**, comme sur le
/// matériel.
fn hauteur_barrette(led: usize) -> f32 {
    let milieu = (LEDS_PER_STICK - 1) as f32 / 2.0;
    RAM_HAUTEUR + (led as f32 - milieu) * RAM_PAS_LED
}

fn distance(a: Place, b: Place) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Le rectangle qu'occupe la maquette, en millimètres du boîtier.
struct Bornes {
    z_min: f32,
    z_max: f32,
    y_min: f32,
    y_max: f32,
}

impl Bornes {
    fn vide() -> Bornes {
        Bornes {
            z_min: f32::INFINITY,
            z_max: f32::NEG_INFINITY,
            y_min: f32::INFINITY,
            y_max: f32::NEG_INFINITY,
        }
    }

    fn ajouter(&mut self, z: f32, y: f32) {
        self.z_min = self.z_min.min(z);
        self.z_max = self.z_max.max(z);
        self.y_min = self.y_min.min(y);
        self.y_max = self.y_max.max(y);
    }

    fn largeur(&self) -> f32 {
        self.z_max - self.z_min
    }

    fn hauteur(&self) -> f32 {
        self.y_max - self.y_min
    }
}
