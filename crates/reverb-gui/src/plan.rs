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
use reverb_proto::composition::Ancre;
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position, Rgb, screen};

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

/// Une paroi du boîtier.
///
/// ⚠️ **`Plafond` existe et rien ne doit jamais le rendre.** C'est délibéré : la
/// demande de Nico est que la face du dessus **ne soit pas** remplie — une
/// plaque pleine masquerait les trois ventilateurs du plafond, qui sont
/// justement ce que l'isométrie sert à montrer. Une absence ne se vérifie que si
/// elle se nomme : une énumération sans `Plafond` rendrait le critère vrai par
/// construction, donc sans valeur le jour où quelqu'un ajoutera une plaque « pour
/// finir le volume ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paroi {
    Plancher,
    Plafond,
    Fond,
    /// Le flanc du plateau de carte mère.
    Flanc,
}

/// Une paroi à remplir, en coordonnées d'écran.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    pub paroi: Paroi,
    /// Les sommets dans l'ordre du contour. **La fermeture est implicite** : le
    /// dernier rejoint le premier et ne le répète pas.
    pub sommets: Vec<Place>,
}

/// Un organe interne du boîtier, suggéré en fond.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece {
    PlateauCarteMere,
    CarteGraphique,
    CacheAlimentation,
}

/// Un organe interne, en coordonnées d'écran.
///
/// Aucun ne porte de LED et aucun n'est cliquable : ce sont des repères de
/// lecture, pas des cibles.
#[derive(Debug, Clone, PartialEq)]
pub struct Organe {
    pub piece: Piece,
    /// Quatre sommets dans l'ordre du contour. Un rectangle du boîtier projeté
    /// est un **parallélogramme** — c'est ce qui distingue un organe posé dans le
    /// repère du boîtier d'un rectangle dessiné sur l'écran.
    pub sommets: Vec<Place>,
}

/// Ce qu'une forme d'habillage figure (issue #64).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ornement {
    CadreVentilateur(Position),
    CorpsBarrette(usize),
    DisqueKraken,
}

/// Une forme d'habillage : un contour, éventuellement percé.
///
/// ⚠️ **Le creux est ce qui permet d'entourer sans masquer.** Un cadre plein
/// recouvrirait les huit LED qu'il entoure, et l'issue interdit qu'une forme
/// masque une LED : la fenêtre est un instrument, chaque LED doit rester
/// cliquable. Le boîtier photographié montre la même solution — le cadre d'un
/// F140 est percé, et l'anneau se voit par le trou.
///
/// `creux` est **vide** quand la forme est pleine, ce qui est le cas ordinaire
/// d'un corps de barrette ou d'une dalle.
#[derive(Debug, Clone, PartialEq)]
pub struct Forme {
    pub ornement: Ornement,
    /// Les sommets dans l'ordre du contour. **La fermeture est implicite**, même
    /// convention que [`Face`] et [`Organe`].
    pub contour: Vec<Place>,
    pub creux: Vec<Place>,
}

/// Une couche du halo d'une LED.
///
/// `rayon` est en multiples du rayon de la pastille, `opacite` de zéro exclu à
/// un exclu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoucheDeHalo {
    pub couleur: Rgb,
    pub rayon: f32,
    pub opacite: f32,
}

/// Les couches du halo, du plus serré au plus large.
///
/// Trois et non deux : avec deux, le débordement garde un bord franc que le
/// boîtier n'a pas. Les rayons sont en multiples du rayon de la pastille — la
/// plus large déborde de cinq fois, ce qui est l'ordre de grandeur qu'on voit
/// sur les photos, où la couleur d'un ventilateur atteint les parois autour.
const COUCHES_DE_HALO: [(f32, f32); 3] = [(2.1, 0.30), (3.4, 0.15), (5.2, 0.07)];

/// Ce qu'une LED de cette couleur diffuse autour d'elle.
///
/// ⚠️ **Une LED éteinte ne diffuse rien**, et « éteinte » veut dire noire —
/// trois composantes nulles, sans seuil. Un seuil de luminosité serait une
/// invention, et il ferait disparaître le halo d'une braise en fin de cycle,
/// qui est justement un endroit où il se voit. Le mode de défaillance inverse
/// est plus coûteux encore parce qu'il rassure : un boîtier qu'on vient
/// d'éteindre, et une maquette restée constellée d'auréoles.
///
/// ⚠️ **Le halo porte exactement la couleur de sa LED.** Rien ne l'atténue que
/// son opacité : un halo blanchi « pour éclairer un peu » donnerait un boîtier
/// dont toutes les LED baignent dans la même lueur, c'est-à-dire qui ne dit
/// plus ce qu'elles affichent.
///
/// C'est **dessiné**, jamais une ombre portée : le rendu logiciel de Slint —
/// celui de `examples/apercu.rs` — ignore `drop-shadow-blur`, mesuré (#64).
pub fn halo(couleur: Rgb) -> Vec<CoucheDeHalo> {
    if couleur == Rgb::BLACK {
        return Vec::new();
    }
    COUCHES_DE_HALO
        .iter()
        .map(|&(rayon, opacite)| CoucheDeHalo {
            couleur,
            rayon,
            opacite,
        })
        .collect()
}

