//! Tests d'intention de la composition de l'écran — issue #80, côté **pur**.
//!
//! Écrits **avant** l'implémentation, depuis l'issue #80 seule et le contrat public qu'elle fixe.
//! Rien de `crates/*/src/` n'a été lu pour les produire. Si l'un de ces tests échoue après
//! implémentation, c'est le code qu'on corrige, jamais le test.
//!
//! Ce fichier ne couvre que ce qui se tient sans matériel et sans démon : les cinq ancres et leurs
//! boîtes, les sources, le fond, la composition et son aller-retour, et le protocole texte. Le
//! **rendu** — dessiner les champs dans un tampon 640 × 640 — vit dans
//! `crates/reverb-daemon/tests/spec_composition_ecran.rs`, parce qu'il a besoin de `Dalle`.
//!
//! ## Ce que l'issue raconte, et que ce fichier garde
//!
//! ⚠️ **La dalle est ronde**, observé sur le matériel le 2026-08-08. Les quatre coins du tampon
//! 640 × 640 sont donc hors du disque visible — 21 % de la surface —, et une ancre posée dans un
//! coin dessinerait dans le vide **sans le moindre signal** : le contrôleur accepte les 1 228 800
//! octets quels qu'ils soient, et ce qui tombe hors de la vitre est simplement invisible. C'est le
//! mode de défaillance le plus coûteux de ce chantier parce qu'il est silencieux, et c'est la
//! raison d'être de [`Boite::dans_le_disque`] et des trois tests qui l'entourent.
//!
//! ## Les quatre pièges que ce fichier garde
//!
//! 1. **Un champ hors du disque ne remonte aucune erreur.** Voir ci-dessus. La géométrie des cinq
//!    ancres est donc vérifiée par le calcul de l'issue — `x² + y² ≤ 320²` aux quatre coins de la
//!    boîte —, et non par la seule parole de `dans_le_disque`, qui pourrait rendre `true` sans rien
//!    calculer. D'où [`dans_le_disque_refuse_les_coins_du_tampon`], qui exige que le prédicat sache
//!    aussi dire non.
//! 2. **Un texte coupé au premier blanc désigne autre chose.** « soirée d'été » tronqué donne
//!    « soirée », `/home/nico/Mes documents/fond.png` donne `/home/nico/Mes`. La convention du
//!    protocole depuis #74 — **au plus un champ libre, en fin de ligne** — est ce qui rend l'issue
//!    réalisable, et c'est aussi pourquoi elle donne **une commande par changement** plutôt qu'une
//!    ligne portant à la fois un chemin et un libellé.
//! 3. **Un plafond qui compte les poses au lieu des ancres se ferme tout seul.** Reposer un champ
//!    sur une ancre déjà occupée est un **remplacement**, jamais un cinquième champ : le contraire
//!    rendrait la quatrième ancre inchangeable après trois corrections.
//! 4. **Un fichier tronqué doit rester détectable.** `ecran.conf` gagne des lignes, et la lecture
//!    stricte de #33 doit continuer à **nommer la ligne fautive** — sans quoi une entrée perdue est
//!    remplacée au jugé par une composition plausible et fausse.
//!
//! ## Cinq points que l'issue laisse ouverts, et que ces tests tranchent
//!
//! 1. **Le chemin d'un fond doit être absolu sur le fil**, comme celui de `screen image` (#33) et
//!    pour la même raison : le démon lit sous son propre répertoire courant, qui n'est pas celui de
//!    son client. Ce fichier exige le refus et exige qu'il le **dise** ; il ne dit pas *où* le
//!    contrôle vit — `Fond::decoder` ou l'analyse de la requête —, seulement qu'il existe.
//! 2. **Les cinq boîtes ne se recouvrent pas deux à deux.** L'issue les dessine à cinq places
//!    distinctes ; deux boîtes qui mordraient l'une sur l'autre feraient qu'un champ efface son
//!    voisin selon l'ordre de rendu, et « quatre champs au plus » deviendrait « quatre champs dont
//!    deux illisibles ».
//! 3. **`Ancre::TOUTES` est l'ordre de rendu et d'écriture, et `champs()` le suit toujours** —
//!    jamais l'ordre de pose. Sans quoi deux compositions portant les mêmes champs s'écriraient
//!    différemment dans `ecran.conf` selon l'ordre des commandes tapées, et le « même tampon à
//!    température identique » du critère 12 dépendrait d'un historique.
//! 4. **Ni un texte ni un libellé décodé n'est vide.** Un champ vide occupe une ancre sans rien
//!    montrer : il se lit comme un bug d'affichage, et il consomme l'un des quatre.
//! 5. **`layout` et `layout-champ` sont deux lignes de réponse distinctes**, et la seconde ne doit
//!    pas se lire comme la première suivie d'un fond « -champ … ».
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! Il ne fige le **texte** d'aucun refus, seulement ce qu'il doit contenir pour être corrigeable :
//! le verbe `screen`, l'ancre fautive, les cinq noms valides, le plafond. Il ne dit rien de la
//! cadence de recomposition ni de la vigie de #70 : ce sont des boucles de service, pas des
//! fonctions pures, et elles ne s'atteignent pas depuis `reverb-proto`.

use std::collections::BTreeSet;

use reverb_proto::composition::{
    Ancre, AncreInconnue, Boite, Composition, CompositionInvalide, Fond, FondInvalide, Source,
    SourceInvalide, TropDeChamps,
};
use reverb_proto::ipc::{
    Request, RequestError, ResponseLine, ScreenAction, encode_request, encode_response_line,
    parse_request, parse_response_line,
};
use reverb_proto::screen;

// ---------------------------------------------------------------------------
// Vecteurs témoins
// ---------------------------------------------------------------------------

/// Une sonde, écrite comme le démon la publie. C'est l'exemple de l'issue.
const SONDE: &str = "kraken2023elite:coolant-temp";

/// Une autre, pour que rien ne puisse être écrit en dur.
const AUTRE_SONDE: &str = "k10temp:tctl";

/// Le libellé qui justifie l'existence du champ : « kraken2023elite:coolant-temp » ne se lit pas
/// sur six centimètres.
const LIBELLE: &str = "Liquide";

/// Un libellé qui porte des espaces et des accents — le cas que la règle du dernier champ existe
/// pour couvrir.
const LIBELLE_A_ESPACE: &str = "Liquide — boucle haute";

/// Un chemin absolu quelconque.
const CHEMIN: &str = "/home/nico/images/fond.png";

/// Un chemin absolu portant une espace, comme en rend un gestionnaire de fichiers.
const CHEMIN_A_ESPACE: &str = "/home/nico/Mes documents/fond d'écran.png";

/// Les cinq noms d'ancre, tels que l'issue les écrit.
const SLUGS: [&str; 5] = ["haut", "bas", "gauche", "droite", "centre"];

/// Champs **hostiles** : ceux qui, mal encodés, produiraient une ligne qu'un client prendrait pour
/// la fin de la réponse, ou pour deux champs là où il n'y en a qu'un.
///
/// Liste reprise de `spec_ipc_ecran.rs` (#33), dont la raison vaut ici encore : un libellé est tapé
/// par un humain, un chemin vient du disque, et ni l'un ni l'autre n'est écrit par le protocole.
const HOSTILES: &[&str] = &[
    "err",
    "end",
    "error",
    "endpoint",
    "ERR",
    "boom\nend",
    "\nend",
    "end\n",
    "a\r\nend",
    "mon libellé",
    "\t",
    "  ",
    "a\u{0}b",
    "screen",
    "layout",
    "off",
    "\u{feff}a",
];

// ---------------------------------------------------------------------------
// Petits outils
// ---------------------------------------------------------------------------

fn temperature(sonde: &str, libelle: Option<&str>) -> Source {
    Source::Temperature {
        sonde: sonde.to_owned(),
        libelle: libelle.map(str::to_owned),
    }
}

fn texte(contenu: &str) -> Source {
    Source::Texte(contenu.to_owned())
}

/// Les sources témoins, dans leurs quatre formes.
fn sources_temoins() -> Vec<Source> {
    vec![
        temperature(SONDE, None),
        temperature(AUTRE_SONDE, None),
        temperature(SONDE, Some(LIBELLE)),
        temperature(SONDE, Some(LIBELLE_A_ESPACE)),
        temperature(AUTRE_SONDE, Some("CPU")),
        texte("Bonjour"),
        texte("soirée d'été"),
        texte("LAN party — salle 2"),
        texte("temp"),
        texte("texte"),
        texte("100 %"),
        texte("夜"),
    ]
}

/// Les deux formes de fond.
fn fonds_temoins() -> Vec<Fond> {
    vec![
        Fond::Noir,
        Fond::Image(CHEMIN.to_owned()),
        Fond::Image(CHEMIN_A_ESPACE.to_owned()),
        Fond::Image("/a.png".to_owned()),
    ]
}

