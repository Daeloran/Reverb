//! Tests d'intention des courbes matérielles du Kraken (issue #9).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #9, son contrat d'API et
//! `docs/VENTILATEURS.md` seuls. Rien n'est relu depuis
//! `crates/reverb-cli/src/` : à l'écriture de ce fichier, ni `Curve`, ni
//! `CurveError`, ni `set_curve` n'existent.
//!
//! Suite directe de `spec_fans.rs` (#7), dont ce fichier reprend les
//! conventions : fausse arborescence sysfs dans un répertoire temporaire,
//! comparaison de `photographie` / `ecarts` pour vérifier qu'une écriture ne
//! déborde pas, un test par comportement, chacun citant sa source. Les
//! utilitaires sont dupliqués plutôt que partagés : deux fichiers de tests
//! d'intégration ne partagent rien.
//!
//! Comme pour les fichiers de spécification précédents : ces tests encodent ce
//! que le logiciel doit faire, pas ce que le code fait. Si l'un d'eux échoue
//! après implémentation, c'est le code qu'on corrige, jamais le test.
//!
//! # Aucun accès matériel
//!
//! issue #9, critère d'acceptation — « Aucun accès matériel dans les tests
//! automatisés ». Les seuls chemins touchés ici sont ceux d'une arborescence
//! construite dans `std::env::temp_dir()`, effacée à la fin de chaque test.
//! **Rien n'est lu ni écrit sous `/sys`.** Aucune dépendance n'est ajoutée :
//! `std::fs` suffit.
//!
//! # Les trois inconnues restent des inconnues
//!
//! issue #9, « Étape préalable — trois inconnues à lever sur le matériel », et
//! son avertissement de la section « Tests d'intention » : « Un test qui
//! affirmerait "le point 14 correspond à 33 °C" avant la mesure inventerait une
//! connaissance que personne n'a. »
//!
//! Aucun test de ce fichier ne fige :
//!
//! 1. **la valeur de `pwm_enable` qui déclenche le mode courbe** — `2` n'est
//!    qu'un candidat. Conséquence directe ici : `set_curve` est vérifiée
//!    n'écrire **que** les 40 fichiers de points, jamais `pwmN_enable`. La
//!    bascule de mode est une décision séparée, qui attend la mesure ;
//! 2. **l'appariement `temp1_*` ↔ pompe et `temp2_*` ↔ ventilateur** — les
//!    tests de découverte vérifient qu'un canal porte *ses* 40 fichiers, qu'ils
//!    proviennent tous du même indice de température et que les deux canaux du
//!    Kraken n'en partagent aucun. Jamais qu'un canal donné porte `temp1` ;
//! 3. **la température de chaque point** — les tests ne parlent que d'**index
//!    de point** (1 à `CURVE_POINTS`), l'unité que manipule le contrat d'API.
//!    Aucune conversion °C ↔ index n'est supposée.
//!
//! # Ce que ces tests ne vérifient pas
//!
//! - **L'exécution de la courbe par le firmware.** « la pompe suit la
//!   température du liquide sans que Reverb tourne » se constate sur le
//!   matériel, pas dans un test automatisé. Ce qui est vérifié ici, c'est que
//!   les octets écrits dans les 40 fichiers sont les bons, et qu'ils ne sont
//!   écrits que là.
//! - **La ligne de commande.** `reverb curve`, `--point`, `--force`,
//!   `reverb fan --auto`, l'affichage du mode courbe par `reverb fans` : le
//!   contrat d'API de cette issue décrit `hwmon.rs`, aucune signature ne
//!   correspond à la surface CLI. Même arbitrage qu'en #7. L'issue le dit
//!   elle-même : « La ligne de commande n'est pas l'enjeu de cette issue ».
//! - **Le critère « une température hors de la plage du matériel produit une
//!   erreur qui donne la plage ».** Il porte sur des degrés, donc sur la
//!   troisième inconnue. Ce qui en est testable aujourd'hui, c'est son
//!   équivalent en index de point : `CurveError::OutOfRange`, pour un index
//!   hors de `1..=CURVE_POINTS`, avec la plage dans le message.
//! - **Le plancher de 20 %.** Comme en #7, il vit dans la commande et pas dans
//!   le type — voir l'arbitrage ci-dessous.
//!
//! # Arbitrages de contrat
//!
//! - **Le plancher ne s'applique pas dans `Curve::interpolate`.** `CurveError`
//!   énumère quatre variantes, aucune ne parle du plancher, et le commentaire
//!   de contrat de #7 tranche déjà : « Le plancher vit dans la commande, pas
//!   dans le type. `Percent::new(5)` réussit. Si le type refusait, `--force`
//!   n'aurait aucun moyen de s'exprimer. » Une courbe sous le plancher est donc
//!   interpolable ; c'est `reverb curve` qui la refuse sans `--force`.
//! - **Deux points identiques à l'identique sont acceptés.** Le contrat définit
//!   `Conflicting` par « deux points portent le même index avec des consignes
//!   **différentes** ». Le doublon exact ne porte aucune contradiction ; le
//!   refuser gênerait un appelant graphique qui relaie les points d'une courbe
//!   dessinée. Interprétation, à confirmer.
//! - **`from` est le point de température la plus basse** dans
//!   `CurveError::Decreasing`, quel que soit l'ordre où l'appelant les a
//!   donnés : le contrat décrit la faute comme « la consigne baisse quand la
//!   température **monte** ».
//! - **`CurveError` porte un message** (`Display`), comme `PercentError` en #7 :
//!   le critère d'acceptation « avec un message citant les deux points
//!   fautifs » n'a pas d'autre point d'appui.
//! - Les types publics sont attendus `Debug`, et `Percent` est `Copy` (établi
//!   en #7), pour que les échecs soient lisibles.
//!
//! # La permission réelle des fichiers de courbe
//!
//! issue #9 — sur le matériel, `tempN_auto_pointM_pwm` est en mode `0200`,
//! écriture seule. **C'est un fait du pilote `nzxt_kraken3`, pas une propriété
//! que Reverb produit** : rien dans ce dépôt ne crée ces fichiers ni ne choisit
//! leurs droits. La fausse arborescence les crée donc lisibles, sans quoi les
//! tests d'écriture n'auraient aucun moyen de vérifier ce qui a été écrit. La
//! conséquence de l'écriture seule est ailleurs, et elle est testée : on écrit
//! les 40 points d'un bloc, et rien ne relit une courbe.

use std::fs;
use std::path::{Path, PathBuf};

use reverb_cli::hwmon::{self, CURVE_POINTS, Curve, CurveError, FanChannel, Percent};

// ---------------------------------------------------------------------------
// Fausse arborescence sysfs
// ---------------------------------------------------------------------------

