#!/usr/bin/env bash
# Installe `packaging/60-reverb.rules` et vérifie qu'elle fait bien le travail.
#
# Deux pièges, rencontrés tous les deux à la première tentative :
#
#   • sur SHYNAEL, `reverb list` fonctionne DÉJÀ sans root, grâce à
#     `92-viia.rules` (MODE="0666" sur tous les hidraw) et aux règles d'OpenRGB.
#     Constater que ça marche ne prouve donc rien sur notre règle ;
#   • `udevadm test` ne journalise PAS les `TAG+=`. Seules les règles à effet de
#     bord (MODE=, RUN{builtin}+=) apparaissent avec leur fichier et leur ligne.
#     Chercher « 60-reverb.rules » dans sa sortie ne trouve que la ligne
#     « Reading rules file: … », qui dit qu'udev a LU le fichier — pas qu'une
#     règle a déclenché.
#
# D'où la vérification retenue : notre règle pose `TAG+="reverb"`, qui
# n'appartient qu'à ce dépôt. Le tag est présent sur le périphérique si et
# seulement si la règle a matché. C'est décisif, et ça se lit sans root.
#
# Usage : sudo ./tools/verifie_udev.sh

set -u

[ "$(id -u)" -eq 0 ] || { echo "Relance avec sudo." >&2; exit 1; }

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE="$RACINE/packaging/60-reverb.rules"
CIBLE="/etc/udev/rules.d/60-reverb.rules"

[ -f "$SOURCE" ] || { echo "Introuvable : $SOURCE" >&2; exit 1; }

echo "═══ Installation ═══"
cp "$SOURCE" "$CIBLE" && echo "  $CIBLE"
udevadm control --reload && udevadm trigger
echo "  règles rechargées"
sleep 2

# Nœud /dev/hidraw* d'un contrôleur, retrouvé par son identifiant produit.
# HID_ID s'écrit « 0003:00001E71:00002019 » : les identifiants y sont complétés
# à huit chiffres. C'est ce qui avait fait échouer la première version.
noeud_hidraw() {
    local produit="${1^^}" d
    for d in /sys/class/hidraw/hidraw*; do
        if grep -q "HID_ID=0003:00001E71:0000${produit}" "$d/device/uevent" 2>/dev/null; then
            echo "/dev/$(basename "$d")"
            return 0
        fi
    done
    return 1
}

# Nœud /dev/bus/usb/BBB/DDD d'un périphérique USB, par son identifiant produit.
noeud_usb() {
    local produit="${1,,}" d
    for d in /sys/bus/usb/devices/*; do
        [ "$(cat "$d/idVendor" 2>/dev/null)" = "1e71" ] || continue
        [ "$(cat "$d/idProduct" 2>/dev/null)" = "$produit" ] || continue
        printf '/dev/bus/usb/%03d/%03d\n' "$(cat "$d/busnum")" "$(cat "$d/devnum")"
        return 0
    done
    return 1
}

echo
echo "═══ Notre règle a-t-elle déclenché ? ═══"
echo "Le tag « reverb » n'est posé par aucune autre règle du système."

controle() {
    local noeud="$1" libelle="$2" tags

    if [ -z "$noeud" ] || [ ! -e "$noeud" ]; then
        echo "  ❌ $libelle : périphérique introuvable"
        return 1
    fi

    tags=$(udevadm info -q all -n "$noeud" 2>/dev/null | grep '^E: CURRENT_TAGS=')
    if echo "$tags" | grep -q ':reverb:'; then
        echo "  ✅ $libelle — $noeud"
        echo "       ${tags#E: }"
        return 0
    fi
    echo "  ❌ $libelle — $noeud : tag « reverb » absent"
    echo "       ${tags:-(aucun tag)}"
    return 1
}

ECHECS=0
controle "$(noeud_hidraw 2019 || true)" "contrôleur RGB + ventilateurs (2019)" || ECHECS=$((ECHECS + 1))
controle "$(noeud_hidraw 2012 || true)" "contrôleur RGB (2012)"                || ECHECS=$((ECHECS + 1))
controle "$(noeud_usb    300c || true)" "Kraken, endpoints bruts (300c)"        || ECHECS=$((ECHECS + 1))

echo
echo "═══ L'accès est-il réellement accordé ? ═══"
echo "uaccess délègue à systemd-logind, qui pose une ACL pour l'utilisateur connecté."
for n in "$(noeud_hidraw 2019 || true)" "$(noeud_hidraw 2012 || true)" "$(noeud_usb 300c || true)"; do
    [ -n "$n" ] && [ -e "$n" ] || continue
    acl=$(getfacl -p "$n" 2>/dev/null | grep '^user:.*:' | grep -v '^user::')
    echo "  $n : ${acl:-aucune ACL nominative}"
done

echo
echo "═══ Bilan ═══"
if [ "$ECHECS" -eq 0 ]; then
    echo "  Les trois périphériques portent le tag de ce dépôt : la règle fait le travail."
    echo "  Désinstaller OpenRGB ne casserait plus l'accès aux contrôleurs."
else
    echo "  $ECHECS périphérique(s) sans le tag « reverb » : la règle ne matche pas."
    echo "  Ne pas conclure que l'accès est acquis — il ne tiendrait que par les règles d'autrui."
fi
