//! La géométrie sur disque.
//!
//! **La fenêtre n'écrit aucun fichier.** Elle demande par le socket, et c'est
//! le démon — qui est root — qui écrit. Le socket reste ainsi l'unique
//! franchissement de privilège, au lieu d'un second mécanisme de droits à
//! entretenir à côté.
//!
//! Le format est maison, une ligne par ventilateur : `serde` serait la plus
//! grosse dépendance du projet pour vingt nombres, et le protocole IPC a déjà
//! prouvé qu'un format texte tenu à la main suffit ici.

use std::fs;
use std::io;
use std::path::Path;

use reverb_anim::Geometrie;

/// Où la géométrie est conservée entre deux démarrages.
pub const CHEMIN: &str = "/etc/reverb/geometrie.conf";

/// En-tête du fichier, pour qui l'ouvre sans savoir ce que c'est.
const EN_TETE: &str = "\
# Orientation des ventilateurs — Reverb (issue #19)
#
# Une ligne par ventilateur : <position> <angle> <sens>
#   angle : degrés de la LED 1, 0 = midi, de 0 a 359
#   sens  : horaire ou antihoraire, vu de l'exterieur du boitier
#
# Ce fichier est ecrit par reverbd. Il se modifie aussi par le socket :
#   geometry <position> angle=<0-359> sens=<horaire|antihoraire>
";

/// Lit la géométrie, ou rend celle mesurée en usine.
///
/// Ne refuse **jamais** de rendre une géométrie : un démon qui ne démarrerait
/// pas parce qu'un fichier de configuration est de travers laisserait la
/// machine sans éclairage du tout. Ce qui cloche est signalé et rendu à
/// l'appelant, qui le journalise avec les autres soucis d'ouverture.
pub fn charger(chemin: &Path) -> (Geometrie, Option<String>) {
    let texte = match fs::read_to_string(chemin) {
        Ok(texte) => texte,
        // L'absence n'est pas une anomalie : c'est le premier démarrage.
        Err(erreur) if erreur.kind() == io::ErrorKind::NotFound => {
            return (Geometrie::mesuree(), None);
        }
        Err(erreur) => {
            return (
                Geometrie::mesuree(),
                Some(format!(
                    "géométrie illisible dans {} ({erreur}) : orientation d'usine appliquée",
                    chemin.display()
                )),
            );
        }
    };

    match Geometrie::decoder(&texte) {
        Ok(geometrie) => (geometrie, None),
        Err(erreur) => (
            Geometrie::mesuree(),
            Some(format!(
                "géométrie invalide dans {} ({erreur}) : orientation d'usine appliquée",
                chemin.display()
            )),
        ),
    }
}

/// Écrit la géométrie, en une fois.
///
/// Par un fichier temporaire puis un renommage : une écriture directe qu'une
/// coupure interromprait laisserait un fichier tronqué, que le démarrage
/// suivant refuserait — et l'utilisateur perdrait une géométrie qu'il avait
/// pourtant réglée.
pub fn enregistrer(chemin: &Path, geometrie: &Geometrie) -> io::Result<()> {
    if let Some(dossier) = chemin.parent() {
        fs::create_dir_all(dossier)?;
    }
    let provisoire = chemin.with_extension("conf.nouveau");
    fs::write(&provisoire, format!("{EN_TETE}\n{}\n", geometrie.encoder()))?;
    fs::rename(&provisoire, chemin)
}
