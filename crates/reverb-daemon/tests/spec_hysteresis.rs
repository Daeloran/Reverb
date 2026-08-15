//! Tests d'intention — issue #111, « La consigne de régulation oscille d'un point et réécrit sans
//! cesse ».
//!
//! Écrits **avant** l'implémentation, depuis l'issue #111 seule et ses commentaires. Aucun fichier
//! de `crates/*/src/` n'a été lu pour les produire : ni `regulation.rs`, ni `main.rs`. Les seules
//! signatures employées sont celles que `spec_regulation_hote.rs` (#99) et
//! `spec_relecture_consigne.rs` (#110) figent déjà en tête de leurs propres fichiers, plus **une**
//! constante que #111 ajoute et qui n'existe pas encore : `HYSTERESIS`. La compilation doit donc
//! échouer, et c'est la phase rouge.
//!
//! Rien ici n'ouvre un `hwmon`, n'écrit un `pwm*`, ne dort ni ne lit l'horloge. La température est
//! **injectée**, ce que le matériel porte est **injecté**, et l'écriture est enregistrée en
//! mémoire : ce fichier vérifie une décision, pas un bus.
//!
//! ## Le défaut que ce fichier existe pour interdire
//!
//! Constaté sur SHYNAEL le 2026-08-15, régulation de #99 active, machine au repos :
//!
//! ```text
//! 15:51:10  nzxtsmart2:fan-{1,2,3} à 46 %   (liquide 40.4 °C)
//! 15:51:48  nzxtsmart2:fan-{1,2,3} à 45 %   (liquide 40.1 °C)
//! 15:52:28  nzxtsmart2:fan-{1,2,3} à 46 %   (liquide 40.2 °C)
//! ```
//!
//! **Trente écritures en huit minutes** sur trois canaux, pour une amplitude de température de
//! **0,3 °C**. La sonde du liquide bruite de ±0,1 à 0,3 °C d'une lecture à l'autre ; la courbe fait
//! 3 %/°C sur ce segment, donc ce bruit vaut **±1 point de consigne**, et chaque point franchi
//! déclenche une écriture.
//!
//! ⚠️ **La règle de #99 est respectée à la lettre et ratée en esprit.** « On n'écrit que ce qui
//! change » est vrai — la consigne *change* vraiment — et la régulation ne se tait jamais pour
//! autant. Extrapolé, c'est ~3 500 écritures par jour et par canal là où la température ne bouge
//! pas, sur un contrôleur qui a déjà montré qu'il n'aime pas les écritures rapprochées.
//!
//! ⚠️ **Ce n'est pas un effet de l'activation** : le second relevé a été pris après des heures de
//! régulation, et la période entre deux bascules y est d'une quarantaine de secondes. La sonde
//! bruite lentement, donc l'hystérésis n'a pas à être rapide — elle a juste à exister.
//!
//! ## Ce que #111 change, en une phrase
//!
//! #110 a posé : **écrire si le canal est lisible et (jamais écrit depuis son activation ou ce
//! qu'il porte diffère de la consigne)**. #111 restreint la seconde branche :
//!
//! > écrire si **lisible** et (**jamais écrit depuis l'activation** ou **l'écart entre ce qu'il
//! > porte et la consigne mérite une écriture**)
//!
//! et « mérite » est ce que ce fichier définit :
//!
//! ```ignore
//! /// L'écart de consigne, en points de pourcentage, à partir duquel une réécriture part.
//! ///
//! /// Le bruit relevé le 2026-08-15 vaut ±1 point ; deux points suffisent donc à le faire taire,
//! /// et laissent passer toute variation dépassant 0,7 °C de liquide.
//! pub const HYSTERESIS: u8 = 2;
//! ```
//!
//! avec **trois exemptions**, toutes exigées par les critères d'acceptation :
//!
//! 1. **Un canal jamais écrit depuis son activation** part quoi qu'il porte — c'est déjà #99
//!    (`un_canal_coupe_puis_repris_est_reecrit`) et #110, et ça doit le rester.
//! 2. **Une consigne qui vaut une borne — 0 % ou 100 %** — part dès qu'elle diffère, sans seuil.
//!    C'est le sens de la borne : « à fond » ne doit pas s'arrêter à 99, et « à l'arrêt » pas à 1.
//! 3. **Le repli d'une sonde muette** part quoi qu'il arrive. C'est la seule écriture du système
//!    qui signale une panne, et l'amortir la rendrait indistincte d'un régime normal.
//!
//! ## Les trois choix que ce fichier tranche, et pourquoi
//!
//! ### 1. Le seuil porte sur `|porté − consigne|`, jamais sur `|consigne − dernière écrite|`
//!
//! C'est la grandeur que #110 vient de rendre **réelle** : « la décision se prend sur ce que le
//! canal porte, et non sur ce qu'on a cru y écrire ». Trois raisons, dans cet ordre :
//!
//! - **elle compose**. `fan-3` bloqué à 25 % pour une consigne de 46 montre un écart de 21 points
//!   et continue d'être réécrit à chaque tour, exactement comme #110 l'exige. La comparaison à
//!   l'intention le rendrait **muet** dès le premier tour de bruit : la consigne repasserait de 46
//!   à 45 sans jamais s'écarter de plus d'un point de la dernière écrite, et un canal bloqué
//!   deviendrait invisible. C'est le seul risque sérieux de cette issue, et
//!   `le_seuil_porte_sur_ce_que_le_canal_porte_et_non_sur_la_derniere_consigne` est le test qui
//!   sépare les deux lectures ;
//! - **elle n'ajoute aucun état**. `|porté − consigne|` se calcule des deux arguments de `tour` ;
//!   `|consigne − dernière écrite|` demanderait de retenir la dernière consigne *calculée* en plus
//!   de ce que le cache de #99 garde déjà, et deux mémoires qui se ressemblent finissent par
//!   diverger ;
//! - **c'est la grandeur que l'utilisateur subit**. Ce qui compte n'est pas de combien la consigne
//!   a bougé, c'est de combien le ventilateur est loin de ce qu'on lui demande.
//!
//! ⚠️ **Conséquence assumée, et il faut la dire** : sous le seuil, #111 rend **indistinguables** un
//! bruit d'un point et une non-application d'un point. Un canal à qui on a écrit 46 et qui n'en
//! porte que 45 ne sera plus rejoué. Ce n'est pas un défaut de la règle, c'est sa définition — les
//! deux situations produisent littéralement la même lecture, et rien dans `portees` ne permet de
//! les distinguer. Le prix est d'un point de duty sur 255 ; le gain est les 86 400 trames par jour
//! que #99 invoquait déjà.
//!
//! ### 2. Le seuil est **atteint**, pas dépassé : on écrit dès que l'écart vaut `HYSTERESIS`
//!
//! Le bruit relevé vaut ±1 point. Le seuil est choisi contre cette amplitude-là : le plus petit
//! écart qui doit **encore** faire taire la régulation est 1, donc le plus petit qui doit la faire
//! écrire est 2, donc `HYSTERESIS` est le premier écart qui part. `écart >= HYSTERESIS`, et le nom
//! se lit alors comme ce qu'il est — « il faut deux points pour bouger ».
//!
//! La forme observable de ce choix est la même que celle de l'anti-dérive, et c'est voulu :
//! **après chaque tour, ce que le canal porte est à moins de `HYSTERESIS` points de la consigne.**
//! C'est le seul énoncé qui borne à la fois le retard d'une montée et la stagnation d'une dérive
//! lente, et c'est lui que les tests de la section B vérifient tour par tour.
//!
//! ### 3. `Motif` ne gagne **aucune** variante
//!
//! Une écriture retenue par l'hystérésis n'est pas une écriture d'un autre genre : c'est une
//! écriture **qui n'a pas lieu**. `tour` rend un vecteur vide, comme pour une consigne inchangée,
//! et le démon n'a rien de neuf à journaliser — journaliser ce qu'on n'écrit pas serait remplacer
//! 86 400 trames par 86 400 lignes. `l_hysteresis_ne_cree_aucun_motif_nouveau` le vérifie par un
//! `match` sans joker, qui cesse de compiler si une troisième variante apparaît.
//!
//! ## ⚠️ Deux tests de #99 deviennent intenables, et ce n'est pas un choix de ce fichier
//!
//! **Mesuré**, avant d'écrire une ligne de ce fichier, en balayant `Courbe::defaut()` sur les
//! rampes que #99 emploie :
//!
//! | test de `spec_regulation_hote.rs` | pas | changements de consigne | dont **d'un seul point** |
//! |---|---|---|---|
//! | `chaque_changement_de_palier_produit_une_ecriture_et_une_seule` | 250 m°C | 50 | **30** |
//! | `tous_les_canaux_regules_recoivent_la_meme_consigne` | 500 m°C | 30 | **10** |
//!
//! Les deux exigent **une écriture à chaque changement de consigne**, changements d'un point
//! compris. C'est précisément ce que #111 interdit, et aucune définition du seuil n'y échappe :
//! sur le segment le plus doux de la courbe par défaut (3 %/°C), un pas de 250 m°C fait monter la
//! consigne de 0,75 point, donc de 0 ou 1 point entier — quel que soit l'arrondi, que #99 laisse
//! d'ailleurs ouvert.
//!
//! La contradiction est donc **entre l'issue #111 et deux tests de #99**, pas entre ce fichier et
//! eux : aucune assertion écrite ici ne rejuge un cas qu'ils jugent. C'est le cas que le workflow
//! prévoit — un test d'intention devenu intenable signale une règle que l'issue suivante a changée
//! — et c'est déjà arrivé une fois sur ce même fichier (commit `d7d0ad9`). Le correctif le plus
//! petit est un **pas de rampe plus large** : à 1 000 m°C, la courbe par défaut monte de 3 points
//! par pas sur son segment le plus doux et de 8 sur le plus raide, donc tout changement y est franc
//! et les deux tests retrouvent leur sens sans perdre une assertion. **Ce fichier n'y touche pas**
//! — la décision appartient à l'implémentation de #111.
//!
//! ## Ce que ce fichier ne teste pas, et pourquoi
//!
//! - **Le quart d'heure de journal** : `regulation.rs` ne journalise rien, et un module qui ne
//!   journalise pas ne se teste pas sur ses phrases. Ce que #111 lui donne, c'est le silence.
//! - **La valeur exacte du seuil** : les tests s'y réfèrent par `HYSTERESIS` et pincent seulement
//!   la fourchette que l'issue autorise, « 2 ou 3 points ». Un test qui comparerait à `2` en dur
//!   figerait la valeur au lieu de la règle.
//! - **Le sort d'une consigne qui **quitte** une borne** — 100 % qui redevient 99 % : c'est une
//!   consigne ordinaire, donc soumise au seuil. Ce n'est pas un oubli, c'est le sens de l'exemption
//!   (« à fond ne doit pas s'arrêter à 99 »), et
//!   `une_consigne_qui_n_est_pas_une_borne_reste_soumise_au_seuil` le vérifie plutôt que de
//!   l'espérer : rester une seconde de plus à plein régime ne coûte rien, ne jamais y arriver si.
//! - **Le lissage de la sonde et la quantification de la courbe** : les deux autres remèdes
//!   proposés par l'issue, tous deux écartés au profit de l'hystérésis.

