//! Pilotage en ligne de commande des contrôleurs RGB NZXT.
//!
//! Outil de validation du protocole (issue #1). Le produit final sera un démon
//! et une fenêtre ; ce binaire sert à prouver la chaîne de bout en bout.

use std::collections::BTreeSet;
use std::process::ExitCode;

use reverb_cli::cli::{self, Cible, Command};
use reverb_cli::hidraw::{self, Controller};
use reverb_proto::{Brightness, Model, Position, Rgb, frame};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    let commande = match cli::parse(&arguments) {
        Ok(commande) => commande,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    let resultat = match commande {
        Command::List => lister(),
        Command::Set {
            cible,
            color,
            brightness,
            skip_init,
        } => appliquer(cible, color, brightness, skip_init),
    };

    match resultat {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("erreur : {message}");
            ExitCode::FAILURE
        }
    }
}

/// Énumère les contrôleurs et les positions que chacun pilote.
fn lister() -> Result<(), String> {
    let controleurs = decouvrir()?;

    for controleur in &controleurs {
        let positions: Vec<&str> = Position::ALL
            .iter()
            .filter(|p| p.placement().serial == controleur.serial)
            .map(|p| p.name())
            .collect();

        println!(
            "{} — {:04x}:{:04x} série {}",
            controleur.path.display(),
            reverb_proto::VENDOR_ID,
            controleur.model.product_id(),
            controleur.serial
        );
        if positions.is_empty() {
            println!("    aucune position connue pour cette série");
        } else {
            println!("    {}", positions.join(", "));
        }
    }

    Ok(())
}

/// Applique une couleur à la cible demandée.
fn appliquer(
    cible: Cible,
    color: Rgb,
    brightness: Brightness,
    skip_init: bool,
) -> Result<(), String> {
    let controleurs = decouvrir()?;

    let positions: Vec<Position> = match cible {
        Cible::Tous => Position::ALL.to_vec(),
        Cible::Une(position) => vec![position],
    };

    // Un contrôleur ne doit être initialisé qu'une fois, même si plusieurs de
    // ses canaux sont visés.
    if !skip_init {
        let series: BTreeSet<&str> = positions.iter().map(|p| p.placement().serial).collect();
        for serie in series {
            let (controleur, modele) = resoudre(&controleurs, serie)?;
            initialiser(controleur, modele)?;
        }
    }

    for position in positions {
        let placement = position.placement();
        let (controleur, _) = resoudre(&controleurs, placement.serial)?;
        let trame = frame::fixed_color(
            placement.mask,
            color,
            brightness,
            reverb_proto::LEDS_PER_FAN,
        );
        hidraw::write_frame(&controleur.path, &trame).map_err(|e| {
            format!(
                "écriture sur {} pour « {position} » : {e}",
                controleur.path.display()
            )
        })?;
    }

    Ok(())
}

/// Rejoue la séquence d'initialisation d'un contrôleur (spec §8).
fn initialiser(controleur: &Controller, modele: Model) -> Result<(), String> {
    for trame in frame::init_sequence(modele) {
        hidraw::write_frame(&controleur.path, &trame)
            .map_err(|e| format!("initialisation de {} : {e}", controleur.path.display()))?;
    }
    Ok(())
}

/// Retrouve un contrôleur par son numéro de série.
fn resoudre<'a>(
    controleurs: &'a [Controller],
    serie: &str,
) -> Result<(&'a Controller, Model), String> {
    controleurs
        .iter()
        .find(|c| c.serial == serie)
        .map(|c| (c, c.model))
        .ok_or_else(|| {
            format!(
                "contrôleur de série {serie} introuvable. \
                 Vérifiez qu'il est branché, puis « reverb list »."
            )
        })
}

/// Découvre les contrôleurs, en expliquant l'échec le cas échéant.
fn decouvrir() -> Result<Vec<Controller>, String> {
    let controleurs =
        hidraw::discover().map_err(|e| format!("exploration de /sys/class/hidraw : {e}"))?;

    if controleurs.is_empty() {
        return Err(
            "aucun contrôleur RGB NZXT trouvé. Vérifiez le branchement, \
             et que /dev/hidraw* est accessible."
                .to_owned(),
        );
    }

    Ok(controleurs)
}
