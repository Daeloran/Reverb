//! Ce que le démon conserve d'un démarrage à l'autre.
//!
//! Deux fichiers, deux natures. La **géométrie** est une donnée de montage :
//! elle se décide une fois, sous le bureau, et ne bouge qu'au démontage d'un
//! ventilateur — sa place est dans `/etc`, avec ce que l'administrateur règle.
//! L'**éclairage** est l'état courant du service, réécrit à chaque commande :
//! sa place est dans `/var/lib`, avec ce qu'un service accumule. Les mêler
//! ferait réécrire à chaque changement de couleur le fichier qui a coûté un
//! relevé au sol.
//!
//! **La fenêtre n'écrit aucun fichier.** Elle demande par le socket, et c'est
//! le démon — qui est root — qui écrit. Le socket reste ainsi l'unique
//! franchissement de privilège, au lieu d'un second mécanisme de droits à
//! entretenir à côté.
//!
//! Le format est maison, une ligne par chose : `serde` serait la plus grosse
//! dépendance du projet pour vingt nombres et quatorze couleurs, et le
//! protocole IPC a déjà prouvé qu'un format texte tenu à la main suffit ici.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use reverb_anim::{Animation, Geometrie, Reglages};
use reverb_proto::{Position, Rgb, ram};

/// Où la géométrie est conservée entre deux démarrages.
pub const CHEMIN_GEOMETRIE: &str = "/etc/reverb/geometrie.conf";

/// Où l'éclairage courant est conservé entre deux démarrages.
///
/// `/var/lib` et non `/etc` : c'est le service qui l'écrit, pas l'utilisateur.
/// L'unité systemd le crée par `StateDirectory=reverb`.
pub const CHEMIN_ECLAIRAGE: &str = "/var/lib/reverb/eclairage.conf";

/// En-tête du fichier de géométrie, pour qui l'ouvre sans savoir ce que c'est.
const EN_TETE_GEOMETRIE: &str = "\
# Orientation des ventilateurs — Reverb (issue #19)
#
# Une ligne par ventilateur : <position> <angle> <sens>
#   angle : degrés de la LED 1, 0 = midi, de 0 a 359
#   sens  : horaire ou antihoraire, vu de l'exterieur du boitier
#
# Ce fichier est ecrit par reverbd. Il se modifie aussi par le socket :
#   geometry <position> angle=<0-359> sens=<horaire|antihoraire>
";

/// En-tête du fichier d'éclairage.
const EN_TETE_ECLAIRAGE: &str = "\
# Eclairage courant — Reverb (issue #21)
#
# Ecrit par reverbd a chaque changement d'etat, relu au demarrage. Ce fichier
# n'est pas fait pour etre edite a la main : le supprimer suffit a revenir a
# l'eclairage d'accueil.
#
#   ventilateur <position> <rrggbb>   les dix, exactement une fois chacune
#   barrette <0-3> <rrggbb>           les quatre, exactement une fois chacune
#   animation <nom> [cle=valeur ...]  au plus une, absente si rien n'est anime
";

/// La couleur du tout premier démarrage.
///
/// Un bleu pur, distinct du magenta par défaut des animations : un boîtier qui
/// s'allume en bleu se lit comme « rien à rejouer », pas comme un réglage qu'on
/// aurait oublié avoir posé.
///
/// ⚠️ Écrite en **RGB logique**, comme partout dans `reverb-proto`. Les
/// ventilateurs la recevront en GRB et la RAM en RGB — c'est `frame` et `ram`
/// qui s'en chargent. Une permutation faite ici ne produirait aucun message,
/// juste un boîtier rouge.
pub const ACCUEIL: Rgb = Rgb::new(0x00, 0x00, 0xff);

