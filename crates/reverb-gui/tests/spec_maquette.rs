//! Tests d'intention du détail « ventilateur », de la vue isométrique et de la sélection au
//! rectangle (issue #28).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #28 et son commentaire — qui corrige le
//! troisième test d'intention après la fusion de #27 — et depuis le contrat déposé dans
//! `crates/reverb-gui/src/plan.rs`, dont `isometrique`, `centre_barrette`, `vue`, `groupe` et
//! `dans` sont tous `todo!("issue #28")` à l'écriture de ce fichier.
//!
//! `spec_plan.rs` tient déjà la vue de face seule (issue #23). Ce fichier ne la refait pas : il
//! tient ce que **deux** vues et **deux** granularités ajoutent.
//!
//! ## Les deux vues ne mentent pas de la même façon, et c'est le sujet
//!
//! La vue de face est une **mise en page** : sept ventilateurs sur dix y sont vus par la tranche
//! et dessinés en cercles quand même, et la RAM y est décalée pour rester cliquable. Elle écrase
//! la **largeur** du boîtier — l'axe qui va de la vitre au plateau de carte mère — et le
//! commentaire de l'issue nomme les trois familles qui y vivent : les six couchés et celui de
//! l'arrière au milieu, la RAM entre les deux, les trois du radiateur contre le plateau.
//!
//! L'isométrique est une **projection** : elle place les positions réelles des cent vingt-quatre
//! LED, sans en replacer une seule. Deux conséquences que ces tests exploitent, et une qu'ils
//! s'interdisent :
//!
//! - elle *lit* la largeur, donc elle sépare ce que la face confond ;
//! - aucun ventilateur n'y est vu par la tranche, donc chaque anneau y garde une étendue dans les
//!   deux directions de l'écran ;
//! - mais, n'ayant droit à aucun décalage, elle **peut** laisser une barrette tomber sur un
//!   anneau. Rien ici n'exige de l'isométrique la non-superposition que `spec_plan.rs` exige de la
//!   face : ce serait exiger d'une projection honnête qu'elle mente comme l'autre.
//!
//! ## Le critère n° 3, et pourquoi il se construit au lieu de se cueillir
//!
//! Le commentaire de l'issue le formule ainsi : « deux LED qui ne diffèrent que par la largeur
//! tombent au même endroit en vue de face, et à des endroits distincts en isométrique ».
//!
//! Une telle paire **n'existe pas** dans la géométrie mesurée : aucune des cent vingt-quatre LED
//! n'en a une autre à la même hauteur et à la même profondeur qu'elle. La paire ne se cueille donc
//! pas, elle se construit — et sans rien connaître de la projection choisie :
//!
//! 1. sept ventilateurs partagent une largeur (celle du milieu). Trois d'entre eux, non alignés,
//!    déterminent la **lecture** hauteur/profondeur → écran de la vue, quelle qu'elle soit ;
//! 2. cette lecture prédit correctement les quatre autres, **dans les deux vues** : c'est le
//!    témoin qui prouve qu'on a bien lu, et non deviné ;
//! 3. appliquée à un ventilateur du radiateur — qui ne diffère d'eux que par la largeur —, elle
//!    tombe **pile** en vue de face, et **à côté** en isométrique. L'écart est donc imputable à la
//!    seule largeur, ce qui est exactement l'énoncé du critère.
//!
//! ## Le point que le contrat laisse ouvert, et que ces tests tranchent
//!
//! **L'isométrique est une projection parallèle.** Le contrat dit « projette les positions réelles
//! […] aucune n'est replacée à la main », « les anneaux y sont des ellipses », et le mot
//! *isométrique* désigne une projection parallèle. Ces trois énoncés réunis disent qu'il existe une
//! application **affine** des millimètres du boîtier vers l'écran, et c'est ce que le test n° 4
//! vérifie. C'est aussi la seule formulation qui donne un sens mesurable à « elle lit la largeur » :
//! la colonne « largeur » de cette application n'est pas nulle. Une projection en perspective
//! échouerait ici — elle ne mérite pas le nom d'isométrique, et elle ferait dépendre la taille d'une
//! LED de sa distance, ce qu'aucune ligne du contrat n'annonce.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **La conservation de la sélection au changement de vue** : elle vit dans la fenêtre, pas dans
//!   le plan. Ce qui en dépend et qui *est* du ressort du plan est vérifié : `Cible` ne porte
//!   aucune vue, et une cible attrapée dans l'une reste une cible que l'autre sait placer.
//! - **`Ctrl` qui ajoute, le cerclage des cibles retenues, la couleur moyenne d'un disque, le
//!   nombre d'objets cliquables affichés** : tout cela est du dessin et de l'état de fenêtre.
//! - **L'échelle uniforme de l'isométrique.** Une normalisation qui étirerait un axe donnerait une
//!   autre application affine — c'est-à-dire une autre projection parallèle, également légitime.
//!   Le contrat ne fixant pas l'angle de vue, aucun test en boîte noire ne peut nommer cette faute.
//!   Ce qu'il peut exiger, et qu'il exige, c'est que l'ellipse d'un anneau ne soit pas aplatie.
//! - Rien ici n'ouvre de fenêtre ni ne parle à un socket : le plan est du calcul pur.

use reverb_anim::Geometrie;
use reverb_gui::plan::{Cible, Place, Plan, Vue};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

const LEDS: usize = LEDS_PER_FAN as usize;

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

/// Les deux vues, avec de quoi les nommer dans un message d'échec.
fn vues() -> [(&'static str, Plan); 2] {
    let geometrie = Geometrie::mesuree();
    [
        ("vue de face", Plan::nouveau(&geometrie)),
        ("vue isométrique", Plan::isometrique(&geometrie)),
    ]
}

fn distance(a: Place, b: Place) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Le diamètre d'une LED dessinée, dans l'unité de [`Place`].
///
/// Même déduction que `spec_plan.rs` : le contrat n'expose pas de taille de LED, et le quart du
/// rayon d'anneau est celle qui se déduit du reste. C'est l'unité dans laquelle « au même endroit »
/// et « à des endroits distincts » ont un sens à l'écran : deux choses plus proches que ça ne se
/// visent pas séparément à la souris.
fn diametre_led(plan: &Plan) -> f32 {
    plan.rayon_anneau() / 4.0
}

/// Les cent vingt-quatre cibles du boîtier.
fn toutes_les_cibles() -> Vec<Cible> {
    let mut cibles = Vec::new();
    for position in Position::ALL {
        for led in 0..LEDS {
            cibles.push(Cible::Led { position, led });
        }
    }
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            cibles.push(Cible::Barrette { slot, led });
        }
    }
    assert_eq!(
        cibles.len(),
        Position::ALL.len() * LEDS + SLOT_COUNT * LEDS_PER_STICK,
        "le boîtier a cent vingt-quatre LED"
    );
    cibles
}

