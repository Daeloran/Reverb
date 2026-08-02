//! Tests d'intention du plafond d'échecs de la dalle du Kraken — issue #70.
//!
//! Écrits **avant** l'implémentation, depuis l'issue #70 seule — aucun fichier de
//! `crates/reverb-daemon/src/` n'a été lu pour les produire, hors les signatures publiques d'[`Etat`]
//! et d'[`Affichage`], reprises telles quelles de `spec_ecran.rs` (#33). À l'écriture de ce fichier,
//! `Vigie`, `Verdict` et `ECHECS_AVANT_ABANDON` **n'existent pas** : la compilation doit échouer, et
//! c'est la phase rouge.
//!
//! Rien ici n'ouvre de périphérique, ne dort, ni n'écrit sur un bus. L'écriture vers la dalle est
//! **injectée** — une fermeture qui rend `Ok` ou `Err` selon le scénario —, exactement comme le
//! repli de `requetes_vers_la_cible` l'est dans `spec_limiteur.rs` (#47). C'est ce qui rend la
//! décision vérifiable sans matériel, et c'est aussi ce qui rend *comptable* la seule propriété que
//! l'issue exige et qu'une égalité ne saurait montrer : après l'abandon, **plus une seule
//! écriture**.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Journal de SHYNAEL, à chacun des trois derniers démarrages :
//!
//! ```text
//! reverbd[663734]: attention : luminosité de l'écran : Connection timed out (os error 110)
//! ```
//!
//! Cinq secondes de délai par tentative (#68), et le démon **recommence** : une image fixe se
//! réémet toutes les 25 s, un cadran toutes les 2 s, un GIF toutes les 100 ms (README). Rien ne
//! compte les échecs, rien ne s'arrête. C'est du bus consommé, du démon gelé, et une insistance sur
//! un contrôleur que la spec signale déjà comme sensible aux envois mal formés
//! (`docs/SPEC-KRAKEN-LCD.md`).
//!
//! ## Le piège de conception, et il est nommé par l'issue
//!
//! **Consécutifs, et non cumulés.** Un compteur cumulé passe tous les tests courts — il abandonne
//! bien après N échecs d'affilée, puisqu'une suite de N échecs est aussi une somme de N. Il ne se
//! trahit qu'après des jours : quelques hoquets sans conséquence, espacés de dix minutes, finissent
//! par éteindre un écran qui marche, et personne ne relie la panne à sa cause. D'où
//! [`un_echec_isole_ne_borne_pas_la_vie_de_la_dalle`], qui accumule des dizaines d'échecs sans
//! jamais en aligner deux, et qui est le seul test de ce fichier que le mauvais compteur ne passe
//! pas.
//!
//! ## Ce que ce fichier fige
//!
//! 1. **[`ECHECS_AVANT_ABANDON`] échecs d'affilée arrêtent l'émission**, et un de moins ne l'arrête
//!    pas. Les deux moitiés sont exigées : sans la seconde, une implémentation qui renonce au
//!    premier refus passerait.
//! 2. **Un succès remet le compte à zéro.** Le compteur mesure une suite, pas un total.
//! 3. **L'abandon est prononcé une seule fois**, et c'est le [`Verdict`] qui le dit — l'appelant n'a
//!    pas à s'en souvenir pour ne journaliser qu'une ligne.
//! 4. **Après l'abandon, la fermeture d'écriture n'est plus appelée du tout** — pas même pour
//!    « voir si ça remarche ». Le rétablissement est explicite, ou n'est pas.
//! 5. **Une commande `screen` relance l'émission et remet le compte à zéro**, et il faut de nouveau
//!    [`ECHECS_AVANT_ABANDON`] échecs pour ré-abandonner.
//! 6. **Le verdict ne dit rien d'autre que ce que la dalle doit devenir.** Ni l'état persisté, ni
//!    l'éclairage, ni les ventilateurs, ni les zones n'apparaissent dans son vocabulaire.
//!
//! ## Ce que le contrat laisse ouvert, et que ces tests tranchent
//!
//! 1. **[`ECHECS_AVANT_ABANDON`] vaut 3**, comme la ligne de journal donnée en exemple par l'issue.
//!    À cinq secondes de délai par tentative (#68), trois refus valent quinze secondes perdues avant
//!    de rendre la main — assez pour qu'un contrôleur qui bafouille une fois ou deux s'en remette,
//!    trop peu pour qu'une dalle morte gèle le démon une minute. Les tests **encadrent** la valeur
//!    (2 à 5) au lieu de la pointer, comme `spec_limiteur.rs` le fait de son intervalle : un
//!    ajustement mesuré sur le matériel n'a pas à se négocier avec un test d'intention. Mais la
//!    borne basse, elle, est dure : à 1, « N-1 échecs n'arrêtent pas » deviendrait « 0 échec
//!    n'arrête pas », c'est-à-dire rien du tout.
//! 2. **L'abandon nomme la *dernière* erreur vue**, pas la première. C'est celle qui a fait déborder
//!    le compte, et c'est la plus récente information sur l'état du bus. Les échecs des tests
//!    portent donc des codes distincts, sans quoi le choix serait invérifiable.
//! 3. **Un échec sous le plafond porte lui aussi son erreur.** Le démon journalise déjà
//!    « attention : luminosité de l'écran : … » à chaque tentative, et #70 ne demande pas de le
//!    retirer — seulement de le borner. Un verdict qui perdrait l'erreur obligerait l'appelant à la
//!    garder de son côté, et rouvrirait la porte au comptage dispersé que le premier critère ferme.
//! 4. **Une relance sur une vigie qui n'a jamais abandonné est sans effet visible**, hors la remise
//!    à zéro du compte. Une commande `screen` est fréquente ; elle ne doit rien casser.
//! 5. **Une vigie neuve émet.** Un premier démarrage n'a essuyé aucun refus : il n'a aucune raison
//!    de se taire.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **La boucle de réémission elle-même** — 25 s, 2 s, 100 ms. C'est du service, pas une fonction,
//!   et `spec_ecran.rs` a déjà tranché de ne pas la figer.
//! - **Le texte exact de la ligne de journal.** L'issue en donne une forme, pas une chaîne ; ce
//!   fichier exige que l'erreur y soit **nommable**, c'est-à-dire portée par le verdict.
//! - **Réveiller le périphérique** par un `unbind`/`bind` USB : hors scope, l'issue le dit.
//! - **Le gel du démon par une sonde muette** (#68) et **la validation du format** (#69).
//! - **La cause première de la panne.** La reproduire délibérément rejouerait la panne sur le seul
//!   exemplaire de matériel du projet.

