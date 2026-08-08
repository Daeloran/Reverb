//! Les profils : un instantané nommé de l'éclairage, qu'on rappelle.
//!
//! Un profil emporte **trois choses** — la couche globale, les zones, et l'écran
//! —, et pas une quatrième. Il n'emporte notamment **pas la géométrie** : c'est
//! une donnée de montage, décidée une fois, relevée à l'œil sous le bureau, et
//! qui n'a rien à faire dans une ambiance. Rappeler un profil enregistré avant
//! qu'un ventilateur ne soit démonté puis remis remettrait sinon l'orientation
//! d'avant, et le boîtier se mettrait à tourner à l'envers sans qu'on fasse le
//! lien.
//!
//! ## Un fichier par profil
//!
//! Dans [`CHEMIN_PROFILS`], sous `/var/lib` : un profil est de l'**état de
//! service**, réécrit à la demande, jetable. La géométrie reste dans `/etc`.
//!
//! Un fichier par profil plutôt qu'un fichier unique : en supprimer un ne
//! réécrit pas les autres, et un profil corrompu n'emporte pas la collection.
//! [`lister`] ne décode d'ailleurs rien — un profil abîmé reste **visible**,
//! sinon on ne saurait même pas quoi réparer.
//!
//! ## Le format se compose, il ne se recopie pas
//!
//! Les trois natures qu'un profil emporte savent déjà s'écrire et se relire.
//! [`Profil::decoder`] ne réimplémente donc aucun décodeur : il **classe** les
//! lignes par leur premier mot, blanchit celles des autres sections, et délègue.
//!
//! ⚠️ **Le blanchiment conserve les rangs**, et c'est tout l'intérêt : une ligne
//! remplacée par une ligne vide est ignorée par les décodeurs, mais garde sa
//! place. Un refus venu d'`Eclairage::decoder` pointe donc la ligne du fichier
//! que l'utilisateur ouvre, pas celle d'un extrait recomposé.
//!
//! ## Une entrée répétée est refusée, ici plus strictement qu'ailleurs
//!
//! [`Profil::decoder`] détecte les doublons **avant** de déléguer, sur tout le
//! fichier. C'est plus strict que `zones.conf`, qui tolère aujourd'hui deux
//! lignes `light` contradictoires pour une même zone et garde la dernière.
//!
//! La raison tient à ce qu'un profil promet : la fidélité. Deux couleurs
//! contradictoires pour la même zone, c'est une ambiance qu'on ne sait plus
//! reconstituer, et en garder une au hasard de l'ordre des lignes revient à
//! inventer. C'est aussi ce qui rend un fichier tronqué ou concaténé deux fois
//! **détectable**, plutôt que complété au jugé par une ambiance plausible et
//! fausse.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use reverb_proto::NomProfil;

use crate::ecran::{self, Etat as EtatEcran};
use crate::persistance::Eclairage;
use crate::zones::Zones;

/// Le répertoire des profils. Un fichier par profil : `<nom>.conf`.
///
/// Sous `/var/lib`, couvert par le `StateDirectory=reverb` de l'unité systemd.
pub const CHEMIN_PROFILS: &str = "/var/lib/reverb/profils";

/// Un instantané nommé de l'éclairage complet. Jamais la géométrie.
#[derive(Debug, Clone, PartialEq)]
pub struct Profil {
    pub eclairage: Eclairage,
    pub zones: Zones,
    /// `None` : le profil ne dit **rien** de l'écran, et le rappeler n'y touche
    /// pas.
    ///
    /// ⚠️ À distinguer de `Some(Etat { affichage: Affichage::Rien, .. })`, qui
    /// est la consigne « rends la dalle au firmware ». C'est la même distinction
    /// qui a coûté le plus cher à `eclairage.conf` : « un fichier absent et un
    /// fichier disant “noir” ne se confondent jamais ». Les confondre ferait
    /// qu'un profil enregistré écran éteint rallume la dalle, ou qu'un profil
    /// qui ne parlait que d'éclairage l'éteigne sans qu'on l'ait demandé.
    pub ecran: Option<EtatEcran>,
}

