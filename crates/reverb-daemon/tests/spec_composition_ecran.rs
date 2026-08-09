//! Tests d'intention de la composition de l'écran — issue #80, côté **rendu et persistance**.
//!
//! Écrits **avant** l'implémentation, depuis l'issue #80 seule et le contrat public qu'elle fixe.
//! Rien de `crates/*/src/` n'a été lu pour les produire, hors les signatures publiques déjà
//! existantes de `reverb_daemon::ecran` (`Affichage`, `Dalle`, `Etat`, `charger`, `enregistrer`,
//! `verifier_format`, `verifier_fichier`) qu'il faut nommer pour que le fichier compile. Si l'un de
//! ces tests échoue après implémentation, c'est le code qu'on corrige, jamais le test.
//!
//! Le pendant pur — les ancres, les sources, le fond, le bloc de `ecran.conf` et le protocole —
//! vit dans `crates/reverb-proto/tests/spec_composition.rs`.
//!
//! ## Le fait neuf, et ce qu'il coûte
//!
//! ⚠️ **La dalle est ronde**, observé sur le matériel le 2026-08-08. Un champ dessiné hors du
//! disque visible ne remonte **aucune** erreur : le contrôleur avale ses 1 228 800 octets quels
//! qu'ils soient, et ce qui tombe hors de la vitre est simplement invisible. Un test qui se
//! contenterait de « ça n'a pas paniqué » laisserait passer exactement ça. D'où la forme des tests
//! de géométrie ici : ils **comparent au fond**, pixel par pixel, et exigent qu'aucun pixel
//! entièrement hors du disque n'ait bougé.
//!
//! ## Les quatre pièges que ce fichier garde
//!
//! 1. **Une composition sans champ doit être l'identité.** Le critère d'acceptation dit « octet
//!    pour octet », et c'est plus fort qu'« à peu près pareil » : une recopie qui passerait par une
//!    conversion de couleur, un arrondi ou une remise à l'échelle changerait l'image de fond sans
//!    qu'on le voie sur une dalle de six centimètres. La comparaison est donc faite sur les octets,
//!    pas sur l'apparence.
//! 2. **L'ordre des composantes est muet.** Le projet en mêle trois — LED en GRB, écran en
//!    [`screen::COMPONENT_ORDER`] c'est-à-dire BGR, RAM en RGB. Une composante permutée s'affiche
//!    sans erreur, nette, et de la mauvaise couleur. Ce fichier ne suppose donc jamais quel octet
//!    d'un pixel porte le rouge : il compare au fond, ou raisonne sur la **moyenne** des trois
//!    octets, qui est invariante par permutation.
//! 3. **Une sonde muette qui affiche « 0 » est rassurante et fausse.** Même règle que le cadran de
//!    #33 : un 34 °C figé derrière une pompe arrêtée, c'est un circuit qui chauffe sans que rien ne
//!    le signale. Les tirets sont exigés, et l'absence de tout chiffre avec eux.
//! 4. **Un rendu qui dépend d'un état invisible fait pousser 1,2 Mo pour rien.** Le démon
//!    recompose toutes les deux secondes et n'envoie que si le tampon a changé : deux rendus sur
//!    les mêmes valeurs doivent être **égaux octet pour octet**, sans quoi la dalle reçoit une
//!    image toutes les deux secondes à température parfaitement stable.
//!
//! ## Quatre points que l'issue laisse ouverts, et que ces tests tranchent
//!
//! 1. **Un champ ne dessine que dans les régions de son ancre.** L'issue exige que la boîte
//!    tienne dans le disque ; c'est vide de sens si le rendu déborde d'elle. `Ancre::boite()` est
//!    donc tenue pour la boîte **englobante** du texte, contour ou plaque compris.
//!
//!    ⚠️ **Amendé par #90, et c'est une évolution de contrat, pas un assouplissement.** Un champ
//!    de température dessine désormais un **arc** sur la couronne du disque, hors de sa boîte.
//!    Englober l'arc dans la boîte était géométriquement impossible : les boîtes englobantes de
//!    deux secteurs voisins se recouvrent, ce qui aurait cassé le point n° 2 juste en dessous.
//!
//!    Ce que ce fichier vérifie est donc devenu : **un champ ne peint que dans sa boîte ou dans
//!    son propre secteur de couronne**. La raison d'être de la règle est intacte — deux champs
//!    voisins ne peuvent toujours pas s'effacer l'un l'autre, puisque leurs boîtes sont disjointes
//!    et leurs secteurs aussi (vérifié au pixel par `spec_police_arcs.rs`).
//! 2. **`Dalle::composee` ne dépend pas de l'ordre des champs qu'on lui passe.** Les cinq boîtes ne
//!    se recouvrent pas (vérifié côté `reverb-proto`) : deux tampons qui différeraient selon
//!    l'ordre trahiraient un débordement.
//! 3. **`valeur_du_champ` ne montre pas le libellé.** Le contrat dit « ce qu'un champ écrit **en
//!    gros** » : le libellé est ce qui légende la valeur, pas la valeur. Un `valeur_du_champ` qui
//!    concaténerait les deux ferait qu'un libellé rallongé rétrécirait le chiffre.
//! 4. **La ligne `affiche` d'une composition ne porte aucun chemin.** Le fond vit dans le bloc de
//!    composition ; le répéter sur la ligne d'affichage ferait deux sources de vérité pour la même
//!    photo, et un fichier édité à la main pourrait les rendre contradictoires.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **La cadence de recomposition** (2 s) et le **battement** (25 s) : ce sont des boucles de
//!   service, pas des fonctions. Ce qui s'en démontre ici, c'est leur condition — un rendu stable
//!   sur des valeurs stables.
//! - **La vigie de #70** — trois échecs d'affilée rendent la dalle au firmware. Elle vit dans la
//!   boucle d'envoi et le contrat de cette issue ne lui donne aucune couture publique ; elle reste
//!   couverte par ses propres tests, que ce chantier ne doit pas toucher.
//! - **L'exemple qui rend une composition dans un fichier** : c'est une cible `--example`, elle se
//!   vérifie en l'exécutant, pas depuis un test d'intégration.
//! - Aucune écriture matérielle, aucun accès à `/dev`, aucun démon lancé.

use std::fs;
use std::path::{Path, PathBuf};

use reverb_daemon::ecran::{
    Affichage, ChampRendu, Dalle, Etat, EtatInvalide, ImageInvalide, charger, enregistrer,
    valeur_du_champ, verifier_fichier, verifier_format,
};
use reverb_proto::composition::{self, Ancre, Composition, Fond, Source};
use reverb_proto::screen;

