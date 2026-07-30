# Spécification du protocole NZXT — contrôleurs RGB & ventilateurs


> Rétro-ingénierie de NZXT CAM 4.76.5 sous Windows 11, le 2026-07-30. Cible : réimplémentation Linux (projet Reverb).
>
> ✅ confirmé par les données · 🔶 hypothèse cohérente non testée · ❓ inconnu

Captures sources, dans `C:\Users\Lupink\Downloads\rgb-capture\` :
- `cible0-cam-restart-20260730-095647.pcap` — redémarrage de CAM (init complet)
- `cible1-modes-nzxt-20260730-102329.pcap` + journal — 12 actions guidées

---

## 0. Validation sur le matériel ✅

**La spécification a été rejouée sur la machine, CAM fermé, et fonctionne.**
Outil : `Set-NzxtColor.ps1` (HID brut via SetupAPI + WriteFile, sans dépendance).

| Test | Résultat |
|---|---|
| `0x2a 0x04`, couleur fixe verte sur les 6+3+1 canaux | ✅ les dix ventilateurs passent au vert |
| Ordre **GRB** | ✅ confirmé — une lecture RGB aurait donné du rouge |
| `0x22 0x10/0x11/0xa0`, arc-en-ciel sur 8 LED | ✅ les 8 LED de chaque ventilateur prennent des couleurs distinctes |

Deux détails d'implémentation découverts à l'écriture, absents des captures :

- ⚠️ `OutputReportByteLength` vaut **64 et non 65** : le premier octet de la trame
  (`0x2a`, `0x22`, `0x62`) est **l'identifiant de rapport HID**. Il ne faut donc **pas**
  préfixer un octet `0x00` comme le veut la convention habituelle — on écrit les 64 octets tels quels.
- ⚠️ **CAM ouvre les périphériques en exclusif.** Toute écriture concurrente échoue avec
  `ERROR_SHARING_VIOLATION` (32). Sans objet sous Linux, mais explique l'échec si l'on teste
  sous Windows sans fermer CAM.

🔶 Ces écritures ont réussi **sans rejouer la séquence d'initialisation du §8**, CAM l'ayant
déjà jouée depuis le démarrage. La nécessité de cette séquence sur un système démarré sans CAM
reste donc non vérifiée — c'est le premier point à tester sous Linux.

### 0.2 Les trois commandes `0x22` sont toutes nécessaires ✅

Test par élimination : après avoir mis tous les ventilateurs en vert uni, envoi de la **seule**
trame `22 10` portant un arc-en-ciel, sans `22 11` ni `22 a0`.

**Résultat : aucun changement, les ventilateurs restent verts.**

`22 10` ne fait donc qu'alimenter un tampon ; **`22 11` puis `22 a0` sont indispensables** pour
que le contrôleur applique le contenu. La séquence complète du §5.2 est obligatoire.

### 0.3 Aucun watchdog — l'état est conservé sans hôte ✅

Test : CAM fermé, écriture d'une couleur, puis **60 secondes sans aucun trafic**.

**Résultat : la couleur tient.** Aucun logiciel ne parlait aux contrôleurs pendant ce temps.

Le `62 01` réémis chaque seconde par CAM n'est donc **pas** un watchdog, mais un simple
rafraîchissement de consigne de sa part. Conséquence directe : un démon Linux **écrit une fois
puis peut dormir** — inutile de maintenir une boucle de rafraîchissement.

---

## 1. Transport

✅ Trois contrôleurs RGB, plus le Kraken, tous sur le même contrôleur hôte AMD xHCI.

| Adresse USB | VID:PID | Rôle |
|---|---|---|
| 6 | `1e71:300c` | Kraken Elite (écran LCD + HID) |
| 7 | `1e71:2019` | 6 canaux LED, 3 canaux ventilateur |
| 8 | `1e71:2012` | 1 canal LED utilisé |
| 9 | `1e71:2012` | 3 canaux LED utilisés |

✅ **HID interrupt, paquets de 64 octets**, endpoint OUT `0x02` / IN `0x81`, `bInterval` = 1.
Pas de report ID : la charge utile commence par l'octet de commande. Les paquets sont
toujours complétés à 64 octets par des zéros.

⚠️ Le Kraken diffuse son écran en continu sur un **bulk OUT de 512 octets** : 655 Mo en 113 s.
Toujours capturer avec `--devices 7,8,9` pour isoler le RGB.

---

## 2. Ordre des composantes : GRB ✅

**Les couleurs partent dans l'ordre G, R, B.** Prouvé par texte clair connu (dix ventilateurs
réglés sur dix couleurs distinctes, puis huit LED d'un même ventilateur peintes séparément).

Le rouge pur part en `00 ff 00` et le **vert pur en `ff 00 00`**. Une lecture RGB naïve
intervertit rouge et vert sur toute l'installation.

> ⚠️ **Formulation corrigée le 2026-07-30.** Cette section affirmait « le vert en `ff 28 00` ».
> C'est exact comme observation — mais `ff 28 00` relu en GRB donne `(40, 255, 0)`, qui est le
> **vert du nuancier CAM** relevé au §5.1, pas le vert pur. Présenté comme illustration de
> l'ordre GRB, c'était trompeur : quiconque implémentait depuis cette phrase pouvait croire à un
> décalage supplémentaire. Le vert pur `#00ff00` part bien en `ff 00 00`.

