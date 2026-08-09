//! La mire qui mesure le disque visible — tests de logique (issue #77).
//!
//! ⚠️ **Une première version de cette mire a échoué sur le matériel**, et ces
//! tests existent pour que la deuxième ne refasse pas la même faute. Elle posait
//! ses bandes entre 248 et 320, laissant tout le centre noir : essayée sur
//! SHYNAEL le 2026-08-09, elle n'a **rien montré** — du noir rétroéclairé. Une
//! mire qui ne sait mesurer que ce qu'elle présuppose confond son résultat avec
//! une panne.
//!
//! Ce que ces tests verrouillent, c'est donc d'abord : **il y a quelque chose à
//! voir partout dans le disque.**

use std::collections::HashSet;

use reverb_proto::screen::{
    self, MIRE_ANNEAU, MIRE_ANNEAUX, MIRE_COINS, MIRE_PAS, MIRE_REPERE, MIRE_REPERE_TOUS_LES,
    VISIBLE_DISC_RADIUS, WIDTH, composantes, mire_cercle, mire_rayon,
};

fn couleur(image: &[u8], x: usize, y: usize) -> (u8, u8, u8) {
    let debut = (y * usize::from(WIDTH) + x) * screen::PIXEL_LEN;
    composantes(&image[debut..debut + screen::PIXEL_LEN])
}

/// Le pixel à ce rayon sur la diagonale montante — loin des quatre rayons
/// tracés, donc jamais sur un cas particulier.
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
fn il_y_a_quelque_chose_a_voir_a_toutes_les_distances_du_centre() {
    // **Le défaut de la première version, nommément.** Elle ne dessinait rien
    // sous 248 px de rayon ; sur une dalle dont le disque visible est plus
    // petit, elle n'affichait donc rien du tout, et son résultat — « je ne vois
    // rien » — se confondait avec une panne d'affichage.
    //
    // Chaque couronne de vingt pixels doit porter de la matière.
    let image = mire_cercle();
    let mut rayon = 0.0;
    while rayon < f32::from(VISIBLE_DISC_RADIUS) {
        let borne = rayon + f32::from(MIRE_PAS);
        let mut vu = false;
        let mut essai = rayon;
        while essai < borne && !vu {
            vu = sur_la_diagonale(&image, essai) != (0, 0, 0);
            essai += 0.5;
        }
        assert!(
            vu,
            "rien n'est dessiné entre {rayon} et {borne} px du centre : une dalle qui s'arrêterait \
             là n'afficherait rien, et on croirait à une panne"
        );
        rayon = borne;
    }
}

#[test]
fn les_anneaux_tombent_aux_rayons_annonces() {
    // Le cœur de la mesure : ce que la légende annonce doit être ce que la
    // photo montre. Un décalage d'un anneau, et le rayon inscrit dans la spec
    // serait faux de vingt pixels — sans que rien ne le signale.
    let image = mire_cercle();
    for anneau in 0..MIRE_ANNEAUX {
        let rayon = f32::from(mire_rayon(anneau));
        assert_ne!(
            sur_la_diagonale(&image, rayon),
            (0, 0, 0),
            "l'anneau n° {} doit se trouver à {rayon} px",
            anneau + 1
        );
        // Et le noir entre deux anneaux, sans quoi on ne les compterait pas.
        assert_eq!(
            sur_la_diagonale(&image, rayon - f32::from(MIRE_PAS) / 2.0),
            (0, 0, 0),
            "il doit faire noir entre l'anneau n° {} et le précédent",
            anneau + 1
        );
    }
}

#[test]
fn un_anneau_sur_quatre_est_un_repere_rouge_et_plus_epais() {
    // ⚠️ **Compter seize anneaux fins sur une photo est une source d'erreur à
    // elle seule.** Les repères font qu'on compte quatre gros et qu'on ajoute
    // les fins qui restent — ce qui ne se trompe pas.
    let image = mire_cercle();
    let mut reperes = 0;
    for anneau in 0..MIRE_ANNEAUX {
        let rang = anneau + 1;
        let attendue = if rang % MIRE_REPERE_TOUS_LES == 0 {
            reperes += 1;
            MIRE_REPERE
        } else {
            MIRE_ANNEAU
        };
        assert_eq!(
            sur_la_diagonale(&image, f32::from(mire_rayon(anneau))),
            attendue,
            "l'anneau n° {rang}, à {} px",
            mire_rayon(anneau)
        );
    }
    assert!(
        reperes >= 3,
        "il faut au moins trois repères pour que le comptage serve à quelque chose, trouvé \
         {reperes}"
    );
}

#[test]
fn les_quatre_coins_portent_leur_couleur_et_elle_ne_sert_a_rien_d_autre() {
    // Les coins sont la question de contrôle : s'ils se voient, la dalle n'est
    // pas ronde et toute la géométrie de #80 est à refaire. Leur couleur ne doit
    // donc apparaître nulle part ailleurs.
    let image = mire_cercle();
    assert_eq!(couleur(&image, 0, 0), MIRE_COINS, "le coin haut-gauche");
    assert_eq!(
        couleur(&image, usize::from(WIDTH) - 1, usize::from(WIDTH) - 1),
        MIRE_COINS,
        "le coin bas-droite"
    );

    let distinctes: HashSet<(u8, u8, u8)> = [MIRE_ANNEAU, MIRE_REPERE, MIRE_COINS, (0, 0, 0)]
        .into_iter()
        .collect();
    assert_eq!(
        distinctes.len(),
        4,
        "les quatre couleurs de la mire doivent se distinguer : le fond, l'anneau, le repère et \
         les coins"
    );
}

#[test]
fn les_quatre_rayons_traces_donnent_le_centre_et_l_orientation() {
    // Ils disent d'un coup d'œil où est le centre de l'image sur la dalle. Un
    // anneau décentré reste un anneau : sans eux, un décalage passerait.
    let image = mire_cercle();
    let milieu = usize::from(WIDTH) / 2;
    assert_eq!(couleur(&image, milieu, milieu), MIRE_ANNEAU, "le mille");
    for (dx, dy, quoi) in [
        (100_isize, 0_isize, "le rayon droit"),
        (-100, 0, "le rayon gauche"),
        (0, 100, "le rayon bas"),
        (0, -100, "le rayon haut"),
    ] {
        let x = milieu.wrapping_add_signed(dx);
        let y = milieu.wrapping_add_signed(dy);
        assert_eq!(couleur(&image, x, y), MIRE_ANNEAU, "{quoi}");
    }
}

#[test]
fn le_dernier_anneau_tombe_sur_le_disque_suppose() {
    // La mire doit couvrir jusqu'au disque inscrit et pas au-delà : un anneau
    // qui déborderait dans les coins ne se verrait de toute façon jamais, et
    // fausserait le comptage.
    assert_eq!(
        mire_rayon(MIRE_ANNEAUX - 1),
        VISIBLE_DISC_RADIUS,
        "le dernier anneau doit tomber exactement sur le rayon supposé"
    );
}
