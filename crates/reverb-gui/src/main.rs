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

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::mpsc::{Sender, channel};
use std::thread;
use std::time::{Duration, Instant};

use reverb_anim::{Animation, CATALOGUE};
use reverb_gui::client::{Abonnement, Client, chemin_du_socket};
use reverb_gui::plan::{Cible, Place, Plan, Vue};
use reverb_gui::reglages::{Poignee, Reglage};
use reverb_gui::sondes::{Historique, Releve};
use reverb_gui::{
    FamilleAnimation, Fenetre, LigneTemperature, LigneVentilateur, LigneZone, PointLed,
};
use reverb_proto::ipc::{FanAction, LightTarget, Request, ResponseLine};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Led, Position, Rgb, Tsl};
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

/// Le niveau de détail de la maquette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Detail {
    /// Dix disques et quatre réglettes : on vise un organe entier.
    Ventilateur,
    /// Les cent vingt-quatre pastilles : on peint LED par LED.
    Led,
}

/// Ce que l'utilisateur vise en ce moment.
///
/// **Un ensemble de LED, jamais autre chose.** Viser un ventilateur entier, c'est
/// viser ses huit LED — ce qui rend une sélection composable à volonté, et donc
/// utilisable telle quelle comme définition d'une zone.
#[derive(Debug, Clone, Default, PartialEq)]
struct Selection {
    cibles: Vec<Cible>,
}

impl Selection {
    /// Les cent vingt-quatre.
    fn tout() -> Selection {
        let mut cibles = Vec::with_capacity(124);
        for position in Position::ALL {
            for led in 0..LEDS_PER_FAN as usize {
                cibles.push(Cible::Led { position, led });
            }
        }
        for slot in 0..SLOT_COUNT {
            for led in 0..LEDS_PER_STICK {
                cibles.push(Cible::Barrette { slot, led });
            }
        }
        Selection { cibles }
    }

    /// Toutes les LED des dix ventilateurs, ou des quatre barrettes.
    fn famille(ventilateurs: bool) -> Selection {
        Selection {
            cibles: Selection::tout()
                .cibles
                .into_iter()
                .filter(|cible| matches!(cible, Cible::Led { .. }) == ventilateurs)
                .collect(),
        }
    }

    fn contient(&self, cible: Cible) -> bool {
        self.cibles.contains(&cible)
    }

    fn ajouter(&mut self, cibles: Vec<Cible>) {
        for cible in cibles {
            if !self.contient(cible) {
                self.cibles.push(cible);
            }
        }
    }

    /// Comment le dire à l'écran.
    ///
    /// Les cas nommés d'abord : « le ventilateur arrière » se lit, « huit LED »
    /// ne se lit pas. On ne retombe sur le compte que lorsque la sélection ne
    /// correspond à aucun organe entier.
    fn nom(&self) -> String {
        let compte = self.cibles.len();
        if compte == 0 {
            return "rien".to_owned();
        }
        if *self == Selection::tout() {
            return "tout le boîtier".to_owned();
        }
        if *self == Selection::famille(true) {
            return "les dix ventilateurs".to_owned();
        }
        if *self == Selection::famille(false) {
            return "les quatre barrettes".to_owned();
        }
        if compte == 1 {
            return match self.cibles[0] {
                Cible::Led { position, led } => {
                    format!("la LED {} de {}", led + 1, position.name())
                }
                Cible::Barrette { slot, led } => {
                    format!("la LED {} de la barrette {slot}", led + 1)
                }
            };
        }
        let organes = self.organes();
        if organes.len() == 1 && self.est_entier(organes[0]) {
            return match organes[0] {
                Organe::Ventilateur(position) => format!("le ventilateur {}", position.name()),
                Organe::Reglette(slot) => format!("la barrette {slot}"),
            };
        }
        if organes.iter().all(|organe| self.est_entier(*organe)) {
            return format!("{} organes entiers", organes.len());
        }
        format!("{compte} LED sur {} organes", organes.len())
    }

    /// Les organes que cette sélection touche, dans l'ordre de la maquette.
    fn organes(&self) -> Vec<Organe> {
        let mut vus = Vec::new();
        for cible in &self.cibles {
            let organe = Organe::de(*cible);
            if !vus.contains(&organe) {
                vus.push(organe);
            }
        }
        vus.sort_by_key(Organe::rang);
        vus
    }

    /// Cet organe est-il **entièrement** dans la sélection ?
    fn est_entier(&self, organe: Organe) -> bool {
        organe
            .cibles()
            .into_iter()
            .all(|cible| self.contient(cible))
    }
}

/// Un ventilateur ou une barrette : ce qu'une commande du protocole sait viser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Organe {
    Ventilateur(Position),
    Reglette(usize),
}

impl Organe {
    fn de(cible: Cible) -> Organe {
        match cible {
            Cible::Led { position, .. } => Organe::Ventilateur(position),
            Cible::Barrette { slot, .. } => Organe::Reglette(slot),
        }
    }

    fn rang(&self) -> usize {
        match self {
            Organe::Ventilateur(position) => position.index(),
            Organe::Reglette(slot) => 10 + slot,
        }
    }

    fn cibles(self) -> Vec<Cible> {
        match self {
            Organe::Ventilateur(position) => (0..LEDS_PER_FAN as usize)
                .map(|led| Cible::Led { position, led })
                .collect(),
            Organe::Reglette(slot) => (0..LEDS_PER_STICK)
                .map(|led| Cible::Barrette { slot, led })
                .collect(),
        }
    }

