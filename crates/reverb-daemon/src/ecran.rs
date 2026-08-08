//! L'écran du Kraken : ce qu'il affiche, et comment on le dessine.
//!
//! Le démon **détient** désormais la dalle. `peripheriques.rs` annonçait la
//! condition : « il rejoindra le démon quand la fenêtre en aura besoin ». La
//! fenêtre n'ouvre aucun périphérique (ADR-002), elle envoie donc un chemin de
//! fichier et le démon lit lui-même — jamais 1,2 Mo de pixels sur un protocole
//! texte.
//!
//! ⚠️ **Régression de capacité assumée** : `reverb screen --image` en direct ne
//! marche plus pendant que le démon tourne, le nœud USB ne se réclamant pas deux
//! fois. Elle est compensée — la ligne de commande passe par le socket comme la
//! fenêtre, et y gagne le PNG, le JPEG et le GIF qu'elle n'avait pas.
//!
//! # Le cadran ne dépend d'aucune pile de texte
//!
//! Des chiffres à sept segments dessinés à la main dans le tampon 640×640, plus
//! un anneau de proportion. Charger une pile de rendu de police pour afficher
//! « 40.5 » serait hors de proportion avec le besoin, et ajouterait une
//! bibliothèque système à un démon qui n'en veut pas.
//!
//! Le libellé de la sonde et son unité, eux, ne sont pas des chiffres : ils
//! passent par une police **matricielle de 5 × 7**, écrite ici colonne par
//! colonne. Ce n'est pas une pile de texte — pas de fichier de police, pas de
//! crénage, pas de dépendance —, c'est une table de 95 caractères.
//!
//! # Le firmware reprend la main au bout de trente secondes
//!
//! `screen::FIRMWARE_FALLBACK_SECS`. Ce qui est affiché doit donc être réémis
//! avant — d'où `screen::REFRESH_INTERVAL_SECS`, vingt-cinq. Un cadran s'en
//! sert pour se rafraîchir ; une image fixe aussi, sans quoi elle disparaîtrait
//! toute seule.

use std::fmt;
use std::io;
use std::path::Path;
use std::time::Duration;

use image::AnimationDecoder;
use image::ImageFormat;
use image::codecs::gif::GifDecoder;
use reverb_proto::composition::{Ancre, Boite, Composition, Fond};
use reverb_proto::screen;

use crate::persistance::ecrire;

/// Où l'état de l'écran est conservé d'un démarrage à l'autre.
pub const CHEMIN_ECRAN: &str = "/var/lib/reverb/ecran.conf";

/// Ce que la dalle montre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Affichage {
    /// Rendue au firmware.
    Rien,
    /// Une sonde, en gros, avec son unité.
    Cadran(String),
    /// Une image fixe, mise à l'échelle par le démon.
    Image(String),
    /// Une animation, jouée en boucle.
    Gif(String),
    /// Un fond, et jusqu'à quatre informations posées dessus (#80).
    Composition(Composition),
}

/// L'état de l'écran : sa luminosité et ce qu'il montre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Etat {
    /// De 0 à 100. Zéro éteint la dalle **sans** perdre ce qu'elle affichait.
    pub luminosite: u8,
    pub affichage: Affichage,
}

/// Un fichier d'écran n'a pas pu être lu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtatInvalide {
    pub ligne: usize,
    pub raison: String,
}

impl fmt::Display for EtatInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ligne == 0 {
            write!(f, "{}", self.raison)
        } else {
            write!(f, "ligne {} : {}", self.ligne, self.raison)
        }
    }
}

impl std::error::Error for EtatInvalide {}

impl Etat {
    /// Ce que la dalle montre au premier démarrage : rien, à pleine luminosité.
    pub fn accueil() -> Etat {
        Etat {
            luminosite: 100,
            affichage: Affichage::Rien,
        }
    }

    /// Le texte du fichier : une ligne `brightness`, une ligne `affiche`.
    ///
    /// **Deux lignes, sans en-tête.** Les autres fichiers du démon en portent
    /// un, en commentaires ; celui-ci ne peut pas, parce que [`Etat::decoder`]
    /// nomme la ligne fautive et qu'un en-tête décalerait les rangs qu'il
    /// annonce. Un fichier de deux lignes se relit sans mode d'emploi.
    ///
    /// Une **composition** en ajoute : `affiche layout`, puis le bloc que
    /// `Composition::encoder` produit. C'est la seule nature d'affichage qui ne
    /// tienne pas sur une ligne, et l'y forcer aurait demandé d'inventer un
    /// échappement là où le reste du projet n'en a jamais eu besoin.
    pub fn encoder(&self) -> String {
        let affichage = match &self.affichage {
            Affichage::Rien => "rien".to_owned(),
            Affichage::Cadran(sonde) => format!("gauge {sonde}"),
            Affichage::Image(chemin) => format!("image {chemin}"),
            Affichage::Gif(chemin) => format!("gif {chemin}"),
            Affichage::Composition(_) => "layout".to_owned(),
        };
        let mut texte = format!("brightness {}\naffiche {affichage}\n", self.luminosite);
        if let Affichage::Composition(composition) = &self.affichage {
            texte.push_str(&composition.encoder());
        }
        texte
    }

    /// L'inverse, strict, en nommant la ligne fautive.
    pub fn decoder(texte: &str) -> Result<Etat, EtatInvalide> {
        let lignes: Vec<&str> = texte.lines().collect();

        let premiere = lignes.first().copied().unwrap_or("");
        let luminosite = luminosite_de(premiere)?;

        let seconde = lignes.get(1).copied().ok_or(EtatInvalide {
            ligne: 2,
            raison: "ligne « affiche » manquante : le fichier dit une luminosité sans dire ce que \
                     la dalle montre"
                .to_owned(),
        })?;
        let affichage = affichage_de(seconde)?;

        // Un affichage simple s'arrête à la seconde ligne. « L'inverse,
        // **strict** » : une ligne surnuméraire signale un fichier d'une version
        // qu'on ne sait pas lire, et l'avaler ferait repartir le démon sur un
        // état amputé de ce qu'il n'a pas compris.
        //
        // ⚠️ **Une composition, elle, en attend.** C'est ce qui rend un fichier
        // d'avant #80 lisible tel quel : il ne dit jamais « layout », donc il ne
        // porte jamais de bloc, donc il se relit comme avant.
        if !matches!(affichage, Affichage::Composition(_)) {
            if lignes.len() > 2 {
                return Err(EtatInvalide {
                    ligne: 3,
                    raison: format!(
                        "ligne de trop : le fichier d'écran en a deux, celle-ci est la {}e",
                        lignes.len()
                    ),
                });
            }
            return Ok(Etat {
                luminosite,
                affichage,
            });
        }

        // `Composition::decoder` compte ses lignes depuis son propre début : il
        // ne sait pas qu'il est le troisième d'un fichier, et lui apprendre à le
        // savoir le rendrait dépendant de qui l'écrit — le profil l'écrit
        // ailleurs. Le décalage se fait donc ici, où il est connu.
        let bloc = lignes[2..].join("\n");
        let composition = Composition::decoder(&bloc).map_err(|erreur| EtatInvalide {
            ligne: erreur.ligne + 2,
            raison: erreur.raison,
        })?;

        Ok(Etat {
            luminosite,
            affichage: Affichage::Composition(composition),
        })
    }
}

/// La première ligne : `brightness <0-100>`.
fn luminosite_de(ligne: &str) -> Result<u8, EtatInvalide> {
    let refus = |raison: String| EtatInvalide { ligne: 1, raison };
    let mut mots = ligne.split_whitespace();
    match mots.next() {
        Some("brightness") => {}
        Some(autre) => {
            return Err(refus(format!(
                "« {autre} » inconnu : la première ligne est « brightness <0-100> »"
            )));
        }
        None => {
            return Err(refus(
                "ligne « brightness » manquante : le fichier d'écran commence par elle".to_owned(),
            ));
        }
    }
    let Some(brut) = mots.next() else {
        return Err(refus(
            "« brightness » attend une luminosité, de 0 à 100".to_owned(),
        ));
    };
    if mots.next().is_some() {
        return Err(refus(
            "« brightness » n'attend qu'une valeur, et rien derrière".to_owned(),
        ));
    }
    let valeur: u32 = brut.parse().map_err(|_| {
        refus(format!(
            "luminosité « {brut} » invalide : attendu un entier de 0 à {}",
            screen::BRIGHTNESS_MAX
        ))
    })?;
    if valeur > u32::from(screen::BRIGHTNESS_MAX) {
        return Err(refus(format!(
            "luminosité {valeur} hors bornes : l'écran va de 0 à {}",
            screen::BRIGHTNESS_MAX
        )));
    }
    u8::try_from(valeur).map_err(|_| refus(format!("luminosité {valeur} hors bornes")))
}

