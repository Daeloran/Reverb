//! Tests d'intention — issue #11, « Livrer la règle udev de Reverb, sans dépendre d'OpenRGB ».
//!
//! Ce que ces tests garantissent :
//! - le dépôt livre lui-même `packaging/60-reverb.rules` ;
//! - cette règle couvre tous les périphériques que Reverb pilote, y compris l'écran du Kraken,
//!   qui passe par un endpoint bulk USB et non par hidraw ;
//! - l'accès est accordé par `TAG+="uaccess"` — donc à l'utilisateur physiquement connecté et
//!   à personne d'autre — et jamais par `MODE=` ni `GROUP=`.
//!
//! Pourquoi. Sans règle propre, Reverb dépend des règles d'OpenRGB (le logiciel qu'il vise à
//! remplacer) et, pour le Kraken, de `92-viia.rules`, une règle tierce qui ouvre en `0666` tous
//! les hidraw de la machine — y compris la lecture brute du clavier, pour tout process local.
//!
//! Le mode de défaillance visé est silencieux : ajouter un modèle de contrôleur dans
//! `reverb_proto::Model` sans ajouter la ligne udev correspondante ne se voit qu'à l'exécution,
//! sur la machine de quelqu'un d'autre. D'où l'itération sur `Model::ALL` plutôt qu'une liste
//! d'identifiants recopiée ici : recopier la liste, ce serait perdre exactement la propriété
//! recherchée.
//!
//! Ces tests portent sur le fichier versionné dans le dépôt, jamais sur l'état de la machine qui
//! les exécute : ils ne lisent ni `/etc/udev`, ni `/dev`, ni `/sys`.
//!
//! Convention. Une « ligne active » est une ligne qui n'est ni vide ni un commentaire (`#`) une
//! fois les espaces de bord retirés. Une règle est donc supposée tenir sur une seule ligne : la
//! continuation par `\` en fin de ligne n'est pas gérée, et une règle ainsi coupée serait vue
//! comme plusieurs lignes actives dont certaines sans `TAG+="uaccess"`.

use reverb_proto::Model;

/// Périphériques relevés dans l'issue, sous la forme (identifiant produit, description).
/// Ils viennent de la constatation faite sur SHYNAEL le 2026-07-31, pas du code.
const PERIPHERIQUES_DE_L_ISSUE: [(&str, &str); 3] = [
    ("2019", "contrôleur RGB + ventilateurs"),
    ("2012", "contrôleur RGB"),
    ("300c", "Kraken, donc l'écran"),
];

/// Le Kraken : le seul des trois qui ne soit couvert par aucune règle NZXT aujourd'hui, et le
/// seul dont l'image passe par un endpoint bulk — donc par `SUBSYSTEM=="usb"`.
const PRODUIT_KRAKEN: &str = "300c";

/// Chemin du fichier de règles, depuis la racine du dépôt.
const CHEMIN_RELATIF: &str = "packaging/60-reverb.rules";

fn chemin_des_regles() -> std::path::PathBuf {
    let racine = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // Uniquement pour un message d'échec lisible : si la racine n'est pas canonisable, on garde
    // le chemin brut, qui reste exact.
    let racine = match racine.canonicalize() {
        Ok(canonique) => canonique,
        Err(_) => racine,
    };
    racine.join(CHEMIN_RELATIF)
}

/// Lit le fichier de règles. Pas d'`unwrap()` silencieux : si le fichier manque, l'échec doit
/// nommer le chemin cherché et dire que c'est au dépôt de le livrer.
fn charge_les_regles() -> String {
    let chemin = chemin_des_regles();
    match std::fs::read_to_string(&chemin) {
        Ok(contenu) => contenu,
        Err(erreur) => panic!(
            "Fichier de règles udev introuvable ou illisible.\n  \
             Chemin cherché : {}\n  \
             Cause système  : {erreur}\n  \
             Ce fichier doit être livré par le dépôt (issue #11) : tant qu'il manque, l'accès aux \
             contrôleurs NZXT repose sur les règles d'OpenRGB et sur `92-viia.rules`.",
            chemin.display()
        ),
    }
}

/// Lignes actives du fichier, avec leur numéro (1-indexé) pour les messages d'échec.
fn lignes_actives(contenu: &str) -> Vec<(usize, &str)> {
    contenu
        .lines()
        .enumerate()
        .map(|(index, ligne)| (index + 1, ligne.trim()))
        .filter(|(_, ligne)| !ligne.is_empty() && !ligne.starts_with('#'))
        .collect()
}

