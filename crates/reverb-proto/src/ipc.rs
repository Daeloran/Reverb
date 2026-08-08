//! Protocole entre le démon et ses clients.
//!
//! Du texte, une ligne à la fois. Pas de JSON : `serde` serait la plus grosse
//! dépendance du projet pour un protocole qui a quatre verbes, et l'ADR-001
//! pose le zéro dépendance. Le prix est ce module — qu'on peut lire, et dont
//! chaque règle est vérifiable sans démarrer de démon ni brancher de matériel.
//!
//! # Forme du dialogue
//!
//! - une **requête** est une ligne ;
//! - une **réponse** est zéro ou plusieurs lignes de données, puis
//!   **exactement une** ligne de fin : `end` en cas de succès, `err <message>`
//!   en cas d'échec.
//!
//! ```text
//! < status
//! > chan nzxtsmart2:fan-1 radiateur-haut 1200 60 manual
//! > temp kraken2023elite:coolant 34200
//! > end
//!
//! < bidule
//! > err commande « bidule » inconnue
//! ```
//!
//! # L'invariant de cadrage, et ce qu'il coûte
//!
//! **Une ligne de données ne commence jamais par `end` ni par `err`, et tient
//! toujours sur une seule ligne.** C'est ce qui permet à un client de lire
//! jusqu'à la première ligne terminale sans compter, et le seul garde-fou du
//! cadre : s'il cède, le client lit une réponse tronquée, affiche un état
//! partiel, puis prend les lignes restantes pour la réponse suivante — un
//! décalage qui ne se rattrape jamais.
//!
//! Le préfixe de type (`chan`, `temp`, `unreadable`) l'assure pour le début de
//! ligne. Il n'assure rien contre un **saut de ligne à l'intérieur d'un champ**,
//! et les noms de canaux viennent du matériel : rien ne garantit qu'un jour
//! l'un d'eux ne portera pas de caractère de contrôle. D'où [`encode_response_line`],
//! qui **neutralise** au lieu de refuser — perdre un canal de la télémétrie
//! parce que son nom est bizarre serait pire que l'afficher avec un souligné.
//!
//! # Sensible à la casse
//!
//! `STATUS` n'est pas `status`. C'est un dialogue entre deux programmes, pas
//! une invite de commande : accepter les variantes n'ajouterait qu'une surface
//! de compatibilité à tenir.

use std::fmt;

use crate::LEDS_PER_FAN;
use crate::color::Rgb;
use crate::composition::{Ancre, Fond, Source};
use crate::led::Led;
use crate::position::Position;
use crate::profil::NomProfil;
use crate::ram::{self, SLOT_COUNT};
use crate::screen::BRIGHTNESS_MAX as LUMINOSITE_MAX;

/// Longueur maximale d'une ligne acceptée, en octets. `1024` passe, `1025` non.
///
/// Au-delà, la ligne est refusée **sans être conservée** : un client qui envoie
/// un mégaoctet sans `\n` ne doit pas faire enfler la mémoire du démon, et
/// recopier la ligne dans l'erreur pour l'afficher ensuite referait au moment du
/// diagnostic exactement l'allocation qu'on refusait à l'analyse.
pub const MAX_LINE_LEN: usize = 1024;

/// Marque d'un champ absent, dans une ligne de réponse.
const ABSENT: &str = "-";

/// Nom réservé qui arrête l'animation en cours.
///
/// ⚠️ Une animation qui s'appellerait `off` serait donc inatteignable. Le
/// catalogue ne doit pas en contenir — c'est le prix d'un verbe unique plutôt
/// que d'un `animate` et d'un `stop` séparés, pour quatre verbes en tout.
const ANIMATION_OFF: &str = "off";

/// Caractère de remplacement des blancs et des caractères de contrôle.
const NEUTRE: char = '_';

// ---------------------------------------------------------------------------
// Requêtes
// ---------------------------------------------------------------------------

/// Un ordre adressé au démon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// `status` — télémétrie complète.
    Status,
    /// `light <cible> <hex>` — une couleur fixe.
    Light { target: LightTarget, color: Rgb },
    /// `animate <nom> [clé=valeur…]` — lance une animation ; `animate off` l'arrête.
    ///
    /// Les réglages sont **transportés bruts**, jamais interprétés ici : ce
    /// crate ne peut pas dépendre de `reverb-anim`, et n'a pas à connaître les
    /// paramètres qu'une animation accepte. Le refus d'une clé inconnue
    /// appartient à l'animation, seule à savoir ce qu'elle accepte.
    Animate {
        name: Option<String>,
        reglages: Vec<(String, String)>,
    },
    /// `fan <canal> pwm <0-100>` ou `fan <canal> auto`.
    Fan { channel: String, action: FanAction },
    /// `geometry` — lit la géométrie ; `geometry <position> clé=valeur…` la change.
    ///
    /// Même partage que pour `Animate` : le protocole transporte, le moteur
    /// interprète.
    Geometry {
        cible: Option<String>,
        reglages: Vec<(String, String)>,
    },
    /// `paint <cible> <rrggbb>,<rrggbb>,…` — une couleur par LED.
    ///
    /// La cible est **une seule** : `fan:<position>` ou `slot:<0-3>`. `all`,
    /// `fans` et `ram` n'ont pas de sens ici — une liste de couleurs appliquée
    /// à dix ventilateurs ne dit pas laquelle va où.
    ///
    /// Le compte est vérifié à la lecture, contre le matériel : huit pour un
    /// ventilateur, onze pour une barrette. Une liste trop courte complétée par
    /// du noir donnerait des LED éteintes sans un message.
    ///
    /// ⚠️ **Cet état ne survit pas à un redémarrage** : `/var/lib/reverb/
    /// eclairage.conf` conserve une couleur par cible (#21), pas une par LED.
    /// Ce qui est peint à la main revient à la couleur de sa cible au
    /// redémarrage suivant.
    Paint {
        target: LightTarget,
        couleurs: Vec<Rgb>,
    },
    /// `lighting` — rend l'état d'éclairage courant, sans rien changer.
    ///
    /// Quatorze lignes `light` — les couleurs fixes, celles que `animate off`
    /// rendrait — plus une ligne `anim` s'il y a une animation, puis `end`.
    ///
    /// **Un verbe à part, et non un `light` sans argument** comme `geometry`
    /// sans argument : un test d'intention de #17 exige que `light` seul soit
    /// refusé en nommant le verbe, et un test d'intention ne se réécrit pas
    /// pour arranger un design venu après lui.
    Lighting,
    /// `zone list` — rend la composition et le rendu de chaque zone.
    ZoneList,
    /// `zone set <nom> <cible>[,<cible>…]` — crée ou redéfinit une zone.
    ///
    /// Une cible est une LED précise (`fan:arriere:3`, `slot:0:7`) ou un organe
    /// entier (`fan:arriere`, `slot:0`), qui vaut pour toutes ses LED.
    ///
    /// ⚠️ **Une LED appartient à au plus une zone.** Les cibles données ici sont
    /// **retirées** des autres zones, sans qu'il faille le demander : c'est ce
    /// qui évite d'avoir à trancher un ordre d'empilement.
    ZoneSet { nom: String, cibles: Vec<Led> },
    /// `zone drop <nom>` — supprime une zone. Ses LED reviennent à la couche
    /// « tout le boîtier », sans s'éteindre.
    ZoneDrop { nom: String },
    /// `zone light <nom> <rrggbb>` — la zone porte une couleur fixe.
    ZoneLight { nom: String, couleur: Rgb },
    /// `zone anim <nom> <animation|off> [clé=valeur…]` — la zone porte une
    /// animation, avec sa propre vitesse et sa propre phase.
    ///
    /// `off` la rend transparente : ses LED reprennent la couche globale.
    ZoneAnim {
        nom: String,
        animation: Option<String>,
        reglages: Vec<(String, String)>,
    },
    /// `screen <action>` — l'écran du Kraken.
    ///
    /// ⚠️ Le démon **détient** désormais l'écran. `peripheriques.rs` disait
    /// « il rejoindra le démon quand la fenêtre en aura besoin » : on y est. La
    /// fenêtre n'ouvre aucun périphérique (ADR-002), elle envoie donc un
    /// **chemin de fichier** et le démon lit lui-même — jamais 1,2 Mo de pixels
    /// sur un protocole texte.
    Screen(ScreenAction),
    /// `profil <action>` — les ambiances nommées (#74).
    Profil(ProfilAction),
    /// `watch` — le démon pousse l'image courante, puis chaque nouvelle.
    ///
    /// Quatorze lignes `frame` puis `end`, et ainsi de suite tant que le client
    /// écoute. La première image part **à l'abonnement**, sans attendre un
    /// changement : une fenêtre qui vient de s'ouvrir doit montrer le boîtier
    /// tel qu'il est, pas un cadre vide jusqu'à la prochaine animation.
    ///
    /// C'est ce qui rend « l'aperçu montre ce que le boîtier reçoit » vrai par
    /// construction : la fenêtre affiche les images calculées par le code même
    /// qui écrit sur les bus, au lieu de les recalculer de son côté.
    Watch,
}

