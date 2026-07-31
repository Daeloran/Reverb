//! Tests d'intention de la géométrie du boîtier (issue #19).
//!
//! Écrits **avant** l'implémentation, depuis l'issue #19 et son commentaire « Contrat d'API »
//! seuls. Aucune ligne n'est relue depuis un corps de fonction : à l'écriture de ce fichier,
//! `crates/reverb-anim/src/` n'est qu'un squelette de signatures dont tous les corps sont
//! `todo!()`. Ils encodent ce que la géométrie doit faire, pas ce que le code fait — si l'un
//! d'eux échoue après implémentation, c'est le code qu'on corrige.
//!
//! ## La règle qui gouverne ce fichier : aucune coordonnée n'est écrite ici
//!
//! La table de géométrie n'est **pas encore mesurée**. Un test qui exigerait « la LED 3 du
//! radiateur haut est à 240 mm du plancher » figerait dans la spécification un nombre que
//! personne n'a relevé, et il faudrait le réécrire au premier coup de mètre — donc il ne
//! spécifie rien.
//!
//! Ces tests ne portent donc que sur des **relations** : les LED sont toutes distinctes et
//! finies, les bornes les encadrent toutes, l'aller-retour `encoder`/`decoder` est fidèle,
//! `definir` ne déplace qu'un ventilateur. Toutes restent vraies quelles que soient les valeurs
//! que la mesure rendra, et toutes seraient fausses si la table était bâclée.
//!
//! ## Trois points que le contrat laisse ouverts, et que ces tests tranchent
//!
//! 1. **Le sens s'écrit `horaire` et `antihoraire`**, en ASCII minuscule. Le contrat donne la
//!    grammaire `<position-slug> <angle> <sens>` sans dire les mots ; l'issue les écrit dans son
//!    exemple de commande (`geometry radiateur-haut angle=90 sens=horaire`), et ils traversent
//!    le protocole IPC, où un accent et une espace sont exclus (même règle que `Position::slug`).
//! 2. **Le champ fautif se nomme `position`, `angle` ou `sens`** — les trois champs de la
//!    grammaire, et les trois mêmes noms que la ligne de réponse `geom` du protocole. Le contrat
//!    exige un refus « nommant le champ fautif » sans figer le vocabulaire ; en prendre un autre
//!    ferait diverger le message du démon et le nom de la clé que l'utilisateur a tapée.
//! 3. **Une ligne qui porte plus de trois champs est refusée**, pas tronquée. Critère
//!    d'acceptation de l'issue : « jamais appliqué de travers en silence ». Le fichier est écrit
//!    par le démon mais relu par un humain qui peut l'avoir édité.
//!
//! Aucun accès matériel : `reverb-anim` est pur, ses tests aussi. Le seul appel au système de
//! ce fichier est la lecture de `/proc/self/fd` dans `spec_animations.rs`, qui sert précisément
//! à prouver que le crate n'ouvre rien.

use reverb_anim::{Geometrie, Orientation, Point, Sens};
use reverb_proto::ram::{LEDS_PER_STICK, SLOT_COUNT};
use reverb_proto::{LEDS_PER_FAN, Position};

// ---------------------------------------------------------------------------
// Vecteurs et aides
// ---------------------------------------------------------------------------

/// Les deux sens, pour n'en oublier aucun quand le domaine est de taille deux.
const SENS: [Sens; 2] = [Sens::Horaire, Sens::Antihoraire];

/// Le nombre de LED du boîtier, recalculé depuis les constantes du protocole plutôt que recopié.
///
/// 10 × 8 + 4 × 11 = 124. Si un jour un ventilateur ou une barrette s'ajoute, c'est le test qui
/// suit le matériel, pas l'inverse.
const LED_DU_BOITIER: usize =
    Position::ALL.len() * LEDS_PER_FAN as usize + SLOT_COUNT * LEDS_PER_STICK;

/// Le mot qui écrit un sens dans le fichier de géométrie et sur le socket.
fn slug_sens(sens: Sens) -> &'static str {
    match sens {
        Sens::Horaire => "horaire",
        Sens::Antihoraire => "antihoraire",
    }
}