## 3. Cartographie des canaux ✅

> ⚠️ **Table corrigée le 2026-07-30 sous Linux.** La version issue de la session Windows était
> fausse sur **deux groupes sur quatre** : les libellés y avaient été posés de mémoire, sans
> vérification canal par canal.
>
> Celle-ci a été établie par **calibration directe** — les dix canaux allumés en rouge/vert/bleu
> au sein de chaque groupe, puis relevés à l'œil. Détail des écarts en fin de section.

| Device | Numéro de série | Masque | Position physique |
|---|---|---|---|
| 7 | `1303F00AAAAD9529610494BE` | `0x01` | bas gauche |
| 7 | `1303F00AAAAD9529610494BE` | `0x02` | bas milieu |
| 7 | `1303F00AAAAD9529610494BE` | `0x04` | bas droite |
| 7 | `1303F00AAAAD9529610494BE` | `0x08` | **radiateur haut** |
| 7 | `1303F00AAAAD9529610494BE` | `0x10` | **radiateur milieu** |
| 7 | `1303F00AAAAD9529610494BE` | `0x20` | **radiateur bas** |
| 8 | `0E014044AB7664C25F063BD5` | `0x01` | **arrière** |
| 9 | `1101F021AA358489609AA5B2` | `0x01` | **haut gauche** |
| 9 | `1101F021AA358489609AA5B2` | `0x02` | haut milieu |
| 9 | `1101F021AA358489609AA5B2` | `0x04` | **haut droite** |

> **Le numéro de série fait référence, pas l'adresse USB.** Celle-ci change d'un démarrage à
> l'autre et ne s'observe pas de la même façon sous Linux ; la série est exposée par `HID_UNIQ`
> dans `/sys/class/hidraw/*/device/uevent`.
>
> Colonne ajoutée le 2026-07-30 : sans elle, relier un device à sa série passait par un
> raisonnement indirect — « celui qui n'a qu'un canal utilisé » — qui casserait silencieusement
> si un ventilateur était ajouté.

Disposition réelle : **3 en bas**, **3 sur l'avant** — le radiateur du Kraken, plaqué contre la
face de la carte mère —, **3 sur le dessus**, **1 à l'arrière**.

⚠️ Les trois ventilateurs du radiateur sont sur le device **7**, pas sur le Kraken. La pompe n'a
aucune LED.

<details>
<summary>Ce qui était faux, et comment ça a été tranché</summary>

