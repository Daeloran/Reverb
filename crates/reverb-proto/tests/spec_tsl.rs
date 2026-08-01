//! Tests d'intention du sélecteur de couleur — teinte, saturation, luminosité (issue #30).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #30 et les seules signatures publiques de
//! `color.rs` (`Tsl`, `TslInvalide`, `Rgb::en_tsl`, `Rgb::depuis_tsl` — corps `todo!()` à
//! l'écriture de ce fichier). Ils encodent ce que la conversion doit faire, pas ce que le code
//! fait : si l'un d'eux échoue après implémentation, c'est le code qu'on corrige, jamais le test.
//!
//! # Ce que ce fichier couvre, et ce qu'il ne couvre pas
//!
//! Il couvre la **conversion** seule : `reverb-proto` est pur, il ne connaît ni curseur ni champ
//! de saisie. Les critères de l'issue qui parlent de l'état de la fenêtre — « les trois curseurs
//! et le champ hexadécimal désignent toujours la même couleur », « une couleur grise garde sa
//! teinte quand on remonte la saturation depuis zéro », « le noir garde sa teinte et sa
//! saturation » — sont des critères de la **fenêtre**, qui garde un `Tsl` comme état ; ils se
//! testeront dans `reverb-gui`. Ce que la conversion en doit, et qui est testé ici, c'est leur
//! contrepartie : une saturation nulle rend un gris quelle que soit la teinte, une luminosité
//! nulle rend le noir quelles que soient la teinte et la saturation. Autrement dit, la conversion
//! ne doit jamais *forcer* la fenêtre à inventer une teinte.
//!
//! # L'aller-retour, dans le bon sens
//!
//! L'issue écrit « la conversion aller-retour RGB → TSL → RGB rend la couleur de départ, pour les
//! 16 777 216 ». C'est bien `Rgb::depuis_tsl(couleur.en_tsl()) == couleur` qui est exigé, et lui
//! seul. Le sens inverse — partir d'un `Tsl` quelconque et y revenir à l'identique — n'est **pas**
//! exigé et ne peut pas l'être : plusieurs `Tsl` désignent la même couleur (tous les `Tsl` de
//! luminosité nulle sont le noir, toutes les teintes d'un gris sont ce gris). C'est aussi pourquoi
//! le contrat prend des flottants : à l'entier près, 360 × 101 × 101 = 3 672 360 triplets ne
//! peuvent pas nommer 16 777 216 couleurs.
//!
//! # Les pièges visés
//!
//! 1. **L'arrondi.** Une troncature (celle que `with_brightness` assume délibérément, spec §11)
//!    casse l'aller-retour : le gris 1 repasse par 0,392 % puis retombe sur 0. C'est le piège que
//!    l'issue désigne elle-même comme « le seul qui compte vraiment ».
//! 2. **Les unités.** Teinte en degrés — pas en radians, pas en tours ; saturation et luminosité
//!    en **pourcents** — pas en fraction de 1. Une erreur d'unité produit du code qui tourne, des
//!    couleurs plausibles, et des curseurs qui ne correspondent à rien.
//! 3. **Le repli silencieux.** Une teinte hors bornes doit être **refusée en nommant le champ**,
//!    pour que la fenêtre sache quel curseur signaler. La repasser au modulo, ou l'écrêter,
//!    transforme une saisie fautive en couleur crédible et personne ne voit l'erreur.
//! 4. **`NaN`.** Un contrôle écrit `si t < 0 || t >= 360` laisse passer `NaN` : toute comparaison
//!    avec `NaN` est fausse. Un `NaN` qui traverse la conversion ressort en couleur arbitraire.
//! 5. **Les frontières de secteur.** Le tour se calcule par sixièmes ; un secteur décalé d'un rang
//!    ou une formule asymétrique donne des couleurs justes sur les six sommets et fausses entre
//!    eux.
//!
//! Aucun accès matériel : ces tests sont purement calculatoires.

use reverb_proto::{Rgb, Tsl, TslInvalide};

// ---------------------------------------------------------------------------
// Repères et outils communs
// ---------------------------------------------------------------------------

/// Tolérance d'affichage : « à une unité près » (issue #30, test d'intention n°1). C'est le pas
/// des valeurs chiffrées montrées à droite des curseurs.
const UNITE: f32 = 1.0;

/// Tolérance fine, pour les égalités que le contrat annonce exactes (les six sommets sur les
/// multiples de 60, la luminosité d'un gris). Un millième de pourcent ou de degré : mille fois
/// plus fin que le pas d'affichage, mais large devant le dernier bit d'un `f32` — on vérifie une
/// valeur, pas un ordre d'opérations.
const FIN: f32 = 1e-3;

/// Tolérance de retour de teinte pour une couleur pleinement saturée. À saturation et luminosité
/// pleines, un pas de composante vaut 60 / 255 ≈ 0,24° de teinte : un demi-degré laisse deux pas
/// de marge, tout en étant cent fois trop serré pour laisser passer un secteur décalé (60°).
const PAS_DE_TEINTE: f32 = 0.5;

/// Une couleur au plus profond du cube RGB, sans composante remarquable : elle ne tombe sur aucun
/// sommet, aucune arête, aucune frontière de secteur.
const QUELCONQUE: Rgb = Rgb::new(0x34, 0x8b, 0x5f);

