// Ventanas que tapan el fondo de esta pantalla (fase 3.3), con la lista de ventanas de
// Plasma (org.kde.taskmanager). Solo cuenta las de esta pantalla, del escritorio virtual y
// de la actividad que se ven, y no las minimizadas. Va en su propio archivo y se carga con un
// Loader: si Plasma no deja usar la lista, el fondo sigue igual (sin pausa por ventanas).
// Probado en el 3.0: avisa 0,2–0,4 s después de cada cambio, igual que KWin.
import QtQuick
import org.kde.taskmanager as TaskManager

Item {
    id: tasks

    property rect screenGeometry
    // Hay alguna ventana en pantalla completa / maximizada encima del fondo.
    property bool fullScreen: false
    property bool maximized: false

    TaskManager.VirtualDesktopInfo { id: desktops }
    TaskManager.ActivityInfo { id: activities }

    TaskManager.TasksModel {
        id: windows
        groupMode: TaskManager.TasksModel.GroupDisabled
        filterByScreen: true
        filterByVirtualDesktop: true
        filterByActivity: true
        screenGeometry: tasks.screenGeometry
        virtualDesktop: desktops.currentDesktop
        activity: activities.currentActivity
    }

    Repeater {
        id: rows
        model: windows
        delegate: Item {
            readonly property bool fullScreen: model.IsFullScreen === true
            readonly property bool maximized: model.IsMaximized === true
            readonly property bool minimized: model.IsMinimized === true
            onFullScreenChanged: Qt.callLater(tasks.refresh)
            onMaximizedChanged: Qt.callLater(tasks.refresh)
            onMinimizedChanged: Qt.callLater(tasks.refresh)
        }
        onItemAdded: Qt.callLater(tasks.refresh)
        onItemRemoved: Qt.callLater(tasks.refresh)
    }

    // Se llama con Qt.callLater: junta los cambios seguidos y corre cuando la lista ya cambió.
    function refresh() {
        let full = false, max = false;
        for (let i = 0; i < rows.count; i++) {
            const w = rows.itemAt(i);
            if (!w || w.minimized)
                continue;
            if (w.fullScreen)
                full = true;
            if (w.maximized)
                max = true;
        }
        tasks.fullScreen = full;
        tasks.maximized = max;
    }

    Component.onCompleted: refresh()
}