| Masque | Ancien libellé | Réel | Verdict |
|---|---|---|---|
| `7 / 0x01`–`0x04` | bas gauche / milieu / droite | idem | ✅ |
| `7 / 0x08` | droit bas | radiateur **haut** | ❌ inversé |
| `7 / 0x10` | droit milieu | radiateur milieu | ✅ |
| `7 / 0x20` | droit haut | radiateur **bas** | ❌ inversé |
| `8 / 0x01` | gauche | **arrière** | ❌ faux |
| `9 / 0x01` | haut droite | haut **gauche** | ❌ inversé |
| `9 / 0x02` | haut milieu | haut milieu | ✅ |
| `9 / 0x04` | haut gauche | haut **droite** | ❌ inversé |

L'écart a été découvert en allumant un seul ventilateur : la commande visant `0x08` a allumé
celui du **haut** du radiateur. Un groupe erroné rendant les autres suspects, les dix canaux ont
été recalibrés d'un bloc plutôt que corrigés au cas par cas — et deux autres erreurs sont
apparues.

**Leçon** : l'appartenance d'un canal à un contrôleur est fiable, elle vient des captures.
L'étiquette physique ne l'est pas — elle vient d'une observation humaine, et se vérifie
canal par canal.

</details>

✅ Un bit par canal. Confirmé en isolant un seul ventilateur (actions 3 et 4) : seule la trame
au bit attendu est émise.

✅ **CAM n'agrège jamais plusieurs canaux dans une trame.** Même en appliquant une couleur à
tous les ventilateurs d'un coup, il envoie une trame par canal, séquentiellement. Les offsets 2
et 3 sont restés égaux sur **toutes** les trames observées — ❓ leur rôle distinct reste inconnu.

---

## 4. Commande `0x2a 0x04` — modes prédéfinis

Le mode d'animation est exécuté **par le contrôleur**. L'hôte n'envoie que les paramètres.

```
offset  0    1     2     3     4     5      6     7..55        56..59
       0x2a 0x04 masque masque mode vitesse  ?   couleurs      trailer
```

| Offset | Contenu | Preuve |
|---|---|---|
| 0–1 | `2a 04` | ✅ |
| 2 | masque de canaux | ✅ |
| 3 | toujours égal à l'offset 2 | ✅ observé · ❓ rôle |
| 4 | **mode** — voir §4.1 | ✅ |
| 5 | **vitesse** de l'animation | ✅ voir §4.2 |
| 6 | `0x00` sauf mode `0x05` où il vaut `0x01` ou `0x03` | 🔶 **variante**, pas direction — §4.4 |
| 7…  | couleurs, triplets GRB consécutifs, autant que le trailer en annonce | ✅ |
| 56 | **nombre de couleurs fournies** | ✅ vaut **toujours au moins 1** — §4.4 |
| 57 | `0x00` ou `0x08` — **constante propre au mode** | ✅ valeur connue pour les 8 modes, §4.4 |
| 58 | `0x08` — **nombre de LED de l'accessoire** | 🔶 |
| 59 | `0x03` — type d'accessoire | 🔶 |

### 4.1 Modes observés

| Mode | Couleurs | off6 | off57 | Vitesses vues | Identification |
|---|---|---|---|---|---|
| `0x00` | 1 | `0x00` | `0x00` | `0x32` | ✅ **couleur fixe** |
| `0x01` | 3 | `0x00` | `0x08` | `0x28` | ✅ **Fading** |
| `0x02` | 1 (noire) | `0x00` | `0x00` | `0xfa`, `0x50` | ✅ **Spectrum Wave** — le contrôleur génère les teintes |
| `0x03` | — | — | — | — | ❓ jamais déclenché pendant la capture |
| `0x04` | 2 ou 3 | `0x00` | `0x00` | `0xfa` | ✅ **Covering Marquee** |
| `0x05` | **exactement 2** | `0x01`, `0x03` | `0x00` | `0xf4`, `0xe8` | ✅ **Alternating** — voir ci-dessous |
| `0x06` | 1 | `0x00` | `0x08` | `0x0f` | ✅ **Pulse** |
| `0x07` | 1 | `0x00` | `0x08` | `0x14` | ✅ **Breathing** |
| `0x09` | 1 | `0x00` | `0x00` | `0x0f` | ✅ **Starry Night** |

