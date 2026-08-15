//! Tests d'intention des courbes du panneau SONDES (issue #118).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #118 seule — et depuis ce que #113 a établi
//! une fois pour toutes sur la mise à l'échelle des `Path` de Slint. Rien de `crates/*/src/` ni de
//! `crates/reverb-gui/ui/` n'a été lu pour les produire, hormis les signatures publiques déjà
//! figées par #31 (`Historique`, `MEMOIRE`, `Releve`) et par #113 (`TRACE_ASPECT`).
//!
//! À l'écriture de ce fichier, `reverb_gui::sondes::COURBE_ASPECT` et
//! `reverb_gui::sondes::commandes_de_courbe` **n'existent pas** : la compilation doit échouer sur
//! eux, et sur eux seuls. C'est la phase rouge.
//!
//! Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne touche un bus.
//!
//! # Le défaut, en une phrase
//!
//! Slint met un `Path` à l'échelle **uniformément** — `min(largeur / viewbox_largeur,
//! hauteur / viewbox_hauteur)` — et il **ne rogne pas** au viewbox. Un tracé émis dans le carré
//! unité, posé dans une tuile large et basse, n'occupe donc que `hauteur` pixels de large, centré :
//! le reste de la tuile reste vide. L'issue le mesure — une tuile de sonde fait ~140 px de large et
//! sa courbe n'en occupe qu'une soixantaine.
//!
//! Le remède est celui de #113, repris tel quel : **émettre le tracé dans un repère qui a déjà le
//! rapport de la tuile**, et faire tenir ce rapport à la tuile — le rapport n'existant **qu'une
//! fois**, dans le Rust, d'où la fenêtre tire *et* son `viewbox` *et* sa hauteur.
//!
//! # Le contrat que ces tests figent
//!
//! ```ignore
//! // dans crates/reverb-gui/src/sondes.rs, à côté de `Historique`.
//!
//! /// Le rapport largeur/hauteur du repère dans lequel la courbe d'une sonde est émise, et
//! /// celui que la tuile doit tenir. Il n'existe qu'ici — voir `TRACE_ASPECT` pour le
//! /// raisonnement complet (#113).
//! pub const COURBE_ASPECT: f32;
//!
//! /// La courbe d'une sonde, en commandes SVG **absolues** sur `COURBE_ASPECT` × 1.
//! pub fn commandes_de_courbe(historique: &Historique, sonde: &str) -> String;
//! ```
//!
//! ## Pourquoi une constante *publique*, et non un chiffre dans le `.slint`
//!
//! C'est le troisième critère d'acceptation de l'issue : « le rapport largeur/hauteur n'est écrit
//! qu'à un seul endroit ». Un chiffre recopié dans le `.slint` et un autre dans le Rust finiraient
//! par diverger, et le symptôme serait exactement celui qu'on corrige — un tracé faux que rien ne
//! signale. La seule façon de **tester** cette unicité depuis un fichier de tests est que la
//! constante soit publique et que le tracé s'y accorde ; c'est ce que fait le test n° 3.
//!
//! ## Trois arbitrages que l'issue ne tranche pas, et qu'il fallait trancher pour tester
//!
//! 1. **Le tracé est une polyligne en commandes absolues** — `M`, `L`, `Z` — **et un point par
//!    mesure lisible.** Sans cette convention, aucune assertion sur « la plus grande abscisse » ne
//!    veut dire quoi que ce soit : des commandes relatives donneraient des écarts et non des
//!    positions, et une aire remplie ajouterait des points de fond de tuile que personne n'a
//!    mesurés. C'est aussi la convention du reste de la maquette — `spec_boitier.rs` exige déjà des
//!    « chemins SVG absolus » de `plan.rs`.
//! 2. **L'axe des ordonnées est celui de l'écran : `y` croît vers le bas.** C'est le repère de tout
//!    `Path` SVG, celui de `plan.rs` et celui de `commandes_de_trace`. Une courbe qui **monte** —
//!    des mesures qui augmentent — a donc des ordonnées qui **descendent**. Une erreur de signe ici
//!    produirait une sparkline parfaitement bornée, parfaitement lisse, et qui montrerait le CPU se
//!    refroidir quand il chauffe : aucun autre test ne la verrait.
//! 3. **La largeur pleine s'exige d'une courbe pleine.** Une sonde apparue il y a trois secondes
//!    trace une courbe **partielle** (#31), et l'issue met hors scope « le contenu des sparklines
//!    — l'historique, la fenêtre de deux minutes, l'échelle verticale ». Décider ici si trois
//!    mesures occupent toute la tuile ou son premier quart inventerait une règle que l'issue n'a
//!    pas voulue. Ces tests exigent donc la largeur pleine sur un historique **plein**, et de tous
//!    les autres seulement qu'ils restent dans le repère.
//!
//! # Ce que ces tests ne vérifient pas
//!
//! - **Le rendu.** L'issue le dit elle-même : le critère se mesure « sur l'image d'aperçu », par
//!   `cargo run --release --example apercu -p reverb-gui`. Ce qui s'écrit en assertions, c'est la
//!   **forme du tracé émis**.
//! - **Le contenu de l'historique** — l'anneau de deux minutes, les bornes, le traitement de
//!   l'illisible. C'est #31, et `spec_historique.rs` le tient. Hors scope de #118.
//! - **La valeur de `COURBE_ASPECT`.** Elle se mesure sur la tuile ; l'issue donne un ordre de
//!   grandeur (~140 px de large pour une soixantaine de haut) mais aucun chiffre à figer. Ce qui se
//!   teste, c'est qu'elle existe, qu'elle est publique, qu'elle est plus large que haute — sinon le
//!   défaut n'est pas corrigé — et qu'elle n'est pas celle de #113.

