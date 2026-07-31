//! Tests d'intention de la cadence de rendu du démon (issue #17).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #17 et son commentaire « Contrat d'API »
//! seuls. À l'écriture de ce fichier, le module `cadence` n'existe pas. Si l'un de ces tests
//! échoue après implémentation, c'est le code qu'on corrige.
//!
//! ## L'horloge est simulée, et ces tests ne dorment jamais
//!
//! Contrat d'API — `Cadence` « ne tient aucune horloge : c'est l'appelant qui lui donne l'instant,
//! ce qui rend le rattrapage et le décrochage testables sans dormir ». C'est toute la raison
//! d'être de ce découpage : une boucle de rendu qui appelle `Instant::now()` elle-même ne se teste
//! qu'en dormant, donc lentement, donc mal — et le cas qui compte, le décrochage de 500 ms, ne se
//! reproduit pas à volonté.
//!
//! Ici le temps avance par pas d'une milliseconde, sous le contrôle du test. Aucun
//! `std::thread::sleep`, aucun `Instant::now`, aucune tolérance statistique.
//!
//! ## Le défaut visé : la rafale de rattrapage
//!
//! Contrat d'API — « **`Produire` ne rend jamais un nombre d'images à rattraper.** Une boucle en
//! retard **saute**, elle ne rattrape pas : sinon une animation qui décroche part en avance rapide
//! dès que la charge retombe, ce qui se voit immédiatement à l'œil. `sautees` sert à le
//! journaliser, pas à produire. »
//!
//! C'est le défaut classique des boucles de rendu, et il ne se signale par aucune erreur : la
//! machine a rattrapé son retard, tous les compteurs sont bons, et l'animation a fait un bond.
//! Sur la RAM Corsair — la seule cible temps réel du projet, le contrôleur ne sachant pas animer
//! seul — le bond passe par le bus SMBus, à raison d'une transaction par image rattrapée.
//!
//! ## Deux points que le contrat laisse ouverts, et que ces tests tranchent
//!
//! 1. **La première image part tout de suite.** Un `tick(0)` sur une `Cadence` neuve produit, il
//!    n'attend pas une première période. Sinon l'éclairage appliqué au démarrage de la machine —
//!    un critère d'acceptation de l'issue — arriverait avec un retard gratuit.
//! 2. **`sautees` est un compte, pas une consigne.** Le contrat n'en fixe pas la valeur exacte au
//!    nanoseconde près ; les tests l'encadrent (≈ 14 pour un retard de 500 ms à 30 img/s) plutôt
//!    que de le figer, parce que sa valeur dépend de l'arrondi de la période — et parce que ce
//!    qu'on exige de lui, c'est d'être non nul et de ne pas servir à produire.

use reverb_daemon::cadence::{Cadence, Tick};
use std::time::Duration;

/// La cadence de l'issue : « une animation tourne à **30 images/s sur les dix ventilateurs et les
/// quatre barrettes**, sans saccade visible à l'œil » (critère d'acceptation).
const IMAGES_PAR_SECONDE: u32 = 30;

/// La période nominale, ≈ 33,333 ms.
fn periode() -> Duration {
    Duration::from_secs(1) / IMAGES_PAR_SECONDE
}

/// Une seconde simulée, plus une demi-période.
///
/// La fenêtre de comptage ne doit **pas** finir sur une échéance, sinon le résultat dépend de
/// l'arrondi de la période au nanoseconde près : 1 s vaut exactement 30 périodes, et selon que
/// 1/30 s est tronqué ou arrondi vers le haut, la trentième échéance tombe un chouïa avant ou un
/// chouïa après la seconde. En arrêtant le comptage 16 ms plus loin, la trentième est dedans et la
/// trente-et-unième (≈ 1 033 ms) dehors, quel que soit l'arrondi.
const FENETRE_MS: u64 = 1_016;

/// Les écarts admissibles entre deux images consécutives, sur une horloge échantillonnée à la
/// milliseconde. 1/30 s vaut 33,33 ms : les crossings tombent donc à 33 ou 34 ms d'intervalle, et
/// jamais ailleurs.
const ECARTS_ADMIS: std::ops::RangeInclusive<u64> = 33..=34;

