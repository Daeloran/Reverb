//! La régulation des canaux que leur pilote ne régule pas (issue #99).
//!
//! # Le défaut que ce module corrige
//!
//! Sept des dix ventilateurs du boîtier sont sur les trois canaux `nzxtsmart2`,
//! et le pilote `nzxt-smart2` n'a **aucun mode automatique** : sa vitesse est
//! celle que l'hôte écrit (`docs/VENTILATEURS.md`, et la lecture du pilote faite
//! en #50). Personne ne l'écrivait. Mesuré sur SHYNAEL le 2026-08-15, 863
//! relevés sur 72 minutes de jeu : le duty de ces trois canaux a pris
//! **exactement une valeur**, `64` sur 255 — 25 %, ~700 tr/min — pendant que le
//! liquide passait quarante-cinq minutes au-dessus de 50 °C.
//!
//! ⚠️ **Le défaut n'était pas qu'une consigne soit fausse, c'est qu'aucune ne
//! soit jamais écrite.**
//!
//! # Ce que ce module fait, et ce qu'il ne fait pas
//!
//! Il **décide**, il n'écrit pas : [`Regulation::tour`] rend la liste des
//! écritures à faire, et c'est le démon qui les fait. C'est ce qui rend « on
//! n'écrit que ce qui change » vérifiable sans matériel — une liste vide est un
//! tour qui ne consomme pas le bus — et c'est le même parti que
//! [`crate::quarantaine`], qui ne lit rien et ne tient aucune horloge.
//!
//! La température arrive donc en paramètre, et la lecture de la sonde reste au
//! démon, seul à savoir ce que la machine expose et seul à payer les cinq
//! secondes d'une sonde muette (#68).
//!
//! # Ce que le canal porte, et non ce qu'on a cru y écrire (issue #110)
//!
//! Le 2026-08-15, quelques minutes après le déploiement de #99, le journal
//! annonçait les trois canaux à 50 % pendant que `nzxtsmart2:fan-3` portait 25 %.
//! `fs::write` sur `pwmN` avait rendu `Ok` sans que le matériel applique ; le
//! cache retenait l'intention, la consigne calculée ne changeait plus, et **rien
//! n'était jamais réémis**.
//!
//! ⚠️ **Le défaut n'était pas qu'une écriture échoue, c'est qu'elle réussisse
//! sans rien appliquer.** C'est la leçon que `docs/VENTILATEURS.md` avait déjà
//! tirée le 2026-07-30 — « restaurer une valeur n'est pas restaurer un
//! comportement », « toute sonde future doit **mesurer** l'état, pas le
//! supposer » — reprise par l'autre bout.
//!
//! [`Regulation::tour`] reçoit donc en second paramètre ce que chaque canal
//! **porte**, relu par le démon, et décide dessus. La relecture entre par la
//! porte : la faire ici rendrait ce module dépendant d'un `hwmon`, donc
//! intestable sans matériel — exactement ce que #99 avait acheté en rendant les
//! écritures au lieu de les faire.
//!
//! # Millidegrés partout, jamais de degrés flottants
//!
//! `Sonde::lire` rend des millidegrés entiers, comme `hwmon` ; une conversion
//! vers `f32` en chemin rendrait la courbe dépendante d'un arrondi. Le projet a
//! déjà payé ce prix une fois — la symétrie des directions locales est calculée
//! sur les **indices**, jamais sur une position flottante (#75). Deux unités
//! dans la même API seraient pire encore : c'est la faute des trois ordres de
//! composantes, qui ne produit aucun message et juste un résultat faux.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use reverb_proto::ipc;

use crate::persistance::ecrire;

/// La sonde dont la régulation dépend, et la seule (issue #99).
///
/// C'est la logique d'un AIO : le liquide bouge lentement — il a mis quarante
/// minutes à monter pendant la session mesurée —, donc les ventilateurs ne
/// pompent pas. C'est aussi la sonde dont le Kraken se sert déjà pour sa propre
/// courbe firmware, donc les deux régulations restent cohérentes entre elles.
///
/// Tctl est écarté : il saute de 20 °C entre deux secondes sur un Zen 5 et
/// demanderait un lissage.
pub const SONDE_DU_LIQUIDE: &str = "kraken2023elite:coolant-temp";

