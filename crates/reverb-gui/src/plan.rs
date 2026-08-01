//! Où dessiner les cent vingt-quatre LED du boîtier, sous deux points de vue.
//!
//! # Deux vues, et une seule dit la vérité
//!
//! [`Vue::Face`] regarde le boîtier depuis le panneau latéral gauche : l'arrière
//! à gauche, l'avant à droite, le haut en haut. C'est le point de vue dans
//! lequel la géométrie a été relevée (`docs/GEOMETRIE.md`) et celui depuis
//! lequel l'utilisateur « lit » ses ventilateurs. Elle **écrase la largeur** du
//! boîtier — l'axe qui va de la vitre au plateau de carte mère — et sept des dix
//! ventilateurs y sont vus par la tranche. Un dessin fidèle en ferait sept
//! traits, où aucune LED ne serait cliquable : ils sont donc dessinés **en
//! cercles quand même**. C'est un schéma, pas une photographie.
//!
//! [`Vue::Isometrique`] regarde le même boîtier de trois-quarts, et **projette
//! les positions réelles** : aucune LED n'y est replacée à la main. Les trois
//! plans occupés s'y distinguent — la RAM cesse d'être confondue avec le
//! radiateur — et aucun ventilateur n'y est vu par la tranche, chacun des trois
//! plans donnant une ellipse franche. C'est la seule des deux qui ne mente pas ;
//! son prix est que les anneaux n'y sont plus des cercles, et que les LED des
//! barrettes, écartées de 4 mm dans la vraie vie, y restent minuscules.
//!
//! La division du travail est donc : la vue de face pour peindre LED par LED, la
//! vue isométrique pour comprendre où les choses sont.
//!
//! # Deux écarts de la vue de face, que la mesure impose
//!
//! - **Les anneaux sont dessinés un peu plus petits que nature.** Entre deux
//!   centres, il faut faire tenir les deux anneaux **et** deux diamètres de LED,
//!   sans quoi deux LED voisines se recouvrent et l'une devient incliquable. Le
//!   rayon n'est donc plus une constante choisie mais un calcul : voir
//!   [`rayon_dessine`]. Il vaut aujourd'hui les quatre cinquièmes du rayon réel,
//!   et c'est la paire « arrière ↔ plafond arrière » qui commande — deux
//!   ventilateurs montés dans des plans perpendiculaires, donc légitimement
//!   proches.
//! - **Les barrettes sont dessinées plus grandes que nature.** Onze LED espacées
//!   de 4 mm ne se cliquent pas. Le **bloc reste à sa place réelle** : seul son
//!   écartement est grossi, autour de son propre milieu.
//!
//! Depuis #27, la RAM n'a plus besoin d'être décalée : le radiateur est parti à
//! l'avant du boîtier et lui a laissé le milieu.

use reverb_anim::{Geometrie, Point};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position};

/// Nombre de LED d'un ventilateur, comme indice.
const LEDS: usize = LEDS_PER_FAN as usize;

/// Ce qui doit tenir entre deux centres d'anneaux voisins, en rayons.
///
/// Deux anneaux — donc deux rayons — plus deux diamètres de LED, un diamètre
/// valant le quart du rayon. Soit `2 + 2/4 = 2,5`.
const ENCOMBREMENT: f32 = 2.5;

/// Un diamètre de LED, en fraction du rayon de l'anneau.
///
/// Huit LED réparties sur un anneau sont à `0,765 · R` l'une de l'autre : un
/// disque de `R/4` laisse donc entre deux voisines deux fois son propre
/// diamètre — visiblement séparées, et visables à la souris.
const DIAMETRE_LED: f32 = 0.25;

/// Écart entre deux LED d'une même barrette, en diamètres de LED.
const RAM_PAS_LED: f32 = 1.5;

/// Écart entre deux barrettes, en diamètres de LED.
const RAM_PAS_SLOT: f32 = 2.5;

/// Marge laissée entre la maquette et le bord du cadre.
const MARGE: f32 = 0.01;

/// Cosinus de trente degrés, l'inclinaison de la projection isométrique.
const COS30: f32 = 0.866_025_4;

/// Sinus de trente degrés.
const SIN30: f32 = 0.5;

/// Un point de la maquette, en coordonnées d'écran.
///
/// Normalisées de 0 à 1, `y` vers le bas — la convention de l'écran, pas celle
/// du boîtier. C'est la fenêtre qui les multiplie par sa taille du moment : un
/// plan en pixels serait faux dès qu'on redimensionne.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub x: f32,
    pub y: f32,
}

