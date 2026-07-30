//! Tests d'intention du pilotage LED par LED (issue #5).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #5, son commentaire
//! « Contrat d'API » et `docs/SPEC-PROTOCOLE-NZXT.md` seuls. Aucun octet n'est
//! relu depuis `src/` : à l'écriture de ce fichier, `Apply`, `LedCountError` et
//! `per_led` n'existent pas encore.
//!
//! Comme pour `spec.rs` (#1) et `spec_modes.rs` (#3) : ces tests encodent ce que
//! le logiciel doit faire, pas ce que le code fait. Si l'un d'eux échoue après
//! implémentation, c'est le code qu'on corrige, jamais le test.
//!
//! ⚠️ **La spec marque toute la trame `22 a0` d'un 🔶 et son offset 12 d'un ❓.**
//! Ces tests figent donc les octets **observés dans la capture**, sans leur
//! prêter de sens. Aucun test n'est nommé d'après une signification supposée :
//! quand la spec avance une hypothèse (offset 3 de `22 10` « index de départ »,
//! offset 12 « vaut 50, comme la vitesse par défaut du mode fixe »), elle est
//! citée en commentaire **comme hypothèse**, jamais transformée en assertion.
//!
//! ⚠️ De même pour `Apply::Animated` : la capture montre ce que **CAM envoie**,
//! pas ce que le contrôleur **fait**. Aucun test n'affirme un comportement
//! visuel — ce que produit `0x02` à l'offset 4 reste à observer à l'œil, et
//! c'est un critère d'acceptation de l'issue, pas un test automatisé.
//!
//! Deux conventions sont reprises de l'existant plutôt que du contrat, qui ne
//! les précise pas : `Apply` et `LedCountError` sont attendus à la racine du
//! crate, comme `Mode` et `ColorCountError` de #3, et le `Frame` du contrat est
//! lu comme le `[u8; FRAME_LEN]` que rendent déjà `fixed_color` et `animation`.
//!
//! Aucun accès matériel, aucune IO (issue #5, critère « Aucun accès matériel
//! dans les tests automatisés »).

use reverb_proto::{Apply, Brightness, FRAME_LEN, Rgb};

/// Paquet commençant par `tete`, complété par des zéros (spec §1 et §11 `_pkt`).
///
/// Les trois trames `0x22` ont toutes leurs octets significatifs **contigus à
/// partir de l'offset 0** : 0..28 pour `22 10`, 0..3 pour `22 11`, 0..16 pour
/// `22 a0`. Le reste est nul — spec §1, « les paquets sont toujours complétés à
/// 64 octets par des zéros ».
fn paquet(tete: &[u8]) -> [u8; FRAME_LEN] {
    let mut t = [0u8; FRAME_LEN];
    t[..tete.len()].copy_from_slice(tete);
    t
}

/// Les huit couleurs de la capture du §5.1, en RGB.
///
/// La spec les donne telles qu'elles partent sur le fil, donc en **GRB** (§2) :
///
/// ```text
/// 22 10 01 00 | 0000ff 00ff00 ff2800 e5ff00 00a9ff ff00b4 50ff00 9f3e2d
///               LED1   LED2   LED3   LED4   LED5   LED6   LED7   LED8
///               bleu   rouge  vert   jaune  rose   cyan   orange olive
/// ```
///
/// Relues en RGB, ce sont les couleurs que l'utilisateur a choisies dans CAM —
/// et donc celles qu'on passe à `per_led`. Le « vert » `ff 28 00` donne bien
/// `#28ff00`, le vert du nuancier CAM relevé au §5.1 et rappelé au §2.
fn couleurs_de_la_capture() -> [Rgb; 8] {
    [
        Rgb::new(0x00, 0x00, 0xff), // LED1 — bleu
        Rgb::new(0xff, 0x00, 0x00), // LED2 — rouge
        Rgb::new(0x28, 0xff, 0x00), // LED3 — vert
        Rgb::new(0xff, 0xe5, 0x00), // LED4 — jaune
        Rgb::new(0xa9, 0x00, 0xff), // LED5 — rose
        Rgb::new(0x00, 0xff, 0xb4), // LED6 — cyan
        Rgb::new(0xff, 0x50, 0x00), // LED7 — orange
        Rgb::new(0x3e, 0x9f, 0x2d), // LED8 — olive
    ]
}

/// `n` couleurs distinctes, sans autre propriété que d'être différentes.
///
/// Sert à faire varier le **nombre** de couleurs fournies sans que la valeur des
/// composantes n'entre en jeu (spec §5.1 : le tampon en porte huit).
fn couleurs(n: usize) -> Vec<Rgb> {
    (0..n).map(|i| Rgb::new(i as u8 + 1, 0x20, 0x30)).collect()
}

/// Les trois trames, à pleine luminosité.
///
/// Contrat d'API — `per_led(mask, colors, apply, brightness)`.
fn trames(masque: u8, couleurs: &[Rgb], apply: Apply) -> [[u8; FRAME_LEN]; 3] {
    reverb_proto::frame::per_led(masque, couleurs, apply, Brightness::FULL)
        .expect("huit couleurs, une par LED (§5.1)")
}

/// Les masques de canal observés, spec §3.
const MASQUES: [u8; 6] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20];

// ---------------------------------------------------------------------------

