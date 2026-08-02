//! Tests d'intention de la couleur posée pendant qu'une animation tourne (issue #63).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #63 seule — aucun corps de
//! `crates/reverb-gui/src/` n'a été lu pour les produire, hors les signatures publiques des types
//! du protocole, du catalogue d'animations et de [`Reglage`]. À l'écriture de ce fichier,
//! `requetes_pour_la_couleur` **n'existe pas** : la compilation de ce test doit échouer sur ce
//! seul nom, et c'est la phase rouge.
//!
//! Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne touche un bus : la correction de #63
//! est **entièrement dans le choix des requêtes**, et c'est ce qui la rend vérifiable sans
//! matériel. Le démon n'est pas touché — `zone set`, `zone anim` et `zone light` font déjà tout ce
//! qu'il faut.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Boîtier en marche, `arc-en-ciel` sur les dix ventilateurs et les quatre barrettes. Nico
//! sélectionne `haut-milieu` à la souris, lui donne une couleur — et **les treize autres cibles se
//! figent** sur la dernière image de l'animation. La fenêtre avait émis `light`, et côté démon une
//! couleur fixe arrête l'animation de la couche globale (`reverb-daemon/src/main.rs:670`, cité par
//! l'issue).
//!
//! Le raisonnement du démon est juste **pour la couche globale**, où couleur fixe et animation
//! visent bien les mêmes LED. Il ne l'est pas pour une sélection partielle : rien ne dispute
//! `bas-gauche` à qui vient de colorer `haut-milieu`.
//!
//! ⚠️ **Le piège de ce fichier tient dans la sélection choisie.** `haut-milieu` est un ventilateur
//! **entier** : `light fan:haut-milieu` ne vise donc aucune LED hors de la sélection, et pourtant
//! c'est exactement la requête qui a tout figé. Un test qui se contenterait de vérifier que les
//! cibles émises restent dans la sélection **laisserait passer le défaut de l'issue**. Ce que ces
//! tests exigent, c'est que rien ne parte sur la **couche globale** — parce que c'est elle, et non
//! l'ensemble des LED touchées, que le démon éteint.
//!
//! ## Ce que ce fichier fige
//!
//! **Une seule règle, énoncée une fois** : *la couleur va à la couche visée, sous la forme que
//! l'animation en cours permet.* Les trois branches en découlent, et c'est ce qui les rend
//! lisibles ensemble plutôt qu'une par une.
//!
//! | ce qui est visé | animation en cours | ce qui part |
//! |---|---|---|
//! | une zone | accepte `couleur` | `zone anim <nom> <la même> couleur=…` |
//! | une zone | `arc-en-ciel` | `zone light <nom> <couleur>` |
//! | une zone | aucune | `zone light <nom> <couleur>` — inchangé depuis #47 |
//! | tout le boîtier | accepte `couleur` | `animate <la même> couleur=…` — elle continue, changée de couleur |
//! | tout le boîtier | `arc-en-ciel` | `light all` — elle s'arrête, faute d'autre choix |
//! | partielle | accepte `couleur` | une **zone** portant `<la même> couleur=…` ; le reste continue |
//! | partielle | `arc-en-ciel` | une **zone** portant la couleur fixe ; le reste continue |
//!
//! Et, quand aucune animation n'est en cours, **exactement** ce que la fenêtre émet aujourd'hui.
//!
//! ## Ce que le contrat laisse ouvert, et que ces tests tranchent
//!
//! 1. **La signature.** L'issue décrit les entrées — « la sélection (son nom, ses cibles, si elle
//!    est entière), l'animation en cours et la couleur » — sans les typer. Ces tests retiennent
//!    des **paramètres nus** plutôt qu'un type de sélection : `crates/reverb-gui/src/main.rs`
//!    possède déjà un `Selection`, et en introduire un second dans `reglages.rs` forcerait un
//!    renommage dans un fichier que la correction n'a aucune raison de remuer.
//! 2. **L'animation en cours est un [`Reglage`], pas un nom.** C'est le cœur du grief de l'issue :
//!    « trois réglages du même panneau, deux comportements » — la vitesse et la direction
//!    repassent par [`Reglage::commande`], la couleur non. Ne recevoir qu'un nom d'animation
//!    reconduirait l'écart, la zone repartant à la vitesse et à la direction par défaut pendant
//!    que le reste du boîtier garde les siennes. Deux allures dans le même boîtier, sans un
//!    message.
//! 3. **La couleur est un paramètre à part, et non `reglage.couleur`.** Elle vient du sélecteur,
//!    qui a bougé ; le réglage porte encore l'ancienne. Les deux diffèrent dans tout ce fichier,
//!    pour qu'une implémentation qui relancerait l'animation dans **son ancienne couleur** — le
//!    geste sans effet, défaut jumeau de #32 — se fasse attraper ici plutôt qu'à l'usage.
//! 4. **Une zone visée reçoit l'animation, elle aussi.** L'issue dit « une zone déjà visée
//!    continue de recevoir la couleur **directement**, sans en créer une seconde (acquis de #47) »,
//!    ce qui se lit volontiers « toujours `zone light` ». Ces tests ne le lisent pas ainsi, et
//!    voici pourquoi.
//!
//!    L'acquis de #47 invoqué par ce critère est `une_zone_visee_recoit_la_couleur_a_sa_place`,
//!    dans `spec_limiteur.rs`. Il porte sur `requetes_vers_la_cible`, une fonction qui **ne reçoit
//!    pas** l'animation en cours : elle ne peut donc rien dire du cas où une animation tourne, et
//!    son propre commentaire le confirme — le figeage y est « voulu quand rien n'est visé ». Il
//!    n'y a donc **aucun acquis à préserver** sur ce cas : il n'a jamais été spécifié.
//!
//!    Reste la demande de l'utilisateur, qui est littérale : « quand une animation tourne, je veux
//!    que si je change la couleur **d'une zone**, l'animation continue de se produire sur cette
//!    zone, mais de la couleur que j'ai choisie ». Il dit « zone ». Une zone visée est exactement
//!    ce cas — c'est même le seul endroit de la fenêtre où le mot a son sens plein.
//!
//!    Ce que « directement » garde de force, et que ces tests exigent : **jamais de seconde
//!    zone**, et **jamais de repli** consulté. La zone existe déjà, elle tient déjà ses LED ; en
//!    déclarer une autre par-dessus les lui prendrait (README : une LED appartient à au plus une
//!    zone), et la zone visée se retrouverait vide.
//! 5. **Le nom de la zone créée est celui de la sélection, tel quel.** L'issue le dit
//!    (`Selection::nom()`), et l'hypothèse porte : c'est de ce déterminisme que vient « deux
//!    couleurs successives ne créent qu'une zone ». Un nom horodaté, numéroté ou tiré au hasard
//!    empilerait une zone par clic jusqu'à ce que le boîtier entier soit découpé.
//! 6. **`zone set` précède ce qui colore la zone.** Une zone qui n'existe pas encore ne se colore
//!    pas ; l'ordre de la salve est donc une partie du contrat, pas un détail de rédaction.
//! 7. **`entiere` est cru sur parole.** La fonction ne recalcule pas si les cibles couvrent les
//!    124 LED : c'est la fenêtre qui sait ce que l'utilisateur a sélectionné, et un second calcul
//!    ne pourrait que diverger du premier.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Ce que le démon fait de ces requêtes.** Il n'est pas touché par #63, et la sémantique de
//!   `light` — arrêter l'animation — est explicitement hors scope : sur la couche globale, elle a
//!   raison.
//! - **Le repli `sans_animation` lui-même**, c'est-à-dire le partage entre `light` et `paint`. Il
//!   existe, il marche, et l'issue demande qu'il soit **inchangé** : ces tests exigent donc qu'il
//!   passe tel quel, pas qu'il soit d'une forme ou d'une autre.
//! - **L'empilement des zones.** Une LED appartient à au plus une zone (README) : la question ne
//!   se pose pas, et l'issue la met hors scope.
//! - **Une sélection vide**, et **un nom d'animation hors du catalogue**. Les deux viennent de la
//!   fenêtre, qui ne propose que ce qu'elle liste. Leur inventer un comportement figerait une
//!   règle que personne n'a choisie.
//! - **Le nommage automatique des zones au-delà de `Selection::nom()`** — hors scope de l'issue.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use reverb_anim::{Animation, CATALOGUE, Direction, Reglages};
use reverb_gui::reglages::{Reglage, requetes_pour_la_couleur};
use reverb_proto::ipc::{LightTarget, Request};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Led, Position, Rgb};

