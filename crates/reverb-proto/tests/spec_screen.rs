//! Tests d'intention — pilotage de l'écran du Kraken Elite (`1e71:300c`), issue #13.
//!
//! Ce que ces tests garantissent : les trames de contrôle et l'en-tête bulk produits par le
//! module `screen` reproduisent **exactement** ce qui a été relevé dans la capture USB de
//! NZXT CAM (`docs/SPEC-KRAKEN-LCD.md`), et que la réponse d'état `31 01` se décode en les
//! valeurs que le contrôleur annonce lui-même. S'y ajoute la mire de quadrants, seul contenu
//! que Reverb engendre lui-même : c'est elle qui doit trancher la question ouverte n°2 de la
//! spec (RGB ou BGR), donc elle doit être exacte avant d'être crue. Rien ici ne touche au
//! matériel : `reverb-proto` est pur, ses tests aussi — ni `/dev`, ni `/sys`, ni écriture de
//! fichier.
//!
//! **Les deux pièges de cette cible sont tous deux silencieux**, d'où l'insistance des tests :
//!
//! 1. **`38 01 02 00` avant l'image.** Sans ce passage en mode de diffusion, le téléversement
//!    réussit, aucun code d'erreur ne remonte, et l'écran ignore purement et simplement
//!    l'image (spec §2.2.1, §3.5). C'est le piège qui coûte le plus cher : il ne ressemble
//!    pas à une panne. La trame est donc verrouillée octet par octet.
//! 2. **Le paquet de longueur nulle après l'image.** 1 228 800 = 2400 × 512, un multiple exact
//!    de `wMaxPacketSize` : sans ZLP le contrôleur ne sait pas où une image s'arrête, il
//!    concatène la suivante et l'affichage dérive (spec §2.2.1). Ce piège-là ne se teste pas
//!    ici — il vit dans le transfert usbfs de `reverb-cli` — mais c'est lui qui explique
//!    pourquoi la longueur annoncée dans l'en-tête bulk est verrouillée avec autant de soin :
//!    c'est le seul endroit du domaine pur où la taille de l'image est déclarée au matériel.
//!
//! Toutes les valeurs attendues viennent de la capture USB ou de l'issue #13, jamais d'une
//! déduction de ce test. Chaque message d'échec rappelle sa source : si un octet ne correspond
//! plus, c'est soit une régression du code, soit une capture à refaire, et il faut de quoi
//! trancher sans rouvrir la spec.

use reverb_proto::screen;

/// Réponse d'état relevée telle quelle dans la capture (issue #13, contrat d'API).
///
/// `31 01 bb 8c 90 82 0e 90 06 30 00 00 00 00 05 00 80 00 00 10 80 02 80 02 50 01 00 ff 00 00 00 00`
///
/// Décodage attendu : largeur 640 (`0x14`–`0x15`), hauteur 640 (`0x16`–`0x17`),
/// luminosité 80 (`0x18`), orientation 0 (`0x1a`) — entiers petit-boutistes.
const TRAME_ETAT_CAPTURE: [u8; 32] = [
    0x31, 0x01, 0xbb, 0x8c, 0x90, 0x82, 0x0e, 0x90, 0x06, 0x30, 0x00, 0x00, 0x00, 0x00, 0x05, 0x00,
    0x80, 0x00, 0x00, 0x10, 0x80, 0x02, 0x80, 0x02, 0x50, 0x01, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00,
];

/// En-tête bulk relevé tel quel dans la capture (spec §2, issue #13) :
///
/// `12 fa 01 e8 | ab cd ef 98 76 54 32 10 | 09 00 00 00 | 00 c0 12 00`
///
/// La signature du milieu est invariante sur les 50 images capturées ✅ ; le champ final est
/// la longueur `0x0012c000` = 1 228 800. Les blocs `12 fa 01 e8` et `09 00 00 00` sont 🔶 :
/// constants ici, rôle inconnu faute d'avoir observé une image d'une autre taille.
const ENTETE_BULK_CAPTURE: [u8; 20] = [
    0x12, 0xfa, 0x01, 0xe8, 0xab, 0xcd, 0xef, 0x98, 0x76, 0x54, 0x32, 0x10, 0x09, 0x00, 0x00, 0x00,
    0x00, 0xc0, 0x12, 0x00,
];

/// Signature magique de l'en-tête bulk, offsets 4 à 11 (spec §2, ✅ invariante).
const SIGNATURE_BULK: [u8; 8] = [0xab, 0xcd, 0xef, 0x98, 0x76, 0x54, 0x32, 0x10];

/// Offset du champ de longueur dans l'en-tête bulk.
const OFFSET_LONGUEUR: usize = 16;

/// Offset de l'orientation dans la réponse `31 01` : c'est le champ le plus loin dans la
/// trame, donc le dernier octet dont le décodeur a besoin.
const OFFSET_ORIENTATION: usize = 0x1a;

