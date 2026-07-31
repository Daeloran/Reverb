#!/usr/bin/env bash
# Vérification matérielle de l'écran du Kraken (issue #13), troisième passe.
#
# ⚠️ REGARDE L'ÉCRAN DU KRAKEN. Seul l'œil répond.
#
# Historique des deux passes précédentes :
#
#   1. aucune image, affichage firmware dégradé. Cause : un paquet de longueur
#      nulle était émis après l'en-tête de 20 octets, qui n'en a pas besoin.
#      Corrigé.
#   2. toujours aucune image, affichage firmware propre. Deux défauts trouvés
#      depuis, tous deux visibles dans la capture depuis le début :
#        - CAM attend l'accusé 37 01 avant d'envoyer les données ; on
#          enchaînait sans rien attendre. Corrigé ;
#        - « 38 01 02 » est peut-être le mode LIQUIDE et non le mode image.
#          liquidctl le nomme ainsi, et l'écran nous montre effectivement le
#          liquide. C'est ce que cette passe teste.
#
# Six variantes, de la plus proche de la capture à la plus proche de liquidctl.
# La première qui affiche la mire tranche.
#
# Aucun sudo. Usage : ./tools/verifie_ecran.sh

set -u

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
REVERB="$RACINE/target/debug/reverb"
NOTES="/tmp/reverb-observations-ecran.txt"

[ -x "$REVERB" ] || { echo "Compile d'abord : cargo build" >&2; exit 1; }
: >"$NOTES"

echo "Mire attendue :   haut-gauche ROUGE    haut-droite VERT"
echo "                  bas-gauche  BLEU     bas-droite  BLANC"
echo
echo "À chaque essai, réponds par ce que tu vois. « rien » si l'écran ne bouge"
echo "pas de son affichage NZXT habituel."
echo

essai() {
    local libelle="$1"; shift
    echo "─── $libelle"
    echo "    $ reverb screen --mire --once $*"
    # shellcheck disable=SC2086
    if ! "$REVERB" screen --mire --once "$@" 2>&1 | sed 's/^/    /'; then
        printf '%s\tCOMMANDE EN ECHEC\n' "$libelle" >>"$NOTES"
        return
    fi
    sleep 4
    read -r -p "    ↳ que montre l'écran ? " reponse
    printf '%s\t%s\n' "$libelle" "$reponse" >>"$NOTES"
    echo
}

essai "1-capture-fidele"
essai "2-mode-4-apres"            --after-mode 4
essai "3-preambule"               --full-init
essai "4-preambule-et-mode-4"     --full-init --after-mode 4
essai "5-mode-1-apres"            --after-mode 1
essai "6-mode-0-apres"            --after-mode 0

echo "═══ Si une mire est apparue ═══"
read -r -p "Quel numéro d'essai a affiché la mire ? (1-6, ou « aucun ») " GAGNANT
printf 'essai-gagnant\t%s\n' "$GAGNANT" >>"$NOTES"

if [ "$GAGNANT" != "aucun" ] && [ -n "$GAGNANT" ]; then
    read -r -p "Couleur du quadrant HAUT-GAUCHE ? (rouge/bleu/vert/blanc) " COIN
    printf 'ordre-composantes\t%s\n' "$COIN" >>"$NOTES"
fi

echo
echo "═══ Relevé ═══"
cat "$NOTES"
echo
echo "Fichier : $NOTES"
