//! Tests d'intention de l'habillage du boîtier (issue #64).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #64 et son commentaire seuls. Rien de
//! `crates/reverb-gui/src/` n'a été lu pour les écrire, hormis les signatures publiques déjà
//! figées par #23, #28 et #52 — `Plan::nouveau`, `Plan::isometrique`, `centre_ventilateur`,
//! `led_ventilateur`, `led_barrette`, `centre_barrette`, `demi_axes_anneau`, `rayon_anneau`,
//! `silhouette`, `sous`, `vue`, `commandes_*`.
//!
//! ## Ce que ce fichier ajoute, et ce qu'il ne refait pas
//!
//! `spec_plan.rs` et `spec_maquette.rs` tiennent le **placement** des cent vingt-quatre LED ;
//! `spec_habillage.rs` tient le **châssis** posé par #52 — silhouette, parois, organes, anneaux.
//! Rien ici ne les respécifie et rien ici ne les contredit : ce fichier tient ce que #64 ajoute
//! par-dessus, c'est-à-dire ce qui fait qu'un ventilateur **ressemble** à un ventilateur, qu'une
//! barrette a un corps, et que l'écran du Kraken existe.
//!
//! La règle de #52 vaut encore, et c'est elle que ces tests protègent en premier : « toute forme
//! ajoutée vient de `plan.rs` : **aucune coordonnée nouvelle dans le `.slint`**, sinon la maquette
//! diverge de la géométrie à la première correction ». D'où des tests qui portent sur des polygones
//! rendus par le plan, jamais sur une capture d'écran.
//!
//! ## Les noms, que l'issue ne donne pas et que ces tests tranchent
//!
//! L'issue décrit « cadre et pales d'un ventilateur, corps d'une barrette, disque du Kraken » sans
//! nommer d'API. Ces tests en fixent le contrat, calqué sur celui de #52 — un accesseur qui rend
//! des polygones, et une sortie en chemins SVG :
//!
//! | ce que l'issue demande | ce que le plan rend |
//! |---|---|
//! | les formes de l'habillage | `habillage() -> &[Forme]` |
//! | ce qu'une forme est | `Forme { ornement, contour, creux }` |
//! | ce qu'une forme figure | `enum Ornement { CadreVentilateur(Position), CorpsBarrette(usize), DisqueKraken }` |
//! | de quoi la dessiner | `commandes_habillage() -> String` |
//! | le halo d'une LED | `halo(Rgb) -> Vec<CoucheDeHalo>`, `CoucheDeHalo { couleur, rayon, opacite }` |
//!
//! Ces tests exigent en plus `Forme: Clone`, `Ornement: PartialEq`, `CoucheDeHalo: Debug`, et des
//! champs publics. `CoucheDeHalo` porte `couleur: Rgb`, `rayon: f32` — en multiples du rayon de la
//! pastille — et `opacite: f32`, de zéro exclu à un exclu.
//!
//! **Une forme est un contour percé d'un creux**, et c'est le point de conception de tout ce
//! fichier. Un cadre de ventilateur qui serait un carré *plein* recouvrirait les huit LED qu'il
//! entoure, et l'issue interdit en toutes lettres qu'une forme masque une LED. Le boîtier
//! photographié montre exactement la solution : le cadre d'un F140 est un carré **percé** d'une
//! ouverture ronde, et l'anneau de LED se voit par ce trou. `creux` est ce trou ; il est **vide**
//! quand la forme est pleine, ce qui est le cas ordinaire d'un corps de barrette ou d'un disque.
//!
//! **Un polygone est une suite de sommets dans l'ordre du contour, et sa fermeture est
//! implicite** — même convention que `spec_habillage.rs`, pour la même raison : une liste qui
//! répéterait son premier point donnerait au dessin une arête de longueur nulle.
//!
//! ## Cinq points que l'issue laisse ouverts, et que ces tests tranchent
//!
//! 1. **« Recouvrir » se lit sur l'intérieur strict, bord exclu.** Une LED posée *sur* l'arête
//!    d'une forme n'est pas masquée : c'est même exactement ce que la photo montre d'une barrette,
//!    dont la bande lumineuse court sur l'arête supérieure du corps. Compter le bord comme
//!    recouvrant interdirait la seule façon naturelle de dessiner une barrette, et n'attraperait
//!    aucune faute réelle — une forme qui masque une LED la masque de son intérieur.
//! 2. **Un cadre ne doit pas contenir le centre d'un autre ventilateur, son creux compris.** Le
//!    creux est un trou, donc il ne recouvre rien ; mais un cadre assez grand pour *avaler* son
//!    voisin par son trou n'est plus un cadre, c'est une auréole autour de deux ventilateurs. Le
//!    critère porte donc sur le contour extérieur entier. Il est tenable et il est serré : mesuré
//!    sur la géométrie de SHYNAEL, en vue isométrique, le centre de `arriere` est à **1,27** fois
//!    le demi-axe de l'anneau de `haut-gauche`, et celui de `haut-gauche` à **1,39** fois celui de
//!    `arriere`. Un cadre dimensionné à la vraie proportion d'un F140 — 70 mm de demi-côté pour
//!    55 mm de rayon d'anneau, soit 1,27 — tomberait **pile** dessus. C'est le cas que le README
//!    annonce (« `bas-milieu` et `radiateur-bas` n'ont que 70 mm entre leurs centres pour un rayon
//!    physique de 55 »), et il casse en isométrie avant de casser en vue de face.
//!    ⚠️ **Serré n'est pas impossible, et la fenêtre a été mesurée avant d'écrire ces tests.** En
//!    facteurs de l'ellipse d'anneau, sur les deux vues et les dix ventilateurs : les huit LED
//!    d'un ventilateur du radiateur montent à **1,17** fois leur ellipse en isométrie — l'anneau y
//!    est vu penché, et `demi_axes_anneau` en rend la boîte englobante, pas les axes propres — et
//!    le premier centre voisin est à **1,27**. Aucune LED du boîtier ne tombe entre les deux. Un
//!    creux à 1,19 et un contour à 1,24 satisfont donc tout ce fichier ; c'est **une** solution,
//!    pas la solution, et surtout pas un décalque de ce que `plan.rs` devra écrire.
//! 3. **Le critère porte sur le centre des autres ventilateurs, pas sur leur cadre.** En vue
//!    isométrique un cadre n'est pas un carré aligné aux axes — le carré d'un ventilateur couché
//!    s'y projette en losange, celui d'un ventilateur debout en parallélogramme penché. « Deux
//!    cadres ne se croisent pas » n'aurait donc pas de formulation simple qui reste vraie dans les
//!    deux vues ; le centre d'un ventilateur, lui, est défini partout et par une seule méthode.
//! 4. **Un cadre entoure le sien.** Sans quoi le critère n° 2 se satisferait d'un cadre réduit à un
//!    point, ou posé à côté de son ventilateur : il ne recouvrirait le centre de personne. Chaque
//!    cadre doit donc contenir, dans son contour extérieur, le centre **et les huit LED** du
//!    ventilateur qu'il habille.
//! 5. **Le halo est une décision pure, et il ne parle pas d'ombre portée.** L'approche technique
//!    de l'issue annonçait `drop-shadow-blur` ; c'est faux, et mesuré depuis : le rendu logiciel de
//!    Slint — celui de `examples/apercu.rs` — ignore l'ombre portée. Le halo se dessine donc, en
//!    disques concentriques translucides. Ce que ces tests figent n'est pas ce dessin mais ce qui
//!    le décide : pour une couleur de LED, **quelles couches** diffuser, de quelle couleur, à quel
//!    rayon et à quelle opacité. C'est pur, c'est testable sans écran, et c'est le seul endroit où
//!    « une LED éteinte ne diffuse rien » peut se vérifier autrement qu'à l'œil.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Que ce soit joli.** C'est la demande d'origine — « un rendu un peu plus joli et propre » —
//!   et elle se regarde : `cargo run --release --example apercu -p reverb-gui`.
//! - **Les pales et le moyeu.** Ils se tracent depuis `demi_axes_anneau` et `centre_ventilateur`,
//!   que #52 expose déjà : un moyeu est une ellipse concentrique réduite, une pale un trait du
//!   moyeu vers l'anneau. Ni l'un ni l'autre n'introduit de position nouvelle, donc ni l'un ni
//!   l'autre n'a de forme à lui dans `habillage()` — et c'est pourquoi le décompte attendu est de
//!   **quinze** formes et pas davantage. Le jour où quelqu'un veut en faire des polygones, c'est
//!   ici qu'il faut le décider, pas ailleurs.
//! - **La température affichée dans le disque du Kraken.** C'est une valeur reçue du démon et un
//!   texte dessiné par la fenêtre ; `spec_sondes_retenues.rs` tient déjà d'où elle vient. Le plan,
//!   lui, ne doit que dire **où** est la dalle.
//! - Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne touche un périphérique : le plan est
//!   du calcul pur, et ses tests aussi.

