//! Vitesse des ventilateurs, par l'interface hwmon du noyau.
//!
//! Contrairement à l'éclairage, la vitesse ne passe **pas** par des trames HID.
//! Les pilotes `nzxt_smart2` et `nzxt_kraken3` exposent déjà les consignes en
//! sysfs ; la commande `0x62 0x01` de la spec §6 ferait doublon, et n'atteindrait
//! de toute façon que trois des cinq canaux. Voir `docs/VENTILATEURS.md`.
//!
//! Écrire la même chose en HID brut donnerait **deux écrivains** sur le même
//! registre, chacun réémettant sa consigne périodiquement.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use reverb_proto::ipc::{
    MODE_COURBE_DE_L_HOTE, MODE_INCONNU_PREFIXE, MODE_MANUEL, MODE_NON_PILOTE, MODE_NON_REGLABLE,
    MODE_PLEIN_REGIME,
};

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
    /// Numéro du canal dans sa source : le N de `pwmN`.
    pub index: u32,
    pub pwm: PathBuf,
    /// `fanN_input`, absent si le canal ne remonte aucun régime.
    pub tach: Option<PathBuf>,
    /// `pwmN_enable`, absent si la source n'expose pas de mode — c'est le cas
    /// de `nct6687`.
    pub enable: Option<PathBuf>,
    /// Les `CURVE_POINTS` fichiers `tempN_auto_pointM_pwm`, dans l'ordre des
    /// points. Vide si le canal n'a pas de courbe matérielle — seul le Kraken
    /// en expose.
    pub curve: Vec<PathBuf>,
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
            // ⚠️ `NonPilote` et non `PleinRegime` : un `0` **lu** ne dit pas ce
            // qu'un `0` **écrit** fait. Voir la documentation des deux
            // variantes (issue #101).
            Ok(0) => Ok(Mode::NonPilote),
            Ok(1) => Ok(Mode::Manual),
            Ok(2) => Ok(Mode::HostCurve),
            Ok(autre) => Ok(Mode::Unknown(autre)),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} ne contient pas un entier", chemin.display()),
            )),
        }
    }

    /// Ce canal sait-il rendre la main à une régulation automatique ?
    ///
    /// Répond depuis le **nom du pilote**, déjà lu par la découverte, et jamais
    /// par une tentative d'écriture : une tentative qui réussit là où il ne
    /// fallait pas envoie le canal à plein régime, en silence.
    ///
    /// ⚠️ **Liste d'autorisation, et non d'exclusion.** Un pilote dont on ne
    /// sait rien répond non. Les deux erreurs ne coûtent pas la même chose :
    /// cacher un bouton légitime se répare d'une ligne, le montrer à tort lance
    /// la pompe à fond.
    ///
    /// Le fondement est la lecture des deux pilotes du noyau (issue #50) :
    ///
    /// - `nzxt-kraken3` accepte `2`, « exécute la courbe du périphérique » ;
    /// - `nzxt-smart2` n'a **aucun** mode automatique — son `set_pwm_enable()`
    ///   ne réaccepte que la valeur déjà portée et rend `-EOPNOTSUPP` sinon.
    pub fn sait_faire_auto(&self) -> bool {
        // Sans `pwmN_enable`, il n'y a nulle part où écrire le `2` : le bouton
        // ne pourrait qu'échouer, quel que soit le pilote.
        self.enable.is_some() && PILOTES_AUTOMATIQUES.contains(&self.source.as_str())
    }
}

/// Les pilotes dont on a **lu** qu'ils exécutent une courbe sur le périphérique.
///
/// Une entrée de plus se gagne en lisant le pilote concerné, pas en essayant.
const PILOTES_AUTOMATIQUES: [&str; 1] = ["kraken2023elite"];

