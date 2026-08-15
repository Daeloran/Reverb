//! Tests d'intention des zones — issue #29, « une zone = une couche ».
//!
//! Écrits **avant** l'implémentation, depuis l'issue #29 et le contrat public de
//! `crates/reverb-daemon/src/zones.rs` — signatures et documentation, tous les corps `todo!("issue
//! #29")` à l'écriture de ce fichier. Si l'un de ces tests échoue après implémentation, c'est le
//! code qu'on corrige, jamais le test.
//!
//! ## Le défaut visé : deux états qui s'excluent
//!
//! Jusqu'ici le démon n'avait qu'un état — une couleur par cible **ou** une animation. Toute
//! peinture tuait l'animation, toute animation écrasait les peintures. Les zones réconcilient les
//! deux, et la contrainte qui rend le modèle tenable tient en une phrase : **une LED appartient à
//! au plus une zone**. C'est elle qui évite d'avoir à trancher un ordre d'empilement, et c'est donc
//! elle que ce fichier surveille le plus — l'invariant est vérifié après chaque manipulation, pas
//! seulement là où on s'attend à le voir céder.
//!
//! ## Ce qui, cassé, ne produirait aucun message
//!
//! Tout, ou presque. Une zone est un objet visuel : une LED laissée dans deux zones, une zone vide
//! conservée, un rendu perdu à la redéfinition, une animation calculée sur la seule zone au lieu du
//! boîtier — aucune de ces fautes ne lève d'erreur. Elles produisent un boîtier qui n'affiche pas
//! ce qu'on a demandé, sous un bureau, et personne ne les relie jamais à une ligne de code. D'où la
//! forme de ces tests : ils ne cherchent pas une erreur, ils comparent une image à celle qu'on
//! avait décrite, LED par LED.
//!
//! ## Quatre points que le contrat laisse ouverts, et que ces tests tranchent
//!
//! 1. **Une zone animée montre exactement ce que l'animation calcule sur la géométrie entière.**
//!    Le contrat dit « calcule son image **sur la géométrie entière** et n'en prend que ses propres
//!    LED » ; ces tests le rendent vérifiable en comparant à `Animation::image(&geometrie,
//!    &reglages, pas)`, LED par LED. C'est la seule lecture possible de la phrase, et c'est celle
//!    qui garde deux zones voisines cohérentes entre elles.
//! 2. **Une zone animée n'a pas de compteur à elle.** `composer` reçoit un `pas` unique et un
//!    `&self` immuable : la « propre phase » d'une zone ne peut venir que de ses réglages, pas d'un
//!    pas décalé. Deux zones à vitesses différentes divergent donc parce que l'animation les lit
//!    différemment au même instant, pas parce qu'on leur compte le temps séparément.
//! 3. **L'ordre de `Zones::liste` est celui de la création, et une redéfinition ne le change
//!    pas.** Le contrat écrit « dans l'ordre de leur création » et « une zone qui existait garde
//!    son rendu » : c'est la même zone, elle garde donc aussi sa place. Sans cela, redéfinir une
//!    zone la ferait sauter en fin de liste dans la fenêtre, à chaque clic.
//! 4. **Un signalement de chargement nomme le fichier**, comme celui de l'éclairage (#21). Le
//!    contrat exige un message sans en fixer le contenu ; un journal qui dit « zones invalides » et
//!    rien d'autre laisse l'utilisateur devant un boîtier qui a perdu ses couches sans savoir
//!    laquelle des deux configurations relire.
//!
//! ## Ce que ces tests ne couvrent pas, et pourquoi
//!
//! - « `watch` continue de pousser une image par tour, composée » et « peindre une LED ne coupe
//!   plus l'animation globale : la LED entre dans une zone » : les deux parlent de la boucle de
//!   rendu et du socket, qui vivent dans `main.rs` et `serveur.rs`. Ce qui en est pur — la
//!   composition elle-même — est vérifié ici ; le reste se constate sur la machine.
//! - Le redémarrage machine : ce qui en est testable est l'aller-retour par le disque, qui en est
//!   le seul mécanisme.

use std::fs;
use std::path::{Path, PathBuf};

use reverb_anim::{Animation, Direction, Geometrie, Image, Reglages};
use reverb_daemon::persistance::{CHEMIN_ECLAIRAGE, CHEMIN_GEOMETRIE};
use reverb_daemon::zones::{
    CHEMIN_ZONES, Rendu, Tampon, Zones, ZonesInvalides, charger, enregistrer,
};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Led, Position, Rgb};

// ---------------------------------------------------------------------------
// Vecteurs témoins
// ---------------------------------------------------------------------------

/// Le nombre de LED du boîtier : dix anneaux de huit, quatre barrettes de onze.
///
/// Recalculé depuis le matériel plutôt qu'écrit `124` : si une barrette s'ajoute un jour, c'est le
/// test qui suit le boîtier.
const LEDS_DU_BOITIER: usize = 10 * LEDS_PER_FAN as usize + SLOT_COUNT * LEDS_PER_STICK;

/// La couleur d'une zone fixe. Son rouge à `0xff` la met **hors de portée** de [`teinte`], dont le
/// rouge ne dépasse pas `0x8b` : aucune LED de fond ne peut se confondre avec elle par hasard.
const ROUGE_DE_ZONE: Rgb = Rgb::new(0xff, 0x20, 0x80);

/// La couleur d'une seconde zone fixe, distincte de [`ROUGE_DE_ZONE`] sur ses trois composantes et
/// hors de portée de [`teinte`] elle aussi.
const VERT_DE_ZONE: Rgb = Rgb::new(0xf0, 0xff, 0x0c);

/// La couleur témoin de la LED de rang `rang` dans la couche globale.
///
/// Trois propriétés, chacune pour un mode de défaillance précis :
/// - **toutes distinctes** entre elles, sinon une composition qui recopierait la couleur de la
///   première LED sur les cent vingt-trois autres passerait inaperçue ;
/// - **`r` différent de `b`**, sinon une permutation de composantes traverserait sans un message ;
/// - **`r` borné à `0x8b`**, donc disjoint de [`ROUGE_DE_ZONE`] et de [`VERT_DE_ZONE`] : une LED de
///   fond ne peut pas se faire passer pour une LED de zone.
fn teinte(rang: usize) -> Rgb {
    let graine = u8::try_from(rang).expect("cent vingt-quatre LED tiennent dans un u8");
    Rgb::new(0x10 + graine, 0x40 ^ graine, 0x90u8.wrapping_sub(graine))
}

/// Un tampon où **chaque LED porte sa propre couleur**.
///
/// C'est ce qui rend une fuite visible : si une zone débordait d'une LED, la couleur qui s'y
/// trouvait était unique, et sa disparition se constate.
fn fond_temoin() -> Tampon {
    let mut fond = Tampon::noir();
    for (rang, led) in Led::toutes().into_iter().enumerate() {
        fond.poser(led, teinte(rang));
    }
    fond
}

/// Les huit LED d'un ventilateur, dans l'ordre du matériel.
fn anneau(position: Position) -> Vec<Led> {
    (0..LEDS_PER_FAN as usize)
        .map(|led| Led::Ventilateur { position, led })
        .collect()
}

/// Les onze LED d'une barrette.
fn barrette(slot: usize) -> Vec<Led> {
    (0..LEDS_PER_STICK)
        .map(|led| Led::Barrette { slot, led })
        .collect()
}

/// La zone que Nico décrit dans l'issue : « ventilateur arrière + bas-milieu + haut-milieu ».
///
/// Ni prédéfinie, ni contiguë — c'est tout l'intérêt du modèle.
fn zone_de_nico() -> Vec<Led> {
    let mut cibles = anneau(Position::Arriere);
    cibles.extend(anneau(Position::BasMilieu));
    cibles.extend(anneau(Position::HautMilieu));
    cibles
}

/// Une seconde zone, disjointe de [`zone_de_nico`], à cheval sur les deux bus.
fn zone_du_radiateur() -> Vec<Led> {
    let mut cibles = anneau(Position::RadiateurHaut);
    cibles.extend(barrette(0));
    cibles.push(Led::Barrette { slot: 3, led: 5 });
    cibles
}