/// Fait avancer l'horloge d'une milliseconde à la fois sur `[debut, fin]`, et rend les instants
/// (en millisecondes) où une image a été produite.
///
/// Vérifie au passage les deux propriétés qui valent sur tout l'intervalle, quelle que soit la
/// phase : `Attendre` ne demande jamais plus d'une période, et une attente ne dépasse jamais la
/// prochaine échéance — « dormir **au plus** ce délai avant de rappeler `tick` » (contrat d'API).
fn balaye(cadence: &mut Cadence, debut: u64, fin: u64) -> Vec<u64> {
    let mut productions = Vec::new();
    let mut attentes = Vec::new();

    for ms in debut..=fin {
        match cadence.tick(Duration::from_millis(ms)) {
            Tick::Produire { sautees } => {
                assert_eq!(
                    sautees, 0,
                    "à {ms} ms, l'horloge avance d'une milliseconde à la fois : rien ne peut être \
                     sauté"
                );
                productions.push(ms);
            }
            Tick::Attendre(delai) => {
                assert!(
                    delai <= periode(),
                    "à {ms} ms, attendre {delai:?} dépasse une période ({:?}) — la prochaine image \
                     serait manquée",
                    periode()
                );
                attentes.push((ms, delai));
            }
        }
    }

    // Les deux listes sont croissantes : un seul parcours suffit à confronter chaque attente à la
    // prochaine image.
    let mut suivante = 0;
    for (ms, delai) in attentes {
        while suivante < productions.len() && productions[suivante] <= ms {
            suivante += 1;
        }
        if let Some(&prochaine) = productions.get(suivante) {
            assert!(
                Duration::from_millis(ms) + delai <= Duration::from_millis(prochaine),
                "à {ms} ms, dormir {delai:?} ferait rater l'image de {prochaine} ms"
            );
        }
    }

    productions
}

/// Vérifie que des images se suivent au rythme nominal : jamais deux d'affilée, jamais un trou.
fn rythme_nominal(productions: &[u64], depuis: u64) {
    let mut precedente = depuis;
    for &ms in productions {
        let ecart = ms - precedente;
        assert!(
            ECARTS_ADMIS.contains(&ecart),
            "l'image de {ms} ms suit la précédente de {ecart} ms, hors de {ECARTS_ADMIS:?} — à \
             {IMAGES_PAR_SECONDE} img/s, deux images sont séparées de 33 ou 34 ms"
        );
        precedente = ms;
    }
}

// ---------------------------------------------------------------------------
// 6 — trente images par seconde, ni vingt-neuf ni trente et une
// ---------------------------------------------------------------------------

#[test]
fn trente_images_par_seconde_simulee_et_rien_entre_deux_echeances() {
    // Issue #17, critère d'acceptation — « une animation tourne à **30 images/s sur les dix
    // ventilateurs et les quatre barrettes**, sans saccade visible à l'œil ».
    // Contrat d'API — `Cadence::new(images_par_seconde)` puis `tick(maintenant)`, où `maintenant`
    // est « un temps écoulé depuis le démarrage, monotone et croissant ».
    //
    // 29 images par seconde, c'est une image perdue toutes les secondes — un à-coup périodique,
    // exactement ce que le critère « sans saccade visible » interdit. 31, c'est une image de trop,
    // donc une transaction SMBus de trop par seconde sur le seul bus partagé du projet.
    let mut cadence = Cadence::new(IMAGES_PAR_SECONDE);

    // La première image part tout de suite : à l'allumage de la machine, l'éclairage ne doit pas
    // attendre une période pour apparaître (critère d'acceptation « l'éclairage est appliqué au
    // démarrage de la machine »).
    assert_eq!(
        cadence.tick(Duration::ZERO),
        Tick::Produire { sautees: 0 },
        "une cadence neuve produit sa première image sans délai"
    );

    let productions = balaye(&mut cadence, 1, FENETRE_MS);

    assert_eq!(
        productions.len(),
        IMAGES_PAR_SECONDE as usize,
        "exactement {IMAGES_PAR_SECONDE} images sur la seconde simulée qui suit la première — \
         obtenu {productions:?}"
    );

    // « et rien entre deux échéances » : les images ne se touchent pas et ne s'espacent pas.
    rythme_nominal(&productions, 0);

    // La dernière tombe bien au bout de la seconde, pas avant : une cadence qui produirait ses
    // trente images en 500 ms puis se tairait passerait le compte, pas celui-ci.
    let derniere = *productions.last().expect("trente images");
    assert!(
        (990..=1_010).contains(&derniere),
        "la trentième image doit tomber vers 1 000 ms, pas à {derniere} ms"
    );

    // Cent secondes simulées, parce qu'une seconde ne suffit pas à voir une dérive.
    //
    // Une période arrondie à la milliseconde entière — 33 ms, le raccourci qui vient
    // naturellement — donne 30,30 img/s : sur une seconde, le compte reste juste et le test
    // passerait. Sur cent, ce sont trente images de trop. Et une échéance recalée sur l'instant
    // reçu (`prochaine = maintenant + période`) au lieu de l'échéance précédente accumule le
    // retard d'échantillonnage à chaque image, et en perd une soixantaine.
    //
    // Les bornes sont décalées d'une demi-période des secondes rondes, pour la même raison qu'en
    // haut de ce fichier : aucun bord de fenêtre ne doit tomber sur une échéance.
    let suite = balaye(&mut cadence, FENETRE_MS + 1, 100_000 + FENETRE_MS - 1_000);
    rythme_nominal(&suite, derniere);

    let total = productions.len() + suite.len();
    assert_eq!(
        total,
        100 * IMAGES_PAR_SECONDE as usize,
        "cent secondes simulées valent {} images, pas {total} — la cadence dérive",
        100 * IMAGES_PAR_SECONDE
    );

    let derniere = *suite.last().expect("la cadence continue");
    assert!(
        (99_990..=100_010).contains(&derniere),
        "la trois-millième image doit tomber vers 100 000 ms, pas à {derniere} ms"
    );
}

