//! Tests d'intention de la projection du boîtier (issue #23).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #23, le contrat déposé dans
//! `crates/reverb-gui/src/plan.rs` — dont tous les corps sont `todo!("#23")` à l'écriture de ce
//! fichier — et `docs/GEOMETRIE.md`, qui est de la documentation, pas du code.
//!
//! ## Le repère, et pourquoi il n'est pas négociable
//!
//! Le boîtier est vu depuis le **panneau latéral gauche**, face à la carte mère : **l'arrière à
//! gauche** de l'écran, l'avant à droite, le haut en haut. C'est le repère dans lequel Nico « lit »
//! ses ventilateurs et dans lequel la géométrie a été relevée. Une fenêtre qui inverserait cet
//! axe fonctionnerait parfaitement et ferait cliquer sur le mauvais ventilateur à chaque fois,
//! sans qu'aucune erreur ne s'affiche — c'est la faute que ce fichier existe pour interdire.
//!
//! ## C'est un schéma, et c'est ce qui rend ces tests exigeants
//!
//! Vus depuis ce panneau, sept des dix ventilateurs sont vus **par la tranche** : une projection
//! orthographique honnête en ferait sept traits. Le contrat tranche : ils sont dessinés **en
//! cercles quand même**. De la même façon, la RAM et la colonne du radiateur sont à des
//! profondeurs différentes qu'une vue de face confondrait — physiquement la RAM masque une partie
//! du radiateur — et le contrat les dit **décalées exprès**.
//!
//! Conséquence : le plan n'est pas une projection, c'est une **mise en page**. Ce qu'on peut donc
//! exiger de lui n'est pas « telle LED est à telles coordonnées » — un nombre que personne n'a
//! choisi — mais des **relations** : l'ordre des axes, l'inclusion dans le cadre, la non-collision,
//! et le fait qu'un clic désigne ce qu'on visait. Toutes restent vraies quelle que soit la mise en
//! page retenue, et toutes seraient fausses si elle était bâclée.
//!
//! ## Trois points que le contrat laisse ouverts, et que ces tests tranchent
//!
//! 1. **Le diamètre d'une LED vaut le quart du rayon d'anneau.** Le contrat expose
//!    `rayon_anneau()` et dit que c'est « ce rayon qui décide si on peut cliquer une LED », mais
//!    il n'expose aucune taille de LED. Il en faut une pour écrire « rien ne se superpose ». Un
//!    quart du rayon est le choix qui se déduit du reste : huit LED réparties sur un anneau y sont
//!    à `2·R·sin(22,5°) ≈ 0,765·R` l'une de l'autre, donc un disque de `R/4` laisse entre deux
//!    voisines deux fois son propre diamètre — visiblement séparées, et visables à la souris.
//! 2. **Au-delà d'un rayon d'anneau de toute LED, c'est le vide.** Le contrat dit que `sous()`
//!    rend « la LED la plus proche **dans son rayon de prise**, et `None` au-delà » sans chiffrer
//!    ce rayon. Ces tests ne le chiffrent pas non plus : ils exigent seulement qu'il ne dépasse
//!    pas le rayon d'un anneau entier. Une prise plus large que tout un ventilateur ferait changer
//!    une couleur au hasard sur un clic manqué, ce que le contrat interdit en toutes lettres.
//! 3. **La colonne du radiateur reste une colonne.** `docs/GEOMETRIE.md` : « les trois du radiateur
//!    sont empilés verticalement sur le flanc du plateau de carte mère ». Ils partagent donc une
//!    abscisse — à un diamètre de LED près, pour laisser à la mise en page le droit de respirer.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! La fluidité, la lisibilité et le fait qu'une animation ressemble à sa phrase se vérifient à
//! l'œil ; l'issue le dit elle-même. Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne
//! touche un périphérique : la projection est du calcul pur, et ses tests aussi.

use reverb_anim::{Geometrie, Orientation, Sens};
use reverb_gui::plan::{Cible, Place, Plan};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Les trois du plancher, de l'arrière vers l'avant (`docs/GEOMETRIE.md`, « Disposition »).
const RANGEE_BASSE: [Position; 3] = [
    Position::BasGauche,
    Position::BasMilieu,
    Position::BasDroite,
];

/// Les trois du plafond, dans le même ordre.
const RANGEE_HAUTE: [Position; 3] = [
    Position::HautGauche,
    Position::HautMilieu,
    Position::HautDroite,
];

