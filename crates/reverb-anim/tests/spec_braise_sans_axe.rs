//! Tests d'intention de « braise sans axe » (issue #119).
//!
//! Écrits **depuis l'issue #119 seule**, avant la correction. Aucune ligne de
//! `crates/reverb-anim/src/` n'a été relue pour les écrire ; seuls ont servi les tests d'intention
//! déjà présents dans ce répertoire — pour les idiomes d'appel du catalogue —, `README.md` et
//! `docs/GEOMETRIE.md`. Si l'un de ces tests échoue après correction, c'est le code qu'on corrige.
//!
//! # Le défaut
//!
//! `braise` superpose deux ondes planes qui défilent **le long de la direction demandée**. Leurs
//! deux périodes temporelles sont incommensurables — d'où « l'œil n'y voit pas de cycle » —, mais
//! spatialement ce sont deux sinusoïdes propres le long du même axe, et le motif se voit. Un lit de
//! braises n'a pas d'axe.
//!
//! # Ce que ce fichier fige
//!
//! 1. **Le rendu ne dépend d'aucune direction**, LED par LED et instant par instant. C'est le test
//!    central de l'issue, et le seul qui condamne à lui seul le code d'aujourd'hui.
//! 2. **`direction` n'est plus acceptée**, et le refus nomme la clé. Rupture de compatibilité
//!    assumée par l'issue : un profil qui porte cette clé cessera de s'appliquer.
//! 3. **Le motif est corrélé, pas granulaire** : deux LED voisines se ressemblent plus que deux LED
//!    opposées. C'est ce qui distingue un feu d'un grésillement, et c'est le premier des deux
//!    écueils que l'issue nomme — « ne pas devenir `scintillement` ».
//! 4. **Ce n'est pas non plus `pouls`** — second écueil nommé : une onde qui ne dépendrait que de la
//!    distance à la pompe donnerait des coquilles concentriques.
//! 5. **Le rendu est reproductible et sans état**, sans quoi « l'aperçu montre ce que le boîtier
//!    reçoit » deviendrait faux sans le dire.
//! 6. **Aucun axe du boîtier ne porte d'onde plane** — voir la réserve ci-dessous, qui est la seule
//!    de ce fichier.
//! 7. **Le motif ne se referme pas sur la période du catalogue.**
//! 8. **`braise` teinte la couleur donnée**, elle n'en produit aucune autre.
//!
//! # Quatre rouges, six garde-fous
//!
//! Doivent échouer avant correction : les points 1, 2, 3 et 7 — ce sont eux qui décrivent le défaut.
//! Les points 4, 5, 6 et 8 passent avant comme après, ainsi que le test d'appareil : ce sont des
//! garde-fous, et leur fonction est d'empêcher que la correction n'échange un défaut contre l'un des
//! deux écueils que l'issue nomme, ou ne perde en chemin une propriété qui a déjà servi (point 8).
//!
//! ## Ce qu'aucun de ces tests ne sait voir, et pourquoi il faut le dire
//!
//! ⚠️ Le critère « aucun axe du boîtier ne porte de gradient systématique » est **le seul de l'issue
//! qui ne se laisse pas mesurer à un seuil défendable**, et ce n'est pas faute d'avoir essayé.
//! Trois observables ont été calculés sur la géométrie mesurée, à la vitesse 1, sur les 124 LED et
//! vingt-quatre instants, pour les dix familles sous les huit directions :
//!
//! | observable | `vague` (onde plane) | `braise` d'aujourd'hui | une braise radiale plausible |
//! |---|---|---|---|
//! | part de dispersion expliquée par l'axe (η²) | 0,79 à 0,91 | **0,54** | **0,45** |
//! | corrélation linéaire position ↔ intensité | 0,72 | **0,52** | **0,43** |
//! | advection du profil le long de l'axe | 0,82 | **0,37** | **0,38** |
//!
//! Les deux dernières colonnes se recouvrent sur les trois observables. La raison est structurelle :
//! `braise` ne défile pas sur une coordonnée géométrique mais sur l'**écoulement**, qui étale à
//! dessein les ventilateurs que la direction aplatit — c'est ce qui casse la signature d'onde plane
//! —, tandis qu'un champ construit sur la distance à la pompe corrèle avec la hauteur autant que
//! lui, la pompe étant à l'intérieur du boîtier. `spec_motifs_locaux.rs` (#75) l'avait déjà relevé
//! par l'autre bout : l'appareil de mesure du sens de défilement **écarte `braise`**, dont la
//! cohérence tombe à 0,15 et dont le retard change de signe d'un couple de LED à l'autre.
//!
//! Le test du point 6 conserve donc l'observable η², mais avec un seuil placé entre les deux
//! **populations mesurées** plutôt qu'entre les deux implémentations : il refuse le régime d'onde
//! plane, il ne prétend pas départager mieux. ⚠️ **Il ne rattraperait donc pas une correction qui se
//! contenterait de figer une direction en dur** au lieu d'en sortir — le point 1 non plus, puisque
//! les huit rendus seraient alors bel et bien identiques. C'est la limite de ce fichier, et elle est
//! nommée ici plutôt que découverte au boîtier.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi.

use std::collections::HashSet;

use reverb_anim::{Animation, Direction, Geometrie, Image, Point, Reglages};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// Le domaine
// ---------------------------------------------------------------------------

/// Durée d'un cycle, en pas, à la vitesse 1 — figée par `spec_sens.rs` (issue #49), qui la vérifie
/// plutôt que de la supposer.
const PERIODE: u32 = 120;

