//! Tests d'intention du délai de garde des questions HID — issue #83, sixième comportement.
//!
//! Écrits **avant** le correctif, depuis le relevé fait sur SHYNAEL le 2026-08-08 et la seule
//! signature publique de la fonction visée. `crates/reverb-hw/src/hidraw.rs` n'a **pas** été lu
//! pour les produire. Si l'un de ces tests échoue après le correctif, c'est le code qu'on corrige.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! **Le démon est resté bloqué vingt minutes, zéro tic de CPU sur tous ses fils**, pendant que cinq
//! clients attendaient une réponse qui n'est jamais venue. `status` : connexion acceptée, requête
//! envoyée, **zéro octet** après quinze secondes. Le Kraken était toujours énuméré, et muet.
//!
//! La cause est une borne qui compte des **trames** et jamais du **temps** :
//!
//! ```ignore
//! const MAX_LECTURES: usize = 20;
//! // « Vingt lectures … sans jamais bloquer indéfiniment. »
//!
//! for _ in 0..MAX_LECTURES {
//!     let lus = fichier.read(&mut reponse)?;   // aucun délai de garde
//!     …
//! }
//! ```
//!
//! Le descripteur est ouvert en mode bloquant : un périphérique qui n'émet plus rien ne fait pas
//! **échouer** la lecture, il la fait **attendre**. Vingt lectures dont la première ne revient
//! jamais, c'est une attente infinie déguisée en boucle bornée. Le commentaire disait déjà le
//! contraire de ce que le code faisait — c'est ce qui rend ce défaut si durable.
//!
//! `usbfs` a ses cinq secondes (`usbfs.rs`, `TIMEOUT_MS`) et le pilote noyau les siennes : `ask`
//! est le **seul** appel non borné du chemin de l'écran.
//!
//! ## Ce que ce défaut coûte à #83, et pourquoi il en fait partie
//!
//! Le fil dédié spécifié par `crates/reverb-daemon/tests/spec_fil_ecran.rs` rend aux LED et au
//! socket leur indépendance — mais **le fil de l'écran, lui, se bloquerait à vie dès la première
//! image**. La vigie de #70 ne compterait jamais un échec, puisque la fermeture d'écriture ne
//! rendrait jamais la main : pas de refus, pas d'abandon, pas de journal. La dalle resterait noire
//! sans que rien ne le dise, et c'est très exactement le mode de défaillance que #70 existait pour
//! supprimer.
//!
//! ## La couture que ces tests exigent
//!
//! ```ignore
//! // crates/reverb-hw/src/hidraw.rs
//!
//! use std::time::Duration;
//!
//! /// Le temps qu'on laisse à **une trame** pour arriver.
//! pub const DELAI_LECTURE: Duration = /* … */;
//!
//! /// Le nombre de trames qu'une question consent à écarter avant d'abandonner.
//! /// Déjà là, mais privée : elle devient publique, parce qu'elle borne le temps
//! /// total avec la précédente et que ce produit est ce que l'appelant doit savoir.
//! pub const MAX_LECTURES: usize = 20;
//!
//! pub fn ask(path: &Path, question: &Frame, attendu: &[u8]) -> io::Result<Frame>;
//! ```
//!
//! **Deux constantes publiques plutôt qu'un nombre enfoui.** Un test qui écrirait « moins de deux
//! secondes » figerait un réglage que la mesure sur le matériel doit pouvoir déplacer ; en
//! raisonnant sur `DELAI_LECTURE`, ce fichier encadre le **comportement** et laisse la valeur
//! libre — comme `spec_plafond.rs` (#70) encadre `ECHECS_AVANT_ABANDON` au lieu de le pointer.
//!
//! ## L'arbitrage que ce fichier tranche : par lecture, et non global
//!
//! ⚠️ Deux corrections étaient possibles, et [`le_bavardage_n_arrete_pas_une_question_meme_quand_il_dure`]
//! choisit :
//!
//! | | ce que ça borne | ce que ça casse |
//! |---|---|---|
//! | délai **global** | toute la question à `DELAI_LECTURE` | un périphérique vivant mais bavard se voit déclaré mort |
//! | délai **par lecture** ← retenu | l'attente d'**une** trame ; le total par `DELAI_LECTURE × MAX_LECTURES` | un périphérique qui parle sans jamais répondre tient jusqu'au produit des deux |
//!
//! Le délai par lecture est retenu parce qu'un faux abandon coûte plus cher qu'une attente
//! généreuse : sous la vigie de #70, trois délais dépassés sur la poignée de main d'image
//! (`36 01` → `37 01`, spec §3.2) **rendent la dalle au firmware**. Une borne prudente qui ne peut
//! pas mentir vaut mieux qu'une borne serrée qui le peut — d'autant que la panne observée est le
//! **silence total**, que les deux corrections traitent, tandis que le bavardage sans réponse n'a
//! jamais été observé.
//!
//! Le prix est nommé, pas caché : le pire cas devient `DELAI_LECTURE × MAX_LECTURES`, et
//! [`les_reperes_de_ce_fichier_ne_sont_aucun_defaut`] exige que ce produit reste sous les trente
//! secondes au bout desquelles le firmware reprend la dalle de toute façon
//! ([`screen::FIRMWARE_FALLBACK_SECS`]) — au-delà, insister ne sert plus à rien.
//!
//! ## Le périphérique muet, sans matériel
//!
//! Un **tube nommé**. `std` ne sait pas en créer, `mkfifo` si — et c'est acceptable dans un test,
//! jamais dans du code de production. Ouvert en lecture **et** en écriture, il ne bloque pas à
//! l'ouverture ; la question qu'on y écrit y revient en lecture, sans correspondre à l'en-tête
//! attendu ; et la lecture suivante attend. C'est, trait pour trait, un périphérique qui a cessé
//! d'émettre — sans avoir à débrancher un Kraken pour le reproduire.
//!
//! Chaque tube porte le numéro du processus et un compteur, pour que deux exécutions parallèles ne
//! se marchent pas dessus, et il est **effacé même quand le test échoue** — c'est un `Drop`, pas
//! une ligne en fin de fonction.
//!
//! ## Une divergence relevée, et non tranchée ici
//!
//! ⚠️ La demande initiale citait « la spec §7.1 : le Kraken émet un rapport spontané `75 02` chaque
//! seconde ». **Cette trame n'est dans aucune des trois spécifications** : `SPEC-KRAKEN-LCD.md` n'a
//! pas de §7, et `75` n'apparaît dans le dépôt que comme une entrée « reponse d etat » de la table
//! de décodage de `tools/extrait_kraken.py`, sans trame associée. Le CLAUDE.md interdit d'inventer
//! une trame absente des specs.
//!
//! Le bavardage de ce fichier est donc pris **là où il est documenté** : `67 02` (rapport d'état des
//! ventilateurs, `SPEC-PROTOCOLE-NZXT.md` §7.1, ✅) et `ff 01` (accusé, §7.2, ✅). Ils jouent
//! exactement le même rôle — des trames non sollicitées, sur l'endpoint IN, qui ne portent pas
//! l'en-tête attendu — et ils ont l'avantage d'exister.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **La valeur du délai.** Elle se mesure sur le matériel, pas dans un test d'intention. Seul son
//!   encadrement est fixé, et ses deux bornes sont raisonnées.
//! - **Le vrai matériel.** Aucune écriture sur un `/dev/hidraw*`, aucun périphérique ouvert : la
//!   règle du projet est dure, et un test qui allume une LED est un test qu'on lance à la main.
//! - **Les autres appels de `hidraw`** — l'écriture simple d'une trame ne lit rien et n'attend rien.

