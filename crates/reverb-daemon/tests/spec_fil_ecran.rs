//! Tests d'intention du fil dédié à la dalle du Kraken — issue #83.
//!
//! Écrits **avant** l'implémentation, depuis l'issue #83 seule — aucun fichier de
//! `crates/reverb-daemon/src/` n'a été lu pour les produire, hors les signatures publiques déjà
//! existantes de [`Dalle`], [`Verdict`] et [`ECHECS_AVANT_ABANDON`], reprises telles quelles de
//! `spec_ecran.rs` (#33) et de `spec_plafond.rs` (#70). À l'écriture de ce fichier, le module
//! `fil_ecran` **n'existe pas** : la compilation doit échouer, et c'est la phase rouge.
//!
//! Aucune écriture matérielle, aucun `/dev`, aucun démon lancé. La dalle est **injectée** — un
//! objet du test qui note ce qu'on lui demande, et qui refuse ou reste coincé sur commande.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Observé sur SHYNAEL le 2026-08-08, avec une composition portant deux sondes vivantes sur un fond
//! image : **toutes les deux secondes, l'éclairage entier se fige, clignote, puis reprend.** Une
//! composition à sonde est recomposée toutes les 2 s et poussée dès qu'elle change (#80) ; une
//! température de CPU bouge en permanence, donc 1,2 Mo partent sur l'USB toutes les deux secondes,
//! **depuis le fil qui anime aussi les LED et sert le socket** (ADR-002).
//!
//! ```text
//! 20 img/s ──► 1,2 Mo poussés ──► 11 à 14 img/s pendant ~20 s ──► 20 img/s
//! ```
//!
//! État du fil principal échantillonné sur 12 s, `D` = sommeil non interruptible :
//!
//! ```text
//! DDDDDDDDRDDDDSSSSSSDDDDSSSSSSDDDDDDDDDDRDDDSSSSSSDDDDSSSSSSD
//! ```
//!
//! ⚠️ **Ce n'est pas un réglage à corriger, c'est architectural.** Le cache de #80 — « on ne pousse
//! que ce qui a changé » — n'y peut rien : quand la valeur change, il *faut* pousser. C'est le même
//! défaut de fond que #68, qui n'avait été traité que pour les sondes.
//!
//! ## La couture que ces tests exigent, et pourquoi celle-là
//!
//! L'issue pose que le `Kraken` sort de `Peripheriques` et passe à un fil dédié qui le détient
//! seul, que l'image voyage par un canal à une seule place, et que le verdict de #70 revient par un
//! second canal lu sans attendre. Elle ne dit pas quelle forme donner à tout cela. Ce fichier
//! tranche, et voici ce qu'il faut implémenter :
//!
//! ```ignore
//! // crates/reverb-daemon/src/fil_ecran.rs
//! // et `pub mod fil_ecran;` dans crates/reverb-daemon/src/lib.rs
//!
//! use std::io;
//!
//! use crate::ecran::{Dalle, Verdict};
//!
//! /// Ce que le fil de la dalle sait demander au matériel.
//! ///
//! /// Une seule implémentation en production — l'écran du Kraken sur usbfs —, et une factice dans
//! /// les tests. C'est cette couture qui rend #83 vérifiable sans brancher de Kraken.
//! pub trait Afficheur: Send {
//!     /// Applique la luminosité, en pourcent.
//!     fn luminosite(&mut self, pourcent: u8) -> io::Result<()>;
//!
//!     /// Pousse les 1 228 800 octets d'une image.
//!     fn image(&mut self, dalle: &Dalle) -> io::Result<()>;
//! }
//!
//! /// La poignée que le fil principal garde sur le fil de la dalle.
//! ///
//! /// **Aucune de ses méthodes ne bloque** — c'est tout l'objet de #83.
//! pub struct FilEcran { /* … */ }
//!
//! impl FilEcran {
//!     /// Démarre le fil dédié. Il détient `afficheur` **seul** : personne d'autre ne le touche.
//!     pub fn demarrer<A: Afficheur + 'static>(afficheur: A) -> FilEcran;
//!
//!     /// Dépose une image. Elle **remplace** celle qui attendait encore, elle ne s'empile pas.
//!     pub fn afficher(&self, dalle: Dalle);
//!
//!     /// Dépose un ordre de luminosité. Il n'est ni perdu, ni doublé, et précède l'image qui
//!     /// le suit (spec §3.4 : `30 02` réinitialise le pipeline d'affichage).
//!     pub fn luminosite(&self, pourcent: u8);
//!
//!     /// Les verdicts de la vigie (#70) survenus depuis le dernier appel. **N'attend rien** :
//!     /// rend un vecteur vide quand la dalle n'a rien à dire.
//!     pub fn verdicts(&self) -> Vec<Verdict>;
//!
//!     /// Remet la vigie en émission, comme le fait une commande `screen` (#70).
//!     ///
//!     /// ⚠️ Elle **prend son rang dans la file**, elle ne la double pas : une relance posée par
//!     /// un drapeau partagé arriverait avant l'abandon qui la motive, et serait effacée par lui.
//!     pub fn relancer(&self);
//!
//!     /// Demande l'arrêt, attend la fin du fil, et rend les verdicts qu'il n'avait pas dits.
//!     pub fn arreter(self) -> Vec<Verdict>;
//! }
//! ```
//!
//! Cinq choix, et ce qu'ils achètent :
//!
//! 1. **Un trait plutôt qu'une fermeture.** Deux gestes distincts partent vers la dalle — une
//!    luminosité et une image — et leur **ordre relatif** est un critère d'acceptation. Une
//!    fermeture unique les aurait confondus, et « la luminosité part avant l'image » serait devenu
//!    invérifiable.
//! 2. **`demarrer` prend l'afficheur par valeur.** C'est ce qui rend « un seul fil détient le nœud
//!    USB » vrai *par construction* et non par discipline : une fois l'objet déplacé dans le fil,
//!    le fil principal n'a plus rien à quoi parler. La promesse de l'ADR-002 est déplacée, pas
//!    rompue.
//! 3. **`afficher` prend la `Dalle` par valeur et ne rend rien.** Rien à attendre, donc rien qui
//!    bloque. Le résultat de l'envoi ne revient pas par retour de fonction — il reviendrait en
//!    faisant attendre —, mais par [`FilEcran::verdicts`], au tour suivant.
//! 4. **`verdicts` rend un `Vec` plutôt qu'un `Option`.** Entre deux tours de boucle, plusieurs
//!    envois ont pu avoir lieu ; n'en rendre qu'un obligerait l'appelant à rappeler jusqu'à vide,
//!    et un abandon (#70) pourrait rester coincé derrière deux refus.
//! 5. **`arreter` rend les verdicts en suspens.** Sans cela, ce que la dalle a dit en dernier
//!    serait perdu en silence à l'arrêt — et, côté test, aucune assertion sur les verdicts ne
//!    serait déterministe sans sondage périodique, que ce fichier s'interdit.
//!
//! ## Ce que ce fichier fige
//!
//! 1. **Le fil principal ne touche jamais la dalle.** Vérifié en relevant le [`ThreadId`] de chaque
//!    geste : un seul fil, et jamais celui du test. C'est l'échantillon `D` ci-dessus qui est
//!    interdit, pas une propriété abstraite.
//! 2. **Aucun dépôt ne bloque**, même quand la dalle ne consomme rien du tout. Mesuré, et borné par
//!    les 100 ms du critère d'acceptation.
//! 3. **Une image plus récente remplace celle qui attendait** — et c'est bien la **dernière** qui
//!    arrive, pas la première ni toutes.
//! 4. **La luminosité précède l'image qui la suit**, n'est jamais perdue, et n'est jamais réémise
//!    à chaque image.
//! 5. **La vigie de #70 renonce toujours après [`ECHECS_AVANT_ABANDON`] refus**, mais depuis son
//!    propre fil, et une relance la fait repartir.
//!
//! ## Les deux pièges de ce fichier, et ils sont structurants
//!
//! ⚠️ **Un correctif qui aurait simplement débranché l'écran passerait tous les tests de
//! non-blocage.** Un fil qui ne pousse jamais rien ne fige évidemment plus personne. Chaque test de
//! non-blocage exige donc aussi que l'image **arrive**, entière et de la bonne couleur — et
//! [`l_image_deposee_arrive_entiere_et_telle_quelle`] est là pour ça et rien d'autre.
//!
//! ⚠️ **« La nouvelle remplace l'ancienne » passerait sur une implémentation qui perd tout sauf la
//! première.** Les images témoins portent donc des couleurs distinctes, et la liste des gestes est
//! comparée **exactement**, jamais par un simple compte.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Le décompte de la vigie lui-même** — N refus, N-1 qui n'arrêtent rien, la dernière erreur
//!   nommée. C'est `spec_plafond.rs` (#70), et ce chantier ne doit pas le toucher. Ici on vérifie
//!   seulement que la vigie a **changé de fil** sans changer d'avis.
//! - **Le relevé des sondes**, qui reste sur le fil principal avec sa quarantaine (#68) — l'issue le
//!   dit explicitement.
//! - **La composition** (#80), la validation de format (#69), la persistance de `ecran.conf`.
//! - **Le socket lui-même.** Les 100 ms de `status`, `zone list` et `geometry` se mesurent ici sur
//!   ce qui les gelait : la disponibilité du fil principal pendant qu'un mégaoctet part.

