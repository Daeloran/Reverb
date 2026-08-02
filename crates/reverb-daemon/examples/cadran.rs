//! Dessine un cadran dans un fichier, sans écran ni matériel.
//!
//! ```bash
//! cargo run --release --example cadran -p reverb-daemon -- /tmp/cadran.ppm 34.2 0.34
//! ```
//!
//! C'est ainsi qu'on regarde le cadran sans brancher de Kraken, et la seule
//! façon de vérifier « lisible à un mètre » ailleurs qu'à l'œil sur la dalle.
//!
//! Le format est du **PPM binaire** (P6), que tout visualiseur ouvre et que
//! `magick cadran.ppm cadran.png` convertit — écrire un PNG demanderait un
//! encodeur de plus dans un binaire qui n'en veut pas.

use std::io::Write;

use reverb_daemon::ecran::Dalle;
use reverb_proto::screen;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let sortie = arguments
        .next()
        .unwrap_or_else(|| "/tmp/cadran.ppm".to_owned());
    // Sans valeur, c'est le cadran d'une sonde muette — celui qu'il faut
    // regarder de près, parce qu'il ne doit surtout pas ressembler à zéro.
    let valeur = arguments.next().and_then(|brut| brut.parse::<f32>().ok());
    let proportion = arguments
        .next()
        .and_then(|brut| brut.parse::<f32>().ok())
        .unwrap_or(0.5);

    let dalle = Dalle::cadran("kraken2023elite:coolant", valeur, "°C", proportion);

    let mut ppm = format!("P6\n{} {}\n255\n", screen::WIDTH, screen::HEIGHT).into_bytes();
    for pixel in dalle.octets().chunks_exact(screen::PIXEL_LEN) {
        // Le PPM est en RGB, la dalle dans l'ordre de l'écran : la conversion
        // passe par `screen::COMPONENT_ORDER`, jamais par un ordre recopié.
        for position in screen::COMPONENT_ORDER {
            ppm.push(pixel[position]);
        }
    }
    let mut fichier = std::fs::File::create(&sortie).expect("création du fichier");
    fichier.write_all(&ppm).expect("écriture du fichier");
    println!("{sortie}");
}
