# Spécification — écran LCD du NZXT Kraken Elite (`1e71:300c`)


> Rétro-ingénierie de NZXT CAM 4.76.5, le 2026-07-30.
> ✅ confirmé · 🔶 hypothèse · ❓ inconnu
>
> Capture : `cible2-kraken-init-20260730-105545.pcap` (démarrage de CAM, 80 s, device 6 seul).

---

## 0. Synthèse pratique

| Besoin | Coût | Statut |
|---|---|---|
| Afficher la **température du liquide** | aucun trafic — c'est le comportement par défaut | ✅ |
| Lire **résolution, luminosité, orientation** | une commande HID | ✅ §3.7 |
| Régler la **luminosité** | une commande HID | ✅ |
| Afficher une **image** | 1,2 Mo, à réémettre toutes les ~25 s | ✅ **affichée sous Linux le 2026-07-31** |
| Afficher une **animation** | 1,2 Mo par image, ~2 images/s maximum | 🔶 |
| Garder une image **stable** à l'envoi répété | le paquet de longueur nulle | ✅ §2.2.1 |
| **Revenir** au mode firmware sur commande | aucune trame connue — cesser d'émettre | ❓ §2.3 |

**Recette minimale pour afficher une image :**

```
1. HID   38 01 02 00                  mode de diffusion, bucket 0   <-- INDISPENSABLE
2. HID   30 02 01 <lum> 00 00 00 00 1e   luminosite, AVANT l image
3. HID   36 01 00 01 09               annonce
4. HID   <- 37 01                     ATTENDRE L ACCUSE             <-- INDISPENSABLE
5. BULK  en-tete de 20 octets         (PAS de paquet de longueur nulle : 20 n est
                                       pas un multiple de wMaxPacketSize)
6. BULK  1 228 800 octets  (640x640)  + PAQUET DE LONGUEUR NULLE
7. HID   36 02                        validation
8. HID   <- 37 02                     accuse
9. repeter les etapes 3 a 8 toutes les ~25 s
```

✅ **Vérifiée sur la capture le 2026-07-31** : c'est exactement ce que fait CAM, cinquante fois de
suite, sans jamais rien intercaler. Aucune gestion de bucket dans la boucle — §3.6.

⚠️ **Les trois pièges de cette cible**, tous silencieux — l'envoi réussit, aucun code d'erreur,
rien ne s'affiche :

1. **sans l'étape 1**, l'image est ignorée ;
2. **sans l'attente de l'accusé à l'étape 4**, les données arrivent trop tôt et sont perdues.
   C'est ce qui a fait échouer trois vérifications matérielles d'affilée alors que toutes les
   trames étaient correctes ;
3. **le paquet de longueur nulle est conditionnel**, pas systématique. Il termine l'étape 6, dont
   la taille est un multiple exact de 512. L'ajouter aussi à l'étape 5 insère un transfert
   parasite entre l'en-tête et l'image, et **dégrade jusqu'à l'affichage firmware**.

---

## 1. Interfaces

✅ Périphérique composite exposant :

| Endpoint | Type | wMaxPacketSize | Usage |
|---|---|---|---|
| `0x01` OUT | interrupt | 64 | commandes de contrôle |
| `0x81` IN | interrupt | 64 | réponses et état |
| `0x02` OUT | **bulk** | 512 | transfert des images |

---

## 2. Transfert d'image ✅

**Format : 640 × 640, RGB888 brut, sans compression — 1 228 800 octets par image.**

Chaque envoi se compose de deux transferts bulk consécutifs :

```
1) en-tete de 20 octets
   12 fa 01 e8 | ab cd ef 98 76 54 32 10 | 09 00 00 00 | 00 c0 12 00
   └─ ? ──────┘ └── signature magique ──┘ └─ ? ──────┘ └ longueur ┘
                                                        0x0012c000
                                                        = 1 228 800

2) 1 228 800 octets de pixels
```

✅ Le champ de longueur correspond exactement à la taille du transfert suivant.
✅ La signature `ab cd ef 98 76 54 32 10` est invariante sur les 50 images capturées.
🔶 Les champs `12 fa 01 e8` et `09 00 00 00` sont constants ici — leur rôle est inconnu,
faute d'avoir observé une image de taille ou de format différent.

### 2.1 Validation du format ✅

