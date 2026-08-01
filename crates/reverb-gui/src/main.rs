//! `reverb-gui` — la fenêtre, cliente du démon.
//!
//! Elle n'ouvre **aucun** périphérique et n'écrit **aucun** fichier : tout ce
//! qu'elle montre et tout ce qu'elle règle passe par le socket Unix du démon,
//! qui reste seul à détenir les bus (ADR-002). Le socket est ainsi l'unique
//! franchissement de privilège, au lieu d'un second mécanisme de droits à
//! entretenir.
//!
//! # Trois fils, et aucun ne fait attendre les deux autres
//!
//! - le fil de l'interface dessine et écoute la souris ;
//! - un fil **regarde** (`watch`) et pousse chaque image reçue dans la fenêtre ;
//! - un fil **agit** : il prend les ordres dans une file et attend les réponses
//!   du démon à la place de l'interface.
//!
//! Sans ce troisième fil, un clic attendrait la réponse du démon — jusqu'à une
//! image entière, cinquante millisecondes — et l'interface collerait aux doigts
//! pendant qu'une animation tourne.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::{Sender, channel};
use std::thread;
use std::time::Duration;

use reverb_anim::{Animation, CATALOGUE};
use reverb_gui::client::{Abonnement, Client, chemin_du_socket};
use reverb_gui::plan::{Cible, Place, Plan};
use reverb_gui::{FamilleAnimation, Fenetre, LigneTemperature, LigneVentilateur, PointLed};
use reverb_proto::ipc::{FanAction, LightTarget, Request, ResponseLine};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb};
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};

/// Ce que chaque animation donne à voir.
///
/// Une phrase par famille, et elle manquait : six animations ont déjà été
/// soumises à l'œil sans qu'aucune n'ait été décrite, et la réponse fut « ne
/// sachant pas ce que c'est censé représenter, je ne sais pas si ça se comporte
/// comme il devrait ». Un nom ne suffit pas — il faut dire ce qui bouge, et où.
const EFFETS: &[(&str, &str)] = &[
    (
        "vague",
        "Un front de couleur traverse le boîtier en ligne droite. Deux LED à la même hauteur \
         s'allument ensemble : c'est l'onde plane, et la preuve que le boîtier et la RAM sont \
         synchronisés dans l'espace.",
    ),
    (
        "comete",
        "Une tête vive suivie d'une traîne qui s'éteint, qui fait le tour du boîtier. Presque \
         tout reste noir — c'est l'animation la plus rapide du catalogue.",
    ),
    (
        "respiration",
        "Tout le boîtier s'éclaire et s'assombrit ensemble, lentement, sans jamais s'éteindre \
         tout à fait.",
    ),
    (
        "arc-en-ciel",
        "Toutes les teintes défilent le long de la direction choisie. La seule qui refuse une \
         couleur : elle les produit toutes.",
    ),
    (
        "balayage",
        "Une bande étroite balaie le boîtier d'un bout à l'autre, puis recommence du même côté.",
    ),
    (
        "braise",
        "Des points chauds apparaissent et retombent au hasard, comme un feu qui couve. Rien ne \
         traverse : ça respire par endroits.",
    ),
];

/// Ce que l'utilisateur vise en ce moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selection {
    Groupe(LightTarget),
    Led(Cible),
}

impl Selection {
    /// Comment le dire à l'écran.
    fn nom(self) -> String {
        match self {
            Selection::Groupe(LightTarget::All) => "tout le boîtier".to_owned(),
            Selection::Groupe(LightTarget::Fans) => "les dix ventilateurs".to_owned(),
            Selection::Groupe(LightTarget::Ram) => "les quatre barrettes".to_owned(),
            Selection::Groupe(LightTarget::Fan(position)) => {
                format!("le ventilateur {}", position.name())
            }
            Selection::Groupe(LightTarget::RamSlot(slot)) => format!("la barrette {slot}"),
            Selection::Led(Cible::Led { position, led }) => {
                format!("la LED {} de {}", led + 1, position.name())
            }
            Selection::Led(Cible::Barrette { slot, led }) => {
                format!("la LED {} de la barrette {slot}", led + 1)
            }
        }
    }
}

/// L'image courante, telle que le démon l'a poussée.
#[derive(Clone)]
struct Tableau {
    ventilateurs: [[Rgb; LEDS_PER_FAN as usize]; 10],
    barrettes: [[Rgb; LEDS_PER_STICK]; SLOT_COUNT],
}

