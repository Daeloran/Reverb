#!/usr/bin/env bash
# Vérification de l'écran du Kraken tenu par le démon (issue #33).
#
# ⚠️ REGARDE L'ÉCRAN DU KRAKEN pendant toute la durée.
#
# Ce que ce script vérifie et que je n'ai PAS pu vérifier moi-même : ce qui
# demande une dalle branchée et un œil. Le reste — protocole, fichier d'état,
# mise à l'échelle, cadence — est couvert par 32 tests qui tournent sans
# matériel.
#
# Le démon doit tourner : c'est lui qui tient l'écran désormais.
#
# Usage : ./tools/verifie_ecran_demon.sh

set -u

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
SOCKET="/run/reverb/reverbd.sock"
NOTES="/tmp/reverb-observations-ecran-demon.txt"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

: >"$NOTES"
pause() { echo; read -r -p "   ↳ $1 " r; printf '%s\t%s\n' "$2" "$r" >>"$NOTES"; }
dis() { echo "$1" | socat - "UNIX-CONNECT:$SOCKET"; }

command -v socat >/dev/null || { echo "socat manquant." >&2; exit 1; }
[ -S "$SOCKET" ] || { echo "Le démon ne tourne pas : sudo systemctl start reverbd" >&2; exit 1; }

echo "═══ 0. L'état de départ ═══"
dis "screen state"
pause "Que dit l'état ci-dessus ? (recopier)" "etat-depart"

echo
echo "═══ 1. La luminosité ═══"
for niveau in 100 40 0 80; do
    echo "   → $niveau %"
    dis "screen brightness $niveau" >/dev/null
    sleep 2
done
pause "La dalle a-t-elle suivi les quatre niveaux, 0 % l'éteignant ? (oui/décrire)" "luminosite"
pause "Ce qu'elle affichait est-il revenu après le retour à 80 % ? (oui/non)" "luminosite-contenu"

echo
echo "═══ 2. Le cadran ═══"
SONDE="$(dis "status" | awk '/^temp /{print $2; exit}')"
if [ -z "$SONDE" ]; then
    echo "   ❌ aucune sonde rapportée par « status »"
else
    echo "   sonde retenue : $SONDE"
    dis "screen gauge $SONDE"
    echo "   Regarde une trentaine de secondes : le chiffre doit CHANGER."
    sleep 30
fi
pause "Le cadran est-il lisible à un mètre — chiffre, unité, anneau ? (oui/décrire)" "cadran"
pause "La valeur a-t-elle bougé pendant les 30 s ? (oui/non)" "cadran-vivant"
echo "   Une sonde inconnue doit être REFUSÉE, sans toucher la dalle :"
dis "screen gauge bidule:inconnue"
pause "Le refus nomme-t-il la sonde, et la dalle est-elle restée sur son cadran ? (oui/décrire)" "cadran-refus"

echo
echo "═══ 3. Une image ═══"
echo "   Colle le chemin ABSOLU d'un PNG ou d'un JPEG (vide pour sauter) :"
read -r IMAGE
if [ -n "$IMAGE" ]; then
    dis "screen image $IMAGE"
    echo "   Elle doit tenir SANS DISPARAÎTRE : le démon la réémet toutes les 25 s."
    sleep 40
    pause "L'image est-elle bien centrée, sans déformation, et toujours là après 40 s ? (oui/décrire)" "image"
fi
echo "   Un fichier qui n'est pas une image doit être REFUSÉ sans rien changer :"
echo "pas une image" >"$TMP/faux.png"
dis "screen image $TMP/faux.png"
pause "Le refus nomme-t-il le fichier, et la dalle n'a-t-elle pas changé ? (oui/décrire)" "image-refus"
echo "   Un chemin relatif doit être refusé EN LE DISANT :"
dis "screen image fond.png"
pause "Le refus parle-t-il de chemin absolu ? (oui/décrire)" "image-relatif"

echo
echo "═══ 4. Un GIF ═══"
echo "   Colle le chemin ABSOLU d'un GIF animé (vide pour sauter) :"
read -r GIF
if [ -n "$GIF" ]; then
    dis "screen gif $GIF"
    sleep 30
    pause "L'animation tourne-t-elle en boucle, complète, sans images sautées ? (oui/décrire)" "gif"
fi

echo
echo "═══ 5. Ce que la dalle retrouve au redémarrage ═══"
dis "screen state"
echo "   Le démon va redémarrer. La dalle doit retrouver CE QU'ELLE MONTRE."
sudo systemctl restart reverbd
sleep 5
dis "screen state"
pause "La dalle a-t-elle retrouvé son affichage et sa luminosité ? (oui/décrire)" "redemarrage"

echo
echo "═══ 6. L'extinction ═══"
dis "screen off"
echo "   Le firmware doit reprendre la main dans une trentaine de secondes."
sleep 35
pause "La dalle est-elle revenue à son affichage d'origine ? (oui/décrire)" "extinction"

echo
echo "   Observations : $NOTES"
cat "$NOTES"
