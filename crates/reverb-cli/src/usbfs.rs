//! Transferts bulk vers l'écran du Kraken, par usbfs.
//!
//! L'image ne passe pas par hidraw. Le Kraken expose deux interfaces
//! (spec §1) : l'interface HID porte les commandes de contrôle, et une
//! interface de classe `0xff` porte un unique endpoint bulk `0x02` par lequel
//! transitent les 1 228 800 octets d'une image.
//!
//! Sous Linux, atteindre un endpoint bulk passe par `/dev/bus/usb/BBB/DDD` et
//! quelques `ioctl`. **Aucun pilote noyau n'est lié à cette interface** —
//! vérifié sur SHYNAEL : `USBDEVFS_CLAIMINTERFACE` réussit sans rien détacher.
//!
//! Pourquoi pas `libusb` : le projet écrit déjà ses trames hidraw sans
//! bibliothèque, et la surface nécessaire ici tient en trois `ioctl`. Le prix
//! est le seul bloc `unsafe` du dépôt, confiné à ce module.
//!
//! ⚠️ **Le paquet de longueur nulle est obligatoire pour l'image, et néfaste
//! pour l'en-tête.** `1 228 800 = 2400 × 512`, multiple exact de
//! `wMaxPacketSize` : sans paquet vide, le contrôleur ne sait pas où l'image
//! s'arrête. Les 20 octets d'en-tête, eux, terminent déjà le transfert par un
//! paquet court — y ajouter un paquet vide insère un transfert parasite.
//!
//! Un seul `ioctl` suffit pour les 1,2 Mo : ce noyau ne découpe pas.

// Seule dérogation du dépôt à `unsafe_code`, que le workspace passe en `deny`
// pour la rendre possible. Elle couvre trois appels à `ioctl`, tous dans ce
// fichier, tous sur un descripteur ouvert par la bibliothèque standard, avec des
// structures dont la disposition est celle de `linux/usbdevice_fs.h`.
//
// Ce que ces appels supposent, et qui n'est pas vérifiable par le compilateur :
//   - la disposition de `BulkTransfer` correspond à `struct usbdevfs_bulktransfer` ;
//   - les numéros d'`ioctl` correspondent à ceux que le noyau attend ;
//   - le tampon pointé par `data` vit au moins jusqu'au retour de l'appel.
//
// Les deux premiers points sont vérifiés par les tests de `screen.rs` côté
// valeurs et par le calcul d'encodage rappelé sur chaque constante. Le
// troisième tient par construction : le tampon est une variable locale qui
// survit à l'appel.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

/// Identifiant constructeur NZXT.
const VENDOR: &str = "1e71";

/// Identifiant produit du Kraken Elite 2023.
const PRODUCT: &str = "300c";

/// Interface de classe `0xff` qui porte l'endpoint bulk (spec §1).
const INTERFACE: u32 = 0;

/// Endpoint bulk sortant (spec §1).
const ENDPOINT: u32 = 0x02;

/// `wMaxPacketSize` de l'endpoint bulk (spec §1). Détermine à lui seul quand un
/// paquet de longueur nulle est nécessaire — voir [`Screen::write_bulk`].
const MAX_PACKET_SIZE: usize = 512;

/// Délai d'un transfert, en millisecondes. Une image met environ 470 ms
/// (spec §2.2.1) ; cinq secondes laissent de la marge sans figer la commande.
const TIMEOUT_MS: u32 = 5_000;

/// `_IOR('U', 15, unsigned int)`
const USBDEVFS_CLAIMINTERFACE: u64 = 0x8004_550f;

/// `_IOR('U', 16, unsigned int)`
const USBDEVFS_RELEASEINTERFACE: u64 = 0x8004_5510;

/// `_IOWR('U', 2, struct usbdevfs_bulktransfer)`
const USBDEVFS_BULK: u64 = 0xc018_5502;

/// `struct usbdevfs_bulktransfer` de `linux/usbdevice_fs.h`.
#[repr(C)]
struct BulkTransfer {
    ep: u32,
    len: u32,
    timeout: u32,
    data: *mut c_void,
}