// Les assertions qui **encadrent** `ECHECS_AVANT_ABANDON` sont constantes une fois la constante
// écrite, et clippy les refuse à ce titre — comme dans `spec_plafond.rs`, où le même `allow` est
// posé pour la même raison. Leur intérêt n'est pas d'observer une exécution mais de casser la
// compilation le jour où le plafond dégénérerait au point de vider ce fichier de son sens.
#![allow(clippy::assertions_on_constants)]

use std::io;
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

use reverb_daemon::ecran::{Dalle, ECHECS_AVANT_ABANDON, Verdict};
use reverb_daemon::fil_ecran::{Afficheur, FilEcran};
use reverb_proto::screen;

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// Le temps qu'on laisse à un rendez-vous avant de déclarer qu'il n'aura pas lieu.
///
/// Généreux **exprès** : ce n'est pas une mesure, c'est un filet. Aucun test de ce fichier ne
/// devient vert parce qu'il a attendu assez longtemps — ils deviennent verts sur un signal explicite
/// de la dalle factice. Cette borne ne sert qu'à transformer un blocage en échec lisible plutôt
/// qu'en test qui pend.
const PATIENCE: Duration = Duration::from_secs(5);

/// Le délai de réponse exigé par le critère d'acceptation : « `status`, `zone list` et `geometry`
/// répondent en moins de 100 ms **pendant** un envoi vers la dalle ».
const REPONSE_MAXIMALE: Duration = Duration::from_millis(100);

/// `ETIMEDOUT`. C'est le code du journal de SHYNAEL, celui de #70 : « Connection timed out (os
/// error 110) ».
const ETIMEDOUT: i32 = 110;

/// Le nombre de requêtes que le fil principal sert pendant qu'un mégaoctet part.
///
/// Cent tours, quand le critère n'en exige qu'un : ce qui est mesuré ici est cent fois plus dur que
/// ce qui est demandé, et laisse donc de la marge sur une machine chargée.
const SERVICES: usize = 100;

/// Le nombre d'images déposées pendant que la dalle est coincée dans son envoi.
///
/// Dix suffisent à faire déborder toute file d'attente raisonnable, et coûtent dix allocations de
/// 1,2 Mo — assez peu pour tenir très en dessous des 100 ms mesurées.
const IMAGES_PENDANT_L_ENVOI: usize = 10;

/// Le nombre de dépôts d'un test qui coince la dalle pour de bon.
///
/// Cent, parce qu'une file d'attente qui ne bloque pas mais qui **empile** passerait sur trois.
const DEPOTS_SUR_UNE_DALLE_MUETTE: usize = 100;

/// Le nombre de tours pendant lesquels on martèle une dalle qui a été abandonnée.
///
/// Cent tours, soit dix secondes de GIF à la cadence du README. C'est le rythme auquel une émission
/// non bornée harcèle un contrôleur déjà en difficulté (#70).
const MARTELEMENT: usize = 100;

/// La luminosité témoin, en pourcent. Ni 0 ni 100 : une valeur qu'aucun défaut ne produirait par
/// hasard.
const LUMINOSITE: u8 = 40;

/// Une seconde luminosité, distincte de la première — sans quoi « jamais perdue » ne se
/// distinguerait pas de « la dernière écrase les autres ».
const AUTRE_LUMINOSITE: u8 = 90;

/// La première image témoin. Ses trois composantes sont distinctes, donc aucune permutation
/// GRB/BGR/RGB ne peut passer inaperçue.
const PREMIERE: (u8, u8, u8) = (0xff, 0x20, 0x80);

/// La deuxième.
const DEUXIEME: (u8, u8, u8) = (0x10, 0xc0, 0x50);

/// La troisième — il en faut trois pour qu'une suite d'images ait un milieu, et donc pour que
/// « c'est la dernière qui arrive » se distingue de « c'en est une au hasard ».
const TROISIEME: (u8, u8, u8) = (0x40, 0x11, 0xa0);