mod sequence {
    use super::{MASQUES, couleurs, couleurs_de_la_capture, paquet, trames};
    use reverb_proto::{Apply, FRAME_LEN};

    #[test]
    fn les_trois_trames_partent_dans_l_ordre_tampon_validation_application() {
        // spec §0.2 — « `22 10` ne fait qu'alimenter un tampon ; `22 11` puis
        // `22 a0` sont indispensables pour que le contrôleur applique le
        // contenu. La séquence complète du §5.2 est obligatoire. »
        // spec §5.2 — « les trois trames partent toujours groupées ».
        // issue #5, critère d'acceptation — « Les trois trames `22 10`, `22 11`,
        // `22 a0` partent **groupées et dans cet ordre** ».
        // Contrat d'API — le retour est un `[Frame; 3]`, « elles sont dans
        // l'ordre d'émission : tampon, validation, application ».
        let t = trames(0x01, &couleurs(8), Apply::Static);

        assert_eq!(t.len(), 3, "trois trames, ni une de plus ni une de moins");
        assert_eq!(&t[0][0..2], &[0x22, 0x10], "trame 0 — tampon");
        assert_eq!(&t[1][0..2], &[0x22, 0x11], "trame 1 — validation du canal");
        assert_eq!(&t[2][0..2], &[0x22, 0xa0], "trame 2 — application");
    }

    #[test]
    fn la_sequence_de_la_capture_est_reproduite_octet_pour_octet() {
        // spec §5.2, séquence complète relevée dans la capture, canal `0x01` :
        //
        //     22 10 01 00 <24 octets de couleurs>              <- tampon
        //     22 11 01                                         <- validation
        //     22 a0 01 00 01 00 00 08 00 00 80 00 32 00 00 01  <- application
        //
        // Les 24 octets de couleurs sont ceux du §5.1, recopiés tels quels.
        // spec §11 — l'implémentation de référence émet exactement ces trois
        // paquets pour huit LED.
        let attendues = [
            paquet(&[
                0x22, 0x10, 0x01, 0x00, //
                0x00, 0x00, 0xff, // LED1
                0x00, 0xff, 0x00, // LED2
                0xff, 0x28, 0x00, // LED3
                0xe5, 0xff, 0x00, // LED4
                0x00, 0xa9, 0xff, // LED5
                0xff, 0x00, 0xb4, // LED6
                0x50, 0xff, 0x00, // LED7
                0x9f, 0x3e, 0x2d, // LED8
            ]),
            paquet(&[0x22, 0x11, 0x01]),
            paquet(&[
                0x22, 0xa0, 0x01, 0x00, 0x01, 0x00, 0x00, 0x08, //
                0x00, 0x00, 0x80, 0x00, 0x32, 0x00, 0x00, 0x01,
            ]),
        ];

        let obtenues = trames(0x01, &couleurs_de_la_capture(), Apply::Static);
        assert_eq!(obtenues, attendues);
    }

    #[test]
    fn les_trois_trames_font_exactement_64_octets() {
        // spec §1 — « HID interrupt, paquets de 64 octets […] toujours complétés
        // à 64 octets par des zéros ». spec §0 — `OutputReportByteLength` vaut
        // **64 et non 65** : pas d'octet `0x00` préfixé.
        assert_eq!(FRAME_LEN, 64);
        for apply in [Apply::Static, Apply::Animated { speed: 0x006a }] {
            for trame in trames(0x01, &couleurs(8), apply) {
                assert_eq!(trame.len(), 64);
            }
        }
    }

    #[test]
    fn aucune_des_trois_trames_n_appartient_a_la_famille_2a_04() {
        // spec §5 — la famille `0x22` « est distincte de `0x2a 0x04` et
        // n'apparaît que lorsque CAM peint des LED individuellement ».
        // issue #5, critère d'acceptation — « La famille `0x2a 0x04` est
        // inchangée ».
        for trame in trames(0x01, &couleurs(8), Apply::Static) {
            assert_eq!(trame[0], 0x22, "octet de commande");
            assert_ne!(trame[0], 0x2a);
        }
    }

    #[test]
    fn le_masque_est_repris_a_l_identique_dans_les_trois_trames() {
        // spec §5.1 — « offset 2 = masque de canal, même encodage qu'au §3 » ;
        // spec §5.2 — les trois trames de la capture portent le même `01`.
        // spec §3 — « un bit par canal », et « CAM n'agrège jamais plusieurs
        // canaux dans une trame ».
        // Contrat d'API — l'offset 2 des trois trames est « masque de canal ».
        for masque in MASQUES {
            let t = trames(masque, &couleurs(8), Apply::Static);
            assert_eq!(t[0][2], masque, "22 10, masque {masque:#04x}");
            assert_eq!(t[1][2], masque, "22 11, masque {masque:#04x}");
            assert_eq!(t[2][2], masque, "22 a0, masque {masque:#04x}");
        }
    }

    #[test]
    fn les_couleurs_ne_modifient_que_la_trame_tampon() {
        // spec §5.2 — seule `22 10` porte les 24 octets de couleurs ; `22 11` ne
        // porte que le masque et `22 a0` n'a aucun octet de couleur.
        let a = trames(0x01, &couleurs_de_la_capture(), Apply::Static);
        let b = trames(0x01, &couleurs(8), Apply::Static);

        assert_ne!(a[0], b[0], "le tampon dépend des couleurs");
        assert_eq!(a[1], b[1], "la validation ne dépend pas des couleurs");
        assert_eq!(a[2], b[2], "l'application ne dépend pas des couleurs");
    }

