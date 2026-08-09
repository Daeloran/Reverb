//! Découverte et écriture sur les périphériques `/dev/hidraw*`.
//!
//! Un rapport de sortie HID est un simple `write()` : aucune bibliothèque C
//! n'est nécessaire. Le noyau interprète le **premier octet comme
//! l'identifiant de rapport** — et sur ces contrôleurs, cet identifiant est
//! l'octet de commande lui-même (`0x2a`, `0x10`, `0x60`). On écrit donc les
//! 64 octets tels quels, sans préfixe (spec §0).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use reverb_proto::{Frame, Model, VENDOR_ID};

/// Un contrôleur NZXT trouvé sur le système.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Controller {
    pub path: PathBuf,
    pub serial: String,
    pub model: Model,
}

/// Ce qu'on retient d'un fichier `uevent` de `hidraw`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uevent {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial: String,
}

/// Extrait l'identification d'un contenu de `/sys/class/hidraw/*/device/uevent`.
///
/// Deux lignes nous intéressent :
///
/// ```text
/// HID_ID=0003:00001E71:00002019
/// HID_UNIQ=1303F00AAAAD9529610494BE
/// ```
///
/// `HID_ID` porte `bus:vendeur:produit` en hexadécimal. `HID_UNIQ` porte le
/// numéro de série — indispensable, car les deux `1e71:2012` sont autrement
/// indiscernables.
pub fn parse_uevent(contents: &str) -> Option<Uevent> {
    let mut ids = None;
    let mut serial = None;

    for ligne in contents.lines() {
        if let Some(valeur) = ligne.strip_prefix("HID_ID=") {
            let mut champs = valeur.split(':');
            let _bus = champs.next()?;
            let vendeur = u16::from_str_radix(champs.next()?.trim(), 16).ok()?;
            let produit = u16::from_str_radix(champs.next()?.trim(), 16).ok()?;
            ids = Some((vendeur, produit));
        } else if let Some(valeur) = ligne.strip_prefix("HID_UNIQ=") {
            let valeur = valeur.trim();
            if !valeur.is_empty() {
                serial = Some(valeur.to_owned());
            }
        }
    }

    let (vendor_id, product_id) = ids?;
    Some(Uevent {
        vendor_id,
        product_id,
        serial: serial?,
    })
}

/// Reconnaît un modèle à partir de son identifiant produit.
pub fn model_from_product_id(product_id: u16) -> Option<Model> {
    [Model::RgbAndFan, Model::Rgb]
        .into_iter()
        .find(|modele| modele.product_id() == product_id)
}

/// Énumère les contrôleurs RGB NZXT présents.
///
/// La résolution se fait par identifiant USB **et numéro de série** : les
/// numéros `hidraw` changent d'un démarrage à l'autre et ne doivent jamais
/// servir de référence.
pub fn discover() -> io::Result<Vec<Controller>> {
    discover_in(Path::new("/sys/class/hidraw"), Path::new("/dev"))
}

/// Variante testable de [`discover`], paramétrée par les racines à parcourir.
pub fn discover_in(sys_class: &Path, dev: &Path) -> io::Result<Vec<Controller>> {
    let mut trouves = Vec::new();

    let entrees = match fs::read_dir(sys_class) {
        Ok(entrees) => entrees,
        Err(erreur) if erreur.kind() == io::ErrorKind::NotFound => return Ok(trouves),
        Err(erreur) => return Err(erreur),
    };

    for entree in entrees {
        let entree = entree?;
        let nom = entree.file_name();
        let uevent = entree.path().join("device/uevent");
        let Ok(contenu) = fs::read_to_string(&uevent) else {
            continue;
        };
        let Some(infos) = parse_uevent(&contenu) else {
            continue;
        };
        if infos.vendor_id != VENDOR_ID {
            continue;
        }
        let Some(model) = model_from_product_id(infos.product_id) else {
            continue;
        };

        trouves.push(Controller {
            path: dev.join(&nom),
            serial: infos.serial,
            model,
        });
    }

    trouves.sort_by(|a, b| a.serial.cmp(&b.serial));
    Ok(trouves)
}