/// Les huit directions, écrites en dur pour n'en oublier aucune.
///
/// Le tableau ne se déduit pas du catalogue : après correction, `braise` ne les nomme plus, et un
/// test qui lirait la liste chez elle ne vérifierait plus rien du jour où elle l'aurait vidée.
const DIRECTIONS: [Direction; 8] = [
    Direction::BasHaut,
    Direction::HautBas,
    Direction::AvantArriere,
    Direction::ArriereAvant,
    Direction::Horaire,
    Direction::Antihoraire,
    Direction::BordsCentre,
    Direction::CentreBords,
];

/// Le blanc, pour lire une intensité sur toute l'échelle.
///
/// `braise` teinte la couleur qu'on lui donne : sous `ff2080`, l'intensité d'une LED ne parcourt
/// qu'un peu plus de la moitié de 0..255, et tous les écarts mesurés plus bas seraient comprimés
/// d'autant. Le blanc les rend lisibles tels quels, en unités de composante.
const BLANC: Rgb = Rgb::new(0xff, 0xff, 0xff);

/// Une couleur dont les trois composantes diffèrent deux à deux, comme dans `spec_animations.rs`.
const TEMOIN: Rgb = Rgb::new(0xff, 0x20, 0x80);

/// Un bleu pur : deux composantes à zéro, qui doivent le rester à toute intensité.
const BLEU: Rgb = Rgb::new(0x00, 0x00, 0xff);

// ---------------------------------------------------------------------------
// Les seuils, et d'où ils viennent
// ---------------------------------------------------------------------------

/// Deux LED sont **voisines** en deçà de cette part de l'étendue du boîtier.
///
/// Un huitième de 657 mm, soit 79 mm. À cette échelle, deux LED sont sur le même objet — deux LED
/// contiguës d'un anneau de 55 mm de rayon sont à 42 mm l'une de l'autre, deux LED contiguës d'une
/// barrette à bien moins — ou sur deux objets jointifs. La définition est **spatiale et non par
/// indice** : c'est la règle de `spec_animations.rs`, « aucun test ne suppose où se trouve une
/// LED », et c'est aussi la seule qui ait un sens pour un motif qui ignore la numérotation.
const PART_VOISINE: f32 = 0.12;

/// Deux LED sont **éloignées** au-delà de cette part de l'étendue du boîtier.
///
/// La moitié de la diagonale : deux organes que rien ne réunit, aux deux bouts du volume.
const PART_LOINTAINE: f32 = 0.5;

/// L'écart moyen entre voisines, rapporté à l'écart moyen entre lointaines, ne doit pas le dépasser.
///
/// Le seuil sépare **deux populations mesurées** sur le catalogue, à la vitesse 1, sur vingt-quatre
/// instants et les huit directions :
///
/// | famille | rapport |
/// |---|---|
/// | `scintillement` — chaque LED indépendante | **1,16** |
/// | `rotation` | 0,98 |
/// | `braise` d'aujourd'hui, sous `bords-centre` | **1,12** |
/// | les familles à onde (`vague`, `comete`, `respiration`, `balayage`) | 0,06 à 0,42 |
/// | un champ radial corrélé, prototypé pour vérifier que le seuil est tenable | 0,56 à 0,59 |
///
/// Trois quarts tombent entre les deux, et plus près de la granularité que de l'onde : il s'agit de
/// refuser le grésillement, pas d'imposer la douceur d'une onde plane. Les marges sont de 23 % au-
/// dessus du prototype et de 23 % au-dessous de `rotation`, la plus basse des granulaires.
///
/// ⚠️ Le premier test d'appareil le vérifie sur `scintillement` plutôt que sur cette table : un
/// seuil qui ne saurait plus voir l'extrême qu'il est censé refuser ne dirait plus rien.
const RAPPORT_MAXIMAL: f64 = 0.75;

/// En deçà, le boîtier est trop uniforme pour que le rapport ci-dessus veuille dire quoi que ce
/// soit — 0 / 0 satisferait n'importe quel rapport.
///
/// Huit unités de composante sur 255, soit 3 % de l'échelle : sous cette valeur, deux LED aux deux
/// bouts du boîtier ne se distinguent pas à l'œil. Mesuré, `braise` rend 64 à 87 sur cette même
/// grandeur : le plancher est un garde-fou d'appareil, pas une exigence de contraste.
const PLANCHER_CONTRASTE: f64 = 8.0;

/// Part de la dispersion des intensités qu'un seul axe du boîtier a le droit d'expliquer.
///
/// Mesuré sur la géométrie réelle, vitesse 1, vingt-quatre instants, huit tranches d'effectif égal :
/// le régime d'onde plane — `vague`, qui ne suit que la position — porte **0,79 à 0,91** sur l'axe
/// qu'elle suit, et tout le reste du catalogue reste **sous 0,54**. Le seuil se pose entre ces deux
/// populations, à 20 % de chacune.
///
/// ⚠️ Il ne sépare **pas** le `braise` d'aujourd'hui d'une braise corrigée : voir l'en-tête du
/// fichier, où les trois observables essayés sont donnés avec leurs mesures. Ce test refuse l'onde
/// plane, et rien de plus fin.
const PART_D_AXE_MAXIMALE: f64 = 0.65;

/// Le nombre de tranches d'un profil le long d'un axe.
///
/// Huit tranches pour 124 LED, soit quinze ou seize par tranche : assez fin pour qu'une onde plane
/// se lise, assez large pour que le seul hasard n'explique que 7/123 ≈ 0,06 de la dispersion.
const TRANCHES: usize = 8;

