//! Découverte et écriture sur les périphériques `/dev/hidraw*`.
//!
//! Un rapport de sortie HID est un simple `write()` : aucune bibliothèque C
//! n'est nécessaire. Le noyau interprète le **premier octet comme
//! l'identifiant de rapport** — et sur ces contrôleurs, cet identifiant est
//! l'octet de commande lui-même (`0x2a`, `0x10`, `0x60`). On écrit donc les
//! 64 octets tels quels, sans préfixe (spec §0).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

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

/// Nombre de trames lues avant d'abandonner l'attente d'une réponse.
///
/// Le Kraken émet spontanément un rapport d'état `75 02` **chaque seconde**
/// (spec §7.1) : une réponse à une question ne sera donc pas forcément la
/// première trame qui arrive. Vingt lectures laissent passer une poignée de ces
/// bavardages sans jamais bloquer indéfiniment.
const MAX_LECTURES: usize = 20;

/// Pose une question et attend la réponse dont on connaît l'en-tête.
///
/// Écrit `question`, puis lit jusqu'à trouver une trame commençant par
/// `attendu`, en écartant les rapports spontanés du contrôleur.
///
/// # Erreurs
///
/// [`io::ErrorKind::TimedOut`] si la réponse n'arrive pas en [`MAX_LECTURES`]
/// trames. Le périphérique est ouvert une seule fois pour les deux sens : le
/// rouvrir entre l'écriture et la lecture ferait perdre les réponses émises
/// entre-temps.
pub fn ask(path: &Path, question: &Frame, attendu: &[u8]) -> io::Result<Frame> {
    let mut fichier = OpenOptions::new().read(true).write(true).open(path)?;
    fichier.write_all(question)?;
    fichier.flush()?;

    for _ in 0..MAX_LECTURES {
        let mut reponse = [0u8; reverb_proto::FRAME_LEN];
        let lus = fichier.read(&mut reponse)?;
        if lus >= attendu.len() && reponse.starts_with(attendu) {
            return Ok(reponse);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "pas de réponse {} après {MAX_LECTURES} trames lues",
            attendu
                .iter()
                .map(|o| format!("{o:02x}"))
                .collect::<Vec<_>>()
                .join(" ")
        ),
    ))
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
