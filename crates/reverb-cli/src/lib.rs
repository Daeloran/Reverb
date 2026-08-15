//! Bibliothèque du binaire `reverb` : analyse des arguments, et les verdicts
//! qui précèdent une écriture.
//!
//! Les quatre modules d'entrée/sortie vivaient ici jusqu'à l'issue #17 ; ils
//! sont maintenant dans `reverb-hw`, que le démon partage.

pub mod cli;
pub mod consigne;

pub use consigne::refus_de_consigne;
