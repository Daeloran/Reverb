//! Tests d'intention de la fonte embarquée et des arcs de couronne — issue #90.
//!
//! Écrits **avant** l'implémentation, depuis l'issue #90 seule. `crates/reverb-daemon/src/ecran.rs`
//! — le fichier que ce chantier modifie — n'a **pas** été lu pour les produire. Ce qui en est repris
//! ici, ce sont les signatures publiques déjà établies par #33 et #80, telles que les tests
//! d'intention de ces deux issues les nomment : `Dalle`, `Dalle::noire`, `Dalle::unie`,
//! `Dalle::octets`, `Dalle::cadran`, `Dalle::composee`, `ChampRendu`, `valeur_du_champ`, et
//! `Ancre::boite` côté `reverb-proto`. Si l'un de ces tests échoue après implémentation, c'est le
//! code qu'on corrige, jamais le test.
//!
//! Ce fichier **prolonge** `spec_composition_ecran.rs` (#80) et `spec_ecran.rs` (#33) : il ne
//! rejoue ni la taille du tampon, ni la persistance, ni le format. Il ne garde que ce que #90
//! ajoute — une vraie fonte, et un arc par champ de température.
//!
//! # ⚠️ Relu par #93, sur un seul point
//!
//! #93 pose une **piste** sous l'arc, qui occupe tout le secteur quelle que soit la proportion.
//! `un_arc_se_remplit_proportionnellement_du_vide_au_plein` mesurait le remplissage par les pixels
//! qu'il **change** ; il le mesure désormais par les pixels **de la couleur d'arc**, qui est la
//! mesure que le critère d'acceptation de #93 nomme. La propriété défendue n'a pas bougé, une seule
//! assertion a été **ajoutée** — l'arc vide ne porte aucune couleur d'arc —, et rien n'a été
//! assoupli. Le raisonnement complet est en tête de ce test.
//!
//! Les autres tests de ce fichier ont été relus au même moment et **laissés tels quels** : ils
//! mesurent tous par rapport à un arc à 0 °C ou à une sonde muette, qui portent la même piste que
//! ce qu'on leur compare, si bien que la soustraction l'annule. `un_champ_texte_ne_dessine_aucun_arc`
//! et `l_ancre_centre_n_a_pas_d_arc` comparent au fond nu, et restent vrais parce qu'un champ sans
//! proportion à montrer n'a ni arc **ni piste** — c'est un contrat de #90 que #93 ne touche pas.
//!
//! # La couture que ce fichier exige
//!
//! Elle n'existe pas encore : c'est le propre de la phase rouge. Six ajouts, tous choisis pour
//! prolonger l'existant plutôt que le refaire.
//!
//! Dans `reverb_proto::composition` — la géométrie de la dalle y vit déjà, et la fenêtre (#76)
//! devra la lire sans dupliquer une coordonnée :
//!
//! ```ignore
//! /// Le rayon intérieur de la couronne où les arcs se dessinent.
//! pub const COURONNE_RAYON_INTERIEUR: u16;
//! /// Son rayon extérieur, qui ne dépasse jamais `screen::VISIBLE_DISC_RADIUS`.
//! pub const COURONNE_RAYON_EXTERIEUR: u16;
//!
//! /// Un secteur angulaire de la couronne, en **degrés**.
//! #[derive(Debug, Clone, Copy, PartialEq)]
//! pub struct Secteur {
//!     /// L'angle où l'arc commence à se remplir.
//!     pub debut: f32,
//!     /// Son ouverture, toujours strictement positive.
//!     pub ouverture: f32,
//! }
//!
//! impl Ancre {
//!     /// Le secteur de couronne de cette ancre — le pendant de [`Ancre::boite`].
//!     ///
//!     /// `None` pour `Centre`, qui n'est pas sur la couronne.
//!     pub fn secteur(self) -> Option<Secteur>;
//! }
//! ```
//!
//! Dans `reverb_daemon::ecran` :
//!
//! ```ignore
//! /// La fonte embarquée dans le binaire, telle quelle.
//! pub const FONTE: &[u8];
//!
//! impl Dalle {
//!     /// `texte` écrit dans la fonte embarquée, sur ce fond, dans cette boîte.
//!     ///
//!     /// **Le texte seul** : ni plaque, ni contour, ni arc. C'est la primitive, pas un champ.
//!     pub fn texte(fond: &Dalle, texte: &str, boite: Boite) -> Dalle;
//!
//!     /// L'arc de ce secteur, rempli de `proportion` de son ouverture.
//!     ///
//!     /// `proportion` va de 0 à 1 ; hors de ces bornes elle est **écrêtée**, jamais repliée —
//!     /// même règle que `Dalle::cadran` (#33).
//!     pub fn arc(fond: &Dalle, secteur: Secteur, proportion: f32) -> Dalle;
//! }
//!
//! /// La proportion d'un arc pour une température, **sur l'échelle du cadran : 0 à 100 °C**.
//! pub fn proportion_de_temperature(valeur: f32) -> f32;
//! ```
//!
//! # La convention d'angle que ce fichier fixe
//!
//! L'issue parle de secteurs sans dire d'où on compte. Il faut trancher, sinon aucun test ne peut
//! nommer un angle : **0° au sommet de la dalle, croissant dans le sens horaire**. C'est le sens de
//! lecture d'un cadran, et celui qui met `haut` à 0°, `droite` à 90°, `bas` à 180°, `gauche` à 270°
//! — c'est-à-dire chaque arc au-dessus de l'ancre qu'il sert.
//!
//! # Les quatre pièges que ce fichier garde
//!
//! 1. **« Écrit dans une vraie fonte » ne se vérifie pas contre une image de référence.** Figer un
//!    rendu au pixel près, c'est un test qui casse au premier réglage d'interlettrage et qu'on
//!    finira par désactiver. Ce fichier vérifie donc des **propriétés** : une aire non nulle, deux
//!    chaînes distinctes qui donnent deux tampons distincts, les accents qui s'écrivent, et
//!    l'anticrénelage — qu'une matrice 5 × 7 blittée ne peut pas produire. Une implémentation qui
//!    n'écrirait **rien** meurt dès le deuxième test.
//! 2. **« L'arc est rempli proportionnellement » passe sur un arc toujours plein.** Trois
//!    proportions sont donc comparées — 0 %, 50 %, 100 % —, et ce qu'on mesure n'est pas l'arc mais
//!    **ce qui le distingue de l'arc vide** : cela isole le remplissage même si l'implémentation
//!    dessine un rail sous lui. Le compte doit croître **strictement**, le remplissage de 50 %
//!    doit être **inclus** dans celui de 100 %, et son étendue angulaire doit valoir la moitié de
//!    l'ouverture.
//! 3. **« Tout reste dans le disque » passe sur une implémentation qui ne dessine rien.** Le test
//!    de débordement exige donc, en plus, qu'il y ait bien des pixels peints **dans** le disque.
//!    La dalle est ronde (`SPEC-KRAKEN-LCD` §2.1.1), 21 % du tampon ne s'affiche nulle part, et un
//!    arc qui déborderait serait invisible sans qu'aucune erreur ne le dise.
//! 4. **Une sonde muette dont l'arc reste au dernier remplissage est rassurante et fausse.** C'est
//!    la règle de #68 appliquée à l'arc : un arc aux trois quarts derrière une pompe arrêtée, c'est
//!    un circuit qui chauffe sans que rien ne le signale. L'arc est vide, et ce sont les tirets qui
//!    portent l'information.
//!
//! # Ce que ce fichier tranche, là où l'issue laisse ouvert
//!
//! 1. **Le rendu du texte est anticrénelé.** L'issue dit « une vraie fonte » sans dire comment on
//!    le prouverait. Seuiller la couverture que rend le rastériseur pour retomber sur du noir et
//!    blanc serait du travail en plus pour un résultat plus laid — c'est-à-dire exactement ce que
//!    l'issue veut corriger. Le test exige donc plusieurs niveaux de gris sur les bords des glyphes.
//! 2. **La couronne est un anneau réservé.** `COURONNE_RAYON_INTERIEUR`…`COURONNE_RAYON_EXTERIEUR`
//!    ne porte **que** des arcs : « libellé et valeur juste à l'intérieur » se lit comme une
//!    exclusion, sinon « un champ `texte` n'a pas d'arc » n'est pas décidable sur un tampon.
//! 3. **Deux arcs voisins laissent un intervalle.** L'issue dit « ne se chevauchent pas » ; ce
//!    fichier l'exige au pixel et non au degré. Deux arcs exactement jointifs se liraient comme un
//!    seul, et on ne saurait pas où l'un finit.
//! 4. **`Ancre::secteur()` rend `None` pour `Centre`**, plutôt qu'un secteur d'ouverture nulle : le
//!    critère d'acceptation dit « l'ancre `centre` n'a pas d'arc », et un `Option` le dit dans le
//!    type au lieu de le faire vérifier à chaque appelant.
//!
//! # ⚠️ Un point que l'implémentation devra reconcilier, et que ce fichier ne tranche pas
//!
//! `spec_composition_ecran.rs` (#80) exige que **tout** pixel peint par un champ tienne dans
//! `Ancre::boite()`. Un arc, lui, se dessine sur la couronne — donc hors du texte. Deux issues
//! seulement : ou `Ancre::boite()` grandit pour englober l'arc, ou le contrat de #80 se relit.
//! C'est une décision d'implémentation ; ce fichier n'assied donc **aucune** de ses vérifications
//! sur `Ancre::boite()`, et raisonne exclusivement sur la couronne et sur le disque visible.
//!
//! # Ce que ce fichier ne teste pas
//!
//! - La **taille du binaire** avant/après : c'est une mesure à rapporter dans l'issue, pas une
//!   assertion — un seuil en octets casserait à la première montée de `image` ou de `slint`.
//! - L'**exemple** `--example composition` : c'est une cible qu'on exécute, pas une bibliothèque.
//! - `clippy` et `fmt` : ils ont leur propre commande.
//! - Aucune écriture matérielle, aucun accès à `/dev`, aucun fichier, aucun démon lancé.

