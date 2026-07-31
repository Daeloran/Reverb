//! Où se trouve physiquement chaque LED du boîtier.
//!
//! Les valeurs viennent de la mesure du 2026-07-31, consignée dans
//! `docs/GEOMETRIE.md`. Elles décrivent **une machine** et non un protocole :
//! c'est pourquoi elles se corrigent par une commande, pas par une
//! recompilation.

use std::fmt;

use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position};

/// Sens de rotation de l'anneau de LED, vu depuis l'**extérieur** du boîtier.
///
/// Le protocole ne donne que l'ordre des indices (SPEC-PROTOCOLE-NZXT §5) : le
/// sens apparent dépend de la face par laquelle on regarde le ventilateur,
/// donc du montage. La mesure l'a confirmé de la meilleure façon — le plafond
/// tourne à l'envers du plancher, parce qu'on le voit par l'autre face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sens {
    Horaire,
    Antihoraire,
}

impl Sens {
    /// Le mot qui écrit ce sens dans le fichier de géométrie et sur le socket.
    pub const fn slug(self) -> &'static str {
        match self {
            Sens::Horaire => "horaire",
            Sens::Antihoraire => "antihoraire",
        }
    }

    /// L'inverse de [`Sens::slug`], strict : ni casse ni graphie voisine.
    fn depuis_slug(brut: &str) -> Option<Sens> {
        match brut {
            "horaire" => Some(Sens::Horaire),
            "antihoraire" => Some(Sens::Antihoraire),
            _ => None,
        }
    }
}

/// Un tour complet, en degrés.
const TOUR: u16 = 360;

/// Écart angulaire entre deux LED voisines d'un ventilateur.
///
/// ✅ Mesuré : la LED 5 est apparue diamétralement opposée à la LED 1 sur les
/// dix ventilateurs, ce qui ne peut arriver que si les huit sont régulières.
const PAS_ANNEAU: u16 = TOUR / LEDS_PER_FAN as u16;

/// Où se trouve la LED 1 d'un ventilateur, et dans quel sens l'anneau tourne.
///
/// `angle` en degrés : 0 = midi, croissant dans le sens horaire vu de
/// l'extérieur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Orientation {
    pub angle: u16,
    pub sens: Sens,
}

/// Erreur rendue par [`Orientation::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrientationInvalide {
    pub champ: &'static str,
    pub raison: String,
}

impl fmt::Display for OrientationInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "champ « {} » : {}", self.champ, self.raison)
    }
}

impl std::error::Error for OrientationInvalide {}

impl Orientation {
    /// Refuse un angle hors `0..=359`.
    ///
    /// 360 est le même point que 0 : l'accepter obligerait chaque calcul en
    /// aval à s'en méfier. Le refuser une fois à l'entrée met la question
    /// derrière soi.
    pub fn new(angle: u16, sens: Sens) -> Result<Orientation, OrientationInvalide> {
        if angle >= TOUR {
            return Err(OrientationInvalide {
                champ: "angle",
                raison: format!(
                    "{angle} n'est pas un angle : le tour va de 0 à {}",
                    TOUR - 1
                ),
            });
        }
        Ok(Orientation { angle, sens })
    }

    /// Angle absolu de la LED d'indice donné, en degrés.
    ///
    /// L'indice est pris **modulo huit** : l'anneau est fermé (spec §5, la LED
    /// 8 est contiguë à la 1), donc la LED 9 *est* la LED 1 et rendre son
    /// angle est la réponse juste, pas une tolérance.
    pub fn angle_led(&self, led: usize) -> u16 {
        let rang = (led % LEDS_PER_FAN as usize) as u16;
        let ecart = PAS_ANNEAU * rang;
        match self.sens {
            Sens::Horaire => (self.angle + ecart) % TOUR,
            Sens::Antihoraire => (self.angle + TOUR - ecart) % TOUR,
        }
    }
}

/// Un point du boîtier, en millimètres.
///
/// `x` du flanc gauche vers le flanc droit, `y` du plancher vers le plafond,
/// `z` de l'avant vers l'arrière. Trois axes et non deux : le boîtier a quatre
/// plans occupés, et une projection choisie ici serait un choix d'affichage
/// gelé dans la donnée.
///
/// ⚠️ **Les valeurs absolues n'ont aucune importance.** Les animations ne
/// lisent que des rapports. L'échelle est déduite de la seule longueur connue
/// avec certitude — les ventilateurs sont des F140, donc 140 mm — pour que la
/// maquette 2D de la fenêtre ait des proportions plausibles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Le plan dans lequel tourne l'anneau d'un ventilateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plan {
    /// Couché : plancher et plafond. Toutes ses LED sont à la même hauteur —
    /// c'est ce qui permet à une onde verticale de les atteindre ensemble.
    Couche,
    /// Debout : face avant (le radiateur) et face arrière.
    Debout,
}

