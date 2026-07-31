//! Tests d'intention de l'éclairage de la RAM Corsair (issue #15).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #15, son commentaire « Contrat d'API » et
//! `docs/SPEC-CORSAIR-RAM.md` seuls. Aucun octet n'est relu depuis `src/` : à l'écriture de ce
//! fichier, le module `ram` n'existe pas. Ils encodent ce que le logiciel doit faire, pas ce que
//! le code fait — si l'un d'eux échoue après implémentation, c'est le code qu'on corrige.
//!
//! ⚠️ **C'est la seule cible du projet où une erreur est irréversible.** Le même bus porte les
//! hubs SPD des barrettes en `0x50`–`0x53` (spec §3) : une écriture au mauvais endroit corrompt
//! le SPD et rend un DIMM non démarrable. D'où le test n° 8, **exhaustif sur les 256 valeurs de
//! `u8` exprimées en `usize`** : il ne vérifie pas que `0x50` est refusé, il vérifie qu'aucune
//! entrée, quelle qu'elle soit, ne le produit.
//!
//! ⚠️ **Trois ordres de composantes différents dans ce projet** (spec §4.1.1) : ventilateurs NZXT
//! en GRB, écran Kraken en BGR, **RAM Corsair en RGB**. Une erreur d'ordre ne produit aucun
//! message — juste une mauvaise couleur. Le test n° 3 est donc construit pour l'attraper : onze
//! couleurs dont les trois composantes diffèrent toutes entre elles, si bien qu'aucune
//! permutation ne peut passer au travers.
//!
//! Le module `ram` **n'est pas ré-exporté à la racine** du crate (contrat d'API :
//! `ram::LedCountError` entrerait en collision avec `led::LedCountError`) — d'où les chemins
//! `reverb_proto::ram::…`. `Rgb`, lui, vit à la racine comme pour les autres cibles.
//!
//! `crc8` reste privé (contrat d'API). Le test n° 4 réécrit donc la formule du §4.2 dans ce
//! fichier : une implémentation indépendante vérifie mieux que la nôtre comparée à elle-même.
//!
//! Aucun accès matériel, aucune IO — `reverb-proto` est pur, ses tests aussi.

use reverb_proto::Rgb;
use reverb_proto::ram::{
    HEAD_LEN, HEAD_TRANSFER_LEN, LEDS_PER_STICK, LedCountError, PAYLOAD_LEN, REGISTER_HEAD,
    REGISTER_TAIL, SLOT_COUNT, SlotAddress, TAIL_LEN, TAIL_TRANSFER_LEN, payload, transfers,
};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Onze couleurs dont **les trois composantes diffèrent**, et qui diffèrent entre elles.
///
/// C'est la propriété qui rend le test d'ordre concluant : sur `(0x01, 0x23, 0x45)`, un encodage
/// GRB donnerait `23 01 45` et un encodage BGR `45 23 01` — deux triplets distincts de
/// l'attendu. Une couleur grise, ou une couleur pure comme `ff 00 00`, laisserait passer au
/// moins une permutation. Les onze étant deux à deux différentes, un ordre de LED permuté est
/// détecté par la même comparaison.
const COMPOSANTES_DISTINCTES: [[u8; 3]; LEDS_PER_STICK] = [
    [0x01, 0x23, 0x45],
    [0x67, 0x89, 0xab],
    [0xcd, 0xef, 0x02],
    [0x13, 0x57, 0x9b],
    [0xdf, 0x24, 0x68],
    [0xac, 0xe0, 0x11],
    [0x22, 0x44, 0x88],
    [0x99, 0xbb, 0xdd],
    [0xee, 0x33, 0x55],
    [0x77, 0xaa, 0xcc],
    [0xfe, 0xdc, 0xba],
];

/// Charge utile relevée sur le matériel — `captures/smbus-blocs.csv`, lignes 2 et 3.
///
/// ```text
/// "0x18","0x31","32","32","0b ff 06 c9 ff 06 c9 … ff 06 c9 ff"
/// "0x18","0x32","3","3","06 c9 8c"
/// ```
///
/// Les onze LED de la barrette 0 sont sur la même couleur `ff 06 c9`, et le CRC vaut `0x8c`.
/// Cette valeur n'est **pas calculée ici** : c'est ce qu'iCUE a réellement émis, seul juge de la
/// formule du §4.2. Le même vecteur apparaît en `0x1a` (lignes 58 et 59).
const CHARGE_RELEVEE: [u8; PAYLOAD_LEN] = [
    0x0b, // nombre de LED, spec §4.1
    0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, //
    0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, //
    0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, //
    0x8c, // CRC-8/ATM des 34 octets précédents
];