use reverb_gui::reglages::TRACE_ASPECT;
use reverb_gui::sondes::{COURBE_ASPECT, Historique, MEMOIRE, Releve, commandes_de_courbe};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// La sonde de tous les tests de ce fichier — celle que #31 nomme.
const CPU: &str = "k10temp:tctl";
/// Une seconde sonde, pour vérifier qu'une tuile ne tire pas le tracé d'une autre.
const COOLANT: &str = "kraken2023elite:coolant-temp";

/// La tolérance des comparaisons de flottants.
///
/// Les commandes SVG sont écrites en décimal, donc arrondies à l'écriture ; trois décimales sont
/// la convention de `spec_boitier.rs`, qui compare déjà des sommets relus dans un chemin. Elle est
/// très en deçà du pixel : sur une tuile de 140 px, un millième de repère vaut 0,14 px.
const EPSILON: f32 = 1e-3;

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

/// Un historique où `combien` relevés produits par `mesure` ont été notés pour `sonde`.
fn historique_de(sonde: &str, combien: usize, mesure: impl Fn(usize) -> Releve) -> Historique {
    let mut historique = Historique::nouvel();
    for i in 0..combien {
        historique.noter(sonde, mesure(i));
    }
    historique
}

/// Un historique **plein** — les deux minutes complètes — de mesures produites par `mesure`.
///
/// C'est le seul cas où l'issue permet d'exiger la largeur pleine : voir l'arbitrage n° 3 en tête
/// de fichier.
fn historique_plein(sonde: &str, mesure: impl Fn(usize) -> Releve) -> Historique {
    historique_de(sonde, MEMOIRE, mesure)
}

/// Les points d'un chemin SVG, dans l'ordre où il les trace.
///
/// Panique — donc échoue le test, en le disant — sur toute commande qui n'est pas `M`, `L` ou `Z`.
/// Voir l'arbitrage n° 1 : sans cette convention, une abscisse relue n'est pas une position.
fn points(quoi: &str, commandes: &str) -> Vec<(f32, f32)> {
    fn vider(courant: &mut String, nombres: &mut Vec<f32>, quoi: &str) {
        if courant.is_empty() {
            return;
        }
        let lu = courant.parse::<f32>().unwrap_or_else(|_| {
            panic!("{quoi} : « {courant} » n'est pas un nombre lisible dans un chemin SVG")
        });
        nombres.push(lu);
        courant.clear();
    }

    let mut nombres: Vec<f32> = Vec::new();
    let mut courant = String::new();
    for c in commandes.chars() {
        if c.is_ascii_digit() || c == '.' {
            courant.push(c);
        } else if c == '-' {
            // « 0.12-0.34 » est deux nombres, pas un — même lecture que `spec_boitier.rs`.
            vider(&mut courant, &mut nombres, quoi);
            courant.push(c);
        } else if c.is_ascii_alphabetic() {
            vider(&mut courant, &mut nombres, quoi);
            assert!(
                matches!(c, 'M' | 'L' | 'Z'),
                "{quoi} : commande « {c} » dans « {commandes} ». Une sparkline est une polyligne en \
                 coordonnées **absolues** : une commande relative rendrait des écarts et non des \
                 positions, et aucune assertion sur la largeur du tracé ne voudrait plus rien dire"
            );
        } else {
            vider(&mut courant, &mut nombres, quoi);
        }
    }
    vider(&mut courant, &mut nombres, quoi);

    assert!(
        nombres.len().is_multiple_of(2),
        "{quoi} : {} nombres dans « {commandes} », donc un point dépareillé",
        nombres.len()
    );
    nombres.chunks(2).map(|p| (p[0], p[1])).collect()
}

