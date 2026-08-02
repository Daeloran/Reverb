//! Tests d'intention — issue #74, « Enregistrer une ambiance sous un nom et y revenir ».
//!
//! Écrits **avant** l'implémentation, depuis l'issue #74 seule. Rien de `crates/*/src/` n'a été
//! lu pour les produire, hors les signatures publiques nécessaires à la compilation. Si l'un de
//! ces tests échoue après implémentation, c'est le code qu'on corrige, jamais le test.
//!
//! Ce fichier ne couvre qu'**une** chose : le nom d'un profil. Tout le reste — encodage,
//! disque, application — vit dans `crates/reverb-daemon/tests/spec_profils.rs`, parce qu'un
//! profil emporte un `Eclairage`, des `Zones` et un `ecran::Etat`, trois types du démon.
//!
//! ## Le défaut visé : le démon est root
//!
//! `profil save <nom>` fait construire un chemin de fichier par un processus qui tourne en
//! `User=root` (ADR-002). Un nom qui traverse jusqu'à `open()` sans avoir été borné est une
//! écriture arbitraire sur le système de fichiers : `../../etc/reverb/geometrie.conf` détruirait
//! un relevé fait au sol, `../../../etc/shadow` bien pire. Ce n'est pas une hypothèse d'école,
//! c'est la forme exacte de la faille que l'approche technique de l'issue nomme : « viser hors du
//! répertoire doit être **irreprésentable**, pas refusé à l'exécution », sur le modèle de
//! `SlotAddress` pour la RAM — dont le garde-fou est lui aussi vérifié par un test exhaustif sur
//! les 256 entrées possibles (README, « La RAM Corsair »).
//!
//! D'où la forme de ces tests. Ils ne se contentent pas d'une liste noire de chaînes hostiles :
//! [`aucun_nom_accepte_ne_sort_du_repertoire_des_profils`] balaie **les 256 valeurs d'octet**, à
//! quatre places dans le nom, et exige de chacune ou bien un refus, ou bien la preuve structurelle
//! que le fichier reste dans son répertoire. Une liste noire ne prouve que ce qu'on a pensé à y
//! mettre.
//!
//! ## L'API que ces tests supposent — c'est ici qu'elle se décide
//!
//! Rien n'existe encore côté `src/`. Le contrat que l'implémentation doit honorer, au complet :
//!
//! ```ignore
//! // crates/reverb-proto/src/profil.rs, réexporté à la racine du crate
//! pub struct NomProfil(/* privé */);
//!
//! pub struct NomInvalide {
//!     pub saisi: String,
//!     pub raison: String,
//! }
//!
//! impl NomProfil {
//!     /// Borne haute d'un nom, en octets. Le fichier qui en sort porte en plus
//!     /// [`NomProfil::EXTENSION`], et l'ensemble doit tenir sous `NAME_MAX`.
//!     pub const MAX_OCTETS: usize;
//!     /// Le suffixe du fichier, extension comprise — « .conf ».
//!     pub const EXTENSION: &'static str;
//!
//!     pub fn nouveau(saisi: &str) -> Result<NomProfil, NomInvalide>;
//!     pub fn as_str(&self) -> &str;
//!     /// Le **nom de fichier**, jamais un chemin : aucun séparateur ne peut y figurer.
//!     pub fn fichier(&self) -> String;
//!     /// L'inverse de [`NomProfil::fichier`], strict.
//!     pub fn depuis_fichier(fichier: &str) -> Result<NomProfil, NomInvalide>;
//! }
//! ```
//!
//! Dérivés exigés : `NomProfil: Debug + Clone + PartialEq + Eq + PartialOrd + Ord + Hash +
//! Display`, `NomInvalide: Debug + Clone + PartialEq + Eq + Display + std::error::Error`.
//! `Ord` n'est pas décoratif : `profil list` doit rendre les noms dans un ordre stable, et
//! l'ordre de `read_dir` ne l'est pas.
//!
//! ## Trois points que l'issue laisse ouverts, et que ces tests tranchent
//!
//! 1. **`..` est refusé partout dans le nom, pas seulement seul.** L'issue écrit « contenant …
//!    `..` ». C'est la lecture littérale, et c'est la seule qui ne demande pas de raisonner sur
//!    l'interaction entre `..`, le suffixe ajouté et le répertoire courant du démon. Refuser
//!    « avant..après » coûte un nom que personne n'écrit ; l'accepter demande de prouver quelque
//!    chose.
//! 2. **Tout caractère de contrôle est refusé, pas seulement `\0` et `\n`.** L'issue nomme ces
//!    deux-là. Ils sont le cas dangereux d'une même famille : le nom voyage sur un protocole
//!    **texte, une ligne par requête** (ADR-002, [`reverb_proto::ipc::MAX_LINE_LEN`]), où un `\r`
//!    tronque une ligne aussi bien qu'un `\n`, et finit dans un nom de fichier écrit par root, où
//!    une tabulation rend le fichier intypable. Refuser la famille entière est plus simple à tenir
//!    qu'une liste de deux.
//! 3. **Un nom est accepté tel quel ou refusé — jamais nettoyé.** C'est la propriété que
//!    [`un_nom_accepte_n_est_jamais_transforme`] verrouille, et elle vaut plus que toutes les
//!    autres réunies : une implémentation qui *retire* les `/` au lieu de refuser transforme
//!    « a/../b » en « a..b », passe tous les tests de chemin, et donne deux profils différents le
//!    même fichier. Le nettoyage silencieux est la façon classique dont ce garde-fou cède.
//!
//! ## Ce que ces tests ne couvrent pas, et pourquoi
//!
//! - Le verbe `profil` sur le socket. L'issue liste quatre commandes mais ne fixe aucune forme
//!   de requête, et inventer une variante de `Request` reviendrait à spécifier ce que l'issue n'a
//!   pas décidé. Ce qui en est vérifiable ici sans rien inventer, c'est qu'un nom accepté **peut**
//!   traverser un protocole d'une ligne : [`un_nom_accepte_traverse_une_ligne_du_socket`].
//! - L'unicité de casse : `Nuit` et `nuit` font deux fichiers sous Linux, et l'issue n'en dit rien.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Component, Path};