impl Tableau {
    fn noir() -> Tableau {
        Tableau {
            ventilateurs: [[Rgb::BLACK; LEDS_PER_FAN as usize]; 10],
            barrettes: [[Rgb::BLACK; LEDS_PER_STICK]; SLOT_COUNT],
        }
    }

    /// Range une ligne `frame` à sa place.
    fn poser(&mut self, cible: &str, couleurs: &[Rgb]) {
        if let Some(Ok(position)) = cible.strip_prefix("fan:").map(Position::from_slug) {
            for (place, couleur) in self.ventilateurs[position.index()].iter_mut().zip(couleurs) {
                *place = *couleur;
            }
        } else if let Some(barrette) = cible
            .strip_prefix("slot:")
            .and_then(|numero| numero.parse::<usize>().ok())
            .and_then(|slot| self.barrettes.get_mut(slot))
        {
            for (place, couleur) in barrette.iter_mut().zip(couleurs) {
                *place = *couleur;
            }
        }
    }
}

fn main() -> ExitCode {
    let fenetre = match Fenetre::new() {
        Ok(fenetre) => fenetre,
        Err(erreur) => {
            // Le cas le plus probable sur une machine de bureau : pas de rendu
            // accéléré disponible. Le dire, et dire par quoi s'en sortir.
            eprintln!("erreur : impossible d'ouvrir la fenêtre : {erreur}");
            eprintln!("         essai possible : SLINT_BACKEND=winit-software reverb-gui");
            return ExitCode::FAILURE;
        }
    };

    let socket = chemin_du_socket();
    let plan = Plan::nouveau(&reverb_anim::Geometrie::mesuree());
    let tableau = std::rc::Rc::new(std::cell::RefCell::new(Tableau::noir()));
    let selection = std::rc::Rc::new(std::cell::Cell::new(Selection::Groupe(LightTarget::All)));

    fenetre.set_familles(familles());
    fenetre.set_cible(SharedString::from(selection.get().nom()));
    dessiner(&fenetre, &plan, &tableau.borrow(), selection.get());

    let (ordres, file) = channel::<Request>();
    lancer_le_fil_des_ordres(socket.clone(), file, fenetre.as_weak());
    lancer_le_fil_des_images(
        socket,
        fenetre.as_weak(),
        plan.clone(),
        tableau.clone(),
        selection.clone(),
    );

    brancher(&fenetre, &plan, &tableau, &selection, ordres.clone());

    // La télémétrie n'a pas de flux : on la redemande, doucement. Une seconde
    // suffit pour des tours par minute, et n'ajoute rien de mesurable au démon.
    let horloge = slint::Timer::default();
    horloge.start(
        slint::TimerMode::Repeated,
        Duration::from_secs(1),
        move || {
            let _ = ordres.send(Request::Status);
        },
    );

    if let Err(erreur) = fenetre.run() {
        eprintln!("erreur : {erreur}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Le fil qui agit : il attend les réponses du démon à la place de l'interface.
fn lancer_le_fil_des_ordres(
    socket: PathBuf,
    file: std::sync::mpsc::Receiver<Request>,
    fenetre: Weak<Fenetre>,
) {
    thread::spawn(move || {
        let mut client: Option<Client> = None;
        for requete in file {
            if client.is_none() {
                client = Client::connecter(&socket).ok();
            }
            let Some(ouvert) = client.as_mut() else {
                dire(
                    &fenetre,
                    false,
                    format!("démon injoignable sur {}", socket.display()),
                );
                continue;
            };
            match ouvert.demander(&requete) {
                Ok(lignes) => {
                    let etait = matches!(requete, Request::Status);
                    repondre(&fenetre, &requete, &lignes);
                    if !etait {
                        // Une commande qui passe est la meilleure preuve de vie
                        // qu'on puisse afficher.
                        dire(&fenetre, true, String::new());
                    }
                }
                Err(erreur) => {
                    client = None;
                    dire(&fenetre, false, format!("démon injoignable : {erreur}"));
                }
            }
        }
    });
}

/// Le fil qui regarde : une image, une mise à jour de la maquette.
fn lancer_le_fil_des_images(
    socket: PathBuf,
    fenetre: Weak<Fenetre>,
    plan: Plan,
    tableau: std::rc::Rc<std::cell::RefCell<Tableau>>,
    selection: std::rc::Rc<std::cell::Cell<Selection>>,
) {
    // `Rc` et `Cell` vivent dans le fil de l'interface : le fil des images ne
    // les touche pas, il lui envoie les couleurs par le canal de Slint.
    let _ = (&tableau, &selection);
    thread::spawn(move || {
        loop {
            match Abonnement::ouvrir(&socket) {
                Ok(mut abonnement) => {
                    dire(&fenetre, true, "connecté".to_owned());
                    while let Some(image) = abonnement.image_suivante() {
                        let cadres: Vec<(String, Vec<Rgb>)> = image
                            .into_iter()
                            .filter_map(|ligne| match ligne {
                                ResponseLine::Frame { cible, couleurs } => Some((cible, couleurs)),
                                _ => None,
                            })
                            .collect();
                        if cadres.is_empty() {
                            continue;
                        }
                        let plan = plan.clone();
                        let _ = fenetre.upgrade_in_event_loop(move |fenetre| {
                            appliquer_image(&fenetre, &plan, &cadres);
                        });
                    }
                    dire(&fenetre, false, "le démon a fermé le flux".to_owned());
                }
                Err(erreur) => {
                    dire(
                        &fenetre,
                        false,
                        format!("démon injoignable ({erreur}) — « systemctl start reverbd »"),
                    );
                }
            }
            // On réessaie : un démon redémarré ne doit pas obliger à relancer la
            // fenêtre.
            thread::sleep(Duration::from_secs(2));
        }
    });
}

/// Range une image reçue et redessine.
///
/// Le tableau et la sélection vivent dans la fenêtre : c'est le seul endroit où
/// le fil de l'interface et les images se croisent, et ils s'y croisent sans
/// verrou puisque tout se passe dans la boucle d'événements.
fn appliquer_image(fenetre: &Fenetre, plan: &Plan, cadres: &[(String, Vec<Rgb>)]) {
    let mut tableau = Tableau::noir();
    // Reprendre ce qui est déjà affiché : une image ne porte que les cibles
    // qu'elle réécrit.
    for (index, led) in fenetre.get_leds().iter().enumerate() {
        if let Some((position, rang)) = index_ventilateur(index) {
            tableau.ventilateurs[position][rang] = depuis_slint(led.couleur);
        } else if let Some((slot, rang)) = index_barrette(index) {
            tableau.barrettes[slot][rang] = depuis_slint(led.couleur);
        }
    }
    for (cible, couleurs) in cadres {
        tableau.poser(cible, couleurs);
    }
    fenetre.set_leds(modele_leds(plan, &tableau, choix(fenetre)));
}

/// Redessine la maquette entière.
fn dessiner(fenetre: &Fenetre, plan: &Plan, tableau: &Tableau, selection: Selection) {
    fenetre.set_leds(modele_leds(plan, tableau, Some(selection)));
}

fn modele_leds(plan: &Plan, tableau: &Tableau, selection: Option<Selection>) -> ModelRc<PointLed> {
    let mut points = Vec::with_capacity(124);
    for position in Position::ALL {
        for led in 0..LEDS_PER_FAN as usize {
            let place = plan
                .led_ventilateur(position, led)
                .unwrap_or(Place { x: 0.0, y: 0.0 });
            points.push(point(
                place,
                plan,
                tableau.ventilateurs[position.index()][led],
                choisie(selection, Cible::Led { position, led }),
            ));
        }
    }
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            let place = plan
                .led_barrette(slot, led)
                .unwrap_or(Place { x: 0.0, y: 0.0 });
            points.push(point(
                place,
                plan,
                tableau.barrettes[slot][led],
                choisie(selection, Cible::Barrette { slot, led }),
            ));
        }
    }
    ModelRc::new(VecModel::from(points))
}

fn point(place: Place, plan: &Plan, couleur: Rgb, choisie: bool) -> PointLed {
    PointLed {
        x: place.x,
        y: place.y,
        // Le rayon d'une LED : le huitième de celui de l'anneau, soit la moitié
        // du diamètre que la projection s'est donné.
        rayon: plan.rayon_anneau() / 8.0,
        couleur: Color::from_rgb_u8(couleur.r, couleur.g, couleur.b),
        choisie,
    }
}

/// La LED désignée est-elle celle qu'on vise ?
fn choisie(selection: Option<Selection>, cible: Cible) -> bool {
    match selection {
        Some(Selection::Led(visee)) => visee == cible,
        Some(Selection::Groupe(groupe)) => match (groupe, cible) {
            (LightTarget::All, _) => true,
            (LightTarget::Fans, Cible::Led { .. }) => true,
            (LightTarget::Ram, Cible::Barrette { .. }) => true,
            (LightTarget::Fan(vise), Cible::Led { position, .. }) => vise == position,
            (LightTarget::RamSlot(vise), Cible::Barrette { slot, .. }) => vise == slot,
            _ => false,
        },
        None => false,
    }
}

fn brancher(
    fenetre: &Fenetre,
    plan: &Plan,
    tableau: &std::rc::Rc<std::cell::RefCell<Tableau>>,
    selection: &std::rc::Rc<std::cell::Cell<Selection>>,
    ordres: Sender<Request>,
) {
    // ── Cliquer une LED ────────────────────────────────────────────────────
    {
        let plan = plan.clone();
        let selection = selection.clone();
        let faible = fenetre.as_weak();
        fenetre.on_clic_maquette(move |x, y| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            if let Some(cible) = plan.sous(Place { x, y }) {
                selection.set(Selection::Led(cible));
                rafraichir_selection(&fenetre, &plan, selection.get());
            }
        });
    }

    // ── Choisir un groupe ──────────────────────────────────────────────────
    {
        let plan = plan.clone();
        let selection = selection.clone();
        let faible = fenetre.as_weak();
        fenetre.on_choisir_cible(move |nom| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            let groupe = match nom.as_str() {
                "fans" => LightTarget::Fans,
                "ram" => LightTarget::Ram,
                _ => LightTarget::All,
            };
            selection.set(Selection::Groupe(groupe));
            rafraichir_selection(&fenetre, &plan, selection.get());
        });
    }

    // ── L'aperçu de la couleur saisie ──────────────────────────────────────
    {
        let faible = fenetre.as_weak();
        fenetre.on_couleur_saisie(move |texte| {
            if let (Some(fenetre), Some(couleur)) = (faible.upgrade(), lire_couleur(&texte)) {
                fenetre.set_apercu(Color::from_rgb_u8(couleur.r, couleur.g, couleur.b));
            }
        });
    }

    // ── Appliquer la couleur ───────────────────────────────────────────────
    {
        let selection = selection.clone();
        let tableau = tableau.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_appliquer_couleur(move || {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            let Some(couleur) = lire_couleur(&fenetre.get_couleur()) else {
                fenetre.set_message(SharedString::from(
                    "couleur : six chiffres hexadécimaux, par exemple ff40ff",
                ));
                return;
            };
            let requete = match selection.get() {
                Selection::Groupe(target) => Request::Light {
                    target,
                    color: couleur,
                },
                // Une seule LED : le protocole n'a pas de cible « une LED », et
                // c'est voulu — `paint` réécrit la cible entière, en gardant
                // les couleurs affichées pour les autres LED. C'est aussi ce
                // qui rend le geste réversible : la LED voisine ne bouge pas.
                Selection::Led(cible) => peinture(&fenetre, &tableau.borrow(), cible, couleur),
            };
            let _ = envoi.send(requete);
        });
    }

    // ── Les animations ─────────────────────────────────────────────────────
    {
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_lancer_animation(move |nom| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            let Ok(animation) = Animation::par_nom(&nom) else {
                return;
            };
            // Seules les clés que cette animation accepte : `arc-en-ciel`
            // refuse `couleur`, et la lui donner ferait refuser la commande
            // entière.
            let acceptees = animation.parametres_acceptes();
            let mut reglages = Vec::new();
            if let (true, Some(couleur)) = (
                acceptees.contains(&"couleur"),
                lire_couleur(&fenetre.get_couleur()),
            ) {
                reglages.push((
                    "couleur".to_owned(),
                    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b),
                ));
            }
            if acceptees.contains(&"vitesse") {
                reglages.push(("vitesse".to_owned(), fenetre.get_vitesse().to_string()));
            }
            let index = usize::try_from(fenetre.get_direction()).unwrap_or(0);
            if let (true, Some(direction)) = (
                acceptees.contains(&"direction"),
                reverb_anim::Direction::ALL.get(index),
            ) {
                reglages.push(("direction".to_owned(), direction.slug().to_owned()));
            }
            fenetre.set_animation_courante(nom.clone());
            let _ = envoi.send(Request::Animate {
                name: Some(nom.to_string()),
                reglages,
            });
        });
    }
    {
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_arreter_animation(move || {
            if let Some(fenetre) = faible.upgrade() {
                fenetre.set_animation_courante(SharedString::from("aucune"));
            }
            let _ = envoi.send(Request::Animate {
                name: None,
                reglages: Vec::new(),
            });
        });
    }

    // ── Les ventilateurs ───────────────────────────────────────────────────
    {
        let envoi = ordres.clone();
        fenetre.on_regler_ventilateur(move |canal, consigne| {
            let _ = envoi.send(Request::Fan {
                channel: canal.to_string(),
                action: FanAction::Pwm(u8::try_from(consigne.clamp(0, 100)).unwrap_or(0)),
            });
        });
    }
    {
        let envoi = ordres;
        fenetre.on_rendre_au_firmware(move |canal| {
            let _ = envoi.send(Request::Fan {
                channel: canal.to_string(),
                action: FanAction::Auto,
            });
        });
    }
}