unsafe extern "C" {
    fn ioctl(fd: i32, request: u64, arg: *mut c_void) -> i32;
}

/// L'écran, ouvert et son interface réclamée.
///
/// L'interface est rendue à la fermeture : voir l'implémentation de [`Drop`].
pub struct Screen {
    file: File,
    path: PathBuf,
}

impl Screen {
    /// Ouvre l'écran du Kraken et réclame son interface bulk.
    ///
    /// # Erreurs
    ///
    /// [`io::ErrorKind::NotFound`] si aucun `1e71:300c` n'est branché,
    /// [`io::ErrorKind::PermissionDenied`] si la règle udev de `packaging/`
    /// n'est pas installée.
    pub fn open() -> io::Result<Self> {
        let path = find_in(Path::new("/sys/bus/usb/devices"), Path::new("/dev/bus/usb"))?;
        Self::open_at(&path)
    }

    /// Ouvre un nœud `/dev/bus/usb/...` donné et réclame son interface.
    pub fn open_at(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let screen = Screen {
            file,
            path: path.to_path_buf(),
        };
        screen.claim()?;
        Ok(screen)
    }

    /// Chemin du nœud ouvert, pour les messages d'erreur.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn claim(&self) -> io::Result<()> {
        let mut interface = INTERFACE;
        let code = unsafe {
            ioctl(
                self.file.as_raw_fd(),
                USBDEVFS_CLAIMINTERFACE,
                (&raw mut interface).cast::<c_void>(),
            )
        };
        if code < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Écrit une charge utile sur l'endpoint bulk, en terminant le transfert
    /// comme la spécification USB l'exige.
    ///
    /// Un transfert bulk se termine sur un paquet **plus court** que
    /// `wMaxPacketSize`. Quand la charge utile est un multiple exact de cette
    /// taille, aucun paquet court n'arrive jamais et il faut en émettre un vide
    /// — le *zero-length packet*. Sinon le contrôleur ne sait pas où le
    /// transfert s'arrête et concatène le suivant : c'est la dérive du §2.2.1.
    ///
    /// ⚠️ **La règle est conditionnelle, et l'oublier coûte cher.** Une première
    /// version émettait le paquet vide après *chaque* transfert, donc aussi
    /// après l'en-tête de 20 octets qui se termine déjà tout seul. Le contrôleur
    /// recevait un transfert vide entre l'en-tête et l'image : aucune image ne
    /// s'affichait, et l'affichage firmware lui-même se dégradait.
    pub fn write_bulk(&self, data: &[u8]) -> io::Result<()> {
        self.transfer(data)?;
        if !data.is_empty() && data.len().is_multiple_of(MAX_PACKET_SIZE) {
            self.transfer(&[])?;
        }
        Ok(())
    }

    fn transfer(&self, data: &[u8]) -> io::Result<()> {
        let mut copie = data.to_vec();
        let mut requete = BulkTransfer {
            ep: ENDPOINT,
            len: u32::try_from(copie.len()).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "transfert trop volumineux pour usbfs",
                )
            })?,
            timeout: TIMEOUT_MS,
            data: copie.as_mut_ptr().cast::<c_void>(),
        };

        let ecrits = unsafe {
            ioctl(
                self.file.as_raw_fd(),
                USBDEVFS_BULK,
                (&raw mut requete).cast::<c_void>(),
            )
        };
        if ecrits < 0 {
            return Err(io::Error::last_os_error());
        }
        // Un transfert partiel n'a pas de sens ici : le contrôleur attend une
        // image entière. Le signaler plutôt que de laisser croire au succès.
        if ecrits as usize != copie.len() {
            return Err(io::Error::other(format!(
                "transfert partiel : {ecrits} octets écrits sur {}",
                copie.len()
            )));
        }
        Ok(())
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let mut interface = INTERFACE;
        // Rendre l'interface est un nettoyage : si ça échoue, la fermeture du
        // descripteur s'en charge de toute façon. Rien à signaler dans un Drop.
        unsafe {
            ioctl(
                self.file.as_raw_fd(),
                USBDEVFS_RELEASEINTERFACE,
                (&raw mut interface).cast::<c_void>(),
            );
        }
    }
}

