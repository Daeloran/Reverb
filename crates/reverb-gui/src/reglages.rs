//! Ce que valent les deux curseurs, et quand ils parlent au démon.
//!
//! Ce module n'existe que parce que les deux curseurs de la fenêtre ont chacun
//! un défaut, et qu'aucun des deux ne se teste dans du Slint : la logique en
//! sort pour être vérifiable.
//!
//! # Le curseur de vitesse ne disait rien
//!
//! Il n'écrivait qu'une propriété de l'interface. Rien ne renvoyait l'animation,
//! donc le démon gardait la vitesse d'avant jusqu'au prochain clic sur le bouton
//! de l'animation. [`Reglage::commande`] est la réponse : à chaque changement,
//! elle dit s'il y a une commande à envoyer, et laquelle.
//!
//! # La poignée d'un ventilateur sautait sous les doigts
//!
//! Sa valeur était liée à la mesure, que la télémétrie réécrit chaque seconde.
//! Or un ventilateur met plusieurs secondes à rejoindre une consigne : pendant
//! ce temps, la mesure **contredit** ce que l'utilisateur vient de demander, et
//! la poignée revenait en arrière toute seule.
//!
//! [`Poignee`] arbitre : la consigne l'emporte tant qu'elle est fraîche, la
//! mesure reprend la main ensuite. C'est un état par canal, pas un verrou
//! global — deux ventilateurs se règlent indépendamment.
//!
//! # La fenêtre ne savait pas ce qui tournait déjà
//!
//! Le démon rétablit l'éclairage au démarrage : quand la fenêtre s'ouvre, une
//! animation tourne le plus souvent déjà. La fenêtre l'ignorait, donc
//! [`Reglage::commande`] rendait `None` et le curseur de vitesse redevenait
//! muet — le défaut d'au-dessus, revenu par une autre porte. [`eclairage_lu`]
//! et [`Reglage::adopter`] sont la réponse : la fenêtre lit ce que le démon
//! rapporte avant de prétendre le piloter.

use std::time::Duration;

use reverb_anim::{Animation, CATALOGUE, Direction, PALETTES, Palette, ReglageInvalide, Reglages};
use reverb_proto::composition::{Ancre, Fond, Source};
use reverb_proto::ipc::{ProfilAction, Request, ResponseLine, ScreenAction};
use reverb_proto::regulation::Courbe;
use reverb_proto::{Led, NomInvalide, NomProfil, Rgb};

use crate::telemetrie::trace_de_courbe;

/// Combien de temps une consigne l'emporte sur la mesure après le relâchement.
///
/// Un ventilateur de 140 mm met environ deux secondes à passer d'un régime à un
/// autre, et la télémétrie arrive chaque seconde. Sous ce délai, la poignée
/// reviendrait en arrière sous les yeux de celui qui vient de la poser.
pub const GRACE: Duration = Duration::from_secs(3);

/// Les réglages d'animation tels que la fenêtre les affiche.
#[derive(Debug, Clone, PartialEq)]
pub struct Reglage {
    /// L'animation en cours, `None` si aucune ne tourne.
    pub animation: Option<String>,
    pub couleur: Rgb,
    pub vitesse: u8,
    /// Rang dans `reverb_anim::Direction::ALL`.
    pub direction: usize,
    /// Rang dans [`reverb_anim::PALETTES`], ou `None` pour « aucune » (#126).
    ///
    /// ⚠️ **Un rang, pas une `Palette`** : c'est ce que le menu de la fenêtre
    /// manipule — `current-index` d'un `ComboBox` —, et convertir au bord évite
    /// de porter deux représentations du même choix.
    pub palette: Option<usize>,
}

impl Reglage {
    /// La commande à envoyer pour que le démon applique ces réglages.
    ///
    /// `None` quand aucune animation ne tourne : bouger la vitesse à vide ne
    /// doit rien envoyer, sinon la fenêtre démarrerait une animation que
    /// personne n'a demandée.
    ///
    /// Les clés portées sont **celles que l'animation accepte**, et elles
    /// seules : `arc-en-ciel` refuse `couleur`, et la lui donner ferait refuser
    /// la commande entière.
    pub fn commande(&self) -> Option<Request> {
        let nom = self.animation.as_ref()?;
        let animation = Animation::par_nom(nom).ok()?;
        let acceptees = animation.parametres_acceptes();
        let mut reglages = Vec::new();
        // ⚠️ **`palette` chasse `couleur`, jamais les deux ensemble** : le démon
        // refuse le couple, et la commande **entière** avec (#126).
        match self.palette.and_then(|rang| PALETTES.get(rang)) {
            Some(palette) if acceptees.contains(&"palette") => {
                reglages.push(("palette".to_owned(), (*palette).to_owned()));
            }
            // Un rang hors des douze n'envoie **rien** plutôt qu'une palette de
            // repli, pour la même raison qu'une direction hors des huit : une
            // teinte que personne n'a demandée, et sans un mot.
            Some(_) => {}
            None => {
                if acceptees.contains(&"couleur") {
                    reglages.push(("couleur".to_owned(), hexa(self.couleur)));
                }
            }
        }
        if acceptees.contains(&"vitesse") {
            reglages.push(("vitesse".to_owned(), self.vitesse.to_string()));
        }
        if acceptees.contains(&"direction") {
            // Un rang hors des six n'envoie **rien**. Le repli sur une
            // direction par défaut ferait tourner l'animation dans un sens que
            // personne n'a demandé, et sans rien dire.
            let direction = Direction::ALL.get(self.direction)?;
            reglages.push(("direction".to_owned(), direction.slug().to_owned()));
        }
        Some(Request::Animate {
            name: Some(nom.clone()),
            reglages,
        })
    }

