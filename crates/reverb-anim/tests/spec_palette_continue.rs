//! Tests d'intention du parcours continu de palette (issue #138).
//!
//! Écrits **depuis l'issue #138 seule**, avant l'implémentation. Aucun corps de fonction de
//! `crates/reverb-anim/src/` n'a été relu pour les écrire — ni `Animation::peindre`, ni
//! `Palette::echantillon`, que l'issue nomme toutes les deux. N'ont servi que les **signatures
//! publiques** relevées au `grep`, les tests d'intention déjà présents dans ce répertoire pour les
//! idiomes d'appel du catalogue, `README.md`, et des **mesures faites sur le rendu courant** —
//! c'est-à-dire sur ce que le boîtier reçoit aujourd'hui, pas sur le code qui le produit.
//!
//! # Ce que l'issue demande, et ce que ce fichier vérifie
//!
//! `fraction` enroule : l'index de palette repasse de 255 à 0 une fois par cycle, et la LED saute
//! de l'écart entre les deux bouts du dégradé. La correction demandée est un parcours **en
//! aller-retour** : dans un sens sur la première moitié du cycle, dans l'autre sur la seconde.
//!
//! | ce que l'issue liste | le test qui le porte | état attendu avant correction |
//! |---|---|---|
//! | sans palette, le catalogue rend les images d'avant | [`sans_palette_le_catalogue_rend_les_images_figees`] | **vert** |
//! | les cinq familles vraiment invariantes rendent les images d'avant | [`sous_palette_une_famille_a_index_borne_rend_les_images_figees`] | **vert** |
//! | aucune donnée de palette n'est modifiée | [`les_douze_palettes_portent_les_arrets_figes`] | **vert** |
//! | aucune LED ne saute de plus de 150 sur `vague` et `respiration` | [`sur_vague_et_respiration_aucune_led_ne_saute_de_plus_de_150`] | **rouge** |
//! | à plein niveau, une bougie rend la **fin** du dégradé | [`a_plein_niveau_les_trois_familles_de_127_rendent_la_fin_du_degrade`] | **rouge** |
//! | le parcours atteint les deux arrêts extrêmes | [`le_parcours_atteint_les_deux_arrets_extremes`] | **vert** |
//! | le parcours est symétrique | [`le_parcours_est_symetrique_chaque_teinte_est_vue_deux_fois`] | **rouge** |
//!
//! # ⚠️ Ce que le critère n° 2 disait, ce qu'il dit maintenant, et ce qui a été retiré
//!
//! La première rédaction de ce fichier figeait `bougie`, `nuee` et `artifice` avec les cinq autres
//! familles à index borné : le critère demandait alors qu'elles soient « inchangées, à l'octet
//! près ». Trois empreintes complètes y figeaient **l'éclair noir** — `bougie` LED n° 0 au pas 0,
//! `nuee` LED n° 71 au pas 6, `artifice` LED n° 15 au pas 54 —, c'est-à-dire les points où l'index
//! vaut exactement 1,0 et où `fraction(1.0) == 0.0` renvoie le **premier** arrêt de la palette :
//! sous `lava`, du noir là où le motif veut du blanc.
//!
//! **Ces trois empreintes ont été retirées, et la présente rédaction fige l'inverse.** Le critère
//! n° 2 a été réécrit dans l'issue : cinq familles restent inchangées à l'octet près, trois
//! changent — et changent *uniquement* là. Mesuré sur les douze palettes : le pas et la LED où
//! l'intensité est maximale rendent aujourd'hui le premier arrêt sur **un** échantillon sur 14 880
//! pour `bougie`, **1 271** pour `nuee`, **1 163** pour `artifice`. C'est le même défaut que la
//! couture, pris par l'autre bout : un scalaire borné projeté sur un chemin qui ne l'est pas.
//!
//! Sans cette trace, la relecture verrait une garde disparaître sans savoir ce qu'elle gardait.
//!
//! # Quatre arbitrages que l'issue laisse ouverts, et qui sont tranchés ici
//!
//! 1. ⚠️ **Un « saut » se mesure sur la plus grande des trois composantes**, entre deux images
//!    successives et sur **la même LED**. L'issue donne des sauts de 254 et 255 pour des palettes
//!    dont l'écart entre bouts vaut 255 : c'est la seule lecture qui rende ses chiffres. Mesuré
//!    ici sur le rendu courant, vitesse 1, les huit directions, un cycle entier : `vague` saute de
//!    **7** sans palette et de **255** avec `lava`, `respiration` de **6** et **255**. Les deux
//!    populations ne se touchent pas, et le seuil de l'issue tombe entre les deux.
//!
//! 2. ⚠️ **La symétrie du parcours se lit comme un *doublement des teintes*.** « La teinte à `t` et
//!    celle à `1 - t` sont la même » n'est pas observable directement — le parcours vit dans une
//!    fermeture privée de `peindre`, et `t` n'est jamais rendu. Ce qui *est* observable, c'est sa
//!    conséquence : une LED fixe, sur un cycle entier, voit **deux fois** chaque teinte du chemin —
//!    une à l'aller, une au retour — là où un parcours qui enroule ne la voit qu'une seule fois.
//!    C'est aussi ce qui distingue l'aller-retour de la solution que l'issue met hors scope,
//!    « boucler les palettes elles-mêmes » : une palette bouclée serait continue **sans** doubler
//!    quoi que ce soit.
//!
//! 3. ⚠️ **`vague` ne peut pas porter le test des arrêts extrêmes**, et c'est une mesure, pas un
//!    confort. Son enveloppe d'intensité s'annule **complètement** une fois par cycle — relevé LED
//!    par LED sur le rendu courant : `254, 254, …, 2, 1, 0, 0, 0, 0, 0, 1, 2, …`. Une extrémité du
//!    parcours qui tomberait dans ce creux serait rendue **noire**, et une LED noire ne porte
//!    aucune teinte. Le critère est donc vérifié sur `respiration`, dont l'enveloppe plancher à
//!    38/255, et sur `rotation`, dont l'arête de luminosité laisse la mi-cycle à ~125/255.
//!
//! 4. ⚠️ **Les palettes témoins sont *choisies par mesure*, jamais nommées en dur.** Les douze
//!    n'ont pas toutes de quoi porter chaque critère : cinq ont leurs deux bouts trop sombres ou
//!    trop semblables pour qu'on y lise une teinte, et onze repassent par des teintes déjà vues, ce
//!    qui rendrait le doublement indécidable. Les deux tests concernés retiennent donc les palettes
//!    sur une propriété de leurs **arrêts** — donnée publique — et refusent d'aboutir si la
//!    sélection est vide.
//!
//! # Ce que ce fichier ne vérifie pas
//!
//! ⚠️ `rotation` ne figure pas au test des sauts, et c'est l'issue qui l'exclut : « l'arête de
//! luminosité de `rotation` (`1 - fraction(angle - temps)`), qui est voulue et reste » est hors
//! scope. Mesuré : elle saute déjà de **253** sans aucune palette. Son parcours de palette est donc
//! jugé par les deux tests de teinte, qui ignorent l'intensité par construction.
//!
//! ⚠️ La raideur interne de `paysage` et de `nuit-avril` est hors scope — l'issue annonce d'avance
//! que `nuit-avril` empire (92 → 138), et rien ici ne prétend le contraire. Le seuil de 150 tient
//! compte de ce résidu, qui est une propriété des données reprises de WLED.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi.

use reverb_anim::{Animation, CATALOGUE, Direction, Geometrie, Image, Palette, Reglages};
use reverb_proto::ram::LEDS_PER_STICK;
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// Le domaine
// ---------------------------------------------------------------------------

