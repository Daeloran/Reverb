//! Les palettes de couleur (issue #126).
//!
//! Une palette est un dégradé à arrêts : quelques couples `(position, couleur)`
//! entre lesquels on interpole. C'est de la **donnée pure** — aucune IO, aucune
//! géométrie — et elle se branche là où le catalogue employait jusqu'ici la
//! couleur unique des réglages.
//!
//! # Pourquoi ça ne coûte aucune pastille de plus
//!
//! Chaque famille calcule déjà une **position scalaire** par LED : la projection
//! sur l'axe demandé, la distance à la pompe, l'angle sur l'anneau, la traversée
//! du ventilateur. C'est exactement l'entrée d'une palette. Douze palettes
//! multiplient donc les familles existantes au lieu d'en ajouter.
//!
//! # Provenance et licence
//!
//! Les douze dégradés sont repris de **WLED** (`wled00/palettes.cpp`), qui les
//! a lui-même importés de **cpt-city** (`seaviewsensing.com`) en leur appliquant
//! sa correction gamma. Les valeurs sont recopiées **telles que WLED les
//! porte** : ce sont celles qui produisent, sur une bande, l'aspect que
//! l'utilisateur connaît. Les recalculer donnerait un autre rendu sous le même
//! nom.
//!
//! WLED est sous **EUPL-1.2**, une licence copyleft. Reverb est sous
//! **GPL-3.0-or-later**, et l'annexe de l'EUPL 1.2 liste GPL-3.0 parmi les
//! licences compatibles : la reprise est donc licite, à condition d'attribuer et
//! de rester GPL. C'est un heureux hasard — en MIT, Reverb aurait dû changer de
//! licence pour une douzaine de dégradés.
//!
//! ⚠️ **Seules les données sont reprises, jamais le code.** WLED interpole en
//! C++ sur des `CRGBPalette16` de FastLED ; ici l'interpolation est réécrite,
//! sur les arrêts eux-mêmes et sans quantifier à seize entrées.

use reverb_proto::Rgb;

/// Les noms des douze palettes, dans l'ordre du tableau.
///
/// Cet ordre est celui de l'issue, et il est **figé par un test d'intention** :
/// c'est celui sous lequel le menu de la fenêtre les présentera, et le
/// réordonner déplacerait ce que l'utilisateur clique sans rien changer d'autre.
pub const PALETTES: &[&str] = &[
    "light-pink",
    "lava",
    "ocean",
    "paysage",
    "couchant",
    "aurore",
    "atlantica",
    "sakura",
    "nuit-avril",
    "glace",
    "orange-teal",
    "sorbet",
];

/// Un dégradé à arrêts.
///
/// ⚠️ **`Copy` et `Eq`** : `Reglages` la porte dans un `Option`, et le catalogue
/// se compare. Un index dans le tableau plutôt que le tableau lui-même, pour que
/// la copie ne coûte rien.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    rang: usize,
}

impl Palette {
    /// La palette de ce nom, ou le refus qui liste les douze.
    ///
    /// Calqué sur `Animation::par_nom` : un nom inconnu ne dit pas seulement
    /// qu'il est inconnu, il dit ce qui existe.
    pub fn par_nom(nom: &str) -> Result<Palette, PaletteInconnue> {
        PALETTES
            .iter()
            .position(|connu| *connu == nom)
            .map(|rang| Palette { rang })
            .ok_or_else(|| PaletteInconnue {
                saisi: nom.to_owned(),
                valides: PALETTES,
            })
    }

