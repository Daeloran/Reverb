//! Accès SMBus aux contrôleurs RGB des barrettes, par `i2c-dev`.
//!
//! Corsair n'emploie aucun protocole propriétaire (spec §1) : les barrettes se
//! joignent par des transferts SMBus par bloc standard, sur le contrôleur AMD
//! FCH que `i2c-piix4` gère déjà. Un `write()` sur `/dev/i2c-N` de
//! `[registre][compte][données]` **est** une écriture par bloc — le noyau
//! reprend ces deux premiers octets pour `SMBHSTCMD` et `SMBHSTDAT0`.
//!
//! ⚠️ **Ce bus porte aussi les hubs SPD des barrettes, en `0x50`–`0x53`**
//! (spec §3). Une écriture au mauvais endroit corrompt le SPD et rend un DIMM
//! non démarrable. Deux garde-fous, aux deux bouts :
//!
//! - [`Bus::target`] ne prend pas un `u8` mais un `SlotAddress`, qui ne se
//!   construit que depuis un index d'emplacement. Viser une adresse arbitraire
//!   n'est pas refusé à l'exécution : c'est irreprésentable ;
//! - l'`ioctl` employé est `I2C_SLAVE` et **non** `I2C_SLAVE_FORCE`. Le
//!   premier échoue si un pilote noyau détient déjà l'adresse, ce qui fait de
//!   `spd5118` une protection au lieu d'un risque. Le second existe
//!   précisément pour passer outre — il n'a rien à faire ici.
//!
//! ⚠️ **Le bus n'est jamais sondé.** Le §6 de la spec suggère d'essayer
//! `0x18`–`0x1b` sur chaque adaptateur pour trouver le bon ; le garde-fou du
//! projet l'interdit, un scan en lecture seule ayant déjà altéré l'éclairage
//! par défaut de cette RAM. L'adaptateur est reconnu à son **nom**, lu dans
//! sysfs, sans qu'un octet parte sur le fil.

// Seconde dérogation du dépôt à `unsafe_code`, que le workspace passe en `deny`
// pour les rendre possibles (ADR-004). Elle couvre un unique appel à `ioctl`,
// dans ce fichier, sur un descripteur ouvert par la bibliothèque standard.
//
// Ce que cet appel suppose, et qui n'est pas vérifiable par le compilateur :
//   - `I2C_SLAVE` vaut bien `0x0703`, comme `linux/i2c-dev.h` le déclare ;
//   - cet `ioctl` prend son argument **par valeur** et non par pointeur, ce qui
//     est la seule différence de forme avec ceux de `usbfs.rs`.
//
// Rien n'est emprunté ni déréférencé : il n'y a pas de durée de vie à garantir.
#![allow(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

use reverb_proto::ram::SlotAddress;

/// Nom exact de l'adaptateur qui porte les barrettes, tel que `i2c-piix4` le
/// publie dans `/sys/class/i2c-dev/*/name`.
///
/// Sur SHYNAEL, `i2c-piix4` en enregistre **trois** : « port 0 at 0b00 »,
/// « port 2 at 0b00 » et « port 1 at 0b20 ». La capture iCUE ne donne que la
/// base d'E/S `0x0B00`, qui n'en distingue que le troisième.
///
/// C'est le noyau qui tranche, sans qu'on touche au bus : les quatre hubs SPD
/// des barrettes sont liés à `spd5118` en `8-0050`…`8-0053`, donc sur `i2c-8`,
/// donc sur « port 0 at 0b00 ». Un contrôleur RGB partage les broches du hub
/// SPD de sa propre barrette.
///
/// **À réviser si** ce nom se révèle fragile — autre carte, autre BIOS,
/// renumérotation par `i2c-piix4`. Le critère portable serait alors
/// « l'adaptateur qui porte des `spd5118` en `0x50`–`0x53` », qui se lit dans
/// sysfs sans plus de trafic, mais suppose ce pilote chargé.
pub const ADAPTER_NAME: &str = "SMBus PIIX4 adapter port 0 at 0b00";

/// `I2C_SLAVE` de `linux/i2c-dev.h` — fixe l'adresse esclave du descripteur.
///
/// Échoue avec `EBUSY` si un pilote noyau détient déjà l'adresse. C'est
/// recherché : voir l'en-tête du module.
const I2C_SLAVE: u64 = 0x0703;