/// Les points d'une courbe qu'on exige non vide.
fn courbe_de(quoi: &str, historique: &Historique, sonde: &str) -> Vec<(f32, f32)> {
    let commandes = commandes_de_courbe(historique, sonde);
    let lus = points(quoi, &commandes);
    assert!(
        lus.len() >= 2,
        "{quoi} : la courbe rend « {commandes} », soit {} point(s) — il n'y a rien à dessiner",
        lus.len()
    );
    lus
}

/// Une mesure qui monte d'un dixième de degré par seconde, en millidegrés.
fn qui_monte(i: usize) -> Releve {
    Releve::Valeur(30_000 + 100 * i as i32)
}

/// La même, à l'envers.
fn qui_descend(i: usize) -> Releve {
    Releve::Valeur(30_000 + 100 * (MEMOIRE - 1 - i) as i32)
}

/// Une mesure qui ne bouge pas — le liquide d'une machine au repos.
fn constante(_: usize) -> Releve {
    Releve::Valeur(40_200)
}

/// Vérifie qu'un tracé tient dans le repère `COURBE_ASPECT` × 1, bornes comprises.
fn tient_dans_le_repere(quoi: &str, lus: &[(f32, f32)]) {
    for (i, (x, y)) in lus.iter().enumerate() {
        assert!(
            x.is_finite() && y.is_finite(),
            "{quoi} : le point {i} n'est pas fini — ({x}, {y}). Un NaN dans un chemin SVG ne lève \
             rien : il efface le tracé, en silence"
        );
        assert!(
            (-EPSILON..=COURBE_ASPECT + EPSILON).contains(x),
            "{quoi} : l'abscisse du point {i} sort du repère — {x} hors de 0..={COURBE_ASPECT}"
        );
        assert!(
            (-EPSILON..=1.0 + EPSILON).contains(y),
            "{quoi} : l'ordonnée du point {i} sort du repère — {y} hors de 0..=1. Slint ne rogne \
             pas au viewbox : elle déborderait sur la tuile voisine"
        );
    }
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tous les tests qui suivent supposent que l'historique plein est bien plein, et que les trois
    // suites de mesures se distinguent l'une de l'autre. Si l'un de ces repères se dégradait,
    // plusieurs tests deviendraient vrais sans rien vérifier — et personne ne le verrait.
    assert_eq!(
        historique_plein(CPU, qui_monte).courbe(CPU).len(),
        MEMOIRE,
        "l'historique plein doit porter les deux minutes entières, sinon l'exigence de largeur \
         pleine se vérifierait sur une courbe partielle"
    );
    assert_ne!(
        qui_monte(0),
        qui_monte(MEMOIRE - 1),
        "une suite croissante doit croître"
    );
    assert_eq!(
        qui_descend(0),
        qui_monte(MEMOIRE - 1),
        "la suite décroissante est la croissante retournée"
    );
    assert_eq!(
        constante(0),
        constante(MEMOIRE - 1),
        "une suite constante doit être constante"
    );
}

// ---------------------------------------------------------------------------
// 1 — le rapport est public, et il est plus large que haut
// ---------------------------------------------------------------------------

