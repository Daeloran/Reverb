//! Tests d'intention des modes d'animation matériels (issue #3).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #3, son commentaire
//! « Contrat d'API » et `docs/SPEC-PROTOCOLE-NZXT.md` seuls. Aucun octet n'est
//! relu depuis `src/` : à l'écriture de ce fichier, `Mode`, `animation`,
//! `UnknownMode` et `ColorCountError` n'existent pas encore.
//!
//! Comme pour `spec.rs` (#1) : ces tests encodent ce que le logiciel doit
//! faire, pas ce que le code fait. Si l'un d'eux échoue après implémentation,
//! c'est le code qu'on corrige, jamais le test.
//!
//! Les valeurs attendues viennent de la table §4.1 de la spec — colonnes
//! `off6`, `off57` et vitesses **extraites de la capture**
//! `cible1-modes-nzxt-20260730-102329.pcap` par `tools/extrait_modes.py` — et
//! sont reprises telles quelles dans la table de vérité du contrat d'API de
//! l'issue #3.
//!
//! ⚠️ Le nom du mode `0x09` reste 🔶 (spec §4.1, §4.5) : `confirmed()` doit
//! continuer à dire qu'il n'a pas été vérifié à l'œil. Les quatre autres
//! hypothèses ont été confirmées le 2026-07-30 (§4.5).
//!
//! Le jour où un mode est vu à l'œil, c'est la **spec §4.1** qui change, et les
//! tests de `confirmed()` la suivent. Ce n'est pas une entorse au garde-fou
//! « ne jamais modifier un test d'intention » : ce garde-fou interdit de plier
//! un test pour faire passer du code, pas de réécrire un test quand la
//! spécification dont il découle a bougé. La distinction tient à l'ordre — la
//! spec d'abord, le test ensuite, le code en dernier.
//!
//! Aucun accès matériel, aucune IO (issue #3, critère « Aucun accès matériel
//! dans les tests automatisés »).

use reverb_proto::{FRAME_LEN, Rgb};

/// Trame attendue : 64 octets à zéro, puis les octets significatifs.
///
/// Spec §1 — « Les paquets sont toujours complétés à 64 octets par des zéros ».
fn trame_avec(octets: &[(usize, u8)]) -> [u8; FRAME_LEN] {
    let mut t = [0u8; FRAME_LEN];
    for &(offset, valeur) in octets {
        t[offset] = valeur;
    }
    t
}

/// `n` couleurs distinctes, sans autre propriété que d'être différentes.
///
/// Sert à faire varier le **nombre** de couleurs fournies (spec §4.1, colonne
/// « Couleurs ») sans que la valeur des composantes n'entre en jeu.
fn couleurs(n: u8) -> Vec<Rgb> {
    (0..n)
        .map(|i| Rgb::new(0x10 * (i + 1), 0x20, 0x30))
        .collect()
}

// ---------------------------------------------------------------------------

/// La table de vérité des huit modes.
///
/// Contrat d'API de l'issue #3, recopié tel quel ; lui-même issu de la spec
/// §4.1 (numéros de mode ✅, colonnes extraites de la capture) :
///
///     | Constante          | code   | nom                | couleurs | vitesse | off6   | off57  | confirmé |
///     |--------------------|--------|--------------------|----------|---------|--------|--------|----------|
///     | `FIXED`            | `0x00` | `fixed`            | 1..=1    | `0x32`  | `0x00` | `0x00` | ✅       |
///     | `FADING`           | `0x01` | `fading`           | 3..=3    | `0x28`  | `0x00` | `0x08` | ✅       |
///     | `SPECTRUM_WAVE`    | `0x02` | `spectrum-wave`    | 0..=0    | `0xfa`  | `0x00` | `0x00` | ✅       |
///     | `COVERING_MARQUEE` | `0x04` | `covering-marquee` | 2..=3    | `0xfa`  | `0x00` | `0x00` | ✅       |
///     | `ALTERNATING`      | `0x05` | `alternating`      | 2..=2    | `0xf4`  | `0x01` | `0x00` | ✅       |
///     | `PULSE`            | `0x06` | `pulse`            | 1..=1    | `0x0f`  | `0x00` | `0x08` | ✅       |
///     | `BREATHING`        | `0x07` | `breathing`        | 1..=1    | `0x14`  | `0x00` | `0x08` | ✅       |
///     | `STARRY_NIGHT`     | `0x09` | `starry-night`     | 1..=1    | `0x0f`  | `0x00` | `0x00` | 🔶       |
mod table {
    use reverb_proto::Mode;

    /// Vérifie une ligne de la table de vérité, accesseur par accesseur.
    fn verifie(mode: Mode, code: u8, nom: &str, couleurs: (u8, u8), vitesse: u8, confirme: bool) {
        assert_eq!(mode.code(), code, "code de {mode:?}");
        assert_eq!(mode.name(), nom, "nom de {mode:?}");
        assert_eq!(mode.colors(), couleurs, "couleurs attendues de {mode:?}");
        assert_eq!(
            mode.default_speed(),
            vitesse,
            "vitesse par défaut de {mode:?}"
        );
        assert_eq!(
            mode.confirmed(),
            confirme,
            "confirmation à l'œil de {mode:?}"
        );
    }

    #[test]
    fn le_mode_fixe_est_le_0x00() {
        // spec §4.1, ligne `0x00` — 1 couleur, vitesse vue `0x32`,
        // ✅ « couleur fixe ». Contrat d'API : nom `fixed`.
        verifie(Mode::FIXED, 0x00, "fixed", (1, 1), 0x32, true);
    }

    #[test]
    fn le_mode_fading_est_le_0x01() {
        // spec §4.1, ligne `0x01` — 3 couleurs, vitesse vue `0x28`, ✅ « Fading »
        // confirmé à l'œil (§4.5 : « changement de couleurs unies douce »).
        // Contrat d'API, arbitrage « bornes de couleurs » : `3..=3`, ce qui est
        // observé, et non le 1..8 de liquidctl.
        verifie(Mode::FADING, 0x01, "fading", (3, 3), 0x28, true);
    }

    #[test]
    fn le_mode_spectrum_wave_est_le_0x02() {
        // spec §4.1, ligne `0x02` — ✅ « Spectrum Wave, le contrôleur génère les
        // teintes », vitesses vues `0xfa` puis `0x50` (la première est la
        // référence, contrat d'API). Zéro couleur attendue de l'appelant :
        // §4.4, « le mode ignore la couleur fournie ».
        verifie(
            Mode::SPECTRUM_WAVE,
            0x02,
            "spectrum-wave",
            (0, 0),
            0xfa,
            true,
        );
    }

    #[test]
    fn le_mode_covering_marquee_est_le_0x04() {
        // spec §4.1, ligne `0x04` — « 2 ou 3 » couleurs, vitesse vue `0xfa`,
        // ✅ « Covering Marquee » confirmé à l'œil (§4.5 : « les couleurs
        // recouvrent la surface LED par LED »).
        verifie(
            Mode::COVERING_MARQUEE,
            0x04,
            "covering-marquee",
            (2, 3),
            0xfa,
            true,
        );
    }

