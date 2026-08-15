//! Tests d'intention — issue #99, « Régulation automatique côté hôte pour les canaux sans mode
//! auto ».
//!
//! Écrits **avant** l'implémentation, depuis l'issue #99 seule. Aucun fichier de
//! `crates/*/src/` n'a été lu pour les produire, hors la liste des `pub mod` de
//! `reverb-daemon/src/lib.rs` et les signatures publiques de `reverb-hw/src/hwmon.rs`. À
//! l'écriture de ce fichier, le module `regulation` **n'existe pas** : la compilation de ce test
//! doit échouer, et c'est la phase rouge.
//!
//! Rien ici n'ouvre un `hwmon`, n'écrit un `pwm*`, ne dort ni ne lit l'horloge. La température est
//! **injectée** et l'écriture est **enregistrée en mémoire** : ce fichier vérifie une décision, pas
//! un bus.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Mesuré sur SHYNAEL le 2026-08-15, 863 relevés sur 72 minutes de jeu : le duty des trois canaux
//! `nzxtsmart2` a pris **exactement une valeur** — `64`, soit 25 %, ~700 tr/min — pendant que le
//! liquide passait quarante-cinq minutes au-dessus de 50 °C. Le pilote `nzxt-smart2` n'a aucun mode
//! automatique : sa vitesse est celle que l'hôte écrit, et personne ne l'écrit.
//!
//! ⚠️ **Le défaut n'est pas qu'une consigne soit fausse, c'est qu'aucune ne soit jamais écrite.**
//! D'où la forme de ces tests : ils comptent les écritures autant qu'ils en vérifient la valeur.
//!
//! ## La signature que ces tests exigent, et pourquoi celle-là
//!
//! L'issue pose que « le calcul — température → consigne — est **pur** et vit dans un crate
//! testable sans matériel », et que « l'application vit dans le démon, greffée sur le tour de
//! télémétrie existant ». Elle ne dit pas quelle forme lui donner. Ce fichier tranche, et voici ce
//! qu'il faut implémenter :
//!
//! ```ignore
//! // crates/reverb-daemon/src/regulation.rs
//! // et `pub mod regulation;` dans crates/reverb-daemon/src/lib.rs
//!
//! use std::io;
//! use std::path::Path;
//!
//! /// La sonde dont la régulation dépend, et la seule (issue #99, hors scope).
//! pub const SONDE_DU_LIQUIDE: &str = "kraken2023elite:coolant-temp";
//!
//! /// La consigne appliquée quand le liquide est illisible.
//! pub const REPLI: u8 = 50;
//!
//! /// Où vit l'état : de l'état de service, pas une donnée de montage.
//! pub const CHEMIN_REGULATION: &str = "/var/lib/reverb/regulation.conf";
//!
//! /// La courbe température → consigne. Pure, sans E/S, sans horloge.
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub struct Courbe { /* des paliers strictement croissants en température */ }
//!
//! impl Courbe {
//!     /// Le tableau de l'issue : 30 % à 35 °C, 60 % à 45 °C, 100 % à 50 °C.
//!     pub fn defaut() -> Courbe;
//!
//!     /// `paliers` : `(millidegrés, pourcent)`, au moins un, températures strictement
//!     /// croissantes, consignes dans `0..=100`.
//!     pub fn depuis(paliers: &[(i32, u8)]) -> Result<Courbe, CourbeInvalide>;
//!
//!     pub fn paliers(&self) -> &[(i32, u8)];
//!
//!     /// La consigne pour une température en **millidegrés**, comme `hwmon` la rend.
//!     pub fn consigne(&self, milli_degres: i32) -> u8;
//! }
//!
//! #[derive(Debug)]
//! pub struct CourbeInvalide {
//!     pub raison: String,
//! }
//!
//! /// Une écriture à faire sur un canal, et rien de plus : la régulation ne touche aucun bus.
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub struct Ecriture {
//!     pub canal: String,
//!     pub consigne: u8,
//!     /// Ajouté par #110 : pourquoi cette écriture part. Aucun test de ce fichier ne l'observe.
//!     pub motif: Motif,
//! }
//!
//! #[derive(Debug)]
//! pub struct Regulation { /* la courbe, les canaux régulés, la dernière consigne par canal */ }
//!
//! impl Regulation {
//!     pub fn nouvelle(courbe: Courbe) -> Regulation;
//!     pub fn courbe(&self) -> &Courbe;
//!     pub fn activer(&mut self, canal: &str);
//!     pub fn couper(&mut self, canal: &str);
//!     /// Les canaux régulés, triés.
//!     pub fn canaux(&self) -> Vec<String>;
//!
//!     /// Un tour de télémétrie : `liquide` en millidegrés, `None` si la sonde est illisible.
//!     /// Rend ce qu'il faut écrire, et **seulement** ce qu'il faut écrire.
//!     ///
//!     /// ⚠️ **`portees` a été ajouté par #110** — ce que chaque canal **porte réellement**, en
//!     /// pourcentage, relu par le démon avant le tour. La décision se prend dessus, et non plus
//!     /// sur ce qu'on a cru écrire. Aucune assertion de ce fichier n'en dépend : le banc
//!     /// ci-dessous modélise un matériel obéissant, l'hypothèse implicite de tous ses scénarios.
//!     pub fn tour(
//!         &mut self,
//!         liquide: Option<i32>,
//!         portees: &BTreeMap<String, Option<u8>>,
//!     ) -> Vec<Ecriture>;
//!
//!     pub fn encoder(&self) -> String;
//!     pub fn decoder(texte: &str) -> Result<Regulation, RegulationInvalide>;
//! }
//!
//! #[derive(Debug)]
//! pub struct RegulationInvalide {
//!     /// Numéro de ligne à partir de 1 ; 0 pour une entrée absente, qui n'est écrite nulle part.
//!     pub ligne: usize,
//!     pub raison: String,
//! }
//!
//! pub fn charger_regulation(chemin: &Path) -> (Regulation, Option<String>);
//! pub fn enregistrer_regulation(chemin: &Path, regulation: &Regulation) -> io::Result<()>;
//! ```
//!
//! Cinq choix, et ce qu'ils achètent :
//!
//! 1. **Tout est en millidegrés, jamais en degrés flottants.** `Sonde::lire` rend des millidegrés
//!    entiers, comme `hwmon` ; une conversion vers `f32` en chemin rendrait la courbe dépendante
//!    d'un arrondi, et le projet a déjà payé ce prix une fois — « la symétrie est calculée sur les
//!    **indices**, jamais sur une position flottante » (README, directions locales). Deux unités
//!    dans la même API seraient pire encore : c'est exactement la faute des trois ordres de
//!    composantes, qui ne produit aucun message et juste un résultat faux.
//! 2. **`tour` rend les écritures au lieu de les faire.** C'est ce qui rend « on n'écrit que ce qui
//!    change » vérifiable **sans matériel** : une liste vide est un tour qui ne consomme pas le bus,
//!    et un enregistreur en mémoire suffit à tout constater. Le démon, lui, applique la liste.
//! 3. **`Option<i32>` pour le liquide, sans cause d'échec.** La régulation ne fait rien de la
//!    différence entre une sonde en quarantaine (#68), une lecture qui échoue et une valeur
//!    illisible : les trois mènent au repli. C'est déjà la forme que `Quarantaine::tour` rend.
//! 4. **La consigne est un `u8` et non un `reverb_hw::hwmon::Percent`.** Le calcul reste pur et sans
//!    dépendance ; la conversion vers le noyau vit déjà dans `Percent`, où elle « n'existe qu'une
//!    fois ». Au passage, `Percent::FLOOR` — le plancher de 20 % — est une règle de la **commande**
//!    `reverb fan`, pas du type : elle ne s'applique pas ici.
//! 5. **Un état, un fichier, sous `/var/lib`.** L'issue le pose : « c'est de l'état de service
//!    réécrit à chaque commande, pas une donnée de montage ». La géométrie reste dans `/etc`.
//!
//! ## Ce que ce fichier fige
//!
//! 1. **La courbe est bornée par construction.** Sous le premier palier et au-dessus du dernier,
//!    elle rend la **borne** — jamais une extrapolation, jamais un débordement arithmétique. Une
//!    sonde qui rend `i32::MIN` ne doit ni paniquer ni produire 0 %.
//! 2. **Elle ne redescend jamais quand le liquide monte.** Un ventilateur qui ralentit pendant que
//!    le circuit chauffe est l'inverse exact du défaut qu'on corrige, et une erreur de signe dans
//!    l'interpolation ne se voit pas autrement.
//! 3. **On n'écrit que ce qui change**, et le cache est **par canal** : un canal qu'on vient
//!    d'activer reçoit la consigne courante même si la température n'a pas bougé, parce qu'il n'a
//!    jamais rien reçu.
//! 4. **Un canal non régulé n'apparaît dans aucune écriture** — vérifié sur un scénario entier, et
//!    pas seulement sur un tour.
//! 5. **Le repli est écrit une fois**, pas à chaque tour, et **il n'est jamais la dernière valeur
//!    connue**. C'est le mode de défaillance rassurant que le projet refuse partout : « un 34 °C
//!    figé derrière une pompe arrêtée, c'est un circuit qui chauffe sans que rien ne le signale ».
//! 6. **Le retour du liquide reprend la courbe**, sans redémarrage et sans état à réarmer.
//! 7. **Un fichier absent laisse démarrer sans régulation**, un fichier abîmé le dit **en nommant**
//!    ce qui cloche, et aucun des deux n'empêche le démarrage.
//!
//! ## Ce que le contrat laisse ouvert, et que ces tests tranchent
//!
//! 1. **Les paliers sont strictement croissants en température, et une courbe qui ne l'est pas est
//!    refusée** plutôt que triée en silence. Deux paliers à la même température se contredisent —
//!    laquelle des deux consignes à 45 °C ? — et réordonner ce qu'un humain a écrit, c'est
//!    « compléter au jugé », ce que le projet refuse pour `eclairage.conf`.
//! 2. **Un canal coupé oublie ce qu'il avait reçu** : le reprendre le réécrit. Entre-temps
//!    n'importe quel `reverb fan` a pu poser autre chose, et le cache mentirait.
//! 3. **Un fichier abîmé démarre sans régulation**, et non sur le repli. On ne sait pas quels canaux
//!    réguler ; poser 50 % sur des canaux qu'on n'a pas su relire serait décider à la place de
//!    l'utilisateur, sur un bus qu'on ne sait pas cartographier.
//! 4. **L'arrondi de l'interpolation n'est pas figé.** Les valeurs pincées ici tombent toutes juste
//!    en arithmétique entière ; partout ailleurs les tests encadrent au lieu de pointer, et exigent
//!    la monotonie. Choisir entre troncature et arrondi au plus proche serait figer un détail que
//!    l'issue ne tranche pas, pour un écart d'un pourcent invisible sur un ventilateur.
//! 5. **Un refus nomme le fautif en clair**, et les tests demandent que le message **contienne** le
//!    jeton en cause, sans imposer sa mise en forme — « 45 », « 45000 » et « 45 °C » passent tous.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **« Aucun nouveau réveil »** au sens strict : c'est une propriété de la boucle du démon — pas
//!   de `timer`, pas de sondage — et rien d'observable depuis un module pur. Ce qui en est testable
//!   ici l'est : sans canal régulé, `tour` ne produit **jamais** la moindre écriture, quel que soit
//!   ce qu'on lui donne.
//! - **La lecture sysfs et l'écriture du `pwm*`** : de l'autre côté de l'E/S. `set_pwm` ne reçoit
//!   qu'un `&FanChannel` et son confinement est déjà vérifié sur le matériel
//!   (`docs/VENTILATEURS.md`).
//! - **Le nommage de `fan <canal> auto` et le sort de `sait_faire_auto`** : l'issue les met
//!   explicitement en « point de nommage à trancher à l'implémentation ». Écrire un test dessus
//!   maintenant, ce serait trancher à sa place.
//! - **Une courbe par canal**, l'édition depuis la fenêtre, les canaux du Kraken : hors scope de
//!   l'issue.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use reverb_daemon::regulation::{
    CHEMIN_REGULATION, Courbe, CourbeInvalide, Ecriture, REPLI, Regulation, RegulationInvalide,
    SONDE_DU_LIQUIDE, charger_regulation, enregistrer_regulation,
};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// Les millidegrés d'un nombre entier de degrés.
///
/// `degres(45) + 500` se lit donc 45,5 °C, et tout reste entier.
const fn degres(entiers: i32) -> i32 {
    entiers * 1_000
}

