//! Tests d'intention du socle « couleur fixe » (issue #1).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #1 et
//! `docs/SPEC-PROTOCOLE-NZXT.md` seules. Ils encodent ce que le logiciel doit
//! faire, pas ce que le code fait : si l'un d'eux échoue après implémentation,
//! c'est le code qu'on corrige, jamais le test.
//!
//! Chaque test cite sa source. Aucune valeur n'est déduite du code : les corps
//! de `reverb-proto` sont tous `todo!()` à l'écriture de ce fichier.
//!
//! Aucun accès matériel : ces tests sont purement calculatoires (issue #1,
//! critère « Aucun accès matériel dans les tests automatisés »).

use reverb_proto::FRAME_LEN;

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

/// Paquet commençant par `tete`, complété par des zéros (spec §1 et §11 `_pkt`).
fn paquet(tete: &[u8]) -> [u8; FRAME_LEN] {
    let mut t = [0u8; FRAME_LEN];
    t[..tete.len()].copy_from_slice(tete);
    t
}

// ---------------------------------------------------------------------------

mod couleur {
    use reverb_proto::{Brightness, Rgb};

    #[test]
    fn le_rouge_pur_part_en_grb() {
        // spec §2 — « Le rouge pur part en `00 ff 00` » (ordre G, R, B).
        assert_eq!(Rgb::new(0xff, 0x00, 0x00).to_grb(), [0x00, 0xff, 0x00]);
    }

    #[test]
    fn le_vert_pur_part_en_grb() {
        // issue #1, tests d'intention — « le **vert pur** en `ff 00 00` ».
        // (spec §2 illustre « le vert » avec `ff 28 00`, qui est le nuancier CAM
        // du §5.1 et non le vert pur — voir le test suivant.)
        assert_eq!(Rgb::new(0x00, 0xff, 0x00).to_grb(), [0xff, 0x00, 0x00]);
    }

    #[test]
    fn le_bleu_pur_part_en_grb() {
        // spec §5.1 — la LED1 « bleu » du tampon part en `0000ff` : G=00 R=00 B=ff.
        assert_eq!(Rgb::new(0x00, 0x00, 0xff).to_grb(), [0x00, 0x00, 0xff]);
    }

    #[test]
    fn le_vert_du_nuancier_cam_part_en_ff_28_00() {
        // spec §5.1 — la LED3 « vert » part en `ff2800`, soit G=ff R=28 B=00.
        assert_eq!(Rgb::new(0x28, 0xff, 0x00).to_grb(), [0xff, 0x28, 0x00]);
    }

    #[test]
    fn le_rouge_et_le_vert_ne_sont_pas_interchangeables() {
        // spec §2 — « Une lecture RGB naïve intervertit rouge et vert sur toute
        // l'installation. » Le rouge doit sortir les octets du vert naïf, et
        // réciproquement.
        let rouge = Rgb::new(0xff, 0x00, 0x00);
        let vert = Rgb::new(0x00, 0xff, 0x00);
        assert_ne!(rouge.to_grb(), [0xff, 0x00, 0x00]);
        assert_ne!(vert.to_grb(), [0x00, 0xff, 0x00]);
        assert_ne!(rouge.to_grb(), vert.to_grb());
    }

    #[test]
    fn la_luminosite_a_50_pourcent_divise_les_composantes() {
        // spec §4.3 — « Aucun octet ne porte la luminosité. CAM l'applique côté
        // hôte en multipliant les composantes avant l'envoi. »
        // Composantes choisies divisibles par 2 : aucun arrondi en jeu.
        let attendu = Rgb::new(100, 50, 25);
        assert_eq!(
            Rgb::new(200, 100, 50).with_brightness(Brightness::new(50)),
            attendu
        );
    }

    #[test]
    fn la_luminosite_a_30_pourcent_divise_les_composantes() {
        // spec §4.3 — même règle, sur l'exemple `--brightness 30` de l'issue #1.
        // Composantes choisies multiples de 10 : aucun arrondi en jeu.
        let attendu = Rgb::new(30, 60, 15);
        assert_eq!(
            Rgb::new(100, 200, 50).with_brightness(Brightness::new(30)),
            attendu
        );
    }