/// Le nombre de périodes du catalogue observées pour établir que le motif ne se referme pas.
const PERIODES_OBSERVEES: u32 = 20;

/// Combien d'images distinctes il faut trouver, sur autant d'instants séparés d'une période exacte.
///
/// La moitié. Une seule suffirait à prouver l'apériodicité — le rendu d'aujourd'hui n'en rend
/// qu'**une** —, mais un motif qui ne bougerait qu'une fois sur vingt se refermerait tout de même à
/// l'œil. La moitié demande une dérive continue sans exiger que les vingt diffèrent, ce qui
/// interdirait à deux instants de se croiser par hasard sur 124 LED arrondies à l'octet.
const DISTINCTES_MINIMALES: usize = 10;

/// Les instants d'observation des mesures de population : un cycle entier, un pas sur cinq.
///
/// Vingt-quatre images. La moyenne sur les instants est ce qui distingue une **propriété** d'un
/// **tirage** : deux voisines peuvent se trouver de part et d'autre d'une frontière chaude/froide,
/// et c'est même ce qu'on veut d'un lit de braises.
fn instants() -> Vec<u32> {
    (0..PERIODE).step_by(5).collect()
}

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

fn braise() -> Animation {
    Animation::par_nom("braise")
        .unwrap_or_else(|erreur| panic!("« braise » est au catalogue : {erreur}"))
}

fn paire(cle: &str, valeur: &str) -> (String, String) {
    (cle.to_owned(), valeur.to_owned())
}

fn geometrie() -> Geometrie {
    Geometrie::mesuree()
}

/// Des réglages explicites, sur la base des valeurs par défaut.
///
/// ⚠️ Construits **par structure et non par `reglages()`** : après correction, `braise` refuse la
/// clé `direction`, et ces tests doivent pourtant pouvoir lui en imposer une pour vérifier qu'elle
/// n'a aucun effet. Le champ, lui, ne disparaît pas — les autres familles s'en servent.
fn reglages(couleur: Rgb, vitesse: u8, direction: Direction) -> Reglages {
    Reglages {
        couleur,
        vitesse,
        direction,
        ..Reglages::default()
    }
}

/// Les huit couleurs d'un ventilateur dans une image, cherchées **par position** et jamais par
/// indice de tableau.
fn anneau(image: &Image, position: Position) -> [Rgb; LEDS_PER_FAN as usize] {
    image
        .ventilateurs
        .iter()
        .find(|(p, _)| *p == position)
        .unwrap_or_else(|| panic!("l'image ne contient pas {}", position.slug()))
        .1
}

/// Les 124 LED du boîtier, avec l'organe qui les porte, leur nom et leur place.
///
/// L'organe : `0..10` pour les ventilateurs, `10..14` pour les barrettes. Deux LED d'organes
/// distincts sont sur deux objets distincts.
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

/// L'intensité perçue d'une LED, entre 0 et 255.
///
/// La moyenne des trois composantes : `braise` teinte une couleur, donc les trois varient
/// proportionnellement et leur moyenne suit l'intensité du motif. Sous [`BLANC`], elle **est**
/// cette intensité, en unités de composante.
fn intensite(couleur: Rgb) -> f64 {
    (f64::from(couleur.r) + f64::from(couleur.g) + f64::from(couleur.b)) / 3.0
}

/// L'écriture d'une couleur sur le socket : six chiffres hexadécimaux minuscules.
fn hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

fn distance(un: Point, autre: Point) -> f32 {
    ((un.x - autre.x).powi(2) + (un.y - autre.y).powi(2) + (un.z - autre.z).powi(2)).sqrt()
}

/// La plus grande distance entre deux LED du boîtier — l'échelle à laquelle tout le reste se
/// rapporte, calculée et jamais supposée.
fn etendue(places: &[Point]) -> f32 {
    let mut large = 0.0f32;
    for (rang, un) in places.iter().enumerate() {
        for autre in &places[rang + 1..] {
            large = large.max(distance(*un, *autre));
        }
    }
    large
}

/// Une population de couples de LED, désignées par leur rang dans [`toutes_les_led`].
type Population = Vec<(usize, usize)>;

/// Les couples de LED voisines et les couples de LED éloignées, cherchés dans la géométrie.
fn couples(places: &[Point]) -> (Population, Population) {
    let etendue = etendue(places);
    let mut voisines = Vec::new();
    let mut lointaines = Vec::new();
    for (rang, un) in places.iter().enumerate() {
        for (decalage, autre) in places[rang + 1..].iter().enumerate() {
            let ecart = distance(*un, *autre);
            if ecart <= PART_VOISINE * etendue {
                voisines.push((rang, rang + 1 + decalage));
            } else if ecart >= PART_LOINTAINE * etendue {
                lointaines.push((rang, rang + 1 + decalage));
            }
        }
    }
    (voisines, lointaines)
}

/// Les intensités des 124 LED, pour chacun des instants donnés.
fn intensites(
    animation: &Animation,
    geometrie: &Geometrie,
    reglages: &Reglages,
    instants: &[u32],
) -> Vec<Vec<f64>> {
    let combien = Position::ALL.len() * LEDS_PER_FAN as usize + SLOT_COUNT * LEDS_PER_STICK;
    instants
        .iter()
        .map(|pas| {
            let image = animation.image(geometrie, reglages, *pas);
            (0..combien)
                .map(|rang| intensite(couleur_par_rang(&image, rang)))
                .collect()
        })
        .collect()
}