/// Une fausse `/sys/class/hwmon` dans un répertoire temporaire.
///
/// Le répertoire porte le nom du test et le PID : deux tests qui tournent en
/// parallèle ne se marchent pas dessus. Il est effacé à la destruction, y
/// compris quand le test échoue.
struct FauxSysfs {
    racine: PathBuf,
}

impl FauxSysfs {
    fn neuf(nom_du_test: &str) -> Self {
        let racine = std::env::temp_dir().join(format!(
            "reverb-spec-courbes-{nom_du_test}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&racine);
        fs::create_dir_all(&racine).expect("création de la racine temporaire");
        Self { racine }
    }

    fn racine(&self) -> &Path {
        &self.racine
    }

    /// Crée `<racine>/<dossier>` et son fichier `name`.
    fn hwmon(&self, dossier: &str, source: &str) -> PathBuf {
        let chemin = self.racine.join(dossier);
        fs::create_dir_all(&chemin).expect("création d'un répertoire hwmon");
        ecrire(&chemin.join("name"), source);
        chemin
    }

    fn canaux(&self) -> Vec<FanChannel> {
        hwmon::discover_in(self.racine()).expect("découverte sur l'arborescence temporaire")
    }
}

impl Drop for FauxSysfs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.racine);
    }
}

/// Écrit un fichier sysfs.
///
/// Le saut de ligne final n'est pas décoratif : le noyau en ajoute un à chaque
/// attribut.
fn ecrire(chemin: &Path, valeur: &str) {
    fs::write(chemin, format!("{valeur}\n")).expect("écriture d'un fichier sysfs simulé");
}

/// Contenu d'un fichier de l'arborescence temporaire, sans son saut de ligne.
fn lire(chemin: &Path) -> String {
    fs::read_to_string(chemin)
        .expect("lecture d'un fichier sysfs simulé")
        .trim()
        .to_owned()
}

/// Photographie de toute l'arborescence : chemin relatif → contenu.
///
/// Sert à vérifier qu'une écriture n'a touché que les fichiers visés. Comparer
/// deux photographies est plus sûr que de relire les quelques fichiers auxquels
/// on aurait pensé : un fichier oublié serait un fichier non surveillé. Avec
/// 40 points par canal, c'est aussi le seul moyen praticable.
fn photographie(racine: &Path) -> Vec<(String, String)> {
    let mut vue = Vec::new();
    let mut a_visiter = vec![racine.to_owned()];
    while let Some(dossier) = a_visiter.pop() {
        for entree in fs::read_dir(&dossier).expect("parcours de l'arborescence temporaire") {
            let chemin = entree.expect("entrée de répertoire").path();
            if chemin.is_dir() {
                a_visiter.push(chemin);
            } else {
                vue.push((relatif(racine, &chemin), lire(&chemin)));
            }
        }
    }
    vue.sort();
    vue
}

/// Les fichiers apparus, disparus ou modifiés entre deux photographies.
fn ecarts(avant: &[(String, String)], apres: &[(String, String)]) -> Vec<String> {
    let mut noms: Vec<String> = apres
        .iter()
        .filter(|(chemin, contenu)| {
            !avant
                .iter()
                .any(|(ancien, valeur)| ancien == chemin && valeur == contenu)
        })
        .map(|(chemin, _)| chemin.clone())
        .collect();
    noms.extend(
        avant
            .iter()
            .filter(|(chemin, _)| !apres.iter().any(|(nouveau, _)| nouveau == chemin))
            .map(|(chemin, _)| chemin.clone()),
    );
    noms.sort();
    noms.dedup();
    noms
}

/// Le chemin d'un fichier, relatif à la racine de l'arborescence temporaire.
fn relatif(racine: &Path, chemin: &Path) -> String {
    chemin
        .strip_prefix(racine)
        .expect("chemin sous la racine")
        .to_string_lossy()
        .into_owned()
}

/// Les 40 fichiers de courbe d'un indice de température, tous à « 0 ».
///
/// La valeur initiale n'a pas de sens matériel — sur l'appareil ces fichiers ne
/// se relisent pas. Elle sert de témoin : après une écriture, tout fichier
/// resté à « 0 » n'a pas été touché.
fn fichiers_de_courbe(hwmon: &Path, temp: u32) {
    for m in 1..=CURVE_POINTS {
        ecrire(&hwmon.join(format!("temp{temp}_auto_point{m}_pwm")), "0");
    }
}

/// L'arborescence de référence, reprise de `docs/VENTILATEURS.md` et de l'état
/// relevé en #7, complétée des fichiers de courbe de l'issue #9.
///
/// issue #9 — « Le pilote `nzxt_kraken3` expose déjà tout :
/// `temp1_auto_point[1-40]_pwm` et `temp2_auto_point[1-40]_pwm` ». Les deux
/// jeux sont créés, sans que rien ici ne décide lequel pilote la pompe : c'est
/// la deuxième inconnue.
///
/// Les autres sources n'ont aucune courbe — issue #9, hors scope : « seul
/// `kraken2023elite` en expose ».
fn arborescence_de_reference(nom_du_test: &str) -> FauxSysfs {
    let sysfs = FauxSysfs::neuf(nom_du_test);

    // docs/VENTILATEURS.md, table du résumé — `nzxtsmart2`, trois canaux
    // « FAN 1/2/3 » en mode manuel. Aucun fichier de courbe : issue #9,
    // critère d'acceptation — « Un canal qui n'a pas de courbe — les trois
    // `nzxtsmart2`, les `nct6687` ».
    let nzxt = sysfs.hwmon("hwmon4", "nzxtsmart2");
    for (n, libelle, regime) in [
        (1, "FAN 1", "725"),
        (2, "FAN 2", "688"),
        (3, "FAN 3", "715"),
    ] {
        ecrire(&nzxt.join(format!("pwm{n}")), "64");
        ecrire(&nzxt.join(format!("pwm{n}_enable")), "1");
        ecrire(&nzxt.join(format!("fan{n}_input")), regime);
        ecrire(&nzxt.join(format!("fan{n}_label")), libelle);
    }

    // docs/VENTILATEURS.md — `kraken2023elite`, « Pump speed » et « Fan speed »,
    // tous deux en courbe firmware (`enable = 0`). issue #7, « État relevé » :
    // `pwm1 = 171`, `pwm2 = 71`.
    let kraken = sysfs.hwmon("hwmon6", "kraken2023elite");
    for (n, libelle, pwm, regime) in [
        (1, "Pump speed", "171", "2380"),
        (2, "Fan speed", "71", "714"),
    ] {
        ecrire(&kraken.join(format!("pwm{n}")), pwm);
        ecrire(&kraken.join(format!("pwm{n}_enable")), "0");
        ecrire(&kraken.join(format!("fan{n}_input")), regime);
        ecrire(&kraken.join(format!("fan{n}_label")), libelle);
    }
    // Les deux courbes à 40 points. issue #9 — « Le Kraken remonte sa
    // température dans `temp1_input` (~33 °C au repos) » : l'entrée de
    // température voisine les points de courbe sans en être un.
    ecrire(&kraken.join("temp1_input"), "31000");
    ecrire(&kraken.join("temp2_input"), "33000");
    fichiers_de_courbe(&kraken, 1);
    fichiers_de_courbe(&kraken, 2);
    // Bruit : les mesures d'intensité que le firmware remonte à zéro
    // (docs/VENTILATEURS.md, « par l'intensité »).
    ecrire(&kraken.join("curr1_input"), "0");

    // Une source sans aucune sortie PWM, mais avec des températures : elle ne
    // donne aucun canal, donc aucune courbe.
    let nvme = sysfs.hwmon("hwmon0", "nvme");
    ecrire(&nvme.join("temp1_input"), "38850");
    ecrire(&nvme.join("temp2_input"), "38850");

    // docs/VENTILATEURS.md, « Les prises de la carte mère sont vides » —
    // `nct6687` n'expose aucun `pwm*_enable`, et pas davantage de courbe.
    let nct = sysfs.hwmon("hwmon2", "nct6687");
    for (n, libelle) in [(1, "CPU FAN"), (2, "SYS FAN 1")] {
        ecrire(&nct.join(format!("pwm{n}")), "153");
        ecrire(&nct.join(format!("fan{n}_input")), "0");
        ecrire(&nct.join(format!("fan{n}_label")), libelle);
    }

    // Un canal dépouillé, qui expose une température d'entrée sans courbe :
    // `temp1_input` ne doit pas suffire à faire croire à une courbe.
    let autre = sysfs.hwmon("hwmon9", "amdgpu");
    ecrire(&autre.join("pwm1"), "128");
    ecrire(&autre.join("pwm1_enable"), "2");
    ecrire(&autre.join("temp1_input"), "45000");

    sysfs
}

