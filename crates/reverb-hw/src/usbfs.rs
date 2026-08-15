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
//! bibliothèque, et la surface nécessaire ici tient en quatre `ioctl`. Le prix
//! est le seul bloc `unsafe` du dépôt, confiné à ce module.
//!
//! ⚠️ **Le quatrième `ioctl` ne sert pas à l'image** : `USBDEVFS_RESET`
//! réinitialise le port du Kraken quand son firmware cesse de répondre en
//! gardant son lien USB (issue #98). C'est un geste **visible sur la machine** —
//! le périphérique disparaît du bus puis y revient —, et il vit donc derrière une
//! décision bornée, jamais dans une boucle.
//!
//! ⚠️ **Le paquet de longueur nulle est obligatoire pour l'image, et néfaste
//! pour l'en-tête.** `1 228 800 = 2400 × 512`, multiple exact de
//! `wMaxPacketSize` : sans paquet vide, le contrôleur ne sait pas où l'image
//! s'arrête. Les 20 octets d'en-tête, eux, terminent déjà le transfert par un
//! paquet court — y ajouter un paquet vide insère un transfert parasite.
//!
//! Un seul `ioctl` suffit pour les 1,2 Mo : ce noyau ne découpe pas.

// Seule dérogation du dépôt à `unsafe_code`, que le workspace passe en `deny`
// pour la rendre possible. Elle couvre quatre appels à `ioctl`, tous dans ce
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

/// `_IO('U', 20)` — aucune direction, aucun argument : `0x55 << 8 | 20`.
const USBDEVFS_RESET: u64 = 0x0000_5514;

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
    /// La série du périphérique effectivement ouvert, quand il en expose une.
    ///
    /// Gardée pour que le reset de #98 vise **ce** périphérique et pas un autre :
    /// le nœud, lui, change de numéro dès que le bus renumérote.
    serie: Option<String>,
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
        let noeud = resoudre_in(
            Path::new("/sys/bus/usb/devices"),
            Path::new("/dev/bus/usb"),
            None,
        )?;
        let mut screen = Self::open_at(&noeud.chemin)?;
        screen.serie = noeud.serie;
        Ok(screen)
    }

    /// Ouvre un nœud `/dev/bus/usb/...` donné et réclame son interface.
    pub fn open_at(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let screen = Screen {
            file,
            path: path.to_path_buf(),
            // Un nœud ouvert par son chemin n'a été identifié par rien : c'est
            // l'appelant qui a choisi, on ne lui prête pas une série qu'on n'a
            // pas lue.
            serie: None,
        };
        screen.claim()?;
        Ok(screen)
    }

    /// Chemin du nœud ouvert, pour les messages d'erreur.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// La série du périphérique ouvert, si sysfs en donne une.
    ///
    /// C'est elle qu'il faut passer à [`reset`] pour réinitialiser **ce**
    /// périphérique-là : après un reset, le nœud a changé de numéro et le chemin
    /// gardé ici ne désigne plus rien de sûr.
    pub fn serie(&self) -> Option<&str> {
        self.serie.as_deref()
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

/// Un `1e71:300c` branché : son nœud usbfs, et la série qu'il annonce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Noeud {
    /// `/dev/bus/usb/BBB/DDD`. ⚠️ **Il change** — `devnum` est réattribué à
    /// chaque énumération, donc à chaque reset.
    pub chemin: PathBuf,
    /// Ce que `sysfs` lit dans le descripteur du périphérique. `None` si celui-ci
    /// n'expose pas de `serial`.
    pub serie: Option<String>,
}

/// Retrouve le nœud `/dev/bus/usb/BBB/DDD` du Kraken.
///
/// Les deux racines sont des paramètres pour que la fonction se teste contre
/// une fausse arborescence, sans matériel — même approche que `hwmon::discover_in`.
pub fn find_in(sys_bus: &Path, dev_bus: &Path) -> io::Result<PathBuf> {
    Ok(resoudre_in(sys_bus, dev_bus, None)?.chemin)
}

