#!/usr/bin/env bash
# Installation et vérification du démon (issue #17).
#
# Ce que ce script vérifie et que je n'ai PAS pu vérifier moi-même : tout ce qui
# demande root. Le reste — cadence, protocole, descripteurs tenus, télémétrie —
# a déjà été mesuré en session utilisateur sur un socket d'essai.
#
# ⚠️ REGARDE LES VENTILATEURS ET LA RAM aux étapes 5 et 6.
#
# Usage : ./tools/verifie_demon.sh

set -u

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
SOCKET=/run/reverb/reverbd.sock
NOTES="/tmp/reverb-observations-demon.txt"

: >"$NOTES"
pause() { echo; read -r -p "   ↳ $1 " r; printf '%s\t%s\n' "$2" "$r" >>"$NOTES"; }

# Un client minimal : envoie une ligne, lit jusqu'à la ligne terminale.
dis() {
    python3 -c "
import socket, sys
s = socket.socket(socket.AF_UNIX)
s.connect('$SOCKET')
s.sendall((sys.argv[1] + '\n').encode())
for l in s.makefile('r'):
    print('    ' + l.rstrip())
    if l.startswith(('end', 'err')): break
" "$1"
}

echo "═══ 1. Installation ═══"
cd "$RACINE" || exit 1
cargo build --release 2>&1 | tail -1

if ! getent group reverb >/dev/null; then
    echo "   Création du groupe reverb…"
    sudo groupadd -f reverb && sudo usermod -aG reverb "$USER"
    echo "   ⚠️ Groupe créé. Il faut te DÉCONNECTER/RECONNECTER pour qu'il prenne."
    echo "      Le script continue : systemd n'a pas besoin d'attendre, mais les"
    echo "      étapes qui parlent au socket échoueront tant que tu n'as pas"
    echo "      rouvert ta session. Relance-le après."
fi

sudo install -m 0755 target/release/reverbd /usr/local/bin/ \
    && sudo install -m 0644 packaging/reverbd.service /etc/systemd/system/ \
    && sudo systemctl daemon-reload \
    && sudo systemctl enable --now reverbd \
    && echo "   ✅ installé et démarré"
sleep 2

echo
echo "═══ 2. Le service tourne, et systemd l'a attendu ═══"
systemctl is-active reverbd | sed 's/^/   état : /'
systemctl show reverbd -p NRestarts --value | sed 's/^/   redémarrages : /'
echo "   Journal :"
journalctl -u reverbd -n 5 --no-pager -o cat | sed 's/^/     /'

echo
echo "═══ 3. Les descripteurs sont tenus, et il y en a quatre ═══"
PID=$(systemctl show reverbd -p MainPID --value)
echo "   pid $PID"
sudo ls -l "/proc/$PID/fd" 2>/dev/null | grep -E "hidraw|i2c" | awk '{print "     "$9" -> "$11}'
echo "   (trois hidraw + un i2c attendus, et ils ne doivent JAMAIS changer)"

echo
echo "═══ 4. Le socket, ses droits, et la télémétrie ═══"
ls -l "$SOCKET" | sed 's/^/   /'
echo "   Attendu : srw-rw---- root reverb"
echo
echo "   status :"
dis status || echo "   ❌ socket injoignable — as-tu rouvert ta session depuis le groupadd ?"
pause "Les canaux et la température du liquide sont-ils là ? (oui/non)" "telemetrie"

echo
echo "═══ 5. L'éclairage, sans aucune fenêtre ouverte ═══"
for c in ff0000:ROUGE 00ff00:VERT 2040ff:BLEU; do
    echo "   → ${c#*:}"
    dis "light all ${c%%:*}" >/dev/null
    sleep 2
done
pause "Ventilateurs ET barrettes ont-ils suivi les trois couleurs ? (oui / décrire)" "eclairage"

echo
echo "═══ 6. L'animation, à travers tout le boîtier ═══"
echo "   Une comète part du bas, remonte l'avant, passe sur le dessus, finit à"
echo "   l'arrière, puis traverse les quatre barrettes. 12 secondes."
dis "animate vague" >/dev/null
sleep 12
dis "animate off" >/dev/null
pause "Fluide, ou saccadée ? (fluide / saccadée / décrire)" "fluidite"
pause "La comète passe-t-elle bien des ventilateurs à la RAM ? (oui / décrire)" "continuite"

echo
echo "   Ce que le démon dit de lui-même :"
journalctl -u reverbd --since "1 minute ago" --no-pager -o cat | grep "img/s" | sed 's/^/     /'
echo "   (« 0 sautée » = la cadence est tenue)"

echo
echo "═══ 7. Le réglage des ventilateurs, sans sudo côté client ═══"
dis "fan nzxtsmart2:fan-1 pwm 70"
sleep 4
dis status | grep "fan-1" || true
pause "Le canal fan-1 est-il monté à 70 % ? (oui/non)" "consigne-pwm"
dis "fan nzxtsmart2:fan-1 pwm 25" >/dev/null

echo
echo "═══ 8. La ligne de commande cède le pas ═══"
"$RACINE/target/release/reverb" set --all --color ff00ff 2>&1 | head -5 | sed 's/^/   /'
echo "   ↑ doit REFUSER et expliquer"
echo
echo "   Et ce qui ne touche pas aux bus du démon doit marcher :"
"$RACINE/target/release/reverb" screen 2>&1 | head -4 | sed 's/^/   /'
pause "Le premier refuse-t-il et le second répond-il ? (oui / décrire)" "cede-le-pas"

echo
echo "═══ 9. L'éclairage survit à l'arrêt du démon ═══"
dis "light all 00ffcc" >/dev/null
sleep 2
sudo systemctl stop reverbd
sleep 3
pause "Le cyan est-il toujours là, démon arrêté ? (oui/non)" "survit-a-l-arret"

sudo systemctl start reverbd
sleep 2
echo "   Démon redémarré."

echo
echo "═══ Relevé ═══"
cat "$NOTES"
echo
echo "Colle ce bloc dans la conversation."
echo
echo "Reste à vérifier au prochain démarrage de la machine : l'éclairage doit"
echo "être appliqué au boot, sans que tu ouvres quoi que ce soit."
