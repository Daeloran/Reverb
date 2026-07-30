//! Pilotage LED par LED — famille de commandes `0x22` (spec §5).
//!
//! Distincte de `0x2a 0x04` : là où celle-ci confie une animation au firmware,
//! celle-ci peint chaque LED individuellement. C'est ce qui permet des motifs
//! qu'aucun mode NZXT ne propose.
//!
//! ⚠️ Le protocole n'offre **aucune lecture**. `22 10` réécrit toujours les 24
//! octets du tampon complet — la capture montre onze réémissions successives
//! pour peindre huit LED une par une (spec §5.1). Peindre une LED sans toucher
//! aux autres exigerait donc un état côté hôte, que ce module ne tient pas.

use std::fmt;

use crate::LEDS_PER_FAN;

/// Comment le tampon est appliqué — offset 4 de `22 a0` (spec §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Apply {
    /// `0x01` — les couleurs du tampon restent telles quelles.
    Static,
    /// `0x02` — le contrôleur **fait tourner** le tampon (spec §5.4).
    ///
    /// Il décale le motif d'un cran à intervalle régulier, sans que l'hôte
    /// envoie quoi que ce soit de plus : une rotation à l'infini coûte trois
    /// trames, une seule fois.
    ///
    /// La vitesse occupe les offsets 5–6 en uint16 little-endian. 🔶 Son
    /// échelle n'est pas calibrée ; `0x006a` est la seule valeur observée dans
    /// la capture.
    Animated { speed: u16 },
}

impl Apply {
    /// Seule vitesse vue dans la capture, en mode animé (spec §5.2).
    pub const OBSERVED_SPEED: u16 = 0x006a;

    /// Octet 4 de la trame d'application.
    pub(crate) const fn code(self) -> u8 {
        match self {
            Apply::Static => 0x01,
            Apply::Animated { .. } => 0x02,
        }
    }

    /// Octets 5–6 de la trame d'application, en little-endian.
    ///
    /// Nuls en statique : c'est ce que la capture montre, et la vitesse n'a de
    /// toute façon rien à y régler.
    pub(crate) const fn speed(self) -> u16 {
        match self {
            Apply::Static => 0,
            Apply::Animated { speed } => speed,
        }
    }
}

/// Le tampon n'a pas reçu le nombre de couleurs qu'il attend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedCountError {
    pub given: usize,
}

impl fmt::Display for LedCountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "le tampon LED attend exactement {LEDS_PER_FAN} couleurs, une par LED ; {} reçue(s)",
            self.given
        )
    }
}

impl std::error::Error for LedCountError {}
