// Complemento de fondo de Plasma: muestra lo que el daemon eligió para esta pantalla.
// El daemon escribe la configuración por D-Bus y Plasma avisa del cambio, así que aquí no se
// revisa nada a cada rato:
// - `Image`: file:// de un cuadro fijo, ya al tamaño exacto del monitor.
// - `Video` + `Mode`: file:// de un video que se reproduce aquí, en bucle y sin sonido, con
//   el MediaPlayer de QtMultimedia. `Mode` = fill, fit, stretch o center.
// - `Paused`: "true" = el video queda en pausa en su último cuadro (fase 3). Un video nuevo
//   en pausa se detiene en cuanto muestra su primer cuadro.
// - `PauseWhen`: con qué ventanas encima se pausa solo, sin esperar al daemon (fase 3.3):
//   "fullscreen", "maximized", las dos separadas por espacio, o vacío. Las ventanas las mira
//   Windows.qml.
// - `Speed`: velocidad del video, "1" = normal (fase 3.5).
// - `Brightness`: de "-1" (negro) a "1" (blanco), "0" = normal (fase 3.5). Se hace con una
//   capa negra o blanca encima del video, que no se dibuja cuando vale 0.
// Y deja escrito para el daemon (lo lee por D-Bus):
// - `Status`: estado del video: "" (sin video), `loading`, `playing`, o el código del problema
//   (`video_no_decoder`, `video_unreadable`, `video_unplayable`, `video_no_frames`,
//   `video_too_slow`).
// - `Fps`: cuadros por segundo que de verdad llegan a la pantalla, y los que trae el video
//   (se guarda solo cuando cambia bastante: cada escritura va al disco).
// - `Covered`: por qué ventana está en pausa solo: "fullscreen", "maximized" o vacío.
import QtQuick
import QtQuick.Window
import QtMultimedia
import org.kde.plasma.plasmoid

