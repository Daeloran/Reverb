#!/usr/bin/env bash
# Vérification matérielle de l'éclairage de la RAM (issue #15).
#
# ⚠️ REGARDE LES BARRETTES DE RAM pendant toute la durée.
#
# ⚠️ C'est la seule cible du projet où une erreur est irréversible : le même bus
# porte les hubs SPD des barrettes en 0x50–0x53. Reverb ne peut pas les viser —
# `SlotAddress` ne se construit que depuis un index d'emplacement — mais un
# AUTRE logiciel qui parlerait au bus en même temps corromprait les
# transactions. D'où le contrôle de concurrence ci-dessous.
#
# Ce script ne sonde JAMAIS le bus. L'adaptateur est reconnu à son nom dans
# sysfs, et les seules écritures vont en 0x18–0x1b.
#
# Usage : ./tools/verifie_ram.sh

set -u

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
REVERB="$RACINE/target/debug/reverb"
REGLES="$RACINE/packaging/60-reverb.rules"
NOTES="/tmp/reverb-observations-ram.txt"

[ -x "$REVERB" ] || { echo "Compile d'abord : cargo build" >&2; exit 1; }
: >"$NOTES"

pause() { echo; read -r -p "   ↳ $1 " r; printf '%s\t%s\n' "$2" "$r" >>"$NOTES"; }

echo "═══ 0. Personne d'autre ne doit parler au bus ═══"
CONCURRENTS=""
for p in openrgb OpenRGB liquidctl i2cdetect; do
    pgrep -x "$p" >/dev/null 2>&1 && CONCURRENTS="$CONCURRENTS $p"
done
if [ -n "$CONCURRENTS" ]; then
    echo "   ❌ tourne en ce moment :$CONCURRENTS"
    echo "   Arrête-le avant de continuer — un accès SMBus concurrent corrompt"
    echo "   une transaction (spec §6)."
    exit 1
fi
echo "   ✅ aucun logiciel RGB concurrent"

echo
echo "   Hubs SPD vus par le noyau (aucun trafic, simple lecture de sysfs) :"
ls -d /sys/bus/i2c/devices/*-005[0-3] 2>/dev/null | while read -r d; do
    printf '     %-10s driver=%s\n' "$(basename "$d")" \
        "$(basename "$(readlink -f "$d/driver" 2>/dev/null)")"
done

echo
echo "═══ 1. La règle udev ═══"
if [ -f /etc/udev/rules.d/60-reverb.rules ] \
   && cmp -s "$REGLES" /etc/udev/rules.d/60-reverb.rules; then
    echo "   ✅ règle installée et à jour"
else
    echo "   La règle du dépôt n'est pas installée, ou a changé. Installation :"
    sudo cp "$REGLES" /etc/udev/rules.d/ \
        && sudo udevadm control --reload \
        && sudo udevadm trigger \
        && echo "   ✅ installée"
    sleep 1
fi

BUS=$(ls -d /sys/class/i2c-dev/* 2>/dev/null | while read -r a; do
    [ "$(cat "$a/name" 2>/dev/null)" = "SMBus PIIX4 adapter port 0 at 0b00" ] \
        && echo "/dev/$(basename "$a")"
done)
echo "   Adaptateur : ${BUS:-introuvable}"
if [ -n "$BUS" ]; then
    getfacl -p "$BUS" 2>/dev/null | grep -E "^user:" | sed 's/^/     /'
    udevadm info -q all -n "$BUS" 2>/dev/null | grep -E "^E: (CURRENT_)?TAGS" | sed 's/^/     /'
fi

echo
echo "═══ 2. L'énumération, qui n'ouvre rien ═══"
"$REVERB" ram
pause "Les quatre barrettes et /dev/i2c-8 sont-ils listés ? (oui/non)" "enumeration"

echo
echo "═══ 3. Une couleur, les quatre barrettes ═══"
echo "   → magenta"
"$REVERB" ram --all --color ff00ff || echo "   ❌ refusé"
sleep 3
echo "   → vert"
"$REVERB" ram --all --color 00ff00 || echo "   ❌ refusé"
pause "Les QUATRE barrettes sont-elles vertes ? (oui / décrire ce qui diffère)" "couleur-toutes"

echo
echo "═══ 4. L'ordre des composantes ═══"
echo "La spec §4.1 dit RGB, sans permutation. Si Reverb se trompait d'ordre,"
echo "ces trois couleurs sortiraient permutées — et rien ne le signalerait."
for c in ff0000:ROUGE 00ff00:VERT 0000ff:BLEU; do
    echo "   → ${c#*:}"
    "$REVERB" ram --all --color "${c%%:*}" || echo "   ❌ refusé"
    sleep 3
done
pause "Rouge, puis vert, puis bleu — dans cet ordre ? (oui / décrire)" "ordre-rgb"

echo
echo "═══ 5. Une seule barrette ═══"
"$REVERB" ram --all --color 202020 >/dev/null || true
sleep 1
echo "   → barrette 2 en rouge, les autres en gris sombre"
"$REVERB" ram --slot 2 --color ff0000 || echo "   ❌ refusé"
pause "QUELLE barrette physique s'est allumée en rouge ? (1re/2e/3e/4e depuis le CPU)" "slot-2-physique"

echo
echo "═══ 6. Les onze LED, séparément ═══"
echo "Un dégradé rouge → vert sur la seule barrette 2. C'est ce qui prouve que"
echo "les onze LED sont adressables une par une (spec §4.1.1)."
"$REVERB" ram --slot 2 --colors \
    ff0000,ff4000,ff8000,ffc000,ffff00,c0ff00,80ff00,40ff00,00ff00,00ff40,00ff80 \
    || echo "   ❌ refusé"
pause "Un dégradé continu, ou onze zones identiques ? (dégradé / identiques / décrire)" "onze-led"

echo
echo "═══ 7. Pas de watchdog : la couleur survit à la commande ═══"
echo "Aucun processus Reverb ne tourne depuis l'étape 6. La spec §4.5 (test 1)"
echo "dit que l'état écrit tient indéfiniment. Vérification par l'attente."
for i in $(seq 20 -1 1); do printf "\r   %2d s sans le moindre octet sur le bus " "$i"; sleep 1; done
printf "\r                                              \r"
pause "Le dégradé est-il toujours là, inchangé ? (oui/non)" "sans-watchdog"

echo
echo "═══ 8. L'animation, calculée par l'hôte ═══"
echo "Une comète parcourt les 44 LED des quatre barrettes. Elle tourne 15 s"
echo "puis le script la tue — SANS lui laisser le temps de nettoyer."
echo
timeout 15 "$REVERB" ram --all --animate || true
pause "La comète a-t-elle circulé de barrette en barrette ? (oui / décrire)" "animation"

echo
echo "   La commande est morte. La RAM ne sachant pas animer seule (spec §4.5,"
echo "   test 3), l'éclairage doit être FIGÉ sur la dernière image reçue."
sleep 5
pause "Figé sur une image fixe, ou l'animation continue-t-elle ? (figé / continue)" "arret-anime"

echo
echo "═══ Relevé ═══"
cat "$NOTES"
echo
echo "Colle ce bloc dans la conversation."
