#!/usr/bin/env bash
# Confirmation à l'œil du pilotage LED par LED (issue #5, spec §5).
#
# Trois choses à établir, qu'aucun test automatisé ne peut trancher :
#   1. les LED sont bien adressées individuellement ;
#   2. dans quel sens elles sont numérotées sur le ventilateur ;
#   3. ce que fait la variante « animée » de `22 a0` — la capture montre ce
#      que CAM envoie, pas ce que le contrôleur en fait. C'est une inconnue.
#
# Usage : ./tools/confirme_leds.sh
#         (depuis la racine du worktree, sans sudo)

set -u

NOTES="/tmp/reverb-observations-leds.txt"

command -v cargo >/dev/null || . "$HOME/.cargo/env"
REVERB=(cargo run -q --)

VENTILO="radiateur haut"
NOIR="000000"

etape() {
    local numero="$1" titre="$2" question="$3"
    shift 3

    printf '\n'
    printf '═══════════════════════════════════════════════════════════════\n'
    printf ' %s/5 — %s\n' "$numero" "$titre"
    printf '═══════════════════════════════════════════════════════════════\n'
    read -r -p "Entrée pour déclencher… "

    if ! "${REVERB[@]}" "$@"; then
        printf '⚠️  échec de la commande, étape ignorée\n'
        return
    fi

    printf '\n%s\n' "$question"
    read -r -p "> " reponse
    printf '%s\t%s\n' "$titre" "$reponse" >>"$NOTES"
}

printf 'Confirmation à l'"'"'œil du pilotage LED par LED — issue #5\n'
printf 'Ventilateur de test : « %s »\n' "$VENTILO"
printf 'Tes réponses sont notées dans %s\n' "$NOTES"
: >"$NOTES"

etape 1 "Huit couleurs distinctes sur un seul ventilateur" \
    "Vois-tu huit LED de couleurs différentes sur « $VENTILO » ? (o/n + ce que tu vois)" \
    paint --fan "$VENTILO" \
    --colors ff0000,ff8000,ffff00,00ff00,00ffff,0000ff,8000ff,ff00ff

etape 2 "Seule la LED 1 est allumée, en rouge" \
    "Où est la LED allumée sur le ventilateur ? (décris sa position : haut, bas, gauche…)" \
    paint --fan "$VENTILO" \
    --colors ff0000,$NOIR,$NOIR,$NOIR,$NOIR,$NOIR,$NOIR,$NOIR

etape 3 "Seule la LED 8 est allumée, en bleu" \
    "Et celle-ci, où est-elle ? Le sens de numérotation est-il horaire ou antihoraire ?" \
    paint --fan "$VENTILO" \
    --colors $NOIR,$NOIR,$NOIR,$NOIR,$NOIR,$NOIR,$NOIR,0000ff

etape 4 "Le même motif sur les dix ventilateurs" \
    "Les dix ventilateurs ont-ils pris le motif, y compris l'arrière et ceux du haut ? (o/n)" \
    paint --all \
    --colors ff0000,ff0000,ff0000,ff0000,0000ff,0000ff,0000ff,0000ff

etape 5 "La variante ANIMÉE — comportement inconnu" \
    "Que fait le contrôleur du motif ? Il tourne, il fond, il clignote, rien du tout ? Décris." \
    paint --all \
    --colors ff0000,ff8000,ffff00,00ff00,00ffff,0000ff,8000ff,ff00ff \
    --animate

printf '\n═══════════════════════════════════════════════════════════════\n'
printf 'Terminé. Le motif reste en place — le binaire est déjà sorti.\n'
printf 'Si ça bouge encore, c'"'"'est le firmware qui anime, pas nous.\n\n'
read -r -p "Entrée pour revenir à du blanc fixe… "
"${REVERB[@]}" set --all --color ffffff
printf 'Notes : %s\n' "$NOTES"