    fn groupe(self) -> LightTarget {
        match self {
            Organe::Ventilateur(position) => LightTarget::Fan(position),
            Organe::Reglette(slot) => LightTarget::RamSlot(slot),
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

    fn couleur(&self, cible: Cible) -> Rgb {
        match cible {
            Cible::Led { position, led } => self.ventilateurs[position.index()][led],
            Cible::Barrette { slot, led } => self.barrettes[slot][led],
        }
    }

    /// La couleur d'un organe entier : la moyenne de ses LED.
    ///
    /// C'est ce qui rend une animation lisible au détail « ventilateur » — une
    /// LED prise au hasard clignoterait au rythme du motif, la moyenne respire.
    fn moyenne(&self, organe: Organe) -> Rgb {
        let cibles = organe.cibles();
        let compte = cibles.len() as u32;
        let (mut r, mut v, mut b) = (0u32, 0u32, 0u32);
        for cible in cibles {
            let couleur = self.couleur(cible);
            r += u32::from(couleur.r);
            v += u32::from(couleur.g);
            b += u32::from(couleur.b);
        }
        Rgb::new((r / compte) as u8, (v / compte) as u8, (b / compte) as u8)
    }
}

/// Ce que la fenêtre garde entre deux gestes.
///
/// Tout y vit dans le fil de l'interface — d'où `RefCell` et non `Mutex` : les
/// deux autres fils lui parlent par un canal, et ne partagent donc jamais un
/// emprunt.
struct Pupitre {
    tableau: RefCell<Tableau>,
    selection: RefCell<Selection>,
    /// La projection en cours. Changer de vue la reconstruit.
    plan: RefCell<Plan>,
    detail: Cell<Detail>,
    /// Les réglages d'animation tels qu'ils sont affichés.
    reglage: RefCell<Reglage>,
    /// La couleur choisie, gardée **en teinte/saturation/luminosité**.
    ///
    /// Et non en `Rgb` : un gris n'a pas de teinte et le noir n'a pas de
    /// saturation. Repasser par le `Rgb` à chaque geste perdrait la teinte dès
    /// qu'on descend un curseur à zéro, et il faudrait la retrouver au jugé.
    couleur: Cell<Tsl>,
    /// Une poignée par canal de ventilateur, jamais partagée entre canaux.
    poignees: RefCell<HashMap<String, Poignee>>,
    /// Deux minutes glissantes de relevés, pour tracer les courbes.
    historique: RefCell<Historique>,
    /// Les zones telles que le démon les rend : nom, rendu, nombre de LED.
    zones: RefCell<Vec<(String, String, usize)>>,
    /// La zone visée, s'il y en a une. Quand elle existe, la couleur et les
    /// animations lui vont **au lieu** d'aller au boîtier entier.
    visee: RefCell<Option<String>>,
    /// Le modèle des lignes de ventilateur, **gardé vivant** : le reconstruire
    /// à chaque seconde recrée les curseurs, et un curseur recréé sous les
    /// doigts perd le geste en cours.
    canaux: Rc<VecModel<LigneVentilateur>>,
    /// L'origine des temps de la fenêtre. Les poignées raisonnent en durées
    /// depuis elle, ce qui les rend testables sans horloge.
    depart: Instant,
}

impl Pupitre {
    fn nouveau() -> Pupitre {
        Pupitre {
            tableau: RefCell::new(Tableau::noir()),
            selection: RefCell::new(Selection::tout()),
            plan: RefCell::new(Plan::nouveau(&reverb_anim::Geometrie::mesuree())),
            detail: Cell::new(Detail::Led),
            reglage: RefCell::new(Reglage {
                animation: None,
                couleur: Rgb::new(0xff, 0x40, 0xff),
                vitesse: 3,
                direction: 0,
            }),
            couleur: Cell::new(Rgb::new(0xff, 0x40, 0xff).en_tsl()),
            poignees: RefCell::new(HashMap::new()),
            historique: RefCell::new(Historique::nouvel()),
            zones: RefCell::new(Vec::new()),
            visee: RefCell::new(None),
            canaux: Rc::new(VecModel::default()),
            depart: Instant::now(),
        }
    }

    fn maintenant(&self) -> Duration {
        self.depart.elapsed()
    }

    /// La couleur choisie, en composantes.
    ///
    /// Le `Tsl` gardé vient toujours d'un `en_tsl` ou d'un curseur borné : la
    /// conversion ne peut pas échouer, et le noir est le repli qui se voit.
    fn rgb(&self) -> Rgb {
        Rgb::depuis_tsl(self.couleur.get()).unwrap_or(Rgb::BLACK)
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
    let pupitre = Rc::new(Pupitre::nouveau());

    fenetre.set_familles(familles());
    fenetre.set_ventilateurs(ModelRc::from(pupitre.canaux.clone()));
    dessiner(&fenetre, &pupitre);

    let (ordres, file) = channel::<Request>();
    // Les réponses reviennent par un canal plutôt que par la boucle
    // d'événements : ce qu'elles mettent à jour vit dans le `Pupitre`, qui est
    // au fil de l'interface et ne traverse donc aucune frontière de fil.
    let (retours, arrivees) = channel::<Retour>();
    lancer_le_fil_des_ordres(socket.clone(), file, fenetre.as_weak(), retours.clone());
    lancer_le_fil_des_images(socket, fenetre.as_weak(), retours);

    brancher(&fenetre, &pupitre, ordres.clone());

    // La télémétrie n'a pas de flux : on la redemande, doucement. Une seconde
    // suffit pour des tours par minute, et n'ajoute rien de mesurable au démon.
    let horloge = slint::Timer::default();
    horloge.start(
        slint::TimerMode::Repeated,
        Duration::from_secs(1),
        move || {
            let _ = ordres.send(Request::Status);
            let _ = ordres.send(Request::ZoneList);
        },
    );

    // Les arrivées se vident plus vite que le démon ne les produit : il pousse
    // vingt images par seconde, ce qui ne doit pas s'accumuler dans le canal.
    let vidange = slint::Timer::default();
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        vidange.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(30),
            move || {
                let Some(fenetre) = faible.upgrade() else {
                    return;
                };
                let mut redessiner = false;
                while let Ok(retour) = arrivees.try_recv() {
                    redessiner |= ranger(&fenetre, &pupitre, retour);
                }
                if redessiner {
                    dessiner(&fenetre, &pupitre);
                }
            },
        );
    }

    if let Err(erreur) = fenetre.run() {
        eprintln!("erreur : {erreur}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Une réponse du démon, en route vers le fil de l'interface.
enum Retour {
    Telemetrie(Vec<ResponseLine>),
    Eclairage(Vec<ResponseLine>),
    Image(Vec<(String, Vec<Rgb>)>),
    Zones(Vec<ResponseLine>),
}

/// Le fil qui agit : il attend les réponses du démon à la place de l'interface.
fn lancer_le_fil_des_ordres(
    socket: PathBuf,
    file: std::sync::mpsc::Receiver<Request>,
    fenetre: Weak<Fenetre>,
    retours: Sender<Retour>,
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
                    repondre(&fenetre, &retours, &requete, lignes);
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
fn lancer_le_fil_des_images(socket: PathBuf, fenetre: Weak<Fenetre>, retours: Sender<Retour>) {
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
                        if retours.send(Retour::Image(cadres)).is_err() {
                            return;
                        }
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

/// Redessine la maquette et ce qui la décrit.
fn dessiner(fenetre: &Fenetre, pupitre: &Pupitre) {
    let plan = pupitre.plan.borrow();
    let tableau = pupitre.tableau.borrow();
    let selection = pupitre.selection.borrow();
    let detail = pupitre.detail.get();

    fenetre.set_leds(modele_leds(&plan, &tableau, &selection, detail));
    fenetre.set_aretes(SharedString::from(
        plan.aretes()
            .iter()
            .map(|(debut, fin)| format!("M {} {} L {} {} ", debut.x, debut.y, fin.x, fin.y))
            .collect::<String>(),
    ));
    fenetre.set_cible(SharedString::from(
        match pupitre.visee.borrow().as_deref() {
            Some(zone) => format!("la zone « {zone} »"),
            None => selection.nom(),
        },
    ));
    fenetre.set_detail_led(detail == Detail::Led);
    fenetre.set_vue_face(plan.vue() == Vue::Face);
    poser_couleur(fenetre, pupitre);
    fenetre.set_titre_maquette(SharedString::from(match plan.vue() {
        Vue::Face => "LE BOÎTIER — ARRIÈRE À GAUCHE, VU DU PANNEAU LATÉRAL",
        Vue::Isometrique => "LE BOÎTIER — DE TROIS-QUARTS, AUX POSITIONS RÉELLES",
    }));
}

/// Bouge un axe de la couleur choisie, puis réécrit les quatre lectures.
fn bouger_un_axe(faible: &Weak<Fenetre>, pupitre: &Pupitre, changer: impl FnOnce(&mut Tsl)) {
    let Some(fenetre) = faible.upgrade() else {
        return;
    };
    let mut tsl = pupitre.couleur.get();
    changer(&mut tsl);
    pupitre.couleur.set(tsl);
    poser_couleur(&fenetre, pupitre);
}

/// Écrit dans la fenêtre les quatre façons de lire la couleur choisie.
fn poser_couleur(fenetre: &Fenetre, pupitre: &Pupitre) {
    let tsl = pupitre.couleur.get();
    let rgb = pupitre.rgb();
    fenetre.set_teinte(tsl.teinte);
    fenetre.set_saturation(tsl.saturation);
    fenetre.set_luminosite(tsl.luminosite);
    fenetre.set_couleur(SharedString::from(format!(
        "{:02x}{:02x}{:02x}",
        rgb.r, rgb.g, rgb.b
    )));
    fenetre.set_apercu(en_slint(rgb));
    // Les deux extrémités des voies « saturation » et « luminosité » : elles
    // dépendent de la teinte courante, donc elles se recalculent avec elle.
    let pure = Tsl {
        teinte: tsl.teinte,
        saturation: 100.0,
        luminosite: 100.0,
    };
    let grise = Tsl {
        teinte: tsl.teinte,
        saturation: 0.0,
        luminosite: tsl.luminosite,
    };
    fenetre.set_teinte_pure(en_slint(Rgb::depuis_tsl(pure).unwrap_or(Rgb::BLACK)));
    fenetre.set_teinte_grise(en_slint(Rgb::depuis_tsl(grise).unwrap_or(Rgb::BLACK)));
}

/// La même animation, adressée à une zone au lieu du boîtier entier.
///
/// Le protocole a deux verbes pour un seul geste ; les réglages, eux, sont les
/// mêmes. Les recopier ici évite de tenir deux chemins d'accord.
fn vers_la_zone(nom: String, requete: Request) -> Request {
    match requete {
        Request::Animate { name, reglages } => Request::ZoneAnim {
            nom,
            animation: name,
            reglages,
        },
        autre => autre,
    }
}

/// Écrit la liste des zones dans la fenêtre.
fn poser_zones(fenetre: &Fenetre, pupitre: &Pupitre) {
    let visee = pupitre.visee.borrow();
    let lignes: Vec<LigneZone> = pupitre
        .zones
        .borrow()
        .iter()
        .map(|(nom, rendu, combien)| LigneZone {
            nom: SharedString::from(nom.clone()),
            rendu: SharedString::from(rendu.clone()),
            combien: i32::try_from(*combien).unwrap_or(i32::MAX),
            visee: visee.as_deref() == Some(nom.as_str()),
        })
        .collect();
    fenetre.set_zones(ModelRc::new(VecModel::from(lignes)));
}

/// Les LED de la sélection, en cibles du protocole.
fn cibles(selection: &Selection) -> Vec<Led> {
    selection
        .cibles
        .iter()
        .map(|cible| match cible {
            Cible::Led { position, led } => Led::Ventilateur {
                position: *position,
                led: *led,
            },
            Cible::Barrette { slot, led } => Led::Barrette {
                slot: *slot,
                led: *led,
            },
        })
        .collect()
}

fn modele_leds(
    plan: &Plan,
    tableau: &Tableau,
    selection: &Selection,
    detail: Detail,
) -> ModelRc<PointLed> {
    let rayon = plan.rayon_anneau();
    let mut points = Vec::with_capacity(124);
    match detail {
        Detail::Led => {
            for cible in Selection::tout().cibles {
                let place = match cible {
                    Cible::Led { position, led } => plan.led_ventilateur(position, led),
                    Cible::Barrette { slot, led } => plan.led_barrette(slot, led),
                };
                let choisie = selection.contient(cible);
                points.push(PointLed {
                    x: place.map_or(0.0, |place| place.x),
                    y: place.map_or(0.0, |place| place.y),
                    // Le huitième du rayon de l'anneau, soit la moitié du
                    // diamètre que la projection s'est donné. Une pastille visée
                    // grossit : sur cent vingt-quatre points, une nuance de
                    // bordure ne se verrait pas.
                    rayon: rayon / 8.0 * if choisie { 1.7 } else { 1.0 },
                    hauteur: 0.0,
                    couleur: en_slint(tableau.couleur(cible)),
                    choisie,
                });
            }
        }
        Detail::Ventilateur => {
            for position in Position::ALL {
                let organe = Organe::Ventilateur(position);
                let centre = plan.centre_ventilateur(position);
                points.push(PointLed {
                    x: centre.x,
                    y: centre.y,
                    rayon,
                    hauteur: 0.0,
                    couleur: en_slint(tableau.moyenne(organe)),
                    choisie: selection.est_entier(organe),
                });
            }
            for slot in 0..SLOT_COUNT {
                let organe = Organe::Reglette(slot);
                let Some(centre) = plan.centre_barrette(slot) else {
                    continue;
                };
                // La gélule couvre la réglette d'un bout à l'autre : c'est ce
                // qui la distingue d'un ventilateur au premier coup d'œil.
                let hauteur = match (
                    plan.led_barrette(slot, 0),
                    plan.led_barrette(slot, LEDS_PER_STICK - 1),
                ) {
                    (Some(bas), Some(haut)) => (haut.y - bas.y).abs(),
                    _ => 0.0,
                };
                points.push(PointLed {
                    x: centre.x,
                    y: centre.y,
                    rayon: rayon / 3.0,
                    hauteur,
                    couleur: en_slint(tableau.moyenne(organe)),
                    choisie: selection.est_entier(organe),
                });
            }
        }
    }
    ModelRc::new(VecModel::from(points))
}

/// En deçà de cette fraction du cadre, un glissement est un clic.
///
/// Aucune souris ne reste immobile pendant un clic : sans ce seuil, tout clic
/// serait un rectangle minuscule qui n'attrape rien, et la maquette paraîtrait
/// morte.
const SEUIL_GLISSEMENT: f32 = 0.012;

fn brancher(fenetre: &Fenetre, pupitre: &Rc<Pupitre>, ordres: Sender<Request>) {
    // ── Le geste sur la maquette : clic ou rectangle ───────────────────────
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        fenetre.on_geste_maquette(move |x1, y1, x2, y2, ajout| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            let plan = pupitre.plan.borrow();
            let detail = pupitre.detail.get();
            let debut = Place { x: x1, y: y1 };
            let fin = Place { x: x2, y: y2 };

            let attrapees =
                if (x2 - x1).abs() < SEUIL_GLISSEMENT && (y2 - y1).abs() < SEUIL_GLISSEMENT {
                    plan.sous(fin).into_iter().collect()
                } else {
                    plan.dans(debut, fin)
                };
            // Au détail « ventilateur », toucher une LED, c'est toucher son
            // organe : c'est là toute la différence entre les deux niveaux.
            let attrapees = match detail {
                Detail::Led => attrapees,
                Detail::Ventilateur => {
                    let mut tout = Vec::new();
                    for cible in attrapees {
                        for compagne in plan.groupe(cible) {
                            if !tout.contains(&compagne) {
                                tout.push(compagne);
                            }
                        }
                    }
                    tout
                }
            };
            drop(plan);

            let mut selection = pupitre.selection.borrow_mut();
            if ajout {
                selection.ajouter(attrapees);
            } else {
                // Un rectangle qui ne touche rien **vide** la sélection : c'est
                // le seul geste par lequel on peut ne plus rien viser.
                *selection = Selection { cibles: attrapees };
            }
            drop(selection);
            dessiner(&fenetre, &pupitre);
        });
    }

    // ── Les deux bascules ──────────────────────────────────────────────────
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        fenetre.on_basculer_detail(move || {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            pupitre.detail.set(match pupitre.detail.get() {
                Detail::Led => Detail::Ventilateur,
                Detail::Ventilateur => Detail::Led,
            });
            dessiner(&fenetre, &pupitre);
        });
    }
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        fenetre.on_basculer_vue(move || {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            // La sélection ne bouge pas : une `Cible` ne porte pas de vue, elle
            // désigne une LED du boîtier.
            let geometrie = reverb_anim::Geometrie::mesuree();
            let suivante = match pupitre.plan.borrow().vue() {
                Vue::Face => Plan::isometrique(&geometrie),
                Vue::Isometrique => Plan::nouveau(&geometrie),
            };
            *pupitre.plan.borrow_mut() = suivante;
            dessiner(&fenetre, &pupitre);
        });
    }

    // ── Choisir une famille ────────────────────────────────────────────────
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        fenetre.on_choisir_cible(move |nom| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            *pupitre.selection.borrow_mut() = match nom.as_str() {
                "fans" => Selection::famille(true),
                "ram" => Selection::famille(false),
                _ => Selection::tout(),
            };
            dessiner(&fenetre, &pupitre);
        });
    }

