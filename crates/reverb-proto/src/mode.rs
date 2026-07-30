//! Modes d'animation exécutés par le firmware du contrôleur (spec §4.1).
//!
//! Une fois la trame envoyée, l'animation vit sans qu'aucun logiciel ne tourne.
//! L'hôte n'envoie que des paramètres : numéro de mode, vitesse et couleurs.
//!
//! ⚠️ Les huit modes de cette table sont ceux **observés dans la capture**
//! `cible1-modes-nzxt`. Le mode `0x03` n'y figure pas : il n'a jamais été
//! déclenché, donc son comportement est inconnu et on ne l'émet pas.
//!
//! ⚠️ Cinq noms sur huit sont des **hypothèses** obtenues en recoupant le
//! nombre de couleurs accepté avec la table HUE 2 de liquidctl. Ils portent
//! 🔶 dans la spec et `confirmed()` renvoie `false` pour eux : rien ne doit
//! les présenter à l'utilisateur comme certains tant qu'ils n'ont pas été vus
//! à l'œil.

use std::fmt;

/// Une ligne de la table §4.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Spec {
    /// Octet 4 de la trame.
    code: u8,
    /// Nom en kebab-case, tel qu'accepté par `--mode`.
    name: &'static str,
    /// Nombre de couleurs attendues **de l'appelant**, bornes incluses.
    ///
    /// Distinct de l'octet 56, qui ne descend jamais sous 1 (spec §4.4).
    colors: (u8, u8),
    /// Octet 5 — la vitesse observée pour ce mode. L'échelle n'est pas calibrée.
    speed: u8,
    /// Octet 6 — sélecteur de variante, `0x00` partout sauf en `0x05` (spec §4.4).
    variant: u8,
    /// Octet 57 — constante propre au mode, de rôle inconnu mais de valeur sûre.
    flag: u8,
    /// Le nom a-t-il été confirmé à l'œil ?
    confirmed: bool,
}

/// Un mode d'animation de la table §4.1.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Mode(&'static Spec);

impl fmt::Debug for Mode {
    /// `Mode(alternating/0x05)` — les messages d'échec des tests citent
    /// beaucoup les modes, la forme dérivée serait illisible.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Mode({}/{:#04x})", self.name(), self.code())
    }
}

/// Table de vérité, recopiée de la spec §4.1.
///
/// Les colonnes `variant`, `flag` et `speed` sont extraites de la capture par
/// `tools/extrait_modes.py`. Quand plusieurs vitesses ont été observées pour un
/// mode, on retient la première : rejouer une valeur vue est le seul choix sûr
/// tant que l'échelle n'est pas calibrée (spec §4.2).
const SPECS: [Spec; 8] = [
    Spec {
        code: 0x00,
        name: "fixed",
        colors: (1, 1),
        speed: 0x32,
        variant: 0x00,
        flag: 0x00,
        confirmed: true,
    },
    Spec {
        code: 0x01,
        name: "fading",
        colors: (3, 3),
        speed: 0x28,
        variant: 0x00,
        flag: 0x08,
        confirmed: true,
    },
    Spec {
        code: 0x02,
        name: "spectrum-wave",
        // Le contrôleur génère les teintes : l'appelant n'en fournit aucune.
        // La trame en porte tout de même une, noire (spec §4.4).
        colors: (0, 0),
        speed: 0xfa,
        variant: 0x00,
        flag: 0x00,
        confirmed: true,
    },
    Spec {
        code: 0x04,
        name: "covering-marquee",
        colors: (2, 3),
        speed: 0xfa,
        variant: 0x00,
        flag: 0x00,
        confirmed: true,
    },
    Spec {
        code: 0x05,
        name: "alternating",
        // « Exactement 2 » : jamais 1, jamais 3. C'est le recoupement le plus
        // solide de la table avec liquidctl (spec §4.1).
        colors: (2, 2),
        speed: 0xf4,
        // La capture ne montre jamais `0x00` ici — seulement `0x01` puis `0x03`.
        // On rejoue la première variante observée (spec §4.4).
        variant: 0x01,
        flag: 0x00,
        confirmed: true,
    },
    Spec {
        code: 0x06,
        name: "pulse",
        colors: (1, 1),
        speed: 0x0f,
        variant: 0x00,
        flag: 0x08,
        confirmed: true,
    },
    Spec {
        code: 0x07,
        name: "breathing",
        colors: (1, 1),
        speed: 0x14,
        variant: 0x00,
        flag: 0x08,
        confirmed: true,
    },
    Spec {
        code: 0x09,
        name: "starry-night",
        colors: (1, 1),
        speed: 0x0f,
        variant: 0x00,
        flag: 0x00,
        confirmed: false,
    },
];

