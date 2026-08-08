//! Le fil qui détient la dalle, et lui seul (issue #83).
//!
//! # Le défaut que ce module corrige
//!
//! Une composition portant au moins une sonde est recomposée toutes les deux secondes et poussée
//! dès qu'elle diffère de la précédente (#80). Une température de CPU bouge en permanence : l'image
//! change donc à chaque fois, et **1,2 Mo partent sur l'USB toutes les deux secondes**. Cet envoi
//! était fait par le fil qui anime aussi les LED et sert le socket (ADR-002).
//!
//! Mesuré sur SHYNAEL le 2026-08-08, sur les lignes `img/s` du journal :
//!
//! | régime | img/s | sautées/s |
//! |---|---|---|
//! | dalle rendue au firmware | **21** | 11 |
//! | composition à deux sondes vivantes | **15** | 45 |
//! | dalle qui refuse (`ETIMEDOUT`) | **2** | 306 |
//! | dalle muette | **0** | — |
//!
//! La dernière ligne n'est pas une figure de style : le démon est resté **vingt minutes sans
//! consommer un tic de CPU**, cinq clients bloqués à attendre une réponse, `status` sans un octet
//! après quinze secondes.
//!
//! ⚠️ **Ce n'était pas un réglage à corriger, c'était architectural.** Le cache de #80 — « on ne
//! pousse que ce qui a changé » — n'y peut rien : quand la valeur change, il *faut* pousser. C'est
//! le même défaut de fond que #68, qui n'avait été traité que pour les sondes — un seul fil tenait
//! les bus, servait le socket, animait les LED et parlait à la dalle. Le plus lent des quatre gelait
//! les trois autres.
//!
//! # Ce que ce module déplace, et ce qu'il ne rompt pas
//!
//! La promesse de l'ADR-002 — **un seul processus détient les bus** — devient ici *un seul fil
//! détient le nœud USB de la dalle*, et elle est vraie **par construction** : [`FilEcran::demarrer`]
//! prend l'afficheur **par valeur** et le déplace dans son fil. Une fois démarré, plus personne
//! d'autre n'a de quoi lui parler. Ce n'est pas une discipline à tenir, c'est le vérificateur
//! d'emprunts qui la tient.
//!
//! Le fil principal continue de **décider et de composer** — c'est du calcul, pas de l'entrée-sortie
//! — et le relevé des sondes reste chez lui avec sa quarantaine (#68). Le déplacer aussi
//! dédoublerait l'état d'écartement pour un coût qui n'est pas celui-là.
//!
//! # Trois choix, et ce qu'ils achètent
//!
//! **La file remplace les images, elle les empile pour tout le reste.** Une image déposée pendant
//! qu'une autre part **chasse** celle qui attendait : une composition est recomposée toutes les deux
//! secondes et un envoi dure plus longtemps que ça, si bien qu'une file qui accumule afficherait des
//! températures de plus en plus vieilles sur un écran qui n'aurait jamais l'air en panne. Un cadran
//! qui retarde est pire qu'un cadran qui saute. Une luminosité, elle, n'est **ni perdue ni doublée**
//! — c'est un ordre, pas un état à rafraîchir.
//!
//! **L'image nouvelle reprend la queue de la file, elle ne prend pas la place de l'ancienne.** C'est
//! ce qui garde vraie la seule règle d'ordre du protocole : la luminosité part **avant** l'image qui
//! la suit, parce que `30 02` réinitialise le pipeline d'affichage (spec §3.4) et effacerait une
//! image déjà poussée. Reprendre la place de l'ancienne suffirait dans le cas courant et
//! inverserait l'ordre dès qu'une luminosité s'est glissée entre deux images.
//!
//! **La vigie de #70 voyage avec la dalle.** Elle vit sur ce fil parce que c'est lui qui essuie les
//! refus ; son verdict revient au fil principal par un canal lu **sans attendre**. Un verdict qu'on
//! attendrait remettrait les 1,2 Mo dans le chemin critique des LED, par la porte de derrière.
//!
//! ⚠️ **La relance prend son rang dans la file**, elle ne la double pas. Un drapeau partagé serait
//! appliqué *avant* l'abandon qui le motive — donc effacé par lui, et la dalle resterait noire pour
//! de bon. Le scénario n'est pas théorique : on tape `reverb screen --image …` **parce que** l'écran
//! vient de s'éteindre, c'est-à-dire juste au moment où il s'éteint.

