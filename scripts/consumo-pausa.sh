#!/usr/bin/env bash
# Presupuesto de consumo (fase 3.7): comprueba en un Sway sin pantalla que un video en pausa
# casi no gasta CPU, y que el límite de cuadros ahorra. Falla (salida 1) si no se cumple.
# El CI no tiene pantalla ni GPU: esto se corre a mano antes de subir un cambio que toque
# el video, la pausa o el bucle de dibujo.
#
# Uso: scripts/consumo-pausa.sh            (compila el daemon en modo debug si hace falta)
#      PRESUPUESTO=5 scripts/consumo-pausa.sh
# Necesita: sway, swaymsg, ffmpeg (con libvpx) y libmpv.
#
# Cifras de referencia (2026-11-09, llvmpipe, video VP9 640×360 a 60 cuadros, ticks del daemon
# cada 5 s): reproduciendo ~330, límite 15 ~170, en pausa 0–1.
set -u
cd "$(dirname "$0")/.."
PRESUPUESTO=${PRESUPUESTO:-5}   # ticks de CPU cada 5 s, como mucho, en pausa
ROOT=$(git rev-parse --show-toplevel)
for c in sway swaymsg ffmpeg; do
  command -v "$c" >/dev/null || { echo "Falta $c."; exit 2; }
done

echo "== Compilando el daemon…"
cargo build -q -p miraishin-daemon -p miraishin-ctl || exit 2
DAEMON="$ROOT/target/debug/miraishin-daemon"
CTL="$ROOT/target/debug/miraishin-ctl"

# Carpetas propias. La de ejecución va en /tmp con un nombre corto: el socket del IPC tiene
# un largo máximo.
WORK=$(mktemp -d)
RUN=$(mktemp -d /tmp/msw.XXXX)
chmod 700 "$RUN"
APP=$(sed -n 's/.*"appId": *"\([^"]*\)".*/\1/p' brand.json)
WALL="$WORK/data/$APP/wallpapers/v60"
mkdir -p "$WALL" "$WORK/config"
export XDG_RUNTIME_DIR=$RUN XDG_DATA_HOME=$WORK/data XDG_CONFIG_HOME=$WORK/config HOME=$WORK
export WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WAYLAND_DISPLAY=wayland-1
unset XDG_CURRENT_DESKTOP DISPLAY SWAYSOCK

SWAY_PID=""; DAEMON_PID=""
cleanup() {
  [ -n "$DAEMON_PID" ] && { "$CTL" shutdown >/dev/null 2>&1; sleep 1; kill "$DAEMON_PID" 2>/dev/null; }
  [ -n "$SWAY_PID" ] && kill "$SWAY_PID" 2>/dev/null
  wait 2>/dev/null
  rm -rf "$WORK" "$RUN"
}
trap cleanup EXIT

echo "== Preparando un video de 60 cuadros y un Sway sin pantalla…"
ffmpeg -hide_banner -loglevel error -f lavfi -i testsrc2=rate=60:size=640x360:duration=20 \
  -c:v libvpx-vp9 -deadline realtime -cpu-used 8 -b:v 0 -crf 40 "$WALL/v.webm" || exit 2
printf '{"schema_version":1,"id":"v60","type":"video","file":"v.webm","title":"v60"}\n' > "$WALL/manifest.json"
printf 'output HEADLESS-1 resolution 640x360\n' > "$WORK/sway.conf"
sway -c "$WORK/sway.conf" > "$WORK/sway.log" 2>&1 &
SWAY_PID=$!
for _ in $(seq 50); do [ -S "$RUN/wayland-1" ] && break; sleep 0.2; done
[ -S "$RUN/wayland-1" ] || { echo "Sway no arrancó:"; tail -5 "$WORK/sway.log"; exit 2; }
export SWAYSOCK=$(ls "$RUN"/sway-ipc.*.sock 2>/dev/null | head -1)

"$DAEMON" > "$WORK/daemon.log" 2>&1 &
DAEMON_PID=$!
for _ in $(seq 50); do "$CTL" ping >/dev/null 2>&1 && break; sleep 0.2; done
"$CTL" ping >/dev/null 2>&1 || { echo "El daemon no arrancó:"; tail -5 "$WORK/daemon.log"; exit 2; }
"$CTL" rescan >/dev/null
"$CTL" set v60 >/dev/null || { echo "No se pudo poner el video."; exit 2; }
sleep 4

ticks() {  # CPU del daemon en 5 s (ticks del sistema)
  local a b
  a=$(awk '{print $14+$15}' "/proc/$DAEMON_PID/stat"); sleep 5; b=$(awk '{print $14+$15}' "/proc/$DAEMON_PID/stat")
  echo $((b - a))
}
reason() { "$CTL" --json outputs | tr -d '\n ' | grep -o '"paused":[^,}]*' | head -1 | sed 's/"paused"://; s/"//g'; }

FALLAS=0
check() {  # nombre, ticks, condición (texto), resultado (0/1)
  if [ "$4" = 0 ]; then echo "   ok    $1: $2 ticks ($3)"; else echo "   FALLA $1: $2 ticks ($3)"; FALLAS=$((FALLAS + 1)); fi
}

echo "== Midiendo (ticks de CPU del daemon cada 5 s; presupuesto en pausa: $PRESUPUESTO)"
PLAY=$(ticks)
check "reproduciendo" "$PLAY" "más de 20: el video se mueve" "$([ "$PLAY" -gt 20 ]; echo $?)"

"$CTL" pause >/dev/null; sleep 1
T=$(ticks); R=$(reason)
check "en pausa a mano [$R]" "$T" "≤ $PRESUPUESTO" "$([ "$T" -le "$PRESUPUESTO" ] && [ "$R" = manual ]; echo $?)"

"$CTL" brightness -50 >/dev/null; sleep 1
T=$(ticks)
check "en pausa, cambiando el brillo" "$T" "≤ $PRESUPUESTO" "$([ "$T" -le "$PRESUPUESTO" ]; echo $?)"
"$CTL" brightness 0 >/dev/null
"$CTL" resume >/dev/null

swaymsg output HEADLESS-1 power off >/dev/null; sleep 1
T=$(ticks); R=$(reason)
check "pantalla apagada [$R]" "$T" "≤ $PRESUPUESTO" "$([ "$T" -le "$PRESUPUESTO" ] && [ "$R" = screen_off ]; echo $?)"
swaymsg output HEADLESS-1 power on >/dev/null; sleep 2

"$CTL" fps 15 >/dev/null; sleep 2
T=$(ticks)
LIM=$((PLAY * 3 / 4))
check "límite de 15 cuadros" "$T" "≤ $LIM, tres cuartos de lo que gasta reproduciendo" "$([ "$T" -le "$LIM" ]; echo $?)"
"$CTL" fps off >/dev/null; sleep 2

T=$(ticks)
check "otra vez reproduciendo" "$T" "más de 20" "$([ "$T" -gt 20 ]; echo $?)"

if [ "$FALLAS" -gt 0 ]; then
  echo "== $FALLAS comprobaciones fallaron. Registro del daemon:"
  sed 's/\x1b\[[0-9;]*m//g' "$WORK/daemon.log" | grep -aE "WARN|ERROR" | tail -10
  exit 1
fi
echo "== Todo dentro del presupuesto."