/// Retrouve le Kraken par **VID:PID et, si on en connaît une, par sa série**.
///
/// ⚠️ **Un nœud usbfs n'est pas une identité, c'est une adresse du moment.**
/// `busnum` et `devnum` sont réattribués à chaque énumération : un
/// `USBDEVFS_RESET` fait donc changer le chemin du périphérique qu'il vient de
/// réinitialiser. Garder le chemin d'avant pour réessayer, c'est viser une place
/// vide — ou, pire, celle qu'un autre périphérique occupe désormais. D'où une
/// résolution qui repart de sysfs à chaque fois, et une série qui dit **lequel**
/// on cherche (issue #98). C'est la règle des `hidraw` du CLAUDE.md, appliquée à
/// l'USB : jamais de chemin conservé, jamais de chemin codé en dur.
///
/// ⚠️ **Une série demandée est exigée, pas préférée.** Un `1e71:300c` qui porte
/// une autre série est un autre périphérique, et lui envoyer un reset serait le
/// débrancher sans le vouloir. Mieux vaut un `NotFound` qui nomme la série
/// cherchée.
pub fn resoudre_in(sys_bus: &Path, dev_bus: &Path, serie: Option<&str>) -> io::Result<Noeud> {
    for entree in fs::read_dir(sys_bus)? {
        let device = entree?.path();
        if lire(&device, "idVendor").as_deref() != Some(VENDOR) {
            continue;
        }
        if lire(&device, "idProduct").as_deref() != Some(PRODUCT) {
            continue;
        }
        let trouvee = lire(&device, "serial");
        if let Some(voulue) = serie
            && trouvee.as_deref() != Some(voulue)
        {
            continue;
        }
        let (Some(bus), Some(num)) = (lire(&device, "busnum"), lire(&device, "devnum")) else {
            continue;
        };
        let (Ok(bus), Ok(num)) = (bus.parse::<u32>(), num.parse::<u32>()) else {
            continue;
        };
        return Ok(Noeud {
            chemin: dev_bus.join(format!("{bus:03}")).join(format!("{num:03}")),
            serie: trouvee,
        });
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        match serie {
            Some(voulue) => {
                format!("aucun Kraken {VENDOR}:{PRODUCT} de série « {voulue} » branché")
            }
            None => format!("aucun Kraken {VENDOR}:{PRODUCT} branché"),
        },
    ))
}

/// Réinitialise le port du Kraken — `USBDEVFS_RESET` (issue #98).
///
/// Rend le chemin du nœud qui a reçu le geste, pour le journal : c'est la seule
/// trace qu'un opérateur aura de *ce qui* a été secoué.
///
/// ⚠️ **`Ok` ne veut pas dire « guéri ».** L'`ioctl` réussit dès que le noyau a
/// réinitialisé le port ; il ne dit rien de ce que le firmware fait ensuite, et
/// l'incident de #98 est précisément celui d'un périphérique **énuméré qui ne
/// répond plus**. C'est la source qui répond à nouveau qui prouve la guérison,
/// jamais cette valeur de retour.
///
/// ⚠️ **Toute poignée déjà ouverte sur ce périphérique devient invalide** — celle
/// que [`Screen`] tient comprise. L'appelant doit la lâcher et la rouvrir.
pub fn reset(serie: Option<&str>) -> io::Result<PathBuf> {
    reset_in(
        Path::new("/sys/bus/usb/devices"),
        Path::new("/dev/bus/usb"),
        serie,
    )
}

