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
| Protocole RAM Corsair | ✅ décodé, adresses `0x18`–`0x1b` |
| Cartographie physique des 10 canaux | ✅ établie |
| Implémentation Rust | ⏳ à faire |

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
l'initialisation et réapplique les couleurs au démarrage.

## Architecture

Aucune dépendance de runtime — l'OS est immuable, un binaire unique ne casse pas à une
montée d'image.

```
reverb-gui  (fenêtre Slint)
   │  socket Unix
   ▼
reverb-daemon
   ├── write()  ──►  /dev/hidraw*        ventilateurs, GRB
   ├── nusb     ──►  1e71:300c bulk      écran Kraken, BGR
   └── i2cdev   ──►  /dev/i2c-8 0x18..1b RAM Corsair, RGB
        │
   reverb-proto  (encodage des trames, conversions, CRC-8 — pur, testable)
```

⚠️ **Trois ordres de composantes différents** : ventilateurs en **GRB**, écran en **BGR**,
RAM en **RGB**. Une erreur ici ne produit aucun message, juste une mauvaise couleur.

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