Une image partielle a été reconstruite depuis la capture (`Extract-KrakenFrame.ps1`) :
les 34 premières lignes se réassemblent en un **arc de cercle centré et symétrique**, le haut
de la jauge circulaire affichée par CAM. Une largeur incorrecte produirait un décalage
progressif et un motif en diagonale — ce n'est pas le cas.

✅ **Ordre des composantes : BGR.** Les pixels de l'arc valent tous `00 3f 3f` environ. Les deux
lectures possibles donnaient turquoise `(0,63,63)` en RGB ou olive `(63,63,0)` en BGR ; la jauge
affichée à l'écran au moment de la capture était **olive**, ce qui tranche pour **BGR**.

✅ **Confirmé par une mire le 2026-07-31**, ce que la dérive avait empêché jusque-là (§2.2.1). Une
image de quatre quadrants — rouge, vert, bleu, blanc — a été envoyée par `reverb screen --mire`.
Observé à l'écran : **rouge en haut à gauche, vert en haut à droite, bleu en bas à gauche, blanc en
bas à droite**, soit exactement la disposition prescrite. Une inversion rouge/bleu aurait échangé
les deux coins concernés. La question ouverte n° 2 est close.

Attention donc : les LED des ventilateurs sont en **GRB** et l'écran en **BGR**. Deux ordres
différents dans le même écosystème NZXT — c'est une source d'erreur à isoler proprement dans le code.

### 2.1.1 La dalle est ronde, le tampon est carré ✅

✅ **Observé sur le matériel le 2026-08-08.** La dalle du Kraken Elite 2023 est **circulaire**,
là où le protocole transporte un tampon carré de 640 × 640.

Conséquence directe, et elle décide de toute mise en page : **les quatre coins du tampon ne
s'affichent nulle part**. Un disque inscrit occupe π/4 de son carré, soit **21 % de la surface
transmise perdue**. Ce qui est écrit là est écrit dans le vide, et aucun message ne le dit — le
contrôleur accepte l'image entière.

⚠️ **La mire des quatre quadrants (§2.1) ne pouvait pas trancher** : un disque montre ses quatre
quadrants exactement comme un carré. C'est l'œil, sur la dalle allumée, qui a répondu.

🔶 **Le rayon exact du disque dans le tampon reste à mesurer.** Savoir que la dalle est ronde ne
dit pas si le tampon la couvre exactement — disque inscrit de 320 pixels de rayon — ou s'il
déborde encore. Une mire d'un cercle inscrit sur fond contrasté le dira d'un coup d'œil
(issue #77).

Le code n'en connaît qu'un endroit : `reverb_proto::screen::VISIBLE_DISC_RADIUS`, à 320 en
attendant la mesure. C'est lui qui décide où un champ de composition peut se poser.

### 2.2 Cadence ✅

Une image par seconde en mode « température du liquide ». **CAM effectue le rendu côté hôte** et
téléverse une image complète à chaque rafraîchissement — le contrôleur n'affiche pas le texte
lui-même. Soit environ 1,2 Mo/s en continu.

⚠️ Lors d'une capture antérieure, le débit atteignait **655 Mo en 113 s** (~5,8 Mo/s, plusieurs
images par seconde). Le régime dépend donc de ce qu'affiche CAM. Toujours filtrer sur le device
du Kraken pour ne pas noyer les autres captures.

### 2.2.1 Envoi d'image reproduit et validé ✅

L'envoi complet a été **rejoué avec succès depuis Windows**, hors de CAM, par
`Send-KrakenImage.ps1`. L'image apparaît bien à l'écran.

**Séquence exacte, relevée puis vérifiée :**

```
HID   (MI_01)  36 01 00 01 09      annonce
BULK  (MI_00)  en-tete 20 octets
BULK  (MI_00)  1 228 800 octets    640x640
HID   (MI_01)  36 02               validation
```

⚠️ **Le mode d'affichage doit être forcé avant l'envoi** : `38 01 02`. Sans cela, l'écran
reste sur son affichage intégré et **ignore silencieusement l'image** — l'envoi réussit, aucun
code d'erreur, mais rien n'apparaît. C'est le piège de cette cible.

**Accès aux deux interfaces :**

| Interface | Pilote | Accès |
|---|---|---|
| `MI_01` (HID) | HIDClass | `CreateFile` + `WriteFile`, trames de 64 octets |
| `MI_00` (bulk) | **WinUSB** (`winusb.inf`, Microsoft) | `WinUsb_Initialize` + `WinUsb_WritePipe` sur le pipe `0x02` |

