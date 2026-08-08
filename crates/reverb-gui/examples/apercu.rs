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

use reverb_anim::{Animation, CATALOGUE, Direction, Geometrie};
use reverb_gui::plan::{Plan, halo, places_des_ancres, rayon_du_disque};
use reverb_gui::{
    AncreEcran, FamilleAnimation, Fenetre, LigneProfil, LigneTemperature, LigneVentilateur,
    LigneZone, PointHalo, PointLed,
};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};
use slint::{Color, ComponentHandle, ModelRc, PhysicalSize, SharedString, VecModel};

const LARGEUR: u32 = 1280;
const HAUTEUR: u32 = 820;

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

    // Le halo de chaque pastille (#64), couche par couche : la plus large
    // d'abord, donc au fond. C'est `plan::halo` qui décide de tout — combien de
    // couches, à quel rayon, à quelle opacité — et une LED noire n'en a aucune.
    let mut halos = Vec::new();
    for rang in (0..halo(Rgb::new(1, 1, 1)).len()).rev() {
        for point in &points {
            let couleur = Rgb::new(
                point.couleur.red(),
                point.couleur.green(),
                point.couleur.blue(),
            );
            if let Some(couche) = halo(couleur).get(rang) {
                halos.push(PointHalo {
                    x: point.x,
                    y: point.y,
                    rayon: point.rayon * couche.rayon,
                    couleur: Color::from_argb_u8(
                        (couche.opacite * 255.0).round() as u8,
                        couleur.r,
                        couleur.g,
                        couleur.b,
                    ),
                });
            }
        }
    }
    interface.set_halos(ModelRc::new(VecModel::from(halos)));

    interface.set_leds(ModelRc::new(VecModel::from(points)));
    interface.set_habillage(SharedString::from(plan.commandes_habillage()));
    interface.set_silhouette(SharedString::from(plan.commandes_silhouette()));
    interface.set_faces(SharedString::from(plan.commandes_faces()));
    interface.set_organes(SharedString::from(plan.commandes_organes()));
    interface.set_anneaux(SharedString::from(plan.commandes_anneaux()));
    interface.set_aretes(SharedString::from(
        plan.aretes()
            .iter()
            .map(|(debut, fin)| format!("M {} {} L {} {} ", debut.x, debut.y, fin.x, fin.y))
            .collect::<String>(),
    ));

    interface.set_familles(ModelRc::new(VecModel::from(
        CATALOGUE
            .iter()
            .map(|nom| {
                let acceptees = Animation::par_nom(nom)
                    .map_or(&[] as &[&str], |animation| animation.parametres_acceptes());
                FamilleAnimation {
                    nom: SharedString::from(*nom),
                    effet: SharedString::from(
                        "Une phrase qui décrit l'effet, sur deux lignes le cas échéant.",
                    ),
                    accepte_couleur: acceptees.contains(&"couleur"),
                    suit_une_direction: acceptees.contains(&"direction"),
                    exige_une_sonde: Animation::par_nom(nom).is_ok_and(|animation| {
                        animation.parametres_obligatoires().contains(&"sonde")
                    }),
                }
            })
            .collect::<Vec<FamilleAnimation>>(),
    )));
    interface.set_directions(ModelRc::new(VecModel::from(
        Direction::ALL
            .into_iter()
            .map(|direction| SharedString::from(direction.slug()))
            .collect::<Vec<SharedString>>(),
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
    interface.set_luminosite_ecran(60);

    // `REVERB_ONGLET=ecran|ventilos` ouvre l'un des deux autres panneaux : ils
    // ne se regardent pas en même temps que l'éclairage, et c'est justement ce
    // que la mise en page à onglets tranche. Sans cette variable, deux tiers du
    // panneau de droite ne se vérifieraient jamais sans session graphique.
    interface.set_onglet(match std::env::var("REVERB_ONGLET").as_deref() {
        Ok("ecran") => 1,
        Ok("ventilos") => 2,
        _ => 0,
    });

    // Les ambiances : les deux exemples livrés, plus une que l'on vient de
    // rappeler — c'est la pastille allumée qu'on regarde.
    interface.set_profils(ModelRc::new(VecModel::from(
        [("abysse", false), ("forge", false), ("soirée d'été", true)]
            .into_iter()
            .map(|(nom, rappele)| LigneProfil {
                nom: SharedString::from(nom),
                rappele,
            })
            .collect::<Vec<LigneProfil>>(),
    )));

    // Les sondes, sous les noms que la fenêtre produit vraiment (#51) : c'est
    // ce que `thermique`, le cadran et un champ de composition offrent.
    interface.set_sondes_lisibles(ModelRc::new(VecModel::from(
        [
            "CPU",
            "Liquide",
            "GPU",
            "NVMe CT2000T705SSD5",
            "NVMe CT4000P3SSD8",
        ]
        .into_iter()
        .map(SharedString::from)
        .collect::<Vec<SharedString>>(),
    )));
    interface.set_sonde_choisie(1);

    // La composition (#80) : deux champs posés, l'ancre de droite visée. Les
    // places viennent de `Ancre::boite()` — les boîtes du démon, pas un dessin.
    let garnis = [("haut", "LIQUIDE"), ("gauche", "CPU")];
    interface.set_rayon_disque(rayon_du_disque());
    interface.set_champs_max(4);
    interface.set_champs_poses(i32::try_from(garnis.len()).unwrap_or(0));
    interface.set_ancres(ModelRc::new(VecModel::from(
        places_des_ancres()
            .into_iter()
            .map(|place| {
                let porte = garnis
                    .into_iter()
                    .find(|(nom, _)| *nom == place.ancre.slug())
                    .map(|(_, porte)| porte);
                AncreEcran {
                    nom: SharedString::from(place.ancre.slug()),
                    porte: SharedString::from(porte.unwrap_or_default()),
                    x: place.x,
                    y: place.y,
                    largeur: place.largeur,
                    hauteur: place.hauteur,
                    occupee: porte.is_some(),
                    choisie: place.ancre.slug() == "droite",
                }
            })
            .collect::<Vec<AncreEcran>>(),
    )));
    interface.set_libelle_champ(SharedString::from("GPU"));
    interface.set_ancre_visee(SharedString::from("droite"));

    // `REVERB_AFFICHAGE=composition` déroule le panneau de composition ; sinon
    // c'est le cadran, l'affichage le plus courant.
    if std::env::var("REVERB_AFFICHAGE").as_deref() == Ok("composition") {
        interface.set_affichage_choisi(4);
        interface.set_fond_choisi(1);
        interface.set_chemin_fond(SharedString::from("/home/nico/images/abysse.png"));
        interface.set_affichage_ecran(SharedString::from("layout"));
    } else {
        interface.set_affichage_choisi(1);
        interface.set_argument_ecran(SharedString::from("kraken2023elite:coolant"));
        interface.set_affichage_ecran(SharedString::from("gauge:kraken2023elite:coolant"));
    }
    interface.set_ventilateurs(ModelRc::new(VecModel::from(vec![
        // Un canal de chaque espèce : celui qui n'a pas de mode automatique et
        // n'affiche donc pas de bouton « auto », et celui qui en a un (#50).
        LigneVentilateur {
            canal: SharedString::from("nzxtsmart2:fan-1"),
            position: SharedString::from("radiateur haut"),
            rpm: SharedString::from("1180"),
            pwm: 60,
            mode: SharedString::from("manuel"),
            lisible: true,
            sait_faire_auto: false,
        },
        LigneVentilateur {
            canal: SharedString::from("kraken2023elite:pump"),
            position: SharedString::new(),
            rpm: SharedString::from("2400"),
            pwm: 75,
            mode: SharedString::from("courbe-de-l'hôte"),
            lisible: true,
            sait_faire_auto: true,
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