/// Les canaux sur lesquels une courbe a été téléversée depuis le démarrage.
///
/// ⚠️ **Cet état ne peut pas se relire sur le matériel.** Les fichiers
/// `tempN_auto_pointM_pwm` sont en **écriture seule** (`0200`,
/// `docs/VENTILATEURS.md`) : leur simple présence ne dit rien de leur contenu,
/// et une implémentation qui la prendrait pour une preuve relancerait
/// exactement l'incident du 2026-08-15. Le carnet vit donc côté hôte, et il
/// **repart vide à chaque démarrage** — on ne peut rien savoir de ce qu'un autre
/// outil a écrit avant nous (issue #97).
///
/// Il ne se remplit que par [`set_curve`], jamais à côté : « une courbe est
/// partie » devient ainsi structurel plutôt que disciplinaire. Un carnet qu'on
/// remplirait à la main se réoublierait au premier chemin de code ajouté — et le
/// défaut qu'il corrige est justement un défaut silencieux.
///
/// L'instance est détenue par le démon (`peripheriques.rs`) ; le type est
/// déclaré ici, là où part le refus, sinon `reverb-hw` dépendrait de
/// `reverb-daemon`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourbesPosees {
    /// Les noms complets des canaux ayant reçu une courbe — jamais un index de
    /// `pwmN` : deux contrôleurs ont tous les deux un canal 1.
    canaux: BTreeSet<String>,
}

impl CourbesPosees {
    /// Un carnet vide : un démon qui démarre ne sait rien de ce qu'un autre
    /// outil a écrit avant lui.
    pub fn vide() -> Self {
        CourbesPosees {
            canaux: BTreeSet::new(),
        }
    }

    /// Ce canal peut-il passer en automatique **maintenant** ?
    ///
    /// Deux conditions, et c'est un **et** : son pilote sait exécuter une courbe
    /// (issue #50) **et** une courbe y a été posée depuis le démarrage
    /// (issue #97). C'est la même question que celle du refus de [`set_mode`],
    /// posée sans rien écrire — la fenêtre la lit à chaque `status`, une fois
    /// par seconde, et une question qui coûterait une tentative allumerait les
    /// ventilateurs à fond dans le cas où elle réussit.
    pub fn autorise_auto(&self, canal: &FanChannel) -> bool {
        canal.sait_faire_auto() && self.canaux.contains(&canal.name)
    }

    /// Retient qu'une courbe **est partie** sur ce canal.
    ///
    /// Volontairement privée au module : seul [`set_curve`] l'appelle, et
    /// seulement après que les quarante points ont été écrits. Le carnet ne
    /// retient jamais ce qui a été *tenté*.
    fn retenir(&mut self, canal: &FanChannel) {
        self.canaux.insert(canal.name.clone());
    }
}