/// La consigne appliquée quand le liquide est illisible.
///
/// ⚠️ **Jamais la dernière valeur connue.** C'est le mode de défaillance
/// rassurant que le projet refuse partout ailleurs : une consigne figée à 30 %
/// derrière une sonde morte, c'est un CPU qui chauffe sans que rien ne le
/// signale. Et ça arrive — le Kraken se plante périodiquement.
pub const REPLI: u8 = 50;

/// Où l'état de la régulation est conservé entre deux démarrages.
///
/// `/var/lib` et non `/etc` : c'est de l'état de service, réécrit à chaque
/// commande, pas une donnée de montage. La géométrie, qui a coûté un relevé au
/// sol, reste dans `/etc`.
pub const CHEMIN_REGULATION: &str = "/var/lib/reverb/regulation.conf";

/// En-tête du fichier d'état.
const EN_TETE: &str = "\
# Regulation cote hote — Reverb (issue #99)
#
# Ecrit par reverbd a chaque changement, relu au demarrage. Le supprimer suffit
# a repartir sans aucun canal regule.
#
#   courbe <millidegres>:<0-100> ...   exactement une, temperatures croissantes
#   canal <canal>                      un par canal regule, jamais deux fois
";

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

/// Pourquoi cette écriture part (issue #110).
///
/// ⚠️ **Ce n'est pas une phrase de journal, c'est de quoi en écrire une juste.**
/// Une consigne que le canal n'applique pas repart **à chaque tour**, donc une
/// fois par seconde : 86 400 écritures par jour, et autant de lignes si
/// l'appelant les traite toutes pareil — très exactement le chiffre que #99
/// invoquait pour justifier son cache, retourné contre lui. Le motif laisse au
/// démon le choix de n'imprimer que ce qui est neuf, comme
/// [`crate::telemetrie::TourCanaux::a_signaler`] le fait pour la quarantaine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motif {
    /// La consigne a changé — ou le canal vient d'être pris en charge.
    Consigne,
    /// La consigne n'a pas changé, mais le canal ne la porte pas : on rejoue.
    ///
    /// `porte` est ce que la relecture a rendu — les `25 %` qui manquaient au
    /// journal du 2026-08-15. Il est `Option<u8>` par symétrie avec la relecture,
    /// mais un canal illisible n'est jamais écrit : en pratique il est toujours
    /// renseigné.
    NonAppliquee { porte: Option<u8> },
}

/// Une écriture à faire sur un canal, et pourquoi.
///
/// La régulation ne touche aucun bus : elle dit quoi écrire, le démon écrit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ecriture {
    pub canal: String,
    pub consigne: u8,
    pub motif: Motif,
}

/// Quels canaux sont régulés, sur quelle courbe, et où ils en sont.
#[derive(Debug)]
pub struct Regulation {
    courbe: Courbe,
    /// Les canaux régulés, et la **dernière consigne écrite** sur chacun —
    /// `None` tant que rien ne lui a été écrit depuis son activation.
    ///
    /// ⚠️ **Ce n'est plus la source de vérité, et c'est tout le sujet de #110.**
    /// Ce champ portait « on n'écrit que ce qui change », et il mesurait ce
    /// changement sur **nos intentions** : une écriture qui rendait `Ok` sans
    /// rien appliquer y était retenue comme faite, et n'était plus jamais
    /// rejouée. C'est ce qui a laissé `nzxtsmart2:fan-3` à son duty d'allumage
    /// pendant que le démon annonçait 50 %. Ce qu'il faut écrire se décide
    /// désormais sur `portees`, ce que le canal **porte** vraiment.
    ///
    /// Ce qu'il reste ici est une **mémoire d'activation** : ce canal a-t-il reçu
    /// quelque chose depuis qu'on l'a pris en charge, et quoi. Elle sert deux
    /// fois, et deux fois seulement :
    ///
    /// - un canal tout juste activé est écrit même s'il porte déjà la consigne —
    ///   `un_canal_coupe_puis_repris_est_reecrit` (#99) l'exige, parce qu'entre
    ///   la coupure et la reprise le canal appartenait à l'utilisateur ;
    /// - elle départage [`Motif::Consigne`] de [`Motif::NonAppliquee`], donc ce
    ///   que le journal a le droit d'imprimer. La supprimer tout à fait rendrait
    ///   les deux indiscernables.
    ///
    /// ⚠️ **Un cache par canal, jamais un « dernier état global ».** Un canal
    /// qu'on vient d'activer n'a jamais rien reçu : il doit être écrit au tour
    /// suivant même si la température n'a pas bougé d'un millidegré. Un cache
    /// global le laisserait à 25 % jusqu'au prochain changement de palier — ce
    /// qui, un jour de température stable, veut dire jamais.
    ///
    /// `BTreeMap` et non `HashMap` : les canaux se rendent triés, et l'ordre du
    /// fichier d'état ne doit pas changer d'un démarrage à l'autre.
    canaux: BTreeMap<String, Option<u8>>,
}

