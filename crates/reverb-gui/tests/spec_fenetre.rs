//! Tests d'intention de la fenêtre repensée (issue #76).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #76 seule. Rien de `crates/reverb-gui/src/`
//! ni de `crates/reverb-gui/ui/` n'a été lu pour les produire : ni corps de fonction, ni module,
//! ni `.slint`. Seules les signatures publiques déjà figées par les fichiers de spécification
//! voisins ont été relevées — `Reglage` (#32), `sondes_retenues` (#51) —, pour savoir à côté de
//! quoi la nouvelle API vient s'asseoir. À l'écriture de ce fichier, aucun des symboles listés
//! ci-dessous n'existe : la compilation doit échouer sur eux, et **sur eux seuls**. C'est la phase
//! rouge.
//!
//! Rien ici n'ouvre de fenêtre, ne parle à un socket, ni ne touche un bus.
//!
//! ## Ce qu'on peut tester d'une fenêtre, et ce qu'on ne peut pas
//!
//! ⚠️ **Une mise en page ne s'écrit pas en assertions.** Ce qui se vérifie d'une interface, c'est
//! la **couverture du protocole** : pour chaque chose que le démon sait faire, la fenêtre sait
//! fabriquer la requête correspondante — et cette requête est celle que le démon accepte, pas une
//! qui lui ressemble. L'esthétique se juge à l'œil, sur des images, hors de ce fichier.
//!
//! C'est aussi ce qui décide de la forme du contrat : la liste de ce que la fenêtre **offre** doit
//! être une valeur, et non une suite d'entrées écrites à la main dans le `.slint`. Une liste
//! écrite là serait juste le jour où on l'écrit et fausse le jour où le catalogue s'allonge —
//! exactement ce qui est arrivé entre #75 et #76, quatre familles et deux directions étant restées
//! hors de portée de clic sans qu'aucun message ne le dise.
//!
//! ## Le contrat que ces tests figent
//!
//! ```ignore
//! // dans crates/reverb-gui/src/reglages.rs, à côté de `requetes_vers_la_cible` (#47)
//! // et `requetes_pour_la_couleur` (#63).
//!
//! /// Les familles d'animation que la fenêtre met à portée de clic.
//! pub fn familles_offertes() -> Vec<&'static str>;
//!
//! /// Les directions qu'elle propose, dans l'ordre où `Reglage::direction` les range.
//! pub fn directions_offertes() -> Vec<Direction>;
//!
//! /// La commande d'animation qu'elle émet, sonde comprise.
//! ///
//! /// `sonde` est le **slug** choisi dans le panneau des sondes, `None` quand aucune ne l'est.
//! pub fn requete_d_animation(
//!     reglage: &Reglage,
//!     sonde: Option<&str>,
//! ) -> Result<Request, ReglageInvalide>;
//!
//! /// Ce que le panneau des profils demande, avec le nom **tel qu'il a été tapé**.
//! pub enum ChoixDeProfil {
//!     Lister,
//!     Enregistrer(String),
//!     Rappeler(String),
//!     Oublier(String),
//! }
//! pub fn requete_de_profil(choix: &ChoixDeProfil) -> Result<Request, NomInvalide>;
//!
//! /// Ce que le panneau de composition demande.
//! pub enum ChoixDeComposition {
//!     Etat,
//!     Fond(Fond),
//!     Champ(Ancre, Source),
//!     Vide(Ancre),
//!     Aucune,
//! }
//! pub fn requete_de_composition(choix: &ChoixDeComposition) -> Request;
//! ```
//!
//! `ChoixDeProfil` et `ChoixDeComposition` portent `Debug`, `Clone`, `PartialEq` et `Eq`.
//!
//! ## Ce que l'issue laisse ouvert, et que ces tests tranchent
//!
//! 1. **`Reglage` garde ses quatre champs, et la sonde entre par un paramètre.** Lui en ajouter un
//!    cinquième casserait les littéraux de `spec_reglages.rs` (#32) et de `spec_couleur_animee.rs`
//!    (#63) — deux fichiers de tests d'intention, qui ne se réécrivent pas pour arranger un design
//!    venu après eux. Le précédent est celui de #63, qui a posé `requetes_pour_la_couleur` à côté
//!    de `Reglage::commande` plutôt que de la changer.
//! 2. **Choisir « aucune » dans la liste des familles émet `animate off`.** C'est un geste, pas
//!    l'absence de geste : la fenêtre a besoin d'un moyen d'éteindre, et le protocole n'en a qu'un
//!    (`Request::Animate { name: None }`). Cela ne contredit pas
//!    `bouger_les_curseurs_sans_animation_en_cours_n_envoie_rien` (#32), qui porte sur
//!    `Reglage::commande` et sur un **autre déclencheur** : bouger un curseur à vide ne doit rien
//!    envoyer, choisir « aucune » doit éteindre.
//! 3. **Le refus est une valeur, jamais un bouton grisé.** `thermique` exige une sonde ; une
//!    fenêtre qui enverrait la commande quand même se ferait refuser par le démon, et une fenêtre
//!    qui grise son bouton ne se vérifie pas sans ouvrir de session graphique. D'où un `Result`,
//!    dont l'erreur est **celle du démon lui-même** — `ReglageInvalide`, rendue par
//!    `Animation::reglages` : la fenêtre refuse exactement ce que le démon refuserait, par le même
//!    code, et non par une seconde règle qui divergerait de la première.
//! 4. **Le nom d'un profil est validé par `NomProfil::nouveau`, et par rien d'autre.** D'où
//!    `NomInvalide` en erreur : c'est le type que le protocole rend déjà. Une seconde validation
//!    écrite dans la fenêtre serait une seconde règle à tenir d'accord avec la première, et la
//!    façon dont ce garde-fou cède est justement de **nettoyer** le nom au lieu de le refuser
//!    (`reverb-proto/src/profil.rs`).
//! 5. **`requete_de_composition` est infaillible.** Ses cinq variantes se projettent une à une sur
//!    les cinq actions `screen layout` du protocole, et les refus qui restent — chemin relatif,
//!    cinquième champ — sont déjà tenus par le démon, qui les nomme. En inventer une copie côté
//!    fenêtre ferait deux règles pour un seul refus.
//! 6. **La liste des familles n'est pas ordonnée par ce fichier.** Elle doit *couvrir* le
//!    catalogue, sans doublon et sans intrus ; l'ordre d'affichage est une décision de mise en
//!    page. Celle des directions, si : `Reglage::direction` est un **rang** dans cette liste, et
//!    une liste plus courte que `Direction::ALL` rendrait les deux directions locales de #75
//!    inatteignables — la faute que le critère n° 2 de l'issue existe pour attraper.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **La mise en page, les couleurs, la place des panneaux.** Voir plus haut : ça se regarde.
//! - **Les deux critères d'aperçu et de coordonnées de l'issue** (« `apercu` produit un fichier non
//!   vide pour les deux vues », « aucune LED n'a de coordonnée qui ne vienne de `plan.rs` »). Le
//!   premier est un critère d'**outil** : le vérifier demanderait d'instancier Slint ou de
//!   relancer `cargo` depuis un test, ce que ce dépôt ne fait nulle part. Le second est déjà tenu,
//!   et mieux qu'un `grep` du `.slint` ne le ferait : `spec_maquette.rs` exige que les cent
//!   vingt-quatre LED soient placées par le plan **dans les deux vues**, et
//!   `la_silhouette_suit_la_geometrie_et_n_est_pas_une_constante` (`spec_habillage.rs`) interdit
//!   qu'une forme soit une constante plutôt qu'une conséquence de la géométrie.
//! - **Le profil actif.** L'issue veut « voir lequel est actif » ; le protocole ne le dit pas. Ses
//!   états sont `connu`, `cree`, `ecrase`, `applique`, `oublie` (`ResponseLine::Profil`), et aucun
//!   ne survit à la réponse qui le porte. Le savoir demanderait soit une mémoire de fenêtre que
//!   rien ne spécifie — que devient-elle quand on oublie le profil actif ? quand on change une
//!   couleur après l'avoir rappelé ? —, soit un changement de protocole. Aucun des deux n'est
//!   décrit par l'issue, et l'inventer figerait une règle que personne n'a choisie.
//! - **La lecture des réponses** : la liste des profils rendue par `profil list`, la composition
//!   rendue par `screen layout`. C'est le sens démon → fenêtre, de la famille d'`eclairage_lu`
//!   (#43) ; l'issue ne le mentionne dans aucun de ses tests d'intention.
//! - **Le plafond de quatre champs.** Il appartient à `Composition::CHAMPS_MAX`, que #80 tient
//!   déjà, et la fenêtre ne peut le respecter qu'en connaissant la composition courante — donc
//!   après la lecture ci-dessus.