    #[test]
    fn le_mode_alternating_est_le_0x05() {
        // spec §4.1, ligne `0x05` — « exactement 2 » couleurs, vitesses vues
        // `0xf4` puis `0xe8`, ✅ « Alternating » confirmé à l'œil (§4.5 : « les
        // LED changent de couleur en groupe »).
        // Contrat d'API : la première variante observée fait référence, d'où
        // `0xf4`.
        verifie(Mode::ALTERNATING, 0x05, "alternating", (2, 2), 0xf4, true);
    }

    #[test]
    fn le_mode_pulse_est_le_0x06() {
        // spec §4.1, ligne `0x06` — 1 couleur, vitesse vue `0x0f`, ✅ « Pulse »
        // confirmé à l'œil (§4.5 : « pulsation abrupte », distincte du fondu
        // lent de Breathing).
        verifie(Mode::PULSE, 0x06, "pulse", (1, 1), 0x0f, true);
    }

    #[test]
    fn le_mode_breathing_est_le_0x07() {
        // spec §4.1, ligne `0x07` — 1 couleur, vitesse vue `0x14`,
        // ✅ « Breathing ». Issue #3 : l'un des trois modes documentés.
        verifie(Mode::BREATHING, 0x07, "breathing", (1, 1), 0x14, true);
    }

    #[test]
    fn le_mode_starry_night_est_le_0x09() {
        // spec §4.1, ligne `0x09` — 1 couleur, vitesse vue `0x0f`,
        // 🔶 « Starry Night » : le seul nom encore non confirmé (§4.5).
        verifie(
            Mode::STARRY_NIGHT,
            0x09,
            "starry-night",
            (1, 1),
            0x0f,
            false,
        );
    }

    #[test]
    fn all_contient_les_huit_modes_dans_l_ordre_de_la_table() {
        // contrat d'API — `pub const ALL: [Mode; 8]`.
        // spec §4.1 — huit lignes observées : `0x00`, `0x01`, `0x02`, `0x04`,
        // `0x05`, `0x06`, `0x07`, `0x09`. Elles sont énumérées dans l'ordre des
        // codes, comme la table du contrat d'API — c'est cet ordre que
        // `reverb modes` affiche.
        assert_eq!(Mode::ALL.len(), 8);
        let codes: Vec<u8> = Mode::ALL.iter().map(|m| m.code()).collect();
        assert_eq!(codes, vec![0x00, 0x01, 0x02, 0x04, 0x05, 0x06, 0x07, 0x09]);

        let noms: Vec<&str> = Mode::ALL.iter().map(|m| m.name()).collect();
        assert_eq!(
            noms,
            vec![
                "fixed",
                "fading",
                "spectrum-wave",
                "covering-marquee",
                "alternating",
                "pulse",
                "breathing",
                "starry-night",
            ]
        );
    }

    #[test]
    fn aucun_code_ni_aucun_nom_n_est_en_double() {
        // contrat d'API — la table est une bijection constante ↔ code ↔ nom.
        // Un doublon rendrait `from_name` ambigu et `reverb modes` trompeur.
        let mut codes: Vec<u8> = Mode::ALL.iter().map(|m| m.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), 8, "chaque mode porte un code distinct");

        let mut noms: Vec<&str> = Mode::ALL.iter().map(|m| m.name()).collect();
        noms.sort_unstable();
        noms.dedup();
        assert_eq!(noms.len(), 8, "chaque mode porte un nom distinct");
    }

    #[test]
    fn le_mode_0x03_n_existe_pas() {
        // spec §4.1, ligne `0x03` — « ❓ jamais déclenché pendant la capture »,
        // et §10 question 11. Contrat d'API, arbitrage : « le mode `0x03`
        // n'existe pas dans `ALL` ». CLAUDE.md — « ne jamais inventer une trame
        // absente des specs ».
        assert!(
            Mode::ALL.iter().all(|m| m.code() != 0x03),
            "le mode `0x03` n'a jamais été observé : il ne doit pas être émis"
        );
    }

    #[test]
    fn seul_starry_night_reste_a_confirmer() {
        // spec §4.5, session du 2026-07-30 — quatre des cinq hypothèses ont été
        // confirmées à l'œil par une description spécifique du mécanisme.
        // `0x09` reste 🔶 : « je crois voir un léger scintillement mais ce n'est
        // pas très net » est compatible avec Starry Night sans être
        // discriminante.
        //
        // Ce test a bougé quand la spec a bougé, comme annoncé en tête de
        // fichier. L'ordre est resté : spec, puis test, puis code.
        let a_confirmer: Vec<u8> = Mode::ALL
            .iter()
            .filter(|m| !m.confirmed())
            .map(|m| m.code())
            .collect();
        assert_eq!(a_confirmer, vec![0x09]);

        assert!(
            !Mode::STARRY_NIGHT.confirmed(),
            "rien ne doit présenter « starry-night » comme certain (spec §4.5)"
        );
    }

    #[test]
    fn chaque_nom_est_en_kebab_case() {
        // contrat d'API — `name()` : « "spectrum-wave", kebab-case ». C'est ce
        // que `--mode <NOM>` accepte sur la ligne de commande.
        for mode in Mode::ALL {
            let nom = mode.name();
            assert!(!nom.is_empty(), "nom vide pour {mode:?}");
            assert!(
                nom.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "« {nom} » n'est pas en kebab-case ASCII ({mode:?})"
            );
            assert!(
                !nom.starts_with('-') && !nom.ends_with('-'),
                "« {nom} » ({mode:?})"
            );
        }
    }
}

// ---------------------------------------------------------------------------

mod resolution {
    use reverb_proto::{Mode, UnknownMode};

    /// Message d'erreur produit par un mode inconnu.
    ///
    /// Contrat d'API — `UnknownMode` : « Display : liste les noms valides ».
    fn message_d_erreur(entree: &str) -> String {
        let erreur: UnknownMode = Mode::from_name(entree).expect_err("doit être rejeté");
        erreur.to_string()
    }

    #[test]
    fn chaque_nom_kebab_case_se_resout_vers_son_mode() {
        // issue #3, comportement attendu — `reverb set --all --mode spectrum-wave`,
        // `--mode breathing`. Contrat d'API — `from_name(&str)`.
        for mode in Mode::ALL {
            assert_eq!(
                Mode::from_name(mode.name()).ok(),
                Some(mode),
                "résolution de « {} »",
                mode.name()
            );
        }
    }

    #[test]
    fn chaque_numero_decimal_se_resout_vers_son_mode() {
        // issue #3, comportement attendu — `reverb set --all --mode 5 …`, et
        // `--mode <NOM|NUMÉRO>` du contrat d'API. Le numéro est le code de la
        // spec §4.1, en décimal.
        for mode in Mode::ALL {
            let numero = mode.code().to_string();
            assert_eq!(
                Mode::from_name(&numero).ok(),
                Some(mode),
                "résolution du numéro « {numero} »"
            );
        }
    }