/// Une orientation valide, ou l'échec du test si elle ne l'est pas.
fn orientation(angle: u16, sens: Sens) -> Orientation {
    Orientation::new(angle, sens).unwrap_or_else(|erreur| {
        panic!(
            "{angle}° {} est une orientation valide : {erreur}",
            slug_sens(sens)
        )
    })
}

/// L'orientation attribuée au `i`-ième ventilateur par [`texte_varie`].
///
/// Le pas de 37° n'a rien de physique : c'est un nombre premier avec 360, donc les dix angles
/// sont deux à deux distincts, et aucun ne retombe sur un multiple de 45 qui pourrait masquer
/// une confusion entre l'angle du ventilateur et le pas de l'anneau.
fn orientation_variee(i: usize) -> (u16, Sens) {
    let angle = (i as u16 * 37) % 360;
    let sens = if i.is_multiple_of(2) {
        Sens::Horaire
    } else {
        Sens::Antihoraire
    };
    (angle, sens)
}

/// Le texte d'une géométrie où les dix ventilateurs portent la même orientation.
fn texte_uniforme(angle: u16, sens: Sens) -> String {
    let mut lignes = Vec::new();
    for position in Position::ALL {
        lignes.push(format!("{} {} {}", position.slug(), angle, slug_sens(sens)));
    }
    lignes.join("\n")
}

/// Le texte d'une géométrie où chaque ventilateur porte une orientation qui lui est propre.
///
/// C'est le vecteur qui compte : une géométrie uniforme se relirait juste avec un décodeur qui
/// ignore le slug et applique la même orientation partout.
fn texte_varie() -> String {
    let mut lignes = Vec::new();
    for (i, position) in Position::ALL.iter().enumerate() {
        let (angle, sens) = orientation_variee(i);
        lignes.push(format!("{} {} {}", position.slug(), angle, slug_sens(sens)));
    }
    lignes.join("\n")
}

/// La géométrie de [`texte_varie`], construite par le décodeur.
fn geometrie_variee() -> Geometrie {
    Geometrie::decoder(&texte_varie()).expect("le texte varié est une géométrie valide")
}

/// Les 124 points de la géométrie, dans un ordre stable.
fn tous_les_points(geometrie: &Geometrie) -> Vec<(String, Point)> {
    let mut points = Vec::new();
    for position in Position::ALL {
        for led in 0..LEDS_PER_FAN as usize {
            let point = geometrie.led_ventilateur(position, led).unwrap_or_else(|| {
                panic!(
                    "{}, LED {led} : sans place dans le boîtier",
                    position.slug()
                )
            });
            points.push((format!("{} led {led}", position.slug()), point));
        }
    }
    for slot in 0..SLOT_COUNT {
        for led in 0..LEDS_PER_STICK {
            let point = geometrie.led_barrette(slot, led).unwrap_or_else(|| {
                panic!("barrette {slot}, LED {led} : sans place dans le boîtier")
            });
            points.push((format!("barrette {slot} led {led}"), point));
        }
    }
    points
}

/// L'écart angulaire en tournant dans le sens **horaire**, de `depuis` vers `vers`.
fn ecart_horaire(depuis: u16, vers: u16) -> u16 {
    (vers + 360 - depuis) % 360
}

// ---------------------------------------------------------------------------
// 1 — l'angle d'origine est borné au tour, et le refus nomme son champ
// ---------------------------------------------------------------------------

