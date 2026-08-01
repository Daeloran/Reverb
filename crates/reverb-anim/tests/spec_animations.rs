//! Tests d'intention du catalogue d'animations (issue #19).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #19 et son commentaire « Contrat d'API »
//! seuls. Aucune ligne n'est relue depuis un corps de fonction : à l'écriture de ce fichier,
//! `crates/reverb-anim/src/` n'est qu'un squelette de signatures dont tous les corps sont
//! `todo!()`. Ils encodent ce que les animations doivent faire, pas ce que le code fait — si
//! l'un d'eux échoue après implémentation, c'est le code qu'on corrige.
//!
//! ## Pourquoi ces tests-là
//!
//! Une animation fausse ne lève jamais d'erreur : elle s'affiche. Le seul juge est l'œil, et
//! l'œil ne voit pas la différence entre « la couleur demandée est appliquée » et « une couleur
//! est appliquée », ni entre « l'onde monte » et « les LED s'allument dans l'ordre du bus ».
//! C'est exactement le défaut que l'issue vient corriger : la `vague` actuelle « traite les 124
//! LED comme une file d'attente numérotée, pas comme un volume ».
//!
//! Les tests portent donc sur ce que l'œil ne sait pas juger : la **pureté** du rendu, la
//! **traçabilité** des paramètres jusqu'à l'image, et surtout la **cohérence spatiale** — deux
//! LED déclarées à la même hauteur reçoivent la même couleur d'une onde verticale.
//!
//! ## La règle qui gouverne ce fichier : aucune coordonnée n'est écrite ici
//!
//! La table de géométrie n'est pas encore mesurée. Aucun test ne suppose donc où se trouve une
//! LED : les groupes de LED de même hauteur sont **demandés à la géométrie elle-même**, jamais
//! devinés. Un test qui écrirait « la LED 3 du radiateur haut et la LED 7 de l'arrière sont à la
//! même hauteur » spécifierait une mesure que personne n'a faite.
//!
//! ## Quatre points que le contrat laisse ouverts, et que ces tests tranchent
//!
//! 1. **Les clés de réglage sont `couleur`, `vitesse` et la direction** — et rien d'autre, parce
//!    que `reglages` rend un `Reglages` à trois champs : une clé acceptée qui n'aurait aucun
//!    champ où atterrir serait acceptée pour rien. ⚠️ Le contrat nomme le champ `direction`,
//!    l'issue écrit `animate onde sens=bas-haut` : les deux orthographes sont admises ici, faute
//!    de savoir laquelle est la bonne (voir le rapport de session).
//! 2. **La couleur s'écrit en six chiffres hexadécimaux minuscules, sans `#`** — la même écriture
//!    que le protocole IPC (`light all ff2080`, spec_ipc n° 1) et que le README.
//! 3. **Les six directions s'écrivent `bas-haut`, `haut-bas`, `avant-arriere`, `arriere-avant`,
//!    `horaire`, `antihoraire`** : l'issue écrit `sens=bas-haut`, la géométrie écrit `horaire`, et
//!    la règle des slugs (ASCII, sans accent, kebab-case) fait le reste.
//! 4. **`Direction::BasHaut` veut dire que le motif ne dépend que de la hauteur.** C'est la
//!    définition d'une onde plane, et c'est ce que l'issue exige : « une onde qui monte doit
//!    atteindre en même temps deux LED physiquement à la même hauteur ». Exigé ici de `vague`
//!    seule — c'est la seule famille que l'issue nomme, et une famille à venir peut légitimement
//!    n'être pas une onde plane.
//!
//! Aucun accès matériel. Le seul appel au système est la lecture de `/proc/self/fd`, qui sert
//! précisément à prouver que le crate n'ouvre rien.

use std::collections::BTreeMap;

use reverb_anim::{Animation, CATALOGUE, Direction, Geometrie, Image, Orientation, Reglages, Sens};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Les six directions, pour n'en oublier aucune : le domaine tient sur une main.
const DIRECTIONS: [Direction; 6] = [
    Direction::BasHaut,
    Direction::HautBas,
    Direction::AvantArriere,
    Direction::ArriereAvant,
    Direction::Horaire,
    Direction::Antihoraire,
];

/// Une couleur dont **les trois composantes diffèrent**, et diffèrent deux à deux.
///
/// Le projet mélange trois ordres de composantes (CLAUDE.md : ventilateurs en GRB, écran en BGR,
/// RAM en RGB) et une permutation ne produit aucun message — juste une mauvaise couleur. Avec
/// `ff2080`, le rouge domine, le bleu suit, le vert ferme la marche : cet **ordre** survit à
/// n'importe quel assombrissement, et c'est ce qui le rend vérifiable dans un dégradé.
const TEMOIN: Rgb = Rgb::new(0xff, 0x20, 0x80);

/// Un second témoin, de composantes permutées : le vert domine, le bleu suit, le rouge ferme.
///
/// Deux témoins valent mieux qu'un : avec un seul, une animation qui ignorerait la couleur pour
/// peindre toujours la même passerait dès que cette couleur-là aurait le bon ordre.
const TEMOIN_BIS: Rgb = Rgb::new(0x20, 0xff, 0x80);

/// Les clés qu'une animation peut déclarer accepter.
///
/// Le contrat fixe la liste par construction : `reglages` rend un `Reglages` à trois champs
/// (`couleur`, `vitesse`, `direction`), donc une clé hors de cette liste n'aurait nulle part où
/// atterrir. `sens` y figure parce que l'issue l'écrit dans son exemple de commande.
const CLES_CONNUES: [&str; 4] = ["couleur", "vitesse", "direction", "sens"];

/// Le mot qui écrit une direction sur le socket.
fn slug_direction(direction: Direction) -> &'static str {
    match direction {
        Direction::BasHaut => "bas-haut",
        Direction::HautBas => "haut-bas",
        Direction::AvantArriere => "avant-arriere",
        Direction::ArriereAvant => "arriere-avant",
        Direction::Horaire => "horaire",
        Direction::Antihoraire => "antihoraire",
    }
}

