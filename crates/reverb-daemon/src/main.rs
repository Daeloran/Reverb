//! `reverbd` — le processus qui détient les bus et garde ses descripteurs.
//!
//! Un fil écoute le socket, un fil par client transmet les ordres, et **le fil
//! principal détient seul les périphériques**. Aucun verrou sur le matériel :
//! ce qui n'est pas partagé ne peut pas entrer en collision.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, channel};
use std::thread;
use std::time::{Duration, Instant};

use reverb_anim::{Animation, Geometrie, Orientation, Reglages, Sens};
use reverb_daemon::cadence::{Cadence, Tick};
use reverb_daemon::ecran;
use reverb_daemon::peripheriques::{Consigne, Peripheriques, SOURCE_DU_KRAKEN};
use reverb_daemon::persistance::{self, Eclairage};
use reverb_daemon::profils::{self, Ecriture, Profil};
use reverb_daemon::quarantaine::{Quarantaine, Releve, ReleveMotive};
use reverb_daemon::regulation::{self, Courbe, Regulation};
use reverb_daemon::reparation::{Constat, EtatSource, TENTATIVES_MAXIMALES};
use reverb_daemon::serveur::{self, Ordre};
use reverb_daemon::telemetrie;
use reverb_daemon::zones::{self, Rendu, Tampon, Zones};
use reverb_hw::hwmon::Percent;
use reverb_proto::composition::{Ancre, Composition, Fond, Source};
use reverb_proto::ipc::{
    FanAction, LightTarget, ProfilAction, ReguleAction, Request, ResponseLine, ScreenAction,
};
use reverb_proto::ram::{self, SlotAddress};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};

/// Chemin du socket. Le dossier est créé par systemd (`RuntimeDirectory`).
///
/// Un argument peut lui être substitué — `reverbd /tmp/essai.sock`. Ce n'est
/// pas de la configuration mais une couture de test, la même que celle de
/// `hidraw::discover_in` ou `i2c::find_adapter_in` : sans elle, la boucle de
/// rendu ne se vérifie qu'en installant un service root, ce qui est cher pour
/// constater qu'une couleur ne part pas.
const SOCKET: &str = "/run/reverb/reverbd.sock";

/// Cadence de la boucle de rendu, en images par seconde.
const IMAGES_PAR_SECONDE: u32 = 30;

/// Attente quand rien n'est animé.
///
/// La boucle ne tourne pas à vide : sans animation il n'y a rien à réécrire,
/// le matériel gardant son état. Ce délai n'est qu'un plafond de réactivité,
/// et il est de toute façon interrompu dès qu'un ordre arrive.
const REPOS: Duration = Duration::from_millis(250);

fn main() -> ExitCode {
    let (envoi, reception) = channel();

    let (mut peripheriques, soucis) = match Peripheriques::ouvrir() {
        Ok(ouvert) => ouvert,
        Err(erreur) => {
            eprintln!("erreur : découverte du matériel impossible : {erreur}");
            return ExitCode::FAILURE;
        }
    };
    for souci in &soucis {
        eprintln!("attention : {souci}");
    }

    let socket = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from(SOCKET), PathBuf::from);
    let fichier_geometrie = std::env::args().nth(2).map_or_else(
        || PathBuf::from(persistance::CHEMIN_GEOMETRIE),
        PathBuf::from,
    );
    let fichier_eclairage = std::env::args().nth(3).map_or_else(
        || PathBuf::from(persistance::CHEMIN_ECLAIRAGE),
        PathBuf::from,
    );

    let (geometrie, souci) = persistance::charger_geometrie(&fichier_geometrie);
    if let Some(souci) = souci {
        eprintln!("attention : {souci}");
    }
    let (eclairage, souci) = persistance::charger_eclairage(&fichier_eclairage);
    if let Some(souci) = souci {
        eprintln!("attention : {souci}");
    }
    // Découvertes une fois : un `hwmon` n'apparaît pas en cours de route, et
    // relire l'arborescence chaque seconde coûterait plus que la lecture.
    let sondes = reverb_hw::hwmon::sondes().unwrap_or_else(|erreur| {
        eprintln!("attention : sondes de température illisibles : {erreur}");
        Vec::new()
    });
    eprintln!("sondes de température : {}", sondes.len());
    let fichier_zones = std::env::args()
        .nth(4)
        .map_or_else(|| PathBuf::from(zones::CHEMIN_ZONES), PathBuf::from);
    let (couches, souci) = zones::charger(&fichier_zones);
    if let Some(souci) = souci {
        eprintln!("attention : {souci}");
    }

    let fichier_ecran = std::env::args()
        .nth(5)
        .map_or_else(|| PathBuf::from(ecran::CHEMIN_ECRAN), PathBuf::from);
    let (dalle, souci) = ecran::charger(&fichier_ecran);
    if let Some(souci) = souci {
        eprintln!("attention : {souci}");
    }

    let fichier_regulation = std::env::args().nth(7).map_or_else(
        || PathBuf::from(regulation::CHEMIN_REGULATION),
        PathBuf::from,
    );
    let (reguleur, souci) = regulation::charger_regulation(&fichier_regulation);
    if let Some(souci) = souci {
        eprintln!("attention : {souci}");
    }
    // Un canal conservé mais disparu du bus se dit **au démarrage**, une fois :
    // la boucle, elle, ne le découvrirait qu'au prochain changement de palier,
    // et l'écrirait dans le journal à ce moment-là — c'est-à-dire un jour de
    // température stable, jamais.
    for canal in reguleur.canaux() {
        if !peripheriques
            .canaux()
            .iter()
            .any(|connu| connu.name == canal)
        {
            eprintln!(
                "attention : canal « {canal} » régulé mais introuvable — « regule {canal} off » \
                 pour cesser de le viser"
            );
        }
    }

    let mut etat = Etat::nouveau(
        eclairage,
        geometrie,
        Fichiers {
            geometrie: fichier_geometrie,
            eclairage: fichier_eclairage,
            zones: fichier_zones,
            ecran: fichier_ecran,
            profils: std::env::args()
                .nth(6)
                .map_or_else(|| PathBuf::from(profils::CHEMIN_PROFILS), PathBuf::from),
            regulation: fichier_regulation,
        },
        couches,
        sondes,
        lancer_le_fil_du_gpu(),
        dalle,
    )
    .avec_regulation(reguleur);

    // Les ambiances d'exemple, au tout premier démarrage : le répertoire est
    // absent, personne n'a encore rien enregistré, et une machine neuve a de
    // quoi montrer ce qu'un profil sait faire sans avoir à en composer un.
    let poses = profils::poser_les_exemples(&etat.fichiers.profils);
    if !poses.is_empty() {
        eprintln!("profils d'exemple posés : {}", poses.join(", "));
    }

    // La dalle retrouve ce qu'elle affichait, si le démon tient l'écran. Un
    // fichier effacé depuis le dernier démarrage se dit **une fois** : la dalle
    // reste au firmware plutôt que de réessayer en boucle.
    if peripheriques.a_un_ecran() {
        // Déposée, pas tentée (#83) : un refus reviendra par le verdict du
        // premier tour, et sera journalisé là — jamais ici, où l'attendre
        // coûterait les cinq secondes d'un Kraken muet avant même que le socket
        // soit ouvert.
        peripheriques.luminosite_ecran(etat.ecran.luminosite);
        if let Err(message) = etat.preparer_ecran() {
            eprintln!("attention : écran non rétabli : {message}");
            etat.ecran.affichage = ecran::Affichage::Rien;
        }
    }

    let annonce = socket.display().to_string();
    thread::spawn(move || {
        if let Err(erreur) = serveur::servir(&socket, envoi) {
            eprintln!("erreur : socket {} : {erreur}", socket.display());
            std::process::exit(1);
        }
    });

    // Le socket est ouvert : systemd peut débloquer `systemctl start`, et les
    // clients qui suivent ne trouveront pas porte close.
    signaler_pret();
    println!("reverbd écoute sur {annonce}");

    boucle(&mut peripheriques, &reception, etat);
    ExitCode::SUCCESS
}

/// L'état d'éclairage courant.
/// Ce que le fil du GPU a lu en dernier.
///
/// Le pilote propriétaire NVIDIA n'enregistre aucun `hwmon` : la RTX 5070 se lit
/// par `nvidia-smi`, mesuré à 16 ms — un tiers d'image de rendu. Un fil à part
/// et un cache, pour que la boucle qui écrit sur les bus n'attende jamais un
/// processus externe.
type CacheGpu = std::sync::Arc<std::sync::Mutex<Option<(String, i32)>>>;

