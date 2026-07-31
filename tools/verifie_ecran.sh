#!/usr/bin/env bash
# Vérification matérielle de l'écran du Kraken (issue #13), seconde passe.
#
# ⚠️ REGARDE L'ÉCRAN DU KRAKEN. Seul l'œil répond à ces questions.
#
# La première passe avait échoué : aucune image, et l'affichage firmware
# dégradé. Cause trouvée à la sonde — un paquet de longueur nulle était émis
# APRÈS L'EN-TÊTE de 20 octets, qui n'en a pas besoin. Le contrôleur recevait
# donc un transfert vide parasite entre l'en-tête et l'image.
#
# Ce script teste deux hypothèses en un passage :
#   A. la correction du paquet vide suffit ;
#   B. il faut en plus rejouer le préambule complet de CAM.
#
# Aucun sudo. Usage : ./tools/verifie_ecran.sh

set -u

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
REVERB="$RACINE/target/debug/reverb"
NOTES="/tmp/reverb-observations-ecran.txt"

[ -x "$REVERB" ] || { echo "Compile d'abord : cargo build" >&2; exit 1; }
: >"$NOTES"

pause() { echo; read -r -p "   ↳ $1 " r; printf '%s\t%s\n' "$2" "$r" >>"$NOTES"; }

attendre_repli() {
    echo "   (on laisse le firmware reprendre la main — 35 s)"
    for i in $(seq 35 -1 1); do printf "\r      %2d s " "$i"; sleep 1; done
    printf "\r              \r"
}

echo "═══ A. Correction du paquet vide, seule ═══"
echo "Quatre quadrants attendus :"
echo "     haut-gauche ROUGE      haut-droite VERT"
echo "     bas-gauche  BLEU       bas-droite  BLANC"
"$REVERB" screen --mire --once || echo "  ❌ envoi refusé"
sleep 4
pause "Que montre l'écran ? (mire / température / autre — décris)" "A-paquet-vide-corrige"

attendre_repli

echo "═══ B. Avec le préambule complet de CAM ═══"
"$REVERB" screen --mire --once --full-init || echo "  ❌ envoi refusé"
sleep 4
pause "Et maintenant ? (mire / température / autre — décris)" "B-preambule-complet"

# Les étapes suivantes n'ont de sens que si une mire est apparue.
echo
read -r -p "Une mire est-elle apparue à l'étape A ou B ? (A/B/aucune) " QUELLE
printf 'mire-apparue\t%s\n' "$QUELLE" >>"$NOTES"

case "$QUELLE" in
    A|a) OPTS="--once" ;;
    B|b) OPTS="--once --full-init" ;;
    *)
        echo
        echo "Pas de mire : on s'arrête là, le reste n'aurait rien à mesurer."
        echo "Relevé : $NOTES"; cat "$NOTES"; exit 0
        ;;
esac

echo
echo "═══ C. Ordre des composantes ═══"
# shellcheck disable=SC2086
"$REVERB" screen --mire $OPTS >/dev/null 2>&1
sleep 3
pause "Couleur du quadrant HAUT-GAUCHE ? (rouge/bleu/vert/blanc)" "ordre-composantes"

echo
echo "═══ D. Dérive — le test du paquet de longueur nulle ═══"
echo "Dix envois d'affilée. Si l'image glisse ou défile comme une grille,"
echo "le paquet vide ne suffit pas."
for i in $(seq 1 10); do
    printf "\r   envoi %2d/10" "$i"
    # shellcheck disable=SC2086
    "$REVERB" screen --mire $OPTS >/dev/null 2>&1 || { echo; echo "  ❌ envoi $i refusé"; break; }
done
echo
sleep 2
pause "L'image est-elle restée STABLE sur les dix envois ? (oui/non/décrire)" "derive"

echo
echo "═══ E. Repli du firmware ═══"
attendre_repli
pause "L'écran est-il revenu à « NZXT — xx° Liquid » ? (oui/non)" "repli-firmware"

echo
echo "═══ Relevé ═══"
cat "$NOTES"
echo
echo "Fichier : $NOTES"