/// Des compositions de zéro à quatre champs, sur des ancres et des sources variées.
fn compositions_temoins() -> Vec<Composition> {
    let mut temoins = vec![Composition::nouvelle(Fond::Noir)];

    for fond in fonds_temoins() {
        let mut une = Composition::nouvelle(fond.clone());
        une.poser(Ancre::Centre, temperature(SONDE, Some(LIBELLE)))
            .expect("un premier champ tient toujours");
        temoins.push(une);

        let mut deux = Composition::nouvelle(fond.clone());
        deux.poser(Ancre::Haut, texte(LIBELLE_A_ESPACE))
            .expect("un premier champ tient toujours");
        deux.poser(Ancre::Bas, temperature(AUTRE_SONDE, None))
            .expect("un deuxième champ tient toujours");
        temoins.push(deux);

        let mut quatre = Composition::nouvelle(fond);
        for (ancre, source) in [
            (Ancre::Haut, temperature(SONDE, Some(LIBELLE))),
            (Ancre::Bas, temperature(AUTRE_SONDE, Some("CPU"))),
            (Ancre::Gauche, texte("soirée d'été")),
            (Ancre::Droite, texte("GPU")),
        ] {
            quatre
                .poser(ancre, source)
                .expect("quatre champs est le plafond, pas une borne franchie");
        }
        temoins.push(quatre);
    }

    temoins
}

/// Le rang d'une ancre dans [`Ancre::TOUTES`] — l'ordre de rendu et d'écriture.
fn rang(ancre: Ancre) -> usize {
    Ancre::TOUTES
        .iter()
        .position(|&candidate| candidate == ancre)
        .unwrap_or_else(|| panic!("{ancre:?} doit figurer dans Ancre::TOUTES"))
}

/// Les quatre coins de la boîte, en coordonnées **relatives au centre du tampon**, tels que le
/// critère d'acceptation les nomme : « la boîte englobante de chaque champ vérifie `x² + y² ≤ 320²`
/// à ses quatre coins ».
fn coins(boite: Boite) -> [(i64, i64); 4] {
    let centre_x = i64::from(screen::WIDTH) / 2;
    let centre_y = i64::from(screen::HEIGHT) / 2;
    let gauche = i64::from(boite.x) - centre_x;
    let haut = i64::from(boite.y) - centre_y;
    let droite = gauche + i64::from(boite.largeur);
    let bas = haut + i64::from(boite.hauteur);
    [(gauche, haut), (droite, haut), (gauche, bas), (droite, bas)]
}

/// Le refus d'une ancre, avec ce que ce fichier exige de tout refus.
fn refus_d_ancre(saisi: &str) -> AncreInconnue {
    match Ancre::depuis_slug(saisi) {
        Ok(ancre) => panic!(
            "« {} » devait être refusé, il a rendu {ancre:?} — une ancre devinée pose le champ \
             ailleurs que là où on l'a demandé, sans rien dire",
            saisi.escape_debug()
        ),
        Err(erreur) => {
            assert_eq!(
                erreur.saisi, saisi,
                "le refus doit rendre ce qu'on lui a donné, pour que le message soit citable"
            );
            let _: &dyn std::error::Error = &erreur;
            erreur
        }
    }
}

/// Le refus d'une source, non muet.
fn refus_de_source(brut: &str) -> SourceInvalide {
    match Source::decoder(brut) {
        Ok(source) => panic!("« {brut:?} » devait être refusé, il a rendu {source:?}"),
        Err(erreur) => {
            assert!(
                !erreur.raison.trim().is_empty(),
                "« {brut:?} » doit être refusé en disant pourquoi"
            );
            assert!(
                erreur.to_string().contains(erreur.raison.as_str()),
                "le Display porte la raison : « {erreur} »"
            );
            let _: &dyn std::error::Error = &erreur;
            erreur
        }
    }
}

/// Le refus d'un fond, non muet.
fn refus_de_fond(brut: &str) -> FondInvalide {
    match Fond::decoder(brut) {
        Ok(fond) => panic!("« {brut:?} » devait être refusé, il a rendu {fond:?}"),
        Err(erreur) => {
            assert!(
                !erreur.raison.trim().is_empty(),
                "« {brut:?} » doit être refusé en disant pourquoi"
            );
            let _: &dyn std::error::Error = &erreur;
            erreur
        }
    }
}

/// Le refus d'un bloc de composition, avec le rang de ligne qu'il nomme.
fn refus_de_composition(bloc: &str) -> CompositionInvalide {
    match Composition::decoder(bloc) {
        Ok(composition) => panic!(
            "ce bloc devait être refusé, il a rendu {composition:?} :\n{bloc}\nUn bloc abîmé \
             accepté est une composition plausible et fausse, rejouée à chaque démarrage"
        ),
        Err(erreur) => {
            assert!(
                !erreur.raison.trim().is_empty(),
                "un bloc abîmé doit être refusé en disant pourquoi :\n{bloc}"
            );
            assert!(
                erreur.ligne >= 1,
                "les lignes se comptent à partir de 1, la première étant celle du fond — obtenu \
                 {} sur :\n{bloc}",
                erreur.ligne
            );
            let _: &dyn std::error::Error = &erreur;
            erreur
        }
    }
}

/// Le verbe et la raison d'un refus de requête. Convention de `spec_ipc_ecran.rs`.
fn refus_de_requete(ligne: &str) -> (String, String) {
    match parse_request(ligne) {
        Ok(requete) => panic!("« {ligne} » devait être refusée, elle a rendu {requete:?}"),
        Err(RequestError::BadArgument { verb, reason }) => {
            assert!(
                !reason.trim().is_empty(),
                "« {ligne} » doit être refusée en disant pourquoi"
            );
            (verb, reason)
        }
        Err(autre) => panic!("« {ligne} » doit donner un BadArgument, pas {autre:?}"),
    }
}

/// Le refus d'une commande d'écran : le verbe nommé doit être `screen`.
///
/// Même arbitrage que #33 : le verbe reçu **existe**, ce sont ses arguments qui sont mauvais.
fn refus_de_screen(ligne: &str) -> String {
    let (verbe, raison) = refus_de_requete(ligne);
    assert_eq!(
        verbe, "screen",
        "« {ligne} » : le verbe reçu est `screen`, l'erreur doit le nommer"
    );
    raison
}

/// L'action d'une requête `screen`, ou un échec qui dit ce qu'on a reçu à la place.
fn action_de(ligne: &str) -> ScreenAction {
    match parse_request(ligne) {
        Ok(Request::Screen(action)) => action,
        Ok(autre) => {
            panic!("« {ligne} » devait être une commande `screen`, elle a rendu {autre:?}")
        }
        Err(erreur) => panic!("« {ligne} » devait être acceptée : {erreur}"),
    }
}

/// Vérifie qu'une ligne de données ne peut pas se faire prendre pour une fin de réponse.
///
/// Règle n° 5 de `spec_ipc.rs` : « une ligne de données ne commence **jamais** par `end` ni par
/// `err` », et elle tient sur une seule ligne physique.
fn ne_termine_jamais(donnee: &ResponseLine) {
    assert!(
        !donnee.is_terminal(),
        "{donnee:?} n'est pas une ligne terminale"
    );

    let encodee = encode_response_line(donnee);
    assert_eq!(
        encodee.lines().count(),
        1,
        "une ligne de données tient sur une seule ligne : « {encodee} »"
    );
    assert!(
        !encodee.contains('\n') && !encodee.contains('\r'),
        "aucun saut de ligne dans une ligne de données : « {encodee} »"
    );
    for terminal in ["end", "err"] {
        assert!(
            !encodee.starts_with(terminal),
            "la ligne « {encodee} » commence par « {terminal} » — un client y verrait la fin de la \
             réponse et tronquerait sa lecture"
        );
    }
}

// ---------------------------------------------------------------------------
// 1 — les cinq ancres
// ---------------------------------------------------------------------------