    #[test]
    fn le_mode_d_application_ne_modifie_que_la_trame_d_application() {
        // spec §5.2 — statique et animé ne diffèrent que par les offsets 4 à 6
        // de `22 a0`. Le tampon et la validation sont les mêmes.
        let couleurs = couleurs_de_la_capture();
        let statique = trames(0x01, &couleurs, Apply::Static);
        let anime = trames(0x01, &couleurs, Apply::Animated { speed: 0x006a });

        assert_eq!(statique[0], anime[0], "le tampon ne dépend pas du mode");
        assert_eq!(statique[1], anime[1], "la validation non plus");
        assert_ne!(statique[2], anime[2], "l'application, si");
    }
}

// ---------------------------------------------------------------------------

mod tampon {
    use super::{MASQUES, couleurs, paquet, trames};
    use reverb_proto::{Apply, FRAME_LEN, Rgb};

    /// La trame `22 10` seule.
    fn trame(masque: u8, couleurs: &[Rgb]) -> [u8; FRAME_LEN] {
        trames(masque, couleurs, Apply::Static)[0]
    }

    #[test]
    fn la_trame_tampon_est_exacte_octet_pour_octet() {
        // spec §5.1 — `22 10 <masque> <index> | 8 triplets GRB | padding`.
        // Contrat d'API, trame 0 : offsets 0–1 `22 10`, 2 le masque, 3 `0x00`,
        // 4…27 les huit triplets GRB, 28…63 des zéros.
        // spec §2 — rouge pur `00 ff 00`, vert pur `ff 00 00`, bleu `00 00 ff`.
        // Canal `0x08` = « radiateur haut » (§3).
        let attendue = paquet(&[
            0x22, 0x10, 0x08, 0x00, //
            0x00, 0xff, 0x00, // LED1 — rouge
            0xff, 0x00, 0x00, // LED2 — vert
            0x00, 0x00, 0xff, // LED3 — bleu
            0xff, 0xff, 0x00, // LED4 — jaune
            0x00, 0xff, 0xff, // LED5 — magenta
            0xff, 0x00, 0xff, // LED6 — cyan
            0xff, 0xff, 0xff, // LED7 — blanc
            0x00, 0x00, 0x00, // LED8 — noir
        ]);

        let obtenue = trame(
            0x08,
            &[
                Rgb::new(0xff, 0x00, 0x00),
                Rgb::new(0x00, 0xff, 0x00),
                Rgb::new(0x00, 0x00, 0xff),
                Rgb::new(0xff, 0xff, 0x00),
                Rgb::new(0xff, 0x00, 0xff),
                Rgb::new(0x00, 0xff, 0xff),
                Rgb::new(0xff, 0xff, 0xff),
                Rgb::new(0x00, 0x00, 0x00),
            ],
        );
        assert_eq!(obtenue, attendue);
    }

    #[test]
    fn les_offsets_0_et_1_valent_22_10() {
        // spec §5.1 — la commande d'écriture du tampon.
        for masque in MASQUES {
            let t = trame(masque, &couleurs(8));
            assert_eq!(t[0], 0x22, "masque {masque:#04x}");
            assert_eq!(t[1], 0x10, "masque {masque:#04x}");
        }
    }

    #[test]
    fn l_offset_2_porte_le_masque_de_canal() {
        // spec §5.1 — « offset 2 = masque de canal, même encodage qu'au §3 ✅ ».
        for masque in MASQUES {
            assert_eq!(trame(masque, &couleurs(8))[2], masque);
        }
    }

    #[test]
    fn l_offset_3_vaut_zero() {
        // spec §5.1 — « offset 3 = `0x00` » ✅ observé dans toute la capture.
        // 🔶 La spec y voit un « index de départ, permettant de chaîner plusieurs
        // paquets pour les accessoires de plus de 8 LED », et le dit
        // « **non vérifiable ici** : tous les accessoires en font exactement 8 »
        // (§10, question 1). Cette lecture reste une **hypothèse** : le seul fait
        // testable est que l'octet vaut zéro.
        for masque in MASQUES {
            assert_eq!(trame(masque, &couleurs(8))[3], 0x00);
        }
    }

    #[test]
    fn les_huit_triplets_occupent_les_offsets_4_a_27() {
        // spec §5.1 — « 8 triplets GRB, 24 octets » après `22 10 <masque> <index>`.
        // Contrat d'API, trame 0 : « 4…27 : 8 triplets GRB ».
        let c = couleurs(8);
        let t = trame(0x01, &c);
        for (i, couleur) in c.iter().enumerate() {
            let debut = 4 + 3 * i;
            assert_eq!(
                &t[debut..debut + 3],
                &couleur.to_grb(),
                "LED {} à l'offset {debut}",
                i + 1
            );
        }
    }

    #[test]
    fn les_octets_28_a_63_sont_nuls() {
        // spec §5.1 — « padding jusqu'a 64 » après les 24 octets de couleurs.
        // spec §1 — « les paquets sont toujours complétés à 64 octets par des
        // zéros ».
        let t = trame(0x01, &[Rgb::new(0xff, 0xff, 0xff); 8]);
        assert!(
            t[28..FRAME_LEN].iter().all(|&o| o == 0),
            "offsets 28..64 : {:?}",
            &t[28..FRAME_LEN]
        );
    }
}

