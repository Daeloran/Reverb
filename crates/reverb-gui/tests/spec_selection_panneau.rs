//! Tests d'intention de la sélection au rectangle sur **tout le panneau** (issue #121).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #121 seule. Rien de `crates/reverb-gui/src/`
//! ni de `crates/reverb-gui/ui/` n'a été lu pour les écrire : la fonction visée n'existe pas, et
//! ce fichier ne compile donc pas tant qu'elle n'est pas écrite.
//!
//! ## Le défaut, et ce qui le corrige
//!
//! La maquette vit dans un **carré** de côté `min(largeur, hauteur)` du panneau, et la zone
//! sensible de sélection est dedans. Tout ce qui dépasse du carré — les marges, à gauche et à
//! droite ou en haut et en bas selon la forme de la fenêtre — n'est pas cliquable, et un geste
//! commencé là ne trace rien. Les bords du carré tombent près des extrémités des ventilateurs,
//! d'où le signalement : « la zone cliquable suit les extrémités des ventilateurs ».
//!
//! Le geste doit porter sur **tout le panneau**. Le dessin, lui, ne bouge pas : la maquette reste
//! dans son carré, parce que les tracés du châssis sont des `Path` en `viewbox` carré que Slint
//! met à l'échelle uniformément. L'issue ne déplace **que la surface sensible**.
//!
//! ## Le contrat visé
//!
//! ```ignore
//! /// Le rectangle du panneau, ramené au repère du carré de la maquette.
//! ///
//! /// `panneau_largeur` et `panneau_hauteur` sont en pixels logiques ; les quatre coordonnées
//! /// sont des **fractions du panneau**, et le résultat des **fractions du carré** — celles que
//! /// [`Plan::dans`] et [`Plan::sous`] attendent.
//! ///
//! /// Le carré a pour côté `min(largeur, hauteur)` et il est **centré** dans le panneau : une
//! /// moitié de marge de chaque côté, sur l'axe le plus long. Les coordonnées rendues peuvent
//! /// donc sortir de `0..1` — le panneau déborde du carré — et **rien n'est écrêté**.
//! pub fn trace_dans_la_maquette(
//!     panneau_largeur: f32,
//!     panneau_hauteur: f32,
//!     x0: f32,
//!     y0: f32,
//!     x1: f32,
//!     y1: f32,
//! ) -> (f32, f32, f32, f32);
//! ```
//!
//! Le calcul qu'elle doit rendre, écrit une fois pour toutes — c'est de lui que sortent tous les
//! nombres figés plus bas, et il se relit à la main :
//!
//! ```text
//! côté  = min(largeur, hauteur)
//! marge = (largeur − côté) / 2          en pixels, à gauche comme à droite
//! sx    = (x · largeur − marge) / côté  x en fraction de panneau, sx en fraction de carré
//! ```
//!
//! et le symétrique sur l'ordonnée, avec `(hauteur − côté) / 2`.
//!
//! ## Ce qui se teste, et ce qui se regarde
//!
//! **La décision, jamais le dessin** — la règle de #100, #102, #112 et #113. Rien ici n'ouvre de
//! fenêtre : une entrée entre, on regarde ce qui sort. La mise en page, elle, se vérifie sur
//! l'exemple `apercu`, et le fait que le dessin n'ait pas bougé se vérifie en comparant deux
//! images.
//!
//! Ne sont donc pas ici, et ne peuvent pas y être :
//!
//! - **la `TouchArea` remontée au panneau** et le `FocusScope` qui la suit : c'est du `.slint`,
//!   il n'entre par aucune fonction ;
//! - **`Ctrl` qui ajoute à la sélection** : de l'état de fenêtre, pas un calcul ;
//! - **le dessin inchangé** et `REVERB_TRACE` : deux images d'aperçu qu'on compare.
//!
//! ## Ce que `spec_maquette.rs` fige déjà, et comment ce fichier s'y compose
//!
//! `spec_maquette.rs` tient `dans` **dans le repère du carré**, et ce fichier n'y touche pas :
//!
//! - une cible est retenue si **son centre** est dans le rectangle, et celles-là seules ;
//! - les deux coins sont donnés dans **n'importe quel ordre**, et l'ordre du résultat est celui
//!   de la maquette, pas celui du geste ;
//! - un rectangle qui ne touche rien rend une **liste vide** — c'est ce sur quoi la fenêtre
//!   s'appuie pour désélectionner ;
//! - `dans` accepte déjà des coordonnées **hors de `0..1`** : `(−1, −1)`–`(2, 2)` retient les cent
//!   vingt-quatre, et trois rectangles franchement dehors ne retiennent rien.
//!
//! Ce dernier point est ce qui rend l'issue faisable sans toucher à `dans` : le débordement est
//! déjà admis en aval. Ce fichier ajoute donc **une seule chose** — la conversion panneau → carré
//! — et vérifie que la composer avec `dans` et `sous` désigne le même matériel qu'aujourd'hui.
//! Il ne redéfinit aucune de ces quatre propriétés ; il les invoque.

// Les bancs de ce fichier associent un libellé à un panneau, à un tracé et à deux drapeaux — un
// `[(&str, (f32, f32), Trace, bool, bool); N]` que clippy compte comme un type trop complexe. Le
// même `allow` est déjà posé pour la même raison dans `reverb-cli/tests/spec_refus_de_consigne.rs`
// (#112), et par le commit d'implémentation : il ne touche à aucune assertion, aucun vecteur et
// aucune valeur attendue, il lève seulement un refus de style sur une forme que ces tests ont
// choisie exprès — nommer chaque cas plutôt que le compter.
#![allow(clippy::type_complexity)]

use reverb_anim::Geometrie;
use reverb_gui::plan::{Cible, Place, Plan, trace_dans_la_maquette};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Les quatre nombres d'un tracé : `x0, y0, x1, y1`.
type Trace = (f32, f32, f32, f32);