    #[test]
    fn le_numero_5_designe_alternating() {
        // issue #3 — `reverb set --all --mode 5 --color ff0000 --color 0000ff`,
        // exactement deux couleurs (spec §4.1, ligne `0x05`).
        assert_eq!(Mode::from_name("5").ok(), Some(Mode::ALTERNATING));
        assert_eq!(Mode::from_name("0").ok(), Some(Mode::FIXED));
        assert_eq!(Mode::from_name("9").ok(), Some(Mode::STARRY_NIGHT));
    }

    #[test]
    fn le_nom_et_la_resolution_font_un_aller_retour() {
        // contrat d'API — la table est une bijection nom ↔ mode.
        for mode in Mode::ALL {
            let resolu = Mode::from_name(mode.name()).ok();
            assert_eq!(resolu, Some(mode));
            assert_eq!(resolu.map(|m| m.name()), Some(mode.name()));
        }
    }

    #[test]
    fn le_mode_3_est_refuse_comme_n_importe_quel_inconnu() {
        // spec §4.1 — le mode `0x03` n'a « jamais été déclenché pendant la
        // capture ». Contrat d'API, arbitrage : « `--mode 3` doit échouer comme
        // n'importe quel inconnu ».
        assert!(Mode::from_name("3").is_err(), "le mode 3 n'est pas observé");
    }

    #[test]
    fn un_numero_hors_de_la_table_est_refuse() {
        // spec §4.1 — seuls huit codes ont été observés. CLAUDE.md — « ne jamais
        // inventer une trame absente des specs ».
        for entree in ["3", "8", "10", "255", "-1", "0x05"] {
            assert!(
                Mode::from_name(entree).is_err(),
                "« {entree} » ne désigne aucun mode observé"
            );
        }
    }

    #[test]
    fn un_nom_inconnu_est_rejete_en_listant_les_modes_valides() {
        // issue #3, critère d'acceptation — « Un mode inconnu produit une erreur
        // listant les modes valides ».
        let message = message_d_erreur("arc-en-ciel");
        for mode in Mode::ALL {
            assert!(
                message.contains(mode.name()),
                "l'erreur doit citer le mode valide « {} » : « {message} »",
                mode.name()
            );
        }
    }

    #[test]
    fn un_nom_vide_est_rejete() {
        // issue #3, même critère : rien ne doit être résolu par défaut — sans
        // `--mode`, c'est la CLI qui retient `fixed`, pas `from_name`.
        assert!(Mode::from_name("").is_err());
        assert!(Mode::from_name(" ").is_err());
    }

    #[test]
    fn un_nom_proche_mais_faux_est_rejete() {
        // issue #3 — la validation vient de `reverb-proto` : une faute de frappe
        // ne doit jamais produire une trame silencieusement fausse.
        for entree in ["spectrum wave", "spectrum_wave", "breath", "fixe"] {
            assert!(
                Mode::from_name(entree).is_err(),
                "« {entree} » n'est pas un nom de la table §4.1"
            );
        }
    }
}

// ---------------------------------------------------------------------------

mod trame {
    use super::{couleurs, trame_avec};
    use reverb_proto::frame::animation;
    use reverb_proto::{Brightness, FRAME_LEN, LEDS_PER_FAN, Mode, Rgb};

    /// Trame produite à luminosité pleine, à la vitesse par défaut du mode, sur
    /// un accessoire de 8 LED (spec §4, offset 58).
    fn trame_de(masque: u8, mode: Mode, couleurs: &[Rgb]) -> [u8; FRAME_LEN] {
        animation(
            masque,
            mode,
            couleurs,
            mode.default_speed(),
            Brightness::FULL,
            LEDS_PER_FAN,
        )
        .expect("le nombre de couleurs respecte la table §4.1")
    }

    /// Le plus petit jeu de couleurs accepté par le mode (spec §4.1).
    fn couleurs_minimales(mode: Mode) -> Vec<Rgb> {
        couleurs(mode.colors().0)
    }

    // --- Une trame de référence par mode, octet pour octet (issue #3) ---

    #[test]
    fn la_trame_du_mode_fixe_est_exacte_octet_pour_octet() {
        // spec §4 (table des offsets) et §4.1 (ligne `0x00`), sur l'exemple
        // `--color ff00ff` de l'issue #1, canal `0x01` = « bas gauche » (§3).
        // spec §2 — ordre G, R, B : `#ff00ff` part en `00 ff ff`.
        let attendue = trame_avec(&[
            (0, 0x2a),
            (1, 0x04),
            (2, 0x01),
            (3, 0x01),
            (4, 0x00),
            (5, 0x32),
            (6, 0x00),
            (7, 0x00), // G
            (8, 0xff), // R
            (9, 0xff), // B
            (56, 0x01),
            (57, 0x00),
            (58, 0x08),
            (59, 0x03),
        ]);

        let obtenue = trame_de(0x01, Mode::FIXED, &[Rgb::new(0xff, 0x00, 0xff)]);
        assert_eq!(obtenue, attendue);
    }

    #[test]
    fn la_trame_du_mode_fading_est_exacte_octet_pour_octet() {
        // spec §4.1, ligne `0x01` — 3 couleurs, vitesse `0x28`, off6 `0x00`,
        // off57 `0x08` (§4.4 : « `0x08` pour `0x01`, `0x06` et `0x07` »).
        // spec §4 — « offset 7… : couleurs, triplets GRB consécutifs ».
        // spec §2 — rouge `00 ff 00`, vert `ff 00 00`, bleu `00 00 ff`.
        // Canal `0x08` = « radiateur haut » (§3).
        let attendue = trame_avec(&[
            (0, 0x2a),
            (1, 0x04),
            (2, 0x08),
            (3, 0x08),
            (4, 0x01),
            (5, 0x28),
            (6, 0x00),
            (7, 0x00),  // G — rouge
            (8, 0xff),  // R
            (9, 0x00),  // B
            (10, 0xff), // G — vert
            (11, 0x00), // R
            (12, 0x00), // B
            (13, 0x00), // G — bleu
            (14, 0x00), // R
            (15, 0xff), // B
            (56, 0x03),
            (57, 0x08),
            (58, 0x08),
            (59, 0x03),
        ]);

        let obtenue = trame_de(
            0x08,
            Mode::FADING,
            &[
                Rgb::new(0xff, 0x00, 0x00),
                Rgb::new(0x00, 0xff, 0x00),
                Rgb::new(0x00, 0x00, 0xff),
            ],
        );
        assert_eq!(obtenue, attendue);
    }

    #[test]
    fn la_trame_du_mode_spectrum_wave_est_exacte_octet_pour_octet() {
        // issue #3, comportement attendu — `reverb set --all --mode spectrum-wave`,
        // sans `--color`.
        // spec §4.4 — « la trame réelle porte `off56 = 0x01` avec une couleur
        // **noire** `#000000` […] une implémentation qui écrirait `0x00`
        // s'écarterait de ce que fait CAM ».
        // spec §4.1, ligne `0x02` — vitesse `0xfa`, off6 `0x00`, off57 `0x00`.
        // Les offsets 7 à 9 sont donc nuls : c'est le triplet noir.
        let attendue = trame_avec(&[
            (0, 0x2a),
            (1, 0x04),
            (2, 0x02),
            (3, 0x02),
            (4, 0x02),
            (5, 0xfa),
            (6, 0x00),
            (56, 0x01),
            (57, 0x00),
            (58, 0x08),
            (59, 0x03),
        ]);

        let obtenue = trame_de(0x02, Mode::SPECTRUM_WAVE, &[]);
        assert_eq!(obtenue, attendue);
        assert_eq!(&obtenue[7..10], &[0x00, 0x00, 0x00], "triplet noir (§4.4)");
        assert_eq!(obtenue[56], 0x01, "off56 ne descend jamais à zéro (§4.4)");
    }

