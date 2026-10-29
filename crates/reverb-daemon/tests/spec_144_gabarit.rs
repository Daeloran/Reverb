//! Tests d'intention du mode gabarit SVG de l'écran — issue #144, côté **état et rendu**.
//!
//! Écrits avant l'implémentation depuis l'issue #144 seule ; si un test échoue après
//! implémentation, c'est le code qu'on corrige. Rien de `crates/*/src/` n'a été lu pour les
//! produire, hors les signatures publiques existantes de `reverb_daemon::ecran` (`Affichage`,
//! `Etat`, `Dalle::octets`, `charger`, `enregistrer`, `verifier_fichier`) et le contrat neuf que
//! l'issue nomme : `Affichage::Gabarit(String)`, le module `reverb_daemon::gabarit` avec
//! `sondes_du_gabarit`, `rendre_gabarit` et `GabaritInvalide`.
//!
//! Le pendant côté protocole — `screen template <chemin>` — vit dans
//! `crates/reverb-proto/tests/spec_144_gabarit.rs`.
//!
//! ## Ce que ces tests gardent
//!
//! 1. **L'ordre des composantes est muet.** La dalle est en [`screen::COMPONENT_ORDER`] ; une
//!    composante permutée s'affiche nette et fausse. Ces tests ne supposent jamais quel octet porte
//!    le rouge : un gris neutre rend les trois octets égaux, une couleur pure se vérifie sur le
//!    multiensemble trié de ses trois octets.
//! 2. **Une sonde muette qui affiche « 0 » est rassurante et fausse.** Le rendu d'une sonde muette
//!    doit être celui du texte littéral `--`, octet pour octet.
//! 3. **Un rendu non déterministe pousse 1,2 Mo pour rien** : le démon n'envoie que si le tampon a
//!    changé.
//!
//! ## Points que l'issue laisse ouverts, et que ces tests tranchent
//!
//! 1. **Le slug d'un marqueur va jusqu'à `}}`, deux-points compris** : les slugs réels s'écrivent
//!    `k10temp:tctl` ou `kraken2023elite:coolant-temp`.
//! 2. **« Arrondie à l'entier » se vérifie par équivalence de rendu** : 34,4 se rend comme le texte
//!    littéral `34`, 34,6 comme `35`. Les demi-entiers ne sont pas tranchés.
//! 3. **Une police inconnue tombe sur le repli embarqué** : un `<text>` dont la `font-family`
//!    n'existe nulle part, sans dossier de polices, doit quand même dessiner quelque chose.
//! 4. **Les polices du dossier du gabarit ne sont pas testées ici** : distinguer une police du
//!    dossier du repli demanderait une fonte tierce vendue dans le dépôt, que l'issue exclut.

use std::fs;
use std::path::{Path, PathBuf};

use reverb_daemon::ecran::{self, Affichage, Dalle, Etat};
use reverb_daemon::gabarit::{GabaritInvalide, rendre_gabarit, sondes_du_gabarit};
use reverb_proto::screen;

const COTE: usize = 640;

// ---------------------------------------------------------------------------
// Gabarits témoins
// ---------------------------------------------------------------------------

/// Un texte en très grande taille, centré, dans une police qui n'existe nulle part : le rendu
/// passe donc par la police de repli du démon. `contenu` est soit un marqueur, soit un littéral.
fn gabarit_texte(contenu: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="640" viewBox="0 0 640 640">
  <rect x="0" y="0" width="640" height="640" fill="#000000"/>
  <text x="320" y="400" font-family="PoliceQuiNExistePas" font-size="240" fill="#ffffff" text-anchor="middle">{contenu}°</text>
</svg>"##
    )
}

const SONDE: &str = "k10temp:tctl";

fn marqueur() -> String {
    format!("{{{{sonde:{SONDE}}}}}")
}

