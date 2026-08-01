//! `reverb-gui` — la fenêtre, cliente du démon.
//!
//! Elle n'ouvre **aucun** périphérique et n'écrit **aucun** fichier : tout ce
//! qu'elle montre et tout ce qu'elle règle passe par le socket Unix du démon,
//! qui reste seul à détenir les bus (ADR-002). Le socket est ainsi l'unique
//! franchissement de privilège, au lieu d'un second mécanisme de droits à
//! entretenir.
//!
//! À ce commit, elle ne fait rien d'autre que s'ouvrir.

use std::process::ExitCode;

slint::include_modules!();

fn main() -> ExitCode {
    let fenetre = match Fenetre::new() {
        Ok(fenetre) => fenetre,
        Err(erreur) => {
            // Le cas qui justifie ce commit : pas de rendu accéléré possible.
            // Le dire, et dire par quoi s'en sortir, plutôt que de quitter sur
            // un code d'erreur muet.
            eprintln!("erreur : impossible d'ouvrir la fenêtre : {erreur}");
            eprintln!("         essai possible : SLINT_BACKEND=winit-software reverb-gui");
            return ExitCode::FAILURE;
        }
    };

    if let Err(erreur) = fenetre.run() {
        eprintln!("erreur : {erreur}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
