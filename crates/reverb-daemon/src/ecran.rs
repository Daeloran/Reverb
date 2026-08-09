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
use std::sync::OnceLock;
use std::time::Duration;

use image::AnimationDecoder;
use image::ImageFormat;
use image::codecs::gif::GifDecoder;
use reverb_proto::composition::{self, Ancre, Boite, Composition, Fond, Secteur};
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
    ///
    /// ⚠️ **Remplie par doublements successifs, et non pixel par pixel.** Un
    /// `cycle().take(IMAGE_LEN).collect()` — ce qu'il y avait ici — coûte
    /// 1 228 800 pas d'itérateur ; sans optimisation ils ne s'inlinent pas, et
    /// une dalle unie prenait alors une quinzaine de millisecondes. Ce n'est pas
    /// un détail de confort : `Dalle::noire` passe par ici, une composition à
    /// fond noir la reconstruit à chaque recomposition, et #83 vient justement de
    /// mesurer que le temps passé sur ce chemin gèle le boîtier.
    ///
    /// Vingt recopies de tailles croissantes remplacent le million de pas, et le
    /// compilateur les rend en autant de `memcpy`.
    pub fn unie(couleur: (u8, u8, u8)) -> Dalle {
        let triplet = screen::pixel(couleur.0, couleur.1, couleur.2);
        let mut octets = Vec::with_capacity(screen::IMAGE_LEN);
        octets.extend_from_slice(&triplet);
        while octets.len() < screen::IMAGE_LEN {
            // Jamais plus que ce qu'on a déjà écrit, ni que ce qu'il reste à
            // écrire : la longueur finale est donc exactement `IMAGE_LEN`.
            let a_recopier = (screen::IMAGE_LEN - octets.len()).min(octets.len());
            octets.extend_from_within(..a_recopier);
        }
        Dalle { octets }
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

        // La valeur, en gros, au centre — dans la fonte embarquée depuis #90.
        // Les chiffres à sept segments qu'il y avait ici tenaient tant qu'on
        // n'écrivait que des nombres ; ils ne savaient rien écrire d'autre.
        let texte = chiffres(valeur);
        toile.ligne(
            CADRAN_BANDE,
            &texte,
            CADRAN_CHIFFRE_HAUTEUR as f32,
            COULEUR_CHIFFRE,
        );

        // L'unité juste sous les chiffres, le libellé au-dessus.
        toile.ligne(
            Boite {
                y: CADRAN_UNITE_Y as u16,
                hauteur: CADRAN_UNITE_HAUTEUR,
                ..CADRAN_BANDE
            },
            unite,
            f32::from(CADRAN_UNITE_HAUTEUR),
            COULEUR_UNITE,
        );
        toile.ligne(
            Boite {
                y: CADRAN_LIBELLE_Y as u16,
                hauteur: CADRAN_LIBELLE_HAUTEUR,
                ..CADRAN_BANDE
            },
            libelle,
            f32::from(CADRAN_LIBELLE_HAUTEUR),
            COULEUR_LIBELLE,
        );

        toile.dalle()
    }

    /// Le fond recopié, un texte écrit dessus dans la fonte embarquée (#90).
    ///
    /// **Le texte seul** : ni plaque assombrie, ni arc. C'est la primitive que
    /// `composee` assemble, et celle sur laquelle un test vérifie qu'une vraie
    /// fonte est à l'œuvre.
    pub fn texte(fond: &Dalle, texte: &str, boite: Boite) -> Dalle {
        let mut toile = Toile {
            octets: fond.octets.clone(),
        };
        let taille = taille_qui_tient(
            texte,
            f32::from(boite.largeur) * 0.94,
            f32::from(boite.hauteur) * 0.72,
        );
        let centre = f32::from(boite.y) + f32::from(boite.hauteur) / 2.0;
        toile.ecrire(boite, texte, centre, taille, COULEUR_CHIFFRE);
        toile.dalle()
    }

    /// Le fond recopié, un arc de couronne dessiné dessus (#90).
    pub fn arc(fond: &Dalle, secteur: Secteur, proportion: f32) -> Dalle {
        let mut toile = Toile {
            octets: fond.octets.clone(),
        };
        toile.arc(secteur, proportion);
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
            // ⚠️ **L'arc d'abord, le champ ensuite.** Le champ assombrit sa
            // boîte ; l'arc, lui, vit sur la couronne, hors d'elle. L'ordre ne
            // change donc rien au rendu — mais le poser d'abord garde vrai que
            // la boîte est la dernière chose écrite à cet endroit.
            if let (Some(secteur), ChampRendu::Temperature { valeur, .. }) =
                (ancre.secteur(), rendu)
            {
                toile.arc(secteur, valeur.map_or(0.0, proportion_de_temperature));
            }
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
/// Rang de la ligne de l'unité, **sous** les chiffres.
const CADRAN_UNITE_Y: usize = 432;

/// La bande horizontale où le cadran écrit, centrée sur la dalle.
///
/// Assez étroite pour que les lignes tiennent dans le disque visible quelle que
/// soit leur longueur : c'est [`Toile::ligne`] qui rétrécit le texte pour y
/// entrer, jamais un cadrage espéré.
const CADRAN_BANDE: Boite = Boite {
    x: (screen::WIDTH - CADRAN_LARGEUR_UTILE as u16) / 2,
    y: (screen::HEIGHT - CADRAN_CHIFFRE_HAUTEUR as u16) / 2,
    largeur: CADRAN_LARGEUR_UTILE as u16,
    hauteur: CADRAN_CHIFFRE_HAUTEUR as u16,
};

/// La hauteur d'œil de l'unité, sous les chiffres.
const CADRAN_UNITE_HAUTEUR: u16 = 52;

/// Celle du libellé, au-dessus.
const CADRAN_LIBELLE_HAUTEUR: u16 = 34;
/// Rang de la ligne du libellé, **au-dessus** des chiffres et dans l'anneau.
///
/// À ce rang, le disque intérieur de l'anneau est large de quelque 430 pixels :
/// un nom de sonde de trente caractères à l'échelle 2 en occupe 360, et reste
/// donc dedans.
const CADRAN_LIBELLE_Y: usize = 168;

/// Rayons de l'anneau de proportion, en pixels depuis le centre.
const ANNEAU_INTERIEUR: f32 = 262.0;
const ANNEAU_EXTERIEUR: f32 = 300.0;

/// Ce qu'il reste d'un fond sous un champ, en pour cent.
///
/// Trente : un blanc pur y tombe à 76, où du texte blanc se détache nettement,
/// et un fond déjà sombre ne s'écroule pas à zéro — la photo reste devinable
/// sous le champ.
const CHAMP_ASSOMBRISSEMENT: u16 = 30;

/// La part de la hauteur d'un champ qu'occupe l'œil d'un texte seul.
const PART_TEXTE_SEUL: f32 = 0.58;
/// Celle d'une mesure sans libellé.
const PART_MESURE_SEULE: f32 = 0.62;
/// Celle du libellé quand il y en a un ; la mesure prend le reste, et elle est
/// plus grosse.
const PART_LIBELLE: f32 = 0.36;

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
/// La fonte embarquée dans le binaire (issue #90).
///
/// `LiberationSans-Bold`, une grotesque large et grasse — métriquement
/// compatible Arial, licence OFL-1.1-RFN, redistribuable telle quelle. Elle est
/// **dans le binaire**, jamais lue sur le système : l'ADR-001 pose qu'un binaire
/// unique ne doit pas casser à une montée d'image, et une fonte absente de l'OS
/// est exactement ce genre de casse.
///
/// # Ce qu'elle remplace
///
/// Des chiffres à **sept segments** et une matrice **5 × 7** dessinés à la main.
/// C'était le bon choix pour afficher « 34.2 » sans traîner de bibliothèque
/// système (#33) ; ça ne l'était plus dès qu'il a fallu écrire des libellés, et
/// ça se voyait à six centimètres.
pub const FONTE: &[u8] = include_bytes!("../assets/LiberationSans-Bold.ttf");

/// La fonte analysée, une fois pour la vie du processus.
///
/// L'analyse coûte quelques centaines de microsecondes ; la refaire à chaque
/// composition la remettrait dans le chemin des deux secondes de recomposition.
fn fonte() -> &'static fontdue::Font {
    static LUE: OnceLock<fontdue::Font> = OnceLock::new();
    LUE.get_or_init(|| {
        // Une fonte compilée dans le binaire ne peut pas être absente ni
        // tronquée : ce qui est vérifié ici l'est une fois, au premier texte.
        fontdue::Font::from_bytes(FONTE, fontdue::FontSettings::default())
            .expect("la fonte embarquée par include_bytes! est valide")
    })
}