Sous Windows, le GUID d'interface WinUSB est `{300c300b-7EE7-1125-0724-101503010819}`.
Sous Linux, `libusb` suffit — aucun pilote à installer.

**Débit mesuré : environ 470 ms par image**, soit ~2 images par seconde au maximum.

⚠️ **Défaut de l'implémentation Windows de référence : l'image dérive.** À l'envoi répété, le
contenu se décalait progressivement à l'écran, comme une grille qui défile.

✅ **Cause confirmée et corrigée le 2026-07-31 : le paquet de longueur nulle manquant.**
`1 228 800 = 2400 × 512`, un multiple exact de `wMaxPacketSize` ; la spécification USB exige alors
un **ZLP** pour signaler la fin du transfert, sans quoi le contrôleur ne sait pas où une image
s'arrête et concatène la suivante.

L'hypothèse posée pendant la rétro-ingénierie était donc juste. Vérifiée sous Linux par vingt
envois consécutifs : **l'image ne bouge pas d'un pixel**, frontières entre quadrants comprises.

⚠️ Le ZLP est **conditionnel** — voir le §0, piège n° 3. L'ajouter après l'en-tête de 20 octets,
qui n'est pas un multiple de 512, insère un transfert parasite qui empêche toute image de
s'afficher.

Sous Linux, un second `USBDEVFS_BULK` de longueur nulle suffit ; avec `libusb`, c'est le drapeau
`LIBUSB_TRANSFER_ADD_ZERO_PACKET`.

C'est cette dérive qui avait empêché de **vérifier l'ordre des composantes** — la mire défilant,
les quadrants n'étaient jamais au même endroit. Une fois corrigée, la vérification a pu être faite :
voir §2.1.

### 2.2.3 L'affichage ne survit pas à la fermeture du périphérique ✅

Observé le 2026-07-31 en enchaînant vingt envois **par vingt lancements successifs** du binaire.
Entre chaque image, l'écran repasse brièvement au noir puis à l'affichage firmware, avant que
l'image suivante ne s'affiche. Sur une boucle unique qui garde le périphérique ouvert, rien de tel :
l'image reste affichée sans clignoter.

🔶 Deux causes candidates, non départagées, chacune se produisant une fois par lancement :

- l'interface bulk est **réclamée puis rendue** à chaque ouverture ;
- la trame `38 01 02 00` est **réémise** à chaque lancement, et le §3.4 constate déjà qu'une
  commande d'affichage réinitialise le pipeline.

**Sans conséquence pratique** : un affichage durable impose de toute façon un processus qui réémet
(§2.2.2), et celui-là garde le périphérique ouvert. À trancher seulement si un usage réclame des
envois ponctuels rapprochés.

### 2.2.2 Délai de garde de 30 secondes ✅

Mesuré : une fois les envois arrêtés, **la dernière image reste affichée environ 30 secondes**,
puis le firmware reprend la main et réaffiche « NZXT — 39° Liquid ».

Conséquence : un affichage personnalisé permanent impose de **réémettre l'image au moins toutes
les ~25 secondes**. Ce n'est pas un flux temps réel — deux ordres de grandeur en dessous des
1,2 Mo/s de CAM — mais ce n'est pas non plus une écriture unique.

❓ Il existe peut-être une commande désactivant ce repli ; elle n'a pas été identifiée.

### 2.3 Le contrôleur sait afficher seul ✅ — résultat majeur

Test : CAM fermé plusieurs minutes, aucun téléversement, écran observé.

**Résultat : l'écran affiche « NZXT — 39° Liquid »**, sur fond noir.

Ce n'est **pas** la dernière image figée : CAM affichait une jauge circulaire turquoise, d'une
mise en page entièrement différente. Le contrôleur a donc basculé sur un **mode intégré au
firmware**, qui lit lui-même la température du liquide et la met en page.

Conséquence pour l'implémentation Linux : **afficher la température du liquide ne demande aucun
streaming**. Les 1,2 Mo par seconde ne sont nécessaires que pour un contenu arbitraire (image,
animation, mise en page personnalisée).