> Colonnes `off6`, `off57` et vitesses **extraites de la capture** `cible1-modes-nzxt` le
> 2026-07-30 sous Linux, par `tools/extrait_modes.py`. Les numéros de mode sont certains ✅.

Les noms venaient d'un recoupement avec la table HUE 2 de liquidctl, dont les numéros de mode
coïncident. **Les huit ont été vérifiés à l'œil** le 2026-07-30 — voir §4.5. Le seul mode encore
inconnu est `0x03`, jamais déclenché.

`0x05` était le recoupement le plus solide : Alternating est le **seul** mode dont liquidctl fixe
le minimum **et** le maximum à 2 couleurs, et c'est exactement ce qu'on observe — jamais 1,
jamais 3. Ça éclaire aussi l'offset 6, qui sélectionne chez liquidctl la taille des blocs
alternés (quatre variantes).

### 4.4 Ce que la capture a tranché ✅

Trois inconnues levées le 2026-07-30, sans toucher au matériel.

**L'offset 56 ne descend jamais à zéro.** Le §4 affirmait « vaut 1, 2 ou 3 », le §4.1 attribuait
« 0 couleur » à Spectrum Wave — contradiction apparente. La trame réelle porte `off56 = 0x01`
avec une couleur **noire** `#000000`. Le mode ignore la couleur fournie mais le compteur reste à
1. Une implémentation qui écrirait `0x00` s'écarterait de ce que fait CAM.

**L'offset 57 est une constante propre au mode**, pas du bruit : `0x08` pour `0x01`, `0x06` et
`0x07` ; `0x00` pour `0x00`, `0x02`, `0x04`, `0x05` et `0x09`. Sa signification reste ❓, mais sa
valeur est désormais connue pour les huit modes observés — il suffit de la reproduire.

**L'offset 6 est un sélecteur de variante, pas une direction.** En mode `0x05` il vaut `0x01`
puis `0x03`, et la vitesse change en même temps (`0xf4` → `0xe8`). Une direction serait binaire ;
quatre valeurs possibles correspondent aux quatre tailles de blocs d'Alternating.

### 4.5 Confirmation à l'œil des noms de modes

Session du **2026-07-30** sous Linux, via `tools/confirme_modes.sh`. Chaque mode a été déclenché
sur les dix ventilateurs ; l'observateur décrivait ce qu'il voyait **avant** que l'hypothèse ne
lui soit montrée, pour ne pas la lui suggérer.

| Mode | Observé | Verdict |
|---|---|---|
| `0x01` Fading | « changement de couleurs unies douce du ventilateur complet » | ✅ |
| `0x04` Covering Marquee | « les couleurs recouvrent la surface LED par LED sur chaque ventilo » | ✅ |
| `0x05` Alternating | « les LED changent de couleur en groupe » | ✅ |
| `0x06` Pulse | « pulsation abrupte » | ✅ |
| `0x09` Starry Night | « des LED isolées s'allument et s'éteignent au hasard » | ✅ (2ᵉ passe) |

Chaque description est **spécifique** : elle nomme un mécanisme (fondu, recouvrement LED par
LED, alternance par groupes, battement sec, LED isolées au hasard) qui distingue ce mode des
autres, et pas seulement « ça bouge ». C'est le seuil retenu pour passer à ✅.

`0x09` a demandé deux passes. La première — rouge à la vitesse `0x0f` — n'avait donné qu'un
« je crois voir un léger scintillement mais ce n'est pas très net », compatible avec Starry
Night mais **pas discriminant** : un battement lent et faible aurait produit la même phrase. La
seconde passe, en **blanc** à la vitesse `0x50`, a tranché : ce sont bien des LED **isolées**,
allumées au hasard, et non le ventilateur entier qui monte et descend. L'observateur note que
c'est « très rapide et peu intense » même à cette vitesse.

> Leçon de méthode : une observation qui *concorde* avec l'hypothèse ne la *confirme* pas. Il
> faut qu'elle exclue les autres candidats. Changer la couleur et la vitesse a suffi ici.

