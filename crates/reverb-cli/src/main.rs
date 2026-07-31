//! Pilotage en ligne de commande des contrôleurs RGB NZXT.
//!
//! Outil de validation du protocole (issue #1). Le produit final sera un démon
//! et une fenêtre ; ce binaire sert à prouver la chaîne de bout en bout.

use std::collections::BTreeSet;
use std::process::ExitCode;

use reverb_cli::cli::{
    self, ActionEcran, ActionRam, ActionVentilateur, Cible, CibleCanal, CibleRam, Command,
};
use reverb_hw::hidraw::{self, Controller};
use reverb_hw::hwmon::{self, FanChannel, Percent};
use reverb_hw::i2c;
use reverb_hw::usbfs;
use reverb_proto::ram::{self, SlotAddress};
use reverb_proto::{Apply, Brightness, Mode, Model, Position, Rgb, frame, screen};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    let commande = match cli::parse(&arguments) {
        Ok(commande) => commande,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(message) = ceder_le_pas(&commande) {
        eprintln!("erreur : {message}");
        return ExitCode::FAILURE;
    }

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
        Command::Curve {
            canal,
            points,
            force,
        } => poser_courbe(&canal, &points, force),
        Command::Screen { action } => piloter_ecran(action),
        Command::Ram { action } => piloter_ram(action),
    };

    match resultat {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("erreur : {message}");
            ExitCode::FAILURE
        }
    }
}

/// Chemin du socket du démon.
const SOCKET_DU_DEMON: &str = "/run/reverb/reverbd.sock";

/// Refuse d'écrire sur le matériel quand le démon tourne.
///
/// L'ADR-002 pose qu'un seul processus doit détenir les bus. Rien dans le noyau
/// ne l'impose — plusieurs processus peuvent ouvrir le même `/dev/hidraw*` ou
/// le même `/dev/i2c-*` — et deux écritures SMBus qui se croisent corrompent
/// une transaction (SPEC-CORSAIR-RAM §6).
///
/// Le refus ne porte que sur les commandes qui **écrivent**. Énumérer reste
/// permis, et `screen` aussi : le démon ne tient pas l'écran, précisément pour
/// que cet outil garde de quoi diagnostiquer quand la fenêtre ne suffit pas.
///
/// La présence du fichier ne suffit pas à conclure — un socket peut survivre à
/// un arrêt brutal. On se connecte : c'est le seul test qui distingue un démon
/// vivant d'un fichier mort.
fn ceder_le_pas(commande: &Command) -> Result<(), String> {
    let ecrit = matches!(
        commande,
        Command::Set { .. }
            | Command::Paint { .. }
            | Command::Fan { .. }
            | Command::Curve { .. }
            | Command::Ram { .. }
    );
    if !ecrit || std::os::unix::net::UnixStream::connect(SOCKET_DU_DEMON).is_err() {
        return Ok(());
    }

    Err(format!(
        "le démon tourne et détient les bus — cet outil refuse d'écrire en même temps.\n  \
         Passer par lui :\n    \
         echo 'light all ff00ff' | socat - UNIX-CONNECT:{SOCKET_DU_DEMON}\n  \
         ou l'arrêter le temps d'un diagnostic :\n    \
         sudo systemctl stop reverbd"
    ))
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
            ActionVentilateur::Curve => {
                // Mettre en service une courbe qui n'existe pas laisserait le
                // canal sur ce que le firmware a en mémoire — au mieux inconnu.
                if canal.curve.is_empty() {
                    return Err(format!(
                        "« {} » n'a pas de courbe matérielle : rien à mettre en service.",
                        canal.name
                    ));
                }
                hwmon::set_mode(canal, hwmon::Mode::HostCurve)
                    .map_err(|e| echec_ecriture(canal, &e))?;
            }
            ActionVentilateur::Consigne {
                percent,
                force,
                manual,
            } => consigner(canal, percent, force, manual)?,
        }
    }

    Ok(())
}

