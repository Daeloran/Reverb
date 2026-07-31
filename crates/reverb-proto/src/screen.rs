//! Écran LCD du Kraken Elite 2023 (`1e71:300c`).
//!
//! Toutes les valeurs viennent de `docs/SPEC-KRAKEN-LCD.md`, elle-même issue
//! d'une capture de NZXT CAM. **Ne rien inventer** : une trame absente de la
//! spec est inconnue, et le §2.3 rappelle ce que ça coûte — une version
//! antérieure y devinait une commande de retour au mode firmware qui n'existe
//! pas.
//!
//! Deux pièges portés par ce module :
//!
//! - sans [`broadcast_mode`], l'image envoyée est **ignorée en silence**.
//!   L'envoi réussit, aucun code d'erreur, rien n'apparaît (spec §2.2.1) ;
//! - l'écran retombe sur son affichage firmware au bout d'une trentaine de
//!   secondes. Un affichage durable impose de réémettre — voir
//!   [`REFRESH_INTERVAL_SECS`].
//!
//! ⚠️ L'ordre des composantes n'est pas celui des ventilateurs. Les LED NZXT
//! sont en GRB, cet écran en [`COMPONENT_ORDER`], la RAM Corsair en RGB.

use crate::frame::{FRAME_LEN, Frame, packet};

/// Largeur de l'écran, annoncée par le contrôleur lui-même (spec §3.7).
pub const WIDTH: u16 = 640;

/// Hauteur de l'écran, annoncée par le contrôleur lui-même (spec §3.7).
pub const HEIGHT: u16 = 640;

/// Octets par pixel. **Trois**, et c'est ce qui distingue ce modèle du
/// Kraken Z3 : `liquidctl` en envoie quatre (`R, G, B, 0`) et se déclare
/// lui-même `(broken)` pour le `1e71:300c`.
pub const PIXEL_LEN: usize = 3;

/// Taille exacte d'une image, en octets : 640 × 640 × 3 (spec §2).
pub const IMAGE_LEN: usize = WIDTH as usize * HEIGHT as usize * PIXEL_LEN;

/// Position du rouge, du vert et du bleu dans le triplet écrit à l'écran.
///
/// `[2, 1, 0]` — l'écran est en **BGR** (spec §2.1). Conclusion établie par le
/// raisonnement de la jauge olive, pas par une mire : le §2.2.1 précise qu'elle
/// n'a pas pu être vérifiée directement, la dérive de l'image déplaçant les
/// quadrants d'un envoi à l'autre.
///
/// Si la mire renverse cette conclusion, **c'est cette constante qui change**,
/// et rien d'autre : c'est le seul endroit du code qui connaît l'ordre.
pub const COMPONENT_ORDER: [usize; 3] = [2, 1, 0];

/// Longueur de l'en-tête d'un transfert bulk (spec §2).
pub const BULK_HEADER_LEN: usize = 20;

/// Signature magique de l'en-tête bulk, invariante sur les 50 images de la
/// capture (spec §2).
const BULK_SIGNATURE: [u8; 8] = [0xab, 0xcd, 0xef, 0x98, 0x76, 0x54, 0x32, 0x10];

/// Quatre premiers octets de l'en-tête bulk. Constants dans toutes les
/// observations, rôle inconnu (spec §2, question ouverte n° 3).
const BULK_PREFIX: [u8; 4] = [0x12, 0xfa, 0x01, 0xe8];

/// Sélecteur de contenu, offsets 12 à 15 de l'en-tête bulk.
///
/// `liquidctl` place à cet endroit `0x01` pour un GIF et `0x02` pour une image
/// fixe ; CAM y met `0x09`. On reproduit CAM, seule valeur observée sur ce
/// modèle (spec §2, question ouverte n° 3).
const BULK_CONTENT: [u8; 4] = [0x09, 0x00, 0x00, 0x00];

/// Repli du firmware, mesuré à une trentaine de secondes (spec §2.2.2).
///
/// Passé ce délai sans nouvel envoi, l'écran réaffiche « NZXT — xx° Liquid ».
pub const FIRMWARE_FALLBACK_SECS: u64 = 30;

/// Intervalle de réémission d'une image, strictement inférieur au repli.
pub const REFRESH_INTERVAL_SECS: u64 = 25;

/// Mode de diffusion, seule valeur observée sur ce modèle (spec §3.5).
const BROADCAST: u8 = 0x02;

/// Emplacement de stockage visé. CAM emploie toujours le premier et ne se sert
/// jamais des seize emplacements énumérés à l'init (spec §3.6).
const BUCKET: u8 = 0x00;

/// Octet de fin de la trame de luminosité.
///
/// 🔶 Vaut 30, exactement le délai de repli du §2.2.2 — l'hypothèse qu'il porte
/// ce délai en secondes est cohérente mais **non vérifiée**. Reproduit tel quel,
/// sans lui prêter de sens (spec §3.4).
const BRIGHTNESS_TRAILER: u8 = 0x1e;