/// Entraxe de deux ventilateurs jointifs : leur taille.
const ENTRAXE: f32 = 140.0;

/// Rayon de l'anneau de LED dans un ventilateur de 140 mm.
///
/// 🔶 Estimé, non mesuré. Seul son rapport à l'entraxe compte, et il ne doit
/// pas dépasser la moitié de celui-ci sous peine de faire se chevaucher deux
/// ventilateurs voisins.
const RAYON: f32 = 55.0;

/// Hauteur du boîtier occupée, soit trois ventilateurs empilés sur la face avant.
const HAUTEUR: f32 = 3.0 * ENTRAXE;

/// Profondeur occupée, soit trois ventilateurs alignés au plancher.
const PROFONDEUR: f32 = 3.0 * ENTRAXE;

/// Centre et plan de chaque ventilateur, dans l'ordre de [`Position::ALL`].
///
/// « gauche » est le plus proche de l'**arrière** : c'est ce que la mesure a
/// établi, et ce que la disposition ATX recoupe.
const CENTRES: [(Point, Plan); 10] = [
    // Plancher, d'arrière en avant.
    (pt(0.0, 0.0, 350.0), Plan::Couche),
    (pt(0.0, 0.0, 210.0), Plan::Couche),
    (pt(0.0, 0.0, 70.0), Plan::Couche),
    // Face avant, le radiateur du Kraken, de haut en bas.
    (pt(0.0, 350.0, 0.0), Plan::Debout),
    (pt(0.0, 210.0, 0.0), Plan::Debout),
    (pt(0.0, 70.0, 0.0), Plan::Debout),
    // Face arrière, en haut.
    (pt(0.0, 350.0, PROFONDEUR), Plan::Debout),
    // Plafond, d'arrière en avant.
    (pt(0.0, HAUTEUR, 350.0), Plan::Couche),
    (pt(0.0, HAUTEUR, 210.0), Plan::Couche),
    (pt(0.0, HAUTEUR, 70.0), Plan::Couche),
];

/// Écart de `Point` utilisable dans une constante.
const fn pt(x: f32, y: f32, z: f32) -> Point {
    Point { x, y, z }
}

/// Décalage latéral des barrettes par rapport au plan des ventilateurs.
///
/// Non nul à dessein : il place la RAM hors du plan médian, comme elle l'est
/// réellement, et garantit qu'aucune LED de barrette ne tombe sur une LED de
/// ventilateur.
const RAM_X: f32 = 20.0;

/// Profondeur de la barrette la plus proche du CPU.
const RAM_Z: f32 = 240.0;

/// Écart entre deux slots DIMM voisins.
const RAM_PAS_SLOT: f32 = 10.0;

/// Hauteur de la LED la plus basse d'une barrette.
///
/// Entre plancher et plafond, un peu plus près du plafond — c'est ce que la
/// mesure décrit.
const RAM_Y: f32 = 240.0;

/// Écart entre deux LED voisines d'une barrette.
const RAM_PAS_LED: f32 = 4.0;

/// Orientations mesurées le 2026-07-31 sur SHYNAEL (`docs/GEOMETRIE.md`).
///
/// Dans l'ordre de [`Position::ALL`]. « bas droite » est monté à un quart de
/// tour de ses deux voisins, et le plafond tourne à l'envers du plancher.
const MESUREE: [(u16, Sens); 10] = [
    (300, Sens::Horaire),    // bas gauche
    (300, Sens::Horaire),    // bas milieu
    (210, Sens::Horaire),    // bas droite
    (210, Sens::Horaire),    // radiateur haut
    (210, Sens::Horaire),    // radiateur milieu
    (210, Sens::Horaire),    // radiateur bas
    (300, Sens::Horaire),    // arrière
    (60, Sens::Antihoraire), // haut gauche
    (60, Sens::Antihoraire), // haut milieu
    (60, Sens::Antihoraire), // haut droite
];

