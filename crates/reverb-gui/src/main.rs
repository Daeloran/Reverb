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
use reverb_gui::plan::{
    Cible, Place, Plan, Vue, halo, nom_de_zone, places_des_ancres, rayon_du_disque,
};
use reverb_gui::reglages::{
    AFFICHAGES, ChoixDeComposition, ChoixDeProfil, EcranChoisi, Limiteur, Poignee, Reglage,
    consigne_affichee, directions_offertes, eclairage_lu, requete_d_animation,
    requete_de_composition, requete_de_profil, requetes_pour_la_couleur,
};
use reverb_gui::sondes::{
    Historique, ModelesNvme, Releve, SondeRetenue, modeles_nvme, sondes_retenues,
};
use reverb_gui::telemetrie::{LigneCanal, Tri};
use reverb_gui::{
    AncreEcran, FamilleAnimation, Fenetre, LigneProfil, LigneTemperature, LigneVentilateur,
    LigneZone, PointHalo, PointLed,
};
use reverb_proto::composition::{Ancre, Fond, Source};
use reverb_proto::ipc::{FanAction, LightTarget, Request, ResponseLine, ScreenAction};
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
    (
        "rotation",
        "Chaque anneau tourne sur lui-même, à sa place dans le boîtier. Elle suit l'angle relevé \
         de chaque ventilateur, jamais le numéro de LED — sans quoi le motif tournerait à \
         l'envers sur les trois du plafond, montés antihoraire.",
    ),
    (
        "thermique",
        "La couleur suit une sonde : du bleu au vert, à l'orange, au rouge entre 25 et 60 °C. \
         Une sonde qui ne répond plus fait pulser le boîtier en blanc — aucune température n'est \
         achromatique, et aucune ne pulse.",
    ),
    (
        "pouls",
        "Une onde sphérique naît au bloc-pompe et se propage. Deux LED à égale distance de lui \
         s'allument ensemble, quels que soient leur organe et leur axe.",
    ),
    (
        "scintillement",
        "Des LED s'allument au hasard, chacune à sa cadence et à sa phase propres. La seule \
         famille sans période — et sans horloge : le rendu est tiré d'un hachage du numéro de \
         LED, donc identique ici et dans le démon.",
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

    /// Le nom de la zone que cette sélection produit quand on la colore.
    ///
    /// ⚠️ **Ce n'est pas [`Selection::nom`], et les confondre était un défaut.**
    /// La règle vit dans `plan.rs`, où elle se teste ; voir
    /// [`reverb_gui::plan::nom_de_zone`] pour ce qu'elle corrige.
    fn nom_de_zone(&self) -> String {
        nom_de_zone(&self.cibles)
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
    /// Le tri de la télémétrie, qui se souvient des canaux déjà vus (#100).
    tri: RefCell<Tri>,
    /// Les lignes de canaux du dernier tour, telles que le tri les a rendues.
    ///
    /// C'est d'elles que les poignées tirent leur requête : une commande vers un
    /// canal en quarantaine ne part pas, et [`LigneCanal::commande`] est le seul
    /// endroit qui le décide.
    canaux_lus: RefCell<Vec<LigneCanal>>,
    /// L'origine des temps de la fenêtre. Les poignées raisonnent en durées
    /// depuis elle, ce qui les rend testables sans horloge.
    depart: Instant,
    /// L'affichage d'écran que la fenêtre montre, et la poignée qui protège
    /// ce que l'utilisateur est en train de composer (#48).
    ecran: RefCell<EcranChoisi>,
    /// Ce qui empêche une jauge traînée d'inonder le démon (#47).
    limiteur: RefCell<Limiteur>,
    /// Le modèle des deux disques, lu **une seule fois** au démarrage.
    ///
    /// Un modèle ne change pas en cours de session : le relire à chaque seconde
    /// ferait une ouverture de fichier par tour d'horloge pour une constante.
    modeles_nvme: ModelesNvme,
    /// Les sondes que le panneau offre, dans l'ordre où il les montre.
    ///
    /// C'est la table de conversion des menus : l'utilisateur choisit un **rang**
    /// — « Liquide » —, et c'est ici qu'on retrouve le `slug` que le socket
    /// attend. Le slug ne remonte jamais jusqu'à l'interface, ce que le cadran
    /// imposait encore et que l'issue nomme comme un défaut.
    sondes: RefCell<Vec<SondeRetenue>>,
    /// Les ambiances enregistrées, telles que `profil list` les rend (#74).
    profils: RefCell<Vec<String>>,
    /// Celle que la fenêtre vient de rappeler, s'il y en a une.
    ///
    /// ⚠️ **« Rappelée », et non « active ».** Le protocole ne dit pas quel
    /// profil est actif — `ResponseLine::Profil` ne survit pas à la réponse qui
    /// la porte, et `status` n'en garde aucune trace. C'est donc une mémoire de
    /// fenêtre, et elle s'efface dès qu'une commande change l'éclairage : dire
    /// « actif » d'un profil dont on vient de changer la couleur serait faux, et
    /// c'est exactement ce qu'on regarderait pour savoir où on en est.
    rappele: RefCell<Option<String>>,
    /// Ce que la dalle compose, tel que le démon vient de le décrire (#80).
    ///
    /// Le fond en clair — « noir » ou « image <chemin> » —, puis un champ par
    /// ancre garnie. La fenêtre ne recompose rien : elle range ce qu'on lui dit.
    composition: RefCell<(String, Vec<(Ancre, String)>)>,
    /// L'ancre que le panneau de composition édite.
    ancre_visee: Cell<Ancre>,
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
            tri: RefCell::new(Tri::nouveau()),
            canaux_lus: RefCell::new(Vec::new()),
            depart: Instant::now(),
            ecran: RefCell::new(EcranChoisi::default()),
            limiteur: RefCell::new(Limiteur::nouveau()),
            modeles_nvme: modeles_nvme(),
            sondes: RefCell::new(Vec::new()),
            profils: RefCell::new(Vec::new()),
            rappele: RefCell::new(None),
            composition: RefCell::new((String::new(), Vec::new())),
            ancre_visee: Cell::new(Ancre::Haut),
        }
    }

    /// Le `slug` de la sonde retenue à ce rang, s'il y en a une.
    ///
    /// `None` tant que la première télémétrie n'est pas arrivée : le menu est
    /// alors vide, et inventer un slug enverrait au démon une sonde qu'il
    /// refuserait en donnant la liste — un message juste, pour une faute qui
    /// n'est pas celle de l'utilisateur.
    fn sonde_au_rang(&self, rang: i32) -> Option<String> {
        let rang = usize::try_from(rang).ok()?;
        self.sondes
            .borrow()
            .get(rang)
            .map(|retenue| retenue.slug.clone())
    }

    /// La requête qu'une consigne produit pour ce canal, s'il en accepte une.
    ///
    /// ⚠️ **`None` quand le canal est en quarantaine** (#100) : on ne tire pas
    /// une poignée vers un canal qui ne répond pas. `None` aussi quand aucun
    /// tour de télémétrie ne l'a encore nommé — la liste est alors vide, et il
    /// n'y a rien à régler.
    fn commande_du_canal(&self, canal: &str, action: FanAction) -> Option<Request> {
        self.canaux_lus
            .borrow()
            .iter()
            .find(|ligne| ligne.canal == canal)?
            .commande(action)
    }

    /// La mémoire du profil rappelé s'efface : l'éclairage vient de changer.
    ///
    /// Appelé par **tout** ce qui repeint — couleur, animation, zone. Sans cela,
    /// la pastille resterait allumée sur une ambiance qu'on ne voit plus.
    fn oublier_le_rappel(&self) {
        self.rappele.borrow_mut().take();
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
    fenetre.set_animations(noms_du_menu());
    fenetre.set_animations_lisibles(noms_lisibles_du_menu());
    fenetre.set_directions(noms_des_directions());
    fenetre.set_affichages(noms_des_affichages());
    fenetre.set_affichages_lisibles(noms_lisibles_des_affichages());
    fenetre.set_rayon_disque(rayon_du_disque());
    fenetre.set_champs_max(
        i32::try_from(reverb_proto::composition::Composition::CHAMPS_MAX).unwrap_or(4),
    );
    fenetre.set_ventilateurs(ModelRc::from(pupitre.canaux.clone()));
    poser_ancres(&fenetre, &pupitre);
    dessiner(&fenetre, &pupitre);

    let (ordres, file) = channel::<Request>();
    // Les réponses reviennent par un canal plutôt que par la boucle
    // d'événements : ce qu'elles mettent à jour vit dans le `Pupitre`, qui est
    // au fil de l'interface et ne traverse donc aucune frontière de fil.
    let (retours, arrivees) = channel::<Retour>();
    lancer_le_fil_des_ordres(socket.clone(), file, fenetre.as_weak(), retours.clone());
    lancer_le_fil_des_images(socket, fenetre.as_weak(), retours);

    brancher(&fenetre, &pupitre, ordres.clone());

    // **Avant tout le reste** : ce que le boîtier fait déjà. Le démon rétablit
    // l'éclairage au démarrage, si bien qu'une animation tourne le plus souvent
    // avant que cette fenêtre existe. Sans cette question, la fenêtre croirait
    // piloter un boîtier éteint et son curseur de vitesse resterait muet (#41).
    let _ = ordres.send(Request::Lighting);
    let _ = ordres.send(Request::Screen(ScreenAction::State));
    // Les ambiances, une fois : la liste ne change que sur un `save` ou un
    // `drop`, tous deux passés par cette fenêtre, qui la redemande alors.
    let _ = ordres.send(Request::Profil(reverb_proto::ipc::ProfilAction::List));

    // La télémétrie n'a pas de flux : on la redemande, doucement. Une seconde
    // suffit pour des tours par minute, et n'ajoute rien de mesurable au démon.
    let horloge = slint::Timer::default();
    {
        let ordres = ordres.clone();
        horloge.start(
            slint::TimerMode::Repeated,
            Duration::from_secs(1),
            move || {
                let _ = ordres.send(Request::Status);
                let _ = ordres.send(Request::ZoneList);
                // `lighting` aussi : une animation lancée ailleurs — par le socket,
                // par une seconde fenêtre — doit finir par se voir ici. C'est
                // `Reglage::adopter` qui décide de ce qu'on en retient, et qui
                // laisse en place ce que l'utilisateur est en train de régler.
                let _ = ordres.send(Request::Lighting);
                // L'écran a sa vie propre : une commande passée par le socket, ou
                // un cadran qui a changé de sonde, doit finir par se voir ici.
                let _ = ordres.send(Request::Screen(ScreenAction::State));
            },
        );
    }

    // Les arrivées se vident plus vite que le démon ne les produit : il pousse
    // vingt images par seconde, ce qui ne doit pas s'accumuler dans le canal.
    let vidange = slint::Timer::default();
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        let envoi = ordres.clone();
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
                // La dernière couleur d'une rafale de jauge part ici, et non à
                // l'horloge d'une seconde : trente millisecondes après le doigt
                // relevé, le boîtier montre la couleur choisie. C'est ce qui
                // rend le limiteur invisible à l'usage (#47).
                let maintenant = pupitre.maintenant();
                let restante = pupitre.limiteur.borrow_mut().a_envoyer(maintenant);
                if let Some(couleur) = restante {
                    appliquer_la_couleur(&pupitre, &envoi, couleur);
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
    Ecran(Vec<ResponseLine>),
    Profils(Vec<ResponseLine>),
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
    fenetre.set_habillage(SharedString::from(plan.commandes_habillage()));
    fenetre.set_halos(modele_halos(&plan, &tableau, detail));
    fenetre.set_aretes(SharedString::from(
        plan.aretes()
            .iter()
            .map(|(debut, fin)| format!("M {} {} L {} {} ", debut.x, debut.y, fin.x, fin.y))
            .collect::<String>(),
    ));
    fenetre.set_silhouette(SharedString::from(plan.commandes_silhouette()));
    fenetre.set_faces(SharedString::from(plan.commandes_faces()));
    fenetre.set_organes(SharedString::from(plan.commandes_organes()));
    fenetre.set_anneaux(SharedString::from(plan.commandes_anneaux()));
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
/// Bouge un axe du sélecteur, et **applique** la couleur obtenue.
///
/// L'aperçu suivait déjà le doigt ; les LED non, et c'est tout le défaut de
/// #47. Le limiteur est là parce qu'une jauge traînée émet bien plus vite que le
/// démon n'encaisse : il laisse passer la première valeur, retient les
/// suivantes, et garde la dernière — celle sur laquelle le doigt s'arrête.
fn bouger_un_axe(
    faible: &Weak<Fenetre>,
    pupitre: &Pupitre,
    envoi: &Sender<Request>,
    changer: impl FnOnce(&mut Tsl),
) {
    let Some(fenetre) = faible.upgrade() else {
        return;
    };
    let mut tsl = pupitre.couleur.get();
    changer(&mut tsl);
    pupitre.couleur.set(tsl);
    poser_couleur(&fenetre, pupitre);

    let maintenant = pupitre.maintenant();
    let a_envoyer = pupitre
        .limiteur
        .borrow_mut()
        .proposer(pupitre.rgb(), maintenant);
    if let Some(couleur) = a_envoyer {
        appliquer_la_couleur(pupitre, envoi, couleur);
    }
}

/// Envoie une couleur là où elle doit aller, **sous la forme que l'animation en
/// cours permet** (#63).
///
/// Une couleur posée pendant qu'une animation tourne ne l'éteint plus : elle la
/// rejoue, changée de couleur. Et une sélection partielle devient une zone, pour
/// que le reste du boîtier garde la sienne. C'est `reglages.rs` qui décide — ici
/// on ne fait que lui fournir de quoi décider, et poster ce qu'il rend.
fn appliquer_la_couleur(pupitre: &Pupitre, envoi: &Sender<Request>, couleur: Rgb) {
    // L'éclairage change : la pastille du profil rappelé s'éteint. Une ambiance
    // dont on vient de repeindre une LED n'est plus celle qu'on a enregistrée,
    // et la laisser allumée serait dire le contraire.
    pupitre.oublier_le_rappel();
    let visee = pupitre.visee.borrow().clone();
    let selection = pupitre.selection.borrow();
    let requetes = requetes_pour_la_couleur(
        &pupitre.reglage.borrow(),
        couleur,
        visee.as_deref(),
        // ⚠️ `nom_de_zone`, jamais `nom` : le second est un libellé pour l'œil,
        // et deux sélections différentes s'y écrivent pareil.
        &selection.nom_de_zone(),
        &cibles(&selection),
        *selection == Selection::tout(),
        |couleur| commandes_de_couleur(&pupitre.tableau.borrow(), &selection, couleur),
    );
    drop(selection);

    // La liste des zones se redemande dès qu'une zone a pu bouger — celle qui
    // était visée, ou celle que la sélection vient de faire naître. Sans quoi le
    // panneau montrerait une zone de moins que le démon n'en tient.
    let touche_une_zone = requetes.iter().any(|requete| {
        matches!(
            requete,
            Request::ZoneSet { .. } | Request::ZoneLight { .. } | Request::ZoneAnim { .. }
        )
    });
    for requete in requetes {
        let _ = envoi.send(requete);
    }
    if touche_une_zone {
        let _ = envoi.send(Request::ZoneList);
    }
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

/// Les couches de halo de toutes les LED allumées (#64).
///
/// ⚠️ **De la plus large à la plus serrée, toutes LED confondues.** L'ordre du
/// modèle est l'ordre de rendu : couche par couche et non LED par LED, sinon le
/// halo large d'une LED passerait par-dessus le halo serré de sa voisine et le
/// dégradé aurait des marches là où deux ventilateurs se touchent.
///
/// C'est `plan::halo` qui décide de tout — combien de couches, à quel rayon, à
/// quelle opacité, et qu'une LED noire n'en a aucune. Ici on ne fait que placer.
fn modele_halos(plan: &Plan, tableau: &Tableau, detail: Detail) -> ModelRc<PointHalo> {
    let rayon = plan.rayon_anneau() / 8.0;
    let mut couches: Vec<(usize, PointHalo)> = Vec::new();
    for point in modele_leds_brut(plan, tableau, detail) {
        for (rang, couche) in halo(point.1).into_iter().enumerate() {
            couches.push((
                rang,
                PointHalo {
                    x: point.0.x,
                    y: point.0.y,
                    rayon: rayon * couche.rayon,
                    couleur: avec_opacite(couche.couleur, couche.opacite),
                },
            ));
        }
    }
    // Rang décroissant : la plus large d'abord, donc au fond.
    couches.sort_by_key(|(rang, _)| std::cmp::Reverse(*rang));
    ModelRc::new(VecModel::from(
        couches
            .into_iter()
            .map(|(_, halo)| halo)
            .collect::<Vec<_>>(),
    ))
}

/// Où sont les LED et de quelle couleur, sans rien décider de leur dessin.
///
/// Le détail « ventilateur » regroupe : dix anneaux et quatre barrettes valent
/// alors quatorze halos, et non cent vingt-quatre. Le halo suit ce que la
/// maquette montre, sinon un boîtier vu en gros porterait le halo de LED qu'on
/// ne voit pas.
fn modele_leds_brut(plan: &Plan, tableau: &Tableau, detail: Detail) -> Vec<(Place, Rgb)> {
    match detail {
        Detail::Led => Selection::tout()
            .cibles
            .into_iter()
            .filter_map(|cible| {
                let place = match cible {
                    Cible::Led { position, led } => plan.led_ventilateur(position, led),
                    Cible::Barrette { slot, led } => plan.led_barrette(slot, led),
                };
                place.map(|place| (place, tableau.couleur(cible)))
            })
            .collect(),
        Detail::Ventilateur => Position::ALL
            .into_iter()
            .map(|position| {
                (
                    plan.centre_ventilateur(position),
                    tableau.moyenne(Organe::Ventilateur(position)),
                )
            })
            .chain((0..SLOT_COUNT).filter_map(|slot| {
                plan.centre_barrette(slot)
                    .map(|centre| (centre, tableau.moyenne(Organe::Reglette(slot))))
            }))
            .collect(),
    }
}

/// Une couleur de LED, à l'opacité que le halo demande.
fn avec_opacite(couleur: Rgb, opacite: f32) -> Color {
    Color::from_argb_u8(
        (opacite.clamp(0.0, 1.0) * 255.0).round() as u8,
        couleur.r,
        couleur.g,
        couleur.b,
    )
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
        let envoi = ordres.clone();
        fenetre.on_teinte_changee(move |valeur| {
            bouger_un_axe(&faible, &pupitre, &envoi, |tsl| {
                tsl.teinte = valeur.clamp(0.0, 359.0);
            });
        });
    }
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        let envoi = ordres.clone();
        fenetre.on_saturation_changee(move |valeur| {
            bouger_un_axe(&faible, &pupitre, &envoi, |tsl| {
                tsl.saturation = valeur.clamp(0.0, 100.0);
            });
        });
    }
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        let envoi = ordres.clone();
        fenetre.on_luminosite_changee(move |valeur| {
            bouger_un_axe(&faible, &pupitre, &envoi, |tsl| {
                tsl.luminosite = valeur.clamp(0.0, 100.0);
            });
        });
    }

    // ── Appliquer la couleur ───────────────────────────────────────────────
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        fenetre.on_appliquer_couleur(move || {
            // Le bouton et le champ hexadécimal ne passent **pas** par le
            // limiteur : ce sont des gestes uniques, pas une rafale, et les
            // retenir ferait attendre un clic pour rien. Une zone visée reçoit
            // la couleur à la place du boîtier — c'est tout l'intérêt d'en
            // avoir une, et `requetes_vers_la_cible` porte cette règle pour les
            // deux chemins (#47).
            appliquer_la_couleur(&pupitre, &envoi, pupitre.rgb());
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
            // ⚠️ `requete_d_animation` et non `Reglage::commande` : elle porte la
            // sonde, que `thermique` **exige**, et elle **rend le refus** au
            // lieu de l'avaler. Un `None` silencieux ici, c'est un panneau qui
            // ne fait rien quand on choisit `thermique` sans sonde.
            let sonde = pupitre.sonde_au_rang(fenetre.get_sonde_choisie());
            let requete = match requete_d_animation(&reglage, sonde.as_deref()) {
                Ok(requete) => requete,
                Err(refus) => {
                    fenetre.set_message(SharedString::from(refus.to_string()));
                    return;
                }
            };
            pupitre.oublier_le_rappel();
            poser_profils(&fenetre, &pupitre);
            if let Some(zone) = pupitre.visee.borrow().clone() {
                let _ = envoi.send(vers_la_zone(zone, requete));
                let _ = envoi.send(Request::ZoneList);
                return;
            }
            fenetre.set_animation_courante(SharedString::from(lisible_d_animation(&nom)));
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
            // pas démarrer une animation que personne n'a demandée. Cette
            // garde-là reste sur `commande`, qui la porte depuis #32.
            if reglage.animation.is_none() {
                return;
            }
            let sonde = pupitre.sonde_au_rang(fenetre.get_sonde_choisie());
            // Un refus est **tu** ici, et c'est voulu : `regler-animation` part
            // à chaque cran de curseur, et une phrase d'erreur par cran
            // clignoterait dans le bandeau. Le clic sur la famille, lui, le dit.
            let Ok(requete) = requete_d_animation(&reglage, sonde.as_deref()) else {
                return;
            };
            match pupitre.visee.borrow().clone() {
                Some(zone) => {
                    let _ = envoi.send(vers_la_zone(zone, requete));
                }
                None => {
                    let _ = envoi.send(requete);
                }
            }
        });
    }
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_arreter_animation(move || {
            // Le bouton « Arrêter » ramène aussi le menu au rang zéro : le
            // sélectionner depuis le menu l'y met tout seul, le bouton non.
            if let Some(fenetre) = faible.upgrade() {
                fenetre.set_animation_choisie(0);
            }
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
                fenetre.set_animation_courante(SharedString::from(lisible_d_animation(AUCUNE)));
            }
            pupitre.reglage.borrow_mut().animation = None;
            let _ = envoi.send(Request::Animate {
                name: None,
                reglages: Vec::new(),
            });
        });
    }

    // ── Les ambiances ──────────────────────────────────────────────────────
    //
    // ⚠️ **Le nom est validé ici, par `NomProfil`** — le type du démon, appelé
    // par le même code. Laisser passer « ../etc/passwd » ferait revenir le refus
    // une seconde plus tard, sans rien dire de plus, et le nom serait perdu.
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_rappeler_profil(move |nom| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            match requete_de_profil(&ChoixDeProfil::Rappeler(nom.to_string())) {
                Ok(requete) => {
                    *pupitre.rappele.borrow_mut() = Some(nom.to_string());
                    poser_profils(&fenetre, &pupitre);
                    let _ = envoi.send(requete);
                    // Une ambiance emporte l'éclairage, les zones et l'écran :
                    // les trois se redemandent, sinon la fenêtre montrerait
                    // l'ambiance d'avant jusqu'au prochain tour d'horloge.
                    let _ = envoi.send(Request::Lighting);
                    let _ = envoi.send(Request::ZoneList);
                    let _ = envoi.send(Request::Screen(ScreenAction::State));
                }
                Err(refus) => fenetre.set_message(SharedString::from(refus.to_string())),
            }
        });
    }
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_enregistrer_profil(move |nom| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            match requete_de_profil(&ChoixDeProfil::Enregistrer(nom.to_string())) {
                Ok(requete) => {
                    // Enregistrer, c'est nommer ce qui est à l'écran : la
                    // pastille s'allume sur ce nom, et le champ se vide — le
                    // laisser plein ferait réenregistrer au clic suivant.
                    *pupitre.rappele.borrow_mut() = Some(nom.to_string());
                    fenetre.set_nouveau_profil(SharedString::new());
                    let _ = envoi.send(requete);
                    let _ = envoi.send(Request::Profil(reverb_proto::ipc::ProfilAction::List));
                }
                Err(refus) => fenetre.set_message(SharedString::from(refus.to_string())),
            }
        });
    }
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_oublier_profil(move |nom| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            match requete_de_profil(&ChoixDeProfil::Oublier(nom.to_string())) {
                Ok(requete) => {
                    // Oublier celle qu'on venait de rappeler éteint la pastille :
                    // elle désignerait sinon une ambiance qui n'existe plus.
                    if pupitre.rappele.borrow().as_deref() == Some(nom.as_str()) {
                        pupitre.oublier_le_rappel();
                    }
                    let _ = envoi.send(requete);
                    let _ = envoi.send(Request::Profil(reverb_proto::ipc::ProfilAction::List));
                }
                Err(refus) => fenetre.set_message(SharedString::from(refus.to_string())),
            }
        });
    }

    // ── L'écran du Kraken ──────────────────────────────────────────────────
    //
    // La fenêtre n'ouvre aucun périphérique (ADR-002) : elle envoie un **chemin
    // de fichier**, et c'est le démon qui lit. Jamais 1,2 Mo de pixels sur un
    // protocole texte.
    {
        let envoi = ordres.clone();
        fenetre.on_regler_luminosite_ecran(move |pourcent| {
            let borne = u8::try_from(pourcent.clamp(0, 100)).unwrap_or(100);
            let _ = envoi.send(Request::Screen(ScreenAction::Brightness(borne)));
        });
    }
    {
        let pupitre = pupitre.clone();
        fenetre.on_saisir_ecran(move || {
            pupitre.ecran.borrow_mut().saisir();
        });
    }
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_poser_ecran(move |quoi, argument| {
            let argument = argument.trim().to_owned();
            // Le cadran ne se tape plus : la sonde se choisit sous son **nom
            // lisible**, et c'est ici qu'on retrouve le slug. Le README relevait
            // déjà que le cadran imposait `kraken2023elite:coolant-temp`, et que
            // c'en était un défaut.
            let action = match quoi.as_str() {
                "rien" => ScreenAction::Off,
                "cadran" => {
                    let Some(fenetre) = faible.upgrade() else {
                        return;
                    };
                    ScreenAction::Gauge(
                        pupitre
                            .sonde_au_rang(fenetre.get_sonde_choisie())
                            .unwrap_or_default(),
                    )
                }
                "image" => ScreenAction::Image(argument),
                "gif" => ScreenAction::Gif(argument),
                _ => return,
            };
            // Un argument vide est refusé **ici** : l'envoyer ferait refuser la
            // ligne par le démon avec un message sur le cadrage du protocole,
            // là où l'utilisateur a simplement oublié de remplir le champ.
            if matches!(
                &action,
                ScreenAction::Gauge(vide) | ScreenAction::Image(vide) | ScreenAction::Gif(vide)
                    if vide.is_empty()
            ) {
                if let Some(fenetre) = faible.upgrade() {
                    fenetre.set_message(SharedString::from(
                        "il manque le chemin du fichier, ou le nom de la sonde",
                    ));
                }
                return;
            }
            // La poignée retombe : l'ordre est parti, et c'est désormais l'état
            // réel qui doit s'afficher — y compris quand le démon refuse, ce qui
            // est justement le cas où l'utilisateur doit voir la vérité (#48).
            pupitre.ecran.borrow_mut().relacher();
            let _ = envoi.send(Request::Screen(action));
            let _ = envoi.send(Request::Screen(ScreenAction::State));
        });
    }

    // ── La composition (#80) ───────────────────────────────────────────────
    //
    // ⚠️ **Une commande par changement**, jamais une ligne unique qui porterait
    // tout : un chemin de fond et un libellé de champ sur la même ligne seraient
    // ambigus au premier espace. C'est la règle du dernier champ, celle des
    // profils et des chemins d'image.
    {
        let pupitre = pupitre.clone();
        let faible = fenetre.as_weak();
        fenetre.on_viser_ancre(move |nom| {
            let Ok(ancre) = Ancre::depuis_slug(&nom) else {
                return;
            };
            pupitre.ancre_visee.set(ancre);
            if let Some(fenetre) = faible.upgrade() {
                // Le champ déjà posé sur cette ancre remplit le formulaire :
                // cliquer une ancre garnie pour la retrouver vide obligerait à
                // retaper son libellé pour n'en changer que la sonde.
                garnir_le_formulaire(&fenetre, &pupitre, ancre);
                poser_ancres(&fenetre, &pupitre);
            }
        });
    }
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_poser_fond(move || {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            let chemin = fenetre.get_chemin_fond().trim().to_owned();
            let fond = if fenetre.get_fond_choisi() == 0 {
                Fond::Noir
            } else if chemin.is_empty() {
                fenetre.set_message(SharedString::from("il manque le chemin du fond"));
                return;
            } else {
                Fond::Image(chemin)
            };
            pupitre.ecran.borrow_mut().relacher();
            let _ = envoi.send(requete_de_composition(&ChoixDeComposition::Fond(fond)));
            let _ = envoi.send(Request::Screen(ScreenAction::State));
        });
    }
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        let faible = fenetre.as_weak();
        fenetre.on_poser_champ(move |libelle| {
            let Some(fenetre) = faible.upgrade() else {
                return;
            };
            let libelle = libelle.trim().to_owned();
            let source = if fenetre.get_source_choisie() == 0 {
                let Some(sonde) = pupitre.sonde_au_rang(fenetre.get_sonde_choisie()) else {
                    fenetre.set_message(SharedString::from(
                        "aucune sonde n'est encore connue — le démon n'a pas répondu",
                    ));
                    return;
                };
                Source::Temperature {
                    sonde,
                    // Un libellé vide n'est pas un libellé blanc : c'est
                    // l'absence de libellé, et le démon écrit alors le slug.
                    libelle: (!libelle.is_empty()).then_some(libelle),
                }
            } else if libelle.is_empty() {
                fenetre.set_message(SharedString::from("il manque le texte à afficher"));
                return;
            } else {
                Source::Texte(libelle)
            };
            pupitre.ecran.borrow_mut().relacher();
            let _ = envoi.send(requete_de_composition(&ChoixDeComposition::Champ(
                pupitre.ancre_visee.get(),
                source,
            )));
            let _ = envoi.send(Request::Screen(ScreenAction::State));
        });
    }
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        fenetre.on_vider_champ(move |_| {
            pupitre.ecran.borrow_mut().relacher();
            let _ = envoi.send(requete_de_composition(&ChoixDeComposition::Vide(
                pupitre.ancre_visee.get(),
            )));
            let _ = envoi.send(Request::Screen(ScreenAction::State));
        });
    }
    {
        let pupitre = pupitre.clone();
        let envoi = ordres.clone();
        fenetre.on_arreter_composition(move || {
            pupitre.ecran.borrow_mut().relacher();
            let _ = envoi.send(requete_de_composition(&ChoixDeComposition::Aucune));
            let _ = envoi.send(Request::Screen(ScreenAction::State));
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
            // poignée qui le décide, pas la cadence de la souris. Et c'est la
            // ligne du canal qui décide s'il y a quelqu'un pour la recevoir : un
            // canal en quarantaine n'émet rien (#100).
            if let Some(consigne) = poignee.a_envoyer()
                && let Some(requete) = pupitre.commande_du_canal(&canal, FanAction::Pwm(consigne))
            {
                let _ = envoi.send(requete);
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
            // ⚠️ **« auto » non plus ne part vers un canal en quarantaine**
            // (#100), et c'est justement le bouton qu'on irait chercher quand la
            // ligne ne montre plus rien. La poignée n'est pas libérée pour
            // autant : rien n'a été demandé, donc rien n'a changé.
            let Some(requete) = pupitre.commande_du_canal(&canal, FanAction::Auto) else {
                return;
            };
            pupitre
                .poignees
                .borrow_mut()
                .entry(canal.to_string())
                .or_insert_with(Poignee::nouvelle)
                .liberer();
            let _ = envoi.send(requete);
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
        Request::Screen(_) => {
            let _ = retours.send(Retour::Ecran(lignes));
        }
        // ⚠️ `List` seulement. Une réponse à `save`, `load` ou `drop` porte elle
        // aussi des lignes `profil`, mais elle décrit **une** ambiance : la
        // prendre pour la liste réduirait la barre à celle-là. Ces trois-là
        // redemandent la liste eux-mêmes quand elle a pu changer.
        Request::Profil(reverb_proto::ipc::ProfilAction::List) => {
            let _ = retours.send(Retour::Profils(lignes));
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
            // Une zone visée : le panneau édite **sa** couche, pas celle du
            // boîtier entier. Adopter l'état global ramènerait les curseurs sur
            // une animation que personne n'est en train de régler.
            if pupitre.visee.borrow().is_some() {
                return false;
            }
            // Rien à faire tant que le démon décrit l'animation que la fenêtre
            // pilote déjà : réécrire les curseurs chaque seconde les ferait
            // sauter sous les doigts qui les tirent.
            let lu = eclairage_lu(&lignes);
            if !pupitre.reglage.borrow_mut().adopter(&lu) {
                return false;
            }
            let reglage = pupitre.reglage.borrow().clone();
            fenetre.set_animation_courante(SharedString::from(lisible_d_animation(
                reglage.animation.as_deref().unwrap_or(AUCUNE),
            )));
            // Le menu montre ce qui tourne, y compris quand la fenêtre vient de
            // s'ouvrir sur un boîtier qui animait déjà.
            fenetre.set_animation_choisie(rang_dans_le_menu(reglage.animation.as_deref()));
            // Les curseurs, ensuite : `relever` les relit à chaque geste, et
            // les laisser en arrière renverrait au démon les réglages du
            // sélecteur — régler la vitesse repeindrait le boîtier.
            fenetre.set_vitesse(i32::from(reglage.vitesse));
            fenetre.set_direction(i32::try_from(reglage.direction).unwrap_or(0));
            pupitre.couleur.set(reglage.couleur.en_tsl());
            poser_couleur(fenetre, pupitre);
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
        Retour::Profils(lignes) => {
            // ⚠️ **Seule la réponse à `profil list` arrive ici** — c'est
            // `repondre` qui trie, sur la requête et non sur ce qui revient. Une
            // réponse à `save` porte elle aussi une ligne `profil`, et la
            // prendre pour une liste réduirait la barre à cette seule ambiance.
            //
            // Seul l'état `connu` fait une entrée : `cree`, `ecrase`, `applique`
            // et `oublie` disent ce qui vient d'arriver à l'une d'elles.
            *pupitre.profils.borrow_mut() = lignes
                .iter()
                .filter_map(|ligne| match ligne {
                    ResponseLine::Profil { etat, nom } if etat == "connu" => Some(nom.clone()),
                    _ => None,
                })
                .collect();
            poser_profils(fenetre, pupitre);
            false
        }
        Retour::Ecran(lignes) => {
            // La composition d'abord : elle vit sur les lignes qui **suivent**
            // `screen`, et le disque des ancres doit la montrer même quand la
            // poignée de #48 retient le reste du panneau.
            if lignes
                .iter()
                .any(|ligne| matches!(ligne, ResponseLine::Screen { .. }))
            {
                let fond = lignes
                    .iter()
                    .find_map(|ligne| match ligne {
                        ResponseLine::Layout { fond } => Some(fond.clone()),
                        _ => None,
                    })
                    // Un `screen` **sans** ligne `layout` dit que la dalle n'en
                    // porte pas : leur absence *est* l'information (#80). Garder
                    // la composition d'avant montrerait des champs disparus.
                    .unwrap_or_default();
                let champs = lignes
                    .iter()
                    .filter_map(|ligne| match ligne {
                        ResponseLine::LayoutChamp { ancre, source } => Ancre::depuis_slug(ancre)
                            .ok()
                            .map(|ancre| (ancre, source.clone())),
                        _ => None,
                    })
                    .collect();
                *pupitre.composition.borrow_mut() = (fond, champs);
                poser_ancres(fenetre, pupitre);
            }

            for ligne in &lignes {
                let ResponseLine::Screen {
                    luminosite,
                    affichage,
                } = ligne
                else {
                    continue;
                };
                // Le menu suit ce que la dalle montre vraiment, et le champ
                // porte son argument : ouvrir la fenêtre sur un cadran doit
                // montrer **quelle** sonde, pas un champ vide qui perdrait le
                // réglage au premier « Appliquer ».
                //
                // ⚠️ Mais **rien de tout cela ne s'écrit pendant qu'on compose**
                // (#48). Seule la luminosité traverse la poignée : elle part à
                // chaque cran et n'est jamais composée.
                let mut choisi = pupitre.ecran.borrow_mut();
                choisi.adopter(*luminosite, affichage);
                fenetre.set_luminosite_ecran(i32::from(choisi.luminosite));
                fenetre.set_affichage_ecran(SharedString::from(affichage.clone()));
                if !choisi.compose() {
                    let rang = i32::try_from(choisi.affichage).unwrap_or(0);
                    fenetre.set_affichage_choisi(rang);
                    fenetre.set_argument_ecran(SharedString::from(choisi.argument.clone()));
                    // Le cadran n'a plus de champ de texte : sa sonde se
                    // retrouve dans le menu, par son slug.
                    //
                    // ⚠️ **Le menu n'offre que les sondes retenues** — quatre
                    // familles sur seize (#51). Un cadran posé par le socket sur
                    // une autre laisse donc le menu où il est, et c'est le
                    // bandeau « ÉCRAN — gauge:… » qui dit la vérité. Le déplacer
                    // au hasard serait pire : le clic suivant changerait de
                    // sonde sans qu'on l'ait demandé.
                    if let Some(rang) = pupitre
                        .sondes
                        .borrow()
                        .iter()
                        .position(|retenue| retenue.slug == choisi.argument)
                        .and_then(|rang| i32::try_from(rang).ok())
                    {
                        fenetre.set_sonde_choisie(rang);
                    }
                }
            }
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
    let mut temperatures = Vec::new();
    let maintenant = pupitre.maintenant();
    // Le tri est du calcul, et il vit dans la bibliothèque : c'est lui qui sait
    // qu'un `unreadable` dont le sujet est un canal déjà vu est une ligne de
    // ventilateur, et non un relevé de sonde (#100).
    let vue = pupitre.tri.borrow_mut().poser(lignes);
    let mut poignees = pupitre.poignees.borrow_mut();
    let mut canaux = Vec::new();
    for ligne in &vue.canaux {
        let poignee = poignees
            .entry(ligne.canal.clone())
            .or_insert_with(Poignee::nouvelle);
        // La mesure passe par la poignée : c'est elle qui décide si elle a le
        // droit de déplacer le curseur, ou si une consigne encore fraîche le
        // tient.
        //
        // ⚠️ **Elle ne reçoit que ce qui a été mesuré**, et un canal en
        // quarantaine ne mesure rien (#100). Le 0 que la fenêtre lui donnait
        // quand la consigne manquait ferait retomber le curseur à zéro à chaque
        // hoquet du Kraken — ce qui se lit comme un ventilateur qu'on vient
        // d'arrêter, et c'est le maquillage que le projet refuse partout
        // ailleurs.
        if let Some(pwm) = ligne.pwm {
            poignee.mesurer(pwm, maintenant);
        }
        canaux.push(LigneVentilateur {
            canal: SharedString::from(ligne.canal.clone()),
            position: SharedString::from(
                ligne
                    .position
                    .map_or_else(String::new, |position| position.name().to_owned()),
            ),
            rpm: SharedString::from(
                ligne
                    .rpm
                    .map_or_else(|| "—".to_owned(), |tours| tours.to_string()),
            ),
            pwm: i32::from(poignee.affichee()),
            // Le texte et la barre lisent la **même** poignée : c'est ce qui les
            // garde d'accord pendant qu'on tire, sans attendre le tour de
            // télémétrie suivant. Le `pwm` ne sert qu'à dire si le canal répond
            // (#102) — et depuis #100 c'est celui de la ligne triée, donc `None`
            // pour un canal en quarantaine, qui écrit alors `-- %`.
            consigne: SharedString::from(consigne_affichee(poignee, ligne.pwm)),
            mode: SharedString::from(ligne.mode.clone()),
            lisible: ligne.lisible,
            sait_faire_auto: ligne.sait_faire_auto,
        });
    }
    *pupitre.canaux_lus.borrow_mut() = vue.canaux;

    {
        let mut historique = pupitre.historique.borrow_mut();
        for (sonde, releve) in vue.sondes {
            historique.noter(&sonde, releve);
        }
        // ⚠️ **Le démon rend ses seize sondes, la fenêtre en montre quatre.**
        // C'est un choix d'affichage, pas un filtre de relevé : `status` les
        // rend toutes et le cadran de l'écran les vise toutes (issue #51).
        let retenues = sondes_retenues(&historique.sondes(), &pupitre.modeles_nvme);
        // ⚠️ **Le menu des sondes se pose une seule fois**, quand la liste
        // change vraiment. Le réécrire à chaque seconde recréerait le
        // `ComboBox` sous les doigts, et remettrait son rang à zéro — le même
        // défaut que celui des poignées de ventilateur, par une autre porte.
        if *pupitre.sondes.borrow() != retenues {
            fenetre.set_sondes_lisibles(ModelRc::new(VecModel::from(
                retenues
                    .iter()
                    .map(|retenue| SharedString::from(retenue.libelle.clone()))
                    .collect::<Vec<SharedString>>(),
            )));
            *pupitre.sondes.borrow_mut() = retenues.clone();
        }
        for retenue in retenues {
            let sonde = retenue.slug;
            let lisible = matches!(historique.dernier(&sonde), Some(Releve::Valeur(_)));
            temperatures.push(LigneTemperature {
                libelle: SharedString::from(retenue.libelle),
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
        .map(|nom| {
            let acceptees = Animation::par_nom(nom)
                .map_or(&[] as &[&str], |animation| animation.parametres_acceptes());
            FamilleAnimation {
                nom: SharedString::from(*nom),
                effet: SharedString::from(
                    EFFETS
                        .iter()
                        .find(|(famille, _)| famille == nom)
                        .map_or("", |(_, effet)| effet),
                ),
                accepte_couleur: acceptees.contains(&"couleur"),
                // ⚠️ **C'est cette clé qui décide de montrer le menu des
                // directions**, et non une liste de noms écrite ici : `rotation`,
                // `pouls`, `scintillement` et `thermique` la refusent, et la leur
                // donner ferait rejeter l'`animate` **entier** — pas seulement
                // la clé. Le catalogue est le seul juge.
                suit_une_direction: acceptees.contains(&"direction"),
                exige_une_sonde: Animation::par_nom(nom)
                    .is_ok_and(|animation| animation.parametres_obligatoires().contains(&"sonde")),
            }
        })
        .collect();
    ModelRc::new(VecModel::from(familles))
}

/// Les huit directions, sous le mot qui les écrit sur le socket.
///
/// ⚠️ **L'ordre est celui de `Direction::ALL`**, parce que le rang choisi dans
/// le menu *est* ce qui part au démon. Une liste écrite dans le `.slint` serait
/// restée à six quand #75 en a livré deux de plus, et rien ne l'aurait dit.
fn noms_des_directions() -> ModelRc<SharedString> {
    let noms: Vec<SharedString> = directions_offertes()
        .into_iter()
        .map(|direction| SharedString::from(lisible_de_direction(direction.slug())))
        .collect();
    ModelRc::new(VecModel::from(noms))
}

/// Une direction telle qu'on la lit — la flèche dit le sens mieux qu'un tiret.
///
/// Le slug reste ce qui part au démon ; c'est le **rang** dans la liste qui
/// relie les deux, jamais le texte. Une direction inconnue se lit sous son slug
/// plutôt que d'être cachée : un menu plus court que le catalogue mentirait.
fn lisible_de_direction(slug: &str) -> String {
    match slug {
        "bas-haut" => "Bas → haut".to_owned(),
        "haut-bas" => "Haut → bas".to_owned(),
        "avant-arriere" => "Avant → arrière".to_owned(),
        "arriere-avant" => "Arrière → avant".to_owned(),
        "horaire" => "Horaire".to_owned(),
        "antihoraire" => "Antihoraire".to_owned(),
        // Les deux locales de #75 : le motif se répète sur chaque objet, et le
        // dire ici évite d'avoir à l'apprendre du README.
        "bords-centre" => "Bords → centre (chaque objet)".to_owned(),
        "centre-bords" => "Centre → bords (chaque objet)".to_owned(),
        autre => autre.to_owned(),
    }
}

/// Les cinq affichages d'écran, dans l'ordre où `EcranChoisi` les range.
///
/// Les mots du protocole : c'est `poser-ecran` qui les reçoit, et il en fait
/// une `ScreenAction`. Ce qui s'affiche vient de [`noms_lisibles_des_affichages`].
fn noms_des_affichages() -> ModelRc<SharedString> {
    let noms: Vec<SharedString> = AFFICHAGES.iter().copied().map(SharedString::from).collect();
    ModelRc::new(VecModel::from(noms))
}

/// Les mêmes, tels qu'on les lit. « GIF » est un sigle, pas un mot.
fn noms_lisibles_des_affichages() -> ModelRc<SharedString> {
    let noms: Vec<SharedString> = AFFICHAGES
        .iter()
        .map(|nom| {
            SharedString::from(match *nom {
                "gif" => "GIF".to_owned(),
                autre => lisible_d_animation(autre),
            })
        })
        .collect();
    ModelRc::new(VecModel::from(noms))
}

/// Les ambiances, et laquelle vient d'être rappelée.
fn poser_profils(fenetre: &Fenetre, pupitre: &Pupitre) {
    let rappele = pupitre.rappele.borrow().clone();
    let lignes: Vec<LigneProfil> = pupitre
        .profils
        .borrow()
        .iter()
        .map(|nom| LigneProfil {
            nom: SharedString::from(nom.clone()),
            rappele: rappele.as_deref() == Some(nom.as_str()),
        })
        .collect();
    fenetre.set_profils(ModelRc::new(VecModel::from(lignes)));
}

/// Les cinq ancres de la dalle, garnies de ce que le démon vient de décrire.
///
/// ⚠️ **Les places viennent de `Ancre::boite()`** — les boîtes du démon, celles
/// qu'il assombrit et sur lesquelles il écrit. La fenêtre ne les recalcule pas,
/// exactement comme elle ne recalcule pas les images du boîtier.
fn poser_ancres(fenetre: &Fenetre, pupitre: &Pupitre) {
    let composition = pupitre.composition.borrow();
    let visee = pupitre.ancre_visee.get();
    let lignes: Vec<AncreEcran> = places_des_ancres()
        .into_iter()
        .map(|place| {
            let porte = composition
                .1
                .iter()
                .find(|(ancre, _)| *ancre == place.ancre)
                .map(|(_, source)| lisible(source));
            AncreEcran {
                nom: SharedString::from(place.ancre.slug()),
                porte: SharedString::from(porte.clone().unwrap_or_default()),
                x: place.x,
                y: place.y,
                largeur: place.largeur,
                hauteur: place.hauteur,
                occupee: porte.is_some(),
                choisie: place.ancre == visee,
            }
        })
        .collect();
    fenetre.set_champs_poses(i32::try_from(composition.1.len()).unwrap_or(0));
    fenetre.set_ancre_visee(SharedString::from(visee.slug()));
    fenetre.set_ancres(ModelRc::new(VecModel::from(lignes)));
}

/// Ce qu'un champ montre, en une étiquette qui tient dans une ancre de 40 px.
///
/// La source arrive telle que le protocole l'écrit — `temp <slug> <libellé>` ou
/// `texte <libellé>`. Le libellé prime quand il existe : c'est précisément ce
/// pour quoi il existe, `kraken2023elite:coolant-temp` faisant vingt-huit
/// caractères là où on en lit dix.
fn lisible(source: &str) -> String {
    let mut mots = source.split_whitespace();
    match mots.next() {
        Some("temp") => {
            let slug = mots.next().unwrap_or_default();
            let libelle: Vec<&str> = mots.collect();
            if libelle.is_empty() {
                slug.to_owned()
            } else {
                libelle.join(" ")
            }
        }
        Some("texte") => source
            .split_once(char::is_whitespace)
            .map_or_else(String::new, |(_, reste)| reste.trim().to_owned()),
        _ => source.to_owned(),
    }
}

/// Remet dans le formulaire ce que l'ancre visée porte déjà.
///
/// Cliquer une ancre garnie pour la retrouver vide obligerait à retaper son
/// libellé pour n'en changer que la sonde — et c'est le geste le plus courant.
fn garnir_le_formulaire(fenetre: &Fenetre, pupitre: &Pupitre, ancre: Ancre) {
    let composition = pupitre.composition.borrow();
    let Some((_, source)) = composition.1.iter().find(|(porte, _)| *porte == ancre) else {
        return;
    };
    let mut mots = source.split_whitespace();
    match mots.next() {
        Some("temp") => {
            fenetre.set_source_choisie(0);
            let slug = mots.next().unwrap_or_default().to_owned();
            let libelle: Vec<&str> = mots.collect();
            fenetre.set_libelle_champ(SharedString::from(libelle.join(" ")));
            if let Some(rang) = pupitre
                .sondes
                .borrow()
                .iter()
                .position(|retenue| retenue.slug == slug)
                .and_then(|rang| i32::try_from(rang).ok())
            {
                fenetre.set_sonde_choisie(rang);
            }
        }
        Some("texte") => {
            fenetre.set_source_choisie(1);
            fenetre.set_libelle_champ(SharedString::from(lisible(source)));
        }
        _ => {}
    }
}

/// Le nom que le menu déroulant porte au rang zéro.
///
/// Ce n'est pas un trou en tête de liste, c'est un choix : celui d'arrêter. Un
/// menu qui n'aurait que les six familles resterait vide quand rien ne tourne,
/// et un menu vide se lit comme une fenêtre en panne.
const AUCUNE: &str = "aucune";

/// Ce que le menu propose : « aucune », puis l'ordre de `CATALOGUE`.
///
/// ⚠️ **Ce sont les noms du protocole**, ceux qui partent sur le socket. Ce que
/// l'utilisateur lit vient de [`noms_lisibles_du_menu`], et le **rang** relie
/// les deux — jamais une seconde liste qui divergerait.
fn noms_du_menu() -> ModelRc<SharedString> {
    let noms: Vec<SharedString> = std::iter::once(AUCUNE)
        .chain(CATALOGUE.iter().copied())
        .map(SharedString::from)
        .collect();
    ModelRc::new(VecModel::from(noms))
}

/// Les mêmes, tels qu'on les lit : capitale en tête, accents remis.
///
/// Le protocole écrit `comete` et `arc-en-ciel` — sans accent, parce qu'une
/// commande se tape. Une pastille, elle, se lit : « Comète ».
fn noms_lisibles_du_menu() -> ModelRc<SharedString> {
    let noms: Vec<SharedString> = std::iter::once(AUCUNE)
        .chain(CATALOGUE.iter().copied())
        .map(|nom| SharedString::from(lisible_d_animation(nom)))
        .collect();
    ModelRc::new(VecModel::from(noms))
}

/// Le nom d'une animation tel qu'on le lit.
///
/// Une table pour les seuls noms que la capitalisation ne suffit pas à rendre —
/// les accents que le protocole n'écrit pas —, et la règle générale pour tous
/// les autres. Une famille ajoutée demain se lit donc correctement sans qu'on
/// ait à revenir ici.
fn lisible_d_animation(nom: &str) -> String {
    match nom {
        "comete" => return "Comète".to_owned(),
        "arc-en-ciel" => return "Arc-en-ciel".to_owned(),
        _ => {}
    }
    let mut lettres = nom.chars();
    match lettres.next() {
        Some(premiere) => premiere.to_uppercase().chain(lettres).collect(),
        None => String::new(),
    }
}

/// Le rang d'une animation dans le menu — zéro pour « aucune ».
///
/// Une animation que le catalogue ne connaît pas retombe elle aussi sur zéro :
/// le menu ne sait pas la montrer, et le bandeau la nomme déjà. Mieux vaut un
/// menu qui dit « aucune » qu'un menu qui montre une autre animation.
fn rang_dans_le_menu(nom: Option<&str>) -> i32 {
    let Some(nom) = nom else {
        return 0;
    };
    CATALOGUE
        .iter()
        .position(|famille| *famille == nom)
        .and_then(|rang| i32::try_from(rang + 1).ok())
        .unwrap_or(0)
}