// Les assertions qui **encadrent** `ECHECS_AVANT_ABANDON` sont constantes une
// fois la constante écrite, et clippy les refuse à ce titre. C'est précisément
// leur intérêt : elles ne servent pas à observer une exécution mais à casser la
// compilation le jour où quelqu'un pose un plafond hors des bornes que #70 a
// raisonnées. Aucune assertion n'a été touchée pour lever ce lint — seul cet
// `allow` a été ajouté, après l'implémentation.
#![allow(clippy::assertions_on_constants)]

use std::cell::Cell;
use std::io;

use reverb_daemon::ecran::{Affichage, ECHECS_AVANT_ABANDON, Etat, Verdict, Vigie};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// `ETIMEDOUT`. C'est le code du journal de SHYNAEL : « Connection timed out (os error 110) ».
const ETIMEDOUT: i32 = 110;

/// `EIO`. Une seconde panne, distincte de la première : sans deux codes, « l'abandon nomme la
/// dernière erreur » ne se distinguerait pas de « il nomme la première ».
const EIO: i32 = 5;

/// `EPIPE`. Une troisième, pour qu'une suite d'échecs ait vraiment un milieu.
const EPIPE: i32 = 32;

/// Le plancher défendable du plafond.
///
/// À 1, la dalle s'éteindrait au premier hoquet — et le test « N-1 échecs n'arrêtent pas »
/// deviendrait « 0 échec n'arrête pas », qui ne vérifie rien. C'est la borne qui empêche une
/// implémentation trop nerveuse de passer en changeant la constante plutôt que le comportement.
const PLAFOND_MINIMAL: u32 = 2;

/// Le plafond au-delà duquel le remède coûterait plus que le mal.
///
/// Chaque tentative gèle le démon cinq secondes (#68). Cinq refus valent déjà vingt-cinq secondes
/// pendant lesquelles il ne répond ni à l'éclairage, ni aux sondes.
const PLAFOND_MAXIMAL: u32 = 5;

/// Le nombre de tours que le démon ferait en cent secondes avec un GIF à l'affiche : une image
/// toutes les cent millisecondes (README). C'est la cadence à laquelle une émission non bornée
/// martèle le contrôleur, et donc celle à laquelle il faut vérifier qu'elle s'est bien tue.
const TOURS_D_UN_GIF: u32 = 1_000;

/// Ce que la vigie doit encaisser sans broncher : une suite longue, ponctuée d'échecs isolés.
///
/// Soixante répétitions, parce qu'un compteur cumulé tiendrait sur trois ou quatre.
const LONGUE_VIE: u32 = 60;

/// L'état persisté qui sert de témoin : le GIF que `ecran.conf` portait au moment de la panne
/// (contexte de #69, rappelé par #70).
fn etat_temoin() -> Etat {
    Etat {
        luminosite: 50,
        affichage: Affichage::Gif("/home/nico/anims/pluie.gif".to_owned()),
    }
}

/// Le message que la bibliothèque standard associe à un code d'erreur système.
///
/// Calculé ici plutôt que recopié : `strerror` dépend de la locale, et un test qui écrirait
/// « Connection timed out » en dur passerait sur la machine de son auteur et nulle part ailleurs.
/// Seul le suffixe « (os error N) » est stable, et c'est celui qui identifie la panne.
fn message(code: i32) -> String {
    io::Error::from_raw_os_error(code).to_string()
}

// ---------------------------------------------------------------------------
// La dalle factice — elle compte, c'est tout son intérêt
// ---------------------------------------------------------------------------

/// Une dalle qui note chaque écriture qu'on lui demande, et qui rend l'erreur du moment.
///
/// **Compter les appels et non comparer des sorties.** Le critère « après l'arrêt, plus une seule
/// écriture » ne se vérifie pas autrement : une implémentation qui pousserait l'image puis jetterait
/// le résultat rendrait exactement le même verdict qu'une implémentation au repos, tout en
/// consommant le bus et en gelant le démon cinq secondes de plus à chaque tour. C'est précisément
/// le défaut que #70 corrige — il serait donc grotesque de le laisser passer.
struct DalleFactice {
    /// Le nombre de fois où la fermeture d'écriture a été appelée.
    ecritures: Cell<u32>,
    /// Le code d'erreur rendu à la prochaine écriture, ou `None` si la dalle répond.
    panne: Cell<Option<i32>>,
}