Le fait que `0x05` alterne bien **par groupes de LED** conforte au passage la lecture de
l'offset 6 comme sélecteur de taille de bloc (§4.4), sans le prouver : la taille n'a été testée
qu'à la valeur `0x01`.

### 4.2 L'octet 5 est la vitesse, pas la luminosité ✅

Correction d'une hypothèse antérieure. Le changement de vitesse (action 9) fait passer l'octet 5
de `0xfa` à `0x50` en mode Spectrum Wave, tandis que les deux réglages de luminosité (actions 5
et 6) le laissent inchangé. Valeurs observées : `0x32` en fixe, `0xfa` puis `0x50` en Spectrum
Wave, `0x14` en Breathing, `0x0f`, `0x28`, `0xe8`, `0xf4` selon les modes.

🔶 Valeur élevée = animation lente.

### 4.3 La luminosité n'existe pas dans le protocole ✅

**Important pour l'implémentation.** Aucun octet ne porte la luminosité. CAM l'applique
**côté hôte** en multipliant les composantes avant l'envoi :

- luminosité au minimum → CAM envoie `#000000` sur tous les canaux ;
- les transitions montrent la rampe explicite : `#34004f` → `#32004c` → `#12001c` → `#050007` → noir.

Sous Linux, la mise à l'échelle est donc à faire soi-même. Il n'y a rien à régler côté matériel.

---

## 5. Commande `0x22` — pilotage LED par LED ✅

**C'est la famille qui répond à l'objectif.** Elle est distincte de `0x2a 0x04` et n'apparaît
que lorsque CAM peint des LED individuellement.

### 5.1 Écriture du tampon — `0x22 0x10`

```
22 10 <masque> <index> | <8 triplets GRB, 24 octets> | padding jusqu'a 64
```

✅ Observation directe : huit LED d'un même ventilateur colorées une par une dans CAM, chaque
modification réémettant le tampon complet.

```
22 10 01 00 | 0000ff 00ff00 ff2800 e5ff00 00a9ff ff00b4 50ff00 9f3e2d
              LED1   LED2   LED3   LED4   LED5   LED6   LED7   LED8
              bleu   rouge  vert   jaune  rose   cyan   orange olive
```

- offset 2 = masque de canal, même encodage qu'au §3 ✅
- offset 3 = `0x00` — 🔶 index de départ, permettant de chaîner plusieurs paquets pour les
  accessoires de plus de 8 LED. **Non vérifiable ici** : tous les accessoires en font exactement 8.
- offsets **4 à 27** = les 24 octets de couleur ✅ ; **28 à 63** restent nuls ✅

La capture contient la peinture progressive elle-même : onze trames `22 10` successives, LED par
LED, extraites par `tools/extrait_22.py`. Les LED pas encore peintes portent `ff ff ff` — le
tampon part du blanc, il n'est jamais partiel.

```
22 10 01 00  ff ff ff ff ff ff ...                        <- tampon initial, tout blanc
22 10 01 00  0000ff ffffff ffffff ...                     <- LED 1 peinte
22 10 01 00  0000ff 00ff00 ffffff ...                     <- LED 2 peinte
                                        …
22 10 01 00  0000ff 00ff00 ff2800 e5ff00 00a9ff ff00b4 50ff00 9f3e2d
```

C'est la preuve directe qu'**il n'existe aucune écriture d'une seule LED** : chaque modification
réémet les 24 octets. Une implémentation qui voudrait peindre une LED sans toucher aux autres
devrait donc tenir l'état côté hôte — le protocole ne le lui rendra pas.

### 5.2 Séquence complète

✅ Les trois trames partent toujours groupées, en quelques millisecondes :

```
22 10 01 00 <24 octets de couleurs>                  <- tampon
22 11 01                                             <- validation du canal
22 a0 01 00 01 00 00 08 00 00 80 00 32 00 00 01      <- application
```

**Les deux variantes de `22 a0` sont attestées** ✅ — extraites de la capture le 2026-07-30 par
`tools/extrait_22.py`, sans nouvelle session Windows :

