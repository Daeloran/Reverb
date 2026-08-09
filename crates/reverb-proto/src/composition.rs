//! Ce que l'écran du Kraken compose : un fond, et jusqu'à quatre informations
//! posées dessus.
//!
//! Ce module est **pur**. Il dit *quoi* afficher et *où* — jamais avec quels
//! pixels : le rendu vit dans le démon, seul à parler au crate `image`, et la
//! composition doit se relire, s'écrire et se tester sans ouvrir de dalle.
//!
//! # La dalle est ronde
//!
//! Observé sur le matériel le 2026-08-08. Les quatre coins du tampon 640 × 640
//! sont donc **hors du disque visible** — 21 % de la surface —, et c'est ce qui
//! décide de la forme de ce module : les cinq ancres évitent les coins, et
//! [`Boite::dans_le_disque`] le vérifie plutôt que de l'espérer.
//!
//! Le rayon exact du disque reste à mesurer par la mire de l'issue #77 ; il vit
//! dans [`crate::screen::VISIBLE_DISC_RADIUS`], et c'est le seul endroit à
//! corriger quand la mire aura parlé.
//!
//! # Quatre champs, et pas cinq
//!
//! La dalle fait six centimètres, et le cadran de l'issue #33 occupe tout
//! l'écran pour **une** valeur. Au-delà de quatre, on écrit ce que personne ne
//! lit — le plafond est une contrainte physique, pas un choix d'interface.

use std::fmt;

use crate::screen;

/// Les cinq endroits où un champ peut se poser.
///
/// ⚠️ **Aucune ancre en coin.** Les quatre coins du tampon sont hors du disque
/// visible : un champ posé là s'afficherait dans le vide, sans qu'aucun message
/// ne le dise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ancre {
    Haut,
    Bas,
    Gauche,
    Droite,
    Centre,
}

/// La hauteur d'un champ, en pixels.
///
/// De quoi porter un libellé en police matricielle et une valeur en chiffres
/// hauts de quarante-huit pixels — ce qui se lit à un mètre, comme le cadran.
const CHAMP_HAUTEUR: u16 = 96;

/// Largeur d'un champ posé en haut ou en bas.
const BANDE_LARGEUR: u16 = 300;

/// Largeur d'un champ de la bande médiane, où trois se partagent la place.
///
/// ⚠️ **Réduite de 196 à 176 par #90.** La couronne des arcs occupe désormais
/// l'anneau extérieur du disque, et les boîtes ont dû reculer pour lui laisser
/// la place. Trois boîtes de 176 séparées de 18 tiennent dans les 564 pixels
/// qui restent entre les deux marges.
const MEDIANE_LARGEUR: u16 = 176;

/// Marge latérale des deux champs extrêmes de la bande médiane.
///
/// Trente-huit pixels : c'est ce qu'il faut pour que leurs coins retombent sous
/// [`COURONNE_RAYON_INTERIEUR`], calculé et non estimé — le coin le plus éloigné
/// est à `√(282² + 48²) ≈ 286,1`.
const MEDIANE_MARGE: u16 = 38;

/// Rang du haut de la bande médiane : les trois champs y sont centrés
/// verticalement sur la dalle.
const MEDIANE_Y: u16 = screen::HEIGHT / 2 - CHAMP_HAUTEUR / 2;

/// Rang du haut du champ supérieur.
///
/// ⚠️ **Descendu de 40 à 76 par #90.** À quarante pixels du bord, les coins de la
/// boîte étaient à `√(150² + 280²) ≈ 317,6` — dans le disque de 320, mais très
/// exactement là où la couronne des arcs devait passer. À soixante-seize, ils
/// retombent à `√(150² + 244²) ≈ 286,4`, sous [`COURONNE_RAYON_INTERIEUR`].
/// Le calcul est vérifié par [`Boite::dans_le_disque`], pas supposé.
const BANDE_HAUT_Y: u16 = 76;

/// Rang du haut du champ inférieur, symétrique du précédent.
const BANDE_BAS_Y: u16 = screen::HEIGHT - BANDE_HAUT_Y - CHAMP_HAUTEUR;