fn nom(cible: Cible) -> String {
    match cible {
        Cible::Led { position, led } => format!("{} LED {led}", position.slug()),
        Cible::Barrette { slot, led } => format!("barrette {slot} LED {led}"),
    }
}

fn rang(position: Position) -> usize {
    Position::ALL
        .iter()
        .position(|p| *p == position)
        .expect("toute position est dans Position::ALL")
}

/// Une clef d'ordre stable, propre à ces tests : elle sert à comparer des **ensembles** de cibles
/// sans rien présumer de l'ordre dans lequel le plan les rend.
fn clef(cible: Cible) -> (usize, usize, usize) {
    match cible {
        Cible::Led { position, led } => (0, rang(position), led),
        Cible::Barrette { slot, led } => (1, slot, led),
    }
}

fn trie(mut cibles: Vec<Cible>) -> Vec<Cible> {
    cibles.sort_by_key(|c| clef(*c));
    cibles
}

fn noms(cibles: &[Cible]) -> Vec<String> {
    cibles.iter().map(|c| nom(*c)).collect()
}

/// Où le plan place une cible. Une cible est **toujours une LED** — c'est le contrat.
fn place(plan: &Plan, cible: Cible) -> Place {
    match cible {
        Cible::Led { position, led } => plan.led_ventilateur(position, led),
        Cible::Barrette { slot, led } => plan.led_barrette(slot, led),
    }
    .unwrap_or_else(|| panic!("{} doit avoir une place", nom(cible)))
}

fn places(plan: &Plan) -> Vec<(Cible, Place)> {
    toutes_les_cibles()
        .into_iter()
        .map(|c| (c, place(plan, c)))
        .collect()
}

/// Les huit LED d'un ventilateur, comme cibles.
fn anneau(position: Position) -> Vec<Cible> {
    (0..LEDS).map(|led| Cible::Led { position, led }).collect()
}

/// Les onze LED d'une barrette, comme cibles.
fn reglette(slot: usize) -> Vec<Cible> {
    (0..LEDS_PER_STICK)
        .map(|led| Cible::Barrette { slot, led })
        .collect()
}

/// Une lecture affine « hauteur, profondeur → écran », déduite de trois ventilateurs.
///
/// Sert au critère n° 3 : c'est le seul moyen de construire la LED qui « ne diffère que par la
/// largeur » d'une autre, puisque la géométrie mesurée n'en fournit aucune paire.
fn lecture_affine(echantillon: [(f32, f32, Place); 3]) -> impl Fn(f32, f32) -> Place {
    let [(y0, z0, p0), (y1, z1, p1), (y2, z2, p2)] = echantillon;
    let det = (y1 - y0) * (z2 - z0) - (y2 - y0) * (z1 - z0);
    assert!(
        det.abs() > 1.0,
        "les trois ventilateurs témoins doivent être franchement non alignés en \
         (hauteur, profondeur), sinon la lecture est arbitraire : déterminant {det}"
    );
    let coefficients = move |v0: f32, v1: f32, v2: f32| {
        let a = ((v1 - v0) * (z2 - z0) - (v2 - v0) * (z1 - z0)) / det;
        let b = ((y1 - y0) * (v2 - v0) - (y2 - y0) * (v1 - v0)) / det;
        (a, b, v0 - a * y0 - b * z0)
    };
    let (ax, bx, cx) = coefficients(p0.x, p1.x, p2.x);
    let (ay, by, cy) = coefficients(p0.y, p1.y, p2.y);
    move |y, z| Place {
        x: ax * y + bx * z + cx,
        y: ay * y + by * z + cy,
    }
}

/// Résout `M · inconnues = second_membre` par pivot de Gauss, pour un système 4 × 4.
fn resoudre(mut m: [[f64; 5]; 4]) -> [f64; 4] {
    for colonne in 0..4 {
        let pivot = (colonne..4)
            .max_by(|a, b| m[*a][colonne].abs().total_cmp(&m[*b][colonne].abs()))
            .expect("quatre lignes");
        m.swap(colonne, pivot);
        assert!(
            m[colonne][colonne].abs() > 1e-6,
            "les quatre LED témoins doivent être franchement non coplanaires"
        );
        let ligne_pivot = m[colonne];
        for (ligne, valeurs) in m.iter_mut().enumerate() {
            if ligne == colonne {
                continue;
            }
            let facteur = valeurs[colonne] / ligne_pivot[colonne];
            for (k, valeur) in valeurs.iter_mut().enumerate().skip(colonne) {
                *valeur -= facteur * ligne_pivot[k];
            }
        }
    }
    [
        m[0][4] / m[0][0],
        m[1][4] / m[1][1],
        m[2][4] / m[2][2],
        m[3][4] / m[3][3],
    ]
}

/// Huit coupes de l'écran, à pas irrégulier et débordant le cadre des deux côtés.
///
/// Irrégulier exprès : un pas rond ferait tomber un bord de rectangle sur un centre de LED, et
/// c'est justement le cas que ces tests n'ont pas le droit de trancher — le contrat ne dit pas si
/// un centre pile sur le bord entre ou non.
const COUPES: [f32; 8] = [
    -0.0700, 0.0113, 0.1637, 0.3121, 0.4703, 0.6229, 0.8117, 1.0413,
];

fn rectangles() -> Vec<(Place, Place)> {
    let mut liste = Vec::new();
    for (i, x0) in COUPES.iter().enumerate() {
        for x1 in COUPES.iter().skip(i + 1) {
            for (j, y0) in COUPES.iter().enumerate() {
                for y1 in COUPES.iter().skip(j + 1) {
                    liste.push((Place { x: *x0, y: *y0 }, Place { x: *x1, y: *y1 }));
                }
            }
        }
    }
    liste
}

/// Le cadre entier et au-delà : tout ce que la maquette porte.
fn tout_le_cadre() -> (Place, Place) {
    (Place { x: -1.0, y: -1.0 }, Place { x: 2.0, y: 2.0 })
}

// ---------------------------------------------------------------------------
// 1 — chaque plan dit d'où il regarde, et les deux ne regardent pas d'où
// ---------------------------------------------------------------------------

#[test]
fn chaque_plan_dit_de_quel_point_de_vue_il_vient_et_les_deux_different() {
    // Contrat — `vue()` : « depuis quel point de vue ce plan a été construit ». C'est ce que la
    // bascule `[ face | isométrique ]` lit pour savoir laquelle des deux est cochée.
    //
    // Une `vue()` qui mentirait ne casserait rien de visible : la maquette s'afficherait, et
    // seule la bascule serait à l'envers. Et deux vues qui rendraient le même plan feraient une
    // bascule sans effet — la panne la plus décevante possible pour une fonctionnalité dont
    // l'unique raison d'être est de montrer autre chose.
    let geometrie = Geometrie::mesuree();
    let face = Plan::nouveau(&geometrie);
    let isometrique = Plan::isometrique(&geometrie);

    assert_eq!(
        face.vue(),
        Vue::Face,
        "`Plan::nouveau` est la projection de face"
    );
    assert_eq!(
        isometrique.vue(),
        Vue::Isometrique,
        "`Plan::isometrique` est la vue de trois-quarts"
    );

    let deplacees = toutes_les_cibles()
        .into_iter()
        .filter(|c| place(&face, *c) != place(&isometrique, *c))
        .count();
    assert!(
        deplacees > 0,
        "basculer de vue doit déplacer des LED, sinon la bascule ne montre rien de nouveau"
    );
}