/// Ce qu'on désigne en cliquant sur la maquette.
///
/// Toujours **une LED**, quel que soit le niveau de détail affiché : viser un
/// ventilateur entier rend ses huit LED, pas un objet d'un autre genre. C'est ce
/// qui permet à une sélection d'être un simple ensemble de LED, et donc à une
/// zone d'en être un aussi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cible {
    Led { position: Position, led: usize },
    Barrette { slot: usize, led: usize },
}

/// Le point de vue depuis lequel la maquette est dessinée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vue {
    /// De face, depuis le panneau latéral gauche.
    ///
    /// La largeur du boîtier est **écrasée** : deux organes qui ne diffèrent que
    /// par elle se retrouvent au même endroit. Sept ventilateurs sur dix sont
    /// vus par la tranche et dessinés en cercles quand même — c'est un schéma,
    /// pas une photographie.
    Face,
    /// De trois-quarts.
    ///
    /// La seule vue **honnête** : les quatre plans occupés s'y distinguent, et
    /// aucun ventilateur n'y est vu par la tranche. Le prix est que les anneaux
    /// y sont des ellipses, et non des cercles.
    Isometrique,
}

/// Où dessiner chaque LED du boîtier.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    ventilateurs: [(Place, [Place; LEDS]); 10],
    barrettes: [[Place; LEDS_PER_STICK]; SLOT_COUNT],
    /// Les arêtes du volume, à dessiner derrière les LED.
    aretes: Vec<(Place, Place)>,
    rayon: f32,
    vue: Vue,
}

impl Plan {
    /// Projette une géométrie dans le repère « panneau gauche ».
    ///
    /// La géométrie donne des points en trois dimensions ; le plan n'en garde
    /// que deux. La largeur — l'axe qui va du panneau vers la carte mère —
    /// n'est pas dessinée. Voir [`Plan::isometrique`] pour la vue qui la garde.
    pub fn nouveau(geometrie: &Geometrie) -> Plan {
        let rayon = rayon_dessine(geometrie);
        let diametre = rayon * DIAMETRE_LED;

        // Les anneaux : des cercles au bon endroit, y compris pour les sept
        // ventilateurs que ce panneau voit par la tranche.
        let mut ventilateurs = [(ZERO, [ZERO; LEDS]); 10];
        for position in Position::ALL {
            let centre = geometrie.centre_ventilateur(position);
            let plat = de_face(centre);
            let orientation = geometrie.orientation(position);
            let mut anneau = [plat; LEDS];
            for (led, point) in anneau.iter_mut().enumerate() {
                let angle = f32::from(orientation.angle_led(led)).to_radians();
                *point = Place {
                    x: plat.x + rayon * angle.sin(),
                    y: plat.y - rayon * angle.cos(),
                };
            }
            ventilateurs[position.index()] = (plat, anneau);
        }

        // Les barrettes : à leur place réelle, mais écartées pour être
        // cliquables. Le grossissement se fait autour du milieu du bloc, qui ne
        // bouge donc pas.
        let milieu = milieu_de_la_ram(geometrie);
        let mut barrettes = [[ZERO; LEDS_PER_STICK]; SLOT_COUNT];
        for (slot, reglette) in barrettes.iter_mut().enumerate() {
            for (led, point) in reglette.iter_mut().enumerate() {
                *point = Place {
                    x: milieu.x - ecart(slot, SLOT_COUNT) * RAM_PAS_SLOT * diametre,
                    y: milieu.y - ecart(led, LEDS_PER_STICK) * RAM_PAS_LED * diametre,
                };
            }
        }

        // L'étendue d'un anneau de la vue de face **est** son rayon, dans les
        // deux directions : le dire plutôt que le déduire des huit LED garde le
        // cadre au bit près quand un ventilateur tourne sur place.
        cadrer(
            Plan {
                ventilateurs,
                barrettes,
                // Pas d'arêtes de face : les anneaux y sont dessinés plus petits
                // que nature, et une boîte à la bonne taille autour de
                // ventilateurs rapetissés ferait flotter le boîtier dans du
                // vide qui n'existe pas. La vue de face est un schéma, elle
                // n'encadre rien.
                aretes: Vec::new(),
                rayon,
                vue: Vue::Face,
            },
            [(rayon, rayon); 10],
        )
    }

