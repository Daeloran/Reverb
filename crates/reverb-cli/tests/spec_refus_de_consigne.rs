//! Tests d'intention du garde-fou de `reverb fan --pwm` en ligne de commande (issue #112).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #112 seule. Rien de `crates/reverb-cli/src/`
//! n'a été lu pour les produire. Seule a été relevée l'énumération publique
//! `reverb_hw::hwmon::Mode`, telle que `spec_fans.rs` (#7), `spec_auto.rs` (#50),
//! `spec_mode_non_pilote.rs` (#101) et `spec_courbe_avant_auto.rs` (#97) l'emploient déjà.
//!
//! À l'écriture de ce fichier, `refus_de_consigne` **n'existe pas** : la compilation doit échouer
//! sur elle, et sur elle seule. C'est la phase rouge.
//!
//! Rien ici n'ouvre de périphérique, ne lit ni n'écrit un `sysfs`, fût-il factice.
//!
//! ## Le défaut, tel que l'issue le décrit
//!
//! ```text
//! reverb fan --channel kraken2023elite:pump-speed --pwm 50
//! ```
//!
//! détruit aujourd'hui la régulation d'usine **en silence** : la commande bascule le canal en
//! manuel avant d'écrire la consigne, et le canal n'y reviendra pas avant un arrêt complet de la
//! machine (`docs/VENTILATEURS.md`, « `pwm_enable = 0` ne rend pas le Kraken à son profil
//! d'usine »).
//!
//! Le garde existant ne couvre que `courbe-de-l'hôte` — c'est celui que #97 a posé. Il doit couvrir
//! `NonPilote` aussi : « C'est le même défaut par l'autre porte, et c'est par la ligne de commande
//! que passent les gestes qu'on fait sans regarder — typiquement un `--all` qui emporte le Kraken
//! avec les autres. »
//!
//! ⚠️ L'issue relève au passage que le commentaire justifiant l'exemption de `NonPilote` repose sur
//! l'avertissement de #101 : « un canal en `0` n'a rien à perdre » ne valait que si `0` signifiait
//! 100 %. Or `0` **lu** signifie exactement le contraire — le pilote n'a jamais touché ce canal, et
//! le périphérique exécute son propre profil.
//!
//! ## Le contrat que ces tests figent
//!
//! ```ignore
//! // exposé à la racine de `reverb_cli` ; défini où l'implémentation le juge bon.
//!
//! /// Faut-il refuser d'écrire une consigne sur ce canal ?
//! ///
//! /// Rend le message à afficher quand le geste détruirait une régulation que l'hôte ne sait pas
//! /// rétablir, et `None` quand il n'y a rien à perdre. `manual` est le drapeau `--manual`, par
//! /// lequel l'utilisateur dit qu'il sait ce qu'il fait.
//! pub fn refus_de_consigne(canal: &str, mode: Mode, manual: bool) -> Option<String>;
//! ```
//!
//! ### Pourquoi une fonction pure, et pourquoi elle prend le nom du canal
//!
//! La décision est aujourd'hui enfouie dans `main.rs`, au milieu de l'écriture qu'elle doit
//! empêcher. C'est la règle du projet — « ce qui est testable sans matériel est séparé de ce qui
//! touche au matériel » (CLAUDE.md) — appliquée au garde-fou : le **verdict** est du calcul,
//! l'écriture est de l'E/S.
//!
//! ⚠️ **C'est aussi ce qui porte le critère « rien n'est écrit — ni le mode, ni la consigne ».** Une
//! fonction qui ne reçoit ni descripteur, ni `&FanChannel`, ni chemin ne *peut pas* écrire : le
//! critère devient une propriété de la signature plutôt qu'une promesse à relire. Ce que ces tests
//! ne couvrent pas, c'est le **branchement** — que `main.rs` la consulte avant `set_mode` et non
//! après. Cela se lit en relecture, ou se constate sur un Kraken ; c'est la limite assumée de ce
//! fichier, et elle est notée en bas.
//!
//! Le canal entre en argument parce que le message doit le nommer : c'est un critère d'acceptation,
//! et c'est la seule chose qui rende le refus utile sous un `--all`, où l'utilisateur n'a pas
//! désigné le canal lui-même.
//!
//! ## Ce que ces tests ne figent pas, et pourquoi
//!
//! - **La formulation exacte du message.** Ils exigent qu'il nomme le canal, qu'il cite `--manual`
//!   et qu'il nomme ce qui disparaîtrait. Le reste est une affaire de style.
//! - **Le code de sortie et le flux de sortie** — le projet écrit ses refus en `err …`, et aucun
//!   critère de l'issue n'en parle.
//! - **Ce que `--manual` fait ensuite.** Il lève le refus, il ne change pas l'écriture.
//! - **Le refus de `--auto` sans courbe posée**, qui est celui de #97 et vit dans `reverb-hw`
//!   (`spec_courbe_avant_auto.rs`). Ce fichier ne parle que de `--pwm`.
//! - **Le verrou de la fenêtre**, qui a son fichier : `reverb-gui/tests/spec_verrou_canal.rs`.