⚠️ **Corrigé le 2026-07-31.** Ce paragraphe supposait que le mode intégré se sélectionnait
« vraisemblablement via `38 01 02` ». **C'est faux** : le §3.5 établit que cette trame sélectionne
au contraire le mode de **diffusion**. Aucune commande connue ne ramène l'écran à son affichage
firmware — **il y retombe seul**, au bout des ~30 s du §2.2.2.

❓ Une commande de retour explicite existe peut-être ; elle n'a pas été identifiée. Ne pas en
inventer une : cesser d'émettre suffit, et c'est le seul mécanisme observé.

C'est le renversement de perspective de cette cible : le coûteux est optionnel, et le gratuit est
le comportement par défaut.

---

## 3. Commandes de contrôle (endpoint `0x01`) ✅

### 3.1 Séquence d'initialisation, dans l'ordre observé

```
10 02                        <- meme commande d init que les controleurs RGB
70 02 01 b8 0b               <- 0x0bb8 = 3000
74 01                        <- demande d etat
36 04
30 01
36 03
30 02 00 00 00 00 00 00 1e   <- 0x1e = 30
38 01 02 00                  <- mode de diffusion, bucket 0 (§3.5)
32 02 00                     <- puis 32 02 01 .. 32 02 0f
```

✅ **Corrigé le 2026-07-31 — seize emplacements, pas quinze.** `32 02 <n>` est émis pour `n` de
`0x00` à `0x0f`, ce que confirme le décompte de `tools/extrait_kraken.py` : exactement seize
trames `32 02` sortantes et seize réponses `33 02`. La version antérieure de ce paragraphe partait
de `0x01`.

✅ Ce sont bien les « buckets » de stockage d'images, que `liquidctl` manipule sous ce nom.
Chaque réponse `33 02` porte `01` à l'offset 14 et rien d'autre : les seize emplacements sont
vides.

⚠️ **Cette énumération n'a lieu qu'au démarrage.** La boucle de rafraîchissement du §3.2 ne touche
jamais aux buckets — voir §3.6.

### 3.2 Boucle de rafraîchissement, chaque seconde

```
36 01 00 01 09      -->   annonce l envoi d une image
37 01 ... 01 ...    <--   ACCUSE, 3 ms plus tard
   (les deux transferts bulk du §2 passent ici, ~62 ms)
36 02               -->   validation
37 02 ... 01 ...    <--   ACCUSE
```

⚠️ **Le contrôleur accuse chaque étape, et il faut attendre l'accusé ✅.** C'est le
point qui a coûté le plus cher à l'implémentation Linux : trois vérifications matérielles
successives sans aucune image, alors que toutes les trames étaient correctes. Envoyer les
1,2 Mo sans attendre `37 01`, c'est parler à un contrôleur qui n'écoute pas encore — et
l'échec est silencieux, l'`ioctl` rendant « 1 228 800 octets écrits ».

L'octet à l'**offset 14** porte le verdict : `01` pour un succès. `liquidctl` le teste
(`response[14] == 0x1`) et tous les accusés de la capture le portent.

Les délais relevés : 3 ms entre `36 01` et son accusé, 62 ms pour les deux transferts bulk,
18 ms entre `36 02` et son accusé.

### 3.3 Consignes pompe et ventilateur, chaque seconde

```
72 01 01 00  + 40 octets a 0x42     <- 0x42 = 66
72 02 01 01  + 40 octets a 0x1b     <- 0x1b = 27
```

🔶 Deux courbes plates de 40 points, à 66 % et 27 %. La structure — 40 valeurs identiques —
évoque une courbe indexée sur la température, aplatie parce que le mode est en vitesse fixe.
Même logique que le `62 01` des contrôleurs RGB : **CAM réémet la consigne chaque seconde**.

### 3.4 Luminosité — confirmée ✅

```
30 02 01 <lum> 00 00 00 00 1e
   │  │  │  └── luminosite, 0..100
   │  │  └───── 0x01 : actif
   └──┴──────── commande
```

✅ **Vérifié visuellement** : en alternant `<lum>` entre 5 et 100, l'intensité de l'écran change
nettement. L'octet à l'offset 3 est bien la **luminosité en pourcent**.

🔶 L'octet `0x1e` en fin de trame reste sans rôle établi, mais il vaut **30** — exactement le
délai de repli mesuré au §2.2.2. L'hypothèse qu'il porte ce délai en secondes est cohérente et
**testable** : lui donner une autre valeur et mesurer le repli. Non vérifié à ce jour. Tant que
ça n'est pas fait, reproduire `0x1e` tel quel sans lui prêter de sens.