/// L'écart moyen d'intensité sur une population de couples, moyenné sur les instants.
fn ecart_moyen(intensites: &[Vec<f64>], couples: &[(usize, usize)]) -> f64 {
    let par_image: f64 = intensites
        .iter()
        .map(|image| {
            couples
                .iter()
                .map(|(un, autre)| (image[*un] - image[*autre]).abs())
                .sum::<f64>()
                / couples.len() as f64
        })
        .sum();
    par_image / intensites.len() as f64
}

/// La part de la dispersion des intensités qu'explique à elle seule la position le long d'un axe.
///
/// C'est le rapport de corrélation η² : la variance des moyennes par tranche, rapportée à la
/// variance de l'ensemble. Une valeur proche de 1 dit que connaître la tranche suffit à connaître
/// l'intensité — c'est la définition d'un motif qu'on lit le long de cet axe. Une valeur proche de
/// 0 dit que l'axe n'explique rien.
///
/// ⚠️ Les tranches sont d'**effectif égal** et non de largeur égale : les hauteurs du boîtier sont
/// très groupées — dix ventilateurs sur quatre niveaux —, et des tranches de largeur égale en
/// laisseraient de vides tout en en surchargeant d'autres.
///
/// Rend `None` quand toutes les LED portent la même intensité : le rapport n'est alors pas défini,
/// et l'instant ne dit rien plutôt que de mentir.
fn part_expliquee(coordonnees: &[f32], intensites: &[f64]) -> Option<f64> {
    let mut couples: Vec<(f32, f64)> = coordonnees
        .iter()
        .copied()
        .zip(intensites.iter().copied())
        .collect();
    couples.sort_by(|un, autre| un.0.total_cmp(&autre.0));

    let combien = couples.len() as f64;
    let moyenne = couples.iter().map(|(_, i)| *i).sum::<f64>() / combien;
    let dispersion = couples
        .iter()
        .map(|(_, i)| (i - moyenne).powi(2))
        .sum::<f64>()
        / combien;
    if dispersion < 1e-9 {
        return None;
    }
    let expliquee = couples
        .chunks(couples.len().div_ceil(TRANCHES))
        .map(|tranche| {
            let locale = tranche.iter().map(|(_, i)| *i).sum::<f64>() / tranche.len() as f64;
            tranche.len() as f64 * (locale - moyenne).powi(2)
        })
        .sum::<f64>()
        / combien;
    Some(expliquee / dispersion)
}

/// Les axes du boîtier, avec leur nom et la coordonnée de chaque LED le long d'eux.
///
/// Trois axes géométriques — `x` d'un flanc à l'autre, `y` du plancher au plafond, `z` de l'avant
/// vers l'arrière (`spec_disposition.rs`, issue #27) — et l'**azimut**, l'angle autour de la
/// verticale passant par le barycentre des 124 LED. Sans ce quatrième, `horaire` et `antihoraire`
/// n'auraient aucun axe où se lire.
///
/// ⚠️ Les deux directions **locales** n'y figurent pas, et ce n'est pas un oubli : leur « axe » est
/// la distance au milieu de l'objet qui porte la LED, donc quatorze axes et non un. Ce qu'elles
/// produiraient de reconnaissable — un motif identique répété sur chaque objet — se lit dans le
/// test des voisines, où `braise` sous `bords-centre` est justement la mesure la plus granulaire du
/// catalogue.
fn axes(places: &[Point]) -> Vec<(&'static str, Vec<f32>)> {
    let combien = places.len() as f32;
    let cx = places.iter().map(|place| place.x).sum::<f32>() / combien;
    let cz = places.iter().map(|place| place.z).sum::<f32>() / combien;
    vec![
        (
            "x — d'un flanc à l'autre",
            places.iter().map(|p| p.x).collect(),
        ),
        (
            "y — du plancher au plafond",
            places.iter().map(|p| p.y).collect(),
        ),
        (
            "z — de l'avant vers l'arrière",
            places.iter().map(|p| p.z).collect(),
        ),
        (
            "azimut — autour de la verticale",
            places.iter().map(|p| (p.z - cz).atan2(p.x - cx)).collect(),
        ),
    ]
}

