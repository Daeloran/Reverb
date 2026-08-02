//! Tests d'intention de l'écran du Kraken côté démon — issue #33.
//!
//! Écrits **avant** l'implémentation, depuis l'issue #33 et le contrat public de
//! `crates/reverb-daemon/src/ecran.rs` — signatures et documentation seules, tous les corps y
//! étant `todo!("issue #33")` à l'écriture de ce fichier. Si l'un de ces tests échoue après
//! implémentation, c'est le code qu'on corrige.
//!
//! ## Les quatre pièges que ce fichier garde
//!
//! 1. **Une dalle de la mauvaise taille est ignorée en silence.** Le contrôleur attend
//!    [`screen::IMAGE_LEN`] octets, ni plus ni moins, et un transfert court le laisse sur son
//!    affichage firmware sans le moindre code d'erreur (spec §2.2.1). Toute dalle produite ici
//!    passe donc par `screen::check_image`, le validateur du protocole lui-même : ce fichier ne
//!    réinvente pas la taille attendue, il la fait vérifier par le code qui parle au bus.
//! 2. **L'ordre des composantes est muet.** Le projet en mêle trois — les LED NZXT en GRB, la RAM
//!    Corsair en RGB, cet écran en [`screen::COMPONENT_ORDER`], c'est-à-dire BGR. Une image
//!    affichée en RGB reste une image : elle s'affiche, elle est nette, elle est simplement de la
//!    mauvaise couleur, et rien ne le signale. Les couleurs témoins de ce fichier ont donc trois
//!    composantes distinctes, et les comparaisons passent par `screen::pixel`, seule fonction du
//!    projet qui connaisse l'ordre.
//! 3. **Une image étirée reste une image.** Une photo 16/9 écrasée en carré s'affiche sans erreur.
//!    Le contrat le dit — « une photo étirée en carré est laide et personne ne l'a demandée » — et
//!    c'est invérifiable à l'œil sur une dalle de 6 cm. La géométrie est donc mesurée : rang de la
//!    première et de la dernière ligne non noire, comparé à ce que le rapport de la source impose.
//! 4. **Un GIF trop rapide se dégrade de deux façons.** Sauter des images donne un mouvement
//!    saccadé mais à l'heure ; les ralentir donne un mouvement lent et complet. Le contrat tranche
//!    pour le second — « un mouvement lent et complet se regarde, un mouvement saccadé non » — et
//!    la faute est asymétrique : un décodeur qui saute passe tous les tests qui ne comptent pas ses
//!    sorties. D'où un compte exact partout où `cadence` est appelée.
//!
//! ## Trois points que le contrat laisse ouverts, et que ces tests tranchent
//!
//! 1. **Un signalement de `charger` nomme le fichier.** Le contrat exige un message sur fichier
//!    abîmé sans en fixer le contenu. Convention reprise de `spec_eclairage.rs` (#21) : le démon
//!    n'en écrit qu'un au démarrage, et « état d'écran invalide » sans le chemin laisse
//!    l'utilisateur devant une dalle vide sans savoir ce qu'il a perdu.
//! 2. **Le refus d'un chemin relatif ne se confond pas avec un fichier introuvable.** Le contrat
//!    exige un chemin absolu ; ce fichier exige en plus que le message le **dise**, parce qu'un
//!    « fichier introuvable » enverrait l'utilisateur vérifier un fichier qu'il a sous les yeux.
//! 3. **Un délai déjà au-dessus du plancher est laissé intact**, et non remis à l'échelle avec les
//!    autres. `cadence` est un plancher, pas un facteur : un GIF dont une image tient trois
//!    secondes garde ses trois secondes.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! Rien de ce qui touche au bus : `Dalle` s'arrête aux octets, et le transfert vit dans
//! `reverb-hw`. Rien du rafraîchissement à [`screen::REFRESH_INTERVAL_SECS`] non plus — c'est une
//! boucle de service, pas une fonction, et elle se vérifie sur la machine. Ce fichier ne fixe pas
//! le dessin du cadran : il exige qu'il ait la bonne taille, qu'il change avec la valeur, et qu'il
//! dise quelque chose quand la sonde a disparu. Le reste est du goût, et un test d'intention ne
//! fige pas un goût.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reverb_daemon::ecran::{
    Affichage, CHEMIN_ECRAN, Dalle, Etat, EtatInvalide, ImageInvalide, cadence, charger,
    enregistrer,
};
use reverb_daemon::persistance::CHEMIN_ECLAIRAGE;
use reverb_proto::screen;

// ---------------------------------------------------------------------------
// Dimensions et couleurs témoins
// ---------------------------------------------------------------------------

/// La largeur de la dalle, en pixels. Reprise du protocole, jamais réécrite : `screen::WIDTH` est
/// « annoncée par le contrôleur lui-même » (spec §3.7), et un test qui la recopierait continuerait
/// de passer le jour où elle changerait.
const LARGEUR: usize = screen::WIDTH as usize;

/// La hauteur de la dalle, même raison.
const HAUTEUR: usize = screen::HEIGHT as usize;

/// La couleur témoin. Ses trois composantes sont distinctes, donc aucune permutation ne peut
/// passer inaperçue : `ff2080` rendrait `20ff80` en GRB et `8020ff` en BGR. C'est la couleur de
/// `spec_ipc.rs` et de `spec_ipc_zones.rs`, gardée pour que la même faute se lise partout pareil.
const TEMOIN: (u8, u8, u8) = (0xff, 0x20, 0x80);

/// Le noir, celui des bandes ajoutées autour d'une image qui ne remplit pas la dalle.
const NOIR: (u8, u8, u8) = (0x00, 0x00, 0x00);

/// Une source deux fois plus large que haute.
///
/// Ses dimensions divisent celles de la dalle : la mise à l'échelle tombe alors sur des entiers, et
/// un écart d'une ligne n'est pas un arrondi mais une faute.
const SOURCE_LARGEUR: u32 = screen::WIDTH as u32 / 2;

/// La hauteur de cette même source — moitié de sa largeur, d'où un rapport de deux.
const SOURCE_HAUTEUR: u32 = SOURCE_LARGEUR / 2;

/// Les trois images du GIF témoin, dans l'ordre.
///
/// Trois primaires pures, pour deux raisons : un GIF passe par une palette de 256 couleurs, où une
/// teinte composite ressortirait approchée ; et aucune permutation de composantes ne peut confondre
/// du rouge pur avec du bleu pur, ce qui rend l'ordre des images lisible sur un seul pixel.
const IMAGES_DU_GIF: [(u8, u8, u8); 3] =
    [(0xff, 0x00, 0x00), (0x00, 0xff, 0x00), (0x00, 0x00, 0xff)];

/// Écart toléré sur une couleur relue d'un GIF.
///
/// Un GIF est indexé : une image unie traverse sa palette à quelques unités près. Jamais à cent —
/// c'est ce qui laisse le seuil attraper une permutation de composantes, qui déplace 255.
const TOLERANCE_PALETTE: u8 = 16;

/// Le plancher témoin des tests de cadence.
///
/// La documentation de `cadence` donne l'ordre de grandeur — « une image de 1,2 Mo met environ cent
/// millisecondes à passer » — mais la valeur exacte appartient au démon et au bus. La signature la
/// prend en paramètre, ce fichier vérifie donc la **règle**, pas le chiffre.
const PLANCHER: Duration = Duration::from_millis(100);

// ---------------------------------------------------------------------------
// Lecture d'une dalle
// ---------------------------------------------------------------------------

/// Les octets bruts du pixel (`x`, `y`), dans l'ordre où ils partiront sur le bus.
fn triplet(octets: &[u8], x: usize, y: usize) -> [u8; screen::PIXEL_LEN] {
    let debut = (y * LARGEUR + x) * screen::PIXEL_LEN;
    octets[debut..debut + screen::PIXEL_LEN]
        .try_into()
        .expect("un pixel fait screen::PIXEL_LEN octets")
}

/// La couleur du pixel (`x`, `y`), remise dans l'ordre humain pour les messages d'échec.
///
/// Inverse exact de `screen::pixel`, et le seul endroit de ce fichier qui lise
/// [`screen::COMPONENT_ORDER`] : si la mire renverse un jour la conclusion BGR, c'est la constante
/// qui change, et ce fichier suit sans être touché.
fn couleur(octets: &[u8], x: usize, y: usize) -> (u8, u8, u8) {
    let brut = triplet(octets, x, y);
    (
        brut[screen::COMPONENT_ORDER[0]],
        brut[screen::COMPONENT_ORDER[1]],
        brut[screen::COMPONENT_ORDER[2]],
    )
}

/// Vrai si toute la ligne `y` est noire.
fn ligne_noire(octets: &[u8], y: usize) -> bool {
    (0..LARGEUR).all(|x| couleur(octets, x, y) == NOIR)
}