/// Rend un gabarit sans dossier de polices, ou échoue en disant pourquoi.
fn rendre(svg: &str, valeur: &dyn Fn(&str) -> Option<f32>) -> Dalle {
    match rendre_gabarit(svg, None, valeur) {
        Ok(dalle) => dalle,
        Err(erreur) => {
            let erreur: GabaritInvalide = erreur;
            panic!("le gabarit devait se rendre : {erreur:?}\n{svg}")
        }
    }
}

/// La sonde témoin vaut `v`, toute autre est muette.
fn sonde_a(v: f32) -> impl Fn(&str) -> Option<f32> {
    move |slug: &str| (slug == SONDE).then_some(v)
}

fn muette(_: &str) -> Option<f32> {
    None
}

fn pixel(dalle: &Dalle, x: usize, y: usize) -> &[u8] {
    let debut = (y * COTE + x) * screen::PIXEL_LEN;
    &dalle.octets()[debut..debut + screen::PIXEL_LEN]
}

// ---------------------------------------------------------------------------
// Dossier temporaire
// ---------------------------------------------------------------------------

/// Un dossier de travail sous `std::env::temp_dir()`, effacé à la fin du test, même en échec.
struct DossierJetable {
    chemin: PathBuf,
}

impl DossierJetable {
    fn neuf(nom: &str) -> DossierJetable {
        let chemin =
            std::env::temp_dir().join(format!("reverb-spec-144-{}-{nom}", std::process::id()));
        let _ = fs::remove_dir_all(&chemin);
        fs::create_dir_all(&chemin)
            .unwrap_or_else(|erreur| panic!("dossier de test {} : {erreur}", chemin.display()));
        DossierJetable { chemin }
    }

    fn fichier(&self, nom: &str) -> PathBuf {
        self.chemin.join(nom)
    }
}

impl Drop for DossierJetable {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.chemin);
    }
}

fn chemin_texte(chemin: &Path) -> String {
    chemin.to_str().expect("chemin de test en UTF-8").to_owned()
}

// ---------------------------------------------------------------------------
// État d'écran
// ---------------------------------------------------------------------------

#[test]
fn un_etat_gabarit_fait_l_aller_retour_par_le_fichier_d_ecran() {
    let dossier = DossierJetable::neuf("aller-retour");
    for chemin in [
        "/home/nico/gabarits/ecran.svg",
        "/home/nico/Mes documents/écran sobre.svg",
    ] {
        let etat = Etat {
            luminosite: 60,
            affichage: Affichage::Gabarit(chemin.to_owned()),
        };
        let fichier = dossier.fichier("ecran.conf");
        ecran::enregistrer(&fichier, &etat)
            .unwrap_or_else(|erreur| panic!("enregistrer {etat:?} : {erreur}"));
        let (relu, anomalie) = ecran::charger(&fichier);
        assert_eq!(
            anomalie, None,
            "le fichier écrit par le démon doit se relire sans anomalie"
        );
        assert_eq!(
            relu, etat,
            "le gabarit doit survivre à un redémarrage du démon"
        );
    }
}

// ---------------------------------------------------------------------------
// Sondes d'un gabarit
// ---------------------------------------------------------------------------

#[test]
fn les_sondes_d_un_gabarit_sont_listees_sans_doublon_dans_l_ordre_d_apparition() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640">
  <text x="10" y="100">LIQ {{sonde:kraken2023elite:coolant-temp}}°</text>
  <text x="10" y="200">CPU {{sonde:k10temp:tctl}}° · LIQ {{sonde:kraken2023elite:coolant-temp}}°</text>
  <text x="10" y="300">GPU {{sonde:nvidia:gpu}}° CPU {{sonde:k10temp:tctl}}°</text>
</svg>"##;
    assert_eq!(
        sondes_du_gabarit(svg),
        vec![
            "kraken2023elite:coolant-temp".to_owned(),
            "k10temp:tctl".to_owned(),
            "nvidia:gpu".to_owned(),
        ]
    );
}

