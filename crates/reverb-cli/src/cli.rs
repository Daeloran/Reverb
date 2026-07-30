//! Analyse des arguments de la ligne de commande.
//!
//! Écrite à la main : quatre options ne justifient pas une dépendance, et le
//! binaire est un outil de validation interne, pas le produit final (le produit
//! sera un démon et une fenêtre).

use crate::hwmon::Percent;
use reverb_proto::{Apply, Brightness, Mode, Position, Rgb};

/// Ce que l'utilisateur a demandé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Énumère les contrôleurs trouvés et les positions qu'ils pilotent.
    List,
    /// Énumère les modes d'animation connus.
    Modes,
    /// Applique un mode à une cible.
    Set {
        cible: Cible,
        mode: Mode,
        colors: Vec<Rgb>,
        speed: u8,
        brightness: Brightness,
        skip_init: bool,
    },
    /// Peint les LED une par une.
    Paint {
        cible: Cible,
        colors: Vec<Rgb>,
        apply: Apply,
        brightness: Brightness,
        skip_init: bool,
    },
    /// Énumère les canaux de vitesse pilotables.
    Fans,
    /// Règle la vitesse d'un canal, ou le rend à sa courbe firmware.
    Fan {
        canal: CibleCanal,
        action: ActionVentilateur,
    },
    /// Écrit une courbe température → consigne, exécutée par le firmware.
    Curve {
        canal: String,
        /// Couples (point, consigne), tels que donnés — ni triés ni validés :
        /// c'est `Curve::interpolate` qui s'en charge.
        points: Vec<(usize, Percent)>,
        force: bool,
    },
}

/// Quels ventilateurs sont visés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cible {
    Tous,
    Une(Position),
}

/// Quel canal de vitesse est visé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CibleCanal {
    Tous,
    Un(String),
}

/// Ce qu'on demande au canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionVentilateur {
    /// Applique une consigne.
    Consigne {
        percent: Percent,
        /// Autorise à descendre sous le plancher.
        force: bool,
        /// Autorise à sortir un canal de sa courbe firmware.
        manual: bool,
    },
    /// Rend la main au firmware (`pwm_enable = 0`).
    Auto,
    /// Met en service la courbe téléversée (`pwm_enable = 2`).
    Curve,
}

pub const USAGE: &str = "\
reverb — pilotage des contrôleurs RGB NZXT

USAGE :
    reverb list
    reverb modes
    reverb set   --all|--fan <NOM> [OPTIONS]
    reverb paint --all|--fan <NOM> --colors <8 HEX> [OPTIONS]
    reverb fans
    reverb fan   --all|--channel <NOM> --pwm <0-100> [--force] [--manual]
    reverb fan   --all|--channel <NOM> --auto
    reverb curve --channel <NOM> --point <POINT:CONSIGNE>… [--force]

OPTIONS de « fan » — vitesse de rotation (demande les droits root) :
    --channel <NOM>      canal, tel que « reverb fans » le nomme
    --pwm <0-100>        consigne en pourcent
    --force              autorise une consigne sous le plancher de 20 %
    --manual             autorise à sortir un canal de sa courbe firmware.
                         ⚠️ le canal cesse alors de réagir à la température
    --curve              met en service la courbe téléversée par « reverb curve »
    --auto               rend la main au firmware.
                         ⚠️ ce n'est PAS un retour garanti au profil d'usine :
                         après une courbe hôte, le Kraken observé se rabat sur
                         du refroidissement maximal (docs/VENTILATEURS.md)

OPTIONS de « curve » — courbe exécutée par le firmware du Kraken :
    --point <P:C>        consigne C au point P. Répétable ; les points
                         intermédiaires sont interpolés linéairement.
                         Le point 1 vaut 20 °C, un degré par point.
    --force              autorise des consignes sous le plancher de 20 %
                         ⚠️ écrire une courbe ne bascule PAS le canal en mode
                         courbe : c'est un geste distinct, encore à exposer

