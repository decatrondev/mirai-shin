// Prueba mínima (fase 1.1b): franjas como fondo de Plasma, para ver si los íconos
// quedan encima y los clics pasan. No se distribuye. Mismas franjas que layer-probe.
import QtQuick
import org.kde.plasma.plasmoid

WallpaperItem {
    id: root

    Canvas {
        anchors.fill: parent
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
        onPaint: {
            const ctx = getContext("2d");
            const stripe = 40;
            ctx.fillStyle = "#2f6f73";
            ctx.fillRect(0, 0, width, height);
            ctx.fillStyle = "#235356";
            // Franjas diagonales: celdas donde (x + y) / stripe es impar.
            for (let k = 1; k * stripe < width + height; k += 2) {
                const a = k * stripe, b = a + stripe;
                ctx.beginPath();
                ctx.moveTo(a, 0);
                ctx.lineTo(b, 0);
                ctx.lineTo(0, b);
                ctx.lineTo(0, a);
                ctx.closePath();
                ctx.fill();
            }
        }
    }
}
