//! Tests d'intention du sens de défilement des animations (issue #49).
//!
//! Écrits **depuis l'issue #49 et son commentaire seuls**. Aucune ligne de `reverb-anim/src/`
//! n'est relue pour les écrire — ni `projection()`, ni le corps d'une famille, ni la table des
//! centres : ils disent dans quel sens un motif doit traverser le boîtier, pas dans quel sens le
//! code le fait aujourd'hui. Si l'un d'eux échoue après correction, c'est le code qu'on corrige.
//!
//! ## Ce que ce fichier fige
//!
//! 1. **Pour une direction donnée, les six familles défilent dans le même sens.** C'est le cœur de
//!    l'issue : « c'est l'incohérence entre familles qui l'a rendu invisible, pas la valeur d'une
//!    LED prise seule ». `arc-en-ciel` remonte aujourd'hui la direction que les cinq autres
//!    descendent, et cette divergence n'a jamais produit le moindre message.
//! 2. **`avant-arriere` part de l'extrémité de plus grand `z`** — celle du ventilateur `arriere`
//!    et des ventilateurs « gauche » — **et va vers les `z` petits**, ceux du radiateur.
//! 3. **`arriere-avant` est l'exact inverse d'`avant-arriere`** : même axe, même amplitude, sens
//!    opposé.
//! 4. **`bas-haut` part du plancher et monte vers le plafond**, `haut-bas` l'inverse. C'est le
//!    comportement d'aujourd'hui, que la correction ne doit pas emporter avec elle.
//! 5. **`arc-en-ciel` défile comme la tête d'une comète**, critère d'acceptation n° 3.
//!
//! ⚠️ **Le point 2 se lit sur `z`, jamais sur le mot « avant ».** Le commentaire de l'issue tranche
//! que « la correction va sur le nom des directions, pas sur la donnée » : `spec_disposition.rs`
//! (issue #27) a figé « `z` petit veut dire avant » et le relevé physique n'est pas remis en cause.
//! Ce sont donc `Direction::AvantArriere` et `Direction::ArriereAvant` qui changent d'extrémité.
//! Conséquence assumée et voulue : après correction, `avant-arriere` commence du côté du
//! ventilateur nommé `arriere`. Un test qui aurait suivi le mot plutôt que la coordonnée aurait
//! figé exactement le défaut que l'issue corrige.
//!
//! ## L'observable : la phase du fondamental, pas l'instant du maximum
//!
//! Le test d'intention n° 1 de l'issue propose « l'instant du maximum d'intensité ». Il ne
//! convient pas : `arc-en-ciel` fait défiler une **teinte** à luminance quasi constante, et son
//! maximum d'intensité ne dit rien — c'est précisément la famille en cause. Il faut un observable
//! qui marche pour les six.
//!
//! Celui retenu : chaque sonde donne une série temporelle du **vecteur RGB** sur un cycle complet,
//! dont on prend la composante de Fourier de rang 1 (canal par canal). Le produit croisé de deux
//! sondes donne, par son argument, le **retard** de l'une sur l'autre en pas, et par son module
//! une **cohérence** entre 0 et 1. Trois raisons de le préférer à la corrélation croisée :
//!
//! - il rend un retard **continu et signé**, sans argmax à départager quand deux décalages se
//!   valent — cas mesuré sur `braise`, dont la corrélation croisée culmine à 0,999 avec un second
//!   pic à 0,998 ;
//! - il porte sa propre mesure de confiance. Une famille dont la sonde n'est pas une onde simple
//!   le dit, au lieu de rendre un chiffre faux avec assurance ;
//! - il ne suppose ni maximum, ni forme d'onde, ni variation d'intensité : une rotation de teinte
//!   à luminance constante y est aussi lisible qu'une comète.
//!
//! Le cycle est mesuré **à la vitesse 1**, où l'image se répète toutes les 120 étapes — le premier
//! test le vérifie plutôt que de le supposer. C'est ce qui fait de la fenêtre d'observation
//! exactement une période, et donc du rang 1 le fondamental de l'animation. À une vitesse
//! supérieure la fenêtre couvrirait plusieurs cycles et le retard deviendrait ambigu.
//!
//! ## `braise`, qui superposait deux ondes
//!
//! ⚠️ **Retirée du domaine de ce fichier par l'issue #119**, qui refait son rendu et lui retire le
//! réglage `direction` : une famille qui n'accepte aucune direction n'a aucun sens de défilement à
//! mesurer, et [`dirigees`] la laisse partir sans qu'une assertion change. Ce qui suit décrit donc
//! ce qu'elle était — et reste le premier relevé montrant que son motif n'était pas ce qu'on
//! croyait. Le verdict sur les cinq familles qui restent est intact.
//!
//! Son sens de défilement était réel mais bruité : sur certains couples de sondes, la cohérence
//! tombait à 0,15 et le retard mesuré n'avait plus de sens. Ce fichier ne la traitait pas à part,
//! il l'**encadrait** :
//!
//! - un couple de sondes dont la cohérence passe sous [`COHERENCE_MINIMALE`] est déclaré
//!   **inexploitable pour cette famille**, et ne sert à rien conclure ;
//! - mais chaque famille doit rester exploitable sur **au moins un** couple de chaque axe. Sans
//!   cette seconde règle, une famille échapperait au test en devenant illisible partout, ce qui
//!   serait la façon la plus discrète de le désarmer.
//!
//! ## Les couples de sondes, et pourquoi ceux-là
//!
//! Une sonde est un ventilateur, une barrette, ou une LED de barrette ; sa couleur est la moyenne
//! des LED qu'elle observe, sa place leur barycentre. Un couple réunit deux sondes qui **ne
//! diffèrent que par leur place sur un axe** — le deuxième test le vérifie sur la géométrie plutôt
//! que sur ma conviction.
//!
//! Trois pièges dictent ce choix, et le deuxième test les surveille :
//!
//! 1. **Jamais deux LED du même ventilateur.** Six ventilateurs sur dix sont couchés : leurs huit
//!    LED sont à la même hauteur, et un couple pris là-dedans ne mesurerait rien de l'axe
//!    vertical. Les couples relient donc des organes distincts, choisis sur leurs coordonnées
//!    réelles.
//! 2. **Ni deux sondes trop éloignées.** Le retard vit modulo la période : au-delà d'une
//!    demi-période, un retard et une avance sont indiscernables. Deux sondes aux deux bouts du
//!    boîtier — le radiateur et le ventilateur arrière — sont séparées de près d'un cycle entier,
//!    donc de presque rien : mesuré, ce couple rend +12 là où le motif descend. Aucun couple ne
//!    dépasse donc [`ECART_MAXIMAL`] de l'étendue de son axe.
//! 3. **Ni deux sondes trop proches**, sans quoi le retard se noie dans l'arrondi : il doit
//!    dépasser [`RETARD_MINIMAL`].
//!
//! La géométrie testée est `Geometrie::mesuree()`, comme `spec_disposition.rs` : c'est le boîtier
//! réel dont Nico juge le rendu à l'œil, et une géométrie décodée n'aurait aucune coordonnée à
//! comparer.
//!
//! ## Quatre rouges et trois verts
//!
//! Les tests des points 1, 2, 3 et 5 doivent échouer avant la correction — c'est ce qui prouve
//! qu'ils décrivent le défaut. Celui du point 4 doit passer avant comme après : c'est un
//! garde-fou, pas une correction. Il **exclut `arc-en-ciel`**, qui est à contresens sur l'axe
//! vertical aussi ; l'y inclure ferait de ce garde-fou un rouge de plus et lui ôterait sa fonction,
//! alors que le test du point 5 le couvre déjà, en le nommant.
//!
//! S'y ajoutent deux tests d'appareillage, verts par nature : ils vérifient que la fenêtre
//! d'observation est bien une période et que les couples de sondes isolent bien leur axe. Une
//! mesure faite sur un appareil faux ne dit rien, et ne le signale pas.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi.

