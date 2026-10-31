//! Tests d'intention de l'issue #150 : la famille **`course`**, des bandes qui défilent sur une
//! palette fixe (le *Running* de WLED).
//!
//! Écrits **depuis l'issue #150 seule**, avant l'implémentation. Aucun corps de fonction de
//! `crates/reverb-anim/src/` n'a été relu pour les écrire. N'ont servi que les **signatures
//! publiques** relevées au `grep`, les tests d'intention déjà présents dans ce répertoire pour les
//! idiomes (la hauteur de face de `spec_148_schema_face.rs`, la teinte de `spec_126_palettes.rs`,
//! la phase du fondamental de `spec_sens.rs`), et l'exécution de l'API publique telle qu'elle
//! existe sur la branche de #149 (révision `297bde3`) pour mesurer l'appareil et le témoin.
//!
//! Si l'un de ces tests échoue après implémentation, c'est le code qu'on corrige.
//!
//! # Le contrat public que ce fichier fige
//!
//! ```ignore
//! pub const CATALOGUE: &[&str] = &[ /* les treize d'avant, dans leur ordre */, "course"];
//!
//! pub struct Reglages {
//!     pub couleur: Rgb,
//!     pub vitesse: f32,
//!     pub direction: Direction,
//!     pub sonde: Option<String>,
//!     pub palette: Option<Palette>,
//!     /// De 1 à 10, entier. 4 par défaut. Lu et écrit sous la clé `largeur`, par `course` seule.
//!     pub largeur: u8,
//! }
//! ```
//!
//! - La famille s'appelle **`course`**, et elle est la **dernière** du catalogue : l'issue
//!   l'ajoute en dernier « pour ne pas déplacer les boutons de la fenêtre ».
//! - `largeur` est un **`u8`** : l'issue dit « entier de 1 à 10 », et `2.5` est refusé.
//! - La clé du socket, des profils et d'`eclairage.conf` s'appelle `largeur`, comme le champ.
//!
//! # La place `s`, calculée ici et jamais lue dans le code
//!
//! L'issue définit l'intensité d'une LED par `(1 + cos 2π(k·s − t)) / 2`, où `s` est « sa place
//! le long de la direction (entre 0 et 1, celle des autres familles) ». En `bas-haut`, depuis #148,
//! c'est la **hauteur de face** : ce fichier la recalcule comme `spec_148_schema_face.rs` (centre
//! du ventilateur, anneau posé à plat avec le rayon réel de 55 mm, angle relevé de la LED ; les
//! barrettes gardent leur hauteur réelle), puis la ramène à `[0, 1]` entre la plus basse et la
//! plus haute des 124 LED.
//!
//! Ce choix de bornes n'est pas une supposition : à `297bde3`, la phase de `vague bas-haut`
//! (rang 1 du spectre d'une LED sur un cycle) vaut **exactement** cette place, de `0,000` pour la
//! LED la plus basse (−53 mm) à `0,990` juste sous la plus haute (473 mm). `course` « lit la même
//! place `s` que `vague` » (approche technique de l'issue).
//!
//! # Ce que ce fichier vérifie
//!
//! | critère de l'issue | test |
//! |---|---|
//! | n° 1 : réglages acceptés | [`course_est_la_derniere_du_catalogue_et_garde_les_treize_devant_elle`], [`course_accepte_couleur_palette_vitesse_direction_et_largeur`], [`largeur_vaut_quatre_par_defaut`] |
//! | n° 1 : réglages refusés | [`course_refuse_une_largeur_hors_de_un_a_dix_et_couleur_avec_palette`], [`aucune_autre_famille_n_accepte_largeur`] |
//! | n° 2 : palette fixe | [`sous_course_palette_la_teinte_d_une_led_ne_bouge_pas_sur_un_cycle`], témoin [`sous_vague_palette_la_teinte_d_une_led_change_au_fil_du_cycle`] |
//! | n° 3 : nombre de bandes | [`en_bas_haut_le_nombre_de_maxima_suit_la_largeur`], [`en_bas_haut_la_frequence_des_bandes_vaut_dix_sur_largeur`] |
//! | n° 4 : défilement | [`l_intensite_d_une_led_revient_au_bout_d_un_cycle_et_une_bande_y_passe_une_fois`], [`les_bandes_defilent_dans_le_sens_de_la_direction`] |
//! | n° 5 : écriture et relecture (côté catalogue, donc `lighting`) | [`largeur_s_ecrit_hors_defaut_et_se_relit_a_l_identique`] |
//! | (appareil) | [`l_appareil_compte_les_bandes_et_lit_leur_frequence_sur_une_onde_simulee`] |
//!
//! Les profils et `eclairage.conf` sont dans `crates/reverb-daemon/tests/spec_150_course.rs` ; la
//! fenêtre, dans `crates/reverb-gui/tests/spec_150_course.rs`.
//!
//! # Le nombre de bandes : ce qui est fragile, et ce qui ne l'est pas
//!
//! ⚠️ **Compter les maxima le long de `s` n'est pas fiable à `largeur=1`.** Les 124 LED n'occupent
//! que 51 hauteurs de face distinctes, avec des trous jusqu'à **0,064** de l'étendue. Une période
//! de `0,1` (dix bandes) y est sous-échantillonnée : sur l'onde exacte de l'issue, simulée sur ces
//! places, le compte varie de **5 à 10** selon le pas (mesuré par l'appareil). À `largeur` = 2, 5
//! et 10, il reste à une bande près à tous les pas — c'est donc là que le critère n° 3 est testé
//! tel qu'il est écrit.
//!
//! Pour les quatre largeurs, dont 1, un second test mesure la **fréquence spatiale** sans compter :
//! chaque LED donne l'instant où la bande y passe (la phase du fondamental sur un cycle, comme
//! `spec_sens.rs`), cet instant vaut `k·s` modulo 1, et le `k` qui aligne le mieux les 124 phases
//! est lu par régression circulaire. Le sous-échantillonnage ne le gêne pas : une phase est connue
//! à chaque LED, qu'il y ait un maximum à côté d'elle ou non.
//!
//! # Ce que ce fichier ne vérifie pas
//!
//! - La teinte **exacte** sous palette (« `palette` échantillonnée à la place `s` ») : la
//!   correspondance entre `s` et la position d'échantillonnage dans la palette n'est pas écrite par
//!   l'issue au-delà de « indépendante du temps ». Ce fichier exige que la teinte soit fixe dans le
//!   temps **et** qu'elle varie dans l'espace, pas laquelle elle est.
//! - Le rendu « comme `vague` » à `largeur=10` : l'issue le dit en commentaire, pas en critère, et
//!   l'enveloppe d'intensité de `vague` n'est pas figée par la formule de `course`.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi.