/// Ce que le canal fait de sa consigne, lu dans `pwmN_enable`.
///
/// ⚠️ **`0` n'a pas le même sens selon qu'on le lit ou qu'on l'écrit**, et c'est
/// la seule valeur du fichier dans ce cas. Deux variantes le portent donc :
/// [`Mode::NonPilote`] pour ce qu'un `0` lu établit, [`Mode::PleinRegime`] pour
/// ce qu'un `0` écrit provoque. Les confondre affichait « plein-régime-100% »
/// devant une pompe à 30 % (issue #101).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `1` — la consigne écrite est appliquée telle quelle.
    Manual,
    /// `0` **lu** — le pilote ne pilote pas ce canal.
    ///
    /// ⚠️ **Ce n'est pas « le canal est à 100 % »**, comme la lecture de `0` se
    /// traduisait jusqu'au 2026-08-15. `nzxt-kraken3` **n'écrit rien au probe** :
    /// son initialisation n'envoie que `set_interval` et `finish_init`, aucune
    /// consigne ni courbe. Le champ `mode` du pilote sort donc du `kzalloc` à
    /// `0`, et lire `0` sur un canal jamais touché ne dit qu'une chose — que
    /// personne côté hôte ne le pilote (issue #101).
    ///
    /// Ce que le canal fait alors, c'est le périphérique qui le décide, et la
    /// consigne relue dans `pwmN` le dit : 30 % pour un Kraken sur son profil
    /// d'usine, 100 % pour un Kraken à qui l'on vient d'écrire
    /// [`Mode::PleinRegime`]. Dans les deux cas « non piloté » reste vrai — la
    /// nuance est dans le pourcentage, pas dans le mode.
    ///
    /// Mesuré sur SHYNAEL le 2026-08-15 : `pwm1_enable = 0`, `pwm1 = 77`,
    /// pompe à 1357 tr/min, et un duty qui suivait la température par paliers
    /// — 89, 102, 115, 128, 153. Une régulation bien vivante, que le libellé
    /// d'avant annonçait à fond.
    NonPilote,
    /// `0` **écrit** — plein régime, sans régulation.
    ///
    /// ⚠️ **Ce n'est pas « laissé au firmware »**, comme cette variante s'est
    /// appelée jusqu'au 2026-08-02. `nzxt-kraken3` en fait
    /// `kraken3_write_fixed_duty(priv, 255, channel)` puis cesse de piloter :
    /// c'est 100 % et la barre lâchée, pas un retour à une courbe (issue #50).
    /// L'observation de `docs/VENTILATEURS.md` — « le Kraken s'y rabat sur du
    /// refroidissement maximal » — était donc exacte, et son explication à
    /// portée de main dans le pilote.
    ///
    /// ⚠️ **Elle ne sort jamais de [`FanChannel::mode`]** : une fois écrite, le
    /// fichier contient `0`, et un `0` relu ne se distingue pas de celui d'un
    /// canal jamais touché. C'est une **intention**, pas un état observable —
    /// et c'est bien l'aveu qu'il faut faire, plutôt que de deviner.
    ///
    /// La fenêtre ne le propose nulle part, et « auto » vise [`Mode::HostCurve`].
    PleinRegime,
    /// `2` — le firmware exécute la courbe téléversée par **l'hôte**.
    ///
    /// ⚠️ **Ce n'est pas « la courbe du périphérique », ni un retour au profil
    /// d'usine.** `nzxt-kraken3` en fait
    /// `kraken3_write_curve(priv, priv->channel_info[channel].pwm_points, channel)` :
    /// il pousse le tableau de points **détenu par le pilote**. Si aucune courbe
    /// n'y a jamais été téléversée, ce tableau est celui du `kzalloc` — **zéro
    /// partout** —, et le canal cesse de réguler.
    ///
    /// Mesuré sur SHYNAEL le 2026-08-15, après un « auto » posé sans courbe :
    /// `pwm1 = 0`, `pwm1_enable = 2`, pompe à 1910 tr/min, son plancher
    /// matériel. Aucune erreur rendue, le mode relu exactement celui demandé, et
    /// le refroidissement arrêté jusqu'au prochain arrêt complet (issue #97).
    ///
    /// Il n'existe **aucune** valeur de `pwmN_enable` qui rende le Kraken à son
    /// profil d'usine : seule une coupure d'alimentation le fait.
    ///
    /// D'où le refus de [`set_mode`] tant qu'aucune courbe n'a été posée, et le
    /// carnet [`CourbesPosees`] qui en tient la mémoire.
    HostCurve,
    /// Une autre valeur, dont on ne sait rien. On la lit, on ne la réécrit pas.
    Unknown(u8),
    /// La source n'expose pas `pwmN_enable`.
    Unsupported,
}

/// ⚠️ **Ce n'est pas une étiquette d'affichage, c'est un jeton de protocole.**
///
/// Le libellé traverse le socket dans le champ `mode` d'une ligne `chan`, et
/// depuis #50 ce champ n'est plus le dernier — le drapeau « sait faire auto » le
/// suit. C'est l'arité de la ligne qui distingue un démon d'avant d'un démon
/// d'après (six jetons ou sept) : un mode à espaces la rendrait indécidable.
///
/// Les libellés sont donc devenus des mots composés. C'est le prix du champ
/// gagné, et il se paie une fois : « courbe-de-l'hôte » se lit encore, là où
/// deviner l'arité d'une ligne ne se lit pas du tout.
///
/// ⚠️ **Les six graphies vivent dans [`reverb_proto::ipc`]**, avec les autres
/// jetons du format de fil, et non ici. La fenêtre les relit de l'autre côté du
/// socket sans dépendre de `reverb-hw`, et depuis #112 elle **décide** dessus :
/// `non-piloté` et `courbe-de-l'hôte` verrouillent la barre d'un canal qui
/// régule seul. Recopiées aux deux bouts, un renommage les ferait diverger en
/// silence — et la barre de la pompe se déverrouillerait sans que rien ne le
/// dise.
impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Manual => write!(f, "{MODE_MANUEL}"),
            Mode::NonPilote => write!(f, "{MODE_NON_PILOTE}"),
            Mode::PleinRegime => write!(f, "{MODE_PLEIN_REGIME}"),
            Mode::HostCurve => write!(f, "{MODE_COURBE_DE_L_HOTE}"),
            Mode::Unknown(valeur) => write!(f, "{MODE_INCONNU_PREFIXE}{valeur}"),
            Mode::Unsupported => write!(f, "{MODE_NON_REGLABLE}"),
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
        index,
        pwm: repertoire.join(format!("pwm{index}")),
        tach,
        enable,
        curve: courbe_de(repertoire, index),
    }
}