WallpaperItem {
    id: root

    readonly property string wantedImage: root.configuration.Image || ""
    readonly property string wantedVideo: root.configuration.Video || ""
    readonly property string mode: root.configuration.Mode || "fill"
    readonly property var pauseWhen: (root.configuration.PauseWhen || "").split(" ")
    readonly property real speed: Math.min(4, Math.max(0.25, Number(root.configuration.Speed) || 1))
    readonly property real brightness: Math.min(1, Math.max(-1, Number(root.configuration.Brightness) || 0))

    // ---- Ventanas encima (fase 3.3) ----
    readonly property rect screenRect: Qt.rect(Screen.virtualX, Screen.virtualY, Screen.width, Screen.height)
    Loader {
        id: windowsLoader
        source: "Windows.qml"
        onLoaded: item.screenGeometry = Qt.binding(() => root.screenRect)
        onStatusChanged: if (status === Loader.Error)
            console.warn("Mirai Shin: the window list is not available; no pause for full screen")
    }
    readonly property string covered: {
        const w = windowsLoader.item;
        if (!w || root.wantedVideo === "")
            return "";
        if (w.fullScreen && root.pauseWhen.indexOf("fullscreen") >= 0)
            return "fullscreen";
        if (w.maximized && root.pauseWhen.indexOf("maximized") >= 0)
            return "maximized";
        return "";
    }
    onCoveredChanged: save("Covered", covered)

    // En pausa por el daemon (a mano, batería…) o por una ventana encima.
    readonly property bool paused: root.configuration.Paused === "true" || covered !== ""

    onPausedChanged: {
        for (let i = 0; i < slots.count; i++)
            slots.itemAt(i).applyPause();
    }

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
        save("Covered", covered); // por si quedó algo de antes de reiniciar Plasma
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

    // Dos reproductores: el video nuevo arranca por detrás y recién cuando su primer cuadro
    // llega a la pantalla reemplaza al viejo, para que al cambiar no quede un hueco negro.
    // `current` es el que se ve (null si ninguno).
    property var current: null
    readonly property bool videoShown: current !== null

    // Lo que se escribe desde QML queda solo en memoria hasta `writeConfig()` (probado en
    // Plasma 6 el 2026-11-01): sin eso el daemon no lo ve.
    function save(key, value) {
        if (root.configuration[key] === value)
            return;
        root.configuration[key] = value;
        if (typeof root.configuration.writeConfig === "function")
            root.configuration.writeConfig();
    }

    function setStatus(s) {
        if (root.configuration.Status !== s)
            console.warn("Mirai Shin: status", s, root.wantedVideo);
        save("Status", s);
    }

    function loadVideo(url) {
        if (url === "") {
            for (let i = 0; i < slots.count; i++)
                slots.itemAt(i).unload();
            root.current = null;
            setStatus("");
            save("Fps", "");
            return;
        }
        if (slots.count < 2)
            return; // Aún no se crearon: el Repeater lo vuelve a pedir al terminar.
        const next = root.current === slots.itemAt(0) ? slots.itemAt(1) : slots.itemAt(0);
        next.load(url);
        save("Fps", "");
        setStatus("loading");
    }

    // Llamado por el reproductor que acaba de mostrar su primer cuadro.
    function promote(slot) {
        if (slot.url !== root.wantedVideo)
            return;
        const old = root.current;
        root.current = slot;
        if (old && old !== slot)
            old.unload();
        releaseImages();
        setStatus("playing");
    }

    // Llamado si el video pedido no se puede reproducir: queda el grafito (o la imagen, si
    // el daemon ya mandó su primer cuadro) y el código para el daemon.
    function fail(slot, code) {
        if (slot.url !== root.wantedVideo)
            return;
        if (root.current && root.current !== slot)
            root.current.unload();
        root.current = null;
        slot.unload();
        // Se ve el grafito, no un cuadro de otro fondo (el daemon puede mandar el primer
        // cuadro de este video como imagen después).
        releaseImages();
        setStatus(code);
    }

    onWantedVideoChanged: loadVideo(wantedVideo)

    Repeater {
        id: slots
        model: 2
        onCountChanged: if (count === 2) root.loadVideo(root.wantedVideo)

        delegate: Item {
            id: slot
            property string url: ""
            // Tamaño real del video, para «centrar» (sin escalar) y su ritmo de cuadros.
            property size videoSize: Qt.size(0, 0)
            property real videoFps: 0
            property int frames: 0
            property bool firstFrame: false
            property int slowWindows: 0
            property real savedFps: -100

            anchors.fill: parent
            clip: true
            visible: root.current === slot

            function load(u) {
                unload();
                url = u;
                player.source = u;
                player.play();
                noFrames.restart();
            }
            // En pausa no llegan cuadros: se deja de medir (si no, se daría por lento) y se
            // vuelve a empezar al reanudar. Antes del primer cuadro no se pausa todavía.
            function applyPause() {
                if (url === "" || !firstFrame)
                    return;
                if (root.paused) {
                    player.pause();
                    meter.stop();
                } else {
                    frames = 0;
                    slowWindows = 0;
                    player.play();
                    meter.restart();
                }
            }
            function unload() {
                noFrames.stop();
                meter.stop();
                player.stop();
                player.source = "";
                url = "";
                videoSize = Qt.size(0, 0);
                videoFps = 0;
                frames = 0;
                firstFrame = false;
                slowWindows = 0;
                savedFps = -100;
            }

            VideoOutput {
                id: output
                readonly property bool center: root.mode === "center" && slot.videoSize.width > 0
                anchors.centerIn: parent
                width: center ? slot.videoSize.width / Screen.devicePixelRatio : parent.width
                height: center ? slot.videoSize.height / Screen.devicePixelRatio : parent.height
                fillMode: root.mode === "fit" ? VideoOutput.PreserveAspectFit
                    : root.mode === "fill" ? VideoOutput.PreserveAspectCrop
                    : VideoOutput.Stretch
            }

            // Brillo: capa negra (oscurece) o blanca (aclara) sobre el video.
            Rectangle {
                anchors.fill: output
                visible: root.brightness !== 0
                color: root.brightness < 0 ? "black" : "white"
                opacity: Math.abs(root.brightness)
            }

            // Cada cuadro que de verdad llega a la pantalla.
            Connections {
                target: output.videoSink
                function onVideoFrameChanged() {
                    if (slot.url === "")
                        return;
                    slot.frames++;
                    if (!slot.firstFrame) {
                        slot.firstFrame = true;
                        noFrames.stop();
                        slot.frames = 0;
                        if (root.paused)
                            player.pause();
                        else
                            meter.restart();
                        root.promote(slot);
                    }
                }
            }

            // Sin un solo cuadro en este tiempo (y sin error): el equipo no lo puede mostrar.
            Timer {
                id: noFrames
                interval: 10000
                onTriggered: root.fail(slot, "video_no_frames")
            }

            // Cada 5 s: cuántos cuadros llegaron contra los que trae el video. Si dos veces
            // seguidas llega menos del 80 % **y** menos de 24 por segundo, el equipo no da
            // abasto (se ve a saltos). Medido en VMware el 2026-11-01: 17,8 de 30 se ve lento;
            // 35 de 60 se ve fluido (importan los cuadros por segundo, no el porcentaje).
            Timer {
                id: meter
                interval: 5000
                repeat: true
                onTriggered: {
                    const got = slot.frames / (interval / 1000);
                    slot.frames = 0;
                    // Se guarda solo si cambió más de un 20 % (cada escritura va al disco).
                    if (root.current === slot
                            && Math.abs(got - slot.savedFps) > Math.max(1, slot.savedFps * 0.2)) {
                        slot.savedFps = got;
                        root.save("Fps", got.toFixed(1) + "/" + slot.videoFps.toFixed(1));
                    }
                    // Con otra velocidad llegan más o menos cuadros que los del video.
                    const expected = slot.videoFps * root.speed;
                    if (expected > 0 && got < expected * 0.8 && got < 24) {
                        slot.slowWindows++;
                        if (slot.slowWindows === 2 && root.current === slot)
                            root.setStatus("video_too_slow");
                    } else {
                        slot.slowWindows = 0;
                        if (root.current === slot && root.configuration.Status === "video_too_slow")
                            root.setStatus("playing");
                    }
                }
            }

            // Sin `audioOutput`: no se conecta ninguna salida de sonido.
            MediaPlayer {
                id: player
                loops: MediaPlayer.Infinite
                videoOutput: output
                playbackRate: root.speed
                onMetaDataChanged: {
                    const r = metaData.value(MediaMetaData.Resolution);
                    if (r && r.width > 0)
                        slot.videoSize = r;
                    const f = metaData.value(MediaMetaData.VideoFrameRate);
                    if (f > 0)
                        slot.videoFps = f;
                }
                onErrorOccurred: (error, errorString) => {
                    console.warn("Mirai Shin: video error", error, errorString, source);
                    const code = error === MediaPlayer.FormatError ? "video_no_decoder"
                        : error === MediaPlayer.ResourceError ? "video_unreadable"
                        : "video_unplayable";
                    root.fail(slot, code);
                }
            }
        }
    }
}