    /// Adopte ce que le démon rapporte. Rend vrai s'il y a eu changement.
    ///
    /// **L'animation nommée décide, et elle seule.** Tant que le démon rapporte
    /// celle que la fenêtre pilote déjà, rien n'est adopté : les réponses
    /// arrivent chaque seconde, et adopter à chaque fois ferait sauter le
    /// curseur de vitesse sous les doigts de celui qui le tire — le défaut
    /// qu'on a corrigé sur les poignées de ventilateur, refait à l'identique.
    ///
    /// Quand le nom change, en revanche, tout ce que le démon rapporte est
    /// adopté ; ce qu'il ne rapporte pas garde sa valeur. `arc-en-ciel` n'a pas
    /// de couleur, et lui en inventer une déplacerait le sélecteur pour rien.
    ///
    /// Conséquence assumée : un réglage changé **hors** de la fenêtre sur
    /// l'animation en cours — un `animate comete vitesse=9` par le socket — ne
    /// remonte pas. Le prix à payer pour que le curseur tienne en place.
    pub fn adopter(&mut self, venu: &Eclairage) -> bool {
        if venu.animation == self.animation {
            return false;
        }
        self.animation = venu.animation.clone();
        // Chaque réglage séparément : un `Eclairage` sans couleur n'est pas un
        // `Eclairage` noir. Écraser par le vide remettrait le curseur de vitesse
        // à zéro — hors des bornes du catalogue — dès que le démon ne s'est pas
        // prononcé, ce qu'`arc-en-ciel` fait à chaque fois.
        if let Some(couleur) = venu.couleur {
            self.couleur = couleur;
        }
        if let Some(vitesse) = venu.vitesse {
            self.vitesse = vitesse;
        }
        if let Some(direction) = venu.direction {
            self.direction = direction;
        }
        // ⚠️ **Affecté même absent**, contrairement aux trois autres. Une
        // animation rapportée sans `palette` en est une qui n'en a pas : garder
        // celle d'avant afficherait un menu qui ment sur ce que le boîtier
        // montre. Les autres réglages, eux, ne sont *pas rapportés* quand
        // l'animation ne les accepte pas — absence d'information, pas
        // information d'absence.
        self.palette = venu.palette;
        true
    }
}

/// Ce qu'une réponse `lighting` dit de l'animation en cours.
///
/// Chaque réglage est facultatif parce que le démon n'écrit que les clés que
/// l'animation accepte. Un réglage absent n'est pas un réglage à zéro : c'est
/// un réglage sur lequel le démon ne s'est pas prononcé, et que la fenêtre doit
/// donc laisser tel qu'il est.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Eclairage {
    /// Le nom de l'animation, **tel que le démon l'a écrit**, même s'il est
    /// inconnu du catalogue : la fenêtre montre ce que le démon dit plutôt
    /// qu'un « aucune » qui serait faux. [`Reglage::commande`] refusera de son
    /// côté d'en tirer une commande.
    pub animation: Option<String>,
    pub couleur: Option<Rgb>,
    pub vitesse: Option<u8>,
    /// Rang dans [`Direction::ALL`].
    pub direction: Option<usize>,
    /// Rang dans [`reverb_anim::PALETTES`] (#126).
    pub palette: Option<usize>,
}

/// Lit une réponse à `lighting`.
///
/// Une réponse sans ligne `anim` dit qu'aucune animation ne tourne — c'est une
/// information, pas une absence d'information : la fenêtre doit repasser sur
/// « aucune ».
///
/// Une valeur qu'on ne sait pas relire est **ignorée**, jamais remplacée par
/// une valeur de repli : une vitesse de repli tirerait le curseur ailleurs que
/// là où le boîtier tourne vraiment, sans rien dire.
pub fn eclairage_lu(lignes: &[ResponseLine]) -> Eclairage {
    let mut lu = Eclairage::default();
    for ligne in lignes {
        let ResponseLine::Anim { nom, reglages } = ligne else {
            continue;
        };
        // La dernière ligne `anim` l'emporte **entièrement**, réglages remis à
        // zéro compris : une couleur héritée de la ligne d'avant décrirait un
        // éclairage qui n'a jamais existé, et il serait adopté comme tel.
        lu = Eclairage {
            animation: Some(nom.clone()),
            ..Eclairage::default()
        };
        for (cle, valeur) in reglages {
            // Une affectation, jamais un `get_or_insert` : une clé répétée
            // garde sa dernière écriture, y compris quand cette dernière est
            // illisible. Retenir l'avant-dernière valeur *valide* rapporterait
            // une valeur que le démon n'a pas écrite en dernier.
            match cle.as_str() {
                "couleur" => lu.couleur = couleur_lue(valeur),
                "vitesse" => lu.vitesse = vitesse_lue(valeur),
                "direction" => lu.direction = direction_lue(valeur),
                "palette" => lu.palette = palette_lue(valeur),
                // Une clé inconnue est ignorée, et elle seule : c'est ce qui
                // permet à cette fenêtre de rester lisible devant un démon plus
                // récent qu'elle. Refuser la ligne entière n'appartient qu'au
                // démon, seul à savoir ce qu'une animation accepte.
                _ => {}
            }
        }
    }
    lu
}

/// Six chiffres hexadécimaux, sans `#`, tels que le démon les écrit.
///
/// Volontairement plus strict que `Rgb::from_hex`, qui tolère le `#` parce
/// qu'un humain le tape : ici l'émetteur est un programme, et accepter deux
/// écritures d'une même couleur, c'est se priver de l'aller-retour exact.
fn couleur_lue(brut: &str) -> Option<Rgb> {
    if brut.len() != 6 || !brut.bytes().all(|o| o.is_ascii_hexdigit()) {
        return None;
    }
    Rgb::from_hex(brut).ok()
}

/// Le rang d'une palette du catalogue, par son nom (#126).
///
/// Un nom inconnu rend `None` : une fenêtre plus ancienne que le démon doit
/// rester lisible devant une palette qu'elle ne connaît pas, et le bandeau de
/// l'animation dit de toute façon la vérité.
fn palette_lue(brut: &str) -> Option<usize> {
    PALETTES.iter().position(|nom| *nom == brut)
}