/// Le canal nommé `nom`, ou un échec explicite.
fn canal<'a>(canaux: &'a [FanChannel], nom: &str) -> &'a FanChannel {
    canaux
        .iter()
        .find(|c| c.name == nom)
        .unwrap_or_else(|| panic!("le canal « {nom} » doit être découvert"))
}

// ---------------------------------------------------------------------------
// Utilitaires de courbe
// ---------------------------------------------------------------------------

/// La consigne `p`, qui doit être acceptée (contrat de #7 : 0 à 100 %).
fn pct(p: u8) -> Percent {
    Percent::new(p).expect("une consigne de 0 à 100 % est valide")
}

/// Un couple (index de point, consigne), tel que `Curve::interpolate` l'attend.
///
/// Les index sont **1-indexés**, comme les fichiers `..._auto_pointM_pwm`.
fn point(index: usize, pourcent: u8) -> (usize, Percent) {
    (index, pct(pourcent))
}

/// La courbe interpolée depuis `points`, qui doit être acceptée.
fn courbe(points: &[(usize, Percent)]) -> Curve {
    Curve::interpolate(points).expect("cette demande de courbe doit être acceptée")
}

/// Le refus opposé à `points`, qui ne doit pas être acceptée.
fn refus(points: &[(usize, Percent)]) -> CurveError {
    match Curve::interpolate(points) {
        Ok(_) => {
            let demande: Vec<(usize, u8)> = points.iter().map(|(i, p)| (*i, p.percent())).collect();
            panic!("cette demande de courbe doit être refusée : {demande:?}")
        }
        Err(erreur) => erreur,
    }
}

/// Les consignes d'une courbe, en pourcent, dans l'ordre des points.
fn consignes(courbe: &Curve) -> Vec<u8> {
    courbe.points().iter().map(|p| p.percent()).collect()
}

/// La consigne au point `n`, **1-indexé**.
fn au_point(courbe: &Curve, n: usize) -> u8 {
    courbe.points()[n - 1].percent()
}

// ---------------------------------------------------------------------------

mod interpolation {
    use super::{CURVE_POINTS, au_point, consignes, courbe, point};

    #[test]
    fn le_nombre_de_points_est_celui_du_materiel() {
        // issue #9 — « Le firmware du Kraken sait exécuter une courbe […] à
        // **40 points** », et le pilote expose `temp1_auto_point[1-40]_pwm`.
        // Contrat d'API — `pub const CURVE_POINTS: usize = 40`.
        assert_eq!(CURVE_POINTS, 40);
    }

    #[test]
    fn une_courbe_fait_toujours_quarante_consignes() {
        // issue #9, critère d'acceptation — « Les 40 valeurs sont écrites d'un
        // bloc ». Quel que soit le nombre de couples donnés — un seul, deux,
        // les quarante —, la courbe produite en compte toujours 40 : il n'y a
        // pas de courbe partielle à écrire.
        let un_seul = courbe(&[point(7, 40)]);
        let deux = courbe(&[point(1, 20), point(40, 100)]);
        let tous: Vec<(usize, u8)> = (1..=CURVE_POINTS).map(|n| (n, 30)).collect();
        let tous = courbe(
            &tous
                .into_iter()
                .map(|(n, p)| point(n, p))
                .collect::<Vec<_>>(),
        );

        for c in [&un_seul, &deux, &tous] {
            assert_eq!(c.points().len(), CURVE_POINTS);
            assert_eq!(consignes(c).len(), CURVE_POINTS);
        }
    }

    #[test]
    fn deux_points_aux_extremites_donnent_une_rampe_lineaire() {
        // issue #9 — « Quelques couples température:consigne, interpolés
        // **linéairement** sur les 40 points », et critère d'acceptation —
        // « L'interpolation est une fonction pure, testée seule : points aux
        // bornes ».
        //
        // De 20 % au premier point à 100 % au dernier, la progression est de
        // 80 points sur 39 intervalles. L'écart toléré est d'un point de
        // pourcentage : l'arrondi n'est pas tranché par l'issue, la droite si.
        let c = courbe(&[point(1, 20), point(CURVE_POINTS, 100)]);

        assert_eq!(au_point(&c, 1), 20, "la consigne du premier point");
        assert_eq!(au_point(&c, CURVE_POINTS), 100, "celle du dernier");

        for n in 1..=CURVE_POINTS {
            let attendu = 20.0 + 80.0 * ((n - 1) as f64) / ((CURVE_POINTS - 1) as f64);
            let obtenu = f64::from(au_point(&c, n));
            assert!(
                (obtenu - attendu).abs() <= 1.0,
                "point {n} : {obtenu} % pour une droite qui passe par {attendu:.2} %"
            );
        }
    }

    #[test]
    fn un_point_unique_donne_une_courbe_plate() {
        // issue #9, critère d'acceptation — « un seul point ». Avec un seul
        // couple, « avant » et « après » couvrent toute la plage : la courbe
        // vaut cette consigne partout. C'est aussi la forme employée par la
        // mesure de l'étape préalable — « une courbe plate au minimum sauf un
        // seul point à 100 % ».
        for (index, consigne) in [(1usize, 55u8), (17, 55), (CURVE_POINTS, 55)] {
            let c = courbe(&[point(index, consigne)]);
            assert_eq!(
                consignes(&c),
                vec![consigne; CURVE_POINTS],
                "un point unique au point {index} doit aplatir toute la courbe"
            );
        }
    }

