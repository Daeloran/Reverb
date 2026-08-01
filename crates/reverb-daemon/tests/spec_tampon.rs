//! Tests d'intention du tampon des cent vingt-quatre LED — issue #29.
//!
//! Écrits **avant** l'implémentation, depuis le contrat public de `crates/reverb-daemon/src/
//! zones.rs` — `Tampon::noir`, `Tampon::couleur`, `Tampon::poser`, corps `todo!("issue #29")` à
//! l'écriture de ce fichier.
//!
//! ## Pourquoi un fichier pour deux méthodes
//!
//! `Tampon` est l'adressage du boîtier : la traduction d'une `Led` — un ventilateur et un rang, ou
//! une barrette et un rang — vers une case de deux tableaux de formes différentes, `[[Rgb; 8]; 10]`
//! et `[[Rgb; 11]; 4]`. Tout ce que les zones affichent passe par là.
//!
//! Une erreur d'adressage ne produit **aucun** message. Elle produit une LED qui s'allume au
//! mauvais endroit, ou deux LED qui changent ensemble parce qu'elles partagent une case, ou une
//! troisième qui ne change jamais. Rien de tout cela ne se voit dans un test qui pose une couleur
//! et la relit au même endroit — l'aliasing est symétrique, il traverse l'aller-retour sans
//! broncher. Ce fichier le débusque de la seule façon possible : **cent vingt-quatre couleurs
//! distinctes posées ensemble**, puis relues une par une.
//!
//! Le test d'intention n° 10 de l'issue — « une image composée porte exactement 124 LED, une seule
//! fois chacune » — se ramène exactement à cette propriété, et c'est ici qu'elle est prouvée.
//!
//! ## Un point que le contrat laisse ouvert, et que ces tests tranchent
//!
//! **`Led::toutes` énumère les cent vingt-quatre LED sans répétition et sans manque.** Le contrat
//! dit « toutes les LED du boîtier, dans l'ordre du matériel » sans le chiffrer ; ce fichier le
//! chiffre depuis le matériel — dix anneaux de huit, quatre barrettes de onze — plutôt que de
//! recopier `124`, pour qu'une barrette ajoutée un jour fasse suivre le test.

use reverb_anim::Geometrie;
use reverb_daemon::zones::{Tampon, Zones};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Led, Position, Rgb};

/// Le nombre de LED du boîtier, recalculé depuis le matériel.
const LEDS_DU_BOITIER: usize = 10 * LEDS_PER_FAN as usize + SLOT_COUNT * LEDS_PER_STICK;

/// La couleur témoin de la LED de rang `rang`.
///
/// Deux propriétés, chacune pour un mode de défaillance précis :
/// - **toutes distinctes**, sinon deux LED qui partageraient une case du tampon ne se
///   contrediraient jamais ;
/// - **`r` différent de `b`**, sinon une permutation de composantes traverserait l'aller-retour
///   sans un message — et le projet mêle trois ordres de composantes.
fn teinte(rang: usize) -> Rgb {
    let graine = u8::try_from(rang).expect("cent vingt-quatre LED tiennent dans un u8");
    Rgb::new(0x10 + graine, 0x40 ^ graine, 0x90u8.wrapping_sub(graine))
}

// ---------------------------------------------------------------------------
// L'énumération des LED
// ---------------------------------------------------------------------------