    // ── Les zones ──────────────────────────────────────────────────────────
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_creer_zone(move |nom| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            let nom = nom.trim().to_owned();
            if nom.is_empty() {
                fenetre.set_message(SharedString::from("une zone a besoin d'un nom"));
                return;
            }
            let cibles = cibles(&pupitre.selection.borrow());
            if cibles.is_empty() {
                fenetre.set_message(SharedString::from(
                    "sélectionne d'abord des LED sur la maquette",
                ));
                return;
            }
            let _ = envoi.send(Request::ZoneSet {
                nom: nom.clone(),
                cibles,
            });
            // Viser la zone qu'on vient de créer : c'est ce qu'on veut régler
            // dans la seconde qui suit.
            *pupitre.visee.borrow_mut() = Some(nom);
            fenetre.set_nouvelle_zone(SharedString::new());
            let _ = envoi.send(Request::ZoneList);
        });
    }
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_viser_zone(move |nom| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            // Un second clic dévise : sans quoi on ne pourrait plus revenir au
            // boîtier entier sans supprimer la zone.
            let mut visee = pupitre.visee.borrow_mut();
            *visee = if visee.as_deref() == Some(nom.as_str()) {
                None
            } else {
                Some(nom.to_string())
            };
            drop(visee);
            poser_zones(&fenetre, &pupitre);
            dessiner(&fenetre, &pupitre);
            let _ = envoi.send(Request::ZoneList);
        });
    }
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        fenetre.on_supprimer_zone(move |nom| {
            if pupitre.visee.borrow().as_deref() == Some(nom.as_str()) {
                *pupitre.visee.borrow_mut() = None;
            }
            let _ = envoi.send(Request::ZoneDrop {
                nom: nom.to_string(),
            });
            let _ = envoi.send(Request::ZoneList);
        });
    }

    // ── Le sélecteur de couleur ────────────────────────────────────────────
    //
    // Les trois curseurs et le champ hexadécimal désignent la même couleur, et
    // c'est le `Tsl` du pupitre qui l'est : le champ le reformule, les curseurs
    // en bougent un axe. Une saisie fautive ne change **rien** — ni l'aperçu ni
    // les curseurs — et le dit.
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        fenetre.on_couleur_saisie(move |texte| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            let Some(couleur) = lire_couleur(&texte) else {
                return;
            };
            let ancienne = pupitre.couleur.get();
            let mut tsl = couleur.en_tsl();
            // Un gris n'a pas de teinte, le noir pas de saturation : garder
            // celles d'avant plutôt que de les remettre à zéro sous les doigts.
            if tsl.saturation == 0.0 {
                tsl.teinte = ancienne.teinte;
            }
            if tsl.luminosite == 0.0 {
                tsl.saturation = ancienne.saturation;
                tsl.teinte = ancienne.teinte;
            }
            pupitre.couleur.set(tsl);
            poser_couleur(&fenetre, &pupitre);
        });
    }
    // Un axe bougé, les trois autres lectures suivent. Le `clamp` n'est pas une
    // précaution de style : la teinte s'arrête à 359, 360 étant le même point
    // que 0 et refusé par `depuis_tsl`.
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        fenetre.on_teinte_changee(move |valeur| {
            bouger_un_axe(&faible, &pupitre, |tsl| {
                tsl.teinte = valeur.clamp(0.0, 359.0);
            });
        });
    }
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        fenetre.on_saturation_changee(move |valeur| {
            bouger_un_axe(&faible, &pupitre, |tsl| {
                tsl.saturation = valeur.clamp(0.0, 100.0);
            });
        });
    }
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        fenetre.on_luminosite_changee(move |valeur| {
            bouger_un_axe(&faible, &pupitre, |tsl| {
                tsl.luminosite = valeur.clamp(0.0, 100.0);
            });
        });
    }

    // ── Appliquer la couleur ───────────────────────────────────────────────
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        fenetre.on_appliquer_couleur(move || {
            let couleur = pupitre.rgb();
            // Une zone visée reçoit la couleur **à la place** du boîtier : c'est
            // tout l'intérêt d'en avoir une.
            if let Some(nom) = pupitre.visee.borrow().clone() {
                let _ = envoi.send(Request::ZoneLight { nom, couleur });
                let _ = envoi.send(Request::ZoneList);
                return;
            }
            for requete in commandes_de_couleur(
                &pupitre.tableau.borrow(),
                &pupitre.selection.borrow(),
                couleur,
            ) {
                let _ = envoi.send(requete);
            }
        });
    }

    // ── Les animations ─────────────────────────────────────────────────────
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_lancer_animation(move |nom| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            if Animation::par_nom(&nom).is_err() {
                return;
            }
            let mut reglage = pupitre.reglage.borrow_mut();
            reglage.animation = Some(nom.to_string());
            relever(&fenetre, &pupitre, &mut reglage);
            let Some(requete) = reglage.commande() else {
                return;
            };
            if let Some(zone) = pupitre.visee.borrow().clone() {
                let _ = envoi.send(vers_la_zone(zone, requete));
                let _ = envoi.send(Request::ZoneList);
                return;
            }
            fenetre.set_animation_courante(nom.clone());
            let _ = envoi.send(requete);
        });
    }
    // ── Un réglage change pendant que l'animation tourne ───────────────────
    //
    // C'est le rappel qui manquait : le curseur de vitesse et le menu de
    // direction n'écrivaient qu'une propriété de l'interface, et le démon
    // gardait la vitesse d'avant jusqu'au prochain clic sur le bouton de
    // l'animation. Le curseur bougeait pour rien.
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_regler_animation(move || {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            let mut reglage = pupitre.reglage.borrow_mut();
            relever(&fenetre, &pupitre, &mut reglage);
            // `None` quand rien ne tourne : bouger la vitesse à vide ne doit
            // pas démarrer une animation que personne n'a demandée.
            if let Some(requete) = reglage.commande() {
                match pupitre.visee.borrow().clone() {
                    Some(zone) => {
                        let _ = envoi.send(vers_la_zone(zone, requete));
                    }
                    None => {
                        let _ = envoi.send(requete);
                    }
                }
            }
        });
    }
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_arreter_animation(move || {
            if let Some(zone) = pupitre.visee.borrow().clone() {
                let _ = envoi.send(Request::ZoneAnim {
                    nom: zone,
                    animation: None,
                    reglages: Vec::new(),
                });
                let _ = envoi.send(Request::ZoneList);
                return;
            }
            if let Some(fenetre) = faible.upgrade() {
                fenetre.set_animation_courante(SharedString::from("aucune"));
            }
            pupitre.reglage.borrow_mut().animation = None;
            let _ = envoi.send(Request::Animate {
                name: None,
                reglages: Vec::new(),
            });
        });
    }

    // ── Les ventilateurs ───────────────────────────────────────────────────
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        fenetre.on_regler_ventilateur(move |canal, consigne| {
            let maintenant = pupitre.maintenant();
            let mut poignees = pupitre.poignees.borrow_mut();
            let poignee = poignees
                .entry(canal.to_string())
                .or_insert_with(Poignee::nouvelle);
            poignee.saisir(
                u8::try_from(consigne.clamp(0, 100)).unwrap_or(0),
                maintenant,
            );
            // Une commande par pas franchi, pas une par image : c'est la
            // poignée qui le décide, pas la cadence de la souris.
            if let Some(consigne) = poignee.a_envoyer() {
                let _ = envoi.send(Request::Fan {
                    channel: canal.to_string(),
                    action: FanAction::Pwm(consigne),
                });
            }
        });
    }
    {
        let pupitre = pupitre.clone();
        fenetre.on_relacher_ventilateur(move |canal| {
            let maintenant = pupitre.maintenant();
            pupitre
                .poignees
                .borrow_mut()
                .entry(canal.to_string())
                .or_insert_with(Poignee::nouvelle)
                .relacher(maintenant);
        });
    }
    {
        let pupitre = pupitre.clone();
        let envoi = ordres;
        fenetre.on_rendre_au_firmware(move |canal| {
            pupitre
                .poignees
                .borrow_mut()
                .entry(canal.to_string())
                .or_insert_with(Poignee::nouvelle)
                .liberer();
            let _ = envoi.send(Request::Fan {
                channel: canal.to_string(),
                action: FanAction::Auto,
            });
        });
    }
}