// Même signature que la déclaration de `usbfs.rs` — deux `extern` du même
// symbole qui divergeraient déclencheraient `clashing_extern_declarations`.
//
// La forme est trompeuse : `I2C_SLAVE` prend son argument **par valeur** et non
// par pointeur, contrairement aux `ioctl` d'usbfs. D'où le
// `without_provenance_mut` de [`Bus::target`], qui place un entier dans
// l'emplacement du pointeur sans prétendre qu'il en est un. Sur l'ABI x86-64,
// les deux occupent le même registre ; le noyau lit la valeur, pas ce qu'elle
// désignerait.
unsafe extern "C" {
    fn ioctl(fd: i32, request: u64, arg: *mut std::ffi::c_void) -> i32;
}

/// Un adaptateur SMBus ouvert.
pub struct Bus {
    file: File,
    path: PathBuf,
}

impl Bus {
    /// Ouvre l'adaptateur. N'écrit rien et ne sonde rien.
    ///
    /// # Erreurs
    ///
    /// [`io::ErrorKind::PermissionDenied`] si la règle udev de `packaging/`
    /// n'est pas installée.
    pub fn open(chemin: &Path) -> io::Result<Bus> {
        let file = OpenOptions::new().read(true).write(true).open(chemin)?;
        Ok(Bus {
            file,
            path: chemin.to_path_buf(),
        })
    }

    /// Chemin du nœud ouvert, pour les messages d'erreur.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Fixe l'adresse cible — un `ioctl`, une fois par barrette.
    ///
    /// Prend un [`SlotAddress`] et non un `u8` : l'adresse d'un hub SPD reste
    /// irreprésentable jusqu'à la frontière d'entrée/sortie, et pas seulement
    /// dans `reverb-proto`.
    ///
    /// # Erreurs
    ///
    /// `EBUSY` si un pilote noyau détient l'adresse.
    pub fn target(&self, slot: SlotAddress) -> io::Result<()> {
        let adresse = std::ptr::without_provenance_mut(usize::from(slot.address()));
        let code = unsafe { ioctl(self.file.as_raw_fd(), I2C_SLAVE, adresse) };
        if code < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Écrit un transfert, tel que `ram::transfers` l'a produit.
    ///
    /// Volontairement pas de `write_all` : une écriture partielle sur ce bus ne
    /// se rattrape pas en poussant le reste, ça émettrait un second bloc
    /// tronqué. Mieux vaut le signaler.
    pub fn write(&mut self, transfert: &[u8]) -> io::Result<()> {
        let ecrits = self.file.write(transfert)?;
        if ecrits != transfert.len() {
            return Err(io::Error::other(format!(
                "transfert partiel sur {} : {ecrits} octets écrits sur {}",
                self.path.display(),
                transfert.len()
            )));
        }
        Ok(())
    }
}

/// Retrouve `/dev/i2c-N` pour l'adaptateur nommé [`ADAPTER_NAME`].
///
/// Les deux racines sont des paramètres pour que la fonction se teste contre
/// une fausse arborescence, sans matériel — même approche que
/// [`crate::usbfs::find_in`].
///
/// # Erreurs
///
/// Refuse plutôt que de deviner si aucun ou plusieurs adaptateurs
/// correspondent, en listant dans les deux cas ce qui a été trouvé.
pub fn find_adapter_in(sys_class: &Path, dev: &Path) -> io::Result<PathBuf> {
    let mut correspondants = Vec::new();
    let mut tous = Vec::new();

    for entree in fs::read_dir(sys_class)? {
        let adaptateur = entree?.path();
        let Some(noeud) = adaptateur
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        let Ok(nom) = fs::read_to_string(adaptateur.join("name")) else {
            continue;
        };
        let nom = nom.trim().to_owned();

        tous.push(format!("  {noeud}  {nom}"));
        if nom == ADAPTER_NAME {
            correspondants.push(dev.join(&noeud));
        }
    }

    // `read_dir` ne garantit aucun ordre : trier rend le choix et les messages
    // reproductibles d'une exécution à l'autre.
    correspondants.sort();
    tous.sort();
    let inventaire = tous.join("\n");

    match correspondants.len() {
        1 => Ok(correspondants.remove(0)),
        0 => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "aucun adaptateur nommé « {ADAPTER_NAME} ».\nAdaptateurs trouvés :\n{inventaire}"
            ),
        )),
        n => Err(io::Error::other(format!(
            "{n} adaptateurs nommés « {ADAPTER_NAME} » — refus de choisir.\n\
             Adaptateurs trouvés :\n{inventaire}"
        ))),
    }
}

