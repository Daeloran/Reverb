//! Tests d'intention des quatre familles nouvelles (issue #75, partie B).
//!
//! Écrits **depuis l'issue #75 seule**, avant l'implémentation. Aucune ligne de
//! `crates/reverb-anim/src/` n'a été relue pour les écrire ; seules ont servi les signatures
//! publiques nécessaires pour compiler, `docs/GEOMETRIE.md` et `docs/SPEC-PROTOCOLE-NZXT.md`.
//! Si l'un de ces tests échoue après implémentation, c'est le code qu'on corrige.
//!
//! # L'API que ces tests supposent
//!
//! Rien de ce qui suit n'existe encore : c'est la phase rouge attendue.
//!
//! ```ignore
//! pub const CATALOGUE: &[&str] = &[
//!     "vague", "comete", "respiration", "arc-en-ciel", "balayage", "braise",
//!     "rotation", "thermique", "pouls", "scintillement",
//! ];
//!
//! pub struct Reglages {
//!     pub couleur: Rgb,
//!     pub vitesse: u8,
//!     pub direction: Direction,
//!     /// Le `slug` de la sonde à suivre. Seule `thermique` l'accepte, et elle l'exige.
//!     pub sonde: Option<String>,
//! }
//!
//! impl Animation {
//!     /// Inchangée. Équivaut à `image_avec_mesure(…, None)` : une sonde muette.
//!     pub fn image(&self, &Geometrie, &Reglages, pas: u32) -> Image;
//!
//!     /// `mesure` : la valeur relevée par le démon, en degrés, ou `None` si la sonde ne
//!     /// répond pas — écartée par la quarantaine (#68), absente, ou jamais relevée.
//!     pub fn image_avec_mesure(&self, &Geometrie, &Reglages, pas: u32, mesure: Option<f32>) -> Image;
//! }
//!
//! impl Geometrie {
//!     /// Le point d'où part l'onde de `pouls` : le bloc-pompe du Kraken.
//!     pub fn pompe(&self) -> Point;
//! }
//! ```
//!
//! ## Trois arbitrages que l'issue laisse ouverts, et qui sont tranchés ici
//!
//! 1. **La valeur de sonde passe en argument, son nom vit dans `Reglages`.** L'issue laisse le
//!    choix — « soit un paramètre supplémentaire, soit une entrée dans `Reglages` » — mais les
//!    deux réponses ne portent pas sur la même chose. Le **nom** de la sonde est un réglage : il
//!    se tape (`animate thermique sonde=…`), il s'écrit dans `eclairage.conf`, et le critère
//!    d'acceptation exige que `reglages_ecrits` le relise « sans perte au redémarrage ». La
//!    **valeur** n'est pas un réglage : elle change à chaque image, et l'issue exige que la
//!    fonction reste pure — « la sonde est lue par le démon et passée en argument, jamais lue
//!    depuis `reverb-anim` ».
//!
//! 2. **`image()` survit, et vaut « sonde muette ».** Deux raisons. D'abord le test n° 5 de
//!    l'issue, qui compare les images d'avant et d'après : il lui faut la même porte d'entrée.
//!    Ensuite parce que c'est le défaut **sûr** : un appelant qui oublierait de passer la mesure
//!    obtient le blanc pulsant, jamais une température plausible. L'inverse — retomber sur une
//!    valeur par défaut — est exactement le mode de défaillance que l'issue interdit.
//!
//! 3. ⚠️ **`Reglages` perd `Copy`.** `Option<String>` ne l'autorise pas. C'est un coût réel, qui
//!    touche tous les appelants du démon et de la fenêtre, et il est assumé : le seul substitut
//!    qui garderait `Copy` serait une chaîne de taille fixe embarquée dans la structure, pour un
//!    slug qui vient du réseau de sondes et dont personne ne connaît la longueur maximale.
//!
//! ## Ce qui n'est **pas** testable ici, et où ça doit l'être
//!
//! ⚠️ Le critère « `thermique` avec une sonde inconnue est refusée en la nommant » et la moitié
//! « en donnant la liste des sondes » du critère précédent **ne peuvent pas** vivre dans
//! `reverb-anim` : ce crate est pur, il ne connaît aucune sonde et ne doit surtout pas se mettre à
//! en connaître. La liste appartient au démon, qui découvre les seize sondes de `hwmon`. Ces deux
//! refus se testent donc dans `crates/reverb-daemon/tests/`, et ce fichier ne vérifie que ce qui
//! relève du catalogue : la **présence** du réglage `sonde`, et le refus d'une valeur vide.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi.

use std::collections::HashSet;

use reverb_anim::{
    Animation, CATALOGUE, Direction, Geometrie, Image, Orientation, Palette, Point, Reglages, Sens,
};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// Le domaine
// ---------------------------------------------------------------------------

/// Les quatre familles que l'issue ajoute, dans l'ordre du tableau de son § B.
const NOUVELLES: [&str; 4] = ["rotation", "thermique", "pouls", "scintillement"];

/// Les six familles d'avant, pour distinguer ce qui doit changer de ce qui ne doit pas.
const ANCIENNES: [&str; 6] = [
    "vague",
    "comete",
    "respiration",
    "arc-en-ciel",
    "balayage",
    "braise",
];

/// Durée d'un cycle, en pas, à la vitesse 1 — figée par `spec_sens.rs` (issue #49).
const PERIODE: u32 = 120;

/// Une couleur dont les trois composantes diffèrent deux à deux.
const TEMOIN: Rgb = Rgb::new(0xff, 0x20, 0x80);

/// Une sonde plausible, celle que l'issue écrit dans son exemple.
const SONDE: &str = "kraken2023elite:coolant-temp";

/// Écart toléré, par composante, entre deux LED que la géométrie déclare équidistantes.
///
/// Deux unités sur 255. Les couples retenus par le test de `pouls` sont à moins d'un dixième de
/// millimètre l'un de l'autre sur une étendue de plus de cinquante centimètres : leur phase ne
/// peut différer que de l'arrondi vers l'octet.
const TOLERANCE_COULEUR: i32 = 2;

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
}

fn paire(cle: &str, valeur: &str) -> (String, String) {
    (cle.to_owned(), valeur.to_owned())
}

fn geometrie() -> Geometrie {
    Geometrie::mesuree()
}

/// Des réglages sans sonde, sur la base des valeurs par défaut.
fn reglages(vitesse: u8) -> Reglages {
    Reglages {
        couleur: TEMOIN,
        vitesse,
        ..Reglages::default()
    }
}