/// Une vitesse du catalogue, de 1 à 10.
///
/// Les bornes sont recopiées ici parce que `reverb-anim` les garde pour lui.
/// Ce n'est pas une reprise qui dérive en silence : `spec_eclairage_lu.rs` les
/// fait confirmer par `Animation::reglages`, qui est le juge de l'autre côté du
/// socket, et tombe le jour où l'un des deux bouge sans l'autre.
fn vitesse_lue(brut: &str) -> Option<u8> {
    brut.parse::<u8>().ok().filter(|v| (1..=10).contains(v))
}

/// Le rang d'une direction dans [`Direction::ALL`], depuis son slug.
///
/// La direction voyage en **mot** sur le socket et en **rang** dans la fenêtre.
fn direction_lue(brut: &str) -> Option<usize> {
    Direction::ALL.iter().position(|d| d.slug() == brut)
}

fn hexa(couleur: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", couleur.r, couleur.g, couleur.b)
}

/// L'état d'une poignée de ventilateur : consigne contre mesure.
///
/// Un état **par canal**, jamais partagé : deux ventilateurs se règlent
/// indépendamment, et un état global ferait sauter la poignée de l'un quand on
/// tire celle de l'autre.
#[derive(Debug, Clone, PartialEq)]
pub struct Poignee {
    /// La dernière mesure venue du démon.
    ///
    /// Zéro avant la première : la fenêtre n'a alors rien d'autre à montrer, et
    /// la première réponse à `status` arrive dans la seconde.
    mesure: u8,
    /// La consigne posée par l'utilisateur, tant qu'elle a la main.
    consigne: Option<u8>,
    /// Vrai tant que la poignée est tenue.
    tenue: bool,
    /// Quand elle a été relâchée, pour compter la grâce **depuis le
    /// relâchement** et non depuis la saisie : un geste long ne raccourcit pas
    /// le délai qu'il faut au ventilateur pour rejoindre sa consigne.
    relachee: Option<Duration>,
    /// La consigne pas encore envoyée au démon.
    en_attente: Option<u8>,
}

impl Poignee {
    /// Une poignée qui n'a encore rien vu : elle suit la mesure.
    pub fn nouvelle() -> Poignee {
        Poignee {
            mesure: 0,
            consigne: None,
            tenue: false,
            relachee: None,
            en_attente: None,
        }
    }

    /// L'utilisateur tire la poignée à cette valeur.
    ///
    /// Reposer la poignée là où elle est déjà **n'est pas un pas** : sans cette
    /// règle, une interface qui appelle `saisir` à chaque image respecterait la
    /// lettre de « au plus une commande par pas » tout en inondant le démon.
    pub fn saisir(&mut self, consigne: u8, _maintenant: Duration) {
        if self.consigne != Some(consigne) {
            self.consigne = Some(consigne);
            self.en_attente = Some(consigne);
        }
        self.tenue = true;
        self.relachee = None;
    }

    /// Il la lâche. La consigne tient encore [`GRACE`].
    pub fn relacher(&mut self, maintenant: Duration) {
        self.tenue = false;
        self.relachee = Some(maintenant);
    }

    /// Une mesure arrive du démon.
    ///
    /// C'est le seul endroit où la grâce peut expirer, et elle est comptée
    /// depuis le relâchement : une mesure ne relance pas le compte, sinon la
    /// télémétrie qui arrive chaque seconde le relancerait indéfiniment.
    pub fn mesurer(&mut self, pwm: u8, maintenant: Duration) {
        self.mesure = pwm;
        if self.tenue {
            return;
        }
        // À `relâchement + GRACE` pile, la consigne a **fini** de tenir.
        if let Some(depuis) = self.relachee
            && maintenant.saturating_sub(depuis) >= GRACE
        {
            self.consigne = None;
            self.relachee = None;
        }
    }

    /// Le canal repasse à la courbe du firmware : la mesure reprend la main
    /// tout de suite, sans attendre la fin de la grâce.
    ///
    /// Une consigne encore en attente est **annulée** : partie après le
    /// « fan auto », elle ressortirait le canal de l'automatique qu'on vient de
    /// demander.
    pub fn liberer(&mut self) {
        self.consigne = None;
        self.relachee = None;
        self.tenue = false;
        self.en_attente = None;
    }

    /// Ce que le curseur doit afficher.
    pub fn affichee(&self) -> u8 {
        self.consigne.unwrap_or(self.mesure)
    }

    /// La consigne à envoyer au démon, une seule fois par valeur.
    ///
    /// Rend `Some` au premier appel qui suit un changement de consigne, `None`
    /// ensuite : un glissement continu produit une commande par pas franchi,
    /// pas une par image.
    pub fn a_envoyer(&mut self) -> Option<u8> {
        self.en_attente.take()
    }
}

