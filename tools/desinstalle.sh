#!/usr/bin/env bash
# Retire Reverb — tout ce qu'`installe.sh` a posé, et rien d'autre.
#
# ⚠️ Ce qui RESTE, délibérément :
#   /etc/reverb/geometrie.conf   l'orientation des dix ventilateurs, relevée à
#                                l'œil sous le bureau. Une réinstallation la
#                                retrouve ; l'effacer coûterait un second relevé.
#   /var/lib/reverb/             l'éclairage courant.
#   le groupe reverb             d'autres comptes peuvent en être membres.
# Les trois s'enlèvent à la main, et le script dit comment.
#
# Usage : ./tools/desinstalle.sh

set -eu

BINAIRES=/usr/local/bin

etape() { printf '\n\033[1m══ %s\033[0m\n' "$1"; }
retire() { printf '   %s\n' "$1"; }

etape "1. Le service"
if systemctl list-unit-files reverbd.service >/dev/null 2>&1; then
    # Arrêter avant de désinstaller : un démon qui tourne encore garderait ses
    # descripteurs, et le binaire effacé continuerait d'écrire sur les bus.
    sudo systemctl disable --now reverbd 2>/dev/null || true
    sudo rm -f /etc/systemd/system/reverbd.service
    sudo systemctl daemon-reload
    retire "reverbd.service arrêté et retiré"
else
    retire "reverbd.service : absent"
fi

etape "2. Le lanceur"
sudo rm -f /usr/local/share/applications/reverb.desktop
sudo rm -f /usr/local/share/icons/hicolor/scalable/apps/reverb.svg
sudo update-desktop-database /usr/local/share/applications 2>/dev/null || true
retire "entrée de menu et icône"

etape "3. Les binaires"
for binaire in reverbd reverb reverb-gui; do
    sudo rm -f "$BINAIRES/$binaire"
    retire "$BINAIRES/$binaire"
done

etape "4. La règle udev"
sudo rm -f /etc/udev/rules.d/60-reverb.rules
sudo udevadm control --reload
retire "60-reverb.rules"

echo
echo "⚠️  Le boîtier garde la dernière couleur reçue : aucun contrôleur n'a de"
echo "   watchdog, et plus rien ne lui parle. Elle tiendra jusqu'à la coupure."
echo
echo "Ce qui reste, exprès :"
echo "   /etc/reverb/       la géométrie relevée à l'œil — sudo rm -r /etc/reverb"
echo "   /var/lib/reverb/   l'éclairage courant       — sudo rm -r /var/lib/reverb"
echo "   groupe reverb                                — sudo groupdel reverb"