use reverb_anim::{Animation, CATALOGUE, Direction, Geometrie, Image, Palette, Reglages};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// Le domaine
// ---------------------------------------------------------------------------

/// Le nom de la famille, tel que l'issue le fixe.
const COURSE: &str = "course";

/// Les treize familles d'avant #150, dans l'ordre du catalogue — celui de la table d'empreintes de
/// `spec_149_vitesse.rs`. Écrites en dur : lues chez `CATALOGUE`, elles ne vérifieraient rien.
const TREIZE: [&str; 13] = [
    "vague",
    "comete",
    "respiration",
    "arc-en-ciel",
    "balayage",
    "braise",
    "rotation",
    "thermique",
    "pouls",
    "scintillement",
    "bougie",
    "nuee",
    "artifice",
];

/// Les cinq réglages que l'issue donne à `course`.
const REGLAGES_DE_COURSE: [&str; 5] = ["couleur", "palette", "vitesse", "direction", "largeur"];

/// La largeur par défaut, selon l'issue.
const LARGEUR_PAR_DEFAUT: u8 = 4;

/// Les largeurs que le critère n° 3 nomme.
const LARGEURS_DE_L_ISSUE: [u8; 4] = [1, 2, 5, 10];

/// Durée d'un cycle, en pas, à la vitesse 1 — figée par `spec_sens.rs` (#49).
const PERIODE: u32 = 120;

/// Le rayon réel d'un anneau, en mm, posé à plat sur le schéma (#148).
const RAYON: f32 = 55.0;

/// Une couleur dont les trois composantes diffèrent deux à deux, comme dans `spec_148_schema_face.rs`.
/// Sa composante maximale vaut 255 : l'intensité d'une LED se lit sur elle.
const TEMOIN: Rgb = Rgb::new(0xff, 0x20, 0x80);

/// Le nom de sonde que `thermique` exige. Sa valeur n'entre pas dans l'image.
const SONDE: &str = "pompe";

/// La palette des deux tests de teinte : celle que Nico emploie sur ses bandes WLED.
///
/// Choisie parce que le témoin y est le plus net : sous `vague palette=light-pink` en `bas-haut`,
/// à `297bde3`, **chacune** des 80 LED de ventilateur voit sa teinte changer d'au moins
/// [`SEPARATION_TEINTE`] sur un cycle, jusqu'à **0,69** pour la pire.
const PALETTE: &str = "light-pink";

// ---------------------------------------------------------------------------
// Les seuils, et d'où ils viennent
// ---------------------------------------------------------------------------

/// En deçà de cette composante maximale, une LED n'a pas de teinte lisible — le plancher de
/// `spec_126_palettes.rs`, pour la même raison : à `max ≥ 96`, une composante normalisée est
/// connue à `0,5 / 96 ≈ 0,005` près.
const SEUIL_ALLUMEE: u8 = 96;

/// Écart de teinte au-delà duquel une LED n'a plus **la même** teinte d'un pas à l'autre.
///
/// La teinte est `c / max`, deux octets arrondis chacun d'au plus 0,5 : l'erreur sur une
/// composante est d'au plus `0,5/max + (c/max)·0,5/max ≤ 1/96 ≈ 0,0104` au plancher. Entre deux
/// pas, deux erreurs de signe contraire font au plus **0,021**. Le seuil est posé à 0,03 : au-dessus
/// de l'arrondi, cinq fois sous [`SEPARATION_TEINTE`].
///
/// ⚠️ Plus large que le `0,02` de `spec_126_palettes.rs` : là-bas, l'arrondi était mesuré sur des
/// couleurs uniques ; ici la teinte vient d'une palette, dont les deux petites composantes ne sont
/// pas des multiples commodes de la grande.
const TOLERANCE_TEINTE: f32 = 0.03;

/// Deux teintes sont **distinctes** au-delà de cet écart — la valeur de `spec_126_palettes.rs`.
const SEPARATION_TEINTE: f32 = 0.15;

/// Une bande est **allumée** sur une LED au-dessus de cette intensité, **éteinte** en dessous de
/// [`BAS`].
///
/// L'hystérésis est ce qui rend le compte robuste : une crête de l'onde de l'issue passe au-dessus
/// de 0,75 sur un tiers de la période, et un creux sous 0,25 sur un autre tiers. Entre les deux,
/// un plateau ou un arrondi d'octet ne peut ni créer ni fusionner une bande.
const HAUT: f64 = 0.75;

/// Voir [`HAUT`].
const BAS: f64 = 0.25;

/// L'écart toléré entre le nombre de bandes mesuré et `10 / largeur` : « à une bande près »,
/// selon l'issue.
const UNE_BANDE: f64 = 1.0;