**Effet de bord constaté** : un changement de luminosité provoque un **bref retour à l'affichage
intégré** (« NZXT — 39° Liquid ») avant que l'image téléversée ne revienne. La commande semble
donc réinitialiser le pipeline d'affichage. À prévoir si l'on enchaîne luminosité et image :
régler la luminosité **avant** d'envoyer l'image, pas après.

### 3.5 Mode d'affichage — `38 01 <mode> <bucket>` ✅

Décodé le 2026-07-31 en recoupant la capture et `_switch_bucket` de `liquidctl`, qui émet
`[0x38, 0x01, mode, bucketIndex]`. La trame de CAM se lit donc :

```
38 01 02 00
   │  │  └── bucket 0
   │  └───── mode 2
   └──────── commande
```

✅ **`38 01 02` sélectionne le mode de diffusion**, celui dans lequel les images téléversées
s'affichent. Deux observations indépendantes concordent : CAM l'émet à l'init puis diffuse des
images qui apparaissent bien, et le rejeu Windows du §2.2.1 a constaté que **sans cette trame
l'image est ignorée en silence**.

⚠️ `liquidctl` nomme ce même mode 2 « liquid » et réserve le mode 4 à l'affichage d'un bucket
stocké. **Ne pas s'y fier** : son pilote est marqué `(broken)` pour le `1e71:300c`, et nos deux
observations disent le contraire. En cas de doute, la capture fait foi.

✅ **Confirmé sur le matériel le 2026-07-31**, et c'est `liquidctl` qui a tort sur ce modèle : la
mire s'affiche après `38 01 02 00`. Le mode 2 est bien le mode de diffusion, pas un mode « liquid ».

### 3.5.1 Les autres modes, mesurés ✅

Session du 2026-07-31, `tools/verifie_ecran.sh`. Une mire envoyée, puis un `38 01 <mode> 00` émis
juste après la validation :

| Mode | Effet observé |
|---|---|
| **2** | l'image reste affichée — c'est le mode nominal |
| 4 | l'image disparaît **avant** les 30 s du repli. Bascule vers l'emplacement 0, qui est vide |
| 1 | **écran noir** |
| 0 | sans effet, l'image reste |

Le mode 4 est celui que `liquidctl` emploie après avoir rempli un emplacement. Il est cohérent
qu'il éteigne une diffusion : il commute l'affichage vers un emplacement stocké, et le nôtre est
vide.

🔶 Le mode 1 éteint peut-être l'écran ; une seule observation, et la luminosité à 0 (§3.4) fait
déjà le travail par un chemin documenté.

### 3.6 Ni bucket, ni gestion mémoire dans la boucle ✅

Résultat **négatif et acquis**, établi par `tools/extrait_kraken.py` sur la capture d'init.

`liquidctl` interroge les buckets, en cherche un libre, calcule un offset mémoire, le configure,
transfère, puis bascule le bucket actif — à **chaque** image. **CAM ne fait rien de tout cela.**
Sur les cinquante images de la capture, la boucle est strictement :

```
36 01 00 01 09   →     annonce, invariante d'une image à l'autre
   en-tete bulk + 1 228 800 octets
36 02            →     validation
37 01 / 37 02    ←     accuses, offset 14 = 01
```

Aucune trame `32` ni `38` entre deux images. Les seize `32 02 <n>` de l'init ne se reproduisent
jamais.

**Conséquence sur la dérive du §2.2.1** : elle ne peut pas venir d'une gestion mémoire absente du
protocole réel. L'hypothèse d'origine — **le paquet de longueur nulle manquant** — reste la seule
qui tienne, et c'est bien elle qu'il faut corriger en premier.

### 3.7 État de l'écran — `30 01` / `31 01` ✅

Décodé le 2026-07-31. La demande `30 01` est sans paramètre ; la réponse porte la géométrie et les
réglages courants :

```
31 01 bb 8c 90 82 0e 90 06 30 00 00 00 00 05 00 80 00 00 10 80 02 80 02 50 01 00 ff ...
                                                            └─┬─┘ └─┬─┘ │     │
offset 0x14-0x15  largeur      80 02 = 640  ─────────────────┘      │   │     │
offset 0x16-0x17  hauteur      80 02 = 640  ────────────────────────┘   │     │
offset 0x18       luminosite   50    = 80 %  ───────────────────────────┘     │
offset 0x1a       orientation  00                                             │
                                                                    offset 0x19 = 01 ❓
```