use std::collections::{BTreeMap, BTreeSet};

use reverb_daemon::regulation::{Courbe, Ecriture, HYSTERESIS, Motif, REPLI, Regulation};

// ---------------------------------------------------------------------------
// Repères
// ---------------------------------------------------------------------------

/// Les millidegrés d'un nombre entier de degrés, comme `hwmon` les rend.
const fn degres(entiers: i32) -> i32 {
    entiers * 1_000
}

/// Les trois canaux du relevé : ceux du `nzxtsmart2`, dont le pilote n'a aucun mode automatique.
const CANAL_1: &str = "nzxtsmart2:fan-1";
const CANAL_2: &str = "nzxtsmart2:fan-2";
const CANAL_3: &str = "nzxtsmart2:fan-3";

/// Le duty d'allumage de ces canaux, en pourcentage : `pwm = 64` sur 255, que le noyau rend
/// `(64 × 100 + 127) / 255 = 25`. C'est ce que porte un canal dont personne ne s'est encore occupé.
const DUTY_ALLUMAGE: u8 = 25;

/// Les deux bornes d'une consigne, et les deux valeurs que l'exemption n° 2 fait partir sans seuil.
const BORNE_BASSE: u8 = 0;
const BORNE_HAUTE: u8 = 100;

/// La fourchette que l'issue laisse au seuil : « un seuil (2 ou 3 points) ».
///
/// ⚠️ **Les deux bornes sont des exigences, pas des décorations.**
///
/// - En dessous de `SEUIL_MINIMAL`, la règle est vide : `écart >= 1` est **exactement** celle de
///   #110, et le bruit relevé vaut ±1 point — un seuil d'un point ne ferait taire rien du tout.
/// - Au-dessus de `SEUIL_MAXIMAL`, l'hystérésis se met à retarder une vraie montée, ce que l'issue
///   interdit : sur le segment le plus doux de la courbe par défaut (3 %/°C), quatre points de
///   consigne valent déjà 1,3 °C de liquide laissés sans réaction.
const SEUIL_MINIMAL: u8 = 2;
const SEUIL_MAXIMAL: u8 = 3;