/// Les six sommets colorés du cube et leur teinte (issue #30, test d'intention n°5).
const SOMMETS: [(&str, Rgb, f32); 6] = [
    ("rouge", Rgb::new(255, 0, 0), 0.0),
    ("jaune", Rgb::new(255, 255, 0), 60.0),
    ("vert", Rgb::new(0, 255, 0), 120.0),
    ("cyan", Rgb::new(0, 255, 255), 180.0),
    ("bleu", Rgb::new(0, 0, 255), 240.0),
    ("magenta", Rgb::new(255, 0, 255), 300.0),
];

/// La couleur dont le code sur 24 bits vaut `code`, rouge en poids fort.
fn couleur_du_code(code: u32) -> Rgb {
    Rgb::new((code >> 16) as u8, (code >> 8) as u8, code as u8)
}

/// La plus grande teinte représentable strictement sous 360°.
///
/// Le contrat écrit le tour `0..360`, borne haute **exclue** : cette valeur-là doit passer, 360
/// doit être refusé. On la construit par le bit de poids faible plutôt que par une décimale
/// écrite à la main, pour ne pas dépendre de la précision d'une constante recopiée.
fn juste_sous_360() -> f32 {
    f32::from_bits(360.0_f32.to_bits() - 1)
}

/// `depuis_tsl` doit rendre une couleur ; échoue en montrant le `Tsl` refusé.
fn couleur_de(tsl: Tsl) -> Rgb {
    Rgb::depuis_tsl(tsl)
        .unwrap_or_else(|e| panic!("{tsl:?} devait désigner une couleur, refusé : {e} ({e:?})"))
}

/// `depuis_tsl` doit refuser ; échoue en montrant la couleur produite à tort.
fn refus_de(tsl: Tsl) -> TslInvalide {
    match Rgb::depuis_tsl(tsl) {
        Err(e) => e,
        Ok(couleur) => panic!(
            "{tsl:?} est hors du contrat (teinte dans 0..360, saturation et luminosité dans \
             0..=100) et a pourtant produit {couleur:?} : une saisie fautive repliée en silence \
             est une saisie fautive que personne ne voit"
        ),
    }
}

/// Vérifie qu'un refus nomme bien le champ attendu, avec une raison à montrer.
fn assert_refus_sur(champ_attendu: &str, tsl: Tsl) {
    let erreur = refus_de(tsl);
    assert_eq!(
        erreur.champ, champ_attendu,
        "{tsl:?} est fautif sur « {champ_attendu} », l'erreur nomme « {} » : la fenêtre ne saurait \
         pas quel curseur signaler",
        erreur.champ
    );
    assert!(
        !erreur.raison.trim().is_empty(),
        "{tsl:?} est refusé sans raison à afficher : « refusé » sans le dire est aussi muet qu'un \
         repli silencieux"
    );
}

/// Écart absolu, en clair dans les messages d'échec.
fn ecart(a: f32, b: f32) -> f32 {
    (a - b).abs()
}

// ---------------------------------------------------------------------------
// 1 — l'exemple chiffré de l'issue
// ---------------------------------------------------------------------------

/// Issue #30, critère « taper `00aeed` place la teinte à 196, la saturation à 100 et la
/// luminosité à 93 » — et test d'intention n°1, « à une unité près ».
///
/// C'est le test qui attrape les erreurs d'unité : en radians la teinte vaudrait 3,4 ; en fraction
/// de 1 la saturation vaudrait 1 et la luminosité 0,93.
#[test]
fn le_bleu_de_l_issue_donne_196_100_93() {
    let couleur = Rgb::from_hex("00aeed").expect("« 00aeed » est une couleur hexadécimale valide");
    let tsl = couleur.en_tsl();

    assert!(
        ecart(tsl.teinte, 196.0) <= UNITE,
        "00aeed doit placer la teinte à 196° à une unité près, obtenu {}° (écart {}°) — une \
         teinte en radians vaudrait ~3,4, en tours ~0,54",
        tsl.teinte,
        ecart(tsl.teinte, 196.0)
    );
    assert!(
        ecart(tsl.saturation, 100.0) <= UNITE,
        "00aeed doit placer la saturation à 100 % à une unité près, obtenu {} % — une saturation \
         en fraction de 1 vaudrait 1",
        tsl.saturation
    );
    assert!(
        ecart(tsl.luminosite, 93.0) <= UNITE,
        "00aeed doit placer la luminosité à 93 % à une unité près, obtenu {} % — une luminosité \
         en fraction de 1 vaudrait 0,93 ; la moyenne (max + min) / 2 du modèle TSL « lightness » \
         vaudrait 46,5",
        tsl.luminosite
    );
}