    /// Le même boîtier, vu de trois-quarts.
    ///
    /// Contrairement à la vue de face, celle-ci **projette les positions
    /// réelles** des cent vingt-quatre LED : aucune n'est replacée à la main.
    /// C'est ce qui la rend honnête, et ce qui fait qu'aucun ventilateur n'y est
    /// vu par la tranche.
    pub fn isometrique(geometrie: &Geometrie) -> Plan {
        let mut ventilateurs = [(ZERO, [ZERO; LEDS]); 10];
        for position in Position::ALL {
            let centre = en_isometrie(geometrie.centre_ventilateur(position));
            let mut anneau = [centre; LEDS];
            for (led, point) in anneau.iter_mut().enumerate() {
                // `led_ventilateur` rend le point réel, dans le plan où le
                // ventilateur est monté : c'est lui qui donne l'ellipse, et non
                // un aplatissement appliqué après coup.
                if let Some(reel) = geometrie.led_ventilateur(position, led) {
                    *point = en_isometrie(reel);
                }
            }
            ventilateurs[position.index()] = (centre, anneau);
        }

        let mut barrettes = [[ZERO; LEDS_PER_STICK]; SLOT_COUNT];
        for (slot, reglette) in barrettes.iter_mut().enumerate() {
            for (led, point) in reglette.iter_mut().enumerate() {
                if let Some(reel) = geometrie.led_barrette(slot, led) {
                    *point = en_isometrie(reel);
                }
            }
        }

        // Le rayon vient de ce qui a été projeté, pas d'une constante : en
        // isométrie, un anneau est une ellipse, et « le rayon » n'est plus qu'un
        // ordre de grandeur.
        let mut etendues = [(0.0, 0.0); 10];
        for (place, anneau) in etendues.iter_mut().zip(&ventilateurs) {
            *place = etendue(anneau);
        }
        let rayon = etendues
            .iter()
            .map(|(large, haut)| (large + haut) / 2.0)
            .sum::<f32>()
            / 10.0;
        cadrer(
            Plan {
                ventilateurs,
                barrettes,
                aretes: aretes_du_volume(geometrie),
                rayon,
                vue: Vue::Isometrique,
            },
            etendues,
        )
    }

    /// Le centre d'un ventilateur.
    pub fn centre_ventilateur(&self, position: Position) -> Place {
        self.ventilateurs[position.index()].0
    }

    /// Une des huit LED d'un ventilateur, `None` au-delà.
    pub fn led_ventilateur(&self, position: Position, led: usize) -> Option<Place> {
        self.ventilateurs[position.index()].1.get(led).copied()
    }

    /// Une des onze LED d'une barrette, `None` au-delà.
    pub fn led_barrette(&self, slot: usize, led: usize) -> Option<Place> {
        self.barrettes.get(slot)?.get(led).copied()
    }

    /// Le milieu d'une barrette, `None` au-delà du quatrième slot.
    ///
    /// Ce qu'il faut pour dessiner une réglette d'un trait, sans passer par ses
    /// onze LED, quand la maquette est au détail « ventilateur ».
    /// ⚠️ Le milieu se prend **entre les deux extrémités**, pas en moyennant les
    /// onze. Les deux valent la même chose sur une réglette régulière — et elle
    /// l'est dans les deux vues, la projection isométrique étant affine — mais
    /// la moyenne de onze nombres identiques ne rend pas exactement ce nombre.
    /// Une barrette vue par la tranche a ses onze LED à la même abscisse, et le
    /// centre tombait alors juste à côté d'elles.
    pub fn centre_barrette(&self, slot: usize) -> Option<Place> {
        let reglette = self.barrettes.get(slot)?;
        let (premiere, derniere) = (reglette.first()?, reglette.last()?);
        Some(Place {
            x: (premiere.x + derniere.x) / 2.0,
            y: (premiere.y + derniere.y) / 2.0,
        })
    }

    /// Les arêtes du boîtier, deux points par arête.
    ///
    /// Vide en vue de face, douze en isométrie. C'est ce qui donne un volume à
    /// lire : sans elles, la vue de trois-quarts n'est qu'un nuage de points
    /// penché.
    pub fn aretes(&self) -> &[(Place, Place)] {
        &self.aretes
    }

    /// Depuis quel point de vue ce plan a été construit.
    pub fn vue(&self) -> Vue {
        self.vue
    }

