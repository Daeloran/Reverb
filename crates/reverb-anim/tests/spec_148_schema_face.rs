//! Tests d'intention de l'issue #148 : sous une direction globale, les LED de ventilateur suivent
//! le **schéma de la vue de face**, LED par LED, et non leur position 3D.
//!
//! Écrits **depuis l'issue #148 seule**, avant l'implémentation. Aucun corps de fonction de
//! `crates/reverb-anim/src/` n'a été relu pour les écrire — ni `projection`, ni `angle_du_tour`,
//! ni `flux`, ni les bornes. N'ont servi que les **signatures publiques** relevées au `grep`, les
//! tests d'intention déjà présents dans ce répertoire pour les idiomes d'appel, et l'exécution de
//! l'API publique telle qu'elle existe à `main` (révision `19c5254`) pour figer les images qui ne
//! doivent pas bouger.
//!
//! Si l'un de ces tests échoue après implémentation, c'est le code qu'on corrige.
//!
//! # La position de face, calculée ici et jamais lue dans le code
//!
//! L'issue la définit, et ce fichier la recalcule depuis l'API publique seule : le centre du
//! ventilateur ([`Geometrie::centre_ventilateur`]) garde sa hauteur `y` et sa profondeur `z`
//! réelles, l'anneau est posé à plat dans le plan de la vue avec le rayon réel de 55 mm, et l'angle
//! relevé `a` de la LED (`orientation(position).angle_led(led)`, en degrés) part de midi et tourne
//! vers l'avant, c'est-à-dire vers la droite de l'écran :
//!
//! - hauteur de face = `centre.y + 55 · cos a`
//! - profondeur de face = `centre.z − 55 · sin a`
//!
//! Les barrettes gardent leur position réelle ([`Geometrie::led_barrette`]) : c'est déjà leur place
//! sur la vue de face.
//!
//! ⚠️ **« `avant-arriere` part du grand `z` »**, comme `spec_sens.rs` (#49) l'a figé : le schéma
//! met l'arrière à gauche et l'avant à droite, et `z` croît vers l'arrière
//! (`spec_disposition.rs`, #27). La profondeur de face suit donc le même sens que `z`.
//!
//! # Ce que ce fichier vérifie, et dans quel état avant l'implémentation
//!
//! | critère de l'issue | test | avant |
//! |---|---|---|
//! | (appareil) les six couchés sont plats en 3D et pas sur le schéma | [`l_appareil_voit_six_ventilateurs_plats_en_3d_et_debout_sur_le_schema`] | vert |
//! | (appareil) la mesure d'arrivée lit l'ordre d'une barrette | [`l_appareil_lit_l_ordre_d_arrivee_du_balayage_sur_une_barrette`] | vert |
//! | n° 1, première moitié : même hauteur de face, même couleur | [`vague_bas_haut_peint_pareil_deux_led_a_la_meme_hauteur_de_face`] | vert, voir plus bas |
//! | n° 1, seconde moitié : un couché n'est pas uniforme | [`vague_bas_haut_ne_peint_jamais_un_ventilateur_couche_d_une_seule_couleur`] | **rouge** |
//! | n° 2 : les cinq familles dirigées, sur chaque couché | [`les_cinq_familles_dirigees_distinguent_deux_led_de_chaque_ventilateur_couche`] | **rouge** |
//! | n° 3 : `balayage` suit la hauteur de face | [`balayage_bas_haut_et_haut_bas_suivent_la_hauteur_de_face`] | **rouge** |
//! | n° 4 : `balayage` suit la profondeur de face | [`balayage_avant_arriere_et_arriere_avant_suivent_la_profondeur_de_face`] | **rouge** |
//! | n° 5 : sens de `horaire` / `antihoraire` | [`horaire_va_de_l_arriere_vers_l_avant_en_haut_du_schema`] | **rouge** |
//! | n° 6 : locales et familles sans direction inchangées | [`les_directions_locales_et_les_familles_sans_direction_rendent_les_images_d_avant`] | vert |
//!
//! La première moitié du critère n° 1 est verte avant l'implémentation, et ce n'est pas un
//! défaut : les LED de même hauteur de face sont, sur le boîtier mesuré, des LED de même angle sur
//! deux anneaux de même hauteur de centre. Le rendu 3D les peint déjà pareil, puisqu'il peint
//! pareil **tout** un ventilateur couché. C'est la seconde moitié qui porte le rouge, et l'issue les
//! écrit ensemble pour cette raison : la première seule serait satisfaite par un anneau uniforme.
//!
//! # Ce que ce fichier ne vérifie pas
//!
//! ⚠️ **Le critère n° 7 — l'aperçu de la fenêtre montre un dégradé continu de bas en haut — n'a
//! pas de test ici.** L'aperçu peint ce que le démon envoie, c'est-à-dire les images que ce
//! fichier vérifie déjà LED par LED (critères n° 3 et 4) ; ce qui resterait à vérifier, c'est la
//! correspondance entre la position de face et le dessin de `Plan::nouveau`, qui vit dans
//! `reverb-gui` et que l'issue met hors scope (« le dessin de la fenêtre »). L'issue prévient
//! d'ailleurs que la fenêtre **rapetisse** ses anneaux : sur l'écran, l'ordre de deux LED de deux
//! anneaux différents peut donc s'inverser par rapport à la position de face à 55 mm. Un dégradé
//! « continu » à l'écran n'est pas un ordre strict, et ce critère se juge à l'œil sur la fenêtre.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi.