/// Réécrit la cible d'une LED en n'y changeant qu'elle.
fn peinture(fenetre: &Fenetre, _tableau: &Tableau, cible: Cible, couleur: Rgb) -> Request {
    // Les couleurs affichées font foi : ce sont celles que le démon vient
    // d'envoyer au matériel.
    let mut couleurs = Vec::new();
    let (target, base, rang) = match cible {
        Cible::Led { position, led } => (
            LightTarget::Fan(position),
            position.index() * LEDS_PER_FAN as usize,
            led,
        ),
        Cible::Barrette { slot, led } => (
            LightTarget::RamSlot(slot),
            10 * LEDS_PER_FAN as usize + slot * LEDS_PER_STICK,
            led,
        ),
    };
    let combien = match cible {
        Cible::Led { .. } => LEDS_PER_FAN as usize,
        Cible::Barrette { .. } => LEDS_PER_STICK,
    };
    let leds = fenetre.get_leds();
    for index in 0..combien {
        let actuelle = leds
            .row_data(base + index)
            .map_or(Rgb::BLACK, |point| depuis_slint(point.couleur));
        couleurs.push(if index == rang { couleur } else { actuelle });
    }
    Request::Paint { target, couleurs }
}

fn rafraichir_selection(fenetre: &Fenetre, plan: &Plan, selection: Selection) {
    fenetre.set_cible(SharedString::from(selection.nom()));
    let mut tableau = Tableau::noir();
    for (index, led) in fenetre.get_leds().iter().enumerate() {
        if let Some((position, rang)) = index_ventilateur(index) {
            tableau.ventilateurs[position][rang] = depuis_slint(led.couleur);
        } else if let Some((slot, rang)) = index_barrette(index) {
            tableau.barrettes[slot][rang] = depuis_slint(led.couleur);
        }
    }
    fenetre.set_leds(modele_leds(plan, &tableau, Some(selection)));
}