/// Ce que vise une commande d'éclairage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightTarget {
    /// `all` — tout ce que le démon tient.
    All,
    /// `fans` — les dix ventilateurs.
    Fans,
    /// `fan:<slug>` — un ventilateur, par sa position.
    Fan(Position),
    /// `ram` — les quatre barrettes.
    Ram,
    /// `slot:<0-3>` — une barrette.
    RamSlot(usize),
}

/// Ce qu'une commande `screen` demande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenAction {
    /// `screen state` — luminosité et contenu courants.
    State,
    /// `screen brightness <0-100>` — zéro éteint la dalle sans perdre ce qu'elle
    /// affichait.
    Brightness(u8),
    /// `screen off` — la dalle est rendue au firmware.
    Off,
    /// `screen image <chemin>` — une image fixe, PNG ou JPEG.
    ///
    /// Le chemin doit être **absolu** : le démon lit sous son propre répertoire
    /// courant, qui n'est pas celui du client.
    Image(String),
    /// `screen gauge <sonde>` — un cadran sur une sonde, rafraîchi par le démon.
    Gauge(String),
    /// `screen gif <chemin>` — une animation, jouée en boucle.
    Gif(String),
    /// `screen layout` — la composition courante, sans rien changer.
    LayoutState,
    /// `screen layout fond noir` · `screen layout fond image <chemin>`.
    ///
    /// Poser un fond **crée** la composition si la dalle n'en portait pas :
    /// c'est par là qu'on y entre, et exiger une commande d'ouverture avant de
    /// pouvoir poser quoi que ce soit n'apprendrait rien à personne.
    LayoutFond(Fond),
    /// `screen layout champ <ancre> temp <sonde> [libellé]`
    /// · `screen layout champ <ancre> texte <libellé>`.
    LayoutChamp(Ancre, Source),
    /// `screen layout vide <ancre>` — retire le champ de cette ancre.
    LayoutVide(Ancre),
    /// `screen layout off` — retour à l'affichage simple.
    LayoutOff,
}

/// Ce qu'une commande `profil` demande.
///
/// ⚠️ **Le nom est déjà validé** — c'est un [`NomProfil`], pas une chaîne. La
/// validation appartient au décodage de la requête, en amont de toute
/// construction de chemin : le démon est root, et un nom qui traverserait
/// jusqu'à `open()` sans avoir été borné serait une écriture arbitraire sur le
/// système de fichiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfilAction {
    /// `profil list` — les ambiances connues, sans en appliquer aucune.
    List,
    /// `profil save <nom>` — enregistre l'état courant sous ce nom.
    Save(NomProfil),
    /// `profil load <nom>` — applique cette ambiance.
    Load(NomProfil),
    /// `profil drop <nom>` — oublie cette ambiance.
    Drop(NomProfil),
}

/// Ce qu'on demande à un canal de vitesse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanAction {
    Pwm(u8),
    Auto,
}

/// Une ligne de requête n'a pas pu être comprise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestError {
    /// Ligne vide, ou uniquement des blancs.
    Empty,
    /// Ligne au-delà de [`MAX_LINE_LEN`]. **Ne porte que la longueur.**
    TooLong { given: usize },
    /// Premier mot inconnu.
    UnknownVerb { verb: String },
    /// Verbe connu, arguments mauvais.
    BadArgument { verb: String, reason: String },
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestError::Empty => write!(f, "ligne vide"),
            RequestError::TooLong { given } => write!(
                f,
                "ligne de {given} octets, maximum {MAX_LINE_LEN} — refusée sans être lue"
            ),
            RequestError::UnknownVerb { verb } => {
                write!(f, "commande « {verb} » inconnue")
            }
            RequestError::BadArgument { verb, reason } => {
                write!(f, "« {verb} » : {reason}")
            }
        }
    }
}

impl std::error::Error for RequestError {}

