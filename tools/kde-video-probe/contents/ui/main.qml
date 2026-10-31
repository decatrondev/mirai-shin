// Prueba de la fase 2.0: un video en bucle, sin sonido, como fondo de Plasma, con el
// MediaPlayer de QtMultimedia. Sirve para decidir si en KDE el video lo reproduce Plasma.
// No se distribuye. El video se elige con la clave `Video` (nombre de archivo en ../videos/).
import QtQuick
import QtMultimedia
import org.kde.plasma.plasmoid

WallpaperItem {
    id: root

    readonly property string videoName: root.configuration.Video || "1080p30-h264.mp4"

    // Grafito: se ve hasta que llega el primer cuadro (o si el video falla).
    Rectangle {
        anchors.fill: parent
        color: "#1c1d21"
    }

    VideoOutput {
        id: output
        anchors.fill: parent
        fillMode: VideoOutput.PreserveAspectCrop
    }

    // Sin `audioOutput`: no se conecta ninguna salida de sonido (se comprueba en la prueba).
    MediaPlayer {
        id: player
        source: Qt.resolvedUrl("../videos/" + root.videoName)
        loops: MediaPlayer.Infinite
        videoOutput: output
        onSourceChanged: play()
        onErrorOccurred: (error, errorString) => console.warn("MIRAISHIN-PROBE error", error, errorString)
        onPlaybackStateChanged: console.log("MIRAISHIN-PROBE estado", playbackState, source)
    }

    Component.onCompleted: player.play()

    // Si falla, se dice en pantalla (es una prueba: el texto no lleva traducción).
    Text {
        anchors.centerIn: parent
        visible: player.error !== MediaPlayer.NoError
        color: "#e8e8ea"
        font.pixelSize: 28
        text: "Mirai Shin (prueba de video): " + player.errorString + "\n" + root.videoName
    }
}