/// Les trois canaux que l'issue vise : ceux du `nzxtsmart2`, dont le pilote n'a aucun mode
/// automatique (`docs/VENTILATEURS.md`).
const CANAL_1: &str = "nzxtsmart2:fan-1";
const CANAL_2: &str = "nzxtsmart2:fan-2";
const CANAL_3: &str = "nzxtsmart2:fan-3";

/// Un canal du Kraken, **hors scope** : « leur firmware régule déjà correctement, mesuré ».
///
/// Il ne sert qu'à une chose ici — être le témoin de « un canal non régulé n'est jamais écrit ».
const CANAL_KRAKEN: &str = "kraken2023elite:fan-speed";

/// Les trois températures relevées le 2026-08-15, en millidegrés : minimum, médiane, maximum du
/// liquide sur 863 relevés.
const LIQUIDE_MINIMAL: i32 = 36_900;
const LIQUIDE_MEDIAN: i32 = 50_700;
const LIQUIDE_MAXIMAL: i32 = 51_300;

/// La consigne subie pendant toute la session : `pwm = 64` sur 255, soit 25 %.
///
/// C'est la valeur que la régulation doit rendre impossible.
const DUTY_SUBI: u8 = 25;

/// Une température tiède, dans le premier segment de la courbe par défaut, choisie pour que
/// l'interpolation tombe juste : 30 + (36 − 35) × 3 = 33 %.
const TIEDE: i32 = degres(36);
const CONSIGNE_TIEDE: u8 = 33;

/// Une température chaude, dans le second segment : 60 + (47 − 45) × 8 = 76 %.
const CHAUD: i32 = degres(47);
const CONSIGNE_CHAUD: u8 = 76;

/// Le milieu du premier segment : 30 + (40 − 35) × 3 = 45 %.
const MEDIAN: i32 = degres(40);
const CONSIGNE_MEDIANE: u8 = 45;

// ---------------------------------------------------------------------------
// Aides — la courbe
// ---------------------------------------------------------------------------

/// Une courbe qu'on exige valide, avec un refus qui recopie les paliers fautifs.
fn courbe(paliers: &[(i32, u8)]) -> Courbe {
    match Courbe::depuis(paliers) {
        Ok(courbe) => courbe,
        Err(erreur) => panic!(
            "Ces paliers devaient être acceptés : {paliers:?}\n  Refus : {}",
            erreur.raison
        ),
    }
}

/// Une courbe qu'on exige refusée, et le refus lui-même.
fn refus_de_courbe(paliers: &[(i32, u8)]) -> CourbeInvalide {
    match Courbe::depuis(paliers) {
        Ok(acceptee) => panic!(
            "Ces paliers devaient être refusés, ils ont été acceptés et rendent {:?} :\n  {paliers:?}",
            acceptee.paliers()
        ),
        Err(erreur) => erreur,
    }
}

/// Exige une consigne exacte. À réserver aux températures où l'arithmétique entière tombe juste,
/// sans quoi c'est un arrondi qu'on fige et non un comportement.
fn exige_consigne(courbe: &Courbe, temperature: i32, attendue: u8, contexte: &str) {
    let obtenue = courbe.consigne(temperature);
    assert_eq!(
        obtenue,
        attendue,
        "{contexte} : à {temperature} m°C la courbe {:?} doit rendre {attendue} %, elle rend \
         {obtenue} %",
        courbe.paliers()
    );
}

/// Exige une consigne encadrée, bornes comprises — la forme à employer dès que l'interpolation ne
/// tombe pas sur un entier.
fn exige_entre(courbe: &Courbe, temperature: i32, bas: u8, haut: u8, contexte: &str) {
    let obtenue = courbe.consigne(temperature);
    assert!(
        (bas..=haut).contains(&obtenue),
        "{contexte} : à {temperature} m°C la courbe {:?} doit rendre entre {bas} % et {haut} %, \
         elle rend {obtenue} %",
        courbe.paliers()
    );
}

// ---------------------------------------------------------------------------
// Aides — la boucle
// ---------------------------------------------------------------------------

/// Les écritures faites, dans l'ordre où les tours les ont produites.
///
/// C'est le seul « matériel » de ce fichier : un `Vec`. Une régulation qui écrirait vraiment sur un
/// `pwm*` ne serait pas testable, et une qui se contenterait de rendre la bonne consigne sans dire
/// **quand** ne le serait pas non plus — d'où l'accumulation sur tout un scénario.
#[derive(Default)]
struct Enregistreur {
    faites: Vec<(String, u8)>,
    /// Ce que chaque canal **porte**, tel que le démon le relit avant chaque tour (#110).
    ///
    /// ⚠️ **Ajouté mécaniquement par #110, et aucune assertion de ce fichier n'a changé.**
    /// `Regulation::tour` prend désormais la relecture en argument et décide dessus. Le matériel
    /// modélisé ici est donc **obéissant** — ce qu'on lui écrit, il le porte —, ce qui est
    /// exactement l'hypothèse implicite de tous les scénarios de #99 : ils comptent des écritures
    /// sur un matériel dont personne ne doutait qu'il appliquât. Un matériel qui encaisse sans
    /// bouger est le sujet de `spec_relecture_consigne.rs`, et de lui seul.
    ///
    /// Sous cette hypothèse les deux règles coïncident, et pas seulement en pratique : un canal
    /// jamais écrit est écrit quoi qu'il porte, et un canal déjà écrit porte exactement sa dernière
    /// consigne — « ce qu'il porte diffère » et « ce qu'on a écrit diffère » y sont la même
    /// condition. La valeur de départ ci-dessous est donc **inobservable**, et c'est vérifié : les
    /// trente-neuf tests passent à l'identique de 0 % à 100 %.
    portees: BTreeMap<String, Option<u8>>,
}

impl Enregistreur {
    fn neuf() -> Enregistreur {
        Enregistreur::default()
    }

    /// Un tour de télémétrie : la régulation dit quoi écrire, l'enregistreur le note.
    ///
    /// Rend les écritures de **ce** tour, triées par canal — l'ordre dans lequel `tour` les rend
    /// n'est pas un contrat, et un test qui l'exigerait figerait un détail d'implémentation.
    fn tour(&mut self, regulation: &mut Regulation, liquide: Option<i32>) -> Vec<(String, u8)> {
        // Un canal dont on n'a encore rien vu porte le duty d'allumage des `nzxtsmart2` : `pwm =
        // 64`, soit 25 % — la valeur unique des 863 relevés du 2026-08-15.
        for canal in regulation.canaux() {
            self.portees.entry(canal).or_insert(Some(DUTY_SUBI));
        }

        let mut ce_tour: Vec<(String, u8)> = regulation
            .tour(liquide, &self.portees)
            .into_iter()
            // Le motif ajouté par #110 est le sujet de son fichier ; ici on ne compte que ce qui
            // part sur le bus, comme avant lui.
            .map(|ecriture: Ecriture| (ecriture.canal, ecriture.consigne))
            .collect();
        ce_tour.sort();

        // Deux écritures sur le même canal dans le même tour, ce serait une trame de plus sur le
        // bus pour rien — et le signe qu'une couche décide deux fois.
        let mut vus: BTreeSet<&str> = BTreeSet::new();
        for (canal, _) in &ce_tour {
            assert!(
                vus.insert(canal.as_str()),
                "« {canal} » est écrit deux fois dans le même tour : {ce_tour:?}"
            );
        }

        // Le matériel obéit : ce qu'on lui écrit, il le porte au tour suivant.
        for (canal, consigne) in &ce_tour {
            self.portees.insert(canal.clone(), Some(*consigne));
        }

        self.faites.extend(ce_tour.iter().cloned());
        ce_tour
    }

    /// Toutes les écritures depuis le début, tous tours confondus.
    fn tout(&self) -> &[(String, u8)] {
        &self.faites
    }

    /// Les canaux qui ont reçu au moins une écriture.
    fn canaux_touches(&self) -> BTreeSet<String> {
        self.faites.iter().map(|(canal, _)| canal.clone()).collect()
    }

    /// Les consignes reçues par un canal, dans l'ordre.
    fn consignes_de(&self, canal: &str) -> Vec<u8> {
        self.faites
            .iter()
            .filter(|(nom, _)| nom == canal)
            .map(|(_, consigne)| *consigne)
            .collect()
    }
}