#[test]
fn les_cinq_ancres_ont_des_noms_stables_distincts_et_relus() {
    // Comportement attendu de l'issue : « Cinq ancres : `haut`, `bas`, `gauche`, `droite`,
    // `centre`. Aucune en coin — elles seraient hors du disque. »
    //
    // Les noms sont écrits sur le socket **et** dans `ecran.conf`. Les renommer casserait les deux
    // à la fois, et un fichier écrit avant le renommage deviendrait illisible au redémarrage
    // suivant — ce qui est exactement la panne que #69 a coûtée.
    assert_eq!(
        Ancre::TOUTES.len(),
        5,
        "cinq ancres, ni quatre ni six : les coins sont hors du disque visible"
    );

    let noms: Vec<&str> = Ancre::TOUTES.iter().map(|ancre| ancre.slug()).collect();
    let attendus: BTreeSet<&str> = SLUGS.into_iter().collect();
    let obtenus: BTreeSet<&str> = noms.iter().copied().collect();
    assert_eq!(
        obtenus, attendus,
        "les cinq noms sont ceux de l'issue, à la lettre : {noms:?}"
    );
    assert_eq!(
        obtenus.len(),
        5,
        "deux ancres qui partageraient un nom en rendraient une inatteignable : {noms:?}"
    );

    for ancre in Ancre::TOUTES {
        assert_eq!(
            Ancre::depuis_slug(ancre.slug()),
            Ok(ancre),
            "« {} » doit se relire en {ancre:?}",
            ancre.slug()
        );
    }

    // Sensible à la casse, comme tout le reste du protocole : `Haut` n'est pas `haut`. Accepter les
    // deux ferait deux écritures possibles du même fichier, et l'aller-retour cesserait d'être une
    // égalité.
    for variante in ["Haut", "HAUT", "Centre", " haut", "haut "] {
        refus_d_ancre(variante);
    }
}

#[test]
fn aucune_ancre_n_est_dans_un_coin_et_toutes_tiennent_dans_le_disque() {
    // Test d'intention n° 2 de l'issue, et critère d'acceptation : « les cinq ancres tiennent dans
    // le **disque inscrit** : la boîte englobante de chaque champ vérifie `x² + y² ≤ 320²` à ses
    // quatre coins, et c'est testé sans matériel ».
    //
    // Piège n° 1 du préambule : ce qui tombe hors de la vitre ne remonte **aucune** erreur. Le
    // contrôleur avale les 1 228 800 octets quels qu'ils soient, et un champ posé dans un coin est
    // simplement invisible — un champ qu'on croit affiché et que personne ne voit.
    //
    // Le calcul est fait ici, à la main, et non délégué à `dans_le_disque` : un prédicat qui
    // rendrait `true` sans rien calculer passerait toutes les autres assertions de ce fichier.
    let rayon = i64::from(screen::VISIBLE_DISC_RADIUS);
    let carre = rayon * rayon;

    for ancre in Ancre::TOUTES {
        let boite = ancre.boite();

        assert!(
            boite.largeur >= 5 && boite.hauteur >= 7,
            "{ancre:?} : une boîte de {}×{} est plus petite qu'un caractère de la matricielle \
             5 × 7 — elle ne peut rien montrer",
            boite.largeur,
            boite.hauteur
        );
        assert!(
            u32::from(boite.x) + u32::from(boite.largeur) <= u32::from(screen::WIDTH)
                && u32::from(boite.y) + u32::from(boite.hauteur) <= u32::from(screen::HEIGHT),
            "{ancre:?} : la boîte {boite:?} sort du tampon {}×{}",
            screen::WIDTH,
            screen::HEIGHT
        );

        for (x, y) in coins(boite) {
            assert!(
                x * x + y * y <= carre,
                "{ancre:?} : le coin ({x}, {y}) de la boîte {boite:?} donne {} pour un disque de \
                 rayon {rayon}, soit {carre} — un champ dessiné là est invisible, et rien ne le dit",
                x * x + y * y
            );
        }

        assert!(
            boite.dans_le_disque(),
            "{ancre:?} : `dans_le_disque` doit être d'accord avec le calcul de l'issue sur \
             {boite:?}"
        );
    }
}

#[test]
fn dans_le_disque_refuse_les_coins_du_tampon() {
    // Le pendant du test précédent, et il vaut autant : un prédicat qui ne sait pas dire non ne
    // protège de rien. Les quatre coins du tampon sont **hors** du disque — c'est le fait observé
    // le 2026-08-08 et le chiffre que l'issue en tire, 21 % de la surface.
    /// Le côté des boîtes témoins posées dans les coins, en pixels.
    const COIN: u16 = 40;

    for (x, y) in [
        (0, 0),
        (screen::WIDTH - COIN, 0),
        (0, screen::HEIGHT - COIN),
        (screen::WIDTH - COIN, screen::HEIGHT - COIN),
    ] {
        let boite = Boite {
            x,
            y,
            largeur: COIN,
            hauteur: COIN,
        };
        assert!(
            !boite.dans_le_disque(),
            "{boite:?} est dans un coin du tampon : elle est hors du disque visible, et \
             `dans_le_disque` doit le dire"
        );
    }

    // Et une boîte centrée y est, sans quoi le prédicat refuserait tout.
    let centree = Boite {
        x: screen::WIDTH / 2 - 20,
        y: screen::HEIGHT / 2 - 10,
        largeur: 40,
        hauteur: 20,
    };
    assert!(
        centree.dans_le_disque(),
        "{centree:?} entoure le centre du tampon : elle est dans le disque"
    );
}

#[test]
fn les_cinq_boites_ne_se_recouvrent_pas() {
    // Point n° 2 des conventions tranchées en tête de fichier. L'issue dessine cinq places
    // distinctes ; deux boîtes qui mordraient l'une sur l'autre feraient qu'un champ efface son
    // voisin selon l'ordre de rendu — et « jusqu'à quatre informations » deviendrait « quatre
    // informations dont deux illisibles », sans le moindre message.
    //
    // C'est aussi ce qui rend le rendu indépendant de l'ordre des champs, propriété que
    // `spec_composition_ecran.rs` exige de `Dalle::composee`.
    for (rang_une, une) in Ancre::TOUTES.iter().enumerate() {
        for autre in &Ancre::TOUTES[rang_une + 1..] {
            let a = une.boite();
            let b = autre.boite();
            let chevauche = u32::from(a.x) < u32::from(b.x) + u32::from(b.largeur)
                && u32::from(b.x) < u32::from(a.x) + u32::from(a.largeur)
                && u32::from(a.y) < u32::from(b.y) + u32::from(b.hauteur)
                && u32::from(b.y) < u32::from(a.y) + u32::from(a.hauteur);
            assert!(
                !chevauche,
                "{une:?} ({a:?}) et {autre:?} ({b:?}) se recouvrent — l'un des deux champs \
                 effacerait l'autre selon l'ordre de rendu"
            );
        }
    }
}

#[test]
fn le_disque_visible_est_celui_qui_est_inscrit_dans_le_tampon() {
    // Approche technique de l'issue : « Le disque visible est une **constante nommée**, pas un
    // nombre semé dans le code : la mire cercle de #77 dira s'il est exactement inscrit, et il n'y
    // aura alors qu'un endroit à corriger. »
    //
    // Ce test ne fige donc pas 320 comme une vérité de matériel — il vérifie que la constante est
    // bien celle du disque **inscrit**, valeur de départ que la mire affinera. Et il vérifie le
    // chiffre que l'issue en tire, 21 % de surface perdue : c'est lui qui justifie qu'aucune ancre
    // ne soit en coin, et il tomberait avec la constante si elle dérivait.
    let rayon = u32::from(screen::VISIBLE_DISC_RADIUS);
    assert!(
        rayon > 0
            && rayon * 2 <= u32::from(screen::WIDTH)
            && rayon * 2 <= u32::from(screen::HEIGHT),
        "le disque visible tient dans le tampon {}×{} : rayon {rayon}",
        screen::WIDTH,
        screen::HEIGHT
    );

    let centre = f64::from(screen::WIDTH) / 2.0;
    let carre = f64::from(rayon) * f64::from(rayon);
    let mut dehors = 0u32;
    for y in 0..u32::from(screen::HEIGHT) {
        for x in 0..u32::from(screen::WIDTH) {
            let dx = f64::from(x) + 0.5 - centre;
            let dy = f64::from(y) + 0.5 - centre;
            if dx * dx + dy * dy > carre {
                dehors += 1;
            }
        }
    }
    let perdu = f64::from(dehors) / (f64::from(screen::WIDTH) * f64::from(screen::HEIGHT));
    assert!(
        (0.20..0.23).contains(&perdu),
        "l'issue mesure 21 % de surface hors du disque ; ce rayon en donne {:.1} %. Si la mire de \
         #77 a vraiment révisé le rayon, c'est ce test qu'on relit — pas la constante qu'on \
         rattrape",
        perdu * 100.0
    );
}