/// Un fichier de profil qu'on ne sait pas lire, et où.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfilInvalide {
    /// Numéro de ligne, à partir de 1, comme un éditeur. `0` si la faute ne
    /// tient à aucune ligne — une entrée absente n'est écrite nulle part.
    pub ligne: usize,
    pub raison: String,
}

impl fmt::Display for ProfilInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ligne {} : {}", self.ligne, self.raison)
    }
}

impl std::error::Error for ProfilInvalide {}

/// Ce que [`enregistrer`] a fait, pour que le démon puisse le dire.
///
/// Écraser sans le dire est une ambiance perdue sans que personne l'ait voulu,
/// et c'est la seule occasion de prévenir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecriture {
    Creee,
    Ecrasee,
}

/// Pourquoi un profil ne se charge pas.
///
/// ⚠️ **Absent et illisible ne se confondent pas.** « Introuvable » servi aux
/// deux n'apprend pas si le nom est mal tapé ou si le fichier est là mais abîmé
/// — deux situations qui ne se réparent pas de la même façon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfilRefuse {
    Absent(NomProfil),
    Illisible(NomProfil, ProfilInvalide),
}

impl fmt::Display for ProfilRefuse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfilRefuse::Absent(nom) => write!(f, "aucun profil « {nom} »"),
            ProfilRefuse::Illisible(nom, erreur) => {
                write!(f, "profil « {nom} » illisible : {erreur}")
            }
        }
    }
}

impl std::error::Error for ProfilRefuse {}

/// Ce qu'un profil demande d'appliquer, une fois le disque consulté.
#[derive(Debug, Clone, PartialEq)]
pub struct Application {
    pub eclairage: Eclairage,
    pub zones: Zones,
    /// `None` si le profil ne disait rien, **ou** si ce qu'il désignait n'est
    /// plus affichable.
    pub ecran: Option<EtatEcran>,
    /// Vide quand tout s'applique. Non vide, l'éclairage et les zones
    /// s'appliquent **quand même** : un profil à moitié appliqué qui le dit vaut
    /// mieux qu'un profil refusé en bloc parce qu'une photo a été déplacée.
    pub signalements: Vec<String>,
}

/// À quelle nature appartient une ligne, d'après son premier mot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Eclairage,
    Zones,
    Ecran,
}

/// Les premiers mots que ce format connaît, et ce qu'ils décrivent.
///
/// Source unique : `section_de` en décide, [`Profil::decoder`] s'en sert pour
/// classer **et** pour dire ce qu'il attendait. Une entrée ajoutée ici sans être
/// déléguée plus bas se verrait au premier test.
fn section_de(premier: &str) -> Option<Section> {
    match premier {
        "ventilateur" | "barrette" | "animation" => Some(Section::Eclairage),
        "zone" | "light" | "anim" => Some(Section::Zones),
        // `fond` et `champ` sont les lignes d'une composition d'écran (#80) :
        // elles suivent `affiche layout`, et n'apparaissent jamais sans lui.
        "brightness" | "affiche" | "fond" | "champ" => Some(Section::Ecran),
        _ => None,
    }
}

/// Les mots attendus, pour un refus qui dit quoi écrire.
const MOTS_CONNUS: &str =
    "ventilateur, barrette, animation, zone, light, anim, brightness, affiche, fond, champ";

/// Une ligne d'entrée : ni vide, ni commentaire.
fn est_une_entree(ligne: &str) -> bool {
    let taille = ligne.trim();
    !taille.is_empty() && !taille.starts_with('#')
}