/// Chemins des points de courbe du canal, dans l'ordre, ou rien.
///
/// Tout ou rien : une courbe partielle n'aurait aucun sens, puisque le firmware
/// interpole entre les 40 points qu'il détient.
fn courbe_de(repertoire: &Path, index: u32) -> Vec<PathBuf> {
    let points: Vec<PathBuf> = (1..=CURVE_POINTS)
        .map(|point| repertoire.join(format!("temp{index}_auto_point{point}_pwm")))
        .collect();

    if points.iter().all(|p| p.exists()) {
        points
    } else {
        Vec::new()
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
/// Si le canal n'expose pas `pwmN_enable`, si le mode demandé est
/// [`Mode::Unknown`] — on ne réémet pas une valeur qu'on n'a pas comprise —, si
/// l'on demande [`Mode::HostCurve`] à un canal qui ne sait pas faire auto, ou si
/// on le demande à un canal qui n'a **reçu aucune courbe** depuis le démarrage.
/// Dans tous les cas, **rien n'est écrit**.
///
/// Les deux refus d'`HostCurve` sont produits ici et non laissés au noyau, qui
/// n'a rien à en dire : `nzxt-smart2` rend `-EOPNOTSUPP`, que l'utilisateur
/// lisait sous la forme « Operation not supported (os error 95) » (issue #50), et
/// `nzxt-kraken3` **accepte sans broncher** un `2` sans courbe, en arrêtant la
/// régulation (issue #97). Un errno nu n'explique rien ; un succès qui casse le
/// refroidissement explique encore moins.
pub fn set_mode(channel: &FanChannel, mode: Mode, posees: &CourbesPosees) -> io::Result<()> {
    // ⚠️ Avant le `match`, et donc avant toute écriture — y compris quand le
    // fichier porte déjà `2`. Un no-op silencieux dans ce cas ferait dépendre le
    // refus de l'état courant, alors qu'il dépend du matériel. Et l'état
    // dangereux — un canal trouvé en `2` sans courbe — serait justement le seul
    // que le court-circuit rendrait irréparable.
    if mode == Mode::HostCurve && !channel.sait_faire_auto() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "le contrôleur « {} » n'a pas de mode automatique : le canal « {} » garde la \
                 consigne que l'hôte lui écrit",
                channel.source, channel.name
            ),
        ));
    }

    // ⚠️ **Après le refus du pilote, jamais avant.** Les deux conditions sont un
    // « et » (voir [`CourbesPosees::autorise_auto`]), et c'est le pilote qui
    // l'emporte : envoyer poser une courbe sur un canal qui n'exécutera jamais
    // de `2` serait un faux diagnostic.
    if mode == Mode::HostCurve && !posees.autorise_auto(channel) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "aucune courbe n'a été téléversée sur le canal « {} » depuis le démarrage : « 2 » \
                 y pousserait le tableau du pilote, à zéro partout, et arrêterait la régulation. \
                 Posez-la d'abord — « reverb curve --channel {} --point … ».",
                channel.name, channel.name
            ),
        ));
    }

    let valeur = match mode {
        Mode::Manual => "1",
        // Les deux s'écrivent `0`, et c'est le fait matériel : le fichier n'a
        // qu'une valeur pour les deux sens. Réécrire le mode qu'on vient de
        // lire ne doit pas changer le canal, d'où `NonPilote` acceptée ici
        // plutôt que refusée (issue #101).
        Mode::NonPilote | Mode::PleinRegime => "0",
        Mode::HostCurve => "2",
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

/// Nombre de points d'une courbe matérielle du Kraken.
pub const CURVE_POINTS: usize = 40;

/// Une courbe validée, prête à écrire : une consigne par point.
///
/// ⚠️ Les fichiers de courbe sont en **écriture seule** (`0200`). Une courbe se
/// pose, elle ne se relit jamais — d'où l'absence de toute fonction de lecture
/// et de tout état conservé côté hôte : il mentirait dès qu'un autre outil
/// écrirait, sans le moindre moyen de le détecter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Curve([Percent; CURVE_POINTS]);

impl Curve {
    /// Interpole linéairement une courbe depuis quelques couples
    /// `(index de point, consigne)`, donnés dans n'importe quel ordre.
    ///
    /// Avant le premier point, la valeur du premier ; après le dernier, celle
    /// du dernier. Les index sont ceux des fichiers sysfs : de 1 à
    /// [`CURVE_POINTS`].
    ///
    /// # Erreurs
    ///
    /// Voir [`CurveError`]. En particulier, une courbe qui **descend** est
    /// refusée : une consigne qui baisse quand la température monte est une
    /// faute de frappe ou un décalage d'indice, et l'écriture seule rend
    /// l'erreur invisible jusqu'à la surchauffe.
    pub fn interpolate(points: &[(usize, Percent)]) -> Result<Self, CurveError> {
        if points.is_empty() {
            return Err(CurveError::Empty);
        }

        let mut tries: Vec<(usize, Percent)> = Vec::with_capacity(points.len());
        for &(index, consigne) in points {
            if !(1..=CURVE_POINTS).contains(&index) {
                return Err(CurveError::OutOfRange { given: index });
            }
            match tries.iter().find(|(deja, _)| *deja == index) {
                Some(&(_, precedente)) if precedente != consigne => {
                    return Err(CurveError::Conflicting { at: index });
                }
                Some(_) => continue,
                None => tries.push((index, consigne)),
            }
        }
        tries.sort_by_key(|(index, _)| *index);

        for paire in tries.windows(2) {
            let (depuis, vers) = (paire[0], paire[1]);
            if vers.1 < depuis.1 {
                return Err(CurveError::Decreasing {
                    from: depuis,
                    to: vers,
                });
            }
        }

        let mut valeurs = [Percent(0); CURVE_POINTS];
        for (rang, valeur) in valeurs.iter_mut().enumerate() {
            let point = rang + 1;
            *valeur = interpole(&tries, point);
        }

        Ok(Curve(valeurs))
    }

    /// Les consignes, dans l'ordre des points.
    pub fn points(&self) -> &[Percent; CURVE_POINTS] {
        &self.0
    }
}

/// Consigne au point `point`, par interpolation entre les couples donnés.
///
/// `tries` est trié par index, non vide, et sans doublon d'index.
fn interpole(tries: &[(usize, Percent)], point: usize) -> Percent {
    let (premier_index, premiere) = tries[0];
    if point <= premier_index {
        return premiere;
    }

    let (dernier_index, derniere) = tries[tries.len() - 1];
    if point >= dernier_index {
        return derniere;
    }

    // Le segment qui encadre le point. Les bornes plates sont déjà écartées.
    let segment = tries
        .windows(2)
        .find(|paire| paire[0].0 <= point && point <= paire[1].0)
        .expect("un point strictement entre les bornes tombe dans un segment");

    let ((gauche, basse), (droite, haute)) = (segment[0], segment[1]);
    let largeur = droite - gauche;
    let hauteur = u32::from(haute.percent() - basse.percent());
    let avance = point - gauche;

    // Arrondi au plus proche, pour ne pas introduire de palier descendant.
    let pourcent = u32::from(basse.percent())
        + (hauteur * avance as u32 + (largeur as u32) / 2) / largeur as u32;
    Percent(pourcent as u8)
}

/// Ce qui peut clocher dans une demande de courbe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurveError {
    /// Aucun point fourni.
    Empty,
    /// Un index hors de `1..=CURVE_POINTS`.
    OutOfRange { given: usize },
    /// La consigne baisse quand la température monte.
    Decreasing {
        from: (usize, Percent),
        to: (usize, Percent),
    },
    /// Deux points portent le même index avec des consignes différentes.
    Conflicting { at: usize },
}