use reverb_anim::{Animation, CATALOGUE, Direction, Geometrie, Image, Point, Reglages};
use reverb_proto::ram::LEDS_PER_STICK;
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// Les seuils de la mesure
// ---------------------------------------------------------------------------

/// Durée d'un cycle, en pas, à la vitesse 1.
///
/// Vérifiée par le premier test plutôt que supposée : c'est elle qui fait de la fenêtre
/// d'observation exactement une période.
const PERIODE: u32 = 120;

/// En deçà, la sonde n'est pas une onde simple et son retard ne veut rien dire.
///
/// Mesuré : `braise` descend à 0,15 sur certains couples — un chiffre rendu avec assurance et
/// faux. Au-dessus de ce seuil, les mesures relevées vont de 0,66 à 1,00.
const COHERENCE_MINIMALE: f64 = 0.5;

/// En deçà, deux sondes sont trop proches pour que leur ordre soit lisible : un demi-pas sur cent
/// vingt.
///
/// Ce n'est pas une exigence sur l'amplitude — une famille peut légitimement n'avoir qu'un faible
/// gradient — mais le refus d'un retard nul, dont le signe ne voudrait rien dire.
const RETARD_MINIMAL: f64 = 0.5;

/// Part de l'étendue d'un axe qu'un couple de sondes ne doit pas dépasser.
///
/// Le retard vit modulo la période : au-delà d'une demi-période, il se confond avec une avance.
/// Deux cinquièmes laissent de la marge sous cette moitié, y compris pour une famille dont le
/// gradient est plus raide que celui d'une onde plane.
const ECART_MAXIMAL: f32 = 0.4;

/// Écart toléré, en pas, entre deux mesures qui doivent être exactement opposées.
///
/// Deux pas sur cent vingt. Relevé : les couples mesurés s'opposent à 0,2 pas près.
const TOLERANCE_OPPOSE: f64 = 2.0;

/// Écart toléré, en unités de géométrie, sur les deux coordonnées qu'un couple n'explore pas.
const TOLERANCE_HORS_AXE: f32 = 1.0;

// ---------------------------------------------------------------------------
// Les axes
// ---------------------------------------------------------------------------

/// L'axe qu'un couple de sondes explore.
///
/// `Horaire` et `Antihoraire` n'y figurent pas : elles tournent dans le plan `x`/`y` et l'issue
/// les met explicitement hors scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axe {
    /// Du plancher au plafond.
    Hauteur,
    /// De l'avant du boîtier vers le fond.
    Profondeur,
}