use reverb_anim::{Animation, CATALOGUE, Direction, Reglages};
use reverb_gui::reglages::{
    ChoixDeComposition, ChoixDeProfil, Reglage, directions_offertes, familles_offertes,
    requete_d_animation, requete_de_composition, requete_de_profil,
};
use reverb_gui::sondes::{ModelesNvme, SondeRetenue, sondes_retenues};
use reverb_proto::composition::{Ancre, Fond, Source};
use reverb_proto::ipc::{ProfilAction, Request, ScreenAction, encode_request, parse_request};
use reverb_proto::{NomProfil, Rgb};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// La couleur affichée par le panneau. **Différente du défaut de `Reglages`** : le décodeur du
/// démon comble une clé absente par son défaut, donc une couleur perdue en route rendrait le même
/// résultat qu'une couleur transmise si le repère était le défaut.
const COULEUR: Rgb = Rgb::new(0x12, 0x9a, 0x40);

/// La vitesse affichée. Même précaution.
const VITESSE: u8 = 7;

/// La direction affichée, et elle est **locale** : c'est l'une des deux que #75 a ajoutées, donc
/// celle qu'une liste restée à six rendrait inatteignable.
const DIRECTION: Direction = Direction::BordsCentre;

/// L'animation qui exige une sonde. La seule du catalogue, et le cœur du critère n° 4 de l'issue.
const THERMIQUE: &str = "thermique";

