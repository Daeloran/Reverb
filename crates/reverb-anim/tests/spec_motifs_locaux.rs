//! Tests d'intention des deux directions locales (issue #75, partie A).
//!
//! Écrits **depuis l'issue #75 seule**, avant l'implémentation. Aucune ligne de
//! `crates/reverb-anim/src/` n'a été relue pour les écrire — ni `projection()`, ni le corps d'une
//! famille, ni la table des centres. Seules ont servi les **signatures publiques** nécessaires
//! pour compiler, `docs/GEOMETRIE.md`, et l'exécution de l'API publique telle qu'elle existe
//! aujourd'hui (voir la section « Les empreintes de non-régression » plus bas).
//!
//! Si l'un de ces tests échoue après implémentation, c'est le code qu'on corrige.
//!
//! # L'API que ces tests supposent
//!
//! Rien de ce qui suit n'existe encore : c'est la phase rouge attendue.
//!
//! ```ignore
//! pub enum Direction {
//!     BasHaut, HautBas, AvantArriere, ArriereAvant, Horaire, Antihoraire,
//!     BordsCentre,   // slug « bords-centre » : des deux bords de chaque objet vers son milieu
//!     CentreBords,   // slug « centre-bords » : l'inverse
//! }
//! impl Direction { pub const ALL: [Direction; 8]; }
//! ```
//!
//! Rien d'autre ne change pour cette moitié de l'issue : `Animation::image(&Geometrie,
//! &Reglages, u32) -> Image` garde sa signature, et c'est **exigé** — c'est elle qui rend
//! comparables les images d'avant et d'après (test n° 5 de l'issue).
//!
//! ⚠️ L'autre moitié de l'issue (les quatre familles nouvelles) ajoute un champ `sonde` à
//! `Reglages`, ce qui lui **retire `Copy`**. Ce fichier n'en dépend pas : il construit ses
//! réglages par `..Reglages::default()`, jamais champ par champ.
//!
//! # Ce que ce fichier fige, et pourquoi
//!
//! Une direction locale ne se distingue d'une onde plane par **aucun message d'erreur** : les deux
//! rendent 124 couleurs plausibles. L'issue le dit : « une direction locale silencieusement
//! traitée comme globale donnerait une image plausible et fausse ». Les observables retenus sont
//! donc des **égalités entre objets**, que seule une projection locale peut satisfaire :
//!
//! 1. sous `bords-centre`, une barrette est **symétrique** — sa LED 0 et sa LED 10 portent la
//!    même couleur, à tous les pas, pour les six familles ;
//! 2. sous `bords-centre`, les **quatre barrettes** portent la même image ;
//! 3. sous `bords-centre`, deux ventilateurs **montés pareil** portent la même image, où qu'ils
//!    soient dans le boîtier ;
//! 4. et — c'est le contrôle négatif, sans lequel une implémentation qui rendrait *tout* local
//!    passerait les trois premiers — sous une direction **globale**, ces mêmes objets diffèrent.
//!
//! ## Pourquoi le contrôle négatif ne se fait pas sous `bas-haut`
//!
//! ⚠️ **Le test d'intention n° 4 de l'issue est infaisable tel qu'il est écrit.** Il demande que
//! « sous `bas-haut`, l'image de la barrette 0 diffère de celle de la barrette 3 ». Or les quatre
//! barrettes sont **à la même hauteur** : elles ne diffèrent que par leur profondeur (mesuré :
//! même `x`, même `y`, `z` de 330 à 300 par pas de 10). Sous `bas-haut`, elles portent la même
//! image — et elles la porteraient aussi avec une implémentation parfaitement globale. Le test tel
//! qu'écrit passerait donc **toujours**, y compris sur le défaut qu'il vise.
//!
//! Le seul axe du boîtier qui sépare réellement les quatre barrettes est la profondeur. Le
//! contrôle se fait donc sous **`avant-arriere`**, où les six familles les distinguent — vérifié
//! sur l'implémentation actuelle avant d'écrire ce fichier. L'intention de l'issue est préservée
//! mot pour mot : « la direction locale ne doit pas avoir contaminé les globales ».
//!
//! Même correction pour les ventilateurs : chaque groupe de ventilateurs montés pareil est
//! comparé sous la direction globale qui le sépare, cherchée parmi les six plutôt que choisie
//! d'avance.
//!
//! ## Ce que ces tests ne disent **pas** des ventilateurs
//!
//! Les huit LED d'un ventilateur sont **toutes à la même distance du centre de leur anneau** —
//! c'est un cercle. Une direction locale prise au pied de la lettre les aplatit donc toutes les
//! huit, exactement comme `bas-haut` aplatit un ventilateur couché. `docs/GEOMETRIE.md` nomme le
//! remède déjà en place pour ce cas — la traversée depuis le point d'entrée de l'écoulement — et
//! l'issue ne tranche pas si `bords-centre` doit s'en servir ou faire clignoter l'anneau d'un
//! bloc.
//!
//! Ces tests **laissent donc le choix ouvert** et n'exigent que ce qui vaut dans les deux cas :
//! deux ventilateurs de même montage rendent la même image. C'est la définition même de « local »,
//! et c'est ce que l'issue promet — « le motif se répète à l'identique sur chacun des quatorze
//! objets ».
//!
//! # Les empreintes de non-régression
//!
//! Le test n° 5 de l'issue est un filet, pas une spécification : « les six directions existantes
//! produisent **exactement** les mêmes images qu'avant ». Les valeurs de [`EMPREINTES`] ont été
//! relevées le 2026-08-02 sur `feature/75-animations`, **avant** toute modification, en appelant
//! `Animation::image` — l'API publique, jamais son corps. Elles couvrent, pour chaque couple
//! (famille, direction) : deux géométries, deux couleurs, trois vitesses et dix-huit pas.
//!
//! ⚠️ **Une empreinte qui bouge n'est jamais une empreinte à corriger.** C'est une image qui a
//! changé, donc une régression — la seule issue est de corriger le code. Ces valeurs ne se
//! régénèrent que si Nico décide, en connaissance de cause, que le rendu des six familles doit
//! changer ; ce n'est pas ce que l'issue #75 demande.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi.