/// Retrouve le nœud `/dev/bus/usb/BBB/DDD` du Kraken.
///
/// Les deux racines sont des paramètres pour que la fonction se teste contre
/// une fausse arborescence, sans matériel — même approche que `hwmon::discover_in`.
pub fn find_in(sys_bus: &Path, dev_bus: &Path) -> io::Result<PathBuf> {
    for entree in fs::read_dir(sys_bus)? {
        let device = entree?.path();
        if lire(&device, "idVendor").as_deref() != Some(VENDOR) {
            continue;
        }
        if lire(&device, "idProduct").as_deref() != Some(PRODUCT) {
            continue;
        }
        let (Some(bus), Some(num)) = (lire(&device, "busnum"), lire(&device, "devnum")) else {
            continue;
        };
        let (Ok(bus), Ok(num)) = (bus.parse::<u32>(), num.parse::<u32>()) else {
            continue;
        };
        return Ok(dev_bus.join(format!("{bus:03}")).join(format!("{num:03}")));
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("aucun Kraken {VENDOR}:{PRODUCT} branché"),
    ))
}

fn lire(device: &Path, attribut: &str) -> Option<String> {
    fs::read_to_string(device.join(attribut))
        .ok()
        .map(|valeur| valeur.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_kraken_est_retrouve_par_ses_identifiants() {
        let base = std::env::temp_dir().join("reverb-usbfs-trouve");
        let _ = fs::remove_dir_all(&base);
        let device = base.join("1-9.1");
        fs::create_dir_all(&device).unwrap();
        fs::write(device.join("idVendor"), "1e71\n").unwrap();
        fs::write(device.join("idProduct"), "300c\n").unwrap();
        fs::write(device.join("busnum"), "1\n").unwrap();
        fs::write(device.join("devnum"), "7\n").unwrap();

        let trouve = find_in(&base, Path::new("/dev/bus/usb")).unwrap();
        assert_eq!(
            trouve,
            Path::new("/dev/bus/usb/001/007"),
            "les numéros de bus et de périphérique sont complétés à trois chiffres"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn un_peripherique_nzxt_qui_n_est_pas_l_ecran_est_ignore() {
        let base = std::env::temp_dir().join("reverb-usbfs-ignore");
        let _ = fs::remove_dir_all(&base);
        // Le contrôleur RGB porte le même identifiant constructeur.
        let device = base.join("1-9.3");
        fs::create_dir_all(&device).unwrap();
        fs::write(device.join("idVendor"), "1e71\n").unwrap();
        fs::write(device.join("idProduct"), "2019\n").unwrap();
        fs::write(device.join("busnum"), "1\n").unwrap();
        fs::write(device.join("devnum"), "9\n").unwrap();

        let erreur = find_in(&base, Path::new("/dev/bus/usb")).unwrap_err();
        assert_eq!(
            erreur.kind(),
            io::ErrorKind::NotFound,
            "un 1e71 qui n'est pas un 300c ne doit pas être pris pour l'écran"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn un_peripherique_sans_numero_de_bus_ne_fait_pas_echouer_la_recherche() {
        let base = std::env::temp_dir().join("reverb-usbfs-partiel");
        let _ = fs::remove_dir_all(&base);
        // « usb1 » et consorts n'ont pas tous les attributs : les ignorer sans
        // interrompre le parcours.
        let boiteux = base.join("usb1");
        fs::create_dir_all(&boiteux).unwrap();
        fs::write(boiteux.join("idVendor"), "1e71\n").unwrap();
        fs::write(boiteux.join("idProduct"), "300c\n").unwrap();

        let erreur = find_in(&base, Path::new("/dev/bus/usb")).unwrap_err();
        assert_eq!(erreur.kind(), io::ErrorKind::NotFound);
        let _ = fs::remove_dir_all(&base);
    }
}