    #[test]
    fn les_valeurs_aux_points_fournis_sont_exactement_celles_demandees() {
        // issue #9 — les couples donnés sont l'intention de l'utilisateur ;
        // l'interpolation remplit entre eux, elle ne les corrige pas. Un
        // arrondi qui déplacerait un point fourni rendrait la courbe dessinée
        // à la souris différente de la courbe écrite.
        let demandes = [point(1, 20), point(14, 45), point(30, 80), point(40, 100)];
        let c = courbe(&demandes);

        for (index, consigne) in demandes {
            assert_eq!(
                au_point(&c, index),
                consigne.percent(),
                "le point {index} a été demandé à {} %",
                consigne.percent()
            );
        }
    }

    #[test]
    fn avant_le_premier_point_la_consigne_est_celle_du_premier() {
        // issue #9 — « En deçà du premier point, la valeur du premier ».
        // Contrat d'API — « Avant le premier point, la valeur du premier ».
        // Sans cette règle, les points de basse température resteraient à zéro
        // et la pompe s'arrêterait au repos.
        let c = courbe(&[point(10, 30), point(30, 90)]);

        for n in 1..=10 {
            assert_eq!(
                au_point(&c, n),
                30,
                "le point {n} précède le premier point fourni"
            );
        }
    }

    #[test]
    fn apres_le_dernier_point_la_consigne_est_celle_du_dernier() {
        // issue #9 — « au-delà du dernier, celle du dernier ».
        // Conséquence pratique : une courbe qui s'arrête à mi-plage ne laisse
        // pas le haut de la plage à zéro. Sur une pompe, ce serait la panne la
        // plus coûteuse que puisse produire cette brique.
        let c = courbe(&[point(10, 30), point(30, 90)]);

        for n in 30..=CURVE_POINTS {
            assert_eq!(
                au_point(&c, n),
                90,
                "le point {n} suit le dernier point fourni"
            );
        }
    }

    #[test]
    fn des_points_dans_le_desordre_donnent_la_meme_courbe() {
        // issue #9, critère d'acceptation — « points dans le désordre ».
        // Contrat d'API — « Les points peuvent être donnés dans le désordre ».
        // Le consommateur réel est une interface graphique : l'ordre de
        // création des points à la souris n'est pas celui des températures.
        let ordonnes = courbe(&[point(1, 20), point(14, 45), point(30, 80), point(40, 100)]);
        let desordonnes = courbe(&[point(30, 80), point(1, 20), point(40, 100), point(14, 45)]);

        assert_eq!(consignes(&ordonnes), consignes(&desordonnes));
    }

    #[test]
    fn trois_points_donnent_des_segments_independants() {
        // issue #9 — « interpolés linéairement », par segments : trois couples
        // décrivent deux droites, pas une seule. L'exemple de l'issue en
        // compte trois (`--point 20:30 --point 35:50 --point 45:100`), et ses
        // deux pentes diffèrent.
        //
        // Le cas est choisi pour qu'une interpolation globale du premier au
        // dernier point se voie immédiatement : elle donnerait ~59 % au point
        // 20, là où le segment plat impose 20 %.
        let c = courbe(&[point(1, 20), point(20, 20), point(40, 100)]);

        for n in 1..=20 {
            assert_eq!(au_point(&c, n), 20, "le premier segment est plat");
        }
        assert_eq!(au_point(&c, 40), 100);

        // Milieu du second segment : 20 % + 80 % × 10/20 = 60 %.
        let milieu = f64::from(au_point(&c, 30));
        assert!(
            (milieu - 60.0).abs() <= 1.0,
            "le second segment doit passer par ~60 % au point 30, pas {milieu} %"
        );

        // Et le second segment monte réellement, point après point.
        for n in 21..=40 {
            assert!(
                au_point(&c, n) > au_point(&c, n - 1),
                "le point {n} doit dépasser le point {}",
                n - 1
            );
        }
    }

