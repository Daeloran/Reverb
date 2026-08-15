//! Les périphériques que le démon détient, ouverts une fois pour toutes.
//!
//! Tout ce module existe pour une mesure : ouvrir un `/dev/hidraw*` coûte
//! **51 ms**, y écrire une trame de 64 octets **~1,3 ms**. Rouvrir à chaque
//! trame plafonne à une image et demie par seconde ; garder les descripteurs
//! ouvre la voie à une animation qui ne saccade pas.
//!
//! **L'écran du Kraken est ici depuis #33.** La condition posée s'est
//! réalisée — « il rejoindra le démon quand la fenêtre en aura besoin » —, et
//! la fenêtre n'ouvre aucun périphérique (ADR-002). Elle envoie donc un chemin
//! de fichier, et c'est le démon qui lit et qui pousse les 1,2 Mo.
//!
//! ⚠️ **Régression de capacité assumée** : `reverb screen --image` en direct ne
//! marche plus pendant que le démon tourne, le nœud USB ne se réclamant pas
//! deux fois. Elle est compensée — la ligne de commande passe par le socket
//! comme la fenêtre, et y gagne le PNG, le JPEG et le GIF qu'elle n'avait pas.
//!
//! ⚠️ **Depuis #83, l'écran n'est plus ici qu'une poignée.** Le `Kraken` lui-même
//! a été déplacé dans le fil de [`crate::fil_ecran`], qui le détient seul : un
//! `write` de 1 228 800 octets sur usbfs gelait sinon les LED et le socket, qui
//! partagent ce fil-ci. Les deux méthodes d'écran de ce module **ne bloquent
//! plus et ne rendent plus d'erreur** — elles déposent, et le verdict revient
//! par [`Peripheriques::verdicts_ecran`].

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use reverb_hw::hidraw::{self, OpenController};
use reverb_hw::hwmon::{self, CourbesPosees, FanChannel, Mode, Percent};
use reverb_hw::i2c;
use reverb_hw::usbfs;
use reverb_proto::ram::{self, SlotAddress};
use reverb_proto::{Apply, Brightness, LEDS_PER_FAN, Position, Rgb, frame, screen};

use crate::ecran::{Dalle, Verdict};
use crate::fil_ecran::{Afficheur, FilEcran};
use crate::fil_reparation::{FilReparation, Reparateur};
use crate::reparation::{Constat, EtatSource};

/// L'identifiant produit du Kraken Elite 2023.
const KRAKEN: u16 = 0x300c;

/// Le nom que le pilote `nzxt-kraken3` donne au `hwmon` du Kraken.
///
/// Relevé dans le journal de l'incident du 2026-08-15 (issue #98) :
/// `kraken2023elite:fan-speed`, `kraken2023elite:pump-speed`,
/// `kraken2023elite:coolant-temp`. C'est le seul lien entre une source qui se
/// tait et le périphérique USB qu'il faudrait secouer.
pub const SOURCE_DU_KRAKEN: &str = "kraken2023elite";

/// Couleurs des huit LED d'un ventilateur.
pub type CouleursVentilateur = [Rgb; LEDS_PER_FAN as usize];

/// Couleurs des onze LED d'une barrette.
pub type CouleursBarrette = [Rgb; ram::LEDS_PER_STICK];