/// Le bloc `0x31` tel qu'il part sur le fil : `[registre][compte][32 octets]` (spec §4.3, §4.4).
const TETE_RELEVEE: [u8; HEAD_TRANSFER_LEN] = [
    0x31, 0x20, // registre, puis 32 octets annoncés
    0x0b, 0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9,
    0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, 0xff, 0x06, 0xc9, 0xff,
];

/// Le bloc `0x32` : les trois octets restants, dont le CRC (spec §4.3).
const QUEUE_RELEVEE: [u8; TAIL_TRANSFER_LEN] = [0x32, 0x03, 0x06, 0xc9, 0x8c];

/// Les onze couleurs du vecteur relevé.
fn couleurs_relevees() -> [Rgb; LEDS_PER_STICK] {
    [Rgb::new(0xff, 0x06, 0xc9); LEDS_PER_STICK]
}

/// Les onze couleurs de `COMPOSANTES_DISTINCTES`.
fn couleurs_distinctes() -> [Rgb; LEDS_PER_STICK] {
    COMPOSANTES_DISTINCTES.map(|[r, v, b]| Rgb::new(r, v, b))
}

/// Les 33 octets de couleur attendus, dans l'ordre RGB, sans permutation.
fn composantes_a_plat() -> [u8; LEDS_PER_STICK * 3] {
    let mut plat = [0u8; LEDS_PER_STICK * 3];
    for (triplet, place) in COMPOSANTES_DISTINCTES.iter().zip(plat.chunks_mut(3)) {
        place.copy_from_slice(triplet);
    }
    plat
}

/// Onze couleurs reconstituées depuis 33 composantes à plat.
fn couleurs_depuis(plat: &[u8; LEDS_PER_STICK * 3]) -> Vec<Rgb> {
    plat.chunks(3).map(|c| Rgb::new(c[0], c[1], c[2])).collect()
}

/// `n` couleurs distinctes, sans autre propriété que d'être différentes.
///
/// Sert à faire varier le **nombre** de couleurs sans que leur valeur n'entre en jeu.
fn couleurs(n: usize) -> Vec<Rgb> {
    (0..n).map(|i| Rgb::new(i as u8 + 1, 0x20, 0x30)).collect()
}

/// La charge utile, pour un nombre de couleurs qu'on sait valide.
fn charge_utile(couleurs: &[Rgb]) -> [u8; PAYLOAD_LEN] {
    payload(couleurs).expect("onze couleurs, une par LED (spec §4.1)")
}

/// CRC-8/ATM, réécrit depuis le §4.2 : polynôme `0x07`, valeur initiale `0x00`, **sans**
/// réflexion ni XOR final.
///
/// Écrit ici et pas importé : `crc8` est privé (contrat d'API), et une implémentation
/// indépendante vérifie mieux que la nôtre comparée à elle-même.
fn crc8_atm(donnees: &[u8]) -> u8 {
    let mut reste: u8 = 0x00;
    for &octet in donnees {
        reste ^= octet;
        for _ in 0..8 {
            reste = if reste & 0x80 != 0 {
                (reste << 1) ^ 0x07
            } else {
                reste << 1
            };
        }
    }
    reste
}

// ---------------------------------------------------------------------------
// 1 — la taille de la charge utile
// ---------------------------------------------------------------------------

#[test]
fn la_charge_utile_fait_exactement_trente_cinq_octets() {
    // spec §4.1 — « Charge utile logique — 35 octets : [0] nombre de LED, [1..33] 11 triplets
    // RGB consécutifs, [34] CRC-8 ». Contrat d'API — `PAYLOAD_LEN: usize = 35`, « 1 + 11 × 3 + 1 ».
    assert_eq!(PAYLOAD_LEN, 35);
    assert_eq!(
        PAYLOAD_LEN,
        1 + LEDS_PER_STICK * 3 + 1,
        "le compte, les onze triplets, le CRC — et rien d'autre"
    );

    let charge = charge_utile(&couleurs_distinctes());
    assert_eq!(charge.len(), PAYLOAD_LEN);
    assert_eq!(charge_utile(&couleurs_relevees()).len(), PAYLOAD_LEN);
}