/// Ce qui identifie une entrée, pour détecter qu'elle est donnée deux fois.
///
/// ⚠️ **Le nom en fait partie.** Une clé réduite au premier mot ferait refuser
/// dix ventilateurs comme un seul doublon ; une clé qui l'ignorerait laisserait
/// passer deux couleurs contradictoires pour la même zone. C'est le second cas
/// qui coûte cher : il est indécidable, le fichier portant deux ambiances sans
/// que rien ne dise laquelle.
fn cle_de(jetons: &[&str]) -> String {
    match jetons {
        // Ces entrées se répètent légitimement, une par cible : c'est leur
        // second mot qui les distingue.
        [
            "ventilateur" | "barrette" | "zone" | "light" | "anim" | "champ",
            nom,
            ..,
        ] => {
            format!("{} {nom}", jetons[0])
        }
        // Les autres sont uniques par nature : une animation globale, une
        // luminosité, un affichage.
        [premier, ..] => (*premier).to_owned(),
        [] => String::new(),
    }
}

const EN_TETE: &str = "\
# Profil d'eclairage — Reverb (issue #74)
#
# Un instantane nomme : la couche globale, les zones, et l'ecran. Jamais la
# geometrie, qui est une donnee de montage et vit dans /etc/reverb.
#
# Ecrit par reverbd sur « profil save <nom> », relu sur « profil load <nom> ».

";

impl Profil {
    /// Le texte du fichier.
    ///
    /// Déterministe : deux appels sur le même état rendent les mêmes octets. Un
    /// fichier qui changerait sans que l'ambiance ait bougé rendrait impossible
    /// de dire, six mois plus tard, si le profil a été modifié ou seulement
    /// réenregistré.
    pub fn encoder(&self) -> String {
        let mut texte = String::from(EN_TETE);
        texte.push_str(&self.eclairage.encoder());
        // Les entrées seules : l'en-tête de `zones.conf` parle de son propre
        // fichier, et le recopier ici décrirait un fichier qui n'est pas
        // celui-là.
        texte.push_str(&self.zones.encoder_entrees());
        if let Some(ecran) = &self.ecran {
            texte.push_str(&ecran.encoder());
        }
        texte
    }

    /// L'inverse d'[`Profil::encoder`].
    ///
    /// Trois passes sur les mêmes lignes, dans cet ordre, parce qu'il compte :
    ///
    /// 1. **classer** — un premier mot inconnu est refusé là où il est, avant
    ///    que quoi que ce soit ne soit interprété ;
    /// 2. **détecter les doublons** — en pointant la **seconde** occurrence,
    ///    celle qui est de trop ;
    /// 3. **déléguer** — chaque décodeur reçoit ses lignes, les autres blanchies
    ///    pour que les rangs tiennent.
    pub fn decoder(texte: &str) -> Result<Profil, ProfilInvalide> {
        let lignes: Vec<&str> = texte.lines().collect();
        let mut sections: Vec<Option<Section>> = vec![None; lignes.len()];
        let mut vues: Vec<(String, usize)> = Vec::new();
        let mut ecran_brut: Vec<(usize, &str)> = Vec::new();

        for (rang, brut) in lignes.iter().enumerate() {
            let ligne = rang + 1;
            let contenu = brut.trim();
            if !est_une_entree(contenu) {
                continue;
            }
            let jetons: Vec<&str> = contenu.split_whitespace().collect();

            let Some(section) = section_de(jetons[0]) else {
                // Deviner serait pire que refuser : ce qu'on ne sait pas lire,
                // on ne sait pas non plus le réécrire, et un `profil save`
                // suivant effacerait silencieusement l'entrée incomprise.
                return Err(ProfilInvalide {
                    ligne,
                    raison: format!(
                        "« {} » n'est pas une entrée de profil. Entrées attendues : {MOTS_CONNUS}",
                        jetons[0]
                    ),
                });
            };

            let cle = cle_de(&jetons);
            if let Some((_, premiere)) = vues.iter().find(|(vue, _)| *vue == cle) {
                return Err(ProfilInvalide {
                    ligne,
                    raison: format!(
                        "« {cle} » est donné deux fois, ligne {premiere} et ligne {ligne} : \
                         impossible de savoir laquelle des deux valeurs était la bonne"
                    ),
                });
            }
            vues.push((cle, ligne));

            sections[rang] = Some(section);
            if section == Section::Ecran {
                ecran_brut.push((ligne, contenu));
            }
        }

        let eclairage = Eclairage::decoder(&blanchir(&lignes, &sections, Section::Eclairage))
            .map_err(|erreur| ProfilInvalide {
                ligne: erreur.ligne,
                raison: erreur.raison,
            })?;
        let zones =
            Zones::decoder(&blanchir(&lignes, &sections, Section::Zones)).map_err(|erreur| {
                ProfilInvalide {
                    ligne: erreur.ligne,
                    raison: erreur.raison,
                }
            })?;
        let ecran = decoder_ecran(&ecran_brut)?;

        Ok(Profil {
            eclairage,
            zones,
            ecran,
        })
    }

