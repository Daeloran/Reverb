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

use reverb_anim::{Animation, Direction};
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
        let nom = self.animation.as_ref()?;
        let animation = Animation::par_nom(nom).ok()?;
        let acceptees = animation.parametres_acceptes();
        let mut reglages = Vec::new();
        if acceptees.contains(&"couleur") {
            reglages.push(("couleur".to_owned(), hexa(self.couleur)));
        }
        if acceptees.contains(&"vitesse") {
            reglages.push(("vitesse".to_owned(), self.vitesse.to_string()));
        }
        if acceptees.contains(&"direction") {
            // Un rang hors des six n'envoie **rien**. Le repli sur une
            // direction par défaut ferait tourner l'animation dans un sens que
            // personne n'a demandé, et sans rien dire.
            let direction = Direction::ALL.get(self.direction)?;
            reglages.push(("direction".to_owned(), direction.slug().to_owned()));
        }
        Some(Request::Animate {
            name: Some(nom.clone()),
            reglages,
        })
    }
}

fn hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

/// L'état d'une poignée de ventilateur : consigne contre mesure.
///
/// Un état **par canal**, jamais partagé : deux ventilateurs se règlent
/// indépendamment, et un état global ferait sauter la poignée de l'un quand on
/// tire celle de l'autre.
#[derive(Debug, Clone, PartialEq)]
pub struct Poignee {
    /// La dernière mesure venue du démon.
    ///
    /// Zéro avant la première : la fenêtre n'a alors rien d'autre à montrer, et
    /// la première réponse à `status` arrive dans la seconde.
    mesure: u8,
    /// La consigne posée par l'utilisateur, tant qu'elle a la main.
    consigne: Option<u8>,
    /// Vrai tant que la poignée est tenue.
    tenue: bool,
    /// Quand elle a été relâchée, pour compter la grâce **depuis le
    /// relâchement** et non depuis la saisie : un geste long ne raccourcit pas
    /// le délai qu'il faut au ventilateur pour rejoindre sa consigne.
    relachee: Option<Duration>,
    /// La consigne pas encore envoyée au démon.
    en_attente: Option<u8>,
}

impl Poignee {
    /// Une poignée qui n'a encore rien vu : elle suit la mesure.
    pub fn nouvelle() -> Poignee {
        Poignee {
            mesure: 0,
            consigne: None,
            tenue: false,
            relachee: None,
            en_attente: None,
        }
    }

    /// L'utilisateur tire la poignée à cette valeur.
    ///
    /// Reposer la poignée là où elle est déjà **n'est pas un pas** : sans cette
    /// règle, une interface qui appelle `saisir` à chaque image respecterait la
    /// lettre de « au plus une commande par pas » tout en inondant le démon.
    pub fn saisir(&mut self, consigne: u8, _maintenant: Duration) {
        if self.consigne != Some(consigne) {
            self.consigne = Some(consigne);
            self.en_attente = Some(consigne);
        }
        self.tenue = true;
        self.relachee = None;
    }

    /// Il la lâche. La consigne tient encore [`GRACE`].
    pub fn relacher(&mut self, maintenant: Duration) {
        self.tenue = false;
        self.relachee = Some(maintenant);
    }

    /// Une mesure arrive du démon.
    ///
    /// C'est le seul endroit où la grâce peut expirer, et elle est comptée
    /// depuis le relâchement : une mesure ne relance pas le compte, sinon la
    /// télémétrie qui arrive chaque seconde le relancerait indéfiniment.
    pub fn mesurer(&mut self, pwm: u8, maintenant: Duration) {
        self.mesure = pwm;
        if self.tenue {
            return;
        }
        // À `relâchement + GRACE` pile, la consigne a **fini** de tenir.
        if let Some(depuis) = self.relachee
            && maintenant.saturating_sub(depuis) >= GRACE
        {
            self.consigne = None;
            self.relachee = None;
        }
    }

    /// Le canal repasse à la courbe du firmware : la mesure reprend la main
    /// tout de suite, sans attendre la fin de la grâce.
    ///
    /// Une consigne encore en attente est **annulée** : partie après le
    /// « fan auto », elle ressortirait le canal de l'automatique qu'on vient de
    /// demander.
    pub fn liberer(&mut self) {
        self.consigne = None;
        self.relachee = None;
        self.tenue = false;
        self.en_attente = None;
    }

    /// Ce que le curseur doit afficher.
    pub fn affichee(&self) -> u8 {
        self.consigne.unwrap_or(self.mesure)
    }

    /// La consigne à envoyer au démon, une seule fois par valeur.
    ///
    /// Rend `Some` au premier appel qui suit un changement de consigne, `None`
    /// ensuite : un glissement continu produit une commande par pas franchi,
    /// pas une par image.
    pub fn a_envoyer(&mut self) -> Option<u8> {
        self.en_attente.take()
    }
}