// ---------------------------------------------------------------------------
// 2 — l'octet de tête
// ---------------------------------------------------------------------------

#[test]
fn l_octet_zero_annonce_onze_led() {
    // spec §4.1 — « [0] nombre de LED = 0x0b (11) ». spec, en-tête — « 11 LED par barrette ».
    // Contrat d'API — `LEDS_PER_STICK: usize = 11`, « octet 0 de la charge utile (spec §4.1) ».
    // La valeur est écrite en dur *et* comparée à la constante : le protocole dit `0x0b`, la
    // constante doit dire la même chose, et la charge utile ne doit pas s'en écarter.
    assert_eq!(LEDS_PER_STICK, 11);

    for jeu in [couleurs_distinctes(), couleurs_relevees()] {
        let charge = charge_utile(&jeu);
        assert_eq!(charge[0], 0x0b, "spec §4.1, octet 0");
        assert_eq!(usize::from(charge[0]), LEDS_PER_STICK);
    }
}

// ---------------------------------------------------------------------------
// 3 — l'ordre des composantes
// ---------------------------------------------------------------------------

#[test]
fn les_onze_triplets_partent_en_rgb_sans_permutation() {
    // spec §4.1 — « ✅ Ordre des composantes : RGB. Simple et direct. »
    // spec §4.1.1 — « ⚠️ Attention aux trois ordres différents dans ce projet : ventilateurs
    // NZXT en GRB, écran Kraken en BGR, RAM Corsair en RGB. »
    // spec §6, implémentation de référence — `payload += bytes((r, g, b))  # RGB, sans permutation`.
    // issue #15 — l'ordre est la première source d'erreur du projet, et une erreur ici ne produit
    // aucun message.
    //
    // Les onze couleurs ont leurs trois composantes distinctes : un encodage GRB ou BGR
    // donnerait un triplet différent dès la première LED, et un ordre de LED permuté serait
    // détecté par la même comparaison, les onze couleurs étant deux à deux différentes.
    let charge = charge_utile(&couleurs_distinctes());
    assert_eq!(
        &charge[1..1 + LEDS_PER_STICK * 3],
        &composantes_a_plat()[..],
        "les 33 octets de couleur, en RGB, dans l'ordre des LED"
    );

    // La même chose LED par LED, pour que l'échec désigne le triplet fautif.
    for (i, attendu) in COMPOSANTES_DISTINCTES.iter().enumerate() {
        let debut = 1 + 3 * i;
        assert_eq!(
            &charge[debut..debut + 3],
            attendu,
            "LED {} à l'offset {debut} — R puis V puis B",
            i + 1
        );
    }

    // spec §4.1, tableau des couleurs franches : le rouge de la barrette `0x18` part en tête de
    // triplet. Les écarts qu'on y lit (`ff 13 00`) sont « la correction colorimétrique appliquée
    // par iCUE, pas une propriété du protocole » — Reverb n'en applique aucune.
    let rouge = charge_utile(&[Rgb::new(0xff, 0x00, 0x00); LEDS_PER_STICK]);
    assert_eq!(&rouge[1..4], &[0xff, 0x00, 0x00], "rouge pur");
    let vert = charge_utile(&[Rgb::new(0x00, 0xff, 0x00); LEDS_PER_STICK]);
    assert_eq!(&vert[1..4], &[0x00, 0xff, 0x00], "vert pur");
    let bleu = charge_utile(&[Rgb::new(0x00, 0x00, 0xff); LEDS_PER_STICK]);
    assert_eq!(&bleu[1..4], &[0x00, 0x00, 0xff], "bleu pur");
}

// ---------------------------------------------------------------------------
// 4 — le CRC
// ---------------------------------------------------------------------------