#[test]
fn une_ancre_inconnue_est_refusee_en_donnant_les_cinq_noms() {
    // Test d'intention n° 8 de l'issue, et critère d'acceptation : « une ancre inconnue est refusée
    // **en donnant la liste** des cinq ».
    //
    // Donner la liste n'est pas du confort : les ancres sont cinq mots français dont aucun n'est
    // évident — « milieu » et « haut-gauche » sont les deux premiers qu'on essaie, et ni l'un ni
    // l'autre n'existe. Un « ancre inconnue » sec fait tâtonner devant un boîtier qui n'affiche
    // rien.
    for saisi in [
        "milieu",
        "haut-gauche",
        "coin",
        "top",
        "nord",
        "centre-haut",
        "",
        "   ",
        "haut bas",
    ] {
        let erreur = refus_d_ancre(saisi);
        let message = erreur.to_string();
        for nom in SLUGS {
            assert!(
                message.contains(nom),
                "« {} » : le refus doit citer les cinq ancres valides, il manque « {nom} ». \
                 Obtenu : « {message} »",
                saisi.escape_debug()
            );
        }
    }

    // Et le refus porte ce qui a été saisi : sans lui, un client qui journalise le message ne sait
    // pas quelle commande l'a produit.
    let erreur = refus_d_ancre("milieu");
    assert!(
        erreur.to_string().contains("milieu"),
        "le Display porte la saisie fautive : « {erreur} »"
    );
}

// ---------------------------------------------------------------------------
// 2 — sources et fond
// ---------------------------------------------------------------------------

#[test]
fn une_source_s_encode_et_se_decode_sans_perte() {
    // Contrat — `Source::encoder` : « `temp <slug>` · `temp <slug> <libellé>` · `texte <libellé>`.
    // Le dernier champ va jusqu'au bout de la ligne : il a droit aux espaces. »
    //
    // Piège n° 2 du préambule : un libellé coupé au premier blanc reste un libellé **lisible**.
    // « Liquide — boucle haute » deviendrait « Liquide », s'afficherait sans erreur, et personne ne
    // saurait qu'il manque quelque chose.
    for source in sources_temoins() {
        let encodee = source.encoder();
        assert!(
            !encodee.contains('\n') && !encodee.contains('\r'),
            "une source tient sur une ligne : « {encodee} »"
        );
        assert_eq!(
            Source::decoder(&encodee),
            Ok(source.clone()),
            "aller-retour de {source:?} par « {encodee} » — rien ne se perd en route"
        );
    }

    // La forme exacte, une fois par cas. Sans ces égalités, un encodeur pourrait écrire
    // `sonde <slug>` et tous les allers-retours resteraient verts : le décodeur rattraperait sa
    // propre écriture, et un `ecran.conf` écrit par une autre version deviendrait illisible.
    assert_eq!(temperature(SONDE, None).encoder(), format!("temp {SONDE}"));
    assert_eq!(
        temperature(SONDE, Some(LIBELLE)).encoder(),
        format!("temp {SONDE} {LIBELLE}")
    );
    assert_eq!(
        temperature(SONDE, Some(LIBELLE_A_ESPACE)).encoder(),
        format!("temp {SONDE} {LIBELLE_A_ESPACE}")
    );
    assert_eq!(
        texte(LIBELLE_A_ESPACE).encoder(),
        format!("texte {LIBELLE_A_ESPACE}")
    );

    // Une température sans libellé et une température au libellé vide ne sont pas la même chose :
    // point n° 4 du préambule, un champ vide occupe une ancre sans rien montrer.
    for brut in ["temp ", "temp", "texte", "texte ", "texte   ", "", "   "] {
        refus_de_source(brut);
    }
    for brut in [
        "bidule x",
        "sonde k10temp:tctl",
        "TEMP k10temp:tctl",
        "Texte a",
    ] {
        refus_de_source(brut);
    }

    // Aucune source décodée ne porte de champ vide, quelle que soit la forme de la ligne.
    for brut in [
        "temp k10temp:tctl CPU",
        "temp k10temp:tctl",
        "texte Bonjour",
        "texte  Bonjour",
    ] {
        if let Ok(source) = Source::decoder(brut) {
            match source {
                Source::Temperature { sonde, libelle } => {
                    assert!(
                        !sonde.trim().is_empty(),
                        "« {brut} » : une sonde vide ne désigne rien"
                    );
                    assert!(
                        !sonde.chars().any(char::is_whitespace),
                        "« {brut} » : la sonde est un jeton, comme dans `ResponseLine::Temp` — un \
                         blanc en ferait deux champs"
                    );
                    if let Some(libelle) = libelle {
                        assert!(
                            !libelle.trim().is_empty(),
                            "« {brut} » : un libellé vide occupe une ancre sans rien montrer"
                        );
                    }
                }
                Source::Texte(contenu) => assert!(
                    !contenu.trim().is_empty(),
                    "« {brut} » : un texte vide occupe une ancre sans rien montrer"
                ),
            }
        }
    }

    // Un texte qui **commence** par le mot d'une autre forme ne bascule pas dessus : c'est le
    // premier mot de la ligne qui décide, pas un mot trouvé au milieu.
    assert_eq!(
        Source::decoder("texte temp k10temp:tctl"),
        Ok(texte("temp k10temp:tctl")),
        "« temp » écrit dans un libellé reste un libellé"
    );
}

#[test]
fn un_fond_s_encode_et_se_decode_sans_perte() {
    // Comportement attendu : « `screen layout fond image <chemin>` le fond, mis à l'échelle comme
    // aujourd'hui · `screen layout fond noir` ou du noir uni ».
    //
    // Deux formes, et elles doivent rester distinctes : un fond noir qui se relirait en image
    // vide — ou l'inverse — ferait repartir le démon sur une dalle qu'il n'affichera jamais.
    for fond in fonds_temoins() {
        let encode = fond.encoder();
        assert!(
            !encode.contains('\n') && !encode.contains('\r'),
            "un fond tient sur une ligne : « {encode} »"
        );
        assert_eq!(
            Fond::decoder(&encode),
            Ok(fond.clone()),
            "aller-retour de {fond:?} par « {encode} »"
        );
    }

    assert_eq!(Fond::Noir.encoder(), "noir");
    assert_eq!(
        Fond::Image(CHEMIN.to_owned()).encoder(),
        format!("image {CHEMIN}")
    );
    assert_eq!(
        Fond::Image(CHEMIN_A_ESPACE.to_owned()).encoder(),
        format!("image {CHEMIN_A_ESPACE}"),
        "le chemin est le dernier champ de sa ligne : neutraliser ses espaces le ferait pointer \
         ailleurs"
    );

    for brut in [
        "",
        "   ",
        "image",
        "image ",
        "noir /a.png",
        "bidule",
        "Noir",
        "IMAGE /a.png",
    ] {
        refus_de_fond(brut);
    }

    assert_ne!(
        Fond::decoder("noir"),
        Fond::decoder("image /a.png"),
        "un fond noir et une image ne se confondent pas"
    );
}

// ---------------------------------------------------------------------------
// 3 — la composition
// ---------------------------------------------------------------------------

#[test]
fn une_composition_neuve_n_a_que_son_fond() {
    // « Un **fond** et jusqu'à **quatre champs** » : zéro champ est le cas de départ, et c'est
    // celui du critère d'acceptation n° 1 — « une composition sans aucun champ rend **exactement**
    // ce que `screen image` produit aujourd'hui ». Ce qui se vérifie ici, c'est la moitié pure :
    // rien n'est posé tant qu'on n'a rien posé.
    for fond in fonds_temoins() {
        let composition = Composition::nouvelle(fond.clone());
        assert_eq!(composition.fond(), &fond);
        assert!(
            composition.champs().is_empty(),
            "une composition neuve n'a aucun champ : {:?}",
            composition.champs()
        );
        for ancre in Ancre::TOUTES {
            assert!(composition.champ(ancre).is_none(), "{ancre:?} est vide");
        }

        let bloc = composition.encoder();
        let lignes: Vec<&str> = bloc.lines().collect();
        assert_eq!(
            lignes.len(),
            1,
            "sans champ, le bloc n'a que sa ligne de fond : {lignes:?}"
        );
    }

    // Changer le fond ne touche pas aux champs : ce sont deux commandes distinctes, et perdre les
    // champs en changeant de photo obligerait à tout reposer.
    let mut composition = Composition::nouvelle(Fond::Noir);
    composition
        .poser(Ancre::Haut, texte("Bonjour"))
        .expect("un premier champ tient");
    composition.changer_fond(Fond::Image(CHEMIN.to_owned()));
    assert_eq!(composition.fond(), &Fond::Image(CHEMIN.to_owned()));
    assert_eq!(
        composition.champ(Ancre::Haut),
        Some(&texte("Bonjour")),
        "changer de fond ne retire pas les champs"
    );
}

