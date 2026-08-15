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
use reverb_gui::reglages::{
    Poignee, TRACE_ASPECT, TRACE_CHAUD, TRACE_FROID, commandes_de_trace, consigne_affichee,
    degres_lisibles, point_du_palier,
};
use reverb_gui::sondes::{COURBE_ASPECT, Historique, MEMOIRE, Releve, commandes_de_courbe};
use reverb_gui::{
    AncreEcran, FamilleAnimation, Fenetre, LigneProfil, LigneTemperature, LigneVentilateur,
    LigneZone, PalierCourbe, PointHalo, PointLed,
};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::regulation::Courbe;
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
    // Les douze palettes de #126, précédées de « Aucune ». Sans ce modèle, le
    // menu se rendrait **vide** dans l'image d'aperçu — soit exactement le
    // genre de défaut que cette image existe pour attraper.
    //
    // `REVERB_PALETTE=<nom>` en choisit une : le menu à « Aucune » est le cas
    // par défaut, et celui où une palette est posée ne se verrait sinon sur
    // aucune image.
    {
        let mut noms = vec![SharedString::from("Aucune")];
        noms.extend(
            reverb_anim::PALETTES
                .iter()
                .map(|nom| SharedString::from(*nom)),
        );
        interface.set_palettes(ModelRc::new(VecModel::from(noms)));
        if let Ok(choisie) = std::env::var("REVERB_PALETTE")
            && let Some(rang) = reverb_anim::PALETTES.iter().position(|nom| *nom == choisie)
        {
            interface.set_palette_choisie(i32::try_from(rang + 1).unwrap_or(0));
        }
    }
    interface.set_directions(ModelRc::new(VecModel::from(
        Direction::ALL
            .into_iter()
            .map(|direction| {
                SharedString::from(match direction.slug() {
                    "bas-haut" => "Bas → haut",
                    "haut-bas" => "Haut → bas",
                    "avant-arriere" => "Avant → arrière",
                    "arriere-avant" => "Arrière → avant",
                    "horaire" => "Horaire",
                    "antihoraire" => "Antihoraire",
                    "bords-centre" => "Bords → centre (chaque objet)",
                    _ => "Centre → bords (chaque objet)",
                })
            })
            .collect::<Vec<SharedString>>(),
    )));
    interface.set_animations(ModelRc::new(VecModel::from(
        std::iter::once("aucune")
            .chain(CATALOGUE.iter().copied())
            .map(SharedString::from)
            .collect::<Vec<SharedString>>(),
    )));
    // Ce que les pastilles affichent : capitale en tête, accents remis. Le
    // protocole, lui, écrit `comete` — sans accent, parce qu'une commande se tape.
    interface.set_animations_lisibles(ModelRc::new(VecModel::from(
        std::iter::once("aucune")
            .chain(CATALOGUE.iter().copied())
            .map(|nom| {
                SharedString::from(match nom {
                    "comete" => "Comète".to_owned(),
                    "arc-en-ciel" => "Arc-en-ciel".to_owned(),
                    autre => {
                        let mut lettres = autre.chars();
                        lettres.next().map_or_else(String::new, |premiere| {
                            premiere.to_uppercase().chain(lettres).collect()
                        })
                    }
                })
            })
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

    // `REVERB_TRACE=x0,y0,x1,y1` pose un geste de sélection **en cours**, en
    // fractions du **panneau** — la surface où le geste a lieu depuis #121, et
    // non plus le carré de la maquette qui y est centré.
    //
    // ⚠️ **Le repère a changé, et l'image le montre** : sur la fenêtre de cet
    // aperçu, `0,0,1,1` couvrait 276 à 824 px, le carré ; il couvre maintenant
    // 262 à 838, le panneau — quatorze pixels de marge de chaque côté, où un
    // geste ne traçait rien.
    //
    // ⚠️ **C'est ce qui manquait pour que #76 soit vérifiable.** Ce rectangle ne
    // vit que pendant un appui de souris : aucune des quatre autres variables ne
    // pouvait le montrer, et personne n'a donc pu constater qu'il restait large
    // de zéro. Un correctif de couleur avait été livré, et n'avait rien corrigé.
    if let Ok(trace) = std::env::var("REVERB_TRACE") {
        let coins: Vec<f32> = trace
            .split(',')
            .filter_map(|nombre| nombre.trim().parse().ok())
            .collect();
        if let [x0, y0, x1, y1] = coins[..] {
            interface.set_trace_x0(x0);
            interface.set_trace_y0(y0);
            interface.set_trace_x1(x1);
            interface.set_trace_y1(y1);
            interface.set_trace(true);
        } else {
            eprintln!("REVERB_TRACE attend quatre fractions séparées par des virgules");
        }
    }

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
    // `REVERB_VERROU=ouvert` montre les deux canaux verrouillés cadenas ouvert.
    // ⚠️ **Sans elle, la moitié du verrou ne se verrait sur aucune image** : le
    // grisé et le libellé du bouton sont deux états d'un même geste, et celui
    // qu'on ne rend pas est justement celui qui laisse écrire.
    let ouvert = std::env::var("REVERB_VERROU").as_deref() == Ok("ouvert");
    interface.set_ventilateurs(ModelRc::new(VecModel::from(vec![
        // Un canal de chaque espèce : celui qui n'a pas de mode automatique et
        // n'affiche donc pas de bouton « auto », celui que le démon régule
        // (#113), celui qui régule seul sur une courbe, celui qui régule seul
        // sur le profil d'usine du périphérique (#112) — c'est le mode des deux
        // canaux du Kraken sur SHYNAEL, et donc le cadenas qu'on verra vraiment
        // —, et celui qui est en quarantaine (#100, #102). Sans ces lignes, ni
        // le grisé d'un canal muet, ni ce qu'il écrit à côté de sa barre, ni le
        // cadenas ne se vérifieraient autrement qu'en débranchant un Kraken.
        //
        // ⚠️ **Le régulé, le verrouillé et l'ordinaire sont sur LA MÊME image**,
        // et c'est la seule façon de vérifier ce que l'issue #113 exige : les
        // deux premiers grisent une barre pour des raisons opposées, et
        // l'interface doit les distinguer. Les regarder sur deux captures
        // successives ne le dirait pas.
        LigneVentilateur {
            canal: SharedString::from("nzxtsmart2:fan-1"),
            position: SharedString::from("radiateur haut"),
            rpm: SharedString::from("1180"),
            pwm: 60,
            consigne: consigne(Some(60)),
            mode: SharedString::from("manuel"),
            lisible: true,
            sait_faire_auto: false,
            regule_seul: false,
            deverrouille: false,
            // Régulable et libre : sa barre se tire, et son bouton propose de
            // le prendre en charge.
            regulable: true,
            sous_regulation: false,
        },
        LigneVentilateur {
            canal: SharedString::from("nzxtsmart2:fan-2"),
            position: SharedString::from("radiateur milieu"),
            rpm: SharedString::from("1042"),
            // 33 %, ce que la courbe par défaut calcule à 36 °C de liquide : la
            // consigne d'un canal régulé est celle que le démon vient d'écrire,
            // et sa ligne `chan` la republie.
            pwm: 33,
            consigne: consigne(Some(33)),
            mode: SharedString::from("manuel"),
            lisible: true,
            sait_faire_auto: false,
            // ⚠️ Il ne régule pas seul — c'est le démon qui le régule. Sa barre
            // est grisée comme celle du canal verrouillé plus bas, et rien
            // d'autre que le témoin et le libellé du bouton ne les sépare.
            regule_seul: false,
            deverrouille: false,
            regulable: true,
            sous_regulation: true,
        },
        LigneVentilateur {
            canal: SharedString::from("kraken2023elite:pump-speed"),
            position: SharedString::new(),
            rpm: SharedString::from("1357"),
            pwm: 30,
            consigne: consigne(Some(30)),
            mode: SharedString::from("non-piloté"),
            lisible: true,
            sait_faire_auto: false,
            regule_seul: true,
            deverrouille: ouvert,
            // Son firmware régule déjà, et #99 le met hors scope : aucun bouton
            // de régulation, donc aucune confusion possible avec le cadenas.
            regulable: false,
            sous_regulation: false,
        },
        LigneVentilateur {
            canal: SharedString::from("kraken2023elite:fan-speed"),
            position: SharedString::new(),
            rpm: SharedString::from("714"),
            pwm: 75,
            consigne: consigne(Some(75)),
            mode: SharedString::from("courbe-de-l'hôte"),
            lisible: true,
            sait_faire_auto: true,
            regule_seul: true,
            deverrouille: ouvert,
            regulable: false,
            sous_regulation: false,
        },
        LigneVentilateur {
            canal: SharedString::from("nct6687:sys-fan-1"),
            position: SharedString::new(),
            // Ni régime ni mode : rien n'a été mesuré à ce tour, et la ligne ne
            // montre donc aucune mesure. Le curseur, lui, **reste où il était**
            // (#100) — d'où `pwm: 40` et non `0`, qui se lirait comme un
            // ventilateur qu'on vient d'arrêter.
            //
            // Le texte, lui, écrit `-- %` (#102) : le curseur garde une position,
            // il ne prétend pas que c'est une mesure. Les deux règles se
            // rencontrent exactement ici.
            rpm: SharedString::from("—"),
            pwm: 40,
            consigne: consigne(None),
            mode: SharedString::new(),
            lisible: false,
            sait_faire_auto: true,
            // Le mode d'une ligne en quarantaine est vide : elle ne régule donc
            // seule aux yeux de personne, et n'a pas de cadenas. Elle reste
            // inerte pour l'autre raison, celle de #100.
            regule_seul: false,
            deverrouille: false,
            // ⚠️ `regulable` est gardé d'un tour où le canal répondait : c'est
            // une capacité du pilote, pas une mesure. Son bouton reste donc
            // visible et **inerte**, comme « auto » — une commande qui
            // disparaît à chaque hoquet du contrôleur fait bouger la ligne sous
            // les doigts.
            regulable: true,
            sous_regulation: false,
        },
    ])));

    // La courbe de régulation (#113). Les paliers sont ceux de #99, et le tracé
    // vient de `trace_de_courbe` — la fonction que le démon exécute : c'est
    // aussi ce qui rend l'écrêtage vérifiable à l'œil, la plage débordant des
    // deux côtés des paliers.
    //
    // `REVERB_COURBE=invalide` montre l'autre face : une courbe dont les paliers
    // ne montent pas, refusée **en le disant** et sans tracé. Sans cette
    // variable, le refus ne se verrait sur aucune image — il ne vit que le
    // temps d'un réglage de travers.
    let paliers: Vec<(i32, u8)> = if std::env::var("REVERB_COURBE").as_deref() == Ok("invalide") {
        vec![(35_000, 30), (50_000, 100), (45_000, 60)]
    } else {
        Courbe::defaut().paliers().to_vec()
    };
    interface.set_borne_froide(SharedString::from(degres_lisibles(TRACE_FROID)));
    interface.set_borne_chaude(SharedString::from(degres_lisibles(TRACE_CHAUD)));
    // Le même rapport que la fenêtre, et pour la même raison : sans lui, cette
    // image montrerait le tracé écrasé dans un carré — c'est **sur elle** que le
    // défaut a été mesuré le 2026-08-15. Voir `TRACE_ASPECT`.
    interface.set_trace_aspect(TRACE_ASPECT);
    interface.set_courbe_aspect(COURBE_ASPECT);
    interface.set_paliers(ModelRc::new(VecModel::from(
        paliers
            .iter()
            .map(|(milli, pourcent)| PalierCourbe {
                temperature: SharedString::from(degres_lisibles(*milli)),
                consigne: SharedString::from(format!("{pourcent} %")),
                degres: milli.div_euclid(1_000),
                pourcent: i32::from(*pourcent),
                // La poignée, posée par `point_du_palier` — l'inverse exact de
                // la conversion qui lit le geste (#122).
                trace_x: point_du_palier((*milli, *pourcent)).0,
                trace_y: point_du_palier((*milli, *pourcent)).1,
            })
            .collect::<Vec<PalierCourbe>>(),
    )));
    match Courbe::depuis(&paliers) {
        Ok(courbe) => {
            interface.set_trace_courbe(SharedString::from(commandes_de_trace(&courbe)));
            interface.set_refus_courbe(SharedString::new());
        }
        Err(erreur) => {
            interface.set_trace_courbe(SharedString::new());
            interface.set_refus_courbe(SharedString::from(erreur.raison));
        }
    }
    // Les cinq sondes retenues. Les libellés sont ceux que la fenêtre produit
    // vraiment, modèles de disques compris (issue #51).
    //
    // ⚠️ **Les relevés sont inventés, le tracé non** : on garnit un vrai
    // `Historique` et c'est `commandes_de_courbe` qui dessine, la fonction même
    // que la fenêtre appelle. Cet aperçu fabriquait ses commandes à la main dans
    // le carré unité, si bien qu'il a montré le `viewbox` corrigé de #118
    // par-dessus des coordonnées qui ne l'étaient pas — un tracé **plus** écrasé
    // qu'avant, et une image qui démentait le correctif. Une seconde
    // implémentation n'a aucune raison d'être la bonne (#113).
    let mut historique = Historique::nouvel();
    let sondes = [
        ("CPU", "61.8 °C", 0.30_f32, true),
        ("Liquide", "34.2 °C", 0.55, true),
        ("GPU", "51.0 °C", 0.70, true),
        ("NVMe CT2000T705SSD5", "36.9 °C", 0.20, true),
        ("NVMe CT4000P3SSD8", "illisible", 0.0, false),
    ];
    for (libelle, _, base, lisible) in sondes {
        for rang in 0..MEMOIRE {
            let avance = rang as f32 / (MEMOIRE - 1) as f32;
            let releve = if lisible {
                let onde = (avance * 9.0 + base * 6.0).sin() * 0.35 * (0.4 + base);
                Releve::Valeur((40_000.0 + onde * 20_000.0) as i32)
            } else {
                Releve::Illisible
            };
            historique.noter(libelle, releve);
        }
    }
    interface.set_temperatures(ModelRc::new(VecModel::from(
        sondes
            .into_iter()
            .map(|(libelle, valeur, _, lisible)| LigneTemperature {
                libelle: SharedString::from(libelle),
                valeur: SharedString::from(valeur),
                courbe: SharedString::from(commandes_de_courbe(&historique, libelle)),
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
    interface.set_animation_courante(SharedString::from("Comète"));
    // `REVERB_MESSAGE=` (vide) montre la tête de fenêtre telle qu'elle est au
    // repos — c'est-à-dire au moment où la pastille verte est la seule chose à
    // lire, et où son libellé doit donc apparaître. Sans ça, l'aperçu porte
    // toujours un message et ne montre jamais le repli.
    interface.set_message(SharedString::from(match std::env::var("REVERB_MESSAGE") {
        Ok(pose) => pose,
        Err(_) if vraies.is_some() => "aperçu de la vraie image, prise sur le socket".to_owned(),
        Err(_) => "aperçu hors ligne — aucun démon interrogé".to_owned(),
    }));
    interface.set_connecte(true);
}

/// Ce qu'un canal écrit à côté de sa barre, par le même code que la fenêtre.
///
/// Un libellé écrit à la main ici montrerait une mise en page qui n'existe
/// pas — et l'aperçu ne sert qu'à juger celle qui existe. `None` est le canal
/// que le démon a rendu illisible.
fn consigne(pwm: Option<u8>) -> SharedString {
    let mut poignee = Poignee::nouvelle();
    poignee.mesurer(pwm.unwrap_or(0), std::time::Duration::ZERO);
    SharedString::from(consigne_affichee(&poignee, pwm))
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