/// Tout ce que le démon détient.
pub struct Peripheriques {
    controleurs: Vec<OpenController>,
    /// Le bus SMBus des barrettes. Absent si la RAM n'est pas joignable — une
    /// machine sans elle doit continuer de piloter ses ventilateurs.
    bus: Option<i2c::Bus>,
    canaux: Vec<FanChannel>,
    /// Les canaux ayant reçu une courbe **depuis le démarrage de ce démon**.
    ///
    /// ⚠️ **Il repart vide à chaque démarrage, et c'est correct** : les fichiers
    /// de courbe sont en écriture seule, donc rien ne se relit sur le matériel,
    /// et on ne peut rien savoir de ce qu'un autre outil a écrit avant nous.
    /// Sans lui, « auto » écrivait `2` sur un tableau de zéros et arrêtait la
    /// régulation de la pompe, en silence (issue #97).
    ///
    /// ⚠️ **Le démon n'a aucun verbe pour poser une courbe**, et ce carnet reste
    /// donc vide toute sa vie : « auto » est refusé par le socket, et la fenêtre
    /// n'affiche pas le bouton. Conséquence connue et assumée de #97 — un bouton
    /// qui ne peut qu'arrêter la pompe vaut moins que pas de bouton —, suivie
    /// par l'issue #104, qui ajoutera le verbe `curve`.
    courbes: CourbesPosees,
    /// Le fil qui tient l'écran du Kraken (#83). Absent si le Kraken n'est pas
    /// branché, ou si la règle udev manque — une machine sans lui doit continuer
    /// de s'éclairer, et aucun fil n'est alors démarré.
    ecran: Option<FilEcran>,
    /// Le fil qui pose les `USBDEVFS_RESET`, et lui seul (#98).
    ///
    /// ⚠️ Toujours présent, même sans Kraken branché : sans périphérique il n'y a
    /// pas de source `kraken2023elite` dans `hwmon`, donc aucune cible, donc aucun
    /// effondrement à constater — le fil dort et ne coûte rien.
    reparation: FilReparation,
    /// Ce qui a été écrit en dernier, pour ne pas le réécrire.
    ///
    /// **Aucune de ces cibles n'a de watchdog** : l'état écrit tient
    /// indéfiniment. Réécrire une couleur identique ne fait donc rien d'autre
    /// que consommer du bus — et sur une animation localisée, c'est l'essentiel
    /// du travail. Dans la comète, 24 LED sur 124 sont allumées ; les autres
    /// sont noires et le restent d'une image à l'autre.
    ///
    /// Mesuré sur SHYNAEL le 2026-07-31, animation complète à 30 img/s :
    ///
    /// | | Coût d'une image | Cadence tenue |
    /// |---|---|---|
    /// | sans ce cache | 52 ms | 21 img/s, 12 sautées/s |
    /// | avec | **12 ms** | **31 img/s, 0 sautée** |
    ///
    /// ⚠️ **Contrepartie assumée** : si un autre programme écrit sur ces
    /// périphériques, le démon ne le corrigera pas tant que la couleur qu'il
    /// veut n'aura pas changé. C'est acceptable parce que la ligne de commande
    /// refuse d'écrire quand le démon tourne, et qu'aucun autre logiciel n'est
    /// censé toucher ces bus. À revoir si un jour il faut cohabiter.
    dernier: Dernier,
}

/// Le dernier état écrit sur chaque cible.
#[derive(Default)]
struct Dernier {
    ventilateurs: [Option<CouleursVentilateur>; 10],
    barrettes: [Option<CouleursBarrette>; ram::SLOT_COUNT],
}

impl Peripheriques {
    /// Découvre et ouvre tout, une fois.
    ///
    /// Ce qui manque est signalé mais n'empêche pas de démarrer : un démon qui
    /// refuse de se lancer parce qu'une barrette ne répond pas laisse la
    /// machine sans éclairage du tout.
    pub fn ouvrir() -> io::Result<(Peripheriques, Vec<String>)> {
        let mut soucis = Vec::new();

        let mut controleurs = Vec::new();
        for controleur in hidraw::discover()? {
            let chemin = controleur.path.clone();
            match controleur.open() {
                Ok(ouvert) => controleurs.push(ouvert),
                Err(erreur) => soucis.push(format!("{} : {erreur}", chemin.display())),
            }
        }
        if controleurs.is_empty() {
            soucis.push("aucun contrôleur d'éclairage NZXT ouvert".to_owned());
        }

        let bus = match i2c::find_adapter().and_then(|chemin| i2c::Bus::open(&chemin)) {
            Ok(bus) => Some(bus),
            Err(erreur) => {
                soucis.push(format!("RAM Corsair injoignable : {erreur}"));
                None
            }
        };

        let canaux = hwmon::discover().unwrap_or_else(|erreur| {
            soucis.push(format!("ventilateurs illisibles : {erreur}"));
            Vec::new()
        });

        // Les contrôleurs veulent leur séquence d'initialisation une fois par
        // vie de périphérique (spec §8), pas une fois par image.
        for controleur in &mut controleurs {
            let modele = controleur.controller.model;
            for trame in frame::init_sequence(modele) {
                if let Err(erreur) = controleur.write_frame(&trame) {
                    soucis.push(format!(
                        "initialisation de {} : {erreur}",
                        controleur.controller.path.display()
                    ));
                    break;
                }
            }
        }

        // L'écran est facultatif : une machine sans Kraken doit démarrer, et le
        // dire **une fois**, sans boucler à le chercher. Le fil dédié n'est
        // lancé que si le périphérique a été ouvert — un fil qui n'aurait rien à
        // tenir dormirait pour rien jusqu'à l'arrêt du démon.
        let (ecran, serie_du_kraken) = match Kraken::ouvrir() {
            Ok(kraken) => {
                // Relevée **avant** de confier le Kraken à son fil : après, plus
                // personne ne peut le lui demander, et c'est bien le but (#83).
                let serie = kraken.bulk.serie().map(str::to_owned);
                (Some(FilEcran::demarrer(kraken)), serie)
            }
            Err(erreur) => {
                soucis.push(format!("écran du Kraken injoignable : {erreur}"));
                (None, None)
            }
        };

        Ok((
            Peripheriques {
                controleurs,
                bus,
                canaux,
                courbes: CourbesPosees::vide(),
                ecran,
                reparation: FilReparation::demarrer(ReparateurUsb {
                    serie: serie_du_kraken,
                }),
                dernier: Dernier::default(),
            },
            soucis,
        ))
    }

