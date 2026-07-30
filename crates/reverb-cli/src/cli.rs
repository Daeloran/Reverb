//! Analyse des arguments de la ligne de commande.
//!
//! Écrite à la main : quatre options ne justifient pas une dépendance, et le
//! binaire est un outil de validation interne, pas le produit final (le produit
//! sera un démon et une fenêtre).

use reverb_proto::{Brightness, Mode, Position, Rgb};

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
}

/// Quels ventilateurs sont visés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cible {
    Tous,
    Une(Position),
}

pub const USAGE: &str = "\
reverb — pilotage des contrôleurs RGB NZXT

USAGE :
    reverb list
    reverb modes
    reverb set --all       [OPTIONS]
    reverb set --fan <NOM> [OPTIONS]

OPTIONS :
    --mode <NOM|N>       mode d'animation, « fixed » par défaut (« reverb modes »)
    --color <HEX>        couleur, six chiffres hexadécimaux (ff00ff ou #ff00ff).
                         Répétable : certains modes attendent 2 ou 3 couleurs.
    --speed <0-255>      vitesse de l'animation, celle du mode par défaut.
                         ⚠️ échelle non calibrée : valeur brute du protocole.
    --brightness <N>     luminosité en pourcent, 100 par défaut
    --skip-init          n'envoie pas la séquence d'initialisation
    -h, --help           affiche cette aide

EXEMPLES :
    reverb set --all --color ff00ff
    reverb set --fan \"radiateur bas\" --color 00ff00
    reverb set --all --mode spectrum-wave
    reverb set --all --mode breathing --color ff0000 --speed 20
    reverb set --all --mode alternating --color ff0000 --color 0000ff
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
        autre => Err(format!(
            "sous-commande « {autre} » inconnue. Attendu : list, modes, set."
        )),
    }
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
            "--brightness" => {
                let brut = args
                    .next()
                    .ok_or_else(|| "« --brightness » attend un pourcentage.".to_owned())?;
                let valeur: u8 = brut.parse().map_err(|_| {
                    format!("luminosité « {brut} » invalide : attendu un entier de 0 à 100.")
                })?;
                if valeur > 100 {
                    return Err(format!(
                        "luminosité {valeur} hors bornes : attendu 0 à 100."
                    ));
                }
                brightness = Brightness::new(valeur);
            }
            autre => return Err(format!("option « {autre} » inconnue.")),
        }
    }

    let cible = match (tous, fan) {
        (true, Some(_)) => {
            return Err("« --all » et « --fan » s'excluent.".to_owned());
        }
        (false, None) => {
            return Err("préciser « --all » ou « --fan <NOM> ».".to_owned());
        }
        (true, None) => Cible::Tous,
        (false, Some(nom)) => Cible::Une(Position::from_name(&nom).map_err(|e| e.to_string())?),
    };

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

    #[test]
    fn analyse_la_sous_commande_list() {
        assert_eq!(parse(["list"]), Ok(Command::List));
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
