#!/usr/bin/env bash
# Lève les trois inconnues de la courbe matérielle du Kraken (issue #9).
#
#   1. quelle valeur de `pwm_enable` déclenche le mode courbe ?
#   2. `temp1_*` pilote-t-il la pompe, et `temp2_*` le ventilateur ?
#   3. à quelle température correspond chaque point de la courbe ?
#
# Les fichiers de courbe sont en écriture seule : rien ne se relit. La seule
# mesure disponible est le régime, et elle suffit — ce script conclut seul, sans
# rien demander à l'œil ni à l'oreille.
#
# Méthode : écrire une courbe plate et basse, avec **un seul point** à 100 %,
# placé là où la température courante devrait tomber selon l'hypothèse testée.
# Si la pompe s'emballe, l'hypothèse est bonne.
#
# Sécurité : les modes d'origine sont relus au départ et restaurés à la sortie,
# y compris sur Ctrl-C. La ligne de base est à 30 %, sans danger quelques
# secondes au repos.
#
# Usage : sudo ./tools/sonde_courbe_kraken.sh

set -u

if [ "$(id -u)" -ne 0 ]; then
    echo "Ce script écrit dans /sys/class/hwmon : relance-le avec sudo." >&2
    exit 1
fi

NOTES="/tmp/reverb-sonde-courbe.txt"
POINTS=40
BASE=77    # ~30 % sur l'échelle 0-255
SOMMET=255 # 100 %

HWMON=""
for h in /sys/class/hwmon/hwmon*; do
    if [ "$(cat "$h/name" 2>/dev/null)" = "kraken2023elite" ]; then
        HWMON="$h"
        break
    fi
done

if [ -z "$HWMON" ]; then
    echo "Kraken introuvable. Le module nzxt_kraken3 est-il chargé ?" >&2
    exit 1
fi

ENABLE1_ORIGINE=$(cat "$HWMON/pwm1_enable")
ENABLE2_ORIGINE=$(cat "$HWMON/pwm2_enable")

restaure() {
    echo
    echo "Restauration : pwm1_enable=$ENABLE1_ORIGINE, pwm2_enable=$ENABLE2_ORIGINE"
    echo "$ENABLE1_ORIGINE" >"$HWMON/pwm1_enable" 2>/dev/null
    echo "$ENABLE2_ORIGINE" >"$HWMON/pwm2_enable" 2>/dev/null
    echo "La pompe est rendue à sa courbe firmware."
}
trap restaure EXIT

temperature() { echo $(( $(cat "$HWMON/temp1_input") / 1000 )); }
pompe()       { cat "$HWMON/fan1_input"; }
ventilateur() { cat "$HWMON/fan2_input"; }

# Écrit les 40 points d'une courbe : `courbe <temp1|temp2> <point à 100 %>`.
# Un point à 0 met toute la courbe à la ligne de base.
courbe() {
    local prefixe="$1" sommet="$2" i valeur
    for i in $(seq 1 $POINTS); do
        if [ "$i" = "$sommet" ]; then valeur=$SOMMET; else valeur=$BASE; fi
        if ! echo "$valeur" >"$HWMON/${prefixe}_auto_point${i}_pwm" 2>/dev/null; then
            return 1
        fi
    done
    return 0
}

echo "Kraken : $HWMON"
echo "Température du liquide : $(temperature) °C"
echo "Au repos — pompe $(pompe) tr/min, ventilateur $(ventilateur) tr/min"
echo "Modes d'origine : pwm1_enable=$ENABLE1_ORIGINE pwm2_enable=$ENABLE2_ORIGINE"
: >"$NOTES"

# ── Inconnue 1 ────────────────────────────────────────────────────────────────
echo
echo "═══ 1. Quelle valeur de pwm_enable accepte le noyau ? ═══"
for valeur in 0 1 2 3; do
    if echo "$valeur" >"$HWMON/pwm1_enable" 2>/dev/null; then
        relu=$(cat "$HWMON/pwm1_enable")
        echo "  pwm1_enable=$valeur  accepté, relu à $relu"
        printf 'enable\t%s\tacceptee (relue %s)\n' "$valeur" "$relu" >>"$NOTES"
    else
        echo "  pwm1_enable=$valeur  REFUSÉ"
        printf 'enable\t%s\trefusee\n' "$valeur" >>"$NOTES"
    fi
done
echo "$ENABLE1_ORIGINE" >"$HWMON/pwm1_enable" 2>/dev/null

# ── Inconnues 2 et 3 ──────────────────────────────────────────────────────────
# L'hypothèse « point 1 = 20 °C, un degré par point » place la température
# courante au point (T - 19). On essaie aussi deux autres origines plausibles,
# et un point volontairement faux comme témoin.
T=$(temperature)
declare -a HYPOTHESES=(
    "$((T - 19)):origine 20 °C"
    "$((T - 24)):origine 25 °C"
    "$((T + 1)):origine 0 °C"
)

echo
echo "═══ 2 et 3. Quel point correspond à $T °C, et quelle courbe pilote quoi ? ═══"
echo "Ligne de base 30 %, un seul point à 100 %. La pompe s'emballe si le point vise juste."

for prefixe in temp1 temp2; do
    echo
    echo "── courbe $prefixe ──"
    for hypothese in "${HYPOTHESES[@]}"; do
        point="${hypothese%%:*}"
        libelle="${hypothese#*:}"

        if [ "$point" -lt 1 ] || [ "$point" -gt $POINTS ]; then
            echo "  $libelle : point $point hors de 1..$POINTS, ignoré"
            continue
        fi

        if ! courbe "$prefixe" "$point"; then
            echo "  $libelle : écriture de la courbe REFUSÉE"
            printf '%s\t%s\tecriture refusee\n' "$prefixe" "$libelle" >>"$NOTES"
            continue
        fi
        echo 2 >"$HWMON/pwm1_enable" 2>/dev/null
        echo 2 >"$HWMON/pwm2_enable" 2>/dev/null
        sleep 4

        p=$(pompe)
        v=$(ventilateur)
        echo "  $libelle, point $point : pompe $p tr/min, ventilateur $v tr/min"
        printf '%s\t%s\tpoint %s\tpompe %s\tventilo %s\n' \
            "$prefixe" "$libelle" "$point" "$p" "$v" >>"$NOTES"
    done
done

echo
echo "═══════════════════════════════════════════════════════════════"
echo "Relevé complet : $NOTES"
echo "La ligne qui montre le régime le plus élevé désigne la bonne hypothèse."
