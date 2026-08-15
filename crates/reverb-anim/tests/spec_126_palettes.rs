//! Tests d'intention des palettes de couleur (issue #126).
//!
//! Écrits **depuis l'issue #126 seule**, avant l'implémentation. Aucun corps de fonction de
//! `crates/reverb-anim/src/` n'a été relu pour les écrire ; seules ont servi les **signatures
//! publiques** nécessaires à la compilation — relevées au `grep` —, les tests d'intention déjà
//! présents dans ce répertoire, pour les idiomes d'appel du catalogue, et `README.md`. Si l'un de
//! ces tests échoue après implémentation, c'est le code qu'on corrige.
//!
//! # L'API que ces tests supposent
//!
//! Rien de ce qui suit n'existe encore : c'est la phase rouge attendue.
//!
//! ```ignore
//! /// Les douze palettes, dans l'ordre du tableau de l'issue.
//! pub const PALETTES: &[&str] = &[
//!     "light-pink", "lava", "ocean", "paysage", "couchant", "aurore",
//!     "atlantica", "sakura", "nuit-avril", "glace", "orange-teal", "sorbet",
//! ];
//!
//! /// Un dégradé à arrêts, repris de WLED. Données pures, aucune IO.
//! #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//! pub struct Palette { /* … */ }
//!
//! impl Palette {
//!     /// Ouvre une palette par son nom. Un nom inconnu est refusé en donnant la liste.
//!     pub fn par_nom(nom: &str) -> Result<Palette, PaletteInconnue>;
//!     /// Le nom sous lequel elle s'écrit sur le socket et dans les fichiers d'état.
//!     pub fn nom(&self) -> &'static str;
//!     /// Ses arrêts, position croissante, au moins deux.
//!     pub fn arrets(&self) -> &'static [(u8, Rgb)];
//!     /// La couleur à cette position. Voir l'arbitrage n° 1 sur l'échelle.
//!     pub fn echantillon(&self, position: f32) -> Rgb;
//! }
//!
//! /// Erreur rendue lorsqu'un nom de palette ne figure pas dans [`PALETTES`].
//! /// Calquée sur `AnimationInconnue`, et pour la même raison : le refus est la seule
//! /// documentation que l'utilisateur ait sous la main au moment où il en a besoin.
//! pub struct PaletteInconnue { pub saisi: String, pub valides: &'static [&'static str] }
//!
//! pub struct Reglages {
//!     pub couleur: Rgb,
//!     pub vitesse: u8,
//!     pub direction: Direction,
//!     pub sonde: Option<String>,
//!     /// Le dégradé que la famille échantillonne. Exclusif de `couleur`.
//!     pub palette: Option<Palette>,
//! }
//! ```
//!
//! # Trois arbitrages que l'issue laisse ouverts, et qui sont tranchés ici
//!
//! 1. ⚠️ **`echantillon` prend la position sur l'échelle des arrêts, `0.0..=255.0`.** L'issue
//!    écrit « une liste d'arrêts `(position 0..=255, Rgb)`, et `echantillon(position: f32)`
//!    interpole » sans dire sur quelle échelle vit l'argument, alors que les familles calculent,
//!    elles, une position **normalisée**. Le choix se joue sur un critère de l'issue : « les
//!    bornes rendent **exactement** la couleur d'arrêt ». Sur `0.0..=1.0`, un appelant qui veut
//!    l'arrêt `k` passe `k as f32 / 255.0`, que l'implémentation remultiplie par 255 — et
//!    `k / 255.0 * 255.0` ne rend pas `k` au bit près en `f32`. L'exactitude promise dépendrait
//!    alors de l'ordre des opérations plutôt que du contrat. Sur l'échelle des arrêts, elle est
//!    une propriété. Le prix est un `* 255.0` sur chaque site d'appel du catalogue.
//!
//! 2. **`reglages_ecrits` écrit `couleur` **ou** `palette`, jamais les deux.** Ce n'est pas un
//!    choix de confort : les deux clés ensemble sont refusées à la relecture, donc les écrire
//!    toutes les deux produirait un `eclairage.conf` que le démon refuserait au redémarrage
//!    suivant. Un état qui ne se relit pas est exactement le défaut de #69 — « un affichage
//!    impossible persisté faisait redémarrer le démon dans un état cassé ».
//!    ⚠️ **Conséquence : `Reglages { couleur: <autre que le défaut>, palette: Some(…) }` ne fait
//!    pas l'aller-retour**, puisque sa couleur n'est pas écrite. C'est cohérent — l'exclusivité
//!    dit que cette couleur ne peint rien — et c'est pourquoi le test de persistance construit son
//!    témoin avec la couleur par défaut.
//!
//! 3. **`palette` est acceptée exactement là où `couleur` l'est**, ni plus ni moins. L'issue dit
//!    « accepté là où `couleur` l'est » d'un côté et « `arc-en-ciel` refuse `palette`, comme elle
//!    refuse déjà `couleur` » de l'autre : les deux moitiés font une équivalence, et c'est elle qui
//!    est figée ici. `thermique` la refuse au même titre — elle tient sa couleur d'une mesure.
//!
//! # Ce que ce fichier ne vérifie pas, et où ça vit
//!
//! ⚠️ La persistance de bout en bout — `eclairage.conf`, `zones.conf`, profils — et le rendu du
//! réglage par `status` / `lighting` appartiennent au démon et au protocole. Ce fichier vérifie la
//! seule moitié qui vive dans `reverb-anim` : que `reglages_ecrits` et `reglages` forment un
//! aller-retour fidèle, ce sur quoi ces trois fichiers reposent.
//!
//! ⚠️ Le critère « les données de palette sont accompagnées de leur attribution » est une exigence
//! de licence, pas de comportement : elle se lit dans le source et dans le `LICENSE` du dépôt, et
//! aucun test ne la remplacerait.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi.

use reverb_anim::{Animation, CATALOGUE, Direction, Geometrie, Image, Palette, Reglages};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// Le domaine
// ---------------------------------------------------------------------------

