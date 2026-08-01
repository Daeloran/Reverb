//! Dessine la fenêtre **sans écran**, dans un fichier.
//!
//! Slint sait rendre en logiciel, dans un tampon de pixels, sans compositeur ni
//! carte graphique. C'est ce qui permet de regarder la maquette sans ouvrir de
//! fenêtre sur un bureau — pratique quand on travaille par-dessus l'épaule de la
//! machine, et seule façon de vérifier la mise en page ailleurs qu'à l'œil nu.
//!
//! ```bash
//! cargo run --release --example apercu -p reverb-gui -- /tmp/reverb.ppm
//! ```
//!
//! Le format est du **PPM binaire** (P6), que tout visualiseur ouvre et que
//! `magick reverb.ppm reverb.png` convertit — écrire un PNG demanderait un
//! encodeur, donc une dépendance, pour un outil de diagnostic.

use std::rc::Rc;

use reverb_anim::{CATALOGUE, Geometrie};
use reverb_gui::plan::Plan;
use reverb_gui::{FamilleAnimation, Fenetre, LigneTemperature, LigneVentilateur, PointLed};
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
    garnir(&interface);
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
fn garnir(interface: &Fenetre) {
    let plan = Plan::nouveau(&Geometrie::mesuree());
    let mut points = Vec::new();

    for (rang, position) in Position::ALL.into_iter().enumerate() {
        for led in 0..LEDS_PER_FAN as usize {
            let place = plan.led_ventilateur(position, led).expect("huit LED");
            points.push(PointLed {
                x: place.x,
                y: place.y,
                rayon: plan.rayon_anneau() / 8.0,
                couleur: teinte(rang as f32 / 10.0 + led as f32 / 8.0 / 3.0),
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
                couleur: teinte(0.55 + slot as f32 / 20.0 + led as f32 / 11.0 / 6.0),
                choisie: false,
            });
        }
    }
    interface.set_leds(ModelRc::new(VecModel::from(points)));

    interface.set_familles(ModelRc::new(VecModel::from(
        CATALOGUE
            .iter()
            .map(|nom| FamilleAnimation {
                nom: SharedString::from(*nom),
                effet: SharedString::from("…"),
                accepte_couleur: true,
            })
            .collect::<Vec<FamilleAnimation>>(),
    )));
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
    interface.set_temperatures(ModelRc::new(VecModel::from(vec![LigneTemperature {
        capteur: SharedString::from("kraken2023elite:coolant"),
        valeur: SharedString::from("34.2 °C"),
    }])));
    interface.set_cible(SharedString::from("le ventilateur arriere"));
    interface.set_animation_courante(SharedString::from("comete"));
    interface.set_message(SharedString::from(
        "aperçu hors ligne — aucun démon interrogé",
    ));
    interface.set_connecte(true);
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
