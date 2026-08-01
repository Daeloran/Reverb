//! Tests d'intention de l'historique des sondes (issue #31).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #31 et le contrat déposé
//! dans `crates/reverb-gui/src/sondes.rs` — dont tous les corps sont
//! `todo!("issue #31")` à l'écriture de ce fichier. Rien n'est relu du corps des
//! fonctions déjà écrites, ici ou ailleurs.
//!
//! Ces tests encodent ce que la fenêtre doit dire, pas ce que le code fait. Si
//! l'un d'eux échoue après implémentation, c'est le code qu'on corrige, jamais
//! le test.
//!
//! # Ce qu'une courbe ne doit pas faire dire
//!
//! Tout ce fichier tourne autour d'une seule idée, celle que le contrat pose en
//! tête de `sondes.rs` : **une courbe ne montre que des mesures qui ont eu
//! lieu.** Les trois façons de mentir sont ici, et aucune ne produit d'erreur —
//! elles produisent un joli graphe faux :
//!
//! 1. **Compléter le passé.** Une sonde apparue il y a trois secondes dont la
//!    courbe part de zéro raconte une chute de 40 °C qui n'a pas eu lieu (issue
//!    #31, critère — « Une sonde qui vient d'apparaître trace une courbe
//!    partielle, pas une ligne à zéro »).
//! 2. **Figer le présent.** Une sonde débranchée dont la dernière valeur reste
//!    affichée raconte une machine qu'on croit surveillée alors qu'on ne lit
//!    plus rien (critère — « Une sonde qui disparaît en cours de route [...] le
//!    dit au lieu de figer sa dernière valeur »).
//! 3. **Compter l'illisible comme un zéro.** `Releve::Illisible` traité comme
//!    `Valeur(0)` est le même mensonge que le premier, en plein milieu de la
//!    courbe — et il écrase la mise à l'échelle par la même occasion.
//!
//! # Ce que ces tests ne vérifient pas
//!
//! - **Le tracé.** L'épaisseur du trait, la couleur, la disposition des cartes
//!   se regardent ; l'issue le dit elle-même en parlant de « l'esprit de ce
//!   qu'il a montré ». Ce qui est vérifiable sans écran, c'est ce que la fenêtre
//!   **garde** et ce qu'elle en calcule.
//! - **Le pas d'échantillonnage.** « La fenêtre demande `status` chaque
//!   seconde » (issue #31, approche technique) : c'est la boucle de la fenêtre
//!   qui appelle `noter`, pas `Historique` qui prend le temps. Aucune horloge
//!   n'entre dans ce contrat, et aucun de ces tests ne dort.
//! - **La provenance des relevés.** Que la sonde vienne de `hwmon`, de
//!   `nvidia-smi` ou d'un tachymètre ne change rien ici : l'historique ne
//!   connaît qu'un nom et une suite de relevés. Les tours par minute d'un canal
//!   y ont leur place « au même titre qu'une température » (issue #31), et c'est
//!   précisément parce que ce type ne fait aucune hypothèse sur l'unité.
//!
//! # Arbitrage de contrat
//!
//! **`bornes()` sur un seul relevé rend `(v, v)`.** Le contrat dit « la plus
//! basse et la plus haute valeur lisible » : sur une seule valeur, les deux sont
//! cette valeur. Élargir l'intervalle — `(v-1, v+1)`, ou un pourcentage —
//! reviendrait à faire dire à `bornes` autre chose que ce qu'il annonce, et
//! surtout à inventer une amplitude que la donnée n'a pas. La courbe qui ne peut
//! pas diviser par une amplitude nulle a besoin d'une marge : c'est une décision
//! de **tracé**, elle se prend là où on dessine, avec la hauteur de la carte
//! sous les yeux. Ce que ces tests exigent de `bornes`, c'est un intervalle
//! valide — `basse <= haute` — et jamais une borne qu'aucun relevé n'a produite.

use reverb_gui::sondes::{Historique, MEMOIRE, Releve};

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

/// Une sonde de l'issue : le CPU, qui n'était pas affiché avant #31.
const CPU: &str = "k10temp:tctl";
/// Une seconde sonde, pour vérifier que rien ne déborde de l'une à l'autre.
const COOLANT: &str = "kraken2023elite:coolant-temp";

