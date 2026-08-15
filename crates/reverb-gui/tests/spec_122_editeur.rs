//! Tests d'intention de l'éditeur de courbe **par points sur le tracé** (issue #122).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #122 et son commentaire de décisions — qui
//! fait autorité au même titre qu'elle. Rien de `crates/*/src/` n'a été lu pour les produire, hors
//! les signatures publiques strictement nécessaires à la compilation : `TRACE_ASPECT`,
//! `TRACE_FROID`, `TRACE_CHAUD` (#113), `Courbe`, `CourbeInvalide` (#99, descendus dans
//! `reverb-proto` par #113).
//!
//! À l'écriture de ce fichier, `palier_dans_le_trace`, `point_du_palier`, `palier_saisi`,
//! `palier_deplace`, `palier_retire`, `palier_ajoute`, `PALIERS_MAX` et `RAYON_DE_SAISIE`
//! **n'existent pas** : la compilation doit échouer sur eux, et sur eux seuls. C'est la phase rouge.
//!
//! Rien ici n'ouvre de fenêtre, ne lit une souris, ne parle à un socket, ni ne touche un bus.
//!
//! ## Ce que ces tests prolongent
//!
//! `spec_courbe_fenetre.rs` (#113) fige déjà les deux moitiés qui ne bougent pas :
//!
//! - **le juge est `Courbe::depuis`**, et la fenêtre refuse exactement ce que le démon refuse, avec
//!   la **même raison** — `la_fenetre_refuse_exactement_ce_que_le_demon_refuse_et_pour_la_meme_raison` ;
//! - **le tracé est `Courbe::consigne`**, point par point —
//!   `chaque_point_du_trace_est_celui_que_consigne_rend`.
//!
//! #122 n'ajoute qu'une chose : les **gestes**. Ce fichier ne redéfinit aucune de ces deux
//! propriétés, il les invoque — un geste rend une courbe, et c'est `Courbe::depuis` qui dit si
//! elle vaut quelque chose.
//!
//! ## Le contrat que ces tests figent
//!
//! ```ignore
//! // dans crates/reverb-gui/src/reglages.rs, à côté de TRACE_ASPECT et de CourbeEditee.
//!
//! /// Le nombre de paliers qu'un éditeur accepte de porter (commentaire de décisions de #122).
//! pub const PALIERS_MAX: usize = 8;
//!
//! /// Le rayon dans lequel un clic attrape un point, dans le repère du tracé.
//! pub const RAYON_DE_SAISIE: f32 = 0.12;
//!
//! /// Le palier que désigne un point du cadre, dans le repère du tracé.
//! pub fn palier_dans_le_trace(x: f32, y: f32) -> (i32, u8);
//!
//! /// Où un palier se pose dans le cadre — l'inverse de la précédente.
//! pub fn point_du_palier(palier: (i32, u8)) -> (f32, f32);
//!
//! /// Le rang du palier qu'un clic attrape, s'il en attrape un.
//! pub fn palier_saisi(paliers: &[(i32, u8)], x: f32, y: f32) -> Option<usize>;
//!
//! /// La courbe où le palier de rang `rang` a été traîné jusqu'à `(x, y)`.
//! pub fn palier_deplace(paliers: &[(i32, u8)], rang: usize, x: f32, y: f32)
//!     -> Result<Vec<(i32, u8)>, CourbeInvalide>;
//!
//! /// La courbe privée de son palier de rang `rang` — refusée s'il n'en resterait qu'un.
//! pub fn palier_retire(paliers: &[(i32, u8)], rang: usize)
//!     -> Result<Vec<(i32, u8)>, CourbeInvalide>;
//!
//! /// La courbe augmentée d'un palier en `(x, y)` — refusée au-delà de `PALIERS_MAX`.
//! pub fn palier_ajoute(paliers: &[(i32, u8)], x: f32, y: f32)
//!     -> Result<Vec<(i32, u8)>, CourbeInvalide>;
//! ```
//!
//! ### Le repère : celui du tracé, `TRACE_ASPECT` × 1
//!
//! ⚠️ **La conversion part de `TRACE_ASPECT`, et d'aucun second chiffre** — l'issue l'exige mot pour
//! mot : « cette conversion doit partir de la même constante, jamais d'un second chiffre ».
//!
//! Le repère de ces six fonctions est donc **exactement celui que `commandes_de_trace` émet** :
//! `x` court sur `0..=TRACE_ASPECT`, `y` sur `0..=1`, et le cadre tient ce même rapport (#113,
//! README). Un point du tracé et une poignée d'éditeur vivent ainsi dans le même système de
//! coordonnées : il n'y en a pas un second à tenir d'accord.
//!
//! ```text
//! température = TRACE_FROID + (x / TRACE_ASPECT) · (TRACE_CHAUD − TRACE_FROID)
//! consigne    = (1 − y) · 100                      ⚠️ y croît vers le BAS
//! ```
//!
//! Deux conséquences qui se testent :
//!
//! - **le cadre étant à `TRACE_ASPECT` × 1 et mis à l'échelle uniformément, un pas de `d` en `x`
//!   vaut le même nombre de pixels qu'un pas de `d` en `y`.** Le rayon de saisie est donc un vrai
//!   rayon, sur un vrai disque, et non une ellipse déguisée ;
//! - **les quatre coins du cadre sont les quatre extrêmes de la plage tracée.** Un implémenteur qui
//!   écrirait `x / 4.0` au lieu de `x / TRACE_ASPECT` passerait aujourd'hui et casserait le jour où
//!   la constante bouge — d'où des attentes toutes exprimées **à partir d'elle**.
//!
//! ### Le plancher : **un** palier, et l'histoire de ce chiffre
//!
//! ⚠️ **Ce paragraphe existe pour quelqu'un qui relirait l'issue sans son dernier commentaire.** Le
//! premier commentaire de décisions posait un plancher de **deux** paliers, et le justifiait ainsi :
//!
//! > **on ne peut pas descendre sous deux paliers**, `Courbe::depuis` refusant une courbe qui n'en
//! > a pas au moins deux. Le clic droit sur l'avant-dernier point est donc inerte, et le dit.
//!
//! **La phrase est fausse, et vérifiée fausse.** `Courbe::depuis` refuse une courbe **vide**, une
//! consigne au-dessus de cent, deux paliers à la même température et une température qui décroît.
//! Elle n'exige **pas** deux paliers, et deux fichiers de tests d'intention figent l'inverse : le
//! `une_courbe_a_un_seul_palier_est_plate` de #99 côté démon, et le
//! `une_courbe_a_un_seul_palier_reste_acceptee` de #113 côté fenêtre. **Une consigne fixe est une
//! courbe parfaitement sensée** — la même à toute température —, et `regule courbe 45000:80` doit
//! continuer de passer par le socket.
//!
//! Le chiffre « deux » ne reposait donc que sur cette phrase. Un commentaire ultérieur de l'issue
//! l'a corrigé, et ce fichier applique la correction : **le plancher de l'éditeur est UN palier**,
//! celui du protocole et rien de plus. Garder deux ferait de la fenêtre le seul chemin incapable
//! d'exprimer une courbe que le socket accepte — une contrainte inventée, justifiée par une erreur.
//!
//! Deux conséquences testées :
//!
//! - un clic droit retire un palier **tant qu'il en reste au moins un** ; retirer l'avant-dernier
//!   est permis et rend une courbe plate et valide ;
//! - sur le **dernier**, rien ne bouge et l'éditeur le dit.
//!
//! ⚠️ **Le juge de ce plancher est l'éditeur, jamais `Courbe::depuis`** — c'est la moitié de la
//! correction qui survit au changement de chiffre. `palier_retire` le porte, et
//! `le_juge_du_plancher_est_l_editeur_car_une_courbe_a_un_palier_reste_valide` protège
//! `Courbe::depuis` contre la « correction » qu'on serait tenté d'y faire en relisant l'ancien
//! commentaire.
//!
//! ### Ce que ces tests ne figent pas, et pourquoi
//!
//! - **Le dessin** — poignées, rayon dessiné, curseur, couleur du point saisi. « Ce qui est du
//!   calcul se teste, ce qui est du dessin se regarde » : cf. l'exemple `apercu`.
//! - **Quel bouton de souris fait quoi.** `palier_deplace` et `palier_retire` sont deux fonctions ;
//!   les brancher sur le clic gauche et le clic droit est du `.slint`, il n'entre par aucune
//!   fonction.
//! - **Les barres de #113**, gardées en réglage fin par le commentaire de décisions, et le palier
//!   « sélectionné » qu'elles reflètent : c'est de l'état de fenêtre, pas un calcul. `CourbeEditee`,
//!   `regler_temperature` et `regler_consigne` ne bougent pas, et ce fichier n'y touche pas.
//! - **Ce que `Courbe::depuis` accepte ou refuse** : c'est #99, et `spec_regulation_hote.rs` le
//!   tient. Ce fichier n'exige qu'une chose : que chaque geste lui soumette son résultat.
//! - **L'envoi par le socket** : `ordre_de_courbe` (#113) s'en charge, et son fichier le tient.