/// Les douze palettes, dans l'ordre du tableau de l'issue, écrites en dur.
///
/// Écrites ici et non lues chez le code : c'est la liste que l'issue arrête, et un test qui la
/// lirait dans `PALETTES` serait vert quelle que soit la liste livrée.
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

/// Celle que Nico utilise sur ses bandes WLED, et celle que l'issue nomme dans ses tests.
const LIGHT_PINK: &str = "light-pink";

/// Durée d'un cycle, en pas, à la vitesse 1 — figée par `spec_sens.rs` (issue #49).
const PERIODE: u32 = 120;

/// Une couleur dont les trois composantes diffèrent deux à deux, comme dans `spec_animations.rs`.
const TEMOIN: Rgb = Rgb::new(0xff, 0x20, 0x80);

/// Le nombre de LED du boîtier : 10 × 8 + 4 × 11.
const LED_DU_BOITIER: usize = 124;

// ---------------------------------------------------------------------------
// Les seuils de teinte, et d'où ils viennent
// ---------------------------------------------------------------------------

/// En deçà de cette composante maximale, une LED n'a pas de teinte lisible.
///
/// Une LED éteinte n'a **aucune** teinte, et une LED presque éteinte n'en a qu'une très arrondie :
/// à `max = 8`, un rapport de composantes ne porte que trois bits. Le plancher borne l'arrondi —
/// à `max ≥ 96`, l'erreur sur une composante normalisée est d'au plus `0,5 / 96 ≈ 0,005`.
///
/// ⚠️ Sans ce plancher, le test de l'échantillonnage serait **trivialement vrai** : deux LED
/// éteintes rendues `(0, 0, 0)` et `(1, 0, 0)` porteraient déjà « deux teintes distinctes ».
const SEUIL_ALLUMEE: u8 = 96;

/// Deux teintes sont **distinctes** au-delà de cet écart, composante normalisée par composante.
///
/// Quinze centièmes, soit quinze fois le bruit d'arrondi mesuré ci-dessous. C'est un écart qui se
/// voit sur un boîtier, pas une différence d'arrondi promue en propriété.
const SEPARATION_TEINTE: f32 = 0.15;

/// Écart de teinte au-delà duquel deux LED ne portent plus la **même** teinte.
///
/// Mesuré sur `main` à la révision `dadf093`, avec `couleur=ff2080`, vitesse 3, les huit
/// directions, un cycle entier et le plancher ci-dessus : l'écart maximal entre deux LED allumées
/// vaut **0,0103** sur `vague`, `comete`, `respiration`, `balayage`, `braise` et `pouls` — c'est
/// l'arrondi vers l'octet, rien d'autre. Le seuil est posé au double, et à un septième de
/// [`SEPARATION_TEINTE`] : les deux populations ne se touchent pas.
const TOLERANCE_TEINTE: f32 = 0.02;

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
}

fn palette(nom: &str) -> Palette {
    Palette::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est une palette : {erreur}"))
}

fn paire(cle: &str, valeur: &str) -> (String, String) {
    (cle.to_owned(), valeur.to_owned())
}

fn geometrie() -> Geometrie {
    Geometrie::mesuree()
}

/// L'écriture d'une couleur sur le socket : six chiffres hexadécimaux minuscules.
fn hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

/// Les familles qui acceptent la clé donnée.
///
/// ⚠️ **Elle refuse de rendre une liste vide**, et c'est un garde-fou de non-vacuité, pas une
/// commodité : tant que `palette` n'est déclarée par personne, tout test qui boucle là-dessus est
/// **vert sans rien vérifier**. Mesuré — dix des douze tests de ce fichier passaient contre une
/// implémentation qui n'avait pas encore déclaré la clé.
fn familles_avec(cle: &str) -> Vec<Animation> {
    let familles: Vec<Animation> = CATALOGUE
        .iter()
        .map(|nom| animation(nom))
        .filter(|animation| animation.parametres_acceptes().contains(&cle))
        .collect();
    assert!(
        !familles.is_empty(),
        "aucune famille du catalogue n'accepte « {cle} » : le test qui boucle sur cette liste ne \
         vérifie rien — {CATALOGUE:?}"
    );
    familles
}

/// La couleur d'une LED désignée par son rang : les dix ventilateurs, puis les quatre barrettes.
///
/// Les ventilateurs sont cherchés **par position** et jamais par indice de tableau : l'`Image`
/// porte la position à côté des couleurs précisément pour que personne n'ait à connaître l'ordre
/// du tableau.
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