use reverb_anim::Geometrie;
use reverb_gui::plan::{Cible, Place, Plan};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// La couture du banc de mutation : une ligne d'import, trois fonctions
// ---------------------------------------------------------------------------
//
// Tout l'accès à ce que #64 ajoute passe par la ligne ci-dessous et par les trois fonctions qui la
// suivent, et c'est délibéré : le banc de mutation recopie ce fichier en ne réécrivant que ces
// quatre endroits, pour le faire juger une implémentation de référence jetable et ses fautes
// injectées. Le reste du fichier est le même octet pour octet des deux côtés — sans quoi le banc
// ne prouverait rien sur *ces* tests-ci.

use reverb_gui::plan::{CoucheDeHalo, Forme, Ornement, halo};

/// Ce que le plan rend d'habillage, dans l'ordre où il le rend.
fn habillage(plan: &Plan) -> Vec<Forme> {
    plan.habillage().to_vec()
}

/// Les chemins SVG de cet habillage.
fn commandes_habillage(plan: &Plan) -> String {
    plan.commandes_habillage()
}

/// Les couches de halo que diffuse une LED de cette couleur.
fn halo_de(couleur: Rgb) -> Vec<CoucheDeHalo> {
    halo(couleur)
}

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Nombre de LED d'un ventilateur, comme indice.
const LEDS: usize = LEDS_PER_FAN as usize;

/// Le nombre de LED du boîtier, recalculé depuis les constantes du matériel plutôt que recopié.
const LED_DU_BOITIER: usize = Position::ALL.len() * LEDS + SLOT_COUNT * LEDS_PER_STICK;

/// Le nombre de formes attendu : dix cadres, quatre corps de barrette, un disque d'écran.
const FORMES_ATTENDUES: usize = Position::ALL.len() + SLOT_COUNT + 1;

/// Ce qu'on tolère sur une comparaison de places, en unités de cadre.
///
/// Même valeur et même raison que `spec_habillage.rs` : six ordres de grandeur sous le rayon d'un
/// anneau, assez large pour absorber un aller-retour en `f32`, assez étroit pour qu'une forme
/// décalée d'un pour cent ne passe pas au travers.
const EPSILON: f32 = 1e-6;

/// Les deux vues de la géométrie mesurée, avec de quoi les nommer dans un message d'échec.
fn vues() -> [(&'static str, Plan); 2] {
    let geometrie = Geometrie::mesuree();
    [
        ("vue de face", Plan::nouveau(&geometrie)),
        ("vue isométrique", Plan::isometrique(&geometrie)),
    ]
}

fn nom(cible: Cible) -> String {
    match cible {
        Cible::Led { position, led } => format!("{} LED {led}", position.slug()),
        Cible::Barrette { slot, led } => format!("barrette {slot} LED {led}"),
    }
}

/// Où le plan place une cible. Une cible est **toujours une LED** — c'est le contrat de #23.
fn place(plan: &Plan, cible: Cible) -> Place {
    match cible {
        Cible::Led { position, led } => plan.led_ventilateur(position, led),
        Cible::Barrette { slot, led } => plan.led_barrette(slot, led),
    }
    .unwrap_or_else(|| panic!("{} doit avoir une place", nom(cible)))
}

/// Les cent vingt-quatre LED du boîtier et leur place dans ce plan.
fn toutes_les_leds(plan: &Plan) -> Vec<(Cible, Place)> {
    let mut leds = Vec::with_capacity(LED_DU_BOITIER);
    for position in Position::ALL {
        for led in 0..LEDS {
            let cible = Cible::Led { position, led };
            leds.push((cible, place(plan, cible)));
        }
    }
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            let cible = Cible::Barrette { slot, led };
            leds.push((cible, place(plan, cible)));
        }
    }
    assert_eq!(
        leds.len(),
        LED_DU_BOITIER,
        "le boîtier a cent vingt-quatre LED, et la maquette les montre toutes"
    );
    leds
}