impl DalleFactice {
    /// Une dalle qui accepte tout ce qu'on lui pousse.
    fn qui_repond() -> DalleFactice {
        DalleFactice {
            ecritures: Cell::new(0),
            panne: Cell::new(None),
        }
    }

    /// Une dalle en délai d'attente, comme celle du journal de SHYNAEL.
    fn qui_refuse() -> DalleFactice {
        DalleFactice {
            ecritures: Cell::new(0),
            panne: Cell::new(Some(ETIMEDOUT)),
        }
    }

    /// La dalle se met à refuser, avec ce code.
    fn tombe_en_panne(&self, code: i32) {
        self.panne.set(Some(code));
    }

    /// La dalle se remet à répondre.
    fn se_retablit(&self) {
        self.panne.set(None);
    }

    /// L'écriture elle-même : c'est ce que la vigie appelle, ou n'appelle pas.
    fn pousser(&self) -> io::Result<()> {
        self.ecritures.set(self.ecritures.get() + 1);
        match self.panne.get() {
            None => Ok(()),
            Some(code) => Err(io::Error::from_raw_os_error(code)),
        }
    }

    /// Le compte des écritures depuis le début.
    fn ecritures(&self) -> u32 {
        self.ecritures.get()
    }
}

/// Le vocabulaire complet de [`Verdict`], énuméré **sans joker et sans `..`**.
///
/// Ce `match` est un fil-piège, et c'est toute sa raison d'être. Les deux derniers critères de #70 —
/// l'arrêt n'efface pas `ecran.conf`, et ne touche ni l'éclairage, ni les ventilateurs, ni les
/// zones — ne se vérifient pas en observant un effet : **la vigie n'a la main sur rien de tout
/// cela**, et c'est la garantie qu'on veut. Ils se vérifient en tenant fermé ce qu'elle a le droit
/// de *dire* au démon. Le jour où une variante s'ajouterait pour instruire un effacement, ou
/// qu'`Abandon` gagnerait un champ « et remets l'éclairage à zéro », ce fichier cesserait de
/// compiler — ce qui est exactement le signal voulu.
fn resume(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::Emise => "émise",
        Verdict::Refusee { erreur: _ } => "refusée",
        Verdict::Abandon { erreur: _ } => "abandon",
        Verdict::Repos => "repos",
    }
}

/// Pousse une image et rend le verdict, en nommant le tour dans le message d'échec.
///
/// Passer par une fonction plutôt que par une fermeture inline évite qu'un test long ne raconte que
/// des `vigie.tour(|| dalle.pousser())` empilés.
fn tour(vigie: &mut Vigie, dalle: &DalleFactice) -> Verdict {
    vigie.tour(|| dalle.pousser())
}

/// Fait échouer la dalle `combien` fois d'affilée et rend les verdicts, dans l'ordre.
fn echouer(vigie: &mut Vigie, dalle: &DalleFactice, combien: u32) -> Vec<Verdict> {
    (0..combien).map(|_| tour(vigie, dalle)).collect()
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tous les tests qui suivent sont écrits en fonction de `ECHECS_AVANT_ABANDON` plutôt que d'un
    // nombre en dur : c'est ce que veut le critère « N nommé dans le code, pas dispersé ». Mais une
    // suite ainsi paramétrée a une faiblesse — elle devient vraie sans rien vérifier si la
    // constante dégénère. À 1, « N-1 échecs n'arrêtent pas » ne parle plus que du cas où il ne
    // s'est rien passé. À 0, la dalle ne s'allumerait jamais. Ce test est là pour que la panne soit
    // ici, et nulle part ailleurs.
    assert!(
        ECHECS_AVANT_ABANDON >= PLAFOND_MINIMAL,
        "un plafond de {ECHECS_AVANT_ABANDON} rend la dalle nerveuse au point qu'un hoquet unique \
         l'éteindrait, et il vide de sens le test « N-1 échecs n'arrêtent pas »"
    );
    assert!(
        ECHECS_AVANT_ABANDON <= PLAFOND_MAXIMAL,
        "un plafond de {ECHECS_AVANT_ABANDON} laisse le démon gelé {} secondes avant de rendre la \
         main, à cinq secondes par tentative (#68)",
        ECHECS_AVANT_ABANDON * 5
    );

    // Les trois codes de panne doivent différer, sinon « l'abandon nomme la dernière erreur » ne
    // distinguerait plus rien.
    let codes = [("ETIMEDOUT", ETIMEDOUT), ("EIO", EIO), ("EPIPE", EPIPE)];
    for (i, (nom, code)) in codes.iter().enumerate() {
        for (autre_nom, autre) in codes.iter().skip(i + 1) {
            assert_ne!(
                code, autre,
                "{nom} et {autre_nom} doivent différer, sinon nommer la dernière erreur ne se \
                 distingue plus de nommer la première"
            );
        }
        assert_ne!(
            message(*code),
            String::new(),
            "{nom} doit produire un message non vide, sinon « en nommant l'erreur » ne veut rien dire"
        );
    }

    // Et les longues suites doivent être assez longues pour qu'un compteur cumulé y déborde
    // plusieurs fois — sans quoi le test qui le vise passerait par chance.
    assert!(
        LONGUE_VIE > ECHECS_AVANT_ABANDON * 4,
        "une suite de {LONGUE_VIE} échecs isolés ne déborderait pas assez de fois un compteur \
         cumulé de plafond {ECHECS_AVANT_ABANDON}"
    );
    assert!(
        TOURS_D_UN_GIF > ECHECS_AVANT_ABANDON,
        "il faut plus de tours après l'abandon qu'il n'en a fallu pour l'atteindre"
    );
}

