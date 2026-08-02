//! Tests d'intention de l'unité systemd du démon — issue #62, « Le démon ne voit aucun fichier de
//! l'utilisateur : l'écran refuse toute image ».
//!
//! Écrits **depuis l'issue seule**, avant correction : le fichier livré porte encore
//! `ProtectHome=yes` et `PrivateTmp=yes`, donc cette suite doit être rouge tant que la correction
//! n'est pas faite. Si l'un de ces tests échoue après correction, c'est l'unité qu'on corrige.
//!
//! ## Ce qu'ils gardent
//!
//! Le démon lit des images **nommées par l'utilisateur** : `screen image <chemin>` lui passe un
//! chemin, à charge pour lui d'ouvrir le fichier (ADR-002 — la fenêtre n'ouvre aucun périphérique
//! et ne lit rien elle-même, le mégaoctet ne traverse jamais le socket). Deux durcissements de
//! l'unité rendent cette lecture impossible :
//!
//! - `ProtectHome=yes` monte un tmpfs vide sur `/home` — donc sur `/var/home`, dont `/home` est le
//!   lien sur cette machine. Le dossier personnel devient un dossier vide ;
//! - `PrivateTmp=yes` donne au démon un `/tmp` à lui, distinct de celui où l'utilisateur range ce
//!   qu'il vient de télécharger.
//!
//! **Le mode de défaillance est trompeur, et c'est ce qui le rend coûteux** : le démon répond
//! « No such file or directory » sur un fichier qui existe, ce qui accuse l'utilisateur d'avoir mal
//! tapé son chemin. Rien dans le journal ne dit que le démon est aveugle. D'où des tests sur le
//! **contenu de l'unité livrée** plutôt que sur le message d'erreur : la cause est ici, pas là-bas.
//!
//! `ProtectHome=read-only` donne exactement ce qu'il faut — lecture oui, écriture non. Le démon n'a
//! jamais besoin d'écrire dans le dossier personnel : il lit, décode, pousse les pixels. Son état à
//! lui vit dans `/var/lib/reverb` (`StateDirectory`) et son socket dans `/run/reverb`
//! (`RuntimeDirectory`) — deux dossiers que systemd lui crée, et les seuls où il écrit.
//!
//! ## Ce qu'ils ne testent pas, et pourquoi
//!
//! Ni le démon, ni le matériel, ni la machine qui exécute la suite : ces tests lisent deux fichiers
//! texte **versionnés dans le dépôt**, jamais `/etc/systemd/system`. Une unité corrigée dans le
//! dépôt et jamais réinstallée est justement le scénario que le dernier test couvre, du côté de
//! `tools/installe.sh`.
//!
//! Ils ne vérifient pas non plus qu'une image du dossier personnel s'affiche vraiment : c'est un
//! critère de l'issue, mais il demande un Kraken branché et un démon en cours. Il se vérifie à la
//! main, avec `systemd-run --property=ProtectHome=…` comme l'issue le montre.
//!
//! ## Convention d'analyse
//!
//! Une directive est dite **effective** si systemd la lirait : ligne ni vide ni commentée (`#` ou
//! `;`), et située dans la section `[Service]`. C'est la nuance qui fait tout l'intérêt du fichier
//! d'analyse ci-dessous plutôt qu'un `contains()` sur le texte brut : l'unité **mentionne**
//! `ProtectKernelTunables` et `chown` dans ses commentaires, précisément pour dire de ne pas les
//! employer. Un test naïf y verrait des directives, et échouerait pour une raison fausse — ou pire,
//! passerait parce qu'une correction a été écrite en commentaire.
//!
//! La continuation de ligne par `\` n'est pas gérée : l'unité n'en emploie aucune, et une directive
//! ainsi coupée serait vue comme deux lignes dont la seconde sans `=`, donc ignorée.

/// Chemins des deux fichiers examinés, depuis la racine du dépôt.
const CHEMIN_UNITE: &str = "packaging/reverbd.service";
const CHEMIN_INSTALLATEUR: &str = "tools/installe.sh";