// ---------------------------------------------------------------------------
// Repères et aides
// ---------------------------------------------------------------------------

/// La couleur que l'utilisateur vient de poser. C'est elle qui doit partir.
///
/// Choisie **différente du défaut** de [`Reglages`] et de [`ANCIENNE`] : le décodeur du démon
/// remplace une clé absente par son défaut, donc une implémentation qui perdrait la couleur en
/// route rendrait exactement la même chose qu'une implémentation correcte si le test se
/// contentait de la valeur par défaut.
const POSEE: Rgb = Rgb::new(0x12, 0x9a, 0x40);

/// La couleur que le réglage porte encore : celle dans laquelle l'animation tourne avant le geste.
///
/// Elle ne doit **jamais** partir. Une implémentation qui relancerait l'animation depuis le
/// réglage sans y injecter la couleur posée rendrait un `animate` parfaitement valide, accepté par
/// le démon, et sans le moindre effet visible — le défaut de #32 sous un autre nom.
const ANCIENNE: Rgb = Rgb::new(0xff, 0x00, 0x04);

/// La vitesse et la direction que l'animation en cours affiche.
///
/// Toutes deux **différentes du défaut** de [`Reglages`] (vitesse 3, direction `horaire`), pour
/// qu'une zone qui repartirait aux valeurs d'usine se distingue d'une zone qui reprend les
/// réglages du boîtier. Sans cet écart, les deux rendraient le même `Reglages` relu.
const VITESSE: u8 = 7;
const DIRECTION: Direction = Direction::ArriereAvant;

/// L'animation d'exemple : elle accepte `couleur`.
const AVEC_COULEUR: &str = "comete";

/// Celle qui la refuse — elle produit ses propres teintes.
///
/// C'est celle qui tournait le jour du constat de Nico, et celle qui n'a pas d'autre issue que de
/// se figer. Le test [`le_catalogue_dit_lui_meme_laquelle_refuse_la_couleur`] vérifie que ces deux
/// constantes disent encore la vérité du catalogue : si elle change, c'est lui qu'on croit.
const SANS_COULEUR: &str = "arc-en-ciel";

/// La sélection de Nico, celle du rapport : le ventilateur du haut, au milieu.
///
/// ⚠️ **C'est un organe entier**, et c'est délibéré. `light fan:haut-milieu` ne vise aucune LED
/// hors de cette sélection, et c'est pourtant la requête qui a figé les treize autres cibles. Une
/// sélection qui chevaucherait deux organes rendrait le test plus facile à passer, donc moins
/// utile.
const NOM_PARTIELLE: &str = "haut-milieu";
const SLUG_PARTIELLE: &str = "fan:haut-milieu";

/// Une seconde sélection partielle, à cheval sur les deux bus.
///
/// Elle existe pour que la règle ne se lise pas « un organe se traite ainsi » : un ventilateur et
/// une barrette ensemble ne sont aucun organe, et n'ont aucune [`LightTarget`] qui les recouvre.
const NOM_MELEE: &str = "arrière et la première barrette";

/// Le nom que la sélection du boîtier entier se donne.
const NOM_ENTIERE: &str = "tout le boîtier";

/// Une zone déjà visée par la fenêtre, au sens de #47.
const ZONE_VISEE: &str = "le radiateur";

/// Le rang d'une direction dans [`Direction::ALL`], tel que [`Reglage::direction`] le porte.
fn rang(direction: Direction) -> usize {
    Direction::ALL
        .into_iter()
        .position(|d| d == direction)
        .unwrap_or_else(|| panic!("{direction:?} est une des six directions"))
}

/// Les réglages tels que la fenêtre les affiche au moment du geste.
fn reglage(animation: Option<&str>) -> Reglage {
    Reglage {
        animation: animation.map(str::to_owned),
        couleur: ANCIENNE,
        vitesse: VITESSE,
        direction: rang(DIRECTION),
    }
}

/// Les huit LED du ventilateur du haut, au milieu.
fn partielle() -> Vec<Led> {
    Led::depuis_slug(SLUG_PARTIELLE)
        .unwrap_or_else(|e| panic!("« {SLUG_PARTIELLE} » est un organe du boîtier : {e}"))
}

/// Le ventilateur arrière et la première barrette : dix-neuf LED sur les deux bus.
fn melee() -> Vec<Led> {
    let mut leds = Led::depuis_slug("fan:arriere").expect("« fan:arriere » est un organe");
    leds.extend(Led::depuis_slug("slot:0").expect("« slot:0 » est un organe"));
    leds
}

/// Les 124 LED du boîtier.
fn entiere() -> Vec<Led> {
    Led::toutes()
}