/// La sélection courante, relue depuis ce que la fenêtre affiche.
fn choix(fenetre: &Fenetre) -> Option<Selection> {
    // La bordure blanche est le seul état de sélection que la fenêtre porte :
    // le relire évite de partager une cellule entre deux fils pour une
    // information que le dessin contient déjà.
    let leds = fenetre.get_leds();
    let choisies: Vec<usize> = (0..leds.row_count())
        .filter(|index| leds.row_data(*index).is_some_and(|point| point.choisie))
        .collect();
    match choisies.len() {
        0 => None,
        1 => {
            let index = choisies[0];
            index_ventilateur(index)
                .map(|(position, led)| {
                    Selection::Led(Cible::Led {
                        position: Position::ALL[position],
                        led,
                    })
                })
                .or_else(|| {
                    index_barrette(index)
                        .map(|(slot, led)| Selection::Led(Cible::Barrette { slot, led }))
                })
        }
        _ => Some(Selection::Groupe(LightTarget::All)),
    }
}

fn index_ventilateur(index: usize) -> Option<(usize, usize)> {
    (index < 10 * LEDS_PER_FAN as usize)
        .then(|| (index / LEDS_PER_FAN as usize, index % LEDS_PER_FAN as usize))
}

fn index_barrette(index: usize) -> Option<(usize, usize)> {
    let debut = 10 * LEDS_PER_FAN as usize;
    (index >= debut && index < debut + SLOT_COUNT * LEDS_PER_STICK).then(|| {
        (
            (index - debut) / LEDS_PER_STICK,
            (index - debut) % LEDS_PER_STICK,
        )
    })
}