impl Axe {
    fn nom(self) -> &'static str {
        match self {
            Axe::Hauteur => "hauteur",
            Axe::Profondeur => "profondeur",
        }
    }

    /// La coordonnée que cet axe mesure.
    fn coordonnee(self, place: Point) -> f32 {
        match self {
            Axe::Hauteur => place.y,
            Axe::Profondeur => place.z,
        }
    }

    /// Les deux coordonnées qu'un couple ne doit pas explorer.
    fn hors_axe(self, place: Point) -> [f32; 2] {
        match self {
            Axe::Hauteur => [place.x, place.z],
            Axe::Profondeur => [place.x, place.y],
        }
    }

    /// L'étendue du boîtier le long de cet axe.
    fn etendue(self, geometrie: &Geometrie) -> f32 {
        let (bas, haut) = geometrie.bornes();
        self.coordonnee(haut) - self.coordonnee(bas)
    }

    /// La direction qui doit conduire le motif **des grandes coordonnées vers les petites**.
    ///
    /// `haut-bas` descend du plafond vers le plancher : personne ne le conteste.
    /// `avant-arriere` part de l'extrémité de plus grand `z` et va vers le radiateur : c'est la
    /// correction que l'issue demande, et c'est aujourd'hui l'inverse.
    fn vers_les_petites(self) -> Direction {
        match self {
            Axe::Hauteur => Direction::HautBas,
            Axe::Profondeur => Direction::AvantArriere,
        }
    }

    /// La direction opposée à [`Axe::vers_les_petites`], sur le même axe.
    fn vers_les_grandes(self) -> Direction {
        match self {
            Axe::Hauteur => Direction::BasHaut,
            Axe::Profondeur => Direction::ArriereAvant,
        }
    }
}

/// Le sens dans lequel un motif doit parcourir un axe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parcours {
    VersLesPetites,
    VersLesGrandes,
}

// ---------------------------------------------------------------------------
// Les sondes
// ---------------------------------------------------------------------------

/// Un endroit du boîtier qu'on observe : sa couleur moyenne, et la place de ce qu'il observe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sonde {
    /// Les huit LED d'un ventilateur.
    Ventilateur(Position),
    /// Les onze LED d'une barrette.
    Barrette(usize),
    /// Une LED de barrette, seule.
    LedDeBarrette(usize, usize),
}

impl Sonde {
    fn nom(self) -> String {
        match self {
            Sonde::Ventilateur(position) => position.slug().to_owned(),
            Sonde::Barrette(slot) => format!("barrette {slot}"),
            Sonde::LedDeBarrette(slot, led) => format!("barrette {slot} led {led}"),
        }
    }

    /// Les places, dans le boîtier, des LED que cette sonde observe.
    fn places(self, geometrie: &Geometrie) -> Vec<Point> {
        let manquante = |quoi: String| panic!("{quoi} n'a pas de place dans le boîtier");
        match self {
            Sonde::Ventilateur(position) => (0..LEDS_PER_FAN as usize)
                .map(|led| {
                    geometrie
                        .led_ventilateur(position, led)
                        .unwrap_or_else(|| manquante(format!("{} led {led}", position.slug())))
                })
                .collect(),
            Sonde::Barrette(slot) => (0..LEDS_PER_STICK)
                .map(|led| {
                    geometrie
                        .led_barrette(slot, led)
                        .unwrap_or_else(|| manquante(format!("barrette {slot} led {led}")))
                })
                .collect(),
            Sonde::LedDeBarrette(slot, led) => vec![
                geometrie
                    .led_barrette(slot, led)
                    .unwrap_or_else(|| manquante(format!("barrette {slot} led {led}"))),
            ],
        }
    }

    /// Le barycentre de ce qu'elle observe : la place à laquelle sa couleur se rapporte.
    fn place(self, geometrie: &Geometrie) -> Point {
        let places = self.places(geometrie);
        let combien = places.len() as f32;
        Point {
            x: places.iter().map(|p| p.x).sum::<f32>() / combien,
            y: places.iter().map(|p| p.y).sum::<f32>() / combien,
            z: places.iter().map(|p| p.z).sum::<f32>() / combien,
        }
    }

    /// Sa couleur dans une image : la moyenne des LED qu'elle observe, canal par canal.
    fn couleur(self, image: &Image) -> [f64; 3] {
        let couleurs: &[Rgb] = match self {
            Sonde::Ventilateur(position) => couleurs_du_ventilateur(image, position),
            Sonde::Barrette(slot) => &image.barrettes[slot],
            Sonde::LedDeBarrette(slot, led) => std::slice::from_ref(&image.barrettes[slot][led]),
        };
        let combien = couleurs.len() as f64;
        let mut somme = [0.0f64; 3];
        for couleur in couleurs {
            somme[0] += f64::from(couleur.r);
            somme[1] += f64::from(couleur.g);
            somme[2] += f64::from(couleur.b);
        }
        [somme[0] / combien, somme[1] / combien, somme[2] / combien]
    }
}

/// Les huit couleurs d'un ventilateur dans une image, cherchées **par position**.
///
/// Jamais par indice de tableau : l'`Image` porte la position à côté des couleurs précisément pour
/// que personne n'ait à connaître l'ordre du tableau.
fn couleurs_du_ventilateur(image: &Image, position: Position) -> &[Rgb; LEDS_PER_FAN as usize] {
    let (_, couleurs) = image
        .ventilateurs
        .iter()
        .find(|(p, _)| *p == position)
        .unwrap_or_else(|| panic!("l'image ne contient pas {}", position.slug()));
    couleurs
}