/// L'attendu d'un tour, sous la même forme que ce que l'enregistreur rend.
fn ecritures(paires: &[(&str, u8)]) -> Vec<(String, u8)> {
    let mut attendues: Vec<(String, u8)> = paires
        .iter()
        .map(|(canal, consigne)| ((*canal).to_owned(), *consigne))
        .collect();
    attendues.sort();
    attendues
}

/// Une régulation sur la courbe par défaut, avec ces canaux activés.
fn regulation_sur(canaux: &[&str]) -> Regulation {
    let mut regulation = Regulation::nouvelle(Courbe::defaut());
    for canal in canaux {
        regulation.activer(canal);
    }
    regulation
}

// ---------------------------------------------------------------------------
// Aides — le fichier d'état
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

/// Les lignes actives d'un genre donné (`courbe`, `canal`).
fn lignes_du_genre(texte: &str, genre: &str) -> Vec<String> {
    lignes_actives(texte)
        .into_iter()
        .filter(|ligne| ligne.split_whitespace().next() == Some(genre))
        .collect()
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

/// Le fichier d'état qu'on exige relisible, avec un refus qui recopie le texte fautif.
fn decoder(texte: &str) -> Regulation {
    match Regulation::decoder(texte) {
        Ok(regulation) => regulation,
        Err(erreur) => panic!(
            "Ce fichier devait se relire.\n  Refus ligne {} : {}\n  Texte :\n{texte}",
            erreur.ligne, erreur.raison
        ),
    }
}

/// Le fichier d'état qu'on exige refusé, et le refus lui-même.
fn refus_de_fichier(texte: &str) -> RegulationInvalide {
    match Regulation::decoder(texte) {
        Ok(regulation) => panic!(
            "Ce fichier devait être refusé, il a été accepté et rend les canaux {:?} sur la courbe \
             {:?}.\n  Texte :\n{texte}",
            regulation.canaux(),
            regulation.courbe().paliers()
        ),
        Err(erreur) => erreur,
    }
}

/// Un dossier de travail sous `std::env::temp_dir()`, effacé à la fin du test.
///
/// L'effacement passe par `Drop` : un test qui échoue part en `panic!`, et une fin de test jamais
/// atteinte laisserait le dossier derrière elle à chaque régression.
struct DossierJetable {
    chemin: PathBuf,
}

impl DossierJetable {
    /// `nom` doit être le nom du test : `cargo` les exécute en parallèle dans un même processus.
    fn neuf(nom: &str) -> DossierJetable {
        let chemin = std::env::temp_dir().join(format!(
            "reverb-spec-regulation-{}-{nom}",
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
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tous les tests qui suivent supposent que les canaux diffèrent, que le repli ne coïncide avec
    // aucune des consignes témoins, et que la courbe par défaut est bien celle de l'issue. Si l'un
    // de ces repères se dégradait, plusieurs tests deviendraient vrais sans rien vérifier — et
    // personne ne le verrait. Ce test est là pour que la panne soit ici.
    let canaux = [
        ("CANAL_1", CANAL_1),
        ("CANAL_2", CANAL_2),
        ("CANAL_3", CANAL_3),
        ("CANAL_KRAKEN", CANAL_KRAKEN),
    ];
    for (i, (nom, canal)) in canaux.iter().enumerate() {
        assert!(!canal.is_empty(), "{nom} doit porter un nom");
        for (autre_nom, autre) in canaux.iter().skip(i + 1) {
            assert_ne!(
                canal, autre,
                "{nom} et {autre_nom} doivent différer, sinon deux canaux n'en feraient qu'un et \
                 les tests de confinement ne testeraient rien"
            );
        }
    }

    // Le repli est un critère d'acceptation écrit : « le liquide illisible fait retomber la
    // consigne à 50 % ».
    assert_eq!(
        REPLI, 50,
        "la consigne de repli est de 50 % (issue #99), trouvé {REPLI}"
    );

    // Et la sonde aussi : « la sonde est `kraken2023elite:coolant-temp` ».
    assert_eq!(
        SONDE_DU_LIQUIDE, "kraken2023elite:coolant-temp",
        "la régulation suit le liquide, et lui seul (issue #99)"
    );

    // Les trois consignes témoins doivent différer du repli, sinon les tests de repli
    // confondraient « la courbe a parlé » et « la sonde s'est tue ».
    let defaut = Courbe::defaut();
    for (nom, temperature, attendue) in [
        ("TIEDE", TIEDE, CONSIGNE_TIEDE),
        ("CHAUD", CHAUD, CONSIGNE_CHAUD),
        ("MEDIAN", MEDIAN, CONSIGNE_MEDIANE),
    ] {
        assert_eq!(
            defaut.consigne(temperature),
            attendue,
            "{nom} : le témoin de ce fichier doit valoir {attendue} % sur la courbe par défaut"
        );
        assert_ne!(
            attendue, REPLI,
            "{nom} vaut la consigne de repli : un test de repli ne prouverait plus rien"
        );
    }
    assert_ne!(
        CONSIGNE_TIEDE, CONSIGNE_CHAUD,
        "les deux températures témoins doivent donner deux consignes distinctes"
    );

    // Le duty subi est bien en dessous de tout ce que la courbe sait produire : c'est ce qui donne
    // son sens au test de la session mesurée. Et les trois relevés sont ordonnés.
    //
    // Ces trois-là ne comparent que des constantes de ce fichier : elles sont donc vérifiées à la
    // compilation, ce qui est mieux — la panne arrive avant même que le test ne tourne.
    const {
        assert!(
            DUTY_SUBI < CONSIGNE_TIEDE,
            "le duty subi doit être inférieur à la plus basse consigne témoin, sinon « la \
             régulation aurait changé quelque chose » serait indémontrable"
        );
        assert!(
            LIQUIDE_MINIMAL < LIQUIDE_MEDIAN,
            "les relevés de la session doivent être ordonnés : minimum, puis médiane"
        );
        assert!(
            LIQUIDE_MEDIAN < LIQUIDE_MAXIMAL,
            "les relevés de la session doivent être ordonnés : médiane, puis maximum"
        );
    }
}

// ---------------------------------------------------------------------------
// A — la courbe, qui est le cœur pur du sujet
// ---------------------------------------------------------------------------

#[test]
fn la_courbe_par_defaut_est_le_tableau_de_l_issue() {
    // Issue #99 : « Valeur par défaut, calée sur les mesures ci-dessus : ≤ 35 °C → 30 %,
    // 45 °C → 60 %, ≥ 50 °C → 100 %. »
    //
    // Le tableau est pinné aux trois paliers **et** aux trois consignes qu'ils produisent : une
    // courbe qui garderait les bons paliers en les interpolant à l'envers passerait la première
    // moitié de ce test et pas la seconde.
    let defaut = Courbe::defaut();
    assert_eq!(
        defaut.paliers(),
        [(degres(35), 30), (degres(45), 60), (degres(50), 100)],
        "la courbe par défaut est le tableau de l'issue #99, en millidegrés"
    );

    exige_consigne(&defaut, degres(35), 30, "le premier palier");
    exige_consigne(&defaut, degres(45), 60, "le palier du milieu");
    exige_consigne(&defaut, degres(50), 100, "le dernier palier");
}

#[test]
fn la_courbe_rend_trente_a_trente_degres_soixante_a_quarante_cinq_et_cent_a_cinquante_cinq() {
    // Test d'intention n° 1 de l'issue, mot pour mot : « la courbe rend 30 % à 30 °C, 60 % à 45 °C,
    // 100 % à 55 °C ».
    //
    // Les deux extrêmes sont hors des paliers — 30 °C est sous le premier, 55 °C au-dessus du
    // dernier — et c'est voulu : l'issue vérifie du même geste la valeur et le fait qu'on ne sorte
    // pas de la table.
    let defaut = Courbe::defaut();
    exige_consigne(&defaut, degres(30), 30, "sous le premier palier");
    exige_consigne(&defaut, degres(45), 60, "sur le palier du milieu");
    exige_consigne(&defaut, degres(55), 100, "au-dessus du dernier palier");
}

#[test]
fn sous_le_premier_palier_la_courbe_rend_la_borne_sans_extrapoler() {
    // Critère d'acceptation : « une température hors des paliers connus est ramenée aux bornes,
    // pas extrapolée ».
    //
    // La faute visée est la prolongation de la droite : à 25 °C, une extrapolation du segment
    // 35→45 rendrait 0 %, c'est-à-dire des ventilateurs **à l'arrêt** sur un circuit qui démarre.
    // C'est pire que le défaut qu'on corrige.
    let defaut = Courbe::defaut();
    let premiere = 30;

    for froid in [
        i32::MIN,
        i32::MIN + 1,
        degres(-273),
        degres(-40),
        0,
        degres(20),
        degres(34),
        degres(35) - 1,
    ] {
        exige_consigne(&defaut, froid, premiere, "sous le premier palier");
    }
}

#[test]
fn au_dessus_du_dernier_palier_la_courbe_rend_la_borne_sans_extrapoler() {
    // L'autre moitié du même critère, et la faute y est symétrique : une extrapolation rendrait
    // plus de 100 % — donc un `u8` qui déborde, ou une consigne que le noyau refusera.
    //
    // 51,3 °C est le maximum relevé pendant la session du 2026-08-15 : ce n'est pas une valeur de
    // laboratoire, c'est le régime réel de cette machine.
    let defaut = Courbe::defaut();
    let derniere = 100;

    for chaud in [
        degres(50),
        degres(50) + 1,
        LIQUIDE_MAXIMAL,
        degres(60),
        degres(100),
        degres(1_000),
        i32::MAX - 1,
        i32::MAX,
    ] {
        exige_consigne(&defaut, chaud, derniere, "au-dessus du dernier palier");
    }
}

#[test]
fn la_courbe_interpole_lineairement_entre_deux_paliers() {
    // Issue #99 : « interpolée linéairement entre les paliers ».
    //
    // Les points pinçés ici tombent tous juste en arithmétique entière — le premier segment monte
    // de 3 % par degré, le second de 8 % — donc aucun de ces tests ne fige un arrondi. Les points
    // qui ne tombent pas juste sont encadrés plus bas, et c'est délibéré : l'issue ne tranche pas
    // entre troncature et arrondi au plus proche, et ce fichier n'a pas à le faire à sa place.
    let defaut = Courbe::defaut();

    // Premier segment : (35 °C, 30 %) → (45 °C, 60 %), soit 3 % par degré.
    for degre in 35..=45 {
        let attendue = 30 + u8::try_from((degre - 35) * 3).expect("30 + 30 tient dans un u8");
        exige_consigne(&defaut, degres(degre), attendue, "le premier segment");
    }

    // Second segment : (45 °C, 60 %) → (50 °C, 100 %), soit 8 % par degré.
    for degre in 45..=50 {
        let attendue = 60 + u8::try_from((degre - 45) * 8).expect("60 + 40 tient dans un u8");
        exige_consigne(&defaut, degres(degre), attendue, "le second segment");
    }

    // Deux demi-degrés qui tombent juste eux aussi : 45,5 °C → 60 + 4, et 47,5 °C → 60 + 20.
    exige_consigne(
        &defaut,
        degres(45) + 500,
        64,
        "un demi-degré, second segment",
    );
    exige_consigne(&defaut, degres(47) + 500, 80, "le milieu du second segment");

    // Et un point qui ne tombe pas juste — 35,5 °C vaudrait 31,5 % — encadré et non pointé.
    exige_entre(
        &defaut,
        degres(35) + 500,
        31,
        32,
        "un demi-degré, premier segment",
    );

    // Le palier du milieu appartient aux deux segments : il doit rendre la même chose vu de
    // gauche et vu de droite. Une implémentation qui choisirait mal son intervalle se ferait
    // attraper ici, à un millidegré près.
    exige_consigne(
        &defaut,
        degres(45) - 1,
        59,
        "juste avant le palier du milieu",
    );
    exige_consigne(&defaut, degres(45), 60, "sur le palier du milieu");
    exige_consigne(
        &defaut,
        degres(45) + 1,
        60,
        "juste après le palier du milieu",
    );
}

#[test]
fn une_consigne_calculee_reste_toujours_dans_zero_cent() {
    // Critère d'acceptation : « la courbe est bornée : une consigne calculée reste dans 0–100 ».
    //
    // Balayage de −100 °C à +200 °C par pas d'un dixième de degré, sur quatre courbes de formes
    // très différentes : la courbe par défaut, une plate, une quasi verticale — un degré pour
    // passer de 0 à 100, de quoi faire déborder une interpolation naïve — et une qui commence à
    // zéro pour cent.
    let courbes = [
        Courbe::defaut(),
        courbe(&[(degres(45), 80)]),
        courbe(&[(degres(40), 0), (degres(41), 100)]),
        courbe(&[(degres(0), 0), (degres(90), 100)]),
    ];

    for c in &courbes {
        for temperature in (degres(-100)..=degres(200)).step_by(100) {
            let consigne = c.consigne(temperature);
            assert!(
                consigne <= 100,
                "à {temperature} m°C, la courbe {:?} rend {consigne} % : au-delà de cent pour cent \
                 il n'y a plus de ventilateur",
                c.paliers()
            );
        }
    }
}

#[test]
fn une_temperature_extreme_ne_fait_ni_deborder_ni_sortir_des_bornes() {
    // Le corollaire du critère précédent, et il vise une faute d'arithmétique précise : une
    // implémentation qui calculerait `(temperature - premier_palier)` **avant** de borner déborde
    // dès que la sonde rend une valeur aberrante. En `debug`, c'est une panique — dans le fil qui
    // sert aussi le socket.
    //
    // Une sonde ne rend pas `i32::MIN` en temps normal. Mais rien ne le lui interdit, et le projet
    // a déjà vu ce contrôleur rendre n'importe quoi avant de se taire tout à fait.
    let courbes = [
        Courbe::defaut(),
        courbe(&[(degres(45), 80)]),
        courbe(&[(i32::MIN / 2, 10), (i32::MAX / 2, 90)]),
    ];

    for c in &courbes {
        let premiere = c
            .paliers()
            .first()
            .expect("une courbe a au moins un palier")
            .1;
        let derniere = c
            .paliers()
            .last()
            .expect("une courbe a au moins un palier")
            .1;

        for temperature in [i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX] {
            let consigne = c.consigne(temperature);
            assert!(
                consigne <= 100,
                "à {temperature} m°C, la courbe {:?} rend {consigne} %",
                c.paliers()
            );
            assert!(
                (premiere.min(derniere)..=premiere.max(derniere)).contains(&consigne),
                "à {temperature} m°C, la courbe {:?} rend {consigne} % : hors des paliers connus, \
                 une consigne ne peut valoir que ce que la table contient déjà",
                c.paliers()
            );
        }
    }
}

#[test]
fn la_courbe_ne_redescend_jamais_quand_le_liquide_monte() {
    // Ce n'est pas un critère écrit, c'est ce qui rend les autres vrais : une erreur de signe dans
    // l'interpolation produirait une courbe parfaitement bornée, parfaitement continue, et qui
    // ralentirait les ventilateurs pendant que le circuit chauffe. Aucun des autres tests ne la
    // verrait, et le boîtier ne dirait rien.
    let defaut = Courbe::defaut();
    let mut precedente = defaut.consigne(degres(-100));

    for temperature in (degres(-100)..=degres(200)).step_by(100) {
        let consigne = defaut.consigne(temperature);
        assert!(
            consigne >= precedente,
            "à {temperature} m°C la consigne retombe de {precedente} % à {consigne} % : un \
             ventilateur qui ralentit quand le liquide chauffe est l'inverse exact du défaut de #99"
        );
        precedente = consigne;
    }

    assert_eq!(
        precedente, 100,
        "au bout du balayage, la courbe par défaut doit être à fond"
    );
}

#[test]
fn une_courbe_a_un_seul_palier_est_plate() {
    // C'est la forme la plus nue de « ramenée aux bornes, pas extrapolée » : avec un seul palier il
    // n'y a aucune pente à prolonger, donc aucune ambiguïté possible. Une implémentation qui
    // extrapole se trahit ici même si elle passe partout ailleurs, parce qu'elle n'a rien pour
    // extrapoler et doit malgré tout rendre quelque chose.
    let plate = courbe(&[(degres(45), 80)]);

    for temperature in [
        i32::MIN,
        degres(-50),
        0,
        degres(44),
        degres(45),
        degres(46),
        degres(200),
        i32::MAX,
    ] {
        exige_consigne(&plate, temperature, 80, "une courbe à un seul palier");
    }
}

#[test]
fn une_courbe_reglee_est_suivie_telle_quelle() {
    // Issue #99 : « la consigne suit une courbe température → pourcentage, **réglable** et
    // conservée ». Réglable veut dire suivie : une implémentation qui garderait la table par défaut
    // en coulisses passerait tous les tests de la courbe par défaut.
    let mienne = courbe(&[(degres(30), 40), (degres(50), 60), (degres(60), 100)]);

    assert_eq!(
        mienne.paliers(),
        [(degres(30), 40), (degres(50), 60), (degres(60), 100)],
        "une courbe réglée garde exactement les paliers qu'on lui a donnés"
    );

    // Ses propres paliers.
    exige_consigne(&mienne, degres(30), 40, "le premier palier réglé");
    exige_consigne(&mienne, degres(50), 60, "le palier du milieu réglé");
    exige_consigne(&mienne, degres(60), 100, "le dernier palier réglé");

    // Ses propres bornes : 45 °C n'est plus 60 % comme sur la courbe par défaut.
    exige_consigne(
        &mienne,
        degres(40),
        50,
        "le milieu du premier segment réglé",
    );
    exige_consigne(&mienne, degres(45), 55, "45 °C sur la courbe réglée");
    exige_consigne(&mienne, degres(20), 40, "sous le premier palier réglé");
    exige_consigne(
        &mienne,
        degres(80),
        100,
        "au-dessus du dernier palier réglé",
    );

    assert_ne!(
        mienne.consigne(degres(45)),
        Courbe::defaut().consigne(degres(45)),
        "à 45 °C, la courbe réglée et la courbe par défaut doivent différer — sans quoi ce test ne \
         prouverait pas que c'est bien la courbe réglée qu'on suit"
    );
}

#[test]
fn une_courbe_qui_promet_plus_de_cent_est_refusee_en_le_nommant() {
    // Corollaire de « une consigne calculée reste dans 0–100 » : si une table peut contenir 150,
    // alors ou bien `consigne` déborde, ou bien elle rabote en silence une valeur que l'utilisateur
    // a écrite. Les deux sont mauvais, et le refus à la construction évite les deux.
    for consigne in [101u8, 150, 200, 255] {
        let erreur = refus_de_courbe(&[(degres(35), 30), (degres(45), consigne)]);
        assert!(
            erreur.raison.contains(&consigne.to_string()),
            "le refus doit nommer la consigne fautive « {consigne} ». Raison obtenue : {}",
            erreur.raison
        );
    }
}

#[test]
fn une_courbe_vide_ou_desordonnee_est_refusee_en_nommant_le_palier_fautif() {
    // Trois façons de ne pas être une courbe, et une seule conduite : refuser en nommant.
    //
    // — **vide** : il n'y a aucune consigne à rendre, et rendre 0 % serait arrêter les
    //   ventilateurs sur une table que personne n'a écrite ;
    // — **répétée** : deux consignes à 45 °C se contredisent, et n'en garder qu'une ferait dépendre
    //   la régulation de l'ordre des lignes du fichier ;
    // — **désordonnée** : réordonner en silence, c'est « compléter au jugé » ce qu'un humain a
    //   tapé. Le projet refuse déjà de le faire pour `eclairage.conf`.
    let vide = refus_de_courbe(&[]);
    assert!(
        !vide.raison.trim().is_empty(),
        "une courbe sans palier doit être refusée avec une raison, pas avec un silence"
    );

    for paliers in [
        &[(degres(35), 30), (degres(45), 60), (degres(45), 90)][..],
        &[(degres(45), 60), (degres(45), 60)][..],
    ] {
        let erreur = refus_de_courbe(paliers);
        assert!(
            erreur.raison.contains("45"),
            "le refus doit nommer la température répétée (45 °C). Raison obtenue : {}",
            erreur.raison
        );
    }

    for paliers in [
        &[(degres(50), 100), (degres(35), 30)][..],
        &[(degres(35), 30), (degres(50), 100), (degres(45), 60)][..],
    ] {
        let erreur = refus_de_courbe(paliers);
        assert!(
            !erreur.raison.trim().is_empty(),
            "une courbe dont les paliers ne montent pas doit être refusée avec une raison. \
             Paliers : {paliers:?}"
        );
    }
}

#[test]
fn les_bornes_zero_et_cent_sont_des_consignes_acceptables() {
    // « Reste dans 0–100 » : les deux bornes sont dedans. Refuser 100 % rendrait la courbe par
    // défaut inconstructible, et refuser 0 % interdirait une courbe silencieuse au repos — un
    // réglage que rien dans l'issue n'écarte.
    let extremes = courbe(&[(degres(20), 0), (degres(70), 100)]);
    exige_consigne(
        &extremes,
        degres(20),
        0,
        "une consigne nulle est acceptable",
    );
    exige_consigne(
        &extremes,
        degres(70),
        100,
        "une consigne pleine est acceptable",
    );
    exige_consigne(&extremes, degres(10), 0, "sous le premier palier à zéro");
    exige_consigne(
        &extremes,
        degres(80),
        100,
        "au-dessus du dernier palier à cent",
    );
}

#[test]
fn la_courbe_repond_a_la_session_mesuree_du_2026_08_15() {
    // Ce test relie la courbe à ce qui a motivé l'issue : 863 relevés, un duty unique de 64 sur 255
    // — 25 % — et quarante-cinq minutes au-dessus de 50 °C.
    //
    // Il ne vérifie pas une implémentation, il vérifie que la courbe **répond au problème posé** :
    // aux trois régimes relevés, elle n'aurait jamais laissé les ventilateurs là où ils étaient.
    let defaut = Courbe::defaut();

    for (nom, releve) in [
        ("le minimum", LIQUIDE_MINIMAL),
        ("la médiane", LIQUIDE_MEDIAN),
        ("le maximum", LIQUIDE_MAXIMAL),
    ] {
        let consigne = defaut.consigne(releve);
        assert!(
            consigne > DUTY_SUBI,
            "{nom} de la session ({releve} m°C) donne {consigne} %, soit au plus les {DUTY_SUBI} % \
             subis pendant 72 minutes : la courbe ne corrigerait rien"
        );
    }

    // À la médiane comme au maximum, le liquide est au-dessus du dernier palier : plein régime.
    exige_consigne(&defaut, LIQUIDE_MEDIAN, 100, "la médiane de la session");
    exige_consigne(&defaut, LIQUIDE_MAXIMAL, 100, "le maximum de la session");

    // Au minimum relevé — 36,9 °C — on est juste au-dessus du premier palier : 30 + 1,9 × 3.
    exige_entre(&defaut, LIQUIDE_MINIMAL, 35, 36, "le minimum de la session");
}

// ---------------------------------------------------------------------------
// B — la boucle : ce qui est écrit, et surtout ce qui ne l'est pas
// ---------------------------------------------------------------------------

#[test]
fn un_canal_regule_recoit_la_consigne_calculee_depuis_le_liquide() {
    // Critère d'acceptation : « un canal régulé reçoit une consigne calculée depuis le liquide, à
    // chaque changement de palier ».
    //
    // Le premier tour est le cas le plus simple et le plus important : sans lui, rien n'est jamais
    // écrit et c'est exactement l'état actuel de la machine.
    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut enregistreur = Enregistreur::neuf();

    assert_eq!(
        enregistreur.tour(&mut regulation, Some(MEDIAN)),
        ecritures(&[(CANAL_1, CONSIGNE_MEDIANE)]),
        "un canal régulé, un liquide lisible : la consigne de la courbe part sur le canal"
    );
}

#[test]
fn chaque_changement_de_palier_produit_une_ecriture_et_une_seule() {
    // La suite du même critère : la consigne suit le liquide, montée comme descente. La rampe
    // choisie traverse les deux segments et les deux bornes.
    //
    // À chaque tour, deux exigences : la valeur écrite est **exactement** celle de la courbe, et
    // aucune écriture n'a lieu si la consigne n'a pas changé.
    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut enregistreur = Enregistreur::neuf();
    let defaut = Courbe::defaut();

    let mut derniere: Option<u8> = None;
    let montee: Vec<i32> = (degres(20)..=degres(60)).step_by(1000).collect();
    let rampe: Vec<i32> = montee
        .iter()
        .copied()
        .chain(montee.iter().copied().rev())
        .collect();

    for temperature in rampe {
        let attendue = defaut.consigne(temperature);
        let faites = enregistreur.tour(&mut regulation, Some(temperature));

        if derniere == Some(attendue) {
            assert!(
                faites.is_empty(),
                "à {temperature} m°C la consigne vaut toujours {attendue} % : aucune écriture ne \
                 doit partir, or {faites:?}"
            );
        } else {
            assert_eq!(
                faites,
                ecritures(&[(CANAL_1, attendue)]),
                "à {temperature} m°C la consigne passe à {attendue} % : une écriture, et une seule"
            );
            derniere = Some(attendue);
        }
    }

    // La rampe monte puis redescend : la dernière consigne écrite doit être celle du bas.
    assert_eq!(
        enregistreur.consignes_de(CANAL_1).last().copied(),
        Some(defaut.consigne(degres(20))),
        "la rampe finit au froid : la dernière consigne écrite est celle du premier palier"
    );
}

#[test]
fn une_temperature_inchangee_ne_produit_aucune_ecriture() {
    // Critère d'acceptation : « une température inchangée ne produit **aucune** écriture ».
    //
    // ⚠️ C'est le critère qui vient du cache de LED : « aucune de ces cibles n'a de watchdog,
    // réécrire une consigne identique ne fait que consommer le bus ». Le tour de télémétrie passe
    // une fois par seconde, donc l'écart entre une régulation qui se tait et une qui réécrit, c'est
    // 86 400 trames par jour pour rien.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    let mut enregistreur = Enregistreur::neuf();

    assert_eq!(
        enregistreur.tour(&mut regulation, Some(TIEDE)),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE), (CANAL_2, CONSIGNE_TIEDE)]),
        "le premier tour écrit : les canaux ne savent rien encore"
    );

    // Mille tours à la même température : une heure de démon, pas une trame.
    for tour in 1..=1_000u32 {
        let faites = enregistreur.tour(&mut regulation, Some(TIEDE));
        assert!(
            faites.is_empty(),
            "au tour n° {tour}, la température n'a pas bougé : rien ne doit partir, or {faites:?}"
        );
    }

    // Et des variations qui ne changent pas la consigne ne changent rien non plus. Un millidegré
    // de plus, c'est ce que la sonde du liquide fait toute la journée.
    for ecart in [1, -1, 2, -2, 10, -10] {
        let faites = enregistreur.tour(&mut regulation, Some(TIEDE + ecart));
        assert!(
            faites.is_empty(),
            "un écart de {ecart} m°C ne change pas la consigne : rien ne doit partir, or {faites:?}"
        );
    }

    assert_eq!(
        enregistreur.tout().len(),
        2,
        "sur mille tours, seules les deux écritures du premier doivent avoir eu lieu : {:?}",
        enregistreur.tout()
    );
}

#[test]
fn un_canal_non_regule_n_apparait_dans_aucune_ecriture() {
    // Critère d'acceptation : « un canal non régulé n'est jamais écrit ».
    //
    // Deux canaux témoins, et deux raisons distinctes de ne pas y toucher :
    //
    // — `CANAL_KRAKEN` est **hors scope** : « leur firmware régule déjà correctement, mesuré. On ne
    //   remplace pas une régulation qui marche par du code à écrire ». Lui écrire, c'est écraser
    //   une courbe firmware par une boucle hôte ;
    // — `CANAL_3` est un canal du même contrôleur que les deux régulés, simplement pas activé.
    //   C'est le témoin qui attrape une implémentation qui écrirait « à tout le `nzxtsmart2` ».
    //
    // Le scénario est long et irrégulier à dessein : une régulation ne se trompe pas de canal sur
    // un seul tour.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    let mut enregistreur = Enregistreur::neuf();

    for temperature in (degres(20)..=degres(60)).step_by(137) {
        enregistreur.tour(&mut regulation, Some(temperature));
        enregistreur.tour(&mut regulation, None);
        enregistreur.tour(&mut regulation, Some(temperature));
    }

    let touches = enregistreur.canaux_touches();
    for interdit in [CANAL_3, CANAL_KRAKEN] {
        assert!(
            !touches.contains(interdit),
            "« {interdit} » n'est pas régulé et ne doit recevoir aucune écriture. Canaux touchés : \
             {touches:?}"
        );
        assert!(
            enregistreur.consignes_de(interdit).is_empty(),
            "« {interdit} » a reçu {:?}",
            enregistreur.consignes_de(interdit)
        );
    }

    assert!(
        !enregistreur.consignes_de(CANAL_1).is_empty(),
        "le scénario doit avoir écrit sur les canaux régulés, sinon il ne prouve rien"
    );
}

#[test]
fn le_liquide_illisible_produit_le_repli_une_fois_et_pas_a_chaque_tour() {
    // Deux critères d'acceptation d'un coup :
    //
    // — « le liquide illisible produit la consigne de repli » ;
    // — « … une fois, et pas à chaque tour ».
    //
    // Le second compte autant que le premier : le Kraken se plante périodiquement, et une
    // régulation qui réécrirait 50 % toutes les secondes pendant qu'il est muet passerait sa vie
    // sur le bus sans rien changer à rien.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    let mut enregistreur = Enregistreur::neuf();

    assert_eq!(
        enregistreur.tour(&mut regulation, None),
        ecritures(&[(CANAL_1, REPLI), (CANAL_2, REPLI)]),
        "sonde illisible dès le premier tour : le repli part sur chaque canal régulé"
    );

    for tour in 1..=500u32 {
        let faites = enregistreur.tour(&mut regulation, None);
        assert!(
            faites.is_empty(),
            "au tour n° {tour} de silence, le repli est déjà posé : rien ne doit partir, or \
             {faites:?}"
        );
    }

    assert_eq!(
        enregistreur.tout().len(),
        2,
        "sur cinq cents tours muets, deux écritures — une par canal — et pas une de plus : {:?}",
        enregistreur.tout()
    );
}

#[test]
fn le_repli_n_est_jamais_la_derniere_valeur_connue() {
    // Issue #99 : « le liquide illisible fait retomber la consigne à 50 %, **jamais à la dernière
    // valeur connue**. C'est le mode de défaillance rassurant que le projet refuse partout
    // ailleurs : une consigne figée à 30 % derrière une sonde morte, c'est un CPU qui chauffe sans
    // que rien ne le signale. »
    //
    // Deux cas symétriques, et il faut les deux : le liquide était froid — garder la valeur serait
    // sous-ventiler —, puis le liquide était brûlant — garder la valeur serait sur-ventiler pour
    // rien, et masquer la panne aussi sûrement.
    for (temperature, avant) in [(TIEDE, CONSIGNE_TIEDE), (degres(55), 100)] {
        let mut regulation = regulation_sur(&[CANAL_1]);
        let mut enregistreur = Enregistreur::neuf();

        assert_eq!(
            enregistreur.tour(&mut regulation, Some(temperature)),
            ecritures(&[(CANAL_1, avant)]),
            "le liquide est lisible : la courbe s'applique"
        );

        let faites = enregistreur.tour(&mut regulation, None);
        assert_eq!(
            faites,
            ecritures(&[(CANAL_1, REPLI)]),
            "la sonde se tait : le repli s'applique, et il ne dépend pas de ce qu'on venait de lire"
        );
        assert_ne!(
            faites,
            ecritures(&[(CANAL_1, avant)]),
            "garder {avant} % derrière une sonde morte, c'est laisser croire que le liquide est \
             mesuré alors que plus rien ne le mesure"
        );
    }
}

#[test]
fn le_retour_du_liquide_reprend_la_courbe_sans_redemarrage() {
    // Critère d'acceptation : « le retour du liquide reprend la courbe sans redémarrage ».
    //
    // C'est la moitié qu'on oublie : un repli qui s'installe pour de bon transformerait chaque
    // hoquet du Kraken — et il en a — en 50 % définitif jusqu'au prochain `systemctl restart`.
    // Le cycle est joué deux fois, pour qu'aucun état à un coup ne puisse le faire passer.
    //
    // ⚠️ **Le 4ᵉ tour était à `TIEDE` à la première écriture de ce fichier, et le test était alors
    // intenable.** Le cycle étant joué deux fois, ce 4ᵉ tour et le 1ᵉʳ du passage suivant sont le
    // MÊME tour — `Some(TIEDE)` deux fois d'affilée — et tous deux étaient exigés écrivants. Or
    // `une_temperature_inchangee_ne_produit_aucune_ecriture` interdit exactement cela, et
    // `un_repli_egal_a_ce_qui_est_deja_ecrit_ne_reecrit_rien` ferme la seule échappatoire en
    // interdisant de retenir autre chose que la valeur écrite.
    //
    // La contradiction était donc **interne à ce fichier**, entre trois de ses propres tests, et
    // non entre la spec et une implémentation. C'est le cas que le workflow prévoit : un test
    // d'intention intenable signale un critère mal posé, jamais un code à plier.
    //
    // Corrigé au plus petit : le 4ᵉ tour passe à `MEDIAN`. Aucune assertion n'est perdue — « et
    // elle continue de suivre » reste vérifié, sur une troisième valeur plutôt qu'une deuxième —
    // et le passage suivant repart bien d'une consigne différente de celle qu'il vient d'écrire.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    let mut enregistreur = Enregistreur::neuf();

    for passage in 1..=2u32 {
        assert_eq!(
            enregistreur.tour(&mut regulation, Some(TIEDE)),
            ecritures(&[(CANAL_1, CONSIGNE_TIEDE), (CANAL_2, CONSIGNE_TIEDE)]),
            "passage n° {passage} : la courbe s'applique"
        );
        assert_eq!(
            enregistreur.tour(&mut regulation, None),
            ecritures(&[(CANAL_1, REPLI), (CANAL_2, REPLI)]),
            "passage n° {passage} : la sonde se tait, le repli s'applique"
        );
        assert_eq!(
            enregistreur.tour(&mut regulation, Some(CHAUD)),
            ecritures(&[(CANAL_1, CONSIGNE_CHAUD), (CANAL_2, CONSIGNE_CHAUD)]),
            "passage n° {passage} : la sonde répond de nouveau, la courbe reprend la main — et sur \
             la valeur du moment, pas sur celle d'avant la panne"
        );
        assert_eq!(
            enregistreur.tour(&mut regulation, Some(MEDIAN)),
            ecritures(&[(CANAL_1, CONSIGNE_MEDIANE), (CANAL_2, CONSIGNE_MEDIANE)]),
            "passage n° {passage} : et elle continue de suivre"
        );
    }
}

#[test]
fn un_repli_egal_a_ce_qui_est_deja_ecrit_ne_reecrit_rien() {
    // Le point de rencontre des deux règles : « on n'écrit que ce qui change » et « la sonde muette
    // fait retomber à 50 % ». Si la consigne courante vaut déjà 50 %, la sonde peut se taire : il
    // n'y a rien à écrire.
    //
    // Le cache porte sur la **valeur écrite**, pas sur le régime qui l'a produite. Une
    // implémentation qui garderait un drapeau « je suis en repli » réécrirait ici, et réécrirait de
    // nouveau au retour de la sonde.
    //
    // La courbe est plate à la valeur du repli exprès : c'est la seule façon de construire ce cas
    // sans dépendre d'un arrondi.
    let mut regulation = Regulation::nouvelle(courbe(&[(degres(30), REPLI), (degres(60), REPLI)]));
    regulation.activer(CANAL_1);
    let mut enregistreur = Enregistreur::neuf();

    assert_eq!(
        enregistreur.tour(&mut regulation, Some(MEDIAN)),
        ecritures(&[(CANAL_1, REPLI)]),
        "la courbe plate rend exactement la consigne de repli"
    );

    for (contexte, liquide) in [
        ("la sonde se tait", None),
        ("elle se tait encore", None),
        ("elle revient", Some(MEDIAN)),
        ("elle bouge sans changer la consigne", Some(CHAUD)),
    ] {
        let faites = enregistreur.tour(&mut regulation, liquide);
        assert!(
            faites.is_empty(),
            "{contexte} : la consigne vaut déjà {REPLI} %, rien ne doit partir, or {faites:?}"
        );
    }
}

#[test]
fn activer_un_canal_en_cours_de_route_lui_donne_la_consigne_courante() {
    // Issue #99 : « la régulation s'active et se coupe par canal ».
    //
    // Le cache est donc **par canal**, et non un « dernier état global » : un canal qu'on vient
    // d'activer n'a jamais rien reçu, il doit être écrit au tour suivant même si la température n'a
    // pas bougé d'un millidegré. Une implémentation à cache global le laisserait à 25 % jusqu'au
    // prochain changement de palier — ce qui, un jour de température stable, veut dire jamais.
    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut enregistreur = Enregistreur::neuf();

    assert_eq!(
        enregistreur.tour(&mut regulation, Some(TIEDE)),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE)]),
        "un seul canal régulé au départ"
    );

    regulation.activer(CANAL_2);
    assert_eq!(
        regulation.canaux(),
        vec![CANAL_1.to_owned(), CANAL_2.to_owned()],
        "les deux canaux sont désormais régulés, et `canaux` les rend triés"
    );

    assert_eq!(
        enregistreur.tour(&mut regulation, Some(TIEDE)),
        ecritures(&[(CANAL_2, CONSIGNE_TIEDE)]),
        "le canal tout juste activé reçoit la consigne courante ; celui qui l'avait déjà ne \
         reçoit rien"
    );

    let faites = enregistreur.tour(&mut regulation, Some(TIEDE));
    assert!(
        faites.is_empty(),
        "et au tour d'après, plus rien ne bouge : {faites:?}"
    );
}

