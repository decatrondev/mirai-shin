#!/usr/bin/env bash
# Arma el complemento de prueba de Plasma (fase 1.1b) y lo deja en
# https://<dominio>/flatpak/kde-wallpaper-probe.tar.gz. No se distribuye.
# Uso: tools/kde-wallpaper-probe/build.sh
set -euo pipefail
cd "$(dirname "$0")"

PUBLISH_DIR=/var/www/html/miraishin-flatpak
mapfile -t BRAND < <(python3 -c '
import json; b = json.load(open("../../brand.json")); print(b["appId"]); print(b["name"])')
APP_ID=${BRAND[0]} NAME=${BRAND[1]}
ID="$APP_ID.probe"

rm -rf dist && mkdir -p "dist/$ID"
cp -r contents "dist/$ID/"
APP_ID="$APP_ID" NAME="$NAME" ID="$ID" python3 - > "dist/$ID/metadata.json" <<'PY'
import json, os
n = os.environ["NAME"]
print(json.dumps({
    "KPackageStructure": "Plasma/Wallpaper",
    "KPlugin": {
        "Id": os.environ["ID"],
        "Name": f"{n} (probe)",
        "Name[es]": f"{n} (prueba)",
        "Description": "Test stripes behind the desktop icons",
        "Description[es]": "Franjas de prueba detrás de los íconos",
        "Icon": "preferences-desktop-wallpaper",
        "License": "AGPL-3.0-or-later",
    },
    "X-KDE-ParentApp": "org.kde.plasmashell",
}, ensure_ascii=False, indent=4))
PY

tar -C dist -czf "dist/kde-wallpaper-probe.tar.gz" "$ID"
install -m 644 dist/kde-wallpaper-probe.tar.gz "$PUBLISH_DIR/"
echo "Publicado: $ID en $PUBLISH_DIR/kde-wallpaper-probe.tar.gz"
