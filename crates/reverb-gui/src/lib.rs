//! Ce que la fenêtre calcule, et qui n'a pas besoin d'un écran pour être vrai.
//!
//! Le binaire, lui, ne fait qu'ouvrir la fenêtre et parler au socket. Tout ce
//! qui se vérifie sans afficher quoi que ce soit vit ici — à commencer par la
//! projection du boîtier, qui décide où chacune des 124 LED se dessine.

pub mod plan;