    /// Vrai si le démon tient l'écran.
    pub fn a_un_ecran(&self) -> bool {
        self.ecran.is_some()
    }

    /// Dépose un ordre de luminosité pour la dalle. **Ne bloque pas.**
    ///
    /// ⚠️ **Avant l'image, jamais après** : `30 02` réinitialise le pipeline
    /// d'affichage (spec §3.4), et l'envoyer ensuite ferait clignoter la dalle
    /// vers son affichage firmware. L'ordre relatif est préservé par la file du
    /// fil de l'écran, pas par la chance d'un ordonnancement.
    ///
    /// ⚠️ **Aucune erreur n'est rendue ici** (#83) : l'écriture a lieu sur un
    /// autre fil, et l'attendre remettrait les 51 ms d'ouverture d'un `hidraw`
    /// dans le chemin critique des LED. Un refus revient par
    /// [`Peripheriques::verdicts_ecran`], et le journal le dit.
    pub fn luminosite_ecran(&self, pourcent: u8) {
        if let Some(ecran) = &self.ecran {
            ecran.luminosite(pourcent);
        }
    }

    /// Dépose une image à pousser sur la dalle. **Ne bloque pas.**
    ///
    /// Elle **remplace** celle qui attendait encore : une composition périmée
    /// n'a aucune raison d'atteindre la dalle derrière celle qui la corrige.
    pub fn afficher_ecran(&self, dalle: Dalle) {
        if let Some(ecran) = &self.ecran {
            ecran.afficher(dalle);
        }
    }

    /// Ce que la dalle a dit depuis le dernier tour. **N'attend rien.**
    pub fn verdicts_ecran(&self) -> Vec<Verdict> {
        self.ecran
            .as_ref()
            .map(FilEcran::verdicts)
            .unwrap_or_default()
    }

    /// Remet la dalle en émission après un abandon (#70).
    pub fn relancer_ecran(&self) {
        if let Some(ecran) = &self.ecran {
            ecran.relancer();
        }
    }

    // -----------------------------------------------------------------------
    // La réparation d'une source muette (issue #98)
    // -----------------------------------------------------------------------

    /// Les sources dont il vaut la peine de soumettre l'état.
    pub fn sources_reparables(&self) -> &[String] {
        self.reparation.sources()
    }

    /// Confie l'état d'une source au fil de réparation. **Ne bloque pas.**
    pub fn soumettre_reparation(&self, etat: EtatSource, maintenant: Duration) {
        self.reparation.soumettre(etat, maintenant);
    }

    /// Ce que le fil de réparation a décidé depuis le dernier tour. **N'attend
    /// rien.**
    pub fn constats_reparation(&self) -> Vec<(String, Constat)> {
        self.reparation.constats()
    }

