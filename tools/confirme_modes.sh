#!/usr/bin/env bash
# Confirmation à l'œil des modes d'animation (issue #3, spec §4.1).
#
# Un nom de mode ne peut être confirmé que par un humain devant le boîtier :
# ce script les déclenche un par un, à ton rythme, et note ce que tu décris.
#
# Il a servi le 2026-07-30 à lever les cinq noms encore hypothétiques (§4.5).
# Il resservira pour tout mode ajouté à la table sans observation — le `0x03`,
# jamais déclenché pendant la capture, en premier.
#
# Usage : ./tools/confirme_modes.sh
#         (depuis la racine du worktree, sans sudo)

set -u

NOTES="/tmp/reverb-observations.txt"

command -v cargo >/dev/null || . "$HOME/.cargo/env"
REVERB=(cargo run -q --)

# Mode, couleurs, hypothèse à ne révéler qu'APRÈS la description.
# La formuler avant biaiserait ce que tu crois voir.
etape() {
    local numero="$1" mode="$2" hypothese="$3"
    shift 3
    local couleurs=("$@")

    local args=(set --all --mode "$mode")
    local couleur
    for couleur in "${couleurs[@]}"; do
        args+=(--color "$couleur")
    done

    printf '\n'
    printf '═══════════════════════════════════════════════════════════════\n'
    printf ' %s/5 — mode « %s »' "$numero" "$mode"
    if [ ${#couleurs[@]} -gt 0 ]; then
        printf '   couleurs : %s' "${couleurs[*]}"
    fi
    printf '\n'
    printf '═══════════════════════════════════════════════════════════════\n'
    read -r -p "Entrée pour déclencher… "

    if ! "${REVERB[@]}" "${args[@]}"; then
        printf '⚠️  échec de la commande, mode « %s » ignoré\n' "$mode"
        return
    fi

    printf '\nRegarde les ventilateurs. Décris ce que tu vois, avec tes mots.\n'
    read -r -p "> " description

    printf '\n💡 Hypothèse de la spec pour ce mode : %s\n' "$hypothese"
    read -r -p "Ça correspond ? (o/n/?) " verdict

    printf '%s\t%s\t%s\t%s\n' "$mode" "$verdict" "$description" "$hypothese" >>"$NOTES"
}

printf 'Confirmation à l'"'"'œil des modes d'"'"'animation — issue #3\n'
printf 'Tes réponses sont notées dans %s\n' "$NOTES"
: >"$NOTES"

# Référence : mode confirmé, pour avoir un point de comparaison en tête.
printf '\nD'"'"'abord une référence — spectrum-wave, déjà confirmé.\n'
read -r -p "Entrée pour le déclencher… "
"${REVERB[@]}" set --all --mode spectrum-wave
printf 'Les teintes doivent défiler. C'"'"'est à quoi ressemble un mode qui marche.\n'
read -r -p "Entrée pour passer aux cinq inconnus… "

etape 1 fading            "Fading — fondu enchaîné d'une couleur à l'autre" ff0000 00ff00 0000ff
etape 2 covering-marquee  "Covering Marquee — une bande de couleur qui recouvre le tour" ff0000 00ff00 0000ff
etape 3 alternating       "Alternating — les LED alternent par blocs entre les deux couleurs" ff0000 0000ff
etape 4 pulse             "Pulse — battement plus sec qu'un breathing" ff0000
etape 5 starry-night      "Starry Night — quelques LED scintillent au hasard" ff0000

printf '\n═══════════════════════════════════════════════════════════════\n'
printf 'Terminé. Je remets une couleur fixe blanche.\n'
"${REVERB[@]}" set --all --color ffffff
printf 'Notes : %s\n' "$NOTES"
