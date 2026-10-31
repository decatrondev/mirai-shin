#!/usr/bin/env bash
# Arma el complemento de prueba de video de Plasma (fase 2.0) y lo deja en
# https://<dominio>/flatpak/kde-video-probe.tar.gz. No se distribuye.
# Uso: tools/kde-video-probe/build.sh
set -euo pipefail
cd "$(dirname "$0")"

./make-videos.sh
PUBLISH_DIR=/var/www/html/miraishin-flatpak
mapfile -t BRAND < <(python3 -c '
import json; b = json.load(open("../../brand.json")); print(b["appId"]); print(b["name"])')
APP_ID=${BRAND[0]} NAME=${BRAND[1]}
ID="$APP_ID.videoprobe"

rm -rf dist && mkdir -p "dist/$ID"
cp -r contents "dist/$ID/"
APP_ID="$APP_ID" NAME="$NAME" ID="$ID" python3 - > "dist/$ID/metadata.json" <<'PY'
import json, os
n = os.environ["NAME"]
print(json.dumps({
    "KPackageStructure": "Plasma/Wallpaper",
    "KPlugin": {
        "Id": os.environ["ID"],
        "Name": f"{n} (video probe)",
        "Name[es]": f"{n} (prueba de video)",
        "Description": "Test video looping behind the desktop icons",
        "Description[es]": "Video de prueba en bucle detrás de los íconos",
        "Icon": "preferences-desktop-wallpaper",
        "License": "AGPL-3.0-or-later",
    },
    "X-KDE-ParentApp": "org.kde.plasmashell",
}, ensure_ascii=False, indent=4))
PY

tar -C dist -czf "dist/kde-video-probe.tar.gz" "$ID"
install -m 644 dist/kde-video-probe.tar.gz "$PUBLISH_DIR/"
echo "Publicado: $ID en $PUBLISH_DIR/kde-video-probe.tar.gz"
