//! Combien de cibles changent d'une image à l'autre, par animation.
//!
//! Le démon ne réécrit que les cibles dont la couleur a changé (#17) : c'est ce
//! qui l'a fait passer de 21 à 31 images par seconde sur la comète, où cent LED
//! sur cent vingt-quatre sont noires et le restent. Ce compte dit donc quelle
//! cadence chaque animation peut espérer tenir, **avant** de la mesurer sur le
//! matériel.
//!
//! Coûts mesurés le 2026-07-31 (issue #17) : ~0,98 ms par ventilateur réécrit,
//! ~5,4 ms par barrette — dont ~3 ms de plancher physique sur le fil SMBus.
//!
//! `cargo run --release --example densite -p reverb-anim`

use reverb_anim::{Animation, CATALOGUE, Geometrie, Reglages};

/// Coût d'une réécriture, en millisecondes (mesures de l'issue #17).
///
/// Un ventilateur coûte **trois** trames HID indissociables (spec §0.2) à
/// ~0,98 ms chacune, pas une seule — c'est le total de 29,5 ms mesuré pour les
/// dix. Une barrette coûte deux blocs SMBus, dont ~3 ms de plancher physique
/// sur le fil à 100 kHz.
const COUT_VENTILATEUR: f64 = 2.95;
const COUT_BARRETTE: f64 = 5.4;

fn main() {
    let geometrie = Geometrie::mesuree();
    let reglages = Reglages::default();

    println!(
        "{:<14} {:>9} {:>10} {:>10} {:>9}",
        "animation", "ventilos", "barrettes", "image", "img/s"
    );
    for nom in CATALOGUE {
        let animation = Animation::par_nom(nom).expect("nom du catalogue");
        let (mut ventilateurs, mut barrettes) = (0u32, 0u32);
        let images = 120u32;

        for pas in 1..=images {
            let avant = animation.image(&geometrie, &reglages, pas - 1);
            let apres = animation.image(&geometrie, &reglages, pas);
            for (avant, apres) in avant.ventilateurs.iter().zip(&apres.ventilateurs) {
                if avant.1 != apres.1 {
                    ventilateurs += 1;
                }
            }
            for (avant, apres) in avant.barrettes.iter().zip(&apres.barrettes) {
                if avant != apres {
                    barrettes += 1;
                }
            }
        }

        let par_image = f64::from(images);
        let ventilateurs = f64::from(ventilateurs) / par_image;
        let barrettes = f64::from(barrettes) / par_image;
        let cout = ventilateurs * COUT_VENTILATEUR + barrettes * COUT_BARRETTE;
        println!(
            "{nom:<14} {ventilateurs:>9.1} {barrettes:>10.1} {cout:>7.1} ms {:>9.0}",
            if cout > 0.0 {
                1000.0 / cout
            } else {
                f64::INFINITY
            }
        );
    }
    println!("\n(cadence visée : 30 img/s, soit 33,3 ms par image)");
}