```
22 a0 01 00 01 00 00 08 00 00 80 00 32 00 00 01      <- statique  (×10)
22 a0 01 00 02 6a 00 08 00 00 80 00 32 00 00 01      <- animé     (×1)
              ^^ ^^^^^
              |  vitesse 0x006a, uint16 little-endian
              mode
```

Seuls les offsets 4, 5 et 6 changent entre les deux. **Les octets 8 à 15 sont identiques dans les
deux modes** : `00 00 80 00 32 00 00 01`. C'est acquis, pas déduit.

| Offset | Valeur | Interprétation |
|---|---|---|
| 2 | `0x01` | masque de canal ✅ |
| 3 | `0x00` | ❓ |
| 4 | `0x01` statique, `0x02` **rotation** | mode ✅ — §5.4 |
| 5–6 | `00 00` statique, `6a 00` animé | vitesse, uint16 little-endian ✅ |
| 7 | `0x08` | nombre de LED ✅ |
| 8–15 | `00 00 80 00 32 00 00 01` | ❓ — **valeurs certaines dans les deux modes**, sens inconnu |

L'offset 12 vaut `0x32`, soit 50, comme la vitesse par défaut du mode fixe en `2a 04`. 🔶
Coïncidence numérique, rien de plus : aucune observation ne relie les deux.

### 5.3 Animations personnalisées — `0x22 0x20`

✅ Une animation CAM personnalisée téléverse ses images clés :

```
22 20 01 <index 00..07> 04 39 00 ... <couleurs>
...
22 03 01 08                                <- validation, 8 images
```

Huit images numérotées de `00` à `07`. Le mécanisme dépasse le besoin immédiat, mais il montre
que le contrôleur sait stocker et rejouer une séquence sans l'hôte.

### 5.4 Confirmation à l'œil du pilotage LED par LED ✅

Session du **2026-07-30** sous Linux, via `tools/confirme_leds.sh`.

**Les LED sont bien adressées individuellement.** Huit couleurs distinctes envoyées sur un même
ventilateur donnent huit LED de couleurs différentes.

**La famille `0x22` fonctionne sur les deux modèles de contrôleur.** Le motif envoyé avec `--all`
a été pris par les dix ventilateurs, y compris celui de l'arrière et ceux du haut, qui dépendent
de contrôleurs `1e71:2012` et non du `2019`. La capture Windows ne montrait que le `2019` : c'est
donc une extension du domaine connu, obtenue sous Linux.

**Le mode `0x02` fait tourner le motif** ✅ — c'est la réponse à la question qui restait ouverte.
Le contrôleur décale le tampon d'un cran à intervalle régulier ; l'hôte n'envoie rien de plus. Une
rotation à l'infini coûte donc trois trames, une seule fois.

**Numérotation des LED.** Les LED forment un anneau fermé : la 8 est contiguë à la 1, un cran
avant elle dans le sens antihoraire. Les indices progressent donc dans un sens de rotation
constant, `1 → 8` puis retour à `1`.

> ⚠️ **La position absolue de la LED 1 dépend du montage.** Elle est apparue « en bas à gauche »
> sur le ventilateur testé, mais ce n'est pas une propriété du protocole : c'est l'orientation
> physique du ventilateur dans le boîtier. Un motif qui suppose « la LED 1 est en haut » sera faux
> sur un ventilateur monté autrement. Seul l'**ordre** est une donnée du protocole ; l'origine et
> le sens apparent sont une donnée de montage, et devront être configurables par ventilateur.

---

## 6. Ventilateurs — `0x62 0x01`

✅ Émise **chaque seconde** par CAM, vers le device 7 uniquement.

```
62 01 07 19 19 19
│  │  │  └──┴──┴── consigne PWM par canal : 0x19 = 25 %
│  │  └─────────── masque des canaux ventilateur : 0x07 = canaux 1,2,3
└──┴────────────── commande
```

✅ Recoupé par les rapports d'état : consigne `19 19 19` et régimes de 708 à 728 tr/min.

---