/// La dernière, celle qu'on doit retrouver sur la dalle quand tout le reste a été abandonné.
const DERNIERE: (u8, u8, u8) = (0x80, 0xfe, 0x30);

/// Le message que la bibliothèque standard associe à un code d'erreur système.
///
/// Calculé plutôt que recopié : `strerror` dépend de la locale, et un test qui écrirait
/// « Connection timed out » en dur passerait sur la machine de son auteur et nulle part ailleurs.
fn message(code: i32) -> String {
    io::Error::from_raw_os_error(code).to_string()
}

// ---------------------------------------------------------------------------
// La dalle factice — elle note, elle refuse, et elle sait rester coincée
// ---------------------------------------------------------------------------

/// Ce que le fil de la dalle a demandé au matériel.
///
/// Une image n'y est pas gardée en entier — 1,2 Mo par geste noyaierait le message d'échec — mais
/// résumée par sa **taille** et la **couleur** de son premier pixel. Les deux comptent : la taille
/// attrape une dalle tronquée, que le contrôleur ignore en silence (spec §2.2.1), et la couleur
/// attrape une image qui n'est pas celle qu'on croit.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Geste {
    /// Une luminosité, en pourcent.
    Luminosite(u8),
    /// Une image, résumée.
    Image {
        /// Le nombre d'octets poussés.
        taille: usize,
        /// La couleur de son premier pixel, décodée par le protocole.
        couleur: (u8, u8, u8),
    },
}

impl Geste {
    /// Résume l'image qu'on vient de recevoir.
    fn depuis(dalle: &Dalle) -> Geste {
        let octets = dalle.octets();
        let couleur = octets
            .get(..screen::PIXEL_LEN)
            .map(screen::composantes)
            .unwrap_or_default();
        Geste::Image {
            taille: octets.len(),
            couleur,
        }
    }
}

/// Le geste qu'une image unie de cette couleur doit produire : entière, et de cette couleur-là.
fn image_de(couleur: (u8, u8, u8)) -> Geste {
    Geste::Image {
        taille: screen::IMAGE_LEN,
        couleur,
    }
}

/// L'état partagé entre le test et le fil de la dalle.
struct Etat {
    /// Ce qui a été demandé au matériel, dans l'ordre.
    gestes: Vec<Geste>,
    /// Les fils d'exécution qui l'ont touché, dans l'ordre de première apparition.
    fils: Vec<ThreadId>,
    /// Le code d'erreur rendu au prochain geste, ou `None` si la dalle répond.
    panne: Option<i32>,
    /// Tant qu'il est fermé, un geste entré ne ressort pas — c'est ainsi qu'on tient la dalle
    /// « au milieu de son mégaoctet » sans dormir.
    portail_ouvert: bool,
}

/// Une dalle qui note tout, qui refuse sur commande, et que le test peut coincer à volonté.
///
/// **Compter et confronter, jamais comparer une sortie.** Les critères de #83 portent sur *qui*
/// écrit, *quand*, et *ce qui n'arrive pas* — aucun ne se lit dans une valeur de retour. Cette
/// factice existe pour rendre ces trois choses observables.
struct DalleFactice {
    etat: Mutex<Etat>,
    signal: Condvar,
}

impl DalleFactice {
    /// Une dalle qui accepte tout ce qu'on lui pousse, et tout de suite.
    fn qui_repond() -> Arc<DalleFactice> {
        DalleFactice::neuve(None)
    }

    /// Une dalle en délai d'attente, comme celle du journal de SHYNAEL (#70).
    fn qui_refuse() -> Arc<DalleFactice> {
        DalleFactice::neuve(Some(ETIMEDOUT))
    }

    fn neuve(panne: Option<i32>) -> Arc<DalleFactice> {
        Arc::new(DalleFactice {
            etat: Mutex::new(Etat {
                gestes: Vec::new(),
                fils: Vec::new(),
                panne,
                portail_ouvert: true,
            }),
            signal: Condvar::new(),
        })
    }

    /// L'objet remis au fil dédié. Le test, lui, garde l'`Arc` pour observer.
    fn afficheur(self: &Arc<Self>) -> Factice {
        Factice(Arc::clone(self))
    }

    /// Coince la dalle : le prochain geste entrera et ne ressortira pas.
    fn coincer(&self) {
        let mut etat = self.etat.lock().expect("la dalle factice n'a pas paniqué");
        etat.portail_ouvert = false;
    }

    /// Libère la dalle, et réveille le geste qui attendait.
    fn liberer(&self) {
        let mut etat = self.etat.lock().expect("la dalle factice n'a pas paniqué");
        etat.portail_ouvert = true;
        self.signal.notify_all();
    }

    /// La dalle se remet à répondre.
    fn se_retablit(&self) {
        let mut etat = self.etat.lock().expect("la dalle factice n'a pas paniqué");
        etat.panne = None;
    }

    /// Tout ce qu'on lui a demandé, dans l'ordre.
    fn gestes(&self) -> Vec<Geste> {
        self.etat
            .lock()
            .expect("la dalle factice n'a pas paniqué")
            .gestes
            .clone()
    }

    /// Les fils qui l'ont touchée.
    fn fils(&self) -> Vec<ThreadId> {
        self.etat
            .lock()
            .expect("la dalle factice n'a pas paniqué")
            .fils
            .clone()
    }

    /// Attend qu'une condition soit vraie, et échoue en la nommant si elle ne l'est jamais.
    ///
    /// C'est le seul mécanisme d'attente de ce fichier : un rendez-vous explicite, jamais un délai
    /// choisi au jugé. [`PATIENCE`] n'est pas une durée de test, c'est une borne de diagnostic.
    fn attendre(&self, quoi: &str, condition: impl Fn(&Etat) -> bool) {
        let etat = self.etat.lock().expect("la dalle factice n'a pas paniqué");
        let (etat, expiration) = self
            .signal
            .wait_timeout_while(etat, PATIENCE, |etat| !condition(etat))
            .expect("la dalle factice n'a pas paniqué");
        assert!(
            !expiration.timed_out(),
            "au bout de {PATIENCE:?}, {quoi} n'est toujours pas arrivé ; gestes vus : {:?}",
            etat.gestes
        );
    }

