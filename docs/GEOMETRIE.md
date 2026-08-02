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
deux voisins d'un quart de tour, parce qu'il est **physiquement monté autrement**. Rien dans le
protocole ne l'aurait laissé deviner — c'est exactement ce que la mesure était censée attraper, et
la raison pour laquelle ces valeurs doivent rester modifiables depuis la fenêtre : un ventilateur
démonté puis remis reprend une orientation quelconque.

**Le haut tourne à l'envers, et c'est cohérent.** Un ventilateur du plancher se regarde par sa
face supérieure, un ventilateur du plafond par sa face inférieure : deux faces opposées d'un
même anneau donnent deux sens apparents inverses. Le relevé est donc cohérent avec un montage
identique en haut et en bas — c'est un contrôle croisé qui n'était pas demandé et qui passe.

## Disposition ✅

Les trois du **bas** et les trois du **dessus** s'alignent d'**avant en arrière**, pas d'un flanc
à l'autre. Dans les deux cas, « gauche » est le plus proche de l'**arrière** du boîtier.

Les trois du **radiateur** sont empilés verticalement sur le **flanc du plateau de carte mère**,
« radiateur bas » étant bien le plus bas, et **à l'avant du boîtier** — devant les deux rangées
couchées, qui commencent derrière eux.

```
vue du panneau latéral gauche, arrière ← → avant

   ○   ○   ○         plafond
                 ○
 ○         ▮▮▮▮  ○   ← la colonne du radiateur
(arrière)  RAM   ○
   ○   ○   ○         plancher
```

> ⚠️ **Deux corrections, le même jour, sur deux axes différents.**
>
> **Le plan, corrigé le 2026-08-01.** `SPEC-PROTOCOLE-NZXT.md` §3 se contredit : « 3 sur l'avant »
> et, dans la même phrase, « plaqué contre la face de la carte mère ». La première version de la
> table les avait mis sur la **face** avant, ce qui faisait traverser la direction `avant-arriere`
> de travers. Le boîtier n'a **rien** sur sa face avant : la colonne est dans le plan du flanc.
>
> **La profondeur, corrigée le 2026-08-01 également**, après le schéma que Nico a dessiné : la
> table les avait ensuite placés à **mi-profondeur**, entre les deux rangées couchées. Ils sont à
> l'avant. Une onde `avant-arriere` les atteignait au milieu du parcours au lieu de commencer par
> eux, et la maquette dessinait la colonne par-dessus la RAM — c'est ce qui la rendait illisible.
>
> Les deux énoncés ne se contredisent pas : « sur le flanc » dit le **plan**, « à l'avant » dit la
> **profondeur**.

> ⚠️ **Le nom des deux directions de profondeur a été échangé le 2026-08-02** (issue #49).
>
> La géométrie ci-dessus n'a **pas** bougé : le radiateur est toujours devant les deux rangées
> couchées, la RAM toujours entre lui et le fond. Ce qui a changé, c'est le nom accroché à chaque
> extrémité. C'est **`arriere-avant`** qui commence par la colonne du radiateur et finit par le
> ventilateur du fond ; `avant-arriere` fait l'inverse.
>
> Le critère n° 6 de l'issue #27 disait le contraire — il avait été écrit d'après un schéma. Nico a
> regardé une comète tourner sur le boîtier monté et jugé la paire inversée. C'est la règle du
> projet : **l'étiquette physique se vérifie sur le matériel, elle ne se déduit pas d'un dessin.**
> `crates/reverb-anim/tests/spec_disposition.rs` porte le détail de l'arbitrage.

La **RAM** se situe entre le plancher et le plafond, un peu plus près du plafond, et à mi-chemin
entre le plan du radiateur et le ventilateur arrière — c'est-à-dire dans le vide que le
déplacement de la colonne a ouvert au milieu du boîtier. L'écran du Kraken est immédiatement du
côté arrière des barrettes.

> Ce dernier point recoupe la disposition ATX : sur une carte mère, le socket CPU est du côté du
> panneau d'E/S arrière par rapport aux slots DIMM. Deux observations indépendantes concordent sur
> « gauche = arrière », ce qui est plus solide que l'une des deux seule.

## Par où l'écoulement entre dans un ventilateur ✅

Relevé le **2026-08-01**, dans les termes de Nico :

> on part du bas des ventilos d'en bas et on remonte vers la face du fond du boîtier, où se situe
> la CM, puis on grimpe ce fond, puis on arrive en haut et là on part du fond des ventilos du haut
> pour revenir vers nous

Soit, en heures d'horloge — le point par lequel le motif **entre** dans chaque ventilateur :

| Groupe | Entrée |
|---|---|
| les trois du plancher | 6 h |
| les trois du radiateur | 6 h |
| les trois du plafond | 12 h |
| celui du fond | 12 h |

**Pourquoi cette donnée doit exister.** Quand la direction demandée aplatit un ventilateur — une
onde verticale sur un ventilateur couché, qui n'a aucune hauteur — sa position ne dit plus par où
le motif doit le traverser. Ni le protocole ni la géométrie ne le portent : c'est un choix de
lecture, et il appartient à celui qui regarde le boîtier.

Les heures sont **absolues dans le repère du boîtier**, même convention que les angles ci-dessus.
Elles ne dépendent donc pas de l'orientation de la LED 1 et ne changent pas si l'on remonte un
ventilateur.

Sur un ventilateur que la direction n'aplatit **pas**, la traversée ainsi calculée coïncide
exactement avec la position réelle. Ce n'est donc pas un motif plaqué par-dessus la géométrie,
c'est son prolongement là où elle se tait.

## Échelle

Aucune dimension du boîtier n'a été mesurée. L'échelle est **déduite d'une seule longueur connue
avec certitude** : les ventilateurs sont des F140, donc 140 mm.

Les entraxes en découlent en supposant les ventilateurs jointifs, et le rayon de l'anneau de LED
est estimé à 55 mm.

⚠️ **Ces valeurs absolues n'ont aucune importance.** Les animations ne lisent que des rapports —
« cette LED est-elle plus haute que celle-là », « à quelle fraction du volume se trouve-t-elle ».
Une échelle fausse d'un facteur constant ne changerait rien à ce qui s'affiche. Elles sont là pour
que la maquette 2D de la fenêtre ait des proportions plausibles.

## L'origine des angles ✅

Pour un ventilateur **debout** — le radiateur, l'arrière — « midi » est sans ambiguïté : c'est le
haut du boîtier.

Pour un ventilateur **couché** — les trois du plancher, les trois du plafond — le plan est
horizontal, et « midi » n'a de sens que rapporté à la direction depuis laquelle on l'a regardé.
Les six ont été relevés depuis le même point de vue, et **midi pointe vers le plateau de carte
mère**, c'est-à-dire vers le flanc du boîtier — pas vers l'arrière.

La distinction n'est pas anodine : une convention prise vers l'arrière décalerait ces six
ventilateurs d'un quart de tour, sans effet sur une onde verticale mais bien visible sur toute
rotation.

## Comment la changer

```
geometry                                          # lit la géométrie courante
geometry bas-droite angle=300 sens=horaire        # corrige un ventilateur
```

Le démon persiste dans `/etc/reverb/geometrie.conf`. La fenêtre passera par le même chemin : elle
demande, elle n'écrit aucun fichier.
