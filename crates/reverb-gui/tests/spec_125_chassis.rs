//! Tests d'intention du châssis rectangulaire de la vue de face (issue #125).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #125 seule. Rien de `crates/reverb-gui/src/`
//! n'a été lu pour les écrire, hormis les signatures publiques déjà figées par `spec_plan.rs`
//! (#23), `spec_habillage.rs` (#52) et `spec_boitier.rs` (#64) — `silhouette`, `habillage`,
//! `organes`, `faces`, `aretes`, `led_ventilateur`, `led_barrette`, `vue`.
//!
//! ## Ce que l'issue reproche au châssis d'aujourd'hui
//!
//! La silhouette de la vue de face est l'**enveloppe convexe** des cent vingt-quatre places de
//! LED, dilatée d'un rayon de pastille. Deux défauts en découlent, et aucun des deux ne lève quoi
//! que ce soit — ils se regardent :
//!
//! 1. le contour a des tranches obliques et une pointe vers le ventilateur arrière, là où un
//!    boîtier vu de face est un rectangle ;
//! 2. **les ventilateurs dépassent du trait.** L'enveloppe s'arrête aux centres de LED plus une
//!    pastille, quand le cadre d'un ventilateur est dessiné entre 1,20 et 1,25 fois son demi-axe.
//!    Vingt pour cent d'un demi-axe passent donc au travers de la ligne censée les contenir.
//!
//! ## Ce que « le rectangle » veut dire dans ce fichier
//!
//! Le critère d'acceptation n° 2 dit « hors de **ce** rectangle », et « ce rectangle » est celui
//! que le critère n° 1 définit : la silhouette elle-même. Les tests d'inclusion mesurent donc
//! l'appartenance aux **extrêmes de la silhouette**, puis vérifient que ces extrêmes *sont* la
//! silhouette — un rectangle à axes alignés à quatre sommets. Sans cette seconde moitié, « dans le
//! rectangle » se réduirait à « dans la boîte englobante d'une forme quelconque », qui est vrai de
//! n'importe quel polygone et ne dit rien.
//!
//! ⚠️ **L'ordre des deux assertions est délibéré.** L'inclusion vient d'abord, la forme ensuite :
//! en phase rouge, c'est le dépassement des cadres de ventilateur — le défaut n° 2, celui qui se
//! voit sur `cargo run --example apercu -p reverb-gui` — qui doit s'annoncer, et non la forme du
//! contour, que le test n° 1 tient déjà pour lui seul.
//!
//! ## Deux tests qui gardent ce qui ne doit pas changer
//!
//! Les tests n° 4 et n° 5 portent sur les critères n° 3 et n° 5, qui demandent tous deux qu'une
//! chose **reste vraie**. Ils passent donc avant travaux, et c'est leur rôle : la vue isométrique
//! est explicitement hors scope, et un rectangle « dilaté d'une marge » est justement la forme de
//! correction qui pousse un sommet hors du carré unité sans qu'aucune erreur ne le dise. Un
//! garde-fou qui n'existe qu'après la panne ne garde rien.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Que ce soit joli.** C'est la demande d'origine, et elle se regarde — l'issue renvoie pour
//!   cela au rendu hors écran `cargo run --release --example apercu -p reverb-gui`.
//! - **Que la silhouette suive la géométrie** (critère n° 4). `spec_habillage.rs` n° 2 le tient
//!   déjà, dans les deux vues, et ce fichier ne le respécifie pas — il se contente d'exiger la
//!   forme rectangulaire sous **deux** géométries différentes, ce qu'un polygone écrit à la main
//!   dans le `.slint` satisferait mais que `spec_habillage.rs` refuserait pour lui.
//! - **Que la silhouette contienne les LED en isométrie.** C'est `spec_habillage.rs` n° 1.
//! - Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne touche un périphérique : le plan est
//!   du calcul pur, et ses tests aussi.

use reverb_anim::{Geometrie, Orientation};
use reverb_gui::plan::{Place, Plan, Vue};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Nombre de LED d'un ventilateur, comme indice.
const LEDS: usize = LEDS_PER_FAN as usize;