    /// Lâche la poignée usbfs de la dalle, sans attendre son fil.
    ///
    /// ⚠️ **Un `USBDEVFS_RESET` invalide toute poignée ouverte sur le
    /// périphérique**, y compris celle que le fil de l'écran détient : ses
    /// `ioctl` rendront `ENODEV` jusqu'à ce qu'on rouvre. On la lâche donc dès que
    /// le geste a réussi, sans attendre — joindre le fil remettrait, dans le
    /// chemin critique des LED, l'envoi de 1,2 Mo que #83 en a sorti. Son `Drop`
    /// lui demande de sortir, il termine ce qu'il avait en main et s'en va.
    ///
    /// Entre ce moment et [`Peripheriques::rouvrir_ecran`], la dalle est rendue au
    /// firmware et les dépôts d'image sont sans effet.
    ///
    /// Rend `true` s'il y avait bien une poignée à lâcher — donc s'il y en aura
    /// une à rouvrir.
    pub fn lacher_ecran(&mut self) -> bool {
        self.ecran.take().is_some()
    }

    /// Rouvre la poignée usbfs de la dalle après un reset.
    ///
    /// À n'appeler qu'une fois le périphérique réénuméré : ouvert trop tôt, le
    /// nœud n'existe pas encore et l'ouverture échoue pour une raison qui n'a rien
    /// à voir avec l'état du Kraken.
    pub fn rouvrir_ecran(&mut self) -> Result<(), String> {
        let kraken = Kraken::ouvrir().map_err(|erreur| erreur.to_string())?;
        self.ecran = Some(FilEcran::demarrer(kraken));
        Ok(())
    }

    /// Redécouvre les canaux de vitesse, par leur nom.
    ///
    /// ⚠️ **Les chemins changent, les noms non.** Un `hwmon` qui disparaît puis
    /// revient reçoit le numéro libre, qui n'est pas forcément le sien : garder les
    /// anciennes poignées après un reset, c'est lire le fichier d'un **autre**
    /// périphérique et l'afficher sous le nom du premier. Le mode de défaillance
    /// le plus coûteux du projet est celui qui rassure.
    pub fn redecouvrir_canaux(&mut self) -> Result<usize, String> {
        let canaux = hwmon::discover().map_err(|erreur| erreur.to_string())?;
        self.canaux = canaux;
        Ok(self.canaux.len())
    }