/// La seule section dont les directives sont appliquées au processus. Une `ProtectHome=read-only`
/// posée dans `[Unit]` serait inerte, et le bug survivrait à sa propre correction.
const SECTION_APPLIQUEE: &str = "Service";

/// Les clés dont cette suite fixe la valeur — ou l'absence. Elles servent aussi au test d'unicité :
/// systemd garde la **dernière** affectation d'une clé simple, donc une seconde `ProtectHome=yes`
/// écrite plus bas annulerait silencieusement la correction, tout en laissant la bonne ligne
/// visible à la relecture.
const CLES_TESTEES: [&str; 9] = [
    "ProtectHome",
    "PrivateTmp",
    "ProtectKernelTunables",
    "User",
    "Group",
    "UMask",
    "StateDirectory",
    "RuntimeDirectory",
    "NoNewPrivileges",
];

/// Une directive telle que systemd la lirait, avec de quoi la situer dans un message d'échec.
struct Directive {
    /// Numéro de ligne, 1-indexé — pour qu'un échec envoie directement au bon endroit.
    numero: usize,
    /// Section courante au moment de la lecture (`Unit`, `Service`, `Install`).
    section: String,
    cle: String,
    valeur: String,
}

fn racine_du_depot() -> std::path::PathBuf {
    let racine = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // Uniquement pour un message d'échec lisible : si la racine n'est pas canonisable, on garde le
    // chemin brut, qui reste exact.
    match racine.canonicalize() {
        Ok(canonique) => canonique,
        Err(_) => racine,
    }
}

fn chemin(relatif: &str) -> std::path::PathBuf {
    racine_du_depot().join(relatif)
}

/// Lit un fichier du dépôt. Pas d'`unwrap()` silencieux : si le fichier manque, l'échec doit nommer
/// le chemin cherché, sinon on cherche un bug d'unité là où c'est le test qui vise à côté.
fn charge(relatif: &str) -> String {
    let complet = chemin(relatif);
    match std::fs::read_to_string(&complet) {
        Ok(contenu) => contenu,
        Err(erreur) => panic!(
            "Fichier introuvable ou illisible.\n  \
             Chemin cherché : {}\n  \
             Cause système  : {erreur}\n  \
             Ce fichier est livré par le dépôt : c'est lui qui est installé dans \
             /etc/systemd/system par tools/installe.sh.",
            complet.display()
        ),
    }
}

/// Analyse une unité systemd et rend ses directives **effectives**, commentaires et lignes vides
/// écartés, chacune rattachée à sa section.
fn directives(contenu: &str) -> Vec<Directive> {
    let mut section = String::new();
    let mut lues = Vec::new();

    for (index, brute) in contenu.lines().enumerate() {
        let ligne = brute.trim();

        // systemd accepte `#` et `;` comme marqueurs de commentaire en début de ligne. Les deux
        // sont ignorés ici : une directive correcte mais commentée n'est pas une directive.
        if ligne.is_empty() || ligne.starts_with('#') || ligne.starts_with(';') {
            continue;
        }

        if let Some(nom) = ligne.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            section = nom.trim().to_string();
            continue;
        }

        // Une ligne sans `=` n'est pas une affectation : on l'ignore plutôt que de l'interpréter.
        if let Some((cle, valeur)) = ligne.split_once('=') {
            lues.push(Directive {
                numero: index + 1,
                section: section.clone(),
                // systemd tolère les espaces autour du `=` : `UMask = 0007` vaut `UMask=0007`.
                cle: cle.trim().to_string(),
                valeur: valeur.trim().to_string(),
            });
        }
    }

    lues
}

/// Toutes les affectations effectives d'une clé dans `[Service]`. Rendre la liste, et non la
/// dernière valeur, est ce qui permet de dénoncer un doublon au lieu de le subir.
fn affectations<'a>(directives: &'a [Directive], cle: &str) -> Vec<&'a Directive> {
    directives
        .iter()
        .filter(|d| d.section == SECTION_APPLIQUEE && d.cle == cle)
        .collect()
}