// ⚠️ `assertions_on_constants` reproche aux trois assertions ci-dessous d'avoir
// une valeur connue à la compilation, donc d'être « toujours vraies ». C'est
// précisément ce que ce test existe pour garantir : elles sont vraies de la
// valeur d'aujourd'hui, et le test est là pour qu'elles le restent de celle de
// demain. L'exception est donc posée ici plutôt que l'assertion réécrite — un
// `const { assert!(…) }` perdrait le message, qui nomme le chiffre fautif.
//
// Seule modification apportée à ce fichier après son écriture, et elle ne touche
// aucune assertion.
#[allow(clippy::assertions_on_constants)]
#[test]
fn le_rapport_de_la_tuile_est_public_et_plus_large_que_haut() {
    // Critère d'acceptation n° 3 : « le rapport largeur/hauteur n'est écrit qu'à un seul endroit ».
    // Il ne peut l'être que s'il est **publié** : la fenêtre le lit pour son `viewbox` et pour la
    // hauteur de sa tuile, et ce fichier le lit pour vérifier que le tracé s'y accorde. Un chiffre
    // gardé privé forcerait la fenêtre à en recopier un second dans le `.slint` — la divergence que
    // l'issue nomme, et dont le symptôme est muet.
    assert!(
        COURBE_ASPECT.is_finite(),
        "le rapport doit être un nombre : {COURBE_ASPECT}"
    );

    // ⚠️ **Strictement plus grand que un, sinon le défaut n'est pas corrigé.** Un rapport de 1 est
    // le carré unité d'aujourd'hui, et la tuile resterait vide aux deux tiers.
    assert!(
        COURBE_ASPECT > 1.0,
        "une tuile de sonde est plus large que haute — l'issue la mesure à ~140 px pour une \
         soixantaine de courbe. Un rapport de {COURBE_ASPECT} laisse le tracé écrasé dans un carré, \
         c'est-à-dire le défaut intact"
    );

    // ⚠️ **Et ce n'est pas celui de #113.** L'issue est explicite : « les tuiles de sonde n'ont pas
    // la même forme que le cadre de la courbe de régulation. Le rapport de #113 ne se recopie donc
    // pas : c'est une seconde constante, avec sa propre valeur. » Réutiliser `TRACE_ASPECT`
    // donnerait une tuile au mauvais rapport, et le tracé y serait de nouveau faux — sans que rien
    // ne le dise.
    assert!(
        (COURBE_ASPECT - TRACE_ASPECT).abs() > EPSILON,
        "le rapport des sondes vaut celui de la régulation ({TRACE_ASPECT}) : le cadre de #113 fait \
         359 × 88 px, une tuile de sonde ~140 px de large. Deux formes différentes ne partagent pas \
         un rapport"
    );
}

// ---------------------------------------------------------------------------
// 2 — le tracé est une polyligne, un point par mesure
// ---------------------------------------------------------------------------

#[test]
fn le_trace_est_une_polyligne_absolue_d_un_point_par_mesure() {
    // Arbitrage n° 1 en tête de fichier. Ce n'est pas un critère écrit de l'issue : c'est la
    // convention sans laquelle aucun des tests suivants ne veut dire quoi que ce soit. Une aire
    // remplie ajouterait des points de fond de tuile — que personne n'a mesurés — et « toutes les
    // ordonnées sont égales sur une série constante » deviendrait faux pour une bonne raison.
    //
    // `points()` refuse déjà toute commande autre que `M`, `L` et `Z` ; ce test ajoute le compte.
    let historique = historique_plein(CPU, qui_monte);
    let lus = courbe_de("cent vingt mesures croissantes", &historique, CPU);
    assert_eq!(
        lus.len(),
        MEMOIRE,
        "un point par mesure lisible : {MEMOIRE} attendus, {} tracés",
        lus.len()
    );
}

// ---------------------------------------------------------------------------
// 3 — le tracé couvre toute la largeur du repère
// ---------------------------------------------------------------------------

