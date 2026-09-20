import QtQuick
import QtQuick.Controls
import QtQuick.Shapes

// A piano key: the cap travels into its well, latches while engaged, and can carry a lamp.
Button {
    id: key
    property string glyph: ""
    property string caption: ""
    property color ink: "#e1dfd5"
    property color face: "#303638"
    property color well: "#0c1012"
    property color lamp: ink
    property bool engaged: false
    property bool hasLamp: false
    property bool lit: false
    property int seekDirection: 0
    // On a tape the wind keys work like a CD player's: a tap is for the owner's onClicked (track
    // search), and only a hold winds. `wound` tells that click handler a hold already did its work.
    property bool trackSearch: false
    property bool wound: false
    readonly property bool winding: down && seekDirection !== 0 && (!trackSearch || wound)
    signal wind(int seconds)
    readonly property bool sunk: down || engaged
    // Icons live in a 28x24 box; "line" is stroked, "fill" is solid.
    readonly property var glyphs: ({
        "rew": { fill: "M14 4 L14 20 L2 12 Z M27 4 L27 20 L15 12 Z" },
        "play": { fill: "M8 2 L24 12 L8 22 Z" },
        "pause": { fill: "M7 3 h5 v18 h-5 Z M16 3 h5 v18 h-5 Z" },
        "fwd": { fill: "M1 4 L13 12 L1 20 Z M14 4 L26 12 L14 20 Z" },
        "stop": { fill: "M6 4 h16 v16 h-16 Z" },
        "prev": { fill: "M2 4 h3 v16 h-3 Z M16 4 L16 20 L6 12 Z M27 4 L27 20 L17 12 Z" },
        "next": { fill: "M1 4 L11 12 L1 20 Z M12 4 L22 12 L12 20 Z M23 4 h3 v16 h-3 Z" },
        "open": { fill: "M14 3 L25 15 L3 15 Z M3 18 h22 v3 h-22 Z" },
        "mark": { fill: "M8 3 h12 v18 l-6 -5 l-6 5 Z" },
        "loop": { fill: "M23.2 9.8 L22.8 3.2 L16.5 9.5 Z", line: "M21.7 14.1 A8 8 0 1 1 19.7 6.3" }
    })
    readonly property var art: glyphs[glyph] || ({})
    implicitWidth: 88; implicitHeight: 80
    padding: 0
    focusPolicy: Qt.NoFocus
    Accessible.name: caption
    contentItem: Item {}
    background: Item {
        Rectangle { anchors.fill: parent; radius: 6; color: key.well }
        Rectangle {
            id: cap
            y: key.sunk ? 4 : 0
            width: parent.width; height: parent.height - 5; radius: 5
            color: key.hovered && key.enabled ? Qt.lighter(key.face, 1.14) : key.face
            border.color: Qt.tint(key.face, Qt.alpha(key.ink, 0.22))
            Behavior on y { NumberAnimation { duration: 70 } }
            Rectangle { x: 6; y: 1; width: parent.width - 12; height: 1; color: Qt.alpha(key.ink, key.sunk ? 0.04 : 0.14) }
            Rectangle { x: 1; y: parent.height - 7; width: parent.width - 2; height: 6; radius: 4; color: Qt.alpha(key.well, key.sunk ? 0.2 : 0.45) }
            Item {
                anchors.fill: parent; opacity: key.enabled ? 1 : 0.35
                Shape {
                    x: (parent.width - width) / 2; y: 17; width: 28; height: 24
                    visible: key.glyph !== "help"
                    preferredRendererType: Shape.CurveRenderer
                    ShapePath { strokeColor: "transparent"; fillColor: key.ink; PathSvg { path: key.art.fill || "" } }
                    ShapePath { strokeColor: key.ink; strokeWidth: 2.4; fillColor: "transparent"; capStyle: ShapePath.RoundCap; PathSvg { path: key.art.line || "" } }
                }
                Text { visible: key.glyph === "help"; anchors.horizontalCenter: parent.horizontalCenter; y: 13; text: "?"; font.family: "monospace"; font.pixelSize: 24; font.weight: Font.Bold; color: key.ink }
                Text { anchors.horizontalCenter: parent.horizontalCenter; y: 53; text: key.caption; font.family: "monospace"; font.pixelSize: 9; font.letterSpacing: 1.2; color: key.ink; opacity: 0.72 }
            }
            Rectangle {
                visible: key.hasLamp && key.lit
                x: parent.width - 17; y: 5; width: 14; height: 14; radius: 7; color: Qt.alpha(key.lamp, 0.22)
            }
            Rectangle {
                visible: key.hasLamp
                x: parent.width - 13; y: 9; width: 6; height: 6; radius: 3
                color: key.lit ? key.lamp : key.well; border.color: key.lit ? key.lamp : Qt.alpha(key.ink, 0.25)
            }
        }
    }
    // Every press starts fresh: a hold that wound the tape must not swallow the tap after it.
    onPressed: { wound = false; if (seekDirection) { if (!trackSearch) wind(seekDirection * 5); delay.restart() } }
    onReleased: { delay.stop(); repeat.stop() }
    onCanceled: { delay.stop(); repeat.stop() }
    Timer { id: delay; interval: 350; onTriggered: if (key.down) { key.wound = true; repeat.start() } }
    Timer { id: repeat; interval: 100; repeat: true; onTriggered: if (key.down) key.wind(key.seekDirection * 2); else stop() }
}
