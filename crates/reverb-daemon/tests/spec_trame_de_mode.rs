//! Tests d'intention — la trame de mode ne part qu'à l'ouverture. Issue #137.
//!
//! Écrits depuis l'issue #137 et `docs/SPEC-KRAKEN-LCD.md` seuls. Aucun fichier de
//! `crates/*/src/` n'a été lu pour les produire, hors les **signatures publiques** de
//! [`reverb_proto::screen`] et du trait `reverb_daemon::fil_ecran::Afficheur` — consultées pour
//! savoir ce que ce fichier pouvait observer, et, ci-dessous, pour constater qu'il ne peut pas
//! observer l'essentiel.
//!
//! ## Le défaut
//!
//! Observé devant le boîtier le 2026-08-16 : **la dalle passe au noir puis revient sur son image,
//! toutes les ~25 secondes**. Ce n'est pas le repli du firmware — celui-là arrive au bout de 30 s
//! de silence (§2.2.2), et les 25 s de réémission existent précisément pour qu'il n'arrive jamais.
//! C'est `38 01 02 00`, réémise avant **chaque** image :
//!
//! | | |
//! |---|---|
//! | §3.6, sur les cinquante images de la capture de CAM | « **Aucune trame `32` ni `38` entre deux images.** » |
//! | §3.4 | une commande d'affichage « réinitialise le pipeline d'affichage » |
//! | §2.2.3 | le clignotement, déjà observé, avec cette trame nommée comme l'une des deux causes |
//!
//! L'autre candidate du §2.2.3 — l'interface bulk réclamée puis rendue — ne s'applique pas au
//! démon, qui garde sa poignée usbfs ouverte pour toute sa durée de vie.
//!
//! Une image fixe réémise toutes les 25 s, ce sont ~3 456 réinitialisations de pipeline par jour,
//! là où l'implémentation de référence n'en inflige qu'une par session.
//!
//! ## ⚠️ Le drapeau doit sortir de `Kraken`, sans quoi rien de tout cela ne se teste
//!
//! Première rédaction de ce fichier : deux des trois tests d'intention demandés par l'issue étaient
//! **inécrivables**, et le constat tenait en trois faits relevés sur les seules signatures
//! publiques.
//!
//! 1. le trait `Afficheur` (`crates/reverb-daemon/src/fil_ecran.rs`) n'a que deux méthodes,
//!    `luminosite(&mut self, pourcent: u8)` et `image(&mut self, dalle: &Dalle)`. **Aucune trame
//!    ne le traverse** : un double d'essai note une luminosité et une dalle, jamais un octet de
//!    HID. C'est la couture de #83, et elle a été taillée pour observer des **gestes**, pas un
//!    protocole ;
//! 2. la trame part sous ce trait, dans l'implémentation `Kraken` de
//!    `crates/reverb-daemon/src/peripheriques.rs`. `struct Kraken` et `Kraken::ouvrir` sont
//!    **privés** : un test d'intégration ne peut ni en construire un, ni l'atteindre ;
//! 3. l'écriture elle-même passe par `reverb_hw::hidraw::write_frame(&Path, &Frame)`, une fonction
//!    libre qui ouvre le nœud et écrit. Elle prend bien un chemin — donc un fichier ordinaire
//!    conviendrait —, mais l'image attend ensuite les accusés `37 01` et `37 02` par
//!    `hidraw::ask`, et un fichier ordinaire ne répond pas. **La séquence entière ne se rejoue pas
//!    contre un double de fichier.**
//!
//! **La décision est prise sur l'issue** : la question « faut-il armer le mode ? » sort de `Kraken`
//! sous la forme d'un état pur, sur le motif de `refus_de_consigne` (#101) — où « rien n'est
//! écrit » est devenu une **propriété de signature** plutôt qu'une promesse de commentaire.
//!
//! ```ignore
//! // crates/reverb-daemon/src/fil_ecran.rs
//!
//! /// Le mode de diffusion a-t-il déjà été armé sur ce périphérique ?
//! ///
//! /// ⚠️ **Ni descripteur, ni chemin, ni trame.**
//! #[derive(Debug, Default)]
//! pub struct ModeDeDiffusion { arme: bool }
//!
//! impl ModeDeDiffusion {
//!     /// Rend `true` la première fois, `false` ensuite.
//!     pub fn faut_il_armer(&mut self) -> bool;
//! }
//! ```
//!
//! ⚠️ **Dans `fil_ecran`, pas dans `peripheriques`**, et c'est un choix. `peripheriques` est le
//! module qui ouvre les nœuds ; y ranger un type pur, c'est le mettre dans le seul endroit du crate
//! que rien ne teste — l'inverse exact de #101, où `refus_de_consigne` a justement quitté le module
//! qui appelait `set_pwm` pour son propre fichier. `fil_ecran` est déjà l'endroit où l'on dit **ce
//! que la dalle exige du matériel sans toucher au matériel** : c'est la raison d'être d'`Afficheur`,
//! et `Kraken`, qui implémente ce trait, importe ce module de toute façon.
//!
//! ⚠️ **Aucun lecteur de l'état, et c'est délibéré.** « Un mode neuf n'est pas armé » s'observe
//! entièrement par sa première réponse ; ajouter un `arme()` donnerait deux façons de poser la même
//! question, dont une seule serait consultée par `image()`. Le drapeau n'a qu'un lecteur, et c'est
//! `faut_il_armer`.
//!
//! ⚠️ **`Default` est le seul constructeur, et c'est ce qui fait tenir le critère 3.** Une
//! réouverture réarme *sans code supplémentaire* : `Kraken::ouvrir` construit un
//! `ModeDeDiffusion::default()`, donc la poignée lâchée puis rouverte après un reset USB (#98)
//! repart armée. Rien à penser au moment du reset, rien à oublier — et
//! `un_peripherique_rouvert_rearme_le_mode` interdit qu'un jour un `static` ou un `AtomicBool`
//! vienne partager cet état entre deux instances.
//!
//! ## Ce qui est déjà figé ailleurs, et qu'il ne faut pas redoubler ici
//!
//! - le **contenu** de `38 01 02 00`, octet par octet, et celui de `36 01 00 01 09` et `36 02` :
//!   `crates/reverb-proto/tests/spec_screen.rs` (#13) ;
//! - la **cadence** de réémission, 25 s, strictement sous les 30 s du repli : même fichier. C'est
//!   le hors-scope de #137, et il est déjà gardé ;
//! - l'**ordre luminosité → image** tel qu'il traverse le fil de la dalle :
//!   `spec_fil_ecran.rs::une_luminosite_precede_toujours_l_image_qui_la_suit` (#83). ⚠️ Ce n'est
//!   pas tout à fait le critère 4 de #137, qui porte sur l'ordre des **trames** `30 02` et `36 01`
//!   à l'intérieur de `Kraken` — invisible du fil, et donc non couvrable ici non plus.
//!
//! ## Les deux tests verts, et pourquoi ils restent
//!
//! Le §3.6 est un résultat **négatif** : la boucle d'image ne porte ni `32` ni `38`. Un correctif
//! qui se contenterait de replier la trame de mode dans l'annonce d'image — « puisqu'il en faut
//! une, mettons-la dans `begin_image()` » — laisserait tous les tests existants verts et
//! reproduirait le clignotement à l'identique. Ni le drapeau, ni aucun de ses tests ne l'attrape :
//! `faut_il_armer` peut très bien rendre `false` pendant qu'une trame `38` voyage à l'intérieur de
//! `begin_image()`. C'est ce que fige
//! `la_boucle_d_une_image_ne_porte_ni_trame_de_mode_ni_trame_de_bucket`.
//!
//! S'y ajoute le verdict des accusés du §3.2, nommé par le critère 2 (« attente de `37 01` ») et
//! qu'aucun test ne portait jusqu'ici.
//!
//! ## Où vit ce fichier, et pourquoi
//!
//! Dans `reverb-daemon` : le défaut, les critères et le drapeau sont ceux du démon. Déposé dans
//! `reverb-proto`, l'en-tête ci-dessus parlerait de `Kraken` et d'`Afficheur` depuis un crate qui
//! ignore leur existence.

