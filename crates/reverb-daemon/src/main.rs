//! `reverbd` — le processus qui détient les bus et garde ses descripteurs.
//!
//! Un fil écoute le socket, un fil par client transmet les ordres, et **le fil
//! principal détient seul les périphériques**. Aucun verrou sur le matériel :
//! ce qui n'est pas partagé ne peut pas entrer en collision.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, channel};
use std::thread;
use std::time::{Duration, Instant};

use reverb_anim::{Animation, Geometrie, Orientation, Reglages, Sens};
use reverb_daemon::cadence::{Cadence, Tick};
use reverb_daemon::peripheriques::{Consigne, Peripheriques};
use reverb_daemon::persistance::{self, Eclairage};
use reverb_daemon::serveur::{self, Ordre};
use reverb_daemon::telemetrie;
use reverb_daemon::zones::{self, Rendu, Tampon, Zones};
use reverb_hw::hwmon::Percent;
use reverb_proto::ipc::{FanAction, LightTarget, Request, ResponseLine};
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

    let etat = Etat::nouveau(
        eclairage,
        geometrie,
        Fichiers {
            geometrie: fichier_geometrie,
            eclairage: fichier_eclairage,
            zones: fichier_zones,
        },
        couches,
        sondes,
        lancer_le_fil_du_gpu(),
    );

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

/// Les trois fichiers que le démon conserve, et leur nature.
///
/// `geometrie.conf` décrit la **machine** — un montage, qu'un administrateur
/// peut corriger — d'où `/etc`. Les deux autres décrivent l'**état du service**,
/// écrit par le démon, d'où `/var/lib`.
struct Fichiers {
    geometrie: PathBuf,
    eclairage: PathBuf,
    zones: PathBuf,
}

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
}

impl Etat {
    fn nouveau(
        eclairage: Eclairage,
        geometrie: Geometrie,
        fichiers: Fichiers,
        zones: Zones,
        sondes: Vec<reverb_hw::hwmon::Sonde>,
        gpu: CacheGpu,
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
        let mut tampon = match self.animation {
            Some((animation, reglages)) => {
                let image = animation.image(&self.geometrie, &reglages, pas);
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
            animation: self.animation,
        }
    }
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
        Request::Screen(_) => todo!("issue #33"),

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
                Some(anime) => lancer(&anime, &reglages).map(Some),
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
            let mut lignes = telemetrie::releve(
                peripheriques.canaux(),
                &etat.sondes,
                etat.gpu.lock().ok().and_then(|lu| lu.clone()),
            );
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
            Some(nom) => match lancer(&nom, &reglages) {
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
            match consigne.map_err(|e| e.to_string()).and_then(|consigne| {
                peripheriques
                    .consigner(&channel, consigne)
                    .map_err(|e| e.to_string())
            }) {
                Ok(()) => vec![ResponseLine::End],
                Err(message) => vec![ResponseLine::Error { message }],
            }
        }
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
fn lancer(nom: &str, reglages: &[(String, String)]) -> Result<(Animation, Reglages), String> {
    let animation = Animation::par_nom(nom).map_err(|erreur| erreur.to_string())?;
    let reglages = animation
        .reglages(reglages)
        .map_err(|erreur| erreur.to_string())?;
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