// ---------------------------------------------------------------------------
// 2 — les deux granularités montrent le même boîtier
// ---------------------------------------------------------------------------

#[test]
fn les_organes_pavent_les_cent_vingt_quatre_led_sans_trou_ni_doublon() {
    // Test d'intention n° 1 : « les deux niveaux de détail rendent le même cadre, à des
    // granularités différentes ». Critère d'acceptation : « la bascule ventilateur/LED change le
    // nombre d'objets cliquables (14 contre 124) sans changer leur disposition ».
    //
    // « Le même cadre » veut dire : les quatorze organes sont exactement un pavage des cent
    // vingt-quatre LED. Un `groupe` qui rendrait la cible seule ferait du détail « ventilateur » un
    // synonyme du détail « LED » — la bascule existerait et ne ferait rien. Un `groupe` qui
    // déborderait ferait allumer un ventilateur voisin sur un clic, et personne ne saurait
    // pourquoi.
    for (nom_vue, plan) in vues() {
        let mut vus: Vec<Cible> = Vec::new();
        let mut organes = 0usize;

        for position in Position::ALL {
            let attendu = trie(anneau(position));
            for cible in &attendu {
                let obtenu = trie(plan.groupe(*cible));
                assert_eq!(
                    noms(&obtenu),
                    noms(&attendu),
                    "en {nom_vue}, le groupe de {} est le ventilateur entier, ses huit LED et rien \
                     d'autre",
                    nom(*cible)
                );
            }
            organes += 1;
            vus.extend(attendu);
        }

        for slot in 0..SLOT_COUNT {
            let attendu = trie(reglette(slot));
            for cible in &attendu {
                let obtenu = trie(plan.groupe(*cible));
                assert_eq!(
                    noms(&obtenu),
                    noms(&attendu),
                    "en {nom_vue}, le groupe de {} est la barrette entière, ses onze LED et rien \
                     d'autre",
                    nom(*cible)
                );
            }
            organes += 1;
            vus.extend(attendu);
        }

        assert_eq!(
            organes,
            Position::ALL.len() + SLOT_COUNT,
            "dix ventilateurs et quatre réglettes font les quatorze objets du détail « \
             ventilateur » — {nom_vue}"
        );
        assert_eq!(
            noms(&trie(vus)),
            noms(&trie(toutes_les_cibles())),
            "l'union des quatorze organes est exactement les cent vingt-quatre LED, sans trou ni \
             doublon — {nom_vue}"
        );
    }
}