/// Interroge `nvidia-smi` une fois par seconde, à côté.
fn lancer_le_fil_du_gpu() -> CacheGpu {
    let cache: CacheGpu = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sien = cache.clone();
    std::thread::spawn(move || {
        loop {
            let lu = reverb_hw::gpu::nvidia();
            if let Ok(mut place) = sien.lock() {
                *place = lu;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
    cache
}

/// Les fichiers que le démon conserve, et leur nature.
///
/// `geometrie.conf` décrit la **machine** — un montage, qu'un administrateur
/// peut corriger — d'où `/etc`. Les autres décrivent l'**état du service**,
/// écrit par le démon, d'où `/var/lib`.
struct Fichiers {
    geometrie: PathBuf,
    eclairage: PathBuf,
    zones: PathBuf,
    ecran: PathBuf,
    /// Le **répertoire** des ambiances nommées, et non un fichier : un profil
    /// par fichier, pour qu'en supprimer un ne réécrive pas les autres (#74).
    profils: PathBuf,
    /// Quels canaux le démon régule, et sur quelle courbe (#99).
    regulation: PathBuf,
}

/// Ce que le démon pousse sur la dalle, et à quel rythme.
///
/// Le firmware reprend la main une trentaine de secondes après le dernier envoi
/// (spec §2.2.2) : **rien de ce qui est affiché ne tient sans réémission**, pas
/// même une image fixe. D'où une échéance dans tous les cas sauf `Rien`.
enum Diffusion {
    /// La dalle est rendue au firmware : le démon n'écrit plus rien.
    Rien,
    /// Une image fixe, réémise avant le repli.
    Fixe(Box<ecran::Dalle>),
    /// Un GIF, une image par délai, en boucle.
    Anime {
        dalles: Vec<ecran::Dalle>,
        delais: Vec<Duration>,
        rang: usize,
    },
    /// Un cadran, redessiné à chaque tour avec la valeur du moment.
    Cadran(String),
    /// Une composition : un fond décodé une fois, des champs redessinés dessus
    /// (#80).
    Composee {
        /// Le fond, décodé et mis à l'échelle **une seule fois**.
        ///
        /// ⚠️ Redécoder un PNG toutes les deux secondes pour changer trois
        /// chiffres coûterait le décodage complet d'une image à chaque tour. Le
        /// fond ne change que quand on le change.
        fond: Box<ecran::Dalle>,
        /// La dernière image **réellement poussée**.
        ///
        /// C'est elle qui permet de ne rien envoyer quand rien n'a bougé : le
        /// protocole n'a aucune mise à jour partielle (spec §2, §3.6), donc
        /// chaque changement coûte 1,2 Mo, et à température stable il n'y a
        /// rien à changer.
        derniere: Option<Box<ecran::Dalle>>,
        /// Quand elle est partie. Le repli du firmware ne se négocie pas :
        /// même identique, l'image doit repartir avant les trente secondes.
        dernier_envoi: Option<Instant>,
    },
}

/// Entre deux images d'un cadran.
///
/// Bien plus court que le repli du firmware : un cadran qui ne bougerait
/// qu'une fois toutes les vingt-cinq secondes serait une photo de température,
/// et « le cadran se met à jour tant qu'il est affiché » serait faux à l'œil.
const CADENCE_CADRAN: Duration = Duration::from_secs(2);

/// Le plancher d'un GIF : ce qu'une image de 1,2 Mo coûte sur le bus.
///
/// Mesuré à l'envoi : annonce, en-tête, 1,2 Mo de bulk, validation. En deçà, le
/// démon ne tiendrait pas la cadence et prendrait du retard sans le dire — d'où
/// `ecran::cadence`, qui **ralentit** le GIF au lieu d'en sauter des images.
const PLANCHER_GIF: Duration = Duration::from_millis(100);

struct Etat {
    ventilateurs: [Rgb; 10],
    barrettes: [Rgb; ram::SLOT_COUNT],
    animation: Option<(Animation, Reglages)>,
    /// Où se trouve physiquement chaque LED, et comment chaque ventilateur est
    /// monté. Réglable par le socket, conservée sur disque.
    geometrie: Geometrie,
    /// Les trois fichiers qui conservent l'état d'un démarrage à l'autre.
    fichiers: Fichiers,
    /// Une couleur fixe a changé et n'a pas encore été écrite.
    a_ecrire: bool,
    /// Les fenêtres abonnées aux images, s'il y en a.
    abonnes: Vec<SyncSender<Vec<ResponseLine>>>,
    /// Toutes les sondes de température de la machine, découvertes au démarrage.
    sondes: Vec<reverb_hw::hwmon::Sonde>,
    /// La dernière température du GPU discret, lue par un fil à part.
    gpu: CacheGpu,
    /// Ce que chaque LED affiche vraiment, hors animation.
    ///
    /// ⚠️ **Plus fin que ce qui est conservé sur disque.** `eclairage.conf`
    /// garde une couleur par cible (#21) ; une LED peinte à la main revient
    /// donc à la couleur de son ventilateur au redémarrage suivant. Le
    /// dire ici, parce que rien d'autre ne le dira.
    /// L'éclairage fixe, LED par LED. C'est la **couche globale** : ce que les
    /// zones recouvrent, et ce qui reste là où aucune ne passe.
    fixe: Tampon,
    /// Les couches nommées.
    zones: Zones,
    /// L'écran du Kraken : sa luminosité et ce qu'il montre. Conservé sur
    /// disque, comme l'éclairage.
    ecran: ecran::Etat,
    /// Ce qui part vraiment sur la dalle, déjà décodé et mis à l'échelle.
    ///
    /// Séparé de `ecran` parce que ce n'est pas la même nature : l'un est un
    /// choix qu'on conserve, l'autre est un mégaoctet de pixels qu'on ne
    /// conserve jamais et qu'on redécode au démarrage.
    diffusion: Diffusion,
    /// Quand la prochaine image doit partir. `None` : rien à pousser.
    ///
    /// ⚠️ **Ce n'est pas une horloge d'envoi mais une horloge de dépôt** (#83) :
    /// l'image est confiée au fil de l'écran, qui la pousse quand il peut. Un
    /// envoi qui traîne ne décale donc pas le suivant, il le remplace.
    echeance_ecran: Option<Instant>,
    /// Les sondes écartées parce qu'elles ne répondent plus (#68).
    quarantaine: Quarantaine,
    /// Les cibles — canaux et sondes — dont la **dernière lecture** a échoué.
    ///
    /// ⚠️ **Ce n'est pas la quarantaine**, et les confondre coûterait le plafond
    /// de #98. Une réparation réussie **oublie** la quarantaine d'une cible pour
    /// qu'elle soit relue sans délai ; la cible n'a pas répondu pour autant. Si
    /// « muette » se lisait dans la quarantaine, chaque reset ferait paraître la
    /// source revenue, le compte de tentatives repartirait de zéro, et le démon
    /// secouerait le Kraken jusqu'au redémarrage — la boucle exacte que l'issue
    /// interdit, atteinte par le chemin qu'elle ne nomme pas.
    ///
    /// Seule une lecture qui **réussit** en retire une cible.
    muettes: HashSet<String>,
    /// Quand soumettre le prochain état des sources au fil de réparation (#98).
    prochaine_soumission: Duration,
    /// Ce qu'il reste à faire après un reset qui a réussi, et à partir de quand.
    reprise: Option<Reprise>,
    /// La dernière valeur de la sonde que `thermique` suit, en degrés (#75).
    ///
    /// `None` : aucune animation ne suit de sonde, ou celle-ci est écartée. Le
    /// boîtier passe alors au blanc pulsant — jamais à la dernière valeur
    /// connue, qui ferait croire à une mesure à jour.
    mesure: Option<f32>,
    /// Quand relire cette sonde.
    prochaine_mesure: Duration,
    /// Les canaux de ventilation que le démon régule lui-même (#99).
    regulation: Regulation,
    /// Les canaux régulés qui **ne portent pas** ce qu'on leur a écrit (#110).
    ///
    /// ⚠️ **C'est une mémoire de journal, et rien d'autre.** La décision d'écrire
    /// appartient tout entière à [`Regulation::tour`], qui la prend sur la
    /// relecture ; ce jeu-là ne sert qu'à n'imprimer qu'une ligne à l'entrée dans
    /// l'épisode et une à la sortie. Une consigne non appliquée repart à chaque
    /// tour, soit 86 400 fois par jour : la journaliser à chaque fois rendrait le
    /// journal inutilisable au moment précis où il servirait.
    ///
    /// C'est le même partage que pour la quarantaine, dont le `signaler` vit
    /// dans [`crate::quarantaine`] et la phrase dans l'appelant.
    non_appliques: HashSet<String>,
    /// Quand relire le liquide pour eux.
    ///
    /// ⚠️ **Ce n'est pas un réveil, c'est une échéance.** La boucle tourne déjà
    /// — toutes les 250 ms au repos, à la cadence des images sous animation — et
    /// la régulation ne fait que s'y greffer. Sans canal régulé, ce champ n'est
    /// même pas consulté.
    prochaine_regulation: Duration,
    /// L'instant du démarrage, seule origine de temps du démon.
    ///
    /// La quarantaine ne tient aucune horloge — c'est ce qui la rend
    /// vérifiable sans attendre. Elle reçoit donc un instant, et il se compte
    /// depuis ici.
    naissance: Instant,
}

impl Etat {
    fn nouveau(
        eclairage: Eclairage,
        geometrie: Geometrie,
        fichiers: Fichiers,
        zones: Zones,
        sondes: Vec<reverb_hw::hwmon::Sonde>,
        gpu: CacheGpu,
        ecran: ecran::Etat,
    ) -> Etat {
        Etat {
            ventilateurs: eclairage.ventilateurs,
            barrettes: eclairage.barrettes,
            animation: eclairage.animation,
            geometrie,
            fichiers,
            // Écrire l'état au démarrage : c'est ce qui donne un éclairage
            // connu au boot, sans qu'aucune fenêtre soit ouverte.
            a_ecrire: true,
            abonnes: Vec::new(),
            sondes,
            gpu,
            fixe: Tampon {
                ventilateurs: eclairage
                    .ventilateurs
                    .map(|couleur| [couleur; LEDS_PER_FAN as usize]),
                barrettes: eclairage
                    .barrettes
                    .map(|couleur| [couleur; ram::LEDS_PER_STICK]),
            },
            zones,
            ecran,
            // Décodée juste après la construction, par `reprendre_l_ecran` : le
            // fichier ne garde qu'un chemin, jamais des pixels.
            diffusion: Diffusion::Rien,
            echeance_ecran: None,
            quarantaine: Quarantaine::nouvelle(),
            muettes: HashSet::new(),
            prochaine_soumission: Duration::ZERO,
            reprise: None,
            mesure: None,
            prochaine_mesure: Duration::ZERO,
            // Celle d'un premier démarrage : aucun canal régulé.
            // `avec_regulation` pose ensuite celle qu'on vient de relire.
            regulation: Regulation::nouvelle(Courbe::defaut()),
            // Aucun épisode en cours : rien n'a encore été écrit nulle part.
            non_appliques: HashSet::new(),
            // Zéro : les canaux régulés sont écrits au tout premier tour. Rien
            // ne survit au redémarrage côté matériel, et `nzxtsmart2` repart de
            // son défaut d'allumage — 25 %, le défaut que #99 corrige.
            prochaine_regulation: Duration::ZERO,
            naissance: Instant::now(),
        }
    }

    /// Note ce qu'une lecture vient de dire d'une cible (#98).
    ///
    /// C'est la seule mémoire de « qui a répondu » du démon, et c'est elle qui
    /// décide de l'effondrement d'une source. Elle est alimentée par **toutes**
    /// les lectures — celles du `status`, celle de la sonde que `thermique` suit,
    /// celles des champs d'une composition — parce qu'une source ne se déclare
    /// effondrée que si tout ce qu'elle porte s'est tu.
    fn noter(&mut self, cible: &str, a_repondu: bool) {
        if a_repondu {
            self.muettes.remove(cible);
        } else if !self.muettes.contains(cible) {
            self.muettes.insert(cible.to_owned());
        }
    }

    /// Pose la régulation relue au démarrage (#99).
    ///
    /// À part de [`Etat::nouveau`], qui porte déjà les sept paramètres que
    /// clippy tolère — et parce que ce sont deux gestes distincts : construire
    /// l'état courant, puis y verser ce qu'un fichier a rendu.
    fn avec_regulation(mut self, regulation: Regulation) -> Etat {
        self.regulation = regulation;
        self
    }

    /// Prépare ce que la dalle doit montrer, depuis l'affichage conservé.
    ///
    /// Rend le message d'échec s'il y en a un — un fichier effacé depuis le
    /// dernier démarrage, par exemple. La dalle reste alors au firmware, et le
    /// démon le dit **une fois** plutôt que de réessayer en boucle.
    fn preparer_ecran(&mut self) -> Result<(), String> {
        let (diffusion, echeance) = match &self.ecran.affichage {
            ecran::Affichage::Rien => (Diffusion::Rien, None),
            ecran::Affichage::Cadran(sonde) => {
                (Diffusion::Cadran(sonde.clone()), Some(Instant::now()))
            }
            ecran::Affichage::Image(chemin) => {
                let dalles = ecran::Dalle::depuis_fichier(Path::new(chemin))
                    .map_err(|erreur| erreur.to_string())?;
                let premiere = dalles
                    .into_iter()
                    .next()
                    .ok_or_else(|| format!("{chemin} : aucune image"))?;
                (Diffusion::Fixe(Box::new(premiere)), Some(Instant::now()))
            }
            ecran::Affichage::Composition(composition) => {
                let fond = ecran::fond_en_dalle(composition.fond())
                    .map_err(|erreur| erreur.to_string())?;
                (
                    Diffusion::Composee {
                        fond: Box::new(fond),
                        derniere: None,
                        dernier_envoi: None,
                    },
                    Some(Instant::now()),
                )
            }
            ecran::Affichage::Gif(chemin) => {
                let (dalles, ecrits) = ecran::Dalle::animee_depuis_fichier(Path::new(chemin))
                    .map_err(|erreur| erreur.to_string())?;
                // Les délais écrits dans le fichier, **relevés au plancher du
                // bus** : un GIF à trente images par seconde est ralenti, pas
                // tronqué. Un mouvement lent et complet se regarde.
                let delais = ecran::cadence(&ecrits, PLANCHER_GIF);
                (
                    Diffusion::Anime {
                        dalles,
                        delais,
                        rang: 0,
                    },
                    Some(Instant::now()),
                )
            }
        };
        self.diffusion = diffusion;
        self.echeance_ecran = echeance;
        Ok(())
    }

    /// L'état de la dalle, sous la forme qu'un client attend.
    fn ligne_ecran(&self) -> ResponseLine {
        ResponseLine::Screen {
            luminosite: self.ecran.luminosite,
            affichage: match &self.ecran.affichage {
                ecran::Affichage::Rien => "rien".to_owned(),
                ecran::Affichage::Cadran(sonde) => format!("gauge:{sonde}"),
                ecran::Affichage::Image(chemin) => format!("image:{chemin}"),
                ecran::Affichage::Gif(chemin) => format!("gif:{chemin}"),
                // Un jeton, et le détail sur les lignes qui suivent : un fond
                // et quatre champs ne tiennent pas sur une ligne, et les y
                // tasser demanderait un échappement que ce protocole n'a pas.
                ecran::Affichage::Composition(_) => "layout".to_owned(),
            },
        }
    }

    /// Les lignes qui décrivent la composition courante, s'il y en a une.
    ///
    /// Vides quand la dalle n'en porte pas : leur absence *est* l'information,
    /// et une ligne « layout rien » ferait un second moyen de dire ce que
    /// `screen` dit déjà.
    fn lignes_composition(&self) -> Vec<ResponseLine> {
        let ecran::Affichage::Composition(composition) = &self.ecran.affichage else {
            return Vec::new();
        };
        let mut lignes = vec![ResponseLine::Layout {
            fond: composition.fond().encoder(),
        }];
        lignes.extend(composition.champs().into_iter().map(|(ancre, source)| {
            ResponseLine::LayoutChamp {
                ancre: ancre.slug().to_owned(),
                source: source.encoder(),
            }
        }));
        lignes
    }

    /// La composition courante, ou une neuve sur fond noir.
    ///
    /// ⚠️ **Poser un champ ou un fond entre dans la composition** : la dalle qui
    /// montrait une image ou rien en porte une dès la première commande. Exiger
    /// un « layout on » avant de pouvoir poser quoi que ce soit n'apprendrait
    /// rien à personne et ferait échouer la commande la plus évidente.
    fn composition_courante(&self) -> Composition {
        match &self.ecran.affichage {
            ecran::Affichage::Composition(composition) => composition.clone(),
            // Une image affichée devient le fond de la composition qu'on
            // commence : c'est ce que quelqu'un qui tape « layout champ » sur
            // une photo veut, et repartir du noir lui ferait perdre sa photo.
            ecran::Affichage::Image(chemin) => Composition::nouvelle(Fond::Image(chemin.clone())),
            _ => Composition::nouvelle(Fond::Noir),
        }
    }

    /// Une zone au moins tourne : le boîtier doit être redessiné à la cadence,
    /// même si la couche globale est fixe.
    fn zone_animee(&self) -> bool {
        self.zones
            .liste()
            .iter()
            .any(|zone| matches!(zone.rendu, Rendu::Animee(..)))
    }

    /// Ce que le boîtier montre à ce pas : la couche globale, puis les zones
    /// par-dessus.
    fn tampon(&self, pas: u32) -> Tampon {
        let mut tampon = match &self.animation {
            Some((animation, reglages)) => {
                // ⚠️ **La mesure vient du cache, jamais d'une lecture ici.**
                // `tampon` est appelée à chaque image — vingt à trente fois par
                // seconde —, et une lecture sysfs sur une sonde qui ne répond
                // plus bloque cinq secondes en sommeil non interruptible (#68).
                // Relever ici gèlerait le démon à chaque image.
                let image =
                    animation.image_avec_mesure(&self.geometrie, reglages, pas, self.mesure);
                let mut tampon = Tampon::noir();
                for (position, couleurs) in &image.ventilateurs {
                    tampon.ventilateurs[position.index()] = *couleurs;
                }
                tampon.barrettes = image.barrettes;
                tampon
            }
            None => self.fixe.clone(),
        };
        self.zones.composer(&self.geometrie, pas, &mut tampon);
        tampon
    }

    /// L'état à conserver sur disque.
    fn eclairage(&self) -> Eclairage {
        Eclairage {
            ventilateurs: self.ventilateurs,
            barrettes: self.barrettes,
            animation: self.animation.clone(),
        }
    }

    /// Relit la sonde que l'animation en cours suit, si l'heure est venue.
    ///
    /// ⚠️ **Au plus une fois par [`INTERVALLE_MESURE`]**, et par la quarantaine
    /// (#68). Une température ne bouge pas plus vite que cela, et une sonde
    /// muette coûte cinq secondes de sommeil non interruptible — la relire à la
    /// cadence des images rendrait le démon inutilisable, ce qui est exactement
    /// la panne que #68 a corrigée.
    ///
    /// Une sonde écartée rend `None`, donc le blanc pulsant : jamais la dernière
    /// valeur connue, qui ferait croire à une température à jour.
    fn rafraichir_la_mesure(&mut self) {
        let Some(slug) = self
            .animation
            .as_ref()
            .and_then(|(_, reglages)| reglages.sonde.clone())
        else {
            // Aucune animation ne suit de sonde : rien à relever, et le cache
            // est vidé pour qu'une reprise ne parte pas d'une valeur périmée.
            self.mesure = None;
            return;
        };

        let maintenant = self.naissance.elapsed();
        if maintenant < self.prochaine_mesure {
            return;
        }
        self.prochaine_mesure = maintenant + INTERVALLE_MESURE;

        let Some(sonde) = self.sondes.iter().find(|sonde| sonde.slug == slug) else {
            self.mesure = None;
            return;
        };
        let verdict = self
            .quarantaine
            .tour(&sonde.slug, maintenant, || sonde.lire().ok());
        self.mesure = match verdict {
            Releve::Valeur(millidegres) => Some(millidegres as f32 / 1000.0),
            Releve::Muette { .. } => None,
        };
        // Cette lecture-ci compte comme les autres : une machine qui n'ouvre
        // jamais de fenêtre mais anime `thermique` est un cas parfaitement
        // ordinaire, et son démon doit voir la source tomber (#98).
        self.noter(&slug, self.mesure.is_some());
    }

    /// Un tour de régulation des canaux que leur pilote ne régule pas (#99).
    ///
    /// ⚠️ **Aucun réveil ajouté, et c'est un critère d'acceptation.** Cette
    /// fonction ne dort pas, ne pose pas de minuterie et ne raccourcit aucune
    /// attente : elle se greffe sur les tours que la boucle fait déjà — un
    /// toutes les 250 ms au repos, un par image sous animation. Sans canal
    /// régulé, elle rend la main avant de lire l'horloge, et le démon reste au
    /// repos absolu.
    ///
    /// ⚠️ **Le liquide est relu au plus une fois par seconde**, jamais à la
    /// cadence des images, et **par la quarantaine** (#68) : une lecture sysfs
    /// sur un périphérique muet bloque cinq secondes en sommeil non
    /// interruptible, dans le fil qui sert aussi le socket. Une sonde écartée
    /// rend `None`, donc le repli — jamais la dernière valeur connue.
    ///
    /// ⚠️ **Ce que les canaux régulés portent est relu ici, et eux seuls** (#110).
    /// Relire tous les canaux ferait payer chaque seconde une lecture du Kraken —
    /// le périphérique même dont on sait qu'il se tait —, dans ce même fil. La
    /// lecture passe par la **même quarantaine et sous la même clef** que celle
    /// de `status` (#88) : un canal muet est écarté une fois pour les deux
    /// chemins, et non deux fois pour cinq secondes chacune.
    fn tour_de_regulation(&mut self, peripheriques: &Peripheriques) {
        if self.regulation.canaux().is_empty() {
            return;
        }

        let maintenant = self.naissance.elapsed();
        if maintenant < self.prochaine_regulation {
            return;
        }
        self.prochaine_regulation = maintenant + INTERVALLE_MESURE;

        let liquide = match self
            .sondes
            .iter()
            .find(|sonde| sonde.slug == regulation::SONDE_DU_LIQUIDE)
        {
            Some(sonde) => match self
                .quarantaine
                .tour(&sonde.slug, maintenant, || sonde.lire().ok())
            {
                Releve::Valeur(millidegres) => Some(millidegres),
                Releve::Muette { .. } => None,
            },
            // La sonde n'existe pas sur cette machine : c'est aussi un liquide
            // illisible, et le repli s'applique. Le taire ferait tourner les
            // sept ventilateurs à leur défaut d'allumage sans un mot.
            None => None,
        };

        let regules = self.regulation.canaux();
        let portees = self.relire_les_canaux(peripheriques, &regules, maintenant);

        // ⚠️ **Ces deux ensembles portent la décision, pas son issue.** Une
        // écriture qui échoue laisse le canal exactement où il était : la ranger
        // avec celles qui n'ont pas eu lieu ferait conclure à `clore_les_episodes`
        // qu'il porte enfin sa consigne — et le démon annoncerait une réparation
        // qui n'a pas eu lieu, très exactement la faute que #110 corrige.
        let mut rejoues: HashSet<String> = HashSet::new();
        let mut neuves: HashSet<String> = HashSet::new();
        for regulation::Ecriture {
            canal,
            consigne,
            motif,
        } in self.regulation.tour(liquide, &portees)
        {
            match motif {
                regulation::Motif::Consigne => neuves.insert(canal.clone()),
                regulation::Motif::NonAppliquee { .. } => rejoues.insert(canal.clone()),
            };

            let ecrite = Percent::new(consigne)
                .map_err(|erreur| erreur.to_string())
                .and_then(|percent| {
                    peripheriques
                        .consigner(&canal, Consigne::Pwm(percent))
                        .map_err(|erreur| erreur.to_string())
                });
            let Err(message) = ecrite else {
                match motif {
                    // Une consigne qui change se dit : sept ventilateurs sur dix
                    // en dépendent, et c'est la trace qui manquait pendant les
                    // soixante-douze minutes mesurées le 2026-08-15.
                    regulation::Motif::Consigne => println!(
                        "régulation : {canal} à {consigne} %{}",
                        match liquide {
                            Some(millidegres) =>
                                format!(" (liquide {:.1} °C)", f64::from(millidegres) / 1000.0),
                            None => " (liquide illisible — repli)".to_owned(),
                        }
                    ),
                    // ⚠️ **Une seule ligne par épisode.** Cette écriture-ci repart
                    // à chaque tour tant que le canal n'applique pas, soit 86 400
                    // par jour — le chiffre même que #99 invoquait pour justifier
                    // son cache. Un journal qui les imprimerait toutes serait
                    // illisible au moment précis où il servirait.
                    regulation::Motif::NonAppliquee { porte } => {
                        if self.non_appliques.insert(canal.clone()) {
                            eprintln!(
                                "attention : régulation : « {canal} » porte {} alors qu'il a reçu \
                                 {consigne} % — la consigne est rejouée à chaque tour, en silence, \
                                 jusqu'à ce qu'elle prenne",
                                match porte {
                                    Some(pourcent) => format!("{pourcent} %"),
                                    None => "autre chose".to_owned(),
                                }
                            );
                        }
                    }
                }
                continue;
            };
            eprintln!("attention : régulation de {canal} : {message}");
        }

        self.clore_les_episodes(&regules, &portees, &rejoues, &neuves);
    }

    /// Ce que les canaux régulés portent vraiment, relu par la quarantaine (#110).
    ///
    /// ⚠️ **`tour_motive` et non `tour`**, alors que seul le pourcentage est
    /// gardé : la quarantaine est partagée avec `status`, et c'est elle qui
    /// retient la raison d'un silence pour la redire tour après tour. Perdre la
    /// raison ici, c'est faire répondre `unreadable <canal>` sans diagnostic à la
    /// fenêtre — or « Connection timed out (os error 110) » est très exactement
    /// ce qui a permis d'écrire #88.
    ///
    /// ⚠️ **Un canal absent du matériel rend `None`**, comme un illisible : entre
    /// une redécouverte (#98) et un débranchement, la régulation ne peut rien
    /// faire de la différence — dans les deux cas elle ne sait pas ce qu'il porte.
    fn relire_les_canaux(
        &mut self,
        peripheriques: &Peripheriques,
        regules: &[String],
        maintenant: Duration,
    ) -> BTreeMap<String, Option<u8>> {
        let mut portees = BTreeMap::new();
        for nom in regules {
            let Some(canal) = peripheriques.canaux().iter().find(|c| &c.name == nom) else {
                portees.insert(nom.clone(), None);
                continue;
            };
            let verdict = self.quarantaine.tour_motive(nom, maintenant, || {
                telemetrie::lire_le_canal(canal, peripheriques.courbes_posees())
            });
            let porte = match verdict {
                ReleveMotive::Valeur(lu) => lu.pwm,
                // L'écartement se dit une fois, comme dans `telemetrie::releve` —
                // et il faut le dire ici aussi : sans fenêtre ouverte, aucun
                // `status` ne passe, et l'épisode ne serait signalé nulle part.
                ReleveMotive::Muette { signaler, raison } => {
                    if signaler {
                        eprintln!(
                            "attention : canal « {nom} » écarté : {raison} — retenté plus tard, \
                             de plus en plus espacé"
                        );
                    }
                    None
                }
            };
            portees.insert(nom.clone(), porte);
        }
        portees
    }

    /// Dit, une fois, qu'un canal s'est remis à porter sa consigne (#110).
    ///
    /// ⚠️ **La sortie d'épisode compte autant que l'entrée.** Sans elle le journal
    /// laisse le lecteur sur un incident sans fin : une ligne « fan-3 porte 25 %
    /// au lieu de 50 % », et plus rien, y compris le jour où le canal obéit de
    /// nouveau.
    ///
    /// Un canal lisible sur lequel rien n'est parti porte **forcément** sa
    /// consigne — c'est la règle de [`Regulation::tour`] —, d'où un message qui
    /// affirme un fait mesuré et non une espérance. Les trois autres sorties se
    /// taisent, et chacune pour sa raison : un canal coupé ne nous regarde plus,
    /// une consigne neuve vient de s'imprimer d'elle-même, et un canal devenu
    /// illisible n'a rien dit du tout — son épisode reste donc ouvert.
    fn clore_les_episodes(
        &mut self,
        regules: &[String],
        portees: &BTreeMap<String, Option<u8>>,
        rejoues: &HashSet<String>,
        neuves: &HashSet<String>,
    ) {
        self.non_appliques.retain(|canal| {
            if rejoues.contains(canal) {
                return true;
            }
            if !regules.contains(canal) || neuves.contains(canal) {
                return false;
            }
            match portees.get(canal) {
                Some(Some(porte)) => {
                    println!("régulation : « {canal} » porte enfin sa consigne ({porte} %)");
                    false
                }
                _ => true,
            }
        });
    }
}

/// Entre deux lectures de la sonde que `thermique` suit.
///
/// Une seconde : c'est la cadence de la télémétrie, et une température de
/// liquide ne bouge pas plus vite.
const INTERVALLE_MESURE: Duration = Duration::from_secs(1);

/// Entre deux dépôts d'état auprès du fil de réparation (#98).
///
/// Une seconde, la cadence des `status` que la fenêtre demande : déposer à celle
/// de la boucle de rendu — jusqu'à trente fois par seconde — n'apprendrait rien
/// de plus, l'état ne changeant qu'au rythme des lectures.
const INTERVALLE_REPARATION: Duration = Duration::from_secs(1);

/// Ce qu'on laisse au périphérique pour revenir avant de redécouvrir ce qu'il
/// porte (#98).
///
/// ⚠️ **Cette durée n'a pas été mesurée**, et rien dans le dépôt ne permet de la
/// mesurer sans réinitialiser un vrai Kraken. Ce qu'on sait : un
/// `USBDEVFS_RESET` fait quitter le bus au périphérique, le noyau le réénumère,
/// puis `nzxt-kraken3` se relie et réenregistre son `hwmon`. Redécouvrir trop tôt
/// ne trouverait rien — et une source sans cible n'est **pas** une source
/// effondrée (voir `reparation::effondree`), ce qui remettrait le compte de
/// tentatives à zéro et ferait boucler la réparation.
///
/// Cinq secondes sont donc une marge, pas une mesure. Elle ne coûte rien : la
/// prochaine tentative n'est de toute façon due que
/// [`reparation::DELAI_ENTRE_TENTATIVES`] plus tard.
const DELAI_DE_REENUMERATION: Duration = Duration::from_secs(5);

/// Ce qu'il reste à faire une fois qu'un reset a réussi, et à partir de quand.
struct Reprise {
    /// La source réinitialisée, par son **nom** — les numéros `hwmonN` ont pu
    /// changer entre-temps, c'est précisément pourquoi on redécouvre.
    source: String,
    /// Quand le périphérique aura eu le temps de revenir.
    quand: Instant,
    /// La poignée usbfs de la dalle a-t-elle été lâchée, et donc à rouvrir ?
    ///
    /// ⚠️ Distinct de « la source est celle du Kraken » : sur une machine où
    /// aucune dalle n'a jamais été ouverte, il n'y a rien à rouvrir, et tenter le
    /// ferait apparaître un écran que personne n'avait.
    rouvrir_l_ecran: bool,
}

fn boucle(peripheriques: &mut Peripheriques, ordres: &Receiver<Ordre>, mut etat: Etat) {
    let mut cadence = Cadence::new(IMAGES_PAR_SECONDE);
    let mut depart = Instant::now();
    let mut animait = false;
    let mut pas: u32 = 0;
    let mut compte = Compteur::default();

    loop {
        while let Ok(ordre) = ordres.try_recv() {
            traiter(ordre, &mut etat, peripheriques);
        }

        // L'horloge de la cadence part avec l'animation, pas avec le processus.
        // Sans ce recalage, la première image d'une animation lancée dix
        // secondes après le démarrage compterait trois cents échéances
        // manquées — un décrochage annoncé qui n'a jamais eu lieu.
        // Une zone animée suffit à faire tourner la boucle : la couche globale
        // peut être fixe pendant que la colonne du radiateur couve.
        // La sonde que `thermique` suit, relue au plus une fois par seconde
        // (#75). Placée avant le calcul de l'image, et hors de `tampon` qui est
        // appelée trente fois par seconde.
        etat.rafraichir_la_mesure();

        // La régulation des canaux que leur pilote ne régule pas (#99), au même
        // endroit et pour la même raison que le reste : c'est le tour que la
        // boucle fait déjà. Aucune minuterie, aucun sondage — sans canal régulé,
        // elle rend la main sans rien lire.
        etat.tour_de_regulation(peripheriques);

        // Puis la réparation d'une source entièrement muette, par un reset USB
        // borné (#98). Ce tour ne fait que décider et ramasser : le geste vit
        // sur son propre fil, comme la dalle depuis #83.
        //
        // ⚠️ **Après la régulation, et non avant.** C'est elle qui vient de lire
        // le liquide, donc de dire si la sonde du Kraken répond encore ; la
        // réparation consomme cette observation, elle ne la produit pas.
        // L'inverse la ferait décider sur un tour de retard.
        tour_de_reparation(&mut etat, peripheriques);

        let anime = etat.animation.is_some() || etat.zone_animee();
        if anime != animait {
            animait = anime;
            if animait {
                cadence = Cadence::new(IMAGES_PAR_SECONDE);
                depart = Instant::now();
                pas = 0;
            }
        }

        let attente = if anime {
            match cadence.tick(depart.elapsed()) {
                Tick::Produire { sautees } => {
                    let tampon = etat.tampon(pas);
                    let debut = Instant::now();
                    ecrire_tampon(peripheriques, &tampon);
                    compte.image(debut.elapsed(), sautees);
                    if !etat.abonnes.is_empty() {
                        let lignes = lignes_tampon(&tampon);
                        serveur::diffuser(&mut etat.abonnes, &lignes);
                    }
                    pas = pas.wrapping_add(1);
                    Duration::ZERO
                }
                Tick::Attendre(delai) => delai,
            }
        } else {
            compte.repos();
            if etat.a_ecrire {
                let tampon = etat.tampon(pas);
                ecrire_tampon(peripheriques, &tampon);
                etat.a_ecrire = false;
                if !etat.abonnes.is_empty() {
                    let lignes = lignes_tampon(&tampon);
                    serveur::diffuser(&mut etat.abonnes, &lignes);
                }
            }
            REPOS
        };

        // L'écran a sa propre horloge : rien de ce qu'il affiche ne tient sans
        // réémission, pas même une image fixe. Elle est **indépendante** de
        // celle de l'éclairage — une dalle qui montre un cadran n'oblige pas
        // les ventilateurs à s'animer, et l'inverse non plus.
        let attente = attente.min(tour_d_ecran(&mut etat, peripheriques));

        if attente > Duration::ZERO {
            match ordres.recv_timeout(attente) {
                Ok(ordre) => traiter(ordre, &mut etat, peripheriques),
                Err(RecvTimeoutError::Timeout) => {}
                // Plus aucun client possible : le fil du socket est mort.
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

/// Pousse une image sur la dalle si son échéance est venue, et dit dans combien
/// de temps revenir.
///
/// Rend [`REPOS`] quand il n'y a rien à afficher : le démon doit être au repos
/// absolu quand la dalle est rendue au firmware.
fn tour_d_ecran(etat: &mut Etat, peripheriques: &mut Peripheriques) -> Duration {
    // ⚠️ **Les verdicts d'abord, avant même l'échéance** (#83). Ils portent sur
    // un envoi décidé à un tour précédent, et peuvent tomber alors que
    // l'échéance a déjà été retirée. Les lire plus bas ferait perdre l'abandon
    // qui suit un `screen off` — la seule ligne de journal qui dise pourquoi la
    // dalle s'est tue.
    if encaisser_les_verdicts(etat, peripheriques) {
        return REPOS;
    }

    let Some(echeance) = etat.echeance_ecran else {
        return REPOS;
    };
    let maintenant = Instant::now();
    if maintenant < echeance {
        return echeance - maintenant;
    }

    // ⚠️ **Les sondes sont relevées avant de toucher à la diffusion.** Elle est
    // empruntée en mutable juste après, et `etat.sondes` ne serait alors plus
    // lisible. Un relevé vide quand la dalle ne porte pas de composition : rien
    // n'est lu, et le cas le plus courant ne coûte rien.
    let champs = champs_du_moment(etat);

    // Le délai jusqu'à l'image suivante est décidé **avant** le dépôt : un
    // envoi qui échoue ne doit pas figer l'horloge, sinon un Kraken débranché
    // ferait tourner cette boucle à plein régime.
    let repli = Duration::from_secs(reverb_proto::screen::REFRESH_INTERVAL_SECS);
    let (image, prochain) = match &mut etat.diffusion {
        Diffusion::Rien => {
            etat.echeance_ecran = None;
            return REPOS;
        }
        Diffusion::Fixe(dalle) => (Some((**dalle).clone()), repli),

        Diffusion::Composee {
            fond,
            derniere,
            dernier_envoi,
        } => {
            let image = ecran::Dalle::composee(fond, &champs);

            // Une composition dont aucun champ ne suit de sonde **ne change
            // jamais**. La repasser toutes les deux secondes pour la comparer à
            // elle-même serait du travail qui ne peut rien découvrir : elle
            // suit alors le rythme d'une image fixe.
            let vivante = champs
                .iter()
                .any(|(_, champ)| matches!(champ, ecran::ChampRendu::Temperature { .. }));
            let prochain = if vivante { CADENCE_CADRAN } else { repli };

            // ⚠️ **On ne pousse que ce qui a changé.** Le protocole n'a aucune
            // mise à jour partielle (spec §2, §3.6) : un dixième de degré coûte
            // 1,2 Mo. À température stable, il n'y a rien à envoyer — et le
            // démon doit alors être au repos, comme il l'est déjà pour
            // l'animation `thermique`.
            //
            // Le repli du firmware, lui, ne se négocie pas : identique ou non,
            // l'image repart avant les trente secondes (spec §2.2.2).
            let repli_du_firmware = dernier_envoi
                .is_none_or(|envoi| maintenant.saturating_duration_since(envoi) >= repli);
            if derniere.as_deref() == Some(&image) && !repli_du_firmware {
                (None, prochain)
            } else {
                *derniere = Some(Box::new(image.clone()));
                *dernier_envoi = Some(maintenant);
                (Some(image), prochain)
            }
        }
        Diffusion::Anime {
            dalles,
            delais,
            rang,
        } => {
            let courant = *rang % dalles.len().max(1);
            let image = dalles.get(courant).cloned();
            let delai = delais.get(courant).copied().unwrap_or(PLANCHER_GIF);
            *rang = (courant + 1) % dalles.len().max(1);
            (image, delai)
        }
        Diffusion::Cadran(sonde) => (Some(cadran_du_moment(sonde, &etat.sondes)), CADENCE_CADRAN),
    };

    // ⚠️ **Déposé, jamais poussé d'ici** (#83). Le `write` de 1 228 800 octets
    // appartient au fil de l'écran ; ce fil-ci anime les LED et sert le socket,
    // et il a payé ce mégaoctet pendant tout #80. Le verdict de #70 reviendra au
    // tour suivant, par `encaisser_les_verdicts`.
    if let Some(image) = image {
        peripheriques.afficher_ecran(image);
    }

    etat.echeance_ecran = Some(maintenant + prochain);
    prochain
}

/// Lit ce que la dalle a dit, sans jamais l'attendre, et le journalise.
///
/// Rend `true` quand l'abandon de #70 vient d'être prononcé : l'appelant repasse
/// alors au repos absolu.
///
/// ⚠️ **L'état persisté n'est pas touché** — ce qu'on voulait afficher reste ce
/// qu'on voulait afficher, et un redémarrage le retente.
fn encaisser_les_verdicts(etat: &mut Etat, peripheriques: &Peripheriques) -> bool {
    let mut abandonne = false;
    for verdict in peripheriques.verdicts_ecran() {
        match verdict {
            ecran::Verdict::Emise | ecran::Verdict::Repos => {}
            ecran::Verdict::Refusee { erreur } => {
                eprintln!("attention : écran : {erreur}");
            }
            ecran::Verdict::Abandon { erreur } => {
                eprintln!(
                    "{} échecs d'affilée sur la dalle : {erreur}\n\
                     → écran rendu au firmware, émission arrêtée (relancer avec « screen … »)",
                    ecran::ECHECS_AVANT_ABANDON
                );
                abandonne = true;
            }
        }
    }
    if abandonne {
        etat.echeance_ecran = None;
    }
    abandonne
}

// ---------------------------------------------------------------------------
// La réparation d'une source muette (issue #98)
// ---------------------------------------------------------------------------

/// Les cibles connues d'une source `hwmon` — ses canaux et ses sondes.
///
/// ⚠️ **Toutes celles qui ont été découvertes**, et non seulement celles qu'on a
/// relevées. C'est ce qui donne son sens au « une seule cible muette ne déclenche
/// rien » : une source dont on n'a lu qu'une cible sur trois n'a pas prouvé son
/// effondrement, et un `USBDEVFS_RESET` sur un contrôleur qui répond encore casse
/// ce qui marchait.
///
/// Conséquence assumée : le démon ne constate l'effondrement que lorsqu'il relève
/// **tout**, c'est-à-dire en servant un `status`. La fenêtre en demande un par
/// seconde, et c'est le régime sous lequel les deux incidents de l'issue ont été
/// observés.
fn cibles_de(etat: &Etat, peripheriques: &Peripheriques, source: &str) -> Vec<String> {
    let mut cibles: Vec<String> = peripheriques
        .canaux()
        .iter()
        .filter(|canal| canal.source == source)
        .map(|canal| canal.name.clone())
        .collect();
    cibles.extend(
        etat.sondes
            .iter()
            .filter(|sonde| sonde.origine == source)
            .map(|sonde| sonde.slug.clone()),
    );
    cibles
}

/// Un tour de réparation : ce qu'on soumet, et ce qu'on encaisse (#98).
///
/// ⚠️ **Rien ici ne réinitialise quoi que ce soit.** Le geste vit sur le fil de
/// [`reverb_daemon::fil_reparation`], qui détient le réparateur seul ; ce fil-ci
/// dépose un état, ramasse un verdict, et ne bloque sur ni l'un ni l'autre. C'est
/// le motif de #83, appliqué au quatrième chemin qui portait le même défaut.
fn tour_de_reparation(etat: &mut Etat, peripheriques: &mut Peripheriques) {
    let maintenant = etat.naissance.elapsed();

    if maintenant >= etat.prochaine_soumission {
        etat.prochaine_soumission = maintenant + INTERVALLE_REPARATION;
        for source in peripheriques.sources_reparables().to_vec() {
            let cibles = cibles_de(etat, peripheriques, &source);
            // ⚠️ **Une source dont on ne connaît aucune cible n'est pas soumise du
            // tout**, et ce n'est pas la même chose que la soumettre à vide.
            //
            // Un reset fait quitter le bus au périphérique : entre la
            // redécouverte et son retour, il n'a plus ni canal ni sonde. Soumis à
            // vide, il rendrait `Rien` — « pas de symptôme » —, ce qui **clôt
            // l'épisode** et remet le compte de tentatives à zéro. Un contrôleur
            // qui met plus longtemps que prévu à revenir serait alors resecoué au
            // premier essai, indéfiniment : la boucle exacte que #98 interdit,
            // atteinte par le chemin qu'elle ne nomme pas.
            //
            // En ne soumettant rien, l'épisode est **gelé** : il reprend au rang
            // suivant si la source revient encore muette, et se clôt normalement
            // dès qu'une de ses cibles répond.
            if cibles.is_empty() {
                continue;
            }
            let muettes = cibles
                .iter()
                .filter(|cible| etat.muettes.contains(*cible))
                .cloned()
                .collect();
            peripheriques.soumettre_reparation(
                EtatSource {
                    source,
                    cibles,
                    muettes,
                },
                maintenant,
            );
        }
    }

    for (source, constat) in peripheriques.constats_reparation() {
        match constat {
            Constat::Reussie { tentative } => {
                eprintln!(
                    "réparation de « {source} » : reset n° {tentative} passé — redécouverte dans \
                     {} s.\n→ « passé » qualifie l'ioctl, pas la guérison : c'est la source qui \
                     répond à nouveau qui le dira",
                    DELAI_DE_REENUMERATION.as_secs()
                );
                let rouvrir_l_ecran = source == SOURCE_DU_KRAKEN && peripheriques.lacher_ecran();
                etat.reprise = Some(Reprise {
                    source,
                    quand: Instant::now() + DELAI_DE_REENUMERATION,
                    rouvrir_l_ecran,
                });
            }
            Constat::Echouee { tentative, erreur } => {
                eprintln!(
                    "attention : réparation de « {source} » : reset n° {tentative} refusé : \
                     {erreur} — rien n'a changé"
                );
            }
            Constat::Abandon => {
                eprintln!(
                    "{TENTATIVES_MAXIMALES} resets USB sans retour de « {source} » : le démon \
                     renonce à la réparer.\n→ ses cibles restent en quarantaine, et sur les \
                     incidents relevés seul un redémarrage l'a ramenée"
                );
            }
            // Le fil ne remonte pas les tours sans geste — ces bras sont là pour
            // que l'ajout d'un verdict ne passe pas inaperçu.
            Constat::Rien | Constat::Patiente | Constat::Repos => {}
        }
    }

    let due = etat
        .reprise
        .as_ref()
        .is_some_and(|reprise| Instant::now() >= reprise.quand);
    if due && let Some(reprise) = etat.reprise.take() {
        reprendre_apres_reset(etat, peripheriques, &reprise);
    }
}

/// Redécouvre ce qu'une source porte, une fois qu'elle a eu le temps de revenir.
///
/// ⚠️ **Les noms survivent, les chemins non.** Un `hwmon` qui disparaît puis
/// revient reçoit le numéro libre — ce que le CLAUDE.md relève déjà des `hidraw`,
/// « constaté, pas supposé », et qu'un reset provoque à volonté. Garder les
/// anciennes poignées, c'est lire le fichier d'un **autre** périphérique et
/// l'afficher sous le nom du premier : une température plausible sous le mauvais
/// nom, et rien pour le signaler.
fn reprendre_apres_reset(etat: &mut Etat, peripheriques: &mut Peripheriques, reprise: &Reprise) {
    let source = &reprise.source;

    match reverb_hw::hwmon::sondes() {
        Ok(sondes) => {
            eprintln!(
                "réparation de « {source} » : {} sonde(s) redécouverte(s)",
                sondes.len()
            );
            etat.sondes = sondes;
        }
        // Les anciennes poignées sont conservées faute de mieux : elles sont
        // peut-être fausses, mais les jeter ferait disparaître toutes les sondes
        // de la machine pour un incident qui n'en concerne qu'une source.
        Err(erreur) => eprintln!(
            "attention : redécouverte des sondes impossible après le reset de « {source} » : \
             {erreur}"
        ),
    }
    match peripheriques.redecouvrir_canaux() {
        Ok(compte) => {
            eprintln!("réparation de « {source} » : {compte} canal/canaux redécouvert(s)")
        }
        Err(erreur) => eprintln!(
            "attention : redécouverte des canaux impossible après le reset de « {source} » : \
             {erreur}"
        ),
    }

    // ⚠️ **Les quarantaines de cette source sont libérées, cible par cible.**
    // Sans cela, la réparation ne servirait à rien pendant cinq minutes : les
    // cibles porteraient encore le délai accumulé avant la panne, et le démon
    // attendrait tout ce temps avant de découvrir que le contrôleur est revenu.
    //
    // ⚠️ **`etat.muettes` n'est pas touché.** Oublier une quarantaine autorise une
    // relecture ; elle ne fait pas répondre la cible. Les vider ici ferait
    // paraître la source revenue au tour suivant, remettrait le compte de
    // tentatives à zéro, et le plafond serait inatteignable.
    for cible in cibles_de(etat, peripheriques, source) {
        etat.quarantaine.oublie(&cible);
    }

    if reprise.rouvrir_l_ecran {
        match peripheriques.rouvrir_ecran() {
            Ok(()) => {
                eprintln!("réparation de « {source} » : poignée usbfs de la dalle rouverte");
                peripheriques.luminosite_ecran(etat.ecran.luminosite);
                if let Err(message) = etat.preparer_ecran() {
                    eprintln!("attention : écran non rétabli : {message}");
                    etat.ecran.affichage = ecran::Affichage::Rien;
                }
            }
            // Dit une fois, puis plus rien : réessayer en boucle d'ouvrir un nœud
            // qui n'existe pas est exactement l'insistance que #70 a bornée. Une
            // commande `screen` ne suffira pas à la rouvrir — il faut redémarrer
            // le service —, et c'est ce que cette ligne doit permettre de
            // comprendre.
            Err(message) => {
                eprintln!(
                    "attention : dalle non rouverte après le reset : {message}\n→ elle reste au \
                     firmware jusqu'au prochain démarrage de reverbd"
                );
                // L'horloge d'émission s'arrête aussi, comme après l'abandon de
                // #70 : composer une image de 1,2 Mo toutes les deux secondes
                // pour n'en déposer nulle part serait du travail pur perte.
                // L'état persisté, lui, n'est pas touché — un redémarrage retente.
                etat.echeance_ecran = None;
            }
        }
    }
}

/// Ce que les champs de la composition montrent à cet instant (#80).
///
/// Vide quand la dalle n'en porte pas : aucune sonde n'est alors lue.
///
/// ⚠️ **Les sondes passent par la quarantaine** (#68). Une composition en lit
/// jusqu'à quatre, toutes les deux secondes ; une lecture sysfs sur un
/// périphérique muet bloque cinq secondes en sommeil non interruptible, et ce
/// fil est celui qui sert aussi le socket et tient les bus (ADR-002). Sans elle,
/// quatre champs sur des sondes mortes gèleraient le service vingt secondes sur
/// deux.
///
/// Une sonde écartée rend `None`, donc des tirets — jamais la dernière valeur
/// connue.
fn champs_du_moment(etat: &mut Etat) -> Vec<(Ancre, ecran::ChampRendu)> {
    let ecran::Affichage::Composition(composition) = &etat.ecran.affichage else {
        return Vec::new();
    };

    let maintenant = etat.naissance.elapsed();
    let sources: Vec<(Ancre, Source)> = composition
        .champs()
        .into_iter()
        .map(|(ancre, source)| (ancre, source.clone()))
        .collect();

    sources
        .into_iter()
        .map(|(ancre, source)| {
            let rendu = match source {
                Source::Texte(texte) => ecran::ChampRendu::Texte(texte),
                Source::Temperature { sonde, libelle } => {
                    // ⚠️ **Deux `None` distincts, et c'est tout l'objet de ce
                    // `Option<Option<_>>`** : la sonde inconnue, qui n'a pas été
                    // relevée du tout, et la sonde relevée qui s'est tue. Les
                    // confondre inscrirait dans les muettes de #98 une cible dont
                    // on n'a rien demandé, et ferait paraître effondrée une source
                    // que personne n'a lue.
                    let releve = match etat.sondes.iter().find(|connue| connue.slug == sonde) {
                        Some(connue) => Some(
                            match etat
                                .quarantaine
                                .tour(&sonde, maintenant, || connue.lire().ok())
                            {
                                Releve::Valeur(millidegres) => Some(millidegres as f32 / 1000.0),
                                Releve::Muette { .. } => None,
                            },
                        ),
                        None => None,
                    };
                    if let Some(valeur) = releve {
                        etat.noter(&sonde, valeur.is_some());
                    }
                    let valeur = releve.flatten();
                    ecran::ChampRendu::Temperature { libelle, valeur }
                }
            };
            (ancre, rendu)
        })
        .collect()
}

/// Le cadran d'une sonde, avec sa valeur du moment.
///
/// Une sonde absente ou muette rend un cadran **sans valeur** : la dalle le dit
/// plutôt que de figer la dernière lecture. Un 34 °C affiché derrière une pompe
/// arrêtée est le mode de défaillance le plus coûteux du cadran, parce qu'il
/// est rassurant.
fn cadran_du_moment(slug: &str, sondes: &[reverb_hw::hwmon::Sonde]) -> ecran::Dalle {
    let sonde = sondes.iter().find(|sonde| sonde.slug == slug);
    let valeur = sonde
        .and_then(|sonde| sonde.lire().ok())
        .map(|millidegres| millidegres as f32 / 1000.0);
    let libelle = sonde.map_or(slug, |sonde| sonde.slug.as_str());
    // La proportion se lit sur une échelle de température de boîtier : zéro
    // degré vide l'anneau, cent le remplit. Ce n'est pas une mesure, c'est une
    // lecture au coup d'œil — la valeur exacte est écrite au centre.
    let proportion = valeur.map_or(0.0, |degres| degres / 100.0);
    ecran::Dalle::cadran(libelle, valeur, "°C", proportion)
}

/// Ce que la boucle de rendu tient réellement, une seconde à la fois.
///
/// Tout ce chantier repose sur une affirmation de performance — une repeinte
/// complète coûte quelques dizaines de millisecondes au lieu de 643. Sans ce
/// relevé, cette affirmation ne serait vérifiable que par une mesure hors du
/// produit, donc jamais après la mise en service.
///
/// Un `sautees` qui ne retombe pas à zéro veut dire que la boucle ne tient pas
/// la cadence : c'est le signal à surveiller, et il est plus honnête qu'un
/// affichage saccadé qu'on met sur le compte de l'écran.
#[derive(Default)]
struct Compteur {
    images: u32,
    sautees: u32,
    cumul: Duration,
    pire: Duration,
    debut: Option<Instant>,
    /// La première seconde d'une animation a déjà été annoncée.
    annonce: bool,
}

impl Compteur {
    fn image(&mut self, duree: Duration, sautees: u32) {
        self.debut.get_or_insert_with(Instant::now);
        self.images += 1;
        self.sautees += sautees;
        self.cumul += duree;
        self.pire = self.pire.max(duree);

        if !self
            .debut
            .is_some_and(|d| d.elapsed() >= Duration::from_secs(1))
        {
            return;
        }

        // Une ligne au démarrage d'une animation, puis plus rien tant que la
        // cadence tient. Journaliser chaque seconde noierait le seul message
        // qui compte — celui qui dit qu'on a décroché.
        if !self.annonce || self.sautees > 0 {
            println!(
                "{} img/s · écriture {:.1} ms en moyenne, {:.1} ms au pire · {} sautée(s)",
                self.images,
                self.cumul.as_secs_f64() * 1000.0 / f64::from(self.images),
                self.pire.as_secs_f64() * 1000.0,
                self.sautees,
            );
        }
        *self = Compteur {
            annonce: true,
            ..Compteur::default()
        };
    }

    /// L'animation s'est arrêtée : le prochain relevé repart de zéro plutôt que
    /// de moyenner une seconde à cheval sur deux régimes, et la prochaine
    /// animation aura droit à sa ligne d'annonce.
    fn repos(&mut self) {
        if self.images > 0 || self.annonce {
            *self = Compteur::default();
        }
    }
}

fn traiter(ordre: Ordre, etat: &mut Etat, peripheriques: &mut Peripheriques) {
    let Ordre {
        requete,
        reponse,
        abonnement,
    } = ordre;
    let lignes = match requete {
        Request::ZoneList => lignes_zones(etat),

        Request::ZoneSet { nom, cibles } => {
            etat.zones.poser(&nom, &cibles);
            etat.a_ecrire = true;
            conserver_zones(etat)
        }
        Request::ZoneDrop { nom } => {
            if etat.zones.retirer(&nom) {
                etat.a_ecrire = true;
                conserver_zones(etat)
            } else {
                vec![zone_inconnue(&nom)]
            }
        }
        Request::ZoneLight { nom, couleur } => {
            if etat.zones.eclairer(&nom, couleur) {
                etat.a_ecrire = true;
                conserver_zones(etat)
            } else {
                vec![zone_inconnue(&nom)]
            }
        }
        Request::ZoneAnim {
            nom,
            animation,
            reglages,
        } => {
            // Le refus d'une animation inconnue passe avant tout : la zone ne
            // doit pas perdre le rendu qu'elle avait parce qu'on a mal tapé le
            // nom de celui qu'on voulait.
            let rendu = match animation {
                None => Ok(None),
                Some(anime) => lancer(&anime, &reglages, &etat.sondes).map(Some),
            };
            match rendu {
                Err(message) => vec![ResponseLine::Error { message }],
                Ok(rendu) => {
                    if etat.zones.animer(&nom, rendu) {
                        etat.a_ecrire = true;
                        conserver_zones(etat)
                    } else {
                        vec![zone_inconnue(&nom)]
                    }
                }
            }
        }
        Request::Status => {
            let maintenant = etat.naissance.elapsed();
            let gpu = etat.gpu.lock().ok().and_then(|lu| lu.clone());
            let mut lignes = telemetrie::releve(
                peripheriques.canaux(),
                peripheriques.courbes_posees(),
                &etat.sondes,
                gpu,
                &mut etat.quarantaine,
                maintenant,
            );
            // ⚠️ **C'est ici que l'effondrement se constate** (#98). Un `status`
            // est le seul moment où le démon relève **tout** ce que la machine
            // porte, et « toutes les cibles d'une source se taisent » ne peut se
            // savoir que de ce qui a été relevé. La fenêtre en demande un par
            // seconde : c'est le régime sous lequel les deux incidents de l'issue
            // ont été observés.
            for ligne in &lignes {
                match ligne {
                    ResponseLine::Channel { channel, .. } => etat.noter(channel, true),
                    ResponseLine::Temp { sensor, .. } => etat.noter(sensor, true),
                    ResponseLine::Unreadable { subject, .. } => etat.noter(subject, false),
                    _ => {}
                }
            }
            lignes.push(ResponseLine::End);
            lignes
        }

        Request::Light { target, color } => {
            appliquer_couleur(etat, target, color);
            // Une couleur fixe arrête l'animation : les deux se disputeraient
            // les mêmes LED, et la dernière écriture gagnerait au hasard.
            etat.animation = None;
            etat.a_ecrire = true;
            conserver(etat)
        }

        Request::Animate { name, reglages } => match name {
            None => {
                etat.animation = None;
                // L'éclairage fixe reprend la main là où l'animation l'a laissé.
                etat.a_ecrire = true;
                conserver(etat)
            }
            Some(nom) => match lancer(&nom, &reglages, &etat.sondes) {
                Ok(anime) => {
                    etat.animation = Some(anime);
                    conserver(etat)
                }
                Err(message) => vec![ResponseLine::Error { message }],
            },
        },

        Request::Geometry { cible, reglages } => geometrie(etat, cible.as_deref(), &reglages),

        Request::Paint { target, couleurs } => {
            peindre(etat, target, &couleurs);
            // Une peinture arrête l'animation, comme une couleur fixe : les
            // deux se disputeraient les mêmes LED.
            etat.animation = None;
            etat.a_ecrire = true;
            conserver(etat)
        }

        Request::Lighting => etat_eclairage(etat),

        Request::Screen(action) => ecran_commande(etat, peripheriques, action),

        Request::Profil(action) => profil_commande(etat, peripheriques, action),

        Request::Watch => {
            if let Some(canal) = abonnement {
                // La première image part **tout de suite** quand rien n'est
                // animé : sans elle, une fenêtre qui vient de s'ouvrir sur un
                // boîtier en couleur fixe resterait vide jusqu'à la prochaine
                // commande. Sous animation, la suivante arrive dans 50 ms.
                if etat.animation.is_none() {
                    let _ = canal.try_send(lignes_tampon(&etat.tampon(0)));
                }
                etat.abonnes.push(canal);
            }
            // Rien à répondre : le premier octet que l'abonné recevra est une
            // image, pas un accusé de réception.
            Vec::new()
        }

        Request::Fan { channel, action } => {
            // `Percent::new` refait la vérification de bornes que `parse_request`
            // a déjà faite. Ce n'est pas redondant pour rien : le socket ne doit
            // pas être une porte dérobée qui contourne les garde-fous de la
            // ligne de commande, et c'est ce type qui les porte.
            let consigne = match action {
                FanAction::Auto => Ok(Consigne::Auto),
                FanAction::Pwm(percent) => Percent::new(percent).map(Consigne::Pwm),
            };
            let ecrite = consigne.map_err(|e| e.to_string()).and_then(|consigne| {
                peripheriques
                    .consigner(&channel, consigne)
                    .map_err(|e| e.to_string())
            });

            // Une consigne posée à la main **reprend** le canal : sans cela la
            // régulation la réécrirait au prochain changement de palier, et
            // l'utilisateur verrait sa valeur disparaître sans explication.
            // Après un échec, en revanche, le canal reste régulé — le rendre
            // laisserait les ventilateurs à leur défaut d'allumage.
            if ecrite.is_ok() && etat.regulation.canaux().contains(&channel) {
                etat.regulation.couper(&channel);
                if let Err(erreur) =
                    regulation::enregistrer_regulation(&etat.fichiers.regulation, &etat.regulation)
                {
                    eprintln!("attention : régulation coupée mais non conservée : {erreur}");
                }
                println!("régulation : « {channel} » repris à la main par « fan »");
            }

            match ecrite {
                Ok(()) => vec![ResponseLine::End],
                // ⚠️ Le refus vient de `reverb-hw`, qui ne connaît pas le
                // socket : « ce contrôleur n'a pas de mode automatique » (#50)
                // est exact et n'apprend pas ce qu'il faut taper à la place.
                // Le panneau indicateur est posé ici, où le protocole est connu.
                Err(message) if matches!(action, FanAction::Auto) => {
                    vec![ResponseLine::Error {
                        message: format!(
                            "{message} — « regule {channel} on » le régule depuis l'hôte, sur la \
                             température du liquide"
                        ),
                    }]
                }
                Err(message) => vec![ResponseLine::Error { message }],
            }
        }

        Request::Regule(action) => regule_commande(etat, peripheriques, action),
    };

    // Le client peut être parti entre l'ordre et la réponse : ce n'est pas une
    // erreur du démon.
    let _ = reponse.send(lignes);
}

/// L'état d'éclairage courant, tel que `lighting` le rend.
///
/// Les couleurs **fixes**, celles que `animate off` rendrait, et non ce qui est
/// affiché à l'instant : une fenêtre doit pouvoir montrer les réglages sous
/// l'animation, sinon les arrêter ferait apparaître des couleurs qu'elle
/// n'avait jamais montrées.
fn etat_eclairage(etat: &Etat) -> Vec<ResponseLine> {
    let mut lignes: Vec<ResponseLine> = Position::ALL
        .into_iter()
        .map(|position| ResponseLine::Light {
            cible: format!("fan:{}", position.slug()),
            couleur: etat.ventilateurs[position.index()],
        })
        .collect();
    for (slot, couleur) in etat.barrettes.iter().enumerate() {
        lignes.push(ResponseLine::Light {
            cible: format!("slot:{slot}"),
            couleur: *couleur,
        });
    }
    if let Some((animation, reglages)) = &etat.animation {
        lignes.push(ResponseLine::Anim {
            nom: animation.nom().to_owned(),
            reglages: animation.reglages_ecrits(reglages),
        });
    }
    lignes.push(ResponseLine::End);
    lignes
}

/// Ce que le boîtier montre, sous la forme qu'un abonné attend.
///
/// Un abonné ne connaît qu'un seul format : les couleurs du moment. Qu'elles
/// viennent d'une animation, d'une couleur posée à la main ou d'une zone ne le
/// regarde pas.
fn lignes_tampon(tampon: &Tampon) -> Vec<ResponseLine> {
    let mut lignes: Vec<ResponseLine> = Position::ALL
        .into_iter()
        .map(|position| ResponseLine::Frame {
            cible: format!("fan:{}", position.slug()),
            couleurs: tampon.ventilateurs[position.index()].to_vec(),
        })
        .collect();
    for (slot, couleurs) in tampon.barrettes.iter().enumerate() {
        lignes.push(ResponseLine::Frame {
            cible: format!("slot:{slot}"),
            couleurs: couleurs.to_vec(),
        });
    }
    lignes.push(ResponseLine::End);
    lignes
}

/// Les zones, telles que `zone list` les rend.
///
/// ⚠️ Les cibles d'une zone sont réparties sur **plusieurs lignes** quand il le
/// faut : une zone couvrant les cent vingt-quatre LED pèse près de deux mille
/// octets, contre `MAX_LINE_LEN = 1024`. Le client accumule par nom.
fn lignes_zones(etat: &Etat) -> Vec<ResponseLine> {
    let mut lignes = Vec::new();
    for zone in etat.zones.liste() {
        for paquet in zone.cibles.chunks(CIBLES_PAR_LIGNE) {
            lignes.push(ResponseLine::Zone {
                nom: zone.nom.clone(),
                cibles: paquet.to_vec(),
            });
        }
        match &zone.rendu {
            Rendu::Transparente => {}
            Rendu::Fixe(couleur) => lignes.push(ResponseLine::ZoneLight {
                nom: zone.nom.clone(),
                couleur: *couleur,
            }),
            Rendu::Animee(animation, reglages) => lignes.push(ResponseLine::ZoneAnim {
                nom: zone.nom.clone(),
                animation: animation.nom().to_owned(),
                reglages: animation.reglages_ecrits(reglages),
            }),
        }
    }
    lignes.push(ResponseLine::End);
    lignes
}

/// Combien de cibles tiennent sur une ligne `zone`.
///
/// La plus longue cible fait vingt-quatre octets (`fan:radiateur-milieu:7`), le
/// nom au plus quarante-huit : quarante cibles laissent une marge confortable
/// sous les 1024 octets que le protocole s'impose.
const CIBLES_PAR_LIGNE: usize = 40;

/// Écrit l'éclairage courant sur disque, et répond.
///
/// À **chaque** changement d'état, et non à l'arrêt du démon : ce qu'on veut
/// retrouver, c'est précisément l'éclairage d'avant une coupure de courant ou
/// un arrêt au bouton, qui ne laissent pas le temps d'écrire quoi que ce soit.
/// Le débit reste celui des commandes humaines — une animation qui tourne à
/// 21 img/s ne change pas d'état, elle avance.
///
/// Un échec d'écriture est dit au client plutôt qu'avalé : la couleur est bien
/// appliquée, mais elle ne survivra pas au redémarrage, et c'est exactement la
/// panne que cette issue corrige.
fn conserver(etat: &Etat) -> Vec<ResponseLine> {
    match persistance::enregistrer_eclairage(&etat.fichiers.eclairage, &etat.eclairage()) {
        Ok(()) => vec![ResponseLine::End],
        Err(erreur) => vec![ResponseLine::Error {
            message: format!(
                "éclairage appliqué mais non conservé : {} ({erreur})",
                etat.fichiers.eclairage.display()
            ),
        }],
    }
}

/// Ce qu'une commande `screen` fait de la dalle.
///
/// ⚠️ **Rien n'est appliqué avant que tout soit accepté.** Une image d'un format
/// inconnu est refusée sans que la dalle change — c'est le critère
/// d'acceptation, et c'est aussi ce qui distingue « refuser » d'« éteindre en
/// silence ».
fn ecran_commande(
    etat: &mut Etat,
    peripheriques: &mut Peripheriques,
    action: ScreenAction,
) -> Vec<ResponseLine> {
    // ⚠️ **Une commande explicite relance l'émission** (#70). Après un abandon,
    // c'est le seul moyen de reprendre : le démon ne réessaie plus de lui-même,
    // et une dalle qui ne répond plus au noyau ne se répare pas en insistant.
    // `State` est exclue — la fenêtre l'envoie chaque seconde, et relancer là
    // rendrait le plafond inopérant.
    if !matches!(action, ScreenAction::State) {
        peripheriques.relancer_ecran();
    }

    match action {
        // La composition part avec l'état, et non sur une seconde demande : une
        // fenêtre qui interroge la dalle chaque seconde doublerait sinon son
        // trafic pour un détail qu'elle affiche au même endroit.
        ScreenAction::State => {
            let mut lignes = vec![etat.ligne_ecran()];
            lignes.extend(etat.lignes_composition());
            lignes.push(ResponseLine::End);
            lignes
        }

        ScreenAction::Brightness(pourcent) => {
            // ⚠️ **Plus de refus rendu au client** (#83). L'écriture a lieu sur
            // le fil de la dalle ; en attendre le résultat rendrait au client
            // une erreur qu'il ne peut de toute façon pas corriger, au prix des
            // 51 ms d'ouverture d'un `hidraw` payées par la boucle des LED. Un
            // refus est journalisé par le verdict, comme celui d'une image.
            peripheriques.luminosite_ecran(pourcent);
            etat.ecran.luminosite = pourcent;
            // ⚠️ `30 02` réinitialise le pipeline d'affichage (spec §3.4) :
            // l'image doit repartir tout de suite derrière, sinon la dalle
            // clignote vers le firmware et y reste.
            if !matches!(etat.diffusion, Diffusion::Rien) {
                etat.echeance_ecran = Some(Instant::now());
            }
            conserver_ecran(etat)
        }

        ScreenAction::Off => {
            etat.ecran.affichage = ecran::Affichage::Rien;
            etat.diffusion = Diffusion::Rien;
            etat.echeance_ecran = None;
            // Aucune trame ne ramène au mode firmware : **cesser d'émettre est
            // le seul moyen connu d'y revenir** (spec §2.3). La dalle reprend
            // donc son affichage d'origine dans la trentaine de secondes.
            conserver_ecran(etat)
        }

        ScreenAction::Image(chemin) => poser_affichage(etat, ecran::Affichage::Image(chemin)),
        ScreenAction::Gif(chemin) => poser_affichage(etat, ecran::Affichage::Gif(chemin)),

        ScreenAction::LayoutState => {
            let mut lignes = etat.lignes_composition();
            lignes.push(ResponseLine::End);
            lignes
        }

        ScreenAction::LayoutOff => {
            // Ce que la composition montrait, elle le rend : son fond. Une
            // composition qu'on quitte en éteignant la dalle ferait disparaître
            // la photo qu'on avait posée dessous, ce que personne ne demande en
            // tapant « off » sur les champs.
            let affichage = match &etat.ecran.affichage {
                ecran::Affichage::Composition(composition) => match composition.fond() {
                    Fond::Image(chemin) => ecran::Affichage::Image(chemin.clone()),
                    Fond::Noir => ecran::Affichage::Rien,
                },
                // Pas de composition : rien à quitter, et rien ne bouge.
                autre => autre.clone(),
            };
            poser_affichage(etat, affichage)
        }

        ScreenAction::LayoutFond(fond) => {
            let mut composition = etat.composition_courante();
            composition.changer_fond(fond);
            poser_affichage(etat, ecran::Affichage::Composition(composition))
        }

        ScreenAction::LayoutChamp(ancre, source) => {
            // Une sonde inconnue est refusée **ici** : le démon est seul à
            // savoir lesquelles la machine expose, et un champ qui afficherait
            // « --- » pour une faute de frappe se lirait comme une sonde en
            // panne — la même règle que le cadran.
            if let Source::Temperature { sonde, .. } = &source
                && !etat.sondes.iter().any(|connue| connue.slug == *sonde)
            {
                return vec![ResponseLine::Error {
                    message: format!(
                        "sonde « {sonde} » inconnue — « status » donne celles de la machine"
                    ),
                }];
            }
            let mut composition = etat.composition_courante();
            if let Err(erreur) = composition.poser(ancre, source) {
                return vec![ResponseLine::Error {
                    message: erreur.to_string(),
                }];
            }
            poser_affichage(etat, ecran::Affichage::Composition(composition))
        }

        ScreenAction::LayoutVide(ancre) => {
            let ecran::Affichage::Composition(composition) = &etat.ecran.affichage else {
                return vec![ResponseLine::Error {
                    message: "la dalle ne porte pas de composition : rien à vider".to_owned(),
                }];
            };
            let mut composition = composition.clone();
            if !composition.vider(ancre) {
                return vec![ResponseLine::Error {
                    message: format!("l'ancre « {ancre} » ne porte aucun champ"),
                }];
            }
            poser_affichage(etat, ecran::Affichage::Composition(composition))
        }
        ScreenAction::Gauge(sonde) => {
            // Une sonde inconnue est refusée **ici**, pas sur la dalle : un
            // cadran qui afficherait « --- » pour une faute de frappe se lirait
            // comme une sonde en panne.
            if !etat.sondes.iter().any(|connue| connue.slug == sonde) {
                return vec![ResponseLine::Error {
                    message: format!(
                        "sonde « {sonde} » inconnue — « status » donne celles de la machine"
                    ),
                }];
            }
            poser_affichage(etat, ecran::Affichage::Cadran(sonde))
        }
    }
}

/// Change ce que la dalle montre, en refusant sans rien casser.
fn poser_affichage(etat: &mut Etat, affichage: ecran::Affichage) -> Vec<ResponseLine> {
    // ⚠️ **Le format se vérifie avant que quoi que ce soit bouge** (#69). Un GIF
    // déclaré sur un `.jpg` était accepté, persisté, et rejoué à chaque
    // démarrage du service : le démon repartait dans un état cassé sans moyen
    // d'en sortir seul. La vérification lit un en-tête, pas un fichier entier.
    if let Err(erreur) = ecran::verifier_fichier(&affichage) {
        return vec![ResponseLine::Error {
            message: erreur.to_string(),
        }];
    }

    let precedent = std::mem::replace(&mut etat.ecran.affichage, affichage);
    match etat.preparer_ecran() {
        Ok(()) => conserver_ecran(etat),
        Err(message) => {
            // Le refus remet **tout** en place, y compris ce qui était en train
            // de tourner : une image illisible ne doit pas éteindre le cadran
            // qu'elle remplaçait.
            etat.ecran.affichage = precedent;
            let _ = etat.preparer_ecran();
            vec![ResponseLine::Error { message }]
        }
    }
}

/// Ce qu'une commande `regule` fait de la régulation côté hôte (#99).
///
/// ⚠️ **Un canal dont le pilote sait faire auto est refusé.** Les deux verbes se
/// partagent les canaux : `fan <canal> auto` pour ceux dont le firmware exécute
/// une courbe — les deux du Kraken, qui régulent déjà correctement, mesuré —, et
/// `regule` pour les autres. Remplacer une régulation qui marche par une boucle
/// hôte, c'est écrire du code pour perdre une garantie : la courbe firmware,
/// elle, tient sans démon.
fn regule_commande(
    etat: &mut Etat,
    peripheriques: &Peripheriques,
    action: ReguleAction,
) -> Vec<ResponseLine> {
    let echec = |message: String| vec![ResponseLine::Error { message }];

    match action {
        ReguleAction::Etat => {
            let mut lignes = vec![ResponseLine::Courbe {
                paliers: etat.regulation.courbe().paliers().to_vec(),
            }];
            lignes.extend(
                etat.regulation
                    .canaux()
                    .into_iter()
                    .map(|canal| ResponseLine::Regule { canal }),
            );
            lignes.push(ResponseLine::End);
            lignes
        }

        ReguleAction::Activer(canal) => {
            let Some(connu) = peripheriques.canaux().iter().find(|c| c.name == canal) else {
                return echec(format!(
                    "canal « {canal} » inconnu — « status » donne ceux de la machine"
                ));
            };
            if connu.sait_faire_auto() {
                return echec(format!(
                    "le pilote de « {canal} » exécute déjà sa propre courbe : « fan {canal} auto » \
                     la lui rend, et elle tient sans démon"
                ));
            }
            etat.regulation.activer(&canal);
            // Le canal est écrit au prochain tour de boucle, dans les
            // millisecondes : il n'a jamais rien reçu, et le cache est par canal.
            etat.prochaine_regulation = Duration::ZERO;
            conserver_regulation(etat)
        }

        ReguleAction::Couper(canal) => {
            if !etat.regulation.canaux().contains(&canal) {
                return echec(format!(
                    "canal « {canal} » non régulé — « regule » donne ceux qui le sont"
                ));
            }
            // ⚠️ Le canal garde la dernière consigne écrite : rien n'a de
            // watchdog, et le rendre au pilote demanderait un mode automatique
            // qu'il n'a pas — c'est tout le sujet de #99. `fan <canal> pwm …`
            // reprend la main.
            etat.regulation.couper(&canal);
            conserver_regulation(etat)
        }

        ReguleAction::Courbe(paliers) => {
            // Le refus vient de la `Courbe` elle-même, seule à juger : le
            // protocole transporte les paliers sans les interpréter, comme les
            // réglages d'une animation.
            match Courbe::depuis(&paliers) {
                Ok(courbe) => {
                    etat.regulation.regler(courbe);
                    etat.prochaine_regulation = Duration::ZERO;
                    conserver_regulation(etat)
                }
                Err(erreur) => echec(erreur.raison),
            }
        }
    }
}

/// Écrit la régulation sur disque, et répond.
///
/// Même règle que l'éclairage : à **chaque** changement, et un échec est dit au
/// client plutôt qu'avalé. Une régulation appliquée mais non conservée ne
/// survivrait pas au redémarrage, et les sept ventilateurs repartiraient à leur
/// défaut d'allumage — le défaut même que #99 corrige.
fn conserver_regulation(etat: &Etat) -> Vec<ResponseLine> {
    match regulation::enregistrer_regulation(&etat.fichiers.regulation, &etat.regulation) {
        Ok(()) => vec![ResponseLine::End],
        Err(erreur) => vec![ResponseLine::Error {
            message: format!(
                "régulation appliquée mais non conservée : {} ({erreur})",
                etat.fichiers.regulation.display()
            ),
        }],
    }
}

/// Ce qu'une commande `profil` fait des ambiances nommées (#74).
///
/// ⚠️ **Aucun second chemin d'écriture vers le matériel.** Rappeler une ambiance
/// repose l'état puis, pour l'écran, repasse par [`ecran_commande`] — le seul
/// endroit qui décode, met à l'échelle et pousse. Un profil qui écrirait
/// lui-même sur la dalle doublerait un chemin dont #69 et #70 ont montré le prix.
fn profil_commande(
    etat: &mut Etat,
    peripheriques: &mut Peripheriques,
    action: ProfilAction,
) -> Vec<ResponseLine> {
    let repertoire = etat.fichiers.profils.clone();

    match action {
        ProfilAction::List => {
            let mut lignes: Vec<ResponseLine> = profils::lister(&repertoire)
                .into_iter()
                .map(|nom| ResponseLine::Profil {
                    etat: "connu".to_owned(),
                    nom: nom.as_str().to_owned(),
                })
                .collect();
            lignes.push(ResponseLine::End);
            lignes
        }

        ProfilAction::Save(nom) => {
            // L'écran fait toujours partie de l'instantané : un profil qui ne
            // dirait rien de la dalle serait celui d'un boîtier à moitié.
            let profil = Profil {
                eclairage: etat.eclairage(),
                zones: etat.zones.clone(),
                ecran: Some(etat.ecran.clone()),
            };
            match profils::enregistrer(&repertoire, &nom, &profil) {
                Ok(ecriture) => vec![
                    ResponseLine::Profil {
                        etat: match ecriture {
                            Ecriture::Creee => "cree".to_owned(),
                            Ecriture::Ecrasee => "ecrase".to_owned(),
                        },
                        nom: nom.as_str().to_owned(),
                    },
                    ResponseLine::End,
                ],
                Err(erreur) => vec![ResponseLine::Error {
                    message: format!("profil « {nom} » non enregistré : {erreur}"),
                }],
            }
        }

        ProfilAction::Drop(nom) => match profils::oublier(&repertoire, &nom) {
            Ok(()) => vec![
                ResponseLine::Profil {
                    etat: "oublie".to_owned(),
                    nom: nom.as_str().to_owned(),
                },
                ResponseLine::End,
            ],
            Err(erreur) => vec![ResponseLine::Error {
                message: erreur.to_string(),
            }],
        },

        ProfilAction::Load(nom) => {
            let profil = match profils::charger(&repertoire, &nom) {
                Ok(profil) => profil,
                Err(erreur) => {
                    return vec![ResponseLine::Error {
                        message: erreur.to_string(),
                    }];
                }
            };
            appliquer_profil(etat, peripheriques, &nom.to_string(), profil.preparer())
        }
    }
}

/// Repose l'éclairage, les zones et l'écran d'une ambiance rappelée.
///
/// ⚠️ **Ce qui s'applique s'applique, même si l'écran ne suit pas.** Un profil à
/// moitié appliqué qui le dit vaut mieux qu'un profil refusé en bloc parce
/// qu'une photo a été déplacée. Les manques passent en `unreadable` — la ligne
/// que ce protocole réserve à ce qui existe mais n'a pas pu être lu — et non en
/// `err`, qui terminerait la réponse en échec alors que le boîtier vient bel et
/// bien de changer de couleur.
fn appliquer_profil(
    etat: &mut Etat,
    peripheriques: &mut Peripheriques,
    nom: &str,
    application: profils::Application,
) -> Vec<ResponseLine> {
    let mut lignes: Vec<ResponseLine> = application
        .signalements
        .into_iter()
        .map(|raison| ResponseLine::Unreadable {
            subject: "ecran".to_owned(),
            reason: raison,
        })
        .collect();

    etat.ventilateurs = application.eclairage.ventilateurs;
    etat.barrettes = application.eclairage.barrettes;
    etat.animation = application.eclairage.animation;
    // La couche globale reprend une couleur unie par cible : `eclairage.conf`
    // n'a jamais gardé plus fin (#21), et un profil non plus.
    etat.fixe = Tampon {
        ventilateurs: application
            .eclairage
            .ventilateurs
            .map(|couleur| [couleur; LEDS_PER_FAN as usize]),
        barrettes: application
            .eclairage
            .barrettes
            .map(|couleur| [couleur; ram::LEDS_PER_STICK]),
    };
    etat.zones = application.zones;
    etat.a_ecrire = true;

    if let Some(dalle) = application.ecran {
        let mut actions = vec![ScreenAction::Brightness(dalle.luminosite)];
        match &dalle.affichage {
            ecran::Affichage::Rien => actions.push(ScreenAction::Off),
            ecran::Affichage::Cadran(sonde) => actions.push(ScreenAction::Gauge(sonde.clone())),
            ecran::Affichage::Image(chemin) => actions.push(ScreenAction::Image(chemin.clone())),
            ecran::Affichage::Gif(chemin) => actions.push(ScreenAction::Gif(chemin.clone())),
            // Une composition ne tient pas en une action : elle se rejoue en
            // plusieurs, et toutes passent par le même chemin d'écriture.
            //
            // ⚠️ **Le `off` de tête n'est pas décoratif.** Un profil est un
            // instantané, pas un ajout : sans lui, les champs de l'ambiance
            // qu'on quitte survivraient à celle qu'on rappelle, aux ancres que
            // la nouvelle laisse libres. Il ne touche aucun fichier, donc il ne
            // peut pas échouer — ce qu'un retour au fond précédent, lui, ferait
            // si l'image avait été déplacée depuis.
            ecran::Affichage::Composition(composition) => {
                actions.push(ScreenAction::Off);
                actions.push(ScreenAction::LayoutFond(composition.fond().clone()));
                actions.extend(
                    composition
                        .champs()
                        .into_iter()
                        .map(|(ancre, source)| ScreenAction::LayoutChamp(ancre, source.clone())),
                );
            }
        }
        for action in actions {
            for ligne in ecran_commande(etat, peripheriques, action) {
                if let ResponseLine::Error { message } = ligne {
                    lignes.push(ResponseLine::Unreadable {
                        subject: "ecran".to_owned(),
                        reason: message,
                    });
                }
            }
        }
    }

    for souci in [
        persistance::enregistrer_eclairage(&etat.fichiers.eclairage, &etat.eclairage())
            .err()
            .map(|erreur| format!("éclairage appliqué mais non conservé : {erreur}")),
        zones::enregistrer(&etat.fichiers.zones, &etat.zones)
            .err()
            .map(|erreur| format!("zones appliquées mais non conservées : {erreur}")),
    ]
    .into_iter()
    .flatten()
    {
        lignes.push(ResponseLine::Unreadable {
            subject: "disque".to_owned(),
            reason: souci,
        });
    }

    lignes.push(ResponseLine::Profil {
        etat: "applique".to_owned(),
        nom: nom.to_owned(),
    });
    lignes.push(ResponseLine::End);
    lignes
}

/// Écrit l'état de l'écran sur disque, et répond.
fn conserver_ecran(etat: &Etat) -> Vec<ResponseLine> {
    match ecran::enregistrer(&etat.fichiers.ecran, &etat.ecran) {
        Ok(()) => vec![ResponseLine::End],
        Err(erreur) => vec![ResponseLine::Error {
            message: format!(
                "écran appliqué mais non conservé : {} ({erreur})",
                etat.fichiers.ecran.display()
            ),
        }],
    }
}

/// Le refus d'une zone qu'on n'a jamais créée.
///
/// Nommer la zone plutôt que dire « inconnue » : sur un fichier qui en porte
/// plusieurs, la faute de frappe se voit tout de suite.
fn zone_inconnue(nom: &str) -> ResponseLine {
    ResponseLine::Error {
        message: format!("zone « {nom} » inconnue — « zone list » les donne toutes"),
    }
}

/// Écrit les zones sur disque, et répond.
///
/// Même règle que pour l'éclairage : à **chaque** changement, et un échec est
/// dit au client plutôt qu'avalé.
fn conserver_zones(etat: &Etat) -> Vec<ResponseLine> {
    match zones::enregistrer(&etat.fichiers.zones, &etat.zones) {
        Ok(()) => vec![ResponseLine::End],
        Err(erreur) => vec![ResponseLine::Error {
            message: format!(
                "zone appliquée mais non conservée : {} ({erreur})",
                etat.fichiers.zones.display()
            ),
        }],
    }
}

/// Ouvre une animation du catalogue et valide ses réglages.
///
/// Les deux refus portent leur propre message : `reverb-anim` cite le
/// catalogue pour un nom inconnu, et les clés acceptées pour un réglage
/// fautif. Le démon n'a rien à y ajouter — il les répète tels quels, ce qui
/// garantit que le socket et la ligne de commande disent la même chose.
fn lancer(
    nom: &str,
    reglages: &[(String, String)],
    sondes: &[reverb_hw::hwmon::Sonde],
) -> Result<(Animation, Reglages), String> {
    let animation = Animation::par_nom(nom).map_err(|erreur| erreur.to_string())?;
    let reglages = animation
        .reglages(reglages)
        .map_err(|erreur| erreur.to_string())?;

    // ⚠️ **Le seul refus que `reverb-anim` ne peut pas porter** (#75). Ce crate
    // est pur : il sait que `thermique` veut une sonde, il ne sait pas
    // lesquelles existent. Le démon, lui, les a découvertes au démarrage.
    //
    // Sans ce refus, une faute de frappe lancerait une animation condamnée au
    // blanc pulsant, que rien ne distinguerait d'une sonde en panne.
    if let Some(voulue) = &reglages.sonde
        && !sondes.iter().any(|sonde| sonde.slug == *voulue)
    {
        let mut connues: Vec<&str> = sondes.iter().map(|sonde| sonde.slug.as_str()).collect();
        connues.sort_unstable();
        return Err(format!(
            "sonde « {voulue} » inconnue. Sondes de cette machine : {}",
            if connues.is_empty() {
                "aucune n'a été découverte".to_owned()
            } else {
                connues.join(", ")
            }
        ));
    }
    Ok((animation, reglages))
}

/// Lit ou modifie la géométrie.
///
/// Sans cible, c'est une lecture — dix lignes puis `end`. Avec une cible et
/// des réglages, c'est une écriture, immédiatement persistée : « elle survit à
/// `systemctl restart reverbd` » est un critère d'acceptation, pas un effet de
/// bord souhaitable.
fn geometrie(
    etat: &mut Etat,
    cible: Option<&str>,
    reglages: &[(String, String)],
) -> Vec<ResponseLine> {
    let echec = |message: String| vec![ResponseLine::Error { message }];

    let Some(cible) = cible else {
        if !reglages.is_empty() {
            // Un réglage sans cible ne désigne rien : l'appliquer aux dix
            // ventilateurs effacerait d'un coup une mesure qui a coûté un
            // passage sous le bureau.
            return echec(
                "« geometry » attend un ventilateur avant ses réglages, par exemple « geometry \
                 radiateur-haut angle=90 »"
                    .to_owned(),
            );
        }
        let mut lignes: Vec<ResponseLine> = Position::ALL
            .into_iter()
            .map(|position| ligne_geom(etat, position))
            .collect();
        lignes.push(ResponseLine::End);
        return lignes;
    };

    let position = match Position::from_slug(cible) {
        Ok(position) => position,
        Err(erreur) => return echec(erreur.to_string()),
    };

    if reglages.is_empty() {
        return vec![ligne_geom(etat, position), ResponseLine::End];
    }

    // Les deux champs se règlent séparément : corriger un angle ne doit pas
    // obliger à retaper un sens qui n'a pas changé.
    let courante = etat.geometrie.orientation(position);
    let mut angle = courante.angle;
    let mut sens = courante.sens;
    for (cle, valeur) in reglages {
        match cle.as_str() {
            "angle" => match valeur.parse() {
                Ok(degres) => angle = degres,
                Err(_) => {
                    return echec(format!(
                        "réglage « angle » : « {valeur} » n'est pas un nombre entier de degrés"
                    ));
                }
            },
            "sens" => match valeur.as_str() {
                "horaire" => sens = Sens::Horaire,
                "antihoraire" => sens = Sens::Antihoraire,
                _ => {
                    return echec(format!(
                        "réglage « sens » : « {valeur} » n'est ni « horaire » ni « antihoraire »"
                    ));
                }
            },
            autre => {
                return echec(format!(
                    "réglage « {autre} » inconnu. Réglages de « geometry » : angle, sens"
                ));
            }
        }
    }

    let orientation = match Orientation::new(angle, sens) {
        Ok(orientation) => orientation,
        Err(erreur) => return echec(erreur.to_string()),
    };
    etat.geometrie.definir(position, orientation);

    if let Err(erreur) =
        persistance::enregistrer_geometrie(&etat.fichiers.geometrie, &etat.geometrie)
    {
        // L'orientation est appliquée en mémoire mais ne survivra pas : le dire
        // plutôt que de laisser croire à un réglage acquis.
        return echec(format!(
            "orientation appliquée mais non conservée : {} ({erreur})",
            etat.fichiers.geometrie.display()
        ));
    }
    vec![ResponseLine::End]
}

fn ligne_geom(etat: &Etat, position: Position) -> ResponseLine {
    let orientation = etat.geometrie.orientation(position);
    ResponseLine::Geom {
        position: position.slug(),
        angle: orientation.angle,
        sens: orientation.sens.slug().to_owned(),
    }
}

fn appliquer_couleur(etat: &mut Etat, cible: LightTarget, couleur: Rgb) {
    match cible {
        LightTarget::All => {
            etat.ventilateurs = [couleur; 10];
            etat.barrettes = [couleur; ram::SLOT_COUNT];
        }
        LightTarget::Fans => etat.ventilateurs = [couleur; 10],
        LightTarget::Fan(position) => etat.ventilateurs[position.index()] = couleur,
        LightTarget::Ram => etat.barrettes = [couleur; ram::SLOT_COUNT],
        LightTarget::RamSlot(slot) => etat.barrettes[slot] = couleur,
    }
    // Une couleur de cible efface la peinture de ses LED : c'est ce qui rend
    // « repose une couleur unie » possible après avoir peint LED par LED.
    match cible {
        LightTarget::All => {
            etat.fixe.ventilateurs = [[couleur; LEDS_PER_FAN as usize]; 10];
            etat.fixe.barrettes = [[couleur; ram::LEDS_PER_STICK]; ram::SLOT_COUNT];
        }
        LightTarget::Fans => etat.fixe.ventilateurs = [[couleur; LEDS_PER_FAN as usize]; 10],
        LightTarget::Fan(position) => {
            etat.fixe.ventilateurs[position.index()] = [couleur; LEDS_PER_FAN as usize];
        }
        LightTarget::Ram => {
            etat.fixe.barrettes = [[couleur; ram::LEDS_PER_STICK]; ram::SLOT_COUNT];
        }
        LightTarget::RamSlot(slot) => {
            etat.fixe.barrettes[slot] = [couleur; ram::LEDS_PER_STICK];
        }
    }
}

/// Pose une couleur par LED sur une cible unique.
///
/// Le protocole a déjà vérifié le compte contre le matériel ; ce qui arrive ici
/// tient dans la cible.
fn peindre(etat: &mut Etat, cible: LightTarget, couleurs: &[Rgb]) {
    match cible {
        LightTarget::Fan(position) => {
            for (place, couleur) in etat.fixe.ventilateurs[position.index()]
                .iter_mut()
                .zip(couleurs)
            {
                *place = *couleur;
            }
        }
        LightTarget::RamSlot(slot) => {
            for (place, couleur) in etat.fixe.barrettes[slot].iter_mut().zip(couleurs) {
                *place = *couleur;
            }
        }
        // `parse_request` les refuse : une liste de couleurs ne dit pas
        // laquelle va où sur dix ventilateurs.
        LightTarget::All | LightTarget::Fans | LightTarget::Ram => {}
    }
}

fn ecrire_tampon(peripheriques: &mut Peripheriques, tampon: &Tampon) {
    for position in Position::ALL {
        signaler(
            peripheriques.peindre_ventilateur(position, &tampon.ventilateurs[position.index()]),
            &format!("ventilateur {}", position.name()),
        );
    }
    for slot in SlotAddress::ALL {
        signaler(
            peripheriques.peindre_barrette(slot, &tampon.barrettes[slot.slot()]),
            &format!("barrette {}", slot.slot()),
        );
    }
}

/// Journalise un échec d'écriture sans arrêter la boucle.
///
/// Un périphérique qui refuse une trame ne doit pas emporter l'éclairage des
/// neuf autres — et surtout pas la boucle entière, qui ne redémarrerait qu'au
/// prochain `systemctl restart`.
fn signaler(resultat: std::io::Result<()>, quoi: &str) {
    if let Err(erreur) = resultat {
        eprintln!("attention : {quoi} : {erreur}");
    }
}

/// Prévient systemd que le service est prêt (`Type=notify`).
///
/// Une quinzaine de lignes plutôt qu'une dépendance : le protocole est un
/// datagramme `READY=1` sur le socket que `$NOTIFY_SOCKET` désigne. Sans ça,
/// `systemctl start` rendrait la main avant que le socket existe, et le premier
/// client trouverait porte close.
fn signaler_pret() {
    let Ok(adresse) = std::env::var("NOTIFY_SOCKET") else {
        return;
    };
    let Ok(socket) = std::os::unix::net::UnixDatagram::unbound() else {
        return;
    };

    // systemd emploie soit un chemin, soit un socket abstrait — que `@` préfixe
    // dans la variable d'environnement.
    let envoi = match adresse.strip_prefix('@') {
        Some(abstrait) => {
            use std::os::linux::net::SocketAddrExt;
            std::os::unix::net::SocketAddr::from_abstract_name(abstrait.as_bytes())
                .and_then(|adresse| socket.send_to_addr(b"READY=1", &adresse))
        }
        None => socket.send_to(b"READY=1", &adresse),
    };
    if let Err(erreur) = envoi {
        eprintln!("attention : notification systemd impossible : {erreur}");
    }
}