    #[test]
    fn la_luminosite_a_0_pourcent_produit_du_noir() {
        // spec §4.3 — « luminosité au minimum → CAM envoie `#000000` sur tous
        // les canaux ».
        assert_eq!(
            Rgb::new(0xff, 0xff, 0xff).with_brightness(Brightness::new(0)),
            Rgb::BLACK
        );
        assert_eq!(
            Rgb::new(0x34, 0x00, 0x4f).with_brightness(Brightness::new(0)),
            Rgb::new(0, 0, 0)
        );
    }

    #[test]
    fn la_luminosite_pleine_ne_change_pas_la_couleur() {
        // spec §4.3 — la luminosité est une simple multiplication : à 100 % elle
        // est neutre.
        let couleur = Rgb::new(0x12, 0x34, 0x56);
        assert_eq!(couleur.with_brightness(Brightness::FULL), couleur);
        assert_eq!(couleur.with_brightness(Brightness::new(100)), couleur);
    }

    #[test]
    fn la_luminosite_est_ecretee_a_100_pourcent() {
        // `Brightness::new` — « Construit une luminosité, en écrêtant au-delà de 100. »
        assert_eq!(Brightness::new(200).percent(), 100);
        assert_eq!(Brightness::new(255).percent(), 100);
        assert_eq!(Brightness::new(42).percent(), 42);
        assert_eq!(Brightness::FULL.percent(), 100);
    }

    #[test]
    fn une_couleur_hexadecimale_est_analysee_avec_ou_sans_diese() {
        // issue #1, comportement attendu — `reverb set --all --color ff00ff`.
        // `Rgb::from_hex` : « à six chiffres, avec ou sans `#` ».
        assert_eq!(Rgb::from_hex("ff00ff"), Ok(Rgb::new(0xff, 0x00, 0xff)));
        assert_eq!(Rgb::from_hex("#ff00ff"), Ok(Rgb::new(0xff, 0x00, 0xff)));
        assert_eq!(Rgb::from_hex("00ff00"), Ok(Rgb::new(0x00, 0xff, 0x00)));
        assert_eq!(Rgb::from_hex("ffffff"), Ok(Rgb::new(0xff, 0xff, 0xff)));
    }

    #[test]
    fn une_couleur_hexadecimale_invalide_est_rejetee() {
        // issue #1 — la couleur vient de la ligne de commande, une saisie
        // erronée ne doit pas produire de trame.
        let erreur = Rgb::from_hex("pas-une-couleur").expect_err("doit être rejetée");
        assert_eq!(erreur.input, "pas-une-couleur");
        assert!(Rgb::from_hex("ff00").is_err(), "trop court");
        assert!(Rgb::from_hex("ff00ff00").is_err(), "trop long");
        assert!(Rgb::from_hex("").is_err(), "vide");
    }
}

// ---------------------------------------------------------------------------

mod position {
    use reverb_proto::position::{SERIAL_FAN_CONTROLLER, SERIAL_RGB_SINGLE, SERIAL_RGB_TRIPLE};
    use reverb_proto::{Model, Position};

    /// Les dix noms de position, tels qu'écrits dans la table de la spec §3.
    /// L'issue #1 les reprend dans son exemple `reverb set --fan "droit bas"`.
    const NOMS: [&str; 10] = [
        "bas gauche",
        "bas milieu",
        "bas droite",
        "droit bas",
        "droit milieu",
        "droit haut",
        "gauche",
        "haut droite",
        "haut milieu",
        "haut gauche",
    ];

    fn verifie(position: Position, serie: &str, modele: Model, masque: u8) {
        let placement = position.placement();
        assert_eq!(placement.serial, serie, "numéro de série de {position:?}");
        assert_eq!(placement.model, modele, "modèle de {position:?}");
        assert_eq!(placement.mask, masque, "masque de {position:?}");
    }

    // --- Device 7, `1e71:2019` (spec §3, lignes « 7 ») ---