#[test]
fn un_gabarit_sans_marqueur_ne_lit_aucune_sonde() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640">
  <text x="10" y="100">SHYNAEL</text>
</svg>"##;
    assert!(sondes_du_gabarit(svg).is_empty());
}

// ---------------------------------------------------------------------------
// Rendu
// ---------------------------------------------------------------------------

#[test]
fn le_rendu_fait_640_par_640_dans_le_format_de_la_dalle() {
    let dalle = rendre(&gabarit_texte(&marqueur()), &sonde_a(34.0));
    assert_eq!(dalle.octets().len(), COTE * COTE * 3);
    assert_eq!(dalle.octets().len(), screen::IMAGE_LEN);
    assert!(screen::check_image(dalle.octets()).is_ok());
}

#[test]
fn deux_rendus_sur_les_memes_valeurs_sont_egaux_octet_pour_octet() {
    let svg = gabarit_texte(&marqueur());
    let premier = rendre(&svg, &sonde_a(42.0));
    let second = rendre(&svg, &sonde_a(42.0));
    assert!(
        premier.octets() == second.octets(),
        "un tampon qui change à valeur égale ferait pousser 1,2 Mo toutes les deux secondes"
    );
}

#[test]
fn le_texte_est_dessine_avec_la_police_de_repli() {
    let dalle = rendre(&gabarit_texte(&marqueur()), &sonde_a(42.0));
    assert!(
        dalle.octets().iter().any(|&o| o > 128),
        "une police inconnue doit tomber sur le repli embarqué, pas laisser la dalle noire"
    );
}

#[test]
fn deux_valeurs_differentes_donnent_des_tampons_differents() {
    let svg = gabarit_texte(&marqueur());
    let a = rendre(&svg, &sonde_a(34.0));
    let b = rendre(&svg, &sonde_a(71.0));
    assert!(
        a.octets() != b.octets(),
        "34 et 71 ne peuvent pas se dessiner pareil"
    );
}

#[test]
fn la_valeur_est_arrondie_a_l_entier_sans_unite() {
    let svg = gabarit_texte(&marqueur());
    let litteral_34 = rendre(&gabarit_texte("34"), &muette);
    let litteral_35 = rendre(&gabarit_texte("35"), &muette);
    assert!(
        rendre(&svg, &sonde_a(34.4)).octets() == litteral_34.octets(),
        "34,4 doit s'écrire « 34 », exactement comme le texte littéral"
    );
    assert!(
        rendre(&svg, &sonde_a(34.6)).octets() == litteral_35.octets(),
        "34,6 doit s'écrire « 35 », exactement comme le texte littéral"
    );
}

#[test]
fn une_sonde_muette_s_ecrit_en_tirets_jamais_en_zero() {
    let svg = gabarit_texte(&marqueur());
    let muet = rendre(&svg, &muette);
    let tirets = rendre(&gabarit_texte("--"), &muette);
    let zero = rendre(&svg, &sonde_a(0.0));
    assert!(
        muet.octets() == tirets.octets(),
        "une sonde muette doit se rendre exactement comme le texte littéral `--`"
    );
    assert!(
        muet.octets() != zero.octets(),
        "une sonde muette ne doit pas se rendre comme la valeur 0"
    );
}

#[test]
fn une_sonde_inconnue_s_ecrit_aussi_en_tirets() {
    // La sonde témoin a une valeur, mais le gabarit en nomme une autre.
    let svg = gabarit_texte("{{sonde:inconnue:nulle-part}}");
    let rendu = rendre(&svg, &sonde_a(55.0));
    let tirets = rendre(&gabarit_texte("--"), &muette);
    assert!(rendu.octets() == tirets.octets());
}