    #[test]
    fn l_interpolation_n_introduit_aucune_baisse() {
        // Conséquence du critère « une courbe décroissante est refusée » : si
        // l'entrée est croissante, la sortie doit l'être aussi. Un arrondi qui
        // ferait redescendre une consigne d'un point produirait exactement la
        // courbe que la validation prétend interdire, sans passer par elle.
        for demande in [
            vec![point(1, 20), point(40, 100)],
            vec![point(1, 0), point(40, 100)],
            vec![point(3, 21), point(9, 22), point(38, 99)],
            vec![point(1, 20), point(20, 20), point(40, 100)],
        ] {
            let c = courbe(&demande);
            let valeurs = consignes(&c);
            for n in 1..valeurs.len() {
                assert!(
                    valeurs[n] >= valeurs[n - 1],
                    "la courbe redescend entre les points {} et {} : {valeurs:?}",
                    n,
                    n + 1
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------

mod validation {
    use super::{CURVE_POINTS, au_point, consignes, courbe, point, refus};
    use reverb_cli::hwmon::{CurveError, Percent};

    #[test]
    fn une_courbe_strictement_plate_est_acceptee() {
        // issue #9, critère d'acceptation — « Une courbe **décroissante** est
        // refusée ». Décroissante, pas « non strictement croissante » : une
        // consigne constante d'un point au suivant est la forme normale d'un
        // palier, et c'est celle de la courbe de mesure de l'étape préalable.
        let plate = courbe(&[point(1, 50), point(40, 50)]);
        assert_eq!(consignes(&plate), vec![50; CURVE_POINTS]);

        let paliers = courbe(&[point(1, 50), point(20, 50), point(40, 50)]);
        assert_eq!(consignes(&paliers), vec![50; CURVE_POINTS]);
    }

    #[test]
    fn deux_points_qui_descendent_sont_refuses() {
        // issue #9, critère d'acceptation — « **Une courbe décroissante est
        // refusée** […] Une consigne qui baisse quand la température monte est
        // une faute de frappe ou un décalage d'indice, et l'écriture seule rend
        // l'erreur invisible jusqu'à la surchauffe. »
        // Contrat d'API — `CurveError::Decreasing { from, to }`.
        let erreur = refus(&[point(1, 80), point(40, 40)]);

        match erreur {
            CurveError::Decreasing { from, to } => {
                assert_eq!(from.0, 1, "le point de départ de la baisse");
                assert_eq!(from.1.percent(), 80);
                assert_eq!(to.0, 40, "le point d'arrivée de la baisse");
                assert_eq!(to.1.percent(), 40);
            }
            autre => panic!("une courbe descendante doit donner `Decreasing`, pas {autre:?}"),
        }
    }

    #[test]
    fn le_refus_cite_les_deux_points_fautifs() {
        // issue #9, critère d'acceptation — « avec un message citant les
        // **deux** points fautifs ». C'est le critère le plus important de
        // l'issue : la courbe ne se relit pas, le message d'erreur est la seule
        // occasion de voir la faute. Un « courbe décroissante » sec obligerait à
        // recompter les points à la main.
        //
        // Les quatre nombres sont distincts pour qu'aucun ne puisse être
        // confondu avec un autre dans le message.
        let erreur = refus(&[point(3, 25), point(12, 80), point(30, 45)]);
        let message = erreur.to_string();

        for attendu in ["12", "80", "30", "45"] {
            assert!(
                message.contains(attendu),
                "le message doit citer « {attendu} » : « {message} »"
            );
        }
    }

    #[test]
    fn une_baisse_au_milieu_est_refusee() {
        // Même critère : la faute n'est pas forcément entre le premier et le
        // dernier point. Le couple désigné est celui où la consigne baisse, pas
        // les extrémités de la courbe.
        let erreur = refus(&[point(1, 20), point(20, 80), point(40, 60)]);

        match erreur {
            CurveError::Decreasing { from, to } => {
                assert_eq!((from.0, from.1.percent()), (20, 80));
                assert_eq!((to.0, to.1.percent()), (40, 60));
            }
            autre => panic!("la baisse est entre les points 20 et 40, erreur reçue : {autre:?}"),
        }
    }

    #[test]
    fn une_baisse_reste_detectee_quand_les_points_sont_dans_le_desordre() {
        // Croisement des deux critères : « points dans le désordre » et
        // « courbe décroissante refusée ». La faute porte sur la courbe, pas
        // sur l'ordre de saisie — sinon il suffirait de saisir ses points à
        // l'envers pour contourner le garde-fou.
        //
        // `from` est le point de température la plus basse, quel que soit
        // l'ordre où l'appelant les a donnés : le contrat décrit la faute comme
        // « la consigne baisse quand la température **monte** ».
        let erreur = refus(&[point(40, 40), point(1, 80)]);

        match erreur {
            CurveError::Decreasing { from, to } => {
                assert_eq!((from.0, from.1.percent()), (1, 80));
                assert_eq!((to.0, to.1.percent()), (40, 40));
            }
            autre => panic!("l'ordre de saisie ne doit rien changer, erreur reçue : {autre:?}"),
        }
    }

    #[test]
    fn une_liste_vide_est_refusee() {
        // Contrat d'API — `CurveError::Empty`, « Aucun point fourni ». Une
        // courbe sans point n'a pas de valeur par défaut raisonnable : écrire
        // 0 % partout arrêterait la pompe, écrire 100 % ferait hurler le
        // boîtier. Le seul comportement honnête est le refus.
        let vide: [(usize, Percent); 0] = [];
        let erreur = refus(&vide);

        assert!(
            matches!(erreur, CurveError::Empty),
            "une liste vide doit donner `Empty`, pas {erreur:?}"
        );
    }

    #[test]
    fn l_index_zero_est_refuse() {
        // Contrat d'API — `OutOfRange { given }`, « Un index de point hors de
        // `1..=CURVE_POINTS` ». Les points sont 1-indexés comme les fichiers
        // sysfs `..._auto_point1_pwm` : accepter 0 en silence décalerait toute
        // la courbe d'un cran, exactement le « décalage d'indice » que le
        // critère de décroissance cherche à rendre visible.
        let erreur = refus(&[point(0, 30), point(20, 60)]);

        match erreur {
            CurveError::OutOfRange { given } => assert_eq!(given, 0),
            autre => panic!("l'index 0 n'existe pas, erreur reçue : {autre:?}"),
        }
    }

    #[test]
    fn un_index_au_dela_du_dernier_point_est_refuse() {
        // Même variante, à l'autre bord. `CURVE_POINTS + 1` est le premier
        // index invalide ; les valeurs plus grandes ne doivent pas déborder
        // davantage — un index arbitraire écrirait dans un fichier que la
        // découverte n'a pas listé, ou nulle part.
        for index in [CURVE_POINTS + 1, 100, usize::MAX] {
            let erreur = refus(&[point(1, 30), point(index, 60)]);
            match erreur {
                CurveError::OutOfRange { given } => assert_eq!(given, index),
                autre => panic!("l'index {index} est hors plage, erreur reçue : {autre:?}"),
            }
        }
    }

    #[test]
    fn le_refus_d_index_cite_l_index_et_la_plage() {
        // issue #9, critère d'acceptation — « Une température hors de la plage
        // du matériel produit une erreur qui **donne la plage** ». La
        // conversion température ↔ index est la troisième inconnue et n'est pas
        // figée ici ; ce qui l'est, c'est la plage que le contrat d'API
        // manipule : `1..=CURVE_POINTS`. Le message doit donner l'index fautif
        // et ses bornes, faute de quoi il faut aller compter les fichiers du
        // pilote pour comprendre.
        let message = refus(&[point(41, 60)]).to_string();

        for attendu in ["41", "1", &CURVE_POINTS.to_string()] {
            assert!(
                message.contains(attendu),
                "le message doit citer « {attendu} » : « {message} »"
            );
        }
    }

    #[test]
    fn tous_les_index_du_premier_au_dernier_point_sont_acceptes() {
        // La borne haute est inclusive : le point 40 existe, c'est
        // `temp1_auto_point40_pwm`. Une plage exclusive rendrait le dernier
        // point inatteignable, donc figé à la valeur de l'avant-dernier.
        for index in 1..=CURVE_POINTS {
            let c = courbe(&[point(index, 60)]);
            assert_eq!(
                au_point(&c, index),
                60,
                "l'index {index} est dans la plage du matériel"
            );
        }
    }

    #[test]
    fn deux_points_au_meme_index_avec_des_consignes_differentes_sont_refuses() {
        // Contrat d'API — `Conflicting { at }`, « Deux points portent le même
        // index avec des consignes différentes ». Choisir l'un des deux
        // silencieusement écrirait une courbe que l'utilisateur n'a pas
        // demandée, et l'écriture seule l'empêcherait de s'en apercevoir.
        let erreur = refus(&[point(1, 20), point(25, 50), point(25, 70)]);

        match erreur {
            CurveError::Conflicting { at } => assert_eq!(at, 25),
            autre => panic!("deux consignes au point 25, erreur reçue : {autre:?}"),
        }

        // Dans l'autre sens, le refus reste un refus. La variante exacte n'est
        // pas exigée ici : à index égal la température ne « monte » pas, donc
        // `Decreasing` ne s'applique pas au sens du contrat — mais l'issue ne
        // tranche pas la priorité entre les deux. À confirmer.
        let inverse = refus(&[point(1, 20), point(25, 70), point(25, 50)]);
        assert!(
            matches!(
                inverse,
                CurveError::Conflicting { .. } | CurveError::Decreasing { .. }
            ),
            "deux consignes au même index restent un refus, erreur reçue : {inverse:?}"
        );
    }

    #[test]
    fn deux_points_identiques_a_l_identique_sont_acceptes() {
        // **Interprétation** : le contrat définit `Conflicting` par « deux
        // points portent le même index avec des consignes **différentes** ». Un
        // doublon exact ne porte aucune contradiction — il n'y a rien à
        // arbitrer, la courbe est la même avec ou sans lui. Le refuser gênerait
        // l'interface graphique, qui relaiera les points d'une courbe dessinée
        // sans les dédoublonner. À confirmer.
        let c = courbe(&[point(1, 20), point(25, 70), point(25, 70), point(40, 90)]);

        assert_eq!(au_point(&c, 25), 70);
        assert_eq!(au_point(&c, 1), 20);
        assert_eq!(au_point(&c, 40), 90);
    }

    #[test]
    fn une_consigne_sous_le_plancher_reste_interpolable() {
        // issue #9, critère d'acceptation — « Le **plancher de 20 %** de #7
        // s'applique à chaque point, **contournable par `--force`** ».
        // issue #7, contrat d'API — « Le plancher vit dans la commande, pas
        // dans le type. `Percent::new(5)` réussit. Si le type refusait,
        // `--force` n'aurait aucun moyen de s'exprimer. »
        //
        // Même raisonnement ici, et `CurveError` le confirme : ses quatre
        // variantes ne parlent pas du plancher. `interpolate` ne peut donc pas
        // le faire respecter — c'est `reverb curve` qui l'applique.
        let sous_plancher = courbe(&[point(1, Percent::FLOOR - 15), point(40, 100)]);

        assert_eq!(au_point(&sous_plancher, 1), Percent::FLOOR - 15);
        assert!(
            consignes(&sous_plancher)
                .iter()
                .any(|&p| p < Percent::FLOOR),
            "une courbe qui démarre sous le plancher garde ses points bas"
        );
    }
}

// ---------------------------------------------------------------------------

mod ecriture {
    use super::{
        CURVE_POINTS, arborescence_de_reference, canal, courbe, ecarts, lire, photographie, point,
        relatif,
    };
    use reverb_cli::hwmon::set_curve;

    /// Les 40 chemins de courbe d'un canal, relatifs à la racine, triés.
    fn chemins_attendus(
        racine: &std::path::Path,
        canal: &reverb_cli::hwmon::FanChannel,
    ) -> Vec<String> {
        let mut chemins: Vec<String> = canal.curve.iter().map(|c| relatif(racine, c)).collect();
        chemins.sort();
        chemins
    }

    #[test]
    fn set_curve_ecrit_la_valeur_brute_du_noyau_et_non_le_pourcentage() {
        // Contrat d'API — `set_curve`, « Écrit les 40 points de la courbe du
        // canal », et `Percent::raw`, « Échelle du noyau, 0 à 255 ».
        // Le pilote lit ces fichiers sur la même échelle que `pwmN` : y écrire
        // « 60 » là où il attend 153 donnerait une pompe à 24 % pour une courbe
        // demandée à 60 %, sans le moindre message — et sans relecture possible
        // pour s'en rendre compte.
        let sysfs = arborescence_de_reference("set_curve_valeur");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "kraken2023elite:pump-speed");
        let demande = courbe(&[point(1, 20), point(CURVE_POINTS, 100)]);

        set_curve(c, &demande).expect("écriture de la courbe");

        for (n, chemin) in c.curve.iter().enumerate() {
            let attendu = demande.points()[n].raw().to_string();
            assert_eq!(
                lire(chemin),
                attendu,
                "point {} : {} doit porter la valeur brute",
                n + 1,
                chemin.display()
            );
        }

        // Dit autrement : ce n'est pas le pourcentage qui part sur le fil.
        assert_ne!(
            lire(&c.curve[0]),
            "20",
            "c'est la valeur brute du noyau qui est écrite, pas les 20 %"
        );
    }

    #[test]
    fn le_premier_point_va_dans_point1_et_le_dernier_dans_point40() {
        // issue #9 — le pilote expose `temp1_auto_point[1-40]_pwm`. Un ordre
        // inversé écrirait une courbe rigoureusement à l'envers : consigne
        // maximale à froid, minimale à chaud. L'écriture seule ne laisserait
        // aucune trace de l'erreur.
        //
        // La courbe est strictement montante pour qu'une inversion se voie sur
        // les valeurs, et les noms de fichiers sont vérifiés en plus.
        let sysfs = arborescence_de_reference("set_curve_ordre");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "kraken2023elite:pump-speed");
        let demande = courbe(&[point(1, 20), point(CURVE_POINTS, 100)]);

        set_curve(c, &demande).expect("écriture de la courbe");

        let premier = &c.curve[0];
        let dernier = &c.curve[CURVE_POINTS - 1];

        assert!(
            premier
                .file_name()
                .and_then(|f| f.to_str())
                .is_some_and(|f| f.ends_with("_auto_point1_pwm")),
            "le premier chemin doit être celui du point 1 : {}",
            premier.display()
        );
        assert!(
            dernier
                .file_name()
                .and_then(|f| f.to_str())
                .is_some_and(|f| f.ends_with("_auto_point40_pwm")),
            "le dernier chemin doit être celui du point 40 : {}",
            dernier.display()
        );

        assert_eq!(lire(premier), demande.points()[0].raw().to_string());
        assert_eq!(
            lire(dernier),
            demande.points()[CURVE_POINTS - 1].raw().to_string()
        );
        let bas: u32 = lire(premier).parse().expect("valeur brute");
        let haut: u32 = lire(dernier).parse().expect("valeur brute");
        assert!(
            bas < haut,
            "une courbe montante doit rester montante sur le disque : {bas} puis {haut}"
        );
    }

    #[test]
    fn set_curve_ecrit_les_quarante_points_et_pas_seulement_ceux_donnes() {
        // issue #9, critère d'acceptation — « Les 40 valeurs sont écrites **d'un
        // bloc** ; aucune écriture partielle n'est possible depuis l'extérieur
        // du module ». Deux couples en entrée, quarante fichiers en sortie :
        // laisser les 38 autres tels quels mélangerait la courbe demandée avec
        // celle qui s'y trouvait, sans moyen de la relire pour le constater.
        //
        // La fausse arborescence part de « 0 » partout, et la courbe demandée
        // ne contient aucun zéro : tout fichier resté à « 0 » n'a pas été écrit.
        let sysfs = arborescence_de_reference("set_curve_bloc");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "kraken2023elite:pump-speed");
        let demande = courbe(&[point(1, 20), point(CURVE_POINTS, 100)]);

        assert_eq!(c.curve.len(), CURVE_POINTS);
        set_curve(c, &demande).expect("écriture de la courbe");

        for (n, chemin) in c.curve.iter().enumerate() {
            assert_ne!(
                lire(chemin),
                "0",
                "le point {} n'a pas été écrit : {}",
                n + 1,
                chemin.display()
            );
        }
    }

    #[test]
    fn set_curve_ne_touche_que_les_quarante_fichiers_de_la_courbe() {
        // issue #9, approche technique — « l'écriture prend un `&FanChannel` et
        // une courbe déjà validée, comme `set_pwm` : le seul chemin ouvrable
        // reste celui que la découverte a listé ».
        //
        // En particulier, `pwmN_enable` n'est **pas** touché : la valeur qui
        // fait passer le Kraken en mode courbe est la première des trois
        // inconnues de l'issue, et `set_curve` n'a de toute façon pas à en
        // décider. Téléverser une courbe et basculer un mode sont deux gestes.
        let sysfs = arborescence_de_reference("set_curve_confine");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "kraken2023elite:pump-speed");
        let demande = courbe(&[point(1, 20), point(CURVE_POINTS, 100)]);

        let avant = photographie(sysfs.racine());
        set_curve(c, &demande).expect("écriture de la courbe");
        let apres = photographie(sysfs.racine());

        assert_eq!(ecarts(&avant, &apres), chemins_attendus(sysfs.racine(), c));

        // Dit autrement, sur les voisins immédiats du canal visé.
        let enable = c.enable.as_ref().expect("le Kraken expose `pwm1_enable`");
        assert_eq!(lire(enable), "0", "le mode n'est pas basculé au passage");
        assert_eq!(lire(&c.pwm), "171", "la consigne fixe n'est pas touchée");
    }

    #[test]
    fn ecrire_la_courbe_d_un_canal_ne_touche_pas_celle_de_l_autre() {
        // La pompe et le ventilateur du Kraken ont chacun leur courbe
        // (issue #9 — `temp1_auto_point[1-40]_pwm` **et**
        // `temp2_auto_point[1-40]_pwm`). Écrire l'une en écrasant l'autre
        // rendrait les deux canaux indissociables, et l'écriture seule
        // interdirait de s'en apercevoir.
        //
        // Le test ne dit pas lequel des deux indices de température pilote la
        // pompe — c'est la deuxième inconnue de l'issue. Il dit seulement que
        // les deux jeux de fichiers sont disjoints, ce qui vaut dans les deux
        // cas.
        let sysfs = arborescence_de_reference("set_curve_deux_canaux");
        let canaux = sysfs.canaux();
        let pompe = canal(&canaux, "kraken2023elite:pump-speed");
        let ventilateur = canal(&canaux, "kraken2023elite:fan-speed");
        let demande = courbe(&[point(1, 20), point(CURVE_POINTS, 100)]);

        set_curve(pompe, &demande).expect("écriture de la courbe de la pompe");

        for (n, chemin) in ventilateur.curve.iter().enumerate() {
            assert_eq!(
                lire(chemin),
                "0",
                "le point {} de l'autre canal doit être intact : {}",
                n + 1,
                chemin.display()
            );
        }
    }

    #[test]
    fn set_curve_echoue_sur_un_canal_sans_courbe_et_n_ecrit_nulle_part() {
        // issue #9, critère d'acceptation — « Un canal qui n'a pas de courbe —
        // les trois `nzxtsmart2`, les `nct6687` — produit une erreur **qui le
        // dit**, pas un échec d'écriture brut ».
        // Contrat d'API — `set_curve` « Échoue si le canal n'a pas de courbe ».
        //
        // Le mot cherché dans le message est une **interprétation** : l'issue
        // exige que l'erreur nomme le problème, sans en fixer la formulation.
        let sysfs = arborescence_de_reference("set_curve_sans_courbe");
        let canaux = sysfs.canaux();
        let demande = courbe(&[point(1, 20), point(CURVE_POINTS, 100)]);

        let avant = photographie(sysfs.racine());
        for nom in [
            "nzxtsmart2:fan-1",
            "nzxtsmart2:fan-2",
            "nzxtsmart2:fan-3",
            "nct6687:cpu-fan",
            "amdgpu:fan1",
        ] {
            let c = canal(&canaux, nom);
            assert!(c.curve.is_empty(), "{nom} n'a pas de courbe");

            let erreur = set_curve(c, &demande)
                .expect_err("un canal sans courbe ne peut pas en recevoir une");
            let message = erreur.to_string().to_lowercase();
            assert!(
                message.contains("courbe"),
                "{nom} : le message doit dire que le canal n'a pas de courbe : « {message} »"
            );
        }
        let apres = photographie(sysfs.racine());

        assert!(
            ecarts(&avant, &apres).is_empty(),
            "un échec n'écrit nulle part : {:?}",
            ecarts(&avant, &apres)
        );
    }
}

// ---------------------------------------------------------------------------

mod decouverte {
    use super::{CURVE_POINTS, arborescence_de_reference, canal};