#[test]
fn le_boitier_porte_cent_vingt_quatre_led_toutes_distinctes() {
    // Contrat — `Led::toutes` : « toutes les LED du boîtier, dans l'ordre du matériel ».
    //
    // Un doublon ferait compter une LED deux fois partout où le démon compte, et un manque ferait
    // une LED qu'aucune zone ne peut désigner — invisible dans la fenêtre, allumée dans le boîtier,
    // et sans explication.
    let toutes = Led::toutes();
    assert_eq!(
        toutes.len(),
        LEDS_DU_BOITIER,
        "dix anneaux de {LEDS_PER_FAN} LED et {SLOT_COUNT} barrettes de {LEDS_PER_STICK} font \
         {LEDS_DU_BOITIER} LED"
    );

    let mut uniques = toutes.clone();
    uniques.sort_unstable();
    uniques.dedup();
    assert_eq!(
        uniques.len(),
        LEDS_DU_BOITIER,
        "aucune LED ne doit paraître deux fois dans l'énumération"
    );

    // Et chacune y est nommément : un décodage qui oublierait la dernière barrette rendrait 113
    // LED, un compte plausible et faux de onze.
    for position in Position::ALL {
        for led in 0..LEDS_PER_FAN as usize {
            let cible = Led::Ventilateur { position, led };
            assert!(
                toutes.contains(&cible),
                "{cible:?} manque à l'énumération du boîtier"
            );
        }
    }
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            let cible = Led::Barrette { slot, led };
            assert!(
                toutes.contains(&cible),
                "{cible:?} manque à l'énumération du boîtier"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// `poser` puis `couleur`
// ---------------------------------------------------------------------------

#[test]
fn un_tampon_noir_est_noir_partout() {
    // `Tampon::noir` est le point de départ de chaque tour de rendu : s'il portait autre chose que
    // du noir, une LED que personne n'a peinte s'allumerait toute seule.
    let tampon = Tampon::noir();
    for led in Led::toutes() {
        assert_eq!(
            tampon.couleur(led),
            Rgb::BLACK,
            "{led:?} doit être éteinte dans un tampon noir"
        );
    }
}

#[test]
fn le_tampon_range_chaque_led_a_la_place_que_le_materiel_lui_donne() {
    // `Tampon` porte deux champs **publics**, `ventilateurs` et `barrettes` : c'est par eux que la
    // boucle de rendu écrit sur les bus, et leur indexation est donc du contrat, pas du détail.
    //
    // Sans ce test, une permutation **symétrique** entre `poser` et `couleur` — les deux indexant
    // `ventilateurs[9 - position.index()]`, par exemple — traverserait tous les autres tests de ce
    // fichier sans un mot : on relit toujours ce qu'on a écrit. Elle allumerait pourtant le
    // ventilateur d'en face, et personne ne saurait pourquoi.
    let mut tampon = Tampon::noir();
    for (rang, position) in Position::ALL.into_iter().enumerate() {
        for led in 0..LEDS_PER_FAN as usize {
            let couleur = teinte(rang * LEDS_PER_FAN as usize + led);
            tampon.poser(Led::Ventilateur { position, led }, couleur);
            assert_eq!(
                tampon.ventilateurs[position.index()][led],
                couleur,
                "la LED {led} de « {} » se range en ventilateurs[{}][{led}] — c'est cette case que \
                 la boucle de rendu envoie sur son canal",
                position.name(),
                position.index()
            );
        }
    }
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            let couleur = teinte(slot * LEDS_PER_STICK + led);
            tampon.poser(Led::Barrette { slot, led }, couleur);
            assert_eq!(
                tampon.barrettes[slot][led], couleur,
                "la LED {led} de la barrette {slot} se range en barrettes[{slot}][{led}]"
            );
        }
    }
}

#[test]
fn chaque_led_relit_la_couleur_qu_on_lui_a_posee() {
    // Le cœur du fichier. Les cent vingt-quatre couleurs sont posées **ensemble** puis relues
    // ensemble : poser et relire une seule LED à la fois passerait même si toutes partageaient la
    // même case, puisque la dernière écrite serait toujours celle qu'on relit.
    let mut tampon = Tampon::noir();
    for (rang, led) in Led::toutes().into_iter().enumerate() {
        tampon.poser(led, teinte(rang));
    }

    for (rang, led) in Led::toutes().into_iter().enumerate() {
        let attendue = teinte(rang);
        let relue = tampon.couleur(led);
        assert_eq!(
            relue, attendue,
            "{led:?} (rang {rang}) doit porter sa propre couleur. Deux LED qui partagent une case \
             du tampon changent ensemble dans le boîtier, et rien ne le signale."
        );
        // Composante par composante : une permutation traverserait l'égalité ci-dessus si `poser`
        // et `couleur` permutaient toutes les deux, ce qu'un copier-coller de `to_grb` produit.
        assert_eq!(relue.r, attendue.r, "rouge de {led:?}");
        assert_eq!(relue.g, attendue.g, "vert de {led:?}");
        assert_eq!(relue.b, attendue.b, "bleu de {led:?}");
    }
}