/// Comment nommer une forme dans un message d'échec.
fn nom_de_forme(forme: &Forme) -> String {
    match &forme.ornement {
        Ornement::CadreVentilateur(position) => format!("le cadre de {}", position.slug()),
        Ornement::CorpsBarrette(slot) => format!("le corps de la barrette {slot}"),
        Ornement::DisqueKraken => "le disque de l'écran du Kraken".to_string(),
    }
}

/// Le ventilateur qu'un cadre habille, ou `None` si cette forme n'est pas un cadre.
fn ventilateur_du_cadre(forme: &Forme) -> Option<Position> {
    match &forme.ornement {
        Ornement::CadreVentilateur(position) => Some(*position),
        _ => None,
    }
}

fn distance(a: Place, b: Place) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

// --- l'outil de test : appartenance d'un point à une forme percée -----------
//
// Ces quatre fonctions sont celles de `spec_habillage.rs`, recopiées plutôt que partagées : un
// test d'intention se lit seul, et un module commun entre deux fichiers de spécification ferait
// qu'une correction faite pour l'un changerait silencieusement ce que l'autre vérifie.

/// La distance d'un point au segment `[a, b]`.
fn distance_au_segment(point: Place, a: Place, b: Place) -> f32 {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let longueur = vx * vx + vy * vy;
    if longueur <= f32::MIN_POSITIVE {
        return distance(point, a);
    }
    let t = (((point.x - a.x) * vx + (point.y - a.y) * vy) / longueur).clamp(0.0, 1.0);
    distance(
        point,
        Place {
            x: a.x + t * vx,
            y: a.y + t * vy,
        },
    )
}

/// Le point est-il sur le contour du polygone, à [`EPSILON`] près ?
fn sur_le_bord(polygone: &[Place], point: Place) -> bool {
    (0..polygone.len()).any(|i| {
        distance_au_segment(point, polygone[i], polygone[(i + 1) % polygone.len()]) <= EPSILON
    })
}

/// Le point est-il dans le polygone fermé, bord compris ?
///
/// Lancer de rayon horizontal et parité des traversées. C'est un **outil de test** : il n'a rien à
/// faire dans `plan.rs`, qui dessine des polygones sans jamais avoir à savoir ce qu'ils
/// contiennent. Un polygone vide ne contient rien — ce qui est la réponse voulue pour le creux
/// d'une forme pleine.
fn dans_le_polygone(polygone: &[Place], point: Place) -> bool {
    if polygone.is_empty() {
        return false;
    }
    if sur_le_bord(polygone, point) {
        return true;
    }
    let mut dedans = false;
    for i in 0..polygone.len() {
        let a = polygone[i];
        let b = polygone[(i + 1) % polygone.len()];
        if (a.y > point.y) != (b.y > point.y) {
            let coupe = a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if point.x < coupe {
                dedans = !dedans;
            }
        }
    }
    dedans
}

/// Le point est-il **strictement** dans le polygone, bord exclu ?
fn strictement_dedans(polygone: &[Place], point: Place) -> bool {
    dans_le_polygone(polygone, point) && !sur_le_bord(polygone, point)
}

/// La forme **recouvre**-t-elle ce point ?
///
/// Dans le contour, hors du creux, bord exclu des deux côtés — voir le point n° 1 de l'en-tête :
/// une LED posée sur une arête n'est pas masquée, et c'est ce qui autorise une bande lumineuse à
/// courir sur l'arête du corps d'une barrette, comme sur la photo.
fn recouvre(forme: &Forme, point: Place) -> bool {
    strictement_dedans(&forme.contour, point) && !dans_le_polygone(&forme.creux, point)
}

/// L'aire d'un polygone fermé, par la formule du lacet. Toujours positive.
fn aire(polygone: &[Place]) -> f32 {
    let mut deux_fois = 0.0;
    for i in 0..polygone.len() {
        let a = polygone[i];
        let b = polygone[(i + 1) % polygone.len()];
        deux_fois += a.x * b.y - b.x * a.y;
    }
    (deux_fois / 2.0).abs()
}

/// Ce qu'un polygone doit être pour pouvoir se dessiner, quelle que soit sa nature.
///
/// Trois sommets au moins, aucun sommet répété, une aire non nulle, et tout dans le cadre. Un
/// polygone plat ou fermé sur deux points se dessine sans aucune erreur : il ne se voit simplement
/// pas.
fn verifie_le_polygone(quoi: &str, polygone: &[Place]) {
    assert!(
        polygone.len() >= 3,
        "{quoi} est un polygone fermé : il lui faut au moins trois sommets, il en a {} — {:?}",
        polygone.len(),
        polygone
    );
    for (i, sommet) in polygone.iter().enumerate() {
        assert!(
            sommet.x.is_finite() && sommet.y.is_finite(),
            "{quoi} : le sommet {i} doit être un point fini — {sommet:?}"
        );
        assert!(
            (0.0..=1.0).contains(&sommet.x) && (0.0..=1.0).contains(&sommet.y),
            "{quoi} : le sommet {i} sort du cadre — {sommet:?}"
        );
        let suivant = polygone[(i + 1) % polygone.len()];
        assert!(
            distance(*sommet, suivant) > EPSILON,
            "{quoi} : les sommets {i} et {} sont confondus ({sommet:?}). La fermeture d'un \
             polygone est implicite : on ne répète pas le premier point",
            (i + 1) % polygone.len()
        );
    }
    assert!(
        aire(polygone) > EPSILON,
        "{quoi} n'enferme aucune surface : aire {} — un polygone aplati se dessine sans erreur et \
         ne se voit pas",
        aire(polygone)
    );
}

