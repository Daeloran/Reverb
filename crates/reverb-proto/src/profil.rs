//! Le nom d'un profil d'éclairage.
//!
//! Un profil est un instantané nommé de l'éclairage, enregistré sous
//! `/var/lib/reverb/profils/<nom>.conf`. Ce module ne connaît ni le contenu d'un
//! profil ni le disque : il ne porte que **le nom**, et la garantie qui va avec.
//!
//! ## Pourquoi le nom vit ici, et pas dans le démon
//!
//! Le démon tourne en `User=root` (ADR-002). Un nom qui traverserait jusqu'à
//! `open()` sans avoir été borné serait une écriture arbitraire sur le système
//! de fichiers : `../../etc/reverb/geometrie.conf` effacerait un relevé fait au
//! sol, et pire ailleurs.
//!
//! Le remède est celui de [`crate::ram::SlotAddress`] : viser hors du répertoire
//! n'est pas refusé à l'exécution, c'est **irreprésentable**. [`NomProfil`] ne se
//! construit que par [`NomProfil::nouveau`], et son [`NomProfil::fichier`] rend un
//! nom de fichier — jamais un chemin. Le démon n'a donc aucun moyen d'en composer
//! un qui sorte de son répertoire, et le garde-fou se vérifie ici, dans un crate
//! pur, par un balayage des 256 valeurs d'octet.
//!
//! ## Accepté tel quel, ou refusé — jamais nettoyé
//!
//! ⚠️ La façon dont ce garde-fou cède habituellement n'est pas de laisser passer
//! un nom hostile : c'est de le **nettoyer**. Retirer les `/` transforme
//! « a/../b » en « a..b », donne un chemin impeccable, et fait pointer deux
//! profils différents sur le même fichier. D'où la règle : un nom accepté est
//! rendu à l'octet près.
//!
//! ## Ce qui est accepté
//!
//! Reverb est un outil personnel dont l'ergonomie est la raison d'être. Une
//! liste blanche `[a-z0-9-]` tiendrait le garde-fou et rendrait l'outil
//! désagréable : « Noël » doit s'écrire « Noël », et « soirée d'été » est un nom
//! légitime. Ce module refuse donc une **liste courte de fautes nommées**, et
//! accepte tout le reste.

use std::fmt;

/// Le nom d'un profil, validé.
///
/// Ne se construit que par [`NomProfil::nouveau`]. Le type est la garantie : là
/// où il apparaît, le nom a été borné.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NomProfil(String);

/// Un nom refusé, et pourquoi.
///
/// Porte le nom **tel qu'il a été saisi** : un message qui montrerait autre
/// chose que ce que l'utilisateur a tapé le laisserait chercher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NomInvalide {
    /// Ce qui a été donné, sans réécriture.
    pub saisi: String,
    /// Ce qui cloche, en une phrase citable.
    ///
    /// ⚠️ **Les raisons se distinguent.** Un unique « nom de profil invalide »
    /// servi à toutes les fautes *nomme* quelque chose et n'apprend rien : un
    /// octet nul et un nom vide ne se corrigent pas de la même façon.
    pub raison: String,
}

impl fmt::Display for NomInvalide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `escape_debug` parce que le nom fautif contient justement, souvent, ce
        // qui ne s'affiche pas — et un message tronqué par un `\r` de la saisie
        // serait le comble pour un refus qui vise les caractères de contrôle.
        write!(
            f,
            "« {} » n'est pas un nom de profil : {}",
            self.saisi.escape_debug(),
            self.raison
        )
    }
}

impl std::error::Error for NomInvalide {}

impl NomProfil {
    /// Borne haute d'un nom, en **octets**.
    ///
    /// C'est ce que compte le système de fichiers : un compte en `chars()`
    /// laisserait passer un nom d'accents au double de la borne.
    ///
    /// Deux plafonds l'encadrent, et 200 tient sous les deux avec de la marge :
    ///
    /// - `NAME_MAX` vaut 255 octets, extension comprise ;
    /// - la requête `profil load <nom>` doit tenir sur une ligne du socket,
    ///   bornée par [`crate::ipc::MAX_LINE_LEN`].
    ///
    /// Sans borne, l'écriture échouerait sur un `ENAMETOOLONG` au moment le plus
    /// inutile — après que l'utilisateur a composé son ambiance —, avec une
    /// erreur que rien ne relie au nom qu'il a tapé.
    pub const MAX_OCTETS: usize = 200;