/// Le pas de balayage de la régression circulaire, en bandes par étendue.
///
/// Un deux-centième : très en dessous de la tolérance d'une bande, et assez fin pour que la crête
/// de la régression (large de `1 / étendue des s ≈ 1`) ne tombe jamais entre deux essais.
const PAS_DE_K: f64 = 0.005;

/// La plus grande fréquence essayée, en bandes par étendue : au-delà des dix bandes de
/// `largeur=1`, avec de la marge pour qu'une implémentation qui en mettrait trop soit vue trop
/// haute et non repliée.
const K_MAXIMAL: f64 = 15.0;

/// L'alignement minimal des phases sur la droite `k·s` retenue, de 0 à 1.
///
/// L'onde exacte de l'issue l'aligne à 1 ; l'arrondi à l'octet et l'échantillonnage du temps sur
/// 120 pas ne le font perdre que de quelques millièmes (mesuré sur l'appareil). Neuf dixièmes :
/// une onde qui ne serait pas une fonction de `k·s − t` n'y arrive pas.
const ALIGNEMENT_MINIMAL: f64 = 0.9;

// ---------------------------------------------------------------------------
// Les LED, leur place, leur intensité
// ---------------------------------------------------------------------------

/// Une LED du boîtier, désignée comme l'`Image` la désigne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Led {
    Ventilateur(Position, usize),
    Barrette(usize, usize),
}

impl Led {
    fn nom(self) -> String {
        match self {
            Led::Ventilateur(position, led) => format!("{} led {led}", position.slug()),
            Led::Barrette(slot, led) => format!("barrette {slot} led {led}"),
        }
    }

    /// Sa couleur dans une image, le ventilateur cherché **par position**.
    fn couleur(self, image: &Image) -> Rgb {
        match self {
            Led::Ventilateur(position, led) => {
                let (_, couleurs) = image
                    .ventilateurs
                    .iter()
                    .find(|(portee, _)| *portee == position)
                    .unwrap_or_else(|| panic!("l'image porte le ventilateur {}", position.slug()));
                couleurs[led]
            }
            Led::Barrette(slot, led) => image.barrettes[slot][led],
        }
    }
}

/// Les 124 LED du boîtier, dans un ordre stable.
fn toutes_les_led() -> Vec<Led> {
    let mut leds = Vec::new();
    for position in Position::ALL {
        for indice in 0..LEDS_PER_FAN as usize {
            leds.push(Led::Ventilateur(position, indice));
        }
    }
    for slot in 0..SLOT_COUNT {
        for indice in 0..LEDS_PER_STICK {
            leds.push(Led::Barrette(slot, indice));
        }
    }
    leds
}

/// La hauteur de face d'une LED, par la formule de #148.
fn hauteur_de_face(geometrie: &Geometrie, led: Led) -> f32 {
    match led {
        Led::Ventilateur(position, indice) => {
            let centre = geometrie.centre_ventilateur(position);
            let angle = f32::from(geometrie.orientation(position).angle_led(indice)).to_radians();
            centre.y + RAYON * angle.cos()
        }
        Led::Barrette(slot, indice) => {
            geometrie
                .led_barrette(slot, indice)
                .unwrap_or_else(|| panic!("{} n'a pas de place dans le boîtier", led.nom()))
                .y
        }
    }
}

/// La place `s` de chaque LED en `bas-haut` : sa hauteur de face, ramenée à `[0, 1]` entre la plus
/// basse et la plus haute LED. Dans l'ordre de [`toutes_les_led`].
fn places(geometrie: &Geometrie) -> Vec<f64> {
    let hauteurs: Vec<f32> = toutes_les_led()
        .into_iter()
        .map(|led| hauteur_de_face(geometrie, led))
        .collect();
    let minimum = hauteurs.iter().copied().fold(f32::INFINITY, f32::min);
    let maximum = hauteurs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    assert!(
        maximum > minimum,
        "appareil : les hauteurs de face ont une étendue"
    );
    hauteurs
        .into_iter()
        .map(|hauteur| f64::from((hauteur - minimum) / (maximum - minimum)))
        .collect()
}

/// L'intensité d'une LED peinte de [`TEMOIN`] : sa composante maximale rapportée à 255.
fn intensite(couleur: Rgb) -> f64 {
    f64::from(couleur.r.max(couleur.g).max(couleur.b)) / 255.0
}

/// La teinte d'une LED : ses composantes ramenées à la plus grande, ou `None` sous
/// [`SEUIL_ALLUMEE`] — celle de `spec_126_palettes.rs`.
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

/// Le plus grand écart entre deux teintes d'une liste.
fn etalement(teintes: &[[f32; 3]]) -> f32 {
    let mut pire = 0.0f32;
    for (rang, une) in teintes.iter().enumerate() {
        for autre in &teintes[rang + 1..] {
            pire = pire.max(ecart_de_teinte(*une, *autre));
        }
    }
    pire
}

// ---------------------------------------------------------------------------
// La mesure des bandes
// ---------------------------------------------------------------------------

/// Le nombre de bandes d'un profil d'intensité **ouvert** (le long de `s`), par hystérésis.
///
/// Une bande commence quand l'intensité franchit [`HAUT`] après être passée sous [`BAS`] — ou au
/// premier point, s'il est déjà haut. Les points sont triés par place.
fn bandes_le_long(points: &[(f64, f64)]) -> usize {
    let mut bandes = 0;
    let mut armee = true;
    for &(_, valeur) in points {
        if armee && valeur >= HAUT {
            bandes += 1;
            armee = false;
        } else if valeur <= BAS {
            armee = true;
        }
    }
    bandes
}

