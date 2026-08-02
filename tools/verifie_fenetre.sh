#!/usr/bin/env bash
# Vérification de la fenêtre (issue #23).
#
# Ce que ce script vérifie et que je n'ai PAS pu vérifier moi-même : ce qui
# demande root, un écran, et un œil. Le reste — protocole, projection, dialogue
# avec le démon — est couvert par 460 tests qui tournent sans matériel.
#
# ⚠️ La fenêtre s'ouvre à l'étape 3 et NE SE FERME PAS toute seule. Le script
# attend que tu la fermes pour continuer.
#
# Le mot de passe sudo est demandé une fois, à l'étape 1.
#
# Usage : ./tools/verifie_fenetre.sh

set -u

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
NOTES="/tmp/reverb-observations-fenetre.txt"

: >"$NOTES"
pause() { echo; read -r -p "   ↳ $1 " r; printf '%s\t%s\n' "$2" "$r" >>"$NOTES"; }

echo "═══ 1. Installation ═══"
cd "$RACINE" || exit 1
cargo build --release 2>&1 | tail -1
sudo install -m 0755 target/release/reverbd /usr/local/bin/ || exit 1
sudo install -m 0755 target/release/reverb-gui /usr/local/bin/ || exit 1
sudo install -m 0644 packaging/reverbd.service /etc/systemd/system/ || exit 1
sudo install -Dm 0644 packaging/reverb.svg \
    /usr/local/share/icons/hicolor/scalable/apps/reverb.svg || exit 1
sudo install -Dm 0644 packaging/reverb.desktop \
    /usr/local/share/applications/reverb.desktop || exit 1
sudo update-desktop-database /usr/local/share/applications 2>/dev/null
sudo systemctl daemon-reload
sudo systemctl restart reverbd && echo "   ✅ démon et fenêtre installés"
sleep 2

if ! id -nG | grep -qw reverb; then
    echo "   Le groupe reverb n'est pas actif dans cette session, relance sous « sg »…"
    exec sg reverb "$0"
fi

echo
echo "═══ 2. Le lanceur ═══"
echo "   Cherche « Reverb » dans le menu des applications (touche Windows, puis tape)."
echo "   L'icône est un anneau de huit LED."
pause "L'entrée apparaît-elle dans le menu ? (oui/non)" "lanceur"

echo
echo "═══ 3. La fenêtre ═══"
echo "   Elle s'ouvre maintenant. REGARDE-LA, essaie tout, puis FERME-LA pour"
echo "   que le script continue."
echo
echo "   Ce qu'il y a à vérifier :"
echo "   • le boîtier se lit — l'arrière à GAUCHE, le radiateur en colonne au"
echo "     milieu, la RAM entre les deux, le haut en haut ;"
echo "   • les couleurs de la maquette bougent EN MÊME TEMPS que le boîtier ;"
echo "   • un clic sur une LED la vise (elle grossit et se cercle de blanc) ;"
echo "   • « Appliquer » avec une couleur change bien cette LED-là, et elle seule ;"
echo "   • le menu des animations montre CE QUI TOURNE DÉJÀ à l'ouverture,"
echo "     porte les six familles plus « aucune », et la phrase sous lui"
echo "     décrit celle qui est choisie ;"
echo "   • la vitesse et la direction changent le mouvement ;"
echo "   • les températures et les tours/minute se rafraîchissent ;"
echo "   • un curseur de ventilateur agit, « auto » rend la main au firmware."
echo
/usr/local/bin/reverb-gui

echo
pause "Le boîtier se lit-il — arrière à gauche, radiateur au milieu ? (oui/décrire)" "maquette"
pause "L'aperçu bouge-t-il en même temps que le boîtier ? (oui/décrire)" "aperçu-vivant"
pause "Le clic sur une LED, puis « Appliquer », change-t-il cette LED seule ? (oui/décrire)" "clic-led"
pause "Les six animations tournent-elles, et leur phrase décrit-elle ce que tu vois ? (décrire)" "animations"
pause "Les températures et vitesses s'affichent-elles et bougent-elles ? (oui/non)" "telemetrie"
pause "Les curseurs de ventilateur agissent-ils, et « auto » rend-il la main ? (oui/décrire)" "ventilateurs"
pause "L'interface est-elle restée fluide, y compris pendant une animation ? (oui/décrire)" "fluidite"

echo
echo "═══ 4. La fenêtre fermée n'éteint rien ═══"
echo "   Elle est fermée depuis un instant. Le boîtier doit continuer comme avant."
pause "Le boîtier est-il resté allumé/animé après la fermeture ? (oui/non)" "fermeture"

echo
echo "═══ 5. Démon absent : elle le dit ═══"
sudo systemctl stop reverbd
echo "   Le démon est arrêté. La fenêtre s'ouvre à nouveau : elle doit AFFICHER"
echo "   que le démon est injoignable, pas rester vide ni se fermer."
echo "   Ferme-la quand tu as vu."
/usr/local/bin/reverb-gui
sudo systemctl start reverbd
sleep 2
pause "A-t-elle dit clairement que le démon manquait ? (oui/décrire)" "demon-absent"

echo
echo "   Observations : $NOTES"
cat "$NOTES"