/// Le `i`-ième relevé d'une suite, en millidegrés.
///
/// Une base plausible et un pas de un : deux relevés ne sont jamais égaux, donc
/// un décalage d'un cran dans l'anneau se voit. Ce ne sont pas des températures
/// attendues du matériel, seulement des jalons discernables.
fn releve(i: usize) -> Releve {
    Releve::Valeur(30_000 + i as i32)
}

/// Les relevés `debut..fin` de cette suite, du plus ancien au plus récent.
fn suite(debut: usize, fin: usize) -> Vec<Releve> {
    (debut..fin).map(releve).collect()
}

/// Un historique où `combien` relevés de la suite ont été notés pour `sonde`.
fn historique_de(sonde: &str, combien: usize) -> Historique {
    let mut historique = Historique::nouvel();
    for i in 0..combien {
        historique.noter(sonde, releve(i));
    }
    historique
}

/// La valeur d'un relevé, ou un échec explicite.
fn valeur(releve: Releve) -> i32 {
    match releve {
        Releve::Valeur(v) => v,
        Releve::Illisible => panic!("relevé lisible attendu, obtenu Illisible"),
    }
}

// ---------------------------------------------------------------------------

mod anneau {
    use super::{CPU, Historique, MEMOIRE, Releve, historique_de, releve, suite};

    #[test]
    fn la_memoire_couvre_deux_minutes_au_pas_d_une_seconde() {
        // issue #31, critère d'acceptation — « La courbe couvre deux minutes
        // glissantes » ; approche technique — « un anneau de 120 relevés par
        // sonde », « la fenêtre demande `status` chaque seconde ».
        // Deux minutes à un relevé par seconde font 120 relevés : c'est la seule
        // valeur de `MEMOIRE` qui tient la promesse affichée à l'utilisateur.
        const SECONDES_PAR_MINUTE: usize = 60;
        assert_eq!(MEMOIRE, 2 * SECONDES_PAR_MINUTE);
    }

    #[test]
    fn l_anneau_garde_exactement_les_derniers_releves_dans_l_ordre() {
        // issue #31, test d'intention n° 6 — « L'anneau d'historique garde
        // exactement les 120 derniers relevés, dans l'ordre » ; contrat —
        // `noter`, « Au-delà de `MEMOIRE`, le plus ancien tombe », et `courbe`,
        // « du plus ancien au plus récent ».
        // Ce qui tombe est le **plus ancien** : garder les premiers arrivés
        // figerait la courbe sur les deux premières minutes de la fenêtre, et
        // rien ne le dirait — la courbe resterait pleine et lisse.
        const SURPLUS: usize = 5;
        let historique = historique_de(CPU, MEMOIRE + SURPLUS);
        let courbe = historique.courbe(CPU);

        assert_eq!(
            courbe.len(),
            MEMOIRE,
            "la courbe ne garde que MEMOIRE relevés"
        );
        assert_eq!(
            courbe,
            suite(SURPLUS, MEMOIRE + SURPLUS),
            "les {SURPLUS} plus anciens doivent être tombés, dans cet ordre"
        );
        assert_eq!(courbe.first().copied(), Some(releve(SURPLUS)));
        assert_eq!(courbe.last().copied(), Some(releve(MEMOIRE + SURPLUS - 1)));
    }

    #[test]
    fn rien_ne_tombe_avant_que_la_memoire_soit_pleine_et_un_seul_ensuite() {
        // Le même critère, sur ses deux bords. Un anneau décalé de un garde 119
        // relevés — la courbe perd sa première seconde sans que personne ne le
        // voie — ou 121, et la « fenêtre de deux minutes » n'en est plus une.
        let pleine = historique_de(CPU, MEMOIRE);
        assert_eq!(pleine.courbe(CPU), suite(0, MEMOIRE));

        let debordee = historique_de(CPU, MEMOIRE + 1);
        assert_eq!(debordee.courbe(CPU).len(), MEMOIRE);
        assert_eq!(
            debordee.courbe(CPU),
            suite(1, MEMOIRE + 1),
            "un relevé de trop fait tomber un seul relevé, le plus ancien"
        );
    }