/// Les cinq lectures du liquide relevées le 2026-08-15 entre 15:51 et 15:52, en millidegrés :
/// 40,4 → 40,1 → 40,2 → 40,4 → 40,1 °C. Amplitude totale : 0,3 °C.
///
/// C'est le scénario qui dit si l'issue est résolue.
const RELEVE_DU_2026_08_15: [i32; 5] = [40_400, 40_100, 40_200, 40_400, 40_100];

/// Le point de travail de la courbe témoin, en degrés entiers : 40 °C, donc 40 %.
const PALIER_TEMOIN: i32 = 40;

/// Une heure de tours, à un tour par seconde — la cadence de la boucle du démon.
const TOURS_D_UNE_HEURE: u32 = 3_600;

/// L'amplitude, en points de consigne, de la dérive lente de la section B.
const AMPLITUDE_DE_LA_DERIVE: i32 = 19;

// ---------------------------------------------------------------------------
// Aides — les courbes
// ---------------------------------------------------------------------------

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

/// La courbe témoin de ce fichier : **un point de consigne par degré, exactement**.
///
/// `consigne(degres(n)) == n` pour tout `n` de 0 à 100 — vérifié par
/// `les_reperes_de_ce_fichier_ne_sont_aucun_defaut`, et non supposé.
///
/// ⚠️ Elle existe pour une raison précise : #99 laisse **l'arrondi de l'interpolation ouvert**
/// (« choisir entre troncature et arrondi au plus proche serait figer un détail que l'issue ne
/// tranche pas »). Un test qui dirait « ces deux températures donnent des consignes distantes d'un
/// point » sur la courbe par défaut dépendrait donc de ce choix. Ici, un écart de consigne s'écrit
/// en degrés et se lit sans arrondi. La courbe par défaut, elle, garde les scénarios qui viennent
/// du relevé réel, où c'est justement elle qui est en cause.
fn courbe_temoin() -> Courbe {
    courbe(&[(degres(0), BORNE_BASSE), (degres(100), BORNE_HAUTE)])
}

/// La consigne que `courbe_temoin` rend à `n` degrés : `n` lui-même.
const fn consigne_a(degres_entiers: i32) -> u8 {
    degres_entiers as u8
}

/// Une régulation sur cette courbe, avec ces canaux activés.
fn regulation_sur(courbe: Courbe, canaux: &[&str]) -> Regulation {
    let mut regulation = Regulation::nouvelle(courbe);
    for canal in canaux {
        regulation.activer(canal);
    }
    regulation
}

// ---------------------------------------------------------------------------
// Le banc : un matériel qui porte ce qu'on lui écrit, ou qui n'en fait rien
// ---------------------------------------------------------------------------

/// Un matériel simulé, et le registre de ce qu'on lui a écrit.
///
/// C'est le banc de `spec_relecture_consigne.rs`, repris parce que #111 décide sur la même
/// grandeur que #110 : ce que le canal **porte**. Deux tables et un `Vec` — un canal peut appliquer
/// ce qu'on lui écrit, ou l'encaisser sans bouger.
struct Banc {
    /// Ce que chaque canal porte, tel que le démon vient de le relire.
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

    /// Le canal se met à porter cette valeur, sans que la régulation y soit pour rien : un
    /// `reverb fan`, un firmware qui reprend la main, ou le mécanisme inconnu de #110.
    fn porte_desormais(&mut self, canal: &str, valeur: u8) {
        self.portees.insert(canal.to_owned(), Some(valeur));
    }

    /// Un tour de télémétrie : la régulation lit ce que le matériel porte, dit quoi écrire, le banc
    /// le note — puis le matériel applique, ou non.
    ///
    /// Rend les écritures de **ce** tour, triées par canal : l'ordre dans lequel `tour` les rend
    /// n'est pas un contrat.
    fn tour(&mut self, regulation: &mut Regulation, liquide: Option<i32>) -> Vec<Ecriture> {
        let mut ce_tour = regulation.tour(liquide, &self.portees);
        ce_tour.sort_by(|gauche, droite| gauche.canal.cmp(&droite.canal));

        let mut vus: BTreeSet<&str> = BTreeSet::new();
        for ecriture in &ce_tour {
            assert!(
                vus.insert(ecriture.canal.as_str()),
                "« {} » est écrit deux fois dans le même tour : {ce_tour:?}",
                ecriture.canal
            );
        }

        self.faites.extend(ce_tour.iter().cloned());

        for ecriture in &ce_tour {
            if self.obeissants.contains(&ecriture.canal) {
                self.portees
                    .insert(ecriture.canal.clone(), Some(ecriture.consigne));
            }
        }

        ce_tour
    }

    /// Ce que le canal porte à cet instant, du point de vue de la relecture.
    fn porte(&self, canal: &str) -> u8 {
        self.portees
            .get(canal)
            .copied()
            .flatten()
            .unwrap_or_else(|| panic!("« {canal} » devrait être lisible dans ce scénario"))
    }