/// Ce qu'une ligne de canal écrit à côté de sa barre de consigne (issue #102).
///
/// # Le défaut que ceci corrige
///
/// Une barre sans repère chiffré rend une classe entière de défauts
/// **invisible**. Le bouton « auto » du Kraken a été cru sans effet : il en avait
/// un, très mauvais — il mettait la consigne à 0 % —, mais rien à l'écran ne
/// montrait la valeur, donc rien ne distinguait « sans effet » de « effet
/// désastreux ». Il a fallu lire sysfs à la main pour le voir.
///
/// ⚠️ **Le nombre vient de [`Poignee::affichee`], et de nulle part ailleurs.**
/// C'est ce qui garde le texte et la barre d'accord pendant la saisie comme
/// pendant la grâce : un texte branché sur la télémétrie brute afficherait 30 %
/// pendant qu'on tire la poignée à 80, la barre et son chiffre se
/// contrediraient, et on croirait la consigne perdue.
///
/// `mesure` est le `pwm` de la dernière ligne `chan` reçue du démon, `None`
/// quand il a rendu le canal illisible. **Son absence dit que le canal ne répond
/// pas ; sa valeur n'écrit jamais le nombre** — [`Poignee::mesurer`] l'a déjà
/// portée.
///
/// ⚠️ **Un canal muet sans consigne n'écrit aucun chiffre.** Il ne reste alors
/// qu'une mesure périmée, et l'afficher serait le mode de défaillance le plus
/// coûteux du projet parce qu'il est rassurant : c'est déjà la règle du cadran,
/// « une sonde muette affiche des tirets, jamais un zéro ni la dernière valeur
/// connue ». Le texte reste **non vide** pour la même raison — une case vide se
/// lit comme une ligne sans consigne, pas comme un canal qui ne répond pas.
///
/// ⚠️ **Une consigne tirée sur un canal muet garde son chiffre, marqué d'un
/// `?`.** La barre le montre de toute façon : le cacher ferait diverger le texte
/// de la poignée juste au-dessus, c'est-à-dire refaire le défaut d'à côté. Ce
/// que le `?` dit, c'est que rien ne l'a confirmée.
///
/// L'unité est écrite, et c'est le `%` qui sépare cette grandeur du régime en
/// tours par minute affiché sur la même ligne de canal : un nombre nu à côté
/// d'un autre nombre nu ferait croire qu'une consigne à 50 % donne 50 % du
/// régime maximal.
pub fn consigne_affichee(poignee: &Poignee, mesure: Option<u8>) -> String {
    match (mesure, poignee.consigne) {
        (Some(_), _) => format!("{} %", poignee.affichee()),
        (None, Some(_)) => format!("{} % ?", poignee.affichee()),
        (None, None) => "-- %".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Le limiteur de débit des jauges de couleur (issue #47)
// ---------------------------------------------------------------------------

/// Délai minimal entre deux couleurs envoyées au démon.
///
/// **Cinquante millisecondes, soit vingt par seconde.** C'est la cadence du
/// démon quand ses quatorze cibles changent — 29,5 ms de trames HID plus 21,6 ms
/// de blocs SMBus, un plancher **physique** et non logiciel (README). Poser une
/// couleur unie sur tout le boîtier est exactement ce cas-là : rien ne sert
/// d'émettre plus vite que le bus ne prend.
pub const INTERVALLE: Duration = Duration::from_millis(50);

/// Ce qui empêche une jauge traînée d'inonder le démon.
///
/// Une jauge émet des dizaines de valeurs par seconde ; le démon en encaisse
/// vingt quand tout change. Le limiteur laisse passer la première, retient les
/// suivantes, et **garde la dernière** — c'est elle qui compte, puisque c'est
/// sur elle que le doigt s'arrête.
///
/// Le temps est un **paramètre**, jamais lu depuis l'horloge : c'est ce qui rend
/// ce type vérifiable sans attendre, et reproductible à l'identique.
#[derive(Debug, Clone, Default)]
pub struct Limiteur {
    /// La dernière couleur réellement envoyée.
    posee: Option<Rgb>,
    /// Quand elle est partie. `None` tant que rien n'est parti — un limiteur
    /// neuf n'a aucune raison de retenir, et faire démarrer chaque glissement
    /// par un temps mort serait l'impression même que #47 corrige.
    dernier_envoi: Option<Duration>,
    /// La couleur proposée pendant la retenue, s'il y en a une.
    attente: Option<Rgb>,
}

impl Limiteur {
    pub fn nouveau() -> Limiteur {
        Limiteur::default()
    }

    /// Propose une couleur. Rend celle à envoyer, ou `None` si elle attend.
    pub fn proposer(&mut self, couleur: Rgb, maintenant: Duration) -> Option<Rgb> {
        self.attente = Some(couleur);
        self.a_envoyer(maintenant)
    }

    /// Ce qu'il reste à envoyer, si l'intervalle le permet.
    ///
    /// Respecte l'intervalle **comme `proposer`** : sans cela, « au plus une
    /// requête par intervalle » ne serait plus une propriété du limiteur mais
    /// une politesse de son appelant. L'attente n'est jamais perdue pour
    /// autant — elle attend, et l'horloge d'une seconde de la fenêtre la trouve
    /// toujours largement au-delà.
    pub fn a_envoyer(&mut self, maintenant: Duration) -> Option<Rgb> {
        let couleur = self.attente?;
        // Déjà posée : il n'y a rien de nouveau à dire. Le matériel n'a pas de
        // watchdog, réécrire une couleur identique ne fait que consommer du bus.
        if self.posee == Some(couleur) {
            self.attente = None;
            return None;
        }
        // `>=` et non `>` : l'intervalle est un délai **minimal** entre deux
        // envois, et à l'instant précis où il est écoulé la couleur part.
        if let Some(precedent) = self.dernier_envoi
            && maintenant.saturating_sub(precedent) < INTERVALLE
        {
            return None;
        }
        self.attente = None;
        self.posee = Some(couleur);
        self.dernier_envoi = Some(maintenant);
        Some(couleur)
    }
}

/// Où va une couleur : à la zone visée, ou au boîtier entier.
///
/// ⚠️ **Le repli est une fermeture, et il n'est pas consulté quand une zone est
/// visée.** C'est ce qui rend la faute de #47 observable — « ça passe tout en
/// fixe et couleur unie » alors qu'une zone était visée : une implémentation qui
/// calculerait les deux branches et choisirait ensuite passerait un test
/// d'égalité, mais pas un test qui compte les appels.
pub fn requetes_vers_la_cible(
    zone: Option<&str>,
    couleur: Rgb,
    sans_zone: impl FnOnce(Rgb) -> Vec<Request>,
) -> Vec<Request> {
    match zone {
        Some(nom) => vec![Request::ZoneLight {
            nom: nom.to_owned(),
            couleur,
        }],
        None => sans_zone(couleur),
    }
}

/// Ce que la couche visée peut faire de la couleur qu'on vient de poser.
///
/// Trois états et non deux : « rien ne tourne » et « ça tourne mais refuse la
/// couleur » se comportent pareil partout **sauf** sur une sélection partielle,
/// où le premier se contente du repli et le second doit passer par une zone pour
/// que le reste du boîtier garde son animation.
enum Couche {
    /// Aucune animation exploitable : la couleur se pose telle quelle.
    ///
    /// Un nom que le catalogue ne connaît pas compte ici. Le démon peut en
    /// rapporter un que cette fenêtre ignore ; en tirer une relance émettrait
    /// une commande qu'elle ne sait pas composer, là où le repli fait ce qu'elle
    /// a toujours fait.
    Fixe,
    /// Une animation tourne et accepte la couleur : elle se rejoue, changée.
    Rejouee(Request),
    /// Une animation tourne et refuse la couleur — `arc-en-ciel` les produit
    /// toutes. Elle cède la place à la couleur demandée, sur la couche visée
    /// seulement.
    Figee,
}

/// L'animation en cours, relancée dans la couleur qu'on vient de poser.
///
/// ⚠️ **La couleur vient du sélecteur, pas du réglage.** Le réglage porte encore
/// celle d'avant le geste : relancer depuis lui rendrait un `animate` valide,
/// accepté par le démon, et sans le moindre effet visible.
///
/// Les clés portées sont celles que l'animation accepte, et [`Reglage::commande`]
/// les filtre déjà — c'est le catalogue qui décide, jamais une liste recopiée ici
/// qui divergerait à la première animation ajoutée.
fn couche_visee(reglage: &Reglage, couleur: Rgb) -> Couche {
    let Some(nom) = reglage.animation.as_deref() else {
        return Couche::Fixe;
    };
    let Ok(animation) = Animation::par_nom(nom) else {
        return Couche::Fixe;
    };
    if !animation.parametres_acceptes().contains(&"couleur") {
        return Couche::Figee;
    }
    let relance = Reglage {
        couleur,
        ..reglage.clone()
    };
    match relance.commande() {
        Some(requete) => Couche::Rejouee(requete),
        None => Couche::Fixe,
    }
}

/// La même requête, adressée à une zone plutôt qu'au boîtier.
fn pour_la_zone(nom: &str, requete: Request) -> Request {
    match requete {
        Request::Animate { name, reglages } => Request::ZoneAnim {
            nom: nom.to_owned(),
            animation: name,
            reglages,
        },
        autre => autre,
    }
}

/// Où va une couleur, et sous quelle forme (issue #63).
///
/// **Une seule règle** : *la couleur va à la couche visée, sous la forme que
/// l'animation en cours permet.* [`requetes_vers_la_cible`] en devient un cas —
/// celui où rien ne tourne.
///
/// # Le défaut que ceci corrige
///
/// Boîtier en `arc-en-ciel`, l'utilisateur sélectionne `haut-milieu` et lui donne
/// une couleur : les treize autres cibles se figeaient. La fenêtre émettait
/// `light`, et une couleur fixe arrête l'animation côté démon — à raison sur la
/// couche globale, où les deux se disputent les mêmes LED, à tort ici où rien ne
/// dispute `bas-gauche` à qui vient de colorer `haut-milieu`.
///
/// ⚠️ **Ce n'est pas l'étendue des LED touchées qui compte, c'est la couche.**
/// `haut-milieu` est un ventilateur entier : `light fan:haut-milieu` ne vise
/// aucune LED hors sélection, et fige pourtant les cent seize autres.
///
/// ⚠️ **Le repli n'est jamais consulté quand la couleur va ailleurs**, et ce
/// n'est pas une économie : le calculer puis le jeter rendrait le bon résultat
/// tout en peignant les LED d'un organe entamé, qui ne sont pas à peindre.
pub fn requetes_pour_la_couleur(
    reglage: &Reglage,
    couleur: Rgb,
    zone_visee: Option<&str>,
    selection: &str,
    cibles: &[Led],
    entiere: bool,
    sans_animation: impl FnOnce(Rgb) -> Vec<Request>,
) -> Vec<Request> {
    // Une sélection partielle pendant qu'une animation tourne devient une zone :
    // c'est le seul moyen de lui donner sa couleur sans que le reste du boîtier
    // perde la sienne. Le nom vient de la sélection, donc il est le même à chaque
    // geste — sans ce déterminisme, chaque clic empilerait une zone de plus.
    let en_zone = zone_visee.is_none() && !entiere;
    let declarer = || Request::ZoneSet {
        nom: selection.to_owned(),
        cibles: cibles.to_vec(),
    };

    match couche_visee(reglage, couleur) {
        Couche::Rejouee(requete) => match zone_visee {
            // Une zone visée tient déjà ses LED : la redéclarer les lui
            // prendrait, une LED n'appartenant qu'à une zone à la fois.
            Some(zone) => vec![pour_la_zone(zone, requete)],
            // Tout le boîtier : la couche globale est exactement ce qu'on vise,
            // et il n'y a rien à préserver à côté.
            None if entiere => vec![requete],
            // Déclarer d'abord : une zone qui n'existe pas encore ne se colore
            // pas.
            None => vec![declarer(), pour_la_zone(selection, requete)],
        },

        // L'animation refuse la couleur — `arc-en-ciel` les produit toutes — et
        // la sélection est partielle : elle cède la place, mais sur sa zone
        // seulement.
        Couche::Figee if en_zone => vec![
            declarer(),
            Request::ZoneLight {
                nom: selection.to_owned(),
                couleur,
            },
        ],

        // Tout le reste, c'est le comportement d'avant #63, mot pour mot :
        // `requetes_vers_la_cible` **en devient un cas**, elle n'est pas doublée.
        // Rien ne tourne, ou rien ne se rejoue et la couche visée est de toute
        // façon celle qu'une couleur fixe emporterait.
        _ => requetes_vers_la_cible(zone_visee, couleur, sans_animation),
    }
}

// ---------------------------------------------------------------------------
// Ce que la fenêtre offre, et ce qu'un choix envoie (issue #76)
// ---------------------------------------------------------------------------

/// Les familles d'animation que la fenêtre propose.
///
/// [`CATALOGUE`] tel quel, et c'est tout l'intérêt : une liste recopiée dans le
/// `.slint` serait restée à six quand #75 en a livré quatre de plus, sans
/// qu'aucun message ne le dise — le grief qui ouvre l'issue.
pub fn familles_offertes() -> Vec<&'static str> {
    CATALOGUE.to_vec()
}

/// Les directions que la fenêtre propose, dans l'ordre de [`Direction::ALL`].
///
/// ⚠️ **L'ordre est le contrat**, pas une commodité : [`Reglage::direction`] est
/// un **rang** dans cette liste. Un ordre à soi ferait choisir une direction
/// pour une autre, et sans le moindre message — les cent vingt-quatre couleurs
/// rendues resteraient parfaitement plausibles.
pub fn directions_offertes() -> Vec<Direction> {
    Direction::ALL.to_vec()
}

/// La requête qui pose ce que le panneau d'animation affiche.
///
/// Deux différences avec [`Reglage::commande`], et elles font le tour de #76 :
///
/// - **la sonde entre par un paramètre.** `thermique` l'exige, et `Reglage`
///   n'a pas à porter un réglage que neuf familles sur dix refusent — lui
///   ajouter un cinquième champ casserait par ailleurs les littéraux de deux
///   fichiers de tests d'intention voisins ;
/// - **le refus est rendu, jamais avalé.** `commande` répond `None`, ce qui
///   convient à un curseur qu'on bouge à vide et pas du tout à un clic :
///   `thermique` sans sonde laisserait un panneau qui ne fait rien, exactement
///   le défaut de #32.
///
/// Les clés portées **et** le refus viennent tous deux de `reverb-anim`, par le
/// même code que le démon : la fenêtre refuse ce que le démon refuserait,
/// plutôt que par une liste recopiée qui divergerait à la prochaine famille.
pub fn requete_d_animation(
    reglage: &Reglage,
    sonde: Option<&str>,
) -> Result<Request, ReglageInvalide> {
    // « aucune » est un geste, pas une absence d'information : c'est le seul
    // moyen d'éteindre, et le protocole n'en a qu'un.
    let Some(nom) = reglage.animation.as_deref() else {
        return Ok(Request::Animate {
            name: None,
            reglages: Vec::new(),
        });
    };
    let animation = Animation::par_nom(nom).map_err(|erreur| ReglageInvalide {
        cle: "animation".to_owned(),
        raison: erreur.to_string(),
        acceptees: &[],
    })?;

    let acceptees = animation.parametres_acceptes();
    // Le rang n'est vérifié que s'il va partir. Refuser sur lui pour `pouls`,
    // qui ne suit aucune direction, bloquerait un panneau parfaitement valide.
    let direction = if acceptees.contains(&"direction") {
        *Direction::ALL
            .get(reglage.direction)
            .ok_or_else(|| ReglageInvalide {
                cle: "direction".to_owned(),
                raison: format!(
                    "le rang {} ne désigne aucune des {} directions",
                    reglage.direction,
                    Direction::ALL.len()
                ),
                acceptees,
            })?
    } else {
        Reglages::default().direction
    };

    // `reglages_ecrits` ne garde que les clés acceptées : une sonde choisie
    // pour `thermique` ne suit pas jusqu'à `comete`, où elle ferait refuser la
    // commande **entière**.
    let reglages = animation.reglages_ecrits(&Reglages {
        couleur: reglage.couleur,
        vitesse: reglage.vitesse,
        direction,
        sonde: sonde.map(str::to_owned),
        // ⚠️ `reglages_ecrits` écrit `couleur` **ou** `palette`, jamais les deux
        // (#126) : c'est lui qui tranche, pas cet appelant.
        palette: reglage
            .palette
            .and_then(|rang| PALETTES.get(rang))
            .and_then(|nom| Palette::par_nom(nom).ok()),
    });
    // Relu par le juge d'en face avant de partir : c'est lui qui nomme la sonde
    // manquante, et le dire ici épargne un aller-retour pour l'apprendre.
    animation.reglages(&reglages)?;

    Ok(Request::Animate {
        name: Some(nom.to_owned()),
        reglages,
    })
}

/// Ce qu'un geste sur la barre des profils demande (#74).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoixDeProfil {
    Lister,
    Enregistrer(String),
    Rappeler(String),
    Oublier(String),
}

/// La requête qu'un geste de la barre des profils produit.
///
/// ⚠️ **Le nom est validé ici, par [`NomProfil`] — le type du démon.** Une
/// fenêtre qui laisserait passer « ../etc/passwd » se ferait refuser à
/// l'arrivée, une seconde plus tard, par un message qui ne dirait rien de plus.
/// Le refuser tout de suite, par le même code, c'est la garantie de #74 tenue
/// des deux côtés du socket plutôt que d'un seul.
pub fn requete_de_profil(choix: &ChoixDeProfil) -> Result<Request, NomInvalide> {
    Ok(Request::Profil(match choix {
        ChoixDeProfil::Lister => ProfilAction::List,
        ChoixDeProfil::Enregistrer(nom) => ProfilAction::Save(NomProfil::nouveau(nom)?),
        ChoixDeProfil::Rappeler(nom) => ProfilAction::Load(NomProfil::nouveau(nom)?),
        ChoixDeProfil::Oublier(nom) => ProfilAction::Drop(NomProfil::nouveau(nom)?),
    }))
}

/// Ce qu'un geste sur le panneau de composition demande (#80).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoixDeComposition {
    /// Ce que la dalle compose en ce moment.
    Etat,
    Fond(Fond),
    Champ(Ancre, Source),
    Vide(Ancre),
    /// Retour à l'affichage simple.
    Aucune,
}