/// Le `/dev/hidraw*` d'un périphérique NZXT donné par son identifiant produit.
///
/// [`discover`] ne rend que les **contrôleurs d'éclairage** — ceux dont
/// [`model_from_product_id`] connaît le modèle. Le Kraken en est un autre
/// usage : son interface HID porte les commandes de l'écran, pas des LED.
///
/// ⚠️ Comme partout, le numéro change au redémarrage : cette recherche se
/// refait à chaque ouverture, elle ne se met jamais en cache dans un fichier.
pub fn find_path(product_id: u16) -> io::Result<PathBuf> {
    find_path_in(
        Path::new("/sys/class/hidraw"),
        Path::new("/dev"),
        product_id,
    )
}

/// Variante testable de [`find_path`], paramétrée par les racines à parcourir.
pub fn find_path_in(sys_class: &Path, dev: &Path, product_id: u16) -> io::Result<PathBuf> {
    for entree in fs::read_dir(sys_class)?.flatten() {
        let uevent = entree.path().join("device/uevent");
        let Ok(contenu) = fs::read_to_string(&uevent) else {
            continue;
        };
        let Some(infos) = parse_uevent(&contenu) else {
            continue;
        };
        if infos.vendor_id == VENDOR_ID && infos.product_id == product_id {
            return Ok(dev.join(entree.file_name()));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("aucun périphérique {VENDOR_ID:04x}:{product_id:04x} branché"),
    ))
}

/// Écrit une trame de 64 octets sur un périphérique.
///
/// ⚠️ **Rouvre le périphérique à chaque appel, et ouvrir coûte 51 ms.** C'est
/// tenable pour la ligne de commande, qui écrit une fois puis rend la main ;
/// c'est rédhibitoire pour une boucle d'animation, où ça plafonne à une image
/// et demie par seconde. Un appelant qui écrit en boucle veut [`Controller::open`].
pub fn write_frame(path: &Path, frame: &Frame) -> io::Result<()> {
    let mut fichier = OpenOptions::new().write(true).open(path)?;
    fichier.write_all(frame)?;
    fichier.flush()
}

/// Un contrôleur dont le descripteur reste ouvert.
///
/// Toute la raison d'être du démon tient dans ce type. Mesuré sur SHYNAEL le
/// 2026-07-31 : ouvrir un `/dev/hidraw*` coûte **51 ms**, y écrire une trame de
/// 64 octets **~1,3 ms**. Le coût est entièrement dans l'ouverture et linéaire
/// en nombre d'ouvertures — repeindre les dix ventilateurs passe de 643 ms à
/// quelques dizaines de millisecondes selon qu'on rouvre ou non.
///
/// ❓ La cause des 51 ms n'est pas établie. L'autosuspend USB est hors de cause
/// (`power/control=on` sur les quatre périphériques). Le chiffre suffit à la
/// décision ; l'explication reste une question ouverte.
pub struct OpenController {
    pub controller: Controller,
    fichier: File,
}

impl Controller {
    /// Ouvre le contrôleur et garde son descripteur.
    ///
    /// # Erreurs
    ///
    /// [`io::ErrorKind::PermissionDenied`] si la règle udev de `packaging/`
    /// n'est pas installée.
    pub fn open(self) -> io::Result<OpenController> {
        let fichier = OpenOptions::new().write(true).open(&self.path)?;
        Ok(OpenController {
            controller: self,
            fichier,
        })
    }
}

impl OpenController {
    /// Écrit une trame sur le descripteur déjà ouvert.
    pub fn write_frame(&mut self, frame: &Frame) -> io::Result<()> {
        self.fichier.write_all(frame)?;
        self.fichier.flush()
    }
}