/// [`reset`], avec ses racines en paramètre.
///
/// ⚠️ **Aucun test automatisé n'appelle cette fonction**, et c'est délibéré : elle
/// ferait disparaître puis réapparaître un périphérique de la machine qui la
/// lance. Ce qui se teste est la **résolution** — juste au-dessus — et la
/// **décision** de réinitialiser, qui vit dans `reverb-daemon::reparation`.
pub fn reset_in(sys_bus: &Path, dev_bus: &Path, serie: Option<&str>) -> io::Result<PathBuf> {
    let noeud = resoudre_in(sys_bus, dev_bus, serie)?;
    // En lecture-écriture : le noyau refuse un reset sur un descripteur ouvert
    // en lecture seule, un reset n'étant pas une consultation.
    let fichier = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&noeud.chemin)?;

    let code = unsafe {
        ioctl(
            fichier.as_raw_fd(),
            USBDEVFS_RESET,
            std::ptr::null_mut::<c_void>(),
        )
    };
    if code < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(noeud.chemin)
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

    // -----------------------------------------------------------------------
    // Résolution par VID:PID **et série** (issue #98)
    // -----------------------------------------------------------------------
    //
    // ⚠️ **Tests de logique, écrits avec le code.** Le critère d'acceptation de
    // #98 — « le périphérique à réinitialiser est résolu par VID:PID + série,
    // jamais par un chemin codé en dur » — n'avait pas de test d'intention :
    // l'issue ne disait pas si `find_in` devait être étendue ou doublée, et un
    // test d'intention ne pouvait pas trancher ce qu'elle ne posait pas. La
    // partie que #98 fige à l'aveugle est la **décision**
    // (`spec_reparation_source.rs`), qui garantit qu'aucun chemin ne traverse la
    // couture. Le **geste**, lui, se vérifie ici.
    //
    // ⚠️ Aucun de ces tests ne réinitialise quoi que ce soit : ils lisent une
    // fausse arborescence sysfs et n'ouvrent aucun nœud.

    /// Pose un `1e71:300c` dans une fausse arborescence sysfs.
    fn poser_kraken(base: &Path, dossier: &str, serie: &str, devnum: u32) {
        let device = base.join(dossier);
        fs::create_dir_all(&device).unwrap();
        fs::write(device.join("idVendor"), "1e71\n").unwrap();
        fs::write(device.join("idProduct"), "300c\n").unwrap();
        fs::write(device.join("busnum"), "1\n").unwrap();
        fs::write(device.join("devnum"), format!("{devnum}\n")).unwrap();
        fs::write(device.join("serial"), format!("{serie}\n")).unwrap();
    }

    #[test]
    fn la_serie_designe_le_kraken_quand_deux_repondent_aux_memes_identifiants() {
        // Deux `1e71:300c` sur la même machine ne sont pas un cas d'école : le
        // dépôt en compte déjà deux pour `1e71:2012`, physiquement identiques et
        // que **seule** leur série distingue (CLAUDE.md). Sans elle, un reset
        // part sur le premier que `read_dir` rend — c'est-à-dire au hasard, et
        // il débranche un périphérique qui allait bien.
        let base = std::env::temp_dir().join("reverb-usbfs-serie");
        let _ = fs::remove_dir_all(&base);
        poser_kraken(&base, "1-9.1", "BB8C90820E900630", 7);
        poser_kraken(&base, "1-4.2", "AA0000000000AAAA", 12);

        let vise = resoudre_in(&base, Path::new("/dev/bus/usb"), Some("AA0000000000AAAA")).unwrap();
        assert_eq!(
            vise.chemin,
            Path::new("/dev/bus/usb/001/012"),
            "la série demandée doit désigner son propre nœud, pas le premier 300c venu"
        );
        assert_eq!(vise.serie.as_deref(), Some("AA0000000000AAAA"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn une_serie_absente_du_bus_est_refusee_plutot_que_remplacee() {
        // La faute symétrique, et la plus dangereuse : se rabattre sur « l'autre
        // 300c, faute de mieux ». Un `USBDEVFS_RESET` fait disparaître le
        // périphérique du bus ; le poser sur celui qu'on n'a pas demandé, c'est
        // casser ce qui marchait pour réparer ce qui n'est plus là.
        let base = std::env::temp_dir().join("reverb-usbfs-serie-absente");
        let _ = fs::remove_dir_all(&base);
        poser_kraken(&base, "1-9.1", "BB8C90820E900630", 7);

        let erreur = resoudre_in(
            &base,
            Path::new("/dev/bus/usb"),
            Some("SERIE-QUI-N-EXISTE-PAS"),
        )
        .unwrap_err();
        assert_eq!(erreur.kind(), io::ErrorKind::NotFound);
        assert!(
            erreur.to_string().contains("SERIE-QUI-N-EXISTE-PAS"),
            "le refus doit nommer la série cherchée — c'est le seul diagnostic \
             que l'opérateur reçoit ; trouvé « {erreur} »"
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn apres_un_reset_le_numero_du_noeud_change_mais_la_serie_non() {
        // ⚠️ **Le corollaire USB du test n° 12 de `spec_reparation_source.rs`.**
        // Là-bas, ce sont les répertoires `hwmonN` qui échangent leurs numéros ;
        // ici, c'est `devnum` que le noyau réattribue en réénumérant. Le même
        // danger dans les deux cas : garder l'adresse d'avant fait viser un
        // **autre** périphérique, et rien ne le signale.
        //
        // Ce test est la raison pour laquelle `reset_in` re-résout depuis sysfs
        // au lieu de réutiliser le chemin que `Screen` avait gardé.
        let base = std::env::temp_dir().join("reverb-usbfs-renumerote");
        let _ = fs::remove_dir_all(&base);
        const SERIE: &str = "BB8C90820E900630";
        poser_kraken(&base, "1-9.1", SERIE, 7);

        let avant = resoudre_in(&base, Path::new("/dev/bus/usb"), Some(SERIE)).unwrap();
        assert_eq!(avant.chemin, Path::new("/dev/bus/usb/001/007"));

        // Le reset a eu lieu : le périphérique a quitté le bus puis y est revenu,
        // et le noyau lui a donné le numéro libre suivant. Sa série, elle, est
        // dans son descripteur et ne bouge pas.
        fs::remove_dir_all(base.join("1-9.1")).unwrap();
        poser_kraken(&base, "1-9.1", SERIE, 8);

        let apres = resoudre_in(&base, Path::new("/dev/bus/usb"), Some(SERIE)).unwrap();
        assert_eq!(
            apres.chemin,
            Path::new("/dev/bus/usb/001/008"),
            "le nœud a changé de numéro : le retrouver par sa série est le seul \
             moyen de viser encore le bon périphérique"
        );
        assert_ne!(
            apres.chemin, avant.chemin,
            "sans changement de numéro, ce test ne dit rien du danger qu'il décrit"
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn un_kraken_sans_serie_reste_trouvable_sans_serie_demandee() {
        // Tous les périphériques n'exposent pas de `serial`. Celui de SHYNAEL le
        // fait — relevé le 2026-08-15, `BB8C90820E900630` — mais exiger une série
        // qu'un contrôleur n'annonce pas rendrait la dalle introuvable là où elle
        // l'était hier. Sans série demandée, la recherche reste celle d'avant.
        let base = std::env::temp_dir().join("reverb-usbfs-sans-serie");
        let _ = fs::remove_dir_all(&base);
        let device = base.join("1-9.1");
        fs::create_dir_all(&device).unwrap();
        fs::write(device.join("idVendor"), "1e71\n").unwrap();
        fs::write(device.join("idProduct"), "300c\n").unwrap();
        fs::write(device.join("busnum"), "1\n").unwrap();
        fs::write(device.join("devnum"), "7\n").unwrap();

        let noeud = resoudre_in(&base, Path::new("/dev/bus/usb"), None).unwrap();
        assert_eq!(noeud.chemin, Path::new("/dev/bus/usb/001/007"));
        assert_eq!(
            noeud.serie, None,
            "une série absente se dit absente, elle ne s'invente pas"
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
