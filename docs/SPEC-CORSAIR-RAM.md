# Spécification — RGB de la RAM Corsair DDR5 (Dominator Titanium)


> Rétro-ingénierie d'iCUE 5.48.58 sous Windows 11, le 2026-07-30.
> ✅ confirmé par les données · 🔶 hypothèse · ❓ inconnu

Matériel : 4 × `CMP32GX5M2B6000C30`, DDR5-6000, firmware 1.0.18.
iCUE les identifie comme **DOMINATOR TITANIUM**, `vid=1b1c pid=0x40b`.
**11 LED par barrette.**

---

## 0. Synthèse pratique

| Besoin | Coût | Statut |
|---|---|---|
| Couleur fixe, une barrette entière | une paire de blocs SMBus | ✅ |
| **Couleur par LED** — 11 LED indépendantes | même coût, on remplit les 33 octets | ✅ |
| **Animation** | **boucle SMBus permanente** — l'hôte recalcule chaque image | ✅ constaté |
| Animation autonome (`onDevice`) | supprimerait la boucle | ❌ **testé, ne fonctionne pas** |

Le contrôleur **n'a pas de watchdog** : un état écrit tient indéfiniment sans hôte. Mais il ne
sait pas animer seul — dès qu'iCUE est tué en pleine animation, l'affichage se fige sur la
dernière image reçue.

**C'est la seule contrainte temps réel de tout le projet** : les ventilateurs NZXT animent seuls,
l'écran du Kraken affiche la température seul, la RAM non.

---

## 1. Le résultat structurant

✅ **Corsair n'utilise aucun protocole propriétaire pour atteindre la RAM.**

`CorsairLLAccess64.sys` n'est qu'un pilote d'**entrées/sorties sur ports bruts**. Tout le
protocole SMBus est implémenté **en espace utilisateur**, dans
`CorsairDeviceControlService.exe`, qui pilote directement le contrôleur **SMBus AMD FCH**.

Conséquence pour Linux : c'est exactement le contrôleur que gère **`i2c-piix4`**. Rien à
réimplémenter côté bus — les barrettes sont joignables en `i2c-dev`, avec des transferts
SMBus par bloc standard.

---

## 2. Interface du pilote Corsair ✅

Deux IOCTL, `METHOD_BUFFERED`, `FILE_DEVICE_UNKNOWN` (`0x22`) :

| Code | Sens |
|---|---|
| `0x225358` | **lecture** d'un port d'E/S |
| `0x229354` | **écriture** d'un port d'E/S |

Tampon d'entrée, **10 octets** : `[uint32 port][uint32 valeur][uint16 taille=1]`.

Périphérique : `\Device\CorsairLLAccess<hash>`, ouvert par `CorsairDeviceControlService.exe`
(LocalSystem). Bibliothèque appelante : `CorsairLLAccessLib64.dll`, export principal
`CrGetLLAccessInterface`.

*Utile seulement pour comprendre la capture — sans objet sous Linux.*

---

## 3. Contrôleur SMBus et carte des adresses ✅

Base d'E/S : **`0x0B00`**, disposition compatible PIIX4.

| Port | Registre | Rôle |
|---|---|---|
| `0x0B00` | `SMBHSTSTS` | état, interrogé en boucle |
| `0x0B02` | `SMBHSTCNT` | contrôle : `0x54` = START + protocole **bloc** |
| `0x0B03` | `SMBHSTCMD` | registre visé |
| `0x0B04` | `SMBHSTADD` | adresse 7 bits décalée, bit 0 = lecture |
| `0x0B05` | `SMBHSTDAT0` | **nombre d'octets** du bloc |
| `0x0B07` | `SMBBLKDAT` | données du bloc, un octet par accès |

| Adresses 7 bits | Rôle |
|---|---|
| `0x50`–`0x53` | hubs SPD |
| `0x48`–`0x4b` | PMIC (tensions) |
| **`0x18`–`0x1b`** | **contrôleurs RGB — un par slot, dans l'ordre** |

Recoupe le journal d'iCUE : `DIMM 0x1800`, `0x1901`, `0x1a02`, `0x1b03`
(octet haut = adresse SMBus, octet bas = index de slot).

---

## 4. Protocole des couleurs ✅

### 4.1 Charge utile logique — 35 octets

```
[0]      nombre de LED = 0x0b (11)
[1..33]  11 triplets RGB consecutifs, 3 octets chacun
[34]     CRC-8
```

✅ **Ordre des composantes : RGB.** Simple et direct.

Établi par texte clair connu — quatre barrettes réglées sur quatre couleurs franches, état
final de la capture :