    /// Le geste lui-même : il est noté, il attend le portail, puis il rend sa réponse.
    ///
    /// ⚠️ **La réponse est décidée à l'entrée**, avant l'attente du portail, et non à la sortie.
    /// Sinon un [`DalleFactice::se_retablit`] appelé pendant qu'un geste est coincé le ferait
    /// réussir rétroactivement, et l'ordre des appels du test déciderait du résultat — c'est
    /// exactement le genre de dépendance à l'ordonnancement qu'une suite de tests concurrents ne
    /// doit pas porter.
    fn noter(&self, geste: Geste) -> io::Result<()> {
        let mut etat = self.etat.lock().expect("la dalle factice n'a pas paniqué");
        etat.gestes.push(geste);
        let moi = thread::current().id();
        if !etat.fils.contains(&moi) {
            etat.fils.push(moi);
        }
        let reponse = etat.panne;
        self.signal.notify_all();

        let (etat, expiration) = self
            .signal
            .wait_timeout_while(etat, PATIENCE, |etat| !etat.portail_ouvert)
            .expect("la dalle factice n'a pas paniqué");
        assert!(
            !expiration.timed_out(),
            "le portail de la dalle factice est resté fermé plus de {PATIENCE:?} : le test l'a \
             coincée sans jamais la libérer"
        );
        drop(etat);

        match reponse {
            None => Ok(()),
            Some(code) => Err(io::Error::from_raw_os_error(code)),
        }
    }
}

/// Ce que le fil dédié détient, et que plus personne ne peut atteindre une fois `demarrer` appelé.
struct Factice(Arc<DalleFactice>);

impl Afficheur for Factice {
    fn luminosite(&mut self, pourcent: u8) -> io::Result<()> {
        self.0.noter(Geste::Luminosite(pourcent))
    }

    fn image(&mut self, dalle: &Dalle) -> io::Result<()> {
        self.0.noter(Geste::depuis(dalle))
    }
}

// ---------------------------------------------------------------------------
// Déposer sans se laisser prendre — le filet qui transforme un gel en échec
// ---------------------------------------------------------------------------

/// Exécute `corps` avec la poignée, dans un fil à part, et rend la poignée et le résultat.
///
/// C'est le filet de ce fichier. Le défaut de #83 est **un blocage** : un test qui déposerait une
/// image sur le fil principal et qui se ferait prendre ne « échouerait » pas, il **pendrait** — et
/// une suite qui pend est une suite qu'on finit par lancer avec `--skip`. En déportant le dépôt, un
/// blocage devient une assertion qui nomme ce qui ne s'est pas produit.
///
/// Le fil déporté, lui, reste bloqué pour toujours si l'implémentation bloque. C'est assumé : le
/// test a déjà échoué, et le processus s'arrête juste après.
fn sans_bloquer<R: Send + 'static>(
    quoi: &str,
    fil: FilEcran,
    corps: impl FnOnce(&FilEcran) -> R + Send + 'static,
) -> (FilEcran, R) {
    let (envoi, reception) = mpsc::channel();
    thread::spawn(move || {
        let resultat = corps(&fil);
        let _ = envoi.send((fil, resultat));
    });
    match reception.recv_timeout(PATIENCE) {
        Ok(rendu) => rendu,
        Err(_) => panic!(
            "au bout de {PATIENCE:?}, {quoi} n'a pas rendu la main : un dépôt vers la dalle bloque \
             le fil qui l'appelle, et c'est exactement le gel de #83"
        ),
    }
}

/// Le cas courant de [`sans_bloquer`] : on dépose, on ne rend rien.
fn deposer(quoi: &str, fil: FilEcran, corps: impl FnOnce(&FilEcran) + Send + 'static) -> FilEcran {
    sans_bloquer(quoi, fil, corps).0
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tous les tests qui suivent identifient une image à sa couleur et une suite d'images à l'ordre
    // de leurs couleurs. Si deux témoins se confondaient — même triplet, ou l'un permutation de
    // l'autre —, « c'est la dernière qui arrive » deviendrait vrai sans rien vérifier. Ce test
    // existe pour que la panne soit ici, et nulle part ailleurs.
    let temoins = [
        ("PREMIERE", PREMIERE),
        ("DEUXIEME", DEUXIEME),
        ("TROISIEME", TROISIEME),
        ("DERNIERE", DERNIERE),
    ];
    for (rang, (nom, couleur)) in temoins.iter().enumerate() {
        let mut composantes = [couleur.0, couleur.1, couleur.2];
        composantes.sort_unstable();
        assert!(
            composantes[0] != composantes[1] && composantes[1] != composantes[2],
            "{nom} doit avoir trois composantes distinctes, sinon une permutation GRB/BGR/RGB \
             passerait inaperçue"
        );
        for (autre_nom, autre) in temoins.iter().skip(rang + 1) {
            let mut autres = [autre.0, autre.1, autre.2];
            autres.sort_unstable();
            assert_ne!(
                composantes, autres,
                "{nom} et {autre_nom} ne doivent pas être permutation l'une de l'autre : une \
                 erreur d'ordre des composantes ferait passer l'une pour l'autre"
            );
        }
    }

    assert_ne!(
        LUMINOSITE, AUTRE_LUMINOSITE,
        "les deux luminosités témoins doivent différer, sinon « jamais perdue » ne se distingue \
         pas de « la dernière écrase les autres »"
    );

    // La factice identifie une image par le premier pixel de son tampon. Si ce résumé ne survivait
    // pas à l'aller-retour, tous les tests de contenu de ce fichier compareraient du bruit.
    assert_eq!(
        Geste::depuis(&Dalle::unie(PREMIERE)),
        image_de(PREMIERE),
        "une dalle unie doit se résumer à sa couleur et à ses {} octets — sans quoi la dalle \
         factice n'identifie plus rien",
        screen::IMAGE_LEN
    );

    // La borne de diagnostic doit rester très au-dessus de la borne mesurée, sinon un test lent
    // deviendrait un test qui ment.
    assert!(
        PATIENCE > REPONSE_MAXIMALE * 10,
        "{PATIENCE:?} de filet pour {REPONSE_MAXIMALE:?} de mesure : trop serré, un test chargé \
         confondrait la lenteur et la panne"
    );

    // Et le plafond de #70 doit valoir au moins deux, sans quoi « trois refus d'affilée » ne
    // décrirait plus rien.
    assert!(
        ECHECS_AVANT_ABANDON >= 2,
        "un plafond de {ECHECS_AVANT_ABANDON} viderait de son sens le test de la vigie déplacée"
    );
}

// ---------------------------------------------------------------------------
// 1 — un seul fil détient le nœud USB, et ce n'est jamais celui des LED
// ---------------------------------------------------------------------------

