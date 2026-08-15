//! `reverb-anim` ne laisse aucun descripteur ouvert derrière lui.
//!
//! Critère d'acceptation de l'issue #19 — « les animations restent calculables hors du démon : un
//! test rend une image sans ouvrir un seul périphérique ». C'est ce qui rend vraie la promesse
//! « l'aperçu montre ce que le boîtier reçoit » : la fenêtre affiche les images exactement
//! calculées par le code qui écrit sur le bus.
//!
//! # Pourquoi ce fichier existe à part (issue #55)
//!
//! La mesure vivait dans `spec_animations.rs` et **échouait au hasard**. Elle compte
//! `/proc/self/fd`, qui est global au **processus**, alors que les trente tests de ce binaire y
//! tournent en parallèle : tout descripteur ouvert par un autre test — ou par le harnais — entre
//! les deux relevés comptait comme une ouverture faite par `Animation::image`. Le test avait
//! raison sur le fond et tort sur la façon de mesurer, exactement comme #43.
//!
//! ⚠️ **Rien dans ce binaire ne doit ouvrir de fichier à l'exécution.** C'est la condition qui
//! rend la mesure valable, et elle est fragile : ajouter ici un test qui lit un fichier
//! ressusciterait la course sans que personne ne le voie. Le second test la vérifie plutôt que de
//! la demander en commentaire — un commentaire ne se lit pas au moment où l'on ajoute un test. Il
//! le fait par `include_str!`, qui embarque le fichier **à la compilation** et n'ouvre donc rien
//! quand il tourne.
//!
//! L'observable complémentaire — les sources du crate ne nomment aucune porte de sortie — vit
//! dans `spec_animations.rs` : il lit le disque, il n'a donc pas sa place ici.

use reverb_anim::{Animation, CATALOGUE, Direction, Geometrie, Reglages};
use reverb_proto::Rgb;

/// Le nombre de descripteurs de fichier ouverts par le processus.
///
/// Rend 0 si `/proc` n'est pas monté — les deux mesures valent alors 0, et le test ne dit rien
/// plutôt que de mentir.
fn descripteurs_ouverts() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .map(std::iter::Iterator::count)
        .unwrap_or(0)
}

#[test]
fn rendre_toutes_les_images_du_catalogue_n_ouvre_aucun_descripteur() {
    let geometrie = Geometrie::mesuree();
    let reglages = Reglages {
        couleur: Rgb::new(0x20, 0xc0, 0xff),
        vitesse: 3,
        direction: Direction::BasHaut,
        // Champ ajouté par #75, sans effet sur ce que ce test observe : cette
        // suite parcourt tout le catalogue, donc `thermique`, qui rendra son
        // blanc pulsant faute de sonde — et un blanc pulsant n'ouvre pas plus de
        // descripteur qu'un gradient.
        sonde: None,
        // Champ ajouté par #126. `None` est le comportement d'avant à l'octet
        // près — un test d'intention de #126 le fige —, donc ce témoin observe
        // exactement ce qu'il observait.
        palette: None,
    };

    // Mesure à blanc d'abord : `read_dir` ouvre lui-même un descripteur, et c'est la **variation**
    // qui est observée, pas le nombre absolu.
    let _ = descripteurs_ouverts();
    let avant = descripteurs_ouverts();

    let mut rendues = 0usize;
    for nom in CATALOGUE {
        let animation = Animation::par_nom(nom).expect("le catalogue se nomme lui-même");
        for pas in [0u32, 1, 7, 30, 900] {
            let image = animation.image(&geometrie, &reglages, pas);
            // L'image est consommée, et pas seulement calculée : une optimisation qui l'élirait
            // rendrait ce test vide sans rien dire.
            assert_eq!(image.ventilateurs.len(), 10, "« {nom} » au pas {pas}");
            rendues += 1;
        }
    }

    let apres = descripteurs_ouverts();
    assert_eq!(
        avant,
        apres,
        "rendre {rendues} images a ouvert {} descripteur(s) : le crate n'est plus pur",
        apres.saturating_sub(avant)
    );
}

#[test]
fn ce_binaire_n_ouvre_de_fichier_que_pour_mesurer() {
    // ⚠️ Le garde-fou de tout le fichier, et la raison pour laquelle #55 ne se contente pas d'un
    // déménagement. Le décompte de descripteurs n'est juste que dans un binaire où rien d'autre
    // n'ouvre de fichier pendant qu'il mesure : un test ajouté ici qui lirait le disque
    // rétablirait la course d'origine, et l'échec reviendrait une fois sur deux, sous charge, des
    // mois plus tard.
    //
    // `include_str!` embarque le fichier **à la compilation** : ce test ne s'observe donc pas
    // lui-même en train d'ouvrir quoi que ce soit.
    let fichier = include_str!("spec_purete.rs");
    let lignes_de_code = fichier
        .lines()
        .map(str::trim_start)
        .filter(|ligne| !ligne.starts_with("//"));

    // ⚠️ Les motifs sont **recollés** à la compilation, et ce n'est pas de la coquetterie : écrits
    // d'un seul tenant, ils apparaîtraient tels quels dans la ligne qui les déclare, et ce test se
    // compterait lui-même comme une ouverture de fichier.
    let portes = [
        concat!("std::", "fs::"),
        concat!("File", "::open"),
        concat!("read_to", "_string"),
        concat!("read_", "dir"),
    ];
    let ouvertures: Vec<&str> = lignes_de_code
        .filter(|ligne| portes.iter().any(|porte| ligne.contains(porte)))
        .collect();

    assert_eq!(
        ouvertures.len(),
        1,
        "ce binaire ouvre {} fichier(s) : seule la mesure de « /proc/self/fd » a le droit de le \
         faire, sans quoi le décompte redevient une course — {ouvertures:?}",
        ouvertures.len()
    );
    assert!(
        ouvertures[0].contains("/proc/self/fd"),
        "la seule ouverture de ce binaire doit être celle qui mesure : « {} »",
        ouvertures[0]
    );
}