/// La requête qu'un geste du panneau de composition produit.
///
/// Rien à refuser ici, et c'est voulu : le chemin d'un fond et le libellé d'un
/// champ sont **les derniers champs de leur ligne** (#80), donc rien n'y est
/// ambigu. Ce qui pourrait être refusé — une sonde inconnue, un cinquième champ
/// — ne l'est que par le démon, seul à savoir quelles sondes existent et ce que
/// la composition porte déjà.
pub fn requete_de_composition(choix: &ChoixDeComposition) -> Request {
    Request::Screen(match choix {
        ChoixDeComposition::Etat => ScreenAction::LayoutState,
        ChoixDeComposition::Fond(fond) => ScreenAction::LayoutFond(fond.clone()),
        ChoixDeComposition::Champ(ancre, source) => {
            ScreenAction::LayoutChamp(*ancre, source.clone())
        }
        ChoixDeComposition::Vide(ancre) => ScreenAction::LayoutVide(*ancre),
        ChoixDeComposition::Aucune => ScreenAction::LayoutOff,
    })
}

// ---------------------------------------------------------------------------
// Ce que la fenêtre compose pour l'écran (issue #48)
// ---------------------------------------------------------------------------

/// Les affichages du menu de l'écran, dans l'ordre.
///
/// « composition » est arrivée avec #80, et elle est la seule dont l'argument ne
/// tient pas sur une ligne : un fond et jusqu'à quatre champs. Le démon la
/// nomme d'un seul jeton, `layout`, et détaille sur les lignes qui suivent.
pub const AFFICHAGES: [&str; 5] = ["rien", "cadran", "image", "gif", "composition"];