// ---------------------------------------------------------------------------
// Le banc de sondes
// ---------------------------------------------------------------------------

/// Deux sondes qui ne diffèrent que par leur place sur un axe.
#[derive(Debug, Clone, Copy)]
struct Couple {
    axe: Axe,
    une: Sonde,
    autre: Sonde,
}

impl Couple {
    /// Les deux sondes, **celle de plus grande coordonnée d'abord**.
    ///
    /// L'ordre vient de la géométrie mesurée, jamais d'une conviction sur l'endroit où se trouve
    /// tel ventilateur : c'est ce qui rend ces tests insensibles à une remesure du boîtier, et ce
    /// qui les empêche de figer la table des centres au lieu du sens de défilement.
    fn ordonne(self, geometrie: &Geometrie) -> (Sonde, Sonde) {
        let une = self.axe.coordonnee(self.une.place(geometrie));
        let autre = self.axe.coordonnee(self.autre.place(geometrie));
        if une >= autre {
            (self.une, self.autre)
        } else {
            (self.autre, self.une)
        }
    }

    /// Ce qui sépare les deux sondes le long de l'axe.
    fn ecart(self, geometrie: &Geometrie) -> f32 {
        let (grande, petite) = self.ordonne(geometrie);
        self.axe.coordonnee(grande.place(geometrie)) - self.axe.coordonnee(petite.place(geometrie))
    }

    fn nom(self, geometrie: &Geometrie) -> String {
        let (grande, petite) = self.ordonne(geometrie);
        format!("{} → {}", grande.nom(), petite.nom())
    }
}

/// Les couples de sondes d'un axe.
///
/// Chacun relie deux organes distincts qui partagent leurs deux autres coordonnées, à un écart
/// que le deuxième test vérifie compris entre « lisible » et « une demi-période ». Les barrettes
/// y figurent parce que la RAM subit les mêmes directions que le boîtier — l'issue #19 : « une
/// onde qui monte atteint en même temps deux LED à la même hauteur, quels que soient leur
/// ventilateur, leur barrette et leur numéro d'ordre ».
fn banc(axe: Axe) -> Vec<Couple> {
    let couples: Vec<(Sonde, Sonde)> = match axe {
        Axe::Profondeur => vec![
            (
                Sonde::Ventilateur(Position::BasGauche),
                Sonde::Ventilateur(Position::BasMilieu),
            ),
            (
                Sonde::Ventilateur(Position::HautGauche),
                Sonde::Ventilateur(Position::HautMilieu),
            ),
            (
                Sonde::Ventilateur(Position::BasMilieu),
                Sonde::Ventilateur(Position::BasDroite),
            ),
            (
                Sonde::Ventilateur(Position::HautMilieu),
                Sonde::Ventilateur(Position::HautDroite),
            ),
            (Sonde::Barrette(0), Sonde::Barrette(3)),
        ],
        Axe::Hauteur => vec![
            (
                Sonde::Ventilateur(Position::RadiateurHaut),
                Sonde::Ventilateur(Position::RadiateurMilieu),
            ),
            (
                Sonde::Ventilateur(Position::RadiateurMilieu),
                Sonde::Ventilateur(Position::RadiateurBas),
            ),
            (
                Sonde::LedDeBarrette(1, LEDS_PER_STICK - 1),
                Sonde::LedDeBarrette(1, 0),
            ),
        ],
    };
    couples
        .into_iter()
        .map(|(une, autre)| Couple { axe, une, autre })
        .collect()
}

// ---------------------------------------------------------------------------
// La mesure : phase du fondamental
// ---------------------------------------------------------------------------

/// Ce qu'une paire de sondes rend : un retard, et de quoi savoir s'il veut dire quelque chose.
#[derive(Debug, Clone, Copy)]
struct Mesure {
    /// Retard de la sonde d'arrivée sur celle de départ, en pas, dans `]-60, 60]`.
    ///
    /// Positif : l'arrivée est atteinte **après** le départ.
    retard: f64,
    /// De 0 à 1. Vaut 1 quand les deux sondes portent la même onde à un décalage près.
    coherence: f64,
}

impl Mesure {
    fn exploitable(self) -> bool {
        self.coherence >= COHERENCE_MINIMALE
    }
}

