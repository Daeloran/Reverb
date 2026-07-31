# CLAUDE.md — Reverb

> Instructions projet pour Claude Code. Lu automatiquement au démarrage de session.

## Imports

<!-- Importe le workflow global perso (issues, branches, PR, tests) -->

@~/.claude/CLAUDE.md

<!-- Documents projet utiles à charger -->

@README.md

> Ce fichier ne contient QUE les spécificités du projet. Le workflow général (création d'issue, branches, PR, tests d'intention vs logique) est défini dans `~/.claude/CLAUDE.md`.

---

## Contexte projet

- **Nature** : contrôle de l'éclairage RGB du poste SHYNAEL sous Linux. 10 ventilateurs NZXT, écran LCD du Kraken, 4 barrettes de RAM Corsair. Démon léger + fenêtre de pilotage.
- **Utilisateurs** : Nico, sur sa machine. Outil personnel.
- **Repo** : https://github.com/Daeloran/Reverb — **public**
- **Plateforme** : GitHub → CLI à utiliser : **`gh`**
- **Statut** : protocoles décodés ; couleur fixe livrée (#1), modes d'animation en cours (#3)
- **Branche par défaut** : `main`

> 📓 **LES TROIS SPÉCIFICATIONS DE PROTOCOLE FONT FOI**, et la copie de ce dépôt est la référence :
> - `docs/SPEC-PROTOCOLE-NZXT.md` — ventilateurs, modes, LED par LED, initialisation
> - `docs/SPEC-KRAKEN-LCD.md` — écran 640×640, luminosité, mode firmware
> - `docs/SPEC-CORSAIR-RAM.md` — SMBus, 11 LED par barrette, CRC-8
>
> **Les lire avant toute session touchant au matériel.** Chaque affirmation y porte un marqueur : ✅ confirmé par les données · 🔶 hypothèse non testée · ❓ inconnu. **Ne jamais implémenter depuis un ❓**, et ne jamais inventer une trame absente — dire qu'elle est inconnue.
>
> Le vault Obsidian (`~/Documents/Kirin Tor/02 - Projets/Reverb/`) en garde une copie de lecture, plus des notes d'environnement Linux (udev, pièges d'outillage). **En cas de divergence, `docs/` fait foi.**

> 🔍 **Avant d'organiser une nouvelle session de capture Windows, relire les `.pcap` déjà pris.**
> Trois questions ouvertes ont été tranchées sans toucher au matériel, en réanalysant la capture existante avec `tools/extrait_modes.py`. Les captures sont dans `captures/` — hors git, 178 Mo.

## Stack

| Composant | Choix |
|---|---|
| Langage | Rust (édition 2024) |
| Interface | **Slint** — licence libre de droits pour le bureau, runtime < 300 Kio |
| HID (ventilateurs) | **écriture directe sur `/dev/hidraw*`** — pas de `hidapi`, pas de dépendance C |
| SMBus (RAM) | `i2cdev` → `smbus_write_block_data` |
| USB bulk (écran) | `nusb` (Rust pur) — ⚠️ vérifier le support du paquet de longueur nulle |
| IPC démon ↔ interface | socket Unix |
| Distribution | binaire statique dans `~/.local/bin` + unité `systemd --user` |

**Aucune dépendance de runtime.** C'est l'exigence structurante : Bazzite est immuable, et un binaire unique ne casse pas à une montée d'image. Pas de Qt, pas de GTK, pas de WebKit, pas d'interpréteur.

> ⚠️ Ne pas introduire de dépendance qui exige une bibliothèque système sans ADR. C'est ce critère qui a écarté Tauri (WebKit2GTK) et `hidapi` (libudev).

## Matériel cible

| Périphérique | Transport | Ordre couleur | Canaux |
|---|---|---|---|
| `1e71:2019` | HID, `/dev/hidraw*` | **GRB** | 6 ARGB + 3 PWM |
| `1e71:2012` (série `0E01…`) | HID, `/dev/hidraw*` | **GRB** | 3 ARGB, **1 utilisé** |
| `1e71:2012` (série `1101…`) | HID, `/dev/hidraw*` | **GRB** | 3 ARGB, **3 utilisés** |
| `1e71:300c` | HID + USB bulk | **BGR** | écran 640×640 |
| RAM Corsair | `/dev/i2c-8`, `0x18`–`0x1b` | **RGB** | 11 LED par barrette |