/// La valeur effective d'une clé, ou `None` si elle n'est affectée nulle part dans `[Service]`.
/// En cas de doublon, c'est la dernière qui est rendue — comme systemd la lirait. Le doublon
/// lui-même est dénoncé par son propre test.
fn valeur<'a>(directives: &'a [Directive], cle: &str) -> Option<&'a str> {
    affectations(directives, cle)
        .last()
        .map(|d| d.valeur.as_str())
}

/// Recopie la section `[Service]` dans un message d'échec, pour qu'on voie ce qui est livré sans
/// rouvrir le fichier.
fn recopie_du_service(directives: &[Directive]) -> String {
    let lignes: String = directives
        .iter()
        .filter(|d| d.section == SECTION_APPLIQUEE)
        .map(|d| format!("    ligne {} : {}={}\n", d.numero, d.cle, d.valeur))
        .collect();
    if lignes.is_empty() {
        format!("    (aucune directive effective en [{SECTION_APPLIQUEE}])\n")
    } else {
        lignes
    }
}

/// Vérifie qu'une clé porte exactement la valeur attendue, et rend la faute constatée s'il y en a
/// une. Le message nomme la valeur trouvée : « assertion failed » sur un fichier de configuration
/// oblige à rouvrir le fichier pour comprendre.
fn faute_de_valeur(
    directives: &[Directive],
    cle: &str,
    attendue: &str,
    pourquoi: &str,
) -> Option<String> {
    match valeur(directives, cle) {
        Some(trouvee) if trouvee == attendue => None,
        Some(trouvee) => Some(format!(
            "    {cle} vaut « {trouvee} », attendu « {attendue} » — {pourquoi}\n"
        )),
        None => Some(format!(
            "    {cle} est absente de [{SECTION_APPLIQUEE}], attendu « {attendue} » — {pourquoi}\n"
        )),
    }
}

/// Lignes actives d'un script shell : ni vides, ni commentaires. Le `#!` de la première ligne est
/// un commentaire pour le test comme pour le shell, ce qui convient.
fn lignes_actives(contenu: &str) -> Vec<(usize, &str)> {
    contenu
        .lines()
        .enumerate()
        .map(|(index, ligne)| (index + 1, ligne.trim()))
        .filter(|(_, ligne)| !ligne.is_empty() && !ligne.starts_with('#'))
        .collect()
}

/// Numéro de la première ligne active satisfaisant tous les fragments donnés. Chercher des
/// fragments plutôt qu'une commande exacte laisse passer un `sudo`, un chemin absolu ou une option
/// ajoutée, sans laisser passer l'absence de la commande.
fn premiere_ligne_avec(actives: &[(usize, &str)], fragments: &[&str]) -> Option<usize> {
    actives
        .iter()
        .find(|(_, ligne)| fragments.iter().all(|f| ligne.contains(f)))
        .map(|(numero, _)| *numero)
}

fn recopie_des_lignes_actives(actives: &[(usize, &str)]) -> String {
    actives
        .iter()
        .map(|(numero, ligne)| format!("    ligne {numero} : {ligne}\n"))
        .collect()
}

#[test]
fn l_unite_du_demon_est_livree_par_le_depot() {
    let contenu = charge(CHEMIN_UNITE);
    assert!(
        !contenu.trim().is_empty(),
        "Le fichier {} existe mais est vide.\n  \
         C'est lui que tools/installe.sh pose dans /etc/systemd/system : sans lui, le démon ne \
         démarre pas au boot et l'éclairage ne survit pas au redémarrage.",
        chemin(CHEMIN_UNITE).display()
    );
}