/// Nombre de trames qu'une question consent à écarter avant d'abandonner.
///
/// Ces contrôleurs **émettent sans qu'on leur demande** — un rapport d'état des
/// ventilateurs `67 02` par seconde et par contrôleur (`SPEC-PROTOCOLE-NZXT`
/// §7.1), des accusés `ff 01` (§7.2), et sur le Kraken un `75 02` mesuré au
/// milieu de l'attente d'un accusé d'image (`SPEC-KRAKEN-LCD` §7). Une réponse à
/// une question n'est donc pas forcément la première trame qui arrive.
///
/// **Huit, et non plus vingt.** Le relevé du 2026-08-09 n'a jamais vu qu'**une
/// seule** trame écartée avant un accusé. Vingt n'était pas une marge, c'était un
/// chiffre : multiplié par [`DELAI_LECTURE`], il donnait un pire cas plus long
/// que le repli du firmware, donc une borne qui ne bornait plus rien d'utile.
///
/// ⚠️ **Cette borne compte des trames, jamais du temps** — voir
/// [`DELAI_LECTURE`], qui est l'autre moitié, et sans laquelle celle-ci ne borne
/// rien du tout.
pub const MAX_LECTURES: usize = 8;

/// Le temps laissé à **une** trame pour arriver.
///
/// # Le défaut que ceci corrige (#83)
///
/// Le commentaire de [`MAX_LECTURES`] promettait « sans jamais bloquer
/// indéfiniment ». C'était faux, et ça a coûté vingt minutes de démon gelé sur
/// SHYNAEL le 2026-08-08 : zéro tic de CPU sur tous les fils, cinq clients sans
/// réponse, `status` sans un octet après quinze secondes. Le descripteur était
/// ouvert en mode bloquant, et un périphérique qui n'émet plus **rien** ne fait
/// pas échouer la lecture — il la fait attendre. Vingt lectures dont la première
/// ne revient jamais, c'est une attente infinie déguisée en boucle bornée.
///
/// # Pourquoi deux secondes, et ce que la première valeur a coûté
///
/// ⚠️ **Une demi-seconde a été essayée, et elle éteignait l'écran.** Elle venait
/// des 18 ms que relève `SPEC-KRAKEN-LCD` §3.2 pour `36 02` → `37 02` — une
/// mesure faite sous Windows, et qui ne décrit pas ce matériel-ci. Relevé sur
/// SHYNAEL le 2026-08-09, en journalisant chaque accusé :
///
/// | accusé | latence mesurée | trames écartées |
/// |---|---|---|
/// | `37 01`, l'annonce | **2 ms**, invariablement | 0 |
/// | `37 02`, la validation | **98 ms**, puis **1,17 s**, puis **1,17 s** | 0, puis 1, puis 1 |
///
/// La validation suit les 1 228 800 octets : le contrôleur digère l'image avant
/// d'accuser, et il y met jusqu'à **soixante-cinq fois** ce que la spec annonce.
/// Le créneau d'attente le plus long relevé — entre `36 02` et la première trame
/// reçue — est de **673 ms**, juste au-dessus des 500 ms fixées. D'où trois faux
/// refus, l'abandon de #70, et la dalle rendue au firmware au bout d'une
/// trentaine de secondes alors que **l'image s'affichait correctement**.
///
/// Deux secondes laissent trois fois le pire créneau relevé.
///
/// ⚠️ **Le délai vaut par lecture, pas pour la question entière.** Le pire cas est
/// `DELAI_LECTURE × MAX_LECTURES`, soit seize secondes, et il reste sous les
/// [`reverb_proto::screen::FIRMWARE_FALLBACK_SECS`] au bout desquelles le
/// firmware reprend la dalle de toute façon. C'est délibéré : un périphérique
/// vivant mais bavard ne doit pas être déclaré mort, parce que trois questions
/// expirées font **rendre la dalle au firmware** (#70) — et c'est exactement ce
/// qui vient d'arriver.
pub const DELAI_LECTURE: Duration = Duration::from_secs(2);

/// Le pire cas d'une question doit rester sous le repli du firmware.
///
/// Vérifié à la **compilation**, et non par un test : ces deux constantes se
/// règlent séparément — l'une sur une latence mesurée, l'autre sur un nombre de
/// trames observé — et rien n'empêcherait leur produit de dériver au fil des
/// relevés. Passé les trente secondes, le firmware a repris la dalle : insister
/// au-delà, c'est attendre l'accusé d'un affichage qui n'existe plus.
const _: () = assert!(
    DELAI_LECTURE.as_millis() as u64 * MAX_LECTURES as u64
        <= reverb_proto::screen::FIRMWARE_FALLBACK_SECS * 1000,
    "DELAI_LECTURE × MAX_LECTURES dépasse le repli du firmware"
);