/// Vrai si toute la colonne `x` est noire.
fn colonne_noire(octets: &[u8], x: usize) -> bool {
    (0..HAUTEUR).all(|y| couleur(octets, x, y) == NOIR)
}

/// Vérifie qu'une dalle a exactement la taille qu'attend le contrôleur.
///
/// Passe par `screen::check_image`, le validateur du protocole : une dalle trop courte est
/// **ignorée en silence** par le matériel, et ce fichier n'a pas à réécrire la règle que
/// `reverb-proto` porte déjà.
fn dalle_bien_dimensionnee(dalle: &Dalle, quoi: &str) {
    let octets = dalle.octets();
    assert_eq!(
        octets.len(),
        screen::IMAGE_LEN,
        "{quoi} doit faire screen::IMAGE_LEN octets — une dalle courte est ignorée par le \
         contrôleur sans le moindre code d'erreur"
    );
    assert!(
        screen::check_image(octets).is_ok(),
        "{quoi} doit passer le validateur du protocole"
    );
}

/// L'unique dalle d'une image fixe, ou un échec qui dit ce qu'on a reçu à la place.
fn dalle_unique(chemin: &Path) -> Dalle {
    let dalles = Dalle::depuis_fichier(chemin)
        .unwrap_or_else(|erreur| panic!("« {} » doit être lisible : {erreur}", chemin.display()));
    assert_eq!(
        dalles.len(),
        1,
        "« {} » est une image fixe : elle rend une seule dalle",
        chemin.display()
    );
    dalles.into_iter().next().expect("une dalle exactement")
}

/// Le refus d'un fichier, avec ce que ce fichier exige de tout refus : une raison non vide, et un
/// `Display` qui la porte.
fn refus_d_image(chemin: &Path) -> ImageInvalide {
    match Dalle::depuis_fichier(chemin) {
        Ok(dalles) => panic!(
            "« {} » devait être refusé, il a rendu {} dalle(s) — une dalle inventée sur un \
             fichier illisible afficherait n'importe quoi sans que rien ne le dise",
            chemin.display(),
            dalles.len()
        ),
        Err(erreur) => {
            assert!(
                !erreur.raison.trim().is_empty(),
                "« {} » doit être refusé **en le disant**",
                chemin.display()
            );
            assert!(
                erreur.to_string().contains(erreur.raison.as_str()),
                "le Display porte la raison : « {erreur} »"
            );
            let _: &dyn std::error::Error = &erreur;
            erreur
        }
    }
}

/// Le refus d'un texte d'état, avec les mêmes exigences minimales.
fn refus_d_etat(texte: &str) -> EtatInvalide {
    match Etat::decoder(texte) {
        Ok(etat) => panic!("« {texte:?} » devait être refusé, il a rendu {etat:?}"),
        Err(erreur) => {
            assert!(
                !erreur.raison.trim().is_empty(),
                "« {texte:?} » doit être refusé en disant pourquoi"
            );
            let _: &dyn std::error::Error = &erreur;
            erreur
        }
    }
}

// ---------------------------------------------------------------------------
// États témoins
// ---------------------------------------------------------------------------

/// Les quatre affichages, chacun avec une luminosité qui lui est propre.
///
/// Des luminosités distinctes parce qu'un encodeur qui écrirait toujours la même — ou qui
/// intervertirait les deux lignes du fichier — passerait un aller-retour où elles se ressemblent.
fn etats_temoins() -> Vec<Etat> {
    vec![
        Etat {
            luminosite: 100,
            affichage: Affichage::Rien,
        },
        Etat {
            luminosite: 0,
            affichage: Affichage::Rien,
        },
        Etat {
            luminosite: 1,
            affichage: Affichage::Cadran("kraken2023elite:coolant".to_owned()),
        },
        Etat {
            luminosite: 42,
            affichage: Affichage::Cadran("k10temp:tctl".to_owned()),
        },
        Etat {
            luminosite: 99,
            affichage: Affichage::Image("/home/nico/images/fond.png".to_owned()),
        },
        Etat {
            luminosite: 50,
            affichage: Affichage::Gif("/home/nico/anims/pluie.gif".to_owned()),
        },
        // Un chemin portant une espace, parce que le protocole en accepte un : `spec_ipc_ecran.rs`
        // tranche que `screen image <chemin>` prend sa ligne jusqu'au bout, un sélecteur de
        // fichiers rendant `Mes documents/fond d'écran.png` tous les jours. Ce que le socket
        // accepte, le fichier doit savoir le conserver — sans quoi la commande réussit, la dalle
        // s'allume, et le choix disparaît au redémarrage suivant.
        Etat {
            luminosite: 7,
            affichage: Affichage::Image("/home/nico/Mes documents/fond d'écran.png".to_owned()),
        },
    ]
}

// ---------------------------------------------------------------------------
// Dossier temporaire et fabrication de fichiers d'image
// ---------------------------------------------------------------------------

/// Un dossier de travail sous `std::env::temp_dir()`, effacé à la fin du test.
///
/// L'effacement passe par `Drop` et non par une ligne en fin de test : un test qui échoue part en
/// `panic!`, et une fin de test jamais atteinte laisserait le dossier derrière elle à chaque
/// régression. Convention reprise de `spec_eclairage.rs`.
struct DossierJetable {
    chemin: PathBuf,
}

impl DossierJetable {
    /// `nom` doit être le nom du test : c'est ce qui rend le chemin unique entre tests, `cargo` les
    /// exécutant en parallèle dans un même processus.
    fn neuf(nom: &str) -> DossierJetable {
        let chemin =
            std::env::temp_dir().join(format!("reverb-spec-ecran-{}-{nom}", std::process::id()));
        let _ = fs::remove_dir_all(&chemin);
        fs::create_dir_all(&chemin)
            .unwrap_or_else(|erreur| panic!("dossier de test {} : {erreur}", chemin.display()));
        DossierJetable { chemin }
    }

    fn fichier(&self, nom: &str) -> PathBuf {
        self.chemin.join(nom)
    }

    fn chemin(&self) -> &Path {
        &self.chemin
    }
}

impl Drop for DossierJetable {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.chemin);
    }
}

/// Les octets d'une image unie, en RGB à trois octets par pixel — le format d'entrée du crate
/// `image`, sans rapport avec l'ordre de la dalle.
fn aplat(largeur: u32, hauteur: u32, (r, v, b): (u8, u8, u8)) -> Vec<u8> {
    let pixels = largeur as usize * hauteur as usize;
    let mut octets = Vec::with_capacity(pixels * 3);
    for _ in 0..pixels {
        octets.extend_from_slice(&[r, v, b]);
    }
    octets
}

/// Écrit un PNG uni. Le crate `image` est une dépendance de `reverb-daemon` : ces tests s'en
/// servent pour **fabriquer** leurs entrées, jamais pour vérifier leurs sorties.
fn ecrire_png(chemin: &Path, largeur: u32, hauteur: u32, couleur: (u8, u8, u8)) {
    image::save_buffer_with_format(
        chemin,
        &aplat(largeur, hauteur, couleur),
        largeur,
        hauteur,
        image::ExtendedColorType::Rgb8,
        image::ImageFormat::Png,
    )
    .unwrap_or_else(|erreur| panic!("écriture de {} : {erreur}", chemin.display()));
}

/// Écrit un JPEG uni — l'autre format que l'issue exige d'accepter.
fn ecrire_jpeg(chemin: &Path, largeur: u32, hauteur: u32, couleur: (u8, u8, u8)) {
    image::save_buffer_with_format(
        chemin,
        &aplat(largeur, hauteur, couleur),
        largeur,
        hauteur,
        image::ExtendedColorType::Rgb8,
        image::ImageFormat::Jpeg,
    )
    .unwrap_or_else(|erreur| panic!("écriture de {} : {erreur}", chemin.display()));
}

/// Écrit un GIF d'une image unie par couleur donnée, dans l'ordre donné.
fn ecrire_gif(chemin: &Path, cote: u32, couleurs: &[(u8, u8, u8)], delai_ms: u32) {
    let fichier = fs::File::create(chemin)
        .unwrap_or_else(|erreur| panic!("création de {} : {erreur}", chemin.display()));
    let images: Vec<image::Frame> = couleurs
        .iter()
        .map(|&(r, v, b)| {
            let tampon = image::RgbaImage::from_pixel(cote, cote, image::Rgba([r, v, b, 0xff]));
            image::Frame::from_parts(tampon, 0, 0, image::Delay::from_numer_denom_ms(delai_ms, 1))
        })
        .collect();

    // L'encodeur termine le fichier à sa destruction : il doit mourir avant qu'on relise.
    let mut encodeur = image::codecs::gif::GifEncoder::new(fichier);
    encodeur
        .encode_frames(images)
        .unwrap_or_else(|erreur| panic!("écriture de {} : {erreur}", chemin.display()));
    drop(encodeur);
}

