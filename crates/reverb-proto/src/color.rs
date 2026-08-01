//! Couleurs et luminosité.
//!
//! ⚠️ Les contrôleurs de ventilateurs attendent les composantes dans l'ordre
//! **GRB** (spec §2). L'écran du Kraken utilise BGR et la RAM Corsair RGB —
//! d'où l'isolement strict de la conversion dans ce module.

use std::fmt;

/// Erreur d'analyse d'une couleur hexadécimale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseColorError {
    pub input: String,
}

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "couleur « {} » invalide : attendu six chiffres hexadécimaux, par exemple ff00ff ou #ff00ff",
            self.input
        )
    }
}

impl std::error::Error for ParseColorError {}

/// Une couleur, en composantes rouge/vert/bleu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    /// Analyse une couleur hexadécimale à six chiffres, avec ou sans `#`.
    pub fn from_hex(input: &str) -> Result<Self, ParseColorError> {
        let invalide = || ParseColorError {
            input: input.to_owned(),
        };

        let chiffres = input.strip_prefix('#').unwrap_or(input);
        if chiffres.len() != 6 || !chiffres.bytes().all(|o| o.is_ascii_hexdigit()) {
            return Err(invalide());
        }

        // Le découpage est sûr : six caractères, tous hexadécimaux ASCII.
        let composante = |debut: usize| {
            u8::from_str_radix(&chiffres[debut..debut + 2], 16).map_err(|_| invalide())
        };

        Ok(Rgb {
            r: composante(0)?,
            g: composante(2)?,
            b: composante(4)?,
        })
    }

    /// Applique la luminosité en multipliant les composantes.
    ///
    /// Le protocole NZXT **n'a aucun octet de luminosité** : CAM l'applique
    /// côté hôte avant l'envoi (spec §4.3). Une luminosité nulle produit donc
    /// du noir, pas une LED éteinte par une commande dédiée.
    ///
    /// L'arrondi est une **troncature**, comme l'implémentation de référence de
    /// la spec §11 (`int(c * luminosite)`). La spec ne documente pas la règle
    /// d'arrondi de CAM : ce choix est le nôtre, pas une observation du matériel.
    pub fn with_brightness(self, brightness: Brightness) -> Self {
        let echelle = |composante: u8| {
            ((u16::from(composante) * u16::from(brightness.percent())) / 100) as u8
        };
        Rgb {
            r: echelle(self.r),
            g: echelle(self.g),
            b: echelle(self.b),
        }
    }

    /// Sérialise dans l'ordre attendu par les contrôleurs de ventilateurs (spec §2).
    pub fn to_grb(self) -> [u8; 3] {
        [self.g, self.r, self.b]
    }
}

/// Luminosité en pourcentage, bornée à `0..=100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Brightness(u8);

impl Brightness {
    pub const FULL: Brightness = Brightness(100);

    /// Construit une luminosité, en écrêtant au-delà de 100.
    pub const fn new(percent: u8) -> Self {
        Brightness(if percent > 100 { 100 } else { percent })
    }

    pub const fn percent(self) -> u8 {
        self.0
    }
}

impl Default for Brightness {
    fn default() -> Self {
        Brightness::FULL
    }
}

/// Une couleur en teinte, saturation et luminosité.
///
/// C'est le repère dans lequel on **choisit** une couleur : trois axes qu'on
/// comprend en les bougeant, là où trois octets de rouge, vert et bleu ne se
/// devinent pas. `Rgb` reste le repère dans lequel on l'**écrit** sur un bus.
///
/// # ⚠️ C'est HSV, malgré le nom
///
/// « TSL » se lit d'ordinaire *teinte, saturation, lightness*. Ce n'est **pas**
/// ce modèle-ci : la luminosité vaut ici la composante la plus forte — la
/// *value* de HSV — et non la moyenne des extrêmes. C'est ce que montrent les
/// sélecteurs de couleur sous le nom de *Brightness*, et c'est ce que Nico a
/// demandé : `00aeed` doit afficher 93, ce qui est `0xed / 255`. En HSL la même
/// couleur afficherait 46,5, et les curseurs seraient faux sans qu'aucun test de
/// forme ne s'en aperçoive. Trois tests d'intention épinglent HSV explicitement.
///
/// # Pourquoi des flottants
///
/// Un tour de teinte à l'entier près ne compte que 360 valeurs, la saturation et
/// la luminosité 101 chacune : trois millions sept cent mille triplets pour
/// seize millions sept cent mille couleurs. Des entiers ne peuvent donc **pas**
/// rendre l'aller-retour exact, et un curseur qui perd la couleur qu'on vient de
/// saisir est un curseur qu'on ne peut pas utiliser. La fenêtre arrondit pour
/// **afficher** ; elle ne calcule jamais sur l'arrondi.
///
/// # Pas de canal alpha
///
/// Une LED n'a pas de transparence, et le modèle de zones ne superpose jamais
/// deux couches sur une même LED : un alpha n'y aurait aucun effet observable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tsl {
    /// Degrés sur le tour, `0..360`. Rouge à 0, vert à 120, bleu à 240.
    pub teinte: f32,
    /// Pourcents, `0..=100`. Zéro donne un gris.
    pub saturation: f32,
    /// Pourcents, `0..=100`. Zéro donne le noir.
    pub luminosite: f32,
}

