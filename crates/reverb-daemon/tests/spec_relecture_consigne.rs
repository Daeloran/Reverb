//! Tests d'intention — issue #110, « La régulation croit une écriture qui n'a pas pris, et ne la
//! rejoue jamais ».
//!
//! Écrits **avant** l'implémentation, depuis l'issue #110 seule. Aucun fichier de `crates/*/src/`
//! n'a été lu pour les produire : ni `regulation.rs`, ni `main.rs`, ni `telemetrie.rs`. Les seules
//! signatures employées sont celles que `spec_regulation_hote.rs` (#99) fige déjà en tête de son
//! propre fichier. À l'écriture de celui-ci, `Regulation::tour` ne prend **qu'un** argument et
//! `Ecriture` n'a **que deux** champs : la compilation doit échouer, et c'est la phase rouge.
//!
//! Rien ici n'ouvre un `hwmon`, n'écrit un `pwm*`, ne dort ni ne lit l'horloge. La température est
//! **injectée**, ce que le matériel porte est **injecté**, et l'écriture est enregistrée en
//! mémoire : ce fichier vérifie une décision, pas un bus.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Constaté sur SHYNAEL le 2026-08-15, à la toute première activation de la régulation de #99. Le
//! journal annonce les trois canaux écrits à 50 % ; le matériel dit autre chose :
//!
//! ```text
//! fan-1 : 1100 tr/min  pwm=128     ← 50 %, appliqué
//! fan-2 : 1061 tr/min  pwm=128     ← 50 %, appliqué
//! fan-3 :  707 tr/min  pwm=64      ← 25 %, PAS appliqué
//! ```
//!
//! `fs::write` sur `pwmN` a rendu `Ok`, le cache de #99 a retenu **ce qu'on a voulu écrire**, la
//! consigne calculée n'a plus jamais changé — et `fan-3` est resté à son duty d'allumage pendant que
//! le démon affirmait le réguler.
//!
//! ⚠️ **Le défaut n'est pas qu'une écriture échoue, c'est qu'elle réussisse sans rien appliquer.**
//! Aucune erreur, un journal qui dit exactement ce qu'on veut lire, et sept ventilateurs sur dix qui
//! ne tournent pas à la vitesse annoncée pendant que le liquide monte. C'est le mode de défaillance
//! silencieux et rassurant que le projet refuse partout ailleurs, et c'est la leçon que
//! `docs/VENTILATEURS.md` avait déjà tirée le 2026-07-30 : « **restaurer une valeur n'est pas
//! restaurer un comportement** […] toute sonde future doit **mesurer** l'état après restauration,
//! pas le supposer ». Le cache de #99 a repris la faute par l'autre bout : il suppose l'état au lieu
//! de le mesurer.
//!
//! D'où la forme de ces tests : ils font dire au matériel simulé autre chose que ce qu'on lui a
//! écrit, et comptent les réémissions.
//!
//! **Ce que #110 spécifie tient en une phrase : la décision se prend sur ce que le canal porte, et
//! non sur ce qu'on a cru y écrire.**
//!
//! ## La signature que ces tests exigent, et pourquoi celle-là
//!
//! `regulation.rs` est **pur** — il ne touche aucun bus, il dit quoi écrire et le démon écrit. La
//! relecture que l'issue demande est donc faite **par le démon**, qui relit déjà ces canaux pour
//! servir `status`, et son résultat est **passé en argument** :
//!
//! ```ignore
//! /// Pourquoi cette écriture part.
//! #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//! pub enum Motif {
//!     /// La consigne a changé — ou le canal vient d'être pris en charge.
//!     Consigne,
//!     /// La consigne n'a pas changé, mais le canal ne la porte pas : on rejoue.
//!     NonAppliquee { porte: Option<u8> },
//! }
//!
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub struct Ecriture {
//!     pub canal: String,
//!     pub consigne: u8,
//!     pub motif: Motif,
//! }
//!
//! impl Regulation {
//!     /// `portees` : ce que chaque canal **porte réellement**, relu par le démon avant ce tour.
//!     /// Une entrée absente ou `None` = canal illisible (quarantaine #68).
//!     pub fn tour(
//!         &mut self,
//!         liquide: Option<i32>,
//!         portees: &BTreeMap<String, Option<u8>>,
//!     ) -> Vec<Ecriture>;
//! }
//! ```
//!
//! Quatre choix, et ce qu'ils achètent :
//!
//! 1. **La relecture entre par la porte, elle n'est pas faite ici.** Faire lire `regulation.rs`
//!    serait lui donner une E/S, donc un `hwmon` à ouvrir, donc un module qui ne se teste plus sans
//!    matériel — exactement ce que #99 avait acheté en rendant les écritures au lieu de les faire.
//!    Le démon relit, la régulation décide.
//! 2. **`portees` est en pourcentage, pas en duty 0–255.** Trois raisons, dans cet ordre :
//!    - `Ecriture::consigne` est un pourcentage. Comparer un pourcentage à un duty, c'est remettre
//!      deux unités dans la même API — la faute des trois ordres de composantes, « qui ne produit
//!      aucun message et juste un résultat faux » ;
//!    - le démon **a déjà** cette valeur en pourcentage : `ResponseLine::Channel` expose un `pwm` en
//!      pourcent, c'est ce que la fenêtre affiche, et c'est le relevé que la quarantaine de #68 fait
//!      passer. Redemander le duty brut serait une seconde lecture du même fichier ;
//!    - le pourcentage est la seule unité dans laquelle l'utilisateur, le journal, la fenêtre et
//!      cette régulation parlent tous. Le duty est un détail du noyau, et il vit déjà derrière
//!      `Percent`, « où la conversion n'existe qu'une fois ».
//! 3. **`Option<u8>` plutôt que `u8`, et une entrée absente vaut `None`.** La régulation ne fait
//!    rien de la différence entre un canal en quarantaine, une lecture qui échoue et un canal que le
//!    démon n'a pas listé : les trois veulent dire « je ne sais pas ce qu'il porte ». C'est déjà la
//!    forme que `Quarantaine::tour` rend.
//! 4. **`Motif` sépare une consigne neuve d'une réémission, et s'arrête là.** L'issue exige qu'une
//!    écriture non appliquée soit rejouée « au tour suivant, sans intervention » : sur un tour par
//!    seconde, un canal bloqué comme `fan-3` produit **86 400 écritures par jour**, et autant de
//!    lignes de journal si l'appelant les traite toutes pareil. C'est le chiffre même que #99
//!    invoquait pour justifier son cache, retourné contre lui — et un journal illisible au moment
//!    précis où il servirait. `tour` rend donc **pourquoi** il écrit, et le démon décide quoi
//!    imprimer : la phrase n'est pas fabriquée ici, exactement comme `TourCanaux::a_signaler` rend
//!    des noms et laisse l'appelant faire la phrase. **Aucune logique de journalisation dans
//!    `regulation.rs`.**
//!
//! ⚠️ Conséquence sur le cache de #99 : il **survit, mais démis**. Il ne décide plus s'il faut
//! écrire — c'est la relecture qui décide — il sert seulement à savoir si la consigne est neuve,
//! donc à choisir le motif. Une implémentation qui le supprimerait tout à fait ne saurait plus
//! distinguer `Consigne` de `NonAppliquee`.
//!
//! ⚠️ `porte` est un `Option<u8>` par symétrie avec `portees`, mais aucun test de ce fichier ne
//! l'observe à `None` : une réémission ne peut naître que d'une relecture réussie, puisqu'un canal
//! illisible est écarté. Un `u8` nu en ferait une garantie de type plutôt qu'une propriété tenue par
//! ailleurs — c'est un resserrement possible, laissé à l'implémentation.
//!
//! ### ⚠️ La comparaison est **exacte**, et elle peut l'être
//!
//! On aurait pu croire qu'un aller-retour pourcentage → duty → pourcentage perd un point ici ou là,
//! et qu'il faut donc tolérer un écart d'une unité. **C'est faux**, vérifié sur les 101 valeurs :
//!
//! ```text
//! raw(p)      = (p * 255 + 50) / 100      arrondi au plus proche
//! from_raw(r) = (r * 100 + 127) / 255     arrondi au plus proche
//! ```
//!
//! `from_raw(raw(p)) == p` pour tout `p` de 0 à 100, sans exception — c'est forcé : un point de
//! pourcentage vaut 2,55 pas de duty, le pas est expansif, donc aucun arrondi ne peut replier deux
//! pourcentages sur le même duty. Les deux valeurs de l'incident le montrent : `128 → 50 %` et
//! `64 → 25 %`.
//!
//! Ce que cela achète : **un écart relu est un écart réel**, jamais un artefact de conversion. Il
//! n'y a donc aucune tolérance à ajouter « au cas où l'arrondi » — une tolérance de complaisance
//! masquerait une vraie dérive sans rien corriger. La propriété d'arrondi elle-même appartient à
//! `Percent` et se teste là-bas.
//!
//! ### ⚠️ L'écart d'**un point** n'est délibérément pas spécifié ici
//!
//! Aucun test de ce fichier ne dit ce qu'il advient d'un canal qui porte 45 % pour une consigne de
//! 46 %. **C'est un choix, pas un trou**, et il appartient à l'issue **#111**, relevée sur SHYNAEL
//! le 2026-08-15, machine au repos :
//!
//! ```text
//! 15:51:10  fan-{1,2,3} à 46 %   (liquide 40.4 °C)
//! 15:51:48  fan-{1,2,3} à 45 %   (liquide 40.1 °C)
//! 15:52:28  fan-{1,2,3} à 46 %   (liquide 40.2 °C)
//! ```
//!
//! Trente écritures en huit minutes pour 0,3 °C d'amplitude. #111 posera une hystérésis, et un test
//! de #110 qui exigerait aujourd'hui « un point d'écart écrit » devrait être modifié pour la laisser
//! passer — ce que le workflow interdit. Tous les écarts employés ici sont donc **francs** : au
//! moins `ECART_FRANC` points, et `les_reperes_de_ce_fichier_ne_sont_aucun_defaut` le vérifie plutôt
//! que de l'espérer. Ce que #110 possède, c'est *sur quoi* la décision se prend ; le *seuil* au-delà
//! duquel un écart mérite une écriture est à #111.
//!
//! ## Ce que ce fichier fige
//!
//! 1. **Le cache est la mesure, plus l'intention.** On écrit si ce que le canal porte diffère
//!    franchement de la consigne calculée. La promesse « on n'écrit que ce qui change » de #99 est
//!    **tenue davantage**, pas moins : elle devient vraie du matériel et non de nos intentions.
//! 2. **Une écriture qui n'a pas pris est rejouée au tour suivant, indéfiniment**, sans intervention
//!    et sans plafond. Un plafond de tentatives recréerait le défaut qu'on corrige : un canal
//!    abandonné en silence à son duty d'allumage.
//! 3. **La décision est par canal, sur sa propre relecture.** Deux canaux régulés, l'un qui applique
//!    et l'autre non : seul le second est réémis. C'est littéralement le 2026-08-15.
//! 4. **Un canal qu'on ne sait pas relire n'est pas écrit.** Ni écriture à l'aveugle, ni repli, ni
//!    « une fois pour voir » : écrire là où on ne mesure pas, c'est refaire #110 à l'identique. Il
//!    est écarté, et les autres continuent — le tour d'un canal muet ne gèle ni ne vide celui de ses
//!    voisins.
//! 5. **Le retour d'un canal écarté ne déclenche pas d'écriture fantôme.** S'il revient en portant
//!    déjà la consigne, il ne reçoit rien : la décision est sur ce qu'il porte, pas sur le temps
//!    passé sans nouvelles.
//! 6. **Le repli traverse exactement la même règle** que la courbe, motif compris.
//! 7. **Un canal non régulé n'est jamais écrit, même s'il figure dans la relecture.** Le démon relit
//!    tout le matériel pour `status` ; `portees` contient donc les canaux du Kraken, hors scope de
//!    #99 comme de #110.
//! 8. **Une réémission se distingue d'une consigne neuve**, et porte ce que le canal portait — le
//!    `25 %` qui manquait au journal du 2026-08-15.
//!
//! ## Ce que ce fichier laisse ouvert, et qui reste à trancher à l'implémentation
//!
//! ⚠️ **Le sort du tout premier tour après `activer`, quand le canal porte déjà la consigne.** Deux
//! lectures se défendent — écrire une fois par principe, ou se taire parce qu'il n'y a rien à
//! changer — et `un_canal_coupe_puis_repris_est_reecrit` (#99) exige la première dans son scénario
//! précis. **Aucun test de ce fichier ne rejuge ce cas** : `un_canal_qui_porte_la_consigne_n_est_pas_reecrit`
//! et `un_canal_qui_porte_deja_le_repli_ne_recoit_rien_quand_la_sonde_se_tait` se placent au
//! deuxième tour à dessein, et tous les canaux qu'on active ici partent du duty d'allumage, jamais
//! de la consigne. Les deux fichiers sont donc satisfaisables ensemble par la règle « écrire si le
//! canal est lisible **et** (jamais écrit depuis son activation **ou** ce qu'il porte diffère) ».
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **« La lecture de contrôle passe par la quarantaine »** : c'est une propriété du démon, pas de
//!   ce module. Ce qui en est testable ici l'est, et c'est ce qui compte pour la régulation : un
//!   `None` — quelle qu'en soit la cause, quarantaine comprise — écarte le canal sans emporter les
//!   autres.
//! - **Le journal lui-même** : il est émis par le démon, et un module qui ne journalise pas ne peut
//!   pas être testé sur ses phrases. Ce que ce fichier garantit, c'est la matière qui les rend
//!   justes — une annonce fausse est corrigée au tour suivant, et une répétition se reconnaît à son
//!   motif au lieu d'être imprimée quatre-vingt-six mille fois.
//! - **« Un canal repris à la main (`fan … pwm`) n'est pas réécrit »** : la reprise passe par
//!   `couper`, déjà figé par #99. Ce fichier n'en garde que la moitié que #110 met en danger — la
//!   relecture ne doit pas ressusciter un canal coupé.
//! - **La cause matérielle du non-application** (piste `nzxt-smart2` et son rapport HID groupé) :
//!   hors scope, explicitement. Cette issue rend le symptôme auto-réparable, elle ne prétend pas
//!   expliquer le pilote.

use std::collections::{BTreeMap, BTreeSet};

use reverb_daemon::regulation::{Courbe, Ecriture, Motif, REPLI, Regulation};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// Les millidegrés d'un nombre entier de degrés, comme `hwmon` les rend.
const fn degres(entiers: i32) -> i32 {
    entiers * 1_000
}

/// Les trois canaux de l'incident : ceux du `nzxtsmart2`, dont le pilote n'a aucun mode automatique.
const CANAL_1: &str = "nzxtsmart2:fan-1";
const CANAL_2: &str = "nzxtsmart2:fan-2";
const CANAL_3: &str = "nzxtsmart2:fan-3";

/// Un canal du Kraken. Hors scope de #99 comme de #110 — son firmware régule déjà — mais il figure
/// dans la relecture, parce que le démon relit tout le matériel pour servir `status`.
const CANAL_KRAKEN: &str = "kraken2023elite:fan-speed";

/// L'écart minimal que ce fichier s'autorise entre une consigne témoin et ce qu'un canal porte.
///
/// ⚠️ #111 posera une hystérésis sur cette même décision — le relevé du 15:51 montre une oscillation
/// d'un point qui produit trente écritures en huit minutes. Un test de #110 bâti sur un écart d'un
/// ou deux points deviendrait faux le jour où elle sera posée, et il faudrait le modifier pour la
/// laisser passer : ce que le workflow interdit. Cinq points laissent la marge, et les écarts
/// réellement employés ici sont de huit à vingt-cinq.
const ECART_FRANC: u8 = 5;

/// Le duty d'allumage de ces canaux, en pourcentage : `pwm = 64` sur 255, que le noyau rend
/// `(64 × 100 + 127) / 255 = 25`.
///
/// C'est ce que `fan-3` a porté pendant que le démon annonçait 50 %.
const DUTY_ALLUMAGE: u8 = 25;

/// Le liquide au moment de l'incident, en millidegrés : les trois lignes de journal disent
/// « (liquide 41.7 °C) ».
const LIQUIDE_DE_L_INCIDENT: i32 = 41_700;

/// Ce que la courbe par défaut en tire : 30 + (41,7 − 35) × 3 = 50,1 %, soit 50 % en entier — et
/// `pwm = 128`, la valeur qui s'est appliquée sur `fan-1` et `fan-2`.
const CONSIGNE_DE_L_INCIDENT: u8 = 50;

/// Une température tiède, dans le premier segment de la courbe par défaut, choisie pour que
/// l'interpolation tombe juste : 30 + (36 − 35) × 3 = 33 %.
const TIEDE: i32 = degres(36);
const CONSIGNE_TIEDE: u8 = 33;

/// Une température chaude, dans le second segment : 60 + (47 − 45) × 8 = 76 %.
const CHAUD: i32 = degres(47);
const CONSIGNE_CHAUD: u8 = 76;

/// Le milieu du premier segment : 30 + (40 − 35) × 3 = 45 %.
const MEDIAN: i32 = degres(40);
const CONSIGNE_MEDIANE: u8 = 45;

// ---------------------------------------------------------------------------
// Le banc : un matériel qui n'obéit pas toujours
// ---------------------------------------------------------------------------

/// Un matériel simulé, et le registre de ce qu'on lui a écrit.
///
/// C'est tout le « matériel » de ce fichier : deux tables et un `Vec`. Sa seule particularité, et
/// c'est celle de l'incident, c'est qu'un canal peut **encaisser une écriture sans bouger**. Un banc
/// où tout s'applique toujours ne pourrait pas distinguer une régulation qui mesure d'une régulation
/// qui suppose — c'est précisément pourquoi #99 n'a pas attrapé ce défaut.
struct Banc {
    /// Ce que chaque canal porte, tel que le démon vient de le relire. `None` = illisible ; une
    /// entrée absente = le démon ne l'a pas listé du tout.
    portees: BTreeMap<String, Option<u8>>,
    /// Les canaux qui appliquent ce qu'on leur écrit. Un canal absent de cet ensemble est sourd.
    obeissants: BTreeSet<String>,
    /// Les écritures faites, dans l'ordre où les tours les ont produites.
    faites: Vec<Ecriture>,
}

impl Banc {
    fn neuf() -> Banc {
        Banc {
            portees: BTreeMap::new(),
            obeissants: BTreeSet::new(),
            faites: Vec::new(),
        }
    }

    /// Un canal qui porte `depart` et qui appliquera ce qu'on lui écrira.
    fn obeissant(mut self, canal: &str, depart: u8) -> Banc {
        self.portees.insert(canal.to_owned(), Some(depart));
        self.obeissants.insert(canal.to_owned());
        self
    }

    /// Un canal qui porte `depart` et qui **encaisse sans bouger** : c'est `fan-3` le 2026-08-15.
    fn sourd(mut self, canal: &str, depart: u8) -> Banc {
        self.portees.insert(canal.to_owned(), Some(depart));
        self.obeissants.remove(canal);
        self
    }

    /// Un canal dont la relecture échoue : le démon l'a listé, il n'en sait rien.
    fn illisible(mut self, canal: &str) -> Banc {
        self.portees.insert(canal.to_owned(), None);
        self
    }

    /// Un canal que la relecture ne mentionne pas du tout.
    fn absent(mut self, canal: &str) -> Banc {
        self.portees.remove(canal);
        self
    }

    /// Le canal se met à appliquer ce qu'on lui écrit — le blocage du 2026-08-15 a fini par céder.
    fn se_met_a_obeir(&mut self, canal: &str) {
        self.obeissants.insert(canal.to_owned());
    }

    /// Le canal cesse de répondre à la relecture, en cours de scénario.
    fn devient_illisible(&mut self, canal: &str) {
        self.portees.insert(canal.to_owned(), None);
    }

    /// Le canal se met à porter cette valeur, sans que la régulation y soit pour rien. C'est ce
    /// qu'on ne peut pas savoir sans mesurer : un `reverb fan`, un firmware qui reprend la main, ou
    /// le mécanisme inconnu de l'incident.
    fn porte_desormais(&mut self, canal: &str, valeur: u8) {
        self.portees.insert(canal.to_owned(), Some(valeur));
    }

    /// Un tour de télémétrie : la régulation lit ce que le matériel porte, dit quoi écrire, le banc
    /// le note — puis le matériel applique, ou non.
    ///
    /// Rend les écritures de **ce** tour, triées par canal : l'ordre dans lequel `tour` les rend
    /// n'est pas un contrat, et un test qui l'exigerait figerait un détail d'implémentation.
    fn tour(&mut self, regulation: &mut Regulation, liquide: Option<i32>) -> Vec<Ecriture> {
        let mut ce_tour = regulation.tour(liquide, &self.portees);
        ce_tour.sort_by(|gauche, droite| gauche.canal.cmp(&droite.canal));

        // Deux écritures sur le même canal dans le même tour, ce serait une trame de plus sur le
        // bus pour rien — et le signe qu'une couche décide deux fois.
        let mut vus: BTreeSet<&str> = BTreeSet::new();
        for ecriture in &ce_tour {
            assert!(
                vus.insert(ecriture.canal.as_str()),
                "« {} » est écrit deux fois dans le même tour : {ce_tour:?}",
                ecriture.canal
            );
        }

        self.faites.extend(ce_tour.iter().cloned());

        // Le matériel applique — ou pas. C'est ici que vit tout le sujet de #110.
        for ecriture in &ce_tour {
            if self.obeissants.contains(&ecriture.canal) {
                self.portees
                    .insert(ecriture.canal.clone(), Some(ecriture.consigne));
            }
        }

        ce_tour
    }

    /// Ce que le canal porte à cet instant, du point de vue de la relecture.
    fn porte(&self, canal: &str) -> Option<u8> {
        self.portees.get(canal).copied().flatten()
    }

    /// Toutes les écritures depuis le début, tous tours confondus.
    fn tout(&self) -> &[Ecriture] {
        &self.faites
    }

    /// Les canaux qui ont reçu au moins une écriture.
    fn canaux_touches(&self) -> BTreeSet<String> {
        self.faites
            .iter()
            .map(|ecriture| ecriture.canal.clone())
            .collect()
    }

    /// Les consignes reçues par un canal, dans l'ordre.
    fn consignes_de(&self, canal: &str) -> Vec<u8> {
        self.faites
            .iter()
            .filter(|ecriture| ecriture.canal == canal)
            .map(|ecriture| ecriture.consigne)
            .collect()
    }

    /// Les motifs des écritures reçues par un canal, dans l'ordre.
    fn motifs_de(&self, canal: &str) -> Vec<Motif> {
        self.faites
            .iter()
            .filter(|ecriture| ecriture.canal == canal)
            .map(|ecriture| ecriture.motif)
            .collect()
    }
}

/// Ce qu'un tour a écrit, motif mis de côté.
///
/// La plupart des tests de ce fichier portent sur **quoi** et **quand** ; le **pourquoi** a ses
/// propres tests, en section E. Les mêler rendrait chaque assertion trois fois plus longue pour
/// vérifier deux choses à la fois.
fn sans_motif(ecritures: &[Ecriture]) -> Vec<(String, u8)> {
    ecritures
        .iter()
        .map(|ecriture| (ecriture.canal.clone(), ecriture.consigne))
        .collect()
}

/// L'attendu d'un tour, sous la même forme que `sans_motif`.
fn ecritures(paires: &[(&str, u8)]) -> Vec<(String, u8)> {
    let mut attendues: Vec<(String, u8)> = paires
        .iter()
        .map(|(canal, consigne)| ((*canal).to_owned(), *consigne))
        .collect();
    attendues.sort();
    attendues
}

/// Une régulation sur la courbe par défaut, avec ces canaux activés.
fn regulation_sur(canaux: &[&str]) -> Regulation {
    let mut regulation = Regulation::nouvelle(Courbe::defaut());
    for canal in canaux {
        regulation.activer(canal);
    }
    regulation
}

/// Une courbe qu'on exige valide, avec un refus qui recopie les paliers fautifs.
fn courbe(paliers: &[(i32, u8)]) -> Courbe {
    match Courbe::depuis(paliers) {
        Ok(courbe) => courbe,
        Err(erreur) => panic!(
            "Ces paliers devaient être acceptés : {paliers:?}\n  Refus : {}",
            erreur.raison
        ),
    }
}

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tout ce fichier repose sur une distinction : « ce qu'on a écrit » contre « ce que le canal
    // porte ». Si le duty d'allumage coïncidait avec l'une des consignes témoins, plusieurs tests
    // deviendraient vrais sans rien vérifier — un canal sourd porterait déjà la bonne valeur, et
    // « il ne réémet pas » passerait pour la pire des raisons. Ce test est là pour que la panne soit
    // ici.
    let canaux = [
        ("CANAL_1", CANAL_1),
        ("CANAL_2", CANAL_2),
        ("CANAL_3", CANAL_3),
        ("CANAL_KRAKEN", CANAL_KRAKEN),
    ];
    for (i, (nom, canal)) in canaux.iter().enumerate() {
        assert!(!canal.is_empty(), "{nom} doit porter un nom");
        for (autre_nom, autre) in canaux.iter().skip(i + 1) {
            assert_ne!(
                canal, autre,
                "{nom} et {autre_nom} doivent être deux canaux distincts"
            );
        }
    }

    // ⚠️ L'écart doit être **franc**, et pas seulement non nul : #111 posera une hystérésis, et un
    // écart d'un point cesserait alors de produire une écriture. Voir l'en-tête du fichier.
    for (nom, consigne) in [
        ("CONSIGNE_TIEDE", CONSIGNE_TIEDE),
        ("CONSIGNE_CHAUD", CONSIGNE_CHAUD),
        ("CONSIGNE_MEDIANE", CONSIGNE_MEDIANE),
        ("CONSIGNE_DE_L_INCIDENT", CONSIGNE_DE_L_INCIDENT),
        ("REPLI", REPLI),
    ] {
        let ecart = consigne.abs_diff(DUTY_ALLUMAGE);
        assert!(
            ecart >= ECART_FRANC,
            "{nom} ({consigne} %) n'est qu'à {ecart} point(s) du duty d'allumage \
             ({DUTY_ALLUMAGE} %). Il en faut au moins {ECART_FRANC} : sous l'hystérésis que #111 \
             posera, un écart plus petit cesserait de produire une écriture et les tests de \
             réémission de ce fichier deviendraient faux"
        );
    }

    // La courbe par défaut est celle de #99, et elle rend bien les consignes annoncées ci-dessus.
    let defaut = Courbe::defaut();
    for (temperature, attendue, nom) in [
        (TIEDE, CONSIGNE_TIEDE, "TIEDE"),
        (CHAUD, CONSIGNE_CHAUD, "CHAUD"),
        (MEDIAN, CONSIGNE_MEDIANE, "MEDIAN"),
        (
            LIQUIDE_DE_L_INCIDENT,
            CONSIGNE_DE_L_INCIDENT,
            "LIQUIDE_DE_L_INCIDENT",
        ),
    ] {
        assert_eq!(
            defaut.consigne(temperature),
            attendue,
            "{nom} : la courbe par défaut doit rendre {attendue} % à {temperature} m°C"
        );
    }

    // ⚠️ Coïncidence assumée et rendue visible : la consigne de l'incident vaut exactement le repli.
    // `l_incident_du_2026_08_15_se_repare_seul` ne peut donc pas, à lui seul, distinguer la courbe
    // du repli — mais il n'a pas à le faire, il travaille sur un liquide lisible du début à la fin,
    // et cette distinction-là est celle de #99. Les autres tests emploient `TIEDE` (33 %), qui
    // diffère du repli.
    assert_eq!(
        CONSIGNE_DE_L_INCIDENT, REPLI,
        "cette égalité est celle des relevés du 2026-08-15 ; si elle cesse, relire le commentaire \
         de `l_incident_du_2026_08_15_se_repare_seul` avant de la corriger"
    );

    // Et le banc lui-même sépare bien un canal obéissant d'un canal sourd, sans quoi tout ce qui
    // suit ne vérifierait rien.
    let banc = Banc::neuf()
        .obeissant(CANAL_1, DUTY_ALLUMAGE)
        .sourd(CANAL_2, DUTY_ALLUMAGE)
        .illisible(CANAL_3);
    assert_eq!(
        banc.porte(CANAL_1),
        Some(DUTY_ALLUMAGE),
        "un canal obéissant porte sa valeur de départ"
    );
    assert_eq!(
        banc.porte(CANAL_3),
        None,
        "un canal illisible ne porte rien de connu"
    );
}

