//! Démon Reverb — le processus qui détient les bus et garde ses descripteurs (issue #17).
//!
//! Ouvrir un `/dev/hidraw*` coûte 51 ms sur SHYNAEL, mesuré le 2026-07-31 ; écrire une trame de
//! 64 octets en coûte 1,3. Tout le prix est dans l'ouverture, et il est linéaire en nombre
//! d'ouvertures. Garder les descripteurs pour toute la vie du processus fait passer une repeinte
//! des dix ventilateurs de 643 ms à ~40 ms — c'est la raison d'être de ce crate, pas un détail
//! d'implémentation.
//!
//! La partie **pure** du démon est sa cadence de rendu : elle ne tient aucune horloge, c'est
//! l'appelant qui lui donne l'instant. C'est ce qui rend le rattrapage et le décrochage testables
//! sans dormir, et c'est la seule chose que `crates/reverb-daemon/tests/` vérifie.
//!
//! Le reste — socket Unix, unité systemd, ouverture des périphériques — est de l'entrée/sortie :
//! ça se vérifie sur la machine, avec les critères d'acceptation de l'issue.

pub mod cadence;