#[test]
fn couper_un_canal_l_arrete_sans_toucher_aux_autres() {
    // L'autre moitié de « s'active et se coupe par canal ». Couper, c'est rendre le canal à
    // l'utilisateur : `reverb fan` y écrira ce qu'il veut, et la régulation ne doit plus jamais s'y
    // manifester — surtout pas au prochain changement de palier.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    let mut enregistreur = Enregistreur::neuf();

    enregistreur.tour(&mut regulation, Some(TIEDE));
    regulation.couper(CANAL_1);
    assert_eq!(
        regulation.canaux(),
        vec![CANAL_2.to_owned()],
        "un canal coupé sort de la liste des canaux régulés"
    );

    let mut apres_la_coupure = Enregistreur::neuf();
    for temperature in (degres(20)..=degres(60)).step_by(500) {
        apres_la_coupure.tour(&mut regulation, Some(temperature));
    }
    apres_la_coupure.tour(&mut regulation, None);

    assert!(
        apres_la_coupure.consignes_de(CANAL_1).is_empty(),
        "« {CANAL_1} » est coupé : ni la courbe ni le repli ne doivent plus rien lui écrire, or il \
         a reçu {:?}",
        apres_la_coupure.consignes_de(CANAL_1)
    );
    assert!(
        !apres_la_coupure.consignes_de(CANAL_2).is_empty(),
        "« {CANAL_2} » est resté régulé : il doit continuer de recevoir la courbe"
    );
}