/// Retrouve l'adaptateur sur la machine.
pub fn find_adapter() -> io::Result<PathBuf> {
    find_adapter_in(Path::new("/sys/class/i2c-dev"), Path::new("/dev"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit une fausse `/sys/class/i2c-dev`, un dossier par adaptateur.
    fn arborescence(nom_du_test: &str, adaptateurs: &[(&str, &str)]) -> PathBuf {
        let base = std::env::temp_dir().join(format!("reverb-i2c-{nom_du_test}"));
        let _ = fs::remove_dir_all(&base);
        for (noeud, nom) in adaptateurs {
            let dossier = base.join(noeud);
            fs::create_dir_all(&dossier).unwrap();
            fs::write(dossier.join("name"), format!("{nom}\n")).unwrap();
        }
        base
    }

    /// Le nom de l'adaptateur est écrit à deux endroits — ici et dans la règle
    /// udev — et rien dans le langage ne les relie.
    ///
    /// Le mode de défaillance est silencieux et différé : changer
    /// [`ADAPTER_NAME`] sans toucher `packaging/60-reverb.rules` donnerait un
    /// Reverb qui trouve son adaptateur et se voit refuser l'ouverture, sur la
    /// machine de quelqu'un d'autre. Même raisonnement que `Model::ALL` face à
    /// la même règle, côté NZXT.
    #[test]
    fn la_regle_udev_ouvre_bien_l_adaptateur_que_ce_module_cherche() {
        let regles = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packaging/60-reverb.rules");
        let contenu = fs::read_to_string(&regles)
            .unwrap_or_else(|e| panic!("{} illisible : {e}", regles.display()));

        let attendu = format!("ATTR{{name}}==\"{ADAPTER_NAME}\"");
        assert!(
            contenu
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .any(|l| l.contains(&attendu)),
            "aucune règle active ne vise « {ADAPTER_NAME} » — attendu une ligne portant {attendu}"
        );
    }

    #[test]
    fn l_adaptateur_est_retrouve_par_son_nom() {
        let base = arborescence(
            "trouve",
            &[
                ("i2c-4", "NVIDIA i2c adapter 3 at 1:00.0"),
                ("i2c-8", ADAPTER_NAME),
                ("i2c-9", "SMBus PIIX4 adapter port 2 at 0b00"),
                ("i2c-10", "SMBus PIIX4 adapter port 1 at 0b20"),
            ],
        );

        let trouve = find_adapter_in(&base, Path::new("/dev")).unwrap();
        assert_eq!(trouve, Path::new("/dev/i2c-8"));
    }

    #[test]
    fn aucun_adaptateur_correspondant_est_un_refus_qui_liste_ce_qu_il_a_vu() {
        // Le piège que le nom évite : « port 2 at 0b00 » a la même base d'E/S
        // que celui qu'on cherche, et la capture iCUE ne les distingue pas.
        let base = arborescence(
            "aucun",
            &[
                ("i2c-9", "SMBus PIIX4 adapter port 2 at 0b00"),
                ("i2c-10", "SMBus PIIX4 adapter port 1 at 0b20"),
            ],
        );

        let erreur = find_adapter_in(&base, Path::new("/dev")).unwrap_err();
        assert_eq!(erreur.kind(), io::ErrorKind::NotFound);
        let message = erreur.to_string();
        assert!(message.contains("port 2 at 0b00"), "{message}");
        assert!(message.contains("port 1 at 0b20"), "{message}");
    }

    #[test]
    fn plusieurs_adaptateurs_correspondants_est_un_refus_de_choisir() {
        let base = arborescence(
            "plusieurs",
            &[("i2c-8", ADAPTER_NAME), ("i2c-12", ADAPTER_NAME)],
        );

        let erreur = find_adapter_in(&base, Path::new("/dev")).unwrap_err();
        let message = erreur.to_string();
        assert!(message.contains("refus de choisir"), "{message}");
        assert!(
            message.contains("i2c-8") && message.contains("i2c-12"),
            "{message}"
        );
    }
}