// ---------------------------------------------------------------------------
// A — le cache devient la mesure, et non plus l'intention
// ---------------------------------------------------------------------------

#[test]
fn un_canal_qui_ne_porte_pas_la_consigne_est_reecrit_au_tour_suivant() {
    // Critère d'acceptation : « une consigne écrite mais non appliquée est réémise au tour
    // suivant ».
    //
    // Le scénario prend le cas par son côté le plus retors : le canal a d'abord **vraiment**
    // appliqué, le cache de #99 est donc juste, puis la valeur s'en va sans que personne ne
    // prévienne. C'est ce qu'aucun cache d'intention ne peut voir — le noyau ne signale rien — et
    // c'est exactement ce qui s'est passé le 2026-08-15, à ceci près qu'on ignore encore quand la
    // valeur a filé.
    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(TIEDE))),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE)]),
        "premier tour : le canal porte son duty d'allumage, la consigne part"
    );
    assert_eq!(
        banc.porte(CANAL_1),
        Some(CONSIGNE_TIEDE),
        "le canal a appliqué : la relecture doit maintenant rendre la consigne"
    );

    let calme = banc.tour(&mut regulation, Some(TIEDE));
    assert!(
        calme.is_empty(),
        "rien n'a bougé, ni la température ni ce que le canal porte : rien ne doit partir, or \
         {calme:?}"
    );

    // La valeur s'en va, franchement — huit points. Aucune erreur, aucun message : c'est le cœur de
    // #110.
    banc.porte_desormais(CANAL_1, DUTY_ALLUMAGE);

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(TIEDE))),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE)]),
        "le canal ne porte plus la consigne : elle est réémise, sans que la température ait changé \
         d'un millidegré. Une régulation qui compare à ce qu'elle a cru écrire se tairait ici pour \
         toujours"
    );
}