/// La teinte d'une LED : ses composantes ramenées à la plus grande.
///
/// Rend `None` quand la LED est trop sombre pour porter une teinte lisible — voir
/// [`SEUIL_ALLUMEE`]. Deux LED d'une même couleur à deux intensités ont la **même** teinte : c'est
/// exactement ce qu'il faut ici, puisqu'une famille applique `couleur × intensité` et que le
/// critère porte sur ce que la palette ajoute, pas sur l'enveloppe du motif.
fn teinte(couleur: Rgb) -> Option<[f32; 3]> {
    let plus_grande = couleur.r.max(couleur.g).max(couleur.b);
    if plus_grande < SEUIL_ALLUMEE {
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

/// Les teintes des LED allumées d'une image, avec le rang de la LED qui les porte.
fn teintes(image: &Image) -> Vec<(usize, [f32; 3])> {
    (0..LED_DU_BOITIER)
        .filter_map(|rang| teinte(couleur_par_rang(image, rang)).map(|teinte| (rang, teinte)))
        .collect()
}

/// Le plus grand écart de teinte d'une image, avec les deux LED qui le portent.
fn plus_grand_ecart(image: &Image) -> Option<(usize, usize, f32)> {
    let teintes = teintes(image);
    let mut pire: Option<(usize, usize, f32)> = None;
    for (rang, (numero, une)) in teintes.iter().enumerate() {
        for (autre_numero, autre) in &teintes[rang + 1..] {
            let ecart = ecart_de_teinte(*une, *autre);
            if pire.is_none_or(|(_, _, vu)| ecart > vu) {
                pire = Some((*numero, *autre_numero, ecart));
            }
        }
    }
    pire
}

// ---------------------------------------------------------------------------
// 1 — les douze palettes
// ---------------------------------------------------------------------------

#[test]
fn les_douze_palettes_sont_celles_de_l_issue_et_portent_des_slugs() {
    // Critère d'acceptation : « Douze palettes, reprises de WLED, toutes issues de `palettes.cpp`
    // (donc une seule provenance et une seule attribution) ».
    //
    // Les noms traversent le protocole comme des jetons séparés par des espaces — `animate vague
    // palette=light-pink` —, donc la même règle de slug que les noms d'animation et les positions
    // physiques : ASCII, minuscules, sans espace. Un nom accentué serait refusé par le décodeur du
    // socket sans que rien ne dise pourquoi.
    assert_eq!(
        LED_DU_BOITIER,
        Position::ALL.len() * LEDS_PER_FAN as usize + SLOT_COUNT * LEDS_PER_STICK,
        "appareil : 10 × 8 + 4 × 11"
    );
    assert_eq!(
        reverb_anim::PALETTES,
        DOUZE.as_slice(),
        "les douze palettes de l'issue, dans l'ordre de son tableau"
    );

    let mut vus: Vec<&str> = reverb_anim::PALETTES.to_vec();
    vus.sort_unstable();
    vus.dedup();
    assert_eq!(
        vus.len(),
        reverb_anim::PALETTES.len(),
        "deux palettes ne peuvent pas partager un nom : {:?}",
        reverb_anim::PALETTES
    );

    for nom in DOUZE {
        assert!(nom.is_ascii(), "« {nom} » n'est pas en ASCII");
        assert!(!nom.contains(' '), "« {nom} » contient une espace");
        assert_eq!(nom, nom.to_lowercase(), "« {nom} » n'est pas en minuscules");

        let palette = palette(nom);
        assert_eq!(
            palette.nom(),
            nom,
            "la palette ouverte par « {nom} » doit se nommer « {nom} »"
        );

        // Un dégradé, donc au moins deux arrêts, aux positions strictement croissantes. Un seul
        // arrêt ne serait pas un dégradé mais une couleur, et deux arrêts à la même position
        // rendraient l'interpolation ambiguë — laquelle des deux couleurs y répond ?
        let arrets = palette.arrets();
        assert!(
            arrets.len() >= 2,
            "« {nom} » n'a que {} arrêt(s) : ce n'est pas un dégradé",
            arrets.len()
        );
        for couple in arrets.windows(2) {
            assert!(
                couple[0].0 < couple[1].0,
                "« {nom} » : l'arrêt {} ne précède pas l'arrêt {} — les positions doivent être \
                 strictement croissantes",
                couple[0].0,
                couple[1].0
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 2 — l'interpolation
// ---------------------------------------------------------------------------

#[test]
fn chaque_palette_rend_exactement_sa_couleur_a_chaque_arret() {
    // Test d'intention n° 1 de l'issue — « une palette rend ses couleurs d'arrêt exactement aux
    // positions d'arrêt » —, et critère d'acceptation : « les bornes rendent exactement la couleur
    // d'arrêt ».
    //
    // « Exactement » se lit au bit près, sur les trois composantes. Un échantillonnage qui rendrait
    // la bonne couleur à une unité près partout serait invisible à l'œil et rendrait ce contrat
    // faux : c'est la seule position où l'on sait, sans calculer, ce que la palette doit répondre.
    //
    // ⚠️ L'échelle de l'argument est celle des arrêts — voir l'arbitrage n° 1 en tête de fichier.
    for nom in DOUZE {
        let palette = palette(nom);
        for (position, attendue) in palette.arrets() {
            let rendue = palette.echantillon(f32::from(*position));
            assert_eq!(
                rendue,
                *attendue,
                "« {nom} » à l'arrêt {position} rend {} au lieu de {}",
                hexa(rendue),
                hexa(*attendue)
            );
        }
    }
}

#[test]
fn entre_deux_arrets_l_echantillon_reste_entre_les_deux_couleurs() {
    // Test d'intention n° 2 de l'issue — « entre deux arrêts, l'échantillon est **entre** les deux
    // couleurs, composante par composante » —, et critère d'acceptation : « l'échantillonnage est
    // une interpolation linéaire entre arrêts ».
    //
    // ⚠️ **« Entre » ne suffit pas, et il faut les deux moitiés.** Un échantillonnage par plus
    // proche voisin — un escalier — rend toujours l'une des deux couleurs d'arrêt, donc satisfait
    // l'encadrement sans interpoler quoi que ce soit. La seconde moitié le condamne : sur un
    // segment dont une composante varie d'au moins quatre unités, le milieu doit tomber
    // **strictement** entre les deux, ce qu'aucun escalier ne fait. Quatre unités parce que
    // l'arrondi vers l'octet ne peut décaler le milieu que d'une demie : à partir de quatre, il
    // reste au moins une unité de marge de chaque côté.
    let mut segments_stricts = 0usize;

    for nom in DOUZE {
        let palette = palette(nom);
        for couple in palette.arrets().windows(2) {
            let (debut, avant) = couple[0];
            let (fin, apres) = couple[1];
            let etendue = f32::from(fin) - f32::from(debut);

            for millieme in [1u32, 250, 500, 750, 999] {
                let part = millieme as f32 / 1000.0;
                let position = f32::from(debut) + part * etendue;
                let rendue = palette.echantillon(position);
                for (composante, ici, la, rendu) in [
                    ("rouge", avant.r, apres.r, rendue.r),
                    ("vert", avant.g, apres.g, rendue.g),
                    ("bleu", avant.b, apres.b, rendue.b),
                ] {
                    let bas = ici.min(la);
                    let haut = ici.max(la);
                    assert!(
                        rendu >= bas && rendu <= haut,
                        "« {nom} » à la position {position:.1}, entre les arrêts {debut} ({}) et \
                         {fin} ({}) : le {composante} vaut {rendu}, hors de [{bas}, {haut}] — \
                         l'échantillon sort du segment qui l'encadre",
                        hexa(avant),
                        hexa(apres)
                    );
                }
            }

            // Le milieu, pour les segments où l'arrondi ne peut pas masquer l'interpolation.
            let milieu = palette.echantillon(f32::from(debut) + etendue / 2.0);
            for (composante, ici, la, rendu) in [
                ("rouge", avant.r, apres.r, milieu.r),
                ("vert", avant.g, apres.g, milieu.g),
                ("bleu", avant.b, apres.b, milieu.b),
            ] {
                if ici.abs_diff(la) < 4 {
                    continue;
                }
                segments_stricts += 1;
                let bas = ici.min(la);
                let haut = ici.max(la);
                assert!(
                    rendu > bas && rendu < haut,
                    "« {nom} » au milieu des arrêts {debut} ({}) et {fin} ({}) : le {composante} \
                     vaut {rendu}, l'une des deux bornes [{bas}, {haut}] — c'est un escalier, pas \
                     une interpolation",
                    hexa(avant),
                    hexa(apres)
                );
            }
        }
    }

    // Appareil : sans segment assez contrasté, la moitié « strictement entre » n'aurait rien
    // vérifié, et le test serait vert en ne mesurant rien.
    assert!(
        segments_stricts >= 12,
        "seulement {segments_stricts} composante(s) de segment varient d'au moins quatre unités \
         sur les douze palettes : l'escalier ne serait pas détecté"
    );
}

#[test]
fn hors_bornes_l_echantillon_est_ramene_a_la_couleur_de_borne() {
    // Test d'intention n° 3 de l'issue — « hors bornes, l'échantillon est ramené à la couleur de
    // borne, jamais extrapolé » —, et critère d'acceptation : « pas d'extrapolation ».
    //
    // C'est la même règle que la courbe de régulation, et pour la même raison (README, « ramenée
    // aux bornes au-delà — jamais extrapolée ») : prolonger la droite du premier segment donne une
    // couleur que personne n'a choisie, et qui sort du dégradé sans le dire. Une composante
    // extrapolée déborderait de surcroît l'octet, où elle serait tronquée en silence.
    //
    // ⚠️ Les bornes sont **lues chez la palette**, jamais supposées à 0 et 255 : rien dans l'issue
    // ne dit qu'un `_gp` de WLED couvre toute l'échelle, et un test qui l'écrirait figerait une
    // donnée qu'il ne connaît pas.
    for nom in DOUZE {
        let palette = palette(nom);
        let arrets = palette.arrets();
        let (premiere_position, premiere) = arrets[0];
        let (derniere_position, derniere) = arrets[arrets.len() - 1];

        for sous in [-1000.0f32, -255.0, -1.0, -0.5, -0.001] {
            let position = f32::from(premiere_position) + sous;
            let rendue = palette.echantillon(position);
            assert_eq!(
                rendue,
                premiere,
                "« {nom} » à la position {position} — sous le premier arrêt ({premiere_position}) \
                 — rend {} au lieu de {} : la palette est extrapolée",
                hexa(rendue),
                hexa(premiere)
            );
        }
        for au_dela in [0.001f32, 0.5, 1.0, 255.0, 1000.0] {
            let position = f32::from(derniere_position) + au_dela;
            let rendue = palette.echantillon(position);
            assert_eq!(
                rendue,
                derniere,
                "« {nom} » à la position {position} — au-delà du dernier arrêt \
                 ({derniere_position}) — rend {} au lieu de {} : la palette est extrapolée",
                hexa(rendue),
                hexa(derniere)
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — les refus
// ---------------------------------------------------------------------------

#[test]
fn palette_est_acceptee_exactement_la_ou_couleur_l_est() {
    // Critère d'acceptation : « `animate <famille> palette=<nom>` est accepté par toute famille qui
    // accepte `couleur` », et approche technique : « `arc-en-ciel` […] par défaut elle **refuse**
    // `palette`, comme elle refuse déjà `couleur` ».
    //
    // Les deux moitiés font une équivalence, et c'est elle qu'on fige — voir l'arbitrage n° 3. Elle
    // se justifie de part et d'autre : une famille qui produit ses propres teintes n'a rien à
    // échantillonner, et une famille qui accepte une couleur unique accepte a fortiori un dégradé,
    // puisque c'est exactement au même endroit du calcul qu'il se substitue.
    //
    // ⚠️ `parametres_acceptes` est **ce que la fenêtre lit** pour décider d'afficher le menu des
    // palettes (#20, #119) : une famille qui l'accepterait sans le déclarer n'aurait pas de menu, et
    // une famille qui le déclarerait sans l'accepter offrirait un menu dont chaque choix ferait
    // rejeter l'`animate` **entier**.
    let mut avec = 0usize;
    let mut sans = 0usize;

    for nom in CATALOGUE {
        let animation = animation(nom);
        let acceptes = animation.parametres_acceptes();
        let colore = acceptes.contains(&"couleur");
        assert_eq!(
            acceptes.contains(&"palette"),
            colore,
            "« {nom} » accepte `couleur` = {colore} et `palette` = {} : une palette se donne \
             exactement là où une couleur se donne — {acceptes:?}",
            acceptes.contains(&"palette")
        );

        if colore {
            avec += 1;
            let lus = animation
                .reglages(&[paire("palette", LIGHT_PINK)])
                .unwrap_or_else(|erreur| {
                    panic!("« {nom} » doit accepter « palette={LIGHT_PINK} » : {erreur}")
                });
            assert_eq!(
                lus.palette,
                Some(palette(LIGHT_PINK)),
                "« {nom} » : « palette={LIGHT_PINK} » doit atterrir dans le champ `palette`"
            );
        } else {
            sans += 1;
            let erreur = animation
                .reglages(&[paire("palette", LIGHT_PINK)])
                .expect_err("une famille qui produit ses teintes refuse une palette");
            assert_eq!(
                erreur.cle, "palette",
                "« {nom} » doit nommer la clé fautive : {erreur}"
            );
        }
    }

    // Appareil : les deux camps existent. `arc-en-ciel` et `thermique` d'un côté, les huit autres
    // familles de l'autre — un catalogue où tout le monde accepterait ne vérifierait qu'une moitié.
    assert!(
        avec >= 2 && sans >= 1,
        "{avec} famille(s) acceptent `couleur` et {sans} la refusent : le test ne vérifie qu'un \
         seul camp"
    );
}

#[test]
fn couleur_et_palette_ensemble_sont_refuses_en_nommant_les_deux_cles() {
    // Test d'intention n° 4 de l'issue — « `palette=` + `couleur=` est refusé, et le message nomme
    // les deux clés » —, et critère d'acceptation : « `couleur` et `palette` ensemble sont refusés,
    // en nommant le conflit ».
    //
    // L'issue dit pourquoi ce n'est pas une clé de trop : « c'est une ambiguïté — rien ne dirait
    // laquelle gagne ». Le refus doit donc dire **le conflit**, et non « clé inconnue » ni « valeur
    // invalide » : celui qui vient de taper les deux ne cherche pas laquelle est mal écrite, il
    // cherche à savoir qu'elles s'excluent.
    //
    // ⚠️ **Le message est jugé sur sa `raison`, jamais sur son rendu complet.** `ReglageInvalide`
    // affiche déjà « Réglages acceptés : couleur, vitesse, direction, palette » : un test qui
    // chercherait les deux mots dans `to_string()` serait **trivialement vert**, la liste des clés
    // acceptées les contenant toutes les deux. On exige donc que la clé nommée soit l'une des deux,
    // et que la raison nomme l'autre — l'union des deux couvre le conflit, et rien de moins ne le
    // couvre.
    for animation in familles_avec("couleur") {
        let nom = animation.nom();
        for ordre in [
            vec![
                paire("couleur", &hexa(TEMOIN)),
                paire("palette", LIGHT_PINK),
            ],
            vec![
                paire("palette", LIGHT_PINK),
                paire("couleur", &hexa(TEMOIN)),
            ],
            // Entourées de clés valides : un décodeur qui s'arrêterait à la première paire
            // acceptable appliquerait la moitié des réglages sans rien dire.
            vec![
                paire("vitesse", "5"),
                paire("couleur", &hexa(TEMOIN)),
                paire("palette", LIGHT_PINK),
            ],
        ] {
            let erreur = animation
                .reglages(&ordre)
                .expect_err("« couleur » et « palette » ensemble sont refusés");

            assert!(
                erreur.cle == "couleur" || erreur.cle == "palette",
                "« {nom} » : le refus nomme « {} », qui n'est ni « couleur » ni « palette » — {erreur}",
                erreur.cle
            );
            let autre = if erreur.cle == "palette" {
                "couleur"
            } else {
                "palette"
            };
            assert!(
                erreur.raison.contains(autre),
                "« {nom} » : le refus nomme « {} » mais sa raison ne dit pas « {autre} » — les deux \
                 clés doivent être nommées, sinon rien ne dit que c'est un conflit et non une clé \
                 mal écrite. Raison : « {} »",
                erreur.cle,
                erreur.raison
            );

            let message = erreur.to_string();
            assert!(
                message.contains("couleur") && message.contains("palette"),
                "le message rendu à l'utilisateur doit nommer les deux clés : « {message} »"
            );
            let _: &dyn std::error::Error = &erreur;
        }

        // Et chacune **seule** passe : l'issue refuse leur réunion, pas l'une des deux.
        let seule_couleur = animation
            .reglages(&[paire("couleur", &hexa(TEMOIN))])
            .unwrap_or_else(|erreur| panic!("« {nom} » accepte une couleur seule : {erreur}"));
        assert_eq!(seule_couleur.couleur, TEMOIN);
        assert_eq!(
            seule_couleur.palette, None,
            "« {nom} » : une couleur seule ne doit pas poser de palette"
        );

        let seule_palette = animation
            .reglages(&[paire("palette", LIGHT_PINK)])
            .unwrap_or_else(|erreur| panic!("« {nom} » accepte une palette seule : {erreur}"));
        assert_eq!(seule_palette.palette, Some(palette(LIGHT_PINK)));
    }
}

#[test]
fn une_palette_inconnue_est_refusee_en_listant_les_douze() {
    // Test d'intention n° 5 de l'issue — « une palette inconnue est refusée, et le message contient
    // les douze noms » —, et critère d'acceptation : « une palette inconnue est refusée **en
    // donnant la liste** ».
    //
    // L'issue le rattache explicitement au précédent : « comme une sonde inconnue de `thermique` ».
    // Citer les noms valides n'est pas une politesse — c'est la seule documentation que
    // l'utilisateur ait sous la main au moment où il en a besoin, sur un socket sans complétion.
    //
    // ⚠️ Contrairement au conflit ci-dessus, chercher les douze noms dans le message rendu n'a rien
    // de trivial : la liste des clés acceptées ne contient aucun nom de palette.
    let saisies = [
        "bidule",
        "light_pink",
        "Light-Pink",
        "LIGHT-PINK",
        "light pink",
        "lightpink",
        "pink",
        "rainbow",
        "",
        " ",
        "0",
        "🌈",
    ];

    for saisi in saisies {
        // Par la porte du catalogue de palettes…
        let erreur = Palette::par_nom(saisi).expect_err("un nom hors des douze est refusé");
        let message = erreur.to_string();
        for connue in DOUZE {
            assert!(
                message.contains(connue),
                "le refus de « {saisi} » doit citer « {connue} » : « {message} »"
            );
        }
        if !saisi.trim().is_empty() {
            assert!(
                message.contains(saisi),
                "le refus doit citer le nom reçu, tel qu'il a été reçu : « {message} »"
            );
        }
        let _: &dyn std::error::Error = &erreur;

        // … et par celle du réglage, qui est celle que l'utilisateur emprunte vraiment.
        for animation in familles_avec("palette") {
            let erreur = animation
                .reglages(&[paire("palette", saisi)])
                .expect_err("un nom hors des douze est refusé comme réglage");
            assert_eq!(
                erreur.cle,
                "palette",
                "« {} » : le refus doit nommer la clé fautive — {erreur}",
                animation.nom()
            );
            let message = erreur.to_string();
            for connue in DOUZE {
                assert!(
                    message.contains(connue),
                    "« {} » : le refus de « palette={saisi} » doit citer « {connue} » — \
                     « {message} »",
                    animation.nom()
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — l'échantillonnage est effectif
// ---------------------------------------------------------------------------

#[test]
fn l_appareil_ne_voit_qu_une_teinte_quand_une_couleur_unique_est_donnee() {
    // Test d'appareil, vert avant comme après. Une mesure faite sur un appareil faux ne dit rien et
    // ne le signale pas — c'est la règle de `spec_braise_sans_axe.rs`, qui s'appareille avant de
    // conclure.
    //
    // Ce qu'il établit : la teinte, telle que ce fichier la définit, **ne bouge pas** sous une
    // couleur unique. Une famille applique `couleur × intensité`, donc les 124 LED sont
    // proportionnelles entre elles et ne diffèrent que par l'arrondi vers l'octet. Sans cette
    // mesure, le test suivant pourrait être vert pour la seule raison que l'observable est bruyant.
    //
    // Mesuré sur `main` à la révision `dadf093` : l'écart maximal vaut 0,0103, contre 0,02 tolérés
    // ici et 0,15 exigés du test suivant.
    let geometrie = geometrie();
    let mut images_mesurees = 0usize;

    for animation in familles_avec("couleur") {
        // ⚠️ `braise` n'accepte plus `direction` depuis #119 : la lui imposer ferait rejeter la
        // commande entière. Elle est donc mesurée sous la seule direction par défaut, et elle doit
        // l'être — c'est la famille dont le motif est le plus haché du catalogue.
        let directions: Vec<Direction> = if animation.parametres_acceptes().contains(&"direction") {
            Direction::ALL.to_vec()
        } else {
            vec![Reglages::default().direction]
        };
        for direction in directions {
            let reglages = Reglages {
                couleur: TEMOIN,
                vitesse: 3,
                direction,
                ..Reglages::default()
            };
            for pas in (0..PERIODE).step_by(5) {
                let image = animation.image(&geometrie, &reglages, pas);
                let Some((une, autre, ecart)) = plus_grand_ecart(&image) else {
                    continue;
                };
                images_mesurees += 1;
                assert!(
                    ecart <= TOLERANCE_TEINTE,
                    "appareil : « {} » sous « {} », pas {pas} : les LED n° {une} ({}) et n° {autre} \
                     ({}) diffèrent de {ecart:.4} de teinte alors qu'une couleur unique leur est \
                     donnée — l'observable est bruyant, et le test de l'échantillonnage ne \
                     mesurerait plus rien",
                    animation.nom(),
                    direction.slug(),
                    hexa(couleur_par_rang(&image, une)),
                    hexa(couleur_par_rang(&image, autre))
                );
            }
        }
    }

    assert!(
        images_mesurees >= 100,
        "seulement {images_mesurees} image(s) portaient deux LED allumées : l'appareil n'a rien \
         mesuré"
    );
}

#[test]
fn une_image_avec_la_palette_light_pink_porte_au_moins_deux_teintes() {
    // Test d'intention n° 6 de l'issue — « une image produite avec `palette=light-pink` porte au
    // moins deux teintes distinctes » —, et critère d'acceptation : « produit une image dont les LED
    // ne sont **pas toutes de la même teinte** — la palette est bien échantillonnée, pas réduite à
    // un point ».
    //
    // Le défaut visé est muet : une palette rangée dans `Reglages`, relue, persistée, et dont le
    // rendu n'échantillonnerait qu'une seule position — la couleur du milieu, celle du premier
    // arrêt. Le boîtier serait uni d'une couleur plausible, et rien ne le signalerait.
    //
    // ⚠️ **Deux couleurs ne suffisent pas, il faut deux *teintes*.** Une LED éteinte n'a pas de
    // teinte, et deux LED de la même couleur à deux intensités non plus : c'est exactement ce que
    // produit déjà `couleur × intensité`, et un test qui compterait les couleurs distinctes serait
    // vert sans qu'aucune palette n'existe. Voir [`SEUIL_ALLUMEE`] et [`TOLERANCE_TEINTE`].
    let geometrie = geometrie();
    let light_pink = palette(LIGHT_PINK);

    for animation in familles_avec("palette") {
        let nom = animation.nom();
        let avec_palette = animation
            .reglages(&[paire("palette", LIGHT_PINK)])
            .unwrap_or_else(|erreur| {
                panic!("« {nom} » accepte « palette={LIGHT_PINK} » : {erreur}")
            });
        assert_eq!(avec_palette.palette, Some(light_pink));

        let mut pire = 0.0f32;
        let mut trouvee = false;
        for pas in 0..PERIODE {
            let image = animation.image(&geometrie, &avec_palette, pas);
            if let Some((_, _, ecart)) = plus_grand_ecart(&image) {
                pire = pire.max(ecart);
                if ecart > SEPARATION_TEINTE {
                    trouvee = true;
                    break;
                }
            }
        }
        assert!(
            trouvee,
            "« {nom} » avec « palette={LIGHT_PINK} » : sur un cycle entier, deux LED allumées ne \
             diffèrent jamais de plus de {pire:.4} de teinte, pour {SEPARATION_TEINTE:.2} exigés — \
             la palette est réduite à un point au lieu d'être échantillonnée"
        );

        // Et le rendu change vraiment : une palette rangée puis ignorée est le défaut que le
        // catalogue nomme déjà — « une clé acceptée, validée, rangée dans son champ, puis ignorée
        // au rendu est le pire des trois défauts » (`spec_animations.rs`, n° 4).
        let sans_palette = Reglages::default();
        let differe = (0..PERIODE).any(|pas| {
            animation.image(&geometrie, &avec_palette, pas)
                != animation.image(&geometrie, &sans_palette, pas)
        });
        assert!(
            differe,
            "« {nom} » peint la même image avec et sans « palette={LIGHT_PINK} » : la palette est \
             rangée dans les réglages, puis ignorée au rendu"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — la non-régression, octet pour octet
// ---------------------------------------------------------------------------

/// Trois images relevées sur `main` à la révision `dadf093`, **avant** que #126 ne touche à quoi
/// que ce soit.
///
/// C'est le test le plus important de ce fichier, et le seul dont l'énoncé — « une image produite
/// **sans** `palette` est identique octet pour octet à celle d'avant le changement » — ne se
/// vérifie pas contre le code : il se vérifie contre le **passé**. D'où un figeage : les 124
/// couleurs de trois familles, sous des réglages fixes et à un pas fixe, écrites bout à bout en
/// hexadécimal, 744 caractères par image.
///
/// Ce qu'il protège est nommé par un critère de l'issue : « les profils et `eclairage.conf`
/// existants continuent de s'appliquer sans être réécrits ». Sur SHYNAEL, l'état courant est un
/// `anim braise couleur=… vitesse=…` ; une refonte du calcul de couleur qui décalerait le rendu
/// d'une unité changerait ce que le boîtier affiche sans qu'aucun autre test ne s'en aperçoive —
/// tous les autres portent sur des propriétés, et une propriété survit à un décalage.
///
/// Les trois familles ne sont pas prises au hasard : `vague` est l'onde plane, qui ne suit que la
/// position ; `comete` a une traînée, donc un gradient d'intensité et beaucoup de LED éteintes ;
/// `braise` croise deux ondes et un frémissement par LED, et c'est celle que #119 vient de
/// réécrire. Trois mécaniques de couleur différentes.
const FIGEES: [(&str, Rgb, u8, Direction, u32, &str); 3] = [
    (
        "vague",
        Rgb::new(0xff, 0x20, 0x80),
        3,
        Direction::BasHaut,
        17,
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
        Rgb::new(0x20, 0xff, 0x80),
        5,
        Direction::Horaire,
        40,
        "00000000000000000000000000000002150a01070400000000000000000000000000000000000002150a010704000000\
         01070400000000000000000000000000000000000002150a0000000000000000001ad36a1ce070000000000000000000\
         00000019cc660d673405291506371b1187441dec760000000e7339042211000000000000000000000000084221108040\
         000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
         000000000000000000000000000000000000000000000000000000000000000000000000",
    ),
    (
        "braise",
        Rgb::new(0x39, 0x39, 0xff),
        2,
        Direction::Horaire,
        91,
        "14145d1b1b7d1f1f8b2424a42121941d1d8322229c2424a11f1f8d14145d09092a0e0e4114145a1b1b7b20208f2626aa\
         2a2abc2626aa2a2abc3535ee2d2dcb18186f1b1b7d2929ba2c2cc72424a41c1c802121953838fb2424a31d1d822626aa\
         2d2dca23239d2020903232e02b2bc21414591d1d822d2dcb12125214145a1919702121943838fa3636f23636f22525a6\
         00000301010607071f14145a1212540b0b3306061c0101050b0b3413135518186f1a1a7506061e06061f1010480f0f46\
         1c1c7f2121952020930e0e4004041406061d0e0e4014145d12125403030e17176a2626ae23239f1c1c8014145b121252\
         2828b32f2fd42b2bc42323a01f1f8e2020901e1e8823239f2c2cc62c2cc52828b72828b53030da3535ee2a2abc2525a6\
         1f1f8b2727ae2b2bc32f2fd33131dc2828b32828b42f2fd63636f33333e52a2abf2525a82828b63030d93636f13030d8\
         2828b52727b22e2ece3636f43636f43131dc2727af3030d73838fd3636f32d2dcd2727af",
    ),
];

#[test]
fn une_animation_sans_palette_rend_l_image_figee_sur_main() {
    // Test d'intention n° 7 de l'issue — « une image produite **sans** `palette` est identique
    // octet pour octet à celle d'avant le changement » —, et critère d'acceptation : « une commande
    // sans `palette` rend **exactement** l'image d'avant : les profils et `eclairage.conf` existants
    // continuent de s'appliquer sans être réécrits ».
    //
    // ⚠️ **Les octets attendus ont été relevés sur `main` à la révision `dadf093`**, en exécutant le
    // code de ce jour-là hors du dépôt. Ce ne sont donc ni des valeurs calculées à la main ni des
    // valeurs relues dans l'implémentation : c'est le comportement d'avant, figé.
    //
    // ⚠️ **On ne corrige jamais cette constante pour faire passer le test.** Si elle diffère après
    // implémentation, c'est que #126 a changé le rendu d'un boîtier qui ne demandait pas de palette
    // — exactement ce que l'issue interdit. Le message nomme la première LED qui a bougé.
    //
    // ⚠️ `braise` n'accepte plus `direction` depuis #119 : les réglages sont donc construits **par
    // structure**, avec `..Reglages::default()`, et non par la porte du décodeur. C'est aussi ce qui
    // garantit que le champ `palette` que l'issue ajoute y prend sa valeur d'absence.
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
            "appareil : les réglages par défaut ne portent aucune palette — sans quoi ce test ne \
             mesurerait pas « sans palette »"
        );

        let rendue = empreinte_hexa(&animation(nom).image(&geometrie, &reglages, pas));
        if rendue == attendue {
            continue;
        }
        let rang = (0..LED_DU_BOITIER)
            .find(|rang| rendue[rang * 6..rang * 6 + 6] != attendue[rang * 6..rang * 6 + 6])
            .expect("deux empreintes qui diffèrent diffèrent sur au moins une LED");
        panic!(
            "« {nom} » (couleur {}, vitesse {vitesse}, direction {}, pas {pas}) ne rend plus \
             l'image d'avant #126 : la LED n° {rang} porte {} au lieu de {} — un éclairage sans \
             palette doit être identique octet pour octet à celui de la révision dadf093, sans quoi \
             les profils et « eclairage.conf » existants n'affichent plus la même chose",
            hexa(couleur),
            direction.slug(),
            &rendue[rang * 6..rang * 6 + 6],
            &attendue[rang * 6..rang * 6 + 6],
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — la persistance, et la compatibilité ascendante
// ---------------------------------------------------------------------------

#[test]
fn la_palette_se_reecrit_et_se_relit_sans_perte() {
    // Test d'intention n° 8 de l'issue — « un `eclairage.conf` écrit avec `palette=` se relit à
    // l'identique » —, et critère d'acceptation : « le réglage est **persisté** — `eclairage.conf`,
    // `zones.conf`, profils — et relu tel quel ».
    //
    // C'est ce qui rend vraie la promesse du README : « le boîtier retrouve seul, après un
    // redémarrage, ce qu'il affichait […] une animation avec ses réglages ». Une palette qui se
    // perdrait au passage par le fichier d'état ferait redémarrer le boîtier sur la couleur par
    // défaut, sans le dire.
    //
    // ⚠️ **`couleur` ne doit pas être écrite quand une palette est posée** — voir l'arbitrage n° 2.
    // Les deux clés ensemble sont refusées à la relecture : les écrire toutes les deux produirait un
    // fichier d'état que le démon refuserait au démarrage suivant, ce qui est le défaut de #69 —
    // « un affichage impossible persisté faisait redémarrer le démon dans un état cassé ».
    //
    // ⚠️ Les douze palettes, et pas seulement `light-pink` : un encodage qui rendrait le nom en
    // dur, ou qui perdrait un tiret, ne se verrait que sur les noms composés.
    for animation in familles_avec("palette") {
        let nom = animation.nom();
        let acceptes = animation.parametres_acceptes();

        for palette_nom in DOUZE {
            // La couleur reste au défaut : elle ne sera pas écrite, donc elle ne pourrait pas
            // revenir. Les autres réglages prennent une valeur qui n'est **pas** celle par défaut,
            // sans quoi un aller-retour qui les perdrait ne se verrait pas.
            let mut temoin = Reglages {
                palette: Some(palette(palette_nom)),
                ..Reglages::default()
            };
            if acceptes.contains(&"vitesse") {
                temoin.vitesse = 7;
            }
            if acceptes.contains(&"direction") {
                temoin.direction = Direction::BordsCentre;
            }

            let ecrits = animation.reglages_ecrits(&temoin);
            assert!(
                ecrits
                    .iter()
                    .any(|(cle, valeur)| cle == "palette" && valeur == palette_nom),
                "« {nom} » ne réécrit pas « palette={palette_nom} » : {ecrits:?}"
            );
            assert!(
                !ecrits.iter().any(|(cle, _)| cle == "couleur"),
                "« {nom} » écrit `couleur` **et** `palette` : le fichier d'état ainsi produit serait \
                 refusé à la relecture, et le démon redémarrerait sur un état cassé — {ecrits:?}"
            );

            let relus = animation.reglages(&ecrits).unwrap_or_else(|erreur| {
                panic!("« {nom} » doit relire ce qu'elle écrit ({ecrits:?}) : {erreur}")
            });
            assert_eq!(
                relus, temoin,
                "« {nom} » ne se relit pas sans perte avec « palette={palette_nom} » : écrit \
                 {ecrits:?}"
            );
        }
    }
}

#[test]
fn un_reglage_sans_palette_se_relit_et_n_ecrit_aucune_cle_palette() {
    // Test d'intention n° 9 de l'issue — « un `eclairage.conf` d'avant, sans `palette`, se relit
    // sans erreur ».
    //
    // La compatibilité ascendante a deux faces, et l'issue les demande toutes les deux. Un fichier
    // d'avant se relit — c'est la face évidente. Et un état sans palette **ne fait apparaître aucune
    // clé `palette`** à la réécriture : le démon réécrit `eclairage.conf` à chaque changement, et un
    // `palette=aucune` ou un `palette=` vide y apparaîtrait sans que personne ne l'ait demandé, pour
    // être ensuite refusé par un démon d'avant ou par le décodeur des profils.
    //
    // ⚠️ C'est la moitié qui interdit le raccourci « on écrit toujours toutes les clés acceptées »,
    // et c'est ce raccourci qui vaut aujourd'hui.
    for animation in familles_avec("palette") {
        let nom = animation.nom();
        let acceptes = animation.parametres_acceptes();

        // Un état d'avant : une couleur, une vitesse, une direction. Rien d'autre n'existait.
        let mut avant: Vec<(String, String)> = vec![paire("couleur", &hexa(TEMOIN))];
        if acceptes.contains(&"vitesse") {
            avant.push(paire("vitesse", "7"));
        }
        if acceptes.contains(&"direction") {
            avant.push(paire("direction", Direction::BasHaut.slug()));
        }

        let relus = animation.reglages(&avant).unwrap_or_else(|erreur| {
            panic!("« {nom} » doit relire un jeu de réglages d'avant #126 ({avant:?}) : {erreur}")
        });
        assert_eq!(
            relus.couleur, TEMOIN,
            "« {nom} » : la couleur d'un état d'avant doit être reprise telle quelle"
        );
        assert_eq!(
            relus.palette, None,
            "« {nom} » : un état sans `palette` ne doit pas s'en voir attribuer une"
        );

        let ecrits = animation.reglages_ecrits(&relus);
        assert!(
            !ecrits.iter().any(|(cle, _)| cle == "palette"),
            "« {nom} » fait apparaître une clé `palette` dans l'état réécrit d'un éclairage qui n'en \
             a pas : {ecrits:?}"
        );
        assert!(
            ecrits
                .iter()
                .any(|(cle, valeur)| cle == "couleur" && *valeur == hexa(TEMOIN)),
            "« {nom} » : sans palette, la couleur doit continuer d'être écrite — {ecrits:?}"
        );

        let re_relus = animation.reglages(&ecrits).unwrap_or_else(|erreur| {
            panic!("« {nom} » doit relire ce qu'elle écrit ({ecrits:?}) : {erreur}")
        });
        assert_eq!(
            re_relus, relus,
            "« {nom} » ne fait pas l'aller-retour sur un état sans palette : écrit {ecrits:?}"
        );
    }
}