    #[test]
    fn la_trame_du_mode_covering_marquee_est_exacte_octet_pour_octet() {
        // spec §4.1, ligne `0x04` — « 2 ou 3 » couleurs, vitesse `0xfa`,
        // off6 `0x00`, off57 `0x00`. Ici deux couleurs, rouge puis bleu.
        // Canal `0x10` = « radiateur milieu » (§3).
        let attendue = trame_avec(&[
            (0, 0x2a),
            (1, 0x04),
            (2, 0x10),
            (3, 0x10),
            (4, 0x04),
            (5, 0xfa),
            (6, 0x00),
            (7, 0x00),  // G — rouge
            (8, 0xff),  // R
            (9, 0x00),  // B
            (10, 0x00), // G — bleu
            (11, 0x00), // R
            (12, 0xff), // B
            (56, 0x02),
            (57, 0x00),
            (58, 0x08),
            (59, 0x03),
        ]);

        let obtenue = trame_de(
            0x10,
            Mode::COVERING_MARQUEE,
            &[Rgb::new(0xff, 0x00, 0x00), Rgb::new(0x00, 0x00, 0xff)],
        );
        assert_eq!(obtenue, attendue);
    }

    #[test]
    fn la_trame_du_mode_covering_marquee_accepte_une_troisieme_couleur() {
        // spec §4.1, ligne `0x04` — « 2 ou 3 » couleurs. La troisième occupe le
        // triplet suivant (offsets 13 à 15) et l'offset 56 passe à `0x03` (§4).
        let obtenue = trame_de(
            0x10,
            Mode::COVERING_MARQUEE,
            &[
                Rgb::new(0xff, 0x00, 0x00),
                Rgb::new(0x00, 0x00, 0xff),
                Rgb::new(0x00, 0xff, 0x00),
            ],
        );
        assert_eq!(obtenue[56], 0x03);
        assert_eq!(
            &obtenue[13..16],
            &[0xff, 0x00, 0x00],
            "vert pur en GRB (§2)"
        );
    }

    #[test]
    fn la_trame_du_mode_alternating_est_exacte_octet_pour_octet() {
        // spec §4.1, ligne `0x05` — « exactement 2 » couleurs, vitesse `0xf4`,
        // off6 `0x01`, off57 `0x00`.
        // spec §4.4 — « en mode `0x05` [l'offset 6] vaut `0x01` puis `0x03` » :
        // c'est un sélecteur de variante, jamais `0x00`. Contrat d'API : on
        // rejoue la première variante observée.
        // Canal `0x20` = « radiateur bas » (§3).
        let attendue = trame_avec(&[
            (0, 0x2a),
            (1, 0x04),
            (2, 0x20),
            (3, 0x20),
            (4, 0x05),
            (5, 0xf4),
            (6, 0x01),
            (7, 0x00),  // G — rouge
            (8, 0xff),  // R
            (9, 0x00),  // B
            (10, 0x00), // G — bleu
            (11, 0x00), // R
            (12, 0xff), // B
            (56, 0x02),
            (57, 0x00),
            (58, 0x08),
            (59, 0x03),
        ]);

        let obtenue = trame_de(
            0x20,
            Mode::ALTERNATING,
            &[Rgb::new(0xff, 0x00, 0x00), Rgb::new(0x00, 0x00, 0xff)],
        );
        assert_eq!(obtenue, attendue);
    }

    #[test]
    fn la_trame_du_mode_pulse_est_exacte_octet_pour_octet() {
        // spec §4.1, ligne `0x06` — 1 couleur, vitesse `0x0f`, off6 `0x00`,
        // off57 `0x08` (§4.4). Canal `0x04` = « bas droite » (§3).
        // Vert pur → `ff 00 00` en GRB (§2).
        let attendue = trame_avec(&[
            (0, 0x2a),
            (1, 0x04),
            (2, 0x04),
            (3, 0x04),
            (4, 0x06),
            (5, 0x0f),
            (6, 0x00),
            (7, 0xff), // G
            (8, 0x00), // R
            (9, 0x00), // B
            (56, 0x01),
            (57, 0x08),
            (58, 0x08),
            (59, 0x03),
        ]);

        let obtenue = trame_de(0x04, Mode::PULSE, &[Rgb::new(0x00, 0xff, 0x00)]);
        assert_eq!(obtenue, attendue);
    }

    #[test]
    fn la_trame_du_mode_breathing_est_exacte_octet_pour_octet() {
        // issue #3, comportement attendu — `reverb set --all --mode breathing
        // --color ff0000`.
        // spec §4.1, ligne `0x07` — 1 couleur, vitesse `0x14`, off6 `0x00`,
        // off57 `0x08` (§4.4). Rouge pur → `00 ff 00` en GRB (§2).
        let attendue = trame_avec(&[
            (0, 0x2a),
            (1, 0x04),
            (2, 0x01),
            (3, 0x01),
            (4, 0x07),
            (5, 0x14),
            (6, 0x00),
            (7, 0x00), // G
            (8, 0xff), // R
            (9, 0x00), // B
            (56, 0x01),
            (57, 0x08),
            (58, 0x08),
            (59, 0x03),
        ]);

        let obtenue = trame_de(0x01, Mode::BREATHING, &[Rgb::new(0xff, 0x00, 0x00)]);
        assert_eq!(obtenue, attendue);
    }

    #[test]
    fn la_trame_du_mode_starry_night_est_exacte_octet_pour_octet() {
        // spec §4.1, ligne `0x09` — 1 couleur, vitesse `0x0f`, off6 `0x00`,
        // off57 `0x00`. Bleu pur → `00 00 ff` en GRB (§2, §5.1).
        let attendue = trame_avec(&[
            (0, 0x2a),
            (1, 0x04),
            (2, 0x02),
            (3, 0x02),
            (4, 0x09),
            (5, 0x0f),
            (6, 0x00),
            (7, 0x00), // G
            (8, 0x00), // R
            (9, 0xff), // B
            (56, 0x01),
            (57, 0x00),
            (58, 0x08),
            (59, 0x03),
        ]);

        let obtenue = trame_de(0x02, Mode::STARRY_NIGHT, &[Rgb::new(0x00, 0x00, 0xff)]);
        assert_eq!(obtenue, attendue);
    }

    // --- Les offsets, un par un, sur les huit modes ---

    #[test]
    fn les_offsets_0_et_1_valent_toujours_2a_04() {
        // spec §4 — la commande est `2a 04` pour tous les modes prédéfinis.
        // spec §0 — le premier octet est l'identifiant de rapport HID : pas de
        // `0x00` préfixé.
        for mode in Mode::ALL {
            let t = trame_de(0x01, mode, &couleurs_minimales(mode));
            assert_eq!(t[0], 0x2a, "offset 0 de {mode:?}");
            assert_eq!(t[1], 0x04, "offset 1 de {mode:?}");
        }
    }

