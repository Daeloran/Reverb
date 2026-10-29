//! Le mode gabarit de l'écran (issue #144) : un SVG complet, rendu tel quel.
//!
//! La composition (#80) dessine elle-même ses arcs, ses boîtes et son texte en
//! Nunito : elle ne peut pas reproduire une maquette faite ailleurs. Ici, c'est
//! le SVG qui décide de tout, et le démon n'y change que ses **marqueurs**
//! `{{sonde:<slug>}}`, remplacés par la valeur de la sonde.
//!
//! ⚠️ **Le slug va jusqu'à `}}`, deux-points compris.** Les vrais slugs en
//! portent (`k10temp:tctl`, `kraken2023elite:coolant-temp`) : couper au premier
//! deux-points désignerait une sonde qui n'existe pas.
//!
//! ⚠️ **Une sonde muette ou inconnue s'écrit `--`**, jamais `0` ni la dernière
//! valeur connue — la règle du cadran et de la composition, pour la même raison :
//! un chiffre figé derrière une pompe arrêtée est rassurant, et faux.
//!
//! Le rendu passe par `resvg`, en Rust pur : la promesse de l'ADR-001 tient.
//! **Aucune police du système n'est lue** — le démon est root, et ce qu'il
//! affiche ne doit pas dépendre de ce que l'image Bazzite installe ce mois-ci.
//! Les polices viennent du dossier du gabarit, et la fonte embarquée du démon
//! sert de repli à toute famille introuvable.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use resvg::tiny_skia;
use resvg::usvg;
use reverb_proto::screen;

use crate::ecran::{Dalle, FONTE};

/// Ce qu'on écrit à la place d'une valeur qu'on n'a pas.
const TIRETS: &str = "--";

/// Le début d'un marqueur. La fin est `}}`.
const OUVERTURE: &str = "{{sonde:";

/// Au-delà, un « gabarit » n'en est pas un : un SVG d'écran pèse quelques
/// kilooctets, et lire un fichier de plusieurs centaines de mégaoctets parce
/// qu'on s'est trompé de chemin ne doit pas geler le démon.
pub const TAILLE_MAX: u64 = 4 * 1024 * 1024;

/// Un gabarit refusé : illisible, trop gros, ou pas du SVG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GabaritInvalide {
    pub raison: String,
}

impl fmt::Display for GabaritInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raison)
    }
}

impl std::error::Error for GabaritInvalide {}

/// Les sondes que les marqueurs nomment, sans doublon, dans l'ordre où elles
/// apparaissent.
pub fn sondes_du_gabarit(svg: &str) -> Vec<String> {
    let mut sondes: Vec<String> = Vec::new();
    let mut reste = svg;
    while let Some(debut) = reste.find(OUVERTURE) {
        let apres = &reste[debut + OUVERTURE.len()..];
        let Some(fin) = apres.find("}}") else {
            break;
        };
        let slug = apres[..fin].trim();
        if !slug.is_empty() && !sondes.iter().any(|connue| connue == slug) {
            sondes.push(slug.to_owned());
        }
        reste = &apres[fin + 2..];
    }
    sondes
}

/// Ce qu'un marqueur devient : l'entier le plus proche, sans unité, ou des
/// tirets.
pub fn texte_de_valeur(valeur: Option<f32>) -> String {
    match valeur {
        Some(degres) if degres.is_finite() => {
            // `as i64` après `round` : « -0 » n'existe pas, et 34,6 donne 35.
            let entier = degres.round() as i64;
            entier.to_string()
        }
        _ => TIRETS.to_owned(),
    }
}