use reverb_cli::refus_de_consigne;
use reverb_hw::hwmon::Mode;

// ---------------------------------------------------------------------------
// Les modes, et de quel côté ils tombent
// ---------------------------------------------------------------------------
//
// Les fabriques plutôt que les valeurs : `Mode` n'a pas à être `Copy` pour que ces tests
// compilent, et chaque mode est essayé sous les deux valeurs du drapeau `--manual`.

/// Les deux modes où quelque chose d'autre que l'hôte décide de la vitesse.
///
/// C'est le déclencheur que l'issue nomme, et il est le même que celui du verrou de la fenêtre :
/// « le même garde-fou en ligne de commande ». Un seul fait matériel, deux portes.
const MODES_QUI_REFUSENT: [(&str, fn() -> Mode); 2] = [
    ("non-piloté", || Mode::NonPilote),
    ("courbe-de-l'hôte", || Mode::HostCurve),
];

/// Les quatre autres, où il n'y a aucune régulation à perdre.
///
/// - `manuel` : c'est déjà l'hôte qui décide.
/// - `plein-régime-100%` : le pilote a lâché la barre et le canal tourne à fond (#101). C'est
///   l'état dont on veut le plus pouvoir sortir, pas celui qu'on protège.
/// - `inconnu-N` : l'hôte ne sait pas ce que fait ce canal. Refuser là-dessus, ce serait affirmer
///   qu'il régule — le projet refuse d'implémenter depuis un inconnu (CLAUDE.md).
/// - `non-réglable` : la source n'expose aucun `pwmN_enable`, aucun mode ne s'y écrit ni ne s'y
///   perd.
///
/// La liste est exactement le complément de celle du verrou de la fenêtre, et ce n'est pas une
/// coïncidence : « le même fait doit produire le même verdict qu'on clique ou qu'on tape ».
const MODES_QUI_ACCEPTENT: [(&str, fn() -> Mode); 4] = [
    ("manuel", || Mode::Manual),
    ("plein-régime-100%", || Mode::PleinRegime),
    ("inconnu-7", || Mode::Unknown(7)),
    ("non-réglable", || Mode::Unsupported),
];

/// Le canal qui a motivé l'issue — celui dont la régulation d'usine a été mesurée vivante le
/// 2026-08-15, 35 % à 37 °C et 60 % à 51 °C.
const POMPE: &str = "kraken2023elite:pump-speed";

/// Un canal d'un tout autre pilote, pour que le verdict ne se décide jamais sur un nom.
const SMART: &str = "nzxtsmart2:fan-1";

/// Les canaux du banc.
const BANC: [&str; 3] = [POMPE, "kraken2023elite:fan-speed", SMART];

/// L'option par laquelle l'utilisateur dit qu'il sait ce qu'il fait.
///
/// Elle doit être **citée telle qu'on la tape** : un refus qui dirait « utilisez l'option manuelle »
/// laisserait chercher.
const OPTION_DE_PASSAGE: &str = "--manual";

/// Ce que le refus doit nommer : la chose qui disparaîtrait.
///
/// La liste est volontairement courte. Le critère « le refus explique ce qui serait perdu » n'est
/// pas testable au sens fort, mais il est vérifiable au sens faible : un message qui ne prononce ni
/// « régulation » ni « courbe » ne dit pas ce qui s'arrête, il dit seulement qu'on refuse — et
/// c'est exactement l'information qui manquait le 2026-08-15.
const MOTS_DE_LA_PERTE: [&str; 4] = ["régulation", "régule", "courbe", "profil"];

