//! Cadence de la boucle de rendu.
//!
//! Ne tient aucune horloge : c'est l'appelant qui lui donne l'instant. C'est ce
//! qui rend le décrochage testable sans jamais dormir — et le décrochage est
//! précisément ce qui se voit à l'œil quand on se trompe.

use std::time::Duration;

/// Cadence d'une boucle de rendu, sans horloge propre.
pub struct Cadence {
    periode: Duration,
    /// Instant de la prochaine image due.
    ///
    /// Avancée **par ajout de périodes**, jamais recalée sur l'instant reçu :
    /// un recalage ferait dériver la cadence d'un peu de retard à chaque
    /// image, et trente images par seconde deviendraient vingt-neuf.
    prochaine: Duration,
}

/// Ce qu'il y a à faire à un instant donné.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    /// Trop tôt. Dormir au plus ce délai avant de rappeler [`Cadence::tick`].
    Attendre(Duration),
    /// Produire **une** image. `sautees` compte celles délibérément passées
    /// depuis la précédente.
    Produire { sautees: u32 },
}

impl Cadence {
    /// Construit une cadence. `images_par_seconde` nul est ramené à un.
    ///
    /// Ramené plutôt que refusé : c'est une boucle de démon, et paniquer au
    /// démarrage sur une valeur de configuration aberrante coûterait
    /// l'éclairage entier pour une division par zéro.
    pub fn new(images_par_seconde: u32) -> Cadence {
        let images_par_seconde = images_par_seconde.max(1);
        Cadence {
            periode: Duration::from_nanos(1_000_000_000 / u64::from(images_par_seconde)),
            // La première image est due tout de suite : l'éclairage au
            // démarrage n'a aucune raison d'arriver avec une période de retard.
            prochaine: Duration::ZERO,
        }
    }

    /// Que faire à cet instant.
    ///
    /// `maintenant` est un temps écoulé depuis le démarrage, monotone et
    /// croissant.
    ///
    /// ⚠️ **Une boucle en retard saute, elle ne rattrape pas.** `Produire` ne
    /// rend jamais un nombre d'images à produire d'affilée : sinon une
    /// animation qui décroche part en avance rapide dès que la charge retombe,
    /// ce qui se voit immédiatement. `sautees` sert à le journaliser, pas à
    /// produire.
    pub fn tick(&mut self, maintenant: Duration) -> Tick {
        let Some(retard) = maintenant.checked_sub(self.prochaine) else {
            return Tick::Attendre(self.prochaine - maintenant);
        };

        // Combien d'échéances se sont écoulées en plus de celle qu'on honore.
        let sautees =
            u32::try_from(retard.as_nanos() / self.periode.as_nanos()).unwrap_or(u32::MAX);
        self.prochaine += self.periode * (sautees + 1);
        Tick::Produire { sautees }
    }
}