/// Le rang de « composition » dans [`AFFICHAGES`].
pub const COMPOSITION: usize = 4;

/// Le rang de « rien », qui est aussi le repli.
///
/// C'est le seul rang dont le sens reste vrai quand la fenêtre ne sait pas
/// nommer ce que le démon affiche.
const RIEN: usize = 0;

/// L'affichage d'écran que la fenêtre montre, et la poignée qui le protège.
///
/// # Le défaut
///
/// La fenêtre demande `screen state` chaque seconde et réécrivait le rang du
/// menu depuis la réponse. Tant que « Appliquer » n'a pas été cliqué, le démon
/// répond encore `rien` — la sélection en cours était donc effacée sous les
/// doigts, avec le chemin ou le nom de sonde qu'on était en train de taper.
///
/// C'est le même défaut que celui de la poignée de ventilateur et que celui de
/// l'animation (#41) : **une valeur que l'utilisateur compose ne se laisse pas
/// écraser par un relevé.**
///
/// ⚠️ **La luminosité, elle, est toujours adoptée.** Elle part à chaque cran et
/// n'est jamais composée : la retenir la ferait sauter en arrière.
#[derive(Debug, Clone, Default)]
pub struct EcranChoisi {
    /// Rang dans [`AFFICHAGES`].
    pub affichage: usize,
    /// Chemin de fichier ou nom de sonde, selon l'affichage.
    pub argument: String,
    /// En pour cent.
    pub luminosite: u8,
    /// L'utilisateur est en train de composer un affichage.
    compose: bool,
}

