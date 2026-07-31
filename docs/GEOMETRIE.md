# Géométrie du boîtier — SHYNAEL

> **Ce document n'est pas une spécification de protocole.** Les trois `SPEC-*.md` décrivent des
> périphériques ; celui-ci décrit **une machine**. Quelqu'un qui reprend Reverb sur un autre
> boîtier aura d'autres valeurs, et c'est pourquoi elles se changent par une commande plutôt que
> par une recompilation.

Mesuré le **2026-07-31** avec `tools/mesure_orientation.sh`.

## Pourquoi cette mesure existe

`SPEC-PROTOCOLE-NZXT.md` §5 :

> ⚠️ **La position absolue de la LED 1 dépend du montage.** […] Seul l'**ordre** est une donnée
> du protocole ; l'origine et le sens apparent sont une donnée de montage.

Le protocole dit que les huit LED d'un ventilateur forment un anneau parcouru dans un sens
constant. Il ne dit pas **où** commence l'anneau, ni **dans quel sens il tourne quand on le
regarde**. Sans ces deux nombres par ventilateur, aucune animation ne peut être synchronisée dans
l'espace : elle ne peut que courir le long d'une file d'attente numérotée.

## Le pas angulaire : régulier ✅

Les LED 1 et 5 sont diamétralement opposées sur les dix ventilateurs. Les huit LED sont donc
réparties à **45°** exactement, et non selon un découpage irrégulier.

C'est ce que le repère bleu servait à vérifier, et c'est confirmé.

## Orientation, ventilateur par ventilateur ✅

Angle de la **LED 1**, en heures d'horloge relevées à l'œil, converties à raison de 30° par heure.

| Position | LED 1 | Angle | Sens vu de l'extérieur |
|---|---|---|---|
| bas gauche | 10 h | 300° | horaire |
| bas milieu | 10 h | 300° | horaire |
| bas droite | 7 h | 210° | horaire |
| radiateur bas | 7 h | 210° | horaire |
| radiateur milieu | 7 h | 210° | horaire |
| radiateur haut | 7 h | 210° | horaire |
| haut gauche | 2 h | 60° | **antihoraire** |
| haut milieu | 2 h | 60° | **antihoraire** |
| haut droite | 2 h | 60° | **antihoraire** |
| arrière | 10 h | 300° | horaire |

**Deux ventilateurs au même endroit ne sont pas montés pareil.** « bas droite » diffère de ses
deux voisins d'un quart de tour. Rien dans le protocole ne l'aurait laissé deviner — c'est
exactement ce que la mesure était censée attraper.

**Le haut tourne à l'envers, et c'est cohérent.** Un ventilateur du plancher se regarde par sa
face supérieure, un ventilateur du plafond par sa face inférieure : deux faces opposées d'un
même anneau donnent deux sens apparents inverses. Le relevé est donc cohérent avec un montage
identique en haut et en bas — c'est un contrôle croisé qui n'était pas demandé et qui passe.

## Disposition ✅

Les trois du **bas** et les trois du **dessus** s'alignent d'**avant en arrière**, pas d'un flanc
à l'autre. Dans les deux cas, « gauche » est le plus proche de l'**arrière** du boîtier.

Les trois du **radiateur** sont empilés verticalement sur la face avant, « radiateur bas » étant
bien le plus bas.

La **RAM** se situe entre le plancher et le plafond, un peu plus près du plafond, et à mi-chemin
entre le plan du radiateur et le ventilateur arrière. L'écran du Kraken est immédiatement du côté
arrière des barrettes.

> Ce dernier point recoupe la disposition ATX : sur une carte mère, le socket CPU est du côté du
> panneau d'E/S arrière par rapport aux slots DIMM. Deux observations indépendantes concordent sur
> « gauche = arrière », ce qui est plus solide que l'une des deux seule.

## Échelle

Aucune dimension du boîtier n'a été mesurée. L'échelle est **déduite d'une seule longueur connue
avec certitude** : les ventilateurs sont des F140, donc 140 mm.

Les entraxes en découlent en supposant les ventilateurs jointifs, et le rayon de l'anneau de LED
est estimé à 55 mm.

⚠️ **Ces valeurs absolues n'ont aucune importance.** Les animations ne lisent que des rapports —
« cette LED est-elle plus haute que celle-là », « à quelle fraction du volume se trouve-t-elle ».
Une échelle fausse d'un facteur constant ne changerait rien à ce qui s'affiche. Elles sont là pour
que la maquette 2D de la fenêtre ait des proportions plausibles.

## Ce qui reste incertain

**L'origine des angles pour les quatre ventilateurs horizontaux** (les trois du bas, les trois du
dessus). Pour un ventilateur vertical — le radiateur, l'arrière — « midi » est sans ambiguïté :
c'est le haut du boîtier. Pour un ventilateur couché, le plan est horizontal, et « midi » dépend
de la direction depuis laquelle on l'a regardé : vers l'avant, vers l'arrière, ou vers un flanc.

Conséquence concrète, et elle est bornée :

- une onde **verticale** est insensible au problème — toutes les LED d'un ventilateur couché sont
  à la même hauteur ;
- une onde **avant-arrière** ou une rotation seraient décalées d'un quart ou d'un demi-tour sur
  ces six ventilateurs.

C'est une donnée de configuration, donc **une commande la corrige** sans recompiler — précisément
ce pour quoi `geometry` existe. Elle se tranchera d'un coup d'œil sur une onde avant-arrière.

## Comment la changer

```
geometry                                          # lit la géométrie courante
geometry bas-droite angle=300 sens=horaire        # corrige un ventilateur
```

Le démon persiste dans `/etc/reverb/geometrie.conf`. La fenêtre passera par le même chemin : elle
demande, elle n'écrit aucun fichier.
