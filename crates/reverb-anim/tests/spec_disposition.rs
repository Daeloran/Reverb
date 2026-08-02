//! Tests d'intention de la disposition du boîtier en profondeur (issue #27).
//!
//! Écrits **avant** la correction, depuis l'issue #27 seule. Aucune ligne de `reverb-anim/src/`
//! n'est relue pour les écrire — ni `CENTRES`, ni `RAM_Z`, ni un corps de fonction : ils disent où
//! les ventilateurs doivent être les uns par rapport aux autres, pas où la table les met
//! aujourd'hui. Si l'un d'eux échoue après correction, c'est la table qu'on corrige.
//!
//! ## Ce que ce fichier spécifie
//!
//! Le repère du boîtier : `x` d'un flanc à l'autre, `y` du plancher au plafond, `z` de l'avant
//! vers l'arrière — **`z` petit veut dire avant**. La table place aujourd'hui les trois
//! ventilateurs du radiateur à mi-profondeur, entre les deux rangées couchées, alors qu'ils sont
//! à l'avant. C'est une donnée fausse, pas un défaut de dessin : les animations la lisent aussi,
//! donc une onde avant-arrière traverse la colonne au mauvais moment.
//!
//! Une donnée de géométrie fausse ne lève jamais d'erreur. Le dessin s'affiche, l'onde tourne, et
//! rien ne distingue « le radiateur est au bon endroit » de « le radiateur est quelque part ».
//! Seul l'œil de quelqu'un qui connaît la machine l'attrape — c'est exactement ce qui vient de se
//! passer, un mois après la mesure.
//!
//! ## Trois pièges, et ce que ce fichier en fait
//!
//! 1. **Aucune coordonnée absolue n'est écrite ici.** Un test qui exigerait « le radiateur est à
//!    `z = 70` » figerait une mesure que personne n'a prise au mètre — l'échelle du boîtier est
//!    déduite d'une seule longueur connue (`docs/GEOMETRIE.md`, § Échelle), et elle se raffinera.
//!    Tous les tests portent donc sur des **relations** : devant, derrière, à la même profondeur,
//!    dans cet ordre. Elles restent vraies quelle que soit l'échelle.
//! 2. **La tolérance elle-même est déduite, jamais écrite.** L'issue accorde « un rayon de LED »
//!    pour dire que la colonne partage une profondeur ; ce rayon est mesuré sur la géométrie
//!    rendue — la plus grande distance d'un centre à l'une de ses LED — et non recopié depuis la
//!    documentation.
//! 3. **La géométrie testée est `Geometrie::mesuree()`, pas une géométrie décodée.** C'est
//!    l'inverse du choix de `spec_geometrie.rs` et `spec_animations.rs`, qui évitaient `mesuree()`
//!    parce que la table n'était pas encore relevée : ici, c'est précisément **son contenu** qui
//!    est en cause. Un test qui passerait par `decoder` ne dirait rien du défaut, puisque le
//!    fichier de géométrie ne persiste que les orientations, jamais les centres.
//!
//! ## Deux de ces tests sont des garde-fous, et passent déjà
//!
//! L'issue les écrit au présent — « le ventilateur arrière **reste** le plus en arrière », « ils
//! **restent** dans le plan du flanc ». Ce sont des propriétés que la correction ne doit pas
//! casser en déplaçant la profondeur, pas des propriétés à obtenir. Ils sont verts avant la
//! correction, et c'est leur rôle : la correction ne touche qu'à `z`, et rien d'autre ne doit
//! bouger avec.
//!
//! ## L'ordre d'arrivée d'une onde se lit en boucle, pas depuis un début
//!
//! Une animation est une fonction du pas, et elle boucle. « Le radiateur d'abord » n'a donc aucun
//! sens absolu : selon le pas où l'on regarde, n'importe quel ventilateur vient d'être atteint.
//! Ce qui a un sens, c'est l'**ordre du tour** : en partant de l'instant où la colonne du
//! radiateur est à son maximum, les autres organes doivent être atteints dans l'ordre de leurs
//! profondeurs, et le ventilateur arrière en dernier. C'est ce que mesure le dernier test, et
//! c'est faux aujourd'hui : depuis un radiateur à mi-profondeur, la rangée avant est atteinte en
//! fin de tour au lieu du début.
//!
//! ## Ce que ce fichier ne teste pas
//!
//! - La mise à jour de `docs/GEOMETRIE.md` (critère d'acceptation n° 7) : c'est une prose, elle
//!   se relit à la revue. Un test qui chercherait une date dans un fichier de documentation
//!   vérifierait la présence d'une chaîne, pas l'exactitude de ce qui l'entoure.
//! - Le décalage `RAM_PROFONDEUR` de `reverb-gui/src/plan.rs` : il vit dans un autre crate, et le
//!   dessin est l'objet de l'issue #28.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi.