use reverb_gui::reglages::{
    PALIERS_MAX, RAYON_DE_SAISIE, TRACE_ASPECT, TRACE_CHAUD, TRACE_FROID, palier_ajoute,
    palier_dans_le_trace, palier_deplace, palier_retire, palier_saisi, point_du_palier,
};
// La porte du socket, posée par #113 : ce fichier ne la redéfinit pas, il vérifie que ce que les
// gestes composent peut la franchir.
use reverb_gui::telemetrie::ordre_de_courbe;
use reverb_proto::regulation::{Courbe, CourbeInvalide};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// Les millidegrés d'un nombre entier de degrés.
///
/// ⚠️ **Tout est en millidegrés entiers, jamais en degrés flottants** — la règle de #99, que #122
/// répète : « le millidegré reste l'unité. Un point traîné rend une température en millidegrés,
/// jamais un flottant de degrés arrondi à l'affichage. »
const fn degres(entiers: i32) -> i32 {
    entiers * 1_000
}

/// La courbe par défaut de #99 : 30 % à 35 °C, 60 % à 45 °C, 100 % à 50 °C.
const PALIERS_PAR_DEFAUT: [(i32, u8); 3] = [(degres(35), 30), (degres(45), 60), (degres(50), 100)];

/// L'étendue de température que le cadre couvre, en millidegrés : 50 °C de large.
const PLAGE: i32 = TRACE_CHAUD - TRACE_FROID;

/// L'écart toléré entre deux températures, en millidegrés.
///
/// **Un millième de degré.** Trois ordres de grandeur sous ce que la sonde du liquide sait rendre —
/// elle bruite de ±0,3 °C, soit ±300 fois cette tolérance (README, #111) — et de quoi absorber les
/// deux arrondis `f32` d'un aller-retour palier → point → palier sur une plage de 50 000
/// millidegrés : l'erreur relative d'un `f32` y vaut de l'ordre du centième de millidegré.
///
/// Elle ne sert **pas** à excuser un arrondi au degré : le test
/// `un_point_traine_rend_des_millidegres_et_non_des_degres_arrondis` l'interdit explicitement.
const TOLERANCE_MILLI: i32 = 1;

/// L'écart toléré entre deux coordonnées du repère du tracé.
///
/// Le cadre mesuré fait 359 × 88 px (#113, README) : une unité de ce repère vaut donc ~88 px, et
/// `1e-4` en est un centième de pixel. Quatre ordres de grandeur sous le rayon de saisie, qui est
/// la plus petite distance décidant de quelque chose ici.
const EPSILON: f32 = 1e-4;

/// Une courbe de huit paliers — le plafond du commentaire de décisions, atteint.
///
/// De 25 à 60 °C par pas de cinq degrés : entièrement dans la plage tracée, et régulièrement
/// répartie, pour que les tests de saisie ne tombent jamais sur deux poignées confondues.
const PALIERS_AU_PLAFOND: [(i32, u8); 8] = [
    (degres(25), 20),
    (degres(30), 30),
    (degres(35), 40),
    (degres(40), 50),
    (degres(45), 60),
    (degres(50), 70),
    (degres(55), 85),
    (degres(60), 100),
];

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

/// L'abscisse, dans le repère du tracé, d'une température en millidegrés.
///
/// ⚠️ **Elle part de `TRACE_ASPECT`**, comme la fonction qu'elle sert à interroger : la calculer
/// avec un `4.0` écrit ici rendrait ces tests complices du défaut qu'ils traquent.
fn abscisse(milli: i32) -> f32 {
    (milli - TRACE_FROID) as f32 / PLAGE as f32 * TRACE_ASPECT
}

/// L'ordonnée, dans le repère du tracé, d'une consigne en pourcent. ⚠️ `y` croît vers le bas.
fn ordonnee(pourcent: u8) -> f32 {
    1.0 - pourcent as f32 / 100.0
}

/// Le point du repère où un palier se pose, calculé par ce fichier et non par la fonction testée.
fn point(palier: (i32, u8)) -> (f32, f32) {
    (abscisse(palier.0), ordonnee(palier.1))
}

/// La courbe qu'on exige d'un déplacement, avec un refus qui dit pourquoi il n'y en a pas eu.
fn deplace(paliers: &[(i32, u8)], rang: usize, x: f32, y: f32) -> Vec<(i32, u8)> {
    palier_deplace(paliers, rang, x, y).unwrap_or_else(|erreur| {
        panic!(
            "Le palier de rang {rang} devait pouvoir être traîné en ({x}, {y}) : {paliers:?}\n  \
             Refus : {}",
            erreur.raison
        )
    })
}

/// Le refus qu'on exige d'un déplacement, et le refus lui-même.
fn refus_de_deplacement(paliers: &[(i32, u8)], rang: usize, x: f32, y: f32) -> CourbeInvalide {
    match palier_deplace(paliers, rang, x, y) {
        Ok(rendus) => panic!(
            "Traîner le palier de rang {rang} en ({x}, {y}) devait être refusé **en le disant**, \
             l'éditeur a rendu {rendus:?}\n  Départ : {paliers:?}"
        ),
        Err(erreur) => erreur,
    }
}

/// La courbe qu'on exige d'un retrait, avec un refus qui dit pourquoi il n'y en a pas eu.
fn retire(paliers: &[(i32, u8)], rang: usize) -> Vec<(i32, u8)> {
    palier_retire(paliers, rang).unwrap_or_else(|erreur| {
        panic!(
            "Le palier de rang {rang} devait pouvoir être retiré : {paliers:?}\n  Refus : {}",
            erreur.raison
        )
    })
}

/// Le refus qu'on exige d'un retrait, et le refus lui-même.
fn refus_de_retrait(paliers: &[(i32, u8)], rang: usize) -> CourbeInvalide {
    match palier_retire(paliers, rang) {
        Ok(rendus) => panic!(
            "Retirer le palier de rang {rang} devait être refusé **en le disant**, l'éditeur a \
             rendu {rendus:?}\n  Départ : {paliers:?}"
        ),
        Err(erreur) => erreur,
    }
}

/// La courbe qu'on exige d'un ajout, avec un refus qui dit pourquoi il n'y en a pas eu.
fn ajoute(paliers: &[(i32, u8)], x: f32, y: f32) -> Vec<(i32, u8)> {
    palier_ajoute(paliers, x, y).unwrap_or_else(|erreur| {
        panic!(
            "Un palier devait pouvoir être ajouté en ({x}, {y}) : {paliers:?}\n  Refus : {}",
            erreur.raison
        )
    })
}

/// Le refus qu'on exige d'un ajout, et le refus lui-même.
fn refus_d_ajout(paliers: &[(i32, u8)], x: f32, y: f32) -> CourbeInvalide {
    match palier_ajoute(paliers, x, y) {
        Ok(rendus) => panic!(
            "Ajouter un palier en ({x}, {y}) devait être refusé **en le disant**, l'éditeur a rendu \
             {rendus:?}\n  Départ : {paliers:?}"
        ),
        Err(erreur) => erreur,
    }
}

/// Le verdict du démon sur ce qu'un geste vient de rendre : la courbe si le geste a abouti, `None`
/// s'il a été refusé — et jamais un refus muet.
///
/// C'est le juge de #113 appliqué aux trois gestes, et l'exigence de l'issue : « Le juge reste
/// `Courbe::depuis`, celui du démon ».
fn le_demon_juge(
    geste: &str,
    verdict: Result<Vec<(i32, u8)>, CourbeInvalide>,
) -> Option<Vec<(i32, u8)>> {
    match verdict {
        Ok(rendus) => {
            Courbe::depuis(&rendus).unwrap_or_else(|erreur| {
                panic!(
                    "{geste} rend {rendus:?}, que le démon refuse — « {} ». Le refus arriverait \
                     **après** le geste, dans un journal, au lieu d'arriver sous la souris.",
                    erreur.raison
                )
            });
            Some(rendus)
        }
        Err(erreur) => {
            assert!(
                !erreur.raison.trim().is_empty(),
                "{geste} est refusé **sans raison** : un geste inerte et muet se lit comme une \
                 fenêtre en panne"
            );
            None
        }
    }
}

