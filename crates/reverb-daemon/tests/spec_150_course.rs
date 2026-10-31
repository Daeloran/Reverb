//! Tests d'intention de l'issue #150 côté démon : la `largeur` de `course` traverse
//! `eclairage.conf`, `zones.conf` et les profils, et ne s'écrit pas quand elle vaut son défaut.
//!
//! Écrits **depuis l'issue #150 seule**, avant l'implémentation. Rien de `crates/*/src/` n'a été lu
//! pour les produire, hors les signatures publiques nécessaires à la compilation ; les idiomes
//! (témoins, dossier jetable, jetons `cle=valeur`) viennent de `spec_149_vitesse.rs`. Si l'un de ces
//! tests échoue après implémentation, c'est le code qu'on corrige.
//!
//! # Le contrat de signature supposé
//!
//! `Reglages` gagne un champ public `largeur: u8`, de 1 à 10, 4 par défaut, lu et écrit sous la clé
//! `largeur` par la seule famille `course` — voir l'en-tête de
//! `crates/reverb-anim/tests/spec_150_course.rs`, qui le fige et vérifie l'écriture
//! (`reglages_ecrits`, l'émetteur de `lighting`) et la lecture sur les dix largeurs.
//!
//! # Ce que ce fichier vérifie
//!
//! Critère d'acceptation n° 5 : « `largeur` est écrit […] dans les profils, et relu à l'identique.
//! Absent, il vaut 4 et ne s'écrit pas ». La couche globale et une zone, parce qu'un profil emporte
//! les deux et qu'elles passent par deux écritures du même réglage ; et `eclairage.conf`, qui est
//! ce que le démon relit au redémarrage.

use std::fs;
use std::path::{Path, PathBuf};

use reverb_anim::{Animation, Direction, Reglages};
use reverb_daemon::persistance::{Eclairage, charger_eclairage, enregistrer_eclairage};
use reverb_daemon::profils::{Profil, charger, enregistrer};
use reverb_daemon::zones::Zones;
use reverb_proto::ram::SLOT_COUNT;
use reverb_proto::{Led, NomProfil, Rgb};

// ---------------------------------------------------------------------------
// Témoins
// ---------------------------------------------------------------------------

/// Le nom de la famille, tel que l'issue le fixe.
const COURSE: &str = "course";

/// La largeur par défaut, selon l'issue : elle ne s'écrit pas.
const LARGEUR_PAR_DEFAUT: u8 = 4;

/// Une largeur qui n'est pas le défaut, et qui diffère de [`LARGEUR_DE_ZONE`] : un profil qui
/// recopierait l'une dans l'autre ne passerait pas.
const LARGEUR_GLOBALE: u8 = 7;

/// Voir [`LARGEUR_GLOBALE`]. Une borne du domaine, pour qu'elle soit éprouvée aussi.
const LARGEUR_DE_ZONE: u8 = 1;

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
}

fn leds(slug: &str) -> Vec<Led> {
    Led::depuis_slug(slug).unwrap_or_else(|erreur| panic!("« {slug} » désigne des LED : {erreur}"))
}

/// Des réglages de `course` à la largeur donnée, couleur, vitesse et direction hors défaut.
fn reglages(largeur: u8) -> Reglages {
    Reglages {
        couleur: Rgb::new(0x00, 0xff, 0x00),
        vitesse: 2.5,
        direction: Direction::HautBas,
        largeur,
        ..Reglages::default()
    }
}

/// Un éclairage fixe sous une `course` à la largeur donnée.
fn eclairage(largeur: u8) -> Eclairage {
    Eclairage {
        ventilateurs: [Rgb::new(0x10, 0x80, 0xf0); 10],
        barrettes: [Rgb::new(0xa0, 0x30, 0x05); SLOT_COUNT],
        animation: Some((animation(COURSE), reglages(largeur))),
    }
}

/// Un profil dont la couche globale court à `globale` et une zone à `de_zone`.
fn profil(globale: u8, de_zone: u8) -> Profil {
    let mut zones = Zones::vide();
    zones.poser("barre", &leds("slot:1"));
    zones.animer(
        "barre",
        Some((
            animation(COURSE),
            Reglages {
                couleur: Rgb::new(0x12, 0x34, 0x56),
                direction: Direction::BordsCentre,
                largeur: de_zone,
                ..Reglages::default()
            },
        )),
    );
    Profil {
        eclairage: eclairage(globale),
        zones,
        ecran: None,
    }
}

/// Toutes les valeurs écrites après `largeur=` dans les lignes d'entrée d'un texte, triées.
///
/// Le découpage suit les jetons séparés par des blancs, et les commentaires sont écartés, comme
/// `spec_149_vitesse.rs` le fait pour `vitesse=`.
fn largeurs_ecrites(texte: &str) -> Vec<String> {
    let mut valeurs: Vec<String> = texte
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with('#'))
        .flat_map(str::split_whitespace)
        .filter_map(|jeton| jeton.strip_prefix("largeur="))
        .map(str::to_owned)
        .collect();
    valeurs.sort();
    valeurs
}

/// Un dossier de travail sous `std::env::temp_dir()`, effacé à la fin du test par `Drop`.
struct DossierJetable {
    chemin: PathBuf,
}

impl DossierJetable {
    fn neuf(nom: &str) -> DossierJetable {
        let chemin =
            std::env::temp_dir().join(format!("reverb-spec-150-{}-{nom}", std::process::id()));
        let _ = fs::remove_dir_all(&chemin);
        fs::create_dir_all(&chemin)
            .unwrap_or_else(|erreur| panic!("dossier de test {} : {erreur}", chemin.display()));
        DossierJetable { chemin }
    }