/// La seconde ligne : `affiche rien|gauge <sonde>|image <chemin>|gif <chemin>`.
///
/// Le chemin est le **dernier champ**, et prend tout ce qui reste : un
/// sélecteur de fichiers rend `Mes documents/fond d'écran.png` tous les jours,
/// et le protocole a déjà tranché que la commande le porte entier. Ce que le
/// socket accepte, le fichier doit savoir le conserver.
fn affichage_de(ligne: &str) -> Result<Affichage, EtatInvalide> {
    let refus = |raison: String| EtatInvalide { ligne: 2, raison };
    let mut mots = ligne.split_whitespace();
    match mots.next() {
        Some("affiche") => {}
        Some(autre) => {
            return Err(refus(format!(
                "« {autre} » inconnu : la seconde ligne est « affiche \
                 <rien|gauge|image|gif|layout> »"
            )));
        }
        None => {
            return Err(refus(
                "ligne « affiche » manquante : le fichier dit une luminosité sans dire ce que la \
                 dalle montre"
                    .to_owned(),
            ));
        }
    }
    let Some(quoi) = mots.next() else {
        return Err(refus(
            "« affiche » attend rien, gauge, image, gif ou layout".to_owned(),
        ));
    };
    if quoi == "rien" {
        if mots.next().is_some() {
            return Err(refus(
                "« affiche rien » n'attend aucun argument : la dalle est rendue au firmware"
                    .to_owned(),
            ));
        }
        return Ok(Affichage::Rien);
    }
    if quoi == "layout" {
        if mots.next().is_some() {
            return Err(refus(
                "« affiche layout » n'attend rien sur sa ligne : le fond et les champs viennent \
                 sur les suivantes"
                    .to_owned(),
            ));
        }
        // Une composition **vide de sens**, que [`Etat::decoder`] remplace par
        // celle du bloc qui suit. Cette fonction ne lit qu'une ligne : lui
        // passer le reste du fichier pour qu'elle en décode deux lui ferait
        // porter le cadrage du fichier, qui n'est pas son affaire.
        return Ok(Affichage::Composition(Composition::nouvelle(Fond::Noir)));
    }
    let Some(argument) = apres_le_second_mot(ligne) else {
        return Err(refus(format!(
            "« affiche {quoi} » attend un argument : une sonde pour gauge, un chemin pour image \
             et gif"
        )));
    };
    match quoi {
        "gauge" => Ok(Affichage::Cadran(argument.to_owned())),
        "image" => Ok(Affichage::Image(argument.to_owned())),
        "gif" => Ok(Affichage::Gif(argument.to_owned())),
        autre => Err(refus(format!(
            "« {autre} » inconnu : la dalle affiche rien, gauge, image, gif ou layout"
        ))),
    }
}

/// Ce qui reste d'une ligne après ses deux premiers mots, blancs de tête ôtés.
///
/// `None` quand il n'y a rien derrière. Les blancs **internes** sont gardés :
/// ce sont ceux d'un chemin, et les fusionner désignerait un autre fichier.
fn apres_le_second_mot(ligne: &str) -> Option<&str> {
    let (_, reste) = ligne.trim_start().split_once(char::is_whitespace)?;
    let (_, dernier) = reste.trim_start().split_once(char::is_whitespace)?;
    let dernier = dernier.trim_start();
    if dernier.is_empty() {
        None
    } else {
        Some(dernier)
    }
}

/// Lit le fichier d'écran, en disant ce qui a cloché plutôt qu'en échouant.
pub fn charger(chemin: &Path) -> (Etat, Option<String>) {
    let texte = match std::fs::read_to_string(chemin) {
        Ok(texte) => texte,
        // L'absence n'est pas une anomalie : c'est le premier démarrage. Un
        // message ici polluerait le journal de toute installation neuve.
        Err(erreur) if erreur.kind() == io::ErrorKind::NotFound => {
            return (Etat::accueil(), None);
        }
        Err(erreur) => {
            return (
                Etat::accueil(),
                Some(format!(
                    "écran illisible dans {} ({erreur}) : dalle rendue au firmware",
                    chemin.display()
                )),
            );
        }
    };

    match Etat::decoder(&texte) {
        Ok(etat) => (etat, None),
        Err(erreur) => (
            Etat::accueil(),
            Some(format!(
                "écran invalide dans {} ({erreur}) : dalle rendue au firmware",
                chemin.display()
            )),
        ),
    }
}

/// Écrit le fichier d'écran, par fichier provisoire puis renommage.
pub fn enregistrer(chemin: &Path, etat: &Etat) -> io::Result<()> {
    ecrire(chemin, &etat.encoder())
}

/// Une image prête pour le bus : 640×640 en BGR, dans l'ordre du Kraken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dalle {
    octets: Vec<u8>,
}

/// Une image n'a pas pu être lue ou n'a pas sa place sur la dalle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInvalide {
    pub raison: String,
}

impl fmt::Display for ImageInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raison)
    }
}

impl std::error::Error for ImageInvalide {}

/// Le format de ce fichier est-il celui que l'affichage demande ? (issue #69)
///
/// # Le défaut que ceci corrige
///
/// `/var/lib/reverb/ecran.conf` a contenu, sur SHYNAEL :
///
/// ```text
/// gif:/home/lupink/Images/Wallpapers/pxfuel.jpg
/// ```
///
/// Un **GIF déclaré sur un `.jpg`**. La commande avait été acceptée, l'état
/// écrit, et le démon rejouait cet affichage impossible à chaque démarrage du
/// service — trois fois de suite. C'est très probablement ce qui a mis la dalle
/// dans l'état où elle a cessé de répondre à son pilote noyau.
///
/// ⚠️ **Le défaut n'est pas qu'un humain se trompe de format** — la fenêtre
/// propose « image » et « gif » dans le même menu et le même champ. Le défaut
/// est qu'on puisse **persister** un état que le démon ne saura jamais
/// afficher, et donc qu'il redémarre dans un état cassé sans moyen d'en sortir.
///
/// ⚠️ **Le verdict vient du contenu, jamais de l'extension.** Un `.png` qui
/// porte du JPEG est un JPEG, et un fichier sans extension a un format comme
/// les autres.
///
/// `Image` accepte les trois formats du crate — un GIF affiché en image fixe
/// est un cas légitime, et le refuser serait une régression de capacité. `Gif`
/// n'accepte que le GIF, puisqu'il promet une animation.
pub fn verifier_format(affichage: &Affichage, octets: &[u8]) -> Result<(), ImageInvalide> {
    let (chemin, attendus) = match affichage {
        // Aucun fichier, donc aucun format à vérifier. C'est la sortie de
        // secours : `off` et `gauge` doivent rester possibles quoi qu'il arrive.
        Affichage::Rien | Affichage::Cadran(_) => return Ok(()),
        // Un fond noir non plus : c'est le fond qui ne demande rien à personne,
        // et celui vers lequel une composition retombe.
        Affichage::Composition(composition) => match composition.fond() {
            Fond::Noir => return Ok(()),
            Fond::Image(chemin) => (
                chemin,
                &[ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Gif][..],
            ),
        },
        Affichage::Image(chemin) => (
            chemin,
            &[ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Gif][..],
        ),
        Affichage::Gif(chemin) => (chemin, &[ImageFormat::Gif][..]),
    };

    // Reconnaître, et non décoder : l'en-tête suffit, et un fichier de plusieurs
    // mégaoctets ne doit pas coûter son décodage complet pour être refusé.
    match image::guess_format(octets).ok() {
        Some(format) if attendus.contains(&format) => Ok(()),
        Some(format) => Err(ImageInvalide {
            raison: format!(
                "« {chemin} » n'est pas {} : {} — essaie « {} »",
                nom_attendu(affichage),
                nom_de_format(format),
                if matches!(format, ImageFormat::Gif) {
                    "gif"
                } else {
                    "image"
                }
            ),
        }),
        None => Err(ImageInvalide {
            raison: format!("« {chemin} » : aucun format d'image reconnu"),
        }),
    }
}

