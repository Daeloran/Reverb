//! Les zones : une zone, une couche.
//!
//! Jusqu'ici le démon n'avait qu'un état — une couleur par cible **ou** une
//! animation, jamais les deux. Toute peinture tuait l'animation, toute animation
//! écrasait les peintures. Une zone est ce qui réconcilie les deux.
//!
//! # Le modèle, et ce qu'il refuse
//!
//! - une **zone** est un ensemble de LED que l'utilisateur compose lui-même, à
//!   la souris. Ni prédéfinie, ni contiguë : « le ventilateur arrière plus
//!   bas-milieu plus haut-milieu » est une zone légitime ;
//! - une LED appartient à **au plus une** zone. Mettre une LED dans une nouvelle
//!   zone la retire de celle qui la tenait — c'est ce qui évite d'avoir à gérer
//!   un ordre d'empilement, et donc une notion de transparence qu'une LED ne
//!   sait pas porter ;
//! - ce qui n'est dans aucune zone suit la couche **« tout le boîtier »**,
//!   c'est-à-dire l'état d'avant les zones ;
//! - chaque zone porte soit une **couleur fixe**, soit une **animation** avec
//!   ses propres réglages, donc sa propre vitesse et sa propre phase.
//!
//! # Une animation de zone se calcule sur le boîtier entier
//!
//! Une vague donnée à la colonne du radiateur traverse le **boîtier**, et la
//! zone n'en montre que sa part. C'est ce qui garde deux zones voisines
//! cohérentes entre elles, et ce qui évite d'inventer une géométrie par zone.
//!
//! Conséquence assumée : une vague sur une zone d'une seule LED clignote au lieu
//! de défiler. C'est le prix d'une composition qui ne ment pas sur l'espace.
//!
//! # Un second fichier, et non un format élargi
//!
//! `eclairage.conf` porte la couche globale, `zones.conf` les couches nommées.
//! Deux fichiers pour deux natures, comme `geometrie.conf` et `eclairage.conf`.
//!
//! ⚠️ Le format d'`eclairage.conf` ne bouge **pas** : trente-six tests
//! d'intention de #21 le décrivent, et un design venu après eux ne les réécrit
//! pas.

use std::fmt;
use std::io;
use std::path::Path;

use reverb_anim::{Animation, Geometrie, Reglages};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Led, Rgb};

/// Où les zones sont conservées d'un démarrage à l'autre.
///
/// Un état de service, donc `/var/lib` et non `/etc` : le fichier est écrit par
/// le démon, pas par un administrateur.
pub const CHEMIN_ZONES: &str = "/var/lib/reverb/zones.conf";

/// Les couleurs des cent vingt-quatre LED, avant écriture sur les bus.
#[derive(Debug, Clone, PartialEq)]
pub struct Tampon {
    pub ventilateurs: [[Rgb; LEDS_PER_FAN as usize]; 10],
    pub barrettes: [[Rgb; LEDS_PER_STICK]; SLOT_COUNT],
}

impl Tampon {
    pub fn noir() -> Tampon {
        Tampon {
            ventilateurs: [[Rgb::BLACK; LEDS_PER_FAN as usize]; 10],
            barrettes: [[Rgb::BLACK; LEDS_PER_STICK]; SLOT_COUNT],
        }
    }

    pub fn couleur(&self, led: Led) -> Rgb {
        match led {
            Led::Ventilateur { position, led } => self.ventilateurs[position.index()][led],
            Led::Barrette { slot, led } => self.barrettes[slot][led],
        }
    }

    pub fn poser(&mut self, led: Led, couleur: Rgb) {
        match led {
            Led::Ventilateur { position, led } => {
                self.ventilateurs[position.index()][led] = couleur
            }
            Led::Barrette { slot, led } => self.barrettes[slot][led] = couleur,
        }
    }
}

/// Ce qu'une zone affiche.
#[derive(Debug, Clone, PartialEq)]
pub enum Rendu {
    /// Elle ne masque rien : ses LED suivent la couche globale.
    Transparente,
    Fixe(Rgb),
    Animee(Animation, Reglages),
}

/// Une zone : un nom, un ensemble de LED, et ce qu'elle affiche.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub nom: String,
    /// Triées et sans doublon : deux zones de même composition sont égales,
    /// quel que soit l'ordre dans lequel on a cliqué.
    pub cibles: Vec<Led>,
    pub rendu: Rendu,
}