use reverb_proto::ipc::MAX_LINE_LEN;
use reverb_proto::{NomInvalide, NomProfil};

// ---------------------------------------------------------------------------
// Vecteurs témoins
// ---------------------------------------------------------------------------

/// Le répertoire des profils, tel que l'approche technique de l'issue le pose. Il ne sert ici
/// qu'à composer un chemin **en mémoire** : aucun test de ce fichier ne touche au disque.
const REPERTOIRE: &str = "/var/lib/reverb/profils";

/// Des noms qu'un utilisateur écrit vraiment, et que le refus ne doit jamais attraper.
///
/// L'issue est explicite : « un nom valide de caractères accentués ou d'espaces est **accepté** —
/// “soirée d'été” est un nom légitime ». Une validation trop zélée — liste blanche
/// `[a-z0-9-]` — casserait la moitié de cette liste sans qu'aucun test de sécurité ne s'en
/// aperçoive, et rendrait l'outil désagréable pour la seule personne qui s'en sert.
fn noms_legitimes() -> Vec<&'static str> {
    vec![
        "nuit",
        "jour",
        "soirée d'été",
        "Noël",
        "week-end",
        "travail_concentré",
        "LAN party",
        "façade",
        "ambiance n°3",
        "très très calme",
        "цвет",
        "夜",
        "a",
        "nuit.conf",
        "100%",
        "prêt !",
        "(essai)",
        "café — pause",
    ]
}

/// Les noms hostiles nommés par l'issue, chacun avec la sous-chaîne que le refus doit citer.
///
/// `None` là où la raison ne peut pas se citer littéralement : un nom vide n'a rien à montrer, et
/// un octet nul ne s'écrit pas dans un message. Ces cas-là sont couverts autrement — par le refus
/// lui-même, et par [`les_refus_distinguent_ce_qui_cloche`].
fn noms_hostiles() -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    vec![
        ("le nom vide", "", None),
        ("un nom fait d'espaces", "   ", None),
        ("une barre oblique", "nuit/jour", Some("/")),
        ("un chemin absolu", "/etc/reverb/geometrie", Some("/")),
        ("une barre finale", "nuit/", Some("/")),
        ("une contre-oblique", "nuit\\jour", Some("\\")),
        ("un chemin Windows", "C:\\profils\\nuit", Some("\\")),
        ("le répertoire parent", "..", Some("..")),
        ("une remontée", "../../etc/passwd", Some("..")),
        (
            "une remontée enfouie",
            "nuit/../../../etc/shadow",
            Some(".."),
        ),
        ("un « .. » au milieu", "avant..apres", Some("..")),
        ("un octet nul", "nuit\0", None),
        ("un octet nul au milieu", "nuit\0.conf", None),
        ("un retour à la ligne", "nuit\nlight all ff0000", None),
        ("un retour chariot", "nuit\rjour", None),
        ("une tabulation", "nuit\tjour", None),
    ]
}