    #[test]
    fn bas_gauche_est_le_canal_01_du_2019() {
        // spec §3 — device 7, masque `0x01`.
        verifie(
            Position::BasGauche,
            SERIAL_FAN_CONTROLLER,
            Model::RgbAndFan,
            0x01,
        );
    }

    #[test]
    fn bas_milieu_est_le_canal_02_du_2019() {
        // spec §3 — device 7, masque `0x02`.
        verifie(
            Position::BasMilieu,
            SERIAL_FAN_CONTROLLER,
            Model::RgbAndFan,
            0x02,
        );
    }

    #[test]
    fn bas_droite_est_le_canal_04_du_2019() {
        // spec §3 — device 7, masque `0x04`.
        verifie(
            Position::BasDroite,
            SERIAL_FAN_CONTROLLER,
            Model::RgbAndFan,
            0x04,
        );
    }

    #[test]
    fn droit_bas_est_le_canal_08_du_2019() {
        // spec §3 — device 7, masque `0x08`.
        verifie(
            Position::DroitBas,
            SERIAL_FAN_CONTROLLER,
            Model::RgbAndFan,
            0x08,
        );
    }

    #[test]
    fn droit_milieu_est_le_canal_10_du_2019() {
        // spec §3 — device 7, masque `0x10`.
        verifie(
            Position::DroitMilieu,
            SERIAL_FAN_CONTROLLER,
            Model::RgbAndFan,
            0x10,
        );
    }

    #[test]
    fn droit_haut_est_le_canal_20_du_2019() {
        // spec §3 — device 7, masque `0x20`.
        verifie(
            Position::DroitHaut,
            SERIAL_FAN_CONTROLLER,
            Model::RgbAndFan,
            0x20,
        );
    }

    // --- Device 8, `1e71:2012` série `0E01…` (spec §3 + table de l'issue #1) ---

    #[test]
    fn gauche_est_le_canal_01_du_2012_a_un_canal() {
        // spec §3 — device 8, masque `0x01`.
        // issue #1, table des positions — `2012` série `0E01…`, masque `0x01`.
        verifie(Position::Gauche, SERIAL_RGB_SINGLE, Model::Rgb, 0x01);
    }

    // --- Device 9, `1e71:2012` série `1101…` (spec §3 + table de l'issue #1) ---

    #[test]
    fn haut_droite_est_le_canal_01_du_2012_a_trois_canaux() {
        // spec §3 — device 9, masque `0x01`.
        // issue #1, table des positions — `2012` série `1101…`.
        verifie(Position::HautDroite, SERIAL_RGB_TRIPLE, Model::Rgb, 0x01);
    }

    #[test]
    fn haut_milieu_est_le_canal_02_du_2012_a_trois_canaux() {
        // spec §3 — device 9, masque `0x02`.
        verifie(Position::HautMilieu, SERIAL_RGB_TRIPLE, Model::Rgb, 0x02);
    }

    #[test]
    fn haut_gauche_est_le_canal_04_du_2012_a_trois_canaux() {
        // spec §3 — device 9, masque `0x04`.
        verifie(Position::HautGauche, SERIAL_RGB_TRIPLE, Model::Rgb, 0x04);
    }

    // --- Couverture et unicité ---

    #[test]
    fn les_dix_positions_physiques_sont_couvertes() {
        // spec §3 — la table compte exactement dix lignes.
        // issue #1, critère d'acceptation — « Les **10 ventilateurs** prennent
        // la couleur demandée ».
        assert_eq!(Position::ALL.len(), 10);
        assert_eq!(Position::names().len(), 10);

        let mut couples: Vec<(&str, u8)> = Position::ALL
            .iter()
            .map(|p| {
                let placement = p.placement();
                (placement.serial, placement.mask)
            })
            .collect();
        couples.sort_unstable();
        couples.dedup();
        assert_eq!(
            couples.len(),
            10,
            "chaque position doit viser un couple (série, masque) distinct"
        );
    }

    #[test]
    fn chaque_position_porte_le_nom_de_la_spec() {
        // spec §3, colonne « Position physique ».
        for (position, nom) in Position::ALL.iter().zip(NOMS) {
            assert_eq!(position.name(), nom, "nom de {position:?}");
        }
        assert_eq!(Position::names(), NOMS);
    }