// Les assertions qui **encadrent** `DELAI_LECTURE` sont constantes une fois la constante écrite, et
// clippy les refuse à ce titre — même `allow` que `spec_plafond.rs` (#70), pour la même raison.
// Leur intérêt n'est pas d'observer une exécution mais de casser la compilation le jour où le délai
// dégénérerait au point de vider ce fichier de son sens.
#![allow(clippy::assertions_on_constants)]

use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use reverb_hw::hidraw::{self, DELAI_LECTURE, MAX_LECTURES};
use reverb_proto::screen;
use reverb_proto::{FRAME_LEN, Frame};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// Le temps qu'on laisse à un appel avant de déclarer qu'il ne rendra jamais la main.
///
/// Ce n'est pas une mesure, c'est un filet : il transforme le blocage à vie du 2026-08-08 en une
/// assertion qui parle, au lieu d'une suite de tests qui pend. Il doit rester très au-dessus du
/// pire cas légitime — `DELAI_LECTURE` pour une question muette, environ deux fois et quart pour
/// celle du bavardage.
const PATIENCE: Duration = Duration::from_secs(10);

/// Le plancher défendable du délai de lecture.
///
/// La poignée de main d'image est acquittée en 3 ms, et la validation en 18 ms (spec §3.2). Un
/// quart de seconde laisse plus de dix fois le pire temps relevé : en dessous, on transformerait
/// une lenteur passagère en panne, et la vigie de #70 rendrait la dalle au firmware pour rien.
const DELAI_MINIMAL: Duration = Duration::from_millis(250);