// Les assertions qui **encadrent** les constantes de couronne sont constantes une fois ces
// constantes écrites, et clippy les refuse à ce titre — comme dans `spec_plafond.rs` et
// `spec_fil_ecran.rs`, où le même `allow` est posé pour la même raison. Leur intérêt n'est pas
// d'observer une exécution mais de casser la compilation le jour où la couronne sortirait du
// disque ou perdrait son épaisseur.
#![allow(clippy::assertions_on_constants)]

use std::collections::BTreeSet;

use reverb_daemon::ecran::{
    ARC_COULEUR, ChampRendu, Dalle, FONTE, proportion_de_temperature, valeur_du_champ,
};
use reverb_proto::composition::{self, Ancre, Boite, Secteur};
use reverb_proto::screen;

// ---------------------------------------------------------------------------
// Témoins
// ---------------------------------------------------------------------------

/// La largeur de la dalle. Reprise du protocole, jamais réécrite.
const LARGEUR: u32 = screen::WIDTH as u32;

/// La hauteur de la dalle.
const HAUTEUR: u32 = screen::HEIGHT as u32;

/// Le noir.
const NOIR: (u8, u8, u8) = (0x00, 0x00, 0x00);

/// La couleur témoin du projet. Ses trois composantes sont distinctes, donc aucune permutation ne
/// peut passer inaperçue.
const TEMOIN: (u8, u8, u8) = (0xff, 0x20, 0x80);

/// Le libellé témoin, court et lisible — celui de l'exemple `composition`.
const LIBELLE: &str = "LIQUIDE";

/// Les quatre ancres qui portent un arc. `Centre` en est exclue par le critère d'acceptation.
const ANCRES_DE_COURONNE: [Ancre; 4] = [Ancre::Haut, Ancre::Droite, Ancre::Bas, Ancre::Gauche];

// ---------------------------------------------------------------------------
// Lecture d'une dalle
// ---------------------------------------------------------------------------

/// Les octets bruts du pixel (`x`, `y`), dans l'ordre où ils partiront sur le bus.
fn triplet(dalle: &Dalle, x: u32, y: u32) -> [u8; screen::PIXEL_LEN] {
    let octets = dalle.octets();
    let debut = (y as usize * LARGEUR as usize + x as usize) * screen::PIXEL_LEN;
    octets[debut..debut + screen::PIXEL_LEN]
        .try_into()
        .expect("un pixel fait screen::PIXEL_LEN octets")
}

/// La clarté d'un pixel : la moyenne de ses trois octets.
///
/// La moyenne est **invariante par permutation** des composantes. Le projet en mêle trois — LED en
/// GRB, écran en BGR, RAM en RGB —, et ce fichier n'a pas à savoir lequel des trois octets porte le
/// rouge pour dire « ce pixel est à mi-chemin entre le fond et l'encre ».
fn clarte(dalle: &Dalle, x: u32, y: u32) -> u16 {
    let brut = triplet(dalle, x, y);
    (u16::from(brut[0]) + u16::from(brut[1]) + u16::from(brut[2])) / 3
}

/// Les pixels qui diffèrent entre deux dalles.
///
/// C'est la mesure de base de ce fichier : elle ne suppose rien du dessin — ni sa couleur, ni sa
/// police, ni sa forme —, seulement qu'il a lieu quelque part et pas ailleurs. Un test d'intention
/// ne fige pas un goût.
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

/// Le nombre de pixels qu'un dessin a changés.
fn aire(avant: &Dalle, apres: &Dalle) -> usize {
    pixels_changes(avant, apres).len()
}

