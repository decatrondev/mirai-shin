// Prueba de la fase 3.0 (no se distribuye): ¿puede el complemento de fondo saber si hay una
// ventana en pantalla completa o maximizada en SU monitor? Usa la lista de ventanas de Plasma
// (org.kde.taskmanager). La lista va en otro archivo (Tasks.qml) y se carga con un Loader:
// si Plasma no deja usarla desde un fondo, el error se ve en pantalla en vez de un fondo vacío.
import QtQuick
import QtQuick.Window
import org.kde.plasma.plasmoid

WallpaperItem {
    id: root

    readonly property rect screenRect: Qt.rect(Screen.virtualX, Screen.virtualY, Screen.width, Screen.height)

    Rectangle {
        anchors.fill: parent
        color: "#1c1d21"
    }

    Loader {
        id: loader
        source: "Tasks.qml"
        onLoaded: item.screenGeometry = Qt.binding(() => root.screenRect)
        onStatusChanged: if (status === Loader.Error)
            console.warn("MIRAISHIN-PAUSE", Screen.name, "ERROR no se pudo cargar la lista de ventanas")
    }

    Text {
        anchors.centerIn: parent
        width: parent.width * 0.8
        wrapMode: Text.Wrap
        color: "#e8e8ea"
        font.pixelSize: 22
        text: "Mirai Shin (prueba de pausa) — monitor " + Screen.name + " "
            + root.screenRect.width + "x" + root.screenRect.height + "+" + root.screenRect.x + "+" + root.screenRect.y + "\n\n"
            + (loader.status === Loader.Error ? "No se pudo usar la lista de ventanas de Plasma."
               : loader.item ? loader.item.summary : "cargando…")
    }
}