// ---------------------------------------------------------------------------

mod validation {
    use super::{MASQUES, couleurs, paquet, trames};
    use reverb_proto::{Apply, FRAME_LEN};

    #[test]
    fn la_trame_de_validation_est_exacte_octet_pour_octet() {
        // spec §5.2 — « 22 11 01 <- validation du canal », rien d'autre.
        // Contrat d'API, trame 1 : offsets 0–1 `22 11`, 2 le masque, 3…63 zéros.
        let attendue = paquet(&[0x22, 0x11, 0x01]);
        let obtenue = trames(0x01, &couleurs(8), Apply::Static)[1];
        assert_eq!(obtenue, attendue);
    }

    #[test]
    fn la_validation_ne_porte_que_la_commande_et_le_masque() {
        // spec §5.2 — la trame observée s'arrête au masque ; le reste est du
        // remplissage (§1). Contrat d'API : « 3…63 : zéros ».
        for masque in MASQUES {
            let t = trames(masque, &couleurs(8), Apply::Animated { speed: 0x1234 })[1];
            assert_eq!(&t[0..3], &[0x22, 0x11, masque]);
            assert!(
                t[3..FRAME_LEN].iter().all(|&o| o == 0),
                "masque {masque:#04x}, offsets 3..64 : {:?}",
                &t[3..FRAME_LEN]
            );
        }
    }
}

// ---------------------------------------------------------------------------

mod application {
    use super::{MASQUES, couleurs, paquet, trames};
    use reverb_proto::{Apply, FRAME_LEN, LEDS_PER_FAN};

    /// La trame `22 a0` seule.
    fn trame(masque: u8, apply: Apply) -> [u8; FRAME_LEN] {
        trames(masque, &couleurs(8), apply)[2]
    }

    #[test]
    fn la_trame_d_application_statique_est_exacte_octet_pour_octet() {
        // spec §5.2 — trame relevée dans la capture, canal `0x01` :
        //     22 a0 01 00 01 00 00 08 00 00 80 00 32 00 00 01
        // spec §11 — l'implémentation de référence réémet ces mêmes octets.
        // Contrat d'API, trame 2 : `0x01` à l'offset 4, `00 00` aux offsets 5–6,
        // `0x08` à l'offset 7, puis les huit octets 8…15 verbatim.
        let attendue = paquet(&[
            0x22, 0xa0, 0x01, 0x00, 0x01, 0x00, 0x00, 0x08, //
            0x00, 0x00, 0x80, 0x00, 0x32, 0x00, 0x00, 0x01,
        ]);
        assert_eq!(trame(0x01, Apply::Static), attendue);
    }

    #[test]
    fn la_trame_d_application_animee_est_exacte_octet_pour_octet() {
        // spec §5.2 — trame **attestée** dans la capture au même titre que la
        // statique :
        //     22 a0 01 00 02 6a 00 08 00 00 80 00 32 00 00 01
        // « Les octets 8 à 15 sont identiques dans les deux modes […] c'est
        // acquis, pas déduit. » Seuls les offsets 4, 5 et 6 changent.
        // ⚠️ Ce test fige ce que **CAM envoie**. Ce que le contrôleur en fait
        // reste ❓ (issue #5 : « c'est une inconnue, pas un acquis »).
        // Canal `0x04` = « bas droite » (§3).
        let attendue = paquet(&[
            0x22, 0xa0, 0x04, 0x00, 0x02, 0x6a, 0x00, 0x08, //
            0x00, 0x00, 0x80, 0x00, 0x32, 0x00, 0x00, 0x01,
        ]);
        assert_eq!(
            trame(0x04, Apply::Animated { speed: 0x006a }),
            attendue,
            "seule valeur de vitesse observée dans la capture"
        );
    }

    #[test]
    fn les_offsets_0_et_1_valent_22_a0() {
        // spec §5.2 — la commande d'application.
        for apply in [Apply::Static, Apply::Animated { speed: 0x006a }] {
            let t = trame(0x01, apply);
            assert_eq!(t[0], 0x22, "{apply:?}");
            assert_eq!(t[1], 0xa0, "{apply:?}");
        }
    }

    #[test]
    fn l_offset_2_porte_le_masque_et_l_offset_3_vaut_zero() {
        // spec §5.2, table — « offset 2 : `0x01`, masque de canal ». L'offset 3
        // n'y figure pas ; la trame de la capture le montre à `0x00`, et le
        // contrat d'API le fige à `0x00`. Aucune interprétation n'y est attachée.
        for masque in MASQUES {
            for apply in [Apply::Static, Apply::Animated { speed: 0x006a }] {
                let t = trame(masque, apply);
                assert_eq!(t[2], masque, "{apply:?}, masque {masque:#04x}");
                assert_eq!(t[3], 0x00, "{apply:?}, masque {masque:#04x}");
            }
        }
    }

    #[test]
    fn l_offset_4_vaut_01_en_statique_et_02_en_anime() {
        // spec §5.2, table — « offset 4 : `0x01` statique, `0x02` animé, mode ».
        // issue #5, critère d'acceptation — « `--animate` bascule l'offset 4 de
        // `22 a0` à `0x02` ».
        assert_eq!(trame(0x01, Apply::Static)[4], 0x01);
        for vitesse in [0x0000, 0x006a, 0x1234, 0xffff] {
            assert_eq!(
                trame(0x01, Apply::Animated { speed: vitesse })[4],
                0x02,
                "vitesse {vitesse:#06x}"
            );
        }
    }

