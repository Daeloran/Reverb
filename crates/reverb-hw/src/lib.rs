//! Les quatre chemins d'entrée/sortie vers le matériel.
//!
//! Extrait de `reverb-cli` : le démon en a besoin à l'identique, et le faire
//! dépendre d'un crate nommé « cli » aurait été un nom qui ment.
//!
//! | Module | Chemin | Ce qui y passe |
//! |---|---|---|
//! | [`hidraw`] | `write()` sur `/dev/hidraw*` | les trames d'éclairage des contrôleurs NZXT |
//! | [`usbfs`] | trois `ioctl` | l'image de l'écran du Kraken |
//! | [`i2c`] | deux `ioctl` | les couleurs de la RAM Corsair, par SMBus |
//! | [`hwmon`] | sysfs | vitesses, températures et consignes des ventilateurs |
//!
//! ⚠️ **Ouvrir coûte cher, écrire ne coûte rien.** Ouvrir un `/dev/hidraw*`
//! prend 51 ms sur SHYNAEL ; y écrire une trame de 64 octets en prend 1,3.
//! Un appelant qui rouvre à chaque trame plafonne à une image et demie par
//! seconde — c'est ce qui a motivé le démon (issue #17). Les fonctions de ce
//! crate qui prennent un chemin plutôt qu'un descripteur ouvert sont là pour
//! la ligne de commande, qui écrit une fois puis rend la main.

pub mod hidraw;
pub mod hwmon;
pub mod i2c;
pub mod usbfs;