/// La largeur qu'un texte occupe à cette taille.
fn largeur_du_texte(texte: &str, taille: f32) -> f32 {
    let fonte = fonte();
    texte
        .chars()
        .map(|caractere| fonte.metrics(caractere, taille).advance_width)
        .sum()
}

/// La plus grande taille qui tienne à la fois en largeur et en hauteur.
///
/// ⚠️ **Calculée, jamais cherchée par essais.** Les avances d'une fonte sont
/// proportionnelles à la taille : une mesure à une taille de référence donne la
/// bonne d'un seul coup. Une boucle qui rétrécirait de proche en proche
/// tournerait sur chaque champ, quatre fois par composition, toutes les deux
/// secondes.
fn taille_qui_tient(texte: &str, largeur_dispo: f32, hauteur_dispo: f32) -> f32 {
    const REFERENCE: f32 = 100.0;
    let large = largeur_du_texte(texte, REFERENCE);
    let par_largeur = if large > 0.0 {
        REFERENCE * largeur_dispo / large
    } else {
        hauteur_dispo
    };
    hauteur_dispo.min(par_largeur).max(1.0)
}

/// La proportion d'anneau qu'une température représente (issue #90).
///
/// ⚠️ **L'échelle est celle du cadran** : zéro degré vide l'anneau, cent le
/// remplit. Ce n'est pas une mesure, c'est une lecture au coup d'œil — la valeur
/// exacte est écrite à côté.
///
/// Tout ce qui sort des bornes y est **ramené** plutôt que de déborder : une
/// sonde qui rend `NaN` — température divisée par une borne nulle — donnerait
/// sinon un index calculé hors du tampon.
pub fn proportion_de_temperature(valeur: f32) -> f32 {
    if valeur.is_nan() {
        return 0.0;
    }
    (valeur / 100.0).clamp(0.0, 1.0)
}

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

    /// Lit un pixel déjà peint.
    fn lire(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let debut = (y * usize::from(screen::WIDTH) + x) * screen::PIXEL_LEN;
        self.octets
            .get(debut..debut + screen::PIXEL_LEN)
            .map(screen::composantes)
            .unwrap_or_default()
    }

    /// Pose une couleur avec une couverture partielle, sur ce qui est déjà là.
    ///
    /// ⚠️ **C'est ce qui rend l'anticrénelage possible**, et donc ce qui
    /// distingue une vraie fonte d'une matrice dessinée à la main : un
    /// rastériseur rend une couverture de 0 à 255, pas un pixel allumé ou
    /// éteint.
    fn melanger(&mut self, x: i32, y: i32, clip: Boite, couleur: (u8, u8, u8), couverture: u8) {
        if couverture == 0 || x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x < usize::from(clip.x)
            || x >= usize::from(clip.x) + usize::from(clip.largeur)
            || y < usize::from(clip.y)
            || y >= usize::from(clip.y) + usize::from(clip.hauteur)
            || x >= usize::from(screen::WIDTH)
            || y >= usize::from(screen::HEIGHT)
        {
            return;
        }
        let fond = self.lire(x, y);
        let melee = |dessous: u8, dessus: u8| -> u8 {
            let a = u16::from(couverture);
            let melee = u16::from(dessous) * (255 - a) + u16::from(dessus) * a;
            (melee / 255) as u8
        };
        self.poser(
            x,
            y,
            (
                melee(fond.0, couleur.0),
                melee(fond.1, couleur.1),
                melee(fond.2, couleur.2),
            ),
        );
    }

    /// Écrit un texte dans la fonte embarquée, centré horizontalement dans
    /// `clip`, sa hauteur d'œil centrée sur `centre_y`.
    ///
    /// Rien n'est écrit hors de `clip` : le bornage est dans [`Toile::melanger`],
    /// et non dans un calcul de mise en page qu'il faudrait refaire juste.
    fn ecrire(
        &mut self,
        clip: Boite,
        texte: &str,
        centre_y: f32,
        taille: f32,
        couleur: (u8, u8, u8),
    ) {
        let fonte = fonte();
        let largeur = largeur_du_texte(texte, taille);
        let mut plume = f32::from(clip.x) + (f32::from(clip.largeur) - largeur) / 2.0;

        // La ligne de base se déduit des métriques de la fonte : centrer sur la
        // boîte englobante des glyphes ferait sautiller le texte selon qu'il
        // porte ou non un jambage.
        let lignes = fonte.horizontal_line_metrics(taille);
        let base = match lignes {
            Some(m) => centre_y + (m.ascent + m.descent) / 2.0,
            None => centre_y + taille / 2.0,
        };

        for caractere in texte.chars() {
            let (metriques, couvertures) = fonte.rasterize(caractere, taille);
            if metriques.width > 0 && metriques.height > 0 {
                let x0 = plume + metriques.xmin as f32;
                let y0 = base - (metriques.height as f32 + metriques.ymin as f32);
                for (rang, &couverture) in couvertures.iter().enumerate() {
                    let x = x0 + (rang % metriques.width) as f32;
                    let y = y0 + (rang / metriques.width) as f32;
                    self.melanger(
                        x.round() as i32,
                        y.round() as i32,
                        clip,
                        couleur,
                        couverture,
                    );
                }
            }
            plume += metriques.advance_width;
        }
    }

    /// Écrit une ligne centrée dans sa boîte, réduite si elle est trop large.
    ///
    /// ⚠️ **Réduite, jamais tronquée ni débordante.** Un libellé de sonde fait
    /// vingt-huit caractères et la dalle six centimètres : sans cette réduction,
    /// il sortirait du disque, où le contrôleur le coupe sans rien dire.
    fn ligne(&mut self, boite: Boite, texte: &str, hauteur: f32, couleur: (u8, u8, u8)) {
        let taille = taille_qui_tient(texte, f32::from(boite.largeur), hauteur);
        let centre = f32::from(boite.y) + f32::from(boite.hauteur) / 2.0;
        self.ecrire(boite, texte, centre, taille, couleur);
    }

    /// Dessine l'arc d'un secteur, rempli de sa proportion (issue #90).
    ///
    /// ⚠️ **Rien n'est dessiné pour la part vide.** Une piste de fond ferait
    /// qu'un arc à zéro pour cent peindrait autant de pixels qu'un arc plein, et
    /// « rempli proportionnellement » ne se distinguerait plus de « toujours
    /// dessiné ». La valeur, elle, est écrite à côté : un secteur vide ne cache
    /// aucune information.
    fn arc(&mut self, secteur: Secteur, proportion: f32) {
        let part = if proportion.is_nan() {
            0.0
        } else {
            proportion.clamp(0.0, 1.0)
        };
        let etendue = secteur.ouverture * part;
        if etendue <= 0.0 {
            return;
        }

        let centre = f32::from(screen::WIDTH) / 2.0;
        let interieur = f32::from(composition::COURONNE_RAYON_INTERIEUR);
        let exterieur = f32::from(composition::COURONNE_RAYON_EXTERIEUR);
        let (interieur2, exterieur2) = (interieur * interieur, exterieur * exterieur);

        for y in 0..usize::from(screen::HEIGHT) {
            for x in 0..usize::from(screen::WIDTH) {
                let dx = x as f32 + 0.5 - centre;
                let dy = y as f32 + 0.5 - centre;
                // Le carré du rayon d'abord : la racine et l'arc tangente ne
                // sont calculés que sur les quelques pour cent de pixels qui
                // tombent dans la couronne.
                let rayon2 = dx * dx + dy * dy;
                if rayon2 < interieur2 || rayon2 > exterieur2 {
                    continue;
                }
                // Zéro au sommet, croissant dans le sens horaire — la convention
                // qu'on lit sur une dalle ronde posée à plat.
                let angle = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
                if (angle - secteur.debut).rem_euclid(360.0) < etendue {
                    self.poser(x, y, COULEUR_ANNEAU);
                }
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

    /// Un champ de la composition, dessiné dans sa boîte (#80).
    fn champ(&mut self, boite: Boite, rendu: &ChampRendu) {
        self.assombrir(boite);

        let hauteur = f32::from(boite.hauteur);
        match rendu {
            ChampRendu::Texte(texte) => {
                self.ligne(boite, texte, hauteur * PART_TEXTE_SEUL, COULEUR_CHIFFRE);
            }
            ChampRendu::Temperature { libelle, valeur: _ } => {
                // Les chiffres et leur unité forment **une seule chaîne** depuis
                // #90. Les composer côte à côte, comme le faisaient les sept
                // segments, demandait de mesurer chaque morceau puis de centrer
                // le groupe à la main — un calcul que la fonte fait mieux, et
                // qui se décalait dès qu'une valeur changeait de longueur.
                let mesure = format!("{}{UNITE}", valeur_du_champ(rendu));

                let Some(libelle) = libelle else {
                    // Sans libellé, la mesure occupe seule la boîte.
                    self.ligne(boite, &mesure, hauteur * PART_MESURE_SEULE, COULEUR_CHIFFRE);
                    return;
                };

                // Avec, les deux se partagent la hauteur : le libellé dessus, la
                // mesure dessous et plus grosse — c'est elle qu'on lit d'un
                // mètre, le libellé ne sert qu'à savoir de quoi il s'agit.
                let haut = (hauteur * PART_LIBELLE).round() as u16;
                self.ligne(
                    Boite {
                        hauteur: haut,
                        ..boite
                    },
                    libelle,
                    hauteur * PART_LIBELLE * 0.82,
                    COULEUR_CHAMP_LIBELLE,
                );
                self.ligne(
                    Boite {
                        y: boite.y + haut,
                        hauteur: boite.hauteur - haut,
                        ..boite
                    },
                    &mesure,
                    hauteur * (1.0 - PART_LIBELLE) * 0.86,
                    COULEUR_CHIFFRE,
                );
            }
        }
    }
}