/// L'écriture d'une couleur sur le socket : six chiffres hexadécimaux minuscules.
fn hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

/// Une paire brute, telle qu'elle arrive du protocole.
fn paire(cle: &str, valeur: &str) -> (String, String) {
    (cle.to_owned(), valeur.to_owned())
}

/// Les animations du catalogue, ouvertes par leur nom.
fn catalogue() -> Vec<Animation> {
    CATALOGUE
        .iter()
        .map(|nom| {
            Animation::par_nom(nom)
                .unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
        })
        .collect()
}

/// Une géométrie de test, construite par le décodeur.
///
/// ⚠️ Volontairement pas `Geometrie::mesuree()` : ce qu'elle rendra n'est pas encore mesuré, et
/// aucun test ne doit dépendre d'une coordonnée. Passer par `decoder` fige les dix orientations,
/// donc rend le rendu reproductible, sans rien supposer des coordonnées elles-mêmes.
fn geometrie_de_test() -> Geometrie {
    let mut lignes = Vec::new();
    for position in Position::ALL {
        lignes.push(format!("{} 0 horaire", position.slug()));
    }
    Geometrie::decoder(&lignes.join("\n")).expect("la géométrie de test est valide")
}

/// Des réglages explicites, sur la base des valeurs par défaut.
fn reglages(couleur: Rgb, direction: Direction) -> Reglages {
    Reglages {
        couleur,
        direction,
        ..Reglages::default()
    }
}

/// Une LED du boîtier, désignée comme l'`Image` la désigne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Led {
    Ventilateur(Position, usize),
    Barrette(usize, usize),
}

impl Led {
    /// L'organe qui la porte : un ventilateur ou une barrette.
    ///
    /// Sert à exiger qu'un groupe de LED de même hauteur en traverse **deux** — l'issue :
    /// « quels que soient leur ventilateur et leur numéro d'ordre ».
    fn organe(self) -> (bool, usize) {
        match self {
            Led::Ventilateur(position, _) => (true, position.index()),
            Led::Barrette(slot, _) => (false, slot),
        }
    }

    fn nom(self) -> String {
        match self {
            Led::Ventilateur(position, led) => format!("{} led {led}", position.slug()),
            Led::Barrette(slot, led) => format!("barrette {slot} led {led}"),
        }
    }

    /// Sa hauteur dans le boîtier, telle que la géométrie la déclare.
    fn hauteur(self, geometrie: &Geometrie) -> f32 {
        let point = match self {
            Led::Ventilateur(position, led) => geometrie.led_ventilateur(position, led),
            Led::Barrette(slot, led) => geometrie.led_barrette(slot, led),
        };
        point
            .unwrap_or_else(|| panic!("{} n'a pas de place dans le boîtier", self.nom()))
            .y
    }

    /// Sa couleur dans une image.
    fn couleur(self, image: &Image) -> Rgb {
        match self {
            Led::Ventilateur(position, led) => couleurs_du_ventilateur(image, position)[led],
            Led::Barrette(slot, led) => image.barrettes[slot][led],
        }
    }
}

/// Les 124 LED du boîtier, dans un ordre stable.
fn toutes_les_led() -> Vec<Led> {
    let mut led = Vec::new();
    for position in Position::ALL {
        for indice in 0..LEDS_PER_FAN as usize {
            led.push(Led::Ventilateur(position, indice));
        }
    }
    for slot in 0..SLOT_COUNT {
        for indice in 0..LEDS_PER_STICK {
            led.push(Led::Barrette(slot, indice));
        }
    }
    led
}

/// Les huit couleurs d'un ventilateur dans une image, cherchées **par position**.
///
/// Jamais par indice de tableau : l'`Image` porte la position à côté des couleurs précisément
/// pour que personne n'ait à connaître l'ordre du tableau, et un test qui supposerait cet ordre
/// figerait un détail que le contrat ne fixe pas.
fn couleurs_du_ventilateur(image: &Image, position: Position) -> &[Rgb; LEDS_PER_FAN as usize] {
    let (_, couleurs) = image
        .ventilateurs
        .iter()
        .find(|(p, _)| *p == position)
        .unwrap_or_else(|| panic!("l'image ne contient pas {}", position.slug()));
    couleurs
}

/// Le nombre de descripteurs de fichier ouverts par le processus.
///
/// Rend 0 si `/proc` n'est pas monté — les deux mesures valent alors 0, et le test ne dit rien
/// plutôt que de mentir.
fn descripteurs_ouverts() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .map(std::iter::Iterator::count)
        .unwrap_or(0)
}

/// La clé de regroupement d'une hauteur : les bits du flottant, `-0.0` ramené à `0.0`.
///
/// L'égalité exacte est bien ce qu'on veut. Le test d'intention n° 9 dit « deux LED **déclarées**
/// à la même hauteur » : la déclaration, c'est la table, et deux hauteurs déclarées égales le sont
/// au bit près. Un regroupement à la tolérance dirait autre chose — que deux LED *voisines*
/// reçoivent la même couleur, ce qui est faux d'un dégradé continu.
fn cle_hauteur(y: f32) -> u32 {
    if y == 0.0 {
        0.0f32.to_bits()
    } else {
        y.to_bits()
    }
}

// ---------------------------------------------------------------------------
// 1 — une image de 124 couleurs, sans ouvrir un périphérique
// ---------------------------------------------------------------------------

