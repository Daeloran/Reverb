//! Le nom d'une zone née d'une sélection est un **identifiant**, pas un libellé.
//!
//! Tests de logique, écrits avec le correctif. Ils tiennent en une phrase :
//! *deux sélections différentes ne doivent jamais porter le même nom de zone.*
//!
//! # Ce qu'ils empêchent de revenir
//!
//! Colorer une sélection partielle pendant qu'une animation tourne la déclare
//! en zone (#63). Ce nom-là était `Selection::nom()`, le libellé d'affichage :
//! « 3 organes entiers », « 24 LED sur 4 organes ». Deux sélections
//! **différentes** de trois organes entiers s'y écrivaient pareil, si bien que
//! la seconde réécrivait la zone de la première et lui prenait ses LED — une
//! LED n'appartenant qu'à une zone à la fois.
//!
//! Observé sur SHYNAEL le 2026-08-08 : « j'ai attribué une couleur à une zone,
//! ça l'a aussi attribué et changé l'animation d'une autre, et pas à une
//! troisième ». Aucun message ne le signalait : les zones existaient toujours,
//! seul leur contenu avait changé.

use std::collections::HashSet;

use reverb_gui::plan::{Cible, nom_de_zone};
use reverb_proto::Position;
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};

/// Les huit LED d'un ventilateur.
fn ventilateur(position: Position) -> Vec<Cible> {
    (0..8).map(|led| Cible::Led { position, led }).collect()
}

/// Les onze LED d'une barrette.
fn barrette(slot: usize) -> Vec<Cible> {
    (0..LEDS_PER_STICK)
        .map(|led| Cible::Barrette { slot, led })
        .collect()
}

#[test]
fn deux_selections_de_trois_organes_ne_portent_pas_le_meme_nom() {
    // **Le défaut, nommément.** `Selection::nom()` rendait « 3 organes entiers »
    // pour ces deux-là, et la seconde volait ses LED à la première.
    let premiere: Vec<Cible> = [Position::ALL[0], Position::ALL[1], Position::ALL[2]]
        .into_iter()
        .flat_map(ventilateur)
        .collect();
    let seconde: Vec<Cible> = [Position::ALL[3], Position::ALL[4], Position::ALL[5]]
        .into_iter()
        .flat_map(ventilateur)
        .collect();

    assert_ne!(
        nom_de_zone(&premiere),
        nom_de_zone(&seconde),
        "trois ventilateurs et trois AUTRES ventilateurs sont deux zones : leur donner le même \
         nom fait que la seconde prend les LED de la première, sans un message"
    );
}

#[test]
fn la_meme_selection_rend_toujours_le_meme_nom() {
    // Le déterminisme reste la propriété qu'on veut : sans lui, chaque clic
    // empilerait une zone de plus, et le panneau se remplirait de doublons.
    let selection = ventilateur(Position::ALL[0]);
    assert_eq!(
        nom_de_zone(&selection),
        nom_de_zone(&selection.clone()),
        "deux fois la même sélection, c'est la même zone"
    );
}

#[test]
fn l_ordre_du_geste_ne_change_pas_le_nom() {
    // Les cibles sont ajoutées dans l'ordre où la souris les attrape. Tracer un
    // rectangle de gauche à droite ou de droite à gauche prend les mêmes LED :
    // c'est la même zone, et deux noms en feraient deux.
    let dans_l_ordre = ventilateur(Position::ALL[0]);
    let mut a_l_envers = dans_l_ordre.clone();
    a_l_envers.reverse();

    assert_eq!(
        nom_de_zone(&dans_l_ordre),
        nom_de_zone(&a_l_envers),
        "le sens du geste n'est pas une propriété de la sélection"
    );
}

#[test]
fn un_ventilateur_et_une_barrette_ne_se_confondent_pas() {
    // Les deux familles de cibles vivent dans le même espace de noms. Un rang
    // écrit sans dire de quelle famille il vient les ferait collisionner —
    // c'est le piège classique d'une clef composite aplatie.
    let mut vus = HashSet::new();
    for position in Position::ALL {
        assert!(
            vus.insert(nom_de_zone(&ventilateur(position))),
            "« {} » porte un nom déjà pris",
            position.slug()
        );
    }
    for slot in 0..SLOT_COUNT {
        assert!(
            vus.insert(nom_de_zone(&barrette(slot))),
            "la barrette {slot} porte un nom déjà pris"
        );
    }
}

#[test]
fn chaque_led_prise_seule_porte_un_nom_a_elle() {
    // Le balayage complet : 124 LED, 124 noms. C'est la borne haute du nombre
    // de sélections d'une seule cible, et la façon la plus dense de faire
    // collisionner une empreinte de 32 bits — si elle devait collisionner.
    let mut vus = HashSet::new();
    let mut combien = 0;
    for position in Position::ALL {
        for led in 0..8 {
            combien += 1;
            assert!(
                vus.insert(nom_de_zone(&[Cible::Led { position, led }])),
                "la LED {led} de « {} » porte un nom déjà pris",
                position.slug()
            );
        }
    }
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            combien += 1;
            assert!(
                vus.insert(nom_de_zone(&[Cible::Barrette { slot, led }])),
                "la LED {led} de la barrette {slot} porte un nom déjà pris"
            );
        }
    }
    assert_eq!(combien, 124, "le boîtier compte cent vingt-quatre LED");
}

#[test]
fn le_nom_est_utilisable_tel_quel_comme_nom_de_fichier_et_de_zone() {
    // Une zone finit dans `/var/lib/reverb/zones.conf`, sur une ligne du
    // protocole texte. Un nom qui porterait un blanc, un séparateur de chemin
    // ou un caractère de contrôle casserait l'un ou l'autre — et le nom n'est
    // plus tapé par un humain depuis ce correctif, donc plus rien ne l'y borne.
    let nom = nom_de_zone(&ventilateur(Position::ALL[0]));
    assert!(
        nom.starts_with("sélection-"),
        "le nom doit dire d'où la zone vient : « {nom} »"
    );
    assert!(
        !nom.contains(char::is_whitespace),
        "un blanc couperait la ligne du protocole en deux : « {nom} »"
    );
    assert!(
        !nom.contains(['/', '\\', '.']),
        "un séparateur de chemin sortirait du répertoire : « {nom} »"
    );
    assert!(
        !nom.chars().any(char::is_control),
        "un caractère de contrôle traverserait le socket : « {nom} »"
    );
}