/// Les pixels peints **exactement** de la couleur d'arc.
///
/// ⚠️ **Ajouté par #93, qui a relu le remplissage de l'arc.** Depuis que l'arc repose sur une
/// **piste** occupant tout le secteur quelle que soit la proportion, « ce que l'arc a changé » et
/// « ce que l'arc a rempli » ne sont plus le même ensemble, et c'est le second que le critère
/// d'acceptation nomme : « le remplissage reste strictement croissant, mesuré sur les pixels de la
/// couleur d'arc, **la piste ne comptant pas** ».
///
/// La mesure est **conservatrice** : elle laisse de côté l'anticrénelage des bords, donc elle ne
/// peut pas être gonflée par le lissage que #93 ajoute. Ce qu'elle compte est bien du remplissage.
fn pixels_d_arc(dalle: &Dalle) -> Vec<(u32, u32)> {
    // Les octets de la couleur d'arc sont obtenus **en peignant une dalle**, jamais recopiés :
    // l'écran est en BGR, les LED en GRB, la RAM en RGB, et un test qui écrirait l'ordre à la main
    // serait le premier endroit du projet à pouvoir se tromper sans que rien ne le dise.
    let encre = triplet(&Dalle::unie(ARC_COULEUR), 0, 0);
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
/// Une dalle courte est **ignorée en silence** par le matériel (spec §2.2.1) : c'est le genre de
/// défaut qu'on ne découvre qu'en regardant l'écran, et seulement si on y pense.
fn dalle_bien_dimensionnee(dalle: &Dalle, quoi: &str) {
    assert_eq!(
        dalle.octets().len(),
        screen::IMAGE_LEN,
        "{quoi} doit faire screen::IMAGE_LEN octets — une dalle courte est ignorée par le \
         contrôleur sans le moindre code d'erreur"
    );
    assert!(
        screen::check_image(dalle.octets()).is_ok(),
        "{quoi} doit passer le validateur du protocole"
    );
}

// ---------------------------------------------------------------------------
// Géométrie : le disque, la couronne, les angles
// ---------------------------------------------------------------------------

/// La distance du centre de la dalle au centre du pixel (`x`, `y`).
fn distance_au_centre(x: u32, y: u32) -> f64 {
    let centre = f64::from(LARGEUR) / 2.0;
    (f64::from(x) + 0.5 - centre).hypot(f64::from(y) + 0.5 - centre)
}

/// Vrai si le carré du pixel (`x`, `y`) est **entièrement** hors du disque visible.
///
/// La distance retenue est celle du centre de la dalle au point du pixel qui lui est le plus
/// proche : si même celui-là est au-delà du rayon, aucun point du pixel n'est visible. La marge d'un
/// pixel ainsi laissée sur le bord est délibérée — ce test attrape un arc posé dans un coin, pas un
/// arrondi d'anticrénelage sur la limite de la vitre.
fn entierement_hors_du_disque(x: u32, y: u32) -> bool {
    let centre_x = f64::from(LARGEUR) / 2.0;
    let centre_y = f64::from(HAUTEUR) / 2.0;
    let proche_x = centre_x.clamp(f64::from(x), f64::from(x + 1));
    let proche_y = centre_y.clamp(f64::from(y), f64::from(y + 1));
    let dx = centre_x - proche_x;
    let dy = centre_y - proche_y;
    let rayon = f64::from(screen::VISIBLE_DISC_RADIUS);
    dx * dx + dy * dy >= rayon * rayon
}

/// Vrai si le pixel tombe dans l'anneau réservé aux arcs.
///
/// Point n° 2 de ce que ce fichier tranche : cet anneau ne porte **que** des arcs. C'est ce qui rend
/// « un champ `texte` n'a pas d'arc » décidable sur un tampon plutôt que sur une bonne intention.
fn dans_la_couronne(x: u32, y: u32) -> bool {
    let distance = distance_au_centre(x, y);
    distance >= f64::from(composition::COURONNE_RAYON_INTERIEUR)
        && distance <= f64::from(composition::COURONNE_RAYON_EXTERIEUR)
}

/// Ne garde des pixels que ceux qui tombent dans la couronne.
fn couronne_seulement(pixels: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    pixels
        .into_iter()
        .filter(|&(x, y)| dans_la_couronne(x, y))
        .collect()
}

/// L'angle d'un pixel vu du centre de la dalle, en degrés dans `[0, 360)`.
///
/// **0° au sommet, croissant dans le sens horaire** — la convention que ce fichier fixe, faute que
/// l'issue en donne une.
fn angle_du_pixel(x: u32, y: u32) -> f64 {
    let centre = f64::from(LARGEUR) / 2.0;
    let dx = f64::from(x) + 0.5 - centre;
    let dy = f64::from(y) + 0.5 - centre;
    // `atan2(dx, -dy)` : le sommet donne 0, la droite 90. C'est un cadran, pas un cercle
    // trigonométrique — l'écran se lit dans ce sens-là.
    let brut = dx.atan2(-dy).to_degrees();
    brut.rem_euclid(360.0)
}

/// L'angle d'un pixel **rapporté au début d'un secteur**.
///
/// Un secteur peut enjamber 0° — celui de `haut` le fera forcément —, et comparer des angles
/// absolus s'y casse. Tout est donc ramené à « combien de degrés après le début de l'arc », et une
/// fenêtre de 45° avant le début est rendue en négatif plutôt qu'en 315 : sans elle, un pixel
/// d'anticrénelage posé un demi-degré avant le départ ferait croire à un arc de 359°.
fn relatif_au_secteur(angle: f64, secteur: Secteur) -> f64 {
    let brut = (angle - f64::from(secteur.debut)).rem_euclid(360.0);
    if brut > 315.0 { brut - 360.0 } else { brut }
}

/// L'étendue angulaire d'un ensemble de pixels dans son secteur : le plus tôt, le plus tard.
fn etendue(pixels: &[(u32, u32)], secteur: Secteur) -> Option<(f64, f64)> {
    let mut debut = f64::INFINITY;
    let mut fin = f64::NEG_INFINITY;
    for &(x, y) in pixels {
        let rel = relatif_au_secteur(angle_du_pixel(x, y), secteur);
        debut = debut.min(rel);
        fin = fin.max(rel);
    }
    (debut <= fin).then_some((debut, fin))
}

/// Deux secteurs se recouvrent-ils ?
///
/// Calculé, jamais échantillonné : `b` est ramené dans le repère de `a`, et les deux sont disjoints
/// si `b` commence après la fin de `a` et se termine avant que `a` ne recommence, un tour plus loin.
fn se_recouvrent(a: Secteur, b: Secteur) -> bool {
    let decalage = (f64::from(b.debut) - f64::from(a.debut)).rem_euclid(360.0);
    let apres_a = decalage >= f64::from(a.ouverture);
    let avant_le_tour = decalage + f64::from(b.ouverture) <= 360.0;
    !(apres_a && avant_le_tour)
}

/// Vrai si le secteur couvre cet angle absolu.
fn secteur_contient(secteur: Secteur, angle: f64) -> bool {
    relatif_au_secteur(angle, secteur) >= 0.0
        && (angle - f64::from(secteur.debut)).rem_euclid(360.0) <= f64::from(secteur.ouverture)
}

/// Le secteur d'une ancre, ou l'échec du test — les quatre ancres de couronne en ont un.
fn secteur_de(ancre: Ancre) -> Secteur {
    ancre
        .secteur()
        .unwrap_or_else(|| panic!("{ancre:?} est sur la couronne et doit avoir un secteur"))
}

// ---------------------------------------------------------------------------
// Champs témoins
// ---------------------------------------------------------------------------

/// Un champ de température au libellé témoin.
fn champ_temp(valeur: Option<f32>) -> ChampRendu {
    ChampRendu::Temperature {
        libelle: Some(LIBELLE.to_owned()),
        valeur,
    }
}

/// Le rendu d'un champ unique, seul sur son fond.
fn rendu_du_champ(fond: &Dalle, ancre: Ancre, champ: ChampRendu) -> Dalle {
    Dalle::composee(fond, &[(ancre, champ)])
}

// ---------------------------------------------------------------------------
// 1 — la fonte est embarquée, et c'en est une
// ---------------------------------------------------------------------------

#[test]
fn la_fonte_embarquee_est_une_vraie_fonte_et_pas_une_matrice_dessinee_a_la_main() {
    // Approche technique de l'issue : « `LiberationSans-Bold.ttf` […] embarquée telle quelle par
    // `include_bytes!` ».
    //
    // C'est le test le moins subtil du fichier, et celui qui rend les cinq autres lisibles : sans
    // fonte dans le binaire, « écrit dans la fonte embarquée » n'a aucun sens, et le démon cesserait
    // d'être un binaire unique sans dépendance de runtime (ADR-001) le jour où quelqu'un lirait un
    // `.ttf` depuis `/usr/share/fonts`.
    assert!(
        !FONTE.is_empty(),
        "la fonte doit être dans le binaire : le démon ne lit aucun fichier de police, sinon il \
         casse à la première montée d'image de Bazzite"
    );

    // Une matrice 5 × 7 dessinée à la main tient en quelques centaines d'octets. Une fonte
    // vectorielle complète en fait des centaines de milliers — le seuil sépare les deux sans figer
    // la taille exacte du fichier, qui n'a pas à être vérifiée par un test.
    assert!(
        FONTE.len() >= 100 * 1024,
        "une fonte vectorielle pèse des centaines de kilooctets ; {} octets, c'est encore une \
         table dessinée à la main",
        FONTE.len()
    );

    // Et c'est bien un fichier de fonte, pas un blob quelconque : les quatre premiers octets d'un
    // TrueType, d'un OpenType ou d'une collection sont connus. Un `include_bytes!` qui viserait le
    // mauvais chemin — un README, un `.rs` — passerait les deux assertions précédentes.
    let magie: [u8; 4] = FONTE[..4].try_into().expect("une fonte a plus de 4 octets");
    assert!(
        matches!(&magie, b"\x00\x01\x00\x00" | b"true" | b"OTTO" | b"ttcf"),
        "les quatre premiers octets doivent être ceux d'une fonte TrueType ou OpenType. \
         Obtenu : {magie:02x?}"
    );
}

// ---------------------------------------------------------------------------
// 2 — un texte occupe une aire, et distingue ses chaînes
// ---------------------------------------------------------------------------

#[test]
fn un_texte_ecrit_dans_la_fonte_couvre_une_aire_non_nulle_et_distingue_ses_chaines() {
    // Comportement à tester n° 1 de l'issue : « Un texte rendu dans la fonte embarquée occupe une
    // aire non nulle, et deux textes différents donnent deux images différentes — le garde-fou
    // contre un rastériseur qui ne dessine rien. »
    //
    // Piège n° 1 du préambule. Un rastériseur mal branché ne panique pas : il rend une couverture
    // vide, la boucle de dessin la recopie consciencieusement, et la dalle affiche le fond. Sur six
    // centimètres, derrière un bureau, personne ne le voit avant des semaines.
    let fond = Dalle::unie(TEMOIN);
    let boite = Ancre::Centre.boite();
    let ecrire = |texte: &str| Dalle::texte(&fond, texte, boite);

    // Une chaîne vide n'écrit rien : c'est la borne basse, et elle rend les autres assertions
    // significatives.
    assert_eq!(
        aire(&fond, &ecrire("")),
        0,
        "une chaîne vide ne dessine rien — sinon l'aire mesurée plus bas ne dit plus si c'est le \
         texte qu'on voit ou un habillage"
    );

    // Une chaîne quelconque, elle, laisse une trace franche. Vingt pixels seraient déjà beaucoup
    // pour du bruit d'arrondi, et très peu pour un glyphe haut de plusieurs dizaines de pixels.
    for texte in ["8", "34.2", LIBELLE, "SHYNAEL"] {
        let ecrit = ecrire(texte);
        dalle_bien_dimensionnee(&ecrit, &format!("« {texte} » écrit dans la fonte"));
        assert!(
            aire(&fond, &ecrit) >= 20,
            "« {texte} » doit laisser des pixels sur la dalle. Obtenu : {} — un rastériseur qui \
             rend une couverture vide ne se signale d'aucune autre façon",
            aire(&fond, &ecrit)
        );
    }

    // Deux textes différents donnent deux tampons différents : sans quoi le rendu serait une image
    // fixe qui ressemble à du texte, et un cadran figé se lirait comme une mesure.
    let distincts = ["ABC", "ABD", "8", "1", "34.2", "80.0", LIBELLE, "CPU"];
    for (rang, gauche) in distincts.iter().enumerate() {
        for droite in &distincts[rang + 1..] {
            assert_ne!(
                ecrire(gauche).octets(),
                ecrire(droite).octets(),
                "« {gauche} » et « {droite} » ne doivent pas rendre la même dalle"
            );
        }
    }

    // Un second caractère occupe plus de place qu'un seul. La boîte témoin a largement de quoi
    // porter deux chiffres : aucune réduction n'a lieu ici, et l'aire croît donc bien avec le
    // nombre de glyphes.
    assert!(
        aire(&fond, &ecrire("88")) > aire(&fond, &ecrire("8")),
        "« 88 » couvre plus de pixels que « 8 » — un rendu qui ne dessinerait que le premier glyphe \
         passerait toutes les assertions précédentes"
    );

    // Et la fonte sait écrire ce qu'une table dessinée à la main n'a aucune raison de contenir : le
    // degré, les accents, les capitales accentuées. La dalle affiche « °C » et des libellés que
    // l'utilisateur tape lui-même — « soirée d'été » est un nom de profil du README.
    for texte in ["°C", "é", "É", "à", "ç", "µ", "soirée d'été"] {
        assert!(
            aire(&fond, &ecrire(texte)) >= 8,
            "« {texte} » doit s'écrire : la fonte est complète, et un libellé se tape à la main"
        );
    }
    assert_ne!(
        ecrire("E").octets(),
        ecrire("É").octets(),
        "« E » et « É » ne s'écrivent pas pareil — un repli qui mangerait l'accent rendrait tous \
         les libellés accentués identiques à leur version nue"
    );
}

// ---------------------------------------------------------------------------
// 3 — le cadran et la composition passent par la fonte
// ---------------------------------------------------------------------------

#[test]
fn le_cadran_et_la_composition_ecrivent_avec_la_fonte_et_non_avec_une_matrice() {
    // Critère d'acceptation : « le cadran et la composition sont écrits avec la fonte embarquée ».
    //
    // Point n° 1 de ce que ce fichier tranche. Le contrôle est l'**anticrénelage** : un rastériseur
    // vectoriel rend une couverture, donc des bords en dégradé ; une matrice 5 × 7 blittée ne
    // connaît que deux valeurs, l'encre et le fond. Compter les niveaux de gris distincts sur les
    // pixels d'un glyphe sépare les deux sans rien figer d'un rendu.
    //
    // ⚠️ Chaque mesure isole le texte en comparant **deux rendus qui ne diffèrent que par lui** :
    // même ancre, même proportion, même anneau, mêmes tirets. Sans cette précaution, le dégradé de
    // l'anneau du cadran suffirait à faire passer le test sans qu'une seule lettre soit écrite.
    let nuances = |avant: &Dalle, apres: &Dalle| -> usize {
        pixels_changes(avant, apres)
            .into_iter()
            .map(|(x, y)| clarte(apres, x, y))
            .collect::<BTreeSet<u16>>()
            .len()
    };
    let minimum = 4;

    // La primitive d'abord, sur un fond uni : les pixels changés sont exactement ceux du glyphe.
    let fond = Dalle::unie(TEMOIN);
    let sur_la_primitive = nuances(&fond, &Dalle::texte(&fond, "88.8", Ancre::Centre.boite()));
    assert!(
        sur_la_primitive >= minimum,
        "un texte rastérisé a des bords en dégradé : {sur_la_primitive} nuance(s) pour {minimum} \
         attendues — deux seules valeurs, c'est une matrice blittée, et c'est ce que l'issue vient \
         corriger"
    );

    // Le cadran ensuite. Deux cadrans qui ne diffèrent que par leur libellé : ce qui change entre
    // eux, ce sont les glyphes du libellé, et rien d'autre.
    let libelle_a = Dalle::cadran("AAAA", Some(11.1), "°C", 0.5);
    let libelle_b = Dalle::cadran("BBBB", Some(11.1), "°C", 0.5);
    assert_ne!(
        libelle_a.octets(),
        libelle_b.octets(),
        "le cadran écrit son libellé : deux libellés différents ne peuvent pas donner la même dalle"
    );
    let sur_le_libelle = nuances(&libelle_a, &libelle_b);
    assert!(
        sur_le_libelle >= minimum,
        "le libellé du cadran doit être écrit dans la fonte : {sur_le_libelle} nuance(s) pour \
         {minimum} attendues"
    );

    // Et sa valeur, par la même méthode : même libellé, même proportion, seuls les chiffres bougent.
    let valeur_basse = Dalle::cadran(LIBELLE, Some(11.1), "°C", 0.5);
    let valeur_haute = Dalle::cadran(LIBELLE, Some(88.8), "°C", 0.5);
    let sur_les_chiffres = nuances(&valeur_basse, &valeur_haute);
    assert!(
        sur_les_chiffres >= minimum,
        "les chiffres du cadran doivent être écrits dans la fonte, et non en sept segments : \
         {sur_les_chiffres} nuance(s) pour {minimum} attendues"
    );

    // La composition enfin, sur les cinq ancres — le texte d'un champ passe par la même fonte que
    // le cadran, sinon la dalle mêlerait deux typographies selon ce qu'on affiche.
    for ancre in Ancre::TOUTES {
        let texte_a = rendu_du_champ(&fond, ancre, ChampRendu::Texte("AAAA".to_owned()));
        let texte_b = rendu_du_champ(&fond, ancre, ChampRendu::Texte("BBBB".to_owned()));
        assert_ne!(
            texte_a.octets(),
            texte_b.octets(),
            "{ancre:?} : deux textes différents ne rendent pas la même dalle"
        );
        let sur_le_champ = nuances(&texte_a, &texte_b);
        assert!(
            sur_le_champ >= minimum,
            "{ancre:?} : le texte d'un champ doit être écrit dans la fonte. {sur_le_champ} \
             nuance(s) pour {minimum} attendues"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — l'arc se remplit, du vide au plein
// ---------------------------------------------------------------------------

#[test]
fn un_arc_se_remplit_proportionnellement_du_vide_au_plein() {
    // Comportement à tester n° 2 de l'issue : « Une sonde à 0 °C rend un arc vide, une sonde à
    // 100 °C un arc plein sur toute l'ouverture de son secteur. »
    //
    // Piège n° 2 du préambule. « L'arc est rempli proportionnellement » passe sans broncher sur une
    // implémentation qui dessine toujours l'arc entier : il faut trois proportions, et une mesure
    // qui isole **le remplissage** plutôt que l'arc.
    //
    // ⚠️ **Relu par #93, qui a posé une piste sous l'arc.** Ce test comptait les pixels que le
    // remplissage **change** par rapport à l'arc vide. La propriété qu'il défend n'a pas bougé d'un
    // mot ; c'est la façon de la mesurer qui n'est plus la bonne, et pour deux raisons :
    //
    // 1. le critère d'acceptation de #93 nomme désormais la mesure — « strictement croissant,
    //    **mesuré sur les pixels de la couleur d'arc**, la piste ne comptant pas ». Un test qui
    //    mesure autre chose que ce que le critère dit finit par diverger de lui ;
    // 2. « ce que le remplissage change » dépend de ce qu'il recouvre. Tant que la piste est
    //    identique aux trois proportions, la soustraction l'annule — mais elle n'a plus à l'être :
    //    une piste dessinée sur le seul reste à remplir, ou décorée à son extrémité mobile,
    //    resterait parfaitement conforme à #93 et ferait mentir le comptage.
    //
    // Compter les pixels **de la couleur d'arc** ne dépend, lui, que de l'arc. C'est la seule chose
    // qui change ici : rien n'est assoupli, et les cinq assertions qui suivent disent exactement ce
    // qu'elles disaient.
    let fond = Dalle::unie(TEMOIN);

    for ancre in ANCRES_DE_COURONNE {
        let secteur = secteur_de(ancre);
        let ouverture = f64::from(secteur.ouverture);
        assert!(
            ouverture > 0.0,
            "{ancre:?} : un secteur d'ouverture nulle ne peut rien remplir"
        );

        let vide = Dalle::arc(&fond, secteur, 0.0);
        let moitie = Dalle::arc(&fond, secteur, 0.5);
        let plein = Dalle::arc(&fond, secteur, 1.0);
        for (quoi, dalle) in [("vide", &vide), ("à moitié", &moitie), ("plein", &plein)] {
            dalle_bien_dimensionnee(dalle, &format!("l'arc {quoi} de {ancre:?}"));
        }

        // Quelque chose est dessiné : un arc plein qui ne peindrait pas un pixel passerait tout
        // ce qui suit, les ensembles comparés étant alors tous vides.
        let remplissage_plein = couronne_seulement(pixels_d_arc(&plein));
        assert!(
            remplissage_plein.len() >= 200,
            "{ancre:?} : un arc plein doit peindre la couronne. Obtenu {} pixel(s) — un arc qu'on \
             ne voit pas ne dit ni la valeur ni où elle se situe",
            remplissage_plein.len()
        );

        // Et l'arc vide, lui, ne porte **aucun** pixel de la couleur d'arc : c'est la borne basse de
        // la progression, et depuis #93 elle se dit sans détour — une piste, et rien d'autre.
        assert!(
            couronne_seulement(pixels_d_arc(&vide)).is_empty(),
            "{ancre:?} : un arc vide ne peint aucun pixel de la couleur d'arc"
        );

        // La progression est **stricte**. C'est elle, et rien d'autre, qui tue l'arc toujours plein.
        let remplissage_moitie = couronne_seulement(pixels_d_arc(&moitie));
        assert!(
            !remplissage_moitie.is_empty(),
            "{ancre:?} : un arc à moitié rempli peint quelque chose"
        );
        assert!(
            remplissage_moitie.len() < remplissage_plein.len(),
            "{ancre:?} : le remplissage doit croître avec la proportion — {} pixel(s) à moitié \
             pour {} au plein",
            remplissage_moitie.len(),
            remplissage_plein.len()
        );

        // Et il **croît** au lieu de se déplacer : tout ce qui est peint à moitié l'est encore au
        // plein. Un arc qui glisserait le long du secteur au lieu de s'allonger aurait des comptes
        // strictement croissants et un sens faux.
        let plein_en_ensemble: BTreeSet<(u32, u32)> = remplissage_plein.iter().copied().collect();
        for pixel in &remplissage_moitie {
            assert!(
                plein_en_ensemble.contains(pixel),
                "{ancre:?} : le pixel {pixel:?} est peint à moitié et plus au plein — l'arc \
                 s'allonge depuis son début, il ne se déplace pas"
            );
        }

        // L'étendue angulaire, enfin : c'est ce qui donne son sens à « proportionnellement ». Le
        // remplissage part du début du secteur et s'arrête à la fraction demandée de son ouverture.
        let marge = (0.15 * ouverture).max(6.0);
        for (proportion, attendu) in [(0.5f32, 0.5), (1.0f32, 1.0)] {
            let dessine = Dalle::arc(&fond, secteur, proportion);
            let remplissage = couronne_seulement(pixels_d_arc(&dessine));
            let (debut, fin) = etendue(&remplissage, secteur)
                .unwrap_or_else(|| panic!("{ancre:?} : l'arc de {proportion} ne peint rien"));
            assert!(
                debut.abs() <= marge,
                "{ancre:?}, proportion {proportion} : le remplissage commence à {debut:.1}° du \
                 début du secteur — un arc se remplit **depuis** son origine, sinon deux ancres \
                 voisines n'ont plus de point de départ commun à l'œil"
            );
            let voulu: f64 = attendu * ouverture;
            assert!(
                (fin - voulu).abs() <= marge,
                "{ancre:?}, proportion {proportion} : le remplissage s'arrête à {fin:.1}° pour \
                 {voulu:.1}° attendus sur une ouverture de {ouverture:.1}° (marge {marge:.1}°)"
            );
        }

        // Hors bornes, la proportion est **écrêtée**, jamais repliée — la règle de `Dalle::cadran`
        // (#33). Un arc qui repartirait à zéro au-delà de 1 montrerait un circuit froid au moment
        // où il chauffe le plus.
        for au_dela in [1.001f32, 2.0, 1.0e30, f32::INFINITY] {
            assert_eq!(
                Dalle::arc(&fond, secteur, au_dela).octets(),
                plein.octets(),
                "{ancre:?} : une proportion de {au_dela} est ramenée à 1"
            );
        }
        for en_deca in [-0.001f32, -1.0, -1.0e30, f32::NEG_INFINITY] {
            assert_eq!(
                Dalle::arc(&fond, secteur, en_deca).octets(),
                vide.octets(),
                "{ancre:?} : une proportion de {en_deca} est ramenée à 0"
            );
        }
        // Un `NaN` ne doit ni paniquer ni tronquer la dalle : le démon tient l'écran, et une panique
        // ici emporte aussi l'éclairage.
        dalle_bien_dimensionnee(
            &Dalle::arc(&fond, secteur, f32::NAN),
            &format!("l'arc de {ancre:?} pour une proportion NaN"),
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — l'arc d'un champ suit l'échelle du cadran
// ---------------------------------------------------------------------------

#[test]
fn l_arc_d_un_champ_temp_suit_l_echelle_du_cadran_de_zero_a_cent_degres() {
    // Critère d'acceptation : « chaque champ `temp` porte son arc, rempli selon la même échelle que
    // le cadran (0–100 °C) ».
    //
    // Ce test relie les deux moitiés de l'issue : l'arc que `Dalle::arc` sait remplir, et la
    // température que le champ porte. Il se mesure dans la **couronne seule** — le libellé et la
    // valeur changent eux aussi entre 0 et 100 °C, et les compter mêlerait le texte à l'arc.
    let fond = Dalle::unie(TEMOIN);

    for ancre in ANCRES_DE_COURONNE {
        let secteur = secteur_de(ancre);
        let ouverture = f64::from(secteur.ouverture);
        let marge = (0.15 * ouverture).max(6.0);
        let a_zero = rendu_du_champ(&fond, ancre, champ_temp(Some(0.0)));

        for (valeur, attendu) in [(25.0f32, 0.25), (50.0, 0.5), (75.0, 0.75), (100.0, 1.0)] {
            let rendu = rendu_du_champ(&fond, ancre, champ_temp(Some(valeur)));
            dalle_bien_dimensionnee(&rendu, &format!("{ancre:?} à {valeur} °C"));
            let remplissage = couronne_seulement(pixels_changes(&a_zero, &rendu));
            let (_, fin) = etendue(&remplissage, secteur).unwrap_or_else(|| {
                panic!("{ancre:?} : {valeur} °C ne remplit rien alors que 0 °C est vide")
            });
            let voulu: f64 = attendu * ouverture;
            assert!(
                (fin - voulu).abs() <= marge,
                "{ancre:?} à {valeur} °C : l'arc va jusqu'à {fin:.1}° pour {voulu:.1}° attendus. \
                 L'échelle est celle du cadran — 0 °C vide l'arc, 100 °C le remplit"
            );
        }

        // Au-delà de cent degrés l'arc reste plein, en deçà de zéro il reste vide : une machine en
        // surchauffe ne doit pas afficher un arc qui se vide.
        let plein = couronne_seulement(pixels_changes(
            &a_zero,
            &rendu_du_champ(&fond, ancre, champ_temp(Some(100.0))),
        ));
        for canicule in [100.5f32, 150.0, 1.0e6] {
            let rendu = rendu_du_champ(&fond, ancre, champ_temp(Some(canicule)));
            let remplissage = couronne_seulement(pixels_changes(&a_zero, &rendu));
            assert_eq!(
                remplissage.len(),
                plein.len(),
                "{ancre:?} à {canicule} °C : l'arc reste plein, il ne repart pas à zéro"
            );
        }
        for gel in [-0.5f32, -40.0, -273.15] {
            let rendu = rendu_du_champ(&fond, ancre, champ_temp(Some(gel)));
            assert!(
                couronne_seulement(pixels_changes(&a_zero, &rendu)).is_empty(),
                "{ancre:?} à {gel} °C : l'arc reste vide, comme à 0 °C"
            );
        }
    }

    // Et l'échelle est nommée quelque part, une seule fois : c'est elle que le cadran et les arcs
    // partagent, et la partager par recopie de deux `/ 100.0` serait la faire diverger un jour.
    for (valeur, attendu) in [(0.0f32, 0.0f32), (25.0, 0.25), (50.0, 0.5), (100.0, 1.0)] {
        let obtenu = proportion_de_temperature(valeur);
        assert!(
            (obtenu - attendu).abs() <= 1.0e-6,
            "{valeur} °C vaut {attendu} sur l'échelle du cadran. Obtenu : {obtenu}"
        );
    }
}

#[test]
fn proportion_de_temperature_ecrete_au_lieu_de_deborder() {
    // Contrat de `proportion_de_temperature`. La valeur vient d'une sonde, et rien n'oblige une
    // sonde à rester entre 0 et 100 : un `k10temp` rend des négatifs au démarrage, un capteur
    // débranché rend ce qu'il veut. Une proportion hors bornes qui traverse un calcul de pixel sort
    // du tampon, et une dalle courte est ignorée en silence par le contrôleur.
    for (entree, attendu) in [
        (-0.001f32, 0.0f32),
        (-40.0, 0.0),
        (-273.15, 0.0),
        (f32::NEG_INFINITY, 0.0),
        (f32::MIN, 0.0),
        (100.001, 1.0),
        (99_999.0, 1.0),
        (f32::INFINITY, 1.0),
        (f32::MAX, 1.0),
    ] {
        let obtenu = proportion_de_temperature(entree);
        assert!(
            (obtenu - attendu).abs() <= 1.0e-6,
            "{entree} °C est écrêté à {attendu}, jamais replié. Obtenu : {obtenu}"
        );
    }

    // `NaN` n'est ni au-dessus ni au-dessous : il faut le nommer, sinon un `clamp` panique et le
    // démon emporte l'éclairage avec l'écran.
    let sur_nan = proportion_de_temperature(f32::NAN);
    assert!(
        sur_nan.is_finite() && (0.0..=1.0).contains(&sur_nan),
        "un NaN rend une proportion valide, sans paniquer. Obtenu : {sur_nan}"
    );
}

// ---------------------------------------------------------------------------
// 6 — une sonde muette
// ---------------------------------------------------------------------------

#[test]
fn une_sonde_muette_ecrit_des_tirets_et_vide_son_arc() {
    // Comportement à tester n° 3 de l'issue, et critère d'acceptation : « une sonde muette écrit des
    // tirets, et son arc est **vide** — jamais figé au dernier remplissage connu (règle de #68
    // appliquée à l'arc) ».
    //
    // Piège n° 4 du préambule. C'est le mode de défaillance le plus coûteux du projet, parce qu'il
    // est **rassurant** : un arc resté aux trois quarts derrière une pompe arrêtée, c'est un circuit
    // qui chauffe sans que rien ne le signale. Et l'arc est pire que le chiffre — on le lit d'un
    // coup d'œil, sans lire la valeur.
    let muette = champ_temp(None);
    let ecrit = valeur_du_champ(&muette);
    assert!(
        ecrit.contains('-'),
        "une sonde muette écrit des tirets. Obtenu : « {ecrit} »"
    );
    assert!(
        !ecrit.chars().any(|c| c.is_ascii_digit()),
        "aucun chiffre dans ce qu'écrit une sonde muette — « 0 » comme « 34.2 » y seraient une \
         information fausse. Obtenu : « {ecrit} »"
    );

    let fond = Dalle::unie(TEMOIN);
    for ancre in ANCRES_DE_COURONNE {
        let rendu_muet = rendu_du_champ(&fond, ancre, muette.clone());
        dalle_bien_dimensionnee(&rendu_muet, &format!("{ancre:?} sur une sonde muette"));

        // L'arc est vide, c'est-à-dire indistinguable de celui de 0 °C **dans la couronne**.
        let a_zero = rendu_du_champ(&fond, ancre, champ_temp(Some(0.0)));
        assert!(
            couronne_seulement(pixels_changes(&a_zero, &rendu_muet)).is_empty(),
            "{ancre:?} : l'arc d'une sonde muette est vide, comme celui de 0 °C"
        );

        // Et pourtant les deux dalles diffèrent : ce sont les tirets qui portent l'information, pas
        // l'arc. Sans cette assertion, « arc vide » serait satisfait par un champ qui affiche 0 °C.
        assert_ne!(
            rendu_muet.octets(),
            a_zero.octets(),
            "{ancre:?} : une sonde muette ne s'affiche pas comme 0 °C — le chiffre zéro se lit \
             comme une mesure, pas comme une absence"
        );

        // « Jamais figé au dernier remplissage » : l'arc d'une sonde muette diffère de celui de
        // toute valeur qui remplissait quelque chose.
        for derniere in [34.2f32, 80.0, 100.0] {
            let rendu_valeur = rendu_du_champ(&fond, ancre, champ_temp(Some(derniere)));
            assert!(
                !couronne_seulement(pixels_changes(&rendu_valeur, &rendu_muet)).is_empty(),
                "{ancre:?} : l'arc d'une sonde muette ne peut pas ressembler à celui de \
                 {derniere} °C — c'est exactement le figement que #68 interdit"
            );
        }

        // Le rendu est une fonction de ses arguments et de rien d'autre : composer une valeur puis
        // une absence redonne l'absence, octet pour octet. C'est la forme testable de « jamais la
        // dernière valeur connue ».
        let _ = rendu_du_champ(&fond, ancre, champ_temp(Some(100.0)));
        assert_eq!(
            rendu_du_champ(&fond, ancre, muette.clone()).octets(),
            rendu_muet.octets(),
            "{ancre:?} : aucune valeur d'avant ne survit d'un rendu au suivant"
        );
    }
}

// ---------------------------------------------------------------------------
// 7 — les secteurs ne se recouvrent pas
// ---------------------------------------------------------------------------

#[test]
fn les_secteurs_des_ancres_de_couronne_ne_se_recouvrent_pas() {
    // Comportement à tester n° 4 de l'issue, et critère d'acceptation : « les arcs ne se chevauchent
    // pas, quelles que soient les quatre ancres garnies — vérifié par calcul ».
    //
    // « Par calcul » est le mot de l'issue, et c'est le bon : quatre ancres garnies parmi cinq font
    // cinq combinaisons, mais deux arcs qui se recouvrent le font indépendamment de ce qui est posé
    // ailleurs. La disjonction deux à deux les couvre toutes, et elle se démontre sur les angles
    // plutôt que sur des pixels.
    for ancre in ANCRES_DE_COURONNE {
        let secteur = secteur_de(ancre);
        assert!(
            secteur.ouverture > 0.0,
            "{ancre:?} : un secteur a une ouverture strictement positive"
        );
        assert!(
            f64::from(secteur.ouverture) <= 360.0,
            "{ancre:?} : un secteur ne fait pas plus d'un tour"
        );
        assert!(
            secteur.debut.is_finite() && secteur.ouverture.is_finite(),
            "{ancre:?} : un secteur n'a ni NaN ni infini — ces angles servent à calculer des pixels"
        );
    }

    for (rang, &a) in ANCRES_DE_COURONNE.iter().enumerate() {
        for &b in &ANCRES_DE_COURONNE[rang + 1..] {
            let (sa, sb) = (secteur_de(a), secteur_de(b));
            assert!(
                !se_recouvrent(sa, sb),
                "{a:?} ({sa:?}) et {b:?} ({sb:?}) se recouvrent : deux températures se \
                 superposeraient sur la couronne, et on lirait l'une pour l'autre"
            );
        }
    }

    // Chaque arc est **au-dessus de l'ancre qu'il sert**. C'est le sens de « dans le secteur de son
    // ancre » : un arc de `bas` posé en haut de la dalle serait disjoint des autres et faux quand
    // même. La convention d'angle de ce fichier — 0° au sommet, horaire — donne les quatre cardinaux.
    for (ancre, cardinal) in [
        (Ancre::Haut, 0.0f64),
        (Ancre::Droite, 90.0),
        (Ancre::Bas, 180.0),
        (Ancre::Gauche, 270.0),
    ] {
        let secteur = secteur_de(ancre);
        assert!(
            secteur_contient(secteur, cardinal),
            "{ancre:?} : son secteur {secteur:?} doit couvrir {cardinal}°, c'est-à-dire se trouver \
             du côté de la dalle que son nom annonce"
        );
    }

    // La couronne, elle, tient dans le disque visible et a une épaisseur. Un anneau posé au-delà du
    // rayon visible ne s'afficherait nulle part, et le contrôleur avalerait l'image sans rien dire
    // (`SPEC-KRAKEN-LCD` §2.1.1).
    assert!(
        composition::COURONNE_RAYON_INTERIEUR < composition::COURONNE_RAYON_EXTERIEUR,
        "la couronne a une épaisseur : {} n'est pas inférieur à {}",
        composition::COURONNE_RAYON_INTERIEUR,
        composition::COURONNE_RAYON_EXTERIEUR
    );
    assert!(
        composition::COURONNE_RAYON_EXTERIEUR <= screen::VISIBLE_DISC_RADIUS,
        "la couronne tient dans le disque visible : {} dépasse {}",
        composition::COURONNE_RAYON_EXTERIEUR,
        screen::VISIBLE_DISC_RADIUS
    );
}

#[test]
fn deux_arcs_voisins_ne_partagent_aucun_pixel_de_la_couronne() {
    // Le même critère d'acceptation, mais vérifié là où il se voit : sur le tampon. Deux secteurs
    // disjoints au degré près peuvent encore partager un pixel d'anticrénelage si l'implémentation
    // les fait jointifs.
    //
    // Point n° 3 de ce que ce fichier tranche : deux arcs voisins laissent un intervalle. Collés,
    // ils se liraient comme un seul arc, et l'intérêt de la couronne — voir d'un coup d'œil où
    // chaque sonde se situe — disparaîtrait.
    let fond = Dalle::unie(TEMOIN);

    // Chacune seule, remplie à fond : c'est le cas où deux arcs voisins sont au plus près.
    let mut peints: Vec<(Ancre, BTreeSet<(u32, u32)>)> = Vec::new();
    for ancre in ANCRES_DE_COURONNE {
        let rendu = rendu_du_champ(&fond, ancre, champ_temp(Some(100.0)));
        peints.push((
            ancre,
            couronne_seulement(pixels_changes(&fond, &rendu))
                .into_iter()
                .collect(),
        ));
    }

    for (rang, (a, pixels_a)) in peints.iter().enumerate() {
        assert!(
            !pixels_a.is_empty(),
            "{a:?} à 100 °C doit peindre sa couronne, sinon la disjonction ne prouve rien"
        );
        for (b, pixels_b) in &peints[rang + 1..] {
            let partages = pixels_a.intersection(pixels_b).count();
            assert_eq!(
                partages, 0,
                "{a:?} et {b:?} partagent {partages} pixel(s) de couronne à 100 °C — deux arcs \
                 jointifs se lisent comme un seul"
            );
        }
    }

    // Et les quatre posées ensemble peignent exactement la réunion de ce que chacune peint seule :
    // aucun arc n'est raccourci par la présence d'un voisin, aucun n'en déborde.
    let quatre: Vec<(Ancre, ChampRendu)> = ANCRES_DE_COURONNE
        .iter()
        .map(|&ancre| (ancre, champ_temp(Some(100.0))))
        .collect();
    let ensemble = Dalle::composee(&fond, &quatre);
    dalle_bien_dimensionnee(&ensemble, "les quatre ancres de couronne garnies");
    let couronne_ensemble: BTreeSet<(u32, u32)> =
        couronne_seulement(pixels_changes(&fond, &ensemble))
            .into_iter()
            .collect();
    let reunion: BTreeSet<(u32, u32)> = peints
        .iter()
        .flat_map(|(_, pixels)| pixels.iter().copied())
        .collect();
    assert_eq!(
        couronne_ensemble, reunion,
        "les quatre arcs posés ensemble peignent exactement ce que chacun peint seul — ni plus, \
         donc aucun débordement, ni moins, donc aucun arc écrasé par son voisin"
    );
}

#[test]
fn l_ancre_centre_n_a_pas_d_arc() {
    // Comportement attendu de l'issue : « l'ancre `centre` n'a pas d'arc : elle n'est pas sur la
    // couronne. »
    //
    // Point n° 4 de ce que ce fichier tranche : c'est un `None`, pas un secteur d'ouverture nulle.
    // Un secteur vide serait une valeur que chaque appelant devrait penser à écarter, et qui
    // passerait silencieusement les tests de disjonction.
    assert!(
        Ancre::Centre.secteur().is_none(),
        "`centre` est au milieu de la dalle : aucun secteur de couronne ne lui correspond"
    );

    // Et cela se voit : un champ de température au centre, à cent degrés, ne peint pas la couronne.
    let fond = Dalle::unie(TEMOIN);
    let rendu = rendu_du_champ(&fond, Ancre::Centre, champ_temp(Some(100.0)));
    dalle_bien_dimensionnee(&rendu, "un champ de température au centre");
    assert!(
        couronne_seulement(pixels_changes(&fond, &rendu)).is_empty(),
        "un champ posé au centre ne dessine rien sur la couronne — il y empiéterait sur les arcs \
         des quatre autres ancres"
    );
    // Il écrit quand même sa valeur : « pas d'arc » ne veut pas dire « pas de champ ».
    assert_ne!(
        rendu.octets(),
        fond.octets(),
        "le champ central s'affiche : seul son arc n'existe pas"
    );
}

// ---------------------------------------------------------------------------
// 8 — un champ texte n'a pas d'arc
// ---------------------------------------------------------------------------

#[test]
fn un_champ_texte_ne_dessine_aucun_arc() {
    // Comportement à tester n° 6 de l'issue, et critère d'acceptation : « un champ `texte` n'a pas
    // d'arc ».
    //
    // Un arc dit une proportion. Sous « SHYNAEL », il en dirait une qui n'existe pas — et un arc
    // figé au même remplissage à chaque rendu se lirait comme une sonde en panne, c'est-à-dire
    // exactement le contraire de ce qu'il montre.
    let fond = Dalle::unie(TEMOIN);

    for ancre in Ancre::TOUTES {
        for contenu in ["SHYNAEL", "100 %", "soirée d'été", "0", "34.2"] {
            let rendu = rendu_du_champ(&fond, ancre, ChampRendu::Texte(contenu.to_owned()));
            dalle_bien_dimensionnee(&rendu, &format!("{ancre:?} portant le texte « {contenu} »"));
            let sur_la_couronne = couronne_seulement(pixels_changes(&fond, &rendu));
            assert!(
                sur_la_couronne.is_empty(),
                "{ancre:?} porte le texte « {contenu} » et peint {} pixel(s) de couronne — un texte \
                 fixe n'a aucune proportion à montrer, et « 34.2 » tapé à la main n'est pas une \
                 mesure",
                sur_la_couronne.len()
            );
        }
    }

    // Et le texte lui-même s'affiche bien : sans cette assertion, « aucun arc » serait satisfait par
    // un champ qui ne dessine rien du tout.
    for ancre in Ancre::TOUTES {
        let rendu = rendu_du_champ(&fond, ancre, ChampRendu::Texte("SHYNAEL".to_owned()));
        assert!(
            aire(&fond, &rendu) >= 20,
            "{ancre:?} : un champ de texte s'écrit sur la dalle"
        );
    }
}

// ---------------------------------------------------------------------------
// 9 — rien ne sort du disque, et quelque chose y entre
// ---------------------------------------------------------------------------

#[test]
fn rien_de_ce_qui_est_dessine_ne_sort_du_disque_et_quelque_chose_y_est_dessine() {
    // Comportement à tester n° 5 de l'issue : « Tout ce qui est dessiné reste dans le disque visible
    // — la dalle est ronde (§2.1.1). »
    //
    // Piège n° 3 du préambule : ce test passerait tout seul sur une implémentation qui ne dessine
    // rien. Chaque cas exige donc les **deux** — aucun pixel hors du disque, et un minimum de
    // pixels dedans. La dalle est ronde, 21 % du tampon ne s'affiche nulle part, et le contrôleur
    // avale l'image entière sans jamais dire qu'il en a jeté les coins.
    let minimum = 20usize;
    let controle = |avant: &Dalle, apres: &Dalle, quoi: &str| {
        dalle_bien_dimensionnee(apres, quoi);
        let changes = pixels_changes(avant, apres);
        let mut dedans = 0usize;
        for &(x, y) in &changes {
            assert!(
                !entierement_hors_du_disque(x, y),
                "{quoi} peint le pixel ({x}, {y}), entièrement hors du disque visible de rayon {} \
                 — la dalle est ronde, ce pixel n'existe pas, et le contrôleur ne dira jamais rien",
                screen::VISIBLE_DISC_RADIUS
            );
            dedans += 1;
        }
        assert!(
            dedans >= minimum,
            "{quoi} doit peindre au moins {minimum} pixel(s) dans le disque. Obtenu : {dedans} — \
             un dessin absent ne sort évidemment jamais du disque"
        );
    };

    let fond = Dalle::unie(TEMOIN);

    // La primitive de texte, sur les cinq boîtes et sur des chaînes démesurées : chacune est une
    // occasion d'écrire hors du tampon.
    let demesure = "un libellé démesurément long que personne n'écrirait, mais que rien n'empêche \
                    de taper — avec des accents et des espaces";
    for ancre in Ancre::TOUTES {
        for texte in ["8", "88.8", LIBELLE, demesure, &"W".repeat(200)] {
            controle(
                &fond,
                &Dalle::texte(&fond, texte, ancre.boite()),
                &format!("« {texte} » écrit dans la boîte de {ancre:?}"),
            );
        }
    }

    // Les arcs, à toutes les proportions et sur les quatre secteurs.
    for ancre in ANCRES_DE_COURONNE {
        let secteur = secteur_de(ancre);
        for proportion in [0.05f32, 0.25, 0.5, 0.75, 1.0] {
            controle(
                &fond,
                &Dalle::arc(&fond, secteur, proportion),
                &format!("l'arc de {ancre:?} rempli à {proportion}"),
            );
        }
    }

    // Le cadran, y compris sur les valeurs qui font déborder un dessin fait à la main.
    for valeur in [
        Some(34.2f32),
        Some(0.0),
        Some(100.0),
        Some(-273.15),
        Some(99_999.0),
        Some(f32::NAN),
        Some(f32::INFINITY),
        None,
    ] {
        controle(
            &Dalle::unie(NOIR),
            &Dalle::cadran(LIBELLE, valeur, "°C", 0.5),
            &format!("le cadran de {valeur:?}"),
        );
    }

    // Et la composition complète, telle que l'issue la décrit : quatre champs, arcs compris, dont
    // une sonde muette et un texte fixe.
    let complete = [
        (Ancre::Haut, champ_temp(Some(34.2))),
        (
            Ancre::Gauche,
            ChampRendu::Temperature {
                libelle: Some("CPU".to_owned()),
                valeur: Some(72.5),
            },
        ),
        (
            Ancre::Droite,
            ChampRendu::Temperature {
                libelle: Some("GPU".to_owned()),
                valeur: None,
            },
        ),
        (Ancre::Bas, ChampRendu::Texte("SHYNAEL".to_owned())),
    ];
    controle(
        &fond,
        &Dalle::composee(&fond, &complete),
        "la composition complète de l'issue",
    );

    // Les cinq ancres à la fois — au-delà du plafond de quatre, exprès : c'est la géométrie qu'on
    // regarde ici, pas la règle de composition, et le champ central doit tenir entre ses voisins.
    let cinq: Vec<(Ancre, ChampRendu)> = Ancre::TOUTES
        .iter()
        .map(|&ancre| (ancre, champ_temp(Some(88.8))))
        .collect();
    controle(
        &fond,
        &Dalle::composee(&fond, &cinq),
        "les cinq ancres garnies d'une température",
    );
}

// ---------------------------------------------------------------------------
// Un garde-fou sur les repères de ce fichier
// ---------------------------------------------------------------------------

#[test]
fn la_convention_d_angle_de_ce_fichier_est_celle_qu_il_annonce() {
    // Ce fichier fixe une convention — 0° au sommet, croissant dans le sens horaire — et toutes ses
    // mesures d'étendue en dépendent. Si `angle_du_pixel` se trompait de sens, les tests d'arc
    // continueraient de passer sur une implémentation qui remplit à l'envers.
    //
    // Ce test ne vérifie donc pas le code du démon : il vérifie l'outil de mesure. C'est le seul du
    // fichier dans ce cas, et il est là pour que les autres veuillent dire quelque chose.
    let milieu = LARGEUR / 2;
    let bord = 10u32;
    for (x, y, attendu, ou) in [
        (milieu, bord, 0.0f64, "le sommet"),
        (LARGEUR - bord, milieu, 90.0, "la droite"),
        (milieu, HAUTEUR - bord, 180.0, "le bas"),
        (bord, milieu, 270.0, "la gauche"),
    ] {
        let obtenu = angle_du_pixel(x, y);
        assert!(
            (obtenu - attendu).abs() <= 1.0,
            "{ou} de la dalle est à {attendu}°, pas à {obtenu:.1}°"
        );
    }

    // Et `se_recouvrent` sait dire oui : un prédicat qui rendrait toujours `false` ferait passer la
    // disjonction des secteurs quoi qu'ils vaillent.
    let a = Secteur {
        debut: 350.0,
        ouverture: 40.0,
    };
    assert!(
        se_recouvrent(
            a,
            Secteur {
                debut: 10.0,
                ouverture: 20.0
            }
        ),
        "deux secteurs qui se chevauchent par-dessus 0° doivent être vus comme tels"
    );
    assert!(
        !se_recouvrent(
            a,
            Secteur {
                debut: 40.0,
                ouverture: 20.0
            }
        ),
        "deux secteurs qui se suivent sans se toucher sont disjoints"
    );
    assert!(
        secteur_contient(a, 0.0),
        "un secteur qui enjambe 0° contient 0°"
    );
}

// ---------------------------------------------------------------------------
// Un repère de lecture, pour que `Boite` ne soit pas importée pour rien
// ---------------------------------------------------------------------------

#[test]
fn la_primitive_de_texte_respecte_la_boite_qu_on_lui_donne() {
    // Contrat de `Dalle::texte` : « `texte` écrit dans la fonte embarquée, sur ce fond, dans cette
    // boîte ». La boîte est un argument, pas une décoration : deux boîtes différentes doivent
    // donner deux tampons différents, sinon le texte se poserait toujours au même endroit et les
    // cinq ancres n'auraient plus de sens.
    let fond = Dalle::unie(TEMOIN);
    let haut: Boite = Ancre::Haut.boite();
    let bas: Boite = Ancre::Bas.boite();
    assert_ne!(haut, bas, "les deux boîtes témoins diffèrent bien");

    let en_haut = Dalle::texte(&fond, "88.8", haut);
    let en_bas = Dalle::texte(&fond, "88.8", bas);
    assert_ne!(
        en_haut.octets(),
        en_bas.octets(),
        "le même texte dans deux boîtes différentes ne rend pas la même dalle"
    );

    // Et il se pose bien du côté qu'on lui indique : le barycentre vertical de ce qui est peint
    // suit la boîte. C'est une propriété, pas une position au pixel — un test d'intention ne fige
    // pas un interlettrage.
    let hauteur_moyenne = |dalle: &Dalle| -> f64 {
        let pixels = pixels_changes(&fond, dalle);
        assert!(
            !pixels.is_empty(),
            "il faut des pixels pour en faire la moyenne"
        );
        pixels.iter().map(|&(_, y)| f64::from(y)).sum::<f64>() / pixels.len() as f64
    };
    assert!(
        hauteur_moyenne(&en_haut) < hauteur_moyenne(&en_bas),
        "le texte de la boîte haute se dessine au-dessus de celui de la boîte basse"
    );
}