/// La longueur au-dessous de laquelle un refus ne peut pas tenir sa promesse.
///
/// Il doit dire **ce qui serait perdu** et **comment passer outre** : deux informations, plus le
/// nom du canal. Le seuil est bas — bien en dessous de n'importe quelle phrase honnête, bien
/// au-dessus d'un « refusé » suivi d'un nom.
const LONGUEUR_MINIMALE_DU_REFUS: usize = 60;

// ---------------------------------------------------------------------------
// Lire un verdict
// ---------------------------------------------------------------------------

/// Le message de refus, ou l'échec qui dit que le geste est passé.
fn refus(canal: &str, mode: fn() -> Mode, nom_du_mode: &str) -> String {
    refus_de_consigne(canal, mode(), false).unwrap_or_else(|| {
        panic!(
            "« {canal} » est en « {nom_du_mode} » et `--pwm` passe sans `{OPTION_DE_PASSAGE}` : \
             c'est le défaut de #112, et il est silencieux — la régulation disparaît sans message \
             et ne revient pas avant un arrêt complet de la machine"
        )
    })
}

// ---------------------------------------------------------------------------
// 1 — un canal qui régule seul refuse la consigne
// ---------------------------------------------------------------------------

#[test]
fn un_canal_non_pilote_refuse_la_consigne_sans_manual() {
    // Critère d'acceptation n° 5 : « `reverb fan --pwm` refuse sur un canal en `non-piloté` sans
    // `--manual` ». C'est le cœur de l'issue côté ligne de commande.
    //
    // Le mode est celui des deux canaux du Kraken sur SHYNAEL, et #101 dit ce qu'il établit : le
    // pilote n'a jamais touché ce canal, donc le périphérique exécute son propre profil. La mesure
    // du même jour a montré ce profil bien vivant. Le laisser passer, c'est le remplacer par une
    // valeur figée sans que rien ne le dise.
    for canal in BANC {
        assert!(
            refus_de_consigne(canal, Mode::NonPilote, false).is_some(),
            "« {canal} » lit « non-piloté » : une consigne écrite là remplace une régulation \
             qu'aucune commande ne rétablit"
        );
    }
}

#[test]
fn un_canal_sur_la_courbe_de_l_hote_refuse_toujours_sans_manual() {
    // Critère d'acceptation n° 7 : « Le refus existant sur `courbe-de-l'hôte` est inchangé ».
    //
    // C'est une non-régression : le garde de #97 est déjà là, et #112 l'élargit sans le déplacer.
    // Une correction qui remplacerait la condition au lieu de l'étendre — `NonPilote` au lieu de
    // `HostCurve` plutôt qu'en plus — passerait tous les autres tests de ce fichier.
    for canal in BANC {
        assert!(
            refus_de_consigne(canal, Mode::HostCurve, false).is_some(),
            "« {canal} » exécute une courbe posée par l'hôte : écrire une consigne la remplace, et \
             c'est la seule courbe que l'utilisateur ait posée lui-même"
        );
    }
}