#[test]
fn un_canal_qui_porte_la_consigne_n_est_pas_reecrit() {
    // Critère d'acceptation : « une consigne réellement appliquée n'est pas réémise ».
    //
    // C'est la promesse de #99 qui doit **tenir davantage**, pas moins : « aucune de ces cibles n'a
    // de watchdog, le tour passe une fois par seconde, et l'écart entre une régulation qui se tait
    // et une qui réécrit, c'est 86 400 trames par jour pour rien ». Le correctif de #110 doit se
    // payer en lectures, pas en écritures.
    //
    // ⚠️ Le silence n'est exigé qu'à partir du **deuxième** tour. Le sort du tout premier tour après
    // `activer`, sur un canal qui porterait déjà la consigne, est laissé ouvert en tête de fichier :
    // `un_canal_coupe_puis_repris_est_reecrit` (#99) y exige une écriture, et ce fichier ne rejuge
    // pas ce cas.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    let mut banc = Banc::neuf()
        .obeissant(CANAL_1, DUTY_ALLUMAGE)
        .obeissant(CANAL_2, DUTY_ALLUMAGE);

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(TIEDE))),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE), (CANAL_2, CONSIGNE_TIEDE)]),
        "le premier tour écrit : les deux canaux portent leur duty d'allumage"
    );

    // Mille tours à la même température, sur un matériel qui a vraiment appliqué : une heure de
    // démon, pas une trame.
    for tour in 1..=1_000u32 {
        let faites = banc.tour(&mut regulation, Some(TIEDE));
        assert!(
            faites.is_empty(),
            "au tour n° {tour}, les canaux portent déjà la consigne : rien ne doit partir, or \
             {faites:?}"
        );
    }

    assert_eq!(
        banc.tout().len(),
        2,
        "sur mille tours, seules les deux écritures du premier doivent avoir eu lieu : {:?}",
        banc.tout()
    );
}

