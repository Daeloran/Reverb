//! La courbe de la régulation côté hôte : température → consigne.
//!
//! Pure : aucune E/S, aucune horloge, aucune dépendance au matériel. C'est ce
//! qui lui permet de vivre ici plutôt que dans le démon.
//!
//! # Pourquoi elle a déménagé (issue #113)
//!
//! Elle est née dans `reverb_daemon::regulation` (issue #99), où seule la
//! boucle du démon la lisait. #113 donne l'édition de la courbe à la fenêtre,
//! **avec le tracé de ce qu'elle donne**, et l'issue pose la condition :
//!
//! > ⚠️ Le tracé de la courbe doit venir de la **même** fonction que celle
//! > qu'exécute le démon, comme la maquette partage `reverb-anim` avec lui —
//! > sinon l'aperçu montrerait une courbe que le boîtier n'applique pas.
//!
//! Or `reverb-gui` ne dépend pas de `reverb-daemon`, et n'a aucune raison de se
//! mettre à en dépendre : le tracé serait donc une réimplémentation, c'est-à-dire
//! deux interpolations à tenir d'accord dont l'une n'aurait aucune raison d'être
//! la bonne. [`Courbe`] descend donc dans le crate que les deux partagent —
//! `ipc.rs` y encode déjà ses paliers —, et `reverb_daemon::regulation` la
//! **ré-exporte** plutôt que de la recopier. C'est exactement le remède que #112
//! a appliqué aux jetons de mode.
//!
//! # Millidegrés partout, jamais de degrés flottants
//!
//! `Sonde::lire` rend des millidegrés entiers, comme `hwmon` ; une conversion
//! vers `f32` en chemin rendrait la courbe dépendante d'un arrondi. Le projet a
//! déjà payé ce prix une fois — la symétrie des directions locales est calculée
//! sur les **indices**, jamais sur une position flottante (#75). Deux unités
//! dans la même API seraient pire encore : c'est la faute des trois ordres de
//! composantes, qui ne produit aucun message et juste un résultat faux.

use std::fmt;

/// La courbe température → consigne.
///
/// Pure : aucune E/S, aucune horloge, aucune dépendance au matériel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Courbe {
    /// Des paliers `(millidegrés, pourcent)`, au moins un, **strictement
    /// croissants** en température.
    paliers: Vec<(i32, u8)>,
}

impl Courbe {
    /// Le tableau de l'issue #99, calé sur les mesures du 2026-08-15.
    ///
    /// 30 % à 35 °C, 60 % à 45 °C, 100 % à 50 °C — la médiane relevée pendant la
    /// session était de 50,7 °C, donc plein régime là où le boîtier subissait
    /// 25 %.
    pub fn defaut() -> Courbe {
        Courbe {
            paliers: vec![(35_000, 30), (45_000, 60), (50_000, 100)],
        }
    }

    /// Une courbe réglée, vérifiée à la construction.
    ///
    /// # Erreurs
    ///
    /// Une courbe sans palier, une consigne au-delà de cent, ou des
    /// températures qui ne montent pas strictement.
    ///
    /// ⚠️ **Refusée plutôt que triée en silence.** Deux paliers à la même
    /// température se contredisent — laquelle des deux consignes à 45 °C ? — et
    /// réordonner ce qu'un humain a écrit, c'est « compléter au jugé », ce que
    /// le projet refuse déjà pour `eclairage.conf`.
    ///
    /// ⚠️ **C'est le seul juge, et la fenêtre s'en sert aussi** (#113). Une
    /// fenêtre plus sévère refuserait une courbe que le socket accepte ; une
    /// fenêtre plus laxiste laisserait partir une ligne que le démon rejettera,
    /// et le refus arriverait après le geste au lieu d'arriver devant le champ.
    pub fn depuis(paliers: &[(i32, u8)]) -> Result<Courbe, CourbeInvalide> {
        let refus = |raison: String| CourbeInvalide { raison };

        let Some(((premiere_t, _), suite)) = paliers.split_first() else {
            return Err(refus(
                "une courbe sans palier ne dit aucune consigne : rendre 0 % arrêterait les \
                 ventilateurs sur une table que personne n'a écrite"
                    .to_owned(),
            ));
        };

        for (temperature, consigne) in paliers {
            if *consigne > 100 {
                return Err(refus(format!(
                    "palier à {temperature} m°C : consigne de {consigne} %, au-delà de cent pour \
                     cent il n'y a plus de ventilateur"
                )));
            }
        }

        let mut precedente = *premiere_t;
        for (temperature, _) in suite {
            if *temperature == precedente {
                return Err(refus(format!(
                    "palier répété à {temperature} m°C : deux consignes pour la même température \
                     se contredisent"
                )));
            }
            if *temperature < precedente {
                return Err(refus(format!(
                    "palier à {temperature} m°C après {precedente} m°C : les températures d'une \
                     courbe montent, et les réordonner serait deviner ce qui a été tapé"
                )));
            }
            precedente = *temperature;
        }

        Ok(Courbe {
            paliers: paliers.to_vec(),
        })
    }

