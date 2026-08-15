# Contrôle de la vitesse des ventilateurs — ce qui est atteignable

> Relevé du 2026-07-30 sur SHYNAEL (MSI X670E GAMING PLUS WIFI, Kraken Elite 2023,
> NZXT RGB & Fan Controller). Complète `SPEC-PROTOCOLE-NZXT.md`, qui ne traite que
> l'éclairage.

## Résumé

Cinq canaux sont pilotables sous Linux, tous par **hwmon**, aucun par le protocole HID.

| Source | Canal | Libellé | Régime au repos | Mode |
|---|---|---|---|---|
| `nzxtsmart2` | `pwm1` | FAN 1 | ~725 tr/min | manuel |
| `nzxtsmart2` | `pwm2` | FAN 2 | ~688 tr/min | manuel |
| `nzxtsmart2` | `pwm3` | FAN 3 | ~715 tr/min | manuel |
| `kraken2023elite` | `pwm1` | Pump speed | ~2380 tr/min | courbe firmware |
| `kraken2023elite` | `pwm2` | Fan speed | ~714 tr/min | courbe firmware |

## Vérification sur le matériel ✅

Session du 2026-07-30. `reverb fan --channel nzxtsmart2:fan-1 --pwm 80`, régimes relevés par
`reverb fans` avant et après :

| Canal | Avant | Après |
|---|---|---|
| `nzxtsmart2:fan-1` | 728 tr/min · 25 % | **1570 tr/min · 80 %** |
| `nzxtsmart2:fan-2` | 682 tr/min · 25 % | 665 tr/min · 25 % |
| `nzxtsmart2:fan-3` | 717 tr/min · 25 % | 706 tr/min · 25 % |

La consigne est appliquée, et **les canaux voisins ne bougent pas**. Le confinement de l'écriture,
garanti par construction — `set_pwm` ne reçoit qu'un `&FanChannel` — est donc vérifié aussi sur le
matériel, et pas seulement contre une fausse arborescence.

Au passage, deux points de calibrage : 25 % donne ~700 tr/min et 80 % ~1570 tr/min sur ces
ventilateurs. Trop peu pour établir une courbe, assez pour confirmer que l'échelle du noyau est
bien prise dans le bon sens.

## Pourquoi pas `0x62 0x01`

La spec §6 documente une commande HID de consigne PWM, émise chaque seconde par CAM. Elle est
écartée :

- elle n'adresse que les **3 canaux** du `1e71:2019`. Les contrôleurs `1e71:2012`, qui portent
  quatre des dix canaux d'éclairage, n'ont aucun canal ventilateur ;
- le pilote noyau `nzxt_smart2` fait déjà ce travail et expose les mêmes réglages en sysfs.
  Écrire la trame en parallèle créerait **deux écrivains sur le même registre**, chacun
  réémettant sa consigne périodiquement.

La rétro-ingénierie garde sa valeur documentaire, mais la réimplémenter serait un doublon fragile.

## Les prises de la carte mère sont vides ❌

Résultat **négatif et acquis** — ne pas rouvrir la question sans élément nouveau.

`modprobe nct6683 force=1` charge correctement et détecte le Nuvoton de la carte, exposé sous le
nom `nct6687` avec **8 sorties PWM et 10 entrées tachymétriques**. Sur ces dix :

```
fan2_input  = 2352 tr/min
tous les autres = 0
```

Le seul tachymètre actif remonte ~2352 tr/min, quand le pilote `kraken2023elite` annonce
~2380 tr/min pour la pompe. **C'est le même appareil** : le tachymètre de la pompe est câblé sur
`CPU_FAN`, pratique standard sur les AIO pour que le BIOS ne signale pas d'absence de ventilateur
processeur au démarrage. Les relevés successifs confirment le couplage — `nct6687:fan2` suit la
pompe à quelques tours près (2352, 2357) d'une lecture à l'autre.

Aucun ventilateur n'est donc branché sur la carte mère.

Détail qui clôt définitivement la question annexe « MSI accepte-t-il l'écriture ? » : `nct6687`
n'expose **aucun `pwm*_enable`**. Même avec des ventilateurs branchés, il n'y aurait aucun moyen
de passer un canal en mode manuel proprement.

## Combien de ventilateurs par canal ? — sans réponse, et sans conséquence