/// Sélectionne le mode de diffusion — `38 01 02 00` (spec §3.5).
///
/// **Indispensable avant tout envoi d'image.** Sans cette trame, l'écran reste
/// sur son affichage intégré et ignore l'image en silence (spec §2.2.1).
///
/// Il n'existe pas de fonction inverse : aucune trame connue ne ramène l'écran
/// à son affichage firmware. Il y retombe seul au bout de
/// [`FIRMWARE_FALLBACK_SECS`] — cesser d'émettre suffit, et c'est le seul
/// mécanisme observé (spec §2.3).
pub fn broadcast_mode() -> Frame {
    packet(&[0x38, 0x01, BROADCAST, BUCKET])
}

/// Demande l'état de l'écran — `30 01` (spec §3.7).
///
/// La réponse, à lire sur l'endpoint entrant, se décode par [`parse_state`].
/// Cette trame ne modifie aucun réglage.
pub fn query_state() -> Frame {
    packet(&[0x30, 0x01])
}

/// Règle la luminosité — `30 02 01 <pourcent> 00 00 00 00 1e` (spec §3.4).
///
/// ⚠️ **À émettre avant l'image, jamais après** : un changement de luminosité
/// provoque un bref retour à l'affichage intégré, la commande réinitialisant le
/// pipeline d'affichage.
///
/// # Erreurs
///
/// [`BrightnessError::OutOfRange`] au-delà de 100. Zéro est accepté : éteindre
/// l'écran est un réglage légitime.
pub fn set_brightness(percent: u8) -> Result<Frame, BrightnessError> {
    if percent > 100 {
        return Err(BrightnessError::OutOfRange { given: percent });
    }
    Ok(packet(&[
        0x30,
        0x02,
        0x01,
        percent,
        0x00,
        0x00,
        0x00,
        0x00,
        BRIGHTNESS_TRAILER,
    ]))
}

/// Annonce l'envoi d'une image — `36 01 00 01 09` (spec §3.2).
///
/// Invariante d'une image à l'autre : la capture la montre à l'identique sur
/// les cinquante images, sans jamais rien intercaler (spec §3.6).
pub fn begin_image() -> Frame {
    packet(&[0x36, 0x01, 0x00, 0x01, 0x09])
}

/// Valide l'envoi d'une image — `36 02` (spec §3.2).
pub fn end_image() -> Frame {
    packet(&[0x36, 0x02])
}

/// En-tête d'un transfert bulk, pour une charge utile de `len` octets (spec §2).
///
/// ```text
/// offset  0..3    12 fa 01 e8              constant, role inconnu
/// offset  4..11   ab cd ef 98 76 54 32 10  signature invariante
/// offset 12..15   09 00 00 00              selecteur de contenu
/// offset 16..19   <len>                    petit-boutiste
/// ```
pub fn bulk_header(len: u32) -> [u8; BULK_HEADER_LEN] {
    let mut header = [0u8; BULK_HEADER_LEN];
    header[0..4].copy_from_slice(&BULK_PREFIX);
    header[4..12].copy_from_slice(&BULK_SIGNATURE);
    header[12..16].copy_from_slice(&BULK_CONTENT);
    header[16..20].copy_from_slice(&len.to_le_bytes());
    header
}

/// Compose un pixel dans l'ordre qu'attend l'écran.
///
/// Sert à générer les images que Reverb produit lui-même — la mire de
/// vérification, notamment. Une image fournie par l'utilisateur arrive déjà au
/// bon format et ne passe pas par ici.
pub fn pixel(r: u8, g: u8, b: u8) -> [u8; PIXEL_LEN] {
    let composantes = [r, g, b];
    let mut sortie = [0u8; PIXEL_LEN];
    for (composante, &position) in composantes.iter().zip(COMPONENT_ORDER.iter()) {
        sortie[position] = *composante;
    }
    sortie
}

/// Couleurs de la mire, dans l'ordre : haut-gauche, haut-droite, bas-gauche,
/// bas-droite.
///
/// Choisies pour **trancher** entre RGB et BGR d'un seul coup d'œil : une
/// inversion échange le rouge et le bleu, laisse le vert en place et ne touche
/// pas au blanc. Si le quadrant haut-gauche apparaît bleu, [`COMPONENT_ORDER`]
/// est faux.
pub const TEST_PATTERN_QUADRANTS: [(u8, u8, u8); 4] = [
    (255, 0, 0),     // haut-gauche  — rouge
    (0, 255, 0),     // haut-droite  — vert
    (0, 0, 255),     // bas-gauche   — bleu
    (255, 255, 255), // bas-droite   — blanc
];