    #[test]
    fn un_canal_dont_la_source_expose_une_courbe_porte_quarante_chemins() {
        // issue #9, approche technique — « la découverte associe au canal la
        // liste de ses fichiers de courbe, s'ils existent. Un canal sans courbe
        // se distingue alors **par son type**, pas par une vérification à
        // l'écriture. »
        let sysfs = arborescence_de_reference("courbe_decouverte");
        let canaux = sysfs.canaux();

        for nom in ["kraken2023elite:pump-speed", "kraken2023elite:fan-speed"] {
            let c = canal(&canaux, nom);
            assert_eq!(
                c.curve.len(),
                CURVE_POINTS,
                "{nom} doit porter ses {CURVE_POINTS} points"
            );
            for chemin in &c.curve {
                assert!(chemin.exists(), "{nom} : {} doit exister", chemin.display());
            }
        }
    }

    #[test]
    fn les_chemins_de_courbe_vont_du_point_1_au_point_40_dans_l_ordre() {
        // Contrat d'API — `curve: Vec<PathBuf>`, « Les 40 fichiers
        // `tempN_auto_pointM_pwm` ». L'ordre de la liste est celui des points,
        // parce que c'est lui qui sera apparié aux 40 consignes de la courbe.
        //
        // Le piège est concret : l'ordre de lecture d'un répertoire est
        // arbitraire, et un tri alphabétique placerait le point 10 avant le
        // point 2.
        let sysfs = arborescence_de_reference("courbe_ordre");
        let canaux = sysfs.canaux();
        let c = canal(&canaux, "kraken2023elite:pump-speed");

        for (n, chemin) in c.curve.iter().enumerate() {
            let attendu = format!("_auto_point{}_pwm", n + 1);
            let fichier = chemin
                .file_name()
                .and_then(|f| f.to_str())
                .expect("nom de fichier lisible");
            assert!(
                fichier.ends_with(&attendu),
                "position {} de la liste : « {fichier} » n'est pas le point {}",
                n,
                n + 1
            );
        }
    }