/// Le bord intérieur de la couronne où se dessinent les arcs (issue #90).
///
/// Aucune boîte d'ancre ne l'atteint : la plus lointaine s'arrête à 286,4.
pub const COURONNE_RAYON_INTERIEUR: u16 = 292;

/// Le bord extérieur de la couronne.
///
/// Sous [`crate::screen::VISIBLE_DISC_RADIUS`], parce que la dalle est ronde et
/// que 21 % du tampon ne s'affiche nulle part (`SPEC-KRAKEN-LCD` §2.1.1). Quatre
/// pixels de retrait : de quoi absorber l'anticrénelage du bord sans mordre sur
/// ce que le contrôleur coupe.
pub const COURONNE_RAYON_EXTERIEUR: u16 = 316;

/// L'ouverture d'un secteur d'ancre, en degrés.
///
/// Soixante-dix, et quatre secteurs : il reste vingt degrés entre deux voisins,
/// assez pour qu'aucun arc plein n'en touche un autre — c'est vérifié au pixel,
/// pas supposé.
const SECTEUR_OUVERTURE: f32 = 70.0;

/// Un secteur de couronne, en degrés.
///
/// ⚠️ **Zéro au sommet, croissant dans le sens horaire.** C'est la convention
/// qu'on lit sur une dalle ronde posée à plat, et elle est celle des quatre
/// ancres de bord : `haut` à 0°, `droite` à 90°, `bas` à 180°, `gauche` à 270°.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Secteur {
    /// L'angle où l'arc commence, en degrés, dans `[0, 360)`.
    pub debut: f32,
    /// De combien de degrés il s'étend, dans le sens horaire.
    pub ouverture: f32,
}

impl Ancre {
    /// Les cinq, dans l'ordre où les champs sont rendus et écrits.
    ///
    /// ⚠️ **C'est cet ordre qui fait foi**, jamais celui des poses : deux
    /// compositions portant les mêmes champs doivent s'écrire à l'identique,
    /// sinon un fichier changerait sans que rien n'ait changé.
    pub const TOUTES: [Ancre; 5] = [
        Ancre::Haut,
        Ancre::Bas,
        Ancre::Gauche,
        Ancre::Droite,
        Ancre::Centre,
    ];

    /// Le nom que le protocole et le fichier portent.
    pub fn slug(self) -> &'static str {
        match self {
            Ancre::Haut => "haut",
            Ancre::Bas => "bas",
            Ancre::Gauche => "gauche",
            Ancre::Droite => "droite",
            Ancre::Centre => "centre",
        }
    }

    /// L'inverse, strict.
    pub fn depuis_slug(saisi: &str) -> Result<Ancre, AncreInconnue> {
        Ancre::TOUTES
            .into_iter()
            .find(|ancre| ancre.slug() == saisi)
            .ok_or_else(|| AncreInconnue {
                saisi: saisi.to_owned(),
            })
    }

    /// Là où ce champ se dessine dans le tampon 640 × 640.
    ///
    /// Les trois ancres de la bande médiane se partagent la largeur ; celles du
    /// haut et du bas ont la leur pour elles seules.
    pub fn boite(self) -> Boite {
        match self {
            Ancre::Haut => Boite {
                x: (screen::WIDTH - BANDE_LARGEUR) / 2,
                y: BANDE_HAUT_Y,
                largeur: BANDE_LARGEUR,
                hauteur: CHAMP_HAUTEUR,
            },
            Ancre::Bas => Boite {
                x: (screen::WIDTH - BANDE_LARGEUR) / 2,
                y: BANDE_BAS_Y,
                largeur: BANDE_LARGEUR,
                hauteur: CHAMP_HAUTEUR,
            },
            Ancre::Gauche => Boite {
                x: MEDIANE_MARGE,
                y: MEDIANE_Y,
                largeur: MEDIANE_LARGEUR,
                hauteur: CHAMP_HAUTEUR,
            },
            Ancre::Centre => Boite {
                x: (screen::WIDTH - MEDIANE_LARGEUR) / 2,
                y: MEDIANE_Y,
                largeur: MEDIANE_LARGEUR,
                hauteur: CHAMP_HAUTEUR,
            },
            Ancre::Droite => Boite {
                x: screen::WIDTH - MEDIANE_MARGE - MEDIANE_LARGEUR,
                y: MEDIANE_Y,
                largeur: MEDIANE_LARGEUR,
                hauteur: CHAMP_HAUTEUR,
            },
        }
    }

    /// Le secteur de couronne où cette ancre dessine son arc (issue #90).
    ///
    /// ⚠️ **`Centre` n'en a pas, et ce n'est pas un oubli** : elle est au milieu
    /// du disque, pas sur son bord. Lui donner une ouverture nulle aurait fait
    /// d'elle un secteur dégénéré que tout calcul d'angle aurait dû traiter à
    /// part ; `None` le dit une fois pour toutes.
    pub fn secteur(self) -> Option<Secteur> {
        let milieu = match self {
            Ancre::Haut => 0.0,
            Ancre::Droite => 90.0,
            Ancre::Bas => 180.0,
            Ancre::Gauche => 270.0,
            Ancre::Centre => return None,
        };
        Some(Secteur {
            debut: (milieu - SECTEUR_OUVERTURE / 2.0).rem_euclid(360.0),
            ouverture: SECTEUR_OUVERTURE,
        })
    }
}