/// Ce pixel est-il dans le secteur de couronne de cette ancre ? (issue #90)
///
/// La couronne est l'anneau extérieur du disque, où les champs de température
/// dessinent leur arc. Aucune boîte d'ancre ne l'atteint : la plus lointaine
/// s'arrête à un rayon de 286,4 pour un bord intérieur de couronne à 292.
fn dans_son_secteur(ancre: Ancre, x: u32, y: u32) -> bool {
    let Some(secteur) = ancre.secteur() else {
        return false;
    };
    let centre = f32::from(screen::WIDTH) / 2.0;
    let dx = x as f32 + 0.5 - centre;
    let dy = y as f32 + 0.5 - centre;
    let rayon = (dx * dx + dy * dy).sqrt();
    // ⚠️ **Deux marges, et elles ne sont pas de la tolérance à l'aveugle** (#93).
    //
    // Un pixel en rayon : l'arc est anticrénelé depuis #93, et sa couverture
    // décroît sur un demi-pixel de part et d'autre du ruban. Sans cette marge,
    // le pixel (295, 4) — à 316,5 d'un bord posé à 316 — serait déclaré hors
    // secteur alors qu'il est le lissage du bord lui-même.
    //
    // Trois degrés en angle : les extrémités sont des demi-disques de rayon 12
    // centrés sur la ligne médiane, à 304 du centre. Ils débordent donc de
    // `asin(12 / 304) ≈ 2,26°` derrière chaque bout, plus l'anticrénelage. Vingt
    // degrés séparent deux secteurs voisins : la marge ne peut pas les faire se
    // toucher, et `spec_arcs_lisses.rs` le vérifie au pixel.
    const MARGE_RAYON: f32 = 1.0;
    const MARGE_ANGLE: f32 = 3.0;

    if rayon < f32::from(composition::COURONNE_RAYON_INTERIEUR) - MARGE_RAYON
        || rayon > f32::from(composition::COURONNE_RAYON_EXTERIEUR) + MARGE_RAYON
    {
        return false;
    }
    // Zéro au sommet, croissant dans le sens horaire — la convention de #90.
    let angle = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
    (angle - secteur.debut + MARGE_ANGLE).rem_euclid(360.0) < secteur.ouverture + 2.0 * MARGE_ANGLE
}

// ---------------------------------------------------------------------------
// Dimensions, couleurs et sources témoins
// ---------------------------------------------------------------------------

/// La largeur de la dalle. Reprise du protocole, jamais réécrite.
const LARGEUR: u32 = screen::WIDTH as u32;

/// La hauteur de la dalle.
const HAUTEUR: u32 = screen::HEIGHT as u32;

/// Le blanc, l'un des deux fonds que le critère de lisibilité nomme.
const BLANC: (u8, u8, u8) = (0xff, 0xff, 0xff);

/// Le noir, l'autre.
const NOIR: (u8, u8, u8) = (0x00, 0x00, 0x00);

/// La couleur témoin du projet. Ses trois composantes sont distinctes, donc aucune permutation ne
/// peut passer inaperçue.
const TEMOIN: (u8, u8, u8) = (0xff, 0x20, 0x80);

/// La sonde d'exemple de l'issue.
const SONDE: &str = "kraken2023elite:coolant-temp";

/// Un texte long, qui ne tient dans aucune des cinq boîtes — le cas du critère d'acceptation « un
/// texte trop large pour la corde disponible à son ancre est tronqué ou réduit, **jamais** dessiné
/// hors du disque ».
const TEXTE_TROP_LONG: &str = concat!(
    "un libellé démesurément long que personne n'écrirait, mais que rien ",
    "n'empêche de taper — avec des accents et des espaces, encore et encore"
);

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
/// Piège n° 2 du préambule — la moyenne est **invariante par permutation** des composantes. Ce
/// fichier n'a donc pas à savoir lequel des trois octets porte le rouge pour dire « sombre » ou
/// « clair », et il continuera de dire vrai le jour où la mire de #77 renverserait la conclusion
/// BGR.
fn clarte(dalle: &Dalle, x: u32, y: u32) -> u16 {
    let brut = triplet(dalle, x, y);
    (u16::from(brut[0]) + u16::from(brut[1]) + u16::from(brut[2])) / 3
}

/// Vrai si le carré du pixel (`x`, `y`) est **entièrement** hors du disque visible.
///
/// La distance retenue est celle du centre de la dalle au point du pixel qui lui est le plus
/// proche : si même celui-là est au-delà du rayon, aucun point du pixel n'est visible. La marge
/// d'un pixel ainsi laissée sur le bord est délibérée — ce test attrape un champ posé dans un coin,
/// pas un arrondi d'anticrénelage sur la limite de la vitre.
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

/// Les pixels que la composition a changés par rapport à son fond.
///
/// C'est la mesure de base de ce fichier : elle ne suppose rien du dessin — ni sa couleur, ni sa
/// police, ni sa forme —, seulement qu'il a lieu quelque part et pas ailleurs. Un test d'intention
/// ne fige pas un goût.
fn pixels_changes(fond: &Dalle, composee: &Dalle) -> Vec<(u32, u32)> {
    let avant = fond.octets().chunks_exact(screen::PIXEL_LEN);
    let apres = composee.octets().chunks_exact(screen::PIXEL_LEN);
    let mut changes = Vec::new();
    for (rang, (pixel_avant, pixel_apres)) in avant.zip(apres).enumerate() {
        if pixel_avant != pixel_apres {
            let rang = u32::try_from(rang).expect("un tampon de 640 × 640 tient dans un u32");
            changes.push((rang % LARGEUR, rang / LARGEUR));
        }
    }
    changes
}

/// Vérifie qu'une dalle a exactement la taille qu'attend le contrôleur.
///
/// Passe par `screen::check_image`, le validateur du protocole : une dalle trop courte est
/// **ignorée en silence** par le matériel (spec §2.2.1), et ce fichier n'a pas à réécrire la règle
/// que `reverb-proto` porte déjà.
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

/// Compose un champ unique à l'ancre donnée, et rend les positions qu'il a changées.
///
/// Chaque appel vérifie au passage les deux invariants qui valent pour **tout** champ : la dalle
/// garde sa taille, et rien ne sort du disque visible.
fn champ_seul(fond: &Dalle, ancre: Ancre, champ: ChampRendu) -> Vec<(u32, u32)> {
    let composee = Dalle::composee(fond, &[(ancre, champ.clone())]);
    dalle_bien_dimensionnee(
        &composee,
        &format!("la composition de {champ:?} sur {ancre:?}"),
    );

    let changes = pixels_changes(fond, &composee);
    for &(x, y) in &changes {
        assert!(
            !entierement_hors_du_disque(x, y),
            "{ancre:?} porte {champ:?} et a peint le pixel ({x}, {y}), qui est entièrement hors du \
             disque visible de rayon {} — la dalle est ronde, ce pixel n'existe pas, et le \
             contrôleur ne dira jamais rien",
            screen::VISIBLE_DISC_RADIUS
        );
    }
    changes
}

// ---------------------------------------------------------------------------
// 1 — une composition sans champ est l'identité
// ---------------------------------------------------------------------------