/// Tous les nombres d'un chemin SVG, dans l'ordre où ils s'y trouvent.
///
/// Un découpage volontairement grossier : on ne veut pas réécrire un analyseur SVG, seulement
/// pouvoir dire « cette coordonnée-là est bien dans la chaîne ». Les lettres de commande servent de
/// séparateurs au même titre que les espaces et les virgules.
fn nombres(texte: &str) -> Vec<f32> {
    let mut trouves = Vec::new();
    let mut courant = String::new();
    let vide = |courant: &mut String, trouves: &mut Vec<f32>| {
        if !courant.is_empty() {
            if let Ok(valeur) = courant.parse::<f32>() {
                trouves.push(valeur);
            }
            courant.clear();
        }
    };
    for c in texte.chars() {
        if c == '-' && !courant.is_empty() {
            // « 0.12-0.34 » est deux nombres, pas un.
            vide(&mut courant, &mut trouves);
            courant.push(c);
        } else if c.is_ascii_digit() || c == '.' || c == '-' {
            courant.push(c);
        } else {
            vide(&mut courant, &mut trouves);
        }
    }
    vide(&mut courant, &mut trouves);
    trouves
}

/// Les couleurs témoins du halo — **toutes allumées**, le noir ayant son test à lui (n° 5).
///
/// Les trois primaires et le blanc d'abord, parce que ce sont les couleurs dont une erreur d'ordre
/// de composantes se voit ; puis deux mélanges, parce qu'un halo qui ne saurait diffuser que des
/// primaires passerait les quatre premiers ; puis un rouge à 1/255, qui n'est **pas** éteint et
/// doit donc diffuser — voir le test n° 5 pour pourquoi cette distinction-là compte.
fn couleurs_temoins() -> Vec<(&'static str, Rgb)> {
    vec![
        ("rouge pur", Rgb::new(0xff, 0x00, 0x00)),
        ("vert pur", Rgb::new(0x00, 0xff, 0x00)),
        ("bleu pur", Rgb::new(0x00, 0x00, 0xff)),
        ("blanc", Rgb::new(0xff, 0xff, 0xff)),
        ("magenta", Rgb::new(0xff, 0x00, 0xff)),
        ("orange", Rgb::new(0xff, 0x80, 0x00)),
        ("une couleur quelconque", Rgb::new(0x12, 0x34, 0x56)),
        ("un rouge à peine allumé", Rgb::new(0x01, 0x00, 0x00)),
    ]
}

// ---------------------------------------------------------------------------
// 0 — l'outil de test dit vrai
// ---------------------------------------------------------------------------

#[test]
fn l_outil_de_recouvrement_dit_vrai_sur_une_forme_percee_connue() {
    // Tout ce fichier repose sur `recouvre` : un outil qui répondrait « non » partout ferait passer
    // n'importe quel habillage, y compris un qui enterre les cent vingt-quatre LED. Il se vérifie
    // donc sur une forme dont la réponse se lit à l'œil, avant de servir à juger le code.
    let p = |x: f32, y: f32| Place { x, y };

    // Un cadre : carré de 0,2 à 0,8, percé d'un carré de 0,4 à 0,6.
    let cadre = Forme {
        ornement: Ornement::DisqueKraken,
        contour: vec![p(0.2, 0.2), p(0.8, 0.2), p(0.8, 0.8), p(0.2, 0.8)],
        creux: vec![p(0.4, 0.4), p(0.6, 0.4), p(0.6, 0.6), p(0.4, 0.6)],
    };
    for dedans in [p(0.3, 0.3), p(0.7, 0.5), p(0.5, 0.25)] {
        assert!(
            recouvre(&cadre, dedans),
            "{dedans:?} est dans la matière du cadre"
        );
    }
    for dehors in [p(0.5, 0.5), p(0.45, 0.55), p(0.1, 0.5), p(9.0, 9.0)] {
        assert!(
            !recouvre(&cadre, dehors),
            "{dehors:?} n'est pas dans la matière du cadre : il est dans le trou, ou dehors"
        );
    }
    // Le bord ne recouvre pas — ni celui du contour, ni celui du creux. Voir le point n° 1 de
    // l'en-tête : c'est ce qui autorise une bande lumineuse posée sur l'arête d'un corps.
    for bord in [p(0.2, 0.5), p(0.5, 0.8), p(0.4, 0.5), p(0.5, 0.6)] {
        assert!(
            !recouvre(&cadre, bord),
            "{bord:?} est sur une arête du cadre, donc pas recouvert"
        );
    }

    // Et une forme pleine, dont le creux est vide : elle recouvre tout son intérieur.
    let plein = Forme {
        ornement: Ornement::DisqueKraken,
        contour: vec![p(0.2, 0.2), p(0.8, 0.2), p(0.8, 0.8), p(0.2, 0.8)],
        creux: Vec::new(),
    };
    assert!(
        recouvre(&plein, p(0.5, 0.5)),
        "une forme sans creux recouvre son centre — sinon aucun de ces tests ne prouverait rien"
    );
    assert!(!recouvre(&plein, p(0.1, 0.1)), "et pas ce qui est dehors");

    // L'aire, qui sert à refuser un polygone aplati.
    assert!(
        (aire(&plein.contour) - 0.36).abs() < 1e-6,
        "le carré fait 0,6 × 0,6"
    );
    assert!(
        aire(&[p(0.1, 0.1), p(0.5, 0.5), p(0.9, 0.9)]) < EPSILON,
        "trois points alignés n'enferment rien"
    );

    // Et le découpage des nombres d'un chemin, sur lequel repose le test n° 7.
    let lus = nombres("M 0.1234 0.5678 L0.9-0.25Z");
    assert_eq!(
        lus,
        vec![0.1234, 0.5678, 0.9, -0.25],
        "les lettres de commande séparent les nombres, et « 0.9-0.25 » en fait deux"
    );
}

// ---------------------------------------------------------------------------
// 1 — l'habillage existe, dans les deux vues, et il est dessinable
// ---------------------------------------------------------------------------