    /// Toutes les LED de l'organe qui porte cette cible.
    ///
    /// Les huit d'un ventilateur, les onze d'une barrette. C'est ce qui fait le
    /// détail « ventilateur » : le geste reste un clic sur une LED, et c'est le
    /// niveau de détail qui décide s'il en entraîne sept autres.
    pub fn groupe(&self, cible: Cible) -> Vec<Cible> {
        match cible {
            Cible::Led { position, .. } => {
                (0..LEDS).map(|led| Cible::Led { position, led }).collect()
            }
            Cible::Barrette { slot, .. } => (0..LEDS_PER_STICK)
                .map(|led| Cible::Barrette { slot, led })
                .collect(),
        }
    }

    /// Le rayon d'un anneau de ventilateur, dans la même unité que [`Place`].
    ///
    /// Deux LED voisines doivent rester distinctes à l'écran : c'est ce rayon
    /// qui décide si on peut en cliquer une. En isométrie, où un anneau est une
    /// ellipse, c'est la distance **moyenne** du centre à ses LED — un ordre de
    /// grandeur, pas une dimension exacte.
    pub fn rayon_anneau(&self) -> f32 {
        self.rayon
    }

    /// Ce qui se trouve sous un point, s'il y a quelque chose.
    ///
    /// C'est la fonction du clic. Elle rend la LED **la plus proche** dans son
    /// rayon de prise, et `None` au-delà : cliquer dans le vide ne doit pas
    /// changer une couleur au hasard.
    ///
    /// La prise vaut un rayon d'anneau. Plus courte, l'aperçu deviendrait
    /// pénible — ce qui se voit tout de suite ; plus longue, un clic manqué
    /// changerait la couleur d'une LED qu'on ne visait pas — ce qu'on ne relie
    /// jamais au clic qui l'a causée.
    pub fn sous(&self, place: Place) -> Option<Cible> {
        // La fenêtre est plus grande que la maquette : un clic dans sa marge ne
        // doit pas se rabattre sur la LED la plus proche.
        if !(0.0..=1.0).contains(&place.x) || !(0.0..=1.0).contains(&place.y) {
            return None;
        }

        let mut meilleure: Option<(f32, Cible)> = None;
        for (cible, point) in self.toutes() {
            let ecart = distance(place, point);
            if ecart <= self.rayon && meilleure.is_none_or(|(pire, _)| ecart < pire) {
                meilleure = Some((ecart, cible));
            }
        }
        meilleure.map(|(_, cible)| cible)
    }

    /// Tout ce que le rectangle de deux coins attrape.
    ///
    /// Les deux coins sont donnés dans n'importe quel ordre — un glissement va
    /// dans les quatre sens. Une cible est retenue si **son centre** est dans le
    /// rectangle : un critère de recouvrement partiel attraperait des LED qu'on
    /// ne voit pas dedans.
    ///
    /// L'ordre du résultat est celui de la maquette, pas celui du geste : deux
    /// glissements qui couvrent la même zone rendent la même liste.
    pub fn dans(&self, coin: Place, oppose: Place) -> Vec<Cible> {
        let (gauche, droite) = (coin.x.min(oppose.x), coin.x.max(oppose.x));
        let (haut, bas) = (coin.y.min(oppose.y), coin.y.max(oppose.y));
        self.toutes()
            .filter(|(_, place)| {
                (gauche..=droite).contains(&place.x) && (haut..=bas).contains(&place.y)
            })
            .map(|(cible, _)| cible)
            .collect()
    }

    /// Les cent vingt-quatre LED et leur place, dans l'ordre de la maquette.
    fn toutes(&self) -> impl Iterator<Item = (Cible, Place)> + '_ {
        let anneaux = Position::ALL.into_iter().flat_map(move |position| {
            self.ventilateurs[position.index()]
                .1
                .iter()
                .enumerate()
                .map(move |(led, place)| (Cible::Led { position, led }, *place))
        });
        let reglettes = self.barrettes.iter().enumerate().flat_map(|(slot, barre)| {
            barre
                .iter()
                .enumerate()
                .map(move |(led, place)| (Cible::Barrette { slot, led }, *place))
        });
        anneaux.chain(reglettes)
    }
}

/// L'origine, pour initialiser les tableaux avant de les remplir.
const ZERO: Place = Place { x: 0.0, y: 0.0 };