#[test]
fn une_composition_sans_champ_rend_le_fond_octet_pour_octet() {
    // Test d'intention n° 1 de l'issue, et critère d'acceptation : « une composition sans aucun
    // champ rend **exactement** ce que `screen image` produit aujourd'hui, octet pour octet — la
    // valeur par défaut ne change pas ce qu'on voit ».
    //
    // Piège n° 1 du préambule : c'est « octet pour octet » qui compte, pas « à peu près pareil ».
    // Une recopie qui repasserait par une conversion de couleur ou une remise à l'échelle donnerait
    // une image plausible et différente, invérifiable à l'œil sur six centimètres.
    let dossier = DossierJetable::neuf("sans-champ");
    let fichier = dossier.fichier("fond.png");
    ecrire_png(&fichier, LARGEUR / 2, HAUTEUR / 2, TEMOIN);

    // Le fond tel que `screen image` le produit aujourd'hui : le même chemin, le même décodeur.
    let depuis_image = dalle_unique(&fichier);

    for (quoi, fond) in [
        ("la dalle noire", Dalle::noire()),
        ("une dalle unie", Dalle::unie(TEMOIN)),
        ("une dalle blanche", Dalle::unie(BLANC)),
        ("l'image de `screen image`", depuis_image),
    ] {
        let composee = Dalle::composee(&fond, &[]);
        assert_eq!(
            composee.octets(),
            fond.octets(),
            "{quoi} composée sans champ doit être {quoi}, octet pour octet"
        );
        assert!(
            composee == fond,
            "{quoi} : `PartialEq` sur `Dalle` compare les octets, et le démon s'en sert pour \
             décider s'il pousse 1,2 Mo sur le bus"
        );
        dalle_bien_dimensionnee(&composee, &format!("{quoi} composée sans champ"));
    }
}

// ---------------------------------------------------------------------------
// 2 et 12 — la géométrie : dans la boîte, et jamais hors du disque
// ---------------------------------------------------------------------------

#[test]
fn chaque_ancre_dessine_dans_sa_boite_et_jamais_hors_du_disque() {
    // Test d'intention n° 2 de l'issue, prolongé côté rendu : « Un champ sur chacune des cinq
    // ancres : les cinq boîtes tiennent dans le disque inscrit. »
    //
    // Point n° 1 des conventions tranchées en tête : que la **boîte** tienne dans le disque ne
    // prouve rien si le rendu déborde d'elle. `Ancre::boite()` est donc la boîte englobante du
    // champ, contour ou plaque compris — c'est ce qui relie le calcul pur de `reverb-proto` à ce
    // qui part vraiment sur le bus.
    let fond = Dalle::unie(TEMOIN);

    for ancre in Ancre::TOUTES {
        let boite = ancre.boite();
        for champ in champs_temoins() {
            let changes = champ_seul(&fond, ancre, champ.clone());
            assert!(
                !changes.is_empty(),
                "{ancre:?} porte {champ:?} et n'a rien dessiné — un champ posé qui ne se voit pas \
                 se lit comme un démon qui n'a pas reçu la commande"
            );

            for &(x, y) in &changes {
                let dedans = x >= u32::from(boite.x)
                    && x < u32::from(boite.x) + u32::from(boite.largeur)
                    && y >= u32::from(boite.y)
                    && y < u32::from(boite.y) + u32::from(boite.hauteur);
                assert!(
                    dedans || dans_son_secteur(ancre, x, y),
                    "{ancre:?} porte {champ:?} et a peint le pixel ({x}, {y}), hors de sa boîte \
                     {boite:?} et hors de son secteur de couronne — deux champs voisins finiraient \
                     par s'effacer l'un l'autre, et le calcul « la boîte tient dans le disque » ne \
                     prouverait plus rien"
                );
            }
        }
    }
}

