//! Les périphériques que le démon détient, ouverts une fois pour toutes.
//!
//! Tout ce module existe pour une mesure : ouvrir un `/dev/hidraw*` coûte
//! **51 ms**, y écrire une trame de 64 octets **~1,3 ms**. Rouvrir à chaque
//! trame plafonne à une image et demie par seconde ; garder les descripteurs
//! ouvre la voie à une animation qui ne saccade pas.
//!
//! **L'écran du Kraken n'est pas ici.** Le démon ne le tient pas, pour que
//! `reverb screen --image` continue de marcher pendant que le démon tourne :
//! l'envoi d'une image de 1,2 Mo n'est pas exposable sur un protocole texte, et
//! prendre l'écran sans l'exposer serait une régression de capacité pour rien.
//! Il rejoindra le démon quand la fenêtre en aura besoin.

use std::io;

use reverb_hw::hidraw::{self, OpenController};
use reverb_hw::hwmon::{self, FanChannel, Mode, Percent};
use reverb_hw::i2c;
use reverb_proto::ram::{self, SlotAddress};
use reverb_proto::{Apply, Brightness, LEDS_PER_FAN, Position, Rgb, frame};

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

        Ok((
            Peripheriques {
                controleurs,
                bus,
                canaux,
                dernier: Dernier::default(),
            },
            soucis,
        ))
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
            Consigne::Auto => hwmon::set_mode(canal, Mode::FirmwareCurve),
            Consigne::Pwm(percent) => {
                // Un canal laissé sur sa courbe firmware ignorerait la consigne
                // en silence : le passer en manuel fait partie de l'ordre.
                hwmon::set_mode(canal, Mode::Manual)?;
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