/// Une courbe qu'on exige valide, avec un refus qui recopie les paliers fautifs.
fn courbe(paliers: &[(i32, u8)]) -> Courbe {
    Courbe::depuis(paliers).unwrap_or_else(|erreur| {
        panic!(
            "Ces paliers devaient être acceptés : {paliers:?}\n  Refus : {}",
            erreur.raison
        )
    })
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

// ⚠️ `assertions_on_constants` reproche à ces assertions d'avoir une valeur
// connue à la compilation, donc d'être « toujours vraies ». C'est exactement ce
// que ce test existe pour garantir : elles sont vraies des valeurs
// d'aujourd'hui, et le test est là pour qu'elles le restent de celles de demain.
// L'exception est posée ici plutôt que les assertions réécrites — un
// `const { assert!(…) }` perdrait le message, qui nomme les chiffres fautifs.
//
// Seule modification apportée à ce fichier après son écriture, et elle ne touche
// aucune assertion.
#[allow(clippy::assertions_on_constants)]
#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tout ce qui suit suppose que la plage tracée monte, que la courbe par défaut y tient
    // entièrement, que le plafond de huit laisse de la place au-dessus de ses trois paliers, et
    // que huit poignées régulièrement réparties ne se recouvrent pas. Si l'un de ces repères se
    // dégradait, plusieurs tests deviendraient vrais sans rien vérifier — et personne ne le
    // verrait. Ce test est là pour que la panne soit ici.
    const {
        assert!(
            TRACE_CHAUD > TRACE_FROID,
            "la plage tracée doit monter, sinon toutes les abscisses de ce fichier se replient"
        );
        assert!(
            TRACE_ASPECT > 0.0,
            "le cadre a une largeur, sinon il n'y a pas de repère où viser"
        );
        assert!(
            PALIERS_MAX == 8,
            "le commentaire de décisions de #122 tranche : « **huit au maximum** »"
        );
    }

    // La courbe livrée en a trois — le commentaire le dit —, et le plafond doit laisser de quoi en
    // ajouter, sinon le geste d'ajout ne se testerait que par son refus.
    assert_eq!(
        Courbe::defaut().paliers(),
        PALIERS_PAR_DEFAUT,
        "la courbe par défaut est le tableau de #99, en millidegrés"
    );
    assert!(
        PALIERS_PAR_DEFAUT.len() < PALIERS_MAX,
        "la courbe livrée doit laisser de la place sous le plafond : {} paliers pour {PALIERS_MAX}",
        PALIERS_PAR_DEFAUT.len()
    );
    assert_eq!(
        PALIERS_AU_PLAFOND.len(),
        PALIERS_MAX,
        "le banc « au plafond » doit valoir exactement le plafond, sinon il ne le teste pas"
    );

    // Tous les paliers des bancs tiennent dans la plage tracée : hors d'elle, un point serait
    // dessiné hors du cadre et aucun clic ne pourrait l'attraper.
    for paliers in [&PALIERS_PAR_DEFAUT[..], &PALIERS_AU_PLAFOND[..]] {
        courbe(paliers);
        for (milli, pourcent) in paliers {
            assert!(
                (TRACE_FROID..=TRACE_CHAUD).contains(milli),
                "{milli} m°C sort de la plage tracée {TRACE_FROID}..={TRACE_CHAUD}"
            );
            assert!(*pourcent <= 100, "{pourcent} % dépasse cent pour cent");
        }
    }

    // Et le rayon de saisie doit rester assez petit pour que huit poignées régulièrement réparties
    // ne se recouvrent jamais : sinon un clic serait ambigu par construction, et les tests de
    // saisie ne mesureraient que l'ordre du parcours.
    let ecart_minimal = TRACE_ASPECT / (PALIERS_MAX - 1) as f32;
    assert!(
        2.0 * RAYON_DE_SAISIE < ecart_minimal,
        "deux zones de saisie de rayon {RAYON_DE_SAISIE} se touchent à {ecart_minimal} d'écart : \
         huit paliers répartis sur {TRACE_ASPECT} ne pourraient plus être visés un par un"
    );
    assert!(
        RAYON_DE_SAISIE > 0.0,
        "un rayon nul n'attraperait qu'un point visé au pixel près"
    );
}

// ---------------------------------------------------------------------------
// 1 — la conversion : un point du cadre est un palier
// ---------------------------------------------------------------------------

#[test]
fn les_quatre_coins_du_cadre_sont_les_quatre_extremes_de_la_plage_tracee() {
    // Critère d'acceptation du commentaire de décisions : « La conversion pixels → (millidegrés,
    // pourcent) part de `TRACE_ASPECT`, jamais d'un second chiffre. »
    //
    // ⚠️ **`y` croît vers le bas**, comme partout en interface : le coin en haut à gauche est donc
    // le plus froid **et** le plus rapide, et le coin en bas à gauche le plus froid à l'arrêt. Une
    // erreur de signe ici produirait une courbe parfaitement valide, parfaitement tracée, et
    // renverserait la régulation du poste sans un message.
    for (nom, x, y, attendu) in [
        ("en haut à gauche", 0.0, 0.0, (TRACE_FROID, 100u8)),
        ("en bas à gauche", 0.0, 1.0, (TRACE_FROID, 0)),
        ("en haut à droite", TRACE_ASPECT, 0.0, (TRACE_CHAUD, 100)),
        ("en bas à droite", TRACE_ASPECT, 1.0, (TRACE_CHAUD, 0)),
    ] {
        let (milli, pourcent) = palier_dans_le_trace(x, y);
        assert!(
            (milli - attendu.0).abs() <= TOLERANCE_MILLI,
            "le coin {nom} ({x}, {y}) doit valoir {} m°C, il en rend {milli}",
            attendu.0
        );
        assert_eq!(
            pourcent, attendu.1,
            "le coin {nom} ({x}, {y}) doit valoir {} %, il en rend {pourcent}",
            attendu.1
        );
    }
}

#[test]
fn la_conversion_part_de_trace_aspect_et_d_aucun_second_chiffre() {
    // Le cœur de l'exigence : `x` court sur `0..=TRACE_ASPECT`, pas sur le carré unité, pas sur un
    // `4.0` recopié. Le milieu du cadre — `TRACE_ASPECT / 2` — doit donc tomber au milieu de la
    // plage tracée, et l'abscisse `1.0` au **quart** de cette plage tant que `TRACE_ASPECT` vaut
    // quatre : ce que ce test écrit à partir de la constante, jamais à partir du chiffre.
    let (milieu, moitie) = palier_dans_le_trace(TRACE_ASPECT / 2.0, 0.5);
    assert!(
        (milieu - (TRACE_FROID + PLAGE / 2)).abs() <= TOLERANCE_MILLI,
        "au milieu du cadre, la température doit être au milieu de la plage : {milieu} m°C au lieu \
         de {} m°C",
        TRACE_FROID + PLAGE / 2
    );
    assert_eq!(
        moitie, 50,
        "à mi-hauteur, la consigne doit valoir cinquante pour cent"
    );

    // Et un quart, un tiers, deux tiers du cadre — trois fractions dont aucune n'est un coin, pour
    // qu'une conversion affine fausse d'un facteur se voie ailleurs qu'aux bornes.
    for fraction in [0.25f32, 1.0 / 3.0, 2.0 / 3.0, 0.75] {
        let attendu = TRACE_FROID + (fraction * PLAGE as f32).round() as i32;
        let (milli, _) = palier_dans_le_trace(fraction * TRACE_ASPECT, 0.5);
        assert!(
            (milli - attendu).abs() <= TOLERANCE_MILLI,
            "à {fraction} du cadre, la température doit valoir {attendu} m°C, elle en rend {milli}"
        );
    }
}

#[test]
fn un_point_traine_rend_des_millidegres_et_non_des_degres_arrondis() {
    // Exigence de l'issue, mot pour mot : « **Le millidegré reste l'unité.** Un point traîné rend
    // une température en millidegrés, jamais un flottant de degrés arrondi à l'affichage. »
    //
    // Une conversion qui passerait par des degrés entiers — ce que l'éditeur affiche, forcément —
    // ferait disparaître le demi-degré sans un message, exactement comme #113 l'a exigé du socket.
    //
    // 35,5 °C et 35,7 °C tombent dans le même degré : une conversion arrondie au degré les
    // confondrait, et le point cesserait de suivre la souris par petits pas.
    let (chaud_et_demi, _) = palier_dans_le_trace(abscisse(35_500), 0.5);
    let (un_peu_plus, _) = palier_dans_le_trace(abscisse(35_700), 0.5);

    assert!(
        (chaud_et_demi - 35_500).abs() <= TOLERANCE_MILLI,
        "35,5 °C doit rendre 35500 m°C, l'éditeur en rend {chaud_et_demi}"
    );
    assert!(
        (un_peu_plus - 35_700).abs() <= TOLERANCE_MILLI,
        "35,7 °C doit rendre 35700 m°C, l'éditeur en rend {un_peu_plus}"
    );
    assert_ne!(
        chaud_et_demi, un_peu_plus,
        "deux points du même degré doivent rendre deux températures différentes : arrondis au \
         degré, ils valent tous deux 36 °C et le point cesse de suivre la souris"
    );

    // Et la valeur elle-même ne doit pas être un multiple de mille : c'est la signature d'un
    // arrondi au degré, et aucun autre test ne la verrait.
    assert_ne!(
        chaud_et_demi % 1_000,
        0,
        "35500 m°C rendu en {chaud_et_demi} : un multiple de mille trahit un arrondi au degré"
    );
}