use reverb_daemon::fil_ecran::ModeDeDiffusion;
use reverb_proto::{FRAME_LEN, Frame, screen};

// ---------------------------------------------------------------------------
// Repères, tous relevés dans la spec — aucun déduit par ce fichier
// ---------------------------------------------------------------------------

/// Premier octet de la trame de mode d'affichage, `38 01 <mode> <bucket>` (spec §3.5, ✅).
const COMMANDE_MODE: u8 = 0x38;

/// Premier octet des trames de bucket, `32 02 <n>` pour `n` de `0x00` à `0x0f` (spec §3.1, ✅).
///
/// Le §3.6 les nomme dans la même phrase que la précédente : ni les unes ni l'autre ne
/// reparaissent entre deux images.
const COMMANDE_BUCKET: u8 = 0x32;

/// Premier octet des deux trames qui encadrent un transfert d'image, `36 01` et `36 02`
/// (spec §2.2.1 et §3.2, ✅).
const COMMANDE_IMAGE: u8 = 0x36;

/// Premier octet des accusés du contrôleur, `37 01` et `37 02` (spec §3.2, ✅).
const ACCUSE: u8 = 0x37;

/// Verdict de succès porté par un accusé, à l'offset 14 (spec §3.2, ✅ — `liquidctl` teste
/// `response[14] == 0x1`, et tous les accusés de la capture le portent).
const VERDICT_SUCCES: u8 = 0x01;