impl fmt::Display for Ancre {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

/// Une ancre qu'on ne connaît pas.
///
/// ⚠️ Le refus **donne la liste**. Il n'y en a que cinq, elles ne se devinent
/// pas, et un « ancre invalide » sec laisserait chercher dans une documentation
/// que personne n'a sous la main au moment où il tape la commande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AncreInconnue {
    /// Ce qui a été saisi, sans réécriture.
    pub saisi: String,
}

impl fmt::Display for AncreInconnue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let noms: Vec<&str> = Ancre::TOUTES.iter().map(|ancre| ancre.slug()).collect();
        write!(
            f,
            "« {} » n'est pas une ancre : les cinq sont {}",
            self.saisi.escape_debug(),
            noms.join(", ")
        )
    }
}

impl std::error::Error for AncreInconnue {}

/// Un rectangle du tampon, en pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Boite {
    pub x: u16,
    pub y: u16,
    pub largeur: u16,
    pub hauteur: u16,
}

impl Boite {
    /// Les quatre coins tiennent-ils dans le disque visible ?
    ///
    /// En **entiers**, comparés au carré du rayon : une racine carrée en
    /// flottant sur une boîte tangente rendrait le verdict dépendant d'un
    /// arrondi, là où la question est exacte.
    pub fn dans_le_disque(self) -> bool {
        let centre = i32::from(screen::WIDTH) / 2;
        let rayon = i32::from(screen::VISIBLE_DISC_RADIUS);
        let (x0, y0) = (i32::from(self.x), i32::from(self.y));
        let (x1, y1) = (x0 + i32::from(self.largeur), y0 + i32::from(self.hauteur));
        [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
            .into_iter()
            .all(|(x, y)| {
                let (dx, dy) = (x - centre, y - centre);
                dx * dx + dy * dy <= rayon * rayon
            })
    }
}

/// Ce qu'un champ affiche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Une sonde de température, avec un libellé facultatif.
    ///
    /// ⚠️ **Le libellé existe parce que le slug ne se lit pas.**
    /// « kraken2023elite:coolant-temp » fait vingt-huit caractères sur une dalle
    /// de six centimètres. Le README relève déjà que le cadran impose le slug et
    /// que c'est un défaut ; ici, on nomme soi-même ce qu'on regarde.
    Temperature {
        sonde: String,
        libelle: Option<String>,
    },
    /// Un texte fixe.
    Texte(String),
}