/// Toutes les zones, dans l'ordre de leur création.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Zones {
    zones: Vec<Zone>,
}

/// Un fichier de zones n'a pas pu être lu.
#[derive(Debug, Clone, PartialEq)]
pub struct ZonesInvalides {
    /// Numéro de ligne, **à partir de 1**, comme un éditeur. Zéro quand la
    /// faute ne tient à aucune ligne en particulier.
    pub ligne: usize,
    pub raison: String,
}

impl fmt::Display for ZonesInvalides {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ligne == 0 {
            write!(f, "{}", self.raison)
        } else {
            write!(f, "ligne {} : {}", self.ligne, self.raison)
        }
    }
}

impl std::error::Error for ZonesInvalides {}

impl Zones {
    pub fn vide() -> Zones {
        Zones::default()
    }

    /// Crée ou redéfinit une zone.
    ///
    /// ⚠️ Les cibles données sont **retirées de toutes les autres zones**. Une
    /// zone qui se retrouve sans aucune LED est supprimée : une zone vide
    /// n'affiche rien et ne se désigne plus.
    ///
    /// Une zone qui existait garde son rendu ; une zone nouvelle naît
    /// transparente.
    pub fn poser(&mut self, nom: &str, cibles: &[Led]) {
        let mut triees: Vec<Led> = cibles.to_vec();
        triees.sort_unstable();
        triees.dedup();

        // D'abord retirer aux autres : une LED n'est jamais dans deux zones, et
        // c'est ce qui dispense d'un ordre d'empilement.
        for zone in &mut self.zones {
            if zone.nom != nom {
                zone.cibles.retain(|cible| !triees.contains(cible));
            }
        }
        // Une zone vidée par ce transfert ne se désigne plus : elle disparaît.
        self.zones
            .retain(|zone| !zone.cibles.is_empty() || zone.nom == nom);

        match self.zones.iter_mut().find(|zone| zone.nom == nom) {
            // Redéfinie : elle garde son rendu **et sa place**. C'est la même
            // zone, pas une nouvelle qui reprendrait le nom.
            Some(deja) => deja.cibles = triees,
            None if triees.is_empty() => {}
            None => self.zones.push(Zone {
                nom: nom.to_owned(),
                cibles: triees,
                rendu: Rendu::Transparente,
            }),
        }
        self.zones.retain(|zone| !zone.cibles.is_empty());
    }

    /// Supprime une zone. Ses LED reviennent à la couche globale sans
    /// s'éteindre. Faux si elle n'existait pas.
    pub fn retirer(&mut self, nom: &str) -> bool {
        let avant = self.zones.len();
        self.zones.retain(|zone| zone.nom != nom);
        self.zones.len() != avant
    }

    /// Donne une couleur fixe à une zone. Faux si elle n'existe pas.
    pub fn eclairer(&mut self, nom: &str, couleur: Rgb) -> bool {
        match self.zones.iter_mut().find(|zone| zone.nom == nom) {
            Some(zone) => {
                zone.rendu = Rendu::Fixe(couleur);
                true
            }
            None => false,
        }
    }

    /// Donne une animation à une zone, ou la rend transparente avec `None`.
    /// Faux si elle n'existe pas.
    pub fn animer(&mut self, nom: &str, rendu: Option<(Animation, Reglages)>) -> bool {
        match self.zones.iter_mut().find(|zone| zone.nom == nom) {
            Some(zone) => {
                zone.rendu = match rendu {
                    Some((animation, reglages)) => Rendu::Animee(animation, reglages),
                    None => Rendu::Transparente,
                };
                true
            }
            None => false,
        }
    }

    pub fn liste(&self) -> &[Zone] {
        &self.zones
    }

    pub fn zone(&self, nom: &str) -> Option<&Zone> {
        self.zones.iter().find(|zone| zone.nom == nom)
    }