OPTIONS de « set » — animation confiée au contrôleur :
    --mode <NOM|N>       mode d'animation, « fixed » par défaut (« reverb modes »)
    --color <HEX>        couleur, six chiffres hexadécimaux (ff00ff ou #ff00ff).
                         Répétable : certains modes attendent 2 ou 3 couleurs.
    --speed <0-255>      vitesse de l'animation, celle du mode par défaut.

OPTIONS de « paint » — une couleur par LED :
    --colors <LISTE>     huit couleurs séparées par des virgules, une par LED
    --animate            fait tourner le motif autour du ventilateur.
                         Le contrôleur s'en charge seul : une fois la commande
                         passée, ça tourne sans qu'aucun logiciel ne tourne.
    --speed <0-65535>    vitesse de rotation, 106 par défaut

OPTIONS communes :
    --brightness <N>     luminosité en pourcent, 100 par défaut
    --skip-init          n'envoie pas la séquence d'initialisation
    -h, --help           affiche cette aide

⚠️ Les deux échelles de vitesse sont des valeurs brutes du protocole, non
   calibrées, et n'ont rien à voir l'une avec l'autre.

EXEMPLES :
    reverb set --all --color ff00ff
    reverb set --all --mode spectrum-wave
    reverb set --all --mode alternating --color ff0000 --color 0000ff
    reverb paint --fan \"radiateur haut\" --colors ff0000,00ff00,0000ff,ffff00,ff00ff,00ffff,ffffff,000000
    reverb paint --all --colors ff0000,ff0000,ff0000,ff0000,0000ff,0000ff,0000ff,0000ff
    reverb fans
    sudo reverb fan --channel nzxtsmart2:fan-1 --pwm 60
    sudo reverb fan --all --pwm 40
    sudo reverb fan --channel kraken2023elite:pump-speed --pwm 80 --manual
    sudo reverb fan --channel kraken2023elite:pump-speed --auto
";

/// Analyse les arguments, hors nom du programme.
pub fn parse<I, S>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args.into_iter().map(|a| a.as_ref().to_owned()).collect();
    let mut args = args.into_iter();

    let Some(sous_commande) = args.next() else {
        return Err("aucune sous-commande. Essayez « reverb --help ».".to_owned());
    };

    match sous_commande.as_str() {
        "-h" | "--help" => Err(USAGE.to_owned()),
        "list" => Ok(Command::List),
        "modes" => Ok(Command::Modes),
        "set" => parse_set(args),
        "paint" => parse_paint(args),
        "fans" => Ok(Command::Fans),
        "fan" => parse_fan(args),
        "curve" => parse_curve(args),
        autre => Err(format!(
            "sous-commande « {autre} » inconnue. \
             Attendu : list, modes, set, paint, fans, fan, curve."
        )),
    }
}

fn parse_curve(mut args: std::vec::IntoIter<String>) -> Result<Command, String> {
    let mut canal: Option<String> = None;
    let mut points: Vec<(usize, Percent)> = Vec::new();
    let mut force = false;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--force" => force = true,
            "--channel" => {
                canal = Some(
                    args.next()
                        .ok_or_else(|| "« --channel » attend un nom de canal.".to_owned())?,
                );
            }
            "--point" => {
                let brut = args
                    .next()
                    .ok_or_else(|| "« --point » attend « POINT:CONSIGNE ».".to_owned())?;
                points.push(parse_point(&brut)?);
            }
            autre => return Err(format!("option « {autre} » inconnue.")),
        }
    }

    let canal = canal.ok_or_else(|| "« --channel » est obligatoire.".to_owned())?;
    if points.is_empty() {
        return Err("préciser au moins un « --point POINT:CONSIGNE ».".to_owned());
    }

    Ok(Command::Curve {
        canal,
        points,
        force,
    })
}

/// Analyse un couple « POINT:CONSIGNE ».
fn parse_point(brut: &str) -> Result<(usize, Percent), String> {
    let (point, consigne) = brut.split_once(':').ok_or_else(|| {
        format!("« {brut} » : attendu « POINT:CONSIGNE », par exemple « 11:50 ».")
    })?;

    let point: usize = point
        .trim()
        .parse()
        .map_err(|_| format!("point « {point} » invalide : attendu un entier."))?;
    let consigne: u8 = consigne
        .trim()
        .parse()
        .map_err(|_| format!("consigne « {consigne} » invalide : attendu un entier de 0 à 100."))?;

    Ok((point, Percent::new(consigne).map_err(|e| e.to_string())?))
}