#[test]
fn chaque_vue_habille_les_dix_ventilateurs_les_quatre_barrettes_et_l_ecran() {
    // Critères d'acceptation : « un ventilateur se lit comme un ventilateur : cadre, anneau, pales,
    // moyeu », « les quatre barrettes portent leur bande lumineuse », « l'écran du Kraken est
    // figuré », et « les deux vues […] donnent l'impression du boîtier photographié ».
    //
    // Le décompte est la première chose à tenir, parce que c'est ce qui rend les autres tests de ce
    // fichier utiles : une liste de formes vide passe « aucune LED masquée » et « aucun cadre n'en
    // recouvre un autre » sans effort, et sans rien dessiner. Il est aussi ce qui attrape la faute
    // la plus facile à commettre — habiller une vue et pas l'autre —, puisqu'il est exigé des deux.
    //
    // Test d'intention de l'issue : « les formes ajoutées tiennent dans le cadrage », comme la
    // silhouette, les faces et les organes de #52.
    for (vue, plan) in vues() {
        let formes = habillage(&plan);
        assert_eq!(
            formes.len(),
            FORMES_ATTENDUES,
            "la {vue} habille dix ventilateurs, quatre barrettes et un écran, soit \
             {FORMES_ATTENDUES} formes — elle en rend {} : {:?}",
            formes.len(),
            formes.iter().map(nom_de_forme).collect::<Vec<_>>()
        );

        // Un cadre par ventilateur, un corps par barrette, un disque. Chacun une fois : deux cadres
        // pour le même ventilateur en laisseraient un sans, et le décompte total ne le dirait pas.
        for position in Position::ALL {
            let combien = formes
                .iter()
                .filter(|forme| ventilateur_du_cadre(forme) == Some(position))
                .count();
            assert_eq!(
                combien,
                1,
                "la {vue} rend {combien} cadre(s) pour {} : il en faut exactement un",
                position.slug()
            );
        }
        for slot in 0..SLOT_COUNT {
            let combien = formes
                .iter()
                .filter(|forme| forme.ornement == Ornement::CorpsBarrette(slot))
                .count();
            assert_eq!(
                combien, 1,
                "la {vue} rend {combien} corps pour la barrette {slot} : il en faut exactement un"
            );
        }
        let ecrans = formes
            .iter()
            .filter(|forme| forme.ornement == Ornement::DisqueKraken)
            .count();
        assert_eq!(
            ecrans, 1,
            "la {vue} rend {ecrans} disque(s) d'écran : le Kraken n'a qu'une dalle"
        );

        // Chaque forme est dessinable, et tient dans le cadre. Un sommet à 1,4 ne lève rien : il se
        // multiplie par la taille de la fenêtre comme les autres, et la forme déborde sur le reste
        // de l'interface.
        for forme in &formes {
            let quoi = format!("{} de la {vue}", nom_de_forme(forme));
            verifie_le_polygone(&quoi, &forme.contour);
            if !forme.creux.is_empty() {
                verifie_le_polygone(&format!("le creux de {quoi}"), &forme.creux);
                // Un trou hors de la forme qu'il perce n'est pas un trou. Il se dessinerait
                // comme une seconde surface, ailleurs, sans que rien ne le signale.
                for (i, sommet) in forme.creux.iter().enumerate() {
                    assert!(
                        dans_le_polygone(&forme.contour, *sommet),
                        "le sommet {i} du creux de {quoi} est hors du contour qu'il perce — \
                         {sommet:?}"
                    );
                }
                assert!(
                    aire(&forme.creux) < aire(&forme.contour),
                    "le creux de {quoi} est plus grand que le contour qu'il perce : il ne \
                     resterait aucune matière à dessiner"
                );
            }
        }
    }

    // Et l'habillage vient bien du boîtier projeté, pas de la fenêtre : les deux vues n'y placent
    // pas la même chose. Des formes identiques dans les deux vues auraient été posées sur l'écran,
    // et resteraient plantées au même endroit le jour où un ventilateur change de place — c'est
    // exactement ce que l'approche technique de l'issue interdit.
    let [(_, face), (_, iso)] = vues();
    let (formes_face, formes_iso) = (habillage(&face), habillage(&iso));
    let differe = formes_face
        .iter()
        .zip(formes_iso.iter())
        .any(|(un, autre)| {
            un.contour.len() != autre.contour.len()
                || un
                    .contour
                    .iter()
                    .zip(autre.contour.iter())
                    .any(|(a, b)| distance(*a, *b) > EPSILON)
        });
    assert!(
        differe,
        "les deux vues rendent le même habillage, au point près : il n'a pas été projeté depuis le \
         repère du boîtier"
    );
}

// ---------------------------------------------------------------------------
// 2 — aucune LED masquée
// ---------------------------------------------------------------------------