## 7. Rapports d'état — `0x67` (endpoint IN)

### 7.1 `0x67 0x02` — ventilateurs ✅

```
 0: 67 02 0a f0 03 13 29 95 ad aa be 94 04 61 03 ff
16: 02 02 02 00 00 00 00 00 d8 02 c4 02 d2 02 00 00
32: 00 00 00 00 00 00 00 00 19 19 19 00 00 00 00 00
48: 19 19 19 ...
```

| Offset | Contenu |
|---|---|
| 2–13 | constante (🔶 firmware / série) |
| 16–18 | un octet par canal ventilateur, type détecté |
| 24–29 | ✅ **régimes tr/min, uint16 LE** : `0x02d8`=728, `0x02c4`=708, `0x02d2`=722 |
| 40–42 | ✅ consigne PWM par canal |
| 48–50 | 🔶 consigne dupliquée ou valeur appliquée |

### 7.2 Accusés `0xff 0x01` ✅

Chaque commande est acquittée, l'identifiant acquitté étant réémis en fin de trame :

```
ff 01 <constante 12 octets> 62 01   <- accuse 0x62 0x01
ff 01 <constante 12 octets> 2a 04   <- accuse 0x2a 0x04
```

Le compte concorde exactement avec le nombre de commandes émises.

---

## 8. Initialisation ✅

Jouée par CAM à chaque démarrage.

**Device 7 (`1e71:2019`)**
```
10 02
20 03
60 03
60 02 01 e8 03 01 e8 03      <- 0x03e8 = 1000, deux fois
```

**Devices 8 et 9 (`1e71:2012`)**
```
10 01
20 03
```

Puis, ~6 s plus tard, les trames de couleur et la consigne ventilateur.
❓ Sémantique de `0x10`, `0x20`, `0x60`. L'argument de `0x10` dépend du modèle.

---

## 9. Persistance : aucune ✅

**La configuration ne survit pas au redémarrage.** Vérifié physiquement : après reboot les LED
restent éteintes jusqu'au lancement de CAM.

Conséquence : une implémentation Linux doit rejouer la séquence du §8 puis réappliquer les
couleurs à chaque démarrage. Un `set` unique au boot ne suffira pas.

> [!note] Nuance ajoutée le 2026-07-30 — l'état est volatile, pas absent
> Observation de Nico : après un **redémarrage à chaud**, les ventilateurs **gardent leur couleur**. Ce n'est qu'après une **coupure complète de l'alimentation** qu'ils reviennent au **blanc** (défaut firmware). Même comportement pour la RAM.
>
> Les deux constats se concilient : l'état vit en **mémoire volatile** du contrôleur. Il survit tant que l'alimentation de veille est maintenue, et disparaît dès qu'elle est coupée. Il n'y a bien **aucune mémoire non volatile** — la conclusion du §9 tient — mais la formulation « ne survit pas au redémarrage » est trop absolue.
>
> Conséquence pratique inchangée pour l'implémentation : le démon doit appliquer l'état au démarrage. Conséquence pour les **tests** en revanche : un redémarrage à chaud ne remet **pas** le matériel à zéro. Seule une coupure d'alimentation fournit une machine vierge — indispensable pour trancher si la séquence d'initialisation du §8 est réellement obligatoire (question ouverte du §0).

---

## 10. Questions ouvertes