/// Le même verdict, en lisant le fichier que l'affichage nomme.
///
/// C'est ce que le démon appelle : il a un chemin, pas des octets. Seuls les
/// premiers octets sont lus — reconnaître un format n'en demande pas plus, et
/// charger cinq mégaoctets pour lire un en-tête irait contre le but.
pub fn verifier_fichier(affichage: &Affichage) -> Result<(), ImageInvalide> {
    let chemin = match affichage {
        Affichage::Rien | Affichage::Cadran(_) => return Ok(()),
        Affichage::Composition(composition) => match composition.fond() {
            Fond::Noir => return Ok(()),
            Fond::Image(chemin) => chemin,
        },
        Affichage::Image(chemin) | Affichage::Gif(chemin) => chemin,
    };
    let mut fichier = std::fs::File::open(chemin).map_err(|erreur| ImageInvalide {
        raison: format!("« {chemin} » illisible : {erreur}"),
    })?;
    // Assez pour toutes les signatures que le crate reconnaît, et assez peu pour
    // que le coût ne dépende pas de la taille du fichier.
    let mut entete = [0u8; 64];
    let lus = io::Read::read(&mut fichier, &mut entete).map_err(|erreur| ImageInvalide {
        raison: format!("« {chemin} » illisible : {erreur}"),
    })?;
    verifier_format(affichage, &entete[..lus])
}

/// Ce que l'affichage attend, dit comme un humain le dirait.
fn nom_attendu(affichage: &Affichage) -> &'static str {
    match affichage {
        Affichage::Gif(_) => "un GIF",
        _ => "une image (PNG, JPEG ou GIF)",
    }
}

/// Le fond d'une composition, décodé et mis à l'échelle.
///
/// Un fond noir ne lit rien : c'est ce qui permet de composer sans image, et
/// c'est aussi le repli d'une composition dont on retire le fond.
///
/// ⚠️ Un **GIF** en fond ne garde que sa première image. Le hors-scope de #80 le
/// dit : recomposer du texte sur trente images par seconde pour une dalle de six
/// centimètres ne vaut pas son coût. Le fichier n'est pas refusé pour autant —
/// un GIF est une image valide, et le refuser serait une régression sur ce que
/// `screen image` accepte déjà.
pub fn fond_en_dalle(fond: &Fond) -> Result<Dalle, ImageInvalide> {
    match fond {
        Fond::Noir => Ok(Dalle::noire()),
        Fond::Image(chemin) => Dalle::depuis_fichier(Path::new(chemin))?
            .into_iter()
            .next()
            .ok_or_else(|| ImageInvalide {
                raison: format!("{chemin} : aucune image"),
            }),
    }
}

/// Le nom d'un format, tel qu'il doit apparaître dans un refus.
///
/// Écrit à la main plutôt que tiré du `Debug` du crate : c'est un message que
/// quelqu'un lit, et « Jpeg » n'est pas ce qu'on écrit quand on parle de JPEG.
fn nom_de_format(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "décodé comme PNG",
        ImageFormat::Jpeg => "décodé comme JPEG",
        ImageFormat::Gif => "décodé comme GIF",
        ImageFormat::WebP => "décodé comme WebP",
        ImageFormat::Bmp => "décodé comme BMP",
        ImageFormat::Tiff => "décodé comme TIFF",
        _ => "d'un format que ce démon ne sait pas afficher",
    }
}

/// Ce qu'un champ de la composition montre **au moment du rendu**.
///
/// La `Composition` dit quoi afficher ; ceci dit combien ça vaut. Les deux sont
/// séparés parce que la composition est ce qu'on conserve — elle traverse un
/// redémarrage — et que la mesure ne l'est jamais : c'est aussi ce qui permet de
/// dessiner un champ sans matériel, dans un fichier, pour vérifier qu'il se lit.
#[derive(Debug, Clone, PartialEq)]
pub enum ChampRendu {
    /// Une température, en degrés Celsius.
    ///
    /// ⚠️ `valeur: None` — la sonde ne répond plus. Le champ écrit alors des
    /// tirets, **jamais un zéro** ni la dernière valeur connue : un 34 °C figé
    /// derrière une pompe arrêtée est le mode de défaillance le plus coûteux du
    /// projet, parce qu'il est rassurant.
    Temperature {
        libelle: Option<String>,
        valeur: Option<f32>,
    },
    /// Un texte fixe.
    Texte(String),
}

/// Ce qu'un champ écrit en gros, en clair.
///
/// Pure, et publique : c'est ce qui rend « une sonde muette rend des tirets »
/// vérifiable sans compter des pixels.
pub fn valeur_du_champ(champ: &ChampRendu) -> String {
    match champ {
        ChampRendu::Temperature { valeur, .. } => chiffres(*valeur),
        ChampRendu::Texte(texte) => texte.clone(),
    }
}

impl Dalle {
    pub fn noire() -> Dalle {
        Dalle::unie((0, 0, 0))
    }

    /// Une dalle d'une seule couleur, donnée en RGB.
    pub fn unie(couleur: (u8, u8, u8)) -> Dalle {
        let triplet = screen::pixel(couleur.0, couleur.1, couleur.2);
        Dalle {
            octets: triplet
                .iter()
                .copied()
                .cycle()
                .take(screen::IMAGE_LEN)
                .collect(),
        }
    }

    /// Les octets à pousser sur l'endpoint bulk.
    ///
    /// Toujours `screen::IMAGE_LEN`, quelle que soit l'image de départ.
    pub fn octets(&self) -> &[u8] {
        &self.octets
    }

    /// Lit un fichier et le met à l'échelle de la dalle.
    ///
    /// Rend **plusieurs** dalles pour un GIF, une seule pour une image fixe.
    ///
    /// L'image est mise à l'échelle **sans déformer ses proportions**, puis
    /// centrée sur du noir : une photo étirée en carré est laide et personne ne
    /// l'a demandée.
    ///
    /// ⚠️ Le chemin doit être **absolu**. Le démon ne partage pas le répertoire
    /// courant de son client, et un chemin relatif y désignerait autre chose —
    /// ou rien.
    pub fn depuis_fichier(chemin: &Path) -> Result<Vec<Dalle>, ImageInvalide> {
        Dalle::animee_depuis_fichier(chemin).map(|(dalles, _)| dalles)
    }

    /// Comme [`Dalle::depuis_fichier`], mais en gardant les délais du GIF.
    ///
    /// Deux entrées pour un seul décodage : le délai d'une image ne vit que
    /// dans le fichier, et le relire à part demanderait de décoder deux fois un
    /// mégaoctet de pixels pour quelques microsecondes de timing. Une image
    /// fixe rend un délai vide — elle n'en a pas, et lui en inventer un ferait
    /// tourner la boucle d'animation pour rien.
    pub fn animee_depuis_fichier(
        chemin: &Path,
    ) -> Result<(Vec<Dalle>, Vec<Duration>), ImageInvalide> {
        // Avant toute lecture : un chemin relatif n'est pas un fichier
        // introuvable. Confondre les deux enverrait l'utilisateur vérifier un
        // fichier qu'il a sous les yeux, dans le répertoire d'où il a lancé sa
        // commande.
        if !chemin.is_absolute() {
            return Err(ImageInvalide {
                raison: format!(
                    "chemin « {} » relatif : le démon ne partage pas le répertoire courant de son \
                     client, il faut un chemin absolu",
                    chemin.display()
                ),
            });
        }

        let octets = std::fs::read(chemin).map_err(|erreur| ImageInvalide {
            raison: format!("{} illisible : {erreur}", chemin.display()),
        })?;

        // Le format vient des **octets**, jamais de l'extension : un fichier
        // peut s'appeler `.png` et ne pas en être un, et lancer le décodeur PNG
        // sur du GIF donnerait un refus obscur au lieu du bon.
        let format = image::guess_format(&octets).map_err(|erreur| ImageInvalide {
            raison: format!("{} d'un format inconnu : {erreur}", chemin.display()),
        })?;

        if format == ImageFormat::Gif {
            let decodeur =
                GifDecoder::new(std::io::Cursor::new(&octets)).map_err(|erreur| ImageInvalide {
                    raison: format!("{} : GIF illisible ({erreur})", chemin.display()),
                })?;
            let mut dalles = Vec::new();
            let mut delais = Vec::new();
            for image in decodeur.into_frames() {
                let image = image.map_err(|erreur| ImageInvalide {
                    raison: format!("{} : image de GIF illisible ({erreur})", chemin.display()),
                })?;
                delais.push(Duration::from(image.delay()));
                dalles.push(Dalle::depuis_rgba(&image.into_buffer()));
            }
            if dalles.is_empty() {
                return Err(ImageInvalide {
                    raison: format!("{} : GIF sans aucune image", chemin.display()),
                });
            }
            return Ok((dalles, delais));
        }

        let image = image::load_from_memory_with_format(&octets, format).map_err(|erreur| {
            ImageInvalide {
                raison: format!("{} : image illisible ({erreur})", chemin.display()),
            }
        })?;
        Ok((vec![Dalle::depuis_rgba(&image.to_rgba8())], Vec::new()))
    }

