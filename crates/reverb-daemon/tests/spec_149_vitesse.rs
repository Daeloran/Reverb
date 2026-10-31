//! Tests d'intention de l'issue #149 côté démon : une vitesse au dixième traverse `eclairage.conf`,
//! `zones.conf` et les profils, et s'y écrit comme on l'a tapée.
//!
//! Écrits **depuis l'issue #149 seule**, avant l'implémentation. Rien de `crates/*/src/` n'a été lu
//! pour les produire, hors les signatures publiques nécessaires à la compilation ; les idiomes
//! (témoins, dossier jetable, aller-retour) viennent de `spec_eclairage.rs`, `spec_profils.rs` et
//! `spec_zones.rs`. Si l'un de ces tests échoue après implémentation, c'est le code qu'on corrige.
//!
//! # Le contrat de signature supposé
//!
//! `Reglages::vitesse` est un **`f32`**, de 0.1 à 10 au dixième — voir l'en-tête de
//! `crates/reverb-anim/tests/spec_149_vitesse.rs`, qui le fige et vérifie l'écriture
//! (`reglages_ecrits`) et la lecture (`reglages`) sur les cent valeurs du domaine. Ce fichier-ci
//! vérifie que les trois fichiers du démon s'en servent, sans réécrire la vitesse à leur façon.
//!
//! # Ce que ce fichier vérifie
//!
//! Critère d'acceptation n° 4 : « aller-retour d'écriture : `0.7` → `0.7`, `3` → `3`, profil et
//! `eclairage.conf` compris ». Et le comportement attendu : « une vitesse entière s'écrit sans
//! décimale (`3`, et non `3.0`), une vitesse décimale avec un seul chiffre après le point (`0.7`) »
//! — sans quoi un fichier écrit avant #149, relu puis réenregistré, changerait d'octets sans que
//! rien ait changé.
//!
//! # Ce que ce fichier ne vérifie pas
//!
//! Le socket (`lighting`, `animate`) : la vitesse y voyage en texte, telle que `reglages_ecrits`
//! l'écrit et que `reglages` la lit — c'est ce que vérifie le fichier de `reverb-anim`, et ce
//! qu'éprouve la fenêtre dans `crates/reverb-gui/tests/spec_149_vitesse.rs`.

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

/// Les vitesses éprouvées, avec ce qui doit en être écrit.
///
/// `0.7` et `3` sont ceux de l'issue. `0.1` et `10` sont les deux bornes ; `0.5` est le cran le plus
/// bas de la fenêtre ; `9.9` a deux chiffres avant le point et un après ; `1` est l'ancien minimum.
const VITESSES: [(f32, &str); 7] = [
    (0.7, "0.7"),
    (3.0, "3"),
    (0.1, "0.1"),
    (0.5, "0.5"),
    (9.9, "9.9"),
    (10.0, "10"),
    (1.0, "1"),
];

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
}

fn leds(slug: &str) -> Vec<Led> {
    Led::depuis_slug(slug).unwrap_or_else(|erreur| panic!("« {slug} » désigne des LED : {erreur}"))
}

/// Des réglages dont la couleur et la direction diffèrent du défaut, comme les témoins de
/// `spec_eclairage.rs` : seule la vitesse varie d'un cas à l'autre.
fn reglages(vitesse: f32) -> Reglages {
    Reglages {
        couleur: Rgb::new(0x00, 0xff, 0x00),
        vitesse,
        direction: Direction::HautBas,
        sonde: None,
        palette: None,
        // #150 : `largeur` rejoint `Reglages` ; 4 est son défaut, sans effet hors `course`.
        largeur: 4,
    }
}

/// Un éclairage fixe sous une `vague` à la vitesse donnée.
fn eclairage(vitesse: f32) -> Eclairage {
    Eclairage {
        ventilateurs: [Rgb::new(0x10, 0x80, 0xf0); 10],
        barrettes: [Rgb::new(0xa0, 0x30, 0x05); SLOT_COUNT],
        animation: Some((animation("vague"), reglages(vitesse))),
    }
}

/// Toutes les valeurs écrites après `vitesse=` dans les lignes d'entrée d'un texte, dans l'ordre.
///
/// Le découpage suit les jetons séparés par des blancs : c'est la forme `cle=valeur` du protocole
/// et des fichiers (`animation vague couleur=… vitesse=… direction=…`, `spec_eclairage.rs`). Les
/// commentaires sont écartés, comme le fait `spec_profils.rs` : un en-tête qui documente le
/// format peut citer un exemple de vitesse sans qu'il soit un réglage.
fn vitesses_ecrites(texte: &str) -> Vec<String> {
    texte
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with('#'))
        .flat_map(str::split_whitespace)
        .filter_map(|jeton| jeton.strip_prefix("vitesse="))
        .map(str::to_owned)
        .collect()
}

/// Un dossier de travail sous `std::env::temp_dir()`, effacé à la fin du test par `Drop` — celui
/// de `spec_profils.rs`.
struct DossierJetable {
    chemin: PathBuf,
}

