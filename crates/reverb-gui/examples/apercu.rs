//! Dessine la fenêtre **sans écran**, dans un fichier.
//!
//! Slint sait rendre en logiciel, dans un tampon de pixels, sans compositeur ni
//! carte graphique. C'est ce qui permet de regarder la maquette sans ouvrir de
//! fenêtre sur un bureau — pratique quand on travaille par-dessus l'épaule de la
//! machine, et seule façon de vérifier la mise en page ailleurs qu'à l'œil nu.
//!
//! ```bash
//! cargo run --release --example apercu -p reverb-gui -- /tmp/reverb.ppm
//! cargo run --release --example apercu -p reverb-gui -- /tmp/reverb.ppm /run/reverb/reverbd.sock
//! ```
//!
//! Avec un socket, il montre **le vrai boîtier** : il s'abonne, prend la
//! première image que le démon pousse, et la dessine. C'est la chaîne complète
//! — `watch`, décodage, projection, rendu — vérifiée d'un seul coup.
//!
//! Le format est du **PPM binaire** (P6), que tout visualiseur ouvre et que
//! `magick reverb.ppm reverb.png` convertit — écrire un PNG demanderait un
//! encodeur, donc une dépendance, pour un outil de diagnostic.

use std::rc::Rc;

use reverb_anim::{CATALOGUE, Geometrie};
use reverb_gui::plan::Plan;
use reverb_gui::{
    FamilleAnimation, Fenetre, LigneTemperature, LigneVentilateur, LigneZone, PointLed,
};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};
use slint::{Color, ComponentHandle, ModelRc, PhysicalSize, SharedString, VecModel};

const LARGEUR: u32 = 1180;
const HAUTEUR: u32 = 760;

/// Une plateforme Slint qui n'a pas d'écran, et n'en cherche pas.
struct SansEcran {
    fenetre: Rc<MinimalSoftwareWindow>,
}

impl Platform for SansEcran {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.fenetre.clone())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sortie = std::env::args().nth(1).unwrap_or_else(|| {
        std::env::temp_dir()
            .join("reverb-apercu.ppm")
            .display()
            .to_string()
    });

    let fenetre = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(SansEcran {
        fenetre: fenetre.clone(),
    }))?;
    fenetre.set_size(PhysicalSize::new(LARGEUR, HAUTEUR));

    let interface = Fenetre::new()?;
    garnir(&interface, std::env::args().nth(2));
    interface.show()?;

    // Une passe suffit : rien n'anime, tout est posé avant le rendu.
    slint::platform::update_timers_and_animations();
    let mut pixels = vec![PremultipliedRgbaColor::default(); (LARGEUR * HAUTEUR) as usize];
    fenetre.draw_if_needed(|rendu| {
        rendu.render(&mut pixels, LARGEUR as usize);
    });

    let mut ppm = format!("P6\n{LARGEUR} {HAUTEUR}\n255\n").into_bytes();
    for pixel in &pixels {
        ppm.extend_from_slice(&[pixel.red, pixel.green, pixel.blue]);
    }
    std::fs::write(&sortie, ppm)?;
    println!("{sortie}");
    Ok(())
}