#[test]
fn un_canal_qui_n_applique_jamais_est_reecrit_a_chaque_tour() {
    // Test d'intention de l'issue : « deux tours consécutifs sur un canal qui n'applique jamais
    // produisent deux écritures ».
    //
    // Le fichier va plus loin que deux, et c'est délibéré : l'issue pose « rejouée au tour suivant,
    // **sans intervention** », donc sans plafond de tentatives. Un plafond recréerait le défaut
    // qu'on corrige — un canal abandonné en silence à son duty d'allumage pendant que le liquide
    // monte — et c'est exactement ce que le démon a fait pendant deux minutes le 2026-08-15.
    //
    // La contrepartie est assumée : tant que le canal n'applique pas, la régulation consomme une
    // écriture par tour. C'est le prix de la réparation automatique, il cesse dès qu'elle a lieu, et
    // c'est `Motif` qui empêche le journal de le payer aussi (section E).
    const TOURS: u32 = 20;

    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut banc = Banc::neuf().sourd(CANAL_1, DUTY_ALLUMAGE);

    for tour in 1..=TOURS {
        assert_eq!(
            sans_motif(&banc.tour(&mut regulation, Some(TIEDE))),
            ecritures(&[(CANAL_1, CONSIGNE_TIEDE)]),
            "tour n° {tour} : le canal porte toujours {DUTY_ALLUMAGE} %, la consigne doit repartir"
        );
    }

    assert_eq!(
        banc.consignes_de(CANAL_1),
        vec![CONSIGNE_TIEDE; TOURS as usize],
        "un canal qui n'applique jamais reçoit une écriture par tour, indéfiniment"
    );

    // Le blocage cède : le canal applique enfin, et la régulation se tait aussitôt.
    banc.se_met_a_obeir(CANAL_1);
    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(TIEDE))),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE)]),
        "le tour de la réparation écrit une dernière fois, et cette fois-ci ça prend"
    );
    let apres = banc.tour(&mut regulation, Some(TIEDE));
    assert!(
        apres.is_empty(),
        "le canal porte enfin la consigne : la régulation doit se taire, or {apres:?}"
    );
}