// ---------------------------------------------------------------------------
// Petits outils
// ---------------------------------------------------------------------------

fn accepte(saisi: &str) -> NomProfil {
    NomProfil::nouveau(saisi).unwrap_or_else(|erreur| {
        panic!("« {saisi} » devait être accepté, il a été refusé : {erreur}")
    })
}

fn refuse(saisi: &str) -> NomInvalide {
    match NomProfil::nouveau(saisi) {
        Ok(nom) => panic!(
            "« {} » devait être refusé, il a été accepté et a rendu le fichier « {} ». Le démon \
             est root : un nom qui passe est un fichier qu'il écrit.",
            saisi.escape_debug(),
            nom.fichier()
        ),
        Err(erreur) => erreur,
    }
}

/// Le chemin que le démon composerait pour ce nom, **en mémoire**.
fn chemin_compose(nom: &NomProfil) -> std::path::PathBuf {
    Path::new(REPERTOIRE).join(nom.fichier())
}

/// La preuve structurelle qu'un nom ne sort pas de son répertoire.
///
/// Trois vérifications, et il en faut trois : `parent()` seul se laisse tromper par un chemin
/// qui remonte (`a/../b` a bien un parent), `components()` seul ne dit pas *où* on atterrit, et
/// `file_name()` seul ne voit pas un `.` isolé.
fn reste_dans_le_repertoire(nom: &NomProfil, saisi: &str) {
    let fichier = nom.fichier();
    let chemin = chemin_compose(nom);

    assert_eq!(
        chemin.parent(),
        Some(Path::new(REPERTOIRE)),
        "« {} » compose {} — le parent doit être {REPERTOIRE} et rien d'autre",
        saisi.escape_debug(),
        chemin.display()
    );
    assert_eq!(
        chemin.file_name(),
        Some(OsStr::new(&fichier)),
        "« {} » : le dernier segment du chemin doit être exactement le fichier rendu par \
         `fichier()`",
        saisi.escape_debug()
    );

    let segments: Vec<Component<'_>> = Path::new(&fichier).components().collect();
    assert_eq!(
        segments.len(),
        1,
        "« {} » : `fichier()` doit rendre **un nom de fichier**, pas un chemin — obtenu « {} », \
         soit {} segments",
        saisi.escape_debug(),
        fichier.escape_debug(),
        segments.len()
    );
    assert!(
        matches!(segments[0], Component::Normal(_)),
        "« {} » : le seul segment doit être un nom ordinaire, ni « . », ni « .. », ni une racine \
         — obtenu {:?}",
        saisi.escape_debug(),
        segments[0]
    );
}

// ---------------------------------------------------------------------------
// 3 et 5 — ce qui est refusé, ce qui est accepté
// ---------------------------------------------------------------------------

#[test]
fn un_nom_vide_est_refuse() {
    // Test d'intention n° 3 de l'issue : « Un nom vide est refusé ».
    //
    // Un profil sans nom ne se rappelle pas : `profil load` ne peut pas le désigner, et
    // `profil list` afficherait une ligne blanche. Le fichier qu'il produirait — « .conf » —
    // serait de surcroît invisible dans un `ls`.
    let erreur = refuse("");
    assert_eq!(
        erreur.saisi, "",
        "le refus doit rendre ce qu'on lui a donné, pour que le message soit citable"
    );
    assert!(
        !erreur.raison.trim().is_empty(),
        "« refusé en nommant la raison » (critère d'acceptation) : un refus muet renvoie \
         l'utilisateur deviner"
    );

    // Un nom fait d'espaces est le même profil sans nom, écrit autrement : il produirait un
    // fichier « <espaces>.conf » qu'aucun `profil load` ne saurait retaper.
    let erreur = refuse("   ");
    assert!(!erreur.raison.trim().is_empty());
}