    /// Met une image à l'échelle de la dalle, sans déformer, centrée sur du noir.
    fn depuis_rgba(source: &image::RgbaImage) -> Dalle {
        let (largeur, hauteur) = (u64::from(source.width()), u64::from(source.height()));
        let (dalle_l, dalle_h) = (
            u64::from(screen::WIDTH as u32),
            u64::from(screen::HEIGHT as u32),
        );
        if largeur == 0 || hauteur == 0 {
            return Dalle::noire();
        }

        // Le rapport est comparé en **entiers croisés** plutôt qu'en flottants :
        // une source de 320 × 160 doit tomber sur 640 × 320 pile, et un arrondi
        // d'une ligne se lirait comme une bande de trop.
        let (nouvelle_l, nouvelle_h) = if largeur * dalle_h >= hauteur * dalle_l {
            (dalle_l, (hauteur * dalle_l / largeur).max(1))
        } else {
            ((largeur * dalle_h / hauteur).max(1), dalle_h)
        };
        let (nouvelle_l, nouvelle_h) = (
            u32::try_from(nouvelle_l).unwrap_or(screen::WIDTH.into()),
            u32::try_from(nouvelle_h).unwrap_or(screen::HEIGHT.into()),
        );

        let mise_a_l_echelle = image::imageops::resize(
            source,
            nouvelle_l,
            nouvelle_h,
            image::imageops::FilterType::Triangle,
        );

        let mut toile = Toile::noire();
        let gauche = (u32::from(screen::WIDTH) - nouvelle_l) / 2;
        let haut = (u32::from(screen::HEIGHT) - nouvelle_h) / 2;
        for (x, y, pixel) in mise_a_l_echelle.enumerate_pixels() {
            let [r, v, b, _] = pixel.0;
            toile.poser((gauche + x) as usize, (haut + y) as usize, (r, v, b));
        }
        toile.dalle()
    }

    /// Le cadran d'une sonde.
    ///
    /// `valeur` absente quand la sonde ne répond plus : la dalle le **dit** au
    /// lieu de figer la dernière valeur lue.
    ///
    /// `proportion` sert l'anneau qui entoure le chiffre, de 0 à 1 ; hors de ces
    /// bornes, il est ramené dedans plutôt que de déborder.
    pub fn cadran(libelle: &str, valeur: Option<f32>, unite: &str, proportion: f32) -> Dalle {
        let mut toile = Toile::noire();

        // `f32::clamp` rendrait `NaN` tel quel, et un index calculé dessus
        // sortirait du tampon. Le `NaN` vient d'un calcul très ordinaire :
        // température divisée par une borne maximale valant zéro.
        let part = if proportion.is_nan() {
            0.0
        } else {
            proportion.clamp(0.0, 1.0)
        };

        toile.anneau(part);

        // La valeur, en gros, au centre. Les chiffres sont à sept segments : le
        // seul alphabet dont on ait besoin pour un nombre.
        let texte = chiffres(valeur);
        toile.sept_segments(&texte, CADRAN_CHIFFRE_HAUTEUR);

        // L'unité juste sous les chiffres, le libellé au-dessus.
        toile.matriciel(unite, CADRAN_UNITE_Y, CADRAN_UNITE_ECHELLE, COULEUR_UNITE);
        toile.matriciel(
            libelle,
            CADRAN_LIBELLE_Y,
            CADRAN_LIBELLE_ECHELLE,
            COULEUR_LIBELLE,
        );

        toile.dalle()
    }

    /// Le fond recopié, les champs dessinés dessus (#80).
    ///
    /// ⚠️ **Sans aucun champ, rend le fond inchangé, octet pour octet.** C'est
    /// le critère qui garantit qu'ajouter la composition ne change rien à ce
    /// qu'une image affiche aujourd'hui : une composition vide *est* l'image.
    ///
    /// ⚠️ **Aucun pixel n'est écrit hors du disque visible.** Chaque champ est
    /// borné à sa boîte, et les cinq boîtes tiennent dans le disque — ce que
    /// `Boite::dans_le_disque` vérifie plutôt que d'en faire la promesse.
    pub fn composee(fond: &Dalle, champs: &[(Ancre, ChampRendu)]) -> Dalle {
        let mut toile = Toile {
            octets: fond.octets.clone(),
        };
        for (ancre, rendu) in champs {
            toile.champ(ancre.boite(), rendu);
        }
        toile.dalle()
    }
}

/// Les délais d'un GIF, ramenés à ce que le bus tient.
///
/// Une image de 1,2 Mo met environ cent millisecondes à passer : un GIF à
/// trente images par seconde demanderait trois fois le débit disponible. On le
/// **ralentit** au lieu de sauter des images — un mouvement lent et complet se
/// regarde, un mouvement saccadé non.
///
/// C'est un **plancher**, pas un facteur : un délai déjà au-dessus traverse
/// intact, et un GIF dont une image tient trois secondes garde ses trois
/// secondes.
pub fn cadence(delais: &[Duration], plancher: Duration) -> Vec<Duration> {
    delais.iter().map(|delai| (*delai).max(plancher)).collect()
}

// ---------------------------------------------------------------------------
// Le plafond d'échecs de la dalle (issue #70)
// ---------------------------------------------------------------------------

/// Le nombre d'échecs **consécutifs** après lequel le démon renonce à la dalle.
///
/// Trois, et la valeur se raisonne : à cinq secondes de délai par tentative —
/// c'est ce qu'a mesuré #68 sur un Kraken qui ne répond plus —, trois refus
/// valent quinze secondes perdues avant de rendre la main. Assez pour qu'un
/// contrôleur qui bafouille une fois ou deux s'en remette, trop peu pour qu'une
/// dalle morte gèle le démon une minute entière.
pub const ECHECS_AVANT_ABANDON: u32 = 3;

/// Ce qu'un tour de boucle a fait de la dalle.
///
/// ⚠️ **Ce vocabulaire ne parle que de la dalle.** Ni l'état persisté, ni
/// l'éclairage, ni les ventilateurs, ni les zones n'y apparaissent — renoncer à
/// un écran n'est pas une raison d'éteindre un boîtier, et `ecran.conf` garde ce
/// qu'on voulait afficher pour qu'un redémarrage le retente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// L'image est partie.
    Emise,
    /// L'écriture a échoué, sous le plafond : l'émission continue.
    Refusee { erreur: String },
    /// L'abandon vient d'être prononcé. Rendu **une seule fois** par épisode,
    /// pour que l'appelant journalise une ligne sans avoir à s'en souvenir.
    Abandon { erreur: String },
    /// L'abandon est déjà prononcé : rien n'a été écrit, rien n'est à dire.
    Repos,
}