// ---------------------------------------------------------------------------
// 1 — l'aller-retour du fichier d'état
// ---------------------------------------------------------------------------

#[test]
fn l_aller_retour_encoder_decoder_est_exact_pour_les_quatre_affichages() {
    // Test d'intention n° 8 de l'issue : « L'aller-retour écriture/lecture d'`ecran.conf` est exact
    // pour les quatre états », et critère d'acceptation : « Le choix survit à l'arrêt du démon : au
    // redémarrage, la dalle retrouve ce qu'elle affichait. »
    //
    // Le mode de défaillance n'est pas une erreur, c'est un état plausible et faux : un cadran qui
    // revient en image fixe, ou une luminosité qui remonte à 100 sur une dalle qu'on avait éteinte.
    for temoin in etats_temoins() {
        let texte = temoin.encoder();
        assert_eq!(
            Etat::decoder(&texte),
            Ok(temoin.clone()),
            "aller-retour de {temoin:?} par {texte:?} — rien ne se perd en route"
        );
    }

    // Toutes les luminosités, pas seulement les témoins : une seule d'entre elles écrite en
    // hexadécimal, arrondie ou écrêtée déplacerait le curseur au redémarrage suivant.
    for luminosite in 0..=100u8 {
        let pose = Etat {
            luminosite,
            affichage: Affichage::Cadran("k10temp:tctl".to_owned()),
        };
        assert_eq!(
            Etat::decoder(&pose.encoder()),
            Ok(pose.clone()),
            "aller-retour à {luminosite} %"
        );
    }

    // Les quatre affichages sont quatre états distincts. Un décodeur qui rendrait `Image` pour un
    // `gif` afficherait la première image d'une animation, fixe, sans que rien ne le dise.
    let quatre = [
        Affichage::Rien,
        Affichage::Cadran("k10temp:tctl".to_owned()),
        Affichage::Image("/a".to_owned()),
        Affichage::Gif("/a".to_owned()),
    ];
    for (rang, un) in quatre.iter().enumerate() {
        for autre in &quatre[rang + 1..] {
            let gauche = Etat {
                luminosite: 50,
                affichage: un.clone(),
            };
            let droite = Etat {
                luminosite: 50,
                affichage: autre.clone(),
            };
            assert_ne!(
                gauche.encoder(),
                droite.encoder(),
                "{un:?} et {autre:?} ne s'écrivent pas pareil"
            );
        }
    }

    // Le même chemin passé à `Image` et à `Gif` ne se confond pas : c'est le verbe qui dit comment
    // le fichier est joué, pas son extension.
    let fixe = Etat {
        luminosite: 50,
        affichage: Affichage::Image("/a.gif".to_owned()),
    };
    let anime = Etat {
        luminosite: 50,
        affichage: Affichage::Gif("/a.gif".to_owned()),
    };
    assert_eq!(Etat::decoder(&fixe.encoder()), Ok(fixe));
    assert_eq!(Etat::decoder(&anime.encoder()), Ok(anime));
}

#[test]
fn le_fichier_d_ecran_a_les_deux_lignes_documentees() {
    // Contrat — `Etat::encoder` : « Le texte du fichier : une ligne `brightness`, une ligne
    // `affiche`. » Ce test n'est pas décoratif : c'est lui qui donne aux tests de refus le droit de
    // désigner « la ligne de la luminosité » et « la ligne de l'affichage » sans écrire un format
    // que le contrat ne fixe pas.
    for temoin in etats_temoins() {
        let texte = temoin.encoder();
        let lignes: Vec<&str> = texte.lines().collect();
        assert_eq!(
            lignes.len(),
            2,
            "le fichier d'écran a deux lignes et pas une de plus : {texte:?}"
        );
        assert_eq!(
            rang_de(&texte, "brightness"),
            1,
            "la première ligne est celle de la luminosité : {texte:?}"
        );
        assert_eq!(
            rang_de(&texte, "affiche"),
            2,
            "la seconde est celle de l'affichage : {texte:?}"
        );
    }
}

/// Le rang, à partir de 1, de la ligne dont le premier mot est `mot`.
fn rang_de(texte: &str, mot: &str) -> usize {
    texte
        .lines()
        .position(|ligne| ligne.split_whitespace().next() == Some(mot))
        .map_or_else(
            || panic!("aucune ligne ne commence par « {mot} » dans {texte:?}"),
            |rang| rang + 1,
        )
}

/// Le texte témoin, avec la ligne de rang `rang` (à partir de 1) remplacée.
fn avec_ligne(texte: &str, rang: usize, remplacement: &str) -> String {
    let mut lignes: Vec<String> = texte.lines().map(str::to_owned).collect();
    lignes[rang - 1] = remplacement.to_owned();
    lignes.join("\n")
}

// ---------------------------------------------------------------------------
// 2 — un texte abîmé est refusé en nommant la ligne
// ---------------------------------------------------------------------------

#[test]
fn un_texte_abime_est_refuse_en_nommant_la_ligne() {
    // Contrat — `Etat::decoder` : « L'inverse, strict, en nommant la ligne fautive », et
    // `EtatInvalide { ligne, raison }`, dont le `Display` écrit « ligne N : raison » dès que `ligne`
    // n'est pas zéro.
    //
    // Nommer la ligne n'est pas du confort : le fichier est écrit par le démon mais **édité à la
    // main** le jour où quelqu'un veut poser un cadran sans passer par la fenêtre. Un « fichier
    // d'écran invalide » sans plus laisse relire deux lignes dont on ne sait pas laquelle cloche —
    // et le démon, lui, repart sur l'accueil.
    let temoin = Etat {
        luminosite: 42,
        affichage: Affichage::Cadran("k10temp:tctl".to_owned()),
    };
    let texte = temoin.encoder();
    let rang_luminosite = rang_de(&texte, "brightness");
    let rang_affichage = rang_de(&texte, "affiche");

    let mut raisons = Vec::new();

    // a — premier mot inconnu, sur chacune des deux lignes.
    for (rang, faux) in [
        (rang_luminosite, "luminosite 42"),
        (rang_luminosite, "bidule 42"),
        (rang_affichage, "affichage gauge"),
        (rang_affichage, "montre gauge"),
    ] {
        let abime = avec_ligne(&texte, rang, faux);
        let erreur = refus_d_etat(&abime);
        assert_eq!(
            erreur.ligne, rang,
            "« {faux} » est à la ligne {rang} : c'est celle que le message doit nommer. Obtenu : \
             {erreur}"
        );
        assert!(
            erreur.to_string().contains(&rang.to_string()),
            "le Display porte le rang de la ligne : « {erreur} »"
        );
        raisons.push(erreur.raison);
    }

    // b — luminosité hors bornes ou pas un nombre. `screen::set_brightness` refuse au-delà de 100 ;
    // un fichier qui passerait 255 ferait échouer le réglage sur le bus, longtemps après la
    // lecture, sans que rien ne rattache l'échec au fichier.
    for valeur in ["101", "200", "255", "256", "-1", "abc", "1.5", ""] {
        let abime = avec_ligne(&texte, rang_luminosite, &format!("brightness {valeur}"));
        let erreur = refus_d_etat(&abime);
        assert_eq!(
            erreur.ligne, rang_luminosite,
            "une luminosité de « {valeur} » se corrige à la ligne {rang_luminosite}. Obtenu : \
             {erreur}"
        );
        raisons.push(erreur.raison);
    }

    // Et les bornes passent : une borne posée à `< 100` perdrait la pleine luminosité, une borne à
    // `> 0` perdrait l'extinction.
    for valeur in [0u8, 1, 100] {
        let bon = avec_ligne(&texte, rang_luminosite, &format!("brightness {valeur}"));
        let etat = Etat::decoder(&bon)
            .unwrap_or_else(|erreur| panic!("{valeur} % est une luminosité légitime : {erreur}"));
        assert_eq!(etat.luminosite, valeur, "relue depuis {bon:?}");
    }

    // c — affichage inconnu, et affichage sans son argument.
    for faux in [
        "affiche bidule",
        "affiche",
        "affiche  ",
        "affiche rien rien",
    ] {
        let abime = avec_ligne(&texte, rang_affichage, faux);
        let erreur = refus_d_etat(&abime);
        assert_eq!(
            erreur.ligne, rang_affichage,
            "« {faux} » est à la ligne {rang_affichage}. Obtenu : {erreur}"
        );
        raisons.push(erreur.raison);
    }

    // d — ligne manquante. La ligne nommée est celle qui **manque** : c'est celle qu'il faut
    // ajouter, et nommer la dernière ligne présente enverrait corriger une ligne correcte.
    let tronque = texte
        .lines()
        .take(rang_affichage - 1)
        .collect::<Vec<&str>>()
        .join("\n");
    let erreur = refus_d_etat(&tronque);
    assert_eq!(
        erreur.ligne, rang_affichage,
        "la ligne {rang_affichage} manque : c'est elle que le message doit nommer. Obtenu : \
         {erreur}"
    );
    raisons.push(erreur.raison);

    // Un fichier vide n'a même pas sa première ligne.
    for vide in ["", "\n", "   ", "\n\n"] {
        let erreur = refus_d_etat(vide);
        assert_eq!(
            erreur.ligne, 1,
            "un fichier vide manque de sa première ligne. Obtenu sur {vide:?} : {erreur}"
        );
    }

    // e — une ligne de trop. « L'inverse, **strict** » : une ligne surnuméraire est le signe d'un
    // fichier d'une version qu'on ne sait pas lire, et l'avaler ferait repartir le démon sur un
    // état amputé de ce qu'il n'a pas compris.
    let bavard = format!("{}\nbidule 3", texte.trim_end());
    let erreur = refus_d_etat(&bavard);
    assert_eq!(
        erreur.ligne, 3,
        "la ligne 3 est celle qui est de trop. Obtenu : {erreur}"
    );

    // Les refus distinguent les fautes : un mot-clé inconnu, une valeur hors bornes et une ligne
    // manquante ne se corrigent pas de la même façon, et une phrase unique laisserait tout le
    // diagnostic à faire alors que le fichier est sous les yeux.
    raisons.sort_unstable();
    raisons.dedup();
    assert!(
        raisons.len() >= 3,
        "quatre familles de fautes ne peuvent pas partager une seule explication : {raisons:?}"
    );
}

