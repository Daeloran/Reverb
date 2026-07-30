//! Vitesse des ventilateurs, par l'interface hwmon du noyau.
//!
//! Contrairement à l'éclairage, la vitesse ne passe **pas** par des trames HID.
//! Les pilotes `nzxt_smart2` et `nzxt_kraken3` exposent déjà les consignes en
//! sysfs ; la commande `0x62 0x01` de la spec §6 ferait doublon, et n'atteindrait
//! de toute façon que trois des cinq canaux. Voir `docs/VENTILATEURS.md`.
//!
//! Écrire la même chose en HID brut donnerait **deux écrivains** sur le même
//! registre, chacun réémettant sa consigne périodiquement.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Un canal de vitesse découvert dans sysfs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FanChannel {
    /// Nom du pilote, lu dans le fichier `name` — « nzxtsmart2 ».
    pub source: String,
    /// Libellé du canal, lu dans `fanN_label` — « Pump speed ». À défaut `fanN`.
    pub label: String,
    /// Nom complet, celui que l'utilisateur tape — « kraken2023elite:pump-speed ».
    ///
    /// Toujours préfixé par la source : deux pilotes nomment volontiers un
    /// canal « FAN 1 », et un nom ambigu qui désigne le mauvais ventilateur est
    /// pire que verbeux.
    pub name: String,
    pub pwm: PathBuf,
    /// `fanN_input`, absent si le canal ne remonte aucun régime.
    pub tach: Option<PathBuf>,
    /// `pwmN_enable`, absent si la source n'expose pas de mode — c'est le cas
    /// de `nct6687`.
    pub enable: Option<PathBuf>,
}

impl FanChannel {
    /// Lit le mode courant du canal.
    ///
    /// Faillible : c'est un accès disque, et le fichier peut disparaître si le
    /// périphérique est débranché entre la découverte et la lecture.
    pub fn mode(&self) -> io::Result<Mode> {
        let Some(chemin) = &self.enable else {
            return Ok(Mode::Unsupported);
        };
        let brut = fs::read_to_string(chemin)?;
        match brut.trim().parse::<u8>() {
            Ok(0) => Ok(Mode::FirmwareCurve),
            Ok(1) => Ok(Mode::Manual),
            Ok(autre) => Ok(Mode::Unknown(autre)),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} ne contient pas un entier", chemin.display()),
            )),
        }
    }
}

/// Ce que le canal fait de sa consigne, lu dans `pwmN_enable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `1` — la consigne écrite est appliquée telle quelle.
    Manual,
    /// `0` — le firmware pilote le canal, typiquement selon une courbe de
    /// température. Lui imposer une consigne fixe l'en sort.
    FirmwareCurve,
    /// Une autre valeur, dont on ne sait rien. On la lit, on ne la réécrit pas.
    Unknown(u8),
    /// La source n'expose pas `pwmN_enable`.
    Unsupported,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Manual => write!(f, "manuel"),
            Mode::FirmwareCurve => write!(f, "courbe firmware"),
            Mode::Unknown(valeur) => write!(f, "inconnu ({valeur})"),
            Mode::Unsupported => write!(f, "non réglable"),
        }
    }
}

/// Consigne de vitesse, en pourcent.
///
/// Le noyau travaille sur `0..=255` ; l'utilisateur pense en pourcentage. La
/// conversion vit ici pour n'exister qu'une fois.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Percent(u8);

impl Percent {
    /// Plancher au-dessous duquel `reverb fan` refuse sans `--force`.
    ///
    /// La constante vit ici, mais la règle est appliquée par la commande :
    /// si le type refusait, `--force` n'aurait aucun moyen de s'exprimer.
    pub const FLOOR: u8 = 20;

    pub fn new(percent: u8) -> Result<Self, PercentError> {
        if percent > 100 {
            return Err(PercentError { given: percent });
        }
        Ok(Percent(percent))
    }

    pub fn percent(self) -> u8 {
        self.0
    }

    /// Valeur sur l'échelle du noyau, arrondie au plus proche.
    pub fn raw(self) -> u8 {
        ((u16::from(self.0) * 255 + 50) / 100) as u8
    }

