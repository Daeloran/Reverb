//! Le verdict qui précède une consigne de vitesse (issue #112).
//!
//! ⚠️ **Un calcul, et rien d'autre.** La décision vivait dans `main.rs`, au
//! milieu de l'écriture qu'elle doit empêcher. C'est la règle du projet — « ce
//! qui est testable sans matériel est séparé de ce qui touche au matériel »
//! (CLAUDE.md) — appliquée au garde-fou : le **verdict** est du calcul,
//! l'écriture est de l'E/S.
//!
//! C'est aussi ce qui porte le critère « rien n'est écrit — ni le mode, ni la
//! consigne » : une fonction qui ne reçoit ni descripteur, ni `&FanChannel`, ni
//! chemin ne *peut pas* écrire. Le critère devient une propriété de la
//! signature, plutôt qu'une promesse à relire.

use reverb_hw::hwmon::Mode;

/// Faut-il refuser d'écrire une consigne sur ce canal ?
///
/// Rend le message à afficher quand le geste détruirait une régulation que
/// l'hôte ne sait pas rétablir, et `None` quand il n'y a rien à perdre.
/// `manual` est le drapeau `--manual`, par lequel l'utilisateur dit qu'il sait
/// ce qu'il fait.
///
/// ⚠️ **Deux modes refusent, et ce sont ceux où quelque chose d'autre que l'hôte
/// décide de la vitesse** : `non-piloté` et `courbe-de-l'hôte`. C'est exactement
/// le déclencheur du verrou de la fenêtre (`reverb_gui::telemetrie`) — un seul
/// fait matériel, deux portes, et le même verdict qu'on clique ou qu'on tape.
///
/// ⚠️ **Le nom du canal n'entre jamais dans la décision.** Refuser sur « kraken »
/// donnerait aujourd'hui le bon résultat sur SHYNAEL, seuls ses canaux lisant
/// `non-piloté` ; ça casse au premier pilote qui change, et ça casse en silence.
/// Le canal est là pour être **nommé** dans le message, ce qui est le seul moyen
/// de rendre le refus utile sous un `--all`, où l'utilisateur n'a désigné aucun
/// canal.
///
/// ⚠️ **Les quatre autres modes passent, `--manual` ou non.** Imposer le drapeau
/// ailleurs coûterait un geste à sept ventilateurs sur dix — les trois canaux
/// `nzxtsmart2` lisent `manuel` — sans rien protéger, et un garde-fou qu'on tape
/// par réflexe ne garde plus rien.
pub fn refus_de_consigne(canal: &str, mode: Mode, manual: bool) -> Option<String> {
    if manual {
        return None;
    }
    match mode {
        // Le mode des deux canaux du Kraken sur SHYNAEL. Un `0` **lu** dit que
        // le pilote n'a jamais touché ce canal (#101) : ce qu'il fait, c'est le
        // périphérique qui le décide, et la mesure du 2026-08-15 a montré cette
        // régulation d'usine bien vivante — 35 % à 37 °C, 60 % à 51 °C.
        Mode::NonPilote => Some(format!(
            "« {canal} » n'est piloté par personne côté hôte : c'est le périphérique \
             qui régule, sur son propre profil. Lui imposer une consigne fixe l'en \
             sortirait, et aucune commande ne l'y rend — seule une coupure \
             d'alimentation complète. Ajoutez « --manual » si c'est voulu."
        )),
        // Le garde de #97, élargi et non déplacé. Sa formulation est celle
        // d'avant, moins le pourcentage : cette fonction ne le reçoit pas, et
        // le lui passer la ferait dépendre de ce qu'elle refuse d'écrire.
        Mode::HostCurve => Some(format!(
            "« {canal} » exécute une courbe et s'adapte à la température. \
             Lui imposer une consigne fixe l'en sortirait définitivement : ajoutez \
             « --manual » si c'est voulu, et « reverb fan --channel {canal} --auto » \
             pour l'y rendre."
        )),
        // - `manuel` : c'est déjà l'hôte qui décide.
        // - `plein-régime-100%` : le pilote a lâché la barre et le canal tourne
        //   à fond (#101). C'est l'état dont on veut le plus pouvoir sortir.
        // - `inconnu-N` : l'hôte ne sait pas ce que fait ce canal. Refuser
        //   là-dessus, ce serait affirmer qu'il régule — et le projet refuse
        //   d'implémenter depuis un inconnu (CLAUDE.md).
        // - `non-réglable` : la source n'expose aucun `pwmN_enable`, aucun mode
        //   ne s'y écrit ni ne s'y perd.
        Mode::Manual | Mode::PleinRegime | Mode::Unknown(_) | Mode::Unsupported => None,
    }
}