/// Où se trouve chaque LED, et comment chaque ventilateur est monté.
///
/// Ne porte **que** les orientations : les centres et les rayons sont les
/// mêmes pour toutes les géométries d'une même machine. Deux géométries sont
/// donc égales si et seulement si leurs dix orientations le sont, ce qui rend
/// l'aller-retour `encoder`/`decoder` exact par construction.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometrie {
    orientations: [Orientation; 10],
}

/// Erreur rendue par [`Geometrie::decoder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometrieInvalide {
    /// Numéro de ligne dans le texte, **à partir de 1** — comme un éditeur.
    /// Vaut 0 quand la faute ne tient à aucune ligne en particulier.
    pub ligne: usize,
    pub champ: &'static str,
    pub raison: String,
}

impl fmt::Display for GeometrieInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ligne == 0 {
            write!(f, "champ « {} » : {}", self.champ, self.raison)
        } else {
            write!(
                f,
                "ligne {}, champ « {} » : {}",
                self.ligne, self.champ, self.raison
            )
        }
    }
}

impl std::error::Error for GeometrieInvalide {}

impl Geometrie {
    /// La géométrie mesurée sur SHYNAEL.
    pub fn mesuree() -> Geometrie {
        let mut orientations = [Orientation {
            angle: 0,
            sens: Sens::Horaire,
        }; 10];
        for (place, (angle, sens)) in orientations.iter_mut().zip(MESUREE) {
            *place = Orientation { angle, sens };
        }
        Geometrie { orientations }
    }

    pub fn orientation(&self, position: Position) -> Orientation {
        self.orientations[position.index()]
    }

    pub fn definir(&mut self, position: Position, orientation: Orientation) {
        self.orientations[position.index()] = orientation;
    }

    /// Position d'une LED de ventilateur. `led` dans `0..8`.
    pub fn led_ventilateur(&self, position: Position, led: usize) -> Option<Point> {
        if led >= LEDS_PER_FAN as usize {
            return None;
        }
        let (centre, plan) = CENTRES[position.index()];
        let angle = f32::from(self.orientations[position.index()].angle_led(led)).to_radians();
        // Vers quoi pointe midi, selon le plan :
        //
        // - **debout** (radiateur, arrière) — vers le haut du boîtier, sans
        //   ambiguïté possible ;
        // - **couché** (plancher, plafond) — ✅ **vers le plateau de carte
        //   mère**, donc vers le flanc, et non vers l'arrière. Le plan étant
        //   horizontal, « midi » n'a de sens que rapporté à la direction depuis
        //   laquelle on l'a regardé ; la mesure a tranché.
        let (sin, cos) = (angle.sin(), angle.cos());
        Some(match plan {
            // La hauteur est **exactement** celle du centre, sans passer par un
            // cosinus : c'est ce qui fait que les vingt-quatre LED du plancher
            // partagent la même hauteur au bit près, et donc qu'une onde
            // verticale puisse les atteindre ensemble.
            Plan::Couche => Point {
                x: centre.x + RAYON * cos,
                y: centre.y,
                z: centre.z + RAYON * sin,
            },
            Plan::Debout => Point {
                x: centre.x + RAYON * sin,
                y: centre.y + RAYON * cos,
                z: centre.z,
            },
        })
    }

    /// Position d'une LED de barrette. `slot` dans `0..4`, `led` dans `0..11`.
    ///
    /// Les LED montent **de bas en haut** (SPEC-CORSAIR-RAM §3, confirmé par
    /// le dégradé), et le slot 0 est le plus proche du CPU, donc le plus à
    /// l'arrière.
    pub fn led_barrette(&self, slot: usize, led: usize) -> Option<Point> {
        if slot >= SLOT_COUNT || led >= LEDS_PER_STICK {
            return None;
        }
        Some(Point {
            x: RAM_X,
            y: RAM_Y + RAM_PAS_LED * led as f32,
            z: RAM_Z - RAM_PAS_SLOT * slot as f32,
        })
    }