    #[test]
    fn chaque_nom_se_resout_vers_sa_position() {
        // issue #1 — « désignés par leur **position physique** », exemple
        // `reverb set --fan "droit bas"`. Noms de la spec §3.
        for (nom, attendue) in NOMS.iter().zip(Position::ALL) {
            assert_eq!(
                Position::from_name(nom),
                Ok(attendue),
                "résolution de « {nom} »"
            );
        }
    }

    #[test]
    fn le_nom_et_la_resolution_font_un_aller_retour() {
        // spec §3 — la table est une bijection position ↔ nom.
        for position in Position::ALL {
            assert_eq!(Position::from_name(position.name()), Ok(position));
        }
    }

    #[test]
    fn une_position_inconnue_est_rejetee_en_listant_les_positions_valides() {
        // issue #1, critère d'acceptation — « Une position inconnue produit une
        // erreur explicite listant les positions valides ».
        let erreur = Position::from_name("milieu du plafond").expect_err("doit être rejetée");
        assert_eq!(erreur.input, "milieu du plafond");
        for nom in NOMS {
            assert!(
                erreur.valid.contains(&nom),
                "l'erreur doit citer la position valide « {nom} »"
            );
        }
        assert_eq!(erreur.valid.len(), 10);
    }

    #[test]
    fn une_position_vide_est_rejetee() {
        // issue #1 — même critère : rien ne doit être résolu par défaut.
        assert!(Position::from_name("").is_err());
    }

    #[test]
    fn les_deux_controleurs_2012_sont_distingues_par_leur_serie() {
        // issue #1, critère d'acceptation — « Les deux contrôleurs `1e71:2012`,
        // physiquement identiques, sont distingués par leur série ».
        // spec §11 — « le canal 1 du device 7 et le canal 1 du device 9 sont
        // deux ventilateurs différents ».
        let unique = Position::Gauche.placement();
        let triple = Position::HautDroite.placement();

        assert_eq!(unique.mask, triple.mask, "même masque de canal `0x01`");
        assert_eq!(unique.model, Model::Rgb);
        assert_eq!(triple.model, Model::Rgb);
        assert_eq!(unique.model.product_id(), triple.model.product_id());
        assert_ne!(
            unique.serial, triple.serial,
            "la série est le seul moyen de les distinguer"
        );
        assert_eq!(unique.serial, SERIAL_RGB_SINGLE);
        assert_eq!(triple.serial, SERIAL_RGB_TRIPLE);
    }

    #[test]
    fn le_2019_et_les_2012_ne_partagent_aucune_serie() {
        // spec §1 — trois contrôleurs distincts sur le bus.
        // issue #1 — résolution par VID:PID + numéro de série.
        assert_ne!(SERIAL_FAN_CONTROLLER, SERIAL_RGB_SINGLE);
        assert_ne!(SERIAL_FAN_CONTROLLER, SERIAL_RGB_TRIPLE);
        assert_ne!(SERIAL_RGB_SINGLE, SERIAL_RGB_TRIPLE);
        assert_eq!(Position::BasGauche.placement().model, Model::RgbAndFan);
    }

    #[test]
    fn le_2019_pilote_six_positions_et_les_2012_quatre() {
        // spec §1 — device 7 : « 6 canaux LED » ; device 8 : « 1 canal LED
        // utilisé » ; device 9 : « 3 canaux LED utilisés ».
        let compte = |serie: &str| {
            Position::ALL
                .iter()
                .filter(|p| p.placement().serial == serie)
                .count()
        };
        assert_eq!(compte(SERIAL_FAN_CONTROLLER), 6);
        assert_eq!(compte(SERIAL_RGB_SINGLE), 1);
        assert_eq!(compte(SERIAL_RGB_TRIPLE), 3);
    }
}

// ---------------------------------------------------------------------------

mod trame {
    use super::{paquet, trame_avec};
    use reverb_proto::frame::fixed_color;
    use reverb_proto::{Brightness, FRAME_LEN, LEDS_PER_FAN, Position, Rgb};

