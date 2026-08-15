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
//! # Une consigne ne suit pas le bruit de la sonde (issue #111)
//!
//! Le 2026-08-15, régulation de #99 active depuis des heures, machine au repos :
//!
//! ```text
//! 15:51:10  nzxtsmart2:fan-{1,2,3} à 46 %   (liquide 40.4 °C)
//! 15:51:48  nzxtsmart2:fan-{1,2,3} à 45 %   (liquide 40.1 °C)
//! 15:52:28  nzxtsmart2:fan-{1,2,3} à 46 %   (liquide 40.2 °C)
//! ```
//!
//! **Trente écritures en huit minutes** pour 0,3 °C d'amplitude, soit ~3 500 par
//! jour et par canal, sur un contrôleur qui a déjà montré qu'il n'aimait pas les
//! écritures rapprochées.
//!
//! ⚠️ **La règle de #99 était respectée à la lettre et ratée en esprit.** « On
//! n'écrit que ce qui change » est vrai — la consigne *change* vraiment, la
//! sonde bruite de ±0,3 °C et la courbe fait 3 %/°C —, et la régulation ne se
//! tait jamais pour autant.
//!
//! D'où [`HYSTERESIS`] : ce n'est plus l'existence de l'écart qui décide, c'est
//! sa **taille**. Voir `merite_une_ecriture` pour la grandeur mesurée et les
//! exemptions.
//!
//! # Millidegrés partout, jamais de degrés flottants
//!
//! `Sonde::lire` rend des millidegrés entiers, comme `hwmon` ; une conversion
//! vers `f32` en chemin rendrait la courbe dépendante d'un arrondi. Le projet a
//! déjà payé ce prix une fois — la symétrie des directions locales est calculée
//! sur les **indices**, jamais sur une position flottante (#75). Deux unités
//! dans la même API seraient pire encore : c'est la faute des trois ordres de
//! composantes, qui ne produit aucun message et juste un résultat faux.
//!
//! # Où vit la courbe, depuis #113
//!
//! Dans [`reverb_proto::regulation`], et ce module la **ré-exporte**. La fenêtre
//! trace désormais la courbe qu'elle édite, et l'issue #113 exige que ce tracé
//! vienne de la même fonction que celle qu'exécute le démon — or `reverb-gui` ne
//! dépend pas de `reverb-daemon`. Le crate partagé est donc le seul endroit d'où
//! les deux peuvent la lire.
//!
//! ⚠️ **Une ré-exportation, jamais une copie.** Deux définitions jumelles
//! divergeraient au premier palier corrigé d'un seul côté, et rien ne le dirait :
//! la fenêtre refuserait une courbe que le socket accepte, ou l'inverse. C'est
//! aussi ce qui laisse `tests/spec_regulation_hote.rs`, le fichier d'intention de
//! #99, compiler sans une ligne de changement.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use reverb_proto::ipc;

pub use reverb_proto::regulation::{Courbe, CourbeInvalide};

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

/// L'écart de consigne, en points de pourcentage, à partir duquel une
/// réécriture part (issue #111).
///
/// La sonde du liquide bruite de ±0,1 à 0,3 °C d'une lecture à l'autre ; la
/// courbe fait 3 %/°C sur ce segment, donc ce bruit vaut **±1 point de
/// consigne**. Mesuré sur SHYNAEL le 2026-08-15, machine au repos, régulation
/// active depuis des heures : trente écritures en huit minutes sur les trois
/// canaux, pour 0,3 °C d'amplitude.
///
/// ⚠️ **Le seuil est atteint, pas dépassé** : on écrit dès que l'écart vaut
/// `HYSTERESIS`. Un seuil « strictement supérieur » ferait de 2 un seuil de 3
/// en pratique, et le chiffre écrit ici ne voudrait plus dire ce qu'il dit.
///
/// Deux points suffisent à faire taire le bruit relevé, et laissent passer
/// toute variation dépassant 0,7 °C de liquide — soit très en deçà d'une vraie
/// montée en charge, qui a mis quarante minutes à gagner quinze degrés le
/// 2026-08-15. Plus large, l'hystérésis se mettrait à retarder cette montée-là.
pub const HYSTERESIS: u8 = 2;

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