    #[test]
    fn un_releve_illisible_occupe_sa_place_dans_l_anneau() {
        // Contrat — `Releve::Illisible` est un relevé, pas une absence de
        // relevé. Une sonde débranchée pendant deux minutes doit finir par
        // pousser hors de la courbe les valeurs d'avant, sinon la fenêtre
        // continue d'afficher une courbe pleine de mesures qui ont cessé.
        let mut historique = Historique::nouvel();
        historique.noter(CPU, releve(0));
        for _ in 0..MEMOIRE {
            historique.noter(CPU, Releve::Illisible);
        }

        let courbe = historique.courbe(CPU);
        assert_eq!(courbe.len(), MEMOIRE);
        assert!(
            courbe.iter().all(|r| *r == Releve::Illisible),
            "la valeur d'avant doit être sortie de l'anneau : {courbe:?}"
        );
    }
}

// ---------------------------------------------------------------------------

mod courbe_partielle {
    use super::{CPU, Historique, MEMOIRE, Releve, historique_de, suite};

    #[test]
    fn une_sonde_qui_vient_d_apparaitre_ne_fabrique_pas_d_historique_derriere_elle() {
        // issue #31, critère d'acceptation — « Une sonde qui vient d'apparaître
        // trace une courbe partielle, pas une ligne à zéro » ; test d'intention
        // n° 7 ; contrat — `courbe`, « **Jamais complétée** : une sonde apparue
        // il y a trois secondes rend trois relevés, pas cent vingt ».
        // Compléter à zéro dessine une chute de la température ambiante vers le
        // zéro absolu de la carte : c'est la faute la plus facile à commettre —
        // un tableau de taille fixe rempli par défaut — et la plus difficile à
        // voir, parce que le graphe reste beau.
        const SECONDES: usize = 3;
        let historique = historique_de(CPU, SECONDES);
        let courbe = historique.courbe(CPU);

        assert_eq!(courbe.len(), SECONDES, "trois relevés, pas {MEMOIRE}");
        assert_eq!(courbe, suite(0, SECONDES));
        assert!(
            !courbe.contains(&Releve::Valeur(0)),
            "aucun zéro ne doit être inventé : {courbe:?}"
        );
    }

    #[test]
    fn une_sonde_jamais_vue_n_a_ni_courbe_ni_dernier_releve() {
        // Le cas limite du même critère : la fenêtre demande la courbe d'une
        // sonde qu'elle vient de découvrir, avant le premier `status`. Une
        // courbe pleine de zéros ou un `dernier()` à zéro afficherait « 0 °C »
        // en gros sur la carte d'une sonde qui marche.
        let neuf = Historique::nouvel();
        assert_eq!(neuf.courbe(CPU), Vec::<Releve>::new());
        assert_eq!(neuf.dernier(CPU), None);
        assert_eq!(neuf.bornes(CPU), None);
        assert_eq!(neuf.sondes(), Vec::<String>::new());

        // Et une sonde connue ne prête rien à une inconnue.
        let historique = historique_de(CPU, 3);
        assert_eq!(
            historique.courbe("k10temp:inexistante"),
            Vec::<Releve>::new()
        );
        assert_eq!(historique.dernier("k10temp:inexistante"), None);
    }

    #[test]
    fn un_historique_par_defaut_est_un_historique_vide() {
        // `Historique` dérive `Default` : c'est de l'API publique. Les deux
        // constructions doivent dire la même chose, sinon un champ oublié dans
        // `nouvel()` ne se verrait que par l'un des deux chemins.
        let defaut = Historique::default();
        assert_eq!(defaut.sondes(), Vec::<String>::new());
        assert_eq!(defaut.courbe(CPU), Vec::<Releve>::new());
    }
}

// ---------------------------------------------------------------------------

mod illisible {
    use super::{CPU, Historique, Releve, releve};

    #[test]
    fn un_releve_illisible_se_garde_tel_quel_entre_deux_valeurs() {
        // issue #31, critère d'acceptation — « Une sonde qui disparaît en cours
        // de route — périphérique débranché — le dit au lieu de figer sa
        // dernière valeur » ; contrat de `sondes.rs` — « une sonde qui disparaît
        // [...] ne doit pas figer sa dernière valeur et laisser croire qu'elle
        // est encore lue ».
        // Deux replis tentants et faux : ignorer le relevé illisible — la courbe
        // saute le trou et se referme dessus —, ou l'enregistrer comme zéro — la
        // courbe pique au fond. Le trou doit rester un trou.
        let mut historique = Historique::nouvel();
        historique.noter(CPU, releve(0));
        historique.noter(CPU, Releve::Illisible);
        historique.noter(CPU, releve(2));

        let courbe = historique.courbe(CPU);
        assert_eq!(courbe.len(), 3, "l'illisible compte comme un relevé");
        assert_eq!(courbe, vec![releve(0), Releve::Illisible, releve(2)]);
        assert_ne!(
            courbe[1],
            Releve::Valeur(0),
            "un illisible n'est pas un zéro"
        );
    }