/// Analyse une ligne de requête.
///
/// # Erreurs
///
/// Voir [`RequestError`]. La longueur est vérifiée **avant** tout découpage :
/// c'est ce qui rend le refus d'une ligne de 10 Ko gratuit.
pub fn parse_request(line: &str) -> Result<Request, RequestError> {
    if line.len() > MAX_LINE_LEN {
        return Err(RequestError::TooLong { given: line.len() });
    }

    let mut mots = line.split_whitespace();
    let Some(verbe) = mots.next() else {
        return Err(RequestError::Empty);
    };
    let arguments: Vec<&str> = mots.collect();

    let mauvais = |raison: &str| RequestError::BadArgument {
        verb: verbe.to_owned(),
        reason: raison.to_owned(),
    };

    match verbe {
        "status" => {
            if arguments.is_empty() {
                Ok(Request::Status)
            } else {
                Err(mauvais("n'attend aucun argument"))
            }
        }

        "lighting" => {
            if arguments.is_empty() {
                Ok(Request::Lighting)
            } else {
                Err(mauvais("n'attend aucun argument"))
            }
        }

        "watch" => {
            if arguments.is_empty() {
                Ok(Request::Watch)
            } else {
                Err(mauvais("n'attend aucun argument"))
            }
        }

        "zone" => {
            let [action, reste @ ..] = arguments.as_slice() else {
                return Err(mauvais(
                    "attend une action : list, set, drop, light ou anim",
                ));
            };
            match *action {
                "list" => {
                    if reste.is_empty() {
                        Ok(Request::ZoneList)
                    } else {
                        Err(mauvais("« zone list » n'attend aucun argument"))
                    }
                }
                "set" => {
                    let [nom, cibles] = reste else {
                        return Err(mauvais(
                            "« zone set » attend un nom et des cibles séparées par des virgules, \
                             par exemple « zone set devant fan:arriere,slot:0:3 »",
                        ));
                    };
                    let nom = nom_de_zone(nom).map_err(|raison| mauvais(&raison))?;
                    let mut toutes = Vec::new();
                    for brut in cibles.split(',') {
                        toutes.extend(
                            Led::depuis_slug(brut)
                                .map_err(|erreur| mauvais(&erreur.to_string()))?,
                        );
                    }
                    if toutes.is_empty() {
                        return Err(mauvais("une zone sans aucune LED ne se désigne plus"));
                    }
                    Ok(Request::ZoneSet {
                        nom,
                        cibles: toutes,
                    })
                }
                "drop" => {
                    let [nom] = reste else {
                        return Err(mauvais("« zone drop » attend un nom, et lui seul"));
                    };
                    Ok(Request::ZoneDrop {
                        nom: nom_de_zone(nom).map_err(|raison| mauvais(&raison))?,
                    })
                }
                "light" => {
                    let [nom, couleur] = reste else {
                        return Err(mauvais("« zone light » attend un nom et une couleur"));
                    };
                    Ok(Request::ZoneLight {
                        nom: nom_de_zone(nom).map_err(|raison| mauvais(&raison))?,
                        couleur: couleur_stricte(couleur).ok_or_else(|| mauvais(HEXA))?,
                    })
                }
                "anim" => {
                    let [nom, animation, reglages @ ..] = reste else {
                        return Err(mauvais(
                            "« zone anim » attend un nom et une animation, ou « off »",
                        ));
                    };
                    let nom = nom_de_zone(nom).map_err(|raison| mauvais(&raison))?;
                    if *animation == "off" {
                        if !reglages.is_empty() {
                            return Err(mauvais(
                                "« zone anim <nom> off » n'attend aucun réglage : il n'y a plus \
                                 d'animation à régler",
                            ));
                        }
                        return Ok(Request::ZoneAnim {
                            nom,
                            animation: None,
                            reglages: Vec::new(),
                        });
                    }
                    Ok(Request::ZoneAnim {
                        nom,
                        animation: Some((*animation).to_owned()),
                        reglages: paires(reglages).map_err(|raison| mauvais(&raison))?,
                    })
                }
                autre => Err(mauvais(&format!(
                    "action « {autre} » inconnue : list, set, drop, light ou anim"
                ))),
            }
        }

        "screen" => {
            let [action, reste @ ..] = arguments.as_slice() else {
                return Err(mauvais(
                    "attend une action : state, brightness, off, image, gauge, gif ou layout",
                ));
            };
            match *action {
                // ⚠️ **Une commande par changement, et un seul champ libre par
                // ligne.** Une ligne unique qui porterait à la fois un chemin de
                // fond et un libellé de champ serait ambiguë au premier espace :
                // rien ne dirait où finit le chemin et où commence le texte.
                "layout" => {
                    let [sous_action, apres @ ..] = reste else {
                        return Ok(Request::Screen(ScreenAction::LayoutState));
                    };
                    match *sous_action {
                        "off" => {
                            if !apres.is_empty() {
                                return Err(mauvais(
                                    "« screen layout off » n'attend aucun argument",
                                ));
                            }
                            Ok(Request::Screen(ScreenAction::LayoutOff))
                        }
                        "vide" => {
                            let [ancre] = apres else {
                                return Err(mauvais(
                                    "« screen layout vide » attend une ancre, et elle seule",
                                ));
                            };
                            let ancre = Ancre::depuis_slug(ancre)
                                .map_err(|erreur| mauvais(&erreur.to_string()))?;
                            Ok(Request::Screen(ScreenAction::LayoutVide(ancre)))
                        }
                        // Le fond porte un chemin : dernier champ de la ligne,
                        // et il va jusqu'au bout, espaces comprises.
                        "fond" => {
                            let brut = apres_n_mots(line, 3).unwrap_or("");
                            let fond =
                                Fond::decoder(brut).map_err(|erreur| mauvais(&erreur.raison))?;
                            if let Fond::Image(chemin) = &fond
                                && !chemin.starts_with('/')
                            {
                                return Err(mauvais(&format!(
                                    "chemin « {chemin} » relatif : le démon ne partage ni le \
                                     répertoire courant ni le foyer de son client, il faut un \
                                     chemin absolu"
                                )));
                            }
                            Ok(Request::Screen(ScreenAction::LayoutFond(fond)))
                        }
                        // Le libellé aussi : « champ haut temp <sonde> CPU
                        // gauche » porte quatre champs, pas cinq.
                        "champ" => {
                            let [ancre, ..] = apres else {
                                return Err(mauvais(
                                    "« screen layout champ » attend une ancre puis une source : \
                                     « temp <sonde> [libellé] » ou « texte <libellé> »",
                                ));
                            };
                            let ancre = Ancre::depuis_slug(ancre)
                                .map_err(|erreur| mauvais(&erreur.to_string()))?;
                            let brut = apres_n_mots(line, 4).unwrap_or("");
                            let source =
                                Source::decoder(brut).map_err(|erreur| mauvais(&erreur.raison))?;
                            Ok(Request::Screen(ScreenAction::LayoutChamp(ancre, source)))
                        }
                        autre => Err(mauvais(&format!(
                            "« screen layout {autre} » inconnu : fond, champ, vide ou off"
                        ))),
                    }
                }
                "state" | "off" => {
                    if !reste.is_empty() {
                        return Err(mauvais(&format!(
                            "« screen {action} » n'attend aucun argument"
                        )));
                    }
                    Ok(Request::Screen(if *action == "state" {
                        ScreenAction::State
                    } else {
                        ScreenAction::Off
                    }))
                }
                "brightness" => {
                    let [valeur] = reste else {
                        return Err(mauvais(
                            "« screen brightness » attend une luminosité, de 0 à 100, et elle \
                             seule",
                        ));
                    };
                    Ok(Request::Screen(ScreenAction::Brightness(luminosite(
                        valeur,
                    )?)))
                }
                "gauge" => {
                    // Une sonde est un **jeton**, à l'inverse d'un chemin : elle
                    // ne vient pas d'une frappe humaine mais du matériel, et le
                    // reste du protocole la transporte déjà ainsi
                    // (`ResponseLine::Temp`). Un second mot derrière elle est
                    // une faute, pas un nom à rallonge.
                    let [sonde] = reste else {
                        return Err(mauvais(
                            "« screen gauge » attend une sonde, et elle seule — par exemple \
                             « screen gauge kraken2023elite:coolant »",
                        ));
                    };
                    Ok(Request::Screen(ScreenAction::Gauge((*sonde).to_owned())))
                }
                "image" | "gif" => {
                    // ⚠️ Le chemin est le **dernier champ de sa ligne**, et va
                    // jusqu'au bout, espaces comprises — la règle du dernier
                    // champ de #17. On repart donc de `line`, que
                    // `split_whitespace` a déjà fusionné.
                    //
                    // Partout ailleurs, un champ mal cadré donne une commande
                    // refusée. Ici, `screen image /home/nico/photos anciennes/a.png`
                    // coupé au premier blanc donnerait `/home/nico/photos` : un
                    // chemin parfaitement lisible, qui n'est pas celui qu'on
                    // visait, et que le démon afficherait sans un mot.
                    let chemin = apres_l_action(line).unwrap_or("");
                    if chemin.is_empty() {
                        return Err(mauvais(&format!(
                            "« screen {action} » attend un chemin de fichier absolu"
                        )));
                    }
                    if !chemin.starts_with('/') {
                        return Err(mauvais(&format!(
                            "chemin « {chemin} » relatif : le démon ne partage ni le répertoire \
                             courant ni le foyer de son client, il faut un chemin absolu"
                        )));
                    }
                    let chemin = chemin.to_owned();
                    Ok(Request::Screen(if *action == "image" {
                        ScreenAction::Image(chemin)
                    } else {
                        ScreenAction::Gif(chemin)
                    }))
                }
                autre => Err(mauvais(&format!(
                    "action « {autre} » inconnue : state, brightness, off, image, gauge, gif ou \
                     layout"
                ))),
            }
        }

        "profil" => {
            let [action, ..] = arguments.as_slice() else {
                return Err(mauvais("attend une action : list, save, load ou drop"));
            };
            match *action {
                "list" => {
                    if arguments.len() > 1 {
                        return Err(mauvais("« profil list » n'attend aucun argument"));
                    }
                    Ok(Request::Profil(ProfilAction::List))
                }
                "save" | "load" | "drop" => {
                    // ⚠️ Le nom est le **dernier champ de sa ligne**, et va
                    // jusqu'au bout, espaces comprises — la règle de #17, celle
                    // qui vaut déjà pour un chemin d'image. « soirée d'été » et
                    // « LAN party » sont des noms légitimes ; coupés au premier
                    // blanc, ils désigneraient « soirée » et « LAN », deux
                    // profils qui n'existent pas.
                    let brut = apres_l_action(line).unwrap_or("");
                    let nom =
                        NomProfil::nouveau(brut).map_err(|erreur| mauvais(&erreur.to_string()))?;
                    Ok(Request::Profil(match *action {
                        "save" => ProfilAction::Save(nom),
                        "load" => ProfilAction::Load(nom),
                        _ => ProfilAction::Drop(nom),
                    }))
                }
                autre => Err(mauvais(&format!(
                    "action « {autre} » inconnue : list, save, load ou drop"
                ))),
            }
        }

        "paint" => {
            let [cible, couleurs] = arguments[..] else {
                return Err(mauvais(
                    "attend une cible et des couleurs séparées par des virgules, par exemple \
                     « paint slot:0 ff0000,00ff00,… »",
                ));
            };
            let target = cible_eclairage(cible).map_err(|raison| mauvais(&raison))?;
            let attendues = match target {
                LightTarget::Fan(_) => LEDS_PER_FAN as usize,
                LightTarget::RamSlot(_) => ram::LEDS_PER_STICK,
                LightTarget::All | LightTarget::Fans | LightTarget::Ram => {
                    return Err(mauvais(
                        "vise une seule cible : « fan:<position> » ou « slot:<0-3> ». Une liste de \
                         couleurs ne dit pas laquelle va où",
                    ));
                }
            };
            let mut liste = Vec::with_capacity(attendues);
            for brut in couleurs.split(',') {
                liste.push(couleur_hex(brut).map_err(|raison| mauvais(&raison))?);
            }
            if liste.len() != attendues {
                return Err(mauvais(&format!(
                    "« {cible} » a {attendues} LED, et {} couleur(s) ont été données",
                    liste.len()
                )));
            }
            Ok(Request::Paint {
                target,
                couleurs: liste,
            })
        }

        "light" => {
            let [cible, couleur] = arguments[..] else {
                return Err(mauvais(
                    "attend une cible et une couleur, par exemple « light all ff00ff »",
                ));
            };
            Ok(Request::Light {
                target: cible_eclairage(cible).map_err(|raison| mauvais(&raison))?,
                color: couleur_hex(couleur).map_err(|raison| mauvais(&raison))?,
            })
        }

        "animate" => {
            let Some((nom, suite)) = arguments.split_first() else {
                return Err(mauvais(
                    "attend un nom d'animation, ou « off » pour l'arrêter",
                ));
            };
            Ok(Request::Animate {
                name: (*nom != ANIMATION_OFF).then(|| (*nom).to_owned()),
                reglages: paires(suite).map_err(|raison| mauvais(&raison))?,
            })
        }

        "geometry" => {
            // Le premier argument est la cible **sauf** s'il porte un `=`, auquel
            // cas c'est déjà un réglage. Sans cette règle, `geometry angle=90`
            // ferait chercher un ventilateur nommé « angle=90 », et le message
            // d'erreur enverrait sur une fausse piste.
            let (cible, suite) = match arguments.split_first() {
                Some((premier, suite)) if !premier.contains('=') => {
                    (Some((*premier).to_owned()), suite)
                }
                _ => (None, &arguments[..]),
            };
            Ok(Request::Geometry {
                cible,
                reglages: paires(suite).map_err(|raison| mauvais(&raison))?,
            })
        }

        "fan" => {
            let action = match arguments[..] {
                [_, "auto"] => FanAction::Auto,
                [_, "pwm", brut] => FanAction::Pwm(consigne(brut).map_err(|r| mauvais(&r))?),
                [_] | [_, _] => {
                    return Err(mauvais("attend « pwm <0-100> » ou « auto »"));
                }
                [] => return Err(mauvais("attend un nom de canal")),
                _ => return Err(mauvais("attend « pwm <0-100> » ou « auto »")),
            };
            Ok(Request::Fan {
                channel: arguments[0].to_owned(),
                action,
            })
        }

        autre => Err(RequestError::UnknownVerb {
            verb: autre.to_owned(),
        }),
    }
}