#[test]
fn le_trace_d_une_sonde_couvre_toute_la_largeur_du_repere() {
    // Test d'intention n° 1 de l'issue : « le tracé d'une sonde couvre toute la largeur du repère
    // dans lequel il est émis ». Critères d'acceptation n° 1 et n° 3 réunis — c'est ici que
    // « le rapport publié à la fenêtre est celui dans lequel le tracé a été émis » se vérifie : si
    // les deux divergeaient, l'abscisse maximale ne tomberait pas sur `COURBE_ASPECT`.
    //
    // ⚠️ **Sur un historique plein**, voir l'arbitrage n° 3 : une courbe partielle est hors scope.
    for (quoi, historique) in [
        ("une série croissante", historique_plein(CPU, qui_monte)),
        ("une série décroissante", historique_plein(CPU, qui_descend)),
        ("une série constante", historique_plein(CPU, constante)),
    ] {
        let lus = courbe_de(quoi, &historique, CPU);
        let mini = lus.iter().map(|(x, _)| *x).fold(f32::INFINITY, f32::min);
        let maxi = lus
            .iter()
            .map(|(x, _)| *x)
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(
            (mini - 0.0).abs() <= EPSILON,
            "{quoi} : la plus petite abscisse vaut {mini} et non 0 — le tracé ne part pas du bord \
             gauche de la tuile"
        );
        assert!(
            (maxi - COURBE_ASPECT).abs() <= EPSILON,
            "{quoi} : la plus grande abscisse vaut {maxi} et non {COURBE_ASPECT}. Slint met le \
             `Path` à l'échelle uniformément : un tracé émis plus étroit que son cadre reste \
             centré, et le reste de la tuile demeure vide"
        );

        // ⚠️ Et surtout : pas 1,0. C'est le carré unité d'aujourd'hui, le défaut exact que l'issue
        // corrige — mesuré à 24 % du cadre sur la courbe de régulation de #113.
        assert!(
            (maxi - 1.0).abs() > EPSILON,
            "{quoi} : le tracé s'arrête à 1,0, donc il est encore émis dans le carré unité. Il n'en \
             occuperait qu'une fraction de la tuile, centré, et rien ne le dirait — une sparkline \
             comprimée ressemble encore à une sparkline"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — les ordonnées restent dans le repère
// ---------------------------------------------------------------------------

#[test]
fn les_ordonnees_restent_dans_le_repere_bornes_comprises() {
    // Test d'intention n° 2 de l'issue : « ses ordonnées restent dans le repère, bornes comprises ».
    // Critère d'acceptation n° 2 : « elle ne déborde par aucun bord ».
    //
    // ⚠️ **C'est le piège que l'issue nomme** : « ne corriger que le `viewbox-height` ne marche
    // pas — essayé et mesuré : la courbe déborde par le haut au lieu de s'étirer, puisque rien ne
    // la rogne ». Slint ne rognant pas au viewbox, une ordonnée hors de `0..=1` se dessine
    // réellement, par-dessus la tuile voisine.
    for (quoi, historique) in [
        ("une série croissante", historique_plein(CPU, qui_monte)),
        ("une série décroissante", historique_plein(CPU, qui_descend)),
        ("une série constante", historique_plein(CPU, constante)),
        (
            "une série qui traverse le zéro",
            historique_plein(CPU, |i| Releve::Valeur(-60_000 + 1_000 * i as i32)),
        ),
        (
            "une série de deux valeurs qui alternent",
            historique_plein(CPU, |i| {
                Releve::Valeur(if i.is_multiple_of(2) { 30_000 } else { 90_000 })
            }),
        ),
    ] {
        let lus = courbe_de(quoi, &historique, CPU);
        tient_dans_le_repere(quoi, &lus);
    }
}

// ---------------------------------------------------------------------------
// 5 — une série constante est un trait horizontal
// ---------------------------------------------------------------------------

#[test]
fn une_serie_constante_produit_un_trait_horizontal() {
    // Le liquide d'une machine au repos ne bouge pas d'un millidegré pendant deux minutes : sa
    // sparkline doit être un trait droit. C'est aussi le cas où l'amplitude est **nulle**, donc
    // celui où une mise à l'échelle naïve divise par zéro — `spec_historique.rs` le dit en toutes
    // lettres : « la courbe qui ne peut pas diviser par une amplitude nulle a besoin d'une marge :
    // c'est une décision de tracé ». Un NaN ne lève rien dans un chemin SVG, il efface le tracé.
    //
    // Ce test n'impose **pas** la hauteur à laquelle le trait se pose — milieu, haut ou bas est une
    // décision d'affichage que l'issue met hors scope. Il impose qu'il soit horizontal, fini, et
    // dans le repère.
    let historique = historique_plein(CPU, constante);
    let lus = courbe_de("une série constante", &historique, CPU);
    tient_dans_le_repere("une série constante", &lus);

    let (_, premiere) = lus[0];
    for (i, (x, y)) in lus.iter().enumerate() {
        assert!(
            (y - premiere).abs() <= EPSILON,
            "une série constante : le point {i} est à ({x}, {y}) alors que le premier est à \
             {premiere}. Cent vingt mesures identiques ne dessinent pas un relief"
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — une série qui monte se dessine en montant
// ---------------------------------------------------------------------------

#[test]
fn une_serie_croissante_monte_donc_ses_ordonnees_descendent() {
    // Arbitrage n° 2 en tête de fichier, et il vaut d'être écrit ici aussi : **en coordonnées
    // d'écran, `y` croît vers le bas.** Une courbe qui « monte » visuellement — des mesures qui
    // augmentent — a donc des ordonnées qui **décroissent** : la plus vieille mesure, la plus
    // basse, est proche de `y = 1` (le bas de la tuile) ; la plus récente, la plus haute, est
    // proche de `y = 0` (le haut).
    //
    // Une erreur de signe ici produirait un tracé parfaitement borné, parfaitement lisse, et qui
    // montrerait le CPU se refroidir pendant qu'il chauffe. Aucun autre test de ce fichier ne la
    // verrait — c'est le même garde-fou que `spec_courbe_fenetre.rs` s'est donné sur le tracé de la
    // régulation.
    let montante = historique_plein(CPU, qui_monte);
    let lus = courbe_de("une série croissante", &montante, CPU);
    tient_dans_le_repere("une série croissante", &lus);

    let mut precedente = f32::INFINITY;
    for (i, (x, y)) in lus.iter().enumerate() {
        assert!(
            *y <= precedente + EPSILON,
            "une série croissante : le point {i} est à ({x}, {y}) après {precedente}. Une mesure \
             qui augmente doit faire *remonter* le tracé, donc faire *baisser* l'ordonnée"
        );
        precedente = *y;
    }
    let (_, debut) = lus[0];
    let (_, fin) = lus[lus.len() - 1];
    assert!(
        debut - fin > EPSILON,
        "une série croissante : le tracé part de {debut} et finit à {fin} — il ne monte nulle part"
    );

    // Et l'exacte réciproque : une série qui redescend redescend.
    let descendante = historique_plein(COOLANT, qui_descend);
    let lus = courbe_de("une série décroissante", &descendante, COOLANT);
    tient_dans_le_repere("une série décroissante", &lus);

    let mut precedente = f32::NEG_INFINITY;
    for (i, (x, y)) in lus.iter().enumerate() {
        assert!(
            *y >= precedente - EPSILON,
            "une série décroissante : le point {i} est à ({x}, {y}) après {precedente}"
        );
        precedente = *y;
    }
    let (_, debut) = lus[0];
    let (_, fin) = lus[lus.len() - 1];
    assert!(
        fin - debut > EPSILON,
        "une série décroissante : le tracé part de {debut} et finit à {fin} — il ne descend nulle \
         part"
    );
}

// ---------------------------------------------------------------------------
// 7 — les cas dégénérés ne produisent aucune commande invalide
// ---------------------------------------------------------------------------

#[test]
fn une_serie_vide_ou_a_un_seul_point_ne_produit_aucune_commande_invalide() {
    // Une sonde qui vient d'apparaître n'a qu'une mesure, et la fenêtre demande sa courbe au tour
    // suivant — donc avant même la deuxième (#31 : « une sonde apparue il y a trois secondes rend
    // trois relevés, pas cent vingt »). Une sonde jamais vue n'en a aucune.
    //
    // Ce test n'exige **aucun** résultat en particulier : la chaîne vide est une réponse légitime —
    // le `.slint` teste déjà `if sonde.courbe != ""` — et un point unique en est une autre. Ce
    // qu'il refuse, c'est une commande invalide : un `NaN` ou un infini, qui ne lèvent rien et
    // effacent le tracé en silence ; une division par une amplitude nulle en est la source
    // naturelle, et c'est exactement ce que produit un historique d'un seul relevé.
    let vide = Historique::nouvel();
    let une_seule = historique_de(CPU, 1, qui_monte);
    let deux = historique_de(CPU, 2, qui_monte);
    let trois_dont_une_illisible = {
        let mut h = Historique::nouvel();
        h.noter(CPU, Releve::Valeur(45_000));
        h.noter(CPU, Releve::Illisible);
        h.noter(CPU, Releve::Valeur(46_000));
        h
    };

    for (quoi, historique, sonde) in [
        ("un historique vide", &vide, CPU),
        ("une sonde jamais vue", &deux, "k10temp:inexistante"),
        ("une seule mesure", &une_seule, CPU),
        ("deux mesures", &deux, CPU),
        (
            "une mesure, un trou, une mesure",
            &trois_dont_une_illisible,
            CPU,
        ),
    ] {
        let commandes = commandes_de_courbe(historique, sonde);
        let lus = points(quoi, &commandes);
        tient_dans_le_repere(quoi, &lus);
    }
}

#[test]
fn une_sonde_illisible_se_comporte_comme_les_quatre_autres() {
    // Critère d'acceptation n° 4 : « les cinq tuiles se comportent pareil, la sonde illisible
    // comprise ». Une sonde muette n'a **aucune valeur lisible**, donc aucune borne à laquelle
    // s'échelonner (`bornes` rend `None`, #31) : c'est le second chemin par lequel un `NaN`
    // arriverait dans les commandes, et il effacerait le tracé sans un message.
    //
    // Comme ci-dessus, aucun résultat n'est imposé — la chaîne vide dit très bien « rien à
    // tracer ». Ce qui est imposé, c'est que la tuile ne devienne pas un cas particulier qui
    // déborde ou qui panique.
    let muette = historique_plein(CPU, |_| Releve::Illisible);
    let commandes = commandes_de_courbe(&muette, CPU);
    let lus = points("une sonde entièrement illisible", &commandes);
    tient_dans_le_repere("une sonde entièrement illisible", &lus);

    // Et une sonde muette n'emporte pas sa voisine : la tuile du liquide garde sa largeur pleine.
    let mut deux_sondes = historique_plein(CPU, |_| Releve::Illisible);
    for i in 0..MEMOIRE {
        deux_sondes.noter(COOLANT, qui_monte(i));
    }
    let lus = courbe_de("la voisine d'une sonde muette", &deux_sondes, COOLANT);
    tient_dans_le_repere("la voisine d'une sonde muette", &lus);
    let maxi = lus
        .iter()
        .map(|(x, _)| *x)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        (maxi - COURBE_ASPECT).abs() <= EPSILON,
        "la voisine d'une sonde muette : la plus grande abscisse vaut {maxi} et non \
         {COURBE_ASPECT} — les cinq tuiles doivent se comporter pareil"
    );
}

// ---------------------------------------------------------------------------
// 8 — le tracé est pur
// ---------------------------------------------------------------------------

#[test]
fn deux_appels_sur_le_meme_historique_rendent_le_meme_trace() {
    // Ce n'est pas un critère écrit ; c'est ce qui rend l'image d'aperçu utilisable comme mesure.
    // L'issue exige que le premier critère soit vérifié « sur l'image d'aperçu et non à l'œil » :
    // un tracé qui dépendrait d'une horloge, d'un état caché ou de l'ordre des appels rendrait
    // cette vérification irreproductible — et la mesure au pixel, sans valeur.
    let historique = historique_plein(CPU, qui_monte);
    assert_eq!(
        commandes_de_courbe(&historique, CPU),
        commandes_de_courbe(&historique, CPU),
        "deux appels rendent deux tracés différents : le tracé n'est pas pur"
    );

    // Et deux sondes aux mesures différentes ne rendent pas le même tracé — sinon la tuile
    // afficherait la courbe de sa voisine, ce qu'aucune assertion de forme ne verrait.
    let mut deux_sondes = historique_plein(CPU, qui_monte);
    for i in 0..MEMOIRE {
        deux_sondes.noter(COOLANT, qui_descend(i));
    }
    assert_ne!(
        commandes_de_courbe(&deux_sondes, CPU),
        commandes_de_courbe(&deux_sondes, COOLANT),
        "deux sondes aux mesures opposées rendent le même tracé"
    );
}