/// Où dessiner chaque LED du boîtier.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    ventilateurs: [(Place, [Place; LEDS]); 10],
    barrettes: [[Place; LEDS_PER_STICK]; SLOT_COUNT],
    /// Les arêtes du volume, à dessiner derrière les LED.
    aretes: Vec<(Place, Place)>,
    /// Le contour du châssis, à dessiner derrière tout le reste.
    silhouette: Vec<Place>,
    /// Les parois à remplir. Vide en vue de face.
    faces: Vec<Face>,
    /// Les organes internes, dans les deux vues.
    organes: Vec<Organe>,
    /// L'habillage : dix cadres, quatre corps de barrette, une dalle.
    habillage: Vec<Forme>,
    /// Les deux demi-axes de chaque anneau, dans l'ordre de `Position::ALL`.
    demi_axes: [(f32, f32); 10],
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
                silhouette: Vec::new(),
                habillage: Vec::new(),
                // Pas de paroi remplie de face : la vue écrase la largeur du
                // boîtier, donc plancher, fond et flanc s'y superposent en un
                // seul rectangle qui ne dit rien de plus que la silhouette.
                faces: Vec::new(),
                organes: organes_projetes(geometrie, de_face),
                demi_axes: [(rayon, rayon); 10],
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
                silhouette: Vec::new(),
                habillage: Vec::new(),
                faces: faces_du_volume(geometrie),
                organes: organes_projetes(geometrie, en_isometrie),
                demi_axes: etendues,
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

    /// Le contour du châssis, dans l'ordre du contour.
    ///
    /// **Sa forme dépend de la vue**, et les deux se déduisent du contenu :
    ///
    /// | vue | contour | pourquoi |
    /// |---|---|---|
    /// | [`Vue::Face`] | un **rectangle** à axes alignés | une boîte vue de face en projection orthographique *est* un rectangle |
    /// | [`Vue::Isometrique`] | l'enveloppe convexe du nuage | de trois-quarts, la même boîte se projette en hexagone |
    ///
    /// ⚠️ **Ni l'un ni l'autre n'est écrit à la main.** C'est la contrainte qui
    /// commande : `Geometrie` ne porte que dix orientations — les centres et le
    /// rayon sont les mêmes pour toutes les géométries d'une même machine —, si
    /// bien qu'un contour calculé sur les anneaux serait insensible à toute
    /// géométrie possible, c'est-à-dire indiscernable d'un polygone posé en
    /// dur. Le rectangle de face se déduit des **bornes du contenu dessiné**,
    /// l'enveloppe isométrique du **nuage des cent vingt-quatre LED**.
    ///
    /// Contrepartie assumée : remonter un ventilateur fait respirer le contour
    /// de quelques millièmes de cadre. Les douze arêtes de #28 le font déjà,
    /// elles viennent de `Geometrie::bornes()`, donc des LED elles-mêmes.
    ///
    /// ⚠️ **En vue de face, il contient l'habillage et pas seulement les LED**
    /// (issue #125). Voir [`rectangle_du_chassis`].
    pub fn silhouette(&self) -> &[Place] {
        &self.silhouette
    }

    /// Les parois à remplir, **jamais celle du dessus**.
    ///
    /// Vide en vue de face. Voir [`Paroi`].
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    /// Les organes internes, dans les deux vues.
    pub fn organes(&self) -> &[Organe] {
        &self.organes
    }

    /// L'habillage : dix cadres de ventilateur, quatre corps de barrette, une
    /// dalle. Quinze formes, dans les deux vues.
    pub fn habillage(&self) -> &[Forme] {
        &self.habillage
    }

    /// Les deux demi-axes de l'anneau d'un ventilateur : horizontal, vertical.
    ///
    /// ⚠️ Ceux de l'anneau **continu**, et non le maximum de ses huit LED. Un
    /// anneau est un cercle dont les huit LED sont un échantillon à quarante-cinq
    /// degrés : le maximum échantillonné vaut entre `cos(22,5°)` et une fois le
    /// demi-axe réel. Un cerclage tracé sur l'échantillon passerait au travers de
    /// ses propres LED.
    ///
    /// En vue de face, les deux valent [`Plan::rayon_anneau`] — c'est un cercle.
    pub fn demi_axes_anneau(&self, position: Position) -> (f32, f32) {
        self.demi_axes[position.index()]
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

// ---------------------------------------------------------------------------
// Du panneau au carré (issue #121)
// ---------------------------------------------------------------------------

/// Le rectangle du panneau, ramené au repère du carré de la maquette.
///
/// `panneau_largeur` et `panneau_hauteur` sont en pixels logiques ; les quatre
/// coordonnées sont des **fractions du panneau**, et le résultat des
/// **fractions du carré** — celles que [`Plan::dans`] et [`Plan::sous`]
/// attendent.
///
/// # Pourquoi cette conversion existe
///
/// La maquette vit dans un **carré** de côté `min(largeur, hauteur)`, centré
/// dans le panneau : ses tracés sont des `Path` en `viewbox` carré, que Slint
/// met à l'échelle uniformément. Le **geste**, lui, porte sur tout le panneau
/// (#121) — les bords du carré tombent près des extrémités des ventilateurs, et
/// une marge qui ne se clique pas se lit comme une maquette qui ne répond pas.
/// Les deux repères diffèrent donc d'une **moitié de marge** sur l'axe le plus
/// long, et c'est ici, en Rust, que la différence se calcule : dans le `.slint`
/// elle ne se testerait pas.
///
/// ```text
/// côté  = min(largeur, hauteur)
/// marge = (largeur − côté) / 2          en pixels, à gauche comme à droite
/// sx    = (x · largeur − marge) / côté
/// ```
///
/// et le symétrique sur l'ordonnée, avec `(hauteur − côté) / 2`.
///
/// # Rien n'est écrêté
///
/// Les coordonnées rendues **sortent de `0..1`** dès que le geste touche une
/// marge, et c'est voulu. Écrêter replierait un rectangle parti de la marge sur
/// le bord du carré, où il attraperait la première rangée de LED : la zone morte
/// d'hier deviendrait une sélection au hasard. [`Plan::dans`] et [`Plan::sous`]
/// acceptent déjà le débordement — l'un ne retient rien, l'autre rend `None`.
///
/// La conversion est **point par point**, jamais rectangle par rectangle : les
/// deux coins ne sont pas réordonnés, un geste tiré de droite à gauche le reste.
/// C'est ce qui la laisse composer avec un clic simple, où les deux coins sont
/// confondus.
///
/// # Un panneau sans surface
///
/// Avant la première mise en page, le panneau n'a ni largeur ni hauteur : il n'y
/// a pas de carré où viser, et la conversion diviserait par zéro. Elle rend
/// alors un point franchement **hors** du carré, plutôt que le tracé inchangé
/// « à défaut de mieux » — ce qui ferait retenir les cent vingt-quatre LED d'un
/// geste sur un panneau qui n'existe pas encore.
pub fn trace_dans_la_maquette(
    panneau_largeur: f32,
    panneau_hauteur: f32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
) -> (f32, f32, f32, f32) {
    let cote = panneau_largeur.min(panneau_hauteur);
    if !cote.is_finite() || cote <= 0.0 {
        return (HORS_DU_CARRE, HORS_DU_CARRE, HORS_DU_CARRE, HORS_DU_CARRE);
    }
    let marge_x = (panneau_largeur - cote) / 2.0;
    let marge_y = (panneau_hauteur - cote) / 2.0;
    let sur_x = |x: f32| (x * panneau_largeur - marge_x) / cote;
    let sur_y = |y: f32| (y * panneau_hauteur - marge_y) / cote;
    (sur_x(x0), sur_y(y0), sur_x(x1), sur_y(y1))
}

/// Où tombe un geste quand le panneau n'a pas de surface.
///
/// Un point, et franchement hors du cadre : [`Plan::dans`] n'y retient rien et
/// [`Plan::sous`] y rend `None`. Le tracé est ainsi sans effet, sans qu'aucune
/// coordonnée cesse d'être un nombre.
const HORS_DU_CARRE: f32 = -1.0;

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

    // La silhouette se calcule **avant** le cadrage, sur les places brutes : le
    // cadrage est une application affine, et l'enveloppe convexe d'un nuage
    // transformé est la transformée de son enveloppe. La calculer ici plutôt que
    // dans chaque constructeur la donne aux deux vues d'un seul coup.
    let mut nuage: Vec<Place> = Vec::with_capacity(124);
    for (_, anneau) in &plan.ventilateurs {
        nuage.extend_from_slice(anneau);
    }
    for reglette in &plan.barrettes {
        nuage.extend_from_slice(reglette);
    }

    // L'habillage aussi se calcule avant, et pour la même raison : il dérive des
    // places brutes et des demi-axes bruts, et il doit entrer dans les bornes.
    let habillage = habillage_brut(&plan.ventilateurs, &plan.barrettes, etendues, pastille);

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
    // ⚠️ L'habillage entre dans le cadre comme le reste. Sans cela, la vue de
    // face déborderait : ses anneaux y sont dessinés **plus petits que nature**
    // (voir `rayon_dessine`), si bien que le nuage de LED y est plus étroit que
    // le volume dont les organes internes tirent leurs proportions.
    for face in &plan.faces {
        for sommet in &face.sommets {
            bornes.ajouter(sommet.x, sommet.y);
        }
    }
    for organe in &plan.organes {
        for sommet in &organe.sommets {
            bornes.ajouter(sommet.x, sommet.y);
        }
    }
    for forme in &habillage {
        for sommet in forme.contour.iter().chain(&forme.creux) {
            bornes.ajouter(sommet.x, sommet.y);
        }
    }

    // La silhouette vient **après** les bornes du contenu, et non plus avant :
    // en vue de face elle s'en déduit (issue #125).
    let silhouette = match plan.vue {
        Vue::Face => rectangle_du_chassis(&bornes, pastille),
        Vue::Isometrique => silhouette_du_nuage(&nuage, pastille),
    };
    // Puis elle rentre dans le cadre à son tour — elle déborde du contenu de la
    // marge qu'on vient de lui donner.
    for sommet in &silhouette {
        bornes.ajouter(sommet.x, sommet.y);
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
    let silhouette = silhouette.into_iter().map(place).collect();
    let faces = plan
        .faces
        .into_iter()
        .map(|face| Face {
            paroi: face.paroi,
            sommets: face.sommets.into_iter().map(place).collect(),
        })
        .collect();
    let organes = plan
        .organes
        .into_iter()
        .map(|organe| Organe {
            piece: organe.piece,
            sommets: organe.sommets.into_iter().map(place).collect(),
        })
        .collect();
    let habillage = habillage
        .into_iter()
        .map(|forme| Forme {
            ornement: forme.ornement,
            contour: forme.contour.into_iter().map(place).collect(),
            creux: forme.creux.into_iter().map(place).collect(),
        })
        .collect();
    let mut demi_axes = plan.demi_axes;
    for (large, haut) in &mut demi_axes {
        *large *= echelle;
        *haut *= echelle;
    }

    Plan {
        ventilateurs,
        barrettes,
        aretes,
        silhouette,
        faces,
        organes,
        habillage,
        demi_axes,
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

// ---------------------------------------------------------------------------
// L'habillage (issue #52)
// ---------------------------------------------------------------------------

/// Les huit coins du volume occupé, indexés par bits — `1` pour `x`, `2` pour
/// `y`, `4` pour `z`.
///
/// La même numérotation que [`aretes_du_volume`], et c'est ce qui garantit
/// qu'une face se pose **sur** les arêtes plutôt qu'à côté.
fn coin_du_volume(geometrie: &Geometrie, i: usize) -> Point {
    let (bas, haut) = geometrie.bornes();
    Point {
        x: if i & 1 == 0 { bas.x } else { haut.x },
        y: if i & 2 == 0 { bas.y } else { haut.y },
        z: if i & 4 == 0 { bas.z } else { haut.z },
    }
}

/// Les trois parois remplies de la vue isométrique.
///
/// ⚠️ **Le plafond n'y est pas, et ne doit jamais y être.** Une plaque pleine au
/// dessus masquerait les trois ventilateurs du plafond, qui sont ce que
/// l'isométrie sert à montrer (demande de Nico, issue #52).
fn faces_du_volume(geometrie: &Geometrie) -> Vec<Face> {
    // Les coins sont donnés dans l'ordre du contour de chaque quadrilatère : un
    // ordre quelconque tracerait un nœud papillon, qui se dessine sans erreur et
    // ne remplit rien.
    let paroi = |paroi, coins: [usize; 4]| Face {
        paroi,
        sommets: coins
            .into_iter()
            .map(|i| en_isometrie(coin_du_volume(geometrie, i)))
            .collect(),
    };
    vec![
        paroi(Paroi::Plancher, [0, 1, 5, 4]),
        paroi(Paroi::Fond, [4, 5, 7, 6]),
        paroi(Paroi::Flanc, [1, 5, 7, 3]),
    ]
}

/// Les trois organes internes, posés dans le repère du boîtier puis projetés.
///
/// Chacun est un rectangle du plan de la carte mère — donc à `x` constant, du
/// côté du plateau — exprimé en **fractions du volume occupé**. C'est ce qui les
/// fait suivre la géométrie au lieu de rester plantés là où on les a dessinés.
///
/// Les proportions viennent des photos du boîtier du 2026-08-02 : le plateau
/// occupe la moitié arrière, la carte graphique une bande horizontale sous la
/// RAM, le cache d'alimentation le bas.
fn organes_projetes(geometrie: &Geometrie, projeter: fn(Point) -> Place) -> Vec<Organe> {
    let (bas, haut) = geometrie.bornes();
    let entre = |min: f32, max: f32, part: f32| min + (max - min) * part;
    // Le plan de la carte mère, très légèrement en deçà du flanc : posé
    // exactement dessus, il disputerait ses pixels à la paroi remplie.
    let x = entre(bas.x, haut.x, 0.94);

    let dalle = |piece, (y0, y1): (f32, f32), (z0, z1): (f32, f32)| {
        let point = |y: f32, z: f32| {
            projeter(Point {
                x,
                y: entre(bas.y, haut.y, y),
                z: entre(bas.z, haut.z, z),
            })
        };
        Organe {
            piece,
            // Dans l'ordre du contour, fermeture implicite.
            sommets: vec![point(y0, z0), point(y0, z1), point(y1, z1), point(y1, z0)],
        }
    };

    vec![
        dalle(Piece::PlateauCarteMere, (0.10, 0.88), (0.42, 0.98)),
        dalle(Piece::CarteGraphique, (0.28, 0.46), (0.46, 0.92)),
        dalle(Piece::CacheAlimentation, (0.02, 0.16), (0.20, 0.96)),
    ]
}

/// Le contour convexe d'un nuage de places, dans l'ordre du contour.
///
/// Parcours de Graham par angle polaire autour du point le plus bas : le nuage
/// des LED n'a pas de creux qu'on veuille lire, et un contour convexe se
/// remplit sans se croiser.
fn enveloppe(nuage: &[Place]) -> Vec<Place> {
    if nuage.len() < 3 {
        return nuage.to_vec();
    }
    let mut points = nuage.to_vec();
    // Le pivot : le plus bas, puis le plus à gauche. Il est sur l'enveloppe.
    let pivot = points
        .iter()
        .copied()
        .reduce(|a, b| if (b.y, b.x) < (a.y, a.x) { b } else { a })
        .unwrap_or(ZERO);
    points.sort_by(|a, b| {
        let angle = |p: &Place| (p.y - pivot.y).atan2(p.x - pivot.x);
        angle(a)
            .partial_cmp(&angle(b))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let d = |p: &Place| (p.x - pivot.x).hypot(p.y - pivot.y);
                d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let tourne =
        |o: Place, a: Place, b: Place| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut pile: Vec<Place> = Vec::with_capacity(points.len());
    for point in points {
        while pile.len() >= 2 {
            let (o, a) = (pile[pile.len() - 2], pile[pile.len() - 1]);
            // ⚠️ `<=` : on écarte aussi les points **alignés**. Trois sommets
            // colinéaires donneraient un contour dont une arête est plate,
            // c'est-à-dire un sommet qui ne dit rien.
            if tourne(o, a, point) <= 0.0 {
                pile.pop();
            } else {
                break;
            }
        }
        pile.push(point);
    }
    pile
}

// ---------------------------------------------------------------------------
// L'habillage : ce qui fait qu'un ventilateur ressemble à un ventilateur (#64)
// ---------------------------------------------------------------------------

/// Le contour extérieur d'un cadre, en demi-axes d'anneau.
///
/// ⚠️ **Ces deux constantes ne sont pas choisies, elles sont mesurées**, et la
/// fenêtre entre elles est étroite. Sur la géométrie de SHYNAEL, en isométrie :
/// les huit LED d'un ventilateur montent à **1,172** fois leur ellipse — l'anneau
/// y est vu penché — et le centre du voisin le plus proche est à **1,273**. Le
/// creux doit passer au-dessus de la première valeur pour ne masquer aucune LED,
/// le contour rester en deçà de la seconde pour ne pas avaler son voisin.
///
/// ⚠️ **Le cadre est une ellipse, et non le carré d'un F140**, parce qu'un carré
/// est géométriquement impossible ici : en isométrie, les LED du ventilateur
/// atteignent **0,998** en norme rectangle quand le centre du voisin n'est qu'à
/// **0,900**. Aucun rectangle aligné aux axes ne peut donc contenir ses propres
/// LED sans contenir le centre d'un autre ventilateur. Mesuré avant d'être écrit.
const CADRE_EXTERIEUR: f32 = 1.25;

/// Le creux d'un cadre, en demi-axes d'anneau. Voir [`CADRE_EXTERIEUR`].
const CADRE_INTERIEUR: f32 = 1.20;

/// Sommets d'une ellipse d'habillage.
///
/// Assez pour que l'arête reste au-dessus du rayon nominal moins un demi-pour-
/// cent : les marges du cadre se comptent en deux centièmes, un polygone
/// grossier les mangerait.
const SOMMETS_ELLIPSE: usize = 48;

/// Ce qu'une forme laisse au minimum entre elle et la LED la plus proche, en
/// pastilles.
///
/// Le critère est « ne masque aucune LED » ; s'en approcher à moins d'une
/// pastille masquerait le disque sans masquer son centre — vrai pour le test,
/// faux pour l'œil.
const DEGAGEMENT: f32 = 1.2;

/// Un polygone elliptique centré, dans l'ordre du contour.
fn ellipse(centre: Place, rx: f32, ry: f32) -> Vec<Place> {
    (0..SOMMETS_ELLIPSE)
        .map(|rang| {
            let angle = rang as f32 / SOMMETS_ELLIPSE as f32 * std::f32::consts::TAU;
            Place {
                x: centre.x + rx * angle.cos(),
                y: centre.y + ry * angle.sin(),
            }
        })
        .collect()
}

/// L'habillage d'un plan **avant cadrage**, dans le repère des places brutes.
///
/// Calculé ici et non après, pour la même raison que la silhouette : le cadrage
/// est une application affine, et il vaut mieux que les formes entrent dans ses
/// bornes que de déborder du carré unité une fois tout réduit.
fn habillage_brut(
    ventilateurs: &[(Place, [Place; LEDS]); 10],
    barrettes: &[[Place; LEDS_PER_STICK]; SLOT_COUNT],
    etendues: [(f32, f32); 10],
    pastille: f32,
) -> Vec<Forme> {
    let mut leds: Vec<Place> = Vec::with_capacity(124);
    for (_, anneau) in ventilateurs {
        leds.extend_from_slice(anneau);
    }
    for reglette in barrettes {
        leds.extend_from_slice(reglette);
    }

    let mut formes = Vec::with_capacity(15);

    // Les dix cadres : un anneau épais, percé de son propre trou.
    for (rang, position) in Position::ALL.into_iter().enumerate() {
        let (centre, _) = ventilateurs[rang];
        let (large, haut) = etendues[rang];
        formes.push(Forme {
            ornement: Ornement::CadreVentilateur(position),
            contour: ellipse(centre, large * CADRE_EXTERIEUR, haut * CADRE_EXTERIEUR),
            creux: ellipse(centre, large * CADRE_INTERIEUR, haut * CADRE_INTERIEUR),
        });
    }

    // Les quatre corps de barrette : une bande le long de la colonne de LED,
    // posée **à côté** d'elle et jamais dessus. Sur la photo, la bande lumineuse
    // court sur l'arête du corps ; ici elle la longe à une pastille, parce que
    // les onze LED ne sont pas exactement colinéaires en isométrie et qu'une
    // arête qui les frôlerait finirait par en avaler une.
    for (slot, reglette) in barrettes.iter().enumerate() {
        formes.push(Forme {
            ornement: Ornement::CorpsBarrette(slot),
            contour: corps_de_barrette(reglette, &leds, pastille),
            creux: Vec::new(),
        });
    }

    // La dalle du Kraken, **sous le bloc de RAM** : c'est là qu'elle est sur les
    // photos du boîtier, la pompe étant posée sur le processeur et les quatre
    // barrettes juste au-dessus de lui. Son rayon n'est pas choisi : c'est ce que
    // la place permet, et la place est mince — les trois ventilateurs du plafond
    // se projettent par-dessus en isométrie.
    let centre = sous_la_ram(barrettes);
    let libre = leds
        .iter()
        .map(|led| distance(centre, *led))
        .fold(f32::INFINITY, f32::min);
    let rayon = (libre - DEGAGEMENT * pastille).max(pastille);
    formes.push(Forme {
        ornement: Ornement::DisqueKraken,
        contour: ellipse(centre, rayon, rayon),
        creux: Vec::new(),
    });

    formes
}

/// Juste sous le bloc de RAM, où se trouve la pompe.
///
/// **Relevé sur les photos du boîtier**, pas déduit : les quatre barrettes sont
/// au-dessus du processeur, et la dalle du Kraken se lit juste en dessous
/// d'elles. Accroché à la RAM plutôt qu'au plateau de carte mère, parce que la
/// RAM a des LED — donc une place mesurée — là où le plateau n'est qu'un
/// rectangle décoratif dont le centre ne veut rien dire.
fn sous_la_ram(barrettes: &[[Place; LEDS_PER_STICK]; SLOT_COUNT]) -> Place {
    let mut bornes = Bornes::vide();
    for reglette in barrettes {
        for place in reglette {
            bornes.ajouter(place.x, place.y);
        }
    }
    Place {
        x: (bornes.x_min + bornes.x_max) / 2.0,
        // Une demi-hauteur de bloc sous son bord bas : assez pour sortir de la
        // RAM, assez peu pour rester sur la carte mère.
        y: bornes.y_max + bornes.hauteur() / 2.0,
    }
}

/// Une bande le long d'une barrette, du côté où elle ne masque rien.
///
/// L'épaisseur n'est pas une constante : c'est **ce que la place permet**. Les
/// quatre barrettes sont serrées, et une épaisseur choisie une fois marcherait
/// dans une vue pour recouvrir une LED dans l'autre.
fn corps_de_barrette(
    reglette: &[Place; LEDS_PER_STICK],
    leds: &[Place],
    pastille: f32,
) -> Vec<Place> {
    let debut = reglette[0];
    let fin = reglette[LEDS_PER_STICK - 1];
    let (dx, dy) = (fin.x - debut.x, fin.y - debut.y);
    let norme = dx.hypot(dy).max(f32::MIN_POSITIVE);
    let (ux, uy) = (dx / norme, dy / norme);
    // La normale, tournée d'un quart de tour. Le signe est celui qui éloigne du
    // milieu du nuage : une bande posée vers l'intérieur du bloc de RAM tomberait
    // sur la barrette voisine.
    let (nx, ny) = (-uy, ux);

    // Ce qu'on peut prendre de chaque côté avant de toucher une LED. La bande
    // longe la colonne : un point ne la gêne que s'il se projette dans sa
    // longueur, débords compris — la bande dépasse d'un retrait à chaque bout.
    let marge = DEGAGEMENT * pastille;
    let place_libre = |signe: f32| {
        leds.iter()
            .filter_map(|led| {
                let (ex, ey) = (led.x - debut.x, led.y - debut.y);
                let le_long = ex * ux + ey * uy;
                if !(-marge..=norme + marge).contains(&le_long) {
                    return None;
                }
                let en_travers = (ex * nx + ey * ny) * signe;
                (en_travers > 0.0).then_some(en_travers)
            })
            .fold(f32::INFINITY, f32::min)
    };

    // Le côté le plus dégagé gagne.
    let (signe, libre) = [1.0f32, -1.0]
        .into_iter()
        .map(|signe| (signe, place_libre(signe)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .expect("deux côtés");

    // ⚠️ **Le dégagement cède avant l'épaisseur, et jamais l'inverse.** Un
    // plancher d'épaisseur forcerait la bande à déborder là où la place manque —
    // et la place manque : en isométrie les quatre barrettes se projettent
    // presque l'une sur l'autre. Le retrait suit donc la place quand elle se
    // resserre, ce qui garde la bande **toujours** entre les deux colonnes,
    // fût-elle mince.
    let retrait = marge.min(libre / 3.0);
    let epaisseur = (libre - 2.0 * retrait).min(pastille * 3.0);

    let bord = |le_long: f32, en_travers: f32| Place {
        x: debut.x + ux * le_long + nx * signe * en_travers,
        y: debut.y + uy * le_long + ny * signe * en_travers,
    };
    let proche = retrait;
    let loin = retrait + epaisseur;
    vec![
        bord(-retrait, proche),
        bord(norme + retrait, proche),
        bord(norme + retrait, loin),
        bord(-retrait, loin),
    ]
}

/// Le contour du châssis **en vue de face** : le rectangle du contenu, écarté
/// d'une marge (issue #125).
///
/// Un boîtier est une boîte, et une boîte vue de face en projection
/// orthographique **est** un rectangle. L'enveloppe convexe employée jusqu'ici
/// donnait un polygone à quatorze tranches obliques, qui n'était le contour de
/// rien — c'était l'enveloppe de l'*éclairage*, pas celle du châssis.
///
/// ⚠️ **Elle part des bornes du contenu dessiné, jamais des seules LED.** Les
/// cadres de ventilateur sont tracés entre 1,20 et 1,25 fois leur demi-axe
/// (`CADRE_EXTERIEUR`) là où le nuage s'arrête aux centres de LED : un
/// rectangle déduit du seul nuage laisserait les ventilateurs dépasser du
/// trait censé les contenir, ce qui était le second défaut de #125. Mesuré
/// avant correction : le cadre de `BasGauche` sortait de 1,6·10⁻⁴ de cadre.
///
/// La justification de [`Plan::silhouette`] tient toujours : ces bornes
/// dérivent des places dessinées, donc le rectangle **bouge avec la
/// géométrie**. Ce n'est pas un polygone écrit à la main.
fn rectangle_du_chassis(bornes: &Bornes, marge: f32) -> Vec<Place> {
    let (x0, y0) = (bornes.x_min - marge, bornes.y_min - marge);
    let (x1, y1) = (bornes.x_max + marge, bornes.y_max + marge);
    // Dans l'ordre du contour, fermeture implicite — comme partout ailleurs.
    vec![
        Place { x: x0, y: y0 },
        Place { x: x1, y: y0 },
        Place { x: x1, y: y1 },
        Place { x: x0, y: y1 },
    ]
}

/// Le contour du châssis : l'enveloppe des LED, écartée d'une pastille.
///
/// L'écart se prend depuis le centre du nuage, ce qui suffit ici : l'enveloppe
/// contient déjà toutes les LED, et la dilatation ne sert qu'à ne pas dessiner
/// le trait **sur** celles du bord.
fn silhouette_du_nuage(nuage: &[Place], marge: f32) -> Vec<Place> {
    let contour = enveloppe(nuage);
    if contour.is_empty() {
        return contour;
    }
    let compte = contour.len() as f32;
    let centre = Place {
        x: contour.iter().map(|p| p.x).sum::<f32>() / compte,
        y: contour.iter().map(|p| p.y).sum::<f32>() / compte,
    };
    contour
        .into_iter()
        .map(|sommet| {
            let (dx, dy) = (sommet.x - centre.x, sommet.y - centre.y);
            let norme = dx.hypot(dy);
            if norme <= f32::EPSILON {
                return sommet;
            }
            Place {
                x: sommet.x + dx / norme * marge,
                y: sommet.y + dy / norme * marge,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// De la géométrie aux commandes de tracé
// ---------------------------------------------------------------------------

/// Un polygone fermé, en commandes SVG dans le carré unité.
fn contour(sommets: &[Place]) -> String {
    let mut sortie = String::new();
    for (rang, sommet) in sommets.iter().enumerate() {
        let verbe = if rang == 0 { 'M' } else { 'L' };
        sortie.push_str(&format!("{verbe} {:.4} {:.4} ", sommet.x, sommet.y));
    }
    if !sommets.is_empty() {
        sortie.push_str("Z ");
    }
    sortie
}

impl Plan {
    /// Le contour du châssis, en commandes SVG dans le carré unité.
    ///
    /// Ces quatre méthodes existent pour que le `.slint` ne porte **aucune
    /// coordonnée** : un habillage dessiné là-bas marcherait le jour où il est
    /// écrit et deviendrait faux sans un mot le jour où un ventilateur change de
    /// place (issue #52).
    pub fn commandes_silhouette(&self) -> String {
        contour(&self.silhouette)
    }

    /// Les parois remplies, en commandes SVG. Vide en vue de face.
    pub fn commandes_faces(&self) -> String {
        self.faces
            .iter()
            .map(|face| contour(&face.sommets))
            .collect()
    }

    /// Les organes internes, en commandes SVG.
    pub fn commandes_organes(&self) -> String {
        self.organes
            .iter()
            .map(|organe| contour(&organe.sommets))
            .collect()
    }

    /// L'habillage, en commandes SVG.
    ///
    /// Chaque forme sort en un ou deux sous-chemins : son contour, puis son creux
    /// s'il en a un. Deux sous-chemins imbriqués dans un même `Path` se rendent en
    /// anneau — c'est la règle du pair-impair, et c'est ce qui perce le cadre sans
    /// qu'aucune LED soit masquée.
    pub fn commandes_habillage(&self) -> String {
        self.habillage
            .iter()
            .map(|forme| format!("{}{}", contour(&forme.contour), contour(&forme.creux)))
            .collect()
    }

    /// Les dix anneaux et leurs moyeux, en commandes SVG.
    ///
    /// Deux arcs par ellipse : un demi-tour chacun. SVG n'a pas de primitive
    /// « ellipse » dans un chemin, et deux arcs de cent quatre-vingts degrés en
    /// tiennent lieu sans approximation.
    pub fn commandes_anneaux(&self) -> String {
        let mut sortie = String::new();
        for position in Position::ALL {
            let centre = self.centre_ventilateur(position);
            let (rx, ry) = self.demi_axes_anneau(position);
            for facteur in [1.0, 0.22] {
                let (rx, ry) = (rx * facteur, ry * facteur);
                sortie.push_str(&format!(
                    "M {:.4} {:.4} A {rx:.4} {ry:.4} 0 1 0 {:.4} {:.4} \
                     A {rx:.4} {ry:.4} 0 1 0 {:.4} {:.4} ",
                    centre.x - rx,
                    centre.y,
                    centre.x + rx,
                    centre.y,
                    centre.x - rx,
                    centre.y,
                ));
            }
        }
        sortie
    }
}

// ---------------------------------------------------------------------------
// Les cinq ancres de la dalle (issue #76, protocole de #80)
// ---------------------------------------------------------------------------

/// Une ancre de composition, ramenée au carré unité.
///
/// Les mêmes quatre nombres que [`reverb_proto::composition::Boite`], divisés
/// par le côté du tampon — le `.slint` place ses rectangles en fraction de sa
/// largeur, comme il place les LED.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaceAncre {
    pub ancre: Ancre,
    pub x: f32,
    pub y: f32,
    pub largeur: f32,
    pub hauteur: f32,
}

/// Où les cinq ancres se posent sur la dalle, dans le carré unité.
///
/// ⚠️ **Ces places viennent de `Ancre::boite()` — les boîtes du démon**, celles
/// qu'il assombrit et sur lesquelles il écrit. Ce n'est donc pas une seconde
/// implémentation qui divergerait à la première correction : c'est la même
/// donnée, lue de l'autre côté du socket. La règle de #52 pour la maquette,
/// appliquée à la dalle.
pub fn places_des_ancres() -> Vec<PlaceAncre> {
    let cote = f32::from(screen::WIDTH);
    Ancre::TOUTES
        .into_iter()
        .map(|ancre| {
            let boite = ancre.boite();
            PlaceAncre {
                ancre,
                x: f32::from(boite.x) / cote,
                y: f32::from(boite.y) / cote,
                largeur: f32::from(boite.largeur) / cote,
                hauteur: f32::from(boite.hauteur) / cote,
            }
        })
        .collect()
}

/// Le rayon du disque visible, en fraction du côté du tampon.
///
/// ⚠️ **La dalle est ronde, le tampon est carré** (`SPEC-KRAKEN-LCD` §2.1.1,
/// observé le 2026-08-08) : 21 % de ce qu'on transmet ne s'affiche nulle part,
/// et rien ne le signale — le contrôleur accepte l'image entière. Composer sur
/// un carré, ce serait juger la mise en page sur une surface qui n'existe pas.
///
/// Le rayon exact reste 🔶 — la mire de #77 le tranchera. Le prendre de
/// `screen` plutôt que de l'écrire dans le `.slint` fait qu'il n'y aura ce
/// jour-là qu'une constante à corriger.
pub fn rayon_du_disque() -> f32 {
    f32::from(screen::VISIBLE_DISC_RADIUS) / f32::from(screen::WIDTH)
}

// ---------------------------------------------------------------------------
// Le nom d'une zone née d'une sélection
// ---------------------------------------------------------------------------

/// Le nom de la zone qu'une sélection produit quand on la colore.
///
/// # Le défaut que ceci corrige
///
/// Colorer une sélection partielle pendant qu'une animation tourne la déclare
/// en zone, pour que le reste du boîtier garde la sienne (#63). Ce nom-là était
/// le **libellé d'affichage** de la sélection — « 3 organes entiers », « 24 LED
/// sur 4 organes » —, et un libellé n'est pas un identifiant : **deux
/// sélections différentes de trois organes s'y écrivent pareil**.
///
/// La seconde réécrivait donc la zone de la première, et lui **prenait ses
/// LED** — une LED n'appartenant qu'à une zone à la fois. Symptôme observé :
/// « j'ai donné une couleur à une zone, ça a changé l'animation d'une autre, et
/// pas d'une troisième ». Aucun message : les deux zones existaient toujours.
///
/// ⚠️ **Le déterminisme reste la propriété qu'on veut** : deux fois la même
/// sélection rendent le même nom, sans quoi chaque clic empilerait une zone de
/// plus. Ce qui manquait, c'est l'**injectivité** — deux sélections différentes
/// doivent rendre deux noms différents.
pub fn nom_de_zone(cibles: &[Cible]) -> String {
    // Trié : les cibles sont ajoutées dans l'ordre du geste, et deux sélections
    // des mêmes LED prises dans un ordre différent sont la même sélection.
    let mut rangs: Vec<String> = cibles
        .iter()
        .map(|cible| match cible {
            Cible::Led { position, led } => format!("f{}-{led}", position.slug()),
            Cible::Barrette { slot, led } => format!("s{slot}-{led}"),
        })
        .collect();
    rangs.sort();
    rangs.dedup();
    format!("sélection-{:08x}", empreinte(&rangs.join(",")))
}

/// Une empreinte stable d'une chaîne.
///
/// FNV-1a 32 bits, huit lignes. ⚠️ **`DefaultHasher` ne promet pas d'être
/// stable entre deux versions de Rust**, et ce nom-là finit dans
/// `/var/lib/reverb/zones.conf` : une zone qui changerait de nom à une montée
/// de compilateur laisserait l'ancienne derrière elle, sans un mot.
fn empreinte(texte: &str) -> u32 {
    let mut somme: u32 = 0x811c_9dc5;
    for octet in texte.as_bytes() {
        somme ^= u32::from(*octet);
        somme = somme.wrapping_mul(0x0100_0193);
    }
    somme
}