    /// Peint les huit LED d'un ventilateur.
    ///
    /// Trois trames, sur un descripteur déjà ouvert — leur indissociabilité
    /// (spec §0.2) est préservée par le fait qu'elles partent d'affilée.
    pub fn peindre_ventilateur(
        &mut self,
        position: Position,
        couleurs: &CouleursVentilateur,
    ) -> io::Result<()> {
        if self.dernier.ventilateurs[position.index()].as_ref() == Some(couleurs) {
            return Ok(());
        }

        let placement = position.placement();
        let controleur = self
            .controleurs
            .iter_mut()
            .find(|c| c.controller.serial == placement.serial)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("contrôleur {} absent", placement.serial),
                )
            })?;

        let trames = frame::per_led(placement.mask, couleurs, Apply::Static, Brightness::FULL)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

        for trame in &trames {
            controleur.write_frame(trame)?;
        }
        // Après l'écriture seulement : une trame refusée ne doit pas faire
        // croire que la couleur est en place, sinon on ne la réessaierait
        // jamais.
        self.dernier.ventilateurs[position.index()] = Some(*couleurs);
        Ok(())
    }

    /// Peint les onze LED d'une barrette.
    pub fn peindre_barrette(
        &mut self,
        slot: SlotAddress,
        couleurs: &CouleursBarrette,
    ) -> io::Result<()> {
        if self.dernier.barrettes[slot.slot()].as_ref() == Some(couleurs) {
            return Ok(());
        }

        let bus = self
            .bus
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "bus SMBus non ouvert"))?;

        let (tete, queue) = ram::transfers(couleurs)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

        bus.target(slot)?;
        // Les deux blocs se suivent immédiatement (spec §4.3) : une barrette qui
        // reçoit le premier sans le second affiche un état dont le CRC n'est
        // jamais arrivé.
        bus.write_block(&tete)?;
        bus.write_block(&queue)?;

        self.dernier.barrettes[slot.slot()] = Some(*couleurs);
        Ok(())
    }

    /// Les canaux de vitesse découverts.
    pub fn canaux(&self) -> &[FanChannel] {
        &self.canaux
    }

    /// Le carnet des courbes posées depuis le démarrage (issue #97).
    ///
    /// C'est lui qui décide du booléen « auto » de chaque ligne `chan`, donc du
    /// bouton de la fenêtre : la même question que celle du refus, posée sans
    /// rien écrire.
    pub fn courbes_posees(&self) -> &CourbesPosees {
        &self.courbes
    }

    /// Applique une consigne à un canal, ou le rend à son firmware.
    ///
    /// Les garde-fous de la ligne de commande — plancher de 20 %, `--manual`
    /// pour sortir un canal de sa courbe — ne sont **pas** rejoués ici : le
    /// socket est une porte de service pour la fenêtre, qui affichera les mêmes
    /// avertissements. Ce qui reste vérifié, c'est la borne 0–100, et elle l'est
    /// par `Percent`.
    pub fn consigner(&self, canal: &str, action: Consigne) -> io::Result<()> {
        let canal = self
            .canaux
            .iter()
            .find(|c| c.name == canal)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("canal « {canal} » inconnu"),
                )
            })?;

        match action {
            // ⚠️ `HostCurve` (2) et non `PleinRegime` (0), et **ce n'est pas un
            // retour à la courbe du périphérique** — comme ce commentaire l'a
            // prétendu jusqu'au 2026-08-15. `2` rend la main à la courbe de
            // l'**hôte** : `nzxt-kraken3` pousse le tableau de points que le
            // pilote détient, lequel est à zéro partout tant que personne n'y a
            // téléversé de courbe. « auto » arrêtait donc la régulation de la
            // pompe au lieu de la rendre (issue #97). Aucune valeur de
            // `pwmN_enable` ne rend le Kraken à son profil d'usine ; seule une
            // coupure d'alimentation le fait.
            //
            // `0`, lui, envoie le canal à 100 % et lâche la barre — sur la
            // pompe, en silence (issue #50).
            //
            // Deux refus tombent ici, avant toute écriture, chacun en nommant le
            // canal : un pilote sans mode automatique, et un canal sans courbe
            // posée. Le second est **toujours** vrai côté démon tant que le
            // socket n'a pas de verbe `curve` (issue #104).
            Consigne::Auto => hwmon::set_mode(canal, Mode::HostCurve, &self.courbes),
            Consigne::Pwm(percent) => {
                // Un canal laissé sur une courbe ignorerait la consigne en
                // silence : le passer en manuel fait partie de l'ordre.
                hwmon::set_mode(canal, Mode::Manual, &self.courbes)?;
                hwmon::set_pwm(canal, percent)
            }
        }
    }
}

/// Ce qu'on demande à un canal de vitesse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consigne {
    Pwm(Percent),
    Auto,
}

/// Le seul geste de réparation que le projet connaisse (issue #98).
///
/// ⚠️ **Le mécanisme n'est pas propre au Kraken, mais le geste l'est.** L'issue met
/// hors scope « réparer autre chose que le Kraken » tout en demandant que le
/// mécanisme ne lui soit pas propre : la décision est donc écrite par source
/// (`crate::reparation`), et c'est ici, au dernier moment, que la seule
/// correspondance connue apparaît. Le jour où un autre contrôleur se tait de la
/// même façon, c'est cette structure qui grandit, pas la décision.
struct ReparateurUsb {
    /// La série du Kraken, relevée à l'ouverture de sa poignée. `None` quand
    /// aucun n'a été ouvert, ou qu'il n'en expose pas — la résolution retombe
    /// alors sur VID:PID seuls, ce qui suffit tant qu'il n'y en a qu'un.
    ///
    /// ⚠️ **Elle est figée au démarrage du démon**, ce fil détenant le réparateur
    /// seul. C'est sans conséquence : une série est dans le descripteur du
    /// périphérique et ne change pas d'une énumération à l'autre. Un Kraken
    /// **remplacé** par un autre pendant la vie du démon ferait viser l'ancienne,
    /// donc échouer proprement en la nommant — et un redémarrage du service la
    /// relève.
    serie: Option<String>,
}

impl Reparateur for ReparateurUsb {
    fn sources(&self) -> Vec<String> {
        vec![SOURCE_DU_KRAKEN.to_owned()]
    }