    #[test]
    fn la_vitesse_est_ecrite_en_uint16_little_endian_aux_offsets_5_et_6() {
        // spec §5.2, table — « offsets 5–6 : vitesse, uint16 little-endian ».
        // issue #5, critère d'acceptation — « écrit la vitesse en **uint16
        // little-endian** aux offsets 5–6 ».
        // Les valeurs choisies ont leurs deux octets différents : une écriture
        // big-endian les échangerait visiblement.
        for vitesse in [0x1234u16, 0x0100, 0x00ff, 0xff00, 0xabcd, 0xffff, 0x0000] {
            let t = trame(0x01, Apply::Animated { speed: vitesse });
            assert_eq!(
                &t[5..7],
                &vitesse.to_le_bytes(),
                "vitesse {vitesse:#06x} en little-endian"
            );
        }

        // Le cas le plus parlant, écrit à la main plutôt que dérivé.
        let t = trame(0x01, Apply::Animated { speed: 0x1234 });
        assert_eq!(t[5], 0x34, "octet de poids faible d'abord");
        assert_eq!(t[6], 0x12, "octet de poids fort ensuite");
    }

    #[test]
    fn la_vitesse_observee_part_en_6a_00() {
        // spec §5.2, table — « `6a 00` animé ». Contrat d'API —
        // `Apply::OBSERVED_SPEED`, « seule valeur vue dans la capture ».
        // issue #5 — « `0x006a` par défaut (seule valeur observée) ».
        assert_eq!(Apply::OBSERVED_SPEED, 0x006a);

        let t = trame(
            0x01,
            Apply::Animated {
                speed: Apply::OBSERVED_SPEED,
            },
        );
        assert_eq!(&t[5..7], &[0x6a, 0x00]);
    }

    #[test]
    fn en_statique_les_offsets_5_et_6_sont_nuls() {
        // spec §5.2, table — « `00 00` statique ». Contrat d'API, arbitrage
        // « la vitesse est portée par `Apply::Animated` » : « elle n'a aucun sens
        // en statique — la capture y montre `00 00` ».
        for masque in MASQUES {
            let t = trame(masque, Apply::Static);
            assert_eq!(&t[5..7], &[0x00, 0x00], "masque {masque:#04x}");
        }
    }

    #[test]
    fn l_offset_7_vaut_08_comme_le_nombre_de_led_de_l_accessoire() {
        // spec §5.2, table — « offset 7 : `0x08`, nombre de LED ».
        // issue #5, approche technique — « le nombre de LED attendu vient de
        // l'accessoire (`LEDS_PER_FAN`), pas d'une constante en dur ».
        for apply in [Apply::Static, Apply::Animated { speed: 0x006a }] {
            let t = trame(0x01, apply);
            assert_eq!(t[7], 0x08, "{apply:?}");
            assert_eq!(t[7], LEDS_PER_FAN, "{apply:?}");
        }
    }

    #[test]
    fn les_octets_8_a_15_sont_reproduits_tels_quels() {
        // spec §5.2 — la trame observée porte `00 00 80 00 32 00 00 01` à partir
        // de l'offset 8, et la section entière est marquée 🔶.
        // Contrat d'API, trame 2 : « 8…15 : reproduits **verbatim**, sens
        // inconnu ». issue #5, critère d'acceptation — « les octets 7 à 15 de
        // `22 a0` sont reproduits **verbatim** et documentés comme inconnus,
        // même politique que l'offset 57 (#3) ».
        //
        // 🔶❓ La spec avance une lecture pour l'offset 12 — « `0x32` : vaut 50,
        // comme la vitesse par défaut du mode fixe » — mais la marque ❓. C'est
        // une **hypothèse**, pas un acquis : ce test ne vérifie que la valeur.
        // Rien ici n'est nommé d'après une signification supposée.
        for apply in [Apply::Static, Apply::Animated { speed: 0x1234 }] {
            let t = trame(0x01, apply);
            assert_eq!(
                &t[8..16],
                &[0x00, 0x00, 0x80, 0x00, 0x32, 0x00, 0x00, 0x01],
                "{apply:?}"
            );
        }
    }

    #[test]
    fn les_octets_16_a_63_sont_nuls() {
        // spec §5.2 — la trame observée s'arrête à l'offset 15 ; spec §1 — le
        // reste est du remplissage à zéro. Contrat d'API : « 16…63 : zéros ».
        for apply in [Apply::Static, Apply::Animated { speed: 0xffff }] {
            let t = trame(0x01, apply);
            assert!(
                t[16..FRAME_LEN].iter().all(|&o| o == 0),
                "{apply:?}, offsets 16..64 : {:?}",
                &t[16..FRAME_LEN]
            );
        }
    }