    /// Les paliers, dans l'ordre où ils ont été donnés.
    pub fn paliers(&self) -> &[(i32, u8)] {
        &self.paliers
    }

    /// La consigne pour une température en **millidegrés**.
    ///
    /// ⚠️ **Bornée, jamais extrapolée.** Sous le premier palier et au-dessus du
    /// dernier, la courbe rend la borne : prolonger la droite du premier
    /// segment donnerait 0 % à 25 °C, c'est-à-dire des ventilateurs à l'arrêt
    /// sur un circuit qui démarre — pire que le défaut qu'on corrige.
    ///
    /// ⚠️ **Le bornage a lieu avant toute soustraction.** Une sonde qui rendrait
    /// `i32::MIN` ferait déborder `temperature - premier_palier`, et en `debug`
    /// c'est une panique — dans le fil qui sert aussi le socket.
    ///
    /// ⚠️ **L'interpolation arrondit au plus proche, et un palier n'est atteint
    /// qu'à sa température.** Les deux règles se lisent sur les deux exigences
    /// du fichier d'intention, et aucune n'est cosmétique :
    ///
    /// - une **troncature** ferait basculer la consigne au moindre hoquet de la
    ///   sonde là où un palier tombe juste. 36,000 °C vaut exactement 33 % ;
    ///   35,990 °C — dix millidegrés plus bas, ce que le liquide fait toute la
    ///   journée — vaudrait 32 %, soit une écriture sur le bus par frémissement.
    ///   C'est ce que `une_temperature_inchangee_ne_produit_aucune_ecriture`
    ///   attrape, à dix millidegrés près ;
    /// - un **arrondi seul** annoncerait la consigne du palier suivant avant sa
    ///   température : 44,999 °C rendrait 60 %, et « 60 % à 45 °C » cesserait de
    ///   décrire la table. C'est ce que `la_courbe_interpole_lineairement…`
    ///   attrape, à un millidegré près.
    ///
    /// D'où des segments **semi-ouverts** — un palier appartient au segment qui
    /// part de lui — et une consigne interpolée qui ne prend jamais la valeur du
    /// palier haut.
    pub fn consigne(&self, milli_degres: i32) -> u8 {
        let (premiere_t, premiere_c) = self.paliers[0];
        if milli_degres <= premiere_t {
            return premiere_c;
        }
        let (derniere_t, derniere_c) = self.paliers[self.paliers.len() - 1];
        if milli_degres >= derniere_t {
            return derniere_c;
        }

        for fenetre in self.paliers.windows(2) {
            let (bas_t, bas_c) = fenetre[0];
            let (haut_t, haut_c) = fenetre[1];
            // Semi-ouvert : `haut_t` appartient au segment suivant, où il vaut
            // exactement `haut_c`.
            if milli_degres < bas_t || milli_degres >= haut_t {
                continue;
            }
            // En `i64` : l'écart de deux températures ne tient pas toujours dans
            // un `i32`, et son produit par l'écart de consignes encore moins.
            let largeur = i64::from(haut_t) - i64::from(bas_t);
            let avance = i64::from(milli_degres) - i64::from(bas_t);
            let denivele = i64::from(haut_c) - i64::from(bas_c);
            let interpolee = i64::from(bas_c) + arrondi(denivele * avance, largeur);

            // La consigne du palier haut lui reste réservée. Un segment plat n'a
            // rien à réserver — les deux paliers portent la même consigne.
            let bornee = match denivele.signum() {
                1 => interpolee.min(i64::from(haut_c) - 1),
                -1 => interpolee.max(i64::from(haut_c) + 1),
                _ => interpolee,
            };
            return u8::try_from(bornee.clamp(0, 100)).unwrap_or(100);
        }

        // Inatteignable : les paliers montent, et les deux bornes sont déjà
        // traitées. Rendre la dernière consigne plutôt que paniquer — une
        // panique ici gèlerait le boîtier pour une arithmétique.
        derniere_c
    }
}

/// Une division entière arrondie au plus proche, la moitié vers le haut.
///
/// `div_euclid` et non `/` : la division de Rust tronque vers zéro, ce qui
/// arrondirait dans deux sens différents selon le signe du dénivelé — une
/// courbe descendante n'a pas à se comporter autrement qu'une montante.
///
/// `denominateur` est une largeur de segment, donc strictement positive : les
/// paliers d'une [`Courbe`] sont strictement croissants.
fn arrondi(numerateur: i64, denominateur: i64) -> i64 {
    (2 * numerateur + denominateur).div_euclid(2 * denominateur)
}

/// Ce qui empêche une suite de paliers d'être une courbe.
#[derive(Debug)]
pub struct CourbeInvalide {
    pub raison: String,
}

impl fmt::Display for CourbeInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raison)
    }
}

impl std::error::Error for CourbeInvalide {}
