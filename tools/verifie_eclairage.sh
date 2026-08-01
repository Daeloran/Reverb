#!/usr/bin/env bash
# Vérification de l'éclairage retrouvé au démarrage (issue #21).
#
# Ce que ce script vérifie et que je n'ai PAS pu vérifier moi-même : tout ce qui
# demande root ou un œil. Le reste — encodage, décodage, refus, aller-retour par
# le disque — est couvert par 36 tests d'intention qui tournent sans matériel.
#
# ⚠️ POURQUOI LE BOÎTIER EST ÉTEINT À CHAQUE ÉTAPE.
# « La couleur survit à un redémarrage » ne se voit pas si le boîtier la garde
# tout seul : les contrôleurs n'ont aucun watchdog, la couleur reste allumée
# même démon arrêté. Un test qui redémarrerait le service et constaterait « c'est
# toujours vert » ne prouverait donc RIEN — c'est exactement le genre de critère
# creux qui a fait naître cette issue.
# D'où la manœuvre : on arrête le démon, on éteint le boîtier à la main avec
# « reverb » (qui n'a le droit d'écrire que quand le démon dort), puis on
# redémarre. Si le boîtier se rallume seul, il n'y a qu'une explication.
#
# ⚠️ REGARDE LE BOÎTIER aux étapes 2, 4, 5, 6, 7 et 8. Le script s'arrête et
# pose la question à chaque fois.
#
# Le mot de passe sudo est demandé une fois, à l'étape 1.
#
# Usage : ./tools/verifie_eclairage.sh

set -u

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
SOCKET=/run/reverb/reverbd.sock
FICHIER=/var/lib/reverb/eclairage.conf
REVERB="$RACINE/target/release/reverb"
NOTES="/tmp/reverb-observations-eclairage.txt"

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
" "$1" | grep -v '^    end$'
}

# Éteint le boîtier SANS le démon : c'est ce qui rend la reprise observable.
eteins_a_la_main() {
    sudo systemctl stop reverbd
    sleep 1
    sudo "$REVERB" set --all --color 000000 2>&1 | sed 's/^/    /'
    sudo "$REVERB" ram --all --color 000000 2>&1 | sed 's/^/    /'
}

rallume_le_demon() {
    sudo systemctl start reverbd
    sleep 2
}

echo "═══ 1. Installation ═══"
cd "$RACINE" || exit 1
cargo build --release 2>&1 | tail -1
sudo install -m 0755 target/release/reverbd /usr/local/bin/ || exit 1
sudo install -m 0644 packaging/reverbd.service /etc/systemd/system/ || exit 1
sudo systemctl daemon-reload
sudo systemctl restart reverbd && echo "   ✅ installé, service redémarré"
sleep 2
echo "   Le boîtier a pu s'allumer en BLEU à l'instant : c'est l'accueil, le"
echo "   fichier d'état n'existe pas encore. L'étape 7 le vérifiera pour de bon."

if ! id -nG | grep -qw reverb; then
    echo "   Le groupe reverb n'est pas actif dans cette session, relance sous « sg »…"
    exec sg reverb "$0"
fi

echo
echo "═══ 2. Preuve de vie — le boîtier s'allume MAINTENANT ═══"
echo "   Trois couleurs, deux secondes chacune. Si rien ne s'allume ici, tout ce"
echo "   qui suit est sans objet : arrête le script et dis-le."
for c in ff0000:ROUGE 00ff00:VERT 0040ff:BLEU; do
    printf '   → %s\n' "${c#*:}"
    dis "light all ${c%%:*}"
    sleep 2
done
pause "Le boîtier s'est-il allumé en rouge, vert, bleu ? (oui/non)" "preuve-de-vie"

