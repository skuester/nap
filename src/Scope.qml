import QtQuick
import QtQuick.Shapes

// The display between the reels. A click steps through its scenes: the cell oscilloscope, a spectrum
// analyzer with falling caps, and a pair of VU meters. All three work on the same 4px cell grid or
// the label's paper, so they belong to the tape rather than to a screen.
Rectangle {
    id: scope
    property string mode: "scope"
    property bool playing: false
    property var wave: []
    property var spectrum: []
    property var levels: []
    property color glow: "#89b4fa"
    property color fg: "#cdd6f4"
    property color paper: "#d9d5c5"
    property color ink: "#1c1213"
    property color stripe: "#684c59"
    property string mono: "monospace"
    signal cycled()
    readonly property int rows: 17
    readonly property int barCount: 24
    width: 208; height: 84; radius: 4

    // Oscilloscope: one column per pair of samples.
    property var trace: []
    function retrace() {
        const next = []
        for (let i = 0; i < 48; i++) {
            const v = wave.length > 2 * i + 1 ? (wave[2 * i] + wave[2 * i + 1]) / 2 : 0
            // A gentle curve keeps quiet passages moving without clipping loud ones.
            const shaped = Math.sign(v) * Math.pow(Math.min(1, Math.abs(v) * 1.4), 0.7)
            next.push(wave.length ? (trace[i] || 0) * 0.3 + shaped * 0.7 : 0)
        }
        trace = next
    }
    onWaveChanged: if (mode === "scope") retrace()

    // Analyzer and meters move on their own clock: bars drop, caps hang then fall, needles swing.
    property var bars: []
    property var caps: []
    property var hang: []
    property var drop: []
    property var needle: [0, 0]
    property var swing: [0, 0]
    property var over: [0, 0]
    property bool moving: false
    function step(dt) {
        let busy = false
        if (mode === "bars") {
            const b = [], c = [], h = [], d = []
            for (let i = 0; i < barCount; i++) {
                const target = spectrum[i] || 0
                b[i] = Math.max(target, (bars[i] || 0) - 1.5 * dt)
                c[i] = caps[i] || 0; h[i] = hang[i] || 0; d[i] = drop[i] || 0
                if (b[i] >= c[i]) { c[i] = b[i]; h[i] = 0.5; d[i] = 0 }
                else if (h[i] > 0) h[i] -= dt
                else { d[i] += 1.4 * dt; c[i] = Math.max(b[i], c[i] - d[i] * dt) }
                busy = busy || c[i] > 0.001
            }
            bars = b; caps = c; hang = h; drop = d
        } else if (mode === "vu") {
            const n = [], s = [], o = []
            for (let i = 0; i < 2; i++) {
                const target = playing ? (levels[i] || 0) : 0
                // A lightly underdamped spring: about 300ms to rise, with the real meter's hint of overshoot.
                s[i] = (swing[i] || 0) + (120 * (target - needle[i]) - 15 * (swing[i] || 0)) * dt
                n[i] = Math.max(-0.01, Math.min(1.1, needle[i] + s[i] * dt))
                o[i] = target > 0.708 ? 0.3 : Math.max(0, (over[i] || 0) - dt)
                busy = busy || n[i] > 0.002 || Math.abs(s[i]) > 0.002
            }
            needle = n; swing = s; over = o
        }
        moving = busy
    }
    onModeChanged: { bars = []; caps = []; hang = []; drop = []; needle = [0, 0]; swing = [0, 0]; over = [0, 0]; trace = [] }
    FrameAnimation {
        running: scope.mode !== "scope" && (scope.playing || scope.moving)
        onTriggered: scope.step(Math.min(0.05, frameTime))
    }

    Item {
        x: 9; y: 9; width: 191; height: 67
        visible: scope.mode === "scope"
        Repeater {
            model: 48
            Rectangle {
                required property int index
                readonly property int cells: Math.round((scope.trace[index] || 0) * 8)
                x: index * 4; y: (8 - Math.max(0, cells)) * 4
                width: 3; height: (Math.abs(cells) + 1) * 4 - 1
                color: scope.playing ? scope.glow : Qt.alpha(scope.fg, 0.22)
            }
        }
    }
    Item {
        x: 9; y: 9; width: 191; height: 67
        visible: scope.mode === "bars"
        Repeater {
            model: scope.barCount
            Item {
                id: band
                required property int index
                readonly property int cells: Math.round((scope.bars[index] || 0) * scope.rows)
                readonly property int capRow: Math.min(scope.rows, Math.max(cells, Math.round((scope.caps[index] || 0) * scope.rows)) + 1)
                x: index * 8; width: 6; height: parent.height
                Item {
                    y: 68 - 4 * band.cells; width: parent.width; height: Math.max(0, 4 * band.cells - 1); clip: true
                    Rectangle {
                        y: parent.height - height; width: parent.width; height: 67
                        gradient: Gradient {
                            GradientStop { position: 0; color: scope.fg }
                            GradientStop { position: 0.45; color: scope.glow }
                            GradientStop { position: 1; color: Qt.tint(scope.color, Qt.alpha(scope.glow, 0.3)) }
                        }
                    }
                }
                Rectangle { y: 68 - 4 * band.capRow; width: parent.width; height: 3; color: scope.playing || scope.moving ? scope.fg : Qt.alpha(scope.fg, 0.22) }
            }
        }
    }
    // The cell grid: gaps cut across whatever the scope or analyzer drew.
    Item {
        x: 9; y: 9; width: 191; height: 67
        visible: scope.mode !== "vu"
        Repeater {
            model: scope.rows - 1
            Rectangle { required property int index; y: index * 4 + 3; width: parent.width; height: 1; color: scope.color }
        }
    }

    component Meter: Rectangle {
        id: meter
        property real deflection: 0
        property bool peaking: false
        property string channel: ""
        // The scale is linear in voltage, as on the real instrument: 0 VU sits at 71% of the sweep.
        function angle(db) { return -40 + 80 * Math.pow(10, db / 20) / Math.pow(10, 3 / 20) }
        width: 93; height: 67; radius: 3; clip: true
        color: scope.paper; border.color: Qt.alpha(scope.ink, 0.5)
        Shape {
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: scope.ink; strokeWidth: 1.2; fillColor: "transparent"; capStyle: ShapePath.FlatCap
                PathAngleArc { centerX: 46.5; centerY: 82; radiusX: 56; radiusY: 56; startAngle: -130; sweepAngle: meter.angle(0) + 40 }
            }
            ShapePath {
                strokeColor: scope.stripe; strokeWidth: 5; fillColor: "transparent"; capStyle: ShapePath.FlatCap
                PathAngleArc { centerX: 46.5; centerY: 82; radiusX: 53.5; radiusY: 53.5; startAngle: -90 + meter.angle(0); sweepAngle: 40 - meter.angle(0) }
            }
        }
        Repeater {
            model: [-20, -10, -7, -5, -3, -2, -1, 0, 1, 2, 3]
            Item {
                required property int modelData
                x: 46.5; y: 82; rotation: meter.angle(modelData)
                Rectangle { x: -0.5; y: -(modelData % 5 === 0 || modelData === 3 ? 63 : 60); width: 1; height: modelData % 5 === 0 || modelData === 3 ? 7 : 4; color: scope.ink }
            }
        }
        Repeater {
            model: [[-20, "20"], [-10, "10"], [-5, "5"], [0, "0"]]
            Text {
                required property var modelData
                readonly property real radians: meter.angle(modelData[0]) * Math.PI / 180
                x: 46.5 + 70 * Math.sin(radians) - width / 2; y: 82 - 70 * Math.cos(radians) - height / 2
                text: modelData[1]; font.family: scope.mono; font.pixelSize: 7; color: scope.ink
            }
        }
        Text { anchors.horizontalCenter: parent.horizontalCenter; y: 40; text: "VU"; font.family: scope.mono; font.pixelSize: 9; font.weight: Font.Bold; color: Qt.alpha(scope.ink, 0.7) }
        Text { x: 6; y: 54; text: meter.channel; font.family: scope.mono; font.pixelSize: 8; font.weight: Font.Bold; color: Qt.alpha(scope.ink, 0.7) }
        Rectangle { x: parent.width - 12; y: 55; width: 6; height: 6; radius: 3; color: meter.peaking ? scope.stripe : Qt.alpha(scope.ink, 0.18) }
        Item {
            x: 46.5; y: 82; rotation: -40 + 80 * meter.deflection
            Rectangle { x: -0.75; y: -66; width: 1.5; height: 66; color: scope.ink; antialiasing: true }
        }
        Rectangle { anchors.horizontalCenter: parent.horizontalCenter; y: 61; width: 22; height: 12; radius: 6; color: scope.ink }
    }
    Row {
        x: 9; y: 9; spacing: 5
        visible: scope.mode === "vu"
        Meter { channel: "L"; deflection: scope.needle[0]; peaking: scope.over[0] > 0 }
        Meter { channel: "R"; deflection: scope.needle[1]; peaking: scope.over[1] > 0 }
    }

    MouseArea {
        anchors.fill: parent; cursorShape: Qt.PointingHandCursor
        onClicked: scope.cycled()
        Accessible.role: Accessible.Button; Accessible.name: "Change the visualizer"
    }
}