/// Le SVG, marqueurs remplacés.
///
/// Public parce que c'est ce que le démon compare d'un tour à l'autre : un
/// texte identique donne une dalle identique, et le rendu peut alors être sauté.
pub fn substituer(svg: &str, valeur: &dyn Fn(&str) -> Option<f32>) -> String {
    let mut sortie = String::with_capacity(svg.len());
    let mut reste = svg;
    while let Some(debut) = reste.find(OUVERTURE) {
        let apres = &reste[debut + OUVERTURE.len()..];
        let Some(fin) = apres.find("}}") else {
            break;
        };
        sortie.push_str(&reste[..debut]);
        sortie.push_str(&texte_de_valeur(valeur(apres[..fin].trim())));
        reste = &apres[fin + 2..];
    }
    sortie.push_str(reste);
    sortie
}

/// Un gabarit prêt à rendre : son texte, et ses polices chargées **une fois**.
///
/// ⚠️ Recharger les polices à chaque tour coûterait la lecture de plusieurs
/// fichiers toutes les deux secondes pour changer deux chiffres.
pub struct Gabarit {
    svg: String,
    options: usvg::Options<'static>,
}

impl Gabarit {
    /// Prépare un gabarit depuis son texte. `dossier_polices` : le dossier dont
    /// les `.ttf`/`.otf`, et ceux de son sous-dossier `fonts/`, sont chargés.
    pub fn nouveau(svg: &str, dossier_polices: Option<&Path>) -> Result<Gabarit, GabaritInvalide> {
        let mut base = usvg::fontdb::Database::new();
        base.load_font_data(FONTE.to_vec());
        // La famille de la fonte embarquée, lue dans la fonte et non écrite à la
        // main : c'est elle qui sert de repli à toute famille introuvable.
        let repli = base
            .faces()
            .next()
            .and_then(|face| face.families.first().map(|(nom, _)| nom.clone()))
            .unwrap_or_else(|| "Nunito".to_owned());
        if let Some(dossier) = dossier_polices {
            for fichier in polices_du_dossier(dossier) {
                // Une police illisible est ignorée : le repli prend sa place, et
                // un gabarit ne doit pas tomber pour un fichier abîmé à côté de lui.
                if let Ok(octets) = fs::read(&fichier) {
                    base.load_font_data(octets);
                }
            }
        }
        // usvg ajoute la famille générique `serif` à toute requête : la faire
        // pointer sur la fonte embarquée, c'est ce qui donne le repli.
        base.set_serif_family(repli.clone());
        base.set_sans_serif_family(repli.clone());
        base.set_monospace_family(repli.clone());
        base.set_cursive_family(repli.clone());
        base.set_fantasy_family(repli.clone());

        let options = usvg::Options {
            font_family: repli,
            fontdb: Arc::new(base),
            ..usvg::Options::default()
        };
        let gabarit = Gabarit {
            svg: svg.to_owned(),
            options,
        };
        // Vérifié dès la préparation : un SVG invalide doit être refusé à la
        // commande, pas découvert au premier tour de rendu.
        gabarit.arbre(&gabarit.svg)?;
        Ok(gabarit)
    }

    /// Les sondes que ce gabarit lit.
    pub fn sondes(&self) -> Vec<String> {
        sondes_du_gabarit(&self.svg)
    }

    /// Le SVG du moment, marqueurs remplacés.
    pub fn texte(&self, valeur: &dyn Fn(&str) -> Option<f32>) -> String {
        substituer(&self.svg, valeur)
    }

    /// Rend un texte déjà substitué.
    pub fn rendre_texte(&self, texte: &str) -> Result<Dalle, GabaritInvalide> {
        let arbre = self.arbre(texte)?;
        Ok(rendre_arbre(&arbre))
    }

    /// Rend le gabarit avec les valeurs du moment.
    pub fn rendre(&self, valeur: &dyn Fn(&str) -> Option<f32>) -> Result<Dalle, GabaritInvalide> {
        self.rendre_texte(&self.texte(valeur))
    }

    fn arbre(&self, texte: &str) -> Result<usvg::Tree, GabaritInvalide> {
        usvg::Tree::from_str(texte, &self.options).map_err(|erreur| GabaritInvalide {
            raison: format!("gabarit SVG invalide : {erreur}"),
        })
    }
}