// ---------------------------------------------------------------------------
// 1 — une vigie neuve émet
// ---------------------------------------------------------------------------

#[test]
fn une_vigie_neuve_emet_et_n_a_essuye_aucun_echec() {
    // Point n° 5 de l'en-tête. Un démon qui vient de démarrer n'a essuyé aucun refus ; il n'a aucune
    // raison de se taire. La faute symétrique de #70 — une vigie qui naîtrait abandonnée — laisserait
    // l'écran noir sans un mot, et c'est le seul mode de défaillance pire que celui qu'on corrige :
    // silencieux, permanent, et indistinguable d'un matériel mort.
    let vigie = Vigie::neuve();

    assert!(
        vigie.emet(),
        "une vigie neuve doit émettre : rien ne lui a encore été refusé"
    );
    assert_eq!(
        vigie.echecs_consecutifs(),
        0,
        "une vigie neuve part de zéro, elle a trouvé {} échecs",
        vigie.echecs_consecutifs()
    );
}

#[test]
fn une_dalle_qui_repond_est_servie_indefiniment() {
    // Le pendant du test précédent, dans la durée : rien de ce que #70 ajoute ne doit border une
    // dalle qui marche. C'est la première chose qu'un plafond mal posé casserait, et ça ne se
    // verrait qu'après des heures.
    let dalle = DalleFactice::qui_repond();
    let mut vigie = Vigie::neuve();

    for numero in 1..=TOURS_D_UN_GIF {
        let verdict = tour(&mut vigie, &dalle);
        assert_eq!(
            verdict,
            Verdict::Emise,
            "au tour {numero}, une dalle qui répond doit être servie, verdict trouvé : {}",
            resume(&verdict)
        );
    }
    assert_eq!(
        dalle.ecritures(),
        TOURS_D_UN_GIF,
        "chaque tour d'une dalle qui répond vaut une écriture, {} trouvées",
        dalle.ecritures()
    );
    assert!(vigie.emet(), "rien n'a échoué : rien ne doit s'être arrêté");
    assert_eq!(
        vigie.echecs_consecutifs(),
        0,
        "aucun échec n'a eu lieu, le compte doit être nul"
    );
}

// ---------------------------------------------------------------------------
// 2 — N échecs d'affilée arrêtent l'émission
// ---------------------------------------------------------------------------

#[test]
fn n_echecs_d_affilee_arretent_l_emission() {
    // Critère d'acceptation : « N échecs consécutifs arrêtent l'émission — N nommé dans le code,
    // pas dispersé », et « après l'arrêt, le démon est au repos absolu sur l'écran ».
    //
    // Le scénario est celui du journal : la dalle est en délai d'attente, et le démon réémet.
    let dalle = DalleFactice::qui_refuse();
    let mut vigie = Vigie::neuve();

    let verdicts = echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON);

    // Les N-1 premiers refus laissent l'émission en place — c'est l'objet du test suivant, repris
    // ici pour que celui-ci décrive la suite complète et non sa seule fin.
    for (rang, verdict) in verdicts.iter().enumerate().take(verdicts.len() - 1) {
        assert_eq!(
            *verdict,
            Verdict::Refusee {
                erreur: message(ETIMEDOUT)
            },
            "au refus n° {}, l'émission continue, verdict trouvé : {}",
            rang + 1,
            resume(verdict)
        );
    }

    // Le N-ième prononce l'abandon, et il le prononce en nommant l'erreur.
    assert_eq!(
        verdicts.last(),
        Some(&Verdict::Abandon {
            erreur: message(ETIMEDOUT)
        }),
        "le {ECHECS_AVANT_ABANDON}-ième refus d'affilée prononce l'abandon, en nommant l'erreur ; \
         verdict trouvé : {}",
        verdicts.last().map_or("aucun", resume)
    );

    assert!(
        !vigie.emet(),
        "après l'abandon, la vigie n'émet plus ; elle prétend le contraire"
    );
}

// ---------------------------------------------------------------------------
// 3 — N-1 échecs ne l'arrêtent pas
// ---------------------------------------------------------------------------

#[test]
fn n_moins_un_echecs_n_arretent_pas_l_emission() {
    // Critère d'acceptation, moitié indispensable du précédent. Sans ce test, une implémentation qui
    // renonce au **premier** refus passerait tout le reste du fichier — et elle serait pire que le
    // défaut d'origine : le contrôleur bafouille, la dalle s'éteint, et il faut une commande pour la
    // rallumer. Un délai d'attente isolé arrive ; il ne doit pas coûter l'écran.
    let dalle = DalleFactice::qui_refuse();
    let mut vigie = Vigie::neuve();

    let verdicts = echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON - 1);

    for (rang, verdict) in verdicts.iter().enumerate() {
        assert_eq!(
            *verdict,
            Verdict::Refusee {
                erreur: message(ETIMEDOUT)
            },
            "au refus n° {} sur {} avant le plafond, rien n'est encore abandonné ; verdict trouvé : \
             {}",
            rang + 1,
            ECHECS_AVANT_ABANDON - 1,
            resume(verdict)
        );
    }

    assert!(
        vigie.emet(),
        "{} refus, pour un plafond de {ECHECS_AVANT_ABANDON} : l'émission doit continuer",
        ECHECS_AVANT_ABANDON - 1
    );
    assert_eq!(
        vigie.echecs_consecutifs(),
        ECHECS_AVANT_ABANDON - 1,
        "le compte doit valoir {}, il vaut {}",
        ECHECS_AVANT_ABANDON - 1,
        vigie.echecs_consecutifs()
    );
    assert_eq!(
        dalle.ecritures(),
        ECHECS_AVANT_ABANDON - 1,
        "chacun de ces tours a bien tenté d'écrire"
    );
}

