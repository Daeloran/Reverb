# Reverb

Contrôle de l'éclairage RGB sous Linux, taillé pour une machine précise : 10 ventilateurs
NZXT F140 RGB Core, un Kraken Elite 2023 et 4 barrettes Corsair Dominator Titanium DDR5.

Un démon léger tient le matériel, une fenêtre le pilote.

## Pourquoi

OpenRGB détecte et colore ce matériel, mais laisse trois manques :

1. **Aucune animation.** Seul son mode *Direct* produit un effet visible ; les modes
   d'animation sont acceptés sans erreur et n'allument rien.
2. **Ergonomie.** Colorer un ventilateur demande contrôleur → zone → menu déroulant.
   Aucune notion de « celui du haut » ou « ceux du radiateur ».
3. **Rien pour l'écran du Kraken, rien pour la RAM.**

Les trois protocoles ont donc été **rétro-ingénierés depuis Windows le 2026-07-30**, décodés
puis **rejoués avec succès sur le matériel**. Ils couvrent les modes d'animation exécutés par
le firmware, le pilotage LED par LED, l'écran 640×640 et les barrettes de RAM.

## État

| Élément | État |
|---|---|
| Protocole ventilateurs NZXT | ✅ décodé et validé |
| Protocole écran Kraken | ✅ décodé, envoi d'image reproduit |
| Protocole RAM Corsair | ✅ décodé, adresses `0x18`–`0x1b`, éclairage reproduit |
| Cartographie physique des 10 canaux | ✅ établie |
| Outil de validation en ligne de commande | ✅ les trois cibles pilotées |
| Démon | ✅ éclairage sans fenêtre, descripteurs tenus |
| Géométrie du boîtier | ✅ mesurée le 2026-07-31 |
| Catalogue d'animations | ✅ dix familles, huit directions dont deux locales |
| Interface Slint | ✅ trois colonnes, onglets, profils, dix familles, composition |
| Écran du Kraken dans le démon | ✅ luminosité, cadran, image, GIF |
| Profils — une ambiance sous un nom | ✅ éclairage, zones et écran, deux exemples livrés |
| Composition de l'écran | ✅ un fond, quatre informations, cinq ancres dans le disque |
| La fenêtre expose tout le protocole | ✅ vérifié par des tests de couverture, pas à l'œil |
| La dalle n'arrête plus le boîtier | ✅ son propre fil, et toute question HID bornée dans le temps |
| Un Kraken muet, le démon le dit et attend qu'on demande | ✅ `repare <source>` — trois resets USB bornés, sur son propre fil, puis redécouverte |
| Régulation des ventilateurs | ✅ les trois canaux sans mode auto, sur la courbe du liquide |
| Un canal qui régule seul ne se défait pas d'un geste | ✅ verrou dans la fenêtre, refus en ligne de commande |
| La régulation se pilote à la souris | ✅ prise en charge, courbe éditée **par points sur son tracé** |

## Ce que les protocoles permettent

| Cible | Animation | Coût du démon |
|---|---|---|
| Ventilateurs | **par le firmware** | écrit une fois, puis dort |
| Écran — température liquide | **par le firmware** | rien |
| Écran — image, cadran, GIF | l'hôte | 1,2 Mo toutes les 25 s, 2 s ou 100 ms |
| **RAM** | **l'hôte, obligatoirement** | boucle SMBus permanente |

La RAM est la seule contrainte temps réel. Tout le reste est en écriture unique — le démon
est au repos absolu le reste du temps.