/// Le rendu en une fois : préparer, puis rendre.
pub fn rendre_gabarit(
    svg: &str,
    dossier_polices: Option<&Path>,
    valeur: &dyn Fn(&str) -> Option<f32>,
) -> Result<Dalle, GabaritInvalide> {
    Gabarit::nouveau(svg, dossier_polices)?.rendre(valeur)
}

/// Lit un gabarit sur le disque, polices de son dossier comprises.
pub fn charger(chemin: &Path) -> Result<Gabarit, GabaritInvalide> {
    let texte = lire(chemin)?;
    Gabarit::nouveau(&texte, chemin.parent())
}

/// Le texte d'un gabarit, borné en taille.
pub fn lire(chemin: &Path) -> Result<String, GabaritInvalide> {
    let illisible = |erreur: std::io::Error| GabaritInvalide {
        raison: format!("« {} » illisible : {erreur}", chemin.display()),
    };
    let taille = fs::metadata(chemin).map_err(illisible)?.len();
    if taille > TAILLE_MAX {
        return Err(GabaritInvalide {
            raison: format!(
                "« {} » pèse {taille} octets : un gabarit d'écran en fait quelques milliers",
                chemin.display()
            ),
        });
    }
    let octets = fs::read(chemin).map_err(illisible)?;
    String::from_utf8(octets).map_err(|_| GabaritInvalide {
        raison: format!(
            "« {} » n'est pas un texte UTF-8 : pas un SVG",
            chemin.display()
        ),
    })
}

/// Les polices d'un dossier : ses `.ttf`/`.otf`, puis ceux de `fonts/`.
///
/// ⚠️ **Pas de parcours récursif du dossier lui-même** : un gabarit posé dans
/// un dossier personnel ferait lire au démon toute l'arborescence.
fn polices_du_dossier(dossier: &Path) -> Vec<PathBuf> {
    let mut polices = Vec::new();
    for lieu in [dossier.to_path_buf(), dossier.join("fonts")] {
        let Ok(entrees) = fs::read_dir(&lieu) else {
            continue;
        };
        let mut trouvees: Vec<PathBuf> = entrees
            .filter_map(Result::ok)
            .map(|entree| entree.path())
            .filter(|chemin| {
                chemin.is_file()
                    && chemin
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| {
                            matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf")
                        })
            })
            .collect();
        // Trié : l'ordre de `read_dir` ne l'est pas, et deux familles homonymes
        // donneraient sinon un rendu qui change d'un démarrage à l'autre.
        trouvees.sort();
        polices.extend(trouvees);
    }
    polices
}

/// Met l'arbre à l'échelle de la dalle, sans déformer, centré sur du noir.
fn rendre_arbre(arbre: &usvg::Tree) -> Dalle {
    let (largeur, hauteur) = (u32::from(screen::WIDTH), u32::from(screen::HEIGHT));
    let Some(mut toile) = tiny_skia::Pixmap::new(largeur, hauteur) else {
        return Dalle::noire();
    };
    toile.fill(tiny_skia::Color::BLACK);

    let taille = arbre.size();
    let echelle = (largeur as f32 / taille.width()).min(hauteur as f32 / taille.height());
    let decalage_x = (largeur as f32 - taille.width() * echelle) / 2.0;
    let decalage_y = (hauteur as f32 - taille.height() * echelle) / 2.0;
    let transformation =
        tiny_skia::Transform::from_scale(echelle, echelle).post_translate(decalage_x, decalage_y);
    resvg::render(arbre, transformation, &mut toile.as_mut());

    // Le fond est opaque : la prémultiplication de tiny-skia n'a rien changé aux
    // composantes, qui se lisent donc telles quelles.
    let rgb: Vec<u8> = toile
        .data()
        .chunks_exact(4)
        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();
    Dalle::depuis_rgb(&rgb)
}