impl fmt::Display for CurveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CurveError::Empty => write!(
                f,
                "aucun point fourni : une courbe demande au moins un couple point:consigne."
            ),
            CurveError::OutOfRange { given } => write!(
                f,
                "point {given} hors de la courbe : attendu 1 à {CURVE_POINTS}."
            ),
            CurveError::Decreasing { from, to } => write!(
                f,
                "la consigne descend de {} % au point {} à {} % au point {} : \
                 une courbe de refroidissement ne descend pas.",
                from.1.percent(),
                from.0,
                to.1.percent(),
                to.0
            ),
            CurveError::Conflicting { at } => write!(
                f,
                "deux consignes différentes au point {at} : laquelle faut-il retenir ?"
            ),
        }
    }
}

impl std::error::Error for CurveError {}

/// Écrit les points de la courbe du canal.
///
/// Ne touche **pas** `pwmN_enable` : basculer un canal en mode courbe est un
/// geste distinct, qui change durablement son comportement thermique. Le faire
/// au passage d'une écriture de courbe serait exactement l'effet de bord que le
/// projet s'interdit.
///
/// Le carnet `posees` **retient ce qui est parti**, et lui seul ouvre ensuite
/// [`Mode::HostCurve`] sur ce canal (issue #97). Il traverse cette fonction
/// plutôt que de se remplir à côté : on ne peut donc pas écrire une courbe sans
/// que le carnet l'apprenne, ni lui faire croire qu'une courbe est partie alors
/// qu'elle a échoué.
///
/// # Erreurs
///
/// Si le canal n'a pas de courbe matérielle — rien n'est alors écrit.
pub fn set_curve(
    channel: &FanChannel,
    curve: &Curve,
    posees: &mut CourbesPosees,
) -> io::Result<()> {
    if channel.curve.len() != CURVE_POINTS {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "le canal « {} » n'a pas de courbe matérielle : \
                 seul le Kraken en expose une.",
                channel.name
            ),
        ));
    }

    for (chemin, consigne) in channel.curve.iter().zip(curve.points()) {
        fs::write(chemin, consigne.raw().to_string())?;
    }

    // Après l'écriture seulement : une courbe qui n'est pas partie ne doit pas
    // ouvrir « auto », sinon une tentative ratée rouvrirait la porte que #97
    // ferme.
    posees.retenir(channel);
    Ok(())
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
            index: 1,
            pwm: PathBuf::from("/inexistant/pwm1"),
            tach: None,
            enable: Some(PathBuf::from("/inexistant/pwm1_enable")),
            curve: Vec::new(),
        };
        let erreur =
            set_mode(&canal, Mode::Unknown(2), &CourbesPosees::vide()).expect_err("doit refuser");
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