#[test]
fn un_canal_coupe_puis_repris_est_reecrit() {
    // Ce que le contrat laisse ouvert, et que ce fichier tranche : un canal coupé **oublie** ce
    // qu'il avait reçu.
    //
    // La raison n'est pas une préférence de style. Entre la coupure et la reprise, le canal
    // appartient à l'utilisateur : un `reverb fan --channel … --pwm 80` a pu passer par là, et rien
    // ne le dit à la régulation — le noyau ne prévient personne. Garder le cache, ce serait
    // réguler un canal en croyant savoir où il en est.
    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut enregistreur = Enregistreur::neuf();

    assert_eq!(
        enregistreur.tour(&mut regulation, Some(TIEDE)),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE)])
    );

    regulation.couper(CANAL_1);
    let pendant = enregistreur.tour(&mut regulation, Some(TIEDE));
    assert!(
        pendant.is_empty(),
        "canal coupé : rien ne part, or {pendant:?}"
    );

    regulation.activer(CANAL_1);
    assert_eq!(
        enregistreur.tour(&mut regulation, Some(TIEDE)),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE)]),
        "repris à la même température, le canal est réécrit : la régulation ne sait pas ce qui lui \
         est arrivé pendant qu'elle ne le tenait plus"
    );
}

#[test]
fn aucun_canal_regule_ne_produit_jamais_la_moindre_ecriture() {
    // Critère d'acceptation : « la régulation n'ajoute aucun réveil quand aucun canal n'est
    // régulé ».
    //
    // ⚠️ L'absence de `timer` ne s'observe pas depuis un module pur — c'est une propriété de la
    // boucle du démon. Ce qui s'observe, et qui en est la condition, c'est qu'une régulation vide
    // ne produise **rien** : ni écriture, ni repli, ni « juste une fois pour initialiser ». Le
    // démon « doit rester au repos absolu quand rien ne l'occupe ».
    let mut regulation = Regulation::nouvelle(Courbe::defaut());
    let mut enregistreur = Enregistreur::neuf();

    assert!(
        regulation.canaux().is_empty(),
        "une régulation neuve ne régule rien"
    );

    for temperature in (degres(-50)..=degres(120)).step_by(311) {
        enregistreur.tour(&mut regulation, Some(temperature));
        enregistreur.tour(&mut regulation, None);
    }
    enregistreur.tour(&mut regulation, Some(i32::MAX));
    enregistreur.tour(&mut regulation, Some(i32::MIN));

    assert!(
        enregistreur.tout().is_empty(),
        "sans canal régulé, aucune écriture ne doit jamais partir, or {:?}",
        enregistreur.tout()
    );
}