    #[test]
    fn la_trame_couleur_fixe_est_exacte_octet_pour_octet() {
        // spec §4 (table des offsets) et §11 (implémentation de référence
        // `couleur_fixe`), sur l'exemple `--color ff00ff` de l'issue #1,
        // canal `0x01` = « bas gauche » (spec §3).
        //
        //   0–1 : `2a 04`            5    : vitesse `0x32` en mode fixe (§4.2)
        //   2   : masque             6    : `0x00` hors mode `0x05` (§4)
        //   3   : même masque (§4)   7–9  : couleur, triplet GRB (§2, §4)
        //   4   : mode fixe `0x00`   56   : une seule couleur fournie (§4)
        //                            58   : 8 LED (§4)   59 : type `0x03` (§4)
        //
        // L'offset 57 vaut `0x00` ici : §4 le marque ❓ (« `0x00` ou `0x08`
        // selon le mode ») et §11 laisse le tampon à zéro en mode fixe.
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

        let obtenue = fixed_color(
            Position::BasGauche.placement().mask,
            Rgb::new(0xff, 0x00, 0xff),
            Brightness::FULL,
            LEDS_PER_FAN,
        );

        assert_eq!(obtenue, attendue);
    }

    #[test]
    fn la_trame_verte_du_dernier_canal_est_exacte_octet_pour_octet() {
        // spec §0 — « couleur fixe verte sur les 6+3+1 canaux : les dix
        // ventilateurs passent au vert ». Ici le canal `0x20` du `2019`,
        // soit « droit haut » (spec §3), en vert pur → GRB `ff 00 00`
        // (issue #1, tests d'intention).
        let attendue = trame_avec(&[
            (0, 0x2a),
            (1, 0x04),
            (2, 0x20),
            (3, 0x20),
            (5, 0x32),
            (7, 0xff), // G
            (8, 0x00), // R
            (9, 0x00), // B
            (56, 0x01),
            (58, 0x08),
            (59, 0x03),
        ]);

        let obtenue = fixed_color(
            Position::DroitHaut.placement().mask,
            Rgb::new(0x00, 0xff, 0x00),
            Brightness::FULL,
            LEDS_PER_FAN,
        );

        assert_eq!(obtenue, attendue);
    }

    #[test]
    fn la_trame_fait_exactement_64_octets() {
        // spec §1 — « HID interrupt, paquets de 64 octets […] toujours complétés
        // à 64 octets par des zéros ».
        // spec §0 — `OutputReportByteLength` vaut 64 et non 65.
        assert_eq!(FRAME_LEN, 64);
        let t = fixed_color(0x01, Rgb::new(1, 2, 3), Brightness::FULL, LEDS_PER_FAN);
        assert_eq!(t.len(), 64);
    }

    #[test]
    fn la_trame_ne_prefixe_pas_d_octet_de_rapport_nul() {
        // spec §0 — « le premier octet de la trame (`0x2a`) est l'identifiant de
        // rapport HID. Il ne faut donc pas préfixer un octet `0x00` ».
        let t = fixed_color(
            0x01,
            Rgb::new(0xff, 0xff, 0xff),
            Brightness::FULL,
            LEDS_PER_FAN,
        );
        assert_eq!(
            t[0], 0x2a,
            "l'octet 0 est la commande, pas un `0x00` ajouté"
        );
        assert_eq!(t[1], 0x04);
    }

    #[test]
    fn les_offsets_2_et_3_portent_le_meme_masque() {
        // spec §4 — offset 2 : masque de canaux ; offset 3 : « toujours égal à
        // l'offset 2 ». spec §3 — « les offsets 2 et 3 sont restés égaux sur
        // toutes les trames observées ».
        for masque in [0x01u8, 0x02, 0x04, 0x08, 0x10, 0x20] {
            let t = fixed_color(masque, Rgb::new(1, 2, 3), Brightness::FULL, LEDS_PER_FAN);
            assert_eq!(t[2], masque, "offset 2 pour le masque {masque:#04x}");
            assert_eq!(t[3], t[2], "offset 3 pour le masque {masque:#04x}");
        }
    }

