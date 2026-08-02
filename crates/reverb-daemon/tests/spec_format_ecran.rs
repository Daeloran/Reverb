//! Tests d'intention du contrôle de format de l'écran — issue #69.
//!
//! Écrits **avant** l'implémentation, depuis l'issue #69 seule. Rien de `crates/*/src/` n'a été lu
//! pour les écrire, hors les signatures publiques déjà existantes de `reverb_daemon::ecran`
//! (`Affichage`, `Etat`, `ImageInvalide`, `charger`, `enregistrer`) qu'il faut nommer pour que le
//! fichier compile. Si l'un de ces tests échoue après implémentation, c'est le code qu'on corrige.
//!
//! ## Ce que l'issue raconte, et que ce fichier garde
//!
//! Relevé sur SHYNAEL le 2026-08-02, `/var/lib/reverb/ecran.conf` contenait
//! `gif:/home/lupink/Images/Wallpapers/pxfuel.jpg` — **un GIF déclaré sur un `.jpg`**. La commande
//! avait été acceptée, l'état écrit, et le démon a rejoué cet affichage impossible à chaque
//! redémarrage du service. Le défaut n'est pas qu'un humain se trompe de format : la fenêtre
//! propose « image » et « gif » dans le même menu. Le défaut est qu'on puisse **persister** un état
//! que le démon ne saura jamais afficher, et donc qu'il redémarre cassé sans moyen d'en sortir
//! seul.
//!
//! ## Les quatre pièges que ce fichier garde
//!
//! 1. **« ça échoue » ne suffit pas.** Une vérification qui refuserait tout ferait passer la moitié
//!    d'une suite naïve, et rendrait l'écran inutilisable. Chaque refus attendu a donc ici son
//!    pendant accepté : un vrai GIF donné à `gif` **doit** passer, et les trois formats donnés à
//!    `image` aussi.
//! 2. **« n'est pas un GIF » ne suffit pas non plus.** L'issue veut « décodé comme JPEG — essaie
//!    "image" » : le message doit nommer ce que le fichier **est**, pas seulement ce qu'il n'est
//!    pas. Sans ça l'utilisateur relit son chemin, le trouve juste, et n'a aucune piste. Les tests
//!    de refus exigent donc la présence du nom du format trouvé.
//! 3. **L'extension ment, et c'est tout le sujet.** Le fichier fautif de l'issue s'appelait `.jpg`
//!    et était bien un JPEG ; mais une vérification qui lirait l'extension au lieu du contenu
//!    passerait ce cas-là et raterait tous les autres — un GIF renommé `.jpg` serait refusé à tort,
//!    un JPEG renommé `.gif` accepté à tort. Deux témoins portent donc une extension **mensongère**,
//!    dans les deux sens.
//! 4. **Reconnaître un format ne doit pas coûter un décodage.** L'approche technique de l'issue le
//!    dit : `ImageReader::with_guessed_format` reconnaît sans décoder, « ce qui rend la vérification
//!    bon marché même sur un fichier de plusieurs mégaoctets ». Un fichier réduit à son en-tête reçoit
//!    donc le **même verdict** que le fichier entier.
//!
//! ## Trois points que l'issue laisse ouverts, et que ces tests tranchent
//!
//! 1. **La forme du refus.** L'issue donne un message d'exemple, pas un contrat. Ces tests
//!    n'exigent que deux choses de la `raison` : le **nom du fichier** et le **nom du format
//!    trouvé**, comparés sans distinction de casse pour laisser libre « JPEG », « Jpeg » ou
//!    « image/jpeg ». La suggestion de verbe — « essaie "image" » — n'est **pas** exigée : le mot
//!    « image » est trop courant dans un message d'erreur d'image pour qu'une assertion dessus
//!    prouve quoi que ce soit, et une assertion qui passe par accident vaut moins que pas
//!    d'assertion.
//! 2. **Le mot des octets non reconnus.** Quand le fichier n'est aucun des trois formats, il n'y a
//!    pas de format à nommer. Le message doit alors le **dire** plutôt que se taire : ces tests
//!    exigent le radical « reconnu » (« format non reconnu », « aucun format reconnu »…). C'est une
//!    convention posée ici, faute d'être dans l'issue.
//! 3. **`Affichage::Image` accepte les trois formats.** L'issue ne refuse pour `image` que « ni PNG,
//!    ni JPEG, ni GIF », et son hors-scope dit qu'« un GIF affiché comme image fixe est un cas
//!    légitime ». `Affichage::Gif`, lui, n'accepte que le GIF.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! **L'ordre des opérations dans le démon.** Le critère « un refus n'écrit pas `ecran.conf` » se
//! tient en deux moitiés : que la vérification soit **appelée avant** l'écriture, et qu'elle
//! n'écrive **rien** elle-même. La première moitié vit dans la boucle de commande du démon, qui
//! n'est pas atteignable depuis un test d'intégration — elle se vérifie sur la machine. La seconde
//! est ici, et c'est celle qui se démontre : la vérification laisse un `ecran.conf` existant
//! **octet pour octet** et n'en crée pas d'absent.
//!
//! **Le critère « un refus ne touche pas la dalle » est tenu par la signature**, non par un test :
//! `verifier_format` rend `Result<(), ImageInvalide>` et ne peut donc produire aucune `Dalle`. Rien
//! à pousser, rien à écrire sur le bus. C'est plus fort qu'une assertion, et ça ne se contourne pas.
//!
//! **Un GIF tronqué reste accepté**, et c'est assumé : le piège 4 est un arbitrage — la
//! vérification porte sur le **format annoncé**, pas sur l'intégrité du fichier. Un fichier du bon
//! format mais abîmé échouera plus loin, au décodage, comme aujourd'hui. L'issue traite le mauvais
//! format, pas le mauvais contenu, et la relecture au démarrage (#33) sait déjà ne pas boucler
//! dessus.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use reverb_daemon::ecran::{
    Affichage, Etat, ImageInvalide, charger, enregistrer, verifier_fichier, verifier_format,
};