/// Ce qu'une [`LightTarget`] recouvre réellement, reconstruit depuis le protocole seul.
///
/// C'est ce qui permet d'inspecter les **cibles** d'une requête plutôt que de comparer des `Vec`
/// entiers : le critère « aucune requête qui vise une cible hors sélection » se lit sur les LED
/// atteintes, pas sur la forme des requêtes.
fn leds_de(cible: &LightTarget) -> BTreeSet<Led> {
    let toutes = Led::toutes();
    match cible {
        LightTarget::All => toutes.into_iter().collect(),
        LightTarget::Fans => toutes
            .into_iter()
            .filter(|led| matches!(led, Led::Ventilateur { .. }))
            .collect(),
        LightTarget::Ram => toutes
            .into_iter()
            .filter(|led| matches!(led, Led::Barrette { .. }))
            .collect(),
        LightTarget::Fan(position) => (0..LEDS_PER_FAN as usize)
            .map(|led| Led::Ventilateur {
                position: *position,
                led,
            })
            .collect(),
        LightTarget::RamSlot(slot) => (0..LEDS_PER_STICK)
            .map(|led| Led::Barrette { slot: *slot, led })
            .collect(),
    }
}

/// Toutes les cibles protocolaires, de la plus large à la plus étroite.
///
/// Sert au repli à retrouver l'organe d'une sélection, comme la fenêtre le fait aujourd'hui.
fn organes() -> Vec<LightTarget> {
    let mut tous = vec![LightTarget::All, LightTarget::Fans, LightTarget::Ram];
    tous.extend(Position::ALL.into_iter().map(LightTarget::Fan));
    tous.extend((0..SLOT_COUNT).map(LightTarget::RamSlot));
    tous
}

/// Ce que la fenêtre émet **aujourd'hui**, hors animation : `light` sur un organe entier, `paint`
/// sinon (issue #63, « quand aucune animation ne tourne, rien ne change »).
///
/// Il n'est pas ici pour être spécifié — l'issue le met explicitement hors scope — mais pour être
/// **reconnaissable** : les tests exigent qu'il ressorte tel quel, ce qui suppose de savoir à quoi
/// il ressemble. Il tient donc lieu de repli dans tout ce fichier.
fn requetes_d_aujourd_hui(cibles: &[Led], couleur: Rgb) -> Vec<Request> {
    let selection: BTreeSet<Led> = cibles.iter().copied().collect();

    // Un organe recouvert exactement se colore d'un seul `light`.
    for organe in organes() {
        if leds_de(&organe) == selection {
            return vec![Request::Light {
                target: organe,
                color: couleur,
            }];
        }
    }

    // Sinon, chaque organe entamé est peint LED par LED : la couleur là où l'utilisateur a cliqué,
    // le noir ailleurs.
    let mut peintures = Vec::new();
    for organe in organes()
        .into_iter()
        .filter(|o| matches!(o, LightTarget::Fan(_) | LightTarget::RamSlot(_)))
    {
        let leds = leds_de(&organe);
        if leds.is_disjoint(&selection) {
            continue;
        }
        peintures.push(Request::Paint {
            target: organe,
            couleurs: leds
                .into_iter()
                .map(|led| {
                    if selection.contains(&led) {
                        couleur
                    } else {
                        Rgb::BLACK
                    }
                })
                .collect(),
        });
    }
    peintures
}

/// Un mouchard sur le repli : combien de fois il a été consulté, et avec quelle couleur.
///
/// Compter les appels et pas seulement comparer les sorties, c'est ce qui distingue une
/// implémentation qui **choisit** d'une implémentation qui calcule les deux branches et jette la
/// mauvaise — la seconde passerait un test d'égalité.
struct Journal(RefCell<Vec<Rgb>>);

impl Journal {
    fn neuf() -> Journal {
        Journal(RefCell::new(Vec::new()))
    }

    fn noter(&self, couleur: Rgb) {
        self.0.borrow_mut().push(couleur);
    }

    fn appels(&self) -> Vec<Rgb> {
        self.0.borrow().clone()
    }
}

/// L'appel sous test, avec son repli branché sur un journal.
fn emises(
    animation: Option<&str>,
    zone_visee: Option<&str>,
    nom: &str,
    cibles: &[Led],
    est_entiere: bool,
    journal: &Journal,
) -> Vec<Request> {
    requetes_pour_la_couleur(
        &reglage(animation),
        POSEE,
        zone_visee,
        nom,
        cibles,
        est_entiere,
        |couleur| {
            journal.noter(couleur);
            requetes_d_aujourd_hui(cibles, couleur)
        },
    )
}

/// Le même appel quand le repli n'intéresse pas le test.
fn emises_sans_journal(
    animation: Option<&str>,
    zone_visee: Option<&str>,
    nom: &str,
    cibles: &[Led],
    est_entiere: bool,
) -> Vec<Request> {
    emises(
        animation,
        zone_visee,
        nom,
        cibles,
        est_entiere,
        &Journal::neuf(),
    )
}

/// Ce qu'une salve de requêtes touche, **séparé par couche**.
///
/// La distinction est tout le sujet de #63 : le démon éteint l'animation en cours dès qu'on écrit
/// sur la couche globale, quelle que soit l'étendue de ce qu'on y écrit. `light fan:haut-milieu`
/// ne vise que huit LED et fige les cent seize autres.
#[derive(Debug, Default)]
struct Portee {
    /// Les verbes globaux rencontrés, décrits pour que l'échec dise lequel est passé.
    globaux: Vec<String>,
    /// Les LED atteintes par la couche globale.
    leds_globales: BTreeSet<Led>,
    /// Tous les noms de zone touchés, quel que soit le verbe.
    zones: BTreeSet<String>,
    /// Les LED que chaque `zone set` enferme.
    cibles_de_zone: BTreeMap<String, BTreeSet<Led>>,
}

fn portee(requetes: &[Request]) -> Portee {
    let mut vue = Portee::default();
    for requete in requetes {
        match requete {
            Request::Light { target, .. } => {
                vue.globaux.push(format!("light {target:?}"));
                vue.leds_globales.extend(leds_de(target));
            }
            Request::Paint { target, .. } => {
                vue.globaux.push(format!("paint {target:?}"));
                vue.leds_globales.extend(leds_de(target));
            }
            Request::Animate { name, .. } => {
                vue.globaux.push(format!("animate {name:?}"));
                vue.leds_globales.extend(leds_de(&LightTarget::All));
            }
            Request::ZoneSet { nom, cibles } => {
                vue.zones.insert(nom.clone());
                vue.cibles_de_zone
                    .entry(nom.clone())
                    .or_default()
                    .extend(cibles.iter().copied());
            }
            Request::ZoneLight { nom, .. }
            | Request::ZoneAnim { nom, .. }
            | Request::ZoneDrop { nom } => {
                vue.zones.insert(nom.clone());
            }
            _ => {}
        }
    }
    vue
}