// ---------------------------------------------------------------------------
// 4 — un succès remet le compte à zéro
// ---------------------------------------------------------------------------

#[test]
fn un_succes_au_milieu_de_n_moins_un_echecs_remet_le_compte_a_zero() {
    // Critère d'acceptation : « un succès remet le compte à zéro ».
    //
    // La suite est : N-1 refus, un succès, N-1 refus de nouveau. Cumulés, cela fait 2(N-1) échecs,
    // soit strictement plus que le plafond dès que N vaut 2 ou plus. Consécutifs, cela n'en fait
    // jamais plus de N-1. Un compteur qui ne se remet pas à zéro abandonne ici ; le bon ne bronche
    // pas.
    let dalle = DalleFactice::qui_refuse();
    let mut vigie = Vigie::neuve();

    echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON - 1);
    assert_eq!(
        vigie.echecs_consecutifs(),
        ECHECS_AVANT_ABANDON - 1,
        "le compte doit avoir monté avant qu'on le remette à zéro"
    );

    dalle.se_retablit();
    let verdict = tour(&mut vigie, &dalle);
    assert_eq!(
        verdict,
        Verdict::Emise,
        "la dalle répond de nouveau : l'image part, verdict trouvé : {}",
        resume(&verdict)
    );
    assert_eq!(
        vigie.echecs_consecutifs(),
        0,
        "un succès remet le compte à zéro, il en reste {}",
        vigie.echecs_consecutifs()
    );

    dalle.tombe_en_panne(ETIMEDOUT);
    echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON - 1);
    assert!(
        vigie.emet(),
        "{} échecs cumulés, mais jamais plus de {} d'affilée : l'émission doit continuer",
        2 * (ECHECS_AVANT_ABANDON - 1),
        ECHECS_AVANT_ABANDON - 1
    );
}

#[test]
fn un_echec_isole_ne_borne_pas_la_vie_de_la_dalle() {
    // Le piège de conception nommé par l'issue : « **Consécutifs, et non cumulés.** Un compteur
    // cumulé finirait par arrêter un écran qui marche, après des jours de fonctionnement et
    // quelques hoquets sans conséquence. »
    //
    // C'est le seul test de ce fichier qu'un compteur cumulé ne passe pas, et c'est aussi le seul
    // qui décrive la vraie vie de la machine : un délai d'attente isolé toutes les dix minutes,
    // pendant des jours. Soixante répétitions, soit soixante échecs cumulés — vingt fois le plafond
    // — sans jamais en aligner deux.
    let dalle = DalleFactice::qui_repond();
    let mut vigie = Vigie::neuve();

    for cycle in 1..=LONGUE_VIE {
        // Le hoquet.
        dalle.tombe_en_panne(ETIMEDOUT);
        let refus = tour(&mut vigie, &dalle);
        assert_eq!(
            refus,
            Verdict::Refusee {
                erreur: message(ETIMEDOUT)
            },
            "au cycle {cycle}, un hoquet isolé n'est qu'un refus ; verdict trouvé : {}",
            resume(&refus)
        );

        // Les dix minutes qui suivent, où tout va bien.
        dalle.se_retablit();
        let reussite = tour(&mut vigie, &dalle);
        assert_eq!(
            reussite,
            Verdict::Emise,
            "au cycle {cycle}, la dalle répond de nouveau ; verdict trouvé : {}",
            resume(&reussite)
        );

        assert!(
            vigie.emet(),
            "au cycle {cycle}, soit {cycle} échecs cumulés, la vigie a renoncé — c'est le compteur \
             cumulé que l'issue interdit"
        );
        assert_eq!(
            vigie.echecs_consecutifs(),
            0,
            "au cycle {cycle}, le succès qui vient d'avoir lieu doit avoir remis le compte à zéro"
        );
    }

    assert_eq!(
        dalle.ecritures(),
        LONGUE_VIE * 2,
        "aucun tour n'a été sauté : {} écritures attendues, {} trouvées",
        LONGUE_VIE * 2,
        dalle.ecritures()
    );
}