// ---------------------------------------------------------------------------
// 3 — charger : absent, valide, abîmé
// ---------------------------------------------------------------------------

#[test]
fn charger_un_fichier_absent_rend_l_accueil_sans_message() {
    // Contrat — `charger` : « Lit le fichier d'écran, en disant ce qui a cloché plutôt qu'en
    // échouant », et `Etat::accueil` : « Ce que la dalle montre au premier démarrage : rien, à
    // pleine luminosité. »
    //
    // Critère d'acceptation : « Le démon sans Kraken branché démarre, et le dit une fois, sans
    // boucler. » Un message ici polluerait le journal de toute installation neuve, et rendrait
    // indistinguable le premier démarrage d'un fichier abîmé.
    let dossier = DossierJetable::neuf("fichier_absent");
    let chemin = dossier.fichier("ecran.conf");

    let (etat, signalement) = charger(&chemin);
    assert_eq!(
        etat,
        Etat::accueil(),
        "un fichier absent, c'est le premier démarrage : la dalle part sur l'accueil"
    );
    assert_eq!(
        signalement, None,
        "le premier démarrage n'est pas une anomalie : rien à signaler. Obtenu : {signalement:?}"
    );

    // Lire, c'est lire. Un démarrage qui écrirait l'accueil en passant effacerait la distinction
    // entre « jamais réglé » et « réglé », et ferait de la première lecture un état posé.
    assert!(
        !chemin.exists(),
        "charger un écran absent ne doit rien écrire : {} a été créé",
        chemin.display()
    );
}

#[test]
fn charger_un_fichier_abime_rend_l_accueil_et_un_message() {
    // Même contrat, l'autre branche : « en disant ce qui a cloché plutôt qu'en échouant ». Le démon
    // doit démarrer — une dalle sans affichage n'empêche pas la machine de tourner — mais la
    // différence entre « jamais réglé » et « réglé puis abîmé » ne doit pas se perdre.
    let dossier = DossierJetable::neuf("fichier_abime");

    // Trois formes d'illisible, pour la même conclusion.
    let texte_faux = dossier.fichier("texte.conf");
    fs::write(&texte_faux, "bidule\n").expect("écriture du fichier abîmé");

    // Des octets qui ne sont pas de l'UTF-8 : une écriture coupée par une panne de courant peut
    // laisser exactement ça.
    let binaire = dossier.fichier("binaire.conf");
    fs::write(&binaire, [0xff_u8, 0xfe, 0x00, 0x80]).expect("écriture d'octets invalides");

    // Un dossier là où le démon attend un fichier : la lecture échoue au niveau système, avant même
    // qu'il y ait du texte à analyser.
    let en_dossier = dossier.fichier("dossier.conf");
    fs::create_dir(&en_dossier).expect("création du faux fichier");

    for chemin in [&texte_faux, &binaire, &en_dossier] {
        let (etat, signalement) = charger(chemin);
        assert_eq!(
            etat,
            Etat::accueil(),
            "un fichier abîmé ne doit pas empêcher le démon de démarrer : l'accueil s'applique. \
             Chemin : {}",
            chemin.display()
        );
        let message = signalement.unwrap_or_else(|| {
            panic!(
                "un fichier abîmé doit être signalé, sans quoi il se confond avec un premier \
                 démarrage. Chemin : {}",
                chemin.display()
            )
        });
        assert!(
            !message.trim().is_empty(),
            "le signalement doit dire quelque chose. Chemin : {}",
            chemin.display()
        );
        let nom = chemin
            .file_name()
            .and_then(|nom| nom.to_str())
            .expect("les fichiers de test ont un nom lisible");
        assert!(
            message.contains(nom),
            "le signalement nomme le fichier — le démon n'en écrit qu'un au démarrage, et « état \
             d'écran invalide » sans le chemin laisse l'utilisateur devant une dalle vide. \
             Obtenu : {message}"
        );
    }
}

#[test]
fn charger_un_fichier_valide_rend_ce_qu_il_dit_sans_message() {
    // Critère d'acceptation : « Le choix survit à l'arrêt du démon : au redémarrage, la dalle
    // retrouve ce qu'elle affichait. » Et l'accueil ne doit pas se substituer à un état valide qui
    // lui ressemble : une dalle éteinte volontairement (`brightness 0`) ne se rallume pas à 100 %.
    let dossier = DossierJetable::neuf("fichier_valide");

    for (rang, temoin) in etats_temoins().into_iter().enumerate() {
        let chemin = dossier.fichier(&format!("ecran-{rang}.conf"));
        fs::write(&chemin, temoin.encoder()).expect("écriture de l'état témoin");

        let (etat, signalement) = charger(&chemin);
        assert_eq!(etat, temoin, "l'état relu est celui qu'on avait écrit");
        assert_eq!(
            signalement, None,
            "un fichier valide n'a rien à signaler. Obtenu : {signalement:?}"
        );
    }

    // Le cas qui ressemble le plus à l'accueil, et qui n'en est pas un : une dalle éteinte que le
    // démarrage rallumerait.
    let eteinte = Etat {
        luminosite: 0,
        affichage: Affichage::Rien,
    };
    let chemin = dossier.fichier("eteinte.conf");
    fs::write(&chemin, eteinte.encoder()).expect("écriture de l'état éteint");
    let (relue, _) = charger(&chemin);
    assert_eq!(relue, eteinte);
    assert_ne!(
        relue,
        Etat::accueil(),
        "une dalle éteinte volontairement ne se confond pas avec un premier démarrage : la \
         rallumer à chaque redémarrage rendrait le curseur inutilisable"
    );
}

// ---------------------------------------------------------------------------
// 4 — enregistrer puis charger
// ---------------------------------------------------------------------------

#[test]
fn enregistrer_puis_charger_rend_le_meme_etat_sans_fichier_provisoire() {
    // Contrat — `enregistrer` : « Écrit le fichier d'écran, par fichier provisoire puis
    // renommage. » C'est ce qui protège l'état d'une coupure en cours d'écriture : ou bien l'ancien
    // fichier entier, ou bien le nouveau, jamais la moitié des deux.
    //
    // Le provisoire ne doit pas survivre : à raison d'un par commande, ce serait un fichier de plus
    // dans `/var/lib/reverb` à chaque mouvement du curseur.
    let dossier = DossierJetable::neuf("enregistrer_charger");
    let chemin = dossier.fichier("ecran.conf");

    for temoin in etats_temoins() {
        enregistrer(&chemin, &temoin).expect("l'enregistrement doit réussir");
        let (relu, signalement) = charger(&chemin);
        assert_eq!(
            relu, temoin,
            "l'aller-retour par le disque est exact : c'est le seul mécanisme du « ce que la dalle \
             affichait » de l'issue"
        );
        assert_eq!(
            signalement, None,
            "ce que le démon vient d'écrire, il doit savoir le relire. Obtenu : {signalement:?}"
        );
    }

    let entrees: Vec<PathBuf> = fs::read_dir(dossier.chemin())
        .expect("lecture du dossier de test")
        .map(|entree| entree.expect("entrée lisible").path())
        .collect();
    assert_eq!(
        entrees,
        vec![chemin.clone()],
        "après tous ces enregistrements, le dossier ne contient que le fichier d'écran — aucun \
         provisoire ne doit survivre"
    );

    // Le dernier état écrit est celui qu'on relit : un enregistrement qui ajouterait au lieu de
    // remplacer ferait grandir le fichier et rendrait le second état illisible.
    let premier = Etat {
        luminosite: 10,
        affichage: Affichage::Gif("/a.gif".to_owned()),
    };
    let second = Etat {
        luminosite: 90,
        affichage: Affichage::Rien,
    };
    enregistrer(&chemin, &premier).expect("premier enregistrement");
    enregistrer(&chemin, &second).expect("second enregistrement");
    let (relu, _) = charger(&chemin);
    assert_eq!(relu, second, "le second enregistrement remplace le premier");
}