/// Ce qu'on tolère sur une comparaison de places, en unités de cadre.
///
/// Le cadre va de 0 à 1, donc l'ulp d'une coordonnée y vaut au plus 1,2·10⁻⁷ : `EPSILON` absorbe
/// une dizaine d'aller-retours en `f32` sans jamais dépendre de l'ordre des opérations. Et il est
/// quatre ordres de grandeur sous ce qu'il doit attraper — le cadre d'un ventilateur déborde de
/// 0,20 à 0,25 fois son demi-axe, soit quelques centièmes de cadre. Une tolérance qui laisserait
/// passer ce dépassement-là rendrait ce fichier muet sur le défaut n° 2 de l'issue.
///
/// C'est la même valeur que `spec_habillage.rs`, pour la même raison et sur les mêmes grandeurs.
const EPSILON: f32 = 1e-6;

/// Un rectangle à axes alignés, dans le repère du cadre.
#[derive(Debug, Clone, Copy)]
struct Rectangle {
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
}

impl Rectangle {
    /// Les extrêmes d'une suite de sommets.
    ///
    /// C'est un **calcul**, sans jugement de forme : il rend une boîte pour n'importe quel
    /// polygone. Le jugement de forme est [`rectangle_a_axes_alignes`], et il est demandé
    /// séparément — voir l'en-tête, section « ce que "le rectangle" veut dire ».
    fn englobant(sommets: &[Place]) -> Rectangle {
        assert!(
            !sommets.is_empty(),
            "un contour sans sommet n'a pas d'extrêmes, et ce test ne prouverait rien"
        );
        let mut boite = Rectangle {
            x0: f32::INFINITY,
            x1: f32::NEG_INFINITY,
            y0: f32::INFINITY,
            y1: f32::NEG_INFINITY,
        };
        for sommet in sommets {
            boite.x0 = boite.x0.min(sommet.x);
            boite.x1 = boite.x1.max(sommet.x);
            boite.y0 = boite.y0.min(sommet.y);
            boite.y1 = boite.y1.max(sommet.y);
        }
        boite
    }

    /// Le point est-il dans le rectangle, à [`EPSILON`] près ?
    ///
    /// Le bord compte comme dedans : un sommet posé pile sur le châssis serait sinon déclaré
    /// dehors par une erreur d'arrondi.
    fn contient(&self, point: Place) -> bool {
        point.x >= self.x0 - EPSILON
            && point.x <= self.x1 + EPSILON
            && point.y >= self.y0 - EPSILON
            && point.y <= self.y1 + EPSILON
    }

