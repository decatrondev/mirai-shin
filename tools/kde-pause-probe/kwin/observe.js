// Prueba de la fase 3.0 (no se distribuye): script de KWin que toma la primera ventana nueva,
// le cambia el estado sola cada 3 s (maximizada, pantalla completa, minimizada, otro escritorio
// virtual, normal, cerrada) y escribe en el registro lo que KWin ve de ella. Así se compara con
// lo que ve el fondo (MIRAISHIN-PAUSE) sin pedirle nada a la persona.
var before = workspace.windowList().map(function (w) { return String(w.internalId); });
var target = null, step = 0, homeDesktop = null;
function log(m) { console.warn("MIRAISHIN-KWIN " + m); }
function desc(w) {
    var d = w.onAllDesktops ? "todos" : w.desktops.map(function (x) { return x.name; }).join(",");
    return "«" + String(w.caption).slice(0, 30) + "» completa=" + w.fullScreen + " min=" + w.minimized
        + " activa=" + w.active + " monitor=" + (w.output ? w.output.name : "?") + " escritorio=" + d
        + " (actual: " + workspace.currentDesktop.name + ")";
}
var timer = new QTimer();
timer.interval = 3000;
timer.timeout.connect(function () {
    if (!target) {
        var ws = workspace.windowList();
        for (var i = 0; i < ws.length; i++)
            if (ws[i].normalWindow && before.indexOf(String(ws[i].internalId)) < 0) target = ws[i];
        if (!target) { log("esperando la ventana nueva"); return; }
    }
    log("kwin ve: " + desc(target));
    step++;
    switch (step) {
    case 1: target.setMaximize(true, true); log("paso 1: maximizar"); break;
    case 2: target.fullScreen = true; log("paso 2: pantalla completa"); break;
    case 3: target.minimized = true; log("paso 3: minimizar (sigue en pantalla completa)"); break;
    case 4: target.minimized = false; workspace.activeWindow = target; log("paso 4: volver a mostrarla (pantalla completa)"); break;
    case 5:
        homeDesktop = workspace.currentDesktop;
        var other = workspace.desktops.filter(function (d) { return d !== homeDesktop; })[0];
        if (other) { workspace.currentDesktop = other; log("paso 5: cambiar al escritorio virtual " + other.name + " (la ventana queda en el otro)"); }
        else log("paso 5: hay un solo escritorio virtual, se salta");
        break;
    case 6:
        if (homeDesktop) workspace.currentDesktop = homeDesktop;
        target.fullScreen = false; target.setMaximize(false, false);
        log("paso 6: volver al escritorio, ventana normal"); break;
    case 7: target.closeWindow(); log("paso 7: cerrar"); timer.stop(); break;
    }
});
timer.start();
log("script cargado");