    #[test]
    fn les_quarante_chemins_viennent_du_meme_indice_de_temperature() {
        // Une courbe qui mélangerait `temp1_auto_point3_pwm` et
        // `temp2_auto_point4_pwm` n'aurait aucun sens : ce sont deux courbes
        // distinctes du firmware.
        //
        // Le test ne dit **pas** quel indice va à quel canal — c'est la
        // deuxième inconnue de l'issue, « `temp1_*` pilote-t-il la pompe et
        // `temp2_*` le ventilateur ? ». Il vérifie seulement la cohérence
        // interne de la liste.
        let sysfs = arborescence_de_reference("courbe_meme_temp");
        let canaux = sysfs.canaux();

        for nom in ["kraken2023elite:pump-speed", "kraken2023elite:fan-speed"] {
            let c = canal(&canaux, nom);
            let prefixes: Vec<String> = c
                .curve
                .iter()
                .map(|chemin| {
                    let fichier = chemin
                        .file_name()
                        .and_then(|f| f.to_str())
                        .expect("nom de fichier lisible");
                    fichier
                        .split_once("_auto_point")
                        .map(|(prefixe, _)| prefixe.to_owned())
                        .unwrap_or_else(|| {
                            panic!("{nom} : « {fichier} » n'est pas un point de courbe")
                        })
                })
                .collect();

            assert!(
                prefixes.windows(2).all(|paire| paire[0] == paire[1]),
                "{nom} : les 40 points doivent venir du même indice de température, vu {prefixes:?}"
            );
            assert!(
                prefixes[0].starts_with("temp"),
                "{nom} : le préfixe attendu est « tempN », vu « {} »",
                prefixes[0]
            );
        }
    }

