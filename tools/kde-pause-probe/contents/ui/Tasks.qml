// Lista de ventanas del monitor, del escritorio virtual y de la actividad actuales.
import QtQuick
import QtQuick.Window
import org.kde.taskmanager as TaskManager

Item {
    id: tasks

    property rect screenGeometry
    property string summary: ""

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

    // Sin filtros: para ver si la ventana falta en la lista o la quitan los filtros.
    TaskManager.TasksModel {
        id: all
        groupMode: TaskManager.TasksModel.GroupDisabled
    }

    Repeater {
        id: rows
        model: windows
        delegate: Item {
            readonly property string title: model.display || ""
            readonly property bool fullScreen: model.IsFullScreen === true
            readonly property bool maximized: model.IsMaximized === true
            readonly property bool minimized: model.IsMinimized === true
            readonly property bool active: model.IsActive === true
            onFullScreenChanged: refresh.restart()
            onMaximizedChanged: refresh.restart()
            onMinimizedChanged: refresh.restart()
            onActiveChanged: refresh.restart()
            onTitleChanged: refresh.restart()
        }
        onItemAdded: refresh.restart()
        onItemRemoved: refresh.restart()
    }

    onScreenGeometryChanged: refresh.restart()

    // Junta los cambios que llegan seguidos en una sola línea. Además se vuelve a mirar cada
    // segundo, por si algún cambio no avisa (es una prueba: el gasto no importa).
    property string lastLog: ""
    Timer {
        interval: 1000
        repeat: true
        running: true
        onTriggered: refresh.restart()
    }
    Timer {
        id: refresh
        interval: 200
        onTriggered: {
            let lines = [], full = false, max = false;
            for (let i = 0; i < rows.count; i++) {
                const w = rows.itemAt(i);
                if (!w) continue;
                if (w.fullScreen && !w.minimized) full = true;
                if (w.maximized && !w.minimized) max = true;
                lines.push("«" + w.title.slice(0, 40) + "» completa=" + w.fullScreen + " max=" + w.maximized
                    + " min=" + w.minimized + " activa=" + w.active);
            }
            tasks.summary = "ventanas: " + rows.count + " (sin filtros: " + all.count + ")" + " | pantalla completa: " + (full ? "SÍ" : "no")
                + " | maximizada: " + (max ? "SÍ" : "no") + " | escritorio " + desktops.currentDesktop
                + "\n" + lines.join("\n");
            if (tasks.summary !== tasks.lastLog) {
                tasks.lastLog = tasks.summary;
                console.warn("MIRAISHIN-PAUSE", Screen.name, tasks.summary.replace(/\n/g, " || "));
            }
        }
    }

    Component.onCompleted: refresh.restart()
}