Le boîtier compte dix ventilateurs pour quatre canaux de vitesse. Ils sont donc repiqués : un
canal en alimente plusieurs, et un seul remonte son régime, les répartiteurs ne transmettant
qu'un fil tachymétrique.

Deux tentatives de mesure ont échoué :

- **à l'oreille** — compter les ventilateurs qui accélèrent n'est pas une observation fiable ;
- **par l'intensité** — `curr1..3_input` seraient proportionnels au nombre de ventilateurs par
  canal, mais le firmware remonte `0 mA` et `0 mV`. Les fichiers existent, les valeurs non.

**Cette inconnue ne change aucune ligne de code.** `reverb fan --all` écrit sur les cinq canaux ;
si dix ventilateurs y sont repiqués, les dix répondent. Ceux qui ne seraient reliés à aucun canal
ne sont contrôlables par aucun logiciel, sous aucun système — ce serait un fait de câblage, pas
une limite de Reverb.

Si la répartition devient utile un jour — pour un profil par zone, par exemple — elle se lira en
suivant les câbles dans le boîtier, bien plus sûrement qu'en écoutant.

## La courbe matérielle du Kraken

`kraken2023elite` expose `temp[1-2]_auto_point[1-40]_pwm` : une **courbe température → PWM à
40 points** exécutée par le firmware. Même philosophie que les modes d'animation `0x2a 0x04` —
on téléverse une intention, le matériel l'exécute sans hôte.

⚠️ **Ces fichiers sont en écriture seule** (`--w-------`, `0200`). Une courbe se pose, elle ne se
relit jamais. Conséquences directes : pas d'édition partielle, pas d'affichage de la courbe
courante, et aucun état conservé côté hôte — il mentirait dès qu'un autre outil écrirait, sans le
moindre moyen de le détecter. Même raisonnement que pour le tampon LED du §5 de la spec.

### Ce que la sonde a établi ✅

Session du 2026-07-30, `tools/sonde_courbe_kraken.sh`. La mesure est le régime, pas l'œil.

**Les valeurs de `pwm_enable`** : `0`, `1` et `2` sont acceptées, `3` refusée.

**`temp1_*` pilote la pompe, et le point 1 vaut 20 °C, un degré par point.** Une courbe plate à
30 % avec un seul point à 100 % a été écrite, ce point étant placé selon trois hypothèses
concurrentes. Le liquide était à 42 °C :

| Courbe | Origine supposée | Point | Pompe |
|---|---|---|---|
| `temp1` | 20 °C | 23 | **2857 tr/min** |
| `temp1` | 25 °C | 18 | 1685 |
| `temp2` | 20 °C | 23 | 1675 |
| `temp2` | 25 °C | 18 | 1685 |

42 − 19 = 23. La pompe s'emballe exactement là où l'hypothèse « point 1 = 20 °C » le prédit, et
nulle part ailleurs. Le point 40 vaut donc 59 °C.

**Le mode courbe fonctionne** : la pompe a suivi le pic. La fonctionnalité était démontrée avant
d'être écrite.

### `temp2` pilote le ventilateur, même cartographie ✅

Seconde sonde, `tools/sonde_courbe_ventilateur.sh`, liquide à 39 °C :

| Essai | Point | Ventilateur |
|---|---|---|
| pic au bon point | 20 | **1785 tr/min** |
| pic à un point volontairement faux | 35 | 751 |
| aucun pic, ligne de base à 30 % | — | 714 |

39 − 19 = 20. Le ventilateur ne répond qu'au pic correctement placé ; le témoin et l'essai à plat
restent à la ligne de base. **Les deux courbes partagent donc la même origine et le même pas** :
point 1 = 20 °C, un degré par point, point 40 = 59 °C.

**Pourquoi la première sonde avait échoué** : elle écrivait `pwm2_enable = 2` sans jamais relire
la valeur. La seconde vérifie chaque écriture de mode. La cause exacte du refus initial reste
inconnue — ce qui est établi, c'est qu'une écriture de mode non relue ne prouve rien.

### Vérification croisée de la cartographie ✅

À la sortie de la seconde sonde, les deux canaux ont été laissés sur des courbes connues. À 39 °C,
elles prescrivent 76 % pour la pompe et 45 % pour le ventilateur. Mesuré : **2586 et 952 tr/min**,
soit exactement les régimes attendus pour ces consignes.