impl Regulation {
    /// Une régulation qui ne régule rien, sur cette courbe.
    pub fn nouvelle(courbe: Courbe) -> Regulation {
        Regulation {
            courbe,
            canaux: BTreeMap::new(),
        }
    }

    pub fn courbe(&self) -> &Courbe {
        &self.courbe
    }

    /// Change la courbe sans toucher aux canaux régulés.
    ///
    /// Le cache n'est pas vidé : si la nouvelle courbe rend la même consigne à
    /// la température du moment, il n'y a rien à écrire, et une réécriture ne
    /// ferait que consommer le bus.
    pub fn regler(&mut self, courbe: Courbe) {
        self.courbe = courbe;
    }

    /// Prend ce canal en charge.
    ///
    /// Un canal déjà régulé garde son cache : le réactiver n'est pas un
    /// événement.
    pub fn activer(&mut self, canal: &str) {
        self.canaux.entry(canal.to_owned()).or_insert(None);
    }

    /// Rend ce canal à l'utilisateur.
    ///
    /// ⚠️ **Il oublie ce qu'il avait reçu.** Entre la coupure et une éventuelle
    /// reprise, le canal appartient à l'utilisateur : un `fan <canal> pwm 80` a
    /// pu passer par là, et rien ne le dit à la régulation — le noyau ne
    /// prévient personne. Garder le cache, ce serait réguler un canal en croyant
    /// savoir où il en est.
    pub fn couper(&mut self, canal: &str) {
        self.canaux.remove(canal);
    }

    /// Les canaux régulés, triés.
    pub fn canaux(&self) -> Vec<String> {
        self.canaux.keys().cloned().collect()
    }

    /// Un tour de régulation : `liquide` en millidegrés, `None` si la sonde est
    /// illisible ; `portees` ce que chaque canal **porte réellement**, en
    /// **pourcentage**, relu par le démon avant ce tour.
    ///
    /// Une entrée absente ou `None` veut dire « je ne sais pas ce qu'il porte » —
    /// une quarantaine installée (#68, #88), une lecture qui échoue, un canal que
    /// le démon n'a pas listé. Les trois se valent ici : la régulation ne peut
    /// rien faire de la différence.
    ///
    /// Rend ce qu'il faut écrire, et **seulement** ce qu'il faut écrire : aucune
    /// de ces cibles n'a de watchdog, et réécrire une consigne identique ne fait
    /// que consommer le bus. Le tour passe une fois par seconde — l'écart entre
    /// une régulation qui se tait et une qui réécrit, c'est 86 400 trames par
    /// jour pour rien.
    ///
    /// # La règle, en une ligne
    ///
    /// On écrit si le canal est **lisible** et (**jamais écrit depuis son
    /// activation** ou **ce qu'il porte diffère de la consigne**).
    ///
    /// ⚠️ **La comparaison est exacte, sans tolérance**, et elle peut l'être :
    /// `Percent::from_raw(Percent::raw(p)) == p` sur les 101 valeurs — un point
    /// de pourcentage vaut 2,55 pas de duty, le pas est expansif, donc aucun
    /// arrondi ne replie deux pourcentages sur le même duty. Un écart relu est
    /// donc un écart réel, et une tolérance « au cas où l'arrondi » masquerait
    /// une vraie dérive sans rien corriger.
    ///
    /// ⚠️ **Un canal qu'on ne sait pas relire n'est pas écrit.** Ni écriture à
    /// l'aveugle, ni « une fois pour voir » : écrire là où on ne mesure pas,
    /// c'est refaire #110 à l'identique — annoncer une consigne sans aucun moyen
    /// de savoir si elle a pris.
    ///
    /// ⚠️ **La consigne, elle, ne vient jamais de la relecture.** Sonde muette,
    /// c'est [`REPLI`] et non ce que le canal porte — qui serait précisément « la
    /// dernière valeur connue », lue à la source cette fois.
    pub fn tour(
        &mut self,
        liquide: Option<i32>,
        portees: &BTreeMap<String, Option<u8>>,
    ) -> Vec<Ecriture> {
        // Sans canal régulé, la boucle ne tourne pas : ni écriture, ni repli, ni
        // « juste une fois pour initialiser ». Le démon doit rester au repos
        // absolu quand rien ne l'occupe.
        let consigne = match liquide {
            Some(milli_degres) => self.courbe.consigne(milli_degres),
            None => REPLI,
        };

        let mut ecritures = Vec::new();
        // ⚠️ **Les canaux régulés, jamais ceux de la relecture.** `portees` porte
        // tout le matériel de la machine — le démon le relit pour servir
        // `status` —, y compris les deux canaux du Kraken dont le firmware
        // régule déjà correctement. Le parcourir écraserait une courbe firmware
        // par une boucle hôte.
        for (canal, derniere) in &mut self.canaux {
            let Some(porte) = portees.get(canal).copied().flatten() else {
                continue;
            };
            let jamais_ecrit = derniere.is_none();
            if !jamais_ecrit && porte == consigne {
                continue;
            }
            // Quand les deux motifs se disputent — la consigne change **et** le
            // canal ne l'applique pas —, c'est `Consigne` qui l'emporte : le
            // palier a bougé, c'est l'information neuve. Sans cette règle, un
            // canal durablement bloqué ne journaliserait plus jamais un
            // changement de palier.
            let motif = if *derniere == Some(consigne) {
                Motif::NonAppliquee { porte: Some(porte) }
            } else {
                Motif::Consigne
            };
            *derniere = Some(consigne);
            ecritures.push(Ecriture {
                canal: canal.clone(),
                consigne,
                motif,
            });
        }
        ecritures
    }

