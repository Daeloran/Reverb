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

use crate::color::Rgb;
use crate::position::Position;
use crate::ram::SLOT_COUNT;

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
    /// `animate <nom>` — lance une animation ; `animate off` l'arrête.
    Animate { name: Option<String> },
    /// `fan <canal> pwm <0-100>` ou `fan <canal> auto`.
    Fan { channel: String, action: FanAction },
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
            let [nom] = arguments[..] else {
                return Err(mauvais(
                    "attend un nom d'animation, ou « off » pour l'arrêter",
                ));
            };
            Ok(Request::Animate {
                name: (nom != ANIMATION_OFF).then(|| nom.to_owned()),
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
            let cible = match target {
                LightTarget::All => "all".to_owned(),
                LightTarget::Fans => "fans".to_owned(),
                LightTarget::Fan(position) => format!("fan:{}", position.slug()),
                LightTarget::Ram => "ram".to_owned(),
                LightTarget::RamSlot(slot) => format!("slot:{slot}"),
            };
            format!(
                "light {cible} {:02x}{:02x}{:02x}",
                color.r, color.g, color.b
            )
        }
        Request::Animate { name } => {
            format!("animate {}", name.as_deref().unwrap_or(ANIMATION_OFF))
        }
        Request::Fan { channel, action } => match action {
            FanAction::Auto => format!("fan {channel} auto"),
            FanAction::Pwm(percent) => format!("fan {channel} pwm {percent}"),
        },
    }
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
    /// `chan <canal> <position|-> <rpm|-> <pwm|-> <mode>`
    Channel {
        channel: String,
        position: Option<Position>,
        rpm: Option<u32>,
        pwm: Option<u8>,
        mode: String,
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
        } => format!(
            "chan {} {} {} {} {}",
            jeton(channel),
            position.map_or_else(|| ABSENT.to_owned(), Position::slug),
            rpm.map_or_else(|| ABSENT.to_owned(), |v| v.to_string()),
            pwm.map_or_else(|| ABSENT.to_owned(), |v| v.to_string()),
            jeton(mode),
        ),
        ResponseLine::Temp {
            sensor,
            millidegrees,
        } => format!("temp {} {millidegrees}", jeton(sensor)),
        ResponseLine::Unreadable { subject, reason } => {
            format!("unreadable {} {}", jeton(subject), reste(reason))
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
    if let Some(reste) = line.strip_prefix("unreadable ") {
        let (sujet, raison) = reste
            .split_once(' ')
            .ok_or_else(|| illisible("« unreadable » attend un sujet et une raison"))?;
        return Ok(ResponseLine::Unreadable {
            subject: sujet.to_owned(),
            reason: raison.to_owned(),
        });
    }

    let champs: Vec<&str> = line.split(' ').collect();
    match champs[..] {
        ["chan", canal, position, rpm, pwm, mode] => Ok(ResponseLine::Channel {
            channel: canal.to_owned(),
            position: match position {
                ABSENT => None,
                slug => Some(Position::from_slug(slug).map_err(|e| illisible(&e.to_string()))?),
            },
            rpm: absent_ou(rpm).map_err(|r| illisible(&r))?,
            pwm: absent_ou(pwm).map_err(|r| illisible(&r))?,
            mode: mode.to_owned(),
        }),
        ["temp", capteur, millidegres] => Ok(ResponseLine::Temp {
            sensor: capteur.to_owned(),
            millidegrees: millidegres.parse().map_err(|_| {
                illisible("température illisible : attendu des millidegrés entiers")
            })?,
        }),
        ["chan", ..] => Err(illisible("« chan » attend cinq champs")),
        ["temp", ..] => Err(illisible("« temp » attend deux champs")),
        [prefixe, ..] => Err(illisible(&format!("préfixe « {prefixe} » inconnu"))),
        [] => Err(illisible("ligne vide")),
    }
}

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