    /// Le suffixe du fichier, extension comprise.
    pub const EXTENSION: &'static str = ".conf";

    /// Valide un nom saisi.
    ///
    /// L'ordre des vérifications n'est pas indifférent : `..` passe **avant**
    /// les séparateurs, sans quoi « ../../etc/passwd » serait refusé en citant
    /// `/`, alors que ce qui le rend dangereux est le `..`.
    pub fn nouveau(saisi: &str) -> Result<NomProfil, NomInvalide> {
        let refus = |raison: String| NomInvalide {
            saisi: saisi.to_owned(),
            raison,
        };

        // Un profil sans nom ne se rappelle pas : `profil load` ne peut pas le
        // désigner, et le fichier « .conf » qu'il produirait serait invisible
        // dans un `ls`. Un nom fait d'espaces est le même profil sans nom,
        // écrit autrement — et il ne se retape pas.
        if saisi.trim().is_empty() {
            return Err(refus(
                "un nom vide ne désigne aucun profil et ne se retape pas".to_owned(),
            ));
        }

        if saisi.len() > Self::MAX_OCTETS {
            return Err(refus(format!(
                "{} octets, maximum {} — c'est la place que laisse un nom de fichier",
                saisi.len(),
                Self::MAX_OCTETS
            )));
        }

        // ⚠️ Toute la famille des caractères de contrôle, pas seulement `\0` et
        // `\n`. Le nom voyage sur un protocole **texte, une ligne par requête**
        // (ADR-002), où un `\r` tronque une ligne aussi bien qu'un `\n`, et
        // finit dans un nom de fichier écrit par root, où une tabulation rend le
        // fichier intypable. Refuser la famille entière se tient mieux qu'une
        // liste de deux.
        if let Some(controle) = saisi.chars().find(|c| c.is_control()) {
            return Err(refus(format!(
                "le caractère de contrôle U+{:04X} ne peut pas figurer dans un nom de fichier",
                controle as u32
            )));
        }

        // Avant les séparateurs : c'est le `..` qui fait sortir du répertoire.
        // Refusé **partout**, pas seulement seul — accepter « avant..apres »
        // coûterait de prouver quelque chose sur son interaction avec le suffixe
        // ajouté, quand le refuser ne coûte qu'un nom que personne n'écrit.
        if saisi.contains("..") {
            return Err(refus(
                "« .. » désigne le répertoire parent : un profil ne s'écrit pas ailleurs que \
                 dans le sien"
                    .to_owned(),
            ));
        }

        for separateur in ['/', '\\'] {
            if saisi.contains(separateur) {
                return Err(refus(format!(
                    "« {separateur} » sépare les répertoires : un nom de profil est un nom, pas \
                     un chemin"
                )));
            }
        }

        Ok(NomProfil(saisi.to_owned()))
    }

    /// Le nom, tel qu'il a été saisi.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Le **nom de fichier** correspondant — jamais un chemin.
    ///
    /// Aucun séparateur ne peut y figurer : c'est [`NomProfil::nouveau`] qui
    /// l'a garanti, et c'est ce qui rend `répertoire.join(nom.fichier())`
    /// incapable de sortir du répertoire.
    pub fn fichier(&self) -> String {
        format!("{}{}", self.0, Self::EXTENSION)
    }

    /// L'inverse de [`NomProfil::fichier`], **strict**.
    ///
    /// `profil list` lit un répertoire et rend des noms. Il lui faut donc
    /// l'inverse — et il doit être strict : un fichier posé à la main dans
    /// `/var/lib/reverb/profils` ne devient pas un profil au seul motif qu'il
    /// traîne là. Un `nuit.conf.tmp` laissé par une écriture interrompue n'est
    /// pas un profil.
    pub fn depuis_fichier(fichier: &str) -> Result<NomProfil, NomInvalide> {
        let nom = fichier.strip_suffix(Self::EXTENSION).ok_or(NomInvalide {
            saisi: fichier.to_owned(),
            raison: format!(
                "un fichier de profil se termine par « {} »",
                Self::EXTENSION
            ),
        })?;
        Self::nouveau(nom)
    }
}

impl fmt::Display for NomProfil {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