/// La plus grande part expliquée par un axe, sur les instants donnés, avec l'axe qui la porte.
fn part_d_axe_maximale(
    intensites: &[Vec<f64>],
    axes: &[(&'static str, Vec<f32>)],
) -> (&'static str, f64) {
    let mut pire = ("aucun axe", 0.0f64);
    for (nom, coordonnees) in axes {
        let mut somme = 0.0;
        let mut combien = 0.0;
        for image in intensites {
            if let Some(part) = part_expliquee(coordonnees, image) {
                somme += part;
                combien += 1.0;
            }
        }
        if combien > 0.0 && somme / combien > pire.1 {
            pire = (nom, somme / combien);
        }
    }
    pire
}

// ---------------------------------------------------------------------------
// 1 — le rendu ne dépend d'aucune direction
// ---------------------------------------------------------------------------

#[test]
fn braise_rend_la_meme_image_sous_les_huit_directions() {
    // Test d'intention n° 1 de l'issue, et son premier critère d'acceptation : « le rendu de
    // `braise` ne dépend d'aucune direction ».
    //
    // C'est le test central du chantier, et le seul qui condamne à lui seul le code d'aujourd'hui :
    // `braise` y est « deux ondes planes superposées qui défilent le long d'un axe », et l'axe est
    // celui que la direction demande. Le défaut a été signalé devant le boîtier — « le sens de
    // l'effet a un effet sur l'animation et du coup on voit clairement un pattern ».
    //
    // La comparaison est **LED par LED et instant par instant**, et à deux vitesses : une égalité
    // qui ne tiendrait qu'à la vitesse par défaut laisserait passer un motif dont la direction ne
    // resurgirait qu'une fois le curseur poussé.
    let geometrie = geometrie();
    let braise = braise();
    let led = toutes_les_led(&geometrie);

    for vitesse in [1u8, 7] {
        let temoin = reglages(TEMOIN, vitesse, DIRECTIONS[0]);
        for pas in (0..PERIODE).step_by(3).chain([200, 901]) {
            let attendue = braise.image(&geometrie, &temoin, pas);
            for direction in DIRECTIONS.into_iter().skip(1) {
                let rendue = braise.image(&geometrie, &reglages(TEMOIN, vitesse, direction), pas);
                if rendue == attendue {
                    continue;
                }
                let (rang, nom) = led
                    .iter()
                    .enumerate()
                    .find(|(rang, _)| {
                        couleur_par_rang(&rendue, *rang) != couleur_par_rang(&attendue, *rang)
                    })
                    .map(|(rang, (_, nom, _))| (rang, nom.clone()))
                    .expect("deux images qui diffèrent diffèrent sur au moins une LED");
                let ici = couleur_par_rang(&rendue, rang);
                let la = couleur_par_rang(&attendue, rang);
                panic!(
                    "« braise » suit encore un axe : à la vitesse {vitesse}, au pas {pas}, {nom} \
                     porte ({}, {}, {}) sous « {} » et ({}, {}, {}) sous « {} » — un lit de braises \
                     n'a pas de direction",
                    ici.r,
                    ici.g,
                    ici.b,
                    direction.slug(),
                    la.r,
                    la.g,
                    la.b,
                    DIRECTIONS[0].slug(),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 2 — `direction` n'est plus au contrat
// ---------------------------------------------------------------------------

#[test]
fn braise_n_accepte_plus_direction_et_le_refus_nomme_la_cle() {
    // Test d'intention n° 2 de l'issue, et critère d'acceptation : « `animate braise direction=…`
    // est refusé, en nommant la clé ».
    //
    // Les deux moitiés comptent, et pour deux raisons différentes. `parametres_acceptes` est « la
    // seule source de vérité du refus » (#20) et ce que **la fenêtre lit** pour décider d'afficher
    // ou non le menu des directions : l'y laisser afficherait un menu dont chaque choix ferait
    // rejeter l'`animate` entier. Le refus nommé, lui, est ce qui rend la rupture de compatibilité
    // lisible — un profil enregistré qui porte cette clé doit dire pourquoi il ne s'applique plus.
    //
    // ⚠️ `sens` est refusé au même titre : `spec_animations.rs` le traite comme l'autre graphie de
    // la même clé, et un contrat qui n'en fermerait qu'une porte n'en fermerait aucune.
    let braise = braise();
    let acceptes = braise.parametres_acceptes();

    for cle in ["direction", "sens"] {
        assert!(
            !acceptes.contains(&cle),
            "« braise » ne suit plus aucun axe : elle ne doit plus accepter `{cle}` — {acceptes:?}"
        );
        for direction in DIRECTIONS {
            let erreur = braise
                .reglages(&[paire(cle, direction.slug())])
                .expect_err("une clé que « braise » ne déclare plus est refusée");
            assert_eq!(
                erreur.cle,
                cle,
                "« {cle}={} » : le refus doit nommer la clé fautive, pas se contenter d'échouer — \
                 {erreur}",
                direction.slug()
            );
            assert!(
                !erreur.raison.is_empty(),
                "le refus doit dire pourquoi : {erreur}"
            );
        }
    }

    // Et les deux réglages qu'elle garde sont bien là : l'issue retire une clé, elle n'en retire
    // pas trois. Le README les annonce toujours au tableau des dix familles.
    for cle in ["couleur", "vitesse"] {
        assert!(
            acceptes.contains(&cle),
            "« braise » doit continuer d'accepter `{cle}` : {acceptes:?}"
        );
    }
    let lus = braise
        .reglages(&[paire("couleur", "3939ff"), paire("vitesse", "2")])
        .unwrap_or_else(|erreur| panic!("« braise » accepte couleur et vitesse : {erreur}"));
    assert_eq!(lus.couleur, Rgb::new(0x39, 0x39, 0xff));
    assert_eq!(lus.vitesse, 2);
}

// ---------------------------------------------------------------------------
// 3 — un motif corrélé, pas granulaire
// ---------------------------------------------------------------------------

#[test]
fn l_appareil_distingue_le_gresillement_de_l_onde() {
    // Test d'appareil, vert par nature. Une mesure faite sur un appareil faux ne dit rien, et ne le
    // signale pas — c'est la règle de `spec_sens.rs`, qui s'appareille avant de conclure.
    //
    // Trois vérifications : les deux populations de couples existent et sont grandes, l'étendue est
    // celle d'un boîtier, et surtout **le rapport sait voir l'extrême qu'il refuse** —
    // `scintillement`, dont l'issue dit qu'il ne faut pas devenir, doit le dépasser franchement.
    // Sans cette dernière, un seuil devenu aveugle passerait pour un seuil respecté.
    let geometrie = geometrie();
    let places: Vec<Point> = toutes_les_led(&geometrie)
        .into_iter()
        .map(|(_, _, place)| place)
        .collect();
    let (voisines, lointaines) = couples(&places);

    assert_eq!(places.len(), 124, "le boîtier porte 124 LED");
    assert!(
        voisines.len() > 100 && lointaines.len() > 100,
        "appareil : {} couples voisins et {} couples lointains — trop peu pour qu'une moyenne soit \
         une propriété plutôt qu'un tirage",
        voisines.len(),
        lointaines.len()
    );

    let scintillement =
        Animation::par_nom("scintillement").expect("« scintillement » est au catalogue");
    let mesures = intensites(
        &scintillement,
        &geometrie,
        &reglages(BLANC, 1, DIRECTIONS[0]),
        &instants(),
    );
    let rapport = ecart_moyen(&mesures, &voisines) / ecart_moyen(&mesures, &lointaines);
    assert!(
        rapport > RAPPORT_MAXIMAL,
        "appareil : « scintillement » — la famille où chaque LED est indépendante — rend un rapport \
         de {rapport:.2}, sous le seuil de {RAPPORT_MAXIMAL:.2} qui doit précisément le refuser : \
         la mesure ne voit plus la granularité"
    );

    let vague = Animation::par_nom("vague").expect("« vague » est au catalogue");
    let axes = axes(&places);
    let pire = DIRECTIONS
        .into_iter()
        .map(|direction| {
            let mesures = intensites(
                &vague,
                &geometrie,
                &reglages(BLANC, 1, direction),
                &instants(),
            );
            part_d_axe_maximale(&mesures, &axes).1
        })
        .fold(0.0f64, f64::max);
    assert!(
        pire > PART_D_AXE_MAXIMALE,
        "appareil : « vague » — l'onde plane du catalogue, celle qui ne suit que la position — ne \
         porte que {pire:.2} sur son meilleur axe, sous le seuil de {PART_D_AXE_MAXIMALE:.2} qui \
         doit précisément la refuser : la mesure ne voit plus le régime d'onde plane"
    );
}

#[test]
fn braise_est_un_motif_correle_et_non_un_gresillement() {
    // Test d'intention n° 3 de l'issue, et ses deux critères d'acceptation : « deux LED voisines
    // ont des intensités proches — le motif est corrélé, pas granulaire », « deux LED éloignées se
    // décorrèlent ».
    //
    // C'est ce qui distingue un feu d'un grésillement, et c'est le premier des deux écueils que
    // l'issue nomme : « ne pas devenir `scintillement` — chaque LED indépendante donne du bruit,
    // pas un feu. C'est la famille voisine, et elle existe déjà. »
    //
    // ⚠️ **La mesure porte sur une population, jamais sur un couple.** Deux LED voisines peuvent se
    // trouver de part et d'autre d'une frontière chaude/froide — c'est même ce qu'on veut d'un lit
    // de braises. Ce sont donc des milliers de couples, sur vingt-quatre instants, et c'est la
    // moyenne qui porte la propriété.
    //
    // ⚠️ **Sous les huit directions**, et c'est ce qui le rend rouge aujourd'hui : sous
    // `bords-centre`, `braise` rend 1,12 — plus granulaire que `rotation`, presque autant que
    // `scintillement` —, parce que le motif entier s'y replie sur les onze LED d'une barrette.
    let geometrie = geometrie();
    let braise = braise();
    let places: Vec<Point> = toutes_les_led(&geometrie)
        .into_iter()
        .map(|(_, _, place)| place)
        .collect();
    let (voisines, lointaines) = couples(&places);

    for direction in DIRECTIONS {
        let mesures = intensites(
            &braise,
            &geometrie,
            &reglages(BLANC, 1, direction),
            &instants(),
        );
        let entre_lointaines = ecart_moyen(&mesures, &lointaines);
        assert!(
            entre_lointaines >= PLANCHER_CONTRASTE,
            "« braise » sous « {} » : deux LED aux deux bouts du boîtier ne diffèrent en moyenne \
             que de {entre_lointaines:.1} sur 255 — le boîtier est uni, il n'y a pas de motif à \
             mesurer",
            direction.slug()
        );
        let entre_voisines = ecart_moyen(&mesures, &voisines);
        let rapport = entre_voisines / entre_lointaines;
        assert!(
            rapport <= RAPPORT_MAXIMAL,
            "« braise » sous « {} » : deux voisines diffèrent de {entre_voisines:.1} quand deux \
             lointaines diffèrent de {entre_lointaines:.1}, soit un rapport de {rapport:.2} pour \
             {RAPPORT_MAXIMAL:.2} au plus — chaque LED vit sa vie, c'est un grésillement et non un \
             feu",
            direction.slug()
        );
    }
}

#[test]
fn braise_n_est_pas_une_onde_plane_le_long_d_un_axe() {
    // Test d'intention n° 4 de l'issue : « aucun des axes du boîtier ne montre de corrélation
    // systématique entre position et intensité ».
    //
    // ⚠️ **Garde-fou, et non rouge** — l'en-tête du fichier donne les trois observables essayés et
    // leurs mesures. Aucun ne sépare le `braise` d'aujourd'hui d'une braise radiale plausible, parce
    // que le défaut d'aujourd'hui ne vit pas sur une coordonnée géométrique mais sur l'écoulement.
    // Ce que ce test refuse, et il le refuse franchement, c'est le **régime d'onde plane** : celui
    // de `vague`, qui ne suit que la position, et que mesure le test d'appareil ci-dessus.
    //
    // Sa fonction est donc celle d'un filet : une correction qui remplacerait la projection sur la
    // direction demandée par une projection sur un axe figé y tomberait, et c'est exactement le
    // raccourci qu'une lecture rapide de l'issue peut suggérer.
    let geometrie = geometrie();
    let braise = braise();
    let places: Vec<Point> = toutes_les_led(&geometrie)
        .into_iter()
        .map(|(_, _, place)| place)
        .collect();
    let axes = axes(&places);

    for direction in DIRECTIONS {
        let mesures = intensites(
            &braise,
            &geometrie,
            &reglages(BLANC, 1, direction),
            &instants(),
        );
        let (axe, part) = part_d_axe_maximale(&mesures, &axes);
        assert!(
            part <= PART_D_AXE_MAXIMALE,
            "« braise » sous « {} » : la seule position le long de l'axe « {axe} » explique \
             {part:.2} de la dispersion des intensités, pour {PART_D_AXE_MAXIMALE:.2} au plus — \
             connaître la tranche suffit à connaître la couleur, c'est une onde plane et elle se \
             lit",
            direction.slug()
        );
    }
}

#[test]
fn braise_n_est_pas_une_onde_spherique_autour_de_la_pompe() {
    // Second écueil nommé par l'issue : « ne pas devenir `pouls` — une onde qui ne dépendrait que
    // de la distance à la pompe donnerait des coquilles concentriques, ce qui est exactement l'autre
    // famille voisine ».
    //
    // ⚠️ **Garde-fou, vert avant comme après.** Il n'existe que parce que la matière première que
    // l'issue met en avant — `rayon`, la distance à la pompe — est précisément celle de `pouls` : le
    // raccourci est à portée de main, et il rendrait une seconde famille identique à la première
    // sans qu'aucun autre test de ce fichier ne s'en aperçoive.
    //
    // L'observable est l'exact inverse de celui de `pouls` (#75, `spec_familles_nouvelles.rs`) : le
    // couple inter-organes le plus serré en distance à la pompe — moins d'un dixième de millimètre
    // sur une étendue de plus d'un demi-mètre — doit **différer** à quelques instants au moins.
    // Deux LED que rien ne distingue qu'une distance égale à la pompe.
    let geometrie = geometrie();
    let braise = braise();
    let led = toutes_les_led(&geometrie);
    let pompe = geometrie.pompe();

    let mut serre: Option<(usize, usize, String, String, f32)> = None;
    for (rang_un, (organe_un, nom_un, place_un)) in led.iter().enumerate() {
        for (decalage, (organe_autre, nom_autre, place_autre)) in
            led[rang_un + 1..].iter().enumerate()
        {
            if organe_un == organe_autre {
                continue;
            }
            let ecart = (distance(*place_un, pompe) - distance(*place_autre, pompe)).abs();
            if serre.as_ref().is_none_or(|(_, _, _, _, vu)| ecart < *vu) {
                serre = Some((
                    rang_un,
                    rang_un + 1 + decalage,
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

    let reglages = reglages(BLANC, 1, DIRECTIONS[0]);
    let differe = (0..PERIODE).any(|pas| {
        let image = braise.image(&geometrie, &reglages, pas);
        couleur_par_rang(&image, rang_un) != couleur_par_rang(&image, rang_autre)
    });
    assert!(
        differe,
        "« braise » : {nom_un} et {nom_autre} sont à {ecart:.3} près à la même distance de la pompe, \
         et portent la même couleur à tous les pas d'un cycle entier — c'est « pouls », pas un lit \
         de braises"
    );
}

// ---------------------------------------------------------------------------
// 4 — reproductible, sans état
// ---------------------------------------------------------------------------

#[test]
fn braise_rend_la_meme_image_au_meme_pas() {
    // Test d'intention n° 5 de l'issue, et son avertissement : « aucun `rand`, aucune horloge,
    // aucun état — le rendu doit être identique dans la fenêtre et dans le démon, sans quoi
    // "l'aperçu montre ce que le boîtier reçoit" deviendrait faux sans le dire ».
    //
    // ⚠️ **Garde-fou, vert avant comme après** — `braise` est déjà pure. Il existe parce que la
    // correction demandée introduit un aléa par LED, et que c'est exactement le moment où l'on
    // atteint un générateur. C'est la règle que `scintillement` respecte déjà, et son test le plus
    // proche est repris ici mot pour mot dans l'esprit (#75).
    let geometrie = geometrie();
    let braise = braise();
    let reglages = reglages(TEMOIN, 3, DIRECTIONS[0]);
    for pas in [0u32, 1, 42, 119, 120, 901] {
        assert_eq!(
            braise.image(&geometrie, &reglages, pas),
            braise.image(&geometrie, &reglages, pas),
            "« braise » : deux appels au pas {pas} rendent deux images différentes"
        );
    }
}

#[test]
fn braise_ne_garde_aucun_etat_entre_deux_appels() {
    // Le même critère, mais qu'un aléa ensemencé une fois passerait sans lui : deux appels
    // consécutifs à la même microseconde peuvent coïncider par hasard, un compteur global non.
    //
    // Mille images sont donc intercalées entre les deux relevés, et chaque relevé repasse par une
    // animation fraîchement ouverte. Puis les pas sont redemandés **à rebours** : le démon en saute
    // quand une image est en retard, et un motif qui avancerait par incréments s'en apercevrait.
    let geometrie = geometrie();
    let reglages = reglages(TEMOIN, 3, DIRECTIONS[0]);
    let attendue = braise().image(&geometrie, &reglages, 42);

    for pas in 0..1000u32 {
        let _ = braise().image(&geometrie, &reglages, pas);
    }
    assert_eq!(
        braise().image(&geometrie, &reglages, 42),
        attendue,
        "« braise » au pas 42 dépend de ce qui a été rendu avant : elle garde un état, ou tire son \
         aléa d'ailleurs que du numéro de pas"
    );

    let a_rebours: Vec<Image> = (0..8u32)
        .rev()
        .map(|pas| braise().image(&geometrie, &reglages, pas))
        .collect();
    for (rang, image) in a_rebours.iter().rev().enumerate() {
        assert_eq!(
            *image,
            braise().image(&geometrie, &reglages, rang as u32),
            "« braise » : le pas {rang} dépend de l'ordre dans lequel on le demande"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — le motif ne se referme pas
// ---------------------------------------------------------------------------

#[test]
fn braise_ne_se_referme_pas_sur_la_periode_du_catalogue() {
    // Test d'intention n° 6 de l'issue : « sur une longue suite d'instants, le motif ne se répète
    // pas à la période du catalogue ».
    //
    // C'est la moitié de la promesse d'origine que le code ne tenait qu'en apparence. Le commentaire
    // annonçait « deux ondes de périodes incommensurables : l'œil n'y voit pas de cycle », et c'était
    // vrai **dans une période** — 3 et 7 ne se referment pas l'une sur l'autre — mais faux d'une
    // période à la suivante : `temps` boucle, donc tout recommence à l'identique toutes les cent
    // vingt étapes. L'issue nomme le remède : « `derive` et non `temps` ».
    //
    // ⚠️ **Vingt instants séparés d'une période exacte**, et non deux : deux images peuvent
    // coïncider par hasard sur 124 LED arrondies à l'octet, une dérive continue ne peut pas
    // coïncider vingt fois.
    let geometrie = geometrie();
    let braise = braise();
    let reglages = reglages(TEMOIN, 1, DIRECTIONS[0]);

    for depart in [0u32, 7, 61] {
        let distinctes: HashSet<Vec<(u8, u8, u8)>> = (0..PERIODES_OBSERVEES)
            .map(|tour| {
                let image = braise.image(&geometrie, &reglages, depart + tour * PERIODE);
                (0..124)
                    .map(|rang| {
                        let couleur = couleur_par_rang(&image, rang);
                        (couleur.r, couleur.g, couleur.b)
                    })
                    .collect()
            })
            .collect();
        assert!(
            distinctes.len() >= DISTINCTES_MINIMALES,
            "« braise » : sur {PERIODES_OBSERVEES} instants séparés d'une période exacte à partir du \
             pas {depart}, elle ne rend que {} image(s) distincte(s) — il en faut \
             {DISTINCTES_MINIMALES}. Le motif se referme sur le cycle du catalogue, et un feu n'a \
             pas de cycle",
            distinctes.len()
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — `braise` teinte, elle ne colore pas
// ---------------------------------------------------------------------------

#[test]
fn braise_teinte_la_couleur_donnee_et_n_en_produit_aucune_autre() {
    // Hors scope de l'issue, et c'est précisément pourquoi il faut le figer : « la palette :
    // `braise` continue de teinter la couleur donnée, elle n'en produit pas ». Une correction qui
    // introduit un aléa par LED est le genre d'endroit où une composante se met à vivre sa vie.
    //
    // ⚠️ **Cette propriété a déjà servi.** C'est elle qui a permis d'écarter l'animation dans le
    // diagnostic de la LED verte (#120) : si `braise` en bleu ne peut produire que du bleu, une LED
    // verte vient d'ailleurs. La perdre, c'est perdre le raisonnement avec.
    //
    // Deux témoins, deux observables. Le bleu pur fige le cas fort — deux composantes à zéro qui
    // doivent le rester à toute intensité —, et `ff2080` fige l'**ordre** des composantes, qui
    // survit à tout assombrissement et ne survit ni à une couleur ignorée ni à une permutation.
    let geometrie = geometrie();
    let braise = braise();

    for direction in DIRECTIONS {
        for pas in (0..PERIODE).step_by(3) {
            let image = braise.image(&geometrie, &reglages(BLEU, 1, direction), pas);
            for rang in 0..124 {
                let couleur = couleur_par_rang(&image, rang);
                assert!(
                    couleur.r == 0 && couleur.g == 0,
                    "« braise » en bleu pur, pas {pas} : la LED n° {rang} porte ({}, {}, {}) — une \
                     braise teinte la couleur donnée, elle n'en produit pas",
                    couleur.r,
                    couleur.g,
                    couleur.b
                );
            }

            let image = braise.image(&geometrie, &reglages(TEMOIN, 1, direction), pas);
            for rang in 0..124 {
                let couleur = couleur_par_rang(&image, rang);
                assert!(
                    couleur.r >= couleur.b && couleur.b >= couleur.g,
                    "« braise » sous {} : au pas {pas}, la LED n° {rang} porte ({}, {}, {}), dont \
                     les composantes ne sont plus dans l'ordre de la couleur demandée (ff2080, où \
                     le rouge domine, le bleu suit, le vert ferme la marche)",
                    hexa(TEMOIN),
                    couleur.r,
                    couleur.g,
                    couleur.b
                );
            }
        }
    }
}
