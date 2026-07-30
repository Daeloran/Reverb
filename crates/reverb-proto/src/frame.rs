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

/// Mode « couleur fixe » (spec §4.1).
const MODE_FIXED: u8 = 0x00;

/// Vitesse d'animation par défaut, sans effet en mode fixe (spec §4.2, §11).
const DEFAULT_SPEED: u8 = 0x32;

/// Type d'accessoire annoncé à l'offset 59 (spec §4).
const ACCESSORY_TYPE: u8 = 0x03;

/// Construit un paquet de 64 octets à partir de ses octets significatifs.
///
/// Spec §1 — « Les paquets sont toujours complétés à 64 octets par des zéros. »
fn packet(head: &[u8]) -> Frame {
    let mut frame = [0u8; FRAME_LEN];
    frame[..head.len()].copy_from_slice(head);
    frame
}

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
///
/// La luminosité est appliquée **ici**, aux composantes, car le protocole n'a
/// aucun octet dédié (spec §4.3).
pub fn fixed_color(mask: u8, color: Rgb, brightness: Brightness, leds: u8) -> Frame {
    let [g, r, b] = color.with_brightness(brightness).to_grb();

    let mut frame = [0u8; FRAME_LEN];
    frame[0] = 0x2a;
    frame[1] = 0x04;
    frame[2] = mask;
    // L'offset 3 est resté égal à l'offset 2 sur toutes les trames observées.
    // Son rôle distinct est inconnu (spec §4, §3).
    frame[3] = mask;
    frame[4] = MODE_FIXED;
    frame[5] = DEFAULT_SPEED;
    // L'offset 6 ne varie qu'en mode 0x05 (spec §4).
    frame[6] = 0x00;
    frame[7] = g;
    frame[8] = r;
    frame[9] = b;
    // Offset 56 : nombre de couleurs fournies — une seule en mode fixe.
    frame[56] = 0x01;
    // L'offset 57 est marqué ❓ dans la spec ; il reste nul en mode fixe (§11).
    frame[58] = leds;
    frame[59] = ACCESSORY_TYPE;
    frame
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
    match model {
        Model::RgbAndFan => vec![
            packet(&[0x10, 0x02]),
            packet(&[0x20, 0x03]),
            packet(&[0x60, 0x03]),
            // 0x03e8 = 1000, répété deux fois (spec §8).
            packet(&[0x60, 0x02, 0x01, 0xe8, 0x03, 0x01, 0xe8, 0x03]),
        ],
        Model::Rgb => vec![packet(&[0x10, 0x01]), packet(&[0x20, 0x03])],
    }
}