#[test]
fn l_unite_declare_une_section_service() {
    // Ce test garde l'analyse elle-même, pas l'unité : toutes les vérifications qui suivent
    // filtrent sur `[Service]`. Si la section disparaissait ou était renommée, elles passeraient
    // toutes à vide sur les absences et échoueraient toutes sur les présences — un signal illisible.
    let contenu = charge(CHEMIN_UNITE);
    let lues = directives(&contenu);
    let compte = lues
        .iter()
        .filter(|d| d.section == SECTION_APPLIQUEE)
        .count();

    assert!(
        compte > 0,
        "Aucune directive effective en [{SECTION_APPLIQUEE}] dans {}.\n  \
         Les directives des autres sections ne s'appliquent pas au processus : systemd les lit \
         pour lui-même. Sections rencontrées : {}\n",
        chemin(CHEMIN_UNITE).display(),
        {
            let mut vues: Vec<&str> = lues.iter().map(|d| d.section.as_str()).collect();
            vues.dedup();
            if vues.is_empty() {
                "(aucune)".to_string()
            } else {
                vues.join(", ")
            }
        }
    );
}

#[test]
fn le_dossier_personnel_est_lisible_mais_pas_inscriptible() {
    // Critères de l'issue #62 : « packaging/reverbd.service porte ProtectHome=read-only » et « le
    // démon ne peut pas écrire dans le dossier personnel ».
    //
    // Les trois états possibles se distinguent, et deux sont des pannes :
    // - `yes` (ou `tmpfs`) : un tmpfs vide masque /home, donc /var/home. Le démon est aveugle et
    //   répond ENOENT sur des fichiers qui existent — le bug de l'issue ;
    // - absente : la protection par défaut d'une unité système est nulle. Le démon tourne en root
    //   et pourrait écrire, voire effacer, dans le dossier personnel. Il n'en a jamais besoin ;
    // - `read-only` : lecture oui, écriture non. C'est exactement le besoin, et c'est vérifié dans
    //   l'issue par `systemd-run --property=ProtectHome=read-only`.
    let contenu = charge(CHEMIN_UNITE);
    let lues = directives(&contenu);

    let diagnostic = match valeur(&lues, "ProtectHome") {
        Some("read-only") => None,
        Some(trouvee) => Some(format!(
            "ProtectHome vaut « {trouvee} » au lieu de « read-only ».\n  \
             Avec « yes » ou « tmpfs », systemd monte un tmpfs vide sur /home — donc sur /var/home, \
             dont /home est le lien sur cette machine. Le démon ne voit AUCUN fichier de \
             l'utilisateur, et `screen image <chemin>` répond « No such file or directory » sur un \
             fichier qui existe."
        )),
        None => Some(
            "ProtectHome n'est affectée nulle part dans [Service].\n  \
             Sans elle, le démon — qui tourne en root — peut écrire et effacer dans le dossier \
             personnel, alors qu'il n'a jamais qu'à lire. Une ligne commentée ne compte pas : \
             systemd ne lit pas les commentaires, ce test non plus."
                .to_string(),
        ),
    };

    assert!(
        diagnostic.is_none(),
        "{}\n  Fichier : {}\n  Section [{SECTION_APPLIQUEE}] telle qu'elle est livrée :\n{}",
        diagnostic.unwrap_or_default(),
        chemin(CHEMIN_UNITE).display(),
        recopie_du_service(&lues)
    );
}

#[test]
fn tmp_n_est_pas_isole_du_reste_du_systeme() {
    // Critère de l'issue #62 : « packaging/reverbd.service … n'a plus PrivateTmp ».
    //
    // /tmp est l'autre endroit où atterrit une image qu'on veut afficher tout de suite. Avec
    // `PrivateTmp=yes`, le démon a un /tmp à lui, vide de tout ce que l'utilisateur y a mis : même
    // panne que ProtectHome, même message trompeur.
    let contenu = charge(CHEMIN_UNITE);
    let lues = directives(&contenu);
    let posees = affectations(&lues, "PrivateTmp");

    let fautives: String = posees
        .iter()
        .map(|d| format!("    ligne {} : {}={}\n", d.numero, d.cle, d.valeur))
        .collect();

    assert!(
        posees.is_empty(),
        "L'unité {} isole /tmp :\n{}  \
         `PrivateTmp` doit être retirée, pas mise à « no » : son absence est déjà son défaut, et \
         l'écrire laisserait croire qu'un arbitrage a été fait alors qu'il n'y en a pas.\n  \
         Section [{SECTION_APPLIQUEE}] telle qu'elle est livrée :\n{}",
        chemin(CHEMIN_UNITE).display(),
        fautives,
        recopie_du_service(&lues)
    );
}