| Adresse | Slot | Octets transmis | Couleur demandée |
|---|---|---|---|
| `0x18` | 0 | `ff 13 00` | rouge |
| `0x19` | 1 | `00 ff 25` | vert |
| `0x1a` | 2 | `08 00 ff` | bleu |
| `0x1b` | 3 | `ff ff ff` | blanc |

Les écarts (`0x13`, `0x25`, `0x08`) sont la correction colorimétrique appliquée par iCUE, pas
une propriété du protocole.

### 4.1.1 Les 11 LED sont adressables individuellement ✅

Confirmé par une capture d'effet animé : les 11 triplets d'un même bloc diffèrent entre eux.
Exemple relevé sur une seule barrette, une vague qui progresse :

```
c36aff  a774ff  8b7fff  6f89ff  2a4a81  000000 ...
```

Les 11 triplets sont donc **indépendants**, comme les 8 LED d'un ventilateur NZXT. Rien à faire
de particulier : il suffit de remplir les 33 octets avec des valeurs différentes.

⚠️ **Attention aux trois ordres différents dans ce projet** : ventilateurs NZXT en **GRB**,
écran Kraken en **BGR**, RAM Corsair en **RGB**. À isoler proprement dans le code.

### 4.2 CRC-8 ✅

Polynôme **`0x07`**, valeur initiale **`0x00`**, sans réflexion ni XOR final — le CRC-8/ATM
classique, calculé sur les **34 premiers octets** de la charge utile.

Vérifié sur **40 blocs, 40 concordances exactes**.

### 4.3 Découpage en deux transferts ✅

Un bloc SMBus est limité à 32 octets ; les 35 octets sont donc scindés :

| Registre | Octets | Contenu |
|---|---|---|
| `0x31` | 32 | charge utile `[0..31]` |
| `0x32` | 3 | charge utile `[32..34]`, dont le CRC |

Les deux transferts se suivent immédiatement, vers la même adresse.

### 4.4 Séquence complète observée

```
W  0x0B04 = 0x30      adresse 0x18 decalee, ecriture
W  0x0B03 = 0x31      registre
W  0x0B05 = 0x20      32 octets
W  0x0B07 x32         charge utile [0..31]
W  0x0B02 = 0x54      START + bloc

W  0x0B04 = 0x30
W  0x0B03 = 0x32
W  0x0B05 = 0x03      3 octets
W  0x0B07 x3          charge utile [32..34]
W  0x0B02 = 0x54
```

🔶 iCUE réémet cette paire en continu, plusieurs fois par seconde, tant qu'un effet est actif
(mode « direct lighting »). En mode « onDevice », le contrôleur exécute l'effet seul et le bus
reste muet.

### 4.5 Aucun watchdog, mais les animations viennent de l'hôte ✅

**Test 1 — état statique.** iCUE arrêté, quatre barrettes en rouge / vert / bleu / blanc, puis
observation sans aucun trafic sur le bus. **Les couleurs tiennent.** Le contrôleur conserve
l'état écrit sans intervention de l'hôte.

**Test 2 — effet animé.** Un effet animé lancé dans iCUE, puis iCUE tué en pleine animation.
**Les barrettes se figent sur une couleur fixe.** L'animation s'arrête net.

⚠️ **Conclusion structurante : l'animation de la RAM est calculée par l'hôte.** iCUE recompose
les 35 octets et les réécrit plusieurs fois par seconde ; le contrôleur ne fait qu'afficher le
dernier état reçu.

Pour un démon Linux :

| Besoin | Coût |
|---|---|
| couleur fixe, même par LED | une écriture, puis plus rien |
| **animation** | **boucle SMBus permanente** |

C'est la **seule contrainte temps réel du projet** : les ventilateurs NZXT animent seuls, l'écran
du Kraken sait afficher la température seul, la RAM non.

**Test 3 — mode `onDevice`. ❌ Ne fonctionne pas.**

Le journal d'iCUE mentionne un `onDevice lighting mode`. Il a été testé explicitement :
l'éclairage matériel a été sélectionné dans iCUE, la capture lancée, puis iCUE tué en pleine
animation.

Résultats, tous négatifs :

- la capture ne contient **que du streaming** vers `0x31`/`0x32` — aucune commande de bascule,
  aucun registre inhabituel touché ;
- à la mort d'iCUE, **les barrettes se figent sur une couleur fixe**, l'animation s'arrête net.

**Conclusion : il n'existe pas de moyen connu de faire animer la RAM sans l'hôte.** La boucle
SMBus permanente est obligatoire pour toute animation.

