#!/usr/bin/env bash
# Quels ventilateurs répondent à quel canal PWM ? (issue #7)
#
# Le contrôleur NZXT n'expose que trois canaux ventilateur, mais le boîtier en
# compte dix. Les prises de la carte mère se sont révélées vides — un seul
# tachymètre y tourne, et c'est celui de la pompe, câblé sur CPU_FAN.
#
# Hypothèse à vérifier : les dix ventilateurs sont repiqués en série sur les
# trois canaux NZXT. Un canal alimente alors plusieurs ventilateurs, dont un
# seul remonte son régime — ce qui expliquerait tout.
#
# Ce script monte un canal à la fois à 100 %, à ton rythme, et remet la
# consigne d'origine à la fin. C'est bruyant, pas dangereux.
#
# Usage : sudo ./tools/couverture_pwm.sh

set -u

if [ "$(id -u)" -ne 0 ]; then
    echo "Ce script écrit dans /sys/class/hwmon : relance-le avec sudo." >&2
    exit 1
fi

NOTES="/tmp/reverb-observations-pwm.txt"

# Résout le contrôleur par son nom : les numéros hwmon changent au redémarrage.
HWMON=""
for h in /sys/class/hwmon/hwmon*; do
    if [ "$(cat "$h/name" 2>/dev/null)" = "nzxtsmart2" ]; then
        HWMON="$h"
        break
    fi
done

if [ -z "$HWMON" ]; then
    echo "Contrôleur « nzxtsmart2 » introuvable. Le module nzxt_smart2 est-il chargé ?" >&2
    exit 1
fi

echo "Contrôleur : $HWMON"
echo "Tes réponses sont notées dans $NOTES"
: >"$NOTES"

# Sauvegarde des consignes, pour les rendre telles quelles à la fin.
ORIGINE=""
for i in 1 2 3; do
    ORIGINE="$ORIGINE $(cat "$HWMON/pwm$i")"
done
echo "Consignes d'origine :$ORIGINE"

restaure() {
    echo
    echo "Restauration des consignes d'origine…"
    set -- $ORIGINE
    for i in 1 2 3; do
        eval "valeur=\$$i"
        echo "$valeur" >"$HWMON/pwm$i"
    done
    echo "Fait."
}
trap restaure EXIT

canal() {
    local numero="$1"

    printf '\n'
    printf '═══════════════════════════════════════════════════════════════\n'
    printf ' Canal %s seul à 100 %%, les deux autres au minimum\n' "$numero"
    printf '═══════════════════════════════════════════════════════════════\n'
    read -r -p "Entrée pour lancer… "

    for i in 1 2 3; do
        if [ "$i" = "$numero" ]; then
            echo 255 >"$HWMON/pwm$i"
        else
            echo 51 >"$HWMON/pwm$i"
        fi
    done

    sleep 3
    printf 'Régimes remontés :'
    for i in 1 2 3; do
        printf ' fan%s=%s' "$i" "$(cat "$HWMON/fan${i}_input")"
    done
    printf ' tr/min\n\n'

    printf 'Combien de ventilateurs accélèrent, et lesquels ? (bas, radiateur, haut, arrière…)\n'
    read -r -p "> " reponse
    printf 'canal %s\t%s\n' "$numero" "$reponse" >>"$NOTES"
}

printf '\nD'"'"'abord la question qui tranche tout.\n'
read -r -p "Entrée pour monter les TROIS canaux à 100 %… "
for i in 1 2 3; do echo 255 >"$HWMON/pwm$i"; done
sleep 3
printf '\nCombien de ventilateurs sur les dix accélèrent ? Trois, ou beaucoup plus ?\n'
read -r -p "> " global
printf 'les trois canaux\t%s\n' "$global" >>"$NOTES"

canal 1
canal 2
canal 3

printf '\n═══════════════════════════════════════════════════════════════\n'
printf 'Terminé.\n'
