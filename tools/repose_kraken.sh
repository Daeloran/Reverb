#!/usr/bin/env bash
# Rend le Kraken à un fonctionnement calme et sensible à la température.
#
# Après une session de sonde, `pwm_enable = 0` ne restaure PAS le profil
# d'usine : le pilote cesse simplement de piloter, et le firmware retombe sur
# son repli de sécurité — 100 % partout. Sûr, mais bruyant.
#
# Ce script écrit une vraie courbe et la fait exécuter par le firmware. C'est
# la fonctionnalité de l'issue #9, utilisée pour se sortir d'affaire.
#
# La cartographie est celle établie par `sonde_courbe_kraken.sh` :
# point 1 = 20 °C, un degré par point, jusqu'au point 40 = 59 °C.
#
# Si la courbe ne prend pas, repli sur le mode manuel aux valeurs d'origine.
#
# Usage : sudo ./tools/repose_kraken.sh

set -u

if [ "$(id -u)" -ne 0 ]; then
    echo "Ce script écrit dans /sys/class/hwmon : relance-le avec sudo." >&2
    exit 1
fi

HWMON=""
for h in /sys/class/hwmon/hwmon*; do
    if [ "$(cat "$h/name" 2>/dev/null)" = "kraken2023elite" ]; then
        HWMON="$h"
        break
    fi
done
[ -n "$HWMON" ] || { echo "Kraken introuvable." >&2; exit 1; }

temperature() { echo $(( $(cat "$HWMON/temp1_input") / 1000 )); }
etat() {
    printf '  liquide %s °C — pompe %s tr/min, ventilateur %s tr/min\n' \
        "$(temperature)" "$(cat "$HWMON/fan1_input")" "$(cat "$HWMON/fan2_input")"
}

# Consigne, sur l'échelle 0-255, pour le point `i` (1 = 20 °C, 40 = 59 °C).
# Deux paliers linéaires, volontairement conservateurs.
pwm_pompe() {
    local t=$(( $1 + 19 ))
    if   [ "$t" -le 30 ]; then echo 128   # 50 % jusqu'à 30 °C
    elif [ "$t" -ge 45 ]; then echo 255   # 100 % à partir de 45 °C
    else echo $(( 128 + (t - 30) * 127 / 15 ))
    fi
}

pwm_ventilateur() {
    local t=$(( $1 + 19 ))
    if   [ "$t" -le 32 ]; then echo 64    # 25 % jusqu'à 32 °C
    elif [ "$t" -ge 50 ]; then echo 255   # 100 % à partir de 50 °C
    else echo $(( 64 + (t - 32) * 191 / 18 ))
    fi
}

ecrit_courbe() {
    local prefixe="$1" calcul="$2" i
    for i in $(seq 1 40); do
        echo "$($calcul "$i")" >"$HWMON/${prefixe}_auto_point${i}_pwm" 2>/dev/null || return 1
    done
    return 0
}

echo "Kraken : $HWMON"
echo "Avant :"
etat

echo
echo "Écriture de la courbe de la pompe (temp1)…"
if ecrit_courbe temp1 pwm_pompe && echo 2 >"$HWMON/pwm1_enable" 2>/dev/null; then
    echo "  courbe écrite, mode courbe activé"
else
    echo "  échec — repli sur le mode manuel à 67 %"
    echo 1 >"$HWMON/pwm1_enable" && echo 171 >"$HWMON/pwm1"
fi

echo "Écriture de la courbe du ventilateur (temp2)…"
if ecrit_courbe temp2 pwm_ventilateur && echo 2 >"$HWMON/pwm2_enable" 2>/dev/null; then
    echo "  courbe écrite, mode courbe activé"
else
    echo "  échec — repli sur le mode manuel à 28 %"
    echo 1 >"$HWMON/pwm2_enable" && echo 71 >"$HWMON/pwm2"
fi

echo
echo "Stabilisation…"
sleep 8
echo "Après :"
etat

echo
echo "Si la pompe est encore à fond, un arrêt COMPLET de la machine (pas un"
echo "redémarrage) rend le Kraken à son état d'usine — même mécanique que les"
echo "contrôleurs RGB, dont l'état vit en mémoire volatile (spec §9)."