    /// Écrase, sur le tampon de la couche globale, ce que les zones affichent.
    ///
    /// Une zone transparente ne touche à rien. Une zone fixe pose sa couleur.
    /// Une zone animée calcule son image **sur la géométrie entière** et n'en
    /// prend que ses propres LED.
    pub fn composer(&self, geometrie: &Geometrie, pas: u32, fond: &mut Tampon) {
        for zone in &self.zones {
            match &zone.rendu {
                Rendu::Transparente => {}
                Rendu::Fixe(couleur) => {
                    for cible in &zone.cibles {
                        fond.poser(*cible, *couleur);
                    }
                }
                Rendu::Animee(animation, reglages) => {
                    // ⚠️ Calculée sur la géométrie **entière**, puis découpée :
                    // c'est ce qui garde deux zones voisines cohérentes entre
                    // elles, et ce qui évite d'inventer une géométrie par zone.
                    let image = animation.image(geometrie, reglages, pas);
                    for cible in &zone.cibles {
                        let couleur = match cible {
                            Led::Ventilateur { position, led } => {
                                image.ventilateurs[position.index()].1[*led]
                            }
                            Led::Barrette { slot, led } => image.barrettes[*slot][*led],
                        };
                        fond.poser(*cible, couleur);
                    }
                }
            }
        }
    }

    /// Le texte du fichier de zones.
    ///
    /// Une ligne `zone <nom> <cible>,<cible>,…` par zone, suivie de son rendu :
    /// `light <nom> <rrggbb>` ou `anim <nom> <animation> [clé=valeur…]`. Une
    /// zone transparente n'a pas de ligne de rendu.
    pub fn encoder(&self) -> String {
        let mut texte = String::from(EN_TETE);
        for zone in &self.zones {
            texte.push_str(&format!(
                "zone {} {}\n",
                zone.nom,
                zone.cibles
                    .iter()
                    .map(|cible| cible.slug())
                    .collect::<Vec<String>>()
                    .join(",")
            ));
            match &zone.rendu {
                Rendu::Transparente => {}
                Rendu::Fixe(couleur) => texte.push_str(&format!(
                    "light {} {:02x}{:02x}{:02x}\n",
                    zone.nom, couleur.r, couleur.g, couleur.b
                )),
                Rendu::Animee(animation, reglages) => {
                    texte.push_str(&format!("anim {} {}", zone.nom, animation.nom()));
                    for (cle, valeur) in animation.reglages_ecrits(reglages) {
                        texte.push_str(&format!(" {cle}={valeur}"));
                    }
                    texte.push('\n');
                }
            }
        }
        texte
    }

    /// L'inverse d'[`Zones::encoder`], strict.
    ///
    /// Refuse en nommant la ligne : un premier mot inconnu, un nom de zone
    /// répété, un rendu pour une zone qu'aucune ligne `zone` n'a déclarée, une
    /// LED illisible, une LED présente dans deux zones.
    pub fn decoder(texte: &str) -> Result<Zones, ZonesInvalides> {
        let mut zones: Vec<Zone> = Vec::new();
        let mut prises: Vec<Led> = Vec::new();

        for (rang, ligne) in texte.lines().enumerate() {
            let numero = rang + 1;
            let refus = |raison: String| ZonesInvalides {
                ligne: numero,
                raison,
            };
            let ligne = ligne.trim();
            // Les lignes vides et les commentaires sont ignorés, comme dans les
            // deux autres fichiers : celui-ci porte un en-tête qui en est un.
            if ligne.is_empty() || ligne.starts_with('#') {
                continue;
            }
            let mots: Vec<&str> = ligne.split_whitespace().collect();
            match mots.as_slice() {
                ["zone", nom, cibles] => {
                    if zones.iter().any(|zone| zone.nom == *nom) {
                        return Err(refus(format!("zone « {nom} » déjà déclarée")));
                    }
                    let mut triees = Vec::new();
                    for brut in cibles.split(',') {
                        let lues =
                            Led::depuis_slug(brut).map_err(|erreur| refus(erreur.to_string()))?;
                        for led in lues {
                            if prises.contains(&led) {
                                return Err(refus(format!(
                                    "la LED « {} » est déjà dans une autre zone",
                                    led.slug()
                                )));
                            }
                            prises.push(led);
                            triees.push(led);
                        }
                    }
                    if triees.is_empty() {
                        return Err(refus(format!("zone « {nom} » sans aucune LED")));
                    }
                    triees.sort_unstable();
                    zones.push(Zone {
                        nom: (*nom).to_owned(),
                        cibles: triees,
                        rendu: Rendu::Transparente,
                    });
                }
                ["light", nom, couleur] => {
                    let zone = zones
                        .iter_mut()
                        .find(|zone| zone.nom == *nom)
                        .ok_or_else(|| refus(format!("zone « {nom} » jamais déclarée")))?;
                    zone.rendu = Rendu::Fixe(
                        Rgb::from_hex(couleur).map_err(|erreur| refus(erreur.to_string()))?,
                    );
                }
                ["anim", nom, animation, reglages @ ..] => {
                    let paires: Vec<(String, String)> = reglages
                        .iter()
                        .map(|brut| {
                            brut.split_once('=')
                                .map(|(cle, valeur)| (cle.to_owned(), valeur.to_owned()))
                                .ok_or_else(|| refus(format!("réglage « {brut} » sans signe égal")))
                        })
                        .collect::<Result<_, _>>()?;
                    let anime = Animation::par_nom(animation)
                        .map_err(|erreur| refus(erreur.to_string()))?;
                    let reglages = anime
                        .reglages(&paires)
                        .map_err(|erreur| refus(erreur.to_string()))?;
                    let zone = zones
                        .iter_mut()
                        .find(|zone| zone.nom == *nom)
                        .ok_or_else(|| refus(format!("zone « {nom} » jamais déclarée")))?;
                    zone.rendu = Rendu::Animee(anime, reglages);
                }
                // Une ligne connue mais tronquée : le dire par son nom plutôt
                // que par « premier mot inconnu », qui enverrait relire le mot
                // qui était juste.
                ["zone", nom] => {
                    return Err(refus(format!("zone « {nom} » sans aucune LED")));
                }
                ["light", nom] => {
                    return Err(refus(format!("zone « {nom} » sans couleur")));
                }
                ["anim", nom] => {
                    return Err(refus(format!("zone « {nom} » sans animation")));
                }
                ["zone" | "light" | "anim"] => {
                    return Err(refus(format!("« {} » sans nom de zone", mots[0])));
                }
                [premier, ..] => {
                    return Err(refus(format!(
                        "« {premier} » n'est ni « zone », ni « light », ni « anim »"
                    )));
                }
                [] => {}
            }
        }
        Ok(Zones { zones })
    }
}