/// Erreur rendue par [`Rgb::depuis_tsl`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TslInvalide {
    pub champ: &'static str,
    pub raison: String,
}

impl fmt::Display for TslInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "champ « {} » : {}", self.champ, self.raison)
    }
}

impl std::error::Error for TslInvalide {}

/// Refuse une valeur non finie ou hors de `0..=haut`.
///
/// `haut_inclus` dit si la borne haute compte. Une valeur hors bornes est
/// **refusée** et non repliée : replier en silence ferait tourner une animation
/// dans une teinte que personne n'a demandée, et sans rien dire.
fn borner(
    champ: &'static str,
    valeur: f32,
    haut: f32,
    haut_inclus: bool,
) -> Result<(), TslInvalide> {
    if !valeur.is_finite() {
        return Err(TslInvalide {
            champ,
            raison: format!("{valeur} n'est pas un nombre"),
        });
    }
    let dedans = valeur >= 0.0
        && if haut_inclus {
            valeur <= haut
        } else {
            valeur < haut
        };
    if dedans {
        return Ok(());
    }
    Err(TslInvalide {
        champ,
        raison: format!(
            "{valeur} est hors de 0 à {haut}{}",
            if haut_inclus { " inclus" } else { " exclu" }
        ),
    })
}

/// Une fraction de 0 à 1, en octet, **arrondie au plus proche**.
///
/// ⚠️ Pas tronquée, contrairement à [`Rgb::with_brightness`] : là-bas la
/// troncature est la règle du protocole (spec §11), ici elle décalerait chaque
/// aller-retour d'un cran vers le noir.
fn octet(fraction: f32) -> u8 {
    (fraction * 255.0).round().clamp(0.0, 255.0) as u8
}

impl Rgb {
    /// La même couleur, en teinte, saturation et luminosité.
    ///
    /// ⚠️ **Un gris n'a pas de teinte** et **le noir n'a pas de saturation** :
    /// la conversion rend zéro, faute de mieux. C'est pourquoi la fenêtre garde
    /// un [`Tsl`] comme état et non un `Rgb` — sinon la teinte se perdrait en
    /// passant par le gris, et il faudrait la retrouver au jugé.
    pub fn en_tsl(self) -> Tsl {
        let (r, v, b) = (f32::from(self.r), f32::from(self.g), f32::from(self.b));
        let fort = r.max(v).max(b);
        let faible = r.min(v).min(b);
        let ecart = fort - faible;

        let teinte = if ecart == 0.0 {
            // Un gris n'a pas de teinte. Zéro faute de mieux — et c'est
            // précisément pourquoi la fenêtre garde un `Tsl` comme état : sinon
            // la teinte se perdrait en passant par le gris.
            0.0
        } else if fort == r {
            60.0 * (v - b) / ecart
        } else if fort == v {
            60.0 * ((b - r) / ecart + 2.0)
        } else {
            60.0 * ((r - v) / ecart + 4.0)
        };
        let teinte = if teinte < 0.0 { teinte + 360.0 } else { teinte };

        Tsl {
            // ⚠️ Jamais 360, même par arrondi : `depuis_tsl` refuse cette valeur,
            // et refuser une couleur que `en_tsl` vient de produire serait un
            // aller-retour cassé sur une poignée de teintes.
            teinte: if teinte >= 360.0 { 0.0 } else { teinte },
            saturation: if fort == 0.0 {
                // Le noir n'a pas de saturation : `0 / 0` rendrait `NaN`.
                0.0
            } else {
                ecart / fort * 100.0
            },
            luminosite: fort / 255.0 * 100.0,
        }
    }

    /// La couleur que ces trois axes désignent.
    ///
    /// L'aller-retour est **exact** : `Rgb::depuis_tsl(couleur.en_tsl())` rend
    /// `couleur`, pour les seize millions sept cent mille.
    pub fn depuis_tsl(tsl: Tsl) -> Result<Rgb, TslInvalide> {
        // La borne haute de la teinte est **exclue** : 360 est le même point que
        // 0, et l'accepter donnerait deux écritures pour une seule couleur.
        borner("teinte", tsl.teinte, 360.0, false)?;
        borner("saturation", tsl.saturation, 100.0, true)?;
        borner("luminosite", tsl.luminosite, 100.0, true)?;

        let valeur = tsl.luminosite / 100.0;
        let saturation = tsl.saturation / 100.0;
        let vive = valeur * saturation;
        let secteur = tsl.teinte / 60.0;
        // La composante intermédiaire : elle monte d'un bord du sextant à
        // l'autre, ce qui fait la continuité du tour.
        let milieu = vive * (1.0 - ((secteur % 2.0) - 1.0).abs());
        let fond = valeur - vive;

        let (r, v, b) = match secteur as u32 {
            0 => (vive, milieu, 0.0),
            1 => (milieu, vive, 0.0),
            2 => (0.0, vive, milieu),
            3 => (0.0, milieu, vive),
            4 => (milieu, 0.0, vive),
            _ => (vive, 0.0, milieu),
        };
        Ok(Rgb::new(octet(r + fond), octet(v + fond), octet(b + fond)))
    }
}