use reverb_anim::{Animation, CATALOGUE, Direction, Geometrie, Image, Palette, Reglages};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// Le domaine
// ---------------------------------------------------------------------------

/// Le rayon réel d'un anneau, en mm, posé à plat sur le schéma — la valeur que l'issue donne.
const RAYON: f32 = 55.0;

/// Durée d'un cycle, en pas, à la vitesse 1 — figée et vérifiée par `spec_sens.rs` (#49).
const PERIODE: u32 = 120;

/// Les six ventilateurs couchés : plancher puis plafond, comme `spec_disposition.rs` les nomme.
const COUCHES: [Position; 6] = [
    Position::BasGauche,
    Position::BasMilieu,
    Position::BasDroite,
    Position::HautGauche,
    Position::HautMilieu,
    Position::HautDroite,
];

/// Les cinq familles qui acceptent une direction, telles que l'issue les nomme.
///
/// Écrites en dur, puis **recoupées** avec `parametres_acceptes` par le test qui s'en sert : une
/// famille qui gagnerait le réglage `direction` sans figurer ici ferait échouer ce recoupement,
/// au lieu d'échapper au critère.
const DIRIGEES: [&str; 5] = ["vague", "comete", "respiration", "arc-en-ciel", "balayage"];

/// Les huit familles sans direction que l'issue déclare inchangées.
const SANS_DIRECTION: [&str; 8] = [
    "braise",
    "rotation",
    "pouls",
    "scintillement",
    "bougie",
    "nuee",
    "artifice",
    "thermique",
];

/// Les deux directions locales, que l'issue déclare inchangées.
const LOCALES: [Direction; 2] = [Direction::BordsCentre, Direction::CentreBords];

/// En deçà, deux coordonnées de face sont tenues pour égales, en mm.
///
/// Un demi-millimètre : très en dessous de l'écart entre deux LED voisines d'un même anneau
/// (`2 · 55 · sin 22,5° ≈ 42 mm`) et entre deux LED voisines d'une barrette (4 mm sur le boîtier
/// mesuré), mais au-dessus de l'arrondi d'un `f32` sur des coordonnées de quelques centaines.
const TOLERANCE_EGALITE: f32 = 0.5;

/// Part de l'étendue au-delà de laquelle deux LED ne sont plus comparées par leur instant
/// d'arrivée.
///
/// L'instant d'arrivée vit modulo la période : au-delà d'une demi-période, un retard et une avance
/// sont indiscernables. Deux cinquièmes, la valeur de `spec_sens.rs` et pour la même raison,
/// laissent de la marge sous cette moitié. L'ordre de deux LED plus éloignées découle de celui des
/// LED intermédiaires : la monotonie locale, sur toute la chaîne, est la monotonie globale.
const ECART_MAXIMAL: f32 = 0.4;

/// Part de l'étendue au-delà de laquelle deux LED doivent être atteintes à **deux pas
/// différents**, et non seulement dans le bon ordre.
///
/// Trois centièmes : 3,6 pas sur 120 si l'étendue du schéma couvre un cycle, donc au moins trois pas
/// d'écart, loin de l'arrondi d'un pas. En deçà, deux LED proches peuvent légitimement s'allumer au
/// même pas, et on n'exige que l'absence d'inversion.
const ECART_STRICT: f32 = 0.03;

/// Une couleur dont les trois composantes diffèrent deux à deux, comme dans `spec_animations.rs`.
const TEMOIN: Rgb = Rgb::new(0xff, 0x20, 0x80);

// ---------------------------------------------------------------------------
// Les LED et leur place sur le schéma
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
            Led::Ventilateur(position, led) => couleurs_du_ventilateur(image, position)[led],
            Led::Barrette(slot, led) => image.barrettes[slot][led],
        }
    }

    /// Le ventilateur qui la porte, s'il y en a un.
    fn ventilateur(self) -> Option<Position> {
        match self {
            Led::Ventilateur(position, _) => Some(position),
            Led::Barrette(..) => None,
        }
    }
}

/// Une place sur le schéma de la vue de face.
#[derive(Debug, Clone, Copy)]
struct Face {
    hauteur: f32,
    profondeur: f32,
}