    #[test]
    fn le_masque_de_chaque_position_est_recopie_tel_quel() {
        // spec §3 — un bit par canal, aucune agrégation : « CAM n'agrège jamais
        // plusieurs canaux dans une trame ».
        for position in Position::ALL {
            let masque = position.placement().mask;
            let t = fixed_color(masque, Rgb::new(1, 2, 3), Brightness::FULL, LEDS_PER_FAN);
            assert_eq!(t[2], masque, "masque de {position:?}");
            assert_eq!(t[3], masque, "masque de {position:?}");
            assert_eq!(masque.count_ones(), 1, "un seul bit par canal");
        }
    }

    #[test]
    fn l_offset_4_vaut_00_le_mode_couleur_fixe() {
        // spec §4.1 — mode `0x00` : « couleur fixe », une seule couleur.
        let t = fixed_color(0x01, Rgb::new(1, 2, 3), Brightness::FULL, LEDS_PER_FAN);
        assert_eq!(t[4], 0x00);
    }

    #[test]
    fn l_offset_5_porte_la_vitesse_0x32_en_mode_fixe() {
        // spec §4.2 — « l'octet 5 est la vitesse, pas la luminosité »,
        // « valeurs observées : `0x32` en fixe ». spec §11 — `buf[5] = 0x32`.
        let t = fixed_color(0x01, Rgb::new(1, 2, 3), Brightness::FULL, LEDS_PER_FAN);
        assert_eq!(t[5], 0x32);
    }

    #[test]
    fn l_offset_5_ne_bouge_pas_avec_la_luminosite() {
        // spec §4.2 — « les deux réglages de luminosité (actions 5 et 6) le
        // laissent inchangé ».
        for pourcent in [0u8, 30, 50, 100] {
            let t = fixed_color(
                0x01,
                Rgb::new(200, 100, 50),
                Brightness::new(pourcent),
                LEDS_PER_FAN,
            );
            assert_eq!(t[5], 0x32, "vitesse à {pourcent} % de luminosité");
        }
    }

    #[test]
    fn l_offset_6_vaut_00_hors_du_mode_05() {
        // spec §4 — offset 6 : « `0x00` sauf mode `0x05` ». Le mode fixe est `0x00`.
        let t = fixed_color(0x01, Rgb::new(1, 2, 3), Brightness::FULL, LEDS_PER_FAN);
        assert_eq!(t[6], 0x00);
    }

    #[test]
    fn la_couleur_occupe_les_offsets_7_a_9_en_grb() {
        // spec §4 — « offset 7… : couleurs, triplets GRB consécutifs ».
        // spec §2 — ordre G, R, B.
        let couleur = Rgb::new(0x12, 0x34, 0x56);
        let t = fixed_color(0x01, couleur, Brightness::FULL, LEDS_PER_FAN);
        assert_eq!(&t[7..10], &couleur.to_grb());
        assert_eq!(&t[7..10], &[0x34, 0x12, 0x56]);
    }

    #[test]
    fn une_seule_couleur_est_fournie_offset_56() {
        // spec §4 — offset 56 : « nombre de couleurs fournies […] vaut 1, 2 ou 3
        // et correspond exactement ». spec §4.1 — le mode fixe en attend 1.
        let t = fixed_color(0x01, Rgb::new(1, 2, 3), Brightness::FULL, LEDS_PER_FAN);
        assert_eq!(t[56], 0x01);
    }

    #[test]
    fn les_offsets_58_et_59_valent_08_et_03() {
        // spec §4 — offset 58 : « `0x08`, nombre de LED de l'accessoire » ;
        // offset 59 : « `0x03`, type d'accessoire ». spec §11 — `buf[58] = nb_led`.
        let t = fixed_color(0x01, Rgb::new(1, 2, 3), Brightness::FULL, LEDS_PER_FAN);
        assert_eq!(t[58], 0x08);
        assert_eq!(t[58], LEDS_PER_FAN);
        assert_eq!(t[59], 0x03);
    }