/// Recopie dans les réglages ce que la fenêtre affiche de la couleur, de la
/// vitesse et de la direction.
fn relever(fenetre: &Fenetre, pupitre: &Pupitre, reglage: &mut Reglage) {
    reglage.couleur = pupitre.rgb();
    reglage.vitesse = u8::try_from(fenetre.get_vitesse().clamp(0, 255)).unwrap_or(3);
    reglage.direction = usize::try_from(fenetre.get_direction()).unwrap_or(0);
}

/// Les commandes qui donnent cette couleur à la sélection, organe par organe.
///
/// ⚠️ **`light` quand l'organe est entier, `paint` sinon.** Ce n'est pas une
/// économie de trafic : `light` est ce que le démon **conserve** (#21), `paint`
/// non. Peindre les huit LED d'un ventilateur une par une donnerait le même
/// éclairage et ne survivrait pas au redémarrage.
fn commandes_de_couleur(tableau: &Tableau, selection: &Selection, couleur: Rgb) -> Vec<Request> {
    if *selection == Selection::tout() {
        return vec![Request::Light {
            target: LightTarget::All,
            color: couleur,
        }];
    }
    selection
        .organes()
        .into_iter()
        .map(|organe| {
            if selection.est_entier(organe) {
                Request::Light {
                    target: organe.groupe(),
                    color: couleur,
                }
            } else {
                // Les couleurs affichées font foi pour le reste de l'organe :
                // ce sont celles que le démon vient d'envoyer au matériel.
                let couleurs = organe
                    .cibles()
                    .into_iter()
                    .map(|cible| {
                        if selection.contient(cible) {
                            couleur
                        } else {
                            tableau.couleur(cible)
                        }
                    })
                    .collect();
                Request::Paint {
                    target: organe.groupe(),
                    couleurs,
                }
            }
        })
        .collect()
}

