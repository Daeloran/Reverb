//! Éclairage de la RAM Corsair Dominator Titanium DDR5, par SMBus.
//!
//! Toutes les valeurs viennent de `docs/SPEC-CORSAIR-RAM.md`, issue d'une
//! rétro-ingénierie d'iCUE. Le §1 en donne le résultat structurant : **Corsair
//! n'emploie aucun protocole propriétaire**. Les barrettes se joignent par des
//! transferts SMBus par bloc standard, sur le contrôleur que `i2c-piix4` gère
//! déjà — il n'y a rien à réimplémenter côté bus.
//!
//! ⚠️ **C'est la seule cible du projet où une erreur est irréversible.** Le
//! même bus porte les hubs SPD des barrettes en `0x50`–`0x53` (spec §3) : une
//! écriture au mauvais endroit corrompt le SPD et rend un DIMM non démarrable.
//! D'où [`SlotAddress`], qui ne se construit que depuis un index
//! d'emplacement : aucune API publique de ce module ne fabrique une adresse
//! arbitraire.
//!
//! ⚠️ **L'ordre des composantes n'est pas celui des autres cibles.** Les LED
//! NZXT sont en GRB, l'écran du Kraken en BGR, cette RAM en **RGB** (spec
//! §4.1). Une erreur d'ordre ne produit aucun message — juste une mauvaise
//! couleur. L'ordre est écrit une seule fois, dans [`payload`].
//!
//! Le contrôleur **n'a pas de watchdog** : une couleur écrite tient
//! indéfiniment sans hôte. Mais il ne sait pas animer seul — toute animation
//! est recalculée et réécrite par l'hôte (spec §4.5, tests 2 et 3). C'est la
//! seule contrainte temps réel du projet.

use std::fmt;

use crate::color::Rgb;

/// Nombre de LED d'une barrette — octet 0 de la charge utile (spec §4.1).
pub const LEDS_PER_STICK: usize = 11;

/// Nombre de barrettes, une par adresse de `0x18` à `0x1b` (spec §3).
pub const SLOT_COUNT: usize = 4;

/// Charge utile logique : le compte, onze triplets, le CRC (spec §4.1).
pub const PAYLOAD_LEN: usize = 1 + LEDS_PER_STICK * 3 + 1;

/// Registre du premier bloc (spec §4.3).
pub const REGISTER_HEAD: u8 = 0x31;

/// Registre du second bloc (spec §4.3).
pub const REGISTER_TAIL: u8 = 0x32;

/// Octets de charge utile portés par le premier bloc.
///
/// Un bloc SMBus est limité à 32 octets ; les 35 de la charge utile sont donc
/// scindés (spec §4.3).
pub const HEAD_LEN: usize = 32;

/// Octets de charge utile portés par le second bloc, dont le CRC.
pub const TAIL_LEN: usize = PAYLOAD_LEN - HEAD_LEN;

/// Longueur du premier transfert tel qu'il part sur le fil.
pub const HEAD_TRANSFER_LEN: usize = 2 + HEAD_LEN;

/// Longueur du second transfert tel qu'il part sur le fil.
pub const TAIL_TRANSFER_LEN: usize = 2 + TAIL_LEN;

/// Adresse SMBus de la première barrette (spec §3).
const FIRST_ADDRESS: u8 = 0x18;

/// Polynôme du CRC-8/ATM (spec §4.2).
const CRC_POLYNOMIAL: u8 = 0x07;

/// Adresse SMBus du contrôleur RGB d'une barrette (spec §3).
///
/// Ne se construit **que** depuis un index d'emplacement, et ne retient que
/// lui : l'adresse est calculée à la lecture. Il n'existe aucun constructeur
/// prenant une adresse, ce qui rend `0x50` — le hub SPD — inatteignable, et
/// pas seulement refusé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotAddress(u8);

impl SlotAddress {
    /// Les quatre emplacements, dans l'ordre.
    pub const ALL: [SlotAddress; SLOT_COUNT] = [
        SlotAddress(0),
        SlotAddress(1),
        SlotAddress(2),
        SlotAddress(3),
    ];

    /// Emplacement `0..=3`, numéroté comme iCUE le journalise : `DIMM 0x1800`,
    /// `0x1901`, `0x1a02`, `0x1b03`, où l'octet bas est l'index (spec §3).
    ///
    /// # Erreurs
    ///
    /// [`UnknownSlot`] pour tout le reste.
    pub fn new(slot: usize) -> Result<SlotAddress, UnknownSlot> {
        if slot >= SLOT_COUNT {
            return Err(UnknownSlot { given: slot });
        }
        // Le rétrécissement est sûr : `slot` est inférieur à `SLOT_COUNT`, soit 4.
        Ok(SlotAddress(slot as u8))
    }