/// Les trames HID que la boucle d'une image émet, dans l'ordre du §3.2 :
///
/// ```text
/// 36 01 00 01 09      -->   annonce l envoi d une image
/// 37 01 ... 01 ...    <--   ACCUSE
///    (les deux transferts bulk du §2 passent ici)
/// 36 02               -->   validation
/// 37 02 ... 01 ...    <--   ACCUSE
/// ```
///
/// C'est **toute** la boucle : le §3.6 est formel, il n'y a rien d'autre entre deux images.
fn boucle_d_image() -> [(&'static str, Frame); 2] {
    [
        ("l'annonce d'image", screen::begin_image()),
        ("la validation d'image", screen::end_image()),
    ]
}

/// Fabrique un accusé plausible : la commande, sa sous-commande, et un verdict à l'offset 14.
fn accuse(sous_commande: u8, verdict: u8) -> Frame {
    let mut trame: Frame = [0; FRAME_LEN];
    trame[0] = ACCUSE;
    trame[1] = sous_commande;
    trame[screen::ACK_VERDICT_OFFSET] = verdict;
    trame
}

// ---------------------------------------------------------------------------
// §3.6 — la boucle d'image ne porte ni mode, ni bucket
// ---------------------------------------------------------------------------

#[test]
fn la_boucle_d_une_image_ne_porte_ni_trame_de_mode_ni_trame_de_bucket() {
    // Spec §3.6, ✅, résultat négatif établi par `tools/extrait_kraken.py` sur la capture d'init :
    // « Aucune trame `32` ni `38` entre deux images. Les seize `32 02 <n>` de l'init ne se
    // reproduisent jamais. »
    //
    // C'est le fait de protocole dont #137 découle. Ce test le porte au seul endroit où un crate
    // pur puisse le porter : dans le **contenu** des trames de la boucle. Il interdit le correctif
    // le plus tentant et le plus faux — replier `38 01 02 00` dans l'annonce d'image pour n'avoir
    // qu'un seul `write()` —, qui laisserait le clignotement intact en passant tous les tests
    // existants.
    let mode = screen::broadcast_mode();

    assert_eq!(
        mode[0], COMMANDE_MODE,
        "la trame de mode d'affichage doit commencer par {COMMANDE_MODE:#04x} (spec §3.5, \
         `38 01 <mode> <bucket>`) ; elle commence par {:#04x}. Sans ce repère, plus rien ici ne \
         sait reconnaître la trame que le §3.6 interdit entre deux images",
        mode[0]
    );

    for (role, trame) in boucle_d_image() {
        assert_ne!(
            trame, mode,
            "{role} est devenue la trame de mode d'affichage elle-même. Le §3.6 est formel : sur \
             les cinquante images de la capture de CAM, aucune trame `38` ne passe entre deux \
             images — CAM ne l'émet qu'à l'initialisation (§3.1). Émettre `38 01 02 00` par image \
             réinitialise le pipeline d'affichage (§3.4), et c'est le clignotement de 25 s observé \
             le 2026-08-16 (issue #137)"
        );

        assert_ne!(
            trame[0], COMMANDE_MODE,
            "{role} commence par {COMMANDE_MODE:#04x} : c'est une commande de mode d'affichage \
             (§3.5), et le §3.6 interdit qu'il en passe une entre deux images"
        );

        assert_ne!(
            trame[0], COMMANDE_BUCKET,
            "{role} commence par {COMMANDE_BUCKET:#04x} : c'est une commande de bucket (§3.1), et \
             le §3.6 la range avec la précédente — « liquidctl interroge les buckets […] à chaque \
             image. CAM ne fait rien de tout cela »"
        );

        assert_eq!(
            trame[0], COMMANDE_IMAGE,
            "{role} devrait commencer par {COMMANDE_IMAGE:#04x} — la boucle du §3.2 n'émet que \
             `36 01 00 01 09` puis `36 02` —, elle commence par {:#04x}",
            trame[0]
        );
    }
}

// ---------------------------------------------------------------------------
// §3.2 — le verdict de chaque étape, que le critère 2 demande de ne pas perdre
// ---------------------------------------------------------------------------

#[test]
fn chaque_etape_de_l_image_attend_un_accuse_dont_le_verdict_vaut_zero_un() {
    // Spec §3.2, ✅ : « Le contrôleur accuse chaque étape, et il faut attendre l'accusé. C'est le
    // point qui a coûté le plus cher à l'implémentation Linux : trois vérifications matérielles
    // successives sans aucune image, alors que toutes les trames étaient correctes. » L'octet à
    // l'offset 14 porte le verdict, `01` pour un succès.
    //
    // Le critère 2 de #137 exige que la séquence reste inchangée « pour le reste » : `36 01`,
    // attente de `37 01`, transferts bulk, `36 02`, attente de `37 02`. L'attente elle-même n'est
    // pas observable d'ici (voir l'en-tête) ; ce qu'on attend l'est, et rien ne le figeait.
    for sous_commande in [0x01u8, 0x02] {
        assert!(
            screen::check_ack(&accuse(sous_commande, VERDICT_SUCCES)).is_ok(),
            "un accusé `37 {sous_commande:02x}` portant {VERDICT_SUCCES:#04x} à l'offset {} est un \
             succès (spec §3.2, et `liquidctl` teste le même octet) ; le refuser ferait renoncer la \
             vigie au bout de trois images parfaitement valides (#70)",
            screen::ACK_VERDICT_OFFSET
        );
    }

    // Le pendant : un accusé qui ne dit pas succès doit être vu, pas traversé. C'est ce qui
    // distingue un refus du contrôleur d'une image affichée — et le §2.2.1 rappelle que sur cette
    // cible, l'échec silencieux est la règle plutôt que l'exception.
    for verdict in [0x00u8, 0x02, 0xff] {
        let erreur = screen::check_ack(&accuse(0x01, verdict)).expect_err(&format!(
            "un accusé portant {verdict:#04x} à l'offset {} a été pris pour un succès. Seul \
                 {VERDICT_SUCCES:#04x} en est un (spec §3.2) : traverser les autres, c'est \
                 poursuivre la séquence d'image sur un contrôleur qui vient de refuser l'étape \
                 précédente",
            screen::ACK_VERDICT_OFFSET
        ));

        assert_eq!(
            erreur.found, verdict,
            "le refus doit rappeler l'octet trouvé, {verdict:#04x} ; il annonce {:#04x}. Un \
             verdict inexact dans le journal envoie chercher la panne au mauvais endroit",
            erreur.found
        );
    }
}

// ---------------------------------------------------------------------------
// #137 — « une fois par ouverture, et plus jamais entre deux images »
//
// Ces quatre tests portent le cœur de l'issue. À l'écriture de ce fichier,
// `reverb_daemon::fil_ecran::ModeDeDiffusion` **n'existe pas** : la compilation échoue, et c'est
// la phase rouge.
// ---------------------------------------------------------------------------

/// Le nombre d'images qu'on pousse pour vérifier que le compte des armements ne dérive pas.
///
/// Cinquante, comme la capture de CAM du §3.6 — « sur les cinquante images de la capture, la
/// boucle est strictement `36 01` / bulk / `36 02` ». Le chiffre n'est donc pas rond par hasard :
/// c'est l'échantillon sur lequel le résultat négatif a été établi.
const IMAGES_DE_LA_CAPTURE: usize = 50;

#[test]
fn un_mode_neuf_n_est_pas_arme_et_sa_premiere_question_rend_vrai() {
    // Spec §2.2.1, ✅ : « Le mode d'affichage doit être forcé avant l'envoi : `38 01 02`. Sans
    // cela, l'écran reste sur son affichage intégré et ignore silencieusement l'image — l'envoi
    // réussit, aucun code d'erreur, mais rien n'apparaît. C'est le piège de cette cible. »
    //
    // C'est la moitié de #137 qu'il ne faut surtout pas casser en corrigeant l'autre : cesser
    // d'émettre la trame *tout court* ferait disparaître l'image pour de bon, et sans un mot.
    // L'issue le dit dans sa section « Comportement attendu » : la trame est émise **une fois par
    // ouverture du périphérique**, pas zéro.
    //
    // ⚠️ « Un mode neuf n'est pas armé » s'énonce ici par sa seule observation possible, et c'est
    // voulu : le drapeau n'a pas de lecteur séparé (voir l'en-tête). Demander un `arme()` en plus
    // donnerait deux façons de poser la même question, dont une seule serait consultée par
    // `image()` — et le jour où elles divergeraient, c'est celle que le test ne regarde pas qui
    // déciderait de ce que le boîtier affiche.
    let mut mode = ModeDeDiffusion::default();

    assert!(
        mode.faut_il_armer(),
        "un `ModeDeDiffusion` neuf a répondu qu'il n'y avait pas à armer. Sur un périphérique qui \
         vient d'être ouvert, personne n'a encore émis `38 01 02 00` : sans elle, le téléversement \
         réussit et l'image n'apparaît jamais (spec §2.2.1). L'échec est silencieux — aucun code \
         d'erreur ne le dirait"
    );
}

#[test]
fn deux_images_consecutives_n_arment_le_mode_qu_une_seule_fois() {
    // Critère d'acceptation n° 1 de l'issue #137 : « Deux images consécutives poussées sur un même
    // périphérique ouvert n'émettent la trame de mode qu'une seule fois, avant la première. »
    //
    // Spec §3.6, ✅ : « Aucune trame `32` ni `38` entre deux images. » CAM ne l'émet qu'à
    // l'initialisation (§3.1) ; le §3.4 constate qu'une commande d'affichage réinitialise le
    // pipeline, et c'est le retour au noir observé toutes les 25 s le 2026-08-16.
    let mut mode = ModeDeDiffusion::default();

    assert!(
        mode.faut_il_armer(),
        "la première image d'un périphérique ouvert doit armer le mode de diffusion (spec §2.2.1)"
    );

    assert!(
        !mode.faut_il_armer(),
        "la seconde image a redemandé à armer le mode. C'est exactement le défaut de #137 : \
         `38 01 02 00` réémise avant chaque image réinitialise le pipeline d'affichage (spec \
         §3.4), la dalle passe au noir puis revient, et le §3.6 est formel — sur les cinquante \
         images de la capture, CAM n'émet aucune trame `38` entre deux images"
    );
}

#[test]
fn une_longue_suite_d_images_n_arme_toujours_qu_une_seule_fois() {
    // Le même critère n° 1, mais sur la durée : ce que #137 mesure, ce n'est pas un armement de
    // trop, c'est ~3 456 réinitialisations de pipeline par jour sur un contrôleur dont
    // l'implémentation de référence n'en inflige qu'une par session. Un correctif qui rearmerait
    // une fois sur deux, ou tous les N, passerait le test précédent et laisserait le boîtier
    // clignoter.
    //
    // ⚠️ Ce n'est pas cosmétique : c'est la seule divergence connue entre ce que Reverb envoie au
    // Kraken et ce que CAM lui envoie (§3.6), et le Kraken se bloque périodiquement pour une
    // raison qui reste ouverte (#98).
    let mut mode = ModeDeDiffusion::default();

    let armements = (0..IMAGES_DE_LA_CAPTURE)
        .filter(|_| mode.faut_il_armer())
        .count();

    assert_eq!(
        armements, 1,
        "sur {IMAGES_DE_LA_CAPTURE} images poussées sur un même périphérique ouvert, le mode a été \
         armé {armements} fois au lieu d'une seule. La capture du §3.6 porte exactement ce nombre \
         d'images et n'y montre aucune trame `38` : au rythme de réémission de 25 s, chaque \
         armement de trop est une réinitialisation de pipeline de plus, soit ~3 456 par jour pour \
         une image qui ne bouge pas"
    );
}

#[test]
fn un_peripherique_rouvert_rearme_le_mode() {
    // Critère d'acceptation n° 3 de l'issue #137 : « Un périphérique rouvert réarme le mode avant
    // sa première image. » Le cas concret est celui de #98 : quand une source du Kraken se tait
    // entièrement, le démon tente un `USBDEVFS_RESET`, et la poignée usbfs est **lâchée puis
    // rouverte** — un reset l'invalide. Le périphérique qui revient n'a rien gardé : rien ne
    // survit côté matériel, et il faut lui réémettre `38 01 02 00` avant sa première image (spec
    // §2.2.1), sans quoi la dalle reste muette pour de bon après chaque réparation.
    //
    // L'issue le résout sans une ligne de plus : `Kraken::ouvrir` construit un
    // `ModeDeDiffusion::default()`, donc une réouverture repart d'un état neuf.
    let mut avant = ModeDeDiffusion::default();
    for _ in 0..IMAGES_DE_LA_CAPTURE {
        let _ = avant.faut_il_armer();
    }

    let mut apres_reouverture = ModeDeDiffusion::default();

    assert!(
        apres_reouverture.faut_il_armer(),
        "un `ModeDeDiffusion` neuf a refusé d'armer parce qu'un autre l'avait déjà fait. L'état \
         est donc partagé entre instances — un `static`, un `AtomicBool` ou un `OnceLock` —, et \
         c'est le critère n° 3 de #137 qui tombe : après le reset USB de #98, la poignée usbfs est \
         lâchée puis rouverte, et la dalle ne réafficherait plus jamais rien. L'échec serait \
         silencieux (spec §2.2.1) et surviendrait précisément pendant une réparation"
    );

    assert!(
        !avant.faut_il_armer(),
        "l'instance d'avant la réouverture s'est remise à demander un armement. Construire un état \
         neuf ne doit rien remettre à zéro chez le voisin : deux périphériques ouverts, ou une \
         poignée qu'on croyait lâchée, se mettraient à réémettre `38 01 02 00` entre deux images — \
         le clignotement de #137, par l'autre bout"
    );
}