// ---------------------------------------------------------------------------
// 7 — un retard ne se rattrape pas, il se saute
// ---------------------------------------------------------------------------

#[test]
fn un_retard_de_cinq_cents_millisecondes_ne_produit_qu_une_image() {
    // Contrat d'API — « `Produire` ne rend jamais un nombre d'images à rattraper. Une boucle en
    // retard **saute**, elle ne rattrape pas : sinon une animation qui décroche part en avance
    // rapide dès que la charge retombe, ce qui se voit immédiatement à l'œil. `sautees` sert à le
    // journaliser, pas à produire. »
    let mut cadence = Cadence::new(IMAGES_PAR_SECONDE);
    assert_eq!(cadence.tick(Duration::ZERO), Tick::Produire { sautees: 0 });

    // Le décrochage : la boucle a été bloquée 500 ms — une lecture hwmon lente, une transaction
    // SMBus qui traîne, la machine qui swappe.
    let saut = Duration::from_millis(500);
    let Tick::Produire { sautees } = cadence.tick(saut) else {
        panic!("après 500 ms de retard, il est largement temps de produire une image");
    };

    // 500 ms valent quinze périodes ; une image est produite, les autres sont perdues. La valeur
    // exacte dépend de l'arrondi de la période au nanoseconde, d'où l'encadrement : ce qu'on exige
    // de `sautees`, c'est d'être renseigné — il sert au journal, pas au rendu.
    assert!(
        (13..=15).contains(&sautees),
        "un retard de 500 ms à {IMAGES_PAR_SECONDE} img/s perd une quinzaine d'images, pas \
         {sautees}"
    );

    // **Le point du test.** Rappelée au même instant, la cadence n'accouche pas des quatorze
    // images manquantes : elle attend la prochaine échéance. C'est ce qu'une boucle réelle fait —
    // produire, écrire sur le bus, rappeler `tick` — et c'est là que la rafale se produirait.
    for rappel in 0..20 {
        let reponse = cadence.tick(saut);
        assert!(
            matches!(reponse, Tick::Attendre(_)),
            "rappel n° {rappel} au même instant : {reponse:?} — une seule image par échéance, \
             jamais une rafale de rattrapage"
        );
    }

    // Et pas davantage dans les vingt millisecondes qui suivent : la prochaine échéance est à
    // ≈ 533 ms, qu'on recale la cadence sur l'instant du saut ou qu'on la laisse sur sa grille
    // d'origine.
    for ms in 501..=520 {
        let reponse = cadence.tick(Duration::from_millis(ms));
        assert!(
            matches!(reponse, Tick::Attendre(_)),
            "à {ms} ms, {reponse:?} : le retard est déjà soldé, il n'y a rien à rattraper"
        );
    }

    // Puis le rythme nominal reprend, sans dette : exactement trente images sur la seconde
    // simulée qui suit le décrochage.
    let productions = balaye(&mut cadence, 521, 500 + FENETRE_MS);
    assert_eq!(
        productions.len(),
        IMAGES_PAR_SECONDE as usize,
        "après le saut, la cadence reprend à {IMAGES_PAR_SECONDE} img/s — obtenu {productions:?}"
    );
    rythme_nominal(&productions, 500);

    let premiere = *productions.first().expect("trente images");
    assert!(
        (530..=537).contains(&premiere),
        "la première image d'après le saut doit tomber une période après lui, pas à {premiere} ms"
    );

    // La même propriété sur un décrochage bien plus long, pour qu'elle ne tienne pas au hasard des
    // 500 ms : deux secondes d'arrêt valent soixante images perdues, et il n'en sort **qu'une**.
    let mut longue = Cadence::new(IMAGES_PAR_SECONDE);
    assert_eq!(longue.tick(Duration::ZERO), Tick::Produire { sautees: 0 });

    let arret = Duration::from_secs(2);
    let Tick::Produire { sautees } = longue.tick(arret) else {
        panic!("après deux secondes d'arrêt, une image est due");
    };
    assert!(
        (58..=60).contains(&sautees),
        "deux secondes à {IMAGES_PAR_SECONDE} img/s perdent une soixantaine d'images, pas {sautees}"
    );
    for rappel in 0..20 {
        let reponse = longue.tick(arret);
        assert!(
            matches!(reponse, Tick::Attendre(_)),
            "rappel n° {rappel} : une image, et une seule, quelle que soit la longueur du trou"
        );
    }
}