#[test]
fn un_nom_qui_designerait_un_fichier_ailleurs_est_refuse_en_nommant_la_raison() {
    // Test d'intention n° 4 de l'issue : « Un nom contenant `/`, `\`, `..`, `\0` ou `\n` est
    // refusé, et le message nomme la raison ». Critère d'acceptation : « le démon est root, un nom
    // de profil ne doit jamais pouvoir désigner un fichier hors de son répertoire ».
    for (description, saisi, cite) in noms_hostiles() {
        let erreur = refuse(saisi);

        assert_eq!(
            erreur.saisi, saisi,
            "{description} : le refus doit rendre le nom **tel qu'il a été saisi**, sinon le \
             message montre autre chose que ce que l'utilisateur a tapé"
        );
        assert!(
            !erreur.raison.trim().is_empty(),
            "{description} : « le message nomme la raison » — un refus sans explication laisse \
             croire à un bug de l'outil"
        );

        if let Some(cite) = cite {
            assert!(
                erreur.raison.contains(cite),
                "{description} : la raison doit citer « {cite} », le morceau exact à corriger. \
                 Raison obtenue : {}",
                erreur.raison
            );
        }

        let message = erreur.to_string();
        assert!(
            message.contains(&erreur.raison),
            "{description} : le Display doit porter la raison — c'est lui qui part sur le socket. \
             Obtenu : « {message} »"
        );
        let _: &dyn std::error::Error = &erreur;
    }
}

#[test]
fn les_refus_distinguent_ce_qui_cloche() {
    // « Refusé en nommant la raison » n'est tenu que si les raisons diffèrent : un unique
    // « nom de profil invalide » servi aux seize cas ci-dessus **nomme** quelque chose et
    // n'apprend rien. Un octet nul et un nom vide ne se corrigent pas de la même façon.
    let familles: BTreeSet<String> = noms_hostiles()
        .into_iter()
        .map(|(_, saisi, _)| refuse(saisi).raison)
        .collect();
    assert!(
        familles.len() >= 4,
        "un nom vide, un séparateur de chemin, une remontée « .. » et un caractère de contrôle \
         sont quatre fautes distinctes : elles ne peuvent pas partager une seule explication. \
         Raisons obtenues : {familles:?}"
    );
}

#[test]
fn un_nom_accentue_ou_espace_est_accepte() {
    // Test d'intention n° 5 de l'issue : « Un nom valide de caractères accentués ou d'espaces est
    // **accepté** — “soirée d'été” est un nom légitime ».
    //
    // Le garde-fou du chemin se tient très bien avec une liste blanche `[a-z0-9-]`, et c'est
    // précisément le raccourci que ce test interdit : Reverb est un outil personnel dont
    // l'ergonomie est la raison d'être (README, « Pourquoi »), et « Noël » doit s'écrire « Noël ».
    for saisi in noms_legitimes() {
        let nom = accepte(saisi);
        reste_dans_le_repertoire(&nom, saisi);
    }
}

#[test]
fn un_nom_accepte_n_est_jamais_transforme() {
    // La faille classique de ce garde-fou n'est pas de laisser passer un nom hostile, c'est de le
    // **nettoyer** : retirer les `/` transforme « a/../b » en « a..b », donne un chemin
    // impeccable, et fait pointer deux profils différents sur le même fichier. Le nom rendu doit
    // être celui qu'on a donné, à l'octet près, ou alors il n'y a pas de nom.
    for saisi in noms_legitimes() {
        let nom = accepte(saisi);
        assert_eq!(
            nom.as_str(),
            saisi,
            "« {saisi} » : un nom accepté se rend tel quel. Le nettoyer silencieusement rendrait \
             deux noms différents indiscernables."
        );
        assert_eq!(
            nom.to_string(),
            saisi,
            "« {saisi} » : le Display est ce que voit l'utilisateur dans `profil list`"
        );
        assert_eq!(
            nom.fichier(),
            format!("{saisi}{}", NomProfil::EXTENSION),
            "« {saisi} » : le fichier est le nom plus l'extension, sans réécriture"
        );
    }

    // Et le pendant : deux noms différents ne se replient jamais l'un sur l'autre.
    let fichiers: BTreeSet<String> = noms_legitimes()
        .into_iter()
        .map(|saisi| accepte(saisi).fichier())
        .collect();
    assert_eq!(
        fichiers.len(),
        noms_legitimes().len(),
        "deux noms distincts doivent donner deux fichiers distincts — sinon enregistrer l'un \
         écrase l'autre"
    );
}

