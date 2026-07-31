//! Les couleurs qu'une animation envoie réellement, en clair.
//!
//! Une animation fausse ne lève pas d'erreur : elle s'affiche — ou pas. Quand
//! le boîtier reste noir alors que le démon écrit, la question est de savoir ce
//! qu'il écrit, et ce programme y répond sans matériel.
//!
//! `cargo run --release --example apercu -p reverb-anim -- [animation] [pas] [direction]`

use reverb_anim::{Animation, Geometrie, Reglages};
use reverb_proto::Position;

fn main() {
    let mut args = std::env::args().skip(1);
    let nom = args.next().unwrap_or_else(|| "vague".to_owned());
    let pas: u32 = args
        .next()
        .and_then(|brut| brut.parse().ok())
        .unwrap_or_default();
    let direction = args.next();

    let animation = Animation::par_nom(&nom).expect("animation du catalogue");
    let geometrie = Geometrie::mesuree();
    let reglages = match &direction {
        Some(slug) => animation
            .reglages(&[("direction".to_owned(), slug.clone())])
            .expect("direction connue"),
        None => Reglages::default(),
    };
    let image = animation.image(&geometrie, &reglages, pas);

    println!(
        "« {nom} » au pas {pas}, direction {} :\n",
        direction.as_deref().unwrap_or("par défaut")
    );
    for (position, couleurs) in &image.ventilateurs {
        let _ = position;
        print!("{:<18}", position.slug());
        for couleur in couleurs {
            print!(" {:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b);
        }
        println!();
    }
    for (slot, couleurs) in image.barrettes.iter().enumerate() {
        print!("{:<18}", format!("barrette {slot}"));
        for couleur in couleurs {
            print!(" {:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b);
        }
        println!();
    }

    let allumees = image
        .ventilateurs
        .iter()
        .flat_map(|(_, couleurs)| couleurs.iter())
        .chain(image.barrettes.iter().flatten())
        .filter(|c| c.r > 0 || c.g > 0 || c.b > 0)
        .count();
    println!("\n{allumees} LED allumées sur 124");
    let _ = Position::ALL;
}