impl Source {
    /// `temp <slug>` · `temp <slug> <libellé>` · `texte <libellé>`.
    ///
    /// ⚠️ **Le libellé est le dernier champ de sa ligne et va jusqu'au bout** —
    /// comme le nom d'un profil (#74) et comme un chemin d'image. Coupé au
    /// premier blanc, « CPU gauche » deviendrait « CPU ».
    pub fn encoder(&self) -> String {
        match self {
            Source::Temperature {
                sonde,
                libelle: None,
            } => format!("temp {sonde}"),
            Source::Temperature {
                sonde,
                libelle: Some(libelle),
            } => format!("temp {sonde} {libelle}"),
            Source::Texte(texte) => format!("texte {texte}"),
        }
    }

    /// L'inverse, strict, en disant ce qui manque.
    pub fn decoder(texte: &str) -> Result<Source, SourceInvalide> {
        let refus = |raison: String| SourceInvalide { raison };
        let texte = texte.trim();
        let (mot, reste) = match texte.split_once(char::is_whitespace) {
            Some((mot, reste)) => (mot, reste.trim()),
            None => (texte, ""),
        };

        match mot {
            "temp" => {
                let (sonde, libelle) = match reste.split_once(char::is_whitespace) {
                    Some((sonde, libelle)) => (sonde, libelle.trim()),
                    None => (reste, ""),
                };
                if sonde.is_empty() {
                    return Err(refus(
                        "« temp » sans sonde : il faut le nom que « status » donne, par exemple \
                         « kraken2023elite:coolant-temp »"
                            .to_owned(),
                    ));
                }
                Ok(Source::Temperature {
                    sonde: sonde.to_owned(),
                    libelle: (!libelle.is_empty()).then(|| libelle.to_owned()),
                })
            }
            "texte" => {
                if reste.is_empty() {
                    return Err(refus(
                        "« texte » sans libellé : un champ vide n'affiche rien, et « vide \
                         <ancre> » est la façon de retirer un champ"
                            .to_owned(),
                    ));
                }
                Ok(Source::Texte(reste.to_owned()))
            }
            "" => Err(refus(
                "champ sans source : « temp <sonde> » ou « texte <libellé> »".to_owned(),
            )),
            autre => Err(refus(format!(
                "« {} » n'est pas une source : « temp <sonde> » ou « texte <libellé> »",
                autre.escape_debug()
            ))),
        }
    }
}

/// Une source qu'on n'a pas su lire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInvalide {
    pub raison: String,
}

impl fmt::Display for SourceInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raison)
    }
}

impl std::error::Error for SourceInvalide {}

/// Ce que la composition a sous ses champs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fond {
    /// Du noir uni. Les champs se lisent, et rien d'autre n'est affiché.
    Noir,
    /// Une image, mise à l'échelle comme `screen image` la met.
    Image(String),
}

impl Fond {
    /// `noir` · `image <chemin>`.
    pub fn encoder(&self) -> String {
        match self {
            Fond::Noir => "noir".to_owned(),
            Fond::Image(chemin) => format!("image {chemin}"),
        }
    }

    /// L'inverse, strict.
    pub fn decoder(texte: &str) -> Result<Fond, FondInvalide> {
        let refus = |raison: String| FondInvalide { raison };
        let texte = texte.trim();
        let (mot, reste) = match texte.split_once(char::is_whitespace) {
            Some((mot, reste)) => (mot, reste.trim()),
            None => (texte, ""),
        };

        match mot {
            "noir" if reste.is_empty() => Ok(Fond::Noir),
            "noir" => Err(refus(format!(
                "« noir » ne prend rien après lui, et « {} » suit",
                reste.escape_debug()
            ))),
            "image" if reste.is_empty() => Err(refus(
                "« image » sans chemin : il en faut un, et absolu — le démon ne partage pas le \
                 répertoire courant de son client"
                    .to_owned(),
            )),
            "image" => Ok(Fond::Image(reste.to_owned())),
            "" => Err(refus(
                "fond sans nature : « noir » ou « image <chemin> »".to_owned(),
            )),
            autre => Err(refus(format!(
                "« {} » n'est pas un fond : « noir » ou « image <chemin> »",
                autre.escape_debug()
            ))),
        }
    }
}