    #[test]
    fn la_vitesse_ne_modifie_que_les_offsets_5_et_6() {
        // spec §5.2, table — la vitesse occupe les offsets 5–6, et rien d'autre
        // dans la trame ne dépend d'elle. Même exigence qu'en #3 pour l'octet 5
        // de `2a 04` (§4.2).
        let lente = trames(0x01, &couleurs(8), Apply::Animated { speed: 0x0001 });
        let rapide = trames(0x01, &couleurs(8), Apply::Animated { speed: 0xfedc });

        assert_eq!(lente[0], rapide[0], "le tampon ne dépend pas de la vitesse");
        assert_eq!(lente[1], rapide[1], "la validation non plus");
        for offset in (0..FRAME_LEN).filter(|o| !(5..7).contains(o)) {
            assert_eq!(
                lente[2][offset], rapide[2][offset],
                "l'offset {offset} de `22 a0` ne doit pas dépendre de la vitesse"
            );
        }
    }
}

// ---------------------------------------------------------------------------

mod couleurs_des_led {
    use super::{couleurs_de_la_capture, trames};
    use reverb_proto::frame::per_led;
    use reverb_proto::{Apply, Brightness, FRAME_LEN, Rgb};

    /// Le tampon `22 10`, à pleine luminosité.
    fn tampon(couleurs: &[Rgb]) -> [u8; FRAME_LEN] {
        trames(0x01, couleurs, Apply::Static)[0]
    }

    /// Les huit LED de la même couleur.
    fn uniformes(couleur: Rgb) -> [Rgb; 8] {
        [couleur; 8]
    }

    #[test]
    fn les_couleurs_partent_en_grb() {
        // spec §2 — « les couleurs partent dans l'ordre G, R, B. Le rouge pur
        // part en `00 ff 00` et le **vert pur en `ff 00 00`** ». Une lecture RGB
        // naïve intervertit rouge et vert sur toute l'installation.
        // issue #5, critère d'acceptation — « L'ordre **GRB** est respecté (§2) ».
        assert_eq!(
            &tampon(&uniformes(Rgb::new(0xff, 0x00, 0x00)))[4..7],
            &[0x00, 0xff, 0x00],
            "rouge pur"
        );
        assert_eq!(
            &tampon(&uniformes(Rgb::new(0x00, 0xff, 0x00)))[4..7],
            &[0xff, 0x00, 0x00],
            "vert pur"
        );
        assert_eq!(
            &tampon(&uniformes(Rgb::new(0x00, 0x00, 0xff)))[4..7],
            &[0x00, 0x00, 0xff],
            "bleu pur"
        );

        // Une couleur dont les trois composantes diffèrent : toute permutation
        // autre que GRB donnerait un triplet différent.
        let couleur = Rgb::new(0x12, 0x34, 0x56);
        let t = tampon(&uniformes(couleur));
        assert_eq!(&t[4..7], &couleur.to_grb());
        assert_eq!(&t[4..7], &[0x34, 0x12, 0x56]);
    }

    #[test]
    fn la_conversion_grb_vaut_pour_les_huit_led_pas_seulement_la_premiere() {
        // spec §2 — l'ordre vaut pour toute la trame ; §5.1 le montre sur les
        // huit triplets d'un même ventilateur.
        let c = couleurs_de_la_capture();
        let t = tampon(&c);
        for (i, couleur) in c.iter().enumerate() {
            let debut = 4 + 3 * i;
            assert_eq!(
                &t[debut..debut + 3],
                &couleur.to_grb(),
                "LED {} à l'offset {debut}",
                i + 1
            );
        }
    }

    #[test]
    fn la_premiere_led_est_a_l_offset_4_et_la_huitieme_a_l_offset_25() {
        // spec §5.1 — la capture annote les triplets `LED1 … LED8` dans l'ordre,
        // à partir de l'offset 4. Le huitième triplet commence donc à
        // 4 + 3 × 7 = 25 et finit à 27.
        // issue #5 — « Huit couleurs, une par LED, dans l'ordre physique ».
        let premiere = Rgb::new(0xff, 0x00, 0x00);
        let derniere = Rgb::new(0x00, 0x00, 0xff);
        let mut c = [Rgb::new(0x00, 0x00, 0x00); 8];
        c[0] = premiere;
        c[7] = derniere;

        let t = tampon(&c);
        assert_eq!(&t[4..7], &premiere.to_grb(), "LED 1");
        assert_eq!(&t[25..28], &derniere.to_grb(), "LED 8");
        assert!(
            t[7..25].iter().all(|&o| o == 0),
            "les six LED intermédiaires sont noires : {:?}",
            &t[7..25]
        );
    }

    #[test]
    fn l_ordre_des_huit_couleurs_est_conserve() {
        // issue #5, critère d'acceptation — « Les 8 LED d'un ventilateur prennent
        // les couleurs données, **dans l'ordre** ».
        // Contrat d'API, trame 0 — « 8 triplets GRB […] dans l'ordre fourni ».
        let directe = couleurs_de_la_capture();
        let mut inverse = directe;
        inverse.reverse();

        let a = tampon(&directe);
        let b = tampon(&inverse);
        assert_ne!(a, b, "l'ordre des couleurs change la trame");
        for i in 0..8 {
            let debut = 4 + 3 * i;
            assert_eq!(&a[debut..debut + 3], &directe[i].to_grb());
            assert_eq!(&b[debut..debut + 3], &inverse[i].to_grb());
        }
    }

