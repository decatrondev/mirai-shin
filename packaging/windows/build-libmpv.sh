#!/usr/bin/env bash
# libmpv para Windows (fase 2.8), toda LGPL: corre en MSYS2 (UCRT64), en el CI
# (`.github/workflows/windows-libmpv.yml`).
#
# - Compiladas aquí, con las mismas versiones fijas que el Flatpak: FFmpeg 7.1.2 (LGPL, solo
#   decodificar + VP9/WebM para las copias compatibles, con D3D11VA), libass 0.17.4,
#   libplacebo v7.387.0 y mpv v0.41.0 (`-Dgpl=false`, salida `gpu` con Direct3D 11).
# - De los paquetes de MSYS2 solo lo básico (versión exacta anotada en VERSIONES.txt).
#   Cualquier DLL de un paquete que no esté en ALLOWED hace fallar el script.
#
# Salida: dist/ con libmpv-2.dll, ffmpeg.exe, las DLL que necesitan, licenses/ y
# VERSIONES.txt.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
WORK="${WORK:-$ROOT/build}"
PREFIX="$WORK/prefix"
DIST="${DIST:-$ROOT/dist}"
JOBS="$(nproc)"

FFMPEG_URL=https://ffmpeg.org/releases/ffmpeg-7.1.2.tar.xz
FFMPEG_SHA=089bc60fb59d6aecc5d994ff530fd0dcb3ee39aa55867849a2bbc4e555f9c304
LIBASS_TAG=0.17.4
LIBASS_COMMIT=bbb3c7f1570a4a021e52683f3fbdf74fe492ae84
PLACEBO_TAG=v7.387.0
PLACEBO_COMMIT=275f7dea27924cdd34ab2e6c20e448b6bc4fab65
MPV_TAG=v0.41.0
MPV_COMMIT=41f6a645068483470267271e1d09966ca3b9f413

P=mingw-w64-ucrt-x86_64
# Paquetes de MSYS2 cuyas DLL pueden ir junto a libmpv (licencia entre paréntesis).
ALLOWED=(
  "$P-gcc-libs"      # libgcc/libstdc++ (GPL-3.0 con la excepción de runtime de GCC:
  "$P-libgcc"        # se pueden distribuir junto a programas de cualquier licencia).
  "$P-libstdc++"     # MSYS2 las separó de gcc-libs en dos paquetes.
  "$P-libwinpthread" # winpthreads (MIT/BSD)
  "$P-freetype"      # FreeType (FTL o GPL-2.0, se usa la FTL)
  "$P-fribidi"       # LGPL-2.1+
  "$P-harfbuzz"      # MIT
  "$P-graphite2"     # LGPL-2.1+ o MPL (dependencia de harfbuzz)
  "$P-glib2"         # LGPL-2.1+ (dependencia de harfbuzz)
  "$P-gettext-runtime" # libintl, LGPL-2.1+ (dependencia de glib)
  "$P-libiconv"      # LGPL-2.1+
  "$P-pcre2"         # BSD (dependencia de glib)
  "$P-libpng"        # libpng (dependencia de freetype)
  "$P-brotli"        # MIT (dependencia de freetype)
  "$P-bzip2"         # bzip2 (dependencia de freetype)
  "$P-zlib"          # zlib
  "$P-libffi"        # MIT (dependencia de glib)
  "$P-libvpx"        # BSD-3-Clause
  "$P-dav1d"         # BSD-2-Clause
  "$P-shaderc"       # Apache-2.0
  "$P-spirv-tools"   # Apache-2.0 (dependencia de shaderc)
  "$P-glslang"       # BSD/Apache (dependencia de shaderc)
  "$P-spirv-cross"   # Apache-2.0
)

echo "== Paquetes de MSYS2"
pacman -S --needed --noconfirm \
  git make diffutils \
  "$P-toolchain" "$P-meson" "$P-ninja" "$P-pkgconf" "$P-nasm" "$P-python" "$P-python-jinja" \
  "$P-freetype" "$P-fribidi" "$P-harfbuzz" "$P-zlib" \
  "$P-libvpx" "$P-dav1d" "$P-shaderc" "$P-spirv-cross"

mkdir -p "$WORK" "$PREFIX"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig"
export PATH="$PREFIX/bin:$PATH"