impl DossierJetable {
    fn neuf(nom: &str) -> DossierJetable {
        let chemin =
            std::env::temp_dir().join(format!("reverb-spec-149-{}-{nom}", std::process::id()));
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
fn une_vitesse_au_dixieme_traverse_eclairage_conf_ecrite_comme_on_l_a_tapee() {
    // Critère d'acceptation n° 4, côté `eclairage.conf` : la vitesse posée est celle qui revient au
    // redémarrage, et le fichier l'écrit `0.7` ou `3` — jamais `0.70`, `3.0` ni `0.699999988`,
    // l'écriture par défaut d'un `f32` élargi.
    let dossier = DossierJetable::neuf("eclairage");
    for (vitesse, attendue) in VITESSES {
        let pose = eclairage(vitesse);

        let texte = pose.encoder();
        assert_eq!(
            vitesses_ecrites(&texte),
            vec![attendue.to_owned()],
            "la vitesse {vitesse} s'écrit « vitesse={attendue} » dans eclairage.conf :\n{texte}"
        );
        let relu = Eclairage::decoder(&texte).unwrap_or_else(|erreur| {
            panic!("eclairage.conf doit relire ce qu'il écrit ({erreur}) :\n{texte}")
        });
        assert_eq!(
            relu, pose,
            "la vitesse {vitesse} traverse l'aller-retour par le texte"
        );

        // Le redémarrage n'a qu'un mécanisme : le disque.
        let chemin = dossier.chemin().join("eclairage.conf");
        enregistrer_eclairage(&chemin, &pose)
            .unwrap_or_else(|erreur| panic!("écriture de {} : {erreur}", chemin.display()));
        let (charge, signalement) = charger_eclairage(&chemin);
        assert_eq!(
            signalement, None,
            "un fichier écrit par le démon se relit sans un mot — vitesse {vitesse}"
        );
        assert_eq!(charge, pose, "la vitesse {vitesse} survit au redémarrage");
    }
}

// ---------------------------------------------------------------------------
// Profils (et zones, qu'un profil emporte)
// ---------------------------------------------------------------------------

/// Un profil dont la couche globale tourne à `globale` et une zone animée à `de_zone`.
///
/// Les deux vitesses sont **différentes** : un profil qui recopierait l'une dans l'autre ne
/// passerait pas.
fn profil(globale: f32, de_zone: f32) -> Profil {
    let mut zones = Zones::vide();
    zones.poser("barre", &leds("slot:1"));
    zones.animer(
        "barre",
        Some((
            animation("comete"),
            Reglages {
                couleur: Rgb::new(0x12, 0x34, 0x56),
                vitesse: de_zone,
                direction: Direction::ArriereAvant,
                sonde: None,
                palette: None,
                largeur: 4,
            },
        )),
    );
    Profil {
        eclairage: eclairage(globale),
        zones,
        ecran: None,
    }
}

#[test]
fn une_vitesse_au_dixieme_traverse_un_profil_et_ses_zones_ecrite_comme_on_l_a_tapee() {
    // Critère d'acceptation n° 4, côté profils : « `0.7` → `0.7`, `3` → `3`, profil compris ». La
    // zone animée y est, parce que l'issue promet aussi qu'« une zone » écrite avant reste lisible :
    // la zone et la couche globale passent par deux écritures du même réglage.
    let dossier = DossierJetable::neuf("profils");
    let nom = NomProfil::nouveau("vitesses").expect("« vitesses » est un nom de profil");
    for (rang, (globale, ecrite_globale)) in VITESSES.iter().enumerate() {
        // La zone prend la vitesse suivante de la liste : toujours différente de la globale.
        let (de_zone, ecrite_de_zone) = VITESSES[(rang + 1) % VITESSES.len()];
        let pose = profil(*globale, de_zone);

        let texte = pose.encoder();
        let mut ecrites = vitesses_ecrites(&texte);
        ecrites.sort();
        let mut attendues = vec![(*ecrite_globale).to_owned(), ecrite_de_zone.to_owned()];
        attendues.sort();
        assert_eq!(
            ecrites, attendues,
            "le profil écrit « vitesse={ecrite_globale} » pour la couche globale et \
             « vitesse={ecrite_de_zone} » pour la zone :\n{texte}"
        );
        let relu = Profil::decoder(&texte).unwrap_or_else(|erreur| {
            panic!("un profil doit relire ce qu'il écrit ({erreur}) :\n{texte}")
        });
        assert_eq!(
            relu, pose,
            "les vitesses {globale} et {de_zone} traversent l'aller-retour par le texte"
        );

        let texte_des_zones = pose.zones.encoder();
        assert_eq!(
            vitesses_ecrites(&texte_des_zones),
            vec![ecrite_de_zone.to_owned()],
            "zones.conf écrit « vitesse={ecrite_de_zone} » :\n{texte_des_zones}"
        );
        assert_eq!(
            Zones::decoder(&texte_des_zones).unwrap_or_else(|erreur| {
                panic!("zones.conf doit relire ce qu'il écrit ({erreur}) :\n{texte_des_zones}")
            }),
            pose.zones,
            "la vitesse {de_zone} d'une zone traverse zones.conf"
        );

        enregistrer(dossier.chemin(), &nom, &pose).expect("l'enregistrement réussit");
        let charge = charger(dossier.chemin(), &nom).expect("le profil enregistré se recharge");
        assert_eq!(
            charge, pose,
            "les vitesses {globale} et {de_zone} survivent au disque"
        );
    }
}