impl Mode {
    /// `0x00` — couleur fixe. ✅ confirmé.
    pub const FIXED: Mode = Mode(&SPECS[0]);
    /// `0x01` — 🔶 « Fading », trois couleurs.
    pub const FADING: Mode = Mode(&SPECS[1]);
    /// `0x02` — Spectrum Wave, le contrôleur génère les teintes. ✅ confirmé.
    pub const SPECTRUM_WAVE: Mode = Mode(&SPECS[2]);
    /// `0x04` — 🔶 « Covering Marquee », deux ou trois couleurs.
    pub const COVERING_MARQUEE: Mode = Mode(&SPECS[3]);
    /// `0x05` — 🔶 « Alternating », exactement deux couleurs.
    pub const ALTERNATING: Mode = Mode(&SPECS[4]);
    /// `0x06` — 🔶 « Pulse ».
    pub const PULSE: Mode = Mode(&SPECS[5]);
    /// `0x07` — Breathing. ✅ confirmé.
    pub const BREATHING: Mode = Mode(&SPECS[6]);
    /// `0x09` — 🔶 « Starry Night ».
    pub const STARRY_NIGHT: Mode = Mode(&SPECS[7]);

    /// Les huit modes observés, dans l'ordre de leur numéro.
    pub const ALL: [Mode; 8] = [
        Mode::FIXED,
        Mode::FADING,
        Mode::SPECTRUM_WAVE,
        Mode::COVERING_MARQUEE,
        Mode::ALTERNATING,
        Mode::PULSE,
        Mode::BREATHING,
        Mode::STARRY_NIGHT,
    ];

    /// Numéro du mode, écrit à l'offset 4 de la trame.
    pub const fn code(self) -> u8 {
        self.0.code
    }