    #[test]
    fn les_offsets_2_et_3_portent_le_meme_masque() {
        // spec §4 — offset 2 : masque de canaux ; offset 3 : « toujours égal à
        // l'offset 2 ». spec §3 — « les offsets 2 et 3 sont restés égaux sur
        // toutes les trames observées », et « CAM n'agrège jamais plusieurs
        // canaux dans une trame ».
        for mode in Mode::ALL {
            for masque in [0x01u8, 0x02, 0x04, 0x08, 0x10, 0x20] {
                let t = trame_de(masque, mode, &couleurs_minimales(mode));
                assert_eq!(t[2], masque, "offset 2, {mode:?}, masque {masque:#04x}");
                assert_eq!(t[3], t[2], "offset 3, {mode:?}, masque {masque:#04x}");
            }
        }
    }

    #[test]
    fn chaque_mode_place_son_numero_a_l_offset_4() {
        // issue #3, tests d'intention — « Chaque mode place son numéro à
        // l'offset 4 (spec §4) ». Les numéros de la spec §4.1 sont certains ✅.
        for mode in Mode::ALL {
            let t = trame_de(0x01, mode, &couleurs_minimales(mode));
            assert_eq!(t[4], mode.code(), "offset 4 de {mode:?}");
        }
    }

    #[test]
    fn l_offset_5_porte_la_vitesse_telle_quelle() {
        // issue #3, tests d'intention — « La vitesse est écrite telle quelle à
        // l'offset 5 (spec §4.2) », et critère d'acceptation « `--speed` accepte
        // une valeur brute `0-255`, documentée comme non calibrée ».
        for mode in Mode::ALL {
            for vitesse in [0x00u8, 0x0f, 0x50, 0xfa, 0xff] {
                let t = animation(
                    0x01,
                    mode,
                    &couleurs_minimales(mode),
                    vitesse,
                    Brightness::FULL,
                    LEDS_PER_FAN,
                )
                .expect("le nombre de couleurs respecte la table §4.1");
                assert_eq!(t[5], vitesse, "vitesse {vitesse:#04x} de {mode:?}");
            }
        }
    }

    #[test]
    fn la_vitesse_par_defaut_est_celle_du_mode() {
        // spec §4.2 — « valeurs observées : `0x32` en fixe, `0xfa` puis `0x50` en
        // Spectrum Wave, `0x14` en Breathing, `0x0f`, `0x28`, `0xe8`, `0xf4`
        // selon les modes ». Contrat d'API, arbitrage : « la vitesse par défaut
        // est celle de chaque mode, pas une constante globale ».
        for mode in Mode::ALL {
            let t = trame_de(0x01, mode, &couleurs_minimales(mode));
            assert_eq!(t[5], mode.default_speed(), "vitesse par défaut de {mode:?}");
        }
    }

    #[test]
    fn la_vitesse_ne_modifie_aucun_autre_octet() {
        // issue #3, tests d'intention — « La vitesse ne modifie aucun autre
        // octet ». spec §4.2 — l'octet 5 est la vitesse, rien d'autre.
        for mode in Mode::ALL {
            let couleurs = couleurs_minimales(mode);
            let lente = animation(0x01, mode, &couleurs, 0xfa, Brightness::FULL, LEDS_PER_FAN)
                .expect("le nombre de couleurs respecte la table §4.1");
            let rapide = animation(0x01, mode, &couleurs, 0x0f, Brightness::FULL, LEDS_PER_FAN)
                .expect("le nombre de couleurs respecte la table §4.1");

            for offset in (0..FRAME_LEN).filter(|o| *o != 5) {
                assert_eq!(
                    lente[offset], rapide[offset],
                    "l'offset {offset} de {mode:?} ne doit pas dépendre de la vitesse"
                );
            }
        }
    }

    #[test]
    fn l_offset_6_ne_vaut_01_qu_en_mode_alternating() {
        // spec §4 — offset 6 : « `0x00` sauf mode `0x05` où il vaut `0x01` ou
        // `0x03` ». spec §4.4 — sélecteur de variante ; la capture ne montre
        // jamais `0x00` sur ce mode.
        // Contrat d'API : `0x01` pour `ALTERNATING`, `0x00` sinon.
        for mode in Mode::ALL {
            let t = trame_de(0x01, mode, &couleurs_minimales(mode));
            let attendu = if mode == Mode::ALTERNATING {
                0x01
            } else {
                0x00
            };
            assert_eq!(t[6], attendu, "offset 6 de {mode:?}");
        }
    }

    #[test]
    fn l_offset_56_compte_les_couleurs_fournies() {
        // issue #3, tests d'intention — « L'offset 56 correspond au nombre de
        // couleurs réellement fournies (spec §4) ».
        // spec §4.4 — « l'offset 56 ne descend jamais à zéro » : Spectrum Wave,
        // qui n'attend aucune couleur, porte quand même `0x01`.
        for mode in Mode::ALL {
            let (min, max) = mode.colors();
            for nombre in min..=max {
                let t = trame_de(0x01, mode, &couleurs(nombre));
                let attendu = nombre.max(1);
                assert_eq!(
                    t[56], attendu,
                    "offset 56 de {mode:?} pour {nombre} couleurs"
                );
            }
        }
    }

    #[test]
    fn l_offset_57_est_la_constante_du_mode() {
        // spec §4.4 — « l'offset 57 est une constante propre au mode, pas du
        // bruit : `0x08` pour `0x01`, `0x06` et `0x07` ; `0x00` pour `0x00`,
        // `0x02`, `0x04`, `0x05` et `0x09` ». Sens ❓, valeur ✅ : « il suffit de
        // la reproduire ».
        for mode in Mode::ALL {
            let t = trame_de(0x01, mode, &couleurs_minimales(mode));
            let attendu = match mode.code() {
                0x01 | 0x06 | 0x07 => 0x08,
                _ => 0x00,
            };
            assert_eq!(t[57], attendu, "offset 57 de {mode:?}");
        }
    }

    #[test]
    fn les_offsets_58_et_59_portent_les_led_et_le_type_d_accessoire() {
        // spec §4 — offset 58 : « nombre de LED de l'accessoire » ; offset 59 :
        // « `0x03`, type d'accessoire ». spec §11 — `buf[58] = nb_led`.
        // Contrat d'API — l'offset 58 reçoit le paramètre `leds` tel quel.
        for mode in Mode::ALL {
            let t = trame_de(0x01, mode, &couleurs_minimales(mode));
            assert_eq!(t[58], LEDS_PER_FAN, "offset 58 de {mode:?}");
            assert_eq!(t[58], 0x08, "8 LED par ventilateur (§4, §5.1)");
            assert_eq!(t[59], 0x03, "offset 59 de {mode:?}");
        }

        // Le nombre de LED est celui qu'on passe, pas une constante figée.
        let t = animation(
            0x01,
            Mode::BREATHING,
            &couleurs(1),
            Mode::BREATHING.default_speed(),
            Brightness::FULL,
            4,
        )
        .expect("breathing accepte une couleur");
        assert_eq!(t[58], 4);
        assert_eq!(t[59], 0x03);
    }