    #[test]
    fn la_luminosite_s_applique_aux_huit_couleurs() {
        // spec §4.3 — « aucun octet ne porte la luminosité. CAM l'applique
        // **côté hôte** en multipliant les composantes avant l'envoi. […] Sous
        // Linux, la mise à l'échelle est donc à faire soi-même. »
        // issue #5, critère d'acceptation — « la luminosité appliquée côté hôte
        // (§4.3) ». Contrat d'API, trame 0 — « luminosité appliquée ».
        // Composantes toutes paires : aucun arrondi en jeu.
        let c = [
            Rgb::new(200, 100, 50),
            Rgb::new(100, 200, 50),
            Rgb::new(50, 100, 200),
            Rgb::new(60, 80, 100),
            Rgb::new(10, 20, 30),
            Rgb::new(254, 128, 64),
            Rgb::new(0, 0, 0),
            Rgb::new(2, 4, 6),
        ];

        let pleine = tampon(&c);
        assert_eq!(
            &pleine[4..28],
            &[
                100, 200, 50, // GRB
                200, 100, 50, //
                100, 50, 200, //
                80, 60, 100, //
                20, 10, 30, //
                128, 254, 64, //
                0, 0, 0, //
                4, 2, 6,
            ],
            "GRB à pleine luminosité"
        );

        let moitie = per_led(0x01, &c, Apply::Static, Brightness::new(50))
            .expect("huit couleurs, une par LED (§5.1)")[0];
        assert_eq!(
            &moitie[4..28],
            &[
                50, 100, 25, //
                100, 50, 25, //
                50, 25, 100, //
                40, 30, 50, //
                10, 5, 15, //
                64, 127, 32, //
                0, 0, 0, //
                2, 1, 3,
            ],
            "les huit triplets sont divisés par deux, pas seulement le premier"
        );
    }

    #[test]
    fn la_luminosite_ne_touche_que_les_octets_de_couleur() {
        // spec §4.3 — la luminosité n'existe pas dans le protocole : elle ne peut
        // donc modifier que les triplets du tampon, jamais un octet de structure,
        // et surtout aucune des deux autres trames.
        let c = couleurs_de_la_capture();
        let pleine = trames(0x01, &c, Apply::Static);
        let moitie = per_led(0x01, &c, Apply::Static, Brightness::new(50))
            .expect("huit couleurs, une par LED (§5.1)");

        for offset in (0..FRAME_LEN).filter(|o| !(4..28).contains(o)) {
            assert_eq!(
                pleine[0][offset], moitie[0][offset],
                "l'offset {offset} du tampon ne doit pas dépendre de la luminosité"
            );
        }
        assert_eq!(pleine[1], moitie[1], "la validation est inchangée");
        assert_eq!(pleine[2], moitie[2], "l'application est inchangée");
    }

    #[test]
    fn la_luminosite_nulle_noircit_les_huit_led() {
        // spec §4.3 — « luminosité au minimum → CAM envoie `#000000` sur tous les
        // canaux ». La séquence reste une séquence `0x22` valide : seuls les
        // triplets sont nuls.
        let trames = per_led(
            0x01,
            &couleurs_de_la_capture(),
            Apply::Static,
            Brightness::new(0),
        )
        .expect("huit couleurs, une par LED (§5.1)");

        assert!(
            trames[0][4..28].iter().all(|&o| o == 0),
            "triplets : {:?}",
            &trames[0][4..28]
        );
        assert_eq!(&trames[0][0..4], &[0x22, 0x10, 0x01, 0x00]);
        assert_eq!(&trames[1][0..3], &[0x22, 0x11, 0x01]);
        assert_eq!(
            &trames[2][0..8],
            &[0x22, 0xa0, 0x01, 0x00, 0x01, 0x00, 0x00, 0x08]
        );
    }
}

// ---------------------------------------------------------------------------

mod comptage {
    use super::couleurs;
    use reverb_proto::frame::per_led;
    use reverb_proto::{Apply, Brightness, FRAME_LEN, LEDS_PER_FAN, LedCountError};

    /// Tentative d'encodage avec `n` couleurs.
    fn essai(n: usize) -> Result<[[u8; FRAME_LEN]; 3], LedCountError> {
        per_led(0x01, &couleurs(n), Apply::Static, Brightness::FULL)
    }

    #[test]
    fn exactement_huit_couleurs_sont_acceptees() {
        // spec §5.1 — le tampon porte « 8 triplets GRB, 24 octets ».
        // Contrat d'API, arbitrage « exactement 8 couleurs » : « une seule trame
        // `22 10` porte 8 triplets […] 8 est la seule taille que le code sait
        // produire honnêtement ».
        assert!(essai(8).is_ok(), "huit couleurs, une par LED");
    }

    #[test]
    fn le_nombre_attendu_est_celui_de_l_accessoire() {
        // spec §5.1 — « tous les accessoires en font exactement 8 » ; §4, offset
        // 58 — « `0x08`, nombre de LED de l'accessoire ».
        // issue #5, approche technique — « le nombre de LED attendu vient de
        // l'accessoire (`LEDS_PER_FAN`), pas d'une constante en dur ».
        assert_eq!(LEDS_PER_FAN, 8);
        assert!(essai(LEDS_PER_FAN as usize).is_ok());
    }