use reverb_anim::{Animation, Direction, Geometrie, Image, Point, Reglages};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Les trois ventilateurs du radiateur, du haut vers le bas.
const RADIATEUR: [Position; 3] = [
    Position::RadiateurHaut,
    Position::RadiateurMilieu,
    Position::RadiateurBas,
];

/// La rangée du plancher.
const PLANCHER: [Position; 3] = [
    Position::BasGauche,
    Position::BasMilieu,
    Position::BasDroite,
];

/// La rangée du plafond.
const PLAFOND: [Position; 3] = [
    Position::HautGauche,
    Position::HautMilieu,
    Position::HautDroite,
];

/// Les six ventilateurs couchés : les deux rangées, plancher puis plafond.
///
/// « Couchés » parce qu'ils sont à plat — ce sont ceux que l'issue oppose à la colonne, et ce sont
/// les seuls dont la profondeur soit comparable à la sienne sans passer par le ventilateur arrière.
const COUCHES: [Position; 6] = [
    Position::BasGauche,
    Position::BasMilieu,
    Position::BasDroite,
    Position::HautGauche,
    Position::HautMilieu,
    Position::HautDroite,
];

/// Le boîtier tel que la mesure le déclare.
///
/// ⚠️ Volontairement `mesuree()` et non `decoder(...)` : c'est le contenu de la table mesurée qui
/// est en cause dans cette issue, et le fichier de géométrie ne persiste que les orientations.
fn boitier() -> Geometrie {
    Geometrie::mesuree()
}

/// La place d'une LED de ventilateur, ou l'échec du test.
fn led_ventilateur(geometrie: &Geometrie, position: Position, led: usize) -> Point {
    geometrie.led_ventilateur(position, led).unwrap_or_else(|| {
        panic!(
            "{}, LED {led} : sans place dans le boîtier",
            position.slug()
        )
    })
}

/// La place d'une LED de barrette, ou l'échec du test.
fn led_barrette(geometrie: &Geometrie, slot: usize, led: usize) -> Point {
    geometrie
        .led_barrette(slot, led)
        .unwrap_or_else(|| panic!("barrette {slot}, LED {led} : sans place dans le boîtier"))
}

/// La profondeur du centre d'un ventilateur : petite à l'avant, grande à l'arrière.
fn profondeur(geometrie: &Geometrie, position: Position) -> f32 {
    geometrie.centre_ventilateur(position).z
}

/// La distance entre deux points du boîtier.
fn distance(depuis: Point, vers: Point) -> f32 {
    let (dx, dy, dz) = (vers.x - depuis.x, vers.y - depuis.y, vers.z - depuis.z);
    dz.mul_add(dz, dx.mul_add(dx, dy * dy)).sqrt()
}

/// Le rayon de l'anneau de LED d'un ventilateur, **déduit de la géométrie rendue**.
///
/// La plus grande distance du centre à l'une de ses huit LED. Jamais recopié depuis
/// `docs/GEOMETRIE.md`, qui l'estime à 55 mm : ce chiffre est une estimation, il changera au
/// premier coup de mètre, et un test qui l'aurait figé deviendrait faux ce jour-là.
fn rayon_led(geometrie: &Geometrie, position: Position) -> f32 {
    let centre = geometrie.centre_ventilateur(position);
    (0..LEDS_PER_FAN as usize)
        .map(|led| distance(centre, led_ventilateur(geometrie, position, led)))
        .fold(0.0f32, f32::max)
}

/// Le plus petit des trois rayons du radiateur : la tolérance la plus stricte que l'issue accorde.
fn rayon_du_radiateur(geometrie: &Geometrie) -> f32 {
    RADIATEUR
        .into_iter()
        .map(|position| rayon_led(geometrie, position))
        .fold(f32::INFINITY, f32::min)
}

/// Le plus grand des dix rayons : l'écart en deçà duquel deux organes se recouvrent.
fn rayon_maximal(geometrie: &Geometrie) -> f32 {
    Position::ALL
        .into_iter()
        .map(|position| rayon_led(geometrie, position))
        .fold(0.0f32, f32::max)
}

/// La profondeur de la LED la plus avant et de la LED la plus arrière d'un ventilateur.
///
/// C'est l'épaisseur qu'il **occupe** réellement, par opposition à la profondeur de son centre :
/// un anneau de LED n'est pas un point, et « à la profondeur de » se juge sur ce qu'il occupe.
fn tranche_occupee(geometrie: &Geometrie, position: Position) -> (f32, f32) {
    (0..LEDS_PER_FAN as usize)
        .map(|led| led_ventilateur(geometrie, position, led).z)
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), z| {
            (min.min(z), max.max(z))
        })
}

