//! Tests d'intention des trois animations reprises de WLED (issue #127).
//!
//! Écrits **depuis l'issue #127 seule**, avant l'implémentation. Aucun corps de fonction de
//! `crates/reverb-anim/src/` n'a été relu pour les écrire ; seules ont servi les **signatures
//! publiques** nécessaires à la compilation — relevées au `grep` —, les tests d'intention déjà
//! présents dans ce répertoire, pour les idiomes d'appel du catalogue, et `README.md`. Si l'un de
//! ces tests échoue après implémentation, c'est le code qu'on corrige.
//!
//! # L'API que ces tests supposent
//!
//! Rien de ce qui suit n'existe encore : c'est la phase rouge attendue. ⚠️ **Aucun type public
//! nouveau n'est demandé** — l'issue ajoute trois familles, pas une notion.
//!
//! ```ignore
//! /// Treize familles : les dix d'avant, plus les trois de l'issue.
//! pub const CATALOGUE: &[&str] = &[
//!     "vague", "comete", "respiration", "arc-en-ciel", "balayage", "braise",
//!     "rotation", "thermique", "pouls", "scintillement",
//!     "bougie", "nuee", "artifice",
//! ];
//!
//! // Dans l'énumération privée `Famille` : `Bougie`, `Nuee`, `Artifice`.
//! //
//! // `parametres_acceptes()` rend, pour les trois : ["couleur", "vitesse", "palette"].
//! // Ni "direction", ni "sens", ni "sonde" — aucune n'a d'axe, aucune ne suit de mesure.
//! ```
//!
//! # Trois arbitrages que l'issue laisse ouverts, et qui sont tranchés ici
//!
//! 1. **Les trois acceptent `palette` parce qu'elles acceptent `couleur`.** #126 a figé
//!    l'équivalence — « une palette se donne exactement là où une couleur se donne » — et
//!    `spec_126_palettes.rs` la vérifie sur tout le catalogue. Les trois familles de #127 y entrent
//!    donc sans clause particulière, et l'issue le confirme : « les trois acceptent `couleur`,
//!    `vitesse`, et `palette` ».
//!
//! 2. **`vitesse` reste le seul curseur.** L'issue le met hors scope explicitement — « un réglage
//!    d'intensité ou d'échelle par famille — `vitesse` suffit d'abord ». Aucun test n'exige donc de
//!    quatrième clé, et le test de l'aller-retour n'accepte que ce que la famille déclare.
//!
//! 3. **L'observable des motifs est l'intensité, pas la couleur.** Les trois teintent la couleur
//!    donnée ; sous [`BLANC`], la moyenne des trois composantes **est** l'intensité du motif, en
//!    unités de composante sur 255. C'est l'appareil de `spec_braise_sans_axe.rs` (#119), repris
//!    tel quel — un observable déjà calibré vaut mieux qu'un observable neuf.
//!
//! # Ce qu'aucun de ces tests ne sait voir, et pourquoi il faut le dire
//!
//! ⚠️ **L'observable η² a été essayé pour « `nuee` n'a pas d'axe », et il est écarté.** C'est celui
//! que `spec_braise_sans_axe.rs` retient pour refuser le régime d'onde plane, avec un seuil de 0,65.
//! Mesuré ici sur la géométrie réelle, à la vitesse 1, vingt-quatre instants, quatre axes (x, y, z,
//! azimut), huit tranches d'effectif égal :
//!
//! | source | η² sur son meilleur axe |
//! |---|---|
//! | `vague` sur l'axe qu'elle suit | 0,787 à 0,915 |
//! | tout le reste du catalogue | ≤ 0,479 |
//! | **un champ de bruit dont la maille vaut ⅔ du boîtier** | **0,734** |
//! | un champ de bruit dont la maille vaut ⅓ du boîtier | 0,414 |
//!
//! Un champ de bruit qui tient en une maille et demie **est** un dégradé, et η² le dit — mais rien
//! dans l'issue n'interdit cette maille-là, et un test qui la refuserait imposerait un choix
//! d'échelle que l'issue ne fait pas. L'observable est donc mesuré, documenté, et **non retenu comme
//! critère**. Ce que ce fichier retient à sa place, ce sont les deux propriétés que l'issue nomme
//! vraiment : la corrélation spatiale, et l'absence de reconstitution par translation.
//!
//! ⚠️ **L'appariement de LED à LED par translation a été essayé, et il est écarté aussi.** Les 124
//! LED ne pavent aucun réseau : sur la géométrie mesurée, seuls **huit** décalages (±10 mm et
//! ±20 mm en hauteur, ±10 mm et ±140 mm en profondeur) apparient plus de vingt-quatre LED à moins de
//! 6 mm près. Trop peu, et trop courts, pour distinguer un motif qui défile d'un motif qui ne défile
//! pas — mesuré, le prototype de `nuee` y obtenait le même score que `vague`. La translation est
//! donc mesurée sur le **profil** le long de l'axe, en tranches d'effectif égal, ce qui est
//! l'observable d'advection dont `spec_braise_sans_axe.rs` donne déjà les mesures.
//!
//! ⚠️ **Le test d'équidistance de `artifice` est le plus faible du fichier, et c'est assumé.** Les
//! origines des éclats ne sont pas observables — elles naissent d'un hachage interne —, donc le
//! critère « deux LED à égale distance d'une même origine s'allument ensemble » ne peut être vérifié
//! que par sa conséquence : il existe des instants où plusieurs LED **éloignées les unes des
//! autres** portent la même intensité non nulle. Une onde plane satisferait cela aussi. Le garde-fou
//! qui l'accompagne — `artifice` est un motif corrélé, pas un grésillement — et le test de
//! couverture des organes sont ce qui l'empêche d'être vide.
//!
//! ⚠️ Le critère « les trois apparaissent en pastille dans la fenêtre, sans menu de direction » vit
//! dans `reverb-gui`, et il découle de `parametres_acceptes()` (#20, #119) : ce fichier fige la
//! source, pas son affichage. De même, « `docs/` et `README.md` décrivent les trois » est une
//! exigence de documentation qu'aucun test ne remplacerait.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi.

use std::collections::HashSet;

use reverb_anim::{Animation, CATALOGUE, Direction, Geometrie, Image, Point, Reglages};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// Le domaine
// ---------------------------------------------------------------------------

/// Les trois familles que l'issue ajoute, dans l'ordre de son tableau.
const NOUVELLES: [&str; 3] = ["bougie", "nuee", "artifice"];

/// Les dix familles d'avant, écrites en dur pour distinguer ce qui doit changer de ce qui ne doit
/// pas. Lues chez `CATALOGUE`, elles ne vérifieraient rien.
const ANCIENNES: [&str; 10] = [
    "vague",
    "comete",
    "respiration",
    "arc-en-ciel",
    "balayage",
    "braise",
    "rotation",
    "thermique",
    "pouls",
    "scintillement",
];

/// Durée d'un cycle, en pas, à la vitesse 1 — figée par `spec_sens.rs` (issue #49).
const PERIODE: u32 = 120;

/// Le nombre de LED du boîtier : 10 × 8 + 4 × 11.
const LED_DU_BOITIER: usize = 124;

/// Le nombre d'organes : dix ventilateurs et quatre barrettes.
const ORGANES: usize = 14;

/// Le blanc, pour lire une intensité sur toute l'échelle.
///
/// Les trois familles teintent la couleur qu'on leur donne : sous `ff2080`, l'intensité d'une LED
/// ne parcourt qu'un peu plus de la moitié de `0..255`, et tous les écarts mesurés plus bas seraient
/// comprimés d'autant. Le blanc les rend lisibles tels quels, en unités de composante — c'est
/// l'appareil de `spec_braise_sans_axe.rs`.
const BLANC: Rgb = Rgb::new(0xff, 0xff, 0xff);

/// Une couleur dont les trois composantes diffèrent deux à deux, comme dans `spec_animations.rs`.
const TEMOIN: Rgb = Rgb::new(0xff, 0x20, 0x80);

/// Une couleur sombre : un dépassement de plafond d'une seule unité y saute aux yeux.
const SOMBRE: Rgb = Rgb::new(0x30, 0x30, 0x30);

/// Celle que Nico utilise sur ses bandes WLED, et celle que #126 nomme dans ses tests.
const LIGHT_PINK: &str = "light-pink";

// ---------------------------------------------------------------------------
// Les seuils, et d'où ils viennent
//
// Tous ont été **mesurés** sur la géométrie réelle avant d'être écrits, en exécutant hors du dépôt
// le catalogue d'aujourd'hui et des prototypes des trois familles — un bruit de valeur 4D interpolé
// pour `nuee`, une somme d'ondes sphériques datées pour `artifice`. Chaque seuil est posé **entre
// deux populations mesurées**, jamais entre deux implémentations, et les tables ci-dessous donnent
// les deux populations pour qu'on puisse le contester.
// ---------------------------------------------------------------------------

/// Deux LED sont **proches** en deçà de cette part de l'étendue du boîtier.
///
/// Huit centièmes de 657 mm, soit 53 mm. À cette échelle, deux LED sont sur le même objet — deux LED
/// contiguës d'un anneau de 55 mm de rayon sont à 42 mm l'une de l'autre, deux LED contiguës d'une
/// barrette à bien moins — ou sur deux objets jointifs. La définition est **spatiale et non par
/// indice** : c'est la règle de `spec_animations.rs`, « aucun test ne suppose où se trouve une
/// LED », et c'est la seule qui ait un sens pour un champ qui ignore la numérotation.
///
/// ⚠️ Plus serré que le douzième de `spec_braise_sans_axe.rs`, à dessein : là-bas il s'agissait de
/// refuser le grésillement, ici de mesurer la **corrélation d'un champ**, dont la définition porte
/// sur le voisinage immédiat. 1 048 couples sur la géométrie mesurée.
const PART_PROCHE: f32 = 0.08;

/// Deux LED sont **éloignées** au-delà de cette part de l'étendue du boîtier.
///
/// La moitié de la diagonale : deux organes que rien ne réunit, aux deux bouts du volume. 2 176
/// couples. Repris tel quel de `spec_braise_sans_axe.rs`.
const PART_LOINTAINE: f32 = 0.5;