fn animation(nom: &str) -> Animation {
    Animation::par_nom(nom).unwrap_or_else(|erreur| {
        panic!("« {nom} » doit figurer au catalogue des animations : {erreur:?}")
    })
}

/// Des réglages dont les trois champs diffèrent de leurs valeurs par défaut.
///
/// Si l'un coïncidait avec son défaut, un encodage qui le perdrait serait rattrapé par le défaut au
/// décodage et l'aller-retour passerait quand même.
fn reglages_temoins() -> Reglages {
    Reglages {
        couleur: Rgb::new(0x00, 0xff, 0x00),
        vitesse: 7,
        direction: Direction::HautBas,
        // Champ ajouté par #75. Aucune des animations employées ici ne suit de sonde ;
        // `vague` la refuserait même.
        sonde: None,
    }
}

/// La couleur qu'une image d'animation donne à une LED précise.
fn couleur_dans(image: &Image, cible: Led) -> Rgb {
    match cible {
        Led::Ventilateur { position, led } => {
            let (_, couleurs) = image
                .ventilateurs
                .iter()
                .find(|(p, _)| *p == position)
                .unwrap_or_else(|| panic!("l'image doit porter le ventilateur {position:?}"));
            couleurs[led]
        }
        Led::Barrette { slot, led } => image.barrettes[slot][led],
    }
}

/// Un jeu de zones couvrant les trois rendus : transparente, fixe, animée.
fn zones_temoins() -> Zones {
    let mut zones = Zones::vide();
    zones.poser("colonne", &zone_de_nico());
    assert!(zones.eclairer("colonne", ROUGE_DE_ZONE));
    zones.poser("radiateur", &zone_du_radiateur());
    // ⚠️ **`respiration` et non `braise` depuis #119.** L'aller-retour d'une zone animée doit
    // rendre les trois réglages de [`reglages_temoins`], `direction` comprise. #119 retire
    // `direction` à `braise` — elle ne suit plus aucun axe —, si bien que `reglages_ecrits` cesse
    // de l'écrire pour elle et que le témoin ne prouverait plus rien de la troisième. Le domaine du
    // test change, son verdict non : il lui faut une famille qui porte encore une direction.
    assert!(zones.animer(
        "radiateur",
        Some((animation("respiration"), reglages_temoins()))
    ));
    zones.poser(
        "libre",
        &[Led::Ventilateur {
            position: Position::HautDroite,
            led: 2,
        }],
    );
    zones
}

/// Vérifie l'invariant central du modèle : **aucune LED n'est dans deux zones**.
///
/// Appelé après chaque manipulation plutôt qu'une seule fois : c'est l'invariant qui remplace un
/// ordre d'empilement, et le jour où il cède, la couleur affichée dépend de l'ordre d'itération —
/// donc du hasard.
fn aucune_led_dans_deux_zones(zones: &Zones) {
    let mut vues: Vec<(Led, String)> = Vec::new();
    for zone in zones.liste() {
        for cible in &zone.cibles {
            if let Some((_, autre)) = vues.iter().find(|(led, _)| led == cible) {
                panic!(
                    "{cible:?} se trouve à la fois dans « {} » et dans « {autre} » : une LED \
                     appartient à au plus une zone, sans quoi ce qu'elle affiche dépend de l'ordre \
                     d'itération",
                    zone.nom
                );
            }
            vues.push((*cible, zone.nom.clone()));
        }
    }
}

/// Vérifie qu'aucune zone n'est vide : une zone sans LED n'affiche rien et ne se désigne plus.
fn aucune_zone_vide(zones: &Zones) {
    for zone in zones.liste() {
        assert!(
            !zone.cibles.is_empty(),
            "la zone « {} » n'a plus aucune LED : elle devait être supprimée",
            zone.nom
        );
    }
}

/// Les cibles d'une zone, ou un échec qui dit quelles zones existent.
fn cibles_de(zones: &Zones, nom: &str) -> Vec<Led> {
    zones
        .zone(nom)
        .unwrap_or_else(|| {
            let existantes: Vec<&str> = zones.liste().iter().map(|z| z.nom.as_str()).collect();
            panic!("la zone « {nom} » doit exister ; zones présentes : {existantes:?}")
        })
        .cibles
        .clone()
}

/// Le rendu d'une zone.
fn rendu_de(zones: &Zones, nom: &str) -> Rendu {
    zones
        .zone(nom)
        .unwrap_or_else(|| panic!("la zone « {nom} » doit exister"))
        .rendu
        .clone()
}

/// Les noms des zones, dans l'ordre où `liste` les rend.
fn noms(zones: &Zones) -> Vec<String> {
    zones.liste().iter().map(|zone| zone.nom.clone()).collect()
}

/// La même liste, triée et dédoublonnée — la forme sous laquelle une zone garde ses cibles.
fn triees(cibles: &[Led]) -> Vec<Led> {
    let mut rangees = cibles.to_vec();
    rangees.sort_unstable();
    rangees.dedup();
    rangees
}

// ---------------------------------------------------------------------------
// Manipulation de texte
// ---------------------------------------------------------------------------

/// Les lignes d'un genre donné (`zone`, `light`, `anim`), repérées à leur premier mot.
fn lignes_du_genre(texte: &str, genre: &str) -> Vec<String> {
    texte
        .lines()
        .filter(|ligne| ligne.split_whitespace().next() == Some(genre))
        .map(str::to_owned)
        .collect()
}

fn avec_la_ligne(texte: &str, a_ajouter: &str) -> String {
    format!("{}\n{a_ajouter}", texte.trim_end_matches('\n'))
}

/// L'aller-retour en mémoire, avec un échec qui recopie le texte fautif.
fn aller_retour(zones: &Zones) -> Zones {
    let texte = zones.encoder();
    Zones::decoder(&texte).unwrap_or_else(|erreur| {
        panic!(
            "Des zones produites par `encoder` doivent se relire par `decoder`.\n  \
             Refus : {erreur}\n  Texte produit :\n{texte}"
        )
    })
}

fn refus(texte: &str) -> ZonesInvalides {
    match Zones::decoder(texte) {
        Ok(zones) => panic!(
            "Ce texte devait être refusé, il a été accepté et a rendu {zones:?}.\n  \
             Texte :\n{texte}"
        ),
        Err(erreur) => erreur,
    }
}

// ---------------------------------------------------------------------------
// Dossier temporaire — sans dépendance, c'est un garde-fou du projet
// ---------------------------------------------------------------------------

/// Un dossier de travail sous `std::env::temp_dir()`, effacé à la fin du test.
///
/// L'effacement passe par `Drop` et non par une ligne en fin de test : un test qui échoue part en
/// `panic!`, et une fin de test jamais atteinte laisserait le dossier derrière elle.
struct DossierJetable {
    chemin: PathBuf,
}

impl DossierJetable {
    /// `nom` doit être le nom du test : c'est ce qui rend le chemin unique entre tests, `cargo` les
    /// exécutant en parallèle dans un même processus.
    fn neuf(nom: &str) -> DossierJetable {
        let chemin =
            std::env::temp_dir().join(format!("reverb-spec-zones-{}-{nom}", std::process::id()));
        let _ = fs::remove_dir_all(&chemin);
        fs::create_dir_all(&chemin)
            .unwrap_or_else(|erreur| panic!("dossier de test {} : {erreur}", chemin.display()));
        DossierJetable { chemin }
    }

    fn fichier(&self, nom: &str) -> PathBuf {
        self.chemin.join(nom)
    }

    fn chemin(&self) -> &Path {
        &self.chemin
    }
}

impl Drop for DossierJetable {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.chemin);
    }
}

fn ecrire_fichier(chemin: &Path, contenu: &str) {
    fs::write(chemin, contenu)
        .unwrap_or_else(|erreur| panic!("écriture de {} : {erreur}", chemin.display()));
}

/// Le nom du fichier, tel qu'un message d'erreur devrait au minimum le mentionner.
fn nom_de_fichier(chemin: &Path) -> String {
    chemin
        .file_name()
        .expect("le chemin de test porte un nom de fichier")
        .to_string_lossy()
        .into_owned()
}