#[test]
fn tous_les_canaux_regules_recoivent_la_meme_consigne() {
    // Hors scope de l'issue : « une courbe par canal — une seule courbe pour les trois, tant que la
    // répartition physique des ventilateurs par canal reste l'inconnue documentée de
    // `docs/VENTILATEURS.md` ».
    //
    // Le corollaire testable est celui-ci : à tout instant, les trois canaux régulés portent la
    // même consigne. Une implémentation qui les décalerait — un canal par tour, par exemple, pour
    // étaler le bus — passerait tous les tests de valeur et laisserait le boîtier désaccordé.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2, CANAL_3]);
    let mut enregistreur = Enregistreur::neuf();
    let defaut = Courbe::defaut();

    let mut derniere: Option<u8> = None;
    for temperature in (degres(30)..=degres(55)).step_by(1000) {
        let attendue = defaut.consigne(temperature);
        let faites = enregistreur.tour(&mut regulation, Some(temperature));

        if derniere == Some(attendue) {
            assert!(faites.is_empty(), "consigne inchangée : {faites:?}");
        } else {
            assert_eq!(
                faites,
                ecritures(&[
                    (CANAL_1, attendue),
                    (CANAL_2, attendue),
                    (CANAL_3, attendue)
                ]),
                "à {temperature} m°C, les trois canaux doivent partir ensemble et à la même valeur"
            );
            derniere = Some(attendue);
        }
    }

    // Et sur toute la durée, aucun canal n'a reçu une consigne que les autres n'ont pas eue.
    let consignes_1 = enregistreur.consignes_de(CANAL_1);
    for canal in [CANAL_2, CANAL_3] {
        assert_eq!(
            enregistreur.consignes_de(canal),
            consignes_1,
            "« {canal} » n'a pas reçu la même suite de consignes que « {CANAL_1} »"
        );
    }
    assert!(
        consignes_1.len() >= 3,
        "le balayage doit avoir produit plusieurs consignes, sinon l'égalité est triviale : {consignes_1:?}"
    );
}