/// L'en-tête de la réponse attendue à une demande d'état d'écran.
///
/// `30 01` en question, `31 01` en réponse — spec Kraken §3.7, ✅. C'est la paire qu'emploie déjà
/// `reverb screen` pour relire la luminosité.
const ATTENDU: [u8; 2] = [0x31, 0x01];

/// Le nombre de trames de bavardage qui précèdent la réponse dans le test qui dure.
///
/// Deux suffisent : avec un écart de trois quarts de délai, elles portent le total à deux fois et
/// quart un `DELAI_LECTURE`, ce qui est déjà plus qu'un délai global n'en accorderait.
const BAVARDAGES: u32 = 2;

/// Un compteur, pour que deux tubes de la même exécution ne portent jamais le même nom.
static COMPTEUR: AtomicU32 = AtomicU32::new(0);

/// Construit une trame de 64 octets à partir de ses octets significatifs, complétée par des zéros.
///
/// Spec §1 — « Les paquets sont toujours complétés à 64 octets par des zéros. » `reverb-proto` a la
/// même fonction, mais elle y est `pub(crate)`.
fn trame(tete: &[u8]) -> Frame {
    let mut frame = [0u8; FRAME_LEN];
    frame[..tete.len()].copy_from_slice(tete);
    frame
}

/// La réponse `31 01` à une demande d'état d'écran, recopiée telle quelle de la spec Kraken §3.7 :
///
/// ```text
/// 31 01 bb 8c 90 82 0e 90 06 30 00 00 00 00 05 00 80 00 00 10 80 02 80 02 50 01 00 ff …
/// ```
///
/// Largeur et hauteur `80 02` = 640 aux offsets `0x14` et `0x16`, luminosité `50` = 80 % à `0x18`.
/// C'est une vraie trame, relevée le 2026-07-31, et non une suite d'octets plausibles.
fn reponse_attendue() -> Frame {
    trame(&[
        0x31, 0x01, 0xbb, 0x8c, 0x90, 0x82, 0x0e, 0x90, 0x06, 0x30, 0x00, 0x00, 0x00, 0x00, 0x05,
        0x00, 0x80, 0x00, 0x00, 0x10, 0x80, 0x02, 0x80, 0x02, 0x50, 0x01, 0x00, 0xff,
    ])
}

/// Le rapport d'état des ventilateurs, émis sans qu'on l'ait demandé.
///
/// `SPEC-PROTOCOLE-NZXT.md` §7.1, ✅ — les seize premiers octets sont recopiés de la capture :
///
/// ```text
/// 0: 67 02 0a f0 03 13 29 95 ad aa be 94 04 61 03 ff
/// ```
fn bavardage_etat() -> Frame {
    trame(&[
        0x67, 0x02, 0x0a, 0xf0, 0x03, 0x13, 0x29, 0x95, 0xad, 0xaa, 0xbe, 0x94, 0x04, 0x61, 0x03,
        0xff,
    ])
}