    /// Toutes les écritures depuis le début, tous tours confondus.
    fn tout(&self) -> &[Ecriture] {
        &self.faites
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

// ---------------------------------------------------------------------------
// 0 — les repères de ce fichier ne sont aucun défaut
// ---------------------------------------------------------------------------

#[test]
fn les_reperes_de_ce_fichier_ne_sont_aucun_defaut() {
    // Tout ce fichier repose sur trois faits : le seuil est dans la fourchette de l'issue, la
    // courbe témoin est exacte au degré près, et le relevé du 2026-08-15 tient dans une bande de
    // consignes plus étroite que le seuil. Si l'un des trois cède, plusieurs tests deviendraient
    // vrais sans rien vérifier. Ce test est là pour que la panne soit ici.
    assert!(
        (SEUIL_MINIMAL..=SEUIL_MAXIMAL).contains(&HYSTERESIS),
        "le seuil vaut {HYSTERESIS} point(s), hors de la fourchette de l'issue \
         ({SEUIL_MINIMAL} à {SEUIL_MAXIMAL}). Sous {SEUIL_MINIMAL}, la règle est celle de #110 \
         inchangée et le bruit de ±1 point continue de passer ; au-dessus de {SEUIL_MAXIMAL}, \
         l'hystérésis retarde une vraie montée"
    );

    // La courbe témoin rend exactement `n` % à `n` °C, sur toute son étendue. C'est ce qui permet
    // d'écrire un écart de consigne en degrés sans dépendre d'un arrondi que #99 laisse ouvert.
    let temoin = courbe_temoin();
    for degres_entiers in 0..=100 {
        assert_eq!(
            temoin.consigne(degres(degres_entiers)),
            consigne_a(degres_entiers),
            "la courbe témoin doit rendre {degres_entiers} % à {degres_entiers} °C, sinon tous les \
             écarts de ce fichier sont faux d'un arrondi"
        );
    }

    // Le relevé du 2026-08-15 tient dans une bande de consignes strictement plus étroite que le
    // seuil. C'est la propriété qui rend le scénario réel silencieux, et elle se mesure sur la
    // courbe par défaut — celle qui tournait ce jour-là.
    let defaut = Courbe::defaut();
    let bande: Vec<u8> = RELEVE_DU_2026_08_15
        .iter()
        .map(|liquide| defaut.consigne(*liquide))
        .collect();
    let plus_basse = *bande.iter().min().expect("le relevé n'est pas vide");
    let plus_haute = *bande.iter().max().expect("le relevé n'est pas vide");
    assert!(
        plus_haute - plus_basse < HYSTERESIS,
        "les consignes du relevé du 2026-08-15 vont de {plus_basse} % à {plus_haute} %, soit une \
         bande de {} point(s) : elle doit rester sous le seuil de {HYSTERESIS}, sinon le scénario \
         réel ne prouve rien",
        plus_haute - plus_basse
    );
    assert!(
        plus_basse > DUTY_ALLUMAGE,
        "les consignes du relevé doivent dépasser le duty d'allumage ({DUTY_ALLUMAGE} %), sinon un \
         canal bloqué porterait déjà la bonne valeur et « il est réécrit » passerait pour la pire \
         des raisons"
    );

    // ⚠️ Le repli ne doit être ni l'une ni l'autre des bornes : l'exemption des bornes masquerait
    // alors celle du repli, et `le_repli_d_une_sonde_muette_part_meme_sous_le_seuil` vérifierait la
    // mauvaise des deux.
    assert!(
        REPLI != BORNE_BASSE && REPLI != BORNE_HAUTE,
        "le repli vaut {REPLI} %, qui est une borne : les exemptions 2 et 3 ne seraient plus \
         séparables par un test"
    );
    assert!(
        REPLI > HYSTERESIS && REPLI < BORNE_HAUTE - HYSTERESIS,
        "le repli ({REPLI} %) doit laisser la place à un écart de {HYSTERESIS} point(s) de part et \
         d'autre, sans quoi les scénarios de repli sortiraient de 0–100"
    );

    // Et les canaux sont bien trois canaux distincts.
    let canaux = [CANAL_1, CANAL_2, CANAL_3];
    for (rang, canal) in canaux.iter().enumerate() {
        assert!(!canal.is_empty(), "un canal doit porter un nom");
        for autre in canaux.iter().skip(rang + 1) {
            assert_ne!(canal, autre, "les canaux doivent être distincts");
        }
    }
}

// ---------------------------------------------------------------------------
// A — le bruit se tait
// ---------------------------------------------------------------------------

#[test]
fn deux_tours_dont_les_consignes_different_d_un_point_ne_produisent_qu_une_ecriture() {
    // Test d'intention de l'issue, et le plus petit énoncé du correctif : « deux tours dont les
    // consignes diffèrent d'un point ne produisent qu'une écriture ».
    //
    // Un point de consigne, c'est exactement ce que 0,3 °C de bruit produit sur le segment à
    // 3 %/°C — le pas de quantification de la mesure, pas une information sur le circuit.
    let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN)))),
        ecritures(&[(CANAL_1, consigne_a(PALIER_TEMOIN))]),
        "premier tour : le canal porte son duty d'allumage et n'a jamais rien reçu, la consigne part"
    );

    let second = banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN + 1)));
    assert!(
        second.is_empty(),
        "la consigne passe de {} % à {} %, un seul point : rien ne doit partir, or {second:?}",
        consigne_a(PALIER_TEMOIN),
        consigne_a(PALIER_TEMOIN + 1)
    );
    assert_eq!(
        banc.tout().len(),
        1,
        "sur les deux tours, une seule écriture : {:?}",
        banc.tout()
    );
}

#[test]
fn deux_tours_dont_les_consignes_different_du_seuil_en_produisent_deux() {
    // Test d'intention de l'issue : « deux tours dont les consignes diffèrent au-delà du seuil en
    // produisent deux ».
    //
    // Le pendant du test précédent, et il compte autant : une hystérésis qui ne laisserait plus
    // rien passer serait une régulation débranchée, ce qui est le défaut de #99 obtenu par l'autre
    // bout. L'écart choisi est **exactement** le seuil, parce que c'est là que la règle se décide :
    // le seuil est atteint, pas dépassé.
    let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    let franc = PALIER_TEMOIN + HYSTERESIS as i32;

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN)))),
        ecritures(&[(CANAL_1, consigne_a(PALIER_TEMOIN))]),
        "premier tour : la consigne part"
    );
    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(degres(franc)))),
        ecritures(&[(CANAL_1, consigne_a(franc))]),
        "la consigne s'écarte de {HYSTERESIS} point(s), soit exactement le seuil : elle doit \
         partir. Un seuil « dépassé » plutôt qu'« atteint » laisserait le bruit de ±1 point \
         inchangé et n'aurait fait taire que lui — ce que le premier test de cette section vérifie \
         déjà"
    );
    assert_eq!(
        banc.tout().len(),
        2,
        "deux tours, deux écritures : {:?}",
        banc.tout()
    );
}

#[test]
fn une_suite_de_tours_bruitant_autour_d_une_valeur_ne_produit_qu_une_ecriture() {
    // Test d'intention de l'issue : « une suite de tours bruitant autour d'une valeur ne produit
    // qu'une écriture ».
    //
    // La bande de bruit est large de `HYSTERESIS - 1` points, la largeur du relevé réel rapportée
    // au seuil. Une bande de cette largeur est silencieuse **quel que soit** le point où le premier
    // tour l'attrape — d'où les deux passages, l'un qui l'attrape par le bas, l'autre par le haut.
    //
    // ⚠️ Les deux passages ne sont pas une redondance : ils couvrent les deux sens du bruit. Le
    // premier ne voit ensuite que des consignes plus hautes que celle qu'il a écrite, le second que
    // des plus basses. Une implémentation qui comparerait sans valeur absolue passerait l'un et
    // échouerait l'autre — et une erreur de signe ne produit aucun message, juste un ventilateur
    // qui ne redescend jamais.
    const TOURS: u32 = 200;

    let largeur = HYSTERESIS as i32;

    for (nom, depuis_le_bas) in [("par le bas", true), ("par le haut", false)] {
        let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
        let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);
        let mut premiere: Option<u8> = None;

        for numero in 0..TOURS {
            let rang = numero as i32 % largeur;
            let ecart = if depuis_le_bas {
                rang
            } else {
                largeur - 1 - rang
            };
            let temperature = degres(PALIER_TEMOIN + ecart);
            let faites = banc.tour(&mut regulation, Some(temperature));

            match premiere {
                None => {
                    assert_eq!(
                        sans_motif(&faites),
                        ecritures(&[(CANAL_1, consigne_a(PALIER_TEMOIN + ecart))]),
                        "{nom} : le tout premier tour écrit, le canal n'a encore rien reçu"
                    );
                    premiere = Some(consigne_a(PALIER_TEMOIN + ecart));
                }
                Some(posee) => assert!(
                    faites.is_empty(),
                    "{nom}, tour n° {numero} : la consigne bruite autour de {posee} % sans jamais \
                     s'en écarter de {HYSTERESIS} point(s), rien ne doit partir, or {faites:?}"
                ),
            }
        }

        assert_eq!(
            banc.tout().len(),
            1,
            "{nom} : sur {TOURS} tours de bruit, une seule écriture — celle de la prise en charge. \
             Obtenu : {:?}",
            banc.tout()
        );
    }
}