/// Le nombre de bandes qui passent sur une LED pendant un cycle **bouclé** sur lui-même.
///
/// La série est relue à partir de son minimum : c'est un creux, et une bande à cheval sur la fin
/// et le début du cycle n'est comptée qu'une fois.
fn bandes_par_cycle(serie: &[f64]) -> usize {
    let depart = serie.iter().enumerate().fold(
        0,
        |pire, (rang, valeur)| {
            if *valeur < serie[pire] { rang } else { pire }
        },
    );
    let tour: Vec<(f64, f64)> = (0..serie.len())
        .map(|decalage| (0.0, serie[(depart + decalage) % serie.len()]))
        .collect();
    bandes_le_long(&tour)
}

/// Le profil d'intensité le long de `s` : une valeur par hauteur distincte, triée.
///
/// Les LED de même place reçoivent la même intensité d'une onde qui ne dépend que de `s` ; leur
/// moyenne n'efface donc rien, et elle empêche deux LED d'une même hauteur de compter pour deux
/// points d'un profil qui n'en a qu'un.
fn profil(places: &[f64], intensites: &[f64]) -> Vec<(f64, f64)> {
    let mut points: Vec<(f64, f64)> = places
        .iter()
        .copied()
        .zip(intensites.iter().copied())
        .collect();
    points.sort_by(|une, autre| une.0.total_cmp(&autre.0));
    let mut regroupes: Vec<(f64, f64, u32)> = Vec::new();
    for (place, valeur) in points {
        match regroupes.last_mut() {
            Some((derniere, somme, combien)) if (place - *derniere).abs() < 1e-6 => {
                *somme += valeur;
                *combien += 1;
            }
            _ => regroupes.push((place, valeur, 1)),
        }
    }
    regroupes
        .into_iter()
        .map(|(place, somme, combien)| (place, somme / f64::from(combien)))
        .collect()
}

/// L'instant, en fraction de cycle dans `[0, 1[`, où la bande passe sur une LED : la phase du
/// fondamental de sa série d'intensités, comme `spec_sens.rs`.
///
/// Pour `(1 + cos 2π(k·s − t)) / 2`, c'est exactement `k·s` modulo 1.
fn passage(serie: &[f64]) -> f64 {
    let combien = serie.len() as f64;
    let (mut cosinus, mut sinus) = (0.0f64, 0.0f64);
    for (pas, valeur) in serie.iter().enumerate() {
        let angle = std::f64::consts::TAU * pas as f64 / combien;
        cosinus += valeur * angle.cos();
        sinus += valeur * angle.sin();
    }
    (sinus.atan2(cosinus) / std::f64::consts::TAU).rem_euclid(1.0)
}

/// Le `k` signé qui aligne le mieux les instants de passage sur `k·s`, et cet alignement (de 0 à
/// 1) : la régression circulaire de `|moyenne de e^{2πi(φ − k·s)}|`, balayée de `−K_MAXIMAL` à
/// `K_MAXIMAL`.
///
/// `k > 0` : la bande atteint les grands `s` **après** les petits — elle monte, en `bas-haut`.
fn frequence(places: &[f64], passages: &[f64]) -> (f64, f64) {
    let essais = (2.0 * K_MAXIMAL / PAS_DE_K).round() as i64;
    let mut meilleur = (0.0f64, -1.0f64);
    for rang in 0..=essais {
        let k = -K_MAXIMAL + rang as f64 * PAS_DE_K;
        let (mut reel, mut imaginaire) = (0.0f64, 0.0f64);
        for (place, phase) in places.iter().zip(passages) {
            let angle = std::f64::consts::TAU * (phase - k * place);
            reel += angle.cos();
            imaginaire += angle.sin();
        }
        let alignement = reel.hypot(imaginaire) / places.len() as f64;
        if alignement > meilleur.1 {
            meilleur = (k, alignement);
        }
    }
    meilleur
}

/// L'onde exacte de l'issue, `(1 + cos 2π(k·s − t)) / 2`.
fn onde(k: f64, place: f64, temps: f64) -> f64 {
    (1.0 + (std::f64::consts::TAU * (k * place - temps)).cos()) / 2.0
}

/// `10 / largeur`, le nombre de bandes de l'issue.
fn k_attendu(largeur: u8) -> f64 {
    10.0 / f64::from(largeur)
}

// ---------------------------------------------------------------------------
// Aides communes
// ---------------------------------------------------------------------------

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
}

fn paire(cle: &str, valeur: &str) -> (String, String) {
    (cle.to_owned(), valeur.to_owned())
}

fn palette(nom: &str) -> Palette {
    Palette::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est une palette : {erreur:?}"))
}

/// Les réglages des mesures : vitesse 1, pour qu'un cycle tienne en [`PERIODE`] pas.
fn reglages(largeur: u8, direction: Direction, palette: Option<Palette>) -> Reglages {
    Reglages {
        couleur: if palette.is_some() {
            Reglages::default().couleur
        } else {
            TEMOIN
        },
        vitesse: 1.0,
        direction,
        palette,
        largeur,
        ..Reglages::default()
    }
}

/// Un cycle entier d'une famille, pas par pas.
fn cycle(nom: &str, reglages: &Reglages) -> Vec<Image> {
    let famille = animation(nom);
    let geometrie = Geometrie::mesuree();
    (0..PERIODE)
        .map(|pas| famille.image(&geometrie, reglages, pas))
        .collect()
}

