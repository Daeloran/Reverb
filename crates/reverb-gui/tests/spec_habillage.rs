//! Tests d'intention de l'habillage de la maquette (issue #52).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #52 et son commentaire seuls. Rien de
//! `crates/reverb-gui/src/` n'a été lu pour les écrire, hormis les signatures publiques déjà
//! figées par `spec_plan.rs` (#23) et `spec_maquette.rs` (#28) — `centre_ventilateur`,
//! `led_ventilateur`, `led_barrette`, `rayon_anneau`, `sous`, `aretes`, `vue`.
//!
//! ## Ce que ce fichier fige, et ce qu'il ne refait pas
//!
//! Le **placement** des cent vingt-quatre LED est déjà tenu, dans les deux vues, par
//! `spec_plan.rs` et `spec_maquette.rs`. Rien ici ne le respécifie et rien ici ne le contredit :
//! ce fichier tient l'**habillage** — ce qu'on dessine *autour* des LED pour que la maquette
//! ressemble à un boîtier plutôt qu'à dix anneaux de points flottant sur du noir.
//!
//! L'approche technique de l'issue en donne la règle, et c'est elle que ces tests protègent :
//! « tout ce qui a une position reste dans `plan.rs` […] **aucune coordonnée en dur dans le
//! `.slint`**, sinon la maquette diverge de la géométrie à la première correction ». Un habillage
//! dessiné dans le `.slint` marcherait parfaitement le jour où il est écrit, et deviendrait faux
//! sans un mot le jour où un ventilateur change de place. C'est la faute que ce fichier existe
//! pour interdire — d'où des tests qui portent sur des **polygones rendus par le plan**, et non
//! sur une capture d'écran.
//!
//! ## Les noms, que l'issue ne donne pas et que ces tests tranchent
//!
//! L'issue décrit « une silhouette », « la liste des faces à remplir », « deux demi-axes », « les
//! organes internes » sans nommer d'API. Ces tests en fixent le contrat :
//!
//! | ce que l'issue demande | ce que le plan rend |
//! |---|---|
//! | la silhouette du châssis | `silhouette() -> &[Place]` |
//! | les faces à remplir | `faces() -> &[Face]`, `Face { paroi, sommets }`, `enum Paroi` |
//! | les demi-axes d'un anneau | `demi_axes_anneau(Position) -> (f32, f32)` |
//! | les organes internes | `organes() -> &[Organe]`, `Organe { piece, sommets }`, `enum Piece` |
//!
//! **Un polygone est une suite de sommets dans l'ordre du contour, et sa fermeture est
//! implicite** : le dernier sommet est relié au premier et ne le répète pas. Sans cette
//! convention, « fermé » n'a pas de sens vérifiable — et une liste qui répéterait son premier
//! point donnerait au dessin une arête de longueur nulle.
//!
//! **`Paroi::Plafond` existe dans l'énumération alors que rien ne doit jamais le rendre.** C'est
//! délibéré : la demande de Nico est que la face du dessus **ne soit pas** remplie, et une absence
//! ne se vérifie que si elle se nomme. Une énumération sans `Plafond` rendrait le critère vrai par
//! construction, donc invérifiable, donc sans valeur le jour où quelqu'un ajoutera une plaque au
//! plafond « pour finir le volume ».
//!
//! ## Trois points que l'issue laisse ouverts, et que ces tests tranchent
//!
//! 1. **« La silhouette suit la géométrie » se mesure sur le nuage de LED, pas sur les anneaux.**
//!    `Geometrie` ne porte **que** les dix orientations (`crates/reverb-anim/src/geometrie.rs` :
//!    « ne porte que les orientations : les centres et les rayons sont les mêmes pour toutes les
//!    géométries d'une même machine »). La seule façon de changer une géométrie est donc de faire
//!    tourner des anneaux — et `spec_plan.rs` n° 9 exige justement que cela **ne déplace ni les
//!    centres ni le rayon**, seulement les LED. Une silhouette calculée sur les anneaux serait
//!    donc insensible à toute géométrie possible, c'est-à-dire indiscernable d'une constante
//!    codée en dur ; et c'est exactement ce que le critère d'acceptation demande de pouvoir
//!    distinguer. La silhouette épouse donc le **nuage des cent vingt-quatre LED**.
//!    ⚠️ Contrepartie assumée : remonter un ventilateur fait respirer la silhouette de quelques
//!    millièmes de cadre. C'est déjà le cas des douze arêtes de #28, qui viennent de
//!    `Geometrie::bornes()`, donc des LED elles-mêmes.
//! 2. **Les demi-axes sont ceux de l'anneau, pas ceux de ses huit points.** Un anneau est un
//!    cercle continu dont les huit LED sont un échantillon à quarante-cinq degrés : le maximum
//!    échantillonné vaut donc entre `cos(22,5°) = 0,924` et 1 fois le demi-axe réel. Les tests
//!    exigent l'encadrement correspondant — l'ellipse dessinée **contient** les huit LED, et ne
//!    les dépasse pas de plus de `1 / cos(22,5°)`. Trop petite, le cerclage passerait au travers
//!    de ses propres LED ; trop grande, le ventilateur serait dessiné plus gros que nature et
//!    mordrait son voisin, ce que `spec_plan.rs` n° 4 interdit.
//! 3. **Un organe interne ne prend aucun clic.** `Cible` n'a que deux variantes, donc `sous()` ne
//!    peut structurellement pas désigner un organe ; ce qui reste vérifiable, et qui est vérifié,
//!    c'est qu'un point posé **dans** un organe et loin de toute LED ne désigne toujours rien, et
//!    que les cent vingt-quatre LED restent atteignables au clic dans les deux vues.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Que ce soit joli.** C'est la demande d'origine, elle se regarde ; l'issue prévoit pour cela
//!   le rendu hors écran `cargo run --release --example apercu -p reverb-gui`.
//! - **La conservation de la sélection au changement de vue.** Elle vit dans la fenêtre, pas dans
//!   le plan — `spec_maquette.rs` le note déjà.
//! - **Le placement des douze arêtes de #28.** Elles restent, et ce fichier ne leur demande qu'une
//!   chose, la même qu'au reste : tenir dans le cadre.
//! - Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne touche un périphérique : le plan est
//!   du calcul pur, et ses tests aussi.