#[test]
fn le_releve_du_2026_08_15_ne_produit_plus_qu_une_ecriture_par_canal() {
    // ⚠️ **Le test qui dit si l'issue est résolue.** Ce n'est pas une bande de bruit construite pour
    // l'occasion : ce sont les cinq lectures du liquide relevées à 15:51 sur SHYNAEL, sur la courbe
    // par défaut, sur les trois canaux qui les ont subies.
    //
    // Ce que #99 faisait : trente écritures en huit minutes.
    // Ce que #111 exige : trois, celles de la prise en charge, puis le silence.
    const CYCLES: u32 = 100;

    let canaux = [CANAL_1, CANAL_2, CANAL_3];
    let mut regulation = regulation_sur(Courbe::defaut(), &canaux);
    let mut banc = Banc::neuf()
        .obeissant(CANAL_1, DUTY_ALLUMAGE)
        .obeissant(CANAL_2, DUTY_ALLUMAGE)
        .obeissant(CANAL_3, DUTY_ALLUMAGE);

    // Le relevé rejoué en boucle : 40,4 → 40,1 → 40,2 → 40,4 → 40,1 → 40,4 → … Huit minutes de
    // machine au repos, et bien au-delà.
    let premiere = Courbe::defaut().consigne(RELEVE_DU_2026_08_15[0]);
    let mut prise_en_charge_faite = false;

    for cycle in 0..CYCLES {
        for (rang, liquide) in RELEVE_DU_2026_08_15.iter().enumerate() {
            let faites = banc.tour(&mut regulation, Some(*liquide));
            if prise_en_charge_faite {
                assert!(
                    faites.is_empty(),
                    "cycle n° {cycle}, lecture n° {rang} ({liquide} m°C) : 0,3 °C d'amplitude ne \
                     sont pas une information sur le circuit, rien ne doit partir, or {faites:?}"
                );
            } else {
                assert_eq!(
                    sans_motif(&faites),
                    ecritures(&[
                        (CANAL_1, premiere),
                        (CANAL_2, premiere),
                        (CANAL_3, premiere),
                    ]),
                    "15:51:10, liquide à 40,4 °C : les trois canaux portent leur duty d'allumage \
                     et reçoivent la consigne"
                );
                prise_en_charge_faite = true;
            }
        }
    }

    assert_eq!(
        banc.tout().len(),
        canaux.len(),
        "une écriture par canal, et pas une de plus sur {} tours : {:?}",
        CYCLES as usize * RELEVE_DU_2026_08_15.len(),
        banc.tout()
    );
}

#[test]
fn une_heure_a_temperature_bruitante_ne_produit_aucune_ecriture_apres_la_premiere() {
    // Critère d'acceptation : « le nombre d'écritures sur une heure à température stable est nul ».
    //
    // Une heure de démon, à un tour par seconde, sur le bruit réel du 2026-08-15 : la seule
    // écriture est celle de la prise en charge du canal. Le chiffre que l'issue oppose est
    // ~3 500 écritures par jour et par canal.
    let mut regulation = regulation_sur(Courbe::defaut(), &[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    for numero in 0..TOURS_D_UNE_HEURE {
        let liquide = RELEVE_DU_2026_08_15[numero as usize % RELEVE_DU_2026_08_15.len()];
        let faites = banc.tour(&mut regulation, Some(liquide));
        if numero > 0 {
            assert!(
                faites.is_empty(),
                "tour n° {numero} sur {TOURS_D_UNE_HEURE} : la température bruite sans bouger, \
                 rien ne doit partir, or {faites:?}"
            );
        }
    }

    assert_eq!(
        banc.tout().len(),
        1,
        "une heure de tours, une seule écriture — celle du premier tour : {:?}",
        banc.tout()
    );
}

// ---------------------------------------------------------------------------
// B — rien n'est retardé, et rien ne stagne
// ---------------------------------------------------------------------------

#[test]
fn une_montee_franche_produit_une_ecriture_au_tour_qui_la_voit() {
    // Test d'intention de l'issue : « une montée franche produit une écriture au premier tour qui
    // la voit ».
    //
    // ⚠️ L'exigence porte sur **ce tour-là**, pas sur le suivant. Une implémentation qui retiendrait
    // la première écriture pour « confirmer » la montée au tour d'après passerait un test qui se
    // contenterait de compter, et ajouterait une seconde de retard à chaque changement de régime.
    const MONTEE: i32 = 20;

    let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN)));
    assert_eq!(
        banc.porte(CANAL_1),
        consigne_a(PALIER_TEMOIN),
        "le canal part de la consigne tiède"
    );

    let franche = banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN + MONTEE)));
    assert_eq!(
        sans_motif(&franche),
        ecritures(&[(CANAL_1, consigne_a(PALIER_TEMOIN + MONTEE))]),
        "le liquide gagne {MONTEE} °C d'un coup : l'écriture part au tour qui la voit, pas au \
         suivant"
    );
    assert_eq!(
        banc.porte(CANAL_1),
        consigne_a(PALIER_TEMOIN + MONTEE),
        "et le canal porte la nouvelle consigne dès la fin de ce tour"
    );
}

#[test]
fn une_montee_continue_ne_laisse_jamais_le_canal_deriver_plus_que_le_seuil() {
    // Critère d'acceptation : « une vraie variation de température produit une écriture sans retard
    // notable », et l'avertissement de l'issue : « l'hystérésis ne doit pas retarder une vraie
    // montée. Un seuil trop large laisserait le liquide gagner plusieurs degrés avant de réagir. »
    //
    // Le « retard notable » se mesure ici en points de consigne, et l'énoncé est le seul qui borne
    // vraiment quelque chose : **après chaque tour, ce que le canal porte est à moins de
    // `HYSTERESIS` points de ce que la courbe demande**. Sur le segment le plus doux de la courbe
    // par défaut, deux points valent 0,7 °C de liquide.
    let temoin = courbe_temoin();
    let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    let depart = degres(30);
    let arrivee = degres(60);

    for temperature in (depart..=arrivee).step_by(250) {
        let demandee = temoin.consigne(temperature);
        banc.tour(&mut regulation, Some(temperature));
        let porte = banc.porte(CANAL_1);
        assert!(
            porte.abs_diff(demandee) < HYSTERESIS,
            "à {temperature} m°C la courbe demande {demandee} % et le canal porte {porte} % : \
             l'écart doit rester sous {HYSTERESIS} point(s) après chaque tour, sinon l'hystérésis \
             retarde la montée au lieu de faire taire le bruit"
        );
    }

    // Et elle a bien écrit régulièrement pendant tout ce temps : la montée vaut
    // `consigne(arrivee) - consigne(depart)` points, donc au moins un pas de seuil par tranche.
    let gagnes = temoin.consigne(arrivee) - temoin.consigne(depart);
    let plancher = 1 + gagnes as usize / HYSTERESIS as usize;
    let recues = banc.consignes_de(CANAL_1);
    assert!(
        recues.len() >= plancher,
        "la consigne a gagné {gagnes} points : il faut au moins {plancher} écritures pour la \
         suivre par pas de {HYSTERESIS}, or il y en a {} — {recues:?}",
        recues.len()
    );
}