/// Encode une requête, sans le `\n` final.
///
/// `parse_request(&encode_request(&r)) == Ok(r)` pour toute requête — à la
/// réserve près d'une animation nommée [`ANIMATION_OFF`], que le catalogue ne
/// doit pas contenir.
pub fn encode_request(request: &Request) -> String {
    match request {
        Request::Status => "status".to_owned(),
        Request::Light { target, color } => {
            format!("light {} {}", cible_ecrite(*target), hexa(*color))
        }
        Request::Animate { name, reglages } => ecrire_paires(
            format!(
                "animate {}",
                jeton(name.as_deref().unwrap_or(ANIMATION_OFF))
            ),
            reglages,
        ),
        Request::Fan { channel, action } => match action {
            FanAction::Auto => format!("fan {channel} auto"),
            FanAction::Pwm(percent) => format!("fan {channel} pwm {percent}"),
        },
        // Le nom est le dernier champ : il n'est ni neutralisé ni cité, ses
        // espaces étant justement ce qu'il faut transmettre. Il a traversé
        // `NomProfil::nouveau`, qui a déjà refusé tout caractère de contrôle.
        Request::Profil(action) => match action {
            ProfilAction::List => "profil list".to_owned(),
            ProfilAction::Save(nom) => format!("profil save {nom}"),
            ProfilAction::Load(nom) => format!("profil load {nom}"),
            ProfilAction::Drop(nom) => format!("profil drop {nom}"),
        },
        Request::Geometry { cible, reglages } => ecrire_paires(
            match cible {
                Some(cible) => format!("geometry {}", jeton(cible)),
                None => "geometry".to_owned(),
            },
            reglages,
        ),
        Request::Paint { target, couleurs } => format!(
            "paint {} {}",
            cible_ecrite(*target),
            couleurs
                .iter()
                .map(|couleur| hexa(*couleur))
                .collect::<Vec<String>>()
                .join(",")
        ),
        Request::Lighting => "lighting".to_owned(),
        Request::Screen(action) => match action {
            ScreenAction::State => "screen state".to_owned(),
            ScreenAction::Off => "screen off".to_owned(),
            ScreenAction::Brightness(pourcent) => format!("screen brightness {pourcent}"),
            // Le chemin garde ses espaces et perd ses caractères de contrôle :
            // les premières font partie du nom du fichier, les seconds
            // scinderaient la commande en deux.
            ScreenAction::Image(chemin) => format!("screen image {}", sans_controle(chemin)),
            ScreenAction::Gif(chemin) => format!("screen gif {}", sans_controle(chemin)),
            // La sonde, elle, est un jeton : ses blancs tombent aussi.
            ScreenAction::Gauge(sonde) => format!("screen gauge {}", sur_une_ligne(sonde)),
            ScreenAction::LayoutState => "screen layout".to_owned(),
            ScreenAction::LayoutOff => "screen layout off".to_owned(),
            ScreenAction::LayoutVide(ancre) => format!("screen layout vide {}", ancre.slug()),
            // Le fond et la source finissent la ligne : leurs espaces restent
            // — chemin ou libellé —, leurs caractères de contrôle tombent.
            ScreenAction::LayoutFond(fond) => {
                format!("screen layout fond {}", sans_controle(&fond.encoder()))
            }
            ScreenAction::LayoutChamp(ancre, source) => format!(
                "screen layout champ {} {}",
                ancre.slug(),
                sans_controle(&source.encoder())
            ),
        },
        Request::ZoneList => "zone list".to_owned(),
        Request::ZoneSet { nom, cibles } => format!(
            "zone set {} {}",
            sur_une_ligne(nom),
            cibles
                .iter()
                .map(|cible| cible.slug())
                .collect::<Vec<String>>()
                .join(",")
        ),
        Request::ZoneDrop { nom } => format!("zone drop {}", sur_une_ligne(nom)),
        Request::ZoneLight { nom, couleur } => {
            format!("zone light {} {}", sur_une_ligne(nom), hexa(*couleur))
        }
        Request::ZoneAnim {
            nom,
            animation,
            reglages,
        } => match animation {
            None => format!("zone anim {} off", sur_une_ligne(nom)),
            Some(animation) => {
                let mut ligne = format!(
                    "zone anim {} {}",
                    sur_une_ligne(nom),
                    sur_une_ligne(animation)
                );
                for (cle, valeur) in reglages {
                    ligne.push_str(&format!(
                        " {}={}",
                        sur_une_ligne(cle),
                        sur_une_ligne(valeur)
                    ));
                }
                ligne
            }
        },
        Request::Watch => "watch".to_owned(),
    }
}

/// Décode des arguments `clé=valeur`.
///
/// Le protocole **transporte** : il ne sait pas quelles clés une animation
/// accepte, et n'a pas à le savoir. Ce qu'il vérifie est le seul contrôle
/// possible sans cette connaissance, et il vaut la peine — une paire sans `=`
/// est une faute de frappe (`couleur ff00ff`, l'espace au lieu du signe), une
/// valeur vide vient d'une variable de shell non substituée. Avalées en
/// silence, les deux donneraient une animation réglée par personne.
fn paires(arguments: &[&str]) -> Result<Vec<(String, String)>, String> {
    let mut paires = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let Some((cle, valeur)) = argument.split_once('=') else {
            return Err(format!(
                "« {argument} » n'est pas un réglage : il faut la forme clé=valeur"
            ));
        };
        if cle.is_empty() || valeur.is_empty() {
            return Err(format!(
                "« {argument} » n'est pas un réglage : ni la clé ni la valeur ne peuvent être vides"
            ));
        }
        paires.push((cle.to_owned(), valeur.to_owned()));
    }
    Ok(paires)
}

/// Écrit des paires derrière un début de ligne, dans l'ordre reçu.
///
/// L'ordre est celui de la frappe et il ne bouge pas : c'est lui qui décide
/// quelle clé un message d'erreur nommera en premier, et un encodeur qui
/// trierait casserait l'aller-retour.
fn ecrire_paires(mut ligne: String, reglages: &[(String, String)]) -> String {
    for (cle, valeur) in reglages {
        ligne.push(' ');
        ligne.push_str(&jeton(cle));
        ligne.push('=');
        ligne.push_str(&jeton(valeur));
    }
    ligne
}

fn cible_eclairage(brut: &str) -> Result<LightTarget, String> {
    match brut {
        "all" => Ok(LightTarget::All),
        "fans" => Ok(LightTarget::Fans),
        "ram" => Ok(LightTarget::Ram),
        _ => {
            if let Some(slug) = brut.strip_prefix("fan:") {
                return Position::from_slug(slug)
                    .map(LightTarget::Fan)
                    .map_err(|e| e.to_string());
            }
            if let Some(numero) = brut.strip_prefix("slot:") {
                let slot: usize = numero
                    .parse()
                    .map_err(|_| format!("barrette « {numero} » invalide"))?;
                if slot >= SLOT_COUNT {
                    return Err(format!(
                        "barrette {slot} inconnue : les barrettes vont de 0 à {}",
                        SLOT_COUNT - 1
                    ));
                }
                return Ok(LightTarget::RamSlot(slot));
            }
            Err(format!(
                "cible « {brut} » inconnue : attendu all, fans, ram, fan:<position> ou slot:<0-3>"
            ))
        }
    }
}

/// L'inverse de [`cible_eclairage`].
fn cible_ecrite(target: LightTarget) -> String {
    match target {
        LightTarget::All => "all".to_owned(),
        LightTarget::Fans => "fans".to_owned(),
        LightTarget::Fan(position) => format!("fan:{}", position.slug()),
        LightTarget::Ram => "ram".to_owned(),
        LightTarget::RamSlot(slot) => format!("slot:{slot}"),
    }
}