    /// Ce qu'il y a à appliquer, une fois le disque consulté.
    ///
    /// **Consulte le disque pour l'écran seul, et n'écrit nulle part.** Rien
    /// d'autre dans un profil ne dépend d'un fichier extérieur.
    ///
    /// ⚠️ Le format est reconnu **au contenu**, jamais à l'extension, et avant
    /// que rien ne bouge. Un profil est un second fichier qui décide de ce que
    /// la dalle montre : il hérite du devoir que #69 a payé cher — un affichage
    /// impossible persisté faisait redémarrer le démon dans un état cassé,
    /// indéfiniment.
    pub fn preparer(&self) -> Application {
        let mut signalements = Vec::new();
        let ecran = self.ecran.clone().and_then(|etat| {
            match ecran::verifier_fichier(&etat.affichage) {
                Ok(()) => Some(etat),
                Err(erreur) => {
                    // Un seul signalement, et il nomme le chemin : c'est la
                    // seule information qui permette de le remettre en place.
                    signalements.push(erreur.raison);
                    None
                }
            }
        });

        Application {
            eclairage: self.eclairage.clone(),
            zones: self.zones.clone(),
            ecran,
            signalements,
        }
    }
}

/// Le texte d'une section, les lignes des autres remplacées par des lignes vides.
///
/// ⚠️ **Les rangs sont conservés**, et c'est le seul point qui compte ici : les
/// décodeurs ignorent les lignes vides mais comptent les rangs, si bien qu'un
/// refus pointe la ligne du fichier qu'on ouvre.
fn blanchir(lignes: &[&str], sections: &[Option<Section>], voulue: Section) -> String {
    let mut texte = String::new();
    for (rang, ligne) in lignes.iter().enumerate() {
        // Les commentaires et les lignes vides passent tels quels — ils sont
        // ignorés partout, et les recopier garde le fichier lisible en cas de
        // mise au point.
        if sections[rang] == Some(voulue) || sections[rang].is_none() {
            texte.push_str(ligne);
        }
        texte.push('\n');
    }
    texte
}