use std::collections::VecDeque;
use std::io;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use crate::ecran::{Dalle, Verdict, Vigie};

/// Ce que le fil de la dalle sait demander au matériel.
///
/// Une seule implémentation en production — l'écran du Kraken, `MI_01` en HID pour les commandes et
/// `MI_00` en bulk pour les pixels — et une factice dans les tests. C'est cette couture qui rend
/// #83 vérifiable sans brancher de Kraken, et sans qu'un test automatisé n'écrive jamais sur un bus.
///
/// **Deux gestes et non un seul**, parce que leur ordre relatif est un critère d'acceptation :
/// confondus dans une fermeture unique, « la luminosité part avant l'image » deviendrait
/// invérifiable.
pub trait Afficheur: Send {
    /// Applique la luminosité, en pourcent.
    fn luminosite(&mut self, pourcent: u8) -> io::Result<()>;

    /// Pousse les 1 228 800 octets d'une image.
    fn image(&mut self, dalle: &Dalle) -> io::Result<()>;
}

/// Ce qu'on dépose à l'intention de la dalle.
enum Ordre {
    Luminosite(u8),
    Image(Dalle),
    /// Une commande `screen` explicite : la vigie repart de zéro (#70).
    Relancer,
}

/// La file partagée entre le fil principal et celui de la dalle.
struct File {
    ordres: VecDeque<Ordre>,
    /// Posé par [`FilEcran::arreter`] ou par le `Drop` de la poignée.
    fini: bool,
}

/// Le rendez-vous entre les deux fils.
struct Boite {
    file: Mutex<File>,
    reveil: Condvar,
}

/// Prend le verrou, y compris s'il a été empoisonné.
///
/// ⚠️ **Un `unwrap` ici ferait tomber le fil de rendu avec celui de la dalle.** Un verrou empoisonné
/// veut dire que le fil de l'écran a paniqué ; c'est un écran perdu, pas une raison d'éteindre le
/// boîtier. Les données protégées restent cohérentes — une file d'ordres qu'on n'a pas fini de
/// modifier reste une file d'ordres — et le pire qui puisse en découler est un ordre de trop dedans.
fn verrou(file: &Mutex<File>) -> MutexGuard<'_, File> {
    file.lock().unwrap_or_else(PoisonError::into_inner)
}

/// La poignée que le fil principal garde sur le fil de la dalle.
///
/// **Aucune de ses méthodes ne bloque** — c'est tout l'objet de #83. Elles prennent un verrou tenu
/// le temps de pousser un élément dans une file ; l'entrée-sortie, elle, a lieu hors du verrou.
pub struct FilEcran {
    boite: Arc<Boite>,
    verdicts: Receiver<Verdict>,
    /// `None` une fois [`FilEcran::arreter`] passé : le fil est déjà joint.
    fil: Option<JoinHandle<()>>,
}

impl FilEcran {
    /// Démarre le fil dédié, qui détient `afficheur` **seul**.
    ///
    /// Personne ne peut plus le toucher : il a été déplacé.
    pub fn demarrer<A: Afficheur + 'static>(afficheur: A) -> FilEcran {
        let boite = Arc::new(Boite {
            file: Mutex::new(File {
                ordres: VecDeque::new(),
                fini: false,
            }),
            reveil: Condvar::new(),
        });
        let (envoi, verdicts) = channel();
        let sienne = Arc::clone(&boite);
        let fil = thread::spawn(move || tourner(&sienne, afficheur, &envoi));