#[test]
fn le_dernier_octet_est_le_crc8_atm_des_trente_quatre_premiers() {
    // spec §4.2 — « Polynôme 0x07, valeur initiale 0x00, sans réflexion ni XOR final — le CRC-8/ATM
    // classique, calculé sur les 34 premiers octets de la charge utile. Vérifié sur 40 blocs,
    // 40 concordances exactes. »
    // Contrat d'API — « le test n° 4 réécrit la formule depuis le §4.2 : une implémentation
    // indépendante vérifie mieux que la nôtre comparée à elle-même ».
    for jeu in [couleurs_distinctes(), couleurs_relevees()] {
        let charge = charge_utile(&jeu);
        assert_eq!(
            charge[PAYLOAD_LEN - 1],
            crc8_atm(&charge[..PAYLOAD_LEN - 1]),
            "le CRC porte sur les 34 premiers octets, lui-même exclu"
        );
    }

    // Changer un seul des 34 octets change le dernier. Le seul octet de charge utile que ce test
    // ne pilote pas est le compte de LED, figé à `0x0b` par le protocole ; les 33 autres sont les
    // composantes de couleur, modifiées une par une. Un CRC dont le générateur a un terme
    // constant détecte toujours une altération d'un octet unique : si l'une de ces 33 variantes
    // rend le même CRC, la formule implémentée n'est pas celle du §4.2.
    let plat = composantes_a_plat();
    let temoin = charge_utile(&couleurs_distinctes())[PAYLOAD_LEN - 1];
    for (indice, &originale) in plat.iter().enumerate() {
        let mut modifie = plat;
        modifie[indice] = originale ^ 0xff;

        let variante = charge_utile(&couleurs_depuis(&modifie));
        assert_ne!(
            variante[PAYLOAD_LEN - 1],
            temoin,
            "l'octet de couleur {indice} ({originale:#04x} → {:#04x}) doit changer le CRC",
            modifie[indice]
        );
        assert_eq!(
            variante[PAYLOAD_LEN - 1],
            crc8_atm(&variante[..PAYLOAD_LEN - 1]),
            "et la variante reste un CRC-8/ATM correct"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — le vecteur relevé sur le matériel
// ---------------------------------------------------------------------------

#[test]
fn le_vecteur_releve_sur_le_materiel_est_reproduit_octet_pour_octet() {
    // `captures/smbus-blocs.csv`, lignes 2 et 3 — la barrette `0x18`, onze LED en `ff 06 c9` :
    //
    //     "0x18","0x31","32","32","0b ff 06 c9 ff 06 c9 … ff 06 c9 ff"
    //     "0x18","0x32","3","3","06 c9 8c"
    //
    // Le CRC `0x8c` est **relevé, pas calculé** : c'est l'octet qu'iCUE a réellement émis. C'est
    // le seul test du fichier qui confronte l'encodage à du trafic observé plutôt qu'à une règle
    // relue — si la spec §4.2 s'était trompée de polynôme, c'est ici que ça se verrait.
    let charge = charge_utile(&couleurs_relevees());
    assert_eq!(charge, CHARGE_RELEVEE);
    assert_eq!(charge[PAYLOAD_LEN - 1], 0x8c, "CRC relevé sur le matériel");

    let (tete, queue) = transfers(&couleurs_relevees()).expect("onze couleurs (spec §4.1)");
    assert_eq!(tete, TETE_RELEVEE, "bloc 0x31 de la capture");
    assert_eq!(queue, QUEUE_RELEVEE, "bloc 0x32 de la capture");
}

// ---------------------------------------------------------------------------
// 6 — le découpage en deux transferts
// ---------------------------------------------------------------------------

#[test]
fn les_deux_transferts_portent_leur_registre_leur_compte_et_la_charge_utile() {
    // spec §4.3 — « Un bloc SMBus est limité à 32 octets ; les 35 octets sont donc scindés :
    // `0x31` → 32 octets, charge utile [0..31] ; `0x32` → 3 octets, charge utile [32..34], dont
    // le CRC. Les deux transferts se suivent immédiatement, vers la même adresse. »
    // spec §4.4 — sur le fil : `SMBHSTCMD` = le registre, `SMBHSTDAT0` = le nombre d'octets,
    // puis les données. Contrat d'API — « une écriture SMBus par bloc est, sur le fil, un
    // write() I2C de [registre][compte][données] — d'où les deux octets d'en-tête ».
    assert_eq!(REGISTER_HEAD, 0x31);
    assert_eq!(REGISTER_TAIL, 0x32);
    assert_eq!(HEAD_LEN, 32);
    assert_eq!(TAIL_LEN, 3);
    assert_eq!(
        HEAD_LEN + TAIL_LEN,
        PAYLOAD_LEN,
        "rien ne se perd au découpage"
    );
    assert_eq!(HEAD_TRANSFER_LEN, 2 + HEAD_LEN);
    assert_eq!(TAIL_TRANSFER_LEN, 2 + TAIL_LEN);

    for jeu in [couleurs_distinctes(), couleurs_relevees()] {
        let (tete, queue) = transfers(&jeu).expect("onze couleurs (spec §4.1)");

        assert_eq!(tete.len(), HEAD_TRANSFER_LEN, "34 octets sur le fil");
        assert_eq!(tete[0], REGISTER_HEAD, "registre du premier bloc");
        assert_eq!(
            tete[1], 0x20,
            "spec §4.4 — SMBHSTDAT0 = 0x20, soit 32 octets"
        );
        assert_eq!(usize::from(tete[1]), HEAD_LEN);

        assert_eq!(queue.len(), TAIL_TRANSFER_LEN, "5 octets sur le fil");
        assert_eq!(queue[0], REGISTER_TAIL, "registre du second bloc");
        assert_eq!(queue[1], 0x03, "spec §4.4 — SMBHSTDAT0 = 0x03");
        assert_eq!(usize::from(queue[1]), TAIL_LEN);

        // La concaténation des deux blocs reconstitue la charge utile, dans l'ordre : c'est la
        // propriété qui garantit qu'aucun octet n'est dupliqué, omis ni interverti entre les
        // deux transferts. Le CRC est le dernier octet du second bloc.
        let mut reconstituee = [0u8; PAYLOAD_LEN];
        reconstituee[..HEAD_LEN].copy_from_slice(&tete[2..]);
        reconstituee[HEAD_LEN..].copy_from_slice(&queue[2..]);
        assert_eq!(reconstituee, charge_utile(&jeu));
        assert_eq!(
            queue[TAIL_TRANSFER_LEN - 1],
            charge_utile(&jeu)[PAYLOAD_LEN - 1],
            "le CRC voyage dans le bloc 0x32"
        );
    }
}

// ---------------------------------------------------------------------------
// 7 — le nombre de couleurs
// ---------------------------------------------------------------------------

#[test]
fn un_nombre_de_couleurs_different_de_onze_est_refuse() {
    // spec §4.1 — la charge utile porte « 11 triplets RGB consécutifs », ni plus ni moins ;
    // §6, implémentation de référence — `assert len(leds) == NB_LED`.
    // Contrat d'API — `payload` et `transfers` rendent un `Result<_, LedCountError>`, et
    // « + Display — le message nomme `LEDS_PER_STICK` et `given` ».
    // Même politique qu'en #5 : aucune valeur par défaut, aucune répétition, aucun complément
    // noir — jamais de charge utile silencieusement fausse.
    assert!(payload(&couleurs(LEDS_PER_STICK)).is_ok());
    assert!(transfers(&couleurs(LEDS_PER_STICK)).is_ok());

    for n in [0usize, 1, 3, 10, 12, 22, 44] {
        let erreur: LedCountError = payload(&couleurs(n)).expect_err("seules onze sont acceptées");
        assert_eq!(erreur.given, n, "l'erreur porte le nombre reçu");

        let message = erreur.to_string();
        assert!(
            message.contains(&LEDS_PER_STICK.to_string()),
            "le message doit dire que {LEDS_PER_STICK} couleurs sont attendues : « {message} »"
        );
        assert!(
            message.contains(&n.to_string()),
            "le message doit dire que {n} ont été reçues : « {message} »"
        );

        let _: &dyn std::error::Error = &erreur;

        // `transfers` refuse au même titre, et avec la même erreur : la validation a lieu avant
        // qu'un seul octet ne soit mis en forme pour le bus.
        let erreur_transferts: LedCountError =
            transfers(&couleurs(n)).expect_err("seules onze sont acceptées");
        assert_eq!(erreur_transferts, erreur);
    }
}

// ---------------------------------------------------------------------------
// 8 — aucune entrée ne produit une adresse de SPD
// ---------------------------------------------------------------------------

// ⚠️ Écart assumé sur ce test, consigné ici plutôt qu'en tête de fichier parce que c'est
// l'emplacement où le test manquant se serait trouvé.
//
// Le critère d'acceptation de l'issue #15 dit « une adresse hors de `0x18`–`0x1b` est impossible
// à construire, pas seulement refusée ». Il suppose un constructeur prenant une adresse ; il n'y
// en a pas (contrat d'API : « Ne se construit **que** depuis un index d'emplacement. Il n'existe
// aucun constructeur prenant une adresse : sur ce bus, `0x50` est le SPD. »). Ce critère est donc
// tenu **par la compilation** : nommer un tel constructeur serait une erreur de compilation, pas
// un échec de test, et l'énumération de la surface publique d'un module ne se fait pas à
// l'exécution. Comme pour #13, cette part de la garantie se vérifie en relecture de la revue.
//
// Ce qui reste testable est plus fort que « `0x50` est refusé » : sur les 256 valeurs de `u8`
// exprimées en `usize`, **aucune ne produit une adresse de SPD**. C'est une preuve exhaustive du
// domaine d'entrée, pas un échantillon.

#[test]
fn aucun_emplacement_ne_produit_jamais_l_adresse_d_un_hub_spd() {
    // spec §3, carte des adresses — `0x50`–`0x53` hubs SPD, `0x48`–`0x4b` PMIC,
    // `0x18`–`0x1b` contrôleurs RGB, « un par slot, dans l'ordre ».
    // issue #15, évaluation du risque — « écrire en `0x50`–`0x53` : corruption du SPD, DIMM non
    // démarrable — rendue impossible par `SlotAddress` ».
    const ADRESSES_RGB: [u8; SLOT_COUNT] = [0x18, 0x19, 0x1a, 0x1b];

    for slot in 0..=255usize {
        match SlotAddress::new(slot) {
            Ok(adresse) => {
                assert!(
                    slot < SLOT_COUNT,
                    "l'emplacement {slot} ne devrait pas être constructible"
                );
                assert!(
                    ADRESSES_RGB.contains(&adresse.address()),
                    "l'emplacement {slot} donne {:#04x}, hors de 0x18–0x1b",
                    adresse.address()
                );
                assert!(
                    !(0x50..=0x53).contains(&adresse.address()),
                    "l'emplacement {slot} vise un hub SPD"
                );
                assert!(
                    !(0x48..=0x4b).contains(&adresse.address()),
                    "l'emplacement {slot} vise un PMIC"
                );
                assert_eq!(adresse.slot(), slot);
            }
            Err(erreur) => {
                assert!(
                    slot >= SLOT_COUNT,
                    "l'emplacement {slot} devrait être constructible"
                );
                assert_eq!(erreur.given, slot, "l'erreur porte l'emplacement demandé");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 9 — les quatre emplacements
// ---------------------------------------------------------------------------

#[test]
fn les_quatre_emplacements_donnent_0x18_a_0x1b_dans_l_ordre() {
    // spec §3 — « `0x18`–`0x1b` : contrôleurs RGB — un par slot, dans l'ordre », recoupé par le
    // journal d'iCUE : `DIMM 0x1800`, `0x1901`, `0x1a02`, `0x1b03` (octet haut = adresse SMBus,
    // octet bas = index de slot).
    // Contrat d'API — « Emplacements numérotés 0 à 3, comme iCUE les journalise », et
    // `SlotAddress::ALL`, « les quatre emplacements, dans l'ordre ».
    assert_eq!(SLOT_COUNT, 4);
    assert_eq!(SlotAddress::ALL.len(), SLOT_COUNT);
    assert_eq!(
        SlotAddress::ALL.map(|a| a.address()),
        [0x18, 0x19, 0x1a, 0x1b],
        "l'ordre compte : c'est lui qui relie un emplacement physique à une adresse"
    );

    for (index, adresse) in SlotAddress::ALL.iter().enumerate() {
        assert_eq!(
            adresse.slot(),
            index,
            "ALL[{index}] est l'emplacement {index}"
        );
        assert_eq!(
            *adresse,
            SlotAddress::new(index).expect("les emplacements 0 à 3 se construisent"),
            "ALL[{index}] et new({index}) désignent la même barrette"
        );
        assert_eq!(
            SlotAddress::new(index)
                .expect("les emplacements 0 à 3 se construisent")
                .address(),
            0x18 + index as u8,
            "spec §3 — un par slot, dans l'ordre"
        );
    }
}