    /// Le nom tel qu'il s'écrit sur le socket.
    pub fn nom(&self) -> &'static str {
        TABLEAU[self.rang].0
    }

    /// Les arrêts, à positions **strictement croissantes**, de 0 à 255.
    pub fn arrets(&self) -> &'static [(u8, Rgb)] {
        TABLEAU[self.rang].1
    }

    /// La couleur à cette position du dégradé.
    ///
    /// ⚠️ **L'échelle est celle des arrêts, `0.0..=255.0`**, et non le carré
    /// unité. C'est ce qui rend « aux positions d'arrêt, la couleur d'arrêt
    /// exactement » vrai au bit près : passer par `k as f32 / 255.0` puis
    /// remultiplier ne rend pas `k`, et la propriété deviendrait « à une unité
    /// près », c'est-à-dire invérifiable.
    ///
    /// ⚠️ **Hors bornes, la couleur de borne** — jamais d'extrapolation. C'est
    /// le même refus que celui de la courbe de régulation : prolonger la droite
    /// du premier segment donnerait des composantes négatives, donc un
    /// débordement silencieux à l'autre bout de l'octet.
    pub fn echantillon(&self, position: f32) -> Rgb {
        let arrets = self.arrets();
        let (premier, dernier) = (arrets[0], arrets[arrets.len() - 1]);
        // ⚠️ Le NaN est dit explicitement plutôt que capté par un `!(a > b)` :
        // une position qui n'est pas un nombre retombe sur le premier arrêt au
        // lieu de propager son NaN dans les trois composantes, où il
        // deviendrait un octet quelconque.
        if position.is_nan() || position <= f32::from(premier.0) {
            return premier.1;
        }
        if position >= f32::from(dernier.0) {
            return dernier.1;
        }
        for couple in arrets.windows(2) {
            let ((debut, depuis), (fin, vers)) = (couple[0], couple[1]);
            if position < f32::from(fin) {
                let part = (position - f32::from(debut)) / f32::from(fin - debut);
                return Rgb {
                    r: entre(depuis.r, vers.r, part),
                    g: entre(depuis.g, vers.g, part),
                    b: entre(depuis.b, vers.b, part),
                };
            }
        }
        dernier.1
    }
}

/// Une composante interpolée, arrondie au plus proche.
///
/// ⚠️ **L'arrondi, et non la troncature.** Tronquer biaiserait tout le dégradé
/// vers le bas d'une demi-unité, ce qui ne se voit pas sur une LED mais fait
/// rater la couleur d'arrêt exacte au voisinage immédiat d'un arrêt.
fn entre(depuis: u8, vers: u8, part: f32) -> u8 {
    let valeur = f32::from(depuis) + (f32::from(vers) - f32::from(depuis)) * part;
    valeur.round().clamp(0.0, 255.0) as u8
}

/// Un nom de palette qui n'est pas au tableau.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteInconnue {
    pub saisi: String,
    pub valides: &'static [&'static str],
}

impl core::fmt::Display for PaletteInconnue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "palette « {} » inconnue. Palettes connues : {}",
            self.saisi,
            self.valides.join(", ")
        )
    }
}

impl std::error::Error for PaletteInconnue {}

