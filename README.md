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
| Interface Slint | ✅ maquette habillée, deux vues, zones, sondes, écran |
| Écran du Kraken dans le démon | ✅ luminosité, cadran, image, GIF |
| Profils — une ambiance sous un nom | ✅ éclairage, zones et écran, deux exemples livrés |

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
   └── I2C_SMBUS    ──►  /dev/i2c-8 0x18..1b RAM Corsair, RGB

reverb  (outil de validation, garde l'usbfs ──► 1e71:300c bulk, écran Kraken, BGR)
   │
   ├── reverb-hw     (les quatre chemins d'E/S — hidraw, usbfs, i2c, hwmon)
   ├── reverb-anim   (géométrie du boîtier + catalogue d'animations — pur)
   └── reverb-proto  (encodage des trames, conversions, CRC-8, protocole IPC — pur)
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
| `vague`, `respiration`, `arc-en-ciel`, `braise`, `rotation`, `scintillement` | **14** | **~20 img/s** |

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

Dix familles, réglables par `couleur` (six chiffres hexadécimaux), `vitesse` (1 à 10) et
`direction`. Chacune n'accepte que ce qu'elle sait porter — une clé de trop fait refuser la
commande **entière**, pas seulement la clé.

| famille | ce qu'elle fait | réglages |
|---|---|---|
| `vague` | l'onde plane, le long de la direction | couleur, vitesse, direction |
| `comete` | une tête vive suivie d'une traînée | couleur, vitesse, direction |
| `respiration` | le boîtier respire, et la respiration se propage | couleur, vitesse, direction |
| `arc-en-ciel` | le spectre déroulé — elle produit ses teintes | vitesse, direction |
| `balayage` | une bande nette dont on voit la limite bouger | couleur, vitesse, direction |
| `braise` | deux ondes incommensurables, sans cycle apparent | couleur, vitesse, direction |
| `rotation` | **chaque anneau tourne sur lui-même** | couleur, vitesse |
| `thermique` | **la couleur suit une sonde** | vitesse, **sonde** |
| `pouls` | **une onde sphérique née à la pompe** | couleur, vitesse |
| `scintillement` | **des LED s'allument au hasard** | couleur, vitesse |

Huit directions : `bas-haut`, `haut-bas`, `avant-arriere`, `arriere-avant`, `horaire`,
`antihoraire`, et les deux **locales** — `bords-centre`, `centre-bords`.

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

#### `rotation`, `pouls`, `scintillement` — trois motifs qui ne suivent aucun axe

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
`bas-haut` les allumerait d'un seul bloc. Cinq familles suivent donc l'**écoulement** plutôt que
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

### L'éclairage retrouvé

Le boîtier retrouve seul, après un redémarrage, ce qu'il affichait — une couleur fixe comme une
animation avec ses réglages. Le démon écrit son état dans `/var/lib/reverb/eclairage.conf` à
**chaque changement**, pas à l'arrêt : ce qu'on veut retrouver, c'est justement l'éclairage
d'avant une coupure de courant, qui ne laisse le temps d'écrire nulle part.

Deux natures, quatre fichiers et un répertoire. La géométrie est une donnée de montage, décidée une
fois, et reste dans `/etc` ; l'éclairage, les zones, l'écran et [les profils](#les-profils--une-ambiance-sous-un-nom)
sont l'état courant du service, réécrits à chaque commande, et vont dans `/var/lib`
(`StateDirectory=reverb`). Les mêler ferait réécrire à chaque changement de couleur le fichier qui
a coûté un relevé au sol.

L'écran suit la même règle : `ecran.conf` garde sa luminosité et **le chemin** de ce qu'il montre,
jamais les pixels. Au redémarrage le démon relit le fichier ; s'il a disparu depuis, la dalle reste
au firmware et le journal le dit **une fois**, sans boucler.

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

Le boîtier y est dessiné **vu depuis le panneau latéral gauche, face à la carte mère** :
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

Les deux vues sont habillées d'un châssis, de parois et des organes internes — plateau de carte
mère, carte graphique, cache d'alimentation —, d'un cadre par ventilateur, du corps des quatre
barrettes et de la dalle du Kraken. **Aucune de ces formes n'a de coordonnée dans le `.slint`** :
toutes viennent de `plan.rs`, sinon la maquette divergerait de la géométrie à la première
correction.

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
fichier : c'est ainsi qu'on vérifie une mise en page sans ouvrir de session graphique.

### Ce que la fenêtre ne fait pas

- Elle **n'ouvre aucun périphérique** et **n'écrit aucun fichier**. Tout passe par le socket, qui
  reste l'unique franchissement de privilège (ADR-002).
- La fermer n'éteint rien : le démon continue. Il n'y a **pas d'icône dans la barre système** —
  sur GNOME/Wayland elle dépendrait d'une extension du bureau, qui casse aux montées de version.
- Elle ne montre **pas les seize sondes** que la machine expose, seulement quatre familles — CPU,
  liquide, GPU, et un disque NVMe par SSD — sous des noms qui se lisent. Le démon, lui, continue de
  toutes les découvrir et de toutes les rendre : le tri est un choix d'affichage, pas un filtre de
  relevé.
- Le bouton **« auto » n'apparaît que sur les deux canaux du Kraken**. Le pilote `nzxt-smart2` n'a
  aucun mode automatique — sa vitesse est celle que l'hôte écrit —, et montrer un bouton qui ne
  peut qu'échouer vaut moins que ne pas le montrer.
- Une sonde qui cesse de répondre s'affiche **illisible**, et le reste de la fenêtre continue à
  pleine vitesse. Voir [ci-dessous](#une-sonde-muette-nemporte-pas-le-démon).
- Une LED peinte à la main (`paint`) **ne survit pas à un redémarrage** : `eclairage.conf` garde
  une couleur par cible, pas une par LED (#21). La cible reprend sa couleur unie au démarrage.
  **Une zone, si** — c'est le moyen de rendre une peinture durable : sélectionner les LED, les
  nommer, leur donner leur couleur.
- Le chemin d'une image se **colle dans un champ de texte**, il ne s'ouvre pas dans un sélecteur de
  fichiers : une boîte de dialogue demanderait le portail XDG, donc un client D-Bus, donc une
  dépendance de runtime que l'ADR-001 refuse. C'est le seul endroit de la fenêtre où l'on tape au
  lieu de cliquer, et c'est un manque assumé.

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

**Il ne dépend d'aucune pile de texte** : des chiffres à sept segments et une police matricielle de
5 × 7, dessinés à la main dans le tampon 640 × 640. Charger un moteur de rendu de police pour
afficher « 34.2 » serait hors de proportion, et ajouterait une bibliothèque système à un démon qui
n'en veut pas.

```bash
cargo run --release --example cadran -p reverb-daemon -- /tmp/cadran.ppm 34.2 0.34
```

dessine un cadran **sans matériel**, dans un fichier : c'est ainsi qu'on vérifie « lisible à un
mètre » sans brancher de Kraken.

⚠️ **Une sonde muette affiche des tirets, jamais un zéro.** C'est le mode de défaillance le plus
coûteux du cadran, parce qu'il est rassurant : un 34 °C figé derrière une pompe arrêtée, c'est un
circuit qui chauffe sans que rien ne le signale.

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
| rien | jamais — le démon est au repos absolu |

### Ce qui reste en direct

`reverb screen --mire` affiche quatre quadrants de couleurs connues. C'est la mire qui a confirmé
l'ordre BGR, que la rétro-ingénierie n'avait jamais pu vérifier. Elle n'est **pas** dans le
protocole — c'est un outil de diagnostic — et elle écrit donc en direct, ce qui suppose le démon
arrêté : le nœud USB ne se réclame pas deux fois.

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
