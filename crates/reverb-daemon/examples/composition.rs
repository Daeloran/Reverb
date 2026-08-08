//! Dessine une composition d'écran dans un fichier, sans écran ni matériel.
//!
//! ```bash
//! cargo run --release --example composition -p reverb-daemon -- /tmp/composition.ppm
//! cargo run --release --example composition -p reverb-daemon -- /tmp/sur-blanc.ppm blanc
//! cargo run --release --example composition -p reverb-daemon -- /tmp/fond.ppm /home/nico/fond.png
//! ```
//!
//! C'est ainsi qu'on vérifie qu'un champ **se lit sur n'importe quel fond**
//! ailleurs qu'à l'œil sur une dalle de six centimètres. Le second argument est
//! `noir`, `blanc`, ou le chemin d'une image ; le disque visible est tracé
//! par-dessus, parce que le tampon est carré et que la dalle ne l'est pas —
//! sans ce repère, on jugerait la mise en page sur 21 % de surface qui n'existe
//! pas.
//!
//! Le format est du **PPM binaire** (P6), comme l'exemple `cadran`.

use std::io::Write;
use std::path::Path;

use reverb_daemon::ecran::{ChampRendu, Dalle};
use reverb_proto::composition::Ancre;
use reverb_proto::screen;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let sortie = arguments
        .next()
        .unwrap_or_else(|| "/tmp/composition.ppm".to_owned());
    let fond = match arguments.next().as_deref() {
        Some("blanc") => Dalle::unie((0xff, 0xff, 0xff)),
        Some(chemin) if chemin != "noir" => Dalle::depuis_fichier(Path::new(chemin))
            .expect("image de fond lisible")
            .into_iter()
            .next()
            .expect("au moins une image"),
        _ => Dalle::noire(),
    };

    // Les cinq ancres à la fois — au-delà du plafond de quatre, exprès : c'est
    // la mise en page qu'on regarde, pas la règle, et il faut voir si le champ
    // central tient entre ses deux voisins.
    let champs = [
        (
            Ancre::Haut,
            ChampRendu::Temperature {
                libelle: Some("LIQUIDE".to_owned()),
                valeur: Some(34.2),
            },
        ),
        (
            Ancre::Gauche,
            ChampRendu::Temperature {
                libelle: Some("CPU".to_owned()),
                valeur: Some(72.5),
            },
        ),
        (
            Ancre::Centre,
            ChampRendu::Temperature {
                libelle: None,
                valeur: Some(100.0),
            },
        ),
        (
            Ancre::Droite,
            ChampRendu::Temperature {
                libelle: Some("GPU".to_owned()),
                // Une sonde muette : elle doit rendre des tirets, et surtout
                // pas ressembler à un zéro.
                valeur: None,
            },
        ),
        (Ancre::Bas, ChampRendu::Texte("SHYNAEL".to_owned())),
    ];

    let dalle = Dalle::composee(&fond, &champs);

    let mut ppm = format!("P6\n{} {}\n255\n", screen::WIDTH, screen::HEIGHT).into_bytes();
    let centre = f64::from(screen::WIDTH) / 2.0;
    let rayon = f64::from(screen::VISIBLE_DISC_RADIUS);
    for (rang, pixel) in dalle.octets().chunks_exact(screen::PIXEL_LEN).enumerate() {
        let (x, y) = (
            (rang % usize::from(screen::WIDTH)) as f64,
            (rang / usize::from(screen::WIDTH)) as f64,
        );
        // Le bord du disque, en magenta : ce qui tombe dehors ne s'affichera
        // jamais, et une mise en page jugée sur le carré serait jugée à faux.
        let distance = (x - centre + 0.5).hypot(y - centre + 0.5);
        if (distance - rayon).abs() < 1.0 {
            ppm.extend_from_slice(&[0xff, 0x00, 0xff]);
            continue;
        }
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
