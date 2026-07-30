# Spécification — écran LCD du NZXT Kraken Elite (`1e71:300c`)


> Rétro-ingénierie de NZXT CAM 4.76.5, le 2026-07-30.
> ✅ confirmé · 🔶 hypothèse · ❓ inconnu
>
> Capture : `cible2-kraken-init-20260730-105545.pcap` (démarrage de CAM, 80 s, device 6 seul).

---

## 0. Synthèse pratique

| Besoin | Coût | Statut |
|---|---|---|
| Afficher la **température du liquide** | aucun trafic, mode firmware | ✅ |
| Régler la **luminosité** | une commande HID | ✅ |
| Afficher une **image** | 1,2 Mo, à réémettre toutes les ~25 s | ✅ envoi reproduit, ⚠️ dérive à corriger |
| Afficher une **animation** | 1,2 Mo par image, ~2 images/s maximum | 🔶 |

**Recette minimale pour afficher une image :**

```
1. HID   38 01 02                     forcer le mode d affichage   <-- INDISPENSABLE
2. HID   30 02 01 <lum> 00 00 00 00 1e   luminosite, AVANT l image
3. HID   36 01 00 01 09               annonce
4. BULK  en-tete de 20 octets
5. BULK  1 228 800 octets  (640x640)  + PAQUET DE LONGUEUR NULLE
6. HID   36 02                        validation
7. repeter les etapes 3 a 6 toutes les ~25 s
```

⚠️ Les deux pièges qui coûtent le plus cher, détaillés plus bas : **sans l'étape 1 l'image est
ignorée en silence**, et **sans le paquet de longueur nulle de l'étape 5 l'image dérive**.

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

Attention donc : les LED des ventilateurs sont en **GRB** et l'écran en **BGR**. Deux ordres
différents dans le même écosystème NZXT — c'est une source d'erreur à isoler proprement dans le code.

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

⚠️ **Défaut connu de l'implémentation de référence : l'image dérive.** À l'envoi répété, le
contenu se décale progressivement à l'écran, comme une grille qui défile.

🔶 Cause quasi certaine : **absence de paquet de longueur nulle** en fin de transfert.
`1 228 800 = 2400 × 512`, un multiple exact de `wMaxPacketSize`. Dans ce cas la spécification USB
exige un **ZLP** (*zero-length packet*) pour signaler la fin du transfert ; sans lui, le
contrôleur ne sait pas où une image se termine et concatène la suivante — ce qui produit
exactement ce décalage.

Sous Linux avec `libusb`, cela correspond au drapeau `LIBUSB_TRANSFER_ADD_ZERO_PACKET`, ou à
l'envoi explicite d'un transfert de 0 octet après l'image. **À traiter en premier** si l'image
n'est pas stable.

⚠️ C'est aussi pourquoi **l'ordre des composantes n'a pas pu être vérifié directement** : la mire
défilant, les quadrants n'étaient jamais au même endroit. L'ordre **BGR** reste établi par le
raisonnement du §2.1 (jauge olive), pas par une mire. À reconfirmer une fois la dérive corrigée.

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
animation, mise en page personnalisée). Pour l'usage courant, il suffit de sélectionner le mode
intégré — vraisemblablement via `38 01 02` (🔶 correspondance non isolée formellement).

C'est le renversement de perspective de cette cible : le coûteux est optionnel.

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
38 01 02                     <- mode d affichage 2
32 02                        <- puis 32 02 01 .. 32 02 0f
```

✅ `32 02 <n>` est émis pour `n` de `0x01` à `0x0f` : **quinze emplacements** sont énumérés
ou effacés au démarrage. 🔶 Il s'agit vraisemblablement des « buckets » de stockage d'images
du Kraken, que `liquidctl` manipule déjà sous ce nom.

### 3.2 Boucle de rafraîchissement, chaque seconde

```
36 01 00 01 09      <- annonce l envoi d une image
36 02               <- validation
   (puis les deux transferts bulk du §2)
```

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

❓ L'octet `0x1e` (30) en fin de trame reste inexpliqué — inchangé dans toutes les observations.

**Effet de bord constaté** : un changement de luminosité provoque un **bref retour à l'affichage
intégré** (« NZXT — 39° Liquid ») avant que l'image téléversée ne revienne. La commande semble
donc réinitialiser le pipeline d'affichage. À prévoir si l'on enchaîne luminosité et image :
régler la luminosité **avant** d'envoyer l'image, pas après.

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
| 1 | **Corriger la dérive de l'image** — voir §2.2.1 | ajouter le paquet de longueur nulle en fin de transfert. **À faire en premier** |
| 2 | **RGB ou BGR** confirmé par une mire | une fois la dérive corrigée, afficher les quadrants et lire les positions |
| 3 | Rôle des octets `12 fa 01 e8` et `09 00 00 00` de l'en-tête | envoyer une image de taille différente |
| 4 | Orientation | faire varier les octets restants de `30 02` |
| 5 | Peut-on désactiver le repli de 30 s ? | non identifié — voir §2.2.2 |
| ~~—~~ | ~~Luminosité~~ | ✅ **tranché — §3.4** |
| ~~—~~ | ~~Mode d'affichage autonome~~ | ✅ **tranché — §2.3** |

⚠️ **Limite de capture** : USBPcap tronque à 65 535 octets. Les images ne sont donc capturées
qu'à 5 % (65 508 octets sur 1 228 800). Suffisant pour valider en-tête, cadence et géométrie ;
insuffisant pour reconstruire une image entière.
