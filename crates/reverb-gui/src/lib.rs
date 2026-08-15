//! Ce que la fenêtre calcule, et qui n'a pas besoin d'un écran pour être vrai.
//!
//! Le binaire, lui, ne fait qu'ouvrir la fenêtre et parler au socket. Tout ce
//! qui se vérifie sans afficher quoi que ce soit vit ici — à commencer par la
//! projection du boîtier, qui décide où chacune des 124 LED se dessine.
//!
//! L'interface elle-même y est aussi, générée depuis `ui/fenetre.slint` : c'est
//! ce qui permet de la **dessiner sans écran** (`examples/apercu.rs`), et donc
//! de regarder la maquette autrement qu'en ouvrant une fenêtre sur un bureau.

pub mod client;
pub mod plan;
pub mod reglages;
pub mod sondes;
pub mod telemetrie;

slint::include_modules!();