/// Les douze palettes, dans l'ordre du tableau de l'issue #126, écrites en dur.
///
/// L'ordre compte : les empreintes figées plus bas balaient les palettes dans cet ordre-là.
const DOUZE: [&str; 12] = [
    "light-pink",
    "lava",
    "ocean",
    "paysage",
    "couchant",
    "aurore",
    "atlantica",
    "sakura",
    "nuit-avril",
    "glace",
    "orange-teal",
    "sorbet",
];

/// Les trois familles « dont l'index de palette défile avec le temps » (critère n° 1).
const CYCLIQUES: [&str; 3] = ["vague", "respiration", "rotation"];

/// Les cinq familles à index borné « **inchangées, à l'octet près** — leur index ne sort jamais de
/// `[0, 1[` » (critère n° 2 réécrit), dans l'ordre où l'issue les nomme.
const BORNEES_INVARIANTES: [&str; 5] = ["comete", "balayage", "braise", "pouls", "scintillement"];

/// Les trois familles à index borné qui « changent **uniquement là où leur index atteignait
/// exactement 1,0**, que `fraction` repliait sur le premier arrêt de la palette » (critère n° 2
/// réécrit) — les trois de #127, livrées la veille de cette issue.
const BORNEES_RECADREES: [&str; 3] = ["bougie", "nuee", "artifice"];

/// Les huit directions, dans un ordre écrit ici et non lu chez le code.
///
/// Les empreintes figées les balaient dans cet ordre : le figeage cesserait d'être un figeage si
/// son parcours pouvait changer sous lui.
const HUIT_DIRECTIONS: [Direction; 8] = [
    Direction::BasHaut,
    Direction::HautBas,
    Direction::AvantArriere,
    Direction::ArriereAvant,
    Direction::Horaire,
    Direction::Antihoraire,
    Direction::BordsCentre,
    Direction::CentreBords,
];

/// Durée d'un cycle, en pas, à la vitesse 1 — figée par `spec_sens.rs` (issue #49).
const PERIODE: u32 = 120;

/// Le nombre de LED du boîtier : 10 × 8 + 4 × 11.
const LED_DU_BOITIER: usize = 124;

/// Le plus grand saut toléré d'une image à la suivante, sur une LED, sous palette.
///
/// C'est le chiffre de l'issue : « le pire saut d'une image à la suivante passe **sous 150** sur
/// les douze palettes, contre 254 aujourd'hui ». Il n'est pas rond par hasard — le prototype de
/// l'issue mesure 138 sur `nuit-avril`, dont la rampe interne est traversée deux fois plus vite par
/// l'aller-retour. Descendre le seuil sous 138 exigerait de corriger une donnée de WLED, ce que
/// l'issue met hors scope.
const SAUT_MAXIMAL: u8 = 150;

/// Ce que le même saut vaut **sans** palette, à ne pas dépasser.
///
/// Appareil, et non critère : mesuré sur le rendu courant, vitesse 1, les huit directions, un cycle
/// entier — `vague` **7**, `respiration` **6**. Le plafond est posé cinq fois plus haut. Sans lui,
/// un test qui ne mesurerait plus rien du tout — une famille figée sur une image unique, par
/// exemple — passerait le seuil de 150 sans rien prouver.
const SAUT_SANS_PALETTE: u8 = 40;

/// Écart minimal entre les deux bouts d'une palette pour qu'elle serve de témoin au plein niveau.
///
/// « Une palette dont les deux bouts diffèrent nettement » : sept des douze passent — `lava`,
/// `glace` et `orange-teal` à 255, `paysage` à 225, `couchant` à 207, `sorbet` à 153, `atlantica` à
/// 142. Sous ce seuil, rendre le premier arrêt au lieu du dernier ne se verrait pas plus qu'une
/// erreur d'arrondi, et le test cesserait de mesurer quoi que ce soit.
const ECART_DES_BOUTS: u8 = 128;

/// Écart toléré entre ce qu'une LED à plein niveau rend et le dernier arrêt de la palette.
///
/// À intensité maximale la LED rend le point de palette lui-même, sans atténuation : l'écart
/// attendu est nul. Huit couvre le cas où l'intensité maximale relevée vaut 254/255 plutôt que 255
/// — le point échantillonné est alors 254,5 au lieu de 255, et `lava`, dont la dernière rampe est la
/// plus raide des douze, y perd cinq unités de bleu. Seize fois moins que [`ECART_DES_BOUTS`] : les
/// deux bouts ne peuvent pas se confondre.
const TOLERANCE_PLEIN_NIVEAU: u8 = 8;

// ---------------------------------------------------------------------------
// Les seuils de teinte, et d'où ils viennent
// ---------------------------------------------------------------------------

/// En deçà de cette composante maximale, une LED est trop sombre pour porter une teinte.
///
/// Huit et non quatre-vingt-seize comme dans `spec_126_palettes.rs` : là-bas on cherchait à
/// distinguer deux teintes voisines, ici on cherche à **reconnaître** une teinte connue d'avance,
/// et l'extrémité d'un aller-retour est vue à intensité réduite — 38/255 sur `respiration`,
/// ~125/255 sur `rotation`. Un plancher calibré pour le plein éclat déclarerait illisible le seul
/// endroit que ce test regarde.
const SEUIL_EXTREMITE: u8 = 8;

/// Écart de teinte au-delà duquel une LED ne porte plus la couleur d'un arrêt.
///
/// Mesuré sur le rendu courant, cinq palettes, `vague`, `respiration` et `rotation` : le pire écart
/// vaut **0,111**, sur `rotation` sous `couchant`, dont l'extrémité lointaine n'est vue qu'à très
/// basse intensité. Tous les autres tiennent sous 0,05. Le seuil est posé juste au-dessus du pire.
const TOLERANCE_EXTREMITE: f32 = 0.15;

/// Plancher de lisibilité du test de symétrie.
///
/// Seize, parce que le doublement se lit sur **tout** le cycle et non sur un unique instant :
/// monter le plancher retirerait des pas du dénominateur au lieu de rendre la mesure plus sûre.
/// Mesuré : à ce plancher, le rendu courant donne 0,02 à 0,05 de doublement, là où l'aller-retour
/// doit en donner au moins dix fois plus.
const SEUIL_SYMETRIE: u8 = 16;

/// Deux LED portent la **même** teinte en deçà de cet écart.
///
/// Cinq centièmes, soit trois fois le bruit de quantification au plancher ci-dessus (0,5/16 ≈
/// 0,031) et deux fois l'écart qui sépare deux pas voisins sur le chemin d'une palette témoin.
const MEME_TEINTE: f32 = 0.05;

/// Distance minimale, en pas, entre deux instants qui portent la même teinte.
///
/// Sans elle, le test serait trivialement vert : deux pas voisins portent presque toujours la même
/// teinte, parcours enroulé ou non. Cinq pas valent au moins 0,08 de chemin sur la palette témoin,
/// soit largement plus que [`MEME_TEINTE`].
const ECART_DE_PAS: i64 = 5;

/// Part des instants qui doivent voir leur teinte une seconde fois, dans le cycle.
///
/// Deux populations, mesurées : le rendu courant donne **0,02 à 0,05** — un parcours qui enroule ne
/// voit chaque teinte qu'une fois —, un aller-retour en donne au moins 0,80 en théorie et 0,74 dans
/// le pire cas estimé (`rotation`, dont l'arête de luminosité éteint une partie des retours). Le
/// seuil est posé à mi-chemin, dix fois au-dessus de ce qu'on mesure aujourd'hui.
const DOUBLEMENT_EXIGE: f32 = 0.50;

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
}

fn palette(nom: &str) -> Palette {
    Palette::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est une palette : {erreur}"))
}

fn geometrie() -> Geometrie {
    Geometrie::mesuree()
}

/// L'écriture d'une couleur sur le socket : six chiffres hexadécimaux minuscules.
fn hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

