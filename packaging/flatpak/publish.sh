#!/usr/bin/env bash
# Compila el Flatpak, lo firma y lo publica en https://<dominio>/flatpak/ (como Nexe).
# Uso: packaging/flatpak/publish.sh [rama]   (por defecto: beta; «stable» solo para versiones reales)
set -euo pipefail
cd "$(dirname "$0")"

BRANCH="${1:-beta}"
GPG_HOME="$HOME/.miraishin-flatpak-gpg"
PUBLISH_DIR=/var/www/html/miraishin-flatpak
GPG_KEY=$(gpg --homedir "$GPG_HOME" --list-secret-keys --with-colons | awk -F: '/^sec/{print $5; exit}')
[[ -n "$GPG_KEY" ]] || { echo "No hay llave de firma en $GPG_HOME" >&2; exit 1; }

# Identidad desde brand.json (única fuente del nombre).
mapfile -t BRAND < <(python3 -c '
import json; b = json.load(open("../../brand.json")); print(b["appId"]); print(b["name"]); print(b["domain"])')
APP_ID=${BRAND[0]} NAME=${BRAND[1]} DOMAIN=${BRAND[2]}
URL="https://$DOMAIN/flatpak/repo/"
GPG_B64=$(base64 -w0 miraishin-flatpak.gpg)

flatpak-builder --force-clean --default-branch="$BRANCH" \
  --gpg-sign="$GPG_KEY" --gpg-homedir="$GPG_HOME" \
  --repo=repo --disable-rofiles-fuse build "$APP_ID.yml"

flatpak build-update-repo --gpg-sign="$GPG_KEY" --gpg-homedir="$GPG_HOME" \
  --title="$NAME" --generate-static-deltas --prune repo

mkdir -p "$PUBLISH_DIR"
rsync -a --delete repo/ "$PUBLISH_DIR/repo/"

cat > "$PUBLISH_DIR/$APP_ID.flatpakref" <<REF
[Flatpak Ref]
Title=$NAME
Name=$APP_ID
Branch=$BRANCH
Url=$URL
SuggestRemoteName=miraishin
IsRuntime=false
RuntimeRepo=https://dl.flathub.org/repo/flathub.flatpakrepo
GPGKey=$GPG_B64
REF

cat > "$PUBLISH_DIR/miraishin.flatpakrepo" <<REPO
[Flatpak Repo]
Title=$NAME
Url=$URL
Homepage=https://$DOMAIN
DefaultBranch=$BRANCH
GPGKey=$GPG_B64
REPO

echo "Publicado ($BRANCH). En los equipos: flatpak update $APP_ID"