/// Ce qu'une requête qui relance une animation porte : le nom de l'animation, et ses réglages.
///
/// Le nom est une `Option` parce que le protocole porte l'extinction ainsi (`animate off` vaut
/// `name: None`) : c'est un cas que ces tests doivent pouvoir **constater** pour l'interdire, pas
/// un cas qu'ils peuvent supposer absent.
struct Relance<'a> {
    nom: Option<String>,
    paires: &'a [(String, String)],
}

/// Ce qu'une requête relance, si elle relance quelque chose — globale ou de zone, indifféremment.
fn relance(requete: &Request) -> Option<Relance<'_>> {
    match requete {
        Request::Animate { name, reglages } => Some(Relance {
            nom: name.clone(),
            paires: reglages.as_slice(),
        }),
        Request::ZoneAnim {
            animation,
            reglages,
            ..
        } => Some(Relance {
            nom: animation.clone(),
            paires: reglages.as_slice(),
        }),
        _ => None,
    }
}

/// Ce que le démon lira de ces paires — sa propre validation, pas une relecture de complaisance.
///
/// C'est `reverb-anim` qui décode de l'autre côté du socket, et son refus porte sur la commande
/// **entière** : passer par lui, c'est vérifier que la salve est jouable, pas seulement qu'elle
/// est jolie.
fn relu(nom: &str, paires: &[(String, String)]) -> Reglages {
    let animation = Animation::par_nom(nom)
        .unwrap_or_else(|e| panic!("« {nom} » doit être une animation du catalogue : {e}"));
    animation
        .reglages(paires)
        .unwrap_or_else(|e| panic!("le démon doit accepter {paires:?} pour « {nom} » : {e}"))
}

/// Vrai si cette animation du catalogue accepte le réglage `couleur`.
///
/// **Interrogé, jamais codé en dur.** Le jour où le catalogue change d'avis, ces tests changent
/// avec lui plutôt que de mentir.
fn se_colore(nom: &str) -> bool {
    Animation::par_nom(nom)
        .unwrap_or_else(|e| panic!("« {nom} » est au catalogue : {e}"))
        .parametres_acceptes()
        .contains(&"couleur")
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Pas un critère d'acceptation : la condition pour que les autres en soient. Le démon remplace
    // une clé absente par le défaut de `Reglages` ; si les valeurs de ce fichier étaient ces
    // défauts, une implémentation qui perdrait tout en route rendrait exactement ce qu'une
    // implémentation correcte rend, et toute la suite passerait au vert sur du vide.
    let defaut = Reglages::default();
    assert_ne!(
        POSEE, defaut.couleur,
        "la couleur posée doit différer du défaut du démon, sinon la perdre ne se verrait pas"
    );
    assert_ne!(
        VITESSE, defaut.vitesse,
        "la vitesse affichée doit différer du défaut : {VITESSE} contre {}",
        defaut.vitesse
    );
    assert_ne!(
        DIRECTION, defaut.direction,
        "la direction affichée doit différer du défaut : {DIRECTION:?} contre {:?}",
        defaut.direction
    );

    // Et la couleur posée doit différer de celle que le réglage porte encore, sans quoi
    // « l'animation repart dans son ancienne couleur » serait indiscernable de « elle repart dans
    // la nouvelle ». C'est le geste sans effet que #63 cite comme la moitié de son grief.
    assert_ne!(
        POSEE, ANCIENNE,
        "la couleur posée et celle du réglage doivent différer, sinon l'injection ne se teste pas"
    );

    // La sélection partielle doit être strictement incluse dans le boîtier : c'est ce qui donne un
    // « hors sélection » à vérifier.
    let choisies: BTreeSet<Led> = partielle().into_iter().collect();
    let toutes: BTreeSet<Led> = entiere().into_iter().collect();
    assert!(
        choisies.is_subset(&toutes) && choisies.len() < toutes.len(),
        "la sélection partielle laisse des LED dehors : {} sur {}",
        choisies.len(),
        toutes.len()
    );

    // Le piège du fichier, énoncé : la sélection de Nico est un organe entier, donc `light` sur
    // elle ne vise rien de plus qu'elle. Si ce n'était plus vrai, le test phare perdrait sa force
    // sans rien signaler.
    assert_eq!(
        requetes_d_aujourd_hui(&partielle(), POSEE),
        vec![Request::Light {
            target: LightTarget::Fan(Position::HautMilieu),
            color: POSEE,
        }],
        "« {NOM_PARTIELLE} » est un organe entier : la fenêtre le colore d'un `light`, et c'est \
         précisément la requête qui a tout figé"
    );

    // La sélection mêlée, elle, n'est recouverte par aucune cible du protocole : rien ne peut la
    // colorer d'un seul `light`.
    let melee: BTreeSet<Led> = melee().into_iter().collect();
    assert!(
        organes().into_iter().all(|o| leds_de(&o) != melee),
        "« {NOM_MELEE} » ne doit être aucun organe du protocole, sinon elle ne dit rien de plus \
         que « {NOM_PARTIELLE} »"
    );
}

// ---------------------------------------------------------------------------
// 1 — c'est le catalogue qui dit laquelle refuse la couleur
// ---------------------------------------------------------------------------

#[test]
fn le_catalogue_dit_lui_meme_laquelle_refuse_la_couleur() {
    // Pas un critère d'acceptation non plus : la garantie que le critère « la sélection reçoit une
    // couleur fixe, jamais `arc-en-ciel couleur=…` » repose sur le catalogue et non sur une
    // croyance de ce fichier. Une suite qui écrirait « arc-en-ciel refuse couleur » en dur
    // mentirait le jour où le catalogue change d'avis, et le mensonge serait vert.
    assert!(
        se_colore(AVEC_COULEUR),
        "« {AVEC_COULEUR} » sert d'exemple d'animation qui se colore : le catalogue doit le dire — \
         {:?}",
        Animation::par_nom(AVEC_COULEUR).map(|a| a.parametres_acceptes())
    );
    assert!(
        !se_colore(SANS_COULEUR),
        "« {SANS_COULEUR} » sert d'exemple d'animation qui refuse la couleur : le catalogue doit \
         le dire — {:?}",
        Animation::par_nom(SANS_COULEUR).map(|a| a.parametres_acceptes())
    );

    // Et le catalogue doit contenir les deux espèces, sans quoi la moitié du tableau de l'issue
    // n'aurait plus de cas à décrire.
    assert!(
        CATALOGUE.iter().any(|nom| se_colore(nom)),
        "le catalogue doit garder au moins une animation qui se colore : {CATALOGUE:?}"
    );
    assert!(
        CATALOGUE.iter().any(|nom| !se_colore(nom)),
        "le catalogue doit garder au moins une animation qui refuse la couleur : {CATALOGUE:?}"
    );
}

// ---------------------------------------------------------------------------
// 2 — sans animation en cours, rien ne change
// ---------------------------------------------------------------------------

