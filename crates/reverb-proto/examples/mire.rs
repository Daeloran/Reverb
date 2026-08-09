//! Rend une mire dans un fichier, **sans matériel** (issue #77).
//!
//! ```bash
//! cargo run --release --example mire -p reverb-proto -- /tmp/mire.ppm cercle
//! ```
//!
//! ⚠️ **Une mire se regarde avant de se brancher.** Celle du disque sert à une
//! mesure faite à l'œil sur un boîtier, le démon arrêté : s'apercevoir devant la
//! machine que ses anneaux sont indiscernables coûterait un redémarrage de
//! service pour rien.

use std::io::Write;

use reverb_proto::screen;

fn main() -> std::io::Result<()> {
    let mut arguments = std::env::args().skip(1);
    let chemin = arguments
        .next()
        .unwrap_or_else(|| "/tmp/mire.ppm".to_owned());
    let laquelle = arguments.next().unwrap_or_else(|| "cercle".to_owned());

    let image = match laquelle.as_str() {
        "cercle" => screen::mire_cercle(),
        "quadrants" => screen::test_pattern(),
        autre => {
            eprintln!("mire inconnue : « {autre} » — les deux sont « cercle » et « quadrants »");
            std::process::exit(2);
        }
    };

    // Le tampon est en BGR, l'ordre de la dalle ; un PPM est en RGB. La
    // conversion passe par `composantes`, la seule fonction qui connaisse cet
    // ordre — la recopier ici rendrait une image juste et fausse à la fois.
    let mut fichier = std::fs::File::create(&chemin)?;
    write!(fichier, "P6\n{} {}\n255\n", screen::WIDTH, screen::HEIGHT)?;
    for pixel in image.chunks_exact(screen::PIXEL_LEN) {
        let (r, g, b) = screen::composantes(pixel);
        fichier.write_all(&[r, g, b])?;
    }

    println!("{chemin}");
    Ok(())
}
