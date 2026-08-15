//! Le fil qui réinitialise, et lui seul (issue #98).
//!
//! # Pourquoi un fil, alors que la décision est pure
//!
//! [`crate::reparation`] ne touche à rien : elle décide. Le **geste**, lui, est un
//! `USBDEVFS_RESET` sur un périphérique en difficulté, et personne n'a mesuré ce
//! qu'il coûte — il fait disparaître puis réapparaître un nœud sur le bus, le
//! noyau réénumère, les pilotes se relient. Le poser sur le fil qui anime les LED
//! et sert le socket (ADR-002), c'est refaire exactement la faute que #83 a
//! corrigée pour la dalle et #68 pour les sondes : **le plus lent des quatre gèle
//! les trois autres**.
//!
//! ⚠️ **Le même défaut a été traité quatre fois, sur les quatre chemins qui le
//! portaient** : les sondes (#68), la dalle (#83), les canaux de ventilation
//! (#88), et maintenant la réparation. À chaque fois, un périphérique qui ne
//! répond plus à son pilote noyau gelait le fil qui sert le socket.
//!
//! # Ce que ce module déplace, et ce qu'il ne rompt pas
//!
//! **La décision voyage avec le fil, elle ne reste pas derrière.** C'est ce qui
//! distingue cette couture de celle de la dalle : [`crate::reparation::Reparations`]
//! est déplacée *dans* le fil, qui reçoit des [`EtatSource`] et rend des
//! [`Constat`]. Laisser la décision au fil principal l'obligerait à **attendre**
//! le verdict de l'`ioctl` pour savoir s'il a réussi — la fermeture rend un
//! `io::Result`, elle ne se dépose pas —, et le mégaoctet reviendrait par la porte
//! de derrière.
//!
//! **Deux tentatives ne peuvent pas se chevaucher, par construction.** Un seul fil
//! traite un état à la fois, et il détient seul le réparateur — [`FilReparation::demarrer`]
//! le prend **par valeur**. Ce n'est pas une discipline à tenir, c'est le
//! vérificateur d'emprunts qui la tient.
//!
//! **Un état périmé ne se juge pas.** L'état d'une source **remplace** celui qui
//! attendait encore : le fil principal en dépose un par seconde, un reset dure
//! bien plus longtemps que ça, et une file qui accumule ferait juger, après coup,
//! des effondrements que la réalité a déjà démentis. C'est la règle des images de
//! #83, pour la même raison.
//!
//! **Les tours sans geste ne sont pas annoncés.** `Rien`, `Patiente` et `Repos`
//! disent qu'il ne s'est rien passé ; les remonter noierait le journal d'une ligne
//! par seconde et par source, dans un journal qu'on lit justement pour trouver ce
//! genre d'incident. C'est la règle du `Repos` de la dalle (#83).

use std::collections::VecDeque;
use std::io;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::reparation::{Constat, EtatSource, Reparations};

/// Ce que le fil de réparation sait demander au matériel.
///
/// Une seule implémentation en production — le `USBDEVFS_RESET` du Kraken — et
/// une factice dans les tests. C'est cette couture qui rend #98 vérifiable sans
/// réinitialiser un périphérique de la machine qui lance les tests.
pub trait Reparateur: Send {
    /// Les sources que ce réparateur sait remettre en route.
    ///
    /// Interrogée **une fois**, avant que le réparateur ne parte dans son fil :
    /// le fil principal a besoin de savoir de quelles sources il vaut la peine de
    /// déposer l'état, et il ne peut plus lui parler ensuite.
    fn sources(&self) -> Vec<String>;

    /// Réinitialise le périphérique qui porte cette source.
    ///
    /// ⚠️ `Ok` qualifie le geste, jamais la guérison — voir
    /// [`crate::reparation::Constat::Reussie`].
    fn reinitialiser(&mut self, source: &str) -> io::Result<()>;
}

/// La file partagée entre le fil principal et celui de la réparation.
struct File {
    /// Au plus un état par source, dans l'ordre de dépôt.
    etats: VecDeque<(EtatSource, Duration)>,
    /// Posé par [`FilReparation::arreter`] ou par le `Drop` de la poignée.
    fini: bool,
}

/// Le rendez-vous entre les deux fils.
struct Boite {
    file: Mutex<File>,
    reveil: Condvar,
}

/// Prend le verrou, y compris s'il a été empoisonné.
///
/// ⚠️ **Un `unwrap` ici ferait tomber le boîtier avec la réparation.** Un verrou
/// empoisonné veut dire que le fil de réparation a paniqué ; c'est une réparation
/// perdue, pas une raison d'éteindre les LED. Les données protégées restent
/// cohérentes, et le pire qui puisse en découler est un état de trop dans la file.
fn verrou(file: &Mutex<File>) -> MutexGuard<'_, File> {
    file.lock().unwrap_or_else(PoisonError::into_inner)
}