/// Un fond qu'on n'a pas su lire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FondInvalide {
    pub raison: String,
}

impl fmt::Display for FondInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raison)
    }
}

impl std::error::Error for FondInvalide {}

/// Un fond, et jusqu'à quatre champs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composition {
    fond: Fond,
    /// Les champs posés, **tenus triés** dans l'ordre de [`Ancre::TOUTES`].
    ///
    /// ⚠️ Le tri n'est pas un confort d'écriture : il fait de l'égalité une
    /// égalité **logique**. Deux compositions portant les mêmes champs posés
    /// dans un autre ordre doivent être égales, sinon un aller-retour par le
    /// fichier — qui les réordonne — rendrait une composition différente de
    /// celle qu'on y a écrite.
    champs: Vec<(Ancre, Source)>,
}

impl Composition {
    /// Quatre. La dalle fait six centimètres — voir l'en-tête du module.
    pub const CHAMPS_MAX: usize = 4;

    /// Une composition sur ce fond, sans aucun champ.
    pub fn nouvelle(fond: Fond) -> Composition {
        Composition {
            fond,
            champs: Vec::new(),
        }
    }

    pub fn fond(&self) -> &Fond {
        &self.fond
    }

    pub fn changer_fond(&mut self, fond: Fond) {
        self.fond = fond;
    }

    pub fn champ(&self, ancre: Ancre) -> Option<&Source> {
        self.champs
            .iter()
            .find(|(posee, _)| *posee == ancre)
            .map(|(_, source)| source)
    }

    /// Les champs posés, dans l'ordre de [`Ancre::TOUTES`].
    pub fn champs(&self) -> Vec<(Ancre, &Source)> {
        self.champs
            .iter()
            .map(|(ancre, source)| (*ancre, source))
            .collect()
    }

    /// Combien de champs sont posés.
    pub fn compte(&self) -> usize {
        self.champs.len()
    }

    /// Pose un champ, ou **remplace** celui que l'ancre portait déjà.
    ///
    /// ⚠️ **Remplacer n'est jamais un cinquième champ.** Refuser de corriger le
    /// libellé d'un champ existant sous prétexte que la composition est pleine
    /// serait un plafond qui se retourne contre celui qui le respecte.
    pub fn poser(&mut self, ancre: Ancre, source: Source) -> Result<(), TropDeChamps> {
        if let Some(place) = self.champs.iter_mut().find(|(posee, _)| *posee == ancre) {
            place.1 = source;
            return Ok(());
        }
        if self.compte() >= Composition::CHAMPS_MAX {
            return Err(TropDeChamps {
                plafond: Composition::CHAMPS_MAX,
            });
        }
        // Inséré à sa place dans l'ordre des ancres, jamais ajouté au bout :
        // c'est ce qui rend deux compositions identiques égales quel que soit
        // l'ordre où on les a construites.
        let rang = Composition::rang(ancre);
        let place = self
            .champs
            .iter()
            .position(|(posee, _)| Composition::rang(*posee) > rang)
            .unwrap_or(self.champs.len());
        self.champs.insert(place, (ancre, source));
        Ok(())
    }

    /// Retire le champ de cette ancre. Vrai s'il y en avait un.
    pub fn vider(&mut self, ancre: Ancre) -> bool {
        let Some(place) = self.champs.iter().position(|(posee, _)| *posee == ancre) else {
            return false;
        };
        self.champs.remove(place);
        true
    }

    /// Les lignes du fichier : `fond …`, puis une `champ …` par champ posé.
    pub fn encoder(&self) -> String {
        let mut texte = format!("fond {}\n", self.fond.encoder());
        for (ancre, source) in self.champs() {
            texte.push_str(&format!("champ {} {}\n", ancre.slug(), source.encoder()));
        }
        texte
    }