// ---------------------------------------------------------------------------
// C — l'état : relu au démarrage, refusé en nommant
// ---------------------------------------------------------------------------

#[test]
fn l_etat_vit_dans_var_lib_et_non_dans_etc() {
    // Issue #99 : « l'état va dans `/var/lib/reverb/` : c'est de l'état de service réécrit à chaque
    // commande, pas une donnée de montage. »
    //
    // La distinction n'est pas cosmétique : `/etc/reverb/geometrie.conf` a coûté un relevé au sol,
    // ventilateur par ventilateur, et `tools/desinstalle.sh` le préserve exprès. Y ranger une
    // consigne réécrite à chaque commande ferait sauvegarder l'un avec l'autre.
    assert!(
        CHEMIN_REGULATION.starts_with("/var/lib/reverb/"),
        "l'état de la régulation vit sous `/var/lib/reverb/`, obtenu « {CHEMIN_REGULATION} »"
    );
    assert!(
        !CHEMIN_REGULATION.starts_with("/etc"),
        "l'état de la régulation n'est pas une donnée de montage, obtenu « {CHEMIN_REGULATION} »"
    );
}

#[test]
fn l_etat_traverse_l_aller_retour() {
    // Critère d'acceptation : « l'état — quels canaux sont régulés, et la courbe — est relu au
    // démarrage ». L'aller-retour par le texte en est le seul mécanisme.
    //
    // La courbe témoin diffère de la courbe par défaut sur ses trois paliers : un encodeur qui
    // oublierait de l'écrire serait rattrapé par le défaut au décodage, et l'aller-retour passerait
    // sans rien prouver.
    let mienne = courbe(&[(degres(32), 25), (degres(48), 70), (degres(52), 100)]);
    assert_ne!(
        mienne.paliers(),
        Courbe::defaut().paliers(),
        "la courbe témoin doit différer de celle par défaut, sinon ce test ne dit rien"
    );

    let mut avant = Regulation::nouvelle(mienne);
    avant.activer(CANAL_2);
    avant.activer(CANAL_1);

    let apres = decoder(&avant.encoder());
    assert_eq!(
        apres.courbe().paliers(),
        avant.courbe().paliers(),
        "la courbe doit traverser l'aller-retour telle quelle"
    );
    assert_eq!(
        apres.canaux(),
        avant.canaux(),
        "les canaux régulés doivent traverser l'aller-retour, et dans un ordre stable"
    );

    // Une régulation sans aucun canal se relit aussi : c'est l'état d'après un `couper` sur le
    // dernier canal, et il ne doit pas se confondre avec un fichier absent.
    let vide = Regulation::nouvelle(Courbe::defaut());
    let relue = decoder(&vide.encoder());
    assert!(
        relue.canaux().is_empty(),
        "un état sans canal régulé se relit sans canal régulé, obtenu {:?}",
        relue.canaux()
    );
}

#[test]
fn le_format_documente_en_tete_de_fichier_se_relit() {
    // Un fichier d'état se lit et se corrige à la main quand quelque chose cloche. Ce test fixe la
    // forme des deux entrées, une fois — tous les autres tests de refus travaillent sur ce que
    // l'encodeur produit, pour ne pas figer sa mise en forme deux fois.
    let texte = "\
# Régulation côté hôte — Reverb (issue #99)
courbe 35000:30 45000:60 50000:100
canal nzxtsmart2:fan-1
canal nzxtsmart2:fan-2
";
    let regulation = decoder(texte);
    assert_eq!(
        regulation.courbe().paliers(),
        Courbe::defaut().paliers(),
        "la ligne `courbe` porte les paliers en millidegrés, comme `hwmon` les rend"
    );
    assert_eq!(
        regulation.canaux(),
        vec![CANAL_1.to_owned(), CANAL_2.to_owned()],
        "une ligne `canal` par canal régulé, sous son slug de protocole"
    );

    // L'ordre des lignes ne veut rien dire, et les blancs non plus.
    let melange = "\
canal nzxtsmart2:fan-2

# un commentaire au milieu
courbe 35000:30 45000:60 50000:100
   canal nzxtsmart2:fan-1
";
    let relue = decoder(melange);
    assert_eq!(relue.canaux(), regulation.canaux());
    assert_eq!(relue.courbe().paliers(), regulation.courbe().paliers());
}

#[test]
fn un_fichier_absent_laisse_le_demon_demarrer_sans_regulation() {
    // Test d'intention n° 10 de l'issue : « un fichier d'état absent laisse le démon démarrer sans
    // régulation ».
    //
    // C'est le premier démarrage, pas une anomalie : rien à signaler. Un message ici polluerait le
    // journal de toute installation neuve.
    let dossier = DossierJetable::neuf("fichier_absent");
    let chemin = dossier.fichier("regulation.conf");

    let (regulation, signalement) = charger_regulation(&chemin);
    assert!(
        regulation.canaux().is_empty(),
        "sans fichier, aucun canal n'est régulé : la régulation ne s'invite pas sur un canal que \
         personne ne lui a confié. Obtenu {:?}",
        regulation.canaux()
    );
    assert_eq!(
        regulation.courbe().paliers(),
        Courbe::defaut().paliers(),
        "et la courbe est celle par défaut, prête à servir dès qu'un canal sera activé"
    );
    assert_eq!(
        signalement, None,
        "un premier démarrage n'est pas une anomalie. Obtenu : {signalement:?}"
    );
}

#[test]
fn lire_un_fichier_absent_ne_le_cree_pas() {
    // Lire, c'est lire. Un démarrage qui écrirait l'état par défaut en passant ferait de la
    // première lecture un état posé, et rendrait indistinguables « jamais réglé » et « réglé ».
    let dossier = DossierJetable::neuf("lecture_sans_ecriture");
    let chemin = dossier.fichier("regulation.conf");

    let _ = charger_regulation(&chemin);
    assert!(
        !chemin.exists(),
        "charger une régulation absente ne doit rien écrire : {} a été créé",
        chemin.display()
    );
}

#[test]
fn une_entree_manquante_est_refusee_en_la_nommant() {
    // Critère d'acceptation : « un fichier d'état absent, tronqué ou répété est refusé **en le
    // nommant** ». Test d'intention n° 9 : « un fichier d'état tronqué est refusé en nommant
    // l'entrée fautive ».
    //
    // Une écriture coupée par une panne de courant laisse exactement ça : un fichier sans sa ligne
    // `courbe`. Compléter au jugé — par la courbe par défaut — rendrait une régulation plausible et
    // fausse, sans un mot dans le journal.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    regulation.activer(CANAL_3);
    let texte = regulation.encoder();

    let ligne_courbe = lignes_du_genre(&texte, "courbe")
        .into_iter()
        .next()
        .expect("l'encodeur écrit une ligne `courbe`");
    let erreur = refus_de_fichier(&sans_la_ligne(&texte, &ligne_courbe));
    assert!(
        erreur.raison.contains("courbe"),
        "le refus doit nommer l'entrée manquante — la courbe. Raison obtenue : {}",
        erreur.raison
    );
    assert_eq!(
        erreur.ligne, 0,
        "une entrée absente n'est écrite nulle part : le numéro de ligne doit valoir 0 plutôt que \
         de pointer une ligne innocente"
    );

    // Une ligne coupée en plein milieu : un palier sans sa consigne. Elle, on peut la pointer.
    for tronquee in [
        "courbe 35000:30 45000:60 50000:",
        "courbe 35000:30 45000",
        "courbe",
    ] {
        let erreur = refus_de_fichier(&avec_la_ligne(
            &sans_la_ligne(&texte, &ligne_courbe),
            tronquee,
        ));
        assert!(
            erreur.ligne >= 1,
            "une ligne écrite quelque part doit être pointée, pas 0. Ligne fautive : « {tronquee} »"
        );
        assert!(
            !erreur.raison.trim().is_empty(),
            "le refus de « {tronquee} » doit dire ce qui cloche"
        );
    }

    // Une ligne `canal` sans son canal.
    let erreur = refus_de_fichier(&avec_la_ligne(&texte, "canal"));
    assert!(
        erreur.raison.contains("canal"),
        "le refus doit nommer l'entrée fautive. Raison obtenue : {}",
        erreur.raison
    );
}

