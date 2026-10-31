//! Tests d'intention de la palette `red-flash` côté profils (issue #146).
//!
//! Écrits **depuis l'issue #146 seule**, avant l'implémentation, avec les mécanismes de
//! `spec_profils.rs` : `Profil::encoder` / `Profil::decoder` pour l'aller-retour en mémoire,
//! `enregistrer` / `charger` dans un dossier jetable pour l'aller-retour par le disque. Rien de
//! `crates/*/src/` n'a été lu hors les signatures publiques.
//!
//! Critère d'acceptation : « un profil enregistré avec `red-flash` se recharge à l'identique ».
//!
//! ⚠️ **La couleur des témoins reste au défaut.** `couleur` et `palette` s'excluent, et sous
//! palette `couleur` n'est pas écrite (arbitrage n° 2 de `spec_126_palettes.rs`) : une couleur
//! autre que le défaut ne pourrait pas revenir, et l'aller-retour échouerait pour une raison
//! étrangère à cette issue. Vitesse et direction, elles, quittent leur défaut — sans quoi un
//! encodage qui les perdrait passerait inaperçu.

use std::fs;
use std::path::{Path, PathBuf};

use reverb_anim::{Animation, Direction, Palette, Reglages};
use reverb_daemon::ecran::{Affichage, Etat as EtatEcran};
use reverb_daemon::persistance::Eclairage;
use reverb_daemon::profils::{Profil, charger, enregistrer};
use reverb_daemon::zones::Zones;
use reverb_proto::ram::SLOT_COUNT;
use reverb_proto::{Led, NomProfil, Rgb};

/// Le nom de la palette que l'issue ajoute.
const RED_FLASH: &str = "red-flash";

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| panic!("« {nom} » est au catalogue : {erreur}"))
}

fn red_flash() -> Palette {
    Palette::par_nom(RED_FLASH)
        .unwrap_or_else(|erreur| panic!("« {RED_FLASH} » est une palette : {erreur}"))
}

fn leds(slug: &str) -> Vec<Led> {
    Led::depuis_slug(slug).unwrap_or_else(|erreur| panic!("« {slug} » désigne des LED : {erreur}"))
}

fn nom(saisi: &str) -> NomProfil {
    NomProfil::nouveau(saisi).unwrap_or_else(|erreur| panic!("« {saisi} » est un nom : {erreur}"))
}

/// Des réglages sous `red-flash`, la couleur au défaut — voir l'en-tête.
fn sous_red_flash(vitesse: u8, direction: Direction) -> Reglages {
    Reglages {
        vitesse,
        direction,
        palette: Some(red_flash()),
        ..Reglages::default()
    }
}

/// Un profil où `red-flash` est posée deux fois : sur la couche globale, et sur une zone. Deux
/// endroits, parce que ce sont deux encodeurs — `eclairage` et `zones` — et qu'un seul des deux
/// pourrait perdre le nom composé.
fn profil_sous_red_flash() -> Profil {
    let mut ventilateurs = [Rgb::BLACK; 10];
    for (index, couleur) in ventilateurs.iter_mut().enumerate() {
        let graine = u8::try_from(index).expect("dix ventilateurs tiennent dans un u8");
        *couleur = Rgb::new(0x11 + graine, 0x82, 0xf0 - graine);
    }
    let mut zones = Zones::vide();
    zones.poser("barre", &leds("slot:1"));
    zones.animer(
        "barre",
        Some((
            animation("comete"),
            sous_red_flash(9, Direction::ArriereAvant),
        )),
    );
    Profil {
        eclairage: Eclairage {
            ventilateurs,
            barrettes: [Rgb::new(0xa4, 0x37, 0x05); SLOT_COUNT],
            animation: Some((animation("vague"), sous_red_flash(7, Direction::HautBas))),
        },
        zones,
        ecran: Some(EtatEcran {
            luminosite: 37,
            affichage: Affichage::Rien,
        }),
    }
}

/// Un dossier de travail sous `std::env::temp_dir()`, effacé à la fin du test — même garde-fou
/// que `spec_profils.rs`, sans dépendance.
struct DossierJetable {
    chemin: PathBuf,
}

impl DossierJetable {
    fn neuf(nom: &str) -> DossierJetable {
        let chemin =
            std::env::temp_dir().join(format!("reverb-spec-146-{}-{nom}", std::process::id()));
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

#[test]
fn un_profil_sous_red_flash_traverse_l_encodage_a_l_identique() {
    // Test d'intention de l'issue — « Profil : aller-retour avec `red-flash` » —, en mémoire.
    let pose = profil_sous_red_flash();
    let texte = pose.encoder();
    assert!(
        texte.contains(RED_FLASH),
        "le profil encodé doit nommer « {RED_FLASH} » :\n{texte}"
    );
    let relu = Profil::decoder(&texte).unwrap_or_else(|erreur| {
        panic!("un profil sous « {RED_FLASH} » doit se relire : {erreur}\n{texte}")
    });
    assert_eq!(
        relu, pose,
        "le profil sous « {RED_FLASH} » doit revenir à l'identique :\n{texte}"
    );
}

#[test]
fn un_profil_enregistre_avec_red_flash_se_recharge_a_l_identique() {
    // Critère d'acceptation : « un profil enregistré avec `red-flash` se recharge à l'identique ».
    // Le redémarrage n'a qu'un mécanisme, le disque : c'est lui qu'on traverse.
    let dossier = DossierJetable::neuf("red-flash-disque");
    let rouge = nom("rouge");
    let pose = profil_sous_red_flash();

    enregistrer(dossier.chemin(), &rouge, &pose).expect("l'enregistrement réussit");
    let relu = charger(dossier.chemin(), &rouge).expect("le profil enregistré se recharge");
    assert_eq!(
        relu, pose,
        "un profil enregistré sous « {RED_FLASH} » doit se recharger à l'identique"
    );
}