    /// Les quatre coins, nommés dans le repère de l'écran — `y` va vers le bas.
    fn coins(&self) -> [(&'static str, Place); 4] {
        [
            (
                "haut gauche",
                Place {
                    x: self.x0,
                    y: self.y0,
                },
            ),
            (
                "haut droite",
                Place {
                    x: self.x1,
                    y: self.y0,
                },
            ),
            (
                "bas droite",
                Place {
                    x: self.x1,
                    y: self.y1,
                },
            ),
            (
                "bas gauche",
                Place {
                    x: self.x0,
                    y: self.y1,
                },
            ),
        ]
    }
}

/// Les valeurs deux à deux distinctes d'une liste, à [`EPSILON`] près, dans l'ordre croissant.
fn valeurs_distinctes(mut valeurs: Vec<f32>) -> Vec<f32> {
    valeurs.sort_by(|a, b| {
        a.partial_cmp(b)
            .expect("une coordonnée de sommet est un nombre fini, donc elle se compare")
    });
    valeurs.dedup_by(|a, b| (*a - *b).abs() <= EPSILON);
    valeurs
}

/// Exige d'un contour qu'il soit un rectangle à axes alignés, et rend ce rectangle.
///
/// Critère d'acceptation n° 1 : « quatre sommets, formant un rectangle à axes alignés (deux
/// abscisses distinctes, deux ordonnées distinctes) ».
///
/// Les quatre sommets doivent être les quatre **combinaisons** des deux abscisses et des deux
/// ordonnées, chacune une fois. Quatre sommets à deux abscisses et deux ordonnées ne suffisent
/// pas : `[(x0,y0), (x0,y0), (x1,y1), (x1,y1)]` les a tous et ne dessine qu'une diagonale.
fn rectangle_a_axes_alignes(quoi: &str, contour: &[Place]) -> Rectangle {
    for (i, sommet) in contour.iter().enumerate() {
        assert!(
            sommet.x.is_finite() && sommet.y.is_finite(),
            "{quoi} : le sommet {i} doit être un point fini — {sommet:?}"
        );
    }
    assert_eq!(
        contour.len(),
        4,
        "{quoi} : un boîtier vu de face est un rectangle, donc quatre sommets — il en a {} : \
         {contour:?}",
        contour.len()
    );

    let abscisses = valeurs_distinctes(contour.iter().map(|sommet| sommet.x).collect());
    let ordonnees = valeurs_distinctes(contour.iter().map(|sommet| sommet.y).collect());
    assert_eq!(
        abscisses.len(),
        2,
        "{quoi} : un rectangle à axes alignés n'a que deux abscisses distinctes, il en a {} \
         ({abscisses:?}) — ses tranches sont obliques : {contour:?}",
        abscisses.len()
    );
    assert_eq!(
        ordonnees.len(),
        2,
        "{quoi} : un rectangle à axes alignés n'a que deux ordonnées distinctes, il en a {} \
         ({ordonnees:?}) — ses tranches sont obliques : {contour:?}",
        ordonnees.len()
    );

    let rectangle = Rectangle {
        x0: abscisses[0],
        x1: abscisses[1],
        y0: ordonnees[0],
        y1: ordonnees[1],
    };
    for (nom, coin) in rectangle.coins() {
        let trouves = contour
            .iter()
            .filter(|sommet| {
                (sommet.x - coin.x).abs() <= EPSILON && (sommet.y - coin.y).abs() <= EPSILON
            })
            .count();
        assert_eq!(
            trouves, 1,
            "{quoi} : le coin {nom} ({coin:?}) doit être rendu une fois et une seule, il l'est \
             {trouves} fois — un quadrilatère à quatre sommets qui répète un coin dessine une \
             diagonale, pas un rectangle : {contour:?}"
        );
    }
    rectangle
}

/// Une géométrie qui ne diffère de la mesurée que par l'orientation de ses dix anneaux.
///
/// Vingt-deux degrés, un demi-pas d'anneau à un degré près : c'est le décalage qui déplace le plus
/// les huit LED d'un ventilateur sans rien changer d'autre — les centres et le rayon restent ceux
/// de la mesure (`spec_plan.rs` n° 9). Même vecteur que `spec_habillage.rs`.
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

/// Les deux géométries sur lesquelles la vue de face est jugée, avec de quoi les nommer.
fn geometries() -> [(&'static str, Geometrie); 2] {
    let deux = [
        ("géométrie mesurée", Geometrie::mesuree()),
        ("géométrie réorientée", geometrie_reorientee()),
    ];
    assert_ne!(
        deux[0].1, deux[1].1,
        "les deux géométries doivent bien différer, sinon les balayer deux fois ne prouve rien"
    );
    deux
}

/// La vue de face d'une géométrie, en vérifiant que c'en est bien une.
fn vue_de_face(geometrie: &Geometrie) -> Plan {
    let plan = Plan::nouveau(geometrie);
    assert_eq!(
        plan.vue(),
        Vue::Face,
        "`Plan::nouveau` rend la vue de face — c'est elle que cette issue corrige"
    );
    plan
}

/// Les cent vingt-quatre LED du boîtier et leur place, avec de quoi les nommer.
fn toutes_les_leds(plan: &Plan) -> Vec<(String, Place)> {
    let mut leds = Vec::new();
    for position in Position::ALL {
        for led in 0..LEDS {
            let place = plan
                .led_ventilateur(position, led)
                .unwrap_or_else(|| panic!("{} LED {led} doit avoir une place", position.slug()));
            leds.push((format!("{} LED {led}", position.slug()), place));
        }
    }
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            let place = plan
                .led_barrette(slot, led)
                .unwrap_or_else(|| panic!("barrette {slot} LED {led} doit avoir une place"));
            leds.push((format!("barrette {slot} LED {led}"), place));
        }
    }
    assert_eq!(
        leds.len(),
        Position::ALL.len() * LEDS + SLOT_COUNT * LEDS_PER_STICK,
        "le boîtier a cent vingt-quatre LED, et la maquette les montre toutes"
    );
    leds
}

// ---------------------------------------------------------------------------
// 1 — en vue de face, la silhouette est un rectangle à axes alignés
// ---------------------------------------------------------------------------