/// Ce qui compte les refus de la dalle et finit par renoncer.
///
/// # Le défaut que ceci corrige
///
/// Le démon réémettait sans fin vers un Kraken qui refusait — une image fixe
/// toutes les 25 s, un cadran toutes les 2 s, un GIF toutes les 100 ms — en
/// journalisant `Connection timed out (os error 110)` à chaque tour. Du bus
/// consommé, cinq secondes de gel par tentative, et une insistance sur un
/// contrôleur déjà en difficulté.
///
/// ⚠️ **Le compte est celui d'une suite, jamais d'un total.** Un compteur cumulé
/// passe tous les essais courts et ne se trahit qu'après des jours : quelques
/// hoquets sans conséquence, espacés de dix minutes, finissent par éteindre un
/// écran qui marche, et personne ne relie la panne à sa cause.
///
/// ⚠️ **Après l'abandon, l'écriture n'est plus tentée du tout** — pas même « pour
/// voir si ça remarche ». Le rétablissement est explicite, par [`Vigie::relancer`],
/// ou n'est pas : une dalle qui ne répond plus au noyau lui-même ne se répare pas
/// en réessayant.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Vigie {
    echecs: u32,
    abandonnee: bool,
}

impl Vigie {
    /// Une vigie qui n'a essuyé aucun refus : elle émet.
    pub fn neuve() -> Vigie {
        Vigie::default()
    }

    /// Un tour de boucle.
    ///
    /// `pousser` n'est appelée **que si** l'émission est encore permise. C'est
    /// une propriété du type et non une politesse de son appelant : la calculer
    /// puis jeter le résultat consommerait le bus qu'on cherche à laisser
    /// tranquille.
    pub fn tour(&mut self, pousser: impl FnOnce() -> io::Result<()>) -> Verdict {
        if self.abandonnee {
            return Verdict::Repos;
        }
        match pousser() {
            Ok(()) => {
                self.echecs = 0;
                Verdict::Emise
            }
            Err(erreur) => {
                let erreur = erreur.to_string();
                self.echecs += 1;
                if self.echecs >= ECHECS_AVANT_ABANDON {
                    self.abandonnee = true;
                    // La **dernière** erreur, pas la première : c'est elle qui a
                    // fait déborder le compte, et la plus récente information
                    // qu'on ait sur l'état du bus.
                    Verdict::Abandon { erreur }
                } else {
                    Verdict::Refusee { erreur }
                }
            }
        }
    }

    /// Une commande `screen` explicite : l'émission reprend, le compte repart de
    /// zéro.
    ///
    /// Sans effet visible sur une vigie qui n'a jamais abandonné, hors la remise
    /// à zéro — une commande `screen` est fréquente, elle ne doit rien casser.
    pub fn relancer(&mut self) {
        self.echecs = 0;
        self.abandonnee = false;
    }

    /// La dalle est-elle encore servie ?
    pub fn emet(&self) -> bool {
        !self.abandonnee
    }

    /// Combien de refus d'affilée, depuis le dernier succès.
    pub fn echecs_consecutifs(&self) -> u32 {
        self.echecs
    }
}

// ---------------------------------------------------------------------------
// Le dessin
// ---------------------------------------------------------------------------

/// Hauteur des chiffres du cadran, en pixels.
///
/// Les chiffres sont centrés verticalement : ils occupent donc les rangs 225 à
/// 415, ce dont les deux constantes suivantes doivent se tenir à l'écart.
const CADRAN_CHIFFRE_HAUTEUR: usize = 190;
/// Largeur dont les chiffres disposent à l'intérieur de l'anneau.
const CADRAN_LARGEUR_UTILE: usize = 470;
/// En deçà, on ne réduit plus : mieux vaut un nombre qui dépasse qu'un nombre
/// qu'on ne lit plus. Un cadran illisible ne dit rien.
const CADRAN_CHIFFRE_MINIMUM: usize = 70;
/// Rang de la ligne de l'unité, **sous** les chiffres.
const CADRAN_UNITE_Y: usize = 432;
/// Échelle de l'unité : elle se lit d'un mètre, comme les chiffres.
const CADRAN_UNITE_ECHELLE: usize = 4;
/// Rang de la ligne du libellé, **au-dessus** des chiffres et dans l'anneau.
///
/// À ce rang, le disque intérieur de l'anneau est large de quelque 430 pixels :
/// un nom de sonde de trente caractères à l'échelle 2 en occupe 360, et reste
/// donc dedans.
const CADRAN_LIBELLE_Y: usize = 168;
/// Échelle du libellé. Deux, et pas davantage : c'est la valeur qu'on lit d'un
/// mètre, pas le nom de la sonde, qu'on lit en se penchant.
const CADRAN_LIBELLE_ECHELLE: usize = 2;

/// Rayons de l'anneau de proportion, en pixels depuis le centre.
const ANNEAU_INTERIEUR: f32 = 262.0;
const ANNEAU_EXTERIEUR: f32 = 300.0;

/// La dalle entière, quand un dessin n'a pas à être borné plus étroitement.
const TOUTE_LA_DALLE: Boite = Boite {
    x: 0,
    y: 0,
    largeur: screen::WIDTH,
    hauteur: screen::HEIGHT,
};

/// Ce qu'il reste d'un fond sous un champ, en pour cent.
///
/// Trente : un blanc pur y tombe à 76, où du texte blanc se détache nettement,
/// et un fond déjà sombre ne s'écroule pas à zéro — la photo reste devinable
/// sous le champ.
const CHAMP_ASSOMBRISSEMENT: u16 = 30;

/// Hauteur des chiffres d'un champ. Le tiers de ceux du cadran : quatre valeurs
/// à la taille d'une seule ne tiendraient pas dans un disque de 640 pixels.
const CHAMP_CHIFFRE_HAUTEUR: usize = 48;
/// Échelle du libellé d'un champ.
const CHAMP_LIBELLE_ECHELLE: usize = 2;
/// Échelle de l'unité d'un champ.
const CHAMP_UNITE_ECHELLE: usize = 3;
/// Échelle d'un champ de texte fixe : il n'a rien d'autre à montrer, donc il
/// prend la place que la mesure aurait prise.
const CHAMP_TEXTE_ECHELLE: usize = 3;
/// Entre le libellé et la mesure.
const CHAMP_INTERLIGNE: usize = 6;
/// Entre les chiffres et leur unité.
const CHAMP_ECART_UNITE: usize = 8;

/// Le libellé d'un champ, sur son fond assombri.
///
/// Plus clair que le libellé du cadran, qui se pose sur du noir : ici le fond
/// est ce qu'il reste d'une photo, et un gris sombre s'y noierait — mesuré sur
/// un fond blanc, où l'assombrissement laisse 76.
const COULEUR_CHAMP_LIBELLE: (u8, u8, u8) = (0xc4, 0xc6, 0xd4);

/// L'unité d'un champ de température.
///
/// Le degré n'est pas de l'ASCII et la police matricielle le porte quand même :
/// c'est l'unité qui compte le plus sur cette dalle.
const UNITE: &str = "°C";

const COULEUR_CHIFFRE: (u8, u8, u8) = (0xff, 0xff, 0xff);
const COULEUR_UNITE: (u8, u8, u8) = (0x9a, 0x9c, 0xb0);
const COULEUR_LIBELLE: (u8, u8, u8) = (0x6f, 0x71, 0x80);
/// La part parcourue de l'anneau.
const COULEUR_ANNEAU: (u8, u8, u8) = (0x30, 0xa0, 0xff);
/// Le reste de la piste, pour que l'anneau vide se distingue d'un écran noir.
const COULEUR_PISTE: (u8, u8, u8) = (0x1c, 0x1e, 0x28);

/// Le texte des chiffres du cadran.
///
/// Une sonde muette rend des tirets, **jamais un zéro** : « 0 °C » affiché pour
/// une sonde qui ne répond plus est un mensonge, pas une valeur par défaut, et
/// c'est le mode de défaillance le plus coûteux du cadran parce qu'il est
/// rassurant.
fn chiffres(valeur: Option<f32>) -> String {
    match valeur {
        None => "---".to_owned(),
        // Ni un nombre, ni une absence de sonde : une case vide, qui ne se lit
        // pas comme une mesure.
        Some(valeur) if !valeur.is_finite() => " ".to_owned(),
        Some(valeur) => {
            let mut texte = format!("{valeur:.1}");
            // Une valeur immense ne doit pas déborder du tampon : le cadran en
            // montre ce qui tient, et la dalle garde sa taille.
            texte.truncate(6);
            texte
        }
    }
}

/// Un tampon 640×640 qu'on peint avant de le rendre au bus.
///
/// ⚠️ Les octets sont écrits **dans l'ordre de l'écran** dès la pose, par
/// `screen::pixel` — la seule fonction du projet qui connaisse cet ordre. Une
/// image en RGB au lieu de BGR s'affiche nette, et de la mauvaise couleur, sans
/// qu'aucun message ne le dise.
struct Toile {
    octets: Vec<u8>,
}