> ⚠️ **TROIS ORDRES DE COMPOSANTES DIFFÉRENTS.** Ventilateurs en GRB, écran en BGR, RAM en RGB.
> C'est la première source d'erreur du projet, et une erreur ici ne produit **aucun message** — juste une mauvaise couleur. La conversion doit vivre dans un seul endroit par périphérique, avec des tests qui la couvrent.

> ⚠️ **Les numéros de `hidraw` changent au redémarrage — constaté, pas supposé.** Entre deux démarrages du 2026-07-29 et du 2026-07-30, le Kraken est passé de `hidraw11` à `hidraw10` et le contrôleur `0x2019` de `hidraw10` à `hidraw11`.
>
> Résoudre le périphérique par **VID:PID + numéro de série**, en parcourant `/sys/class/hidraw/*/device/uevent`. Jamais de chemin codé en dur.

Les deux `1e71:2012` sont physiquement identiques et ne se distinguent **que** par leur série :

| Série | Ventilateurs | Positions |
|---|---|---|
| `0E014044AB7664C25F063BD5` | 1 | arrière |
| `1101F021AA358489609AA5B2` | 3 | haut gauche / haut milieu / haut droite |

Le Kraken expose **deux interfaces** : `MI_01` en HID (commandes) et `MI_00` en bulk (image). Sous Linux, `libusb`/`nusb` accède aux deux, mais l'accès brut exige la règle udev.

## Cartographie physique des canaux

Établie par **calibration directe sur le matériel** le 2026-07-30.

| Périphérique | Masque | Position |
|---|---|---|
| `2019` | `0x01` | bas gauche |
| `2019` | `0x02` | bas milieu |
| `2019` | `0x04` | bas droite |
| `2019` | `0x08` | radiateur haut |
| `2019` | `0x10` | radiateur milieu |
| `2019` | `0x20` | radiateur bas |
| `2012` (0E01…) | `0x01` | arrière |
| `2012` (1101…) | `0x01` | haut gauche |
| `2012` (1101…) | `0x02` | haut milieu |
| `2012` (1101…) | `0x04` | haut droite |

Disposition du boîtier : **3 en bas**, **3 sur l'avant** — le radiateur du Kraken, plaqué contre la face de la carte mère —, **3 sur le dessus**, **1 à l'arrière**.

> ⚠️ **La table issue de la session Windows était fausse sur deux groupes sur quatre**, les libellés y ayant été posés de mémoire. Correction documentée dans `docs/SPEC-PROTOCOLE-NZXT.md` §3.
>
> **Règle qui en découle** : l'appartenance d'un canal à un contrôleur vient des captures et se croit ; l'**étiquette physique** vient d'une observation humaine et se **vérifie canal par canal**. Ne jamais ajouter ni renommer une position sans rallumer le canal concerné.

## Structure du projet

Espace de travail Cargo. Le découpage suit une règle : **ce qui est testable sans matériel est séparé de ce qui touche au matériel.**

```
crates/
  reverb-proto/    # encodage des trames, conversions de couleur, CRC-8.
                   #   PUR : aucune IO. C'est ici que vivent les tests.
  reverb-daemon/   # binaire. Ouvre les periphériques, applique l'état,
                   #   fait tourner la boucle RAM, expose un socket Unix.
  reverb-gui/      # binaire. Fenêtre Slint, cliente du démon.
tests/
  spec/            # tests d'INTENTION — écrits depuis l'issue, sans accès au code
  unit/            # tests de LOGIQUE — écrits avec le code
tools/             # diagnostic. Scripts Python et PowerShell hérités de l'exploration.
captures/          # captures USB/SMBus de la rétro-ingénierie (hors git)
docs/decisions/    # ADR
```

## Coût d'exécution attendu

À connaître, parce que ça dicte l'architecture du démon :

| Cible | Qui anime | Coût du démon |
|---|---|---|
| Ventilateurs NZXT | **le firmware** | écrit une fois, puis dort |
| Écran — température liquide | **le firmware** | rien du tout |
| Écran — image personnalisée | l'hôte | 1,2 Mo toutes les ~25 s |
| **RAM Corsair** | **l'hôte, obligatoirement** | boucle SMBus permanente |

