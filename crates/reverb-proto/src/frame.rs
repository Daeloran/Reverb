//! Construction des trames HID.
//!
//! ⚠️ Le premier octet de la trame **est** l'identifiant de rapport HID.
//! `OutputReportByteLength` vaut 64 et non 65 : il ne faut donc **pas**
//! préfixer un octet `0x00` comme le veut la convention habituelle (spec §0).

use crate::mode::ColorCountError;
use crate::{Brightness, Mode, Model, Rgb};

/// Longueur d'une trame HID, complétée par des zéros (spec §1).
pub const FRAME_LEN: usize = 64;

/// Une trame prête à écrire sur `/dev/hidraw*`.
pub type Frame = [u8; FRAME_LEN];

/// Type d'accessoire annoncé à l'offset 59 (spec §4).
const ACCESSORY_TYPE: u8 = 0x03;

/// Premier octet de couleur, offset 7 (spec §4).
const COLORS_OFFSET: usize = 7;

/// Construit un paquet de 64 octets à partir de ses octets significatifs.
///
/// Spec §1 — « Les paquets sont toujours complétés à 64 octets par des zéros. »
fn packet(head: &[u8]) -> Frame {
    let mut frame = [0u8; FRAME_LEN];
    frame[..head.len()].copy_from_slice(head);
    frame
}

/// Construit la trame `0x2a 0x04` d'un mode d'animation (spec §4).
///
/// ```text
/// offset  0    1     2      3      4     5       6      7..      56       57
///        0x2a 0x04 masque masque  mode vitesse variante couleurs nb  constante
/// ```
///
/// L'animation est ensuite exécutée **par le contrôleur** : l'hôte peut
/// s'arrêter, l'éclairage continue (spec §0.3).
///
/// La luminosité est appliquée **ici**, aux composantes, car le protocole n'a
/// aucun octet dédié (spec §4.3).
///
/// # Erreurs
///
/// Si le nombre de couleurs sort des bornes du mode (spec §4.1). C'est une
/// contrainte du protocole, pas de l'interface : elle est donc validée ici, et
/// jamais contournée par une trame de repli.
pub fn animation(
    mask: u8,
    mode: Mode,
    colors: &[Rgb],
    speed: u8,
    brightness: Brightness,
    leds: u8,
) -> Result<Frame, ColorCountError> {
    mode.check_colors(colors.len())?;

    let mut frame = [0u8; FRAME_LEN];
    frame[0] = 0x2a;
    frame[1] = 0x04;
    frame[2] = mask;
    // L'offset 3 est resté égal à l'offset 2 sur toutes les trames observées.
    // Son rôle distinct est inconnu (spec §4, §3).
    frame[3] = mask;
    frame[4] = mode.code();
    frame[5] = speed;
    frame[6] = mode.variant();

    for (index, color) in colors.iter().enumerate() {
        let [g, r, b] = color.with_brightness(brightness).to_grb();
        let base = COLORS_OFFSET + index * 3;
        frame[base] = g;
        frame[base + 1] = r;
        frame[base + 2] = b;
    }

    // Offset 56 : nombre de couleurs annoncé. Il ne descend **jamais** à zéro —
    // Spectrum Wave, qui n'en attend aucune, porte tout de même 1 avec un
    // triplet noir, exactement comme CAM (spec §4.4). Les octets de couleur
    // sont déjà nuls.
    frame[56] = colors.len().max(1) as u8;
    frame[57] = mode.flag();
    frame[58] = leds;
    frame[59] = ACCESSORY_TYPE;
    Ok(frame)
}

/// Construit la trame `0x2a 0x04` en mode couleur fixe (spec §4).
///
/// Cas particulier d'[`animation`] : mode `0x00`, une seule couleur, vitesse
/// `0x32` — sans effet ici, mais c'est ce que CAM émet (spec §4.2).
pub fn fixed_color(mask: u8, color: Rgb, brightness: Brightness, leds: u8) -> Frame {
    animation(
        mask,
        Mode::FIXED,
        &[color],
        Mode::FIXED.default_speed(),
        brightness,
        leds,
    )
    .expect("le mode fixe accepte exactement une couleur, fournie ici")
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
