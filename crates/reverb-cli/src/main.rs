//! Pilotage en ligne de commande des contrôleurs RGB NZXT.
//!
//! Outil de validation du protocole (issue #1). Le produit final sera un démon
//! et une fenêtre ; ce binaire sert à prouver la chaîne de bout en bout.

use std::collections::BTreeSet;
use std::process::ExitCode;

use reverb_cli::cli::{self, ActionVentilateur, Cible, CibleCanal, Command};
use reverb_cli::hidraw::{self, Controller};
use reverb_cli::hwmon::{self, FanChannel, Percent};
use reverb_proto::{Apply, Brightness, Mode, Model, Position, Rgb, frame};

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
        Command::Modes => {
            lister_modes();
            Ok(())
        }
        Command::Set {
            cible,
            mode,
            colors,
            speed,
            brightness,
            skip_init,
        } => appliquer(cible, mode, &colors, speed, brightness, skip_init),
        Command::Paint {
            cible,
            colors,
            apply,
            brightness,
            skip_init,
        } => peindre(cible, &colors, apply, brightness, skip_init),
        Command::Fans => lister_ventilateurs(),
        Command::Fan { canal, action } => regler_ventilateur(&canal, action),
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

/// Énumère les modes d'animation de la spec §4.1.
fn lister_modes() {
    println!(
        "{:>4}  {:<17}  {:>8}  {:>7}",
        "CODE", "NOM", "COULEURS", "VITESSE"
    );
    for mode in Mode::ALL {
        let (min, max) = mode.colors();
        let couleurs = if min == max {
            min.to_string()
        } else {
            format!("{min} ou {max}")
        };
        let marque = if mode.confirmed() {
            ""
        } else {
            "  🔶 nom à confirmer"
        };
        // La vitesse est affichée sur deux chiffres : c'est un octet du
        // protocole, pas un nombre à lire comme une quantité.
        let vitesse = format!("{:#04x}", mode.default_speed());
        println!(
            "{:>4}  {:<17}  {couleurs:>8}  {vitesse:>7}{marque}",
            mode.code(),
            mode.name(),
        );
    }
    println!();
    // La légende ne s'affiche que si elle a un objet : depuis la session §4.5,
    // les huit modes sont confirmés. Elle resservira si un mode est ajouté à la
    // table sans avoir été vu à l'œil — le `0x03`, par exemple.
    if Mode::ALL.iter().any(|m| !m.confirmed()) {
        println!(
            "🔶 : le numéro du mode est certain, son nom reste une hypothèse — il n'a pas\n     \
             encore été vérifié à l'œil sur le matériel (spec §4.1)."
        );
    }
    println!("Vitesse : valeur brute du protocole, échelle non calibrée (spec §4.2).");
}

/// Énumère les canaux de vitesse pilotables.
fn lister_ventilateurs() -> Result<(), String> {
    let canaux = canaux_decouverts()?;

    println!(
        "{:<32}  {:<12}  {:>7}  {:>4}  MODE",
        "CANAL", "LIBELLÉ", "TR/MIN", "PWM"
    );
    for canal in &canaux {
        let regime = canal
            .tach
            .as_ref()
            .and_then(|c| std::fs::read_to_string(c).ok())
            .map(|brut| brut.trim().to_owned())
            .unwrap_or_else(|| "—".to_owned());

        let consigne = std::fs::read_to_string(&canal.pwm)
            .ok()
            .and_then(|brut| brut.trim().parse::<u8>().ok())
            .map(|brut| format!("{}%", Percent::from_raw(brut).percent()))
            .unwrap_or_else(|| "—".to_owned());

        let mode = match canal.mode() {
            Ok(mode) => mode.to_string(),
            Err(e) => format!("illisible ({e})"),
        };

        println!(
            "{:<32}  {:<12}  {regime:>7}  {consigne:>4}  {mode}",
            canal.name, canal.label
        );
    }

    Ok(())
}

/// Règle un canal, ou le rend à sa courbe firmware.
fn regler_ventilateur(cible: &CibleCanal, action: ActionVentilateur) -> Result<(), String> {
    let canaux = canaux_decouverts()?;

    let vises: Vec<&FanChannel> = match cible {
        CibleCanal::Tous => canaux.iter().collect(),
        CibleCanal::Un(nom) => vec![
            canaux
                .iter()
                .find(|c| &c.name == nom)
                .ok_or_else(|| canal_inconnu(nom, &canaux))?,
        ],
    };

    for canal in vises {
        match action {
            ActionVentilateur::Auto => hwmon::set_mode(canal, hwmon::Mode::FirmwareCurve)
                .map_err(|e| echec_ecriture(canal, &e))?,
            ActionVentilateur::Consigne {
                percent,
                force,
                manual,
            } => consigner(canal, percent, force, manual)?,
        }
    }

    Ok(())
}