/// Entre deux tentatives de lecture sur un descripteur non bloquant.
///
/// Deux millisecondes : un acquittement arrive en 3 à 18 ms (spec §3.2), donc le
/// cas courant coûte une poignée de réveils. Le cas muet en coûte deux cent
/// cinquante, chacun un appel système qui rend `EAGAIN` sans rien faire — et il
/// ne se produit que trois fois avant que la vigie de #70 renonce.
const PAS_DE_SCRUTATION: Duration = Duration::from_millis(2);

/// `O_NONBLOCK`, valeur Linux.
///
/// Écrite ici parce que la bibliothèque standard ne l'expose pas et que le projet
/// refuse une dépendance pour une constante (ADR-001). C'est la même approche que
/// les numéros d'`ioctl` de `usbfs.rs`, à ceci près qu'elle ne demande **aucun
/// `unsafe`** : `OpenOptionsExt::custom_flags` est sûr.
const O_NONBLOCK: i32 = 0o4000;

/// Pose une question et attend la réponse dont on connaît l'en-tête.
///
/// Écrit `question`, puis lit jusqu'à trouver une trame commençant par
/// `attendu`, en écartant les rapports spontanés du contrôleur.
///
/// # Erreurs
///
/// [`io::ErrorKind::TimedOut`] si aucune trame n'arrive en [`DELAI_LECTURE`], ou
/// si la réponse attendue ne s'est pas montrée en [`MAX_LECTURES`] trames. Le
/// périphérique est ouvert une seule fois pour les deux sens : le rouvrir entre
/// l'écriture et la lecture ferait perdre les réponses émises entre-temps.
pub fn ask(path: &Path, question: &Frame, attendu: &[u8]) -> io::Result<Frame> {
    // ⚠️ **Non bloquant dès l'ouverture.** C'est le seul point du chemin qui
    // rende le délai possible : la bibliothèque standard n'expose ni `poll` ni
    // `SO_RCVTIMEO` sur un fichier, et un fil dédié par lecture laisserait un fil
    // et un descripteur en fuite à chaque périphérique muet.
    let mut fichier = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(O_NONBLOCK)
        .open(path)?;
    ecrire(&mut fichier, question)?;

    for _ in 0..MAX_LECTURES {
        let mut reponse = [0u8; reverb_proto::FRAME_LEN];
        let lus = lire(&mut fichier, &mut reponse, attendu)?;
        if lus >= attendu.len() && reponse.starts_with(attendu) {
            return Ok(reponse);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "pas de réponse {} après {MAX_LECTURES} trames lues",
            en_hexa(attendu)
        ),
    ))
}

/// Écrit la question, en tolérant qu'un descripteur non bloquant se dérobe.
fn ecrire(fichier: &mut File, question: &Frame) -> io::Result<()> {
    let echeance = Instant::now() + DELAI_LECTURE;
    loop {
        match fichier.write_all(question).and_then(|()| fichier.flush()) {
            Ok(()) => return Ok(()),
            Err(erreur) if patienter(&erreur, echeance) => {}
            Err(erreur) if erreur.kind() == io::ErrorKind::WouldBlock => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("le contrôleur n'a pas pris la question en {DELAI_LECTURE:?}"),
                ));
            }
            Err(erreur) => return Err(erreur),
        }
    }
}