/// Les dix ventilateurs triés de l'avant vers l'arrière.
fn du_plus_avant_au_plus_arriere(geometrie: &Geometrie) -> Vec<(Position, f32)> {
    let mut classement: Vec<(Position, f32)> = Position::ALL
        .into_iter()
        .map(|position| (position, profondeur(geometrie, position)))
        .collect();
    classement.sort_by(|(_, gauche), (_, droite)| {
        gauche
            .partial_cmp(droite)
            .expect("aucune profondeur déclarée n'est indéfinie")
    });
    classement
}

// ---------------------------------------------------------------------------
// 1 — le radiateur est en avant des deux rangées couchées
// ---------------------------------------------------------------------------

#[test]
fn les_trois_du_radiateur_sont_devant_les_deux_rangees_couchees() {
    // Test d'intention n° 1 de l'issue, critère d'acceptation n° 1 — « Les trois du radiateur sont
    // plus en avant que le plus avant des ventilateurs couchés ».
    //
    // C'est le défaut lui-même : la table les pose à mi-profondeur, exactement entre les deux
    // rangées, et le schéma que Nico a fourni le 2026-08-01 les met devant. Une comparaison par
    // paires plutôt qu'au seul minimum, pour que le message dise **lequel** des six est mal
    // ordonné le jour où un seul l'est.
    let geometrie = boitier();

    for radiateur in RADIATEUR {
        for couche in COUCHES {
            let avant = profondeur(&geometrie, radiateur);
            let arriere = profondeur(&geometrie, couche);
            assert!(
                avant < arriere,
                "{} est à z = {avant} et {} à z = {arriere} : le radiateur doit être devant les \
                 ventilateurs couchés, et z croît vers l'arrière",
                radiateur.slug(),
                couche.slug()
            );
        }
    }

    // Les deux rangées séparément : le critère dit « les ventilateurs couchés » sans distinguer,
    // mais une table qui n'aurait déplacé la colonne que devant le plancher — en la laissant
    // derrière le plafond — satisferait un test écrit sur le seul minimum des six.
    for rangee in [PLANCHER, PLAFOND] {
        let plus_avant = rangee
            .into_iter()
            .map(|position| profondeur(&geometrie, position))
            .fold(f32::INFINITY, f32::min);
        let plus_arriere_du_radiateur = RADIATEUR
            .into_iter()
            .map(|position| profondeur(&geometrie, position))
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            plus_arriere_du_radiateur < plus_avant,
            "la colonne du radiateur descend jusqu'à z = {plus_arriere_du_radiateur} alors que la \
             rangée qui commence par {} est déjà à z = {plus_avant}",
            rangee[0].slug()
        );
    }

    // Et le dire une troisième fois, autrement : les trois du radiateur sont les trois premiers
    // du boîtier d'avant en arrière. Cette forme-là attrape une table où un organe hors des six
    // couchés — le ventilateur arrière, demain un autre — se serait glissé devant la colonne.
    let classement = du_plus_avant_au_plus_arriere(&geometrie);
    let trois_premiers: Vec<Position> = classement.iter().take(3).map(|(p, _)| *p).collect();
    for radiateur in RADIATEUR {
        assert!(
            trois_premiers.contains(&radiateur),
            "{} n'est pas dans les trois ventilateurs les plus avant du boîtier ; d'avant en \
             arrière : {}",
            radiateur.slug(),
            resume_des_profondeurs(&classement)
        );
    }
}

/// Un classement de profondeurs, lisible dans un message d'échec.
fn resume_des_profondeurs(classement: &[(Position, f32)]) -> String {
    classement
        .iter()
        .map(|(position, z)| format!("{} (z = {z})", position.slug()))
        .collect::<Vec<String>>()
        .join(", ")
}

// ---------------------------------------------------------------------------
// 2 — la colonne partage une profondeur : une colonne, pas un escalier
// ---------------------------------------------------------------------------

