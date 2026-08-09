//! La mire qui mesure le disque visible — tests de logique (issue #77, étape 1).
//!
//! Elle n'a qu'un travail : porter une réponse **lisible à l'œil** sur un
//! boîtier. Ce que ces tests vérifient, c'est qu'elle la porte sans ambiguïté —
//! pas qu'elle est jolie, ce qui ne se teste pas.
//!
//! ⚠️ **Le résultat de cette mire est une mesure, pas un affichage.** Si ses
//! bandes se confondaient ou si ses rayons ne correspondaient pas à ce que la
//! ligne de commande imprime, on inscrirait dans la spec un rayon faux — et
//! toute la géométrie de #80 et #90 en dépend.

use reverb_proto::screen::{
    self, MIRE_BANDE, MIRE_BANDES, MIRE_COINS, MIRE_RAYON_MINIMAL, WIDTH, composantes, mire_cercle,
    mire_rayon,
};

/// La couleur du pixel `(x, y)` de la mire.
fn couleur(image: &[u8], x: usize, y: usize) -> (u8, u8, u8) {
    let debut = (y * usize::from(WIDTH) + x) * screen::PIXEL_LEN;
    composantes(&image[debut..debut + screen::PIXEL_LEN])
}

/// Le pixel à ce rayon, sur la diagonale montante — loin de la croix centrale
/// et des quatre axes, donc jamais sur un cas particulier.
fn sur_la_diagonale(image: &[u8], rayon: f32) -> (u8, u8, u8) {
    let centre = f32::from(WIDTH) / 2.0;
    let pas = std::f32::consts::FRAC_1_SQRT_2;
    let x = (centre + rayon * pas) as usize;
    let y = (centre - rayon * pas) as usize;
    couleur(image, x, y)
}

#[test]
fn la_mire_a_la_taille_d_une_image_de_dalle() {
    // Une mire tronquée serait ignorée par le contrôleur sans le moindre code
    // d'erreur (spec §2.2.1) : on regarderait un écran noir en croyant mesurer.
    assert_eq!(mire_cercle().len(), screen::IMAGE_LEN);
}

#[test]
fn chaque_bande_porte_sa_couleur_a_son_rayon() {
    // Le cœur de la mesure : ce que la ligne de commande imprime doit être ce
    // que l'œil voit. Un décalage d'une bande, et le rayon inscrit dans la spec
    // serait faux de huit pixels — sans que rien ne le signale.
    let image = mire_cercle();
    for (rang, (nom, attendue)) in MIRE_BANDES.iter().enumerate() {
        // Le milieu de la bande, pour ne pas mesurer sur une frontière.
        let rayon = f32::from(mire_rayon(rang)) - f32::from(MIRE_BANDE) / 2.0;
        assert_eq!(
            sur_la_diagonale(&image, rayon),
            *attendue,
            "la bande « {nom} » doit occuper le rayon {rayon}, c'est ce que la légende annonce"
        );
    }
}

#[test]
fn les_neuf_bandes_portent_neuf_couleurs_et_neuf_noms_distincts() {
    // Deux bandes de même couleur rendraient la réponse ambiguë : « je vois
    // jusqu'au vert » ne désignerait plus un rayon.
    let mut couleurs = std::collections::HashSet::new();
    let mut noms = std::collections::HashSet::new();
    for (nom, couleur) in MIRE_BANDES {
        assert!(
            couleurs.insert(couleur),
            "la couleur de « {nom} » est déjà prise"
        );
        assert!(noms.insert(nom), "le nom « {nom} » est déjà pris");
    }
    assert!(
        !couleurs.contains(&MIRE_COINS),
        "les coins doivent se distinguer des neuf bandes : c'est eux qui diraient que la dalle \
         n'est pas ronde"
    );
}

#[test]
fn le_coeur_reste_noir_et_les_coins_portent_leur_couleur() {
    // Le cœur noir concentre le regard sur le bord, qui est le seul endroit
    // qu'on mesure. Les coins, eux, sont la question de contrôle : s'ils se
    // voient, la dalle n'est pas ronde et toute la géométrie est à refaire.
    let image = mire_cercle();
    assert_eq!(
        sur_la_diagonale(&image, f32::from(MIRE_RAYON_MINIMAL) - 20.0),
        (0, 0, 0),
        "le cœur de la mire est noir"
    );
    assert_eq!(couleur(&image, 0, 0), MIRE_COINS, "le coin haut-gauche");
    assert_eq!(
        couleur(&image, usize::from(WIDTH) - 1, usize::from(WIDTH) - 1),
        MIRE_COINS,
        "le coin bas-droite"
    );
}

#[test]
fn la_croix_centrale_est_blanche_et_ne_deborde_pas_sur_les_bandes() {
    // Elle dit si l'image est centrée sur la dalle, ce qu'aucun anneau ne
    // montrerait — un anneau décentré reste un anneau.
    let image = mire_cercle();
    let milieu = usize::from(WIDTH) / 2;
    assert_eq!(
        couleur(&image, milieu, milieu),
        (255, 255, 255),
        "le centre"
    );
    assert_eq!(
        couleur(&image, milieu + 40, milieu),
        (255, 255, 255),
        "le bras droit de la croix"
    );
    assert_eq!(
        couleur(&image, milieu + 60, milieu),
        (0, 0, 0),
        "au-delà de la croix, le cœur reste noir"
    );
}

#[test]
fn la_derniere_bande_s_arrete_ou_le_disque_inscrit_commence() {
    // La mire doit couvrir jusqu'au disque inscrit et pas au-delà : une bande
    // qui déborderait dans les coins ne se verrait de toute façon jamais, et
    // fausserait la lecture de celui qui compte les anneaux.
    assert_eq!(
        mire_rayon(MIRE_BANDES.len() - 1),
        screen::VISIBLE_DISC_RADIUS,
        "les neuf bandes doivent finir exactement sur le rayon supposé"
    );
    assert_eq!(
        MIRE_RAYON_MINIMAL + MIRE_BANDE * MIRE_BANDES.len() as u16,
        screen::VISIBLE_DISC_RADIUS
    );
}