/// La série d'intensités de chaque LED sur un cycle, dans l'ordre de [`toutes_les_led`].
fn series(images: &[Image]) -> Vec<Vec<f64>> {
    toutes_les_led()
        .into_iter()
        .map(|led| {
            images
                .iter()
                .map(|image| intensite(led.couleur(image)))
                .collect()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Appareillage — vert par nature
// ---------------------------------------------------------------------------

#[test]
fn l_appareil_compte_les_bandes_et_lit_leur_frequence_sur_une_onde_simulee() {
    // Avant de juger `course`, on juge la règle : l'onde **exacte** de l'issue, calculée ici sur
    // les 124 places réelles, doit donner le bon compte de bandes aux largeurs où on le demande, et
    // la bonne fréquence à toutes. Sinon un échec plus bas dirait quelque chose de l'appareil, pas
    // de l'implémentation.
    let places = places(&Geometrie::mesuree());
    assert_eq!(places.len(), 124, "le boîtier porte 124 LED");

    for largeur in LARGEURS_DE_L_ISSUE {
        let k = k_attendu(largeur);

        // Le compte, à chacun des 120 pas — sauf à `largeur=1`, voir l'en-tête.
        if largeur != 1 {
            for pas in 0..PERIODE {
                let temps = f64::from(pas) / f64::from(PERIODE);
                let intensites: Vec<f64> =
                    places.iter().map(|place| onde(k, *place, temps)).collect();
                let bandes = bandes_le_long(&profil(&places, &intensites));
                assert!(
                    (bandes as f64 - k.round()).abs() <= UNE_BANDE,
                    "appareil : l'onde exacte à largeur={largeur}, pas {pas}, compte {bandes} \
                     bandes au lieu de {} à une près",
                    k.round()
                );
            }
        }

        // La fréquence, lue sur les instants de passage de l'onde exacte échantillonnée comme le
        // sera l'animation : 120 pas par cycle.
        let passages: Vec<f64> = places
            .iter()
            .map(|place| {
                let serie: Vec<f64> = (0..PERIODE)
                    .map(|pas| onde(k, *place, f64::from(pas) / f64::from(PERIODE)))
                    .collect();
                passage(&serie)
            })
            .collect();
        let (lu, alignement) = frequence(&places, &passages);
        assert!(
            (lu - k).abs() < 0.05 && alignement > 0.99,
            "appareil : la régression lit k = {lu:.3} (alignement {alignement:.3}) sur l'onde \
             exacte de k = {k}"
        );
    }
}

// ---------------------------------------------------------------------------
// Critère n° 1 — le catalogue et les réglages acceptés
// ---------------------------------------------------------------------------

#[test]
fn course_est_la_derniere_du_catalogue_et_garde_les_treize_devant_elle() {
    // Comportement attendu : « une 14ᵉ famille, `course` ». Approche technique : « ajoutée **en
    // dernier** au catalogue pour ne pas déplacer les boutons de la fenêtre ».
    assert!(
        CATALOGUE.len() > TREIZE.len(),
        "le catalogue doit gagner une famille : {CATALOGUE:?}"
    );
    assert_eq!(
        &CATALOGUE[..TREIZE.len()],
        &TREIZE[..],
        "les treize familles d'avant restent en tête, dans leur ordre"
    );
    assert_eq!(
        CATALOGUE.last(),
        Some(&COURSE),
        "« {COURSE} » est ajoutée en dernier : {CATALOGUE:?}"
    );
    assert_eq!(
        animation(COURSE).nom(),
        COURSE,
        "« {COURSE} » se nomme elle-même"
    );
}

#[test]
fn course_accepte_couleur_palette_vitesse_direction_et_largeur() {
    // Critère n° 1 : « `animate course` est accepté, ainsi que `couleur`, `palette`, `vitesse`,
    // `direction` et `largeur` (1 à 10) ». Comportement attendu : « `direction` (toutes, locales
    // comprises) ».
    let course = animation(COURSE);
    let mut declares: Vec<&str> = course.parametres_acceptes().to_vec();
    declares.sort_unstable();
    let mut attendus = REGLAGES_DE_COURSE.to_vec();
    attendus.sort_unstable();
    assert_eq!(
        declares, attendus,
        "« {COURSE} » déclare exactement les réglages de l'issue"
    );
    assert!(
        course.parametres_obligatoires().is_empty(),
        "« animate course » sans réglage est accepté : rien n'est obligatoire — {:?}",
        course.parametres_obligatoires()
    );
    course
        .reglages(&[])
        .unwrap_or_else(|erreur| panic!("« animate course » seul est accepté : {erreur}"));

    let lus = course
        .reglages(&[paire("couleur", "ff2080")])
        .unwrap_or_else(|erreur| panic!("« couleur=ff2080 » est accepté : {erreur}"));
    assert_eq!(lus.couleur, TEMOIN, "la couleur atterrit dans `couleur`");

    let lus = course
        .reglages(&[paire("palette", PALETTE)])
        .unwrap_or_else(|erreur| panic!("« palette={PALETTE} » est acceptée : {erreur}"));
    assert_eq!(
        lus.palette,
        Some(palette(PALETTE)),
        "la palette atterrit dans `palette`"
    );

    let lus = course
        .reglages(&[paire("vitesse", "0.5")])
        .unwrap_or_else(|erreur| panic!("« vitesse=0.5 » est acceptée (#149) : {erreur}"));
    assert_eq!(lus.vitesse, 0.5, "la vitesse atterrit dans `vitesse`");

    for direction in Direction::ALL {
        let lus = course
            .reglages(&[paire("direction", direction.slug())])
            .unwrap_or_else(|erreur| {
                panic!("« direction={} » est acceptée : {erreur}", direction.slug())
            });
        assert_eq!(
            lus.direction,
            direction,
            "« {} » désigne sa direction",
            direction.slug()
        );
    }

    for largeur in 1..=10u8 {
        let lus = course
            .reglages(&[paire("largeur", &largeur.to_string())])
            .unwrap_or_else(|erreur| panic!("« largeur={largeur} » est acceptée : {erreur}"));
        assert_eq!(
            lus.largeur, largeur,
            "« largeur={largeur} » atterrit dans `largeur`"
        );
    }
}

#[test]
fn largeur_vaut_quatre_par_defaut() {
    // Comportement attendu : « entier de 1 à 10, **4 par défaut** ». Critère n° 5 : « absent, il
    // vaut 4 ».
    assert_eq!(
        Reglages::default().largeur,
        LARGEUR_PAR_DEFAUT,
        "le défaut de `Reglages`"
    );
    let lus = animation(COURSE)
        .reglages(&[])
        .expect("« animate course » seul est accepté");
    assert_eq!(
        lus.largeur, LARGEUR_PAR_DEFAUT,
        "« animate course » sans `largeur` tourne à la largeur 4"
    );
}

// ---------------------------------------------------------------------------
// Critère n° 1 — les refus
// ---------------------------------------------------------------------------

/// Les largeurs refusées : les trois de l'issue, puis deux fautes qu'on commet vraiment.
const LARGEURS_REFUSEES: [(&str, &str); 5] = [
    ("0", "sous la borne basse"),
    ("11", "au-dessus de la borne haute"),
    ("2.5", "pas un entier"),
    ("-1", "un négatif"),
    ("large", "pas un nombre"),
];

#[test]
fn course_refuse_une_largeur_hors_de_un_a_dix_et_couleur_avec_palette() {
    // Critère n° 1 : « `largeur=0`, `largeur=11`, `largeur=2.5` et `couleur` + `palette` ensemble
    // sont refusés ».
    let course = animation(COURSE);
    for (valeur, pourquoi) in LARGEURS_REFUSEES {
        let Err(erreur) = course.reglages(&[paire("largeur", valeur)]) else {
            panic!("« {COURSE} » a accepté « largeur={valeur} » ({pourquoi})");
        };
        assert_eq!(
            erreur.cle, "largeur",
            "« largeur={valeur} » ({pourquoi}) est fautive au titre de `largeur` — {erreur}"
        );
    }

    assert!(
        course
            .reglages(&[paire("couleur", "ff2080"), paire("palette", PALETTE)])
            .is_err(),
        "« {COURSE} » refuse `couleur` et `palette` ensemble, comme les autres familles (#126)"
    );
}

#[test]
fn aucune_autre_famille_n_accepte_largeur() {
    // Critère n° 1 : « `largeur` est refusé par toute autre famille ». Une clé acceptée et ignorée
    // serait un curseur qui ne fait rien.
    for nom in CATALOGUE.iter().copied().filter(|nom| *nom != COURSE) {
        let famille = animation(nom);
        assert!(
            !famille.parametres_acceptes().contains(&"largeur"),
            "« {nom} » ne déclare pas `largeur` — {:?}",
            famille.parametres_acceptes()
        );
        let mut paires = vec![paire("largeur", "4")];
        if nom == "thermique" {
            paires.push(paire("sonde", SONDE));
        }
        let Err(erreur) = famille.reglages(&paires) else {
            panic!("« {nom} » a accepté « largeur=4 » : seule « {COURSE} » la connaît");
        };
        assert_eq!(
            erreur.cle, "largeur",
            "« {nom} » nomme la clé fautive — {erreur}"
        );
    }
}

// ---------------------------------------------------------------------------
// Critère n° 5 — l'écriture, et l'aller-retour
// ---------------------------------------------------------------------------

#[test]
fn largeur_s_ecrit_hors_defaut_et_se_relit_a_l_identique() {
    // Critère n° 5 : « `largeur` est écrit par `lighting` et dans les profils, et relu à
    // l'identique. Absent, il vaut 4 et ne s'écrit pas ». `reglages_ecrits` est l'émetteur de
    // `lighting`, des profils et d'`eclairage.conf` (`spec_149_vitesse.rs`) : l'écriture se juge
    // ici une fois pour les trois.
    let course = animation(COURSE);
    for palette in [None, Some(palette(PALETTE))] {
        for largeur in 1..=10u8 {
            let posee = Reglages {
                couleur: if palette.is_some() {
                    Reglages::default().couleur
                } else {
                    TEMOIN
                },
                vitesse: 2.5,
                direction: Direction::BordsCentre,
                palette,
                largeur,
                ..Reglages::default()
            };
            let ecrits = course.reglages_ecrits(&posee);
            let valeurs: Vec<&str> = ecrits
                .iter()
                .filter(|(cle, _)| cle == "largeur")
                .map(|(_, valeur)| valeur.as_str())
                .collect();
            let attendues: Vec<String> = if largeur == LARGEUR_PAR_DEFAUT {
                Vec::new()
            } else {
                vec![largeur.to_string()]
            };
            assert_eq!(
                valeurs, attendues,
                "la largeur {largeur} s'écrit {attendues:?} — {ecrits:?}"
            );

            let relus = course.reglages(&ecrits).unwrap_or_else(|erreur| {
                panic!("« {COURSE} » relit ce qu'elle écrit, {ecrits:?} : {erreur}")
            });
            assert_eq!(
                relus, posee,
                "la largeur {largeur} traverse l'aller-retour — {ecrits:?}"
            );
        }
    }

    // Ailleurs, jamais : une famille qui écrirait `largeur` ferait refuser sa propre relecture.
    for nom in CATALOGUE.iter().copied().filter(|nom| *nom != COURSE) {
        let porte = Reglages {
            sonde: (nom == "thermique").then(|| SONDE.to_owned()),
            largeur: 7,
            ..Reglages::default()
        };
        let ecrits = animation(nom).reglages_ecrits(&porte);
        assert!(
            ecrits.iter().all(|(cle, _)| cle != "largeur"),
            "« {nom} » n'écrit pas `largeur`, même portée par ses réglages — {ecrits:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Critère n° 2 — la palette est fixe dans l'espace
// ---------------------------------------------------------------------------

#[test]
fn sous_course_palette_la_teinte_d_une_led_ne_bouge_pas_sur_un_cycle() {
    // Critère n° 2 : « sous `course palette=<p>`, pour chaque LED allumée, la teinte (couleur
    // rapportée à sa composante maximale) est la même à tous les pas d'un cycle ».
    //
    // Les huit directions, et trois largeurs : la palette ne dépend que de la place, quelle que
    // soit la façon dont les bandes la traversent.
    //
    // Et la seconde moitié, sans laquelle la première serait triviale : la palette **est**
    // appliquée. Une famille qui ignorerait la palette et peindrait une couleur unique aurait une
    // teinte fixe à chaque LED — mais la même partout.
    let leds = toutes_les_led();
    for direction in Direction::ALL {
        for largeur in [1u8, LARGEUR_PAR_DEFAUT, 10] {
            let images = cycle(
                COURSE,
                &reglages(largeur, direction, Some(palette(PALETTE))),
            );
            let mut fautes = Vec::new();
            let mut allumees = 0;
            for led in &leds {
                let teintes: Vec<[f32; 3]> = images
                    .iter()
                    .filter_map(|image| teinte(led.couleur(image)))
                    .collect();
                if teintes.is_empty() {
                    continue;
                }
                allumees += 1;
                let ecart = etalement(&teintes);
                if ecart > TOLERANCE_TEINTE {
                    fautes.push(format!("{} ({ecart:.3})", led.nom()));
                }
            }
            assert!(
                allumees > 0,
                "appareil : sous « {COURSE} palette={PALETTE} » en {} à largeur={largeur}, aucune \
                 LED ne passe le plancher de luminosité sur tout un cycle",
                direction.slug()
            );
            assert!(
                fautes.is_empty(),
                "« {COURSE} palette={PALETTE} » en {}, largeur={largeur} : {} LED sur {allumees} \
                 changent de teinte au fil du cycle, au-delà de {TOLERANCE_TEINTE} — la palette \
                 doit rester fixe dans l'espace. Les dix premières : {}",
                direction.slug(),
                fautes.len(),
                fautes
                    .iter()
                    .take(10)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    // La palette se voit dans l'espace : sur un cycle, les LED allumées ne portent pas toutes la
    // même teinte.
    let images = cycle(
        COURSE,
        &reglages(
            LARGEUR_PAR_DEFAUT,
            Direction::BasHaut,
            Some(palette(PALETTE)),
        ),
    );
    let toutes: Vec<[f32; 3]> = images
        .iter()
        .flat_map(|image| leds.iter().filter_map(|led| teinte(led.couleur(image))))
        .collect();
    let ecart = etalement(&toutes);
    assert!(
        ecart >= SEPARATION_TEINTE,
        "« {COURSE} palette={PALETTE} » en bas-haut : les LED allumées ne s'écartent que de \
         {ecart:.3} en teinte — la palette n'est pas échantillonnée le long de `s`"
    );
}

#[test]
fn sous_vague_palette_la_teinte_d_une_led_change_au_fil_du_cycle() {
    // Le témoin du critère n° 2 : « sous `vague palette=<p>`, elle varie ». Sans lui, la mesure du
    // test précédent pourrait être aveugle — un plancher trop haut, une tolérance trop large — et
    // passer sur n'importe quoi.
    //
    // Vert avant l'implémentation : `vague` existe déjà, et c'est ce qui la distingue de `course`.
    // Exigé sur au moins la moitié des LED : à `297bde3`, toutes celles des ventilateurs y sont.
    let leds = toutes_les_led();
    let images = cycle(
        "vague",
        &Reglages {
            vitesse: 1.0,
            direction: Direction::BasHaut,
            palette: Some(palette(PALETTE)),
            ..Reglages::default()
        },
    );
    let changeantes = leds
        .iter()
        .filter(|led| {
            let teintes: Vec<[f32; 3]> = images
                .iter()
                .filter_map(|image| teinte(led.couleur(image)))
                .collect();
            etalement(&teintes) >= SEPARATION_TEINTE
        })
        .count();
    assert!(
        changeantes * 2 >= leds.len(),
        "témoin : sous « vague palette={PALETTE} » en bas-haut, seules {changeantes} LED sur {} \
         changent de teinte d'au moins {SEPARATION_TEINTE} sur un cycle — la mesure ne distingue \
         plus une palette qui voyage d'une palette fixe",
        leds.len()
    );
}

// ---------------------------------------------------------------------------
// Critère n° 3 — le nombre de bandes
// ---------------------------------------------------------------------------

#[test]
fn en_bas_haut_le_nombre_de_maxima_suit_la_largeur() {
    // Critère n° 3, tel qu'il est écrit : « en `bas-haut`, à un pas donné, le nombre de maxima
    // d'intensité le long de la place `s` vaut `10 / largeur` arrondi, à une bande près ».
    //
    // À **chacun** des 120 pas d'un cycle, pour `largeur` = 2, 5 et 10. ⚠️ Pas 1 : à dix bandes,
    // les 51 hauteurs distinctes du boîtier ne suffisent pas à voir chaque crête (voir l'en-tête) ;
    // le test suivant couvre `largeur=1` par la fréquence.
    let places = places(&Geometrie::mesuree());
    for largeur in LARGEURS_DE_L_ISSUE
        .into_iter()
        .filter(|largeur| *largeur != 1)
    {
        let attendu = k_attendu(largeur).round();
        let images = cycle(COURSE, &reglages(largeur, Direction::BasHaut, None));
        let mut fautes = Vec::new();
        for (pas, image) in images.iter().enumerate() {
            let intensites: Vec<f64> = toutes_les_led()
                .into_iter()
                .map(|led| intensite(led.couleur(image)))
                .collect();
            let bandes = bandes_le_long(&profil(&places, &intensites));
            if (bandes as f64 - attendu).abs() > UNE_BANDE {
                fautes.push(format!("pas {pas} : {bandes}"));
            }
        }
        assert!(
            fautes.is_empty(),
            "« {COURSE} largeur={largeur} » en bas-haut doit montrer {attendu} bande(s) à une près \
             le long de `s` ; {} pas sur {PERIODE} s'en écartent — {}",
            fautes.len(),
            fautes
                .iter()
                .take(10)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

#[test]
fn en_bas_haut_la_frequence_des_bandes_vaut_dix_sur_largeur() {
    // Critère n° 3, par la mesure robuste : « `k = 10 / largeur` » — la période spatiale vaut
    // `largeur / 10` de l'étendue. Les quatre largeurs de l'issue, et le défaut (2,5 bandes).
    //
    // Chaque LED dit **quand** la bande passe sur elle ; si l'intensité est bien
    // `(1 + cos 2π(k·s − t)) / 2`, cet instant vaut `k·s` modulo 1, et le `k` qui aligne les 124
    // instants est le nombre de bandes sur l'étendue.
    let places = places(&Geometrie::mesuree());
    for largeur in [1u8, 2, LARGEUR_PAR_DEFAUT, 5, 10] {
        let attendu = k_attendu(largeur);
        let passages: Vec<f64> =
            series(&cycle(COURSE, &reglages(largeur, Direction::BasHaut, None)))
                .iter()
                .map(|serie| passage(serie))
                .collect();
        let (lu, alignement) = frequence(&places, &passages);
        assert!(
            alignement >= ALIGNEMENT_MINIMAL,
            "« {COURSE} largeur={largeur} » en bas-haut : les instants de passage ne s'alignent sur \
             aucune droite `k·s` (au mieux {alignement:.3} à k = {lu:.2}) — l'intensité n'est pas \
             une onde qui défile le long de `s`"
        );
        assert!(
            (lu.abs() - attendu).abs() <= UNE_BANDE,
            "« {COURSE} largeur={largeur} » en bas-haut : {:.2} bandes sur l'étendue, au lieu de \
             10 / {largeur} = {attendu} à une près",
            lu.abs()
        );
    }
}

// ---------------------------------------------------------------------------
// Critère n° 4 — le défilement
// ---------------------------------------------------------------------------

#[test]
fn l_intensite_d_une_led_revient_au_bout_d_un_cycle_et_une_bande_y_passe_une_fois() {
    // Critère n° 4, première moitié : « l'intensité d'une LED est périodique de période un
    // cycle ». Comportement attendu : « une LED voit passer une bande par cycle, quel que soit
    // `largeur` ».
    //
    // Une globale dans chaque sens et une locale : la périodicité ne dépend pas de la direction.
    let geometrie = Geometrie::mesuree();
    let course = animation(COURSE);
    for direction in [
        Direction::BasHaut,
        Direction::HautBas,
        Direction::BordsCentre,
    ] {
        for largeur in [1u8, 2, LARGEUR_PAR_DEFAUT, 5, 10] {
            let regles = reglages(largeur, direction, None);
            for pas in [0u32, 1, 37, 59, 119, 1_000] {
                assert_eq!(
                    course.image(&geometrie, &regles, pas),
                    course.image(&geometrie, &regles, pas + PERIODE),
                    "« {COURSE} largeur={largeur} » en {} : l'image du pas {pas} revient {PERIODE} \
                     pas plus tard",
                    direction.slug()
                );
            }

            let images = cycle(COURSE, &regles);
            assert!(
                images.windows(2).any(|deux| deux[0] != deux[1]),
                "« {COURSE} largeur={largeur} » en {} est figée : une image périodique et immobile \
                 satisferait la moitié de ce critère",
                direction.slug()
            );
            for (led, serie) in toutes_les_led().into_iter().zip(series(&images)) {
                let bandes = bandes_par_cycle(&serie);
                assert_eq!(
                    bandes,
                    1,
                    "« {COURSE} largeur={largeur} » en {} : {} voit passer {bandes} bande(s) par \
                     cycle, et non une",
                    direction.slug(),
                    led.nom()
                );
            }
        }
    }
}

#[test]
fn les_bandes_defilent_dans_le_sens_de_la_direction() {
    // Critère n° 4, seconde moitié : « un pas plus tard le motif s'est décalé dans le sens de la
    // direction ».
    //
    // Mesuré par le signe de `k` : `k > 0` veut dire que la bande atteint les grands `s` (le
    // haut) **après** les petits, donc qu'elle monte d'un pas sur l'autre. En `bas-haut` elle
    // monte, en `haut-bas` elle descend — `s` restant ici la hauteur, mesurée depuis le bas. Les
    // deux sens, à chaque largeur : une implémentation qui inverserait le temps à une seule largeur
    // ne passerait pas.
    let places = places(&Geometrie::mesuree());
    for (direction, monte) in [(Direction::BasHaut, true), (Direction::HautBas, false)] {
        for largeur in [1u8, 2, LARGEUR_PAR_DEFAUT, 5, 10] {
            let passages: Vec<f64> = series(&cycle(COURSE, &reglages(largeur, direction, None)))
                .iter()
                .map(|serie| passage(serie))
                .collect();
            let (lu, alignement) = frequence(&places, &passages);
            assert!(
                alignement >= ALIGNEMENT_MINIMAL,
                "« {COURSE} largeur={largeur} » en {} : aucun défilement lisible (alignement \
                 {alignement:.3})",
                direction.slug()
            );
            assert_eq!(
                lu > 0.0,
                monte,
                "« {COURSE} largeur={largeur} » en {} : les bandes doivent {} — mesuré k = {lu:.2}",
                direction.slug(),
                if monte { "monter" } else { "descendre" }
            );
        }
    }
}
