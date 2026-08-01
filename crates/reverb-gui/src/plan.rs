//! Le boîtier vu depuis le panneau latéral gauche, face à la carte mère.
//!
//! C'est le point de vue de l'utilisateur : celui depuis lequel il « lit » ses
//! ventilateurs, et celui dans lequel la géométrie a été relevée (`docs/
//! GEOMETRIE.md`). L'**arrière du boîtier est donc à gauche** de l'écran,
//! l'avant à droite, le haut en haut.
//!
//! # C'est un schéma, pas une photographie
//!
//! Vus depuis ce panneau, seuls les trois ventilateurs du radiateur présentent
//! leur disque : les six couchés et celui de l'arrière sont vus **par la
//! tranche**. Un dessin fidèle en ferait sept traits, où aucune LED ne serait
//! cliquable. Ils sont donc dessinés **en cercles quand même**, à leur place
//! dans ce repère. Une vue dont le but est de ne plus regarder sous le bureau
//! doit montrer les cent vingt-quatre LED, pas sept traits.
//!
//! Deuxième écart assumé, pour la même raison : les barrettes et la colonne du
//! radiateur sont à des **profondeurs différentes** qu'une vue de face
//! confondrait — la RAM se dresse devant la carte mère, le radiateur est plaqué
//! contre elle. Physiquement, la RAM masque une partie du radiateur ; sur le
//! plan, elle est décalée pour que les deux restent cliquables.

use reverb_anim::Geometrie;
use reverb_proto::Position;

/// Un point de la maquette, en coordonnées d'écran.
///
/// Normalisées de 0 à 1, `y` vers le bas — la convention de l'écran, pas celle
/// du boîtier. C'est la fenêtre qui les multiplie par sa taille du moment : un
/// plan en pixels serait faux dès qu'on redimensionne.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub x: f32,
    pub y: f32,
}

/// Ce qu'on désigne en cliquant sur la maquette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cible {
    Led { position: Position, led: usize },
    Barrette { slot: usize, led: usize },
}

/// Où dessiner chaque LED du boîtier.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    _reserve: (),
}

impl Plan {
    /// Projette une géométrie dans le repère « panneau gauche ».
    ///
    /// La géométrie donne des points en trois dimensions ; le plan n'en garde
    /// que deux. La profondeur — l'axe qui va du panneau vers la carte mère —
    /// n'est pas dessinée : elle ne distingue rien que l'utilisateur cherche.
    pub fn nouveau(geometrie: &Geometrie) -> Plan {
        let _ = geometrie;
        todo!("#23")
    }

    /// Le centre d'un ventilateur.
    pub fn centre_ventilateur(&self, position: Position) -> Place {
        let _ = position;
        todo!("#23")
    }

    /// Une des huit LED d'un ventilateur, `None` au-delà.
    pub fn led_ventilateur(&self, position: Position, led: usize) -> Option<Place> {
        let _ = (position, led);
        todo!("#23")
    }

    /// Une des onze LED d'une barrette, `None` au-delà.
    pub fn led_barrette(&self, slot: usize, led: usize) -> Option<Place> {
        let _ = (slot, led);
        todo!("#23")
    }

    /// Le rayon d'un anneau de ventilateur, dans la même unité que [`Place`].
    ///
    /// Deux LED voisines doivent rester distinctes à l'écran : c'est ce rayon
    /// qui décide si on peut en cliquer une.
    pub fn rayon_anneau(&self) -> f32 {
        todo!("#23")
    }

    /// Ce qui se trouve sous un point, s'il y a quelque chose.
    ///
    /// C'est la fonction du clic. Elle rend la LED **la plus proche** dans son
    /// rayon de prise, et `None` au-delà : cliquer dans le vide ne doit pas
    /// changer une couleur au hasard.
    pub fn sous(&self, place: Place) -> Option<Cible> {
        let _ = place;
        todo!("#23")
    }
}