/// La couleur d'une LED désignée par son rang : les dix ventilateurs, puis les quatre barrettes.
///
/// Idiome repris de `spec_126_palettes.rs` : les ventilateurs sont cherchés **par position** et
/// jamais par indice de tableau, l'`Image` portant la position à côté des couleurs précisément pour
/// que personne n'ait à connaître l'ordre du tableau.
fn couleur_par_rang(image: &Image, rang: usize) -> Rgb {
    let par_ventilateur = Position::ALL.len() * LEDS_PER_FAN as usize;
    if rang < par_ventilateur {
        let position = Position::ALL[rang / LEDS_PER_FAN as usize];
        image
            .ventilateurs
            .iter()
            .find(|(p, _)| *p == position)
            .unwrap_or_else(|| panic!("l'image ne contient pas {}", position.slug()))
            .1[rang % LEDS_PER_FAN as usize]
    } else {
        let reste = rang - par_ventilateur;
        image.barrettes[reste / LEDS_PER_STICK][reste % LEDS_PER_STICK]
    }
}

/// Les 124 couleurs d'une image, écrites bout à bout en hexadécimal.
fn empreinte_hexa(image: &Image) -> String {
    (0..LED_DU_BOITIER)
        .map(|rang| hexa(couleur_par_rang(image, rang)))
        .collect()
}

/// Les couleurs d'un cycle entier, rangées `[pas][rang]`.
fn couleurs_du_cycle(animation: &Animation, reglages: &Reglages) -> Vec<Vec<Rgb>> {
    let geometrie = geometrie();
    (0..PERIODE)
        .map(|pas| {
            let image = animation.image(&geometrie, reglages, pas);
            (0..LED_DU_BOITIER)
                .map(|rang| couleur_par_rang(&image, rang))
                .collect()
        })
        .collect()
}

/// La teinte d'une couleur : ses composantes ramenées à la plus grande.
///
/// Rend `None` sous le plancher donné. Deux couleurs proportionnelles ont la **même** teinte, ce
/// qui est exactement ce qu'il faut : une famille applique `couleur × intensité`, et les critères
/// de cette issue portent sur le chemin de couleur, jamais sur l'enveloppe du motif.
fn teinte(couleur: Rgb, plancher: u8) -> Option<[f32; 3]> {
    let plus_grande = couleur.r.max(couleur.g).max(couleur.b);
    if plus_grande < plancher {
        return None;
    }
    let plus_grande = f32::from(plus_grande);
    Some([
        f32::from(couleur.r) / plus_grande,
        f32::from(couleur.g) / plus_grande,
        f32::from(couleur.b) / plus_grande,
    ])
}

/// L'écart entre deux teintes : la plus grande différence, composante par composante.
fn ecart_de_teinte(une: [f32; 3], autre: [f32; 3]) -> f32 {
    (0..3)
        .map(|composante| (une[composante] - autre[composante]).abs())
        .fold(0.0f32, f32::max)
}

/// L'écart entre deux couleurs : la plus grande différence, composante par composante.
///
/// C'est la lecture du « saut » de l'issue — voir l'arbitrage n° 1 en tête de fichier.
fn saut(une: Rgb, autre: Rgb) -> u8 {
    une.r
        .abs_diff(autre.r)
        .max(une.g.abs_diff(autre.g))
        .max(une.b.abs_diff(autre.b))
}

/// La distance cyclique entre deux pas d'un même cycle.
fn distance_cyclique(un: usize, autre: usize) -> i64 {
    let brut = (un as i64 - autre as i64).rem_euclid(i64::from(PERIODE));
    brut.min(i64::from(PERIODE) - brut)
}

/// FNV-1a 64 bits, écrit ici plutôt qu'emprunté.
///
/// Il ne sert qu'à condenser des milliers d'empreintes en une constante qu'on puisse relire : ce
/// n'est pas une empreinte cryptographique, et il n'a rien à protéger. Les images qu'un digest
/// résume sont, elles, comparées octet pour octet — ce sont les mêmes 124 couleurs en hexadécimal
/// que les empreintes complètes, simplement pliées.
fn digest(morceaux: impl IntoIterator<Item = String>) -> String {
    let mut etat: u64 = 0xcbf2_9ce4_8422_2325;
    for morceau in morceaux {
        for octet in morceau.as_bytes() {
            etat ^= u64::from(*octet);
            etat = etat.wrapping_mul(0x100_0000_01b3);
        }
    }
    format!("{etat:016x}")
}

/// Les réglages du figeage « sans palette » : une couleur dont les trois composantes diffèrent deux
/// à deux, une vitesse et une direction qui ne sont pas celles par défaut.
fn reglages_sans_palette(animation: &Animation, direction: Direction) -> Reglages {
    Reglages {
        couleur: Rgb::new(0xff, 0x20, 0x80),
        vitesse: 3,
        direction: if animation.parametres_acceptes().contains(&"direction") {
            direction
        } else {
            Reglages::default().direction
        },
        sonde: None,
        palette: None,
    }
}

/// Les réglages du figeage « sous palette » : tout par défaut sauf la vitesse, posée à 1 pour que
/// [`PERIODE`] pas balaient exactement un cycle.
fn reglages_sous_palette(nom_de_palette: &str) -> Reglages {
    Reglages {
        vitesse: 1,
        palette: Some(palette(nom_de_palette)),
        ..Reglages::default()
    }
}

/// Les palettes dont les deux bouts sont **deux couleurs nettement différentes**.
///
/// Sept des douze passent — voir [`ECART_DES_BOUTS`]. Contrairement à
/// [`palettes_a_bouts_lisibles`], un bout noir convient parfaitement ici : le test du plein niveau
/// compare des **couleurs**, pas des teintes, et le noir de `lava` contre son blanc est justement
/// le contraste le plus fort des douze.
fn palettes_a_bouts_distants() -> Vec<Palette> {
    DOUZE
        .iter()
        .map(|nom| palette(nom))
        .filter(|palette| {
            let arrets = palette.arrets();
            saut(arrets[0].1, arrets[arrets.len() - 1].1) >= ECART_DES_BOUTS
        })
        .collect()
}

/// Les palettes dont les **deux bouts** portent une teinte lisible et distincte.
///
/// Lues chez les arrêts, jamais nommées en dur — voir l'arbitrage n° 4. Cinq des douze passent :
/// `lava`, `paysage` et `glace` commencent sur du noir, `aurore` et `nuit-avril` sur du
/// `01052d` trop sombre, `ocean` sur un `103033` de même, et les deux bouts de `sakura` sont deux
/// rouges qu'aucun test ne saurait départager.
fn palettes_a_bouts_lisibles() -> Vec<Palette> {
    DOUZE
        .iter()
        .map(|nom| palette(nom))
        .filter(|palette| {
            let arrets = palette.arrets();
            match (
                teinte(arrets[0].1, 96),
                teinte(arrets[arrets.len() - 1].1, 96),
            ) {
                (Some(premier), Some(dernier)) => ecart_de_teinte(premier, dernier) >= 0.25,
                _ => false,
            }
        })
        .collect()
}