#[test]
fn la_colonne_du_radiateur_partage_une_profondeur_a_un_rayon_de_led_pres() {
    // Test d'intention n° 2 de l'issue, critère d'acceptation n° 2 — « Ils partagent la même
    // profondeur : une colonne, pas un escalier ».
    //
    // Le défaut visé n'est pas le même que celui du test n° 1 : une table peut poser les trois
    // devant les rangées couchées et les décaler l'un de l'autre, par exemple en les alignant sur
    // les trois profondeurs d'une rangée. Rien ne le signalerait — trois ventilateurs en escalier
    // se dessinent aussi bien que trois alignés, et l'onde les traverserait l'un après l'autre au
    // lieu d'un bloc.
    //
    // La tolérance est celle que l'issue accorde — « à un rayon de LED près » — et elle est
    // mesurée sur la géométrie, pas écrite ici.
    let geometrie = boitier();
    let rayon = rayon_du_radiateur(&geometrie);
    assert!(
        rayon > 0.0 && rayon.is_finite(),
        "les anneaux de LED du radiateur ont un rayon de {rayon} : sans rayon, la tolérance de ce \
         test n'a pas de sens"
    );

    for haut in RADIATEUR {
        for bas in RADIATEUR {
            let (une, autre) = (profondeur(&geometrie, haut), profondeur(&geometrie, bas));
            assert!(
                (une - autre).abs() <= rayon,
                "{} est à z = {une} et {} à z = {autre}, soit {} d'écart pour un rayon de LED de \
                 {rayon} : c'est un escalier, pas une colonne",
                haut.slug(),
                bas.slug(),
                (une - autre).abs()
            );
        }
    }

    // Le rayon est une tolérance généreuse — c'est le prix à payer pour qu'un raffinement de la
    // mesure ne casse pas le test. Voici ce qui empêche de s'y cacher : l'étalement de la colonne
    // doit rester plus petit que ce qui la sépare du couché le plus avant. Une colonne vaut par
    // le fait qu'elle se distingue du reste, pas par un nombre.
    let profondeurs: Vec<f32> = RADIATEUR
        .into_iter()
        .map(|position| profondeur(&geometrie, position))
        .collect();
    let etalement = profondeurs
        .iter()
        .fold(f32::NEG_INFINITY, |max, z| max.max(*z))
        - profondeurs.iter().fold(f32::INFINITY, |min, z| min.min(*z));
    let plus_avant_couche = COUCHES
        .into_iter()
        .map(|position| profondeur(&geometrie, position))
        .fold(f32::INFINITY, f32::min);
    let ecart_au_reste = plus_avant_couche
        - profondeurs
            .iter()
            .fold(f32::NEG_INFINITY, |max, z| max.max(*z));
    assert!(
        etalement < ecart_au_reste,
        "la colonne s'étale sur {etalement} en profondeur et le ventilateur couché le plus avant \
         est à {ecart_au_reste} devant son bord arrière (négatif : il est derrière) : elle n'est \
         pas plus groupée qu'éloignée du reste"
    );
}

// ---------------------------------------------------------------------------
// 3 — le ventilateur arrière reste le plus en arrière des dix
// ---------------------------------------------------------------------------