use reverb_anim::{Animation, CATALOGUE, Direction, Geometrie, Image, Reglages};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// Le domaine
// ---------------------------------------------------------------------------

/// Les six directions d'avant l'issue #75, écrites en dur.
///
/// ⚠️ **Jamais `Direction::ALL`** : c'est précisément la liste qui s'allonge de deux entrées, et
/// un test de non-régression qui la lirait perdrait son objet le jour où elle change.
const GLOBALES: [Direction; 6] = [
    Direction::BasHaut,
    Direction::HautBas,
    Direction::AvantArriere,
    Direction::ArriereAvant,
    Direction::Horaire,
    Direction::Antihoraire,
];

/// Les deux directions que l'issue ajoute.
const LOCALES: [Direction; 2] = [Direction::BordsCentre, Direction::CentreBords];

/// Les six familles d'avant l'issue #75, écrites en dur pour la même raison que [`GLOBALES`].
const ANCIENNES_FAMILLES: [&str; 6] = [
    "vague",
    "comete",
    "respiration",
    "arc-en-ciel",
    "balayage",
    "braise",
];

/// Durée d'un cycle, en pas, à la vitesse 1 — figée par `spec_sens.rs` (issue #49).
const PERIODE: u32 = 120;

/// Une couleur dont les trois composantes diffèrent deux à deux, comme dans `spec_animations.rs` :
/// le projet mélange trois ordres de composantes et une permutation ne produit aucun message.
const TEMOIN: Rgb = Rgb::new(0xff, 0x20, 0x80);

/// En deçà, le retard mesuré par la phase du fondamental ne veut rien dire — seuil de
/// `spec_sens.rs`.
const COHERENCE_MINIMALE: f64 = 0.5;

/// En deçà, deux LED sont trop proches pour que leur ordre soit lisible : un demi-pas sur cent
/// vingt. Seuil de `spec_sens.rs`, et pour la même raison — c'est le refus d'un retard nul, dont
/// le signe ne voudrait rien dire, pas une exigence sur l'amplitude.
const RETARD_MINIMAL: f64 = 0.5;

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

/// Les animations d'avant l'issue, ouvertes par leur nom.
fn anciennes() -> Vec<Animation> {
    ANCIENNES_FAMILLES
        .iter()
        .map(|nom| {
            Animation::par_nom(nom)
                .unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
        })
        .collect()
}

/// Une paire brute, telle qu'elle arrive du protocole.
fn paire(cle: &str, valeur: &str) -> (String, String) {
    (cle.to_owned(), valeur.to_owned())
}

/// Des réglages explicites, sur la base des valeurs par défaut.
///
/// Passe par `..Reglages::default()` et non par une construction champ par champ : la seconde
/// moitié de l'issue ajoute un champ `sonde`, et ce fichier n'a rien à en dire.
fn reglages(direction: Direction, vitesse: u8) -> Reglages {
    Reglages {
        couleur: TEMOIN,
        vitesse,
        direction,
        ..Reglages::default()
    }
}

/// La géométrie du boîtier réel : c'est celle dont Nico juge le rendu à l'œil.
fn geometrie() -> Geometrie {
    Geometrie::mesuree()
}

/// Une seconde géométrie, sans orientation particulière, construite par le décodeur.
fn geometrie_plate() -> Geometrie {
    let lignes: Vec<String> = Position::ALL
        .iter()
        .map(|position| format!("{} 0 horaire", position.slug()))
        .collect();
    Geometrie::decoder(&lignes.join("\n")).expect("la géométrie de test est valide")
}

/// Les huit couleurs d'un ventilateur dans une image, cherchées **par position**.
///
/// Jamais par indice de tableau : l'`Image` porte la position à côté des couleurs précisément
/// pour que personne n'ait à connaître l'ordre du tableau.
fn ventilateur(image: &Image, position: Position) -> [Rgb; LEDS_PER_FAN as usize] {
    image
        .ventilateurs
        .iter()
        .find(|(p, _)| *p == position)
        .unwrap_or_else(|| panic!("l'image ne contient pas {}", position.slug()))
        .1
}