/// La série temporelle d'une sonde sur un cycle complet, à la vitesse 1.
///
/// La couleur reste celle des réglages par défaut : le sens de défilement ne doit pas en dépendre,
/// et c'est ce qu'un utilisateur obtient en tapant `animate braise direction=avant-arriere`.
fn serie(
    animation: &Animation,
    geometrie: &Geometrie,
    direction: Direction,
    sonde: Sonde,
) -> Vec<[f64; 3]> {
    let reglages = Reglages {
        vitesse: 1,
        direction,
        ..Reglages::default()
    };
    (0..PERIODE)
        .map(|pas| sonde.couleur(&animation.image(geometrie, &reglages, pas)))
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

/// Le retard de `arrivee` sur `depart`, et la confiance qu'on peut lui accorder.
///
/// Si `arrivee(t) = depart(t - d)`, le produit croisé des fondamentaux vaut `|F|² e^{-i·2π·d/N}` :
/// son argument donne `d`, son module rapporté aux deux énergies donne la cohérence.
fn retard(depart: &[[f64; 3]], arrivee: &[[f64; 3]]) -> Mesure {
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
    Mesure {
        retard: -imaginaire.atan2(reel) * f64::from(PERIODE) / std::f64::consts::TAU,
        coherence,
    }
}

/// Le retard de la sonde de petite coordonnée sur celle de grande coordonnée.
///
/// Positif : le motif **descend** l'axe, des grandes coordonnées vers les petites.
fn mesure(
    animation: &Animation,
    geometrie: &Geometrie,
    direction: Direction,
    couple: Couple,
) -> Mesure {
    let (grande, petite) = couple.ordonne(geometrie);
    retard(
        &serie(animation, geometrie, direction, grande),
        &serie(animation, geometrie, direction, petite),
    )
}

// ---------------------------------------------------------------------------
// Aides communes aux tests
// ---------------------------------------------------------------------------

/// Le boîtier tel que la mesure le déclare.
fn boitier() -> Geometrie {
    Geometrie::mesuree()
}

/// Les animations du catalogue, ouvertes par leur nom.
fn catalogue() -> Vec<(&'static str, Animation)> {
    CATALOGUE
        .iter()
        .map(|nom| {
            let animation = Animation::par_nom(nom)
                .unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"));
            (*nom, animation)
        })
        .collect()
}

/// Les familles qui **suivent une direction du boîtier**.
///
/// ⚠️ **Domaine restreint par #75, et l'intention est intacte.** Quand ce fichier a été écrit, les
/// six familles du catalogue acceptaient toutes le réglage `direction` : « les six familles » et
/// « le catalogue » désignaient le même ensemble.
///
/// #75 en ajoute quatre qui n'en suivent aucune, et le refusent explicitement — `rotation` suit le
/// montage relevé de chaque anneau, `pouls` la distance à la pompe, `scintillement` le hasard,
/// `thermique` une sonde. Leur demander de défiler dans le sens d'une direction qu'elles
/// n'acceptent pas n'aurait aucun sens : la question ne se pose pas pour elles.
///
/// ⚠️ **#119 en retire une cinquième, et celle-là en revient : `braise`.** Elle défilait bel et bien
/// le long de la direction demandée — deux ondes planes superposées —, et c'est justement le défaut
/// que l'issue corrige : « un lit de braises n'a pas d'axe ». Elle ne l'accepte donc plus, et ce
/// filtre la laisse partir sans qu'une ligne change ici. C'est une **spécification remplacée par une
/// autre**, décidée dans l'issue, et non un test plié à une implémentation.
///
/// Le filtre lit `parametres_acceptes`, seule source de vérité : une famille qui gagnerait un jour
/// le réglage `direction` rejoindrait ces tests d'elle-même, et celle qui le perd en sort de même —
/// ce que #119 vient de démontrer.
fn dirigees() -> Vec<&'static str> {
    CATALOGUE
        .iter()
        .copied()
        .filter(|nom| {
            Animation::par_nom(nom)
                .expect("le catalogue s'ouvre")
                .parametres_acceptes()
                .contains(&"direction")
        })
        .collect()
}

/// Les mêmes, ouvertes.
fn catalogue_dirige() -> Vec<(&'static str, Animation)> {
    catalogue()
        .into_iter()
        .filter(|(nom, _)| dirigees().contains(nom))
        .collect()
}

/// Exige qu'une direction conduise le motif dans le sens attendu, pour les familles données.
///
/// Chaque couple de sondes exploitable doit conclure dans le bon sens, et chaque famille doit être
/// exploitable sur au moins un couple : une famille illisible partout serait une façon discrète de
/// désarmer le test.
fn exige_le_sens(axe: Axe, direction: Direction, parcours: Parcours, familles: &[&str]) {
    let geometrie = boitier();
    let banc = banc(axe);

    for nom in familles {
        let animation = Animation::par_nom(nom)
            .unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"));
        let mut exploitables = 0;
        let mut inexploitables = Vec::new();

        for couple in &banc {
            let observee = mesure(&animation, &geometrie, direction, *couple);
            if !observee.exploitable() {
                inexploitables.push(format!(
                    "{} (cohérence {:.2})",
                    couple.nom(&geometrie),
                    observee.coherence
                ));
                continue;
            }
            exploitables += 1;

            let (attendu, sens) = match parcours {
                Parcours::VersLesPetites => (
                    observee.retard > RETARD_MINIMAL,
                    "des grandes coordonnées vers les petites",
                ),
                Parcours::VersLesGrandes => (
                    observee.retard < -RETARD_MINIMAL,
                    "des petites coordonnées vers les grandes",
                ),
            };
            assert!(
                attendu,
                "« {nom} » en {} : sur l'axe {}, {} rend un retard de {:+.1} pas (cohérence \
                 {:.2}) alors que le motif doit aller {sens}. Un retard positif veut dire que la \
                 sonde de petite coordonnée est atteinte après celle de grande coordonnée.",
                direction.slug(),
                axe.nom(),
                couple.nom(&geometrie),
                observee.retard,
                observee.coherence
            );
        }

        assert!(
            exploitables > 0,
            "« {nom} » en {} : aucun couple de sondes de l'axe {} n'est exploitable — {}. Sans \
             une seule mesure lisible, cette famille échappe au test.",
            direction.slug(),
            axe.nom(),
            inexploitables.join(", ")
        );
    }
}