/// Écrit une courbe sur un canal qui en a une.
fn poser_courbe(nom: &str, points: &[(usize, Percent)], force: bool) -> Result<(), String> {
    let canaux = canaux_decouverts()?;
    let canal = canaux
        .iter()
        .find(|c| c.name == nom)
        .ok_or_else(|| canal_inconnu(nom, &canaux))?;

    if canal.curve.is_empty() {
        let avec_courbe: Vec<&str> = canaux
            .iter()
            .filter(|c| !c.curve.is_empty())
            .map(|c| c.name.as_str())
            .collect();
        return Err(if avec_courbe.is_empty() {
            format!("« {nom} » n'a pas de courbe matérielle, et aucun canal n'en expose.")
        } else {
            format!(
                "« {nom} » n'a pas de courbe matérielle. Canaux qui en ont une : {}.",
                avec_courbe.join(", ")
            )
        });
    }

    // Le plancher s'applique point par point, comme la consigne fixe de #7.
    let sous_plancher = points
        .iter()
        .find(|(_, consigne)| consigne.percent() < Percent::FLOOR);
    if let (false, Some((point, consigne))) = (force, sous_plancher) {
        return Err(format!(
            "consigne de {} % au point {point}, sous le plancher de {} %. \
             Utilisez « --force » si c'est voulu.",
            consigne.percent(),
            Percent::FLOOR
        ));
    }

    let courbe = hwmon::Curve::interpolate(points).map_err(|e| e.to_string())?;
    hwmon::set_curve(canal, &courbe).map_err(|e| echec_ecriture(canal, &e))?;

    // Écrire la courbe ne la met pas en service : le canal continue de suivre
    // le mode qu'il avait. Le dire, plutôt que de laisser croire à un effet.
    println!(
        "Courbe écrite sur « {nom} ». Le canal reste en mode « {} ».",
        canal
            .mode()
            .map(|m| m.to_string())
            .unwrap_or_else(|_| "illisible".to_owned())
    );

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

// ─── Écran du Kraken (issue #13) ─────────────────────────────────────────────

/// Retrouve le `/dev/hidraw*` du Kraken.
///
/// `hidraw::discover` ne rend que les contrôleurs d'éclairage : le Kraken n'est
/// pas un `Model`, il ne pilote aucune LED de ventilateur.
fn hidraw_du_kraken() -> Result<std::path::PathBuf, String> {
    const KRAKEN: u16 = 0x300c;

    let entrees = std::fs::read_dir("/sys/class/hidraw")
        .map_err(|e| format!("/sys/class/hidraw illisible : {e}"))?;

    for entree in entrees.flatten() {
        let uevent = entree.path().join("device/uevent");
        let Ok(contenu) = std::fs::read_to_string(&uevent) else {
            continue;
        };
        let Some(infos) = hidraw::parse_uevent(&contenu) else {
            continue;
        };
        if infos.vendor_id == reverb_proto::VENDOR_ID && infos.product_id == KRAKEN {
            return Ok(std::path::Path::new("/dev").join(entree.file_name()));
        }
    }

    Err("aucun Kraken 1e71:300c branché.".to_owned())
}

fn piloter_ecran(action: ActionEcran) -> Result<(), String> {
    match action {
        ActionEcran::Etat => afficher_etat_ecran(),
        ActionEcran::Luminosite(percent) => regler_luminosite(percent),
        ActionEcran::Image { chemin, once } => {
            let donnees = std::fs::read(&chemin)
                .map_err(|e| format!("« {} » illisible : {e}", chemin.display()))?;
            // Refusée AVANT d'ouvrir le moindre périphérique.
            screen::check_image(&donnees).map_err(|e| {
                format!(
                    "{e}.\n  Convertir une image quelconque :\n    \
                     ffmpeg -i image.png -vf scale={}:{} -f rawvideo -pix_fmt bgr24 image.raw",
                    screen::WIDTH,
                    screen::HEIGHT
                )
            })?;
            diffuser(&donnees, once)
        }
        ActionEcran::Mire { once } => diffuser(&screen::test_pattern(), once),
    }
}

fn afficher_etat_ecran() -> Result<(), String> {
    let chemin = hidraw_du_kraken()?;
    let reponse = hidraw::ask(&chemin, &screen::query_state(), &[0x31, 0x01])
        .map_err(|e| format!("pas de réponse du Kraken : {e}"))?;
    let etat = screen::parse_state(&reponse).map_err(|e| format!("réponse illisible : {e}"))?;

    println!("Écran du Kraken — {}", chemin.display());
    println!("  résolution  : {} × {}", etat.width, etat.height);
    println!("  luminosité  : {} %", etat.brightness);
    println!("  orientation : {}", etat.orientation);
    Ok(())
}

fn regler_luminosite(percent: u8) -> Result<(), String> {
    let trame = screen::set_brightness(percent).map_err(|e| e.to_string())?;
    let chemin = hidraw_du_kraken()?;
    hidraw::write_frame(&chemin, &trame).map_err(|e| format!("écriture refusée : {e}"))?;
    println!("Luminosité de l'écran réglée à {percent} %.");
    if percent == 0 {
        println!("  (0 % éteint l'écran — « reverb screen --brightness 80 » le rallume)");
    }
    Ok(())
}

/// Envoie une image, une fois ou en boucle.
///
/// La boucle n'est pas un confort : l'écran retombe sur son affichage firmware
/// au bout d'une trentaine de secondes sans nouvel envoi (spec §2.2.2). C'est
/// aussi le seul moyen connu d'en revenir — aucune trame ne ramène au mode
/// firmware, il suffit de cesser d'émettre (spec §2.3).
fn diffuser(image: &[u8], once: bool) -> Result<(), String> {
    let chemin_hid = hidraw_du_kraken()?;
    let ecran = usbfs::Screen::open().map_err(|e| {
        format!(
            "écran inaccessible : {e}.\n  \
             Si c'est un refus de permission, installer la règle udev :\n    \
             sudo cp packaging/60-reverb.rules /etc/udev/rules.d/ && \
             sudo udevadm control --reload && sudo udevadm trigger"
        )
    })?;

    // INDISPENSABLE : sans cette trame, l'image est ignorée en silence.
    hidraw::write_frame(&chemin_hid, &screen::broadcast_mode())
        .map_err(|e| format!("mode de diffusion refusé : {e}"))?;

    let entete = screen::bulk_header(
        u32::try_from(image.len()).map_err(|_| "image trop volumineuse".to_owned())?,
    );

    // Le contrôleur ACQUITTE chaque étape, et CAM attend l'accusé avant de
    // passer à la suivante (spec §3.2) : 36 01 → 37 01, puis les données, puis
    // 36 02 → 37 02. Envoyer les 1,2 Mo sans attendre 37 01, c'est parler à un
    // contrôleur qui n'écoute pas encore.
    let envoyer = || -> Result<(), String> {
        let accuse = hidraw::ask(&chemin_hid, &screen::begin_image(), &[0x37, 0x01])
            .map_err(|e| format!("annonce sans accusé : {e}"))?;
        verifier_accuse(&accuse, "l'annonce")?;

        ecran
            .write_bulk(&entete)
            .map_err(|e| format!("en-tête refusé : {e}"))?;
        ecran
            .write_bulk(image)
            .map_err(|e| format!("image refusée : {e}"))?;

        let accuse = hidraw::ask(&chemin_hid, &screen::end_image(), &[0x37, 0x02])
            .map_err(|e| format!("validation sans accusé : {e}"))?;
        verifier_accuse(&accuse, "la validation")?;

        Ok(())
    };

    envoyer()?;

    if once {
        println!(
            "Image envoyée une fois. Le firmware reprendra la main dans ~{} s.",
            screen::FIRMWARE_FALLBACK_SECS
        );
        return Ok(());
    }

    println!(
        "Image affichée, réémise toutes les {} s. Ctrl-C pour rendre l'écran au firmware.",
        screen::REFRESH_INTERVAL_SECS
    );
    loop {
        std::thread::sleep(std::time::Duration::from_secs(
            screen::REFRESH_INTERVAL_SECS,
        ));
        envoyer()?;
    }
}

/// Cadence de l'animation de la RAM.
///
/// **Un choix, pas une observation** : la spec §4.4 dit seulement qu'iCUE
/// réémet « plusieurs fois par seconde ». Chaque image coûte huit transferts
/// (deux blocs × quatre barrettes) ; 10 Hz suffit à une vague et limite la
/// contention sur un bus que `spd5118` partage. À revoir si le rendu saccade.
const INTERVALLE_ANIMATION_MS: u64 = 100;

/// Longueur de la traînée de la comète, en LED.
const TRAINEE: u32 = 12;

fn piloter_ram(action: ActionRam) -> Result<(), String> {
    match action {
        ActionRam::Lister => {
            lister_barrettes();
            Ok(())
        }
        ActionRam::Couleur { cible, color } => {
            let barrettes = barrettes_visees(cible)?;
            let couleurs = [color; ram::LEDS_PER_STICK];
            let bus = ouvrir_bus()?;
            for barrette in &barrettes {
                ecrire_barrette(&bus, *barrette, &couleurs)?;
            }
            println!(
                "{} barrette(s) en #{:02x}{:02x}{:02x}. \
                 La couleur tient sans hôte — ce contrôleur n'a pas de watchdog.",
                barrettes.len(),
                color.r,
                color.g,
                color.b
            );
            Ok(())
        }
        ActionRam::Couleurs { slot, colors } => {
            let barrette = SlotAddress::new(slot).map_err(|e| e.to_string())?;
            // Refusée AVANT d'ouvrir le moindre périphérique.
            ram::payload(&colors).map_err(|e| e.to_string())?;

            let bus = ouvrir_bus()?;
            ecrire_barrette(&bus, barrette, &colors)?;
            println!("{barrette} : {} LED peintes une à une.", colors.len());
            Ok(())
        }
        ActionRam::Animer => animer(),
    }
}

/// Énumère les barrettes sans ouvrir `/dev/i2c-*`.
///
/// L'adaptateur est cherché, mais seulement dans sysfs : lire un nom n'est pas
/// parler sur le bus. Le §6 de la spec suggère de sonder `0x18`–`0x1b` pour
/// identifier le bon adaptateur — **on ne le fait pas**, un scan en lecture
/// seule ayant déjà altéré l'éclairage par défaut de cette RAM.
fn lister_barrettes() {
    println!(
        "RAM Corsair — {} barrettes, {} LED chacune",
        ram::SLOT_COUNT,
        ram::LEDS_PER_STICK
    );
    for barrette in SlotAddress::ALL {
        println!(
            "  emplacement {}   adresse SMBus {:#04x}",
            barrette.slot(),
            barrette.address()
        );
    }

    println!();
    match i2c::find_adapter() {
        Ok(chemin) => println!(
            "Adaptateur : {} — « {} »",
            chemin.display(),
            i2c::ADAPTER_NAME
        ),
        Err(erreur) => println!("Adaptateur : introuvable.\n{erreur}"),
    }
}

fn barrettes_visees(cible: CibleRam) -> Result<Vec<SlotAddress>, String> {
    match cible {
        CibleRam::Toutes => Ok(SlotAddress::ALL.to_vec()),
        CibleRam::Une(slot) => Ok(vec![SlotAddress::new(slot).map_err(|e| e.to_string())?]),
    }
}

fn ouvrir_bus() -> Result<i2c::Bus, String> {
    let chemin =
        i2c::find_adapter().map_err(|e| format!("adaptateur SMBus non identifié : {e}"))?;
    i2c::Bus::open(&chemin).map_err(|e| {
        format!(
            "{} inaccessible : {e}.\n  \
             Si c'est un refus de permission, installer la règle udev :\n    \
             sudo cp packaging/60-reverb.rules /etc/udev/rules.d/ && \
             sudo udevadm control --reload && sudo udevadm trigger",
            chemin.display()
        )
    })
}

/// Écrit les onze couleurs d'une barrette : deux blocs, vers la même adresse.
fn ecrire_barrette(bus: &i2c::Bus, barrette: SlotAddress, colors: &[Rgb]) -> Result<(), String> {
    let (tete, queue) = ram::transfers(colors).map_err(|e| e.to_string())?;

    bus.target(barrette).map_err(|erreur| {
        let indice = if erreur.kind() == std::io::ErrorKind::ResourceBusy {
            "\n  Un pilote noyau détient cette adresse. C'est le garde-fou qui joue : \
             Reverb emploie I2C_SLAVE, qui refuse, et non I2C_SLAVE_FORCE, qui passerait outre."
        } else {
            ""
        };
        format!("{barrette} injoignable : {erreur}{indice}")
    })?;

    // Les deux transferts se suivent immédiatement (spec §4.3). Rien entre eux :
    // une barrette qui reçoit le premier bloc sans le second affiche un état
    // dont le CRC n'est jamais arrivé.
    bus.write_block(&tete)
        .map_err(|e| format!("{barrette}, bloc {:#04x} refusé : {e}", ram::REGISTER_HEAD))?;
    bus.write_block(&queue)
        .map_err(|e| format!("{barrette}, bloc {:#04x} refusé : {e}", ram::REGISTER_TAIL))?;
    Ok(())
}

/// Anime la RAM, jusqu'à ce qu'on arrête la commande.
///
/// ⚠️ **C'est la seule contrainte temps réel du projet.** Les ventilateurs NZXT
/// animent seuls, l'écran du Kraken affiche la température seul, la RAM non :
/// son contrôleur ne fait qu'afficher le dernier état reçu (spec §4.5, testé et
/// négatif pour le mode `onDevice`).
///
/// Aucun gestionnaire de signal : le contrôleur n'ayant pas de watchdog, la
/// mort du processus laisse la dernière image affichée. C'est exactement le
/// comportement attendu, et il ne coûte pas une ligne.
fn animer() -> Result<(), String> {
    let bus = ouvrir_bus()?;
    println!(
        "Vague sur les {} barrettes, une image toutes les {INTERVALLE_ANIMATION_MS} ms.\n\
         Ctrl-C pour arrêter — la dernière image reste affichée.",
        ram::SLOT_COUNT
    );

    let mut pas: u32 = 0;
    loop {
        for barrette in SlotAddress::ALL {
            ecrire_barrette(&bus, barrette, &vague(pas, barrette.slot()))?;
        }
        pas = pas.wrapping_add(1);
        std::thread::sleep(std::time::Duration::from_millis(INTERVALLE_ANIMATION_MS));
    }
}

/// Une image de la vague : une comète qui parcourt les 44 LED des quatre
/// barrettes, tête en tête de la première.
///
/// **Cette animation est de nous, pas de la capture** — c'est pourquoi elle vit
/// ici et non dans `reverb-proto`, dont la règle est de ne rien inventer. Seule
/// sa *nécessité* est observée : le §4.1.1 montre bien une vague qui s'éteint
/// LED par LED, ce qui prouve que les onze sont adressables séparément.
fn vague(pas: u32, slot: usize) -> [Rgb; ram::LEDS_PER_STICK] {
    const TOTAL: u32 = (ram::SLOT_COUNT * ram::LEDS_PER_STICK) as u32;

    let tete = pas % TOTAL;
    let mut couleurs = [Rgb::BLACK; ram::LEDS_PER_STICK];

    for (led, couleur) in couleurs.iter_mut().enumerate() {
        let position = (slot * ram::LEDS_PER_STICK + led) as u32;
        // Recul derrière la tête, sur l'anneau des 44 LED.
        let recul = (TOTAL + position - tete) % TOTAL;
        if recul >= TRAINEE {
            continue;
        }
        let intensite = (255 - recul * 255 / TRAINEE) as u8;
        *couleur = Rgb::new(intensite, intensite / 3, intensite);
    }

    couleurs
}

/// Offset du verdict dans un accusé du Kraken.
///
/// `liquidctl` lit `response[14] == 0x1` pour conclure au succès, et tous les
/// accusés de la capture portent bien `01` à cet offset (spec §3.2). Une autre
/// valeur signale donc un refus, qu'il vaut mieux voir que traverser.
const OFFSET_VERDICT: usize = 14;

fn verifier_accuse(accuse: &reverb_proto::Frame, etape: &str) -> Result<(), String> {
    if accuse[OFFSET_VERDICT] == 0x01 {
        return Ok(());
    }
    Err(format!(
        "{etape} refusée par le contrôleur : accusé portant {:#04x} à l'offset {OFFSET_VERDICT}, attendu 0x01",
        accuse[OFFSET_VERDICT]
    ))
}