/// Une famille qui accepte couleur, vitesse et direction — le cas courant.
const AVEC_DIRECTION: &str = "vague";

/// Une famille qui n'accepte **aucune** direction (#75) : lui en donner une ferait refuser la
/// commande entière, pas seulement la clé.
const SANS_DIRECTION: &str = "rotation";

/// Le CPU, tel que `status` le nomme.
const CPU: &str = "k10temp:tctl";
/// Le liquide du Kraken.
const LIQUIDE: &str = "kraken2023elite:coolant-temp";
/// La carte graphique.
const GPU: &str = "nvidia:NVIDIA_GeForce_RTX_5070";
/// Le premier disque NVMe.
const NVME0: &str = "nvme:nvme0:composite";
/// Le second disque NVMe.
const NVME1: &str = "nvme:nvme1:composite";

/// Ce que le démon rend à `status` : les cinq sondes que la fenêtre retient, mêlées à quelques-unes
/// des onze qu'elle écarte (#51).
const RENDUES: [&str; 8] = [
    "amdgpu:edge",
    CPU,
    LIQUIDE,
    "spd5118:8-0050:temp1",
    GPU,
    NVME0,
    NVME1,
    "r8169_0_e00:00:temp1",
];

/// Un nom de profil qui porte une espace et deux accents — « soirée d'été » du README.
const NOM_PROFIL: &str = "soirée d'été";

/// Un nom qui doit être refusé : il désigne un fichier hors du répertoire des profils.
const NOM_HORS_REPERTOIRE: &str = "../geometrie";

/// Un chemin d'image absolu **qui porte une espace** : coupé au premier blanc, il désignerait un
/// fichier qui n'existe pas.
const CHEMIN_FOND: &str = "/home/nico/mes photos/abysse.png";

/// Le libellé d'un champ de composition, avec une espace lui aussi.
const LIBELLE_CHAMP: &str = "LIQUIDE KRAKEN";

// ---------------------------------------------------------------------------
// Aides
// ---------------------------------------------------------------------------

/// Le rang d'une direction dans [`Direction::ALL`], tel que [`Reglage::direction`] le porte.
fn rang(direction: Direction) -> usize {
    Direction::ALL
        .into_iter()
        .position(|connue| connue == direction)
        .unwrap_or_else(|| panic!("{direction:?} est une des huit directions"))
}

/// Les réglages tels que la fenêtre les affiche.
fn reglage(animation: Option<&str>) -> Reglage {
    Reglage {
        animation: animation.map(str::to_owned),
        couleur: COULEUR,
        vitesse: VITESSE,
        direction: rang(DIRECTION),
        // Champ ajouté par #126 : « aucune palette », le comportement d'avant.
        palette: None,
    }
}

/// Le nom et les paires portés par la requête d'animation, ou un échec qui dit ce qui est venu.
fn animate(reglage: &Reglage, sonde: Option<&str>) -> (String, Vec<(String, String)>) {
    match requete_d_animation(reglage, sonde) {
        Ok(Request::Animate {
            name: Some(nom),
            reglages,
        }) => (nom, reglages),
        Ok(Request::Animate {
            name: None,
            reglages,
        }) => panic!(
            "choisir « {:?} » doit lancer cette animation, pas l'éteindre : « animate off » reçu, \
             avec {reglages:?}",
            reglage.animation
        ),
        Ok(autre) => panic!(
            "le panneau d'animation émet un « animate », pas un autre verbe : {autre:?} pour \
             {reglage:?}"
        ),
        Err(erreur) => panic!("{reglage:?} avec sonde={sonde:?} doit être acceptée : {erreur}"),
    }
}

/// Ce que le démon lira de ces paires — c'est `reverb-anim` qui les valide de l'autre côté du
/// socket, et son refus porte sur la commande **entière**.
fn relu(nom: &str, paires: &[(String, String)]) -> Reglages {
    let animation = Animation::par_nom(nom)
        .unwrap_or_else(|erreur| panic!("« {nom} » doit être au catalogue : {erreur}"));
    animation.reglages(paires).unwrap_or_else(|erreur| {
        panic!("le démon doit accepter {paires:?} pour « {nom} » : {erreur}")
    })
}

/// La valeur portée pour une clé, si elle est portée.
fn valeur<'a>(paires: &'a [(String, String)], cle: &str) -> Option<&'a str> {
    paires
        .iter()
        .find(|(portee, _)| portee == cle)
        .map(|(_, valeur)| valeur.as_str())
}