    #[test]
    fn le_dernier_releve_d_une_sonde_debranchee_est_illisible_pas_sa_valeur_d_avant() {
        // Le même critère, vu par ce que la carte affiche en gros. Après le
        // débranchement, `dernier()` doit dire « illisible » ; rendre la
        // dernière valeur connue, c'est exactement « figer sa dernière valeur ».
        let mut historique = Historique::nouvel();
        historique.noter(CPU, releve(0));
        assert_eq!(historique.dernier(CPU), Some(releve(0)));

        historique.noter(CPU, Releve::Illisible);
        assert_eq!(
            historique.dernier(CPU),
            Some(Releve::Illisible),
            "la sonde débranchée doit le dire"
        );
    }

    #[test]
    fn une_sonde_vue_seulement_illisible_est_connue_sans_avoir_de_valeur() {
        // Une sonde présente dans `status` mais illisible depuis le début — le
        // périphérique était déjà parti quand la fenêtre s'est ouverte. Elle
        // existe, donc elle a sa carte et son nom ; elle n'a simplement rien à
        // mettre à l'échelle.
        let mut historique = Historique::nouvel();
        historique.noter(CPU, Releve::Illisible);

        assert_eq!(historique.sondes(), vec![CPU.to_owned()]);
        assert_eq!(historique.courbe(CPU), vec![Releve::Illisible]);
        assert_eq!(historique.dernier(CPU), Some(Releve::Illisible));
        assert_eq!(
            historique.bornes(CPU),
            None,
            "rien de lisible, donc rien à mettre à l'échelle"
        );
    }
}

// ---------------------------------------------------------------------------

mod bornes {
    use super::{CPU, Historique, Releve, releve, valeur};

    #[test]
    fn les_bornes_encadrent_les_valeurs_lisibles_et_ignorent_les_autres() {
        // Contrat — `bornes`, « la plus basse et la plus haute valeur lisible » ;
        // issue #31, critère — la courbe « se met à l'échelle sur ce qu'elle
        // contient ». Compter un `Illisible` comme un zéro écraserait toute la
        // courbe sur le haut du cadre : trente degrés d'amplitude réelle
        // deviendraient un trait plat au-dessus d'un fond de zéro.
        let (basse, haute) = (29_000, 35_000);
        let mut historique = Historique::nouvel();
        for r in [
            Releve::Valeur(31_000),
            Releve::Illisible,
            Releve::Valeur(basse),
            Releve::Valeur(haute),
        ] {
            historique.noter(CPU, r);
        }

        assert_eq!(historique.bornes(CPU), Some((basse, haute)));
    }

    #[test]
    fn aucun_releve_lisible_ne_donne_aucune_borne() {
        // Contrat — « `None` quand aucun relevé n'est lisible — il n'y a alors
        // rien à mettre à l'échelle, et forcer un intervalle inventerait une
        // courbe ». `Some((0, 0))` serait ce mensonge-là.
        let mut historique = Historique::nouvel();
        for _ in 0..3 {
            historique.noter(CPU, Releve::Illisible);
        }
        assert_eq!(historique.bornes(CPU), None);
        assert_eq!(historique.bornes("jamais vue"), None);
    }

    #[test]
    fn une_seule_valeur_donne_un_intervalle_valide() {
        // Voir l'arbitrage en tête de fichier : sur un seul relevé, la plus
        // basse et la plus haute valeur lisible sont cette valeur. `bornes` rend
        // ce que la donnée dit ; la marge d'affichage d'une courbe plate se
        // décide au tracé, pas ici.
        let mut historique = Historique::nouvel();
        historique.noter(CPU, releve(0));

        let (basse, haute) = historique
            .bornes(CPU)
            .expect("une valeur lisible donne des bornes");
        assert!(basse <= haute, "intervalle inversé : ({basse}, {haute})");
        assert_eq!((basse, haute), (valeur(releve(0)), valeur(releve(0))));
    }