/// Remplit la fenêtre d'un boîtier plausible : chaque LED sa teinte.
///
/// Pas de socket, pas de démon : cet aperçu montre la **mise en page**, et un
/// dégradé y rend chaque LED discernable de ses voisines — ce qu'une couleur
/// unie cacherait.
fn garnir(interface: &Fenetre, socket: Option<String>) {
    // `REVERB_VUE=iso` dessine la vue de trois-quarts. C'est le seul moyen de
    // la regarder sans ouvrir de session graphique.
    let geometrie = Geometrie::mesuree();
    let plan = if std::env::var("REVERB_VUE").as_deref() == Ok("iso") {
        Plan::isometrique(&geometrie)
    } else {
        Plan::nouveau(&geometrie)
    };
    let mut points = Vec::new();

    // Avec un socket, les couleurs viennent du démon ; sans, d'un dégradé.
    let vraies = socket.and_then(|chemin| premiere_image(&chemin));

    for (rang, position) in Position::ALL.into_iter().enumerate() {
        for led in 0..LEDS_PER_FAN as usize {
            let place = plan.led_ventilateur(position, led).expect("huit LED");
            points.push(PointLed {
                x: place.x,
                y: place.y,
                rayon: plan.rayon_anneau() / 8.0
                    * if position == Position::Arriere {
                        1.7
                    } else {
                        1.0
                    },
                hauteur: 0.0,
                couleur: vraies.as_ref().map_or_else(
                    || teinte(rang as f32 / 10.0 + led as f32 / 8.0 / 3.0),
                    |image| couleur_de(image, &format!("fan:{}", position.slug()), led),
                ),
                choisie: position == Position::Arriere,
            });
        }
    }
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            let place = plan.led_barrette(slot, led).expect("onze LED");
            points.push(PointLed {
                x: place.x,
                y: place.y,
                rayon: plan.rayon_anneau() / 8.0,
                hauteur: 0.0,
                couleur: vraies.as_ref().map_or_else(
                    || teinte(0.55 + slot as f32 / 20.0 + led as f32 / 11.0 / 6.0),
                    |image| couleur_de(image, &format!("slot:{slot}"), led),
                ),
                choisie: false,
            });
        }
    }
    // `REVERB_DETAIL=ventilo` montre les quatorze organes au lieu des cent
    // vingt-quatre LED : la seconde mise en page que la fenêtre sait dessiner.
    if std::env::var("REVERB_DETAIL").as_deref() == Ok("ventilo") {
        points.clear();
        for (rang, position) in Position::ALL.into_iter().enumerate() {
            let centre = plan.centre_ventilateur(position);
            points.push(PointLed {
                x: centre.x,
                y: centre.y,
                rayon: plan.rayon_anneau(),
                hauteur: 0.0,
                couleur: teinte(rang as f32 / 10.0),
                choisie: position == Position::Arriere,
            });
        }
        for slot in 0..SLOT_COUNT {
            let centre = plan.centre_barrette(slot).expect("quatre barrettes");
            let bas = plan.led_barrette(slot, 0).expect("onze LED");
            let haut = plan
                .led_barrette(slot, LEDS_PER_STICK - 1)
                .expect("onze LED");
            points.push(PointLed {
                x: centre.x,
                y: centre.y,
                rayon: plan.rayon_anneau() / 3.0,
                hauteur: (haut.y - bas.y).abs(),
                couleur: teinte(0.55 + slot as f32 / 20.0),
                choisie: false,
            });
        }
    }

    interface.set_leds(ModelRc::new(VecModel::from(points)));
    interface.set_aretes(SharedString::from(
        plan.aretes()
            .iter()
            .map(|(debut, fin)| format!("M {} {} L {} {} ", debut.x, debut.y, fin.x, fin.y))
            .collect::<String>(),
    ));

    interface.set_familles(ModelRc::new(VecModel::from(
        CATALOGUE
            .iter()
            .map(|nom| FamilleAnimation {
                nom: SharedString::from(*nom),
                effet: SharedString::from(
                    "Une phrase qui décrit l'effet, sur deux lignes le cas échéant.",
                ),
                accepte_couleur: true,
            })
            .collect::<Vec<FamilleAnimation>>(),
    )));
    interface.set_animations(ModelRc::new(VecModel::from(
        std::iter::once("aucune")
            .chain(CATALOGUE.iter().copied())
            .map(SharedString::from)
            .collect::<Vec<SharedString>>(),
    )));
    // « comete » : rang 1 dans le catalogue, donc 2 dans un menu qui commence
    // par « aucune ».
    interface.set_animation_choisie(2);
    interface.set_affichage_choisi(1);
    interface.set_argument_ecran(SharedString::from("kraken2023elite:coolant"));
    interface.set_affichage_ecran(SharedString::from("gauge:kraken2023elite:coolant"));
    interface.set_luminosite_ecran(60);
    interface.set_ventilateurs(ModelRc::new(VecModel::from(vec![
        LigneVentilateur {
            canal: SharedString::from("nzxtsmart2:fan-1"),
            position: SharedString::from("radiateur haut"),
            rpm: SharedString::from("1180"),
            pwm: 60,
            mode: SharedString::from("courbe firmware"),
            lisible: true,
        },
        LigneVentilateur {
            canal: SharedString::from("kraken2023elite:pump"),
            position: SharedString::new(),
            rpm: SharedString::from("2400"),
            pwm: 75,
            mode: SharedString::from("courbe de l'hote"),
            lisible: true,
        },
    ])));
    // Les cinq sondes retenues, avec une courbe dessinée à la main : de quoi
    // regarder la carte sans machine ni démon. Les libellés sont ceux que la
    // fenêtre produit vraiment, modèles de disques compris (issue #51).
    interface.set_temperatures(ModelRc::new(VecModel::from(
        [
            ("CPU", "61.8 °C", 0.30, true),
            ("Liquide", "34.2 °C", 0.55, true),
            ("GPU", "51.0 °C", 0.70, true),
            ("NVMe CT2000T705SSD5", "36.9 °C", 0.20, true),
            ("NVMe CT4000P3SSD8", "illisible", 0.0, false),
        ]
        .into_iter()
        .map(|(libelle, valeur, base, lisible)| LigneTemperature {
            libelle: SharedString::from(libelle),
            valeur: SharedString::from(valeur),
            courbe: SharedString::from(if lisible {
                (0..40_i32)
                    .map(|rang| {
                        let x = rang as f32 / 39.0;
                        let y = 0.5 + (x * 9.0 + base * 6.0).sin() * 0.35 * (0.4 + base);
                        format!("{} {x:.3} {y:.3} ", if rang == 0 { "M" } else { "L" })
                    })
                    .collect::<String>()
            } else {
                String::new()
            }),
            lisible,
        })
        .collect::<Vec<LigneTemperature>>(),
    )));
    interface.set_zones(ModelRc::new(VecModel::from(vec![
        LigneZone {
            nom: SharedString::from("radiateur"),
            rendu: SharedString::from("braise"),
            combien: 24,
            visee: true,
        },
        LigneZone {
            nom: SharedString::from("ram"),
            rendu: SharedString::from("#00aeed"),
            combien: 44,
            visee: false,
        },
    ])));
    interface.set_cible(SharedString::from("la zone « radiateur »"));
    interface.set_animation_courante(SharedString::from("comete"));
    interface.set_message(SharedString::from(if vraies.is_some() {
        "aperçu de la vraie image, prise sur le socket"
    } else {
        "aperçu hors ligne — aucun démon interrogé"
    }));
    interface.set_connecte(true);
}