/// Les cent vingt-quatre LED du boîtier, recalculées depuis le matériel plutôt que recopiées.
const LED_DU_BOITIER: usize =
    Position::ALL.len() * LEDS_PER_FAN as usize + SLOT_COUNT * LEDS_PER_STICK;

/// Un panneau plus **large** que haut : côté 600, et 100 px de marge à gauche comme à droite.
///
/// Soit `100 / 800 = 0,125` du panneau — un nombre exact en `f32`, pour que les vecteurs figés
/// plus bas se relisent à la main sans traîner d'arrondi.
const PANNEAU_LARGE: (f32, f32) = (800.0, 600.0);

/// Un panneau plus **haut** que large : côté 600, et 150 px de marge en haut comme en bas.
///
/// Soit `150 / 900 = 1/6` du panneau. La conversion y vaut `sy = 1,5 · y − 0,25`, dont les images
/// des fractions dyadiques le sont aussi.
const PANNEAU_HAUT: (f32, f32) = (600.0, 900.0);

/// Un panneau **carré**, de côté une puissance de deux : le cas dégénéré, sans aucune marge.
///
/// La puissance de deux n'est pas un ornement. L'exactitude exigée de ce cas doit porter sur la
/// conversion, et non sur le hasard d'un arrondi : avec un côté dyadique et des fractions
/// dyadiques, `x · côté / côté` est exact quelle que soit la forme donnée au calcul.
const PANNEAU_CARRE: (f32, f32) = (512.0, 512.0);

/// Un second panneau carré, de côté quelconque : l'identité doit y tenir aussi, à l'arrondi près.
const PANNEAU_CARRE_QUELCONQUE: (f32, f32) = (740.0, 740.0);

/// La marge d'un [`PANNEAU_LARGE`], en fraction de panneau : `100 / 800`.
///
/// C'est l'abscisse à laquelle le carré commence — donc celle qui valait `0` avant #121, et qui
/// est le bord de la zone morte que l'issue supprime.
const MARGE_LARGE: f32 = 0.125;

/// L'écart toléré entre deux coordonnées de carré.
///
/// Un panneau fait de l'ordre du millier de pixels logiques : `1e-5` de fraction en est un
/// centième de pixel. C'est trois ordres de grandeur sous la plus petite distance qui décide de
/// quelque chose ici — deux LED voisines d'une barrette sont à quelques centièmes de cadre l'une
/// de l'autre — et de quoi absorber les quatre arrondis `f32` d'un aller-retour panneau → carré,
/// dont l'`epsilon` vaut déjà `1,2e-7`. Un écart plus grand que ça n'est plus un arrondi, c'est
/// une autre conversion : une demi-marge oubliée en vaut `0,08`.
const TOLERANCE: f32 = 1e-5;

/// Les deux vues, avec de quoi les nommer dans un message d'échec.
///
/// L'issue le dit : « la vue isométrique partage la même `TouchArea` et suit donc sans travail
/// propre ». Elle n'a de valeur que vérifiée — c'est le même carré, et le même déborderait.
fn vues() -> [(&'static str, Plan); 2] {
    let geometrie = Geometrie::mesuree();
    [
        ("vue de face", Plan::nouveau(&geometrie)),
        ("vue isométrique", Plan::isometrique(&geometrie)),
    ]
}

/// Les cent vingt-quatre LED d'un plan, avec de quoi les nommer.
fn toutes_les_leds(plan: &Plan) -> Vec<(String, Cible, Place)> {
    let mut leds = Vec::with_capacity(LED_DU_BOITIER);
    for position in Position::ALL {
        for led in 0..LEDS_PER_FAN as usize {
            let place = plan
                .led_ventilateur(position, led)
                .unwrap_or_else(|| panic!("{} LED {led} doit avoir une place", position.slug()));
            leds.push((
                format!("{} LED {led}", position.slug()),
                Cible::Led { position, led },
                place,
            ));
        }
    }
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            let place = plan
                .led_barrette(slot, led)
                .unwrap_or_else(|| panic!("la barrette {slot} LED {led} doit avoir une place"));
            leds.push((
                format!("barrette {slot} LED {led}"),
                Cible::Barrette { slot, led },
                place,
            ));
        }
    }
    assert_eq!(
        leds.len(),
        LED_DU_BOITIER,
        "le boîtier a cent vingt-quatre LED"
    );
    leds
}

fn nom(cible: Cible) -> String {
    match cible {
        Cible::Led { position, led } => format!("{} LED {led}", position.slug()),
        Cible::Barrette { slot, led } => format!("barrette {slot} LED {led}"),
    }
}

fn noms(cibles: &[Cible]) -> Vec<String> {
    cibles.iter().map(|c| nom(*c)).collect()
}

/// Un tracé du panneau, converti puis donné à [`Plan::dans`] sous la forme qu'il attend.
fn converti(panneau: (f32, f32), trace: Trace) -> (Place, Place) {
    let (x0, y0, x1, y1) =
        trace_dans_la_maquette(panneau.0, panneau.1, trace.0, trace.1, trace.2, trace.3);
    (Place { x: x0, y: y0 }, Place { x: x1, y: y1 })
}

/// Les cibles qu'un tracé du panneau retient, dans une vue donnée.
fn retenues(plan: &Plan, panneau: (f32, f32), trace: Trace) -> Vec<Cible> {
    let (coin, oppose) = converti(panneau, trace);
    plan.dans(coin, oppose)
}

/// Compare deux tracés à [`TOLERANCE`] près, en disant lequel des quatre nombres cloche.
fn presque(obtenu: Trace, attendu: Trace, quoi: &str) {
    let obtenus = [obtenu.0, obtenu.1, obtenu.2, obtenu.3];
    let attendus = [attendu.0, attendu.1, attendu.2, attendu.3];
    for (rang, (o, a)) in obtenus.iter().zip(attendus.iter()).enumerate() {
        let nom_du_nombre = ["x0", "y0", "x1", "y1"][rang];
        assert!(
            o.is_finite(),
            "{quoi} : {nom_du_nombre} doit être un nombre, et c'est {o}"
        );
        assert!(
            (o - a).abs() <= TOLERANCE,
            "{quoi} : {nom_du_nombre} vaut {o}, attendu {a} (écart {}, toléré {TOLERANCE})",
            (o - a).abs()
        );
    }
}