    /// Coin bas-avant-gauche et coin haut-arrière-droit du volume occupé.
    ///
    /// Sert aux animations à normaliser sans coder de dimensions en dur.
    pub fn bornes(&self) -> (Point, Point) {
        let mut bas = Point {
            x: f32::INFINITY,
            y: f32::INFINITY,
            z: f32::INFINITY,
        };
        let mut haut = Point {
            x: f32::NEG_INFINITY,
            y: f32::NEG_INFINITY,
            z: f32::NEG_INFINITY,
        };
        let mut etendre = |point: Point| {
            bas.x = bas.x.min(point.x);
            bas.y = bas.y.min(point.y);
            bas.z = bas.z.min(point.z);
            haut.x = haut.x.max(point.x);
            haut.y = haut.y.max(point.y);
            haut.z = haut.z.max(point.z);
        };
        for position in Position::ALL {
            for led in 0..LEDS_PER_FAN as usize {
                if let Some(point) = self.led_ventilateur(position, led) {
                    etendre(point);
                }
            }
        }
        for slot in 0..SLOT_COUNT {
            for led in 0..LEDS_PER_STICK {
                if let Some(point) = self.led_barrette(slot, led) {
                    etendre(point);
                }
            }
        }
        (bas, haut)
    }

    /// Une ligne par ventilateur : `<position-slug> <angle> <sens>`.
    pub fn encoder(&self) -> String {
        let mut lignes = Vec::with_capacity(Position::ALL.len());
        for position in Position::ALL {
            let orientation = self.orientations[position.index()];
            lignes.push(format!(
                "{} {} {}",
                position.slug(),
                orientation.angle,
                orientation.sens.slug()
            ));
        }
        lignes.join("\n")
    }

    /// Réciproque exacte d'[`Geometrie::encoder`].
    ///
    /// Les lignes vides et celles qui commencent par `#` sont ignorées : ce
    /// fichier finit sous les yeux de quelqu'un, et un fichier de
    /// configuration qui n'accepte pas de commentaire finit par en recevoir un
    /// et par être refusé.
    ///
    /// Une position manquante ou répétée est **refusée**, pas complétée par un
    /// défaut : appliquer une valeur d'usine à la place d'une ligne supprimée
    /// ferait passer une erreur d'édition pour un choix.
    pub fn decoder(texte: &str) -> Result<Geometrie, GeometrieInvalide> {
        let mut trouvees: [Option<Orientation>; 10] = [None; 10];

        for (indice, ligne) in texte.lines().enumerate() {
            let numero = indice + 1;
            let utile = ligne.trim();
            if utile.is_empty() || utile.starts_with('#') {
                continue;
            }

            let champs: Vec<&str> = utile.split_whitespace().collect();
            let [brut_position, brut_angle, brut_sens] = champs[..] else {
                return Err(GeometrieInvalide {
                    ligne: numero,
                    champ: "position",
                    raison: format!(
                        "« {utile} » n'a pas la forme <position> <angle> <sens> : {} champ(s) au \
                         lieu de 3",
                        champs.len()
                    ),
                });
            };

            let position = Position::from_slug(brut_position).map_err(|_| GeometrieInvalide {
                ligne: numero,
                champ: "position",
                raison: format!("« {brut_position} » n'est pas un ventilateur connu"),
            })?;

            let angle: u16 = brut_angle.parse().map_err(|_| GeometrieInvalide {
                ligne: numero,
                champ: "angle",
                raison: format!("« {brut_angle} » n'est pas un nombre entier de degrés"),
            })?;

            let sens = Sens::depuis_slug(brut_sens).ok_or_else(|| GeometrieInvalide {
                ligne: numero,
                champ: "sens",
                raison: format!("« {brut_sens} » n'est ni « horaire » ni « antihoraire »"),
            })?;

            let orientation =
                Orientation::new(angle, sens).map_err(|erreur| GeometrieInvalide {
                    ligne: numero,
                    champ: erreur.champ,
                    raison: erreur.raison,
                })?;

            if trouvees[position.index()].is_some() {
                return Err(GeometrieInvalide {
                    ligne: numero,
                    champ: "position",
                    raison: format!("« {brut_position} » est décrit deux fois"),
                });
            }
            trouvees[position.index()] = Some(orientation);
        }

        let mut orientations = [Orientation {
            angle: 0,
            sens: Sens::Horaire,
        }; 10];
        for (place, (trouvee, position)) in orientations
            .iter_mut()
            .zip(trouvees.into_iter().zip(Position::ALL))
        {
            *place = trouvee.ok_or_else(|| GeometrieInvalide {
                ligne: 0,
                champ: "position",
                raison: format!("« {} » n'est décrit par aucune ligne", position.slug()),
            })?;
        }
        Ok(Geometrie { orientations })
    }
}
