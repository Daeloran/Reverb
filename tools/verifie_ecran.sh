#!/usr/bin/env bash
# Dernière vérification de l'écran du Kraken (issue #13) : la dérive.
#
# ⚠️ REGARDE L'ÉCRAN DU KRAKEN pendant toute la durée.
#
# Tout le reste est acquis : la mire s'affiche, et ses quadrants sont à leur
# place — rouge en haut à gauche, ce qui confirme l'ordre BGR et clôt la
# question ouverte n° 2 de la spec.
#
# Ce qui reste : le §2.2.1 signale qu'à l'envoi RÉPÉTÉ, l'implémentation
# Windows voyait l'image se décaler « comme une grille qui défile ». La cause
# supposée était le paquet de longueur nulle, que Reverb émet maintenant au bon
# endroit. Reste à le vérifier.
#
# Usage : ./tools/verifie_ecran.sh

set -u

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
REVERB="$RACINE/target/debug/reverb"
NOTES="/tmp/reverb-observations-ecran.txt"

[ -x "$REVERB" ] || { echo "Compile d'abord : cargo build" >&2; exit 1; }
: >"$NOTES"

pause() { echo; read -r -p "   ↳ $1 " r; printf '%s\t%s\n' "$2" "$r" >>"$NOTES"; }

echo "═══ 1. Vingt envois d'affilée ═══"
echo "La mire doit rester IMMOBILE. Surveille surtout la frontière entre les"
echo "quadrants : une dérive d'un pixel par envoi s'y verrait avant tout."
echo
for i in $(seq 1 20); do
    printf "\r   envoi %2d/20" "$i"
    "$REVERB" screen --mire --once >/dev/null 2>&1 || { echo; echo "   ❌ envoi $i refusé"; break; }
done
echo
sleep 2
pause "L'image a-t-elle bougé, glissé ou défilé ? (stable / décrire)" "derive"

echo
echo "═══ 2. Le mode boucle, tel qu'on l'utilisera vraiment ═══"
echo "La commande tourne et réémet toute les 25 s. L'écran ne doit JAMAIS"
echo "revenir à l'affichage NZXT tant qu'elle tourne."
echo "Elle s'arrête seule au bout de 70 s — assez pour couvrir deux réémissions."
echo
timeout 70 "$REVERB" screen --mire || true
echo
pause "L'écran est-il resté sur la mire tout du long ? (oui / décrire)" "boucle"

echo
echo "═══ 3. Le retour au firmware ═══"
echo "Plus rien n'émet. L'écran doit revenir à « NZXT — xx° Liquid »."
for i in $(seq 40 -1 1); do printf "\r   %2d s " "$i"; sleep 1; done
printf "\r         \r"
pause "L'écran est-il revenu à l'affichage NZXT ? (oui/non)" "repli-firmware"

echo
echo "═══ Relevé ═══"
cat "$NOTES"
echo
echo "Fichier : $NOTES"