fn parse_fan(mut args: std::vec::IntoIter<String>) -> Result<Command, String> {
    let mut tous = false;
    let mut canal: Option<String> = None;
    let mut pwm: Option<u8> = None;
    let mut auto = false;
    let mut curve = false;
    let mut force = false;
    let mut manual = false;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--all" => tous = true,
            "--auto" => auto = true,
            "--curve" => curve = true,
            "--force" => force = true,
            "--manual" => manual = true,
            "--channel" => {
                canal = Some(
                    args.next()
                        .ok_or_else(|| "« --channel » attend un nom de canal.".to_owned())?,
                );
            }
            "--pwm" => {
                let brut = args
                    .next()
                    .ok_or_else(|| "« --pwm » attend un pourcentage.".to_owned())?;
                pwm = Some(brut.parse().map_err(|_| {
                    format!("consigne « {brut} » invalide : attendu un entier de 0 à 100.")
                })?);
            }
            autre => return Err(format!("option « {autre} » inconnue.")),
        }
    }

    let canal = match (tous, canal) {
        (true, Some(_)) => return Err("« --all » et « --channel » s'excluent.".to_owned()),
        (false, None) => return Err("préciser « --all » ou « --channel <NOM> ».".to_owned()),
        (true, None) => CibleCanal::Tous,
        (false, Some(nom)) => CibleCanal::Un(nom),
    };

    if auto && curve {
        return Err("« --auto » et « --curve » s'excluent.".to_owned());
    }

    let action = match (auto || curve, pwm) {
        (true, Some(_)) => {
            return Err("« --auto » et « --curve » changent le mode : pas de consigne.".to_owned());
        }
        (true, None) if auto => ActionVentilateur::Auto,
        (true, None) => ActionVentilateur::Curve,
        (false, None) => {
            return Err("préciser « --pwm <0-100> », « --curve » ou « --auto ».".to_owned());
        }
        (false, Some(valeur)) => ActionVentilateur::Consigne {
            percent: Percent::new(valeur).map_err(|e| e.to_string())?,
            force,
            manual,
        },
    };

    Ok(Command::Fan { canal, action })
}

/// Résout `--all` / `--fan <NOM>`, communs à `set` et `paint`.
fn cible_de(tous: bool, fan: Option<String>) -> Result<Cible, String> {
    match (tous, fan) {
        (true, Some(_)) => Err("« --all » et « --fan » s'excluent.".to_owned()),
        (false, None) => Err("préciser « --all » ou « --fan <NOM> ».".to_owned()),
        (true, None) => Ok(Cible::Tous),
        (false, Some(nom)) => Position::from_name(&nom)
            .map(Cible::Une)
            .map_err(|e| e.to_string()),
    }
}

fn parse_paint(mut args: std::vec::IntoIter<String>) -> Result<Command, String> {
    let mut tous = false;
    let mut fan: Option<String> = None;
    let mut colors: Option<Vec<Rgb>> = None;
    let mut animate = false;
    let mut speed: Option<u16> = None;
    let mut brightness = Brightness::FULL;
    let mut skip_init = false;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--all" => tous = true,
            "--skip-init" => skip_init = true,
            "--animate" => animate = true,
            "--fan" => {
                fan = Some(
                    args.next()
                        .ok_or_else(|| "« --fan » attend un nom de position.".to_owned())?,
                );
            }
            "--colors" => {
                let brut = args.next().ok_or_else(|| {
                    "« --colors » attend des couleurs séparées par des virgules.".to_owned()
                })?;
                colors = Some(
                    brut.split(',')
                        .map(|c| Rgb::from_hex(c.trim()).map_err(|e| e.to_string()))
                        .collect::<Result<Vec<Rgb>, String>>()?,
                );
            }
            "--speed" => {
                let brut = args
                    .next()
                    .ok_or_else(|| "« --speed » attend une valeur.".to_owned())?;
                speed = Some(brut.parse().map_err(|_| {
                    format!("vitesse « {brut} » invalide : attendu un entier de 0 à 65535.")
                })?);
            }
            "--brightness" => brightness = parse_brightness(args.next())?,
            autre => return Err(format!("option « {autre} » inconnue.")),
        }
    }

    let cible = cible_de(tous, fan)?;
    let colors = colors.ok_or_else(|| "« --colors » est obligatoire.".to_owned())?;

    // La vitesse n'existe qu'en animé — la capture montre `00 00` en statique
    // (spec §5.2). Refuser plutôt qu'ignorer : une option acceptée sans effet
    // laisse croire qu'elle en a un.
    let apply = match (animate, speed) {
        (false, Some(_)) => {
            return Err("« --speed » n'a de sens qu'avec « --animate ».".to_owned());
        }
        (false, None) => Apply::Static,
        (true, valeur) => Apply::Animated {
            speed: valeur.unwrap_or(Apply::OBSERVED_SPEED),
        },
    };

    Ok(Command::Paint {
        cible,
        colors,
        apply,
        brightness,
        skip_init,
    })
}

