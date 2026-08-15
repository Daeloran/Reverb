//! Pilotage en ligne de commande des contrôleurs RGB NZXT.
//!
//! Outil de validation du protocole (issue #1). Le produit final sera un démon
//! et une fenêtre ; ce binaire sert à prouver la chaîne de bout en bout.

use std::collections::BTreeSet;
use std::process::ExitCode;

use reverb_cli::cli::{
    self, ActionEcran, ActionRam, ActionVentilateur, Cible, CibleCanal, CibleRam, Command,
};
use reverb_cli::refus_de_consigne;
use reverb_hw::hidraw::{self, Controller};
use reverb_hw::hwmon::{self, FanChannel, Percent};
use reverb_hw::i2c;
use reverb_hw::usbfs;
use reverb_proto::ipc::{self, Request, ResponseLine, ScreenAction};
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
            activer,
        } => poser_courbe(&canal, &points, force, activer),
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
/// Le refus ne porte que sur les commandes qui **écrivent en direct**. Énumérer
/// reste permis, et les commandes d'écran aussi — elles passent désormais par
/// le socket (#33), donc par le démon lui-même. La seule exception est
/// `screen --mire`, outil de diagnostic qui n'est pas dans le protocole et qui
/// écrit donc sur le bus.
///
/// La présence du fichier ne suffit pas à conclure — un socket peut survivre à
/// un arrêt brutal. On se connecte : c'est le seul test qui distingue un démon
/// vivant d'un fichier mort.
///
/// ⚠️ **Un échec de connexion ne veut pas dire « pas de démon ».** Un
/// utilisateur absent du groupe `reverb` se voit refuser la connexion par un
/// démon parfaitement vivant. Traiter cet échec comme une absence laisserait
/// précisément cet utilisateur écrire sur un bus déjà tenu — l'inverse de ce
/// que cette fonction protège. On ne conclut donc à l'absence que sur les deux
/// erreurs qui la signifient vraiment : pas de fichier, ou personne à l'écoute.
fn ceder_le_pas(commande: &Command) -> Result<(), String> {
    use std::io::ErrorKind;

    let ecrit = matches!(
        commande,
        Command::Set { .. }
            | Command::Paint { .. }
            | Command::Fan { .. }
            | Command::Ram { .. }
            // Les mires sont les seules commandes d'écran qui ne passent pas
            // par le socket : ce sont des outils de diagnostic, et elles n'ont
            // pas leur place dans le protocole. Elles écrivent donc en direct,
            // ce qui suppose un démon arrêté — le nœud USB ne se réclame pas
            // deux fois.
            | Command::Screen {
                action: ActionEcran::Mire { .. } | ActionEcran::MireCercle { .. }
            }
    );
    if !ecrit {
        return Ok(());
    }

    match std::os::unix::net::UnixStream::connect(SOCKET_DU_DEMON) {
        // Aucun socket, ou un fichier mort dont plus personne n'écoute.
        Err(erreur)
            if matches!(
                erreur.kind(),
                ErrorKind::NotFound | ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(());
        }
        Err(erreur) => {
            return Err(format!(
                "impossible de savoir si le démon tourne : {SOCKET_DU_DEMON} : {erreur}.\n  \
                 Refus par précaution — écrire sur un bus peut-être déjà tenu corromprait une \
                 transaction.\n  \
                 Si c'est un refus de permission, il manque l'appartenance au groupe :\n    \
                 sudo usermod -aG reverb \"$USER\"   (puis rouvrir la session)"
            ));
        }
        Ok(_) => {}
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

    // ⚠️ **Un carnet neuf, et il le restera** : chaque invocation de `reverb`
    // est un processus, donc un démarrage, et le carnet des courbes posées ne
    // survit jamais à un démarrage — les fichiers de courbe étant en écriture
    // seule, rien ne se relit sur le matériel (issue #97). `--auto` et
    // `--curve`, qui écrivent tous deux `2`, sont donc refusés ici tant que la
    // courbe n'est pas posée dans le même souffle. Même conséquence que côté
    // démon, et pour la même raison ; suivie par l'issue #104.
    let posees = hwmon::CourbesPosees::vide();

    for canal in vises {
        match action {
            // ⚠️ `HostCurve` (2) et non `PleinRegime` (0). `0` n'a jamais rendu
            // la main à une courbe : sur `nzxt-kraken3` il écrit un rapport
            // cyclique de 255 et cesse de piloter — 100 %, la barre lâchée. Et
            // sur `nzxt-smart2` il est refusé, ce contrôleur n'ayant aucun mode
            // automatique (issue #50).
            //
            // ⚠️ **Mais `2` ne rend pas non plus la main au profil d'usine**,
            // contrairement à ce que « auto » laisse croire : il fait exécuter
            // la courbe de l'**hôte**, celle que le pilote détient — zéro
            // partout tant que personne ne l'a posée. Mesuré sur SHYNAEL le
            // 2026-08-15 : pompe à 0 % de consigne, 1910 tr/min, sans une
            // erreur (issue #97).
            ActionVentilateur::Auto => hwmon::set_mode(canal, hwmon::Mode::HostCurve, &posees)
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
                hwmon::set_mode(canal, hwmon::Mode::HostCurve, &posees)
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

/// Écrit une courbe sur un canal qui en a une, et la bascule si `activer`.
///
/// ⚠️ **Passe par le démon quand il tourne**, comme `screen` depuis #33 : une
/// courbe fait quarante points et tient sur une ligne de texte, contrairement au
/// mégaoctet d'une image, donc le protocole n'a pas à être contourné. Sans démon,
/// elle écrit en direct comme avant.
///
/// ⚠️ **Le plancher et l'interpolation restent ici**, en amont du socket : ce
/// sont les garde-fous de la ligne de commande, et les déplacer côté démon les
/// imposerait aussi à la fenêtre, qui a les siens.
fn poser_courbe(
    nom: &str,
    points: &[(usize, Percent)],
    force: bool,
    activer: bool,
) -> Result<(), String> {
    // Le démon d'abord : lui seul détient les bus quand il tourne. La découverte
    // des canaux qui suit lit sysfs, ce qui n'écrit rien — mais l'interroger
    // avant d'avoir cédé le pas ferait deux découvertes pour rien.
    if let Some(courbe) = courbe_pour_le_socket(nom, points, force)? {
        let requete = Request::Curve {
            channel: nom.to_owned(),
            points: courbe,
            activer,
        };
        if let Some(lignes) = parler_au_demon(&requete)? {
            for ligne in &lignes {
                if let ResponseLine::Error { message } = ligne {
                    return Err(message.clone());
                }
            }
            println!(
                "Courbe écrite sur « {nom} » par le démon.{}",
                if activer {
                    " Le canal l'exécute désormais."
                } else {
                    " Le canal garde son mode ; « --enable » l'y bascule."
                }
            );
            return Ok(());
        }
    }

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

    let courbe = hwmon::Curve::interpolate(points).map_err(|e| e.to_string())?;
    // Le carnet meurt avec le processus, et c'est voulu : il n'existe que pour
    // que « une courbe est partie » traverse `set_curve` plutôt que de se
    // décréter à côté (issue #97).
    //
    // ⚠️ **C'est ce carnet-là que #104 existe pour ne pas perdre.** Tant que la
    // bascule tenait dans un second processus — `reverb fan --curve` —, elle
    // repartait d'un carnet vide et se refusait elle-même. `--enable` l'exécute
    // donc ici, sur le carnet que `set_curve` vient de remplir.
    let mut posees = hwmon::CourbesPosees::vide();
    hwmon::set_curve(canal, &courbe, &mut posees).map_err(|e| echec_ecriture(canal, &e))?;

    if activer {
        hwmon::set_mode(canal, hwmon::Mode::HostCurve, &posees)
            .map_err(|e| echec_ecriture(canal, &e))?;
        println!("Courbe écrite sur « {nom} », et le canal l'exécute désormais.");
        return Ok(());
    }

    // Écrire la courbe ne la met pas en service : le canal continue de suivre
    // le mode qu'il avait. Le dire, plutôt que de laisser croire à un effet.
    println!(
        "Courbe écrite sur « {nom} ». Le canal reste en mode « {} » ; « --enable » l'y bascule.",
        canal
            .mode()
            .map(|m| m.to_string())
            .unwrap_or_else(|_| "illisible".to_owned())
    );

    Ok(())
}

/// Les quarante consignes prêtes pour le socket, plancher vérifié.
///
/// ⚠️ **Le plancher s'applique point par point**, comme la consigne fixe de #7,
/// et il est vérifié **ici** — donc sur les deux chemins, socket comme direct.
/// Le laisser au seul chemin direct ferait du démon une porte dérobée sur un
/// garde-fou, ce que le projet refuse partout ailleurs.
fn courbe_pour_le_socket(
    nom: &str,
    points: &[(usize, Percent)],
    force: bool,
) -> Result<Option<[u8; hwmon::CURVE_POINTS]>, String> {
    let sous_plancher = points
        .iter()
        .find(|(_, consigne)| consigne.percent() < Percent::FLOOR);
    if let (false, Some((point, consigne))) = (force, sous_plancher) {
        return Err(format!(
            "consigne de {} % au point {point}, sous le plancher de {} % pour « {nom} ». \
             Utilisez « --force » si c'est voulu.",
            consigne.percent(),
            Percent::FLOOR
        ));
    }
    let courbe = hwmon::Curve::interpolate(points).map_err(|e| e.to_string())?;
    Ok(Some(courbe.points().map(|c| c.percent())))
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

    // Un canal qui régule seul — sur une courbe, ou sur le profil d'usine du
    // périphérique — perd cette régulation dès qu'on lui écrit une consigne, et
    // il n'y reviendra pas tout seul. Ça ne doit jamais être un effet de bord.
    //
    // ⚠️ **Le verdict est calculé, et il tombe avant toute écriture** — ni
    // `set_mode`, ni `set_pwm`. `refus_de_consigne` ne reçoit ni descripteur ni
    // `&FanChannel` : elle ne *peut* pas écrire, et le critère « rien n'est
    // écrit » devient une propriété de sa signature.
    //
    // ⚠️ **Le garde a visé `0` jusqu'au 2026-08-02, puis ne l'a plus visé du
    // tout, et les deux étaient faux.** Il disait alors « suit sa courbe
    // firmware et s'adapte à la température » d'un canal qui tourne en fait à
    // 100 % sans rien réguler, ce que #50 a corrigé en le déplaçant sur
    // `HostCurve`. Mais l'exemption qui en est restée — « un canal en `0` n'a
    // rien à perdre » — ne valait que si `0` voulait dire 100 %. Or un `0`
    // **lu** dit le contraire (#101) : le pilote n'a jamais touché ce canal, et
    // le périphérique exécute son propre profil. Sur SHYNAEL le 2026-08-15, la
    // pompe y suivait le liquide de 35 à 60 %. **L'issue #112 ferme ce trou** :
    // `NonPilote` refuse comme `HostCurve`, et `--manual` lève les deux.
    let mode = canal
        .mode()
        .map_err(|e| format!("lecture du mode de « {} » : {e}", canal.name))?;

    if let Some(refus) = refus_de_consigne(&canal.name, mode, manual) {
        return Err(refus);
    }

    // Le refus levé, il faut bien sortir le canal de son mode : `set_pwm` seul
    // n'aurait aucun effet tant que `pwmN_enable` ne vaut pas `1`.
    //
    // Le carnet ne sert à rien ici — seul `HostCurve` le consulte — mais la
    // signature le réclame, et un carnet vide est ce qu'un processus neuf a.
    if matches!(mode, hwmon::Mode::HostCurve | hwmon::Mode::NonPilote) {
        hwmon::set_mode(canal, hwmon::Mode::Manual, &hwmon::CourbesPosees::vide())
            .map_err(|e| echec_ecriture(canal, &e))?;
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

/// Pilote l'écran, par le démon s'il tourne, en direct sinon.
///
/// **Le démon détient l'écran depuis #33** : le nœud USB ne se réclamant pas
/// deux fois, cet outil ne peut plus y écrire en même temps. Il passe donc par
/// le socket, comme la fenêtre — et y gagne le PNG, le JPEG et le GIF qu'il
/// n'avait pas.
///
/// Sans démon, tout ce qui marchait avant marche encore : l'état, la
/// luminosité, une image brute de 640 × 640 et la mire. C'est ce qui garde cet
/// outil utilisable pour diagnostiquer, y compris quand le démon est arrêté.
fn piloter_ecran(action: ActionEcran) -> Result<(), String> {
    if let Some(requete) = requete_d_ecran(&action)?
        && let Some(lignes) = parler_au_demon(&requete)?
    {
        return afficher_reponse_d_ecran(&lignes);
    }

    match action {
        ActionEcran::Etat => afficher_etat_ecran(),
        ActionEcran::Luminosite(percent) => regler_luminosite(percent),
        // Ces trois-là n'existent que par le démon : lui seul décode un GIF et
        // lit les sondes, et l'extinction n'est que le fait de cesser d'émettre.
        ActionEcran::Gif { .. } | ActionEcran::Cadran { .. } | ActionEcran::Eteindre => Err(
            "cette action passe par le démon, qui seul décode les images et lit les \
                 sondes.\n  Le démarrer :\n    sudo systemctl start reverbd"
                .to_owned(),
        ),
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
        ActionEcran::MireCercle { once } => {
            // ⚠️ **La légende s'imprime avant l'envoi.** C'est elle qui rend la
            // mire lisible : sans elle, on regarde neuf anneaux colorés sans
            // savoir à quel rayon chacun correspond, et la mesure ne se fait
            // pas. L'imprimer après supposerait que l'envoi réussisse.
            // ⚠️ **Cette mire se photographie, elle ne se lit pas.** La
            // première version demandait de nommer une couleur à l'œil derrière
            // une vitre teintée, et posait ses bandes dans le seul quart
            // extérieur : essayée sur SHYNAEL, elle n'a rien montré du tout.
            println!("Mire de mesure du disque visible (issue #77).");
            println!();
            println!(
                "  {} anneaux blancs, un tous les {} px de rayon.",
                screen::MIRE_ANNEAUX,
                screen::MIRE_PAS
            );
            println!(
                "  Un anneau sur {} est un repère ROUGE, deux fois plus épais :",
                screen::MIRE_REPERE_TOUS_LES
            );
            let mut reperes = Vec::new();
            for anneau in 0..screen::MIRE_ANNEAUX {
                if (anneau + 1).is_multiple_of(screen::MIRE_REPERE_TOUS_LES) {
                    reperes.push(format!("{} px", screen::mire_rayon(anneau)));
                }
            }
            println!("    {}", reperes.join(", "));
            println!();
            println!("  → PHOTOGRAPHIE la dalle bien en face, et compte les repères rouges.");
            println!("  → Les quatre coins sont rouge sombre : en voir un dirait que la");
            println!("    dalle n'est pas ronde.");
            println!("  → Les quatre rayons blancs et le point central disent si l'image");
            println!("    est centrée sur la dalle.");
            println!();
            diffuser(&screen::mire_cercle(), once)
        }
    }
}

/// La requête à envoyer au démon pour cette action, s'il y en a une.
///
/// `None` pour la mire : c'est un outil de diagnostic, elle n'a pas sa place
/// dans le protocole — et elle reste donc réservée au démon arrêté.
///
/// ⚠️ Le chemin est rendu **absolu ici**, dans le processus qui connaît le
/// répertoire courant de l'utilisateur. Le démon lit sous le sien, et un chemin
/// relatif y désignerait autre chose — ou rien.
fn requete_d_ecran(action: &ActionEcran) -> Result<Option<Request>, String> {
    let absolu = |chemin: &std::path::Path| -> Result<String, String> {
        let entier = if chemin.is_absolute() {
            chemin.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| format!("répertoire courant illisible : {e}"))?
                .join(chemin)
        };
        entier
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("chemin « {} » non représentable en UTF-8", entier.display()))
    };

    Ok(match action {
        ActionEcran::Etat => Some(Request::Screen(ScreenAction::State)),
        ActionEcran::Luminosite(percent) => {
            Some(Request::Screen(ScreenAction::Brightness(*percent)))
        }
        ActionEcran::Image { chemin, .. } => {
            Some(Request::Screen(ScreenAction::Image(absolu(chemin)?)))
        }
        ActionEcran::Gif { chemin } => Some(Request::Screen(ScreenAction::Gif(absolu(chemin)?))),
        ActionEcran::Cadran { sonde } => Some(Request::Screen(ScreenAction::Gauge(sonde.clone()))),
        ActionEcran::Eteindre => Some(Request::Screen(ScreenAction::Off)),
        ActionEcran::Mire { .. } | ActionEcran::MireCercle { .. } => None,
    })
}

/// Envoie une requête au démon et rend ses lignes, ou `None` s'il ne tourne pas.
///
/// Même prudence que `ceder_le_pas` : seules l'absence de fichier et le refus
/// de connexion signifient « pas de démon ». Un refus de permission est un
/// démon **vivant** qui ne veut pas de nous, et le traiter comme une absence
/// ferait écrire sur un bus déjà tenu.
fn parler_au_demon(requete: &Request) -> Result<Option<Vec<ResponseLine>>, String> {
    use std::io::{BufRead, BufReader, ErrorKind, Write};

    let flux = match std::os::unix::net::UnixStream::connect(SOCKET_DU_DEMON) {
        Ok(flux) => flux,
        Err(erreur)
            if matches!(
                erreur.kind(),
                ErrorKind::NotFound | ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(None);
        }
        Err(erreur) => {
            return Err(format!(
                "{SOCKET_DU_DEMON} : {erreur}.\n  \
                 Si c'est un refus de permission, il manque l'appartenance au groupe :\n    \
                 sudo usermod -aG reverb \"$USER\"   (puis rouvrir la session)"
            ));
        }
    };

    let mut sortie = flux
        .try_clone()
        .map_err(|e| format!("socket illisible : {e}"))?;
    writeln!(sortie, "{}", ipc::encode_request(requete))
        .map_err(|e| format!("commande non transmise : {e}"))?;
    sortie
        .flush()
        .map_err(|e| format!("commande non transmise : {e}"))?;

    let mut lignes = Vec::new();
    for ligne in BufReader::new(flux).lines() {
        let ligne = ligne.map_err(|e| format!("réponse illisible : {e}"))?;
        let lue = ipc::parse_response_line(&ligne)
            .map_err(|e| format!("réponse illisible : {} ({})", e.line, e.reason))?;
        let terminale = lue.is_terminal();
        lignes.push(lue);
        if terminale {
            break;
        }
    }
    Ok(Some(lignes))
}

/// Affiche ce que le démon a répondu à une commande d'écran.
fn afficher_reponse_d_ecran(lignes: &[ResponseLine]) -> Result<(), String> {
    for ligne in lignes {
        match ligne {
            ResponseLine::Screen {
                luminosite,
                affichage,
            } => {
                println!("Écran du Kraken — tenu par le démon");
                println!("  luminosité : {luminosite} %");
                println!("  affichage  : {affichage}");
            }
            // Une composition (#80) : le fond, puis un champ par ligne. Les
            // afficher ici plutôt que de les taire — « affichage : layout » ne
            // dit pas ce que la dalle montre, et c'est la question posée.
            ResponseLine::Layout { fond } => println!("  fond       : {fond}"),
            ResponseLine::LayoutChamp { ancre, source } => {
                println!("  champ {ancre:<7}: {source}");
            }
            ResponseLine::Error { message } => return Err(message.clone()),
            _ => {}
        }
    }
    Ok(())
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