/// La colonne du radiateur, du bas vers le haut.
const COLONNE_RADIATEUR: [Position; 3] = [
    Position::RadiateurBas,
    Position::RadiateurMilieu,
    Position::RadiateurHaut,
];

/// Le nombre de LED du boîtier, recalculé depuis les constantes du matériel plutôt que recopié.
const LED_DU_BOITIER: usize =
    Position::ALL.len() * LEDS_PER_FAN as usize + SLOT_COUNT * LEDS_PER_STICK;

/// Le plan de la géométrie mesurée sur SHYNAEL.
fn plan() -> Plan {
    Plan::nouveau(&Geometrie::mesuree())
}

/// La distance à l'écran entre deux places.
fn distance(a: Place, b: Place) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Le diamètre d'une LED dessinée, en unités de [`Place`].
///
/// Voir le point n° 1 de l'en-tête : le contrat n'en expose pas, il en faut un pour écrire « rien
/// ne se superpose », et le quart du rayon d'anneau est celui qui se déduit du reste.
fn diametre_led(plan: &Plan) -> f32 {
    plan.rayon_anneau() / 4.0
}

/// Les 124 LED du boîtier, avec de quoi les nommer dans un message d'échec.
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
                .unwrap_or_else(|| panic!("barrette {slot} LED {led} doit avoir une place"));
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
        "le boîtier a cent vingt-quatre LED, et la maquette les montre toutes"
    );
    leds
}

/// Une géométrie qui ne diffère de la mesurée que par les angles et les sens de ses anneaux.
fn geometrie_reorientee() -> Geometrie {
    let mut geometrie = Geometrie::mesuree();
    for (i, position) in Position::ALL.into_iter().enumerate() {
        // Pas de 37° : premier avec 360, donc dix angles deux à deux distincts, et aucun qui
        // retombe sur un multiple de 45 où le pas de l'anneau pourrait masquer la faute.
        let angle = (i as u16 * 37 + 13) % 360;
        let sens = if i.is_multiple_of(2) {
            Sens::Antihoraire
        } else {
            Sens::Horaire
        };
        geometrie.definir(
            position,
            Orientation::new(angle, sens).expect("un angle dans le tour est une orientation"),
        );
    }
    geometrie
}

// ---------------------------------------------------------------------------
// 1 — l'arrière est à gauche, et les rangées vont de l'arrière vers l'avant
// ---------------------------------------------------------------------------

