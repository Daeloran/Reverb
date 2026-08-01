//! Compile l'interface Slint en Rust, à la compilation du crate.
//!
//! Le `.slint` est du texte que `slint-build` transforme en code : la fenêtre
//! n'est pas interprétée au démarrage, et il n'y a donc rien à installer à côté
//! du binaire — c'est ce que l'ADR-001 exige.

fn main() {
    slint_build::compile("ui/fenetre.slint").expect("compilation de l'interface Slint");
}