    #[test]
    fn moins_de_huit_couleurs_est_refuse() {
        // issue #5, critère d'acceptation — « Un nombre de couleurs différent de
        // 8 produit une erreur explicite […] jamais une trame silencieusement
        // fausse ».
        // Contrat d'API — « accepter 3 couleurs en les répétant inventerait un
        // comportement ; en fournir 2 ou 12 est une erreur explicite ». Aucune
        // valeur par défaut, aucune répétition, aucun complément noir.
        for n in [0, 1, 7] {
            assert!(essai(n).is_err(), "{n} couleurs doit être refusé");
        }
    }

    #[test]
    fn plus_de_huit_couleurs_est_refuse() {
        // issue #5, même critère. Le chaînage par l'offset 3 de `22 10` est 🔶 et
        // « non vérifiable ici » (§5.1, §10 question 1) : rien ne permet d'écrire
        // une séquence de plus de 8 LED.
        for n in [9, 16] {
            assert!(essai(n).is_err(), "{n} couleurs doit être refusé");
        }
    }

    #[test]
    fn le_message_d_erreur_dit_l_attendu_et_le_recu() {
        // issue #5, critère d'acceptation — « une erreur explicite disant
        // combien étaient attendues ». Même exigence qu'en #3 pour
        // `ColorCountError`.
        for n in [0usize, 1, 7, 9, 16] {
            let erreur: LedCountError = essai(n).expect_err("seules 8 couleurs sont acceptées");
            let message = erreur.to_string();
            assert!(
                message.contains('8'),
                "le message doit dire que 8 couleurs sont attendues : « {message} »"
            );
            assert!(
                message.contains(&n.to_string()),
                "le message doit dire que {n} ont été reçues : « {message} »"
            );
        }
    }

    #[test]
    fn l_erreur_porte_le_nombre_recu() {
        // Contrat d'API — `pub struct LedCountError { pub given: usize }`.
        for n in [0usize, 1, 7, 9, 16] {
            let erreur: LedCountError = essai(n).expect_err("seules 8 couleurs sont acceptées");
            assert_eq!(erreur.given, n);
        }
    }

    #[test]
    fn une_erreur_de_comptage_ne_produit_aucune_trame() {
        // issue #5, critère d'acceptation — « jamais une trame silencieusement
        // fausse ». Le retour est un `Result` : il n'y a pas de séquence
        // « par défaut » à récupérer.
        // issue #5, approche technique — « le nombre de couleurs est validé par
        // `reverb-proto` avant d'ouvrir le moindre `/dev/hidraw*` ».
        let resultat = essai(3);
        assert!(resultat.is_err());
        assert!(resultat.ok().is_none());
    }

    #[test]
    fn le_comptage_ne_depend_ni_du_masque_ni_du_mode_ni_de_la_luminosite() {
        // spec §5.1 — la taille du tampon est une propriété de la trame, pas du
        // canal visé ni de la façon dont elle sera appliquée.
        for masque in super::MASQUES {
            for apply in [Apply::Static, Apply::Animated { speed: 0x006a }] {
                for luminosite in [Brightness::FULL, Brightness::new(0)] {
                    assert!(
                        per_led(masque, &couleurs(7), apply, luminosite).is_err(),
                        "7 couleurs, masque {masque:#04x}, {apply:?}"
                    );
                    assert!(
                        per_led(masque, &couleurs(8), apply, luminosite).is_ok(),
                        "8 couleurs, masque {masque:#04x}, {apply:?}"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------

mod mode_d_application {
    use reverb_proto::Apply;

    #[test]
    fn la_vitesse_observee_vaut_0x006a() {
        // spec §5.2, table — « `6a 00` animé », lu en little-endian.
        // Contrat d'API — « `pub const OBSERVED_SPEED: u16 = 0x006a` : vitesse
        // par défaut en animé, seule valeur vue dans la capture ».
        assert_eq!(Apply::OBSERVED_SPEED, 0x006a);
        assert_eq!(Apply::OBSERVED_SPEED.to_le_bytes(), [0x6a, 0x00]);
    }

    #[test]
    fn la_vitesse_n_existe_qu_en_anime() {
        // Contrat d'API, arbitrage « la vitesse est portée par
        // `Apply::Animated` » : « elle n'a aucun sens en statique — la capture y
        // montre `00 00`. Un `Option<u16>` à côté d'un booléen permettrait
        // d'écrire "statique à la vitesse 106", qui ne veut rien dire. »
        // Ce test vérifie la forme du type : `Static` ne porte pas de vitesse,
        // `Animated` en porte une, et c'est un `u16`.
        let anime = Apply::Animated { speed: 0x1234 };
        let Apply::Animated { speed } = anime else {
            panic!("la variante animée porte une vitesse");
        };
        let _: u16 = speed;
        assert_eq!(speed, 0x1234);
    }

    #[test]
    fn statique_et_anime_sont_deux_valeurs_distinctes() {
        // Contrat d'API — `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`.
        // Deux vitesses différentes décrivent deux applications différentes :
        // la spec les écrit à des offsets porteurs (§5.2).
        assert_ne!(
            Apply::Static,
            Apply::Animated {
                speed: Apply::OBSERVED_SPEED
            }
        );
        assert_ne!(
            Apply::Animated { speed: 0x0001 },
            Apply::Animated { speed: 0x0002 }
        );
        assert_eq!(
            Apply::Animated {
                speed: Apply::OBSERVED_SPEED
            },
            Apply::Animated { speed: 0x006a }
        );
        assert_eq!(Apply::Static, Apply::Static);
    }
}