    /// Le texte du fichier, en-tête exclu.
    ///
    /// ⚠️ **Le cache d'écriture n'y figure pas**, et ce n'est pas un oubli : rien
    /// ne survit au redémarrage côté matériel (CLAUDE.md), et les canaux
    /// `nzxtsmart2` repartent à `pwm = 64`. Une régulation qui relirait « j'avais
    /// déjà écrit 33 % » se tairait, et laisserait les ventilateurs à 25 %
    /// jusqu'au prochain changement de palier. Ce que le fichier conserve, c'est
    /// l'**intention** — quels canaux, quelle courbe.
    pub fn encoder(&self) -> String {
        let mut texte = String::new();
        // Les mêmes jetons que sur le socket, écrits par le même code : une
        // courbe posée par `regule courbe …` doit se relire au démarrage
        // suivant, et deux écritures d'un même palier divergeraient.
        texte.push_str(&format!(
            "courbe {}\n",
            ipc::encode_paliers(self.courbe.paliers())
        ));
        for canal in self.canaux.keys() {
            texte.push_str(&format!("canal {canal}\n"));
        }
        texte
    }

    /// L'inverse de [`Regulation::encoder`].
    ///
    /// Les lignes vides et celles commençant par `#` sont ignorées ; l'ordre des
    /// lignes est sans importance. Une entrée **absente ou répétée** est refusée
    /// en la nommant : c'est ce qui rend un fichier tronqué détectable, plutôt
    /// que complété au jugé par une régulation plausible et fausse.
    ///
    /// # Erreurs
    ///
    /// Voir [`RegulationInvalide`].
    pub fn decoder(texte: &str) -> Result<Regulation, RegulationInvalide> {
        let mut courbe: Option<Courbe> = None;
        let mut canaux: BTreeMap<String, Option<u8>> = BTreeMap::new();

        for (rang, brut) in texte.lines().enumerate() {
            // Le rang dans le fichier qu'on ouvre, commentaires et lignes vides
            // compris : c'est le numéro qu'affiche un éditeur.
            let ligne = rang + 1;
            let refus = |raison: String| RegulationInvalide { ligne, raison };

            let contenu = brut.trim();
            if contenu.is_empty() || contenu.starts_with('#') {
                continue;
            }
            let jetons: Vec<&str> = contenu.split_whitespace().collect();

            match jetons[0] {
                "courbe" => {
                    if courbe.is_some() {
                        return Err(refus(
                            "une courbe à la fois : « courbe » est donnée deux fois, et c'est \
                             l'ordre de lecture qui déciderait de la vitesse des ventilateurs"
                                .to_owned(),
                        ));
                    }
                    let paliers = &jetons[1..];
                    if paliers.is_empty() {
                        return Err(refus(
                            "« courbe » attend au moins un palier « millidegrés:pourcent »"
                                .to_owned(),
                        ));
                    }
                    let mut lus = Vec::with_capacity(paliers.len());
                    for palier in paliers {
                        lus.push(ipc::parse_palier(palier).map_err(refus)?);
                    }
                    courbe = Some(Courbe::depuis(&lus).map_err(|erreur| refus(erreur.raison))?);
                }

                "canal" => {
                    let [_, canal] = jetons[..] else {
                        return Err(refus(format!(
                            "« canal » attend un nom de canal, et lui seul : « {contenu} »"
                        )));
                    };
                    if canaux.insert(canal.to_owned(), None).is_some() {
                        return Err(refus(format!(
                            "canal « {canal} » donné deux fois : un fichier qui se répète est un \
                             fichier qu'on a mal réécrit"
                        )));
                    }
                }

                autre => {
                    return Err(refus(format!(
                        "« {autre} » n'est pas une ligne de régulation. Lignes attendues : \
                         courbe, canal"
                    )));
                }
            }
        }

        // Ce qui manque n'est écrit nulle part : la faute ne tient à aucune
        // ligne, d'où le numéro 0. Une écriture coupée par une panne de courant
        // laisse exactement ça — un fichier sans sa ligne `courbe`.
        let courbe = courbe.ok_or_else(|| RegulationInvalide {
            ligne: 0,
            raison: "« courbe » absente du fichier : sans elle, aucune consigne ne se calcule"
                .to_owned(),
        })?;

        Ok(Regulation { courbe, canaux })
    }
}

