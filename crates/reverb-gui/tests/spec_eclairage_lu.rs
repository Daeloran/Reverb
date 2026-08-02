//! Tests d'intention de l'éclairage que la fenêtre lit au démarrage (issue #41).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #41 et le contrat déposé dans
//! `crates/reverb-gui/src/reglages.rs` — dont les corps de [`eclairage_lu`] et de
//! [`Reglage::adopter`] sont `todo!()` à l'écriture de ce fichier, et dont **rien d'autre n'a été
//! lu**. Aucun test ici n'ouvre de socket, ne démarre de démon ni ne touche un bus : une réponse
//! `lighting` est une tranche de [`ResponseLine`], et c'est tout ce dont ces deux fonctions ont
//! besoin.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Le démon rétablit l'éclairage au démarrage depuis `/var/lib/reverb/eclairage.conf` : quand la
//! fenêtre s'ouvre, une animation tourne **le plus souvent déjà**. La fenêtre ne le demandait
//! jamais, donc elle croyait piloter un boîtier éteint : `Reglage::commande()` rendait `None` et le
//! curseur de vitesse redevenait muet — exactement le défaut réparé par #32, revenu par une autre
//! porte.
//!
//! Le second défaut est plus discret et se voit une fois le premier réparé : adopter le **nom** de
//! l'animation sans adopter ses réglages ferait partir, au premier mouvement du curseur de vitesse,
//! un `animate` portant la couleur et la direction du sélecteur de la fenêtre. Régler la vitesse
//! changerait la couleur du boîtier. D'où le test n° 15, qui compose la chaîne entière.
//!
//! ## Ce que le contrat laisse ouvert, et que ces tests tranchent
//!
//! 1. **Une clé répétée dont la dernière occurrence est illisible ne vaut rien.** Le contrat pose
//!    deux règles qui se croisent ici : « la dernière valeur l'emporte » et « une valeur qu'on ne
//!    sait pas relire est ignorée ». Composées dans cet ordre — prendre la dernière, puis la
//!    valider — `vitesse=8 vitesse=99` ne rapporte aucune vitesse. L'ordre inverse — garder la
//!    dernière valeur *valide* — rapporterait `8`, c'est-à-dire une valeur que le démon n'a pas
//!    écrite en dernier. C'est la lecture littérale qui est retenue, et c'est aussi celle qu'une
//!    boucle naïve produit.
//! 2. **Plusieurs lignes `anim` ne se mélangent pas.** « La dernière l'emporte » porte sur la ligne
//!    entière, réglages compris : une couleur venue de la ligne d'avant décrirait un éclairage qui
//!    n'a jamais existé.
//! 3. **Le fond de la vitesse est celui du catalogue, pas celui du type.** `1` à `10`, bornes
//!    comprises. Le test n° 7 ne se contente pas de les écrire : il les fait confirmer par
//!    [`Animation::reglages`], seul juge du côté du démon, pour qu'un déplacement des bornes
//!    fasse tomber ce fichier au lieu de le laisser mentir.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **La casse d'un slug de direction.** Le contrat accepte les majuscules pour la couleur
//!   (« Majuscules acceptées ») et ne dit rien pour la direction. Exiger ici que `BAS-HAUT` soit
//!   refusé — ou accepté — inventerait une règle que personne n'a choisie.
//! - **Les formes que `u8::from_str` tolère au-delà du contrat** : `+5`, `05`. Le contrat dit « un
//!   entier décimal de 1 à 10 » sans se prononcer sur leur écriture, et le démon n'écrit jamais ni
//!   l'un ni l'autre.
//! - **Ce que devient une animation inconnue du catalogue une fois adoptée.** `eclairage_lu` doit
//!   la rapporter telle quelle (critère n° 2), et le contrat de `commande` — déjà écrit, déjà
//!   couvert par `spec_reglages.rs` — ne dit pas ce qu'elle en fait. Ce fichier s'arrête donc à ce
//!   que le démon a dit.

use reverb_anim::{Animation, CATALOGUE, Direction, Reglages};
use reverb_gui::reglages::{Eclairage, Reglage, eclairage_lu};
use reverb_proto::ipc::{Request, ResponseLine};
use reverb_proto::{Position, Rgb};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// Ce que le démon rapporte : la couleur de l'animation qui tourne déjà.
const COULEUR_LUE: Rgb = Rgb::new(0x12, 0x9a, 0x40);
/// Ce que la fenêtre affiche avant d'avoir rien lu — son sélecteur au repos.
const COULEUR_FENETRE: Rgb = Rgb::new(0x33, 0x66, 0x99);
/// La couleur d'une ligne `anim` que la suivante doit faire oublier.
const COULEUR_AUTRE: Rgb = Rgb::new(0xab, 0xcd, 0xef);
/// La couleur portée par les quatorze lignes `light` d'une réponse `lighting`.
///
/// Différente de toutes les autres : une implémentation qui prendrait la couleur d'une ligne
/// `light` — la couleur fixe, celle que `animate off` rendrait — au lieu de celle de la ligne
/// `anim` se ferait attraper ici plutôt qu'à l'œil, sur un boîtier presque de la bonne teinte.
const COULEUR_FIXE: Rgb = Rgb::new(0xde, 0xad, 0xbe);

/// La vitesse rapportée par le démon, celle qu'affiche la fenêtre avant lecture, celle d'une ligne
/// `anim` périmée, et celle où l'utilisateur tire le curseur après adoption.
///
/// Quatre valeurs distinctes, toutes dans les bornes, **et aucune n'est le défaut du catalogue** —
/// sans quoi un réglage perdu en route se confondrait avec un réglage bien lu.
const VITESSE_LUE: u8 = 8;
const VITESSE_FENETRE: u8 = 4;
const VITESSE_AUTRE: u8 = 2;
const VITESSE_TIREE: u8 = 10;

/// Les directions correspondantes.
///
/// [`Direction::BasHaut`] est le **rang zéro** de [`Direction::ALL`] : c'est ce qu'une
/// implémentation qui perdrait le rang rendrait par accident. Il sert donc de valeur *périmée*,
/// jamais de valeur attendue.
const DIRECTION_LUE: Direction = Direction::AvantArriere;
const DIRECTION_FENETRE: Direction = Direction::Antihoraire;
const DIRECTION_AUTRE: Direction = Direction::BasHaut;

/// L'animation d'exemple : elle accepte les trois réglages.
const ANIM_LUE: &str = "comete";
/// Une seconde, pour les changements de nom.
const ANIM_AUTRE: &str = "braise";
/// Celle qui produit ses teintes et ne rapporte donc aucune couleur.
const ANIM_SANS_COULEUR: &str = "arc-en-ciel";
/// Un nom que le catalogue ne connaît pas — un démon plus récent que la fenêtre.
const ANIM_INCONNUE: &str = "tourbillon";

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