/// La poignée que le fil principal garde sur le fil de réparation.
///
/// **Aucune de ses méthodes ne bloque** — c'est tout l'objet de ce module. Elles
/// prennent un verrou tenu le temps de pousser un élément dans une file ; le
/// `USBDEVFS_RESET`, lui, a lieu hors du verrou et sur l'autre fil.
pub struct FilReparation {
    boite: Arc<Boite>,
    constats: Receiver<(String, Constat)>,
    /// Ce que le réparateur a déclaré savoir faire, relevé avant son départ.
    sources: Vec<String>,
    /// `None` une fois [`FilReparation::arreter`] passé : le fil est déjà joint.
    fil: Option<JoinHandle<()>>,
}

impl FilReparation {
    /// Démarre le fil dédié, qui détient `reparateur` **seul**.
    ///
    /// Personne ne peut plus le toucher : il a été déplacé.
    pub fn demarrer<R: Reparateur + 'static>(reparateur: R) -> FilReparation {
        let sources = reparateur.sources();
        let boite = Arc::new(Boite {
            file: Mutex::new(File {
                etats: VecDeque::new(),
                fini: false,
            }),
            reveil: Condvar::new(),
        });
        let (envoi, constats) = channel();
        let sienne = Arc::clone(&boite);
        let fil = thread::spawn(move || tourner(&sienne, reparateur, &envoi));

        FilReparation {
            boite,
            constats,
            sources,
            fil: Some(fil),
        }
    }

    /// Les sources dont il vaut la peine de déposer l'état.
    ///
    /// Déposer celui d'une source que personne ne sait réparer ferait brûler trois
    /// tentatives et une ligne d'abandon pour apprendre ce qu'on savait déjà.
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    /// Dépose l'état d'une source à juger. **Ne bloque pas.**
    ///
    /// Il **remplace** celui de la même source qui attendait encore : un
    /// effondrement d'il y a trois secondes n'a aucune raison d'être jugé derrière
    /// celui qui le corrige.
    pub fn soumettre(&self, etat: EtatSource, maintenant: Duration) {
        let mut file = verrou(&self.boite.file);
        file.etats.retain(|(autre, _)| autre.source != etat.source);
        file.etats.push_back((etat, maintenant));
        drop(file);
        self.boite.reveil.notify_one();
    }

    /// Les constats survenus depuis le dernier appel. **N'attend rien.**
    ///
    /// Un `Vec` et non un `Option` : entre deux tours de la boucle de rendu,
    /// plusieurs sources ont pu être jugées, et n'en rendre qu'une laisserait un
    /// abandon coincé derrière une réussite.
    pub fn constats(&self) -> Vec<(String, Constat)> {
        self.constats.try_iter().collect()
    }

    /// Demande l'arrêt, attend la fin du fil, et rend les constats qu'il n'avait
    /// pas encore dits.
    ///
    /// Sans ce dernier point, un reset fait juste avant l'arrêt ne laisserait
    /// aucune trace — or c'est le geste le plus visible que le démon pose.
    pub fn arreter(mut self) -> Vec<(String, Constat)> {
        self.signaler_la_fin();
        if let Some(fil) = self.fil.take() {
            let _ = fil.join();
        }
        self.constats()
    }

    fn signaler_la_fin(&self) {
        let mut file = verrou(&self.boite.file);
        file.fini = true;
        drop(file);
        self.boite.reveil.notify_all();
    }
}

impl Drop for FilReparation {
    /// Une poignée lâchée sans [`FilReparation::arreter`] ne laisse pas un fil
    /// dormir sur un `Condvar` pour toute la vie du processus. Le fil n'est pas
    /// joint ici : il termine le geste en cours puis sort.
    fn drop(&mut self) {
        if self.fil.is_some() {
            self.signaler_la_fin();
        }
    }
}

/// La boucle du fil dédié.
///
/// Un état est **sorti de la file avant** que le réparateur soit appelé, et le
/// verrou est relâché entre les deux : c'est ce qui permet au fil principal de
/// continuer à déposer pendant qu'un périphérique quitte le bus.
fn tourner(
    boite: &Arc<Boite>,
    mut reparateur: impl Reparateur,
    constats: &Sender<(String, Constat)>,
) {
    let mut reparations = Reparations::nouvelles();

    while let Some((etat, maintenant)) = prochain(boite) {
        let constat =
            reparations.tour(&etat, maintenant, || reparateur.reinitialiser(&etat.source));

        // Rien ne s'est passé : le dire à chaque tour et pour chaque source
        // remplirait le journal de silence.
        if matches!(constat, Constat::Rien | Constat::Patiente | Constat::Repos) {
            continue;
        }
        if constats.send((etat.source.clone(), constat)).is_err() {
            // Plus personne pour lire : la poignée est partie sans passer par
            // `arreter`.
            return;
        }
    }
}

/// Le prochain état à juger, ou `None` quand l'arrêt est demandé et la file vide.
fn prochain(boite: &Arc<Boite>) -> Option<(EtatSource, Duration)> {
    let mut file = verrou(&boite.file);
    loop {
        if let Some(etat) = file.etats.pop_front() {
            return Some(etat);
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
