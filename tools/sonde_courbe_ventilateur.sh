#!/usr/bin/env bash
# `temp2_auto_point*` pilote-t-il le ventilateur du Kraken ? (issue #9)
#
# La première sonde a laissé la question ouverte : le ventilateur n'a jamais
# bougé, mais elle ne vérifiait pas que `pwm2_enable = 2` avait bien pris. Elle
# a aussi laissé la machine à 100 % en croyant l'avoir restaurée.
#
# Ce qui change ici :
#
#   • la pompe est mise sur une VRAIE courbe dès le départ et y reste. Elle
#     n'est jamais la variable, et le liquide reste maîtrisé ;
#   • chaque écriture de mode est RELUE pour vérifier qu'elle a pris ;
#   • la sortie ne rend pas une valeur, elle laisse les deux canaux sur une
#     courbe saine et MESURE le résultat avant de rendre la main.
#
# Usage : sudo ./tools/sonde_courbe_ventilateur.sh

set -u

[ "$(id -u)" -eq 0 ] || { echo "Relance avec sudo." >&2; exit 1; }

NOTES="/tmp/reverb-sonde-ventilateur.txt"

HWMON=""
for h in /sys/class/hwmon/hwmon*; do
    [ "$(cat "$h/name" 2>/dev/null)" = "kraken2023elite" ] && HWMON="$h" && break
done
[ -n "$HWMON" ] || { echo "Kraken introuvable." >&2; exit 1; }

temperature() { echo $(( $(cat "$HWMON/temp1_input") / 1000 )); }
pompe()       { cat "$HWMON/fan1_input"; }
ventilateur() { cat "$HWMON/fan2_input"; }

# Point de courbe correspondant à une température : point 1 = 20 °C, un degré
# par point. Cartographie établie par la première sonde.
point_de() { echo $(( $1 - 19 )); }

# `courbe <temp1|temp2> <base 0-255> <point à 100 %, ou 0 pour aucun>`
courbe() {
    local prefixe="$1" base="$2" sommet="$3" i valeur
    for i in $(seq 1 40); do
        if [ "$i" = "$sommet" ]; then valeur=255; else valeur=$base; fi
        echo "$valeur" >"$HWMON/${prefixe}_auto_point${i}_pwm" 2>/dev/null || return 1
    done
}

# Courbe de sécurité pour la pompe : 60 % jusqu'à 35 °C, 100 % à partir de 45.
courbe_pompe_sure() {
    local i t valeur
    for i in $(seq 1 40); do
        t=$(( i + 19 ))
        if   [ "$t" -le 35 ]; then valeur=153
        elif [ "$t" -ge 45 ]; then valeur=255
        else valeur=$(( 153 + (t - 35) * 102 / 10 ))
        fi
        echo "$valeur" >"$HWMON/temp1_auto_point${i}_pwm" 2>/dev/null || return 1
    done
}

# Écrit un mode et vérifie qu'il a pris. C'est ce qui manquait la dernière fois.
mode() {
    local canal="$1" valeur="$2" relu
    echo "$valeur" >"$HWMON/pwm${canal}_enable" 2>/dev/null
    relu=$(cat "$HWMON/pwm${canal}_enable" 2>/dev/null)
    if [ "$relu" != "$valeur" ]; then
        echo "  ⚠️  pwm${canal}_enable demandé à $valeur, relu à $relu"
        return 1
    fi
    return 0
}

echo "Kraken : $HWMON"
echo "Départ — liquide $(temperature) °C, pompe $(pompe) tr/min, ventilateur $(ventilateur) tr/min"
: >"$NOTES"

# ── La pompe d'abord, et elle n'y touche plus ────────────────────────────────
echo
echo "═══ Mise en sécurité de la pompe ═══"
if courbe_pompe_sure && mode 1 2; then
    echo "  pompe sur sa courbe : 60 % jusqu'à 35 °C, 100 % à 45 °C"
else
    echo "  ⚠️  échec — repli en manuel à 67 %"
    mode 1 1 && echo 171 >"$HWMON/pwm1"
fi
sleep 5
echo "  liquide $(temperature) °C, pompe $(pompe) tr/min"

# ── Le ventilateur ───────────────────────────────────────────────────────────
echo
echo "═══ temp2 pilote-t-il le ventilateur ? ═══"
echo "Ligne de base 30 %, un seul point à 100 %."

essai() {
    local libelle="$1" point="$2"

    if ! courbe temp2 77 "$point"; then
        echo "  $libelle : écriture de la courbe REFUSÉE"
        printf '%s\tecriture refusee\n' "$libelle" >>"$NOTES"
        return
    fi
    if ! mode 2 2; then
        printf '%s\tmode 2 refuse\n' "$libelle" >>"$NOTES"
        return
    fi
    sleep 5
    local v=$(ventilateur)
    echo "  $libelle (point $point) : ventilateur $v tr/min"
    printf '%s\tpoint %s\tventilo %s\n' "$libelle" "$point" "$v" >>"$NOTES"
}

T=$(temperature)
JUSTE=$(point_de "$T")
FAUX=$(( JUSTE > 20 ? JUSTE - 15 : JUSTE + 15 ))

essai "pic au bon point ($T °C)" "$JUSTE"
essai "pic à un point volontairement faux" "$FAUX"
essai "aucun pic, tout à 30 %" 0

# ── Sortie mesurée, pas supposée ─────────────────────────────────────────────
echo
echo "═══ Sortie ═══"
echo "Les deux canaux sont laissés sur une courbe saine, en mode 2."
echo "On ne remet PAS pwm_enable à 0 : c'est ce qui avait laissé la machine à fond."

# Ventilateur : 25 % jusqu'à 35 °C, 100 % à 50 °C.
for i in $(seq 1 40); do
    t=$(( i + 19 ))
    if   [ "$t" -le 35 ]; then v=64
    elif [ "$t" -ge 50 ]; then v=255
    else v=$(( 64 + (t - 35) * 191 / 15 ))
    fi
    echo "$v" >"$HWMON/temp2_auto_point${i}_pwm" 2>/dev/null
done
mode 2 2

sleep 8
echo
echo "Mesure finale — liquide $(temperature) °C, pompe $(pompe) tr/min, ventilateur $(ventilateur) tr/min"
echo "modes : pwm1_enable=$(cat "$HWMON/pwm1_enable") pwm2_enable=$(cat "$HWMON/pwm2_enable")"
echo
echo "Si la pompe est descendue sous 2500 tr/min, la machine est calme et"
echo "sensible à la température. Sinon, dis-le-moi : ce sera un résultat, pas"
echo "une supposition."
echo
echo "Relevé : $NOTES"