#[test]
fn l_incident_du_2026_08_15_se_repare_seul() {
    // Le scénario de l'issue, à la valeur près : trois canaux activés à la suite, une consigne de
    // 50 % calculée depuis un liquide à 41,7 °C, deux canaux qui appliquent et un qui reste à son
    // duty d'allumage — vingt-cinq points plus bas.
    //
    // Ce que #99 faisait : trois lignes de journal, puis plus rien pendant deux minutes.
    // Ce que #110 exige : le tour suivant réémet sur `fan-3`, et sur lui seul.
    //
    // ⚠️ La consigne de ce tour vaut exactement `REPLI`. Ce test ne peut donc pas distinguer la
    // courbe du repli — il n'a pas à le faire, le liquide y est lisible du début à la fin, et cette
    // distinction est figée par #99.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2, CANAL_3]);
    let mut banc = Banc::neuf()
        .obeissant(CANAL_1, DUTY_ALLUMAGE)
        .obeissant(CANAL_2, DUTY_ALLUMAGE)
        .sourd(CANAL_3, DUTY_ALLUMAGE);

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(LIQUIDE_DE_L_INCIDENT))),
        ecritures(&[
            (CANAL_1, CONSIGNE_DE_L_INCIDENT),
            (CANAL_2, CONSIGNE_DE_L_INCIDENT),
            (CANAL_3, CONSIGNE_DE_L_INCIDENT),
        ]),
        "les trois canaux sont écrits — c'est ce que le journal a dit le 2026-08-15, et c'était vrai"
    );
    assert_eq!(
        banc.porte(CANAL_3),
        Some(DUTY_ALLUMAGE),
        "et c'est là que le journal a menti : « {CANAL_3} » porte toujours son duty d'allumage"
    );

    // Deux minutes de relevés, à liquide stable.
    for tour in 2..=120u32 {
        assert_eq!(
            sans_motif(&banc.tour(&mut regulation, Some(LIQUIDE_DE_L_INCIDENT))),
            ecritures(&[(CANAL_3, CONSIGNE_DE_L_INCIDENT)]),
            "tour n° {tour} : seul « {CANAL_3} » ne porte pas sa consigne, seul lui doit être réécrit"
        );
    }

    assert_eq!(
        banc.consignes_de(CANAL_1),
        vec![CONSIGNE_DE_L_INCIDENT],
        "« {CANAL_1} » a appliqué du premier coup : une écriture, et une seule sur deux minutes"
    );
    assert_eq!(
        banc.consignes_de(CANAL_2),
        vec![CONSIGNE_DE_L_INCIDENT],
        "« {CANAL_2} » aussi"
    );
    assert_eq!(
        banc.consignes_de(CANAL_3).len(),
        120,
        "« {CANAL_3} » n'a jamais appliqué : la régulation doit avoir insisté à chaque tour"
    );

    // Le canal cède enfin — c'est ce qui s'est produit après un `fan … pwm 70` à la main.
    banc.se_met_a_obeir(CANAL_3);
    banc.tour(&mut regulation, Some(LIQUIDE_DE_L_INCIDENT));

    let apres = banc.tour(&mut regulation, Some(LIQUIDE_DE_L_INCIDENT));
    assert!(
        apres.is_empty(),
        "les trois canaux portent enfin la même consigne : le bus doit redevenir silencieux, or \
         {apres:?}"
    );
    for canal in [CANAL_1, CANAL_2, CANAL_3] {
        assert_eq!(
            banc.porte(canal),
            Some(CONSIGNE_DE_L_INCIDENT),
            "« {canal} » doit porter la consigne à la fin du scénario"
        );
    }
}

#[test]
fn la_relecture_d_un_canal_non_regule_ne_declenche_aucune_ecriture() {
    // Ce que fige ce test : `portees` est une **relecture**, pas une liste de travail. Le démon
    // relit tout le matériel pour servir `status` — la fenêtre le lui demande une fois par seconde —
    // donc la carte contient les canaux du Kraken et ceux du `nzxtsmart2` qu'on n'a pas activés.
    //
    // Une implémentation qui parcourrait `portees` au lieu de ses propres canaux écrirait sur les
    // deux canaux du Kraken, dont « le firmware régule déjà correctement, mesuré » (#99, hors
    // scope). Elle passerait tous les tests de valeur de ce fichier, et écraserait une courbe
    // firmware par une boucle hôte.
    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut banc = Banc::neuf()
        .obeissant(CANAL_1, DUTY_ALLUMAGE)
        .sourd(CANAL_2, DUTY_ALLUMAGE)
        .sourd(CANAL_3, 100)
        .sourd(CANAL_KRAKEN, 0);

    // Long et irrégulier à dessein : une régulation ne se trompe pas de canal sur un seul tour.
    for temperature in (degres(20)..=degres(60)).step_by(137) {
        banc.tour(&mut regulation, Some(temperature));
        banc.tour(&mut regulation, None);
        banc.tour(&mut regulation, Some(temperature));
    }

    let touches = banc.canaux_touches();
    for interdit in [CANAL_2, CANAL_3, CANAL_KRAKEN] {
        assert!(
            !touches.contains(interdit),
            "« {interdit} » figure dans la relecture mais n'est pas régulé : il ne doit recevoir \
             aucune écriture. Canaux touchés : {touches:?}"
        );
    }
    assert!(
        !banc.consignes_de(CANAL_1).is_empty(),
        "le scénario doit avoir écrit sur le canal régulé, sinon il ne prouve rien"
    );
}

// ---------------------------------------------------------------------------
// B — une relecture qui échoue n'emporte pas le tour
// ---------------------------------------------------------------------------

#[test]
fn un_canal_illisible_est_ecarte_et_les_autres_continuent() {
    // Critère d'acceptation : « un canal dont la lecture échoue ne fait pas échouer le tour des
    // autres ».
    //
    // Deux exigences, et il faut les deux :
    //
    // — les voisins reçoivent leur consigne comme si de rien n'était. C'est la leçon des quatre
    //   chemins qui ont porté le même défaut — les sondes (#68), la dalle (#83), les canaux (#88) :
    //   « un périphérique qui cesse de répondre gelait le fil qui sert le socket » ;
    // — le canal muet ne reçoit **rien**. Écrire là où on ne mesure pas, c'est refaire #110 à
    //   l'identique : on annoncerait une consigne sans aucun moyen de savoir si elle a pris. La
    //   fenêtre l'affiche illisible (#100), l'utilisateur voit qu'il y a quelque chose à réparer, et
    //   c'est plus honnête qu'une écriture dans le vide.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2, CANAL_3]);
    let mut banc = Banc::neuf()
        .obeissant(CANAL_1, DUTY_ALLUMAGE)
        .illisible(CANAL_2)
        .obeissant(CANAL_3, DUTY_ALLUMAGE);

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(TIEDE))),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE), (CANAL_3, CONSIGNE_TIEDE)]),
        "le canal illisible est écarté, ses deux voisins reçoivent la consigne du tour"
    );

    let calme = banc.tour(&mut regulation, Some(TIEDE));
    assert!(
        calme.is_empty(),
        "les voisins portent la consigne et le muet est toujours muet : rien ne doit partir, or \
         {calme:?}"
    );

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(CHAUD))),
        ecritures(&[(CANAL_1, CONSIGNE_CHAUD), (CANAL_3, CONSIGNE_CHAUD)]),
        "le liquide monte de quarante-trois points de consigne : les voisins suivent la courbe, le \
         muet reste écarté"
    );

    assert!(
        banc.consignes_de(CANAL_2).is_empty(),
        "« {CANAL_2} » n'a pas répondu une seule fois : il ne doit avoir reçu aucune écriture, or \
         il a reçu {:?}",
        banc.consignes_de(CANAL_2)
    );
}