/// Les groupes de ventilateurs **montés pareil** : même angle, même sens, même point d'entrée de
/// l'écoulement.
///
/// Les angles et les sens viennent de `docs/GEOMETRIE.md` § « Orientation, ventilateur par
/// ventilateur » ; les points d'entrée de § « Par où l'écoulement entre dans un ventilateur »
/// (plancher 6 h, radiateur 6 h, plafond 12 h, fond 12 h).
///
/// ⚠️ `bas-droite` ne rejoint **pas** ses deux voisins du plancher : il est monté d'un quart de
/// tour à côté (210° contre 300°), et la spec insiste — « deux ventilateurs au même endroit ne
/// sont pas montés pareil ». Il ne figure donc dans aucun groupe, et c'est ce qui rend ces groupes
/// vérifiables plutôt que devinés : le premier test les recoupe avec la géométrie.
fn groupes_de_meme_montage() -> Vec<(&'static str, Vec<Position>)> {
    vec![
        (
            "plancher, 300° horaire, entrée 6 h",
            vec![Position::BasGauche, Position::BasMilieu],
        ),
        (
            "radiateur, 210° horaire, entrée 6 h",
            vec![
                Position::RadiateurHaut,
                Position::RadiateurMilieu,
                Position::RadiateurBas,
            ],
        ),
        (
            "plafond, 60° antihoraire, entrée 12 h",
            vec![
                Position::HautGauche,
                Position::HautMilieu,
                Position::HautDroite,
            ],
        ),
    ]
}

// ---------------------------------------------------------------------------
// La mesure d'un retard : phase du fondamental
// ---------------------------------------------------------------------------
//
// Même appareil que `spec_sens.rs` (issue #49), et pour la même raison : `arc-en-ciel` fait
// défiler une teinte à luminance quasi constante, si bien que « l'instant du maximum » ne dit rien
// pour elle. Le produit croisé des fondamentaux rend un retard **continu et signé**, avec sa
// propre mesure de confiance.

/// La série temporelle d'une LED de barrette sur un cycle complet, à la vitesse 1.
fn serie_barrette(
    animation: &Animation,
    geometrie: &Geometrie,
    direction: Direction,
    slot: usize,
    led: usize,
) -> Vec<[f64; 3]> {
    let reglages = reglages(direction, 1);
    (0..PERIODE)
        .map(|pas| {
            let couleur = animation.image(geometrie, &reglages, pas).barrettes[slot][led];
            [
                f64::from(couleur.r),
                f64::from(couleur.g),
                f64::from(couleur.b),
            ]
        })
        .collect()
}

/// La composante de Fourier de rang 1 de chaque canal, en (partie réelle, partie imaginaire).
fn fondamental(serie: &[[f64; 3]]) -> [(f64, f64); 3] {
    let combien = serie.len() as f64;
    let mut spectre = [(0.0f64, 0.0f64); 3];
    for (pas, echantillon) in serie.iter().enumerate() {
        let (sinus, cosinus) = (-std::f64::consts::TAU * pas as f64 / combien).sin_cos();
        for (bin, valeur) in spectre.iter_mut().zip(echantillon.iter()) {
            bin.0 += valeur * cosinus;
            bin.1 += valeur * sinus;
        }
    }
    spectre
}

/// Le retard de `arrivee` sur `depart`, en pas dans `]-60, 60]`, et la confiance à lui accorder.
///
/// Positif : l'arrivée est atteinte **après** le départ.
fn retard(depart: &[[f64; 3]], arrivee: &[[f64; 3]]) -> (f64, f64) {
    let (avant, apres) = (fondamental(depart), fondamental(arrivee));
    let (mut reel, mut imaginaire) = (0.0f64, 0.0f64);
    let (mut energie_depart, mut energie_arrivee) = (0.0f64, 0.0f64);
    for (un, autre) in avant.iter().zip(apres.iter()) {
        reel += un.0 * autre.0 + un.1 * autre.1;
        imaginaire += un.0 * autre.1 - un.1 * autre.0;
        energie_depart += un.0 * un.0 + un.1 * un.1;
        energie_arrivee += autre.0 * autre.0 + autre.1 * autre.1;
    }
    let coherence = if energie_depart > 0.0 && energie_arrivee > 0.0 {
        reel.hypot(imaginaire) / (energie_depart.sqrt() * energie_arrivee.sqrt())
    } else {
        0.0
    };
    (
        -imaginaire.atan2(reel) * f64::from(PERIODE) / std::f64::consts::TAU,
        coherence,
    )
}

// ---------------------------------------------------------------------------
// 1 — huit directions acceptées, une neuvième refusée en les citant
// ---------------------------------------------------------------------------

