//! Ce que valent les deux curseurs, et quand ils parlent au démon.
//!
//! Ce module n'existe que parce que les deux curseurs de la fenêtre ont chacun
//! un défaut, et qu'aucun des deux ne se teste dans du Slint : la logique en
//! sort pour être vérifiable.
//!
//! # Le curseur de vitesse ne disait rien
//!
//! Il n'écrivait qu'une propriété de l'interface. Rien ne renvoyait l'animation,
//! donc le démon gardait la vitesse d'avant jusqu'au prochain clic sur le bouton
//! de l'animation. [`Reglage::commande`] est la réponse : à chaque changement,
//! elle dit s'il y a une commande à envoyer, et laquelle.
//!
//! # La poignée d'un ventilateur sautait sous les doigts
//!
//! Sa valeur était liée à la mesure, que la télémétrie réécrit chaque seconde.
//! Or un ventilateur met plusieurs secondes à rejoindre une consigne : pendant
//! ce temps, la mesure **contredit** ce que l'utilisateur vient de demander, et
//! la poignée revenait en arrière toute seule.
//!
//! [`Poignee`] arbitre : la consigne l'emporte tant qu'elle est fraîche, la
//! mesure reprend la main ensuite. C'est un état par canal, pas un verrou
//! global — deux ventilateurs se règlent indépendamment.

use std::time::Duration;

use reverb_proto::Rgb;
use reverb_proto::ipc::Request;

/// Combien de temps une consigne l'emporte sur la mesure après le relâchement.
///
/// Un ventilateur de 140 mm met environ deux secondes à passer d'un régime à un
/// autre, et la télémétrie arrive chaque seconde. Sous ce délai, la poignée
/// reviendrait en arrière sous les yeux de celui qui vient de la poser.
pub const GRACE: Duration = Duration::from_secs(3);

/// Les réglages d'animation tels que la fenêtre les affiche.
#[derive(Debug, Clone, PartialEq)]
pub struct Reglage {
    /// L'animation en cours, `None` si aucune ne tourne.
    pub animation: Option<String>,
    pub couleur: Rgb,
    pub vitesse: u8,
    /// Rang dans `reverb_anim::Direction::ALL`.
    pub direction: usize,
}

impl Reglage {
    /// La commande à envoyer pour que le démon applique ces réglages.
    ///
    /// `None` quand aucune animation ne tourne : bouger la vitesse à vide ne
    /// doit rien envoyer, sinon la fenêtre démarrerait une animation que
    /// personne n'a demandée.
    ///
    /// Les clés portées sont **celles que l'animation accepte**, et elles
    /// seules : `arc-en-ciel` refuse `couleur`, et la lui donner ferait refuser
    /// la commande entière.
    pub fn commande(&self) -> Option<Request> {
        todo!("issue #32")
    }
}

/// L'état d'une poignée de ventilateur : consigne contre mesure.
#[derive(Debug, Clone, PartialEq)]
pub struct Poignee {
    _prive: (),
}

impl Poignee {
    /// Une poignée qui n'a encore rien vu : elle suit la mesure.
    pub fn nouvelle() -> Poignee {
        todo!("issue #32")
    }

    /// L'utilisateur tire la poignée à cette valeur.
    pub fn saisir(&mut self, _consigne: u8, _maintenant: Duration) {
        todo!("issue #32")
    }

    /// Il la lâche. La consigne tient encore [`GRACE`].
    pub fn relacher(&mut self, _maintenant: Duration) {
        todo!("issue #32")
    }

    /// Une mesure arrive du démon.
    pub fn mesurer(&mut self, _pwm: u8, _maintenant: Duration) {
        todo!("issue #32")
    }

    /// Le canal repasse à la courbe du firmware : la mesure reprend la main
    /// tout de suite, sans attendre la fin de la grâce.
    pub fn liberer(&mut self) {
        todo!("issue #32")
    }

    /// Ce que le curseur doit afficher.
    pub fn affichee(&self) -> u8 {
        todo!("issue #32")
    }

    /// La consigne à envoyer au démon, une seule fois par valeur.
    ///
    /// Rend `Some` au premier appel qui suit un changement de consigne, `None`
    /// ensuite : un glissement continu produit une commande par pas franchi,
    /// pas une par image.
    pub fn a_envoyer(&mut self) -> Option<u8> {
        todo!("issue #32")
    }
}