impl Toile {
    fn noire() -> Toile {
        Toile {
            octets: vec![0u8; screen::IMAGE_LEN],
        }
    }

    fn dalle(self) -> Dalle {
        Dalle {
            octets: self.octets,
        }
    }

    /// Peint un pixel. Hors dalle, ne fait rien — le débordement d'un cadran ne
    /// doit ni paniquer ni tronquer le tampon.
    fn poser(&mut self, x: usize, y: usize, couleur: (u8, u8, u8)) {
        if x >= usize::from(screen::WIDTH) || y >= usize::from(screen::HEIGHT) {
            return;
        }
        let debut = (y * usize::from(screen::WIDTH) + x) * screen::PIXEL_LEN;
        let triplet = screen::pixel(couleur.0, couleur.1, couleur.2);
        self.octets[debut..debut + screen::PIXEL_LEN].copy_from_slice(&triplet);
    }

    /// Peint un rectangle, **borné** à la boîte donnée.
    ///
    /// ⚠️ Le bornage n'est pas une précaution de style : la boîte du champ
    /// supérieur passe à 2,4 pixels du bord du disque visible, et un arrondi de
    /// centrage qui déborderait d'un pixel écrirait hors de la dalle — sans
    /// qu'aucun message ne le dise, puisque le tampon, lui, est carré.
    fn rectangle(
        &mut self,
        clip: Boite,
        x: usize,
        y: usize,
        largeur: usize,
        hauteur: usize,
        c: (u8, u8, u8),
    ) {
        let (x0, y0) = (usize::from(clip.x), usize::from(clip.y));
        let (x1, y1) = (
            x0 + usize::from(clip.largeur),
            y0 + usize::from(clip.hauteur),
        );
        for py in y.max(y0)..(y + hauteur).min(y1) {
            for px in x.max(x0)..(x + largeur).min(x1) {
                self.poser(px, py, c);
            }
        }
    }

    /// Assombrit ce qui est déjà peint dans cette boîte.
    ///
    /// ⚠️ **C'est ce qui rend un champ lisible sur n'importe quel fond.** Une
    /// photo claire avale du texte blanc, et une couleur de texte qu'on
    /// « espère contrastée » n'est pas une garantie : la seule qui en soit une
    /// est de décider soi-même du fond derrière les caractères.
    ///
    /// Assombrir plutôt que peindre un aplat : la photo reste devinable sous le
    /// champ, ce qu'un rectangle noir opaque perdrait sur une dalle de six
    /// centimètres où le fond est justement ce qu'on a choisi de montrer.
    fn assombrir(&mut self, boite: Boite) {
        let (x0, y0) = (usize::from(boite.x), usize::from(boite.y));
        let (x1, y1) = (
            x0 + usize::from(boite.largeur),
            y0 + usize::from(boite.hauteur),
        );
        for y in y0..y1.min(usize::from(screen::HEIGHT)) {
            for x in x0..x1.min(usize::from(screen::WIDTH)) {
                let debut = (y * usize::from(screen::WIDTH) + x) * screen::PIXEL_LEN;
                let (r, v, b) = screen::composantes(&self.octets[debut..debut + screen::PIXEL_LEN]);
                let sombre =
                    |composante: u8| ((u16::from(composante) * CHAMP_ASSOMBRISSEMENT) / 100) as u8;
                self.poser(x, y, (sombre(r), sombre(v), sombre(b)));
            }
        }
    }

    /// L'anneau de proportion, parcouru depuis midi dans le sens horaire.
    fn anneau(&mut self, part: f32) {
        let centre = f64::from(screen::WIDTH) / 2.0;
        let (interieur, exterieur) = (f64::from(ANNEAU_INTERIEUR), f64::from(ANNEAU_EXTERIEUR));
        let parcourue = f64::from(part) * std::f64::consts::TAU;
        for y in 0..usize::from(screen::HEIGHT) {
            for x in 0..usize::from(screen::WIDTH) {
                let (dx, dy) = (x as f64 - centre + 0.5, y as f64 - centre + 0.5);
                let rayon = dx.hypot(dy);
                if rayon < interieur || rayon > exterieur {
                    continue;
                }
                // Zéro à midi, croissant dans le sens horaire.
                let mut angle = dx.atan2(-dy);
                if angle < 0.0 {
                    angle += std::f64::consts::TAU;
                }
                self.poser(
                    x,
                    y,
                    if angle <= parcourue {
                        COULEUR_ANNEAU
                    } else {
                        COULEUR_PISTE
                    },
                );
            }
        }
    }

    /// Le nombre, en chiffres à sept segments, centré sur la dalle.
    ///
    /// La hauteur demandée est **réduite** tant que le nombre déborde de
    /// l'anneau : « 1250.0 » a deux chiffres de plus que « 34.2 », et les
    /// laisser sortir du cercle ferait un cadran illisible là où la mesure
    /// compte le plus — les tours par minute d'une pompe.
    fn sept_segments(&mut self, texte: &str, hauteur: usize) {
        let mut hauteur = hauteur;
        while hauteur > CADRAN_CHIFFRE_MINIMUM
            && largeur_des_chiffres(texte, hauteur) > CADRAN_LARGEUR_UTILE
        {
            hauteur -= 2;
        }

        let total = largeur_des_chiffres(texte, hauteur);
        let x = usize::from(screen::WIDTH).saturating_sub(total) / 2;
        let y = usize::from(screen::HEIGHT).saturating_sub(hauteur) / 2;
        self.sept_segments_a(TOUTE_LA_DALLE, x, y, texte, hauteur);
    }

    /// Le même nombre, posé où on le demande et borné à une boîte.
    ///
    /// La hauteur n'est **pas** réduite ici : c'est l'appelant qui a choisi une
    /// place, et la réduire sous ses pieds décalerait ce qu'il a centré. Un
    /// nombre trop large est coupé par le bornage, comme le reste.
    fn sept_segments_a(&mut self, clip: Boite, x: usize, y: usize, texte: &str, hauteur: usize) {
        let largeur = largeur_de_chiffre(hauteur);
        let epaisseur = epaisseur_de_chiffre(hauteur);
        let ecart = epaisseur;
        let mut x = x;

        for glyphe in texte.chars() {
            if glyphe == '.' {
                self.rectangle(
                    clip,
                    x,
                    y + hauteur - epaisseur,
                    epaisseur * 2,
                    epaisseur,
                    COULEUR_CHIFFRE,
                );
                x += epaisseur * 2 + ecart;
                continue;
            }
            self.chiffre(clip, x, y, hauteur, segments(glyphe));
            x += largeur + ecart;
        }
    }

    /// Un chiffre à sept segments, `a` en poids faible jusqu'à `g`.
    ///
    /// La largeur et l'épaisseur se **déduisent** de la hauteur plutôt que de
    /// s'ajouter aux arguments : elles n'ont jamais eu d'autre valeur, et deux
    /// endroits qui les recalculent finissent par ne plus les recalculer
    /// pareil.
    fn chiffre(&mut self, clip: Boite, x: usize, y: usize, hauteur: usize, masque: u8) {
        let largeur = largeur_de_chiffre(hauteur);
        let epaisseur = epaisseur_de_chiffre(hauteur);
        let milieu = y + hauteur / 2 - epaisseur / 2;
        let bas = y + hauteur - epaisseur;
        let droite = x + largeur - epaisseur;
        let demi = hauteur / 2;
        let barres: [(bool, usize, usize, usize, usize); 7] = [
            (masque & 0x01 != 0, x, y, largeur, epaisseur),
            (masque & 0x02 != 0, droite, y, epaisseur, demi),
            (masque & 0x04 != 0, droite, y + demi, epaisseur, demi),
            (masque & 0x08 != 0, x, bas, largeur, epaisseur),
            (masque & 0x10 != 0, x, y + demi, epaisseur, demi),
            (masque & 0x20 != 0, x, y, epaisseur, demi),
            (masque & 0x40 != 0, x, milieu, largeur, epaisseur),
        ];
        for (allume, bx, by, bl, bh) in barres {
            if allume {
                self.rectangle(clip, bx, by, bl, bh, COULEUR_CHIFFRE);
            }
        }
    }