fn depuis_slint(couleur: Color) -> Rgb {
    Rgb::new(couleur.red(), couleur.green(), couleur.blue())
}

fn lire_couleur(texte: &str) -> Option<Rgb> {
    Rgb::from_hex(texte.trim()).ok()
}

fn familles() -> ModelRc<FamilleAnimation> {
    let familles: Vec<FamilleAnimation> = CATALOGUE
        .iter()
        .map(|nom| FamilleAnimation {
            nom: SharedString::from(*nom),
            effet: SharedString::from(
                EFFETS
                    .iter()
                    .find(|(famille, _)| famille == nom)
                    .map_or("", |(_, effet)| effet),
            ),
            accepte_couleur: Animation::par_nom(nom)
                .is_ok_and(|animation| animation.parametres_acceptes().contains(&"couleur")),
        })
        .collect();
    ModelRc::new(VecModel::from(familles))
}

/// Range une réponse du démon dans la fenêtre.
fn repondre(fenetre: &Weak<Fenetre>, requete: &Request, lignes: &[ResponseLine]) {
    if let Some(ResponseLine::Error { message }) = lignes.last() {
        let message = message.clone();
        let _ = fenetre.upgrade_in_event_loop(move |fenetre| {
            fenetre.set_connecte(true);
            fenetre.set_message(SharedString::from(message));
        });
        return;
    }

    match requete {
        Request::Status => {
            let lignes = lignes.to_vec();
            let _ = fenetre.upgrade_in_event_loop(move |fenetre| {
                poser_telemetrie(&fenetre, &lignes);
            });
        }
        Request::Lighting => {
            let lignes = lignes.to_vec();
            let _ = fenetre.upgrade_in_event_loop(move |fenetre| {
                for ligne in &lignes {
                    if let ResponseLine::Anim { nom, .. } = ligne {
                        fenetre.set_animation_courante(SharedString::from(nom.clone()));
                    }
                }
            });
        }
        _ => {}
    }
}