/// L'écart de couleur moyen entre proches, rapporté à celui entre lointaines.
///
/// **Un facteur deux, franc.** C'est la définition d'un champ de bruit, et c'est le test qui a manqué
/// à `braise` jusqu'à #119. Mesuré à la vitesse 1, sous [`BLANC`], sur vingt-quatre instants :
///
/// | source | rapport |
/// |---|---|
/// | `comete` sous `bords-centre` — chaque objet indépendant | **1,29** |
/// | `scintillement` — chaque LED indépendante | **1,18** |
/// | `vague` sous `bords-centre` | 1,08 |
/// | `rotation` | 0,96 |
/// | — seuil — | **0,50** |
/// | `braise` corrigée par #119 | 0,41 |
/// | `pouls` | 0,28 |
/// | `vague` sur un axe du boîtier | 0,05 à 0,22 |
/// | **prototypes de `nuee`, maille de 1/1 à 1/10 du boîtier** | **0,04 à 0,28** |
/// | prototypes d'`artifice` | 0,29 à 0,33 |
///
/// Les deux populations ne se touchent pas : la plus haute acceptée vaut 0,28 — le seuil est à 78 %
/// au-dessus —, la plus basse refusée 0,96 — le seuil est à 48 % au-dessous. Le facteur deux exigé
/// tombe donc au milieu d'un vide, et non sur le bord d'une mesure.
const RAPPORT_MAXIMAL: f64 = 0.5;

/// En deçà, le boîtier est trop uniforme pour que le rapport ci-dessus veuille dire quoi que ce
/// soit — 0 / 0 satisferait n'importe quel rapport.
///
/// Huit unités de composante sur 255, soit 3 % de l'échelle : sous cette valeur, deux LED aux deux
/// bouts du boîtier ne se distinguent pas à l'œil. Repris de `spec_braise_sans_axe.rs`.
const PLANCHER_CONTRASTE: f64 = 8.0;

/// Le nombre de tranches d'un profil le long d'un axe, pour la mesure de reconstitution.
///
/// Douze tranches pour 124 LED, soit dix ou onze par tranche. Le décalage explorant quatre tranches
/// au plus, le recouvrement vaut toujours au moins huit tranches — deux tiers du profil.
const TRANCHES: usize = 12;

/// Le plus grand décalage exploré, en tranches.
const DECALAGE_MAXIMAL: i32 = 4;

/// L'écart de profil en deçà duquel un pas antérieur est **reconstitué** par une translation.
///
/// L'observable : le profil moyen des intensités le long d'un axe, en tranches d'effectif égal, à
/// deux instants séparés de `k` pas. `E(s)` est l'écart moyen entre le profil du second décalé de
/// `s` tranches et celui du premier ; `E(0)` est l'écart sans décalage. Un motif qui **défile** le
/// long de cet axe rend `E(s) ≈ 0` pour le `s` qui vaut sa vitesse : l'image d'avant est
/// littéralement retrouvée ailleurs.
///
/// Mesuré à la vitesse 1, sous [`BLANC`], quatre axes (x, y, z, azimut), vingt-quatre départs,
/// `k ∈ {5, 10, 15, 20, 30}`, et seulement là où `E(0) ≥` [`CHANGEMENT_MINIMAL`] — sans quoi on
/// mesurerait la reconstitution d'une image qui n'a pas bougé :
///
/// | source | `min E(s)`, `s ≠ 0` |
/// |---|---|
/// | `balayage` sous `horaire` et `antihoraire` | **0,00** |
/// | `comete` sous `horaire` | **0,00** |
/// | `comete` sous `antihoraire` | 2,28 |
/// | `balayage` sous `avant-arriere` / `arriere-avant` | 2,32 / 2,58 |
/// | — seuil — | **4,00** |
/// | `vague` | 6,68 à 21,34 |
/// | **prototypes de `nuee`** | **6,84 à 16,64** |
/// | `respiration` | 7,19 à 16,86 |
/// | `pouls`, `rotation`, `braise`, `scintillement` | 7,97 à 15,68 |
///
/// Le seuil est à 55 % au-dessus de la plus haute refusée et à 41 % au-dessous de la plus basse
/// acceptée. ⚠️ **Il ne refuse pas l'onde plane en général** — `vague` le passe : une onde dont la
/// longueur d'onde ne cadre pas avec les tranches ne se retrouve à aucun décalage entier. Ce qu'il
/// refuse est précisément ce que l'issue nomme, et rien de plus : qu'un **décalage spatial
/// reconstitue une image antérieure**.
const RECONSTITUTION_MINIMALE: f64 = 4.0;

/// L'écart de profil en deçà duquel deux instants n'ont pas assez changé pour conclure.
///
/// Vingt unités de composante sur 255. Sous cette valeur, `E(s) ≈ E(0) ≈ 0` pour tout `s`, et « rien
/// ne reconstitue rien » serait vrai d'un boîtier figé. Mesuré : abaissé à 12, le prototype de
/// `nuee` tombe à 4,10 et rejoint la population refusée — le plancher n'est pas un ornement, c'est
/// lui qui rend la mesure lisible.
const CHANGEMENT_MINIMAL: f64 = 20.0;

/// Combien de couples (axe, départ, `k`) doivent franchir le plancher pour que la mesure existe.
///
/// Mesuré : les prototypes de `nuee` en fournissent 79 à 360 sur les 480 possibles. Vingt est un
/// garde-fou de non-vacuité, pas une exigence.
const COUPLES_MINIMAUX: usize = 20;

/// L'écart, en unités de composante, sous lequel deux LED portent la **même** intensité.
///
/// Deux unités sur 255 : l'arrondi vers l'octet, et rien de plus. C'est la tolérance de
/// `spec_familles_nouvelles.rs` pour les couples équidistants de `pouls`, reprise ici pour la même
/// raison — deux LED que la géométrie déclare équidistantes d'une origine ne peuvent différer que de
/// l'arrondi.
const TOLERANCE_INTENSITE: f64 = 2.0;

/// En deçà, une LED est éteinte et ne porte aucune intensité qu'on puisse comparer.
///
/// Huit unités sur 255. Sans ce plancher, le test d'équidistance d'`artifice` serait **trivialement
/// vrai** : cent LED éteintes portent toutes « la même intensité », et elles sont éloignées.
const SEUIL_ALLUMEE: f64 = 8.0;

/// Le diamètre minimal, en part de l'étendue, d'un groupe de LED de même intensité.
///
/// Plus du tiers du boîtier : le groupe traverse le volume, il n'est pas la tache locale d'un seul
/// organe. C'est la coquille sphérique de `pouls` transposée à un motif dont l'origine bouge.
const DIAMETRE_MINIMAL: f32 = 0.35;

/// La part des pas où un tel groupe doit exister.
///
/// Un dixième. Mesuré sur quatre cents pas, à la vitesse 1, sous [`BLANC`] :
///
/// | source | part des pas |
/// |---|---|
/// | `pouls` — l'onde sphérique du catalogue, l'appareil | **28 %** |
/// | prototypes d'`artifice` | 96 % |
///
/// Le seuil est à moins de la moitié de `pouls`, et c'est délibéré : une coquille fine n'allume que
/// quelques LED, et il y a des instants où elle n'en touche aucune. Exiger davantage reviendrait à
/// imposer une épaisseur de front, que l'issue ne demande pas.
const PART_AVEC_GROUPE: f64 = 0.10;

/// Le nombre de pas observés pour les mesures d'`artifice` qui portent sur une image isolée.
const PAS_OBSERVES: u32 = 400;

/// Le nombre de pas observés pour la couverture des organes.
///
/// Dix mille, comme l'issue l'écrit : « sur dix mille pas, les origines touchent plus d'un organe ».
const PAS_DE_COUVERTURE: u32 = 10_000;

/// Le nombre d'organes qu'`artifice` doit toucher sur [`PAS_DE_COUVERTURE`].
///
/// Huit sur quatorze. L'observable : l'organe qui porte la LED **la plus lumineuse** de l'image, au
/// plus près de l'origine de l'éclat en cours. Mesuré à la vitesse 1 :
///
/// | source | organes touchés |
/// |---|---|
/// | `rotation` | 3 |
/// | `balayage` | 4 |
/// | `vague`, `comete`, `respiration` | 5 |
/// | — seuil — | **8** |
/// | prototypes de `nuee` | 12 à 13 |
/// | `braise`, `pouls`, `scintillement` | **14** |
/// | **prototypes d'`artifice`** | **14** |
///
/// Le seuil est à 60 % au-dessus de la plus haute refusée et à 43 % au-dessous des prototypes. Un
/// feu d'artifice qui n'éclaterait que sur un ventilateur en toucherait un.
const ORGANES_MINIMAUX: usize = 8;

/// L'écart d'intensité moyen, entre la plus et la moins lumineuse des LED **allumées**, sous lequel
/// une bougie ne vacille pas LED par LED.
///
/// Seize unités sur 255, soit 6 % de l'échelle — visible sur un boîtier, et huit fois l'arrondi vers
/// l'octet. Le seuil est bas à dessein : il refuse une bougie dont les 124 LED montent et descendent
/// **ensemble** — pour laquelle l'écart vaut exactement zéro, l'appareil le vérifie — sans imposer
/// la profondeur du vacillement, que l'issue laisse ouverte.
const ECART_MINIMAL_BOUGIE: f64 = 16.0;

/// Le nombre de pas balayés pour le plafond de `bougie`.
///
/// Mille, comme l'issue l'écrit : « sur mille pas, aucune LED ne dépasse le plafond ».
const PAS_DE_PLAFOND: u32 = 1000;

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

/// Des réglages explicites, sur la base des valeurs par défaut.
///
/// ⚠️ Construits **par structure et non par le décodeur** : les trois familles refusent la clé
/// `direction`, et ces tests doivent pourtant pouvoir leur en imposer une pour vérifier qu'elle n'a
/// aucun effet. Le champ, lui, ne disparaît pas — les autres familles s'en servent.
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
/// L'organe : `0..10` pour les ventilateurs, `10..14` pour les barrettes.
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

/// L'intensité perçue d'une LED, entre 0 et 255 : la moyenne des trois composantes.
fn intensite(couleur: Rgb) -> f64 {
    (f64::from(couleur.r) + f64::from(couleur.g) + f64::from(couleur.b)) / 3.0
}

/// L'écriture d'une couleur sur le socket : six chiffres hexadécimaux minuscules.
fn hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

/// Les 124 couleurs d'une image, écrites bout à bout en hexadécimal.
fn empreinte_hexa(image: &Image) -> String {
    (0..LED_DU_BOITIER)
        .map(|rang| hexa(couleur_par_rang(image, rang)))
        .collect()
}