// ---------------------------------------------------------------------------
// 1 — une cible ne peut se trouver dans deux zones
// ---------------------------------------------------------------------------

#[test]
fn une_cible_mise_dans_une_zone_quitte_celle_qui_la_detenait() {
    // Critère d'acceptation n° 2 et test d'intention n° 3 de l'issue : « une cible mise dans une
    // zone quitte la zone qui la détenait ; aucune cible n'est dans deux zones. »
    //
    // C'est la contrainte qui remplace un ordre d'empilement. Si elle cède, la couleur d'une LED
    // dépend de l'ordre dans lequel on parcourt les zones — donc de rien de visible, et elle change
    // le jour où une zone est renommée.
    let mut zones = Zones::vide();
    zones.poser("anneau", &anneau(Position::Arriere));
    aucune_led_dans_deux_zones(&zones);

    let disputee = Led::Ventilateur {
        position: Position::Arriere,
        led: 3,
    };
    zones.poser("volee", &[disputee]);
    aucune_led_dans_deux_zones(&zones);
    aucune_zone_vide(&zones);

    assert_eq!(
        cibles_de(&zones, "volee"),
        vec![disputee],
        "la seconde affectation gagne : « volee » tient la LED disputée"
    );
    assert!(
        !cibles_de(&zones, "anneau").contains(&disputee),
        "la première perd : « anneau » ne doit plus tenir {disputee:?}"
    );
    assert_eq!(
        cibles_de(&zones, "anneau").len(),
        LEDS_PER_FAN as usize - 1,
        "et elle ne perd **que** celle-là : les sept autres LED de l'anneau restent à elle"
    );

    // Le vol traverse les deux bus et plusieurs zones à la fois : une zone qui prend une LED à
    // chacune des autres doit les retirer toutes, pas seulement à la première rencontrée.
    let mut zones = Zones::vide();
    zones.poser("a", &anneau(Position::BasGauche));
    zones.poser("b", &barrette(1));
    zones.poser("c", &anneau(Position::HautDroite));
    let butin = vec![
        Led::Ventilateur {
            position: Position::BasGauche,
            led: 0,
        },
        Led::Barrette { slot: 1, led: 10 },
        Led::Ventilateur {
            position: Position::HautDroite,
            led: 7,
        },
    ];
    zones.poser("pillarde", &butin);
    aucune_led_dans_deux_zones(&zones);

    assert_eq!(
        cibles_de(&zones, "pillarde"),
        triees(&butin),
        "la pillarde tient ses trois prises"
    );
    for (nom, reste) in [
        ("a", LEDS_PER_FAN as usize - 1),
        ("b", LEDS_PER_STICK - 1),
        ("c", LEDS_PER_FAN as usize - 1),
    ] {
        assert_eq!(
            cibles_de(&zones, nom).len(),
            reste,
            "la zone « {nom} » perd la LED volée, et une seule"
        );
    }
}

#[test]
fn les_cibles_d_une_zone_sont_triees_et_sans_doublon() {
    // Contrat — `Zone::cibles` : « triées et sans doublon : deux zones de même composition sont
    // égales, quel que soit l'ordre dans lequel on a cliqué ».
    //
    // Sans déduplication, `fan:arriere,fan:arriere:3` — une forme courte plus une de ses LED, ce
    // qu'un clic de trop produit — mettrait une LED deux fois dans la même zone. Elle serait alors
    // comptée deux fois par tout ce qui compte, et l'invariant « aucune LED dans deux zones »
    // deviendrait faux à l'intérieur d'une seule.
    let mut zones = Zones::vide();
    let mut brouillon = anneau(Position::Arriere);
    brouillon.reverse();
    brouillon.push(Led::Ventilateur {
        position: Position::Arriere,
        led: 3,
    });
    brouillon.insert(0, Led::Barrette { slot: 2, led: 4 });
    brouillon.push(Led::Barrette { slot: 2, led: 4 });

    zones.poser("brouillonne", &brouillon);
    let rangees = cibles_de(&zones, "brouillonne");
    assert_eq!(
        rangees,
        triees(&brouillon),
        "les cibles doivent être triées et dédoublonnées : ce qu'on a cliqué deux fois ne compte \
         qu'une"
    );
    assert_eq!(
        rangees.len(),
        LEDS_PER_FAN as usize + 1,
        "huit LED d'anneau plus une de barrette, les répétitions retirées"
    );

    // Deux ordres de clic, une seule zone : c'est ce que le contrat appelle « deux zones de même
    // composition sont égales ».
    let mut autre = Zones::vide();
    autre.poser("brouillonne", &triees(&brouillon));
    assert_eq!(
        autre, zones,
        "l'ordre des clics ne doit pas produire deux zones différentes"
    );
}

// ---------------------------------------------------------------------------
// 2 — une zone qui perd toutes ses LED est supprimée
// ---------------------------------------------------------------------------