    #[test]
    fn toutes_les_trames_font_exactement_64_octets() {
        // issue #3, tests d'intention — « Toutes les trames font exactement
        // 64 octets, complétées par des zéros ».
        // spec §1 — « HID interrupt, paquets de 64 octets » ; spec §0 —
        // `OutputReportByteLength` vaut 64 et non 65.
        assert_eq!(FRAME_LEN, 64);
        for mode in Mode::ALL {
            let (min, max) = mode.colors();
            for nombre in min..=max {
                let t = trame_de(0x01, mode, &couleurs(nombre));
                assert_eq!(t.len(), 64, "{mode:?} avec {nombre} couleurs");
            }
        }
    }

    #[test]
    fn les_octets_sans_signification_restent_nuls() {
        // spec §1 — « les paquets sont toujours complétés à 64 octets par des
        // zéros ». spec §4 — au-delà des triplets annoncés par l'offset 56,
        // rien n'est porté jusqu'au trailer.
        for mode in Mode::ALL {
            let couleurs = couleurs_minimales(mode);
            let t = trame_de(0x01, mode, &couleurs);
            // Spectrum Wave écrit un triplet noir malgré ses zéro couleurs (§4.4).
            let fin = 7 + 3 * couleurs.len().max(1);
            assert!(
                t[fin..56].iter().all(|&o| o == 0),
                "{mode:?} : offsets {fin}..56 doivent être nuls, obtenu {:?}",
                &t[fin..56]
            );
            assert!(
                t[60..64].iter().all(|&o| o == 0),
                "{mode:?} : offsets 60..64 doivent être nuls, obtenu {:?}",
                &t[60..64]
            );
        }
    }

    #[test]
    fn un_mode_a_une_couleur_laisse_les_offsets_10_a_55_nuls() {
        // spec §4 — une seule couleur occupe les offsets 7 à 9 ; le reste de la
        // zone 7..55 n'est pas utilisé. spec §11 — tampon zéro-initialisé.
        for mode in [
            Mode::FIXED,
            Mode::PULSE,
            Mode::BREATHING,
            Mode::STARRY_NIGHT,
        ] {
            let t = trame_de(0x01, mode, &[Rgb::new(0xff, 0xff, 0xff)]);
            assert!(
                t[10..56].iter().all(|&o| o == 0),
                "{mode:?} : offsets 10..56 : {:?}",
                &t[10..56]
            );
        }
    }

    // --- Couleurs : ordre GRB, ordre des triplets, luminosité ---

    #[test]
    fn les_couleurs_partent_en_grb() {
        // spec §2 — « les couleurs partent dans l'ordre G, R, B. Le rouge pur
        // part en `00 ff 00` et le vert pur en `ff 00 00` ». Une lecture RGB
        // naïve intervertit rouge et vert sur toute l'installation.
        let rouge = trame_de(0x01, Mode::BREATHING, &[Rgb::new(0xff, 0x00, 0x00)]);
        assert_eq!(&rouge[7..10], &[0x00, 0xff, 0x00]);

        let vert = trame_de(0x01, Mode::BREATHING, &[Rgb::new(0x00, 0xff, 0x00)]);
        assert_eq!(&vert[7..10], &[0xff, 0x00, 0x00]);

        let bleu = trame_de(0x01, Mode::BREATHING, &[Rgb::new(0x00, 0x00, 0xff)]);
        assert_eq!(&bleu[7..10], &[0x00, 0x00, 0xff]);

        // Quel que soit le mode, la conversion est la même.
        for mode in Mode::ALL {
            let (min, _) = mode.colors();
            if min == 0 {
                continue; // Spectrum Wave n'accepte aucune couleur (§4.4).
            }
            let couleur = Rgb::new(0x12, 0x34, 0x56);
            let t = trame_de(0x01, mode, &vec![couleur; min as usize]);
            assert_eq!(&t[7..10], &couleur.to_grb(), "premier triplet de {mode:?}");
            assert_eq!(&t[7..10], &[0x34, 0x12, 0x56], "{mode:?}");
        }
    }

    #[test]
    fn les_couleurs_multiples_sont_des_triplets_grb_consecutifs() {
        // issue #3, tests d'intention — « Un mode multicolore place ses couleurs
        // en triplets GRB consécutifs à partir de l'offset 7 (spec §2, §4) ».
        // spec §4 — « offset 7… : couleurs, triplets GRB consécutifs, autant que
        // le trailer en annonce ».
        let a = Rgb::new(0x11, 0x22, 0x33);
        let b = Rgb::new(0x44, 0x55, 0x66);
        let c = Rgb::new(0x77, 0x88, 0x99);
        let t = trame_de(0x08, Mode::FADING, &[a, b, c]);

        assert_eq!(&t[7..10], &a.to_grb(), "triplet 1 à l'offset 7");
        assert_eq!(&t[10..13], &b.to_grb(), "triplet 2 à l'offset 10");
        assert_eq!(&t[13..16], &c.to_grb(), "triplet 3 à l'offset 13");
        assert_eq!(
            &t[7..16],
            &[0x22, 0x11, 0x33, 0x55, 0x44, 0x66, 0x88, 0x77, 0x99]
        );
        assert_eq!(t[56], 0x03);
    }

    #[test]
    fn l_ordre_des_couleurs_fournies_est_conserve() {
        // contrat d'API — « offset 7… : triplets GRB, luminosité appliquée,
        // dans l'ordre fourni ». `--color` est répétable : l'ordre de la ligne
        // de commande est celui de la trame.
        let a = Rgb::new(0xff, 0x00, 0x00);
        let b = Rgb::new(0x00, 0x00, 0xff);

        let directe = trame_de(0x20, Mode::ALTERNATING, &[a, b]);
        let inverse = trame_de(0x20, Mode::ALTERNATING, &[b, a]);

        assert_eq!(&directe[7..10], &a.to_grb());
        assert_eq!(&directe[10..13], &b.to_grb());
        assert_eq!(&inverse[7..10], &b.to_grb());
        assert_eq!(&inverse[10..13], &a.to_grb());
        assert_ne!(directe, inverse, "l'ordre des couleurs change la trame");
    }

    #[test]
    fn la_luminosite_s_applique_a_toutes_les_couleurs() {
        // issue #3, tests d'intention — « La luminosité s'applique à toutes les
        // couleurs d'un mode multicolore, pas seulement à la première (spec
        // §4.3) ».
        // spec §4.3 — « aucun octet ne porte la luminosité. CAM l'applique côté
        // hôte en multipliant les composantes avant l'envoi ».
        // Composantes divisibles par 2 : aucun arrondi en jeu.
        let couleurs = [
            Rgb::new(200, 100, 50),
            Rgb::new(100, 200, 50),
            Rgb::new(50, 100, 200),
        ];

        let pleine = trame_de(0x08, Mode::FADING, &couleurs);
        assert_eq!(
            &pleine[7..16],
            &[100, 200, 50, 200, 100, 50, 100, 50, 200],
            "GRB à pleine luminosité"
        );

        let moitie = animation(
            0x08,
            Mode::FADING,
            &couleurs,
            Mode::FADING.default_speed(),
            Brightness::new(50),
            LEDS_PER_FAN,
        )
        .expect("fading accepte trois couleurs");
        assert_eq!(
            &moitie[7..16],
            &[50, 100, 25, 100, 50, 25, 50, 25, 100],
            "chacun des trois triplets est divisé par deux"
        );
    }