/// Lit le fichier de zones, en disant ce qui a cloché plutôt qu'en échouant.
///
/// Un fichier absent donne des zones vides : c'est le cas d'un premier
/// démarrage, et ce n'est pas une anomalie. Un fichier **abîmé** donne des zones
/// vides **et** un message : le démon doit démarrer sur la couche globale seule
/// plutôt que de refuser de s'allumer.
pub fn charger(chemin: &Path) -> (Zones, Option<String>) {
    let texte = match std::fs::read_to_string(chemin) {
        Ok(texte) => texte,
        // Absent : premier démarrage, ce n'est pas une anomalie et ça ne se dit
        // pas.
        Err(erreur) if erreur.kind() == io::ErrorKind::NotFound => return (Zones::vide(), None),
        Err(erreur) => {
            return (
                Zones::vide(),
                Some(format!("{} illisible : {erreur}", chemin.display())),
            );
        }
    };
    match Zones::decoder(&texte) {
        Ok(zones) => (zones, None),
        Err(erreur) => (
            Zones::vide(),
            Some(format!(
                "{} : {erreur} — aucune zone chargée",
                chemin.display()
            )),
        ),
    }
}

/// Écrit le fichier de zones, par fichier temporaire puis renommage.
pub fn enregistrer(chemin: &Path, zones: &Zones) -> io::Result<()> {
    if let Some(dossier) = chemin.parent() {
        std::fs::create_dir_all(dossier)?;
    }
    // Fichier provisoire puis renommage : une coupure de courant au milieu de
    // l'écriture laisserait sinon un fichier tronqué, et le démarrage suivant
    // perdrait toutes les zones au lieu d'une seule.
    let provisoire = chemin.with_extension("conf.nouveau");
    std::fs::write(&provisoire, zones.encoder())?;
    std::fs::rename(&provisoire, chemin)
}

/// L'en-tête du fichier de zones.
const EN_TETE: &str = "\
# Zones d'éclairage — Reverb (issue #29)
#
# Une zone = une couche. Une LED appartient a au plus une zone ; ce qui n'est
# dans aucune suit la couche « tout le boitier » d'eclairage.conf.
#
#   zone  <nom> <cible>,<cible>,...   fan:<position>:<0-7> ou slot:<0-3>:<0-10>
#   light <nom> <rrggbb>              couleur fixe
#   anim  <nom> <animation> [cle=valeur...]
#
# Ce fichier est ecrit par reverbd. Il se modifie aussi par le socket :
#   zone set <nom> <cibles> / zone light <nom> <rrggbb> / zone anim <nom> <...>

";