/// Le rang d'une direction dans [`Direction::ALL`], tel que [`Eclairage::direction`] le porte.
fn rang(direction: Direction) -> usize {
    Direction::ALL
        .into_iter()
        .position(|d| d == direction)
        .unwrap_or_else(|| panic!("{direction:?} est une des six directions"))
}

/// Une couleur telle que le démon l'écrit : six chiffres hexadécimaux, sans `#`.
fn hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

/// Une ligne `anim` portant ces paires, dans cet ordre.
fn ligne_anim(nom: &str, reglages: &[(&str, &str)]) -> ResponseLine {
    ResponseLine::Anim {
        nom: nom.to_owned(),
        reglages: reglages
            .iter()
            .map(|(cle, valeur)| ((*cle).to_owned(), (*valeur).to_owned()))
            .collect(),
    }
}

/// Une ligne `anim` telle que **le démon** l'écrit pour cette animation et ces réglages.
///
/// Passe par [`Animation::reglages_ecrits`], qui est l'émetteur réel de l'autre côté du socket :
/// c'est ce qui rend ces tests sensibles au **format** — `129a40` et non `#129a40`, `8` et non
/// `vitesse 8`, `avant-arriere` et non `AvantArriere` — sans le recopier à la main.
fn ligne_du_demon(nom: &str, reglages: &Reglages) -> ResponseLine {
    let animation = animation(nom);
    ResponseLine::Anim {
        nom: nom.to_owned(),
        reglages: animation.reglages_ecrits(reglages),
    }
}

/// L'animation du catalogue portant ce nom, ou un échec qui le dit.
fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|e| panic!("« {nom} » est au catalogue : {e}"))
}

/// Une réponse `lighting` complète : les quatorze couleurs fixes, les lignes `anim` données, `end`.
///
/// Le cadre est celui du protocole (`Request::Lighting` : « quatorze lignes `light` […] plus une
/// ligne `anim` s'il y a une animation, puis `end` »). Il est reconstitué ici parce que
/// `eclairage_lu` reçoit la réponse **entière** : elle doit y trouver l'animation sans se laisser
/// distraire par les treize lignes qui la précèdent ni par celle qui la termine.
fn reponse(lignes_anim: &[ResponseLine]) -> Vec<ResponseLine> {
    let mut lignes: Vec<ResponseLine> = Position::ALL
        .into_iter()
        .map(|position| ResponseLine::Light {
            cible: format!("fan:{}", position.slug()),
            couleur: COULEUR_FIXE,
        })
        .chain((0..4).map(|slot| ResponseLine::Light {
            cible: format!("slot:{slot}"),
            couleur: COULEUR_FIXE,
        }))
        .collect();
    lignes.extend(lignes_anim.iter().cloned());
    lignes.push(ResponseLine::End);
    lignes
}

/// Les réglages tels que la fenêtre les affiche.
fn reglage(animation: Option<&str>, couleur: Rgb, vitesse: u8, direction: Direction) -> Reglage {
    Reglage {
        animation: animation.map(str::to_owned),
        couleur,
        vitesse,
        direction: rang(direction),
    }
}

/// Ce que la fenêtre affiche avant d'avoir lu quoi que ce soit : aucune animation, ses propres
/// réglages. C'est l'état exact de #41 au démarrage.
fn fenetre_neuve() -> Reglage {
    reglage(None, COULEUR_FENETRE, VITESSE_FENETRE, DIRECTION_FENETRE)
}

/// Un `Eclairage` construit à la main, sans passer par une réponse.
fn venu(
    animation: Option<&str>,
    couleur: Option<Rgb>,
    vitesse: Option<u8>,
    direction: Option<Direction>,
) -> Eclairage {
    Eclairage {
        animation: animation.map(str::to_owned),
        couleur,
        vitesse,
        direction: direction.map(rang),
    }
}

/// Le nom et les paires portés par la commande d'un réglage, ou un échec qui dit ce qui est venu.
fn animate(reglage: &Reglage) -> (String, Vec<(String, String)>) {
    match reglage.commande() {
        Some(Request::Animate {
            name: Some(nom),
            reglages,
        }) => (nom, reglages),
        autre => panic!(
            "après adoption, bouger un curseur doit relancer l'animation que le démon a rapportée : \
             {autre:?} pour {reglage:?}"
        ),
    }
}

/// Ce que le démon lira de ces paires — le juge de l'autre côté du socket.
fn relu(nom: &str, paires: &[(String, String)]) -> Reglages {
    animation(nom)
        .reglages(paires)
        .unwrap_or_else(|e| panic!("le démon doit accepter {paires:?} pour « {nom} » : {e}"))
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Garde-fou, pas critère. Deux confusions rendraient tous les tests qui suivent complaisants :
    // une valeur « lue » égale à ce que la fenêtre affichait déjà — l'adoption ne se verrait pas —
    // et une valeur « lue » égale au défaut du catalogue, que le démon comble tout seul quand une
    // clé manque. Ce test est la condition de validité des autres.
    let defaut = Reglages::default();
    for (nom, couleur) in [
        ("lue", COULEUR_LUE),
        ("de la fenêtre", COULEUR_FENETRE),
        ("périmée", COULEUR_AUTRE),
        ("fixe", COULEUR_FIXE),
    ] {
        assert_ne!(
            couleur, defaut.couleur,
            "la couleur {nom} doit différer du défaut du catalogue"
        );
        assert_ne!(
            couleur,
            Rgb::BLACK,
            "la couleur {nom} doit différer du noir, valeur de repli la plus probable"
        );
    }
    let couleurs = [COULEUR_LUE, COULEUR_FENETRE, COULEUR_AUTRE, COULEUR_FIXE];
    for (i, couleur) in couleurs.iter().enumerate() {
        assert!(
            !couleurs[i + 1..].contains(couleur),
            "les quatre couleurs de ce fichier doivent être distinctes : {couleur:?} revient"
        );
    }

    let vitesses = [VITESSE_LUE, VITESSE_FENETRE, VITESSE_AUTRE, VITESSE_TIREE];
    for (i, vitesse) in vitesses.iter().enumerate() {
        assert_ne!(
            *vitesse, defaut.vitesse,
            "la vitesse {vitesse} doit différer du défaut du catalogue {}",
            defaut.vitesse
        );
        assert_ne!(*vitesse, 0, "zéro est hors bornes, il ne peut être attendu");
        assert!(
            !vitesses[i + 1..].contains(vitesse),
            "les quatre vitesses de ce fichier doivent être distinctes : {vitesse} revient"
        );
    }

    let directions = [DIRECTION_LUE, DIRECTION_FENETRE, DIRECTION_AUTRE];
    for (i, direction) in directions.iter().enumerate() {
        assert!(
            !directions[i + 1..].contains(direction),
            "les trois directions de ce fichier doivent être distinctes : {direction:?} revient"
        );
    }
    assert_ne!(
        DIRECTION_LUE, defaut.direction,
        "la direction lue doit différer du défaut du catalogue"
    );
    assert_ne!(
        rang(DIRECTION_LUE),
        0,
        "la direction lue ne doit pas être le rang zéro, qu'un rang perdu rendrait par accident"
    );
    assert_ne!(rang(DIRECTION_FENETRE), 0, "celle de la fenêtre non plus");

    assert!(
        Animation::par_nom(ANIM_INCONNUE).is_err(),
        "« {ANIM_INCONNUE} » doit rester hors du catalogue, sinon le critère n° 2 ne teste rien"
    );
    assert!(
        !animation(ANIM_SANS_COULEUR)
            .parametres_acceptes()
            .contains(&"couleur"),
        "« {ANIM_SANS_COULEUR} » produit ses teintes : le démon n'écrit pas sa couleur"
    );
    assert!(
        animation(ANIM_LUE)
            .parametres_acceptes()
            .contains(&"couleur"),
        "« {ANIM_LUE} » se colore : le démon écrit sa couleur"
    );
}