#[test]
fn un_canal_absent_de_la_relecture_est_ecarte_comme_un_illisible() {
    // L'issue dit « un canal dont la lecture échoue ». Reste à savoir ce qu'est un échec du point de
    // vue de `tour`, et ce fichier tranche : une entrée `None` et une entrée absente sont la même
    // chose — « je ne sais pas ce que ce canal porte ».
    //
    // Les confondre est le bon choix parce que la régulation ne peut rien faire de la différence :
    // un canal en quarantaine (#68) n'est plus relevé du tout, donc il ne figure dans aucun relevé,
    // donc l'absence est exactement la forme que prend une quarantaine installée. Traiter l'absence
    // comme « écris quand même » ferait écrire précisément sur les canaux dont on sait qu'ils ne
    // répondent plus.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    let mut banc = Banc::neuf()
        .obeissant(CANAL_1, DUTY_ALLUMAGE)
        .absent(CANAL_2);

    for tour in 1..=5u32 {
        let faites = sans_motif(&banc.tour(&mut regulation, Some(TIEDE)));
        let attendues = if tour == 1 {
            ecritures(&[(CANAL_1, CONSIGNE_TIEDE)])
        } else {
            ecritures(&[])
        };
        assert_eq!(
            faites, attendues,
            "tour n° {tour} : « {CANAL_2} » ne figure pas dans la relecture, il est écarté ; \
             « {CANAL_1} » suit son cours"
        );
    }

    assert!(
        banc.consignes_de(CANAL_2).is_empty(),
        "un canal absent de la relecture ne reçoit rien, or il a reçu {:?}",
        banc.consignes_de(CANAL_2)
    );
}

#[test]
fn le_retour_d_un_canal_ecarte_reprend_la_regulation() {
    // Test d'intention de l'issue : « le retour d'un canal écarté reprend la régulation ».
    //
    // C'est la moitié qu'on oublie. Un écartement définitif transformerait chaque hoquet du bus en
    // canal abandonné pour toujours — sans message, sans erreur, et sans rien pour le distinguer
    // d'un canal correctement régulé. Exactement le défaut de #110, obtenu par un autre chemin.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    let mut banc = Banc::neuf()
        .obeissant(CANAL_1, DUTY_ALLUMAGE)
        .illisible(CANAL_2);

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(TIEDE))),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE)]),
        "« {CANAL_2} » est muet : écarté"
    );

    for tour in 2..=10u32 {
        let faites = banc.tour(&mut regulation, Some(TIEDE));
        assert!(
            faites.is_empty(),
            "tour n° {tour} : rien n'a changé, rien ne doit partir, or {faites:?}"
        );
    }

    // Le canal répond de nouveau, et il porte son duty d'allumage — personne ne l'a régulé pendant
    // tout ce temps.
    banc.porte_desormais(CANAL_2, DUTY_ALLUMAGE);
    banc.se_met_a_obeir(CANAL_2);

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(TIEDE))),
        ecritures(&[(CANAL_2, CONSIGNE_TIEDE)]),
        "« {CANAL_2} » répond de nouveau et ne porte pas la consigne : la régulation le reprend, \
         sans redémarrage et sans qu'on ait rien réarmé"
    );

    let apres = banc.tour(&mut regulation, Some(TIEDE));
    assert!(
        apres.is_empty(),
        "et une fois qu'il porte la consigne, le silence revient : {apres:?}"
    );
}

#[test]
fn un_canal_ecarte_qui_revient_en_portant_la_consigne_ne_recoit_rien() {
    // Le pendant du test précédent, et il compte autant : la décision se prend sur ce que le canal
    // **porte**, jamais sur le temps passé sans nouvelles.
    //
    // Une implémentation qui poserait un drapeau « je l'ai perdu de vue, je le réécrirai au retour »
    // passerait `le_retour_d_un_canal_ecarte_reprend_la_regulation` et échouerait ici — et sur un
    // bus qui hoquette, elle écrirait à chaque retour pour rien. Le remède de #110 est de mesurer,
    // et mesurer suffit : il n'y a pas d'état à tenir en plus.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    let mut banc = Banc::neuf()
        .obeissant(CANAL_1, DUTY_ALLUMAGE)
        .obeissant(CANAL_2, DUTY_ALLUMAGE);

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(TIEDE))),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE), (CANAL_2, CONSIGNE_TIEDE)]),
        "les deux canaux partent de leur duty d'allumage et reçoivent la consigne"
    );

    banc.devient_illisible(CANAL_2);
    for tour in 2..=5u32 {
        let faites = banc.tour(&mut regulation, Some(TIEDE));
        assert!(
            faites.is_empty(),
            "tour n° {tour} : « {CANAL_2} » est muet et « {CANAL_1} » porte sa consigne, or \
             {faites:?}"
        );
    }

    // Il revient, et il avait bel et bien appliqué avant de se taire.
    banc.porte_desormais(CANAL_2, CONSIGNE_TIEDE);

    let retour = banc.tour(&mut regulation, Some(TIEDE));
    assert!(
        retour.is_empty(),
        "« {CANAL_2} » revient en portant déjà la consigne : il n'y a rien à écrire, or {retour:?}"
    );

    // Et il continue d'être régulé comme les autres, ce qui prouve qu'il n'a pas été oublié.
    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(CHAUD))),
        ecritures(&[(CANAL_1, CONSIGNE_CHAUD), (CANAL_2, CONSIGNE_CHAUD)]),
        "le liquide monte : les deux canaux suivent, y compris celui qui s'était tu"
    );
}

// ---------------------------------------------------------------------------
// C — le repli traverse exactement la même règle
// ---------------------------------------------------------------------------

#[test]
fn un_repli_non_applique_est_rejoue_comme_une_consigne_de_courbe() {
    // #110 ne mentionne que « la consigne », sans distinguer d'où elle vient. Ce test tranche : le
    // repli est une consigne comme une autre, et il est réémis tant qu'il n'a pas pris.
    //
    // C'est le cas qui compte le plus, parce que c'est le pire moment pour se tromper : le repli
    // part quand le liquide est illisible, donc quand le Kraken est en difficulté, donc quand
    // personne ne mesure plus la température du circuit. Un repli qu'on croit posé et qui ne l'est
    // pas, c'est un ventilateur à 25 % derrière une sonde morte — la superposition exacte des deux
    // modes de défaillance que le projet refuse.
    const TOURS: u32 = 5;

    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut banc = Banc::neuf().sourd(CANAL_1, DUTY_ALLUMAGE);

    for tour in 1..=TOURS {
        assert_eq!(
            sans_motif(&banc.tour(&mut regulation, None)),
            ecritures(&[(CANAL_1, REPLI)]),
            "tour n° {tour} : la sonde est muette et le canal ne porte pas le repli, il doit \
             repartir"
        );
    }

    banc.se_met_a_obeir(CANAL_1);
    banc.tour(&mut regulation, None);

    let apres = banc.tour(&mut regulation, None);
    assert!(
        apres.is_empty(),
        "le repli est enfin porté : plus rien ne doit partir, or {apres:?}"
    );
}