/// Un accusé de réception, qui arrive lui aussi sans être la réponse qu'on attend.
///
/// `SPEC-PROTOCOLE-NZXT.md` §7.2, ✅ — « `ff 01 <constante 12 octets> 2a 04` », l'identifiant
/// acquitté étant réémis en fin de trame.
fn bavardage_accuse() -> Frame {
    let mut frame = trame(&[0xff, 0x01]);
    frame[14] = 0x2a;
    frame[15] = 0x04;
    frame
}

/// Vrai si le message nomme la suite d'octets attendue, quelle que soit sa typographie.
///
/// L'issue exige que l'en-tête soit **lisible** dans l'erreur, elle n'en fixe pas l'écriture : ce
/// test accepte donc `31 01`, `0x31 0x01`, `[0x31, 0x31]` ou `3101`, et refuse un message qui ne le
/// nomme pas du tout. Un test d'intention ne fige pas un goût — mais il exige qu'on puisse relier
/// une ligne de journal à l'étape du protocole qui a échoué.
fn nomme_l_en_tete(message: &str, en_tete: &[u8]) -> bool {
    let condense: String = message
        .to_lowercase()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    let attendu: String = en_tete.iter().map(|octet| format!("{octet:02x}")).collect();
    condense.contains(&attendu) || condense.replace("0x", "").contains(&attendu)
}

// ---------------------------------------------------------------------------
// Le tube nommé — un périphérique qui n'émet plus rien
// ---------------------------------------------------------------------------

/// Un tube nommé, effacé quoi qu'il arrive.
struct Tube {
    chemin: PathBuf,
    /// Un bout ouvert en lecture **et** en écriture, gardé par le test.
    ///
    /// Il tient les deux extrémités ouvertes : sans lui, la fermeture du descripteur de `ask`
    /// donnerait une fin de fichier, c'est-à-dire une lecture qui **rend la main** — soit
    /// exactement ce qu'on veut empêcher de se produire par accident.
    bout: File,
}

impl Tube {
    /// Crée un tube nommé, ou rend `None` en le disant si `mkfifo` manque.
    ///
    /// Sauter proprement plutôt qu'échouer : un test rouge pour une raison qui n'est pas la sienne
    /// coûte le temps de découvrir que ce n'était pas la bonne.
    fn neuf(nom: &str) -> Option<Tube> {
        let numero = COMPTEUR.fetch_add(1, Ordering::Relaxed);
        let chemin = std::env::temp_dir().join(format!(
            "reverb-83-{}-{numero}-{nom}.fifo",
            std::process::id()
        ));
        let _ = fs::remove_file(&chemin);

        match Command::new("mkfifo").arg(&chemin).status() {
            Ok(etat) if etat.success() => {}
            Ok(etat) => panic!(
                "`mkfifo {}` a échoué ({etat}) : le tube du test n'a pas pu être créé",
                chemin.display()
            ),
            Err(erreur) if erreur.kind() == io::ErrorKind::NotFound => {
                eprintln!(
                    "test « {nom} » sauté : `mkfifo` est introuvable, et c'est le seul moyen de \
                     simuler un périphérique muet sans matériel"
                );
                return None;
            }
            Err(erreur) => panic!("`mkfifo` n'a pas pu être lancé : {erreur}"),
        }

        match OpenOptions::new().read(true).write(true).open(&chemin) {
            Ok(bout) => Some(Tube { chemin, bout }),
            Err(erreur) => {
                let _ = fs::remove_file(&chemin);
                panic!(
                    "un tube nommé s'ouvre en lecture+écriture sans bloquer ; celui-ci a refusé : \
                     {erreur}"
                );
            }
        }
    }

    /// Le chemin à passer à la fonction interrogée.
    fn chemin(&self) -> &Path {
        &self.chemin
    }

    /// Dépose une trame dans le tube. Soixante-quatre octets tiennent dans un tube sans être
    /// coupés, donc une trame déposée est une trame lue d'un bloc.
    fn deposer(&self, frame: &Frame) {
        (&self.bout)
            .write_all(frame)
            .expect("écrire 64 octets dans un tube ouvert ne peut pas échouer");
    }
}