/// Lit **une** trame, ou rend [`io::ErrorKind::TimedOut`] au bout de
/// [`DELAI_LECTURE`].
///
/// ⚠️ **Une lecture qui expire arrête la question.** Réessayer consommerait les
/// vingt tentatives à attendre un périphérique dont on vient d'établir qu'il ne
/// dit plus rien — dix secondes pour apprendre ce qu'on savait à la première
/// demi-seconde.
///
/// `attendu` ne sert qu'au message : sans lui, les trois étapes de la poignée de
/// main d'image (`30 01`, `36 01`, `36 02`) rendraient la même ligne de journal,
/// et on ne saurait pas laquelle a lâché.
fn lire(fichier: &mut File, tampon: &mut [u8], attendu: &[u8]) -> io::Result<usize> {
    let echeance = Instant::now() + DELAI_LECTURE;
    loop {
        match fichier.read(tampon) {
            Ok(lus) => return Ok(lus),
            Err(erreur) if patienter(&erreur, echeance) => {}
            Err(erreur) if erreur.kind() == io::ErrorKind::WouldBlock => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "pas de trame {} en {DELAI_LECTURE:?} — le contrôleur ne répond plus",
                        en_hexa(attendu)
                    ),
                ));
            }
            Err(erreur) => return Err(erreur),
        }
    }
}

/// Faut-il redemander ? Vrai tant que l'échéance tient, pour les deux erreurs qui
/// ne disent rien de l'état du périphérique.
fn patienter(erreur: &io::Error, echeance: Instant) -> bool {
    match erreur.kind() {
        // Un signal reçu pendant l'appel : il n'a pas eu lieu, il se refait.
        io::ErrorKind::Interrupted => true,
        io::ErrorKind::WouldBlock if Instant::now() < echeance => {
            thread::sleep(PAS_DE_SCRUTATION);
            true
        }
        _ => false,
    }
}

fn en_hexa(octets: &[u8]) -> String {
    octets
        .iter()
        .map(|octet| format!("{octet:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const UEVENT_2019: &str = "DRIVER=nzxt-smart2\n\
        HID_ID=0003:00001E71:00002019\n\
        HID_NAME=NZXT, Inc. NZXT RGB & Fan Controller\n\
        HID_PHYS=usb-0000:12:00.0-9.3/input0\n\
        HID_UNIQ=1303F00AAAAD9529610494BE\n\
        MODALIAS=hid:b0003g0001v00001E71p00002019\n";

    #[test]
    fn extrait_identifiants_et_serie() {
        let infos = parse_uevent(UEVENT_2019).expect("uevent reconnu");
        assert_eq!(infos.vendor_id, 0x1E71);
        assert_eq!(infos.product_id, 0x2019);
        assert_eq!(infos.serial, "1303F00AAAAD9529610494BE");
    }

    #[test]
    fn rejette_un_uevent_sans_serie() {
        // Sans HID_UNIQ, les deux 1e71:2012 seraient indiscernables : on préfère
        // ignorer le périphérique plutôt que risquer de colorer le mauvais.
        let sans_serie = "HID_ID=0003:00001E71:00002012\nHID_NAME=NZXT\n";
        assert_eq!(parse_uevent(sans_serie), None);
    }

    #[test]
    fn rejette_un_uevent_sans_identifiants() {
        assert_eq!(parse_uevent("HID_UNIQ=ABC\n"), None);
        assert_eq!(parse_uevent(""), None);
    }

    #[test]
    fn rejette_un_hid_id_malforme() {
        assert_eq!(parse_uevent("HID_ID=0003:ZZZZ\nHID_UNIQ=ABC\n"), None);
        assert_eq!(parse_uevent("HID_ID=0003\nHID_UNIQ=ABC\n"), None);
    }

    #[test]
    fn ignore_une_serie_vide() {
        let vide = "HID_ID=0003:00001E71:00002012\nHID_UNIQ=\n";
        assert_eq!(parse_uevent(vide), None);
    }

    #[test]
    fn reconnait_les_deux_modeles_et_rejette_le_kraken() {
        assert_eq!(model_from_product_id(0x2019), Some(Model::RgbAndFan));
        assert_eq!(model_from_product_id(0x2012), Some(Model::Rgb));
        // Le Kraken Elite n'a aucune LED : il n'est pas un contrôleur RGB.
        assert_eq!(model_from_product_id(0x300C), None);
    }

    #[test]
    fn la_decouverte_sur_une_racine_absente_ne_trouve_rien() {
        let vide = discover_in(Path::new("/inexistant-reverb"), Path::new("/dev")).unwrap();
        assert!(vide.is_empty());
    }
}