#[test]
fn un_geste_qui_sort_du_cadre_est_ramene_a_ses_bornes() {
    // Ce n'est pas un critère écrit, c'est ce qui rend le geste utilisable : une souris sort du
    // cadre en un dixième de seconde. Deux raisons de ramener aux bornes plutôt que de laisser
    // filer :
    //
    // - une consigne est un `u8` de 0 à 100, et `Courbe::depuis` refuse au-delà (#99) : au-dessus
    //   du cadre, laisser filer produirait 110 % — donc un refus — au lieu d'un point qui bute ;
    // - un point traîné à gauche du cadre serait dessiné hors de lui, **et plus jamais
    //   attrapable** : le geste détruirait le palier au lieu de le déplacer.
    //
    // ⚠️ Ce n'est pas l'écrêtage que #121 a refusé sur la maquette. Là-bas, replier un coin sur le
    // bord du carré transformait une zone morte en **sélection au hasard** ; ici, buter contre la
    // borne est le seul résultat que l'utilisateur puisse voir et corriger.
    for (nom, x, y, attendu) in [
        ("à gauche du cadre", -1.0, 0.5, (TRACE_FROID, 50u8)),
        (
            "à droite du cadre",
            TRACE_ASPECT + 1.0,
            0.5,
            (TRACE_CHAUD, 50),
        ),
        (
            "au-dessus du cadre",
            TRACE_ASPECT / 2.0,
            -0.5,
            (TRACE_FROID + PLAGE / 2, 100),
        ),
        (
            "sous le cadre",
            TRACE_ASPECT / 2.0,
            2.0,
            (TRACE_FROID + PLAGE / 2, 0),
        ),
    ] {
        let (milli, pourcent) = palier_dans_le_trace(x, y);
        assert!(
            (milli - attendu.0).abs() <= TOLERANCE_MILLI,
            "un geste {nom} doit buter à {} m°C, il rend {milli}",
            attendu.0
        );
        assert_eq!(
            pourcent, attendu.1,
            "un geste {nom} doit buter à {} %, il rend {pourcent}",
            attendu.1
        );
        assert!(
            (TRACE_FROID..=TRACE_CHAUD).contains(&milli) && pourcent <= 100,
            "un geste {nom} rend ({milli} m°C, {pourcent} %), hors du cadre : le palier serait \
             indessinable, ou refusé par `Courbe::depuis`"
        );
    }
}