    #[test]
    fn la_luminosite_ne_touche_que_les_octets_de_couleur() {
        // spec §4.3 — la luminosité n'existe pas dans le protocole : elle ne
        // peut donc modifier que les triplets, jamais un octet de structure.
        let couleurs = [
            Rgb::new(200, 100, 50),
            Rgb::new(100, 200, 50),
            Rgb::new(50, 100, 200),
        ];
        let pleine = trame_de(0x08, Mode::FADING, &couleurs);
        let moitie = animation(
            0x08,
            Mode::FADING,
            &couleurs,
            Mode::FADING.default_speed(),
            Brightness::new(50),
            LEDS_PER_FAN,
        )
        .expect("fading accepte trois couleurs");

        for offset in (0..FRAME_LEN).filter(|o| !(7..16).contains(o)) {
            assert_eq!(
                pleine[offset], moitie[offset],
                "l'offset {offset} ne doit pas dépendre de la luminosité"
            );
        }
    }

    #[test]
    fn la_luminosite_nulle_noircit_toutes_les_couleurs() {
        // spec §4.3 — « luminosité au minimum → CAM envoie `#000000` sur tous
        // les canaux ». La trame reste une trame `2a 04` valide : seuls les
        // triplets sont nuls, l'offset 56 continue d'annoncer les couleurs.
        let t = animation(
            0x08,
            Mode::FADING,
            &[
                Rgb::new(0xff, 0x00, 0x00),
                Rgb::new(0x00, 0xff, 0x00),
                Rgb::new(0x00, 0x00, 0xff),
            ],
            Mode::FADING.default_speed(),
            Brightness::new(0),
            LEDS_PER_FAN,
        )
        .expect("fading accepte trois couleurs");

        assert!(
            t[7..16].iter().all(|&o| o == 0),
            "triplets : {:?}",
            &t[7..16]
        );
        assert_eq!(t[56], 0x03, "le compteur de couleurs reste celui du mode");
        assert_eq!(t[4], 0x01);
        assert_eq!(t[57], 0x08);
    }
}

// ---------------------------------------------------------------------------

mod comptage {
    use super::couleurs;
    use reverb_proto::frame::animation;
    use reverb_proto::{Brightness, ColorCountError, LEDS_PER_FAN, Mode, Rgb};

    /// Tentative d'encodage avec `n` couleurs, à la vitesse par défaut du mode.
    fn essai(mode: Mode, n: u8) -> Result<[u8; reverb_proto::FRAME_LEN], ColorCountError> {
        animation(
            0x01,
            mode,
            &couleurs(n),
            mode.default_speed(),
            Brightness::FULL,
            LEDS_PER_FAN,
        )
    }

    #[test]
    fn le_mode_fixe_exige_exactement_une_couleur() {
        // spec §4.1, ligne `0x00` — 1 couleur.
        // issue #3, critère d'acceptation — « en fournir trop ou trop peu produit
        // une erreur explicite, jamais une trame silencieusement fausse ».
        assert!(essai(Mode::FIXED, 0).is_err(), "zéro couleur");
        assert!(essai(Mode::FIXED, 1).is_ok(), "une couleur");
        assert!(essai(Mode::FIXED, 2).is_err(), "deux couleurs");
    }

    #[test]
    fn le_mode_fading_exige_exactement_trois_couleurs() {
        // spec §4.1, ligne `0x01` — 3 couleurs observées. Contrat d'API,
        // arbitrage : « les bornes reproduisent l'observé, pas la table
        // liquidctl […] à élargir seulement après observation ».
        assert!(essai(Mode::FADING, 2).is_err(), "deux couleurs");
        assert!(essai(Mode::FADING, 3).is_ok(), "trois couleurs");
        assert!(essai(Mode::FADING, 4).is_err(), "quatre couleurs");
    }

    #[test]
    fn le_mode_alternating_exige_exactement_deux_couleurs() {
        // spec §4.1, ligne `0x05` — « exactement 2 » couleurs : « jamais 1,
        // jamais 3 ». C'est le recoupement le plus solide de la table.
        assert!(essai(Mode::ALTERNATING, 1).is_err(), "une couleur");
        assert!(essai(Mode::ALTERNATING, 2).is_ok(), "deux couleurs");
        assert!(essai(Mode::ALTERNATING, 3).is_err(), "trois couleurs");
    }

    #[test]
    fn le_mode_covering_marquee_accepte_deux_ou_trois_couleurs() {
        // spec §4.1, ligne `0x04` — « 2 ou 3 » couleurs observées.
        assert!(essai(Mode::COVERING_MARQUEE, 1).is_err(), "une couleur");
        assert!(essai(Mode::COVERING_MARQUEE, 2).is_ok(), "deux couleurs");
        assert!(essai(Mode::COVERING_MARQUEE, 3).is_ok(), "trois couleurs");
        assert!(essai(Mode::COVERING_MARQUEE, 4).is_err(), "quatre couleurs");
    }

    #[test]
    fn le_mode_spectrum_wave_refuse_toute_couleur() {
        // spec §4.1, ligne `0x02` — « le contrôleur génère les teintes » ;
        // §4.4 — « le mode ignore la couleur fournie ».
        // Contrat d'API, arbitrage : « passer `--color` avec ce mode est une
        // erreur explicite plutôt qu'une couleur silencieusement ignorée par le
        // matériel — c'est le critère "jamais une trame silencieusement
        // fausse" ».
        assert!(essai(Mode::SPECTRUM_WAVE, 0).is_ok(), "aucune couleur");
        assert!(essai(Mode::SPECTRUM_WAVE, 1).is_err(), "une couleur");
        assert!(essai(Mode::SPECTRUM_WAVE, 2).is_err(), "deux couleurs");
    }

    #[test]
    fn les_modes_a_une_couleur_refusent_zero_et_deux() {
        // spec §4.1 — lignes `0x06`, `0x07` et `0x09` : 1 couleur chacune.
        for mode in [Mode::PULSE, Mode::BREATHING, Mode::STARRY_NIGHT] {
            assert!(essai(mode, 0).is_err(), "{mode:?} sans couleur");
            assert!(essai(mode, 1).is_ok(), "{mode:?} avec une couleur");
            assert!(essai(mode, 2).is_err(), "{mode:?} avec deux couleurs");
        }
    }

    #[test]
    fn chaque_mode_accepte_tout_nombre_dans_ses_bornes() {
        // spec §4.1, colonne « Couleurs » — les bornes de la table sont
        // atteignables, sinon la table serait fausse.
        for mode in Mode::ALL {
            let (min, max) = mode.colors();
            assert!(min <= max, "bornes de {mode:?} : ({min}, {max})");
            for nombre in min..=max {
                assert!(
                    essai(mode, nombre).is_ok(),
                    "{mode:?} doit accepter {nombre} couleurs"
                );
            }
        }
    }