// ---------------------------------------------------------------------------
// Le garde-fou lui-même — exhaustif, à la façon de `SlotAddress`
// ---------------------------------------------------------------------------

#[test]
fn aucun_nom_accepte_ne_sort_du_repertoire_des_profils() {
    // Approche technique de l'issue : « le type qui en sort est le seul que le démon sait
    // transformer en fichier — même principe que `SlotAddress` pour la RAM : viser hors du
    // répertoire doit être **irreprésentable**, pas refusé à l'exécution ».
    //
    // `SlotAddress` est vérifié par un test exhaustif sur ses 256 entrées possibles (README,
    // « La RAM Corsair »). Celui-ci balaie les 256 valeurs d'octet, à quatre places dans le nom,
    // et n'exige rien d'autre que l'alternative : ou bien le nom est refusé, ou bien le fichier
    // qu'il compose reste un enfant direct du répertoire. Une liste noire ne prouve que ce qu'on
    // a pensé à y mettre ; ce balayage ne suppose rien.
    let mut acceptes = 0usize;
    for octet in 0u8..=u8::MAX {
        let caractere = char::from(octet);
        for saisi in [
            format!("{caractere}"),
            format!("nuit{caractere}"),
            format!("{caractere}nuit"),
            format!("nu{caractere}it"),
            format!("..{caractere}"),
            format!("{caractere}{caractere}"),
        ] {
            if let Ok(nom) = NomProfil::nouveau(&saisi) {
                acceptes += 1;
                reste_dans_le_repertoire(&nom, &saisi);
                assert_eq!(
                    nom.as_str(),
                    saisi,
                    "un nom accepté n'est pas réécrit — même pour l'octet {octet:#04x}"
                );
            }
        }
    }
    assert!(
        acceptes > 300,
        "ce balayage doit **accepter** l'essentiel des octets imprimables, sinon il ne prouve \
         rien : un `nouveau` qui refuse tout passerait. Acceptés : {acceptes}"
    );
}

#[test]
fn un_nom_trop_long_est_refuse_en_disant_la_borne() {
    // Le fichier composé porte le nom **plus** l'extension, et `NAME_MAX` vaut 255 octets sur les
    // systèmes de fichiers de Linux. Un nom sans borne ne fait pas sortir du répertoire, mais fait
    // échouer l'écriture au moment le plus inutile — après que l'utilisateur a composé son
    // ambiance —, avec un `ENAMETOOLONG` que rien ne relie au nom qu'il a tapé.
    // La borne est lue dans une variable, et non comparée en place : `clippy` refuse une assertion
    // dont il sait la valeur à la compilation, et c'est justement une constante qu'on veut ici
    // encadrer par deux bornes.
    let borne = NomProfil::MAX_OCTETS;
    assert!(
        borne + NomProfil::EXTENSION.len() <= 255,
        "nom et extension doivent tenir sous NAME_MAX : {borne} + {} octets",
        NomProfil::EXTENSION.len()
    );
    assert!(
        borne >= 64,
        "la borne doit rester généreuse : « soirée d'été chez les voisins » fait déjà 30 octets, \
         et un accent en coûte deux. Obtenu : {borne}"
    );

    let juste = "n".repeat(borne);
    assert_eq!(
        accepte(&juste).as_str(),
        juste,
        "un nom exactement à la borne est accepté : la borne est un maximum, pas un interdit"
    );

    let trop = "n".repeat(borne + 1);
    let erreur = refuse(&trop);
    assert!(
        erreur.raison.contains(&NomProfil::MAX_OCTETS.to_string()),
        "la raison doit dire la borne — « nom trop long » sans le chiffre fait tâtonner. \
         Raison obtenue : {}",
        erreur.raison
    );

    // La borne est en **octets**, pas en caractères : c'est ce que compte le système de fichiers.
    // Un nom d'accents fait deux octets par lettre, et un compte en `chars()` le laisserait
    // passer au double de la borne.
    let accents = "é".repeat(borne);
    assert!(
        NomProfil::nouveau(&accents).is_err(),
        "« {} » fait {} octets pour {} caractères : la borne doit compter les octets, comme le \
         système de fichiers",
        accents.chars().take(4).collect::<String>(),
        accents.len(),
        accents.chars().count()
    );
}

