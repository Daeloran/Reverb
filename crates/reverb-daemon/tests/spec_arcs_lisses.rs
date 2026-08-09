//! Tests d'intention des arcs lissés et de la fonte à terminaisons rondes — issue #93.
//!
//! Écrits **avant** l'implémentation, depuis l'issue #93 seule.
//! `crates/reverb-daemon/src/ecran.rs` — le fichier que ce chantier modifie — n'a **pas** été lu
//! pour les produire. Ce qui en est repris ici, ce sont les signatures publiques déjà établies par
//! #33, #80 et #90, telles que les tests d'intention de ces issues les nomment : `Dalle`,
//! `Dalle::unie`, `Dalle::octets`, `Dalle::texte`, `Dalle::arc`, et côté `reverb-proto`
//! `Ancre::boite`, `Ancre::secteur`, `Secteur`, `COURONNE_RAYON_INTERIEUR`/`EXTERIEUR`. Si l'un de
//! ces tests échoue après implémentation, c'est le code qu'on corrige, jamais le test.
//!
//! Ce fichier **prolonge** `spec_police_arcs.rs` (#90) : il ne rejoue ni la taille du tampon, ni
//! l'échelle de température, ni la disjonction des secteurs au degré, ni « rien ne sort du disque »
//! — ce dernier point couvrant déjà `Dalle::arc` à cinq proportions, donc la piste avec. Il ne garde
//! que ce que #93 ajoute : des **bords lissés**, des **extrémités en demi-disque**, une **piste**
//! sous l'arc, et une fonte dont les **terminaisons sont rondes**.
//!
//! # La couture que ce fichier exige
//!
//! Un seul ajout, et c'est délibéré : tout le reste se mesure par différence entre deux dalles, sans
//! qu'aucune couleur ait à être recopiée dans un test.
//!
//! ```ignore
//! /// La couleur pleine dont l'arc est peint, en `(rouge, vert, bleu)`.
//! ///
//! /// Publique parce que les critères d'acceptation de #93 la nomment : « un arc vide montre sa
//! /// piste et **aucun pixel de couleur d'arc** », « le remplissage reste strictement croissant,
//! /// **mesuré sur les pixels de la couleur d'arc**, la piste ne comptant pas ».
//! pub const ARC_COULEUR: (u8, u8, u8);
//! ```
//!
//! ⚠️ **La couleur de la piste n'est pas exigée**, et c'est un choix. La piste se mesure ici par
//! différence avec le fond — « ce que l'arc vide change » —, ce qui laisse l'implémentation libre de
//! la peindre d'une couleur fixe ou d'assombrir ce qu'il y a dessous, comme les champs de #80
//! assombrissent leur plaque à 30 %. Une constante de plus aurait figé ce choix sans qu'aucun
//! critère ne le demande.
//!
//! # Ce que ce fichier tranche, là où l'issue laisse ouvert
//!
//! 1. **L'arc est d'une seule couleur.** Les critères disent « la couleur d'arc » au singulier et
//!    comptent « les pixels de la couleur d'arc » : un dégradé le long de l'arc rendrait ces deux
//!    phrases indécidables. Si l'implémentation veut un dégradé un jour, c'est l'issue qu'on rouvre,
//!    pas ce fichier qu'on assouplit.
//! 2. **L'arc occupe l'essentiel de l'épaisseur de la couronne.** Au moins 40 % de
//!    `COURONNE_RAYON_INTERIEUR`…`EXTERIEUR`. Sans ce plancher, « les extrémités sont des
//!    demi-disques » se mesurerait sur un trait d'un pixel où plus rien ne distingue un demi-disque
//!    d'un carré.
//! 3. **La roundeur des terminaisons se mesure sur le `I` capitale**, un fût vertical nu dans les
//!    deux fontes en présence — sans empattement chez Liberation Sans, sans panse ni queue chez
//!    Nunito. C'est le seul glyphe dont la coupe se lit sans démêler une courbe de la terminaison
//!    elle-même.
//! 4. **Le nom de la fonte n'est pas vérifié.** L'issue nomme Nunito, mais ce qu'elle veut est une
//!    propriété — des terminaisons arrondies —, et une autre fonte ronde y répondrait aussi bien.
//!    Vérifier « Nunito » dans la table `name` ferait échouer un choix meilleur, et forcerait
//!    précisément la relecture de test que le workflow interdit.
//!
//! # Les trois pièges que ce fichier garde
//!
//! 1. **« Les bords sont lissés » passe sur un arc entièrement translucide.** Une implémentation qui
//!    mélangerait l'arc au fond partout aurait des dizaines de nuances et un rendu délavé. Le test
//!    exige donc les **deux** : des nuances **aux bords**, et un **cœur à pleine couleur** couvrant
//!    l'essentiel du remplissage.
//! 2. **« Les bouts sont arrondis » passe sur un arc qu'on aurait simplement raccourci.** Un compte
//!    global de pixels ne les distingue pas. Ce qui les distingue, c'est le **profil de portée à
//!    travers l'épaisseur** : un demi-disque va plus loin au milieu de l'épaisseur qu'à ses deux
//!    coins, un bout carré va aussi loin partout. C'est cette différence-là qui est mesurée, en
//!    pixels de longueur d'arc, aux deux extrémités.
//! 3. **« La fonte est ronde » n'est pas un jugement de goût.** Mesuré sur le tampon, hors de tout
//!    avis : au bout du fût, un demi-disque **retire de l'encre aux coins et pas au milieu**, et la
//!    dernière ligne encrée porte bien moins d'encre que le fût. Une terminaison droite finit par
//!    une ligne pleine jusqu'aux coins — relevé sur les deux fontes du dépôt avant d'écrire ce
//!    fichier, pour que les seuils viennent d'une mesure et non d'une intuition.
//!
//! # Ce que ce fichier ne teste pas
//!
//! - **Le débordement hors du disque** : `spec_police_arcs.rs` le couvre déjà pour `Dalle::arc`, et
//!   la piste, qui vit dans la même couronne, y passe avec. Un seul contrôle est ajouté ici, sur
//!   l'arc **vide** — la seule proportion que #90 n'essaie pas, et justement celle où la piste est
//!   seule à peindre.
//! - **La taille du binaire** avant/après : une mesure à rapporter dans l'issue, pas une assertion.
//! - `clippy` et `fmt` : ils ont leur propre commande.
//! - Aucune écriture matérielle, aucun accès à `/dev`, aucun fichier, aucun démon lancé.

// `ARC_COULEUR` et `TEMOIN` sont deux constantes, et clippy refuse qu'on les compare dans un
// `assert!` à ce titre. L'intérêt de cette comparaison n'est pas d'observer une exécution mais de
// casser la compilation le jour où la couleur d'arc rejoindrait le témoin sur lequel tout ce fichier
// mesure. Même `allow` que `spec_police_arcs.rs`, pour la même raison.
#![allow(clippy::assertions_on_constants)]