#[test]
fn un_rectangle_gris_couvrant_le_viewbox_donne_une_dalle_uniforme_de_ce_gris() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="0" y="0" width="100" height="100" fill="#808080"/>
</svg>"##;
    let dalle = rendre(svg, &muette);
    assert!(
        dalle.octets().iter().all(|&o| o == 0x80),
        "un gris neutre plein doit donner 0x80 partout, quel que soit l'ordre des composantes"
    );
}

#[test]
fn un_rectangle_rouge_couvrant_le_viewbox_donne_une_dalle_uniforme_de_ce_rouge() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="0" y="0" width="100" height="100" fill="#ff0000"/>
</svg>"##;
    let dalle = rendre(svg, &muette);
    let premier = pixel(&dalle, 0, 0).to_vec();
    let mut trie = premier.clone();
    trie.sort_unstable();
    assert_eq!(
        trie,
        vec![0, 0, 255],
        "un rouge pur : une composante à fond, deux à zéro"
    );
    assert!(
        dalle
            .octets()
            .chunks(screen::PIXEL_LEN)
            .all(|p| p == premier.as_slice()),
        "la dalle doit être uniforme"
    );
    // Et l'octet à fond est bien celui que l'ordre de la dalle attribue au rouge.
    let rouge = screen::composantes(&premier);
    assert_eq!(
        rouge,
        (255, 0, 0),
        "l'ordre des composantes est celui de la dalle"
    );
}

#[test]
fn un_viewbox_deux_pour_un_est_centre_sur_du_noir_sans_deformation() {
    // 200×100 rempli de blanc : mis à l'échelle sans déformation, il occupe 640×320, centré,
    // soit des bandes noires de 160 lignes en haut et en bas. Deux lignes de marge pour l'anticrénelage.
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100">
  <rect x="0" y="0" width="200" height="100" fill="#ffffff"/>
</svg>"##;
    let dalle = rendre(svg, &muette);
    let ligne = |y: usize| -> Vec<u8> {
        (0..COTE)
            .flat_map(|x| pixel(&dalle, x, y).to_vec())
            .collect()
    };
    for y in (0..158).chain(482..COTE) {
        assert!(
            ligne(y).iter().all(|&o| o == 0),
            "la ligne {y} doit être une bande noire"
        );
    }
    for y in 162..478 {
        assert!(
            ligne(y).iter().all(|&o| o == 255),
            "la ligne {y} doit être blanche d'un bord à l'autre"
        );
    }
}

// ---------------------------------------------------------------------------
// Refus
// ---------------------------------------------------------------------------

#[test]
fn un_svg_invalide_est_refuse_par_le_rendu() {
    for svg in [
        "",
        "ceci n'est pas un SVG",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 640 640\"><rect",
        "\u{89}PNG\r\n\u{1a}\n",
    ] {
        assert!(
            rendre_gabarit(svg, None, &muette).is_err(),
            "« {svg} » doit être refusé avec une erreur"
        );
    }
}

#[test]
fn verifier_fichier_refuse_un_gabarit_absent_ou_invalide_et_accepte_un_svg_valide() {
    let dossier = DossierJetable::neuf("verifier");

    let absent = dossier.fichier("absent.svg");
    assert!(
        ecran::verifier_fichier(&Affichage::Gabarit(chemin_texte(&absent))).is_err(),
        "un gabarit absent doit être refusé"
    );

    let invalide = dossier.fichier("invalide.svg");
    fs::write(&invalide, "ceci n'est pas un SVG").expect("écriture du gabarit invalide");
    assert!(
        ecran::verifier_fichier(&Affichage::Gabarit(chemin_texte(&invalide))).is_err(),
        "un gabarit illisible doit être refusé sans changer l'affichage en cours"
    );

    let valide = dossier.fichier("ecran.svg");
    fs::write(&valide, gabarit_texte(&marqueur())).expect("écriture du gabarit valide");
    assert_eq!(
        ecran::verifier_fichier(&Affichage::Gabarit(chemin_texte(&valide))),
        Ok(()),
        "un SVG valide doit être accepté"
    );
}