#[test]
fn chaque_animation_rend_les_cent_vingt_quatre_couleurs_sans_rien_ouvrir() {
    // Tests d'intention n° 1 de l'issue — « Une animation rend une image de 124 couleurs, sans
    // qu'aucun périphérique soit ouvert », et critère d'acceptation : « Les animations restent
    // calculables hors du démon : un test rend une image sans ouvrir un seul périphérique ».
    //
    // C'est ce qui rend vraie la promesse « un aperçu qui change en même temps que mes
    // réglages » : la fenêtre affichera les images exactement calculées par le code qui écrit sur
    // le bus. Un crate qui lirait un fichier de configuration ou sonderait un `/dev` ne serait
    // plus calculable dans la fenêtre — et ne le serait plus dans ce test non plus, ce que le
    // décompte de descripteurs constate.
    let geometrie = geometrie_de_test();
    let reglages = reglages(TEMOIN, Direction::BasHaut);

    // Mesure à blanc d'abord : `read_dir` ouvre lui-même un descripteur, et c'est la **variation**
    // qui est observée, pas le nombre absolu.
    let _ = descripteurs_ouverts();
    let avant = descripteurs_ouverts();

    let mut images = Vec::new();
    for animation in catalogue() {
        for pas in [0u32, 1, 7, 30, 900] {
            images.push((animation.nom(), animation.image(&geometrie, &reglages, pas)));
        }
    }

    let apres = descripteurs_ouverts();
    assert_eq!(
        avant,
        apres,
        "rendre {} images a ouvert {} descripteur(s) : le crate n'est plus pur",
        images.len(),
        apres.saturating_sub(avant)
    );

    // Les dix ventilateurs, chacun une fois, désignés par leur position. Un tableau de dix
    // entrées dont deux porteraient la même position laisserait un ventilateur sans couleur, et
    // le tableau resterait de la bonne taille.
    for (nom, image) in &images {
        let mut positions: Vec<usize> = image.ventilateurs.iter().map(|(p, _)| p.index()).collect();
        positions.sort_unstable();
        positions.dedup();
        assert_eq!(
            positions.len(),
            Position::ALL.len(),
            "« {nom} » : les dix ventilateurs doivent être peints, chacun une fois"
        );
        for position in Position::ALL {
            assert!(
                image.ventilateurs.iter().any(|(p, _)| *p == position),
                "« {nom} » : {} n'est pas dans l'image",
                position.slug()
            );
        }
    }

    // Le compte des couleurs, recalculé depuis les constantes du protocole.
    let couleurs_par_image =
        Position::ALL.len() * LEDS_PER_FAN as usize + SLOT_COUNT * LEDS_PER_STICK;
    assert_eq!(couleurs_par_image, 124, "10 × 8 + 4 × 11");
    assert_eq!(toutes_les_led().len(), couleurs_par_image);

    // Et une image, ça s'allume. Une animation qui rendrait du noir à tous les pas passerait
    // tous les tests de structure de ce fichier sans éclairer une seule LED.
    for animation in catalogue() {
        let allumee = (0..64).any(|pas| {
            let image = animation.image(&geometrie, &reglages, pas);
            toutes_les_led()
                .iter()
                .any(|led| led.couleur(&image) != Rgb::BLACK)
        });
        assert!(
            allumee,
            "« {} » n'allume aucune LED en 64 pas — ce n'est pas une animation",
            animation.nom()
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — le rendu est une fonction du pas, pas un état
// ---------------------------------------------------------------------------

#[test]
fn deux_appels_au_meme_pas_rendent_la_meme_image() {
    // Test d'intention n° 2 de l'issue — « Deux appels au même pas rendent la même image — le
    // rendu est une fonction, pas un état », et contrat d'API : « `image` est pure : deux appels
    // identiques rendent la même image ».
    //
    // Deux raisons, et la seconde est celle qui coûte cher. Un rendu à état ne peut pas être
    // partagé entre le démon et la fenêtre : les deux avanceraient chacun de leur côté, et
    // l'aperçu montrerait autre chose que le boîtier. Et un rendu à état interdit le cache de
    // cibles inchangées du critère de cadence — comparer l'image du pas *n* à celle du pas *n-1*
    // suppose qu'on puisse les recalculer.
    let geometrie = geometrie_de_test();
    let reglages = reglages(TEMOIN, Direction::BasHaut);

    for animation in catalogue() {
        for pas in [0u32, 1, 2, 29, 30, 31, 1000, u32::MAX] {
            let premiere = animation.image(&geometrie, &reglages, pas);
            let seconde = animation.image(&geometrie, &reglages, pas);
            assert_eq!(
                premiere,
                seconde,
                "« {} » au pas {pas} : deux appels de suite doivent rendre la même image",
                animation.nom()
            );

            // Et depuis une autre instance, ouverte séparément : l'animation ne porte pas d'état
            // qui la ferait dériver d'un appelant à l'autre. C'est ce que la fenêtre fera —
            // ouvrir la sienne, sans rien partager avec celle du démon.
            let autre = Animation::par_nom(animation.nom()).expect("le nom vient du catalogue");
            assert_eq!(
                autre.image(&geometrie, &reglages, pas),
                premiere,
                "« {} » au pas {pas} : deux instances doivent rendre la même image",
                animation.nom()
            );
        }

        // L'ordre des appels n'a pas d'influence : rendre le pas 5 après le pas 900 doit donner
        // la même chose que de le rendre en premier. C'est le défaut exact d'une implémentation
        // qui avancerait un compteur interne à chaque appel.
        let cinq = animation.image(&geometrie, &reglages, 5);
        let _ = animation.image(&geometrie, &reglages, 900);
        assert_eq!(
            animation.image(&geometrie, &reglages, 5),
            cinq,
            "« {} » : le pas 5 ne dépend pas de ce qui a été rendu avant",
            animation.nom()
        );
    }

    // Une animation, ça bouge. Exigé ici de `vague` seule : c'est la seule famille que l'issue
    // nomme, et c'est une onde — si son image ne change jamais, il n'y a pas d'onde. Vingt
    // secondes de scrutation à 30 img/s, largement de quoi contenir une période.
    let vague = Animation::par_nom("vague").expect("« vague » est au catalogue");
    let premiere = vague.image(&geometrie, &reglages, 0);
    let bouge = (1..600).any(|pas| vague.image(&geometrie, &reglages, pas) != premiere);
    assert!(
        bouge,
        "« vague » rend la même image du pas 0 au pas 599 : ce n'est pas une onde, c'est une \
         couleur fixe"
    );
}

// ---------------------------------------------------------------------------
// 3 — un nom hors catalogue est refusé, et l'erreur cite les noms valides
// ---------------------------------------------------------------------------

#[test]
fn un_nom_hors_catalogue_est_refuse_en_citant_les_noms_valides() {
    // Test d'intention n° 3 de l'issue, et contrat d'API — « `AnimationInconnue` cite les noms du
    // `CATALOGUE` ».
    //
    // Citer les noms valides n'est pas une politesse : c'est la seule documentation que
    // l'utilisateur aura sous la main au moment où il en a besoin. Il vient de taper un nom au
    // hasard sur un socket qui n'a pas de complétion.

    // Le catalogue, d'abord. Critère d'acceptation : « Au moins cinq familles d'animations,
    // chacune paramétrable », et « `animate vague` (sans paramètre) continue de marcher ».
    assert!(
        CATALOGUE.len() >= 5,
        "au moins cinq familles, {} au catalogue : {CATALOGUE:?}",
        CATALOGUE.len()
    );
    assert!(
        CATALOGUE.contains(&"vague"),
        "« vague » doit rester au catalogue — le protocole s'étend, il ne casse pas : {CATALOGUE:?}"
    );

    let mut noms: Vec<&str> = CATALOGUE.to_vec();
    noms.sort_unstable();
    noms.dedup();
    assert_eq!(
        noms.len(),
        CATALOGUE.len(),
        "deux familles ne peuvent pas partager un nom : {CATALOGUE:?}"
    );

    for nom in CATALOGUE {
        // Un nom traverse le protocole comme un jeton séparé par des espaces : la même règle que
        // les slugs de position (spec_ipc n° 4), pour la même raison.
        assert!(!nom.is_empty(), "un nom vide n'est pas un nom");
        assert!(nom.is_ascii(), "« {nom} » n'est pas en ASCII");
        assert!(!nom.contains(' '), "« {nom} » contient une espace");
        assert_eq!(
            *nom,
            nom.to_lowercase(),
            "« {nom} » n'est pas en minuscules"
        );

        let animation = Animation::par_nom(nom).expect("un nom du catalogue s'ouvre");
        assert_eq!(
            animation.nom(),
            *nom,
            "l'animation ouverte par « {nom} » doit se nommer « {nom} »"
        );
    }

    // `off` n'est pas une animation : c'est l'absence d'animation, portée par `name: None` dans
    // le protocole. S'il figurait aussi au catalogue, deux chemins arrêteraient l'éclairage et
    // l'un des deux serait faux.
    assert!(
        !CATALOGUE.contains(&"off"),
        "« off » est l'absence d'animation, pas une animation : {CATALOGUE:?}"
    );

    for saisi in [
        "bidule",
        "vagues",
        "vagu",
        "VAGUE",
        "Vague",
        "",
        " ",
        "vague vague",
        "off",
        "--help",
        "0",
        "🌈",
    ] {
        let erreur = Animation::par_nom(saisi).expect_err("un nom hors catalogue est refusé");
        assert_eq!(
            erreur.saisi, saisi,
            "l'erreur doit porter le nom reçu, tel qu'il a été reçu"
        );
        assert_eq!(
            erreur.valides, CATALOGUE,
            "l'erreur doit citer le catalogue, et le catalogue en entier"
        );

        let message = erreur.to_string();
        for valide in CATALOGUE {
            assert!(
                message.contains(valide),
                "le message doit citer « {valide} » : « {message} »"
            );
        }
        if !saisi.trim().is_empty() {
            assert!(
                message.contains(saisi),
                "le message doit citer le nom refusé : « {message} »"
            );
        }
        let _: &dyn std::error::Error = &erreur;
    }
}

// ---------------------------------------------------------------------------
// 4 — un paramètre inconnu est refusé, en nommant la clé fautive
// ---------------------------------------------------------------------------

#[test]
fn un_parametre_inconnu_est_refuse_en_nommant_la_cle_fautive() {
    // Test d'intention n° 4 de l'issue, et contrat d'API — « une clé absente de
    // `parametres_acceptes` est refusée **en la nommant** », « Les clés que cette animation
    // accepte — la seule source de vérité du refus ».
    //
    // Approche technique de l'issue : « le refus d'un paramètre inconnu vient de cette
    // déclaration — pas d'un `match` par animation qu'on oublierait d'étendre ». Le test le
    // vérifie **des deux côtés** : tout ce qui est déclaré doit être accepté, et tout ce qui ne
    // l'est pas doit être refusé. Une déclaration qui mentirait dans un sens ou dans l'autre
    // ferait de `parametres_acceptes` une documentation, pas une source de vérité.
    let geometrie = geometrie_de_test();
    let mut au_moins_une_cle = false;

    for animation in catalogue() {
        let acceptees = animation.parametres_acceptes();
        assert!(
            !acceptees.is_empty(),
            "« {} » n'accepte aucun paramètre — critère d'acceptation : chacune paramétrable",
            animation.nom()
        );
        au_moins_une_cle = true;

        let mut vues: Vec<&str> = acceptees.to_vec();
        vues.sort_unstable();
        vues.dedup();
        assert_eq!(
            vues.len(),
            acceptees.len(),
            "« {} » déclare deux fois la même clé : {acceptees:?}",
            animation.nom()
        );

        for cle in acceptees {
            assert!(
                CLES_CONNUES.contains(cle),
                "« {} » accepte « {cle} », qui n'a aucun champ où atterrir dans `Reglages`",
                animation.nom()
            );

            // Ce qui est déclaré est accepté, pour de bon : la valeur passe, et le réglage rendu
            // reste utilisable pour rendre une image.
            let valeur = valeur_valide(cle);
            let reglages = animation
                .reglages(&[paire(cle, &valeur)])
                .unwrap_or_else(|erreur| {
                    panic!(
                        "« {} » déclare accepter « {cle} » mais refuse « {cle}={valeur} » : {erreur}",
                        animation.nom()
                    )
                });
            let _ = animation.image(&geometrie, &reglages, 0);

            // Et la valeur atterrit dans le champ qui porte son nom. Une clé acceptée qui
            // remplirait un autre champ serait un réglage qui ment : `vitesse=3` ne doit pas
            // ressortir en couleur, et rien dans le type ne l'empêche.
            match *cle {
                "couleur" => assert_eq!(
                    reglages.couleur,
                    TEMOIN,
                    "« {} » : `couleur={valeur}` doit atterrir dans le champ `couleur`",
                    animation.nom()
                ),
                "vitesse" => assert_eq!(
                    reglages.vitesse,
                    3,
                    "« {} » : `vitesse=3` doit atterrir dans le champ `vitesse`",
                    animation.nom()
                ),
                "direction" | "sens" => {
                    // Les six directions, exhaustivement : chaque mot désigne **sa** direction.
                    // C'est le défaut le plus silencieux du lot — une onde qui descend quand on a
                    // demandé qu'elle monte reste une belle animation, et personne ne va lire le
                    // code pour vérifier qu'un `match` n'a pas deux bras intervertis.
                    for direction in DIRECTIONS {
                        let slug = slug_direction(direction);
                        let reglages =
                            animation
                                .reglages(&[paire(cle, slug)])
                                .unwrap_or_else(|erreur| {
                                    panic!(
                                        "« {} » refuse « {cle}={slug} » : {erreur}",
                                        animation.nom()
                                    )
                                });
                        assert_eq!(
                            reglages.direction,
                            direction,
                            "« {} » : « {slug} » doit désigner {direction:?}",
                            animation.nom()
                        );
                    }
                }
                autre => unreachable!("« {autre} » n'est pas une clé connue de ce test"),
            }

            // Et le paramètre déclaré **fait** quelque chose. Critère d'acceptation : « Au moins
            // cinq familles d'animations, chacune paramétrable ». Une clé acceptée, validée,
            // rangée dans son champ, puis ignorée au rendu est le pire des trois défauts : elle
            // répond `end` sans rien changer, et on cherche ailleurs.
            for (une, autre) in valeurs_distinctes(cle) {
                let premier = animation
                    .reglages(&[paire(cle, une)])
                    .expect("valeur valide");
                let second = animation
                    .reglages(&[paire(cle, autre)])
                    .expect("valeur valide");
                assert!(
                    rendus_differents(&animation, &geometrie, &premier, &second),
                    "« {} » : `{cle}={une}` et `{cle}={autre}` peignent la même chose à tous les \
                     pas — le paramètre est déclaré mais n'entre pas dans le rendu",
                    animation.nom()
                );
            }
        }

        // Et ce qui n'est pas déclaré est refusé, en nommant la clé fautive. `angle` et `sens`
        // valent le détour : ce sont les clés du verbe `geometry`, celles qu'on tapera par
        // mégarde après avoir réglé une orientation.
        let mut inconnues = vec![
            "bidule", "color", "speed", "couleurs", "Couleur", "VITESSE", "", " ", "angle", "led",
            "position", "🌈", "couleur ",
        ];
        inconnues.retain(|cle| !acceptees.contains(cle));

        for cle in inconnues {
            let erreur = animation
                .reglages(&[paire(cle, "1")])
                .expect_err("une clé non déclarée est refusée");
            assert_eq!(
                erreur.cle,
                cle,
                "« {} » : le refus doit nommer la clé fautive",
                animation.nom()
            );
            assert_eq!(
                erreur.acceptees,
                acceptees,
                "« {} » : le refus doit citer les clés que l'animation accepte",
                animation.nom()
            );
            assert!(!erreur.raison.is_empty(), "le refus doit dire pourquoi");

            let message = erreur.to_string();
            if !cle.trim().is_empty() {
                assert!(
                    message.contains(cle),
                    "le message doit nommer la clé fautive : « {message} »"
                );
            }
            for acceptee in acceptees {
                assert!(
                    message.contains(acceptee),
                    "le message doit citer « {acceptee} », que l'animation accepte : « {message} »"
                );
            }
            let _: &dyn std::error::Error = &erreur;

            // Une clé inconnue **au milieu** de clés valides est refusée aussi, et c'est bien
            // elle qui est nommée. Un décodeur qui s'arrêterait à la première paire acceptable
            // appliquerait la moitié des réglages sans rien dire.
            let mut paires: Vec<(String, String)> = acceptees
                .iter()
                .map(|c| paire(c, &valeur_valide(c)))
                .collect();
            paires.insert(paires.len() / 2, paire(cle, "1"));
            let erreur = animation
                .reglages(&paires)
                .expect_err("une clé non déclarée reste refusée entourée de clés valides");
            assert_eq!(
                erreur.cle,
                cle,
                "« {} » : c'est « {cle} » qui est fautive, pas une autre",
                animation.nom()
            );
        }
    }

    assert!(
        au_moins_une_cle,
        "le catalogue est vide : il n'y a rien à paramétrer"
    );

    // Au moins une animation accepte une couleur : l'issue en donne l'exemple
    // (`animate comete couleur=ff00ff`), et le test n° 8 de ce fichier serait vide sans elle.
    assert!(
        catalogue()
            .iter()
            .any(|a| a.parametres_acceptes().contains(&"couleur")),
        "aucune animation n'accepte de couleur, alors que l'issue en montre une"
    );
}

// ---------------------------------------------------------------------------
// 5 — un paramètre hors bornes est refusé, jamais tronqué
// ---------------------------------------------------------------------------

#[test]
fn un_parametre_hors_bornes_est_refuse_jamais_tronque_en_silence() {
    // Test d'intention n° 5 de l'issue — « Un paramètre hors bornes est refusé, jamais tronqué en
    // silence », et critère d'acceptation : « Un paramètre inconnu ou hors bornes est **refusé
    // avec un message exploitable**, jamais appliqué de travers en silence ».
    //
    // C'est la même faute que `fan … pwm 250` du protocole (spec_ipc n° 3) : 250 tient dans un
    // `u8`, donc un décodeur qui se contente du type l'accepte. Ici, `vitesse=300` tronqué à 255
    // donnerait une animation qui tourne à une vitesse que personne n'a demandée, et
    // `couleur=ff00ff00` tronqué à `ff00ff` donnerait une couleur juste par accident.
    for animation in catalogue() {
        for cle in animation.parametres_acceptes() {
            for valeur in valeurs_invalides(cle) {
                let Err(erreur) = animation.reglages(&[paire(cle, valeur)]) else {
                    panic!(
                        "« {} » a accepté « {cle}={valeur} » — une valeur hors bornes doit être \
                         refusée, jamais ramenée en silence dans le domaine",
                        animation.nom()
                    );
                };
                assert_eq!(
                    erreur.cle,
                    *cle,
                    "« {} » : « {valeur} » est fautive au titre de « {cle} »",
                    animation.nom()
                );
                assert!(!erreur.raison.is_empty(), "le refus doit dire pourquoi");

                let message = erreur.to_string();
                assert!(
                    message.contains(cle),
                    "le message doit nommer la clé : « {message} »"
                );
                // La valeur refusée doit apparaître : c'est elle que l'utilisateur vient de
                // taper, et un message qui dit seulement « valeur invalide » n'aide pas à voir
                // qu'on a écrit `ff00ff00` au lieu de `ff00ff`.
                if !valeur.trim().is_empty() && valeur.len() < 32 {
                    assert!(
                        message.contains(valeur),
                        "le message doit dire la valeur refusée « {valeur} » : « {message} »"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 6 — sans paramètre, ce sont les valeurs par défaut
// ---------------------------------------------------------------------------

#[test]
fn sans_parametre_le_rendu_est_celui_des_valeurs_par_defaut_explicites() {
    // Test d'intention n° 6 de l'issue — « Une animation sans paramètre rend la même chose
    // qu'avec ses valeurs par défaut explicites », et contrat d'API : « `reglages(&[])` égale
    // `Reglages::default()` ».
    //
    // C'est ce qui rend `animate vague` équivalent à `animate vague couleur=… vitesse=…` écrit en
    // toutes lettres, donc ce qui permet à la fenêtre d'envoyer toujours la forme longue sans
    // changer le rendu de la forme courte. Sans cette égalité, l'aperçu et la commande sans
    // paramètre divergeraient — exactement le divorce que le crate partagé vient empêcher.
    let geometrie = geometrie_de_test();
    let defaut = Reglages::default();

    for animation in catalogue() {
        let implicites = animation
            .reglages(&[])
            .expect("aucune paire, donc aucune paire fautive");
        assert_eq!(
            implicites,
            defaut,
            "« {} » sans paramètre doit rendre les réglages par défaut",
            animation.nom()
        );

        // Et la même chose écrite en toutes lettres, clé par clé, dans l'écriture du protocole.
        let paires: Vec<(String, String)> = animation
            .parametres_acceptes()
            .iter()
            .map(|cle| paire(cle, &valeur_par_defaut(cle, &defaut)))
            .collect();
        let explicites = animation.reglages(&paires).unwrap_or_else(|erreur| {
            panic!(
                "« {} » refuse ses propres valeurs par défaut : {erreur}",
                animation.nom()
            )
        });
        assert_eq!(
            explicites,
            implicites,
            "« {} » : {paires:?} doit valoir l'absence de paramètre",
            animation.nom()
        );

        // Jusque dans l'image, qui est la seule chose que l'utilisateur verra.
        for pas in [0u32, 1, 17, 30] {
            assert_eq!(
                animation.image(&geometrie, &explicites, pas),
                animation.image(&geometrie, &implicites, pas),
                "« {} » au pas {pas} : le défaut implicite et le défaut explicite doivent peindre \
                 la même image",
                animation.nom()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 8 — la couleur demandée se retrouve dans l'image
// ---------------------------------------------------------------------------

#[test]
fn la_couleur_demandee_se_retrouve_dans_l_image_rendue() {
    // Test d'intention n° 8 de l'issue — « Le paramètre couleur se retrouve réellement dans
    // l'image rendue ».
    //
    // Le défaut visé est muet : une animation qui accepterait `couleur=` puis peindrait sa
    // teinte à elle. Rien ne le signale, et à l'œil « c'est joli » l'emporte souvent sur « ce
    // n'est pas la couleur que j'ai demandée ».
    //
    // Ce qui est exigé n'est **pas** que la couleur apparaisse au bit près : un dégradé peut
    // n'atteindre son maximum sur aucune LED, et l'exiger spécifierait la forme du motif. Ce qui
    // est exigé, c'est que l'**ordre des composantes** de la couleur demandée se retrouve quelque
    // part dans l'image. Cet ordre survit à tout assombrissement et à tout éclaircissement, il ne
    // survit ni à une couleur ignorée ni à une permutation.
    let geometrie = geometrie_de_test();
    let led = toutes_les_led();
    let mut testees = 0;

    for animation in catalogue() {
        if !animation.parametres_acceptes().contains(&"couleur") {
            continue;
        }
        testees += 1;

        for temoin in [TEMOIN, TEMOIN_BIS] {
            let reglages = animation
                .reglages(&[paire("couleur", &hexa(temoin))])
                .expect("six chiffres hexadécimaux sont une couleur");
            assert_eq!(
                reglages.couleur,
                temoin,
                "« {} » : la clé `couleur` doit atterrir dans le champ `couleur`",
                animation.nom()
            );

            let mut trouvee = false;
            for pas in 0..64u32 {
                let image = animation.image(&geometrie, &reglages, pas);
                if led
                    .iter()
                    .any(|l| meme_ordre_de_composantes(l.couleur(&image), temoin))
                {
                    trouvee = true;
                    break;
                }
            }
            assert!(
                trouvee,
                "« {} » : aucune LED ne porte la teinte {} en 64 pas — la couleur demandée n'entre \
                 pas dans le rendu",
                animation.nom(),
                hexa(temoin)
            );
        }

        // Et deux couleurs différentes donnent deux images différentes. C'est le même défaut vu
        // de l'autre côté, et il attrape le cas où la couleur ne servirait qu'à une LED témoin.
        let une = animation
            .reglages(&[paire("couleur", &hexa(TEMOIN))])
            .expect("couleur valide");
        let autre = animation
            .reglages(&[paire("couleur", &hexa(TEMOIN_BIS))])
            .expect("couleur valide");
        let differe = (0..64u32).any(|pas| {
            animation.image(&geometrie, &une, pas) != animation.image(&geometrie, &autre, pas)
        });
        assert!(
            differe,
            "« {} » peint la même image pour {} et {} : la couleur est ignorée",
            animation.nom(),
            hexa(TEMOIN),
            hexa(TEMOIN_BIS)
        );
    }

    assert!(
        testees > 0,
        "aucune animation n'accepte de couleur : le test ne vérifie rien"
    );
}

// ---------------------------------------------------------------------------
// 9 — une onde verticale atteint ensemble les LED de même hauteur
// ---------------------------------------------------------------------------

#[test]
fn une_onde_verticale_atteint_ensemble_les_led_declarees_a_la_meme_hauteur() {
    // Test d'intention n° 9 de l'issue — « Deux LED déclarées à la même hauteur reçoivent la même
    // couleur d'une onde verticale », et comportement attendu : « une onde qui monte doit
    // atteindre en même temps deux LED physiquement à la même hauteur, quels que soient leur
    // ventilateur et leur numéro d'ordre ».
    //
    // C'est **le** test de ce chantier. Le défaut qu'il vise est celui que l'issue décrit :
    // l'animation actuelle « traite les 124 LED comme une file d'attente numérotée, pas comme un
    // volume », et ça se lit comme un balayage. Une implémentation qui indexerait par numéro de
    // LED plutôt que par coordonnée le passerait à l'œil et échouerait ici.
    //
    // ⚠️ Les groupes de même hauteur sont **demandés à la géométrie**, jamais écrits ici : la
    // table n'est pas encore mesurée, et un test qui nommerait deux LED précises spécifierait une
    // mesure que personne n'a faite.
    let geometrie = geometrie_de_test();

    let mut par_hauteur: BTreeMap<u32, Vec<Led>> = BTreeMap::new();
    for led in toutes_les_led() {
        par_hauteur
            .entry(cle_hauteur(led.hauteur(&geometrie)))
            .or_default()
            .push(led);
    }
    let groupes: Vec<&Vec<Led>> = par_hauteur.values().filter(|g| g.len() >= 2).collect();

    assert!(
        !groupes.is_empty(),
        "aucune LED du boîtier n'en a une autre à sa hauteur : la table ne peut porter aucune \
         onde synchronisée, et le critère d'acceptation devient invérifiable"
    );
    assert!(
        groupes.iter().any(|groupe| {
            let premier = groupe[0].organe();
            groupe.iter().any(|led| led.organe() != premier)
        }),
        "aucun groupe de même hauteur ne traverse deux organes : l'issue demande que ce soit vrai \
         « quels que soient leur ventilateur et leur numéro d'ordre »"
    );

    let vague = Animation::par_nom("vague").expect("« vague » est au catalogue");

    for direction in [Direction::BasHaut, Direction::HautBas] {
        let reglages = reglages(TEMOIN, direction);
        for pas in 0..24u32 {
            let image = vague.image(&geometrie, &reglages, pas);
            for groupe in &groupes {
                let referente = groupe[0];
                let attendue = referente.couleur(&image);
                for led in groupe.iter().skip(1) {
                    assert_eq!(
                        led.couleur(&image),
                        attendue,
                        "au pas {pas} en {}, {} et {} sont à la même hauteur ({}) et reçoivent \
                         deux couleurs différentes",
                        slug_direction(direction),
                        referente.nom(),
                        led.nom(),
                        referente.hauteur(&geometrie)
                    );
                }
            }
        }
    }

    // Le pendant, sans lequel le test précédent est satisfait par une image uniforme : deux LED
    // de hauteurs **différentes** doivent, à un moment, recevoir des couleurs différentes. Une
    // onde qui monte se voit précisément à ça.
    let reglages = reglages(TEMOIN, Direction::BasHaut);
    let hauteurs: Vec<&Vec<Led>> = par_hauteur.values().collect();
    assert!(
        hauteurs.len() >= 2,
        "toutes les LED du boîtier sont à la même hauteur : il n'y a pas de volume"
    );
    let basse = hauteurs[0][0];
    let haute = hauteurs[hauteurs.len() - 1][0];
    let distingue = (0..600u32).any(|pas| {
        let image = vague.image(&geometrie, &reglages, pas);
        basse.couleur(&image) != haute.couleur(&image)
    });
    assert!(
        distingue,
        "« vague » peint {} (y = {}) et {} (y = {}) de la même couleur à tous les pas : l'onde ne \
         monte pas",
        basse.nom(),
        basse.hauteur(&geometrie),
        haute.nom(),
        haute.hauteur(&geometrie)
    );
}

// ---------------------------------------------------------------------------
// 12 — changer une orientation ne change que ce ventilateur
// ---------------------------------------------------------------------------

#[test]
fn changer_l_orientation_d_un_ventilateur_change_son_image_et_elle_seule() {
    // Test d'intention n° 12 de l'issue — « Changer l'orientation d'un ventilateur change l'image
    // rendue pour ce ventilateur **et pour lui seul** », et comportement attendu : « La géométrie
    // se corrige sans recompiler : remonter un ventilateur à l'envers se rattrape par une
    // commande ».
    //
    // Les deux moitiés comptent autant. Que les neuf autres ne bougent pas dit que la correction
    // est confinée — sinon corriger un ventilateur en dérèglerait un autre, et on tournerait en
    // rond sous le bureau. Que **celui-là** bouge dit que la correction sert à quelque chose : une
    // orientation rangée dans une table mais absente du calcul de l'image serait un réglage sans
    // effet, et c'est le défaut le plus facile à commettre.
    let base = geometrie_de_test();
    let animations = catalogue();
    let pas_scrutes: [u32; 6] = [0, 1, 5, 17, 30, 97];

    for cible in Position::ALL {
        let mut avant = base.clone();
        let mut apres = base.clone();
        avant.definir(
            cible,
            Orientation::new(0, Sens::Horaire).expect("0° horaire est valide"),
        );
        apres.definir(
            cible,
            Orientation::new(90, Sens::Antihoraire).expect("90° antihoraire est valide"),
        );

        let mut a_change = false;
        for animation in &animations {
            for direction in DIRECTIONS {
                let reglages = reglages(TEMOIN, direction);
                for pas in pas_scrutes {
                    let image_avant = animation.image(&avant, &reglages, pas);
                    let image_apres = animation.image(&apres, &reglages, pas);

                    for autre in Position::ALL {
                        if autre == cible {
                            continue;
                        }
                        assert_eq!(
                            couleurs_du_ventilateur(&image_avant, autre),
                            couleurs_du_ventilateur(&image_apres, autre),
                            "« {} » en {} au pas {pas} : réorienter {} a changé l'image de {}",
                            animation.nom(),
                            slug_direction(direction),
                            cible.slug(),
                            autre.slug()
                        );
                    }
                    assert_eq!(
                        image_avant.barrettes,
                        image_apres.barrettes,
                        "« {} » en {} au pas {pas} : réorienter {} a changé l'image des barrettes",
                        animation.nom(),
                        slug_direction(direction),
                        cible.slug()
                    );

                    if couleurs_du_ventilateur(&image_avant, cible)
                        != couleurs_du_ventilateur(&image_apres, cible)
                    {
                        a_change = true;
                    }
                }
            }
        }

        assert!(
            a_change,
            "réorienter {} de 0° horaire à 90° antihoraire ne change l'image d'aucune animation, \
             dans aucune direction, à aucun des pas scrutés : l'orientation n'entre pas dans le \
             rendu",
            cible.slug()
        );
    }
}

// ---------------------------------------------------------------------------
// Valeurs de réglage, par clé
// ---------------------------------------------------------------------------

/// Une valeur que la clé donnée doit accepter.
fn valeur_valide(cle: &str) -> String {
    match cle {
        "couleur" => hexa(TEMOIN),
        // 3 n'a rien de particulier : une vitesse au milieu du domaine, ni la borne basse ni la
        // borne haute, dont le contrat ne dit pas si elles sont admises.
        "vitesse" => "3".to_owned(),
        "direction" | "sens" => slug_direction(Direction::BasHaut).to_owned(),
        autre => panic!("« {autre} » n'est pas une clé connue de ce test"),
    }
}

/// Des couples de valeurs valides que la clé donnée doit **distinguer** au rendu.
///
/// Pour la direction, les quinze couples des six : deux directions qui peindraient la même image
/// à tous les pas rendraient l'une des deux inutile. Pour la vitesse, 1 et 7 — deux entiers sans
/// autre propriété que d'être distincts et loin l'un de l'autre.
fn valeurs_distinctes(cle: &str) -> Vec<(&'static str, &'static str)> {
    match cle {
        "couleur" => vec![("ff2080", "20ff80")],
        "vitesse" => vec![("1", "7")],
        "direction" | "sens" => {
            let mut couples = Vec::new();
            for (i, une) in DIRECTIONS.iter().enumerate() {
                for autre in DIRECTIONS.iter().skip(i + 1) {
                    couples.push((slug_direction(*une), slug_direction(*autre)));
                }
            }
            couples
        }
        autre => panic!("« {autre} » n'est pas une clé connue de ce test"),
    }
}

/// Deux jeux de réglages peignent-ils deux images différentes, sur un balayage de pas ?
///
/// Vingt-quatre pas, moins d'une seconde à 30 img/s : de quoi laisser tout motif se déplacer, et
/// assez court pour que la boucle exhaustive sur les couples reste rapide.
fn rendus_differents(
    animation: &Animation,
    geometrie: &Geometrie,
    une: &Reglages,
    autre: &Reglages,
) -> bool {
    (0..24u32)
        .any(|pas| animation.image(geometrie, une, pas) != animation.image(geometrie, autre, pas))
}

/// La valeur par défaut de la clé donnée, écrite comme le protocole l'écrit.
fn valeur_par_defaut(cle: &str, defaut: &Reglages) -> String {
    match cle {
        "couleur" => hexa(defaut.couleur),
        "vitesse" => defaut.vitesse.to_string(),
        "direction" | "sens" => slug_direction(defaut.direction).to_owned(),
        autre => panic!("« {autre} » n'est pas une clé connue de ce test"),
    }
}

/// Des valeurs que la clé donnée doit refuser.
///
/// Chacune est le genre de faute qu'on commet vraiment : une couleur à quatre ou huit chiffres,
/// un `#` recopié d'un sélecteur de couleur web, une vitesse qui déborde le `u8`, un négatif, un
/// décimal, un mot à la place d'un nombre, une casse ou une graphie voisine pour une direction.
fn valeurs_invalides(cle: &str) -> &'static [&'static str] {
    match cle {
        "couleur" => &[
            "zzzzzz", "ff00", "ff00ff00", "#ff00ff", "-1", "", "ff 00 ff", "0xff00ff", "rouge",
            "ffffffff",
        ],
        "vitesse" => &[
            "256", "1000", "65536", "-1", "2.5", "beaucoup", "", " 3", "0x10", "3 ",
        ],
        "direction" | "sens" => &[
            "diagonal",
            "bas_haut",
            "BAS-HAUT",
            "Bas-Haut",
            "haut",
            "bas-haut-bas",
            "",
            "0",
            "avant-arrière",
            "anti-horaire",
        ],
        autre => panic!("« {autre} » n'est pas une clé connue de ce test"),
    }
}

/// Les composantes de `rendue` sont-elles rangées dans le même ordre que celles de `demandee` ?
///
/// Deux couleurs proportionnelles ont le même ordre de composantes : c'est ce qui rend le critère
/// vrai d'un dégradé, où la teinte demandée n'apparaît jamais à pleine intensité. Le noir est
/// exclu, sans quoi une image éteinte satisferait tout.
fn meme_ordre_de_composantes(rendue: Rgb, demandee: Rgb) -> bool {
    if rendue == Rgb::BLACK {
        return false;
    }
    ordre(rendue.r, rendue.g) == ordre(demandee.r, demandee.g)
        && ordre(rendue.g, rendue.b) == ordre(demandee.g, demandee.b)
        && ordre(rendue.r, rendue.b) == ordre(demandee.r, demandee.b)
}

fn ordre(gauche: u8, droite: u8) -> std::cmp::Ordering {
    gauche.cmp(&droite)
}