    /// Pourcentage correspondant à une valeur brute, arrondi au plus proche.
    ///
    /// L'arrondi n'est pas cosmétique : le noyau rend `71` là où l'utilisateur
    /// a demandé 28 %, et une troncature afficherait 27 %.
    pub fn from_raw(raw: u8) -> Self {
        Percent(((u16::from(raw) * 100 + 127) / 255) as u8)
    }
}

/// Consigne au-dessus de 100 %.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PercentError {
    pub given: u8,
}

impl fmt::Display for PercentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "consigne de {} % hors bornes : attendu 0 à 100.",
            self.given
        )
    }
}

impl std::error::Error for PercentError {}

/// Met un libellé en kebab-case ASCII : « Pump speed » devient « pump-speed ».
pub fn slug(label: &str) -> String {
    let mut sortie = String::with_capacity(label.len());
    let mut tiret_en_attente = false;

    for caractere in label.chars() {
        if caractere.is_ascii_alphanumeric() {
            if tiret_en_attente && !sortie.is_empty() {
                sortie.push('-');
            }
            tiret_en_attente = false;
            sortie.push(caractere.to_ascii_lowercase());
        } else {
            // Une suite de séparateurs ne produit qu'un tiret, et seulement
            // s'il reste quelque chose derrière : pas de tiret en bord.
            tiret_en_attente = true;
        }
    }

    sortie
}

/// Découvre les canaux de vitesse sous une racine sysfs, triés par nom.
///
/// L'ordre de lecture d'un répertoire n'est pas déterministe ; celui de la
/// liste rendue doit l'être, sinon `reverb fans` change d'affichage d'un appel
/// à l'autre.
pub fn discover_in(sys_class: &Path) -> io::Result<Vec<FanChannel>> {
    let mut canaux = Vec::new();

    let entrees = match fs::read_dir(sys_class) {
        Ok(entrees) => entrees,
        // Une racine absente n'a rien découvert. Ce n'est pas une panne du
        // programme : `reverb fans` doit pouvoir en parler.
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(canaux),
        Err(e) => return Err(e),
    };

    for entree in entrees {
        let repertoire = entree?.path();
        let Ok(source) = fs::read_to_string(repertoire.join("name")) else {
            // Tout ce que le noyau met dans /sys/class/hwmon n'a pas de nom.
            continue;
        };
        let source = source.trim().to_owned();

        for index in indices_pwm(&repertoire)? {
            canaux.push(canal(&repertoire, &source, index));
        }
    }

    canaux.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(canaux)
}

/// Numéros des fichiers `pwmN` d'un répertoire, triés.
///
/// `pwm1_enable` et `pwm1_mode` sont des voisins de `pwm1`, pas des canaux.
fn indices_pwm(repertoire: &Path) -> io::Result<Vec<u32>> {
    let mut indices = Vec::new();

    for entree in fs::read_dir(repertoire)? {
        let nom = entree?.file_name();
        let Some(nom) = nom.to_str() else { continue };
        let Some(reste) = nom.strip_prefix("pwm") else {
            continue;
        };
        if let Ok(index) = reste.parse::<u32>() {
            indices.push(index);
        }
    }

    indices.sort_unstable();
    Ok(indices)
}

fn canal(repertoire: &Path, source: &str, index: u32) -> FanChannel {
    let tach = presence(repertoire.join(format!("fan{index}_input")));
    let enable = presence(repertoire.join(format!("pwm{index}_enable")));

    let label = fs::read_to_string(repertoire.join(format!("fan{index}_label")))
        .ok()
        .map(|brut| brut.trim().to_owned())
        .filter(|libelle| !libelle.is_empty())
        .unwrap_or_else(|| format!("fan{index}"));

    FanChannel {
        name: format!("{source}:{}", slug(&label)),
        source: source.to_owned(),
        label,
        pwm: repertoire.join(format!("pwm{index}")),
        tach,
        enable,
    }
}

fn presence(chemin: PathBuf) -> Option<PathBuf> {
    chemin.exists().then_some(chemin)
}