echo
echo "═══ 3. Le fichier d'état, tel que le démon vient de l'écrire ═══"
sudo cat "$FICHIER" | sed 's/^/    /'
echo "   Attendu : dix lignes « ventilateur », quatre « barrette », toutes en 0040ff"
echo "   (le dernier « light » de l'étape 2), et aucune ligne « animation »."
pause "Le fichier est-il là, complet, et en 0040ff ? (oui/non)" "fichier-ecrit"

echo
echo "═══ 4. Une couleur fixe est retrouvée au redémarrage du service ═══"
echo "   On pose du VERT, on éteint le boîtier démon arrêté, puis on redémarre."
dis "light all 00ff40"
sleep 2
echo "   → on arrête le démon et on éteint le boîtier à la main"
eteins_a_la_main
pause "Le boîtier est-il bien ÉTEINT là, maintenant ? (oui/non)" "extinction-manuelle"
echo "   → on redémarre le démon, sans lui envoyer la moindre commande"
rallume_le_demon
pause "S'est-il rallumé en VERT tout seul ? (oui/non)" "couleur-retrouvee"

echo
echo "═══ 5. Une animation reprend, réglages compris ═══"
echo "   Comète violette, vitesse 5. Même manœuvre."
dis "animate comete couleur=ff00ff vitesse=5 direction=horaire"
sleep 4
echo "   → on arrête le démon et on éteint le boîtier à la main"
eteins_a_la_main
sleep 1
echo "   → on redémarre le démon"
rallume_le_demon
sleep 4
pause "La comète violette est-elle repartie toute seule, à la même allure ? (oui/décrire)" "animation-reprise"

echo
echo "═══ 6. Éteindre volontairement, c'est un état comme un autre ═══"
echo "   C'est le point le plus subtil de l'issue : un boîtier réglé sur NOIR doit"
echo "   rester NOIR au démarrage suivant. S'il se rallumait en bleu, éteindre son"
echo "   boîtier serait devenu impossible."
dis "animate off"
dis "light all 000000"
sleep 1
echo "   → redémarrage du service"
sudo systemctl restart reverbd
sleep 3
pause "Le boîtier est-il resté ÉTEINT (et surtout PAS bleu) ? (oui/non)" "noir-reste-noir"

echo
echo "═══ 7. Premier démarrage : le fichier absent allume en bleu ═══"
echo "   On efface le fichier d'état — c'est l'état d'une installation neuve."
sudo rm -f "$FICHIER"
sudo systemctl restart reverbd
sleep 3
pause "Le boîtier s'est-il allumé en BLEU PUR, ventilateurs ET barrettes ? (oui/décrire)" "accueil-bleu"

echo
echo "═══ 8. Un fichier de travers ne bloque pas le démarrage ═══"
echo "   On écrit n'importe quoi dans le fichier, et on redémarre."
sudo sh -c "printf 'ceci nest pas un eclairage\n' > $FICHIER"
sudo systemctl restart reverbd
sleep 3
echo "   Ce que le démon en a dit :"
journalctl -u reverbd --since "10 seconds ago" --no-pager -o cat 2>/dev/null \
    | grep -i "éclairage\|eclairage" | tail -3 | sed 's/^/    /'
systemctl is-active reverbd | sed 's/^/    service : /'
pause "Le service est-il actif, le boîtier bleu, et le journal dit-il ce qui cloche ? (oui/décrire)" "fichier-abime"

echo
echo "═══ 9. Remise en état ═══"
dis "animate comete couleur=ff00ff vitesse=5 direction=horaire"
echo "   Le boîtier repart sur la comète violette."
echo
echo "   ⚠️ IL RESTE UN CRITÈRE, ET IL DEMANDE UN VRAI REDÉMARRAGE MACHINE."
echo "   Quand tu voudras : redémarre SHYNAEL, et regarde le boîtier au retour"
echo "   sur le bureau — la comète violette doit être repartie seule, sans avoir"
echo "   rien tapé. C'est le critère d'origine de l'issue, celui que le"
echo "   redémarrage du service ne remplace pas tout à fait."
echo
echo "   Observations : $NOTES"
cat "$NOTES"