⚠️ **Aucune persistance matérielle** : rien ne survit au redémarrage. Le démon rejoue
l'initialisation et réapplique les couleurs au démarrage, depuis
[l'état qu'il a conservé](#léclairage-retrouvé).

## Architecture

Aucune dépendance de runtime — l'OS est immuable, un binaire unique ne casse pas à une
montée d'image.

```
reverb-gui  (fenêtre Slint — maquette du boîtier, animations, ventilateurs)
   │  socket Unix : « lighting » lit l'état, « watch » reçoit les images
   ▼
reverb-daemon
   ├── write()      ──►  /dev/hidraw*        ventilateurs, GRB
   ├── I2C_SMBUS    ──►  /dev/i2c-8 0x18..1b RAM Corsair, RGB
   ├── write()      ──►  sysfs pwm*          vitesse des canaux régulés
   ├── fil de l'écran ──► usbfs 1e71:300c    dalle du Kraken, BGR
   │      ▲ une image déposée, un verdict rendu — jamais d'attente
   └── fil de réparation ──► USBDEVFS_RESET  sur demande « repare », jamais seul
          ▲ un état déposé, un constat rendu — trois gestes au plus

reverb  (outil de validation, garde l'usbfs ──► 1e71:300c bulk, écran Kraken, BGR)
   │
   ├── reverb-hw     (les quatre chemins d'E/S — hidraw, usbfs, i2c, hwmon)
   ├── reverb-anim   (géométrie du boîtier + catalogue d'animations — pur)
   └── reverb-proto  (encodage des trames, conversions, CRC-8, protocole IPC,
                      courbe de régulation — pur)
```

`reverb-anim` est importé par le démon **et** par la fenêtre : la maquette place ses LED avec la
géométrie qui sert à calculer les images, et affiche les couleurs que le démon a réellement
envoyées au bus — pas une réimplémentation qui divergerait à la première animation ajoutée d'un
seul côté.

⚠️ **Trois ordres de composantes différents** : ventilateurs en **GRB**, écran en **BGR**,
RAM en **RGB**. Une erreur ici ne produit aucun message, juste une mauvaise couleur.

## Le démon

```bash
sudo groupadd -f reverb && sudo usermod -aG reverb "$USER"   # puis se reconnecter
cargo build --release
sudo install -m 0755 target/release/reverbd /usr/local/bin/
sudo install -m 0644 packaging/reverbd.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now reverbd

echo 'status'            | socat - UNIX-CONNECT:/run/reverb/reverbd.sock
echo 'light all ff00ff'  | socat - UNIX-CONNECT:/run/reverb/reverbd.sock
echo 'animate vague'     | socat - UNIX-CONNECT:/run/reverb/reverbd.sock
```

**Il existe parce qu'ouvrir coûte cher.** Mesuré sur SHYNAEL : ouvrir un `/dev/hidraw*` prend
**51 ms**, y écrire une trame de 64 octets **~1 ms**. Le coût est entièrement dans l'ouverture.
Un outil qui rouvre à chaque trame plafonne à une image et demie par seconde ; un processus qui
garde ses descripteurs tient trente images.

| | Coût d'une image | Cadence |
|---|---|---|
| `reverb paint --all` (rouvre douze fois) | 643 ms | 1,5 img/s |
| démon, descripteurs tenus | 52 ms | 21 img/s |
| démon, + les cibles inchangées sautées | **12 ms** | **31 img/s** |

Le second saut vient de ce qu'aucune cible n'a de watchdog : réécrire une couleur identique ne
fait que consommer du bus. Dans une comète, 24 LED sur 124 sont allumées.

⚠️ **Ce gain dépend de l'animation, et il disparaît quand toutes les LED changent.** Le cache ne
saute que ce qui n'a pas bougé :

| animation | cibles réécrites (sur 14) | cadence |
|---|---|---|
| `balayage` | 3 | ~90 img/s |
| `comete`, `pouls` | 5 | ~45 img/s |
| `artifice` | 12 | ~23 img/s |
| `vague`, `respiration`, `arc-en-ciel`, `braise`, `rotation`, `scintillement`, `bougie`, `nuee` | **14** | **~20 img/s** |

`thermique` ne figure pas dans ce tableau : à température stable elle **ne change pas d'image**, et
le cache saute alors les quatorze cibles. C'est la seule animation du catalogue qui laisse le démon
au repos.

Vingt images par seconde n'est pas un décrochage : c'est le **plancher physique** de ce matériel
— 29,5 ms de trames HID plus 21,6 ms de blocs SMBus, dont ~3 ms par bloc sur le fil à 100 kHz.
Rien de logiciel n'en descend. Ces animations ont une période de quatre secondes, où vingt images
par seconde restent continues à l'œil ; ce qui saccaderait à cette cadence, ce sont justement les
motifs rapides — et eux tiennent 50 à 100.

`cargo run --release --example densite -p reverb-anim` recalcule ce tableau sans matériel.

Tant que le démon tourne, `reverb set|paint|ram|fan|curve` **refuse d'écrire** — un seul processus
détient les bus (ADR-002), et deux écritures SMBus qui se croisent corrompent une transaction.
`reverb list|modes|fans` continuent de marcher, et `reverb screen` aussi : depuis #33 il **passe
par le démon** au lieu d'écrire lui-même. Seule `screen --mire` reste en direct, et rejoint donc la
liste des refusées.

### Les animations

```bash
echo 'animate comete couleur=ff00ff vitesse=5 direction=horaire'
echo 'animate arc-en-ciel direction=avant-arriere'
echo 'animate off'
```

Treize familles, réglables par `couleur` (six chiffres hexadécimaux), `vitesse` (1 à 10) et
`direction`. Chacune n'accepte que ce qu'elle sait porter — une clé de trop fait refuser la
commande **entière**, pas seulement la clé.

| famille | ce qu'elle fait | réglages |
|---|---|---|
| `vague` | l'onde plane, le long de la direction | couleur, vitesse, direction |
| `comete` | une tête vive suivie d'une traînée | couleur, vitesse, direction |
| `respiration` | le boîtier respire, et la respiration se propage | couleur, vitesse, direction |
| `arc-en-ciel` | le spectre déroulé — elle produit ses teintes | vitesse, direction |
| `balayage` | une bande nette dont on voit la limite bouger | couleur, vitesse, direction |
| `braise` | **un lit de braises, sans axe ni cycle** | couleur, vitesse |
| `rotation` | **chaque anneau tourne sur lui-même** | couleur, vitesse |
| `thermique` | **la couleur suit une sonde** | vitesse, **sonde** |
| `pouls` | **une onde sphérique née à la pompe** | couleur, vitesse |
| `scintillement` | **des LED s'allument au hasard** | couleur, vitesse |
| `bougie` | **chaque LED vacille seule, par creux irréguliers** | couleur, vitesse |
| `nuee` | **un champ de bruit dérive à travers le boîtier** | couleur, vitesse |
| `artifice` | **des éclats naissent au hasard et se propagent en sphère** | couleur, vitesse |

Partout où `couleur` figure, **`palette` la remplace** (voir ci-dessous).

Huit directions : `bas-haut`, `haut-bas`, `avant-arriere`, `arriere-avant`, `horaire`,
`antihoraire`, et les deux **locales** — `bords-centre`, `centre-bords`.

#### `bougie`, `nuee`, `artifice` — trois motifs repris de WLED

```bash
echo 'animate bougie palette=lava vitesse=2'
echo 'animate nuee palette=atlantica'
echo 'animate artifice couleur=ff40ff vitesse=6'
```

WLED a une centaine d'effets, et **la plupart sont 1D** : ils indexent les LED le long d'un ruban.
Reverb refuse ce modèle — traiter les 124 LED comme une file d'attente ferait sauter un motif d'un
ventilateur à l'autre dans l'ordre arbitraire où les canaux sont énumérés. Trois familles échappent
à cette limite, pour deux raisons distinctes : **par LED** (la disposition n'entre pas) ou **par
champ spatial** (remplacer l'indice 1D par la position 3D réelle rend l'effet *meilleur*, pas
approximatif).

| famille | origine WLED | ce qui a dû être réécrit |
|---|---|---|
| `bougie` | Candle Multi | la marche aléatoire, qui était **à état** |
| `nuee` | Noise Pal | l'indice de ruban, devenu une **position 3D** |
| `artifice` | Fireworks | rien d'équivalent au catalogue : le premier motif **événementiel** |

⚠️ **Aucune n'accepte `direction`**, et elles rejoignent `braise`, `rotation`, `pouls` et
`scintillement` : `bougie` est LED par LED, `nuee` se déforme sur place, `artifice` naît d'origines
tirées. Aucune de ces grandeurs n'est l'axe de personne.

⚠️ **Les trois lisent l'horloge de 1021 pas, pas le cycle de 120.** Elles ne se referment donc pas
sur le cycle du catalogue, comme `scintillement` et `braise`. Leur demander de boucler serait leur
demander de redevenir périodiques.

##### `bougie` — la marche aléatoire, sans état

WLED tient par LED un triplet `(niveau, cible, pas)` et tire `hw_random8()` à chaque arrivée.
`image()` étant **pure**, la marche se réécrit sur un hachage de `(numéro de LED, segment)` : même
allure — maintiens irréguliers, rampes, creux —, rendu reproductible.

⚠️ **Deux tirages *multipliés*, et non moyennés.** C'est le seul détail qui décide si ça ressemble à
une bougie. La moyenne donne une loi triangulaire **centrée**, donc un niveau qui passe la moitié de
son temps sous les deux tiers : mesuré sur prototype, **41 % du temps au-dessus de 0,7**, ce qui est
un stroboscope. Le produit concentre la masse près du plein éclat : **85 %**. Le plein éclat est un
**plafond** — une bougie faiblit, elle ne surbrille pas.

##### `nuee` — le bruit en trois dimensions, pas en une

WLED échantillonne `perlin8(i × échelle, dérive + i × échelle)` : deux axes, mais le premier est
l'**indice** de la LED. Ici c'est la **position réelle** qui est échantillonnée, si bien que deux LED
voisines de deux ventilateurs différents partagent leur couleur.

⚠️ **La dérive porte sur une *quatrième* dimension**, jamais sur un décalage le long d'un axe.
Décaler ferait *défiler* le motif — c'est-à-dire lui rendrait exactement l'axe qu'il ne doit pas
avoir, le défaut que `braise` a porté jusqu'à #119. Mesuré avant d'écrire une ligne : deux points
séparés de 20 mm diffèrent **6,6 fois moins** que deux points séparés de 250 mm, et aucun décalage
n'améliore la ressemblance avec une image antérieure — le meilleur essayé fait 0,1719 contre 0,1697
sur place, donc **pire**.

⚠️ **Sa vitesse de dérive a été choisie par mesure.** La première valeur essayée déformait le champ
si peu qu'il ne changeait pas mesurablement en trente pas, et « rien ne le reconstitue » ne voulait
alors plus rien dire — le test d'intention le refuse par son propre appareil, avant même de mesurer
une translation.

##### `artifice` — le premier motif événementiel du catalogue

Tout le reste est continu. Ici un éclat a une **date de naissance** et une **origine**, toutes deux
tirées de son numéro : l'intensité ne dépend que de la **distance à l'origine** et du temps écoulé —
la géométrie de `pouls`, mais depuis un point qui change, et avec une extinction.

⚠️ **Ses quatre constantes se tiennent, et la première rédaction les avait toutes trop lentes d'un
ordre de grandeur** : le boîtier restait uni, le front n'atteignant jamais que 0,32 du cube unité
quand sa diagonale vaut 1,73. Mesuré par le test d'intention avant qu'aucun œil ne le voie.

##### Ce qui a été laissé de côté

- **Plasma** — transposable, mais très proche de `vague` munie d'une palette. Une famille de plus
  pour une différence que l'œil ne ferait pas.
- **Chase, Running, Scanner, Theater, Larson, Meteor** — 1D par construction.
- **Glitter, Sparkle** — déjà couverts par `scintillement`.

⚠️ **Les trois sont réécrites, jamais recopiées** : le code de WLED est du C++ à état, lié à son API
de segments. La question de licence est traitée avec [les palettes](#les-palettes--douze-dégradés-repris-de-wled),
qui, elles, reprennent des données.

#### Les palettes — douze dégradés, repris de WLED

```bash
echo 'animate vague palette=light-pink vitesse=4'
echo 'animate braise palette=lava'
echo 'animate pouls palette=atlantica'
```

Une palette est un **dégradé à arrêts** dont l'animation tire ses teintes, à la place de sa
couleur unique. Douze sont livrées, reprises de WLED :

| | | | |
|---|---|---|---|
| `light-pink` | `lava` | `ocean` | `paysage` |
| `couchant` | `aurore` | `atlantica` | `sakura` |
| `nuit-avril` | `glace` | `orange-teal` | `sorbet` |

⚠️ **Ça ne coûte aucune pastille de plus, et c'est la raison d'être du choix.** Chaque famille
calcule déjà une **position scalaire** par LED — la projection sur l'axe demandé, la distance à la
pompe, l'angle sur l'anneau, la traversée du ventilateur. C'est exactement l'entrée d'une palette.
Les douze **multiplient** donc les huit familles qui se colorent, au lieu d'en ajouter.

⚠️ **L'index de palette est le scalaire qui place le motif ; la luminosité ne change pas.** C'est
le modèle de WLED, et c'est ce qui garde à chaque famille son caractère : une vague ondule
toujours, elle change seulement de teinte en chemin. Indexer sur la *luminosité* ferait de
`balayage`, dont l'intensité ne vaut que 0 ou 1, un motif à deux couleurs — la palette y serait
invisible.

##### Un index qui défile parcourt le dégradé en aller-retour

⚠️ **Une palette n'est pas un cercle**, et c'est ce qui faisait sauter la couleur une fois par
cycle — « on voit clairement un rafraîchissement à chaque tour », relevé devant le boîtier. Son
premier et son dernier arrêt sont deux couleurs différentes ; un index **cyclique** projeté droit
dessus repassait de 255 à 0, et la LED sautait de tout l'écart :

| palette | écart entre ses deux bouts | | palette | écart |
|---|---|---|---|---|
| `lava`, `glace`, `orange-teal` | **255** | | `couchant` | 207 |
| `paysage` | 225 | | `sorbet` | 153 |
| `atlantica` | 142 | | `light-pink` | 119 |
| `ocean` | 101 | | `sakura` | 27 |
| `aurore`, `nuit-avril` | **0** | | | |

Sur onze palettes sur douze, le saut mesuré d'une image à la suivante **égalait cet écart à deux
unités près**. Le pire saut d'une LED, sur un cycle entier :

| famille | sans palette | avant | après |
|---|---|---|---|
| `vague` | 20 | **254** | **138** — et 25 sur `lava`, 26 sur `glace`, 19 sur `orange-teal` |
| `respiration` | 17 | **254** | **138** |

Les trois familles dont l'index défile — `vague`, `respiration`, `rotation` — parcourent donc le
dégradé **dans un sens puis dans l'autre**. C'est l'analogue exact de la fonction périodique par
laquelle la luminosité passait déjà, et que la teinte n'avait pas.

⚠️ **On replie le parcours, jamais la palette.** Boucler le dégradé — un arrêt virtuel qui ramène à
la couleur de départ — inventerait une rampe que WLED n'a pas, et obligerait à choisir quelle part
du cycle lui donner : un réglage arbitraire de plus. L'aller-retour est continu et périodique par
construction, sans réglage, et il ne touche pas aux données reprises.

⚠️ **Le prix est une traversée deux fois plus rapide**, donc les rampes internes raides sont
franchies deux fois plus vite. Sur `nuit-avril`, dont les deux bouts sont **déjà identiques**, le
pire saut passe de 92 à 138 : la correction tombe à côté sur la seule palette qui n'avait pas le
défaut. Les onze autres y gagnent.

⚠️ **`rotation` saute toujours de 248 sans qu'aucune palette y soit pour rien** : son arête de
luminosité (`1 - fraction(angle - temps)`) est voulue, et reste. La teinte, elle, est continue.

⚠️ **Sur `vague`, une extrémité du dégradé est rendue noire** — son enveloppe d'intensité s'annule
pile au point de rebroussement. C'est une conséquence de la correction, pas un défaut.

⚠️ **Les huit familles à index borné ne parcourent rien : leur 0 et leur 1 sont les deux bouts du
motif**, pas deux instants successifs. Cinq sont inchangées à l'octet près — `comete`, `balayage`,
`braise`, `pouls`, `scintillement`. Trois ont été **recadrées**, et c'est le même défaut pris par
l'autre bout : `fraction(1.0)` vaut `0.0`, si bien qu'une **bougie à plein niveau** rendait le
*premier* arrêt de la palette au lieu du dernier — sous `lava`, du noir au lieu du blanc. `bougie`,
`nuee` et `artifice` atteignent exactement 1,0 ; elles rendent désormais la fin du dégradé.
`Palette::echantillon` promettait déjà « hors bornes, la couleur de borne », et c'est le `fraction`
qui défaisait cette garantie précisément à la borne.

⚠️ **Sans palette, rien n'a changé du tout** — un test d'intention fige les treize familles, huit
directions et soixante pas, octet pour octet. La correction ne touche que le chemin qui échantillonne
un dégradé.

⚠️ **`couleur` et `palette` ensemble sont refusés**, et la commande entière avec. Ce n'est pas une
clé de trop — les deux sont acceptées séparément — mais une **ambiguïté** : rien ne dirait laquelle
décide de la teinte, et en choisir une en silence serait le réglage qui ment. La même exclusion
vaut dans les fichiers d'état : `eclairage.conf` porte l'une **ou** l'autre, jamais les deux, sinon
le démon refuserait au redémarrage suivant sa propre écriture — le défaut de #69.

⚠️ **Sous palette, la couleur n'est pas conservée.** C'est la contrepartie de l'exclusion :
revenir à « aucune palette » rend la couleur par défaut, pas celle d'avant. La persister quand même
supposerait d'écrire les deux clés.

⚠️ **Un réglage purement additif.** Une commande sans `palette` rend **exactement** l'image
d'avant — un test d'intention le fige octet pour octet —, donc les profils et `eclairage.conf`
écrits avant continuent de s'appliquer sans être réécrits. C'est l'inverse de ce qu'a coûté #119 sur
`braise`.

##### Provenance et licence

Les douze dégradés viennent de **WLED** (`wled00/palettes.cpp`), qui les a lui-même importés de
**cpt-city** (`seaviewsensing.com`) en leur appliquant sa correction gamma. Les valeurs sont
recopiées **telles que WLED les porte** : ce sont celles qui produisent, sur une bande, l'aspect
qu'on connaît. Les recalculer donnerait un autre rendu sous le même nom.

⚠️ **WLED n'est plus sous MIT — il est passé en EUPL-1.2**, une licence copyleft. Reverb est sous
**GPL-3.0-or-later**, et l'annexe de l'EUPL 1.2 liste GPL-3.0 parmi les licences compatibles : la
reprise est donc licite, à condition d'attribuer et de rester GPL. C'est un heureux hasard — en
MIT, Reverb aurait dû changer de licence pour une douzaine de dégradés.

⚠️ **Seules les données sont reprises, jamais le code.** WLED interpole en C++ sur des
`CRGBPalette16` de FastLED ; ici l'interpolation est réécrite, sur les arrêts eux-mêmes et sans
quantifier à seize entrées.

#### Les directions locales — le motif se répète sur chaque objet

```bash
echo 'animate vague direction=bords-centre'
echo 'animate comete direction=centre-bords'
```

Les six premières directions projettent une LED sur un **axe du boîtier** : l'onde traverse les
quatorze objets comme un volume. Les deux locales la projettent sur sa **distance au milieu de
l'objet qui la porte** — sa barrette, son ventilateur. Le motif se répète donc à l'identique sur
chacun des quatorze, ce que fait iCUE sur la RAM : il part des deux bords de *chaque* barrette et
converge vers son milieu. Douze motifs pour deux directions écrites.

⚠️ **Les deux LED d'extrémité d'un objet s'allument toujours ensemble** — c'est la définition du
motif, et la symétrie est calculée sur les **indices**, jamais sur une position flottante. Mesuré :
`2 × (9/10) − 1` ne vaut pas exactement `|2 × (1/10) − 1|` en `f32`, et les LED 1 et 9 d'une
barrette ressortaient à 153 et 152 — un écart d'une unité sur 255, invisible à l'œil, et une
symétrie fausse.

#### `thermique` — et ce qu'une sonde muette doit montrer

```bash
echo 'animate thermique sonde=kraken2023elite:coolant-temp'
```

Le boîtier passe du **bleu au vert, à l'orange, au rouge** entre 25 °C et 60 °C : le cadran de
l'écran transposé aux 124 LED.

⚠️ **La sonde est exigée, pas seulement acceptée** — seule du catalogue. `Reglages::default()` n'a
aucune valeur sensée à lui donner : il n'existe pas de sonde par défaut, la machine en expose
seize. Un nom inconnu est refusé **en donnant la liste**, et ce refus-là vit dans le démon, seul à
savoir lesquelles existent.

⚠️ **Une sonde qui ne répond plus fait pulser le boîtier en blanc**, jamais la dernière couleur
connue. C'est le mode de défaillance le plus coûteux du projet parce qu'il est rassurant : un
34 °C figé derrière une pompe arrêtée, c'est un circuit qui chauffe sans que rien ne le signale. Le
blanc est achromatique — aucune étape du gradient ne l'est — et il pulse, ce qu'aucune température
ne fait. La quarantaine de #68 s'applique, et le gradient reprend dès que la sonde répond.

La sonde est relue **une fois par seconde**, jamais à la cadence des images : une lecture sysfs sur
un périphérique muet bloque cinq secondes en sommeil non interruptible.

#### `braise`, `rotation`, `pouls`, `scintillement` — quatre motifs qui ne suivent aucun axe

**`braise`** est un lit de braises : des zones chaudes et froides qui respirent et se déplacent, où
aucun sens de lecture ne se devine. Une onde **lente** respire depuis la pompe et porte les sept
dixièmes de l'amplitude, une onde **vive** tourne en sens inverse sur chaque anneau, et un
frémissement d'un quinzième de cycle — tiré du numéro de LED, jamais d'un `rand` — casse ce qui
resterait de régulier. Les deux grandeurs qu'elle croise, la distance à la pompe et l'angle sur
l'anneau, ne sont l'axe de personne.

⚠️ **Elle en suivait un jusqu'à #119, et ça se voyait** — signalé devant le boîtier : « le *sens* de
l'effet a un effet sur l'animation et du coup on voit clairement un pattern ». Le code le disait :
deux ondes planes superposées défilant le long de la direction demandée. La promesse annoncée, « deux
ondes de périodes incommensurables : l'œil n'y voit pas de cycle », valait **dans le temps** — 3 et 7
ne se referment pas — et ne disait rien de l'**espace**, qui est justement ce qu'on regarde. Le
défaut vivait dans la moitié de la propriété que personne n'avait énoncée.

⚠️ **`animate braise direction=…` est désormais refusé, et la commande entière avec** — c'est la
règle du projet, une clé de trop fait rejeter l'`animate` complet. **Un profil enregistré qui porte
cette clé cesse donc de s'appliquer**, et doit être réenregistré : sur SHYNAEL, l'état courant au
moment de l'issue était `anim braise couleur=3939ff vitesse=2 direction=centre-bords`. L'exemple
`forge` livré par le dépôt a été corrigé, les profils personnels non — rien ne les réécrit à votre
place. Accepter la clé pour ne rien en faire aurait été le réglage qui ment, refusé partout ailleurs.

⚠️ **Elle est aussi la seule famille avec `scintillement` à ne pas se refermer sur le cycle du
catalogue.** Elle défilait sur l'horloge qui se replie toutes les cent vingt étapes — quarante
valeurs seulement à la vitesse 3 —, et repart donc à l'identique d'un cycle au suivant ; elle suit
désormais celle de `scintillement`, dont la période de 1021 pas est première et n'a de facteur commun
avec aucune vitesse.

**`rotation`** fait tourner chaque anneau **sur lui-même**, à sa place dans le boîtier. C'est le
motif le plus « ventilateur » du catalogue, et il manquait : `horaire` est une rotation dans le
*volume*, pas d'un anneau. Elle suit l'**angle relevé** de chaque ventilateur, jamais le numéro de
LED — sans quoi le motif tournerait à l'envers sur les trois du plafond, montés antihoraire. Les
barrettes, qui n'ont pas d'anneau, suivent bords↔centre : une RAM éteinte pendant qu'un motif
tourne se lirait comme une panne.

**`pouls`** propage une onde **sphérique** depuis le bloc-pompe. Deux LED à égale distance de lui
s'allument ensemble, quels que soient leur organe et leur axe — la première animation à exploiter
la géométrie comme une distance plutôt que comme une projection.

**`scintillement`** est la seule famille **sans période** : chaque LED pulse à sa cadence et à sa
phase propres, tirées d'un hachage de son numéro. Aucun `rand`, aucune horloge, aucun état — le
rendu doit rester reproductible à l'identique dans la fenêtre et dans le démon, sans quoi
« l'aperçu montre ce que le boîtier reçoit » deviendrait faux sans le dire.

**Un motif traverse le boîtier comme un volume, pas comme une file d'attente.** Chaque LED est
ramenée à sa position le long de la direction demandée, si bien qu'une onde qui monte atteint en
même temps deux LED à la même hauteur — quels que soient leur ventilateur, leur barrette et leur
numéro d'ordre. C'est ce que la [géométrie mesurée](docs/GEOMETRIE.md) rend possible.

⚠️ **Une onde purement plane aplatit ce qui n'a pas d'épaisseur dans sa direction.** Six
ventilateurs sur dix sont couchés : leurs vingt-quatre LED sont exactement à la même hauteur, et
`bas-haut` les allumerait d'un seul bloc. Quatre familles suivent donc l'**écoulement** plutôt que
la seule position : chaque ventilateur est traversé d'un bord à l'autre depuis un point d'entrée
relevé à l'œil (`docs/GEOMETRIE.md`), de sorte que le motif le franchit LED par LED même quand la
direction ne lui donne aucune épaisseur. Sur un ventilateur que la direction n'aplatit pas, cette
traversée **coïncide** avec la position réelle : ce n'est pas un motif plaqué par-dessus la
géométrie, c'est son prolongement là où elle se tait.

`vague` s'en tient à la position, seule du catalogue : elle *est* l'onde plane, et la
démonstration que le boîtier et la RAM sont synchronisés dans l'espace.

### La géométrie

```bash
echo 'geometry'                                            # les dix orientations
echo 'geometry bas-droite angle=210 sens=horaire'          # en corriger une
```

Le protocole ne dit **pas** où commence l'anneau de LED d'un ventilateur ni dans quel sens il
tourne : c'est une donnée de montage (spec §5), relevée à l'œil et conservée dans
`/etc/reverb/geometrie.conf`. Un ventilateur démonté puis remis reprend une orientation
quelconque — d'où une commande plutôt qu'une recompilation. Le démon, qui est root, écrit le
fichier ; **la fenêtre ne l'écrira jamais**, elle demandera par le socket.

### La régulation — les sept ventilateurs que personne ne pilotait

```bash
echo 'regule'                                        # la courbe, et les canaux régulés
echo 'regule nzxtsmart2:fan-1 on'                    # le démon prend ce canal
echo 'regule nzxtsmart2:fan-1 off'                   # il le rend
echo 'regule courbe 35000:30 45000:60 50000:100'     # la courbe, en millidegrés
```

Sept des dix ventilateurs sont sur les trois canaux `nzxtsmart2`, et **rien ne les régulait sous
Linux** : le pilote `nzxt-smart2` n'a aucun mode automatique, sa vitesse est celle que l'hôte
écrit, et personne ne l'écrivait. Mesuré sur SHYNAEL le 2026-08-15, 863 relevés sur 72 minutes de
jeu :

| | min | médiane | max |
|---|---|---|---|
| liquide | 36,9 | 50,7 | **51,3 °C** |
| Tctl | 62,6 | 76,2 | **91,5 °C** |
| duty des trois `nzxtsmart2` | 64 | 64 | **64** — soit 25 %, ~700 tr/min |

**Une seule valeur sur 863 relevés**, pendant que le liquide passait quarante-cinq minutes au-dessus
de 50 °C. Le problème n'était pas une consigne fausse, c'est qu'aucune n'était jamais écrite.

Le démon calcule donc la sienne, sur une courbe réglable :

| liquide | consigne |
|---|---|
| ≤ 35 °C | 30 % |
| 45 °C | 60 % |
| ≥ 50 °C | 100 % |

interpolée entre les paliers, **ramenée aux bornes** au-delà — jamais extrapolée : prolonger la
droite du premier segment donnerait 0 % à 25 °C, c'est-à-dire des ventilateurs à l'arrêt sur un
circuit qui démarre.

⚠️ **La sonde est celle du liquide, et elle seule.** C'est la logique d'un AIO : le liquide bouge
lentement — il a mis quarante minutes à monter — donc les ventilateurs ne pompent pas. C'est aussi
la sonde dont le Kraken se sert pour sa propre courbe firmware, donc les deux régulations restent
cohérentes. Tctl saute de 20 °C entre deux secondes sur un Zen 5 et demanderait un lissage.

⚠️ **Aucun réveil ajouté.** La régulation se greffe sur le tour que la boucle fait déjà — un toutes
les 250 ms au repos, un par image sous animation — et relit le liquide au plus une fois par seconde.
Sans canal régulé, elle rend la main avant même de lire l'horloge : le démon reste au repos absolu.

⚠️ **On n'écrit que ce que le canal ne porte pas** — et non ce qu'on a cru y écrire —, **et
seulement si l'écart vaut deux points**. Le tour relit chacun des canaux régulés, compare la
consigne à ce que le matériel porte vraiment, et n'écrit que l'écart qui le mérite. Le silence reste
celui du cache de LED — aucune de ces cibles n'a de watchdog, et le tour passe une fois par seconde,
soit 86 400 trames par jour pour rien —, mais il est désormais vrai du matériel et non de nos
intentions, et il n'est plus rompu par le bruit de la sonde.

Parce que l'autre version était fausse, mesuré sur SHYNAEL le 2026-08-15 à 15:52, machine au repos :

```
journal   régulation : nzxtsmart2:fan-3 à 46 % (liquide 40.2 °C)
sysfs     pwm3 = 130  →  51 %
```

`fs::write` sur `pwmN` avait rendu `Ok` sans que le matériel applique — piste : `nzxt-smart2`
regroupe les duty de ses canaux dans un même rapport HID. La consigne calculée ne changeant plus, le
cache disait « déjà écrit » et **rien n'était jamais réémis** : le canal est resté figé sur une
consigne d'il y a plusieurs minutes pendant que le démon en annonçait une autre, sans une erreur
nulle part. C'est la leçon que [`docs/VENTILATEURS.md`](docs/VENTILATEURS.md) avait tirée le
2026-07-30 — « restaurer une valeur n'est pas restaurer un comportement » — reprise par l'autre bout.

Ce que la relecture achète, et ce qu'elle coûte :

- une consigne qui n'a pas pris est **rejouée au tour suivant**, indéfiniment et sans intervention.
  Un plafond de tentatives recréerait le défaut qu'on corrige : un canal abandonné en silence à son
  duty d'allumage ;
- **la comparaison est exacte** : l'aller-retour pourcentage → duty → pourcentage est l'identité sur
  les 101 valeurs — un point vaut 2,55 pas de duty, le pas est expansif. Un écart relu est donc un
  écart réel, jamais un artefact d'arrondi. Le seuil ci-dessous ne le contredit pas : il décide de
  la **taille** qu'un écart doit avoir pour valoir une trame, pas de la fiabilité de la mesure ;
- **un canal qu'on ne sait pas relire n'est pas écrit.** Écrire là où on ne mesure pas, c'est refaire
  le défaut à l'identique — annoncer une consigne sans aucun moyen de savoir si elle a pris. La
  fenêtre l'affiche illisible (#100), et c'est plus honnête ;
- le coût est une lecture sysfs par canal régulé et par tour — **0,001 s** sur `nzxtsmart2`, trois
  ordres de grandeur sous les 5 s d'un contrôleur muet. Elle passe par la **quarantaine et sous la
  même clef que `status`** (#88) : un canal qui se tairait est écarté une fois pour les deux chemins,
  pas deux fois pour cinq secondes chacune. Et seuls les canaux **régulés** sont relus : relire tous
  les canaux ferait payer chaque seconde une lecture du Kraken, celui-là même dont on sait qu'il se
  tait ;
- le journal, lui, ne se répète pas. Une réémission part à chaque tour, soit 86 400 lignes par jour
  si on les imprimait toutes — le chiffre même qui justifiait le cache, retourné contre lui. Le démon
  dit l'**entrée** dans l'épisode, en nommant ce que le canal portait, puis se tait, puis dit sa
  **sortie** :

```
attention : régulation : « nzxtsmart2:fan-3 » porte 25 % alors qu'il a reçu 50 % — la consigne est
rejouée à chaque tour, en silence, jusqu'à ce qu'elle prenne
régulation : « nzxtsmart2:fan-3 » porte enfin sa consigne (50 %)
```

⚠️ **Un écart d'un point ne fait rien partir** — c'est du bruit de sonde, pas une information sur le
circuit. Mesuré sur SHYNAEL le 2026-08-15, machine au repos, régulation en place depuis des heures :

```
15:51:10  fan-{1,2,3} à 46 %   (liquide 40.4 °C)
15:51:48  fan-{1,2,3} à 45 %   (liquide 40.1 °C)
15:52:28  fan-{1,2,3} à 46 %   (liquide 40.2 °C)
```

**Trente écritures en huit minutes pour 0,3 °C d'amplitude**, soit ~3 500 par jour et par canal. La
règle de #99 y était respectée à la lettre et ratée en esprit : la consigne *changeait* vraiment —
la sonde bruite de ±0,3 °C, la courbe fait 3 %/°C sur ce segment, donc ce bruit vaut ±1 point — et
la régulation ne se taisait jamais pour autant.

Le seuil est de **deux points de consigne**, et il est **atteint, pas dépassé** : on écrit dès que
l'écart vaut deux. Un seuil « strictement supérieur » ferait de 2 un seuil de 3, et le chiffre ne
voudrait plus dire ce qu'il dit. Deux points font taire le bruit relevé et laissent passer toute
variation dépassant 0,7 °C de liquide — très en deçà d'une montée en charge, qui a mis quarante
minutes à gagner quinze degrés le même jour. Plus large, l'hystérésis se mettrait à retarder cette
montée-là au lieu de faire taire le bruit.

⚠️ **Le seuil porte sur l'écart avec ce que le canal *porte*, jamais avec la dernière consigne
calculée.** C'est la seule grandeur qui compose avec la relecture ci-dessus : un canal bloqué à 25 %
pour une consigne de 46 % montre vingt et un points d'écart et reste réécrit à chaque tour. Mesurée
contre l'intention, l'hystérésis le rendrait **muet dès le premier tour de bruit** — la consigne
repasserait de 46 à 45 sans jamais s'écarter d'un point de la dernière écrite —, et un canal bloqué
redeviendrait invisible.

Trois écritures échappent au seuil, et trois seulement :

- **un canal jamais écrit depuis son activation** part quoi qu'il porte. Rien ne survit au
  redémarrage côté matériel : le laisser sous prétexte qu'il est à un point près, c'est le laisser à
  son duty d'allumage ;
- **les bornes de la courbe**, 0 % et 100 %, partent dès qu'elles diffèrent. C'est le sens même
  d'une borne : « à fond » ne doit pas s'arrêter à 99 — le tableau ci-dessus promet 100 % au-delà de
  50 °C —, ni « à l'arrêt » s'immobiliser à 1 %. En **quitter** une, en revanche, est une consigne
  ordinaire : rester une seconde de plus à plein régime ne coûte rien, ne jamais y arriver si ;
- **le repli d'une sonde muette** part quoi qu'il arrive. C'est la seule écriture du système qui
  **signale une panne**, et l'amortir la rendrait indistincte d'un régime normal.

⚠️ **Contrepartie assumée : sous le seuil, un bruit d'un point et une non-application d'un point
deviennent indistinguables.** Un canal à qui on a écrit 46 et qui n'en porte que 45 n'est plus
rejoué. Ce n'est pas un défaut de la règle, c'est sa définition — les deux situations produisent
littéralement la même lecture, et rien dans ce que le démon relit ne permet de les séparer. Le prix
est d'**un point de duty sur 255**.

⚠️ **Un liquide illisible fait retomber la consigne à 50 %, jamais à la dernière valeur connue.**
C'est le mode de défaillance rassurant que le projet refuse partout ailleurs : une consigne figée à
30 % derrière une sonde morte, c'est un CPU qui chauffe sans que rien ne le signale — et le Kraken
se plante périodiquement. Le repli part **une fois** — puis, comme toute consigne, à chaque tour tant
que le canal ne le porte pas —, et le retour de la sonde reprend la courbe sans redémarrage.

⚠️ **`fan <canal> pwm …` reprend le canal**, et la régulation le lâche : sans cela elle réécrirait la
valeur posée à la main **au tour suivant**, une seconde plus tard, et elle disparaîtrait sans
explication. C'est plus vrai depuis #110 qu'avant lui : la régulation ne compare plus à ce qu'elle a
écrit, mais à ce que le canal porte — donc une valeur posée à la main lui apparaît exactement comme
une consigne qui n'a pas pris.

#### La courbe firmware du Kraken — poser et activer d'un seul geste

Rien à voir avec `regule`, qui est la boucle de l'hôte sur les `nzxtsmart2`. Ici, c'est la courbe
que le **firmware du Kraken** exécute, sur ses deux canaux à lui :

```bash
sudo reverb curve --channel kraken2023elite:pump-speed --point 1:35 --point 25:60 --point 40:100
sudo reverb curve --channel kraken2023elite:pump-speed --point 1:35 ... --enable   # pose ET bascule
echo 'curve kraken2023elite:pump-speed enable 35,35,…,100' | socat - UNIX-CONNECT:/run/reverb/reverbd.sock
```

⚠️ **Poser et activer doivent tenir dans un seul processus, et c'est toute l'issue #104.** Depuis
#97, `fan <canal> auto` refuse tant qu'aucune courbe n'a été téléversée sur le canal — et ce carnet
ne peut vivre **que** dans le processus qui a écrit, les fichiers `tempN_auto_pointM_pwm` étant en
écriture seule et ne se relisant jamais. Deux commandes, c'est deux carnets neufs : le flux en deux
temps qui marchait avant #97 refusait ensuite, **même démon arrêté**.

⚠️ **`set` n'est jamais sous-entendu sur le fil.** Un `enable` facultatif en fin de ligne se lirait
aussi bien comme un oubli que comme un choix, et c'est le geste qui a mis la consigne de la pompe à
0 %.

⚠️ **Les quarante consignes sont un tableau de taille fixe, pas une liste.** Une courbe incomplète
devient irreprésentable au lieu d'être refusée à l'exécution — la règle de `SlotAddress` pour la
RAM, appliquée au seul autre endroit où une écriture partielle coûterait cher : ici, c'est la
régulation de la pompe qui s'arrête.

⚠️ **Une courbe qui descend est refusée sur les deux chemins.** Le socket est une porte de service
pour la fenêtre, jamais une porte dérobée : une consigne qui baisse quand la température monte est
une faute de frappe ou un décalage d'indice, et l'écriture seule la rend invisible jusqu'à la
surchauffe — un canal ne dit jamais quelle courbe il porte. Le plancher de 20 % vaut lui aussi des
deux côtés.

⚠️ **Aucune ligne de réponse ne rend la courbe posée**, et rien ne doit prétendre le contraire. Le
matériel ne la relit pas ; un `curve` qui répondrait par ce qu'il vient d'écrire dirait ce qu'on a
*voulu*, jamais ce que le canal porte — le mode de défaillance rassurant que #110 a corrigé sur la
régulation.

`reverb curve` **passe par le démon quand il tourne**, comme `reverb screen` depuis #33 : quarante
points tiennent sur une ligne de texte, contrairement au mégaoctet d'une image, donc le protocole
n'a pas à être contourné. Démon arrêté, il écrit en direct comme avant.

Les canaux régulés et la courbe vivent dans `/var/lib/reverb/regulation.conf`, relu au démarrage —
et **ce qui a été écrit sur le bus n'y figure pas** : rien ne survit au redémarrage côté matériel,
les canaux repartent à `pwm = 64`, et un démon qui se souviendrait d'avoir déjà écrit 33 % prendrait
la reprise de ses trois canaux pour la réparation d'une écriture perdue — le journal dirait « fan-1
porte 25 % alors qu'il a reçu 33 % » là où il doit dire que le démon vient de prendre ce canal. Ce
que le fichier conserve, c'est l'**intention** : quels canaux, quelle courbe. Un fichier tronqué ou
répété est refusé **en
nommant l'entrée fautive**, et n'empêche jamais le démarrage : le démon part alors sans réguler,
et le dit. Sans canal, il ne décide rien — poser 50 % sur des canaux qu'on n'a pas su relire serait
choisir à la place de l'utilisateur.

#### Pourquoi `regule` et non `fan <canal> auto`

L'issue posait la question : `auto` pourrait devenir « régule ce canal par le moyen disponible ».
Le verbe est resté séparé, et les deux se **partagent** les canaux au lieu de se recouvrir — c'est
le drapeau « sait faire auto » d'une ligne `chan` qui les départage :

| | `fan <canal> auto` | `regule <canal> on` |
|---|---|---|
| canaux | les deux du Kraken | les trois `nzxtsmart2` |
| qui régule | le **firmware**, `pwm_enable = 2` | la boucle du démon |
| sans démon | **tient** | s'arrête |

C'est la troisième ligne qui a tranché : un seul verbe pour les deux ferait disparaître du
protocole la seule différence qui compte le jour où le service ne tourne plus. Deux raisons de
plus : « ce contrôleur sait faire auto » est un fait matériel, lu dans le nom du pilote et figé par
les tests d'intention de #50 — l'élargir, c'était les réécrire ; et `status` aurait contredit la
commande dès la seconde suivante, en répondant `mode manuel` et `sait_faire_auto non` sur un canal
qui venait d'accepter « auto ».

Le sign-post, lui, est posé : `fan nzxtsmart2:fan-1 auto` refuse toujours — « ce contrôleur n'a pas
de mode automatique » (#50) — et ajoute désormais où aller.

**Ce qui n'y est pas** : une courbe par canal — tant que la répartition physique des ventilateurs
par canal reste l'inconnue documentée de [`docs/VENTILATEURS.md`](docs/VENTILATEURS.md), les trois
en partagent une —, toute sonde autre que le liquide, et les deux canaux du Kraken dont le firmware
régule déjà correctement.

Tout cela se pilote aussi [depuis la fenêtre](#la-régulation-depuis-la-fenêtre) depuis #113 — y
compris la courbe, que #99 laissait en hors scope.

### Un canal qui régule seul ne se défait pas d'un geste distrait

```
$ reverb fan --channel kraken2023elite:pump-speed --pwm 50
erreur : « kraken2023elite:pump-speed » n'est piloté par personne côté hôte : c'est le
périphérique qui régule, sur son propre profil. Lui imposer une consigne fixe l'en sortirait, et
aucune commande ne l'y rend — seule une coupure d'alimentation complète. Ajoutez « --manual »
si c'est voulu.
```

Rien n'est écrit — ni le mode, ni la consigne. `--manual` lève le refus, et lui seul.

⚠️ **Le déclencheur est le mode, jamais le nom du contrôleur.** Deux modes refusent : `non-piloté`
et `courbe-de-l'hôte`, les deux où **quelque chose d'autre que l'hôte** décide de la vitesse. Un
canal du Kraken déjà passé en `manuel` n'a plus rien à protéger et laisse passer ; le jour où un
`nzxtsmart2` lirait `non-piloté`, il refuserait comme les autres. Coder « Kraken » donnerait
aujourd'hui le bon résultat sur SHYNAEL — seuls ses deux canaux lisent `non-piloté` — et casserait
au premier pilote qui change, **en silence**.

⚠️ **Ni le drapeau `sait_faire_auto`.** Depuis #97 il vaut « le pilote sait faire auto **et** une
courbe a été posée », donc toujours `non` : il ne dit plus rien du matériel.

⚠️ **Ce n'est pas théorique, et la moitié du garde manquait.** Il visait `0` jusqu'au 2026-08-02,
en annonçant « suit sa courbe firmware et s'adapte à la température » d'un canal qui tournait en
fait à 100 % sans rien réguler (#50) ; il a alors été déplacé sur `courbe-de-l'hôte`, en laissant
derrière lui une exemption écrite noir sur blanc — « un canal en `0` n'a rien à perdre ». Elle ne
valait que si `0` voulait dire 100 %. Or un `0` **lu** dit le contraire (#101) : le pilote n'a
jamais touché ce canal, et le périphérique exécute son propre profil. Le 2026-08-15, la pompe y
suivait le liquide de 35 à 60 %, et un `reverb fan --pwm` l'aurait remplacé sans un mot.

⚠️ **Le refus est un calcul, pas une relecture.** `refus_de_consigne` ne reçoit ni descripteur, ni
canal ouvert, ni chemin : « rien n'est écrit » devient une propriété de sa signature. C'est la règle
du projet — ce qui est testable sans matériel est séparé de ce qui y touche — appliquée à un
garde-fou.

**Le même fait produit le même verdict dans la fenêtre**, où il prend la forme d'un
[cadenas](#le-verrou-dun-canal-qui-régule-seul). Un seul fait matériel, deux portes.

### L'éclairage retrouvé

Le boîtier retrouve seul, après un redémarrage, ce qu'il affichait — une couleur fixe comme une
animation avec ses réglages. Le démon écrit son état dans `/var/lib/reverb/eclairage.conf` à
**chaque changement**, pas à l'arrêt : ce qu'on veut retrouver, c'est justement l'éclairage
d'avant une coupure de courant, qui ne laisse le temps d'écrire nulle part.

Deux natures, cinq fichiers et un répertoire. La géométrie est une donnée de montage, décidée une
fois, et reste dans `/etc` ; l'éclairage, les zones, l'écran,
[la régulation](#la-régulation--les-sept-ventilateurs-que-personne-ne-pilotait) et
[les profils](#les-profils--une-ambiance-sous-un-nom) sont l'état courant du service, réécrits à
chaque commande, et vont dans `/var/lib` (`StateDirectory=reverb`). Les mêler ferait réécrire à
chaque changement de couleur le fichier qui a coûté un relevé au sol.

L'écran suit la même règle : `ecran.conf` garde sa luminosité et **le chemin** de ce qu'il montre,
jamais les pixels. Au redémarrage le démon relit le fichier ; s'il a disparu depuis, la dalle reste
au firmware et le journal le dit **une fois**, sans boucler.

Une [composition](#la-composition--un-fond-et-jusquà-quatre-informations-dessus) y ajoute ses
lignes — son fond, puis un `champ` par ancre. C'est le seul affichage qui ne tienne pas sur une
ligne, et un `ecran.conf` d'avant en a toujours exactement deux : il ne dit jamais « layout », donc
il ne porte jamais de bloc, donc il se relit tel quel.

**Un fichier absent et un fichier disant « noir » ne se confondent jamais.** Le premier est un
premier démarrage : le boîtier s'allume en **bleu pur**, ce qui prouve du même coup que les deux
bus répondent, sans avoir eu à taper une commande. Le second est un choix : `animate off` puis
extinction se retrouve éteint. Sans cette distinction, éteindre volontairement son boîtier le
rallumerait au démarrage suivant.

Un fichier illisible ou tronqué ne bloque pas le démarrage : il est signalé dans le journal, et
l'accueil s'applique. Une entrée absente ou répétée est refusée **en la nommant** — c'est ce qui
rend un fichier tronqué détectable, plutôt que complété au jugé par un éclairage plausible et
faux.

### Les profils — une ambiance sous un nom

Un profil est un **instantané nommé** de tout ce que le boîtier montre : la couche globale, les
zones avec leurs couleurs et leurs animations, et l'écran. On l'enregistre, on le rappelle, le
boîtier reprend exactement ce qu'il affichait.

```bash
echo 'profil save soirée d'\''été'
echo 'profil load soirée d'\''été'
echo 'profil list'
echo 'profil drop soirée d'\''été'
```

Un profil **n'emporte pas la géométrie**. C'est une donnée de montage, relevée à l'œil sous le
bureau, et rappeler une ambiance enregistrée avant qu'un ventilateur ait été démonté puis remis
remettrait l'orientation d'avant — le boîtier se mettrait à tourner à l'envers sans qu'on fasse le
lien.

⚠️ **Un nom peut porter des espaces et des accents** — « soirée d'été », « LAN party ». Il est donc
le **dernier champ de sa ligne** et va jusqu'au bout, comme un chemin d'image : coupé au premier
blanc, il désignerait « soirée », un profil qui n'existe pas.

⚠️ **Un nom ne peut pas désigner un fichier ailleurs.** Le démon est root ; `/`, `\`, `..` et tout
caractère de contrôle sont refusés **en nommant ce qui cloche**. Ce n'est pas une vérification à
l'exécution mais un type — comme `SlotAddress` pour la RAM, viser hors du répertoire est
irreprésentable, et un balayage des 256 valeurs d'octet le vérifie.

**Un profil dont l'image a disparu s'applique quand même.** L'éclairage et les zones sont posés, et
seul l'écran est signalé : un profil à moitié appliqué qui le dit vaut mieux qu'un profil refusé en
bloc parce qu'une photo a été déplacée. Le format est reconnu **au contenu avant que rien ne bouge**,
comme partout depuis #69.

Les profils vivent dans `/var/lib/reverb/profils/`, **un fichier par profil** : en supprimer un ne
réécrit pas les autres, et un profil corrompu n'emporte pas la collection. `profil list` ne décode
d'ailleurs rien — un profil abîmé y **reste visible**, sinon on ne saurait pas quoi réparer.

Deux ambiances d'exemple, `abysse` et `forge`, sont **embarquées dans le binaire** et posées au tout
premier démarrage. `tools/installe.sh` n'y touche pas : il promet de ne jamais écrire dans
`/var/lib/reverb`, et le répertoire doit être créé par `StateDirectory=reverb` pour l'être au bon
propriétaire. Un exemple supprimé exprès ne repousse pas — la condition est l'absence du
**répertoire**, pas celle de chaque fichier.

## La fenêtre

```bash
cargo build --release
sudo install -m 0755 target/release/reverb-gui /usr/local/bin/
sudo install -Dm 0644 packaging/reverb.svg /usr/local/share/icons/hicolor/scalable/apps/reverb.svg
sudo install -Dm 0644 packaging/reverb.desktop /usr/local/share/applications/reverb.desktop
```

**Trois colonnes : ce qu'on vise à gauche, l'objet au centre, ce qu'on lui fait à droite.**

```
 REVERB ●          AMBIANCES  abysse  forge  [soirée d'été]   [nom…] Enregistrer
┌──────────────┬────────────────────────────────┬───────────────────────────────┐
│ CIBLE        │ LE BOÎTIER      [LED][de face] │ ÉCLAIRAGE │ ÉCRAN │ VENTILOS  │
│ [tout][…][…] │                                │ COULEUR ████████  ff40ff      │
│              │                                │ teinte ══════○══              │
│ ZONES        │        ( la maquette )         │ ANIMATION — comete            │
│ ▸ radiateur  │                                │ [aucune][vague][comète]       │
│ ▸ ram        │                                │ [respiration][arc-en-ciel]…   │
│ [+ de la     ├────────────────────────────────┤ vitesse ══○══  [direction ▾]  │
│    sélection]│ SONDES  CPU 61.8  Liquide 34.2 │                               │
└──────────────┴────────────────────────────────┴───────────────────────────────┘
```

⚠️ **Les onglets ne sont pas un rangement, ils corrigent une mesure.** La fenêtre empilait ses
six panneaux dans **une seule** colonne de 340 px : à 1180×760, leur hauteur cumulée dépassait
1 400 px pour 690 px visibles — ÉCRAN et VENTILATEURS étaient entièrement sous le pli, et rien
ne le disait. Les profils, les quatre familles d'animation nouvelles et la composition d'écran
y ajoutaient six cents pixels de plus.

**Les ambiances sont en tête de fenêtre**, et non dans un panneau : rappeler un profil change le
plus de choses d'un seul clic. ⚠️ La pastille allumée dit « **rappelé** », jamais « actif » — le
protocole ne dit pas quel profil est actif, `status` n'en garde aucune trace. C'est une mémoire de
fenêtre, effacée dès qu'une commande change l'éclairage : dire « actif » d'une ambiance dont on
vient de changer la couleur serait faux, et c'est justement ce qu'on regarderait pour s'y retrouver.

**Les dix familles sont des pastilles, pas un menu déroulant** : dix lignes cachées derrière un
clic, c'est ne pas savoir ce qui existe. ⚠️ **Le menu des directions ne s'affiche que pour les
familles qui en acceptent une**, et c'est le catalogue qui décide — `rotation`, `pouls`,
`scintillement` et `thermique` la refusent, et la leur donner ferait rejeter l'`animate` **entier**,
pas seulement la clé.

**Les sondes se choisissent sous leur nom lisible** — « Liquide », « CPU » —, pour `thermique`
comme pour le cadran de l'écran, qui imposait encore `kraken2023elite:coolant-temp`. ⚠️ Le menu
n'offre que les **quatre familles retenues** ; un cadran posé par le socket sur l'une des seize
autres laisse donc le menu où il est, et c'est le bandeau « ÉCRAN — gauge:… » qui dit la vérité.

**Chaque barre de consigne porte sa valeur en pourcentage**, contre la barre elle-même. Une barre
sans repère chiffré rend une classe entière de défauts invisible : le bouton « auto » du Kraken a
été cru sans effet alors qu'il mettait la consigne à **0 %**, et il a fallu lire sysfs à la main
pour le voir. ⚠️ **Le nombre vient de la poignée** — celle qui décide déjà si une mesure a le droit
de déplacer le curseur (#32) — et de nulle part ailleurs : branché sur la télémétrie brute, il
afficherait 30 % pendant qu'on tire la barre à 80. Le régime en tr/min reste sur la ligne du dessus,
gris et plus petit ; ce sont deux grandeurs, et les confondre ferait croire qu'une consigne à 50 %
donne 50 % du régime maximal. Un canal qui ne répond pas écrit `-- %`, jamais la dernière valeur
connue, et une consigne qu'on lui tire quand même garde son chiffre suivi d'un `?`.

Le boîtier est dessiné **vu depuis le panneau latéral gauche, face à la carte mère** :
l'arrière à gauche, l'avant à droite. C'est le point de vue depuis lequel la géométrie a été
relevée, et celui depuis lequel on lit ses ventilateurs quand on se penche sur le bureau.

**L'aperçu ne recalcule rien.** Il affiche les images que le démon envoie vraiment au matériel,
reçues par `watch` — ce qui rend « l'aperçu montre ce que le boîtier reçoit » vrai par
construction, et non par la coïncidence de deux implémentations qu'il faudrait tenir d'accord.

⚠️ **C'est un schéma, pas une photographie**, et deux écarts s'imposent d'eux-mêmes :

| | |
|---|---|
| anneaux dessinés plus petits que nature | `bas-milieu` et `radiateur-bas` n'ont que **70 mm** entre leurs centres, pour un rayon physique de **55** |
| RAM décalée vers l'arrière | sa profondeur réelle la met **dans** la colonne du radiateur, qu'elle masquerait |

Sept des dix ventilateurs sont vus par la tranche depuis ce panneau ; les dessiner fidèlement en
ferait sept traits, où aucune LED ne serait cliquable. Ils sont donc dessinés en cercles quand
même. Une vue faite pour ne plus regarder sous le bureau doit montrer les 124 LED.

**Une seconde vue, isométrique**, se prend au bouton en haut à droite de la maquette. Elle projette
les positions réelles — aucun ventilateur n'y est vu par la tranche, et les quatre plans occupés
s'y distinguent. Son plafond reste **ouvert** : une plaque pleine masquerait les trois ventilateurs
du dessus, qui sont ce qu'elle sert à montrer. La sélection survit au changement de vue.

**Le geste de sélection porte sur tout le panneau, le dessin reste dans son carré.** La maquette
vit dans un carré de côté `min(largeur, hauteur)`, centré dans le panneau « LE BOÎTIER » : ses
tracés sont des `Path` en `viewbox` carré, que Slint met à l'échelle **uniformément**, et les
élargir demanderait de tous les reprendre — c'est le sujet de #118. La **surface sensible**, elle,
a quitté le carré pour le panneau (#121) : ses bords tombaient près des extrémités des
ventilateurs, si bien qu'un rectangle commencé dans une marge ne traçait rien, et qu'une maquette
qui répond partout sauf sur ses bords se lit comme une maquette en panne.

⚠️ **Les deux repères diffèrent d'une demi-marge, et la conversion se teste.** Elle vit dans
`plan::trace_dans_la_maquette` — une fonction pure, sans fenêtre ni souris —, jamais dans le
`.slint` : ce qui est du calcul se teste, ce qui est du dessin se regarde. Mesuré sur la fenêtre de
l'aperçu, dont le panneau fait 576 × 548 px : `REVERB_TRACE=0,0,1,1` couvrait **276 à 824 px**, le
carré ; il couvre désormais **262 à 838**, le panneau — quatorze pixels de chaque côté qui ne
répondaient pas.

⚠️ **Un geste parti d'une marge sort de `0..1`, et rien ne l'écrête.** Écrêter serait la correction
qu'on écrit sans y penser, pour « rester dans le cadre » : elle replierait le coin sur le bord du
carré, où il attraperait la première rangée de LED — la zone morte d'hier deviendrait une sélection
au hasard. Un rectangle **entièrement** dans une marge ne retient donc rien, et c'est un résultat :
il désélectionne, comme n'importe quel coin vide du carré.

Les deux vues sont habillées d'un châssis, de parois et des organes internes — plateau de carte
mère, carte graphique, cache d'alimentation —, d'un cadre par ventilateur, du corps des quatre
barrettes et de la dalle du Kraken. **Aucune de ces formes n'a de coordonnée dans le `.slint`** :
toutes viennent de `plan.rs`, sinon la maquette divergerait de la géométrie à la première
correction.

**Le châssis a la forme de la vue.** De face c'est un **rectangle**, en isométrie l'enveloppe
convexe des LED : une boîte vue de face en projection orthographique *est* un rectangle, et de
trois-quarts la même boîte se projette en hexagone.

⚠️ **Ce n'était pas le cas jusqu'à #125, et ça se voyait.** Les deux vues partageaient l'enveloppe
convexe, si bien que le châssis de face était un polygone à **quatorze tranches obliques** — le
contour de l'*éclairage*, pas celui du boîtier. Il avait un second défaut, moins visible et plus
gênant : l'enveloppe s'arrête aux centres de LED plus une pastille, quand un cadre de ventilateur
est tracé entre 1,20 et 1,25 fois son demi-axe. **Les ventilateurs dépassaient du trait censé les
contenir** — mesuré à 1,6·10⁻⁴ de cadre sur `bas-gauche`. Le rectangle part donc des bornes de
**tout ce qui est dessiné**, jamais des seules LED.

⚠️ **Il reste déduit, jamais écrit à la main**, et c'est la contrainte qui commande : `Geometrie`
ne porte que dix orientations — les centres et le rayon sont les mêmes pour toutes les géométries
d'une même machine —, donc un contour calculé sur les anneaux serait insensible à toute géométrie
possible, c'est-à-dire indiscernable d'un polygone posé en dur.

**Chaque LED baigne son entourage de sa couleur.** C'est ce qu'on reconnaît d'un boîtier RGB avant
la forme des pales, et une LED éteinte ne diffuse rien — un halo sur du noir ferait croire à un
démon qui n'a pas reçu la commande.

⚠️ **Le halo est dessiné, en disques translucides, et non par une ombre portée.** Le rendu logiciel
de Slint — celui de l'aperçu ci-dessous — ignore `drop-shadow-blur` : mesuré, pixel par pixel. Le
halo aurait été invisible dans l'outil même qui sert à vérifier la maquette.

⚠️ **Le cadre d'un ventilateur est une ellipse, pas le carré d'un F140.** Un carré est
géométriquement impossible ici : en isométrie, les LED d'un ventilateur atteignent **0,998** fois
son demi-axe quand le centre de son voisin n'est qu'à **0,900**. Aucun rectangle ne peut contenir
ses propres LED sans avaler le centre d'un autre ventilateur. Le cadre est donc percé et
elliptique, entre **1,20** et **1,25** — les deux bornes que la mesure laisse.

`cargo run --release --example apercu -p reverb-gui` dessine la fenêtre **sans écran**, dans un
fichier : c'est ainsi qu'on vérifie une mise en page sans ouvrir de session graphique. Quatre
variables d'environnement en montrent les autres faces — sans elles, deux tiers du panneau de
droite ne se vérifieraient jamais autrement qu'à l'œil nu :

```bash
REVERB_VUE=iso                              apercu iso.ppm        # la vue de trois-quarts
REVERB_DETAIL=ventilo                       apercu organes.ppm    # les 14 organes, pas les 124 LED
REVERB_ONGLET=ecran REVERB_AFFICHAGE=composition apercu compo.ppm # le disque des cinq ancres
REVERB_ONGLET=ventilos                      apercu ventilos.ppm   # les quatorze canaux
REVERB_TRACE=0.2,0.2,0.6,0.7                apercu trace.ppm      # une sélection en cours
REVERB_MESSAGE=                             apercu tete.ppm       # la tête de fenêtre au repos
```

⚠️ **`REVERB_TRACE` s'exprime en fractions du panneau, et non plus du carré** (#121) — le repère
du geste, celui où le rectangle se dessine. `0,0,1,1` couvre donc le panneau entier, marges
comprises, et `0,0.12,0.55,0.78` est un geste parti de la marge gauche : c'est le cas que l'issue
existe pour corriger, et il ne se voit sur aucune autre image.

⚠️ **Les deux dernières sont nées d'un correctif qui n'a rien corrigé.** Le rectangle de sélection
ne vit que pendant un appui de souris : aucune des autres variables ne pouvait le montrer. On a
donc cru le rendre visible en montant son opacité, alors qu'il était large de **zéro** — le coin
mobile lisait `pressed-x`, qui en Slint est la position de l'**appui** et ne bouge pas d'un pixel
pendant le geste. Une image aurait suffi à le voir.

### La composition, depuis la fenêtre

L'onglet ÉCRAN dessine **le disque de la dalle** et ses cinq ancres : on clique celle qu'on veut
garnir, on choisit une température ou un texte, on pose. Les ancres occupées portent leur libellé.

⚠️ **Ce disque n'est pas un dessin.** Ses cinq boîtes viennent de `Ancre::boite()` — celles que le
démon assombrit et sur lesquelles il écrit —, et son rayon de `screen::VISIBLE_DISC_RADIUS`. La
dalle est ronde, 21 % du tampon ne s'affiche nulle part, et composer sur un carré serait juger la
mise en page sur une surface qui n'existe pas. C'est la règle de la maquette — aucune coordonnée
dans le `.slint` — appliquée à la dalle : le jour où la mire de #77 mesurera le vrai bord, une
constante changera et la fenêtre suivra.

### Le verrou d'un canal qui régule seul

Une barre dont le canal régule seul est **grisée** et porte un bouton qui dit ce que le clic fait —
« Déverrouiller », puis « Verrouiller ». Tant qu'il n'a pas été pressé, ni la poignée ni « auto »
n'émettent quoi que ce soit.

⚠️ **C'est le même déclencheur qu'en [ligne de commande](#un-canal-qui-régule-seul-ne-se-défait-pas-dun-geste-distrait)** —
le **mode**, `non-piloté` ou `courbe-de-l'hôte`, jamais le nom du contrôleur. Sur SHYNAEL les deux
canaux du Kraken lisent `non-piloté` et les trois `nzxtsmart2` lisent `manuel` : le comportement
obtenu est exactement celui qu'on voulait, sans que « Kraken » soit écrit nulle part. Sept
ventilateurs sur dix n'ont donc aucun cadenas — leur en imposer un serait payer le prix du verrou
sans en tirer la protection.

⚠️ **Ce qu'il protège ne se répare pas.** Le 2026-08-15, un clic sur « auto » a mis la consigne de la
pompe à 0 % (#97) ; le même jour, la mesure a montré la courbe d'usine parfaitement vivante — 35 % à
37 °C, 60 % à 51 °C. **Ce qui régule bien est exactement ce qu'un geste distrait détruit**, et il
n'existe aucune valeur de `pwm_enable` qui rende le Kraken à son profil d'usine : seule une coupure
d'alimentation complète le fait ([`docs/VENTILATEURS.md`](docs/VENTILATEURS.md)).

⚠️ **Le verrou repart fermé à chaque ouverture de la fenêtre.** C'est une mémoire de fenêtre, comme
la pastille du profil rappelé : rien ne la persiste. Un verrou qui se souviendrait d'avoir été
ouvert ne protégerait plus rien le lendemain.

⚠️ **Un canal illisible reste inerte, cadenas ouvert ou fermé** (#100). Les deux règles se composent
au lieu de se remplacer : le Kraken part périodiquement en quarantaine, et ouvrir son cadenas pendant
ce temps rendrait sa barre manipulable vers un périphérique qui ne répond plus.

Un **point d'interrogation**, en tête du panneau, ouvre ce que chacun des six modes veut dire — qui
décide de la vitesse, et ce qu'on perd en y touchant. C'est l'information qui manquait le
2026-08-15 : `non-piloté` et `plein-régime-100%` sont les deux sens opposés du **même** `0`, et rien
ne le disait.

`REVERB_ONGLET=ventilos` rend le panneau verrou fermé,
`REVERB_ONGLET=ventilos REVERB_VERROU=ouvert` le rend verrou ouvert — les deux moitiés d'un même
geste, dont celle qui laisse écrire ne se verrait sinon sur aucune image.

### La régulation depuis la fenêtre

Chaque canal **régulable** — celui dont le pilote n'a aucun mode automatique — porte un bouton
« Réguler », et « Ne plus réguler » une fois pris en charge. La courbe s'édite sous la liste, avec
le tracé de ce qu'elle donne, et le bouton « Appliquer la courbe » la pose par le socket.

#### La courbe s'édite par points, sur son tracé

| geste | ce qu'il fait |
|---|---|
| clic gauche sur un point | le saisit ; le traîner change sa température **et** sa consigne |
| clic gauche sur le cadre | pose un palier de plus — **refusé au-delà de huit**, en le disant |
| clic droit sur un point | le retire — **sauf le dernier**, auquel cas rien ne bouge et la fenêtre le dit |

Les barres de #113 restent, en **réglage fin** : la souris place un point à quelques centaines de
millidegrés près dans un cadre de 360 px, et viser 45 000 exactement reste impossible sans elles.

⚠️ **Le plancher est d'UN palier, pas de deux — et l'issue disait le contraire.** Elle affirmait
que « `Courbe::depuis` refuse une courbe qui n'a pas au moins deux paliers ». C'est faux, et
vérifié : cette fonction refuse une courbe **vide**, une consigne au-dessus de 100, deux paliers à
la même température et une température décroissante. Deux tests d'intention figent l'inverse —
`une_courbe_a_un_seul_palier_est_plate` (#99) et `une_courbe_a_un_seul_palier_reste_acceptee`
(#113). Une consigne fixe est une courbe parfaitement sensée, et `regule courbe 45000:80` passe.

⚠️ **Le juge de ce plancher est donc l'éditeur, jamais `Courbe::depuis`.** Rendre cette dernière
plus stricte casserait les deux tests ci-dessus et ferait diverger la fenêtre du démon. C'est
l'inverse du réflexe qu'on a en lisant l'issue, et un test existe pour l'empêcher.

⚠️ **Un point traîné en travers d'un voisin est refusé en le disant, jamais réordonné.** Les
réordonner serait deviner ce qui a été tapé. Le refus vient du même juge que le bouton « Appliquer »,
donc de `Courbe::depuis` — celle que le démon exécute.

⚠️ **Le geste est ramené aux bornes du cadre**, et les poignées ne le sont **pas**. La souris sort du
cadre en un dixième de seconde ; sans borner, une consigne à 110 % ferait refuser la courbe et un
palier traîné à gauche deviendrait indessinable *et* inattrapable. Mais un palier venu du socket hors
de la plage tracée doit montrer **où il est**, sinon deux paliers distincts se confondraient sous la
même poignée. On borne donc ce qu'un geste **produit**, jamais ce qu'on **affiche**.

⚠️ **Ce n'est pas l'écrêtage que #121 a refusé sur la maquette.** Là-bas, replier un coin sur le bord
créait une **sélection au hasard** — un résultat faux et invisible. Ici, buter contre le bord est le
seul résultat visible, et il se corrige d'un second geste.

⚠️ **Aucune coordonnée de poignée n'est calculée dans le `.slint`.** Elles viennent de
`point_du_palier`, l'inverse exact de la conversion qui lit le geste : deux calculs séparés
divergeraient, et une poignée finirait ailleurs que là où le clic la reprend.

⚠️ **« Régulé » et « verrouillé » grisent tous deux une barre, pour des raisons opposées**, et la
fenêtre doit donc les distinguer : un canal régulé est piloté **par le démon** et on protège la
boucle ; un canal verrouillé régule **tout seul** et on protège sa régulation. D'où deux signes
différents — une pastille « régulé » d'un côté, un bouton « Déverrouiller » de l'autre. Les
confondre laisserait « pourquoi je ne peux pas bouger cette barre » sans réponse.

⚠️ **Le fait « ce canal se régule côté hôte » vient du protocole, pas d'une liste tenue dans la
fenêtre.** La ligne `chan` porte un drapeau de plus, lu dans le **nom du pilote** (#50). Le champ
voisin `sait_faire_auto` ne pouvait pas servir : depuis #97 il vaut « le bouton auto marcherait
maintenant », donc toujours `non`, et il ne dit plus rien du matériel.

⚠️ **Le tracé passe par `Courbe::consigne`, la fonction que le démon exécute** — d'où la descente de
`Courbe` dans `reverb-proto`, que la fenêtre et le démon partagent. Une seconde interpolation écrite
côté fenêtre n'aurait aucune raison d'être la bonne, et l'aperçu montrerait une courbe que le
boîtier n'applique pas. Le **refus** d'une courbe invalide vient du même juge, `Courbe::depuis`, et
arrive donc devant la poignée qu'on vient de bouger plutôt que dans un journal après coup.

⚠️ **Le tracé était écrasé dans un carré, et c'est l'image d'aperçu qui l'a montré.** Slint met un
`Path` à l'échelle **uniformément** — `min(largeur/viewbox_largeur, hauteur/viewbox_hauteur)` — et
il **ne rogne pas** au viewbox : un carré unité dans un cadre de 359 × 88 px n'occupait que 88 px de
large, centré. **24 % du cadre, mesuré au pixel**, les deux plateaux comprimés avec la rampe — c'est
justement l'écrêtage aux bornes qu'on ne voyait plus. Et ne corriger que le `viewbox-height` fait
**déborder** la courbe par le haut au lieu de l'étirer, puisque rien ne la rogne : essayé, mesuré
aussi. Le tracé est donc émis sur `TRACE_ASPECT` × 1, et le cadre tient ce même rapport ; la
constante vit dans le Rust et sert deux fois côté fenêtre, pour n'exister qu'une seule fois.

⚠️ **Le même défaut vivait sur les courbes du panneau SONDES, et il y a survécu deux mois** (#118) :
elles aussi étaient émises dans le carré unité, et n'occupaient que **74 %** de leur tuile — 67 %
sur celle qu'un libellé de NVMe élargit. Elles ont leur propre constante, `COURBE_ASPECT`, **et
c'est délibéré** : une tuile de sonde et le cadre de la courbe de régulation n'ont pas la même
forme, et leur donner un rapport commun remettrait le défaut sur l'un des deux.

⚠️ **Sa valeur a été trouvée en balayant, pas en la déduisant, et c'est la leçon de #118.** Le
calcul « largeur du cadre ÷ hauteur du cadre » donnait 1,4 et laissait 16 % de vide : le rendu de
Slint ne s'y ramène pas exactement. Une dérivation géométrique aurait été plus convaincante que
juste. Mesuré : 74 % à 1,0, 84 % à 1,4, 90 % à 1,7, **98 % à 2,0**, puis le tracé cesse de gagner en
largeur et se met à perdre en hauteur. **2,0 est le genou**, donc le rapport qui dessine la plus
grande courbe — et non le plus grand nombre.

⚠️ **L'aperçu fabriquait ses propres commandes de sparkline**, dans le carré unité, au lieu
d'appeler la fonction de la fenêtre. Il a donc montré le `viewbox` corrigé par-dessus des
coordonnées qui ne l'étaient pas — un tracé **plus** écrasé qu'avant, et une image qui démentait son
propre correctif. Il garnit désormais un vrai `Historique` et laisse `commandes_de_courbe` dessiner.
C'est la règle de la maquette et celle du tracé de régulation, appliquées à un troisième endroit :
**une seconde implémentation n'a aucune raison d'être la bonne.**

```bash
REVERB_ONGLET=ventilos                          apercu ventilos.ppm  # la liste et la courbe
REVERB_ONGLET=ventilos REVERB_COURBE=invalide   apercu refus.ppm     # le refus, sans tracé
```

⚠️ **La seconde n'est pas un ornement** : un refus ne vit que le temps d'un réglage de travers, et
sans elle il ne se verrait sur aucune image.

### Ce que la fenêtre ne fait pas

- Elle **n'ouvre aucun périphérique** et **n'écrit aucun fichier**. Tout passe par le socket, qui
  reste l'unique franchissement de privilège (ADR-002).
- La fermer n'éteint rien : le démon continue. Il n'y a **pas d'icône dans la barre système** —
  sur GNOME/Wayland elle dépendrait d'une extension du bureau, qui casse aux montées de version.
- Elle ne montre **pas les seize sondes** que la machine expose, seulement quatre familles — CPU,
  liquide, GPU, et un disque NVMe par SSD — sous des noms qui se lisent. Le démon, lui, continue de
  toutes les découvrir et de toutes les rendre : le tri est un choix d'affichage, pas un filtre de
  relevé.
- Le bouton **« auto » n'apparaît que sur un canal qui peut l'exécuter maintenant**. Le pilote
  `nzxt-smart2` n'a aucun mode automatique — sa vitesse est celle que l'hôte écrit —, et montrer un
  bouton qui ne peut qu'échouer vaut moins que ne pas le montrer. Ce que le démon sait faire pour
  ces trois canaux-là s'appelle [`regule`](#la-régulation--les-sept-ventilateurs-que-personne-ne-pilotait),
  et se pilote depuis #113 [par la fenêtre comme par le socket](#la-régulation-depuis-la-fenêtre) —
  sous un bouton « Réguler », à la place où « auto » ne peut pas s'afficher.
  ⚠️ **De #97 à #104, il ne s'affichait nulle part** : « auto » écrit `pwm_enable = 2`, qui fait
  exécuter la courbe de l'**hôte** — zéro partout tant qu'aucune n'a été téléversée, ce qui arrête
  la régulation de la pompe au lieu de la rendre. Le démon n'ayant pas de verbe `curve` sur le
  socket, son carnet de courbes posées restait vide, et les deux canaux du Kraken rejoignaient les
  autres. **#104 lui a donné le verbe** : une courbe posée par le socket remplit le carnet, et le
  bouton réapparaît sur le canal concerné — sur celui-là seulement, et jusqu'au prochain
  redémarrage du démon.
- Une sonde qui cesse de répondre s'affiche **illisible**, et le reste de la fenêtre continue à
  pleine vitesse. Voir [ci-dessous](#une-sonde-muette-nemporte-pas-le-démon).
- Un **canal de ventilation** muet s'affiche illisible lui aussi — à sa place dans la liste, sa
  poignée et son « auto » inertes —, là où sa ligne **disparaissait** : une ligne absente laisse
  croire que le canal n'existe pas, donc qu'il n'y a rien à régler ni rien à réparer (#100).
  ⚠️ Encore faut-il qu'il ait répondu **une fois depuis l'ouverture de la fenêtre** : une ligne
  `unreadable` ne dit pas la nature de son sujet, et la fenêtre reconnaît un canal à l'avoir vu en
  `chan`. Un Kraken déjà en rade au lancement manque donc à l'appel jusqu'à sa première réponse.
- Le **cadenas** n'est pas un dessin : c'est un bouton qui dit ce que le clic fait. Un glyphe
  tiendrait — U+1F512 sort bien de la police de repli du rendu logiciel, mesuré le 2026-08-15, là où
  U+26BF sort en carré vide —, mais un cadenas seul ne dit pas dans quel sens il bascule : montre-t-il
  l'état, ou l'action ? C'est l'arbitrage des dix pastilles d'animation contre un menu déroulant, et
  ce projet écrit les mots. Le prix est en largeur de barre, et il tombe sur les seuls canaux qu'on
  ne règle presque jamais — ceux qui régulent tout seuls.
  ⚠️ L'aide sur les modes est **un** point d'interrogation pour le panneau, et non un par ligne :
  le panneau qu'il ouvre liste les six modes quelle que soit la ligne d'où l'on part, et cinq icônes
  ouvriraient cinq fois le même texte — dans la rangée même où le cadenas vient de prendre la place.
- Une LED peinte à la main (`paint`) **ne survit pas à un redémarrage** : `eclairage.conf` garde
  une couleur par cible, pas une par LED (#21). La cible reprend sa couleur unie au démarrage.
  **Une zone, si** — c'est le moyen de rendre une peinture durable : sélectionner les LED, les
  nommer, leur donner leur couleur.
- Le chemin d'une image se **colle dans un champ de texte**, il ne s'ouvre pas dans un sélecteur de
  fichiers : une boîte de dialogue demanderait le portail XDG, donc un client D-Bus, donc une
  dépendance de runtime que l'ADR-001 refuse. C'est, avec le nom d'une zone, d'un profil et le
  libellé d'un champ, l'un des rares endroits où l'on tape au lieu de cliquer — un manque assumé.
- Elle ne montre **pas quel profil est actif**, parce que le protocole ne le dit pas. Elle montre
  celui qu'elle vient de **rappeler**, et l'oublie au premier changement d'éclairage.

### Les zones — une zone, une couche

Une zone est un ensemble de LED que l'on compose soi-même sur la maquette : « le ventilateur
arrière plus bas-milieu plus haut-milieu » en est une. Elle porte soit une **couleur fixe**, soit
une **animation** avec sa propre vitesse, pendant que le reste du boîtier continue sa vie.

```
zone set   <nom> <cible>[,<cible>…]   fan:<position>[:<0-7>] ou slot:<0-3>[:<0-10>]
zone light <nom> <rrggbb>
zone anim  <nom> <animation|off> [clé=valeur…]
zone drop  <nom>
zone list
```

⚠️ **Une LED appartient à au plus une zone.** La mettre dans une nouvelle la retire de celle qui la
tenait — c'est ce qui dispense d'un ordre d'empilement, et donc d'une notion de transparence qu'une
LED ne sait pas porter. Ce qui n'est dans aucune zone suit la couche « tout le boîtier ».

⚠️ **Une animation de zone se calcule sur le boîtier entier**, et la zone n'en montre que sa part.
C'est ce qui garde deux zones voisines cohérentes entre elles. Conséquence assumée : une vague sur
une zone d'une seule LED clignote au lieu de défiler.

Les zones vivent dans `/var/lib/reverb/zones.conf`, à côté de `eclairage.conf` qui porte la couche
globale. Deux fichiers pour deux natures.

## Installation

```bash
./tools/installe.sh      # groupe, règle udev, binaires, service, lanceur
./tools/desinstalle.sh   # tout retirer, sauf la géométrie et l'éclairage
```

Rejouable : une seconde exécution met à jour ce qui a changé. Elle ne rend la main qu'une fois le
socket ouvert — un démon qui ne répond pas se découvrirait sinon en ouvrant la fenêtre.

Au tout premier passage, le groupe `reverb` vient d'être accordé et la session ne l'a pas encore :
`sg reverb -c reverb-gui` l'active sans se déconnecter. La désinstallation laisse
`/etc/reverb/geometrie.conf` — une orientation relevée à l'œil sous le bureau ne se jette pas sans
le dire.

## Prérequis

Rust, installé dans le `$HOME` — aucune surcouche `rpm-ostree` nécessaire :

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

La règle udev livrée par le dépôt, qui ouvre les contrôleurs NZXT à l'utilisateur connecté :

```bash
sudo cp packaging/60-reverb.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
```

Elle couvre les deux contrôleurs d'éclairage et le Kraken, et n'emploie que `TAG+="uaccess"` :
l'accès va à l'utilisateur physiquement connecté et lui est retiré à la déconnexion, là où un
`MODE=` l'ouvrirait à tout process local. Lis-la avant de la poser, elle tient en trois lignes.

`sudo ./tools/verifie_udev.sh` installe la règle et vérifie qu'elle déclenche vraiment.
La nuance a son importance : sur une machine où OpenRGB est installé, `reverb list` marche
de toute façon, et `udevadm test` ne journalise pas les `TAG+=`. D'où le tag `reverb`, que la
règle pose et qu'aucune autre ne pose — il est présent sur le périphérique si et seulement si
c'est bien elle qui a matché.

Sans elle, `reverb` fonctionne quand même sur une machine où OpenRGB est installé — mais par
ses règles à lui. **Un projet qui vise à remplacer OpenRGB n'a pas à dépendre de ses règles
udev** : c'est ce que corrige `packaging/60-reverb.rules`.

Deux accès restent hors de sa portée :

- **les attributs hwmon** (`pwm*`, courbes du Kraken) appartiennent à `root` et `uaccess` ne
  s'applique pas à sysfs. `reverb fan` et `reverb curve` demandent donc `sudo` ; les commandes
  d'éclairage et `reverb fans` n'en ont pas besoin ;
- **`/dev/i2c-*`**, nécessaire à la RAM Corsair, dépend encore de la règle d'OpenRGB. À traiter
  avec le chantier RAM.

## L'écran du Kraken

**Le démon tient la dalle.** La fenêtre n'ouvre aucun périphérique (ADR-002) : elle envoie un
**chemin de fichier**, et c'est le démon qui lit, décode, met à l'échelle et pousse les 1,2 Mo. Le
mégaoctet ne traverse jamais le socket ; seul le chemin le fait.

```bash
reverb screen                              # luminosite et affichage courants
reverb screen --brightness 40
reverb screen --image /home/nico/fond.png  # PNG ou JPEG, n'importe quelle taille
reverb screen --gif   /home/nico/pluie.gif # en boucle, a la cadence du fichier
reverb screen --gauge kraken2023elite:coolant
reverb screen --off                        # la dalle est rendue au firmware
```

⚠️ **Le chemin doit être absolu**, et la ligne de commande le complète pour vous : le démon lit
sous son propre répertoire courant, qui n'est pas celui de son client.

Une image est mise à l'échelle **sans déformer ses proportions**, puis centrée sur du noir. Une
photo 16/9 écrasée en carré s'afficherait sans erreur, nette et fausse ; c'est mesuré par les
tests, pas regardé sur une dalle de 6 cm.

Un GIF est joué à la cadence de ses propres images, **plancher compris** : une image de 1,2 Mo met
une centaine de millisecondes à passer, et un GIF à trente images par seconde est donc **ralenti,
jamais tronqué**. Un mouvement lent et complet se regarde, un mouvement saccadé non.

### Le cadran

`--gauge <sonde>` affiche une sonde en gros, avec son unité et un anneau de proportion.

⚠️ **Le nom à donner est celui du protocole, pas celui de la fenêtre.** Le panneau SONDES montre
« CPU », « Liquide », « GPU » et les deux disques sous leur modèle ; le cadran, lui, attend le
`slug` — `kraken2023elite:coolant-temp`. `echo status | socat - UNIX-CONNECT:/run/reverb/reverbd.sock`
en donne la liste complète, seize lignes `temp`.

**Il est écrit dans une vraie fonte, embarquée dans le binaire** — `Nunito`, en graisse 700, dont
les traits se terminent par des **demi-cercles**. Le rastériseur est `fontdue` : du Rust pur,
`no_std`, aucune bibliothèque système. La promesse de l'ADR-001 tient — ce qu'il refuse, c'est une
bibliothèque *système*, pas une dépendance Rust.

⚠️ **Deux remplacements successifs, et le second est un retour d'usage.** D'abord des chiffres à
sept segments et une matrice 5 × 7 dessinés à la main (#33) : le bon choix pour afficher « 34.2 »
sans traîner de moteur de rendu, plus du tout dès qu'il a fallu écrire des libellés. Puis
`LiberationSans-Bold` (#90), métriquement compatible Arial — très lisible, mais un dessin de 1982,
jugé « trop archaïque, trop anguleux » devant le boîtier.

⚠️ **Nunito n'est publiée qu'en fonte variable**, et `fontdue` ne sait pas appliquer d'axe de
variation : la graisse est figée hors ligne par `fonttools varLib.instancer` et l'instance vendue
dans le dépôt. Elle n'a **pas de nom de fonte réservé**, donc elle garde son nom. Le prix est
mesuré, et il est négatif : `reverbd` passe de 2 780 600 à **2 499 368** octets — l'instance pèse
132 Kio là où l'Arial en pesait 414.

```bash
cargo run --release --example cadran -p reverb-daemon -- /tmp/cadran.ppm 34.2 0.34
```

dessine un cadran **sans matériel**, dans un fichier : c'est ainsi qu'on vérifie « lisible à un
mètre » sans brancher de Kraken.

⚠️ **Une sonde muette affiche des tirets, jamais un zéro.** C'est le mode de défaillance le plus
coûteux du cadran, parce qu'il est rassurant : un 34 °C figé derrière une pompe arrêtée, c'est un
circuit qui chauffe sans que rien ne le signale.

### La composition — un fond, et jusqu'à quatre informations dessus

Le cadran montre **une** sonde et rien d'autre. Une composition en montre plusieurs, sur la photo
qu'on veut :

```
screen layout fond image /home/nico/fond.png       ou « fond noir »
screen layout champ haut   temp kraken2023elite:coolant-temp LIQUIDE
screen layout champ gauche temp k10temp:tctl CPU
screen layout champ droite temp nvidia:NVIDIA_GeForce_RTX_5070 GPU
screen layout champ bas    texte SHYNAEL
screen layout vide bas                              retirer ce champ
screen layout off                                   revenir à l'affichage simple
screen layout                                       ce que la dalle compose
```

Cinq ancres — `haut`, `bas`, `gauche`, `droite`, `centre` —, **quatre champs au plus**. Chacun
porte soit une **température** avec un libellé qu'on choisit, soit un **texte fixe**.

**Chaque température dessine son arc sur la couronne**, dans le secteur de son ancre, rempli
proportionnellement de 0 à 100 °C — la même échelle que le cadran. On lit où en est la valeur d'un
coup d'œil, sans lire le chiffre.

⚠️ **Une piste sombre occupe l'ouverture entière, sous l'arc.** Sans elle, un arc à vingt pour cent
se lit comme « une petite barre » et non comme « vingt pour cent de quelque chose » : c'est la piste
qui porte l'échelle.

⚠️ **Les bords sont lissés et les extrémités arrondies**, parce que la couverture d'un pixel se
**calcule** au lieu de se décider. #90 testait l'appartenance au secteur — dedans ou dehors —, d'où
des bords en escalier et des bouts coupés à l'équerre. Chaque pixel reçoit maintenant sa distance au
ruban, et les demi-disques des extrémités en découlent sans code supplémentaire : au-delà de
l'étendue angulaire, la distance cesse d'être radiale et devient celle au centre du bout.

⚠️ **L'anneau du cadran passe par le même ruban** — c'est la barre la plus visible de la dalle, et la
laisser en escalier pendant qu'on lisse les quatre petites aurait été le pire des deux mondes. Seule
exception : un tour complet se dessine sans bouts, sinon les deux demi-disques se recouvriraient au
sommet et y creuseraient une encoche.

⚠️ **`centre` n'a pas d'arc**, et ce n'est pas un oubli : elle est au milieu du disque, pas sur son
bord. Un **texte** n'en a pas non plus — il n'y a rien à remplir proportionnellement. Et une sonde
muette **vide** son arc plutôt que de le figer au dernier remplissage connu, exactement comme elle
écrit des tirets plutôt qu'un zéro.

⚠️ **Les boîtes des champs ont reculé pour laisser la place à la couronne.** Celle du haut passait à
317,6 du centre pour un disque de 320 : il n'y avait pas la place d'un anneau à l'extérieur. Elles
s'arrêtent maintenant à 286,4, et la couronne occupe 292 à 316.

⚠️ **La dalle est ronde**, observé le 2026-08-08 (`SPEC-KRAKEN-LCD` §2.1.1). Le tampon, lui, est
carré : **21 % de ce qu'on transmet ne s'affiche nulle part**, et rien ne le signale — le contrôleur
accepte l'image entière. D'où l'absence d'ancre en coin, et une vérification plutôt qu'une promesse :
les cinq boîtes tiennent dans le disque, et un test le calcule.

⚠️ **Le libellé n'est pas un ornement.** `kraken2023elite:coolant-temp` fait vingt-huit caractères
sur une dalle de six centimètres. Le cadran impose ce slug et le README relève déjà que c'en est un
défaut ; ici, on nomme soi-même ce qu'on regarde.

⚠️ **Une commande par changement**, et non une ligne unique qui porterait tout. Un chemin et un
libellé sur la même ligne seraient ambigus au premier espace : rien ne dirait où finit l'un et où
commence l'autre. C'est la règle du dernier champ, celle des profils (#74) et des chemins d'image.

**Un champ pose son propre fond.** Le tampon est assombri à 30 % sous chaque champ, puis le texte
est écrit en blanc par-dessus. Ce n'est pas une couleur qu'on espère contrastée : une photo claire
avale du texte blanc, et la seule garantie est de décider soi-même du fond derrière les caractères.
La photo reste devinable sous le champ, ce qu'un rectangle noir opaque perdrait.

⚠️ **Une sonde muette écrit des tirets**, jamais un zéro ni la dernière valeur connue — la règle du
cadran, pour la même raison. Les sondes d'une composition passent par la **quarantaine de #68** :
elle en lit jusqu'à quatre toutes les deux secondes, et une lecture sysfs sur un périphérique muet
bloque cinq secondes dans le fil qui sert aussi le socket.

```bash
cargo run --release --example composition -p reverb-daemon -- /tmp/composition.ppm blanc
```

dessine une composition **sans matériel**, dans un fichier, avec le bord du disque tracé par-dessus
— sans ce repère on jugerait la mise en page sur une surface qui n'existe pas. Le second argument
est `noir`, `blanc`, ou le chemin d'une image : c'est ainsi qu'on vérifie qu'un champ se lit sur les
deux extrêmes.

**Ce qui n'y est pas** : un GIF animé en fond (recomposer du texte sur trente images par seconde
pour six centimètres de dalle ne vaut pas son coût — un `.gif` posé en fond n'affiche que sa
première image), les tours/minute et l'heure en champ, un positionnement au pixel. La **fenêtre**
ne l'expose pas encore : #76 s'en charge.

### Ce que le démon réémet, et pourquoi

Rien de ce que la dalle affiche ne tient sans réémission — **pas même une image fixe**. Le firmware
reprend la main une trentaine de secondes après le dernier envoi, et il n'existe aucune commande
pour y revenir : **cesser d'émettre est le seul moyen connu**, et c'est exactement ce que
`--off` fait.

| affichage | réémission |
|---|---|
| image fixe | toutes les 25 s |
| GIF | à la cadence de ses images, au plus vite tous les 100 ms |
| cadran | toutes les 2 s, avec la valeur du moment |
| composition | recomposée toutes les 2 s, **poussée seulement si elle a changé** |
| composition sans sonde | toutes les 25 s — rien ne peut y bouger |
| rien | jamais — le démon est au repos absolu |

⚠️ **Le protocole n'a aucune mise à jour partielle** (spec §2, §3.6 — CAM lui-même repousse tout) :
changer un chiffre coûte les 1,2 Mo entiers. D'où la comparaison avant l'envoi, qui est au fond le
même principe que le cache de LED du démon — à température stable, il n'y a rien à envoyer. Le
battement de 25 s, lui, ne se négocie pas : identique ou non, l'image repart avant que le firmware
ne reprenne la main.

⚠️ **La trame de mode ne part qu'à l'ouverture, et c'est un correctif, pas une optimisation.**
`38 01 02 00` est indispensable avant la **première** image — sans elle, la dalle reste sur son
affichage intégré et ignore l'image en silence (spec §2.2.1). Mais c'est aussi une commande
d'affichage, donc elle **réinitialise le pipeline** (spec §3.4) : émise avant chaque envoi, elle
faisait passer l'écran au noir toutes les vingt-cinq secondes — observé devant le boîtier, et pris
d'abord pour le repli du firmware, qu'elle imite. La capture ne laisse pourtant aucun doute : sur
les cinquante images de CAM, « aucune trame `32` ni `38` entre deux images » (spec §3.6).

⚠️ **Ce n'était pas cosmétique.** Une image fixe réémise toutes les 25 s, c'était ~3 456
réinitialisations de pipeline par jour sur un contrôleur dont l'implémentation de référence n'en
inflige qu'une par session — la **seule divergence connue** entre ce que Reverb envoie au Kraken et
ce que CAM lui envoie, sur un périphérique qui se bloque périodiquement pour une raison qui reste
[inconnue](#une-source-entièrement-muette-le-démon-tente-de-la-réparer). Ça n'en fait pas la cause ;
ça en fait la piste qu'on pouvait suivre en **cessant** de faire quelque chose.

⚠️ **Une réouverture réarme, et rien n'a eu à être écrit pour ça** : `Default` est le seul
constructeur du drapeau, donc la poignée lâchée puis rouverte après un reset USB (#98) repart d'un
mode neuf. Un drapeau qui aurait survécu à l'ouverture rendrait la dalle définitivement muette après
la première réparation.

⚠️ **Le mode de défaillance est silencieux, donc la vérification est à l'œil.** Sans la trame,
« l'envoi réussit, aucun code d'erreur, mais rien n'apparaît » (§2.2.1) : aucun accusé ne le dit, et
aucun test automatisé ne peut l'attraper. Ce qui *est* figé sans matériel, c'est la décision
elle-même — `fil_ecran::ModeDeDiffusion` ne reçoit ni descripteur, ni chemin, ni trame, sur le motif
de `refus_de_consigne` (#101).

### Ce qui reste en direct

`reverb screen --mire` affiche quatre quadrants de couleurs connues. C'est la mire qui a confirmé
l'ordre BGR, que la rétro-ingénierie n'avait jamais pu vérifier.

`reverb screen --mire=cercle` **mesure le rayon du disque visible** : seize anneaux blancs, un tous
les 20 px du centre jusqu'à 320, dont un sur quatre est un **repère rouge** deux fois plus épais —
à 80, 160, 240 et 320 px. Quatre rayons blancs et un point central disent si l'image est centrée.
Les quatre coins sont en rouge sombre : en voir un dirait que la dalle n'est pas ronde.

⚠️ **Elle se photographie, elle ne se lit pas.** Compter seize anneaux fins à l'œil derrière une
vitre teintée est une source d'erreur à soi seule ; sur une photo prise bien en face, on compte les
repères rouges et on ajoute les anneaux fins qui restent.

⚠️ **Une première version couvrait le seul quart extérieur** — neuf bandes colorées entre 248 et
320 — et laissait le centre noir. Essayée sur SHYNAEL le 2026-08-09, elle n'a **rien montré** : du
noir rétroéclairé. Une mire qui ne sait mesurer que ce qu'elle présuppose confond son résultat avec
une panne. Celle-ci couvre le disque entier.

⚠️ **La mire des quadrants ne pouvait pas répondre à cette question** — un disque montre ses quatre
quadrants exactement comme un carré. D'où une seconde mire, et non un réglage de la première.

```bash
cargo run --release --example mire -p reverb-proto -- /tmp/mire.ppm cercle
```

la rend dans un fichier, sans matériel : une mire se regarde avant de se brancher.

Ni l'une ni l'autre n'est dans le protocole — ce sont des outils de diagnostic — et elles écrivent
donc en direct, ce qui suppose le démon arrêté : le nœud USB ne se réclame pas deux fois.

Sans démon, `reverb screen` retrouve tout ce qu'il savait faire seul : l'état lu sur le
contrôleur, la luminosité, et une image **brute** de 1 228 800 octets en BGR, telle que
`ffmpeg -i photo.png -vf scale=640:640 -f rawvideo -pix_fmt bgr24 img.raw` la produit.

### Ce que le démon refuse d'afficher

Le format est reconnu **au contenu, avant que rien ne bouge** — jamais à l'extension :

```
$ reverb screen --gif photo.jpg
err « /home/nico/photo.jpg » n'est pas un GIF : décodé comme JPEG — essaie « image »
```

Rien n'est écrit, rien n'est appliqué, la dalle continue ce qu'elle montrait. Ce n'est pas
une politesse : un affichage impossible **persisté** faisait redémarrer le démon dans un état
cassé, indéfiniment, sans moyen d'en sortir seul. C'est arrivé, et ça a probablement planté
la dalle (#69).

⚠️ **Après trois échecs d'affilée sur la dalle, le démon renonce** et la rend au firmware. Il
le dit une fois, puis se tait. Réémettre sans fin vers un contrôleur qui refuse ne le
réveille pas : ça consomme le bus et insiste sur un périphérique déjà en difficulté. Une
commande `screen` relance quand on veut ; le fichier d'état, lui, n'est pas effacé, donc un
redémarrage retente (#70).

### Une dalle muette n'emporte pas le boîtier

**La dalle a son propre fil, et il la détient seul.** Le fil de rendu compose l'image — c'est du
calcul — puis la **dépose** ; le verdict revient au tour suivant, lu sans attendre. Le mégaoctet ne
passe plus jamais par le fil qui anime les LED et sert le socket.

⚠️ **Ce n'était pas un réglage à corriger.** Le cache de #80 — « on ne pousse que ce qui a changé »
— n'y pouvait rien : quand la température change, il *faut* pousser. Mesuré sur SHYNAEL le
2026-08-08, composition à deux sondes vivantes sur fond image :

| régime | avant | après |
|---|---|---|
| dalle rendue au firmware | 21 img/s · 11 sautées | 21 · 11 |
| composition à deux sondes vivantes | **15 · 45** | **21 · 11** |
| dalle qui refuse | **2 · 306** | **21 · 11** |
| **dalle muette** | **0 — vingt minutes sans un tic de CPU** | **21 · 11** |

Cette dernière ligne se lit telle quelle : le démon est resté vingt minutes gelé, cinq clients
bloqués sur une réponse qui n'est jamais venue, `status` sans un octet après quinze secondes.
`hidraw::ask` bornait le nombre de **trames** lues et jamais le **temps** — sur un descripteur
bloquant, vingt lectures dont la première ne revient jamais font une attente infinie déguisée en
boucle bornée. Le commentaire promettait pourtant « sans jamais bloquer indéfiniment », et c'est ce
qui a rendu le défaut si durable.

Toute question HID est désormais bornée : le descripteur s'ouvre en `O_NONBLOCK` et chaque lecture a
son échéance. **Une demi-seconde par trame**, soit vingt-sept fois le pire acquittement relevé (18 ms,
spec §3.2). Le délai vaut **par lecture** et non pour la question entière : ces contrôleurs émettent
sans qu'on leur demande, et un délai global déclarerait mort un périphérique vivant mais bavard — or
trois questions expirées rendent la dalle au firmware (#70), donc un faux abandon coûte plus cher
qu'une attente généreuse. Le pire cas reste borné à dix secondes, sous les trente au bout desquelles
le firmware la reprend de toute façon.

⚠️ **Deux capacités perdues, et c'est le prix du déménagement** : une luminosité refusée ne rend plus
d'erreur au client, et un échec au démarrage n'est plus dit au démarrage. Les deux reviennent par le
journal. En attendre le résultat remettrait les 51 ms d'ouverture d'un `hidraw` dans le chemin des
LED, pour rendre au client une erreur qu'il ne peut de toute façon pas corriger.

### Une sonde muette n'emporte pas le démon — ni un canal de ventilation

⚠️ **Le même défaut a été traité trois fois, sur les trois chemins qui le portaient** : les sondes
(#68), la dalle (#83), et les canaux de ventilation (#88). À chaque fois, un périphérique qui cesse
de répondre à son pilote noyau gelait le fil qui sert le socket.

Pour les canaux, mesuré sur SHYNAEL le 2026-08-09, Kraken en rade :

```
$ status
36,306 s · 811 octets
30,708 s · 811 octets        ← reproductible, pas un hoquet
unreadable kraken2023elite:fan-speed:mode  Connection timed out (os error 110)
```

La fenêtre demande `status` **une fois par seconde**. Mesuré : `geometry`, qui ne touche aucun
matériel, a mis **10,2 s** à répondre pour la seule raison qu'un `status` le précédait. Le boîtier
s'animait à 21 img/s pendant ce temps — d'où l'impression d'une fenêtre morte devant un boîtier
vivant.

⚠️ **La clef d'écartement est le canal entier, pas l'attribut.** Un canal porte `rpm`, `pwm` et
`mode` là où une sonde n'a qu'une valeur ; quand un contrôleur ne répond plus, aucun des trois ne
répond, et écarter attribut par attribut ferait payer trois fois cinq secondes pour l'apprendre.

⚠️ **Le budget des 100 ms vaut en régime établi, la retente exceptée.** Relire est le seul moyen de
savoir si un canal est revenu, et cette relecture coûte ses cinq secondes. Sur une minute de fenêtre
ouverte avec les deux canaux du Kraken en rade : **une** relecture, cinquante-neuf `status`
gratuits, et un coût moyen de 174 ms là où un seul `status` en coûtait 30 000.

### Une sonde muette n'emporte pas le démon

⚠️ **Une lecture sysfs peut bloquer cinq secondes**, en sommeil non interruptible, quand un
périphérique cesse de répondre à son pilote noyau. Mesuré sur SHYNAEL le 2026-08-02, Kraken
planté :

```
$ time cat /sys/class/hwmon/hwmon5/temp1_input      # kraken2023elite
cat: … : Connexion terminée par expiration du délai d'attente
        5,218 total
$ time cat /sys/class/hwmon/hwmon6/fan1_input       # nzxtsmart2
716
        0,001 total
```

Le démon relève ses sondes dans le fil qui sert aussi le socket et tient les bus (ADR-002) :
une seule sonde muette **gelait le service entier**, y compris `zone list` et `geometry`, qui
ne touchent aucun matériel.

Une sonde qui ne répond pas est donc **écartée**. Elle n'est plus lue, la fenêtre l'affiche
illisible, et elle est retentée après un délai qui **double** — 30 s, 1 min, 2 min… — plafonné
à cinq minutes. Le délai n'est pas un ornement : chaque retente coûte ses cinq secondes, et
une retente par minute laisserait le socket muet 8 % du temps.

Une retente réussie la remet en service et remet le délai à zéro. L'écart est **par sonde** :
celle qui répond continue d'être lue quand sa voisine du même contrôleur se tait.

### Une source entièrement muette, le démon le dit — et attend qu'on lui demande

La quarantaine ci-dessus est **purement défensive** : elle empêche une lecture muette de geler
le service, et attend un rétablissement. Sur les trois effondrements du Kraken relevés, il n'est
jamais venu avant un redémarrage.

```
12:29:49  écran : pas de trame 37 02 en 2s — le contrôleur ne répond plus
12:29:56  canal « kraken2023elite:fan-speed » écarté : Connection timed out
12:29:56  canal « kraken2023elite:pump-speed » écarté : Connection timed out
12:30:01  sonde « kraken2023elite:coolant-temp » écartée : Connection timed out
12:30:41  3 échecs d'affilée sur la dalle → écran rendu au firmware
```

Sur les deux premiers incidents, ce n'était ni le lien USB — le périphérique restait énuméré, le
journal noyau ne disait rien — ni le service : la lecture échouait hors de Reverb, dans un simple
shell. **C'est le firmware du Kraken qui cesse de répondre en gardant son lien USB.**

Quand **toutes** les cibles d'une même source `hwmon` se taisent, le démon le **dit une fois**, en
nommant la commande à taper — et n'écrit rien :

```
attention : « kraken2023elite » ne répond plus sur aucune de ses cibles (kraken2023elite:fan-speed,
kraken2023elite:pump-speed, kraken2023elite:coolant-temp).
→ un reset USB peut être tenté : « repare kraken2023elite » — il fait quitter le bus au
  périphérique, et sur les incidents relevés il ne l'a jamais ramené
```

```bash
reverb repare kraken2023elite
echo 'repare kraken2023elite' | socat - UNIX-CONNECT:/run/reverb/reverbd.sock
```

Le geste, lui, n'a pas changé : `USBDEVFS_RESET`, **trois tentatives espacées de trente
secondes**, sur le fil de réparation, puis redécouverte de ce que la source porte.

#### ⚠️ Pourquoi il ne part plus tout seul

**Le 2026-08-16, il a été suivi de la disparition définitive du Kraken.** Le déclenchement
automatique a été retiré ce jour-là (#136).

```
12:53:37  reverbd  écran : pas de trame 37 02 en 2s — le contrôleur ne répond plus
12:53:50  reverbd  réparation : reset USB de « kraken2023elite » (BB8C90820E900630)
12:53:55  kernel   usb 1-9.1: device descriptor read/64, error -110
12:54:53  kernel   usb 1-9.1: USB disconnect, device number 5
12:55:35  kernel   usb 1-9-port1: attempt power cycle
12:55:56  kernel   usb 1-9-port1: unable to enumerate USB device
```

`lsusb` ne voyait plus `1e71:300c`, `hwmon5` avait disparu, et **seule une coupure d'alimentation
complète l'a ramené** — le cycle d'alimentation du port par le noyau n'a pas suffi.

⚠️ **Ce que la chronologie établit, et ce qu'elle n'établit pas.** Le blocage précède le reset de
treize secondes : le reset n'a pas causé la panne. Mais le noyau ne se plaint qu'**après** lui, et
il n'a jamais récupéré le périphérique. Sur un seul incident, « le firmware s'est enfoncé seul » et
« notre reset a transformé un blocage récupérable en un périphérique inénumérable » sont
indiscernables.

Ce qui, lui, est établi sur les **trois** incidents : aucun reset n'a jamais ramené le Kraken. Le
geste ne guérit rien de mesuré, et il est le seul `ioctl` du projet qui fasse **disparaître un
périphérique du bus**. Il garde sa place — sous la main de l'utilisateur, pas dans une boucle.

⚠️ **Le refus est un calcul, pas une relecture.** `demande_de_reparation` ne reçoit que des noms et
des listes : ni descripteur, ni chemin, ni périphérique. « Rien n'est écrit » se lit dans sa
signature — la règle de `refus_de_consigne` (#101), et elle vaut plus encore ici, puisque ce qu'on
refuse est un geste qui fait quitter le bus.

⚠️ **De même, `Veille::tour` ne prend aucune fermeture.** #98 lui en donnait une pour que ce tour
**soit** l'endroit du reset ; on veut l'inverse, et la façon la plus forte de l'obtenir est de ne
pas lui donner de quoi le faire. C'est la règle de `SlotAddress` (#15) et de `NomProfil` (#74),
appliquée à un geste plutôt qu'à une adresse.

⚠️ **`repare` accuse la prise en compte, il ne rend pas le résultat.** Trois tentatives espacées de
trente secondes ne tiennent pas dans une requête socket, et une ligne qui suivrait la fin des
essais se lirait « réparé » alors qu'elle ne dirait que « j'ai fini d'essayer ». Le déroulé va au
journal ; `status` dit si les cibles sont revenues. C'est la règle posée pour `curve` (#104).

⚠️ **Une seule cible muette est refusée, et le geste manuel n'y déroge pas.** C'est la moitié qui
protège la machine : un reset fait disparaître puis réapparaître le périphérique, et le déclencher
sur un contrôleur qui répond encore casse ce qui marchait. « Manuel » ne veut pas dire « sous la
responsabilité de celui qui tape ». Le refus **nomme ce qui répond encore** — « la source répond
encore » invite à réessayer plus tard, « la pompe répond encore » dit ce qu'on aurait cassé.

⚠️ **Une source sans aucune cible relevée est refusée aussi.** « Toutes ses cibles se taisent » est
vrai à vide, et un périphérique dont on n'a jamais rien lu n'a montré aucun symptôme.

⚠️ **La source est jugée sur ce qui a été relevé**, et le démon ne relève **tout** qu'en servant
un `status` — que la fenêtre demande une fois par seconde. Sans fenêtre ouverte, une composition
ou l'animation `thermique` alimentent le constat pour leurs seules sondes ; l'effondrement d'une
source entière ne se constate alors pas.

⚠️ **C'est la source qui répond à nouveau qui remet le compteur à zéro, jamais l'`ioctl` qui rend
`Ok`.** Un reset réussit dès que le noyau a réinitialisé le port ; il ne dit rien de ce que le
firmware fait ensuite, et l'incident est précisément celui d'un périphérique **énuméré qui ne
répond plus**. Sans cette règle le plafond serait écrit et inatteignable, et le démon secouerait
le Kraken jusqu'au redémarrage.

⚠️ **La tentative vit hors du fil qui sert le socket**, comme la dalle depuis #83 : c'est le
quatrième chemin à porter le même défaut, après les sondes (#68), la dalle (#83) et les canaux
(#88). Le fil principal dépose un état, ramasse un verdict, et n'attend ni l'un ni l'autre.

Après un reset qui passe, **cinq secondes** sont laissées au périphérique pour revenir, puis :

| | |
|---|---|
| les sondes et les canaux | **redécouverts par leur nom** — les numéros `hwmonN` ont pu s'échanger |
| les quarantaines de cette source | **oubliées**, cible par cible : elles sont relues sans délai |
| la poignée usbfs de la dalle | **lâchée puis rouverte** — un reset l'invalide |

⚠️ **La redécouverte n'est pas une politesse.** Un `hwmon` qui disparaît puis revient reçoit le
numéro libre, qui n'est pas forcément le sien. Garder l'ancienne poignée, c'est lire le fichier
d'un **autre** périphérique et l'afficher sous le nom du premier : une température plausible sous
le mauvais nom, et rien pour le signaler.

⚠️ **Le périphérique se résout par VID:PID *et série*** — `1e71:300c`, série `BB8C90820E900630`
sur SHYNAEL — et jamais par un chemin conservé : `devnum` est réattribué à chaque énumération,
donc le nœud `/dev/bus/usb/BBB/DDD` change à chaque reset. C'est la règle des `hidraw` du
CLAUDE.md, appliquée à l'USB.

⚠️ **La cause du plantage reste inconnue**, et ceci en traite la conséquence. Deux pistes
ouvertes : l'émission de la dalle est le seul trafic que Reverb envoie au Kraken — mais l'un des
deux incidents a eu lieu émission arrêtée —, et `nzxt-kraken3` parle au même périphérique en HID
noyau pendant que Reverb lui parle en hidraw et en usbfs.

## La RAM Corsair

```bash
reverb ram                                  # emplacements et adresses, sans ouvrir le bus
reverb ram --all --color ff00ff
reverb ram --slot 2 --color 00ff00          # emplacement 2 = 3e barrette depuis le CPU
reverb ram --slot 2 --colors <11 HEX>       # une couleur par LED, de bas en haut
reverb ram --all --animate                  # boucle jusqu'a Ctrl-C
```

**Une couleur fixe tient sans hôte.** Ce contrôleur n'a pas de watchdog : la commande écrit, rend
la main, et l'éclairage reste — y compris après la fermeture de la session.

⚠️ **L'animation, elle, est calculée par l'hôte.** C'est la seule contrainte temps réel du projet :
les ventilateurs NZXT animent seuls, l'écran affiche la température seul, la RAM non. Le mode
« onDevice » d'iCUE a été testé pendant la rétro-ingénierie — négatif. Arrêter la commande fige
l'éclairage sur la dernière image, ce qui est aussi la façon prévue de l'arrêter.

⚠️ **C'est la seule cible du projet où une erreur serait irréversible.** Le même bus porte les hubs
SPD des barrettes en `0x50`–`0x53`, et y écrire rend un DIMM non démarrable. Trois garde-fous :

- `SlotAddress` ne se construit que depuis un index d'emplacement — viser une autre adresse n'est
  pas refusé à l'exécution, c'est irreprésentable, et un test exhaustif sur les 256 entrées
  possibles le vérifie ;
- l'ioctl employé est `I2C_SLAVE`, qui **échoue** si un pilote noyau détient l'adresse, et non
  `I2C_SLAVE_FORCE`, qui passerait outre. `spd5118` devient ainsi une protection ;
- **le bus n'est jamais sondé.** L'adaptateur est reconnu à son nom dans sysfs. Un scan en lecture
  seule avait déjà altéré l'éclairage par défaut de cette RAM.

## Documentation

**Les spécifications de protocole font foi**, et cette copie est la référence :

| Document | Contenu |
|---|---|
| [`docs/SPEC-PROTOCOLE-NZXT.md`](docs/SPEC-PROTOCOLE-NZXT.md) | trames couleur, modes d'animation, LED par LED, initialisation |
| [`docs/SPEC-KRAKEN-LCD.md`](docs/SPEC-KRAKEN-LCD.md) | écran 640×640, luminosité, mode firmware, dérive d'image |
| [`docs/SPEC-CORSAIR-RAM.md`](docs/SPEC-CORSAIR-RAM.md) | SMBus, 11 LED par barrette, CRC-8 |

Chaque affirmation y porte un marqueur : ✅ confirmé par les données · 🔶 hypothèse cohérente
non testée · ❓ inconnu. **Ne rien implémenter à partir d'un ❓.**

## Contenu du repo hors code

| Dossier | Contenu |
|---|---|
| `packaging/` | règle udev livrée par le dépôt, à installer soi-même |
| `tools/windows/` | scripts PowerShell de capture et de décodage (rétro-ingénierie) |
| `tools/*.py` | sondes Python héritées de l'exploration Linux initiale |
| `captures/` | captures USB et SMBus brutes — **hors git**, 178 Mo |

Parmi les sondes Python, `set_color_2019.py` documente une approche qui **échoue** : le
protocole HUE 2 classique ne fonctionne plus sur le contrôleur `0x2019`. Conservé comme trace.

## Contribution amont

Le protocole du `0x2019` était inconnu du monde open source
([liquidctl #541](https://github.com/liquidctl/liquidctl/issues/541), ouverte depuis 2022).
Les spécifications produites ici sont de quoi la fermer.

## Licence

**GPL-3.0-or-later**, comme `Cargo.toml` l'annonce depuis le début du projet — le texte
complet est désormais dans [`LICENSE`](LICENSE), qui manquait.

Ce n'est pas un détail de forme sur un dépôt public : sans le texte, la mention du
`Cargo.toml` ne donne à personne les droits qu'elle prétend accorder.