#[test]
fn un_texte_plus_large_que_la_corde_ne_sort_jamais_du_disque() {
    // Test d'intention n° 12 de l'issue, et critère d'acceptation : « un texte trop large pour la
    // corde disponible à son ancre est tronqué ou réduit, **jamais** dessiné hors du disque ».
    //
    // Ce fichier ne choisit pas entre tronquer et réduire — l'issue laisse les deux ouvertes, et un
    // test d'intention ne tranche pas un goût. Ce qu'il exige, c'est que le champ reste dans sa
    // boîte quoi qu'on lui donne : un texte de deux cents caractères, un nombre à cinq chiffres,
    // un infini, un `NaN`. Chacun est une occasion d'écrire hors du tampon, et une dalle courte est
    // ignorée en silence par le contrôleur.
    let fond = Dalle::unie(TEMOIN);

    let demesures = [
        ChampRendu::Texte(TEXTE_TROP_LONG.to_owned()),
        ChampRendu::Texte("W".repeat(200)),
        ChampRendu::Texte("é".repeat(200)),
        ChampRendu::Texte("0123456789".repeat(20)),
        ChampRendu::Temperature {
            libelle: Some(TEXTE_TROP_LONG.to_owned()),
            valeur: Some(34.2),
        },
        ChampRendu::Temperature {
            libelle: None,
            valeur: Some(99_999.0),
        },
        ChampRendu::Temperature {
            libelle: None,
            valeur: Some(-273.15),
        },
        ChampRendu::Temperature {
            libelle: None,
            valeur: Some(f32::MAX),
        },
        ChampRendu::Temperature {
            libelle: None,
            valeur: Some(f32::MIN),
        },
        ChampRendu::Temperature {
            libelle: None,
            valeur: Some(f32::NAN),
        },
        ChampRendu::Temperature {
            libelle: None,
            valeur: Some(f32::INFINITY),
        },
        ChampRendu::Temperature {
            libelle: None,
            valeur: Some(f32::NEG_INFINITY),
        },
    ];

    for ancre in Ancre::TOUTES {
        let boite = ancre.boite();
        for champ in &demesures {
            // `champ_seul` vérifie déjà la taille de la dalle et le disque ; reste la boîte.
            for &(x, y) in &champ_seul(&fond, ancre, champ.clone()) {
                let dedans = x >= u32::from(boite.x)
                    && x < u32::from(boite.x) + u32::from(boite.largeur)
                    && y >= u32::from(boite.y)
                    && y < u32::from(boite.y) + u32::from(boite.hauteur);
                assert!(
                    dedans || dans_son_secteur(ancre, x, y),
                    "{ancre:?} porte {champ:?} et a peint le pixel ({x}, {y}), hors de sa boîte \
                     {boite:?} et hors de son secteur de couronne — un texte trop large se tronque \
                     ou se réduit, il ne déborde pas"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — lisible sur blanc comme sur noir
// ---------------------------------------------------------------------------

#[test]
fn un_champ_est_lisible_sur_un_fond_blanc_comme_sur_un_fond_noir() {
    // Test d'intention n° 4 de l'issue : « Un champ dessiné sur un tampon entièrement blanc y
    // laisse des pixels sombres ; sur un tampon entièrement noir, des pixels clairs », et critère
    // d'acceptation : « contour ou fond derrière les caractères, pas une couleur qu'on espère
    // contrastée ».
    //
    // Le mutant que ce test existe pour tuer est banal et invisible en développement : un champ
    // écrit en blanc, essayé sur une photo sombre, parfait — et illisible le jour où quelqu'un met
    // une photo de neige en fond. Le fond est choisi par l'utilisateur, il ne se négocie pas.
    //
    // Piège n° 2 du préambule : « sombre » et « clair » se mesurent sur la moyenne des trois octets
    // d'un pixel, invariante par permutation des composantes.
    let sombre_max = 96u16;
    let clair_min = 160u16;
    let minimum = 20usize;

    for champ in champs_temoins() {
        for ancre in Ancre::TOUTES {
            let sur_blanc = Dalle::unie(BLANC);
            let composee = Dalle::composee(&sur_blanc, &[(ancre, champ.clone())]);
            let sombres = pixels_changes(&sur_blanc, &composee)
                .into_iter()
                .filter(|&(x, y)| clarte(&composee, x, y) <= sombre_max)
                .count();
            assert!(
                sombres >= minimum,
                "{champ:?} sur {ancre:?}, fond blanc : {sombres} pixel(s) sombre(s) pour {minimum} \
                 attendus — un champ écrit en clair disparaît sur une photo de neige, et rien ne \
                 le signale"
            );

            let sur_noir = Dalle::unie(NOIR);
            let composee = Dalle::composee(&sur_noir, &[(ancre, champ.clone())]);
            let clairs = pixels_changes(&sur_noir, &composee)
                .into_iter()
                .filter(|&(x, y)| clarte(&composee, x, y) >= clair_min)
                .count();
            assert!(
                clairs >= minimum,
                "{champ:?} sur {ancre:?}, fond noir : {clairs} pixel(s) clair(s) pour {minimum} \
                 attendus"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — une sonde muette
// ---------------------------------------------------------------------------

#[test]
fn une_sonde_muette_ecrit_des_tirets_jamais_un_zero() {
    // Test d'intention n° 3 de l'issue, et critère d'acceptation : « un champ sur une sonde muette
    // affiche des **tirets**, jamais un zéro ni la dernière valeur connue (même règle que le
    // cadran) ».
    //
    // Piège n° 3 du préambule. C'est le mode de défaillance le plus coûteux du projet parce qu'il
    // est **rassurant** : un 34 °C affiché derrière une pompe arrêtée, c'est un circuit qui chauffe
    // sans que rien ne le signale. Zéro est pire encore — c'est ce qu'un `unwrap_or_default()`
    // écrirait, et « 0 °C » se lit comme une information, pas comme une absence.
    let muette = ChampRendu::Temperature {
        libelle: Some("Liquide".to_owned()),
        valeur: None,
    };
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

    for valeur in [0.0f32, -0.0, 34.2, 100.0] {
        let presente = ChampRendu::Temperature {
            libelle: Some("Liquide".to_owned()),
            valeur: Some(valeur),
        };
        assert_ne!(
            valeur_du_champ(&presente),
            ecrit,
            "une sonde muette ne s'écrit pas comme {valeur} °C"
        );
    }

    // Et cela se voit sur la dalle, pas seulement dans une chaîne : le rendu d'une sonde muette
    // diffère de celui de toute valeur, y compris de zéro.
    let fond = Dalle::unie(TEMOIN);
    let rendu_muet = Dalle::composee(&fond, &[(Ancre::Centre, muette.clone())]);
    for valeur in [0.0f32, 34.2] {
        let rendu_valeur = Dalle::composee(
            &fond,
            &[(
                Ancre::Centre,
                ChampRendu::Temperature {
                    libelle: Some("Liquide".to_owned()),
                    valeur: Some(valeur),
                },
            )],
        );
        assert_ne!(
            rendu_muet.octets(),
            rendu_valeur.octets(),
            "le champ d'une sonde muette ne doit pas ressembler à celui de {valeur} °C"
        );
    }

    // « Jamais la dernière valeur connue » : le rendu est une fonction de ses arguments, et rien
    // d'autre. Composer une valeur puis une absence donne la même dalle que l'absence seule — c'est
    // la forme testable de la promesse, et la signature de `composee` en est la garantie.
    let apres_une_valeur = Dalle::composee(
        &fond,
        &[(
            Ancre::Centre,
            ChampRendu::Temperature {
                libelle: Some("Liquide".to_owned()),
                valeur: Some(34.2),
            },
        )],
    );
    assert_ne!(apres_une_valeur.octets(), rendu_muet.octets());
    assert_eq!(
        Dalle::composee(&fond, &[(Ancre::Centre, muette)]).octets(),
        rendu_muet.octets(),
        "aucune valeur d'avant ne survit d'un rendu au suivant"
    );
}

#[test]
fn valeur_du_champ_montre_la_valeur_et_pas_le_libelle() {
    // Contrat — `valeur_du_champ` : « Ce qu'un champ écrit **en gros**, en clair. « 34.2 » pour une
    // mesure, « --- » pour une sonde muette, le texte lui-même pour un texte fixe. »
    //
    // Point n° 3 des conventions tranchées en tête : le libellé légende la valeur, il n'en fait pas
    // partie. Les concaténer ferait qu'un libellé rallongé rétrécirait le chiffre — et le chiffre
    // est ce qu'on lit à un mètre.
    for libelle in [
        None,
        Some("Liquide"),
        Some("Liquide — boucle haute"),
        Some(""),
    ] {
        let champ = ChampRendu::Temperature {
            libelle: libelle.map(str::to_owned),
            valeur: Some(34.2),
        };
        assert_eq!(
            valeur_du_champ(&champ),
            valeur_du_champ(&ChampRendu::Temperature {
                libelle: None,
                valeur: Some(34.2)
            }),
            "le libellé {libelle:?} ne doit pas changer ce que le champ écrit en gros"
        );
    }

    // Une mesure s'écrit avec ses chiffres, et une valeur qui change change ce qui est écrit —
    // sans quoi le champ serait une image fixe qui ressemble à une mesure.
    let mesure = valeur_du_champ(&ChampRendu::Temperature {
        libelle: None,
        valeur: Some(34.2),
    });
    assert!(
        mesure.contains("34"),
        "une mesure de 34,2 °C s'écrit avec ses chiffres. Obtenu : « {mesure} »"
    );
    assert_ne!(
        mesure,
        valeur_du_champ(&ChampRendu::Temperature {
            libelle: None,
            valeur: Some(80.0)
        }),
        "34 °C et 80 °C ne s'écrivent pas pareil"
    );

    // Un texte fixe s'écrit tel quel : le tronquer ou le normaliser ici ferait qu'un champ posé ne
    // montre pas ce qu'on a tapé.
    for contenu in ["Bonjour", "soirée d'été", "LAN party — salle 2", "100 %"] {
        assert_eq!(
            valeur_du_champ(&ChampRendu::Texte(contenu.to_owned())),
            contenu,
            "un texte fixe s'écrit tel qu'on l'a posé"
        );
    }
}

// ---------------------------------------------------------------------------
// 11 — deux rendus identiques
// ---------------------------------------------------------------------------

#[test]
fn deux_rendus_sur_les_memes_valeurs_sont_egaux_octet_pour_octet() {
    // Test d'intention n° 11 de l'issue, et critère d'acceptation : « deux recompositions à
    // température identique donnent le **même tampon**, et rien n'est poussé ».
    //
    // Piège n° 4 du préambule : c'est la condition du repos du démon. Le protocole n'a **aucune
    // mise à jour partielle** — changer un chiffre, c'est repousser 1 228 800 octets —, et le démon
    // ne pousse que si le tampon a changé. Un rendu qui dépendrait d'une horloge, d'un compteur ou
    // d'un parcours de table non déterministe ferait partir une image toutes les deux secondes sur
    // une machine parfaitement stable.
    let fond = Dalle::unie(TEMOIN);
    let champs: Vec<(Ancre, ChampRendu)> = vec![
        (
            Ancre::Haut,
            ChampRendu::Temperature {
                libelle: Some("Liquide".to_owned()),
                valeur: Some(34.2),
            },
        ),
        (
            Ancre::Bas,
            ChampRendu::Temperature {
                libelle: Some("GPU".to_owned()),
                valeur: None,
            },
        ),
        (Ancre::Gauche, ChampRendu::Texte("soirée d'été".to_owned())),
        (Ancre::Droite, ChampRendu::Texte("100 %".to_owned())),
    ];

    let premier = Dalle::composee(&fond, &champs);
    for tour in 0..3 {
        assert_eq!(
            Dalle::composee(&fond, &champs).octets(),
            premier.octets(),
            "recomposition n° {tour} : même fond, mêmes valeurs, même tampon"
        );
    }

    // Le pendant, sans lequel le test précédent serait satisfait par un rendu qui ne dessine rien :
    // une valeur qui bouge fait bouger le tampon, et c'est ce qui déclenche l'envoi.
    let mut autre = champs.clone();
    autre[0] = (
        Ancre::Haut,
        ChampRendu::Temperature {
            libelle: Some("Liquide".to_owned()),
            valeur: Some(34.3),
        },
    );
    assert_ne!(
        Dalle::composee(&fond, &autre).octets(),
        premier.octets(),
        "un dixième de degré de plus doit se voir, sinon le champ n'affiche pas ce qu'il prétend"
    );
}

#[test]
fn l_ordre_des_champs_ne_change_pas_le_tampon() {
    // Point n° 2 des conventions tranchées en tête. Les cinq boîtes ne se recouvrent pas —
    // `spec_composition.rs` l'exige —, donc l'ordre dans lequel on les dessine ne peut pas se voir.
    // Deux tampons qui différeraient trahiraient un débordement d'une boîte sur sa voisine, c'est-
    // à-dire un champ en train d'en effacer un autre.
    let fond = Dalle::unie(TEMOIN);
    let mut champs: Vec<(Ancre, ChampRendu)> = vec![
        (Ancre::Haut, ChampRendu::Texte("haut".to_owned())),
        (Ancre::Bas, ChampRendu::Texte("bas".to_owned())),
        (Ancre::Gauche, ChampRendu::Texte("gauche".to_owned())),
        (Ancre::Droite, ChampRendu::Texte("droite".to_owned())),
    ];
    let dans_l_ordre = Dalle::composee(&fond, &champs);
    champs.reverse();
    assert_eq!(
        Dalle::composee(&fond, &champs).octets(),
        dans_l_ordre.octets(),
        "l'ordre des champs ne se voit pas : leurs boîtes sont disjointes"
    );
}

// ---------------------------------------------------------------------------
// 6, 7 et la persistance
// ---------------------------------------------------------------------------

#[test]
fn une_composition_survit_au_redemarrage_du_demon() {
    // Critère d'acceptation : « une composition survit au redémarrage du démon ». Le mécanisme est
    // celui de #33 — `ecran.conf`, écrit à chaque changement — et le mode de défaillance n'est pas
    // une erreur, c'est un état plausible et faux : un champ perdu, un libellé tronqué au premier
    // blanc, un fond noir revenu à la place d'une photo.
    let dossier = DossierJetable::neuf("survit");
    let chemin = dossier.fichier("ecran.conf");

    for etat in etats_de_composition() {
        let texte = etat.encoder();
        assert_eq!(
            Etat::decoder(&texte),
            Ok(etat.clone()),
            "aller-retour de {etat:?} par :\n{texte}"
        );

        enregistrer(&chemin, &etat).expect("l'enregistrement doit réussir");
        let (relu, signalement) = charger(&chemin);
        assert_eq!(relu, etat, "l'aller-retour par le disque est exact");
        assert_eq!(
            signalement, None,
            "ce que le démon vient d'écrire, il doit savoir le relire. Obtenu : {signalement:?}"
        );
    }

    // Point n° 4 des conventions tranchées en tête : la ligne `affiche` d'une composition ne porte
    // aucun chemin. Le fond vit dans le bloc, et le répéter ferait deux sources de vérité pour la
    // même photo — qu'un fichier édité à la main pourrait rendre contradictoires.
    let etat = etat_de_composition_temoin();
    let texte = etat.encoder();
    let lignes: Vec<&str> = texte.lines().collect();
    assert!(
        lignes.len() >= 3,
        "une composition ajoute au moins la ligne de son fond : {lignes:?}"
    );
    assert_eq!(
        rang_de(&texte, "brightness"),
        1,
        "la première ligne reste celle de la luminosité : {texte}"
    );
    assert_eq!(
        rang_de(&texte, "affiche"),
        2,
        "la seconde reste celle de l'affichage : {texte}"
    );
    assert_eq!(
        lignes[1].split_whitespace().count(),
        2,
        "la ligne d'affichage d'une composition est un jeton unique : le fond et les champs ont \
         leurs propres lignes. Obtenu : « {} »",
        lignes[1]
    );
    assert_eq!(
        rang_de(&texte, "fond"),
        3,
        "le bloc de composition commence juste après, par son fond : {texte}"
    );
}

#[test]
fn un_ecran_conf_de_deux_lignes_se_lit_toujours() {
    // Test d'intention n° 6 de l'issue, et critère d'acceptation : « un `ecran.conf` d'avant ce
    // chantier (deux lignes) se lit toujours ».
    //
    // C'est la compatibilité qui compte ici, et elle a un coût connu quand on la rate : un fichier
    // devenu illisible fait repartir le démon sur l'accueil, en bleu, en perdant l'affichage réglé.
    // Les quatre affichages d'avant gardent donc leur fichier de **deux lignes**, sans bloc de
    // composition — un bloc vide ajouté « pour l'uniformité » suffirait à casser la lecture par une
    // version antérieure, et surtout signalerait une composition là où il n'y en a pas.
    let dossier = DossierJetable::neuf("deux-lignes");
    let chemin = dossier.fichier("ecran.conf");

    for etat in etats_d_avant() {
        let texte = etat.encoder();
        let lignes: Vec<&str> = texte.lines().collect();
        assert_eq!(
            lignes.len(),
            2,
            "{:?} garde le fichier de deux lignes de #33 : {lignes:?}",
            etat.affichage
        );

        // Le fichier tel qu'une version d'avant l'écrivait, reconstruit à partir de ses deux
        // lignes : avec et sans saut de ligne final, parce qu'un fichier écrit à la main en a
        // souvent un et qu'un fichier tronqué n'en a pas.
        for variante in [lignes.join("\n"), format!("{}\n", lignes.join("\n"))] {
            assert_eq!(
                Etat::decoder(&variante),
                Ok(etat.clone()),
                "un fichier de deux lignes se lit toujours : {variante:?}"
            );
        }

        enregistrer(&chemin, &etat).expect("l'enregistrement doit réussir");
        let (relu, signalement) = charger(&chemin);
        assert_eq!(relu, etat);
        assert_eq!(signalement, None, "obtenu : {signalement:?}");
    }

    // Et la lecture reste **stricte** : une ligne de trop derrière un affichage d'avant est le
    // signe d'un fichier d'une version qu'on ne sait pas lire, et l'avaler ferait repartir le démon
    // sur un état amputé de ce qu'il n'a pas compris. C'est l'acquis de #33, que ce chantier ne
    // doit pas relâcher en ouvrant le fichier à des lignes supplémentaires.
    let temoin = Etat {
        luminosite: 42,
        affichage: Affichage::Cadran(SONDE.to_owned()),
    };
    let bavard = format!("{}\nfond noir", temoin.encoder().trim_end());
    let erreur = refus_d_etat(&bavard);
    assert_eq!(
        erreur.ligne, 3,
        "un bloc de composition derrière un cadran est une ligne de trop, et c'est la ligne 3. \
         Obtenu : {erreur}"
    );
}

#[test]
fn une_entree_de_composition_absente_repetee_ou_aberrante_est_refusee_en_la_nommant() {
    // Test d'intention n° 7 de l'issue, et critère d'acceptation : « une entrée de composition
    // absente, répétée ou aberrante dans le fichier est refusée **en la nommant** ».
    //
    // C'est ce qui rend un fichier tronqué détectable. Une entrée manquante complétée au jugé donne
    // une composition plausible et fausse, rejouée à chaque démarrage — et le rang de ligne est ce
    // qui permet de la corriger : le fichier est écrit par le démon, mais **édité à la main** le
    // jour où quelqu'un veut poser un champ sans passer par la fenêtre.
    let etat = etat_de_composition_temoin();
    let texte = etat.encoder();
    let lignes: Vec<String> = texte.lines().map(str::to_owned).collect();
    let rang_fond = rang_de(&texte, "fond");
    assert!(
        lignes.len() >= rang_fond + 2,
        "l'état témoin porte un fond et au moins deux champs : {texte}"
    );

    let mut raisons = Vec::new();

    // a — le bloc entier manque : le fichier s'arrête après `affiche`. C'est la ligne du fond qu'il
    // faut ajouter, et nommer la dernière ligne présente enverrait corriger une ligne correcte.
    let tronque = lignes[..rang_fond - 1].join("\n");
    let erreur = refus_d_etat(&tronque);
    assert_eq!(
        erreur.ligne, rang_fond,
        "la ligne {rang_fond} manque : c'est elle que le message doit nommer. Obtenu : {erreur}"
    );
    assert!(
        erreur.to_string().contains(&rang_fond.to_string()),
        "le Display porte le rang de la ligne — c'est ce qui rend le fichier corrigeable à la \
         main : « {erreur} »"
    );
    raisons.push(erreur.raison);

    // b — la ligne du fond manque, mais les champs sont là.
    let mut sans_fond = lignes.clone();
    sans_fond.remove(rang_fond - 1);
    let erreur = refus_d_etat(&sans_fond.join("\n"));
    assert_eq!(
        erreur.ligne, rang_fond,
        "le fond manque à la ligne {rang_fond}. Obtenu : {erreur}"
    );
    raisons.push(erreur.raison);

    // c — une ancre répétée. Le message nomme **l'ancre**, parce que c'est elle qu'on va chercher
    // dans le fichier : deux `champ haut` ne se départagent pas, et laisser le dernier gagner
    // ferait afficher un champ qu'on croit avoir remplacé.
    let rang_premier_champ = rang_fond + 1;
    let mut repetee = lignes.clone();
    repetee.push(lignes[rang_premier_champ - 1].clone());
    let erreur = refus_d_etat(&repetee.join("\n"));
    assert_eq!(
        erreur.ligne,
        repetee.len(),
        "la répétition est la dernière ligne du fichier. Obtenu : {erreur}"
    );
    assert!(
        erreur.raison.contains(Ancre::Haut.slug()),
        "le refus nomme l'ancre répétée. Obtenu : {}",
        erreur.raison
    );
    raisons.push(erreur.raison);

    // d — une ancre inconnue, en plein fichier.
    let mut inconnue = lignes.clone();
    inconnue[rang_premier_champ - 1] = "champ milieu texte Bonjour".to_owned();
    let erreur = refus_d_etat(&inconnue.join("\n"));
    assert_eq!(erreur.ligne, rang_premier_champ, "obtenu : {erreur}");
    assert!(
        erreur.raison.contains("milieu"),
        "le refus nomme l'ancre fautive. Obtenu : {}",
        erreur.raison
    );
    raisons.push(erreur.raison);

    // e — des entrées aberrantes, à la ligne du fond puis à celle d'un champ.
    for (rang, aberrante) in [
        (rang_fond, "fond"),
        (rang_fond, "fond bidule"),
        (rang_fond, "bidule noir"),
        (rang_premier_champ, "champ"),
        (rang_premier_champ, "champ haut"),
        (rang_premier_champ, "champ haut bidule Bonjour"),
        (rang_premier_champ, "champ haut texte"),
        (rang_premier_champ, "bidule haut texte Bonjour"),
    ] {
        let mut abime = lignes.clone();
        abime[rang - 1] = aberrante.to_owned();
        let erreur = refus_d_etat(&abime.join("\n"));
        assert_eq!(
            erreur.ligne, rang,
            "« {aberrante} » est à la ligne {rang}. Obtenu : {erreur}"
        );
        raisons.push(erreur.raison);
    }

    // f — cinq champs dans le fichier. Le plafond ne se contourne pas en éditant `ecran.conf` : le
    // démon repartirait sur une composition qu'aucune commande ne sait produire.
    let mut cinq: Vec<String> = lignes[..rang_fond].to_vec();
    for ancre in Ancre::TOUTES {
        cinq.push(format!("champ {} texte {}", ancre.slug(), ancre.slug()));
    }
    let erreur = refus_d_etat(&cinq.join("\n"));
    assert!(
        erreur.raison.contains(&Composition::CHAMPS_MAX.to_string()),
        "le refus dit le plafond. Obtenu : {}",
        erreur.raison
    );
    raisons.push(erreur.raison);

    // Les refus distinguent les fautes : une entrée manquante, une entrée répétée, une ancre
    // inconnue et un mot-clé inconnu ne se corrigent pas de la même façon, et une phrase unique
    // laisserait tout le diagnostic à faire alors que le fichier est sous les yeux.
    raisons.sort_unstable();
    raisons.dedup();
    assert!(
        raisons.len() >= 4,
        "quatre familles de fautes ne peuvent pas partager une seule explication : {raisons:?}"
    );
}

// ---------------------------------------------------------------------------
// 10 — rien n'est persisté avant d'avoir été validé
// ---------------------------------------------------------------------------

#[test]
fn un_fond_qui_n_est_pas_une_image_est_refuse_avant_toute_persistance() {
    // Test d'intention n° 10 de l'issue, et critère d'acceptation : « rien n'est persisté ni
    // affiché avant d'avoir été validé — le fond est reconnu **au contenu**, comme depuis #69 ».
    //
    // L'issue #69 raconte ce que coûte l'inverse : un affichage impossible persisté faisait
    // redémarrer le démon dans un état cassé, indéfiniment, sans moyen d'en sortir seul. Un fond de
    // composition est exactement le même risque, avec la même issue si on l'oublie.
    let dossier = DossierJetable::neuf("fond-verifie");

    let png = dossier.fichier("fond.png");
    ecrire_png(&png, 32, 32, TEMOIN);
    let jpeg = dossier.fichier("fond.jpg");
    ecrire_jpeg(&jpeg, 32, 32, TEMOIN);
    let texte = dossier.fichier("notes.txt");
    fs::write(&texte, "ceci n'est pas une image\n").expect("écriture du faux fichier");
    let absent = dossier.fichier("jamais-ecrit.png");

    // Ce qui est une image passe : une vérification qui refuserait tout rendrait la composition
    // inutilisable, ce qui est une panne plus grave que celle qu'on prévient.
    for bon in [&png, &jpeg] {
        let affichage = composition_sur(bon);
        let octets = fs::read(bon).expect("relecture du témoin");
        verifier_format(&affichage, &octets)
            .unwrap_or_else(|erreur| panic!("{} doit être accepté : {erreur}", bon.display()));
        verifier_fichier(&affichage).unwrap_or_else(|erreur| {
            panic!("{} doit être accepté sur fichier : {erreur}", bon.display())
        });
    }

    // Ce qui n'en est pas est refusé, en nommant le fichier — c'est lui qu'on va corriger, et le
    // reste de la composition n'y est pour rien.
    let erreur = refus_d_image(
        &composition_sur(&texte),
        &fs::read(&texte).expect("relecture"),
    );
    assert!(
        erreur.raison.contains("notes.txt"),
        "le refus nomme le fichier en cause. Obtenu : {}",
        erreur.raison
    );
    assert!(
        erreur.raison.to_uppercase().contains("RECONNU"),
        "rien à nommer comme format trouvé : le message doit le **dire**, comme depuis #69. \
         Obtenu : {}",
        erreur.raison
    );

    // Un fichier absent est une faute distincte, et il ne doit pas se dégrader en « format non
    // reconnu » — qui enverrait chercher un problème de format sur un fichier qui n'est pas là.
    let sur_absent = verifier_fichier(&composition_sur(&absent))
        .expect_err("un fond qui n'existe pas doit être refusé");
    assert!(
        sur_absent.raison.contains("jamais-ecrit.png"),
        "le refus nomme le fichier absent. Obtenu : {}",
        sur_absent.raison
    );

    // Et un refus n'écrit rien : l'état d'avant reste intact au bit près. C'est la moitié du
    // critère qui se démontre ici — que la vérification soit **appelée avant** l'écriture vit dans
    // la boucle de commande du démon, et se vérifie sur la machine (même partage qu'à #69).
    let ecran_conf = dossier.fichier("ecran.conf");
    let avant = etat_de_composition_temoin();
    enregistrer(&ecran_conf, &avant).expect("l'état témoin doit s'écrire");
    let octets_avant = fs::read(&ecran_conf).expect("l'état témoin doit se relire");

    for mauvais in [&texte, &absent] {
        let affichage = composition_sur(mauvais);
        assert!(
            verifier_fichier(&affichage).is_err(),
            "{} devait être refusé",
            mauvais.display()
        );
    }
    assert_eq!(
        fs::read(&ecran_conf).expect("l'état doit toujours être là"),
        octets_avant,
        "un refus a modifié l'état persisté — l'état d'avant doit rester intact au bit près"
    );
    assert_eq!(
        charger(&ecran_conf).0,
        avant,
        "l'état relu après une série de refus n'est plus celui d'avant"
    );
}

#[test]
fn une_composition_sur_fond_noir_n_a_aucun_fichier_a_verifier() {
    // `Fond::Noir` ne nomme aucun fichier : il n'y a rien à reconnaître, et refuser faute d'avoir
    // trouvé un format empêcherait de **sortir** d'un fond fautif — ce qui est exactement la
    // situation de #69, où l'état cassé ne pouvait plus être remplacé.
    let mut composition = Composition::nouvelle(Fond::Noir);
    composition
        .poser(
            Ancre::Centre,
            Source::Temperature {
                sonde: SONDE.to_owned(),
                libelle: Some("Liquide".to_owned()),
            },
        )
        .expect("un premier champ tient");
    let affichage = Affichage::Composition(composition);

    verifier_format(&affichage, b"ceci n'est pas une image")
        .expect("un fond noir n'a aucun format à reconnaître");
    verifier_format(&affichage, &[]).expect("ni sur zéro octet");
    verifier_fichier(&affichage).expect("ni sur fichier : il n'y en a pas");
}

// ---------------------------------------------------------------------------
// États et champs témoins
// ---------------------------------------------------------------------------

/// Les champs témoins du rendu, dans les trois formes que l'issue nomme.
fn champs_temoins() -> Vec<ChampRendu> {
    vec![
        ChampRendu::Temperature {
            libelle: Some("Liquide".to_owned()),
            valeur: Some(34.2),
        },
        ChampRendu::Temperature {
            libelle: None,
            valeur: Some(80.0),
        },
        ChampRendu::Temperature {
            libelle: Some("GPU".to_owned()),
            valeur: None,
        },
        ChampRendu::Texte("Bonjour".to_owned()),
        ChampRendu::Texte("soirée d'été".to_owned()),
    ]
}

/// Une composition posée sur ce fichier, sans champ : ce qui est vérifié, c'est le fond.
fn composition_sur(chemin: &Path) -> Affichage {
    Affichage::Composition(Composition::nouvelle(Fond::Image(chaine(chemin))))
}

/// L'état témoin de composition : un fond image, deux champs, une luminosité qui n'est pas celle de
/// l'accueil.
fn etat_de_composition_temoin() -> Etat {
    let mut composition = Composition::nouvelle(Fond::Image(
        "/home/nico/Mes documents/fond d'écran.png".to_owned(),
    ));
    composition
        .poser(
            Ancre::Haut,
            Source::Temperature {
                sonde: SONDE.to_owned(),
                libelle: Some("Liquide — boucle haute".to_owned()),
            },
        )
        .expect("premier champ");
    composition
        .poser(Ancre::Bas, Source::Texte("soirée d'été".to_owned()))
        .expect("deuxième champ");
    Etat {
        luminosite: 63,
        affichage: Affichage::Composition(composition),
    }
}

/// Des états de composition de zéro à quatre champs.
fn etats_de_composition() -> Vec<Etat> {
    let mut etats = vec![
        Etat {
            luminosite: 100,
            affichage: Affichage::Composition(Composition::nouvelle(Fond::Noir)),
        },
        etat_de_composition_temoin(),
    ];

    let mut quatre = Composition::nouvelle(Fond::Image("/a.png".to_owned()));
    for (ancre, source) in [
        (
            Ancre::Haut,
            Source::Temperature {
                sonde: SONDE.to_owned(),
                libelle: None,
            },
        ),
        (
            Ancre::Bas,
            Source::Temperature {
                sonde: "k10temp:tctl".to_owned(),
                libelle: Some("CPU".to_owned()),
            },
        ),
        (Ancre::Gauche, Source::Texte("LAN party".to_owned())),
        (Ancre::Droite, Source::Texte("100 %".to_owned())),
    ] {
        quatre.poser(ancre, source).expect("quatre champs tiennent");
    }
    etats.push(Etat {
        luminosite: 0,
        affichage: Affichage::Composition(quatre),
    });

    etats
}

/// Les quatre affichages d'avant ce chantier — ceux dont le fichier tient sur deux lignes.
fn etats_d_avant() -> Vec<Etat> {
    vec![
        Etat {
            luminosite: 100,
            affichage: Affichage::Rien,
        },
        Etat {
            luminosite: 42,
            affichage: Affichage::Cadran(SONDE.to_owned()),
        },
        Etat {
            luminosite: 99,
            affichage: Affichage::Image("/home/nico/images/fond.png".to_owned()),
        },
        Etat {
            luminosite: 7,
            affichage: Affichage::Gif("/home/nico/anims/pluie.gif".to_owned()),
        },
    ]
}

// ---------------------------------------------------------------------------
// Refus
// ---------------------------------------------------------------------------

/// Le refus d'un texte d'état, avec ce que ce fichier exige de tout refus : une raison non vide, un
/// rang de ligne, et un type qui soit une vraie erreur.
fn refus_d_etat(texte: &str) -> EtatInvalide {
    match Etat::decoder(texte) {
        Ok(etat) => panic!(
            "ce fichier devait être refusé, il a rendu {etat:?} :\n{texte}\nUn fichier abîmé \
             accepté est un état plausible et faux, rejoué à chaque démarrage"
        ),
        Err(erreur) => {
            assert!(
                !erreur.raison.trim().is_empty(),
                "ce fichier doit être refusé en disant pourquoi :\n{texte}"
            );
            assert!(
                erreur.to_string().contains(erreur.raison.as_str()),
                "le Display porte la raison — c'est elle qui part dans le journal : « {erreur} »"
            );
            let _: &dyn std::error::Error = &erreur;
            erreur
        }
    }
}

/// Le refus d'un fond, non muet.
fn refus_d_image(affichage: &Affichage, octets: &[u8]) -> ImageInvalide {
    match verifier_format(affichage, octets) {
        Ok(()) => panic!(
            "{affichage:?} devait être refusé sur {} octet(s) — l'accepter le persiste puis le \
             rejoue à chaque démarrage du service",
            octets.len()
        ),
        Err(erreur) => {
            assert!(
                !erreur.raison.trim().is_empty(),
                "{affichage:?} doit être refusé **en disant pourquoi**"
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

// ---------------------------------------------------------------------------
// Outils de fichier
// ---------------------------------------------------------------------------

/// Le rang, à partir de 1, de la ligne dont le premier mot est `mot`.
fn rang_de(texte: &str, mot: &str) -> usize {
    texte
        .lines()
        .position(|ligne| ligne.split_whitespace().next() == Some(mot))
        .map_or_else(
            || panic!("aucune ligne ne commence par « {mot} » dans :\n{texte}"),
            |rang| rang + 1,
        )
}

/// Le chemin en texte, tel que le protocole le transporte et que `Fond` le porte.
fn chaine(chemin: &Path) -> String {
    chemin
        .to_str()
        .unwrap_or_else(|| panic!("chemin de test non-UTF-8 : {}", chemin.display()))
        .to_owned()
}

/// L'unique dalle d'une image fixe, ou un échec qui dit ce qu'on a reçu à la place.
fn dalle_unique(chemin: &Path) -> Dalle {
    let dalles = Dalle::depuis_fichier(chemin)
        .unwrap_or_else(|erreur| panic!("« {} » doit être lisible : {erreur}", chemin.display()));
    assert_eq!(
        dalles.len(),
        1,
        "« {} » est une image fixe : elle rend une seule dalle",
        chemin.display()
    );
    dalles.into_iter().next().expect("une dalle exactement")
}

/// Les octets d'une image unie, en RGB à trois octets par pixel — le format d'entrée du crate
/// `image`, sans rapport avec l'ordre de la dalle.
fn aplat(largeur: u32, hauteur: u32, (r, v, b): (u8, u8, u8)) -> Vec<u8> {
    let pixels = largeur as usize * hauteur as usize;
    let mut octets = Vec::with_capacity(pixels * 3);
    for _ in 0..pixels {
        octets.extend_from_slice(&[r, v, b]);
    }
    octets
}

/// Écrit un PNG uni. Le crate `image` est une dépendance de `reverb-daemon` (ADR-005) : ces tests
/// s'en servent pour **fabriquer** leurs entrées, jamais pour vérifier leurs sorties.
fn ecrire_png(chemin: &Path, largeur: u32, hauteur: u32, couleur: (u8, u8, u8)) {
    image::save_buffer_with_format(
        chemin,
        &aplat(largeur, hauteur, couleur),
        largeur,
        hauteur,
        image::ExtendedColorType::Rgb8,
        image::ImageFormat::Png,
    )
    .unwrap_or_else(|erreur| panic!("écriture de {} : {erreur}", chemin.display()));
}

/// Écrit un JPEG uni — l'autre format qu'un fond doit accepter.
fn ecrire_jpeg(chemin: &Path, largeur: u32, hauteur: u32, couleur: (u8, u8, u8)) {
    image::save_buffer_with_format(
        chemin,
        &aplat(largeur, hauteur, couleur),
        largeur,
        hauteur,
        image::ExtendedColorType::Rgb8,
        image::ImageFormat::Jpeg,
    )
    .unwrap_or_else(|erreur| panic!("écriture de {} : {erreur}", chemin.display()));
}

/// Un dossier de travail sous `std::env::temp_dir()`, effacé à la fin du test.
///
/// L'effacement passe par `Drop` et non par une ligne en fin de test : un test qui échoue part en
/// `panic!`, et une fin de test jamais atteinte laisserait le dossier derrière elle à chaque
/// régression. Convention reprise de `spec_ecran.rs` (#33).
struct DossierJetable {
    chemin: PathBuf,
}

impl DossierJetable {
    /// `nom` doit être le nom du test : c'est ce qui rend le chemin unique entre tests, `cargo` les
    /// exécutant en parallèle dans un même processus.
    fn neuf(nom: &str) -> DossierJetable {
        let chemin = std::env::temp_dir().join(format!(
            "reverb-spec-composition-{}-{nom}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&chemin);
        fs::create_dir_all(&chemin)
            .unwrap_or_else(|erreur| panic!("dossier de test {} : {erreur}", chemin.display()));
        DossierJetable { chemin }
    }

    fn fichier(&self, nom: &str) -> PathBuf {
        self.chemin.join(nom)
    }
}

impl Drop for DossierJetable {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.chemin);
    }
}