/// Une sonde de température de la machine.
///
/// Toutes les sondes, pas seulement celles qui portent un ventilateur de
/// Reverb : le CPU, les NVMe, le chipset, les barrettes de RAM et la carte
/// réseau en ont, et ce sont elles que l'utilisateur regarde.
///
/// ⚠️ **Lecture seule, et par `sysfs` uniquement.** Aucune sonde ne s'interroge
/// sur un bus I²C : un scan en lecture seule a déjà altéré l'éclairage par
/// défaut de la RAM sur cette machine. Les sondes `spd5118` des barrettes se
/// lisent ici par le pilote du noyau, qui détient le bus — pas un octet n'est
/// émis par Reverb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sonde {
    /// Identifiant stable : `<nom du hwmon>:<libellé>`.
    ///
    /// Le nom porte la source parce que deux pilotes nomment volontiers leur
    /// capteur `temp1`, et qu'un nom ambigu vaut moins que pas de nom du tout.
    pub slug: String,
    /// Ce qu'on affiche : le libellé du noyau, ou `tempN` à défaut.
    pub libelle: String,
    /// D'où elle vient : le nom du `hwmon`.
    pub origine: String,
    /// Le fichier à relire à chaque tour.
    pub chemin: PathBuf,
}

impl Sonde {
    /// La température du moment, en millidegrés.
    ///
    /// `Err` quand le fichier a disparu — un périphérique débranché — ce qui
    /// doit se dire plutôt que de figer la dernière valeur lue.
    pub fn lire(&self) -> io::Result<i32> {
        let brut = fs::read_to_string(&self.chemin)?;
        // ⚠️ Jamais zéro en repli. Zéro est une température plausible : un repli
        // silencieux se lirait comme une chute, pas comme une panne.
        brut.trim().parse::<i32>().map_err(|erreur| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("« {} » : {erreur}", self.chemin.display()),
            )
        })
    }
}