use reverb_anim::{Geometrie, Orientation};
use reverb_gui::plan::{Cible, Face, Organe, Paroi, Piece, Place, Plan, Vue};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Nombre de LED d'un ventilateur, comme indice.
const LEDS: usize = LEDS_PER_FAN as usize;

/// Ce qu'on tolère sur une comparaison de places, en unités de cadre.
///
/// Six ordres de grandeur sous le rayon d'un anneau : assez large pour absorber un aller-retour
/// en `f32`, assez étroit pour qu'un habillage rétréci d'un pour cent — soit cinq millièmes de
/// cadre — ne passe pas au travers.
const EPSILON: f32 = 1e-6;

/// Le cosinus de vingt-deux degrés et demi, soit un demi-pas d'anneau.
///
/// Huit LED à quarante-cinq degrés l'une de l'autre : au pire, aucune ne tombe sur l'extrémité
/// d'un demi-axe, et la plus proche en est à un demi-pas. Voir le point n° 2 de l'en-tête.
const COS_DEMI_PAS: f32 = 0.923_879_5;

/// Les deux vues de la géométrie mesurée, avec de quoi les nommer dans un message d'échec.
fn vues() -> [(&'static str, Plan); 2] {
    vues_de(&Geometrie::mesuree())
}

/// Les deux vues d'une géométrie donnée.
fn vues_de(geometrie: &Geometrie) -> [(&'static str, Plan); 2] {
    [
        ("vue de face", Plan::nouveau(geometrie)),
        ("vue isométrique", Plan::isometrique(geometrie)),
    ]
}

/// Une géométrie qui ne diffère de la mesurée que par l'orientation de ses dix anneaux.
///
/// Vingt-deux degrés : un demi-pas d'anneau, à un degré près. C'est le décalage qui déplace le
/// plus les huit LED d'un ventilateur sans rien changer d'autre — les centres et le rayon restent
/// ceux de la mesure (`spec_plan.rs` n° 9).
fn geometrie_reorientee() -> Geometrie {
    let mut geometrie = Geometrie::mesuree();
    for position in Position::ALL {
        let orientation = geometrie.orientation(position);
        geometrie.definir(
            position,
            Orientation::new((orientation.angle + 22) % 360, orientation.sens)
                .expect("un angle dans le tour est une orientation"),
        );
    }
    geometrie
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
    let mut leds = Vec::new();
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
        Position::ALL.len() * LEDS + SLOT_COUNT * LEDS_PER_STICK,
        "le boîtier a cent vingt-quatre LED, et la maquette les montre toutes"
    );
    leds
}

fn distance(a: Place, b: Place) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Les parois qu'une liste de faces prétend remplir, dans l'ordre où elle les rend.
fn parois(faces: &[Face]) -> Vec<Paroi> {
    faces.iter().map(|face| face.paroi).collect()
}

/// Les pièces qu'une liste d'organes prétend dessiner.
fn pieces(organes: &[Organe]) -> Vec<Piece> {
    organes.iter().map(|organe| organe.piece).collect()
}

// --- l'outil de test : appartenance d'un point à un polygone ----------------

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
/// Lancer de rayon horizontal et parité des traversées — le test d'appartenance le plus court qui
/// vaille pour un polygone quelconque, convexe ou non. C'est un **outil de test** : il n'a rien à
/// faire dans `plan.rs`, qui dessine des polygones sans jamais avoir à savoir ce qu'ils
/// contiennent. Le bord est compté dedans, sans quoi une LED posée pile sur le châssis serait
/// déclarée dehors par une erreur d'arrondi.
fn dans_le_polygone(polygone: &[Place], point: Place) -> bool {
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
/// Trois sommets au moins — deux points ne cernent aucune surface —, aucun sommet répété, une aire
/// non nulle, et tout dans le cadre. Un polygone plat ou fermé sur deux points se dessine sans
/// aucune erreur : il ne se voit simplement pas.
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
        // La fermeture est implicite (voir l'en-tête) : deux sommets consécutifs confondus, y
        // compris le dernier et le premier, sont une arête de longueur nulle.
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

// ---------------------------------------------------------------------------
// 0 — l'outil de test dit vrai
// ---------------------------------------------------------------------------

#[test]
fn l_appartenance_a_un_polygone_dit_vrai_sur_des_formes_connues() {
    // Tout ce fichier repose sur `dans_le_polygone` : un outil qui répondrait « oui » partout
    // ferait passer n'importe quelle silhouette, y compris une réduite à un point. Il se vérifie
    // donc sur des formes dont la réponse se lit à l'œil, avant de servir à juger le code.
    let p = |x: f32, y: f32| Place { x, y };
    let carre = [p(0.2, 0.2), p(0.8, 0.2), p(0.8, 0.8), p(0.2, 0.8)];

    for dedans in [p(0.5, 0.5), p(0.21, 0.79), p(0.5, 0.2001)] {
        assert!(
            dans_le_polygone(&carre, dedans),
            "{dedans:?} est dans le carré"
        );
    }
    for dehors in [
        p(0.1, 0.5),
        p(0.9, 0.5),
        p(0.5, 0.1),
        p(0.5, 0.9),
        p(9.0, 9.0),
    ] {
        assert!(
            !dans_le_polygone(&carre, dehors),
            "{dehors:?} est hors du carré"
        );
    }
    // Le bord compte comme dedans — c'est la convention de l'en-tête, et ce qui évite qu'une LED
    // posée pile sur le châssis soit déclarée dehors par une erreur d'arrondi.
    for bord in [p(0.2, 0.2), p(0.5, 0.2), p(0.8, 0.5)] {
        assert!(
            dans_le_polygone(&carre, bord),
            "{bord:?} est sur le bord du carré, donc dedans"
        );
    }

    // Et une forme concave, parce qu'un boîtier n'est pas un rectangle : le creux du L doit être
    // dehors, alors qu'il est dans l'enveloppe convexe.
    let el = [
        p(0.1, 0.1),
        p(0.9, 0.1),
        p(0.9, 0.4),
        p(0.4, 0.4),
        p(0.4, 0.9),
        p(0.1, 0.9),
    ];
    assert!(
        dans_le_polygone(&el, p(0.2, 0.2)),
        "le pied du L est dedans"
    );
    assert!(
        dans_le_polygone(&el, p(0.8, 0.2)),
        "la barre du L est dedans"
    );
    assert!(
        !dans_le_polygone(&el, p(0.8, 0.8)),
        "le creux du L est dehors, même s'il est dans l'enveloppe convexe"
    );

    // Enfin l'aire, qui sert à refuser un polygone aplati.
    assert!(
        (aire(&carre) - 0.36).abs() < 1e-6,
        "le carré fait 0,6 × 0,6"
    );
    assert!(
        aire(&[p(0.1, 0.1), p(0.5, 0.5), p(0.9, 0.9)]) < EPSILON,
        "trois points alignés n'enferment rien"
    );
}

// ---------------------------------------------------------------------------
// 1 — la silhouette contient les cent vingt-quatre LED
// ---------------------------------------------------------------------------

#[test]
fn la_silhouette_de_chaque_vue_contient_les_cent_vingt_quatre_led() {
    // Critère d'acceptation n° 1 : « chaque vue expose une silhouette calculée depuis la
    // géométrie, qui contient les 124 LED — aucune LED dessinée hors du boîtier ».
    //
    // Une LED dessinée hors du châssis ne lève rien : elle s'affiche, elle se clique, elle
    // s'allume — et elle flotte à côté du boîtier. C'est le défaut que l'issue décrit en toutes
    // lettres avant travaux (« dix anneaux de points sur du noir »), et il se corrige en dessinant
    // un châssis qui suit les LED, pas en en dessinant un qui a l'air juste.
    for (quelle, geometrie) in [
        ("géométrie mesurée", Geometrie::mesuree()),
        ("géométrie réorientée", geometrie_reorientee()),
    ] {
        for (vue, plan) in vues_de(&geometrie) {
            let silhouette = plan.silhouette();
            verifie_le_polygone(&format!("la silhouette de la {vue} ({quelle})"), silhouette);

            for (cible, place) in toutes_les_leds(&plan) {
                assert!(
                    dans_le_polygone(silhouette, place),
                    "{} est dessinée hors du boîtier en {vue} ({quelle}) : {place:?} n'est pas \
                     dans la silhouette {silhouette:?}",
                    nom(cible)
                );
            }
        }
    }

    // Le garde-fou du test : une silhouette qui couvrirait tout le cadre contiendrait les cent
    // vingt-quatre LED sans rien dire du boîtier. Il doit rester du dehors.
    for (vue, plan) in vues() {
        let silhouette = plan.silhouette();
        let pas = 100;
        let dehors = (0..=pas)
            .flat_map(|i| (0..=pas).map(move |j| (i, j)))
            .filter(|(i, j)| {
                !dans_le_polygone(
                    silhouette,
                    Place {
                        x: *i as f32 / pas as f32,
                        y: *j as f32 / pas as f32,
                    },
                )
            })
            .count();
        assert!(
            dehors > 0,
            "la silhouette de la {vue} couvre tout le cadre : elle contient les LED comme le \
             couvercle contient la boîte, et ce test ne prouverait rien"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — la silhouette suit la géométrie
// ---------------------------------------------------------------------------

#[test]
fn la_silhouette_suit_la_geometrie_et_n_est_pas_une_constante() {
    // Critère d'acceptation n° 2 : « la silhouette n'est pas une constante codée en dur : changer
    // la géométrie la change ».
    //
    // Voir le point n° 1 de l'en-tête pour ce que « changer la géométrie » peut vouloir dire :
    // `Geometrie` ne porte que dix orientations, donc seules les LED bougent — les centres et le
    // rayon ne bougent pas, et `spec_plan.rs` n° 9 l'exige. Une silhouette qui ne suivrait que les
    // anneaux serait insensible à toute géométrie possible, donc indiscernable d'un polygone écrit
    // à la main dans le `.slint` — ce que l'approche technique de l'issue interdit.
    let mesuree = Geometrie::mesuree();
    let reorientee = geometrie_reorientee();
    assert_ne!(
        mesuree, reorientee,
        "les deux géométries doivent bien différer, sinon ce test ne prouve rien"
    );

    // ⚠️ **La vue de face en est sortie depuis #125, et la propriété ne peut PAS y tenir.**
    // Son contour y est devenu un **rectangle**, celui des bornes de tout ce qui est dessiné — et
    // ces bornes sont, après cadrage, le cadre lui-même. Le rectangle est donc **invariant en
    // coordonnées normalisées**, quelle que soit la géométrie : ce n'est pas un contour figé, c'est
    // une conséquence de la normalisation, qui ramène toujours le plus grand des deux côtés à la
    // largeur utile.
    //
    // Exiger qu'il change reviendrait à exiger que le cadrage cesse de cadrer.
    //
    // ⚠️ **Ce qui remplace la garantie, plus bas : le rectangle doit ÉPOUSER le dessin.** Sans
    // cela, un polygone écrit à la main aux mêmes coordonnées passerait — et c'est exactement ce
    // que ce test existe pour empêcher. L'isométrie, elle, garde l'exigence d'origine : son
    // enveloppe convexe suit bel et bien les LED.
    for ((vue, avant), (_, apres)) in vues_de(&mesuree).into_iter().zip(vues_de(&reorientee)) {
        if vue == "vue de face" {
            continue;
        }
        let (a, b) = (avant.silhouette(), apres.silhouette());
        let differe = a.len() != b.len()
            || a.iter()
                .zip(b.iter())
                .any(|(un, autre)| distance(*un, *autre) > EPSILON);
        assert!(
            differe,
            "la silhouette de la {vue} est la même pour deux géométries différentes : elle est \
             écrite à la main, pas calculée depuis le boîtier — {a:?}"
        );
    }

    // Le rectangle de face **épouse** ce qui est dessiné : chacun de ses quatre côtés touche
    // l'extrême d'un sommet réel, à la marge près. Un rectangle posé en dur ne le ferait qu'à la
    // géométrie où on l'a écrit — et ce test balaie les deux.
    for geometrie in [&mesuree, &reorientee] {
        let plan = Plan::nouveau(geometrie);
        let contour = plan.silhouette();
        assert_eq!(
            contour.len(),
            4,
            "la vue de face rend un rectangle : {contour:?}"
        );

        let mut sommets: Vec<Place> = Vec::new();
        for position in Position::ALL {
            for led in 0..LEDS_PER_FAN as usize {
                sommets.extend(plan.led_ventilateur(position, led));
            }
        }
        for forme in plan.habillage() {
            sommets.extend(forme.contour.iter().copied());
        }
        for organe in plan.organes() {
            sommets.extend(organe.sommets.iter().copied());
        }
        assert!(!sommets.is_empty(), "il faut du contenu pour l'épouser");

        let bord = |choix: fn(&Place) -> f32, plus_grand: bool| -> f32 {
            sommets.iter().map(choix).fold(
                if plus_grand {
                    f32::NEG_INFINITY
                } else {
                    f32::INFINITY
                },
                if plus_grand { f32::max } else { f32::min },
            )
        };
        let (rx0, rx1) = (
            contour.iter().map(|p| p.x).fold(f32::INFINITY, f32::min),
            contour
                .iter()
                .map(|p| p.x)
                .fold(f32::NEG_INFINITY, f32::max),
        );
        let (ry0, ry1) = (
            contour.iter().map(|p| p.y).fold(f32::INFINITY, f32::min),
            contour
                .iter()
                .map(|p| p.y)
                .fold(f32::NEG_INFINITY, f32::max),
        );
        // Le rectangle est écarté du contenu d'une marge, la même sur les quatre côtés : ce qu'on
        // vérifie, c'est que cet écart est **le même partout**, donc qu'aucun côté n'a été posé
        // ailleurs que sur l'extrême qu'il borne.
        let ecarts = [
            bord(|p| p.x, false) - rx0,
            rx1 - bord(|p| p.x, true),
            bord(|p| p.y, false) - ry0,
            ry1 - bord(|p| p.y, true),
        ];
        let plus_petit = ecarts.iter().cloned().fold(f32::INFINITY, f32::min);
        let plus_grand = ecarts.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            plus_petit > 0.0,
            "un côté du rectangle passe DANS le dessin : écarts {ecarts:?}"
        );
        assert!(
            plus_grand - plus_petit < EPSILON,
            "les quatre côtés ne sont pas à la même marge du dessin — le rectangle ne l'épouse \
             pas, il est posé : écarts {ecarts:?}"
        );
    }

    // Et le témoin dans l'autre sens : ce qui n'a pas changé ne doit pas changer non plus. Une
    // silhouette qui « suivrait la géométrie » en sautant d'un bout à l'autre du cadre serait tout
    // aussi fausse. Tourner un anneau déplace une LED d'un tiers de rayon au plus : la silhouette
    // ne peut pas s'en émouvoir davantage qu'un anneau entier.
    for ((vue, avant), (_, apres)) in vues_de(&mesuree).into_iter().zip(vues_de(&reorientee)) {
        let (a, b) = (avant.silhouette(), apres.silhouette());
        for sommet in a.iter() {
            let plus_proche = b
                .iter()
                .map(|autre| distance(*sommet, *autre))
                .fold(f32::INFINITY, f32::min);
            assert!(
                plus_proche <= avant.rayon_anneau(),
                "la silhouette de la {vue} a bondi de {plus_proche} pour dix anneaux tournés sur \
                 place : elle ne suit pas le boîtier, elle en change"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — les faces de l'isométrie, et le dessus qui n'en est pas
// ---------------------------------------------------------------------------

#[test]
fn l_isometrie_remplit_le_plancher_le_fond_et_le_flanc_et_jamais_le_dessus() {
    // Comportement attendu, vue isométrique : « les faces occupées teintées faiblement pour qu'on
    // lise un volume : plancher, fond, flanc du plateau de carte mère », et « la face du dessus
    // reste ouverte. C'est la demande explicite de Nico : une plaque pleine masquerait les trois
    // ventilateurs du plafond, qui sont justement ceux que l'isométrie sert à montrer ».
    //
    // C'est la seule exigence de ce chantier qui vienne d'une personne regardant son boîtier, et
    // c'est aussi celle qu'un « pour finir le volume » bien intentionné défera un jour sans s'en
    // apercevoir : la maquette s'afficherait, plus jolie, et trois ventilateurs sur dix
    // deviendraient invisibles.
    let iso = Plan::isometrique(&Geometrie::mesuree());
    let faces = iso.faces();
    let mut rendues = parois(faces);
    rendues.sort_by_key(|paroi| format!("{paroi:?}"));

    let mut attendues = [Paroi::Plancher, Paroi::Fond, Paroi::Flanc];
    attendues.sort_by_key(|paroi| format!("{paroi:?}"));
    assert_eq!(
        rendues,
        attendues.to_vec(),
        "l'isométrie remplit le plancher, le fond et le flanc du plateau de carte mère, chacun une \
         fois"
    );

    // Et dans aucune des deux vues, jamais, le dessus.
    for (vue, plan) in vues() {
        let faces = plan.faces();
        assert!(
            !parois(faces).contains(&Paroi::Plafond),
            "la {vue} remplit la face du dessus : elle masque les trois ventilateurs du plafond, \
             qui sont ce que la vue sert à montrer"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — chaque face est un polygone fermé, dans le cadre
// ---------------------------------------------------------------------------

#[test]
fn chaque_face_est_un_polygone_ferme_d_au_moins_trois_sommets_dans_le_cadre() {
    // Test d'intention n° 4 de l'issue : « chaque face est un polygone fermé d'au moins trois
    // sommets, tous dans le cadre ».
    //
    // Une face à deux sommets, ou dont l'aire est nulle, se dessine sans aucune erreur — elle ne
    // se voit simplement pas, et le volume qu'elle devait donner à lire n'apparaît jamais. Un
    // sommet hors du cadre, lui, s'étale sur la fenêtre entière.
    for (vue, plan) in vues() {
        let faces = plan.faces();
        for face in faces.iter() {
            verifie_le_polygone(
                &format!("la face {:?} de la {vue}", face.paroi),
                &face.sommets,
            );
        }
    }

    // Et elles remplissent **ce** volume-là. L'issue le dit en une ligne : « les arêtes existantes
    // restent, par-dessus les faces ». Des faces posées sur un boîtier inventé à côté se
    // dessineraient sans erreur, décalées des douze arêtes de #28 — et le volume qu'on croirait
    // lire ne serait celui d'aucun boîtier. Chaque sommet de face est donc un coin du volume que
    // les arêtes dessinent, à un diamètre de LED près : de quoi rentrer un remplissage sous son
    // trait, pas de quoi le poser ailleurs.
    let iso = Plan::isometrique(&Geometrie::mesuree());
    let coins: Vec<Place> = iso
        .aretes()
        .iter()
        .flat_map(|(un, autre)| [*un, *autre])
        .collect();
    assert!(
        !coins.is_empty(),
        "l'isométrie dessine ses douze arêtes depuis #28, sinon ce test ne prouve rien"
    );
    let tolerance = iso.rayon_anneau() / 4.0;
    let faces = iso.faces();
    for face in faces.iter() {
        for (i, sommet) in face.sommets.iter().enumerate() {
            let plus_proche = coins
                .iter()
                .map(|coin| distance(*sommet, *coin))
                .fold(f32::INFINITY, f32::min);
            assert!(
                plus_proche <= tolerance,
                "le sommet {i} de la face {:?} est à {plus_proche} du coin de volume le plus \
                 proche : elle remplit un boîtier qui n'est pas celui que les arêtes dessinent",
                face.paroi
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — les demi-axes d'un anneau, dans les deux vues
// ---------------------------------------------------------------------------

#[test]
fn chaque_anneau_rend_deux_demi_axes_strictement_positifs_qui_epousent_ses_led() {
    // Critère d'acceptation n° 4 : « chaque ventilateur expose de quoi tracer son anneau : un
    // rayon en vue de face, deux demi-axes en isométrie — un anneau vu de biais est une ellipse ».
    //
    // Un demi-axe nul donne un trait, et le cerclage disparaît sans rien dire. Un demi-axe pris
    // sur les huit points au lieu de l'anneau donne un cerclage qui passe *au travers* de ses
    // propres LED — voir le point n° 2 de l'en-tête pour l'encadrement, qui vient de ce que huit
    // LED à quarante-cinq degrés échantillonnent un cercle continu.
    for (vue, plan) in vues() {
        for position in Position::ALL {
            let (rx, ry) = plan.demi_axes_anneau(position);
            let centre = plan.centre_ventilateur(position);
            assert!(
                rx.is_finite() && ry.is_finite() && rx > 0.0 && ry > 0.0,
                "l'anneau de {} en {vue} n'est pas une ellipse : demi-axes ({rx}, {ry})",
                position.slug()
            );

            let leds: Vec<Place> = (0..LEDS)
                .map(|led| place(&plan, Cible::Led { position, led }))
                .collect();
            let ecart = |f: fn(&Place) -> f32, c: f32| {
                leds.iter().map(|p| (f(p) - c).abs()).fold(0.0, f32::max)
            };
            let (dx, dy) = (ecart(|p| p.x, centre.x), ecart(|p| p.y, centre.y));

            // L'ellipse contient les huit LED : un cerclage plus étroit passerait au travers.
            assert!(
                dx <= rx * (1.0 + 1e-3) && dy <= ry * (1.0 + 1e-3),
                "le cerclage de {} en {vue} passe au travers de ses LED : demi-axes ({rx}, {ry}) \
                 pour des LED à ({dx}, {dy}) du centre {centre:?}",
                position.slug()
            );
            // Et elle ne les dépasse pas de plus d'un demi-pas d'anneau : au-delà, le ventilateur
            // est dessiné plus gros que nature et mord son voisin.
            assert!(
                rx <= dx / COS_DEMI_PAS * (1.0 + 1e-3) && ry <= dy / COS_DEMI_PAS * (1.0 + 1e-3),
                "le cerclage de {} en {vue} est plus grand que son anneau : demi-axes ({rx}, {ry}) \
                 pour des LED à ({dx}, {dy}) du centre — au plus {} et {}",
                position.slug(),
                dx / COS_DEMI_PAS,
                dy / COS_DEMI_PAS
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 6 — en vue de face, l'anneau est un cercle
// ---------------------------------------------------------------------------

#[test]
fn en_vue_de_face_les_deux_demi_axes_sont_egaux_et_valent_le_rayon_de_l_anneau() {
    // Test d'intention n° 6 de l'issue : « en vue de face, les deux demi-axes d'un anneau sont
    // égaux — c'est un cercle ». Contrat de #23 — `rayon_anneau()` : « le rayon d'un anneau de
    // ventilateur, dans la même unité que `Place` », et `Vue::Face` : les sept ventilateurs vus
    // par la tranche « sont dessinés en cercles quand même ».
    //
    // Deux demi-axes qui différeraient d'un pour cent donneraient dix ovales là où la vue de face
    // promet dix cercles : la faute ne lèverait rien, et elle contredirait la seule chose que
    // cette vue assume de maquiller.
    let face = Plan::nouveau(&Geometrie::mesuree());
    assert_eq!(face.vue(), Vue::Face, "ce test parle de la vue de face");

    for position in Position::ALL {
        let (rx, ry) = face.demi_axes_anneau(position);
        assert!(
            (rx - ry).abs() <= rx * 1e-5,
            "l'anneau de {} est un cercle en vue de face : demi-axes ({rx}, {ry})",
            position.slug()
        );
        assert!(
            (rx - face.rayon_anneau()).abs() <= face.rayon_anneau() * 1e-5,
            "le demi-axe de {} en vue de face est le rayon d'anneau que le plan annonce déjà : \
             {rx} contre {}",
            position.slug(),
            face.rayon_anneau()
        );
    }

    // Et le témoin : en isométrie, au moins un anneau n'est pas un cercle. Sans quoi « deux
    // demi-axes » ne serait qu'une façon compliquée d'écrire un rayon, et la vue de trois-quarts
    // dessinerait des ronds sur un boîtier penché.
    let iso = Plan::isometrique(&Geometrie::mesuree());
    let ovale = Position::ALL.into_iter().any(|position| {
        let (rx, ry) = iso.demi_axes_anneau(position);
        (rx - ry).abs() > rx * 1e-2
    });
    assert!(
        ovale,
        "aucun anneau n'est une ellipse en isométrie : un cercle vu de biais n'est pas un cercle"
    );
}

// ---------------------------------------------------------------------------
// 7 — les organes internes
// ---------------------------------------------------------------------------

#[test]
fn les_organes_internes_sont_poses_dans_le_repere_du_boitier_et_ne_prennent_aucun_clic() {
    // Comportement attendu, vue de face : « les organes internes suggérés en sombre : plateau de
    // carte mère, carte graphique, cache d'alimentation. Aucun n'est cliquable, aucun ne porte de
    // LED. » Approche technique : « les organes internes (carte mère, GPU, alimentation) sont des
    // rectangles posés dans le repère du boîtier, donc dans `plan.rs` eux aussi, et projetés comme
    // le reste. »
    //
    // « Posés dans le repère du boîtier » est la moitié qui compte, et c'est celle qu'on peut
    // perdre sans s'en apercevoir : trois rectangles dessinés directement à l'écran auraient
    // exactement la même allure le premier jour, et resteraient plantés au même endroit le jour
    // où un ventilateur change de place. Un rectangle du boîtier projeté est un
    // **parallélogramme**, et en isométrie il est **penché** — aucune de ses arêtes ne suit les
    // axes de l'écran par hasard.
    let mut attendues = [
        Piece::PlateauCarteMere,
        Piece::CarteGraphique,
        Piece::CacheAlimentation,
    ];
    attendues.sort_by_key(|piece| format!("{piece:?}"));

    for (vue, plan) in vues() {
        let organes = plan.organes();
        let mut rendues = pieces(organes);
        rendues.sort_by_key(|piece| format!("{piece:?}"));
        assert_eq!(
            rendues,
            attendues.to_vec(),
            "la {vue} suggère le plateau de carte mère, la carte graphique et le cache \
             d'alimentation, chacun une fois"
        );

        for organe in organes.iter() {
            let quoi = format!("l'organe {:?} de la {vue}", organe.piece);
            verifie_le_polygone(&quoi, &organe.sommets);
            assert_eq!(
                organe.sommets.len(),
                4,
                "{quoi} est un rectangle du boîtier projeté : quatre sommets, dans l'ordre du \
                 contour"
            );

            // Un rectangle projeté par une application affine est un parallélogramme : les deux
            // côtés opposés restent parallèles et de même longueur.
            let [a, b, c, d] = [
                organe.sommets[0],
                organe.sommets[1],
                organe.sommets[2],
                organe.sommets[3],
            ];
            let (fx, fy) = (a.x - b.x + c.x - d.x, a.y - b.y + c.y - d.y);
            assert!(
                fx.abs() < 1e-4 && fy.abs() < 1e-4,
                "{quoi} n'est pas un parallélogramme : {:?} — un rectangle du boîtier projeté en \
                 est un, un quadrilatère dessiné à la main n'en est pas",
                organe.sommets
            );

            // En isométrie, aucune direction du boîtier ne tombe à la fois sur l'horizontale et la
            // verticale de l'écran : un organe dont les quatre arêtes suivraient les axes de
            // l'écran aurait été posé sur l'écran, pas dans le boîtier.
            if plan.vue() == Vue::Isometrique {
                let penchee = (0..4).any(|i| {
                    let (un, autre) = (organe.sommets[i], organe.sommets[(i + 1) % 4]);
                    (un.x - autre.x).abs() > 1e-3 && (un.y - autre.y).abs() > 1e-3
                });
                assert!(
                    penchee,
                    "{quoi} a ses quatre arêtes alignées sur l'écran : il n'a pas été projeté \
                     depuis le repère du boîtier — {:?}",
                    organe.sommets
                );
            }
        }
    }

    // « Aucun n'est cliquable » : un point posé dans un organe, loin de toute LED, ne désigne
    // toujours rien. Contrat de #23 — `sous()` : « la prise vaut un rayon d'anneau ».
    let mut sondes = 0usize;
    for (vue, plan) in vues() {
        let leds = toutes_les_leds(&plan);
        let organes = plan.organes();
        for organe in organes.iter() {
            let pas = 40;
            for i in 0..=pas {
                for j in 0..=pas {
                    let point = Place {
                        x: i as f32 / pas as f32,
                        y: j as f32 / pas as f32,
                    };
                    if !dans_le_polygone(&organe.sommets, point) {
                        continue;
                    }
                    let plus_proche = leds
                        .iter()
                        .map(|(_, place)| distance(point, *place))
                        .fold(f32::INFINITY, f32::min);
                    if plus_proche > plan.rayon_anneau() {
                        assert_eq!(
                            plan.sous(point),
                            None,
                            "en {vue}, {point:?} est dans l'organe {:?} et à {plus_proche} de la \
                             LED la plus proche : un organe ne se clique pas",
                            organe.piece
                        );
                        sondes += 1;
                    }
                }
            }
        }
    }
    assert!(
        sondes > 0,
        "aucun point sondé dans un organe n'était loin de toute LED : ce test ne prouverait rien"
    );

    // « Aucun ne porte de LED » : les cent vingt-quatre restent atteignables au clic dans les deux
    // vues, c'est-à-dire qu'un clic sur la place d'une LED désigne bien quelque chose qui est à
    // cette place. (Deux LED confondues en isométrie ont le droit de se disputer le clic —
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
// 8 — tout ce qui est dessiné tient dans le cadre
// ---------------------------------------------------------------------------

#[test]
fn tout_ce_qui_est_dessine_tient_dans_le_cadre() {
    // Test d'intention n° 8 de l'issue : « tout ce qui est dessiné tient dans les bornes 0..1 du
    // cadre ». Contrat de #23 — `Place` : « normalisées de 0 à 1 […] c'est la fenêtre qui les
    // multiplie par sa taille du moment ».
    //
    // Un sommet à 1,4 ne lève rien : il se multiplie par la taille de la fenêtre comme les autres,
    // et le châssis déborde sur le reste de l'interface. `spec_plan.rs` tient déjà les LED ; ce
    // test tient l'habillage, qui est bien plus grand qu'elles.
    let dans_le_cadre = |quoi: &str, place: Place| {
        assert!(
            place.x.is_finite() && place.y.is_finite(),
            "{quoi} doit être un point fini — {place:?}"
        );
        assert!(
            (0.0..=1.0).contains(&place.x) && (0.0..=1.0).contains(&place.y),
            "{quoi} sort du cadre — {place:?}"
        );
    };

    for (vue, plan) in vues() {
        let silhouette = plan.silhouette();
        for (i, sommet) in silhouette.iter().enumerate() {
            dans_le_cadre(
                &format!("le sommet {i} de la silhouette de la {vue}"),
                *sommet,
            );
        }
        let faces = plan.faces();
        for face in faces.iter() {
            for (i, sommet) in face.sommets.iter().enumerate() {
                dans_le_cadre(
                    &format!("le sommet {i} de la face {:?} de la {vue}", face.paroi),
                    *sommet,
                );
            }
        }
        let organes = plan.organes();
        for organe in organes.iter() {
            for (i, sommet) in organe.sommets.iter().enumerate() {
                dans_le_cadre(
                    &format!("le sommet {i} de l'organe {:?} de la {vue}", organe.piece),
                    *sommet,
                );
            }
        }

        // Les anneaux : ce qu'on dessine d'un ventilateur, c'est l'ellipse entière, pas son
        // centre. Un demi-axe qui déborde tronque le cerclage et coupe les LED qui vont avec.
        for position in Position::ALL {
            let centre = plan.centre_ventilateur(position);
            let (rx, ry) = plan.demi_axes_anneau(position);
            for (coin, place) in [
                (
                    "haut gauche",
                    Place {
                        x: centre.x - rx,
                        y: centre.y - ry,
                    },
                ),
                (
                    "bas droite",
                    Place {
                        x: centre.x + rx,
                        y: centre.y + ry,
                    },
                ),
            ] {
                dans_le_cadre(
                    &format!("le coin {coin} de l'anneau de {} en {vue}", position.slug()),
                    place,
                );
            }
        }

        // Et les douze arêtes de #28, qui restent dessinées par-dessus les faces.
        for (i, (un, autre)) in plan.aretes().iter().enumerate() {
            dans_le_cadre(&format!("l'arête {i} de la {vue}"), *un);
            dans_le_cadre(&format!("l'arête {i} de la {vue}"), *autre);
        }
    }
}
