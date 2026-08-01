//! Le dialogue avec le démon, et rien d'autre.
//!
//! La fenêtre n'ouvre **aucun** périphérique et n'écrit **aucun** fichier : ce
//! module est le seul point par lequel elle agit, et il ne parle qu'au socket.
//! Le socket reste ainsi l'unique franchissement de privilège (ADR-002).
//!
//! Deux connexions, et c'est voulu : une pour **regarder** (`watch`, qui ne rend
//! jamais la main) et une pour **agir**. Mêler les deux sur le même fil ferait
//! arriver les réponses au milieu des images, sans qu'on sache laquelle on lit.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use reverb_proto::ipc::{self, Request, ResponseLine};

/// Où le démon écoute, sauf indication contraire.
pub const SOCKET: &str = "/run/reverb/reverbd.sock";

/// Le chemin du socket, tel que l'utilisateur peut le changer.
///
/// L'argument d'abord, puis `REVERB_SOCKET`, puis le chemin d'usine. C'est la
/// même couture de test que celle du démon : sans elle, la fenêtre ne se
/// vérifie qu'avec un service root installé.
pub fn chemin_du_socket() -> PathBuf {
    std::env::args()
        .nth(1)
        .or_else(|| std::env::var("REVERB_SOCKET").ok())
        .map_or_else(|| PathBuf::from(SOCKET), PathBuf::from)
}

/// Une connexion pour donner des ordres.
pub struct Client {
    flux: UnixStream,
}

impl Client {
    pub fn connecter(chemin: &Path) -> io::Result<Client> {
        Ok(Client {
            flux: UnixStream::connect(chemin)?,
        })
    }

    /// Envoie une requête et lit sa réponse jusqu'à la ligne terminale.
    ///
    /// Les lignes illisibles sont **ignorées** plutôt que fatales : une version
    /// du démon plus récente que la fenêtre peut répondre des lignes qu'elle ne
    /// connaît pas encore, et perdre la connexion pour ça serait pire que de
    /// n'en montrer qu'une partie.
    pub fn demander(&mut self, requete: &Request) -> io::Result<Vec<ResponseLine>> {
        writeln!(self.flux, "{}", ipc::encode_request(requete))?;
        self.flux.flush()?;

        let mut lecteur = BufReader::new(self.flux.try_clone()?);
        let mut lignes = Vec::new();
        loop {
            let mut ligne = String::new();
            if lecteur.read_line(&mut ligne)? == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "le démon a fermé la connexion",
                ));
            }
            match ipc::parse_response_line(ligne.trim_end()) {
                Ok(ligne) => {
                    let fin = ligne.is_terminal();
                    lignes.push(ligne);
                    if fin {
                        return Ok(lignes);
                    }
                }
                Err(_) => continue,
            }
        }
    }
}

/// Lit le flux d'images, une image à la fois, jusqu'à la fin de la connexion.
///
/// Rend chaque image complète — les lignes reçues entre deux `end`. Le premier
/// appel bloque jusqu'à ce que le démon pousse quelque chose.
pub struct Abonnement {
    lecteur: BufReader<UnixStream>,
}

impl Abonnement {
    pub fn ouvrir(chemin: &Path) -> io::Result<Abonnement> {
        let mut flux = UnixStream::connect(chemin)?;
        writeln!(flux, "{}", ipc::encode_request(&Request::Watch))?;
        flux.flush()?;
        Ok(Abonnement {
            lecteur: BufReader::new(flux),
        })
    }

    /// L'image suivante, ou `None` quand la connexion se ferme.
    pub fn image_suivante(&mut self) -> Option<Vec<ResponseLine>> {
        let mut image = Vec::new();
        loop {
            let mut ligne = String::new();
            if self.lecteur.read_line(&mut ligne).ok()? == 0 {
                return None;
            }
            match ipc::parse_response_line(ligne.trim_end()) {
                Ok(ResponseLine::End) => return Some(image),
                Ok(ResponseLine::Error { .. }) => return Some(image),
                Ok(ligne) => image.push(ligne),
                Err(_) => continue,
            }
        }
    }
}