#[test]
fn des_rafales_sous_le_plafond_n_arretent_jamais_rien() {
    // La variante la plus dure du test précédent : des rafales de N-1 refus, toutes séparées par un
    // seul succès. Un compteur cumulé y déborde au deuxième cycle ; un compteur qui **décrémente**
    // au lieu de se remettre à zéro — la faute discrète, celle qu'on écrit en croyant bien faire —
    // y déborde un peu plus tard, mais il y déborde.
    let dalle = DalleFactice::qui_repond();
    let mut vigie = Vigie::neuve();

    for cycle in 1..=LONGUE_VIE {
        dalle.tombe_en_panne(EIO);
        echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON - 1);
        assert!(
            vigie.emet(),
            "au cycle {cycle}, {} refus d'affilée pour un plafond de {ECHECS_AVANT_ABANDON} : \
             l'émission doit tenir",
            ECHECS_AVANT_ABANDON - 1
        );

        dalle.se_retablit();
        tour(&mut vigie, &dalle);
        assert_eq!(
            vigie.echecs_consecutifs(),
            0,
            "au cycle {cycle}, le compte doit repartir de zéro et non décroître de un ; il vaut {}",
            vigie.echecs_consecutifs()
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — après l'abandon, plus une seule écriture
// ---------------------------------------------------------------------------

#[test]
fn apres_l_abandon_plus_une_seule_ecriture() {
    // Critère d'acceptation : « après l'arrêt, le démon est **au repos absolu** sur l'écran : plus
    // une seule écriture ».
    //
    // Ce test **compte les appels**. C'est la seule façon de le vérifier : une implémentation qui
    // pousserait l'image puis jetterait son résultat rendrait le même verdict qu'une vigie muette,
    // tout en consommant le bus et en gelant le démon cinq secondes de plus par tour (#68). Une
    // égalité de verdicts ne verrait rien.
    let dalle = DalleFactice::qui_refuse();
    let mut vigie = Vigie::neuve();

    echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON);
    let a_l_abandon = dalle.ecritures();
    assert_eq!(
        a_l_abandon, ECHECS_AVANT_ABANDON,
        "il a fallu exactement {ECHECS_AVANT_ABANDON} écritures pour atteindre l'abandon, {a_l_abandon} \
         trouvées"
    );

    // Mille tours de plus : cent secondes de GIF, à une image toutes les cent millisecondes.
    for numero in 1..=TOURS_D_UN_GIF {
        let verdict = tour(&mut vigie, &dalle);
        assert_eq!(
            verdict,
            Verdict::Repos,
            "au tour {numero} après l'abandon, la vigie doit rester au repos ; verdict trouvé : {}",
            resume(&verdict)
        );
    }

    assert_eq!(
        dalle.ecritures(),
        a_l_abandon,
        "après l'abandon, plus une seule écriture : {a_l_abandon} attendues, {} trouvées",
        dalle.ecritures()
    );
}

#[test]
fn une_dalle_qui_se_retablit_seule_ne_reveille_pas_l_emission() {
    // Corollaire du critère « une commande `screen` explicite redémarre l'émission » : la reprise est
    // explicite, ou elle n'est pas. Une vigie qui sonderait la dalle « pour voir si ça remarche »
    // rouvrirait exactement la boucle que #70 ferme — au rythme de la réémission, et sans que rien
    // ne la compte, puisque le sondage n'est pas un échec.
    //
    // Sans ce test, une implémentation qui appelle la fermeture à chaque tour et ne se contente
    // d'ignorer que les erreurs passerait tout le reste.
    let dalle = DalleFactice::qui_refuse();
    let mut vigie = Vigie::neuve();

    echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON);
    let a_l_abandon = dalle.ecritures();

    dalle.se_retablit();
    for numero in 1..=TOURS_D_UN_GIF {
        let verdict = tour(&mut vigie, &dalle);
        assert_eq!(
            verdict,
            Verdict::Repos,
            "au tour {numero}, la dalle répond de nouveau — mais personne ne l'a demandé ; verdict \
             trouvé : {}",
            resume(&verdict)
        );
    }

    assert_eq!(
        dalle.ecritures(),
        a_l_abandon,
        "un rétablissement spontané ne se constate pas : il faudrait écrire pour le voir, et écrire \
         est justement ce qui est interdit ; {} écritures trouvées",
        dalle.ecritures()
    );
    assert!(
        !vigie.emet(),
        "la vigie ne doit pas s'être réveillée toute seule"
    );
}

// ---------------------------------------------------------------------------
// 6 — l'abandon est journalisé une fois, en nommant l'erreur
// ---------------------------------------------------------------------------

#[test]
fn l_abandon_n_est_prononce_qu_une_fois_meme_si_le_cycle_repasse() {
    // Critère d'acceptation : « l'arrêt est journalisé **une fois**, en nommant l'erreur ».
    //
    // C'est une propriété **d'état**, pas de politesse de l'appelant : le verdict doit dire
    // lui-même « c'est la première fois », sinon le démon devrait s'en souvenir de son côté — et le
    // compte de #70 serait de nouveau dispersé, ce que le premier critère interdit. On compte donc
    // les verdicts d'abandon sur un long cycle : il doit y en avoir exactement un.
    let dalle = DalleFactice::qui_refuse();
    let mut vigie = Vigie::neuve();

    let mut verdicts = echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON);
    verdicts.extend((0..TOURS_D_UN_GIF).map(|_| tour(&mut vigie, &dalle)));

    let abandons = verdicts
        .iter()
        .filter(|verdict| matches!(verdict, Verdict::Abandon { .. }))
        .count();
    assert_eq!(
        abandons,
        1,
        "l'abandon se prononce une fois et une seule sur {} tours, {abandons} trouvés — un journal \
         qui répéterait sa ligne toutes les cent millisecondes remplacerait une réémission sans fin \
         par une écriture de journal sans fin",
        verdicts.len()
    );

    let repos = verdicts
        .iter()
        .filter(|verdict| matches!(verdict, Verdict::Repos))
        .count();
    assert_eq!(
        repos, TOURS_D_UN_GIF as usize,
        "tous les tours qui suivent l'abandon sont des repos, {repos} trouvés"
    );
}