fn parse_brightness(valeur: Option<String>) -> Result<Brightness, String> {
    let brut = valeur.ok_or_else(|| "« --brightness » attend un pourcentage.".to_owned())?;
    let pourcent: u8 = brut
        .parse()
        .map_err(|_| format!("luminosité « {brut} » invalide : attendu un entier de 0 à 100."))?;
    if pourcent > 100 {
        return Err(format!(
            "luminosité {pourcent} hors bornes : attendu 0 à 100."
        ));
    }
    Ok(Brightness::new(pourcent))
}

fn parse_set(mut args: std::vec::IntoIter<String>) -> Result<Command, String> {
    let mut tous = false;
    let mut fan: Option<String> = None;
    let mut colors: Vec<Rgb> = Vec::new();
    let mut mode = Mode::FIXED;
    let mut speed: Option<u8> = None;
    let mut brightness = Brightness::FULL;
    let mut skip_init = false;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--all" => tous = true,
            "--skip-init" => skip_init = true,
            "--fan" => {
                fan = Some(
                    args.next()
                        .ok_or_else(|| "« --fan » attend un nom de position.".to_owned())?,
                );
            }
            "--mode" => {
                let brut = args
                    .next()
                    .ok_or_else(|| "« --mode » attend un nom ou un numéro.".to_owned())?;
                mode = Mode::from_name(&brut).map_err(|e| e.to_string())?;
            }
            "--speed" => {
                let brut = args
                    .next()
                    .ok_or_else(|| "« --speed » attend une valeur.".to_owned())?;
                speed = Some(brut.parse().map_err(|_| {
                    format!("vitesse « {brut} » invalide : attendu un entier de 0 à 255.")
                })?);
            }
            "--color" => {
                let brut = args
                    .next()
                    .ok_or_else(|| "« --color » attend une couleur.".to_owned())?;
                colors.push(Rgb::from_hex(&brut).map_err(|e| e.to_string())?);
            }
            "--brightness" => brightness = parse_brightness(args.next())?,
            autre => return Err(format!("option « {autre} » inconnue.")),
        }
    }

    let cible = cible_de(tous, fan)?;

    // Le nombre de couleurs attendu est une contrainte du protocole : la règle
    // vit dans `reverb-proto` et n'est pas réécrite ici. On l'interroge tôt
    // pour refuser la demande avant d'ouvrir le moindre `/dev/hidraw*`.
    mode.check_colors(colors.len())
        .map_err(|e| format!("{e}. Chaque couleur se donne par « --color <HEX> »."))?;

    Ok(Command::Set {
        cible,
        mode,
        colors,
        speed: speed.unwrap_or(mode.default_speed()),
        brightness,
        skip_init,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Huit couleurs valides, forme la plus courte à écrire dans un test.
    const HUIT: &str = "010203,040506,070809,0a0b0c,0d0e0f,101112,131415,161718";

    #[test]
    fn analyse_la_sous_commande_list() {
        assert_eq!(parse(["list"]), Ok(Command::List));
    }

    #[test]
    fn analyse_la_sous_commande_fans() {
        assert_eq!(parse(["fans"]), Ok(Command::Fans));
    }

    #[test]
    fn analyse_une_consigne_sur_un_canal() {
        assert_eq!(
            parse(["fan", "--channel", "nzxtsmart2:fan-1", "--pwm", "60"]),
            Ok(Command::Fan {
                canal: CibleCanal::Un("nzxtsmart2:fan-1".to_owned()),
                action: ActionVentilateur::Consigne {
                    percent: Percent::new(60).unwrap(),
                    force: false,
                    manual: false,
                },
            })
        );
    }

    #[test]
    fn analyse_le_retour_a_la_courbe_firmware() {
        assert_eq!(
            parse(["fan", "--all", "--auto"]),
            Ok(Command::Fan {
                canal: CibleCanal::Tous,
                action: ActionVentilateur::Auto,
            })
        );
    }

    #[test]
    fn transmet_force_et_manual() {
        let Ok(Command::Fan { action, .. }) =
            parse(["fan", "--all", "--pwm", "5", "--force", "--manual"])
        else {
            panic!("doit être une commande fan");
        };
        assert_eq!(
            action,
            ActionVentilateur::Consigne {
                percent: Percent::new(5).unwrap(),
                force: true,
                manual: true,
            }
        );
    }

    #[test]
    fn refuse_une_consigne_au_dessus_de_cent() {
        let erreur = parse(["fan", "--all", "--pwm", "150"]).expect_err("doit être refusé");
        assert!(erreur.contains("150"), "message : {erreur}");
    }

    #[test]
    fn refuse_auto_et_pwm_ensemble() {
        // « --auto » rend le canal à sa courbe : une consigne fixe le
        // contredirait dans la même commande.
        let erreur =
            parse(["fan", "--all", "--auto", "--pwm", "50"]).expect_err("doit être refusé");
        assert!(erreur.contains("--auto"), "message : {erreur}");
    }

    #[test]
    fn refuse_une_commande_fan_sans_action() {
        let erreur = parse(["fan", "--all"]).expect_err("doit être refusé");
        assert!(erreur.contains("--pwm"), "message : {erreur}");
        assert!(erreur.contains("--auto"), "message : {erreur}");
    }

    #[test]
    fn refuse_all_et_channel_ensemble_pour_un_ventilateur() {
        let erreur =
            parse(["fan", "--all", "--channel", "x", "--pwm", "50"]).expect_err("doit être refusé");
        assert!(erreur.contains("s'excluent"), "message : {erreur}");
    }

    #[test]
    fn analyse_une_peinture_led_par_led() {
        let Ok(Command::Paint {
            cible,
            colors,
            apply,
            ..
        }) = parse(["paint", "--fan", "radiateur haut", "--colors", HUIT])
        else {
            panic!("doit être une commande paint");
        };
        assert_eq!(cible, Cible::Une(Position::RadiateurHaut));
        assert_eq!(colors.len(), 8);
        assert_eq!(colors[0], Rgb::new(0x01, 0x02, 0x03));
        assert_eq!(colors[7], Rgb::new(0x16, 0x17, 0x18));
        assert_eq!(apply, Apply::Static);
    }

    #[test]
    fn la_peinture_accepte_des_espaces_autour_des_virgules() {
        let Ok(Command::Paint { colors, .. }) = parse([
            "paint",
            "--all",
            "--colors",
            "ff0000, 00ff00 ,0000ff,ffffff,000000,ff00ff,00ffff,ffff00",
        ]) else {
            panic!("doit être une commande paint");
        };
        assert_eq!(colors.len(), 8);
    }

    #[test]
    fn animate_prend_la_vitesse_observee_par_defaut() {
        let Ok(Command::Paint { apply, .. }) =
            parse(["paint", "--all", "--colors", HUIT, "--animate"])
        else {
            panic!("doit être une commande paint");
        };
        assert_eq!(
            apply,
            Apply::Animated {
                speed: Apply::OBSERVED_SPEED
            }
        );
    }

    #[test]
    fn animate_accepte_une_vitesse_sur_seize_bits() {
        let Ok(Command::Paint { apply, .. }) = parse([
            "paint",
            "--all",
            "--colors",
            HUIT,
            "--animate",
            "--speed",
            "4660",
        ]) else {
            panic!("doit être une commande paint");
        };
        assert_eq!(apply, Apply::Animated { speed: 0x1234 });
    }

    #[test]
    fn refuse_une_vitesse_sans_animate() {
        // La capture montre `00 00` en statique (spec §5.2) : accepter l'option
        // sans effet laisserait croire qu'elle en a un.
        let erreur = parse(["paint", "--all", "--colors", HUIT, "--speed", "100"])
            .expect_err("doit être refusé");
        assert!(erreur.contains("--animate"), "message : {erreur}");
    }

    #[test]
    fn refuse_une_peinture_sans_couleurs() {
        let erreur = parse(["paint", "--all"]).expect_err("doit être refusé");
        assert!(erreur.contains("--colors"), "message : {erreur}");
    }

    #[test]
    fn refuse_une_couleur_invalide_dans_la_liste() {
        let erreur = parse([
            "paint",
            "--all",
            "--colors",
            "ff0000,pasunecouleur,0000ff,ffffff,000000,ff00ff,00ffff,ffff00",
        ])
        .expect_err("doit être refusé");
        assert!(erreur.contains("pasunecouleur"), "message : {erreur}");
    }

    #[test]
    fn la_peinture_partage_les_options_communes_de_set() {
        let Ok(Command::Paint {
            brightness,
            skip_init,
            ..
        }) = parse([
            "paint",
            "--all",
            "--colors",
            HUIT,
            "--brightness",
            "40",
            "--skip-init",
        ])
        else {
            panic!("doit être une commande paint");
        };
        assert_eq!(brightness.percent(), 40);
        assert!(skip_init);
    }

    #[test]
    fn la_peinture_refuse_all_et_fan_ensemble() {
        let erreur = parse(["paint", "--all", "--fan", "gauche", "--colors", HUIT])
            .expect_err("doit être refusé");
        assert!(erreur.contains("s'excluent"), "message : {erreur}");
    }

    #[test]
    fn analyse_une_couleur_sur_tous_les_ventilateurs() {
        // Sans « --mode », la commande de l'issue #1 est inchangée : mode fixe,
        // une couleur, la vitesse du mode.
        let attendu = Command::Set {
            cible: Cible::Tous,
            mode: Mode::FIXED,
            colors: vec![Rgb::new(0xff, 0x00, 0xff)],
            speed: 0x32,
            brightness: Brightness::FULL,
            skip_init: false,
        };
        assert_eq!(parse(["set", "--all", "--color", "ff00ff"]), Ok(attendu));
    }

    #[test]
    fn analyse_la_sous_commande_modes() {
        assert_eq!(parse(["modes"]), Ok(Command::Modes));
    }

    #[test]
    fn analyse_un_mode_par_son_nom() {
        let Ok(Command::Set { mode, colors, .. }) =
            parse(["set", "--all", "--mode", "spectrum-wave"])
        else {
            panic!("doit être une commande set");
        };
        assert_eq!(mode, Mode::SPECTRUM_WAVE);
        assert!(colors.is_empty(), "ce mode n'attend aucune couleur");
    }

    #[test]
    fn analyse_un_mode_par_son_numero() {
        let Ok(Command::Set { mode, .. }) = parse([
            "set", "--all", "--mode", "5", "--color", "ff0000", "--color", "0000ff",
        ]) else {
            panic!("doit être une commande set");
        };
        assert_eq!(mode, Mode::ALTERNATING);
    }

    #[test]
    fn conserve_l_ordre_des_couleurs_repetees() {
        let Ok(Command::Set { colors, .. }) = parse([
            "set",
            "--all",
            "--mode",
            "alternating",
            "--color",
            "ff0000",
            "--color",
            "0000ff",
        ]) else {
            panic!("doit être une commande set");
        };
        assert_eq!(colors, vec![Rgb::new(0xff, 0, 0), Rgb::new(0, 0, 0xff)]);
    }

    #[test]
    fn la_vitesse_par_defaut_est_celle_du_mode() {
        let Ok(Command::Set { speed, .. }) =
            parse(["set", "--all", "--mode", "breathing", "--color", "ff0000"])
        else {
            panic!("doit être une commande set");
        };
        assert_eq!(speed, Mode::BREATHING.default_speed());
    }

    #[test]
    fn la_vitesse_explicite_prime() {
        let Ok(Command::Set { speed, .. }) = parse([
            "set",
            "--all",
            "--mode",
            "breathing",
            "--color",
            "ff0000",
            "--speed",
            "20",
        ]) else {
            panic!("doit être une commande set");
        };
        assert_eq!(speed, 20);
    }

    #[test]
    fn refuse_une_vitesse_hors_bornes() {
        let erreur = parse(["set", "--all", "--color", "ffffff", "--speed", "300"])
            .expect_err("doit être refusé");
        assert!(erreur.contains("0 à 255"), "message : {erreur}");
    }

    #[test]
    fn refuse_un_mode_inconnu_en_listant_les_valides() {
        let erreur =
            parse(["set", "--all", "--mode", "arc-en-ciel"]).expect_err("doit être refusé");
        assert!(erreur.contains("arc-en-ciel"), "message : {erreur}");
        assert!(erreur.contains("breathing"), "doit lister : {erreur}");
    }

    #[test]
    fn refuse_un_nombre_de_couleurs_incorrect_avant_tout_acces_materiel() {
        let erreur = parse(["set", "--all", "--mode", "alternating", "--color", "ff0000"])
            .expect_err("alternating exige exactement deux couleurs");
        assert!(erreur.contains("alternating"), "message : {erreur}");

        let erreur = parse([
            "set",
            "--all",
            "--mode",
            "spectrum-wave",
            "--color",
            "ff0000",
        ])
        .expect_err("spectrum-wave n'attend aucune couleur");
        assert!(erreur.contains("spectrum-wave"), "message : {erreur}");
    }

    #[test]
    fn analyse_une_position_nommee() {
        let Ok(Command::Set { cible, .. }) =
            parse(["set", "--fan", "radiateur bas", "--color", "0f0f0f"])
        else {
            panic!("doit être une commande set");
        };
        assert_eq!(cible, Cible::Une(Position::RadiateurBas));
    }

    #[test]
    fn analyse_la_luminosite_et_le_saut_d_initialisation() {
        let Ok(Command::Set {
            brightness,
            skip_init,
            ..
        }) = parse([
            "set",
            "--all",
            "--color",
            "ffffff",
            "--brightness",
            "30",
            "--skip-init",
        ])
        else {
            panic!("doit être une commande set");
        };
        assert_eq!(brightness.percent(), 30);
        assert!(skip_init);
    }

    #[test]
    fn refuse_all_et_fan_ensemble() {
        let erreur = parse(["set", "--all", "--fan", "gauche", "--color", "ffffff"])
            .expect_err("doit être refusé");
        assert!(erreur.contains("s'excluent"), "message : {erreur}");
    }

    #[test]
    fn refuse_une_cible_absente() {
        let erreur = parse(["set", "--color", "ffffff"]).expect_err("doit être refusé");
        assert!(erreur.contains("--all"), "message : {erreur}");
    }

    #[test]
    fn refuse_une_couleur_absente() {
        let erreur = parse(["set", "--all"]).expect_err("doit être refusé");
        assert!(erreur.contains("--color"), "message : {erreur}");
    }

    #[test]
    fn refuse_une_position_inconnue_en_listant_les_valides() {
        let erreur =
            parse(["set", "--fan", "plafond", "--color", "ffffff"]).expect_err("doit être refusé");
        assert!(erreur.contains("plafond"), "message : {erreur}");
        assert!(
            erreur.contains("radiateur bas"),
            "doit lister les valides : {erreur}"
        );
    }

    #[test]
    fn refuse_une_luminosite_hors_bornes() {
        let erreur = parse(["set", "--all", "--color", "ffffff", "--brightness", "150"])
            .expect_err("doit être refusé");
        assert!(erreur.contains("hors bornes"), "message : {erreur}");
    }

    #[test]
    fn refuse_une_option_inconnue() {
        let erreur =
            parse(["set", "--all", "--color", "ffffff", "--turbo"]).expect_err("doit être refusé");
        assert!(erreur.contains("--turbo"), "message : {erreur}");
    }

    #[test]
    fn refuse_une_sous_commande_inconnue() {
        assert!(parse(["danse"]).is_err());
        assert!(parse(Vec::<String>::new()).is_err());
    }
}