// ---------------------------------------------------------------------------
// Témoins : trois images fabriquées ici, jamais lues du disque
// ---------------------------------------------------------------------------

/// Le côté des images témoins, en pixels.
///
/// Huit, et pas un : c'est le côté du bloc élémentaire du JPEG. Une image d'un pixel se code sans
/// erreur mais fait travailler l'encodeur sur un cas limite, et ce fichier a besoin de témoins dont
/// personne ne puisse dire qu'ils sont dégénérés.
const COTE: u32 = 8;

/// Les octets d'un PNG valide, encodés en mémoire.
///
/// Le crate `image` est déjà une dépendance de `reverb-daemon` (ADR-005) et Cargo l'expose aux
/// cibles de test : ces tests s'en servent pour **fabriquer** leurs entrées. Fabriquer plutôt que
/// livrer un fichier binaire dans le dépôt, parce qu'un fichier témoin qu'on ne peut pas relire en
/// diff est un fichier dont plus personne ne sait ce qu'il contient.
fn png_temoin() -> Vec<u8> {
    encoder(image::ImageFormat::Png)
}

/// Les octets d'un JPEG valide.
fn jpeg_temoin() -> Vec<u8> {
    encoder(image::ImageFormat::Jpeg)
}

/// Les octets d'un GIF valide.
fn gif_temoin() -> Vec<u8> {
    encoder(image::ImageFormat::Gif)
}

fn encoder(format: image::ImageFormat) -> Vec<u8> {
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        COTE,
        COTE,
        image::Rgb([0xff, 0x20, 0x80]),
    ));
    let mut octets = Cursor::new(Vec::new());
    image
        .write_to(&mut octets, format)
        .unwrap_or_else(|erreur| panic!("fabrication du témoin {format:?} : {erreur}"));
    octets.into_inner()
}

/// Des octets qui ne sont d'aucun format d'image.
///
/// Du texte, délibérément : un en-tête d'un **autre** format d'image (BMP, WebP…) serait reconnu
/// par le crate `image`, et ce témoin doit être « rien du tout », pas « autre chose ».
fn octets_quelconques() -> Vec<u8> {
    b"Ceci n'est pas une image. C'est une note laissee la par un humain presse.\n".to_vec()
}

/// Les trois témoins avec le nom de leur format, pour les tests qui balaient les trois.
fn temoins() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("PNG", png_temoin()),
        ("JPEG", jpeg_temoin()),
        ("GIF", gif_temoin()),
    ]
}