fn distance(un: Point, autre: Point) -> f32 {
    ((un.x - autre.x).powi(2) + (un.y - autre.y).powi(2) + (un.z - autre.z).powi(2)).sqrt()
}

/// La plus grande distance entre deux LED du boîtier — l'échelle à laquelle tout se rapporte,
/// calculée et jamais supposée.
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

/// Les couples de LED proches et les couples de LED éloignées, cherchés dans la géométrie.
fn couples(places: &[Point]) -> (Population, Population) {
    let etendue = etendue(places);
    let (mut proches, mut lointaines) = (Vec::new(), Vec::new());
    for (rang, un) in places.iter().enumerate() {
        for (decalage, autre) in places[rang + 1..].iter().enumerate() {
            let ecart = distance(*un, *autre);
            if ecart <= PART_PROCHE * etendue {
                proches.push((rang, rang + 1 + decalage));
            } else if ecart >= PART_LOINTAINE * etendue {
                lointaines.push((rang, rang + 1 + decalage));
            }
        }
    }
    (proches, lointaines)
}

/// Les intensités des 124 LED au pas donné.
fn intensites(
    animation: &Animation,
    geometrie: &Geometrie,
    reglages: &Reglages,
    pas: u32,
) -> Vec<f64> {
    let image = animation.image(geometrie, reglages, pas);
    (0..LED_DU_BOITIER)
        .map(|rang| intensite(couleur_par_rang(&image, rang)))
        .collect()
}

/// Les instants d'observation : un cycle entier, un pas sur cinq. Vingt-quatre images.
///
/// La moyenne sur les instants est ce qui distingue une **propriété** d'un **tirage** : deux LED
/// proches peuvent se trouver de part et d'autre d'une frontière du champ à un instant donné.
fn instants() -> Vec<u32> {
    (0..PERIODE).step_by(5).collect()
}

/// L'écart moyen d'intensité sur une population de couples, moyenné sur les instants.
fn ecart_moyen(images: &[Vec<f64>], couples: &[(usize, usize)]) -> f64 {
    images
        .iter()
        .map(|image| {
            couples
                .iter()
                .map(|(un, autre)| (image[*un] - image[*autre]).abs())
                .sum::<f64>()
                / couples.len() as f64
        })
        .sum::<f64>()
        / images.len() as f64
}

/// Les quatre axes du boîtier, chacun donné comme l'**ordre** des LED le long de lui.
///
/// Trois axes géométriques — `x` d'un flanc à l'autre, `y` du plancher au plafond, `z` de l'avant
/// vers l'arrière (`spec_disposition.rs`, issue #27) — et l'**azimut**, l'angle autour de la
/// verticale passant par le barycentre des 124 LED. Sans ce quatrième, un motif qui tournerait dans
/// le volume n'aurait aucun axe où se lire. Repris de `spec_braise_sans_axe.rs`.
fn axes(places: &[Point]) -> Vec<(&'static str, Vec<usize>)> {
    let combien = places.len() as f32;
    let cx = places.iter().map(|place| place.x).sum::<f32>() / combien;
    let cz = places.iter().map(|place| place.z).sum::<f32>() / combien;
    let coordonnees: Vec<(&'static str, Vec<f32>)> = vec![
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
    ];
    coordonnees
        .into_iter()
        .map(|(nom, valeurs)| {
            let mut ordre: Vec<usize> = (0..places.len()).collect();
            ordre.sort_by(|un, autre| valeurs[*un].total_cmp(&valeurs[*autre]));
            (nom, ordre)
        })
        .collect()
}

/// Le profil moyen des intensités le long d'un axe, en tranches d'**effectif égal**.
///
/// ⚠️ Effectif égal et non largeur égale : les hauteurs du boîtier sont très groupées — dix
/// ventilateurs sur quatre niveaux —, et des tranches de largeur égale en laisseraient de vides tout
/// en en surchargeant d'autres.
fn profil(ordre: &[usize], intensites: &[f64]) -> Vec<f64> {
    let taille = ordre.len().div_ceil(TRANCHES);
    ordre
        .chunks(taille)
        .map(|tranche| {
            tranche.iter().map(|rang| intensites[*rang]).sum::<f64>() / tranche.len() as f64
        })
        .collect()
}

/// `E(s)` : l'écart moyen entre `apres` décalé de `s` tranches et `avant`, sur le recouvrement.
///
/// Rend `None` quand le recouvrement est trop court pour qu'une moyenne veuille dire quelque chose.
fn ecart_de_profil(avant: &[f64], apres: &[f64], decalage: i32) -> Option<f64> {
    let tranches = avant.len() as i32;
    let mut somme = 0.0;
    let mut combien = 0usize;
    for ici in 0..tranches {
        let la = ici + decalage;
        if la < 0 || la >= tranches {
            continue;
        }
        somme += (apres[la as usize] - avant[ici as usize]).abs();
        combien += 1;
    }
    if combien < TRANCHES - DECALAGE_MAXIMAL as usize {
        return None;
    }
    Some(somme / combien as f64)
}