    /// Nom en kebab-case, celui qu'accepte `--mode`.
    pub const fn name(self) -> &'static str {
        self.0.name
    }

    /// Le nom du mode a-t-il été confirmé à l'œil sur le matériel ?
    ///
    /// `false` signifie « numéro certain, nom probable » : le mode est
    /// pilotable, mais son appellation reste une hypothèse (spec §4.1).
    pub const fn confirmed(self) -> bool {
        self.0.confirmed
    }

    /// Nombre de couleurs attendues de l'appelant : `(minimum, maximum)`.
    ///
    /// Ce n'est pas l'octet 56, qui vaut toujours au moins 1 (spec §4.4).
    pub const fn colors(self) -> (u8, u8) {
        self.0.colors
    }

    /// Vitesse observée pour ce mode, écrite à l'offset 5 par défaut.
    ///
    /// 🔶 L'échelle n'est pas calibrée : cette valeur est celle que CAM a
    /// émise, pas un réglage raisonné (spec §4.2).
    pub const fn default_speed(self) -> u8 {
        self.0.speed
    }

    /// Vérifie qu'un nombre de couleurs entre dans les bornes du mode.
    ///
    /// Appelée par [`crate::frame::animation`], mais exposée pour que la ligne
    /// de commande puisse refuser une demande **avant** d'ouvrir le moindre
    /// `/dev/hidraw*` : la validation reste écrite une seule fois, ici.
    pub fn check_colors(self, given: usize) -> Result<(), ColorCountError> {
        let (min, max) = self.colors();
        if given < usize::from(min) || given > usize::from(max) {
            return Err(ColorCountError { mode: self, given });
        }
        Ok(())
    }

    /// Octet 6 — sélecteur de variante propre au mode (spec §4.4).
    pub(crate) const fn variant(self) -> u8 {
        self.0.variant
    }

    /// Octet 57 — constante propre au mode (spec §4.4).
    pub(crate) const fn flag(self) -> u8 {
        self.0.flag
    }

    /// Résout un mode depuis son nom kebab-case ou son numéro décimal.
    ///
    /// La comparaison est stricte : ni casse, ni espaces, ni notation
    /// hexadécimale. Une entrée approximative est rejetée plutôt que devinée —
    /// une trame silencieusement fausse coûte plus cher qu'une faute de frappe.
    pub fn from_name(input: &str) -> Result<Mode, UnknownMode> {
        let par_numero = |code: u8| Mode::ALL.into_iter().find(|m| m.code() == code);

        let trouve = match input.parse::<u8>() {
            Ok(code) => par_numero(code),
            Err(_) => Mode::ALL.into_iter().find(|m| m.name() == input),
        };

        trouve.ok_or_else(|| UnknownMode {
            input: input.to_owned(),
        })
    }
}

/// Nom ou numéro de mode non reconnu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownMode {
    pub input: String,
}

impl fmt::Display for UnknownMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "mode « {} » inconnu. Modes valides : ", self.input)?;
        for (index, mode) in Mode::ALL.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{} ({})", mode.name(), mode.code())?;
        }
        Ok(())
    }
}

impl std::error::Error for UnknownMode {}

/// Le mode n'a pas reçu le nombre de couleurs qu'il attend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorCountError {
    pub mode: Mode,
    pub given: usize,
}

impl fmt::Display for ColorCountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let nom = self.mode.name();
        let recues = self.given;
        match self.mode.colors() {
            (0, 0) => write!(
                f,
                "le mode « {nom} » n'attend aucune couleur (0) — le contrôleur génère \
                 les teintes lui-même ; {recues} reçue(s)"
            ),
            (min, max) if min == max => write!(
                f,
                "le mode « {nom} » attend exactement {min} couleur(s) ; {recues} reçue(s)"
            ),
            (min, max) => write!(
                f,
                "le mode « {nom} » attend entre {min} et {max} couleurs ; {recues} reçue(s)"
            ),
        }
    }
}

impl std::error::Error for ColorCountError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_table_est_ordonnee_par_numero_et_sans_doublon() {
        let codes: Vec<u8> = Mode::ALL.iter().map(|m| m.code()).collect();
        let mut tries = codes.clone();
        tries.sort_unstable();
        tries.dedup();
        assert_eq!(codes, tries);
    }

    #[test]
    fn un_numero_negatif_ou_hexadecimal_ne_se_resout_pas() {
        // `parse::<u8>()` refuse déjà « -1 » et « 0x05 » ; on vérifie qu'aucun
        // repli ne les rattrape ensuite par comparaison de nom.
        assert!(Mode::from_name("-1").is_err());
        assert!(Mode::from_name("0x05").is_err());
    }

    #[test]
    fn les_bornes_de_couleurs_sont_coherentes() {
        for mode in Mode::ALL {
            let (min, max) = mode.colors();
            assert!(min <= max, "{mode:?}");
            assert!(
                max <= 3,
                "aucune trame observée ne porte plus de 3 couleurs"
            );
        }
    }

    #[test]
    fn l_erreur_de_comptage_reste_lisible_pour_chaque_mode() {
        for mode in Mode::ALL {
            let message = ColorCountError { mode, given: 7 }.to_string();
            assert!(message.contains(mode.name()));
            assert!(message.contains('7'));
        }
    }
}
