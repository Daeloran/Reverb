//! Tests d'intention — issue #74, « Enregistrer une ambiance sous un nom et y revenir ».
//!
//! Écrits **avant** l'implémentation, depuis l'issue #74 seule. Rien de `crates/*/src/` n'a été lu
//! pour les produire, hors les signatures publiques nécessaires à la compilation. Si l'un de ces
//! tests échoue après implémentation, c'est le code qu'on corrige, jamais le test.
//!
//! La validation du nom vit dans `crates/reverb-proto/tests/spec_profils.rs` — elle est pure et
//! n'a besoin de rien du démon. Ce fichier-ci couvre le reste : l'encodage, le disque, et ce qu'un
//! profil demande d'appliquer.
//!
//! ## Le défaut visé : un profil qui rend *presque* l'ambiance
//!
//! Un profil est un instantané. Sa seule promesse est la fidélité, et cette promesse a la
//! particularité de céder **en silence** : une zone perdue à l'enregistrement, une vitesse
//! d'animation reprise à sa valeur par défaut, un ordre de zones qui change d'un
//! enregistrement à l'autre — rien de tout cela ne lève d'erreur. On rappelle « nuit » six mois
//! plus tard, le boîtier est *presque* bon, et personne ne relie jamais cet écart à une ligne de
//! code. D'où la forme de ces tests : ils ne cherchent pas des erreurs, ils comparent un état à
//! celui qu'on avait posé, champ par champ.
//!
//! Le mode de défaillance opposé est déjà arrivé, et il a coûté plus cher : un affichage
//! impossible **persisté** faisait redémarrer le démon dans un état cassé, indéfiniment (#69). Un
//! profil est un second fichier qui décide de ce que la dalle montre ; il hérite donc du même
//! devoir de vérifier avant d'appliquer, et [`un_profil_dont_l_image_a_change_de_format_est_signale`]
//! le lui demande.
//!
//! ## L'API que ces tests supposent — c'est ici qu'elle se décide
//!
//! Rien n'existe encore côté `src/`. Le contrat que l'implémentation doit honorer, au complet :
//!
//! ```ignore
//! // crates/reverb-daemon/src/profils.rs
//! use reverb_proto::NomProfil;                 // validé dans reverb-proto, cf. spec_profils.rs
//! use crate::ecran::Etat as EtatEcran;
//! use crate::persistance::Eclairage;
//! use crate::zones::Zones;
//!
//! /// Le répertoire des profils. Un fichier par profil : `<nom>.conf`.
//! pub const CHEMIN_PROFILS: &str = "/var/lib/reverb/profils";
//!
//! /// Un instantané nommé de l'éclairage complet. Jamais la géométrie.
//! #[derive(Debug, Clone, PartialEq)]
//! pub struct Profil {
//!     pub eclairage: Eclairage,
//!     pub zones: Zones,
//!     /// `None` : le profil ne dit **rien** de l'écran, et le rappeler n'y touche pas.
//!     /// À distinguer de `Some(Etat { affichage: Affichage::Rien, .. })`, qui est la
//!     /// consigne « rends la dalle au firmware ».
//!     pub ecran: Option<EtatEcran>,
//! }
//!
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub struct ProfilInvalide {
//!     /// Numéro de ligne, à partir de 1, comme un éditeur. 0 si la faute n'a pas de ligne.
//!     pub ligne: usize,
//!     pub raison: String,
//! }
//!
//! /// Ce que `enregistrer` a fait, pour que le démon puisse le dire.
//! #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//! pub enum Ecriture { Creee, Ecrasee }
//!
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub enum ProfilRefuse {
//!     Absent(NomProfil),
//!     Illisible(NomProfil, ProfilInvalide),
//! }
//!
//! /// Ce qu'un profil demande d'appliquer, une fois le disque consulté.
//! #[derive(Debug, Clone, PartialEq)]
//! pub struct Application {
//!     pub eclairage: Eclairage,
//!     pub zones: Zones,
//!     /// `None` si le profil ne disait rien, **ou** si ce qu'il désignait n'est plus affichable.
//!     pub ecran: Option<EtatEcran>,
//!     /// Vide quand tout s'applique. Non vide, l'éclairage et les zones s'appliquent quand même.
//!     pub signalements: Vec<String>,
//! }
//!
//! impl Profil {
//!     pub fn encoder(&self) -> String;
//!     pub fn decoder(texte: &str) -> Result<Profil, ProfilInvalide>;
//!     /// Consulte le disque pour ce que l'écran désigne, et rien d'autre. N'écrit nulle part.
//!     pub fn preparer(&self) -> Application;
//! }
//!
//! pub fn enregistrer(repertoire: &Path, nom: &NomProfil, profil: &Profil) -> io::Result<Ecriture>;
//! pub fn charger(repertoire: &Path, nom: &NomProfil) -> Result<Profil, ProfilRefuse>;
//! /// Les noms connus, **triés**, sans décoder aucun fichier — un profil abîmé reste listé.
//! pub fn lister(repertoire: &Path) -> Vec<NomProfil>;
//! pub fn oublier(repertoire: &Path, nom: &NomProfil) -> Result<(), ProfilRefuse>;
//! ```
//!
//! `ProfilInvalide` et `ProfilRefuse` implémentent `Display` et `std::error::Error`, comme
//! `ZonesInvalides` et `EclairageInvalide`.
//!
//! ## Cinq points que l'issue laisse ouverts, et que ces tests tranchent
//!
//! 1. **Le format n'est pas figé ici.** L'issue impose « format texte, une ligne par entrée,
//!    cohérent avec l'existant », et rien de plus. Ces tests ne citent donc **aucune ligne
//!    littérale** : ils repèrent une ligne à un jeton distinctif — le slug d'un ventilateur, le nom
//!    d'une zone, le nom d'une animation — et la manipulent. Un test qui écrirait
//!    `eclairage ventilateur bas-gauche 10800f` verrouillerait une mise en page que l'issue laisse
//!    libre, et casserait à la première réorganisation sans que rien ne soit cassé.
//! 2. **Une entrée répétée est refusée, y compris là où `zones.conf` la tolère.** Le test
//!    d'intention n° 8 dit « comme `eclairage.conf` », et `eclairage.conf` refuse en nommant
//!    (README, « L'éclairage retrouvé »). C'est donc `eclairage.conf` qui est la référence, pour
//!    **tout** le fichier de profil. Ce que ça exige de l'implémentation : une détection de doublon
//!    au niveau du profil, en plus de ce que font les décodeurs existants. Ce que ça achète : un
//!    fichier tronqué ou concaténé reste détectable, au lieu d'être complété au jugé par une
//!    ambiance plausible et fausse.
//! 3. **Une entrée qu'on peut légitimement omettre n'est pas une troncature.** Le contraire de la
//!    règle précédente, et il faut les deux : une couche globale sans animation est un éclairage
//!    fixe, une zone sans rendu est transparente, un profil sans section écran ne dit rien de
//!    l'écran. Retirer l'un des dix ventilateurs ou l'une des quatre barrettes, en revanche, laisse
//!    une couleur inventée — c'est là que le refus est exigé.
//! 4. **`preparer` consulte le disque pour l'écran seul.** L'issue veut qu'« un profil dont l'image
//!    a disparu applique l'éclairage et les zones, et signale l'écran sans échouer ». La décision
//!    est pure une fois qu'on sait si le fichier est là ; la rendre observable demande un point de
//!    passage entre « charger » et « écrire sur le matériel », et c'est ce que `preparer` est. Rien
//!    d'autre du profil ne dépend du disque.
//! 5. **L'ordre des zones est celui de leur création, et le profil le respecte.** `Zones::liste`
//!    est ordonnée (#29). Le critère « deux fichiers identiques octet pour octet — l'ordre des
//!    zones ne dépend pas de l'ordre d'un `HashMap` » interdit donc d'introduire une table de
//!    hachage dans le profil, pas de renormaliser un ordre qui existe déjà.
//!
//! ## Ce que ces tests ne couvrent pas, et pourquoi
//!
//! - Le verbe `profil` sur le socket, et le fait que `profil list` n'écrive sur aucun bus : le
//!   socket vit dans `serveur.rs` et les bus dans `peripheriques.rs`. Ce qui en est vérifiable ici
//!   est que `lister` ne touche pas même à son propre répertoire
//!   ([`lister_ne_touche_a_rien`]), et qu'il ne décode aucun fichier.
//! - `systemctl restart reverbd` : ce qui en est testable est l'aller-retour par le disque, qui en
//!   est le seul mécanisme.
//! - « Une seconde installation n'écrase pas ceux que l'utilisateur a modifiés » : c'est
//!   `tools/installe.sh`, du shell. Ce qui en est vérifiable ici est la moitié Rust — le dépôt
//!   livre bien des exemples valides ([`le_depot_livre_des_profils_d_exemple`]) — et le moyen que
//!   le script a de tenir sa promesse, à savoir que `enregistrer` distingue `Creee` d'`Ecrasee`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use reverb_anim::{Animation, CATALOGUE, Direction, Geometrie, Reglages};
use reverb_daemon::ecran::{Affichage, Etat as EtatEcran};
use reverb_daemon::persistance::{CHEMIN_ECLAIRAGE, CHEMIN_GEOMETRIE, Eclairage};
use reverb_daemon::profils::{
    Application, CHEMIN_PROFILS, Ecriture, Profil, ProfilInvalide, ProfilRefuse, charger,
    enregistrer, lister, oublier,
};
use reverb_daemon::zones::{CHEMIN_ZONES, Rendu, Zones};
use reverb_proto::ram::SLOT_COUNT;
use reverb_proto::{Led, NomProfil, Position, Rgb};

// ---------------------------------------------------------------------------
// Vecteurs témoins
// ---------------------------------------------------------------------------

/// La couleur témoin du ventilateur de rang `index`.
///
/// Trois propriétés, chacune pour un mode de défaillance précis :
/// - **toutes distinctes** entre elles, sinon recopier la couleur du premier ventilateur sur les
///   dix passerait inaperçu ;
/// - **`r` différent de `b`**, sinon une permutation des composantes traverserait l'aller-retour
///   sans un seul message — et le boîtier changerait de couleur ;
/// - **hors de la famille des barrettes**, sinon une couleur de ventilateur lue comme couleur de
///   barrette se confondrait avec la bonne.
fn couleur_ventilateur(index: usize) -> Rgb {
    let graine = u8::try_from(index).expect("dix ventilateurs tiennent dans un u8");
    Rgb::new(0x11 + graine, 0x82, 0xf0 - graine)
}

/// La couleur témoin de la barrette de rang `slot`, dans une famille distincte.
fn couleur_barrette(slot: usize) -> Rgb {
    let graine = u8::try_from(slot).expect("quatre barrettes tiennent dans un u8");
    Rgb::new(0xa4 + graine, 0x37, 0x05 + graine)
}

/// Les réglages témoins de la couche globale : les trois champs différents de leurs valeurs par
/// défaut (`ff40ff`, vitesse 3, `horaire`). Si l'un coïncidait avec son défaut, un encodage qui le
/// perdrait serait rattrapé au décodage et l'aller-retour passerait quand même.
fn reglages_globaux() -> Reglages {
    Reglages {
        couleur: Rgb::new(0x00, 0xff, 0x00),
        vitesse: 7,
        direction: Direction::HautBas,
    }
}

/// Les réglages témoins d'une zone, **distincts** de ceux de la couche globale sur les trois
/// champs : une implémentation qui recopierait les réglages globaux dans la zone passerait
/// autrement sans un mot.
fn reglages_de_zone() -> Reglages {
    Reglages {
        couleur: Rgb::new(0x12, 0x34, 0x56),
        vitesse: 9,
        direction: Direction::ArriereAvant,
    }
}

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
}

fn leds(slug: &str) -> Vec<Led> {
    Led::depuis_slug(slug).unwrap_or_else(|erreur| panic!("« {slug} » désigne des LED : {erreur}"))
}

/// Un éclairage où chaque ventilateur et chaque barrette porte sa propre couleur, sous une
/// animation en cours avec ses réglages.
fn eclairage_temoin() -> Eclairage {
    let mut ventilateurs = [Rgb::BLACK; 10];
    for (index, couleur) in ventilateurs.iter_mut().enumerate() {
        *couleur = couleur_ventilateur(index);
    }
    let mut barrettes = [Rgb::BLACK; SLOT_COUNT];
    for (slot, couleur) in barrettes.iter_mut().enumerate() {
        *couleur = couleur_barrette(slot);
    }
    Eclairage {
        ventilateurs,
        barrettes,
        animation: Some((animation("vague"), reglages_globaux())),
    }
}

/// La couleur de la zone fixe. Hors de portée de [`couleur_ventilateur`] et de
/// [`couleur_barrette`] sur le vert : aucune couleur de fond ne peut s'y confondre par hasard.
const ROUGE_DE_ZONE: Rgb = Rgb::new(0xfe, 0xdc, 0xba);

/// Trois zones, une par rendu possible : fixe, animée, transparente.
///
/// Les trois sont nécessaires. Une seule zone fixe laisserait passer une implémentation qui
/// n'écrit jamais l'animation d'une zone ; sans zone transparente, on ne verrait pas celle qui
/// invente un rendu à une zone qui n'en a pas.
fn zones_temoins() -> Zones {
    let mut zones = Zones::vide();
    zones.poser("colonne", &leds("fan:arriere"));
    zones.eclairer("colonne", ROUGE_DE_ZONE);
    zones.poser("barre", &leds("slot:1"));
    zones.animer("barre", Some((animation("braise"), reglages_de_zone())));
    zones.poser("tranche", &leds("fan:haut-milieu"));
    zones
}

/// Le chemin d'image du témoin. Il n'a pas à exister : ce qui est enregistré est **le chemin**,
/// jamais les pixels (issue, « l'écran : sa luminosité et **le chemin** de ce qu'il affiche »).
const IMAGE_TEMOIN: &str = "/home/nico/images/nuit d'été.png";

fn ecran_temoin() -> EtatEcran {
    EtatEcran {
        luminosite: 37,
        affichage: Affichage::Image(IMAGE_TEMOIN.to_owned()),
    }
}

fn profil_temoin() -> Profil {
    Profil {
        eclairage: eclairage_temoin(),
        zones: zones_temoins(),
        ecran: Some(ecran_temoin()),
    }
}

/// Un second profil, différent du premier sur les trois natures à la fois. Sert à vérifier qu'un
/// enregistrement en écrase un autre **entièrement**.
fn autre_profil() -> Profil {
    let mut zones = Zones::vide();
    zones.poser("bandeau", &leds("fan:bas-gauche"));
    zones.eclairer("bandeau", Rgb::new(0x01, 0x02, 0x03));
    Profil {
        eclairage: Eclairage {
            ventilateurs: [Rgb::new(0x7f, 0x11, 0x22); 10],
            barrettes: [Rgb::new(0x33, 0x7f, 0x44); SLOT_COUNT],
            animation: None,
        },
        zones,
        ecran: Some(EtatEcran {
            luminosite: 100,
            affichage: Affichage::Cadran("kraken2023elite:coolant-temp".to_owned()),
        }),
    }
}

// ---------------------------------------------------------------------------
// Manipulation de texte — aucun test ne cite une ligne littérale du format
// ---------------------------------------------------------------------------

fn lignes(texte: &str) -> Vec<String> {
    texte.lines().map(str::to_owned).collect()
}

fn recoller(lignes: &[String]) -> String {
    let mut texte = lignes.join("\n");
    texte.push('\n');
    texte
}

/// Une ligne d'entrée : ni vide, ni commentaire. Les commentaires sont répétables et omissibles,
/// les entrées ne le sont pas — c'est toute la différence que ces tests exploitent.
fn est_une_entree(ligne: &str) -> bool {
    let taille = ligne.trim();
    !taille.is_empty() && !taille.starts_with('#')
}

/// Les rangs (0-based) des lignes d'entrée qui portent `jeton` comme mot entier.
fn rangs_portant(texte: &str, jeton: &str) -> Vec<usize> {
    texte
        .lines()
        .enumerate()
        .filter(|(_, ligne)| est_une_entree(ligne))
        .filter(|(_, ligne)| ligne.split_whitespace().any(|mot| mot == jeton))
        .map(|(rang, _)| rang)
        .collect()
}

/// Le rang de l'unique ligne d'entrée portant `jeton`. Panique s'il y en a zéro ou plusieurs :
/// un témoin qui ne rend pas ce jeton unique ne prouve rien.
fn rang_unique(texte: &str, jeton: &str) -> usize {
    let rangs = rangs_portant(texte, jeton);
    assert_eq!(
        rangs.len(),
        1,
        "le témoin doit porter « {jeton} » sur exactement une ligne d'entrée, sinon la \
         manipulation vise à l'aveugle. Rangs trouvés : {rangs:?}\nTexte :\n{texte}"
    );
    rangs[0]
}

fn refus(texte: &str) -> ProfilInvalide {
    match Profil::decoder(texte) {
        Ok(profil) => panic!(
            "Ce texte devait être refusé, il a été accepté et a rendu {profil:?}.\nTexte :\n{texte}"
        ),
        Err(erreur) => erreur,
    }
}

fn aller_retour(profil: &Profil) -> Profil {
    let texte = profil.encoder();
    Profil::decoder(&texte).unwrap_or_else(|erreur| {
        panic!(
            "Un profil produit par `encoder` doit se relire par `decoder`.\n  Refus : {erreur}\n  \
             Texte produit :\n{texte}"
        )
    })
}

// ---------------------------------------------------------------------------
// Dossier temporaire — sans dépendance, c'est un garde-fou du projet
// ---------------------------------------------------------------------------

/// Un dossier de travail sous `std::env::temp_dir()`, effacé à la fin du test.
///
/// L'effacement passe par `Drop` et non par une ligne en fin de test : un test qui échoue part en
/// `panic!`, et une fin de test jamais atteinte laisserait le dossier derrière elle.
struct DossierJetable {
    chemin: PathBuf,
}

impl DossierJetable {
    /// `nom` doit être le nom du test : c'est ce qui rend le chemin unique entre tests, `cargo` les
    /// exécutant en parallèle dans un même processus.
    fn neuf(nom: &str) -> DossierJetable {
        let chemin =
            std::env::temp_dir().join(format!("reverb-spec-profils-{}-{nom}", std::process::id()));
        let _ = fs::remove_dir_all(&chemin);
        fs::create_dir_all(&chemin)
            .unwrap_or_else(|erreur| panic!("dossier de test {} : {erreur}", chemin.display()));
        DossierJetable { chemin }
    }

    fn chemin(&self) -> &Path {
        &self.chemin
    }

    fn fichier(&self, nom: &str) -> PathBuf {
        self.chemin.join(nom)
    }

    /// Le contenu du dossier, nom de fichier → octets. Sert à prouver qu'une lecture n'a rien
    /// changé, et qu'aucun fichier temporaire ne traîne.
    fn contenu(&self) -> BTreeMap<String, Vec<u8>> {
        let mut vu = BTreeMap::new();
        for entree in fs::read_dir(&self.chemin).expect("le dossier de test est lisible") {
            let entree = entree.expect("entrée lisible");
            vu.insert(
                entree.file_name().to_string_lossy().into_owned(),
                fs::read(entree.path()).unwrap_or_default(),
            );
        }
        vu
    }
}

impl Drop for DossierJetable {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.chemin);
    }
}