/// Les douze arêtes du volume occupé, projetées en isométrie.
///
/// Le volume vient de [`Geometrie::bornes`], donc des LED elles-mêmes : la boîte
/// touche les ventilateurs extrêmes au lieu d'être une taille de boîtier
/// inventée.
fn aretes_du_volume(geometrie: &Geometrie) -> Vec<(Place, Place)> {
    let (bas, haut) = geometrie.bornes();
    let coin = |i: usize| Point {
        x: if i & 1 == 0 { bas.x } else { haut.x },
        y: if i & 2 == 0 { bas.y } else { haut.y },
        z: if i & 4 == 0 { bas.z } else { haut.z },
    };
    // Deux coins sont voisins si leurs indices ne diffèrent que d'un bit : c'est
    // la définition d'une arête du cube, et elle évite d'en écrire douze à la
    // main — donc d'en oublier une.
    let mut aretes = Vec::with_capacity(12);
    for a in 0..8usize {
        for bit in [1, 2, 4] {
            let b = a | bit;
            if b != a {
                aretes.push((en_isometrie(coin(a)), en_isometrie(coin(b))));
            }
        }
    }
    aretes
}

/// Le rayon dessiné d'un anneau, en millimètres du boîtier.
///
/// Il ne dépasse jamais le rayon **réel** — une maquette qui grossirait ses
/// ventilateurs mentirait sur ce qu'on regarde — et il rétrécit autant qu'il le
/// faut pour que deux anneaux voisins laissent entre eux deux diamètres de LED.
/// C'est la paire de centres la plus serrée qui commande, et elle se mesure
/// plutôt qu'elle ne se suppose.
fn rayon_dessine(geometrie: &Geometrie) -> f32 {
    let centres: Vec<Place> = Position::ALL
        .into_iter()
        .map(|position| de_face(geometrie.centre_ventilateur(position)))
        .collect();
    let mut serre = f32::INFINITY;
    for (rang, centre) in centres.iter().enumerate() {
        for autre in &centres[rang + 1..] {
            serre = serre.min(distance(*centre, *autre));
        }
    }

    // Le rayon réel se lit sur la géométrie : la distance d'un centre à l'une de
    // ses LED. Le recopier d'une constante le ferait diverger le jour où la
    // mesure se raffine.
    let reel = Position::ALL
        .into_iter()
        .find_map(|position| {
            let centre = geometrie.centre_ventilateur(position);
            let led = geometrie.led_ventilateur(position, 0)?;
            Some(distance3d(centre, led))
        })
        .unwrap_or(serre / ENCOMBREMENT);

    reel.min(serre / ENCOMBREMENT)
}

/// Le milieu du bloc de RAM, dans le repère de la vue de face.
fn milieu_de_la_ram(geometrie: &Geometrie) -> Place {
    let mut somme = Place { x: 0.0, y: 0.0 };
    let mut compte = 0.0;
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            if let Some(point) = geometrie.led_barrette(slot, led) {
                let plat = de_face(point);
                somme.x += plat.x;
                somme.y += plat.y;
                compte += 1.0;
            }
        }
    }
    Place {
        x: somme.x / compte,
        y: somme.y / compte,
    }
}

/// L'écart d'un rang au milieu d'une série, en pas.
fn ecart(rang: usize, combien: usize) -> f32 {
    rang as f32 - (combien - 1) as f32 / 2.0
}

/// La vue de face : la largeur est jetée, l'arrière va à gauche, le haut en
/// haut.
fn de_face(point: Point) -> Place {
    Place {
        x: -point.z,
        y: -point.y,
    }
}

/// La vue isométrique : les trois axes du boîtier gardés, inclinés de trente
/// degrés.
///
/// L'arrière part vers la gauche et le haut, la largeur vers la droite et le
/// haut, la hauteur vers le haut. C'est ce qui donne à chacun des trois plans
/// occupés une direction propre — et donc à chaque anneau une ellipse franche
/// plutôt qu'un trait.
fn en_isometrie(point: Point) -> Place {
    Place {
        x: COS30 * (point.x - point.z),
        y: -point.y - SIN30 * (point.x + point.z),
    }
}