    #[test]
    fn les_deux_canaux_du_kraken_ne_partagent_aucun_fichier_de_courbe() {
        // Conséquence de la précédente, et garde-fou de l'écriture : deux
        // canaux qui partageraient un fichier seraient impossibles à régler
        // séparément, et l'écriture seule rendrait la collision invisible.
        //
        // Vrai quel que soit le sens de l'appariement — c'est justement ce qui
        // rend ce test écrivable avant la mesure.
        let sysfs = arborescence_de_reference("courbes_disjointes");
        let canaux = sysfs.canaux();
        let pompe = canal(&canaux, "kraken2023elite:pump-speed");
        let ventilateur = canal(&canaux, "kraken2023elite:fan-speed");

        for chemin in &pompe.curve {
            assert!(
                !ventilateur.curve.contains(chemin),
                "{} appartient aux deux canaux",
                chemin.display()
            );
        }
    }

    #[test]
    fn un_canal_sans_courbe_porte_une_liste_vide() {
        // issue #9, hors scope — « Les courbes des autres sources : seul
        // `kraken2023elite` en expose ». Et critère d'acceptation — « Un canal
        // qui n'a pas de courbe — les trois `nzxtsmart2`, les `nct6687` ».
        // Une liste vide, pas une liste de chemins vers des fichiers absents :
        // c'est ce qui permet à `set_curve` de refuser avant d'écrire.
        let sysfs = arborescence_de_reference("sans_courbe");
        let canaux = sysfs.canaux();

        for nom in [
            "nzxtsmart2:fan-1",
            "nzxtsmart2:fan-2",
            "nzxtsmart2:fan-3",
            "nct6687:cpu-fan",
            "nct6687:sys-fan-1",
        ] {
            assert!(
                canal(&canaux, nom).curve.is_empty(),
                "{nom} : sa source n'expose aucune courbe"
            );
        }
    }

    #[test]
    fn une_entree_de_temperature_n_est_pas_une_courbe() {
        // `amdgpu` expose `temp1_input` et `pwm1`, mais aucun
        // `temp1_auto_pointM_pwm`. Confondre une entrée de température avec une
        // courbe ferait croire à `set_curve` qu'elle a 40 fichiers à écrire,
        // sur une source qui n'en a aucun.
        // Même piège qu'en #7 avec `pwm1_mode`, qui commence par `pwm` sans
        // être une sortie.
        let sysfs = arborescence_de_reference("temp_input_seule");
        let canaux = sysfs.canaux();

        assert!(canal(&canaux, "amdgpu:fan1").curve.is_empty());
    }

    #[test]
    fn l_index_du_canal_est_le_n_de_pwm_n() {
        // Contrat d'API — `index: u32`, « Numéro du canal dans sa source : le N
        // de `pwmN` ».
        // issue #9, approche technique — « `FanChannel` gagne son **index de
        // canal**, aujourd'hui perdu après la découverte. C'est lui qui relie
        // `pwm1` à `temp1_auto_point*`. »
        // issue #7, « État relevé sur la machine » — `pwm1` est « Pump speed »
        // et `pwm2` « Fan speed » sur `kraken2023elite`.
        let sysfs = arborescence_de_reference("index_de_canal");
        let canaux = sysfs.canaux();

        for (nom, index) in [
            ("nzxtsmart2:fan-1", 1u32),
            ("nzxtsmart2:fan-2", 2),
            ("nzxtsmart2:fan-3", 3),
            ("kraken2023elite:pump-speed", 1),
            ("kraken2023elite:fan-speed", 2),
            ("nct6687:cpu-fan", 1),
            ("nct6687:sys-fan-1", 2),
            ("amdgpu:fan1", 1),
        ] {
            assert_eq!(canal(&canaux, nom).index, index, "index de {nom}");
        }
    }

    #[test]
    fn l_index_correspond_au_fichier_de_consigne_decouvert() {
        // Le même fait, dit sans recopier la table : l'index doit se relire sur
        // le nom du fichier `pwmN` que la découverte a listé. Un index inventé
        // à partir de la position dans la liste dériverait dès qu'une source
        // saute un numéro.
        let sysfs = arborescence_de_reference("index_et_pwm");
        let canaux = sysfs.canaux();

        for c in &canaux {
            let fichier = c
                .pwm
                .file_name()
                .and_then(|f| f.to_str())
                .expect("nom de fichier lisible");
            assert_eq!(
                fichier,
                format!("pwm{}", c.index),
                "{} : l'index doit être celui de son fichier de consigne",
                c.name
            );
        }
    }

    #[test]
    fn aucun_chemin_de_courbe_ne_sort_de_la_racine_donnee() {
        // issue #9, critère d'acceptation — « Aucun accès matériel dans les
        // tests automatisés ». Si la découverte rendait un chemin absolu vers
        // `/sys`, un test finirait par écrire une courbe sur la vraie pompe.
        let sysfs = arborescence_de_reference("courbes_confinees");
        let racine = sysfs.racine();

        for c in sysfs.canaux() {
            for chemin in &c.curve {
                assert!(
                    chemin.starts_with(racine),
                    "{} sort de la racine {}",
                    chemin.display(),
                    racine.display()
                );
            }
        }
    }
}