#[test]
fn un_angle_hors_du_tour_est_refuse_en_nommant_son_champ() {
    // Contrat d'API — `Orientation::new` : « Refuse un angle hors `0..=359` ».
    //
    // Un angle est un nombre de degrés, pas une valeur libre : 360 est le même point que 0, et
    // 400 n'est le même que 40 qu'à condition que quelqu'un ait pensé au modulo. Laisser entrer
    // 400 obligerait chaque calcul en aval à s'en méfier ; le refuser une fois à l'entrée met la
    // question derrière soi. Et ce champ vient de la ligne de commande, donc d'une frappe humaine.
    //
    // Exhaustif sur les 360 valeurs acceptées : le domaine est petit, un échantillon de trois
    // angles laisserait passer une borne fermée du mauvais côté.
    for angle in 0u16..=359 {
        for sens in SENS {
            let orientation = Orientation::new(angle, sens)
                .unwrap_or_else(|erreur| panic!("{angle}° est dans le tour : {erreur}"));
            assert_eq!(orientation.angle, angle, "l'angle accepté est celui donné");
            assert_eq!(orientation.sens, sens, "le sens accepté est celui donné");
        }
    }

    // 360 est la première valeur refusée. C'est la borne exacte, et c'est celle qu'un décodeur
    // écrit avec un `<=` de trop laisserait passer — en la lisant ensuite comme « midi », donc
    // sans jamais rien signaler.
    for angle in [360u16, 361, 400, 719, 720, 1000, 32_768, u16::MAX] {
        for sens in SENS {
            let erreur = Orientation::new(angle, sens)
                .expect_err("un angle hors du tour n'est pas une orientation");
            assert_eq!(
                erreur.champ, "angle",
                "le refus doit nommer le champ fautif, et il n'y en a que deux"
            );
            assert!(!erreur.raison.is_empty(), "le refus doit dire pourquoi");

            // Un message d'erreur qui ne dit ni le champ ni la valeur envoie chercher. Celui-ci
            // est lu par quelqu'un qui vient de taper `geometry radiateur-haut angle=400`.
            let message = erreur.to_string();
            assert!(
                message.contains("angle"),
                "le message doit nommer le champ : « {message} »"
            );
            assert!(
                message.contains(&angle.to_string()),
                "le message doit dire la valeur refusée : « {message} »"
            );
            let _: &dyn std::error::Error = &erreur;
        }
    }
}

// ---------------------------------------------------------------------------
// 2 — les huit LED forment un anneau, et le sens n'est pas décoratif
// ---------------------------------------------------------------------------