Entiers **petit-boutistes**. Les offsets de luminosité et d'orientation correspondent à ceux que
lit `liquidctl` (`msg[0x18]` et `msg[0x1a]`) — ici les deux sources concordent.

✅ **Le contrôleur annonce lui-même sa résolution.** Le 640×640 du §2 n'est donc plus une déduction
à partir de la taille des transferts : le matériel le déclare.

✅ La luminosité relue vaut 80, et la trame `30 02 01 50 …` émise par CAM porte `0x50` = 80. Écriture
et lecture concordent, ce qui confirme une deuxième fois l'offset 3 du §3.4.

❓ Les octets `0x19` (`01`) et `0x1b` (`ff`) restent inexpliqués, ainsi que `0x12-0x13` (`00 10`,
soit 4096).

---

## 4. Recoupement avec liquidctl

Plusieurs commandes observées correspondent à celles que `liquidctl` implémente déjà pour la
famille Kraken Z3 / Elite : `74 01` (état), `38 01 02` (mode d'affichage), `32 02 <n>`
(emplacements), et une résolution de 640×640.

**À faire avant toute réimplémentation** : tester `liquidctl --match kraken set screen ...` sous
Linux. Il est probable que le seul obstacle soit une règle udev manquante, comme le suggérait la
fiche de préparation. Le décodage ci-dessus sert alors de vérification, pas de base de réécriture.

---

## 5. Questions ouvertes

| # | Question | Comment trancher |
|---|---|---|
| ~~1~~ | ~~Corriger la dérive de l'image~~ | ✅ **tranché — §2.2.1, c'était bien le paquet de longueur nulle** |
| ~~2~~ | ~~RGB ou BGR confirmé par une mire~~ | ✅ **tranché le 2026-07-31 — §2.1, c'est BGR** |
| 3 | Rôle des octets `12 fa 01 e8` et `09 00 00 00` de l'en-tête | `09 00 00 00` occupe la place du sélecteur de contenu de `liquidctl` (`01` gif, `02` image fixe) ; CAM y met une troisième valeur. Envoyer une image de taille différente |
| 4 | Écrire l'orientation | lue en `31 01` offset `0x1a` (§3.7), mais l'offset en **écriture** n'est pas établi : la trame de CAM et celle de `liquidctl` ne coïncident pas |
| 5 | Peut-on désactiver le repli de 30 s ? | 🔶 l'octet `0x1e` de `30 02` vaut 30 (§3.4). Lui donner une autre valeur et mesurer |
| 6 | **Existe-t-il une commande de retour au mode firmware ?** | non identifiée. Cesser d'émettre suffit — §2.3 |
| 7 | Autres valeurs de `<mode>` dans `38 01` | seul le mode 2 est observé sur ce modèle — §3.5 |
| 8 | **Rayon exact du disque visible dans le tampon** | la dalle est ronde (§2.1.1, observé) ; reste à savoir si le disque est celui inscrit à 320 px. Une mire d'un cercle inscrit, coins d'une couleur distincte — issue #77 |
| ~~—~~ | ~~Luminosité~~ | ✅ **tranché — §3.4, reconfirmé §3.7** |
| ~~—~~ | ~~Mode d'affichage autonome~~ | ✅ **tranché — §2.3, corrigé le 2026-07-31** |
| ~~—~~ | ~~Structure de `38 01`~~ | ✅ **tranché — §3.5** |
| ~~—~~ | ~~Rôle des buckets dans la boucle~~ | ✅ **tranché — §3.6, ils n'y jouent aucun rôle** |
| ~~—~~ | ~~Lecture de l'état de l'écran~~ | ✅ **tranché — §3.7** |
| ~~—~~ | ~~Faut-il attendre les accusés ?~~ | ✅ **oui, et c'est indispensable — §3.2** |
| ~~—~~ | ~~Sémantique des modes 0, 1 et 4~~ | ✅ **mesurée — §3.5.1** |

⚠️ **Limite de capture** : USBPcap tronque à 65 535 octets. Les images ne sont donc capturées
qu'à 5 % (65 508 octets sur 1 228 800). Suffisant pour valider en-tête, cadence et géométrie ;
insuffisant pour reconstruire une image entière.