#[test]
fn l_etat_de_l_ecran_est_un_etat_de_service() {
    // Approche technique de l'issue : « Persistance : ce que la dalle affiche est un état de
    // service, donc `/var/lib/reverb/ecran.conf`, à côté d'`eclairage.conf`. »
    //
    // `/etc` serait de la configuration — éditée par un humain, conservée par le paquet. Ce fichier
    // est écrit par le service à chaque mouvement du curseur : le ranger sous `/etc` ferait qu'une
    // mise à jour du paquet demanderait quoi faire d'un fichier « modifié par l'administrateur ».
    assert_eq!(CHEMIN_ECRAN, "/var/lib/reverb/ecran.conf");
    let chemin = Path::new(CHEMIN_ECRAN);
    assert!(
        chemin.is_absolute(),
        "le démon ne lit pas depuis son répertoire courant"
    );
    assert_eq!(
        chemin.parent(),
        Path::new(CHEMIN_ECLAIRAGE).parent(),
        "l'état de l'écran vit à côté de celui de l'éclairage : un seul dossier à créer, un seul à \
         sauvegarder"
    );
}

#[test]
fn l_accueil_est_une_dalle_rendue_au_firmware_a_pleine_luminosite() {
    // Contrat — `Etat::accueil` : « rien, à pleine luminosité ». C'est ce que voit une installation
    // neuve, et le seul état que le démon pose sans qu'on le lui demande.
    assert_eq!(
        Etat::accueil(),
        Etat {
            luminosite: 100,
            affichage: Affichage::Rien
        }
    );
    assert!(
        screen::set_brightness(Etat::accueil().luminosite).is_ok(),
        "l'accueil doit être une luminosité que le matériel accepte"
    );
}

// ---------------------------------------------------------------------------
// 5 — la dalle noire
// ---------------------------------------------------------------------------

#[test]
fn la_dalle_noire_fait_exactement_la_taille_d_une_image() {
    // Piège n° 1 du préambule. Une dalle de la mauvaise taille n'est pas refusée par le
    // contrôleur : elle est **ignorée**, l'envoi réussit, aucun code d'erreur ne remonte, et
    // l'écran reste sur son affichage firmware (spec §2.2.1).
    let noire = Dalle::noire();
    dalle_bien_dimensionnee(&noire, "la dalle noire");
    assert!(
        noire.octets().iter().all(|&octet| octet == 0),
        "la dalle noire est noire : un octet non nul y ferait un point lumineux permanent"
    );

    // Et elle est bien noire au sens des pixels, quel que soit l'ordre des composantes.
    for (x, y) in [
        (0, 0),
        (LARGEUR - 1, 0),
        (0, HAUTEUR - 1),
        (LARGEUR / 2, HAUTEUR / 2),
    ] {
        assert_eq!(couleur(noire.octets(), x, y), NOIR, "pixel ({x}, {y})");
    }
}

// ---------------------------------------------------------------------------
// 6 et 7 — le cadran
// ---------------------------------------------------------------------------

#[test]
fn un_cadran_fait_toujours_la_taille_d_une_image_quelle_que_soit_la_valeur() {
    // Test d'intention n° 6 de l'issue : « Le cadran rend un tampon aux dimensions exactes de la
    // dalle, quelle que soit la valeur. »
    //
    // Le cadran est dessiné à la main, chiffre par chiffre : c'est le seul contenu du projet dont
    // la taille dépende de ce qu'il montre. Une valeur à cinq chiffres, un `NaN`, un infini —
    // chacun est une occasion d'écrire hors du tampon ou de s'arrêter avant sa fin, et une dalle
    // courte est ignorée en silence par le contrôleur.
    let valeurs = [
        None,
        Some(0.0),
        Some(-0.0),
        Some(34.2),
        Some(100.0),
        Some(-40.0),
        Some(-273.15),
        Some(99_999.0),
        Some(1.0e30),
        Some(-1.0e30),
        Some(f32::MAX),
        Some(f32::MIN),
        Some(f32::MIN_POSITIVE),
        Some(f32::EPSILON),
        Some(f32::NAN),
        Some(f32::INFINITY),
        Some(f32::NEG_INFINITY),
    ];

    for valeur in valeurs {
        for unite in ["°C", "", "tr/min", "%"] {
            for libelle in ["coolant", "", "kraken2023elite:coolant"] {
                let dalle = Dalle::cadran(libelle, valeur, unite, 0.5);
                dalle_bien_dimensionnee(
                    &dalle,
                    &format!("le cadran de ({libelle:?}, {valeur:?}, {unite:?})"),
                );
            }
        }
    }

    // Le cadran **change** avec la valeur : un cadran identique pour 34 °C et 80 °C serait une
    // image fixe qui ressemble à un cadran, et « le cadran se met à jour tant qu'il est affiché »
    // deviendrait invérifiable à l'œil.
    let froid = Dalle::cadran("coolant", Some(34.0), "°C", 0.34);
    let chaud = Dalle::cadran("coolant", Some(80.0), "°C", 0.80);
    assert_ne!(
        froid.octets(),
        chaud.octets(),
        "34 °C et 80 °C ne donnent pas la même dalle"
    );

    // La proportion aussi : c'est elle qui porte l'anneau, et deux valeurs égales à proportions
    // différentes n'ont pas le même anneau.
    let vide = Dalle::cadran("coolant", Some(34.0), "°C", 0.0);
    let plein = Dalle::cadran("coolant", Some(34.0), "°C", 1.0);
    assert_ne!(
        vide.octets(),
        plein.octets(),
        "l'anneau de proportion doit se voir : sans lui, le paramètre ne sert à rien"
    );

    // Et un cadran dit quelque chose : une dalle noire ne se distingue pas d'un écran éteint, et
    // « lisible à un mètre » commence par « visible ».
    assert_ne!(
        froid.octets(),
        Dalle::noire().octets(),
        "un cadran n'est pas une dalle noire"
    );
}

#[test]
fn un_cadran_sur_une_sonde_disparue_le_dit_au_lieu_de_figer() {
    // Test d'intention n° 7 de l'issue : « Un cadran sur une sonde disparue le dit sur la dalle au
    // lieu de figer la dernière valeur. » Contrat — `Dalle::cadran` : « `valeur` absente quand la
    // sonde ne répond plus : la dalle le **dit** au lieu de figer la dernière valeur lue. »
    //
    // C'est le mode de défaillance le plus coûteux du cadran, parce qu'il est **rassurant** : un
    // écran qui affiche 34 °C alors que la sonde ne répond plus dit exactement ce qu'on veut lire.
    // Une pompe arrêtée derrière un 34 °C figé, c'est un circuit qui chauffe sans que rien ne le
    // signale.
    let absente = Dalle::cadran("coolant", None, "°C", 0.0);
    dalle_bien_dimensionnee(&absente, "le cadran d'une sonde disparue");

    // Il dit quelque chose : une dalle noire serait indistinguable d'un écran éteint.
    assert_ne!(
        absente.octets(),
        Dalle::noire().octets(),
        "un cadran sans valeur doit **dire** que la sonde a disparu — une dalle noire ne dit rien, \
         elle ressemble à un écran éteint"
    );

    // Et il diffère de tout cadran portant une valeur, y compris de celui de zéro — qui est la
    // valeur qu'un `unwrap_or_default()` mettrait à sa place, et qui se lit « 0 °C », c'est-à-dire
    // une information fausse plutôt qu'une absence.
    for valeur in [0.0f32, -0.0, 34.2, 100.0] {
        let presente = Dalle::cadran("coolant", Some(valeur), "°C", 0.0);
        assert_ne!(
            absente.octets(),
            presente.octets(),
            "le cadran sans valeur ne doit pas ressembler à celui de {valeur} — « 0 °C » affiché \
             pour une sonde muette est un mensonge, pas une valeur par défaut"
        );
    }
}