La RAM est la **seule contrainte temps réel du projet**. Tout le reste est en écriture unique. Le démon doit donc être au repos absolu quand aucune animation de RAM n'est active — pas de boucle de rafraîchissement inutile, pas de sondage périodique.

## Commandes locales

| Action | Commande |
|---|---|
| Compiler | `cargo build --release` |
| Tests d'intention seuls | `cargo test --test spec` |
| Tests de logique seuls | `cargo test --lib` |
| Lint | `cargo clippy -- -D warnings` |
| Format | `cargo fmt --check` |
| Sonder le matériel (hérité) | `uv run --with liquidctl tools/probe_nzxt.py` |

## Conventions spécifiques au projet

- **Aucune écriture matérielle dans les tests automatisés.** `reverb-proto` se teste en comparant des trames à des octets attendus, issus des specs. Un test qui allume une LED est un test d'intégration lancé à la main.
- **Les trames de référence des tests viennent des specs**, recopiées telles quelles en commentaire avec leur section source. Un test qui ne cite pas sa source n'est pas vérifiable.
- **Nommage physique.** L'utilisateur ne voit jamais « canal 4 » ni un masque de bits. Il voit « droit bas ».
- **Le démon détient le matériel, seul.** L'interface ne parle jamais directement à un périphérique — sinon deux processus écrivent sur le même bus.
- **La luminosité se calcule côté hôte.** Le protocole NZXT n'a aucun octet de luminosité (spec §4.3) : c'est une multiplication des composantes avant envoi.

## Pièges connus / gotchas

- **Aucune persistance matérielle.** Rien ne survit au redémarrage : il faut rejouer la séquence d'initialisation (spec NZXT §8) puis réappliquer les couleurs à chaque démarrage.
- **Pas de watchdog.** Une fois écrit, l'état tient indéfiniment sans hôte (spec §0.3). Ne pas construire de boucle de rafraîchissement « au cas où ».
- **Le premier octet de la trame HID est l'identifiant de rapport.** `OutputReportByteLength` vaut 64, pas 65 : ne **pas** préfixer un `0x00`. Écrire les 64 octets tels quels.
- **Les trois trames `0x22` sont indissociables** : `22 10` remplit un tampon, `22 11` valide le canal, `22 a0` applique. Envoyer la première seule ne fait rien (spec §0.2).
- **Image du Kraken : paquet de longueur nulle obligatoire.** 1 228 800 = 2400 × 512, un multiple exact de `wMaxPacketSize` → sans ZLP le contrôleur concatène les images et l'affichage dérive. C'est le défaut connu de l'implémentation de référence.
- **Repli de l'écran après ~30 s** : sans nouvel envoi, le firmware reprend la main. Une image permanente impose de réémettre toutes les ~25 s.
- **Luminosité de l'écran avant l'image, jamais après** : la commande `30 02` réinitialise le pipeline d'affichage.
- **Le Kraken n'a pas de règle udev** → l'accès USB bulk est bloqué tant qu'elle n'est pas ajoutée. Symptôme sous liquidctl : `ValueError: The device has no langid`.
- **`spd5118` ne gêne pas** l'accès aux `0x18`–`0x1b` : il ne réserve que les `0x50`–`0x53`. Inutile de le décharger.
- **Concurrence sur le SMBus** : ne jamais laisser OpenRGB tourner en même temps que le démon. Un accès concurrent peut corrompre une transaction.

## Out of scope — NEVER

- ❌ **Ne jamais écrire sur un bus SMBus à une adresse non documentée dans la spec.** Risque de corruption SPD et de DIMM qui ne bootent plus.
- ❌ Ne jamais scanner un bus I2C « pour voir ». Un simple scan en lecture a déjà modifié l'éclairage par défaut de la RAM.
- ❌ Ne jamais inventer une trame absente des specs. Si c'est inconnu, le dire.
- ❌ Ne jamais introduire une dépendance exigeant une bibliothèque système sans ADR.
- ❌ Ne jamais coder en dur un chemin `/dev/hidraw*` — ils changent au redémarrage.
- ❌ Ne jamais commiter la configuration personnelle (groupes, ambiances) — fournir un exemple.
- ❌ Ne jamais désactiver un test pour le faire passer.