/// Les demi-étendues, en largeur et en hauteur, de l'anneau **continu** dont ces
/// huit LED sont un échantillon.
///
/// ⚠️ Ce n'est **pas** l'écart maximal des huit points, et la nuance tient toute
/// la mise en page : ce que la fenêtre dessine est un anneau, pas huit points, et
/// l'endroit où l'anneau *commence* ne doit rien changer au cadre. Prendre le
/// maximum des huit ferait grandir le boîtier quand on tourne un ventilateur sur
/// place, ce qu'un test d'intention de #23 interdit.
///
/// Un anneau projeté s'écrit `C + A·cos θ + B·sin θ` — un cercle en vue de face,
/// une ellipse en isométrie, dans les deux cas l'image affine d'un cercle. Deux
/// LED séparées d'un quart de tour donnent `A` et `B` à une rotation près, et
/// `Ax² + Bx²` est invariant par cette rotation. Les LED étant à quarante-cinq
/// degrés l'une de l'autre, la LED 0 et la LED 2 font l'affaire.
fn etendue((centre, anneau): &(Place, [Place; LEDS])) -> (f32, f32) {
    let a = Place {
        x: anneau[0].x - centre.x,
        y: anneau[0].y - centre.y,
    };
    let b = Place {
        x: anneau[2].x - centre.x,
        y: anneau[2].y - centre.y,
    };
    (
        (a.x * a.x + b.x * b.x).sqrt(),
        (a.y * a.y + b.y * b.y).sqrt(),
    )
}

/// Ramène un plan encore en millimètres dans le carré `[0, 1]²`.
///
/// L'échelle est **uniforme** : un ventilateur rond doit rester rond. Une
/// échelle libre étirerait les anneaux en ellipses, et la maquette mentirait sur
/// ce qu'elle montre.
///
/// Le cadre englobe les **anneaux**, pas seulement les centres : un ventilateur
/// dont l'anneau déborde se dessine tronqué, et les LED qui manquent sont
/// justement celles qu'on ne pourra pas cliquer.
fn cadrer(plan: Plan, etendues: [(f32, f32); 10]) -> Plan {
    // Une LED porte un disque : le cadre doit contenir le disque, pas son
    // centre.
    let rayon = plan.rayon;
    let pastille = rayon * DIAMETRE_LED / 2.0;
    let mut bornes = Bornes::vide();
    for ((centre, _), (large, haut)) in plan.ventilateurs.iter().zip(etendues) {
        bornes.ajouter(centre.x - large - pastille, centre.y - haut - pastille);
        bornes.ajouter(centre.x + large + pastille, centre.y + haut + pastille);
    }
    for reglette in &plan.barrettes {
        for place in reglette {
            bornes.ajouter(place.x - pastille, place.y - pastille);
            bornes.ajouter(place.x + pastille, place.y + pastille);
        }
    }
    for (debut, fin) in &plan.aretes {
        bornes.ajouter(debut.x, debut.y);
        bornes.ajouter(fin.x, fin.y);
    }

    let utile = 1.0 - 2.0 * MARGE;
    let etendue = bornes.largeur().max(bornes.hauteur());
    let echelle = if etendue > 0.0 { utile / etendue } else { 1.0 };
    let decalage_x = (utile - bornes.largeur() * echelle) / 2.0;
    let decalage_y = (utile - bornes.hauteur() * echelle) / 2.0;
    let place = |brut: Place| Place {
        x: MARGE + decalage_x + (brut.x - bornes.x_min) * echelle,
        y: MARGE + decalage_y + (brut.y - bornes.y_min) * echelle,
    };

    let mut ventilateurs = plan.ventilateurs;
    for (centre, anneau) in &mut ventilateurs {
        *centre = place(*centre);
        for point in anneau.iter_mut() {
            *point = place(*point);
        }
    }
    let mut barrettes = plan.barrettes;
    for reglette in &mut barrettes {
        for point in reglette.iter_mut() {
            *point = place(*point);
        }
    }
    let aretes = plan
        .aretes
        .iter()
        .map(|(debut, fin)| (place(*debut), place(*fin)))
        .collect();

    Plan {
        ventilateurs,
        barrettes,
        aretes,
        rayon: rayon * echelle,
        vue: plan.vue,
    }
}

fn distance(a: Place, b: Place) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

fn distance3d(a: Point, b: Point) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

/// Le rectangle qu'occupe la maquette, avant sa mise à l'échelle.
struct Bornes {
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
}

impl Bornes {
    fn vide() -> Bornes {
        Bornes {
            x_min: f32::INFINITY,
            x_max: f32::NEG_INFINITY,
            y_min: f32::INFINITY,
            y_max: f32::NEG_INFINITY,
        }
    }

    fn ajouter(&mut self, x: f32, y: f32) {
        self.x_min = self.x_min.min(x);
        self.x_max = self.x_max.max(x);
        self.y_min = self.y_min.min(y);
        self.y_max = self.y_max.max(y);
    }

    fn largeur(&self) -> f32 {
        self.x_max - self.x_min
    }

    fn hauteur(&self) -> f32 {
        self.y_max - self.y_min
    }
}