/// Les clés qu'une animation du catalogue accepte.
fn acceptees(nom: &str) -> &'static [&'static str] {
    Animation::par_nom(nom)
        .unwrap_or_else(|erreur| panic!("« {nom} » doit être au catalogue : {erreur}"))
        .parametres_acceptes()
}

/// La ligne que cette requête écrit sur le socket, et ce que le démon en relit.
///
/// Une requête qui ne fait pas l'aller-retour n'est pas une requête : elle est bien formée dans la
/// fenêtre et devient autre chose — ou rien — de l'autre côté.
fn aller_retour(requete: &Request) -> String {
    let ligne = encode_request(requete);
    let relue = parse_request(&ligne)
        .unwrap_or_else(|erreur| panic!("le démon doit comprendre « {ligne} » : {erreur}"));
    assert_eq!(
        relue, *requete,
        "« {ligne} » ne rend pas la requête qui l'a écrite"
    );
    ligne
}

/// Les modèles de disques, tels que `sysfs` les rend sur SHYNAEL.
fn modeles() -> ModelesNvme {
    ModelesNvme {
        nvme0: Some("CT2000T705SSD5".to_owned()),
        nvme1: Some("CT4000P3SSD8".to_owned()),
    }
}

/// Le panneau des sondes, tel que la fenêtre le montre.
fn panneau() -> Vec<SondeRetenue> {
    let rendues: Vec<String> = RENDUES.iter().map(|slug| (*slug).to_owned()).collect();
    sondes_retenues(&rendues, &modeles())
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Garde-fou, pas critère. Le décodeur du démon comble une clé absente par le défaut de
    // `Reglages` : si les valeurs choisies plus haut étaient ces défauts, tous les tests qui
    // suivent passeraient aussi bien sur une implémentation qui n'envoie **rien**. Ce test est la
    // condition de validité des autres.
    let defaut = Reglages::default();
    assert_ne!(
        COULEUR, defaut.couleur,
        "la couleur du panneau doit différer du défaut, sinon une couleur perdue passe inaperçue"
    );
    assert_ne!(
        VITESSE, defaut.vitesse,
        "la vitesse du panneau doit différer du défaut {}",
        defaut.vitesse
    );
    assert_ne!(
        DIRECTION, defaut.direction,
        "la direction du panneau doit différer du défaut {:?}",
        defaut.direction
    );
    assert!(
        DIRECTION.est_locale(),
        "{DIRECTION:?} doit être une des deux directions locales de #75 : ce sont elles qu'une \
         liste restée à six rend inatteignables"
    );

    // Et le second garde-fou, celui du critère n° 4 : « présentée sous son nom lisible, la requête
    // porte son slug » ne veut rien dire si les deux se confondent.
    for sonde in panneau() {
        assert_ne!(
            sonde.libelle, sonde.slug,
            "le libellé de « {} » doit se lire autrement que son slug, sinon ce fichier ne prouve \
             rien sur ce qui part au démon",
            sonde.slug
        );
    }
}

// ---------------------------------------------------------------------------
// 1 — chaque famille du catalogue est atteignable
// ---------------------------------------------------------------------------

mod familles {
    use super::{
        AVEC_DIRECTION, Animation, CATALOGUE, COULEUR, DIRECTION, LIQUIDE, Request, SANS_DIRECTION,
        THERMIQUE, VITESSE, acceptees, aller_retour, animate, familles_offertes, reglage, relu,
        requete_d_animation, valeur,
    };

    #[test]
    fn chaque_famille_du_catalogue_est_atteignable_depuis_la_fenetre() {
        // Test d'intention n° 1 de l'issue : « chaque famille de `CATALOGUE` est atteignable depuis
        // la fenêtre ». C'est le grief qui ouvre l'issue — #75 a livré quatre familles que la
        // fenêtre ne montre pas, et rien ne le signale : le panneau a l'air complet.
        let offertes = familles_offertes();
        for nom in CATALOGUE {
            assert!(
                offertes.contains(nom),
                "« {nom} » est au catalogue et doit être à portée de clic — la fenêtre n'offre que \
                 {offertes:?}"
            );
        }

        // Et rien d'autre : une entrée qui n'est pas au catalogue produirait une commande que le
        // démon refuse en bloc.
        for offerte in &offertes {
            assert!(
                Animation::par_nom(offerte).is_ok(),
                "« {offerte} » est offerte par la fenêtre sans être au catalogue : {CATALOGUE:?}"
            );
        }
        assert!(
            !offertes.contains(&"off"),
            "« off » n'est pas une famille mais l'absence d'animation (`CATALOGUE`) : deux chemins \
             pour éteindre en rendraient un des deux faux — {offertes:?}"
        );

        // Sans doublon : une famille listée deux fois est un menu où le même clic fait deux choses
        // différentes selon la ligne.
        for (rang, offerte) in offertes.iter().enumerate() {
            assert!(
                !offertes.iter().skip(rang + 1).any(|autre| autre == offerte),
                "« {offerte} » est offerte deux fois : {offertes:?}"
            );
        }
    }

