import QtQuick
import QtQuick.Controls

Button {
    id: key
    property string symbol: ""
    property string caption: ""
    property color ink: "#e1dfd5"
    property color face: "#303638"
    property bool engaged: false
    property int seekDirection: 0
    signal wind(int seconds)
    implicitWidth: 106; implicitHeight: 82
    focusPolicy: Qt.NoFocus
    Accessible.name: caption
    ToolTip.visible: hovered
    ToolTip.text: caption + (seekDirection ? " · tap 5s / hold to wind" : "")
    background: Item {
        Rectangle { anchors.fill: parent; radius: 5; color: "#0c1012" }
        Rectangle {
            x: 1; y: key.down || key.engaged ? 5 : 0
            width: parent.width - 2; height: parent.height - 6; radius: 4
            color: key.hovered ? Qt.lighter(key.face, 1.13) : key.face
            border.color: Qt.lighter(key.face, 1.35)
            Behavior on y { NumberAnimation { duration: 70 } }
            Rectangle { x: 8; y: 2; width: parent.width - 16; height: 1; color: key.ink; opacity: 0.08 }
        }
    }
    contentItem: Column {
        spacing: 8; topPadding: key.down || key.engaged ? 19 : 14
        Canvas {
            id: icon
            anchors.horizontalCenter: parent.horizontalCenter; width: 32; height: 25
            onPaint: {
                const c = getContext("2d"); c.reset(); c.fillStyle = key.ink
                function triangle(x, y, direction, size) {
                    c.beginPath(); c.moveTo(x, y); c.lineTo(x + direction * size, y + size / 2); c.lineTo(x, y + size); c.closePath(); c.fill()
                }
                if (key.caption === "OPEN") { c.beginPath(); c.moveTo(5, 16); c.lineTo(16, 3); c.lineTo(27, 16); c.closePath(); c.fill(); c.fillRect(5, 20, 22, 3) }
                else if (key.caption === "STOP") c.fillRect(8, 5, 16, 16)
                else if (key.caption === "PAUSE") { c.fillRect(8, 4, 5, 18); c.fillRect(19, 4, 5, 18) }
                else if (key.caption === "REW") { triangle(16, 6, -1, 14); triangle(30, 6, -1, 14) }
                else if (key.caption === "FWD") { triangle(2, 6, 1, 14); triangle(16, 6, 1, 14) }
                else triangle(10, 3, 1, 20)
            }
            Connections { target: key; function onCaptionChanged() { icon.requestPaint() } function onInkChanged() { icon.requestPaint() } }
        }
        Text { anchors.horizontalCenter: parent.horizontalCenter; text: key.caption; font.family: "monospace"; font.pixelSize: 9; font.letterSpacing: 1.3; color: key.ink; opacity: 0.7 }
    }
    onPressed: if (seekDirection) { wind(seekDirection * 5); delay.restart() }
    onReleased: { delay.stop(); repeat.stop() }
    onCanceled: { delay.stop(); repeat.stop() }
    Timer { id: delay; interval: 350; onTriggered: if (key.down) repeat.start() }
    Timer { id: repeat; interval: 100; repeat: true; onTriggered: if (key.down) key.wind(key.seekDirection * 2); else stop() }
}