/// Ce qui reste d'une ligne après son verbe et son action, blancs de tête ôtés.
///
/// `None` quand la ligne n'a pas deux mots. Les blancs **internes** sont gardés
/// tels quels : ce sont ceux d'un chemin, et les fusionner désignerait un
/// fichier qui n'existe pas.
fn apres_l_action(line: &str) -> Option<&str> {
    apres_n_mots(line, 2)
}

/// Ce qui reste d'une ligne après ses `n` premiers mots.
///
/// La composition de l'écran a des commandes plus profondes que « verbe action »
/// — `screen layout champ haut temp <sonde> <libellé>` en a quatre avant son
/// champ libre —, et le dernier champ y garde la même règle : il va jusqu'au
/// bout, espaces comprises.
fn apres_n_mots(line: &str, n: usize) -> Option<&str> {
    let mut reste = line.trim_start();
    for _ in 0..n {
        let (_, suite) = reste.split_once(char::is_whitespace)?;
        reste = suite.trim_start();
    }
    Some(reste)
}

/// Une luminosité d'écran, de 0 à 100 pour cent.
///
/// ⚠️ **Jamais écrêtée.** Ramener 101 à 100 en silence rendrait la fenêtre
/// incapable de distinguer « réglé à 100 » d'une valeur corrigée derrière son
/// dos, et masquerait un curseur gradué dans la mauvaise unité — 0-255 au lieu
/// de 0-100 passerait pour un curseur qui marche.
///
/// Les deux fautes se disent séparément : « ce n'est pas un nombre » et « ce
/// nombre est trop grand » ne se corrigent pas de la même façon.
fn luminosite(brut: &str) -> Result<u8, RequestError> {
    let mauvais = |raison: String| RequestError::BadArgument {
        verb: "screen".to_owned(),
        reason: raison,
    };
    let valeur: u32 = brut.parse().map_err(|_| {
        mauvais(format!(
            "luminosité « {brut} » invalide : attendu un entier de 0 à {LUMINOSITE_MAX}"
        ))
    })?;
    if valeur > u32::from(LUMINOSITE_MAX) {
        return Err(mauvais(format!(
            "luminosité {valeur} hors bornes : l'écran va de 0 à {LUMINOSITE_MAX} pour cent"
        )));
    }
    u8::try_from(valeur).map_err(|_| mauvais(format!("luminosité {valeur} hors bornes")))
}

/// Neutralise ce qui casserait le cadrage d'une ligne, **et rien d'autre**.
///
/// À la différence de [`sur_une_ligne`], les espaces survivent : un chemin en
/// porte, et les remplacer le ferait pointer ailleurs. Seuls les caractères de
/// contrôle tombent — un fichier peut s'appeler `a\nlight all ffffff` sur ext4,
/// et `screen image` ne doit pas allumer le boîtier en blanc.
fn sans_controle(brut: &str) -> String {
    brut.chars()
        .map(|c| if c.is_control() { '_' } else { c })
        .collect()
}

/// Analyse une couleur **strictement** : six chiffres hexadécimaux, rien d'autre.
///
/// Plus strict que `Rgb::from_hex`, qui tolère le `#` de tête parce qu'un
/// humain le tape. Ici l'émetteur est un programme : tolérer deux écritures
/// pour une même couleur, c'est se priver de l'aller-retour exact.
fn couleur_hex(brut: &str) -> Result<Rgb, String> {
    if brut.len() != 6 || !brut.bytes().all(|o| o.is_ascii_hexdigit()) {
        return Err(format!(
            "couleur « {brut} » invalide : attendu six chiffres hexadécimaux, sans « # »"
        ));
    }
    Rgb::from_hex(brut).map_err(|e| e.to_string())
}

fn consigne(brut: &str) -> Result<u8, String> {
    // Le type ne suffit pas : 250 tient dans un `u8`. C'est exactement le cas
    // qu'un décodeur laisserait passer en se contentant de `parse::<u8>()`.
    let valeur: u16 = brut
        .parse()
        .map_err(|_| format!("consigne « {brut} » invalide : attendu un entier de 0 à 100"))?;
    if valeur > 100 {
        return Err(format!("consigne {valeur} hors bornes : attendu 0 à 100"));
    }
    Ok(valeur as u8)
}

// ---------------------------------------------------------------------------
// Réponses
// ---------------------------------------------------------------------------

/// Une ligne de réponse du démon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseLine {
    /// `chan <canal> <position|-> <rpm|-> <pwm|-> <mode> <oui|non>`
    Channel {
        channel: String,
        position: Option<Position>,
        rpm: Option<u32>,
        pwm: Option<u8>,
        /// Un **seul jeton**, sans espace : c'est l'arité de la ligne qui dit
        /// si le drapeau qui suit est présent (#50).
        mode: String,
        /// Ce canal sait-il rendre la main à une régulation automatique.
        ///
        /// `oui` ou `non`, jamais `-` : `-` veut dire « absent » dans cette
        /// grammaire (#17), et un booléen n'est pas une absence. Jamais `0`/`1`
        /// non plus, qui voisineraient avec `rpm` et `pwm`, où un chiffre a un
        /// tout autre sens.
        ///
        /// **Son absence vaut « non »** : un démon d'avant #50 ne sait rien de
        /// la question, et la fenêtre doit alors cacher le bouton plutôt que
        /// d'en montrer un qui ne peut qu'échouer.
        sait_faire_auto: bool,
    },
    /// `temp <capteur> <millidegres>`
    ///
    /// En millidegrés **entiers signés**, comme hwmon les publie. Pas de
    /// flottant : un aller-retour texte sur un `f32` ne rend pas toujours le
    /// même nombre, et un protocole doit être exact.
    Temp { sensor: String, millidegrees: i32 },
    /// `unreadable <sujet> <raison>` — la valeur existe mais n'a pas pu être lue.
    ///
    /// ⚠️ Ni omise, ni remplacée par zéro : un canal illisible affiché à
    /// 0 tr/min est un mensonge, et un canal omis fait croire qu'il n'existe pas.
    Unreadable { subject: String, reason: String },
    /// `geom <position-slug> <angle> <sens>` — une ligne de géométrie.
    Geom {
        position: String,
        angle: u16,
        sens: String,
    },
    /// `light <cible> <rrggbb>` — une couleur fixe de l'état courant.
    ///
    /// La cible s'écrit comme dans la requête : `fan:<position>` ou
    /// `slot:<0-3>`. Les cibles collectives — `all`, `fans`, `ram` — n'y
    /// paraissent jamais : l'état est ce que chaque cible porte, pas la
    /// commande qui l'y a mise.
    Light { cible: String, couleur: Rgb },
    /// `anim <nom> [clé=valeur…]` — l'animation en cours.
    ///
    /// Absente quand l'éclairage est fixe. Les réglages sont transportés bruts,
    /// comme dans `Request::Animate` : le protocole ne sait pas ce qu'une
    /// animation accepte, et n'a pas à le savoir.
    Anim {
        nom: String,
        reglages: Vec<(String, String)>,
    },
    /// `frame <cible> <rrggbb>,<rrggbb>,…` — les couleurs d'une cible dans
    /// l'image courante.
    ///
    /// Huit couleurs pour un ventilateur, onze pour une barrette, dans l'ordre
    /// du matériel. Séparées par des virgules et non par des espaces : une
    /// ligne de onze champs se relit mal, et le découpage en jetons du reste du
    /// protocole reste ainsi valable.
    Frame { cible: String, couleurs: Vec<Rgb> },
    /// `zone <nom> <cible>,<cible>,…` — la composition d'une zone.
    ///
    /// Les cibles sont toujours rendues **LED par LED**, jamais regroupées en
    /// organes : une zone peut n'en tenir que la moitié, et rendre `fan:arriere`
    /// pour six LED sur huit serait faux.
    Zone { nom: String, cibles: Vec<Led> },
    /// `zone-light <nom> <rrggbb>` — la zone porte cette couleur fixe.
    ZoneLight { nom: String, couleur: Rgb },
    /// `zone-anim <nom> <animation> [clé=valeur…]` — la zone porte cette
    /// animation.
    ///
    /// Absente quand la zone est transparente ou fixe.
    ZoneAnim {
        nom: String,
        animation: String,
        reglages: Vec<(String, String)>,
    },
    /// `screen <0-100> <affichage>` — l'état de la dalle.
    ///
    /// L'affichage s'écrit `rien`, `gauge:<sonde>`, `image:<chemin>`,
    /// `gif:<chemin>` ou `layout`.
    ///
    /// ⚠️ Une composition s'y réduit au **jeton** `layout` : son fond et ses
    /// champs ne tiendraient pas sur une ligne, et ils viennent sur les leurs.
    Screen { luminosite: u8, affichage: String },
    /// `layout <fond>` — le fond de la composition courante.
    ///
    /// Le fond s'écrit comme dans la requête : `noir` ou `image <chemin>`.
    Layout { fond: String },
    /// `layout-champ <ancre> <source>` — un champ de la composition courante.
    ///
    /// Une ligne par champ, dans l'ordre des ancres. Aucune quand la dalle ne
    /// porte pas de composition — leur absence *est* l'information.
    LayoutChamp { ancre: String, source: String },
    /// `profil <etat> <nom>` — ce qu'il est advenu d'une ambiance.
    ///
    /// `etat` est un **seul jeton** : `connu` pour une ligne de `profil list`,
    /// `cree`, `ecrase`, `applique` ou `oublie` pour ce qu'une commande vient de
    /// faire. C'est ainsi que « `profil save` écrase **et le dit** » se tient
    /// sans passer par `err`, qui terminerait la réponse en échec là où
    /// l'écrasement a réussi.
    ///
    /// Le nom est en **dernier champ** : il a le droit de porter des espaces,
    /// comme celui qu'on a tapé pour l'enregistrer.
    Profil { etat: String, nom: String },
    /// `end` — succès, fin de réponse.
    End,
    /// `err <message>` — échec, fin de réponse.
    Error { message: String },
}

