// Complemento de fondo de Plasma: muestra el cuadro que dibuja el daemon para esta pantalla.
// El daemon escribe la ruta en la configuración (`Image`) por D-Bus; Plasma avisa del cambio,
// así que aquí no se revisa nada a cada rato. El cuadro ya viene al tamaño exacto del monitor.
import QtQuick
import org.kde.plasma.plasmoid

WallpaperItem {
    id: root

    // Grafito del sistema de diseño: se ve solo hasta que llega el primer cuadro.
    Rectangle {
        anchors.fill: parent
        color: "#1c1d21"
    }

    // Dos imágenes: la nueva carga por detrás y recién al estar lista reemplaza a la vieja,
    // para que al cambiar de fondo no se vea un parpadeo.
    property bool showingA: true
    readonly property string wanted: root.configuration.Image || ""

    function load(url) {
        const next = showingA ? b : a;
        next.source = url;
    }

    onWantedChanged: load(wanted)
    Component.onCompleted: load(wanted)

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
}