/// Écrit la consigne dans le `pwmN` du canal.
///
/// Primitive brute : elle n'inspecte ni le mode courant ni le plancher. Le
/// refus d'écrire sur un canal en courbe firmware appartient à la commande,
/// qui seule connaît `--manual` et peut expliquer ce que l'option ferait.
///
/// Le canal n'est pris que **par référence** : le seul chemin que cette
/// fonction peut ouvrir est celui que la découverte a listé.
pub fn set_pwm(channel: &FanChannel, percent: Percent) -> io::Result<()> {
    fs::write(&channel.pwm, percent.raw().to_string())
}

/// Écrit le mode dans le `pwmN_enable` du canal.
///
/// # Erreurs
///
/// Si le canal n'expose pas `pwmN_enable`, ou si le mode demandé est
/// [`Mode::Unknown`] : on ne réémet pas une valeur qu'on n'a pas comprise.
/// Dans les deux cas, rien n'est écrit.
pub fn set_mode(channel: &FanChannel, mode: Mode) -> io::Result<()> {
    let valeur = match mode {
        Mode::Manual => "1",
        Mode::FirmwareCurve => "0",
        Mode::Unknown(valeur) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("mode {valeur} inconnu : il se lit, il ne se réécrit pas"),
            ));
        }
        Mode::Unsupported => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "« non réglable » n'est pas un mode qu'on écrit",
            ));
        }
    };

    let Some(chemin) = &channel.enable else {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("le canal « {} » n'a pas de mode réglable", channel.name),
        ));
    };

    fs::write(chemin, valeur)
}

/// Découvre les canaux sous la racine sysfs réelle.
pub fn discover() -> io::Result<Vec<FanChannel>> {
    discover_in(Path::new("/sys/class/hwmon"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    /// Vrai si le chemin porte ce nom de fichier.
    fn se_termine_par(chemin: &Path, nom: &str) -> bool {
        chemin.file_name() == Some(OsStr::new(nom))
    }

    #[test]
    fn l_arrondi_reste_dans_les_bornes_sur_toute_la_plage() {
        for p in 0u8..=100 {
            let brut = Percent::new(p).unwrap().raw();
            let retour = Percent::from_raw(brut).percent();
            assert!(retour.abs_diff(p) <= 1, "{p} % relu à {retour} %");
        }
        for brut in 0u8..=255 {
            assert!(Percent::from_raw(brut).percent() <= 100);
        }
    }

    #[test]
    fn le_slug_ne_produit_jamais_de_tiret_en_bord() {
        for libelle in ["  FAN 1  ", "--pump--", "///", "a  b"] {
            let s = slug(libelle);
            assert!(!s.starts_with('-'), "« {libelle} » -> « {s} »");
            assert!(!s.ends_with('-'), "« {libelle} » -> « {s} »");
            assert!(!s.contains("--"), "« {libelle} » -> « {s} »");
        }
    }

    #[test]
    fn un_mode_inconnu_ne_s_ecrit_pas() {
        let canal = FanChannel {
            source: "test".to_owned(),
            label: "fan1".to_owned(),
            name: "test:fan1".to_owned(),
            pwm: PathBuf::from("/inexistant/pwm1"),
            tach: None,
            enable: Some(PathBuf::from("/inexistant/pwm1_enable")),
        };
        let erreur = set_mode(&canal, Mode::Unknown(2)).expect_err("doit refuser");
        assert_eq!(erreur.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn le_nom_du_fichier_pwm_est_celui_du_canal() {
        // Répertoire volontairement inexistant : ce test ne doit toucher ni
        // /sys ni le moindre fichier réel. Sans `fanN_label` lisible, le
        // libellé retombe sur « fanN ».
        let canal = canal(Path::new("/inexistant/hwmon4"), "nzxtsmart2", 2);
        assert!(se_termine_par(&canal.pwm, "pwm2"));
        assert_eq!(canal.label, "fan2");
        assert_eq!(canal.name, "nzxtsmart2:fan2");
        assert_eq!(canal.tach, None, "aucun fichier n'existe sous ce chemin");
    }
}