#[test]
fn les_champs_sortent_dans_l_ordre_des_ancres_jamais_dans_l_ordre_de_pose() {
    // Point n° 3 des conventions tranchées en tête, et contrat — `Composition::champs` : « Les
    // champs posés, **dans l'ordre de `Ancre::TOUTES`** — jamais dans l'ordre de pose. »
    //
    // Sans cet ordre, deux compositions identiques à l'œil s'écriraient différemment dans
    // `ecran.conf` selon l'ordre des commandes tapées : le fichier changerait sans que rien n'ait
    // changé, et « deux recompositions donnent le même tampon » (critère 12) dépendrait d'un
    // historique.
    let mut a_l_envers = Composition::nouvelle(Fond::Noir);
    for ancre in [Ancre::Centre, Ancre::Droite, Ancre::Bas, Ancre::Haut] {
        a_l_envers
            .poser(ancre, texte(ancre.slug()))
            .expect("quatre champs tiennent");
    }

    let mut a_l_endroit = Composition::nouvelle(Fond::Noir);
    for ancre in [Ancre::Haut, Ancre::Bas, Ancre::Droite, Ancre::Centre] {
        a_l_endroit
            .poser(ancre, texte(ancre.slug()))
            .expect("quatre champs tiennent");
    }

    let rangs: Vec<usize> = a_l_envers
        .champs()
        .into_iter()
        .map(|(ancre, _)| rang(ancre))
        .collect();
    let mut tries = rangs.clone();
    tries.sort_unstable();
    assert_eq!(
        rangs, tries,
        "les champs sortent dans l'ordre de Ancre::TOUTES, pas dans celui des commandes"
    );

    assert_eq!(
        a_l_envers, a_l_endroit,
        "deux compositions portant les mêmes champs sont égales, quel que soit l'ordre de pose"
    );
    assert_eq!(
        a_l_envers.encoder(),
        a_l_endroit.encoder(),
        "et elles s'écrivent pareil : un fichier qui changerait selon l'ordre des commandes se \
         réécrirait sans qu'aucun réglage ait bougé"
    );
}

#[test]
fn un_cinquieme_champ_est_refuse_en_le_disant() {
    // Test d'intention n° 9 de l'issue, et critère d'acceptation : « un cinquième champ est refusé
    // **en le disant** ».
    //
    // L'issue en donne la raison, qui n'est pas une limite technique : « la dalle fait 6 cm et le
    // cadran actuel occupe déjà tout l'écran pour **une** valeur : au-delà, on écrit ce que
    // personne ne lit ». Un refus muet ferait croire à un bug ; le refus doit nommer le plafond.
    assert_eq!(Composition::CHAMPS_MAX, 4);
    assert!(
        Composition::CHAMPS_MAX < Ancre::TOUTES.len(),
        "le plafond doit être plus bas que le nombre d'ancres, sans quoi il ne se rencontre jamais"
    );

    let mut composition = Composition::nouvelle(Fond::Noir);
    for ancre in [Ancre::Haut, Ancre::Bas, Ancre::Gauche, Ancre::Droite] {
        composition
            .poser(ancre, texte(ancre.slug()))
            .unwrap_or_else(|erreur| panic!("{ancre:?} est dans les quatre premiers : {erreur}"));
    }

    let erreur = composition
        .poser(Ancre::Centre, texte("de trop"))
        .expect_err("le cinquième champ doit être refusé");
    assert_eq!(
        erreur,
        TropDeChamps {
            plafond: Composition::CHAMPS_MAX
        }
    );
    let message = erreur.to_string();
    assert!(
        message.contains(&Composition::CHAMPS_MAX.to_string()),
        "le refus doit dire le plafond — « trop de champs » sans le chiffre fait tâtonner. \
         Obtenu : « {message} »"
    );
    let _: &dyn std::error::Error = &erreur;

    // Et le refus ne laisse rien derrière lui : un cinquième champ à moitié posé serait pire que
    // refusé, il serait invisible dans `champs()` et présent dans le compte.
    assert!(
        composition.champ(Ancre::Centre).is_none(),
        "un champ refusé n'est pas posé"
    );
    assert_eq!(
        composition.champs().len(),
        Composition::CHAMPS_MAX,
        "le refus n'a pas changé le compte"
    );

    // Vider une ancre rouvre la place : le plafond compte les champs, pas les commandes.
    assert!(composition.vider(Ancre::Droite), "un champ y était");
    composition
        .poser(Ancre::Centre, texte("désormais possible"))
        .expect("une place a été libérée");
}

#[test]
fn poser_sur_une_ancre_occupee_remplace_sans_compter_pour_un_cinquieme() {
    // Piège n° 3 du préambule, et contrat — `Composition::poser` : « Pose, ou **remplace** si
    // l'ancre porte déjà un champ. Refuse le cinquième — remplacer un champ existant n'est jamais
    // un cinquième. »
    //
    // Un plafond qui compterait les poses se fermerait tout seul : après quatre champs, corriger le
    // libellé de l'un d'eux serait refusé, et il faudrait le vider pour le reposer. C'est la faute
    // que ce test existe pour attraper.
    let mut composition = Composition::nouvelle(Fond::Noir);
    for ancre in [Ancre::Haut, Ancre::Bas, Ancre::Gauche, Ancre::Droite] {
        composition
            .poser(ancre, texte("avant"))
            .expect("quatre champs tiennent");
    }

    for ancre in [Ancre::Haut, Ancre::Bas, Ancre::Gauche, Ancre::Droite] {
        composition
            .poser(ancre, temperature(SONDE, Some(LIBELLE)))
            .unwrap_or_else(|erreur| {
                panic!("remplacer {ancre:?} n'est pas un cinquième champ : {erreur}")
            });
        assert_eq!(
            composition.champ(ancre),
            Some(&temperature(SONDE, Some(LIBELLE))),
            "{ancre:?} porte désormais la nouvelle source"
        );
        assert_eq!(
            composition.champs().len(),
            4,
            "un remplacement ne fait pas grandir le compte"
        );
    }
}

#[test]
fn vider_dit_si_un_champ_y_etait() {
    // Comportement attendu : « `screen layout vide <ancre>` retirer ce champ ». Le booléen n'est pas
    // décoratif : c'est ce qui permet au démon de ne pas réécrire `ecran.conf` — donc de ne pas
    // repousser 1,2 Mo sur la dalle — quand la commande n'a rien changé.
    let mut composition = Composition::nouvelle(Fond::Noir);
    assert!(
        !composition.vider(Ancre::Haut),
        "vider une ancre libre ne retire rien"
    );

    composition
        .poser(Ancre::Haut, texte("Bonjour"))
        .expect("un premier champ tient");
    assert!(composition.vider(Ancre::Haut), "un champ y était");
    assert!(composition.champ(Ancre::Haut).is_none());
    assert!(
        !composition.vider(Ancre::Haut),
        "vider deux fois ne retire qu'une fois"
    );
}

#[test]
fn une_composition_s_encode_et_se_decode_sans_perte() {
    // Test d'intention n° 5 de l'issue : « Une composition s'encode et se décode sans perte, texte
    // à espaces et accents compris », et critère d'acceptation : « une composition survit au
    // redémarrage du démon ».
    //
    // Le mode de défaillance n'est pas une erreur, c'est un état plausible et faux : un libellé
    // tronqué au premier blanc, un champ perdu, un fond noir devenu image. Rien de tout cela ne se
    // signale — la dalle affiche simplement autre chose au redémarrage suivant.
    for composition in compositions_temoins() {
        let bloc = composition.encoder();
        assert!(
            bloc.ends_with('\n'),
            "le bloc se termine par un saut de ligne — il est concaténé à `ecran.conf`, et une \
             ligne collée à la suivante rendrait les deux illisibles : {bloc:?}"
        );
        assert_eq!(
            Composition::decoder(&bloc),
            Ok(composition.clone()),
            "aller-retour de {composition:?} par :\n{bloc}"
        );

        // La forme du bloc : « fond … » d'abord, puis une ligne « champ <ancre> <source> » par
        // champ, dans l'ordre des ancres. C'est ce qui donne aux tests de refus le droit de
        // désigner « la ligne du fond » et « la ligne de tel champ ».
        let lignes: Vec<&str> = bloc.lines().collect();
        assert_eq!(
            lignes.len(),
            1 + composition.champs().len(),
            "une ligne de fond et une par champ : {lignes:?}"
        );
        assert_eq!(
            lignes[0],
            format!("fond {}", composition.fond().encoder()),
            "la première ligne est celle du fond"
        );
        for (ligne, (ancre, source)) in lignes[1..].iter().zip(composition.champs()) {
            assert_eq!(
                *ligne,
                format!("champ {} {}", ancre.slug(), source.encoder()),
                "chaque champ s'écrit « champ <ancre> <source> »"
            );
        }
    }

    // Deux compositions différentes ne s'écrivent pas pareil — sans quoi l'aller-retour ne
    // prouverait rien : un encodeur qui perdrait les champs passerait tous les tests où le
    // décodeur les réinvente.
    let temoins = compositions_temoins();
    for (rang_une, une) in temoins.iter().enumerate() {
        for autre in &temoins[rang_une + 1..] {
            if une != autre {
                assert_ne!(
                    une.encoder(),
                    autre.encoder(),
                    "{une:?} et {autre:?} sont deux compositions différentes"
                );
            }
        }
    }
}

