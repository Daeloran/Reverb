//! Tests d'intention — issue #21, « L'éclairage retrouve son état après un redémarrage ».
//!
//! Écrits **avant** l'implémentation, depuis l'issue #21 et le contrat public de
//! `persistance.rs` (signatures et documentation, corps `todo!()`) seuls. Si l'un de ces tests
//! échoue après implémentation, c'est le code qu'on corrige.
//!
//! ## Le défaut visé : un boîtier qui reste noir
//!
//! Le démon applique déjà un état au démarrage, mais c'est celui d'`Etat::nouveau` — noir partout.
//! La machine redémarre, l'éclairage disparaît, et rien ne le signale : ce n'est pas une panne,
//! c'est un état parfaitement valide qui se trouve être le mauvais. D'où la forme de ces tests :
//! ils ne cherchent pas une erreur, ils comparent un état à celui qu'on avait posé.
//!
//! ## La confusion qui coûterait le plus cher
//!
//! « Un fichier **absent** applique la couleur d'accueil ; un fichier disant **noir** laisse le
//! boîtier éteint. Les deux ne se confondent jamais » (critère d'acceptation). Les deux cas
//! produisent le même écran vide pour qui code vite, et les confondre a deux conséquences
//! symétriques : ou bien éteindre volontairement son boîtier le rallume en bleu au démarrage
//! suivant, ou bien une installation neuve reste noire sans que rien ne le dise. Plusieurs tests
//! ci-dessous ne servent qu'à tenir ces deux cas écartés l'un de l'autre.
//!
//! ## Deux points que le contrat laisse ouverts, et que ces tests tranchent
//!
//! 1. **L'identifiant écrit sur une ligne est le deuxième jeton.** L'en-tête du fichier documente
//!    `ventilateur <position> <rrggbb>` sans dire si `<position>` est le nom d'affichage
//!    (« radiateur haut ») ou le slug du protocole (« radiateur-haut »). Les tests de refus ne
//!    tranchent pas : ils relisent l'identifiant *dans la ligne produite par l'encodeur* et
//!    exigent seulement que le message d'erreur le nomme de la même façon. Un seul test,
//!    `le_format_documente_en_tete_de_fichier_se_relit`, pose le slug — parce qu'un nom
//!    d'affichage porte une espace (« radiateur haut ») et casserait le découpage de la ligne en
//!    jetons, ce que la documentation de `Position::slug` dit déjà pour le socket.
//! 2. **Un signalement nomme ce qui cloche.** Le contrat exige un message sur fichier abîmé sans
//!    en fixer le contenu. Ces tests demandent qu'il porte le nom du fichier et, pour un fichier
//!    tronqué, l'entrée manquante : un journal qui dit « éclairage invalide » et rien d'autre
//!    laisse l'utilisateur devant un boîtier bleu sans moyen de savoir ce qu'il a perdu.
//!
//! ## Ce que ces tests ne couvrent pas, faute de prise
//!
//! - `systemctl restart reverbd` et le redémarrage machine : de l'entrée/sortie et du cycle de vie
//!   de processus. Ce qui en est testable ici est l'aller-retour par le disque, qui en est le seul
//!   mécanisme — le reste se vérifie sur la machine.
//! - « La fenêtre n'écrit aucun fichier » : aucun crate de fenêtre n'existe encore dans l'espace de
//!   travail. Seule la décision qui l'entraîne est vérifiée ici, à savoir que l'éclairage se range
//!   sous `/var/lib` (écrit par le service, root) et non sous `/etc`.
//! - « Une animation reprend au pas 0 » : `Eclairage` ne porte aucun pas, et
//!   `l_encodage_n_ecrit_aucune_cle_refusee_par_l_animation` interdit d'en glisser un dans le
//!   fichier — `pas` ne figure dans les paramètres acceptés d'aucune animation du catalogue.

use std::fs;
use std::path::{Path, PathBuf};

use reverb_anim::{Animation, CATALOGUE, Direction, Reglages};
use reverb_daemon::persistance::{
    ACCUEIL, CHEMIN_ECLAIRAGE, CHEMIN_GEOMETRIE, Eclairage, EclairageInvalide, charger_eclairage,
    enregistrer_eclairage,
};
use reverb_proto::{Position, Rgb, ram};

// ---------------------------------------------------------------------------
// Couleurs témoins
// ---------------------------------------------------------------------------

/// La couleur témoin du ventilateur de rang `index`.
///
/// Trois propriétés, chacune pour un mode de défaillance précis :
/// - **toutes distinctes** entre elles, sinon appliquer la couleur du premier ventilateur aux dix
///   passerait inaperçu ;
/// - **`r` différent de `b`**, sinon une permutation des composantes rouge et bleu à l'encodage
///   traverserait l'aller-retour sans un seul message — et le boîtier deviendrait rouge ;
/// - **`g` différent de celui des barrettes**, sinon une couleur de ventilateur lue comme couleur
///   de barrette se confondrait avec la bonne.
fn couleur_ventilateur(index: usize) -> Rgb {
    let graine = u8::try_from(index).expect("dix ventilateurs tiennent dans un u8");
    Rgb::new(0x10 + graine, 0x80, 0xf0 - graine)
}

/// La couleur témoin de la barrette de rang `slot`, dans une famille distincte de celle des
/// ventilateurs.
fn couleur_barrette(slot: usize) -> Rgb {
    let graine = u8::try_from(slot).expect("quatre barrettes tiennent dans un u8");
    Rgb::new(0xa0 + graine, 0x30, 0x05 + graine)
}

/// Les réglages témoins : les trois champs différents de leurs valeurs par défaut.
///
/// Par défaut `Reglages` vaut `ff40ff`, vitesse 3, `horaire`. Si l'un des trois champs témoins
/// coïncidait avec son défaut, un encodage qui le perdrait serait rattrapé par le défaut au
/// décodage et l'aller-retour passerait quand même.
fn reglages_temoins() -> Reglages {
    Reglages {
        couleur: Rgb::new(0x00, 0xff, 0x00),
        vitesse: 7,
        direction: Direction::HautBas,
    }
}

/// Un éclairage fixe où chaque ventilateur et chaque barrette porte sa propre couleur.
fn eclairage_temoin() -> Eclairage {
    let mut ventilateurs = [Rgb::BLACK; 10];
    for (index, couleur) in ventilateurs.iter_mut().enumerate() {
        *couleur = couleur_ventilateur(index);
    }
    let mut barrettes = [Rgb::BLACK; ram::SLOT_COUNT];
    for (slot, couleur) in barrettes.iter_mut().enumerate() {
        *couleur = couleur_barrette(slot);
    }
    Eclairage {
        ventilateurs,
        barrettes,
        animation: None,
    }
}