#[test]
fn en_vue_de_face_la_silhouette_est_un_rectangle_a_quatre_sommets_a_axes_alignes() {
    // Critère d'acceptation n° 1 : « En vue de face, `Plan::silhouette()` rend exactement quatre
    // sommets, formant un rectangle à axes alignés (deux abscisses distinctes, deux ordonnées
    // distinctes) ». Test d'intention n° 1 de l'issue.
    //
    // C'est le défaut n° 1 de l'issue : l'enveloppe convexe des cent vingt-quatre places de LED
    // donne un polygone irrégulier à tranches obliques, avec une pointe qui part vers le
    // ventilateur arrière. Rien ne le signale — il se dessine parfaitement, et il ne ressemble
    // simplement pas à un boîtier.
    //
    // Les deux géométries : la forme du châssis ne dépend pas de l'orientation des anneaux. Un
    // rectangle qui deviendrait un pentagone dès qu'un ventilateur est démonté puis remis serait
    // faux le jour où on ne le regarderait plus.
    for (quelle, geometrie) in geometries() {
        let plan = vue_de_face(&geometrie);
        let silhouette = plan.silhouette();
        let rectangle = rectangle_a_axes_alignes(
            &format!("la silhouette de la vue de face ({quelle})"),
            silhouette,
        );

        // Un rectangle enferme une surface : deux côtés de longueur nulle se dessinent sans erreur
        // et ne se voient pas. La distinction des coordonnées est faite à `EPSILON` près
        // ci-dessus, ce qui ne garantit qu'une épaisseur d'un millionième de cadre ; un boîtier
        // qui contient dix anneaux est bien plus large que ça.
        assert!(
            rectangle.x1 - rectangle.x0 > plan.rayon_anneau()
                && rectangle.y1 - rectangle.y0 > plan.rayon_anneau(),
            "le châssis de la vue de face ({quelle}) est plus petit qu'un anneau de ventilateur : \
             {rectangle:?} pour un rayon de {}",
            plan.rayon_anneau()
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — les cent vingt-quatre LED sont dans le rectangle
// ---------------------------------------------------------------------------

#[test]
fn en_vue_de_face_chaque_led_des_dix_ventilateurs_et_des_quatre_barrettes_est_dans_le_rectangle() {
    // Critère d'acceptation n° 2 : « Aucun sommet de l'habillage, aucune place de LED et aucun
    // sommet d'organe ne tombe hors de ce rectangle ». Test d'intention n° 2 de l'issue.
    //
    // Une LED dessinée hors du châssis ne lève rien : elle s'affiche, elle se clique, elle
    // s'allume — et elle flotte à côté du boîtier. C'est ce que `spec_habillage.rs` n° 1 tient
    // déjà contre l'enveloppe convexe ; il faut le tenir à nouveau contre le rectangle, qui est
    // une forme entièrement différente et qui pourrait très bien être plus **serrée** que
    // l'enveloppe sur un axe si elle était mal déduite.
    for (quelle, geometrie) in geometries() {
        let plan = vue_de_face(&geometrie);
        let silhouette = plan.silhouette();
        let boite = Rectangle::englobant(silhouette);

        for (nom, place) in toutes_les_leds(&plan) {
            assert!(
                boite.contient(place),
                "{nom} est dessinée hors du châssis en vue de face ({quelle}) : {place:?} n'est \
                 pas dans {boite:?}"
            );
        }

        // Et cette boîte est bien le rectangle du critère n° 1, pas la boîte englobante d'une
        // forme quelconque — voir l'en-tête. Sans cette ligne, « dans le rectangle » serait vrai
        // de n'importe quel contour et ne dirait rien.
        rectangle_a_axes_alignes(
            &format!("la silhouette de la vue de face ({quelle})"),
            silhouette,
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — l'habillage et les organes sont dans le rectangle
// ---------------------------------------------------------------------------

#[test]
fn en_vue_de_face_chaque_sommet_d_habillage_et_d_organe_est_dans_le_rectangle() {
    // Critère d'acceptation n° 2, seconde et troisième moitiés : « Aucun sommet de l'habillage
    // […] et aucun sommet d'organe ne tombe hors de ce rectangle ». Test d'intention n° 3 de
    // l'issue, et comportement attendu : « un rectangle à axes alignés qui contient tout ce qui
    // est dessiné — LED, cadres de ventilateur, corps de barrette, dalle, organes internes — plus
    // une marge ».
    //
    // C'est le défaut n° 2 de l'issue, et le seul des deux qui soit chiffré : le cadre d'un
    // ventilateur est dessiné entre 1,20 et 1,25 fois son demi-axe, quand l'enveloppe convexe
    // s'arrête aux centres de LED plus une pastille. Les trois du bas et les trois de droite
    // chevauchent donc la ligne censée les contenir — « les ventilateurs dépassent du trait ».
    //
    // Les faces et les arêtes s'y ajoutent : l'approche technique de l'issue les nomme comme
    // contenu à borner, et un trait de volume qui sortirait du châssis serait la même faute. La
    // vue de face peut n'en rendre aucune — les faces sont celles de l'isométrie (#52) —, d'où des
    // boucles qui n'exigent rien de leur nombre. Ce qui est exigé, c'est que l'habillage et les
    // organes, eux, existent : sinon ce test ne prouverait rien.
    for (quelle, geometrie) in geometries() {
        let plan = vue_de_face(&geometrie);
        let silhouette = plan.silhouette();
        let boite = Rectangle::englobant(silhouette);

        let dans = |quoi: String, sommet: Place| {
            assert!(
                sommet.x.is_finite() && sommet.y.is_finite(),
                "{quoi} ({quelle}) doit être un point fini — {sommet:?}"
            );
            assert!(
                boite.contient(sommet),
                "{quoi} est dessiné hors du châssis en vue de face ({quelle}) : {sommet:?} n'est \
                 pas dans {boite:?}"
            );
        };

        let habillage = plan.habillage();
        assert!(
            !habillage.is_empty(),
            "la vue de face habille son boîtier — cadres de ventilateur, corps de barrette, dalle \
             du Kraken (#64) —, sinon ce test ne prouve rien"
        );
        for forme in habillage {
            for (i, sommet) in forme.contour.iter().enumerate() {
                dans(
                    format!("le sommet {i} du contour de {:?}", forme.ornement),
                    *sommet,
                );
            }
            // Le creux d'un cadre percé est dessiné lui aussi, et il est plus étroit que son
            // contour : le borner ne coûte rien et ne laisse pas de sommet hors du balayage.
            for (i, sommet) in forme.creux.iter().enumerate() {
                dans(
                    format!("le sommet {i} du creux de {:?}", forme.ornement),
                    *sommet,
                );
            }
        }

        let organes = plan.organes();
        assert!(
            !organes.is_empty(),
            "la vue de face suggère ses organes internes — plateau de carte mère, carte graphique, \
             cache d'alimentation (#52) —, sinon ce test ne prouve rien"
        );
        for organe in organes {
            for (i, sommet) in organe.sommets.iter().enumerate() {
                dans(
                    format!("le sommet {i} de l'organe {:?}", organe.piece),
                    *sommet,
                );
            }
        }

        for face in plan.faces() {
            for (i, sommet) in face.sommets.iter().enumerate() {
                dans(
                    format!("le sommet {i} de la face {:?}", face.paroi),
                    *sommet,
                );
            }
        }

        for (i, (un, autre)) in plan.aretes().iter().enumerate() {
            dans(format!("le premier bout de l'arête {i}"), *un);
            dans(format!("le second bout de l'arête {i}"), *autre);
        }

        // Et cette boîte est bien le rectangle du critère n° 1 — même raison qu'au test n° 2.
        rectangle_a_axes_alignes(
            &format!("la silhouette de la vue de face ({quelle})"),
            silhouette,
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — l'isométrie garde son enveloppe convexe
// ---------------------------------------------------------------------------

#[test]
fn en_vue_isometrique_la_silhouette_garde_plus_de_quatre_sommets_et_reste_convexe() {
    // Critère d'acceptation n° 3 : « En vue isométrique, la silhouette garde plus de quatre
    // sommets — elle reste l'enveloppe convexe ». Test d'intention n° 4 de l'issue, et hors scope
    // déclaré : « La vue isométrique ».
    //
    // Ce test passe avant travaux, et c'est son rôle. La correction vit dans `cadrer`, que les
    // deux vues traversent : poser le rectangle sans distinguer la vue rendrait l'isométrie
    // rectangulaire elle aussi, ce qui lui ferait perdre les trois parois pleines qui la font lire
    // comme une boîte — sans une erreur, et sans que rien dans les critères 1 et 2 ne s'en émeuve.
    //
    // « Elle reste l'enveloppe convexe » se vérifie sans lire le code : une enveloppe convexe est
    // un polygone convexe, donc tous ses virages tournent dans le même sens.
    for (quelle, geometrie) in geometries() {
        let iso = Plan::isometrique(&geometrie);
        assert_eq!(
            iso.vue(),
            Vue::Isometrique,
            "`Plan::isometrique` rend la vue de trois-quarts, que cette issue ne touche pas"
        );

        let silhouette = iso.silhouette();
        assert!(
            silhouette.len() > 4,
            "la silhouette de la vue isométrique ({quelle}) a {} sommets : elle a été rabotée en \
             rectangle avec celle de la vue de face, alors que l'issue la déclare hors scope — \
             {silhouette:?}",
            silhouette.len()
        );

        // Convexité : le produit vectoriel de deux arêtes consécutives garde le même signe sur
        // tout le tour. Les triples alignés sont ignorés — une enveloppe a le droit de garder un
        // point sur un de ses côtés, ce qui ne la rend pas concave.
        let mut signe = 0.0f32;
        for i in 0..silhouette.len() {
            let a = silhouette[i];
            let b = silhouette[(i + 1) % silhouette.len()];
            let c = silhouette[(i + 2) % silhouette.len()];
            let croix = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
            if croix.abs() <= EPSILON {
                continue;
            }
            if signe == 0.0 {
                signe = croix;
            }
            assert!(
                croix * signe > 0.0,
                "la silhouette de la vue isométrique ({quelle}) tourne à l'envers au sommet {} : \
                 elle n'est plus convexe, donc plus une enveloppe convexe — {silhouette:?}",
                (i + 1) % silhouette.len()
            );
        }
        assert!(
            signe != 0.0,
            "tous les sommets de la silhouette isométrique ({quelle}) sont alignés : ce n'est pas \
             un polygone — {silhouette:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — les deux vues restent dans le carré unité
// ---------------------------------------------------------------------------

#[test]
fn dans_les_deux_vues_tous_les_sommets_de_silhouette_tiennent_dans_le_carre_unite() {
    // Critère d'acceptation n° 5 : « La maquette reste cadrée dans le carré unité : rien ne sort
    // de [0, 1]² ». Test d'intention n° 5 de l'issue. Contrat de #23 — `Place` : « normalisées de
    // 0 à 1 […] c'est la fenêtre qui les multiplie par sa taille du moment ».
    //
    // Ce test passe avant travaux, et c'est son rôle : l'approche technique de l'issue pose « la
    // silhouette = ce rectangle **dilaté d'une marge** », et une dilatation est exactement le
    // geste qui pousse un sommet à 1,02 sans qu'aucune erreur ne le dise. Le sommet se multiplie
    // par la taille de la fenêtre comme les autres, et le châssis déborde sur le reste de
    // l'interface. L'approche technique le prévoit — « étendre `bornes` de la silhouette obtenue,
    // comme aujourd'hui, pour que le cadrage la contienne » —, et c'est cette ligne-là que ce test
    // garde.
    for (quelle, geometrie) in geometries() {
        for (vue, plan) in [
            ("vue de face", vue_de_face(&geometrie)),
            ("vue isométrique", Plan::isometrique(&geometrie)),
        ] {
            let silhouette = plan.silhouette();
            assert!(
                silhouette.len() >= 3,
                "la silhouette de la {vue} ({quelle}) est un polygone fermé : il lui faut au moins \
                 trois sommets, elle en a {} — {silhouette:?}",
                silhouette.len()
            );
            for (i, sommet) in silhouette.iter().enumerate() {
                assert!(
                    sommet.x.is_finite() && sommet.y.is_finite(),
                    "le sommet {i} de la silhouette de la {vue} ({quelle}) doit être un point fini \
                     — {sommet:?}"
                );
                assert!(
                    (0.0..=1.0).contains(&sommet.x) && (0.0..=1.0).contains(&sommet.y),
                    "le sommet {i} de la silhouette de la {vue} ({quelle}) sort du carré unité — \
                     {sommet:?}"
                );
            }
        }
    }
}
