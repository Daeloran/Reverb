#!/usr/bin/env bash
# Mesure de l'orientation physique des LED (issue #19).
#
# Ce que ce script obtient, et qu'AUCUNE lecture du matériel ne peut donner :
# où se trouve la LED 1 de chaque ventilateur, et dans quel sens l'anneau
# tourne vu de l'extérieur. La spec §5 le dit — c'est une donnée de montage,
# pas de protocole.
#
# Un seul allumage porte trois informations :
#
#     LED 1 → ROUGE   l'origine
#     LED 2 → VERT    le sens (il est le voisin immédiat du rouge)
#     LED 5 → BLEU    le pas : si les huit LED sont à 45°, il est PILE en face
#
# Et il les porte sur les dix ventilateurs À LA FOIS. Un seul passage sous le
# bureau.
#
# ⚠️ Le démon est arrêté pendant la mesure et redémarré à la fin.
#
# Usage : ./tools/mesure_orientation.sh

set -u

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
REVERB="$RACINE/target/release/reverb"
NOTES="/tmp/reverb-orientation.txt"

: >"$NOTES"
note() { printf '%s\t%s\n' "$1" "$2" >>"$NOTES"; }
demande() {
    echo
    read -r -p "   ↳ $1 " reponse
    note "$2" "$reponse"
}

[ -x "$REVERB" ] || {
    echo "Compilation…"
    (cd "$RACINE" && cargo build --release 2>&1 | tail -1)
}

echo "═══ Le démon doit se taire pendant la mesure ═══"
DEMON_TOURNAIT=non
if systemctl is-active --quiet reverbd 2>/dev/null; then
    DEMON_TOURNAIT=oui
    sudo systemctl stop reverbd && echo "   démon arrêté (il sera relancé à la fin)"
    sleep 1
else
    echo "   le démon ne tourne pas, rien à arrêter"
fi

# Rendre le résultat inobservable serait le pire défaut de ce script : on
# éteint tout d'abord, pour qu'aucune couleur résiduelle ne se fasse prendre
# pour un repère.
echo
echo "═══ Extinction, puis allumage des repères ═══"
"$REVERB" set --all --color 000000 >/dev/null 2>&1
sleep 1
"$REVERB" paint --all --colors ff0000,00ff00,000000,000000,0000ff,000000,000000,000000 \
    || { echo "   ❌ l'allumage a échoué"; exit 1; }

cat <<'TEXTE'

   Chaque ventilateur porte maintenant TROIS LED allumées, les cinq autres
   éteintes :

       ROUGE  = LED 1        VERT = LED 2        BLEU = LED 5

   Va voir. Prends ton temps, rien ne s'éteint tout seul.

TEXTE

read -r -p "   ↳ Appuie sur Entrée quand tu es devant le boîtier. "

echo
echo "═══ 1. Le pas angulaire ═══"
echo "   Si les huit LED sont réparties régulièrement, le BLEU est diamétralement"
echo "   opposé au ROUGE — pile en face, sur tous les ventilateurs."
demande "Le bleu est-il en face du rouge ? (oui / non, décrire)" "pas-angulaire"

echo
echo "═══ 2. Le sens de rotation ═══"
echo "   Regarde un ventilateur de face, comme tu le vois depuis l'extérieur du"
echo "   boîtier. Le VERT est le voisin immédiat du ROUGE."
demande "En allant du rouge au vert, on tourne dans quel sens ? (horaire / antihoraire / ça dépend, décrire)" "sens"

echo
echo "═══ 3. Où est le rouge, ventilateur par ventilateur ═══"
echo "   Donne la position du ROUGE en heures d'horloge (12 = en haut, 3 = à"
echo "   droite, 6 = en bas, 9 = à gauche), vu depuis l'extérieur."
echo
echo "   Si tout un groupe est identique, réponds une fois pour le groupe."
demande "Les TROIS DU BAS — heures, ou « tous à H » (ex. « tous à 12 » / « 12, 3, 9 »)" "bas"
demande "Les TROIS DU RADIATEUR (avant) — idem" "radiateur"
demande "Les TROIS DU DESSUS — idem" "haut"
demande "Celui de l'ARRIÈRE — heure" "arriere"

echo
echo "═══ 4. La disposition, que le code ne connaît pas ═══"
echo "   « bas gauche / milieu / droite » ne dit pas selon quel axe ils sont"
echo "   alignés. Sans ça, une onde qui traverse le boîtier ira dans la mauvaise"
echo "   direction."
demande "Les trois du BAS s'alignent-ils d'AVANT en ARRIÈRE, ou d'un FLANC à l'autre ?" "axe-bas"
demande "Et « bas gauche », c'est le plus proche de l'AVANT du boîtier, ou de l'ARRIÈRE / du flanc gauche ?" "origine-bas"
demande "Même question pour les TROIS DU DESSUS (axe, et où est « haut gauche »)" "axe-haut"
demande "Les trois du RADIATEUR sont-ils empilés verticalement ? « radiateur bas » est-il bien le plus bas ?" "radiateur-vertical"

echo
echo "═══ 5. La RAM dans l'espace ═══"
echo "   Les barrettes seront traversées par les mêmes ondes que les ventilateurs."
demande "Les barrettes sont-elles PLUS HAUTES ou PLUS BASSES que les ventilateurs du dessus ? Et plutôt vers l'AVANT ou l'ARRIÈRE ?" "ram-position"

echo
echo "═══ Remise en état ═══"
"$REVERB" set --all --color 000000 >/dev/null 2>&1
if [ "$DEMON_TOURNAIT" = oui ]; then
    sudo systemctl start reverbd && echo "   démon relancé"
else
    echo "   le démon était arrêté avant, il le reste"
fi

echo
echo "═══ Relevé ═══"
cat "$NOTES"
echo
echo "Colle ce bloc dans la conversation."