🔶 Réserve unique, à ne pas confondre avec un espoir : on ne peut pas distinguer « ce mode
n'existe pas » de « la sélection dans iCUE ne l'a pas réellement armé ». Mais en l'état, aucune
séquence observée ne permet de l'activer — **ne pas bâtir dessus**.

---

## 5. Autres registres observés

| Registre | Sens | Constat |
|---|---|---|
| `0x24` | lecture | renvoie `0x00` ou `0x02` — 🔶 état ou mode |
| `0x61`, `0x21` | écriture `0x00` | 🔶 sélection de page / remise à zéro du pointeur |
| `0x40` | lecture ×32 | lecture d'un bloc de 32 octets, octet par octet |
| `0x42` | lecture | ❓ |

🔶 La séquence `R 0x24` → `W 0x61=0` → `W 0x21=0` → `R 0x40 ×32` ressemble à une lecture de
configuration par fenêtre indirecte. Non nécessaire pour écrire les couleurs.

---

## 6. Implémentation Linux

```python
import smbus2

# L'adaptateur PIIX4 : sur AMD, i2c-piix4 en enregistre PLUSIEURS.
# Les barrettes ne repondent que sur l'un d'eux -> sonder 0x18..0x1b sur chacun.
BUS = 0
ADRESSES = {0: 0x18, 1: 0x19, 2: 0x1a, 3: 0x1b}   # slot -> adresse SMBus
NB_LED = 11

def crc8(data):
    """CRC-8/ATM : polynome 0x07, init 0x00."""
    c = 0
    for b in data:
        c ^= b
        for _ in range(8):
            c = ((c << 1) ^ 0x07) & 0xFF if c & 0x80 else (c << 1) & 0xFF
    return c

def couleurs(bus, slot, leds):
    """`leds` : liste de 11 tuples (r, g, b)."""
    assert len(leds) == NB_LED
    addr = ADRESSES[slot]

    payload = bytearray([NB_LED])
    for (r, g, b) in leds:
        payload += bytes((r, g, b))      # RGB, sans permutation
    payload.append(crc8(payload))        # 35 octets au total

    bus.write_block_data(addr, 0x31, list(payload[0:32]))
    bus.write_block_data(addr, 0x32, list(payload[32:35]))
```

⚠️ **Concurrence sur le bus.** iCUE et NZXT CAM détiennent tous deux le mutex
`Access_SMBUS.HTP.Method` sous Windows. Sous Linux, s'assurer qu'aucun autre logiciel
(OpenRGB, capteurs, `spd5118`) n'accède au bus en même temps — un accès SMBus concurrent
peut corrompre une transaction.

---

## 7. Questions ouvertes

| # | Question |
|---|---|
| ~~1~~ | ~~Le mode « onDevice » permet-il d'éviter la boucle ?~~ ❌ **tranché : non, testé et négatif — voir §4.5** |
| 2 | Rôle exact des registres `0x24`, `0x40`, `0x42`, et du couple `0x61`/`0x21` |
| 3 | Faut-il une séquence d'initialisation avant la première écriture, ou `0x31`/`0x32` suffisent-ils ? |
| 4 | Le CRC est-il vérifié par le contrôleur, ou toléré s'il est faux ? |

Les questions 3 et 4 se tranchent en quelques minutes sous Linux, par essai direct.

---

## 8. Outillage produit

| Script | Rôle |
|---|---|
| `Scan-ApmStrings.ps1` | recense les chaînes, identifie les fonctions tracées |
| `Dump-ApmRecords.ps1` | décode la structure des enregistrements API Monitor |
| `Analyse-ApmSmbus.ps1` | extrait et classe les tampons d'IOCTL |
| `Reconstruire-Smbus.ps1` | reconstitue les transactions octet, export CSV brut |
| `Reconstruire-Blocs.ps1` | **reconstitue les transferts par bloc et extrait les couleurs** |

Le format `.apmx64` est une **archive ZIP** précédée d'un en-tête texte et de la marque `RBAPM` —
elle s'ouvre telle quelle avec un lecteur ZIP. Entrées utiles : `process/N/calls` (index
d'offsets sur 8 octets) et `process/N/data` (enregistrements ; le code IOCTL sert d'ancrage,
le tampon suit 40 octets plus loin).

⚠️ Piège rencontré : en protocole bloc, `SMBHSTDAT0` porte le **nombre d'octets**, pas une
donnée. Une reconstitution qui l'interprète comme une écriture d'octet conclut à tort à un
« battement périodique » alors qu'il s'agit de l'envoi des couleurs.