#[test]
fn les_huit_led_couvrent_l_anneau_dans_le_sens_declare() {
    // Contrat d'API — « les huit `angle_led` d'une orientation sont distincts et couvrent
    // l'anneau », et `Orientation` : « Où se trouve la LED 1 […], `angle` en degrés, 0 = midi,
    // croissant dans le sens horaire vu de l'extérieur ».
    //
    // ⚠️ Le **pas** angulaire n'est pas figé ici. L'issue le dit noir sur blanc : « les huit LED
    // sont-elles réparties à 45° exactement ? C'est l'hypothèse de départ, à confirmer par le
    // rendu ». Un test qui exigerait 45° spécifierait une mesure non faite. Ce qui est exigible
    // sans la mesure, c'est la **forme** : huit angles distincts, parcourus dans un seul sens, et
    // dont les écarts font exactement un tour. C'est vrai à 45° comme à tout autre pas régulier
    // ou non, et faux dès que les LED zigzaguent ou repassent au même endroit.
    let led_du_ventilateur = LEDS_PER_FAN as usize;

    for angle in [0u16, 1, 45, 90, 179, 180, 270, 337, 359] {
        for sens in SENS {
            let orientation = orientation(angle, sens);
            let angles: Vec<u16> = (0..led_du_ventilateur)
                .map(|led| orientation.angle_led(led))
                .collect();

            // La LED 1 est **à** l'angle déclaré : c'est la définition du champ, et c'est ce qui
            // rend la mesure exploitable — l'observation relève l'heure du rouge, c'est-à-dire
            // l'angle de la LED d'indice 0.
            assert_eq!(
                angles[0],
                angle,
                "la LED 1 de {angle}° {} doit être à {angle}°",
                slug_sens(sens)
            );

            for (led, &a) in angles.iter().enumerate() {
                assert!(
                    a < 360,
                    "la LED {led} de {angle}° {} est à {a}°, hors du tour",
                    slug_sens(sens)
                );
            }

            let mut distincts = angles.clone();
            distincts.sort_unstable();
            distincts.dedup();
            assert_eq!(
                distincts.len(),
                led_du_ventilateur,
                "deux LED d'un même anneau ne peuvent pas être au même endroit : {angles:?}"
            );

            // Le tour complet : en passant de chaque LED à la suivante puis en revenant à la
            // première, on parcourt exactement 360°, jamais deux tours ni un demi. Avec des
            // écarts tous strictement inférieurs au demi-tour, c'est ce qui prouve que les
            // indices tournent dans un seul sens sans revenir en arrière.
            let mut tour = 0u32;
            for led in 0..led_du_ventilateur {
                let suivante = angles[(led + 1) % led_du_ventilateur];
                let ecart = match sens {
                    Sens::Horaire => ecart_horaire(angles[led], suivante),
                    Sens::Antihoraire => ecart_horaire(suivante, angles[led]),
                };
                assert!(
                    (1..180).contains(&ecart),
                    "de la LED {led} à la suivante, {ecart}° en sens {} : l'anneau doit avancer, \
                     et d'un pas plus court qu'un demi-tour — {angles:?}",
                    slug_sens(sens)
                );
                tour += u32::from(ecart);
            }
            assert_eq!(
                tour, 360,
                "les huit écarts doivent faire un tour et un seul : {angles:?}"
            );
        }

        // Et le sens change réellement quelque chose. Sans cette vérification, une implémentation
        // qui ignorerait `sens` passerait tout ce qui précède : les deux suites seraient
        // identiques et chacune ferait son tour. Or le sens est la moitié de ce que la mesure a
        // coûté à relever.
        let horaire = orientation(angle, Sens::Horaire);
        let antihoraire = orientation(angle, Sens::Antihoraire);
        assert_ne!(
            horaire.angle_led(1),
            antihoraire.angle_led(1),
            "à {angle}°, la LED 2 ne peut pas être au même endroit dans les deux sens"
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — les 124 LED ont chacune une place, et une seule
// ---------------------------------------------------------------------------

#[test]
fn chaque_led_du_boitier_a_une_place_distincte_et_finie() {
    // Critère d'acceptation de l'issue — « La table de géométrie donne pour chacune des 124 LED
    // une coordonnée dans le boîtier ».
    //
    // Trois défauts sont possibles et aucun ne se voit à l'exécution : une case oubliée (`None`
    // là où une LED existe), une case recopiée d'une autre (deux LED au même point — l'erreur de
    // copier-coller d'une table écrite à la main), et une coordonnée non finie. La dernière est
    // la plus vicieuse : un `NaN` se propage dans tout calcul d'animation, où il rend chaque
    // comparaison fausse **sans** jamais lever d'erreur, et l'animation devient noire sans motif.
    let geometrie = Geometrie::mesuree();
    let points = tous_les_points(&geometrie);
    assert_eq!(
        points.len(),
        LED_DU_BOITIER,
        "le boîtier compte {LED_DU_BOITIER} LED"
    );
    assert_eq!(LED_DU_BOITIER, 124, "10 × 8 + 4 × 11");

    for (nom, point) in &points {
        for (axe, valeur) in [("x", point.x), ("y", point.y), ("z", point.z)] {
            assert!(
                valeur.is_finite(),
                "{nom} : coordonnée {axe} = {valeur}, ni finie ni exploitable"
            );
        }
    }

    let mut vus: Vec<(u32, u32, u32)> = points
        .iter()
        .map(|(_, p)| (p.x.to_bits(), p.y.to_bits(), p.z.to_bits()))
        .collect();
    vus.sort_unstable();
    vus.dedup();
    assert_eq!(
        vus.len(),
        points.len(),
        "deux LED partagent le même point : la table a été recopiée quelque part"
    );

    // Les index hors domaine rendent `None`, ils ne rendent ni un point voisin ni une panique.
    // Contrat d'API — « `led_ventilateur` rend `None` au-delà de 7, `led_barrette` au-delà de 10
    // ou du slot 3 ». Les valeurs extrêmes sont là pour l'arithmétique d'index : `usize::MAX`
    // est ce qu'on obtient d'un `- 1` sur zéro.
    for position in Position::ALL {
        for led in [
            LEDS_PER_FAN as usize,
            LEDS_PER_FAN as usize + 1,
            11,
            124,
            usize::MAX,
        ] {
            assert_eq!(
                geometrie.led_ventilateur(position, led),
                None,
                "{} n'a pas de LED d'indice {led}",
                position.slug()
            );
        }
    }
    for slot in [SLOT_COUNT, SLOT_COUNT + 1, 100, usize::MAX] {
        for led in [0, LEDS_PER_STICK - 1] {
            assert_eq!(
                geometrie.led_barrette(slot, led),
                None,
                "il n'y a pas de barrette {slot}"
            );
        }
    }
    for slot in 0..SLOT_COUNT {
        for led in [LEDS_PER_STICK, LEDS_PER_STICK + 1, 100, usize::MAX] {
            assert_eq!(
                geometrie.led_barrette(slot, led),
                None,
                "la barrette {slot} n'a pas de LED d'indice {led}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — les bornes encadrent tout, et le volume n'est pas plat
// ---------------------------------------------------------------------------

#[test]
fn les_bornes_encadrent_toutes_les_led_sur_les_trois_axes() {
    // Contrat d'API — `bornes` : « Coin bas-avant-gauche et coin haut-arrière-droit du volume
    // occupé. Sert aux animations à normaliser sans coder de dimensions en dur. »
    //
    // C'est donc le **diviseur** de toute animation : la position d'une LED y devient un nombre
    // entre 0 et 1. Deux façons de le casser, toutes deux silencieuses. Des bornes trop étroites
    // : les LED qui débordent sortent de l'intervalle, et le motif se sature aux extrémités.
    // Un axe plat (`min == max`) : la normalisation divise par zéro, et toute une animation part
    // en `NaN` — d'où l'exigence d'un volume réellement tridimensionnel, que le boîtier a par
    // construction (des ventilateurs au plancher *et* au plafond, à l'avant *et* à l'arrière,
    // à gauche *et* à droite).
    for geometrie in [Geometrie::mesuree(), geometrie_variee()] {
        let (bas, haut) = geometrie.bornes();

        for (axe, min, max) in [
            ("x", bas.x, haut.x),
            ("y", bas.y, haut.y),
            ("z", bas.z, haut.z),
        ] {
            assert!(
                min.is_finite() && max.is_finite(),
                "borne {axe} non finie : {min} … {max}"
            );
            assert!(
                min < max,
                "le boîtier est occupé sur l'axe {axe} : {min} … {max} ne laisse aucune épaisseur, \
                 et une animation y diviserait par zéro"
            );
        }

        for (nom, point) in tous_les_points(&geometrie) {
            assert!(
                point.x >= bas.x && point.x <= haut.x,
                "{nom} déborde des bornes en x : {} hors de {} … {}",
                point.x,
                bas.x,
                haut.x
            );
            assert!(
                point.y >= bas.y && point.y <= haut.y,
                "{nom} déborde des bornes en y : {} hors de {} … {}",
                point.y,
                bas.y,
                haut.y
            );
            assert!(
                point.z >= bas.z && point.z <= haut.z,
                "{nom} déborde des bornes en z : {} hors de {} … {}",
                point.z,
                bas.z,
                haut.z
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — l'aller-retour d'une géométrie (test d'intention n° 10)
// ---------------------------------------------------------------------------

#[test]
fn une_geometrie_encodee_puis_decodee_rend_la_geometrie_d_origine() {
    // Contrat d'API — « `decoder(g.encoder()) == g` pour toute géométrie valide », et
    // `encoder` : « Une ligne par ventilateur : `<position-slug> <angle> <sens>` ».
    //
    // Cet aller-retour n'est pas une politesse : c'est lui qui rend vrai le critère
    // d'acceptation « elle survit à `systemctl restart reverbd` ». Le fichier écrit est la seule
    // mémoire de la géométrie ; ce qu'il ne sait pas dire est perdu au premier redémarrage, sans
    // le moindre message — l'utilisateur retrouve juste son onde de travers.
    let mut temoins = vec![geometrie_variee()];
    for angle in [0u16, 90, 180, 359] {
        for sens in SENS {
            temoins.push(
                Geometrie::decoder(&texte_uniforme(angle, sens))
                    .expect("une géométrie uniforme est valide"),
            );
        }
    }
    // Et la mesurée elle-même, qui est la géométrie que le démon écrira le premier jour.
    temoins.push(Geometrie::mesuree());

    for temoin in &temoins {
        let texte = temoin.encoder();

        // Dix lignes, une par ventilateur, chacune à trois champs, et les dix positions
        // exactement une fois. C'est la forme que la persistance relira, et qu'un humain peut
        // avoir à corriger à la main dans `/etc/reverb/geometrie.conf`.
        let lignes: Vec<&str> = texte.lines().collect();
        assert_eq!(
            lignes.len(),
            Position::ALL.len(),
            "une ligne par ventilateur : « {texte} »"
        );

        let mut slugs: Vec<&str> = Vec::new();
        for ligne in &lignes {
            let champs: Vec<&str> = ligne.split_whitespace().collect();
            assert_eq!(
                champs.len(),
                3,
                "la ligne « {ligne} » n'a pas la forme <position> <angle> <sens>"
            );
            let position = Position::from_slug(champs[0])
                .unwrap_or_else(|_| panic!("« {} » n'est pas un slug de position", champs[0]));
            let angle: u16 = champs[1]
                .parse()
                .unwrap_or_else(|_| panic!("« {} » n'est pas un angle", champs[1]));
            assert!(
                angle < 360,
                "l'angle écrit doit être dans le tour : {angle}"
            );
            assert!(
                champs[2] == "horaire" || champs[2] == "antihoraire",
                "« {} » n'est pas un sens",
                champs[2]
            );

            // Ce qui est écrit est ce que l'accesseur rend : sans ça, le fichier et le socket
            // pourraient raconter deux choses différentes de la même géométrie.
            let attendue = temoin.orientation(position);
            assert_eq!(attendue.angle, angle, "l'angle écrit pour {}", champs[0]);
            assert_eq!(
                slug_sens(attendue.sens),
                champs[2],
                "le sens écrit pour {}",
                champs[0]
            );
            slugs.push(champs[0]);
        }
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(
            slugs.len(),
            Position::ALL.len(),
            "les dix positions doivent être écrites, chacune une fois : « {texte} »"
        );

        assert_eq!(
            Geometrie::decoder(&texte).as_ref(),
            Ok(temoin),
            "aller-retour par « {texte} »"
        );

        // Le fichier sur disque finira par un saut de ligne — c'est ce que fait tout éditeur, et
        // c'est ce qu'un `printf '%s\n'` écrit. Le décodeur doit l'accepter, sinon la géométrie
        // se perd le jour où quelqu'un ouvre le fichier pour le lire.
        let avec_fin_de_ligne = format!("{texte}\n");
        assert_eq!(
            Geometrie::decoder(&avec_fin_de_ligne).as_ref(),
            Ok(temoin),
            "un saut de ligne final ne change pas une géométrie"
        );
    }

    // Les deux chemins de construction se rejoignent : régler les dix orientations une par une
    // sur la géométrie mesurée doit donner **exactement** la géométrie décodée du même texte.
    // Sans cette égalité, `decoder` pourrait rendre une table de coordonnées inventée, différente
    // de la table mesurée, et rien ne le dirait — les animations tourneraient juste dans un
    // boîtier imaginaire dès qu'on aurait touché à la géométrie une fois.
    let mut par_definir = Geometrie::mesuree();
    for (i, position) in Position::ALL.iter().enumerate() {
        let (angle, sens) = orientation_variee(i);
        par_definir.definir(*position, orientation(angle, sens));
    }
    assert_eq!(
        par_definir,
        geometrie_variee(),
        "définir les dix orientations ou décoder le texte qui les porte doit donner la même chose"
    );

    // Et l'accesseur rend ce qui a été posé, sur les dix positions.
    for (i, position) in Position::ALL.iter().enumerate() {
        let (angle, sens) = orientation_variee(i);
        assert_eq!(
            par_definir.orientation(*position),
            orientation(angle, sens),
            "l'orientation de {}",
            position.slug()
        );
    }
}

// ---------------------------------------------------------------------------
// 6 — une géométrie invalide est refusée (test d'intention n° 11)
// ---------------------------------------------------------------------------

#[test]
fn une_geometrie_invalide_est_refusee_en_nommant_la_ligne_et_le_champ() {
    // Test d'intention n° 11 de l'issue — « Une géométrie invalide (angle hors 0–359, sens
    // inconnu, ventilateur inconnu) est refusée », et contrat d'API — « refus **nommant** le
    // champ fautif ».
    //
    // Ce texte vient de deux endroits : du socket, donc d'une frappe, et de
    // `/etc/reverb/geometrie.conf`, donc d'un fichier que root peut avoir édité à la main. Une
    // ligne mal comprise et silencieusement ignorée donnerait un ventilateur à l'orientation
    // d'usine au milieu de neuf autres corrigés — le seul défaut de rendu qu'on ne penserait
    // jamais à aller chercher dans un fichier de configuration.
    let base: Vec<String> = texte_uniforme(90, Sens::Horaire)
        .lines()
        .map(str::to_owned)
        .collect();

    /// Remplace la ligne d'indice donné et rend le texte complet.
    fn avec_ligne(base: &[String], indice: usize, ligne: &str) -> String {
        let mut lignes = base.to_vec();
        lignes[indice] = ligne.to_owned();
        lignes.join("\n")
    }

    let premier = Position::ALL[0].slug();
    let cas: [(String, &str); 12] = [
        // L'angle : la borne exacte, puis au-delà, puis ce qui n'est pas un nombre.
        (format!("{premier} 360 horaire"), "angle"),
        (format!("{premier} 361 horaire"), "angle"),
        (format!("{premier} 4000 horaire"), "angle"),
        (format!("{premier} -90 horaire"), "angle"),
        (format!("{premier} midi horaire"), "angle"),
        (format!("{premier} 90.5 horaire"), "angle"),
        // Le sens : un mot voisin, une casse différente, un vide.
        (format!("{premier} 90 diagonal"), "sens"),
        (format!("{premier} 90 Horaire"), "sens"),
        (format!("{premier} 90 anti-horaire"), "sens"),
        // La position : un ventilateur qui n'existe pas, et le nom d'affichage au lieu du slug.
        ("milieu-du-plafond 90 horaire".to_owned(), "position"),
        ("radiateur_haut 90 horaire".to_owned(), "position"),
        ("RADIATEUR-HAUT 90 horaire".to_owned(), "position"),
    ];

    for (ligne, champ) in &cas {
        let texte = avec_ligne(&base, 0, ligne);
        let erreur =
            Geometrie::decoder(&texte).expect_err("une géométrie invalide n'est pas une géométrie");
        assert_eq!(
            &erreur.champ, champ,
            "« {ligne} » : le refus doit nommer le champ fautif"
        );
        assert!(
            !erreur.raison.is_empty(),
            "« {ligne} » : le refus doit dire pourquoi"
        );

        let message = erreur.to_string();
        assert!(
            message.contains(champ) && message.contains(erreur.raison.as_str()),
            "le message doit dire lequel et pourquoi : « {message} »"
        );
        let _: &dyn std::error::Error = &erreur;
    }

    // Une ligne à trop peu ou trop de champs est refusée, pas devinée ni tronquée. Critère
    // d'acceptation de l'issue : « jamais appliqué de travers en silence ».
    for ligne in [
        premier.to_owned(),
        format!("{premier} 90"),
        format!("{premier} 90 horaire bidule"),
        format!("{premier} 90 horaire 45"),
        format!("{premier}90 horaire"),
    ] {
        assert!(
            Geometrie::decoder(&avec_ligne(&base, 0, &ligne)).is_err(),
            "« {ligne} » n'a pas la forme <position> <angle> <sens>"
        );
    }

    // Le numéro de ligne doit désigner **la** ligne fautive, sinon il n'aide pas à corriger le
    // fichier. Le contrat ne dit pas si la première ligne porte le numéro 0 ou le numéro 1 : le
    // test n'en tranche rien, il exige seulement que le numéro **suive** la faute, ligne à ligne.
    let mut numeros = Vec::new();
    for indice in 0..base.len() {
        let position = Position::ALL[indice];
        let texte = avec_ligne(&base, indice, &format!("{} 999 horaire", position.slug()));
        let erreur = Geometrie::decoder(&texte).expect_err("999° n'est pas un angle");
        assert_eq!(erreur.champ, "angle");
        numeros.push(erreur.ligne);
    }
    assert!(
        numeros[0] <= 1,
        "la première ligne se numérote 0 ou 1, pas {}",
        numeros[0]
    );
    for (indice, numero) in numeros.iter().enumerate() {
        assert_eq!(
            *numero,
            numeros[0] + indice,
            "faute posée sur la ligne d'indice {indice}, erreur signalée à la ligne {numero} — \
             le numéro doit suivre la faute"
        );
    }

    // Rien ne panique, quelle que soit l'entrée : ce texte vient d'un fichier, et un fichier peut
    // être n'importe quoi — vide, binaire, coupé au milieu d'un caractère multioctet.
    for texte in [
        "",
        "\n",
        "\n\n\n",
        "   ",
        "\u{feff}radiateur-haut 90 horaire",
        "radiateur-haut 90 horaire\u{0}",
        "🌈 90 horaire",
        "radiateur-haut 🌈 horaire",
        "radiateur-haut 90 🌈",
        "-",
        "radiateur-haut 90 horaire\nradiateur-haut 90 horaire",
    ] {
        // La valeur n'est pas ce qu'on vérifie : c'est que l'appel **revient**.
        let _ = Geometrie::decoder(texte);
    }
}

// ---------------------------------------------------------------------------
// 7 — définir une orientation ne déplace qu'un ventilateur
// ---------------------------------------------------------------------------

#[test]
fn definir_une_orientation_ne_deplace_que_les_led_de_ce_ventilateur() {
    // Contrat d'API — « `definir` sur une position ne change **que** les LED de ce ventilateur ».
    //
    // C'est la promesse de l'issue : « remonter un ventilateur à l'envers se rattrape par une
    // commande ». Une correction qui déborderait sur les neuf autres se verrait à l'œil sans
    // qu'on comprenne pourquoi, et pousserait à corriger le suivant, puis le suivant.
    //
    // Les deux moitiés du test comptent autant l'une que l'autre. Que les autres ne bougent pas
    // dit que la commande est confinée ; que **celui-là** bouge dit qu'elle sert à quelque chose.
    // Une implémentation qui rangerait l'orientation sans la faire entrer dans le calcul des
    // coordonnées passerait la première moitié sans broncher.
    for cible in Position::ALL {
        let mut avant = Geometrie::decoder(&texte_uniforme(0, Sens::Horaire))
            .expect("géométrie uniforme valide");
        let mut apres = avant.clone();

        // Deux orientations qui diffèrent par l'angle **et** par le sens : quelle que soit celle
        // que porte la géométrie de départ, le rendu doit distinguer les deux.
        avant.definir(cible, orientation(0, Sens::Horaire));
        apres.definir(cible, orientation(90, Sens::Antihoraire));

        assert_eq!(
            apres.orientation(cible),
            orientation(90, Sens::Antihoraire),
            "{} porte l'orientation qu'on vient de lui donner",
            cible.slug()
        );

        // Les neuf autres ventilateurs, LED par LED.
        for autre in Position::ALL {
            if autre == cible {
                continue;
            }
            assert_eq!(
                avant.orientation(autre),
                apres.orientation(autre),
                "régler {} a changé l'orientation de {}",
                cible.slug(),
                autre.slug()
            );
            for led in 0..LEDS_PER_FAN as usize {
                assert_eq!(
                    avant.led_ventilateur(autre, led),
                    apres.led_ventilateur(autre, led),
                    "régler {} a déplacé la LED {led} de {}",
                    cible.slug(),
                    autre.slug()
                );
            }
        }

        // Et les quatre barrettes, qui ne sont montées sur aucun ventilateur.
        for slot in 0..SLOT_COUNT {
            for led in 0..LEDS_PER_STICK {
                assert_eq!(
                    avant.led_barrette(slot, led),
                    apres.led_barrette(slot, led),
                    "régler {} a déplacé la LED {led} de la barrette {slot}",
                    cible.slug()
                );
            }
        }

        // Le ventilateur visé, lui, doit avoir bougé : une orientation qui ne déplace aucune LED
        // n'est pas une orientation, c'est un champ décoratif.
        let deplacee = (0..LEDS_PER_FAN as usize)
            .any(|led| avant.led_ventilateur(cible, led) != apres.led_ventilateur(cible, led));
        assert!(
            deplacee,
            "aucune LED de {} n'a bougé entre 0° horaire et 90° antihoraire — l'orientation ne \
             sert donc à rien",
            cible.slug()
        );

        // Le ventilateur reste le même objet physique : ses huit LED restent huit, distinctes et
        // finies. Une rotation ne perd pas de LED en route.
        let mut apres_led: Vec<(u32, u32, u32)> = (0..LEDS_PER_FAN as usize)
            .map(|led| {
                let point = apres
                    .led_ventilateur(cible, led)
                    .expect("les huit LED existent après une rotation");
                (point.x.to_bits(), point.y.to_bits(), point.z.to_bits())
            })
            .collect();
        apres_led.sort_unstable();
        apres_led.dedup();
        assert_eq!(
            apres_led.len(),
            LEDS_PER_FAN as usize,
            "après rotation, deux LED de {} sont au même endroit",
            cible.slug()
        );
    }
}
