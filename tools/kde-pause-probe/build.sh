#!/usr/bin/env bash
# Arma el complemento de prueba de pausa de Plasma (fase 3.0) y lo deja en
# https://<dominio>/flatpak/kde-pause-probe.tar.gz. No se distribuye.
# Uso: tools/kde-pause-probe/build.sh
set -euo pipefail
cd "$(dirname "$0")"

PUBLISH_DIR=/var/www/html/miraishin-flatpak
mapfile -t BRAND < <(python3 -c '
import json; b = json.load(open("../../brand.json")); print(b["appId"]); print(b["name"])')
APP_ID=${BRAND[0]} NAME=${BRAND[1]}
ID="$APP_ID.pauseprobe"

rm -rf dist && mkdir -p "dist/$ID"
cp -r contents "dist/$ID/"
NAME="$NAME" ID="$ID" python3 - > "dist/$ID/metadata.json" <<'PY'
import json, os
n = os.environ["NAME"]
print(json.dumps({
    "KPackageStructure": "Plasma/Wallpaper",
    "KPlugin": {
        "Id": os.environ["ID"],
        "Name": f"{n} (pause probe)",
        "Name[es]": f"{n} (prueba de pausa)",
        "Description": "Lists the windows on this screen and whether one is fullscreen",
        "Description[es]": "Lista las ventanas de este monitor y si alguna está en pantalla completa",
        "Icon": "preferences-desktop-wallpaper",
        "License": "AGPL-3.0-or-later",
    },
    "X-KDE-ParentApp": "org.kde.plasmashell",
}, ensure_ascii=False, indent=4))
PY

tar -C dist -czf "dist/kde-pause-probe.tar.gz" "$ID"
install -m 644 dist/kde-pause-probe.tar.gz "$PUBLISH_DIR/"
install -m 644 kwin/observe.js "$PUBLISH_DIR/kde-pause-kwin.js"
echo "Publicado: $ID en $PUBLISH_DIR/kde-pause-probe.tar.gz"