/// Engendre la mire de vérification : quatre quadrants de couleurs connues.
///
/// Sert à répondre à la question ouverte n° 2 de la spec — l'ordre des
/// composantes, conclu par raisonnement au §2.1 mais jamais vérifié
/// directement, la dérive du §2.2.1 ayant déplacé les quadrants d'un envoi à
/// l'autre.
///
/// L'image fait exactement [`IMAGE_LEN`] octets et se compose par [`pixel`] :
/// elle suit donc [`COMPONENT_ORDER`] par construction, et non par recopie.
pub fn test_pattern() -> Vec<u8> {
    let mut image = Vec::with_capacity(IMAGE_LEN);
    let milieu_x = WIDTH as usize / 2;
    let milieu_y = HEIGHT as usize / 2;

    for y in 0..HEIGHT as usize {
        for x in 0..WIDTH as usize {
            let quadrant = usize::from(y >= milieu_y) * 2 + usize::from(x >= milieu_x);
            let (r, g, b) = TEST_PATTERN_QUADRANTS[quadrant];
            image.extend_from_slice(&pixel(r, g, b));
        }
    }
    image
}

/// Vérifie qu'une image a exactement la taille attendue.
///
/// À appeler **avant** d'ouvrir le moindre périphérique : une image de mauvaise
/// taille est une erreur de l'appelant, pas une panne du matériel. Même
/// exigence que `check_colors` du module `mode`.
pub fn check_image(data: &[u8]) -> Result<(), ImageError> {
    if data.len() != IMAGE_LEN {
        return Err(ImageError::WrongLength {
            given: data.len(),
            expected: IMAGE_LEN,
        });
    }
    Ok(())
}

/// État de l'écran, tel que le contrôleur le déclare (spec §3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenState {
    /// Largeur en pixels, annoncée par le matériel.
    pub width: u16,
    /// Hauteur en pixels, annoncée par le matériel.
    pub height: u16,
    /// Luminosité courante, en pourcent.
    pub brightness: u8,
    /// Orientation courante. ❓ L'échelle n'est pas établie : la lecture est
    /// documentée, l'écriture non (spec, question ouverte n° 4).
    pub orientation: u8,
}

/// Offsets de la réponse `31 01`, en petit-boutiste (spec §3.7).
const STATE_WIDTH: usize = 0x14;
const STATE_HEIGHT: usize = 0x16;
const STATE_BRIGHTNESS: usize = 0x18;
const STATE_ORIENTATION: usize = 0x1a;

/// Longueur minimale exploitable d'une réponse `31 01` : le dernier champ utile
/// est l'orientation, à l'offset `0x1a`.
const STATE_MIN_LEN: usize = STATE_ORIENTATION + 1;

/// Décode une réponse `31 01` (spec §3.7).
///
/// # Erreurs
///
/// [`StateError::NotAState`] si les deux premiers octets ne sont pas `31 01` —
/// le contrôleur émet spontanément des trames d'état `75 02` chaque seconde, et
/// il faut savoir les écarter. [`StateError::TooShort`] si la trame s'arrête
/// avant l'orientation.
pub fn parse_state(frame: &[u8]) -> Result<ScreenState, StateError> {
    if frame.len() < 2 {
        return Err(StateError::TooShort { len: frame.len() });
    }
    if frame[0] != 0x31 || frame[1] != 0x01 {
        return Err(StateError::NotAState {
            first: frame[0],
            second: frame[1],
        });
    }
    if frame.len() < STATE_MIN_LEN {
        return Err(StateError::TooShort { len: frame.len() });
    }
    Ok(ScreenState {
        width: u16::from_le_bytes([frame[STATE_WIDTH], frame[STATE_WIDTH + 1]]),
        height: u16::from_le_bytes([frame[STATE_HEIGHT], frame[STATE_HEIGHT + 1]]),
        brightness: frame[STATE_BRIGHTNESS],
        orientation: frame[STATE_ORIENTATION],
    })
}

/// Luminosité hors des bornes acceptées.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrightnessError {
    /// Au-delà de 100 %.
    OutOfRange { given: u8 },
}

impl std::fmt::Display for BrightnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrightnessError::OutOfRange { given } => {
                write!(f, "luminosité {given} hors des bornes : attendu 0 à 100")
            }
        }
    }
}

impl std::error::Error for BrightnessError {}

/// Image de taille inattendue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageError {
    /// La taille ne correspond pas à une image 640 × 640 en trois octets par
    /// pixel.
    WrongLength { given: usize, expected: usize },
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageError::WrongLength { given, expected } => write!(
                f,
                "image de {given} octets, attendu {expected} ({WIDTH} × {HEIGHT} × {PIXEL_LEN})"
            ),
        }
    }
}

impl std::error::Error for ImageError {}

/// Réponse d'état illisible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateError {
    /// Trop courte pour porter tous les champs.
    TooShort { len: usize },
    /// Ce n'est pas une réponse `31 01`.
    NotAState { first: u8, second: u8 },
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::TooShort { len } => write!(
                f,
                "réponse de {len} octets, il en faut au moins {STATE_MIN_LEN}"
            ),
            StateError::NotAState { first, second } => {
                write!(f, "réponse {first:#04x} {second:#04x}, attendu 0x31 0x01")
            }
        }
    }
}

impl std::error::Error for StateError {}

/// Une trame de contrôle fait toujours 64 octets, complétée par des zéros.
const _: () = assert!(FRAME_LEN == 64);