#[test]
fn l_unite_conserve_ce_qui_donne_au_socket_son_proprietaire() {
    // Critère de l'issue #62 : « l'unité livrée conserve User=root, Group=reverb, UMask=0007 ».
    //
    // Ces trois-là tiennent ensemble et n'ont de sens qu'ensemble (ADR-002) : `User=root` pour les
    // écritures hwmon des ventilateurs, `Group=reverb` pour que le socket naisse au bon groupe SANS
    // `chown` après coup, `UMask=0007` pour qu'il naisse fermé aux « autres » dès le premier
    // instant. Les relâcher ouvrirait l'éclairage et les ventilateurs à tout process local, le
    // temps d'une fenêtre ou pour de bon.
    let contenu = charge(CHEMIN_UNITE);
    let lues = directives(&contenu);

    let fautes: String = [
        (
            "User",
            "root",
            "les écritures hwmon des ventilateurs l'exigent",
        ),
        (
            "Group",
            "reverb",
            "c'est ce qui donne au socket son groupe sans `chown` après coup, donc sans fenêtre \
             pendant laquelle il serait ouvert à tous",
        ),
        (
            "UMask",
            "0007",
            "sans lui le socket naît lisible par les « autres » : l'éclairage et les ventilateurs \
             deviennent pilotables par tout process local",
        ),
    ]
    .into_iter()
    .filter_map(|(cle, attendue, pourquoi)| faute_de_valeur(&lues, cle, attendue, pourquoi))
    .collect();

    assert!(
        fautes.is_empty(),
        "L'unité {} a perdu ce qui donne au socket son propriétaire :\n{}  \
         Section [{SECTION_APPLIQUEE}] telle qu'elle est livrée :\n{}",
        chemin(CHEMIN_UNITE).display(),
        fautes,
        recopie_du_service(&lues)
    );
}

#[test]
fn l_unite_conserve_ses_deux_repertoires() {
    // Critère de l'issue #62 : « l'unité livrée conserve … StateDirectory ».
    //
    // Les deux répertoires ont des durées de vie opposées, et c'est tout leur intérêt :
    // `RuntimeDirectory` crée /run/reverb au démarrage et l'efface à l'arrêt — le socket n'a rien à
    // faire là après ; `StateDirectory` crée /var/lib/reverb et NE l'efface pas — c'est ce qui doit
    // traverser un redémarrage pour que le boîtier retrouve son éclairage.
    let contenu = charge(CHEMIN_UNITE);
    let lues = directives(&contenu);

    let fautes: String = [
        (
            "RuntimeDirectory",
            "reverb",
            "/run/reverb porte le socket ; sans cette ligne, le démon n'a nulle part où l'ouvrir",
        ),
        (
            "StateDirectory",
            "reverb",
            "/var/lib/reverb porte l'éclairage, les zones et l'écran ; sans cette ligne, rien ne \
             survit au redémarrage",
        ),
    ]
    .into_iter()
    .filter_map(|(cle, attendue, pourquoi)| faute_de_valeur(&lues, cle, attendue, pourquoi))
    .collect();

    assert!(
        fautes.is_empty(),
        "L'unité {} a perdu l'un de ses deux répertoires :\n{}  \
         Section [{SECTION_APPLIQUEE}] telle qu'elle est livrée :\n{}",
        chemin(CHEMIN_UNITE).display(),
        fautes,
        recopie_du_service(&lues)
    );
}