#[test]
fn l_abandon_nomme_la_derniere_erreur_vue() {
    // Critère d'acceptation : « en nommant l'erreur ». Point n° 2 de l'en-tête : c'est la
    // **dernière** — celle qui a fait déborder le compte, et la plus récente information sur l'état
    // du bus. Les refus portent donc trois codes distincts, sans quoi le choix serait invérifiable
    // et une implémentation qui garderait le premier passerait.
    let dalle = DalleFactice::qui_repond();
    let mut vigie = Vigie::neuve();

    // Les N-1 premiers refus, sous un code, puis le dernier sous un autre.
    dalle.tombe_en_panne(EPIPE);
    echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON - 1);
    dalle.tombe_en_panne(ETIMEDOUT);
    let verdict = tour(&mut vigie, &dalle);

    assert_eq!(
        verdict,
        Verdict::Abandon {
            erreur: message(ETIMEDOUT)
        },
        "l'abandon nomme la dernière erreur — « {} » —, pas la première ; verdict trouvé : {}",
        message(ETIMEDOUT),
        resume(&verdict)
    );

    // Et le message porte bien de quoi identifier la panne du journal de SHYNAEL. Le libellé
    // dépend de la locale ; le code, non.
    let Verdict::Abandon { erreur } = &verdict else {
        unreachable!("le verdict vient d'être comparé à un abandon")
    };
    assert!(
        erreur.contains(&ETIMEDOUT.to_string()),
        "le message d'abandon doit porter le code de l'erreur, pour qu'on puisse relier la ligne du \
         journal à « Connection timed out (os error {ETIMEDOUT}) » ; trouvé : « {erreur} »"
    );
    assert!(
        !erreur.is_empty(),
        "un abandon sans erreur nommée laisserait l'utilisateur devant une dalle noire sans savoir \
         pourquoi"
    );
}

#[test]
fn un_refus_sous_le_plafond_nomme_aussi_son_erreur() {
    // Point n° 3 de l'en-tête. Le démon journalise déjà « attention : luminosité de l'écran : … » à
    // chaque tentative, et #70 ne demande pas de retirer cette ligne — seulement de la borner. Un
    // verdict qui perdrait l'erreur en chemin obligerait l'appelant à la garder de son côté, et
    // rouvrirait le comptage dispersé que le premier critère ferme.
    let dalle = DalleFactice::qui_repond();
    let mut vigie = Vigie::neuve();

    dalle.tombe_en_panne(EIO);
    let verdict = tour(&mut vigie, &dalle);

    assert_eq!(
        verdict,
        Verdict::Refusee {
            erreur: message(EIO)
        },
        "un refus sous le plafond porte son erreur ; verdict trouvé : {}",
        resume(&verdict)
    );
}

// ---------------------------------------------------------------------------
// 7 — une commande `screen` explicite relance
// ---------------------------------------------------------------------------

#[test]
fn une_commande_screen_explicite_relance_l_emission_et_remet_le_compte_a_zero() {
    // Critère d'acceptation : « une commande `screen` explicite redémarre l'émission et remet le
    // compte à zéro ». C'est la porte de sortie de #70 : sans elle, un abandon serait définitif
    // jusqu'au redémarrage du service, et le remède serait pire que le mal.
    let dalle = DalleFactice::qui_refuse();
    let mut vigie = Vigie::neuve();

    echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON);
    assert!(
        !vigie.emet(),
        "la vigie doit avoir abandonné avant la relance"
    );

    vigie.relancer();

    assert!(
        vigie.emet(),
        "après une commande `screen`, la vigie émet de nouveau"
    );
    assert_eq!(
        vigie.echecs_consecutifs(),
        0,
        "la relance remet le compte à zéro, il en reste {}",
        vigie.echecs_consecutifs()
    );

    // Et elle écrit vraiment : le compte des écritures reprend là où il s'était arrêté.
    dalle.se_retablit();
    let avant = dalle.ecritures();
    let verdict = tour(&mut vigie, &dalle);
    assert_eq!(
        verdict,
        Verdict::Emise,
        "après la relance, l'image part ; verdict trouvé : {}",
        resume(&verdict)
    );
    assert_eq!(
        dalle.ecritures(),
        avant + 1,
        "une relance qui ne rouvrirait pas l'écriture ne relancerait rien : {} écritures trouvées, \
         {} attendues",
        dalle.ecritures(),
        avant + 1
    );
}

#[test]
fn apres_une_relance_il_faut_de_nouveau_n_echecs_pour_reabandonner() {
    // La moitié qu'on oublie : une relance qui laisserait le compte à N-1 ferait ré-abandonner au
    // premier refus suivant. L'utilisateur taperait `screen`, verrait la dalle s'allumer, et la
    // verrait s'éteindre au tour d'après — un comportement qu'aucun message de journal n'expliquerait.
    let dalle = DalleFactice::qui_refuse();
    let mut vigie = Vigie::neuve();

    echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON);
    vigie.relancer();

    // Un refus de moins que le plafond : l'émission doit tenir.
    let verdicts = echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON - 1);
    for (rang, verdict) in verdicts.iter().enumerate() {
        assert_eq!(
            *verdict,
            Verdict::Refusee {
                erreur: message(ETIMEDOUT)
            },
            "après relance, le refus n° {} n'abandonne pas encore ; verdict trouvé : {}",
            rang + 1,
            resume(verdict)
        );
    }
    assert!(
        vigie.emet(),
        "après une relance, il faut de nouveau {ECHECS_AVANT_ABANDON} refus — pas un seul"
    );

    // Et le N-ième prononce un abandon neuf : un abandon par épisode, pas un pour la vie du démon.
    let verdict = tour(&mut vigie, &dalle);
    assert_eq!(
        verdict,
        Verdict::Abandon {
            erreur: message(ETIMEDOUT)
        },
        "un second épisode se journalise comme le premier ; verdict trouvé : {}",
        resume(&verdict)
    );
}