#[test]
fn une_derive_lente_d_un_point_par_tour_finit_toujours_par_ecrire() {
    // ⚠️ **Le piège classique de l'hystérésis, et le seul endroit où un seuil mal posé stagne
    // indéfiniment.** Chaque tour n'apporte qu'un point, c'est-à-dire toujours moins que le seuil ;
    // une implémentation qui comparerait la consigne du tour à celle du tour **précédent** ne
    // partirait donc jamais, et le canal dériverait sans fin pendant que chaque pas, pris seul,
    // paraîtrait négligeable.
    //
    // La règle qui l'évite est celle de la section 1 de l'en-tête : l'écart se mesure contre ce que
    // le canal **porte**, donc il s'accumule tant qu'on n'écrit pas.
    //
    // Les deux sens sont joués : un ventilateur qui ne redescendrait jamais est aussi un défaut,
    // et une comparaison signée passerait la montée et raterait la descente sans un mot.
    let temoin = courbe_temoin();

    for (nom, sens) in [("la montée", 1i32), ("la descente", -1i32)] {
        let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
        let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

        let depart = if sens > 0 {
            PALIER_TEMOIN
        } else {
            PALIER_TEMOIN + AMPLITUDE_DE_LA_DERIVE
        };

        for pas in 0..=AMPLITUDE_DE_LA_DERIVE {
            let temperature = degres(depart + sens * pas);
            let demandee = temoin.consigne(temperature);
            banc.tour(&mut regulation, Some(temperature));
            let porte = banc.porte(CANAL_1);
            assert!(
                porte.abs_diff(demandee) < HYSTERESIS,
                "{nom}, pas n° {pas} : la courbe demande {demandee} % et le canal porte {porte} %. \
                 Un point par tour ne dépasse jamais le seuil pris isolément — c'est l'écart avec \
                 ce que le canal porte qui doit s'accumuler, sans quoi la dérive est sans fin"
            );
        }

        let plancher = 1 + AMPLITUDE_DE_LA_DERIVE as usize / HYSTERESIS as usize;
        let recues = banc.consignes_de(CANAL_1);
        assert!(
            recues.len() >= plancher,
            "{nom} : sur {AMPLITUDE_DE_LA_DERIVE} points de dérive, il faut au moins {plancher} \
             écritures — une par pas de {HYSTERESIS} —, or il y en a {} : {recues:?}",
            recues.len()
        );
    }
}

// ---------------------------------------------------------------------------
// C — les trois exemptions, et rien de plus
// ---------------------------------------------------------------------------

#[test]
fn un_canal_jamais_ecrit_depuis_son_activation_est_ecrit_meme_sous_le_seuil() {
    // Exemption n° 1, déjà posée par #99 (`un_canal_coupe_puis_repris_est_reecrit`) et #110
    // (« écrire si le canal est lisible **et** (jamais écrit depuis son activation **ou** ce qu'il
    // porte diffère) ») : #111 restreint la seconde branche de ce « ou », jamais la première.
    //
    // ⚠️ Le cas testé ici est celui que ni #99 ni #110 ne couvrent : un canal tout juste activé qui
    // porte déjà la consigne **à un point près**. Une implémentation qui appliquerait le seuil
    // avant de regarder si le canal a déjà reçu quelque chose le laisserait à sa valeur — et,
    // « aucune persistance matérielle » oblige, cette valeur est celle du duty d'allumage au
    // redémarrage suivant.
    let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, consigne_a(PALIER_TEMOIN - 1));

    let premier = banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN)));
    assert_eq!(
        sans_motif(&premier),
        ecritures(&[(CANAL_1, consigne_a(PALIER_TEMOIN))]),
        "le canal porte {} % pour une consigne de {} %, un seul point d'écart — mais il n'a jamais \
         rien reçu depuis son activation, donc il est écrit",
        consigne_a(PALIER_TEMOIN - 1),
        consigne_a(PALIER_TEMOIN)
    );
    assert_eq!(
        banc.motifs_de(CANAL_1),
        vec![Motif::Consigne],
        "et c'est une consigne neuve, pas une réémission : c'est la trace de « le démon a pris ce \
         canal »"
    );

    // Et une fois pris en charge, il rejoint le régime commun : le seuil s'applique.
    let ensuite = banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN + 1)));
    assert!(
        ensuite.is_empty(),
        "le canal est désormais pris en charge : un point d'écart ne le fait plus bouger, or \
         {ensuite:?}"
    );

    // La reprise après une coupure remet le compteur à zéro, comme #99 l'exige : « un canal coupé
    // oublie ce qu'il avait reçu ».
    regulation.couper(CANAL_1);
    regulation.activer(CANAL_1);
    let repris = banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN + 1)));
    assert_eq!(
        sans_motif(&repris),
        ecritures(&[(CANAL_1, consigne_a(PALIER_TEMOIN + 1))]),
        "coupé puis repris, le canal n'a plus rien reçu **depuis son activation** : il est écrit \
         même à un point d'écart, parce que la régulation ne sait pas ce qui lui est arrivé pendant \
         qu'elle ne le tenait plus"
    );
}