## Décisions d'architecture clés

> ADR détaillés dans `docs/decisions/`.

- **ADR-001** : Rust + Slint, zéro dépendance de runtime. Motif : OS immuable, exigence d'empreinte minimale au repos, et refus d'un empaquetage Flatpak dont le sandbox gênerait l'accès à `/dev/hidraw*`, `/dev/i2c-8` et l'USB brut.
- **ADR-002** : démon et interface séparés, IPC par socket Unix. Motif : l'éclairage doit vivre sans fenêtre ouverte, et un seul processus doit détenir les bus.
  - **Réalisé le 2026-07-31 (issue #17)**, et le motif s'est doublé d'un second, mesuré : ouvrir un `/dev/hidraw*` coûte **51 ms**, y écrire **~1 ms**. Un processus qui garde ses descripteurs passe de 1,5 à 31 images/s. Le démon n'est donc pas seulement une commodité d'architecture, c'est la condition de la fluidité.
  - Protocole **texte, une ligne par requête**, encodé dans `reverb-proto/src/ipc.rs` — pur et testable sans démarrer de démon. Pas de JSON : `serde` serait la plus grosse dépendance du projet pour quatre verbes.
  - Le socket est en `0660 root:reverb`, obtenu par `User=root` + `Group=reverb` + `UMask=0007` dans l'unité systemd — **sans `chown` après coup**, donc sans fenêtre pendant laquelle il serait ouvert à tous. C'est une entorse assumée au principe retenu pour udev (`uaccess` plutôt qu'un groupe) : ce socket n'expose que l'éclairage et les ventilateurs, là où un `GROUP=` sur `/dev/hidraw*` exposait la lecture brute de tous les périphériques HID. **À réviser si** Reverb tourne un jour sur une machine multi-utilisateurs.
  - **L'écran du Kraken reste hors du démon**, pour que `reverb screen --image` continue de marcher : une image de 1,2 Mo n'est pas exposable sur un protocole texte, et la prendre sans l'exposer serait une régression de capacité pour rien.
- **ADR-003** : accès HID par écriture directe sur `/dev/hidraw*` plutôt que `hidapi`. Motif : un rapport de sortie est un simple `write()`, et `hidapi` imposerait libudev en dépendance C.
- **ADR-004** : transferts bulk vers l'écran par les `ioctl` d'usbfs plutôt que `rusb`/`libusb`, et `unsafe_code` abaissé de `forbid` à `deny` pour le permettre. Motif : même raisonnement que l'ADR-003 étendu à l'USB brut — `rusb` traînerait libusb en dépendance C, or l'ADR-001 pose le zéro dépendance de runtime. **Contrepartie assumée** : la bibliothèque standard n'expose aucun `ioctl`, donc la dérogation est déclarée en tête des modules concernés. Partout ailleurs un `unsafe` reste une erreur de compilation.
  - **Révisé le 2026-07-31 (issue #15)**, la condition posée s'étant réalisée : `crates/reverb-cli/src/i2c.rs` est un second module dérogataire. Décision : **on garde**. La condition a bien déclenché, mais le remède qu'elle nommait ne s'applique pas — `i2c.rs` parle SMBus, pas USB, et `rusb` ne lui apporterait rien. Le seul substitut bon marché serait `libc`, dont l'`ioctl` est **lui aussi `unsafe`** : la dépendance s'ajouterait sans retirer un seul bloc. Il faudrait un crate de haut niveau (`i2cdev`) pour supprimer l'`unsafe`, soit exactement le coût que l'ADR-001 refuse.
  - Surface totale : **quatre appels, tous à `ioctl`, tous sur un descripteur ouvert par la bibliothèque standard** — trois dans `usbfs.rs`, un dans `i2c.rs`.
  - **Nouvelle condition de révision**, plus utile que le simple comptage de modules : un `unsafe` qui ferait autre chose qu'appeler `ioctl` sur un descripteur détenu par `std`. C'est la nature du bloc qui mesure la dette, pas le nombre de fichiers.
