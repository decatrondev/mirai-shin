#!/usr/bin/env bash
# Genera los videos de prueba de la fase 2 en contents/videos/ (10 s cada uno).
# Una barra cruza la pantalla en exactamente 10 s, así el último cuadro empalma con el
# primero: si el bucle corta, se ve un salto o un parpadeo. El número de cuadro va al centro.
# Uso: tools/kde-video-probe/make-videos.sh
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p contents/videos
FONT=$(fc-match -f '%{file}' sans:bold)

make() { # nombre ancho alto fps códec
    local name=$1 w=$2 h=$3 fps=$4 codec=$5 out="contents/videos/$1"
    [[ -f $out ]] && return
    # overlay calcula x en cada cuadro (drawbox no lo hace en ffmpeg 6.1). Dos copias de la
    # barra (x y x-W) para que al salir por la derecha entre por la izquierda sin hueco.
    local fc="[1]split[b1][b2];[0][b1]overlay=x='mod(t/10\\,1)*W':y=0[o1]"
    fc+=";[o1][b2]overlay=x='mod(t/10\\,1)*W-W':y=0"
    fc+=",drawtext=fontfile=${FONT}:text='%{frame_num}':fontcolor=0xe8e8ea:fontsize=${h}/6:x=(w-tw)/2:y=(h-th)/2"
    local enc=(-c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p -movflags +faststart)
    [[ $codec == vp9 ]] && enc=(-c:v libvpx-vp9 -b:v 0 -crf 33 -row-mt 1 -pix_fmt yuv420p)
    ffmpeg -hide_banner -loglevel error -f lavfi -i "color=c=0x1c1d21:s=${w}x${h}:r=${fps}:d=10" \
        -f lavfi -i "color=c=0x2f6f73:s=$((w / 20))x${h}:r=${fps}:d=10" \
        -filter_complex "$fc" "${enc[@]}" -an "$out"
    echo "$out $(du -h "$out" | cut -f1)"
}

make 1080p30-h264.mp4 1920 1080 30 h264
make 1080p60-h264.mp4 1920 1080 60 h264
make 2160p30-h264.mp4 3840 2160 30 h264
make 1080p30-vp9.webm 1920 1080 30 vp9