#[test]
fn une_consigne_a_cent_pour_cent_part_meme_a_un_point_pres() {
    // Exemption n° 2, moitié haute. Critère d'acceptation : « le franchissement d'une borne de la
    // courbe (0 % ou 100 %) est toujours appliqué ».
    //
    // ⚠️ C'est le sens même de la borne : « à fond » ne doit pas s'arrêter à 99. Un canal laissé à
    // 99 % au moment où la courbe réclame le plein régime, c'est un point de duty perdu au pire
    // moment — celui où le liquide a dépassé le dernier palier —, et surtout une promesse fausse :
    // le tableau du README annonce 100 % au-delà de 50 °C.
    let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    let juste_en_dessous = BORNE_HAUTE as i32 - 1;

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(degres(juste_en_dessous)))),
        ecritures(&[(CANAL_1, consigne_a(juste_en_dessous))]),
        "le canal est pris en charge à {} %",
        consigne_a(juste_en_dessous)
    );

    let plein = banc.tour(&mut regulation, Some(degres(BORNE_HAUTE as i32)));
    assert_eq!(
        sans_motif(&plein),
        ecritures(&[(CANAL_1, BORNE_HAUTE)]),
        "la consigne atteint la borne haute pour un seul point d'écart : elle part quand même. \
         Sans cette exemption, le plein régime ne serait jamais atteint autrement que par une \
         montée de {HYSTERESIS} points d'un coup"
    );
    assert_eq!(
        banc.porte(CANAL_1),
        BORNE_HAUTE,
        "et le canal est bel et bien à fond"
    );
}

#[test]
fn une_consigne_a_zero_part_meme_a_un_point_pres() {
    // Exemption n° 2, moitié basse. La borne de repos est une borne comme l'autre : une courbe
    // silencieuse au repos — « un réglage que rien dans l'issue n'écarte » (#99) — doit pouvoir
    // arriver à zéro, et non s'immobiliser à 1 % pour toujours.
    let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    let juste_au_dessus = BORNE_BASSE as i32 + 1;

    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(degres(juste_au_dessus)))),
        ecritures(&[(CANAL_1, consigne_a(juste_au_dessus))]),
        "le canal est pris en charge à {} %",
        consigne_a(juste_au_dessus)
    );

    let arret = banc.tour(&mut regulation, Some(degres(BORNE_BASSE as i32)));
    assert_eq!(
        sans_motif(&arret),
        ecritures(&[(CANAL_1, BORNE_BASSE)]),
        "la consigne atteint la borne basse pour un seul point d'écart : elle part quand même"
    );
    assert_eq!(
        banc.porte(CANAL_1),
        BORNE_BASSE,
        "et le canal est bel et bien à l'arrêt"
    );
}

#[test]
fn une_consigne_qui_n_est_pas_une_borne_reste_soumise_au_seuil() {
    // Le contrôle des deux tests précédents, et il compte autant qu'eux : l'exemption porte sur la
    // **borne visée**, pas sur « un point d'écart, ça part ». Sans ce test, une implémentation qui
    // aurait tout bonnement oublié le seuil passerait les deux.
    //
    // Deux paires, et la seconde est celle qui tranche le sens de l'exemption : **quitter** le
    // plein régime pour 99 % est une consigne ordinaire, donc soumise au seuil. Rester une seconde
    // de plus à fond ne coûte rien ; ne jamais y arriver, si.
    let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    // Une paire au milieu de l'échelle, loin des deux bornes.
    let milieu = BORNE_HAUTE as i32 / 2;
    banc.tour(&mut regulation, Some(degres(milieu)));
    let voisin = banc.tour(&mut regulation, Some(degres(milieu + 1)));
    assert!(
        voisin.is_empty(),
        "{} % puis {} % : un point d'écart au milieu de l'échelle ne fait rien partir, or \
         {voisin:?}",
        consigne_a(milieu),
        consigne_a(milieu + 1)
    );

    // Puis la sortie du plein régime : la consigne n'est plus une borne, le seuil s'applique.
    banc.tour(&mut regulation, Some(degres(BORNE_HAUTE as i32)));
    assert_eq!(
        banc.porte(CANAL_1),
        BORNE_HAUTE,
        "le canal est à fond avant la seconde paire"
    );
    let redescente = banc.tour(&mut regulation, Some(degres(BORNE_HAUTE as i32 - 1)));
    assert!(
        redescente.is_empty(),
        "la consigne quitte la borne pour {} % : ce n'est plus une borne, donc le seuil s'applique \
         et rien ne part. Or {redescente:?}",
        BORNE_HAUTE - 1
    );
}

