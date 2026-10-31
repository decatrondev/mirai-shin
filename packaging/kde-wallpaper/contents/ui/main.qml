// Complemento de fondo de Plasma: muestra lo que el daemon eligió para esta pantalla.
// El daemon escribe la configuración por D-Bus y Plasma avisa del cambio, así que aquí no se
// revisa nada a cada rato:
// - `Image`: file:// de un cuadro fijo, ya al tamaño exacto del monitor.
// - `Video` + `Mode`: file:// de un video que se reproduce aquí, en bucle y sin sonido, con
//   el MediaPlayer de QtMultimedia (prueba 2.0). `Mode` = fill, fit, stretch o center.
import QtQuick
import QtQuick.Window
import QtMultimedia
import org.kde.plasma.plasmoid

WallpaperItem {
    id: root

    readonly property string wantedImage: root.configuration.Image || ""
    readonly property string wantedVideo: root.configuration.Video || ""
    readonly property string mode: root.configuration.Mode || "fill"

    // Grafito del sistema de diseño: se ve hasta que llega algo, en los bordes de «ajustar»
    // y si el video no se puede reproducir.
    Rectangle {
        anchors.fill: parent
        color: "#1c1d21"
    }

    // ---- Imagen fija ----

    // Dos imágenes: la nueva carga por detrás y recién al estar lista reemplaza a la vieja,
    // para que al cambiar de fondo no se vea un parpadeo.
    property bool showingA: true

    function loadImage(url) {
        if (url === "") {
            // Pasando a video: la imagen se suelta cuando el video ya se ve (o falla).
            if (root.wantedVideo === "")
                root.releaseImages();
            return;
        }
        const next = showingA ? b : a;
        next.source = url;
    }

    function releaseImages() {
        a.source = "";
        b.source = "";
    }

    onWantedImageChanged: loadImage(wantedImage)
    Component.onCompleted: {
        loadImage(wantedImage);
        if (wantedVideo !== "")
            player.play();
    }

    Image {
        id: a
        anchors.fill: parent
        visible: root.showingA
        asynchronous: true
        cache: false
        fillMode: Image.Stretch
        onStatusChanged: {
            if (status === Image.Ready && !root.showingA) {
                root.showingA = true;
                b.source = "";
            }
        }
    }

    Image {
        id: b
        anchors.fill: parent
        visible: !root.showingA
        asynchronous: true
        cache: false
        fillMode: Image.Stretch
        onStatusChanged: {
            if (status === Image.Ready && root.showingA) {
                root.showingA = false;
                a.source = "";
            }
        }
    }

    // ---- Video ----

    // Tamaño real del video, para «centrar» (sin escalar).
    property size videoSize: Qt.size(0, 0)
    readonly property bool videoShown: wantedVideo !== "" && player.playing
        && player.error === MediaPlayer.NoError

    onVideoShownChanged: {
        if (videoShown)
            releaseImages();
    }

    Item {
        anchors.fill: parent
        clip: true
        visible: root.videoShown

        VideoOutput {
            id: output
            readonly property bool center: root.mode === "center" && root.videoSize.width > 0
            anchors.centerIn: parent
            width: center ? root.videoSize.width / Screen.devicePixelRatio : parent.width
            height: center ? root.videoSize.height / Screen.devicePixelRatio : parent.height
            fillMode: root.mode === "fit" ? VideoOutput.PreserveAspectFit
                : root.mode === "fill" ? VideoOutput.PreserveAspectCrop
                : VideoOutput.Stretch
        }
    }

    // Sin `audioOutput`: no se conecta ninguna salida de sonido.
    MediaPlayer {
        id: player
        source: root.wantedVideo
        loops: MediaPlayer.Infinite
        videoOutput: output
        onSourceChanged: {
            root.videoSize = Qt.size(0, 0);
            if (source.toString() !== "")
                play();
            else
                stop();
        }
        onMetaDataChanged: {
            const r = metaData.value(MediaMetaData.Resolution);
            if (r && r.width > 0)
                root.videoSize = r;
        }
        onErrorOccurred: (error, errorString) => {
            console.warn("Mirai Shin: video error", error, errorString, source);
            // Se ve el grafito, no un cuadro viejo.
            root.releaseImages();
        }
    }
}