#[test]
fn poser_une_led_ne_touche_a_aucune_autre() {
    // La propriété qui rend une zone possible : écrire ses LED sans effacer celles du voisin. Un
    // `poser` qui déborderait d'une case — un `for` sur l'anneau entier au lieu de la LED visée —
    // repeindrait un ventilateur complet à chaque clic sur une seule de ses LED.
    //
    // Les frontières sont les endroits où ça casse : dernière LED d'un anneau, première du suivant,
    // dernière d'une barrette. Elles sont toutes visitées puisque la boucle passe sur les cent
    // vingt-quatre.
    let toutes = Led::toutes();
    let fond = Rgb::new(0x01, 0x02, 0x03);
    let posee = Rgb::new(0xff, 0x7f, 0x00);
    assert_ne!(
        fond, posee,
        "la couleur posée doit différer du fond, sinon ce test ne prouve rien"
    );

    for (rang, cible) in toutes.iter().enumerate() {
        let mut tampon = Tampon::noir();
        for led in &toutes {
            tampon.poser(*led, fond);
        }
        tampon.poser(*cible, posee);

        assert_eq!(
            tampon.couleur(*cible),
            posee,
            "{cible:?} (rang {rang}) doit porter la couleur qu'on vient d'y poser"
        );
        for autre in &toutes {
            if autre != cible {
                assert_eq!(
                    tampon.couleur(*autre),
                    fond,
                    "poser {cible:?} ne doit pas toucher {autre:?} : c'est ce qui permet à une \
                     zone d'écrire ses LED sans effacer celles du voisin"
                );
            }
        }
    }
}

#[test]
fn deux_tampons_sont_egaux_exactement_quand_toutes_leurs_led_le_sont() {
    // `Tampon` dérive `PartialEq`, et c'est par cette égalité que les tests de composition disent
    // « rien n'a bougé ». Une égalité qui ne regarderait qu'un des deux bus — c'est ce qu'un
    // `PartialEq` écrit à la main sur le seul champ `ventilateurs` donnerait — rendrait tous ces
    // tests aveugles à la RAM.
    let mut reference = Tampon::noir();
    for (rang, led) in Led::toutes().into_iter().enumerate() {
        reference.poser(led, teinte(rang));
    }
    assert_eq!(
        reference,
        reference.clone(),
        "un tampon est égal à sa propre copie"
    );

    for led in Led::toutes() {
        let mut modifie = reference.clone();
        let avant = modifie.couleur(led);
        modifie.poser(led, Rgb::new(!avant.r, !avant.g, !avant.b));
        assert_ne!(
            modifie, reference,
            "changer {led:?} doit rendre les deux tampons différents — une égalité qui ignorerait \
             un bus rendrait aveugles tous les tests de composition"
        );
    }
}

#[test]
fn composer_sans_aucune_zone_laisse_le_tampon_intact() {
    // La couche globale seule, c'est-à-dire l'état d'avant l'issue #29 et celui d'un démon qui
    // vient de démarrer sur un `zones.conf` absent. Rien ne doit bouger : les zones s'ajoutent au
    // démon, elles ne changent pas ce qu'il faisait déjà.
    let mut pose = Tampon::noir();
    for (rang, led) in Led::toutes().into_iter().enumerate() {
        pose.poser(led, teinte(rang));
    }

    let mut compose = pose.clone();
    Zones::vide().composer(&Geometrie::mesuree(), 0, &mut compose);
    assert_eq!(
        compose, pose,
        "sans zone, la composition ne doit rien écrire : le boîtier affiche la couche globale telle \
         quelle"
    );
}