#[test]
fn les_huit_directions_sont_acceptees_par_les_six_familles() {
    // Test d'intention n° 1 de l'issue, et critère d'acceptation : « `direction=bords-centre` et
    // `direction=centre-bords` sont acceptées par les six familles existantes et par
    // `geometry`/`animate` comme les six autres ».
    //
    // Le fait qu'elles s'ajoutent **sans en retirer** est le fond de l'affaire : « Elles s'ajoutent
    // aux six directions existantes et se combinent avec les six familles ».
    assert_eq!(
        Direction::ALL.len(),
        8,
        "l'issue ajoute deux directions aux six existantes"
    );
    for direction in GLOBALES {
        assert!(
            Direction::ALL.contains(&direction),
            "« {} » ne doit pas disparaître de `Direction::ALL`",
            direction.slug()
        );
    }
    for direction in LOCALES {
        assert!(
            Direction::ALL.contains(&direction),
            "« {} » doit rejoindre `Direction::ALL`",
            direction.slug()
        );
    }

    // Les deux mots exacts, écrits dans l'issue : `animate vague direction=bords-centre`. Ils
    // suivent la règle des slugs du projet — ASCII, sans accent, kebab-case.
    assert_eq!(Direction::BordsCentre.slug(), "bords-centre");
    assert_eq!(Direction::CentreBords.slug(), "centre-bords");

    // Huit slugs distincts : deux directions qui s'écriraient pareil en rendraient une
    // inatteignable depuis le socket.
    let mut slugs: Vec<&str> = Direction::ALL.iter().map(|d| d.slug()).collect();
    slugs.sort_unstable();
    let mut sans_doublon = slugs.clone();
    sans_doublon.dedup();
    assert_eq!(slugs, sans_doublon, "deux directions s'écrivent pareil");

    // Et chacune des huit est acceptée par chacune des six familles, et atterrit dans le champ
    // `direction` — une valeur rangée ailleurs serait un réglage qui ment.
    for animation in anciennes() {
        for direction in Direction::ALL {
            let lus = animation
                .reglages(&[paire("direction", direction.slug())])
                .unwrap_or_else(|erreur| {
                    panic!(
                        "« {} » doit accepter « direction={} » : {erreur}",
                        animation.nom(),
                        direction.slug()
                    )
                });
            assert_eq!(
                lus.direction,
                direction,
                "« {} » : « direction={} » doit atterrir dans le champ `direction`",
                animation.nom(),
                direction.slug()
            );
        }
    }
}