| # | Question | Comment trancher |
|---|---|---|
| 1 | L'offset 3 de `22 10` permet-il de chaîner au-delà de 8 LED ? | brancher un accessoire plus long, ou tester `0x08` sous Linux |
| ~~2~~ | ~~`22 11` et `22 a0` sont-ils obligatoires ?~~ | ✅ **tranché — §0.2** |
| ~~3~~ | ~~Le `62 01` répété est-il un watchdog ?~~ | ✅ **tranché — §0.3** |
| ~~4~~ | ~~Rôle de l'offset 6 de `2a 04`~~ | ✅ **tranché — §4.4** : sélecteur de variante, pas direction |
| ~~5~~ | ~~Noms CAM des modes `0x01`, `0x04`, `0x05`, `0x06`, `0x09`~~ | ✅ **tranché — §4.5** : confirmés à l'œil le 2026-07-30 |
| 6 | Différence entre offsets 2 et 3 de `2a 04` | jamais observée différente |
| ~~7~~ | ~~Que porte l'offset 56 quand le mode n'attend aucune couleur ?~~ | ✅ **tranché — §4.4** : toujours ≥ 1, avec une couleur noire |
| 8 | Signification de l'offset 57 | ✅ **valeur connue pour les 8 modes (§4.4)**, sens encore ❓ |
| 9 | À quelle cadence correspond quelle valeur de vitesse ? | chronométrer une animation à deux vitesses connues |
| 10 | La séquence d'initialisation du §8 est-elle obligatoire ? | ⚠️ exige une **coupure d'alimentation** — un redémarrage à chaud ne réinitialise rien (§9) |
| 11 | Que fait le mode `0x03` ? | jamais déclenché pendant la capture ; le tester sous Linux |

Ces questions se règlent désormais mieux **sous Linux par expérimentation directe** — l'outil
`reverb` sait écrire les trames — que par une capture supplémentaire sous Windows.

> Les questions 4, 7 et 8 ont été tranchées **sans matériel**, en réanalysant la capture déjà
> prise avec `tools/extrait_modes.py`. Avant d'organiser une nouvelle session Windows, vérifier
> systématiquement si la réponse n'est pas déjà dans les `.pcap` conservés.

---

## 11. Implémentation de référence

```python
import hid

VID, PID = 0x1E71, 0x2019          # ou 0x2012 pour les deux autres controleurs

def _pkt(*head):
    """Paquet HID de 64 octets, complete par des zeros."""
    return bytes(head) + bytes(64 - len(head))

def init(dev, modele_2019=True):
    """Sequence d'initialisation, indispensable a chaque demarrage (§8)."""
    dev.write(_pkt(0x10, 0x02 if modele_2019 else 0x01))
    dev.write(_pkt(0x20, 0x03))
    if modele_2019:
        dev.write(_pkt(0x60, 0x03))
        dev.write(_pkt(0x60, 0x02, 0x01, 0xE8, 0x03, 0x01, 0xE8, 0x03))

def couleur_fixe(dev, canal, r, g, b, luminosite=1.0, nb_led=8):
    """Mode couleur fixe, toutes les LED du canal identiques (§4)."""
    r, g, b = (int(c * luminosite) for c in (r, g, b))   # luminosite cote hote (§4.3)
    buf = bytearray(64)
    buf[0:2] = b'\x2a\x04'
    buf[2] = buf[3] = 1 << (canal - 1)
    buf[4] = 0x00                       # mode fixe
    buf[5] = 0x32                       # vitesse, sans effet en mode fixe
    buf[7:10] = bytes((g, r, b))        # GRB
    buf[56] = 0x01                      # une seule couleur fournie
    buf[58] = nb_led
    buf[59] = 0x03
    dev.write(bytes(buf))

def leds_individuelles(dev, canal, couleurs, luminosite=1.0):
    """Pilotage LED par LED (§5). `couleurs` = liste de 8 tuples (r, g, b)."""
    assert len(couleurs) == 8, "8 LED par ventilateur"
    mask = 1 << (canal - 1)

    data = bytearray()
    for (r, g, b) in couleurs:
        r, g, b = (int(c * luminosite) for c in (r, g, b))
        data += bytes((g, r, b))        # GRB

    dev.write(_pkt(0x22, 0x10, mask, 0x00, *data))
    dev.write(_pkt(0x22, 0x11, mask))
    dev.write(_pkt(0x22, 0xA0, mask, 0x00, 0x01, 0x00, 0x00, 0x08,
                   0x00, 0x00, 0x80, 0x00, 0x32, 0x00, 0x00, 0x01))
```

⚠️ Les canaux ne sont **pas** interchangeables entre contrôleurs : le canal 1 du device 7 et le
canal 1 du device 9 sont deux ventilateurs différents. Voir la table du §3.