#[test]
fn un_bloc_de_composition_abime_est_refuse_en_nommant_la_ligne() {
    // Test d'intention n° 7 de l'issue : « Un `ecran.conf` dont une entrée de champ est répétée est
    // refusé en nommant l'ancre », et critère d'acceptation : « une entrée de composition absente,
    // répétée ou aberrante dans le fichier est refusée **en la nommant** ».
    //
    // Piège n° 4 du préambule : c'est ce qui rend un fichier tronqué **détectable**. Une entrée
    // manquante complétée au jugé donne une composition plausible et fausse, rejouée à chaque
    // démarrage — la panne de #69, en plus discret.
    let mut composition = Composition::nouvelle(Fond::Image(CHEMIN.to_owned()));
    composition
        .poser(Ancre::Haut, temperature(SONDE, Some(LIBELLE)))
        .expect("premier champ");
    composition
        .poser(Ancre::Bas, texte("soirée d'été"))
        .expect("deuxième champ");
    let bloc = composition.encoder();
    let lignes: Vec<String> = bloc.lines().map(str::to_owned).collect();
    assert_eq!(lignes.len(), 3, "le bloc témoin a trois lignes : {bloc}");

    let mut raisons = Vec::new();

    // a — la ligne du fond manque. C'est la ligne 1 qu'il faut ajouter, et nommer la première
    // ligne **présente** enverrait corriger une ligne correcte.
    let sans_fond = format!("{}\n{}\n", lignes[1], lignes[2]);
    let erreur = refus_de_composition(&sans_fond);
    assert_eq!(
        erreur.ligne, 1,
        "le fond manque, et c'est la ligne 1 du bloc. Obtenu : {erreur}"
    );
    raisons.push(erreur.raison);

    // b — un bloc vide n'a même pas sa ligne de fond.
    for vide in ["", "\n", "   \n"] {
        let erreur = refus_de_composition(vide);
        assert_eq!(erreur.ligne, 1, "un bloc vide manque de sa ligne 1");
    }

    // c — la ligne du fond, répétée. Deux fonds, c'est un fichier de deux versions concaténées : le
    // second gagnerait en silence, et la photo affichée ne serait pas celle du fichier lu.
    let deux_fonds = format!(
        "{}\n{}\n{}\n{}\n",
        lignes[0], lignes[0], lignes[1], lignes[2]
    );
    let erreur = refus_de_composition(&deux_fonds);
    assert_eq!(erreur.ligne, 2, "le fond en trop est à la ligne 2");
    assert!(
        erreur.raison.contains("fond"),
        "le refus nomme l'entrée répétée. Obtenu : {}",
        erreur.raison
    );
    raisons.push(erreur.raison);

    // d — une ancre répétée, le cas nommé par le test d'intention n° 7. Deux `champ haut` ne se
    // départagent pas : le dernier gagnerait, et l'utilisateur verrait un champ qu'il croit avoir
    // remplacé.
    let ancre_repetee = format!(
        "{}\n{}\n{}\n{}\n",
        lignes[0], lignes[1], lignes[2], lignes[1]
    );
    let erreur = refus_de_composition(&ancre_repetee);
    assert_eq!(erreur.ligne, 4, "la répétition est à la ligne 4");
    assert!(
        erreur.raison.contains(Ancre::Haut.slug()),
        "le refus nomme **l'ancre** répétée — c'est elle qu'on va corriger dans le fichier. \
         Obtenu : {}",
        erreur.raison
    );
    raisons.push(erreur.raison);

    // e — une ancre inconnue, en plein fichier. Même exigence qu'au socket : la liste des cinq.
    let ancre_inconnue = format!("{}\nchamp milieu texte Bonjour\n{}\n", lignes[0], lignes[2]);
    let erreur = refus_de_composition(&ancre_inconnue);
    assert_eq!(erreur.ligne, 2);
    assert!(
        erreur.raison.contains("milieu"),
        "le refus nomme l'ancre fautive. Obtenu : {}",
        erreur.raison
    );
    raisons.push(erreur.raison);

    // f — des entrées aberrantes : mot-clé inconnu, source inconnue, champ sans source, ligne
    // tronquée.
    for (rang_ligne, aberrante) in [
        (2, "bidule haut texte Bonjour"),
        (2, "champ"),
        (2, "champ haut"),
        (2, "champ haut bidule Bonjour"),
        (2, "champ haut texte"),
        (2, "champ haut temp"),
        (2, "champs haut texte Bonjour"),
        (1, "fond"),
        (1, "fond bidule"),
        (1, "champ haut texte Bonjour"),
    ] {
        let mut abime = lignes.clone();
        abime[rang_ligne - 1] = aberrante.to_owned();
        let bloc = format!("{}\n", abime.join("\n"));
        let erreur = refus_de_composition(&bloc);
        assert_eq!(
            erreur.ligne, rang_ligne,
            "« {aberrante} » est à la ligne {rang_ligne} : c'est celle que le message doit nommer. \
             Obtenu : {erreur}"
        );
        raisons.push(erreur.raison);
    }

    // g — cinq champs dans le fichier. Le plafond ne se contourne pas en éditant `ecran.conf` à la
    // main : le démon repartirait sur une composition qu'aucune commande ne sait produire.
    let mut cinq = vec![lignes[0].clone()];
    for ancre in Ancre::TOUTES {
        cinq.push(format!("champ {} texte {}", ancre.slug(), ancre.slug()));
    }
    let erreur = refus_de_composition(&format!("{}\n", cinq.join("\n")));
    assert!(
        erreur.raison.contains(&Composition::CHAMPS_MAX.to_string()),
        "le refus dit le plafond. Obtenu : {}",
        erreur.raison
    );
    raisons.push(erreur.raison);

    // Les refus distinguent les fautes : une entrée manquante, une entrée répétée, une ancre
    // inconnue et un mot-clé inconnu ne se corrigent pas de la même façon, et une phrase unique
    // laisserait tout le diagnostic à faire alors que le fichier est sous les yeux.
    let familles: BTreeSet<String> = raisons.into_iter().collect();
    assert!(
        familles.len() >= 4,
        "quatre familles de fautes ne peuvent pas partager une seule explication : {familles:?}"
    );
}

// ---------------------------------------------------------------------------
// 4 — le protocole : les commandes
// ---------------------------------------------------------------------------