#[test]
fn sans_animation_en_cours_les_requetes_sont_celles_d_aujourd_hui() {
    // Critère d'acceptation : « aucune animation en cours : les requêtes émises sont **inchangées**
    // par rapport à aujourd'hui (`light`/`paint`) ».
    //
    // C'est le critère de non-régression du lot, et le plus facile à casser : une implémentation
    // qui prendrait le chemin des zones dès qu'une sélection est partielle découperait le boîtier
    // en zones alors que personne n'a demandé de zone, et une couleur posée sans animation
    // cesserait d'être une couleur posée.
    for (nom, cibles, est_entiere) in [
        (NOM_ENTIERE, entiere(), true),
        (NOM_PARTIELLE, partielle(), false),
        (NOM_MELEE, melee(), false),
    ] {
        let journal = Journal::neuf();
        let rendues = emises(None, None, nom, &cibles, est_entiere, &journal);

        assert_eq!(
            rendues,
            requetes_d_aujourd_hui(&cibles, POSEE),
            "sans animation, « {nom} » reçoit exactement ce que la fenêtre émet aujourd'hui — \
             même contenu, même ordre"
        );
        assert_eq!(
            journal.appels(),
            vec![POSEE],
            "le repli est consulté une fois et une seule, avec la couleur posée — pour « {nom} »"
        );
    }

    // Et si le repli n'a rien à envoyer, la fonction n'invente rien à sa place. Un `light all` de
    // secours ferait d'un clic sans effet un effacement du boîtier.
    let rendues = requetes_pour_la_couleur(
        &reglage(None),
        POSEE,
        None,
        NOM_PARTIELLE,
        &partielle(),
        false,
        |_| Vec::new(),
    );
    assert_eq!(
        rendues,
        Vec::new(),
        "le repli n'a rien produit : la fonction n'invente pas d'ordre à sa place — {rendues:?}"
    );
}

// ---------------------------------------------------------------------------
// 3 — tout le boîtier, animation qui se colore : elle continue, changée de couleur
// ---------------------------------------------------------------------------