/// Ce qui cloche dans un fichier de régulation.
#[derive(Debug)]
pub struct RegulationInvalide {
    /// Numéro de ligne dans le texte, **à partir de 1** — comme un éditeur.
    /// Vaut 0 quand la faute ne tient à aucune ligne : une entrée absente n'est
    /// écrite nulle part.
    pub ligne: usize,
    pub raison: String,
}

impl fmt::Display for RegulationInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ligne == 0 {
            write!(f, "{}", self.raison)
        } else {
            write!(f, "ligne {} : {}", self.ligne, self.raison)
        }
    }
}

impl std::error::Error for RegulationInvalide {}

/// Lit la régulation à reprendre, ou une régulation vide.
///
/// Ne refuse **jamais** de rendre une régulation, pour la même raison que
/// l'éclairage et la géométrie : un fichier de travers ne doit pas empêcher le
/// démon de démarrer. Un fichier **absent** rend une régulation vide sans rien
/// dire — c'est le premier démarrage, et un message ici polluerait le journal de
/// toute installation neuve. Un fichier illisible ou invalide rend une
/// régulation vide **et** un message : « jamais réglé » et « réglé puis abîmé »
/// ne doivent pas se confondre.
///
/// ⚠️ **L'état rendu est « aucune régulation », et non le repli.** On ne sait pas
/// quels canaux réguler ; poser 50 % sur des canaux qu'on n'a pas su relire
/// serait décider à la place de l'utilisateur, sur un bus qu'on ne sait plus
/// cartographier.
pub fn charger_regulation(chemin: &Path) -> (Regulation, Option<String>) {
    let vide = || Regulation::nouvelle(Courbe::defaut());

    let texte = match fs::read_to_string(chemin) {
        Ok(texte) => texte,
        // L'absence n'est pas une anomalie : c'est le premier démarrage.
        Err(erreur) if erreur.kind() == io::ErrorKind::NotFound => return (vide(), None),
        Err(erreur) => {
            return (
                vide(),
                Some(format!(
                    "régulation illisible dans {} ({erreur}) : aucun canal régulé",
                    chemin.display()
                )),
            );
        }
    };

    match Regulation::decoder(&texte) {
        Ok(regulation) => (regulation, None),
        Err(erreur) => (
            vide(),
            Some(format!(
                "régulation invalide dans {} ({erreur}) : aucun canal régulé",
                chemin.display()
            )),
        ),
    }
}

/// Écrit la régulation, en une fois.
///
/// Même précaution que l'éclairage : fichier temporaire puis renommage, et le
/// dossier parent créé s'il manque. `StateDirectory=reverb` le crée déjà au
/// démarrage du service, mais un premier enregistrement ne doit pas en dépendre.
///
/// # Erreurs
///
/// Toute erreur d'écriture ou de renommage.
pub fn enregistrer_regulation(chemin: &Path, regulation: &Regulation) -> io::Result<()> {
    ecrire(chemin, &format!("{EN_TETE}\n{}\n", regulation.encoder()))
}