// ---------------------------------------------------------------------------
// 1 — pas de ligne « anim » : rien ne tourne, et c'est une information
// ---------------------------------------------------------------------------

#[test]
fn une_reponse_sans_ligne_anim_dit_qu_aucune_animation_ne_tourne() {
    // Critère d'acceptation n° 1 : « aucune ligne `Anim` → `Eclairage::default()` : `animation`
    // vaut `None`, et les trois réglages aussi. C'est une information (« rien ne tourne »), pas une
    // absence d'information. »
    //
    // Deux fautes symétriques sont visées. Une implémentation qui inventerait des réglages ici
    // les ferait adopter au premier changement de nom, et la fenêtre repartirait sur une couleur
    // que personne n'a choisie. Et une implémentation qui irait chercher la couleur dans les lignes
    // `light` — elles y sont, quatorze fois — rapporterait la couleur fixe du boîtier comme si
    // c'était celle d'une animation qui ne tourne pas.
    assert_eq!(
        Eclairage::default(),
        venu(None, None, None, None),
        "le repos d'un `Eclairage`, c'est quatre `None` — c'est ce que le critère n° 1 exige"
    );

    for (cas, lignes) in [
        ("une réponse vide", Vec::new()),
        ("une réponse `lighting` sans animation", reponse(&[])),
        ("les quatorze couleurs fixes sans terminateur", {
            let mut sans_fin = reponse(&[]);
            sans_fin.pop();
            sans_fin
        }),
        ("un `end` seul", vec![ResponseLine::End]),
    ] {
        let lu = eclairage_lu(&lignes);
        assert_eq!(
            lu,
            Eclairage::default(),
            "{cas} : aucune animation ne tourne, et aucun réglage n'est rapporté — {lu:?}"
        );
        assert_eq!(
            lu.animation, None,
            "{cas} : la fenêtre doit repasser sur « aucune »"
        );
        assert_eq!(
            (lu.couleur, lu.vitesse, lu.direction),
            (None, None, None),
            "{cas} : la couleur fixe des lignes `light` n'est pas la couleur d'une animation"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — le nom vient du démon, tel quel
// ---------------------------------------------------------------------------

#[test]
fn le_nom_de_l_animation_est_celui_que_le_demon_a_ecrit_meme_hors_catalogue() {
    // Critère d'acceptation n° 2 : « une ligne `Anim` → `animation` vaut `Some(nom)`, tel que le
    // démon l'a écrit, même si ce nom est inconnu du catalogue […] : la fenêtre montre ce que le
    // démon dit plutôt qu'un « aucune » qui serait faux. »
    //
    // La faute visée est un filtrage bien intentionné : passer le nom par `Animation::par_nom` et
    // laisser tomber ce qui ne s'y trouve pas. Le symptôme serait celui même de #41 — une fenêtre
    // qui croit que rien ne tourne — mais seulement contre un démon plus récent qu'elle, donc
    // seulement après une mise à jour partielle, donc jamais sur la machine de développement.
    let lu = eclairage_lu(&reponse(&[ligne_anim(ANIM_LUE, &[])]));
    assert_eq!(
        lu.animation.as_deref(),
        Some(ANIM_LUE),
        "une ligne `anim` nomme l'animation en cours — {lu:?}"
    );
    assert_eq!(
        (lu.couleur, lu.vitesse, lu.direction),
        (None, None, None),
        "cette ligne ne porte aucune paire : aucun réglage n'est rapporté — {lu:?}"
    );

    let inconnue = eclairage_lu(&reponse(&[ligne_anim(
        ANIM_INCONNUE,
        &[
            ("couleur", &hexa(COULEUR_LUE)),
            ("vitesse", &VITESSE_LUE.to_string()),
            ("direction", DIRECTION_LUE.slug()),
        ],
    )]));
    assert_eq!(
        inconnue.animation.as_deref(),
        Some(ANIM_INCONNUE),
        "« {ANIM_INCONNUE} » n'est pas au catalogue, et c'est pourtant ce qui tourne : la fenêtre \
         montre ce que le démon dit — {inconnue:?}"
    );
    assert_eq!(
        (inconnue.couleur, inconnue.vitesse, inconnue.direction),
        (
            Some(COULEUR_LUE),
            Some(VITESSE_LUE),
            Some(rang(DIRECTION_LUE))
        ),
        "et ses réglages se relisent comme les autres : ils ne dépendent pas du catalogue — \
         {inconnue:?}"
    );

    // Sans les lignes `light` autour, ni le `end` : le cadre de la réponse ne change rien.
    let nue = eclairage_lu(&[ligne_anim(ANIM_AUTRE, &[])]);
    assert_eq!(
        nue.animation.as_deref(),
        Some(ANIM_AUTRE),
        "une ligne `anim` seule se lit aussi bien — {nue:?}"
    );
}

// ---------------------------------------------------------------------------
// 3 — les trois réglages se relisent depuis les paires
// ---------------------------------------------------------------------------

#[test]
fn les_trois_reglages_se_relisent_depuis_les_paires_de_la_ligne_anim() {
    // Critère d'acceptation n° 3 : « les réglages sont lus depuis les paires (clé, valeur) ».
    //
    // Les trois ensemble, dans une seule ligne : une implémentation qui lirait la bonne clé dans la
    // mauvaise case — la vitesse rangée dans la direction, deux entiers qui se ressemblent — passe
    // n'importe quel test qui ne vérifie qu'un réglage à la fois.
    let lu = eclairage_lu(&reponse(&[ligne_anim(
        ANIM_LUE,
        &[
            ("couleur", &hexa(COULEUR_LUE)),
            ("vitesse", &VITESSE_LUE.to_string()),
            ("direction", DIRECTION_LUE.slug()),
        ],
    )]));

    assert_eq!(
        lu,
        venu(
            Some(ANIM_LUE),
            Some(COULEUR_LUE),
            Some(VITESSE_LUE),
            Some(DIRECTION_LUE)
        ),
        "les trois paires d'une ligne `anim` remplissent les trois réglages — {lu:?}"
    );

    // L'ordre des paires est celui de la frappe côté protocole, et il n'a pas à compter ici.
    let inverse = eclairage_lu(&reponse(&[ligne_anim(
        ANIM_LUE,
        &[
            ("direction", DIRECTION_LUE.slug()),
            ("vitesse", &VITESSE_LUE.to_string()),
            ("couleur", &hexa(COULEUR_LUE)),
        ],
    )]));
    assert_eq!(
        inverse, lu,
        "les mêmes paires dans l'autre ordre disent le même éclairage — {inverse:?}"
    );
}

// ---------------------------------------------------------------------------
// 4 — la couleur : six chiffres hexadécimaux, et rien d'autre
// ---------------------------------------------------------------------------

#[test]
fn la_couleur_se_lit_en_six_chiffres_hexadecimaux_et_toute_autre_forme_est_ignoree() {
    // Critère d'acceptation n° 3 : « couleur : exactement six chiffres hexadécimaux, sans `#`, tels
    // que le démon les écrit. Majuscules acceptées. Toute autre forme (`#ff00ff`, `ff00`,
    // `ff00ff00`, `zzzzzz`, vide) → la couleur est ignorée, c'est-à-dire `None`. »
    //
    // La faute la plus probable est un appel direct à `Rgb::from_hex`, qui **tolère le `#`** parce
    // qu'un humain le tape. Le démon, lui, est un programme : accepter deux écritures d'une même
    // couleur, c'est accepter que le fichier d'état en porte une que l'aller-retour ne rend plus.
    // Elle n'a aucun symptôme visible tant que personne n'écrit `#` — donc jusqu'au jour où.
    for accepte in [
        hexa(COULEUR_LUE),
        hexa(COULEUR_LUE).to_uppercase(),
        String::from("129A40").to_lowercase(),
        String::from("129a40"),
    ] {
        let lu = eclairage_lu(&[ligne_anim(ANIM_LUE, &[("couleur", &accepte)])]);
        assert_eq!(
            lu.couleur,
            Some(COULEUR_LUE),
            "« {accepte} » est la couleur {COULEUR_LUE:?} écrite en six chiffres hexadécimaux"
        );
    }
    // Les majuscules, sur une couleur dont les six chiffres ne sont pas tous des lettres.
    let majuscules = eclairage_lu(&[ligne_anim(ANIM_LUE, &[("couleur", "129A40")])]);
    assert_eq!(
        majuscules.couleur,
        Some(COULEUR_LUE),
        "« 129A40 » vaut « 129a40 » : les majuscules sont acceptées — {majuscules:?}"
    );

    for refuse in [
        "#129a40",  // le dièse d'un humain, que `Rgb::from_hex` tolère et que le démon n'écrit pas
        "129a4",    // cinq chiffres
        "129a400",  // sept
        "129a40ff", // huit — une couleur avec alpha
        "129a",     // quatre
        "129",      // trois — l'abrégé des feuilles de style, que le démon n'écrit jamais
        "1a",       // deux
        "",         // une variable de shell non substituée
        "zzzzzz",   // six caractères, aucun hexadécimal
        "129a4g",   // un seul caractère de trop
        "0x129a40", // le préfixe d'un littéral
        "129a40 ",  // un blanc de trop
    ] {
        let lu = eclairage_lu(&[ligne_anim(
            ANIM_LUE,
            &[
                ("couleur", refuse),
                ("vitesse", &VITESSE_LUE.to_string()),
                ("direction", DIRECTION_LUE.slug()),
            ],
        )]);
        assert_eq!(
            lu.couleur, None,
            "« {refuse} » n'est pas six chiffres hexadécimaux : la couleur est ignorée, jamais \
             remplacée — {lu:?}"
        );
        assert_eq!(
            (lu.animation.as_deref(), lu.vitesse, lu.direction),
            (Some(ANIM_LUE), Some(VITESSE_LUE), Some(rang(DIRECTION_LUE))),
            "une couleur illisible n'emporte ni le nom ni les deux autres réglages — {lu:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — la vitesse : les bornes du catalogue, en décimal
// ---------------------------------------------------------------------------

#[test]
fn la_vitesse_se_lit_de_un_a_dix_et_toute_autre_valeur_est_ignoree() {
    // Critère d'acceptation n° 3 : « vitesse : un entier décimal de 1 à 10 (les bornes du
    // catalogue). `0`, `11`, `300`, `-1`, `3.5`, `trois`, vide → ignorée, `None`. »
    //
    // Les bornes ne sont pas écrites à la main : elles sont confirmées par `Animation::reglages`,
    // qui est le juge de l'autre côté du socket. Une vitesse acceptée ici que le démon refuserait
    // là-bas ferait rejeter la commande **entière** au premier mouvement du curseur — le défaut de
    // #32 sous un autre nom.
    let comete = animation(ANIM_LUE);
    let jugee = |valeur: &str| {
        comete
            .reglages(&[(String::from("vitesse"), valeur.to_owned())])
            .is_ok()
    };
    for hors in ["0", "11"] {
        assert!(
            !jugee(hors),
            "le catalogue refuse la vitesse {hors} : c'est de lui que ce test tient ses bornes"
        );
    }

    for vitesse in 1..=10u8 {
        assert!(
            jugee(&vitesse.to_string()),
            "le catalogue accepte la vitesse {vitesse}"
        );
        let lu = eclairage_lu(&[ligne_anim(ANIM_LUE, &[("vitesse", &vitesse.to_string())])]);
        assert_eq!(
            lu.vitesse,
            Some(vitesse),
            "« {vitesse} » se relit en {vitesse}, bornes comprises — {lu:?}"
        );
    }

    for refuse in [
        "0", "11", "12", "300", "255", "256", "-1", "3.5", "trois", "",
    ] {
        let lu = eclairage_lu(&[ligne_anim(
            ANIM_LUE,
            &[
                ("couleur", &hexa(COULEUR_LUE)),
                ("vitesse", refuse),
                ("direction", DIRECTION_LUE.slug()),
            ],
        )]);
        assert_eq!(
            lu.vitesse, None,
            "« {refuse} » n'est pas une vitesse du catalogue : elle est ignorée — {lu:?}"
        );
        assert_eq!(
            (lu.animation.as_deref(), lu.couleur, lu.direction),
            (Some(ANIM_LUE), Some(COULEUR_LUE), Some(rang(DIRECTION_LUE))),
            "une vitesse illisible n'emporte ni le nom ni les deux autres réglages — {lu:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — la direction : un slug, rendu en rang
// ---------------------------------------------------------------------------

#[test]
fn la_direction_se_lit_par_son_slug_et_se_rend_en_rang() {
    // Critère d'acceptation n° 3 : « direction : un slug de `reverb_anim::Direction` […] rendu
    // comme le rang dans `Direction::ALL`. Un slug inconnu → ignoré, `None`. »
    //
    // Le piège est la conversion elle-même : la direction voyage en **mot** sur le socket et en
    // **rang** dans la fenêtre. Une table tronquée, ou un rang perdu, rend zéro — `bas-haut` — pour
    // tout le monde, ce qui ressemble à une animation qui marche. D'où le balayage des six, et non
    // un cas.
    for (attendu, direction) in Direction::ALL.into_iter().enumerate() {
        let lu = eclairage_lu(&[ligne_anim(ANIM_LUE, &[("direction", direction.slug())])]);
        assert_eq!(
            lu.direction,
            Some(attendu),
            "« {} » est le rang {attendu} de `Direction::ALL` — {lu:?}",
            direction.slug()
        );
    }

    for refuse in [
        "gauche-droite", // la direction qui n'existe pas : aucun axe ne la porte
        "bas_haut",      // le souligné au lieu du tiret
        "BasHaut",       // le nom de la variante Rust
        "avant",
        "",
        "0",
        "42",
    ] {
        let lu = eclairage_lu(&[ligne_anim(
            ANIM_LUE,
            &[
                ("couleur", &hexa(COULEUR_LUE)),
                ("vitesse", &VITESSE_LUE.to_string()),
                ("direction", refuse),
            ],
        )]);
        assert_eq!(
            lu.direction, None,
            "« {refuse} » n'est aucune des six directions : elle est ignorée — {lu:?}"
        );
        assert_eq!(
            (lu.animation.as_deref(), lu.couleur, lu.vitesse),
            (Some(ANIM_LUE), Some(COULEUR_LUE), Some(VITESSE_LUE)),
            "une direction illisible n'emporte ni le nom ni les deux autres réglages — {lu:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 7 — un réglage illisible n'est jamais remplacé par un repli
// ---------------------------------------------------------------------------

#[test]
fn un_reglage_illisible_n_est_jamais_remplace_par_une_valeur_de_repli() {
    // Critère d'acceptation n° 3, dernier point : « un réglage qu'on ne sait pas relire n'est jamais
    // remplacé par une valeur de repli : une vitesse de repli tirerait le curseur ailleurs que là où
    // le boîtier tourne vraiment, sans rien dire. »
    //
    // C'est la faute la plus séduisante du lot — `unwrap_or_default()` est plus court que le `?` qui
    // convient — et la seule qui **ajoute** de l'information au lieu d'en perdre. Un repli sur le
    // défaut du catalogue est indiscernable d'une lecture réussie : la fenêtre montre une vitesse
    // plausible, l'adopte, et le premier mouvement du curseur l'envoie au démon. Les deux replis
    // qu'on écrirait sans y penser sont épinglés nommément.
    let illisible = eclairage_lu(&reponse(&[ligne_anim(
        ANIM_LUE,
        &[
            ("couleur", "pas-une-couleur"),
            ("vitesse", "99"),
            ("direction", "nulle-part"),
        ],
    )]));

    assert_eq!(
        illisible,
        venu(Some(ANIM_LUE), None, None, None),
        "trois réglages illisibles ne sont pas trois réglages : seul le nom est rapporté — \
         {illisible:?}"
    );

    let defaut = Reglages::default();
    assert_ne!(
        illisible.couleur,
        Some(defaut.couleur),
        "le défaut du catalogue n'est pas une lecture"
    );
    assert_ne!(
        illisible.vitesse,
        Some(defaut.vitesse),
        "le défaut du catalogue n'est pas une lecture"
    );
    assert_ne!(
        illisible.direction,
        Some(rang(defaut.direction)),
        "le défaut du catalogue n'est pas une lecture"
    );
    assert_ne!(
        illisible.couleur,
        Some(Rgb::BLACK),
        "le noir n'est pas une lecture"
    );
    assert_ne!(illisible.vitesse, Some(0), "zéro n'est pas une lecture");
    assert_ne!(
        illisible.direction,
        Some(0),
        "le rang zéro n'est pas une lecture"
    );
}

// ---------------------------------------------------------------------------
// 8 — une clé inconnue ne fait de tort à personne
// ---------------------------------------------------------------------------

#[test]
fn une_cle_inconnue_est_ignoree_sans_que_les_autres_en_patissent() {
    // Critère d'acceptation n° 3 : « une clé inconnue est ignorée sans que les autres en
    // pâtissent. »
    //
    // C'est la compatibilité avec un démon plus récent que la fenêtre : le jour où une animation
    // acceptera un septième paramètre, la fenêtre installée doit continuer d'afficher les trois
    // qu'elle connaît, pas se taire. La faute visée est un décodeur qui refuserait la ligne entière
    // — le refus d'une clé inconnue appartient au démon, qui seul sait ce qu'une animation accepte.
    let lu = eclairage_lu(&reponse(&[ligne_anim(
        ANIM_LUE,
        &[
            ("luminosite", "50"),
            ("couleur", &hexa(COULEUR_LUE)),
            ("bidule", ""),
            ("vitesse", &VITESSE_LUE.to_string()),
            ("phase", "0.5"),
            ("direction", DIRECTION_LUE.slug()),
            ("nom", ANIM_AUTRE),
        ],
    )]));
    assert_eq!(
        lu,
        venu(
            Some(ANIM_LUE),
            Some(COULEUR_LUE),
            Some(VITESSE_LUE),
            Some(DIRECTION_LUE)
        ),
        "quatre clés inconnues n'en emportent aucune des trois connues — {lu:?}"
    );
}

// ---------------------------------------------------------------------------
// 9 — la dernière ligne « anim » l'emporte, entièrement
// ---------------------------------------------------------------------------

#[test]
fn la_derniere_ligne_anim_l_emporte_entierement_et_ne_se_melange_pas_a_la_precedente() {
    // Critère d'acceptation n° 4 : « plusieurs lignes `Anim` dans une même réponse : la dernière
    // l'emporte, entièrement (son nom ET ses réglages ; les réglages d'une ligne précédente ne se
    // mélangent pas à ceux de la dernière). »
    //
    // Deux fautes distinctes. Garder la **première** rendrait un éclairage périmé — le symptôme est
    // muet, la fenêtre affiche quelque chose de plausible. **Mélanger** est pire : le résultat ne
    // décrit aucun état que le boîtier ait jamais eu, et il est adopté comme tel. D'où une première
    // ligne qui porte les trois réglages et une seconde qui n'en porte qu'un : ce qui manque à la
    // seconde doit rester manquant.
    let complete = ligne_anim(
        ANIM_AUTRE,
        &[
            ("couleur", &hexa(COULEUR_AUTRE)),
            ("vitesse", &VITESSE_AUTRE.to_string()),
            ("direction", DIRECTION_AUTRE.slug()),
        ],
    );
    let partielle = ligne_anim(ANIM_LUE, &[("vitesse", &VITESSE_LUE.to_string())]);

    let lu = eclairage_lu(&reponse(&[complete.clone(), partielle.clone()]));
    assert_eq!(
        lu,
        venu(Some(ANIM_LUE), None, Some(VITESSE_LUE), None),
        "la dernière ligne `anim` dit tout : son nom, sa vitesse, et l'absence des deux autres — \
         {lu:?}"
    );

    // Et dans l'autre sens, pour que « la dernière » ne se confonde pas avec « la plus complète »
    // ni avec « la première ».
    let inverse = eclairage_lu(&reponse(&[partielle, complete]));
    assert_eq!(
        inverse,
        venu(
            Some(ANIM_AUTRE),
            Some(COULEUR_AUTRE),
            Some(VITESSE_AUTRE),
            Some(DIRECTION_AUTRE)
        ),
        "c'est bien la dernière qui compte, pas la plus fournie — {inverse:?}"
    );

    // Trois lignes : la faute « on garde la seconde » se voit ici et nulle part ailleurs.
    let trois = eclairage_lu(&reponse(&[
        ligne_anim(ANIM_AUTRE, &[("vitesse", "1")]),
        ligne_anim(ANIM_SANS_COULEUR, &[("vitesse", "5")]),
        ligne_anim(ANIM_LUE, &[("vitesse", &VITESSE_LUE.to_string())]),
    ]));
    assert_eq!(
        trois,
        venu(Some(ANIM_LUE), None, Some(VITESSE_LUE), None),
        "la dernière de trois — {trois:?}"
    );
}

// ---------------------------------------------------------------------------
// 10 — une clé répétée : la dernière valeur, puis on la relit
// ---------------------------------------------------------------------------

#[test]
fn une_cle_repetee_dans_une_ligne_garde_la_derniere_valeur_ecrite() {
    // Critère d'acceptation n° 4, seconde phrase : « une clé répétée dans une même ligne : la
    // dernière valeur l'emporte aussi. »
    //
    // Voir le point n° 1 de l'en-tête : la dernière valeur l'emporte **puis** on la relit. Si cette
    // dernière est illisible, le réglage est ignoré — garder la dernière valeur *valide*
    // rapporterait une valeur que le démon n'a pas écrite en dernier, ce qui est exactement le
    // mensonge que le critère n° 3 interdit.
    let lu = eclairage_lu(&[ligne_anim(
        ANIM_LUE,
        &[
            ("vitesse", &VITESSE_AUTRE.to_string()),
            ("couleur", &hexa(COULEUR_AUTRE)),
            ("direction", DIRECTION_AUTRE.slug()),
            ("vitesse", &VITESSE_LUE.to_string()),
            ("couleur", &hexa(COULEUR_LUE)),
            ("direction", DIRECTION_LUE.slug()),
        ],
    )]);
    assert_eq!(
        lu,
        venu(
            Some(ANIM_LUE),
            Some(COULEUR_LUE),
            Some(VITESSE_LUE),
            Some(DIRECTION_LUE)
        ),
        "des trois clés répétées, c'est la seconde écriture qui compte — {lu:?}"
    );

    let puis_illisible = eclairage_lu(&[ligne_anim(
        ANIM_LUE,
        &[
            ("vitesse", &VITESSE_LUE.to_string()),
            ("vitesse", "99"),
            ("couleur", &hexa(COULEUR_LUE)),
            ("couleur", "#129a40"),
            ("direction", DIRECTION_LUE.slug()),
            ("direction", "nulle-part"),
        ],
    )]);
    assert_eq!(
        puis_illisible,
        venu(Some(ANIM_LUE), None, None, None),
        "la dernière valeur l'emporte, et elle est illisible : le réglage est ignoré — \
         {puis_illisible:?}"
    );
}

// ---------------------------------------------------------------------------
// 11 — arc-en-ciel : pas de couleur, et c'est normal
// ---------------------------------------------------------------------------

#[test]
fn arc_en_ciel_ne_rapporte_aucune_couleur_et_l_animation_reste_bien_presente() {
    // Critère d'acceptation n° 5 : « `arc-en-ciel` ne rapporte aucune clé `couleur` — le cas normal
    // d'un `Eclairage` dont `couleur` vaut `None` avec une animation bien présente. »
    //
    // C'est le contre-exemple qui interdit de traiter « aucune couleur » comme « pas d'animation ».
    // Une implémentation qui exigerait les trois réglages pour croire à une ligne `anim` marcherait
    // sur cinq animations sur six, et #41 resterait entier sur la sixième.
    let ligne = ligne_du_demon(
        ANIM_SANS_COULEUR,
        &Reglages {
            couleur: COULEUR_LUE,
            vitesse: VITESSE_LUE,
            direction: DIRECTION_LUE,
            sonde: None,
        },
    );
    let ResponseLine::Anim { reglages, .. } = &ligne else {
        panic!("`ligne_du_demon` rend une ligne `anim`");
    };
    assert!(
        !reglages.iter().any(|(cle, _)| cle == "couleur"),
        "le démon n'écrit pas la couleur d'« {ANIM_SANS_COULEUR} » : {reglages:?}"
    );

    let lu = eclairage_lu(&reponse(&[ligne]));
    assert_eq!(
        lu,
        venu(
            Some(ANIM_SANS_COULEUR),
            None,
            Some(VITESSE_LUE),
            Some(DIRECTION_LUE)
        ),
        "« {ANIM_SANS_COULEUR} » tourne, sans couleur : c'est une animation bien présente — {lu:?}"
    );
}

// ---------------------------------------------------------------------------
// 12 — ce que le démon écrit, la fenêtre le relit
// ---------------------------------------------------------------------------

#[test]
fn ce_que_le_demon_ecrit_se_relit_pour_chaque_animation_du_catalogue() {
    // Critère d'acceptation n° 3 : les réglages sont lus « tels que le démon les écrit ». Le format
    // n'est donc pas recopié à la main ici : il est produit par `Animation::reglages_ecrits`, qui
    // est l'émetteur réel. C'est ce qui rend ce test sensible au jour où un format changerait d'un
    // seul côté — et c'est le seul test du fichier qui le soit.
    //
    // Toutes les animations, toutes les directions, toutes les vitesses : le catalogue se partage
    // entre celles qui se colorent et `arc-en-ciel`, et une lecture qui marcherait sur les cinq
    // premières et pas sur la sixième laisserait #41 entier une fois sur six.
    for nom in CATALOGUE {
        let animation = animation(nom);
        let accepte_couleur = animation.parametres_acceptes().contains(&"couleur");
        // ⚠️ Ajouté par #75, sur le modèle de `accepte_couleur` juste au-dessus. Quatre familles
        // nouvelles ne suivent aucune direction du boîtier — `rotation` suit le montage relevé de
        // chaque anneau, `pouls` la distance à la pompe, `scintillement` le hasard, `thermique`
        // une sonde. Le démon ne leur écrit donc pas de direction, et la fenêtre n'a rien à en
        // relire. C'est la même règle que pour `arc-en-ciel` et la couleur, à laquelle ce test se
        // pliait déjà.
        let accepte_direction = animation.parametres_acceptes().contains(&"direction");

        for direction in Direction::ALL {
            for vitesse in 1..=10u8 {
                let ecrits = Reglages {
                    couleur: COULEUR_LUE,
                    vitesse,
                    direction,
                    sonde: None,
                };
                let lu = eclairage_lu(&reponse(&[ligne_du_demon(nom, &ecrits)]));
                assert_eq!(
                    lu.animation.as_deref(),
                    Some(*nom),
                    "« {nom} » se relit sous son propre nom — {lu:?}"
                );
                assert_eq!(
                    lu.vitesse,
                    Some(vitesse),
                    "la vitesse écrite par le démon pour « {nom} » se relit — {lu:?}"
                );
                assert_eq!(
                    lu.direction,
                    accepte_direction.then(|| rang(direction)),
                    "« {nom} » {} la direction : c'est ce que la fenêtre doit en rapporter — {lu:?}",
                    if accepte_direction {
                        "accepte"
                    } else {
                        "refuse"
                    }
                );
                assert_eq!(
                    lu.couleur,
                    accepte_couleur.then_some(COULEUR_LUE),
                    "« {nom} » {} la couleur : c'est ce que la fenêtre doit en rapporter — {lu:?}",
                    if accepte_couleur { "accepte" } else { "refuse" }
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 13 — adopter : tant que c'est la même animation, rien ne bouge
// ---------------------------------------------------------------------------

#[test]
fn adopter_la_meme_animation_ne_change_rien_et_rend_faux() {
    // Critère d'acceptation, `adopter` : « même nom (y compris "les deux `None`") → rien n'est
    // modifié, rend `false`. Même si `venu` porte des réglages différents. Motif : les réponses
    // arrivent chaque seconde, et adopter à chaque fois ferait sauter le curseur de vitesse sous les
    // doigts de celui qui le tire. »
    //
    // C'est le défaut réparé sur les poignées de ventilateur en #32, qui reviendrait à l'identique
    // sur le curseur de vitesse : une adoption inconditionnelle ramènerait chaque seconde le curseur
    // là où le démon l'a laissé, c'est-à-dire là où l'utilisateur ne veut plus qu'il soit. Le
    // symptôme est un curseur qui se bat contre la main qui le tire.
    let depart = reglage(
        Some(ANIM_LUE),
        COULEUR_FENETRE,
        VITESSE_FENETRE,
        DIRECTION_FENETRE,
    );

    for (cas, apporte) in [
        (
            "avec trois réglages qui contredisent la fenêtre",
            venu(
                Some(ANIM_LUE),
                Some(COULEUR_LUE),
                Some(VITESSE_LUE),
                Some(DIRECTION_LUE),
            ),
        ),
        ("sans aucun réglage", venu(Some(ANIM_LUE), None, None, None)),
        (
            "avec un seul réglage",
            venu(Some(ANIM_LUE), None, Some(VITESSE_LUE), None),
        ),
    ] {
        let mut regle = depart.clone();
        let change = regle.adopter(&apporte);
        assert!(
            !change,
            "{cas} : c'est déjà « {ANIM_LUE} » qui tourne, il n'y a rien à adopter"
        );
        assert_eq!(
            regle, depart,
            "{cas} : le curseur ne doit pas sauter sous les doigts — {regle:?}"
        );
    }

    // Les deux `None` : la fenêtre ne pilote rien, le démon ne rapporte rien. Rien à faire non plus.
    let a_vide = fenetre_neuve();
    let mut regle = a_vide.clone();
    assert!(
        !regle.adopter(&Eclairage::default()),
        "rien ne tournait, rien ne tourne : aucun changement"
    );
    assert_eq!(
        regle, a_vide,
        "et les réglages du sélecteur restent où ils sont — {regle:?}"
    );
}

// ---------------------------------------------------------------------------
// 14 — adopter : au démarrage, ce qui tourne déjà est adopté en entier
// ---------------------------------------------------------------------------

#[test]
fn adopter_au_demarrage_une_animation_deja_en_cours_prend_son_nom_et_ses_reglages() {
    // Critère d'acceptation, `adopter`, cas particulier : « `self.animation == None` et
    // `venu.animation == Some(...)` → adoption complète, `true`. C'est le cas du démarrage, celui
    // qui motive l'issue. »
    //
    // La fenêtre vient de s'ouvrir sur un boîtier qui anime déjà. Elle croyait piloter du noir ;
    // elle doit repartir de ce que le démon fait, sans quoi `commande()` rend `None` et le curseur
    // de vitesse est muet — le défaut de #41 en une phrase.
    let mut regle = fenetre_neuve();
    assert_eq!(
        regle.commande(),
        None,
        "avant lecture, la fenêtre croit qu'aucune animation ne tourne : c'est le défaut de #41"
    );

    let lu = eclairage_lu(&reponse(&[ligne_du_demon(
        ANIM_LUE,
        &Reglages {
            couleur: COULEUR_LUE,
            vitesse: VITESSE_LUE,
            direction: DIRECTION_LUE,
            sonde: None,
        },
    )]));
    assert!(
        regle.adopter(&lu),
        "une animation tourne alors que la fenêtre n'en pilotait aucune : c'est un changement"
    );
    assert_eq!(
        regle,
        reglage(Some(ANIM_LUE), COULEUR_LUE, VITESSE_LUE, DIRECTION_LUE),
        "la fenêtre repart de ce que le démon fait, réglages compris — {regle:?}"
    );
    assert!(
        regle.commande().is_some(),
        "et le curseur de vitesse a de nouveau quelque chose à dire — {regle:?}"
    );
}

// ---------------------------------------------------------------------------
// 15 — adopter : ce qui n'est pas rapporté garde sa valeur
// ---------------------------------------------------------------------------

#[test]
fn adopter_une_autre_animation_ecrit_ce_qui_est_rapporte_et_garde_le_reste() {
    // Critère d'acceptation, `adopter` : « nom différent → `self.animation` prend celui de `venu` ;
    // chaque réglage rapporté par `venu` est écrit dans `self` ; chaque réglage non rapporté garde
    // sa valeur d'avant. Rend `true`. »
    //
    // La faute visée est l'écrasement par le vide : `self.vitesse = venu.vitesse.unwrap_or_default()`
    // met le curseur à zéro — hors des bornes du catalogue — dès que le démon ne s'est pas prononcé.
    // Le cas normal où il ne se prononce pas est `arc-en-ciel`, qui n'a pas de couleur : passer
    // d'une comète magenta à un arc-en-ciel repeindrait le sélecteur en noir, et la comète suivante
    // repartirait en noir.
    let depart = reglage(
        Some(ANIM_LUE),
        COULEUR_FENETRE,
        VITESSE_FENETRE,
        DIRECTION_FENETRE,
    );

    let mut partiel = depart.clone();
    assert!(
        partiel.adopter(&venu(Some(ANIM_AUTRE), None, Some(VITESSE_LUE), None)),
        "le nom change : il y a bien quelque chose à adopter"
    );
    assert_eq!(
        partiel,
        reglage(
            Some(ANIM_AUTRE),
            COULEUR_FENETRE,
            VITESSE_LUE,
            DIRECTION_FENETRE
        ),
        "la vitesse rapportée est écrite, la couleur et la direction gardent leur valeur — \
         {partiel:?}"
    );

    // Le cas normal du contrat : `arc-en-ciel`, qui ne rapporte aucune couleur.
    let mut arc = depart.clone();
    let lu = eclairage_lu(&reponse(&[ligne_du_demon(
        ANIM_SANS_COULEUR,
        &Reglages {
            couleur: COULEUR_LUE,
            vitesse: VITESSE_LUE,
            direction: DIRECTION_LUE,
            sonde: None,
        },
    )]));
    assert!(
        arc.adopter(&lu),
        "passer à « {ANIM_SANS_COULEUR} » est un changement"
    );
    assert_eq!(
        arc,
        reglage(
            Some(ANIM_SANS_COULEUR),
            COULEUR_FENETRE,
            VITESSE_LUE,
            DIRECTION_LUE
        ),
        "« {ANIM_SANS_COULEUR} » adopte sa vitesse et sa direction ; la couleur du sélecteur reste \
         celle qu'elle était — {arc:?}"
    );

    // Aucun réglage rapporté du tout : seul le nom change.
    let mut nu = depart.clone();
    assert!(nu.adopter(&venu(Some(ANIM_AUTRE), None, None, None)));
    assert_eq!(
        nu,
        reglage(
            Some(ANIM_AUTRE),
            COULEUR_FENETRE,
            VITESSE_FENETRE,
            DIRECTION_FENETRE
        ),
        "un `Eclairage` sans réglage ne déplace aucun curseur — {nu:?}"
    );
}

// ---------------------------------------------------------------------------
// 16 — adopter : l'extinction efface le nom et laisse les curseurs en place
// ---------------------------------------------------------------------------

#[test]
fn adopter_une_extinction_efface_le_nom_et_laisse_les_reglages_ou_ils_sont() {
    // Critère d'acceptation, `adopter`, cas particulier : « `self.animation == Some(...)` et
    // `venu.animation == None` → `self.animation` devient `None`, les trois réglages gardent leur
    // valeur (il n'y a rien à adopter), rend `true`. »
    //
    // C'est ce qui arrive quand quelqu'un envoie `animate off` par le socket, ou pose une couleur
    // fixe : la fenêtre doit cesser de prétendre piloter une animation. Mais les curseurs restent
    // où ils sont — les remettre à zéro serait un geste que personne n'a fait, et la prochaine
    // animation repartirait en noir à vitesse hors bornes.
    let mut regle = reglage(Some(ANIM_LUE), COULEUR_LUE, VITESSE_LUE, DIRECTION_LUE);
    assert!(
        regle.adopter(&eclairage_lu(&reponse(&[]))),
        "l'animation s'est arrêtée : la fenêtre doit le savoir"
    );
    assert_eq!(
        regle,
        reglage(None, COULEUR_LUE, VITESSE_LUE, DIRECTION_LUE),
        "le nom tombe, les trois curseurs restent — {regle:?}"
    );
    assert_eq!(
        regle.commande(),
        None,
        "et plus aucune animation ne tourne : bouger un curseur n'en démarre pas une"
    );

    // Une seconde réponse identique ne change plus rien : « les deux `None` » est un même nom.
    assert!(
        !regle.adopter(&eclairage_lu(&reponse(&[]))),
        "rien ne tourne toujours : plus rien à adopter"
    );
}

// ---------------------------------------------------------------------------
// 17 — le bout par lequel tout ça se voit
// ---------------------------------------------------------------------------

#[test]
fn apres_adoption_bouger_la_vitesse_envoie_la_couleur_et_la_direction_du_demon() {
    // Critère d'acceptation, « le bout par lequel tout ça se voit » : « après adoption au démarrage,
    // bouger la vitesse doit produire une commande qui porte la nouvelle vitesse ET la couleur et la
    // direction adoptées — pas celles du sélecteur de la fenêtre. C'est le second défaut que l'issue
    // corrige : sans ça, régler la vitesse changerait la couleur. »
    //
    // La chaîne entière, dans l'ordre où la fenêtre la parcourt : `lighting` → `eclairage_lu` →
    // `adopter` → la main sur le curseur → `commande`. Ce que le démon reçoit est relu par
    // `Animation::reglages`, qui est son décodeur réel : une commande jolie mais qu'il refuserait ne
    // passerait pas ce test.
    //
    // Adopter le seul nom suffit à faire repartir le curseur — le défaut n° 1 disparaîtrait, et
    // celui-ci prendrait sa place en silence : la comète du boîtier changerait de couleur au premier
    // réglage de vitesse, sans que rien ne l'annonce.
    let mut fenetre = fenetre_neuve();
    let lu = eclairage_lu(&reponse(&[ligne_du_demon(
        ANIM_LUE,
        &Reglages {
            couleur: COULEUR_LUE,
            vitesse: VITESSE_LUE,
            direction: DIRECTION_LUE,
            sonde: None,
        },
    )]));
    assert!(fenetre.adopter(&lu), "une animation tournait déjà");

    // La main tire le curseur de vitesse, et elle seule.
    fenetre.vitesse = VITESSE_TIREE;

    let (nom, paires) = animate(&fenetre);
    assert_eq!(
        nom, ANIM_LUE,
        "c'est l'animation du démon qui repart, pas une autre"
    );

    let recu = relu(&nom, &paires);
    assert_eq!(
        recu.vitesse, VITESSE_TIREE,
        "la nouvelle vitesse part au démon — {paires:?}"
    );
    assert_eq!(
        recu.couleur, COULEUR_LUE,
        "et la couleur est celle que le démon a rapportée, pas celle du sélecteur de la fenêtre : \
         régler la vitesse ne repeint pas le boîtier — {paires:?}"
    );
    assert_eq!(
        recu.direction, DIRECTION_LUE,
        "et la direction aussi — {paires:?}"
    );
    assert_ne!(
        recu.couleur, COULEUR_FENETRE,
        "la couleur du sélecteur n'a rien à faire dans cette commande"
    );
    assert_ne!(
        recu.direction, DIRECTION_FENETRE,
        "la direction du sélecteur non plus"
    );

    // Et la réponse `lighting` suivante — le démon n'a pas encore reçu la commande, il rapporte
    // toujours l'ancienne vitesse — ne doit pas ramener le curseur en arrière.
    let encore = eclairage_lu(&reponse(&[ligne_du_demon(
        ANIM_LUE,
        &Reglages {
            couleur: COULEUR_LUE,
            vitesse: VITESSE_LUE,
            direction: DIRECTION_LUE,
            sonde: None,
        },
    )]));
    assert!(
        !fenetre.adopter(&encore),
        "c'est la même animation : il n'y a rien à adopter"
    );
    assert_eq!(
        fenetre.vitesse, VITESSE_TIREE,
        "le curseur reste où la main l'a mis — {fenetre:?}"
    );
}