/// Applique une consigne à un canal, garde-fous compris.
fn consigner(
    canal: &FanChannel,
    percent: Percent,
    force: bool,
    manual: bool,
) -> Result<(), String> {
    if percent.percent() < Percent::FLOOR && !force {
        return Err(format!(
            "consigne de {} % sous le plancher de {} % pour « {} ». \
             Utilisez « --force » si c'est voulu.",
            percent.percent(),
            Percent::FLOOR,
            canal.name
        ));
    }

    // Un canal sur sa courbe firmware réagit à la température. Lui imposer une
    // consigne fixe l'en sort — et il n'y reviendra pas tout seul. Ça ne doit
    // jamais être un effet de bord.
    let mode = canal
        .mode()
        .map_err(|e| format!("lecture du mode de « {} » : {e}", canal.name))?;

    if mode == hwmon::Mode::FirmwareCurve {
        if !manual {
            return Err(format!(
                "« {} » suit sa courbe firmware et s'adapte à la température. \
                 Lui imposer {} % l'en sortirait définitivement : ajoutez « --manual » \
                 si c'est voulu, et « reverb fan --channel {} --auto » pour l'y rendre.",
                canal.name,
                percent.percent(),
                canal.name
            ));
        }
        hwmon::set_mode(canal, hwmon::Mode::Manual).map_err(|e| echec_ecriture(canal, &e))?;
    }

    hwmon::set_pwm(canal, percent).map_err(|e| echec_ecriture(canal, &e))
}

/// Découvre les canaux, en expliquant l'absence le cas échéant.
fn canaux_decouverts() -> Result<Vec<FanChannel>, String> {
    let canaux = hwmon::discover().map_err(|e| format!("exploration de /sys/class/hwmon : {e}"))?;

    if canaux.is_empty() {
        return Err("aucun canal de vitesse trouvé. \
                    Les pilotes nzxt_smart2 et nzxt_kraken3 sont-ils chargés ?"
            .to_owned());
    }

    Ok(canaux)
}

fn canal_inconnu(nom: &str, canaux: &[FanChannel]) -> String {
    let valides: Vec<&str> = canaux.iter().map(|c| c.name.as_str()).collect();
    format!(
        "canal « {nom} » inconnu. Canaux valides : {}.",
        valides.join(", ")
    )
}

/// Traduit un échec d'écriture, en nommant la cause la plus fréquente.
fn echec_ecriture(canal: &FanChannel, erreur: &std::io::Error) -> String {
    if erreur.kind() == std::io::ErrorKind::PermissionDenied {
        return format!(
            "droits insuffisants pour régler « {} ». \
             /sys/class/hwmon n'est inscriptible que par root : relancez avec sudo.",
            canal.name
        );
    }
    format!("réglage de « {} » : {erreur}", canal.name)
}

/// Applique un mode à la cible demandée.
fn appliquer(
    cible: Cible,
    mode: Mode,
    colors: &[Rgb],
    speed: u8,
    brightness: Brightness,
    skip_init: bool,
) -> Result<(), String> {
    emettre(cible, skip_init, |mask| {
        frame::animation(
            mask,
            mode,
            colors,
            speed,
            brightness,
            reverb_proto::LEDS_PER_FAN,
        )
        .map(|trame| vec![trame])
        .map_err(|e| e.to_string())
    })
}

/// Peint les LED une par une sur la cible demandée (spec §5).
fn peindre(
    cible: Cible,
    colors: &[Rgb],
    apply: Apply,
    brightness: Brightness,
    skip_init: bool,
) -> Result<(), String> {
    emettre(cible, skip_init, |mask| {
        frame::per_led(mask, colors, apply, brightness)
            .map(|trames| trames.to_vec())
            .map_err(|e| e.to_string())
    })
}

/// Construit puis écrit les trames de chaque position visée.
///
/// `trames_de` reçoit le masque d'un canal et rend les trames à lui envoyer,
/// dans l'ordre d'émission. `set` en produit une, `paint` trois — leur
/// indissociabilité (spec §0.2) est préservée par le fait qu'elles voyagent
/// ensemble jusqu'à l'écriture.
fn emettre<F>(cible: Cible, skip_init: bool, trames_de: F) -> Result<(), String>
where
    F: Fn(u8) -> Result<Vec<frame::Frame>, String>,
{
    let controleurs = decouvrir()?;

    let positions: Vec<Position> = match cible {
        Cible::Tous => Position::ALL.to_vec(),
        Cible::Une(position) => vec![position],
    };

    // Toutes les trames sont construites **avant** la première écriture : une
    // demande invalide ne doit jamais laisser l'éclairage à moitié appliqué.
    let mut envois = Vec::with_capacity(positions.len());
    for position in &positions {
        let placement = position.placement();
        envois.push((*position, placement.serial, trames_de(placement.mask)?));
    }

    // Un contrôleur ne doit être initialisé qu'une fois, même si plusieurs de
    // ses canaux sont visés.
    if !skip_init {
        let series: BTreeSet<&str> = positions.iter().map(|p| p.placement().serial).collect();
        for serie in series {
            let (controleur, modele) = resoudre(&controleurs, serie)?;
            initialiser(controleur, modele)?;
        }
    }

    for (position, serie, trames) in envois {
        let (controleur, _) = resoudre(&controleurs, serie)?;
        for trame in &trames {
            hidraw::write_frame(&controleur.path, trame).map_err(|e| {
                format!(
                    "écriture sur {} pour « {position} » : {e}",
                    controleur.path.display()
                )
            })?;
        }
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