#[test]
fn le_verdict_ne_depend_pas_du_nom_du_canal() {
    // « Le déclencheur est le mode, pas le nom du contrôleur » (issue #112). La faute visée
    // passerait aujourd'hui inaperçue : refuser sur « kraken » donne le bon résultat sur SHYNAEL,
    // parce que seuls ses canaux lisent `non-piloté`. Elle casse au premier pilote qui change, et
    // elle casse en silence.
    for (nom_du_mode, mode) in MODES_QUI_REFUSENT {
        let sur_la_pompe = refus_de_consigne(POMPE, mode(), false).is_some();
        let sur_le_smart = refus_de_consigne(SMART, mode(), false).is_some();
        assert_eq!(
            sur_la_pompe, sur_le_smart,
            "« {nom_du_mode} » : « {POMPE} » et « {SMART} » ne reçoivent pas le même verdict — le \
             nom du canal est entré dans la décision"
        );
    }
    for (nom_du_mode, mode) in MODES_QUI_ACCEPTENT {
        let sur_la_pompe = refus_de_consigne(POMPE, mode(), false).is_some();
        let sur_le_smart = refus_de_consigne(SMART, mode(), false).is_some();
        assert_eq!(
            sur_la_pompe, sur_le_smart,
            "« {nom_du_mode} » : « {POMPE} » et « {SMART} » ne reçoivent pas le même verdict"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — `--manual` lève le refus, et lui seul
// ---------------------------------------------------------------------------

#[test]
fn manual_leve_le_refus_sur_les_deux_modes_qui_regulent() {
    // Critère d'acceptation n° 6, second versant : « Le refus explique […] comment passer outre ».
    // L'option doit donc exister et marcher, sinon le message enverrait dans un mur.
    //
    // Les deux modes sont essayés : #112 ajoute `non-piloté` au garde, elle ne retire pas
    // l'échappatoire de `courbe-de-l'hôte` que #97 avait laissée.
    for (nom_du_mode, mode) in MODES_QUI_REFUSENT {
        for canal in BANC {
            assert_eq!(
                refus_de_consigne(canal, mode(), true),
                None,
                "« {canal} » en « {nom_du_mode} » avec `{OPTION_DE_PASSAGE}` : l'utilisateur a dit \
                 qu'il savait ce qu'il faisait, le refus n'a plus de raison d'être"
            );
        }
    }
}

#[test]
fn les_quatre_autres_modes_passent_avec_ou_sans_manual() {
    // Le pendant du choix figé côté fenêtre : seuls `non-piloté` et `courbe-de-l'hôte` protègent
    // quelque chose. Imposer `--manual` ailleurs coûterait un drapeau à sept ventilateurs sur dix —
    // les trois canaux `nzxtsmart2` lisent `manuel` — sans rien protéger, et un garde-fou qu'on
    // tape par réflexe ne garde plus rien.
    for (nom_du_mode, mode) in MODES_QUI_ACCEPTENT {
        for canal in BANC {
            for manual in [false, true] {
                assert_eq!(
                    refus_de_consigne(canal, mode(), manual),
                    None,
                    "« {canal} » en « {nom_du_mode} » (--manual={manual}) : il n'y a aucune \
                     régulation à perdre, la consigne doit passer"
                );
            }
        }
    }
}

#[test]
fn toute_la_famille_des_valeurs_inconnues_passe() {
    // `Mode::Unknown` porte la valeur brute de `pwmN_enable` que l'hôte n'a pas su interpréter.
    // Figer le verdict sur la seule valeur 7 laisserait passer une implémentation qui traiterait à
    // part, disons, `Unknown(0)` — la valeur qu'un `kzalloc` produit, donc la plus tentante.
    for n in [0u8, 1, 3, 7, 42, 255] {
        for manual in [false, true] {
            assert_eq!(
                refus_de_consigne(POMPE, Mode::Unknown(n), manual),
                None,
                "`inconnu-{n}` (--manual={manual}) : une valeur que l'hôte ne sait pas interpréter \
                 n'établit aucune régulation, et refuser reviendrait à affirmer qu'elle en \
                 établit une"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — ce que le refus doit dire
// ---------------------------------------------------------------------------

#[test]
fn le_refus_nomme_le_canal() {
    // Critère d'acceptation n° 6, et test d'intention : « Le message de refus nomme le canal et
    // cite `--manual` ».
    //
    // Ça compte surtout sous `--all` — le geste que l'issue nomme, « typiquement un `--all` qui
    // emporte le Kraken avec les autres ». L'utilisateur n'a alors désigné aucun canal : un refus
    // qui ne dit pas lequel a coincé laisse chercher parmi cinq.
    for (nom_du_mode, mode) in MODES_QUI_REFUSENT {
        for canal in BANC {
            let message = refus(canal, mode, nom_du_mode);
            assert!(
                message.contains(canal),
                "le refus de « {canal} » en « {nom_du_mode} » ne le nomme pas : {message:?}"
            );
        }
    }
}

#[test]
fn le_refus_cite_l_option_qui_permet_de_passer_outre() {
    // Critère d'acceptation n° 6 : « Le refus explique […] comment passer outre ». L'option est
    // citée **telle qu'on la tape** : c'est la différence entre un refus qui débloque et un refus
    // qui renvoie à la page d'aide.
    for (nom_du_mode, mode) in MODES_QUI_REFUSENT {
        for canal in BANC {
            let message = refus(canal, mode, nom_du_mode);
            assert!(
                message.contains(OPTION_DE_PASSAGE),
                "le refus de « {canal} » en « {nom_du_mode} » ne cite pas `{OPTION_DE_PASSAGE}` : \
                 {message:?}"
            );
        }
    }
}

#[test]
fn le_refus_dit_ce_qui_serait_perdu() {
    // Critère d'acceptation n° 6, premier versant : « Le refus explique ce qui serait perdu ».
    //
    // C'est l'information qui manquait le 2026-08-15 : rien, nulle part, ne disait qu'un canal en
    // `non-piloté` exécutait une régulation d'usine, ni qu'un geste la remplacerait sans retour.
    // Un message qui se contenterait de refuser laisserait croire à une restriction arbitraire — et
    // on tape alors `--manual` pour en sortir, ce qui est exactement le geste à ne pas faire sans
    // savoir.
    for (nom_du_mode, mode) in MODES_QUI_REFUSENT {
        let message = refus(POMPE, mode, nom_du_mode);
        assert!(
            message.chars().count() >= LONGUEUR_MINIMALE_DU_REFUS,
            "le refus de « {nom_du_mode} » fait {} caractères : nommer le canal, dire ce qui \
             disparaît et dire comment passer outre ne tient pas en si peu. Message : {message:?}",
            message.chars().count()
        );
        let minuscules = message.to_lowercase();
        assert!(
            MOTS_DE_LA_PERTE
                .iter()
                .any(|mot| minuscules.contains(&mot.to_lowercase())),
            "le refus de « {nom_du_mode} » ne prononce aucun de {MOTS_DE_LA_PERTE:?} : il dit \
             qu'on refuse, pas ce qui s'arrêterait. Message : {message:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — la décision est un calcul, et rien d'autre
// ---------------------------------------------------------------------------

#[test]
fn la_decision_ne_depend_que_du_mode_et_du_drapeau() {
    // Critère d'acceptation : « rien n'est écrit — ni le mode, ni la consigne ».
    //
    // Une fonction qui ne reçoit ni descripteur, ni `&FanChannel`, ni chemin ne peut pas écrire :
    // la signature porte le critère mieux qu'une assertion ne le ferait. Ce que ce test ajoute,
    // c'est qu'elle ne garde pas non plus d'état entre deux appels — un verdict qui changerait au
    // second passage serait le signe qu'elle a consulté ou modifié quelque chose au-dehors, et le
    // `--all` de l'issue l'appelle cinq fois de suite.
    for (nom_du_mode, mode) in MODES_QUI_REFUSENT.iter().chain(&MODES_QUI_ACCEPTENT) {
        for manual in [false, true] {
            let premier = refus_de_consigne(POMPE, mode(), manual);
            let second = refus_de_consigne(POMPE, mode(), manual);
            assert_eq!(
                premier, second,
                "« {nom_du_mode} » (--manual={manual}) rend deux verdicts différents à deux appels \
                 identiques : la décision n'est pas un calcul"
            );
        }
    }
}

#[test]
fn les_six_modes_sont_couverts_par_l_un_des_deux_versants() {
    // Repère de ce fichier : les deux listes doivent partitionner les six modes que
    // `reverb_hw::hwmon::Mode` sait rendre. Un mode oublié ne serait testé d'aucun côté, et c'est
    // précisément ce qui est arrivé à `NonPilote` — il n'était nommé nulle part dans le garde-fou,
    // et son exemption reposait sur un commentaire devenu faux.
    assert_eq!(
        MODES_QUI_REFUSENT.len() + MODES_QUI_ACCEPTENT.len(),
        6,
        "les deux listes doivent couvrir les six modes de la colonne MODE"
    );

    // Et les six libellés nommés ici doivent être six, distincts : deux entrées sous un même nom
    // laisseraient un mode dans l'ombre sans que le compte le montre.
    let noms: Vec<&str> = MODES_QUI_REFUSENT
        .iter()
        .chain(&MODES_QUI_ACCEPTENT)
        .map(|(nom, _)| *nom)
        .collect();
    for (i, nom) in noms.iter().enumerate() {
        for autre in noms.iter().skip(i + 1) {
            assert_ne!(nom, autre, "deux entrées portent le nom « {nom} »");
        }
    }
}