impl Drop for Tube {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.chemin);
    }
}

// ---------------------------------------------------------------------------
// Interroger sans se laisser prendre
// ---------------------------------------------------------------------------

/// Pose la question dans un fil à part, et rend son résultat et sa durée.
///
/// C'est le filet du fichier. Le défaut visé est **un blocage à vie** : un test qui appellerait
/// `ask` depuis son propre fil ne serait pas rouge, il serait **suspendu** — et une suite qui pend
/// est une suite qu'on finit par lancer avec `--skip`, ce qui est la façon la plus discrète de
/// perdre une garantie.
///
/// Le fil déporté reste bloqué pour toujours si le correctif n'est pas là. C'est assumé : le test a
/// déjà échoué, et le processus s'arrête juste après.
fn interroger(chemin: &Path, quoi: &str) -> (io::Result<Frame>, Duration) {
    let chemin = chemin.to_path_buf();
    let (envoi, reception) = mpsc::channel();
    thread::spawn(move || {
        let debut = Instant::now();
        let resultat = hidraw::ask(&chemin, &screen::query_state(), &ATTENDU);
        let _ = envoi.send((resultat, debut.elapsed()));
    });

    match reception.recv_timeout(PATIENCE) {
        Ok(rendu) => rendu,
        Err(_) => panic!(
            "au bout de {PATIENCE:?}, {quoi} n'a toujours pas rendu la main : c'est le blocage du \
             2026-08-08, `read()` sans délai de garde sur un descripteur ouvert en mode bloquant — \
             vingt lectures dont la première ne revient jamais"
        ),
    }
}

/// Le message d'une erreur, ou une phrase qui dit qu'il n'y en a pas eu.
fn resume(resultat: &io::Result<Frame>) -> String {
    match resultat {
        Ok(frame) => format!(
            "aucune erreur, trame reçue commençant par {:02x?}",
            &frame[..2]
        ),
        Err(erreur) => format!("{} ({:?})", erreur, erreur.kind()),
    }
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tous les tests qui suivent raisonnent sur `DELAI_LECTURE` plutôt que sur une durée en dur,
    // pour qu'un réglage mesuré sur le matériel n'ait pas à se négocier avec un test d'intention.
    // Une suite ainsi paramétrée a une faiblesse : elle devient vraie sans rien vérifier si la
    // constante dégénère. À une milliseconde, « le délai est tenu » ne dirait plus rien de la panne
    // observée ; à une heure, il ne dirait plus rien du tout.
    assert!(
        DELAI_LECTURE >= DELAI_MINIMAL,
        "un délai de {DELAI_LECTURE:?} est plus court que dix fois le pire acquittement relevé \
         (18 ms, spec §3.2) : il transformerait une lenteur en panne, et trois lenteurs rendraient \
         la dalle au firmware (#70)"
    );

    let tours = u32::try_from(MAX_LECTURES).expect("vingt lectures tiennent dans un u32");
    let pire_cas = DELAI_LECTURE * tours;
    let repli = Duration::from_secs(screen::FIRMWARE_FALLBACK_SECS);
    assert!(
        pire_cas <= repli,
        "au pire, une question peut durer {pire_cas:?} ({DELAI_LECTURE:?} × {MAX_LECTURES}) — \
         au-delà de {repli:?} le firmware a déjà repris la dalle (spec §2.2.2), et insister ne \
         sert plus à rien"
    );
    // Le filet doit couvrir la plus longue attente qu'un test de ce fichier admet — quatre délais
    // pour la question muette, deux et quart pour celle du bavardage lent. Il n'a pas à couvrir le
    // pire cas absolu ci-dessus : aucun test ne provoque vingt lectures qui expirent, et c'est même
    // ce que la borne de la question muette interdit.
    assert!(
        DELAI_LECTURE * 4 < PATIENCE,
        "le filet de ce fichier ({PATIENCE:?}) doit rester au-dessus de la plus longue attente \
         qu'il admet ({:?}), sinon il ferait échouer une question qui allait aboutir",
        DELAI_LECTURE * 4
    );

    // Le test du bavardage qui dure espace ses trames de trois quarts de délai. Si cet écart
    // atteignait le délai, il attraperait le mauvais défaut — et s'il tombait à zéro, il
    // n'attraperait plus rien.
    let ecart = DELAI_LECTURE * 3 / 4;
    assert!(
        ecart < DELAI_LECTURE && !ecart.is_zero(),
        "l'écart entre deux bavardages ({ecart:?}) doit rester strictement sous {DELAI_LECTURE:?} \
         et non nul : c'est lui qui distingue un délai par lecture d'un délai global"
    );
    assert!(
        ecart * (BAVARDAGES + 1) > DELAI_LECTURE,
        "les {BAVARDAGES} bavardages et la réponse doivent porter le total au-delà d'un seul \
         {DELAI_LECTURE:?}, sinon un délai global passerait ce test lui aussi et l'arbitrage ne \
         serait pas tranché"
    );

    // Et les trames témoins ne doivent surtout pas porter l'en-tête attendu, sinon le bavardage
    // serait pris pour la réponse.
    for (nom, frame) in [
        ("le rapport d'état", bavardage_etat()),
        ("l'accusé", bavardage_accuse()),
        ("la question elle-même", screen::query_state()),
    ] {
        assert_ne!(
            &frame[..ATTENDU.len()],
            &ATTENDU[..],
            "{nom} ne doit pas commencer par l'en-tête attendu, sinon il serait confondu avec la \
             réponse"
        );
    }
    assert_eq!(
        &reponse_attendue()[..ATTENDU.len()],
        &ATTENDU[..],
        "la réponse témoin doit porter l'en-tête attendu, sans quoi le garde-fou du fichier ne \
         garderait rien"
    );

    assert!(
        nomme_l_en_tete("pas de reponse 31 01 apres le delai", &ATTENDU),
        "le détecteur d'en-tête doit reconnaître la forme usuelle, sinon il déclarerait fautif un \
         message correct"
    );
    assert!(
        !nomme_l_en_tete("delai depasse", &ATTENDU),
        "et il doit refuser un message qui ne nomme rien, sinon il déclarerait correct un message \
         muet"
    );
}