    /// Adresse SMBus 7 bits — dans `0x18..=0x1b`, par construction.
    pub const fn address(self) -> u8 {
        FIRST_ADDRESS + self.0
    }

    /// Index d'emplacement d'origine.
    pub const fn slot(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for SlotAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "barrette {} ({:#04x})", self.slot(), self.address())
    }
}

/// L'emplacement demandé n'existe pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSlot {
    pub given: usize,
}

impl fmt::Display for UnknownSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "emplacement {} inconnu : les barrettes sont numérotées 0 à {}",
            self.given,
            SLOT_COUNT - 1
        )
    }
}

impl std::error::Error for UnknownSlot {}

/// La charge utile n'a pas reçu une couleur par LED.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedCountError {
    pub given: usize,
}

impl fmt::Display for LedCountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "une barrette attend exactement {LEDS_PER_STICK} couleurs, une par LED ; {} reçue(s)",
            self.given
        )
    }
}

impl std::error::Error for LedCountError {}

/// Charge utile logique de 35 octets : `[0x0b][11 × RGB][CRC-8]` (spec §4.1).
///
/// # Erreurs
///
/// [`LedCountError`] si le nombre de couleurs n'est pas exactement
/// [`LEDS_PER_STICK`]. Aucun complément, aucune répétition, aucune valeur par
/// défaut : une charge utile silencieusement fausse coûte plus cher qu'un
/// refus.
pub fn payload(colors: &[Rgb]) -> Result<[u8; PAYLOAD_LEN], LedCountError> {
    if colors.len() != LEDS_PER_STICK {
        return Err(LedCountError {
            given: colors.len(),
        });
    }

    let mut charge = [0u8; PAYLOAD_LEN];
    charge[0] = LEDS_PER_STICK as u8;

    // ⚠️ Seul endroit du module qui connaît l'ordre des composantes : **RGB**,
    // sans permutation (spec §4.1). Les ventilateurs sont en GRB et l'écran en
    // BGR — si l'ordre de la RAM devait changer, c'est ici, et nulle part
    // ailleurs.
    for (couleur, place) in colors.iter().zip(charge[1..].chunks_mut(3)) {
        place.copy_from_slice(&[couleur.r, couleur.g, couleur.b]);
    }

    charge[PAYLOAD_LEN - 1] = crc8(&charge[..PAYLOAD_LEN - 1]);
    Ok(charge)
}

/// Les deux transferts SMBus, décrits comme ils partent sur le fil :
/// `[registre][compte][données]`, la forme du §4.4 où le registre alimente
/// `SMBHSTCMD` et le compte `SMBHSTDAT0`. Les deux se suivent immédiatement,
/// vers la même adresse.
///
/// ⚠️ Cette forme décrit le **fil**, pas l'appel système. Un `write()` sur
/// `/dev/i2c-*` ne les émet pas : `i2c-piix4` est un contrôleur SMBus pur et
/// n'expose aucun algorithme I2C brut. C'est `reverb_cli::i2c::Bus::write_block`
/// qui les remet au noyau, par l'ioctl `I2C_SMBUS`.
///
/// # Erreurs
///
/// Celles de [`payload`], rendues avant qu'un seul octet ne soit mis en forme.
pub fn transfers(
    colors: &[Rgb],
) -> Result<([u8; HEAD_TRANSFER_LEN], [u8; TAIL_TRANSFER_LEN]), LedCountError> {
    let charge = payload(colors)?;

    let mut tete = [0u8; HEAD_TRANSFER_LEN];
    tete[0] = REGISTER_HEAD;
    tete[1] = HEAD_LEN as u8;
    tete[2..].copy_from_slice(&charge[..HEAD_LEN]);

    let mut queue = [0u8; TAIL_TRANSFER_LEN];
    queue[0] = REGISTER_TAIL;
    queue[1] = TAIL_LEN as u8;
    queue[2..].copy_from_slice(&charge[HEAD_LEN..]);

    Ok((tete, queue))
}

/// CRC-8/ATM : polynôme `0x07`, valeur initiale `0x00`, sans réflexion ni XOR
/// final (spec §4.2, vérifié sur 40 blocs de la capture).
fn crc8(donnees: &[u8]) -> u8 {
    let mut reste = 0u8;
    for &octet in donnees {
        reste ^= octet;
        for _ in 0..8 {
            reste = if reste & 0x80 != 0 {
                (reste << 1) ^ CRC_POLYNOMIAL
            } else {
                reste << 1
            };
        }
    }
    reste
}