#[test]
fn la_conversion_ne_panique_pas_sur_un_cadre_qui_n_existe_pas_encore() {
    // Le repère vient d'une fenêtre, donc d'un code qui peut se tromper : avant la première mise en
    // page, le cadre n'a ni largeur ni hauteur, et la division qui produit `x` rend un `NaN`. C'est
    // le cas que `plan::trace_dans_la_maquette` traite déjà pour la maquette (#121).
    //
    // ⚠️ Un `NaN` converti par `as i32` ne panique pas : il **saturera à zéro**, c'est-à-dire à une
    // température de 0 m°C hors de la plage tracée et à une consigne de 0 % — des ventilateurs à
    // l'arrêt, silencieusement, sur un geste qui n'a jamais eu lieu. Ce test n'exige aucune valeur
    // en particulier, seulement que le résultat reste dans le cadre.
    for (x, y) in [
        (f32::NAN, f32::NAN),
        (f32::NAN, 0.5),
        (0.5, f32::NAN),
        (f32::INFINITY, f32::NEG_INFINITY),
        (f32::NEG_INFINITY, f32::INFINITY),
        (f32::MAX, f32::MIN),
    ] {
        let (milli, pourcent) = palier_dans_le_trace(x, y);
        assert!(
            (TRACE_FROID..=TRACE_CHAUD).contains(&milli),
            "({x}, {y}) rend {milli} m°C, hors de la plage tracée"
        );
        assert!(
            pourcent <= 100,
            "({x}, {y}) rend {pourcent} %, au-delà de cent pour cent"
        );
        assert!(
            palier_saisi(&PALIERS_PAR_DEFAUT, x, y).is_none(),
            "({x}, {y}) ne désigne aucun point du cadre : rien ne doit être attrapé"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — l'aller-retour : un palier, son point, et le palier de nouveau
// ---------------------------------------------------------------------------

#[test]
fn un_palier_pose_puis_relu_dans_le_cadre_est_le_meme_palier() {
    // Sans cette propriété, une poignée dérive à chaque fois qu'on la saisit sans la bouger : le
    // point serait dessiné là, relu ici, et la courbe changerait sous un clic qui ne demandait
    // rien. C'est l'aller-retour que le README exige déjà de la régulation — « l'aller-retour
    // pourcentage → duty → pourcentage est l'identité sur les 101 valeurs » (#110).
    //
    // La tolérance est explicite : `TOLERANCE_MILLI` en température — un millième de degré — et
    // **exacte** sur la consigne, qui est un entier de 0 à 100 et n'a aucune raison de bouger.
    let mut echantillons: Vec<(i32, u8)> = PALIERS_PAR_DEFAUT.to_vec();
    echantillons.extend_from_slice(&PALIERS_AU_PLAFOND);
    echantillons.extend_from_slice(&[
        (TRACE_FROID, 0),
        (TRACE_CHAUD, 100),
        (45_500, 64), // le demi-degré de #113
        (47_123, 37), // et un millidegré quelconque
        (TRACE_FROID + PLAGE / 3, 33),
    ]);

    for palier in echantillons {
        let (x, y) = point_du_palier(palier);
        let (milli, pourcent) = palier_dans_le_trace(x, y);
        assert!(
            (milli - palier.0).abs() <= TOLERANCE_MILLI,
            "{palier:?} posé en ({x}, {y}) se relit à {milli} m°C : la poignée dériverait d'un \
             clic à l'autre"
        );
        assert_eq!(
            pourcent, palier.1,
            "{palier:?} posé en ({x}, {y}) se relit à {pourcent} % : la consigne est un entier, \
             elle n'a aucune raison de bouger"
        );
    }
}

#[test]
fn un_palier_se_pose_dans_le_cadre_a_l_endroit_que_le_trace_dessine() {
    // La poignée doit tomber **sur** la courbe, sinon l'éditeur montre un point à côté du trait
    // qu'il prétend régler. C'est la promesse de la maquette — « l'aperçu montre ce que le boîtier
    // reçoit », vraie par construction — appliquée à l'éditeur, et elle passe par `Courbe::consigne`
    // (#113), la fonction que le démon exécute.
    let defaut = Courbe::defaut();
    for palier in PALIERS_PAR_DEFAUT {
        let (x, y) = point_du_palier(palier);
        let (attendu_x, attendu_y) = point(palier);
        assert!(
            (x - attendu_x).abs() <= EPSILON && (y - attendu_y).abs() <= EPSILON,
            "{palier:?} doit se poser en ({attendu_x}, {attendu_y}), il se pose en ({x}, {y})"
        );
        assert_eq!(
            defaut.consigne(palier.0),
            palier.1,
            "un palier de la courbe est par définition un point de son tracé : à {} m°C, \
             `Courbe::consigne` rend {} % et la poignée en montre {}",
            palier.0,
            defaut.consigne(palier.0),
            palier.1
        );
    }
}

#[test]
fn un_palier_hors_de_la_plage_tracee_se_pose_hors_du_cadre_et_n_y_est_pas_replie() {
    // ⚠️ **L'asymétrie est voulue** : `palier_dans_le_trace` ramène un **geste** à ses bornes — la
    // souris sort du cadre —, `point_du_palier` ne replie **jamais** un palier venu du socket.
    //
    // Le démon accepte `regule courbe 15000:20 80000:100`, hors de la plage tracée des deux côtés.
    // Replier ces deux paliers sur les bords du cadre les ferait tous deux mentir sur leur valeur,
    // et deux paliers distincts pourraient se confondre sous la même poignée — on en traînerait un
    // en croyant tenir l'autre.
    for (milli, attendue) in [
        (degres(15), abscisse(degres(15))),
        (degres(80), abscisse(degres(80))),
    ] {
        let (x, _) = point_du_palier((milli, 50));
        assert!(
            (x - attendue).abs() <= EPSILON,
            "{milli} m°C doit se poser en {attendue}, hors du cadre, il se pose en {x}"
        );
    }

    let (froid, _) = point_du_palier((degres(15), 50));
    let (chaud, _) = point_du_palier((degres(80), 50));
    assert!(
        froid < 0.0 && chaud > TRACE_ASPECT,
        "les deux paliers hors plage doivent tomber de part et d'autre du cadre : {froid} et \
         {chaud} pour un cadre de 0 à {TRACE_ASPECT}"
    );
}

// ---------------------------------------------------------------------------
// 3 — attraper un point
// ---------------------------------------------------------------------------

#[test]
fn un_clic_sur_un_point_l_attrape_et_un_clic_loin_de_tout_n_attrape_rien() {
    // Premier critère d'acceptation du commentaire de décisions : « Un clic gauche sur un point le
    // saisit ».
    for (rang, palier) in PALIERS_PAR_DEFAUT.iter().enumerate() {
        let (x, y) = point(*palier);
        assert_eq!(
            palier_saisi(&PALIERS_PAR_DEFAUT, x, y),
            Some(rang),
            "un clic exactement sur {palier:?} doit attraper le rang {rang}"
        );
    }

    // Et le contraire : un clic dans un coin vide du cadre n'attrape rien. C'est ce sur quoi la
    // fenêtre s'appuiera pour distinguer « traîner » de « ajouter » — sans quoi le premier clic
    // dans le vide déplacerait un point qu'on n'a pas visé.
    for (nom, x, y) in [
        ("le coin froid en bas", 0.0, 1.0),
        ("le coin chaud en haut", TRACE_ASPECT, 0.0),
        ("le milieu du bas", TRACE_ASPECT / 2.0, 1.0),
        ("franchement hors du cadre", -3.0, -3.0),
    ] {
        assert_eq!(
            palier_saisi(&PALIERS_PAR_DEFAUT, x, y),
            None,
            "un clic {nom} ({x}, {y}) est loin de tout point : il ne doit rien attraper"
        );
    }

    // Une courbe vide n'a aucune poignée — l'éditeur ne doit pas y trouver de rang à traîner.
    assert_eq!(palier_saisi(&[], TRACE_ASPECT / 2.0, 0.5), None);
}

#[test]
fn le_rayon_de_saisie_est_un_vrai_rayon_dans_le_repere_du_trace() {
    // ⚠️ **C'est ici que `TRACE_ASPECT` se paye ou se rate.** Le cadre tient le rapport
    // `TRACE_ASPECT` (#113, README) et Slint met le repère à l'échelle **uniformément** : un pas de
    // `d` en `x` vaut donc exactement le même nombre de pixels qu'un pas de `d` en `y`. Le rayon de
    // saisie est un vrai rayon, sur un vrai disque.
    //
    // Une implémentation qui mesurerait la distance en fractions de largeur d'un côté et en
    // fractions de hauteur de l'autre produirait une zone de saisie **quatre fois plus haute que
    // large** — attrapant un point situé sous la souris, et manquant celui qui est juste à côté.
    // Aucun test des seuls clics exacts ne le verrait.
    let vise = PALIERS_PAR_DEFAUT[1];
    let (x, y) = point(vise);

    for (nom, dx, dy) in [
        ("à droite", 0.9 * RAYON_DE_SAISIE, 0.0),
        ("à gauche", -0.9 * RAYON_DE_SAISIE, 0.0),
        ("au-dessus", 0.0, -0.9 * RAYON_DE_SAISIE),
        ("en dessous", 0.0, 0.9 * RAYON_DE_SAISIE),
        ("en diagonale", 0.7 * RAYON_DE_SAISIE, 0.7 * RAYON_DE_SAISIE),
    ] {
        assert_eq!(
            palier_saisi(&PALIERS_PAR_DEFAUT, x + dx, y + dy),
            Some(1),
            "un clic {nom} du point, à moins d'un rayon ({dx}, {dy}), doit l'attraper"
        );
    }

    for (nom, dx, dy) in [
        ("trop à droite", 1.1 * RAYON_DE_SAISIE, 0.0),
        ("trop à gauche", -1.1 * RAYON_DE_SAISIE, 0.0),
        ("trop au-dessus", 0.0, -1.1 * RAYON_DE_SAISIE),
        ("trop en dessous", 0.0, 1.1 * RAYON_DE_SAISIE),
        (
            "trop loin en diagonale",
            0.8 * RAYON_DE_SAISIE,
            0.8 * RAYON_DE_SAISIE,
        ),
    ] {
        assert_eq!(
            palier_saisi(&PALIERS_PAR_DEFAUT, x + dx, y + dy),
            None,
            "un clic {nom} du point, à plus d'un rayon ({dx}, {dy}), ne doit pas l'attraper"
        );
    }
}

#[test]
fn entre_deux_points_a_portee_c_est_le_plus_proche_qui_est_attrape() {
    // Huit paliers dans un cadre de ~360 px se serrent : deux zones de saisie peuvent se recouvrir
    // dès qu'on rapproche deux paliers à la main. Attraper « le premier trouvé » ferait dépendre le
    // geste de l'ordre du parcours — c'est-à-dire d'un détail d'implémentation —, et un clic posé
    // sur le point de droite traînerait celui de gauche.
    let serres = [(degres(40), 50u8), (degres(41), 55)];
    courbe(&serres);

    let (gauche_x, gauche_y) = point(serres[0]);
    let (droite_x, droite_y) = point(serres[1]);
    assert!(
        ((droite_x - gauche_x).powi(2) + (droite_y - gauche_y).powi(2)).sqrt()
            < 2.0 * RAYON_DE_SAISIE,
        "ce banc n'a de sens que si les deux zones de saisie se recouvrent : rapproche les deux \
         paliers, ou le test ne vérifie rien"
    );

    assert_eq!(
        palier_saisi(&serres, gauche_x, gauche_y),
        Some(0),
        "un clic exactement sur le palier de gauche doit l'attraper, lui et pas son voisin"
    );
    assert_eq!(
        palier_saisi(&serres, droite_x, droite_y),
        Some(1),
        "un clic exactement sur le palier de droite doit l'attraper, lui et pas son voisin"
    );

    // Et juste après le milieu, c'est celui de droite — la bascule tombe où la distance bascule.
    let milieu_x = (gauche_x + droite_x) / 2.0;
    let milieu_y = (gauche_y + droite_y) / 2.0;
    assert_eq!(
        palier_saisi(&serres, milieu_x + EPSILON, milieu_y),
        Some(1),
        "passé le milieu, c'est le palier de droite le plus proche"
    );
}

// ---------------------------------------------------------------------------
// 4 — déplacer un point
// ---------------------------------------------------------------------------

#[test]
fn trainer_un_point_change_sa_temperature_et_sa_consigne() {
    // Critère d'acceptation du commentaire de décisions : « Un clic gauche sur un point le saisit ;
    // le traîner change sa température **et** sa consigne. »
    //
    // Les deux, parce que c'est tout l'intérêt de #122 : #113 séparait le geste de son effet, une
    // barre pour la température et une autre pour la consigne. Un point qui ne changerait qu'une
    // des deux grandeurs ne serait qu'une troisième barre.
    let vise = (degres(43), 75u8);
    let (x, y) = point(vise);
    let rendus = deplace(&PALIERS_PAR_DEFAUT, 1, x, y);

    assert_eq!(
        rendus.len(),
        PALIERS_PAR_DEFAUT.len(),
        "traîner un palier n'en ajoute ni n'en retire aucun"
    );
    assert!(
        (rendus[1].0 - vise.0).abs() <= TOLERANCE_MILLI,
        "le palier traîné doit valoir {} m°C, il en porte {}",
        vise.0,
        rendus[1].0
    );
    assert_eq!(
        rendus[1].1, vise.1,
        "le palier traîné doit porter la consigne du clic, en pourcent"
    );
    assert_ne!(
        rendus[1].0, PALIERS_PAR_DEFAUT[1].0,
        "la température doit avoir changé"
    );
    assert_ne!(
        rendus[1].1, PALIERS_PAR_DEFAUT[1].1,
        "la consigne doit avoir changé"
    );

    // Et les voisins ne bougent pas d'un millidegré : traîner un point n'est pas retracer la courbe.
    assert_eq!(
        rendus[0], PALIERS_PAR_DEFAUT[0],
        "le voisin de gauche bouge"
    );
    assert_eq!(
        rendus[2], PALIERS_PAR_DEFAUT[2],
        "le voisin de droite bouge"
    );

    // Le rang traîné est bien celui qu'un clic sur ce point aurait attrapé — sinon les deux moitiés
    // du geste ne parlent pas du même palier.
    let (saisie_x, saisie_y) = point(PALIERS_PAR_DEFAUT[1]);
    assert_eq!(
        palier_saisi(&PALIERS_PAR_DEFAUT, saisie_x, saisie_y),
        Some(1),
        "le clic qui attrape et le rang qu'on traîne doivent désigner le même palier"
    );

    // La courbe rendue est valide, et c'est `Courbe::depuis` qui le dit.
    courbe(&rendus);
}

#[test]
fn un_point_traine_en_travers_d_un_voisin_est_refuse_jamais_reordonne() {
    // ⚠️ L'exigence de l'issue, mot pour mot : « Un point traîné en travers d'un voisin doit être
    // refusé **en le disant**, pas réordonné en silence — *les réordonner serait deviner ce qui a
    // été tapé*. »
    //
    // C'est la règle que #99 s'est donnée pour `regule courbe` et que le projet applique à
    // `eclairage.conf` : compléter au jugé est refusé partout. Trier ici serait pire qu'ailleurs —
    // le palier qu'on tient changerait de rang sous la souris, et le geste suivant traînerait un
    // autre point.
    for (nom, rang, cible) in [
        ("le premier au-delà du dernier", 0, degres(60)),
        ("le premier au-delà du deuxième", 0, degres(47)),
        ("le dernier en deçà du premier", 2, degres(30)),
        ("celui du milieu en deçà du premier", 1, degres(32)),
        ("celui du milieu au-delà du dernier", 1, degres(55)),
    ] {
        let (x, y) = (abscisse(cible), ordonnee(50));
        let erreur = refus_de_deplacement(&PALIERS_PAR_DEFAUT, rang, x, y);
        assert!(
            !erreur.raison.trim().is_empty(),
            "traîner {nom} doit être refusé **avec une raison**, pas avec un silence"
        );

        // Et la raison est celle du démon, pas une seconde formulation : `Courbe::depuis` juge la
        // courbe telle que le geste la produit — le palier remplacé **à sa place**, sans tri.
        let mut attendus = PALIERS_PAR_DEFAUT.to_vec();
        attendus[rang] = palier_dans_le_trace(x, y);
        let du_demon = Courbe::depuis(&attendus).expect_err(
            "ce banc n'a de sens que si le démon refuse aussi cette courbe : corrige la cible",
        );
        assert_eq!(
            erreur.raison, du_demon.raison,
            "traîner {nom} : deux raisons pour le même défaut, c'est deux messages à maintenir \
             dont un seul serait relu"
        );
    }
}

#[test]
fn un_deplacement_refuse_ne_trie_rien_derriere_le_dos_de_l_utilisateur() {
    // Le refus ci-dessus se vérifie aussi par la négative, et c'est la formulation qui compte : il
    // ne doit exister **aucune** entrée pour laquelle l'éditeur rendrait la courbe triée. Une
    // implémentation qui trierait passerait tous les tests de validité — la courbe rendue serait
    // parfaitement acceptable — et le palier changerait de rang sous la souris.
    let croise = (degres(60), 90u8);
    let (x, y) = point(croise);

    match palier_deplace(&PALIERS_PAR_DEFAUT, 0, x, y) {
        Err(_) => {}
        Ok(rendus) => {
            let mut trie = rendus.clone();
            trie.sort_by_key(|(milli, _)| *milli);
            panic!(
                "l'éditeur a rendu {rendus:?} au lieu de refuser. Trié, ce serait {trie:?} — c'est \
                 exactement ce que l'issue interdit : « les réordonner serait deviner ce qui a été \
                 tapé »"
            );
        }
    }

    // Le même geste sur un palier qui **ne** croise personne reste accepté et garde l'ordre de
    // saisie : le refus porte sur le croisement, pas sur le déplacement.
    let rendus = deplace(&PALIERS_PAR_DEFAUT, 2, x, y);
    assert_eq!(
        rendus,
        vec![
            PALIERS_PAR_DEFAUT[0],
            PALIERS_PAR_DEFAUT[1],
            (croise.0, croise.1),
        ],
        "le dernier palier peut monter à {croise:?} sans croiser personne"
    );
}

#[test]
fn un_deplacement_hors_du_cadre_bute_aux_bornes_sans_perdre_le_palier() {
    // Composition des deux règles : le geste est ramené aux bornes du cadre (§1), puis la courbe
    // est jugée par `Courbe::depuis` (§4). Traîner le premier palier tout à gauche doit donc le
    // poser à `TRACE_FROID`, et non le perdre hors du cadre ni faire refuser la courbe.
    let rendus = deplace(&PALIERS_PAR_DEFAUT, 0, -5.0, -5.0);
    assert_eq!(
        rendus[0],
        (TRACE_FROID, 100),
        "traîné hors du cadre en haut à gauche, le palier bute sur le coin le plus froid et le \
         plus rapide"
    );
    courbe(&rendus);

    let rendus = deplace(&PALIERS_PAR_DEFAUT, 2, TRACE_ASPECT + 5.0, 5.0);
    assert_eq!(
        rendus[2],
        (TRACE_CHAUD, 0),
        "traîné hors du cadre en bas à droite, le palier bute sur le coin le plus chaud et à \
         l'arrêt"
    );
    courbe(&rendus);
}

#[test]
fn trainer_un_rang_qui_n_existe_pas_est_refuse_en_le_disant() {
    // La fenêtre tient un rang saisi entre deux tours de boucle ; un retrait par le socket, une
    // courbe rechargée, et ce rang désigne un palier disparu. Rendre la courbe inchangée « à défaut
    // de mieux » ferait croire à un geste sans effet ; rogner le rang traînerait un autre palier.
    for rang in [
        PALIERS_PAR_DEFAUT.len(),
        PALIERS_PAR_DEFAUT.len() + 7,
        usize::MAX,
    ] {
        let erreur = refus_de_deplacement(&PALIERS_PAR_DEFAUT, rang, TRACE_ASPECT / 2.0, 0.5);
        assert!(
            !erreur.raison.trim().is_empty(),
            "traîner le rang {rang}, qui n'existe pas, doit être refusé avec une raison"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — retirer un point, et le plancher d'UN palier
// ---------------------------------------------------------------------------

#[test]
fn un_clic_droit_retire_le_point_vise_et_lui_seul() {
    // Critère d'acceptation du commentaire de décisions : « Un clic droit sur un point le retire ».
    //
    // La recommandation était un bouton séparé, pour qu'aucun geste destructeur ne parte sans
    // confirmation sur le réglage qui décide de la température du poste ; l'arbitrage a été rendu
    // en connaissance de cause, « c'est la convention des éditeurs de courbe, et elle vaut le
    // risque ». Ce fichier l'exécute sans y revenir.
    for rang in 0..PALIERS_PAR_DEFAUT.len() {
        let rendus = retire(&PALIERS_PAR_DEFAUT, rang);
        let attendus: Vec<(i32, u8)> = PALIERS_PAR_DEFAUT
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != rang)
            .map(|(_, palier)| *palier)
            .collect();
        assert_eq!(
            rendus, attendus,
            "retirer le rang {rang} doit laisser exactement les autres, dans leur ordre"
        );
        courbe(&rendus);
    }

    // Depuis le plafond, aussi : huit paliers moins un en font sept, et la courbe reste valide.
    let rendus = retire(&PALIERS_AU_PLAFOND, 4);
    assert_eq!(rendus.len(), PALIERS_MAX - 1);
    assert!(
        !rendus.contains(&PALIERS_AU_PLAFOND[4]),
        "le palier visé doit avoir disparu : {rendus:?}"
    );
}

#[test]
fn retirer_l_avant_dernier_palier_est_permis_et_rend_une_courbe_plate() {
    // ⚠️ **Ce test contredit exprès le premier commentaire de décisions de l'issue**, et c'est le
    // second qui fait foi. Le premier écrivait : « Un clic droit sur un point le retire, **sauf**
    // s'il n'en reste que deux — auquel cas rien ne bouge et la fenêtre le dit », en le justifiant
    // par « `Courbe::depuis` refusant une courbe qui n'en a pas au moins deux ».
    //
    // **La phrase est fausse** — `Courbe::depuis` refuse une courbe vide, une consigne au-dessus de
    // cent, deux paliers à la même température et une température décroissante, jamais un palier
    // unique —, et le chiffre « deux » ne reposait sur rien d'autre. Une **consigne fixe est une
    // courbe parfaitement sensée** : la même à toute température, exactement ce que
    // `regule courbe 45000:80` pose par le socket.
    //
    // Garder deux ferait de la fenêtre le seul chemin incapable d'exprimer une courbe que le socket
    // accepte. Retirer l'avant-dernier palier est donc **permis**.
    let deux = [(degres(35), 30u8), (degres(50), 100)];
    courbe(&deux);

    for rang in 0..deux.len() {
        let rendus = retire(&deux, rang);
        assert_eq!(
            rendus.len(),
            1,
            "retirer le rang {rang} d'une courbe à deux paliers doit en laisser un : {rendus:?}"
        );
        assert_eq!(
            rendus[0],
            deux[1 - rang],
            "c'est l'autre palier qui reste, pas un palier inventé"
        );

        // Et ce qui reste est une courbe, plate, que le démon accepte — la démonstration que le
        // plancher de deux n'avait aucun fondement.
        let plate = courbe(&rendus);
        for lu in [TRACE_FROID, degres(40), TRACE_CHAUD] {
            assert_eq!(
                plate.consigne(lu),
                rendus[0].1,
                "une courbe à un palier est plate : à {lu} m°C elle doit rendre {} %",
                rendus[0].1
            );
        }
        assert!(
            ordre_de_courbe(&rendus).is_ok(),
            "et elle doit pouvoir partir sur le socket, comme `regule courbe 45000:80` (#113)"
        );
    }
}

#[test]
fn le_clic_droit_sur_le_dernier_point_est_inerte_et_le_dit() {
    // Critère d'acceptation du second commentaire de décisions : « Un clic droit retire un palier
    // tant qu'il en reste **au moins un** ; sur le dernier, rien ne bouge et la fenêtre le dit. »
    //
    // C'est le plancher du **protocole**, et le seul : `Courbe::depuis` refuse une courbe vide —
    // « une table sans palier n'a aucune consigne à rendre, et rendre 0 % serait arrêter les
    // ventilateurs sur une table que personne n'a écrite » (#113). L'éditeur ne doit donc pas
    // pouvoir s'y mettre, et surtout pas en silence : un éditeur vide n'a plus aucune poignée à
    // saisir, donc plus aucun moyen d'en ressortir sans savoir qu'il faut d'abord ajouter un point.
    //
    // ⚠️ **Le juge est l'éditeur, pas `Courbe::depuis`.** Le refus doit tomber avant la construction
    // — voir le test suivant, qui protège cette dernière contre la sévérité qu'on serait tenté d'y
    // ajouter.
    let un_seul = [(degres(45), 80u8)];
    courbe(&un_seul);

    let erreur = refus_de_retrait(&un_seul, 0);
    assert!(
        !erreur.raison.trim().is_empty(),
        "retirer le dernier palier doit être refusé **avec une raison** : « rien ne bouge et la \
         fenêtre le dit »"
    );
    // ⚠️ La raison doit **nommer** le plancher, pas se contenter d'exister : « erreur » tout court
    // laisse l'utilisateur devant un clic sans effet. Trois formulations passent — c'est le fait
    // qui est exigé, pas sa mise en forme.
    assert!(
        ["dernier", "un palier", "un seul", "au moins un"]
            .iter()
            .any(|dit| erreur.raison.contains(dit)),
        "le refus doit dire que c'est le **dernier** palier, pour qu'on sache pourquoi le clic n'a \
         rien fait. Raison obtenue : {}",
        erreur.raison
    );

    // Un éditeur déjà vide n'a pas de rang 0 à retirer : c'est un refus, jamais une panique.
    refus_de_retrait(&[], 0);
}

#[test]
fn le_juge_du_plancher_est_l_editeur_car_une_courbe_a_un_palier_reste_valide() {
    // ⚠️ **Ce test protège `Courbe::depuis` contre la « correction » qu'on serait tenté d'y faire
    // en relisant le premier commentaire de l'issue.**
    //
    // Ce commentaire-là attribuait le plancher à `Courbe::depuis`, « refusant une courbe qui n'en a
    // pas au moins deux ». **C'est faux** — un commentaire ultérieur de l'issue le corrige —, et
    // deux fichiers de tests d'intention figent l'inverse : `une_courbe_a_un_seul_palier_est_plate`
    // (#99, côté démon) et `une_courbe_a_un_seul_palier_reste_acceptee` (#113, côté fenêtre). Une
    // consigne fixe est une courbe parfaitement sensée, et `regule courbe 45000:80` doit continuer
    // de passer par le socket.
    //
    // Le plancher — désormais **un** palier, celui du protocole — est donc porté par `palier_retire`
    // et par lui seul. Rendre `Courbe::depuis` plus sévère casserait les deux fichiers ci-dessus et
    // ferait diverger la fenêtre du démon : exactement ce que #113 a passé un fichier entier à
    // empêcher.
    let un_seul = [(degres(45), 80u8)];
    let plate = Courbe::depuis(&un_seul).unwrap_or_else(|erreur| {
        panic!(
            "`Courbe::depuis` doit continuer d'accepter une courbe à un seul palier : c'est une \
             consigne fixe, figée par #99 et #113. Refus obtenu : {}",
            erreur.raison
        )
    });
    assert_eq!(
        plate.consigne(TRACE_FROID),
        plate.consigne(TRACE_CHAUD),
        "et elle est plate : la même consigne d'un bout à l'autre de la plage"
    );
    assert!(
        ordre_de_courbe(&un_seul).is_ok(),
        "et la fenêtre doit continuer de la laisser partir sur le socket (#113)"
    );

    // La courbe **vide**, elle, reste refusée par le démon — c'est le seul plancher réel, et c'est
    // celui que `palier_retire` reprend à son compte.
    assert!(
        Courbe::depuis(&[]).is_err(),
        "`Courbe::depuis` refuse une courbe vide (#99, #113) : c'est le plancher que l'éditeur \
         applique, et le seul"
    );
}

#[test]
fn retirer_un_rang_qui_n_existe_pas_est_refuse_en_le_disant() {
    // Même raison qu'au déplacement : un rang tenu par la fenêtre peut désigner un palier disparu.
    // Retirer « le dernier à défaut » supprimerait un palier que personne n'a visé.
    for rang in [
        PALIERS_PAR_DEFAUT.len(),
        PALIERS_PAR_DEFAUT.len() + 3,
        usize::MAX,
    ] {
        let erreur = refus_de_retrait(&PALIERS_PAR_DEFAUT, rang);
        assert!(
            !erreur.raison.trim().is_empty(),
            "retirer le rang {rang}, qui n'existe pas, doit être refusé avec une raison"
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — ajouter un point
// ---------------------------------------------------------------------------

#[test]
fn un_clic_sur_le_trace_hors_d_un_point_en_ajoute_un_a_sa_place() {
    // Critère d'acceptation du commentaire de décisions : « Un clic sur le tracé, hors d'un point,
    // en ajoute un ».
    //
    // ⚠️ **Le palier ajouté est celui du clic**, `palier_dans_le_trace(x, y)` et rien d'autre : une
    // seule conversion pour les deux gestes, pas une seconde règle à tenir d'accord avec la
    // première.
    //
    // ⚠️ **Il est inséré à son rang**, celui où sa température le place. Ce n'est pas le tri que
    // l'issue interdit : celui-là réordonne des paliers que l'utilisateur a posés, celui-ci donne
    // son rang à un palier qui n'en avait pas. Insérer au bout ferait refuser tout ajout qui ne
    // serait pas le plus chaud — c'est-à-dire presque tous.
    for (nom, ajout, rang_attendu) in [
        ("entre le premier et le deuxième", (degres(40), 45u8), 1),
        ("entre le deuxième et le troisième", (degres(47), 80), 2),
        ("avant le premier", (degres(25), 10), 0),
        ("après le dernier", (degres(65), 100), 3),
    ] {
        let (x, y) = point(ajout);
        let rendus = ajoute(&PALIERS_PAR_DEFAUT, x, y);

        assert_eq!(
            rendus.len(),
            PALIERS_PAR_DEFAUT.len() + 1,
            "un clic {nom} doit ajouter exactement un palier"
        );
        assert!(
            (rendus[rang_attendu].0 - ajout.0).abs() <= TOLERANCE_MILLI
                && rendus[rang_attendu].1 == ajout.1,
            "le palier ajouté {nom} doit valoir {ajout:?} au rang {rang_attendu}, il vaut {:?} — \
             courbe rendue : {rendus:?}",
            rendus[rang_attendu]
        );

        // Les anciens sont tous là, dans leur ordre.
        let anciens: Vec<(i32, u8)> = rendus
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != rang_attendu)
            .map(|(_, palier)| *palier)
            .collect();
        assert_eq!(
            anciens,
            PALIERS_PAR_DEFAUT.to_vec(),
            "un clic {nom} ne doit toucher à aucun palier existant"
        );

        courbe(&rendus);
    }
}

#[test]
fn ajouter_un_point_sur_le_trace_ne_change_pas_ce_que_le_boitier_appliquera() {
    // Ce n'est pas un critère écrit, c'est ce qui rend le geste sûr : on ajoute un point pour
    // **pouvoir** régler, pas pour régler. Un ajout qui déplacerait la courbe changerait la
    // régulation du poste d'un clic, sans qu'on l'ait demandé.
    //
    // Le point cliqué est pris **sur le tracé**, donc à la consigne que `Courbe::consigne` rend à
    // cette température — la fonction que le démon exécute (#113). La courbe augmentée doit alors
    // rendre la même consigne partout.
    //
    // ⚠️ **À un point de consigne près**, et c'est une conséquence assumée : la consigne est un
    // entier de 0 à 100, donc le palier ajouté est sur la courbe à son propre arrondi près. Un
    // point de duty sur 255, la même contrepartie que l'hystérésis de #111.
    let avant = Courbe::defaut();

    for milli in [degres(38), degres(40), 42_500, degres(47), 49_900] {
        let sur_le_trace = (milli, avant.consigne(milli));
        let (x, y) = point(sur_le_trace);
        let rendus = ajoute(&PALIERS_PAR_DEFAUT, x, y);
        let apres = courbe(&rendus);

        // ⚠️ « Ne rien changer » est aussi ce que fait un geste qui n'ajoute rien : la propriété ne
        // vaut qu'accompagnée de la preuve que le palier est bien là.
        assert_eq!(
            rendus.len(),
            PALIERS_PAR_DEFAUT.len() + 1,
            "le palier de {sur_le_trace:?} doit avoir été ajouté : {rendus:?}"
        );
        assert!(
            rendus
                .iter()
                .any(|(m, p)| (m - sur_le_trace.0).abs() <= TOLERANCE_MILLI && *p == sur_le_trace.1),
            "le palier ajouté doit être {sur_le_trace:?} : {rendus:?}"
        );

        for lu in (TRACE_FROID..=TRACE_CHAUD).step_by(500) {
            let (a, b) = (avant.consigne(lu), apres.consigne(lu));
            assert!(
                a.abs_diff(b) <= 1,
                "ajouter un point à {milli} m°C a changé la régulation : à {lu} m°C, la courbe \
                 rendait {a} % et rend maintenant {b} %"
            );
        }
    }
}

#[test]
fn au_dela_de_huit_paliers_l_ajout_est_refuse_en_le_disant() {
    // Critère d'acceptation du commentaire de décisions : « Un clic sur le tracé, hors d'un point,
    // en ajoute un — **refusé au-delà de huit**, en le disant. »
    //
    // La borne est celle de l'issue : « un tracé à trente points serait illisible autant
    // qu'inutile », et « huit couvre toute forme utile entre 20 et 70 °C, et chaque point reste
    // saisissable à la souris dans un cadre de ~360 px ».
    let (x, y) = point((degres(28), 25));
    let erreur = refus_d_ajout(&PALIERS_AU_PLAFOND, x, y);

    assert!(
        !erreur.raison.trim().is_empty(),
        "un ajout au-delà du plafond doit être refusé **avec une raison**, pas avec un silence"
    );
    assert!(
        erreur.raison.contains("huit") || erreur.raison.contains(&PALIERS_MAX.to_string()),
        "le refus doit nommer le plafond ({PALIERS_MAX}), pour qu'on sache pourquoi le clic n'a \
         rien fait. Raison obtenue : {}",
        erreur.raison
    );

    // Le dernier ajout possible, lui, passe : le plafond est un plafond, pas un mur posé un cran
    // trop bas. Sept paliers doivent pouvoir en accueillir un huitième.
    let sept = &PALIERS_AU_PLAFOND[..PALIERS_MAX - 1];
    let rendus = ajoute(sept, x, y);
    assert_eq!(
        rendus.len(),
        PALIERS_MAX,
        "sept paliers doivent pouvoir en accueillir un huitième : le plafond est atteint, pas \
         dépassé"
    );
    courbe(&rendus);

    // Et une fois au plafond, retirer un point rouvre la porte — les deux gestes se composent, et
    // le plafond n'est pas un aller simple.
    let allege = retire(&rendus, 0);
    let (libre_x, libre_y) = point((degres(26), 22));
    assert_eq!(
        ajoute(&allege, libre_x, libre_y).len(),
        PALIERS_MAX,
        "retirer un palier doit rendre la place d'un autre"
    );
}

#[test]
fn ajouter_un_point_sur_une_temperature_deja_prise_est_refuse_par_le_juge_du_demon() {
    // Deux consignes à la même température se contredisent — laquelle appliquer ? — et #99 les
    // refuse **en nommant** la température fautive. Ce geste n'a aucune raison d'y échapper : c'est
    // le même juge, et le même message.
    //
    // Le cas est réel : la zone de saisie a un rayon fini, et un clic qui la manque de peu tombe
    // très près d'un palier existant.
    let (x, y) = point((degres(45), 20));
    let erreur = refus_d_ajout(&PALIERS_PAR_DEFAUT, x, y);
    assert!(
        erreur.raison.contains("45"),
        "le refus doit nommer la température répétée (45 °C), comme #113 l'exige déjà de la \
         fenêtre. Raison obtenue : {}",
        erreur.raison
    );
}

// ---------------------------------------------------------------------------
// 7 — le juge reste `Courbe::depuis`, pour les trois gestes
// ---------------------------------------------------------------------------

#[test]
fn aucun_geste_ne_rend_une_courbe_que_le_demon_refuserait() {
    // ⚠️ **C'est la règle qui compte, et elle vaut mieux que la liste des cas ci-dessus** : l'éditeur
    // ne décide de rien, il **réutilise** le juge du démon — « Le juge reste `Courbe::depuis`, celui
    // du démon (#113) », dit l'issue en tête de ce qui ne doit pas se perdre en chemin.
    //
    // Une courbe rendue `Ok` par un geste et refusée par `Courbe::depuis` partirait sur le socket
    // pour y être rejetée : le refus arriverait **après** le geste, dans un journal, au lieu
    // d'arriver sous la souris. C'est exactement ce que #113 a écrit son fichier pour empêcher.
    //
    // Le balayage est grossier exprès : il ne cherche pas un cas particulier, il en essaie
    // beaucoup. Chaque `Ok` est soumis au juge, et chaque `Err` doit porter une raison.
    let bancs: [&[(i32, u8)]; 4] = [
        &PALIERS_PAR_DEFAUT,
        &PALIERS_AU_PLAFOND,
        &[(degres(35), 30), (degres(50), 100)],
        &[(degres(30), 0), (degres(40), 40), (degres(70), 100)],
    ];

    for paliers in bancs {
        for pas_x in 0..=8 {
            for pas_y in 0..=4 {
                let x = pas_x as f32 / 8.0 * TRACE_ASPECT;
                let y = pas_y as f32 / 4.0;

                for rang in 0..paliers.len() {
                    le_demon_juge(
                        &format!("traîner le rang {rang} de {paliers:?} en ({x}, {y})"),
                        palier_deplace(paliers, rang, x, y),
                    );
                }

                let ajoutes = le_demon_juge(
                    &format!("ajouter en ({x}, {y}) à {paliers:?}"),
                    palier_ajoute(paliers, x, y),
                );
                if let Some(rendus) = ajoutes {
                    assert!(
                        rendus.len() <= PALIERS_MAX,
                        "ajouter en ({x}, {y}) à {paliers:?} rend {} paliers, au-delà du plafond \
                         de {PALIERS_MAX}",
                        rendus.len()
                    );
                }
            }
        }

        for rang in 0..paliers.len() {
            let restants = le_demon_juge(
                &format!("retirer le rang {rang} de {paliers:?}"),
                palier_retire(paliers, rang),
            );
            if let Some(rendus) = restants {
                assert!(
                    !rendus.is_empty(),
                    "retirer le rang {rang} de {paliers:?} vide l'éditeur : le plancher est **un** \
                     palier, celui du protocole — `Courbe::depuis` refuse une courbe vide, et un \
                     éditeur sans poignée n'a plus aucun moyen d'en ressortir"
                );
            }
        }
    }
}

#[test]
fn une_courbe_editee_par_gestes_part_telle_quelle_sur_le_socket() {
    // Le bout du chemin : ce que les gestes composent doit pouvoir partir par `ordre_de_courbe`,
    // la porte de #113. Sans ce test, l'éditeur pourrait produire des courbes irréprochables que
    // rien n'enverrait — et la boucle ne se fermerait pas.
    let ajoute_puis_traine = {
        let etape = ajoute(&PALIERS_PAR_DEFAUT, abscisse(degres(40)), ordonnee(45));
        deplace(&etape, 1, abscisse(degres(41)), ordonnee(50))
    };
    let puis_retire = retire(&ajoute_puis_traine, 0);

    for paliers in [&ajoute_puis_traine, &puis_retire] {
        ordre_de_courbe(paliers).unwrap_or_else(|erreur| {
            panic!(
                "une courbe composée aux gestes doit pouvoir partir sur le socket : {paliers:?}\n  \
                 Refus : {}",
                erreur.raison
            )
        });
    }

    assert_eq!(
        puis_retire.len(),
        ajoute_puis_traine.len() - 1,
        "les trois gestes se composent : un ajout, un déplacement, un retrait"
    );
}