impl EcranChoisi {
    /// L'utilisateur compose : la poignée se lève.
    pub fn saisir(&mut self) {
        self.compose = true;
    }

    /// « Appliquer » ou « Annuler » : elle retombe.
    pub fn relacher(&mut self) {
        self.compose = false;
    }

    pub fn compose(&self) -> bool {
        self.compose
    }

    /// Range un relevé du démon. Rend `true` si quelque chose a changé.
    ///
    /// L'affichage se coupe de son argument au **premier** deux-points : un nom
    /// de sonde en contient — `kraken2023elite:coolant` — et couper au dernier
    /// tronquerait l'argument sans rien dire.
    pub fn adopter(&mut self, luminosite: u8, affichage: &str) -> bool {
        let mut change = self.luminosite != luminosite;
        self.luminosite = luminosite;

        if self.compose {
            return change;
        }

        let (quoi, argument) = affichage.split_once(':').unwrap_or((affichage, ""));
        // Le démon écrit `gauge:` là où la fenêtre affiche « cadran », et
        // `layout` là où elle affiche « composition » : le mot du protocole d'un
        // côté, celui de l'utilisateur de l'autre.
        let attendu = match quoi {
            "gauge" => "cadran",
            "layout" => "composition",
            autre => autre,
        };
        // ⚠️ Un affichage inconnu retombe sur « rien » **et vide l'argument** :
        // laisser un argument orphelin sous « rien » afficherait un chemin que
        // plus rien n'explique.
        let (rang, argument) = match AFFICHAGES.iter().position(|nom| *nom == attendu) {
            Some(RIEN) | None => (RIEN, ""),
            Some(rang) => (rang, argument),
        };

        change |= self.affichage != rang || self.argument != argument;
        self.affichage = rang;
        self.argument = argument.to_owned();
        change
    }
}

/// La courbe de régulation telle que l'éditeur la montre (issue #113).
///
/// Même arbitrage que [`Reglage::adopter`], et pour la même raison : le démon
/// rend sa courbe à chaque tour, une fois par seconde, et la recopier sans
/// condition remettrait les poignées en place au milieu d'un réglage.
///
/// ⚠️ **Ce n'est pas une mémoire de fenêtre pour autant.** Ce qui s'affiche est
/// toujours ce que le démon a rendu, sauf tant qu'un geste est en cours. Un
/// « Appliquer » remet les deux d'accord ; jusque-là, ce que l'éditeur montre est
/// ce que l'utilisateur vient de poser, et non un état inventé.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CourbeEditee {
    /// Les paliers que l'éditeur montre, en `(millidegrés, pourcent)`.
    paliers: Vec<(i32, u8)>,
    /// Les derniers paliers rendus par le démon, `None` avant le premier tour.
    recue: Option<Vec<(i32, u8)>>,
}

impl CourbeEditee {
    /// Les paliers en cours d'édition.
    pub fn paliers(&self) -> &[(i32, u8)] {
        &self.paliers
    }

