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
processeur au démarrage.

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

## Ce que le matériel sait faire sans nous

`kraken2023elite` expose `temp1_auto_point1..40_pwm` : une **courbe température → PWM à 40 points**
exécutée par le firmware de la pompe. Même philosophie que les modes d'animation `0x2a 0x04` —
on téléverse une intention, le matériel l'exécute sans hôte.

Hors scope pour l'instant, mais c'est la bonne façon de piloter une pompe : une courbe qui survit
à l'extinction de Reverb vaut mieux qu'un démon qui la surveille.

⚠️ Corollaire : la pompe et le ventilateur du Kraken sont en `pwm_enable = 0`, donc **sur leur
courbe firmware**. Leur imposer une consigne fixe les en sort, et ils cessent de réagir à la
température du liquide. Ce basculement doit rester explicite.