/// Un rapport HID de sortie fait 64 octets, premier octet compris : c'est l'identifiant de
/// rapport, pas un préfixe `0x00` à ajouter (spec §0, `CLAUDE.md`).
const TAILLE_TRAME_HID: usize = 64;

/// Lit un octet sans indexation nue : sur une trame plus courte que prévu, on veut un échec
/// de test qui explique ce qui manque, jamais un panic d'index opaque.
fn octet(trame: &[u8], offset: usize, role: &str) -> u8 {
    match trame.get(offset) {
        Some(valeur) => *valeur,
        None => panic!(
            "trame de {} octets : impossible d'y lire l'offset {offset} ({role}), \
             qui doit exister d'après la capture USB",
            trame.len()
        ),
    }
}

/// Longueur annoncée par un en-tête bulk, petit-boutiste sur 4 octets.
fn longueur_annoncee(entete: &[u8]) -> u32 {
    u32::from_le_bytes([
        octet(entete, OFFSET_LONGUEUR, "longueur, octet 0"),
        octet(entete, OFFSET_LONGUEUR + 1, "longueur, octet 1"),
        octet(entete, OFFSET_LONGUEUR + 2, "longueur, octet 2"),
        octet(entete, OFFSET_LONGUEUR + 3, "longueur, octet 3"),
    ])
}

/// Vérifie qu'une trame de contrôle commence par les octets observés dans la capture, et que
/// tout le reste est du remplissage à zéro : la spec n'écrit que les octets significatifs,
/// inventer un octet au-delà serait sortir de ce qui a été observé.
fn verifie_trame(trame: &[u8], attendu: &[u8], nom: &str, source: &str) {
    assert_eq!(
        trame.len(),
        TAILLE_TRAME_HID,
        "{nom} fait {} octets, or un rapport HID de sortie en fait {TAILLE_TRAME_HID}, \
         identifiant de rapport compris — un rapport de la mauvaise taille est refusé par \
         le noyau à l'écriture sur /dev/hidraw*",
        trame.len()
    );

    for (offset, valeur_attendue) in attendu.iter().enumerate() {
        let obtenu = octet(trame, offset, nom);
        assert_eq!(
            obtenu, *valeur_attendue,
            "{nom} : octet {offset} vaut {obtenu:#04x}, attendu {valeur_attendue:#04x}. \
             La séquence attendue est relevée dans {source} — un écart signifie soit une \
             régression de l'encodage, soit une capture à refaire"
        );
    }

    for (offset, valeur) in trame.iter().enumerate().skip(attendu.len()) {
        assert_eq!(
            *valeur,
            0x00,
            "{nom} : octet {offset} vaut {valeur:#04x}, attendu 0x00. La capture ne montre \
             que {} octets significatifs ({source}) ; le reste est du remplissage, et \
             inventer un octet au-delà de ce qui a été observé est précisément ce que la \
             spec interdit",
            attendu.len()
        );
    }
}

// ---------------------------------------------------------------------------
// 1 — en-tête bulk : taille et signature
// ---------------------------------------------------------------------------

#[test]
fn l_en_tete_bulk_fait_vingt_octets_et_porte_la_signature_de_la_capture() {
    let longueur_image = u32::try_from(screen::IMAGE_LEN)
        .expect("IMAGE_LEN (1 228 800) doit tenir dans le champ de longueur de 4 octets");
    let entete = screen::bulk_header(longueur_image);

    assert_eq!(
        screen::BULK_HEADER_LEN,
        20,
        "BULK_HEADER_LEN vaut {}, or les deux transferts bulk observés dans la capture sont \
         un en-tête de 20 octets puis les pixels (spec §2) — une autre taille décale tout le \
         champ de longueur",
        screen::BULK_HEADER_LEN
    );

    assert_eq!(
        entete.len(),
        screen::BULK_HEADER_LEN,
        "bulk_header() rend {} octets alors que BULK_HEADER_LEN en annonce {} : le contrôleur \
         lit un en-tête de taille fixe, une longueur différente décale tous les champs",
        entete.len(),
        screen::BULK_HEADER_LEN
    );

    let signature = entete
        .get(4..12)
        .expect("l'en-tête bulk doit contenir les offsets 4 à 11, siège de la signature");
    assert_eq!(
        signature,
        SIGNATURE_BULK.as_slice(),
        "signature de l'en-tête bulk : {signature:02x?}, attendu {SIGNATURE_BULK:02x?}. \
         Cette suite est invariante sur les 50 images de la capture USB (spec §2, ✅) ; sans \
         elle le contrôleur ne reconnaît pas le transfert"
    );

    // L'en-tête d'une image de 1 228 800 octets est intégralement connu : il a été relevé
    // dans la capture. On le compare tel quel plutôt que champ par champ.
    assert_eq!(
        entete.as_slice(),
        ENTETE_BULK_CAPTURE.as_slice(),
        "l'en-tête produit pour une image de 1 228 800 octets ne reproduit pas celui de la \
         capture USB (spec §2). Obtenu {:02x?}, attendu {:02x?}",
        entete.as_slice(),
        ENTETE_BULK_CAPTURE.as_slice()
    );
}