// ---------------------------------------------------------------------------
// 1 — le délai est tenu
// ---------------------------------------------------------------------------

#[test]
fn une_question_a_un_peripherique_muet_echoue_au_lieu_d_attendre_pour_toujours() {
    // Le cœur du relevé du 2026-08-08 : « le démon est resté bloqué vingt minutes, zéro tic de CPU
    // sur tous ses fils, pendant que cinq clients attendaient une réponse qui n'est jamais venue ».
    //
    // Le tube est muet : la question qu'`ask` y écrit lui revient en lecture — elle ne porte pas
    // l'en-tête attendu —, et la lecture suivante n'a plus rien à lire. C'est un périphérique
    // toujours énuméré, toujours ouvrable, et définitivement silencieux.
    //
    // ⚠️ Ce test échoue en **rendant la main**, jamais en pendant : l'appel est déporté, et un
    // blocage devient l'assertion de `interroger`.
    let Some(tube) = Tube::neuf("muet") else {
        return;
    };

    let (resultat, duree) = interroger(tube.chemin(), "une question à un périphérique muet");

    let erreur = match resultat {
        Ok(frame) => panic!(
            "un périphérique muet n'a rien répondu, et pourtant une trame a été rendue : {:02x?}",
            &frame[..4]
        ),
        Err(erreur) => erreur,
    };
    assert_eq!(
        erreur.kind(),
        io::ErrorKind::TimedOut,
        "un silence doit se dire « délai dépassé » et pas autre chose : c'est ce que l'appelant \
         teste pour distinguer un périphérique muet d'un protocole mal formé ; trouvé : {erreur} \
         ({:?})",
        erreur.kind()
    );

    // Il a vraiment attendu — une implémentation qui renoncerait aussitôt casserait le matériel qui
    // répond en 18 ms un jour de charge (spec §3.2).
    assert!(
        duree >= DELAI_LECTURE / 2,
        "la question a rendu la main en {duree:?}, soit bien avant {DELAI_LECTURE:?} : elle n'a pas \
         attendu la réponse, elle a renoncé d'emblée"
    );

    // Et il n'a pas attendu vingt fois. Un délai qui compterait comme l'une des {MAX_LECTURES}
    // tentatives, au lieu d'arrêter la question, rendrait la main au bout du produit des deux —
    // soit une demi-minute de fil d'écran gelé à chaque image, ce qui n'est pas ce qu'on corrige.
    assert!(
        duree <= DELAI_LECTURE * 4,
        "la question a mis {duree:?} pour un délai de {DELAI_LECTURE:?} : une lecture qui expire \
         doit **arrêter** la question, pas consommer une des {MAX_LECTURES} tentatives et \
         recommencer"
    );
}