    #[test]
    fn chaque_famille_offerte_produit_une_commande_que_le_demon_accepte() {
        // Même critère, sa seconde moitié : « atteignable » veut dire qu'un clic produit une
        // commande **jouable**, pas qu'un nom figure dans un menu. Le refus du démon porte sur la
        // commande entière — une clé de trop, et l'animation ne part pas du tout.
        for nom in familles_offertes() {
            let regle = reglage(Some(nom));
            let (porte, paires) = animate(&regle, Some(LIQUIDE));
            assert_eq!(porte, nom, "« {nom} » se lance sous son propre nom");

            let clefs_acceptees = acceptees(nom);
            for (cle, _) in &paires {
                assert!(
                    clefs_acceptees.contains(&cle.as_str()),
                    "« {nom} » n'accepte que {clefs_acceptees:?} : la clé « {cle} » ferait refuser \
                     la commande entière — {paires:?}"
                );
            }
            for (rang, (cle, _)) in paires.iter().enumerate() {
                assert!(
                    !paires.iter().skip(rang + 1).any(|(autre, _)| autre == cle),
                    "« {cle} » est portée deux fois par « {nom} » : {paires:?}"
                );
            }

            // Ce qui est affiché est ce qui part : une clé acceptée mais laissée de côté ferait
            // retomber le démon sur son défaut, sans un mot. C'est le défaut de #32, par une autre
            // porte.
            let lu = relu(nom, &paires);
            assert_eq!(
                lu.vitesse, VITESSE,
                "« {nom} » reçoit la vitesse affichée : {paires:?}"
            );
            if clefs_acceptees.contains(&"couleur") {
                assert_eq!(
                    lu.couleur, COULEUR,
                    "« {nom} » accepte la couleur, donc il reçoit celle qui est affichée : \
                     {paires:?}"
                );
            }
            if clefs_acceptees.contains(&"direction") {
                assert_eq!(
                    lu.direction, DIRECTION,
                    "« {nom} » reçoit la direction affichée : {paires:?}"
                );
            }
            if clefs_acceptees.contains(&"sonde") {
                assert_eq!(
                    valeur(&paires, "sonde"),
                    Some(LIQUIDE),
                    "« {nom} » suit la sonde choisie : {paires:?}"
                );
            }

            aller_retour(&Request::Animate {
                name: Some(nom.to_owned()),
                reglages: paires,
            });
        }
    }

    #[test]
    fn une_famille_qui_ne_suit_aucune_direction_n_en_recoit_pas_meme_une_affichee() {
        // Corollaire du même critère, et le piège de #75 : `rotation`, `pouls` et `scintillement`
        // n'acceptent pas `direction` — le curseur reste affiché, et la lui porter ferait rejeter
        // l'`animate` entier. Le symptôme est celui de #32 : un panneau qui ne fait rien.
        let (_, paires) = animate(&reglage(Some(SANS_DIRECTION)), None);
        assert_eq!(
            valeur(&paires, "direction"),
            None,
            "« {SANS_DIRECTION} » ne suit aucune direction du boîtier : la lui donner ferait \
             refuser la commande entière — {paires:?}"
        );
        assert_eq!(
            valeur(&paires, "sonde"),
            None,
            "« {SANS_DIRECTION} » ne suit aucune sonde : {paires:?}"
        );

        // Et la sonde ne s'invite pas non plus chez qui n'en veut pas, même quand le panneau en
        // affiche une : c'est le même refus en bloc.
        let (_, avec_sonde_choisie) = animate(&reglage(Some(AVEC_DIRECTION)), Some(LIQUIDE));
        assert_eq!(
            valeur(&avec_sonde_choisie, "sonde"),
            None,
            "« {AVEC_DIRECTION} » n'accepte pas « sonde » : une sonde choisie pour une autre \
             famille ne doit pas la suivre — {avec_sonde_choisie:?}"
        );
    }

    #[test]
    fn une_famille_qui_exige_une_sonde_est_refusee_sans_elle_en_la_nommant() {
        // Critère n° 4 de l'issue, sa moitié de refus, et README : « la sonde est exigée, pas
        // seulement acceptée — seule du catalogue ». Une fenêtre qui enverrait quand même se ferait
        // refuser par le démon ; une fenêtre qui se tairait laisserait un clic sans effet.
        //
        // Le balayage porte sur `parametres_obligatoires`, et non sur le seul nom `thermique` :
        // c'est la source de vérité, et une seconde famille à réglage obligatoire hériterait du
        // même refus sans qu'on y revienne.
        for nom in familles_offertes() {
            let animation = Animation::par_nom(nom)
                .unwrap_or_else(|erreur| panic!("« {nom} » doit être au catalogue : {erreur}"));
            let obligatoires = animation.parametres_obligatoires();
            if obligatoires.is_empty() {
                continue;
            }
            let refus = requete_d_animation(&reglage(Some(nom)), None);
            let erreur = refus.expect_err(&format!(
                "« {nom} » exige {obligatoires:?} : sans sonde, la fenêtre ne doit rien envoyer"
            ));
            assert!(
                obligatoires.contains(&erreur.cle.as_str()),
                "le refus doit nommer ce qui manque — {obligatoires:?} attendu, « {} » reçu : {}",
                erreur.cle,
                erreur.raison
            );
        }

        // La borne du balayage, nommée, pour qu'un échec dise laquelle est tombée.
        assert!(
            !acceptees(THERMIQUE).is_empty(),
            "« {THERMIQUE} » doit rester au catalogue : c'est elle que ce test vise"
        );
        assert!(
            requete_d_animation(&reglage(Some(THERMIQUE)), Some(LIQUIDE)).is_ok(),
            "« {THERMIQUE} » avec une sonde choisie doit partir"
        );
    }