impl ResponseLine {
    /// Vraie pour [`ResponseLine::End`] et [`ResponseLine::Error`] — les deux
    /// seules lignes qui terminent une réponse.
    pub fn is_terminal(&self) -> bool {
        matches!(self, ResponseLine::End | ResponseLine::Error { .. })
    }
}

/// Une ligne de réponse n'a pas pu être comprise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseError {
    pub line: String,
    pub reason: String,
}

impl fmt::Display for ResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "réponse « {} » illisible : {}", self.line, self.reason)
    }
}

impl std::error::Error for ResponseError {}

/// Encode une ligne de réponse, sans le `\n` final.
///
/// **Neutralise** les blancs et les caractères de contrôle des champs qui ne
/// sont pas en fin de ligne, et les seuls caractères de contrôle du champ final
/// — qui, lui, a le droit de porter des espaces puisque rien ne le suit.
///
/// Neutraliser plutôt que refuser : ces champs viennent du matériel, et perdre
/// un canal de la télémétrie parce que son nom porte un caractère bizarre
/// serait pire que l'afficher avec un souligné. Voir l'en-tête du module.
pub fn encode_response_line(line: &ResponseLine) -> String {
    match line {
        ResponseLine::Channel {
            channel,
            position,
            rpm,
            pwm,
            mode,
            sait_faire_auto,
        } => format!(
            "chan {} {} {} {} {} {}",
            jeton(channel),
            position.map_or_else(|| ABSENT.to_owned(), Position::slug),
            rpm.map_or_else(|| ABSENT.to_owned(), |v| v.to_string()),
            pwm.map_or_else(|| ABSENT.to_owned(), |v| v.to_string()),
            // ⚠️ `jeton` et non `reste` depuis #50. Le mode n'est plus le
            // dernier champ, donc il n'a plus droit à ses espaces : c'est
            // l'arité de la ligne qui distingue un démon d'avant d'un démon
            // d'après, et un mode à espaces la rendrait indécidable. Les
            // libellés de `hwmon::Mode` sont devenus des mots composés en
            // conséquence.
            jeton(mode),
            if *sait_faire_auto { "oui" } else { "non" },
        ),
        ResponseLine::Temp {
            sensor,
            millidegrees,
        } => format!("temp {} {millidegrees}", jeton(sensor)),
        ResponseLine::Unreadable { subject, reason } => {
            format!("unreadable {} {}", jeton(subject), reste(reason))
        }
        // Quatre jetons, et le sens en est un : son vocabulaire est fermé
        // (« horaire », « antihoraire »), pas du texte libre. Lui appliquer la
        // règle du dernier champ ferait passer « geom radiateur-haut 90 horaire
        // bidule » pour une orientation valide.
        ResponseLine::Geom {
            position,
            angle,
            sens,
        } => format!("geom {} {angle} {}", jeton(position), jeton(sens)),
        ResponseLine::Light { cible, couleur } => {
            format!("light {} {}", jeton(cible), hexa(*couleur))
        }
        ResponseLine::Anim { nom, reglages } => {
            ecrire_paires(format!("anim {}", jeton(nom)), reglages)
        }
        ResponseLine::Frame { cible, couleurs } => format!(
            "frame {} {}",
            jeton(cible),
            couleurs
                .iter()
                .map(|couleur| hexa(*couleur))
                .collect::<Vec<String>>()
                .join(",")
        ),
        ResponseLine::Zone { nom, cibles } => format!(
            "zone {} {}",
            sur_une_ligne(nom),
            cibles
                .iter()
                .map(|cible| cible.slug())
                .collect::<Vec<String>>()
                .join(",")
        ),
        ResponseLine::ZoneLight { nom, couleur } => {
            format!("zone-light {} {}", sur_une_ligne(nom), hexa(*couleur))
        }
        ResponseLine::ZoneAnim {
            nom,
            animation,
            reglages,
        } => {
            let mut ligne = format!(
                "zone-anim {} {}",
                sur_une_ligne(nom),
                sur_une_ligne(animation)
            );
            for (cle, valeur) in reglages {
                ligne.push_str(&format!(
                    " {}={}",
                    sur_une_ligne(cle),
                    sur_une_ligne(valeur)
                ));
            }
            ligne
        }
        // L'affichage est le dernier champ, et porte un chemin : ses espaces
        // restent, ses caractères de contrôle tombent.
        ResponseLine::Screen {
            luminosite,
            affichage,
        } => format!("screen {luminosite} {}", sans_controle(affichage)),
        // Le fond et la source sont les derniers champs de leur ligne, et
        // portent chemin ou libellé : espaces gardées, contrôles neutralisés.
        ResponseLine::Layout { fond } => format!("layout {}", sans_controle(fond)),
        ResponseLine::LayoutChamp { ancre, source } => {
            format!("layout-champ {} {}", jeton(ancre), sans_controle(source))
        }
        // Le nom est le dernier champ, et porte des espaces légitimes : ses
        // caractères de contrôle tombent, ses espaces restent. L'état, lui, est
        // un jeton, et se neutralise comme tel.
        ResponseLine::Profil { etat, nom } => {
            format!("profil {} {}", jeton(etat), sans_controle(nom))
        }
        ResponseLine::End => "end".to_owned(),
        ResponseLine::Error { message } => format!("err {}", reste(message)),
    }
}