#[test]
fn l_erreur_du_delai_nomme_l_en_tete_qui_n_est_jamais_venu() {
    // Sans cela, le journal du démon dirait « délai dépassé » et rien d'autre — or le chemin de
    // l'image en compte trois (`30 01` pour l'état, `36 01`/`37 01` pour l'annonce, `36 02`/`37 02`
    // pour la validation, spec §3.2 et §3.7). Trois lignes identiques pour trois étapes
    // différentes, c'est une panne qu'on ne saura pas situer.
    //
    // #70 a posé la même exigence pour l'abandon — « en nommant l'erreur » — et pour la même
    // raison : une ligne de journal qui ne se relie à rien ne sert qu'à savoir qu'il s'est passé
    // quelque chose.
    let Some(tube) = Tube::neuf("muet-message") else {
        return;
    };

    let (resultat, _) = interroger(tube.chemin(), "une question à un périphérique muet");

    let message = match &resultat {
        Ok(_) => panic!(
            "un périphérique muet doit faire échouer la question ; {}",
            resume(&resultat)
        ),
        Err(erreur) => erreur.to_string(),
    };
    assert!(
        nomme_l_en_tete(&message, &ATTENDU),
        "le message doit nommer l'en-tête {ATTENDU:02x?} qu'on attendait, sinon on ne saura pas \
         quelle étape du protocole d'image a échoué ; trouvé : « {message} »"
    );
}

// ---------------------------------------------------------------------------
// 2 — le garde-fou : une question à laquelle on répond doit réussir
// ---------------------------------------------------------------------------

#[test]
fn une_question_a_laquelle_on_repond_reussit_et_rend_la_trame_lue() {
    // ⚠️ Le piège de ce fichier, et ce test n'existe que pour lui. Un correctif qui ferait échouer
    // `ask` **systématiquement** — un délai de zéro, une erreur rendue d'emblée — passerait toute
    // la série précédente sans broncher, et l'écran ne s'allumerait plus jamais. Il faut donc au
    // moins une question qui aboutisse, et dont la trame rendue soit vérifiée.
    //
    // La réponse est déposée avant la question : elle est donc lue tout de suite, ce qui rend ce
    // test indépendant de tout ordonnancement. Le cas de la réponse **tardive** est celui du test
    // suivant.
    let Some(tube) = Tube::neuf("repond") else {
        return;
    };
    tube.deposer(&reponse_attendue());

    let (resultat, duree) = interroger(tube.chemin(), "une question à un périphérique qui répond");

    assert_eq!(
        resultat.as_ref().ok(),
        Some(&reponse_attendue()),
        "la question doit rendre la trame `31 01` telle qu'elle a été lue — un délai de garde ne \
         doit rien coûter à un périphérique qui répond ; {}",
        resume(&resultat)
    );
    assert!(
        duree < DELAI_LECTURE,
        "une réponse déjà présente a été rendue en {duree:?} : elle ne doit rien attendre du tout, \
         et surtout pas {DELAI_LECTURE:?}"
    );
}

// ---------------------------------------------------------------------------
// 3 — le bavardage est écarté, et il ne déclenche pas le délai
// ---------------------------------------------------------------------------