La cartographie n'est donc pas seulement confirmée par un pic isolé : elle prédit correctement le
régime en régime établi, sur les deux canaux à la fois.

### ⚠️ `pwm_enable = 0` ne rend pas le Kraken à son profil d'usine

**Le piège de cette session, et il a coûté cher.**

Avant la sonde, la pompe tournait à 67 % en `pwm_enable = 0`, sensible à la température. La sonde
a écrit des courbes, basculé en `2`, puis restauré `0` — la valeur d'origine. Elle n'a pas
restauré le comportement d'origine : pompe et ventilateur sont restés **à 100 %**, et y sont
restés plus de deux minutes à température stable.

L'interprétation la plus prudente est que `0` signifie « le pilote ne pilote plus », et qu'une
fois une courbe hôte chargée, le firmware ne retrouve pas son profil d'usine — il se rabat sur du
refroidissement maximal. Sûr, bruyant, et **irréversible sans coupure d'alimentation complète**.

Deux leçons :

1. **Restaurer une valeur n'est pas restaurer un comportement.** Un `trap` qui réécrit la valeur
   d'origine donne une fausse impression de sécurité. Toute sonde future doit **mesurer** l'état
   après restauration, pas le supposer. C'est la même erreur que d'avoir cru qu'un redémarrage
   réinitialisait les contrôleurs RGB (spec §9).
2. **Une ligne de base à 30 % était mal calibrée** : la pompe tourne normalement à 67 %, et le
   liquide a pris 9 °C en une minute. Une sonde qui ralentit un organe de refroidissement doit
   partir de son régime habituel, pas d'une valeur arbitraire.

La sortie de secours est `tools/repose_kraken.sh`, qui écrit une vraie courbe et l'active — ou, à
défaut, un **arrêt complet** de la machine, pas un redémarrage.

### Conséquence sur `reverb fan --auto`

L'option écrit `pwm_enable = 0` et son aide annonçait « rend le canal à sa courbe firmware ».
C'est ce que le nom du mode laisse croire, et c'est faux sur le Kraken après usage d'une courbe
hôte. L'aide a été corrigée : `--auto` rend la main au pilote par défaut, sans garantir le retour
au profil d'usine.

### ⚠️ `pwm_enable = 0` **lu** ne dit pas ce qu'un `0` **écrit** fait

C'est la seule valeur de ce fichier qui ait deux sens, et les confondre a coûté un diagnostic.

Le pilote `nzxt-kraken3` **n'écrit rien au probe** : son initialisation n'envoie que
`set_interval` et `finish_init`, aucune consigne ni courbe. Le champ `mode` qu'il tient sort donc
du `kzalloc` à `0`, et `pwmN_enable` rend ce `0` tant que personne n'y a touché.

| | ce que ça établit |
|---|---|
| `0` **lu** | le pilote **ne pilote pas** ce canal. Ce qu'il fait, c'est le périphérique qui le décide — et `pwmN` le dit. |
| `0` **écrit** | `kraken3_write_fixed_duty(priv, 255, channel)` puis plus rien : 100 % et la barre lâchée. |

Relevé sur SHYNAEL le 2026-08-15, Kraken jamais touché depuis le démarrage :

```
pwm1_enable = 0     pwm1 = 77 (30 %)     pompe 1357 tr/min
```

et pendant une session de jeu de 72 minutes, un duty qui a suivi le liquide par paliers — 89,
102, 115, 128, 153 — pour une pompe montée de 1500 à 2380 tr/min. **Une régulation d'usine bien
vivante**, que la colonne MODE annonçait « plein-régime-100% ».

Reverb distingue donc deux modes là où le fichier n'a qu'une valeur : `non-piloté` pour ce qu'une
lecture établit, `plein-régime-100%` pour ce qu'une écriture provoque. Le second ne sort jamais
d'une lecture — c'est une **intention**, pas un état observable, et l'avouer vaut mieux que de
deviner (issue #101).

⚠️ **Après une écriture de `0`, la relecture rend donc `non-piloté` et non `plein-régime-100%`.**
Ce n'est pas une perte : « non piloté » reste vrai — le pilote a bien lâché la barre —, et le
`pwmN` relu montre alors 100 % au lieu de 30 %. C'est le pourcentage qui distingue les deux
situations, pas le mode.