#[test]
fn une_relance_sur_une_vigie_qui_marche_ne_lui_fait_rien_perdre() {
    // Point n° 4 de l'en-tête. Une commande `screen` est fréquente — changer d'image, régler la
    // luminosité —, et la plupart du temps la dalle va très bien. Relancer ne doit alors rien
    // casser : ni interrompre l'émission, ni forcer un tour à vide.
    let dalle = DalleFactice::qui_repond();
    let mut vigie = Vigie::neuve();

    tour(&mut vigie, &dalle);
    let avant = dalle.ecritures();

    vigie.relancer();

    assert!(
        vigie.emet(),
        "relancer une vigie qui émet la laisse émettre"
    );
    assert_eq!(
        vigie.echecs_consecutifs(),
        0,
        "et le compte reste nul, il vaut {}",
        vigie.echecs_consecutifs()
    );
    assert_eq!(
        dalle.ecritures(),
        avant,
        "relancer n'écrit pas de soi-même : c'est le tour suivant qui écrit ; {} écritures trouvées",
        dalle.ecritures()
    );

    let verdict = tour(&mut vigie, &dalle);
    assert_eq!(
        verdict,
        Verdict::Emise,
        "et le tour suivant part normalement ; verdict trouvé : {}",
        resume(&verdict)
    );
}

// ---------------------------------------------------------------------------
// 8 — ce que l'abandon ne touche pas
// ---------------------------------------------------------------------------

#[test]
fn l_abandon_ne_touche_pas_l_etat_persiste() {
    // Critère d'acceptation : « l'arrêt **n'efface pas** `ecran.conf` : ce qu'on voulait afficher
    // reste ce qu'on voulait afficher, et un redémarrage le retente ».
    //
    // Ce test est un **fil-piège**, et il est assumé comme tel. Il passe aujourd'hui parce que
    // `Vigie::tour` ne reçoit qu'une fermeture d'écriture : elle n'a aucun moyen d'atteindre l'état
    // persisté, et c'est la garantie qu'on veut. Il cesserait de compiler le jour où quelqu'un lui
    // passerait un `&mut Etat` « pour faire le ménage à l'abandon » — ce qui est exactement le
    // moment où cette décision doit se discuter plutôt que se prendre en passant.
    //
    // L'état témoin est celui de la panne : le GIF que `ecran.conf` portait (contexte de #69).
    let etat = etat_temoin();
    let avant = etat.encoder();

    let dalle = DalleFactice::qui_refuse();
    let mut vigie = Vigie::neuve();
    echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON * 3);

    assert_eq!(
        etat.encoder(),
        avant,
        "l'abandon n'a pas à réécrire ce qu'on voulait afficher ; trouvé : « {} »",
        etat.encoder()
    );
    assert_eq!(
        etat.affichage,
        Affichage::Gif("/home/nico/anims/pluie.gif".to_owned()),
        "le GIF demandé reste le GIF demandé : c'est lui qu'un redémarrage doit retenter"
    );
    assert_eq!(
        etat.luminosite, 50,
        "la luminosité choisie n'a rien à voir avec le refus de la dalle"
    );
}

#[test]
fn le_verdict_ne_dit_rien_de_l_eclairage_ni_des_ventilateurs_ni_des_zones() {
    // Critère d'acceptation : « l'éclairage, les ventilateurs et les zones ne sont **pas** touchés
    // par cet arrêt ».
    //
    // Cela ne s'observe pas : la vigie n'a la main sur aucun des trois, et c'est précisément la
    // garantie. Ce qui s'observe, c'est le **vocabulaire** dans lequel elle parle au démon. Quatre
    // verdicts, tous relatifs à la seule dalle, et aucun champ pour instruire autre chose : un
    // abandon ne peut littéralement pas demander d'éteindre les ventilateurs, parce qu'il n'a pas
    // de mot pour le dire.
    //
    // `resume` énumère ce vocabulaire sans joker et sans `..` ; ce test le parcourt. Une variante
    // ajoutée, ou un champ ajouté à `Abandon`, casse la compilation de ce fichier — c'est le signal
    // voulu, et il arrive avant que le code ne soit écrit plutôt qu'après.
    let dalle = DalleFactice::qui_refuse();
    let mut vigie = Vigie::neuve();

    let mut verdicts = echouer(&mut vigie, &dalle, ECHECS_AVANT_ABANDON);
    verdicts.push(tour(&mut vigie, &dalle));
    dalle.se_retablit();
    vigie.relancer();
    verdicts.push(tour(&mut vigie, &dalle));

    let vus: Vec<&str> = verdicts.iter().map(resume).collect();
    for attendu in ["refusée", "abandon", "repos", "émise"] {
        assert!(
            vus.contains(&attendu),
            "le scénario complet doit passer par « {attendu} » pour que ce fil-piège vaille quelque \
             chose ; verdicts vus : {vus:?}"
        );
    }
}