#[test]
fn un_seul_fil_touche_la_dalle_et_ce_n_est_jamais_celui_qui_anime_les_led() {
    // Comportement n° 1 de l'issue, et critère d'acceptation : « **Un seul fil détient le nœud USB
    // de la dalle** — la promesse de l'ADR-002 est déplacée, pas rompue. »
    //
    // Ce que ce test interdit de revenir, c'est l'échantillon de #83 : `DDDDDDDDRDDDD…`, le fil
    // principal en sommeil non interruptible pendant que 1,2 Mo passent sur l'USB. Il ne se mesure
    // pas en durée — une machine rapide masquerait tout — mais en **identité** : le geste vers la
    // dalle a lieu ailleurs, ou il n'a pas lieu.
    let dalle = DalleFactice::qui_repond();
    let fil = FilEcran::demarrer(dalle.afficheur());
    let principal = thread::current().id();

    fil.luminosite(LUMINOSITE);
    dalle.attendre("l'ordre de luminosité", |etat| !etat.gestes.is_empty());

    for (rang, couleur) in [PREMIERE, DEUXIEME, TROISIEME, DERNIERE]
        .into_iter()
        .enumerate()
    {
        fil.afficher(Dalle::unie(couleur));
        let attendus = rang + 2;
        dalle.attendre(&format!("l'image n° {}", rang + 1), move |etat| {
            etat.gestes.len() >= attendus
        });
    }

    let _ = fil.arreter();

    let fils = dalle.fils();
    assert_eq!(
        fils.len(),
        1,
        "la dalle doit être touchée par un seul et unique fil ; {} l'ont touchée, et deux \
         processus qui écrivent sur le même nœud USB, c'est précisément ce que l'ADR-002 interdit",
        fils.len()
    );
    assert_ne!(
        fils[0], principal,
        "le fil qui pousse le mégaoctet ne doit jamais être celui qui anime les LED et sert le \
         socket : c'est le `D` de l'échantillon de #83"
    );
}

// ---------------------------------------------------------------------------
// 2 — l'image arrive vraiment, et telle quelle
// ---------------------------------------------------------------------------

#[test]
fn l_image_deposee_arrive_entiere_et_telle_quelle() {
    // ⚠️ Le piège du chantier, et ce test n'existe que pour lui. Tous les tests de non-blocage de
    // ce fichier passeraient sur un correctif qui aurait simplement **débranché l'écran** : un fil
    // qui ne pousse jamais rien ne fige évidemment plus personne, et l'utilisateur découvrirait la
    // régression devant une dalle noire.
    //
    // Deux choses sont donc exigées ici, et pas une seule : l'image arrive, et c'est **la bonne**,
    // avec ses 1 228 800 octets. Une dalle tronquée est ignorée par le contrôleur sans le moindre
    // code d'erreur (spec §2.2.1) — elle ne se rattraperait sur aucun autre test.
    let dalle = DalleFactice::qui_repond();
    let fil = FilEcran::demarrer(dalle.afficheur());

    fil.afficher(Dalle::unie(PREMIERE));
    dalle.attendre("l'image déposée", |etat| !etat.gestes.is_empty());

    let _ = fil.arreter();

    assert_eq!(
        dalle.gestes(),
        vec![image_de(PREMIERE)],
        "l'image déposée doit arriver à la dalle, entière et de la couleur demandée — un fil qui \
         ne pousse rien ne fige plus personne, et n'affiche plus rien non plus"
    );
}

// ---------------------------------------------------------------------------
// 3 — une image plus récente remplace celle qui attendait
// ---------------------------------------------------------------------------

#[test]
fn une_image_plus_recente_remplace_celle_qui_attendait() {
    // Comportement n° 2 de l'issue, et critère d'acceptation : « Une image obsolète est
    // **abandonnée**, pas mise en file : si une nouvelle composition arrive pendant qu'une autre
    // part, c'est la nouvelle qui compte. »
    //
    // Ce qu'il empêche de revenir : une file d'attente qui accumule. Une composition à sonde est
    // recomposée toutes les 2 s (#80) et un envoi dure bien plus longtemps ; une file ferait
    // afficher des températures **périmées**, de plus en plus vieilles, sur un écran qui n'aurait
    // jamais l'air en panne. Un cadran qui retarde est pire qu'un cadran qui saute.
    //
    // ⚠️ Et il exige que ce soit la **dernière** qui arrive : la comparaison est exacte, sur trois
    // couleurs distinctes, donc une implémentation qui garderait la première au lieu de la dernière
    // ne peut pas passer.
    let dalle = DalleFactice::qui_repond();
    let fil = FilEcran::demarrer(dalle.afficheur());

    // On coince la dalle au milieu de son envoi : à partir de là, tout ce qui est déposé attend.
    dalle.coincer();
    let fil = deposer("le premier dépôt", fil, |fil| {
        fil.afficher(Dalle::unie(PREMIERE));
    });
    dalle.attendre("le premier envoi, pris dans son mégaoctet", |etat| {
        !etat.gestes.is_empty()
    });

    let fil = deposer("trois dépôts pendant l'envoi", fil, |fil| {
        fil.afficher(Dalle::unie(DEUXIEME));
        fil.afficher(Dalle::unie(TROISIEME));
        fil.afficher(Dalle::unie(DERNIERE));
    });

    dalle.liberer();
    dalle.attendre("la dernière image déposée", |etat| {
        etat.gestes.contains(&image_de(DERNIERE))
    });

    let _ = fil.arreter();

    assert_eq!(
        dalle.gestes(),
        vec![image_de(PREMIERE), image_de(DERNIERE)],
        "deux images seulement devaient partir : celle qui était en cours, puis la plus récente. \
         Les deux intermédiaires étaient périmées avant même d'avoir été prises"
    );
}

// ---------------------------------------------------------------------------
// 4 — déposer ne bloque jamais, même sur une dalle qui ne consomme rien
// ---------------------------------------------------------------------------