// ---------------------------------------------------------------------------
// Appareillage — deux tests verts par nature
// ---------------------------------------------------------------------------

#[test]
fn a_la_vitesse_un_le_motif_boucle_en_cent_vingt_pas_et_bouge_entre_temps() {
    // La fenêtre d'observation de tout ce fichier est de 120 pas. Si l'image ne bouclait pas
    // exactement là, le rang 1 du spectre ne serait plus le fondamental de l'animation et tous les
    // retards mesurés plus bas seraient des chiffres sans objet.
    //
    // La seconde moitié du test est aussi importante que la première : une animation figée boucle
    // en 120 pas comme n'importe quelle autre, et satisferait n'importe quelle exigence de sens
    // faute d'en avoir un. Il faut donc qu'elle bouge.
    // ⚠️ **`scintillement` en est exclue, et c'est sa définition même** (#75) : elle est la seule
    // famille **sans période**. Son horloge court sur 1021 pas — un nombre premier, donc étranger
    // à toutes les vitesses — précisément pour qu'aucun cycle ne s'y installe. Lui demander de
    // boucler en 120 pas serait lui demander de ne plus scintiller.
    //
    // ⚠️ **`braise` l'a rejointe (#119), et pour la même raison.** Elle promettait déjà « deux ondes
    // de périodes incommensurables : l'œil n'y voit pas de cycle », et la promesse était fausse d'un
    // cycle au suivant — elle défilait sur `temps`, qui se replie sur cent vingt pas, donc elle se
    // refermait exactement ici. Elle défile désormais sur la même horloge que `scintillement`, celle
    // qui ne se replie pas, et `spec_braise_sans_axe.rs` l'**exige** plutôt que de le tolérer :
    // vingt instants séparés d'une période exacte doivent rendre dix images distinctes au moins.
    //
    // Ce test ne perd rien à son départ : il appareille une mesure de sens, et `braise` n'a plus de
    // sens à mesurer — elle n'accepte plus de direction, donc [`dirigees`] ne la rend déjà plus aux
    // tests qui suivent.
    //
    // ⚠️ **Les trois de #127 les rejoignent, et c'est la même raison, une troisième fois.**
    // `bougie` fait vaciller chaque LED sur sa propre cadence, `nuee` déforme un champ de bruit sur
    // une quatrième dimension, `artifice` date ses éclats : les trois lisent [`derive`], l'horloge
    // de 1021 pas qui ne se replie pas sur le cycle. Leur demander de boucler en 120 pas serait leur
    // demander de redevenir périodiques — c'est-à-dire de cesser d'être ce que l'issue décrit.
    //
    // Elles ne perdent rien non plus à ce départ : aucune n'accepte de direction, donc [`dirigees`]
    // ne les rend déjà pas aux tests qui suivent.
    //
    // Les sept autres bouclent bien en 120 pas, `thermique` comprise : faute de sonde, elle pulse
    // en blanc sur le cycle ordinaire.
    let geometrie = boitier();

    /// Les familles qui lisent [`derive`] plutôt que [`temps`], donc sans période de 120 pas.
    const SANS_PERIODE: [&str; 5] = ["scintillement", "braise", "bougie", "nuee", "artifice"];

    for (nom, animation) in catalogue()
        .into_iter()
        .filter(|(nom, _)| !SANS_PERIODE.contains(nom))
    {
        for axe in [Axe::Hauteur, Axe::Profondeur] {
            for direction in [axe.vers_les_petites(), axe.vers_les_grandes()] {
                let reglages = Reglages {
                    vitesse: 1,
                    direction,
                    ..Reglages::default()
                };
                let depart = animation.image(&geometrie, &reglages, 0);
                assert_eq!(
                    depart,
                    animation.image(&geometrie, &reglages, PERIODE),
                    "« {nom} » en {} à la vitesse 1 : l'image du pas {PERIODE} diffère de celle du \
                     pas 0, la période n'est pas de {PERIODE} pas",
                    direction.slug()
                );
                assert!(
                    (1..PERIODE).any(|pas| animation.image(&geometrie, &reglages, pas) != depart),
                    "« {nom} » en {} à la vitesse 1 : l'image ne change pas d'un pas à l'autre du \
                     cycle — rien n'y défile, donc rien n'y a de sens",
                    direction.slug()
                );
            }
        }
    }
}