#[test]
fn l_unite_conserve_l_interdiction_d_elever_ses_privileges() {
    // Critère de l'issue #62 : « l'unité livrée conserve … NoNewPrivileges ».
    //
    // Le démon tourne en root : assouplir ProtectHome ne doit pas devenir l'occasion de relâcher
    // ce qui reste. `NoNewPrivileges=yes` est le durcissement qui ne coûte rien ici — le démon
    // n'exécute aucun autre programme.
    let contenu = charge(CHEMIN_UNITE);
    let lues = directives(&contenu);

    let faute = faute_de_valeur(
        &lues,
        "NoNewPrivileges",
        "yes",
        "le démon n'exécute aucun autre programme : ce durcissement ne lui coûte rien et doit \
         rester",
    );

    assert!(
        faute.is_none(),
        "L'unité {} a relâché son durcissement :\n{}  \
         Section [{SECTION_APPLIQUEE}] telle qu'elle est livrée :\n{}",
        chemin(CHEMIN_UNITE).display(),
        faute.unwrap_or_default(),
        recopie_du_service(&lues)
    );
}

#[test]
fn l_unite_ne_monte_pas_sys_en_lecture_seule() {
    // Critère de l'issue #62 : « l'unité livrée … n'a pas ProtectKernelTunables ».
    //
    // C'est le durcissement qu'on ne peut pas prendre : il monte /sys en lecture seule, ce qui coupe
    // précisément le réglage des ventilateurs — le seul motif pour lequel le démon est root.
    // Le fichier l'interdit déjà en commentaire ; ce test rend l'interdiction exécutable, parce
    // qu'un commentaire ne survit pas à un durcissement « de bon sens » ajouté six mois plus tard.
    //
    // ⚠️ Ce test est aussi le contre-exemple qui justifie l'analyse par directives : le mot
    // `ProtectKernelTunables` EST présent dans le fichier, dans l'avertissement qui l'interdit.
    // Un `contains()` sur le texte brut échouerait sur un fichier parfaitement correct.
    let contenu = charge(CHEMIN_UNITE);
    let lues = directives(&contenu);
    let posees = affectations(&lues, "ProtectKernelTunables");

    let fautives: String = posees
        .iter()
        .map(|d| format!("    ligne {} : {}={}\n", d.numero, d.cle, d.valeur))
        .collect();

    assert!(
        posees.is_empty(),
        "L'unité {} durcit l'accès à /sys :\n{}  \
         `ProtectKernelTunables` monte /sys en lecture seule. Les écritures hwmon (`pwm*`, courbes \
         du Kraken) échouent alors en silence côté utilisateur : `reverb fan` et `reverb curve` \
         cessent d'agir. Le fichier l'interdit déjà en commentaire — ce test le rend exécutable.\n  \
         Section [{SECTION_APPLIQUEE}] telle qu'elle est livrée :\n{}",
        chemin(CHEMIN_UNITE).display(),
        fautives,
        recopie_du_service(&lues)
    );
}

#[test]
fn aucune_directive_testee_n_est_ecrite_deux_fois() {
    // Ce test ne vient pas d'un critère de l'issue mais du mode de défaillance que la correction
    // introduit : sur une clé simple, systemd garde la DERNIÈRE affectation. Une `ProtectHome=yes`
    // laissée plus bas dans le fichier annulerait la correction tout en laissant la bonne ligne
    // bien visible à la relecture — et les tests ci-dessus, qui lisent eux aussi la dernière,
    // diraient vrai sur un fichier ambigu. Une seule affectation par clé, ou rien.
    let contenu = charge(CHEMIN_UNITE);
    let lues = directives(&contenu);

    let doublons: String = CLES_TESTEES
        .iter()
        .filter_map(|cle| {
            let posees = affectations(&lues, cle);
            if posees.len() <= 1 {
                return None;
            }
            let ou: Vec<String> = posees
                .iter()
                .map(|d| format!("ligne {} ({}={})", d.numero, d.cle, d.valeur))
                .collect();
            Some(format!("    {cle} : {}\n", ou.join(", ")))
        })
        .collect();

    assert!(
        doublons.is_empty(),
        "Des clés sont affectées plusieurs fois dans [{SECTION_APPLIQUEE}] de {} :\n{}  \
         systemd retient la dernière : la première ligne devient un commentaire qui n'en a pas \
         l'air, et une correction peut être annulée sans que rien ne le montre.",
        chemin(CHEMIN_UNITE).display(),
        doublons
    );
}