#[test]
fn deposer_ne_bloque_jamais_meme_quand_la_dalle_ne_consomme_rien() {
    // Comportement n° 5 de l'issue : « Le fil principal ne bloque pas quand la dalle ne consomme
    // rien. »
    //
    // Ce qu'il empêche de revenir, c'est le remède naïf : sortir l'écran du fil principal mais lui
    // parler par un canal **synchrone**, où déposer attend que le fil d'en face prenne. Le gel
    // serait alors exactement le même qu'avant, à un `mpsc` près — et il se serait déplacé sans se
    // corriger.
    //
    // Cent dépôts, parce qu'une file d'attente qui ne bloque pas mais qui **empile** passerait sur
    // trois : ici, c'est la borne haute sur le nombre de gestes qui la refuse.
    let dalle = DalleFactice::qui_repond();
    let fil = FilEcran::demarrer(dalle.afficheur());

    dalle.coincer();
    let fil = deposer("le dépôt qui coince la dalle", fil, |fil| {
        fil.afficher(Dalle::unie(PREMIERE));
    });
    dalle.attendre("l'envoi qui ne finira pas", |etat| !etat.gestes.is_empty());

    let fil = deposer("cent dépôts sur une dalle muette", fil, |fil| {
        for _ in 0..DEPOTS_SUR_UNE_DALLE_MUETTE {
            fil.afficher(Dalle::unie(DEUXIEME));
        }
        fil.luminosite(LUMINOSITE);
        fil.afficher(Dalle::unie(DERNIERE));
    });

    // Le piège du fichier : tout ce qui précède serait vrai d'un fil qui n'affiche plus rien du
    // tout. On libère donc, et on exige que la dernière image arrive pour de bon.
    dalle.liberer();
    dalle.attendre("la dernière image, une fois la dalle libérée", |etat| {
        etat.gestes.contains(&image_de(DERNIERE))
    });

    let _ = fil.arreter();

    let gestes = dalle.gestes();
    assert!(
        gestes.len() <= 4,
        "cent-trois ordres déposés sur une dalle coincée ne doivent pas s'empiler : au plus l'envoi en \
         cours, la luminosité et la dernière image devaient partir ; {} gestes sont arrivés — {:?}",
        gestes.len(),
        gestes
    );
    assert_eq!(
        gestes.first(),
        Some(&image_de(PREMIERE)),
        "le premier envoi, celui qui était en cours, doit être arrivé ; trouvé : {:?}",
        gestes.first()
    );
    assert_eq!(
        gestes.last(),
        Some(&image_de(DERNIERE)),
        "et c'est la toute dernière image déposée qui doit finir sur la dalle ; trouvé : {:?}",
        gestes.last()
    );
}

// ---------------------------------------------------------------------------
// 5 — cent millisecondes, pendant qu'un mégaoctet part
// ---------------------------------------------------------------------------

#[test]
fn le_fil_principal_reste_sous_cent_millisecondes_pendant_qu_un_megaoctet_part() {
    // Critère d'acceptation : « `status`, `zone list` et `geometry` répondent en moins de 100 ms
    // **pendant** un envoi vers la dalle ».
    //
    // Ces trois commandes ne touchent aucun matériel — et c'est bien ce qui rend le défaut de #83
    // intolérable : elles étaient gelées par un envoi qui ne les concerne en rien. Ce qui les
    // gelait est la seule chose mesurée ici : la disponibilité du fil qui les sert.
    //
    // La dalle est coincée par rendez-vous et non par un `sleep` : elle est donc **certainement**
    // en plein envoi pendant toute la mesure, ce qu'un délai choisi au jugé ne garantirait jamais
    // sur une machine chargée.
    let dalle = DalleFactice::qui_repond();
    let fil = FilEcran::demarrer(dalle.afficheur());

    dalle.coincer();
    let fil = deposer("le dépôt qui coince la dalle", fil, |fil| {
        fil.afficher(Dalle::unie(PREMIERE));
    });
    dalle.attendre("l'envoi en cours", |etat| !etat.gestes.is_empty());

    let (fil, duree) = sans_bloquer("cent tours de service pendant l'envoi", fil, |fil| {
        let debut = Instant::now();
        for _ in 0..SERVICES {
            let _ = fil.verdicts();
        }
        for _ in 0..IMAGES_PENDANT_L_ENVOI {
            fil.afficher(Dalle::unie(DEUXIEME));
        }
        fil.afficher(Dalle::unie(DERNIERE));
        debut.elapsed()
    });

    assert!(
        duree < REPONSE_MAXIMALE,
        "{SERVICES} tours de service et {} dépôts ont pris {duree:?} pendant qu'un mégaoctet \
         partait : le critère borne une seule réponse à {REPONSE_MAXIMALE:?}, et c'est déjà cent \
         fois moins de travail",
        IMAGES_PENDANT_L_ENVOI + 1
    );
    assert_eq!(
        dalle.gestes().len(),
        1,
        "la dalle devait être encore prise dans son envoi pendant toute la mesure — sinon celle-ci \
         n'a pas eu lieu « pendant un envoi », et elle ne démontre rien"
    );

    // Et l'écran n'a pas été débranché pour autant.
    dalle.liberer();
    dalle.attendre("la dernière image, une fois la dalle libérée", |etat| {
        etat.gestes.contains(&image_de(DERNIERE))
    });
    let _ = fil.arreter();
}

// ---------------------------------------------------------------------------
// 6 — la luminosité, avant l'image et jamais deux fois
// ---------------------------------------------------------------------------

#[test]
fn une_luminosite_precede_toujours_l_image_qui_la_suit() {
    // Critère d'acceptation : « La luminosité part **avant** l'image et dans le même ordre (spec
    // §3.4 : `30 02` réinitialise le pipeline d'affichage) ».
    //
    // C'est un ordre matériel, pas une préférence : la trame de luminosité réinitialise le pipeline
    // d'affichage, donc une luminosité arrivée **après** une image efface l'image. Le README le
    // dit déjà en toutes lettres — « luminosité de l'écran avant l'image, jamais après ».
    //
    // Ce que ce test empêche de revenir : un canal à une seule place qui mêlerait les deux natures
    // d'ordre. Les deux sont déposés pendant que la dalle est coincée, donc c'est bien la file
    // d'attente qui est mise à l'épreuve, pas la chance d'un ordonnancement.
    let dalle = DalleFactice::qui_repond();
    let fil = FilEcran::demarrer(dalle.afficheur());

    dalle.coincer();
    let fil = deposer("le dépôt qui coince la dalle", fil, |fil| {
        fil.afficher(Dalle::unie(PREMIERE));
    });
    dalle.attendre("l'envoi en cours", |etat| !etat.gestes.is_empty());

    let fil = deposer("une luminosité puis une image", fil, |fil| {
        fil.luminosite(LUMINOSITE);
        fil.afficher(Dalle::unie(DERNIERE));
    });

    dalle.liberer();
    dalle.attendre("l'image qui suit la luminosité", |etat| {
        etat.gestes.contains(&image_de(DERNIERE))
    });

    let _ = fil.arreter();

    assert_eq!(
        dalle.gestes(),
        vec![
            image_de(PREMIERE),
            Geste::Luminosite(LUMINOSITE),
            image_de(DERNIERE)
        ],
        "la luminosité doit partir avant l'image qu'elle accompagne : dans l'autre ordre, la trame \
         `30 02` réinitialise le pipeline et efface l'image qu'on vient de pousser (spec §3.4)"
    );
}