    /// Du texte en police matricielle 5 × 7, centré horizontalement.
    ///
    /// `echelle` est le côté, en pixels, d'un point de la matrice. Le texte qui
    /// ne tient pas dans la dalle est **coupé**, jamais replié : un cadran se
    /// lit d'un coup d'œil, pas sur deux lignes.
    fn matriciel(&mut self, texte: &str, y: usize, echelle: usize, c: (u8, u8, u8)) {
        self.matriciel_centre(TOUTE_LA_DALLE, texte, y, echelle, c);
    }

    /// Le même texte, centré dans une boîte et borné à elle.
    ///
    /// ⚠️ Le texte est **coupé à ce qui tient dans la boîte**, jamais replié :
    /// un champ se lit d'un coup d'œil, et deux lignes dans une boîte prévue
    /// pour une écraseraient la valeur qu'elle porte.
    fn matriciel_centre(
        &mut self,
        boite: Boite,
        texte: &str,
        y: usize,
        echelle: usize,
        c: (u8, u8, u8),
    ) {
        let visible = tronque(texte, usize::from(boite.largeur), echelle);
        if visible.is_empty() {
            return;
        }
        let total = largeur_matricielle(&visible, echelle);
        let x = usize::from(boite.x) + (usize::from(boite.largeur).saturating_sub(total)) / 2;
        self.matriciel_a(boite, x, y, &visible, echelle, c);
    }

    /// Le texte posé où on le demande, borné à la boîte.
    fn matriciel_a(
        &mut self,
        clip: Boite,
        x: usize,
        y: usize,
        texte: &str,
        echelle: usize,
        c: (u8, u8, u8),
    ) {
        let pas = (POLICE_LARGEUR + 1) * echelle;
        let mut x = x;
        for glyphe in texte.chars() {
            let colonnes = matrice(glyphe);
            for (dx, colonne) in colonnes.iter().enumerate() {
                for dy in 0..POLICE_HAUTEUR {
                    if colonne & (1 << dy) != 0 {
                        self.rectangle(
                            clip,
                            x + dx * echelle,
                            y + dy * echelle,
                            echelle,
                            echelle,
                            c,
                        );
                    }
                }
            }
            x += pas;
        }
    }

    /// Un champ de la composition, dessiné dans sa boîte (#80).
    fn champ(&mut self, boite: Boite, rendu: &ChampRendu) {
        self.assombrir(boite);

        match rendu {
            ChampRendu::Texte(texte) => {
                let y = usize::from(boite.y)
                    + usize::from(boite.hauteur)
                        .saturating_sub(POLICE_HAUTEUR * CHAMP_TEXTE_ECHELLE)
                        / 2;
                self.matriciel_centre(boite, texte, y, CHAMP_TEXTE_ECHELLE, COULEUR_CHIFFRE);
            }
            ChampRendu::Temperature { libelle, valeur: _ } => {
                let mesure = valeur_du_champ(rendu);
                let hauteur_libelle = POLICE_HAUTEUR * CHAMP_LIBELLE_ECHELLE;

                // Sans libellé, la mesure occupe seule la boîte et s'y centre.
                // Avec, les deux forment un bloc qu'on centre ensemble : une
                // mesure qui garderait sa place laisserait le libellé pendre
                // au-dessus de la boîte.
                let (y_libelle, y_mesure) = if libelle.is_some() {
                    let bloc = hauteur_libelle + CHAMP_INTERLIGNE + CHAMP_CHIFFRE_HAUTEUR;
                    let haut =
                        usize::from(boite.y) + usize::from(boite.hauteur).saturating_sub(bloc) / 2;
                    (haut, haut + hauteur_libelle + CHAMP_INTERLIGNE)
                } else {
                    (
                        0,
                        usize::from(boite.y)
                            + usize::from(boite.hauteur).saturating_sub(CHAMP_CHIFFRE_HAUTEUR) / 2,
                    )
                };

                if let Some(libelle) = libelle {
                    self.matriciel_centre(
                        boite,
                        libelle,
                        y_libelle,
                        CHAMP_LIBELLE_ECHELLE,
                        COULEUR_CHAMP_LIBELLE,
                    );
                }

                // Les chiffres et leur unité forment un groupe qu'on centre
                // d'un bloc : centrer les chiffres seuls ferait glisser le
                // « °C » hors de la boîte sur les valeurs longues.
                let largeur_chiffres = largeur_des_chiffres(&mesure, CHAMP_CHIFFRE_HAUTEUR);
                let largeur_unite = largeur_matricielle(UNITE, CHAMP_UNITE_ECHELLE);
                let total = largeur_chiffres + CHAMP_ECART_UNITE + largeur_unite;
                let x = usize::from(boite.x) + usize::from(boite.largeur).saturating_sub(total) / 2;

                self.sept_segments_a(boite, x, y_mesure, &mesure, CHAMP_CHIFFRE_HAUTEUR);
                // L'unité s'aligne sur le **bas** des chiffres : alignée en
                // haut, elle flotterait au-dessus d'un nombre deux fois plus
                // haut qu'elle.
                let y_unite =
                    y_mesure + CHAMP_CHIFFRE_HAUTEUR - POLICE_HAUTEUR * CHAMP_UNITE_ECHELLE;
                self.matriciel_a(
                    boite,
                    x + largeur_chiffres + CHAMP_ECART_UNITE,
                    y_unite,
                    UNITE,
                    CHAMP_UNITE_ECHELLE,
                    COULEUR_UNITE,
                );
            }
        }
    }
}

/// Ce qui tient d'un texte dans une largeur, en police matricielle.
fn tronque(texte: &str, largeur: usize, echelle: usize) -> String {
    let pas = (POLICE_LARGEUR + 1) * echelle;
    let tenables = largeur.checked_div(pas).unwrap_or(0);
    texte.chars().take(tenables).collect()
}

/// La largeur qu'occupe un texte matriciel, sans l'écart de fin.
fn largeur_matricielle(texte: &str, echelle: usize) -> usize {
    let compte = texte.chars().count();
    if compte == 0 {
        return 0;
    }
    compte * (POLICE_LARGEUR + 1) * echelle - echelle
}

/// La largeur qu'occupera un nombre à cette hauteur de chiffre.
///
/// Un point ne prend pas la place d'un chiffre : le compter comme tel
/// décalerait « 34.2 » d'un demi-caractère vers la gauche.
fn largeur_des_chiffres(texte: &str, hauteur: usize) -> usize {
    let largeur = largeur_de_chiffre(hauteur);
    let epaisseur = epaisseur_de_chiffre(hauteur);
    texte
        .chars()
        .map(|glyphe| {
            epaisseur
                + if glyphe == '.' {
                    epaisseur * 2
                } else {
                    largeur
                }
        })
        .sum::<usize>()
        .saturating_sub(epaisseur)
}

/// La largeur d'un chiffre à sept segments, à cette hauteur.
///
/// La moitié de sa hauteur — les proportions d'un afficheur, et la seule
/// définition que le projet en donne.
fn largeur_de_chiffre(hauteur: usize) -> usize {
    hauteur / 2
}

/// L'épaisseur d'un segment, à cette hauteur.
///
/// Un dixième, et jamais moins de deux pixels : en deçà, un segment disparaît
/// à l'arrondi et le chiffre change de valeur à l'œil.
fn epaisseur_de_chiffre(hauteur: usize) -> usize {
    (hauteur / 10).max(2)
}

/// Les sept segments d'un caractère, `a` en poids faible.
///
/// Ce qui n'est pas un chiffre ni un tiret s'écrit vide plutôt que de prendre
/// la forme d'un autre caractère : un « 8 » mis à la place d'un caractère
/// inconnu se lirait comme une mesure.
fn segments(glyphe: char) -> u8 {
    match glyphe {
        '0' => 0x3f,
        '1' => 0x06,
        '2' => 0x5b,
        '3' => 0x4f,
        '4' => 0x66,
        '5' => 0x6d,
        '6' => 0x7d,
        '7' => 0x07,
        '8' => 0x7f,
        '9' => 0x6f,
        '-' => 0x40,
        _ => 0x00,
    }
}

/// Largeur d'un glyphe matriciel, en points.
const POLICE_LARGEUR: usize = 5;
/// Hauteur d'un glyphe matriciel, en points.
const POLICE_HAUTEUR: usize = 7;