#[test]
fn une_proportion_hors_bornes_ne_deborde_ni_ne_panique() {
    // Contrat — `Dalle::cadran` : « `proportion` sert l'anneau qui entoure le chiffre, de 0 à 1 ;
    // hors de ces bornes, il est ramené dedans plutôt que de déborder. »
    //
    // La proportion vient d'un calcul — température sur température maximale — et rien n'empêche ce
    // calcul de rendre 1,2 un jour de canicule, ou un `NaN` le jour où la borne maximale vaut zéro.
    // Un anneau qui déborde écrit hors du tampon ; un `NaN` qui traverse un `as usize` donne zéro
    // en Rust, mais un index calculé à partir de lui peut sortir. Ni panique, ni dalle courte : le
    // démon tient l'écran, et une panique ici emporte aussi l'éclairage.
    for proportion in [
        -1.0f32,
        -0.001,
        1.001,
        2.0,
        1.0e30,
        -1.0e30,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        let dalle = Dalle::cadran("coolant", Some(34.2), "°C", proportion);
        dalle_bien_dimensionnee(&dalle, &format!("le cadran de proportion {proportion}"));
    }

    // « Ramené dedans » a un sens précis, et c'est ce qui distingue un écrêtage d'un repli
    // arbitraire : au-delà de 1, l'anneau est celui de 1 ; en deçà de 0, celui de 0. Un anneau qui
    // repartirait à zéro au-delà de 1 montrerait un circuit froid au moment où il chauffe le plus.
    let plein = Dalle::cadran("coolant", Some(34.2), "°C", 1.0);
    let vide = Dalle::cadran("coolant", Some(34.2), "°C", 0.0);
    assert_ne!(
        plein.octets(),
        vide.octets(),
        "l'anneau plein et l'anneau vide ne se ressemblent pas, sinon la suite ne prouve rien"
    );
    for au_dela in [1.001f32, 2.0, 1.0e30, f32::INFINITY] {
        assert_eq!(
            Dalle::cadran("coolant", Some(34.2), "°C", au_dela).octets(),
            plein.octets(),
            "une proportion de {au_dela} est ramenée à 1, pas repliée sur autre chose"
        );
    }
    for en_deca in [-0.001f32, -1.0, -1.0e30, f32::NEG_INFINITY] {
        assert_eq!(
            Dalle::cadran("coolant", Some(34.2), "°C", en_deca).octets(),
            vide.octets(),
            "une proportion de {en_deca} est ramenée à 0"
        );
    }
}

// ---------------------------------------------------------------------------
// 8 — le chemin d'une image
// ---------------------------------------------------------------------------

#[test]
fn un_chemin_relatif_est_refuse_en_le_disant() {
    // Test d'intention n° 3 de l'issue, et contrat de `Dalle::depuis_fichier` : « ⚠️ Le chemin doit
    // être **absolu**. Le démon ne partage pas le répertoire courant de son client, et un chemin
    // relatif y désignerait autre chose — ou rien. »
    //
    // Point n° 2 du préambule : le refus doit le **dire**. Un « fichier introuvable » enverrait
    // l'utilisateur vérifier un fichier qu'il a sous les yeux, dans le répertoire d'où il a lancé
    // la commande.
    let dossier = DossierJetable::neuf("chemin_relatif");
    let absolu = dossier.fichier("fond.png");
    ecrire_png(&absolu, SOURCE_LARGEUR, SOURCE_LARGEUR, TEMOIN);

    // Le témoin de contrôle : le même fichier, en absolu, passe.
    dalle_bien_dimensionnee(&dalle_unique(&absolu), "l'image témoin");

    let mut raisons = Vec::new();
    for relatif in [
        "fond.png",
        "./fond.png",
        "../fond.png",
        "images/fond.png",
        ".",
    ] {
        let erreur = refus_d_image(Path::new(relatif));
        let bas = erreur.raison.to_lowercase();
        assert!(
            bas.contains("absolu") || bas.contains("relatif"),
            "« {relatif} » doit être refusé **en le disant** : sans le mot, le message se confond \
             avec « fichier introuvable » et envoie chercher la faute ailleurs. Raison obtenue : \
             {}",
            erreur.raison
        );
        raisons.push(erreur.raison);
    }

    // Et ce refus n'est pas celui d'un fichier manquant : deux fautes différentes, deux phrases
    // différentes. Sans cette inégalité, une implémentation qui accepterait les chemins relatifs
    // passerait le test précédent par accident, en tombant sur « introuvable ».
    let manquant = dossier.fichier("jamais-ecrit.png");
    let sur_manquant = refus_d_image(&manquant);
    for raison in &raisons {
        assert_ne!(
            raison, &sur_manquant.raison,
            "« chemin relatif » et « fichier introuvable » sont deux fautes distinctes : les \
             confondre revient à ne pas contrôler le chemin du tout"
        );
    }

    // Le cas qui tranche vraiment : un chemin relatif qui **désigne un vrai fichier**. C'est le
    // seul qui distingue « je refuse les relatifs » de « les relatifs échouent par accident ».
    // Il ne se construit que si le dossier temporaire est atteignable depuis le répertoire courant
    // du test — vrai sur une installation ordinaire, et sauté sinon plutôt que de dépendre d'une
    // disposition de disque.
    if let Some(relatif) = chemin_relatif_vers(&absolu) {
        let erreur = refus_d_image(&relatif);
        assert_ne!(
            erreur.raison,
            sur_manquant.raison,
            "« {} » désigne un fichier lisible : il doit être refusé pour ce qu'il est — un chemin \
             relatif — et non pour un fichier manquant",
            relatif.display()
        );
    }
}

/// Un chemin relatif au répertoire courant qui désigne `cible`, s'il en existe un qui la retrouve.
///
/// Rend `None` plutôt qu'un chemin approximatif : ce test préfère être sauté qu'être faux.
fn chemin_relatif_vers(cible: &Path) -> Option<PathBuf> {
    let courant = std::env::current_dir().ok()?;
    let remontees = courant.components().count() - 1;
    let mut relatif = PathBuf::new();
    for _ in 0..remontees {
        relatif.push("..");
    }
    relatif.push(cible.strip_prefix("/").ok()?);

    let meme = fs::canonicalize(&relatif).ok()? == fs::canonicalize(cible).ok()?;
    if meme && relatif.is_relative() {
        Some(relatif)
    } else {
        None
    }
}

#[test]
fn un_fichier_absent_vide_ou_d_un_format_inconnu_est_refuse_en_le_disant() {
    // Critère d'acceptation : « Une image dont le fichier est illisible ou d'un format inconnu est
    // refusée **en le disant**, et ce que la dalle affiche ne change pas », et test d'intention
    // n° 2 : « Une image d'un format inconnu est refusée en nommant le format, sans toucher la
    // dalle. »
    //
    // « Sans toucher la dalle » est assuré par la signature : `depuis_fichier` rend un `Result` et
    // n'écrit nulle part. Ce que ce test garde, c'est qu'aucune dalle ne soit **inventée** —
    // rendre `Ok(vec![Dalle::noire()])` sur un fichier illisible éteindrait l'écran en silence, ce
    // qui ressemble à un écran qui s'est éteint tout seul.
    //
    // L'issue ajoute que le démon « lit sous son propre uid, ce qu'il faut dire quand elle
    // échoue » : d'où l'exigence que le message porte le fichier.
    let dossier = DossierJetable::neuf("format_inconnu");

    let absent = dossier.fichier("jamais-ecrit.png");

    let vide = dossier.fichier("vide.png");
    fs::write(&vide, []).expect("écriture du fichier vide");

    let texte = dossier.fichier("notes.txt");
    fs::write(&texte, "ceci n'est pas une image\n").expect("écriture du faux fichier");

    // Un fichier qui **ment sur son format** : l'extension dit PNG, le contenu non. Se fier à
    // l'extension et lancer le décodeur PNG dessus doit finir par un refus, jamais par une dalle.
    let menteur = dossier.fichier("menteur.png");
    fs::write(&menteur, "GIF89a mais pas vraiment").expect("écriture du menteur");

    // Un PNG tronqué en plein milieu — ce qu'une copie interrompue laisse derrière elle.
    let complet = dossier.fichier("complet.png");
    ecrire_png(&complet, SOURCE_LARGEUR, SOURCE_HAUTEUR, TEMOIN);
    let octets = fs::read(&complet).expect("relecture du PNG témoin");
    let tronque = dossier.fichier("tronque.png");
    fs::write(&tronque, &octets[..octets.len() / 2]).expect("écriture du PNG tronqué");

    // Un dossier là où l'on attend un fichier.
    let en_dossier = dossier.fichier("dossier.png");
    fs::create_dir(&en_dossier).expect("création du faux fichier");

    let mut raisons = Vec::new();
    for chemin in [&absent, &vide, &texte, &menteur, &tronque, &en_dossier] {
        let erreur = refus_d_image(chemin);
        let nom = chemin
            .file_name()
            .and_then(|nom| nom.to_str())
            .expect("les fichiers de test ont un nom lisible");
        assert!(
            erreur.raison.contains(nom),
            "le refus nomme le fichier — le démon lit sous son propre uid, et « image illisible » \
             sans le chemin ne dit pas lequel des deux comptes n'y a pas accès. Obtenu : {}",
            erreur.raison
        );
        raisons.push(erreur.raison);
    }

    // Un fichier absent et un fichier d'un format inconnu ne se corrigent pas de la même façon :
    // l'un se retrouve, l'autre se reconvertit. Une phrase unique laisserait l'utilisateur devant
    // un fichier qu'il voit dans son gestionnaire et que le démon dit « illisible ».
    let distinctes: Vec<&String> = {
        let mut vues: Vec<&String> = raisons.iter().collect();
        vues.sort();
        vues.dedup();
        vues
    };
    assert!(
        distinctes.len() >= 2,
        "un fichier absent et un fichier d'un format inconnu ne peuvent pas partager une seule \
         explication : {raisons:?}"
    );

    // Et le témoin de contrôle : le fichier complet, lui, passe. Sans lui, un `depuis_fichier` qui
    // refuserait tout passerait ce test entier.
    dalle_bien_dimensionnee(&dalle_unique(&complet), "l'image témoin");
}