    #[test]
    fn les_octets_sans_signification_restent_nuls() {
        // spec §1 — « les paquets sont toujours complétés à 64 octets par des
        // zéros ». spec §4 — au-delà d'une couleur, les offsets 10 à 55 ne
        // portent rien en mode fixe (§11, tampon zéro-initialisé).
        let t = fixed_color(
            0x01,
            Rgb::new(0xff, 0xff, 0xff),
            Brightness::FULL,
            LEDS_PER_FAN,
        );
        assert!(
            t[10..56].iter().all(|&o| o == 0),
            "offsets 10..56 : {:?}",
            &t[10..56]
        );
        assert!(
            t[60..64].iter().all(|&o| o == 0),
            "offsets 60..64 : {:?}",
            &t[60..64]
        );
    }

    #[test]
    fn la_luminosite_ne_touche_que_les_octets_de_couleur() {
        // spec §4.3 — « Aucun octet ne porte la luminosité. » Elle ne doit donc
        // modifier que les offsets 7 à 9. Composantes divisibles par 2 :
        // (200, 100, 50) à 50 % → (100, 50, 25).
        let pleine = fixed_color(0x04, Rgb::new(200, 100, 50), Brightness::FULL, LEDS_PER_FAN);
        let moitie = fixed_color(
            0x04,
            Rgb::new(200, 100, 50),
            Brightness::new(50),
            LEDS_PER_FAN,
        );

        assert_eq!(&pleine[7..10], &[100, 200, 50], "GRB à pleine luminosité");
        assert_eq!(&moitie[7..10], &[50, 100, 25], "GRB à 50 %");

        for offset in (0..FRAME_LEN).filter(|o| !(7..10).contains(o)) {
            assert_eq!(
                pleine[offset], moitie[offset],
                "l'offset {offset} ne doit pas dépendre de la luminosité"
            );
        }
    }

    #[test]
    fn la_luminosite_nulle_produit_une_trame_noire() {
        // spec §4.3 — « luminosité au minimum → CAM envoie `#000000` sur tous
        // les canaux ». La trame reste une trame `2a 04` valide, seule la
        // couleur est nulle.
        let t = fixed_color(
            0x02,
            Rgb::new(0xff, 0xff, 0xff),
            Brightness::new(0),
            LEDS_PER_FAN,
        );
        assert_eq!(&t[7..10], &[0x00, 0x00, 0x00]);
        assert_eq!(
            t,
            trame_avec(&[
                (0, 0x2a),
                (1, 0x04),
                (2, 0x02),
                (3, 0x02),
                (5, 0x32),
                (56, 0x01),
                (58, 0x08),
                (59, 0x03),
            ])
        );
    }

    #[test]
    fn la_trame_est_toujours_completee_par_des_zeros() {
        // spec §1 et §11 (`_pkt`) — le remplissage est du zéro, jamais autre chose.
        let t = fixed_color(0x01, Rgb::BLACK, Brightness::new(0), LEDS_PER_FAN);
        let squelette = paquet(&[0x2a, 0x04, 0x01, 0x01, 0x00, 0x32]);
        assert_eq!(&t[..7], &squelette[..7]);
    }
}

// ---------------------------------------------------------------------------

mod initialisation {
    use super::paquet;
    use reverb_proto::Model;
    use reverb_proto::frame::init_sequence;

    #[test]
    fn la_sequence_du_2019_est_exacte_trame_par_trame() {
        // spec §8 — device 7 (`1e71:2019`) :
        //     10 02
        //     20 03
        //     60 03
        //     60 02 01 e8 03 01 e8 03      <- 0x03e8 = 1000, deux fois
        let attendue = vec![
            paquet(&[0x10, 0x02]),
            paquet(&[0x20, 0x03]),
            paquet(&[0x60, 0x03]),
            paquet(&[0x60, 0x02, 0x01, 0xe8, 0x03, 0x01, 0xe8, 0x03]),
        ];
        assert_eq!(init_sequence(Model::RgbAndFan), attendue);
    }