#[test]
fn l_arriere_est_a_gauche_de_tout_et_les_rangees_vont_vers_l_avant() {
    // Comportement attendu de l'issue : « l'arrière du boîtier est donc à gauche de l'écran,
    // l'avant à droite ». Test d'intention n° 6 : « dans le repère "panneau gauche", l'arrière du
    // boîtier est à gauche de l'avant ».
    //
    // Un axe inversé ne casse rien et ne dit rien : la fenêtre s'ouvre, les LED s'allument, et
    // chaque clic tombe sur le ventilateur symétrique de celui qu'on visait. C'est le seul défaut
    // du chantier qui puisse survivre à une relecture — d'où deux formulations indépendantes de la
    // même contrainte : le ventilateur du fond est le plus à gauche, et chaque rangée est ordonnée.
    let plan = plan();

    let arriere = plan.centre_ventilateur(Position::Arriere);
    for position in Position::ALL {
        if position == Position::Arriere {
            continue;
        }
        let autre = plan.centre_ventilateur(position);
        assert!(
            arriere.x < autre.x,
            "« arrière » est le ventilateur du fond : il est à gauche de {} — {arriere:?} contre \
             {autre:?}",
            position.slug()
        );
    }

    // `docs/GEOMETRIE.md` : « les trois du bas et les trois du dessus s'alignent d'avant en
    // arrière […] "gauche" est le plus proche de l'arrière du boîtier ». Donc « bas gauche » est à
    // gauche de « bas milieu », lui-même à gauche de « bas droite ».
    for rangee in [RANGEE_BASSE, RANGEE_HAUTE] {
        for paire in rangee.windows(2) {
            let [avant, apres] = paire else {
                unreachable!()
            };
            let (a, b) = (
                plan.centre_ventilateur(*avant),
                plan.centre_ventilateur(*apres),
            );
            assert!(
                a.x < b.x,
                "{} est plus près de l'arrière que {}, donc plus à gauche — {a:?} contre {b:?}",
                avant.slug(),
                apres.slug()
            );
        }
    }

    // Et dix ventilateurs, dix places : deux centres confondus feraient deux ventilateurs
    // impossibles à distinguer, donc impossibles à choisir.
    for (i, un) in Position::ALL.into_iter().enumerate() {
        for autre in Position::ALL.into_iter().skip(i + 1) {
            assert_ne!(
                plan.centre_ventilateur(un),
                plan.centre_ventilateur(autre),
                "{} et {} ne peuvent pas être au même endroit",
                un.slug(),
                autre.slug()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 2 — le haut est en haut, et le radiateur entre les deux
// ---------------------------------------------------------------------------

#[test]
fn le_haut_est_au_dessus_du_bas_et_le_radiateur_tient_le_milieu() {
    // Test d'intention n° 6 de l'issue : « et le haut au-dessus du bas ». `Place` documente que
    // `y` va **vers le bas**, convention de l'écran : le haut du boîtier est donc le petit `y`.
    // C'est la seconde chance de se tromper d'axe, et elle est plus vicieuse que la première —
    // trois ventilateurs de plafond dessinés au sol, ça se voit ; le radiateur passé de l'autre
    // côté du milieu, non.
    let plan = plan();
    let toit = RANGEE_HAUTE.map(|p| plan.centre_ventilateur(p).y);
    let sol = RANGEE_BASSE.map(|p| plan.centre_ventilateur(p).y);
    let plus_bas_du_toit = toit.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let plus_haut_du_sol = sol.iter().copied().fold(f32::INFINITY, f32::min);

    assert!(
        plus_bas_du_toit < plus_haut_du_sol,
        "les trois du plafond sont au-dessus des trois du plancher : {toit:?} contre {sol:?}"
    );

    // Le radiateur est empilé sur le flanc, entre le plancher et le plafond.
    for position in COLONNE_RADIATEUR {
        let centre = plan.centre_ventilateur(position);
        assert!(
            plus_bas_du_toit < centre.y && centre.y < plus_haut_du_sol,
            "{} tient le milieu du boîtier : {centre:?} entre {plus_bas_du_toit} et \
             {plus_haut_du_sol}",
            position.slug()
        );
    }

    // Et il est empilé dans le bon ordre : « radiateur bas » est bien le plus bas.
    for paire in COLONNE_RADIATEUR.windows(2) {
        let [dessous, dessus] = paire else {
            unreachable!()
        };
        let (a, b) = (
            plan.centre_ventilateur(*dessous),
            plan.centre_ventilateur(*dessus),
        );
        assert!(
            b.y < a.y,
            "{} est au-dessus de {} — {b:?} contre {a:?}",
            dessus.slug(),
            dessous.slug()
        );
    }

    // Une colonne, pas un escalier : les trois partagent la même profondeur dans le boîtier, donc
    // la même abscisse à l'écran, à un diamètre de LED près (voir le point n° 3 de l'en-tête).
    let abscisses = COLONNE_RADIATEUR.map(|p| plan.centre_ventilateur(p).x);
    for x in abscisses {
        assert!(
            (x - abscisses[0]).abs() <= diametre_led(&plan),
            "le radiateur est une colonne verticale, pas un escalier : {abscisses:?}"
        );
    }

    // Les quatre barrettes sont entre le plancher et le plafond, elles aussi.
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            let place = plan
                .led_barrette(slot, led)
                .unwrap_or_else(|| panic!("barrette {slot} LED {led} doit avoir une place"));
            assert!(
                plus_bas_du_toit < place.y && place.y < plus_haut_du_sol,
                "la barrette {slot} se dresse entre le plancher et le plafond : {place:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — tout tient dans le cadre, anneaux compris
// ---------------------------------------------------------------------------

#[test]
fn tout_le_boitier_tient_dans_le_cadre_anneaux_compris() {
    // Test d'intention n° 7 de l'issue : « les dix ventilateurs et les quatre barrettes tiennent
    // dans le cadre », et contrat — `Place` : « normalisées de 0 à 1 […] c'est la fenêtre qui les
    // multiplie par sa taille du moment ».
    //
    // Une place hors du cadre ne lève rien : elle se multiplie par la taille de la fenêtre comme
    // les autres, et la LED se dessine hors de la zone visible. La cible existe, elle est
    // inatteignable, et on croit qu'un ventilateur est mort.
    let plan = plan();
    let rayon = plan.rayon_anneau();
    assert!(
        rayon > 0.0 && rayon < 0.5,
        "un rayon d'anneau doit être une longueur, et une qui tienne dans le cadre : {rayon}"
    );

    for (nom, _, place) in toutes_les_leds(&plan) {
        assert!(
            place.x.is_finite() && place.y.is_finite(),
            "{nom} doit avoir une place finie : {place:?}"
        );
        assert!(
            (0.0..=1.0).contains(&place.x) && (0.0..=1.0).contains(&place.y),
            "{nom} sort du cadre : {place:?}"
        );
    }

    // Anneaux compris : un ventilateur dont le centre tient dans le cadre mais dont l'anneau
    // déborde se dessine tronqué, et les LED qui manquent sont celles qu'on ne pourra pas cliquer.
    for position in Position::ALL {
        let centre = plan.centre_ventilateur(position);
        for (axe, valeur) in [("x", centre.x), ("y", centre.y)] {
            assert!(
                valeur - rayon >= 0.0 && valeur + rayon <= 1.0,
                "l'anneau de {} déborde du cadre en {axe} : centre {centre:?}, rayon {rayon}",
                position.slug()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — rien ne se superpose, y compris la RAM et le radiateur
// ---------------------------------------------------------------------------

#[test]
fn aucune_led_n_en_recouvre_une_autre_y_compris_la_ram_et_le_radiateur() {
    // Test d'intention n° 7 de l'issue : « aucun ne se superpose ». Contrat — « physiquement, la
    // RAM masque une partie du radiateur ; sur le plan, elle est décalée pour que les deux restent
    // cliquables ».
    //
    // C'est la contrainte qui fait du plan une mise en page plutôt qu'une projection : dans le
    // boîtier, les barrettes et la colonne du radiateur occupent le même rectangle vu de ce
    // panneau. Les projeter telles quelles rendrait la moitié des cibles inatteignables, et la
    // fenêtre n'aurait pas d'autre symptôme qu'un clic qui change la mauvaise couleur.
    let plan = plan();
    let leds = toutes_les_leds(&plan);
    let minimum = diametre_led(&plan);

    let mut le_pire: Option<(f32, String)> = None;
    for (i, (nom, cible, place)) in leds.iter().enumerate() {
        for (autre_nom, autre_cible, autre_place) in leds.iter().skip(i + 1) {
            if cible_commune(cible, autre_cible) {
                continue;
            }
            let ecart = distance(*place, *autre_place);
            if le_pire.as_ref().is_none_or(|(pire, _)| ecart < *pire) {
                le_pire = Some((ecart, format!("{nom} et {autre_nom}")));
            }
            assert!(
                ecart >= minimum,
                "{nom} ({place:?}) et {autre_nom} ({autre_place:?}) sont à {ecart} l'une de \
                 l'autre, moins qu'un diamètre de LED ({minimum}) : elles se recouvrent"
            );
        }
    }

    // Le message qui suit ne sert qu'à faire apparaître la marge réelle dans la sortie d'un échec
    // futur — mais il vérifie aussi qu'on a bien comparé quelque chose.
    let (ecart, coupable) = le_pire.expect("cent vingt-quatre LED font des paires à comparer");
    assert!(
        ecart >= minimum,
        "la paire la plus serrée du boîtier est {coupable}, à {ecart}"
    );

    // Des LED écartées ne suffisent pas : ce qui se dessine, ce sont des **anneaux** et des
    // **réglettes**, pas cent vingt-quatre points isolés. Une barrette peut se glisser entre deux
    // LED d'un anneau sans en toucher aucune, et masquer le ventilateur quand même — c'est
    // exactement la superposition que le contrat décrit entre la RAM et la colonne du radiateur, et
    // exactement celle qu'un plan qui projetterait sans décaler produirait.
    let rayon = plan.rayon_anneau();
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            let place = plan
                .led_barrette(slot, led)
                .unwrap_or_else(|| panic!("barrette {slot} LED {led} doit avoir une place"));
            for position in Position::ALL {
                let centre = plan.centre_ventilateur(position);
                let ecart = distance(place, centre);
                assert!(
                    ecart > rayon + minimum,
                    "la barrette {slot} LED {led} ({place:?}) tombe sur l'anneau de {} \
                     ({centre:?}, rayon {rayon}) : à {ecart}, les deux ne sont pas cliquables \
                     séparément",
                    position.slug()
                );
            }
        }
    }

    // Et deux anneaux ne se chevauchent pas davantage, pour la même raison : huit LED de l'un
    // peuvent s'intercaler entre huit LED de l'autre sans qu'aucune paire ne soit serrée, et les
    // deux ventilateurs seraient pourtant dessinés l'un sur l'autre.
    for (i, un) in Position::ALL.into_iter().enumerate() {
        for autre in Position::ALL.into_iter().skip(i + 1) {
            let ecart = distance(plan.centre_ventilateur(un), plan.centre_ventilateur(autre));
            assert!(
                ecart > 2.0 * rayon + minimum,
                "les anneaux de {} et de {} se chevauchent : {ecart} entre leurs centres, pour \
                 deux rayons de {rayon}",
                un.slug(),
                autre.slug()
            );
        }
    }
}

/// Deux LED de la même cible adressable — même ventilateur, ou même barrette.
///
/// Elles ont le droit d'être serrées : les onze LED d'une barrette se suivent le long d'une
/// réglette, et personne ne demande à en cliquer une plutôt qu'une autre pour choisir la barrette.
fn cible_commune(une: &Cible, autre: &Cible) -> bool {
    match (une, autre) {
        (Cible::Led { position: a, .. }, Cible::Led { position: b, .. }) => a == b,
        (Cible::Barrette { slot: a, .. }, Cible::Barrette { slot: b, .. }) => a == b,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// 5 — les huit LED d'un ventilateur sont distinctes, et visables
// ---------------------------------------------------------------------------

#[test]
fn les_huit_led_d_un_ventilateur_sont_distinctes_et_visables_a_la_souris() {
    // Test d'intention n° 8 de l'issue : « les huit LED d'un ventilateur sont distinctes deux à
    // deux à l'écran — sans quoi on ne peut pas en cliquer une ». Critère d'acceptation : « on
    // choisit une LED précise, en cliquant dessus dans l'aperçu ».
    //
    // C'est la raison même pour laquelle les sept ventilateurs vus par la tranche sont dessinés en
    // cercles : sur un trait, les huit LED se confondraient et la cible « une LED précise »
    // n'existerait plus.
    let plan = plan();
    let minimum = diametre_led(&plan);
    let rayon = plan.rayon_anneau();

    for position in Position::ALL {
        let centre = plan.centre_ventilateur(position);
        let anneau: Vec<Place> = (0..LEDS_PER_FAN as usize)
            .map(|led| {
                plan.led_ventilateur(position, led)
                    .unwrap_or_else(|| panic!("{} LED {led} doit avoir une place", position.slug()))
            })
            .collect();

        for (i, place) in anneau.iter().enumerate() {
            // Chaque LED est sur l'anneau de son ventilateur, pas ailleurs : c'est ce qui donne un
            // sens au rayon que le contrat expose.
            let ecart = distance(*place, centre);
            assert!(
                ecart <= rayon * 1.001,
                "la LED {i} de {} est à {ecart} de son centre, au-delà du rayon {rayon}",
                position.slug()
            );

            // Et le centre le plus proche d'une LED est celui de son propre ventilateur — sans
            // quoi le dessin dirait une chose et le modèle une autre.
            for autre in Position::ALL {
                if autre == position {
                    continue;
                }
                assert!(
                    distance(*place, plan.centre_ventilateur(autre)) > ecart,
                    "la LED {i} de {} est plus près du centre de {} que du sien",
                    position.slug(),
                    autre.slug()
                );
            }

            for (j, voisine) in anneau.iter().enumerate().skip(i + 1) {
                let ecart = distance(*place, *voisine);
                assert!(
                    ecart >= minimum,
                    "les LED {i} et {j} de {} sont à {ecart} l'une de l'autre, moins qu'un \
                     diamètre de LED ({minimum}) : on ne peut pas viser l'une plutôt que l'autre",
                    position.slug()
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 6 — au-delà du dernier indice, il n'y a plus de LED
// ---------------------------------------------------------------------------

#[test]
fn au_dela_du_dernier_indice_il_n_y_a_plus_de_led() {
    // Contrat — `led_ventilateur` : « une des huit LED d'un ventilateur, `None` au-delà », et
    // `led_barrette` : « une des onze LED d'une barrette, `None` au-delà ».
    //
    // Rendre une place pour une LED qui n'existe pas est pire que de planter : la fenêtre
    // dessinerait une neuvième LED sur un ventilateur qui en a huit, et un clic dessus enverrait
    // au démon une cible qu'il refuserait — avec un message qui parlerait du protocole alors que
    // la faute est ici.
    let plan = plan();

    for position in Position::ALL {
        for led in 0..LEDS_PER_FAN as usize {
            assert!(
                plan.led_ventilateur(position, led).is_some(),
                "{} a bien une LED {led}",
                position.slug()
            );
        }
        for led in [
            LEDS_PER_FAN as usize,
            LEDS_PER_FAN as usize + 1,
            100,
            usize::MAX,
        ] {
            assert_eq!(
                plan.led_ventilateur(position, led),
                None,
                "{} n'a que {LEDS_PER_FAN} LED, pas de {led}-ième",
                position.slug()
            );
        }
    }

    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            assert!(
                plan.led_barrette(slot, led).is_some(),
                "la barrette {slot} a bien une LED {led}"
            );
        }
        for led in [LEDS_PER_STICK, LEDS_PER_STICK + 1, 100, usize::MAX] {
            assert_eq!(
                plan.led_barrette(slot, led),
                None,
                "une barrette n'a que {LEDS_PER_STICK} LED, pas de {led}-ième"
            );
        }
    }

    // Et une barrette qui n'est pas montée n'a aucune LED du tout — pas une rangée de places à
    // zéro, qu'on dessinerait dans le coin du cadre.
    for slot in [SLOT_COUNT, SLOT_COUNT + 1, 42, usize::MAX] {
        for led in [0, 1, LEDS_PER_STICK - 1] {
            assert_eq!(
                plan.led_barrette(slot, led),
                None,
                "il n'y a que {SLOT_COUNT} barrettes, pas de {slot}-ième"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 7 — un clic désigne ce qu'on visait, et le vide ne désigne rien
// ---------------------------------------------------------------------------

#[test]
fn un_clic_sur_une_led_la_designe_et_un_clic_dans_le_vide_ne_designe_rien() {
    // Contrat — `sous()` : « c'est la fonction du clic. Elle rend la LED la plus proche dans son
    // rayon de prise, et `None` au-delà : cliquer dans le vide ne doit pas changer une couleur au
    // hasard. »
    //
    // Les deux moitiés comptent autant. Une prise trop courte rend l'aperçu inutilisable, ce qu'on
    // voit tout de suite ; une prise sans limite change la couleur d'une LED qu'on ne visait pas,
    // ce qu'on ne relie jamais au clic qui l'a causée.
    let plan = plan();
    let leds = toutes_les_leds(&plan);

    // On vise à côté du centre, comme une main le fait : au quart de la distance qui sépare la LED
    // de sa plus proche voisine, dans les quatre directions. La LED visée reste la plus proche —
    // toute autre est au moins trois fois plus loin.
    for (nom, cible, place) in &leds {
        let voisine = leds
            .iter()
            .filter(|(_, autre, _)| autre != cible)
            .map(|(_, _, autre)| distance(*place, *autre))
            .fold(f32::INFINITY, f32::min);
        let ecart = voisine / 4.0;
        for (dx, dy) in [(ecart, 0.0), (-ecart, 0.0), (0.0, ecart), (0.0, -ecart)] {
            let vise = Place {
                x: place.x + dx,
                y: place.y + dy,
            };
            assert_eq!(
                plan.sous(vise),
                Some(*cible),
                "un clic en {vise:?}, à {ecart} du centre de {nom} ({place:?}), désigne {nom}"
            );
        }
    }

    // Le vide : tout point du cadre dont la LED la plus proche est à plus d'un anneau entier. Le
    // balayage sert aussi de garde-fou contre un test creux — s'il ne trouvait aucun point vide,
    // il ne prouverait rien, et il le dit.
    let rayon = plan.rayon_anneau();
    let pas = 100;
    let mut vides = 0usize;
    for i in 0..=pas {
        for j in 0..=pas {
            let place = Place {
                x: i as f32 / pas as f32,
                y: j as f32 / pas as f32,
            };
            let plus_proche = leds
                .iter()
                .map(|(_, _, autre)| distance(place, *autre))
                .fold(f32::INFINITY, f32::min);
            if plus_proche > rayon {
                assert_eq!(
                    plan.sous(place),
                    None,
                    "en {place:?}, la LED la plus proche est à {plus_proche}, plus loin qu'un \
                     anneau entier ({rayon}) : c'est du vide"
                );
                vides += 1;
            }
        }
    }
    assert!(
        vides > 0,
        "un boîtier laisse du vide entre ses ventilateurs, sinon ce test ne prouve rien"
    );

    // Et hors du cadre, il n'y a rien du tout : la fenêtre est plus grande que la maquette, et un
    // clic dans sa marge ne doit pas se rabattre sur la LED la plus proche.
    for dehors in [
        Place { x: -1.0, y: -1.0 },
        Place { x: 2.0, y: 0.5 },
        Place { x: 0.5, y: -3.0 },
        Place { x: 10.0, y: 10.0 },
    ] {
        assert_eq!(
            plan.sous(dehors),
            None,
            "{dehors:?} est hors de la maquette"
        );
    }
}

// ---------------------------------------------------------------------------
// 8 — le centre exact d'une LED désigne cette LED, pas sa voisine
// ---------------------------------------------------------------------------

#[test]
fn un_clic_au_centre_exact_d_une_led_designe_cette_led_et_pas_sa_voisine() {
    // Le cas limite de `sous()` : la place que le plan a lui-même rendue. Si le clic le plus facile
    // à réussir — celui qui tombe pile — ne désigne pas sa LED, aucune des cent vingt-quatre n'est
    // fiable, et c'est la cible « une LED précise » qui tombe.
    //
    // Exhaustif sur les 124, et non sur un échantillon : une confusion ne toucherait qu'une paire,
    // et ce sont justement les paires les plus serrées — les onze LED d'une barrette, à quelques
    // millimètres l'une de l'autre dans le boîtier — qu'un échantillon manquerait.
    let plan = plan();
    for (nom, cible, place) in toutes_les_leds(&plan) {
        assert_eq!(
            plan.sous(place),
            Some(cible),
            "le centre de {nom} ({place:?}) désigne {nom}, pas une voisine"
        );
    }
}

// ---------------------------------------------------------------------------
// 9 — le plan ne dépend pas de l'orientation des anneaux
// ---------------------------------------------------------------------------

#[test]
fn le_plan_ne_depend_pas_de_l_orientation_des_anneaux() {
    // `docs/GEOMETRIE.md` : « ces valeurs doivent rester modifiables depuis la fenêtre — un
    // ventilateur démonté puis remis reprend une orientation quelconque ». L'orientation dit **où
    // commence l'anneau**, pas où est le ventilateur.
    //
    // La conséquence tient la fenêtre entière : après un `geometry bas-droite angle=90`, la
    // maquette doit faire tourner les huit LED de ce ventilateur **sur place**. Un plan qui
    // déplacerait les centres ferait sauter le boîtier à chaque correction, et un plan qui
    // ignorerait les angles ne corrigerait jamais rien — les deux fautes sont vérifiées ici.
    let mesuree = Geometrie::mesuree();
    let reorientee = geometrie_reorientee();
    assert_ne!(
        mesuree, reorientee,
        "les deux géométries doivent bien différer, sinon ce test ne prouve rien"
    );

    let a = Plan::nouveau(&mesuree);
    let b = Plan::nouveau(&reorientee);

    assert_eq!(
        a.rayon_anneau(),
        b.rayon_anneau(),
        "le rayon d'un anneau ne dépend pas de l'endroit où il commence"
    );
    for position in Position::ALL {
        assert_eq!(
            a.centre_ventilateur(position),
            b.centre_ventilateur(position),
            "{} n'a pas bougé : seul son anneau a tourné",
            position.slug()
        );
    }
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            assert_eq!(
                a.led_barrette(slot, led),
                b.led_barrette(slot, led),
                "une barrette n'a pas d'anneau : la barrette {slot} LED {led} n'a pas bougé"
            );
        }
    }

    // Et les LED, elles, tournent — c'est le but. Un plan qui rendrait les mêmes places pour deux
    // orientations différentes afficherait un boîtier faux qu'aucune commande ne pourrait
    // corriger.
    let mut deplacees = 0usize;
    for position in Position::ALL {
        for led in 0..LEDS_PER_FAN as usize {
            if a.led_ventilateur(position, led) != b.led_ventilateur(position, led) {
                deplacees += 1;
            }
        }
    }
    assert!(
        deplacees > 0,
        "changer les dix orientations doit déplacer des LED : le plan lit bien la géométrie"
    );
}