/// Le même, avec une animation en cours et ses réglages.
fn eclairage_temoin_anime() -> Eclairage {
    Eclairage {
        animation: Some((animation("vague"), reglages_temoins())),
        ..eclairage_temoin()
    }
}

/// Un éclairage entièrement noir, rien d'animé : ce que laisse `animate off` suivi d'une
/// extinction.
fn eclairage_noir() -> Eclairage {
    Eclairage {
        ventilateurs: [Rgb::BLACK; 10],
        barrettes: [Rgb::BLACK; ram::SLOT_COUNT],
        animation: None,
    }
}

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| {
        panic!("« {nom} » doit figurer au catalogue des animations : {erreur:?}")
    })
}

fn hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

// ---------------------------------------------------------------------------
// Manipulation de texte
// ---------------------------------------------------------------------------

/// Les lignes qui portent une information : ni vides, ni commentaires.
fn lignes_actives(texte: &str) -> Vec<String> {
    texte
        .lines()
        .map(str::trim)
        .filter(|ligne| !ligne.is_empty() && !ligne.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// Les lignes actives d'un genre donné (`ventilateur`, `barrette`, `animation`).
fn lignes_du_genre(texte: &str, genre: &str) -> Vec<String> {
    lignes_actives(texte)
        .into_iter()
        .filter(|ligne| ligne.split_whitespace().next() == Some(genre))
        .collect()
}

/// L'identifiant que porte une ligne : son deuxième jeton.
///
/// Relu dans la ligne produite par l'encodeur plutôt que reconstruit ici — voir le préambule :
/// c'est ce qui permet aux tests de refus de ne pas trancher entre nom d'affichage et slug.
fn identifiant(ligne: &str) -> String {
    ligne
        .split_whitespace()
        .nth(1)
        .unwrap_or_else(|| {
            panic!("la ligne « {ligne} » ne porte aucun identifiant en deuxième jeton")
        })
        .to_owned()
}

/// La même ligne, avec une autre couleur en dernier jeton.
///
/// Sert aux doublons : deux lignes **identiques** laisseraient à un décodeur la tentation de les
/// fusionner sans rien dire. Deux lignes contradictoires forcent à choisir, et le contrat dit de
/// refuser plutôt que de choisir.
fn avec_une_autre_couleur(ligne: &str) -> String {
    let mut jetons: Vec<&str> = ligne.split_whitespace().collect();
    let dernier = jetons.len() - 1;
    jetons[dernier] = "010203";
    jetons.join(" ")
}

fn sans_la_ligne(texte: &str, a_retirer: &str) -> String {
    let restantes: Vec<String> = lignes_actives(texte)
        .into_iter()
        .filter(|ligne| ligne != a_retirer)
        .collect();
    restantes.join("\n")
}

fn avec_la_ligne(texte: &str, a_ajouter: &str) -> String {
    let mut lignes = lignes_actives(texte);
    lignes.push(a_ajouter.to_owned());
    lignes.join("\n")
}

/// L'aller-retour en mémoire, avec un échec qui recopie le texte fautif.
fn aller_retour(eclairage: &Eclairage) -> Eclairage {
    let texte = eclairage.encoder();
    Eclairage::decoder(&texte).unwrap_or_else(|erreur| {
        panic!(
            "Un éclairage produit par `encoder` doit se relire par `decoder`.\n  \
             Refus : {erreur}\n  \
             Texte produit :\n{texte}"
        )
    })
}

fn refus(texte: &str) -> EclairageInvalide {
    match Eclairage::decoder(texte) {
        Ok(eclairage) => panic!(
            "Ce fichier devait être refusé, il a été accepté et a rendu {eclairage:?}.\n  \
             Texte :\n{texte}"
        ),
        Err(erreur) => erreur,
    }
}

// ---------------------------------------------------------------------------
// Dossier temporaire — sans dépendance, c'est un garde-fou du projet
// ---------------------------------------------------------------------------

/// Un dossier de travail sous `std::env::temp_dir()`, effacé à la fin du test.
///
/// L'effacement passe par `Drop` et non par une ligne en fin de test : un test qui échoue part en
/// `panic!`, et une fin de test jamais atteinte laisserait le dossier derrière elle à chaque
/// régression.
struct DossierJetable {
    chemin: PathBuf,
}

impl DossierJetable {
    /// `nom` doit être le nom du test : c'est ce qui rend le chemin unique entre tests, `cargo`
    /// les exécutant en parallèle dans un même processus.
    fn neuf(nom: &str) -> DossierJetable {
        let chemin = std::env::temp_dir().join(format!(
            "reverb-spec-eclairage-{}-{nom}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&chemin);
        fs::create_dir_all(&chemin)
            .unwrap_or_else(|erreur| panic!("dossier de test {} : {erreur}", chemin.display()));
        DossierJetable { chemin }
    }

    fn fichier(&self, nom: &str) -> PathBuf {
        self.chemin.join(nom)
    }

    fn chemin(&self) -> &Path {
        &self.chemin
    }
}

impl Drop for DossierJetable {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.chemin);
    }
}

fn ecrire_fichier(chemin: &Path, contenu: &str) {
    fs::write(chemin, contenu)
        .unwrap_or_else(|erreur| panic!("écriture de {} : {erreur}", chemin.display()));
}

/// Le nom du fichier, tel qu'un message d'erreur devrait au minimum le mentionner.
fn nom_de_fichier(chemin: &Path) -> String {
    chemin
        .file_name()
        .expect("le chemin de test porte un nom de fichier")
        .to_string_lossy()
        .into_owned()
}

// ---------------------------------------------------------------------------
// L'accueil, et où vivent les fichiers
// ---------------------------------------------------------------------------

#[test]
fn l_accueil_est_le_bleu_pur_sur_les_dix_ventilateurs_et_les_quatre_barrettes() {
    // Décision de cadrage : « la couleur d'accueil est `0000ff`, bleu pur, sur les ventilateurs
    // **et** la RAM ». Les deux bus, pour qu'une installation qui n'a jamais reçu de commande
    // prouve d'elle-même que la chaîne complète fonctionne.
    assert_eq!(
        ACCUEIL,
        Rgb::new(0x00, 0x00, 0xff),
        "La couleur d'accueil doit être le bleu pur `0000ff`, en RGB logique.\n  \
         Une permutation des composantes ici ne produirait aucune erreur : juste un boîtier rouge \
         au premier démarrage, que personne ne saurait relier à cette ligne."
    );

    let accueil = Eclairage::accueil();
    for position in Position::ALL {
        assert_eq!(
            accueil.ventilateurs[position.index()],
            ACCUEIL,
            "Le ventilateur « {} » doit s'allumer en accueil au premier démarrage.",
            position.name()
        );
    }
    for slot in 0..ram::SLOT_COUNT {
        assert_eq!(
            accueil.barrettes[slot], ACCUEIL,
            "La barrette {slot} doit s'allumer en accueil au premier démarrage : la RAM fait \
             partie de la chaîne à prouver, au même titre que les ventilateurs."
        );
    }
    assert_eq!(
        accueil.animation, None,
        "L'accueil est un éclairage fixe : rien à rejouer au premier démarrage."
    );
}

#[test]
fn l_accueil_n_est_pas_du_noir() {
    // Tout l'enjeu de l'issue tient dans cet écart : si l'accueil était noir, un fichier absent et
    // un fichier disant noir deviendraient indiscernables, et la moitié des critères
    // d'acceptation passeraient à vide.
    assert_ne!(
        ACCUEIL,
        Rgb::BLACK,
        "L'accueil doit être visible : c'est ce qui distingue « jamais réglé » de « réglé sur \
         noir », et ce qui permet de constater qu'une installation neuve fonctionne."
    );
    assert_ne!(
        Eclairage::accueil(),
        eclairage_noir(),
        "L'éclairage d'accueil ne doit pas être l'éclairage noir."
    );
}

#[test]
fn les_deux_fichiers_ne_vivent_pas_au_meme_endroit() {
    // Décision de cadrage : la géométrie est une donnée de montage (`/etc`, réglée par
    // l'administrateur), l'éclairage est l'état courant du service (`/var/lib`, écrit par le
    // démon à chaque commande). C'est aussi ce qui tient le critère « la fenêtre n'écrit aucun
    // fichier » : un état sous `/var/lib` n'est écrivable que par root, donc par le démon, donc
    // par le socket — l'unique franchissement de privilège.
    assert!(
        CHEMIN_ECLAIRAGE.starts_with("/var/lib/"),
        "L'éclairage courant se range sous `/var/lib`, avec ce qu'un service accumule — pas sous \
         `/etc`, qui est réservé à ce que l'administrateur règle. Trouvé : {CHEMIN_ECLAIRAGE}"
    );
    assert!(
        CHEMIN_GEOMETRIE.starts_with("/etc/"),
        "La géométrie reste sous `/etc` : elle se mesure une fois au montage. Trouvé : \
         {CHEMIN_GEOMETRIE}"
    );
    assert_ne!(
        CHEMIN_ECLAIRAGE, CHEMIN_GEOMETRIE,
        "Mêler les deux ferait réécrire à chaque changement de couleur le fichier qui a coûté un \
         relevé au sol."
    );
}

// ---------------------------------------------------------------------------
// Aller-retour en mémoire
// ---------------------------------------------------------------------------

#[test]
fn un_eclairage_fixe_traverse_l_aller_retour_sans_rien_perdre() {
    let pose = eclairage_temoin();
    assert_eq!(
        aller_retour(&pose),
        pose,
        "Un éclairage fixe encodé puis décodé doit rendre l'état d'origine : c'est le mécanisme \
         entier par lequel une couleur posée par `light` survit à un redémarrage."
    );
}

#[test]
fn chaque_ventilateur_garde_sa_couleur_a_l_aller_retour() {
    // Le mode de défaillance visé n'est pas la perte de couleur mais sa **propagation** : dix
    // ventilateurs qui repartent tous dans la teinte du premier. Un `assert_eq!` sur l'état entier
    // le détecterait, mais dirait mal ce qui cloche — d'où la boucle position par position.
    let relu = aller_retour(&eclairage_temoin());
    for position in Position::ALL {
        let attendue = couleur_ventilateur(position.index());
        assert_eq!(
            relu.ventilateurs[position.index()],
            attendue,
            "Le ventilateur « {} » (rang {}) doit retrouver sa propre couleur {}, pas celle d'un \
             autre. Chaque position porte la sienne dans le fichier : `light fan:arriere` ne doit \
             jamais ressortir appliqué aux dix.",
            position.name(),
            position.index(),
            hexa(attendue)
        );
    }
}

#[test]
fn chaque_barrette_garde_sa_couleur_a_l_aller_retour() {
    let relu = aller_retour(&eclairage_temoin());
    for slot in 0..ram::SLOT_COUNT {
        let attendue = couleur_barrette(slot);
        assert_eq!(
            relu.barrettes[slot],
            attendue,
            "La barrette {slot} doit retrouver sa propre couleur {}, pas celle d'une voisine : \
             l'index de barrette est la seule chose qui les distingue dans le fichier.",
            hexa(attendue)
        );
    }
}

#[test]
fn l_accueil_traverse_l_aller_retour() {
    let accueil = Eclairage::accueil();
    assert_eq!(
        aller_retour(&accueil),
        accueil,
        "L'accueil est un état comme un autre : il doit s'écrire et se relire comme les autres."
    );
}

#[test]
fn un_eclairage_noir_traverse_l_aller_retour() {
    let noir = eclairage_noir();
    assert_eq!(
        aller_retour(&noir),
        noir,
        "« `animate off` puis extinction est un état comme un autre » : le noir doit traverser \
         l'aller-retour tel quel, sans être relevé en accueil au passage."
    );
}

#[test]
fn une_animation_et_ses_reglages_traversent_l_aller_retour() {
    let pose = eclairage_temoin_anime();
    let relu = aller_retour(&pose);

    let (animation_relue, reglages_relus) = relu
        .animation
        .expect("une animation en cours doit encore être en cours après l'aller-retour");
    assert_eq!(
        animation_relue,
        animation("vague"),
        "L'animation rejouée doit être celle qui tournait."
    );
    assert_eq!(
        reglages_relus.couleur,
        reglages_temoins().couleur,
        "La couleur de l'animation doit survivre : la reprendre sur le magenta par défaut serait \
         un réglage perdu que rien ne signalerait."
    );
    assert_eq!(
        reglages_relus.vitesse,
        reglages_temoins().vitesse,
        "La vitesse de l'animation doit survivre."
    );
    assert_eq!(
        reglages_relus.direction,
        reglages_temoins().direction,
        "La direction de l'animation doit survivre."
    );
    assert_eq!(
        relu.ventilateurs, pose.ventilateurs,
        "Les couleurs fixes restent sous l'animation : `animate off` doit rendre la couleur \
         d'avant, y compris après un redémarrage."
    );
    assert_eq!(
        relu.barrettes, pose.barrettes,
        "Les couleurs fixes des barrettes restent sous l'animation, pour la même raison."
    );
    assert_eq!(relu, pose, "L'état entier doit être rendu à l'identique.");
}

#[test]
fn toutes_les_animations_du_catalogue_traversent_l_aller_retour() {
    // Une seule animation testée laisserait passer la famille entière qui refuse `couleur` :
    // `arc-en-ciel` génère ses teintes, on ne lui écrit donc pas de couleur, et son aller-retour
    // doit passer **quand même**. Le catalogue est parcouru en entier pour qu'ajouter une
    // animation sans la faire traverser soit impossible.
    for nom in CATALOGUE {
        let animation = animation(nom);
        let reglages = reglages_acceptables(animation);
        assert_ne!(
            reglages,
            Reglages::default(),
            "Les réglages témoins de « {nom} » doivent différer des réglages par défaut, sinon un \
             encodage qui les perdrait serait rattrapé par le défaut au décodage."
        );

        let pose = Eclairage {
            animation: Some((animation, reglages)),
            ..eclairage_temoin()
        };
        assert_eq!(
            aller_retour(&pose),
            pose,
            "« {nom} » et ses réglages doivent traverser l'aller-retour sans rien perdre."
        );
    }
}

/// Des réglages construits **uniquement** avec les clés que l'animation accepte, comme le ferait
/// une commande venue du socket.
fn reglages_acceptables(animation: Animation) -> Reglages {
    let paires: Vec<(String, String)> = animation
        .parametres_acceptes()
        .iter()
        .map(|cle| {
            let valeur = match *cle {
                "couleur" => hexa(reglages_temoins().couleur),
                "vitesse" => reglages_temoins().vitesse.to_string(),
                "direction" => reglages_temoins().direction.slug().to_owned(),
                autre => panic!(
                    "« {autre} » est un paramètre d'animation que ce test ne sait pas fabriquer : \
                     l'étendre plutôt que le contourner."
                ),
            };
            ((*cle).to_owned(), valeur)
        })
        .collect();

    animation.reglages(&paires).unwrap_or_else(|erreur| {
        panic!(
            "« {} » doit accepter ses propres paramètres : {erreur:?}",
            animation.nom()
        )
    })
}

#[test]
fn l_encodage_n_ecrit_aucune_cle_refusee_par_l_animation() {
    // Contrat : « les réglages d'une animation ne sont écrits que pour les clés qu'elle accepte ».
    // Écrire `couleur=` sur une ligne `arc-en-ciel` ferait rejeter le fichier **en bloc** au
    // démarrage suivant — l'éclairage entier perdu pour une clé de trop. Le mode de défaillance
    // est différé d'un redémarrage : rien ne se voit à l'écriture.
    for nom in CATALOGUE {
        let animation = animation(nom);
        let pose = Eclairage {
            animation: Some((animation, reglages_acceptables(animation))),
            ..eclairage_temoin()
        };
        let texte = pose.encoder();
        let lignes = lignes_du_genre(&texte, "animation");
        assert_eq!(
            lignes.len(),
            1,
            "Un éclairage animé doit produire exactement une ligne `animation`. Texte :\n{texte}"
        );

        let acceptees = animation.parametres_acceptes();
        for jeton in lignes[0].split_whitespace().skip(2) {
            let cle = jeton.split('=').next().unwrap_or(jeton);
            assert!(
                acceptees.contains(&cle),
                "La ligne `{}` porte la clé « {cle} », que « {nom} » n'accepte pas (acceptées : \
                 {acceptees:?}). Ce fichier serait rejeté en bloc au démarrage suivant.",
                lignes[0]
            );
        }
    }
}

#[test]
fn l_absence_d_animation_reste_une_absence_a_l_aller_retour() {
    let texte = eclairage_temoin().encoder();
    assert!(
        lignes_du_genre(&texte, "animation").is_empty(),
        "Un éclairage fixe ne doit écrire aucune ligne `animation` : « au plus une, absente si \
         rien n'est animé ». Texte :\n{texte}"
    );
    assert_eq!(
        aller_retour(&eclairage_temoin()).animation,
        None,
        "Un éclairage fixe doit rester fixe après l'aller-retour : lui inventer une animation au \
         décodage ferait repartir un boîtier posé en couleur fixe."
    );
}

#[test]
fn l_ordre_des_lignes_est_sans_importance() {
    // Contrat : « l'ordre des lignes est sans importance ». Ce n'est pas de la souplesse gratuite :
    // un décodeur qui lirait les lignes par leur rang accepterait un fichier permuté en attribuant
    // chaque couleur au mauvais ventilateur — sans une seule erreur.
    let pose = eclairage_temoin_anime();
    let texte = pose.encoder();

    let mut a_l_envers = lignes_actives(&texte);
    a_l_envers.reverse();
    assert_eq!(
        Eclairage::decoder(&a_l_envers.join("\n")).expect("un fichier inversé reste valide"),
        pose,
        "Les mêmes lignes dans l'ordre inverse doivent rendre le même éclairage."
    );

    let mut decalees = lignes_actives(&texte);
    decalees.rotate_left(3);
    assert_eq!(
        Eclairage::decoder(&decalees.join("\n")).expect("un fichier permuté reste valide"),
        pose,
        "Les mêmes lignes décalées doivent rendre le même éclairage."
    );
}

#[test]
fn les_lignes_vides_et_les_commentaires_sont_ignores() {
    // Le fichier écrit par le démon porte un en-tête en commentaire : si les commentaires n'étaient
    // pas ignorés, le démon ne saurait pas relire ce qu'il vient d'écrire.
    let pose = eclairage_temoin_anime();
    let mut lignes: Vec<String> = vec![
        "# un commentaire en tête".to_owned(),
        String::new(),
        "   ".to_owned(),
    ];
    for ligne in lignes_actives(&pose.encoder()) {
        lignes.push(ligne);
        lignes.push("# intercalé".to_owned());
        lignes.push(String::new());
    }

    assert_eq!(
        Eclairage::decoder(&lignes.join("\n")).expect("commentaires et lignes vides sont ignorés"),
        pose,
        "Les lignes vides et celles commençant par `#` ne portent aucune information : elles ne \
         doivent ni compter comme une entrée, ni faire échouer la lecture."
    );
}

#[test]
fn le_format_documente_en_tete_de_fichier_se_relit() {
    // Le seul test qui pose le format à la main, tel que l'en-tête du fichier le documente :
    //   ventilateur <position> <rrggbb>
    //   barrette <0-3> <rrggbb>
    //   animation <nom> [cle=valeur ...]
    // La position s'écrit en slug et non en nom d'affichage : « radiateur haut » porte une espace,
    // qui casserait le découpage de la ligne en jetons — c'est déjà la raison d'être de
    // `Position::slug` sur le socket.
    let mut lignes: Vec<String> = Vec::new();
    for position in Position::ALL {
        lignes.push(format!(
            "ventilateur {} {}",
            position.slug(),
            hexa(couleur_ventilateur(position.index()))
        ));
    }
    for slot in 0..ram::SLOT_COUNT {
        lignes.push(format!("barrette {slot} {}", hexa(couleur_barrette(slot))));
    }
    lignes.push(format!(
        "animation vague couleur={} vitesse={} direction={}",
        hexa(reglages_temoins().couleur),
        reglages_temoins().vitesse,
        reglages_temoins().direction.slug()
    ));

    let texte = lignes.join("\n");
    assert_eq!(
        Eclairage::decoder(&texte).unwrap_or_else(|erreur| panic!(
            "Le format documenté en tête de fichier doit se relire.\n  Refus : {erreur}\n  \
             Texte :\n{texte}"
        )),
        eclairage_temoin_anime(),
        "Un fichier écrit dans la forme documentée doit rendre l'éclairage qu'il décrit."
    );
}

// ---------------------------------------------------------------------------
// Ce qui doit être refusé, et nommé
// ---------------------------------------------------------------------------

#[test]
fn une_position_absente_est_refusee_en_la_nommant() {
    // « Une position absente est refusée en la nommant : c'est ce qui rend un fichier tronqué
    // détectable. » Compléter au jugé la position manquante — par du noir, par l'accueil — rendrait
    // un éclairage plausible et faux, sans un mot dans le journal.
    let texte = eclairage_temoin().encoder();
    for ligne in lignes_du_genre(&texte, "ventilateur") {
        let manquante = identifiant(&ligne);
        let tronque = sans_la_ligne(&texte, &ligne);
        let erreur = refus(&tronque);

        assert!(
            erreur.raison.contains(&manquante),
            "Le refus doit nommer le ventilateur manquant « {manquante} ». Raison obtenue : {}",
            erreur.raison
        );
        assert_eq!(
            erreur.ligne, 0,
            "Une entrée absente n'est écrite nulle part : le numéro de ligne doit valoir 0 plutôt \
             que de pointer une ligne innocente. Entrée manquante : « {manquante} »"
        );
    }
}

#[test]
fn une_barrette_absente_est_refusee_en_la_nommant() {
    let texte = eclairage_temoin().encoder();
    let lignes = lignes_du_genre(&texte, "barrette");
    assert_eq!(
        lignes.len(),
        ram::SLOT_COUNT,
        "Les quatre barrettes doivent être écrites, exactement une fois chacune. Texte :\n{texte}"
    );

    for ligne in lignes {
        let manquante = identifiant(&ligne);
        let erreur = refus(&sans_la_ligne(&texte, &ligne));
        assert!(
            erreur.raison.contains(&manquante),
            "Le refus doit nommer la barrette manquante « {manquante} ». Raison obtenue : {}",
            erreur.raison
        );
        assert_eq!(
            erreur.ligne, 0,
            "Une barrette absente n'est écrite nulle part : numéro de ligne 0 attendu."
        );
    }
}

#[test]
fn une_position_repetee_est_refusee_en_la_nommant() {
    // Le danger n'est pas le doublon identique mais le doublon **contradictoire** : garder
    // silencieusement la dernière valeur donne un éclairage qui dépend de l'ordre des lignes,
    // alors que le contrat dit justement que cet ordre est sans importance.
    let texte = eclairage_temoin().encoder();
    for ligne in lignes_du_genre(&texte, "ventilateur") {
        let repetee = identifiant(&ligne);
        let double = avec_la_ligne(&texte, &avec_une_autre_couleur(&ligne));
        let erreur = refus(&double);

        assert!(
            erreur.raison.contains(&repetee),
            "Le refus doit nommer le ventilateur répété « {repetee} ». Raison obtenue : {}",
            erreur.raison
        );
        assert!(
            erreur.ligne >= 1,
            "Un doublon est écrit quelque part : le refus doit pointer une ligne, pas 0."
        );
    }
}

#[test]
fn une_barrette_repetee_est_refusee_en_la_nommant() {
    let texte = eclairage_temoin().encoder();
    for ligne in lignes_du_genre(&texte, "barrette") {
        let repetee = identifiant(&ligne);
        let erreur = refus(&avec_la_ligne(&texte, &avec_une_autre_couleur(&ligne)));
        assert!(
            erreur.raison.contains(&repetee),
            "Le refus doit nommer la barrette répétée « {repetee} ». Raison obtenue : {}",
            erreur.raison
        );
        assert!(
            erreur.ligne >= 1,
            "Un doublon est écrit quelque part : le refus doit pointer une ligne, pas 0."
        );
    }
}

#[test]
fn deux_animations_dans_le_meme_fichier_sont_refusees() {
    // « animation <nom> [cle=valeur ...] : **au plus une** ». Deux lignes, et c'est l'ordre de
    // lecture qui déciderait de ce que joue le boîtier au démarrage.
    let texte = eclairage_temoin_anime().encoder();
    let ligne = lignes_du_genre(&texte, "animation")
        .into_iter()
        .next()
        .expect("un éclairage animé écrit une ligne `animation`");
    let seconde = ligne.replacen("vague", "comete", 1);
    assert_ne!(
        seconde, ligne,
        "La seconde ligne d'animation doit contredire la première, sinon le doublon serait \
         inoffensif et le test ne prouverait rien."
    );

    let erreur = refus(&avec_la_ligne(&texte, &seconde));
    assert!(
        erreur.raison.contains("animation"),
        "Le refus doit dire que c'est l'animation qui est en double. Raison obtenue : {}",
        erreur.raison
    );
}

#[test]
fn un_fichier_de_travers_est_refuse_en_pointant_la_ligne() {
    let valide = eclairage_temoin().encoder();
    let une_ligne_ventilateur = lignes_du_genre(&valide, "ventilateur")
        .into_iter()
        .next()
        .expect("l'encodeur écrit des lignes `ventilateur`");
    let position = identifiant(&une_ligne_ventilateur);
    let une_ligne_barrette = lignes_du_genre(&valide, "barrette")
        .into_iter()
        .next()
        .expect("l'encodeur écrit des lignes `barrette`");
    let barrette = identifiant(&une_ligne_barrette);

    let cas: Vec<(String, &str)> = vec![
        (
            avec_la_ligne(&valide, "cequecest arriere ff0000"),
            "un genre de ligne inconnu",
        ),
        (
            sans_la_ligne(&valide, &une_ligne_ventilateur)
                + &format!("\nventilateur {position} pas-une-couleur"),
            "une couleur qui n'est pas de l'hexadécimal",
        ),
        (
            sans_la_ligne(&valide, &une_ligne_ventilateur) + &format!("\nventilateur {position}"),
            "une ligne tronquée en plein milieu",
        ),
        (
            avec_la_ligne(&valide, "ventilateur pas-un-ventilateur ff0000"),
            "une position qui n'existe pas",
        ),
        (
            avec_la_ligne(&valide, &format!("barrette {} 00ff00", ram::SLOT_COUNT)),
            "une barrette hors des quatre",
        ),
        (
            avec_la_ligne(&valide, "animation pas-au-catalogue"),
            "une animation absente du catalogue",
        ),
        // Un jeton de trop n'est ni une entrée absente ni une entrée répétée : rien dans le
        // contrat n'oblige à le refuser. Il est refusé quand même, par cohérence avec le
        // protocole IPC, qui rejette déjà `couleur==ff00ff` — « c'est un dialogue entre deux
        // programmes, pas une saisie humaine ». Une couleur en trop silencieusement jetée, c'est
        // un réglage perdu sans un mot, exactement le mode de défaillance de cette issue.
        (
            sans_la_ligne(&valide, &une_ligne_ventilateur)
                + &format!("\nventilateur {position} ff0000 00ff00"),
            "un jeton de trop sur une ligne de ventilateur",
        ),
        (
            sans_la_ligne(&valide, &une_ligne_barrette)
                + &format!("\nbarrette {barrette} ff0000 00ff00"),
            "un jeton de trop sur une ligne de barrette",
        ),
    ];

    for (texte, description) in cas {
        let erreur = refus(&texte);
        assert!(
            erreur.ligne >= 1,
            "{description} : la faute tient à une ligne précise, le refus doit la pointer plutôt \
             que de rendre 0. Refus obtenu : {erreur}"
        );
        assert!(
            erreur.ligne <= texte.lines().count(),
            "{description} : la ligne {} pointée n'existe pas ({} lignes). Un numéro hors du \
             fichier envoie l'utilisateur regarder au mauvais endroit.",
            erreur.ligne,
            texte.lines().count()
        );
        assert!(
            !erreur.raison.trim().is_empty(),
            "{description} : le refus doit dire ce qui cloche, pas seulement où."
        );
    }
}

#[test]
fn le_numero_de_ligne_compte_a_partir_de_un() {
    // Contrat : « numéro de ligne dans le texte, **à partir de 1** — comme un éditeur ». Un
    // décalage d'une unité ne se voit dans aucun aller-retour, ne fait échouer aucun démarrage, et
    // envoie l'utilisateur corriger la ligne d'à côté. Les lignes vides et les commentaires
    // comptent dans la numérotation : c'est le rang dans le fichier qu'on ouvre, pas le rang parmi
    // les lignes actives.
    let valide = eclairage_temoin().encoder();

    let en_tete = format!("pas-une-ligne-valide\n{valide}");
    assert_eq!(
        refus(&en_tete).ligne,
        1,
        "Une faute sur la première ligne du fichier doit être signalée ligne 1."
    );

    let plus_bas = format!("# un commentaire\n\npas-une-ligne-valide\n{valide}");
    assert_eq!(
        refus(&plus_bas).ligne,
        3,
        "Le commentaire et la ligne vide comptent dans la numérotation : ce qu'on donne à \
         l'utilisateur, c'est le rang de la ligne dans son éditeur."
    );
}

// ---------------------------------------------------------------------------
// Le disque : ce qui se passe vraiment au démarrage
// ---------------------------------------------------------------------------

#[test]
fn un_fichier_absent_rend_l_accueil_sans_rien_dire() {
    let dossier = DossierJetable::neuf("fichier_absent");
    let chemin = dossier.fichier("eclairage.conf");

    let (eclairage, signalement) = charger_eclairage(&chemin);
    assert_eq!(
        eclairage,
        Eclairage::accueil(),
        "Un fichier absent, c'est le premier démarrage : le boîtier s'allume en accueil."
    );
    assert_eq!(
        signalement, None,
        "Le premier démarrage n'est pas une anomalie : rien à signaler. Un message ici polluerait \
         le journal de toute installation neuve. Obtenu : {signalement:?}"
    );
}

#[test]
fn lire_un_fichier_absent_ne_le_cree_pas() {
    // Lire, c'est lire. Un démarrage qui écrirait l'accueil en passant effacerait la distinction
    // entre « jamais réglé » et « réglé », et ferait de la première lecture un état posé.
    let dossier = DossierJetable::neuf("lecture_sans_ecriture");
    let chemin = dossier.fichier("eclairage.conf");

    let _ = charger_eclairage(&chemin);
    assert!(
        !chemin.exists(),
        "Charger un éclairage absent ne doit rien écrire : {} a été créé.",
        chemin.display()
    );
}

#[test]
fn un_fichier_disant_noir_laisse_le_boitier_eteint() {
    // Critère d'acceptation : « `animate off` puis extinction est un état comme un autre : on
    // retrouve le noir, on ne retrouve pas la couleur d'avant ». Rendre l'accueil ici rallumerait
    // en bleu, au démarrage suivant, le boîtier qu'on vient d'éteindre volontairement.
    let dossier = DossierJetable::neuf("fichier_noir");
    let chemin = dossier.fichier("eclairage.conf");
    ecrire_fichier(&chemin, &eclairage_noir().encoder());

    let (eclairage, signalement) = charger_eclairage(&chemin);
    assert_eq!(
        eclairage,
        eclairage_noir(),
        "Un fichier disant noir doit laisser le boîtier éteint."
    );
    assert_eq!(
        signalement, None,
        "Un fichier noir est parfaitement valide : rien à signaler. Obtenu : {signalement:?}"
    );
}

#[test]
fn l_absence_de_fichier_et_un_fichier_noir_ne_se_confondent_jamais() {
    // Les deux cas se ressemblent — un écran vide, un fichier qu'on ne relit pas — et les
    // confondre a deux conséquences symétriques : éteindre son boîtier le rallume, ou une
    // installation neuve reste noire sans que rien ne le dise.
    let dossier = DossierJetable::neuf("absent_contre_noir");
    let absent = dossier.fichier("absent.conf");
    let noir = dossier.fichier("noir.conf");
    ecrire_fichier(&noir, &eclairage_noir().encoder());

    let (sur_absent, _) = charger_eclairage(&absent);
    let (sur_noir, _) = charger_eclairage(&noir);
    assert_ne!(
        sur_absent, sur_noir,
        "Un fichier absent et un fichier disant noir doivent donner deux éclairages différents."
    );
    assert_eq!(
        sur_absent,
        Eclairage::accueil(),
        "L'absent donne l'accueil."
    );
    assert_eq!(sur_noir, eclairage_noir(), "Le noir donne le noir.");
}

#[test]
fn un_fichier_illisible_rend_l_accueil_et_un_signalement() {
    // Deux formes d'illisible, pour la même conclusion : la machine s'allume quand même, et le
    // journal garde trace de ce qui a été perdu. « La différence entre "jamais réglé" et "réglé
    // puis abîmé" ne doit pas se perdre. »
    let dossier = DossierJetable::neuf("fichier_illisible");

    // Un dossier là où le démon attend un fichier : la lecture échoue au niveau système, avant
    // même qu'il y ait du texte à analyser.
    let en_dossier = dossier.fichier("dossier.conf");
    fs::create_dir(&en_dossier).expect("création du faux fichier");

    // Des octets qui ne sont pas de l'UTF-8 : une écriture coupée par une panne de courant peut
    // laisser exactement ça.
    let binaire = dossier.fichier("binaire.conf");
    fs::write(&binaire, [0xff_u8, 0xfe, 0x00, 0x80]).expect("écriture d'octets invalides");

    for chemin in [&en_dossier, &binaire] {
        let (eclairage, signalement) = charger_eclairage(chemin);
        assert_eq!(
            eclairage,
            Eclairage::accueil(),
            "Un fichier illisible ne doit pas laisser la machine dans le noir : l'accueil est \
             appliqué. Chemin : {}",
            chemin.display()
        );
        let message = signalement.unwrap_or_else(|| {
            panic!(
                "Un fichier illisible doit être signalé, sans quoi il se confond avec un premier \
                 démarrage et l'utilisateur ne saura jamais qu'il a perdu son éclairage. Chemin : \
                 {}",
                chemin.display()
            )
        });
        assert!(
            message.contains(&nom_de_fichier(chemin)),
            "Le signalement doit nommer le fichier en cause. Obtenu : {message}"
        );
    }
}

#[test]
fn un_fichier_tronque_rend_l_accueil_et_un_signalement() {
    let dossier = DossierJetable::neuf("fichier_tronque");
    let chemin = dossier.fichier("eclairage.conf");

    let texte = eclairage_temoin().encoder();
    let perdue = lignes_du_genre(&texte, "ventilateur")
        .into_iter()
        .next()
        .expect("l'encodeur écrit des lignes `ventilateur`");
    ecrire_fichier(&chemin, &sans_la_ligne(&texte, &perdue));

    let (eclairage, signalement) = charger_eclairage(&chemin);
    assert_eq!(
        eclairage,
        Eclairage::accueil(),
        "Un fichier tronqué ne doit pas être complété au jugé : l'accueil est appliqué en entier, \
         ce qui se voit, plutôt qu'un éclairage à neuf dixièmes juste qui ne se voit pas."
    );
    let message = signalement.expect("un fichier tronqué doit être signalé");
    assert!(
        message.contains(&nom_de_fichier(&chemin)),
        "Le signalement doit nommer le fichier en cause. Obtenu : {message}"
    );
    assert!(
        message.contains(&identifiant(&perdue)),
        "Le signalement doit dire ce qui manque — ici « {} ». Un journal qui dit seulement \
         « éclairage invalide » laisse l'utilisateur devant un boîtier bleu sans savoir pourquoi. \
         Obtenu : {message}",
        identifiant(&perdue)
    );
}

#[test]
fn un_fichier_de_travers_n_empeche_jamais_le_demarrage() {
    // Critère d'acceptation : « un fichier de travers ne doit jamais empêcher le démon de
    // démarrer ». `charger_eclairage` ne peut pas rendre d'erreur — sa signature l'interdit — donc
    // ce qui est vérifié ici est qu'aucune de ces entrées ne le fait paniquer, et que toutes
    // rendent l'accueil accompagné d'un message.
    let dossier = DossierJetable::neuf("fichier_de_travers");
    let valide = eclairage_temoin().encoder();

    let cas: Vec<(&str, String)> = vec![
        ("vide", String::new()),
        ("que des commentaires", "# rien d'autre\n#\n".to_owned()),
        ("du texte au hasard", "n'importe quoi\n".to_owned()),
        (
            "coupé en plein milieu",
            valide.chars().take(valide.len() / 2).collect(),
        ),
        (
            "un fichier d'une autre nature",
            "bas-gauche 0 horaire\n".to_owned(),
        ),
        ("une seule accolade", "{".to_owned()),
    ];

    for (description, contenu) in cas {
        let chemin = dossier.fichier("eclairage.conf");
        ecrire_fichier(&chemin, &contenu);
        let (eclairage, signalement) = charger_eclairage(&chemin);
        assert_eq!(
            eclairage,
            Eclairage::accueil(),
            "Fichier « {description} » : l'accueil doit être appliqué plutôt que de laisser la \
             machine sans éclairage du tout."
        );
        assert!(
            signalement.is_some(),
            "Fichier « {description} » : un fichier présent mais inexploitable doit être signalé, \
             sinon il se confond avec un premier démarrage."
        );
    }
}

#[test]
fn une_couleur_fixe_est_retrouvee_apres_un_redemarrage() {
    // Le scénario du premier critère d'acceptation, réduit à ce qui en est testable : le démon
    // écrit l'état appliqué, un autre démarrage le relit.
    let dossier = DossierJetable::neuf("couleur_fixe_retrouvee");
    let chemin = dossier.fichier("eclairage.conf");
    let pose = eclairage_temoin();

    enregistrer_eclairage(&chemin, &pose).expect("l'enregistrement doit réussir");
    let (relu, signalement) = charger_eclairage(&chemin);

    assert_eq!(
        relu, pose,
        "La couleur posée par `light` doit être retrouvée telle quelle au démarrage suivant."
    );
    assert_eq!(
        signalement, None,
        "Un fichier que le démon vient d'écrire doit se relire sans un mot. Obtenu : \
         {signalement:?}"
    );
}

#[test]
fn une_animation_en_cours_reprend_apres_un_redemarrage() {
    let dossier = DossierJetable::neuf("animation_reprise");
    let chemin = dossier.fichier("eclairage.conf");
    let pose = eclairage_temoin_anime();

    enregistrer_eclairage(&chemin, &pose).expect("l'enregistrement doit réussir");
    let (relu, signalement) = charger_eclairage(&chemin);

    assert_eq!(
        relu, pose,
        "« Une animation en cours, réglages compris, reprend après un redémarrage. »"
    );
    assert_eq!(signalement, None, "Obtenu : {signalement:?}");
}

#[test]
fn eteindre_volontairement_se_retrouve_eteint() {
    // Décision de cadrage : « on persiste le dernier état appliqué », « écriture à chaque
    // changement d'état ». Le second enregistrement doit donc effacer le premier — et non le
    // compléter, ni être ignoré parce que « le noir, ce n'est rien ».
    let dossier = DossierJetable::neuf("extinction_volontaire");
    let chemin = dossier.fichier("eclairage.conf");

    enregistrer_eclairage(&chemin, &eclairage_temoin_anime()).expect("premier état");
    enregistrer_eclairage(&chemin, &eclairage_noir())
        .expect("second état : `animate off` puis noir");

    let (relu, _) = charger_eclairage(&chemin);
    assert_eq!(
        relu,
        eclairage_noir(),
        "On retrouve le noir, on ne retrouve pas la couleur d'avant."
    );
    assert_eq!(
        relu.animation, None,
        "`animate off` a bien eu lieu : aucune animation ne doit repartir au démarrage suivant."
    );
    assert_ne!(
        relu,
        Eclairage::accueil(),
        "Et surtout pas l'accueil : éteindre son boîtier ne doit pas le rallumer en bleu."
    );
}

#[test]
fn light_sur_un_seul_ventilateur_ne_ressort_pas_applique_a_tous() {
    // Test d'intention n° 5 de l'issue, joué de bout en bout : `light fan:arriere` sur une
    // installation neuve. Le mode de défaillance est silencieux et spectaculaire — dix
    // ventilateurs dans la teinte d'un seul, sans une erreur.
    let dossier = DossierJetable::neuf("light_un_seul_ventilateur");
    let chemin = dossier.fichier("eclairage.conf");

    let choisi = Position::Arriere;
    let peinte = Rgb::new(0xff, 0x7f, 0x00);
    let mut pose = Eclairage::accueil();
    pose.ventilateurs[choisi.index()] = peinte;

    enregistrer_eclairage(&chemin, &pose).expect("l'enregistrement doit réussir");
    let (relu, _) = charger_eclairage(&chemin);

    assert_eq!(
        relu.ventilateurs[choisi.index()],
        peinte,
        "Le ventilateur « {} » doit retrouver la couleur qu'on lui a posée.",
        choisi.name()
    );
    for position in Position::ALL.into_iter().filter(|p| *p != choisi) {
        assert_eq!(
            relu.ventilateurs[position.index()],
            ACCUEIL,
            "Le ventilateur « {} » n'a jamais été peint : il doit rester en accueil, pas prendre \
             la couleur de « {} ».",
            position.name(),
            choisi.name()
        );
    }
    for slot in 0..ram::SLOT_COUNT {
        assert_eq!(
            relu.barrettes[slot], ACCUEIL,
            "La barrette {slot} n'a pas été peinte non plus : peindre un ventilateur ne doit pas \
             déborder sur la RAM."
        );
    }
}

#[test]
fn le_dossier_parent_est_cree_s_il_manque() {
    // `StateDirectory=reverb` crée le dossier sous systemd, mais pas pour un démon lancé à la main,
    // ni au tout premier enregistrement d'une installation neuve. Échouer là ferait perdre l'état
    // sans que la commande, elle, ait échoué.
    let dossier = DossierJetable::neuf("dossier_parent");
    let chemin = dossier
        .fichier("pas-encore")
        .join("la-non-plus")
        .join("eclairage.conf");

    enregistrer_eclairage(&chemin, &eclairage_temoin())
        .unwrap_or_else(|erreur| panic!("le dossier parent doit être créé au besoin : {erreur}"));
    assert!(
        chemin.is_file(),
        "{} doit exister après l'enregistrement.",
        chemin.display()
    );

    let (relu, signalement) = charger_eclairage(&chemin);
    assert_eq!(relu, eclairage_temoin(), "et se relire.");
    assert_eq!(signalement, None, "Obtenu : {signalement:?}");
}

#[test]
fn aucun_fichier_temporaire_ne_traine_apres_l_enregistrement() {
    // L'écriture passe par un provisoire puis un renommage — c'est ce qui protège l'état d'une
    // coupure de courant, et l'écriture est fréquente puisqu'elle a lieu à chaque commande. Le
    // provisoire ne doit pas survivre : à raison d'un par commande, ce serait un fichier de plus
    // dans `/var/lib/reverb` à chaque changement de couleur, et un candidat à la relecture.
    let dossier = DossierJetable::neuf("sans_fichier_temporaire");
    let chemin = dossier.fichier("eclairage.conf");

    enregistrer_eclairage(&chemin, &eclairage_temoin()).expect("premier enregistrement");
    enregistrer_eclairage(&chemin, &eclairage_temoin_anime()).expect("second enregistrement");

    let mut restants: Vec<String> = fs::read_dir(dossier.chemin())
        .expect("lecture du dossier de test")
        .map(|entree| {
            entree
                .expect("entrée de dossier lisible")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    restants.sort();

    assert_eq!(
        restants,
        vec![nom_de_fichier(&chemin)],
        "Le dossier ne doit contenir que le fichier d'éclairage après l'enregistrement."
    );
}

#[test]
fn le_fichier_enregistre_porte_un_en_tete_et_se_relit() {
    // L'en-tête est là pour qui ouvre le fichier sans savoir ce que c'est. Il ne doit pas empêcher
    // le démon de relire ce qu'il vient d'écrire : c'est la seule chose qui compte vraiment ici.
    let dossier = DossierJetable::neuf("en_tete");
    let chemin = dossier.fichier("eclairage.conf");
    let pose = eclairage_temoin_anime();

    enregistrer_eclairage(&chemin, &pose).expect("l'enregistrement doit réussir");
    let contenu = fs::read_to_string(&chemin).expect("relecture du fichier écrit");

    assert!(
        contenu.lines().any(|ligne| ligne.starts_with('#')),
        "Le fichier écrit doit porter un en-tête en commentaire. Contenu :\n{contenu}"
    );
    let (relu, signalement) = charger_eclairage(&chemin);
    assert_eq!(
        relu, pose,
        "En-tête compris, le fichier écrit doit rendre l'état d'origine."
    );
    assert_eq!(
        signalement, None,
        "Son propre en-tête ne doit pas faire signaler un fichier au démon. Obtenu : \
         {signalement:?}"
    );
}