#[test]
fn aucune_des_cent_vingt_quatre_led_n_est_masquee_ni_ne_perd_son_clic() {
    // Critère d'acceptation : « **aucune des 124 LED n'est masquée** par une forme ajoutée : elles
    // restent toutes cliquables, ce qui se vérifie sans ouvrir de fenêtre ».
    //
    // C'est le critère qui décide si la fenêtre reste un instrument. Une LED enterrée sous le corps
    // d'une barrette ou sous le cadre d'un ventilateur ne lève rien : elle s'affiche, elle change
    // de couleur avec les autres, et on ne peut plus ni la voir ni la viser. C'est aussi la faute
    // qu'un habillage plus généreux réintroduit à chaque élargissement — d'où un test sur les cent
    // vingt-quatre, dans les deux vues, et non sur un échantillon.
    //
    // Voir le point n° 1 de l'en-tête pour ce que « masquée » veut dire exactement : recouverte par
    // l'intérieur d'une forme, le bord ne comptant pas.
    for (vue, plan) in vues() {
        let formes = habillage(&plan);
        for (cible, place_de_la_led) in toutes_les_leds(&plan) {
            for forme in &formes {
                assert!(
                    !recouvre(forme, place_de_la_led),
                    "en {vue}, {} est masquée : {} recouvre {place_de_la_led:?}",
                    nom(cible),
                    nom_de_forme(forme)
                );
            }
        }
    }

    // « Elles restent toutes cliquables » : un clic sur la place d'une LED désigne bien quelque
    // chose, et quelque chose qui est à cette place. C'est la même exigence que `spec_habillage.rs`
    // tenait pour les organes de #52, reprise ici parce que l'habillage de #64 est bien plus étendu
    // qu'eux. (Deux LED confondues en isométrie ont le droit de se disputer le clic —
    // `spec_maquette.rs` refuse d'exiger d'une projection honnête qu'elle les écarte.)
    for (vue, plan) in vues() {
        for (cible, place_de_la_led) in toutes_les_leds(&plan) {
            let designee = plan.sous(place_de_la_led).unwrap_or_else(|| {
                panic!(
                    "en {vue}, un clic sur {} ({place_de_la_led:?}) ne désigne rien : l'habillage \
                     lui a pris son clic",
                    nom(cible)
                )
            });
            assert!(
                distance(place(&plan, designee), place_de_la_led) <= EPSILON,
                "en {vue}, un clic sur {} ({place_de_la_led:?}) désigne {}, qui est ailleurs",
                nom(cible),
                nom(designee)
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — deux cadres de ventilateur ne se marchent pas dessus
// ---------------------------------------------------------------------------

#[test]
fn aucun_cadre_de_ventilateur_ne_recouvre_le_centre_d_un_autre() {
    // Critère d'acceptation : « **aucun cadre de ventilateur ne recouvre le centre d'un autre** —
    // `bas-milieu` et `radiateur-bas` n'ont que 70 mm entre leurs centres pour 55 mm de rayon
    // physique (README), c'est le cas qui casse en premier ».
    //
    // Voir les points n° 2, 3 et 4 de l'en-tête : le critère porte sur le **contour extérieur**,
    // creux compris, et sur le **centre** des autres ventilateurs — la seule grandeur qui soit
    // définie de la même façon dans les deux vues, un cadre n'étant pas un carré aligné aux axes en
    // isométrie.
    //
    // Un cadre dimensionné à la vraie proportion d'un F140 — 70 mm de demi-côté pour 55 mm de rayon
    // d'anneau — tombe pile sur son voisin en vue isométrique : le centre de `arriere` y est à
    // 1,27 fois le demi-axe de l'anneau de `haut-gauche`. La marge n'est pas dans le rapport des
    // dimensions réelles, elle est à prendre.
    for (vue, plan) in vues() {
        let formes = habillage(&plan);
        let cadres: Vec<(Position, &Forme)> = formes
            .iter()
            .filter_map(|forme| ventilateur_du_cadre(forme).map(|position| (position, forme)))
            .collect();
        assert_eq!(
            cadres.len(),
            Position::ALL.len(),
            "la {vue} doit rendre un cadre par ventilateur, sinon ce test ne prouve rien"
        );

        for (porteur, cadre) in &cadres {
            // Un cadre entoure le sien — point n° 4 de l'en-tête. Sans cette moitié-là, un cadre
            // réduit à un point, ou posé à côté de son ventilateur, satisferait le critère.
            let centre = plan.centre_ventilateur(*porteur);
            assert!(
                strictement_dedans(&cadre.contour, centre),
                "en {vue}, {} n'entoure pas son ventilateur : son contour ne contient pas le \
                 centre {centre:?}",
                nom_de_forme(cadre)
            );
            for led in 0..LEDS {
                let place_de_la_led = place(
                    &plan,
                    Cible::Led {
                        position: *porteur,
                        led,
                    },
                );
                assert!(
                    strictement_dedans(&cadre.contour, place_de_la_led),
                    "en {vue}, {} laisse sa LED {led} dehors ({place_de_la_led:?}) : un cadre \
                     entoure l'anneau qu'il habille",
                    nom_de_forme(cadre)
                );
            }

            // Et il ne prend pas celui du voisin.
            for (autre, _) in &cadres {
                if autre == porteur {
                    continue;
                }
                let centre_voisin = plan.centre_ventilateur(*autre);
                assert!(
                    !strictement_dedans(&cadre.contour, centre_voisin),
                    "en {vue}, le cadre de {} recouvre le centre de {} ({centre_voisin:?}) : deux \
                     ventilateurs pour un seul cadre, et celui du dessous n'est plus lisible",
                    porteur.slug(),
                    autre.slug()
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — les deux vues montrent autant de LED l'une que l'autre
// ---------------------------------------------------------------------------

#[test]
fn les_deux_vues_rendent_le_meme_nombre_de_led() {
    // Test d'intention de l'issue : « les deux vues rendent le même nombre de LED ».
    //
    // Une vue habillée et l'autre non passerait tous les autres tests de ce fichier pris un à un :
    // la vue nue n'a rien qui masque, la vue habillée a ses quinze formes. Ce test-ci est celui qui
    // refuse la moitié du travail — et il refuse aussi la faute inverse, un habillage si généreux
    // dans une vue qu'il y enterre trois LED de plus que dans l'autre.
    let comptes: Vec<(&str, usize)> = vues()
        .iter()
        .map(|(vue, plan)| {
            let formes = habillage(plan);
            let visibles = toutes_les_leds(plan)
                .into_iter()
                .filter(|(_, place_de_la_led)| {
                    !formes.iter().any(|forme| recouvre(forme, *place_de_la_led))
                })
                .count();
            (*vue, visibles)
        })
        .collect();

    assert_eq!(
        comptes[0].1, comptes[1].1,
        "la {} montre {} LED et la {} en montre {} : les deux vues montrent le même boîtier",
        comptes[0].0, comptes[0].1, comptes[1].0, comptes[1].1
    );
    for (vue, visibles) in &comptes {
        assert_eq!(
            *visibles, LED_DU_BOITIER,
            "la {vue} ne montre que {visibles} LED sur {LED_DU_BOITIER}"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — une LED éteinte ne diffuse rien
// ---------------------------------------------------------------------------

#[test]
fn une_led_eteinte_ne_diffuse_aucun_halo() {
    // Critère d'acceptation : « une LED éteinte ne diffuse rien — un halo sur du noir serait un
    // faux positif visuel ».
    //
    // C'est le mode de défaillance le plus coûteux du halo, et il est coûteux parce qu'il est
    // rassurant : un boîtier qu'on vient d'éteindre par `animate off`, et une maquette qui reste
    // constellée d'auréoles grises. On croirait à un démon qui n'a pas reçu la commande.
    //
    // ⚠️ « Éteinte » veut dire **noire**, c'est-à-dire trois composantes nulles, et rien d'autre.
    // Un seuil de luminosité — « en dessous de 8, on ne diffuse pas » — serait une invention : ni
    // l'issue ni aucune des trois specs de protocole n'en porte, et il ferait disparaître le halo
    // d'une braise en fin de cycle, qui est justement un endroit où il se voit.
    assert!(
        halo_de(Rgb::BLACK).is_empty(),
        "une LED noire ne diffuse rien : {:?}",
        halo_de(Rgb::BLACK)
    );
    assert!(
        halo_de(Rgb::new(0, 0, 0)).is_empty(),
        "le noir écrit à la main est le même noir que `Rgb::BLACK`"
    );

    // Et le témoin, sans lequel « ne diffuse rien » se satisferait d'une fonction qui ne diffuse
    // jamais : une LED à peine allumée diffuse, elle.
    assert!(
        !halo_de(Rgb::new(0x01, 0x00, 0x00)).is_empty(),
        "un rouge à peine allumé n'est pas éteint : il diffuse"
    );
}

// ---------------------------------------------------------------------------
// 6 — le halo est de la couleur de sa LED, et il s'estompe
// ---------------------------------------------------------------------------

#[test]
fn le_halo_d_une_led_allumee_est_de_sa_couleur_et_s_estompe_en_s_eloignant() {
    // Critère d'acceptation : « chaque LED diffuse un halo de **sa** couleur, dans les deux vues ».
    // Contexte de l'issue : « sur les deux photos, la couleur d'un ventilateur déborde largement
    // sur les parois blanches autour de lui. C'est ce qu'on reconnaît d'un boîtier RGB, avant la
    // forme des pales. »
    //
    // Deux fautes silencieuses à interdire, et elles sont symétriques :
    //
    // - **un halo blanc**, ou d'une couleur voisine, « pour éclairer un peu ». Il n'échoue nulle
    //   part, il donne juste un boîtier dont toutes les LED baignent dans la même lueur — c'est-à-
    //   dire qui ne dit plus rien de ce qu'elles affichent. Le halo porte **exactement** la couleur
    //   de sa LED ; ce qui l'atténue est l'opacité, pas un mélange.
    // - **un halo uniforme**, une seule couche à plat. Ce n'est pas un halo, c'est une pastille
    //   plus grosse : il faut au moins deux couches, et l'opacité doit tomber quand le rayon monte,
    //   sans quoi le débordement a un bord net que le boîtier n'a pas.
    for (quelle, couleur) in couleurs_temoins() {
        let couches = halo_de(couleur);
        assert!(
            couches.len() >= 2,
            "{quelle} ({couleur:?}) diffuse {} couche(s) : un halo qui s'estompe en demande au \
             moins deux, sinon c'est une pastille plus grosse",
            couches.len()
        );

        for (i, couche) in couches.iter().enumerate() {
            assert_eq!(
                couche.couleur, couleur,
                "la couche {i} de {quelle} est {:?} au lieu de {couleur:?} : un halo porte la \
                 couleur de sa LED, et rien d'autre ne l'atténue que son opacité",
                couche.couleur
            );
            assert!(
                couche.rayon.is_finite() && couche.rayon > 1.0,
                "la couche {i} de {quelle} a un rayon de {} : un halo déborde de sa pastille, donc \
                 son rayon vaut plus d'un rayon de pastille",
                couche.rayon
            );
            assert!(
                couche.opacite.is_finite() && couche.opacite > 0.0 && couche.opacite < 1.0,
                "la couche {i} de {quelle} a une opacité de {} : une couche invisible ne sert à \
                 rien, et une couche opaque cacherait ce qu'elle entoure",
                couche.opacite
            );
        }

        // De la plus serrée à la plus large, et de plus en plus transparente. C'est ce qui fait un
        // dégradé plutôt qu'un empilement de disques à bords nets.
        for paire in couches.windows(2) {
            let (proche, loin) = (&paire[0], &paire[1]);
            assert!(
                loin.rayon > proche.rayon,
                "les couches de {quelle} ne s'écartent pas : rayons {} puis {} — elles se rendent \
                 dans l'ordre, de la plus serrée à la plus large",
                proche.rayon,
                loin.rayon
            );
            assert!(
                loin.opacite < proche.opacite,
                "les couches de {quelle} ne s'estompent pas : opacités {} puis {} — un halo \
                 s'efface en s'éloignant, sinon il a un bord",
                proche.opacite,
                loin.opacite
            );
        }
    }

    // Deux couleurs différentes diffusent différemment. Sans ce témoin, une fonction qui rendrait
    // toujours le même halo — rouge, gris, n'importe quoi de constant — passerait tout ce qui
    // précède dès lors que la couleur témoin y figure.
    let rouge = halo_de(Rgb::new(0xff, 0x00, 0x00));
    let vert = halo_de(Rgb::new(0x00, 0xff, 0x00));
    assert_ne!(
        rouge.first().map(|couche| couche.couleur),
        vert.first().map(|couche| couche.couleur),
        "une LED rouge et une LED verte diffusent deux halos de couleurs différentes"
    );

    // Et c'est une décision **pure** : deux appels donnent la même réponse. Un halo qui dépendrait
    // de l'ordre des appels, d'une horloge ou d'un état caché ne se testerait plus.
    for (_, couleur) in couleurs_temoins() {
        let (un, deux) = (halo_de(couleur), halo_de(couleur));
        assert_eq!(
            un.len(),
            deux.len(),
            "le halo de {couleur:?} change d'un appel à l'autre"
        );
        for (a, b) in un.iter().zip(deux.iter()) {
            assert_eq!(a.couleur, b.couleur, "le halo de {couleur:?} n'est pas pur");
            assert_eq!(a.rayon, b.rayon, "le halo de {couleur:?} n'est pas pur");
            assert_eq!(a.opacite, b.opacite, "le halo de {couleur:?} n'est pas pur");
        }
    }
}

// ---------------------------------------------------------------------------
// 7 — l'habillage sort en chemins SVG
// ---------------------------------------------------------------------------

#[test]
fn l_habillage_sort_en_chemins_svg_absolus_comme_le_reste_de_la_maquette() {
    // Approche technique de l'issue : les formes ajoutées « sortent en chemins SVG, comme
    // `commandes_silhouette`, `commandes_faces`, `commandes_organes` et `commandes_anneaux` le font
    // déjà », et « toute forme ajoutée vient de `plan.rs` : aucune coordonnée nouvelle dans le
    // `.slint` ».
    //
    // Un accesseur sans rendu laisserait la fenêtre redessiner les formes elle-même, donc réécrire
    // des coordonnées dans le `.slint` — la faute que l'issue nomme, et qui ne se voit pas le jour
    // où on la commet. Ce test ne juge pas la mise en forme du chemin : il exige seulement qu'on y
    // retrouve **chaque sommet de chaque forme**, en coordonnées absolues, et **un sous-chemin par
    // contour** — un creux qui partagerait le sous-chemin de son contour ne se dessinerait pas
    // comme un trou.
    for (vue, plan) in vues() {
        let formes = habillage(&plan);
        let chemin = commandes_habillage(&plan);
        assert!(
            !chemin.trim().is_empty(),
            "la {vue} rend {} formes et un chemin vide : rien ne se dessinerait",
            formes.len()
        );

        let sous_chemins = chemin.matches('M').count();
        let attendus = formes.len() + formes.iter().filter(|f| !f.creux.is_empty()).count();
        assert_eq!(
            sous_chemins, attendus,
            "le chemin de la {vue} a {sous_chemins} « M » pour {attendus} contours à tracer — un \
             creux a son propre sous-chemin, sinon il ne perce rien"
        );

        let lus = nombres(&chemin);
        for forme in &formes {
            for (quoi, polygone) in [
                (nom_de_forme(forme), &forme.contour),
                (format!("le creux de {}", nom_de_forme(forme)), &forme.creux),
            ] {
                for (i, sommet) in polygone.iter().enumerate() {
                    // Trois décimales suffisent à retrouver un sommet ; les quatre commandes de #52
                    // en écrivent quatre. La tolérance couvre les deux.
                    let present = lus.windows(2).any(|paire| {
                        (paire[0] - sommet.x).abs() < 1e-3 && (paire[1] - sommet.y).abs() < 1e-3
                    });
                    assert!(
                        present,
                        "le sommet {i} de {quoi} ({sommet:?}) n'est pas dans le chemin de la \
                         {vue} : la fenêtre devra le réécrire elle-même"
                    );
                }
            }
        }
    }

    // Les deux vues ne rendent pas le même chemin, et l'habillage n'est aucune des quatre familles
    // que #52 rend déjà : quatre accesseurs qui rendraient la même chaîne seraient un seul.
    let [(_, face), (_, iso)] = vues();
    assert_ne!(
        commandes_habillage(&face),
        commandes_habillage(&iso),
        "les deux vues rendent le même chemin d'habillage : il n'a pas été projeté"
    );
    for (quoi, autre) in [
        ("la silhouette", face.commandes_silhouette()),
        ("les faces", face.commandes_faces()),
        ("les organes", face.commandes_organes()),
        ("les anneaux", face.commandes_anneaux()),
    ] {
        assert_ne!(
            commandes_habillage(&face),
            autre,
            "l'habillage rend le même chemin que {quoi} : c'est {quoi} qu'on dessinerait deux fois"
        );
    }
}

// ---------------------------------------------------------------------------
// 8 — tout ce que l'habillage ajoute tient dans le cadre
// ---------------------------------------------------------------------------

#[test]
fn tout_ce_que_l_habillage_ajoute_tient_dans_le_cadre() {
    // Test d'intention de l'issue : « les formes ajoutées tiennent dans le cadrage, comme la
    // silhouette et les organes ». Contrat de #23 — `Place` : « normalisées de 0 à 1 […] c'est la
    // fenêtre qui les multiplie par sa taille du moment ».
    //
    // Un sommet à 1,4 ne lève rien : il se multiplie par la taille de la fenêtre comme les autres,
    // et la forme déborde sur le reste de l'interface — panneau des sondes compris. `spec_plan.rs`
    // tient déjà les LED et `spec_habillage.rs` le châssis de #52 ; ce test tient ce que #64
    // ajoute, qui est de loin ce qu'il y a de plus étendu sur la maquette.
    //
    // ⚠️ Il recoupe volontairement le test n° 1, qui refuse déjà un polygone hors cadre au passage.
    // C'est la façon de faire de `spec_habillage.rs` n° 8, et elle vaut pour une raison : un
    // critère qui a sa ligne dans l'issue a son test, pour qu'un échec le **nomme** au lieu de se
    // présenter comme un défaut de forme parmi d'autres.
    let dans_le_cadre = |quoi: &str, i: usize, place: Place| {
        assert!(
            place.x.is_finite() && place.y.is_finite(),
            "{quoi} : le sommet {i} doit être un point fini — {place:?}"
        );
        assert!(
            (0.0..=1.0).contains(&place.x) && (0.0..=1.0).contains(&place.y),
            "{quoi} : le sommet {i} sort du cadre — {place:?}"
        );
    };

    let mut sommets_vus = 0usize;
    for (vue, plan) in vues() {
        for forme in habillage(&plan) {
            // Le type est écrit plutôt que déduit : `dans_le_cadre` prend un `&str`, et sans
            // annotation l'inférence tire `quoi` vers `str` — un tableau de valeurs non
            // dimensionnées, donc une erreur qui parle de `Sized` là où il n'y a qu'un `String`.
            let a_verifier: [(String, &[Place]); 2] = [
                (
                    format!("{} de la {vue}", nom_de_forme(&forme)),
                    &forme.contour,
                ),
                (
                    format!("le creux de {} de la {vue}", nom_de_forme(&forme)),
                    &forme.creux,
                ),
            ];
            for (quoi, polygone) in a_verifier {
                for (i, sommet) in polygone.iter().enumerate() {
                    dans_le_cadre(&quoi, i, *sommet);
                    sommets_vus += 1;
                }
            }
        }
    }
    // Le garde-fou : un habillage vide tiendrait dans le cadre sans effort. Le décompte exact des
    // formes est au test n° 1 ; ici on veut seulement s'être prononcé sur quelque chose.
    assert!(
        sommets_vus > 0,
        "aucun sommet d'habillage n'a été vérifié : ce test ne prouverait rien"
    );
}
