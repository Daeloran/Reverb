//! Les animations calculées par l'hôte.
//!
//! **Aucune n'est un relevé du matériel.** Elles sont de nous, et c'est
//! pourquoi elles vivent ici et non dans `reverb-proto`, dont la règle est de
//! ne rien inventer. Seule leur *nécessité* est observée : le contrôleur de la
//! RAM ne sait pas animer seul (SPEC-CORSAIR-RAM §4.5), et rien ne permet de
//! synchroniser une animation firmware des ventilateurs avec quoi que ce soit
//! d'autre.
//!
//! Ce chantier n'en livre qu'une. Le catalogue appartient à l'issue suivante.

use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

/// Nom de la seule animation livrée par ce chantier.
pub const VAGUE: &str = "vague";

/// Les animations que le démon sait jouer.
pub const CATALOGUE: [&str; 1] = [VAGUE];

/// Ordre dans lequel la vague traverse le boîtier.
///
/// **C'est notre choix, pas une contrainte du matériel.** Le tour part du bas,
/// remonte la face avant (le radiateur du Kraken), franchit le dessus de
/// gauche à droite et finit à l'arrière — de sorte qu'une vague se lise comme
/// un mouvement continu plutôt que comme un ordre de numérotation.
pub const CHEMIN: [Position; 10] = [
    Position::BasGauche,
    Position::BasMilieu,
    Position::BasDroite,
    Position::RadiateurBas,
    Position::RadiateurMilieu,
    Position::RadiateurHaut,
    Position::HautGauche,
    Position::HautMilieu,
    Position::HautDroite,
    Position::Arriere,
];

/// Nombre de LED que la vague parcourt : dix ventilateurs et quatre barrettes.
pub const LEDS_TOTAL: u32 = 10 * LEDS_PER_FAN as u32 + (SLOT_COUNT * LEDS_PER_STICK) as u32;

/// Longueur de la traînée de la comète, en LED.
const TRAINEE: u32 = 24;

/// Une image complète : les dix ventilateurs, puis les quatre barrettes.
pub struct Image {
    pub ventilateurs: [(Position, [Rgb; LEDS_PER_FAN as usize]); 10],
    pub barrettes: [[Rgb; LEDS_PER_STICK]; SLOT_COUNT],
}

/// Calcule l'image `pas` de l'animation nommée.
///
/// Rend `None` si le nom n'est pas au [`CATALOGUE`].
pub fn image(nom: &str, pas: u32) -> Option<Image> {
    (nom == VAGUE).then(|| vague(pas))
}

/// Une comète qui fait le tour du boîtier puis traverse les barrettes.
fn vague(pas: u32) -> Image {
    let tete = pas % LEDS_TOTAL;
    let couleur_de = |rang: u32| {
        // Recul derrière la tête, sur l'anneau complet.
        let recul = (LEDS_TOTAL + rang - tete) % LEDS_TOTAL;
        if recul >= TRAINEE {
            return Rgb::BLACK;
        }
        let intensite = (255 - recul * 255 / TRAINEE) as u8;
        Rgb::new(intensite, intensite / 3, intensite)
    };

    let mut ventilateurs = [(Position::BasGauche, [Rgb::BLACK; LEDS_PER_FAN as usize]); 10];
    for (rang_ventilateur, position) in CHEMIN.into_iter().enumerate() {
        let mut couleurs = [Rgb::BLACK; LEDS_PER_FAN as usize];
        for (led, couleur) in couleurs.iter_mut().enumerate() {
            let rang = (rang_ventilateur * LEDS_PER_FAN as usize + led) as u32;
            *couleur = couleur_de(rang);
        }
        ventilateurs[rang_ventilateur] = (position, couleurs);
    }

    let debut_ram = 10 * LEDS_PER_FAN as usize;
    let mut barrettes = [[Rgb::BLACK; LEDS_PER_STICK]; SLOT_COUNT];
    for (slot, couleurs) in barrettes.iter_mut().enumerate() {
        for (led, couleur) in couleurs.iter_mut().enumerate() {
            let rang = (debut_ram + slot * LEDS_PER_STICK + led) as u32;
            *couleur = couleur_de(rang);
        }
    }

    Image {
        ventilateurs,
        barrettes,
    }
}