/// L'écart entre ce qu'un canal **porte** et la consigne justifie-t-il de lui
/// écrire ? (issue #111)
///
/// La grandeur est `|porté − consigne|`, **jamais** `|consigne − dernière
/// écrite|`. Trois raisons, dans cet ordre :
///
/// - **elle compose avec #110**. Un canal bloqué à 25 % pour une consigne de
///   46 % montre vingt et un points d'écart : il reste réécrit à chaque tour, et
///   la réparation automatique tient. Comparée à l'intention, l'hystérésis le
///   rendrait **muet** dès le premier tour de bruit — la consigne repasserait de
///   46 à 45 sans jamais s'écarter d'un point de la dernière écrite, et un canal
///   bloqué redeviendrait invisible ;
/// - **elle n'ajoute aucun état**. Elle se calcule des deux arguments de
///   [`Regulation::tour`], là où l'autre demanderait de retenir la dernière
///   consigne *calculée* en plus du cache d'activation — deux mémoires qui se
///   ressemblent finissent par diverger ;
/// - **c'est la grandeur que l'utilisateur subit** : non pas de combien la
///   consigne a bougé, mais de combien le ventilateur est loin de ce qu'on lui
///   demande.
///
/// Trois exemptions, et exactement trois. La première n'est pas ici — « jamais
/// écrit depuis son activation » se décide sur le cache, avant d'appeler cette
/// fonction. Restent :
///
/// - ⚠️ **les bornes**. Une consigne de 0 % ou de 100 % part dès qu'elle diffère,
///   sans seuil : c'est le sens même d'une borne. « À fond » ne doit pas
///   s'arrêter à 99 — le README promet 100 % au-delà de 50 °C —, ni « à
///   l'arrêt » s'immobiliser à 1 % pour toujours. **Quitter** une borne, en
///   revanche, est une consigne ordinaire : rester une seconde de plus à plein
///   régime ne coûte rien, ne jamais y arriver si ;
/// - ⚠️ **le repli d'une sonde muette** ([`REPLI`], `liquide` à `None`). C'est la
///   seule écriture du système qui **signale une panne** : le liquide illisible
///   veut dire que le Kraken est en difficulté, donc que plus rien ne mesure la
///   température du circuit. L'amortir la rendrait indistincte d'un régime
///   normal. L'exemption ne devient pas « on écrit à chaque tour » pour autant —
///   un canal qui porte déjà le repli n'a plus rien à recevoir.
fn merite_une_ecriture(porte: u8, consigne: u8, sonde_muette: bool) -> bool {
    if porte == consigne {
        return false;
    }
    let sans_seuil = sonde_muette || consigne == 0 || consigne == 100;
    // ⚠️ Le seuil est **atteint**, pas dépassé : `>=`, et non `>`. Un seuil
    // strictement supérieur ferait de 2 un seuil de 3 en pratique, et le chiffre
    // écrit dans `HYSTERESIS` ne voudrait plus dire ce qu'il dit.
    //
    // `abs_diff` et non une soustraction : les deux sens du bruit comptent, et
    // une erreur de signe ne produit aucun message — juste un ventilateur qui ne
    // redescend jamais.
    sans_seuil || porte.abs_diff(consigne) >= HYSTERESIS
}

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
    /// activation** ou **l'écart entre ce qu'il porte et la consigne mérite une
    /// écriture**) — voir `merite_une_ecriture`.
    ///
    /// ⚠️ **La comparaison reste exacte, mais elle n'est plus le critère**
    /// (issue #111). Un écart relu est bien un écart réel — `Percent` ne replie
    /// jamais deux pourcentages sur le même duty, un point valant 2,55 pas — et
    /// c'est justement pourquoi le bruit de la sonde passait entier : la
    /// consigne *changeait* vraiment d'un point, et la régulation ne se taisait
    /// jamais pour autant. Ce qui décide désormais est la **taille** de l'écart,
    /// pas son existence.
    ///
    /// ⚠️ **Contrepartie assumée : sous le seuil, un bruit d'un point et une
    /// non-application d'un point deviennent indistinguables.** Un canal à qui
    /// on a écrit 46 et qui n'en porte que 45 n'est plus rejoué. Les deux
    /// situations produisent littéralement la même lecture, et rien dans
    /// `portees` ne permet de les séparer. Le prix est d'**un point de duty sur
    /// 255** ; le gain, les 86 400 trames par jour que #99 invoquait déjà.
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
            if !jamais_ecrit && !merite_une_ecriture(porte, consigne, liquide.is_none()) {
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