#[test]
fn le_ventilateur_arriere_reste_le_plus_en_arriere_des_dix() {
    // Test d'intention n° 3 de l'issue, critère d'acceptation n° 4 — « Le ventilateur arrière
    // reste le plus en arrière de tous ».
    //
    // ⚠️ Garde-fou, pas correction : l'issue l'écrit au présent (« reste »), et il est vert avant
    // la correction. Son rôle est d'attraper une correction qui ferait glisser tout le monde vers
    // l'avant, ou qui échangerait deux constantes de profondeur en croyant n'en déplacer qu'une.
    // C'est aussi ce qui donne son sens au dernier test : sans un plus-en-arrière incontesté, « en
    // dernier » ne veut rien dire.
    let geometrie = boitier();
    let fond = profondeur(&geometrie, Position::Arriere);

    for position in Position::ALL {
        if position == Position::Arriere {
            continue;
        }
        let autre = profondeur(&geometrie, position);
        assert!(
            autre < fond,
            "{} est à z = {autre} et le ventilateur arrière à z = {fond} : rien ne doit être aussi \
             en arrière que lui",
            position.slug()
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — aucune barrette ne se dresse à la profondeur d'un ventilateur du radiateur
// ---------------------------------------------------------------------------

#[test]
fn aucune_barrette_ne_se_dresse_a_la_profondeur_d_un_ventilateur_du_radiateur() {
    // Test d'intention n° 4 de l'issue, critère d'acceptation n° 5 — « Les barrettes se dressent à
    // une profondeur qu'aucun ventilateur du radiateur n'occupe ».
    //
    // C'est ce que la maquette rend illisible aujourd'hui : la colonne est dessinée par-dessus la
    // RAM parce qu'elles occupent la même tranche du boîtier. Ce n'est pas un problème de dessin,
    // c'est que les deux se déclarent au même endroit.
    //
    // « Occupe » se juge sur la tranche des LED, pas sur le centre : un anneau de 8 LED s'étale de
    // part et d'autre de son centre, et une barrette posée à un demi-rayon d'un centre passerait
    // une comparaison de centres tout en traversant l'anneau.
    let geometrie = boitier();

    for position in RADIATEUR {
        let (avant, arriere) = tranche_occupee(&geometrie, position);
        for slot in 0..SLOT_COUNT {
            for led in 0..LEDS_PER_STICK {
                let z = led_barrette(&geometrie, slot, led).z;
                assert!(
                    z < avant || z > arriere,
                    "la LED {led} de la barrette {slot} est à z = {z}, dans la tranche [{avant}, \
                     {arriere}] qu'occupe {} : la RAM et le radiateur se recouvrent",
                    position.slug()
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — la colonne reste dans le plan du flanc, entre plancher et plafond
// ---------------------------------------------------------------------------

#[test]
fn la_colonne_du_radiateur_reste_dans_le_plan_du_flanc_et_entre_plancher_et_plafond() {
    // Critère d'acceptation n° 3 — « Ils restent dans le plan du flanc — même x — et entre
    // plancher et plafond en hauteur ». Absent de la liste des cinq tests d'intention de l'issue,
    // qui ne couvre que la profondeur ; il est pourtant le seul à dire ce que la correction ne
    // doit **pas** toucher.
    //
    // ⚠️ Garde-fou : vert avant la correction. La phrase de Nico du 2026-08-01 — « le radiateur est
    // sur le flanc du plateau de carte mère » — porte sur `x`, et l'issue prend soin de dire que
    // les deux lectures se réconcilient : plan du flanc, profondeur avant. Une correction qui
    // déplacerait `x` en même temps que `z` aurait confondu les deux, et c'est précisément la
    // confusion qui a produit la donnée fausse.
    let geometrie = boitier();

    let plan = geometrie.centre_ventilateur(RADIATEUR[0]).x;
    for position in RADIATEUR {
        let x = geometrie.centre_ventilateur(position).x;
        assert!(
            (x - plan).abs() == 0.0,
            "{} est à x = {x} et {} à x = {plan} : les trois du radiateur sont vissés sur le même \
             flanc, donc dans le même plan",
            position.slug(),
            RADIATEUR[0].slug()
        );

        // Le plan, ce sont les **anneaux** entiers, pas les seuls centres. C'est ce qui distingue
        // un radiateur vissé sur le flanc d'un radiateur vissé sur la face avant : dans les deux
        // cas les trois centres partagent un `x` et une profondeur, mais un anneau du flanc s'étale
        // en profondeur et en hauteur, un anneau de la face avant s'étale en largeur. La première
        // version de la table les avait justement mis sur la face avant
        // (`docs/GEOMETRIE.md`, § Disposition), et remettre la colonne « à l'avant » est la
        // manière la plus naturelle de refaire cette erreur-là.
        for led in 0..LEDS_PER_FAN as usize {
            let point = led_ventilateur(&geometrie, position, led);
            assert!(
                (point.x - plan).abs() == 0.0,
                "la LED {led} de {} est à x = {}, hors du plan du flanc (x = {plan}) : cet anneau \
                 est couché dans un autre plan que celui du plateau de carte mère",
                position.slug(),
                point.x
            );
        }
    }

    // « entre plancher et plafond » se lit largement : la colonne est dans le volume que les deux
    // rangées bornent. Exiger un écart strict figerait une mesure de hauteur que l'issue ne donne
    // pas et que la correction ne touche pas.
    let plancher = PLANCHER
        .into_iter()
        .map(|position| geometrie.centre_ventilateur(position).y)
        .fold(f32::INFINITY, f32::min);
    let plafond = PLAFOND
        .into_iter()
        .map(|position| geometrie.centre_ventilateur(position).y)
        .fold(f32::NEG_INFINITY, f32::max);
    for position in RADIATEUR {
        let y = geometrie.centre_ventilateur(position).y;
        assert!(
            y >= plancher && y <= plafond,
            "{} est à y = {y}, hors du volume que bornent le plancher (y = {plancher}) et le \
             plafond (y = {plafond})",
            position.slug()
        );
    }

    // Et la colonne est bien empilée dans l'ordre de ses noms : « radiateur bas » est le plus bas,
    // « radiateur haut » le plus haut (`docs/GEOMETRIE.md`, § Disposition). Une colonne aplatie
    // sur une seule hauteur satisferait les deux bornes ci-dessus.
    let hauteur = |position: Position| geometrie.centre_ventilateur(position).y;
    assert!(
        hauteur(Position::RadiateurBas) < hauteur(Position::RadiateurMilieu)
            && hauteur(Position::RadiateurMilieu) < hauteur(Position::RadiateurHaut),
        "la colonne s'empile bas ({}), milieu ({}), haut ({}) : ces trois hauteurs doivent croître",
        hauteur(Position::RadiateurBas),
        hauteur(Position::RadiateurMilieu),
        hauteur(Position::RadiateurHaut)
    );
}

// ---------------------------------------------------------------------------
// 6 — une onde avant-arrière traverse le boîtier dans l'ordre des profondeurs
// ---------------------------------------------------------------------------

/// Un organe éclairé du boîtier : un ventilateur ou une barrette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Organe {
    Ventilateur(Position),
    Barrette(usize),
}

impl Organe {
    fn nom(self) -> String {
        match self {
            Organe::Ventilateur(position) => position.slug().to_owned(),
            Organe::Barrette(slot) => format!("barrette {slot}"),
        }
    }

    /// Sa profondeur : le centre déclaré pour un ventilateur, la moyenne des LED pour une barrette.
    ///
    /// La moyenne parce qu'une barrette n'a pas de centre déclaré, et qu'elle est symétrique : le
    /// milieu de ses onze LED est le point que l'onde atteint quand elle l'atteint « en entier ».
    fn profondeur(self, geometrie: &Geometrie) -> f32 {
        match self {
            Organe::Ventilateur(position) => geometrie.centre_ventilateur(position).z,
            Organe::Barrette(slot) => {
                let somme: f32 = (0..LEDS_PER_STICK)
                    .map(|led| led_barrette(geometrie, slot, led).z)
                    .sum();
                somme / LEDS_PER_STICK as f32
            }
        }
    }

    /// Sa clarté dans une image : la somme des composantes de toutes ses LED.
    ///
    /// Une somme, et pas une luminance pondérée : ce qu'on cherche est l'instant où l'onde est sur
    /// cet organe, et n'importe quelle mesure croissante de la lumière le donne. Une pondération
    /// perceptuelle ajouterait une convention sans changer l'instant du maximum.
    fn clarte(self, image: &Image) -> u32 {
        match self {
            Organe::Ventilateur(position) => couleurs_du_ventilateur(image, position)
                .iter()
                .map(clarte)
                .sum(),
            Organe::Barrette(slot) => image.barrettes[slot].iter().map(clarte).sum(),
        }
    }
}

/// La clarté d'une couleur : la somme de ses trois composantes.
fn clarte(couleur: &Rgb) -> u32 {
    u32::from(couleur.r) + u32::from(couleur.g) + u32::from(couleur.b)
}

/// Les quatorze organes éclairés du boîtier, dans un ordre stable.
fn tous_les_organes() -> Vec<Organe> {
    let mut organes: Vec<Organe> = Position::ALL.into_iter().map(Organe::Ventilateur).collect();
    organes.extend((0..SLOT_COUNT).map(Organe::Barrette));
    organes
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

/// Le nombre de pas au-delà duquel on renonce à chercher la période du rendu.
///
/// Trente-quatre secondes à trente images par seconde : une animation qui ne se répéterait pas
/// dans cet intervalle ne serait pas une onde, et l'idée d'un ordre d'arrivée n'aurait plus d'objet.
const FENETRE: u32 = 1024;

/// La période du rendu, en pas.
///
/// Mesurée, jamais supposée : c'est la seule façon de savoir sur quel intervalle chercher une crête
/// sans lire la constante de période dans l'implémentation. Deux pas consécutifs identiques plutôt
/// qu'un seul, pour ne pas prendre un simple croisement de valeurs pour un tour complet.
fn periode(animation: &Animation, geometrie: &Geometrie, reglages: &Reglages) -> u32 {
    let origine = animation.image(geometrie, reglages, 0);
    let suivante = animation.image(geometrie, reglages, 1);
    (1..FENETRE)
        .find(|pas| {
            animation.image(geometrie, reglages, *pas) == origine
                && animation.image(geometrie, reglages, pas + 1) == suivante
        })
        .unwrap_or_else(|| {
            panic!(
                "« {} » ne se répète pas en {FENETRE} pas : sans tour complet, aucun ordre \
                 d'arrivée n'est mesurable",
                animation.nom()
            )
        })
}

/// L'instant, compté depuis `depart`, où l'onde est au plus fort sur cet organe.
///
/// Cherché sur une période entière à partir de `depart`, donc sur tout le tour : le maximum trouvé
/// est le maximum absolu de l'organe, et le premier pas qui l'atteint est le moment où l'onde
/// arrive — le front, et non le milieu d'un éventuel plateau.
fn instant_de_crete(
    animation: &Animation,
    geometrie: &Geometrie,
    reglages: &Reglages,
    organe: Organe,
    depart: u32,
    periode: u32,
) -> u32 {
    let mut sommet = 0;
    let mut instant = 0;
    for ecart in 0..periode {
        let image = animation.image(geometrie, reglages, depart + ecart);
        let clarte = organe.clarte(&image);
        if ecart == 0 || clarte > sommet {
            sommet = clarte;
            instant = ecart;
        }
    }
    instant
}

/// Les délais d'arrivée de l'onde sur les quatorze organes, comptés depuis la crête de `reference`.
///
/// L'origine du tour est posée sur la crête d'un organe et non sur le pas 0 : une animation boucle,
/// donc « en premier » n'existe pas dans l'absolu. Ce que ces délais permettent d'exiger, c'est que
/// l'ordre du tour vu depuis cette origine soit l'ordre des profondeurs.
fn delais_depuis(
    animation: &Animation,
    geometrie: &Geometrie,
    direction: Direction,
    reference: Organe,
) -> Vec<(Organe, u32)> {
    let reglages = Reglages {
        direction,
        ..Reglages::default()
    };
    let periode = periode(animation, geometrie, &reglages);
    let depart = instant_de_crete(animation, geometrie, &reglages, reference, 0, periode);
    tous_les_organes()
        .into_iter()
        .map(|organe| {
            (
                organe,
                instant_de_crete(animation, geometrie, &reglages, organe, depart, periode),
            )
        })
        .collect()
}

/// Les délais et les profondeurs, lisibles dans un message d'échec.
fn resume_des_delais(delais: &[(Organe, u32)], geometrie: &Geometrie) -> String {
    let mut lignes: Vec<(u32, String)> = delais
        .iter()
        .map(|(organe, delai)| {
            (
                *delai,
                format!(
                    "{} (z = {}, +{delai} pas)",
                    organe.nom(),
                    organe.profondeur(geometrie)
                ),
            )
        })
        .collect();
    lignes.sort_by_key(|(delai, _)| *delai);
    lignes
        .into_iter()
        .map(|(_, ligne)| ligne)
        .collect::<Vec<String>>()
        .join(", ")
}

#[test]
fn une_onde_de_profondeur_traverse_le_boitier_dans_l_ordre_des_profondeurs() {
    // Test d'intention n° 5 de l'issue #27, critère d'acceptation n° 6 — « Une onde
    // `avant-arriere` atteint les trois du radiateur avant tout le reste, et le ventilateur
    // arrière en dernier ».
    //
    // ⚠️ **Ce critère a été retiré le 2026-08-02 par l'issue #49, et ce test le dit au lieu de le
    // taire.** Nico a regardé une comète tourner sur le boîtier et jugé `avant-arriere` et
    // `arriere-avant` inversées. Les deux affirmations ne peuvent pas tenir ensemble : il n'existe
    // aucune façon de faire partir `avant-arriere` d'ailleurs que du radiateur tout en gardant le
    // critère n° 6.
    //
    // Ce qui a cédé est le **nom accroché à l'extrémité**, décidé sur schéma le 2026-08-01 ; ce qui
    // tient est l'observation faite sur le matériel qui tourne. C'est la règle du projet —
    // « l'étiquette physique vient d'une observation humaine et se vérifie » — et son précédent,
    // `SPEC-PROTOCOLE-NZXT.md` §3, dont la table s'est révélée fausse sur deux groupes de canaux
    // sur quatre.
    //
    // **Rien d'autre n'a bougé, et c'est ce qui rend le retrait tenable.** La géométrie que ce
    // fichier fige est intacte — le radiateur reste devant les deux rangées couchées (test n° 1),
    // la RAM entre les deux — et cette onde traverse toujours le boîtier dans l'ordre des
    // profondeurs, la colonne d'un bloc, sur au moins trois instants distincts. Seule la paire de
    // noms qui porte ces deux ordres est échangée.
    //
    // C'est le test qui relie la donnée à ce qu'on voit : la géométrie n'est pas qu'un dessin, les
    // animations la lisent. Avec la colonne à mi-profondeur, l'onde la traverse au milieu du tour
    // et la rangée avant est atteinte à la fin — exactement l'envers de ce que Nico regarde.
    //
    // ⚠️ L'ordre se lit **en boucle**. On pose l'origine du tour sur la crête de la colonne, puis
    // on exige que les délais des autres suivent l'ordre des profondeurs. Poser l'origine ailleurs
    // rendrait l'exigence fausse pour une implémentation correcte, et exiger « délai nul pour le
    // radiateur » depuis une origine posée sur lui serait une tautologie : ce qui porte le sens,
    // c'est la monotonie de ce qui suit.
    //
    // Deux organes plus proches qu'un rayon de LED ne sont pas comparés : leurs anneaux se
    // recouvrent en profondeur, un pas d'écart entre leurs crêtes n'aurait aucune signification, et
    // l'issue ne dit rien de leur ordre relatif.
    let geometrie = boitier();
    let vague = Animation::par_nom("vague").expect("« vague » est au catalogue");
    let tolerance = rayon_maximal(&geometrie);

    let colonne = Organe::Ventilateur(Position::RadiateurMilieu);
    let fond = Organe::Ventilateur(Position::Arriere);

    for (direction, premier, dernier) in [
        // La colonne d'abord, le fond en dernier — c'est `arriere-avant` depuis #49.
        (Direction::ArriereAvant, colonne, fond),
        // Le miroir. L'issue ne le nomme pas, mais il coûte la même mesure et il attrape le cas où
        // les deux directions rendraient la même chose — une onde qui ignorerait le signe passerait
        // le premier cas seul.
        (Direction::AvantArriere, fond, colonne),
    ] {
        let vers_l_arriere = direction == Direction::ArriereAvant;
        let delais = delais_depuis(&vague, &geometrie, direction, premier);
        let resume = resume_des_delais(&delais, &geometrie);
        let delai = |cherche: Organe| {
            delais
                .iter()
                .find(|(organe, _)| *organe == cherche)
                .map(|(_, delai)| *delai)
                .unwrap_or_else(|| panic!("{} n'est pas dans le boîtier", cherche.nom()))
        };

        // a. L'ordre d'arrivée est l'ordre des profondeurs. C'est l'assertion qui porte tout :
        //    avec la colonne à mi-profondeur, la rangée avant est atteinte en fin de tour alors
        //    qu'elle est devant, et cette paire-là échoue.
        for (avant, delai_avant) in &delais {
            for (arriere, delai_arriere) in &delais {
                let (une, autre) = (avant.profondeur(&geometrie), arriere.profondeur(&geometrie));
                let devance = if vers_l_arriere {
                    une + tolerance < autre
                } else {
                    autre + tolerance < une
                };
                if !devance {
                    continue;
                }
                assert!(
                    delai_avant <= delai_arriere,
                    "en {}, {} (z = {une}) est atteint au pas +{delai_avant} et {} (z = {autre}) au \
                     pas +{delai_arriere} : l'onde les traverse dans le désordre. Tour complet, \
                     depuis {} : {resume}",
                    direction.slug(),
                    avant.nom(),
                    arriere.nom(),
                    premier.nom()
                );
            }
        }

        // b. Le dernier atteint est bien celui que l'issue nomme. Redit ce que (a) implique, mais
        //    le dit sur l'organe précis dont l'issue parle, et sans dépendre de la tolérance.
        let delai_dernier = delai(dernier);
        for (organe, autre) in &delais {
            assert!(
                *autre <= delai_dernier,
                "en {}, {} est atteint au pas +{autre}, après {} (+{delai_dernier}) qui devrait \
                 fermer le tour. Tour complet, depuis {} : {resume}",
                direction.slug(),
                organe.nom(),
                dernier.nom(),
                premier.nom()
            );
        }
        assert!(
            delai_dernier > 0,
            "en {}, {} est atteint en même temps que {} : l'onde ne traverse pas le boîtier, elle \
             clignote. Tour complet : {resume}",
            direction.slug(),
            dernier.nom(),
            premier.nom()
        );

        // c. La colonne est atteinte d'un bloc. C'est le pendant animé du test n° 2 : trois
        //    ventilateurs qui partagent une profondeur reçoivent l'onde au même pas, et un escalier
        //    se verrait ici même s'il tenait dans la tolérance d'un rayon.
        for position in RADIATEUR {
            assert!(
                delai(Organe::Ventilateur(position)) == delai(colonne),
                "en {}, {} est atteint au pas +{} et {} au pas +{} : la colonne doit prendre l'onde \
                 d'un bloc. Tour complet : {resume}",
                direction.slug(),
                position.slug(),
                delai(Organe::Ventilateur(position)),
                colonne.nom(),
                delai(colonne)
            );
        }

        // d. Le garde-fou contre un test qui ne testerait rien : une image uniforme, ou une onde
        //    qui n'aurait que deux états, satisferait les comparaisons ci-dessus. Un boîtier de
        //    quatorze organes étalés sur toute sa profondeur en donne bien davantage.
        let mut distincts: Vec<u32> = delais.iter().map(|(_, delai)| *delai).collect();
        distincts.sort_unstable();
        distincts.dedup();
        assert!(
            distincts.len() >= 3,
            "en {}, les quatorze organes ne sont atteints qu'à {} instants différents : ce n'est \
             pas une onde qui traverse un volume. Tour complet : {resume}",
            direction.slug(),
            distincts.len()
        );
    }
}