/// Retire tous les espaces : `TAG += "uaccess"` et `TAG+="uaccess"` doivent être traités pareil.
fn compacte(ligne: &str) -> String {
    ligne.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Les identifiants hexadécimaux s'écrivent indifféremment `1e71` ou `1E71` selon la clé udev
/// employée (`ATTRS{idVendor}` ou `KERNELS`). La comparaison ignore donc la casse.
fn contient_sans_casse(texte: &str, motif: &str) -> bool {
    texte
        .to_ascii_lowercase()
        .contains(&motif.to_ascii_lowercase())
}

/// Vrai si la ligne emploie la clé donnée, sous sa forme `=` comme sous sa forme `:=`
/// (affectation non surchargeable) — les deux accordent le même accès.
fn emploie_la_cle(ligne_compactee: &str, cle: &str) -> bool {
    ligne_compactee.contains(&format!("{cle}=")) || ligne_compactee.contains(&format!("{cle}:="))
}

/// Vrai si la ligne couvre le couple vendeur:produit demandé.
fn couvre_le_produit(ligne: &str, vendeur: &str, produit: &str) -> bool {
    contient_sans_casse(ligne, vendeur) && contient_sans_casse(ligne, produit)
}

fn vendeur_nzxt() -> String {
    format!("{:04x}", reverb_proto::VENDOR_ID)
}

fn produit_du_modele(modele: Model) -> String {
    format!("{:04x}", modele.product_id())
}

/// Reproduit les lignes actives dans un message d'échec, pour qu'on voie ce qui est livré
/// sans rouvrir le fichier.
fn recopie_des_lignes_actives(actives: &[(usize, &str)]) -> String {
    if actives.is_empty() {
        return "    (aucune ligne active)".to_string();
    }
    actives
        .iter()
        .map(|(numero, ligne)| format!("    ligne {numero} : {ligne}\n"))
        .collect()
}

#[test]
fn le_fichier_de_regles_udev_est_livre_par_le_depot() {
    let contenu = charge_les_regles();
    assert!(
        !contenu.trim().is_empty(),
        "Le fichier {} existe mais est vide.\n  \
         Il doit contenir les règles d'accès aux contrôleurs NZXT pilotés par Reverb : sans elles, \
         `reverb list` exige `sudo` ou les règles d'OpenRGB.",
        chemin_des_regles().display()
    );
}

#[test]
fn le_fichier_de_regles_ne_se_reduit_pas_a_des_commentaires() {
    let contenu = charge_les_regles();
    let actives = lignes_actives(&contenu);
    assert!(
        !actives.is_empty(),
        "Le fichier {} ne contient aucune ligne active : que du vide et des commentaires.\n  \
         Aucun périphérique n'est donc couvert, et les vérifications d'absence de `MODE=` ou de \
         `GROUP=` passeraient à vide.",
        chemin_des_regles().display()
    );
}

#[test]
fn chaque_modele_supporte_a_sa_ligne_dans_la_regle_udev() {
    let contenu = charge_les_regles();
    let actives = lignes_actives(&contenu);
    let vendeur = vendeur_nzxt();

    let mut manquants: Vec<String> = Vec::new();
    for modele in Model::ALL {
        let produit = produit_du_modele(modele);
        let couvert = actives
            .iter()
            .any(|(_, ligne)| couvre_le_produit(ligne, &vendeur, &produit));
        if !couvert {
            manquants.push(format!("    {vendeur}:{produit}\n"));
        }
    }

    assert!(
        manquants.is_empty(),
        "Des modèles exposés par `reverb_proto::Model::ALL` n'ont aucune ligne dans {} :\n{}  \
         Si vous venez d'ajouter un modèle de contrôleur, la règle udev n'a pas suivi : le \
         binaire connaîtra le périphérique, mais l'utilisateur n'y aura pas accès sans `sudo`.\n  \
         Ajouter pour chacun une ligne du type :\n    \
         SUBSYSTEM==\"hidraw\", ATTRS{{idVendor}}==\"{vendeur}\", ATTRS{{idProduct}}==\"<produit>\", TAG+=\"uaccess\"\n  \
         Lignes actives actuellement livrées :\n{}",
        chemin_des_regles().display(),
        manquants.concat(),
        recopie_des_lignes_actives(&actives)
    );
}

#[test]
fn les_trois_peripheriques_releves_dans_l_issue_sont_couverts() {
    let contenu = charge_les_regles();
    let actives = lignes_actives(&contenu);
    let vendeur = vendeur_nzxt();

    let mut manquants: Vec<String> = Vec::new();
    for (produit, description) in PERIPHERIQUES_DE_L_ISSUE {
        let couvert = actives
            .iter()
            .any(|(_, ligne)| couvre_le_produit(ligne, &vendeur, produit));
        if !couvert {
            manquants.push(format!("    {vendeur}:{produit} — {description}\n"));
        }
    }

    assert!(
        manquants.is_empty(),
        "Des périphériques que Reverb pilote ne sont couverts par aucune ligne active de {} :\n{}  \
         Ce sont les trois relevés sur la machine dans l'issue #11 ; tant qu'ils n'y figurent pas, \
         désinstaller OpenRGB ou `92-viia.rules` casse Reverb.\n  \
         Lignes actives actuellement livrées :\n{}",
        chemin_des_regles().display(),
        manquants.concat(),
        recopie_des_lignes_actives(&actives)
    );
}

#[test]
fn l_ecran_du_kraken_est_couvert_par_une_regle_usb() {
    let contenu = charge_les_regles();
    let actives = lignes_actives(&contenu);
    let vendeur = vendeur_nzxt();

    let couvert = actives.iter().any(|(_, ligne)| {
        couvre_le_produit(ligne, &vendeur, PRODUIT_KRAKEN)
            && compacte(ligne).contains("SUBSYSTEM==\"usb\"")
    });

    assert!(
        couvert,
        "Aucune ligne active de {} ne couvre {vendeur}:{PRODUIT_KRAKEN} en `SUBSYSTEM==\"usb\"`.\n  \
         L'image de l'écran du Kraken passe par un endpoint bulk, pas par hidraw : une règle \
         `SUBSYSTEM==\"hidraw\"` seule laisse l'écran inaccessible sans `sudo`.\n  \
         Lignes actives actuellement livrées :\n{}",
        chemin_des_regles().display(),
        recopie_des_lignes_actives(&actives)
    );
}

#[test]
fn aucune_ligne_active_n_impose_de_mode() {
    let contenu = charge_les_regles();
    let actives = lignes_actives(&contenu);

    let fautives: String = actives
        .iter()
        .filter(|(_, ligne)| emploie_la_cle(&compacte(ligne), "MODE"))
        .map(|(numero, ligne)| format!("    ligne {numero} : {ligne}\n"))
        .collect();

    assert!(
        fautives.is_empty(),
        "Des lignes de {} emploient `MODE=` :\n{}  \
         Un `MODE=` ouvre le périphérique à des process qui ne sont pas ceux de l'utilisateur \
         physiquement connecté — c'est précisément ce que fait `92-viia.rules`, et ce que cette \
         règle doit cesser de reproduire. Utiliser `TAG+=\"uaccess\"` à la place.",
        chemin_des_regles().display(),
        fautives
    );
}

#[test]
fn aucune_ligne_active_n_attribue_de_groupe() {
    let contenu = charge_les_regles();
    let actives = lignes_actives(&contenu);

    let fautives: String = actives
        .iter()
        .filter(|(_, ligne)| emploie_la_cle(&compacte(ligne), "GROUP"))
        .map(|(numero, ligne)| format!("    ligne {numero} : {ligne}\n"))
        .collect();

    assert!(
        fautives.is_empty(),
        "Des lignes de {} emploient `GROUP=` :\n{}  \
         Un `GROUP=` accorde l'accès en permanence à tous les membres du groupe, connectés ou non, \
         et impose au passage la création de ce groupe à l'installation. Utiliser \
         `TAG+=\"uaccess\"` à la place.",
        chemin_des_regles().display(),
        fautives
    );
}

#[test]
fn toute_ligne_active_delegue_l_acces_a_uaccess() {
    let contenu = charge_les_regles();
    let actives = lignes_actives(&contenu);

    let fautives: String = actives
        .iter()
        .filter(|(_, ligne)| !compacte(ligne).contains("TAG+=\"uaccess\""))
        .map(|(numero, ligne)| format!("    ligne {numero} : {ligne}\n"))
        .collect();

    assert!(
        fautives.is_empty(),
        "Des lignes actives de {} ne portent pas `TAG+=\"uaccess\"` :\n{}  \
         Chaque règle livrée doit accorder l'accès à l'utilisateur physiquement connecté, et par \
         ce seul moyen. Une règle qui matche un périphérique sans le taguer ne lui donne aucun \
         accès : le périphérique restera réservé à root.",
        chemin_des_regles().display(),
        fautives
    );
}

#[test]
fn toute_ligne_active_porte_le_tag_reverb() {
    let contenu = charge_les_regles();
    let actives = lignes_actives(&contenu);

    let fautives: String = actives
        .iter()
        .filter(|(_, ligne)| !compacte(ligne).contains("TAG+=\"reverb\""))
        .map(|(numero, ligne)| format!("    ligne {numero} : {ligne}\n"))
        .collect();

    assert!(
        fautives.is_empty(),
        "Des lignes actives de {} ne portent pas `TAG+=\"reverb\"` :\n{}  \
         Ce tag n'accorde aucun droit — un tag udev est une étiquette, pas une permission — et ne \
         remplace donc pas `TAG+=\"uaccess\"` : il sert à signer la règle. Sans lui, rien ne permet \
         de dire si l'`uaccess` posé sur un périphérique vient de la règle de ce dépôt ou d'une \
         autre : `udevadm test` ne journalise pas les `TAG+=` (seules les règles à effet de bord, \
         `MODE=` ou `RUN{{builtin}}+=`, apparaissent avec leur fichier et leur ligne), et sur \
         SHYNAEL `reverb list` fonctionne déjà grâce à `92-viia.rules`, qui ouvre tous les hidraw \
         en `MODE=\"0666\"`. Le critère de vérification matérielle de l'issue #11 devient alors \
         invérifiable. Avec le tag, il se contrôle sans root :\n    \
         udevadm info -q all -n /dev/hidraw12 | grep TAGS   # « reverb » présent <=> notre règle a matché\n  \
         C'est la pratique des règles constructeur : OpenRGB pose `NZXT_RGB__Fan_Controller`.",
        chemin_des_regles().display(),
        fautives
    );
}