/// Les colonnes d'un glyphe, poids faible en haut.
///
/// Table écrite à la main pour l'ASCII imprimable, plus le degré. Ce n'est pas
/// une pile de texte : pas de fichier de police, pas de crénage, pas de
/// dépendance — 95 entrées de cinq octets.
fn matrice(glyphe: char) -> [u8; POLICE_LARGEUR] {
    // Le degré n'est pas de l'ASCII, et c'est l'unité de la sonde qui compte le
    // plus sur cette dalle.
    if glyphe == '°' {
        return [0x00, 0x06, 0x09, 0x09, 0x06];
    }
    let rang = match u32::from(glyphe) {
        code @ 32..=126 => (code - 32) as usize,
        // Tout le reste — accents, emoji, caractères de contrôle — prend le
        // rectangle vide de la fin de table : une case qui se voit, plutôt
        // qu'un caractère inventé.
        _ => POLICE.len() - 1,
    };
    POLICE[rang]
}

#[rustfmt::skip]
const POLICE: [[u8; POLICE_LARGEUR]; 96] = [
    [0x00, 0x00, 0x00, 0x00, 0x00], // espace
    [0x00, 0x00, 0x5f, 0x00, 0x00], // !
    [0x00, 0x07, 0x00, 0x07, 0x00], // "
    [0x14, 0x7f, 0x14, 0x7f, 0x14], // #
    [0x24, 0x2a, 0x7f, 0x2a, 0x12], // $
    [0x23, 0x13, 0x08, 0x64, 0x62], // %
    [0x36, 0x49, 0x55, 0x22, 0x50], // &
    [0x00, 0x05, 0x03, 0x00, 0x00], // '
    [0x00, 0x1c, 0x22, 0x41, 0x00], // (
    [0x00, 0x41, 0x22, 0x1c, 0x00], // )
    [0x14, 0x08, 0x3e, 0x08, 0x14], // *
    [0x08, 0x08, 0x3e, 0x08, 0x08], // +
    [0x00, 0x50, 0x30, 0x00, 0x00], // ,
    [0x08, 0x08, 0x08, 0x08, 0x08], // -
    [0x00, 0x60, 0x60, 0x00, 0x00], // .
    [0x20, 0x10, 0x08, 0x04, 0x02], // /
    [0x3e, 0x51, 0x49, 0x45, 0x3e], // 0
    [0x00, 0x42, 0x7f, 0x40, 0x00], // 1
    [0x42, 0x61, 0x51, 0x49, 0x46], // 2
    [0x21, 0x41, 0x45, 0x4b, 0x31], // 3
    [0x18, 0x14, 0x12, 0x7f, 0x10], // 4
    [0x27, 0x45, 0x45, 0x45, 0x39], // 5
    [0x3c, 0x4a, 0x49, 0x49, 0x30], // 6
    [0x01, 0x71, 0x09, 0x05, 0x03], // 7
    [0x36, 0x49, 0x49, 0x49, 0x36], // 8
    [0x06, 0x49, 0x49, 0x29, 0x1e], // 9
    [0x00, 0x36, 0x36, 0x00, 0x00], // :
    [0x00, 0x56, 0x36, 0x00, 0x00], // ;
    [0x00, 0x08, 0x14, 0x22, 0x41], // <
    [0x14, 0x14, 0x14, 0x14, 0x14], // =
    [0x41, 0x22, 0x14, 0x08, 0x00], // >
    [0x02, 0x01, 0x51, 0x09, 0x06], // ?
    [0x32, 0x49, 0x79, 0x41, 0x3e], // @
    [0x7e, 0x11, 0x11, 0x11, 0x7e], // A
    [0x7f, 0x49, 0x49, 0x49, 0x36], // B
    [0x3e, 0x41, 0x41, 0x41, 0x22], // C
    [0x7f, 0x41, 0x41, 0x22, 0x1c], // D
    [0x7f, 0x49, 0x49, 0x49, 0x41], // E
    [0x7f, 0x09, 0x09, 0x09, 0x01], // F
    [0x3e, 0x41, 0x49, 0x49, 0x7a], // G
    [0x7f, 0x08, 0x08, 0x08, 0x7f], // H
    [0x00, 0x41, 0x7f, 0x41, 0x00], // I
    [0x20, 0x40, 0x41, 0x3f, 0x01], // J
    [0x7f, 0x08, 0x14, 0x22, 0x41], // K
    [0x7f, 0x40, 0x40, 0x40, 0x40], // L
    [0x7f, 0x02, 0x0c, 0x02, 0x7f], // M
    [0x7f, 0x04, 0x08, 0x10, 0x7f], // N
    [0x3e, 0x41, 0x41, 0x41, 0x3e], // O
    [0x7f, 0x09, 0x09, 0x09, 0x06], // P
    [0x3e, 0x41, 0x51, 0x21, 0x5e], // Q
    [0x7f, 0x09, 0x19, 0x29, 0x46], // R
    [0x46, 0x49, 0x49, 0x49, 0x31], // S
    [0x01, 0x01, 0x7f, 0x01, 0x01], // T
    [0x3f, 0x40, 0x40, 0x40, 0x3f], // U
    [0x1f, 0x20, 0x40, 0x20, 0x1f], // V
    [0x7f, 0x20, 0x18, 0x20, 0x7f], // W
    [0x63, 0x14, 0x08, 0x14, 0x63], // X
    [0x03, 0x04, 0x78, 0x04, 0x03], // Y
    [0x61, 0x51, 0x49, 0x45, 0x43], // Z
    [0x00, 0x7f, 0x41, 0x41, 0x00], // [
    [0x02, 0x04, 0x08, 0x10, 0x20], // \
    [0x00, 0x41, 0x41, 0x7f, 0x00], // ]
    [0x04, 0x02, 0x01, 0x02, 0x04], // ^
    [0x40, 0x40, 0x40, 0x40, 0x40], // _
    [0x00, 0x01, 0x02, 0x04, 0x00], // `
    [0x20, 0x54, 0x54, 0x54, 0x78], // a
    [0x7f, 0x48, 0x44, 0x44, 0x38], // b
    [0x38, 0x44, 0x44, 0x44, 0x20], // c
    [0x38, 0x44, 0x44, 0x48, 0x7f], // d
    [0x38, 0x54, 0x54, 0x54, 0x18], // e
    [0x08, 0x7e, 0x09, 0x01, 0x02], // f
    [0x0c, 0x52, 0x52, 0x52, 0x3e], // g
    [0x7f, 0x08, 0x04, 0x04, 0x78], // h
    [0x00, 0x44, 0x7d, 0x40, 0x00], // i
    [0x20, 0x40, 0x44, 0x3d, 0x00], // j
    [0x7f, 0x10, 0x28, 0x44, 0x00], // k
    [0x00, 0x41, 0x7f, 0x40, 0x00], // l
    [0x7c, 0x04, 0x18, 0x04, 0x78], // m
    [0x7c, 0x08, 0x04, 0x04, 0x78], // n
    [0x38, 0x44, 0x44, 0x44, 0x38], // o
    [0x7c, 0x14, 0x14, 0x14, 0x08], // p
    [0x08, 0x14, 0x14, 0x18, 0x7c], // q
    [0x7c, 0x08, 0x04, 0x04, 0x08], // r
    [0x48, 0x54, 0x54, 0x54, 0x20], // s
    [0x04, 0x3f, 0x44, 0x40, 0x20], // t
    [0x3c, 0x40, 0x40, 0x20, 0x7c], // u
    [0x1c, 0x20, 0x40, 0x20, 0x1c], // v
    [0x3c, 0x40, 0x30, 0x40, 0x3c], // w
    [0x44, 0x28, 0x10, 0x28, 0x44], // x
    [0x0c, 0x50, 0x50, 0x50, 0x3c], // y
    [0x44, 0x64, 0x54, 0x4c, 0x44], // z
    [0x00, 0x08, 0x36, 0x41, 0x00], // {
    [0x00, 0x00, 0x7f, 0x00, 0x00], // |
    [0x00, 0x41, 0x36, 0x08, 0x00], // }
    [0x08, 0x04, 0x08, 0x10, 0x08], // ~
    [0x7f, 0x41, 0x41, 0x41, 0x7f], // inconnu : un rectangle vide
];
