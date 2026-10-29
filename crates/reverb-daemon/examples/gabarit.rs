//! Rend un gabarit SVG dans un fichier, **sans matériel** (issue #144).
//!
//! ```text
//! cargo run --release --example gabarit -p reverb-daemon -- ecran.svg /tmp/gabarit.ppm \
//!     k10temp:tctl=38 kraken2023elite:coolant-temp=31
//! ```
//!
//! Chaque argument `slug=valeur` donne la valeur d'une sonde ; une sonde absente
//! de la liste s'écrit en tirets, comme une sonde muette. Les polices du dossier
//! du gabarit sont chargées, exactement comme le démon les charge.
//!
//! Le format est du **PPM binaire** (P6), comme les exemples `cadran` et
//! `composition`, et sans le bord du disque : c'est l'image que la dalle reçoit,
//! octet pour octet, qu'on veut pouvoir comparer à une maquette.

use std::io::Write;
use std::path::Path;

use reverb_daemon::gabarit;
use reverb_proto::screen;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let source = arguments
        .next()
        .expect("usage : gabarit <ecran.svg> <sortie.ppm> [slug=valeur…]");
    let sortie = arguments
        .next()
        .unwrap_or_else(|| "/tmp/gabarit.ppm".to_owned());
    let valeurs: Vec<(String, f32)> = arguments
        .filter_map(|argument| {
            let (slug, valeur) = argument.rsplit_once('=')?;
            Some((slug.to_owned(), valeur.parse().ok()?))
        })
        .collect();

    let gabarit = gabarit::charger(Path::new(&source)).unwrap_or_else(|erreur| panic!("{erreur}"));
    let dalle = gabarit
        .rendre(&|slug| {
            valeurs
                .iter()
                .find(|(connu, _)| connu == slug)
                .map(|(_, valeur)| *valeur)
        })
        .unwrap_or_else(|erreur| panic!("{erreur}"));

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
}