/// Analyse une ligne de réponse.
///
/// # Erreurs
///
/// [`ResponseError`] si le préfixe est inconnu ou si les champs ne s'accordent
/// pas au préfixe.
pub fn parse_response_line(line: &str) -> Result<ResponseLine, ResponseError> {
    let illisible = |raison: &str| ResponseError {
        line: line.to_owned(),
        reason: raison.to_owned(),
    };

    // `err` et `unreadable` portent un dernier champ à espaces : on ne découpe
    // donc que ce qui précède.
    if line == "end" {
        return Ok(ResponseLine::End);
    }
    if let Some(message) = line.strip_prefix("err ") {
        return Ok(ResponseLine::Error {
            message: message.to_owned(),
        });
    }
    if line == "err" {
        return Ok(ResponseLine::Error {
            message: String::new(),
        });
    }
    // `screen` porte lui aussi un dernier champ à espaces — un chemin —, et se
    // découpe donc avant les lignes à champs fixes. ⚠️ Le préfixe est
    // « screen␣ » : `screen state` est une **requête**, et la lire ici comme un
    // état de dalle donnerait une luminosité tirée du mot « state ».
    if let Some(reste) = line.strip_prefix("screen ") {
        let (brut, affichage) = reste
            .split_once(' ')
            .ok_or_else(|| illisible("« screen » attend une luminosité et un affichage"))?;
        let luminosite: u32 = brut.parse().map_err(|_| {
            illisible(&format!(
                "luminosité « {brut} » invalide : attendu un entier de 0 à {LUMINOSITE_MAX}"
            ))
        })?;
        if luminosite > u32::from(LUMINOSITE_MAX) {
            return Err(illisible(&format!(
                "luminosité {luminosite} hors bornes : l'écran va de 0 à {LUMINOSITE_MAX}"
            )));
        }
        return Ok(ResponseLine::Screen {
            luminosite: u8::try_from(luminosite).unwrap_or(LUMINOSITE_MAX),
            affichage: affichage.to_owned(),
        });
    }

    // `layout` et `layout-champ` portent eux aussi un dernier champ à espaces —
    // un chemin de fond, un libellé de champ. ⚠️ `layout-champ` se teste
    // **avant** `layout` : « layout-champ » commence par « layout », et l'ordre
    // inverse lirait le tiret comme le début d'un fond.
    if let Some(reste) = line.strip_prefix("layout-champ ") {
        let (ancre, source) = reste
            .split_once(' ')
            .ok_or_else(|| illisible("« layout-champ » attend une ancre et une source"))?;
        if source.trim().is_empty() {
            return Err(illisible("« layout-champ » attend une source"));
        }
        return Ok(ResponseLine::LayoutChamp {
            ancre: ancre.to_owned(),
            source: source.to_owned(),
        });
    }
    if let Some(fond) = line.strip_prefix("layout ") {
        if fond.trim().is_empty() {
            return Err(illisible("« layout » attend un fond"));
        }
        return Ok(ResponseLine::Layout {
            fond: fond.to_owned(),
        });
    }

    // `profil` porte lui aussi un dernier champ à espaces — le nom. ⚠️ Le
    // préfixe est « profil␣ », l'état est un jeton, et le nom va jusqu'au bout :
    // `profil connu soirée d'été` porte deux champs, pas quatre.
    if let Some(reste) = line.strip_prefix("profil ") {
        let (etat, nom) = reste
            .split_once(' ')
            .ok_or_else(|| illisible("« profil » attend un état et un nom"))?;
        if nom.trim().is_empty() {
            return Err(illisible("« profil » attend un nom"));
        }
        return Ok(ResponseLine::Profil {
            etat: etat.to_owned(),
            nom: nom.to_owned(),
        });
    }

    if let Some(reste) = line.strip_prefix("unreadable ") {
        let (sujet, raison) = reste
            .split_once(' ')
            .ok_or_else(|| illisible("« unreadable » attend un sujet et une raison"))?;
        return Ok(ResponseLine::Unreadable {
            subject: sujet.to_owned(),
            reason: raison.to_owned(),
        });
    }

    // Les trois lignes de la fenêtre se découpent à part : `anim` porte un
    // nombre libre de réglages, et `frame` une liste dont la longueur dépend de
    // la cible. Les compter dans le `splitn` fixe ci-dessous les tronquerait.
    if let Some(champs) = line.strip_prefix("zone ") {
        let [nom, cibles] = champs.split(' ').collect::<Vec<&str>>()[..] else {
            return Err(illisible(
                "« zone » attend un nom et des cibles séparées par des virgules",
            ));
        };
        if nom.is_empty() {
            return Err(illisible("« zone » attend un nom avant ses cibles"));
        }
        let mut toutes = Vec::new();
        for brut in cibles.split(',') {
            // ⚠️ Une ligne de réponse ne porte que des LED précises : accepter
            // ici la forme courte « fan:arriere » ferait relire huit LED là où
            // le démon n'en avait peut-être écrit qu'une.
            match Led::depuis_slug(brut) {
                Ok(cibles) if cibles.len() == 1 => toutes.push(cibles[0]),
                Ok(_) => {
                    return Err(illisible(
                        "une ligne « zone » écrit ses cibles LED par LED, jamais par organe",
                    ));
                }
                Err(erreur) => return Err(illisible(&erreur.to_string())),
            }
        }
        return Ok(ResponseLine::Zone {
            nom: nom.to_owned(),
            cibles: toutes,
        });
    }
    if let Some(champs) = line.strip_prefix("zone-light ") {
        let [nom, couleur] = champs.split(' ').collect::<Vec<&str>>()[..] else {
            return Err(illisible("« zone-light » attend un nom et une couleur"));
        };
        if nom.is_empty() {
            return Err(illisible("« zone-light » attend un nom avant la couleur"));
        }
        return Ok(ResponseLine::ZoneLight {
            nom: nom.to_owned(),
            couleur: couleur_stricte(couleur).ok_or_else(|| illisible(HEXA))?,
        });
    }
    if let Some(champs) = line.strip_prefix("zone-anim ") {
        let mots: Vec<&str> = champs.split(' ').collect();
        let [nom, animation, reglages @ ..] = mots.as_slice() else {
            return Err(illisible("« zone-anim » attend un nom et une animation"));
        };
        if nom.is_empty() || animation.is_empty() {
            return Err(illisible("« zone-anim » attend un nom et une animation"));
        }
        return Ok(ResponseLine::ZoneAnim {
            nom: (*nom).to_owned(),
            animation: (*animation).to_owned(),
            reglages: paires(reglages).map_err(|raison| illisible(&raison))?,
        });
    }
    if let Some(champs) = line.strip_prefix("light ") {
        let [cible, couleur] = champs.split(' ').collect::<Vec<&str>>()[..] else {
            return Err(illisible(
                "« light » attend une cible et une couleur, et rien d'autre",
            ));
        };
        if cible.is_empty() {
            return Err(illisible("« light » attend une cible avant la couleur"));
        }
        return Ok(ResponseLine::Light {
            cible: cible.to_owned(),
            couleur: couleur_stricte(couleur).ok_or_else(|| illisible(HEXA))?,
        });
    }
    if let Some(champs) = line.strip_prefix("frame ") {
        let [cible, couleurs] = champs.split(' ').collect::<Vec<&str>>()[..] else {
            return Err(illisible(
                "« frame » attend une cible et des couleurs séparées par des virgules",
            ));
        };
        if cible.is_empty() {
            return Err(illisible("« frame » attend une cible avant ses couleurs"));
        }
        // Une couleur manquante n'est **jamais** complétée par du noir : ce
        // serait une LED éteinte que personne ne saurait expliquer.
        let mut liste = Vec::new();
        for brut in couleurs.split(',') {
            liste.push(couleur_stricte(brut).ok_or_else(|| illisible(HEXA))?);
        }
        return Ok(ResponseLine::Frame {
            cible: cible.to_owned(),
            couleurs: liste,
        });
    }
    if let Some(champs) = line.strip_prefix("anim ") {
        let mut mots = champs.split(' ');
        let nom = mots.next().unwrap_or_default();
        if nom.is_empty() {
            return Err(illisible("« anim » attend un nom d'animation"));
        }
        let restants: Vec<&str> = mots.collect();
        return Ok(ResponseLine::Anim {
            nom: nom.to_owned(),
            reglages: paires(&restants).map_err(|raison| illisible(&raison))?,
        });
    }

    // `chan` a sa propre branche depuis #50 : c'est le **nombre de champs** qui
    // dit si le drapeau « sait faire auto » est là — cinq pour un démon d'avant,
    // six pour un démon d'après. Un découpage partagé avec les autres lignes
    // n'aurait pas su compter.
    if let Some(champs) = line.strip_prefix("chan ") {
        let mots: Vec<&str> = champs.split(' ').collect();
        let (canal, position, rpm, pwm, mode, sait_faire_auto) = match mots[..] {
            // Cinq champs : un démon d'avant #50. Il ne sait rien de la
            // question, donc on répond « non » — cacher un bouton qui marche se
            // répare, en montrer un qui ne peut qu'échouer est la panne même que
            // #50 corrige.
            [canal, position, rpm, pwm, mode] => (canal, position, rpm, pwm, mode, false),
            [canal, position, rpm, pwm, mode, drapeau] => {
                // ⚠️ Un sixième champ qui n'est ni `oui` ni `non` est **refusé**,
                // et non absorbé dans le mode : l'absorber ferait passer une
                // incompatibilité de protocole pour un mode exotique, et
                // l'erreur ne se verrait jamais.
                let sait = match drapeau {
                    "oui" => true,
                    "non" => false,
                    _ => {
                        return Err(illisible(
                            "« chan » attend « oui » ou « non » en dernier champ",
                        ));
                    }
                };
                (canal, position, rpm, pwm, mode, sait)
            }
            _ => return Err(illisible("« chan » attend cinq ou six champs")),
        };
        return Ok(ResponseLine::Channel {
            channel: canal.to_owned(),
            position: match position {
                ABSENT => None,
                slug => Some(Position::from_slug(slug).map_err(|e| illisible(&e.to_string()))?),
            },
            rpm: absent_ou(rpm).map_err(|r| illisible(&r))?,
            pwm: absent_ou(pwm).map_err(|r| illisible(&r))?,
            mode: mode.to_owned(),
            sait_faire_auto,
        });
    }

    let champs: Vec<&str> = line.splitn(6, ' ').collect();
    match champs[..] {
        ["temp", capteur, millidegres] => Ok(ResponseLine::Temp {
            sensor: capteur.to_owned(),
            millidegrees: millidegres.parse().map_err(|_| {
                illisible("température illisible : attendu des millidegrés entiers")
            })?,
        }),
        ["geom", position, angle, sens] => Ok(ResponseLine::Geom {
            position: (*position).to_owned(),
            angle: angle
                .parse()
                .map_err(|_| illisible("« geom » attend un angle entier en degrés"))?,
            sens: (*sens).to_owned(),
        }),
        ["light", ..] => Err(illisible("« light » attend une cible et une couleur")),
        ["frame", ..] => Err(illisible("« frame » attend une cible et des couleurs")),
        ["anim", ..] => Err(illisible("« anim » attend un nom d'animation")),
        ["chan", ..] => Err(illisible("« chan » attend cinq champs")),
        ["geom", ..] => Err(illisible(
            "« geom » attend une position, un angle et un sens",
        )),
        ["temp", ..] => Err(illisible("« temp » attend deux champs")),
        [prefixe, ..] => Err(illisible(&format!("préfixe « {prefixe} » inconnu"))),
        [] => Err(illisible("ligne vide")),
    }
}