// ---------------------------------------------------------------------------
// Vérification des témoins eux-mêmes
// ---------------------------------------------------------------------------

#[test]
fn les_temoins_portent_bien_la_signature_de_leur_format() {
    // Ce test ne vérifie **rien du code testé** : il vérifie les entrées des autres tests. Sans lui,
    // une suite bâtie sur un faux PNG ne prouverait rien du tout — elle passerait ou échouerait pour
    // des raisons sans rapport avec l'issue, et personne ne le verrait.
    //
    // La vérification passe par les nombres magiques des trois formats, lus dans leurs
    // spécifications respectives, et **pas** par le crate `image` : un témoin fabriqué par `image`
    // puis validé par `image` ne démontrerait que la cohérence de `image` avec lui-même.
    let png = png_temoin();
    assert_eq!(
        png.get(..8),
        Some(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a][..]),
        "le témoin PNG doit commencer par la signature PNG (RFC 2083 §3.1), il commence par {:x?}",
        png.get(..8)
    );

    let jpeg = jpeg_temoin();
    assert_eq!(
        jpeg.get(..3),
        Some(&[0xff, 0xd8, 0xff][..]),
        "le témoin JPEG doit commencer par le marqueur SOI puis un marqueur (JFIF), il commence \
         par {:x?}",
        jpeg.get(..3)
    );

    let gif = gif_temoin();
    assert_eq!(
        gif.get(..4),
        Some(&b"GIF8"[..]),
        "le témoin GIF doit commencer par « GIF8 » (GIF89a §17), il commence par {:x?}",
        gif.get(..4)
    );

    // Et le témoin « rien du tout » ne doit ressembler à aucun des trois, sans quoi les tests de
    // format non reconnu testeraient un format reconnu.
    let quelconque = octets_quelconques();
    for signature in [&[0x89, b'P'][..], &[0xff, 0xd8][..], &b"GIF"[..]] {
        assert!(
            !quelconque.starts_with(signature),
            "le témoin « aucun format » commence par {signature:x?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 1 — un fichier qui n'est pas du format demandé est refusé, en nommant ce qu'il est
// ---------------------------------------------------------------------------

#[test]
fn un_jpeg_donne_a_gif_est_refuse_en_nommant_jpeg() {
    // Critère d'acceptation : « `screen gif <fichier qui n'est pas un GIF>` est refusé, en nommant
    // le format trouvé » et « le message nomme le fichier **et** ce qu'il est vraiment, pas
    // seulement ce qu'il n'est pas ».
    //
    // C'est le cas exact de l'issue : `gif:/home/lupink/Images/Wallpapers/pxfuel.jpg`. Il a été
    // accepté, persisté, et rejoué à chaque démarrage.
    //
    // Le nom du fichier est celui de l'issue, et il ne contient pas la chaîne « JPEG » — son
    // extension est `.jpg`. C'est ce qui donne son sens à la seconde assertion : « JPEG » ne peut
    // venir que du contenu reconnu. Voir la mise en garde sur `nomme`.
    let affichage = Affichage::Gif("/home/nico/Images/pxfuel.jpg".to_owned());
    let erreur = refus(&affichage, &jpeg_temoin());

    nomme(&erreur, "pxfuel.jpg", "le fichier en cause");
    nomme(&erreur, "JPEG", "le format réellement trouvé");
}

#[test]
fn un_png_donne_a_gif_est_refuse_en_nommant_png() {
    // Même critère, autre format trouvé. Deux formats et non un seul : une implémentation qui
    // écrirait « JPEG » en dur dans son message passerait le test précédent, et laisserait
    // l'utilisateur d'un PNG chercher un JPEG qu'il n'a pas.
    //
    // Le fichier est **sans extension**, délibérément. Un `fond.png` aurait fait passer l'assertion
    // « le message contient PNG » par la seule présence du chemin dans le message, et le test aurait
    // été vert devant une implémentation qui ne nomme jamais le format trouvé — ce mutant-là a
    // effectivement survécu au premier jet de ce fichier.
    let affichage = Affichage::Gif("/home/nico/Images/fond-d-ecran".to_owned());
    let erreur = refus(&affichage, &png_temoin());

    nomme(&erreur, "fond-d-ecran", "le fichier en cause");
    nomme(&erreur, "PNG", "le format réellement trouvé");
}

#[test]
fn des_octets_qui_ne_sont_aucun_des_trois_formats_sont_refuses_par_les_deux_verbes() {
    // Critère d'acceptation : « `screen image <fichier qui n'est ni PNG, ni JPEG, ni GIF>` est
    // refusé de même ».
    //
    // Les deux verbes, parce qu'un fichier qui n'est pas une image ne l'est pour aucun des deux, et
    // qu'une implémentation qui ne garderait que la branche `gif` laisserait `screen image` sur
    // l'ancien comportement — celui qui a produit l'état cassé de l'issue.
    for affichage in [
        Affichage::Image("/home/nico/notes/rappel.txt".to_owned()),
        Affichage::Gif("/home/nico/notes/rappel.txt".to_owned()),
    ] {
        let erreur = refus(&affichage, &octets_quelconques());

        nomme(&erreur, "rappel.txt", "le fichier en cause");
        // Rien à nommer comme format trouvé : le message doit le **dire**, pas se taire. Voir le
        // point 2 des conventions tranchées en tête de fichier.
        nomme(&erreur, "reconnu", "l'absence de format reconnaissable");
    }

    // Le cas dégénéré du fichier vide suit la même règle : zéro octet n'est pas une image, et un
    // fichier vide est ce que rend une copie interrompue. Il ne doit ni paniquer, ni passer.
    for affichage in [
        Affichage::Image("/home/nico/vide.png".to_owned()),
        Affichage::Gif("/home/nico/vide.gif".to_owned()),
    ] {
        let erreur = refus(&affichage, &[]);
        nomme(&erreur, "vide.", "le fichier en cause");
    }
}

// ---------------------------------------------------------------------------
// 2 — le pendant : ce qui est du bon format passe
// ---------------------------------------------------------------------------

#[test]
fn un_vrai_gif_donne_a_gif_est_accepte() {
    // Critère d'acceptation : « une commande acceptée écrit `ecran.conf` comme aujourd'hui ». Sans
    // ce test, une vérification qui refuserait **tout** satisfairait tous les tests de refus de ce
    // fichier — et rendrait l'écran du Kraken inutilisable, ce qui est une panne plus grave que
    // celle qu'on corrige.
    accord(
        &Affichage::Gif("/home/nico/anims/pluie.gif".to_owned()),
        &gif_temoin(),
    );
}

#[test]
fn les_trois_formats_sont_acceptes_par_image() {
    // L'issue ne refuse pour `image` que ce qui n'est « ni PNG, ni JPEG, ni GIF », et son hors-scope
    // pose qu'« un GIF affiché comme image fixe est un cas légitime » — c'est même la raison pour
    // laquelle elle refuse de fusionner les deux verbes. `screen image <un GIF>` doit donc passer :
    // c'est la première image du GIF, affichée fixe, et c'est un choix, pas une erreur.
    for (nom, octets) in temoins() {
        accord(
            &Affichage::Image(format!("/home/nico/images/temoin-{nom}")),
            &octets,
        );
    }
}

#[test]
fn un_affichage_sans_fichier_n_a_pas_de_format_a_verifier() {
    // La vérification est appelée sur la commande d'écran, quelle qu'elle soit. `screen off` et
    // `screen gauge <sonde>` ne nomment aucun fichier : il n'y a rien à reconnaître, et refuser
    // faute d'avoir trouvé un format éteindrait le cadran et empêcherait de rendre la dalle au
    // firmware — donc empêcherait de **sortir** d'un affichage fautif, ce qui est exactement la
    // situation de l'issue.
    for affichage in [
        Affichage::Rien,
        Affichage::Cadran("kraken2023elite:coolant".to_owned()),
    ] {
        accord(&affichage, &octets_quelconques());
        accord(&affichage, &[]);
    }
}

// ---------------------------------------------------------------------------
// 3 — le verdict vient du contenu, jamais de l'extension
// ---------------------------------------------------------------------------

#[test]
fn le_verdict_vient_du_contenu_pas_de_l_extension() {
    // Ce test est le cœur de l'issue, et il se lit dans les deux sens.
    //
    // L'état fautif relevé sur SHYNAEL portait `.jpg` sur un verbe `gif` : une vérification qui
    // regarderait l'extension aurait bien attrapé ce cas-là. Elle serait pourtant fausse, parce
    // qu'une extension n'est qu'un nom que l'utilisateur donne à son fichier — un GIF téléchargé
    // sous `.jpg` est un GIF, et un JPEG renommé `.gif` reste un JPEG.
    //
    // La méprise est pire que le défaut d'origine : elle **refuserait** un fichier affichable et
    // **accepterait** celui qui a cassé la dalle, tout en donnant l'impression que le contrôle
    // marche.
    let gif_deguise = Affichage::Gif("/home/nico/telechargements/anim.jpg".to_owned());
    accord(&gif_deguise, &gif_temoin());

    let jpeg_deguise = Affichage::Gif("/home/nico/telechargements/photo.gif".to_owned());
    let erreur = refus(&jpeg_deguise, &jpeg_temoin());
    nomme(&erreur, "photo.gif", "le fichier en cause");
    nomme(&erreur, "JPEG", "le format réellement trouvé");

    // Et sans aucune extension : un fichier peut n'en avoir pas.
    accord(
        &Affichage::Gif("/home/nico/telechargements/sans-suffixe".to_owned()),
        &gif_temoin(),
    );
    let erreur = refus(
        &Affichage::Gif("/home/nico/telechargements/sans-suffixe".to_owned()),
        &png_temoin(),
    );
    nomme(&erreur, "PNG", "le format réellement trouvé");
}

// ---------------------------------------------------------------------------
// 4 — reconnaître un format ne coûte que son en-tête
// ---------------------------------------------------------------------------

#[test]
fn le_verdict_ne_depend_pas_de_ce_qui_suit_l_en_tete() {
    // Approche technique de l'issue : « Le crate `image` sait reconnaître un format sans décoder
    // toute l'image (`ImageReader::with_guessed_format`), ce qui rend la vérification bon marché
    // même sur un fichier de plusieurs mégaoctets. »
    //
    // Une vérification par décodage complet serait indétectable sur les témoins de huit pixels de
    // ce fichier, et paierait plusieurs mégaoctets sur un vrai fond d'écran — sur le chemin d'une
    // commande interactive, deux fois de suite puisque le décodage réel suit. Le fait observable
    // est donc celui-ci : le verdict d'un fichier réduit à son en-tête est le **même** que celui du
    // fichier entier.
    //
    // Arbitrage assumé et documenté en tête : un fichier du bon format mais tronqué est **accepté**
    // ici, et échouera au décodage comme aujourd'hui. L'issue traite le format déclaré, pas
    // l'intégrité du contenu.
    for (nom, octets) in temoins() {
        let en_tete = &octets[..16.min(octets.len())];
        let affichage = Affichage::Image(format!("/home/nico/images/tronque-{nom}"));
        accord(&affichage, en_tete);
    }

    // Le même raisonnement du côté du refus : un JPEG reconnu à son en-tête est refusé par `gif`
    // sans qu'il ait fallu lire la suite.
    let jpeg = jpeg_temoin();
    let erreur = refus(
        &Affichage::Gif("/home/nico/images/tronque.jpg".to_owned()),
        &jpeg[..16.min(jpeg.len())],
    );
    nomme(&erreur, "JPEG", "le format réellement trouvé");
}

// ---------------------------------------------------------------------------
// 5 — la vérification sur fichier rend le même verdict, et nomme un fichier absent
// ---------------------------------------------------------------------------

#[test]
fn la_verification_sur_fichier_rend_le_meme_verdict_que_sur_les_octets() {
    // `verifier_fichier` est ce que le démon appelle : il a un chemin, pas des octets. Il doit être
    // le **même** contrôle, sans quoi la fonction pure que ce fichier couvre ne prouverait rien de
    // ce qui tourne réellement.
    let dossier = DossierJetable::neuf("meme-verdict");

    for (rang, (nom, octets)) in temoins().into_iter().enumerate() {
        // Extensions volontairement toutes identiques et toutes fausses pour deux témoins sur
        // trois : le verdict sur fichier ne doit pas non plus se lire dans le nom.
        //
        // Le nom du format n'apparaît **pas** dans le nom du fichier — un rang, pas « PNG ». Le
        // message d'erreur porte le chemin, et un `temoin-PNG.gif` y ferait passer l'assertion
        // « le message nomme PNG » sans qu'aucune reconnaissance n'ait eu lieu.
        let chemin = dossier.fichier(&format!("temoin-{rang}.gif"));
        fs::write(&chemin, &octets)
            .unwrap_or_else(|erreur| panic!("écriture de {} : {erreur}", chemin.display()));

        for construire in [constructeur_image, constructeur_gif] {
            let affichage = construire(&chemin);
            let attendu = verifier_format(&affichage, &octets);
            let obtenu = verifier_fichier(&affichage);
            assert_eq!(
                attendu.is_ok(),
                obtenu.is_ok(),
                "{affichage:?} sur un {nom} : les octets disent {attendu:?}, le fichier dit \
                 {obtenu:?} — deux contrôles qui divergent, c'est celui qui n'est pas testé qui \
                 tourne en production"
            );
            if let Err(erreur) = obtenu {
                nomme(&erreur, "temoin-", "le fichier en cause");
                nomme(&erreur, nom, "le format réellement trouvé");
            }
        }
    }
}

#[test]
fn un_fichier_inexistant_est_refuse_en_le_nommant() {
    // Test d'intention listé par l'issue : « un fichier inexistant est toujours refusé en le
    // nommant, comme avant ». C'est le mode de défaillance le plus courant — un chemin collé de
    // travers — et il ne doit pas se dégrader en « format non reconnu », qui enverrait chercher un
    // problème de format sur un fichier qui n'est pas là.
    let dossier = DossierJetable::neuf("inexistant");
    let chemin = dossier.fichier("jamais-ecrit.png");

    for construire in [constructeur_image, constructeur_gif] {
        let affichage = construire(&chemin);
        match verifier_fichier(&affichage) {
            Ok(()) => panic!(
                "{affichage:?} désigne un fichier qui n'existe pas et a pourtant été accepté — \
                 c'est un état que le démon persisterait puis rejouerait à chaque démarrage"
            ),
            Err(erreur) => {
                assert!(
                    !erreur.raison.trim().is_empty(),
                    "{affichage:?} doit être refusé **en le disant**"
                );
                nomme(&erreur, "jamais-ecrit.png", "le fichier en cause");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 6 — un refus n'écrit rien
// ---------------------------------------------------------------------------

#[test]
fn un_refus_laisse_l_etat_persiste_intact_au_bit_pres() {
    // Critère d'acceptation : « un refus **n'écrit pas** `ecran.conf` : l'état d'avant est intact,
    // au bit près ».
    //
    // C'est la moitié du critère qui se démontre ici (voir « ce que ce fichier ne teste pas » en
    // tête) : la vérification est sans effet de bord. L'autre moitié — qu'elle soit appelée
    // **avant** l'écriture — vit dans la boucle de commande du démon.
    //
    // « Au bit près » et non « équivalent » : un fichier réécrit à l'identique aurait déjà changé de
    // date, et surtout aurait prouvé qu'un chemin d'écriture est emprunté sur un refus. C'est ce
    // chemin-là qui a laissé un `gif:` sur un `.jpg` dans `/var/lib/reverb/ecran.conf`.
    let dossier = DossierJetable::neuf("refus-n-ecrit-pas");
    let ecran_conf = dossier.fichier("ecran.conf");
    let jamais_cree = dossier.fichier("ecran.conf.absent");

    let avant = Etat {
        luminosite: 42,
        affichage: Affichage::Cadran("kraken2023elite:coolant".to_owned()),
    };
    enregistrer(&ecran_conf, &avant).expect("l'état témoin doit s'écrire");
    let octets_avant = fs::read(&ecran_conf).expect("l'état témoin doit se relire");

    // Tous les refus de ce fichier, l'un après l'autre, sur les deux fonctions.
    let refusables: Vec<(Affichage, Vec<u8>)> = vec![
        (
            Affichage::Gif(chaine(&dossier.fichier("photo.jpg"))),
            jpeg_temoin(),
        ),
        (
            Affichage::Gif(chaine(&dossier.fichier("fond.png"))),
            png_temoin(),
        ),
        (
            Affichage::Image(chaine(&dossier.fichier("rappel.txt"))),
            octets_quelconques(),
        ),
        (Affichage::Image(chaine(&jamais_cree)), Vec::new()),
    ];

    for (affichage, octets) in &refusables {
        assert!(
            verifier_format(affichage, octets).is_err(),
            "{affichage:?} devait être refusé"
        );
        // Et sur fichier : celui-ci n'existe pas, ce qui est un refus de plus.
        assert!(
            verifier_fichier(affichage).is_err(),
            "{affichage:?} devait être refusé sur fichier"
        );
    }

    assert_eq!(
        fs::read(&ecran_conf).expect("l'état doit toujours être là"),
        octets_avant,
        "un refus a modifié l'état persisté — l'état d'avant doit rester intact au bit près"
    );
    assert!(
        !jamais_cree.exists(),
        "un refus a créé {} — un refus n'écrit rien, pas même un fichier vide",
        jamais_cree.display()
    );

    // Et l'état relu est bien celui d'avant, pas un état plausible reconstruit.
    assert_eq!(
        charger(&ecran_conf).0,
        avant,
        "l'état relu après une série de refus n'est plus celui d'avant"
    );
}

// ---------------------------------------------------------------------------
// 7 — ce qui est accepté se persiste comme aujourd'hui
// ---------------------------------------------------------------------------

#[test]
fn un_affichage_accepte_s_ecrit_et_se_relit_comme_avant() {
    // Critère d'acceptation : « une commande acceptée écrit `ecran.conf` comme aujourd'hui ». La
    // vérification s'ajoute devant la persistance, elle ne la change pas : le fichier garde sa
    // forme, et un aller-retour rend le même état.
    let dossier = DossierJetable::neuf("accepte-se-persiste");
    let ecran_conf = dossier.fichier("ecran.conf");

    for (nom, octets) in temoins() {
        let chemin = dossier.fichier(&format!("accepte-{nom}"));
        fs::write(&chemin, &octets)
            .unwrap_or_else(|erreur| panic!("écriture de {} : {erreur}", chemin.display()));

        let affichage = Affichage::Image(chaine(&chemin));
        verifier_fichier(&affichage).unwrap_or_else(|erreur| {
            panic!("un {nom} donné à `screen image` doit être accepté : {erreur}")
        });

        let etat = Etat {
            luminosite: 77,
            affichage,
        };
        enregistrer(&ecran_conf, &etat).expect("un affichage accepté doit s'écrire");
        assert_eq!(
            charger(&ecran_conf).0,
            etat,
            "un affichage accepté doit se relire tel quel — la vérification ne change pas le \
             format du fichier d'état"
        );
    }
}

#[test]
fn la_verification_ne_s_invite_pas_dans_la_relecture_au_demarrage() {
    // Critère d'acceptation : « `ecran.conf` relu au démarrage sur un fichier devenu illisible **ne
    // boucle pas** : le journal le dit une fois, la dalle reste au firmware (acquis de #33, à ne pas
    // casser) ».
    //
    // Le risque de régression est précis : brancher la vérification dans `charger` ferait échouer la
    // relecture d'un état pourtant bien formé, dont le fichier a simplement été déplacé depuis. Le
    // démon repartirait alors sur l'accueil, en bleu, au lieu de reprendre l'affichage voulu dès que
    // le fichier revient. La vérification appartient au chemin de **commande**, pas à celui du
    // démarrage.
    let dossier = DossierJetable::neuf("relecture-permissive");
    let ecran_conf = dossier.fichier("ecran.conf");
    let disparu = dossier.fichier("fond-efface.png");

    let etat = Etat {
        luminosite: 63,
        affichage: Affichage::Image(chaine(&disparu)),
    };
    enregistrer(&ecran_conf, &etat).expect("l'état témoin doit s'écrire");

    let (relu, message) = charger(&ecran_conf);
    assert_eq!(
        relu, etat,
        "un état bien formé se relit tel quel, même si le fichier qu'il nomme a disparu"
    );
    assert_eq!(
        message, None,
        "un état bien formé ne produit aucun message au démarrage : {message:?}"
    );
}

// ---------------------------------------------------------------------------
// Outils du fichier
// ---------------------------------------------------------------------------

/// Exige l'accord, et dit ce qui a été refusé sinon.
fn accord(affichage: &Affichage, octets: &[u8]) {
    if let Err(erreur) = verifier_format(affichage, octets) {
        panic!(
            "{affichage:?} devait être accepté sur {} octet(s), il a été refusé : {erreur} — une \
             vérification qui refuse ce qui s'affiche rend l'écran inutilisable",
            octets.len()
        );
    }
}

/// Exige le refus, avec ce que ce fichier exige de tout refus : une raison non vide, un `Display`
/// qui la porte, et un type qui soit une vraie erreur.
///
/// Convention reprise de `spec_ecran.rs` (#33) : un refus muet est un refus qu'on ne peut pas
/// corriger.
fn refus(affichage: &Affichage, octets: &[u8]) -> ImageInvalide {
    match verifier_format(affichage, octets) {
        Ok(()) => panic!(
            "{affichage:?} devait être refusé sur {} octet(s) — c'est un affichage impossible, et \
             l'accepter le persiste puis le rejoue à chaque démarrage du service",
            octets.len()
        ),
        Err(erreur) => {
            assert!(
                !erreur.raison.trim().is_empty(),
                "{affichage:?} doit être refusé **en disant pourquoi**"
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

/// Exige que la raison du refus contienne `aiguille`, sans distinction de casse.
///
/// Sans distinction de casse parce que le message est libre de dire « JPEG », « Jpeg » ou
/// « image/jpeg » : ce fichier exige que l'information soit là, pas qu'elle soit orthographiée
/// d'une façon.
///
/// ⚠️ **Le message porte le chemin, et un chemin porte souvent son format.** Chercher « PNG » dans
/// un message qui contient déjà `fond.png` ne prouve rien : l'assertion passe devant une
/// implémentation qui ne nomme jamais le format trouvé. Tout test qui exige un nom de format le
/// fait donc sur un chemin qui ne le contient pas — sans extension, ou avec une extension qui ne
/// s'écrit pas comme le format (`.jpg` face à « JPEG »). C'est un mutant survivant qui l'a montré,
/// pas une relecture.
fn nomme(erreur: &ImageInvalide, aiguille: &str, quoi: &str) {
    assert!(
        erreur
            .raison
            .to_uppercase()
            .contains(&aiguille.to_uppercase()),
        "le refus doit nommer {quoi} (« {aiguille} ») : « {} »",
        erreur.raison
    );
}

fn constructeur_image(chemin: &Path) -> Affichage {
    Affichage::Image(chaine(chemin))
}

fn constructeur_gif(chemin: &Path) -> Affichage {
    Affichage::Gif(chaine(chemin))
}

/// Le chemin en texte, tel que le protocole le transporte et que `Affichage` le porte.
fn chaine(chemin: &Path) -> String {
    chemin
        .to_str()
        .unwrap_or_else(|| panic!("chemin de test non-UTF-8 : {}", chemin.display()))
        .to_owned()
}

/// Un dossier de travail sous `std::env::temp_dir()`, effacé à la fin du test.
///
/// L'effacement passe par `Drop` et non par une ligne en fin de test : un test qui échoue part en
/// `panic!`, et une fin de test jamais atteinte laisserait le dossier derrière elle à chaque
/// régression. Convention reprise de `spec_ecran.rs`.
struct DossierJetable {
    chemin: PathBuf,
}

impl DossierJetable {
    /// `nom` doit être le nom du test : c'est ce qui rend le chemin unique entre tests, `cargo` les
    /// exécutant en parallèle dans un même processus.
    fn neuf(nom: &str) -> DossierJetable {
        let chemin =
            std::env::temp_dir().join(format!("reverb-spec-format-{}-{nom}", std::process::id()));
        let _ = fs::remove_dir_all(&chemin);
        fs::create_dir_all(&chemin)
            .unwrap_or_else(|erreur| panic!("dossier de test {} : {erreur}", chemin.display()));
        DossierJetable { chemin }
    }

    fn fichier(&self, nom: &str) -> PathBuf {
        self.chemin.join(nom)
    }
}

impl Drop for DossierJetable {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.chemin);
    }
}