/// La première image que le démon pousse, s'il y en a un.
fn premiere_image(chemin: &str) -> Option<Vec<(String, Vec<reverb_proto::Rgb>)>> {
    let mut abonnement = reverb_gui::client::Abonnement::ouvrir(std::path::Path::new(chemin))
        .map_err(|erreur| eprintln!("attention : {chemin} : {erreur}"))
        .ok()?;
    let image = abonnement.image_suivante()?;
    Some(
        image
            .into_iter()
            .filter_map(|ligne| match ligne {
                reverb_proto::ipc::ResponseLine::Frame { cible, couleurs } => {
                    Some((cible, couleurs))
                }
                _ => None,
            })
            .collect(),
    )
}

/// La couleur d'une LED dans une image reçue, noire si la cible manque.
fn couleur_de(image: &[(String, Vec<reverb_proto::Rgb>)], cible: &str, led: usize) -> Color {
    image
        .iter()
        .find(|(nom, _)| nom == cible)
        .and_then(|(_, couleurs)| couleurs.get(led))
        .map_or(Color::from_rgb_u8(0, 0, 0), |couleur| {
            Color::from_rgb_u8(couleur.r, couleur.g, couleur.b)
        })
}

/// Une teinte de l'arc-en-ciel, pour rendre chaque LED discernable.
fn teinte(tour: f32) -> Color {
    let h = (tour.fract() + 1.0).fract() * 6.0;
    let secteur = h as u32 % 6;
    let f = h - h.floor();
    let (r, v, b) = match secteur {
        0 => (1.0, f, 0.0),
        1 => (1.0 - f, 1.0, 0.0),
        2 => (0.0, 1.0, f),
        3 => (0.0, 1.0 - f, 1.0),
        4 => (f, 0.0, 1.0),
        _ => (1.0, 0.0, 1.0 - f),
    };
    Color::from_rgb_u8((r * 255.0) as u8, (v * 255.0) as u8, (b * 255.0) as u8)
}