#[test]
fn une_reponse_precedee_de_bavardage_est_trouvee_quand_meme() {
    // Le périphérique parle sans qu'on lui demande rien : rapport d'état des ventilateurs `67 02`
    // (spec NZXT §7.1) et accusés `ff 01` (§7.2). Ces trames arrivent sur le même endpoint IN que
    // la réponse, et elles n'ont pas l'en-tête attendu.
    //
    // Ce que ce test empêche de revenir : un délai de garde posé au mauvais endroit, qui
    // confondrait « je n'ai pas encore **la** réponse » avec « je n'ai rien reçu ». Un périphérique
    // qui parle est un périphérique vivant — le déclarer muet serait rigoureusement l'inverse du
    // diagnostic.
    //
    // Ici tout est déposé d'avance : aucune horloge n'intervient, et seul le tri des trames est
    // vérifié.
    let Some(tube) = Tube::neuf("bavard") else {
        return;
    };
    tube.deposer(&bavardage_etat());
    tube.deposer(&bavardage_accuse());
    tube.deposer(&bavardage_etat());
    tube.deposer(&reponse_attendue());

    let (resultat, duree) = interroger(tube.chemin(), "une question à un périphérique bavard");

    assert_eq!(
        resultat.as_ref().ok(),
        Some(&reponse_attendue()),
        "trois trames non sollicitées ne doivent pas empêcher de trouver la réponse qui les suit ; \
         {}",
        resume(&resultat)
    );
    assert!(
        duree < DELAI_LECTURE,
        "toutes les trames étaient déjà là : les écarter a pris {duree:?}, ce qui veut dire qu'on a \
         attendu quelque chose au lieu de lire"
    );
}

#[test]
fn le_bavardage_n_arrete_pas_une_question_meme_quand_il_dure() {
    // ⚠️ **C'est ce test qui tranche l'arbitrage du fichier** : délai **par lecture**, et non
    // délai **global**.
    //
    // Le périphérique émet ses trames non sollicitées espacées de trois quarts de délai, puis
    // répond. Aucune **lecture** n'attend jamais plus que ce délai, mais la **question entière**
    // dure plus du double. Une implémentation qui bornerait la question dans son ensemble
    // déclarerait donc mort un périphérique qui parlait tout du long, et rendrait la dalle au
    // firmware après trois de ces faux abandons (#70).
    //
    // Le prix de ce choix est nommé dans l'en-tête et vérifié par les repères : le pire cas devient
    // `DELAI_LECTURE × MAX_LECTURES`, borné par les trente secondes du repli firmware.
    //
    // C'est le seul test du fichier dont la durée dépend d'une horloge, et c'est irréductible :
    // la différence entre les deux corrections n'existe que dans le temps qui passe entre deux
    // trames. Les sommeils sont donc dans le **stimulus** — ce que le périphérique fait —, jamais
    // dans la condition de succès, qui reste « la réponse a été trouvée ».
    let Some(tube) = Tube::neuf("bavard-lent") else {
        return;
    };
    let ecart = DELAI_LECTURE * 3 / 4;

    let (resultat, duree) = thread::scope(|portee| {
        portee.spawn(|| {
            for _ in 0..BAVARDAGES {
                thread::sleep(ecart);
                tube.deposer(&bavardage_etat());
            }
            thread::sleep(ecart);
            tube.deposer(&reponse_attendue());
        });
        interroger(
            tube.chemin(),
            "une question à un périphérique bavard et lent",
        )
    });

    assert_eq!(
        resultat.as_ref().ok(),
        Some(&reponse_attendue()),
        "aucune lecture n'a attendu plus de {ecart:?}, soit moins que {DELAI_LECTURE:?} : la \
         question devait aboutir. Un délai **global** l'aurait tuée en chemin, sur un périphérique \
         qui parlait à chaque tour ; {}",
        resume(&resultat)
    );
    assert!(
        duree > DELAI_LECTURE,
        "la question a duré {duree:?}, soit moins qu'un seul {DELAI_LECTURE:?} : le bavardage n'a \
         donc pas été espacé comme prévu, et ce test n'a pas tranché ce qu'il devait trancher"
    );
}