    /// Reprend la courbe que le démon vient de rendre. Vrai s'il faut redessiner.
    ///
    /// Ne reprend rien tant que l'éditeur porte autre chose que ce que le démon
    /// avait rendu la dernière fois : c'est le signe qu'un réglage est en cours,
    /// et le lui reprendre effacerait les paliers sous les doigts.
    pub fn adopter(&mut self, paliers: &[(i32, u8)]) -> bool {
        if self.recue.as_deref() == Some(paliers) {
            return false;
        }
        let intacte = self
            .recue
            .as_ref()
            .is_none_or(|rendus| *rendus == self.paliers);
        self.recue = Some(paliers.to_vec());
        if !intacte {
            return false;
        }
        self.paliers = paliers.to_vec();
        true
    }

    /// Déplace la température d'un palier, en **degrés entiers**.
    ///
    /// ⚠️ **Le millidegré est l'unité, le degré n'est que la graduation de la
    /// poignée.** Une courbe rendue par le socket peut porter un demi-degré
    /// (`45500`) ; il reste intact tant que personne ne touche à *ce* palier-là.
    pub fn regler_temperature(&mut self, rang: usize, degres: i32) {
        if let Some((milli, _)) = self.paliers.get_mut(rang) {
            *milli = degres.saturating_mul(1_000);
        }
    }

    /// Déplace la consigne d'un palier, en pourcent.
    pub fn regler_consigne(&mut self, rang: usize, pourcent: u8) {
        if let Some((_, consigne)) = self.paliers.get_mut(rang) {
            *consigne = pourcent.min(100);
        }
    }
}

/// La plage sur laquelle le panneau trace la courbe, en millidegrés.
///
/// Elle **déborde des deux côtés** des paliers par défaut — 20 °C est sous le
/// premier, 70 °C au-dessus du dernier —, et c'est tout l'intérêt : c'est là que
/// l'écrêtage se voit. Une plage collée aux paliers montrerait une droite qui
/// monte, sans jamais dire ce que la courbe fait sur un circuit qui démarre.
pub const TRACE_FROID: i32 = 20_000;
/// L'autre borne de [`TRACE_FROID`].
pub const TRACE_CHAUD: i32 = 70_000;
/// Un point par degré : la courbe est faite de segments, et un point par palier
/// suffirait à la dessiner — mais pas à prouver qu'elle vient de `consigne`.
pub const TRACE_POINTS: usize = 51;

/// Le rapport largeur/hauteur dans lequel le tracé est émis.
///
/// ⚠️ **Slint met un `Path` à l'échelle *uniformément*** — par
/// `min(largeur / viewbox_largeur, hauteur / viewbox_hauteur)` — et il **ne
/// rogne pas** au viewbox. Un carré unité dessiné dans un cadre de 359 × 88 px
/// n'occupe donc que 88 px de large, centré : **24 % du cadre**, mesuré au pixel
/// le 2026-08-15 sur l'image d'aperçu, les deux plateaux comprimés avec la
/// rampe. Et ne corriger que le `viewbox-height` fait **déborder** la courbe par
/// le haut au lieu de l'étirer, puisque rien ne la rogne — essayé, mesuré aussi.
///
/// Le tracé doit donc être émis dans un espace qui a **déjà** le rapport du
/// cadre, et le cadre tenir ce rapport. Cette constante les accorde, et elle vit
/// ici — non dans le `.slint` — pour n'exister qu'une fois : la fenêtre la lit
/// dans `trace-aspect` et en déduit **et** son `viewbox` **et** sa hauteur. Deux
/// chiffres à tenir d'accord finiraient par diverger, et le symptôme serait
/// celui qu'on vient de corriger — un tracé faux que rien ne signale.
pub const TRACE_ASPECT: f32 = 4.0;

/// Une température en millidegrés, telle qu'elle s'écrit dans le panneau.
///
/// ⚠️ **Le millidegré reste l'unité**, et ceci n'est qu'un affichage : le dixième
/// de degré est arrondi vers le bas pour la lecture, jamais dans la valeur qui
/// part sur le socket. Un palier à 45 500 s'écrit « 45,5 °C » et repart 45 500.
pub fn degres_lisibles(milli: i32) -> String {
    let entier = milli.div_euclid(1_000);
    let dixiemes = milli.rem_euclid(1_000) / 100;
    if dixiemes == 0 {
        format!("{entier} °C")
    } else {
        format!("{entier},{dixiemes} °C")
    }
}

/// Le tracé d'une courbe, en commandes SVG sur `TRACE_ASPECT` × 1.
///
/// ⚠️ **Chaque point vient de [`trace_de_courbe`], donc de `Courbe::consigne`** —
/// la fonction que le démon exécute. Une boucle écrite ici serait une seconde
/// interpolation, et elle n'aurait aucune raison d'être la bonne : l'aperçu
/// montrerait une courbe que le boîtier n'applique pas (#113).
///
/// ⚠️ **`x` court sur `0..=TRACE_ASPECT`, et non sur le carré unité**, pour la
/// raison écrite sur [`TRACE_ASPECT`] : Slint ne sait pas étirer un `Path`.
pub fn commandes_de_trace(courbe: &Courbe) -> String {
    let largeur = f32::from(u16::try_from(TRACE_CHAUD - TRACE_FROID).unwrap_or(u16::MAX));
    trace_de_courbe(courbe, TRACE_FROID, TRACE_CHAUD, TRACE_POINTS)
        .into_iter()
        .enumerate()
        .map(|(rang, (milli, consigne))| {
            let avance = f32::from(u16::try_from(milli - TRACE_FROID).unwrap_or(0)) / largeur;
            let x = avance * TRACE_ASPECT;
            let y = 1.0 - f32::from(consigne) / 100.0;
            format!("{} {x:.4} {y:.4} ", if rang == 0 { "M" } else { "L" })
        })
        .collect()
}