/// La place de face d'une LED, calculée par la formule de l'issue.
fn face(geometrie: &Geometrie, led: Led) -> Face {
    match led {
        Led::Ventilateur(position, indice) => {
            let centre = geometrie.centre_ventilateur(position);
            let angle = f32::from(geometrie.orientation(position).angle_led(indice)).to_radians();
            Face {
                hauteur: centre.y + RAYON * angle.cos(),
                profondeur: centre.z - RAYON * angle.sin(),
            }
        }
        Led::Barrette(slot, indice) => {
            let place = geometrie
                .led_barrette(slot, indice)
                .unwrap_or_else(|| panic!("{} n'a pas de place dans le boîtier", led.nom()));
            Face {
                hauteur: place.y,
                profondeur: place.z,
            }
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

/// La clé de regroupement d'une hauteur : les bits du flottant, `-0.0` ramené à `0.0`.
///
/// L'égalité exacte, comme `spec_animations.rs` : deux LED de même angle sur deux anneaux de même
/// hauteur de centre ont la même hauteur de face au bit près, puisque la formule reçoit les mêmes
/// entrées. Un regroupement à la tolérance dirait autre chose — que deux LED **voisines** reçoivent
/// la même couleur, ce qui est faux d'un dégradé continu.
fn cle(valeur: f32) -> u32 {
    if valeur == 0.0 {
        0.0f32.to_bits()
    } else {
        valeur.to_bits()
    }
}

// ---------------------------------------------------------------------------
// Aides communes
// ---------------------------------------------------------------------------

/// Le boîtier réel : c'est sur lui que Nico a vu les couchés passer d'un bloc.
fn boitier() -> Geometrie {
    Geometrie::mesuree()
}

/// Une seconde géométrie, sans orientation particulière, construite par le décodeur.
fn geometrie_plate() -> Geometrie {
    let lignes: Vec<String> = Position::ALL
        .iter()
        .map(|position| format!("{} 0 horaire", position.slug()))
        .collect();
    Geometrie::decoder(&lignes.join("\n")).expect("la géométrie de test est valide")
}

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
}

/// Les réglages des mesures de sens : vitesse 1, pour qu'un cycle tienne en [`PERIODE`] pas, et
/// **sans palette**, comme le critère n° 1 le demande.
fn reglages(direction: Direction) -> Reglages {
    Reglages {
        // #149 : vitesse décimale
        vitesse: 1.0,
        direction,
        ..Reglages::default()
    }
}

/// Une LED est-elle allumée ? Le `balayage` n'a que deux intensités, 0 et 1 (README).
fn allumee(couleur: Rgb) -> bool {
    couleur != Rgb::new(0, 0, 0)
}

/// L'instant où le front du `balayage` atteint chaque LED, sur un cycle.
///
/// Le front, c'est le pas où la LED s'allume alors qu'elle était éteinte au pas précédent, lu en
/// boucle. Chaque LED doit en avoir **exactement un** par cycle : sans quoi « l'instant
/// d'arrivée » n'aurait pas de sens, et le test le dit plutôt que de choisir un des fronts.
fn arrivees(geometrie: &Geometrie, direction: Direction) -> Vec<(Led, u32)> {
    let balayage = animation("balayage");
    let reglages = reglages(direction);
    let images: Vec<Image> = (0..PERIODE)
        .map(|pas| balayage.image(geometrie, &reglages, pas))
        .collect();
    toutes_les_led()
        .into_iter()
        .map(|led| {
            let fronts: Vec<u32> = (0..PERIODE)
                .filter(|pas| {
                    let precedent = ((pas + PERIODE - 1) % PERIODE) as usize;
                    allumee(led.couleur(&images[*pas as usize]))
                        && !allumee(led.couleur(&images[precedent]))
                })
                .collect();
            assert_eq!(
                fronts.len(),
                1,
                "« balayage » en {} : {} s'allume {} fois par cycle (pas {fronts:?}) — un front qui \
                 traverse le boîtier l'atteint une fois",
                direction.slug(),
                led.nom(),
                fronts.len()
            );
            (led, fronts[0])
        })
        .collect()
}

/// Le retard de `vers` sur `depuis`, en pas, ramené dans `]-60, 60]`.
///
/// Positif : `vers` est atteinte **après** `depuis`.
fn retard(depuis: u32, vers: u32) -> i64 {
    let periode = i64::from(PERIODE);
    let brut = (i64::from(vers) - i64::from(depuis)).rem_euclid(periode);
    if brut > periode / 2 {
        brut - periode
    } else {
        brut
    }
}

/// Exige que le front du `balayage` suive une coordonnée de face, dans le sens donné.
///
/// `croissant` : le front va des petites coordonnées vers les grandes. Pour chaque paire de LED
/// séparées de plus de [`TOLERANCE_EGALITE`] et de moins de [`ECART_MAXIMAL`] de l'étendue, celle
/// que le front doit atteindre en second ne doit pas l'être avant l'autre — et strictement après
/// au-delà de [`ECART_STRICT`].
///
/// Exige aussi que chacun des six couchés ait été comparé **à lui-même** sur au moins une paire
/// stricte : c'est là que le rendu 3D les aplatit, et c'est ce que l'issue corrige.
fn exige_l_ordre(
    direction: Direction,
    nom_de_l_axe: &str,
    coordonnee: fn(Face) -> f32,
    croissant: bool,
) {
    let geometrie = boitier();
    let arrivees = arrivees(&geometrie, direction);
    let places: Vec<(Led, f32, u32)> = arrivees
        .iter()
        .map(|(led, instant)| (*led, coordonnee(face(&geometrie, *led)), *instant))
        .collect();
    let minimum = places
        .iter()
        .map(|(_, c, _)| *c)
        .fold(f32::INFINITY, f32::min);
    let maximum = places
        .iter()
        .map(|(_, c, _)| *c)
        .fold(f32::NEG_INFINITY, f32::max);
    let etendue = maximum - minimum;
    assert!(
        etendue > 0.0 && etendue.is_finite(),
        "appareil : la {nom_de_l_axe} de face a une étendue de {etendue}"
    );

    let mut fautes = Vec::new();
    let mut comparees = 0;
    let mut couches_ordonnes: Vec<Position> = Vec::new();
    for (i, une) in places.iter().enumerate() {
        for autre in &places[i + 1..] {
            let (petite, grande) = if une.1 <= autre.1 {
                (une, autre)
            } else {
                (autre, une)
            };
            let ecart = grande.1 - petite.1;
            if ecart < TOLERANCE_EGALITE || ecart > ECART_MAXIMAL * etendue {
                continue;
            }
            let (premiere, seconde) = if croissant {
                (petite, grande)
            } else {
                (grande, petite)
            };
            let observe = retard(premiere.2, seconde.2);
            let strict = ecart >= ECART_STRICT * etendue;
            comparees += 1;
            if observe < 0 || (strict && observe == 0) {
                fautes.push(format!(
                    "{} ({nom_de_l_axe} {:.1}, pas {}) puis {} ({nom_de_l_axe} {:.1}, pas {}) : \
                     retard {observe:+}",
                    premiere.0.nom(),
                    premiere.1,
                    premiere.2,
                    seconde.0.nom(),
                    seconde.1,
                    seconde.2
                ));
            } else if strict
                && let (Some(a), Some(b)) = (premiere.0.ventilateur(), seconde.0.ventilateur())
                && a == b
                && COUCHES.contains(&a)
                && !couches_ordonnes.contains(&a)
            {
                couches_ordonnes.push(a);
            }
        }
    }

    assert!(
        comparees > 0,
        "appareil : aucune paire de LED n'a été comparée en {}",
        direction.slug()
    );
    assert!(
        fautes.is_empty(),
        "« balayage » en {} n'atteint pas les LED dans l'ordre de leur {nom_de_l_axe} de face \
         ({} paires fautives sur {comparees}, les dix premières) : {}",
        direction.slug(),
        fautes.len(),
        fautes
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<String>>()
            .join(" ; ")
    );
    for couche in COUCHES {
        assert!(
            couches_ordonnes.contains(&couche),
            "« balayage » en {} : sur {}, aucune paire de ses propres LED n'est atteinte à deux pas \
             distincts dans l'ordre de leur {nom_de_l_axe} de face — le ventilateur passe d'un bloc",
            direction.slug(),
            couche.slug()
        );
    }
}

// ---------------------------------------------------------------------------
// Appareillage — deux tests verts par nature
// ---------------------------------------------------------------------------

#[test]
fn l_appareil_voit_six_ventilateurs_plats_en_3d_et_debout_sur_le_schema() {
    // Le défaut que l'issue décrit tient à une propriété de la géométrie : les huit LED d'un
    // couché ont la même hauteur dans le boîtier réel. Ce test la vérifie plutôt que de la
    // supposer — si elle cessait d'être vraie, les tests rouges de ce fichier ne prouveraient plus
    // rien du schéma.
    //
    // Et l'autre moitié : sur le schéma, le même anneau couvre bien plus d'un rayon de hauteur.
    // Le minimum sur huit LED à 45° d'écart est `2 · 55 · cos 22,5° ≈ 102 mm` ; un rayon (55 mm)
    // laisse donc une marge large tout en refusant un anneau aplati.
    let geometrie = boitier();
    for couche in COUCHES {
        let hauteurs_3d: Vec<f32> = (0..LEDS_PER_FAN as usize)
            .map(|led| {
                geometrie
                    .led_ventilateur(couche, led)
                    .unwrap_or_else(|| panic!("{} led {led} n'a pas de place", couche.slug()))
                    .y
            })
            .collect();
        assert!(
            hauteurs_3d
                .iter()
                .all(|y| (y - hauteurs_3d[0]).abs() < TOLERANCE_EGALITE),
            "{} n'est pas couché dans le boîtier mesuré (hauteurs {hauteurs_3d:?}) : le défaut de \
             l'issue ne s'y lit plus",
            couche.slug()
        );

        let hauteurs_de_face: Vec<f32> = (0..LEDS_PER_FAN as usize)
            .map(|led| face(&geometrie, Led::Ventilateur(couche, led)).hauteur)
            .collect();
        let etendue = hauteurs_de_face
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max)
            - hauteurs_de_face
                .iter()
                .copied()
                .fold(f32::INFINITY, f32::min);
        assert!(
            etendue > RAYON,
            "{} ne couvre que {etendue} mm de hauteur sur le schéma : l'anneau y serait aplati",
            couche.slug()
        );
    }
}

#[test]
fn l_appareil_lit_l_ordre_d_arrivee_du_balayage_sur_une_barrette() {
    // La mesure d'arrivée est vérifiée sur ce que l'issue **ne change pas** : une barrette garde
    // sa position réelle, verticale, et le front du `balayage` en `bas-haut` doit en atteindre la
    // LED du haut après celle du bas, aujourd'hui comme demain. Une mesure fausse ne le dirait pas.
    let geometrie = boitier();
    let arrivees = arrivees(&geometrie, Direction::BasHaut);
    let instant = |cherchee: Led| {
        arrivees
            .iter()
            .find(|(led, _)| *led == cherchee)
            .map(|(_, instant)| *instant)
            .expect("toutes les LED ont un instant d'arrivée")
    };
    for slot in 0..SLOT_COUNT {
        let (bas, haut) = (
            Led::Barrette(slot, 0),
            Led::Barrette(slot, LEDS_PER_STICK - 1),
        );
        assert!(
            face(&geometrie, bas).hauteur < face(&geometrie, haut).hauteur,
            "appareil : la barrette {slot} devait monter de sa LED 0 à sa LED {}",
            LEDS_PER_STICK - 1
        );
        let observe = retard(instant(bas), instant(haut));
        assert!(
            observe > 0,
            "appareil : en bas-haut, {} est atteinte au pas {} et {} au pas {} — la mesure \
             d'arrivée ne lit pas un ordre que personne ne conteste",
            bas.nom(),
            instant(bas),
            haut.nom(),
            instant(haut)
        );
    }
}

// ---------------------------------------------------------------------------
// Critère n° 1 — vague en bas-haut, sans palette
// ---------------------------------------------------------------------------

#[test]
fn vague_bas_haut_peint_pareil_deux_led_a_la_meme_hauteur_de_face() {
    // Critère d'acceptation n° 1, première moitié : « deux LED de ventilateur à la **même
    // hauteur de face** ont la même couleur ».
    //
    // Vert avant l'implémentation, et c'est attendu — voir l'en-tête : sur le boîtier mesuré, ces
    // paires sont des LED de même angle sur deux anneaux de même hauteur de centre. Ce test garde
    // ce que la nouvelle projection ne doit pas casser ; le test suivant porte le rouge.
    //
    // « À un pas quelconque » : tout un cycle à la vitesse 1, puis vingt pas à la vitesse par
    // défaut, pour ne pas dépendre d'une vitesse.
    let geometrie = boitier();
    let mut groupes: std::collections::BTreeMap<u32, Vec<Led>> = Default::default();
    for led in toutes_les_led()
        .into_iter()
        .filter(|l| l.ventilateur().is_some())
    {
        groupes
            .entry(cle(face(&geometrie, led).hauteur))
            .or_default()
            .push(led);
    }
    let groupes: Vec<Vec<Led>> = groupes.into_values().filter(|g| g.len() >= 2).collect();
    assert!(
        !groupes.is_empty(),
        "appareil : aucune LED de ventilateur n'en a une autre à sa hauteur de face"
    );
    assert!(
        groupes
            .iter()
            .any(|g| g.iter().any(|led| led.ventilateur() != g[0].ventilateur())),
        "appareil : aucun groupe de même hauteur de face ne traverse deux ventilateurs"
    );

    let vague = animation("vague");
    let a_la_vitesse_un = reglages(Direction::BasHaut);
    let par_defaut = Reglages {
        direction: Direction::BasHaut,
        ..Reglages::default()
    };
    assert_eq!(
        par_defaut.palette, None,
        "appareil : le critère se mesure sans palette"
    );
    let instants = (0..PERIODE)
        .map(|pas| (&a_la_vitesse_un, pas))
        .chain((0..20).map(|pas| (&par_defaut, pas * 7)));
    for (reglages, pas) in instants {
        let image = vague.image(&geometrie, reglages, pas);
        for groupe in &groupes {
            let attendue = groupe[0].couleur(&image);
            for led in &groupe[1..] {
                assert_eq!(
                    led.couleur(&image),
                    attendue,
                    "« vague » en bas-haut (vitesse {}, pas {pas}) : {} et {} sont à la même \
                     hauteur de face ({:.2}) et reçoivent deux couleurs différentes",
                    reglages.vitesse,
                    groupe[0].nom(),
                    led.nom(),
                    face(&geometrie, groupe[0]).hauteur
                );
            }
        }
    }
}

#[test]
fn vague_bas_haut_ne_peint_jamais_un_ventilateur_couche_d_une_seule_couleur() {
    // Critère d'acceptation n° 1, seconde moitié : « sur chacun des six ventilateurs couchés, les
    // 8 LED ne sont **pas** toutes de la même couleur » — à un pas quelconque, donc à **tous** les
    // pas d'un cycle.
    //
    // C'est le défaut que Nico a vu : `vague palette=red-flash direction=bas-haut`, et les six
    // couchés passent d'un bloc. Lu à tous les pas et non à un seul, parce que c'est ce que
    // « quelconque » veut dire, et que la marge le permet : sur le schéma, un anneau couvre
    // environ 106 mm de hauteur, un cinquième de l'étendue, soit une vingtaine de pas d'onde. Le
    // seul passage où la `vague` s'éteint tout à fait dure 5 pas (relevé sur une LED de barrette à
    // `19c5254`) : il ne peut pas couvrir un anneau entier.
    let geometrie = boitier();
    let vague = animation("vague");
    let reglages = reglages(Direction::BasHaut);
    for pas in 0..PERIODE {
        let image = vague.image(&geometrie, &reglages, pas);
        for couche in COUCHES {
            let couleurs = couleurs_du_ventilateur(&image, couche);
            assert!(
                couleurs.iter().any(|c| *c != couleurs[0]),
                "« vague » en bas-haut, pas {pas} : les huit LED de {} portent toutes {:?} — le \
                 ventilateur passe d'un bloc au lieu de suivre la hauteur de ses LED sur le schéma",
                couche.slug(),
                couleurs[0]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Critère n° 2 — les cinq familles dirigées, sur chaque couché
// ---------------------------------------------------------------------------

#[test]
fn les_cinq_familles_dirigees_distinguent_deux_led_de_chaque_ventilateur_couche() {
    // Critère d'acceptation n° 2 : « pour toutes les familles qui acceptent une direction, en
    // `bas-haut`, sur chaque ventilateur couché, au moins deux LED d'un même ventilateur diffèrent
    // à un même pas ».
    //
    // L'existence d'un tel pas, sur un cycle entier à la vitesse 1 : c'est ce que le critère
    // écrit, et c'est le plus faible qui attrape le défaut — aujourd'hui, les huit LED d'un couché
    // sont identiques à **tous** les pas, quelle que soit la famille.
    // #150 : `course` accepte aussi une direction ; ce recoupement porte sur les treize familles
    // que #148 connaissait. Sa lecture du schéma de face est vérifiée par `spec_150_course.rs`.
    let dirigees: Vec<&str> = CATALOGUE
        .iter()
        .copied()
        .filter(|nom| *nom != "course")
        .filter(|nom| animation(nom).parametres_acceptes().contains(&"direction"))
        .collect();
    assert_eq!(
        dirigees,
        DIRIGEES.to_vec(),
        "les familles qui acceptent une direction sont celles que l'issue nomme"
    );

    let geometrie = boitier();
    let reglages = reglages(Direction::BasHaut);
    for nom in DIRIGEES {
        let famille = animation(nom);
        let images: Vec<Image> = (0..PERIODE)
            .map(|pas| famille.image(&geometrie, &reglages, pas))
            .collect();
        for couche in COUCHES {
            let distingue = images.iter().any(|image| {
                let couleurs = couleurs_du_ventilateur(image, couche);
                couleurs.iter().any(|c| *c != couleurs[0])
            });
            assert!(
                distingue,
                "« {nom} » en bas-haut : les huit LED de {} portent la même couleur à chacun des \
                 {PERIODE} pas du cycle — ce ventilateur couché passe d'un bloc",
                couche.slug()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Critères n° 3 et 4 — l'ordre d'arrivée du balayage
// ---------------------------------------------------------------------------

#[test]
fn balayage_bas_haut_et_haut_bas_suivent_la_hauteur_de_face() {
    // Critère d'acceptation n° 3 : « `balayage` en `bas-haut` éclaire une LED plus haute sur le
    // schéma **après** une LED plus basse. L'ordre d'arrivée du front suit la hauteur de face, sur
    // tout le boîtier, ventilateurs couchés compris. »
    //
    // `haut-bas` en miroir : l'issue range les deux sous la même lecture, et une projection qui
    // n'aurait corrigé qu'un sens passerait la première moitié seule.
    exige_l_ordre(Direction::BasHaut, "hauteur", |f| f.hauteur, true);
    exige_l_ordre(Direction::HautBas, "hauteur", |f| f.hauteur, false);
}

#[test]
fn balayage_avant_arriere_et_arriere_avant_suivent_la_profondeur_de_face() {
    // Critère d'acceptation n° 4 : « même chose en `avant-arriere` avec la profondeur de face ».
    //
    // ⚠️ Le sens vient de `spec_sens.rs` (#49) et n'est pas rediscuté ici : `avant-arriere` part
    // du grand `z` et va vers le petit. La profondeur de face suit `z` — elle en est la formule —,
    // donc `avant-arriere` va des grandes profondeurs de face vers les petites.
    //
    // ⚠️ L'issue prévient que le schéma place l'angle 90° du radiateur vers l'avant, là où la table
    // 3D le met vers l'arrière, et que « le schéma fait foi ». Ce test suit le schéma : sur le
    // radiateur, il exige l'ordre de la formule, quelle que soit la table 3D.
    exige_l_ordre(
        Direction::AvantArriere,
        "profondeur",
        |f| f.profondeur,
        false,
    );
    exige_l_ordre(
        Direction::ArriereAvant,
        "profondeur",
        |f| f.profondeur,
        true,
    );
}

// ---------------------------------------------------------------------------
// Critère n° 5 — le sens de horaire sur le schéma
// ---------------------------------------------------------------------------

#[test]
fn horaire_va_de_l_arriere_vers_l_avant_en_haut_du_schema() {
    // Critère d'acceptation n° 5 : « `horaire` : à hauteur de face maximale, la LED la plus à
    // l'arrière est atteinte avant la plus à l'avant. `antihoraire` donne l'inverse. »
    //
    // Les LED du sommet du schéma sont **demandées à la formule**, jamais nommées : sur le boîtier
    // mesuré, ce sont les LED de même angle des trois anneaux du plafond, et rien ne doit figer
    // lequel. L'arrière est le grand `z` — à gauche sur le schéma —, l'avant le petit — à droite :
    // en haut, un mouvement horaire va de gauche à droite, donc de l'arrière vers l'avant.
    //
    // Mesuré sur le `balayage`, dont le front se date au pas près. Aujourd'hui, ces LED tournent
    // dans le plan vitre/hauteur, où les trois anneaux du plafond se superposent : elles sont
    // atteintes au même pas, et le retard exigé ici est nul.
    let geometrie = boitier();
    let leds = toutes_les_led();
    let sommet = leds
        .iter()
        .map(|led| face(&geometrie, *led).hauteur)
        .fold(f32::NEG_INFINITY, f32::max);
    let au_sommet: Vec<Led> = leds
        .into_iter()
        .filter(|led| face(&geometrie, *led).hauteur >= sommet - TOLERANCE_EGALITE)
        .collect();
    let profondeur = |led: &Led| face(&geometrie, *led).profondeur;
    let arriere = *au_sommet
        .iter()
        .max_by(|a, b| profondeur(a).total_cmp(&profondeur(b)))
        .expect("au moins une LED au sommet");
    let avant = *au_sommet
        .iter()
        .min_by(|a, b| profondeur(a).total_cmp(&profondeur(b)))
        .expect("au moins une LED au sommet");
    assert!(
        profondeur(&arriere) - profondeur(&avant) > TOLERANCE_EGALITE,
        "appareil : une seule profondeur au sommet du schéma ({:?}) — rien à ordonner",
        au_sommet.iter().map(|l| l.nom()).collect::<Vec<String>>()
    );

    for (direction, signe) in [(Direction::Horaire, 1), (Direction::Antihoraire, -1)] {
        let arrivees = arrivees(&geometrie, direction);
        let instant = |cherchee: Led| {
            arrivees
                .iter()
                .find(|(led, _)| *led == cherchee)
                .map(|(_, instant)| *instant)
                .expect("toutes les LED ont un instant d'arrivée")
        };
        let observe = retard(instant(arriere), instant(avant));
        assert!(
            observe * signe > 0,
            "« balayage » en {} : au sommet du schéma, {} (arrière, profondeur {:.1}) est atteinte \
             au pas {} et {} (avant, profondeur {:.1}) au pas {} — retard {observe:+}. En horaire, \
             le motif avance vers l'avant en haut du schéma ; en antihoraire, vers l'arrière.",
            direction.slug(),
            arriere.nom(),
            profondeur(&arriere),
            instant(arriere),
            avant.nom(),
            profondeur(&avant),
            instant(avant)
        );
    }
}

// ---------------------------------------------------------------------------
// Critère n° 6 — ce qui ne change pas, à l'octet près
// ---------------------------------------------------------------------------

/// Les pas échantillonnés, mêlant petits nombres, bords de période et grands nombres — ceux de
/// `spec_motifs_locaux.rs`.
const PAS: [u32; 18] = [
    0, 1, 2, 3, 5, 7, 11, 17, 29, 41, 59, 60, 61, 89, 119, 120, 240, 901,
];

/// Les vitesses échantillonnées : les deux bornes du domaine, et la valeur par défaut.
const VITESSES: [u8; 3] = [1, 3, 10];

/// Le nom de sonde donné à `thermique`, qui en exige un. Sa valeur n'entre pas dans l'image : c'est
/// la mesure passée à `image_avec_mesure` qui compte.
const SONDE: &str = "pompe";

/// L'empreinte d'une image : FNV-1a 64 bits sur les 372 octets, dans un ordre canonique.
///
/// Les ventilateurs sont parcourus dans l'ordre de `Position::ALL` et cherchés **par position**.
fn empreinte_image(image: &Image, valeur: &mut u64) {
    let mut avale = |octet: u8| {
        *valeur ^= u64::from(octet);
        *valeur = valeur.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for position in Position::ALL {
        for couleur in couleurs_du_ventilateur(image, position) {
            avale(couleur.r);
            avale(couleur.g);
            avale(couleur.b);
        }
    }
    for barrette in &image.barrettes {
        for couleur in barrette {
            avale(couleur.r);
            avale(couleur.g);
            avale(couleur.b);
        }
    }
}

/// L'empreinte d'une famille sous des directions données : deux géométries, deux couleurs, trois
/// vitesses, avec et sans palette, dix-huit pas — et, pour `thermique`, trois mesures de sonde.
fn empreinte(nom: &str, directions: &[Direction]) -> u64 {
    let famille = animation(nom);
    let geometries = [boitier(), geometrie_plate()];
    let couleurs = [TEMOIN, Rgb::new(0x20, 0xff, 0x80)];
    let palettes = [
        None,
        Some(Palette::par_nom("lava").expect("« lava » existe")),
    ];
    let mesures: &[Option<f32>] = if nom == "thermique" {
        &[None, Some(25.0), Some(55.0)]
    } else {
        &[None]
    };
    let sonde = (nom == "thermique").then(|| SONDE.to_owned());

    let mut valeur: u64 = 0xcbf2_9ce4_8422_2325;
    for direction in directions {
        for geometrie in &geometries {
            for couleur in couleurs {
                for vitesse in VITESSES {
                    for palette in palettes {
                        let reglages = Reglages {
                            couleur,
                            vitesse: f32::from(vitesse),
                            direction: *direction,
                            sonde: sonde.clone(),
                            palette,
                            // #150 : `largeur` rejoint `Reglages` ; 10 est son défaut (essai boîtier du 2026-10-31), sans effet hors `course`.
                            largeur: 10,
                        };
                        for mesure in mesures {
                            for pas in PAS {
                                let image =
                                    famille.image_avec_mesure(geometrie, &reglages, pas, *mesure);
                                empreinte_image(&image, &mut valeur);
                            }
                        }
                    }
                }
            }
        }
    }
    valeur
}

/// Les empreintes d'avant, relevées sur `main` à la révision `19c5254`, avant toute
/// implémentation, en appelant l'API publique (`Animation::image_avec_mesure`).
///
/// - Les cinq familles dirigées sous **chacune** des deux directions locales.
/// - Les huit familles sans direction, sous les **huit** directions repliées en une seule
///   empreinte : elles n'en acceptent aucune, et une famille qui se mettrait à en lire une
///   changerait d'image ici.
///
/// ⚠️ **Une empreinte qui bouge n'est jamais une empreinte à corriger.** L'issue #148 change le
/// rendu des directions globales, et lui seul ; si l'une de ces valeurs diffère après
/// implémentation, c'est le code qu'on corrige.
const EMPREINTES: [(&str, &str, u64); 18] = [
    ("vague", "bords-centre", 0x4621_cd31_4f88_6cc5),
    ("vague", "centre-bords", 0x5954_18ed_bf08_e691),
    ("comete", "bords-centre", 0x8ee5_cfd4_49ac_58fd),
    ("comete", "centre-bords", 0x2b0b_0010_e1d2_3bed),
    ("respiration", "bords-centre", 0xe2a9_e804_7200_2e85),
    ("respiration", "centre-bords", 0x038f_88b5_99cd_5a7d),
    ("arc-en-ciel", "bords-centre", 0x266e_fdf7_1b7f_b715),
    ("arc-en-ciel", "centre-bords", 0xd0d3_30a3_edd9_0945),
    ("balayage", "bords-centre", 0xe43a_c75b_445e_30f5),
    ("balayage", "centre-bords", 0x6799_539e_cce8_c905),
    ("braise", "toutes", 0xe685_6829_f295_2f75),
    ("rotation", "toutes", 0x9452_1e20_80b7_11a5),
    ("pouls", "toutes", 0x0602_9067_94dc_5205),
    ("scintillement", "toutes", 0x4f55_7c97_f3ab_f865),
    ("bougie", "toutes", 0xf944_c210_7405_af65),
    ("nuee", "toutes", 0xd046_67a4_536d_2825),
    ("artifice", "toutes", 0xa182_e6ac_83a7_35c5),
    ("thermique", "toutes", 0x1860_f685_67a3_7825),
];

#[test]
fn les_directions_locales_et_les_familles_sans_direction_rendent_les_images_d_avant() {
    // Critère d'acceptation n° 6 : « les directions locales rendent **exactement** les images
    // d'avant, et de même pour `braise`, `rotation`, `pouls`, `scintillement`, `bougie`, `nuee`,
    // `artifice`, `thermique` : images identiques à l'octet près ».
    //
    // C'est le filet du chantier : l'approche technique remplace `flux` et touche `projection`,
    // `angle_du_tour` et les bornes, c'est-à-dire du code que les directions locales et les
    // grandeurs sans axe pourraient partager. Une borne qui bouge d'un cheveu décale une image sans
    // un message.
    //
    // Toutes les empreintes fautives sont listées d'un coup, plutôt que la première seule : savoir
    // si une seule famille a bougé ou toutes à la fois dit déjà où chercher.
    let attendus: Vec<(&str, &str)> = DIRIGEES
        .iter()
        .flat_map(|nom| LOCALES.iter().map(move |d| (*nom, d.slug())))
        .chain(SANS_DIRECTION.iter().map(|nom| (*nom, "toutes")))
        .collect();
    assert_eq!(
        attendus,
        EMPREINTES
            .iter()
            .map(|(nom, slug, _)| (*nom, *slug))
            .collect::<Vec<(&str, &str)>>(),
        "appareil : la table couvre les cinq familles dirigées sous les deux locales, puis les huit \
         familles sans direction, dans cet ordre"
    );

    let mut fautes = Vec::new();
    for (nom, slug, attendue) in EMPREINTES {
        let directions: Vec<Direction> = if slug == "toutes" {
            Direction::ALL.to_vec()
        } else {
            LOCALES
                .iter()
                .copied()
                .filter(|d| d.slug() == slug)
                .collect()
        };
        let obtenue = empreinte(nom, &directions);
        if obtenue != attendue {
            fautes.push(format!(
                "(\"{nom}\", \"{slug}\") : 0x{obtenue:016x} au lieu de 0x{attendue:016x}"
            ));
        }
    }
    assert!(
        fautes.is_empty(),
        "{} cas ne rendent plus les images de 19c5254 — l'issue #148 ne change que les directions \
         globales : {}",
        fautes.len(),
        fautes.join(" ; ")
    );
}