#[test]
fn les_cinq_commandes_de_layout_font_l_aller_retour_sans_rien_perdre() {
    // Comportement attendu de l'issue, les six lignes du tableau des commandes. Une commande par
    // changement, chacune avec **au plus un champ libre en fin de ligne** — « la convention du
    // protocole depuis les profils (#74). Une seule ligne qui porterait à la fois un chemin et un
    // texte libre serait ambiguë au premier espace. »
    let mut temoins = vec![
        Request::Screen(ScreenAction::LayoutOff),
        Request::Screen(ScreenAction::LayoutState),
    ];
    for fond in fonds_temoins() {
        temoins.push(Request::Screen(ScreenAction::LayoutFond(fond)));
    }
    for ancre in Ancre::TOUTES {
        temoins.push(Request::Screen(ScreenAction::LayoutVide(ancre)));
        for source in sources_temoins() {
            temoins.push(Request::Screen(ScreenAction::LayoutChamp(ancre, source)));
        }
    }

    for temoin in &temoins {
        let encodee = encode_request(temoin);
        assert!(
            !encodee.contains('\n') && !encodee.contains('\r'),
            "une requête tient sur une seule ligne physique : « {encodee} »"
        );
        assert_eq!(
            parse_request(&encodee),
            Ok(temoin.clone()),
            "aller-retour de {temoin:?} par « {encodee} » — rien ne se perd en route"
        );
    }

    // La forme exacte du fil. Sans ces égalités, un encodeur pourrait écrire `screen compo …` et
    // tous les allers-retours resteraient verts : le décodeur rattraperait sa propre écriture, et
    // `reverb screen` d'une autre version ne parlerait plus au démon.
    assert_eq!(
        encode_request(&Request::Screen(ScreenAction::LayoutFond(Fond::Noir))),
        "screen layout fond noir"
    );
    assert_eq!(
        encode_request(&Request::Screen(ScreenAction::LayoutFond(Fond::Image(
            CHEMIN.to_owned()
        )))),
        format!("screen layout fond image {CHEMIN}")
    );
    assert_eq!(
        encode_request(&Request::Screen(ScreenAction::LayoutChamp(
            Ancre::Haut,
            temperature(SONDE, None)
        ))),
        format!("screen layout champ haut temp {SONDE}")
    );
    assert_eq!(
        encode_request(&Request::Screen(ScreenAction::LayoutChamp(
            Ancre::Centre,
            texte(LIBELLE_A_ESPACE)
        ))),
        format!("screen layout champ centre texte {LIBELLE_A_ESPACE}")
    );
    assert_eq!(
        encode_request(&Request::Screen(ScreenAction::LayoutVide(Ancre::Bas))),
        "screen layout vide bas"
    );
    assert_eq!(
        encode_request(&Request::Screen(ScreenAction::LayoutOff)),
        "screen layout off"
    );
    assert_eq!(
        encode_request(&Request::Screen(ScreenAction::LayoutState)),
        "screen layout"
    );

    // Le fil se compose des mêmes encodeurs que le fichier : une forme écrite deux fois divergerait
    // à la première correction.
    for ancre in Ancre::TOUTES {
        for source in sources_temoins() {
            assert_eq!(
                encode_request(&Request::Screen(ScreenAction::LayoutChamp(
                    ancre,
                    source.clone()
                ))),
                format!("screen layout champ {} {}", ancre.slug(), source.encoder()),
                "la commande d'un champ est « screen layout champ <ancre> <source> »"
            );
        }
    }

    // Les cinq actions sont cinq commandes distinctes.
    let toutes = [
        "screen layout fond noir",
        "screen layout champ haut texte a",
        "screen layout vide haut",
        "screen layout off",
        "screen layout",
    ];
    for (rang_une, ligne) in toutes.iter().enumerate() {
        for autre in &toutes[rang_une + 1..] {
            assert_ne!(
                parse_request(ligne),
                parse_request(autre),
                "« {ligne} » et « {autre} » sont deux commandes différentes"
            );
        }
    }

    // Et un libellé à espaces traverse intact : c'est le dernier champ de sa ligne.
    assert_eq!(
        action_de(&format!(
            "screen layout champ centre texte {LIBELLE_A_ESPACE}"
        )),
        ScreenAction::LayoutChamp(Ancre::Centre, texte(LIBELLE_A_ESPACE))
    );
    assert_eq!(
        action_de(&format!(
            "screen layout champ haut temp {SONDE} {LIBELLE_A_ESPACE}"
        )),
        ScreenAction::LayoutChamp(Ancre::Haut, temperature(SONDE, Some(LIBELLE_A_ESPACE))),
        "la sonde est un jeton, le libellé prend tout le reste de la ligne"
    );
    assert_eq!(
        action_de(&format!("screen layout fond image {CHEMIN_A_ESPACE}")),
        ScreenAction::LayoutFond(Fond::Image(CHEMIN_A_ESPACE.to_owned())),
        "un chemin coupé au premier blanc désigne un autre fichier, et le démon l'afficherait \
         sans un mot"
    );
}

#[test]
fn une_commande_de_layout_mal_formee_est_refusee_en_nommant_screen() {
    // Le verbe reçu est `screen` : il existe, ce sont ses arguments qui sont mauvais. Même
    // arbitrage que #23, #29 et #33.
    let mut raisons = Vec::new();
    for ligne in [
        "screen layout bidule",
        "screen layout fond",
        "screen layout fond ",
        "screen layout fond bidule",
        "screen layout fond noir /a.png",
        "screen layout fond image",
        "screen layout champ",
        "screen layout champ haut",
        "screen layout champ haut bidule Bonjour",
        "screen layout champ haut texte",
        "screen layout champ haut temp",
        "screen layout vide",
        "screen layout vide haut bas",
        "screen layout off 1",
        "screen layout off off",
        "screen layout 1",
        "screen layout Off",
        "screen layout FOND noir",
        "screen layouts",
    ] {
        raisons.push(refus_de_screen(ligne));
    }

    // Les refus ne se ramènent pas tous à la même phrase : une action inconnue, un argument
    // manquant et un argument de trop ne se corrigent pas de la même façon.
    let familles: BTreeSet<String> = raisons.into_iter().collect();
    assert!(
        familles.len() >= 3,
        "trois familles de fautes ne peuvent pas partager une seule explication : {familles:?}"
    );

    // Une ancre inconnue, sur le socket cette fois : critère d'acceptation « une ancre inconnue est
    // refusée **en donnant la liste** des cinq ».
    for ligne in [
        "screen layout champ milieu texte Bonjour",
        "screen layout champ haut-gauche temp k10temp:tctl",
        "screen layout vide milieu",
    ] {
        let raison = refus_de_screen(ligne);
        for nom in SLUGS {
            assert!(
                raison.contains(nom),
                "« {ligne} » doit être refusée en donnant les cinq ancres, il manque « {nom} ». \
                 Raison obtenue : {raison}"
            );
        }
    }
}

#[test]
fn un_fond_relatif_est_refuse_en_le_disant() {
    // Point n° 1 des conventions tranchées en tête de fichier. Même règle que `screen image` (#33)
    // et même raison : « le démon lit sous son propre répertoire courant, qui n'est pas celui de
    // son client ».
    //
    // Le mode de défaillance est muet et déroutant : `screen layout fond image fond.png` lancé
    // depuis `~/images` ferait chercher au démon `/fond.png` — et un message « fichier
    // introuvable » enverrait l'utilisateur vérifier un fichier qu'il a sous les yeux.
    for brut in [
        "fond.png",
        "./fond.png",
        "../fond.png",
        "images/fond.png",
        "~/fond.png",
        ".",
    ] {
        let ligne = format!("screen layout fond image {brut}");
        let raison = refus_de_screen(&ligne);
        let bas = raison.to_lowercase();
        assert!(
            bas.contains("absolu") || bas.contains("relatif"),
            "« {ligne} » doit être refusée **en le disant** : sans le mot, le message se confond \
             avec « fichier introuvable » et envoie chercher la faute ailleurs. Raison obtenue : \
             {raison}"
        );
    }

    // Et l'absolu passe : c'est au démon de dire si le fichier existe, pas au protocole de deviner.
    for brut in ["/", "/a", "/./fond.png", "/home/nico/Mes documents/a.png"] {
        assert_eq!(
            action_de(&format!("screen layout fond image {brut}")),
            ScreenAction::LayoutFond(Fond::Image(brut.to_owned()))
        );
    }
}