/// Les palettes dont le chemin de teintes ne repasse **jamais** par une teinte déjà vue.
///
/// C'est la condition pour que « cette teinte se revoit plus loin dans le cycle » veuille dire
/// quelque chose : sur une palette qui revient sur ses pas, une teinte vue deux fois ne prouve
/// aucun aller-retour. Mesuré sur les douze : `couchant` est la seule à passer avec un chemin
/// entièrement lisible — `nuit-avril` n'a pas de collision non plus, mais 90 de ses 256 positions
/// seulement portent une teinte, le reste étant trop sombre.
fn palettes_temoins_du_doublement() -> Vec<Palette> {
    DOUZE
        .iter()
        .map(|nom| palette(nom))
        .filter(|palette| {
            let chemin: Vec<Option<[f32; 3]>> = (0..=255u16)
                .map(|position| teinte(palette.echantillon(f32::from(position)), 96))
                .collect();
            if chemin.iter().filter(|teinte| teinte.is_some()).count() < 200 {
                return false;
            }
            !(0..chemin.len()).any(|ici| {
                (ici + 32..chemin.len()).any(|la| match (chemin[ici], chemin[la]) {
                    (Some(une), Some(autre)) => ecart_de_teinte(une, autre) < MEME_TEINTE,
                    _ => false,
                })
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 0 — l'appareil : le catalogue se partage en trois, et rien ne tombe entre
// ---------------------------------------------------------------------------

#[test]
fn chaque_famille_qui_accepte_une_palette_est_dans_un_des_trois_camps() {
    // Appareil, vert avant comme après. L'issue partage le catalogue en trois — trois familles
    // « dont l'index de palette défile avec le temps », cinq à index borné inchangées, et trois
    // recadrées à la borne — et tous les tests de ce fichier reposent sur ce partage.
    //
    // ⚠️ **Une famille qui ne serait dans aucun des trois camps ne serait vérifiée par personne.**
    // Le catalogue s'est étendu trois fois en deux mois (#126, #127, #133) ; le jour où une
    // quatorzième famille arrive, c'est ici qu'on l'apprend, et non en la voyant sauter devant le
    // boîtier.
    let mut declarees: Vec<&str> = CYCLIQUES
        .iter()
        .chain(&BORNEES_INVARIANTES)
        .chain(&BORNEES_RECADREES)
        .copied()
        .collect();
    declarees.sort_unstable();
    let doublons = declarees.len();
    declarees.dedup();
    assert_eq!(
        declarees.len(),
        doublons,
        "une famille est déclarée dans deux camps à la fois : {declarees:?}"
    );

    let mut avec_palette: Vec<&str> = CATALOGUE
        .iter()
        .copied()
        .filter(|nom| animation(nom).parametres_acceptes().contains(&"palette"))
        .collect();
    avec_palette.sort_unstable();
    assert_eq!(
        declarees, avec_palette,
        "les onze familles qui acceptent « palette » doivent être exactement les trois cycliques, \
         les cinq invariantes et les trois recadrées — sinon une famille échappe à tous les tests \
         de cette issue"
    );
}

// ---------------------------------------------------------------------------
// 1 — les données de palette ne bougent pas
// ---------------------------------------------------------------------------

/// Les arrêts des douze palettes, relevés sur `main` à la révision `829f07c`, avant que #138 ne
/// touche à quoi que ce soit : une position sur deux chiffres hexadécimaux puis sa couleur sur six,
/// arrêt après arrêt.
const ARRETS_FIGES: [(&str, &str); 12] = [
    (
        "light-pink",
        "004f206d195a28753366307c4c8d87b966b4def86dd0ecfc72edfaff7acec8ef95b195deb7bb82cbffc66fb8",
    ),
    (
        "lava",
        "000000002e4d000060b100006cc4260977d74c1392eb731daeff9929bcffb229caffcc29daffe629eaffff29f4ffff8f\
         ffffffff",
    ),
    ("ocean", "00103033591ba6af99c5e9ffff009198"),
    (
        "paysage",
        "00000000251f59134c48b22b7f96eb0580baea7782dee9fc99c5dbe7cc84b3fdff1c6be1",
    ),
    (
        "couchant",
        "00b5000016da550033ffaa0055d3554d87a700a9c64900bcff0000cf",
    ),
    ("aurore", "0001052d4000c8178000ff00aa00f32dc8008707ff01052d"),
    (
        "atlantica",
        "00001c70322060ff6400f32d960c5f52c819be5fff28aa50",
    ),
    ("sakura", "00c4130a41ff452d82df2d48c3ff5267ffdf0d11"),
    (
        "nuit-avril",
        "0001052d0a01052d1905a9af2801052d3d01052d4c2daf1f5b01052d7001052d7ff996058f01052da201052db2ff5c00\
         c101052dd601052de5df2d48f401052dff01052d",
    ),
    (
        "glace",
        "000000003b003375770066ff952699ffb456ccffd9a7e6ffffffffff",
    ),
    ("orange-teal", "0000965c3700965cc8ff4800ffff4800"),
    (
        "sorbet",
        "00ff66292bff8c5a56ff335a7fff99a9aafffff9d171ff55ff9dff89",
    ),
];

#[test]
fn les_douze_palettes_portent_les_arrets_figes() {
    // Critère d'acceptation n° 6 : « Aucune donnée de palette n'est modifiée : les arrêts restent
    // ceux que WLED porte. »
    //
    // ⚠️ **C'est le critère le plus facile à violer sans s'en apercevoir**, parce que l'issue met
    // « boucler les palettes elles-mêmes » hors scope en une ligne. Ajouter un arrêt de retour en
    // fin de dégradé rendrait le parcours continu **et** ferait passer le test des sauts : la
    // correction serait invisible sauf ici. Le README dit pourquoi c'est refusé — « ça inventerait
    // une rampe que WLED n'a pas, et demanderait de choisir quelle part du cycle lui donner ».
    //
    // ⚠️ Les valeurs viennent du dépôt tel qu'il est, pas d'une recopie de `palettes.cpp` : c'est
    // « rien ne change » qu'on vérifie, et non la fidélité à l'amont, que #126 a déjà figée.
    for (nom, attendus) in ARRETS_FIGES {
        let attendus: String = attendus.chars().filter(|c| !c.is_whitespace()).collect();
        let palette = palette(nom);
        let rendus: String = palette
            .arrets()
            .iter()
            .map(|(position, couleur)| format!("{position:02x}{}", hexa(*couleur)))
            .collect();
        assert_eq!(
            rendus, attendus,
            "« {nom} » ne porte plus les mêmes arrêts qu'avant #138 — les douze dégradés viennent \
             de WLED et rien dans cette issue ne demande d'y toucher"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — sans palette, rien ne change du tout
// ---------------------------------------------------------------------------

/// Le condensé d'un cycle entier, par famille, **sans palette**, relevé sur `main` à la révision
/// `829f07c`.
///
/// Chaque digest plie 480 images — les huit directions, soixante pas chacune — pour les familles
/// qui acceptent une direction, soixante pour les autres. Les réglages sont ceux de
/// [`reglages_sans_palette`].
///
/// ⚠️ **Un digest et une empreinte ne se remplacent pas.** L'empreinte complète nomme la LED qui a
/// bougé, ce qu'un digest ne saura jamais faire ; le digest couvre le cycle entier et les huit
/// directions, ce qu'aucune empreinte tenant dans un fichier ne couvrirait. Le défaut que
/// l'ensemble guette — une couleur décalée d'une unité sur une seule image — se cache exactement
/// entre les deux.
const SANS_PALETTE_DIGESTS: [(&str, &str); 13] = [
    ("vague", "71bd13c938475bfb"),
    ("comete", "d53a368c74840595"),
    ("respiration", "9dc6f5ee30c9d76c"),
    ("arc-en-ciel", "df19e701ba96949a"),
    ("balayage", "abd235e486545995"),
    ("braise", "073e869bf4adf3f8"),
    ("rotation", "f24b9b3d1a64b91d"),
    ("thermique", "64be32e23870502d"),
    ("pouls", "cb9efd3733304d35"),
    ("scintillement", "f61ea833283b292e"),
    ("bougie", "e38729c1b699d22a"),
    ("nuee", "1376a4d9350a71b8"),
    ("artifice", "4559d8fa4f6d5e3a"),
];

/// Trois images complètes, **sans palette**, relevées sur `main` à la révision `829f07c` :
/// direction `bas-haut` quand la famille en accepte une, pas 17.
///
/// Les trois familles ne sont pas prises au hasard, et c'est le choix de `spec_126_palettes.rs` :
/// `vague` est l'onde plane, qui ne suit que la position ; `comete` a une traînée, donc un gradient
/// d'intensité et beaucoup de LED éteintes ; `braise` croise deux ondes et un frémissement par LED.
const SANS_PALETTE_EMPREINTES: [(&str, &str); 3] = [
    (
        "vague",
        "0d01060d01060d01060d01060d01060d01060d01060d01060d01060d01060d01060d01060d01060d01060d01060d0106\
         0d01060d01060d01060d01060d01060d01060d01060d01065b0b2e25041201000003000101000008010439071c660c33\
         fb1f7efa1f7dd01a68a51453af1658e21c71fe1f7ff81f7c2704135e0b2fac1556d51a6bcd196793124a4608231f0410\
         01000003000101000008010439071c660c335b0b2e2504120d01060d01060d01060d01060d01060d01060d01060d0106\
         0d01060d01060d01060d01060d01060d01060d01060d01060d01060d01060d01060d01060d01060d01060d01060d0106\
         cc1966c61863c01860b9175db21659ab1556a414529c134e95124a8d1147861043cc1966c61863c01860b9175db21659\
         ab1556a414529c134e95124a8d1147861043cc1966c61863c01860b9175db21659ab1556a414529c134e95124a8d1147\
         861043cc1966c61863c01860b9175db21659ab1556a414529c134e95124a8d1147861043",
    ),
    (
        "comete",
        "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         000000d41a6a700e383206193f071f901248f41e7a000000000000000000000000000000000000000000000000000000\
         000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         690d355f0c30560a2b4c092642082139071c2f05172504121b030e120209080104690d355f0c30560a2b4c0926420821\
         39071c2f05172504121b030e120209080104690d355f0c30560a2b4c092642082139071c2f05172504121b030e120209\
         080104690d355f0c30560a2b4c092642082139071c2f05172504121b030e120209080104",
    ),
    (
        "braise",
        "630c3138071c1f030f0b01050200011b030d4d0927680d34580b2c3d071e4e0927420821450822440822640c32630c32\
         1b030d4508223c071e510a2825041214020a0b01050901044f0a28a014509e134f6b0d35520a290801040a0105020001\
         710e38ac1556680d34550a2b2304122104100901041c030ef61e7bfe1f7fd51a6b6d0d37620c3137061b7e0f3fd81b6c\
         911248911249c81964f41e7ab11659ad1557b5165aa81554d71a6bfe1f7ff91f7ce91d75c51862b3165ad51a6bdd1b6f\
         a61453cd1966ea1d75e11c71bb175db21659ac1556a71554fb1f7ea514524008205c0b2e670c337c0f3eb31659ec1d76\
         4108204e092733061a0f01070300012004100600030f010733061a520a293c071e2f05174b092543082115020a000000\
         1b030d01000016020b3b071d4b09253206192204114809244b09252a0515070104050002040002230411490925470823\
         2104111b030e440822570b2c3b071d2004100500021a030d4e09275b0b2d4408221c030e",
    ),
];

#[test]
fn sans_palette_le_catalogue_rend_les_images_figees() {
    // Test d'intention n° 1 de l'issue — « sans palette, le catalogue rend exactement les images
    // d'avant » —, et critère d'acceptation n° 3 : « **Sans palette, rien ne change du tout** — le
    // catalogue entier rend les mêmes images qu'avant, octet pour octet. La correction ne touche
    // que le chemin `Some(palette)`. »
    //
    // ⚠️ **Les octets attendus ont été relevés sur `main` à la révision `829f07c`**, en exécutant le
    // code de ce jour-là. Ce ne sont ni des valeurs calculées à la main ni des valeurs relues dans
    // l'implémentation : c'est le comportement d'avant, figé.
    //
    // ⚠️ **On ne corrige jamais ces constantes pour faire passer le test.** Si elles diffèrent après
    // implémentation, c'est que #138 a changé le rendu d'un boîtier qui ne demande aucune palette —
    // et l'approche technique de l'issue est explicite sur ce point, « trois sites d'appel » et rien
    // d'autre. Sur SHYNAEL, l'état courant est un `anim braise couleur=… vitesse=…` : ce test est ce
    // qui garantit que le boîtier affichera demain ce qu'il affiche ce soir.
    //
    // ⚠️ `thermique` y figure sans sonde, et c'est voulu : `Reglages::default()` n'en porte aucune,
    // le rendu est alors la pulsation blanche de la sonde muette, et elle se fige comme le reste.
    let geometrie = geometrie();

    for (nom, attendue) in SANS_PALETTE_EMPREINTES {
        let attendue: String = attendue.chars().filter(|c| !c.is_whitespace()).collect();
        assert_eq!(
            attendue.len(),
            LED_DU_BOITIER * 6,
            "appareil : l'empreinte figée de « {nom} » ne porte pas {LED_DU_BOITIER} couleurs"
        );

        let animation = animation(nom);
        let reglages = reglages_sans_palette(&animation, Direction::BasHaut);
        assert_eq!(
            reglages.palette, None,
            "appareil : ce test doit mesurer « sans palette »"
        );

        let rendue = empreinte_hexa(&animation.image(&geometrie, &reglages, 17));
        if rendue != attendue {
            let rang = (0..LED_DU_BOITIER)
                .find(|rang| rendue[rang * 6..rang * 6 + 6] != attendue[rang * 6..rang * 6 + 6])
                .expect("deux empreintes qui diffèrent diffèrent sur au moins une LED");
            panic!(
                "« {nom} » sans palette (pas 17) ne rend plus l'image d'avant #138 : la LED n° \
                 {rang} porte {} au lieu de {} — la correction ne doit toucher que le chemin \
                 « Some(palette) »",
                &rendue[rang * 6..rang * 6 + 6],
                &attendue[rang * 6..rang * 6 + 6],
            );
        }
    }

    for (nom, attendu) in SANS_PALETTE_DIGESTS {
        let animation = animation(nom);
        let directions: Vec<Direction> = if animation.parametres_acceptes().contains(&"direction") {
            HUIT_DIRECTIONS.to_vec()
        } else {
            vec![Reglages::default().direction]
        };
        let rendu = digest(directions.into_iter().flat_map(|direction| {
            let reglages = reglages_sans_palette(&animation, direction);
            let geometrie = geometrie.clone();
            (0..60u32).map(move |pas| empreinte_hexa(&animation.image(&geometrie, &reglages, pas)))
        }));
        assert_eq!(
            rendu, attendu,
            "« {nom} » sans palette ne rend plus le même cycle qu'avant #138 (huit directions, \
             soixante pas) — une image au moins a bougé hors du pas 17 que l'empreinte complète \
             couvre"
        );
    }

    // Appareil : les treize familles du catalogue sont couvertes, et non douze.
    let figees: Vec<&str> = SANS_PALETTE_DIGESTS.iter().map(|(nom, _)| *nom).collect();
    assert_eq!(
        figees,
        CATALOGUE.to_vec(),
        "le figeage doit couvrir le catalogue entier, dans son ordre"
    );
}

// ---------------------------------------------------------------------------
// 3 — sous palette, les familles à index borné ne bougent pas non plus
// ---------------------------------------------------------------------------

/// Le condensé d'un cycle entier sous **chacune des douze palettes**, par famille à index borné
/// invariante, relevé sur `main` à la révision `829f07c`.
///
/// Chaque digest plie 1 440 images — douze palettes, [`PERIODE`] pas chacune — sous les réglages de
/// [`reglages_sous_palette`].
///
/// ⚠️ **Cinq familles, et non huit** : `bougie`, `nuee` et `artifice` en ont été retirées quand le
/// critère n° 2 a été réécrit — voir la note en tête de fichier. Les figer ici reviendrait à figer
/// l'éclair noir qu'elles portent aujourd'hui, et
/// [`a_plein_niveau_les_trois_familles_de_127_rendent_la_fin_du_degrade`] demande exactement
/// l'inverse.
const BORNEES_DIGESTS: [(&str, &str); 5] = [
    ("comete", "38ce82045570cc52"),
    ("balayage", "4a914daa9597ec6c"),
    ("braise", "738917df36e8da4a"),
    ("pouls", "41cf695395bc2152"),
    ("scintillement", "b500003f9cc4bc15"),
];

/// Deux images complètes sous `lava`, au pas 17, relevées sur `main` à la révision `829f07c`.
///
/// `comete` a une traînée, donc un gradient d'index et beaucoup de LED éteintes ; `braise` croise
/// deux ondes et un frémissement par LED. Deux mécaniques d'index différentes, et deux familles que
/// la correction ne doit pas effleurer.
///
/// ⚠️ **Trois empreintes ont été retirées d'ici** — `bougie` au pas 0, `nuee` au pas 6, `artifice`
/// au pas 54 —, et elles figeaient l'éclair noir : voir la note en tête de fichier. Elles portaient
/// le seul endroit où l'approche technique de l'issue touchait une famille à index borné, et c'est
/// ce constat qui a fait réécrire le critère n° 2 plutôt que l'approche.
const BORNEES_EMPREINTES: [(&str, u32, &str); 2] = [
    (
        "comete",
        17,
        "000000080000000000000000000000000000000000000000000000080000000000000000000000000000000000000000\
         000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000\
         000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         0000003229082421050000000000000000000000000000000000002722061a1904000000000000000000000000000000\
         0000002722061a19040000000000000000000000000000000000002722061a1904000000000000000000000000000000\
         000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         000000000000000000000000000000000000000000000000000000000000000000000000",
    ),
    (
        "braise",
        17,
        "c89f209751156f2d0b3e00001d00000d0000210000864110ab651aa35e188844115815053a00002300004a0701672509\
         0a00001700000000000c00004d0a024b08022000000b00003300008c47129651150b0000170000360000170000080000\
         520f039c57163400000100005411044c0902190000150000a9631ac89d20d5ba220c00000f00002b00003300006d2b0a\
         c5961fb06c1cb5751debeb33bd851ea8621ab06d1ccaa220c3931fc89d20af6a1c7f3b0e995415e5de24fcfce4dac523\
         a45e18b6781d9d57168c4712ae681cc89d20b3711cac671bdfd023a9631aa8621a7a360d5c1b0678350dc6991ff4f48f\
         0c00002e00006221084905011700000800001900004906015e1d074e0b020700000100001100002b0000440100180000\
         090000170000400000500d030d00000300000000000a00001d00003300001c00000400001c00003a0000200000010000\
         0000000000000000000b00002b0000220000060000210000220000090000000000000000",
    ),
];

#[test]
fn sous_palette_une_famille_a_index_borne_rend_les_images_figees() {
    // Test d'intention n° 2 de l'issue — « une famille à index borné rend exactement les images
    // d'avant, sous palette » —, et critère d'acceptation n° 2 **réécrit** : « `comete`, `balayage`,
    // `braise`, `pouls` et `scintillement` sont **inchangées, à l'octet près** — leur index ne sort
    // jamais de `[0, 1[`. »
    //
    // L'issue dit pourquoi elles ne sont pas concernées : « leur 0 et leur 1 sont les deux bouts du
    // motif, pas deux instants successifs ». Un aller-retour appliqué à `comete` replierait sa
    // traînée sur elle-même — la tête et la queue prendraient la même teinte —, ce qui ferait de la
    // correction d'un défaut la création d'un autre.
    //
    // ⚠️ **Cinq familles et non huit.** Les trois de #127 sortent bel et bien de `[0, 1[` : elles
    // atteignent exactement 1,0, et c'est la deuxième moitié du critère réécrit —
    // [`a_plein_niveau_les_trois_familles_de_127_rendent_la_fin_du_degrade`] la porte. Leur
    // invariance était figée ici jusqu'à la réécriture ; les octets qu'elle gardait sont nommés en
    // tête de fichier.
    //
    // ⚠️ **C'est cette garde-ci qui compte, et elle doit rester verte après implémentation.** La
    // promesse « purement additif » du README — « une commande sans `palette` rend exactement
    // l'image d'avant » — vaut aussi pour ces cinq-là sous palette : leurs profils enregistrés
    // continuent de s'appliquer sans être réécrits.
    let geometrie = geometrie();

    for (nom, pas, attendue) in BORNEES_EMPREINTES {
        let attendue: String = attendue.chars().filter(|c| !c.is_whitespace()).collect();
        assert_eq!(
            attendue.len(),
            LED_DU_BOITIER * 6,
            "appareil : l'empreinte figée de « {nom} » ne porte pas {LED_DU_BOITIER} couleurs"
        );

        let animation = animation(nom);
        let reglages = reglages_sous_palette("lava");
        let rendue = empreinte_hexa(&animation.image(&geometrie, &reglages, pas));
        if rendue != attendue {
            let rang = (0..LED_DU_BOITIER)
                .find(|rang| rendue[rang * 6..rang * 6 + 6] != attendue[rang * 6..rang * 6 + 6])
                .expect("deux empreintes qui diffèrent diffèrent sur au moins une LED");
            panic!(
                "« {nom} » sous « lava » (pas {pas}) ne rend plus l'image d'avant #138 : la LED n° \
                 {rang} porte {} au lieu de {} — l'index de cette famille est borné, l'aller-retour \
                 ne la concerne pas",
                &rendue[rang * 6..rang * 6 + 6],
                &attendue[rang * 6..rang * 6 + 6],
            );
        }
    }

    for (nom, attendu) in BORNEES_DIGESTS {
        let animation = animation(nom);
        let rendu = digest(DOUZE.into_iter().flat_map(|nom_de_palette| {
            let reglages = reglages_sous_palette(nom_de_palette);
            let geometrie = geometrie.clone();
            (0..PERIODE)
                .map(move |pas| empreinte_hexa(&animation.image(&geometrie, &reglages, pas)))
        }));
        assert_eq!(
            rendu, attendu,
            "« {nom} » ne rend plus le même cycle qu'avant #138 sous les douze palettes — son index \
             ne sort jamais de [0, 1[, ni l'aller-retour ni le bornage ne doivent la toucher"
        );
    }

    // Appareil : les cinq familles invariantes sont couvertes, dans l'ordre où l'issue les nomme.
    let figees: Vec<&str> = BORNEES_DIGESTS.iter().map(|(nom, _)| *nom).collect();
    assert_eq!(
        figees,
        BORNEES_INVARIANTES.to_vec(),
        "le figeage doit couvrir les cinq familles à index borné invariantes"
    );
}

// ---------------------------------------------------------------------------
// 3 bis — à la borne, c'est la fin du dégradé et non son début
// ---------------------------------------------------------------------------

#[test]
fn a_plein_niveau_les_trois_familles_de_127_rendent_la_fin_du_degrade() {
    // Seconde moitié du critère d'acceptation n° 2 **réécrit** : « `bougie`, `nuee` et `artifice`
    // changent uniquement là où leur index atteignait exactement 1,0, que `fraction` repliait sur le
    // premier arrêt de la palette. À plein niveau, une bougie rend désormais la **fin** du dégradé. »
    //
    // `fraction(1.0)` vaut `0.0`. Une bougie à son éclat maximal rend donc aujourd'hui la couleur de
    // **départ** de la palette au lieu de sa couleur de **fin** — sous `lava`, du noir au lieu du
    // blanc, c'est-à-dire l'inverse exact de ce que le motif veut dire. `Palette::echantillon`
    // documente pourtant « hors bornes, la couleur de borne, jamais d'extrapolation » (#126) : c'est
    // le `fraction` qui défait cette garantie, précisément à la borne.
    //
    // ⚠️ **Le plein niveau se repère à l'intensité, jamais en supposant un index.** Un index n'est
    // pas observable depuis l'extérieur ; l'intensité, si. La famille est donc rendue une première
    // fois **sans palette et en blanc** — la couleur rendue vaut alors `ffffff × niveau`, dont la
    // composante maximale *est* le niveau —, et les échantillons retenus sont ceux qui atteignent le
    // maximum du cycle. Cette sonde ne passe par aucun chemin `Some(palette)` : la correction ne la
    // déplace pas.
    //
    // ⚠️ **La comparaison porte sur la couleur, pas sur la teinte.** À intensité maximale il n'y a
    // aucune atténuation, donc la LED rend le point de palette lui-même ; et le noir de `lava`, qui
    // est le cas le plus parlant, n'a pas de teinte du tout.
    //
    // ⚠️ **« Uniquement là » est mesuré, pas promis.** L'appareil ci-dessous compte les échantillons
    // à plein niveau : **un** sur 14 880 pour `bougie`, 1 271 pour `nuee`, 1 163 pour `artifice`. Le
    // reste du cycle est couvert par le fait que ces trois familles ne reçoivent **pas**
    // l'aller-retour — l'issue le dit dans « ce que ça n'autorise pas ».
    let temoins = palettes_a_bouts_distants();
    assert!(
        temoins.len() >= 5,
        "appareil : seules {} palette(s) ont deux bouts distants de {ECART_DES_BOUTS} — le test ne \
         mesurerait presque rien",
        temoins.len()
    );

    for nom in BORNEES_RECADREES {
        let animation = animation(nom);

        // La sonde de niveau : la même famille, sans palette, en blanc.
        let sonde = couleurs_du_cycle(
            &animation,
            &Reglages {
                couleur: Rgb::new(0xff, 0xff, 0xff),
                vitesse: 1,
                palette: None,
                ..Reglages::default()
            },
        );
        let niveau = |couleur: Rgb| couleur.r.max(couleur.g).max(couleur.b);
        let plein = sonde
            .iter()
            .flatten()
            .map(|couleur| niveau(*couleur))
            .max()
            .expect("un cycle porte des couleurs");
        let echantillons: Vec<(usize, usize)> = sonde
            .iter()
            .enumerate()
            .flat_map(|(pas, image)| {
                image
                    .iter()
                    .enumerate()
                    .filter(move |(_, couleur)| niveau(**couleur) == plein)
                    .map(move |(rang, _)| (pas, rang))
            })
            .collect();

        // Appareil : le niveau va bien jusqu'au bout, et il y a quelque chose à mesurer. Sans cette
        // borne, une famille dont le niveau plafonnerait à 0,6 rendrait le milieu du dégradé, et
        // exiger d'elle la couleur de fin n'aurait aucun sens.
        assert_eq!(
            plein, 255,
            "appareil : « {nom} » plafonne à {plein}/255 d'intensité — son index n'atteint pas la \
             borne, et « à plein niveau » ne désigne rien"
        );
        assert!(
            !echantillons.is_empty(),
            "appareil : « {nom} » n'atteint son intensité maximale nulle part sur le cycle"
        );

        for palette in &temoins {
            let arrets = palette.arrets();
            let (position_du_bout, dernier) = arrets[arrets.len() - 1];
            let premier = arrets[0].1;
            let couleurs = couleurs_du_cycle(&animation, &reglages_sous_palette(palette.nom()));

            for (pas, rang) in &echantillons {
                let rendue = couleurs[*pas][*rang];
                let ecart = saut(rendue, dernier);
                assert!(
                    ecart <= TOLERANCE_PLEIN_NIVEAU,
                    "« {nom} » sous « {} », pas {pas}, LED n° {rang} : à plein niveau elle rend {}, \
                     à {ecart} du dernier arrêt ({position_du_bout} → {}) pour \
                     {TOLERANCE_PLEIN_NIVEAU} tolérés — le premier arrêt vaut {}, et c'est lui \
                     qu'un index replié par `fraction(1.0) == 0.0` rend à sa place. Une bougie à \
                     plein éclat doit montrer la fin du dégradé, pas son début",
                    palette.nom(),
                    hexa(rendue),
                    hexa(dernier),
                    hexa(premier),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — le parcours est continu
// ---------------------------------------------------------------------------

#[test]
fn sur_vague_et_respiration_aucune_led_ne_saute_de_plus_de_150() {
    // Test d'intention n° 3 de l'issue — « sur `vague` et `respiration`, aucune LED ne saute de plus
    // de 150 d'une image à la suivante, sur les douze palettes et un cycle entier » —, et critère
    // d'acceptation n° 4.
    //
    // C'est le test qui porte le défaut observé devant le boîtier : « on voit qu'il y a clairement
    // un rafraîchissement à chaque tour et que ce n'est pas continu ».
    //
    // ⚠️ **Borner au lieu d'enrouler ne le fera pas passer**, et l'issue le dit d'avance pour avoir
    // essayé : « `vague` reste à 254, parce que `spatiale - temps` parcourt (-1, 1] et qu'un `clamp`
    // l'épingle à 0 la moitié du temps, puis saute quand `temps` se replie ».
    //
    // ⚠️ **`rotation` n'y figure pas**, et c'est l'issue qui l'exclut : son arête de luminosité est
    // hors scope. Mesuré sur le rendu courant : elle saute de **253** sans aucune palette.
    let mut pire_sans_palette = 0u8;
    let mut palettes_qui_changent_l_image = 0usize;

    for nom in ["vague", "respiration"] {
        let animation = animation(nom);
        assert!(
            animation.parametres_acceptes().contains(&"direction"),
            "appareil : « {nom} » doit accepter une direction, sans quoi le balayage ci-dessous \
             mesurerait huit fois la même chose"
        );

        for direction in HUIT_DIRECTIONS {
            let sans_palette = Reglages {
                vitesse: 1,
                direction,
                ..Reglages::default()
            };
            let temoin = couleurs_du_cycle(&animation, &sans_palette);
            for pas in 0..PERIODE as usize {
                let suivant = (pas + 1) % PERIODE as usize;
                for (avant, apres) in temoin[pas].iter().zip(&temoin[suivant]) {
                    pire_sans_palette = pire_sans_palette.max(saut(*avant, *apres));
                }
            }

            for nom_de_palette in DOUZE {
                let reglages = Reglages {
                    vitesse: 1,
                    direction,
                    ..reglages_sous_palette(nom_de_palette)
                };
                let couleurs = couleurs_du_cycle(&animation, &reglages);
                if couleurs != temoin {
                    palettes_qui_changent_l_image += 1;
                }

                for pas in 0..PERIODE as usize {
                    let suivant = (pas + 1) % PERIODE as usize;
                    for (rang, (avant, apres)) in couleurs[pas]
                        .iter()
                        .zip(&couleurs[suivant])
                        .map(|(avant, apres)| (*avant, *apres))
                        .enumerate()
                    {
                        let mesure = saut(avant, apres);
                        assert!(
                            mesure <= SAUT_MAXIMAL,
                            "« {nom} » sous « {nom_de_palette} », direction « {} » : la LED n° \
                             {rang} passe de {} à {} entre les images {pas} et {suivant}, soit un \
                             saut de {mesure} pour {SAUT_MAXIMAL} tolérés — l'index de palette \
                             enroule, et la LED saute de l'écart entre les deux bouts du dégradé",
                            direction.slug(),
                            hexa(avant),
                            hexa(apres),
                        );
                    }
                }
            }
        }
    }

    // Appareil n° 1 : sans palette, le rendu ne saute pas — donc le seuil mesure bien ce que la
    // palette ajoute, et non une discontinuité que le motif porterait déjà.
    assert!(
        pire_sans_palette <= SAUT_SANS_PALETTE,
        "appareil : sans palette, `vague` et `respiration` sautent déjà de {pire_sans_palette} \
         pour {SAUT_SANS_PALETTE} tolérés — le seuil de {SAUT_MAXIMAL} ne mesurerait plus le \
         parcours de palette"
    );

    // Appareil n° 2 : les palettes sont vraiment échantillonnées. Une palette rangée puis ignorée
    // rendrait ce test trivialement vert — c'est « le pire des trois défauts » que
    // `spec_animations.rs` nomme déjà.
    assert_eq!(
        palettes_qui_changent_l_image,
        2 * 8 * DOUZE.len(),
        "appareil : une palette au moins laisse le cycle inchangé — elle est rangée dans les \
         réglages puis ignorée au rendu, et le seuil ne mesure alors rien"
    );
}

// ---------------------------------------------------------------------------
// 5 — le parcours va jusqu'aux deux bouts du dégradé
// ---------------------------------------------------------------------------

#[test]
fn le_parcours_atteint_les_deux_arrets_extremes() {
    // Test d'intention n° 4 de l'issue — « le parcours atteint bien les deux arrêts extrêmes de la
    // palette » —, et critère d'acceptation n° 5 : « Aux deux extrémités du parcours, la couleur est
    // celle de l'arrêt correspondant — l'aller-retour ne rogne pas le dégradé. »
    //
    // Le défaut visé est muet : un aller-retour écrit `0,05 + 0,9 × …` au lieu de `2t` rendrait le
    // parcours parfaitement continu, ferait passer le test des sauts, et amputerait le dégradé de
    // ses deux bouts sans que rien ne le signale. Les palettes de WLED y perdraient justement ce
    // qu'on va y chercher — le noir et le blanc de `lava`, les deux bleus de `couchant`.
    //
    // ⚠️ **`vague` n'y figure pas**, et c'est une mesure : son enveloppe d'intensité s'annule
    // complètement une fois par cycle — relevé LED par LED, `…, 2, 1, 0, 0, 0, 0, 0, 1, 2, …` —, si
    // bien qu'une extrémité du parcours peut y tomber sur une LED noire, qui ne porte aucune teinte.
    // Elle est couverte par le test des sauts et par celui de la symétrie, qui ne demandent ni l'un
    // ni l'autre qu'une extrémité soit lisible.
    //
    // ⚠️ **La couleur est reconnue à sa *teinte*, jamais à ses trois octets.** Une extrémité vue à
    // 15 % d'intensité rend `00001f` là où l'arrêt vaut `0000cf` : ce sont deux écritures de la même
    // couleur, et exiger les octets reviendrait à exiger que l'extrémité tombe au plein éclat, ce
    // que rien dans l'issue ne demande.
    let temoins = palettes_a_bouts_lisibles();
    assert!(
        temoins.len() >= 4,
        "appareil : seules {} palette(s) ont deux bouts lisibles et distincts — le test ne \
         mesurerait presque rien",
        temoins.len()
    );

    for nom in ["respiration", "rotation"] {
        let animation = animation(nom);
        for palette in &temoins {
            let arrets = palette.arrets();
            let bouts = [
                ("premier", arrets[0]),
                ("dernier", arrets[arrets.len() - 1]),
            ];
            let couleurs = couleurs_du_cycle(&animation, &reglages_sous_palette(palette.nom()));

            for (rang_du_bout, (position, couleur)) in bouts {
                let cherchee = teinte(couleur, 96).expect("un bout retenu porte une teinte");
                let mut meilleur = f32::MAX;
                for image in &couleurs {
                    for rendue in image {
                        if let Some(rendue) = teinte(*rendue, SEUIL_EXTREMITE) {
                            meilleur = meilleur.min(ecart_de_teinte(rendue, cherchee));
                        }
                    }
                }
                assert!(
                    meilleur <= TOLERANCE_EXTREMITE,
                    "« {nom} » sous « {} » : sur un cycle entier et les {LED_DU_BOITIER} LED, la \
                     teinte la plus proche du {rang_du_bout} arrêt ({position} → {}) en reste à \
                     {meilleur:.3}, pour {TOLERANCE_EXTREMITE:.2} tolérés — le parcours n'atteint \
                     pas ce bout du dégradé",
                    palette.nom(),
                    hexa(couleur),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 6 — le parcours est symétrique : il revient par où il est venu
// ---------------------------------------------------------------------------

#[test]
fn le_parcours_est_symetrique_chaque_teinte_est_vue_deux_fois() {
    // Test d'intention n° 5 de l'issue — « le parcours est symétrique : la teinte à `t` et celle à
    // `1 - t` sont la même ».
    //
    // ⚠️ Voir l'arbitrage n° 2 en tête de fichier : `t` n'est jamais rendu, mais sa conséquence
    // l'est. Si la teinte à `t` et celle à `1 - t` sont la même, alors une LED fixe voit **deux
    // fois** chaque teinte du chemin sur un cycle — une à l'aller, une au retour. Un parcours qui
    // enroule ne la voit qu'une seule fois, et c'est ce qu'on mesure : 0,02 à 0,05 aujourd'hui.
    //
    // ⚠️ **C'est aussi ce qui sépare l'aller-retour de la solution mise hors scope.** Boucler la
    // palette elle-même rendrait le parcours continu et ferait passer le test des sauts, sans rien
    // doubler du tout. L'issue refuse cette voie — « ça inventerait une rampe que WLED n'a pas » —
    // et ce test est le seul endroit où le refus devient vérifiable.
    //
    // ⚠️ **La palette témoin est choisie par mesure**, jamais nommée en dur : sur une palette qui
    // repasse par une teinte déjà vue, voir deux fois la même teinte ne prouverait aucun retour.
    // Aujourd'hui, `couchant` est la seule des douze dont le chemin soit à la fois injectif et
    // lisible de bout en bout — ses deux extrémités sont un rouge pur et un bleu pur.
    let temoins = palettes_temoins_du_doublement();
    assert!(
        !temoins.is_empty(),
        "appareil : aucune des douze palettes n'a de chemin de teintes injectif — « cette teinte se \
         revoit plus loin » ne voudrait plus rien dire, et le test ne mesurerait rien"
    );

    for nom in CYCLIQUES {
        let animation = animation(nom);
        for palette in &temoins {
            let couleurs = couleurs_du_cycle(&animation, &reglages_sous_palette(palette.nom()));
            let mut led_retenues = 0usize;

            for rang in 0..LED_DU_BOITIER {
                let teintes: Vec<Option<[f32; 3]>> = couleurs
                    .iter()
                    .map(|image| teinte(image[rang], SEUIL_SYMETRIE))
                    .collect();
                let lisibles = teintes.iter().filter(|teinte| teinte.is_some()).count();
                if lisibles * 2 < PERIODE as usize {
                    // Une LED éteinte plus de la moitié du cycle ne dit rien du chemin.
                    continue;
                }
                led_retenues += 1;

                let revues = teintes
                    .iter()
                    .enumerate()
                    .filter_map(|(pas, ici)| ici.map(|ici| (pas, ici)))
                    .filter(|(pas, ici)| {
                        teintes.iter().enumerate().any(|(ailleurs, la)| {
                            distance_cyclique(*pas, ailleurs) >= ECART_DE_PAS
                                && la.is_some_and(|la| ecart_de_teinte(*ici, la) < MEME_TEINTE)
                        })
                    })
                    .count();
                let part = revues as f32 / lisibles as f32;

                assert!(
                    part >= DOUBLEMENT_EXIGE,
                    "« {nom} » sous « {} » : sur la LED n° {rang}, {revues} des {lisibles} instants \
                     lisibles du cycle revoient leur teinte ailleurs — soit {part:.2} pour \
                     {DOUBLEMENT_EXIGE:.2} exigés. Le parcours ne revient pas par où il est venu : \
                     chaque teinte n'est vue qu'une fois, ce qui est la signature d'un index qui \
                     enroule",
                    palette.nom(),
                );
            }

            assert!(
                led_retenues >= 8,
                "appareil : « {nom} » sous « {} » n'a que {led_retenues} LED allumées la moitié du \
                 cycle — le doublement n'a été mesuré nulle part",
                palette.nom(),
            );
        }
    }
}