/// Lit la géométrie, ou rend celle mesurée en usine.
///
/// Ne refuse **jamais** de rendre une géométrie : un démon qui ne démarrerait
/// pas parce qu'un fichier de configuration est de travers laisserait la
/// machine sans éclairage du tout. Ce qui cloche est signalé et rendu à
/// l'appelant, qui le journalise avec les autres soucis d'ouverture.
pub fn charger_geometrie(chemin: &Path) -> (Geometrie, Option<String>) {
    let texte = match fs::read_to_string(chemin) {
        Ok(texte) => texte,
        // L'absence n'est pas une anomalie : c'est le premier démarrage.
        Err(erreur) if erreur.kind() == io::ErrorKind::NotFound => {
            return (Geometrie::mesuree(), None);
        }
        Err(erreur) => {
            return (
                Geometrie::mesuree(),
                Some(format!(
                    "géométrie illisible dans {} ({erreur}) : orientation d'usine appliquée",
                    chemin.display()
                )),
            );
        }
    };

    match Geometrie::decoder(&texte) {
        Ok(geometrie) => (geometrie, None),
        Err(erreur) => (
            Geometrie::mesuree(),
            Some(format!(
                "géométrie invalide dans {} ({erreur}) : orientation d'usine appliquée",
                chemin.display()
            )),
        ),
    }
}

/// Écrit la géométrie, en une fois.
///
/// Par un fichier temporaire puis un renommage : une écriture directe qu'une
/// coupure interromprait laisserait un fichier tronqué, que le démarrage
/// suivant refuserait — et l'utilisateur perdrait une géométrie qu'il avait
/// pourtant réglée.
pub fn enregistrer_geometrie(chemin: &Path, geometrie: &Geometrie) -> io::Result<()> {
    ecrire(
        chemin,
        &format!("{EN_TETE_GEOMETRIE}\n{}\n", geometrie.encoder()),
    )
}

/// L'éclairage tel qu'il se conserve d'un démarrage à l'autre.
///
/// Les couleurs fixes **et** l'animation, ensemble : ce sont deux états du même
/// boîtier, et `animate off` doit rendre la couleur d'avant, y compris après un
/// redémarrage.
#[derive(Debug, Clone, PartialEq)]
pub struct Eclairage {
    pub ventilateurs: [Rgb; 10],
    pub barrettes: [Rgb; ram::SLOT_COUNT],
    /// L'animation en cours, avec ses réglages. `None` quand l'éclairage est
    /// fixe.
    pub animation: Option<(Animation, Reglages)>,
}

/// Ce qui cloche dans un fichier d'éclairage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EclairageInvalide {
    /// Numéro de ligne dans le texte, **à partir de 1** — comme un éditeur.
    /// Vaut 0 quand la faute ne tient à aucune ligne en particulier : une
    /// position absente n'est écrite nulle part.
    pub ligne: usize,
    pub raison: String,
}

impl fmt::Display for EclairageInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ligne == 0 {
            write!(f, "{}", self.raison)
        } else {
            write!(f, "ligne {} : {}", self.ligne, self.raison)
        }
    }
}

impl std::error::Error for EclairageInvalide {}

impl Eclairage {
    /// L'éclairage du tout premier démarrage : [`ACCUEIL`] partout, rien
    /// d'animé.
    ///
    /// Sur les ventilateurs **et** sur la RAM : les deux bus s'éclairent, donc
    /// une installation qui n'a jamais reçu de commande prouve d'elle-même que
    /// la chaîne complète fonctionne.
    pub fn accueil() -> Eclairage {
        Eclairage {
            ventilateurs: [ACCUEIL; 10],
            barrettes: [ACCUEIL; ram::SLOT_COUNT],
            animation: None,
        }
    }

    /// Le texte du fichier, en-tête exclu.
    ///
    /// Les réglages d'une animation ne sont écrits **que pour les clés qu'elle
    /// accepte** : `arc-en-ciel` refuse `couleur`, et un fichier qui la lui
    /// redonnerait au démarrage suivant serait rejeté en bloc — l'éclairage
    /// entier perdu pour une clé de trop.
    pub fn encoder(&self) -> String {
        let mut texte = String::new();
        for position in Position::ALL {
            let couleur = hexa(self.ventilateurs[position.index()]);
            texte.push_str(&format!("ventilateur {} {couleur}\n", position.slug()));
        }
        for (slot, couleur) in self.barrettes.iter().enumerate() {
            texte.push_str(&format!("barrette {slot} {}\n", hexa(*couleur)));
        }

        if let Some((animation, reglages)) = &self.animation {
            // Les clés écrites sont celles que l'animation accepte, et c'est
            // elle qui le dit — le même point unique que le socket emploie.
            texte.push_str(&format!("animation {}", animation.nom()));
            for (cle, valeur) in animation.reglages_ecrits(reglages) {
                texte.push_str(&format!(" {cle}={valeur}"));
            }
            texte.push('\n');
        }
        texte
    }