/// Range une réponse du démon : soit dans la fenêtre, soit dans le canal de
/// retour quand elle touche à ce que le `Pupitre` garde.
fn repondre(
    fenetre: &Weak<Fenetre>,
    retours: &Sender<Retour>,
    requete: &Request,
    lignes: Vec<ResponseLine>,
) {
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
            let _ = retours.send(Retour::Telemetrie(lignes));
        }
        Request::Lighting => {
            let _ = retours.send(Retour::Eclairage(lignes));
        }
        Request::ZoneList => {
            let _ = retours.send(Retour::Zones(lignes));
        }
        _ => {}
    }
}

/// Applique une arrivée. Rend vrai s'il faut redessiner la maquette.
fn ranger(fenetre: &Fenetre, pupitre: &Pupitre, retour: Retour) -> bool {
    match retour {
        Retour::Telemetrie(lignes) => {
            poser_telemetrie(fenetre, pupitre, &lignes);
            false
        }
        Retour::Eclairage(lignes) => {
            let mut anime = None;
            for ligne in &lignes {
                if let ResponseLine::Anim { nom, .. } = ligne {
                    anime = Some(nom.clone());
                }
            }
            fenetre.set_animation_courante(SharedString::from(
                anime.clone().unwrap_or_else(|| "aucune".to_owned()),
            ));
            pupitre.reglage.borrow_mut().animation = anime;
            false
        }
        Retour::Zones(lignes) => {
            // Une zone peut tenir sur plusieurs lignes `zone` : le démon les
            // découpe pour rester sous la longueur maximale d'une ligne, et
            // c'est ici qu'on les recolle.
            let mut vues: Vec<(String, String, usize)> = Vec::new();
            for ligne in &lignes {
                match ligne {
                    ResponseLine::Zone { nom, cibles } => {
                        match vues.iter_mut().find(|(deja, _, _)| deja == nom) {
                            Some((_, _, combien)) => *combien += cibles.len(),
                            None => {
                                vues.push((nom.clone(), "transparente".to_owned(), cibles.len()))
                            }
                        }
                    }
                    ResponseLine::ZoneLight { nom, couleur } => {
                        if let Some((_, rendu, _)) =
                            vues.iter_mut().find(|(deja, _, _)| deja == nom)
                        {
                            *rendu =
                                format!("#{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b);
                        }
                    }
                    ResponseLine::ZoneAnim { nom, animation, .. } => {
                        if let Some((_, rendu, _)) =
                            vues.iter_mut().find(|(deja, _, _)| deja == nom)
                        {
                            *rendu = animation.clone();
                        }
                    }
                    _ => {}
                }
            }
            // Une zone visée qui vient de disparaître ne doit pas continuer de
            // recevoir les couleurs.
            let mut visee = pupitre.visee.borrow_mut();
            if let Some(nom) = visee.clone()
                && !vues.iter().any(|(deja, _, _)| *deja == nom)
            {
                *visee = None;
            }
            drop(visee);
            *pupitre.zones.borrow_mut() = vues;
            poser_zones(fenetre, pupitre);
            false
        }
        Retour::Image(cadres) => {
            let mut tableau = pupitre.tableau.borrow_mut();
            for (cible, couleurs) in &cadres {
                tableau.poser(cible, couleurs);
            }
            true
        }
    }
}

fn poser_telemetrie(fenetre: &Fenetre, pupitre: &Pupitre, lignes: &[ResponseLine]) {
    let mut canaux = Vec::new();
    let mut temperatures = Vec::new();
    let mut releves: Vec<(String, Releve)> = Vec::new();
    let maintenant = pupitre.maintenant();
    let mut poignees = pupitre.poignees.borrow_mut();
    for ligne in lignes {
        match ligne {
            ResponseLine::Channel {
                channel,
                position,
                rpm,
                pwm,
                mode,
            } => {
                // La mesure passe par la poignée du canal : c'est elle qui
                // décide si elle a le droit de déplacer le curseur, ou si une
                // consigne encore fraîche le tient.
                let poignee = poignees
                    .entry(channel.clone())
                    .or_insert_with(Poignee::nouvelle);
                poignee.mesurer(pwm.unwrap_or(0), maintenant);
                canaux.push(LigneVentilateur {
                    canal: SharedString::from(channel.clone()),
                    position: SharedString::from(
                        position.map_or_else(String::new, |position| position.name().to_owned()),
                    ),
                    rpm: SharedString::from(
                        rpm.map_or_else(|| "—".to_owned(), |tours| tours.to_string()),
                    ),
                    pwm: i32::from(poignee.affichee()),
                    mode: SharedString::from(mode.clone()),
                    lisible: rpm.is_some(),
                });
            }
            ResponseLine::Temp {
                sensor,
                millidegrees,
            } => releves.push((sensor.clone(), Releve::Valeur(*millidegrees))),
            ResponseLine::Unreadable { subject, reason } => {
                // Une sonde débranchée le dit, et sa courbe garde la trace du
                // trou : figer sa dernière valeur ferait croire qu'on la lit
                // encore.
                releves.push((subject.clone(), Releve::Illisible));
                let _ = reason;
            }
            _ => {}
        }
    }

    {
        let mut historique = pupitre.historique.borrow_mut();
        for (sonde, releve) in releves {
            historique.noter(&sonde, releve);
        }
        for sonde in historique.sondes() {
            let lisible = matches!(historique.dernier(&sonde), Some(Releve::Valeur(_)));
            // Découpé par le DERNIER deux-points : un nom de `hwmon` en
            // contient — `r8169_0_e00:00` — et couper au premier ferait passer
            // la moitié de l'origine pour un libellé.
            let (origine, capteur) = sonde
                .rsplit_once(':')
                .map_or((sonde.as_str(), ""), |(origine, reste)| (origine, reste));
            temperatures.push(LigneTemperature {
                origine: SharedString::from(origine),
                capteur: SharedString::from(if capteur.is_empty() { origine } else { capteur }),
                valeur: SharedString::from(match historique.dernier(&sonde) {
                    Some(Releve::Valeur(millidegres)) => {
                        format!("{:.1} °C", f64::from(millidegres) / 1000.0)
                    }
                    _ => "illisible".to_owned(),
                }),
                courbe: SharedString::from(courbe(&historique, &sonde)),
                lisible,
            });
        }
    }

    // ⚠️ **Les lignes se modifient en place.** Remplacer le modèle recrée les
    // curseurs à chaque seconde, et un curseur recréé sous les doigts perd le
    // geste en cours : c'est la moitié de la barre « un peu buggée » que Nico a
    // signalée. On ne réécrit que les lignes qui ont vraiment changé.
    if pupitre.canaux.row_count() == canaux.len() {
        for (rang, ligne) in canaux.into_iter().enumerate() {
            if pupitre.canaux.row_data(rang).as_ref() != Some(&ligne) {
                pupitre.canaux.set_row_data(rang, ligne);
            }
        }
    } else {
        pupitre.canaux.set_vec(canaux);
    }
    fenetre.set_temperatures(ModelRc::new(VecModel::from(temperatures)));
}

/// La courbe d'une sonde, en commandes SVG dans le carré unité.
///
/// Vide tant qu'il n'y a qu'un relevé : un point n'est pas une courbe, et le
/// tracer en ferait un trait horizontal qui suggère une mesure stable qu'on n'a
/// pas encore faite.
///
/// Les relevés `Illisible` **coupent** le trait au lieu d'être sautés : une
/// ligne qui enjambe le trou dirait qu'on a mesuré pendant qu'on ne mesurait
/// pas.
fn courbe(historique: &Historique, sonde: &str) -> String {
    let releves = historique.courbe(sonde);
    if releves.len() < 2 {
        return String::new();
    }
    let Some((bas, haut)) = historique.bornes(sonde) else {
        return String::new();
    };
    // Une courbe plate se dessine au milieu : la mettre en haut ou en bas ferait
    // croire à un extrême.
    let etendue = f64::from(haut - bas);
    let dernier = (releves.len() - 1) as f64;
    let mut commandes = String::new();
    let mut pose = false;
    for (rang, releve) in releves.iter().enumerate() {
        let Releve::Valeur(valeur) = releve else {
            pose = false;
            continue;
        };
        let x = rang as f64 / dernier;
        let y = if etendue > 0.0 {
            1.0 - f64::from(valeur - bas) / etendue
        } else {
            0.5
        };
        // Une marge d'un vingtième en haut et en bas : un trait collé au bord se
        // fait rogner par l'épaisseur du trait.
        let y = 0.05 + y * 0.9;
        commandes.push_str(&format!("{} {x:.4} {y:.4} ", if pose { "L" } else { "M" }));
        pose = true;
    }
    commandes
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

fn en_slint(couleur: Rgb) -> Color {
    Color::from_rgb_u8(couleur.r, couleur.g, couleur.b)
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