# Clona `tag` y comprueba que es el commit esperado.
fetch_git() { # url tag commit dir
  if [ ! -d "$WORK/$4" ]; then
    git clone --depth 1 --branch "$2" --recurse-submodules --shallow-submodules "$1" "$WORK/$4"
  fi
  local got
  got="$(git -C "$WORK/$4" rev-parse HEAD)"
  [ "$got" = "$3" ] || { echo "$4: commit $got, se esperaba $3"; exit 1; }
}

echo "== FFmpeg"
if [ ! -f "$PREFIX/bin/ffmpeg.exe" ]; then
  cd "$WORK"
  [ -f ffmpeg.tar.xz ] || curl -fsSL "$FFMPEG_URL" -o ffmpeg.tar.xz
  echo "$FFMPEG_SHA  ffmpeg.tar.xz" | sha256sum -c
  rm -rf ffmpeg && mkdir ffmpeg && tar xf ffmpeg.tar.xz -C ffmpeg --strip-components=1
  cd ffmpeg
  # Igual que el Flatpak; en vez de VA-API, D3D11VA. Sin autodetección: solo entra lo
  # que se pide aquí.
  ./configure --prefix="$PREFIX" \
    --enable-shared --disable-static \
    --disable-autodetect --enable-w32threads \
    --disable-ffplay --disable-ffprobe --disable-doc \
    --disable-encoders --enable-encoder=libvpx_vp9 --enable-libvpx \
    --disable-muxers --enable-muxer=webm \
    --disable-avdevice --disable-network \
    --enable-libdav1d --enable-zlib \
    --enable-d3d11va --enable-dxva2
  make -j"$JOBS"
  make install
  cd "$WORK"
fi
grep -q '^#define FFMPEG_LICENSE "LGPL version 2.1 or later"' "$WORK/ffmpeg/config.h" \
  || { echo "FFmpeg no quedó LGPL: $(grep FFMPEG_LICENSE "$WORK/ffmpeg/config.h")"; exit 1; }

echo "== libass"
fetch_git https://github.com/libass/libass.git "$LIBASS_TAG" "$LIBASS_COMMIT" libass
if [ ! -f "$PREFIX/lib/pkgconfig/libass.pc" ]; then
  meson setup "$WORK/libass/_b" "$WORK/libass" --prefix="$PREFIX" --buildtype=release \
    -Dtest=disabled -Dprofile=disabled -Dfontconfig=disabled -Ddirectwrite=enabled \
    -Dlibunibreak=disabled -Dasm=enabled
  meson install -C "$WORK/libass/_b"
fi

echo "== libplacebo"
fetch_git https://code.videolan.org/videolan/libplacebo.git "$PLACEBO_TAG" "$PLACEBO_COMMIT" libplacebo
if [ ! -f "$PREFIX/lib/pkgconfig/libplacebo.pc" ]; then
  # mpv lo necesita para sus utilidades; el dibujo en Windows es el D3D11 propio de mpv.
  meson setup "$WORK/libplacebo/_b" "$WORK/libplacebo" --prefix="$PREFIX" --buildtype=release \
    -Dvulkan=disabled -Dopengl=disabled -Dd3d11=disabled -Dglslang=disabled -Dshaderc=disabled \
    -Dlcms=disabled -Ddovi=disabled -Dlibdovi=disabled -Dunwind=disabled -Dxxhash=disabled \
    -Ddemos=false -Dtests=false
  meson install -C "$WORK/libplacebo/_b"
fi

echo "== mpv"
fetch_git https://github.com/mpv-player/mpv.git "$MPV_TAG" "$MPV_COMMIT" mpv
if [ ! -f "$PREFIX/bin/libmpv-2.dll" ]; then
  meson setup "$WORK/mpv/_b" "$WORK/mpv" --prefix="$PREFIX" --buildtype=release \
    -Dgpl=false -Dlibmpv=true -Dcplayer=false -Dauto_features=disabled \
    -Dgl=disabled -Dd3d11=enabled -Dd3d-hwaccel=enabled -Dshaderc=enabled -Dspirv-cross=enabled \
    -Dwin32-threads=enabled -Dzlib=enabled \
    -Dmanpage-build=disabled -Dhtml-build=disabled -Dpdf-build=disabled
  meson install -C "$WORK/mpv/_b"