fn poser_telemetrie(fenetre: &Fenetre, lignes: &[ResponseLine]) {
    let mut canaux = Vec::new();
    let mut temperatures = Vec::new();
    for ligne in lignes {
        match ligne {
            ResponseLine::Channel {
                channel,
                position,
                rpm,
                pwm,
                mode,
            } => canaux.push(LigneVentilateur {
                canal: SharedString::from(channel.clone()),
                position: SharedString::from(
                    position.map_or_else(String::new, |position| position.name().to_owned()),
                ),
                rpm: SharedString::from(
                    rpm.map_or_else(|| "—".to_owned(), |tours| tours.to_string()),
                ),
                pwm: i32::from(pwm.unwrap_or(0)),
                mode: SharedString::from(mode.clone()),
                lisible: rpm.is_some(),
            }),
            ResponseLine::Temp {
                sensor,
                millidegrees,
            } => temperatures.push(LigneTemperature {
                capteur: SharedString::from(sensor.clone()),
                valeur: SharedString::from(format!("{:.1} °C", f64::from(*millidegrees) / 1000.0)),
            }),
            ResponseLine::Unreadable { subject, reason } => temperatures.push(LigneTemperature {
                capteur: SharedString::from(subject.clone()),
                valeur: SharedString::from(reason.clone()),
            }),
            _ => {}
        }
    }
    fenetre.set_ventilateurs(ModelRc::new(VecModel::from(canaux)));
    fenetre.set_temperatures(ModelRc::new(VecModel::from(temperatures)));
}

/// Écrit un état de connexion dans la fenêtre, depuis n'importe quel fil.
fn dire(fenetre: &Weak<Fenetre>, connecte: bool, message: String) {
    let _ = fenetre.upgrade_in_event_loop(move |fenetre| {
        fenetre.set_connecte(connecte);
        if !message.is_empty() || connecte {
            fenetre.set_message(SharedString::from(message));
        }
    });
}