// ---------------------------------------------------------------------------
// 9 — la mise à l'échelle
// ---------------------------------------------------------------------------

#[test]
fn une_image_plus_large_que_haute_est_mise_a_l_echelle_sans_deformation_et_centree() {
    // Test d'intention n° 1 de l'issue : « Une image plus large que haute est mise à l'échelle sans
    // déformation, et centrée », et critère d'acceptation : « Une image PNG ou JPEG de n'importe
    // quelle taille est acceptée et mise à l'échelle en 640×640 sans déformer ses proportions. »
    //
    // Piège n° 3 du préambule : une image étirée reste une image. Elle s'affiche, elle est nette,
    // elle est simplement fausse — et sur une dalle de 6 cm, personne ne le voit. La géométrie est
    // donc mesurée, pas regardée.
    let dossier = DossierJetable::neuf("image_large");
    let chemin = dossier.fichier("large.png");
    ecrire_png(&chemin, SOURCE_LARGEUR, SOURCE_HAUTEUR, TEMOIN);

    let dalle = dalle_unique(&chemin);
    dalle_bien_dimensionnee(&dalle, "une image plus large que haute");
    let octets = dalle.octets();

    // La hauteur qu'impose le rapport de la source, une fois sa largeur amenée à celle de la dalle.
    let contenu = HAUTEUR * SOURCE_HAUTEUR as usize / SOURCE_LARGEUR as usize;
    let bande = (HAUTEUR - contenu) / 2;
    assert!(
        contenu < HAUTEUR && bande > 0,
        "la source témoin doit laisser des bandes, sinon ce test ne prouve rien"
    );

    let premiere = (0..HAUTEUR).find(|&y| !ligne_noire(octets, y));
    let derniere = (0..HAUTEUR).rev().find(|&y| !ligne_noire(octets, y));
    assert_eq!(
        premiere,
        Some(bande),
        "le contenu commence à la ligne {bande} : plus haut, l'image est collée en haut au lieu \
         d'être centrée ; à zéro, elle a été étirée sur toute la dalle"
    );
    assert_eq!(
        derniere,
        Some(bande + contenu - 1),
        "le contenu finit à la ligne {} : une autre valeur, et le rapport de l'image n'a pas été \
         gardé",
        bande + contenu - 1
    );

    // Les bandes ajoutées sont noires — et non grises, ni prolongées depuis le bord de l'image.
    for y in (0..bande).chain(bande + contenu..HAUTEUR) {
        assert!(
            ligne_noire(octets, y),
            "la ligne {y} est une bande ajoutée : elle doit être noire. Trouvé {:?} en (0, {y})",
            couleur(octets, 0, y)
        );
    }

    // Le contenu occupe toute la largeur, et porte la couleur de la source dans le bon ordre de
    // composantes. Une image en RGB au lieu de BGR s'affiche nette et de la mauvaise couleur.
    let milieu = bande + contenu / 2;
    for (x, y) in [
        (0, milieu),
        (LARGEUR - 1, milieu),
        (LARGEUR / 2, bande + 1),
        (LARGEUR / 2, bande + contenu - 2),
        (LARGEUR / 2, milieu),
    ] {
        assert_eq!(
            triplet(octets, x, y),
            screen::pixel(TEMOIN.0, TEMOIN.1, TEMOIN.2),
            "le pixel ({x}, {y}) doit porter la couleur de la source dans l'ordre de l'écran. \
             Trouvé {:?}, attendu {TEMOIN:?}",
            couleur(octets, x, y)
        );
    }
}

#[test]
fn une_image_plus_haute_que_large_est_centree_horizontalement() {
    // Le symétrique du précédent. Une mise à l'échelle qui ne saurait traiter que le paysage
    // étirerait tous les portraits, et rien ne le dirait : le compte des bandes est nul dans les
    // deux cas quand on ne les cherche que dans un sens.
    let dossier = DossierJetable::neuf("image_haute");
    let chemin = dossier.fichier("haute.png");
    ecrire_png(&chemin, SOURCE_HAUTEUR, SOURCE_LARGEUR, TEMOIN);

    let dalle = dalle_unique(&chemin);
    dalle_bien_dimensionnee(&dalle, "une image plus haute que large");
    let octets = dalle.octets();

    let contenu = LARGEUR * SOURCE_HAUTEUR as usize / SOURCE_LARGEUR as usize;
    let bande = (LARGEUR - contenu) / 2;

    let premiere = (0..LARGEUR).find(|&x| !colonne_noire(octets, x));
    let derniere = (0..LARGEUR).rev().find(|&x| !colonne_noire(octets, x));
    assert_eq!(
        premiere,
        Some(bande),
        "le contenu commence à la colonne {bande} : un portrait est centré horizontalement"
    );
    assert_eq!(derniere, Some(bande + contenu - 1));

    for x in (0..bande).chain(bande + contenu..LARGEUR) {
        assert!(
            colonne_noire(octets, x),
            "la colonne {x} est une bande ajoutée : elle doit être noire"
        );
    }
}

#[test]
fn une_image_deja_carree_remplit_la_dalle_sans_aucune_bande() {
    // Le cas sans bande, qui garde le test précédent honnête : une implémentation qui ajouterait
    // des bandes à tort — parce qu'elle réduit avant de centrer, ou qu'elle arrondit vers le bas —
    // rétrécirait toutes les images de quelques pixels, et personne ne le verrait.
    let dossier = DossierJetable::neuf("image_carree");

    for cote in [
        screen::WIDTH as u32,
        screen::WIDTH as u32 / 10,
        screen::WIDTH as u32 * 2,
        1,
    ] {
        let chemin = dossier.fichier(&format!("carree-{cote}.png"));
        ecrire_png(&chemin, cote, cote, TEMOIN);

        let dalle = dalle_unique(&chemin);
        dalle_bien_dimensionnee(
            &dalle,
            &format!("une image carrée de {cote} pixels de côté"),
        );
        let octets = dalle.octets();

        for (x, y) in [
            (0, 0),
            (LARGEUR - 1, 0),
            (0, HAUTEUR - 1),
            (LARGEUR - 1, HAUTEUR - 1),
            (LARGEUR / 2, HAUTEUR / 2),
        ] {
            assert_eq!(
                triplet(octets, x, y),
                screen::pixel(TEMOIN.0, TEMOIN.1, TEMOIN.2),
                "une image carrée remplit la dalle jusqu'aux coins : pixel ({x}, {y}) d'un côté \
                 de {cote}. Trouvé {:?}",
                couleur(octets, x, y)
            );
        }
    }
}