#[test]
fn un_canal_qui_porte_deja_le_repli_ne_recoit_rien_quand_la_sonde_se_tait() {
    // Le point de rencontre des deux règles, mesuré cette fois : « on n'écrit que ce que le canal ne
    // porte pas » et « la sonde muette fait retomber à 50 % ». Si le canal porte déjà 50 %, la sonde
    // peut se taire : il n'y a rien à écrire.
    //
    // C'est la version #110 de `un_repli_egal_a_ce_qui_est_deja_ecrit_ne_reecrit_rien` (#99), et
    // elle est plus forte : là-bas la valeur venait du cache, ici elle vient de la relecture. Une
    // implémentation qui garderait un drapeau « je suis en repli » réécrirait ici, puis de nouveau
    // au retour de la sonde.
    //
    // La courbe est plate à la valeur du repli exprès : c'est la seule façon de construire ce cas
    // sans dépendre d'un arrondi.
    let mut regulation = Regulation::nouvelle(courbe(&[(degres(30), REPLI), (degres(60), REPLI)]));
    regulation.activer(CANAL_1);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(MEDIAN))),
        ecritures(&[(CANAL_1, REPLI)]),
        "la courbe plate rend exactement la consigne de repli, et le canal l'applique"
    );

    for (contexte, liquide) in [
        ("la sonde se tait", None),
        ("elle se tait encore", None),
        ("elle revient", Some(MEDIAN)),
        ("elle bouge sans changer la consigne", Some(CHAUD)),
    ] {
        let faites = banc.tour(&mut regulation, liquide);
        assert!(
            faites.is_empty(),
            "{contexte} : le canal porte déjà {REPLI} %, rien ne doit partir, or {faites:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// D — ce que #99 promettait, et qui doit tenir
// ---------------------------------------------------------------------------

#[test]
fn un_canal_tout_juste_active_est_ecrit_au_premier_tour() {
    // Régression de #99 : « la régulation s'active et se coupe par canal », et un canal qu'on vient
    // d'activer reçoit la consigne courante même si la température n'a pas bougé.
    //
    // #110 ne doit pas l'affaiblir : sans ce tour-là, rien n'est jamais écrit, et c'est exactement
    // l'état de la machine avant #99 — sept ventilateurs à 25 % qui n'ont jamais reçu de consigne.
    // Les canaux partent ici de leur duty d'allumage, ce qui est la situation réelle : au démarrage
    // du démon comme à l'activation, le canal ne porte pas la consigne.
    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut banc = Banc::neuf()
        .obeissant(CANAL_1, DUTY_ALLUMAGE)
        .obeissant(CANAL_2, DUTY_ALLUMAGE);

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(TIEDE))),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE)]),
        "un seul canal régulé au départ, et il est écrit dès le premier tour"
    );

    regulation.activer(CANAL_2);
    assert_eq!(
        regulation.canaux(),
        vec![CANAL_1.to_owned(), CANAL_2.to_owned()],
        "les deux canaux sont désormais régulés, et `canaux` les rend triés"
    );

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(TIEDE))),
        ecritures(&[(CANAL_2, CONSIGNE_TIEDE)]),
        "le canal tout juste activé porte son duty d'allumage : il reçoit la consigne courante, \
         sans attendre un changement de palier qui, un jour de température stable, ne viendrait \
         jamais"
    );

    let apres = banc.tour(&mut regulation, Some(TIEDE));
    assert!(
        apres.is_empty(),
        "et au tour d'après, les deux portent la consigne : plus rien ne bouge, or {apres:?}"
    );
}

#[test]
fn sans_canal_regule_aucune_relecture_ne_produit_la_moindre_ecriture() {
    // Régression de #99 : « la régulation n'ajoute aucun réveil quand aucun canal n'est régulé », et
    // sa seule forme observable depuis un module pur — une régulation vide ne produit rien.
    //
    // #110 ajoute une raison de le revérifier : la relecture est fournie par le démon, et elle
    // contient tout le matériel de la machine. Une implémentation qui déciderait à partir de
    // `portees` régulerait dix canaux que personne n'a activés, y compris ceux du Kraken.
    let mut regulation = Regulation::nouvelle(Courbe::defaut());
    let mut banc = Banc::neuf()
        .sourd(CANAL_1, DUTY_ALLUMAGE)
        .sourd(CANAL_2, 0)
        .sourd(CANAL_3, 100)
        .illisible(CANAL_KRAKEN);

    assert!(
        regulation.canaux().is_empty(),
        "une régulation neuve ne régule rien"
    );

    for temperature in (degres(-50)..=degres(120)).step_by(311) {
        banc.tour(&mut regulation, Some(temperature));
        banc.tour(&mut regulation, None);
    }
    banc.tour(&mut regulation, Some(i32::MAX));
    banc.tour(&mut regulation, Some(i32::MIN));

    assert!(
        banc.tout().is_empty(),
        "sans canal régulé, aucune écriture ne doit jamais partir, quoi que la relecture rende, or \
         {:?}",
        banc.tout()
    );
}

#[test]
fn le_liquide_illisible_fait_partir_le_repli_sur_chaque_canal_regule() {
    // Régression de #99 : « le liquide illisible produit la consigne de repli », et « jamais la
    // dernière valeur connue ».
    //
    // ⚠️ La relecture de #110 ne doit pas fournir une nouvelle façon de garder la dernière valeur :
    // le canal porte 33 % et la sonde se tait, la consigne devient le repli — pas ce que le canal
    // porte, qui serait précisément « la dernière valeur connue », lue à la source cette fois.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    let mut banc = Banc::neuf()
        .obeissant(CANAL_1, DUTY_ALLUMAGE)
        .obeissant(CANAL_2, DUTY_ALLUMAGE);

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(TIEDE))),
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE), (CANAL_2, CONSIGNE_TIEDE)]),
        "le liquide est lisible : la courbe s'applique"
    );

    let faites = sans_motif(&banc.tour(&mut regulation, None));
    assert_eq!(
        faites,
        ecritures(&[(CANAL_1, REPLI), (CANAL_2, REPLI)]),
        "la sonde se tait : le repli part sur chaque canal régulé"
    );
    assert_ne!(
        faites,
        ecritures(&[(CANAL_1, CONSIGNE_TIEDE), (CANAL_2, CONSIGNE_TIEDE)]),
        "garder {CONSIGNE_TIEDE} % parce que c'est ce que les canaux portaient, c'est laisser \
         croire que le liquide est mesuré alors que plus rien ne le mesure"
    );

    let ensuite = banc.tour(&mut regulation, None);
    assert!(
        ensuite.is_empty(),
        "le repli est posé et il a pris : il ne doit pas repartir à chaque tour, or {ensuite:?}"
    );
}

#[test]
fn un_canal_coupe_n_est_jamais_reecrit_quoi_que_la_relecture_rende() {
    // Régression de #99 : « couper, c'est rendre le canal à l'utilisateur », et le critère
    // d'acceptation de #110 « un canal repris à la main (`fan … pwm`) n'est pas réécrit par la
    // régulation » — la reprise à la main passe par `couper`.
    //
    // ⚠️ C'est ici que #110 crée un vrai risque de régression, et c'est pourquoi ce test existe. La
    // relecture d'un canal coupé montrera **toujours** un écart avec la consigne que la courbe
    // calcule pour lui — c'est même le but de la coupure. Une implémentation qui déciderait « la
    // relecture diffère, donc j'écris » sans regarder d'abord si le canal est régulé reprendrait la
    // main sur un canal qu'on vient de lui retirer, et effacerait le `fan … pwm 70` de
    // l'utilisateur au tour suivant.
    let mut regulation = regulation_sur(&[CANAL_1, CANAL_2]);
    let mut banc = Banc::neuf()
        .obeissant(CANAL_1, DUTY_ALLUMAGE)
        .obeissant(CANAL_2, DUTY_ALLUMAGE);

    banc.tour(&mut regulation, Some(TIEDE));
    regulation.couper(CANAL_1);
    assert_eq!(
        regulation.canaux(),
        vec![CANAL_2.to_owned()],
        "un canal coupé sort de la liste des canaux régulés"
    );

    // L'utilisateur pose ce qu'il veut, et le canal traverse tous les états possibles de la
    // relecture : une valeur à lui, un extrême, et un silence.
    let porte_par_canal_2 = banc
        .porte(CANAL_2)
        .expect("le canal resté régulé est lisible à la fin du premier scénario");
    let mut apres_la_coupure = Banc::neuf()
        .obeissant(CANAL_1, 70)
        .obeissant(CANAL_2, porte_par_canal_2);

    for temperature in (degres(20)..=degres(60)).step_by(500) {
        apres_la_coupure.tour(&mut regulation, Some(temperature));
    }
    apres_la_coupure.porte_desormais(CANAL_1, 0);
    apres_la_coupure.tour(&mut regulation, Some(CHAUD));
    apres_la_coupure.devient_illisible(CANAL_1);
    apres_la_coupure.tour(&mut regulation, Some(TIEDE));
    apres_la_coupure.tour(&mut regulation, None);

    assert!(
        apres_la_coupure.consignes_de(CANAL_1).is_empty(),
        "« {CANAL_1} » est coupé : ni la courbe, ni le repli, ni un écart constaté à la relecture ne \
         doivent plus rien lui écrire, or il a reçu {:?}",
        apres_la_coupure.consignes_de(CANAL_1)
    );
    assert!(
        !apres_la_coupure.consignes_de(CANAL_2).is_empty(),
        "« {CANAL_2} » est resté régulé : il doit continuer de recevoir la courbe, sinon ce test ne \
         prouve rien"
    );
}