    #[test]
    fn choisir_aucune_animation_eteint_le_boitier() {
        // L'entrée « aucune » du menu des familles. Le protocole n'a qu'un moyen d'éteindre —
        // `Request::Animate { name: None }` —, et la fenêtre doit pouvoir l'atteindre.
        //
        // ⚠️ Ne contredit pas `bouger_les_curseurs_sans_animation_en_cours_n_envoie_rien` (#32) :
        // celui-là porte sur `Reglage::commande` et sur un autre déclencheur — bouger un curseur à
        // vide ne doit rien envoyer, choisir « aucune » doit éteindre.
        let requete = requete_d_animation(&reglage(None), None)
            .expect("choisir « aucune » ne peut pas être refusé");
        match &requete {
            Request::Animate { name, reglages } => {
                assert!(
                    name.is_none(),
                    "« aucune » éteint, elle ne relance rien : {requete:?}"
                );
                assert!(
                    reglages.is_empty(),
                    "« animate off » ne porte aucun réglage : il n'y a plus d'animation à régler — \
                     {reglages:?}"
                );
            }
            autre => panic!("« aucune » émet un « animate off », pas {autre:?}"),
        }
        assert_eq!(aller_retour(&requete), "animate off");
    }
}

// ---------------------------------------------------------------------------
// 2 — chaque direction est atteignable
// ---------------------------------------------------------------------------

mod directions {
    use super::{
        AVEC_DIRECTION, Direction, Reglage, acceptees, animate, directions_offertes, reglage, relu,
    };

    #[test]
    fn chaque_direction_est_offerte_et_les_deux_locales_avec_les_autres() {
        // Test d'intention n° 2 de l'issue : « chaque direction de `Direction::ALL` est
        // atteignable ». #75 en a ajouté deux, et une liste restée à six les rend invisibles sans
        // qu'aucun message ne le dise.
        let offertes = directions_offertes();
        assert_eq!(
            offertes,
            Direction::ALL.to_vec(),
            "les huit directions, dans l'ordre de `Direction::ALL` : `Reglage::direction` est un \
             **rang** dans cette liste, et un ordre à soi ferait choisir une direction pour une \
             autre — sans message, puisque toutes les couleurs rendues restent plausibles"
        );

        // Nommément, parce qu'un échec doit dire laquelle manque plutôt que « les listes diffèrent ».
        for locale in Direction::ALL.into_iter().filter(|d| d.est_locale()) {
            assert!(
                offertes.contains(&locale),
                "« {} » est une direction locale de #75 : le motif s'y répète sur chaque objet, et \
                 c'est ce qu'iCUE fait sur la RAM — {offertes:?}",
                locale.slug()
            );
        }
    }