/// Le plus petit `E(s)`, `s ≠ 0`, rencontré sur tous les axes, départs et écarts — avec le nombre de
/// couples qui ont franchi [`CHANGEMENT_MINIMAL`], et de quoi nommer le cas fautif.
///
/// C'est la mesure de « un décalage spatial reconstitue une image antérieure ». Petit = ça défile.
fn reconstitution(
    animation: &Animation,
    geometrie: &Geometrie,
    reglages: &Reglages,
    axes: &[(&'static str, Vec<usize>)],
) -> (f64, usize, String) {
    let mut plus_petit = f64::INFINITY;
    let mut combien = 0usize;
    let mut coupable = String::new();
    for (nom_axe, ordre) in axes {
        for depart in instants() {
            let avant = profil(ordre, &intensites(animation, geometrie, reglages, depart));
            for ecart in [5u32, 10, 15, 20, 30] {
                let apres = profil(
                    ordre,
                    &intensites(animation, geometrie, reglages, depart + ecart),
                );
                let Some(sans_decalage) = ecart_de_profil(&avant, &apres, 0) else {
                    continue;
                };
                if sans_decalage < CHANGEMENT_MINIMAL {
                    continue;
                }
                combien += 1;
                for decalage in -DECALAGE_MAXIMAL..=DECALAGE_MAXIMAL {
                    if decalage == 0 {
                        continue;
                    }
                    let Some(valeur) = ecart_de_profil(&avant, &apres, decalage) else {
                        continue;
                    };
                    if valeur < plus_petit {
                        plus_petit = valeur;
                        coupable = format!(
                            "axe « {nom_axe} », pas {depart} → {}, décalage de {decalage} \
                             tranche(s) : E({decalage}) = {valeur:.2} pour E(0) = {sans_decalage:.2}",
                            depart + ecart
                        );
                    }
                }
            }
        }
    }
    (plus_petit, combien, coupable)
}

/// Existe-t-il, dans cette image, un groupe d'au moins trois LED allumées de même intensité et dont
/// le diamètre atteint [`DIAMETRE_MINIMAL`] de l'étendue ?
///
/// C'est la conséquence observable de « deux LED à égale distance d'une même origine s'allument
/// ensemble » : une coquille sphérique traverse le volume, une tache locale non.
fn porte_un_groupe_eloigne(niveaux: &[f64], places: &[Point], etendue: f32) -> bool {
    let mut deja_vues: Vec<f64> = Vec::new();
    for base in 0..niveaux.len() {
        if niveaux[base] < SEUIL_ALLUMEE {
            continue;
        }
        // ⚠️ **Une intensité déjà examinée à l'identique** définit le même groupe : on ne la
        // reprend pas. La comparaison est **exacte**, et non à [`TOLERANCE_INTENSITE`] près :
        // deux intensités voisines mais distinctes définissent deux groupes **différents**, et
        // écarter la seconde ferait manquer des coquilles. Mesuré — avec un écartement à la
        // tolérance, « pouls » tombe de 28 % des pas à 9,5 %, sous le seuil qui doit le
        // reconnaître.
        if deja_vues.iter().any(|vue| *vue == niveaux[base]) {
            continue;
        }
        deja_vues.push(niveaux[base]);

        let groupe: Vec<usize> = (0..niveaux.len())
            .filter(|rang| {
                niveaux[*rang] >= SEUIL_ALLUMEE
                    && (niveaux[*rang] - niveaux[base]).abs() <= TOLERANCE_INTENSITE
            })
            .collect();
        if groupe.len() < 3 {
            continue;
        }
        for (rang, un) in groupe.iter().enumerate() {
            for autre in &groupe[rang + 1..] {
                if distance(places[*un], places[*autre]) >= DIAMETRE_MINIMAL * etendue {
                    return true;
                }
            }
        }
    }
    false
}

/// L'organe qui porte la LED la plus lumineuse de l'image, ou `None` si tout est éteint.
///
/// Au plus près de l'origine d'un éclat qui vient de naître : c'est le seul observable dont on
/// dispose, les origines n'étant pas exposées.
fn organe_le_plus_vif(niveaux: &[f64], organes: &[usize]) -> Option<usize> {
    let (rang, valeur) =
        niveaux
            .iter()
            .enumerate()
            .fold((0usize, f64::NEG_INFINITY), |vu, (rang, valeur)| {
                if *valeur > vu.1 { (rang, *valeur) } else { vu }
            });
    (valeur >= SEUIL_ALLUMEE).then(|| organes[rang])
}

/// L'écart entre la plus et la moins lumineuse des LED **allumées** d'une image.
///
/// Rend `None` quand moins de deux LED sont allumées : il n'y a alors rien à comparer, et rendre 0
/// ferait passer un boîtier éteint pour un boîtier uniforme.
fn amplitude_des_allumees(niveaux: &[f64]) -> Option<f64> {
    let allumees: Vec<f64> = niveaux
        .iter()
        .copied()
        .filter(|valeur| *valeur >= SEUIL_ALLUMEE)
        .collect();
    if allumees.len() < 2 {
        return None;
    }
    let haut = allumees.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let bas = allumees.iter().copied().fold(f64::INFINITY, f64::min);
    Some(haut - bas)
}

// ---------------------------------------------------------------------------
// 1 — le catalogue
// ---------------------------------------------------------------------------

#[test]
fn les_treize_familles_figurent_au_catalogue_et_se_resolvent_par_leur_nom() {
    // Test d'intention n° 1 de l'issue — « les trois familles se résolvent par leur nom » — et
    // critère d'acceptation : « les trois familles sont dans `CATALOGUE`, `FAMILLES` et `Famille` ».
    //
    // Les dix d'avant y restent : l'issue en ajoute trois, elle n'en retire aucune. C'est aussi ce
    // qui rend le figeage de la fin de ce fichier vérifiable — une famille disparue ne se comparerait
    // à rien.
    for nom in ANCIENNES {
        assert!(
            CATALOGUE.contains(&nom),
            "« {nom} » doit rester au catalogue : {CATALOGUE:?}"
        );
    }
    for nom in NOUVELLES {
        assert!(
            CATALOGUE.contains(&nom),
            "« {nom} » doit rejoindre le catalogue : {CATALOGUE:?}"
        );
    }
    assert_eq!(
        CATALOGUE.len(),
        ANCIENNES.len() + NOUVELLES.len(),
        "le catalogue compte treize familles : {CATALOGUE:?}"
    );

    for nom in NOUVELLES {
        // Les noms traversent le protocole comme des jetons séparés par des espaces — `animate nuee
        // palette=light-pink` —, donc la même règle de slug que les autres : ASCII, minuscules, sans
        // espace. « nuee » et non « nuée » : un nom accentué serait refusé par le décodeur du socket
        // sans que rien ne dise pourquoi.
        assert!(nom.is_ascii(), "« {nom} » n'est pas en ASCII");
        assert!(!nom.contains(' '), "« {nom} » contient une espace");
        assert_eq!(nom, nom.to_lowercase(), "« {nom} » n'est pas en minuscules");

        let animation = animation(nom);
        assert_eq!(animation.nom(), nom, "« {nom} » se relit sous son nom");
    }
}

#[test]
fn les_trois_familles_animent_vraiment_le_boitier() {
    // Aucun critère de l'issue ne le dit, et c'est bien pour cela qu'il faut l'écrire : tous les
    // autres tests de ce fichier portent sur des **propriétés** d'une image, et une image noire
    // constante en satisfait plusieurs sans rien allumer. `spec_familles_nouvelles.rs` a rencontré
    // exactement ce cas au banc d'essai par mutation — un `rotation` qui ne tourne pas, un `pouls`
    // qui ne se propage pas —, et le remède qu'il a trouvé est repris ici.
    let geometrie = geometrie();
    for nom in NOUVELLES {
        let animation = animation(nom);
        let reglages = reglages(TEMOIN, 3, Direction::BasHaut);
        let distinctes: HashSet<String> = (0..PERIODE)
            .map(|pas| empreinte_hexa(&animation.image(&geometrie, &reglages, pas)))
            .collect();
        assert!(
            distinctes.len() >= 10,
            "« {nom} » ne rend que {} image(s) distincte(s) sur un cycle entier : le boîtier est figé",
            distinctes.len()
        );
        let allumee = (0..PERIODE).any(|pas| {
            (0..LED_DU_BOITIER).any(|rang| {
                couleur_par_rang(&animation.image(&geometrie, &reglages, pas), rang) != Rgb::BLACK
            })
        });
        assert!(
            allumee,
            "« {nom} » n'allume aucune LED sur un cycle entier : elle change d'image noire en image \
             noire"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — les réglages : ce qui est accepté, ce qui est refusé
// ---------------------------------------------------------------------------

#[test]
fn les_trois_familles_refusent_direction_et_le_refus_nomme_la_cle() {
    // Test d'intention n° 2 de l'issue — « `direction=` est refusé sur les trois, et le message
    // nomme la clé » — et critère d'acceptation : « `animate <famille> direction=…` est **refusé**
    // pour les trois, et refuse la commande entière ».
    //
    // Les deux moitiés comptent, et pour deux raisons différentes. `parametres_acceptes` est « la
    // seule source de vérité du refus » (#20) et ce que **la fenêtre lit** pour décider d'afficher le
    // menu des directions : l'y laisser afficherait un menu dont chaque choix ferait rejeter
    // l'`animate` entier. Le refus nommé, lui, est ce qui rend la règle lisible à celui qui vient de
    // taper la clé.
    //
    // ⚠️ `sens` est refusé au même titre : `spec_animations.rs` le traite comme l'autre graphie de la
    // même clé, et un contrat qui n'en fermerait qu'une porte n'en fermerait aucune.
    //
    // ⚠️ **La commande entière est rejetée, pas seulement la clé.** Le troisième jeu d'essai entoure
    // `direction` de clés valides : un décodeur qui s'arrêterait à la première paire acceptable
    // appliquerait la moitié des réglages sans rien dire.
    for nom in NOUVELLES {
        let animation = animation(nom);
        let acceptes = animation.parametres_acceptes();

        for cle in ["direction", "sens"] {
            assert!(
                !acceptes.contains(&cle),
                "« {nom} » n'a aucun axe : elle ne doit pas accepter `{cle}` — {acceptes:?}"
            );
            for direction in Direction::ALL {
                for paires in [
                    vec![paire(cle, direction.slug())],
                    vec![paire("vitesse", "5"), paire(cle, direction.slug())],
                    vec![
                        paire("couleur", &hexa(TEMOIN)),
                        paire(cle, direction.slug()),
                        paire("vitesse", "5"),
                    ],
                ] {
                    let erreur = animation
                        .reglages(&paires)
                        .expect_err("une clé que la famille ne déclare pas est refusée");
                    assert_eq!(
                        erreur.cle,
                        cle,
                        "« {nom} » avec « {cle}={} » : le refus doit nommer la clé fautive, pas se \
                         contenter d'échouer — {erreur}",
                        direction.slug()
                    );
                    assert!(
                        !erreur.raison.is_empty(),
                        "« {nom} » : le refus doit dire pourquoi — {erreur}"
                    );
                }
            }
        }
    }
}

#[test]
fn les_trois_familles_ne_rendent_pas_la_direction_observable() {
    // Corollaire du refus ci-dessus, et il ne va pas de soi : refuser la clé au décodeur n'empêche
    // pas le rendu de lire le champ `direction` de `Reglages`, que les autres familles remplissent.
    // C'est le défaut que #119 a corrigé sur `braise` — la clé est partie, le champ est resté — et
    // c'est le test central de `spec_braise_sans_axe.rs`, repris ici pour les trois familles.
    //
    // La comparaison est **LED par LED et instant par instant**, et à deux vitesses : une égalité qui
    // ne tiendrait qu'à la vitesse par défaut laisserait passer un motif dont la direction ne
    // resurgirait qu'une fois le curseur poussé.
    let geometrie = geometrie();
    let led = toutes_les_led(&geometrie);

    for nom in NOUVELLES {
        let animation = animation(nom);
        for vitesse in [1u8, 7] {
            let temoin = reglages(TEMOIN, vitesse, Direction::ALL[0]);
            for pas in (0..PERIODE).step_by(3).chain([200, 901]) {
                let attendue = animation.image(&geometrie, &temoin, pas);
                for direction in Direction::ALL.into_iter().skip(1) {
                    let rendue =
                        animation.image(&geometrie, &reglages(TEMOIN, vitesse, direction), pas);
                    if rendue == attendue {
                        continue;
                    }
                    let (rang, nom_led) = led
                        .iter()
                        .enumerate()
                        .find(|(rang, _)| {
                            couleur_par_rang(&rendue, *rang) != couleur_par_rang(&attendue, *rang)
                        })
                        .map(|(rang, (_, nom_led, _))| (rang, nom_led.clone()))
                        .expect("deux images qui diffèrent diffèrent sur au moins une LED");
                    panic!(
                        "« {nom} » suit un axe : à la vitesse {vitesse}, au pas {pas}, {nom_led} \
                         porte {} sous « {} » et {} sous « {} » — aucune des trois familles de #127 \
                         n'a de direction",
                        hexa(couleur_par_rang(&rendue, rang)),
                        direction.slug(),
                        hexa(couleur_par_rang(&attendue, rang)),
                        Direction::ALL[0].slug(),
                    );
                }
            }
        }
    }
}

#[test]
fn les_trois_familles_acceptent_couleur_vitesse_et_palette() {
    // Test d'intention n° 3 de l'issue — « `palette=` et `couleur=` sont acceptés sur les trois » —
    // et critère d'acceptation : « les trois acceptent `couleur`, `vitesse`, et `palette` (#126) ».
    //
    // ⚠️ **Acceptée n'est pas rangée, et rangée n'est pas rendue.** `spec_animations.rs` nomme les
    // trois défauts, du moins grave au pire : une clé refusée à tort, une clé acceptée puis mal
    // rangée, et une clé rangée dans son champ **puis ignorée au rendu** — celle-là ne se voit nulle
    // part. Les trois moitiés sont donc vérifiées ici : déclarée, rangée, et effective.
    let geometrie = geometrie();

    for nom in NOUVELLES {
        let animation = animation(nom);
        let acceptes = animation.parametres_acceptes();
        for cle in ["couleur", "vitesse", "palette"] {
            assert!(
                acceptes.contains(&cle),
                "« {nom} » doit accepter `{cle}` : {acceptes:?}"
            );
        }
        // Et rien d'autre : `sonde` n'a de sens que pour la famille pilotée par une mesure.
        assert!(
            !acceptes.contains(&"sonde"),
            "« {nom} » ne suit aucune mesure : elle ne doit pas accepter `sonde` — {acceptes:?}"
        );

        let lus = animation
            .reglages(&[paire("couleur", &hexa(TEMOIN)), paire("vitesse", "7")])
            .unwrap_or_else(|erreur| panic!("« {nom} » accepte couleur et vitesse : {erreur}"));
        assert_eq!(
            lus.couleur, TEMOIN,
            "« {nom} » : la couleur doit atterrir dans son champ"
        );
        assert_eq!(
            lus.vitesse, 7,
            "« {nom} » : la vitesse doit atterrir dans son champ"
        );
        assert_eq!(
            lus.palette, None,
            "« {nom} » : une couleur seule ne doit pas poser de palette"
        );

        let avec_palette = animation
            .reglages(&[paire("palette", LIGHT_PINK)])
            .unwrap_or_else(|erreur| {
                panic!("« {nom} » doit accepter « palette={LIGHT_PINK} » : {erreur}")
            });
        assert_eq!(
            avec_palette.palette.map(|p| p.nom()),
            Some(LIGHT_PINK),
            "« {nom} » : « palette={LIGHT_PINK} » doit atterrir dans le champ `palette`"
        );

        // La couleur est effective : deux couleurs distinctes donnent deux images distinctes.
        let bleu = reglages(Rgb::new(0x00, 0x00, 0xff), 3, Direction::BasHaut);
        let rouge = reglages(Rgb::new(0xff, 0x00, 0x00), 3, Direction::BasHaut);
        assert!(
            (0..PERIODE).any(|pas| animation.image(&geometrie, &bleu, pas)
                != animation.image(&geometrie, &rouge, pas)),
            "« {nom} » peint la même image en bleu pur et en rouge pur : la couleur est rangée dans \
             les réglages, puis ignorée au rendu"
        );

        // La palette aussi : #126 exige qu'elle change le rendu, pas seulement les réglages.
        let sans = reglages(TEMOIN, 3, Direction::BasHaut);
        assert!(
            (0..PERIODE).any(|pas| animation.image(&geometrie, &avec_palette, pas)
                != animation.image(&geometrie, &sans, pas)),
            "« {nom} » peint la même image avec et sans « palette={LIGHT_PINK} » : la palette est \
             rangée dans les réglages, puis ignorée au rendu"
        );

        // Et la vitesse : un curseur qui ne ferait rien serait le même défaut.
        let lente = reglages(TEMOIN, 1, Direction::BasHaut);
        let vive = reglages(TEMOIN, 9, Direction::BasHaut);
        assert!(
            (0..PERIODE).any(|pas| animation.image(&geometrie, &lente, pas)
                != animation.image(&geometrie, &vive, pas)),
            "« {nom} » peint la même image à la vitesse 1 et à la vitesse 9 : le curseur ne fait rien"
        );
    }
}

#[test]
fn les_trois_familles_se_reencodent_et_se_redecodent_sans_perte() {
    // Critère d'acceptation implicite mais indispensable : le README promet que « le boîtier retrouve
    // seul, après un redémarrage, ce qu'il affichait […] une animation avec ses réglages ». Une
    // famille dont un réglage se perdrait au passage par `eclairage.conf` redémarrerait sur autre
    // chose, sans le dire.
    //
    // ⚠️ **Les deux branches de l'exclusion `couleur` / `palette` sont parcourues** (#126) : sous
    // palette, `couleur` n'est **pas écrite**, donc rien ne peut la relire — écrire les deux
    // produirait un fichier d'état que le démon refuse au redémarrage suivant, ce qui est le défaut
    // de #69.
    for nom in NOUVELLES {
        let animation = animation(nom);
        let acceptes = animation.parametres_acceptes();

        let sans_palette = Reglages {
            couleur: TEMOIN,
            vitesse: 7,
            ..Reglages::default()
        };
        let avec_palette = Reglages {
            palette: Some(reverb_anim::Palette::par_nom(LIGHT_PINK).expect("palette du catalogue")),
            vitesse: 7,
            ..Reglages::default()
        };

        for temoin in [sans_palette, avec_palette] {
            let ecrits = animation.reglages_ecrits(&temoin);
            for cle in acceptes {
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
            assert!(
                !ecrits
                    .iter()
                    .any(|(cle, _)| cle == "direction" || cle == "sens"),
                "« {nom} » écrit une direction dans son état : un `eclairage.conf` ainsi produit \
                 serait refusé à la relecture par la famille elle-même — {ecrits:?}"
            );

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
// 3 — la pureté
// ---------------------------------------------------------------------------

#[test]
fn les_trois_familles_rendent_la_meme_image_au_meme_pas() {
    // Test d'intention n° 4 de l'issue — « deux appels identiques à `image()` rendent la même
    // image » — et critère d'acceptation : « le rendu est une **fonction pure** de `(geometrie,
    // reglages, pas)` : deux appels avec les mêmes arguments rendent la même image, octet pour octet.
    // Aucun `rand`, aucune horloge, aucun état. »
    //
    // C'est ce qui rend vraie la promesse du README — « l'aperçu montre ce que le boîtier reçoit » —,
    // et c'est le point où les trois familles de cette issue sont le plus exposées : elles reposent
    // toutes trois sur un aléa, là où WLED en tire un à chaque image depuis un générateur à état.
    //
    // La comparaison est **octet pour octet** : deux images « très proches » ne suffisent pas, la
    // fenêtre et le démon calculent chacun la leur.
    let geometrie = geometrie();
    for nom in NOUVELLES {
        let animation = animation(nom);
        for vitesse in [1u8, 3, 10] {
            let reglages = reglages(TEMOIN, vitesse, Direction::BasHaut);
            for pas in [0u32, 1, 42, 119, 120, 901, 10_007] {
                assert_eq!(
                    empreinte_hexa(&animation.image(&geometrie, &reglages, pas)),
                    empreinte_hexa(&animation.image(&geometrie, &reglages, pas)),
                    "« {nom} » à la vitesse {vitesse} : deux appels au pas {pas} rendent deux images \
                     différentes"
                );
            }
        }
    }
}

#[test]
fn les_trois_familles_ne_gardent_aucun_etat_entre_deux_appels() {
    // Le même critère, mais qu'un aléa ensemencé une fois passerait sans lui : deux appels
    // consécutifs à la même microseconde peuvent coïncider par hasard, un compteur global non.
    //
    // Mille images sont donc intercalées entre les deux relevés, et chaque relevé repasse par une
    // animation fraîchement ouverte. Puis les pas sont redemandés **à rebours** : le démon en saute
    // quand une image est en retard, et un motif qui avancerait par incréments s'en apercevrait.
    //
    // ⚠️ `artifice` est celle des trois où la tentation est la plus forte — un éclat a une date de
    // naissance, et « garder les éclats en cours » est le réflexe qu'on a en l'écrivant. L'issue
    // tranche : « un éclat a une date de naissance et une origine, toutes deux déduites d'un hachage
    // du numéro d'éclat, lui-même déduit de `pas` ».
    let geometrie = geometrie();
    let reglages = reglages(TEMOIN, 3, Direction::BasHaut);

    for nom in NOUVELLES {
        let attendue = animation(nom).image(&geometrie, &reglages, 42);
        for pas in 0..1000u32 {
            let _ = animation(nom).image(&geometrie, &reglages, pas);
        }
        assert_eq!(
            animation(nom).image(&geometrie, &reglages, 42),
            attendue,
            "« {nom} » au pas 42 dépend de ce qui a été rendu avant : elle garde un état, ou tire \
             son aléa d'ailleurs que du numéro de pas"
        );

        let a_rebours: Vec<Image> = (0..16u32)
            .rev()
            .map(|pas| animation(nom).image(&geometrie, &reglages, pas))
            .collect();
        for (rang, image) in a_rebours.iter().rev().enumerate() {
            assert_eq!(
                *image,
                animation(nom).image(&geometrie, &reglages, rang as u32),
                "« {nom} » : le pas {rang} dépend de l'ordre dans lequel on le demande"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — `bougie` : une bougie faiblit, elle ne surbrille pas
// ---------------------------------------------------------------------------

#[test]
fn bougie_ne_depasse_jamais_le_plafond_de_la_couleur_reglee() {
    // Test d'intention n° 6 de l'issue — « sur mille pas, aucune LED ne dépasse le plafond » — et
    // critère d'acceptation : « le niveau d'une LED **ne remonte jamais au-dessus** de son plafond —
    // une bougie faiblit, elle ne surbrille pas ».
    //
    // Le plafond est la couleur réglée à pleine intensité : c'est ce que « le niveau » veut dire pour
    // une famille qui teinte. Une LED qui le dépasserait serait une composante saturée à 255 sur une
    // couleur qu'on a choisie sombre — un blanc qui apparaît dans une bougie rouge.
    //
    // ⚠️ **Deux couleurs, et il faut les deux.** `ff2080` a une composante verte à 32 : une marche
    // qui déborderait d'un dixième s'y voit. `303030` est sombre sur les trois : une implémentation
    // qui calculerait le niveau puis oublierait de le rapporter à la couleur y saturerait partout.
    let geometrie = geometrie();
    let bougie = animation("bougie");

    for couleur in [TEMOIN, SOMBRE, BLANC] {
        for vitesse in [1u8, 5, 10] {
            let reglages = reglages(couleur, vitesse, Direction::BasHaut);
            for pas in 0..PAS_DE_PLAFOND {
                let image = bougie.image(&geometrie, &reglages, pas);
                for rang in 0..LED_DU_BOITIER {
                    let rendue = couleur_par_rang(&image, rang);
                    assert!(
                        rendue.r <= couleur.r && rendue.g <= couleur.g && rendue.b <= couleur.b,
                        "« bougie » sous {} à la vitesse {vitesse}, pas {pas} : la LED n° {rang} \
                         porte {}, au-dessus du plafond — une bougie faiblit, elle ne surbrille pas",
                        hexa(couleur),
                        hexa(rendue)
                    );
                }
            }
        }
    }
}

#[test]
fn bougie_approche_son_plafond_sans_jamais_l_enfoncer() {
    // Garde-fou du précédent, et il est indispensable : « aucune LED au-dessus du plafond » est
    // trivialement satisfait par un boîtier noir. Une bougie brûle, donc au moins une LED doit
    // atteindre une bonne part de son plafond quelque part sur mille pas.
    //
    // La moitié du plafond, et pas davantage : l'issue ne dit rien de la profondeur du vacillement —
    // WLED la règle par un paramètre d'intensité que #127 met explicitement hors scope. Exiger 90 %
    // imposerait un choix que l'issue laisse ouvert.
    let geometrie = geometrie();
    let bougie = animation("bougie");
    let reglages = reglages(BLANC, 3, Direction::BasHaut);

    let plus_vive = (0..PAS_DE_PLAFOND)
        .map(|pas| {
            let image = bougie.image(&geometrie, &reglages, pas);
            (0..LED_DU_BOITIER)
                .map(|rang| intensite(couleur_par_rang(&image, rang)))
                .fold(0.0f64, f64::max)
        })
        .fold(0.0f64, f64::max);
    assert!(
        plus_vive >= 128.0,
        "« bougie » sous blanc : sur {PAS_DE_PLAFOND} pas, la LED la plus vive n'atteint jamais que \
         {plus_vive:.1} sur 255 — le plafond n'est jamais approché, et « ne jamais le dépasser » ne \
         veut alors rien dire"
    );
}

#[test]
fn l_appareil_voit_un_boitier_dont_toutes_les_led_sont_au_meme_niveau() {
    // Test d'appareil, vert par nature. Une mesure faite sur un appareil faux ne dit rien, et ne le
    // signale pas — c'est la règle de `spec_braise_sans_axe.rs`, qui s'appareille avant de conclure.
    //
    // Ce qu'il établit : [`amplitude_des_allumees`] rend **exactement zéro** sur le défaut qu'elle
    // doit voir — 124 LED au même niveau, ce que produirait une bougie dont le vacillement serait
    // global au lieu d'être par LED. Et elle rend `None`, jamais zéro, sur un boîtier éteint : sans
    // cette distinction, un rendu noir passerait pour un rendu uniforme et le test suivant serait
    // trompé dans les deux sens.
    assert_eq!(
        amplitude_des_allumees(&vec![200.0; LED_DU_BOITIER]),
        Some(0.0),
        "appareil : 124 LED au même niveau doivent donner une amplitude nulle"
    );
    assert_eq!(
        amplitude_des_allumees(&vec![0.0; LED_DU_BOITIER]),
        None,
        "appareil : un boîtier éteint ne porte aucune amplitude — rendre zéro le ferait passer pour \
         un boîtier uniforme"
    );
    let mut une_seule = vec![0.0f64; LED_DU_BOITIER];
    une_seule[7] = 200.0;
    assert_eq!(
        amplitude_des_allumees(&une_seule),
        None,
        "appareil : une seule LED allumée ne se compare à personne"
    );
    let mut deux = vec![0.0f64; LED_DU_BOITIER];
    deux[7] = 200.0;
    deux[63] = 100.0;
    assert_eq!(
        amplitude_des_allumees(&deux),
        Some(100.0),
        "appareil : l'amplitude est l'écart entre la plus et la moins lumineuse des allumées"
    );
}

#[test]
fn bougie_ne_met_pas_toutes_les_led_au_meme_niveau() {
    // Test d'intention n° 5 de l'issue — « à `pas` fixé, au moins deux LED ont des niveaux
    // distincts » — et critère d'acceptation : « à un instant donné, les LED ne sont **pas toutes**
    // au même niveau — chacune suit sa propre marche ».
    //
    // ⚠️ **La comparaison porte sur les LED ALLUMÉES**, et c'est ce qui empêche le test d'être
    // trivialement vrai : une bougie qui éteindrait la moitié du boîtier montrerait « des niveaux
    // distincts » sans que rien ne vacille. Voir [`SEUIL_ALLUMEE`], et le test d'appareil ci-dessus.
    //
    // Deux exigences, et il les faut toutes les deux. La littérale — à aucun pas les 124 LED ne
    // portent la même couleur — condamne le vacillement global, celui d'une bougie unique dont les
    // LED seraient les copies. La quantitative — [`ECART_MINIMAL_BOUGIE`] en moyenne — l'empêche
    // d'être satisfaite par une seule unité d'arrondi sur 255.
    let geometrie = geometrie();
    let bougie = animation("bougie");

    for vitesse in [1u8, 5, 10] {
        let reglages = reglages(BLANC, vitesse, Direction::BasHaut);
        let mut somme = 0.0f64;
        let mut combien = 0usize;

        for pas in 0..PAS_DE_PLAFOND {
            let image = bougie.image(&geometrie, &reglages, pas);
            let couleurs: HashSet<(u8, u8, u8)> = (0..LED_DU_BOITIER)
                .map(|rang| {
                    let c = couleur_par_rang(&image, rang);
                    (c.r, c.g, c.b)
                })
                .collect();
            assert!(
                couleurs.len() > 1,
                "« bougie » à la vitesse {vitesse}, pas {pas} : les 124 LED portent la même couleur \
                 {:?} — le vacillement est global, or chaque LED doit suivre sa propre marche",
                couleurs.iter().next()
            );

            let niveaux: Vec<f64> = (0..LED_DU_BOITIER)
                .map(|rang| intensite(couleur_par_rang(&image, rang)))
                .collect();
            if let Some(amplitude) = amplitude_des_allumees(&niveaux) {
                somme += amplitude;
                combien += 1;
            }
        }

        assert!(
            combien * 2 >= PAS_DE_PLAFOND as usize,
            "appareil : « bougie » à la vitesse {vitesse} n'a deux LED allumées qu'à {combien} pas \
             sur {PAS_DE_PLAFOND} — la mesure n'a presque rien mesuré"
        );
        let moyenne = somme / combien as f64;
        assert!(
            moyenne >= ECART_MINIMAL_BOUGIE,
            "« bougie » à la vitesse {vitesse} : entre la plus et la moins lumineuse des LED \
             allumées, l'écart moyen ne vaut que {moyenne:.1} sur 255, pour \
             {ECART_MINIMAL_BOUGIE:.0} exigés — les 124 LED montent et descendent ensemble"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — `nuee` : un champ, et non un axe
// ---------------------------------------------------------------------------

#[test]
fn l_appareil_distingue_le_champ_correle_du_gresillement() {
    // Test d'appareil, vert par nature.
    //
    // Trois vérifications : les deux populations de couples existent et sont grandes, l'étendue est
    // celle d'un boîtier, et surtout **le rapport sait voir l'extrême qu'il refuse** —
    // `scintillement`, la famille où chaque LED est indépendante, doit le dépasser franchement. Sans
    // cette dernière, un seuil devenu aveugle passerait pour un seuil respecté.
    let geometrie = geometrie();
    let places: Vec<Point> = toutes_les_led(&geometrie)
        .into_iter()
        .map(|(_, _, place)| place)
        .collect();
    assert_eq!(places.len(), LED_DU_BOITIER, "le boîtier porte 124 LED");
    assert_eq!(
        LED_DU_BOITIER,
        Position::ALL.len() * LEDS_PER_FAN as usize + SLOT_COUNT * LEDS_PER_STICK,
        "appareil : 10 × 8 + 4 × 11"
    );

    let (proches, lointaines) = couples(&places);
    assert!(
        proches.len() > 500 && lointaines.len() > 500,
        "appareil : {} couples proches et {} couples lointains — trop peu pour qu'une moyenne soit \
         une propriété plutôt qu'un tirage",
        proches.len(),
        lointaines.len()
    );

    let scintillement = animation("scintillement");
    let reglages = reglages(BLANC, 1, Direction::BasHaut);
    let images: Vec<Vec<f64>> = instants()
        .iter()
        .map(|pas| intensites(&scintillement, &geometrie, &reglages, *pas))
        .collect();
    let rapport = ecart_moyen(&images, &proches) / ecart_moyen(&images, &lointaines);
    assert!(
        rapport > RAPPORT_MAXIMAL,
        "appareil : « scintillement » — la famille où chaque LED est indépendante — rend un rapport \
         de {rapport:.3}, sous le seuil de {RAPPORT_MAXIMAL:.2} qui doit précisément le refuser : la \
         mesure ne voit plus la granularité"
    );
}

#[test]
fn nuee_rapproche_en_couleur_les_led_proches_dans_l_espace() {
    // Test d'intention n° 7 de l'issue — « la corrélation spatiale des LED voisines dépasse celle des
    // LED éloignées » — et critère d'acceptation : « deux LED **proches dans l'espace** sont plus
    // proches en couleur que deux LED éloignées — c'est la définition d'un champ de bruit, et c'est
    // le test qui a manqué à `braise` jusqu'à #119 ».
    //
    // C'est le test central de cette issue. L'approche technique dit ce qu'il protège : « Reverb
    // échantillonne le bruit à la **position réelle** de la LED […] deux LED voisines de deux
    // ventilateurs différents partagent leur couleur, ce que la version 1D ne peut pas faire ». Une
    // reprise paresseuse de `perlin8(i*scale, …)` — le bruit indexé par le **numéro** de LED — rend
    // un champ parfaitement joli sur un ruban et parfaitement granulaire dans un volume : deux LED
    // contiguës d'un anneau y sont voisines, deux LED voisines de deux ventilateurs différents non.
    //
    // ⚠️ **Le facteur exigé est deux, et il tombe au milieu d'un vide mesuré** — voir
    // [`RAPPORT_MAXIMAL`], dont la table donne les deux populations.
    //
    // ⚠️ **La mesure porte sur une population, jamais sur un couple.** Deux LED proches peuvent se
    // trouver de part et d'autre d'une frontière du champ ; ce sont donc plus de mille couples, sur
    // vingt-quatre instants, et c'est la moyenne qui porte la propriété.
    let geometrie = geometrie();
    let nuee = animation("nuee");
    let places: Vec<Point> = toutes_les_led(&geometrie)
        .into_iter()
        .map(|(_, _, place)| place)
        .collect();
    let (proches, lointaines) = couples(&places);

    for vitesse in [1u8, 5, 10] {
        let reglages = reglages(BLANC, vitesse, Direction::BasHaut);
        let images: Vec<Vec<f64>> = instants()
            .iter()
            .map(|pas| intensites(&nuee, &geometrie, &reglages, *pas))
            .collect();

        let entre_lointaines = ecart_moyen(&images, &lointaines);
        assert!(
            entre_lointaines >= PLANCHER_CONTRASTE,
            "« nuee » à la vitesse {vitesse} : deux LED aux deux bouts du boîtier ne diffèrent en \
             moyenne que de {entre_lointaines:.1} sur 255 — le boîtier est uni, il n'y a pas de champ \
             à mesurer"
        );
        let entre_proches = ecart_moyen(&images, &proches);
        let rapport = entre_proches / entre_lointaines;
        assert!(
            rapport <= RAPPORT_MAXIMAL,
            "« nuee » à la vitesse {vitesse} : deux LED proches diffèrent de {entre_proches:.1} \
             quand deux lointaines diffèrent de {entre_lointaines:.1}, soit un rapport de \
             {rapport:.3} pour {RAPPORT_MAXIMAL:.2} au plus — le champ n'est pas échantillonné à la \
             position réelle des LED, il l'est à leur numéro"
        );
    }
}

#[test]
fn l_appareil_voit_une_reconstitution_par_translation() {
    // Test d'appareil, vert par nature, et il est le pendant obligé du test suivant : celui-ci exige
    // qu'**aucun** décalage ne reconstitue une image antérieure, ce qu'un appareil aveugle
    // satisferait sans rien regarder.
    //
    // Ce qu'il établit : la mesure sait voir un motif qui défile. `comete` et `balayage` sous
    // `horaire` défilent le long de l'azimut, et leur profil s'y retrouve **à l'identique** après
    // translation — mesuré à 0,00, c'est-à-dire une reconstitution parfaite.
    let geometrie = geometrie();
    let places: Vec<Point> = toutes_les_led(&geometrie)
        .into_iter()
        .map(|(_, _, place)| place)
        .collect();
    let axes = axes(&places);

    for nom in ["comete", "balayage"] {
        let animation = animation(nom);
        let reglages = reglages(BLANC, 1, Direction::Horaire);
        let (plus_petit, combien, _) = reconstitution(&animation, &geometrie, &reglages, &axes);
        assert!(
            combien >= COUPLES_MINIMAUX,
            "appareil : « {nom} » sous « horaire » ne fournit que {combien} couple(s) au-dessus du \
             plancher de changement — la mesure n'a rien mesuré"
        );
        assert!(
            plus_petit < RECONSTITUTION_MINIMALE,
            "appareil : « {nom} » sous « horaire » — un motif qui défile le long de l'azimut — rend \
             un écart minimal de {plus_petit:.2}, au-dessus du seuil de \
             {RECONSTITUTION_MINIMALE:.2} qui doit précisément le refuser : la mesure ne voit plus \
             une image reconstituée par translation"
        );
    }
}

#[test]
fn nuee_n_est_reconstituee_par_aucune_translation_le_long_d_un_axe() {
    // Corollaire du test de corrélation, et c'est celui qui aurait attrapé le défaut de `braise` :
    // l'issue dit de `nuee` qu'elle est « un champ de bruit 3D [qui] dérive lentement à travers le
    // boîtier », et le tableau de son § « Comportement attendu » ajoute « aucune n'accepte
    // `direction` : aucune n'a d'axe ».
    //
    // Un motif qui aurait un axe se **reconstituerait** : l'image du pas `n + k`, décalée le long de
    // cet axe, redonnerait celle du pas `n`. C'est la moitié de la propriété que #119 a dû écrire
    // après coup — la promesse d'origine de `braise`, « deux ondes de périodes incommensurables :
    // l'œil n'y voit pas de cycle », valait **dans le temps** et ne disait rien de l'**espace**, qui
    // est justement ce qu'on regarde.
    //
    // ⚠️ La dérive de `nuee` porte sur un **quatrième axe**, pas sur les trois du boîtier :
    // l'approche technique le dit — « Reverb échantillonne le bruit à la position réelle de la LED,
    // la dérive portant sur un quatrième axe ». Un champ qui dériverait le long de `y` serait une
    // onde, pas une nuée, et il tomberait ici.
    //
    // ⚠️ **Ce que ce test ne refuse pas** est écrit en tête de fichier : il ne refuse pas l'onde plane
    // en général — `vague` le passe —, seulement la reconstitution. Voir [`RECONSTITUTION_MINIMALE`]
    // pour les deux populations mesurées.
    let geometrie = geometrie();
    let nuee = animation("nuee");
    let places: Vec<Point> = toutes_les_led(&geometrie)
        .into_iter()
        .map(|(_, _, place)| place)
        .collect();
    let axes = axes(&places);

    for vitesse in [1u8, 5] {
        let reglages = reglages(BLANC, vitesse, Direction::BasHaut);
        let (plus_petit, combien, coupable) = reconstitution(&nuee, &geometrie, &reglages, &axes);
        assert!(
            combien >= COUPLES_MINIMAUX,
            "appareil : « nuee » à la vitesse {vitesse} ne fournit que {combien} couple(s) dont le \
             profil change d'au moins {CHANGEMENT_MINIMAL:.0} sur 255 — le champ ne dérive presque \
             pas, et « rien ne le reconstitue » ne veut alors rien dire"
        );
        assert!(
            plus_petit >= RECONSTITUTION_MINIMALE,
            "« nuee » à la vitesse {vitesse} : {coupable}. Une image antérieure est reconstituée par \
             une simple translation, pour un écart minimal de {RECONSTITUTION_MINIMALE:.2} exigé — \
             le champ défile le long d'un axe du boîtier au lieu de dériver sur un quatrième"
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — `artifice` : des éclats qui se propagent en sphère
// ---------------------------------------------------------------------------

#[test]
fn l_appareil_voit_les_coquilles_spheriques_de_pouls() {
    // Test d'appareil, vert par nature.
    //
    // Ce qu'il établit : [`porte_un_groupe_eloigne`] sait voir une onde sphérique. `pouls` en est
    // une — « une onde sphérique née à la pompe », dont `spec_familles_nouvelles.rs` vérifie déjà que
    // deux LED équidistantes s'y allument ensemble —, et elle doit franchir le seuil que le test
    // suivant impose à `artifice`.
    //
    // ⚠️ Elle ne le franchit que de loin en loin : mesuré, `pouls` porte un tel groupe à 28 % des pas,
    // parce qu'une coquille fine n'allume que quelques LED. C'est ce qui fixe [`PART_AVEC_GROUPE`] à
    // un dixième — voir sa table.
    let geometrie = geometrie();
    let places: Vec<Point> = toutes_les_led(&geometrie)
        .into_iter()
        .map(|(_, _, place)| place)
        .collect();
    let etendue = etendue(&places);
    let pouls = animation("pouls");
    let reglages = reglages(BLANC, 1, Direction::BasHaut);

    let avec = (0..PAS_OBSERVES)
        .filter(|pas| {
            porte_un_groupe_eloigne(
                &intensites(&pouls, &geometrie, &reglages, *pas),
                &places,
                etendue,
            )
        })
        .count();
    let part = avec as f64 / PAS_OBSERVES as f64;
    assert!(
        part > PART_AVEC_GROUPE,
        "appareil : « pouls » — l'onde sphérique du catalogue — ne porte un groupe de LED \
         équidistantes qu'à {part:.3} des pas, sous le seuil de {PART_AVEC_GROUPE:.2} qui doit \
         précisément la reconnaître : la mesure ne voit plus une coquille"
    );
}

#[test]
fn artifice_allume_ensemble_des_led_eloignees_a_la_meme_intensite() {
    // Test d'intention n° 8 de l'issue — « deux LED équidistantes d'une origine ont la même
    // intensité » — et critère d'acceptation : « deux LED **à égale distance** d'une même origine
    // s'allument ensemble, comme dans `pouls` ».
    //
    // ⚠️ **Les origines ne sont pas observables**, et l'issue le veut ainsi : elles naissent d'« un
    // hachage du numéro d'éclat, lui-même déduit de `pas` », dans une fonction pure sans état. Le
    // critère se vérifie donc par sa conséquence — il existe des instants où au moins trois LED
    // **éloignées les unes des autres** portent la même intensité non nulle, à l'arrondi près. C'est
    // exactement la signature d'une coquille sphérique : deux LED que rien ne réunit sauf leur
    // distance à un point.
    //
    // ⚠️ **C'est le test le plus faible du fichier** — une onde plane le satisferait aussi. Il ne
    // vaut qu'accompagné du garde-fou de corrélation ci-dessous et du test de couverture : voir
    // l'en-tête, où cette limite est nommée plutôt que découverte au boîtier.
    let geometrie = geometrie();
    let artifice = animation("artifice");
    let places: Vec<Point> = toutes_les_led(&geometrie)
        .into_iter()
        .map(|(_, _, place)| place)
        .collect();
    let etendue = etendue(&places);

    for vitesse in [1u8, 5] {
        let reglages = reglages(BLANC, vitesse, Direction::BasHaut);
        let avec = (0..PAS_OBSERVES)
            .filter(|pas| {
                porte_un_groupe_eloigne(
                    &intensites(&artifice, &geometrie, &reglages, *pas),
                    &places,
                    etendue,
                )
            })
            .count();
        let part = avec as f64 / PAS_OBSERVES as f64;
        assert!(
            part >= PART_AVEC_GROUPE,
            "« artifice » à la vitesse {vitesse} : sur {PAS_OBSERVES} pas, seuls {avec} portent au \
             moins trois LED allumées de même intensité écartées de plus du tiers du boîtier, soit \
             {part:.3} pour {PART_AVEC_GROUPE:.2} exigés — les éclats ne se propagent pas en sphère, \
             ils font des taches locales"
        );
    }
}

#[test]
fn artifice_fait_eclater_sur_plusieurs_organes() {
    // Test d'intention n° 9 de l'issue — « sur dix mille pas, les origines touchent plus d'un
    // organe » — et critère d'acceptation : « sur une période longue, les origines couvrent
    // **plusieurs organes** — un feu d'artifice qui n'éclaterait que sur un ventilateur serait un
    // défaut ».
    //
    // L'observable est l'organe qui porte la LED **la plus lumineuse** de l'image : au plus près de
    // l'origine d'un éclat en cours, c'est le seul témoin de l'origine dont on dispose de l'extérieur.
    //
    // ⚠️ **Les barrettes comptent, et le test l'exige.** C'est la même raison que `rotation` (#75) :
    // « une RAM éteinte pendant qu'un motif tourne se lirait comme une panne ». Un feu d'artifice qui
    // n'éclaterait que dans le volume des ventilateurs laisserait les quatre barrettes en simple
    // écho, et l'issue nomme les quatorze objets, pas les dix.
    let geometrie = geometrie();
    let artifice = animation("artifice");
    let organes: Vec<usize> = toutes_les_led(&geometrie)
        .into_iter()
        .map(|(organe, _, _)| organe)
        .collect();
    let reglages = reglages(BLANC, 3, Direction::BasHaut);

    let mut touches: HashSet<usize> = HashSet::new();
    for pas in 0..PAS_DE_COUVERTURE {
        if let Some(organe) =
            organe_le_plus_vif(&intensites(&artifice, &geometrie, &reglages, pas), &organes)
        {
            touches.insert(organe);
        }
    }

    let ventilateurs = touches.iter().filter(|organe| **organe < 10).count();
    let barrettes = touches.iter().filter(|organe| **organe >= 10).count();
    let mut vus: Vec<usize> = touches.iter().copied().collect();
    vus.sort_unstable();

    assert!(
        touches.len() >= ORGANES_MINIMAUX,
        "« artifice » : sur {PAS_DE_COUVERTURE} pas, les éclats ne touchent que {} organe(s) sur \
         {ORGANES} — {vus:?} —, pour {ORGANES_MINIMAUX} exigés. Un feu d'artifice qui n'éclate que \
         sur un ventilateur est un défaut",
        touches.len()
    );
    assert!(
        ventilateurs >= 1 && barrettes >= 1,
        "« artifice » : sur {PAS_DE_COUVERTURE} pas, les éclats touchent {ventilateurs} \
         ventilateur(s) et {barrettes} barrette(s) — {vus:?}. Les quatorze objets du boîtier portent \
         des LED, et une RAM qui ne serait jamais l'origine d'un éclat se lirait comme un écho"
    );
}

#[test]
fn artifice_est_un_motif_correle_et_non_un_gresillement() {
    // Garde-fou, sans critère propre dans l'issue, et il existe pour la même raison que celui de
    // `braise` : « ne pas devenir `scintillement` — chaque LED indépendante donne du bruit, pas un
    // feu. C'est la famille voisine, et elle existe déjà. » (#119)
    //
    // Sans lui, le test d'équidistance ci-dessus se satisferait d'un tirage par LED : sur 124 LED
    // arrondies à l'octet, trois qui partagent une intensité à deux unités près, ce n'est pas une
    // coquille, c'est une coïncidence. Une onde sphérique, elle, est corrélée dans l'espace — deux
    // LED proches sont sur la même coquille ou sur deux coquilles voisines.
    let geometrie = geometrie();
    let artifice = animation("artifice");
    let places: Vec<Point> = toutes_les_led(&geometrie)
        .into_iter()
        .map(|(_, _, place)| place)
        .collect();
    let (proches, lointaines) = couples(&places);

    for vitesse in [1u8, 5] {
        let reglages = reglages(BLANC, vitesse, Direction::BasHaut);
        let images: Vec<Vec<f64>> = instants()
            .iter()
            .map(|pas| intensites(&artifice, &geometrie, &reglages, *pas))
            .collect();
        let entre_lointaines = ecart_moyen(&images, &lointaines);
        assert!(
            entre_lointaines >= PLANCHER_CONTRASTE,
            "« artifice » à la vitesse {vitesse} : deux LED aux deux bouts du boîtier ne diffèrent en \
             moyenne que de {entre_lointaines:.1} sur 255 — le boîtier est uni, il n'y a pas d'éclat \
             à mesurer"
        );
        let entre_proches = ecart_moyen(&images, &proches);
        let rapport = entre_proches / entre_lointaines;
        assert!(
            rapport <= RAPPORT_MAXIMAL,
            "« artifice » à la vitesse {vitesse} : deux LED proches diffèrent de {entre_proches:.1} \
             quand deux lointaines diffèrent de {entre_lointaines:.1}, soit un rapport de \
             {rapport:.3} pour {RAPPORT_MAXIMAL:.2} au plus — chaque LED vit sa vie, c'est un \
             grésillement et non un éclat qui se propage"
        );
    }
}

// ---------------------------------------------------------------------------
// 7 — la non-régression, octet pour octet
// ---------------------------------------------------------------------------

/// Trois images relevées sur la branche de #126, à la révision `115e6f8`, **avant** que #127 ne
/// touche à quoi que ce soit.
///
/// C'est le test le plus important de ce fichier, et le seul dont l'énoncé — « les dix familles
/// existantes rendent des images **identiques** à celles d'avant » — ne se vérifie pas contre le
/// code : il se vérifie contre le **passé**. D'où un figeage : les 124 couleurs de trois familles,
/// sous des réglages fixes et à un pas fixe, écrites bout à bout en hexadécimal, 744 caractères par
/// image.
///
/// Ce qu'il protège : sur SHYNAEL, l'état courant est une animation avec ses réglages, et les profils
/// livrés par le dépôt en sont d'autres. Une refonte qui décalerait le rendu d'une unité changerait
/// ce que le boîtier affiche sans qu'aucun autre test ne s'en aperçoive — tous les autres portent sur
/// des propriétés, et une propriété survit à un décalage.
///
/// Les trois familles ne sont pas prises au hasard : ce sont celles dont #127 réemploie la mécanique.
/// `scintillement` tire son aléa d'un hachage du numéro de LED et d'une horloge de période 1021, que
/// `bougie` et `artifice` reprendront ; `pouls` calcule une distance 3D à un point, ce que fera
/// `artifice` ; `respiration` est l'onde ordinaire, le témoin qui ne partage rien avec les trois
/// familles nouvelles et qui doit bouger encore moins qu'elles.
///
/// ⚠️ Elles complètent celles que #126 a figées — `vague`, `comete`, `braise` —, et ne les répètent
/// pas : les deux constantes ensemble couvrent six des dix familles et cinq mécaniques de couleur.
const FIGEES: [(&str, Rgb, u8, Direction, u32, &str); 3] = [
    (
        "respiration",
        Rgb::new(0xff, 0x20, 0x80),
        4,
        Direction::AvantArriere,
        23,
        "8310427b0f3d7d0f3e87114495124a9d134f9b134e911248620c315b0b2d5c0b2e660c33720e397a0f3d780f3c6e0d37\
         580b2c4f0a28450823400820410820480924530a295a0b2d3e071f38071c3106189a134d9b134e3306193a071d3f071f\
         3e071f38071c3106189a134d9b134e3306193a071d3f071f3e071f38071c3106189a134d9b134e3306193a071d3f071f\
         33061937061b36061b31061895124a8c11468e114799134c95124a9d134f9b134e9112488310427b0f3d7d0f3e871144\
         720e397a0f3d780f3c6e0d37620c315b0b2d5c0b2e660c33530a295a0b2d580b2c4f0a28450823400820410820480924\
         650c33650c33650c33650c33650c33650c33650c33650c33650c33650c33650c33630c31630c31630c31630c31630c31\
         630c31630c31630c31630c31630c31630c31610c30610c30610c30610c30610c30610c30610c30610c30610c30610c30\
         610c305e0b2f5e0b2f5e0b2f5e0b2f5e0b2f5e0b2f5e0b2f5e0b2f5e0b2f5e0b2f5e0b2f",
    ),
    (
        "pouls",
        Rgb::new(0x20, 0xff, 0x80),
        6,
        Direction::BasHaut,
        20,
        "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         0000001ffd7f0000000000000000000000000000000000001ef47a15ac5614a0501ad56a000000000000000000000000\
         1bdb6e16b15919c965000000000000000000000000000000094c260842210f7a3d1bda6d0000000000001ff87c12944a\
         000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         0c66330d69340d6b360d6d360d6e370d6e370d6e370d6d360d6b360d69340c6633094f270a52290a532a0a552a0a562b\
         0a562b0a562b0a552a0a532a0a5229094f2707381c073a1d073c1e073d1e073e1f073e1f073e1f073d1e073c1e073a1d\
         07381c042010042211042412042512042513042613042513042512042412042211042010",
    ),
    (
        "scintillement",
        Rgb::new(0x39, 0x39, 0xff),
        2,
        Direction::BasHaut,
        80,
        "3333e41919730000000000002929ba09092c0000000000003333e60000003131dc07071f00000011114d1f1f8e191970\
         3838fc0000000808270000000000011e1e891a1a750000003131df0000000000002424a32c2cc61010490000002828b4\
         0000000808260707200000003333e72d2dcc0000000000002f2fd60f0f470000000000000000000000003636f3000000\
         3333e80303100000000000000000000000000000000000002f2fd30000000d0d3b0000000000000000000000001e1e89\
         14145d0000003838fd0000000000000e0e420000002a2abd3737f614145b0000000000001d1d850505172c2cc8000000\
         0000000000000000000000000000000000000000000000000a0a2f2121950b0b313838fc0000000000002525a73838fa\
         3838fa0000000000000000002222983737f81212510000003030da0000000000003838fe3838fe0000003838fe2929b8\
         0000003434eb0000000000003838fc0000002e2ecf000000000000000000000000000000",
    ),
];

#[test]
fn les_dix_familles_d_avant_rendent_les_images_figees() {
    // Test d'intention n° 10 de l'issue — « les dix familles existantes rendent des images
    // inchangées » — et critère d'acceptation du même nom.
    //
    // ⚠️ **Les octets attendus ont été relevés à la révision `115e6f8`**, en exécutant le code de ce
    // jour-là hors du dépôt. Ce ne sont donc ni des valeurs calculées à la main ni des valeurs relues
    // dans l'implémentation : c'est le comportement d'avant, figé.
    //
    // ⚠️ **On ne corrige jamais cette constante pour faire passer le test.** Si elle diffère après
    // implémentation, c'est que #127 a changé le rendu d'une famille qui ne l'avait pas demandé —
    // exactement ce que l'issue interdit. Le message nomme la première LED qui a bougé.
    //
    // ⚠️ Les réglages sont construits **par structure**, avec `..Reglages::default()`, et non par la
    // porte du décodeur : c'est ce qui garantit qu'aucun champ ajouté d'ici là n'y prend une valeur
    // autre que celle d'absence.
    let geometrie = geometrie();

    for (nom, couleur, vitesse, direction, pas, attendue) in FIGEES {
        let attendue: String = attendue.chars().filter(|c| !c.is_whitespace()).collect();
        assert_eq!(
            attendue.len(),
            LED_DU_BOITIER * 6,
            "appareil : l'empreinte figée de « {nom} » ne porte pas {LED_DU_BOITIER} couleurs"
        );

        let reglages = Reglages {
            couleur,
            vitesse,
            direction,
            ..Reglages::default()
        };
        assert_eq!(
            reglages.palette, None,
            "appareil : les réglages par défaut ne portent aucune palette"
        );

        let rendue = empreinte_hexa(&animation(nom).image(&geometrie, &reglages, pas));
        if rendue == attendue {
            continue;
        }
        let rang = (0..LED_DU_BOITIER)
            .find(|rang| rendue[rang * 6..rang * 6 + 6] != attendue[rang * 6..rang * 6 + 6])
            .expect("deux empreintes qui diffèrent diffèrent sur au moins une LED");
        panic!(
            "« {nom} » (couleur {}, vitesse {vitesse}, direction {}, pas {pas}) ne rend plus l'image \
             d'avant #127 : la LED n° {rang} porte {} au lieu de {} — les dix familles existantes \
             doivent rendre des images identiques à celles d'avant, sans quoi les profils et \
             « eclairage.conf » n'affichent plus la même chose",
            hexa(couleur),
            direction.slug(),
            &rendue[rang * 6..rang * 6 + 6],
            &attendue[rang * 6..rang * 6 + 6],
        );
    }
}