        FilEcran {
            boite,
            verdicts,
            fil: Some(fil),
        }
    }

    /// Dépose une image. Elle **remplace** celle qui attendait encore, et prend la queue de la file.
    pub fn afficher(&self, dalle: Dalle) {
        self.deposer(Ordre::Image(dalle));
    }

    /// Dépose un ordre de luminosité, qui ne sera ni perdu ni doublé.
    pub fn luminosite(&self, pourcent: u8) {
        self.deposer(Ordre::Luminosite(pourcent));
    }

    /// Remet la vigie en émission, comme le fait une commande `screen` (#70).
    ///
    /// Elle **prend son rang dans la file** : voir l'avertissement en tête de module.
    pub fn relancer(&self) {
        self.deposer(Ordre::Relancer);
    }

    /// Les verdicts survenus depuis le dernier appel. **N'attend rien.**
    ///
    /// Un `Vec` et non un `Option` : entre deux tours de la boucle de rendu plusieurs envois ont pu
    /// avoir lieu, et n'en rendre qu'un laisserait un abandon (#70) coincé derrière deux refus.
    pub fn verdicts(&self) -> Vec<Verdict> {
        self.verdicts.try_iter().collect()
    }

    /// Demande l'arrêt, attend la fin du fil, et rend les verdicts qu'il n'avait pas encore dits.
    ///
    /// Sans ce dernier point, ce que la dalle a dit en dernier serait perdu en silence à l'arrêt.
    pub fn arreter(mut self) -> Vec<Verdict> {
        self.signaler_la_fin();
        if let Some(fil) = self.fil.take() {
            let _ = fil.join();
        }
        self.verdicts()
    }

    fn deposer(&self, ordre: Ordre) {
        let mut file = verrou(&self.boite.file);
        if matches!(ordre, Ordre::Image(_)) {
            // Au plus une image attend à la fois : celle qui était là est périmée par celle-ci.
            file.ordres
                .retain(|autre| !matches!(autre, Ordre::Image(_)));
        }
        file.ordres.push_back(ordre);
        drop(file);
        self.boite.reveil.notify_one();
    }

    fn signaler_la_fin(&self) {
        let mut file = verrou(&self.boite.file);
        file.fini = true;
        drop(file);
        self.boite.reveil.notify_all();
    }
}

impl Drop for FilEcran {
    /// Une poignée lâchée sans [`FilEcran::arreter`] ne laisse pas un fil dormir sur un `Condvar`
    /// pour toute la vie du processus. Le fil n'est pas joint ici : il termine l'envoi en cours puis
    /// sort, et personne n'a de raison de l'attendre.
    fn drop(&mut self) {
        if self.fil.is_some() {
            self.signaler_la_fin();
        }
    }
}

/// La boucle du fil dédié.
///
/// Un ordre est **sorti de la file avant** que l'afficheur soit appelé, et le verrou est relâché
/// entre les deux : c'est ce qui permet au fil principal de continuer à déposer pendant qu'un
/// mégaoctet part.
fn tourner(boite: &Arc<Boite>, mut afficheur: impl Afficheur, verdicts: &Sender<Verdict>) {
    let mut vigie = Vigie::neuve();

    while let Some(ordre) = prochain(boite) {
        let verdict = match ordre {
            // Une relance ne touche pas le matériel : elle n'a pas de verdict à rendre.
            Ordre::Relancer => {
                vigie.relancer();
                continue;
            }
            Ordre::Luminosite(pourcent) => vigie.tour(|| afficheur.luminosite(pourcent)),
            Ordre::Image(dalle) => vigie.tour(|| afficheur.image(&dalle)),
        };

        // ⚠️ `Repos` n'est pas annoncé. Il dit « l'abandon était déjà prononcé, rien n'a été
        // écrit » : le fil principal l'a appris au premier `Abandon` et a cessé d'émettre. Le
        // remonter quand même noierait le journal d'un verdict par ordre resté en route — soit,
        // pour un GIF, dix par seconde —, ce qui est exactement le harcèlement que #70 a borné.
        if !matches!(verdict, Verdict::Repos) && verdicts.send(verdict).is_err() {
            // Plus personne pour lire : la poignée est partie sans passer par `arreter`.
            return;
        }
    }
}

/// Le prochain ordre, ou `None` quand l'arrêt est demandé et la file vide.
fn prochain(boite: &Arc<Boite>) -> Option<Ordre> {
    let mut file = verrou(&boite.file);
    loop {
        if let Some(ordre) = file.ordres.pop_front() {
            return Some(ordre);
        }
        if file.fini {
            return None;
        }
        file = boite
            .reveil
            .wait(file)
            .unwrap_or_else(PoisonError::into_inner);
    }
}