#[test]
fn chaque_couple_de_sondes_isole_bien_son_axe() {
    // Une mesure faite sur un appareil faux ne dit rien, et ne le signale pas. Ce test vérifie sur
    // la géométrie — pas sur ma conviction — que chaque couple :
    //
    // 1. relie deux organes **distincts**, jamais deux LED du même ventilateur, dont six sur dix
    //    sont couchés et n'ont aucune épaisseur verticale ;
    // 2. partage ses deux autres coordonnées, sans quoi il mesurerait un mélange d'axes ;
    // 3. reste séparé d'assez peu pour que son retard ne se confonde pas avec une avance, et
    //    d'assez pour que ce retard soit lisible.
    let geometrie = boitier();

    for axe in [Axe::Hauteur, Axe::Profondeur] {
        let etendue = axe.etendue(&geometrie);
        assert!(
            etendue > 0.0 && etendue.is_finite(),
            "l'axe {} a une étendue de {etendue} : sans étendue, aucun écart n'est comparable",
            axe.nom()
        );

        for couple in banc(axe) {
            assert_ne!(
                couple.une,
                couple.autre,
                "l'axe {} contient un couple de sondes identiques",
                axe.nom()
            );

            let (grande, petite) = couple.ordonne(&geometrie);
            let (une, autre) = (grande.place(&geometrie), petite.place(&geometrie));
            for (mesuree, attendue) in axe.hors_axe(une).iter().zip(axe.hors_axe(autre).iter()) {
                assert!(
                    (mesuree - attendue).abs() <= TOLERANCE_HORS_AXE,
                    "sur l'axe {}, {} diffère aussi hors de l'axe ({mesuree} contre {attendue}) : \
                     ce couple mesurerait un mélange d'axes",
                    axe.nom(),
                    couple.nom(&geometrie)
                );
            }

            let part = couple.ecart(&geometrie) / etendue;
            assert!(
                part > 0.0,
                "sur l'axe {}, {} ne sépare rien du tout",
                axe.nom(),
                couple.nom(&geometrie)
            );
            assert!(
                part <= ECART_MAXIMAL,
                "sur l'axe {}, {} couvre {:.0} % de l'étendue : au-delà de {:.0} %, son retard \
                 risque de se confondre avec une avance",
                axe.nom(),
                couple.nom(&geometrie),
                part * 100.0,
                ECART_MAXIMAL * 100.0
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 1 — les six familles défilent dans le même sens
// ---------------------------------------------------------------------------

#[test]
fn les_six_familles_defilent_dans_le_meme_sens() {
    // Test d'intention n° 2 de l'issue — « Les six familles s'accordent sur ce sens, deux à
    // deux » —, et son cœur : « c'est l'incohérence entre familles qui l'a rendu invisible, pas la
    // valeur d'une LED prise seule ».
    //
    // Ce test ne dit rien du bon sens : il exige seulement qu'il n'y en ait qu'un. C'est
    // volontaire — il reste vrai quelle que soit l'extrémité que l'on décide d'appeler l'avant, et
    // il attrape donc le défaut d'`arc-en-ciel` indépendamment de la correction de l'axe.
    let geometrie = boitier();
    let familles = catalogue_dirige();

    for axe in [Axe::Hauteur, Axe::Profondeur] {
        for direction in [axe.vers_les_petites(), axe.vers_les_grandes()] {
            for couple in banc(axe) {
                let mut sens: Vec<(&str, f64, f64)> = Vec::new();
                for (nom, animation) in &familles {
                    let observee = mesure(animation, &geometrie, direction, couple);
                    if observee.exploitable() {
                        sens.push((nom, observee.retard, observee.coherence));
                    }
                }
                if sens.len() < 2 {
                    continue;
                }

                let descendent = sens.iter().filter(|(_, r, _)| *r > 0.0).count();
                assert!(
                    descendent == 0 || descendent == sens.len(),
                    "en {}, sur {} : les familles ne défilent pas toutes dans le même sens — {}. \
                     Un retard positif et un retard négatif sur le même couple de sondes, ce sont \
                     deux motifs qui se croisent.",
                    direction.slug(),
                    couple.nom(&geometrie),
                    sens.iter()
                        .map(|(nom, retard, coherence)| format!(
                            "{nom} {retard:+.1} pas (cohérence {coherence:.2})"
                        ))
                        .collect::<Vec<String>>()
                        .join(", ")
                );

                for (nom, retard, coherence) in &sens {
                    assert!(
                        retard.abs() > RETARD_MINIMAL,
                        "en {}, sur {} : « {nom} » rend un retard de {retard:+.1} pas (cohérence \
                         {coherence:.2}), trop petit pour avoir un sens",
                        direction.slug(),
                        couple.nom(&geometrie)
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 2 — avant-arriere part du fond du boîtier
// ---------------------------------------------------------------------------

#[test]
fn avant_arriere_part_du_grand_z_et_descend_vers_le_radiateur() {
    // Critère d'acceptation n° 1 de l'issue, moitié `avant-arriere` : « la LED la plus « avant »
    // s'allume avant la plus « arrière » ».
    //
    // ⚠️ Lu sur la coordonnée, jamais sur le mot : le commentaire de l'issue tranche que ce sont
    // les **noms des directions** qui sont attachés aux mauvaises extrémités, `spec_disposition.rs`
    // ayant déjà figé « z petit veut dire avant ». `avant-arriere` doit donc partir de l'extrémité
    // de plus grand `z` — celle du ventilateur `arriere` et des ventilateurs « gauche » — et
    // descendre vers le radiateur.
    //
    // Les six familles, sans exception : le critère les nomme toutes.
    exige_le_sens(
        Axe::Profondeur,
        Direction::AvantArriere,
        Parcours::VersLesPetites,
        &dirigees(),
    );
}

// ---------------------------------------------------------------------------
// 3 — arriere-avant est l'exact inverse
// ---------------------------------------------------------------------------

#[test]
fn arriere_avant_remonte_vers_le_grand_z() {
    // Critère d'acceptation n° 1 de l'issue, moitié `arriere-avant` : « et l'inverse en
    // arriere-avant ».
    //
    // Le tenir séparément de l'exactitude de l'opposition (test suivant) a son importance : deux
    // directions peuvent être exactement opposées l'une à l'autre **et toutes deux du mauvais
    // côté**. C'est d'ailleurs l'état actuel du code, et c'est ce qui rend ce défaut si discret.
    exige_le_sens(
        Axe::Profondeur,
        Direction::ArriereAvant,
        Parcours::VersLesGrandes,
        &dirigees(),
    );
}

#[test]
fn arriere_avant_est_l_exact_oppose_d_avant_arriere() {
    // Test d'intention n° 3 de l'issue — « avant-arriere et arriere-avant sont exactement l'inverse
    // l'une de l'autre ».
    //
    // Même axe, même amplitude, sens opposé : ce qui interdit une correction qui n'aurait retourné
    // qu'une des deux directions, et laisserait les deux descendre le boîtier.
    let geometrie = boitier();

    for (nom, animation) in catalogue_dirige() {
        let mut comparees = 0;
        for couple in banc(Axe::Profondeur) {
            let aller = mesure(&animation, &geometrie, Direction::AvantArriere, couple);
            let retour = mesure(&animation, &geometrie, Direction::ArriereAvant, couple);
            if !aller.exploitable() || !retour.exploitable() {
                continue;
            }
            comparees += 1;
            assert!(
                (aller.retard + retour.retard).abs() <= TOLERANCE_OPPOSE,
                "« {nom} » sur {} : avant-arriere rend {:+.1} pas et arriere-avant {:+.1} pas. \
                 Les deux directions doivent être exactement opposées, à {TOLERANCE_OPPOSE} pas \
                 près sur {PERIODE}.",
                couple.nom(&geometrie),
                aller.retard,
                retour.retard
            );
        }
        assert!(
            comparees > 0,
            "« {nom} » : aucun couple de sondes de l'axe profondeur n'est exploitable dans les \
             deux directions à la fois"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — l'axe vertical ne régresse pas
// ---------------------------------------------------------------------------

#[test]
fn bas_haut_monte_du_plancher_et_haut_bas_descend_du_plafond() {
    // Critère d'acceptation n° 2 de l'issue — « Le même test passe pour bas-haut / haut-bas, qui
    // ne doivent pas régresser ».
    //
    // ⚠️ Garde-fou, pas correction : ce test est vert avant la correction et doit le rester. Son
    // rôle est d'attraper une correction qui retournerait l'axe vertical en même temps que l'axe
    // de profondeur — la façon la plus facile de se tromper quand on inverse un signe dans une
    // projection commune aux deux.
    //
    // ⚠️ `arc-en-ciel` en est **exclue**, et elle seule : elle est à contresens sur l'axe vertical
    // aussi, donc l'inclure ici ferait de ce garde-fou un rouge de plus et lui ôterait sa fonction.
    // Elle est couverte par le test suivant, qui la nomme.
    //
    // ⚠️ Elles étaient cinq jusqu'à #119, qui a retiré `braise` de [`dirigees`] — voir là-bas. La
    // variable ne porte donc plus un compte, seulement ce qu'elle retire : un nom qui compte se
    // met à mentir dès qu'une famille bouge, et deux l'ont fait en deux issues.
    let sans_arc_en_ciel: Vec<&str> = dirigees()
        .into_iter()
        .filter(|nom| *nom != "arc-en-ciel")
        .collect();
    assert_eq!(
        sans_arc_en_ciel.len(),
        dirigees().len() - 1,
        "« arc-en-ciel » doit être au catalogue pour en être exclue ici"
    );

    exige_le_sens(
        Axe::Hauteur,
        Direction::BasHaut,
        Parcours::VersLesGrandes,
        &sans_arc_en_ciel,
    );
    exige_le_sens(
        Axe::Hauteur,
        Direction::HautBas,
        Parcours::VersLesPetites,
        &sans_arc_en_ciel,
    );
}

// ---------------------------------------------------------------------------
// 5 — arc-en-ciel défile comme la comète
// ---------------------------------------------------------------------------

#[test]
fn arc_en_ciel_defile_comme_la_tete_d_une_comete() {
    // Critère d'acceptation n° 3 de l'issue — « arc-en-ciel fait défiler sa teinte dans le même
    // sens que la tête d'une comète ».
    //
    // Le test précédent le dirait déjà, noyé parmi six familles ; celui-ci le dit en nommant les
    // deux fautives possibles, sur les quatre directions d'axe. Une teinte qui remonte la direction
    // demandée n'est pas une couleur fausse : c'est un mouvement à contresens, et rien dans le
    // rendu ne le signale.
    let geometrie = boitier();
    let arc = Animation::par_nom("arc-en-ciel").expect("« arc-en-ciel » est au catalogue");
    let comete = Animation::par_nom("comete").expect("« comete » est au catalogue");

    for axe in [Axe::Hauteur, Axe::Profondeur] {
        for direction in [axe.vers_les_petites(), axe.vers_les_grandes()] {
            for couple in banc(axe) {
                let teinte = mesure(&arc, &geometrie, direction, couple);
                let tete = mesure(&comete, &geometrie, direction, couple);
                if !teinte.exploitable() || !tete.exploitable() {
                    continue;
                }
                assert_eq!(
                    teinte.retard > 0.0,
                    tete.retard > 0.0,
                    "en {}, sur {} : « arc-en-ciel » rend {:+.1} pas et « comete » {:+.1} pas — le \
                     spectre remonte la direction que la comète descend",
                    direction.slug(),
                    couple.nom(&geometrie),
                    teinte.retard,
                    tete.retard
                );
            }
        }
    }
}