// ---------------------------------------------------------------------------
// L'aller-retour avec le nom de fichier
// ---------------------------------------------------------------------------

#[test]
fn le_nom_se_relit_depuis_son_fichier() {
    // `profil list` lit un répertoire et rend des noms (critère d'acceptation). Il lui faut donc
    // l'inverse de `fichier()`, et il doit être **strict** : un fichier posé à la main dans
    // `/var/lib/reverb/profils` ne doit pas devenir un profil au seul motif qu'il traîne là.
    for saisi in noms_legitimes() {
        let nom = accepte(saisi);
        assert_eq!(
            NomProfil::depuis_fichier(&nom.fichier()).as_ref(),
            Ok(&nom),
            "« {saisi} » doit se relire depuis « {} »",
            nom.fichier()
        );
    }

    for (description, fichier) in [
        ("sans extension", "nuit"),
        ("une autre extension", "nuit.txt"),
        ("l'extension seule", NomProfil::EXTENSION),
        ("un fichier caché du système", ".gitignore"),
        ("un fichier temporaire d'écriture", "nuit.conf.tmp"),
        ("un nom interdit affublé de l'extension", "...conf"),
    ] {
        assert!(
            NomProfil::depuis_fichier(fichier).is_err(),
            "{description} : « {fichier} » n'est pas un profil et ne doit pas en devenir un"
        );
    }
}

#[test]
fn les_noms_se_trient() {
    // `profil list` « donne les noms connus » (critère d'acceptation). L'ordre de `read_dir` est
    // celui du système de fichiers, qui n'est pas stable : sans `Ord` sur le nom, la liste change
    // d'ordre entre deux appels sans que rien n'ait bougé.
    let mut noms: Vec<NomProfil> = ["nuit", "aube", "zénith", "Noël"]
        .into_iter()
        .map(accepte)
        .collect();
    noms.sort();
    let lus: Vec<&str> = noms.iter().map(NomProfil::as_str).collect();
    let mut attendu = vec!["nuit", "aube", "zénith", "Noël"];
    attendu.sort_unstable();
    assert_eq!(
        lus, attendu,
        "les noms se trient comme les chaînes qu'ils sont"
    );

    assert_eq!(
        accepte("nuit"),
        accepte("nuit"),
        "deux fois le même nom sont égaux"
    );
    assert_ne!(
        accepte("nuit"),
        accepte("Nuit"),
        "la casse distingue deux fichiers sous Linux : elle doit distinguer deux profils"
    );
}

#[test]
fn un_nom_accepte_traverse_une_ligne_du_socket() {
    // L'IPC est « texte, une ligne par requête » (ADR-002), bornée par `ipc::MAX_LINE_LEN`. Un nom
    // qu'on peut enregistrer mais pas rappeler par le socket serait un profil inaccessible depuis
    // la fenêtre comme depuis la ligne de commande.
    //
    // C'est aussi la justification du refus des caractères de contrôle au-delà des deux que
    // l'issue nomme : un `\r` coupe une ligne exactement comme un `\n`.
    for saisi in noms_legitimes() {
        let nom = accepte(saisi);
        assert!(
            !nom.as_str().chars().any(char::is_control),
            "« {saisi} » : aucun caractère de contrôle ne doit survivre à la validation"
        );
    }

    let requete = format!("profil load {}", "n".repeat(NomProfil::MAX_OCTETS));
    assert!(
        requete.len() <= MAX_LINE_LEN,
        "la borne du nom ({} octets) doit laisser la requête la plus longue tenir sur une ligne \
         du socket ({MAX_LINE_LEN} octets) : {} octets",
        NomProfil::MAX_OCTETS,
        requete.len()
    );
}