    /// L'inverse, strict, en nommant la ligne fautive.
    ///
    /// ⚠️ **La ligne rendue est relative au bloc**, `1` pour le `fond` : ce bloc
    /// est écrit tantôt dans `ecran.conf`, tantôt dans un profil, et il ne peut
    /// pas connaître le rang qu'il y occupe. C'est à l'appelant de le décaler.
    pub fn decoder(texte: &str) -> Result<Composition, CompositionInvalide> {
        let lignes: Vec<(usize, &str)> = texte
            .lines()
            .enumerate()
            .map(|(rang, ligne)| (rang + 1, ligne.trim()))
            .filter(|(_, ligne)| !ligne.is_empty())
            .collect();

        let (rang_fond, premiere) = *lignes.first().ok_or(CompositionInvalide {
            ligne: 1,
            raison: "composition vide : il lui faut au moins sa ligne « fond »".to_owned(),
        })?;

        let reste_du_fond = premiere
            .strip_prefix("fond")
            .filter(|reste| reste.is_empty() || reste.starts_with(char::is_whitespace))
            .ok_or(CompositionInvalide {
                ligne: rang_fond,
                raison: "première ligne d'une composition : « fond noir » ou « fond image \
                         <chemin> »"
                    .to_owned(),
            })?;

        let fond = Fond::decoder(reste_du_fond).map_err(|erreur| CompositionInvalide {
            ligne: rang_fond,
            raison: erreur.raison,
        })?;

        let mut composition = Composition::nouvelle(fond);

        for (rang, ligne) in lignes.into_iter().skip(1) {
            let reste = ligne
                .strip_prefix("champ")
                .filter(|reste| reste.starts_with(char::is_whitespace))
                .ok_or(CompositionInvalide {
                    ligne: rang,
                    raison: format!(
                        "« {} » : une composition n'a que sa ligne « fond » puis des lignes \
                         « champ <ancre> <source> »",
                        ligne.escape_debug()
                    ),
                })?
                .trim_start();

            let (ancre_brute, source_brute) = match reste.split_once(char::is_whitespace) {
                Some((ancre, source)) => (ancre, source.trim()),
                None => (reste, ""),
            };

            let ancre = Ancre::depuis_slug(ancre_brute).map_err(|erreur| CompositionInvalide {
                ligne: rang,
                raison: erreur.to_string(),
            })?;

            // ⚠️ **Le doublon est refusé, jamais écrasé.** Un fichier qui pose
            // deux fois « haut » porte deux compositions sans que rien ne dise
            // laquelle : le compléter au jugé rendrait l'anomalie invisible.
            if composition.champ(ancre).is_some() {
                return Err(CompositionInvalide {
                    ligne: rang,
                    raison: format!(
                        "l'ancre « {ancre} » porte déjà un champ : le fichier en dit deux, et \
                         rien ne dit lequel compte"
                    ),
                });
            }

            let source = Source::decoder(source_brute).map_err(|erreur| CompositionInvalide {
                ligne: rang,
                raison: erreur.raison,
            })?;

            composition
                .poser(ancre, source)
                .map_err(|erreur| CompositionInvalide {
                    ligne: rang,
                    raison: erreur.to_string(),
                })?;
        }

        Ok(composition)
    }

    /// Le rang d'une ancre dans [`Ancre::TOUTES`].
    fn rang(ancre: Ancre) -> usize {
        Ancre::TOUTES
            .iter()
            .position(|connue| *connue == ancre)
            .unwrap_or(0)
    }
}

/// Un cinquième champ, refusé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TropDeChamps {
    pub plafond: usize,
}

impl fmt::Display for TropDeChamps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} champs au plus sur cette dalle : elle fait six centimètres, et un cinquième ne se \
             lirait pas — libère une ancre avec « vide <ancre> »",
            self.plafond
        )
    }
}

impl std::error::Error for TropDeChamps {}

/// Une composition qu'on n'a pas su lire, et où.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionInvalide {
    /// Relative au bloc de composition : `1` est la ligne « fond ».
    pub ligne: usize,
    pub raison: String,
}

impl fmt::Display for CompositionInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ligne {} : {}", self.ligne, self.raison)
    }
}

impl std::error::Error for CompositionInvalide {}