#[test]
fn tout_le_boitier_sous_une_animation_qui_se_colore_relance_l_animation() {
    // Critère d'acceptation : « `comete` global, une couleur posée sur tout le boîtier : il part
    // `animate comete couleur=…`, **pas** `light all` ».
    //
    // C'est la seconde incohérence relevée par l'issue : la vitesse et la direction repassent par
    // `Reglage::commande()` et renvoient l'animation, la couleur envoyait `light` et la tuait.
    // Trois réglages du même panneau, deux comportements — ici on en fait un seul.
    for nom_anim in CATALOGUE.iter().filter(|nom| se_colore(nom)) {
        let journal = Journal::neuf();
        let rendues = emises(
            Some(nom_anim),
            None,
            NOM_ENTIERE,
            &entiere(),
            true,
            &journal,
        );

        assert_eq!(
            rendues.len(),
            1,
            "relancer « {nom_anim} » sur tout le boîtier tient en une requête : {rendues:?}"
        );
        let Relance { nom: porte, paires } = relance(&rendues[0]).unwrap_or_else(|| {
            panic!(
                "une couleur posée pendant « {nom_anim} » doit relancer l'animation, pas la \
                 remplacer : {:?} reçu",
                rendues[0]
            )
        });
        assert_eq!(
            porte.as_deref(),
            Some(*nom_anim),
            "l'animation repart **sous son propre nom** : ni éteinte, ni changée pour une autre — \
             {:?}",
            rendues[0]
        );

        // Ce qui repart, c'est l'animation dans la **nouvelle** couleur, à la vitesse et dans la
        // direction affichées. Relire par le décodeur du démon, c'est vérifier d'un coup que la
        // commande est jouable et qu'elle porte bien les trois.
        let lu = relu(nom_anim, paires);
        assert_eq!(
            lu,
            Reglages {
                couleur: POSEE,
                vitesse: VITESSE,
                direction: DIRECTION,
            },
            "« {nom_anim} » doit repartir dans la couleur posée, sans perdre la vitesse ni la \
             direction affichées — {paires:?}"
        );

        // Le repli n'a rien à faire ici : une animation tourne, il n'y a pas de couleur fixe à
        // poser. Une implémentation qui calculerait les deux et choisirait ensuite passerait une
        // égalité ; elle ne passe pas un compte d'appels.
        assert_eq!(
            journal.appels(),
            Vec::new(),
            "« {nom_anim} » tourne : le repli `light`/`paint` n'est pas consulté"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — tout le boîtier sous une animation qui refuse la couleur : elle s'arrête, faute de mieux
// ---------------------------------------------------------------------------

#[test]
fn tout_le_boitier_sous_une_animation_sans_couleur_pose_la_couleur_fixe() {
    // Test d'intention de l'issue : « une couleur sur tout le boîtier, `arc-en-ciel` en cours, émet
    // `light` — parce qu'il n'y a pas d'autre choix, et **pas parce qu'on aurait oublié le cas** ».
    //
    // La nuance est tout le test. « Oublier le cas », c'est passer `couleur=` à une animation qui
    // la refuse : le démon rejette alors la ligne **entière**, et l'éclairage ne bouge pas d'un
    // pixel — un clic sans effet, sans message. C'est pourquoi ce test ne se contente pas de
    // constater un `light` : il exige que les deux branches **diffèrent**, ce qu'une
    // implémentation étourdie ne produit pas.
    for nom_anim in CATALOGUE.iter().filter(|nom| !se_colore(nom)) {
        let rendues = emises_sans_journal(Some(nom_anim), None, NOM_ENTIERE, &entiere(), true);

        assert_eq!(
            rendues,
            vec![Request::Light {
                target: LightTarget::All,
                color: POSEE,
            }],
            "« {nom_anim} » ne sait pas se colorer : le boîtier entier se fige dans la couleur \
             demandée, et rien d'autre ne part"
        );
    }

    // La preuve que le cas est **traité** et non oublié : l'autre branche, sur la même sélection,
    // ne rend pas la même chose. Deux implémentations se distinguent ici — celle qui décide, et
    // celle qui envoie `light` pour tout le monde.
    let colorable = emises_sans_journal(Some(AVEC_COULEUR), None, NOM_ENTIERE, &entiere(), true);
    let sans = emises_sans_journal(Some(SANS_COULEUR), None, NOM_ENTIERE, &entiere(), true);
    assert_ne!(
        colorable, sans,
        "une animation qui se colore et une qui ne le sait pas ne peuvent pas recevoir la même \
         salve : « {AVEC_COULEUR} » continue, « {SANS_COULEUR} » se fige — {colorable:?}"
    );
}

// ---------------------------------------------------------------------------
// 5 — sélection partielle : rien ne part sur la couche globale
// ---------------------------------------------------------------------------

#[test]
fn une_selection_partielle_sous_animation_ne_touche_jamais_la_couche_globale() {
    // Critère d'acceptation : « `arc-en-ciel` global, une couleur posée sur une sélection
    // partielle : les cibles hors sélection **gardent leur animation** — testable sur les requêtes
    // émises ».
    //
    // ⚠️ Ce critère ne se lit **pas** sur l'étendue des LED touchées. « haut-milieu » est un organe
    // entier : `light fan:haut-milieu` ne vise rien hors de la sélection, et c'est exactement la
    // requête qui a figé les treize autres cibles. Ce qui compte, c'est la **couche** : le démon
    // arrête l'animation dès qu'on écrit sur la couche globale, quelle que soit l'étendue de ce
    // qu'on y écrit.
    //
    // D'où deux assertions distinctes, sur les cibles de chaque requête émise :
    //   1. aucune requête de la couche globale — c'est elle qui éteint ;
    //   2. aucune zone qui déborde de la sélection — une zone trop large gèlerait, elle aussi, des
    //      LED que personne n'a sélectionnées.
    for nom_anim in CATALOGUE {
        for (nom, cibles) in [(NOM_PARTIELLE, partielle()), (NOM_MELEE, melee())] {
            let journal = Journal::neuf();
            let rendues = emises(Some(nom_anim), None, nom, &cibles, false, &journal);
            let vue = portee(&rendues);
            let selection: BTreeSet<Led> = cibles.iter().copied().collect();

            // Le compte qui suit n'est **pas** celui des LED visées hors sélection : il est nul
            // pour `light fan:haut-milieu`, et c'est bien là le piège. C'est le compte des LED que
            // le boîtier perd, parce qu'écrire sur la couche globale y arrête l'animation.
            let toutes: BTreeSet<Led> = Led::toutes().into_iter().collect();
            assert!(
                vue.globaux.is_empty(),
                "« {nom_anim} » tourne et l'utilisateur n'a sélectionné que « {nom} » : rien ne \
                 doit partir sur la couche globale, qui arrête l'animation du boîtier entier. \
                 Reçu {:?} — les {} LED hors sélection se figeraient, quand bien même aucune n'est \
                 visée.",
                vue.globaux,
                toutes.difference(&selection).count()
            );

            for (zone, enfermees) in &vue.cibles_de_zone {
                let debordement: Vec<String> = enfermees
                    .difference(&selection)
                    .map(|led| led.slug())
                    .collect();
                assert!(
                    debordement.is_empty(),
                    "la zone « {zone} » enferme des LED que « {nom} » ne contient pas, et qui \
                     perdraient donc l'animation du boîtier : {debordement:?}"
                );
            }

            // Le repli n'est pas consulté : c'est lui qui produit le `light` fautif.
            assert_eq!(
                journal.appels(),
                Vec::new(),
                "« {nom_anim} » tourne sur « {nom} » : le repli `light`/`paint` n'a rien à faire ici"
            );

            // Et quelque chose part quand même — un silence poli laisserait la sélection dans la
            // couleur de l'animation, ce qui est le clic sans effet, pas la correction.
            assert!(
                !rendues.is_empty(),
                "poser une couleur sur « {nom} » pendant « {nom_anim} » doit produire des \
                 requêtes, pas un silence"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 6 — sélection partielle, animation qui se colore : la zone rejoue la même
// ---------------------------------------------------------------------------

#[test]
fn une_selection_partielle_sous_une_animation_qui_se_colore_la_rejoue_en_zone() {
    // Critère d'acceptation : « `comete` global, une couleur posée sur une sélection partielle : il
    // part de quoi donner à cette sélection `comete` dans la nouvelle couleur ».
    //
    // C'est la demande de Nico, mot pour mot : « je veux que si je change la couleur d'une zone,
    // l'animation continue de se produire sur cette zone, mais de la couleur que j'ai choisie ».
    for nom_anim in CATALOGUE.iter().filter(|nom| se_colore(nom)) {
        for (nom, cibles) in [(NOM_PARTIELLE, partielle()), (NOM_MELEE, melee())] {
            let rendues = emises_sans_journal(Some(nom_anim), None, nom, &cibles, false);
            let vue = portee(&rendues);

            // La zone porte le nom que la sélection se donne. C'est de ce déterminisme que vient
            // « deux couleurs successives ne créent qu'une zone » ; ici on vérifie qu'aucune autre
            // zone n'est nommée en chemin — en nommer une seconde retirerait ses LED à celle qui
            // les tenait (README : une LED appartient à au plus une zone).
            assert_eq!(
                vue.zones,
                BTreeSet::from([nom.to_owned()]),
                "la sélection « {nom} » se donne son nom à elle-même et à aucune autre : {rendues:?}"
            );
            assert_eq!(
                vue.cibles_de_zone.get(nom).cloned().unwrap_or_default(),
                cibles.iter().copied().collect::<BTreeSet<Led>>(),
                "la zone « {nom} » doit enfermer exactement les LED sélectionnées : {rendues:?}"
            );

            // Une seule requête colore la zone, et c'est un `zone anim` qui rejoue l'animation.
            let porteuses: Vec<&Request> = rendues
                .iter()
                .filter(|r| matches!(r, Request::ZoneAnim { .. } | Request::ZoneLight { .. }))
                .collect();
            assert_eq!(
                porteuses.len(),
                1,
                "une seule requête donne sa couleur à « {nom} » : {rendues:?}"
            );
            let Relance { nom: porte, paires } = relance(porteuses[0]).unwrap_or_else(|| {
                panic!(
                    "« {nom_anim} » se colore : la zone « {nom} » doit la **rejouer**, pas se \
                     figer — {:?} reçu",
                    porteuses[0]
                )
            });
            assert_eq!(
                porte.as_deref(),
                Some(*nom_anim),
                "la zone rejoue l'animation du boîtier, sous son nom : {:?}",
                porteuses[0]
            );
            assert_eq!(
                relu(nom_anim, paires),
                Reglages {
                    couleur: POSEE,
                    vitesse: VITESSE,
                    direction: DIRECTION,
                },
                "la zone reprend l'animation dans la couleur posée, à la vitesse et dans la \
                 direction du boîtier — deux allures côte à côte se verraient — {paires:?}"
            );

            // `zone set` d'abord : une zone qui n'existe pas encore ne se colore pas.
            let rang_set = rendues
                .iter()
                .position(|r| matches!(r, Request::ZoneSet { .. }))
                .unwrap_or_else(|| {
                    panic!(
                        "la sélection « {nom} » n'est pas encore une zone : il faut la déclarer — \
                         {rendues:?}"
                    )
                });
            let rang_couleur = rendues
                .iter()
                .position(|r| matches!(r, Request::ZoneAnim { .. } | Request::ZoneLight { .. }))
                .expect("la requête porteuse vient d'être trouvée");
            assert!(
                rang_set < rang_couleur,
                "« zone set » doit précéder ce qui colore la zone, sinon le démon colore une zone \
                 qui n'existe pas : {rendues:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 7 — sélection partielle, animation qui refuse la couleur : la zone se fige
// ---------------------------------------------------------------------------

#[test]
fn une_selection_partielle_sous_une_animation_sans_couleur_recoit_une_couleur_fixe() {
    // Critère d'acceptation : « `arc-en-ciel` global, une couleur posée sur une sélection
    // partielle : la sélection reçoit une **couleur fixe**, jamais `arc-en-ciel couleur=…` que le
    // démon refuserait ».
    //
    // C'est le cas que Nico a observé sans s'en plaindre : sa sélection s'est bien figée dans la
    // couleur demandée. Son grief portait sur **les autres**, et c'est le test 5 qui le couvre.
    for nom_anim in CATALOGUE.iter().filter(|nom| !se_colore(nom)) {
        for (nom, cibles) in [(NOM_PARTIELLE, partielle()), (NOM_MELEE, melee())] {
            let rendues = emises_sans_journal(Some(nom_anim), None, nom, &cibles, false);

            assert!(
                rendues.contains(&Request::ZoneLight {
                    nom: nom.to_owned(),
                    couleur: POSEE,
                }),
                "« {nom_anim} » ne sait pas se colorer : « {nom} » se fige dans la couleur \
                 demandée — {rendues:?}"
            );
            assert!(
                !rendues.iter().any(|r| matches!(
                    r,
                    Request::ZoneAnim {
                        animation: Some(_),
                        ..
                    }
                )),
                "aucune animation ne doit être posée sur « {nom} » : « {nom_anim} » n'y prendrait \
                 pas la couleur voulue — {rendues:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 8 — aucune requête ne porte une clé que l'animation refuse
// ---------------------------------------------------------------------------

#[test]
fn aucune_requete_ne_porte_une_cle_que_l_animation_refuse() {
    // Test d'intention de l'issue : « aucune requête `zone anim` ne porte `couleur` pour une
    // animation qui la refuse ».
    //
    // La faute ne dégrade pas la commande, elle l'**annule** : `reverb-anim` refuse la ligne
    // entière sur une clé de trop, et l'éclairage ne bouge pas. Le symptôme est le clic sans effet
    // — le défaut le plus silencieux du lot, puisque la fenêtre a l'air d'avoir obéi.
    //
    // Balayage complet du catalogue, des trois sélections **et des deux couches** : la faute ne se
    // voit pas sur l'animation d'exemple, qui accepte tout, et elle ne se voit pas non plus sur la
    // seule couche globale. Depuis que la zone visée reçoit elle aussi l'animation (voir le point 4
    // de l'en-tête), c'est un `zone anim arc-en-ciel couleur=…` qui deviendrait possible — refusé
    // par le démon exactement comme l'`animate` qui lui correspond.
    for nom_anim in CATALOGUE {
        for zone_visee in [None, Some(ZONE_VISEE)] {
            for (nom, cibles, est_entiere) in [
                (NOM_ENTIERE, entiere(), true),
                (NOM_PARTIELLE, partielle(), false),
                (NOM_MELEE, melee(), false),
            ] {
                let rendues =
                    emises_sans_journal(Some(nom_anim), zone_visee, nom, &cibles, est_entiere);

                for requete in &rendues {
                    let Some(Relance { nom: porte, paires }) = relance(requete) else {
                        continue;
                    };
                    let Some(porte) = porte else {
                        panic!(
                            "poser une couleur ne doit jamais **éteindre** une animation : \
                             « animate off » reçu pour « {nom} » sous « {nom_anim} », zone visée \
                             {zone_visee:?} — {rendues:?}"
                        );
                    };
                    let acceptees = Animation::par_nom(&porte)
                        .unwrap_or_else(|e| panic!("« {porte} » doit être au catalogue : {e}"))
                        .parametres_acceptes();

                    for (cle, _) in paires {
                        assert!(
                            acceptees.contains(&cle.as_str()),
                            "« {porte} » n'accepte que {acceptees:?} : la clé « {cle} » ferait \
                             refuser la commande entière, et le clic n'aurait aucun effet — zone \
                             visée {zone_visee:?}, sélection « {nom} », {paires:?}"
                        );
                    }

                    // Pas deux fois la même clé : le protocole les transporte telles quelles, et
                    // une paire en double laisse au démon le soin de choisir laquelle compte.
                    for (i, (cle, _)) in paires.iter().enumerate() {
                        assert!(
                            !paires.iter().skip(i + 1).any(|(autre, _)| autre == cle),
                            "« {cle} » est portée deux fois pour « {porte} » : {paires:?}"
                        );
                    }

                    // Et le démon doit accepter la ligne : c'est son propre décodeur qui le dit.
                    relu(&porte, paires);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 9 — deux couleurs successives sur la même sélection ne nomment qu'une zone
// ---------------------------------------------------------------------------

#[test]
fn deux_couleurs_successives_sur_la_meme_selection_ne_nomment_qu_une_zone() {
    // Critère d'acceptation : « deux couleurs successives sur la même sélection ne créent qu'une
    // zone ».
    //
    // C'est une propriété du **nom** : il est tiré de la sélection, donc déterministe, donc la
    // seconde salve réécrit la zone que la première a posée. Un nom horodaté, numéroté ou tiré au
    // hasard empilerait une zone par clic — et comme une LED n'appartient qu'à une zone à la fois
    // (README), le boîtier finirait découpé en autant de zones mortes que de gestes.
    //
    // Deux appels suffisent à le montrer, à condition de faire varier ce qui pourrait servir de
    // graine : la couleur, et l'animation en cours.
    for nom_anim in CATALOGUE {
        for (nom, cibles) in [(NOM_PARTIELLE, partielle()), (NOM_MELEE, melee())] {
            let premiere = emises_sans_journal(Some(nom_anim), None, nom, &cibles, false);

            // La seconde pose une autre couleur — c'est le geste répété de l'utilisateur qui
            // cherche sa teinte, celui qui empilerait les zones.
            let seconde = requetes_pour_la_couleur(
                &reglage(Some(nom_anim)),
                ANCIENNE,
                None,
                nom,
                &cibles,
                false,
                |couleur| requetes_d_aujourd_hui(&cibles, couleur),
            );

            let mut nommees = portee(&premiere).zones;
            nommees.extend(portee(&seconde).zones);
            assert_eq!(
                nommees,
                BTreeSet::from([nom.to_owned()]),
                "deux couleurs posées sur « {nom} » pendant « {nom_anim} » ne nomment qu'une zone, \
                 celle de la sélection : {nommees:?}"
            );
        }
    }

    // Et le nom est stable d'une animation à l'autre : changer d'animation entre deux gestes ne
    // doit pas non plus faire naître une seconde zone sur les mêmes LED.
    let sous_comete = portee(&emises_sans_journal(
        Some(AVEC_COULEUR),
        None,
        NOM_PARTIELLE,
        &partielle(),
        false,
    ))
    .zones;
    let sous_arc = portee(&emises_sans_journal(
        Some(SANS_COULEUR),
        None,
        NOM_PARTIELLE,
        &partielle(),
        false,
    ))
    .zones;
    assert_eq!(
        sous_comete, sous_arc,
        "le nom de la zone vient de la sélection, pas de l'animation en cours : {sous_comete:?} \
         contre {sous_arc:?}"
    );
}

// ---------------------------------------------------------------------------
// 10 — une zone visée reçoit la couleur sous la forme que l'animation permet
// ---------------------------------------------------------------------------

#[test]
fn une_zone_visee_recoit_la_couleur_sous_la_forme_que_l_animation_permet() {
    // Demande de l'utilisateur, mot pour mot : « quand une animation tourne, je veux que si je
    // change la couleur **d'une zone**, l'animation continue de se produire sur cette zone, mais
    // de la couleur que j'ai choisie ». Il dit « zone » : une zone visée est exactement ce cas.
    //
    // Le critère d'acceptation de l'issue — « continue de recevoir la couleur **directement** » —
    // se lit volontiers « toujours `zone light` ». Il ne l'est pas ici, et le point 4 de l'en-tête
    // dit pourquoi : l'acquis de #47 qu'il invoque porte sur `requetes_vers_la_cible`, qui ne
    // reçoit pas l'animation en cours et n'a donc jamais rien spécifié de ce cas.
    //
    // Ce que « directement » garde de force, et que les deux invariants du bas exigent : jamais de
    // seconde zone, jamais de repli.
    for animation in [None, Some(AVEC_COULEUR), Some(SANS_COULEUR)] {
        // La forme n'est pas affaire de sélection : quoi qu'il y ait sous la souris, c'est la zone
        // visée qui reçoit. Balayer les trois sélections, c'est vérifier qu'aucune ne détourne la
        // couleur au passage.
        for (nom, cibles, est_entiere) in [
            (NOM_ENTIERE, entiere(), true),
            (NOM_PARTIELLE, partielle(), false),
            (NOM_MELEE, melee(), false),
        ] {
            let journal = Journal::neuf();
            let rendues = emises(
                animation,
                Some(ZONE_VISEE),
                nom,
                &cibles,
                est_entiere,
                &journal,
            );
            let contexte = format!(
                "zone visée « {ZONE_VISEE} », sélection « {nom} », animation {animation:?}"
            );

            // La règle du fichier : la couleur va à la couche visée, sous la forme que l'animation
            // permet. Une animation qui se colore se rejoue ; les deux autres cas se figent.
            let attendue = match animation {
                Some(nom_anim) if se_colore(nom_anim) => Request::ZoneAnim {
                    nom: ZONE_VISEE.to_owned(),
                    animation: Some(nom_anim.to_owned()),
                    reglages: Animation::par_nom(nom_anim)
                        .unwrap_or_else(|e| panic!("« {nom_anim} » est au catalogue : {e}"))
                        .reglages_ecrits(&Reglages {
                            couleur: POSEE,
                            vitesse: VITESSE,
                            direction: DIRECTION,
                        }),
                },
                // `arc-en-ciel` produit ses propres teintes : « dans la couleur choisie » n'y a
                // pas de sens, et la lui passer ferait refuser la ligne entière. Elle se fige.
                // Sans animation, c'est le cas de #47, et il ne bouge pas.
                _ => Request::ZoneLight {
                    nom: ZONE_VISEE.to_owned(),
                    couleur: POSEE,
                },
            };
            assert_eq!(
                rendues,
                vec![attendue],
                "une couleur posée sur une zone visée vaut une requête et une seule, portée par \
                 cette zone — {contexte}"
            );

            // Invariant 1 — le repli n'est **jamais** consulté quand une zone est visée. Compter
            // les appels et pas seulement comparer les sorties, c'est ce qui distingue une
            // implémentation qui choisit d'une implémentation qui calcule les deux branches et
            // jette la mauvaise : la seconde passerait l'égalité ci-dessus.
            assert_eq!(
                journal.appels(),
                Vec::new(),
                "le repli `light`/`paint` du boîtier n'a rien à faire ici : la zone est visée — \
                 {contexte}"
            );

            // Invariant 2 — jamais de seconde zone. La zone visée existe et tient déjà ses LED ;
            // en déclarer une autre par-dessus les lui prendrait (README : une LED appartient à au
            // plus une zone), et la zone visée se retrouverait vide. C'est aussi ce qui interdit
            // que la sélection sous la souris se nomme au passage.
            let vue = portee(&rendues);
            assert_eq!(
                vue.zones,
                BTreeSet::from([ZONE_VISEE.to_owned()]),
                "seule la zone visée est nommée, et aucune autre n'est déclarée en chemin — \
                 {contexte}"
            );
            assert!(
                !rendues.iter().any(|r| matches!(r, Request::ZoneSet { .. })),
                "la zone visée existe déjà : en déclarer une seconde lui prendrait ses LED — \
                 {contexte}"
            );

            // Invariant 3 — rien ne part sur la couche globale. C'est le défaut de #63 lui-même :
            // une couleur posée sur une zone ne doit pas figer le boîtier autour d'elle.
            assert!(
                vue.globaux.is_empty(),
                "rien ne doit partir sur la couche globale quand une zone est visée : elle \
                 arrêterait l'animation du boîtier entier. Reçu {:?} — {contexte}",
                vue.globaux
            );
        }
    }

    // Et la forme change bien avec l'animation, sans quoi « sous la forme que l'animation permet »
    // ne voudrait rien dire : une implémentation qui figerait toujours la zone — l'ancienne
    // lecture du critère — passerait tout ce qui précède sauf ceci.
    let sous_comete = emises_sans_journal(
        Some(AVEC_COULEUR),
        Some(ZONE_VISEE),
        NOM_PARTIELLE,
        &partielle(),
        false,
    );
    let sans_animation =
        emises_sans_journal(None, Some(ZONE_VISEE), NOM_PARTIELLE, &partielle(), false);
    assert_ne!(
        sous_comete, sans_animation,
        "une zone visée pendant « {AVEC_COULEUR} » doit **rejouer** l'animation, pas se figer \
         comme quand rien ne tourne — {sous_comete:?}"
    );
}