use std::collections::BTreeSet;

use reverb_daemon::ecran::{ARC_COULEUR, Dalle};
use reverb_proto::composition::{self, Ancre, Secteur};
use reverb_proto::screen;

// ---------------------------------------------------------------------------
// Témoins
// ---------------------------------------------------------------------------

/// La largeur de la dalle. Reprise du protocole, jamais réécrite.
const LARGEUR: u32 = screen::WIDTH as u32;

/// La hauteur de la dalle.
const HAUTEUR: u32 = screen::HEIGHT as u32;

/// La couleur témoin du projet. Ses trois composantes sont distinctes, donc aucune permutation ne
/// peut passer inaperçue.
const TEMOIN: (u8, u8, u8) = (0xff, 0x20, 0x80);

/// Les quatre ancres qui portent un arc. `Centre` n'est pas sur la couronne (#90).
const ANCRES_DE_COURONNE: [Ancre; 4] = [Ancre::Haut, Ancre::Droite, Ancre::Bas, Ancre::Gauche];

/// L'épaisseur de la couronne, en pixels.
fn epaisseur_de_la_couronne() -> f64 {
    f64::from(composition::COURONNE_RAYON_EXTERIEUR)
        - f64::from(composition::COURONNE_RAYON_INTERIEUR)
}

// ---------------------------------------------------------------------------
// Lecture d'une dalle
//
// Ces aides sont celles de `spec_police_arcs.rs`, recopiées : deux tests d'intégration sont deux
// binaires distincts, et rien ne se partage entre eux sans passer par le code du crate — ce qu'un
// test d'intention ne fait pas.
// ---------------------------------------------------------------------------

/// Les octets bruts du pixel (`x`, `y`), dans l'ordre où ils partiront sur le bus.
fn triplet(dalle: &Dalle, x: u32, y: u32) -> [u8; screen::PIXEL_LEN] {
    let octets = dalle.octets();
    let debut = (y as usize * LARGEUR as usize + x as usize) * screen::PIXEL_LEN;
    octets[debut..debut + screen::PIXEL_LEN]
        .try_into()
        .expect("un pixel fait screen::PIXEL_LEN octets")
}

/// Les octets qu'une couleur donne une fois écrite dans le tampon.
///
/// ⚠️ **Obtenus en peignant la dalle, jamais recopiés.** L'écran est en BGR, les LED en GRB et la
/// RAM en RGB : un test qui écrirait l'ordre à la main serait un quatrième endroit du projet à
/// connaître celui de l'écran, et le premier à pouvoir se tromper sans que rien ne le dise.
fn octets_de(couleur: (u8, u8, u8)) -> [u8; screen::PIXEL_LEN] {
    triplet(&Dalle::unie(couleur), 0, 0)
}

/// L'écart entre deux pixels, dans l'espace des trois octets.
///
/// La distance euclidienne, et non la moyenne : deux couleurs différentes peuvent avoir la même
/// moyenne, et une encre choisie sans y penser rendrait alors le texte invisible à la mesure alors
/// qu'il se voit à l'œil.
fn ecart(a: [u8; screen::PIXEL_LEN], b: [u8; screen::PIXEL_LEN]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(gauche, droite)| {
            let delta = f64::from(*gauche) - f64::from(*droite);
            delta * delta
        })
        .sum::<f64>()
        .sqrt()
}

/// Les pixels qui diffèrent entre deux dalles.
fn pixels_changes(avant: &Dalle, apres: &Dalle) -> Vec<(u32, u32)> {
    let gauche = avant.octets().chunks_exact(screen::PIXEL_LEN);
    let droite = apres.octets().chunks_exact(screen::PIXEL_LEN);
    let mut changes = Vec::new();
    for (rang, (pixel_avant, pixel_apres)) in gauche.zip(droite).enumerate() {
        if pixel_avant != pixel_apres {
            let rang = u32::try_from(rang).expect("un tampon de 640 × 640 tient dans un u32");
            changes.push((rang % LARGEUR, rang / LARGEUR));
        }
    }
    changes
}

/// Les pixels peints **exactement** de la couleur d'arc.
///
/// C'est la mesure que les critères de #93 nomment : « aucun pixel de couleur d'arc » pour un arc
/// vide, « mesuré sur les pixels de la couleur d'arc » pour la croissance du remplissage. Elle
/// ignore la piste par construction, et elle ignore aussi l'anticrénelage des bords — ce qui en fait
/// une mesure **conservatrice**, jamais gonflée par le lissage qu'on vient d'ajouter.
fn pixels_d_arc(dalle: &Dalle) -> Vec<(u32, u32)> {
    let encre = octets_de(ARC_COULEUR);
    let mut trouves = Vec::new();
    for (rang, pixel) in dalle.octets().chunks_exact(screen::PIXEL_LEN).enumerate() {
        if pixel == encre {
            let rang = u32::try_from(rang).expect("un tampon de 640 × 640 tient dans un u32");
            trouves.push((rang % LARGEUR, rang / LARGEUR));
        }
    }
    trouves
}

/// Vérifie qu'une dalle a exactement la taille qu'attend le contrôleur.
///
/// Une dalle courte est **ignorée en silence** par le matériel (spec §2.2.1).
fn dalle_bien_dimensionnee(dalle: &Dalle, quoi: &str) {
    assert_eq!(
        dalle.octets().len(),
        screen::IMAGE_LEN,
        "{quoi} doit faire screen::IMAGE_LEN octets — une dalle courte est ignorée par le \
         contrôleur sans le moindre code d'erreur"
    );
}

// ---------------------------------------------------------------------------
// Géométrie polaire
//
// Même convention que `spec_police_arcs.rs` : **0° au sommet, croissant dans le sens horaire**.
// C'est celle que `reverb_proto::composition::Secteur` documente.
// ---------------------------------------------------------------------------

/// La distance du centre de la dalle au centre du pixel (`x`, `y`).
fn distance_au_centre(x: u32, y: u32) -> f64 {
    let centre = f64::from(LARGEUR) / 2.0;
    (f64::from(x) + 0.5 - centre).hypot(f64::from(y) + 0.5 - centre)
}

/// L'angle d'un pixel vu du centre de la dalle, en degrés dans `[0, 360)`.
fn angle_du_pixel(x: u32, y: u32) -> f64 {
    let centre = f64::from(LARGEUR) / 2.0;
    let dx = f64::from(x) + 0.5 - centre;
    let dy = f64::from(y) + 0.5 - centre;
    dx.atan2(-dy).to_degrees().rem_euclid(360.0)
}