#[test]
fn l_installateur_pose_l_unite_du_depot() {
    // Critère de l'issue #62 : « tools/installe.sh repose l'unité … pour que la correction prenne
    // sans commande manuelle ». Premier temps : l'unité corrigée doit bien être copiée. Un
    // installateur qui ne la reposerait pas laisserait la machine sur l'ancienne version, corrigée
    // dans le dépôt et nulle part ailleurs.
    let contenu = charge(CHEMIN_INSTALLATEUR);
    let actives = lignes_actives(&contenu);

    let pose = premiere_ligne_avec(&actives, &[CHEMIN_UNITE, "/etc/systemd/system"]);

    assert!(
        pose.is_some(),
        "Aucune ligne active de {} ne copie {CHEMIN_UNITE} dans /etc/systemd/system.\n  \
         L'installateur est rejouable, et c'est ce qui doit propager une unité corrigée. Sans cette \
         copie, la correction de l'issue #62 reste dans le dépôt.\n  \
         Lignes actives du script :\n{}",
        chemin(CHEMIN_INSTALLATEUR).display(),
        recopie_des_lignes_actives(&actives)
    );
}

#[test]
fn l_installateur_redemarre_le_service_apres_avoir_pose_l_unite() {
    // Critère de l'issue #62 : « tools/installe.sh repose l'unité ET REDÉMARRE le service, pour que
    // la correction prenne sans commande manuelle ».
    //
    // L'ordre est ce qui compte, et il en faut trois dans cet ordre :
    //   1. copier l'unité ;
    //   2. `daemon-reload` — sans lui systemd relance l'ancienne définition, encore en mémoire ;
    //   3. `restart` — sans lui le démon en cours garde ses anciens montages, et reste aveugle.
    // Un `reload` fait avant la copie, ou un `restart` fait avant le `reload`, ne relance rien de
    // neuf tout en donnant l'impression que si.
    let contenu = charge(CHEMIN_INSTALLATEUR);
    let actives = lignes_actives(&contenu);

    let pose = premiere_ligne_avec(&actives, &[CHEMIN_UNITE, "/etc/systemd/system"]);
    let relecture = premiere_ligne_avec(&actives, &["daemon-reload"]);
    let redemarrage = premiere_ligne_avec(&actives, &["systemctl", "restart", "reverbd"]);

    let mut fautes = String::new();
    if relecture.is_none() {
        fautes.push_str(
            "    aucun `systemctl daemon-reload` — systemd relancerait l'ancienne définition, \
             encore en mémoire\n",
        );
    }
    if redemarrage.is_none() {
        fautes.push_str(
            "    aucun `systemctl restart reverbd` — le démon en cours garderait ses anciens \
             montages, et resterait aveugle au dossier personnel jusqu'au prochain redémarrage \
             de la machine\n",
        );
    }
    if let (Some(pose), Some(relecture)) = (pose, relecture)
        && relecture < pose
    {
        fautes.push_str(&format!(
            "    `daemon-reload` (ligne {relecture}) précède la copie de l'unité (ligne {pose}) : \
             il relit l'ancienne\n"
        ));
    }
    if let (Some(relecture), Some(redemarrage)) = (relecture, redemarrage)
        && redemarrage < relecture
    {
        fautes.push_str(&format!(
            "    `restart` (ligne {redemarrage}) précède `daemon-reload` (ligne {relecture}) : le \
             service redémarre sur l'ancienne définition\n"
        ));
    }

    assert!(
        fautes.is_empty(),
        "{} ne fait pas prendre l'unité qu'il vient de poser :\n{}  \
         Lignes actives du script :\n{}",
        chemin(CHEMIN_INSTALLATEUR).display(),
        fautes,
        recopie_des_lignes_actives(&actives)
    );
}