// ---------------------------------------------------------------------------
// 1 — le cas dégénéré : un panneau carré ne convertit rien
// ---------------------------------------------------------------------------

#[test]
fn un_panneau_carre_rend_le_trace_inchange_et_a_l_octet_pres() {
    // Comportement attendu : le carré a pour côté `min(largeur, hauteur)` et il est centré. Quand
    // le panneau est déjà carré, la marge vaut zéro des deux côtés et la conversion est
    // l'identité.
    //
    // Pourquoi ça compte : c'est le cas dont on sait à coup sûr ce qu'il doit rendre, et donc le
    // seul qui puisse être exigé **exact**. Une conversion qui décalerait ici — parce qu'elle
    // divise par la mauvaise dimension, ou qu'elle ajoute une demi-marge qui n'existe pas — se
    // trahirait sur toutes les formes de fenêtre à la fois, et ce test la nomme sans ambiguïté.
    //
    // L'exactitude est exigée sur un côté et des fractions **dyadiques**, où elle ne dépend
    // d'aucun arrondi ; sur un côté quelconque, la même identité est exigée à [`TOLERANCE`] près.
    const TRACES_DYADIQUES: [Trace; 5] = [
        (0.0, 0.0, 1.0, 1.0),
        (0.25, 0.25, 0.75, 0.75),
        (0.125, 0.5, 0.5, 0.875),
        (0.5, 0.5, 0.5, 0.5),
        (0.0, 0.75, 0.375, 1.0),
    ];
    for trace in TRACES_DYADIQUES {
        let obtenu = trace_dans_la_maquette(
            PANNEAU_CARRE.0,
            PANNEAU_CARRE.1,
            trace.0,
            trace.1,
            trace.2,
            trace.3,
        );
        assert_eq!(
            obtenu, trace,
            "sur un panneau carré de {} px, le tracé {trace:?} n'a rien à convertir : le carré \
             occupe tout le panneau",
            PANNEAU_CARRE.0
        );
    }

    const TRACES_QUELCONQUES: [Trace; 3] = [
        (0.1, 0.37, 0.9, 0.63),
        (0.333, 0.0, 1.0, 0.777),
        (0.61, 0.61, 0.62, 0.62),
    ];
    for trace in TRACES_QUELCONQUES {
        let obtenu = trace_dans_la_maquette(
            PANNEAU_CARRE_QUELCONQUE.0,
            PANNEAU_CARRE_QUELCONQUE.1,
            trace.0,
            trace.1,
            trace.2,
            trace.3,
        );
        presque(
            obtenu,
            trace,
            &format!(
                "sur un panneau carré de {} px, le tracé {trace:?} est rendu tel quel",
                PANNEAU_CARRE_QUELCONQUE.0
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — un panneau plus large que haut : le carré est centré horizontalement
// ---------------------------------------------------------------------------

#[test]
fn un_panneau_plus_large_que_haut_decale_l_abscisse_et_laisse_l_ordonnee() {
    // Comportement attendu : côté = min(800, 600) = 600, marge = (800 − 600) / 2 = 100 px à gauche
    // et à droite. L'ordonnée, elle, n'a aucune marge : le carré occupe toute la hauteur.
    //
    //   sx = (x · 800 − 100) / 600        sy = y
    //
    // Pourquoi ça compte : c'est la forme de fenêtre courante — plus large que haute —, et c'est
    // le décalage d'une **demi**-marge qui s'y oublie. Une conversion qui diviserait par la
    // largeur sans retrancher la marge tomberait pile au centre et se tromperait partout ailleurs,
    // d'autant plus loin qu'on s'écarte du milieu ; une qui retrancherait la marge entière
    // décalerait la sélection d'un ventilateur sans que rien ne le signale.
    //
    // Les cinq vecteurs sont calculés à la main depuis les deux lignes ci-dessus, et figés :
    //
    //   x = 0      → (0 − 100)/600     = −1/6      le bord gauche du panneau, hors du carré
    //   x = 0,125  → (100 − 100)/600   = 0         le bord gauche du carré
    //   x = 0,25   → (200 − 100)/600   = 1/6
    //   x = 0,5    → (400 − 100)/600   = 0,5       le milieu tombe au milieu
    //   x = 0,875  → (700 − 100)/600   = 1         le bord droit du carré
    //   x = 1      → (800 − 100)/600   = 7/6       le bord droit du panneau, hors du carré
    const UN_SIXIEME: f32 = 1.0 / 6.0;
    let vecteurs: [(Trace, Trace); 5] = [
        // le panneau entier
        (
            (0.0, 0.0, 1.0, 1.0),
            (-UN_SIXIEME, 0.0, 1.0 + UN_SIXIEME, 1.0),
        ),
        // le carré entier, exactement
        ((MARGE_LARGE, 0.0, 0.875, 1.0), (0.0, 0.0, 1.0, 1.0)),
        // le milieu tombe au milieu, sur les deux axes
        ((0.5, 0.5, 0.5, 0.5), (0.5, 0.5, 0.5, 0.5)),
        // un quart de panneau à gauche, la moitié en hauteur
        ((0.25, 0.25, 0.5, 0.75), (UN_SIXIEME, 0.25, 0.5, 0.75)),
        // un geste qui part de la marge gauche et finit dans la marge droite
        (
            (0.0, 0.3, 1.0, 0.7),
            (-UN_SIXIEME, 0.3, 1.0 + UN_SIXIEME, 0.7),
        ),
    ];
    for (trace, attendu) in vecteurs {
        let obtenu = trace_dans_la_maquette(
            PANNEAU_LARGE.0,
            PANNEAU_LARGE.1,
            trace.0,
            trace.1,
            trace.2,
            trace.3,
        );
        presque(
            obtenu,
            attendu,
            &format!(
                "sur un panneau de {}×{}, le carré fait 600 et laisse 100 px de marge de chaque \
                 côté : le tracé {trace:?}",
                PANNEAU_LARGE.0, PANNEAU_LARGE.1
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — un panneau plus haut que large : le symétrique, sur l'ordonnée
// ---------------------------------------------------------------------------

#[test]
fn un_panneau_plus_haut_que_large_decale_l_ordonnee_et_laisse_l_abscisse() {
    // Comportement attendu : côté = min(600, 900) = 600, marge = (900 − 600) / 2 = 150 px en haut
    // et en bas. L'abscisse n'a aucune marge.
    //
    //   sx = x        sy = (y · 900 − 150) / 600 = 1,5 · y − 0,25
    //
    // Pourquoi ça compte : la fenêtre se redimensionne, et une conversion qui ne traiterait que
    // l'axe large marcherait à l'ouverture et cesserait dès qu'on étire la fenêtre en hauteur —
    // le genre de défaut qu'on met des semaines à relier à un geste de souris. C'est aussi la
    // moitié qui vérifie que `min(largeur, hauteur)` est bien un minimum, et non la largeur.
    //
    // Les vecteurs, calculés à la main :
    //
    //   y = 0      → (0 − 150)/600     = −0,25     le bord haut du panneau, hors du carré
    //   y = 1/6    → (150 − 150)/600   = 0         le bord haut du carré
    //   y = 0,25   → (225 − 150)/600   = 0,125
    //   y = 0,5    → (450 − 150)/600   = 0,5       le milieu tombe au milieu
    //   y = 5/6    → (750 − 150)/600   = 1         le bord bas du carré
    //   y = 1      → (900 − 150)/600   = 1,25      le bord bas du panneau, hors du carré
    const MARGE_HAUTE: f32 = 1.0 / 6.0;
    let vecteurs: [(Trace, Trace); 5] = [
        // le panneau entier
        ((0.0, 0.0, 1.0, 1.0), (0.0, -0.25, 1.0, 1.25)),
        // le carré entier, exactement
        (
            (0.0, MARGE_HAUTE, 1.0, 1.0 - MARGE_HAUTE),
            (0.0, 0.0, 1.0, 1.0),
        ),
        // le milieu tombe au milieu
        ((0.5, 0.5, 0.5, 0.5), (0.5, 0.5, 0.5, 0.5)),
        // un quart de panneau depuis le haut
        ((0.25, 0.25, 0.75, 0.5), (0.25, 0.125, 0.75, 0.5)),
        // un geste qui part de la marge haute et finit dans la marge basse
        ((0.3, 0.0, 0.7, 1.0), (0.3, -0.25, 0.7, 1.25)),
    ];
    for (trace, attendu) in vecteurs {
        let obtenu = trace_dans_la_maquette(
            PANNEAU_HAUT.0,
            PANNEAU_HAUT.1,
            trace.0,
            trace.1,
            trace.2,
            trace.3,
        );
        presque(
            obtenu,
            attendu,
            &format!(
                "sur un panneau de {}×{}, le carré fait 600 et laisse 150 px de marge en haut \
                 comme en bas : le tracé {trace:?}",
                PANNEAU_HAUT.0, PANNEAU_HAUT.1
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — rien n'est écrêté : les coordonnées rendues sortent de 0..1
// ---------------------------------------------------------------------------

#[test]
fn un_geste_parti_d_une_marge_sort_du_carre_au_lieu_d_etre_ramene_a_son_bord() {
    // Comportement attendu, et c'est le point de vigilance de l'issue : « les coordonnées de
    // sélection deviennent extérieures au carré, et peuvent sortir de `0..1` ».
    //
    // Pourquoi ça compte : écrêter à `0..1` serait la correction qu'on écrit sans y penser, pour
    // « rester dans le cadre ». Elle replierait un rectangle parti de la marge sur le bord du
    // carré au lieu de le laisser le dépasser — et le geste que l'issue veut rendre possible
    // deviendrait un geste dont le coin est faux. Pire, un rectangle **entièrement** dans une
    // marge s'écraserait en un rectangle plat collé au bord, qui attraperait la première rangée
    // de LED : la zone morte d'aujourd'hui deviendrait une sélection au hasard.
    //
    // On exige donc les deux moitiés : la valeur sort de `0..1`, **et** elle vaut exactement ce
    // que l'affine rend — pas `0`, pas `1`.
    let (x0, _, x1, _) =
        trace_dans_la_maquette(PANNEAU_LARGE.0, PANNEAU_LARGE.1, 0.0, 0.5, 1.0, 0.5);
    assert!(
        x0 < 0.0,
        "le bord gauche d'un panneau large est à gauche du carré : {x0} devrait être négatif"
    );
    assert!(
        x1 > 1.0,
        "le bord droit d'un panneau large est à droite du carré : {x1} devrait dépasser 1"
    );
    presque(
        (x0, 0.0, x1, 0.0),
        (-1.0 / 6.0, 0.0, 1.0 + 1.0 / 6.0, 0.0),
        "les bords d'un panneau large valent −1/6 et 7/6 dans le carré, et non 0 et 1",
    );

    let (_, y0, _, y1) = trace_dans_la_maquette(PANNEAU_HAUT.0, PANNEAU_HAUT.1, 0.5, 0.0, 0.5, 1.0);
    assert!(
        y0 < 0.0,
        "le bord haut d'un panneau haut est au-dessus du carré : {y0} devrait être négatif"
    );
    assert!(
        y1 > 1.0,
        "le bord bas d'un panneau haut est sous le carré : {y1} devrait dépasser 1"
    );
    presque(
        (0.0, y0, 0.0, y1),
        (0.0, -0.25, 0.0, 1.25),
        "les bords d'un panneau haut valent −0,25 et 1,25 dans le carré, et non 0 et 1",
    );
}

#[test]
fn un_rectangle_entierement_dans_une_marge_reste_entierement_hors_du_carre() {
    // Comportement attendu : « un rectangle entièrement hors du carré ne sélectionne rien et ne
    // casse rien ». Avant même de parler de sélection, ses **deux** bords doivent rester du même
    // côté du carré après conversion.
    //
    // Pourquoi ça compte : c'est le test qui distingue « ne pas écrêter » de « écrêter un seul
    // coin ». Un écrêtage qui ne toucherait que le coin le plus dehors laisserait un rectangle
    // dégénéré posé sur le bord du carré — non vide, donc sélectionnant, et sélectionnant
    // justement les LED du bord, celles dont l'issue dit que la zone morte les longe.
    //
    // Les quatre marges d'un boîtier : gauche et droite sur un panneau large, haut et bas sur un
    // panneau haut. Aucune fenêtre n'a les quatre à la fois — le carré est le plus grand qui
    // tienne — mais les deux formes couvrent les quatre cas.
    let cas: [(&str, (f32, f32), Trace, bool, bool); 4] = [
        // (quoi, panneau, tracé, axe des abscisses ?, du côté négatif ?)
        (
            "la marge gauche d'un panneau large",
            PANNEAU_LARGE,
            (0.0, 0.2, 0.10, 0.8),
            true,
            true,
        ),
        (
            "la marge droite d'un panneau large",
            PANNEAU_LARGE,
            (0.90, 0.2, 1.0, 0.8),
            true,
            false,
        ),
        (
            "la marge haute d'un panneau haut",
            PANNEAU_HAUT,
            (0.2, 0.0, 0.8, 0.10),
            false,
            true,
        ),
        (
            "la marge basse d'un panneau haut",
            PANNEAU_HAUT,
            (0.2, 0.90, 0.8, 1.0),
            false,
            false,
        ),
    ];
    for (quoi, panneau, trace, sur_l_abscisse, du_cote_negatif) in cas {
        let (x0, y0, x1, y1) =
            trace_dans_la_maquette(panneau.0, panneau.1, trace.0, trace.1, trace.2, trace.3);
        let (a, b) = if sur_l_abscisse { (x0, x1) } else { (y0, y1) };
        if du_cote_negatif {
            assert!(
                a < 0.0 && b < 0.0,
                "{quoi} : le tracé {trace:?} est tout entier avant le carré, donc ses deux bords \
                 sont négatifs — obtenu {a} et {b}"
            );
        } else {
            assert!(
                a > 1.0 && b > 1.0,
                "{quoi} : le tracé {trace:?} est tout entier après le carré, donc ses deux bords \
                 dépassent 1 — obtenu {a} et {b}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — le sens du geste est préservé
// ---------------------------------------------------------------------------

#[test]
fn le_sens_du_geste_est_preserve_et_les_coins_ne_sont_pas_reordonnes() {
    // Comportement attendu : la conversion ramène chaque coin dans le repère du carré, et rien de
    // plus. Tirer de droite à gauche reste de droite à gauche.
    //
    // Pourquoi ça compte : `dans` accepte déjà ses deux coins dans n'importe quel ordre — c'est
    // figé par `spec_maquette.rs`, « un glissement va dans les quatre sens ». Réordonner ici
    // serait donc du travail inutile, et du travail qui ment : la fenêtre dessine le rectangle en
    // cours avec ces mêmes nombres, et un coin remis à l'endroit ferait sauter le tracé sous la
    // souris au moment où le geste change de sens. C'est aussi ce qui garantit que la conversion
    // est **point par point**, et non « rectangle par rectangle » — la seule forme qui compose
    // proprement avec un clic simple, où les deux coins sont confondus.
    let panneaux = [
        ("large", PANNEAU_LARGE),
        ("haut", PANNEAU_HAUT),
        ("carré", PANNEAU_CARRE),
    ];
    const GESTES: [Trace; 4] = [
        (0.9, 0.8, 0.2, 0.1), // de droite à gauche, de bas en haut
        (0.2, 0.1, 0.9, 0.8), // le même à l'endroit
        (0.9, 0.1, 0.2, 0.8), // de droite à gauche, de haut en bas
        (0.0, 1.0, 1.0, 0.0), // les deux diagonales du panneau entier
    ];
    for (nom_panneau, panneau) in panneaux {
        for trace in GESTES {
            let (x0, y0, x1, y1) =
                trace_dans_la_maquette(panneau.0, panneau.1, trace.0, trace.1, trace.2, trace.3);
            let (mx0, my0, mx1, my1) =
                trace_dans_la_maquette(panneau.0, panneau.1, trace.2, trace.3, trace.0, trace.1);
            presque(
                (mx0, my0, mx1, my1),
                (x1, y1, x0, y0),
                &format!(
                    "sur un panneau {nom_panneau}, échanger les deux coins du geste {trace:?} \
                     échange les deux coins du résultat, et rien d'autre"
                ),
            );
            assert_eq!(
                (trace.0 < trace.2, trace.1 < trace.3),
                (x0 < x1, y0 < y1),
                "sur un panneau {nom_panneau}, le geste {trace:?} garde son sens : \
                 ({x0}, {y0}) → ({x1}, {y1})"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 6 — composé avec `dans`, un tracé de panneau désigne le même matériel
// ---------------------------------------------------------------------------

#[test]
fn un_trace_du_panneau_retient_les_memes_led_que_le_meme_trace_dans_le_carre() {
    // Test d'intention de l'issue : « un rectangle donné en coordonnées de panneau sélectionne les
    // mêmes LED qu'aujourd'hui en coordonnées de carré ».
    //
    // Pourquoi ça compte : c'est la non-régression du geste ordinaire. Tout le monde sait écrire
    // une conversion qui débloque les marges et décale de trois pour cent ce qui marchait déjà —
    // et personne ne le verrait, parce que sélectionner une LED voisine de celle qu'on visait ne
    // produit aucune erreur, juste une couleur au mauvais endroit.
    //
    // Les couples viennent des vecteurs figés plus haut, et rien n'est recalculé ici : la
    // sélection elle-même reste celle que `spec_maquette.rs` tient — « une cible est retenue si
    // son centre est dans le rectangle ». Ce test compose, il ne redéfinit pas.
    const UN_SIXIEME: f32 = 1.0 / 6.0;
    let couples: [((f32, f32), Trace, Trace); 6] = [
        // panneau large : le carré entier, puis sa moitié gauche, puis un quart
        (
            PANNEAU_LARGE,
            (MARGE_LARGE, 0.0, 0.875, 1.0),
            (0.0, 0.0, 1.0, 1.0),
        ),
        (
            PANNEAU_LARGE,
            (MARGE_LARGE, 0.0, 0.5, 1.0),
            (0.0, 0.0, 0.5, 1.0),
        ),
        (
            PANNEAU_LARGE,
            (0.25, 0.25, 0.5, 0.75),
            (UN_SIXIEME, 0.25, 0.5, 0.75),
        ),
        // panneau haut : le carré entier, puis sa moitié haute, puis un quart
        (
            PANNEAU_HAUT,
            (0.0, UN_SIXIEME, 1.0, 1.0 - UN_SIXIEME),
            (0.0, 0.0, 1.0, 1.0),
        ),
        (
            PANNEAU_HAUT,
            (0.0, UN_SIXIEME, 1.0, 0.5),
            (0.0, 0.0, 1.0, 0.5),
        ),
        (
            PANNEAU_HAUT,
            (0.25, 0.25, 0.75, 0.5),
            (0.25, 0.125, 0.75, 0.5),
        ),
    ];
    for (nom_vue, plan) in vues() {
        let mut partiels = 0usize;
        for (panneau, trace, dans_le_carre) in couples {
            let obtenu = retenues(&plan, panneau, trace);
            let attendu = plan.dans(
                Place {
                    x: dans_le_carre.0,
                    y: dans_le_carre.1,
                },
                Place {
                    x: dans_le_carre.2,
                    y: dans_le_carre.3,
                },
            );
            assert_eq!(
                noms(&obtenu),
                noms(&attendu),
                "en {nom_vue}, le tracé de panneau {trace:?} sur {}×{} désigne le carré \
                 {dans_le_carre:?}, et donc les mêmes LED",
                panneau.0,
                panneau.1
            );
            assert!(
                !obtenu.is_empty(),
                "en {nom_vue}, le tracé {trace:?} recouvre une partie du boîtier : s'il ne \
                 retenait rien, ce test ne prouverait rien"
            );
            if obtenu.len() < LED_DU_BOITIER {
                partiels += 1;
            }
        }
        assert!(
            partiels >= 4,
            "en {nom_vue}, il faut des tracés qui ne retiennent qu'une partie du boîtier pour que \
             ce test distingue une conversion juste d'une conversion trop large — {partiels}"
        );
    }
}

#[test]
fn un_geste_parti_de_la_marge_retient_ce_que_le_meme_geste_parti_du_bord_retient() {
    // Le cœur de l'issue, du point de vue de qui s'en sert : « commencer un rectangle dans une
    // marge, le tirer sur les LED et le relâcher fonctionne comme n'importe où ailleurs ».
    //
    // Pourquoi ça compte : c'est exactement le geste qui ne trace rien aujourd'hui. Le formuler
    // comme une **équivalence** — même arrivée, deux départs, même sélection — le rend vérifiable
    // sans nommer une seule LED : la marge n'ajoute que de la surface hors du carré, où aucune LED
    // ne vit, donc la partir de plus loin ne peut ni en ajouter ni en retirer une.
    //
    // Une conversion qui écrêterait passerait le premier assert et échouerait ici : le départ
    // ramené au bord du carré donne bien la même sélection, mais alors le geste **entièrement**
    // dans la marge du test suivant en attraperait, lui.
    // Chaque panneau a ses arrivées, choisies pour recouvrir des LED sur ce panneau-là : les deux
    // formes ne cadrent pas la même chose, et une arrivée qui vaut un geste utile sur l'une peut
    // ne rien recouvrir sur l'autre.
    const MARGE_HAUTE: f32 = 1.0 / 6.0;
    let cas: [(&str, (f32, f32), Trace, Trace); 6] = [
        // (quoi, panneau, départ dans la marge → arrivée, départ au bord du carré → même arrivée)
        (
            "la marge gauche",
            PANNEAU_LARGE,
            (0.0, 0.0, 0.5, 0.66),
            (MARGE_LARGE, 0.0, 0.5, 0.66),
        ),
        (
            "la marge gauche",
            PANNEAU_LARGE,
            (0.0, 0.0, 0.875, 1.0),
            (MARGE_LARGE, 0.0, 0.875, 1.0),
        ),
        (
            "la marge gauche",
            PANNEAU_LARGE,
            (0.0, 0.0, 0.6, 0.2),
            (MARGE_LARGE, 0.0, 0.6, 0.2),
        ),
        (
            "la marge haute",
            PANNEAU_HAUT,
            (0.0, 0.0, 0.5, 0.66),
            (0.0, MARGE_HAUTE, 0.5, 0.66),
        ),
        (
            "la marge haute",
            PANNEAU_HAUT,
            (0.0, 0.0, 1.0, 5.0 / 6.0),
            (0.0, MARGE_HAUTE, 1.0, 5.0 / 6.0),
        ),
        (
            "la marge haute",
            PANNEAU_HAUT,
            (0.0, 0.0, 0.9, 0.6),
            (0.0, MARGE_HAUTE, 0.9, 0.6),
        ),
    ];
    for (nom_vue, plan) in vues() {
        for (quoi, panneau, depuis_la_marge, depuis_le_bord) in cas {
            let marge = retenues(&plan, panneau, depuis_la_marge);
            let bord = retenues(&plan, panneau, depuis_le_bord);
            assert_eq!(
                noms(&marge),
                noms(&bord),
                "en {nom_vue}, un geste parti de {quoi} — {depuis_la_marge:?} — retient ce que le \
                 même geste parti du bord du carré — {depuis_le_bord:?} — retient"
            );
            assert!(
                !marge.is_empty(),
                "en {nom_vue}, le geste {depuis_la_marge:?} doit recouvrir des LED, sinon \
                 l'égalité ci-dessus est celle de deux listes vides et ne prouve rien"
            );
        }
    }
}

#[test]
fn un_trace_entierement_dans_une_marge_ne_retient_aucune_led() {
    // Critère d'acceptation : « un rectangle entièrement hors du carré ne sélectionne rien et ne
    // casse rien ». L'issue insiste : « et c'est un résultat, pas une erreur ».
    //
    // Pourquoi ça compte : la fenêtre s'appuie sur la liste vide pour désélectionner — c'est figé
    // par `spec_maquette.rs`. Une marge devenue cliquable doit donc désélectionner, comme
    // n'importe quel coin vide du carré, et surtout pas attraper la rangée de LED la plus proche.
    let cas: [(&str, (f32, f32), Trace); 6] = [
        (
            "la marge gauche d'un panneau large",
            PANNEAU_LARGE,
            (0.0, 0.0, 0.10, 1.0),
        ),
        (
            "la marge droite d'un panneau large",
            PANNEAU_LARGE,
            (0.90, 0.0, 1.0, 1.0),
        ),
        (
            "un clic simple dans la marge gauche",
            PANNEAU_LARGE,
            (0.05, 0.5, 0.05, 0.5),
        ),
        (
            "la marge haute d'un panneau haut",
            PANNEAU_HAUT,
            (0.0, 0.0, 1.0, 0.10),
        ),
        (
            "la marge basse d'un panneau haut",
            PANNEAU_HAUT,
            (0.0, 0.90, 1.0, 1.0),
        ),
        (
            "un clic simple dans la marge basse",
            PANNEAU_HAUT,
            (0.5, 0.95, 0.5, 0.95),
        ),
    ];
    for (nom_vue, plan) in vues() {
        for (quoi, panneau, trace) in cas {
            assert_eq!(
                noms(&retenues(&plan, panneau, trace)),
                Vec::<String>::new(),
                "en {nom_vue}, {quoi} — le tracé {trace:?} — est hors du carré : il ne retient \
                 rien, et c'est un résultat"
            );
        }
    }
}

#[test]
fn un_geste_qui_balaie_tout_le_panneau_retient_les_cent_vingt_quatre_led() {
    // Comportement attendu : le geste porte sur tout le panneau. Le balayer d'un coin à l'autre
    // couvre donc au moins le carré, et le carré porte la maquette entière.
    //
    // Pourquoi ça compte : c'est la moitié « rien ne manque » de l'issue, en face de la moitié
    // « rien n'est pris en trop » des deux tests précédents. Une conversion qui rétrécirait au
    // lieu d'élargir — en divisant par le panneau plutôt que par le carré, par exemple — laisserait
    // ce geste en retenir cent dix, et on ne s'en apercevrait qu'en comptant.
    //
    // La comparaison se fait contre `(−1, −1)`–`(2, 2)`, le rectangle que `spec_maquette.rs`
    // emploie déjà pour dire « toute la maquette » : on ne réécrit pas ici ce que retient un geste
    // qui déborde de partout, on le cite.
    let tout_le_cadre = (Place { x: -1.0, y: -1.0 }, Place { x: 2.0, y: 2.0 });
    for (nom_vue, plan) in vues() {
        // Précondition de ce test, et elle est mesurée : aucune LED ne touche le bord du cadre.
        // Le balayage d'un panneau large donne `y` de 0 à 1 exactement, et `spec_maquette.rs`
        // laisse ouvert le sort d'un centre pile sur un bord. Si la maquette venait à coller une
        // LED au bord, c'est ce message qu'il faudrait lire — et non un décompte inexpliqué.
        for (nom_led, _, place) in toutes_les_leds(&plan) {
            assert!(
                place.x > 0.0 && place.x < 1.0 && place.y > 0.0 && place.y < 1.0,
                "en {nom_vue}, {nom_led} est en {place:?} : ce test suppose qu'aucune LED ne \
                 touche le bord du cadre, sans quoi le balayage du panneau la laisse au bord"
            );
        }

        let attendu = plan.dans(tout_le_cadre.0, tout_le_cadre.1);
        assert_eq!(
            attendu.len(),
            LED_DU_BOITIER,
            "en {nom_vue}, un rectangle qui déborde de partout retient les cent vingt-quatre LED"
        );

        for (nom_panneau, panneau) in [
            ("large", PANNEAU_LARGE),
            ("haut", PANNEAU_HAUT),
            ("carré", PANNEAU_CARRE),
        ] {
            assert_eq!(
                noms(&retenues(&plan, panneau, (0.0, 0.0, 1.0, 1.0))),
                noms(&attendu),
                "en {nom_vue}, balayer tout un panneau {nom_panneau} retient le boîtier entier"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 7 — rien ne casse, et rien n'est pris par débordement
// ---------------------------------------------------------------------------

#[test]
fn une_coordonnee_hors_du_cadre_ne_fait_pas_paniquer_ni_prendre_de_led_par_debordement() {
    // Point de vigilance de l'issue : « rien ne doit s'y casser, et une LED ne doit pas être
    // sélectionnée parce qu'un calcul a débordé ».
    //
    // Pourquoi ça compte : la souris ne s'arrête pas au bord du panneau. Un glissement commencé
    // dans la maquette et poursuivi hors de la fenêtre continue d'être rapporté, avec des
    // fractions négatives ou franchement supérieures à 1. Ces valeurs traversent la conversion et
    // arrivent dans `dans`, qui les accepte — `spec_maquette.rs` le fige — à condition qu'elles
    // restent des nombres. Un `NaN` ou un infini, eux, feraient basculer des comparaisons de
    // flottants dans un sens qu'on ne choisit pas : la sélection deviendrait vide ou entière, au
    // hasard du sens de la pente.
    //
    // Les rectangles ci-dessous sont tous **franchement d'un seul côté** du panneau, donc du
    // carré : celui qui les traverserait retiendrait légitimement tout le boîtier, et ne dirait
    // rien du débordement.
    const LOIN: [Trace; 6] = [
        (-10.0, -10.0, -5.0, -5.0),
        (5.0, 0.0, 9.0, 1.0),
        (0.0, 5.0, 1.0, 9.0),
        (-3.0, 0.2, -2.0, 0.8),
        (0.2, -3.0, 0.8, -2.0),
        (-1e6, -1e6, -1e5, -1e5),
    ];
    for (nom_vue, plan) in vues() {
        for (nom_panneau, panneau) in [
            ("large", PANNEAU_LARGE),
            ("haut", PANNEAU_HAUT),
            ("carré", PANNEAU_CARRE),
        ] {
            for trace in LOIN {
                let (x0, y0, x1, y1) = trace_dans_la_maquette(
                    panneau.0, panneau.1, trace.0, trace.1, trace.2, trace.3,
                );
                for (nom_du_nombre, valeur) in [("x0", x0), ("y0", y0), ("x1", x1), ("y1", y1)] {
                    assert!(
                        valeur.is_finite(),
                        "sur un panneau {nom_panneau}, le tracé {trace:?} rend {nom_du_nombre} = \
                         {valeur} : une coordonnée hors du cadre reste un nombre"
                    );
                }
                assert_eq!(
                    noms(&retenues(&plan, panneau, trace)),
                    Vec::<String>::new(),
                    "en {nom_vue}, sur un panneau {nom_panneau}, le tracé {trace:?} est tout \
                     entier hors du boîtier : il ne retient aucune LED"
                );
            }
        }
    }
}

#[test]
fn un_panneau_sans_surface_ne_retient_aucune_led() {
    // Comportement attendu, déduit du même point de vigilance : « rien ne doit se casser ». Un
    // panneau a une largeur et une hauteur nulles avant sa première mise en page, et une fenêtre
    // réduite peut lui rendre une dimension nulle. Le côté du carré vaut alors zéro, et la
    // conversion divise par lui.
    //
    // Pourquoi ça compte : c'est le seul cas où la conversion peut rendre autre chose qu'un
    // nombre, et la faute qu'on écrit en s'en protégeant mal est pire que celle qu'on évite —
    // rendre le tracé inchangé « à défaut de mieux » ferait retenir les cent vingt-quatre LED par
    // un geste sur un panneau qui n'a pas de surface. Ce test n'exige donc rien de la valeur
    // rendue : il exige que rien ne panique, et qu'aucune LED ne soit prise.
    for (nom_vue, plan) in vues() {
        for (nom_panneau, panneau) in [
            ("sans surface", (0.0, 0.0)),
            ("sans hauteur", (800.0, 0.0)),
            ("sans largeur", (0.0, 600.0)),
        ] {
            assert_eq!(
                noms(&retenues(&plan, panneau, (0.0, 0.0, 1.0, 1.0))),
                Vec::<String>::new(),
                "en {nom_vue}, un panneau {nom_panneau} n'a pas de carré où viser : le geste qui \
                 le balaie ne retient aucune LED"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 8 — le clic simple, qui est le même geste réduit à un point
// ---------------------------------------------------------------------------

#[test]
fn un_clic_du_panneau_designe_la_led_qu_il_vise() {
    // Critère d'acceptation : « la sélection au clic simple sur une LED continue de marcher ».
    //
    // Pourquoi ça compte : le clic passe par la même conversion que le glissement — c'est un
    // rectangle dont les deux coins sont confondus — puis par `sous`, dont `spec_plan.rs` tient le
    // contrat : « la LED la plus proche dans son rayon de prise, et `None` au-delà ». Une
    // conversion juste sur les rectangles et fausse d'une demi-marge sur les points ferait changer
    // la couleur d'une LED voisine de celle qu'on visait, ce que personne ne relie jamais au clic
    // qui l'a causée.
    //
    // Les cent vingt-quatre, et non un échantillon : on vise le centre exact de chaque LED, ramené
    // en coordonnées de panneau par l'inverse de l'affine figée plus haut —
    //
    //   x = (sx · 600 + 100) / 800     pour un panneau large de 800 × 600
    //
    // — et on exige que la conversion rende le centre d'où l'on est parti, puis que `sous` y
    // désigne **ce qu'il désigne au centre même**. Une confusion ne toucherait qu'une paire, et ce
    // sont les onze LED d'une barrette, les plus serrées de la maquette, qu'un échantillon
    // manquerait.
    //
    // L'égalité est délibérément posée contre `sous(centre)` et non contre la cible attendue : en
    // isométrique, `spec_maquette.rs` autorise une barrette à tomber sur un anneau, et exiger ici
    // que chaque centre désigne sa propre LED reviendrait à interdire à la projection honnête ce
    // qu'elle a le droit de faire. Ce qui est du ressort de #121, c'est que passer par le panneau
    // ne change rien à la réponse.
    const COTE: f32 = 600.0;
    const MARGE_EN_PIXELS: f32 = 100.0;
    for (nom_vue, plan) in vues() {
        for (nom_led, _, centre) in toutes_les_leds(&plan) {
            let x = (centre.x * COTE + MARGE_EN_PIXELS) / PANNEAU_LARGE.0;
            let y = centre.y;
            let (cx, cy, cx1, cy1) =
                trace_dans_la_maquette(PANNEAU_LARGE.0, PANNEAU_LARGE.1, x, y, x, y);
            presque(
                (cx, cy, cx1, cy1),
                (centre.x, centre.y, centre.x, centre.y),
                &format!(
                    "en {nom_vue}, le point ({x}, {y}) du panneau est le centre de {nom_led}, et \
                     un clic reste un point"
                ),
            );
            let designee = plan.sous(Place { x: cx, y: cy });
            assert!(
                designee.is_some(),
                "en {nom_vue}, un clic en ({x}, {y}) sur le panneau tombe sur le centre de \
                 {nom_led} : il doit désigner quelque chose"
            );
            assert_eq!(
                designee,
                plan.sous(centre),
                "en {nom_vue}, un clic en ({x}, {y}) sur le panneau désigne ce que le centre de \
                 {nom_led} ({centre:?}) désigne dans le carré"
            );
        }
    }
}
