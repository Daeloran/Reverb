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

use std::collections::{BTreeMap, VecDeque};

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
///
/// Une file par sonde, jamais partagée : deux sondes qui se partageraient une
/// mémoire se voleraient leurs relevés dès que l'une bat plus vite que l'autre.
#[derive(Debug, Clone, Default)]
pub struct Historique {
    courbes: BTreeMap<String, VecDeque<Releve>>,
}

impl Historique {
    pub fn nouvel() -> Historique {
        Historique::default()
    }

    /// Note un relevé pour cette sonde.
    ///
    /// Au-delà de [`MEMOIRE`], le plus ancien tombe.
    pub fn noter(&mut self, sonde: &str, releve: Releve) {
        let courbe = self.courbes.entry(sonde.to_owned()).or_default();
        courbe.push_back(releve);
        // ⚠️ `Illisible` occupe une place comme les autres. Sans cela, une sonde
        // débranchée garderait indéfiniment ses vieilles valeurs à l'écran, et
        // la courbe dirait qu'on la lit encore.
        while courbe.len() > MEMOIRE {
            courbe.pop_front();
        }
    }

    /// Les relevés d'une sonde, du plus ancien au plus récent.
    ///
    /// Vide si la sonde n'a jamais été vue. **Jamais complétée** : une sonde
    /// apparue il y a trois secondes rend trois relevés, pas cent vingt.
    pub fn courbe(&self, sonde: &str) -> Vec<Releve> {
        self.courbes
            .get(sonde)
            .map(|courbe| courbe.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Les sondes connues, par ordre alphabétique de leur nom.
    pub fn sondes(&self) -> Vec<String> {
        // `BTreeMap` : le tri est celui de la structure, pas un tri de plus à
        // penser à refaire quand une sonde apparaît.
        self.courbes.keys().cloned().collect()
    }

    /// Le dernier relevé d'une sonde, s'il y en a un.
    pub fn dernier(&self, sonde: &str) -> Option<Releve> {
        self.courbes.get(sonde)?.back().copied()
    }

    /// Les bornes d'une courbe : la plus basse et la plus haute valeur lisible.
    ///
    /// `None` quand aucun relevé n'est lisible — il n'y a alors rien à mettre à
    /// l'échelle, et forcer un intervalle inventerait une courbe.
    pub fn bornes(&self, sonde: &str) -> Option<(i32, i32)> {
        let mut lisibles = self
            .courbes
            .get(sonde)?
            .iter()
            .filter_map(|releve| match releve {
                Releve::Valeur(valeur) => Some(*valeur),
                Releve::Illisible => None,
            });
        let premier = lisibles.next()?;
        Some(lisibles.fold((premier, premier), |(bas, haut), valeur| {
            (bas.min(valeur), haut.max(valeur))
        }))
    }
}
