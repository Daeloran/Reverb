#!/usr/bin/env bash
# Installe Reverb — le démon, la fenêtre, l'outil, et de quoi les lancer.
#
# Rejouable : une seconde exécution met à jour ce qui a changé sans rien casser.
# Elle ne touche jamais /etc/reverb ni /var/lib/reverb — la géométrie relevée à
# l'œil et l'éclairage courant lui survivent.
#
# Usage : ./tools/installe.sh

set -eu

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
BINAIRES=/usr/local/bin
SOCKET=/run/reverb/reverbd.sock

etape() { printf '\n\033[1m══ %s\033[0m\n' "$1"; }
pose() { printf '   %s → %s\n' "$1" "$2"; }

cd "$RACINE"

etape "1. Construction"
cargo build --release
pose "reverbd, reverb, reverb-gui" "target/release/"

etape "2. Le groupe reverb"
# Le socket naît en root:reverb : sans ce groupe, la fenêtre et l'outil se
# heurtent à une porte close, et le message ne dit pas pourquoi.
if getent group reverb >/dev/null; then
    pose "groupe reverb" "déjà là"
else
    sudo groupadd reverb
    pose "groupe reverb" "créé"
fi
if id -nG "$USER" | grep -qw reverb; then
    pose "$USER" "déjà membre"
    GROUPE_ACTIF=1
else
    sudo usermod -aG reverb "$USER"
    pose "$USER" "ajouté au groupe"
    GROUPE_ACTIF=0
fi

etape "3. La règle udev"
# `uaccess` confie l'accès à l'utilisateur physiquement connecté, et le retire à
# la déconnexion — là où un GROUP= l'ouvrirait en permanence.
sudo install -m 0644 packaging/60-reverb.rules /etc/udev/rules.d/
sudo udevadm control --reload
sudo udevadm trigger
pose "60-reverb.rules" "/etc/udev/rules.d/"

etape "4. Les binaires"
for binaire in reverbd reverb reverb-gui; do
    sudo install -m 0755 "target/release/$binaire" "$BINAIRES/"
    pose "$binaire" "$BINAIRES/$binaire"
done

etape "5. Le service"
sudo install -m 0644 packaging/reverbd.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable reverbd >/dev/null 2>&1
sudo systemctl restart reverbd
pose "reverbd.service" "actif au démarrage"

etape "6. Le lanceur"
sudo install -Dm 0644 packaging/reverb.svg \
    /usr/local/share/icons/hicolor/scalable/apps/reverb.svg
sudo install -Dm 0644 packaging/reverb.desktop \
    /usr/local/share/applications/reverb.desktop
sudo update-desktop-database /usr/local/share/applications 2>/dev/null || true
sudo gtk-update-icon-cache /usr/local/share/icons/hicolor 2>/dev/null || true
pose "Reverb" "menu des applications"

etape "7. Vérification"
# Rendre la main sur un démon qui ne répond pas laisserait croire que c'est
# installé, et l'utilisateur découvrirait la panne en ouvrant la fenêtre.
for _ in $(seq 1 20); do
    [ -S "$SOCKET" ] && break
    sleep 0.5
done
if [ ! -S "$SOCKET" ]; then
    echo "   ❌ le socket $SOCKET n'est pas apparu"
    echo "      journalctl -u reverbd -n 30 --no-pager"
    exit 1
fi
pose "socket" "$SOCKET"
systemctl is-active reverbd | sed 's/^/   service : /'

echo
if [ "$GROUPE_ACTIF" -eq 0 ]; then
    echo "⚠️  Le groupe reverb vient d'être accordé, mais cette session ne l'a pas"
    echo "   encore. Sans lui, la fenêtre ne pourra pas parler au démon."
    echo
    echo "   Tout de suite, sans se déconnecter :   sg reverb -c reverb-gui"
    echo "   Définitivement :                       se déconnecter, se reconnecter"
else
    echo "✅ Installé. « reverb-gui », ou « Reverb » dans le menu des applications."
fi