/// Le pixel qui contient le point polaire (`rayon`, `angle`).
///
/// L'inverse de [`angle_du_pixel`] et [`distance_au_centre`] — c'est ce qui permet de **suivre**
/// l'arc au lieu de balayer le tampon : un demi-disque de douze pixels de rayon se mesure en
/// dixièmes de degré, pas en pixels entiers.
fn pixel_polaire(rayon: f64, angle: f64) -> (u32, u32) {
    let centre = f64::from(LARGEUR) / 2.0;
    let radians = angle.to_radians();
    let x = (centre + rayon * radians.sin()).floor();
    let y = (centre - rayon * radians.cos()).floor();
    (
        x.clamp(0.0, f64::from(LARGEUR - 1)) as u32,
        y.clamp(0.0, f64::from(HAUTEUR - 1)) as u32,
    )
}

/// L'angle d'un pixel **rapporté au début d'un secteur**.
///
/// Un secteur peut enjamber 0° — celui de `haut` le fait —, et comparer des angles absolus s'y
/// casse. Une fenêtre de 45° avant le début est rendue en négatif : c'est là que vit le demi-disque
/// d'une extrémité arrière.
fn relatif_au_secteur(angle: f64, secteur: Secteur) -> f64 {
    let brut = (angle - f64::from(secteur.debut)).rem_euclid(360.0);
    if brut > 315.0 { brut - 360.0 } else { brut }
}

/// Le secteur d'une ancre, ou l'échec du test.
fn secteur_de(ancre: Ancre) -> Secteur {
    ancre
        .secteur()
        .unwrap_or_else(|| panic!("{ancre:?} est sur la couronne et doit avoir un secteur"))
}

/// Vrai si le carré du pixel (`x`, `y`) est **entièrement** hors du disque visible.
fn entierement_hors_du_disque(x: u32, y: u32) -> bool {
    let centre = f64::from(LARGEUR) / 2.0;
    let proche_x = centre.clamp(f64::from(x), f64::from(x + 1));
    let proche_y = centre.clamp(f64::from(y), f64::from(y + 1));
    let dx = centre - proche_x;
    let dy = centre - proche_y;
    let rayon = f64::from(screen::VISIBLE_DISC_RADIUS);
    dx * dx + dy * dy >= rayon * rayon
}

// ---------------------------------------------------------------------------
// Mesures sur un arc
// ---------------------------------------------------------------------------

/// Les rayons extrêmes où l'arc est **à pleine couleur**, à cet angle relatif au secteur.
///
/// Mesurés, jamais supposés : l'issue ne dit pas que l'arc remplit toute la couronne, et la piste,
/// elle, l'occupe. Tout ce qui suit — le milieu de l'épaisseur, ses deux coins, la profondeur du
/// demi-disque — se déduit de cette mesure, de sorte qu'un arc plus fin que sa couronne reste
/// mesurable sans qu'un seul seuil ne bouge.
fn rayons_de_l_arc(dalle: &Dalle, secteur: Secteur, angle_relatif: f64) -> Option<(f64, f64)> {
    let encre = octets_de(ARC_COULEUR);
    let angle = f64::from(secteur.debut) + angle_relatif;
    let mut dedans = f64::INFINITY;
    let mut dehors = f64::NEG_INFINITY;
    let mut rayon = f64::from(composition::COURONNE_RAYON_INTERIEUR) - 8.0;
    while rayon <= f64::from(composition::COURONNE_RAYON_EXTERIEUR) + 8.0 {
        let (x, y) = pixel_polaire(rayon, angle);
        if triplet(dalle, x, y) == encre {
            dedans = dedans.min(rayon);
            dehors = dehors.max(rayon);
        }
        rayon += 0.25;
    }
    (dedans <= dehors).then_some((dedans, dehors))
}

/// À quel angle relatif l'arc, à ce rayon, cesse d'être à pleine couleur — vers l'avant.
///
/// Le pas est de 0,05°, soit un quart de pixel à trois cents de rayon : la mesure a de la marge sous
/// le pixel, ce qu'il faut pour comparer deux portées qui ne diffèrent que de trois pixels.
fn portee_avant(dalle: &Dalle, secteur: Secteur, rayon: f64) -> Option<f64> {
    let encre = octets_de(ARC_COULEUR);
    let mut trouvee: Option<f64> = None;
    let mut relatif = -8.0f64;
    while relatif <= f64::from(secteur.ouverture) + 8.0 {
        let (x, y) = pixel_polaire(rayon, f64::from(secteur.debut) + relatif);
        if triplet(dalle, x, y) == encre {
            trouvee = Some(relatif);
        }
        relatif += 0.05;
    }
    trouvee
}

/// La même, vers l'arrière : le plus petit angle relatif encore à pleine couleur.
fn portee_arriere(dalle: &Dalle, secteur: Secteur, rayon: f64) -> Option<f64> {
    let encre = octets_de(ARC_COULEUR);
    let mut relatif = -8.0f64;
    while relatif <= f64::from(secteur.ouverture) + 8.0 {
        let (x, y) = pixel_polaire(rayon, f64::from(secteur.debut) + relatif);
        if triplet(dalle, x, y) == encre {
            return Some(relatif);
        }
        relatif += 0.05;
    }
    None
}

/// Dans une fenêtre angulaire et radiale donnée : combien de pixels y sont, combien sont d'arc.
fn part_d_arc(
    dalle: &Dalle,
    secteur: Secteur,
    rayons: (f64, f64),
    angles: (f64, f64),
) -> (usize, usize) {
    let encre = octets_de(ARC_COULEUR);
    let (mut total, mut peints) = (0usize, 0usize);
    for y in 0..HAUTEUR {
        for x in 0..LARGEUR {
            let rayon = distance_au_centre(x, y);
            if rayon < rayons.0 || rayon > rayons.1 {
                continue;
            }
            let relatif = relatif_au_secteur(angle_du_pixel(x, y), secteur);
            if relatif < angles.0 || relatif > angles.1 {
                continue;
            }
            total += 1;
            if triplet(dalle, x, y) == encre {
                peints += 1;
            }
        }
    }
    (peints, total)
}

// ---------------------------------------------------------------------------
// 1 — les bords sont lissés, et le cœur reste à pleine couleur
// ---------------------------------------------------------------------------