/// Les douze palettes, dans l'ordre où l'issue #126 les énumère.
const TABLEAU: [(&str, &[(u8, Rgb)]); 12] = [
    // « Light Pink » dans WLED (`Pink_Purple_gp`), 11 arrêts.
    (
        "light-pink",
        &[
            (
                0,
                Rgb {
                    r: 79,
                    g: 32,
                    b: 109,
                },
            ),
            (
                25,
                Rgb {
                    r: 90,
                    g: 40,
                    b: 117,
                },
            ),
            (
                51,
                Rgb {
                    r: 102,
                    g: 48,
                    b: 124,
                },
            ),
            (
                76,
                Rgb {
                    r: 141,
                    g: 135,
                    b: 185,
                },
            ),
            (
                102,
                Rgb {
                    r: 180,
                    g: 222,
                    b: 248,
                },
            ),
            (
                109,
                Rgb {
                    r: 208,
                    g: 236,
                    b: 252,
                },
            ),
            (
                114,
                Rgb {
                    r: 237,
                    g: 250,
                    b: 255,
                },
            ),
            (
                122,
                Rgb {
                    r: 206,
                    g: 200,
                    b: 239,
                },
            ),
            (
                149,
                Rgb {
                    r: 177,
                    g: 149,
                    b: 222,
                },
            ),
            (
                183,
                Rgb {
                    r: 187,
                    g: 130,
                    b: 203,
                },
            ),
            (
                255,
                Rgb {
                    r: 198,
                    g: 111,
                    b: 184,
                },
            ),
        ],
    ),
    // « Lava » dans WLED (`lava_gp`), 13 arrêts.
    (
        "lava",
        &[
            (0, Rgb { r: 0, g: 0, b: 0 }),
            (46, Rgb { r: 77, g: 0, b: 0 }),
            (96, Rgb { r: 177, g: 0, b: 0 }),
            (
                108,
                Rgb {
                    r: 196,
                    g: 38,
                    b: 9,
                },
            ),
            (
                119,
                Rgb {
                    r: 215,
                    g: 76,
                    b: 19,
                },
            ),
            (
                146,
                Rgb {
                    r: 235,
                    g: 115,
                    b: 29,
                },
            ),
            (
                174,
                Rgb {
                    r: 255,
                    g: 153,
                    b: 41,
                },
            ),
            (
                188,
                Rgb {
                    r: 255,
                    g: 178,
                    b: 41,
                },
            ),
            (
                202,
                Rgb {
                    r: 255,
                    g: 204,
                    b: 41,
                },
            ),
            (
                218,
                Rgb {
                    r: 255,
                    g: 230,
                    b: 41,
                },
            ),
            (
                234,
                Rgb {
                    r: 255,
                    g: 255,
                    b: 41,
                },
            ),
            (
                244,
                Rgb {
                    r: 255,
                    g: 255,
                    b: 143,
                },
            ),
            (
                255,
                Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
            ),
        ],
    ),
    // « Ocean Breeze » dans WLED (`es_ocean_breeze_036_gp`), 4 arrêts.
    (
        "ocean",
        &[
            (
                0,
                Rgb {
                    r: 16,
                    g: 48,
                    b: 51,
                },
            ),
            (
                89,
                Rgb {
                    r: 27,
                    g: 166,
                    b: 175,
                },
            ),
            (
                153,
                Rgb {
                    r: 197,
                    g: 233,
                    b: 255,
                },
            ),
            (
                255,
                Rgb {
                    r: 0,
                    g: 145,
                    b: 152,
                },
            ),
        ],
    ),
    // « Landscape 64 » dans WLED (`es_landscape_64_gp`), 9 arrêts.
    (
        "paysage",
        &[
            (0, Rgb { r: 0, g: 0, b: 0 }),
            (
                37,
                Rgb {
                    r: 31,
                    g: 89,
                    b: 19,
                },
            ),
            (
                76,
                Rgb {
                    r: 72,
                    g: 178,
                    b: 43,
                },
            ),
            (
                127,
                Rgb {
                    r: 150,
                    g: 235,
                    b: 5,
                },
            ),
            (
                128,
                Rgb {
                    r: 186,
                    g: 234,
                    b: 119,
                },
            ),
            (
                130,
                Rgb {
                    r: 222,
                    g: 233,
                    b: 252,
                },
            ),
            (
                153,
                Rgb {
                    r: 197,
                    g: 219,
                    b: 231,
                },
            ),
            (
                204,
                Rgb {
                    r: 132,
                    g: 179,
                    b: 253,
                },
            ),
            (
                255,
                Rgb {
                    r: 28,
                    g: 107,
                    b: 225,
                },
            ),
        ],
    ),
    // « Sunset Real » dans WLED (`Sunset_Real_gp`), 7 arrêts.
    (
        "couchant",
        &[
            (0, Rgb { r: 181, g: 0, b: 0 }),
            (
                22,
                Rgb {
                    r: 218,
                    g: 85,
                    b: 0,
                },
            ),
            (
                51,
                Rgb {
                    r: 255,
                    g: 170,
                    b: 0,
                },
            ),
            (
                85,
                Rgb {
                    r: 211,
                    g: 85,
                    b: 77,
                },
            ),
            (
                135,
                Rgb {
                    r: 167,
                    g: 0,
                    b: 169,
                },
            ),
            (
                198,
                Rgb {
                    r: 73,
                    g: 0,
                    b: 188,
                },
            ),
            (255, Rgb { r: 0, g: 0, b: 207 }),
        ],
    ),
    // « Aurora » dans WLED (`Aurora_gp`), 6 arrêts.
    (
        "aurore",
        &[
            (0, Rgb { r: 1, g: 5, b: 45 }),
            (
                64,
                Rgb {
                    r: 0,
                    g: 200,
                    b: 23,
                },
            ),
            (128, Rgb { r: 0, g: 255, b: 0 }),
            (
                170,
                Rgb {
                    r: 0,
                    g: 243,
                    b: 45,
                },
            ),
            (200, Rgb { r: 0, g: 135, b: 7 }),
            (255, Rgb { r: 1, g: 5, b: 45 }),
        ],
    ),
    // « Atlantica » dans WLED (`Atlantica_gp`), 6 arrêts.
    (
        "atlantica",
        &[
            (
                0,
                Rgb {
                    r: 0,
                    g: 28,
                    b: 112,
                },
            ),
            (
                50,
                Rgb {
                    r: 32,
                    g: 96,
                    b: 255,
                },
            ),
            (
                100,
                Rgb {
                    r: 0,
                    g: 243,
                    b: 45,
                },
            ),
            (
                150,
                Rgb {
                    r: 12,
                    g: 95,
                    b: 82,
                },
            ),
            (
                200,
                Rgb {
                    r: 25,
                    g: 190,
                    b: 95,
                },
            ),
            (
                255,
                Rgb {
                    r: 40,
                    g: 170,
                    b: 80,
                },
            ),
        ],
    ),
    // « Sakura » dans WLED (`Sakura_gp`), 5 arrêts.
    (
        "sakura",
        &[
            (
                0,
                Rgb {
                    r: 196,
                    g: 19,
                    b: 10,
                },
            ),
            (
                65,
                Rgb {
                    r: 255,
                    g: 69,
                    b: 45,
                },
            ),
            (
                130,
                Rgb {
                    r: 223,
                    g: 45,
                    b: 72,
                },
            ),
            (
                195,
                Rgb {
                    r: 255,
                    g: 82,
                    b: 103,
                },
            ),
            (
                255,
                Rgb {
                    r: 223,
                    g: 13,
                    b: 17,
                },
            ),
        ],
    ),
    // « April Night » dans WLED (`April_Night_gp`), 17 arrêts.
    (
        "nuit-avril",
        &[
            (0, Rgb { r: 1, g: 5, b: 45 }),
            (10, Rgb { r: 1, g: 5, b: 45 }),
            (
                25,
                Rgb {
                    r: 5,
                    g: 169,
                    b: 175,
                },
            ),
            (40, Rgb { r: 1, g: 5, b: 45 }),
            (61, Rgb { r: 1, g: 5, b: 45 }),
            (
                76,
                Rgb {
                    r: 45,
                    g: 175,
                    b: 31,
                },
            ),
            (91, Rgb { r: 1, g: 5, b: 45 }),
            (112, Rgb { r: 1, g: 5, b: 45 }),
            (
                127,
                Rgb {
                    r: 249,
                    g: 150,
                    b: 5,
                },
            ),
            (143, Rgb { r: 1, g: 5, b: 45 }),
            (162, Rgb { r: 1, g: 5, b: 45 }),
            (
                178,
                Rgb {
                    r: 255,
                    g: 92,
                    b: 0,
                },
            ),
            (193, Rgb { r: 1, g: 5, b: 45 }),
            (214, Rgb { r: 1, g: 5, b: 45 }),
            (
                229,
                Rgb {
                    r: 223,
                    g: 45,
                    b: 72,
                },
            ),
            (244, Rgb { r: 1, g: 5, b: 45 }),
            (255, Rgb { r: 1, g: 5, b: 45 }),
        ],
    ),
    // « Fierce Ice » dans WLED (`fierce_ice_gp`), 7 arrêts.
    (
        "glace",
        &[
            (0, Rgb { r: 0, g: 0, b: 0 }),
            (
                59,
                Rgb {
                    r: 0,
                    g: 51,
                    b: 117,
                },
            ),
            (
                119,
                Rgb {
                    r: 0,
                    g: 102,
                    b: 255,
                },
            ),
            (
                149,
                Rgb {
                    r: 38,
                    g: 153,
                    b: 255,
                },
            ),
            (
                180,
                Rgb {
                    r: 86,
                    g: 204,
                    b: 255,
                },
            ),
            (
                217,
                Rgb {
                    r: 167,
                    g: 230,
                    b: 255,
                },
            ),
            (
                255,
                Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
            ),
        ],
    ),
    // « Orange & Teal » dans WLED (`Orange_Teal_gp`), 4 arrêts.
    (
        "orange-teal",
        &[
            (
                0,
                Rgb {
                    r: 0,
                    g: 150,
                    b: 92,
                },
            ),
            (
                55,
                Rgb {
                    r: 0,
                    g: 150,
                    b: 92,
                },
            ),
            (
                200,
                Rgb {
                    r: 255,
                    g: 72,
                    b: 0,
                },
            ),
            (
                255,
                Rgb {
                    r: 255,
                    g: 72,
                    b: 0,
                },
            ),
        ],
    ),
    // « Rainbow Sherbet » dans WLED (`rainbowsherbet_gp`), 7 arrêts.
    (
        "sorbet",
        &[
            (
                0,
                Rgb {
                    r: 255,
                    g: 102,
                    b: 41,
                },
            ),
            (
                43,
                Rgb {
                    r: 255,
                    g: 140,
                    b: 90,
                },
            ),
            (
                86,
                Rgb {
                    r: 255,
                    g: 51,
                    b: 90,
                },
            ),
            (
                127,
                Rgb {
                    r: 255,
                    g: 153,
                    b: 169,
                },
            ),
            (
                170,
                Rgb {
                    r: 255,
                    g: 255,
                    b: 249,
                },
            ),
            (
                209,
                Rgb {
                    r: 113,
                    g: 255,
                    b: 85,
                },
            ),
            (
                255,
                Rgb {
                    r: 157,
                    g: 255,
                    b: 137,
                },
            ),
        ],
    ),
];