#[test]
fn une_zone_qui_perd_toutes_ses_led_est_supprimee() {
    // Contrat — `Zones::poser` : « une zone qui se retrouve sans aucune LED est supprimée : une
    // zone vide n'affiche rien et ne se désigne plus ».
    //
    // La garder serait pire qu'inutile : elle paraîtrait dans `zone list` et dans la fenêtre, on
    // pourrait lui donner une couleur, et rien ne s'allumerait. Un réglage sans effet qu'aucun
    // message n'explique.
    let mut zones = Zones::vide();
    let seule = Led::Barrette { slot: 2, led: 4 };
    zones.poser("fragile", &[seule]);
    assert!(zones.eclairer("fragile", ROUGE_DE_ZONE));
    assert_eq!(noms(&zones), vec!["fragile".to_owned()]);

    zones.poser("voleuse", &[seule]);
    aucune_led_dans_deux_zones(&zones);
    aucune_zone_vide(&zones);
    assert!(
        zones.zone("fragile").is_none(),
        "« fragile » n'a plus de LED : elle ne doit plus exister, ni se désigner"
    );
    assert_eq!(
        noms(&zones),
        vec!["voleuse".to_owned()],
        "et elle ne doit pas rester dans la liste en tant que coquille vide"
    );
    assert!(
        !zones.eclairer("fragile", VERT_DE_ZONE),
        "une zone supprimée ne se désigne plus : lui donner une couleur doit échouer"
    );

    // Le dépeçage complet d'une zone à plusieurs LED, pris LED par LED : elle disparaît à la
    // dernière, pas avant.
    let mut zones = Zones::vide();
    zones.poser("proie", &anneau(Position::BasDroite));
    for (rang, led) in anneau(Position::BasDroite).into_iter().enumerate() {
        zones.poser(&format!("morceau-{rang}"), &[led]);
        aucune_led_dans_deux_zones(&zones);
        aucune_zone_vide(&zones);
        let reste = LEDS_PER_FAN as usize - 1 - rang;
        if reste == 0 {
            assert!(
                zones.zone("proie").is_none(),
                "à la huitième LED volée, « proie » est vide : elle doit disparaître"
            );
        } else {
            assert_eq!(
                cibles_de(&zones, "proie").len(),
                reste,
                "après {} LED volées, « proie » en garde {reste}",
                rang + 1
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — une zone redéfinie garde son rendu, une zone nouvelle naît transparente
// ---------------------------------------------------------------------------

#[test]
fn une_zone_nouvelle_nait_transparente_et_une_zone_redefinie_garde_son_rendu() {
    // Contrat — `Zones::poser` : « une zone qui existait garde son rendu ; une zone nouvelle naît
    // transparente ». Critère d'acceptation : « renommer ou redéfinir une zone ne change pas ce que
    // les autres affichent ».
    //
    // Redéfinir, c'est ce qu'on fait en ajoutant un ventilateur à une sélection déjà peinte. Perdre
    // le rendu à cet instant fait retomber la zone sur la couche globale au moment précis où
    // l'utilisateur croit l'agrandir : le rouge qu'il avait choisi disparaît sous l'arc-en-ciel, et
    // il n'a aucune raison de relier les deux.
    let mut zones = Zones::vide();
    zones.poser("colonne", &anneau(Position::Arriere));
    assert_eq!(
        rendu_de(&zones, "colonne"),
        Rendu::Transparente,
        "une zone nouvelle n'affiche rien tant qu'on ne lui a rien donné : ses LED suivent la \
         couche globale"
    );

    assert!(zones.eclairer("colonne", ROUGE_DE_ZONE));
    assert_eq!(rendu_de(&zones, "colonne"), Rendu::Fixe(ROUGE_DE_ZONE));

    let agrandie = zone_de_nico();
    zones.poser("colonne", &agrandie);
    assert_eq!(
        rendu_de(&zones, "colonne"),
        Rendu::Fixe(ROUGE_DE_ZONE),
        "la zone existait : elle garde le rouge qu'on lui avait donné"
    );
    assert_eq!(
        cibles_de(&zones, "colonne"),
        triees(&agrandie),
        "et prend exactement sa nouvelle composition, pas l'union avec l'ancienne"
    );

    // Une zone animée garde son animation **et** ses réglages : reprendre la vitesse par défaut
    // serait un réglage perdu que rien ne signalerait.
    assert!(zones.animer("colonne", Some((animation("comete"), reglages_temoins()))));
    zones.poser("colonne", &anneau(Position::HautGauche));
    assert_eq!(
        rendu_de(&zones, "colonne"),
        Rendu::Animee(animation("comete"), reglages_temoins()),
        "l'animation et ses réglages survivent à la redéfinition"
    );

    // Redéfinir une zone ne touche pas aux autres — ni leur composition, ni leur rendu, ni leur
    // place dans la liste.
    let mut zones = zones_temoins();
    let avant_radiateur = cibles_de(&zones, "radiateur");
    let avant_rendu = rendu_de(&zones, "radiateur");
    let avant_ordre = noms(&zones);
    zones.poser("colonne", &anneau(Position::BasGauche));
    aucune_led_dans_deux_zones(&zones);

    assert_eq!(
        cibles_de(&zones, "radiateur"),
        avant_radiateur,
        "redéfinir « colonne » ne doit rien retirer à « radiateur » : les deux sont disjointes"
    );
    assert_eq!(
        rendu_de(&zones, "radiateur"),
        avant_rendu,
        "ni changer ce qu'elle affiche"
    );
    assert_eq!(
        noms(&zones),
        avant_ordre,
        "ni la faire sauter en fin de liste : l'ordre est celui de la création, et une \
         redéfinition n'est pas une création"
    );
}

#[test]
fn les_zones_sont_rendues_dans_l_ordre_de_leur_creation() {
    // Contrat — `Zones` : « toutes les zones, dans l'ordre de leur création ». Un ordre qui
    // dépendrait d'une table de hachage ferait danser la liste de la fenêtre à chaque commande,
    // sans qu'aucune ligne de code n'ait l'air fautive.
    let mut zones = Zones::vide();
    for (rang, position) in Position::ALL.into_iter().enumerate() {
        zones.poser(&format!("z{rang}"), &anneau(position));
    }
    let attendu: Vec<String> = (0..Position::ALL.len()).map(|r| format!("z{r}")).collect();
    assert_eq!(noms(&zones), attendu, "l'ordre est celui de la création");

    // Supprimer une zone du milieu ne réordonne pas les autres.
    assert!(zones.retirer("z4"));
    let attendu: Vec<String> = attendu.into_iter().filter(|nom| nom != "z4").collect();
    assert_eq!(
        noms(&zones),
        attendu,
        "supprimer une zone retire sa ligne, elle ne mélange pas les autres"
    );
}

// ---------------------------------------------------------------------------
// 4 — une zone inconnue se refuse, elle ne se crée pas
// ---------------------------------------------------------------------------

#[test]
fn retirer_eclairer_et_animer_rendent_faux_sur_une_zone_inconnue() {
    // Test d'intention n° 9 de l'issue : « un nom de zone inconnu est refusé en le nommant ».
    // Contrat — les trois méthodes rendent « faux si elle n'existe pas ».
    //
    // Le danger n'est pas le faux rendu, c'est la **création silencieuse** : `zone light colone
    // ff0000`, une lettre de moins, créerait une zone vide portant du rouge. Rien ne s'allumerait,
    // et `zone list` montrerait deux zones là où l'utilisateur en a fait une.
    let mut zones = zones_temoins();
    let avant = zones.clone();

    for nom in ["inconnue", "", "colone", "COLONNE", "colonne "] {
        assert!(
            !zones.retirer(nom),
            "supprimer « {nom} » doit échouer : cette zone n'existe pas"
        );
        assert!(
            !zones.eclairer(nom, VERT_DE_ZONE),
            "éclairer « {nom} » doit échouer plutôt que de créer une zone vide"
        );
        assert!(
            !zones.animer(nom, Some((animation("vague"), Reglages::default()))),
            "animer « {nom} » doit échouer plutôt que de créer une zone vide"
        );
        assert!(
            !zones.animer(nom, None),
            "rendre « {nom} » transparente doit échouer aussi"
        );
    }

    assert_eq!(
        zones, avant,
        "un refus ne laisse aucune trace : ni zone créée, ni rendu changé"
    );

    // Sur une zone qui existe, les trois réussissent — sans quoi les refus ci-dessus ne
    // prouveraient rien.
    assert!(zones.eclairer("colonne", VERT_DE_ZONE));
    assert_eq!(rendu_de(&zones, "colonne"), Rendu::Fixe(VERT_DE_ZONE));
    assert!(zones.animer("colonne", Some((animation("vague"), Reglages::default()))));
    assert_eq!(
        rendu_de(&zones, "colonne"),
        Rendu::Animee(animation("vague"), Reglages::default())
    );
    assert!(zones.animer("colonne", None));
    assert_eq!(
        rendu_de(&zones, "colonne"),
        Rendu::Transparente,
        "`animer(None)` rend la zone transparente : ses LED reprennent la couche globale"
    );
    assert!(zones.retirer("colonne"));
    assert!(zones.zone("colonne").is_none());
    assert!(
        !zones.retirer("colonne"),
        "et une seconde suppression échoue : elle n'existe plus"
    );
}

// ---------------------------------------------------------------------------
// 5 — une zone à couleur fixe résiste à une animation globale
// ---------------------------------------------------------------------------

#[test]
fn une_zone_a_couleur_fixe_resiste_a_une_animation_globale() {
    // Critère d'acceptation n° 3 et test d'intention n° 1 de l'issue : « une zone à couleur fixe
    // garde sa couleur pendant qu'une animation globale tourne ».
    //
    // Le scénario de l'issue, mot pour mot : « une animation globale tourne. On sélectionne trois
    // ventilateurs, on en fait une zone, on lui donne du rouge fixe : ces trois-là passent au rouge
    // et **y restent**, le reste continue de tourner. »
    //
    // Le fond témoin tient lieu d'animation globale : chaque LED y porte une couleur unique, donc
    // toute fuite de la zone efface une couleur qui n'existe qu'à cet endroit, et se constate.
    let mut zones = Zones::vide();
    let cibles = zone_de_nico();
    zones.poser("colonne", &cibles);
    assert!(zones.eclairer("colonne", ROUGE_DE_ZONE));

    let geometrie = Geometrie::mesuree();
    // Quatre instants espacés le long d'un cycle de cent vingt pas : une zone fixe qui « bougerait »
    // au pas 0 seulement passerait un test à un seul instant.
    for pas in [0, 1, 37, 119, 1_000] {
        let avant = fond_temoin();
        let mut apres = avant.clone();
        zones.composer(&geometrie, pas, &mut apres);

        for led in Led::toutes() {
            if cibles.contains(&led) {
                assert_eq!(
                    apres.couleur(led),
                    ROUGE_DE_ZONE,
                    "pas {pas} : {led:?} est dans la zone fixe, elle doit porter sa couleur — pas \
                     celle que la couche globale y avait mise"
                );
            } else {
                assert_eq!(
                    apres.couleur(led),
                    avant.couleur(led),
                    "pas {pas} : {led:?} n'est dans aucune zone, elle doit garder ce que la couche \
                     globale y a mis. Une zone qui déborde d'une LED efface une couleur unique."
                );
            }
        }
    }
}

#[test]
fn une_zone_transparente_ne_touche_a_rien() {
    // Test d'intention n° 7 de l'issue : « une zone vide ne change rien à l'image », et contrat —
    // `Zones::composer` : « une zone transparente ne touche à rien ». Critère d'acceptation : « une
    // zone dont on retire le rendu redevient transparente : ses LED reprennent la couche globale. »
    //
    // C'est ce qui rend `zone anim <nom> off` réversible : la zone existe toujours, elle est
    // toujours sélectionnable, et elle n'affiche rien de plus que le boîtier.
    let mut zones = Zones::vide();
    zones.poser("colonne", &zone_de_nico());
    zones.poser("radiateur", &zone_du_radiateur());

    let geometrie = Geometrie::mesuree();
    let attendu = fond_temoin();
    let mut compose = attendu.clone();
    zones.composer(&geometrie, 42, &mut compose);
    assert_eq!(
        compose, attendu,
        "deux zones transparentes ne doivent rien changer à l'image : la couche globale passe \
         intacte"
    );

    // Et l'on y revient : une zone peinte puis rendue transparente redonne ses LED à la couche
    // globale, **sans les éteindre**. Les remettre à noir serait le défaut le plus visible et le
    // plus décourageant — trois ventilateurs éteints au lieu de trois ventilateurs rendus.
    assert!(zones.eclairer("colonne", ROUGE_DE_ZONE));
    assert!(zones.animer("colonne", None));
    let mut compose = attendu.clone();
    zones.composer(&geometrie, 42, &mut compose);
    assert_eq!(
        compose, attendu,
        "une zone dont on retire le rendu redevient transparente, elle ne s'éteint pas"
    );

    // Supprimer la zone donne le même résultat, par un autre chemin : ses LED reviennent à la
    // couche globale sans s'éteindre (critère d'acceptation n° 5).
    assert!(zones.eclairer("radiateur", VERT_DE_ZONE));
    assert!(zones.retirer("radiateur"));
    let mut compose = attendu.clone();
    zones.composer(&geometrie, 42, &mut compose);
    assert_eq!(
        compose, attendu,
        "supprimer une zone rend ses LED à la couche globale, sans les éteindre"
    );
}

// ---------------------------------------------------------------------------
// 6 — deux zones animées avancent chacune à sa vitesse
// ---------------------------------------------------------------------------

#[test]
fn une_zone_animee_montre_ce_que_l_animation_calcule_sur_le_boitier_entier() {
    // Contrat — `Zones::composer` : « une zone animée calcule son image **sur la géométrie
    // entière** et n'en prend que ses propres LED », et l'issue : « une vague donnée à la colonne
    // du radiateur traverse le **boîtier**, et la zone n'en montre que sa part. C'est ce qui garde
    // deux zones voisines cohérentes entre elles. »
    //
    // La faute que ce test attrape est celle qui semble la plus naturelle à écrire : recalculer
    // l'animation sur la seule zone, comme si elle était un boîtier miniature. Le résultat est
    // plausible — ça bouge, c'est coloré — et faux : deux zones voisines cesseraient d'être en
    // phase, et une vague repartirait de zéro à chaque frontière.
    let mut zones = Zones::vide();
    let cibles = zone_du_radiateur();
    zones.poser("radiateur", &cibles);
    let anim = animation("vague");
    let reglages = reglages_temoins();
    assert!(zones.animer("radiateur", Some((anim, reglages.clone()))));

    let geometrie = Geometrie::mesuree();
    for pas in [0, 5, 60, 137] {
        let avant = fond_temoin();
        let mut apres = avant.clone();
        zones.composer(&geometrie, pas, &mut apres);

        let reference = anim.image(&geometrie, &reglages, pas);
        for led in Led::toutes() {
            if cibles.contains(&led) {
                assert_eq!(
                    apres.couleur(led),
                    couleur_dans(&reference, led),
                    "pas {pas} : {led:?} doit porter la couleur que « vague » lui donne **sur la \
                     géométrie entière**, pas celle d'une vague recalculée sur la seule zone"
                );
            } else {
                assert_eq!(
                    apres.couleur(led),
                    avant.couleur(led),
                    "pas {pas} : {led:?} n'est dans aucune zone, la couche globale doit y rester"
                );
            }
        }
    }
}

#[test]
fn deux_zones_animees_differemment_avancent_chacune_a_sa_vitesse() {
    // Critère d'acceptation n° 4 et test d'intention n° 2 de l'issue : « une zone animée tourne à
    // sa propre vitesse, indépendamment de la couche globale », « deux zones animées différemment
    // avancent chacune à sa vitesse ».
    //
    // `composer` reçoit **un seul** `pas` et un `&self` immuable : la « propre phase » d'une zone ne
    // peut donc venir que de ses réglages. La faute visée est celle d'un code qui calculerait une
    // image une fois — avec les réglages de la première zone, ou avec ceux de la couche globale —
    // et la distribuerait à toutes : les deux zones battraient alors du même pas, ce qui est
    // exactement ce que l'issue refuse.
    let anim = animation("vague");
    let lente = Reglages {
        vitesse: 1,
        ..reglages_temoins()
    };
    let rapide = Reglages {
        vitesse: 10,
        ..reglages_temoins()
    };
    assert_ne!(
        lente.vitesse, rapide.vitesse,
        "les deux zones doivent différer par leur vitesse, sinon ce test ne prouve rien"
    );

    let lentes = zone_de_nico();
    let rapides = zone_du_radiateur();
    let mut zones = Zones::vide();
    zones.poser("lente", &lentes);
    zones.poser("rapide", &rapides);
    assert!(zones.animer("lente", Some((anim, lente.clone()))));
    assert!(zones.animer("rapide", Some((anim, rapide.clone()))));
    aucune_led_dans_deux_zones(&zones);

    let geometrie = Geometrie::mesuree();

    // Un pas où les deux réglages ne donnent **pas** la même image, sur une LED de chaque zone.
    // Sans lui, une implémentation qui ignore la vitesse passerait le test : au pas 0, toutes les
    // vitesses coïncident.
    let temoin_lent = lentes[0];
    let temoin_rapide = rapides[0];
    let divergent = (1..600u32)
        .find(|pas| {
            let a = anim.image(&geometrie, &lente, *pas);
            let b = anim.image(&geometrie, &rapide, *pas);
            couleur_dans(&a, temoin_lent) != couleur_dans(&b, temoin_lent)
                && couleur_dans(&a, temoin_rapide) != couleur_dans(&b, temoin_rapide)
        })
        .expect(
            "deux vitesses extrêmes doivent finir par diverger sur les deux zones témoins, sinon \
             la vitesse ne règle rien",
        );

    for pas in [divergent, divergent + 1, divergent * 2] {
        let avant = fond_temoin();
        let mut apres = avant.clone();
        zones.composer(&geometrie, pas, &mut apres);

        let image_lente = anim.image(&geometrie, &lente, pas);
        let image_rapide = anim.image(&geometrie, &rapide, pas);

        for led in &lentes {
            assert_eq!(
                apres.couleur(*led),
                couleur_dans(&image_lente, *led),
                "pas {pas} : {led:?} appartient à la zone lente, elle doit suivre **ses** réglages"
            );
        }
        for led in &rapides {
            assert_eq!(
                apres.couleur(*led),
                couleur_dans(&image_rapide, *led),
                "pas {pas} : {led:?} appartient à la zone rapide, elle doit suivre **ses** réglages"
            );
        }
        for led in Led::toutes() {
            if !lentes.contains(&led) && !rapides.contains(&led) {
                assert_eq!(
                    apres.couleur(led),
                    avant.couleur(led),
                    "pas {pas} : {led:?} n'est dans aucune zone, la couche globale doit y rester"
                );
            }
        }
    }

    // Et les deux zones ne battent pas du même pas : au pas choisi, l'image de l'une n'est pas
    // celle de l'autre là où on les compare. C'est ce qui rend l'égalité ci-dessus discriminante.
    let image_lente = anim.image(&geometrie, &lente, divergent);
    let image_rapide = anim.image(&geometrie, &rapide, divergent);
    assert_ne!(
        couleur_dans(&image_lente, temoin_lent),
        couleur_dans(&image_rapide, temoin_lent),
        "au pas {divergent}, les deux vitesses doivent diverger — sinon le test passerait même si \
         les deux zones partageaient les mêmes réglages"
    );
}

// ---------------------------------------------------------------------------
// 7 et 8 — la couverture complète, et le compte des LED
// ---------------------------------------------------------------------------

#[test]
fn une_zone_qui_couvre_les_124_led_masque_entierement_la_couche_globale() {
    // Test d'intention n° 8 de l'issue : « une zone qui couvre tout le boîtier masque entièrement
    // la couche globale ». Le cas limite du modèle — et celui qui attrape une composition qui
    // oublierait un bus : dix ventilateurs repeints et quatre barrettes restées sur l'ancienne
    // couche, ce qu'on ne remarque qu'en regardant sous le bureau.
    let toutes = Led::toutes();
    assert_eq!(
        toutes.len(),
        LEDS_DU_BOITIER,
        "le boîtier porte {LEDS_DU_BOITIER} LED : dix anneaux de huit et quatre barrettes de onze"
    );

    let mut zones = Zones::vide();
    zones.poser("tout", &toutes);
    assert!(zones.eclairer("tout", ROUGE_DE_ZONE));
    assert_eq!(
        cibles_de(&zones, "tout").len(),
        LEDS_DU_BOITIER,
        "la zone tient les {LEDS_DU_BOITIER} LED"
    );

    let mut compose = fond_temoin();
    zones.composer(&Geometrie::mesuree(), 7, &mut compose);
    for led in &toutes {
        assert_eq!(
            compose.couleur(*led),
            ROUGE_DE_ZONE,
            "{led:?} est couverte par la zone : rien de la couche globale ne doit y paraître"
        );
    }
}

#[test]
fn une_image_composee_porte_exactement_124_led_une_seule_fois_chacune() {
    // Test d'intention n° 10 de l'issue : « une image composée porte exactement 124 LED, une seule
    // fois chacune ».
    //
    // La faute visée est l'aliasing : deux `Led` distinctes qui pointeraient la même case du tampon
    // — un index de barrette calculé sur le rang du ventilateur, par exemple. La conséquence est
    // spectaculaire et muette : deux LED éloignées qui changent ensemble, et une troisième qui ne
    // change jamais.
    let toutes = Led::toutes();
    let mut uniques = toutes.clone();
    uniques.sort_unstable();
    uniques.dedup();
    assert_eq!(
        uniques.len(),
        LEDS_DU_BOITIER,
        "les {LEDS_DU_BOITIER} LED doivent être distinctes deux à deux"
    );

    // Deux zones de couleurs différentes, se partageant tout le boîtier : chaque LED doit porter la
    // couleur de **sa** zone. Un aliasing entre deux cases ferait apparaître une couleur du mauvais
    // côté de la frontière.
    let moitie = LEDS_DU_BOITIER / 2;
    let (premieres, secondes) = toutes.split_at(moitie);
    let mut zones = Zones::vide();
    zones.poser("gauche", premieres);
    zones.poser("droite", secondes);
    assert!(zones.eclairer("gauche", ROUGE_DE_ZONE));
    assert!(zones.eclairer("droite", VERT_DE_ZONE));
    aucune_led_dans_deux_zones(&zones);
    assert_eq!(
        cibles_de(&zones, "gauche").len() + cibles_de(&zones, "droite").len(),
        LEDS_DU_BOITIER,
        "les deux zones se partagent le boîtier sans trou ni recouvrement"
    );

    let mut compose = Tampon::noir();
    zones.composer(&Geometrie::mesuree(), 0, &mut compose);
    for led in premieres {
        assert_eq!(
            compose.couleur(*led),
            ROUGE_DE_ZONE,
            "{led:?} appartient à « gauche »"
        );
    }
    for led in secondes {
        assert_eq!(
            compose.couleur(*led),
            VERT_DE_ZONE,
            "{led:?} appartient à « droite » — une couleur de « gauche » ici serait deux LED qui \
             partagent une case"
        );
    }
}

// ---------------------------------------------------------------------------
// 10 — l'aller-retour du fichier de zones
// ---------------------------------------------------------------------------

#[test]
fn l_aller_retour_encoder_decoder_est_exact() {
    // Test d'intention n° 5 de l'issue : « l'aller-retour écriture/lecture de `zones.conf` est
    // exact ». C'est le mécanisme entier par lequel une zone survit à un redémarrage — critère
    // d'acceptation : « les zones et leurs rendus survivent à l'arrêt du démon ».
    let pose = zones_temoins();
    assert_eq!(
        aller_retour(&pose),
        pose,
        "des zones encodées puis décodées doivent rendre l'état d'origine, composition et rendus \
         compris"
    );

    // Zone par zone, pour dire ce qui cloche plutôt que « ce n'est pas égal ».
    let relu = aller_retour(&pose);
    assert_eq!(
        noms(&relu),
        noms(&pose),
        "les zones reviennent toutes, dans le même ordre"
    );
    for zone in pose.liste() {
        assert_eq!(
            cibles_de(&relu, &zone.nom),
            zone.cibles,
            "la zone « {} » doit retrouver sa composition exacte : une LED perdue en route est une \
             LED rendue à la couche globale sans que rien ne le dise",
            zone.nom
        );
        assert_eq!(
            rendu_de(&relu, &zone.nom),
            zone.rendu,
            "la zone « {} » doit retrouver son rendu",
            zone.nom
        );
    }

    // Le vide traverse aussi : un démon sans zone doit écrire un fichier qui se relit en un démon
    // sans zone, et non en une erreur.
    assert_eq!(
        aller_retour(&Zones::vide()),
        Zones::vide(),
        "aucune zone est un état comme un autre"
    );

    // Toutes les animations du catalogue traversent, et chacune avec ses propres réglages. Une
    // seule testée laisserait passer la famille qui refuse `couleur` — `arc-en-ciel` génère ses
    // teintes, on ne lui écrit donc pas de couleur, et son aller-retour doit passer quand même.
    for nom in reverb_anim::CATALOGUE {
        let anim = animation(nom);
        let mut zones = Zones::vide();
        zones.poser("z", &zone_de_nico());
        assert!(zones.animer("z", Some((anim, reglages_acceptables(anim)))));
        assert_eq!(
            aller_retour(&zones),
            zones,
            "« {nom} » et ses réglages doivent traverser l'aller-retour sans rien perdre"
        );
    }

    // Le format documenté en tête de `Zones::encoder` : une ligne `zone` par zone, suivie de son
    // rendu, et **rien** pour une zone transparente. Une ligne de rendu écrite pour une zone
    // transparente ferait repartir au démarrage suivant une couche que l'utilisateur avait éteinte.
    let texte = pose.encoder();
    assert_eq!(
        lignes_du_genre(&texte, "zone").len(),
        pose.liste().len(),
        "une ligne `zone` par zone, ni plus ni moins. Texte :\n{texte}"
    );
    assert_eq!(
        lignes_du_genre(&texte, "light").len(),
        1,
        "une seule zone est fixe dans les témoins. Texte :\n{texte}"
    );
    assert_eq!(
        lignes_du_genre(&texte, "anim").len(),
        1,
        "une seule zone est animée dans les témoins. Texte :\n{texte}"
    );

    let mut transparente = Zones::vide();
    transparente.poser("libre", &anneau(Position::Arriere));
    let texte = transparente.encoder();
    assert!(
        lignes_du_genre(&texte, "light").is_empty() && lignes_du_genre(&texte, "anim").is_empty(),
        "une zone transparente n'a pas de ligne de rendu. Texte :\n{texte}"
    );
    assert_eq!(
        aller_retour(&transparente),
        transparente,
        "et se relit transparente, sans qu'on lui invente un rendu"
    );
}

/// Des réglages construits **uniquement** avec les clés que l'animation accepte, comme le ferait
/// une commande venue du socket.
fn reglages_acceptables(anim: Animation) -> Reglages {
    let paires: Vec<(String, String)> = anim
        .parametres_acceptes()
        .iter()
        .map(|cle| {
            let valeur = match *cle {
                "couleur" => {
                    let c = reglages_temoins().couleur;
                    format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b)
                }
                "vitesse" => reglages_temoins().vitesse.to_string(),
                "direction" => reglages_temoins().direction.slug().to_owned(),
                // Étendu pour #75, comme ce message le demande. `thermique` exige le slug
                // d'une sonde ; ce fichier ne vérifie que l'aller-retour du réglage, et un
                // slug plausible y suffit — l'existence de la sonde est l'affaire du démon.
                "sonde" => "kraken2023elite:coolant-temp".to_owned(),
                autre => panic!(
                    "« {autre} » est un paramètre d'animation que ce test ne sait pas fabriquer : \
                     l'étendre plutôt que le contourner."
                ),
            };
            ((*cle).to_owned(), valeur)
        })
        .collect();

    anim.reglages(&paires).unwrap_or_else(|erreur| {
        panic!(
            "« {} » doit accepter ses propres paramètres : {erreur:?}",
            anim.nom()
        )
    })
}

// ---------------------------------------------------------------------------
// 11 — un texte abîmé est refusé, en nommant la ligne
// ---------------------------------------------------------------------------

#[test]
fn un_texte_abime_est_refuse_en_nommant_la_ligne_fautive() {
    // Contrat — `Zones::decoder` : « refuse en nommant la ligne : un premier mot inconnu, un nom de
    // zone répété, un rendu pour une zone qu'aucune ligne `zone` n'a déclarée, une LED illisible,
    // une LED présente dans deux zones ». Critère d'acceptation : « un fichier de zones abîmé est
    // refusé en nommant la ligne fautive ».
    //
    // Deviner plutôt que refuser coûte cher et en silence : garder la dernière déclaration d'un nom
    // répété donne des zones qui dépendent de l'ordre des lignes, et laisser une LED dans deux
    // zones casse l'invariant qui remplace l'ordre d'empilement.
    let valide = zones_temoins().encoder();
    let une_led = "fan:arriere:0";
    assert!(
        valide.contains(une_led),
        "les témoins doivent contenir « {une_led} », sinon le cas « LED dans deux zones » ne \
         prouve rien. Texte :\n{valide}"
    );

    let cas: Vec<(&str, String, &str)> = vec![
        (
            "un premier mot inconnu",
            "bidule colonne fan:arriere:0".to_owned(),
            "bidule",
        ),
        (
            "un nom de zone répété",
            "zone colonne slot:3:9".to_owned(),
            "colonne",
        ),
        (
            "un rendu pour une zone jamais déclarée",
            "light pas-declaree ff2080".to_owned(),
            "pas-declaree",
        ),
        (
            "une animation pour une zone jamais déclarée",
            "anim pas-declaree-non-plus braise".to_owned(),
            "pas-declaree-non-plus",
        ),
        (
            "une LED illisible",
            "zone abimee fan:arriere:99".to_owned(),
            "fan:arriere:99",
        ),
        (
            "une LED présente dans deux zones",
            format!("zone doublonne {une_led}"),
            une_led,
        ),
        // Ce cas n'est pas dans la liste du contrat, et il n'a pas à y être : `encoder` n'écrit
        // jamais de zone vide, puisque `poser` les supprime. Mais `zones.conf` est un fichier
        // texte qu'on peut éditer à la main, et l'accepter créerait exactement l'objet que tout
        // le reste du modèle interdit — une zone qui paraît dans `zone list`, qu'on peut peindre,
        // et qui n'allume rien.
        (
            "une zone sans aucune cible",
            "zone sans-cible".to_owned(),
            "sans-cible",
        ),
    ];

    let mut raisons = Vec::new();
    for (description, ajout, nomme) in cas {
        let texte = avec_la_ligne(&valide, &ajout);
        let attendue = texte.lines().count();
        let erreur = refus(&texte);

        assert_eq!(
            erreur.ligne, attendue,
            "{description} : la faute est à la dernière ligne du texte, le refus doit la pointer. \
             Refus obtenu : {erreur}\n  Texte :\n{texte}"
        );
        assert!(
            !erreur.raison.trim().is_empty(),
            "{description} : le refus doit dire ce qui cloche, pas seulement où."
        );
        assert!(
            erreur.raison.contains(nomme),
            "{description} : le refus doit nommer « {nomme} » — un fichier porte des dizaines de \
             LED et plusieurs zones, et « ligne invalide » laisse tout à relire. Raison obtenue : \
             {}",
            erreur.raison
        );
        let message = erreur.to_string();
        assert!(
            message.contains(&erreur.ligne.to_string()) && message.contains(&erreur.raison),
            "{description} : le Display dit la ligne et la raison : « {message} »"
        );
        let _: &dyn std::error::Error = &erreur;
        raisons.push(erreur.raison);
    }

    // Les refus doivent **distinguer** les fautes : un mot inconnu, un nom répété et une LED
    // illisible ne se corrigent pas de la même façon.
    raisons.sort_unstable();
    raisons.dedup();
    assert!(
        raisons.len() >= 4,
        "sept fautes de familles différentes ne peuvent pas partager une seule explication : \
         {raisons:?}"
    );
}

#[test]
fn le_numero_de_ligne_compte_a_partir_de_un() {
    // Contrat — `ZonesInvalides::ligne` : « numéro de ligne, **à partir de 1**, comme un éditeur ».
    // Un décalage d'une unité ne fait échouer aucun démarrage et envoie l'utilisateur corriger la
    // ligne d'à côté — sur un fichier de trois lignes, il corrigera la mauvaise zone.
    let valide = zones_temoins().encoder();

    let en_tete = format!("pas-une-ligne-valide\n{valide}");
    assert_eq!(
        refus(&en_tete).ligne,
        1,
        "une faute sur la première ligne du texte doit être signalée ligne 1"
    );

    let plus_bas = format!("pas-une-ligne-valide\npas-une-ligne-valide-non-plus\n{valide}");
    assert_eq!(
        refus(&plus_bas).ligne,
        1,
        "et la **première** faute est celle qu'on signale : corriger la dernière d'abord ferait \
         relire le fichier pour rien"
    );

    let en_second = {
        let mut lignes = valide.lines();
        let premiere = lignes.next().expect("les témoins produisent des lignes");
        format!(
            "{premiere}\npas-une-ligne-valide\n{}",
            lignes.collect::<Vec<_>>().join("\n")
        )
    };
    assert_eq!(
        refus(&en_second).ligne,
        2,
        "une faute sur la deuxième ligne doit être signalée ligne 2, pas 1 ni 3"
    );
}

// ---------------------------------------------------------------------------
// 12 et 13 — le disque
// ---------------------------------------------------------------------------

#[test]
fn les_zones_ne_vivent_pas_dans_le_meme_fichier_que_l_eclairage() {
    // Approche technique de l'issue : « un **second** fichier, `/var/lib/reverb/zones.conf`, à côté
    // d'`eclairage.conf`. Deux fichiers pour deux natures. » ⚠️ « Le format d'`eclairage.conf` ne
    // bouge pas — 36 tests d'intention de #21 le décrivent. »
    assert!(
        CHEMIN_ZONES.starts_with("/var/lib/"),
        "les zones sont un état de service : elles se rangent sous `/var/lib`, écrites par le \
         démon, et non sous `/etc`, réservé à ce que l'administrateur règle. Trouvé : \
         {CHEMIN_ZONES}"
    );
    assert_ne!(
        CHEMIN_ZONES, CHEMIN_ECLAIRAGE,
        "mêler les deux fichiers réécrirait le format d'`eclairage.conf`, que trente-six tests \
         d'intention de #21 décrivent"
    );
    assert_ne!(
        CHEMIN_ZONES, CHEMIN_GEOMETRIE,
        "la géométrie se mesure une fois au montage, les zones changent à chaque clic"
    );
}

#[test]
fn un_fichier_absent_rend_des_zones_vides_sans_rien_dire() {
    // Contrat — `charger` : « un fichier absent donne des zones vides : c'est le cas d'un premier
    // démarrage, et ce n'est pas une anomalie ». Un message ici polluerait le journal de toute
    // installation neuve, et l'utilisateur apprendrait à ignorer la ligne qui compte vraiment.
    let dossier = DossierJetable::neuf("fichier_absent");
    let chemin = dossier.fichier("zones.conf");

    let (zones, signalement) = charger(&chemin);
    assert_eq!(
        zones,
        Zones::vide(),
        "un fichier absent, c'est le premier démarrage : aucune zone, et le boîtier suit la couche \
         globale seule"
    );
    assert_eq!(
        signalement, None,
        "le premier démarrage n'est pas une anomalie : rien à signaler. Obtenu : {signalement:?}"
    );
    assert!(
        !chemin.exists(),
        "lire, c'est lire : charger un fichier absent ne doit pas le créer. {} a été créé",
        chemin.display()
    );
}

#[test]
fn un_fichier_abime_rend_des_zones_vides_et_un_signalement() {
    // Contrat — `charger` : « un fichier **abîmé** donne des zones vides **et** un message : le
    // démon doit démarrer sur la couche globale seule plutôt que de refuser de s'allumer ». Critère
    // d'acceptation : « le démon démarre quand même sur la couche globale seule ».
    //
    // Le message est ce qui distingue « jamais réglé » de « réglé puis abîmé ». Sans lui, un
    // utilisateur qui retrouve un boîtier sans zones n'a aucun moyen de savoir qu'il en avait.
    let dossier = DossierJetable::neuf("fichier_abime");
    let valide = zones_temoins().encoder();

    let cas: Vec<(&str, String)> = vec![
        ("du texte au hasard", "n'importe quoi\n".to_owned()),
        ("un rendu sans zone", "light orpheline ff2080\n".to_owned()),
        ("une LED illisible", "zone z fan:arriere:99\n".to_owned()),
        (
            "un fichier d'une autre nature",
            "ventilateur arriere ff0000\n".to_owned(),
        ),
        // Une écriture coupée par une panne de courant tronque la **fin** du fichier. Quatre
        // caractères de moins suffisent à casser le dernier jeton, quel qu'il soit : la plus
        // courte cible du boîtier fait huit caractères (`slot:0:0`), la plus courte couleur six.
        // Une troncature à mi-fichier, elle, pourrait tomber pile sur une virgule et laisser un
        // texte parfaitement valide — ce test ne prouverait alors rien.
        ("coupé par une panne de courant", {
            let mut restant: Vec<char> = valide.chars().collect();
            restant.truncate(restant.len().saturating_sub(4));
            restant.into_iter().collect()
        }),
        ("une seule accolade", "{".to_owned()),
    ];

    for (description, contenu) in cas {
        let chemin = dossier.fichier("zones.conf");
        ecrire_fichier(&chemin, &contenu);
        let (zones, signalement) = charger(&chemin);
        assert_eq!(
            zones,
            Zones::vide(),
            "fichier « {description} » : le démon démarre sur la couche globale seule, il ne \
             s'arrête pas et n'invente pas de zone"
        );
        let message = signalement.unwrap_or_else(|| {
            panic!(
                "fichier « {description} » : un fichier présent mais inexploitable doit être \
                 signalé, sinon il se confond avec un premier démarrage et l'utilisateur ne saura \
                 jamais qu'il a perdu ses couches"
            )
        });
        assert!(
            message.contains(&nom_de_fichier(&chemin)),
            "fichier « {description} » : le signalement doit nommer le fichier en cause. Obtenu : \
             {message}"
        );
    }

    // Illisible au niveau système, avant même qu'il y ait du texte à analyser : un dossier là où le
    // démon attend un fichier, et des octets qui ne sont pas de l'UTF-8 — ce qu'une écriture
    // coupée par une panne de courant peut laisser.
    let en_dossier = dossier.fichier("dossier.conf");
    fs::create_dir(&en_dossier).expect("création du faux fichier");
    let binaire = dossier.fichier("binaire.conf");
    fs::write(&binaire, [0xff_u8, 0xfe, 0x00, 0x80]).expect("écriture d'octets invalides");

    for chemin in [&en_dossier, &binaire] {
        let (zones, signalement) = charger(chemin);
        assert_eq!(
            zones,
            Zones::vide(),
            "un fichier illisible ne doit pas empêcher le démon de démarrer. Chemin : {}",
            chemin.display()
        );
        assert!(
            signalement.is_some(),
            "un fichier illisible doit être signalé. Chemin : {}",
            chemin.display()
        );
    }

    // Un fichier **vide** n'est pas abîmé : c'est l'état d'un démon dont on vient de supprimer la
    // dernière zone, et `enregistrer` l'écrit tel quel. Le signaler ferait apparaître un
    // avertissement à chaque démarrage de qui n'utilise pas les zones.
    let vide = dossier.fichier("vide.conf");
    ecrire_fichier(&vide, &Zones::vide().encoder());
    let (zones, signalement) = charger(&vide);
    assert_eq!(zones, Zones::vide());
    assert_eq!(
        signalement, None,
        "un fichier sans zone est parfaitement valide : rien à signaler. Obtenu : {signalement:?}"
    );
}

#[test]
fn les_zones_et_leurs_rendus_survivent_a_un_redemarrage() {
    // Critère d'acceptation : « les zones et leurs rendus survivent à l'arrêt du démon et au
    // redémarrage de la machine ». Ce qui en est testable ici est l'aller-retour par le disque, qui
    // en est le seul mécanisme.
    let dossier = DossierJetable::neuf("redemarrage");
    let chemin = dossier.fichier("zones.conf");
    let pose = zones_temoins();

    enregistrer(&chemin, &pose).expect("l'enregistrement doit réussir");
    let (relu, signalement) = charger(&chemin);

    assert_eq!(
        relu, pose,
        "les zones posées doivent être retrouvées telles quelles au démarrage suivant, \
         composition et rendus compris"
    );
    assert_eq!(
        signalement, None,
        "un fichier que le démon vient d'écrire doit se relire sans un mot. Obtenu : {signalement:?}"
    );

    // Le second enregistrement efface le premier — il ne le complète pas. Supprimer sa dernière
    // zone puis redémarrer ne doit pas la faire revenir.
    let mut sans_zone = pose.clone();
    for nom in noms(&pose) {
        assert!(sans_zone.retirer(&nom));
    }
    enregistrer(&chemin, &sans_zone).expect("second enregistrement");
    let (relu, _) = charger(&chemin);
    assert_eq!(
        relu,
        Zones::vide(),
        "on retrouve l'absence de zone, on ne retrouve pas les zones d'avant"
    );
}

#[test]
fn aucun_fichier_temporaire_ne_traine_apres_l_enregistrement() {
    // Contrat — `enregistrer` : « par fichier temporaire puis renommage ». C'est ce qui protège
    // l'état d'une coupure de courant, et l'écriture est fréquente puisqu'elle a lieu à chaque
    // commande de zone. Le provisoire ne doit pas survivre : ce serait un fichier de plus dans
    // `/var/lib/reverb` à chaque clic, et un candidat à la relecture.
    let dossier = DossierJetable::neuf("sans_fichier_temporaire");
    let chemin = dossier.fichier("zones.conf");

    enregistrer(&chemin, &zones_temoins()).expect("premier enregistrement");
    enregistrer(&chemin, &Zones::vide()).expect("second enregistrement");

    let mut restants: Vec<String> = fs::read_dir(dossier.chemin())
        .expect("lecture du dossier de test")
        .map(|entree| {
            entree
                .expect("entrée de dossier lisible")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    restants.sort();

    assert_eq!(
        restants,
        vec![nom_de_fichier(&chemin)],
        "le dossier ne doit contenir que le fichier de zones après l'enregistrement"
    );
}