    fn reinitialiser(&mut self, source: &str) -> io::Result<()> {
        if source != SOURCE_DU_KRAKEN {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("aucun geste de réparation connu pour « {source} »"),
            ));
        }

        // ⚠️ **Journalisé avant, parce qu'un reset USB est visible sur la
        // machine.** Le périphérique disparaît du bus puis y revient ; d'autres
        // programmes peuvent s'en apercevoir, et le noyau va journaliser sa
        // réénumération. Une ligne qui n'apparaîtrait qu'après ferait chercher la
        // cause du côté du matériel.
        eprintln!(
            "réparation : reset USB de « {source} »{} — le périphérique va quitter le bus puis y \
             revenir",
            match &self.serie {
                Some(serie) => format!(" (série {serie})"),
                None => String::new(),
            }
        );

        let noeud = usbfs::reset(self.serie.as_deref())?;
        eprintln!("réparation : reset passé sur {}", noeud.display());
        Ok(())
    }
}

/// L'écran du Kraken, ses deux interfaces tenues ouvertes.
///
/// **Deux nœuds pour un écran** : `MI_01` en HID porte les commandes — mode de
/// diffusion, annonce, validation —, `MI_00` en bulk porte les 1,2 Mo de
/// pixels. Le premier se rouvre à chaque trame, ce qui coûte 51 ms ; c'est
/// tenable ici, une image ne partant qu'une fois toutes les vingt-cinq
/// secondes. Le second, lui, est **tenu** : c'est le nœud que la ligne de
/// commande ne peut plus réclamer tant que le démon tourne.
struct Kraken {
    hidraw: PathBuf,
    bulk: usbfs::Screen,
}

impl Kraken {
    fn ouvrir() -> io::Result<Kraken> {
        Ok(Kraken {
            hidraw: hidraw::find_path(KRAKEN)?,
            bulk: usbfs::Screen::open()?,
        })
    }
}

/// ⚠️ **C'est ici que passe tout ce qui bloque** (#83). Les deux méthodes de ce
/// bloc sont les seules du démon à écrire vers la dalle, et elles ne sont
/// appelées que depuis le fil de [`crate::fil_ecran`] — qui a reçu ce `Kraken`
/// par valeur, et que personne d'autre ne peut donc atteindre.
impl Afficheur for Kraken {
    fn luminosite(&mut self, pourcent: u8) -> io::Result<()> {
        let trame = screen::set_brightness(pourcent)
            .map_err(|erreur| io::Error::new(io::ErrorKind::InvalidInput, erreur.to_string()))?;
        hidraw::write_frame(&self.hidraw, &trame)
    }

    /// Envoie une image, en respectant les accusés du contrôleur.
    ///
    /// Le contrôleur **acquitte chaque étape** et attend l'accusé avant la
    /// suivante (spec §3.2) : `36 01` → `37 01`, puis les données, puis
    /// `36 02` → `37 02`. Envoyer les 1,2 Mo sans attendre `37 01`, c'est
    /// parler à un contrôleur qui n'écoute pas encore.
    fn image(&mut self, dalle: &Dalle) -> io::Result<()> {
        let image = dalle.octets();
        screen::check_image(image)
            .map_err(|erreur| io::Error::new(io::ErrorKind::InvalidInput, erreur.to_string()))?;

        // INDISPENSABLE : sans cette trame, l'image est ignorée en silence.
        hidraw::write_frame(&self.hidraw, &screen::broadcast_mode())?;

        let longueur = u32::try_from(image.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "image trop volumineuse"))?;
        let entete = screen::bulk_header(longueur);

        let accuse = hidraw::ask(&self.hidraw, &screen::begin_image(), &[0x37, 0x01])?;
        verifier(&accuse, "l'annonce")?;

        self.bulk.write_bulk(&entete)?;
        self.bulk.write_bulk(image)?;

        let accuse = hidraw::ask(&self.hidraw, &screen::end_image(), &[0x37, 0x02])?;
        verifier(&accuse, "la validation")
    }
}

/// Un accusé du contrôleur d'écran, dont le troisième octet porte le verdict.
fn verifier(accuse: &reverb_proto::Frame, quoi: &str) -> io::Result<()> {
    match screen::check_ack(accuse) {
        Ok(()) => Ok(()),
        Err(erreur) => Err(io::Error::other(format!("{quoi} refusée : {erreur}"))),
    }
}