    fn chemin(&self) -> &Path {
        &self.chemin
    }
}

impl Drop for DossierJetable {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.chemin);
    }
}

// ---------------------------------------------------------------------------
// eclairage.conf
// ---------------------------------------------------------------------------

#[test]
fn la_largeur_de_course_traverse_eclairage_conf_et_son_defaut_ne_s_ecrit_pas() {
    // Critère n° 5, côté `eclairage.conf` : la largeur posée est celle qui revient au redémarrage ;
    // à 4, rien n'est écrit, et la relecture redonne 4.
    let dossier = DossierJetable::neuf("eclairage");
    for largeur in 1..=10u8 {
        let pose = eclairage(largeur);
        let attendues: Vec<String> = if largeur == LARGEUR_PAR_DEFAUT {
            Vec::new()
        } else {
            vec![largeur.to_string()]
        };

        let texte = pose.encoder();
        assert_eq!(
            largeurs_ecrites(&texte),
            attendues,
            "la largeur {largeur} s'écrit {attendues:?} dans eclairage.conf :\n{texte}"
        );
        let relu = Eclairage::decoder(&texte).unwrap_or_else(|erreur| {
            panic!("eclairage.conf doit relire ce qu'il écrit ({erreur}) :\n{texte}")
        });
        assert_eq!(
            relu, pose,
            "la largeur {largeur} traverse l'aller-retour par le texte"
        );

        let chemin = dossier.chemin().join("eclairage.conf");
        enregistrer_eclairage(&chemin, &pose)
            .unwrap_or_else(|erreur| panic!("écriture de {} : {erreur}", chemin.display()));
        let (charge, signalement) = charger_eclairage(&chemin);
        assert_eq!(
            signalement, None,
            "un fichier écrit par le démon se relit sans un mot — largeur {largeur}"
        );
        assert_eq!(charge, pose, "la largeur {largeur} survit au redémarrage");
    }
}

// ---------------------------------------------------------------------------
// Profils (et zones, qu'un profil emporte)
// ---------------------------------------------------------------------------

#[test]
fn la_largeur_de_course_traverse_un_profil_et_ses_zones() {
    // Critère n° 5, côté profils : « écrit dans les profils, et relu à l'identique ». Deux largeurs
    // différentes, hors défaut, pour la couche globale et la zone.
    let dossier = DossierJetable::neuf("profils");
    let nom = NomProfil::nouveau("course").expect("« course » est un nom de profil");
    let pose = profil(LARGEUR_GLOBALE, LARGEUR_DE_ZONE);

    let texte = pose.encoder();
    let mut attendues = vec![LARGEUR_GLOBALE.to_string(), LARGEUR_DE_ZONE.to_string()];
    attendues.sort();
    assert_eq!(
        largeurs_ecrites(&texte),
        attendues,
        "le profil écrit « largeur={LARGEUR_GLOBALE} » pour la couche globale et \
         « largeur={LARGEUR_DE_ZONE} » pour la zone :\n{texte}"
    );
    let relu = Profil::decoder(&texte).unwrap_or_else(|erreur| {
        panic!("un profil doit relire ce qu'il écrit ({erreur}) :\n{texte}")
    });
    assert_eq!(
        relu, pose,
        "les deux largeurs traversent l'aller-retour par le texte"
    );

    let texte_des_zones = pose.zones.encoder();
    assert_eq!(
        largeurs_ecrites(&texte_des_zones),
        vec![LARGEUR_DE_ZONE.to_string()],
        "zones.conf écrit « largeur={LARGEUR_DE_ZONE} » :\n{texte_des_zones}"
    );
    assert_eq!(
        Zones::decoder(&texte_des_zones).unwrap_or_else(|erreur| {
            panic!("zones.conf doit relire ce qu'il écrit ({erreur}) :\n{texte_des_zones}")
        }),
        pose.zones,
        "la largeur d'une zone traverse zones.conf"
    );

    enregistrer(dossier.chemin(), &nom, &pose).expect("l'enregistrement réussit");
    let charge = charger(dossier.chemin(), &nom).expect("le profil enregistré se recharge");
    assert_eq!(charge, pose, "les deux largeurs survivent au disque");
}

#[test]
fn un_profil_a_la_largeur_par_defaut_n_ecrit_aucune_largeur_et_la_relit_a_quatre() {
    // Critère n° 5, seconde moitié : « absent, il vaut 4 et ne s'écrit pas ». Un profil écrit avant
    // #150 n'a pas de clé `largeur` : relu, il doit tourner à 4, et réenregistré, il ne doit pas
    // changer d'octets.
    let pose = profil(LARGEUR_PAR_DEFAUT, LARGEUR_PAR_DEFAUT);
    let texte = pose.encoder();
    assert_eq!(
        largeurs_ecrites(&texte),
        Vec::<String>::new(),
        "à la largeur par défaut, le profil n'écrit aucune largeur :\n{texte}"
    );
    let relu = Profil::decoder(&texte).unwrap_or_else(|erreur| {
        panic!("un profil doit relire ce qu'il écrit ({erreur}) :\n{texte}")
    });
    let largeur_relue = relu
        .eclairage
        .animation
        .as_ref()
        .map(|(_, reglages)| reglages.largeur);
    assert_eq!(
        largeur_relue,
        Some(LARGEUR_PAR_DEFAUT),
        "sans clé `largeur`, la couche globale court à la largeur 4"
    );
    assert_eq!(
        relu, pose,
        "le profil à la largeur par défaut traverse l'aller-retour"
    );
    assert_eq!(
        relu.encoder(),
        texte,
        "relu puis réécrit, le profil ne change pas d'octets"
    );
}