/// Ce qu'on répond quand une couleur n'en est pas une.
const HEXA: &str = "couleur invalide : attendu six chiffres hexadécimaux, par exemple ff2080";

/// Une couleur en six chiffres hexadécimaux, telle qu'elle s'écrit sur le fil.
fn hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

/// L'inverse d'[`hexa`], **strict**.
///
/// Ni `#`, ni majuscules refusées — mais exactement six chiffres. Ces lignes-là
/// sont écrites par le démon, pas tapées : y tolérer des variantes ferait passer
/// une faute d'encodage pour une écriture valide, et le décodeur rattraperait sa
/// propre erreur au lieu de la signaler.
fn couleur_stricte(brut: &str) -> Option<Rgb> {
    if brut.len() != 6 || !brut.bytes().all(|o| o.is_ascii_hexdigit()) {
        return None;
    }
    let composante = |debut: usize| u8::from_str_radix(&brut[debut..debut + 2], 16).ok();
    Some(Rgb::new(composante(0)?, composante(2)?, composante(4)?))
}

/// Un nom de zone : un seul jeton non vide, tel qu'il est écrit.
///
/// Un seul jeton, parce que le protocole se découpe aux espaces : `zone drop ma
/// zone` supprimerait une zone nommée `ma` sans un mot. Et tel qu'il est écrit,
/// parce que la casse compte partout ailleurs dans ce protocole.
fn nom_de_zone(brut: &str) -> Result<String, String> {
    if brut.is_empty() {
        return Err("un nom de zone ne peut pas être vide".to_owned());
    }
    if brut.chars().any(char::is_whitespace) {
        return Err(format!(
            "« {brut} » : un nom de zone est un seul mot, sans espace"
        ));
    }
    if brut.len() > NOM_ZONE_MAX {
        return Err(format!(
            "nom de {} octets, maximum {NOM_ZONE_MAX}",
            brut.len()
        ));
    }
    Ok(brut.to_owned())
}

/// Rend un champ écrivable au milieu d'une ligne, quoi qu'il porte.
///
/// [`nom_de_zone`] refuse déjà les blancs à la lecture, mais une `Request` se
/// construit aussi **dans le processus**, sans passer par l'analyseur. Trois
/// façons de casser une ligne, fermées ici plutôt que chez chaque appelant :
///
/// - un **saut de ligne** couperait la commande en deux, et la seconde moitié
///   serait lue comme une commande à part entière ;
/// - une **espace** en ferait deux champs, décalant tous les suivants ;
/// - un champ **vide** disparaîtrait entre deux espaces, avec le même décalage —
///   d'où le blanc souligné qui le remplace, plutôt que rien.
///
/// Les caractères de contrôle passent par la même porte : un `NUL` au milieu
/// d'un nom ne coupe rien ici, mais il coupe chez qui lit la ligne ensuite.
fn sur_une_ligne(brut: &str) -> String {
    let propre: String = brut
        .chars()
        .map(|c| {
            if c.is_whitespace() || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    if propre.is_empty() {
        "_".to_owned()
    } else {
        propre
    }
}

/// Longueur maximale d'un nom de zone, en octets.
///
/// Assez pour « radiateur-et-plafond », court assez pour qu'une zone couvrant
/// tout le boîtier tienne dans plusieurs lignes de réponse sans que le nom pèse.
const NOM_ZONE_MAX: usize = 48;

fn absent_ou<T: std::str::FromStr>(champ: &str) -> Result<Option<T>, String> {
    if champ == ABSENT {
        return Ok(None);
    }
    champ
        .parse()
        .map(Some)
        .map_err(|_| format!("champ « {champ} » illisible"))
}

/// Un champ qui n'est pas en fin de ligne : ni blanc, ni caractère de contrôle.
fn jeton(champ: &str) -> String {
    let propre: String = champ
        .chars()
        .map(|c| {
            if c.is_whitespace() || c.is_control() {
                NEUTRE
            } else {
                c
            }
        })
        .collect();
    // Un champ vide disparaîtrait entre deux espaces et décalerait tous les
    // suivants d'un cran.
    if propre.is_empty() {
        NEUTRE.to_string()
    } else {
        propre
    }
}

/// Le champ final : les espaces sont permises, les caractères de contrôle non.
///
/// Un `\n` scinderait l'encodage en deux lignes physiques, dont la seconde
/// pourrait, elle, commencer par `end` — une fin de réponse parfaitement formée
/// qu'aucun client ne distinguerait d'une vraie.
fn reste(champ: &str) -> String {
    champ
        .chars()
        .map(|c| if c.is_control() { NEUTRE } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Tests de logique du verbe `profil` (#74).
    //!
    //! Les tests d'intention du profil vivent dans `tests/spec_profils.rs` — ils
    //! portent sur le **nom**, qui est pur. Ceux-ci portent sur son passage par
    //! le socket, que l'issue n'avait pas spécifié : elle liste quatre commandes
    //! sans fixer de forme de requête.

    use super::*;

    fn nom(saisi: &str) -> NomProfil {
        NomProfil::nouveau(saisi).expect("nom valide")
    }

    #[test]
    fn les_quatre_actions_traversent_l_aller_retour() {
        for action in [
            ProfilAction::List,
            ProfilAction::Save(nom("nuit")),
            ProfilAction::Load(nom("nuit")),
            ProfilAction::Drop(nom("nuit")),
        ] {
            let requete = Request::Profil(action);
            assert_eq!(
                parse_request(&encode_request(&requete)),
                Ok(requete.clone()),
                "{requete:?} doit se relire telle quelle"
            );
        }
    }

    #[test]
    fn un_nom_a_espaces_traverse_le_socket_entier() {
        // La règle du dernier champ (#17), celle qui vaut déjà pour un chemin
        // d'image. Coupé au premier blanc, « soirée d'été » désignerait
        // « soirée » — un profil qui n'existe pas, et que le démon dirait
        // absent sans qu'on comprenne pourquoi.
        for saisi in ["soirée d'été", "LAN party", "très très calme"] {
            let requete = Request::Profil(ProfilAction::Load(nom(saisi)));
            let ligne = encode_request(&requete);
            assert!(
                ligne.ends_with(saisi),
                "« {ligne} » doit finir par le nom entier"
            );
            assert_eq!(parse_request(&ligne), Ok(requete));
        }
    }

    #[test]
    fn un_nom_hostile_est_refuse_par_le_socket() {
        // La validation est **en amont** de toute construction de chemin : le
        // démon ne doit jamais recevoir un `Request::Profil` portant un nom
        // qu'il n'aurait plus qu'à ouvrir.
        for hostile in [
            "profil load ../../etc/shadow",
            "profil save nuit/jour",
            "profil drop ",
            "profil save",
        ] {
            let refus =
                parse_request(hostile).expect_err(&format!("« {hostile} » doit être refusé"));
            assert!(
                matches!(refus, RequestError::BadArgument { ref verb, .. } if verb == "profil"),
                "« {hostile} » doit être refusé au titre de « profil », pas d'un autre verbe : \
                 {refus:?}"
            );
        }
    }

    #[test]
    fn une_action_inconnue_est_refusee_en_donnant_les_quatre() {
        let refus = parse_request("profil sauvegarde nuit").expect_err("action inconnue");
        let RequestError::BadArgument { reason, .. } = refus else {
            panic!("attendu un mauvais argument, obtenu {refus:?}");
        };
        for connue in ["list", "save", "load", "drop"] {
            assert!(
                reason.contains(connue),
                "le refus doit citer « {connue} » : {reason}"
            );
        }
    }

    #[test]
    fn profil_list_n_attend_aucun_argument() {
        assert_eq!(
            parse_request("profil list"),
            Ok(Request::Profil(ProfilAction::List))
        );
        assert!(
            parse_request("profil list nuit").is_err(),
            "« profil list nuit » ressemble à un chargement qui n'en est pas un : le refuser \
             évite qu'il passe pour appliqué"
        );
    }

    #[test]
    fn la_ligne_de_reponse_garde_l_etat_et_le_nom() {
        for etat in ["connu", "cree", "ecrase", "applique", "oublie"] {
            let ligne = ResponseLine::Profil {
                etat: etat.to_owned(),
                nom: "soirée d'été".to_owned(),
            };
            assert_eq!(
                parse_response_line(&encode_response_line(&ligne)),
                Ok(ligne.clone()),
                "{ligne:?} doit se relire"
            );
        }

        // Une ligne sans état n'est pas décodable en état vide : ce serait un
        // nom pris pour un verbe, donc une entrée de liste inventée.
        assert!(parse_response_line("profil nuit").is_err());
        assert!(parse_response_line("profil connu ").is_err());
    }
}