    /// L'inverse d'[`Eclairage::encoder`].
    ///
    /// Les lignes vides et celles commençant par `#` sont ignorées ; l'ordre
    /// des lignes est sans importance. Une position ou une barrette **absente
    /// ou répétée** est refusée en la nommant : c'est ce qui rend un fichier
    /// tronqué détectable, et un fichier tronqué doit être signalé, pas
    /// complété au jugé.
    pub fn decoder(texte: &str) -> Result<Eclairage, EclairageInvalide> {
        let mut ventilateurs: [Option<Rgb>; 10] = [None; 10];
        let mut barrettes: [Option<Rgb>; ram::SLOT_COUNT] = [None; ram::SLOT_COUNT];
        let mut animation = None;

        for (rang, brut) in texte.lines().enumerate() {
            // Le rang dans le fichier qu'on ouvre, commentaires et lignes vides
            // compris : c'est le numéro qu'affiche un éditeur.
            let ligne = rang + 1;
            let refus = |raison: String| EclairageInvalide { ligne, raison };

            let contenu = brut.trim();
            if contenu.is_empty() || contenu.starts_with('#') {
                continue;
            }
            let jetons: Vec<&str> = contenu.split_whitespace().collect();

            match jetons[0] {
                "ventilateur" => {
                    if jetons.len() != 3 {
                        return Err(refus(format!(
                            "« ventilateur » attend une position et une couleur : « {contenu} »"
                        )));
                    }
                    let position = Position::from_slug(jetons[1])
                        .map_err(|erreur| refus(erreur.to_string()))?;
                    let couleur =
                        Rgb::from_hex(jetons[2]).map_err(|erreur| refus(erreur.to_string()))?;
                    if ventilateurs[position.index()].is_some() {
                        return Err(refus(format!(
                            "ventilateur « {} » donné deux fois : impossible de savoir laquelle \
                             des deux couleurs était la bonne",
                            jetons[1]
                        )));
                    }
                    ventilateurs[position.index()] = Some(couleur);
                }

                "barrette" => {
                    if jetons.len() != 3 {
                        return Err(refus(format!(
                            "« barrette » attend un rang et une couleur : « {contenu} »"
                        )));
                    }
                    let dernier = ram::SLOT_COUNT - 1;
                    let slot: usize = jetons[1].parse().map_err(|_| {
                        refus(format!(
                            "barrette « {} » : attendu un rang de 0 à {dernier}",
                            jetons[1]
                        ))
                    })?;
                    if slot >= ram::SLOT_COUNT {
                        return Err(refus(format!(
                            "barrette {slot} : il n'y en a que {}, de 0 à {dernier}",
                            ram::SLOT_COUNT
                        )));
                    }
                    let couleur =
                        Rgb::from_hex(jetons[2]).map_err(|erreur| refus(erreur.to_string()))?;
                    if barrettes[slot].is_some() {
                        return Err(refus(format!(
                            "barrette {slot} donnée deux fois : impossible de savoir laquelle des \
                             deux couleurs était la bonne"
                        )));
                    }
                    barrettes[slot] = Some(couleur);
                }

                "animation" => {
                    if animation.is_some() {
                        return Err(refus(
                            "une animation à la fois : « animation » est donné deux fois"
                                .to_owned(),
                        ));
                    }
                    if jetons.len() < 2 {
                        return Err(refus("« animation » attend un nom du catalogue".to_owned()));
                    }
                    let choisie = Animation::par_nom(jetons[1])
                        .map_err(|erreur| refus(erreur.to_string()))?;

                    // Les mêmes paires que sur le socket, et le même juge :
                    // c'est l'animation qui dit ce qu'elle accepte, ici comme
                    // là-bas. Deux vocabulaires divergeraient au premier
                    // réglage ajouté.
                    let mut paires = Vec::new();
                    for jeton in &jetons[2..] {
                        match jeton.split_once('=') {
                            Some((cle, valeur)) if !cle.is_empty() && !valeur.is_empty() => {
                                paires.push((cle.to_owned(), valeur.to_owned()));
                            }
                            _ => {
                                return Err(refus(format!(
                                    "réglage « {jeton} » : attendu « clé=valeur »"
                                )));
                            }
                        }
                    }
                    let reglages = choisie
                        .reglages(&paires)
                        .map_err(|erreur| refus(erreur.to_string()))?;
                    animation = Some((choisie, reglages));
                }

                autre => {
                    return Err(refus(format!(
                        "« {autre} » n'est pas une ligne d'éclairage. Lignes attendues : \
                         ventilateur, barrette, animation"
                    )));
                }
            }
        }

        // Ce qui manque n'est écrit nulle part : la faute ne tient à aucune
        // ligne, d'où le numéro 0. Compléter au jugé — par du noir, par
        // l'accueil — rendrait un éclairage plausible et faux.
        let mut couleurs_ventilateurs = [Rgb::BLACK; 10];
        for position in Position::ALL {
            couleurs_ventilateurs[position.index()] =
                ventilateurs[position.index()].ok_or_else(|| EclairageInvalide {
                    ligne: 0,
                    raison: format!("ventilateur « {} » absent du fichier", position.slug()),
                })?;
        }
        let mut couleurs_barrettes = [Rgb::BLACK; ram::SLOT_COUNT];
        for (slot, couleur) in barrettes.iter().enumerate() {
            couleurs_barrettes[slot] = couleur.ok_or_else(|| EclairageInvalide {
                ligne: 0,
                raison: format!("barrette {slot} absente du fichier"),
            })?;
        }

        Ok(Eclairage {
            ventilateurs: couleurs_ventilateurs,
            barrettes: couleurs_barrettes,
            animation,
        })
    }
}

