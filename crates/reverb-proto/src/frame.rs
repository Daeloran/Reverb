//! Construction des trames HID.
//!
//! ⚠️ Le premier octet de la trame **est** l'identifiant de rapport HID.
//! `OutputReportByteLength` vaut 64 et non 65 : il ne faut donc **pas**
//! préfixer un octet `0x00` comme le veut la convention habituelle (spec §0).

use crate::{Brightness, Model, Rgb};

/// Longueur d'une trame HID, complétée par des zéros (spec §1).
pub const FRAME_LEN: usize = 64;

/// Une trame prête à écrire sur `/dev/hidraw*`.
pub type Frame = [u8; FRAME_LEN];

/// Construit la trame `0x2a 0x04` en mode couleur fixe (spec §4).
///
/// ```text
/// offset  0    1     2      3      4     5       6    7..     56
///        0x2a 0x04 masque masque  mode vitesse  0x00 couleurs  nb couleurs
/// ```
///
/// Le mode fixe est `0x00` et n'attend qu'une seule couleur. L'octet 5 porte la
/// vitesse d'animation, sans effet ici. Les octets 58 et 59 annoncent le nombre
/// de LED de l'accessoire et son type.
pub fn fixed_color(mask: u8, color: Rgb, brightness: Brightness, leds: u8) -> Frame {
    let _ = (mask, color, brightness, leds);
    todo!()
}

/// Séquence d'initialisation, à rejouer à chaque démarrage (spec §8).
///
/// Le `2019` et les `2012` ne reçoivent pas la même : l'argument de `0x10`
/// diffère, et seul le `2019` reçoit les commandes `0x60`.
///
/// ⚠️ Rien ne survit à une coupure d'alimentation, mais l'état **persiste** en
/// mémoire volatile à travers un redémarrage à chaud. La nécessité réelle de
/// cette séquence sur une machine froide reste à confirmer (spec §0).
pub fn init_sequence(model: Model) -> Vec<Frame> {
    let _ = model;
    todo!()
}