#[test]
fn le_groupe_d_une_cible_est_stable_et_ne_depend_pas_de_la_vue() {
    // Corollaire de « toujours une LED, quel que soit le niveau de détail affiché » : le groupe
    // est une propriété de l'**organe**, pas du geste ni du point de vue.
    //
    // Deux fautes que cela attrape, et qu'aucun test de contenu n'attrape :
    //
    // - un `groupe` non stable — `groupe(groupe(c)[0])` qui ne redonne pas le même organe. Le clic
    //   suivant sur une cible déjà sélectionnée changerait alors la sélection, et l'utilisateur
    //   verrait son choix « glisser » sans avoir rien fait de différent.
    // - un `groupe` qui dépendrait de la vue. La sélection est conservée d'une vue à l'autre ; si
    //   le groupe changeait en même temps, la bascule ajouterait ou retirerait des LED en douce.
    let [(_, face), (_, isometrique)] = vues();

    for cible in toutes_les_cibles() {
        let attendu = trie(face.groupe(cible));
        assert!(
            attendu.contains(&cible),
            "le groupe de {} doit contenir {} : c'est l'organe qui la porte",
            nom(cible),
            nom(cible)
        );
        assert_eq!(
            trie(isometrique.groupe(cible)),
            attendu,
            "le groupe de {} ne dépend pas de la vue",
            nom(cible)
        );
        for membre in &attendu {
            assert_eq!(
                noms(&trie(face.groupe(*membre))),
                noms(&attendu),
                "le groupe est stable : partir de {} au lieu de {} doit rendre le même organe",
                nom(*membre),
                nom(cible)
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — viser un point du disque désigne le ventilateur, et lui seul
// ---------------------------------------------------------------------------

#[test]
fn viser_un_point_du_disque_d_un_ventilateur_designe_ce_ventilateur_et_aucun_autre() {
    // Test d'intention n° 2 : « viser un point du disque d'un ventilateur désigne le ventilateur :
    // `sous` puis `groupe` rend les huit LED de ce ventilateur et d'aucun autre ». Critère
    // d'acceptation : « en vue ventilateur, cliquer un disque vise les huit LED du ventilateur ».
    //
    // C'est la chaîne complète du clic au détail « ventilateur » : `sous` trouve une LED, `groupe`
    // l'étend à son organe. Les deux moitiés peuvent être justes séparément et fausses ensemble —
    // un `sous` qui se rabat sur la LED d'à côté ne se voit pas tant qu'on clique une LED, et se
    // voit tout de suite quand le clic allume le mauvais ventilateur entier.
    //
    // **En vue de face seulement.** C'est là que `spec_plan.rs` garantit qu'aucun organe n'en
    // recouvre un autre, donc que « le point du disque » désigne sans ambiguïté. L'isométrique n'a
    // pas droit à ce décalage : elle projette, et une barrette peut y tomber sur un anneau. Exiger
    // d'elle le même résultat serait lui demander de mentir comme la face.
    let (_, plan) = &vues()[0];
    let rayon = plan.rayon_anneau();

    for position in Position::ALL {
        let centre = plan.centre_ventilateur(position);
        let attendu = trie(anneau(position));

        // Des points du disque, jamais son centre exact : le centre est à un rayon pile de chacune
        // des huit LED, et le contrat ne dit pas si la prise est ouverte ou fermée sur son bord.
        // Une main ne clique pas au micron près, ces tests non plus.
        for pas in [0.2f32, 0.45, 0.6] {
            for huitieme in 0..8u32 {
                let angle = huitieme as f32 * std::f32::consts::FRAC_PI_4;
                let vise = Place {
                    x: centre.x + rayon * pas * angle.cos(),
                    y: centre.y + rayon * pas * angle.sin(),
                };
                let touchee = plan.sous(vise).unwrap_or_else(|| {
                    panic!(
                        "un point du disque de {} ({vise:?}, à {pas} rayon de {centre:?}) doit \
                         toucher une LED",
                        position.slug()
                    )
                });
                assert_eq!(
                    noms(&trie(plan.groupe(touchee))),
                    noms(&attendu),
                    "viser {vise:?} — sur le disque de {} — désigne les huit LED de {} et d'aucun \
                     autre ventilateur ; on a touché {}",
                    position.slug(),
                    position.slug(),
                    nom(touchee)
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — la largeur : écrasée de face, lue en isométrique
// ---------------------------------------------------------------------------

#[test]
fn la_largeur_que_la_vue_de_face_confond_ecarte_les_organes_en_isometrique() {
    // Test d'intention n° 3, **tel que le commentaire de l'issue le corrige** : « deux LED qui ne
    // diffèrent que par la largeur tombent au même endroit en vue de face, et à des endroits
    // distincts en isométrique ».
    //
    // Voir l'en-tête : la géométrie mesurée n'offre aucune paire de ce genre, la paire se
    // construit. Sept ventilateurs partagent une largeur ; trois d'entre eux donnent la lecture
    // hauteur/profondeur → écran de la vue ; les quatre autres la valident **dans les deux vues** ;
    // le radiateur, qui n'en diffère que par la largeur, la dément en isométrique et la confirme de
    // face.
    //
    // Sans le témoin, ce test ne prouverait rien : n'importe quelle vue tordue écarterait le
    // radiateur de n'importe quelle prédiction. C'est le fait que la même lecture tombe juste sur
    // quatre ventilateurs de la même largeur, et faux sur le seul d'une autre, qui impute l'écart à
    // la largeur et à rien d'autre.
    let geometrie = Geometrie::mesuree();
    let familles = familles_de_largeur(&geometrie);
    assert!(
        familles.len() >= 2,
        "le boîtier a plusieurs largeurs occupées, sinon le critère n'a pas d'objet : {familles:?}"
    );

    let (largeur_commune, temoins) = familles
        .iter()
        .max_by_key(|(_, membres)| membres.len())
        .expect("au moins une famille");
    assert!(
        temoins.len() >= 4,
        "il faut trois ventilateurs pour lire une vue et au moins un pour la valider : {temoins:?}"
    );

    // Les trois qui déterminent la lecture : le triplet le moins aligné de la famille.
    let base = triplet_le_moins_aligne(&geometrie, temoins);
    let restants: Vec<Position> = temoins
        .iter()
        .copied()
        .filter(|p| !base.contains(p))
        .collect();
    let (autre_largeur, etrangers) = familles
        .iter()
        .find(|(l, _)| l != largeur_commune)
        .expect("une seconde famille de largeur");

    for (nom_vue, plan) in vues() {
        let tolerance = diametre_led(&plan);
        let lire = lecture_affine(base.map(|p| {
            let c = geometrie.centre_ventilateur(p);
            (c.y, c.z, plan.centre_ventilateur(p))
        }));

        // Le témoin : à largeur égale, la lecture doit tomber juste.
        for position in &restants {
            let c = geometrie.centre_ventilateur(*position);
            let (prevu, reel) = (lire(c.y, c.z), plan.centre_ventilateur(*position));
            let ecart = distance(prevu, reel);
            assert!(
                ecart <= tolerance,
                "en {nom_vue}, {} partage la largeur {largeur_commune} des trois témoins : la \
                 lecture hauteur/profondeur doit le placer, et elle le rate de {ecart} — prévu \
                 {prevu:?}, réel {reel:?}",
                position.slug()
            );
        }

        // Le critère : à hauteur et profondeur lues de la même façon, seule la largeur diffère.
        for position in etrangers {
            let c = geometrie.centre_ventilateur(*position);
            let (prevu, reel) = (lire(c.y, c.z), plan.centre_ventilateur(*position));
            let ecart = distance(prevu, reel);
            match plan.vue() {
                Vue::Face => assert!(
                    ecart <= tolerance,
                    "la vue de face écrase la largeur : {} est à {autre_largeur} au lieu de \
                     {largeur_commune}, et tombe pourtant là où sa seule hauteur et sa seule \
                     profondeur le prédisent — écart {ecart}, toléré {tolerance}",
                    position.slug()
                ),
                Vue::Isometrique => assert!(
                    ecart > tolerance,
                    "l'isométrique lit la largeur : {} est à {autre_largeur} au lieu de \
                     {largeur_commune}, il doit s'écarter d'au moins un diamètre de LED de ce que \
                     hauteur et profondeur seules prédisent — écart {ecart}, minimum {tolerance}",
                    position.slug()
                ),
            }
        }
    }
}

/// Les ventilateurs regroupés par largeur, telle que la géométrie la mesure.
///
/// Le commentaire de l'issue en nomme trois : les six couchés et celui de l'arrière au milieu, la
/// RAM entre les deux, les trois du radiateur contre le plateau de carte mère. Ce sont des familles
/// que la vue de face confond, et que l'isométrique sépare.
fn familles_de_largeur(geometrie: &Geometrie) -> Vec<(i32, Vec<Position>)> {
    let mut familles: Vec<(i32, Vec<Position>)> = Vec::new();
    for position in Position::ALL {
        let largeur = geometrie.centre_ventilateur(position).x.round() as i32;
        match familles.iter_mut().find(|(l, _)| *l == largeur) {
            Some((_, membres)) => membres.push(position),
            None => familles.push((largeur, vec![position])),
        }
    }
    familles
}

/// Le triplet dont l'aire en (hauteur, profondeur) est la plus grande : celui qui donne la lecture
/// la mieux conditionnée, sans qu'aucune coordonnée ne soit écrite ici.
fn triplet_le_moins_aligne(geometrie: &Geometrie, famille: &[Position]) -> [Position; 3] {
    let mut meilleur: Option<(f32, [Position; 3])> = None;
    for (i, a) in famille.iter().enumerate() {
        for (j, b) in famille.iter().enumerate().skip(i + 1) {
            for c in famille.iter().skip(j + 1) {
                let (pa, pb, pc) = (
                    geometrie.centre_ventilateur(*a),
                    geometrie.centre_ventilateur(*b),
                    geometrie.centre_ventilateur(*c),
                );
                let aire = ((pb.y - pa.y) * (pc.z - pa.z) - (pc.y - pa.y) * (pb.z - pa.z)).abs();
                if meilleur.as_ref().is_none_or(|(pire, _)| aire > *pire) {
                    meilleur = Some((aire, [*a, *b, *c]));
                }
            }
        }
    }
    meilleur
        .expect("une famille de trois ventilateurs au moins")
        .1
}

#[test]
fn l_isometrique_projette_les_positions_reelles_sans_en_replacer_aucune() {
    // Contrat — `Plan::isometrique` : « contrairement à la vue de face, celle-ci **projette les
    // positions réelles** des cent vingt-quatre LED : aucune n'est replacée à la main » ; et
    // `Vue::Isometrique` : « les anneaux y sont des ellipses, et non des cercles ».
    //
    // Voir l'en-tête : ces deux phrases, plus le mot *isométrique*, disent qu'il existe une
    // application affine des millimètres du boîtier vers l'écran. Quatre LED non coplanaires la
    // déterminent ; les cent vingt en restent la vérifient.
    //
    // Ce que ce test attrape et qu'aucun autre n'attrape : une isométrique qui redessinerait les
    // anneaux en cercles « quand même », comme la face le fait. Elle serait jolie, non dégénérée,
    // elle séparerait même les largeurs — et elle mentirait exactement là où cette vue est censée
    // être la seule à ne pas mentir.
    let geometrie = Geometrie::mesuree();
    let plan = Plan::isometrique(&geometrie);
    let tolerance = diametre_led(&plan);

    let bases = [
        Cible::Led {
            position: Position::BasGauche,
            led: 0,
        },
        Cible::Led {
            position: Position::RadiateurBas,
            led: 0,
        },
        Cible::Led {
            position: Position::Arriere,
            led: 0,
        },
        Cible::Barrette { slot: 0, led: 0 },
    ];
    let mut systeme_x = [[0.0f64; 5]; 4];
    let mut systeme_y = [[0.0f64; 5]; 4];
    for (ligne, cible) in bases.iter().enumerate() {
        let p = point(&geometrie, *cible);
        let e = place(&plan, *cible);
        let coefficients = [p.0 as f64, p.1 as f64, p.2 as f64, 1.0];
        systeme_x[ligne][..4].copy_from_slice(&coefficients);
        systeme_y[ligne][..4].copy_from_slice(&coefficients);
        systeme_x[ligne][4] = e.x as f64;
        systeme_y[ligne][4] = e.y as f64;
    }
    let cx = resoudre(systeme_x);
    let cy = resoudre(systeme_y);
    let projeter = |p: (f32, f32, f32)| Place {
        x: (cx[0] * p.0 as f64 + cx[1] * p.1 as f64 + cx[2] * p.2 as f64 + cx[3]) as f32,
        y: (cy[0] * p.0 as f64 + cy[1] * p.1 as f64 + cy[2] * p.2 as f64 + cy[3]) as f32,
    };

    for (cible, reelle) in places(&plan) {
        let prevue = projeter(point(&geometrie, cible));
        let ecart = distance(prevue, reelle);
        assert!(
            ecart <= tolerance,
            "l'isométrique ne replace aucune LED à la main : {} devrait tomber en {prevue:?}, la \
             projection que quatre LED suffisent à déterminer, et tombe en {reelle:?} — écart \
             {ecart}, toléré {tolerance}",
            nom(cible)
        );
    }

    // Et la colonne « largeur » de cette projection n'est pas nulle : deux LED qui ne diffèrent que
    // par la largeur sont écartées à l'écran, proportionnellement à ce qui les sépare dans le
    // boîtier. On prend l'écart le plus **petit** entre deux familles de largeur — celui qui est le
    // plus difficile à rendre visible.
    let par_largeur = largeurs_occupees(&geometrie);
    let plus_petit_ecart = par_largeur
        .windows(2)
        .map(|paire| paire[1] - paire[0])
        .fold(f32::INFINITY, f32::min);
    assert!(
        plus_petit_ecart.is_finite() && plus_petit_ecart > 0.0,
        "le boîtier occupe plusieurs largeurs : {par_largeur:?}"
    );
    let par_millimetre = ((cx[0] * cx[0] + cy[0] * cy[0]).sqrt()) as f32;
    let separation = par_millimetre * plus_petit_ecart;
    assert!(
        separation >= tolerance,
        "deux LED qui ne diffèrent que par la largeur — {plus_petit_ecart} mm, le plus petit écart \
         entre deux des plans occupés — doivent se voir séparément en isométrique : elles ne le \
         sont que de {separation}, pour un diamètre de LED de {tolerance}"
    );
}

/// La position réelle d'une cible dans le boîtier, en millimètres.
fn point(geometrie: &Geometrie, cible: Cible) -> (f32, f32, f32) {
    let p = match cible {
        Cible::Led { position, led } => geometrie.led_ventilateur(position, led),
        Cible::Barrette { slot, led } => geometrie.led_barrette(slot, led),
    }
    .unwrap_or_else(|| panic!("{} est une LED du boîtier", nom(cible)));
    (p.x, p.y, p.z)
}

/// Les largeurs auxquelles vivent les organes du boîtier, triées : le milieu des ventilateurs, la
/// RAM, le plateau de carte mère.
fn largeurs_occupees(geometrie: &Geometrie) -> Vec<f32> {
    let mut largeurs: Vec<f32> = Vec::new();
    for position in Position::ALL {
        largeurs.push(geometrie.centre_ventilateur(position).x);
    }
    for slot in 0..SLOT_COUNT {
        largeurs.push(
            geometrie
                .led_barrette(slot, 0)
                .expect("une barrette montée")
                .x,
        );
    }
    largeurs.sort_by(f32::total_cmp);
    largeurs.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    largeurs
}

#[test]
fn aucun_ventilateur_n_est_vu_par_la_tranche_en_isometrique() {
    // Second gain que le commentaire de l'issue ajoute : « en vue de face, sept ventilateurs sur
    // dix sont vus par la tranche et dessinés en cercles par convention. En isométrique, aucun ne
    // l'est. » Contrat — `Vue::Isometrique` : « le prix est que les anneaux y sont des ellipses, et
    // non des cercles ».
    //
    // Une ellipse, oui ; un trait, non. C'est la faute que ce test existe pour interdire : une
    // projection qui regarderait le boîtier pile dans l'axe d'une des trois familles de plans
    // aplatirait ses anneaux à zéro. Rien ne planterait, la maquette s'afficherait, et trois ou
    // sept ventilateurs deviendraient impossibles à cliquer LED par LED — exactement ce que la vue
    // de face doit contourner et que celle-ci est censée n'avoir pas à contourner.
    let plan = Plan::isometrique(&Geometrie::mesuree());
    let minimum = diametre_led(&plan);

    for position in Position::ALL {
        let anneau: Vec<Place> = (0..LEDS)
            .map(|led| {
                plan.led_ventilateur(position, led)
                    .unwrap_or_else(|| panic!("{} LED {led} doit avoir une place", position.slug()))
            })
            .collect();
        let etendue = |f: fn(&Place) -> f32| {
            let haut = anneau.iter().map(f).fold(f32::NEG_INFINITY, f32::max);
            let bas = anneau.iter().map(f).fold(f32::INFINITY, f32::min);
            haut - bas
        };
        let (largeur, hauteur) = (etendue(|p| p.x), etendue(|p| p.y));
        let (petit, grand) = (largeur.min(hauteur), largeur.max(hauteur));

        assert!(
            petit >= minimum,
            "l'anneau de {} est vu par la tranche en isométrique : il s'étend de {largeur} en \
             abscisse et de {hauteur} en ordonnée, moins qu'un diamètre de LED ({minimum}) dans \
             l'une des deux",
            position.slug()
        );
        // Une ellipse est attendue, un ruban ne l'est pas. Un cinquième est large : une projection
        // isométrique franche donne un rapport voisin de 0,58, et même une axonométrie plus
        // écrasée reste bien au-dessus. En deçà, l'anneau est un trait épais.
        assert!(
            petit * 5.0 >= grand,
            "l'anneau de {} est aplati en isométrique : {petit} contre {grand}, soit un rapport de \
             {}. Une ellipse, oui ; un ruban, non",
            position.slug(),
            petit / grand
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — le rectangle retient les centres, et rien d'autre
// ---------------------------------------------------------------------------

#[test]
fn dans_retient_exactement_les_cibles_dont_le_centre_est_dans_le_rectangle() {
    // Test d'intention n° 4 : « `dans` retient exactement les cibles dont le centre est dans le
    // rectangle, dans les deux vues ». Contrat : « une cible est retenue si **son centre** est dans
    // le rectangle : un critère de recouvrement partiel attraperait des LED qu'on ne voit pas
    // dedans ».
    //
    // Les deux moitiés d'« exactement » comptent. Trop peu, et un geste large en oublie au milieu —
    // on refait le geste, on ne comprend pas. Trop, et on emporte les voisines du bord : #29
    // composerait alors des zones qui débordent, sans que rien ne le signale.
    //
    // Les rectangles balaient le cadre et le débordent des deux côtés. Ceux dont un bord frôle un
    // centre sont écartés : le contrat ne dit pas si un centre pile sur le bord entre, et un test
    // d'intention n'a pas à trancher ce que la spécification laisse ouvert.
    for (nom_vue, plan) in vues() {
        let inventaire = places(&plan);
        let frole = diametre_led(&plan) / 100.0;
        let (mut examines, mut interessants) = (0usize, 0usize);

        for (coin, oppose) in rectangles() {
            let (x0, x1) = (coin.x.min(oppose.x), coin.x.max(oppose.x));
            let (y0, y1) = (coin.y.min(oppose.y), coin.y.max(oppose.y));
            if inventaire.iter().any(|(_, p)| {
                [(p.x, x0), (p.x, x1), (p.y, y0), (p.y, y1)]
                    .iter()
                    .any(|(v, bord)| (v - bord).abs() < frole)
            }) {
                continue;
            }
            let attendu: Vec<Cible> = inventaire
                .iter()
                .filter(|(_, p)| x0 < p.x && p.x < x1 && y0 < p.y && p.y < y1)
                .map(|(c, _)| *c)
                .collect();
            let obtenu = plan.dans(coin, oppose);
            assert_eq!(
                noms(&trie(obtenu.clone())),
                noms(&trie(attendu.clone())),
                "en {nom_vue}, le rectangle {coin:?}–{oppose:?} retient les centres qui y sont, et \
                 ceux-là seuls"
            );
            assert_eq!(
                obtenu.len(),
                trie(obtenu.clone()).len(),
                "en {nom_vue}, le rectangle {coin:?}–{oppose:?} ne rend pas deux fois la même cible"
            );
            examines += 1;
            if !attendu.is_empty() && attendu.len() < inventaire.len() {
                interessants += 1;
            }
        }

        assert!(
            interessants > 50,
            "il faut des rectangles qui retiennent une partie du boîtier pour que ce test prouve \
             quelque chose — {interessants} sur {examines} en {nom_vue}"
        );

        // Et le geste qui couvre tout couvre bien tout : les cent vingt-quatre, pas une de moins.
        let (coin, oppose) = tout_le_cadre();
        assert_eq!(
            noms(&trie(plan.dans(coin, oppose))),
            noms(&trie(toutes_les_cibles())),
            "en {nom_vue}, un rectangle qui déborde la maquette de tous côtés la retient entière"
        );
    }
}

#[test]
fn dans_ignore_l_ordre_des_coins_et_rend_la_maquette_dans_son_ordre() {
    // Contrat — `dans` : « les deux coins sont donnés dans n'importe quel ordre — un glissement va
    // dans les quatre sens » et « l'ordre du résultat est celui de la maquette, pas celui du geste :
    // deux glissements qui couvrent la même zone rendent la même liste ».
    //
    // C'est la faute la plus facile à commettre et la plus pénible à vivre : un `dans` qui
    // supposerait le premier coin en haut à gauche rendrait une sélection vide dès qu'on glisse
    // vers le haut ou vers la gauche. Une fois sur deux, donc, et sans message.
    //
    // L'ordre, lui, se vérifie par emboîtement : la liste d'un rectangle est celle du cadre entier
    // filtrée. C'est ce que « l'ordre de la maquette » veut dire, et c'est vérifiable sans savoir
    // quel est cet ordre.
    for (nom_vue, plan) in vues() {
        let (coin, oppose) = tout_le_cadre();
        let maquette = plan.dans(coin, oppose);

        for (coin, oppose) in rectangles() {
            let reference = plan.dans(coin, oppose);
            let (hg, bd) = (
                Place {
                    x: coin.x.min(oppose.x),
                    y: coin.y.min(oppose.y),
                },
                Place {
                    x: coin.x.max(oppose.x),
                    y: coin.y.max(oppose.y),
                },
            );
            let (hd, bg) = (Place { x: bd.x, y: hg.y }, Place { x: hg.x, y: bd.y });
            for (depart, arrivee) in [(hg, bd), (bd, hg), (hd, bg), (bg, hd)] {
                assert_eq!(
                    noms(&plan.dans(depart, arrivee)),
                    noms(&reference),
                    "en {nom_vue}, glisser de {depart:?} vers {arrivee:?} couvre la même zone que \
                     {coin:?}–{oppose:?} : même liste, et dans le même ordre"
                );
            }

            let filtree: Vec<Cible> = maquette
                .iter()
                .copied()
                .filter(|c| reference.contains(c))
                .collect();
            assert_eq!(
                noms(&filtree),
                noms(&reference),
                "en {nom_vue}, le rectangle {coin:?}–{oppose:?} rend ses cibles dans l'ordre de la \
                 maquette, pas dans celui du geste"
            );
        }
    }
}

#[test]
fn un_rectangle_qui_ne_touche_rien_rend_une_liste_vide() {
    // Test d'intention n° 5 : « un rectangle qui ne touche rien rend une liste vide ». Critère
    // d'acceptation : « un rectangle qui ne touche rien vide la sélection au lieu de la conserver ».
    //
    // La fenêtre s'appuie là-dessus pour désélectionner : un clic simple dans le vide est un
    // rectangle de taille nulle. Un `dans` qui se rabattrait sur la cible la plus proche quand il
    // n'attrape rien rendrait la désélection impossible — il n'y aurait plus aucun geste pour
    // revenir à zéro.
    for (nom_vue, plan) in vues() {
        let inventaire = places(&plan);
        let cote = diametre_led(&plan);
        let mut vides = 0usize;

        let pas = 60;
        for i in 0..=pas {
            for j in 0..=pas {
                let centre = Place {
                    x: i as f32 / pas as f32,
                    y: j as f32 / pas as f32,
                };
                let plus_proche = inventaire
                    .iter()
                    .map(|(_, p)| distance(centre, *p))
                    .fold(f32::INFINITY, f32::min);
                if plus_proche <= 2.0 * cote {
                    continue;
                }
                let (coin, oppose) = (
                    Place {
                        x: centre.x - cote,
                        y: centre.y - cote,
                    },
                    Place {
                        x: centre.x + cote,
                        y: centre.y + cote,
                    },
                );
                assert_eq!(
                    noms(&plan.dans(coin, oppose)),
                    Vec::<String>::new(),
                    "en {nom_vue}, le rectangle {coin:?}–{oppose:?} ne contient aucun centre — le \
                     plus proche est à {plus_proche} — donc il ne retient rien"
                );
                // Et le clic simple, qui est le même geste réduit à un point.
                assert_eq!(
                    noms(&plan.dans(centre, centre)),
                    Vec::<String>::new(),
                    "en {nom_vue}, un clic dans le vide en {centre:?} ne retient rien"
                );
                vides += 1;
            }
        }
        assert!(
            vides > 0,
            "un boîtier laisse du vide entre ses organes, sinon ce test ne prouve rien — {nom_vue}"
        );

        // Et hors du cadre, il n'y a rien du tout.
        for (coin, oppose) in [
            (Place { x: -3.0, y: -3.0 }, Place { x: -1.5, y: -1.5 }),
            (Place { x: 1.5, y: 0.0 }, Place { x: 4.0, y: 1.0 }),
            (Place { x: 0.0, y: 2.0 }, Place { x: 1.0, y: 9.0 }),
        ] {
            assert_eq!(
                noms(&plan.dans(coin, oppose)),
                Vec::<String>::new(),
                "en {nom_vue}, {coin:?}–{oppose:?} est hors de la maquette"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 6 — les deux vues gardent le même haut et le même avant
// ---------------------------------------------------------------------------

#[test]
fn les_deux_projections_placent_le_plafond_en_haut_et_l_avant_a_droite() {
    // Test d'intention n° 6 : « les deux projections placent le plafond au-dessus du plancher, et
    // l'avant à droite de l'arrière ». `Place` documente que `y` va **vers le bas**.
    //
    // `spec_plan.rs` tient déjà ce repère pour la vue de face. Ce qui est neuf ici, c'est qu'une
    // seconde projection ne doit pas le retourner : une isométrique prise de l'autre côté du
    // boîtier serait parfaitement correcte en soi, s'afficherait sans une erreur, et ferait cliquer
    // le ventilateur symétrique de celui qu'on vise — une fois sur deux, celle où la bascule est
    // sur « isométrique ».
    //
    // Rien ici n'exige de l'isométrique que le radiateur tienne le milieu de la hauteur, comme
    // `spec_plan.rs` l'exige de la face : une vue de trois-quarts penche, et une colonne à l'avant
    // y monte plus haut qu'une rangée du fond. C'est le sens même de « les quatre plans occupés s'y
    // distinguent ».
    for (nom_vue, plan) in vues() {
        let toit = RANGEE_HAUTE.map(|p| plan.centre_ventilateur(p).y);
        let sol = RANGEE_BASSE.map(|p| plan.centre_ventilateur(p).y);
        let plus_bas_du_toit = toit.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let plus_haut_du_sol = sol.iter().copied().fold(f32::INFINITY, f32::min);
        assert!(
            plus_bas_du_toit < plus_haut_du_sol,
            "en {nom_vue}, les trois du plafond sont au-dessus des trois du plancher : {toit:?} \
             contre {sol:?}"
        );

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
                    "en {nom_vue}, {} est plus près de l'arrière que {}, donc plus à gauche — \
                     {a:?} contre {b:?}",
                    avant.slug(),
                    apres.slug()
                );
            }
        }

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
                "en {nom_vue}, {} est au-dessus de {} — {b:?} contre {a:?}",
                dessus.slug(),
                dessous.slug()
            );
        }

        let arriere = plan.centre_ventilateur(Position::Arriere);
        for position in Position::ALL {
            if position == Position::Arriere {
                continue;
            }
            let autre = plan.centre_ventilateur(position);
            assert!(
                arriere.x < autre.x,
                "en {nom_vue}, « arrière » est le ventilateur du fond : il reste à gauche de {} — \
                 {arriere:?} contre {autre:?}",
                position.slug()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 7 — tout tient dans le cadre, dans l'une comme dans l'autre
// ---------------------------------------------------------------------------

#[test]
fn aucune_led_ne_sort_du_cadre_dans_l_une_comme_dans_l_autre() {
    // Test d'intention n° 7 : « aucune LED ne sort du cadre `[0, 1]²`, dans l'une comme dans
    // l'autre ». Contrat — `Place` : « normalisées de 0 à 1 […] c'est la fenêtre qui les multiplie
    // par sa taille du moment ».
    //
    // Une place hors du cadre ne lève rien : elle se multiplie par la taille de la fenêtre comme
    // les autres, et la LED se dessine hors de la zone visible. C'est la faute typique d'une
    // seconde projection qu'on oublie de normaliser — la vue de face marche, on bascule, et la
    // moitié du boîtier a disparu.
    //
    // Et le contraire aussi : une maquette normalisée dans un coin du cadre serait dans les clous
    // et illisible. Elle doit occuper le cadre.
    for (nom_vue, plan) in vues() {
        let rayon = plan.rayon_anneau();
        assert!(
            rayon > 0.0 && rayon < 0.5,
            "en {nom_vue}, un rayon d'anneau doit être une longueur, et une qui tienne dans le \
             cadre : {rayon}"
        );

        let inventaire = places(&plan);
        for (cible, place) in &inventaire {
            assert!(
                place.x.is_finite() && place.y.is_finite(),
                "en {nom_vue}, {} doit avoir une place finie : {place:?}",
                nom(*cible)
            );
            assert!(
                (0.0..=1.0).contains(&place.x) && (0.0..=1.0).contains(&place.y),
                "en {nom_vue}, {} sort du cadre : {place:?}",
                nom(*cible)
            );
        }

        // Les quatorze centres du détail « ventilateur » aussi : ce sont eux qu'on clique à ce
        // niveau-là, et un disque hors cadre est un ventilateur qu'on ne peut plus viser.
        let mut centres: Vec<(String, Place)> = Position::ALL
            .into_iter()
            .map(|p| (p.slug().to_string(), plan.centre_ventilateur(p)))
            .collect();
        for slot in 0..SLOT_COUNT {
            centres.push((
                format!("barrette {slot}"),
                plan.centre_barrette(slot)
                    .unwrap_or_else(|| panic!("la barrette {slot} est montée")),
            ));
        }
        for (nom_organe, centre) in &centres {
            assert!(
                (0.0..=1.0).contains(&centre.x) && (0.0..=1.0).contains(&centre.y),
                "en {nom_vue}, le centre de {nom_organe} sort du cadre : {centre:?}"
            );
        }

        let etendue = |f: fn(&Place) -> f32| {
            let haut = inventaire
                .iter()
                .map(|(_, p)| f(p))
                .fold(f32::NEG_INFINITY, f32::max);
            let bas = inventaire
                .iter()
                .map(|(_, p)| f(p))
                .fold(f32::INFINITY, f32::min);
            haut - bas
        };
        let (largeur, hauteur) = (etendue(|p| p.x), etendue(|p| p.y));
        assert!(
            largeur > 0.5 && hauteur > 0.5,
            "en {nom_vue}, la maquette occupe son cadre au lieu de se tasser dans un coin : \
             {largeur} sur {hauteur}"
        );
    }
}

// ---------------------------------------------------------------------------
// 8 — la réglette a un milieu, et il n'y a que quatre barrettes
// ---------------------------------------------------------------------------

#[test]
fn centre_barrette_tient_le_milieu_de_sa_reglette_et_s_arrete_au_quatrieme_slot() {
    // Contrat — `centre_barrette` : « le milieu d'une barrette, `None` au-delà du quatrième slot.
    // Ce qu'il faut pour dessiner une réglette d'un trait, sans passer par ses onze LED, quand la
    // maquette est au détail « ventilateur » ».
    //
    // Deux fautes, deux symptômes. Un centre qui ne serait pas au milieu — la première LED, par
    // exemple — dessinerait la réglette décalée d'une demi-longueur : le trait ne serait plus sur
    // les LED qu'il représente, et le clic viserait à côté. Un `Some` au-delà du quatrième slot
    // ferait apparaître une cinquième barrette sur une carte mère qui en porte quatre.
    for (nom_vue, plan) in vues() {
        let mut centres = Vec::new();
        for slot in 0..SLOT_COUNT {
            let centre = plan
                .centre_barrette(slot)
                .unwrap_or_else(|| panic!("la barrette {slot} est montée — {nom_vue}"));
            let leds: Vec<Place> = (0..LEDS_PER_STICK)
                .map(|led| {
                    plan.led_barrette(slot, led)
                        .unwrap_or_else(|| panic!("barrette {slot} LED {led} — {nom_vue}"))
                })
                .collect();

            let (x0, x1) = (
                leds.iter().map(|p| p.x).fold(f32::INFINITY, f32::min),
                leds.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max),
            );
            let (y0, y1) = (
                leds.iter().map(|p| p.y).fold(f32::INFINITY, f32::min),
                leds.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max),
            );
            assert!(
                (x0..=x1).contains(&centre.x) && (y0..=y1).contains(&centre.y),
                "en {nom_vue}, le centre de la barrette {slot} ({centre:?}) est entre ses onze LED \
                 — de ({x0}, {y0}) à ({x1}, {y1})"
            );

            // « Le milieu » : à mi-chemin des deux extrémités, à un intervalle de LED près — la
            // réglette peut être dessinée de la première à la onzième ou déborder un peu, mais
            // pas commencer là où elle devrait être au centre.
            let premiere = leds[0];
            let derniere = leds[LEDS_PER_STICK - 1];
            let milieu = Place {
                x: (premiere.x + derniere.x) / 2.0,
                y: (premiere.y + derniere.y) / 2.0,
            };
            let intervalle = distance(premiere, derniere) / (LEDS_PER_STICK - 1) as f32;
            let ecart = distance(centre, milieu);
            assert!(
                ecart <= intervalle,
                "en {nom_vue}, le centre de la barrette {slot} est son milieu : {centre:?} contre \
                 {milieu:?}, soit {ecart} pour un intervalle de LED de {intervalle}"
            );
            centres.push((slot, centre));
        }

        for (i, (slot, centre)) in centres.iter().enumerate() {
            for (autre_slot, autre) in centres.iter().skip(i + 1) {
                assert_ne!(
                    centre, autre,
                    "en {nom_vue}, les barrettes {slot} et {autre_slot} ne sont pas au même endroit"
                );
            }
        }

        for slot in [SLOT_COUNT, SLOT_COUNT + 1, 42, usize::MAX] {
            assert_eq!(
                plan.centre_barrette(slot),
                None,
                "en {nom_vue}, il n'y a que {SLOT_COUNT} barrettes, pas de {slot}-ième"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 9 — une cible ne connaît pas la vue dans laquelle on l'a attrapée
// ---------------------------------------------------------------------------

#[test]
fn une_cible_attrapee_dans_une_vue_reste_valide_dans_l_autre() {
    // Critère d'acceptation : « la bascule face/isométrique conserve la sélection courante ». La
    // sélection vit dans la fenêtre, pas ici — mais la propriété du plan dont elle dépend, si.
    //
    // `Cible` ne porte aucune vue : c'est une LED du boîtier, pas un point de l'écran. Une
    // sélection est donc conservable **par construction** — à condition que ce que `dans` rend dans
    // une vue soit encore une cible que l'autre sait placer. Un `dans` qui rendrait des indices
    // propres à sa vue casserait la bascule sans qu'aucun test de contenu ne le voie.
    let [(nom_a, a), (nom_b, b)] = vues();
    let (coin, oppose) = tout_le_cadre();

    for (depuis, vers, plan_source, plan_cible) in [(nom_a, nom_b, &a, &b), (nom_b, nom_a, &b, &a)]
    {
        let attrapees = plan_source.dans(coin, oppose);
        assert_eq!(
            noms(&trie(attrapees.clone())),
            noms(&trie(toutes_les_cibles())),
            "en {depuis}, tout le cadre attrape les cent vingt-quatre LED"
        );
        for cible in attrapees {
            let place = match cible {
                Cible::Led { position, led } => plan_cible.led_ventilateur(position, led),
                Cible::Barrette { slot, led } => plan_cible.led_barrette(slot, led),
            };
            assert!(
                place.is_some(),
                "{} a été attrapée en {depuis} : elle doit rester plaçable en {vers}, sinon la \
                 bascule perd la sélection",
                nom(cible)
            );
        }
    }

    // Et une sélection prise dans une vue désigne le même matériel dans l'autre : les organes
    // qu'elle recouvre ne changent pas.
    let coin_partiel = Place { x: 0.0, y: 0.0 };
    let oppose_partiel = Place { x: 0.6, y: 0.6 };
    let choisies = a.dans(coin_partiel, oppose_partiel);
    assert!(
        !choisies.is_empty() && choisies.len() < 124,
        "il faut une sélection partielle pour que ce test prouve quelque chose : {} cibles",
        choisies.len()
    );
    for cible in choisies {
        assert_eq!(
            noms(&trie(a.groupe(cible))),
            noms(&trie(b.groupe(cible))),
            "{} désigne le même organe dans les deux vues",
            nom(cible)
        );
    }
}