#[test]
fn une_image_fixe_rend_une_seule_dalle_en_png_comme_en_jpeg() {
    // Critère d'acceptation : « Une image PNG ou JPEG de n'importe quelle taille est acceptée ». Le
    // JPEG est vérifié à part parce qu'il n'a ni le même décodeur ni le même espace de couleurs, et
    // qu'une implémentation qui n'active que `png` échouerait ici et nulle part ailleurs.
    //
    // Une seule dalle, et non plusieurs : une image fixe rendue en plusieurs dalles ferait tourner
    // la boucle d'animation pour rien, et rendrait le compte du GIF ininterprétable.
    let dossier = DossierJetable::neuf("image_fixe");

    let png = dossier.fichier("fond.png");
    ecrire_png(&png, SOURCE_LARGEUR, SOURCE_LARGEUR, TEMOIN);
    let dalle = dalle_unique(&png);
    dalle_bien_dimensionnee(&dalle, "un PNG");
    assert_eq!(
        triplet(dalle.octets(), LARGEUR / 2, HAUTEUR / 2),
        screen::pixel(TEMOIN.0, TEMOIN.1, TEMOIN.2),
        "un PNG traverse sans perte : trouvé {:?}",
        couleur(dalle.octets(), LARGEUR / 2, HAUTEUR / 2)
    );

    // Le JPEG est compressé avec perte : sa couleur est vérifiée à la tolérance, pas à l'octet.
    let jpeg = dossier.fichier("photo.jpg");
    ecrire_jpeg(&jpeg, SOURCE_LARGEUR, SOURCE_LARGEUR, TEMOIN);
    let dalle = dalle_unique(&jpeg);
    dalle_bien_dimensionnee(&dalle, "un JPEG");
    proche(
        couleur(dalle.octets(), LARGEUR / 2, HAUTEUR / 2),
        TEMOIN,
        "le centre d'un JPEG uni",
    );
}

/// Vérifie qu'une couleur relue est celle qu'on attend, à la tolérance des formats avec perte.
///
/// La tolérance laisse passer la compression et la palette, jamais une permutation de composantes :
/// celle-ci déplace 255, la tolérance en admet [`TOLERANCE_PALETTE`].
fn proche(lue: (u8, u8, u8), attendue: (u8, u8, u8), quoi: &str) {
    let ecart = |a: u8, b: u8| a.abs_diff(b);
    assert!(
        ecart(lue.0, attendue.0) <= TOLERANCE_PALETTE
            && ecart(lue.1, attendue.1) <= TOLERANCE_PALETTE
            && ecart(lue.2, attendue.2) <= TOLERANCE_PALETTE,
        "{quoi} doit porter {attendue:?} à {TOLERANCE_PALETTE} près — trouvé {lue:?}. Un écart de \
         cet ordre n'est pas de la compression, c'est un ordre de composantes"
    );
}

// ---------------------------------------------------------------------------
// 10 — le GIF
// ---------------------------------------------------------------------------

#[test]
fn un_gif_de_trois_images_rend_trois_dalles_dans_l_ordre() {
    // Test d'intention n° 4 de l'issue : « Un GIF de trois images en rend trois, dans l'ordre, en
    // boucle. » La boucle appartient à la boucle de service ; ce qui se vérifie ici est ce qu'elle
    // recevra : trois dalles, et les bonnes, dans le bon sens.
    //
    // Deux fautes plausibles, toutes deux muettes : ne décoder que la première image — le GIF
    // devient une image fixe, ce qui ressemble à un GIF qui n'a qu'une image — et rendre les images
    // à l'envers, ce qui reste une animation, fluide et fausse.
    let dossier = DossierJetable::neuf("gif_trois_images");
    let chemin = dossier.fichier("anim.gif");
    let cote = screen::WIDTH as u32 / 10;
    ecrire_gif(&chemin, cote, &IMAGES_DU_GIF, 100);

    let dalles = Dalle::depuis_fichier(&chemin).expect("le GIF témoin doit être lisible");
    assert_eq!(
        dalles.len(),
        IMAGES_DU_GIF.len(),
        "un GIF de {} images rend {} dalles — une seule, et l'animation devient une image fixe \
         sans que rien ne le dise",
        IMAGES_DU_GIF.len(),
        IMAGES_DU_GIF.len()
    );

    for (rang, (dalle, attendue)) in dalles.iter().zip(IMAGES_DU_GIF).enumerate() {
        dalle_bien_dimensionnee(dalle, &format!("l'image {rang} du GIF"));
        proche(
            couleur(dalle.octets(), LARGEUR / 2, HAUTEUR / 2),
            attendue,
            &format!("le centre de l'image {rang} du GIF"),
        );
    }

    // Les trois dalles sont bel et bien différentes : un décodeur qui rendrait trois fois la
    // première passerait le compte sans animer quoi que ce soit.
    for (rang, dalle) in dalles.iter().enumerate() {
        for (autre_rang, autre) in dalles.iter().enumerate().skip(rang + 1) {
            assert_ne!(
                dalle.octets(),
                autre.octets(),
                "les images {rang} et {autre_rang} du GIF témoin sont de couleurs différentes"
            );
        }
    }

    // Un GIF d'une seule image reste un GIF : il rend une dalle, pas zéro.
    let unique = dossier.fichier("fixe.gif");
    ecrire_gif(&unique, cote, &IMAGES_DU_GIF[..1], 100);
    let dalles = Dalle::depuis_fichier(&unique).expect("un GIF d'une image est lisible");
    assert_eq!(dalles.len(), 1, "un GIF d'une image rend une dalle");
}

// ---------------------------------------------------------------------------
// 11 — la cadence
// ---------------------------------------------------------------------------

#[test]
fn la_cadence_ne_descend_jamais_sous_le_plancher_et_ne_saute_aucune_image() {
    // Test d'intention n° 5 de l'issue : « Une cadence de GIF plus rapide que le bus est ramenée à
    // ce que le bus tient, sans sauter d'image », et critère d'acceptation : « Un GIF dont les
    // images sont plus rapides que le bus est ralenti, pas désynchronisé ni tronqué. »
    //
    // Piège n° 4 du préambule : la faute est asymétrique. Sauter des images donne un mouvement
    // saccadé mais à l'heure, et passe tous les tests qui ne comptent pas les sorties. Le contrat
    // tranche : « on le ralentit au lieu de sauter des images ».
    let rapides = vec![
        Duration::from_millis(10),
        Duration::from_millis(20),
        Duration::ZERO,
        Duration::from_millis(33),
    ];
    let ralentis = cadence(&rapides, PLANCHER);
    assert_eq!(
        ralentis.len(),
        rapides.len(),
        "autant de délais en sortie qu'en entrée : un délai en moins est une image en moins, et \
         une animation amputée"
    );
    for (rang, delai) in ralentis.iter().enumerate() {
        assert_eq!(
            *delai, PLANCHER,
            "le délai {rang} valait {:?}, sous le plancher : il est remonté au plancher",
            rapides[rang]
        );
    }

    // Ce qui est déjà au-dessus est laissé **intact**, et non remis à l'échelle avec le reste :
    // `cadence` est un plancher, pas un facteur. Un GIF dont une image tient trois secondes garde
    // ses trois secondes.
    let lents = vec![
        PLANCHER,
        PLANCHER + Duration::from_millis(1),
        Duration::from_secs(3),
    ];
    assert_eq!(
        cadence(&lents, PLANCHER),
        lents,
        "un délai au plancher ou au-dessus traverse tel quel — le plancher est une borne, pas une \
         cadence imposée"
    );

    // Mêlés, comme dans un vrai GIF : seules les images trop rapides bougent.
    let melange = vec![
        Duration::from_millis(10),
        Duration::from_secs(1),
        Duration::from_millis(50),
        PLANCHER,
        Duration::from_millis(99),
    ];
    assert_eq!(
        cadence(&melange, PLANCHER),
        vec![
            PLANCHER,
            Duration::from_secs(1),
            PLANCHER,
            PLANCHER,
            PLANCHER,
        ],
        "seuls les délais sous le plancher sont relevés"
    );

    // Le cas de l'issue : trente images par seconde, « trois fois le débit disponible ». Toutes
    // sont gardées, et l'animation dure plus longtemps qu'écrite — c'est exactement ce que
    // « ralenti, pas tronqué » veut dire.
    let trente_par_seconde = vec![Duration::from_millis(33); 30];
    let tenu = cadence(&trente_par_seconde, PLANCHER);
    assert_eq!(
        tenu.len(),
        trente_par_seconde.len(),
        "les trente images sont gardées"
    );
    assert!(
        tenu.iter().all(|delai| *delai >= PLANCHER),
        "aucun délai ne descend sous ce que le bus tient : {tenu:?}"
    );
    let ecrit: Duration = trente_par_seconde.iter().sum();
    let joue: Duration = tenu.iter().sum();
    assert!(
        joue > ecrit,
        "l'animation est **ralentie** : {joue:?} pour {ecrit:?} écrites. Une durée inchangée avec \
         trente images signifierait qu'on en a jeté"
    );

    // Les bords : rien à ralentir, et un plancher nul.
    assert_eq!(
        cadence(&[], PLANCHER),
        Vec::<Duration>::new(),
        "un GIF sans image ne rend aucun délai — et ne panique pas sur un `max` d'une liste vide"
    );
    assert_eq!(
        cadence(&melange, Duration::ZERO),
        melange,
        "un plancher nul ne change rien : c'est le cas d'un bus qui suivrait n'importe quelle \
         cadence"
    );
}
