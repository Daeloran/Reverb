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
| Catalogue d'animations | ✅ six familles paramétrables |
| Interface Slint | ⏳ à faire |

## Ce que les protocoles permettent

| Cible | Animation | Coût du démon |
|---|---|---|
| Ventilateurs | **par le firmware** | écrit une fois, puis dort |
| Écran — température liquide | **par le firmware** | rien |
| Écran — image personnalisée | l'hôte | 1,2 Mo toutes les ~25 s |
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
| `balayage`, `comete` | 3 à 5 | 50 à 100 img/s |
| `vague`, `respiration`, `arc-en-ciel`, `braise` | **14** | **~20 img/s** |

Vingt images par seconde n'est pas un décrochage : c'est le **plancher physique** de ce matériel
— 29,5 ms de trames HID plus 21,6 ms de blocs SMBus, dont ~3 ms par bloc sur le fil à 100 kHz.
Rien de logiciel n'en descend. Ces animations ont une période de quatre secondes, où vingt images
par seconde restent continues à l'œil ; ce qui saccaderait à cette cadence, ce sont justement les
motifs rapides — et eux tiennent 50 à 100.

`cargo run --release --example densite -p reverb-anim` recalcule ce tableau sans matériel.

Tant que le démon tourne, `reverb set|paint|ram|fan|curve` **refuse d'écrire** — un seul processus
détient les bus (ADR-002), et deux écritures SMBus qui se croisent corrompent une transaction.
`reverb list|modes|fans|screen` continuent de marcher : le démon ne tient pas l'écran, justement
pour garder de quoi diagnostiquer.

### Les animations

```bash
echo 'animate comete couleur=ff00ff vitesse=5 direction=horaire'
echo 'animate arc-en-ciel direction=avant-arriere'
echo 'animate off'
```

Six familles — `vague`, `comete`, `respiration`, `arc-en-ciel`, `balayage`, `braise` — chacune
réglable par `couleur` (six chiffres hexadécimaux), `vitesse` (1 à 10) et `direction`
(`bas-haut`, `haut-bas`, `avant-arriere`, `arriere-avant`, `horaire`, `antihoraire`).
`arc-en-ciel` n'accepte pas de couleur : elle les produit toutes.

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

Deux fichiers, deux natures. La géométrie est une donnée de montage, décidée une fois, et reste
dans `/etc` ; l'éclairage est l'état courant du service, réécrit à chaque commande, et va dans
`/var/lib` (`StateDirectory=reverb`). Les mêler ferait réécrire à chaque changement de couleur le
fichier qui a coûté un relevé au sol.

**Un fichier absent et un fichier disant « noir » ne se confondent jamais.** Le premier est un
premier démarrage : le boîtier s'allume en **bleu pur**, ce qui prouve du même coup que les deux
bus répondent, sans avoir eu à taper une commande. Le second est un choix : `animate off` puis
extinction se retrouve éteint. Sans cette distinction, éteindre volontairement son boîtier le
rallumerait au démarrage suivant.

Un fichier illisible ou tronqué ne bloque pas le démarrage : il est signalé dans le journal, et
l'accueil s'applique. Une entrée absente ou répétée est refusée **en la nommant** — c'est ce qui
rend un fichier tronqué détectable, plutôt que complété au jugé par un éclairage plausible et
faux.

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

`cargo run --release --example apercu -p reverb-gui` dessine la fenêtre **sans écran**, dans un
fichier : c'est ainsi qu'on vérifie une mise en page sans ouvrir de session graphique.

### Ce que la fenêtre ne fait pas

- Elle **n'ouvre aucun périphérique** et **n'écrit aucun fichier**. Tout passe par le socket, qui
  reste l'unique franchissement de privilège (ADR-002).
- La fermer n'éteint rien : le démon continue. Il n'y a **pas d'icône dans la barre système** —
  sur GNOME/Wayland elle dépendrait d'une extension du bureau, qui casse aux montées de version.
- Une LED peinte à la main (`paint`) **ne survit pas à un redémarrage** : `eclairage.conf` garde
  une couleur par cible, pas une par LED (#21). La cible reprend sa couleur unie au démarrage.
  **Une zone, si** — c'est le moyen de rendre une peinture durable : sélectionner les LED, les
  nommer, leur donner leur couleur.

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

```bash
reverb screen                                   # resolution, luminosite, orientation
reverb screen --brightness 40
ffmpeg -i photo.png -vf scale=640:640 -f rawvideo -pix_fmt bgr24 /tmp/img.raw
reverb screen --image /tmp/img.raw              # boucle jusqu'a Ctrl-C
```

L'image fait exactement 1 228 800 octets — 640 × 640 en **BGR**, trois octets par pixel. Reverb ne
décode ni PNG ni JPEG : la conversion est le travail de `ffmpeg`, et l'interface graphique
produira ces octets directement.

⚠️ **La commande boucle, et ce n'est pas un choix.** Le firmware reprend la main une trentaine de
secondes après le dernier envoi ; un affichage durable impose de réémettre. Il n'existe aucune
commande de retour au mode firmware — arrêter la commande est le seul moyen connu d'y revenir.

`reverb screen --mire` affiche quatre quadrants de couleurs connues. C'est la mire qui a confirmé
l'ordre BGR, que la rétro-ingénierie n'avait jamais pu vérifier.

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