fn nom(saisi: &str) -> NomProfil {
    NomProfil::nouveau(saisi).unwrap_or_else(|erreur| panic!("« {saisi} » est un nom : {erreur}"))
}

/// Un PNG valide de 1 × 1 pixel, RGBA — soixante-dix octets. Sert à poser sur le disque une image que
/// le démon sait vraiment décoder, sans dépendre d'un fichier de l'environnement.
const PNG_1X1: [u8; 70] = [
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 252, 207, 192, 80, 15, 0, 4,
    133, 1, 128, 132, 169, 140, 33, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

// ---------------------------------------------------------------------------
// 1 — l'aller-retour
// ---------------------------------------------------------------------------

#[test]
fn un_profil_traverse_l_aller_retour_sans_rien_perdre() {
    // Test d'intention n° 1 de l'issue : « Un profil encodé puis décodé rend exactement l'état de
    // départ, animations et réglages compris ». Critère d'acceptation : « restitue la couche
    // globale, toutes les zones et l'écran tels qu'ils étaient ».
    //
    // Ce test compare champ par champ plutôt qu'en bloc : `assert_eq!(pose, relu)` seul dirait
    // « ça diffère » sur une structure de cent lignes, et laisserait chercher où.
    let pose = profil_temoin();
    let relu = aller_retour(&pose);

    for position in Position::ALL {
        let rang = position.index();
        assert_eq!(
            relu.eclairage.ventilateurs[rang],
            pose.eclairage.ventilateurs[rang],
            "le ventilateur « {} » doit retrouver sa propre couleur, pas celle d'un voisin",
            position.slug()
        );
    }
    for slot in 0..SLOT_COUNT {
        assert_eq!(
            relu.eclairage.barrettes[slot], pose.eclairage.barrettes[slot],
            "la barrette {slot} doit retrouver sa propre couleur : son rang est la seule chose qui \
             la distingue de ses voisines"
        );
    }

    let (anim_pose, reglages_poses) = pose
        .eclairage
        .animation
        .expect("le témoin porte une animation globale");
    let (anim_relue, reglages_relus) = relu
        .eclairage
        .animation
        .expect("une animation en cours doit encore être en cours après l'aller-retour");
    assert_eq!(
        anim_relue, anim_pose,
        "l'animation globale doit être la même"
    );
    assert_eq!(
        reglages_relus, reglages_poses,
        "couleur, vitesse et direction de l'animation globale doivent survivre : les reprendre à \
         leurs valeurs par défaut serait un réglage perdu que rien ne signalerait"
    );

    assert_eq!(
        relu.zones.liste().len(),
        pose.zones.liste().len(),
        "toutes les zones doivent revenir, et pas une de plus"
    );
    for (relue, posee) in relu.zones.liste().iter().zip(pose.zones.liste()) {
        assert_eq!(
            relue.nom, posee.nom,
            "les zones doivent revenir dans l'ordre de leur création"
        );
        assert_eq!(
            relue.cibles, posee.cibles,
            "la zone « {} » doit retrouver ses cibles, exactement",
            posee.nom
        );
        assert_eq!(
            relue.rendu, posee.rendu,
            "la zone « {} » doit retrouver son rendu — une zone animée qui revient fixe est une \
             ambiance perdue sans un message",
            posee.nom
        );
    }

    assert_eq!(
        relu.ecran, pose.ecran,
        "l'écran doit retrouver sa luminosité et le chemin de ce qu'il affichait"
    );
    assert_eq!(relu, pose, "et donc le profil entier");

    // Ce qui est enregistré est **le chemin**, jamais les pixels. Une image de la dalle pèse
    // 1 228 800 octets (spec KRAKEN-LCD) ; un profil qui les embarquerait se verrait ici.
    let texte = pose.encoder();
    assert!(
        texte.contains(IMAGE_TEMOIN),
        "le chemin de l'image doit figurer tel quel dans le fichier, espaces et accents compris"
    );
    assert!(
        texte.len() < 64 * 1024,
        "un profil est un fichier de configuration, pas un tampon d'image : {} octets",
        texte.len()
    );
    assert!(
        texte.ends_with('\n'),
        "le fichier se termine par un retour à la ligne, comme tout fichier texte"
    );
}

#[test]
fn les_six_animations_et_leurs_reglages_traversent_l_aller_retour() {
    // Une seule animation testée laisserait passer une famille entière — `arc-en-ciel` n'accepte
    // pas de couleur, et un encodeur qui écrirait quand même la sienne ferait refuser le fichier
    // au redémarrage suivant, jamais à l'écriture.
    for nom in CATALOGUE {
        let anim = animation(nom);
        let reglages = reglages_acceptables(anim);

        let mut eclairage = eclairage_temoin();
        eclairage.animation = Some((anim, reglages));
        let mut zones = Zones::vide();
        zones.poser("zone", &leds("fan:arriere"));
        zones.animer("zone", Some((anim, reglages)));

        let pose = Profil {
            eclairage,
            zones,
            ecran: None,
        };
        assert_eq!(
            aller_retour(&pose),
            pose,
            "« {nom} » et ses réglages doivent traverser l'aller-retour, en couche globale comme \
             en zone"
        );
    }
}

/// Des réglages construits **uniquement** avec les clés que l'animation accepte, comme le ferait
/// un utilisateur. `arc-en-ciel` produit ses propres teintes et refuse `couleur`.
fn reglages_acceptables(anim: Animation) -> Reglages {
    let acceptees = anim.parametres_acceptes();
    let mut reglages = Reglages::default();
    for cle in acceptees {
        match *cle {
            "couleur" => reglages.couleur = reglages_globaux().couleur,
            "vitesse" => reglages.vitesse = 8,
            "direction" => reglages.direction = Direction::AvantArriere,
            autre => panic!(
                "« {autre} » est un paramètre d'animation que ce test ne sait pas fabriquer : le \
                 catalogue s'est étendu, ce test doit suivre"
            ),
        }
    }
    reglages
}

// ---------------------------------------------------------------------------
// 2 — deux enregistrements du même état sont le même fichier
// ---------------------------------------------------------------------------

#[test]
fn deux_encodages_du_meme_etat_sont_identiques_octet_pour_octet() {
    // Test d'intention n° 2 de l'issue : « Deux encodages du même état sont identiques octet pour
    // octet, quel que soit l'ordre d'insertion des zones ». Critère d'acceptation : « l'ordre des
    // zones ne dépend pas de l'ordre d'un `HashMap` ».
    //
    // Ce n'est pas de la coquetterie. Un fichier qui change d'octets sans que l'ambiance ait bougé
    // rend impossible de dire, six mois plus tard, si le profil « nuit » a été modifié ou
    // seulement réenregistré — et fait diverger deux exemples livrés par le dépôt à chaque
    // reconstruction.
    //
    // La forme du test compte : `HashMap` tire une graine **par instance**, donc deux appels sur la
    // même table rendent le même ordre. Il faut reconstruire le profil à chaque tour pour que
    // l'instabilité se voie.
    let reference = profil_temoin().encoder();
    for tour in 0..32 {
        assert_eq!(
            profil_temoin().encoder(),
            reference,
            "tour {tour} : deux profils construits de la même façon doivent produire le même \
             fichier, à l'octet près"
        );
    }

    // Encoder ce qu'on vient de décoder redonne les mêmes octets : sans cela, `save` puis `load`
    // puis `save` produirait un troisième fichier.
    assert_eq!(
        aller_retour(&profil_temoin()).encoder(),
        reference,
        "encoder ∘ décoder ∘ encoder doit être encoder"
    );

    // « Quel que soit l'ordre d'insertion des zones » : les cibles d'une zone sont un ensemble
    // (elles sont « triées et sans doublon », #29). Les désigner dans le désordre — comme le fait
    // une sélection à la souris — ne doit pas changer un octet du fichier.
    let dans_l_ordre = {
        let mut zones = Zones::vide();
        zones.poser("colonne", &leds("fan:arriere"));
        zones.eclairer("colonne", ROUGE_DE_ZONE);
        zones
    };
    let en_desordre = {
        let mut cibles = leds("fan:arriere");
        cibles.reverse();
        cibles.push(cibles[0]);
        let mut zones = Zones::vide();
        zones.poser("colonne", &cibles);
        zones.eclairer("colonne", ROUGE_DE_ZONE);
        zones
    };
    let profil = |zones: Zones| Profil {
        eclairage: eclairage_temoin(),
        zones,
        ecran: None,
    };
    assert_eq!(
        profil(dans_l_ordre).encoder(),
        profil(en_desordre).encoder(),
        "les mêmes LED désignées dans un autre ordre sont la même zone, donc le même fichier"
    );

    // Le pendant : deux ambiances **différentes** ne produisent jamais le même fichier, sinon
    // l'égalité ci-dessus se tiendrait par un encodeur qui n'écrit rien.
    assert_ne!(
        profil_temoin().encoder(),
        autre_profil().encoder(),
        "deux ambiances différentes s'écrivent différemment"
    );
}

// ---------------------------------------------------------------------------
// 6 — l'écran absent, l'écran éteint
// ---------------------------------------------------------------------------

#[test]
fn un_profil_sans_consigne_d_ecran_se_decode() {
    // Test d'intention n° 6 de l'issue : « Un profil dont l'entrée d'écran est absente se décode :
    // l'écran est simplement sans consigne ».
    //
    // C'est la même distinction que celle qui a coûté le plus cher à `eclairage.conf` : « un
    // fichier absent et un fichier disant “noir” ne se confondent jamais » (README). Ici : un
    // profil qui ne dit rien de l'écran le laisse tel quel ; un profil qui dit `Rien` le **rend au
    // firmware**. Les confondre fait qu'un profil enregistré écran éteint rallume la dalle, ou
    // qu'un profil qui ne parlait que d'éclairage éteint la dalle sans qu'on l'ait demandé.
    let sans = Profil {
        ecran: None,
        ..profil_temoin()
    };
    let relu = aller_retour(&sans);
    assert_eq!(
        relu.ecran, None,
        "sans consigne d'écran, le profil n'en invente pas"
    );
    assert_eq!(
        relu.eclairage, sans.eclairage,
        "et l'éclairage traverse quand même"
    );
    assert_eq!(relu.zones, sans.zones, "et les zones aussi");

    let eteint = Profil {
        ecran: Some(EtatEcran {
            luminosite: 0,
            affichage: Affichage::Rien,
        }),
        ..profil_temoin()
    };
    assert_eq!(
        aller_retour(&eteint),
        eteint,
        "« rends la dalle au firmware » est une consigne, et elle se relit"
    );
    assert_ne!(
        sans.encoder(),
        eteint.encoder(),
        "« rien à dire de l'écran » et « ne montre rien » sont deux profils différents : ils ne \
         peuvent pas s'écrire pareil"
    );

    // Les quatre affichages sont quatre consignes distinctes. Un décodeur qui rendrait `Image` là
    // où le fichier dit `Gif` afficherait une image fixe au lieu d'une animation, sans erreur.
    for affichage in [
        Affichage::Rien,
        Affichage::Cadran("kraken2023elite:coolant-temp".to_owned()),
        Affichage::Image("/tmp/a.gif".to_owned()),
        Affichage::Gif("/tmp/a.gif".to_owned()),
    ] {
        let pose = Profil {
            ecran: Some(EtatEcran {
                luminosite: 61,
                affichage,
            }),
            ..profil_temoin()
        };
        assert_eq!(aller_retour(&pose), pose);
    }
}

// ---------------------------------------------------------------------------
// 7, 8, 9 — un fichier abîmé
// ---------------------------------------------------------------------------

#[test]
fn une_entree_absente_est_refusee_en_la_nommant() {
    // Test d'intention n° 7 de l'issue : « Un profil dont une ligne est tronquée est refusé en
    // nommant cette ligne ». Critère d'acceptation : « un fichier de profil tronqué ou illisible
    // est refusé **en nommant l'entrée fautive**, comme `eclairage.conf` le fait déjà ».
    //
    // Un fichier tronqué — copie interrompue, disque plein, éditeur fermé trop vite — est le cas
    // qu'on ne voit pas venir. Le compléter au jugé donne une ambiance plausible et fausse ; c'est
    // exactement ce que le README refuse pour `eclairage.conf`, « plutôt que complété au jugé par
    // un éclairage plausible et faux ».
    //
    // Les entrées visées sont celles qu'on ne peut **pas** omettre : les dix ventilateurs et les
    // quatre barrettes portent chacun une couleur qu'aucune valeur par défaut ne peut deviner.
    let valide = profil_temoin().encoder();

    let mut obligatoires: Vec<String> = Position::ALL.iter().map(|p| p.slug()).collect();
    obligatoires.extend((0..SLOT_COUNT).map(|slot| slot.to_string()));

    for jeton in &obligatoires {
        let rangs = rangs_portant(&valide, jeton);
        assert!(
            !rangs.is_empty(),
            "le témoin doit porter une entrée « {jeton} », sinon la couche globale est incomplète"
        );
        let rang = rangs[0];

        // a) l'entrée disparaît entièrement — le fichier s'arrête trop tôt
        let mut ampute = lignes(&valide);
        let retiree = ampute.remove(rang);
        let erreur = refus(&recoller(&ampute));
        assert!(
            erreur.raison.contains(jeton),
            "l'entrée « {jeton} » manque et le refus doit la nommer — un fichier en porte quatorze, \
             et « profil illisible » les fait toutes relire. Ligne retirée : « {retiree} ». Raison \
             obtenue : {}",
            erreur.raison
        );

        // b) l'entrée est là, mais coupée en cours de route
        let mut tronquee = lignes(&valide);
        let sans_valeur = tronquee[rang]
            .rsplit_once(char::is_whitespace)
            .map(|(debut, _)| debut.to_owned())
            .expect("une entrée porte au moins deux mots");
        tronquee[rang] = sans_valeur.clone();
        let erreur = refus(&recoller(&tronquee));
        assert_eq!(
            erreur.ligne,
            rang + 1,
            "la ligne « {sans_valeur} » est tronquée, le refus doit pointer sa ligne, comptée à \
             partir de 1. Refus obtenu : {erreur}"
        );
        assert!(
            erreur.raison.contains(jeton),
            "et nommer l'entrée « {jeton} ». Raison obtenue : {}",
            erreur.raison
        );
        let _: &dyn std::error::Error = &erreur;
    }

    // Le cas limite du fichier tronqué : il ne reste rien. Un profil vide ne doit pas se décoder en
    // ambiance d'accueil — ce serait un « nuit » qui rallume le boîtier en bleu.
    for vide in ["", "\n\n", "# rien que des commentaires\n"] {
        assert!(
            Profil::decoder(vide).is_err(),
            "un fichier sans aucune entrée n'est pas un profil, c'est un fichier tronqué jusqu'au \
             bout : {vide:?}"
        );
    }
}

#[test]
fn une_entree_repetee_est_refusee_en_la_nommant() {
    // Test d'intention n° 8 de l'issue : « Une entrée répétée est refusée en la nommant, comme
    // `eclairage.conf` ».
    //
    // ⚠️ Cette exigence porte sur **tout** le fichier de profil, y compris les entrées que
    // `zones.conf` tolère aujourd'hui en double (le décodeur des zones y garde la dernière). C'est
    // la lecture littérale du test d'intention — « comme `eclairage.conf` » — et c'est la seule qui
    // tienne pour un fichier dont la promesse est la fidélité : deux couleurs contradictoires pour
    // la même zone, c'est une ambiance qu'on ne sait plus reconstituer, et en garder une au hasard
    // de l'ordre des lignes revient à inventer.
    let valide = profil_temoin().encoder();

    // a) chaque entrée, dupliquée telle quelle — la faute d'un fichier concaténé deux fois
    let rangs: Vec<usize> = valide
        .lines()
        .enumerate()
        .filter(|(_, ligne)| est_une_entree(ligne))
        .map(|(rang, _)| rang)
        .collect();
    assert!(
        rangs.len() >= 14,
        "le témoin doit porter au moins les quatorze entrées de la couche globale, sinon ce \
         balayage ne prouve pas grand-chose : {} entrées",
        rangs.len()
    );
    for rang in rangs {
        let mut doublee = lignes(&valide);
        let ligne = doublee[rang].clone();
        doublee.insert(rang + 1, ligne.clone());
        let erreur = refus(&recoller(&doublee));
        assert_eq!(
            erreur.ligne,
            rang + 2,
            "la ligne « {ligne} » est donnée deux fois, le refus doit pointer la seconde. Refus \
             obtenu : {erreur}"
        );
        assert!(
            !erreur.raison.trim().is_empty(),
            "et dire ce qui cloche, pas seulement où"
        );
    }

    // b) la même entrée, avec une valeur **différente** — la faute qui coûte vraiment cher, parce
    //    qu'elle est indécidable : le fichier porte deux ambiances et rien ne dit laquelle.
    for (description, jeton, remplacement) in [
        (
            "deux couleurs pour le même ventilateur",
            Position::BasGauche.slug(),
            None,
        ),
        (
            "deux couleurs pour la même zone",
            "colonne".to_owned(),
            None,
        ),
        (
            "deux animations pour la même zone",
            "barre".to_owned(),
            Some(("braise", "comete")),
        ),
    ] {
        for rang in rangs_portant(&valide, &jeton) {
            let mut contradictoire = lignes(&valide);
            let original = contradictoire[rang].clone();
            let variante = match remplacement {
                Some((avant, apres)) if original.contains(avant) => original.replace(avant, apres),
                _ => match original.rsplit_once(char::is_whitespace) {
                    Some((debut, fin)) => format!("{debut} {}", tordre(fin)),
                    None => continue,
                },
            };
            if variante == original {
                continue;
            }
            contradictoire.insert(rang + 1, variante.clone());
            let erreur = refus(&recoller(&contradictoire));
            assert!(
                erreur.raison.contains(&jeton),
                "{description} : « {original} » puis « {variante} » — le refus doit nommer \
                 « {jeton} », c'est la seule chose qui dit quoi corriger. Raison obtenue : {}",
                erreur.raison
            );
        }
    }
}

/// Une valeur voisine mais différente : de quoi fabriquer une entrée contradictoire sans savoir ce
/// que la valeur signifie. Un chiffre hexadécimal change, un mot prend un suffixe.
fn tordre(valeur: &str) -> String {
    match valeur.chars().next() {
        Some(premier) if valeur.chars().all(|c| c.is_ascii_hexdigit()) => {
            let autre = if premier == '0' { '1' } else { '0' };
            format!("{autre}{}", &valeur[premier.len_utf8()..])
        }
        _ => format!("{valeur}x"),
    }
}

#[test]
fn une_animation_inconnue_est_refusee_en_la_nommant_sans_emporter_le_reste() {
    // Test d'intention n° 9 de l'issue : « Un profil portant une animation inconnue est refusé en
    // nommant l'animation, sans emporter les autres entrées du fichier ».
    //
    // Le cas se produit vraiment : un profil enregistré aujourd'hui, une animation retirée du
    // catalogue demain, et le fichier reste. Ce qui compte alors est que le message dise *quelle*
    // animation et *où*, pour qu'on puisse corriger la ligne au lieu de jeter l'ambiance.
    for connue in ["vague", "braise"] {
        let valide = profil_temoin().encoder();
        let rang = rang_unique(&valide, connue);
        let mut abimee = lignes(&valide);
        abimee[rang] = abimee[rang].replace(connue, "bidule");
        let erreur = refus(&recoller(&abimee));

        assert_eq!(
            erreur.ligne,
            rang + 1,
            "l'animation inconnue est ligne {}, le refus doit y pointer. Refus obtenu : {erreur}",
            rang + 1
        );
        assert!(
            erreur.raison.contains("bidule"),
            "le refus doit nommer l'animation inconnue. Raison obtenue : {}",
            erreur.raison
        );
        assert!(
            CATALOGUE.iter().any(|nom| erreur.raison.contains(nom)),
            "et dire lesquelles existent, comme `AnimationInconnue` le fait déjà : une animation \
             disparue se remplace, encore faut-il savoir par quoi. Raison obtenue : {}",
            erreur.raison
        );

        // « Sans emporter les autres entrées du fichier » : le refus désigne une ligne et une
        // seule. S'il nommait aussi les ventilateurs ou les zones, il ferait croire que tout le
        // fichier est perdu là où une seule ligne l'est.
        for innocent in [Position::BasGauche.slug(), "colonne".to_owned()] {
            assert!(
                !erreur.raison.contains(&innocent),
                "le refus ne doit pas nommer « {innocent} », qui n'a rien à voir avec l'animation \
                 fautive. Raison obtenue : {}",
                erreur.raison
            );
        }
    }
}

#[test]
fn une_section_inconnue_est_refusee_en_la_nommant() {
    // Un fichier écrit par une version future — ou une faute de frappe dans un fichier édité à la
    // main — porte une entrée que ce démon ne comprend pas. La deviner serait pire que la refuser :
    // ce qu'on ne sait pas lire, on ne sait pas non plus le réécrire, et un `profil save` suivant
    // effacerait silencieusement l'entrée qu'on n'avait pas comprise.
    let valide = profil_temoin().encoder();
    let mut abimee = lignes(&valide);
    let rang = abimee
        .iter()
        .position(|ligne| est_une_entree(ligne))
        .expect("le témoin porte des entrées");
    abimee.insert(rang, "bidule truc machin".to_owned());
    let erreur = refus(&recoller(&abimee));
    assert_eq!(
        erreur.ligne,
        rang + 1,
        "le refus pointe la ligne inconnue. Refus obtenu : {erreur}"
    );
    assert!(
        erreur.raison.contains("bidule"),
        "et nomme le mot qu'il n'a pas compris. Raison obtenue : {}",
        erreur.raison
    );
    let message = erreur.to_string();
    assert!(
        message.contains(&erreur.ligne.to_string()) && message.contains(&erreur.raison),
        "le Display dit la ligne et la raison — c'est lui qui part dans le journal : « {message} »"
    );
}

#[test]
fn le_numero_de_ligne_compte_a_partir_de_un() {
    // Comme `ZonesInvalides::ligne` et `EclairageInvalide::ligne`. Un décalage d'une unité
    // n'empêche aucun démarrage et envoie corriger la ligne d'à côté.
    let valide = profil_temoin().encoder();

    let mut en_tete = lignes(&valide);
    en_tete.insert(0, "pas-une-section truc".to_owned());
    assert_eq!(
        refus(&recoller(&en_tete)).ligne,
        1,
        "une faute sur la première ligne du texte est signalée ligne 1"
    );

    let mut plus_bas = lignes(&valide);
    plus_bas.insert(3, "pas-une-section truc".to_owned());
    assert_eq!(
        refus(&recoller(&plus_bas)).ligne,
        4,
        "une faute sur la quatrième ligne est signalée ligne 4 — les commentaires et les lignes \
         vides comptent, c'est ce que voit un éditeur"
    );
}

// ---------------------------------------------------------------------------
// L'écran d'un profil incomplet
// ---------------------------------------------------------------------------

#[test]
fn un_profil_dont_l_image_a_disparu_applique_l_eclairage_et_les_zones_et_signale_l_ecran() {
    // Critère d'acceptation : « `profil load` d'un profil dont l'image a disparu applique
    // **l'éclairage et les zones**, et signale l'écran sans échouer ». L'issue en donne la raison :
    // « un profil à moitié appliqué qui le dit vaut mieux qu'un profil refusé en bloc parce qu'une
    // photo a été déplacée ».
    let dossier = DossierJetable::neuf("image-disparue");
    let disparue = dossier.fichier("jamais-ecrite.png");
    let profil = Profil {
        ecran: Some(EtatEcran {
            luminosite: 55,
            affichage: Affichage::Image(disparue.to_string_lossy().into_owned()),
        }),
        ..profil_temoin()
    };

    let Application {
        eclairage,
        zones,
        ecran,
        signalements,
    } = profil.preparer();

    assert_eq!(
        eclairage, profil.eclairage,
        "la couche globale s'applique quand même : une photo déplacée n'a rien à voir avec elle"
    );
    assert_eq!(zones, profil.zones, "les zones aussi");
    assert_eq!(
        ecran, None,
        "l'écran n'a pas de consigne applicable : le pousser quand même ferait afficher du noir, \
         ou pire, persisterait un affichage impossible (#69)"
    );
    assert_eq!(
        signalements.len(),
        1,
        "« **seul** l'écran est signalé » : un signalement, et un seul. Obtenus : {signalements:?}"
    );
    assert!(
        signalements[0].contains(&disparue.to_string_lossy().into_owned()),
        "le signalement doit nommer le chemin qui a disparu — c'est la seule information qui \
         permette de le remettre en place. Obtenu : {}",
        signalements[0]
    );
}

#[test]
fn un_profil_dont_l_ecran_tient_ne_signale_rien() {
    // Le pendant du test précédent, et il est indispensable : une implémentation qui signale
    // *toujours* l'écran et ne l'applique jamais passerait le test d'à côté sans un mot, et la
    // dalle resterait au firmware pour tout le monde.
    let dossier = DossierJetable::neuf("ecran-intact");
    let png = dossier.fichier("fond.png");
    fs::write(&png, PNG_1X1).expect("écriture de l'image témoin");

    for affichage in [
        Affichage::Rien,
        Affichage::Cadran("kraken2023elite:coolant-temp".to_owned()),
        Affichage::Image(png.to_string_lossy().into_owned()),
    ] {
        let attendu = EtatEcran {
            luminosite: 44,
            affichage: affichage.clone(),
        };
        let profil = Profil {
            ecran: Some(attendu.clone()),
            ..profil_temoin()
        };
        let prete = profil.preparer();
        assert_eq!(
            prete.signalements,
            Vec::<String>::new(),
            "{affichage:?} est affichable : rien à signaler"
        );
        assert_eq!(
            prete.ecran,
            Some(attendu),
            "{affichage:?} doit être appliqué tel quel, luminosité comprise"
        );
    }

    // Un profil qui ne dit rien de l'écran ne signale rien non plus : ce n'est pas un manque, c'est
    // une absence de consigne.
    let muet = Profil {
        ecran: None,
        ..profil_temoin()
    };
    let prete = muet.preparer();
    assert_eq!(prete.ecran, None);
    assert_eq!(prete.signalements, Vec::<String>::new());
    assert_eq!(prete.eclairage, muet.eclairage);
    assert_eq!(prete.zones, muet.zones);
}

#[test]
fn un_profil_dont_l_image_a_change_de_format_est_signale() {
    // #69 : « un affichage impossible **persisté** faisait redémarrer le démon dans un état cassé,
    // indéfiniment, sans moyen d'en sortir seul. C'est arrivé, et ça a probablement planté la
    // dalle. » Un profil est un second fichier qui décide de ce que la dalle montre : il hérite du
    // même devoir, et le format se reconnaît **au contenu, avant que rien ne bouge**.
    //
    // Le cas n'est pas théorique : un profil garde un chemin, et le fichier au bout du chemin
    // change sans prévenir.
    let dossier = DossierJetable::neuf("format-change");
    let menteur = dossier.fichier("anime.gif");
    fs::write(&menteur, PNG_1X1).expect("écriture du fichier témoin");

    let profil = Profil {
        ecran: Some(EtatEcran {
            luminosite: 70,
            affichage: Affichage::Gif(menteur.to_string_lossy().into_owned()),
        }),
        ..profil_temoin()
    };
    let prete = profil.preparer();

    assert_eq!(
        prete.ecran, None,
        "un GIF qui n'en est pas un ne doit pas partir vers la dalle"
    );
    assert_eq!(
        prete.signalements.len(),
        1,
        "et le dire une fois. Obtenus : {:?}",
        prete.signalements
    );
    assert_eq!(
        prete.eclairage, profil.eclairage,
        "pendant que l'éclairage s'applique : c'est la règle du profil incomplet"
    );
    assert_eq!(prete.zones, profil.zones);
}

// ---------------------------------------------------------------------------
// Le disque — save, load, list, drop
// ---------------------------------------------------------------------------

#[test]
fn un_profil_enregistre_se_relit_apres_un_redemarrage() {
    // Critères d'acceptation : « `profil save nuit` puis modification de l'éclairage puis
    // `profil load nuit` restitue la couche globale, toutes les zones et l'écran tels qu'ils
    // étaient » et « un profil enregistré survit au redémarrage du démon ».
    //
    // Le redémarrage n'a qu'un mécanisme : le disque. C'est lui qu'on vérifie ici — l'écriture, puis
    // une lecture qui ne partage plus rien avec elle.
    let dossier = DossierJetable::neuf("aller-retour-disque");
    let nuit = nom("soirée d'été");
    let pose = profil_temoin();

    let verdict = enregistrer(dossier.chemin(), &nuit, &pose).expect("l'enregistrement réussit");
    assert_eq!(
        verdict,
        Ecriture::Creee,
        "un nom neuf crée un profil, il n'en écrase aucun"
    );

    // « Puis modification de l'éclairage » : ce qui se passe entre les deux ne doit rien changer.
    // Un profil est un instantané, pas une vue sur l'état courant.
    let _entre_temps = autre_profil();

    let relu = charger(dossier.chemin(), &nuit).expect("le profil enregistré se recharge");
    assert_eq!(relu, pose, "et rend exactement l'ambiance enregistrée");

    // Un fichier, et un seul, portant le nom du profil — « un fichier par profil » (approche
    // technique de l'issue).
    let contenu = dossier.contenu();
    assert_eq!(
        contenu.keys().collect::<Vec<_>>(),
        vec![&nuit.fichier()],
        "un profil, un fichier, nommé d'après lui — et aucun fichier temporaire laissé derrière"
    );
}

#[test]
fn lister_donne_les_noms_connus_tries_sans_rien_appliquer() {
    // Critère d'acceptation : « `profil list` n'applique rien et donne les noms connus ».
    //
    // L'ordre importe : `read_dir` rend les entrées dans l'ordre du système de fichiers, qui varie.
    // Une liste qui change d'ordre entre deux appels sans que rien n'ait bougé est une liste qu'on
    // ne peut pas lire.
    let dossier = DossierJetable::neuf("lister");
    for saisi in ["nuit", "aube", "soirée d'été", "LAN party"] {
        enregistrer(dossier.chemin(), &nom(saisi), &profil_temoin()).expect("enregistrement");
    }
    // Des fichiers qui ne sont pas des profils, comme il en traîne dans un répertoire.
    fs::write(dossier.fichier("README"), b"pas un profil").expect("écriture");
    fs::write(dossier.fichier("nuit.conf.tmp"), b"reste d'ecriture").expect("écriture");
    fs::write(dossier.fichier(".cache"), b"cache").expect("écriture");

    let avant = dossier.contenu();
    let noms: Vec<String> = lister(dossier.chemin())
        .iter()
        .map(|n| n.as_str().to_owned())
        .collect();

    let mut attendu = vec!["nuit", "aube", "soirée d'été", "LAN party"];
    attendu.sort_unstable();
    assert_eq!(
        noms, attendu,
        "les noms connus, triés, et **seulement** eux : un fichier posé là par autre chose n'est \
         pas un profil"
    );
    assert_eq!(
        lister(dossier.chemin())
            .iter()
            .map(|n| n.as_str().to_owned())
            .collect::<Vec<_>>(),
        noms,
        "deux appels de suite rendent le même ordre"
    );
    assert_eq!(
        dossier.contenu(),
        avant,
        "« n'applique rien » : lister ne touche même pas à son propre répertoire"
    );

    // Un répertoire qui n'existe pas encore — premier démarrage — est une liste vide, pas une
    // panne : `profil list` doit répondre avant le premier `profil save`.
    let absent = dossier.fichier("pas-encore-cree");
    assert!(
        lister(&absent).is_empty(),
        "un répertoire absent donne une liste vide"
    );
}

#[test]
fn un_profil_oublie_ne_se_charge_plus_et_le_refus_le_nomme() {
    // Critère d'acceptation : « `profil drop` retire le profil, et un `load` ultérieur est refusé
    // **en nommant le profil absent** ». Nommer compte : le message part sur le socket, et
    // « profil introuvable » n'apprend pas si c'est le nom qui est mal tapé ou le profil qui a été
    // supprimé la semaine dernière.
    let dossier = DossierJetable::neuf("oublier");
    let nuit = nom("nuit");
    let jour = nom("jour");
    enregistrer(dossier.chemin(), &nuit, &profil_temoin()).expect("enregistrement");
    enregistrer(dossier.chemin(), &jour, &autre_profil()).expect("enregistrement");

    oublier(dossier.chemin(), &nuit).expect("un profil connu s'oublie");

    match charger(dossier.chemin(), &nuit) {
        Ok(profil) => panic!("un profil oublié ne se charge plus, or il a rendu {profil:?}"),
        Err(erreur) => {
            assert_eq!(erreur, ProfilRefuse::Absent(nuit.clone()));
            assert!(
                erreur.to_string().contains(nuit.as_str()),
                "le refus doit nommer « {} ». Obtenu : {erreur}",
                nuit.as_str()
            );
            let _: &dyn std::error::Error = &erreur;
        }
    }

    // Oublier ce qui n'est pas là est refusé en le nommant aussi : sans cela, une faute de frappe
    // dans `profil drop` passe pour une suppression réussie, et le profil visé reste.
    match oublier(dossier.chemin(), &nom("jamais-enregistre")) {
        Ok(()) => panic!("oublier un profil inconnu doit être refusé, pas passé sous silence"),
        Err(erreur) => assert!(
            erreur.to_string().contains("jamais-enregistre"),
            "le refus doit nommer le profil visé. Obtenu : {erreur}"
        ),
    }

    // « Un fichier par profil : en supprimer un ne réécrit pas les autres » (approche technique).
    assert_eq!(
        charger(dossier.chemin(), &jour).expect("l'autre profil est intact"),
        autre_profil(),
        "oublier un profil ne doit toucher à aucun autre"
    );
    assert_eq!(
        lister(dossier.chemin())
            .iter()
            .map(|n| n.as_str().to_owned())
            .collect::<Vec<_>>(),
        vec!["jour".to_owned()]
    );
}

#[test]
fn enregistrer_sur_un_nom_existant_ecrase_tout_et_le_dit() {
    // Critère d'acceptation : « `profil save` sur un nom existant l'écrase, et le dit ».
    //
    // Deux fautes possibles, symétriques et toutes deux silencieuses : ne pas le dire — et
    // l'utilisateur perd une ambiance sans l'avoir voulu —, ou fusionner l'ancien et le nouveau,
    // qui laisse traîner des zones que l'on croyait remplacées.
    let dossier = DossierJetable::neuf("ecraser");
    let nuit = nom("nuit");

    assert_eq!(
        enregistrer(dossier.chemin(), &nuit, &profil_temoin()).expect("premier enregistrement"),
        Ecriture::Creee
    );
    assert_eq!(
        enregistrer(dossier.chemin(), &nuit, &autre_profil()).expect("second enregistrement"),
        Ecriture::Ecrasee,
        "le second doit **dire** qu'il écrase : c'est la seule occasion de prévenir"
    );
    assert_eq!(
        charger(dossier.chemin(), &nuit).expect("chargement"),
        autre_profil(),
        "et l'écrasement est total : les trois zones du premier profil ne doivent pas survivre \
         sous le second, qui n'en a qu'une"
    );

    // Le sens inverse, parce que c'est celui où une fusion se voit le moins : un profil sans
    // animation globale écrasé par un profil qui en a une.
    assert_eq!(
        enregistrer(dossier.chemin(), &nuit, &profil_temoin()).expect("troisième enregistrement"),
        Ecriture::Ecrasee
    );
    assert_eq!(
        charger(dossier.chemin(), &nuit).expect("chargement"),
        profil_temoin()
    );

    assert_eq!(
        dossier.contenu().len(),
        1,
        "trois enregistrements sous le même nom laissent un fichier, pas trois"
    );
}

#[test]
fn un_fichier_de_profil_abime_est_refuse_en_le_nommant_et_ne_casse_pas_la_liste() {
    // Critère d'acceptation : « un fichier de profil tronqué ou illisible est refusé en nommant
    // l'entrée fautive […] **et ne casse pas `profil list`** ».
    //
    // C'est le motif retenu pour un fichier par profil : « un profil corrompu n'emporte pas la
    // collection » (approche technique). Un `profil list` qui échoue parce qu'un fichier sur douze
    // est abîmé laisse sans moyen de savoir lesquels sont sains.
    let dossier = DossierJetable::neuf("abime");
    let saine = nom("nuit");
    let abimee = nom("cassée");
    enregistrer(dossier.chemin(), &saine, &profil_temoin()).expect("enregistrement");
    enregistrer(dossier.chemin(), &abimee, &profil_temoin()).expect("enregistrement");

    // Le fichier est tronqué en cours d'écriture : il ne reste que le début.
    let texte = profil_temoin().encoder();
    let debut: Vec<String> = lignes(&texte).into_iter().take(3).collect();
    fs::write(dossier.fichier(&abimee.fichier()), recoller(&debut)).expect("troncature");

    match charger(dossier.chemin(), &abimee) {
        Ok(profil) => panic!("un profil tronqué ne se charge pas, or il a rendu {profil:?}"),
        Err(ProfilRefuse::Absent(_)) => {
            panic!("le fichier est là mais illisible : ce n'est pas la même chose qu'absent")
        }
        Err(erreur @ ProfilRefuse::Illisible(..)) => {
            let message = erreur.to_string();
            assert!(
                message.contains(abimee.as_str()),
                "le refus doit nommer le profil : {message}"
            );
        }
    }

    let noms: Vec<String> = lister(dossier.chemin())
        .iter()
        .map(|n| n.as_str().to_owned())
        .collect();
    assert_eq!(
        noms,
        vec!["cassée".to_owned(), "nuit".to_owned()],
        "`profil list` liste les noms **sans décoder** : un profil abîmé reste visible, sinon on \
         ne saurait même pas quoi réparer"
    );
    assert_eq!(
        charger(dossier.chemin(), &saine).expect("le profil sain se charge"),
        profil_temoin(),
        "et le profil voisin n'est pas emporté"
    );
}

// ---------------------------------------------------------------------------
// Deux natures, quatre — et maintenant cinq — fichiers
// ---------------------------------------------------------------------------

#[test]
fn les_profils_vivent_dans_leur_propre_repertoire_sous_var_lib() {
    // Approche technique de l'issue : « un fichier par profil dans `/var/lib/reverb/profils/`,
    // couvert par le `StateDirectory=reverb` existant ».
    //
    // Le README pose la règle : la géométrie est une donnée de montage et reste dans `/etc` ;
    // l'état courant du service est réécrit à chaque commande et va dans `/var/lib`. Un profil est
    // de la seconde nature — il s'enregistre à la demande, il se jette, il ne coûte pas un relevé
    // au sol.
    let repertoire = Path::new(CHEMIN_PROFILS);
    assert!(
        repertoire.starts_with("/var/lib/reverb"),
        "les profils sont de l'état de service, pas de la configuration de montage : {CHEMIN_PROFILS}"
    );
    assert!(
        !repertoire.starts_with(
            Path::new(CHEMIN_GEOMETRIE)
                .parent()
                .unwrap_or(Path::new("/etc"))
        ),
        "et surtout pas à côté de la géométrie, qu'une désinstallation conserve"
    );
    for voisin in [CHEMIN_ECLAIRAGE, CHEMIN_ZONES, CHEMIN_GEOMETRIE] {
        assert_ne!(
            CHEMIN_PROFILS, voisin,
            "un profil ne partage son fichier avec aucun autre état"
        );
    }
}

#[test]
fn un_profil_n_emporte_pas_la_geometrie() {
    // Issue : « Il n'emporte **pas** la géométrie : c'est une donnée de montage, décidée une fois,
    // et qui n'a rien à faire dans une ambiance ». Hors scope, explicitement.
    //
    // La conséquence concrète, et c'est elle qui coûterait : rappeler un profil enregistré avant
    // qu'un ventilateur ne soit démonté puis remis remettrait l'orientation d'avant, et le boîtier
    // se mettrait à tourner à l'envers sans qu'on fasse le lien.
    let texte = profil_temoin().encoder();
    for ligne in Geometrie::mesuree().encoder().lines() {
        let ligne = ligne.trim();
        if !est_une_entree(ligne) {
            continue;
        }
        assert!(
            !texte.contains(ligne),
            "« {ligne} » est une entrée de géométrie, elle n'a rien à faire dans un profil"
        );
    }
}

#[test]
fn le_depot_livre_des_profils_d_exemple() {
    // Critère d'acceptation : « des profils d'exemple sont livrés par le dépôt et installés s'ils
    // sont absents ». Approche technique : « `packaging/profils/` […] Ce sont des exemples livrés
    // par le dépôt, pas la configuration personnelle ».
    //
    // La moitié shell — « une seconde installation n'écrase pas ceux que l'utilisateur a modifiés »
    // — est dans `tools/installe.sh` et se vérifie sur la machine. Ce qui se vérifie ici est que ce
    // que le dépôt livre est lisible par le démon : un exemple qui ne se décode pas ferait échouer
    // la toute première installation, sur la machine de quelqu'un qui n'a encore rien fait.
    let racine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let exemples = racine.join("packaging/profils");
    let entrees = fs::read_dir(&exemples)
        .unwrap_or_else(|erreur| panic!("le dépôt doit livrer {} : {erreur}", exemples.display()));

    let mut vus = Vec::new();
    for entree in entrees {
        let entree = entree.expect("entrée lisible");
        let fichier = entree.file_name().to_string_lossy().into_owned();
        let nom = NomProfil::depuis_fichier(&fichier).unwrap_or_else(|erreur| {
            panic!(
                "« {fichier} » est livré dans packaging/profils mais n'est pas un profil : {erreur}"
            )
        });
        let texte = fs::read_to_string(entree.path()).expect("exemple lisible");
        let profil = Profil::decoder(&texte)
            .unwrap_or_else(|erreur| panic!("l'exemple « {nom} » ne se décode pas : {erreur}"));

        // Un exemple livré par le dépôt ne doit désigner aucun fichier personnel : le garde-fou du
        // projet est « ne jamais commiter la configuration personnelle — fournir un exemple ».
        if let Some(ecran) = &profil.ecran {
            let chemin = match &ecran.affichage {
                Affichage::Image(chemin) | Affichage::Gif(chemin) => Some(chemin.clone()),
                Affichage::Rien | Affichage::Cadran(_) => None,
            };
            if let Some(chemin) = chemin {
                assert!(
                    !chemin.starts_with("/home/"),
                    "l'exemple « {nom} » désigne « {chemin} » : un chemin sous /home est la \
                     configuration de quelqu'un, pas un exemple"
                );
            }
        }
        vus.push((nom, profil));
    }

    assert!(
        vus.len() >= 2,
        "un seul exemple ne montre pas ce qu'un profil sait faire — il en faut au moins deux, et \
         différents. Trouvés : {}",
        vus.len()
    );
    let (_, premier) = &vus[0];
    assert!(
        vus.iter().any(|(_, profil)| profil != premier),
        "les exemples livrés doivent différer entre eux, sinon ils n'illustrent rien"
    );
    assert!(
        vus.iter()
            .any(|(_, profil)| profil.eclairage.animation.is_some()
                || profil
                    .zones
                    .liste()
                    .iter()
                    .any(|zone| matches!(zone.rendu, Rendu::Animee(..)))),
        "au moins un exemple doit porter une animation : c'est ce qu'un profil sait faire de plus \
         qu'une couleur, et ce qu'on veut montrer d'abord"
    );
}