/// Les lignes d'écran, ou leur absence.
///
/// `brightness` et `affiche` vont ensemble : une luminosité sans affichage ne
/// dit pas ce que la dalle montre, et un affichage sans luminosité ne dit pas
/// s'il sera visible. Une **composition** y ajoute son `fond` et ses `champ`
/// (#80), qui suivent dans l'ordre où le fichier les porte.
fn decoder_ecran(brut: &[(usize, &str)]) -> Result<Option<EtatEcran>, ProfilInvalide> {
    if brut.is_empty() {
        return Ok(None);
    }

    let trouver = |mot: &str| {
        brut.iter()
            .find(|(_, ligne)| ligne.split_whitespace().next() == Some(mot))
    };
    let (rang_luminosite, luminosite) = trouver("brightness").ok_or(ProfilInvalide {
        ligne: 0,
        raison: "« brightness » absent : le profil dit ce que la dalle montre sans dire si elle \
                 sera visible"
            .to_owned(),
    })?;
    let (rang_affichage, affichage) = trouver("affiche").ok_or(ProfilInvalide {
        ligne: 0,
        raison:
            "« affiche » absent : le profil dit une luminosité sans dire ce que la dalle montre"
                .to_owned(),
    })?;

    // Le bloc d'une composition, dans l'ordre du fichier. Vide quand il n'y en a
    // pas — un profil d'avant #80 n'en porte aucune, et se relit tel quel.
    let bloc: Vec<&(usize, &str)> = brut
        .iter()
        .filter(|(_, ligne)| {
            matches!(
                ligne.split_whitespace().next(),
                Some("fond") | Some("champ")
            )
        })
        .collect();

    // `Etat::decoder` est positionnel — luminosité en 1, affichage en 2, puis la
    // composition. On le nourrit dans cet ordre, puis on **remet le vrai numéro
    // de ligne** : celui de l'extrait n'a aucun sens pour qui ouvre le fichier.
    let mut texte = format!("{luminosite}\n{affichage}\n");
    for (_, ligne) in &bloc {
        texte.push_str(ligne);
        texte.push('\n');
    }

    EtatEcran::decoder(&texte)
        .map(Some)
        .map_err(|erreur| ProfilInvalide {
            ligne: match erreur.ligne {
                1 => *rang_luminosite,
                2 => *rang_affichage,
                // Au-delà, c'est une ligne du bloc : son rang dans le fichier
                // est celui qu'elle y occupe, et non le troisième.
                rang => bloc
                    .get(rang - 3)
                    .map_or(*rang_affichage, |(fichier, _)| *fichier),
            },
            raison: erreur.raison,
        })
}

/// Écrit un profil, en disant s'il en a écrasé un.
pub fn enregistrer(repertoire: &Path, nom: &NomProfil, profil: &Profil) -> io::Result<Ecriture> {
    fs::create_dir_all(repertoire)?;
    let chemin = repertoire.join(nom.fichier());
    // Lu avant d'écrire : après, la question n'a plus de réponse.
    let existait = chemin.exists();
    fs::write(&chemin, profil.encoder())?;
    Ok(if existait {
        Ecriture::Ecrasee
    } else {
        Ecriture::Creee
    })
}

/// Relit un profil enregistré.
pub fn charger(repertoire: &Path, nom: &NomProfil) -> Result<Profil, ProfilRefuse> {
    let texte = fs::read_to_string(repertoire.join(nom.fichier()))
        .map_err(|_| ProfilRefuse::Absent(nom.clone()))?;
    Profil::decoder(&texte).map_err(|erreur| ProfilRefuse::Illisible(nom.clone(), erreur))
}

/// Les noms connus, triés, **sans décoder aucun fichier**.
///
/// Ne décode rien, pour deux raisons : un profil abîmé doit rester listé — sinon
/// on ne saurait pas quoi réparer —, et lister ne doit rien coûter.
///
/// Un répertoire absent donne une liste vide, pas une panne : c'est le premier
/// démarrage, avant le premier `profil save`.
pub fn lister(repertoire: &Path) -> Vec<NomProfil> {
    let Ok(entrees) = fs::read_dir(repertoire) else {
        return Vec::new();
    };
    let mut noms: Vec<NomProfil> = entrees
        .flatten()
        .filter_map(|entree| NomProfil::depuis_fichier(&entree.file_name().to_string_lossy()).ok())
        .collect();
    // `read_dir` rend l'ordre du système de fichiers, qui varie. Une liste qui
    // change d'ordre sans que rien n'ait bougé est une liste qu'on ne peut pas
    // lire.
    noms.sort();
    noms
}

/// Oublie un profil.
///
/// Oublier ce qui n'est pas là est **refusé**, pas passé sous silence : sinon
/// une faute de frappe passe pour une suppression réussie, et le profil visé
/// reste.
pub fn oublier(repertoire: &Path, nom: &NomProfil) -> Result<(), ProfilRefuse> {
    fs::remove_file(repertoire.join(nom.fichier())).map_err(|_| ProfilRefuse::Absent(nom.clone()))
}