#[test]
fn aucun_champ_libre_de_layout_ne_peut_se_faire_passer_pour_deux() {
    // En-tête du module `ipc` : le préfixe de type n'assure rien contre un saut de ligne **à
    // l'intérieur** d'un champ. Un libellé est tapé par un humain, un chemin vient du disque : ni
    // l'un ni l'autre n'est écrit par le protocole, et
    // `screen layout champ haut texte $'a\nlight all ffffff'` ne doit pas allumer le boîtier.
    //
    // ⚠️ Ce test n'exige **pas** qu'un champ hostile soit accepté. Le refuser est une issue
    // parfaitement bonne, et c'est même celle qu'impose le point n° 4 du préambule pour un libellé
    // fait de blancs. Ce qu'il exige, c'est que le refus soit un refus : jamais une commande
    // scindée en deux, jamais une action changée, jamais une ancre déplacée. C'est le tout ou rien
    // qui compte, pas le tour.
    for hostile in HOSTILES {
        let chemin = format!("/tmp/{hostile}");
        for requete in [
            Request::Screen(ScreenAction::LayoutFond(Fond::Image(chemin))),
            Request::Screen(ScreenAction::LayoutChamp(Ancre::Haut, texte(hostile))),
            Request::Screen(ScreenAction::LayoutChamp(
                Ancre::Centre,
                temperature(SONDE, Some(hostile)),
            )),
            Request::Screen(ScreenAction::LayoutChamp(
                Ancre::Bas,
                temperature(hostile, None),
            )),
        ] {
            let encodee = encode_request(&requete);
            assert_eq!(
                encodee.lines().count(),
                1,
                "une requête tient sur une seule ligne, quel que soit son champ libre : \
                 « {encodee} »"
            );
            assert!(
                !encodee.contains('\n') && !encodee.contains('\r'),
                "aucun saut de ligne dans une requête encodée : « {encodee} »"
            );

            let Ok(relue) = parse_request(&encodee) else {
                // Refusée : rien n'a été exécuté, et c'est une fin acceptable.
                continue;
            };
            let Request::Screen(action) = &relue else {
                panic!("« {encodee} » se relit en {relue:?} — le champ hostile a changé de verbe");
            };
            let Request::Screen(posee) = &requete else {
                unreachable!("les témoins sont des commandes d'écran");
            };
            assert_eq!(
                std::mem::discriminant(action),
                std::mem::discriminant(posee),
                "« {encodee} » se relit en {action:?} — le champ hostile a changé l'action"
            );

            // L'ancre, elle, doit traverser intacte : c'est un jeton du protocole, pas un champ
            // libre, et un champ posé sur la mauvaise ancre est un champ qui écrase son voisin.
            match action {
                ScreenAction::LayoutChamp(ancre, source) => {
                    assert_eq!(
                        *ancre,
                        match posee {
                            ScreenAction::LayoutChamp(attendue, _) => *attendue,
                            _ => unreachable!(),
                        },
                        "l'ancre traverse le champ hostile intacte : « {encodee} »"
                    );
                    if let Source::Temperature { sonde, .. } = source {
                        assert!(
                            !sonde.is_empty()
                                && !sonde.chars().any(|c| c.is_whitespace() || c.is_control()),
                            "la sonde « {sonde} » se ferait prendre pour deux champs après \
                             relecture de « {encodee} »"
                        );
                    }
                }
                ScreenAction::LayoutFond(Fond::Image(lu)) => {
                    assert!(
                        !lu.chars().any(char::is_control),
                        "le chemin « {lu} » porte un caractère de contrôle après relecture de \
                         « {encodee} » — il casserait le cadrage de la réponse suivante"
                    );
                    assert!(
                        lu.starts_with('/'),
                        "un chemin absolu le reste après encodage : « {lu} »"
                    );
                }
                autre => panic!("{autre:?} n'est pas une des actions à champ libre"),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — le protocole : les lignes de réponse
// ---------------------------------------------------------------------------

#[test]
fn les_deux_lignes_de_reponse_du_layout_font_l_aller_retour() {
    // Comportement attendu : « `screen layout` l'état courant ». C'est par ces lignes que la
    // fenêtre (#76) et `reverb screen` apprendront ce que la dalle compose. Un champ perdu en route
    // ferait afficher une composition qui n'est pas celle du démon.
    let mut lignes = Vec::new();
    for fond in fonds_temoins() {
        lignes.push(ResponseLine::Layout {
            fond: fond.encoder(),
        });
    }
    for ancre in Ancre::TOUTES {
        for source in sources_temoins() {
            lignes.push(ResponseLine::LayoutChamp {
                ancre: ancre.slug().to_owned(),
                source: source.encoder(),
            });
        }
    }

    for ligne in &lignes {
        ne_termine_jamais(ligne);
        let encodee = encode_response_line(ligne);
        assert_eq!(
            parse_response_line(&encodee).as_ref(),
            Ok(ligne),
            "aller-retour de {ligne:?} par « {encodee} »"
        );
    }

    // La forme exacte, une fois chacune.
    assert_eq!(
        encode_response_line(&ResponseLine::Layout {
            fond: "noir".to_owned()
        }),
        "layout noir"
    );
    assert_eq!(
        encode_response_line(&ResponseLine::Layout {
            fond: format!("image {CHEMIN_A_ESPACE}")
        }),
        format!("layout image {CHEMIN_A_ESPACE}")
    );
    assert_eq!(
        encode_response_line(&ResponseLine::LayoutChamp {
            ancre: "haut".to_owned(),
            source: format!("temp {SONDE} {LIBELLE}")
        }),
        format!("layout-champ haut temp {SONDE} {LIBELLE}")
    );

    // Point n° 5 du préambule : `layout-champ` ne se lit pas comme un `layout` dont le fond
    // commencerait par « -champ ». Les deux lignes partagent leur préfixe, et un découpage trop
    // souple les confondrait — la fenêtre afficherait un fond nommé « -champ haut … ».
    assert_eq!(
        parse_response_line("layout-champ haut texte Bonjour"),
        Ok(ResponseLine::LayoutChamp {
            ancre: "haut".to_owned(),
            source: "texte Bonjour".to_owned()
        })
    );
    assert_eq!(
        parse_response_line("layout noir"),
        Ok(ResponseLine::Layout {
            fond: "noir".to_owned()
        })
    );

    // Une ligne mal formée est refusée en portant la ligne fautive, pour qu'on la retrouve dans un
    // journal.
    for inconnue in [
        "layout",
        "layout ",
        "layout-champ",
        "layout-champ haut",
        "layout-champ ",
        "layouts noir",
        "Layout noir",
    ] {
        let erreur = parse_response_line(inconnue)
            .err()
            .unwrap_or_else(|| panic!("« {inconnue} » n'est pas une ligne de réponse valide"));
        assert_eq!(
            erreur.line, inconnue,
            "le refus porte la ligne fautive, pour qu'on la retrouve dans un journal"
        );
        assert!(
            !erreur.reason.trim().is_empty(),
            "« {inconnue} » doit être refusée en disant pourquoi"
        );
    }

    // Les champs hostiles ne cassent pas le cadrage — même exigence qu'à #29 et #33, le champ vide
    // compris.
    //
    // ⚠️ Comme du côté des requêtes, un champ hostile a le droit d'être **refusé** à la relecture :
    // le démon n'encode jamais qu'un `Fond::encoder()` ou un `Source::encoder()`, et une ligne
    // blanche n'en sort pas. Ce qui n'a jamais le droit d'arriver, c'est qu'elle se scinde, qu'elle
    // termine la réponse, ou qu'elle change de type en chemin — un client tronquerait alors sa
    // lecture au beau milieu d'une composition, sans rien avoir à signaler.
    let mut hostiles: Vec<&str> = HOSTILES.to_vec();
    hostiles.push("");
    for hostile in hostiles {
        for ligne in [
            ResponseLine::Layout {
                fond: hostile.to_owned(),
            },
            ResponseLine::LayoutChamp {
                ancre: "haut".to_owned(),
                source: hostile.to_owned(),
            },
        ] {
            ne_termine_jamais(&ligne);
            let encodee = encode_response_line(&ligne);
            if let Ok(relue) = parse_response_line(&encodee) {
                assert_eq!(
                    std::mem::discriminant(&relue),
                    std::mem::discriminant(&ligne),
                    "« {encodee} » se relit en {relue:?} : le type de ligne a changé"
                );
            }
        }
    }
}

#[test]
fn l_affichage_d_une_composition_est_le_jeton_layout() {
    // Contrat : « `ResponseLine::Screen { luminosite, affichage }` existe déjà ; l'`affichage` d'une
    // composition est le **jeton unique** `layout`. »
    //
    // Un jeton, et non `layout:<chemin>` comme `image:` : le fond et les champs ont leurs propres
    // lignes, et les répéter sur celle-ci ferait deux sources de vérité pour la même chose.
    let ligne = ResponseLine::Screen {
        luminosite: 50,
        affichage: "layout".to_owned(),
    };
    assert_eq!(encode_response_line(&ligne), "screen 50 layout");
    assert_eq!(parse_response_line("screen 50 layout"), Ok(ligne));

    // Et il ne se confond pas avec la ligne `layout`, qui commence par le même mot.
    assert_ne!(
        parse_response_line("screen 50 layout"),
        parse_response_line("layout noir"),
        "« screen 50 layout » est l'état de la dalle, « layout noir » est le fond de sa composition"
    );
}

#[test]
fn les_commandes_d_ecran_d_avant_traversent_toujours() {
    // Un mot de plus derrière `screen` ne doit rien coûter aux six actions de #33. `layout` partage
    // son verbe avec elles, et un découpage trop souple ferait lire `screen layout` comme un
    // `screen <action inconnue>` — ou pire, ferait passer un chemin pour une ancre.
    assert_eq!(
        parse_request("screen state"),
        Ok(Request::Screen(ScreenAction::State))
    );
    assert_eq!(
        parse_request("screen off"),
        Ok(Request::Screen(ScreenAction::Off))
    );
    assert_eq!(
        parse_request("screen brightness 42"),
        Ok(Request::Screen(ScreenAction::Brightness(42)))
    );
    assert_eq!(
        parse_request(&format!("screen image {CHEMIN}")),
        Ok(Request::Screen(ScreenAction::Image(CHEMIN.to_owned())))
    );
    assert_eq!(
        parse_request(&format!("screen gif {CHEMIN}")),
        Ok(Request::Screen(ScreenAction::Gif(CHEMIN.to_owned())))
    );
    assert_eq!(
        parse_request(&format!("screen gauge {SONDE}")),
        Ok(Request::Screen(ScreenAction::Gauge(SONDE.to_owned())))
    );

    // `screen layout off` n'est pas `screen off` : l'un revient à l'affichage simple, l'autre rend
    // la dalle au firmware. Les confondre éteindrait l'écran de qui voulait seulement retirer sa
    // composition.
    assert_ne!(
        parse_request("screen layout off"),
        parse_request("screen off"),
        "`screen layout off` revient à l'affichage simple, `screen off` rend la dalle au firmware"
    );

    // Et les verbes des autres issues continuent de passer.
    assert!(parse_request("status").is_ok());
    assert!(parse_request("zone list").is_ok());
    assert!(parse_request("light all ff2080").is_ok());
    assert_eq!(parse_response_line("end"), Ok(ResponseLine::End));
}