#[test]
fn une_luminosite_n_est_ni_perdue_ni_doublee() {
    // Comportement n° 3 de l'issue : « Un ordre de luminosité n'est jamais perdu ni doublé ».
    //
    // Les deux moitiés comptent, et elles cassent de deux façons opposées :
    //
    // - **perdue** — une file à une seule place qui traiterait la luminosité comme une image, où la
    //   plus récente écrase celle qui attend. `screen --brightness 10` juste avant une image ne
    //   ferait alors rien du tout, silencieusement.
    // - **doublée** — une luminosité gardée « au cas où » et réémise avec chaque image. Une image
    //   fixe repart toutes les 25 s et un GIF toutes les 100 ms (README) : ce serait une
    //   réinitialisation du pipeline d'affichage dix fois par seconde, sur un contrôleur que la
    //   spec signale déjà comme sensible.
    let dalle = DalleFactice::qui_repond();
    let fil = FilEcran::demarrer(dalle.afficheur());

    dalle.coincer();
    let fil = deposer("le dépôt qui coince la dalle", fil, |fil| {
        fil.afficher(Dalle::unie(PREMIERE));
    });
    dalle.attendre("l'envoi en cours", |etat| !etat.gestes.is_empty());

    // Deux luminosités et trois images, toutes déposées pendant que la dalle est coincée. Les
    // images se remplacent ; les luminosités, non.
    let fil = deposer("deux luminosités et trois images", fil, |fil| {
        fil.luminosite(LUMINOSITE);
        fil.luminosite(AUTRE_LUMINOSITE);
        fil.afficher(Dalle::unie(DEUXIEME));
        fil.afficher(Dalle::unie(TROISIEME));
        fil.afficher(Dalle::unie(DERNIERE));
    });

    dalle.liberer();
    dalle.attendre("la dernière image du lot", |etat| {
        etat.gestes.contains(&image_de(DERNIERE))
    });

    // Puis deux images de plus, une à une : aucune ne doit traîner une luminosité derrière elle.
    let mut fil = fil;
    for (rang, couleur) in [PREMIERE, DEUXIEME].into_iter().enumerate() {
        fil = deposer("une image de rappel", fil, move |fil| {
            fil.afficher(Dalle::unie(couleur));
        });
        let attendus = 5 + rang;
        dalle.attendre(&format!("l'image de rappel n° {}", rang + 1), move |etat| {
            etat.gestes.len() >= attendus
        });
    }

    let _ = fil.arreter();

    assert_eq!(
        dalle.gestes(),
        vec![
            image_de(PREMIERE),
            Geste::Luminosite(LUMINOSITE),
            Geste::Luminosite(AUTRE_LUMINOSITE),
            image_de(DERNIERE),
            image_de(PREMIERE),
            image_de(DEUXIEME),
        ],
        "les deux luminosités doivent partir, chacune une fois, dans l'ordre où elles ont été \
         demandées et avant l'image qui les suit — et aucune ne doit repartir avec les images \
         suivantes"
    );
}

// ---------------------------------------------------------------------------
// 7 — la vigie de #70 renonce toujours, mais depuis son propre fil
// ---------------------------------------------------------------------------

#[test]
fn trois_refus_d_affilee_font_renoncer_la_vigie_depuis_son_propre_fil() {
    // Comportement n° 4 de l'issue et critère d'acceptation : « Un Kraken qui ne répond pas ne met
    // plus le fil principal en `D` : la vigie renonce sans que les LED s'arrêtent. »
    //
    // Ce qu'il empêche de revenir : les cinq secondes de #68, multipliées par trois. Chaque
    // tentative sur un contrôleur muet bloque cinq secondes en sommeil non interruptible ; sur le
    // fil principal, cela faisait quinze secondes pendant lesquelles `zone list` et `geometry`, qui
    // ne touchent aucun matériel, ne répondaient plus.
    //
    // Le décompte lui-même — N refus, N-1 qui n'arrêtent rien, la dernière erreur nommée — reste
    // celui de `spec_plafond.rs`, et ce chantier ne doit pas le changer. Ce qui est vérifié ici,
    // c'est qu'il a **changé de fil** sans changer d'avis.
    let dalle = DalleFactice::qui_refuse();
    let fil = FilEcran::demarrer(dalle.afficheur());
    let principal = thread::current().id();
    let plafond = ECHECS_AVANT_ABANDON as usize;

    let mut vus: Vec<Verdict> = Vec::new();
    let mut fil = fil;
    for refus in 1..=plafond {
        fil = deposer("un dépôt sur une dalle qui refuse", fil, |fil| {
            fil.afficher(Dalle::unie(PREMIERE));
        });
        dalle.attendre(&format!("le refus n° {refus}"), move |etat| {
            etat.gestes.len() >= refus
        });
        vus.extend(fil.verdicts());
    }

    // La dalle se remet à répondre — et ça ne doit rien changer : la vigie a renoncé, et le
    // rétablissement est explicite ou n'est pas (#70).
    dalle.se_retablit();
    let fil = deposer("cent tours de martèlement après l'abandon", fil, |fil| {
        for _ in 0..MARTELEMENT {
            fil.afficher(Dalle::unie(DERNIERE));
        }
    });

    vus.extend(fil.arreter());

    assert_eq!(
        dalle.gestes().len(),
        plafond,
        "après {plafond} refus d'affilée, plus rien ne doit partir vers la dalle : {} gestes sont \
         arrivés, ce qui est le harcèlement que #70 a borné",
        dalle.gestes().len()
    );

    let abandons: Vec<&Verdict> = vus
        .iter()
        .filter(|verdict| matches!(verdict, Verdict::Abandon { .. }))
        .collect();
    assert_eq!(
        abandons.len(),
        1,
        "l'abandon se prononce une fois et une seule, pour être journalisé une fois ; {} trouvés \
         parmi {vus:?}",
        abandons.len()
    );
    assert_eq!(
        abandons[0],
        &Verdict::Abandon {
            erreur: message(ETIMEDOUT)
        },
        "et il revient au fil principal en nommant son erreur — sans quoi le journal ne dirait plus \
         pourquoi la dalle s'est tue"
    );

    let fils = dalle.fils();
    assert_eq!(
        fils.len(),
        1,
        "les refus doivent tous être encaissés par le même fil, et lui seul ; {} fils ont touché la \
         dalle",
        fils.len()
    );
    assert_ne!(
        fils[0], principal,
        "les cinq secondes de sommeil non interruptible d'une tentative refusée ne doivent plus \
         jamais être payées par le fil qui anime les LED et sert le socket"
    );
}