/// Une couleur en six chiffres hexadécimaux, comme sur le socket.
fn hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

/// Lit l'éclairage à retrouver, ou celui d'accueil.
///
/// Ne refuse **jamais** de rendre un éclairage, pour la même raison que la
/// géométrie : un fichier de travers ne doit pas laisser la machine dans le
/// noir. Un fichier **absent** rend l'accueil sans rien dire — c'est le premier
/// démarrage. Un fichier illisible ou invalide rend l'accueil **et** un
/// message : la différence entre « jamais réglé » et « réglé puis abîmé » ne
/// doit pas se perdre.
pub fn charger_eclairage(chemin: &Path) -> (Eclairage, Option<String>) {
    let texte = match fs::read_to_string(chemin) {
        Ok(texte) => texte,
        // L'absence n'est pas une anomalie : c'est le premier démarrage.
        Err(erreur) if erreur.kind() == io::ErrorKind::NotFound => {
            return (Eclairage::accueil(), None);
        }
        Err(erreur) => {
            return (
                Eclairage::accueil(),
                Some(format!(
                    "éclairage illisible dans {} ({erreur}) : couleur d'accueil appliquée",
                    chemin.display()
                )),
            );
        }
    };

    match Eclairage::decoder(&texte) {
        Ok(eclairage) => (eclairage, None),
        Err(erreur) => (
            Eclairage::accueil(),
            Some(format!(
                "éclairage invalide dans {} ({erreur}) : couleur d'accueil appliquée",
                chemin.display()
            )),
        ),
    }
}

/// Écrit l'éclairage, en une fois.
///
/// Même précaution que la géométrie : fichier temporaire puis renommage. Ici
/// l'écriture est fréquente — une par commande — donc l'occasion d'être
/// interrompu par une coupure est d'autant plus grande.
pub fn enregistrer_eclairage(chemin: &Path, eclairage: &Eclairage) -> io::Result<()> {
    ecrire(
        chemin,
        &format!("{EN_TETE_ECLAIRAGE}\n{}\n", eclairage.encoder()),
    )
}

/// Écrit un fichier de configuration, en une fois.
pub(crate) fn ecrire(chemin: &Path, contenu: &str) -> io::Result<()> {
    if let Some(dossier) = chemin.parent() {
        fs::create_dir_all(dossier)?;
    }
    let provisoire = chemin.with_extension("conf.nouveau");
    fs::write(&provisoire, contenu)?;
    fs::rename(&provisoire, chemin)
}