#[test]
fn une_neuvieme_direction_inventee_est_refusee_en_citant_les_huit() {
    // Test d'intention n° 1 de l'issue — « une neuvième inventée est refusée en donnant la liste ».
    //
    // La liste, et non un simple refus : c'est elle qui fait la différence entre un utilisateur qui
    // se corrige seul et un utilisateur qui va lire le code. Le refus d'aujourd'hui la donne déjà
    // (« Directions : bas-haut, haut-bas, … ») ; ce test exige qu'elle **s'allonge** avec le
    // domaine, faute de quoi les deux directions neuves resteraient introuvables.
    for animation in anciennes() {
        for saisi in [
            "bords-milieu",
            "centre-milieu",
            "bordscentre",
            "BORDS-CENTRE",
            "",
        ] {
            let erreur = animation
                .reglages(&[paire("direction", saisi)])
                .expect_err("une direction hors domaine est refusée");
            assert_eq!(
                erreur.cle,
                "direction",
                "« {} » : le refus doit nommer la clé fautive",
                animation.nom()
            );
            assert!(
                erreur.raison.contains(saisi),
                "« {} » : le refus doit citer la valeur refusée « {saisi} » — « {} »",
                animation.nom(),
                erreur.raison
            );
            for direction in Direction::ALL {
                assert!(
                    erreur.raison.contains(direction.slug()),
                    "« {} » : le refus doit citer « {} » parmi les directions valides — « {} »",
                    animation.nom(),
                    direction.slug(),
                    erreur.raison
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 2 — sous `bords-centre`, une barrette est symétrique
// ---------------------------------------------------------------------------

#[test]
fn sous_les_directions_locales_une_barrette_est_symetrique() {
    // Test d'intention n° 2 de l'issue, et critère d'acceptation : « sous `bords-centre`, les deux
    // LED d'extrémité d'une **même** barrette s'allument au même instant ».
    //
    // Exigé sur les onze LED et pas seulement sur la paire 0/10 : la symétrie est ce que le motif
    // d'iCUE fait — « part des deux bords de *chaque* barrette et converge vers son milieu » —, et
    // une implémentation qui n'apparierait que les extrémités ne serait pas ce motif-là.
    //
    // Exigé aussi de `centre-bords` : c'est la même symétrie parcourue à l'envers.
    let geometrie = geometrie();
    for animation in anciennes() {
        for direction in LOCALES {
            let reglages = reglages(direction, 1);
            for pas in 0..PERIODE {
                let image = animation.image(&geometrie, &reglages, pas);
                for slot in 0..SLOT_COUNT {
                    for led in 0..LEDS_PER_STICK {
                        let miroir = LEDS_PER_STICK - 1 - led;
                        assert_eq!(
                            image.barrettes[slot][led],
                            image.barrettes[slot][miroir],
                            "« {} » sous « {} », pas {pas} : la LED {led} et la LED {miroir} de la \
                             barrette {slot} sont à égale distance du milieu, elles doivent \
                             s'allumer ensemble",
                            animation.nom(),
                            direction.slug()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn sous_les_directions_locales_le_milieu_d_une_barrette_ne_suit_pas_ses_bords() {
    // Contrôle du test précédent, et il est indispensable : une animation qui peindrait toute la
    // barrette d'une seule couleur serait parfaitement symétrique, et parfaitement fausse. Il faut
    // donc que le milieu **diffère** des bords à un moment du cycle.
    let geometrie = geometrie();
    for animation in anciennes() {
        for direction in LOCALES {
            let reglages = reglages(direction, 1);
            let differe = (0..PERIODE).any(|pas| {
                let barrette = animation.image(&geometrie, &reglages, pas).barrettes[0];
                barrette[0] != barrette[LEDS_PER_STICK / 2]
            });
            assert!(
                differe,
                "« {} » sous « {} » : le milieu d'une barrette ne se distingue jamais de ses \
                 bords — le motif ne la traverse pas",
                animation.nom(),
                direction.slug()
            );
        }
    }
}

#[test]
fn sous_bords_centre_le_milieu_d_une_barrette_s_allume_en_dernier() {
    // Test d'intention n° 2 de l'issue, seconde moitié : « et le milieu en dernier ».
    //
    // C'est ce qui sépare `bords-centre` de `centre-bords`. Sans lui, échanger les deux
    // directions ne produirait aucun message : deux motifs symétriques, deux images plausibles.
    //
    // L'observable est celui de `spec_sens.rs` (issue #49) — le retard tiré de la phase du
    // fondamental — parce que « l'instant du maximum » ne dit rien d'`arc-en-ciel`, qui fait
    // défiler une teinte à luminance quasi constante.
    //
    // ⚠️ **Le retard se mesure entre LED voisines, jamais du bord au milieu.** Le retard vit
    // modulo la période : à une demi-période exactement, un retard et une avance sont
    // indiscernables, et c'est précisément la valeur que rendrait un motif traversant l'objet en
    // un demi-cycle — un choix d'échelle parfaitement raisonnable. Cinq couples voisins
    // enchaînés disent la même chose sans jamais approcher cette ambiguïté.
    //
    // Une famille dont la sonde n'est pas une onde simple le dit par sa cohérence, au lieu de
    // rendre un chiffre faux avec assurance : ce couple-là est alors écarté du verdict. Mais
    // **chaque famille doit rester exploitable sur au moins un couple**, sans quoi elle
    // échapperait au test en devenant illisible partout — la façon la plus discrète de le
    // désarmer.
    // ⚠️ **`braise` est écartée de ce test, et la mesure le justifie.** Elle superpose deux ondes
    // de sens opposés — l'une descend la direction, l'autre la remonte trois fois plus vite — et
    // son intensité s'écrête. Son fondamental n'est donc pas une phase, c'est du bruit, et le
    // retard qu'on en tire ne veut rien dire.
    //
    // Ce n'est **pas** un effet des directions locales. Mesuré le 2026-08-02 avec cet appareil
    // exact, sur les LED voisines de la barrette 0, à la vitesse 1 :
    //
    // | direction      | 0→1    | 1→2    | 2→3    | 3→4    | 4→5    |
    // |----------------|--------|--------|--------|--------|--------|
    // | `bas-haut`     | +45,29 | +47,87 | +40,01 | **−24,74** | +34,78 |
    // | `horaire`      | −31,97 | −9,28  | +11,96 | −30,98 | +6,63  |
    // | `bords-centre` | +4,02  | −56,32 | +12,68 | +4,02  | −56,32 |
    //
    // Sous `bas-haut` et sous `horaire` — deux directions que #75 ne touche pas — `braise` change
    // déjà de signe d'un couple à l'autre, avec des cohérences de 0,29 à 0,78. Exiger d'elle un
    // retard signé mesurerait donc l'arrondi, pas le motif.
    //
    // Ce que `braise` doit à cette issue reste vérifié par les autres tests de ce fichier : sa
    // symétrie autour du milieu, l'identité de ses quatre barrettes, et le fait que son milieu se
    // distingue de ses bords.
    let geometrie = geometrie();
    for animation in anciennes()
        .into_iter()
        .filter(|animation| animation.nom() != "braise")
    {
        for (direction, attendu) in [
            (Direction::BordsCentre, 1.0),
            (Direction::CentreBords, -1.0),
        ] {
            let series: Vec<Vec<[f64; 3]>> = (0..=LEDS_PER_STICK / 2)
                .map(|led| serie_barrette(&animation, &geometrie, direction, 0, led))
                .collect();
            let mut exploitables = 0;
            for (led, couple) in series.windows(2).enumerate() {
                let (avance, coherence) = retard(&couple[0], &couple[1]);
                if coherence < COHERENCE_MINIMALE {
                    continue;
                }
                assert!(
                    avance.abs() < f64::from(PERIODE) / 2.0 - 5.0,
                    "appareil : « {} » sous « {} », LED {led} → {} : {avance:.2} pas d'écart, \
                     trop près de la demi-période pour que le signe veuille dire quelque chose",
                    animation.nom(),
                    direction.slug(),
                    led + 1
                );
                exploitables += 1;
                assert!(
                    avance * attendu > RETARD_MINIMAL,
                    "« {} » sous « {} » : la LED {} est atteinte {avance:.2} pas après la LED \
                     {led} — sous « bords-centre » le motif va du bord vers le milieu, sous \
                     « centre-bords » l'inverse (cohérence {coherence:.2})",
                    animation.nom(),
                    direction.slug(),
                    led + 1
                );
            }
            assert!(
                exploitables > 0,
                "« {} » sous « {} » : aucun couple de LED voisines n'est lisible — le motif ne \
                 progresse pas le long de la barrette, et « le milieu en dernier » n'y veut rien \
                 dire",
                animation.nom(),
                direction.slug()
            );
        }
    }
}

#[test]
fn bords_centre_et_centre_bords_ne_sont_pas_la_meme_animation() {
    // Garde-fou minimal, complémentaire du test précédent : il ne dépend d'aucune mesure de phase,
    // et attrape le cas grossier où les deux directions seraient le même code.
    let geometrie = geometrie();
    for animation in anciennes() {
        let aller = reglages(Direction::BordsCentre, 3);
        let retour = reglages(Direction::CentreBords, 3);
        let differe = (0..PERIODE).any(|pas| {
            animation.image(&geometrie, &aller, pas) != animation.image(&geometrie, &retour, pas)
        });
        assert!(
            differe,
            "« {} » rend la même image sous « bords-centre » et sous « centre-bords » : les deux \
             directions n'en font qu'une",
            animation.nom()
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — sous `bords-centre`, les quatorze objets portent le même motif
// ---------------------------------------------------------------------------

#[test]
fn sous_les_directions_locales_les_quatre_barrettes_affichent_la_meme_image() {
    // Test d'intention n° 3 de l'issue, et critère d'acceptation : « sous `bords-centre`, les
    // quatre barrettes affichent **la même image**, quelle que soit leur place dans le boîtier —
    // c'est ce qui distingue une direction locale d'une onde plane ».
    let geometrie = geometrie();
    for animation in anciennes() {
        for direction in LOCALES {
            let reglages = reglages(direction, 1);
            for pas in 0..PERIODE {
                let image = animation.image(&geometrie, &reglages, pas);
                for slot in 1..SLOT_COUNT {
                    assert_eq!(
                        image.barrettes[0],
                        image.barrettes[slot],
                        "« {} » sous « {} », pas {pas} : la barrette {slot} ne porte pas le même \
                         motif que la barrette 0 — le motif traverse les quatre barrettes au lieu \
                         de se répéter sur chacune",
                        animation.nom(),
                        direction.slug()
                    );
                }
            }
        }
    }
}

#[test]
fn sous_les_directions_locales_deux_ventilateurs_de_meme_montage_affichent_la_meme_image() {
    // Issue : « Le motif se répète donc à l'identique sur chacun des quatorze objets. »
    //
    // Restreint aux ventilateurs **montés pareil**, et c'est volontaire : l'issue ne tranche pas si
    // l'anneau, que la distance au centre aplatit entièrement, doit clignoter d'un bloc ou être
    // traversé depuis son point d'entrée d'écoulement (`docs/GEOMETRIE.md`). Les deux lectures
    // rendent la même image sur deux ventilateurs de même angle, de même sens et de même entrée ;
    // seule une projection **globale** les distingue, puisqu'elle les lit à leur place dans le
    // boîtier. C'est donc exactement le défaut visé, sans rien exiger de plus.
    let geometrie = geometrie();
    for animation in anciennes() {
        for direction in LOCALES {
            let reglages = reglages(direction, 1);
            for pas in 0..PERIODE {
                let image = animation.image(&geometrie, &reglages, pas);
                for (etiquette, membres) in groupes_de_meme_montage() {
                    let repere = ventilateur(&image, membres[0]);
                    for position in &membres[1..] {
                        assert_eq!(
                            ventilateur(&image, *position),
                            repere,
                            "« {} » sous « {} », pas {pas} : {} ne porte pas le même motif que {} \
                             — ils sont pourtant montés pareil ({etiquette}), et une direction \
                             locale ne lit pas leur place dans le boîtier",
                            animation.nom(),
                            direction.slug(),
                            position.slug(),
                            membres[0].slug()
                        );
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — le contrôle négatif : les globales ne deviennent pas locales
// ---------------------------------------------------------------------------

#[test]
fn sous_une_direction_globale_les_quatre_barrettes_different() {
    // Test d'intention n° 4 de l'issue — « contrôle : la direction locale ne doit pas avoir
    // contaminé les globales ».
    //
    // ⚠️ Sous `avant-arriere` et non sous `bas-haut`, que l'issue nomme : les quatre barrettes
    // sont **à la même hauteur** (mesuré : même `x`, même `y`, `z` de 330 à 300), et sous
    // `bas-haut` elles portent la même image y compris avec une implémentation parfaitement
    // globale. Le test tel qu'écrit dans l'issue passerait toujours, donc ne prouverait rien. La
    // profondeur est le seul axe du boîtier qui les sépare.
    //
    // Sans ce contrôle, une implémentation qui rendrait **tout** local passerait les trois tests
    // précédents et serait fausse.
    let geometrie = geometrie();
    for animation in anciennes() {
        let reglages = reglages(Direction::AvantArriere, 1);
        let differe = (0..PERIODE).any(|pas| {
            let image = animation.image(&geometrie, &reglages, pas);
            image.barrettes[0] != image.barrettes[SLOT_COUNT - 1]
        });
        assert!(
            differe,
            "« {} » sous « avant-arriere » : les barrettes 0 et {} portent la même image à tous \
             les pas — une direction globale doit les lire à leur place dans le boîtier",
            animation.nom(),
            SLOT_COUNT - 1
        );
    }
}

#[test]
fn sous_une_direction_globale_deux_ventilateurs_de_meme_montage_different() {
    // Même contrôle, côté ventilateurs. Deux ventilateurs montés pareil mais posés à deux endroits
    // du boîtier doivent différer sous **au moins une** des six directions globales.
    //
    // La direction n'est pas choisie d'avance mais cherchée parmi les six : le radiateur est
    // empilé verticalement (seul l'axe des hauteurs le sépare), le plancher et le plafond sont
    // alignés d'avant en arrière (seule la profondeur les sépare). Écrire une direction en dur
    // rendrait le contrôle vide pour deux groupes sur trois.
    let geometrie = geometrie();
    for animation in anciennes() {
        for (etiquette, membres) in groupes_de_meme_montage() {
            let separe = GLOBALES.iter().any(|direction| {
                let reglages = reglages(*direction, 1);
                (0..PERIODE).any(|pas| {
                    let image = animation.image(&geometrie, &reglages, pas);
                    let repere = ventilateur(&image, membres[0]);
                    membres[1..]
                        .iter()
                        .any(|position| ventilateur(&image, *position) != repere)
                })
            });
            assert!(
                separe,
                "« {} » : aucune des six directions globales ne distingue les ventilateurs du \
                 groupe « {etiquette} » — une direction globale lit la place dans le boîtier, pas \
                 le montage",
                animation.nom()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — non-régression : les six directions d'origine rendent les mêmes images
// ---------------------------------------------------------------------------

/// Les pas échantillonnés, mêlant petits nombres, bords de période et grands nombres.
const PAS: [u32; 18] = [
    0, 1, 2, 3, 5, 7, 11, 17, 29, 41, 59, 60, 61, 89, 119, 120, 240, 901,
];

/// Les vitesses échantillonnées : les deux bornes du domaine, et la valeur par défaut.
const VITESSES: [u8; 3] = [1, 3, 10];

/// L'empreinte d'une image : FNV-1a 64 bits sur les 372 octets, dans un ordre canonique.
///
/// Les ventilateurs sont parcourus dans l'ordre de `Position::ALL` et cherchés **par position**,
/// jamais par indice de tableau : l'ordre du tableau n'est pas un contrat.
fn empreinte_image(image: &Image) -> u64 {
    let mut valeur: u64 = 0xcbf2_9ce4_8422_2325;
    let mut avale = |octet: u8| {
        valeur ^= u64::from(octet);
        valeur = valeur.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for position in Position::ALL {
        for couleur in ventilateur(image, position) {
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

/// L'empreinte d'un couple (famille, direction) : deux géométries, deux couleurs, trois vitesses,
/// dix-huit pas — 216 images repliées en un seul nombre.
fn empreinte_couple(animation: &Animation, direction: Direction) -> u64 {
    let geometries = [geometrie(), geometrie_plate()];
    let couleurs = [TEMOIN, Rgb::new(0x20, 0xff, 0x80)];
    let mut valeur: u64 = 0xcbf2_9ce4_8422_2325;
    for geometrie in &geometries {
        for couleur in couleurs {
            for vitesse in VITESSES {
                for pas in PAS {
                    let reglages = Reglages {
                        couleur,
                        vitesse,
                        direction,
                        ..Reglages::default()
                    };
                    valeur ^= empreinte_image(&animation.image(geometrie, &reglages, pas));
                    valeur = valeur.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
    }
    valeur
}

/// Ce que les six familles rendaient sous les six directions **avant** l'issue #75.
///
/// Relevé le 2026-08-02 sur `feature/75-animations`, avant toute modification, en appelant
/// `Animation::image` — l'API publique.
const EMPREINTES: [(&str, &str, u64); 36] = [
    ("vague", "bas-haut", 0xc699_f733_4a54_c523),
    ("vague", "haut-bas", 0xd85b_b0e4_ac0e_e055),
    ("vague", "avant-arriere", 0xf5fa_c281_d1bf_d0c5),
    ("vague", "arriere-avant", 0x7d94_87ca_2723_1267),
    ("vague", "horaire", 0xcc1b_97a1_ea8c_54d7),
    ("vague", "antihoraire", 0x68bb_e061_e4f7_f85d),
    ("comete", "bas-haut", 0x812f_2e15_5a81_885f),
    ("comete", "haut-bas", 0xb518_0324_c364_b207),
    ("comete", "avant-arriere", 0x57a9_b809_8de5_9a75),
    ("comete", "arriere-avant", 0xc80c_016f_8414_a8db),
    ("comete", "horaire", 0x1d17_1420_af97_6675),
    ("comete", "antihoraire", 0x4668_704f_60e7_cadf),
    ("respiration", "bas-haut", 0x4d68_cf5c_a79a_a6c9),
    ("respiration", "haut-bas", 0x1d9e_ee21_26e4_e591),
    ("respiration", "avant-arriere", 0x5c1b_2313_6f81_e3bd),
    ("respiration", "arriere-avant", 0xc7e3_acbd_f91a_9241),
    ("respiration", "horaire", 0xa119_2eb6_f117_7cad),
    ("respiration", "antihoraire", 0x515d_1f09_74d5_9d81),
    ("arc-en-ciel", "bas-haut", 0x5f8e_42d2_e409_c731),
    ("arc-en-ciel", "haut-bas", 0x5276_2eae_fec7_0069),
    ("arc-en-ciel", "avant-arriere", 0x31fe_3671_544c_c365),
    ("arc-en-ciel", "arriere-avant", 0x97f3_eba4_dd2e_0bed),
    ("arc-en-ciel", "horaire", 0x7be9_c2c2_3604_ed15),
    ("arc-en-ciel", "antihoraire", 0x696d_82a0_84e5_23fd),
    ("balayage", "bas-haut", 0xa97b_8e05_bc0d_fc97),
    ("balayage", "haut-bas", 0x6043_ab2d_13c1_1fcb),
    ("balayage", "avant-arriere", 0x680a_7d96_a954_be97),
    ("balayage", "arriere-avant", 0xdb16_45a1_580c_dfa1),
    ("balayage", "horaire", 0xf6fd_4fda_69c6_9ec1),
    ("balayage", "antihoraire", 0xb4e8_7b87_0c15_9f7d),
    ("braise", "bas-haut", 0x3665_fd4c_c375_e755),
    ("braise", "haut-bas", 0x1bdb_4aed_f3fb_4701),
    ("braise", "avant-arriere", 0x2c81_662a_d88c_1ce1),
    ("braise", "arriere-avant", 0x5793_5bd1_a1d0_548b),
    ("braise", "horaire", 0x76ae_d47d_ae8e_ab47),
    ("braise", "antihoraire", 0xa621_e65b_646e_6b2d),
];

#[test]
fn les_six_familles_sous_les_six_directions_rendent_exactement_les_memes_images() {
    // Test d'intention n° 5 de l'issue, et critère d'acceptation : « les six directions existantes
    // produisent **exactement** les mêmes images qu'avant ».
    //
    // C'est le filet du chantier. L'approche technique annonce que `projection()` change de
    // signature — « une direction locale ne se projette pas sur les bornes du boîtier » — et une
    // signature qui change est exactement le moment où une globale se décale d'un cheveu sans que
    // personne ne le voie. Six familles × six directions × 216 images : 7 776 images comparées.
    //
    // ⚠️ Si une empreinte bouge, c'est le code qu'on corrige, jamais la table.
    assert_eq!(
        EMPREINTES.len(),
        ANCIENNES_FAMILLES.len() * GLOBALES.len(),
        "la table doit couvrir les six familles sous les six directions"
    );
    for (nom, slug, attendue) in EMPREINTES {
        let animation = Animation::par_nom(nom)
            .unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"));
        let direction = GLOBALES
            .into_iter()
            .find(|d| d.slug() == slug)
            .unwrap_or_else(|| panic!("« {slug} » est une des six directions d'origine"));
        let obtenue = empreinte_couple(&animation, direction);
        assert_eq!(
            obtenue, attendue,
            "« {nom} » sous « {slug} » ne rend plus les mêmes images : empreinte \
             0x{obtenue:016x}, attendue 0x{attendue:016x}. L'issue #75 ajoute des directions, \
             elle n'en modifie aucune."
        );
    }
}

#[test]
fn les_six_familles_d_origine_restent_au_catalogue() {
    // Corollaire du filet : une famille renommée ou retirée ferait échouer le test précédent avec
    // un `panic!` de recherche plutôt qu'un écart d'empreinte. Autant le dire directement.
    for nom in ANCIENNES_FAMILLES {
        assert!(
            CATALOGUE.contains(&nom),
            "« {nom} » doit rester au catalogue : l'issue #75 en ajoute quatre, elle n'en retire \
             aucune"
        );
    }
}