fi
python -c "import json,sys; o={x['name']: x['value'] for x in json.load(open(sys.argv[1]))}; sys.exit(0 if o['gpl'] is False else 1)" \
  "$WORK/mpv/_b/meson-info/intro-buildoptions.json" || { echo "mpv no quedó LGPL"; exit 1; }

echo "== Armar dist/"
rm -rf "$DIST" && mkdir -p "$DIST/licenses"
cp "$PREFIX/bin/libmpv-2.dll" "$PREFIX/bin/ffmpeg.exe" "$DIST/"

# Todas las DLL que hacen falta (de nuestro prefix o de MSYS2), sin las de Windows.
deps() { ldd "$@" | awk '{print $3}' | grep -iE "^($PREFIX|/ucrt64)/" | sort -u; }
todo=("$DIST/libmpv-2.dll" "$DIST/ffmpeg.exe")
while :; do
  added=0
  for dll in $(deps "${todo[@]}"); do
    name="$(basename "$dll")"
    if [ ! -f "$DIST/$name" ]; then
      cp "$dll" "$DIST/"
      todo+=("$DIST/$name")
      added=1
    fi
  done
  [ "$added" = 1 ] || break
done

{
  echo "Mirai Shin — libmpv para Windows (LGPL)"
  echo "Compilado: FFmpeg 7.1.2, libass $LIBASS_TAG ($LIBASS_COMMIT), libplacebo $PLACEBO_TAG ($PLACEBO_COMMIT), mpv $MPV_TAG ($MPV_COMMIT)"
  echo
  echo "Paquetes de MSYS2 de los que salen DLL:"
} > "$DIST/VERSIONES.txt"

bad=0
for f in "$DIST"/*.dll; do
  name="$(basename "$f")"
  [ -f "$PREFIX/bin/$name" ] && continue
  pkg="$(pacman -Qoq "/ucrt64/bin/$name")"
  printf '  %-40s %s\n' "$name" "$(pacman -Q "$pkg")" >> "$DIST/VERSIONES.txt"
  if [[ ! " ${ALLOWED[*]} " =~ " $pkg " ]]; then
    echo "NO PERMITIDO: $name viene de $pkg ($(pacman -Qi "$pkg" | sed -n 's/^Licenses *: //p'))"
    bad=1
  fi
  short="${pkg#"$P"-}"
  if [ -d "/ucrt64/share/licenses/$short" ] && [ ! -d "$DIST/licenses/$short" ]; then
    cp -r "/ucrt64/share/licenses/$short" "$DIST/licenses/"
  fi
done
[ "$bad" = 0 ] || exit 1

mkdir -p "$DIST/licenses/ffmpeg" "$DIST/licenses/libass" "$DIST/licenses/libplacebo" "$DIST/licenses/mpv"
cp "$WORK/ffmpeg/LICENSE.md" "$WORK/ffmpeg/COPYING.LGPLv2.1" "$DIST/licenses/ffmpeg/"
cp "$WORK/libass/COPYING" "$DIST/licenses/libass/"
cp "$WORK/libplacebo/LICENSE" "$DIST/licenses/libplacebo/"
cp "$WORK/mpv/LICENSE.LGPL" "$WORK/mpv/Copyright" "$DIST/licenses/mpv/"

ls "$DIST"
for need in libmpv-2.dll ffmpeg.exe avcodec-61.dll avformat-61.dll avutil-59.dll '*ass*.dll' '*placebo*.dll'; do
  compgen -G "$DIST/$need" > /dev/null || { echo "Falta $need en dist/"; exit 1; }
done
strip --strip-unneeded "$DIST"/*.dll "$DIST"/*.exe
# Prueba rápida de las copias compatibles: 1 s de video de prueba a VP9/WebM.
"$DIST/ffmpeg.exe" -nostdin -hide_banner -loglevel error -f lavfi -i testsrc=size=320x180:duration=1 \
  -c:v libvpx-vp9 -deadline realtime -cpu-used 8 -f webm "$WORK/prueba.webm"
[ -s "$WORK/prueba.webm" ] || { echo "ffmpeg no pudo crear la copia VP9"; exit 1; }
echo
cat "$DIST/VERSIONES.txt"
ls -la "$DIST"
du -sh "$DIST"