#[test]
fn le_repli_d_une_sonde_muette_part_meme_sous_le_seuil() {
    // Exemption n° 3. Critère d'acceptation : « le repli d'une sonde muette n'est pas soumis à
    // l'hystérésis — il part quoi qu'il arrive ».
    //
    // ⚠️ C'est **la seule écriture du système qui signale une panne**. Le liquide illisible veut
    // dire que le Kraken est en difficulté, donc que plus rien ne mesure la température du circuit ;
    // amortir cette écriture-là la rendrait indistincte d'un régime normal, et le repli n'aurait
    // plus rien d'un signal. Le README le pose déjà pour la valeur : « jamais à la dernière valeur
    // connue ». #111 le pose pour l'instant où elle part.
    //
    // Les deux côtés sont joués : le canal peut porter un point de trop comme un point de trop peu.
    for (nom, depart) in [
        ("un point sous le repli", REPLI - 1),
        ("un point au-dessus", REPLI + 1),
    ] {
        let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
        let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

        assert_eq!(
            sans_motif(&banc.tour(&mut regulation, Some(degres(depart as i32)))),
            ecritures(&[(CANAL_1, depart)]),
            "{nom} : le canal est pris en charge à {depart} %"
        );

        let repli = banc.tour(&mut regulation, None);
        assert_eq!(
            sans_motif(&repli),
            ecritures(&[(CANAL_1, REPLI)]),
            "{nom} : la sonde se tait, et le repli part malgré un écart de un seul point — sous le \
             seuil de {HYSTERESIS}"
        );

        // Mais l'exemption ne devient pas « on écrit à chaque tour » : le canal porte désormais le
        // repli, il n'y a plus rien à écrire. C'est la règle de #110
        // (`un_canal_qui_porte_deja_le_repli_ne_recoit_rien_quand_la_sonde_se_tait`), que cette
        // exemption ne doit pas défaire.
        for tour in 2..=10u32 {
            let ensuite = banc.tour(&mut regulation, None);
            assert!(
                ensuite.is_empty(),
                "{nom}, tour n° {tour} : le repli est posé et il a pris, rien ne doit repartir, or \
                 {ensuite:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// D — #110 n'est pas défait : un canal bloqué reste visible
// ---------------------------------------------------------------------------

#[test]
fn un_canal_bloque_reste_reecrit_a_chaque_tour_malgre_le_bruit() {
    // ⚠️ **Le seul risque sérieux de cette issue.** #110 existe parce qu'un canal a encaissé une
    // écriture sans bouger — `fan-3` à 25 % pendant que le journal annonçait 50 % — et sa réparation
    // tient à une réémission par tour. Une hystérésis mal posée la ferait taire, et le canal bloqué
    // redeviendrait invisible : aucune erreur, aucun message, un journal qui dit exactement ce qu'on
    // veut lire.
    //
    // Le scénario superpose les deux : un canal sourd **et** le bruit du 2026-08-15. Sous la règle
    // retenue, l'écart entre 25 % et une consigne de 45 ou 46 vaut vingt points — il ne passe jamais
    // sous le seuil, et la réémission continue.
    const TOURS: u32 = 60;

    let mut regulation = regulation_sur(Courbe::defaut(), &[CANAL_3]);
    let mut banc = Banc::neuf().sourd(CANAL_3, DUTY_ALLUMAGE);

    for numero in 0..TOURS {
        let liquide = RELEVE_DU_2026_08_15[numero as usize % RELEVE_DU_2026_08_15.len()];
        let faites = banc.tour(&mut regulation, Some(liquide));
        assert_eq!(
            faites.len(),
            1,
            "tour n° {numero} : « {CANAL_3} » porte toujours {DUTY_ALLUMAGE} % pour une consigne \
             d'environ {} %, et le bruit n'y change rien — l'écriture doit repartir, or {faites:?}",
            Courbe::defaut().consigne(liquide)
        );
    }

    assert_eq!(
        banc.consignes_de(CANAL_3).len(),
        TOURS as usize,
        "un canal qui n'applique jamais reçoit une écriture par tour, bruit compris : c'est la \
         réparation automatique de #110, et l'hystérésis ne doit pas l'éteindre"
    );
}

#[test]
fn le_seuil_porte_sur_ce_que_le_canal_porte_et_non_sur_la_derniere_consigne() {
    // ⚠️ **Le test qui sépare les deux lectures possibles de l'issue**, et la raison du choix
    // expliqué en tête de fichier.
    //
    // La consigne ne bouge pas d'un millidegré pendant tout ce test. C'est le **canal** qui s'écarte
    // — un `reverb fan` posé à la main, un firmware qui reprend la main, ou le mécanisme inconnu de
    // #110. Une hystérésis mesurée sur `|consigne − dernière écrite|` verrait un écart nul et se
    // tairait pour toujours ; mesurée sur `|porté − consigne|`, elle repart dès que le canal
    // s'éloigne du seuil.
    let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    let posee = consigne_a(PALIER_TEMOIN);
    assert_eq!(
        sans_motif(&banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN)))),
        ecritures(&[(CANAL_1, posee)]),
        "le canal est pris en charge à {posee} %"
    );

    // Le canal s'écarte tout seul d'exactement le seuil.
    banc.porte_desormais(CANAL_1, posee - HYSTERESIS);
    let rejoue = banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN)));
    assert_eq!(
        sans_motif(&rejoue),
        ecritures(&[(CANAL_1, posee)]),
        "le canal porte {} % pour une consigne inchangée de {posee} % : l'écart vaut le seuil, \
         l'écriture repart. Une hystérésis mesurée sur la dernière consigne calculée ne verrait \
         rien bouger et laisserait le canal là où il est",
        posee - HYSTERESIS
    );
    assert_eq!(
        banc.motifs_de(CANAL_1).last().copied(),
        Some(Motif::NonAppliquee {
            porte: Some(posee - HYSTERESIS)
        }),
        "et c'est une réémission, qui rapporte ce que le canal portait — le motif de #110, \
         inchangé"
    );

    // Puis il ne s'écarte que d'un point : sous le seuil, on le laisse.
    banc.porte_desormais(CANAL_1, posee - 1);
    let sous_le_seuil = banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN)));
    assert!(
        sous_le_seuil.is_empty(),
        "un point d'écart relu ne se distingue pas d'un point de bruit — les deux produisent \
         littéralement la même lecture. C'est la contrepartie assumée de #111, et elle vaut un \
         point de duty sur 255. Or {sous_le_seuil:?}"
    );
}

// ---------------------------------------------------------------------------
// E — le motif : une écriture retenue est une écriture qui n'a pas lieu
// ---------------------------------------------------------------------------

#[test]
fn l_hysteresis_ne_cree_aucun_motif_nouveau() {
    // ⚠️ Une écriture retenue par l'hystérésis n'est pas une écriture d'un autre genre : c'est une
    // écriture **qui n'a pas lieu**. `tour` rend un vecteur vide, exactement comme pour une consigne
    // inchangée, et le démon n'a rien de neuf à journaliser — remplacer 86 400 trames par 86 400
    // lignes de « je n'ai rien écrit » serait le même défaut déplacé d'un cran.
    //
    // Le `match` ci-dessous n'a **pas de joker**, et c'est tout son intérêt : il cesse de compiler
    // le jour où `Motif` gagne une troisième variante.
    //
    // ⚠️ La consigne ne bouge pas jusqu'à la toute dernière ligne, et ce n'est pas un
    // hasard : #111 rend observable une question que #110 n'avait pas à trancher — le cache
    // retient-il la dernière consigne **écrite** ou la dernière **calculée** ? Un tour retenu les
    // sépare. Ce test ne la tranche pas, il l'évite ; le tour de bruit est joué en dernier, où il
    // n'influe sur aucun motif.
    let mut regulation = regulation_sur(courbe_temoin(), &[CANAL_1]);
    let mut banc = Banc::neuf().obeissant(CANAL_1, DUTY_ALLUMAGE);

    // Prise en charge.
    banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN)));

    // Le canal s'écarte d'un point : sous le seuil, rien ne part.
    banc.porte_desormais(CANAL_1, consigne_a(PALIER_TEMOIN) - 1);
    let retenu = banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN)));
    assert!(
        retenu.is_empty(),
        "un tour retenu par l'hystérésis ne rend aucune écriture — pas une écriture d'un genre \
         particulier. Or {retenu:?}"
    );

    // Il s'écarte du seuil : la réémission de #110 repart.
    banc.porte_desormais(CANAL_1, consigne_a(PALIER_TEMOIN) - HYSTERESIS);
    banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN)));

    // Et un tour de bruit, en dernier, ne produit rien non plus.
    let bruit = banc.tour(&mut regulation, Some(degres(PALIER_TEMOIN + 1)));
    assert!(
        bruit.is_empty(),
        "un tour où c'est la consigne qui bruite ne rend rien non plus : {bruit:?}"
    );

    let motifs = banc.motifs_de(CANAL_1);
    assert_eq!(
        motifs.len(),
        2,
        "deux écritures attendues sur ce scénario — la prise en charge et la réémission —, or \
         {motifs:?}"
    );

    let mut consignes = 0usize;
    let mut reemissions = 0usize;
    for motif in &motifs {
        match motif {
            Motif::Consigne => consignes += 1,
            Motif::NonAppliquee { porte } => {
                assert!(
                    porte.is_some(),
                    "une réémission ne peut naître que d'une relecture réussie : {motif:?}"
                );
                reemissions += 1;
            }
        }
    }
    assert_eq!(
        (consignes, reemissions),
        (1, 1),
        "les deux motifs de #110 suffisent à décrire ce que #111 laisse passer : {motifs:?}"
    );
}