/// Issue #30, critère « le champ accepte la forme avec et sans `#`, en majuscules comme en
/// minuscules » : les quatre écritures du même bleu doivent placer les curseurs au même endroit.
///
/// `from_hex` est déjà éprouvé par `spec.rs` ; ce qui est vérifié ici, c'est que la conversion en
/// aval ne réintroduit pas de différence entre deux `Rgb` égaux.
#[test]
fn les_quatre_ecritures_du_meme_hexadecimal_donnent_le_meme_tsl() {
    let reference = Rgb::from_hex("00aeed")
        .expect("forme minuscule sans dièse")
        .en_tsl();

    for ecriture in ["#00aeed", "00AEED", "#00AEED"] {
        let tsl = Rgb::from_hex(ecriture)
            .unwrap_or_else(|e| panic!("« {ecriture} » doit être acceptée : {e}"))
            .en_tsl();
        assert_eq!(
            tsl, reference,
            "« {ecriture} » place les curseurs sur {tsl:?} alors que « 00aeed » les place sur \
             {reference:?} : c'est la même couleur"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — l'aller-retour, le critère qui compte
// ---------------------------------------------------------------------------

/// Issue #30, critère « la conversion aller-retour RGB → TSL → RGB rend la couleur de départ,
/// pour les 16 777 216 » — et « le seul qui compte vraiment : c'est lui qui interdit l'arrondi
/// qui fait dériver une couleur à chaque aller-retour de l'interface ».
///
/// Test **exhaustif**, pas échantillonné : mesuré à ~1,4 s en profil `test` (donc sans
/// optimisation), il n'a pas besoin d'être mis de côté derrière `#[ignore]` — et un aller-retour
/// « exact sauf sur douze couleurs » n'est pas un aller-retour exact. C'est ce test qui attrape la
/// troncature, un arrondi de biais, et toute perte de précision dans le passage par `f32`.
///
/// Il attrape aussi, gratuitement, le cas où `en_tsl` produirait un `Tsl` que `depuis_tsl`
/// refuse : la composition doit être totale, sans quoi une couleur légitime rendrait la fenêtre
/// inerte.
#[test]
fn l_aller_retour_rend_la_couleur_de_depart_sur_les_16_777_216() {
    /// Nombre d'écarts détaillés dans le message d'échec : de quoi voir le motif (une composante ?
    /// un secteur ? les couleurs sombres ?) sans noyer la sortie.
    const ECHECS_MONTRES: usize = 8;

    let mut echecs = 0u32;
    let mut details = Vec::new();

    for code in 0..=0x00ff_ffffu32 {
        let couleur = couleur_du_code(code);
        let tsl = couleur.en_tsl();
        let retour = Rgb::depuis_tsl(tsl);
        if retour != Ok(couleur) {
            echecs += 1;
            if details.len() < ECHECS_MONTRES {
                details.push(format!("{couleur:?} → {tsl:?} → {retour:?}"));
            }
        }
    }

    assert_eq!(
        echecs,
        0,
        "l'aller-retour perd {echecs} couleurs sur 16 777 216 ({:.4} %) : une couleur choisie \
         dans la fenêtre dériverait à chaque passage par les curseurs. Premiers écarts :\n  {}",
        f64::from(echecs) * 100.0 / 16_777_216.0,
        details.join("\n  ")
    );
}

/// Corollaire du contrat, énoncé à part parce qu'il se casse tout seul : `en_tsl` ne doit jamais
/// sortir des bornes qu'il annonce — teinte dans `0..360` (360 **exclu**), saturation et
/// luminosité dans `0..=100`, et rien qui ne soit fini.
///
/// Une teinte qui ressortirait à 360,0 par arrondi serait refusée par `depuis_tsl` : la couleur
/// se perdrait au retour alors qu'elle n'a rien de particulier. Le test est exhaustif pour la même
/// raison que le précédent, et parce qu'un tel débordement ne concernerait qu'une poignée de
/// couleurs, invisibles à l'échantillonnage.
#[test]
fn en_tsl_reste_toujours_dans_les_bornes_annoncees_par_le_contrat() {
    for code in 0..=0x00ff_ffffu32 {
        let couleur = couleur_du_code(code);
        let tsl = couleur.en_tsl();

        assert!(
            (0.0..360.0).contains(&tsl.teinte),
            "{couleur:?} donne une teinte de {}°, hors du tour 0..360 (360 exclu) : `depuis_tsl` \
             la refuserait",
            tsl.teinte
        );
        assert!(
            (0.0..=100.0).contains(&tsl.saturation),
            "{couleur:?} donne une saturation de {} %, hors de 0..=100",
            tsl.saturation
        );
        assert!(
            (0.0..=100.0).contains(&tsl.luminosite),
            "{couleur:?} donne une luminosité de {} %, hors de 0..=100",
            tsl.luminosite
        );
    }
}

/// Issue #30 : « la fenêtre arrondit pour **afficher** ; elle ne calcule jamais sur l'arrondi. »
///
/// Le pendant du critère précédent : partir d'un `Tsl` saturé et y revenir doit rendre la teinte
/// demandée. On ne l'exige que sur une couleur pleine, où la teinte est la mieux définie — c'est
/// ce test qui attrape un secteur décalé ou une formule de secteur asymétrique, qui laissent les
/// six sommets justes et faussent tout ce qu'il y a entre eux.
#[test]
fn une_teinte_demandee_se_retrouve_dans_la_couleur_rendue() {
    // Deux teintes par secteur, dont deux qui encadrent une frontière (59 et 61) et une qui frôle
    // le bouclage (359).
    for teinte in [0.0, 17.0, 59.0, 61.0, 120.0, 199.0, 240.0, 300.0, 359.0] {
        let demande = Tsl {
            teinte,
            saturation: 100.0,
            luminosite: 100.0,
        };
        let couleur = couleur_de(demande);
        let retour = couleur.en_tsl();

        assert!(
            ecart(retour.teinte, teinte) <= PAS_DE_TEINTE,
            "la teinte {teinte}° donne {couleur:?}, qui se relit {}° (écart {}°, toléré \
             {PAS_DE_TEINTE}°) : au-delà d'un pas de quantification, c'est le calcul de secteur \
             qui dérive",
            retour.teinte,
            ecart(retour.teinte, teinte)
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — les six sommets du cube
// ---------------------------------------------------------------------------

/// Issue #30, test d'intention n°5 : « les six sommets du cube (rouge, jaune, vert, cyan, bleu,
/// magenta) tombent sur 0, 60, 120, 180, 240, 300 ».
///
/// Ce sont les six valeurs exactes du modèle, pas des approximations : la tolérance est fine.
/// C'est le test qui attrape une teinte en radians (le rouge resterait à 0 mais le vert tomberait
/// à 2,09), un demi-tour au lieu d'un tour, ou deux secteurs intervertis.
#[test]
fn les_six_sommets_du_cube_tombent_sur_les_multiples_de_60() {
    for (nom, couleur, teinte_attendue) in SOMMETS {
        let obtenue = couleur.en_tsl().teinte;
        assert!(
            ecart(obtenue, teinte_attendue) <= FIN,
            "le {nom} pur {couleur:?} doit tomber sur {teinte_attendue}°, obtenu {obtenue}° \
             (écart {}°)",
            ecart(obtenue, teinte_attendue)
        );
    }
}

/// Même source : les six sommets sont des couleurs pleines. Saturation et luminosité maximales,
/// exactement — une composante à 255 et une à 0, il n'y a rien à arrondir.
///
/// Sépare deux fautes que le test précédent ne distingue pas : une saturation exprimée en fraction
/// (1 au lieu de 100), et une luminosité calculée en « lightness » `(max + min) / 2`, qui vaudrait
/// 50 % sur ces six couleurs.
#[test]
fn les_six_sommets_du_cube_sont_a_saturation_et_luminosite_pleines() {
    for (nom, couleur, _) in SOMMETS {
        let tsl = couleur.en_tsl();
        assert!(
            ecart(tsl.saturation, 100.0) <= FIN,
            "le {nom} pur {couleur:?} est une couleur pleine : saturation attendue 100 %, obtenu \
             {} %",
            tsl.saturation
        );
        assert!(
            ecart(tsl.luminosite, 100.0) <= FIN,
            "le {nom} pur {couleur:?} porte une composante à 255 : luminosité attendue 100 %, \
             obtenu {} % — la moyenne (max + min) / 2 donnerait 50 %",
            tsl.luminosite
        );
    }
}

/// Le sens aller des six sommets : demander 0, 60, 120, 180, 240 ou 300 à pleine saturation et
/// pleine luminosité doit rendre la couleur pure correspondante, à l'octet près.
///
/// C'est la moitié que le test précédent ne couvre pas — `en_tsl` peut être juste et `depuis_tsl`
/// ranger ses secteurs dans le désordre, ou intervertir rouge et bleu (la confusion GRB du §2 de
/// la spec NZXT n'est jamais loin dans ce dépôt).
#[test]
fn les_multiples_de_60_rendent_les_six_couleurs_pures() {
    for (nom, couleur_attendue, teinte) in SOMMETS {
        let demande = Tsl {
            teinte,
            saturation: 100.0,
            luminosite: 100.0,
        };
        assert_eq!(
            couleur_de(demande),
            couleur_attendue,
            "{teinte}° à saturation et luminosité pleines doit rendre le {nom} pur \
             {couleur_attendue:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — les gris, le blanc et le noir
// ---------------------------------------------------------------------------

/// Issue #30 et documentation de `en_tsl` : « un gris n'a pas de teinte » — sa saturation est
/// nulle, et faute de mieux la teinte rendue est zéro.
///
/// Attrape une saturation calculée sur la « lightness », qui divise par `1 - |2L - 1|` et donne
/// `0 / 0 = NaN` sur le blanc et le noir.
#[test]
fn un_gris_a_une_saturation_nulle() {
    for niveau in [0u8, 1, 64, 127, 128, 200, 254, 255] {
        let gris = Rgb::new(niveau, niveau, niveau);
        let tsl = gris.en_tsl();
        assert_eq!(
            tsl.saturation, 0.0,
            "le gris {niveau} n'a aucune couleur : saturation attendue 0 %, obtenu {} % \
             ({tsl:?})",
            tsl.saturation
        );
        assert_eq!(
            tsl.teinte, 0.0,
            "un gris n'a pas de teinte : `en_tsl` doit rendre 0° faute de mieux, obtenu {}° \
             ({tsl:?})",
            tsl.teinte
        );
    }
}

/// Issue #30 et documentation de `Tsl` : « zéro donne le noir ». Le noir n'a ni teinte ni
/// saturation à rendre — les deux valent zéro faute de mieux.
#[test]
fn le_noir_a_une_luminosite_nulle() {
    let tsl = Rgb::BLACK.en_tsl();
    assert_eq!(
        tsl.luminosite, 0.0,
        "le noir doit donner une luminosité de 0 %, obtenu {} % ({tsl:?})",
        tsl.luminosite
    );
    assert_eq!(
        tsl.saturation, 0.0,
        "le noir n'a pas de saturation : 0 % attendu, obtenu {} % ({tsl:?}) — un `delta / max` \
         non protégé donnerait `NaN`",
        tsl.saturation
    );
    assert_eq!(
        tsl.teinte, 0.0,
        "le noir n'a pas de teinte : 0° attendu, obtenu {}° ({tsl:?})",
        tsl.teinte
    );
}

/// Issue #30, critères 6 des tests demandés : « leur luminosité vaut exactement le niveau de gris
/// en pourcents ».
///
/// Le blanc à 100 %, le noir à 0 %, et entre les deux le niveau rapporté à 255. C'est le test qui
/// fixe l'échelle : une luminosité en fraction de 1 ou une « lightness » en moyenne s'y voit
/// immédiatement, y compris sur le blanc et le noir où les deux modèles coïncident.
#[test]
fn la_luminosite_d_un_gris_vaut_son_niveau_en_pourcents() {
    for niveau in [0u8, 1, 32, 64, 127, 128, 191, 254, 255] {
        let attendue = f32::from(niveau) / 255.0 * 100.0;
        let obtenue = Rgb::new(niveau, niveau, niveau).en_tsl().luminosite;
        assert!(
            ecart(obtenue, attendue) <= FIN,
            "le gris {niveau} vaut {niveau}/255 de blanc, soit {attendue} % : obtenu {obtenue} % \
             (écart {})",
            ecart(obtenue, attendue)
        );
    }
}

/// Issue #30, critères 6 : « `depuis_tsl` avec une saturation nulle rend bien un gris (les trois
/// composantes égales) ».
///
/// Et la teinte n'y change rien : c'est ce qui permet à la fenêtre de garder la teinte d'un gris
/// en réserve pendant qu'on redescend la saturation à zéro (critère « une couleur grise garde sa
/// teinte quand on remonte la saturation depuis zéro »). Si la conversion faisait dépendre le gris
/// de la teinte, la fenêtre ne pourrait pas la conserver sans changer la couleur affichée.
#[test]
fn une_saturation_nulle_rend_un_gris_quelle_que_soit_la_teinte() {
    for luminosite in [0.0, 12.5, 25.0, 50.0, 100.0] {
        let mut premier = None;
        for teinte in [0.0, 42.0, 120.0, 217.5, 300.0, 359.9] {
            let couleur = couleur_de(Tsl {
                teinte,
                saturation: 0.0,
                luminosite,
            });
            assert!(
                couleur.r == couleur.g && couleur.g == couleur.b,
                "saturation nulle et luminosité {luminosite} % : {couleur:?} n'est pas un gris, \
                 ses trois composantes devraient être égales"
            );
            let attendu = *premier.get_or_insert(couleur);
            assert_eq!(
                couleur, attendu,
                "à saturation nulle, la teinte {teinte}° ne doit rien changer : {couleur:?} au \
                 lieu de {attendu:?}"
            );
        }
    }
}

/// Issue #30, critère « le noir garde sa teinte et sa saturation quand on remonte la luminosité
/// depuis zéro » — sa contrepartie côté conversion : luminosité nulle rend le noir, quelles que
/// soient la teinte et la saturation retenues par la fenêtre.
#[test]
fn une_luminosite_nulle_rend_le_noir_quelles_que_soient_teinte_et_saturation() {
    for teinte in [0.0, 42.0, 196.0, 300.0, 359.9] {
        for saturation in [0.0, 37.5, 100.0] {
            let couleur = couleur_de(Tsl {
                teinte,
                saturation,
                luminosite: 0.0,
            });
            assert_eq!(
                couleur,
                Rgb::BLACK,
                "luminosité nulle : la teinte {teinte}° et la saturation {saturation} % doivent \
                 rester en réserve dans la fenêtre sans allumer quoi que ce soit, obtenu \
                 {couleur:?}"
            );
        }
    }
}

/// Issue #30 : « c'est lui qui interdit l'arrondi qui fait dériver une couleur ».
///
/// Le même piège que l'aller-retour, mais pris là où celui-ci est aveugle. L'aller-retour part
/// toujours d'une couleur entière : la valeur à arrondir y retombe sur un entier, et **toute**
/// règle qui ne penche que d'un dixième la rend juste. Un curseur, lui, produit des valeurs
/// quelconques — c'est là que se voit un arrondi qui penche.
///
/// Chaque cas donne la valeur exacte visée sur 255 et le gris le plus proche. Aucune n'est à égale
/// distance de deux entiers, donc aucune convention de départage n'est en jeu ; mais deux d'entre
/// elles frôlent le demi-pas des deux côtés (53,55 juste au-dessus, 63,45 juste en dessous), de
/// sorte qu'un arrondi qui penche d'un dixième dans un sens ou dans l'autre se fait prendre.
///
/// Le test est séparé de l'aller-retour aussi parce que `with_brightness`, juste au-dessus dans le
/// même fichier, **tronque** délibérément (spec §11) : la tentation de reprendre la même règle est
/// concrète, et l'échec doit dire lequel des deux arrondis s'applique où.
#[test]
fn depuis_tsl_arrondit_au_plus_proche_et_ne_tronque_pas() {
    for (vise, attendu) in [(63.75f32, 64u8), (114.75, 115), (53.55, 54), (63.45, 63)] {
        let luminosite = vise / 255.0 * 100.0;
        let couleur = couleur_de(Tsl {
            teinte: 0.0,
            saturation: 0.0,
            luminosite,
        });
        assert_eq!(
            couleur,
            Rgb::new(attendu, attendu, attendu),
            "une luminosité de {luminosite} % place la composante à {vise} sur 255 : le gris le \
             plus proche est {attendu}, obtenu {couleur:?}. Tronquer, arrondir vers le haut ou \
             pencher d'un côté donnerait {} ou {} — la règle du sélecteur est « le plus proche », \
             pas la troncature de `with_brightness`",
            vise.floor(),
            vise.ceil()
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — ce qui doit être refusé, en nommant le champ
// ---------------------------------------------------------------------------

/// Issue #30, test d'intention n°6 : « une teinte hors bornes est refusée en la nommant, pas
/// repliée en silence ».
///
/// Ni modulo, ni écrêtage : la fenêtre doit pouvoir dire quel curseur est en cause. Une teinte
/// repliée au modulo rendrait 400° indiscernable de 40°.
#[test]
fn une_teinte_hors_bornes_est_refusee_en_nommant_le_champ() {
    for teinte in [-0.5, -1.0, -360.0, 400.0, 720.0, 1e9] {
        assert_refus_sur(
            "teinte",
            Tsl {
                teinte,
                saturation: 50.0,
                luminosite: 50.0,
            },
        );
    }
}

/// Même critère, étendu aux deux autres axes : rien ne justifie qu'une saturation de 150 % soit
/// écrêtée sans le dire alors qu'une teinte de 400° est refusée.
#[test]
fn une_saturation_hors_bornes_est_refusee_en_nommant_le_champ() {
    for saturation in [-0.5, -1.0, 100.5, 101.0, 255.0] {
        assert_refus_sur(
            "saturation",
            Tsl {
                teinte: 196.0,
                saturation,
                luminosite: 50.0,
            },
        );
    }
}

/// Même critère pour la luminosité. À noter : `Brightness::new` **écrête** au-delà de 100 (elle le
/// documente), le sélecteur non — deux règles voisines dans le même module, l'échec doit dire
/// laquelle s'applique.
#[test]
fn une_luminosite_hors_bornes_est_refusee_en_nommant_le_champ() {
    for luminosite in [-0.5, -1.0, 100.5, 101.0, 255.0] {
        assert_refus_sur(
            "luminosite",
            Tsl {
                teinte: 196.0,
                saturation: 50.0,
                luminosite,
            },
        );
    }
}

/// Le contrat écrit le tour `0..360` : borne basse **incluse**, borne haute **exclue**. 360° et 0°
/// désignent la même teinte ; en accepter deux écritures ferait deux `Tsl` distincts pour une même
/// couleur, et `en_tsl` n'en rend jamais qu'une.
///
/// Le test presse la borne des deux côtés au flottant près : la plus grande valeur strictement
/// sous 360 doit passer, 360 lui-même doit être refusé en nommant la teinte.
#[test]
fn la_teinte_de_360_est_refusee_mais_la_valeur_juste_en_dessous_passe() {
    let sous = juste_sous_360();
    assert!(
        sous < 360.0,
        "la valeur de test {sous} devait être strictement sous 360 : le test lui-même est faux"
    );

    let derniere = Tsl {
        teinte: sous,
        saturation: 100.0,
        luminosite: 100.0,
    };
    assert!(
        Rgb::depuis_tsl(derniere).is_ok(),
        "{sous}° est strictement sous 360 et doit donc être accepté : {:?}",
        Rgb::depuis_tsl(derniere)
    );

    assert_refus_sur(
        "teinte",
        Tsl {
            teinte: 360.0,
            saturation: 100.0,
            luminosite: 100.0,
        },
    );
}

/// L'autre bord de chaque axe : les bornes valides doivent être acceptées. Un contrôle écrit avec
/// une inégalité stricte de trop refuserait le rouge pur, le noir ou le blanc — les trois couleurs
/// qu'on essaie en premier.
#[test]
fn les_bornes_valides_sont_acceptees() {
    let cas = [
        ("le rouge pur, teinte à la borne basse", 0.0, 100.0, 100.0),
        ("le noir, luminosité à la borne basse", 0.0, 0.0, 0.0),
        (
            "le blanc, saturation nulle et luminosité pleine",
            0.0,
            0.0,
            100.0,
        ),
        ("une couleur pleine en fin de tour", 359.0, 100.0, 100.0),
    ];
    for (quoi, teinte, saturation, luminosite) in cas {
        let tsl = Tsl {
            teinte,
            saturation,
            luminosite,
        };
        assert!(
            Rgb::depuis_tsl(tsl).is_ok(),
            "{quoi} ({tsl:?}) est dans le contrat et doit être accepté : {:?}",
            Rgb::depuis_tsl(tsl)
        );
    }
}

/// `NaN` et les infinis, sur chacun des trois axes.
///
/// Le piège est précis : un contrôle écrit `si v < 0 || v > 100` laisse passer `NaN`, car toute
/// comparaison avec `NaN` est fausse. Il traverserait alors la conversion et ressortirait en
/// couleur arbitraire — un `NaN` converti en entier vaut 0 en Rust, donc en noir crédible, sans
/// message. Ils doivent être refusés **en nommant le champ**, comme n'importe quelle valeur hors
/// bornes : la fenêtre n'a pas à savoir distinguer les deux fautes.
#[test]
fn une_valeur_non_finie_est_refusee_en_nommant_le_champ() {
    for valeur in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_refus_sur(
            "teinte",
            Tsl {
                teinte: valeur,
                saturation: 50.0,
                luminosite: 50.0,
            },
        );
        assert_refus_sur(
            "saturation",
            Tsl {
                teinte: 196.0,
                saturation: valeur,
                luminosite: 50.0,
            },
        );
        assert_refus_sur(
            "luminosite",
            Tsl {
                teinte: 196.0,
                saturation: 50.0,
                luminosite: valeur,
            },
        );
    }
}

/// Quand plusieurs axes sont fautifs à la fois, le contrat ne dit pas lequel est nommé — et n'a
/// pas à le dire : la fenêtre signale le curseur qu'on lui désigne, il suffit qu'il soit
/// réellement en cause.
///
/// Ce que le test interdit, c'est un champ constant : une erreur qui nomme toujours « teinte »
/// passerait les trois tests précédents pour de mauvaises raisons dès qu'un seul d'entre eux
/// existe.
#[test]
fn l_erreur_nomme_toujours_un_champ_reellement_fautif() {
    let cas = [
        (
            "saturation et luminosité fautives, teinte valide",
            Tsl {
                teinte: 196.0,
                saturation: 150.0,
                luminosite: -3.0,
            },
            ["saturation", "luminosite"],
        ),
        (
            "teinte et luminosité fautives, saturation valide",
            Tsl {
                teinte: 400.0,
                saturation: 50.0,
                luminosite: 101.0,
            },
            ["teinte", "luminosite"],
        ),
        (
            "teinte et saturation fautives, luminosité valide",
            Tsl {
                teinte: -1.0,
                saturation: f32::NAN,
                luminosite: 50.0,
            },
            ["teinte", "saturation"],
        ),
    ];

    for (quoi, tsl, fautifs) in cas {
        let erreur = refus_de(tsl);
        assert!(
            fautifs.contains(&erreur.champ),
            "{quoi} : l'erreur nomme « {} », qui est pourtant dans ses bornes. Les champs en \
             cause sont {fautifs:?} ({tsl:?})",
            erreur.champ
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — ce que les curseurs doivent donner à voir
// ---------------------------------------------------------------------------

/// Issue #30 : « la luminosité va du noir à la couleur ». Un curseur qu'on pousse vers le haut ne
/// doit assombrir aucune composante, sur toute sa course.
///
/// Sans cette garantie le dégradé peint sur le curseur mentirait quelque part, et l'aperçu
/// clignoterait en cours de glissement. Attrape une inversion de sens, un secteur qui change en
/// fonction de la luminosité, et les erreurs d'arrondi qui feraient reculer une composante d'un
/// cran au passage.
#[test]
fn la_luminosite_ne_fait_jamais_reculer_une_composante() {
    for teinte in [0.0, 45.0, 137.0, 210.0, 300.0, 359.0] {
        for saturation in [0.0, 25.0, 60.0, 100.0] {
            let mut precedent = Rgb::BLACK;
            for pas in 0..=100u8 {
                let luminosite = f32::from(pas);
                let couleur = couleur_de(Tsl {
                    teinte,
                    saturation,
                    luminosite,
                });
                assert!(
                    couleur.r >= precedent.r
                        && couleur.g >= precedent.g
                        && couleur.b >= precedent.b,
                    "teinte {teinte}°, saturation {saturation} % : passer la luminosité à \
                     {luminosite} % assombrit une composante — {precedent:?} puis {couleur:?}"
                );
                precedent = couleur;
            }
        }
    }
}

/// Issue #30 : « la teinte porte l'arc-en-ciel entier ». Le dégradé du curseur de teinte doit être
/// continu — le tour se calcule par sixièmes, et c'est aux jointures qu'un secteur mal découpé se
/// voit.
///
/// De part et d'autre de chaque frontière, à un millième de degré, les couleurs rendues doivent
/// être voisines : aucune composante ne saute d'une unité. Un secteur décalé d'un rang, une
/// formule qui utilise la fraction du secteur au lieu de sa distance au milieu, ou un `floor` qui
/// bascule au mauvais moment font tous sauter une composante de 255 ici.
#[test]
fn les_frontieres_de_secteur_ne_font_sauter_aucune_composante() {
    /// Un millième de degré, quatre fois plus fin que le pas de quantification d'une composante
    /// (60 / 255 ≈ 0,24°) : de part et d'autre, la couleur doit être la même à une unité près.
    const EPSILON: f32 = 0.001;

    for frontiere in [60.0f32, 120.0, 180.0, 240.0, 300.0] {
        for (saturation, luminosite) in [(100.0, 100.0), (70.0, 80.0), (100.0, 40.0)] {
            let avant = couleur_de(Tsl {
                teinte: frontiere - EPSILON,
                saturation,
                luminosite,
            });
            let apres = couleur_de(Tsl {
                teinte: frontiere + EPSILON,
                saturation,
                luminosite,
            });
            let saut = [
                avant.r.abs_diff(apres.r),
                avant.g.abs_diff(apres.g),
                avant.b.abs_diff(apres.b),
            ];
            assert!(
                saut.iter().all(|&d| d <= 1),
                "frontière {frontiere}° (saturation {saturation} %, luminosité {luminosite} %) : \
                 {avant:?} juste avant, {apres:?} juste après — saut de {saut:?} sur \
                 {}° d'écart",
                2.0 * EPSILON
            );
        }
    }
}

/// La jointure que le test précédent ne peut pas presser des deux côtés : celle où le tour se
/// referme. À 359,999° et à 0°, on est à deux millièmes de degré l'un de l'autre du point de vue
/// de l'œil, et la couleur doit l'être aussi.
///
/// C'est aussi le seul endroit où la teinte change de secteur **sans** changer de valeur : un
/// contrôle de bornes qui laisserait déborder la teinte à 360 tomberait sur un septième secteur
/// inexistant.
#[test]
fn le_tour_se_referme_sans_saut_entre_359_et_0() {
    for (saturation, luminosite) in [(100.0, 100.0), (70.0, 80.0), (100.0, 40.0)] {
        let fin_de_tour = couleur_de(Tsl {
            teinte: juste_sous_360(),
            saturation,
            luminosite,
        });
        let debut_de_tour = couleur_de(Tsl {
            teinte: 0.0,
            saturation,
            luminosite,
        });
        let saut = [
            fin_de_tour.r.abs_diff(debut_de_tour.r),
            fin_de_tour.g.abs_diff(debut_de_tour.g),
            fin_de_tour.b.abs_diff(debut_de_tour.b),
        ];
        assert!(
            saut.iter().all(|&d| d <= 1),
            "le tour ne se referme pas (saturation {saturation} %, luminosité {luminosite} %) : \
             {fin_de_tour:?} juste avant 360°, {debut_de_tour:?} à 0° — saut de {saut:?}"
        );
    }
}

/// Issue #30 : « la saturation va du gris à la couleur ». À teinte et luminosité fixées, monter la
/// saturation ne doit qu'écarter les composantes les unes des autres — jamais les rapprocher.
///
/// La composante la plus forte reste où elle est (c'est la luminosité qui la fixe), la plus faible
/// descend. Une saturation qui agirait sur la mauvaise extrémité rendrait le curseur inutilisable
/// sans être visiblement faux.
#[test]
fn la_saturation_ne_fait_qu_ecarter_les_composantes() {
    for teinte in [0.0, 45.0, 137.0, 210.0, 300.0, 359.0] {
        for luminosite in [20.0, 50.0, 100.0] {
            let mut precedent: Option<(u8, u8)> = None;
            for pas in 0..=100u8 {
                let saturation = f32::from(pas);
                let couleur = couleur_de(Tsl {
                    teinte,
                    saturation,
                    luminosite,
                });
                let haut = couleur.r.max(couleur.g).max(couleur.b);
                let bas = couleur.r.min(couleur.g).min(couleur.b);

                if let Some((haut_avant, bas_avant)) = precedent {
                    assert!(
                        haut >= haut_avant.saturating_sub(1),
                        "teinte {teinte}°, luminosité {luminosite} % : monter la saturation à \
                         {saturation} % a fait chuter la composante la plus forte de \
                         {haut_avant} à {haut} — c'est la luminosité qui la fixe, pas la \
                         saturation"
                    );
                    assert!(
                        bas <= bas_avant,
                        "teinte {teinte}°, luminosité {luminosite} % : monter la saturation à \
                         {saturation} % a remonté la composante la plus faible de {bas_avant} à \
                         {bas} — une couleur plus saturée est une couleur plus écartée du gris"
                    );
                }
                precedent = Some((haut, bas));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 7 — le cas ordinaire, écrit en clair
// ---------------------------------------------------------------------------

/// Un garde-fou lisible : une couleur quelconque, ses trois valeurs calculées à la main depuis la
/// définition du modèle, et son aller-retour.
///
/// `0x348b5f` : max = 0x8b = 139, min = 0x34 = 52, écart = 87.
/// - luminosité = 139 / 255 = 54,510 %
/// - saturation = 87 / 139 = 62,590 %
/// - teinte : le maximum est le vert, donc 60 × (2 + (bleu − rouge) / écart)
///   = 60 × (2 + (95 − 52) / 87) = 149,655°
///
/// Les deux tests exhaustifs plus haut disent « ça marche partout » sans jamais montrer une
/// valeur ; celui-ci montre laquelle, et sert de première marche au débogage quand ils tombent.
#[test]
fn une_couleur_quelconque_se_lit_comme_le_modele_l_annonce() {
    let tsl = QUELCONQUE.en_tsl();

    let luminosite_attendue = 139.0 / 255.0 * 100.0;
    let saturation_attendue = 87.0 / 139.0 * 100.0;
    let teinte_attendue = 60.0 * (2.0 + (95.0 - 52.0) / 87.0);

    assert!(
        ecart(tsl.luminosite, luminosite_attendue) <= FIN,
        "{QUELCONQUE:?} : luminosité attendue {luminosite_attendue} % (139 sur 255), obtenu {} %",
        tsl.luminosite
    );
    assert!(
        ecart(tsl.saturation, saturation_attendue) <= FIN,
        "{QUELCONQUE:?} : saturation attendue {saturation_attendue} % (écart 87 sur un maximum de \
         139), obtenu {} %",
        tsl.saturation
    );
    assert!(
        ecart(tsl.teinte, teinte_attendue) <= FIN,
        "{QUELCONQUE:?} : teinte attendue {teinte_attendue}° (secteur du vert), obtenu {}°",
        tsl.teinte
    );
    assert_eq!(
        Rgb::depuis_tsl(tsl),
        Ok(QUELCONQUE),
        "l'aller-retour de {QUELCONQUE:?} doit rendre la couleur de départ ({tsl:?})"
    );
}