    #[test]
    fn la_sequence_du_2019_utilise_10_02() {
        // spec §8 — « L'argument de `0x10` dépend du modèle » : `0x02` pour le 2019.
        let sequence = init_sequence(Model::RgbAndFan);
        let premiere = sequence.first().expect("la séquence n'est pas vide");
        assert_eq!(&premiere[..2], &[0x10, 0x02]);
    }

    #[test]
    fn la_sequence_du_2019_comprend_les_commandes_60() {
        // spec §8 — seul le `2019` reçoit `60 03` puis
        // `60 02 01 e8 03 01 e8 03`.
        let sequence = init_sequence(Model::RgbAndFan);
        let commandes_60: Vec<&[u8; 64]> = sequence.iter().filter(|t| t[0] == 0x60).collect();
        assert_eq!(commandes_60.len(), 2, "deux commandes `0x60`");
        assert_eq!(&commandes_60[0][..2], &[0x60, 0x03]);
        assert_eq!(
            &commandes_60[1][..8],
            &[0x60, 0x02, 0x01, 0xe8, 0x03, 0x01, 0xe8, 0x03]
        );
    }

    #[test]
    fn la_sequence_des_2012_est_exacte_trame_par_trame() {
        // spec §8 — devices 8 et 9 (`1e71:2012`) :
        //     10 01
        //     20 03
        let attendue = vec![paquet(&[0x10, 0x01]), paquet(&[0x20, 0x03])];
        assert_eq!(init_sequence(Model::Rgb), attendue);
    }

    #[test]
    fn la_sequence_des_2012_utilise_10_01() {
        // spec §8 — l'argument de `0x10` vaut `0x01` sur les `2012`.
        let sequence = init_sequence(Model::Rgb);
        let premiere = sequence.first().expect("la séquence n'est pas vide");
        assert_eq!(&premiere[..2], &[0x10, 0x01]);
    }

    #[test]
    fn la_sequence_des_2012_s_arrete_a_20_03() {
        // spec §8 — la séquence des `2012` ne comporte que deux trames et
        // aucune commande `0x60`.
        let sequence = init_sequence(Model::Rgb);
        assert_eq!(sequence.len(), 2);
        let derniere = sequence.last().expect("la séquence n'est pas vide");
        assert_eq!(&derniere[..2], &[0x20, 0x03]);
        assert!(
            sequence.iter().all(|t| t[0] != 0x60),
            "aucune commande `0x60` sur un `2012`"
        );
    }

    #[test]
    fn les_deux_sequences_different() {
        // issue #1, tests d'intention — « La séquence d'initialisation du `2019`
        // utilise `10 02` et comprend les commandes `0x60`, celle des `2012`
        // utilise `10 01` et s'arrête à `20 03` » (spec §8).
        let du_2019 = init_sequence(Model::RgbAndFan);
        let des_2012 = init_sequence(Model::Rgb);
        assert_ne!(du_2019, des_2012);
        assert_eq!(du_2019.len(), 4);
        assert_eq!(des_2012.len(), 2);
        assert_ne!(du_2019[0], des_2012[0], "l'argument de `0x10` diffère");
        assert_eq!(du_2019[1], des_2012[1], "les deux passent par `20 03`");
    }

    #[test]
    fn toutes_les_trames_d_initialisation_font_64_octets_completes_par_des_zeros() {
        // spec §1 — « les paquets sont toujours complétés à 64 octets par des
        // zéros ». spec §11 — `_pkt` : `bytes(head) + bytes(64 - len(head))`.
        for modele in [Model::RgbAndFan, Model::Rgb] {
            for (index, trame) in init_sequence(modele).iter().enumerate() {
                assert_eq!(trame.len(), 64, "trame {index} de {modele:?}");
            }
        }

        // Au-delà des octets significatifs, tout est nul.
        let sequence = init_sequence(Model::RgbAndFan);
        assert!(sequence[0][2..].iter().all(|&o| o == 0), "après `10 02`");
        assert!(sequence[1][2..].iter().all(|&o| o == 0), "après `20 03`");
        assert!(sequence[2][2..].iter().all(|&o| o == 0), "après `60 03`");
        assert!(sequence[3][8..].iter().all(|&o| o == 0), "après `60 02 …`");
    }
}
