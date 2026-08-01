//! L'historique des sondes, pour les tracer.
//!
//! Il vit **dans la fenêtre** et ne survit pas à sa fermeture. Persister deux
//! minutes de sparkline coûterait un format de fichier et un rythme d'écriture
//! sur disque pour une donnée dont la valeur tombe à zéro dès qu'on ferme.
//!
//! # Ce qu'une courbe ne doit pas faire dire
//!
//! Deux pièges, et les deux mentent de la même façon — en montrant une mesure
//! qui n'a pas eu lieu :
//!
//! - une sonde **qui vient d'apparaître** ne doit pas traîner derrière elle une
//!   ligne à zéro qui ferait croire à une chute ;
//! - une sonde **qui disparaît** — un périphérique débranché — ne doit pas figer
//!   sa dernière valeur et laisser croire qu'elle est encore lue.

use std::collections::HashMap;

/// Combien de relevés une courbe garde.
///
/// Deux minutes au pas d'une seconde, qui est celui auquel la fenêtre interroge
/// le démon. Plus long ne se lirait plus dans la largeur d'une carte.
pub const MEMOIRE: usize = 120;

/// Un relevé : une valeur, ou l'aveu qu'on n'a pas pu lire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Releve {
    /// En millidegrés, ou en tours par minute selon la sonde.
    Valeur(i32),
    Illisible,
}

/// Ce qu'on garde de chaque sonde.
#[derive(Debug, Clone, Default)]
pub struct Historique {
    _prive: (),
}

impl Historique {
    pub fn nouvel() -> Historique {
        todo!("issue #31")
    }

    /// Note un relevé pour cette sonde.
    ///
    /// Au-delà de [`MEMOIRE`], le plus ancien tombe.
    pub fn noter(&mut self, _sonde: &str, _releve: Releve) {
        todo!("issue #31")
    }

    /// Les relevés d'une sonde, du plus ancien au plus récent.
    ///
    /// Vide si la sonde n'a jamais été vue. **Jamais complétée** : une sonde
    /// apparue il y a trois secondes rend trois relevés, pas cent vingt.
    pub fn courbe(&self, _sonde: &str) -> Vec<Releve> {
        todo!("issue #31")
    }

    /// Les sondes connues, par ordre alphabétique de leur nom.
    pub fn sondes(&self) -> Vec<String> {
        todo!("issue #31")
    }

    /// Le dernier relevé d'une sonde, s'il y en a un.
    pub fn dernier(&self, _sonde: &str) -> Option<Releve> {
        todo!("issue #31")
    }

    /// Les bornes d'une courbe : la plus basse et la plus haute valeur lisible.
    ///
    /// `None` quand aucun relevé n'est lisible — il n'y a alors rien à mettre à
    /// l'échelle, et forcer un intervalle inventerait une courbe.
    pub fn bornes(&self, _sonde: &str) -> Option<(i32, i32)> {
        todo!("issue #31")
    }
}

// Le champ privé n'existe que pour empêcher la construction littérale ; ce `use`
// le garde honnête sans exposer la structure interne choisie.
const _: fn() -> HashMap<String, ()> = HashMap::new;
