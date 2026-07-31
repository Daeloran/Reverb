#!/usr/bin/env bash
# Vérification du moteur d'animations (issue #19).
#
# Ce que ce script vérifie et que je n'ai PAS pu vérifier moi-même : tout ce qui
# demande root ou un œil. Le reste — géométrie, catalogue, réglages, protocole —
# est couvert par 385 tests qui tournent sans matériel.
#
# ⚠️ REGARDE LE BOÎTIER aux étapes 4 et 5. Le reste défile tout seul.
#
# Usage : ./tools/verifie_animations.sh

set -u

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
SOCKET=/run/reverb/reverbd.sock
NOTES="/tmp/reverb-observations-animations.txt"

: >"$NOTES"
pause() { echo; read -r -p "   ↳ $1 " r; printf '%s\t%s\n' "$2" "$r" >>"$NOTES"; }

dis() {
    python3 -c "
import socket, sys
try:
    s = socket.socket(socket.AF_UNIX)
    s.connect('$SOCKET')
except OSError as e:
    print(f'    ❌ socket injoignable : {e.strerror}')
    sys.exit(1)
s.sendall((sys.argv[1] + '\n').encode())
for l in s.makefile('r'):
    print('    ' + l.rstrip())
    if l.startswith(('end', 'err')): break
" "$1"
}

echo "═══ 1. Installation ═══"
cd "$RACINE" || exit 1
cargo build --release 2>&1 | tail -1
sudo install -m 0755 target/release/reverbd /usr/local/bin/ \
    && sudo systemctl restart reverbd \
    && echo "   ✅ installé, service redémarré"
sleep 2

if ! id -nG | grep -qw reverb; then
    echo "   Le groupe reverb n'est pas actif dans cette session, relance sous « sg »…"
    exec sg reverb "$0"
fi

echo
echo "═══ 2. La géométrie mesurée, telle que le démon la porte ═══"
dis "geometry"
echo "   Attendu : dix lignes. « bas-droite » à 210°, ses voisins à 300° —"
echo "   c'est le quart de tour que la mesure a trouvé."
pause "Les dix lignes sont-elles là, et bas-droite diffère-t-il de ses voisins ? (oui/non)" "geometrie-lue"

echo
echo "═══ 3. Elle se corrige, et elle survit au redémarrage ═══"
echo "   On met « arriere » à 123° antihoraire, puis on redémarre le démon."
dis "geometry arriere angle=123 sens=antihoraire"
sudo systemctl restart reverbd
sleep 2
echo "   Après redémarrage :"
dis "geometry arriere"
echo "   Attendu : geom arriere 123 antihoraire"
echo
echo "   Le fichier écrit par le démon :"
sudo grep -v '^#' /etc/reverb/geometrie.conf 2>/dev/null | grep -v '^$' | sed 's/^/     /'
pause "L'orientation a-t-elle survécu au redémarrage ? (oui/non)" "persistance"

# Remise en état immédiate : la suite doit tourner sur la vraie géométrie.
dis "geometry arriere angle=300 sens=horaire" >/dev/null

echo
echo "═══ 4. Le catalogue, six familles ═══"
echo "   Chacune tourne 8 secondes. REGARDE LE BOÎTIER."
echo
for a in vague comete respiration arc-en-ciel balayage braise; do
    printf '   → %-14s' "$a"
    dis "animate $a" >/dev/null
    sleep 8
    # Ce que le démon dit de lui-même pendant ces huit secondes.
    journalctl -u reverbd --since "9 seconds ago" --no-pager -o cat 2>/dev/null \
        | grep "img/s" | tail -1 | sed 's/^/  /'
    echo
done
dis "animate off" >/dev/null

pause "Les six sont-elles fluides, ou certaines saccadent-elles ? (décrire)" "fluidite"
pause "Lesquelles t'ont plu ? (libre)" "gout"

echo
echo "═══ 5. La direction change vraiment le mouvement ═══"
echo "   La même vague, deux directions. La première MONTE, la seconde va de"
echo "   l'AVANT vers l'ARRIÈRE."
echo
echo "   → bas-haut (8 s)"
dis "animate vague direction=bas-haut" >/dev/null
sleep 8
echo "   → avant-arriere (8 s)"
dis "animate vague direction=avant-arriere" >/dev/null
sleep 8
dis "animate off" >/dev/null
pause "Le mouvement a-t-il changé de sens, et dans le bon ? (oui / décrire)" "direction"

echo
echo "   Et la couleur, sur la même animation :"
dis "animate vague couleur=00ff40" >/dev/null
sleep 5
dis "animate off" >/dev/null
pause "Était-ce bien vert ? (oui/non)" "couleur"

echo
echo "═══ 6. Ce qui est refusé l'est en le disant ═══"
for mauvais in \
    "animate bidule" \
    "animate vague bidule=3" \
    "animate vague couleur=zzzzzz" \
    "animate vague vitesse=99" \
    "animate arc-en-ciel couleur=ff00ff" \
    "geometry radiateur-haut angle=400" \
    "geometry angle=90"; do
    echo "   « $mauvais »"
    dis "$mauvais"
done
pause "Chaque refus dit-il ce qui cloche et ce qui est accepté ? (oui / décrire)" "refus"

echo
echo "═══ 7. Retour au repos ═══"
dis "light all 000000" >/dev/null
echo "   Éteint."

echo
echo "═══ Relevé ═══"
cat "$NOTES"
echo
echo "Colle ce bloc dans la conversation."
echo
echo "Rappel : l'éclairage au démarrage de la machine n'a toujours pas été"
echo "constaté — le démon n'a jamais encore été présent au boot."