/// Les ambiances que le dépôt livre, **embarquées dans le binaire**.
///
/// ⚠️ **Embarquées, et non installées par `tools/installe.sh`.** Le script
/// promet en toutes lettres de ne jamais toucher à `/var/lib/reverb`, et cette
/// promesse protège l'éclairage courant. Y poser des fichiers avant que systemd
/// n'ait créé le répertoire par `StateDirectory=reverb` le créerait de surcroît
/// au mauvais propriétaire.
///
/// Le coût est de deux kilo-octets dans le binaire, et le bénéfice est celui de
/// tout le projet : un binaire unique, rien à installer à côté.
const EXEMPLES: [(&str, &str); 2] = [
    (
        "abysse",
        include_str!("../../../packaging/profils/abysse.conf"),
    ),
    (
        "forge",
        include_str!("../../../packaging/profils/forge.conf"),
    ),
];

/// Pose les exemples, **au tout premier démarrage seulement**.
///
/// La condition est l'absence du répertoire, et non celle de chaque fichier :
/// un exemple qu'on a supprimé exprès ne doit pas repousser au démarrage
/// suivant. Rendre les noms posés, pour que le démon puisse le dire une fois.
///
/// Un échec n'est pas fatal — le démon démarre sans exemples plutôt que pas du
/// tout.
pub fn poser_les_exemples(repertoire: &Path) -> Vec<String> {
    if repertoire.exists() {
        return Vec::new();
    }
    if fs::create_dir_all(repertoire).is_err() {
        return Vec::new();
    }

    let mut poses = Vec::new();
    for (nom, contenu) in EXEMPLES {
        let Ok(nom) = NomProfil::nouveau(nom) else {
            continue;
        };
        if fs::write(repertoire.join(nom.fichier()), contenu).is_ok() {
            poses.push(nom.as_str().to_owned());
        }
    }
    poses
}

#[cfg(test)]
mod tests {
    //! Tests de logique des exemples embarqués (#74).
    //!
    //! Les tests d'intention vérifient que `packaging/profils/` contient des
    //! profils valides. Ceux-ci vérifient qu'ils sont bien **dans le binaire** —
    //! un `include_str!` sur un chemin qui glisserait embarquerait autre chose,
    //! et la faute ne se verrait qu'au premier démarrage d'une machine neuve.

    use super::*;

    #[test]
    fn les_exemples_embarques_sont_ceux_du_depot() {
        for (nom, contenu) in EXEMPLES {
            let profil = Profil::decoder(contenu)
                .unwrap_or_else(|erreur| panic!("l'exemple « {nom} » ne se décode pas : {erreur}"));
            assert_eq!(
                profil
                    .encoder()
                    .lines()
                    .filter(|l| est_une_entree(l))
                    .count(),
                contenu.lines().filter(|l| est_une_entree(l)).count(),
                "l'exemple « {nom} » doit se réencoder sans perdre ni gagner d'entrée"
            );
        }
    }

    #[test]
    fn les_exemples_ne_repoussent_pas_apres_avoir_ete_supprimes() {
        // La condition est l'absence du **répertoire**. Un exemple supprimé
        // exprès dans un répertoire qui existe ne doit pas revenir : ce serait
        // un fichier qu'on ne peut pas jeter.
        let racine = std::env::temp_dir().join(format!("reverb-exemples-{}", std::process::id()));
        let _ = fs::remove_dir_all(&racine);

        assert_eq!(poser_les_exemples(&racine).len(), EXEMPLES.len());
        assert_eq!(lister(&racine).len(), EXEMPLES.len());

        let premier = lister(&racine).remove(0);
        oublier(&racine, &premier).expect("suppression");
        assert!(
            poser_les_exemples(&racine).is_empty(),
            "le répertoire existe : rien ne doit être reposé"
        );
        assert_eq!(lister(&racine).len(), EXEMPLES.len() - 1);

        let _ = fs::remove_dir_all(&racine);
    }
}