    #[test]
    fn la_direction_choisie_est_celle_que_le_demon_recoit() {
        // Même critère : « atteignable » veut dire que le choix arrive de l'autre côté du socket,
        // pas qu'une ligne s'affiche dans un menu. Le rang est le seul endroit du panneau où une
        // erreur d'indice ne produit aucun message — juste une autre direction.
        assert!(
            acceptees(AVEC_DIRECTION).contains(&"direction"),
            "« {AVEC_DIRECTION} » doit accepter « direction » : c'est par elle que ce test regarde"
        );
        for (rang, attendue) in directions_offertes().into_iter().enumerate() {
            let regle = Reglage {
                direction: rang,
                ..reglage(Some(AVEC_DIRECTION))
            };
            let (_, paires) = animate(&regle, None);
            assert_eq!(
                relu(AVEC_DIRECTION, &paires).direction,
                attendue,
                "le rang {rang} du menu est « {} » : {paires:?}",
                attendue.slug()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — les quatre verbes de profil
// ---------------------------------------------------------------------------

mod profils {
    use super::{
        ChoixDeProfil, NOM_HORS_REPERTOIRE, NOM_PROFIL, NomProfil, ProfilAction, Request,
        aller_retour, requete_de_profil,
    };

    /// Le nom validé, tel que le protocole le construit.
    fn nom() -> NomProfil {
        NomProfil::nouveau(NOM_PROFIL)
            .unwrap_or_else(|erreur| panic!("« {NOM_PROFIL} » est un nom légitime : {erreur}"))
    }

    #[test]
    fn les_quatre_verbes_de_profil_sont_atteignables() {
        // Test d'intention n° 3 de l'issue : « les quatre verbes de profil sont atteignables ».
        // #74 a livré `save`, `load`, `drop` et `list` ; la fenêtre n'en expose aucun.
        let attendus = [
            (ChoixDeProfil::Lister, ProfilAction::List),
            (
                ChoixDeProfil::Enregistrer(NOM_PROFIL.to_owned()),
                ProfilAction::Save(nom()),
            ),
            (
                ChoixDeProfil::Rappeler(NOM_PROFIL.to_owned()),
                ProfilAction::Load(nom()),
            ),
            (
                ChoixDeProfil::Oublier(NOM_PROFIL.to_owned()),
                ProfilAction::Drop(nom()),
            ),
        ];
        for (choix, action) in attendus {
            let requete = requete_de_profil(&choix)
                .unwrap_or_else(|erreur| panic!("{choix:?} doit partir : {erreur}"));
            assert_eq!(
                requete,
                Request::Profil(action.clone()),
                "{choix:?} demande « {action:?} »"
            );
            aller_retour(&requete);
        }
    }

    #[test]
    fn un_nom_qui_porte_espaces_et_accents_traverse_intact() {
        // README : « un nom peut porter des espaces et des accents — “soirée d'été”, “LAN party” ».
        // Il est le dernier champ de sa ligne et va jusqu'au bout ; coupé au premier blanc, il
        // désignerait « soirée », un profil qui n'existe pas.
        let requete = requete_de_profil(&ChoixDeProfil::Rappeler(NOM_PROFIL.to_owned()))
            .expect("« soirée d'été » est un nom légitime");
        let ligne = aller_retour(&requete);
        assert_eq!(
            ligne,
            format!("profil load {NOM_PROFIL}"),
            "le nom va jusqu'au bout de la ligne, espaces comprises"
        );
    }

    #[test]
    fn un_nom_qui_sortirait_du_repertoire_est_refuse_sans_etre_nettoye() {
        // README : « un nom ne peut pas désigner un fichier ailleurs » — le démon est root. Et
        // `reverb-proto/src/profil.rs` : « la façon dont ce garde-fou cède habituellement n'est pas
        // de laisser passer un nom hostile : c'est de le **nettoyer** ». Une fenêtre qui retirerait
        // les « .. » pour être aimable ferait pointer deux profils sur le même fichier.
        let refus = requete_de_profil(&ChoixDeProfil::Enregistrer(NOM_HORS_REPERTOIRE.to_owned()));
        let erreur = refus.expect_err(&format!(
            "« {NOM_HORS_REPERTOIRE} » désigne un fichier hors du répertoire des profils : rien ne \
             doit partir"
        ));
        assert_eq!(
            erreur.saisi, NOM_HORS_REPERTOIRE,
            "le refus montre ce qui a été tapé, sans réécriture : sinon on cherche"
        );
        assert!(
            !erreur.raison.is_empty(),
            "le refus dit ce qui cloche — un « nom invalide » sec n'apprend rien"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — une sonde se lit sous son nom, et voyage sous son slug
// ---------------------------------------------------------------------------

mod sondes {
    use super::{
        Ancre, ChoixDeComposition, LIBELLE_CHAMP, Source, THERMIQUE, aller_retour, animate,
        panneau, reglage, relu, requete_de_composition, valeur,
    };

    #[test]
    fn une_sonde_est_offerte_sous_son_nom_lisible_et_la_requete_porte_son_slug() {
        // Test d'intention n° 4 de l'issue. Le panneau SONDES montre « CPU », « Liquide », « GPU »
        // et les disques sous leur modèle (#51) ; le protocole, lui, n'accepte que le slug —
        // README : « le nom à donner est celui du protocole, pas celui de la fenêtre ».
        //
        // C'est le seul endroit de la fenêtre où deux chaînes désignent la même chose, et les
        // échanger ne produit aucun message : `animate thermique sonde=Liquide` est une commande
        // parfaitement formée, refusée par le démon ou — pire — acceptée et jamais relevée.
        for sonde in panneau() {
            let (_, paires) = animate(&reglage(Some(THERMIQUE)), Some(&sonde.slug));
            assert_eq!(
                valeur(&paires, "sonde"),
                Some(sonde.slug.as_str()),
                "« {} » se montre sous « {} » et voyage sous son slug : {paires:?}",
                sonde.slug,
                sonde.libelle
            );
            assert_eq!(
                relu(THERMIQUE, &paires).sonde.as_deref(),
                Some(sonde.slug.as_str()),
                "le démon relève « {} », pas autre chose",
                sonde.slug
            );
        }
    }

    #[test]
    fn un_champ_de_composition_porte_le_slug_et_le_libelle_a_leur_place() {
        // Même critère, sur l'autre panneau où une sonde se choisit (#80) : « screen layout champ
        // <ancre> temp <sonde> <libellé> ». Deux chaînes voisines sur la même ligne, dont l'une est
        // le slug et l'autre ce qu'on lit sur la dalle — les échanger donnerait une commande
        // acceptée, et six centimètres de dalle affichant « k10temp:tctl » sous des tirets.
        for sonde in panneau() {
            let source = Source::Temperature {
                sonde: sonde.slug.clone(),
                libelle: Some(LIBELLE_CHAMP.to_owned()),
            };
            let requete = requete_de_composition(&ChoixDeComposition::Champ(Ancre::Haut, source));
            let ligne = aller_retour(&requete);
            assert_eq!(
                ligne,
                format!(
                    "screen layout champ {} temp {} {LIBELLE_CHAMP}",
                    Ancre::Haut.slug(),
                    sonde.slug
                ),
                "le slug est un jeton, le libellé est le dernier champ et garde ses espaces"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 7 — les cinq ancres de la composition
// ---------------------------------------------------------------------------

mod composition {
    use super::{
        Ancre, CHEMIN_FOND, ChoixDeComposition, Fond, LIBELLE_CHAMP, Request, ScreenAction, Source,
        aller_retour, requete_de_composition,
    };

    #[test]
    fn les_cinq_ancres_sont_atteignables_et_la_requete_est_celle_du_protocole() {
        // Critère ajouté après la rédaction de l'issue, #80 étant arrivé entre-temps : « les cinq
        // ancres de composition sont atteignables, et la requête émise est bien celle du
        // protocole ».
        //
        // Les cinq, et pas quatre : `centre` est la seule qui puisse porter une valeur seule au
        // milieu de la dalle, et une ancre oubliée dans un menu ne se remarque pas — les quatre
        // autres composent une mise en page qui a l'air complète.
        for ancre in Ancre::TOUTES {
            let source = Source::Texte(LIBELLE_CHAMP.to_owned());
            let pose = requete_de_composition(&ChoixDeComposition::Champ(ancre, source.clone()));
            assert_eq!(
                pose,
                Request::Screen(ScreenAction::LayoutChamp(ancre, source)),
                "poser un champ sur « {} » demande « screen layout champ »",
                ancre.slug()
            );
            assert_eq!(
                aller_retour(&pose),
                format!("screen layout champ {} texte {LIBELLE_CHAMP}", ancre.slug())
            );

            let vide = requete_de_composition(&ChoixDeComposition::Vide(ancre));
            assert_eq!(
                vide,
                Request::Screen(ScreenAction::LayoutVide(ancre)),
                "retirer le champ de « {} » demande « screen layout vide »",
                ancre.slug()
            );
            assert_eq!(
                aller_retour(&vide),
                format!("screen layout vide {}", ancre.slug())
            );
        }
    }

    #[test]
    fn le_fond_l_extinction_et_l_etat_sont_atteignables() {
        // Même critère, les trois autres actions de `screen layout` — sans elles, on entre dans une
        // composition sans pouvoir en sortir, et la dalle garde un affichage que rien ne défait.
        let noir = requete_de_composition(&ChoixDeComposition::Fond(Fond::Noir));
        assert_eq!(
            noir,
            Request::Screen(ScreenAction::LayoutFond(Fond::Noir)),
            "poser un fond noir demande « screen layout fond noir »"
        );
        assert_eq!(aller_retour(&noir), "screen layout fond noir");

        // ⚠️ Le chemin porte une espace, et il est le dernier champ de sa ligne : coupé au premier
        // blanc, il désignerait « /home/nico/mes », un fichier qui n'existe pas.
        let image = requete_de_composition(&ChoixDeComposition::Fond(Fond::Image(
            CHEMIN_FOND.to_owned(),
        )));
        assert_eq!(
            image,
            Request::Screen(ScreenAction::LayoutFond(Fond::Image(
                CHEMIN_FOND.to_owned()
            ))),
            "poser une image de fond demande « screen layout fond image <chemin> »"
        );
        assert_eq!(
            aller_retour(&image),
            format!("screen layout fond image {CHEMIN_FOND}")
        );

        let aucune = requete_de_composition(&ChoixDeComposition::Aucune);
        assert_eq!(
            aucune,
            Request::Screen(ScreenAction::LayoutOff),
            "sortir d'une composition demande « screen layout off »"
        );
        assert_eq!(aller_retour(&aucune), "screen layout off");

        let etat = requete_de_composition(&ChoixDeComposition::Etat);
        assert_eq!(
            etat,
            Request::Screen(ScreenAction::LayoutState),
            "lire la composition courante demande « screen layout », qui ne change rien"
        );
        assert_eq!(aller_retour(&etat), "screen layout");
    }
}