// ---------------------------------------------------------------------------
// 2 — le champ de longueur annonce la taille réelle des données
// ---------------------------------------------------------------------------

#[test]
fn le_champ_de_longueur_de_l_en_tete_annonce_la_taille_reelle_des_donnees() {
    let longueur_image = u32::try_from(screen::IMAGE_LEN)
        .expect("IMAGE_LEN (1 228 800) doit tenir dans le champ de longueur de 4 octets");

    // Sur l'image du Kraken, la valeur attendue est celle de la capture : 0x0012c000.
    let entete = screen::bulk_header(longueur_image);
    assert_eq!(
        longueur_annoncee(&entete),
        0x0012_c000,
        "pour une image de 640×640×3, l'en-tête doit annoncer 0x0012c000 = 1 228 800 octets, \
         valeur relevée dans la capture USB (spec §2). Le champ de longueur doit correspondre \
         exactement à la taille du transfert suivant : c'est le seul endroit où le contrôleur \
         apprend où l'image s'arrête"
    );

    // Et la valeur doit suivre l'argument, sinon la fonction n'encode rien : elle recopie.
    // On ne vérifie ici que le champ de longueur — le rôle des blocs `12 fa 01 e8` et
    // `09 00 00 00` pour une image d'une autre taille est 🔶 (question ouverte n°3 de la
    // spec), donc on n'affirme rien à leur sujet.
    for longueur in [0_u32, 1, 512, 1_228_799, 1_228_801, u32::MAX] {
        let entete = screen::bulk_header(longueur);
        assert_eq!(
            longueur_annoncee(&entete),
            longueur,
            "bulk_header({longueur}) annonce {} octets au lieu de {longueur} : le champ de \
             longueur (offsets 16 à 19, petit-boutiste) doit valoir exactement la taille des \
             données annoncées, ni une constante figée ni un arrondi",
            longueur_annoncee(&entete)
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — refus d'une image de taille inattendue, avant toute ouverture de périphérique
// ---------------------------------------------------------------------------

#[test]
fn une_image_de_taille_inattendue_est_refusee_en_nommant_les_deux_tailles() {
    assert_eq!(
        screen::IMAGE_LEN,
        1_228_800,
        "IMAGE_LEN vaut {} au lieu de 1 228 800. Cette taille n'est pas un choix : c'est \
         640 × 640 × 3, et le contrôleur annonce lui-même sa résolution dans sa réponse \
         `31 01` (spec §2)",
        screen::IMAGE_LEN
    );
    assert_eq!(
        screen::IMAGE_LEN,
        usize::from(screen::WIDTH) * usize::from(screen::HEIGHT) * 3,
        "IMAGE_LEN ({}) ne correspond pas à WIDTH × HEIGHT × 3 ({} × {} × 3) : liquidctl 1.16 \
         est justement cassé sur cette cible pour avoir gardé les 4 octets par pixel du \
         Kraken Z3, quand la capture de CAM en montre 3",
        screen::IMAGE_LEN,
        screen::WIDTH,
        screen::HEIGHT
    );

    // Une image de la bonne taille passe : ce test ne doit pas être satisfait par un refus
    // systématique. Aucun périphérique n'est ouvert ici — la vérification est pure, ce qui
    // est justement l'exigence : elle doit pouvoir échouer avant tout accès à /dev.
    let image_valide = vec![0_u8; screen::IMAGE_LEN];
    assert!(
        screen::check_image(&image_valide).is_ok(),
        "une image de {} octets, exactement la taille attendue, doit être acceptée",
        screen::IMAGE_LEN
    );

    for taille in [
        0,
        1,
        screen::IMAGE_LEN - 1,
        screen::IMAGE_LEN + 1,
        // 640 × 640 × 4 : le format que produit liquidctl, et qu'il faut refuser.
        640 * 640 * 4,
    ] {
        let image = vec![0_u8; taille];
        match screen::check_image(&image) {
            Ok(()) => panic!(
                "une image de {taille} octets a été acceptée alors que le contrôleur attend \
                 exactement {} octets. Une taille erronée n'est pas rattrapable côté \
                 matériel : elle décale l'affichage sans aucun message d'erreur",
                screen::IMAGE_LEN
            ),
            Err(screen::ImageError::WrongLength { given, expected }) => {
                assert_eq!(
                    given, taille,
                    "l'erreur annonce une taille reçue de {given} octets au lieu de {taille} : \
                     le message montré à l'utilisateur doit donner la taille de *son* fichier, \
                     sinon il ne peut pas savoir de combien il se trompe"
                );
                assert_eq!(
                    expected,
                    screen::IMAGE_LEN,
                    "l'erreur annonce une taille attendue de {expected} octets au lieu de {} : \
                     c'est cette valeur qui permet à l'utilisateur de recalculer sa commande \
                     ffmpeg (640×640 en bgr24)",
                    screen::IMAGE_LEN
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — la luminosité refuse une valeur hors de 0..=100
// ---------------------------------------------------------------------------

#[test]
fn la_luminosite_hors_de_zero_cent_est_refusee_en_rappelant_la_valeur_donnee() {
    for pourcent in 0_u8..=100 {
        assert!(
            screen::set_brightness(pourcent).is_ok(),
            "la luminosité {pourcent} % est refusée alors que la spec §3.4 documente l'octet \
             de luminosité comme un pourcentage de 0 à 100, vérifié à l'œil entre 5 et 100"
        );
    }

    for pourcent in [101_u8, 128, 200, 255] {
        match screen::set_brightness(pourcent) {
            Ok(_) => panic!(
                "la luminosité {pourcent} % a été acceptée : la spec §3.4 borne le champ à \
                 0..=100, et rien n'a été observé au-delà. Envoyer une valeur non observée \
                 revient à écrire depuis une hypothèse"
            ),
            Err(screen::BrightnessError::OutOfRange { given }) => assert_eq!(
                given, pourcent,
                "l'erreur rapporte {given} au lieu de {pourcent} : l'utilisateur doit lire \
                 dans le message la valeur qu'il a lui-même tapée"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — la luminosité est encodée à l'offset 3
// ---------------------------------------------------------------------------

#[test]
fn la_trame_de_luminosite_porte_le_pourcentage_a_l_offset_trois() {
    // Trame relevée dans la capture, spec §3.4 (✅ vérifiée visuellement) :
    //   30 02 01 <lum> 00 00 00 00 1e
    //      │  │  │     └── luminosité, 0..100, offset 3
    //      │  │  └──────── 0x01 : actif
    //      └──┴─────────── commande
    // L'octet 0x1e de fin est 🔶 (rôle non établi : il vaut 30, comme le délai de repli du
    // §2.2.2, mais l'hypothèse n'a pas été vérifiée) alors que sa *valeur*, elle, est une
    // observation — identique dans toutes les trames capturées. On la reproduit sans lui
    // prêter de sens, comme le §3.4 le demande explicitement.
    for pourcent in [0_u8, 5, 50, 80, 100] {
        let resultat = screen::set_brightness(pourcent);
        let trame = match &resultat {
            Ok(trame) => trame,
            Err(_) => panic!(
                "la luminosité {pourcent} % est dans 0..=100 et doit produire une trame \
                 (spec §3.4)"
            ),
        };

        let attendu = [0x30, 0x02, 0x01, pourcent, 0x00, 0x00, 0x00, 0x00, 0x1e];
        verifie_trame(
            trame,
            &attendu,
            &format!("trame de luminosité {pourcent} %"),
            "la capture USB de CAM, spec §3.4",
        );

        assert_eq!(
            octet(trame, 3, "luminosité"),
            pourcent,
            "l'octet 3 de la trame de luminosité vaut {} au lieu de {pourcent} : c'est le seul \
             octet à porter le pourcentage (spec §3.4, confirmé à l'œil en alternant 5 et 100)",
            octet(trame, 3, "luminosité")
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — décodage de la réponse d'état `31 01`
// ---------------------------------------------------------------------------

#[test]
fn la_reponse_31_01_de_la_capture_se_decode_en_640_640_80_0() {
    let etat = match screen::parse_state(&TRAME_ETAT_CAPTURE) {
        Ok(etat) => etat,
        Err(_) => panic!(
            "la réponse `31 01` relevée dans la capture USB a été rejetée : c'est pourtant \
             une trame réelle, émise par le contrôleur lui-même en réponse à `30 01`"
        ),
    };

    assert_eq!(
        etat.width, 640,
        "largeur décodée : {} au lieu de 640. Les octets 0x14–0x15 de la capture valent \
         `80 02`, soit 0x0280 = 640 en petit-boutiste — une lecture gros-boutiste donnerait \
         0x8002 = 32770",
        etat.width
    );
    assert_eq!(
        etat.height, 640,
        "hauteur décodée : {} au lieu de 640. Octets 0x16–0x17 de la capture : `80 02`, \
         petit-boutiste. Le contrôleur annonce lui-même sa résolution, on ne la suppose pas",
        etat.height
    );
    assert_eq!(
        etat.brightness, 80,
        "luminosité décodée : {} au lieu de 80. Octet 0x18 de la capture : 0x50 = 80 %",
        etat.brightness
    );
    assert_eq!(
        etat.orientation, 0,
        "orientation décodée : {} au lieu de 0. Octet 0x1a de la capture : 0x00. Attention à \
         ne pas lire 0x19 (qui vaut 0x01 dans cette même trame) : les deux champs se \
         ressemblent et se confondent facilement",
        etat.orientation
    );
}

// ---------------------------------------------------------------------------
// 7 — une réponse tronquée est rejetée, sans débordement
// ---------------------------------------------------------------------------

#[test]
fn une_reponse_31_01_tronquee_est_rejetee_sans_deborder_du_tampon() {
    // Toute trame plus courte que 0x1b octets ne peut pas contenir l'orientation (offset
    // 0x1a) : le décodeur doit refuser, pas lire hors bornes. Une lecture HID interrompue
    // ou un rapport partiel rend exactement ce genre de trame.
    for longueur in 0..=OFFSET_ORIENTATION {
        let tronquee = &TRAME_ETAT_CAPTURE[..longueur];
        match screen::parse_state(tronquee) {
            Ok(_) => panic!(
                "une trame de {longueur} octets a été décodée alors qu'il en faut au moins \
                 {} pour atteindre l'orientation (offset {OFFSET_ORIENTATION}) : les champs \
                 rendus seraient de la mémoire lue au hasard",
                OFFSET_ORIENTATION + 1
            ),
            Err(screen::StateError::TooShort { len }) => assert_eq!(
                len, longueur,
                "l'erreur rapporte une longueur de {len} au lieu de {longueur} : c'est la \
                 taille réellement reçue qui permet de diagnostiquer une lecture partielle"
            ),
            Err(screen::StateError::NotAState { .. }) => panic!(
                "une trame de {longueur} octets, préfixe de la réponse `31 01` de la capture, \
                 a été rejetée comme n'étant pas un état : elle porte bien l'identifiant \
                 attendu, elle est seulement trop courte. Confondre les deux causes envoie \
                 chercher la panne au mauvais endroit"
            ),
        }
    }
}

#[test]
fn une_reponse_qui_n_est_pas_un_etat_ecran_est_rejetee_en_nommant_ses_deux_premiers_octets() {
    // Même longueur que la trame de la capture, mais un autre identifiant : le contrôleur
    // émet plusieurs réponses sur l'endpoint 0x81, il faut savoir les distinguer.
    let mut autre = TRAME_ETAT_CAPTURE;
    autre[0] = 0x38;
    autre[1] = 0x01;

    match screen::parse_state(&autre) {
        Ok(_) => panic!(
            "une trame commençant par `38 01` a été décodée comme un état d'écran : les \
             réponses de l'endpoint 0x81 ne portent pas toutes la même structure, décoder la \
             mauvaise rendrait une résolution inventée"
        ),
        Err(screen::StateError::NotAState { first, second }) => {
            assert_eq!(
                (first, second),
                (0x38, 0x01),
                "l'erreur rapporte ({first:#04x}, {second:#04x}) au lieu de (0x38, 0x01) : ce \
                 sont les deux octets réellement reçus qui disent quelle réponse est arrivée \
                 à la place de `31 01`"
            );
        }
        Err(screen::StateError::TooShort { len }) => panic!(
            "une trame de {len} octets — la longueur même de la réponse de la capture — a été \
             jugée trop courte : le décodeur doit accepter cette taille et ne rejeter que \
             l'identifiant"
        ),
    }
}

// ---------------------------------------------------------------------------
// 8 — la réémission passe avant le repli firmware
// ---------------------------------------------------------------------------

#[test]
fn la_reemission_arrive_avant_le_repli_firmware_de_trente_secondes() {
    let repli = screen::FIRMWARE_FALLBACK_SECS;
    let reemission = screen::REFRESH_INTERVAL_SECS;

    assert_eq!(
        repli, 30,
        "FIRMWARE_FALLBACK_SECS vaut {repli} au lieu de 30 : le délai a été mesuré au \
         chronomètre (spec §2.2.2, ✅) — une fois les envois arrêtés, l'image reste environ \
         30 s puis le firmware réaffiche « NZXT — xx° Liquid »"
    );
    assert!(
        reemission > 0,
        "REFRESH_INTERVAL_SECS vaut 0 : une réémission sans attente sature l'USB pour rien, \
         alors que le contrôleur tient l'image ~30 s tout seul"
    );
    assert!(
        reemission < repli,
        "REFRESH_INTERVAL_SECS ({reemission} s) doit être strictement inférieur à \
         FIRMWARE_FALLBACK_SECS ({repli} s). À égalité ou au-delà, l'image disparaît par \
         intermittence au profit de l'affichage firmware : la panne est intermittente, donc \
         pénible à diagnostiquer"
    );
}

// ---------------------------------------------------------------------------
// 9 — ordre des composantes : le code l'annonce, l'encodage le respecte
// ---------------------------------------------------------------------------

#[test]
fn le_pixel_encode_suit_l_ordre_des_composantes_annonce_par_le_code() {
    // La spec conclut BGR (§2.1) par le raisonnement de la jauge olive, mais §2.2.1 précise
    // que la dérive de l'image a empêché de le vérifier sur une mire — c'est la question
    // ouverte n°2. Ce test ne fige donc AUCUN ordre : il vérifie que l'encodage d'un pixel
    // est cohérent avec l'ordre que le code déclare. Si la mire renverse la conclusion, on
    // change la constante et ce test continue de passer — ce qu'il interdit, c'est qu'une
    // constante dise BGR pendant que l'encodage fait autre chose. Le projet mélange trois
    // ordres (ventilateurs GRB, écran BGR, RAM RGB) et une erreur ici ne produit aucun
    // message : juste une mauvaise couleur.
    //
    // Surface attendue, absente du contrat d'API de l'issue et déduite de son test n°9 :
    //   pub const COMPONENT_ORDER: [usize; 3];   positions de (rouge, vert, bleu) dans le pixel
    //   pub fn pixel(r: u8, g: u8, b: u8) -> [u8; 3];   encode un pixel dans cet ordre
    // En BGR, COMPONENT_ORDER vaut donc [2, 1, 0].
    let ordre = screen::COMPONENT_ORDER;

    let mut vues = [false; 3];
    for (composante, position) in ordre.iter().enumerate() {
        let position = *position;
        assert!(
            position < 3,
            "COMPONENT_ORDER[{composante}] vaut {position} : un pixel de l'écran fait 3 \
             octets, les positions valides sont 0, 1 et 2"
        );
        assert!(
            !vues[position],
            "COMPONENT_ORDER place deux composantes à la position {position} : {ordre:?} n'est \
             pas une permutation de (rouge, vert, bleu), une composante serait perdue"
        );
        vues[position] = true;
    }

    // Couleur repère : trois valeurs distinctes, pour qu'aucune permutation ne passe inaperçue.
    let (rouge, vert, bleu) = (0x11_u8, 0x22_u8, 0x33_u8);
    let pixel = screen::pixel(rouge, vert, bleu);

    assert_eq!(
        pixel.len(),
        3,
        "un pixel encodé fait {} octets au lieu de 3 : c'est exactement l'erreur de \
         liquidctl 1.16 sur cette cible, qui produit 4 octets par pixel (R, G, B, 0) hérités \
         du Kraken Z3 quand la capture de CAM en montre 3",
        pixel.len()
    );

    for (composante, (attendu, nom)) in [(rouge, "rouge"), (vert, "vert"), (bleu, "bleu")]
        .into_iter()
        .enumerate()
    {
        let position = ordre[composante];
        let obtenu = octet(&pixel, position, nom);
        assert_eq!(
            obtenu, attendu,
            "le {nom} ({attendu:#04x}) devrait se trouver à la position {position} du pixel, \
             comme l'annonce COMPONENT_ORDER = {ordre:?} ; on y lit {obtenu:#04x}. Le code se \
             contredit lui-même : soit la constante ment, soit l'encodage ne la suit pas — et \
             une inversion de composantes ne remonte aucune erreur, elle change juste la \
             couleur affichée"
        );
    }
}

// ---------------------------------------------------------------------------
// La mire — l'artefact qui doit trancher entre RGB et BGR (question ouverte n°2)
// ---------------------------------------------------------------------------

/// Nombre de quadrants de la mire.
const QUADRANTS: usize = 4;

/// Quadrant qui contient le point (x, y).
///
/// Découpage déclaré dans le contrat : quatre quadrants de taille égale, coupés à `WIDTH / 2`
/// et `HEIGHT / 2`, dans l'ordre haut-gauche, haut-droite, bas-gauche, bas-droite. Les deux
/// dimensions valant 640, le partage est exact : colonnes 0 à 319 à gauche, 320 à 639 à droite.
fn quadrant_de(x: usize, y: usize) -> usize {
    let colonne = usize::from(x >= usize::from(screen::WIDTH) / 2);
    let ligne = usize::from(y >= usize::from(screen::HEIGHT) / 2);
    ligne * 2 + colonne
}

/// Pixel (x, y) d'une image brute balayée ligne par ligne, de haut en bas et de gauche à
/// droite — l'ordre naturel d'un `rawvideo`, celui que produit `ffmpeg`.
fn pixel_de(image: &[u8], x: usize, y: usize) -> [u8; 3] {
    let debut = (y * usize::from(screen::WIDTH) + x) * 3;
    [
        octet(image, debut, "première composante du pixel"),
        octet(image, debut + 1, "deuxième composante du pixel"),
        octet(image, debut + 2, "troisième composante du pixel"),
    ]
}

/// Les quatre couleurs de la mire, déjà encodées dans l'ordre que le code déclare.
///
/// Passer par `pixel()` est délibéré : la mire doit rester cohérente avec `COMPONENT_ORDER`
/// **sans que ce test ne recopie l'ordre**. Si la mire matérielle renverse la conclusion BGR,
/// la constante change, la mire change avec elle, et ces tests continuent de passer.
fn couleurs_encodees() -> [[u8; 3]; QUADRANTS] {
    let mut couleurs = [[0_u8; 3]; QUADRANTS];
    for (encodee, &(r, v, b)) in couleurs
        .iter_mut()
        .zip(screen::TEST_PATTERN_QUADRANTS.iter())
    {
        *encodee = screen::pixel(r, v, b);
    }
    couleurs
}

/// Nom lisible d'un quadrant, pour les messages d'échec.
fn nom_quadrant(index: usize) -> &'static str {
    match index {
        0 => "haut-gauche",
        1 => "haut-droite",
        2 => "bas-gauche",
        3 => "bas-droite",
        _ => "quadrant inconnu",
    }
}

#[test]
fn la_mire_fait_exactement_la_taille_d_une_image_et_passe_la_verification() {
    let mire = screen::test_pattern();

    assert_eq!(
        mire.len(),
        screen::IMAGE_LEN,
        "la mire fait {} octets au lieu de {} : elle est envoyée au contrôleur par le même \
         chemin qu'une image de l'utilisateur, elle doit donc satisfaire la même contrainte \
         de taille (640 × 640 × 3)",
        mire.len(),
        screen::IMAGE_LEN
    );
    assert!(
        screen::check_image(&mire).is_ok(),
        "la mire engendrée par Reverb est refusée par sa propre vérification de taille : \
         l'outil censé trancher l'ordre des composantes ne pourrait même pas être affiché"
    );
}

#[test]
fn le_centre_de_chaque_quadrant_de_la_mire_porte_la_couleur_declaree() {
    let mire = screen::test_pattern();
    let attendues = couleurs_encodees();
    let demi_largeur = usize::from(screen::WIDTH) / 2;
    let demi_hauteur = usize::from(screen::HEIGHT) / 2;

    let declarees = screen::TEST_PATTERN_QUADRANTS;
    for (index, (attendu, &(r, v, b))) in attendues.iter().zip(declarees.iter()).enumerate() {
        // Centre du quadrant : loin de toutes les frontières, donc insensible à un
        // éventuel désaccord d'un pixel sur le découpage.
        let x = (index % 2) * demi_largeur + demi_largeur / 2;
        let y = (index / 2) * demi_hauteur + demi_hauteur / 2;

        let obtenu = pixel_de(&mire, x, y);
        let attendu = *attendu;
        assert_eq!(
            obtenu,
            attendu,
            "au centre du quadrant {} ({}, {}), la mire porte {obtenu:02x?} au lieu de \
             {attendu:02x?}. La couleur déclarée pour ce quadrant est (r={r}, v={v}, b={b}) \
             et son encodage est celui de pixel(), donc de COMPONENT_ORDER = {:?}. C'est cette \
             mire qui doit trancher la question ouverte n°2 de la spec : si elle n'affiche pas \
             ce qu'elle annonce, elle ne tranche rien",
            nom_quadrant(index),
            x,
            y,
            screen::COMPONENT_ORDER
        );
    }
}

#[test]
fn chaque_pixel_de_la_mire_porte_la_couleur_du_quadrant_qui_le_contient() {
    // Le test précédent dit que la palette est juste ; celui-ci dit que la géométrie l'est.
    // Séparés à dessein : un échec ici seul désigne le découpage ou le sens de balayage
    // (image à l'envers, décalage d'une ligne, tampon rempli à moitié), pas les couleurs.
    let mire = screen::test_pattern();
    let attendues = couleurs_encodees();
    let largeur = usize::from(screen::WIDTH);
    let hauteur = usize::from(screen::HEIGHT);

    for y in 0..hauteur {
        for x in 0..largeur {
            let index = quadrant_de(x, y);
            let attendu = attendues[index];
            let obtenu = pixel_de(&mire, x, y);
            assert!(
                obtenu == attendu,
                "pixel ({x}, {y}) : la mire porte {obtenu:02x?} alors que ce point tombe dans \
                 le quadrant {} ({attendu:02x?}). Les quadrants sont de taille égale, coupés à \
                 WIDTH / 2 = {} et HEIGHT / 2 = {}, en balayage ligne par ligne de haut en bas \
                 et de gauche à droite. Un quadrant mal découpé se lit mal, et une mire qui se \
                 lit mal ne tranche pas l'ordre des composantes — elle l'embrouille",
                nom_quadrant(index),
                largeur / 2,
                hauteur / 2
            );
        }
    }
}

#[test]
fn les_quatre_couleurs_de_la_mire_sont_distinctes_deux_a_deux() {
    let quadrants = screen::TEST_PATTERN_QUADRANTS;

    for (i, premiere) in quadrants.iter().enumerate() {
        for (decalage, seconde) in quadrants.iter().skip(i + 1).enumerate() {
            assert_ne!(
                premiere,
                seconde,
                "les quadrants {} et {} portent la même couleur {premiere:?} : deux quadrants \
                 qu'on ne distingue pas ne donnent aucune information, et la mire est justement \
                 là pour qu'on lise une réponse à l'œil",
                nom_quadrant(i),
                nom_quadrant(i + 1 + decalage)
            );
        }
    }
}

#[test]
fn l_inversion_rouge_bleu_echange_deux_quadrants_et_laisse_les_deux_autres_intacts() {
    // C'est la propriété qui rend la mire discriminante, et rien d'autre : si l'ordre des
    // composantes est renversé, rouge et bleu s'échangent. Deux quadrants doivent donc
    // permuter — l'erreur se lit comme deux couleurs qui ont changé de coin — et deux doivent
    // rester en place, sans quoi on ne saurait pas distinguer un renversement de l'ordre
    // d'une mire simplement fausse. On ne fige ici ni les couleurs ni les positions : on
    // vérifie que la palette a bien cette symétrie.
    let quadrants = screen::TEST_PATTERN_QUADRANTS;
    let echange = |(r, v, b): (u8, u8, u8)| (b, v, r);

    let mut deplaces = 0;
    for (index, &couleur) in quadrants.iter().enumerate() {
        let echangee = echange(couleur);
        if echangee == couleur {
            continue;
        }
        deplaces += 1;
        assert!(
            quadrants.contains(&echangee),
            "le quadrant {} porte {couleur:?} ; rouge et bleu inversés, il afficherait \
             {echangee:?}, qui n'est aucune des quatre couleurs de la mire. On verrait donc \
             une couleur inattendue au lieu de deux quadrants qui ont changé de place — plus \
             difficile à lire, et impossible à confondre avec la réponse attendue",
            nom_quadrant(index)
        );
    }

    assert_eq!(
        deplaces, 2,
        "{deplaces} quadrants sur {QUADRANTS} changent de couleur quand on inverse rouge et \
         bleu, il en faut exactement deux : deux qui permutent pour rendre l'inversion \
         visible, et deux insensibles (rouge = bleu) qui servent de témoins. Avec zéro \
         quadrant déplacé la mire ne tranche rien ; avec quatre, plus rien ne dit que c'est \
         bien un échange rouge/bleu qu'on regarde"
    );
}

// ---------------------------------------------------------------------------
// Trames de contrôle de la « recette minimale » du §0
// ---------------------------------------------------------------------------

#[test]
fn le_mode_de_diffusion_emet_38_01_02_00_sans_quoi_l_image_est_ignoree_en_silence() {
    // Spec §2.2.1 et §3.5, ✅ : sans cette trame l'écran reste sur son affichage intégré et
    // ignore l'image. L'envoi réussit, aucun code d'erreur ne remonte, rien n'apparaît. C'est
    // LE piège de cette cible, et il ne ressemble pas à une panne — d'où ce test.
    //
    // La trame fait quatre octets significatifs, pas trois : `38 01 <mode> <bucket>` (§3.5,
    // recoupé avec `_switch_bucket` de liquidctl). CAM émet le mode 2, bucket 0.
    verifie_trame(
        &screen::broadcast_mode(),
        &[0x38, 0x01, 0x02, 0x00],
        "trame de mode de diffusion",
        "la capture USB de CAM, spec §2.2.1, §3.1 et §3.5",
    );
}

// ---------------------------------------------------------------------------
// Critère d'acceptation NON TESTÉ ici : « aucune fonction publique n'encode un retour au
// mode firmware, tant qu'aucune trame ne l'a établi » (issue #13, amendement du contrat).
//
// C'est une propriété d'**absence** : elle porte sur ce que le module ne contient pas.
// Rien en Rust ne permet de l'écrire en test — un module ne s'énumère pas à l'exécution, et
// nommer une fonction qui ne doit pas exister est une erreur de compilation, pas un échec de
// test. Le vérifier demanderait `trybuild` ou une inspection de `cargo doc`, c'est-à-dire une
// dépendance externe que ce projet s'interdit. Une approximation (« le module n'expose que
// ces N fonctions ») ne testerait rien : elle se contenterait de recopier une liste.
//
// Ce critère se vérifie donc **en relecture de la revue**, et pas ici. Ce qui reste testé, et
// qui en est le pendant positif : la seule trame `38 01` du module est celle observée dans la
// capture, verrouillée octet par octet ci-dessus. Une trame de retour firmware inventée ne
// pourrait pas s'y cacher.
//
// Spec §2.3 (corrigée le 2026-07-31) et question ouverte n°6 : aucune commande connue ne
// ramène l'écran à son affichage firmware. Il y retombe seul au bout des ~30 s du §2.2.2 ;
// cesser d'émettre suffit, et c'est le seul mécanisme observé.
// ---------------------------------------------------------------------------

#[test]
fn l_annonce_et_la_validation_encadrent_le_transfert_de_l_image() {
    // Séquence relevée puis rejouée avec succès (spec §2.2.1, ✅) :
    //   HID  36 01 00 01 09   annonce
    //   BULK en-tête + pixels
    //   HID  36 02            validation
    verifie_trame(
        &screen::begin_image(),
        &[0x36, 0x01, 0x00, 0x01, 0x09],
        "annonce d'image",
        "la capture USB de CAM, spec §2.2.1",
    );
    verifie_trame(
        &screen::end_image(),
        &[0x36, 0x02],
        "validation d'image",
        "la capture USB de CAM, spec §2.2.1",
    );
}

#[test]
fn la_demande_d_etat_emet_30_01_et_n_ecrit_aucun_reglage() {
    // `reverb screen` sans argument doit lire, pas écrire : la seule trame émise est la
    // demande d'état `30 01`, sans paramètre, à laquelle le contrôleur répond `31 01`
    // (spec §3.1 et §3.7).
    verifie_trame(
        &screen::query_state(),
        &[0x30, 0x01],
        "demande d'état",
        "la capture USB de CAM, spec §3.1 et §3.7",
    );
}
