//! `reverbd` — le processus qui détient les bus et garde ses descripteurs.
//!
//! Un fil écoute le socket, un fil par client transmet les ordres, et **le fil
//! principal détient seul les périphériques**. Aucun verrou sur le matériel :
//! ce qui n'est pas partagé ne peut pas entrer en collision.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::thread;
use std::time::{Duration, Instant};

use reverb_anim::{Animation, Geometrie, Image, Orientation, Reglages, Sens};
use reverb_daemon::cadence::{Cadence, Tick};
use reverb_daemon::peripheriques::{Consigne, Peripheriques};
use reverb_daemon::persistance;
use reverb_daemon::serveur::{self, Ordre};
use reverb_daemon::telemetrie;
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
    let fichier_geometrie = std::env::args()
        .nth(2)
        .map_or_else(|| PathBuf::from(persistance::CHEMIN), PathBuf::from);

    let (geometrie, souci) = persistance::charger(&fichier_geometrie);
    if let Some(souci) = souci {
        eprintln!("attention : {souci}");
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

    boucle(&mut peripheriques, &reception, geometrie, fichier_geometrie);
    ExitCode::SUCCESS
}

/// L'état d'éclairage courant.
struct Etat {
    ventilateurs: [Rgb; 10],
    barrettes: [Rgb; ram::SLOT_COUNT],
    animation: Option<(Animation, Reglages)>,
    /// Où se trouve physiquement chaque LED, et comment chaque ventilateur est
    /// monté. Réglable par le socket, conservée sur disque.
    geometrie: Geometrie,
    /// Le fichier qui conserve la géométrie d'un démarrage à l'autre.
    fichier_geometrie: PathBuf,
    /// Une couleur fixe a changé et n'a pas encore été écrite.
    a_ecrire: bool,
}

impl Etat {
    fn nouveau(geometrie: Geometrie, fichier_geometrie: PathBuf) -> Etat {
        Etat {
            ventilateurs: [Rgb::BLACK; 10],
            barrettes: [Rgb::BLACK; ram::SLOT_COUNT],
            animation: None,
            geometrie,
            fichier_geometrie,
            // Écrire l'état au démarrage : c'est ce qui donne un éclairage
            // connu au boot, sans qu'aucune fenêtre soit ouverte.
            a_ecrire: true,
        }
    }
}

fn boucle(
    peripheriques: &mut Peripheriques,
    ordres: &Receiver<Ordre>,
    geometrie: Geometrie,
    fichier_geometrie: PathBuf,
) {
    let mut etat = Etat::nouveau(geometrie, fichier_geometrie);
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
        if etat.animation.is_some() != animait {
            animait = etat.animation.is_some();
            if animait {
                cadence = Cadence::new(IMAGES_PAR_SECONDE);
                depart = Instant::now();
                pas = 0;
            }
        }

        let attente = if let Some((animation, reglages)) = etat.animation {
            match cadence.tick(depart.elapsed()) {
                Tick::Produire { sautees } => {
                    let image = animation.image(&etat.geometrie, &reglages, pas);
                    let debut = Instant::now();
                    ecrire_image(peripheriques, &image);
                    compte.image(debut.elapsed(), sautees);
                    pas = pas.wrapping_add(1);
                    Duration::ZERO
                }
                Tick::Attendre(delai) => delai,
            }
        } else {
            compte.repos();
            if etat.a_ecrire {
                ecrire_fixe(peripheriques, &etat);
                etat.a_ecrire = false;
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
    let lignes = match ordre.requete {
        Request::Status => {
            let mut lignes = telemetrie::releve(peripheriques.canaux());
            lignes.push(ResponseLine::End);
            lignes
        }

        Request::Light { target, color } => {
            appliquer_couleur(etat, target, color);
            // Une couleur fixe arrête l'animation : les deux se disputeraient
            // les mêmes LED, et la dernière écriture gagnerait au hasard.
            etat.animation = None;
            etat.a_ecrire = true;
            vec![ResponseLine::End]
        }

        Request::Animate { name, reglages } => match name {
            None => {
                etat.animation = None;
                // L'éclairage fixe reprend la main là où l'animation l'a laissé.
                etat.a_ecrire = true;
                vec![ResponseLine::End]
            }
            Some(nom) => match lancer(&nom, &reglages) {
                Ok(anime) => {
                    etat.animation = Some(anime);
                    vec![ResponseLine::End]
                }
                Err(message) => vec![ResponseLine::Error { message }],
            },
        },

        Request::Geometry { cible, reglages } => geometrie(etat, cible.as_deref(), &reglages),

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
    let _ = ordre.reponse.send(lignes);
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

    if let Err(erreur) = persistance::enregistrer(&etat.fichier_geometrie, &etat.geometrie) {
        // L'orientation est appliquée en mémoire mais ne survivra pas : le dire
        // plutôt que de laisser croire à un réglage acquis.
        return echec(format!(
            "orientation appliquée mais non conservée : {} ({erreur})",
            etat.fichier_geometrie.display()
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
}

fn ecrire_fixe(peripheriques: &mut Peripheriques, etat: &Etat) {
    for position in Position::ALL {
        let couleurs = [etat.ventilateurs[position.index()]; LEDS_PER_FAN as usize];
        signaler(
            peripheriques.peindre_ventilateur(position, &couleurs),
            &format!("ventilateur {}", position.name()),
        );
    }
    for slot in SlotAddress::ALL {
        let couleurs = [etat.barrettes[slot.slot()]; ram::LEDS_PER_STICK];
        signaler(
            peripheriques.peindre_barrette(slot, &couleurs),
            &format!("barrette {}", slot.slot()),
        );
    }
}

fn ecrire_image(peripheriques: &mut Peripheriques, image: &Image) {
    for (position, couleurs) in &image.ventilateurs {
        signaler(
            peripheriques.peindre_ventilateur(*position, couleurs),
            &format!("ventilateur {}", position.name()),
        );
    }
    for (slot, couleurs) in SlotAddress::ALL.into_iter().zip(&image.barrettes) {
        signaler(
            peripheriques.peindre_barrette(slot, couleurs),
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