    #[test]
    fn chaque_mode_refuse_un_nombre_hors_de_ses_bornes() {
        // issue #3, tests d'intention — « Fournir moins de couleurs que le
        // minimum du mode est rejeté » et « fournir plus que le maximum est
        // rejeté ». La validation vit dans `reverb-proto` : c'est une contrainte
        // du protocole, pas de l'interface.
        for mode in Mode::ALL {
            let (min, max) = mode.colors();
            if min > 0 {
                assert!(
                    essai(mode, min - 1).is_err(),
                    "{mode:?} doit refuser {} couleurs (minimum {min})",
                    min - 1
                );
            }
            assert!(
                essai(mode, max + 1).is_err(),
                "{mode:?} doit refuser {} couleurs (maximum {max})",
                max + 1
            );
        }
    }

    #[test]
    fn le_message_d_erreur_nomme_le_mode_et_l_attendu() {
        // contrat d'API — `ColorCountError` : « Display : mode, attendu, reçu ».
        // issue #3, critère d'acceptation — l'erreur doit être **explicite**.
        let erreur: ColorCountError = essai(Mode::FADING, 1).expect_err("fading exige 3 couleurs");
        let message = erreur.to_string();
        assert!(
            message.contains("fading"),
            "le message doit nommer le mode : « {message} »"
        );
        assert!(
            message.contains('3'),
            "le message doit dire ce qui était attendu : « {message} »"
        );
        assert!(
            message.contains('1'),
            "le message doit dire ce qui a été reçu : « {message} »"
        );
    }

    #[test]
    fn le_message_d_erreur_cite_les_deux_bornes_d_un_intervalle() {
        // spec §4.1, ligne `0x04` — « 2 ou 3 » : les deux bornes sont utiles à
        // l'utilisateur, une seule ne suffit pas à corriger sa commande.
        let erreur: ColorCountError =
            essai(Mode::COVERING_MARQUEE, 4).expect_err("covering-marquee accepte 2 ou 3 couleurs");
        let message = erreur.to_string();
        assert!(
            message.contains("covering-marquee"),
            "le message doit nommer le mode : « {message} »"
        );
        assert!(message.contains('2'), "borne minimale : « {message} »");
        assert!(message.contains('3'), "borne maximale : « {message} »");
        assert!(message.contains('4'), "nombre reçu : « {message} »");
    }

    #[test]
    fn le_message_d_erreur_de_spectrum_wave_le_nomme_aussi() {
        // spec §4.4 — Spectrum Wave n'attend aucune couleur. L'utilisateur qui
        // passe `--color` doit comprendre pourquoi c'est refusé.
        let erreur: ColorCountError =
            essai(Mode::SPECTRUM_WAVE, 1).expect_err("spectrum-wave n'attend aucune couleur");
        let message = erreur.to_string();
        assert!(
            message.contains("spectrum-wave"),
            "le message doit nommer le mode : « {message} »"
        );
        // La formulation n'est pas spécifiée : « 0 » ou « aucune » conviennent,
        // pourvu que le message dise qu'aucune couleur n'est attendue.
        assert!(
            message.contains('0') || message.contains("aucune"),
            "le message doit dire qu'aucune couleur n'est attendue : « {message} »"
        );
    }

    #[test]
    fn une_erreur_de_comptage_ne_produit_aucune_trame() {
        // issue #3, critère d'acceptation — « jamais une trame silencieusement
        // fausse ». Le type de retour est un `Result` : il n'y a pas de trame
        // « par défaut » à récupérer.
        let couleur = Rgb::new(0xff, 0x00, 0xff);
        let resultat = animation(
            0x01,
            Mode::ALTERNATING,
            &[couleur],
            Mode::ALTERNATING.default_speed(),
            Brightness::FULL,
            LEDS_PER_FAN,
        );
        assert!(resultat.is_err());
        assert!(resultat.ok().is_none());
    }
}

// ---------------------------------------------------------------------------

mod non_regression {
    use reverb_proto::frame::{animation, fixed_color};
    use reverb_proto::{Brightness, LEDS_PER_FAN, Mode, Rgb};

    #[test]
    fn la_couleur_fixe_de_l_issue_1_est_reproduite_octet_pour_octet() {
        // issue #3, critère d'acceptation — « Le mode fixe de #1 reste inchangé,
        // octet pour octet (non-régression) ».
        // Contrat d'API — « `fixed_color` reste publique et devient
        // `animation(mask, Mode::FIXED, &[color], 0x32, …)` ».
        for masque in [0x01u8, 0x02, 0x04, 0x08, 0x10, 0x20] {
            for couleur in [
                Rgb::new(0xff, 0x00, 0xff),
                Rgb::new(0x00, 0xff, 0x00),
                Rgb::new(0x12, 0x34, 0x56),
                Rgb::BLACK,
            ] {
                for pourcent in [0u8, 30, 50, 100] {
                    let luminosite = Brightness::new(pourcent);
                    let attendue = fixed_color(masque, couleur, luminosite, LEDS_PER_FAN);
                    let obtenue = animation(
                        masque,
                        Mode::FIXED,
                        &[couleur],
                        0x32,
                        luminosite,
                        LEDS_PER_FAN,
                    )
                    .expect("le mode fixe accepte exactement une couleur");
                    assert_eq!(
                        obtenue, attendue,
                        "masque {masque:#04x}, luminosité {pourcent} %"
                    );
                }
            }
        }
    }

    #[test]
    fn la_vitesse_par_defaut_du_mode_fixe_est_celle_de_l_issue_1() {
        // spec §4.2 — « `0x32` en fixe ». spec §11 — `buf[5] = 0x32`.
        // Contrat d'API, arbitrage : « effet de bord utile : `FIXED` garde
        // `0x32` et la non-régression de #1 est structurelle ».
        assert_eq!(Mode::FIXED.default_speed(), 0x32);

        let attendue = fixed_color(
            0x01,
            Rgb::new(0xff, 0x00, 0xff),
            Brightness::FULL,
            LEDS_PER_FAN,
        );
        let obtenue = animation(
            0x01,
            Mode::FIXED,
            &[Rgb::new(0xff, 0x00, 0xff)],
            Mode::FIXED.default_speed(),
            Brightness::FULL,
            LEDS_PER_FAN,
        )
        .expect("le mode fixe accepte exactement une couleur");
        assert_eq!(obtenue, attendue);
    }

    #[test]
    fn le_mode_fixe_reste_le_mode_par_defaut_de_la_ligne_de_commande() {
        // contrat d'API — « sans `--mode`, le mode reste `fixed` :
        // `reverb set --all --color ff00ff` de #1 est inchangé ».
        // Ici, la seule part testable dans `reverb-proto` : `fixed` se résout
        // bien vers le mode `0x00` de la spec §4.1.
        assert_eq!(Mode::from_name("fixed").ok(), Some(Mode::FIXED));
        assert_eq!(Mode::FIXED.code(), 0x00);
    }
}