/// Toutes les sondes d'une arborescence `sys/class/hwmon`.
///
/// Triées par `slug`, sans doublon. Un `hwmon` sans sonde de température ne
/// produit rien et n'est pas une erreur ; un fichier illisible n'empêche pas les
/// autres d'être rendus.
pub fn sondes_dans(sys_class: &Path) -> io::Result<Vec<Sonde>> {
    let mut brutes = Vec::new();
    for entree in fs::read_dir(sys_class)? {
        let Ok(entree) = entree else {
            continue;
        };
        // Le chemin est suivi, pas inspecté : dans un vrai `sysfs`, chaque
        // `hwmonN` est lui-même un lien symbolique. Filtrer sur `file_type()`
        // ne rendrait aucune sonde sur la machine.
        let dossier = entree.path();
        let Some(origine) = texte(&dossier.join("name")) else {
            continue;
        };
        let Ok(fichiers) = fs::read_dir(&dossier) else {
            continue;
        };
        for fichier in fichiers.flatten() {
            let nom = fichier.file_name();
            let Some(rang) = nom.to_str().and_then(rang_de_sonde) else {
                continue;
            };
            let libelle = texte(&dossier.join(format!("temp{rang}_label")))
                .unwrap_or_else(|| format!("temp{rang}"));
            brutes.push(Brute {
                origine: origine.clone(),
                // Ce qui départage deux `hwmon` homonymes. L'adresse du
                // périphérique d'abord — `8-0050` pour une barrette — parce
                // qu'elle survit au redémarrage ; le nom du répertoire à
                // défaut, faute de mieux.
                adresse: adresse(&dossier)
                    .or_else(|| nom_de_dossier(&dossier))
                    .unwrap_or_default(),
                libelle,
                chemin: fichier.path(),
            });
        }
    }

    // Un `slug` porte l'adresse **seulement** quand elle est nécessaire : les
    // numéros de `hwmon` changent au redémarrage, et un identifiant qui les
    // porte sans raison n'est pas stable.
    let mut compte: HashMap<String, usize> = HashMap::new();
    for brute in &brutes {
        *compte.entry(brute.court()).or_default() += 1;
    }
    let mut sondes: Vec<Sonde> = brutes
        .iter()
        .map(|brute| Sonde {
            slug: if compte.get(&brute.court()) == Some(&1) {
                brute.court()
            } else {
                format!(
                    "{}:{}:{}",
                    brute.origine,
                    brute.adresse,
                    slug(&brute.libelle)
                )
            },
            libelle: brute.libelle.clone(),
            origine: brute.origine.clone(),
            chemin: brute.chemin.clone(),
        })
        .collect();
    // Trié par `slug` et non par ses morceaux : c'est le `slug` qui nomme la
    // sonde partout ailleurs, et une liste triée autrement se lirait de travers.
    sondes.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(sondes)
}

/// Une sonde avant que son identifiant ne soit tranché.
struct Brute {
    origine: String,
    adresse: String,
    libelle: String,
    chemin: PathBuf,
}

impl Brute {
    /// L'identifiant tant qu'aucun homonyme ne le dispute.
    ///
    /// ⚠️ L'origine est gardée **telle quelle** — `mt7921_phy0`, pas
    /// `mt7921-phy0` — comme le fait déjà le nom d'un canal de ventilateur. Seul
    /// le libellé est mis en forme, parce que lui vient d'un humain : « Coolant
    /// temp » devient `coolant-temp`.
    fn court(&self) -> String {
        format!("{}:{}", self.origine, slug(&self.libelle))
    }
}

/// Le rang d'un `tempN_input`, et rien d'autre.
///
/// `temp1_crit`, `temp1_max` et `temperature_input` sont des voisins de nom qui
/// ne sont pas des sondes ; les prendre pour telles ferait apparaître des
/// courbes de seuils constants.
fn rang_de_sonde(fichier: &str) -> Option<&str> {
    let rang = fichier.strip_prefix("temp")?.strip_suffix("_input")?;
    (!rang.is_empty() && rang.bytes().all(|octet| octet.is_ascii_digit())).then_some(rang)
}

/// Le contenu d'un attribut `sysfs`, ébarbé, `None` s'il est absent ou vide.
fn texte(chemin: &Path) -> Option<String> {
    let brut = fs::read_to_string(chemin).ok()?;
    let propre = brut.trim();
    (!propre.is_empty()).then(|| propre.to_owned())
}

/// L'adresse du périphérique derrière un `hwmon` — `8-0050` pour une barrette.
fn adresse(dossier: &Path) -> Option<String> {
    let cible = fs::read_link(dossier.join("device")).ok()?;
    Some(cible.file_name()?.to_str()?.to_owned())
}

fn nom_de_dossier(dossier: &Path) -> Option<String> {
    Some(dossier.file_name()?.to_str()?.to_owned())
}

/// Les sondes de la machine.
pub fn sondes() -> io::Result<Vec<Sonde>> {
    sondes_dans(Path::new("/sys/class/hwmon"))
}