#[test]
fn les_bords_d_un_arc_sont_lisses_et_son_coeur_reste_a_pleine_couleur() {
    // Critère d'acceptation : « les bords d'un arc portent au moins quatre nuances entre la couleur
    // d'arc et ce qu'il y a dessous ».
    //
    // Ce qu'il empêche de revenir : le `Toile::arc` en tout ou rien de #90, qui teste chaque pixel
    // « dans le secteur ou pas » et rend des bords radiaux **en escalier**. Sur une dalle de six
    // centimètres regardée de près, l'escalier est la première chose qu'on voit.
    //
    // Piège n° 1 du préambule. « Des nuances » passe aussi sur un arc **entièrement** translucide,
    // qui n'aurait plus d'escalier et n'aurait plus de couleur non plus. D'où les deux moitiés de ce
    // test : des nuances aux bords, et un cœur qui n'en a aucune.
    assert!(
        ARC_COULEUR != TEMOIN,
        "la couleur d'arc ne peut pas être celle du fond témoin de ce fichier : tout y est mesuré \
         par différence, et un arc invisible passerait tout"
    );

    let fond = Dalle::unie(TEMOIN);
    let encre = octets_de(ARC_COULEUR);
    let proportion = 0.6f32;

    for ancre in ANCRES_DE_COURONNE {
        let secteur = secteur_de(ancre);
        let ouverture = f64::from(secteur.ouverture);
        let vide = Dalle::arc(&fond, secteur, 0.0);
        let rempli = Dalle::arc(&fond, secteur, proportion);
        dalle_bien_dimensionnee(
            &rempli,
            &format!("l'arc de {ancre:?} rempli à {proportion}"),
        );

        // Le remplissage seul : ce qui distingue l'arc de sa piste. La piste est identique dans les
        // deux dalles et disparaît donc de la mesure, sans qu'on ait à connaître sa couleur.
        let remplissage = pixels_changes(&vide, &rempli);
        assert!(
            remplissage.len() >= 500,
            "{ancre:?} : un arc à {proportion} doit peindre la couronne. Obtenu {} pixel(s)",
            remplissage.len()
        );

        let nuances: BTreeSet<[u8; screen::PIXEL_LEN]> = remplissage
            .iter()
            .map(|&(x, y)| triplet(&rempli, x, y))
            .filter(|couleur| *couleur != encre)
            .collect();
        assert!(
            nuances.len() >= 4,
            "{ancre:?} : les bords de l'arc doivent porter au moins quatre nuances entre la couleur \
             d'arc et ce qu'il y a dessous. Obtenu {} — une seule, c'est le tout ou rien de #90, et \
             ses bords radiaux en escalier",
            nuances.len()
        );

        // Le cœur, lui, est à pleine couleur. Sans cette assertion, un arc mélangé au fond partout
        // — délavé, exactement ce que l'issue ne demande pas — aurait des dizaines de nuances et
        // passerait la précédente les yeux fermés.
        let pleins = remplissage
            .iter()
            .filter(|&&(x, y)| triplet(&rempli, x, y) == encre)
            .count();
        assert!(
            pleins >= 200,
            "{ancre:?} : l'arc a un cœur à pleine couleur. Obtenu {pleins} pixel(s) exactement de \
             la couleur d'arc"
        );
        let part = pleins as f64 / remplissage.len() as f64;
        assert!(
            part >= 0.6,
            "{ancre:?} : {:.0} % du remplissage est à pleine couleur, pour 60 % attendus — un arc \
             translucide de bout en bout n'est pas un arc lissé, c'est un arc délavé",
            part * 100.0
        );

        // Et le lissage est bien celui des **extrémités**, pas seulement des deux bords radiaux.
        // Trois pixels à l'intérieur des rayons extrêmes de l'arc, les seules nuances qui restent
        // viennent forcément des deux bouts : un demi-disque coupé à l'équerre n'en laisserait
        // aucune.
        let (dedans, dehors) =
            rayons_de_l_arc(&rempli, secteur, 0.5 * f64::from(proportion) * ouverture)
                .unwrap_or_else(|| {
                    panic!("{ancre:?} : l'arc doit être à pleine couleur en son milieu")
                });
        let nuances_des_bouts: BTreeSet<[u8; screen::PIXEL_LEN]> = remplissage
            .iter()
            .filter(|&&(x, y)| {
                let rayon = distance_au_centre(x, y);
                rayon >= dedans + 3.0 && rayon <= dehors - 3.0
            })
            .map(|&(x, y)| triplet(&rempli, x, y))
            .filter(|couleur| *couleur != encre)
            .collect();
        assert!(
            nuances_des_bouts.len() >= 4,
            "{ancre:?} : loin des bords radiaux, les nuances ne peuvent venir que des deux \
             extrémités. Obtenu {} pour 4 attendues — des bouts coupés à l'équerre n'en laissent \
             aucune",
            nuances_des_bouts.len()
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — les deux extrémités sont des demi-disques
// ---------------------------------------------------------------------------

#[test]
fn les_deux_extremites_d_un_arc_sont_des_demi_disques() {
    // Critère d'acceptation : « les extrémités d'un arc sont arrondies : le coin d'un bout est plus
    // clair que son milieu ». Comportement à tester n° 2 : « à épaisseur égale, un bout rond peint
    // moins de pixels qu'un bout carré ».
    //
    // Ce qu'il empêche de revenir : les deux extrémités **coupées à l'équerre** de #90, qui donnent
    // à l'arc l'allure d'un segment découpé aux ciseaux plutôt que d'une jauge.
    //
    // Piège n° 2 du préambule, et c'est lui qui décide de la forme de ce test. « Le bout est rond »
    // passerait sur un arc simplement raccourci : le compte de pixels baisse, et rien n'est arrondi.
    // Ce qu'on mesure est donc un **contraste à l'intérieur du bout** — jusqu'où l'arc va au milieu
    // de son épaisseur, jusqu'où il va à ses deux coins. Un demi-disque de rayon `h` va `h` plus
    // loin au milieu ; à 15 % de l'épaisseur d'un bord, il n'en fait plus que `0,71 h`, soit un
    // recul de `0,29 h` — trois pixels et demi pour la couronne de #90. Un bout carré, lui, va aussi
    // loin partout : son recul est nul, quelle que soit sa longueur.
    let fond = Dalle::unie(TEMOIN);
    let proportion = 0.6f32;
    let recul_minimum = 1.5f64;

    for ancre in ANCRES_DE_COURONNE {
        let secteur = secteur_de(ancre);
        let ouverture = f64::from(secteur.ouverture);
        let rempli = Dalle::arc(&fond, secteur, proportion);

        // L'épaisseur de l'arc est **relevée**, au milieu du remplissage, là où aucune extrémité
        // n'interfère.
        let milieu_du_remplissage = 0.5 * f64::from(proportion) * ouverture;
        let (dedans, dehors) = rayons_de_l_arc(&rempli, secteur, milieu_du_remplissage)
            .unwrap_or_else(|| {
                panic!("{ancre:?} : l'arc doit être à pleine couleur en son milieu")
            });
        let epaisseur = dehors - dedans;
        assert!(
            epaisseur >= 0.4 * epaisseur_de_la_couronne(),
            "{ancre:?} : l'arc fait {epaisseur:.1} px d'épaisseur pour une couronne de {:.0} — \
             sous 40 %, plus rien ne distingue un demi-disque d'un bout carré, et la jauge se lit \
             comme un trait",
            epaisseur_de_la_couronne()
        );

        let mi_rayon = (dedans + dehors) / 2.0;
        let coins = [
            ("intérieur", dedans + 0.15 * epaisseur),
            ("extérieur", dehors - 0.15 * epaisseur),
        ];

        // Le bout avant.
        let avant_au_milieu = portee_avant(&rempli, secteur, mi_rayon)
            .unwrap_or_else(|| panic!("{ancre:?} : l'arc doit atteindre son mi-rayon"));
        for (ou, rayon) in coins {
            let avant_au_coin = portee_avant(&rempli, secteur, rayon).unwrap_or_else(|| {
                panic!("{ancre:?} : l'arc doit exister au coin {ou} de son épaisseur")
            });
            let recul = (avant_au_milieu - avant_au_coin).to_radians() * mi_rayon;
            assert!(
                recul >= recul_minimum,
                "{ancre:?}, bout avant : le coin {ou} recule de {recul:.1} px derrière le milieu de \
                 l'épaisseur, pour {recul_minimum} au moins. Un demi-disque en reculerait de trois \
                 et demi ; un bout coupé à l'équerre, de zéro — c'est exactement ce qui se mesure ici"
            );
        }

        // Le bout arrière. L'issue dit « ses deux extrémités », et un arc dont seul le bout mobile
        // serait arrondi aurait une origine à l'équerre et une fin ronde : deux styles sur le même
        // objet, ce que #93 vient précisément corriger.
        let arriere_au_milieu = portee_arriere(&rempli, secteur, mi_rayon)
            .unwrap_or_else(|| panic!("{ancre:?} : l'arc doit atteindre son mi-rayon"));
        for (ou, rayon) in coins {
            let arriere_au_coin = portee_arriere(&rempli, secteur, rayon).unwrap_or_else(|| {
                panic!("{ancre:?} : l'arc doit exister au coin {ou} de son épaisseur")
            });
            let recul = (arriere_au_coin - arriere_au_milieu).to_radians() * mi_rayon;
            assert!(
                recul >= recul_minimum,
                "{ancre:?}, bout arrière : le coin {ou} recule de {recul:.1} px derrière le milieu \
                 de l'épaisseur, pour {recul_minimum} au moins"
            );
        }

        // Et « moins de pixels qu'un bout carré », à la même étendue angulaire. La fenêtre est la
        // moitié de la profondeur du demi-disque : un bout carré y remplirait toute l'épaisseur, un
        // demi-disque n'en remplit que 61 % — c'est l'aire d'un segment circulaire de hauteur `h/2`,
        // calculée et non estimée. Le seuil est posé entre les deux.
        let profondeur = epaisseur / 4.0;
        let fenetre = (profondeur / mi_rayon).to_degrees();
        for (quoi, bornes) in [
            ("avant", (avant_au_milieu - fenetre, avant_au_milieu)),
            ("arrière", (arriere_au_milieu, arriere_au_milieu + fenetre)),
        ] {
            let (peints, total) = part_d_arc(&rempli, secteur, (dedans, dehors), bornes);
            assert!(
                total >= 40,
                "{ancre:?}, bout {quoi} : {total} pixel(s) dans la fenêtre terminale, trop peu pour \
                 que la proportion veuille dire quelque chose"
            );
            let part = peints as f64 / total as f64;
            assert!(
                part <= 0.8,
                "{ancre:?}, bout {quoi} : {:.0} % de la fenêtre terminale est peinte, pour 80 % au \
                 plus. Un demi-disque en couvre 61 %, un bout carré la totalité",
                part * 100.0
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — la piste couvre l'ouverture entière, et un arc vide ne montre qu'elle
// ---------------------------------------------------------------------------

#[test]
fn la_piste_couvre_l_ouverture_entiere_et_un_arc_vide_ne_montre_qu_elle() {
    // Critères d'acceptation : « la piste couvre l'ouverture entière du secteur, et l'arc se
    // distingue d'elle » · « un arc vide montre sa piste et aucun pixel de couleur d'arc ».
    //
    // Ce qu'il empêche de revenir : l'arc sans piste de #90, où un remplissage à 20 % se lit comme
    // « une petite barre » et non comme « un cinquième de quelque chose ». Sans piste, la seule
    // façon de savoir où l'arc s'arrêterait à 100 % est de l'avoir déjà vu plein.
    //
    // ⚠️ Le contraire est aussi à garder : une piste qui garderait un reste de couleur d'arc à
    // proportion nulle afficherait une valeur qui n'existe pas, et c'est le figement que #68
    // interdit — un arc qui ne descend jamais tout à fait à zéro derrière une pompe arrêtée.
    let fond = Dalle::unie(TEMOIN);

    for ancre in ANCRES_DE_COURONNE {
        let secteur = secteur_de(ancre);
        let ouverture = f64::from(secteur.ouverture);
        let vide = Dalle::arc(&fond, secteur, 0.0);
        let plein = Dalle::arc(&fond, secteur, 1.0);
        dalle_bien_dimensionnee(&vide, &format!("l'arc vide de {ancre:?}"));

        // Aucun pixel de couleur d'arc, sur **toute** la dalle et pas seulement dans la couronne.
        let restes = pixels_d_arc(&vide);
        assert!(
            restes.is_empty(),
            "{ancre:?} : un arc vide ne porte aucun pixel de couleur d'arc. Obtenu {} — une jauge \
             qui ne retombe pas à zéro est rassurante et fausse",
            restes.len()
        );

        // La piste, elle, est là : l'arc vide change le fond.
        let piste = pixels_changes(&fond, &vide);
        assert!(
            piste.len() >= 500,
            "{ancre:?} : un arc vide montre sa piste. Obtenu {} pixel(s) changés — sans piste, un \
             arc à 20 % se lit comme une petite barre et non comme un cinquième",
            piste.len()
        );

        // Elle tient dans le disque visible. C'est la seule proportion que #90 n'essaie pas, et
        // justement celle où la piste peint seule (`SPEC-KRAKEN-LCD` §2.1.1).
        for &(x, y) in &piste {
            assert!(
                !entierement_hors_du_disque(x, y),
                "{ancre:?} : la piste peint le pixel ({x}, {y}), entièrement hors du disque \
                 visible — la dalle est ronde, et le contrôleur ne dira jamais rien"
            );
        }

        // Elle couvre **l'ouverture entière**, mesurée au rayon où l'arc lui-même vit. La marge
        // laissée aux deux extrémités est la demi-épaisseur exprimée en degrés : c'est là que vivent
        // les demi-disques, et ce test ne juge pas leur forme, seulement la longueur de la piste.
        let (dedans, dehors) = rayons_de_l_arc(&plein, secteur, 0.5 * ouverture)
            .unwrap_or_else(|| panic!("{ancre:?} : l'arc plein doit être à pleine couleur"));
        let mi_rayon = (dedans + dehors) / 2.0;
        let marge = (((dehors - dedans) / 2.0) / mi_rayon).to_degrees() + 0.5;
        let mut relatif = marge;
        while relatif <= ouverture - marge {
            let (x, y) = pixel_polaire(mi_rayon, f64::from(secteur.debut) + relatif);
            assert_ne!(
                triplet(&vide, x, y),
                triplet(&fond, x, y),
                "{ancre:?} : la piste manque à {relatif:.1}° de l'ouverture, au pixel ({x}, {y}). \
                 Elle couvre le secteur **entier**, sinon elle ne dit pas de quoi l'arc est la part"
            );
            // Et l'arc s'en distingue : au même endroit, l'arc plein ne montre pas la piste.
            assert_ne!(
                triplet(&plein, x, y),
                triplet(&vide, x, y),
                "{ancre:?} : à {relatif:.1}°, l'arc plein doit se distinguer de la piste — une \
                 piste de la couleur de l'arc rendrait toute jauge illisible"
            );
            relatif += 0.5;
        }

        // La piste est bien **sous** l'arc, et non à côté : un arc à moitié rempli laisse voir la
        // piste dans sa seconde moitié, à l'endroit exact où l'arc plein est peint.
        let moitie = Dalle::arc(&fond, secteur, 0.5);
        let (x, y) = pixel_polaire(mi_rayon, f64::from(secteur.debut) + 0.8 * ouverture);
        assert_eq!(
            triplet(&moitie, x, y),
            triplet(&vide, x, y),
            "{ancre:?} : aux quatre cinquièmes de l'ouverture, un arc à moitié rempli montre sa \
             piste, la même qu'un arc vide"
        );
        assert_ne!(
            triplet(&moitie, x, y),
            triplet(&plein, x, y),
            "{ancre:?} : au même endroit, l'arc plein est peint — sinon la piste et l'arc \
             occuperaient deux places différentes, et la jauge ne serait plus une jauge"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — le remplissage croît strictement, mesuré sur la couleur d'arc
// ---------------------------------------------------------------------------

#[test]
fn le_remplissage_croit_strictement_sur_les_pixels_de_la_couleur_d_arc() {
    // Critère d'acceptation : « le remplissage reste **strictement croissant** de 0 à 100 %, mesuré
    // sur les pixels de la couleur d'arc, la piste ne comptant pas ».
    //
    // Ce qu'il empêche de revenir : une piste qui, en couvrant tout le secteur quelle que soit la
    // proportion, ferait passer pour identiques trois arcs qui ne le sont pas. C'est la mesure que
    // #93 substitue au comptage des pixels changés — et c'est aussi pourquoi
    // `spec_police_arcs.rs::un_arc_se_remplit_proportionnellement_du_vide_au_plein` a été relu.
    let fond = Dalle::unie(TEMOIN);

    for ancre in ANCRES_DE_COURONNE {
        let secteur = secteur_de(ancre);
        let mut precedent: Option<(f32, usize)> = None;

        for proportion in [0.0f32, 0.2, 0.4, 0.6, 0.8, 1.0] {
            let dalle = Dalle::arc(&fond, secteur, proportion);
            dalle_bien_dimensionnee(&dalle, &format!("l'arc de {ancre:?} à {proportion}"));
            let compte = pixels_d_arc(&dalle).len();

            if proportion == 0.0 {
                assert_eq!(
                    compte, 0,
                    "{ancre:?} : à 0 %, aucun pixel de couleur d'arc — la piste ne compte pas"
                );
            }
            if let Some((avant, compte_avant)) = precedent {
                assert!(
                    compte > compte_avant,
                    "{ancre:?} : le remplissage doit croître **strictement** — {compte_avant} \
                     pixel(s) d'arc à {avant}, {compte} à {proportion}. Un compte qui stagne, c'est \
                     une jauge qui ment sur la moitié de son échelle"
                );
            }
            precedent = Some((proportion, compte));
        }

        let plein = pixels_d_arc(&Dalle::arc(&fond, secteur, 1.0)).len();
        assert!(
            plein >= 200,
            "{ancre:?} : un arc plein doit peindre la couronne. Obtenu {plein} pixel(s) de couleur \
             d'arc — un arc qu'on ne voit pas ne dit ni la valeur ni où elle se situe"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — deux arcs voisins ne partagent aucun pixel, piste comprise
// ---------------------------------------------------------------------------

#[test]
fn deux_arcs_voisins_ne_partagent_aucun_pixel_piste_comprise() {
    // Critère d'acceptation : « deux arcs voisins ne partagent aucun pixel, **piste comprise** ».
    //
    // Ce qu'il empêche de revenir : #90 vérifiait la disjonction sur des arcs pleins, où seul le
    // remplissage peint. La piste peint désormais **quelle que soit la proportion**, et deux pistes
    // jointives feraient un anneau continu où on ne verrait plus où finit une sonde et où commence
    // sa voisine — précisément la lecture d'un coup d'œil que la couronne sert à rendre possible.
    //
    // Le cas décisif est celui des arcs **vides** : c'est là que la piste est seule, et c'est le seul
    // état que #90 n'a jamais mis face à un voisin.
    let fond = Dalle::unie(TEMOIN);

    for proportion in [0.0f32, 0.5, 1.0] {
        let peints: Vec<(Ancre, BTreeSet<(u32, u32)>)> = ANCRES_DE_COURONNE
            .iter()
            .map(|&ancre| {
                let dalle = Dalle::arc(&fond, secteur_de(ancre), proportion);
                (
                    ancre,
                    pixels_changes(&fond, &dalle)
                        .into_iter()
                        .collect::<BTreeSet<_>>(),
                )
            })
            .collect();

        for (rang, (a, pixels_a)) in peints.iter().enumerate() {
            assert!(
                !pixels_a.is_empty(),
                "{a:?} à {proportion} doit peindre quelque chose, sinon la disjonction ne prouve rien"
            );
            for (b, pixels_b) in &peints[rang + 1..] {
                let partages = pixels_a.intersection(pixels_b).count();
                assert_eq!(
                    partages, 0,
                    "{a:?} et {b:?} partagent {partages} pixel(s) à {proportion} — piste comprise, \
                     deux secteurs jointifs se lisent comme un seul"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 6 — la fonte a des terminaisons arrondies
// ---------------------------------------------------------------------------

#[test]
fn les_glyphes_de_la_fonte_portent_des_terminaisons_arrondies() {
    // Comportement attendu de l'issue : « les textes sont écrits dans une fonte à terminaisons
    // arrondies ».
    //
    // Ce qu'il empêche de revenir : `LiberationSans-Bold`, métriquement compatible Arial —
    // terminaisons droites, contreformes carrées, dessin de 1982. C'est la seconde des deux causes
    // que l'issue nomme, et la seule que le protocole ne pouvait pas signaler.
    //
    // Piège n° 3 du préambule : ce test **ne juge rien**. Il mesure deux choses sur le fût du `I`
    // capitale, relevées sur les deux fontes du dépôt avant d'écrire ce fichier — sept tailles de
    // rendu pour Nunito, trois pour Liberation, dont celle à laquelle la dalle écrit vraiment :
    //
    // | | Liberation Sans Bold | Nunito Bold |
    // |---|---|---|
    // | encre de la ligne extrême, rapportée à la pleine largeur du fût | **1,00** en bas | 0,05 à 0,45 |
    // | ce qu'une colonne garde au bout, rapporté à ce qu'elle porte à mi-fût — **au milieu** de l'épaisseur | 0,52 à 1,00 | 0,87 à 1,00 |
    // | la même chose, **au bord** | 0,52 à 1,00 | 0,00 à 0,49 |
    //
    // Dix tailles de rendu pour chacune, de 40 à 96 pixels : Nunito passe aux dix, Liberation échoue
    // aux dix, aux deux bouts et sur les deux bords. Le 0,49 est un cas isolé — 40 px, un fût de six
    // pixels dont la colonne gauche tombe presque pleine —, et il fixe le seuil à 0,6 plutôt qu'à
    // 0,5 : une coupe droite vaut 1,00 quelle que soit la taille, il reste donc 40 % de marge du
    // côté qu'on veut refuser et 22 % du côté qu'on veut accepter. À la taille où la dalle écrit
    // vraiment — un fût de onze pixels —, le bord tombe entre 0,00 et 0,15.
    //
    // Une terminaison droite finit par une ligne pleine **jusqu'aux coins** ; une terminaison ronde
    // vide les coins d'abord et ne garde d'encre qu'au milieu de l'épaisseur. Les trois mesures sont
    // sans unité, donc indépendantes de la taille à laquelle la fonte est rendue.
    //
    // ⚠️ **C'est le rapport entre les deux dernières lignes qui décide, jamais leur valeur.** Le bout
    // d'un fût coupé droit ne tombe pas toujours sur une frontière de pixel : sa dernière ligne peut
    // n'être couverte qu'à 55 % — mesuré sur Liberation —, et ses trois colonnes le sont alors
    // **également**. Une valeur absolue au bord y verrait un coin vidé sans que rien ne soit
    // arrondi ; le rapport au milieu de la même ligne, lui, vaut 1,00 sur une coupe droite quelle
    // que soit la taille, et au plus 0,14 sur Nunito.
    //
    // ⚠️ **La première mesure ne suffirait pas non plus.** Le bout **haut** d'un fût coupé droit
    // laisse, à certaines tailles, une ligne d'anticrénelage qui porte 9 % de l'encre du fût et la
    // satisferait — mesuré sur Liberation à 67 px. Le bout bas, lui, la fait échouer à toutes les
    // tailles. Elle reste une exigence utile ; elle n'est pas le garde-fou.
    //
    // Le `I` capitale est choisi parce qu'il est un fût vertical **nu** dans les deux fontes : ni
    // empattement chez Liberation, ni panse ni queue chez Nunito. Sur un `l` de Nunito, la queue
    // recourbée du bas ferait passer le test pour de mauvaises raisons.
    let fond = Dalle::unie(TEMOIN);
    let boite = Ancre::Haut.boite();
    let ecrit = Dalle::texte(&fond, "I", boite);
    dalle_bien_dimensionnee(&ecrit, "le fût témoin écrit dans la fonte");

    let octets_du_fond = octets_de(TEMOIN);
    let encres = pixels_changes(&fond, &ecrit);
    assert!(
        !encres.is_empty(),
        "le fût témoin doit s'écrire : sans un pixel d'encre, la roundeur de ses bouts n'est pas \
         une question"
    );

    // La couverture d'un pixel : son écart au fond, rapporté au plus grand écart du glyphe. Sans
    // unité, donc indifférente à la couleur de l'encre comme à l'ordre des composantes.
    let ecart_max = encres
        .iter()
        .map(|&(x, y)| ecart(triplet(&ecrit, x, y), octets_du_fond))
        .fold(0.0f64, f64::max);
    assert!(
        ecart_max > 0.0,
        "l'encre doit différer du fond, sinon rien de ce qui suit ne se mesure"
    );
    let couverture = |x: u32, y: u32| ecart(triplet(&ecrit, x, y), octets_du_fond) / ecart_max;

    // Le rectangle d'encre, à 5 % de couverture : c'est le seuil sous lequel un pixel n'est plus que
    // du bruit d'arrondi.
    let dedans: Vec<(u32, u32)> = encres
        .iter()
        .copied()
        .filter(|&(x, y)| couverture(x, y) >= 0.05)
        .collect();
    let (x0, x1) = (
        dedans
            .iter()
            .map(|&(x, _)| x)
            .min()
            .expect("un glyphe a des colonnes"),
        dedans
            .iter()
            .map(|&(x, _)| x)
            .max()
            .expect("un glyphe a des colonnes"),
    );
    let (y0, y1) = (
        dedans
            .iter()
            .map(|&(_, y)| y)
            .min()
            .expect("un glyphe a des lignes"),
        dedans
            .iter()
            .map(|&(_, y)| y)
            .max()
            .expect("un glyphe a des lignes"),
    );
    assert!(
        y1 - y0 >= 12 && x1 - x0 >= 2,
        "le fût témoin fait {} × {} pixel(s) — trop petit pour qu'un demi-disque de terminaison s'y \
         distingue d'une coupe droite. La boîte de l'ancre `haut` en fait 300 × 96",
        x1 - x0 + 1,
        y1 - y0 + 1
    );

    // L'encre de chaque ligne, et la ligne la plus chargée : c'est la pleine largeur du fût.
    let encre_de_la_ligne = |y: u32| -> f64 { (x0..=x1).map(|x| couverture(x, y)).sum::<f64>() };
    let lignes: Vec<(u32, f64)> = (y0..=y1).map(|y| (y, encre_de_la_ligne(y))).collect();
    let pleine_largeur = lignes
        .iter()
        .map(|&(_, encre)| encre)
        .fold(0.0f64, f64::max);
    assert!(
        pleine_largeur > 0.0,
        "le fût témoin doit porter de l'encre quelque part"
    );

    // Première mesure — **la ligne extrême porte bien moins d'encre que le fût**. « Extrême » se
    // dit à 3 % de la pleine largeur : au-dessous, c'est du débordement d'anticrénelage, et une
    // terminaison droite en laisse elle aussi.
    let mut encrees = lignes
        .iter()
        .filter(|&&(_, encre)| encre >= 0.03 * pleine_largeur);
    let haute = encrees
        .next()
        .copied()
        .expect("le fût a une première ligne encrée");
    let basse = encrees.next_back().copied().unwrap_or(haute);
    for (quoi, (y, encre)) in [("du haut", haute), ("du bas", basse)] {
        let part = encre / pleine_largeur;
        assert!(
            part <= 0.85,
            "terminaison {quoi} : la ligne extrême (y = {y}) porte {:.0} % de l'encre du fût, pour \
             85 % au plus. Une terminaison droite finit par une ligne **pleine**, et c'est \
             exactement ce que l'issue vient corriger",
            part * 100.0
        );
    }

    // Seconde mesure — **le coin est plus clair que le milieu**, et c'est celle qui décide. Chaque
    // colonne du fût est comparée **à elle-même** : ce qu'elle porte d'encre au bout, rapporté à ce
    // qu'elle en porte à mi-hauteur, là où le fût est droit. Un fût coupé à l'équerre garde partout
    // ce qu'il avait — relevé à 1,00 sur Liberation Sans, à chacune des trois tailles essayées et
    // pour chacune de ses deux colonnes de bord. Un demi-disque vide ses coins et garde son milieu :
    // relevé entre 0,00 et 0,12 aux coins de Nunito, contre 0,87 à 1,00 en son milieu.
    //
    // ⚠️ Comparer le coin au milieu **sur la seule ligne du bout** ne suffirait pas, et c'est mesuré
    // aussi : la colonne de bord d'un fût rendu à cheval sur deux pixels peut n'être couverte qu'à
    // 32 %, à mi-hauteur comme au bout. Elle passerait alors pour un coin vidé sans que rien ne soit
    // arrondi. Rapporter chaque colonne à elle-même retire ce faux positif.
    let y_milieu = (y0 + y1) / 2;
    let bords: Vec<u32> = (x0..=x1)
        .filter(|&x| couverture(x, y_milieu) >= 0.2)
        .collect();
    let (bord_gauche, bord_droit) = (
        *bords.first().expect("le fût a un bord gauche à mi-hauteur"),
        *bords.last().expect("le fût a un bord droit à mi-hauteur"),
    );
    assert!(
        bord_droit > bord_gauche,
        "le fût témoin doit avoir deux bords distincts à mi-hauteur, sinon « le coin » n'a pas de sens"
    );
    let bord_milieu = (bord_gauche + bord_droit) / 2;

    let mut franches = lignes
        .iter()
        .filter(|&&(_, encre)| encre >= 0.4 * pleine_largeur);
    let franche_haute = franches
        .next()
        .copied()
        .expect("le fût a une première ligne franche");
    let franche_basse = franches.next_back().copied().unwrap_or(franche_haute);
    let garde = |x: u32, y: u32| couverture(x, y) / couverture(x, y_milieu);
    for (quoi, (y, _)) in [("du haut", franche_haute), ("du bas", franche_basse)] {
        // Le milieu de l'épaisseur d'abord : il **ne se vide pas**. Sans cette moitié-là, un glyphe
        // simplement raccourci passerait pour une terminaison ronde.
        let au_milieu = garde(bord_milieu, y);
        assert!(
            au_milieu >= 0.5,
            "terminaison {quoi} : au milieu de l'épaisseur, la ligne y = {y} ne garde que {:.0} % \
             de l'encre du fût. Un demi-disque y garde tout — ce qui s'évanouit, ce sont ses coins",
            au_milieu * 100.0
        );
        for (ou, x) in [("gauche", bord_gauche), ("droit", bord_droit)] {
            let au_coin = garde(x, y);
            assert!(
                au_coin <= 0.6 * au_milieu,
                "terminaison {quoi}, coin {ou} : à la ligne y = {y}, la colonne du bord garde \
                 {:.0} % de son encre de mi-fût quand celle du milieu en garde {:.0} %. Une coupe \
                 droite fait perdre **autant** aux trois colonnes, et c'est l'Arial de 1982 que \
                 l'issue vient remplacer",
                au_coin * 100.0,
                au_milieu * 100.0
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Un garde-fou sur les repères de ce fichier
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_polaires_de_ce_fichier_mesurent_bien_ce_qu_ils_annoncent() {
    // Toutes les mesures d'arc de ce fichier passent par `pixel_polaire`, qui est l'inverse de
    // `angle_du_pixel` et `distance_au_centre`. S'il se trompait de sens ou d'origine, le profil de
    // portée continuerait de rendre des nombres, et ces nombres ne voudraient rien dire.
    //
    // Ce test ne vérifie donc pas le code du démon : il vérifie l'outil de mesure. C'est le seul du
    // fichier dans ce cas, et il est là pour que les cinq autres veuillent dire quelque chose.
    for (angle, ou) in [
        (0.0f64, "le sommet"),
        (90.0, "la droite"),
        (180.0, "le bas"),
        (270.0, "la gauche"),
    ] {
        let rayon = f64::from(composition::COURONNE_RAYON_INTERIEUR);
        let (x, y) = pixel_polaire(rayon, angle);
        let retour = angle_du_pixel(x, y);
        let ecart_angulaire = (retour - angle)
            .rem_euclid(360.0)
            .min((angle - retour).rem_euclid(360.0));
        assert!(
            ecart_angulaire <= 0.5,
            "{ou} : pixel_polaire({rayon}, {angle}) rend ({x}, {y}), que angle_du_pixel relit à \
             {retour:.2}°"
        );
        assert!(
            (distance_au_centre(x, y) - rayon).abs() <= 1.5,
            "{ou} : le pixel rendu doit être à {rayon} du centre, il est à {:.2}",
            distance_au_centre(x, y)
        );
    }

    // Et la couronne a bien une épaisseur à mesurer : sans elle, « le milieu de l'épaisseur » et
    // « ses deux coins » désigneraient le même pixel.
    assert!(
        epaisseur_de_la_couronne() >= 8.0,
        "la couronne fait {:.0} px d'épaisseur — sous huit, un demi-disque de terminaison ne se \
         distingue plus d'une coupe droite, quelle que soit l'implémentation",
        epaisseur_de_la_couronne()
    );
}