#[test]
fn une_relance_fait_repartir_l_emission_sans_reveiller_le_fil_principal() {
    // Critère d'acceptation : « rien ne change pour l'utilisateur, sauf que ça ne clignote plus ».
    // Une commande `screen` relance l'émission après un abandon (#70) — cette porte de sortie doit
    // survivre au déménagement, sinon une dalle qui a bafouillé trois fois resterait noire jusqu'au
    // redémarrage du démon.
    //
    // Ce test vérifie aussi le seul point que le déménagement rend nouveau : la relance se demande
    // depuis le fil principal, et **elle non plus** ne le fait pas attendre.
    //
    // ⚠️ Et il exige que la relance **prenne son rang dans la file**, comme les autres ordres. Elle
    // est ici déposée pendant que le dernier refus est encore en cours : une relance qui doublerait
    // la file — un drapeau partagé, par exemple — serait posée *avant* que l'abandon soit prononcé,
    // donc effacée par lui, et la dalle resterait noire pour de bon. Le scénario est celui de
    // l'utilisateur : on tape `reverb screen --image …` parce que l'écran vient de s'éteindre,
    // c'est-à-dire juste au moment où il s'éteint.
    let dalle = DalleFactice::qui_refuse();
    let fil = FilEcran::demarrer(dalle.afficheur());
    let plafond = ECHECS_AVANT_ABANDON as usize;

    // Les N-1 premiers refus, un par un.
    let mut fil = fil;
    for refus in 1..plafond {
        fil = deposer("un dépôt sur une dalle qui refuse", fil, |fil| {
            fil.afficher(Dalle::unie(PREMIERE));
        });
        dalle.attendre(&format!("le refus n° {refus}"), move |etat| {
            etat.gestes.len() >= refus
        });
    }

    // Le dernier, celui qui fera déborder le compte : on le coince en cours de route.
    dalle.coincer();
    let fil = deposer("le dépôt qui fera déborder le compte", fil, |fil| {
        fil.afficher(Dalle::unie(PREMIERE));
    });
    dalle.attendre("le dernier refus, en cours", move |etat| {
        etat.gestes.len() >= plafond
    });

    // Pendant qu'il est en cours, l'utilisateur relance et redemande une image.
    let fil = deposer("la relance puis une image", fil, |fil| {
        fil.relancer();
        fil.afficher(Dalle::unie(DERNIERE));
    });

    // La dalle se remet à répondre, puis le dernier refus se conclut : abandon, relance, image.
    dalle.se_retablit();
    dalle.liberer();
    dalle.attendre("l'image qui suit la relance", |etat| {
        etat.gestes.contains(&image_de(DERNIERE))
    });

    let _ = fil.arreter();

    assert_eq!(
        dalle.gestes().len(),
        plafond + 1,
        "après la relance, exactement une image devait repartir — les {plafond} refus, puis elle ; \
         {} gestes trouvés, ce qui trahit une émission qui n'avait jamais cessé",
        dalle.gestes().len()
    );
    assert_eq!(
        dalle.gestes().last(),
        Some(&image_de(DERNIERE)),
        "et c'est bien l'image demandée après la relance qui arrive sur la dalle"
    );
}

// ---------------------------------------------------------------------------
// 8 — le verdict revient sans jamais faire attendre
// ---------------------------------------------------------------------------

#[test]
fn les_verdicts_reviennent_sans_jamais_faire_attendre_le_fil_principal() {
    // Approche technique de l'issue : « le verdict de #70 revient par un second canal, **lu sans
    // attendre** au tour suivant ».
    //
    // Ce qu'il empêche de revenir : le gel par la porte de derrière. Un fil principal qui déposerait
    // sans bloquer mais qui **attendrait le verdict** avant de continuer aurait exactement le même
    // comportement qu'aujourd'hui — les 1,2 Mo seraient toujours dans le chemin critique des LED,
    // par le canal de retour au lieu du canal aller.
    let dalle = DalleFactice::qui_repond();
    let fil = FilEcran::demarrer(dalle.afficheur());

    // Une poignée neuve n'a rien à dire, et le lui demander ne doit rien coûter.
    let (fil, (vides, duree)) = sans_bloquer("lire les verdicts d'un fil neuf", fil, |fil| {
        let debut = Instant::now();
        let verdicts = fil.verdicts();
        (verdicts, debut.elapsed())
    });
    assert!(
        vides.is_empty(),
        "un fil qui n'a rien poussé n'a rien à dire ; trouvé : {vides:?}"
    );
    assert!(
        duree < REPONSE_MAXIMALE,
        "lire les verdicts d'un fil au repos a pris {duree:?} : c'est une lecture, pas une attente"
    );

    // La dalle est coincée : le verdict de cet envoi ne peut pas encore exister.
    dalle.coincer();
    let fil = deposer("un dépôt sur une dalle coincée", fil, |fil| {
        fil.afficher(Dalle::unie(PREMIERE));
    });
    dalle.attendre("l'envoi en cours", |etat| !etat.gestes.is_empty());

    let (fil, (pendant, duree)) = sans_bloquer("lire les verdicts pendant l'envoi", fil, |fil| {
        let debut = Instant::now();
        let verdicts = fil.verdicts();
        (verdicts, debut.elapsed())
    });
    assert!(
        duree < REPONSE_MAXIMALE,
        "lire les verdicts pendant qu'un mégaoctet part a pris {duree:?} : le canal de retour ne \
         doit pas faire attendre le fil qui anime les LED"
    );
    assert!(
        pendant.is_empty(),
        "l'envoi n'est pas fini : il n'a pas encore de verdict à rendre ; trouvé : {pendant:?}"
    );

    dalle.liberer();
    dalle.attendre("la fin de l'envoi", |etat| !etat.gestes.is_empty());

    let derniers = fil.arreter();
    assert!(
        derniers.contains(&Verdict::Emise),
        "l'envoi réussi doit finir par se dire — un verdict perdu, c'est un abandon (#70) qui ne se \
         journaliserait jamais ; trouvé : {derniers:?}"
    );
}

// ---------------------------------------------------------------------------
// 9 — ce que les types doivent permettre
// ---------------------------------------------------------------------------

#[test]
fn l_image_la_poignee_et_le_verdict_traversent_la_frontiere_des_fils() {
    // Ce test ne s'exécute pas vraiment : il casse à la **compilation** si l'un de ces trois types
    // cesse de pouvoir franchir une frontière de fils.
    //
    // C'est la condition matérielle de tout le reste : une `Dalle` qui ne serait pas `Send` ne
    // pourrait pas être déposée, un `Verdict` qui ne le serait pas ne pourrait pas revenir, et une
    // poignée qui ne le serait pas interdirait au démon de la ranger où il veut. Aucun des huit
    // tests précédents ne compilerait — mais le message d'erreur, lui, désignerait la dalle factice
    // du fichier plutôt que la vraie cause.
    fn exige_send<T: Send + 'static>() {}

    exige_send::<Dalle>();
    exige_send::<FilEcran>();
    exige_send::<Verdict>();
}