    #[test]
    fn les_bornes_sortent_des_releves_gardes_pas_de_ceux_qui_sont_tombes() {
        // « se met à l'échelle sur ce qu'elle contient » : une pointe sortie de
        // l'anneau ne doit plus peser sur l'échelle, sinon la courbe reste
        // écrasée en bas du cadre longtemps après que le pic est passé.
        use super::MEMOIRE;
        let pic = 90_000;
        let mut historique = Historique::nouvel();
        historique.noter(CPU, Releve::Valeur(pic));
        for i in 0..MEMOIRE {
            historique.noter(CPU, releve(i));
        }

        let (_, haute) = historique.bornes(CPU).expect("des valeurs lisibles");
        assert!(
            haute < pic,
            "le pic {pic} est sorti de l'anneau, il ne doit plus tirer l'échelle \
             (obtenu {haute})"
        );
    }
}

// ---------------------------------------------------------------------------

mod plusieurs_sondes {
    use super::{COOLANT, CPU, Historique, MEMOIRE, Releve, releve, suite};

    #[test]
    fn les_sondes_connues_sont_triees_et_sans_doublon() {
        // Contrat — `sondes`, « Les sondes connues, par ordre alphabétique de
        // leur nom ». La fenêtre range ses cartes dans cet ordre : sans tri,
        // elles changeraient de place à chaque tour de boucle ; avec des
        // doublons, la même sonde aurait deux cartes.
        // L'ordre attendu est celui des octets — « k10temp » avant « kraken »,
        // le chiffre passant avant la lettre — le même que celui des `slug`
        // rendus par la découverte.
        let mut historique = Historique::nouvel();
        for (sonde, tour) in [
            (COOLANT, 0),
            (CPU, 1),
            ("spd5118:temp1", 2),
            (CPU, 3),
            (COOLANT, 4),
        ] {
            historique.noter(sonde, releve(tour));
        }

        assert_eq!(
            historique.sondes(),
            vec![
                CPU.to_owned(),
                COOLANT.to_owned(),
                "spd5118:temp1".to_owned()
            ],
            "trois sondes, triées, notées cinq fois"
        );
    }

    #[test]
    fn deux_sondes_ne_partagent_ni_leur_courbe_ni_leur_memoire() {
        // Un historique commun à toutes les sondes tracerait la même courbe sur
        // toutes les cartes — et la ferait déborder quinze fois plus vite : avec
        // quinze sondes, la « fenêtre de deux minutes » n'en couvrirait que
        // huit secondes.
        const RECENTE: usize = 3;
        let mut historique = Historique::nouvel();
        for i in 0..MEMOIRE + RECENTE {
            historique.noter(CPU, releve(i));
        }
        for i in 0..RECENTE {
            historique.noter(COOLANT, Releve::Valeur(10_000 + i as i32));
        }

        assert_eq!(historique.courbe(CPU).len(), MEMOIRE, "la sonde ancienne");
        assert_eq!(
            historique.courbe(COOLANT).len(),
            RECENTE,
            "la sonde récente garde sa courbe partielle"
        );
        assert_eq!(historique.courbe(CPU), suite(RECENTE, MEMOIRE + RECENTE));
        assert_eq!(historique.dernier(COOLANT), Some(Releve::Valeur(10_002)));
        assert_eq!(
            historique.bornes(COOLANT),
            Some((10_000, 10_002)),
            "les bornes d'une sonde ne regardent qu'elle"
        );
    }

    #[test]
    fn noter_une_sonde_ne_touche_pas_les_autres() {
        // Le même piège, vu à l'envers : la sonde qui disparaît ne doit pas
        // emporter les autres. Un `Illisible` noté pour le coolant ne change ni
        // la courbe, ni le dernier relevé, ni les bornes du CPU.
        let mut historique = Historique::nouvel();
        historique.noter(CPU, releve(0));
        historique.noter(COOLANT, releve(1));

        let avant = historique.courbe(CPU);
        historique.noter(COOLANT, Releve::Illisible);

        assert_eq!(historique.courbe(CPU), avant);
        assert_eq!(historique.dernier(CPU), Some(releve(0)));
        assert_eq!(historique.dernier(COOLANT), Some(Releve::Illisible));
    }
}