// ---------------------------------------------------------------------------
// E — le motif : ce que le démon a le droit d'imprimer
// ---------------------------------------------------------------------------

#[test]
fn une_consigne_qui_change_part_avec_le_motif_consigne() {
    // Critère d'acceptation : « le journal ne peut plus annoncer une consigne que le canal ne porte
    // pas ». `regulation.rs` ne journalise rien — il reste pur —, mais il doit donner à l'appelant
    // de quoi écrire une phrase juste. Le cas normal en est la moitié la plus simple : la
    // température a changé de palier, c'est une information neuve, elle se journalise.
    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    banc.tour(&mut regulation, Some(TIEDE));
    let montee = banc.tour(&mut regulation, Some(CHAUD));

    assert_eq!(
        montee,
        vec![Ecriture {
            canal: CANAL_1.to_owned(),
            consigne: CONSIGNE_CHAUD,
            motif: Motif::Consigne,
        }],
        "la consigne passe de {CONSIGNE_TIEDE} % à {CONSIGNE_CHAUD} % : c'est une consigne neuve, \
         et le motif doit le dire"
    );
}

#[test]
fn un_canal_tout_juste_active_part_avec_le_motif_consigne() {
    // L'autre moitié de `Motif::Consigne` : un canal qu'on vient de prendre en charge n'a jamais
    // rien reçu, donc son écriture n'est pas une réémission. Le journaliser est même souhaitable —
    // c'est la trace de « le démon a pris ce canal », qui n'apparaît nulle part ailleurs.
    //
    // Une implémentation qui classerait ce cas en `NonAppliquee` — parce que, techniquement, le
    // canal ne porte pas la consigne — priverait le journal du seul événement qu'il devrait
    // toujours montrer.
    let mut regulation = Regulation::nouvelle(Courbe::defaut());
    regulation.activer(CANAL_1);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    let premier = banc.tour(&mut regulation, Some(TIEDE));

    assert_eq!(
        premier,
        vec![Ecriture {
            canal: CANAL_1.to_owned(),
            consigne: CONSIGNE_TIEDE,
            motif: Motif::Consigne,
        }],
        "le canal vient d'être pris en charge : sa première écriture est une consigne, pas une \
         réémission"
    );
}

#[test]
fn une_consigne_inchangee_que_le_canal_ne_porte_pas_part_avec_le_motif_non_appliquee() {
    // Le motif qui n'existait pas le 2026-08-15, et l'information qui manquait : **ce que le canal
    // portait**. Le journal disait « fan-3 à 50 % » ; il aurait dû pouvoir dire « fan-3 ne porte que
    // 25 %, je rejoue 50 % ». C'est la différence entre un journal qui rassure et un journal qui
    // diagnostique.
    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut banc = Banc::neuf().sourd(CANAL_1, DUTY_ALLUMAGE);

    let premier = banc.tour(&mut regulation, Some(TIEDE));
    assert_eq!(
        premier,
        vec![Ecriture {
            canal: CANAL_1.to_owned(),
            consigne: CONSIGNE_TIEDE,
            motif: Motif::Consigne,
        }],
        "le premier tour reste une consigne neuve"
    );

    let second = banc.tour(&mut regulation, Some(TIEDE));
    assert_eq!(
        second,
        vec![Ecriture {
            canal: CANAL_1.to_owned(),
            consigne: CONSIGNE_TIEDE,
            motif: Motif::NonAppliquee {
                porte: Some(DUTY_ALLUMAGE)
            },
        }],
        "la consigne n'a pas changé et le canal ne la porte pas : c'est une réémission, et elle \
         doit rapporter les {DUTY_ALLUMAGE} % réellement portés"
    );
}

#[test]
fn trois_tours_sur_un_canal_qui_n_applique_jamais_donnent_une_consigne_puis_des_reemissions() {
    // ⚠️ Le test qui empêche #110 de rendre le journal inutilisable. Une écriture par seconde sur un
    // canal bloqué, c'est **86 400 lignes par jour** si l'appelant les traite toutes pareil — très
    // exactement le chiffre que #99 invoquait pour justifier son cache, retourné contre lui.
    //
    // Trois `Motif::Consigne` d'affilée rendraient le silence impossible à l'appelant : il ne
    // pourrait pas distinguer « la température a bougé trois fois » de « le même ordre répété trois
    // fois ». Une seule consigne, puis des réémissions, lui laisse le choix — journaliser la
    // première, compter les suivantes, et n'en reparler qu'à la réparation.
    const TOURS: usize = 3;

    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut banc = Banc::neuf().sourd(CANAL_1, DUTY_ALLUMAGE);

    for _ in 0..TOURS {
        banc.tour(&mut regulation, Some(TIEDE));
    }

    let attendus: Vec<Motif> = std::iter::once(Motif::Consigne)
        .chain(std::iter::repeat_n(
            Motif::NonAppliquee {
                porte: Some(DUTY_ALLUMAGE),
            },
            TOURS - 1,
        ))
        .collect();

    assert_eq!(
        banc.motifs_de(CANAL_1),
        attendus,
        "{TOURS} tours à température stable sur un canal qui n'applique jamais : une consigne, puis \
         des réémissions. Ce sont les {TOURS} écritures qui partent sur le bus, mais une seule ligne \
         de journal neuve"
    );
    assert_eq!(
        banc.consignes_de(CANAL_1),
        vec![CONSIGNE_TIEDE; TOURS],
        "les {TOURS} écritures partent bel et bien : le motif change ce qu'on en dit, pas ce qu'on \
         écrit"
    );
}

#[test]
fn une_consigne_qui_change_sur_un_canal_qui_n_applique_pas_reste_une_consigne() {
    // Le cas où les deux motifs se disputent : le canal n'a pas appliqué **et** la consigne vient de
    // changer. C'est `Motif::Consigne` qui l'emporte, par la définition même du type — « la consigne
    // a changé » —, et parce que c'est l'information neuve : le palier a bougé, le journal doit le
    // dire, même si le canal traîne.
    //
    // Sans cette règle, un canal durablement bloqué ne journaliserait plus jamais un changement de
    // palier, et le journal cesserait de montrer que la régulation travaille encore.
    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut banc = Banc::neuf().sourd(CANAL_1, DUTY_ALLUMAGE);

    banc.tour(&mut regulation, Some(TIEDE));
    banc.tour(&mut regulation, Some(TIEDE));
    let montee = banc.tour(&mut regulation, Some(CHAUD));

    assert_eq!(
        montee,
        vec![Ecriture {
            canal: CANAL_1.to_owned(),
            consigne: CONSIGNE_CHAUD,
            motif: Motif::Consigne,
        }],
        "le canal n'applique toujours pas, mais la consigne passe à {CONSIGNE_CHAUD} % : c'est un \
         changement de palier, et il se journalise"
    );
}

#[test]
fn le_repli_suit_la_meme_regle_de_motif() {
    // Le repli est une consigne comme une autre, motif compris : neuf au premier tour, réémission
    // ensuite. C'est le moment où le journal compte le plus — la sonde du liquide est muette, le
    // Kraken est en difficulté — et c'est donc le moment où il ne doit ni se taire ni déborder.
    let mut regulation = regulation_sur(&[CANAL_1]);
    let mut banc = Banc::neuf().sourd(CANAL_1, DUTY_ALLUMAGE);

    let premier = banc.tour(&mut regulation, None);
    assert_eq!(
        premier,
        vec![Ecriture {
            canal: CANAL_1.to_owned(),
            consigne: REPLI,
            motif: Motif::Consigne,
        }],
        "la sonde se tait : le repli est une consigne neuve"
    );

    for tour in 2..=4u32 {
        assert_eq!(
            banc.tour(&mut regulation, None),
            vec![Ecriture {
                canal: CANAL_1.to_owned(),
                consigne: REPLI,
                motif: Motif::NonAppliquee {
                    porte: Some(DUTY_ALLUMAGE)
                },
            }],
            "tour n° {tour} : le repli n'a pas pris, il repart — en réémission, pas en consigne neuve"
        );
    }
}