#[test]
fn une_entree_repetee_est_refusee_en_la_nommant() {
    // Même critère, autre moitié : « … ou répété est refusé en le nommant ».
    //
    // Deux lignes `courbe` contradictoires, et c'est l'ordre de lecture qui déciderait de la
    // vitesse des ventilateurs au démarrage. Le fichier ne dit plus ce que la machine fait.
    let regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    let texte = regulation.encoder();

    let seconde_courbe = "courbe 30000:100";
    let erreur = refus_de_fichier(&avec_la_ligne(&texte, seconde_courbe));
    assert!(
        erreur.raison.contains("courbe"),
        "le refus doit dire que c'est la courbe qui est en double. Raison obtenue : {}",
        erreur.raison
    );
    assert!(
        erreur.ligne >= 1,
        "un doublon est écrit quelque part : le refus doit pointer une ligne, pas 0"
    );

    // Et un canal répété. Le doublon est ici inoffensif à l'exécution — le même canal deux fois,
    // c'est le même canal — mais le critère est explicite, et un fichier qui se répète est un
    // fichier qu'on a mal réécrit : le dire vaut mieux que le rattraper en silence.
    for ligne in lignes_du_genre(&texte, "canal") {
        let erreur = refus_de_fichier(&avec_la_ligne(&texte, &ligne));
        let canal = ligne
            .split_whitespace()
            .nth(1)
            .expect("une ligne `canal` porte son canal en deuxième jeton");
        assert!(
            erreur.raison.contains(canal),
            "le refus doit nommer le canal répété « {canal} ». Raison obtenue : {}",
            erreur.raison
        );
    }
}

#[test]
fn un_fichier_de_travers_n_empeche_jamais_le_demarrage() {
    // Critère d'acceptation : « … et n'empêche pas le démarrage ».
    //
    // `charger_regulation` ne peut pas rendre d'erreur — sa signature l'interdit — donc ce qui est
    // vérifié ici est qu'aucune de ces entrées ne le fait paniquer, que toutes rendent un état
    // utilisable, et qu'aucune ne passe sous silence.
    //
    // ⚠️ L'état rendu est « aucune régulation », et non le repli : on ne sait pas quels canaux
    // réguler. Poser 50 % sur des canaux qu'on n'a pas su relire, ce serait décider à la place de
    // l'utilisateur sur un bus qu'on ne sait plus cartographier.
    let dossier = DossierJetable::neuf("fichier_de_travers");
    let valide = regulation_sur(&[CANAL_1]).encoder();

    let cas: Vec<(&str, String)> = vec![
        ("du texte au hasard", "n'importe quoi\n".to_owned()),
        ("que des commentaires", "# rien d'autre\n#\n".to_owned()),
        (
            "coupé en plein milieu",
            valide.chars().take(valide.len() / 2).collect(),
        ),
        (
            "un fichier d'une autre nature",
            "bas-gauche 0 horaire\n".to_owned(),
        ),
        ("une courbe sans palier", "courbe\n".to_owned()),
        (
            "une courbe qui promet 150 %",
            "courbe 35000:30 45000:150\n".to_owned(),
        ),
        (
            "des paliers dans le désordre",
            "courbe 50000:100 35000:30\n".to_owned(),
        ),
        ("une seule accolade", "{".to_owned()),
    ];

    for (nom, contenu) in cas {
        let chemin = dossier.fichier(&format!("{}.conf", nom.replace(' ', "-")));
        ecrire_fichier(&chemin, &contenu);

        let (regulation, signalement) = charger_regulation(&chemin);
        assert!(
            regulation.canaux().is_empty(),
            "cas « {nom} » : un fichier de travers ne régule rien, obtenu {:?}",
            regulation.canaux()
        );
        let message = signalement.unwrap_or_else(|| {
            panic!(
                "cas « {nom} » : un fichier abîmé doit être signalé, sans quoi il se confond avec \
                 un premier démarrage et l'utilisateur ne saura jamais que sa régulation est \
                 tombée"
            )
        });
        assert!(
            message.contains(&nom_de_fichier(&chemin)),
            "cas « {nom} » : le signalement doit nommer le fichier en cause. Obtenu : {message}"
        );
    }

    // Deux formes d'illisible qui n'ont rien à voir avec le texte : un dossier là où le démon
    // attend un fichier, et des octets qui ne sont pas de l'UTF-8 — ce qu'une écriture coupée par
    // une panne de courant laisse derrière elle.
    let en_dossier = dossier.fichier("dossier.conf");
    fs::create_dir(&en_dossier).expect("création du faux fichier");
    let binaire = dossier.fichier("binaire.conf");
    fs::write(&binaire, [0xff_u8, 0xfe, 0x00, 0x80]).expect("écriture d'octets invalides");

    for chemin in [&en_dossier, &binaire] {
        let (regulation, signalement) = charger_regulation(chemin);
        assert!(
            regulation.canaux().is_empty(),
            "un fichier illisible ne régule rien. Chemin : {}",
            chemin.display()
        );
        let message = signalement.unwrap_or_else(|| {
            panic!(
                "un fichier illisible doit être signalé. Chemin : {}",
                chemin.display()
            )
        });
        assert!(
            message.contains(&nom_de_fichier(chemin)),
            "le signalement doit nommer le fichier en cause. Obtenu : {message}"
        );
    }
}

#[test]
fn l_etat_relu_au_demarrage_reprend_la_regulation() {
    // Critère d'acceptation : « la régulation … survit à un redémarrage », et « l'état — quels
    // canaux sont régulés, et la courbe — est relu au démarrage ».
    //
    // Le passage par le disque est le seul mécanisme testable ici ; `systemctl restart` et le
    // redémarrage machine se vérifient sur la machine.
    let dossier = DossierJetable::neuf("relu_au_demarrage");
    let chemin = dossier.fichier("regulation.conf");

    let mienne = courbe(&[(degres(38), 45), (degres(52), 100)]);
    let mut avant = Regulation::nouvelle(mienne);
    avant.activer(CANAL_1);
    avant.activer(CANAL_3);
    enregistrer_regulation(&chemin, &avant).expect("enregistrement de la régulation");

    let (mut apres, signalement) = charger_regulation(&chemin);
    assert_eq!(
        signalement, None,
        "un fichier que le démon vient d'écrire doit se relire sans un mot. Obtenu : {signalement:?}"
    );
    assert_eq!(
        apres.canaux(),
        vec![CANAL_1.to_owned(), CANAL_3.to_owned()],
        "les canaux régulés sont retrouvés"
    );
    assert_eq!(
        apres.courbe().paliers(),
        avant.courbe().paliers(),
        "la courbe réglée est retrouvée"
    );

    // Et elle régule pour de bon : au premier tour d'après le redémarrage, les deux canaux
    // reçoivent la consigne de **leur** courbe — 45 + (45 − 38) × 55/14, encadré parce que la
    // division ne tombe pas juste.
    let mut enregistreur = Enregistreur::neuf();
    let faites = enregistreur.tour(&mut apres, Some(degres(45)));
    let attendue = apres.courbe().consigne(degres(45));
    assert_eq!(
        faites,
        ecritures(&[(CANAL_1, attendue), (CANAL_3, attendue)]),
        "après un redémarrage, la régulation reprend sur les canaux retrouvés"
    );
    assert!(
        attendue > 45,
        "la consigne à 45 °C doit dépasser le premier palier de la courbe réglée, obtenu {attendue} %"
    );
    assert!(
        attendue < 100,
        "la consigne à 45 °C ne doit pas encore être au plafond, obtenu {attendue} %"
    );
    assert_ne!(
        attendue,
        Courbe::defaut().consigne(degres(45)),
        "et elle doit différer de ce que la courbe par défaut aurait donné, sinon rien ne prouve \
         que c'est la courbe relue qu'on suit"
    );
}

#[test]
fn le_cache_d_ecriture_ne_traverse_pas_le_fichier() {
    // ⚠️ « Aucune persistance matérielle » (CLAUDE.md) : rien ne survit au redémarrage, et les
    // canaux `nzxtsmart2` repartent à `pwm = 64`. Une régulation qui relirait « j'avais déjà écrit
    // 33 % » et se tairait laisserait donc les ventilateurs à 25 % jusqu'au prochain changement de
    // palier — c'est-à-dire, un jour de température stable, indéfiniment.
    //
    // Ce que le fichier conserve, c'est l'**intention** — quels canaux, quelle courbe — jamais ce
    // qui a été écrit sur le bus.
    let mut avant = regulation_sur(&[CANAL_1]);
    let mut enregistreur = Enregistreur::neuf();
    assert_eq!(
        enregistreur.tour(&mut avant, Some(TIEDE)),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE)]),
        "avant le redémarrage, le canal a reçu sa consigne"
    );

    let mut apres = decoder(&avant.encoder());
    let mut apres_le_redemarrage = Enregistreur::neuf();
    assert_eq!(
        apres_le_redemarrage.tour(&mut apres, Some(TIEDE)),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE)]),
        "au premier tour après un redémarrage, le canal est réécrit même à température identique : \
         le contrôleur, lui, est reparti de son défaut d'allumage"
    );
}

#[test]
fn le_dossier_parent_est_cree_s_il_manque() {
    // `StateDirectory=reverb` crée `/var/lib/reverb` au démarrage du service, mais l'enregistrement
    // ne doit pas en dépendre : un premier `enregistrer` avant que quoi que ce soit d'autre ait
    // écrit là ne doit pas échouer. C'est déjà le contrat de `enregistrer_eclairage`.
    let dossier = DossierJetable::neuf("dossier_parent");
    let chemin = dossier
        .chemin()
        .join("pas-encore-la")
        .join("regulation.conf");

    let mut regulation = regulation_sur(&[CANAL_1]);
    regulation.activer(CANAL_2);
    enregistrer_regulation(&chemin, &regulation).expect("enregistrement dans un dossier absent");

    let (relue, signalement) = charger_regulation(&chemin);
    assert_eq!(signalement, None);
    assert_eq!(relue.canaux(), regulation.canaux());
}