/// Des réglages de `thermique`, qui exige le nom de sa sonde.
fn reglages_thermique(vitesse: u8) -> Reglages {
    Reglages {
        sonde: Some(SONDE.to_owned()),
        ..reglages(vitesse)
    }
}

/// Les huit couleurs d'un ventilateur dans une image, cherchées **par position**.
fn anneau(image: &Image, position: Position) -> [Rgb; LEDS_PER_FAN as usize] {
    image
        .ventilateurs
        .iter()
        .find(|(p, _)| *p == position)
        .unwrap_or_else(|| panic!("l'image ne contient pas {}", position.slug()))
        .1
}

/// L'anneau décalé de `cran` indices : `decale(a, k)[i] == a[(i + k) % 8]`.
fn decale(source: [Rgb; LEDS_PER_FAN as usize], cran: usize) -> [Rgb; LEDS_PER_FAN as usize] {
    let mut sortie = source;
    for (indice, place) in sortie.iter_mut().enumerate() {
        *place = source[(indice + cran) % LEDS_PER_FAN as usize];
    }
    sortie
}

/// Une empreinte d'image, pour comparer des ensembles d'images sans les garder toutes.
fn empreinte(image: &Image) -> u64 {
    let mut valeur: u64 = 0xcbf2_9ce4_8422_2325;
    let mut avale = |octet: u8| {
        valeur ^= u64::from(octet);
        valeur = valeur.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for position in Position::ALL {
        for couleur in anneau(image, position) {
            avale(couleur.r);
            avale(couleur.g);
            avale(couleur.b);
        }
    }
    for slot in 0..SLOT_COUNT {
        for couleur in image.barrettes[slot] {
            avale(couleur.r);
            avale(couleur.g);
            avale(couleur.b);
        }
    }
    valeur
}

/// Les 124 LED du boîtier, avec l'organe qui les porte, leur nom et leur place.
///
/// L'organe : `0..10` pour les ventilateurs, `10..14` pour les barrettes. Deux LED d'organes
/// distincts sont sur deux objets distincts, au sens de l'issue.
fn toutes_les_led(geometrie: &Geometrie) -> Vec<(usize, String, Point)> {
    let mut led = Vec::new();
    for position in Position::ALL {
        for indice in 0..LEDS_PER_FAN as usize {
            let place = geometrie
                .led_ventilateur(position, indice)
                .unwrap_or_else(|| panic!("{} led {indice} a une place", position.slug()));
            led.push((
                position.index(),
                format!("{} led {indice}", position.slug()),
                place,
            ));
        }
    }
    for slot in 0..SLOT_COUNT {
        for indice in 0..LEDS_PER_STICK {
            let place = geometrie
                .led_barrette(slot, indice)
                .unwrap_or_else(|| panic!("barrette {slot} led {indice} a une place"));
            led.push((
                Position::ALL.len() + slot,
                format!("barrette {slot} led {indice}"),
                place,
            ));
        }
    }
    led
}

/// La couleur d'une LED désignée par son rang dans [`toutes_les_led`].
fn couleur_par_rang(image: &Image, rang: usize) -> Rgb {
    let par_ventilateur = Position::ALL.len() * LEDS_PER_FAN as usize;
    if rang < par_ventilateur {
        let position = Position::ALL[rang / LEDS_PER_FAN as usize];
        anneau(image, position)[rang % LEDS_PER_FAN as usize]
    } else {
        let reste = rang - par_ventilateur;
        image.barrettes[reste / LEDS_PER_STICK][reste % LEDS_PER_STICK]
    }
}

fn distance(un: Point, autre: Point) -> f32 {
    ((un.x - autre.x).powi(2) + (un.y - autre.y).powi(2) + (un.z - autre.z).powi(2)).sqrt()
}

/// Deux couleurs sont-elles indiscernables à l'arrondi près ?
fn proches(un: Rgb, autre: Rgb) -> bool {
    let ecart = |a: u8, b: u8| (i32::from(a) - i32::from(b)).abs();
    ecart(un.r, autre.r) <= TOLERANCE_COULEUR
        && ecart(un.g, autre.g) <= TOLERANCE_COULEUR
        && ecart(un.b, autre.b) <= TOLERANCE_COULEUR
}

// ---------------------------------------------------------------------------
// Le catalogue
// ---------------------------------------------------------------------------

#[test]
fn les_dix_familles_figurent_au_catalogue_et_acceptent_la_vitesse() {
    // Critère d'acceptation : « les quatre familles nouvelles figurent dans `CATALOGUE`,
    // acceptent `vitesse` ». Et les six d'avant y restent : l'issue en ajoute quatre, elle n'en
    // retire aucune.
    for nom in ANCIENNES {
        assert!(
            CATALOGUE.contains(&nom),
            "« {nom} » doit rester au catalogue"
        );
    }
    for nom in NOUVELLES {
        assert!(
            CATALOGUE.contains(&nom),
            "« {nom} » doit rejoindre le catalogue"
        );
    }
    assert_eq!(
        CATALOGUE.len(),
        ANCIENNES.len() + NOUVELLES.len(),
        "le catalogue compte dix familles : {CATALOGUE:?}"
    );

    for nom in NOUVELLES {
        let animation = animation(nom);
        assert_eq!(animation.nom(), nom, "« {nom} » se relit sous son nom");
        assert!(
            animation.parametres_acceptes().contains(&"vitesse"),
            "« {nom} » doit accepter `vitesse` : {:?}",
            animation.parametres_acceptes()
        );
        // `thermique` exige sa sonde en plus : la lui donner ici, sinon c'est ce refus-là qu'on
        // mesurerait, et non l'acceptation de `vitesse`.
        let mut paires = vec![paire("vitesse", "7")];
        if animation.parametres_acceptes().contains(&"sonde") {
            paires.push(paire("sonde", SONDE));
        }
        let lus = animation
            .reglages(&paires)
            .unwrap_or_else(|erreur| panic!("« {nom} » doit accepter « vitesse=7 » : {erreur}"));
        assert_eq!(
            lus.vitesse, 7,
            "« {nom} » : « vitesse=7 » doit atterrir dans le champ `vitesse`"
        );
    }
}

#[test]
fn les_neuf_familles_periodiques_animent_vraiment_le_boitier() {
    // Aucun critère de l'issue ne le dit, et c'est bien le problème : ce test est né du banc
    // d'essai par mutation, où deux variantes fautives ont survécu à tous les autres — un
    // `rotation` dont l'anneau porte huit valeurs distinctes mais **ne tourne pas**, et un `pouls`
    // dont l'onde est bien sphérique mais **ne se propage pas**. Les deux rendaient une image
    // parfaitement conforme à tout le reste de ce fichier, et un boîtier figé.
    //
    // C'est le vocabulaire de l'issue qui les condamne : « chaque anneau **tourne** sur lui-même »,
    // « une onde sphérique naît à la pompe et **se propage** ».
    //
    // ⚠️ **`thermique` en est exclue, et c'est tout son intérêt** : elle est « pilotée par une
    // mesure, pas par le temps ». À température constante, elle doit rester constante — l'inclure
    // ici exigerait exactement le contraire de ce que l'issue demande. Son propre mouvement, le
    // blanc pulsant d'une sonde muette, est vérifié plus bas.
    let geometrie = geometrie();
    for nom in CATALOGUE.iter().filter(|nom| **nom != "thermique") {
        let animation = animation(nom);
        let reglages = reglages(3);
        let distinctes: HashSet<u64> = (0..PERIODE)
            .map(|pas| empreinte(&animation.image(&geometrie, &reglages, pas)))
            .collect();
        assert!(
            distinctes.len() >= 10,
            "« {nom} » ne rend que {} images distinctes sur un cycle entier : le boîtier est figé",
            distinctes.len()
        );
    }
}

#[test]
fn seule_thermique_accepte_le_reglage_sonde() {
    // `sonde` n'a de sens que pour la famille pilotée par une mesure. Une clé acceptée par une
    // famille qui l'ignorerait serait un réglage rangé puis oublié — le pire des trois défauts que
    // `spec_animations.rs` nomme.
    assert!(
        animation("thermique")
            .parametres_acceptes()
            .contains(&"sonde"),
        "« thermique » doit accepter `sonde` : {:?}",
        animation("thermique").parametres_acceptes()
    );
    for nom in CATALOGUE.iter().filter(|nom| **nom != "thermique") {
        let animation = animation(nom);
        assert!(
            !animation.parametres_acceptes().contains(&"sonde"),
            "« {nom} » ne suit aucune mesure : elle ne doit pas accepter `sonde`"
        );
        let erreur = animation
            .reglages(&[paire("sonde", SONDE)])
            .expect_err("une clé non déclarée est refusée");
        assert_eq!(erreur.cle, "sonde", "« {nom} » doit nommer la clé fautive");
    }
}

#[test]
fn les_dix_familles_se_reencodent_et_se_redecodent_sans_perte() {
    // Test d'intention n° 14 de l'issue, et critère d'acceptation : « `reglages_ecrits` les relit
    // sans perte au redémarrage ».
    //
    // C'est ce qui rend vraie la promesse du README — « le boîtier retrouve seul, après un
    // redémarrage, ce qu'il affichait […] une animation avec ses réglages ». Une famille dont un
    // réglage se perdrait au passage par `eclairage.conf` redémarrerait sur autre chose, sans le
    // dire.
    for nom in CATALOGUE {
        let animation = animation(nom);
        let acceptes = animation.parametres_acceptes();

        // Chaque réglage accepté prend une valeur qui n'est **pas** celle par défaut : un
        // aller-retour qui perdrait le champ rendrait la valeur par défaut, et un témoin resté à
        // sa valeur par défaut ne le montrerait pas.
        let mut temoin = Reglages::default();
        if acceptes.contains(&"couleur") {
            temoin.couleur = TEMOIN;
        }
        if acceptes.contains(&"vitesse") {
            temoin.vitesse = 7;
        }
        if acceptes.contains(&"direction") {
            temoin.direction = Direction::BordsCentre;
        }
        if acceptes.contains(&"sonde") {
            temoin.sonde = Some(SONDE.to_owned());
        }

        // ⚠️ **L'aller-retour se vérifie sur les DEUX branches depuis #126**, et
        // c'est la seule chose que cette issue change ici. `couleur` et
        // `palette` sont la première paire de clés du catalogue à s'exclure :
        // une palette *remplace* la couleur, et écrire les deux produirait un
        // `eclairage.conf` que le démon refuse au redémarrage suivant — le
        // défaut de #69.
        //
        // L'invariant d'origine, « tout ce qui est accepté est écrit », valait
        // tant qu'aucune clé n'en chassait une autre. Il devient « tout ce que
        // le témoin **porte** est écrit », et la couverture y **gagne** : les
        // deux branches sont désormais parcourues, là où une seule l'était.
        let sans_palette = temoin.clone();
        let avec_palette = if acceptes.contains(&"palette") {
            let mut autre = temoin.clone();
            autre.palette = Some(Palette::par_nom("light-pink").expect("palette du catalogue"));
            // ⚠️ **La couleur revient à son défaut dans cette branche**, et ce
            // n'est pas un contournement : sous palette, `couleur` n'est **pas
            // écrite**, donc rien ne peut la relire. Exiger qu'elle survive
            // reviendrait à exiger que les deux clés soient persistées, ce que
            // le démon refuse au redémarrage suivant. Ce que le test vérifie
            // ici, c'est que le **reste** traverse sans perte.
            autre.couleur = Reglages::default().couleur;
            Some(autre)
        } else {
            None
        };

        for temoin in std::iter::once(sans_palette).chain(avec_palette) {
            let ecrits = animation.reglages_ecrits(&temoin);
            for cle in acceptes {
                // La clé chassée par l'autre n'est pas attendue : c'est la
                // définition même de l'exclusion.
                let portee = match *cle {
                    "couleur" => temoin.palette.is_none(),
                    "palette" => temoin.palette.is_some(),
                    _ => true,
                };
                assert_eq!(
                    ecrits.iter().any(|(nom_cle, _)| nom_cle == cle),
                    portee,
                    "« {nom} » accepte `{cle}` : il doit l'écrire si et seulement si le témoin le \
                     porte — écrit {ecrits:?}"
                );
            }
            let relus = animation.reglages(&ecrits).unwrap_or_else(|erreur| {
                panic!("« {nom} » doit relire ce qu'elle écrit : {erreur}")
            });
            assert_eq!(
                relus, temoin,
                "« {nom} » ne se relit pas sans perte : écrit {ecrits:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// `rotation` — chaque anneau tourne sur lui-même
// ---------------------------------------------------------------------------

/// Un pas où les huit LED d'un ventilateur portent huit couleurs distinctes.
///
/// Sert d'appareil aux tests d'orientation : sur un anneau où deux LED se ressemblent, un décalage
/// d'un cran ne serait pas identifiable de façon unique.
fn pas_a_huit_couleurs(geometrie: &Geometrie, position: Position) -> Option<u32> {
    let rotation = animation("rotation");
    let reglages = reglages(1);
    (0..PERIODE).find(|pas| {
        let couleurs = anneau(&rotation.image(geometrie, &reglages, *pas), position);
        let mut vues: HashSet<(u8, u8, u8)> = HashSet::new();
        couleurs.iter().all(|c| vues.insert((c.r, c.g, c.b)))
    })
}

#[test]
fn rotation_donne_huit_valeurs_distinctes_sur_un_anneau() {
    // Test d'intention n° 6 de l'issue, et critère d'acceptation : « `rotation` fait tourner
    // chaque anneau sur lui-même : à un instant donné, les huit LED d'un ventilateur portent huit
    // valeurs distinctes du motif ».
    //
    // C'est ce qui la distingue de `horaire`, qui est « une rotation dans le volume, pas d'un
    // anneau » : sous `horaire`, un ventilateur couché prend une seule valeur, puisque ses huit LED
    // occupent le même point du parcours.
    //
    // ⚠️ **Huit valeurs distinctes ne suffisent pas, et le banc d'essai l'a montré** : une
    // variante fautive qui peignait un dégradé fixe sur chaque anneau passait cette moitié du test
    // sans que rien ne tourne. Il y faut la seconde moitié — l'anneau **change** au fil des pas —,
    // sans quoi « chaque anneau tourne sur lui-même » se réduirait à « chaque anneau est dégradé ».
    let geometrie = geometrie();
    let rotation = animation("rotation");
    let reglages = reglages(3);
    for position in Position::ALL {
        assert!(
            pas_a_huit_couleurs(&geometrie, position).is_some(),
            "« rotation » : {} ne porte jamais huit couleurs distinctes sur un cycle entier — son \
             anneau ne tourne pas sur lui-même",
            position.slug()
        );
        let etats: HashSet<Vec<(u8, u8, u8)>> = (0..PERIODE)
            .map(|pas| {
                anneau(&rotation.image(&geometrie, &reglages, pas), position)
                    .iter()
                    .map(|c| (c.r, c.g, c.b))
                    .collect()
            })
            .collect();
        assert!(
            etats.len() >= 8,
            "« rotation » : l'anneau de {} ne prend que {} états sur un cycle entier — il est \
             dégradé, pas tournant",
            position.slug(),
            etats.len()
        );
    }
}

#[test]
fn rotation_suit_l_angle_et_le_sens_releves_de_chaque_ventilateur() {
    // Tests d'intention n° 6 et n° 7 de l'issue, et critère d'acceptation : « `rotation` respecte
    // l'orientation relevée (angle et sens) de chaque ventilateur ».
    //
    // L'observable : **tourner le montage d'un cran doit tourner l'image d'un cran**. Les huit LED
    // sont réparties à 45° exactement (`docs/GEOMETRIE.md`, « Le pas angulaire : régulier ») ;
    // ajouter 45° à l'angle de la LED 1 met donc chaque LED là où sa voisine se trouvait, et
    // l'anneau rendu doit se décaler d'un indice — d'un seul, et dans un sens.
    //
    // ⚠️ **Et le sens décide duquel.** Un montage horaire décale dans un sens, un montage
    // antihoraire dans l'autre : c'est là toute la différence entre « respecte le sens relevé » et
    // « connaît l'angle et ignore le reste ». Une implémentation qui colorerait par le seul
    // **numéro** de LED — ou par la traversée depuis le point d'entrée de l'écoulement, qui ne
    // dépend pas du montage — ne bougerait pas du tout, et le décalage nul est refusé en premier.
    let mut base = geometrie();
    let rotation = animation("rotation");
    let reglages = reglages(1);

    for position in Position::ALL {
        let mut crans = Vec::new();
        for sens in [Sens::Horaire, Sens::Antihoraire] {
            base.definir(
                position,
                Orientation::new(0, sens).expect("0° est un angle valide"),
            );
            let pas = pas_a_huit_couleurs(&base, position).unwrap_or_else(|| {
                panic!(
                    "« rotation » : {} ne porte jamais huit couleurs distinctes",
                    position.slug()
                )
            });
            let avant = anneau(&rotation.image(&base, &reglages, pas), position);

            base.definir(
                position,
                Orientation::new(45, sens).expect("45° est un angle valide"),
            );
            let apres = anneau(&rotation.image(&base, &reglages, pas), position);

            let trouves: Vec<usize> = (0..LEDS_PER_FAN as usize)
                .filter(|cran| decale(avant, *cran) == apres)
                .collect();
            assert_eq!(
                trouves.len(),
                1,
                "« rotation » : {} monté {}, un quart de pas angulaire ne correspond à aucun \
                 décalage unique de l'anneau ({trouves:?}) — l'angle relevé n'est pas suivi",
                position.slug(),
                sens.slug()
            );
            let cran = trouves[0];
            assert_ne!(
                cran,
                0,
                "« rotation » : {} monté {}, tourner le ventilateur de 45° ne change rien à son \
                 image — l'orientation relevée est ignorée",
                position.slug(),
                sens.slug()
            );
            assert!(
                cran == 1 || cran == LEDS_PER_FAN as usize - 1,
                "« rotation » : {} monté {}, 45° de montage décalent l'anneau de {cran} indices au \
                 lieu d'un seul — les huit LED sont pourtant réparties à 45° exactement",
                position.slug(),
                sens.slug()
            );
            crans.push(cran);
        }
        assert_ne!(
            crans[0],
            crans[1],
            "« rotation » : {} décale dans le même sens qu'il soit monté horaire ou antihoraire — \
             le sens relevé est ignoré, et le motif tournera à l'envers sur les trois ventilateurs \
             du plafond",
            position.slug()
        );
    }
}

#[test]
fn rotation_anime_les_barrettes_en_bords_centre() {
    // Critère d'acceptation : « `rotation` anime aussi les barrettes, en bords↔centre », et
    // l'issue explique pourquoi : « un boîtier dont la RAM s'éteint pendant qu'un motif tourne se
    // lirait comme une panne ».
    //
    // Trois exigences distinctes, et il les faut toutes trois : la RAM **s'allume**, elle
    // **bouge**, et elle le fait **en bords↔centre** — symétrique autour de son milieu, et
    // identique sur les quatre barrettes.
    let geometrie = geometrie();
    let rotation = animation("rotation");
    let reglages = reglages(1);
    let images: Vec<Image> = (0..PERIODE)
        .map(|pas| rotation.image(&geometrie, &reglages, pas))
        .collect();

    assert!(
        images
            .iter()
            .any(|image| image.barrettes[0].iter().any(|c| *c != Rgb::BLACK)),
        "« rotation » laisse les barrettes éteintes sur un cycle entier : une RAM noire pendant \
         qu'un motif tourne se lit comme une panne"
    );
    assert!(
        images
            .iter()
            .any(|image| image.barrettes[0] != images[0].barrettes[0]),
        "« rotation » fige les barrettes : elles ne doivent pas rester immobiles pendant que les \
         anneaux tournent"
    );

    for (pas, image) in images.iter().enumerate() {
        for slot in 0..SLOT_COUNT {
            for led in 0..LEDS_PER_STICK {
                let miroir = LEDS_PER_STICK - 1 - led;
                assert_eq!(
                    image.barrettes[slot][led], image.barrettes[slot][miroir],
                    "« rotation », pas {pas} : la barrette {slot} n'est pas symétrique autour de \
                     son milieu — bords↔centre est l'équivalent radial demandé pour un objet \
                     linéaire"
                );
            }
            assert_eq!(
                image.barrettes[slot], image.barrettes[0],
                "« rotation », pas {pas} : la barrette {slot} ne porte pas le même motif que la \
                 barrette 0 — bords↔centre est local à chaque barrette"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// `thermique` — la couleur suit une sonde
// ---------------------------------------------------------------------------

#[test]
fn thermique_sans_sonde_est_refusee_en_nommant_le_parametre_manquant() {
    // Test d'intention n° 8 de l'issue, et critère d'acceptation : « `thermique` sans `sonde=` est
    // refusée **en nommant** le paramètre manquant ».
    //
    // ⚠️ La seconde moitié du critère — « et en donnant la liste des sondes » — n'a pas sa place
    // ici : `reverb-anim` est pur et ne connaît aucune sonde. Elle appartient au démon, qui les
    // découvre. Voir l'en-tête de ce fichier.
    // ⚠️ Les jeux d'essai ne contiennent **que** des clés que `thermique` accepte. Y glisser une
    // clé étrangère ferait nommer celle-là par le refus — ce qui serait juste, et ne dirait rien
    // de la sonde manquante.
    let thermique = animation("thermique");
    for paires in [
        Vec::new(),
        vec![paire("vitesse", "5")],
        vec![paire("vitesse", "10")],
    ] {
        let erreur = thermique
            .reglages(&paires)
            .expect_err("« thermique » sans sonde est refusée");
        assert_eq!(
            erreur.cle, "sonde",
            "le refus doit nommer le paramètre manquant, pas se contenter d'échouer : {erreur}"
        );
        assert!(
            !erreur.raison.is_empty(),
            "le refus doit dire pourquoi : {erreur}"
        );
    }
}

#[test]
fn thermique_a_sonde_vide_est_refusee_en_nommant_la_cle() {
    // Même critère, versant « valeur ». Une chaîne vide n'est le `slug` d'aucune sonde : l'accepter
    // reviendrait à démarrer une animation qui ne pourra jamais rien relever, et donc à afficher un
    // blanc pulsant permanent que rien n'expliquerait.
    let thermique = animation("thermique");
    for saisi in ["", "   "] {
        let erreur = thermique
            .reglages(&[paire("sonde", saisi)])
            .expect_err("une sonde vide est refusée");
        assert_eq!(
            erreur.cle, "sonde",
            "le refus doit nommer la clé fautive : {erreur}"
        );
    }
    // Et un nom plausible passe : le contrôle d'existence est au démon, pas ici.
    let lus = thermique
        .reglages(&[paire("sonde", SONDE)])
        .expect("un slug de sonde plausible est accepté");
    assert_eq!(
        lus.sonde.as_deref(),
        Some(SONDE),
        "« sonde={SONDE} » doit atterrir dans le champ `sonde`"
    );
}

/// La différence moyenne rouge − bleu sur les 124 LED : négative au froid, positive au chaud.
///
/// Une moyenne et non une LED prise au hasard : l'issue ne dit pas si `thermique` peint le boîtier
/// d'une seule couleur ou si elle y met un gradient, et ce test n'a pas à le trancher.
fn chaleur_apparente(image: &Image) -> f64 {
    let mut somme = 0.0f64;
    let mut combien = 0.0f64;
    for position in Position::ALL {
        for couleur in anneau(image, position) {
            somme += f64::from(couleur.r) - f64::from(couleur.b);
            combien += 1.0;
        }
    }
    for slot in 0..SLOT_COUNT {
        for couleur in image.barrettes[slot] {
            somme += f64::from(couleur.r) - f64::from(couleur.b);
            combien += 1.0;
        }
    }
    somme / combien
}

#[test]
fn thermique_suit_le_gradient_du_bleu_au_rouge() {
    // Test d'intention n° 9 de l'issue, et l'énoncé : « Le gradient va **bleu → vert → orange →
    // rouge** sur une plage donnée ».
    //
    // Deux exigences : 25 °C et 55 °C donnent deux couleurs **distinctes**, et dans le **bon
    // ordre** — le froid tire vers le bleu, le chaud vers le rouge. Une implémentation qui aurait
    // le gradient à l'envers afficherait un boîtier bleu sur une pompe en surchauffe, et personne
    // ne verrait l'erreur : c'est une couleur plausible.
    let geometrie = geometrie();
    let thermique = animation("thermique");
    let reglages = reglages_thermique(3);

    let froide = thermique.image_avec_mesure(&geometrie, &reglages, 0, Some(25.0));
    let chaude = thermique.image_avec_mesure(&geometrie, &reglages, 0, Some(55.0));
    assert_ne!(
        froide, chaude,
        "« thermique » rend la même image à 25 °C et à 55 °C : elle ne suit pas sa sonde"
    );
    assert!(
        chaleur_apparente(&froide) < 0.0,
        "« thermique » à 25 °C doit tirer vers le bleu, or rouge − bleu vaut {:.1}",
        chaleur_apparente(&froide)
    );
    assert!(
        chaleur_apparente(&chaude) > 0.0,
        "« thermique » à 55 °C doit tirer vers le rouge, or rouge − bleu vaut {:.1}",
        chaleur_apparente(&chaude)
    );

    // Et le gradient ne revient jamais en arrière entre les deux : bleu, vert, orange, rouge est
    // un chemin, pas un aller-retour.
    let mut precedente = f64::NEG_INFINITY;
    for dixiemes in (200..=650).step_by(25) {
        let degres = f64::from(dixiemes) / 10.0;
        let image = thermique.image_avec_mesure(&geometrie, &reglages, 0, Some(degres as f32));
        let chaleur = chaleur_apparente(&image);
        assert!(
            chaleur >= precedente - 1.0,
            "« thermique » redescend vers le bleu à {degres:.1} °C ({chaleur:.1} après \
             {precedente:.1}) : le gradient doit monter du bleu au rouge"
        );
        precedente = chaleur;
    }
}

/// Les empreintes de tout ce que le gradient peut afficher, sur une plage large et à plusieurs pas.
fn images_du_gradient(geometrie: &Geometrie, reglages: &Reglages) -> HashSet<u64> {
    let thermique = animation("thermique");
    let mut vues = HashSet::new();
    for dixiemes in 50..=1000 {
        let degres = f64::from(dixiemes) / 10.0;
        for pas in [0u32, 1, 7, 30, 59, 60, 61, 119] {
            vues.insert(empreinte(&thermique.image_avec_mesure(
                geometrie,
                reglages,
                pas,
                Some(degres as f32),
            )));
        }
    }
    vues
}

#[test]
fn thermique_sur_sonde_muette_pulse_en_blanc_hors_du_gradient() {
    // Test d'intention n° 10 de l'issue, et son avertissement : « Une sonde écartée par la
    // quarantaine (#68) ne doit **jamais** laisser la dernière couleur en place. […] Le boîtier
    // passe donc en **blanc pulsant** — hors du gradient, impossible à lire comme une
    // température. »
    //
    // C'est le mode de défaillance le plus coûteux du projet, parce qu'il est rassurant : le README
    // le nomme déjà pour le cadran — « un 34 °C figé derrière une pompe arrêtée, c'est un circuit
    // qui chauffe sans que rien ne le signale ».
    //
    // ⚠️ **Trois exigences, et il les faut toutes.** Blanc *fixe* passerait « hors gradient » et
    // serait faux — rien ne distinguerait une sonde muette d'un boîtier volontairement blanc. Un
    // blanc qui pulse mais qu'une température peut produire serait faux aussi.
    let geometrie = geometrie();
    let thermique = animation("thermique");
    let reglages = reglages_thermique(3);

    // 1 — achromatique : trois composantes égales, sur les 124 LED. C'est ce que « blanc » veut
    //     dire, et aucune étape de bleu → vert → orange → rouge ne l'est.
    let mut intensites: HashSet<u8> = HashSet::new();
    for pas in 0..PERIODE {
        let image = thermique.image_avec_mesure(&geometrie, &reglages, pas, None);
        for position in Position::ALL {
            for (indice, couleur) in anneau(&image, position).iter().enumerate() {
                assert!(
                    couleur.r == couleur.g && couleur.g == couleur.b,
                    "« thermique » sur sonde muette, pas {pas} : {} led {indice} vaut \
                     ({}, {}, {}) — le blanc est la seule couleur qu'aucune température ne produit",
                    position.slug(),
                    couleur.r,
                    couleur.g,
                    couleur.b
                );
                intensites.insert(couleur.r);
            }
        }
    }

    // 2 — il **pulse** : deux pas successifs donnent deux intensités différentes.
    assert!(
        intensites.len() > 1,
        "« thermique » sur sonde muette rend un blanc fixe : il doit pulser, sans quoi rien ne le \
         distingue d'un boîtier qu'on a volontairement mis en blanc"
    );
    let successifs = (0..PERIODE - 1).any(|pas| {
        let un = thermique.image_avec_mesure(&geometrie, &reglages, pas, None);
        let autre = thermique.image_avec_mesure(&geometrie, &reglages, pas + 1, None);
        un != autre
    });
    assert!(
        successifs,
        "« thermique » sur sonde muette : aucun couple de pas successifs ne diffère — le blanc ne \
         pulse pas, il clignote au mieux d'un cycle à l'autre"
    );

    // 3 — hors gradient : aucune des images du blanc pulsant n'est produite par une température,
    //     si basse ou si haute soit-elle. C'est ce qui la rend « impossible à lire comme une
    //     température ».
    let gradient = images_du_gradient(&geometrie, &reglages);
    for pas in 0..PERIODE {
        let muette = empreinte(&thermique.image_avec_mesure(&geometrie, &reglages, pas, None));
        assert!(
            !gradient.contains(&muette),
            "« thermique » sur sonde muette, pas {pas} : cette image est aussi celle d'une \
             température du gradient — elle se lirait comme une mesure"
        );
    }
}

#[test]
fn thermique_reprend_le_gradient_des_que_la_sonde_repond() {
    // Critère d'acceptation : « `thermique` sur une sonde écartée affiche le blanc pulsant, et
    // **reprend le gradient** dès que la sonde répond de nouveau ».
    //
    // La reprise est immédiate et sans mémoire : `image_avec_mesure` est pure, donc le pas qui
    // suit une quarantaine levée est déjà coloré par la mesure. Un état interne qui traînerait —
    // une rampe de retour, un lissage — serait exactement ce que l'issue interdit.
    let geometrie = geometrie();
    let thermique = animation("thermique");
    let reglages = reglages_thermique(3);

    for pas in [0u32, 1, 30, 59, 119] {
        let muette = thermique.image_avec_mesure(&geometrie, &reglages, pas, None);
        let revenue = thermique.image_avec_mesure(&geometrie, &reglages, pas, Some(42.0));
        assert_ne!(
            muette, revenue,
            "pas {pas} : « thermique » rend la même chose que la sonde réponde ou non"
        );
        let colorée = (0..SLOT_COUNT).any(|slot| {
            revenue.barrettes[slot]
                .iter()
                .any(|c| c.r != c.g || c.g != c.b)
        }) || Position::ALL.iter().any(|position| {
            anneau(&revenue, *position)
                .iter()
                .any(|c| c.r != c.g || c.g != c.b)
        });
        assert!(
            colorée,
            "pas {pas} : la sonde répond 42 °C et le boîtier reste achromatique — le gradient ne \
             reprend pas"
        );
    }
}

#[test]
fn une_image_sans_mesure_vaut_une_sonde_muette() {
    // Ce que l'en-tête tranche, vérifié plutôt que promis en commentaire : `image()` est le cas
    // « aucune mesure disponible ».
    //
    // C'est le défaut **sûr**. Un appelant qui oublierait de passer la mesure — la fenêtre,
    // l'exemple `densite`, un test — doit obtenir le blanc illisible, jamais une température
    // plausible. Sans cette égalité, `image()` retomberait sur une valeur par défaut, ce qui est
    // précisément le « 34 °C figé » que l'issue interdit.
    let geometrie = geometrie();
    for nom in CATALOGUE {
        let animation = animation(nom);
        let reglages = if nom == &"thermique" {
            reglages_thermique(3)
        } else {
            reglages(3)
        };
        for pas in [0u32, 1, 17, 60, 119, 901] {
            assert_eq!(
                animation.image(&geometrie, &reglages, pas),
                animation.image_avec_mesure(&geometrie, &reglages, pas, None),
                "« {nom} », pas {pas} : `image` doit valoir `image_avec_mesure(…, None)`"
            );
        }
    }
}

#[test]
fn les_neuf_autres_familles_ignorent_la_mesure() {
    // Contrôle du précédent : `thermique` est la seule famille pilotée par une mesure. Une famille
    // qui changerait d'image selon la température serait une surprise que personne n'a demandée —
    // et qui rendrait `vague` non reproductible d'un démarrage à l'autre.
    let geometrie = geometrie();
    for nom in CATALOGUE.iter().filter(|nom| **nom != "thermique") {
        let animation = animation(nom);
        let reglages = reglages(3);
        for pas in [0u32, 1, 17, 60, 119] {
            for mesure in [None, Some(-10.0), Some(25.0), Some(55.0), Some(120.0)] {
                assert_eq!(
                    animation.image(&geometrie, &reglages, pas),
                    animation.image_avec_mesure(&geometrie, &reglages, pas, mesure),
                    "« {nom} », pas {pas} : la mesure {mesure:?} ne doit rien changer"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// `pouls` — une onde sphérique née à la pompe
// ---------------------------------------------------------------------------

/// Les LED classées par leur distance à la pompe, avec leur rang dans [`toutes_les_led`].
fn distances_a_la_pompe(geometrie: &Geometrie) -> Vec<(usize, usize, String, f32)> {
    let pompe = geometrie.pompe();
    toutes_les_led(geometrie)
        .into_iter()
        .enumerate()
        .map(|(rang, (organe, nom, place))| (rang, organe, nom, distance(place, pompe)))
        .collect()
}

#[test]
fn pouls_allume_ensemble_deux_led_a_egale_distance_de_la_pompe() {
    // Test d'intention n° 11 de l'issue, et le tableau du § B : `pouls` est « une onde
    // **sphérique** [qui] naît à la pompe et se propage », dont la nature est « distance 3D, pas
    // projection sur un axe ».
    //
    // Le couple n'est pas écrit en dur : il est **cherché dans la géométrie**, comme le veut la
    // règle de `spec_animations.rs` — « aucun test ne suppose où se trouve une LED ». On retient le
    // couple inter-organes dont les deux distances à la pompe sont les plus proches ; sur la
    // géométrie mesurée, quelle que soit la place de la pompe, il en existe à moins d'un dixième de
    // millimètre — vérifié sur sept positions de pompe balayant tout le volume avant d'écrire ce
    // test.
    let geometrie = geometrie();
    let led = distances_a_la_pompe(&geometrie);

    let mut serre: Option<(usize, usize, String, String, f32)> = None;
    for (rang_un, organe_un, nom_un, distance_un) in &led {
        for (rang_autre, organe_autre, nom_autre, distance_autre) in &led {
            if rang_autre <= rang_un || organe_un == organe_autre {
                continue;
            }
            let ecart = (distance_un - distance_autre).abs();
            if serre.as_ref().is_none_or(|(_, _, _, _, vu)| ecart < *vu) {
                serre = Some((
                    *rang_un,
                    *rang_autre,
                    nom_un.clone(),
                    nom_autre.clone(),
                    ecart,
                ));
            }
        }
    }
    let (rang_un, rang_autre, nom_un, nom_autre, ecart) =
        serre.expect("le boîtier porte des LED sur quatorze objets distincts");
    assert!(
        ecart < 1.0,
        "appareil : le couple inter-organes le plus serré est à {ecart:.3} de la même distance de \
         la pompe ({nom_un} / {nom_autre}) — trop lâche pour conclure quoi que ce soit"
    );

    let pouls = animation("pouls");
    let reglages = reglages(1);
    for pas in 0..PERIODE {
        let image = pouls.image(&geometrie, &reglages, pas);
        let un = couleur_par_rang(&image, rang_un);
        let autre = couleur_par_rang(&image, rang_autre);
        assert!(
            proches(un, autre),
            "« pouls », pas {pas} : {nom_un} et {nom_autre} sont à {ecart:.3} près de la même \
             distance de la pompe, et portent ({}, {}, {}) contre ({}, {}, {}) — l'onde n'est pas \
             sphérique",
            un.r,
            un.g,
            un.b,
            autre.r,
            autre.g,
            autre.b
        );
    }
}

#[test]
fn pouls_n_est_la_projection_d_aucun_axe() {
    // Contrôle du test précédent, et il est indispensable : une image uniforme satisferait
    // l'égalité de deux LED équidistantes sans rien devoir à la pompe. L'issue insiste sur ce
    // point — « distance 3D, pas projection sur un axe » — parce qu'une onde plane est exactement
    // ce que les six familles savent déjà faire.
    //
    // Pour chaque axe du boîtier, on prend le couple inter-organes qui **partage exactement cette
    // coordonnée** et dont les distances à la pompe s'écartent le plus. Une projection sur cet axe
    // les rendrait identiques à tous les pas ; une onde sphérique ne le peut pas.
    let geometrie = geometrie();
    let led = distances_a_la_pompe(&geometrie);
    let places = toutes_les_led(&geometrie);
    let pouls = animation("pouls");
    let reglages = reglages(1);

    for (axe, coordonnee) in [
        ("x", (|place: Point| place.x) as fn(Point) -> f32),
        ("y", |place: Point| place.y),
        ("z", |place: Point| place.z),
    ] {
        let mut large: Option<(usize, usize, String, String, f32)> = None;
        for (rang_un, organe_un, nom_un, distance_un) in &led {
            for (rang_autre, organe_autre, nom_autre, distance_autre) in &led {
                if rang_autre <= rang_un
                    || organe_un == organe_autre
                    || coordonnee(places[*rang_un].2) != coordonnee(places[*rang_autre].2)
                {
                    continue;
                }
                let ecart = (distance_un - distance_autre).abs();
                if large.as_ref().is_none_or(|(_, _, _, _, vu)| ecart > *vu) {
                    large = Some((
                        *rang_un,
                        *rang_autre,
                        nom_un.clone(),
                        nom_autre.clone(),
                        ecart,
                    ));
                }
            }
        }
        let (rang_un, rang_autre, nom_un, nom_autre, ecart) = large.unwrap_or_else(|| {
            panic!("appareil : aucun couple inter-organes ne partage la même coordonnée {axe}")
        });
        assert!(
            ecart > 10.0,
            "appareil : sur l'axe {axe}, le couple retenu n'est qu'à {ecart:.1} d'écart de \
             distance à la pompe — trop peu pour distinguer une sphère d'un plan"
        );

        let differe = (0..PERIODE).any(|pas| {
            let image = pouls.image(&geometrie, &reglages, pas);
            couleur_par_rang(&image, rang_un) != couleur_par_rang(&image, rang_autre)
        });
        assert!(
            differe,
            "« pouls » : {nom_un} et {nom_autre} partagent la même coordonnée {axe} et sont à \
             {ecart:.1} d'écart de la pompe, pourtant ils portent la même couleur à tous les pas — \
             l'onde est projetée sur l'axe {axe} au lieu d'être sphérique"
        );
    }
}

// ---------------------------------------------------------------------------
// `scintillement` — non périodique, et pourtant déterministe
// ---------------------------------------------------------------------------

#[test]
fn scintillement_rend_la_meme_image_au_meme_pas() {
    // Test d'intention n° 12 de l'issue, et critère d'acceptation : « `scintillement` est
    // **déterministe** : même pas, même image — `image()` est une fonction pure, et c'est ce qui
    // rend le catalogue testable sans matériel ».
    //
    // C'est aussi ce qui rend vraie la promesse du README : « l'aperçu montre ce que le boîtier
    // reçoit » est vrai par construction parce que la fenêtre affiche les images du démon. Une
    // source d'aléa non déterministe — l'horloge, un compteur global, un générateur ensemencé au
    // démarrage — le rendrait faux sans le dire.
    let geometrie = geometrie();
    let scintillement = animation("scintillement");
    let reglages = reglages(3);
    for pas in [0u32, 1, 42, 119, 120, 901] {
        assert_eq!(
            scintillement.image(&geometrie, &reglages, pas),
            scintillement.image(&geometrie, &reglages, pas),
            "« scintillement » : deux appels au pas {pas} rendent deux images différentes"
        );
    }
}

#[test]
fn scintillement_ne_garde_aucun_etat_entre_deux_appels() {
    // Le même critère, mais qu'un aléa ensemencé une fois passerait sans lui : deux appels
    // consécutifs à la même microseconde peuvent coïncider par hasard, un compteur global non.
    //
    // On intercale donc mille images entre les deux relevés, et on repasse par une animation
    // fraîchement ouverte. L'issue nomme la seule mise en œuvre acceptable : « `scintillement`
    // dérive son aléa d'un hachage du numéro de pas. Pas de `rand`, pas d'état. »
    let geometrie = geometrie();
    let reglages = reglages(3);
    let attendue = animation("scintillement").image(&geometrie, &reglages, 42);

    for pas in 0..1000u32 {
        let _ = animation("scintillement").image(&geometrie, &reglages, pas);
    }
    assert_eq!(
        animation("scintillement").image(&geometrie, &reglages, 42),
        attendue,
        "« scintillement » au pas 42 dépend de ce qui a été rendu avant : elle garde un état, ou \
         tire son aléa d'ailleurs que du numéro de pas"
    );

    // Et l'image ne dépend pas non plus de l'ordre dans lequel les pas sont demandés : le démon
    // saute des pas quand une image est en retard.
    let a_rebours: Vec<Image> = (0..8u32)
        .rev()
        .map(|pas| animation("scintillement").image(&geometrie, &reglages, pas))
        .collect();
    for (rang, image) in a_rebours.iter().rev().enumerate() {
        assert_eq!(
            *image,
            animation("scintillement").image(&geometrie, &reglages, rang as u32),
            "« scintillement » : le pas {rang} dépend de l'ordre dans lequel on le demande"
        );
    }
}

#[test]
fn scintillement_change_d_image_d_un_pas_a_l_autre() {
    // Test d'intention n° 13 de l'issue, et son avertissement : « un déterminisme qui produirait
    // une image fixe serait conforme et inutile ».
    //
    // Les deux tests vont par paire et ne valent rien l'un sans l'autre : le déterminisme seul est
    // satisfait par une image noire constante, le changement seul par `SystemTime::now()`.
    let geometrie = geometrie();
    let scintillement = animation("scintillement");
    let reglages = reglages(3);
    assert_ne!(
        scintillement.image(&geometrie, &reglages, 42),
        scintillement.image(&geometrie, &reglages, 43),
        "« scintillement » rend la même image aux pas 42 et 43 : rien ne scintille"
    );

    // Et ce n'est pas un battement entre deux images : sur deux cents pas, il en faut beaucoup
    // plus que deux. Le seuil est bas à dessein — il refuse un clignotement, il n'impose pas une
    // loi de tirage.
    let distinctes: HashSet<u64> = (0..200u32)
        .map(|pas| empreinte(&scintillement.image(&geometrie, &reglages, pas)))
        .collect();
    assert!(
        distinctes.len() > 100,
        "« scintillement » ne rend que {} images distinctes sur deux cents pas : c'est un \
         clignotement, pas un scintillement",
        distinctes.len()
    );
}

#[test]
fn scintillement_n_allume_qu_une_partie_du_boitier() {
    // Le tableau du § B : « des LED s'allument au hasard ». Au hasard, donc pas toutes, et pas
    // aucune — un boîtier entièrement allumé ou entièrement éteint changerait bien d'image d'un pas
    // à l'autre sans scintiller pour autant.
    let geometrie = geometrie();
    let scintillement = animation("scintillement");
    let reglages = reglages(3);
    let mixte = (0..PERIODE).any(|pas| {
        let image = scintillement.image(&geometrie, &reglages, pas);
        let mut couleurs: HashSet<(u8, u8, u8)> = HashSet::new();
        for position in Position::ALL {
            for couleur in anneau(&image, position) {
                couleurs.insert((couleur.r, couleur.g, couleur.b));
            }
        }
        for slot in 0..SLOT_COUNT {
            for couleur in image.barrettes[slot] {
                couleurs.insert((couleur.r, couleur.g, couleur.b));
            }
        }
        couleurs.len() > 1
    });
    assert!(
        mixte,
        "« scintillement » peint les 124 LED d'une seule couleur à chaque pas : rien n'y scintille"
    );
}
